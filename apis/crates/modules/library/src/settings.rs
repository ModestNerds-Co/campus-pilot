//! Library policy and locked copy-accession sequence operations.

use anyhow::{Context, Result, anyhow, bail};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_finance::ledger::CurrencyOps;
use serde_json::{Map, Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{LibrarySettingsResponse, UpdateLibrarySettingsRequest, models::SettingsRow};

const EXHAUSTED_ACCESSION_SEQUENCE: i64 = 100_000_000;

pub struct LibrarySettingsOps;

impl LibrarySettingsOps {
    pub async fn get(pool: &PgPool, tenant_id: Uuid) -> Result<LibrarySettingsResponse> {
        let row = sqlx::query_as::<_, SettingsRow>(SETTINGS_SELECT)
            .bind(tenant_id)
            .fetch_optional(pool)
            .await
            .context("Failed to load Library settings")?
            .ok_or_else(|| anyhow!("Library settings were not provisioned"))?;
        let currency = if let Some(currency_id) = row.fine_currency_id {
            CurrencyOps::get_by_id(pool, tenant_id, currency_id).await?
        } else {
            None
        };
        Ok(response(row, currency.as_ref()))
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateLibrarySettingsRequest,
    ) -> Result<LibrarySettingsResponse> {
        let actor_id = person_actor_id(actor)?;
        let prefix = normalize_prefix(&request.accession_prefix)?;
        if request.overdue_fine_minor > 0 && request.fine_currency_id.is_none() {
            bail!("Choose a fine currency before enabling overdue fines");
        }
        if let Some(currency_id) = request.fine_currency_id {
            let currency = CurrencyOps::get_by_id(pool, tenant_id, currency_id)
                .await?
                .ok_or_else(|| anyhow!("The fine currency was not found"))?;
            if currency.status != "active" {
                bail!("The fine currency must be active");
            }
        }
        if request.accession_next_sequence >= EXHAUSTED_ACCESSION_SEQUENCE {
            bail!("The accession sequence is exhausted");
        }
        let preview = render_accession(
            &prefix,
            request.accession_next_sequence,
            request.accession_padding,
        )?;
        let collision = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM library_copies
                 WHERE tenant_id = $1 AND LOWER(accession_number) = LOWER($2)
                   AND deleted_at IS NULL
            )
            "#,
        )
        .bind(tenant_id)
        .bind(&preview)
        .fetch_one(pool)
        .await
        .context("Failed to validate the next accession number")?;
        if collision {
            bail!("The next accession number already exists; advance the sequence");
        }

        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Library settings update")?;
        sqlx::query(
            r#"
            UPDATE library_settings
               SET accession_prefix = $2, accession_next_sequence = $3,
                   accession_padding = $4, learner_loan_days = $5,
                   employee_loan_days = $6, default_loan_limit = $7,
                   maximum_renewals = $8, fine_currency_id = $9,
                   overdue_fine_minor = $10, updated_by = $11, updated_at = NOW()
             WHERE tenant_id = $1
            "#,
        )
        .bind(tenant_id)
        .bind(&prefix)
        .bind(request.accession_next_sequence)
        .bind(request.accession_padding)
        .bind(request.learner_loan_days)
        .bind(request.employee_loan_days)
        .bind(request.default_loan_limit)
        .bind(request.maximum_renewals)
        .bind(request.fine_currency_id)
        .bind(request.overdue_fine_minor)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to update Library settings")?;
        append_event(
            &mut transaction,
            LibraryActivityEvent {
                tenant_id,
                aggregate_id: tenant_id,
                aggregate_type: "settings",
                event_type: "updated",
                actor_id,
                reason: None,
                metadata: json!({ "next_accession_preview": preview }),
            },
        )
        .await?;
        append_domain_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "library.settings.update",
            "library_settings",
            tenant_id,
            json!({ "next_accession_preview": preview }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Library settings")?;
        Self::get(pool, tenant_id).await
    }
}

pub(crate) async fn allocate_accession(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
) -> Result<String> {
    let row = sqlx::query_as::<_, (String, i64, i16)>(
        r#"
        SELECT accession_prefix, accession_next_sequence, accession_padding
          FROM library_settings WHERE tenant_id = $1 FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock the Library accession sequence")?
    .ok_or_else(|| anyhow!("Library settings were not provisioned"))?;
    if row.1 >= EXHAUSTED_ACCESSION_SEQUENCE {
        bail!("The accession sequence is exhausted");
    }
    let accession = render_accession(&row.0, row.1, row.2)?;
    sqlx::query(
        r#"
        UPDATE library_settings
           SET accession_next_sequence = accession_next_sequence + 1,
               updated_at = NOW()
         WHERE tenant_id = $1
        "#,
    )
    .bind(tenant_id)
    .execute(&mut **transaction)
    .await
    .context("Failed to advance the Library accession sequence")?;
    Ok(accession)
}

pub(crate) struct LibraryActivityEvent<'a> {
    pub tenant_id: Uuid,
    pub aggregate_id: Uuid,
    pub aggregate_type: &'a str,
    pub event_type: &'a str,
    pub actor_id: Uuid,
    pub reason: Option<&'a str>,
    pub metadata: Value,
}

pub(crate) async fn append_event(
    transaction: &mut Transaction<'_, Postgres>,
    event: LibraryActivityEvent<'_>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO library_activity_events (
            tenant_id, aggregate_type, aggregate_id, event_type,
            actor_id, reason, metadata
        ) VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(event.tenant_id)
    .bind(event.aggregate_type)
    .bind(event.aggregate_id)
    .bind(event.event_type)
    .bind(event.actor_id)
    .bind(event.reason)
    .bind(event.metadata)
    .execute(&mut **transaction)
    .await
    .context("Failed to append Library activity evidence")?;
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "audit evidence is intentionally explicit"
)]
pub(crate) async fn append_domain_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    action: &str,
    target_type: &str,
    target_id: Uuid,
    metadata: Value,
) -> Result<()> {
    append_audit(
        &mut **transaction,
        &NewAuditEvent::new(
            tenant_id,
            actor,
            action,
            AuditOutcome::Succeeded,
            request_context,
        )
        .with_target(AuditTarget::new(target_type, target_id.to_string()))
        .with_redacted_metadata(object(metadata)),
    )
    .await
    .context("Failed to append Library audit evidence")?;
    Ok(())
}

pub(crate) fn person_actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("Authenticated person actor is required"))
}

pub(crate) fn render_accession(prefix: &str, sequence: i64, padding: i16) -> Result<String> {
    if !(1..EXHAUSTED_ACCESSION_SEQUENCE).contains(&sequence) {
        bail!("The accession sequence is outside the issuable range");
    }
    let width = usize::try_from(padding).context("Accession padding is invalid")?;
    if !(1..=8).contains(&width) {
        bail!("Accession padding must be between 1 and 8");
    }
    Ok(format!("{prefix}-{sequence:0width$}"))
}

fn normalize_prefix(value: &str) -> Result<String> {
    let value = value.trim().to_ascii_uppercase();
    if value.is_empty()
        || value.len() > 16
        || !value.chars().enumerate().all(|(index, character)| {
            character.is_ascii_uppercase()
                || character.is_ascii_digit()
                || (character == '-' && index > 0)
        })
    {
        bail!("The accession prefix must use uppercase letters, numbers, or hyphens");
    }
    Ok(value)
}

fn response(
    row: SettingsRow,
    currency: Option<&cp_finance::ledger::CurrencyResponse>,
) -> LibrarySettingsResponse {
    let preview = render_accession(
        &row.accession_prefix,
        row.accession_next_sequence,
        row.accession_padding,
    )
    .unwrap_or_else(|_| "Sequence exhausted".to_string());
    LibrarySettingsResponse {
        accession_prefix: row.accession_prefix,
        accession_next_sequence: row.accession_next_sequence,
        accession_padding: row.accession_padding,
        next_accession_preview: preview,
        learner_loan_days: row.learner_loan_days,
        employee_loan_days: row.employee_loan_days,
        default_loan_limit: row.default_loan_limit,
        maximum_renewals: row.maximum_renewals,
        fine_currency_id: row.fine_currency_id,
        fine_currency_code: currency.map(|value| value.code.clone()),
        fine_currency_minor_units: currency.map(|value| value.minor_units),
        overdue_fine_minor: row.overdue_fine_minor,
        updated_at: row.updated_at,
    }
}

fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

const SETTINGS_SELECT: &str = r#"
SELECT settings.accession_prefix, settings.accession_next_sequence,
       settings.accession_padding, settings.learner_loan_days,
       settings.employee_loan_days, settings.default_loan_limit,
       settings.maximum_renewals, settings.fine_currency_id,
       settings.overdue_fine_minor, settings.updated_at
  FROM library_settings AS settings
 WHERE settings.tenant_id = $1
"#;

#[cfg(test)]
mod tests {
    use super::{normalize_prefix, render_accession};

    #[test]
    fn accession_renderer_uses_configured_padding() {
        assert_eq!(render_accession("LIB", 42, 6).unwrap(), "LIB-000042");
    }

    #[test]
    fn accession_prefix_is_normalized_and_bounded() {
        assert_eq!(normalize_prefix(" lib-main ").unwrap(), "LIB-MAIN");
        assert!(normalize_prefix("LIB space").is_err());
        assert!(normalize_prefix("-LIB").is_err());
    }
}
