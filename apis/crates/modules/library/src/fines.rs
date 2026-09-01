//! Library fine assessment and typed submission to the Fees work queue.

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use cp_audit::{AuditActor, RequestContext};
use cp_fees::{charge_requests::FeesChargeRequestOps, foundation::BillingAccountOps};
use cp_finance::ledger::CurrencyOps;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    AssessFineRequest, BorrowerKind, BorrowingListQuery, FineKind, FineResponse,
    LibraryAccessScope, ReasonedVersionRequest, SubmitFineRequest,
    catalogue::LibraryCatalogueOps,
    circulation::{LibraryCirculationOps, borrower_identities, borrower_key},
    members::visible_membership_ids,
    models::FineRow,
    settings::{LibraryActivityEvent, append_domain_audit, append_event, person_actor_id},
};

pub struct LibraryFineOps;

impl LibraryFineOps {
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: LibraryAccessScope,
        query: &BorrowingListQuery,
    ) -> Result<(Vec<FineResponse>, i64)> {
        validate_fine_status(query.status.as_deref())?;
        let visible = visible_membership_ids(pool, tenant_id, scope).await?;
        let membership_filter = constrain_membership(query.membership_id, visible.as_deref());
        let (page, per_page) = bounded_page(query.page, query.per_page);
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, FineRow>(FINE_LIST_SELECT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(membership_filter.as_deref())
            .bind(per_page)
            .bind(offset)
            .fetch_all(pool)
            .await
            .context("Failed to list Library fines")?;
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM library_fines WHERE tenant_id = $1 AND ($2::TEXT IS NULL OR status = $2) AND ($3::UUID[] IS NULL OR membership_id = ANY($3))",
        )
        .bind(tenant_id)
        .bind(query.status.as_deref())
        .bind(membership_filter.as_deref())
        .fetch_one(pool)
        .await
        .context("Failed to count Library fines")?;
        Ok((hydrate_fines(pool, tenant_id, rows).await?, total))
    }

    pub async fn get(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        scope: LibraryAccessScope,
    ) -> Result<Option<FineResponse>> {
        let visible = visible_membership_ids(pool, tenant_id, scope).await?;
        let row = sqlx::query_as::<_, FineRow>(FINE_BY_ID_SELECT)
            .bind(tenant_id)
            .bind(id)
            .bind(visible.as_deref())
            .fetch_optional(pool)
            .await
            .context("Failed to load the Library fine")?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(hydrate_fines(pool, tenant_id, vec![row]).await?.pop())
    }

    pub async fn assess(
        pool: &PgPool,
        tenant_id: Uuid,
        loan_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &AssessFineRequest,
    ) -> Result<FineResponse> {
        let actor_id = person_actor_id(actor)?;
        let loan =
            LibraryCirculationOps::get_loan(pool, tenant_id, loan_id, LibraryAccessScope::Campus)
                .await?
                .ok_or_else(|| anyhow!("The Library loan was not found"))?;
        let (currency_id, amount_minor, assessed_days) = match request.kind {
            FineKind::Overdue => {
                let through = loan.returned_on.unwrap_or_else(|| Utc::now().date_naive());
                let days = (through - loan.due_on).num_days();
                if days <= 0 {
                    bail!("This Library loan has no overdue days to assess");
                }
                let policy = sqlx::query_as::<_, (Option<Uuid>, i64)>(
                    "SELECT fine_currency_id, overdue_fine_minor FROM library_settings WHERE tenant_id = $1",
                )
                .bind(tenant_id)
                .fetch_one(pool)
                .await
                .context("Failed to load the Library fine policy")?;
                let currency_id = policy
                    .0
                    .ok_or_else(|| anyhow!("Configure a Library fine currency first"))?;
                if policy.1 <= 0 {
                    bail!("Configure a positive daily overdue fine first");
                }
                let amount = policy
                    .1
                    .checked_mul(days)
                    .filter(|value| *value <= 9_000_000_000_000_000)
                    .ok_or_else(|| anyhow!("The calculated overdue fine is too large"))?;
                (
                    currency_id,
                    amount,
                    Some(i32::try_from(days).context("Too many overdue days")?),
                )
            }
            FineKind::Replacement => {
                if loan.status != "lost" {
                    bail!("A replacement fine requires a lost Library loan");
                }
                let title = LibraryCatalogueOps::get_title(pool, tenant_id, loan.title_id)
                    .await?
                    .ok_or_else(|| anyhow!("The Library title was not found"))?;
                let amount = title
                    .replacement_cost_minor
                    .ok_or_else(|| anyhow!("Set a replacement cost on the Library title first"))?;
                let currency_id = title.replacement_currency_id.ok_or_else(|| {
                    anyhow!("Set a replacement currency on the Library title first")
                })?;
                (currency_id, amount, None)
            }
        };
        let currency = CurrencyOps::get_by_id(pool, tenant_id, currency_id)
            .await?
            .ok_or_else(|| anyhow!("The Library fine currency was not found"))?;
        if currency.status != "active" {
            bail!("The Library fine currency must be active");
        }
        let id = Uuid::new_v4();
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Library fine assessment")?;
        sqlx::query(
            r#"
            INSERT INTO library_fines (
                id, tenant_id, loan_id, membership_id, kind, currency_id,
                amount_minor, assessed_days, assessed_by
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .bind(loan_id)
        .bind(loan.membership_id)
        .bind(request.kind.as_str())
        .bind(currency_id)
        .bind(amount_minor)
        .bind(assessed_days)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| fine_database_error(error, "Failed to assess the Library fine"))?;
        append_event(
            &mut transaction,
            LibraryActivityEvent {
                tenant_id,
                aggregate_id: id,
                aggregate_type: "fine",
                event_type: "assessed",
                actor_id,
                reason: None,
                metadata: json!({ "loan_id": loan_id, "kind": request.kind.as_str(), "currency_id": currency_id, "amount_minor": amount_minor, "assessed_days": assessed_days }),
            },
        )
        .await?;
        append_domain_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "library.fines.assess",
            "library_fine",
            id,
            json!({ "loan_id": loan_id, "kind": request.kind.as_str(), "currency_id": currency_id, "amount_minor": amount_minor }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit the Library fine")?;
        Self::get(pool, tenant_id, id, LibraryAccessScope::Campus)
            .await?
            .ok_or_else(|| anyhow!("The assessed Library fine could not be loaded"))
    }

    pub async fn submit_to_fees(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &SubmitFineRequest,
    ) -> Result<Option<FineResponse>> {
        let current = Self::get(pool, tenant_id, id, LibraryAccessScope::Campus).await?;
        let Some(current) = current else {
            return Ok(None);
        };
        if current.status != "assessed" {
            bail!("Only an assessed Library fine can be submitted to Fees");
        }
        if current.version != request.expected_version {
            bail!("The Library fine changed since it was loaded");
        }
        if current.borrower_kind != BorrowerKind::Learner {
            bail!(
                "Employee Library fines remain in Library because Fees billing accounts are learner-owned"
            );
        }
        let membership_learner_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT learner_id FROM library_memberships WHERE tenant_id = $1 AND id = $2 AND learner_id IS NOT NULL",
        )
        .bind(tenant_id)
        .bind(current.membership_id)
        .fetch_optional(pool)
        .await
        .context("Failed to resolve the Library fine learner")?
        .ok_or_else(|| anyhow!("The Library membership learner was not found"))?;
        let billing =
            BillingAccountOps::get_by_id(pool, tenant_id, request.billing_account_id, None)
                .await?
                .ok_or_else(|| anyhow!("The learner billing account was not found"))?;
        if billing.learner_id != membership_learner_id {
            bail!("The selected billing account belongs to a different learner");
        }
        let description = format!("Library {} fine · {}", current.kind, current.title);
        let charge = FeesChargeRequestOps::submit(
            pool,
            tenant_id,
            actor,
            request_context,
            request.billing_account_id,
            current.currency_id,
            "library",
            id,
            &description,
            current.amount_minor,
        )
        .await?;
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Library fine submission")?;
        let affected = sqlx::query(
            r#"
            UPDATE library_fines
               SET status = 'submitted_to_fees', fees_charge_request_id = $3,
                   submitted_by = $4, submitted_at = NOW(),
                   version = version + 1, updated_at = NOW()
             WHERE tenant_id = $1 AND id = $2 AND status = 'assessed' AND version = $5
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(charge.id)
        .bind(actor_id)
        .bind(request.expected_version)
        .execute(&mut *transaction)
        .await
        .context("Failed to submit the Library fine to Fees")?
        .rows_affected();
        if affected == 0 {
            transaction.rollback().await.ok();
            bail!("The Library fine changed since it was loaded");
        }
        append_event(
            &mut transaction,
            LibraryActivityEvent {
                tenant_id,
                aggregate_id: id,
                aggregate_type: "fine",
                event_type: "submitted_to_fees",
                actor_id,
                reason: None,
                metadata: json!({ "fees_charge_request_id": charge.id, "fees_charge_status": charge.status, "version": request.expected_version + 1 }),
            },
        )
        .await?;
        append_domain_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "library.fines.submit_to_fees",
            "library_fine",
            id,
            json!({ "fees_charge_request_id": charge.id, "version": request.expected_version + 1 }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit the Library fine submission")?;
        Self::get(pool, tenant_id, id, LibraryAccessScope::Campus).await
    }

    pub async fn waive(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ReasonedVersionRequest,
    ) -> Result<Option<FineResponse>> {
        let actor_id = person_actor_id(actor)?;
        let reason = request.reason.trim();
        if reason.is_empty() {
            bail!("A waiver reason is required");
        }
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Library fine waiver")?;
        let affected = sqlx::query(
            r#"
            UPDATE library_fines
               SET status = 'waived', waived_by = $3, waived_at = NOW(),
                   waiver_reason = $4, version = version + 1, updated_at = NOW()
             WHERE tenant_id = $1 AND id = $2 AND status = 'assessed' AND version = $5
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(actor_id)
        .bind(reason)
        .bind(request.expected_version)
        .execute(&mut *transaction)
        .await
        .context("Failed to waive the Library fine")?
        .rows_affected();
        if affected == 0 {
            transaction.rollback().await.ok();
            let current = Self::get(pool, tenant_id, id, LibraryAccessScope::Campus).await?;
            if current
                .as_ref()
                .is_some_and(|value| value.version != request.expected_version)
            {
                bail!("The Library fine changed since it was loaded");
            }
            return Ok(current);
        }
        append_event(
            &mut transaction,
            LibraryActivityEvent {
                tenant_id,
                aggregate_id: id,
                aggregate_type: "fine",
                event_type: "waived",
                actor_id,
                reason: Some(reason),
                metadata: json!({ "version": request.expected_version + 1 }),
            },
        )
        .await?;
        append_domain_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "library.fines.waive",
            "library_fine",
            id,
            json!({ "version": request.expected_version + 1 }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit the Library fine waiver")?;
        Self::get(pool, tenant_id, id, LibraryAccessScope::Campus).await
    }
}

async fn hydrate_fines(
    pool: &PgPool,
    tenant_id: Uuid,
    rows: Vec<FineRow>,
) -> Result<Vec<FineResponse>> {
    let identities = borrower_identities(
        pool,
        tenant_id,
        rows.iter().map(|row| (row.learner_id, row.employee_id)),
    )
    .await?;
    let currency_ids = rows
        .iter()
        .map(|row| row.currency_id)
        .collect::<HashSet<_>>();
    let mut currencies = HashMap::new();
    for id in currency_ids {
        if let Some(currency) = CurrencyOps::get_by_id(pool, tenant_id, id).await? {
            currencies.insert(id, currency);
        }
    }
    let mut values = Vec::with_capacity(rows.len());
    for row in rows {
        let (kind, borrower_id) = borrower_key(row.learner_id, row.employee_id)?;
        let identity = identities
            .get(&(kind.as_str(), borrower_id))
            .ok_or_else(|| anyhow!("The Library fine borrower is unavailable"))?;
        let currency = currencies
            .get(&row.currency_id)
            .ok_or_else(|| anyhow!("The Library fine currency is unavailable"))?;
        let fees_charge_status = if row.fees_charge_request_id.is_some() {
            FeesChargeRequestOps::get_by_source(pool, tenant_id, "library", row.id)
                .await?
                .map(|value| value.status)
        } else {
            None
        };
        values.push(FineResponse {
            id: row.id,
            loan_id: row.loan_id,
            membership_id: row.membership_id,
            borrower_kind: kind,
            borrower_number: identity.number.clone(),
            borrower_name: identity.display_name.clone(),
            title: row.title,
            kind: row.kind,
            currency_id: row.currency_id,
            currency_code: currency.code.clone(),
            currency_minor_units: currency.minor_units,
            amount_minor: row.amount_minor,
            status: row.status,
            assessed_days: row.assessed_days,
            fees_charge_request_id: row.fees_charge_request_id,
            fees_charge_status,
            version: row.version,
            waiver_reason: row.waiver_reason,
            created_at: row.created_at,
            updated_at: row.updated_at,
        });
    }
    Ok(values)
}

fn constrain_membership(requested: Option<Uuid>, visible: Option<&[Uuid]>) -> Option<Vec<Uuid>> {
    match (requested, visible) {
        (Some(id), Some(values)) => Some(values.contains(&id).then_some(id).into_iter().collect()),
        (Some(id), None) => Some(vec![id]),
        (None, Some(values)) => Some(values.to_vec()),
        (None, None) => None,
    }
}

fn validate_fine_status(value: Option<&str>) -> Result<()> {
    if value.is_some_and(|value| !matches!(value, "assessed" | "submitted_to_fees" | "waived")) {
        bail!("The fine status filter is invalid");
    }
    Ok(())
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).max(1),
        per_page.unwrap_or(25).clamp(1, 100),
    )
}

fn fine_database_error(error: sqlx::Error, context: &str) -> anyhow::Error {
    if let sqlx::Error::Database(database) = &error
        && database.code().as_deref() == Some("23505")
    {
        return anyhow!("That fine type has already been assessed for this Library loan");
    }
    anyhow!("{context}: {error}")
}

const FINE_LIST_SELECT: &str = r#"
SELECT fine.id, fine.loan_id, fine.membership_id, member.learner_id,
       member.employee_id, title.title, fine.kind, fine.currency_id,
       fine.amount_minor, fine.status, fine.assessed_days,
       fine.fees_charge_request_id, fine.version, fine.waiver_reason,
       fine.created_at, fine.updated_at
  FROM library_fines AS fine
  JOIN library_memberships AS member
    ON member.tenant_id = fine.tenant_id AND member.id = fine.membership_id
  JOIN library_loans AS loan
    ON loan.tenant_id = fine.tenant_id AND loan.id = fine.loan_id
  JOIN library_copies AS copy
    ON copy.tenant_id = loan.tenant_id AND copy.id = loan.copy_id
  JOIN library_titles AS title
    ON title.tenant_id = copy.tenant_id AND title.id = copy.title_id
 WHERE fine.tenant_id = $1
   AND ($2::TEXT IS NULL OR fine.status = $2)
   AND ($3::UUID[] IS NULL OR fine.membership_id = ANY($3))
 ORDER BY CASE fine.status WHEN 'assessed' THEN 0 WHEN 'submitted_to_fees' THEN 1 ELSE 2 END,
          fine.created_at DESC, fine.id
 LIMIT $4 OFFSET $5
"#;

const FINE_BY_ID_SELECT: &str = r#"
SELECT fine.id, fine.loan_id, fine.membership_id, member.learner_id,
       member.employee_id, title.title, fine.kind, fine.currency_id,
       fine.amount_minor, fine.status, fine.assessed_days,
       fine.fees_charge_request_id, fine.version, fine.waiver_reason,
       fine.created_at, fine.updated_at
  FROM library_fines AS fine
  JOIN library_memberships AS member
    ON member.tenant_id = fine.tenant_id AND member.id = fine.membership_id
  JOIN library_loans AS loan
    ON loan.tenant_id = fine.tenant_id AND loan.id = fine.loan_id
  JOIN library_copies AS copy
    ON copy.tenant_id = loan.tenant_id AND copy.id = loan.copy_id
  JOIN library_titles AS title
    ON title.tenant_id = copy.tenant_id AND title.id = copy.title_id
 WHERE fine.tenant_id = $1 AND fine.id = $2
   AND ($3::UUID[] IS NULL OR fine.membership_id = ANY($3))
"#;

#[cfg(test)]
mod tests {
    use super::constrain_membership;
    use uuid::Uuid;

    #[test]
    fn fine_filters_never_widen_self_scope() {
        let visible = Uuid::new_v4();
        let hidden = Uuid::new_v4();
        assert!(
            constrain_membership(Some(hidden), Some(&[visible]))
                .unwrap()
                .is_empty()
        );
    }
}
