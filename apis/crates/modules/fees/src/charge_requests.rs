//! Typed intake for charges assessed by other licensed campus modules.
//!
//! A charge request is not an invoice, balance, receipt, or payment. Fees owns
//! this pending work item and may later accept, reject, or invoice it through a
//! dedicated Fees workflow. Source modules never write Fees tables directly.

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Utc};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_finance::ledger::CurrencyOps;
use serde::Serialize;
use serde_json::{Map, Value, json};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::foundation::BillingAccountOps;

/// Stable Fees-owned projection returned to an authorised source module.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ChargeRequestResponse {
    pub id: Uuid,
    pub billing_account_id: Uuid,
    pub currency_id: Uuid,
    pub source_module: String,
    pub source_record_id: Uuid,
    pub description: String,
    pub amount_minor: i64,
    pub status: String,
    pub submitted_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Fees-owned boundary used by source modules to submit idempotent charge work.
pub struct FeesChargeRequestOps;

impl FeesChargeRequestOps {
    #[allow(
        clippy::too_many_arguments,
        reason = "charge provenance is intentionally explicit"
    )]
    pub async fn submit(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        billing_account_id: Uuid,
        currency_id: Uuid,
        source_module: &str,
        source_record_id: Uuid,
        description: &str,
        amount_minor: i64,
    ) -> Result<ChargeRequestResponse> {
        let actor_id = actor
            .user_id()
            .ok_or_else(|| anyhow!("Authenticated person actor is required"))?;
        let description = validate_charge_input(source_module, description, amount_minor)?;
        let billing = BillingAccountOps::get_by_id(pool, tenant_id, billing_account_id, None)
            .await?
            .ok_or_else(|| anyhow!("The learner billing account was not found"))?;
        if billing.status != "active" {
            bail!("The learner billing account must be active");
        }
        let currency = CurrencyOps::get_by_id(pool, tenant_id, currency_id)
            .await?
            .ok_or_else(|| anyhow!("The fine currency was not found"))?;
        if currency.status != "active" {
            bail!("The fine currency must be active");
        }
        if let Some(existing) =
            Self::get_by_source(pool, tenant_id, source_module, source_record_id).await?
        {
            return Ok(existing);
        }

        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Fees charge request")?;
        let request = sqlx::query_as::<_, ChargeRequestResponse>(
            r#"
            INSERT INTO fees_charge_requests (
                tenant_id, billing_account_id, currency_id, source_module,
                source_record_id, description, amount_minor, submitted_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (tenant_id, source_module, source_record_id)
            DO UPDATE SET source_record_id = EXCLUDED.source_record_id
            RETURNING id, billing_account_id, currency_id, source_module,
                      source_record_id, description, amount_minor, status,
                      submitted_by, created_at, updated_at
            "#,
        )
        .bind(tenant_id)
        .bind(billing_account_id)
        .bind(currency_id)
        .bind(source_module)
        .bind(source_record_id)
        .bind(description)
        .bind(amount_minor)
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to submit the Fees charge request")?;

        let metadata = json!({
            "source_module": source_module,
            "source_record_id": source_record_id,
            "billing_account_id": billing_account_id,
            "currency_id": currency_id,
            "amount_minor": amount_minor,
            "status": request.status,
        });
        append_audit(
            &mut *transaction,
            &NewAuditEvent::new(
                tenant_id,
                actor,
                "fees.charge_requests.submit",
                AuditOutcome::Succeeded,
                request_context,
            )
            .with_target(AuditTarget::new(
                "fees_charge_request",
                request.id.to_string(),
            ))
            .with_redacted_metadata(object(metadata)),
        )
        .await
        .context("Failed to append Fees charge-request audit evidence")?;
        transaction
            .commit()
            .await
            .context("Failed to commit the Fees charge request")?;
        Ok(request)
    }

    pub async fn get_by_source(
        pool: &PgPool,
        tenant_id: Uuid,
        source_module: &str,
        source_record_id: Uuid,
    ) -> Result<Option<ChargeRequestResponse>> {
        sqlx::query_as::<_, ChargeRequestResponse>(
            r#"
            SELECT id, billing_account_id, currency_id, source_module,
                   source_record_id, description, amount_minor, status,
                   submitted_by, created_at, updated_at
              FROM fees_charge_requests
             WHERE tenant_id = $1 AND source_module = $2 AND source_record_id = $3
            "#,
        )
        .bind(tenant_id)
        .bind(source_module)
        .bind(source_record_id)
        .fetch_optional(pool)
        .await
        .context("Failed to load the Fees charge request")
    }
}

fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

fn validate_charge_input<'a>(
    source_module: &str,
    description: &'a str,
    amount_minor: i64,
) -> Result<&'a str> {
    if source_module != "library" {
        bail!("The charge source module is not supported");
    }
    let description = description.trim();
    if description.is_empty() || description.len() > 500 {
        bail!("The charge description is invalid");
    }
    if !(1..=9_000_000_000_000_000).contains(&amount_minor) {
        bail!("The charge amount is invalid");
    }
    Ok(description)
}

#[cfg(test)]
mod tests {
    use super::validate_charge_input;

    #[test]
    fn charge_input_rejects_unknown_sources_and_non_positive_amounts() {
        assert_eq!(
            validate_charge_input("fleet", "Damage", 100)
                .expect_err("unknown sources must fail")
                .to_string(),
            "The charge source module is not supported"
        );
        assert_eq!(
            validate_charge_input("library", "Damage", 0)
                .expect_err("zero amount must fail")
                .to_string(),
            "The charge amount is invalid"
        );
    }

    #[test]
    fn charge_input_trims_valid_library_descriptions() {
        assert_eq!(
            validate_charge_input("library", "  Lost copy  ", 1500)
                .expect("valid Library charge must pass"),
            "Lost copy"
        );
    }
}
