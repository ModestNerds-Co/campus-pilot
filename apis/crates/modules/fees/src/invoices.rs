//! Immutable learner invoices and their Finance posting-request boundary.

use std::collections::HashSet;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use cp_academics::ops::ClassGroupOps;
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_finance::posting_requests::{
    NewPostingRequest, NewPostingRequestLine, PostingRequestOps, PostingRequestSource,
};
use cp_sis::ops::EnrolmentOps;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;
use validator::Validate;

use crate::foundation::{BillingAccountOps, FeeStructureOps, FeeStructureResponse};

// Issuing an invoice produces one debit and one credit request line per fee.
// Finance accepts at most 100 request lines.
const MAX_INVOICE_LINES: usize = 50;

#[derive(Debug, Deserialize)]
pub struct InvoiceListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
#[validate(schema(function = "validate_invoice_dates"))]
pub struct CreateInvoiceRequest {
    pub billing_account_id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_term_id: Option<Uuid>,
    pub invoice_date: NaiveDate,
    pub due_date: NaiveDate,
    #[validate(length(max = 1000))]
    pub description: Option<String>,
    #[validate(length(max = 160))]
    pub reference: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub fee_structure_ids: Vec<Uuid>,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
}

fn validate_invoice_dates(
    request: &CreateInvoiceRequest,
) -> Result<(), validator::ValidationError> {
    if request.due_date < request.invoice_date {
        Err(validator::ValidationError::new("invoice_dates"))
    } else {
        Ok(())
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct IssueInvoiceRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct InvoiceSummaryResponse {
    pub id: Uuid,
    pub billing_account_id: Uuid,
    pub billing_account_number: String,
    pub learner_id: Uuid,
    pub learner_number: String,
    pub learner_name: String,
    pub academic_year_id: Uuid,
    pub academic_year_name: String,
    pub academic_term_id: Option<Uuid>,
    pub academic_term_name: Option<String>,
    pub currency_id: Uuid,
    pub currency_code: String,
    pub currency_minor_units: i16,
    pub posting_request_id: Option<Uuid>,
    pub posting_request_status: Option<String>,
    pub invoice_number: String,
    pub invoice_date: NaiveDate,
    pub due_date: NaiveDate,
    pub description: Option<String>,
    pub reference: Option<String>,
    pub total_minor: i64,
    pub status: String,
    pub version: i32,
    pub line_count: i64,
    pub created_by: Uuid,
    pub issued_by: Option<Uuid>,
    pub issued_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct InvoiceLineResponse {
    pub id: Uuid,
    pub line_number: i16,
    pub fee_structure_id: Uuid,
    pub receivable_account_id: Uuid,
    pub revenue_account_id: Uuid,
    pub fee_code: String,
    pub description: String,
    pub amount_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct InvoiceResponse {
    #[serde(flatten)]
    pub invoice: InvoiceSummaryResponse,
    pub lines: Vec<InvoiceLineResponse>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedInvoicesResponse {
    pub invoices: Vec<InvoiceSummaryResponse>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvoiceDeleteOutcome {
    Deleted,
    NotFound,
}

pub struct InvoiceOps;

impl InvoiceOps {
    #[allow(clippy::too_many_arguments)]
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
        visible_learner_ids: Option<&[Uuid]>,
    ) -> Result<(Vec<InvoiceSummaryResponse>, i64)> {
        validate_status(status)?;
        let page = page.max(1);
        let per_page = per_page.clamp(1, 100);
        let offset = (page - 1) * per_page;
        let search = search.map(|value| format!("%{value}%"));
        let learner_ids = visible_learner_ids.map(Vec::from);
        let rows = sqlx::query_as::<_, InvoiceSummaryResponse>(&format!(
            r#"
            {} WHERE invoice.tenant_id = $1 AND invoice.deleted_at IS NULL
               AND ($2::TEXT IS NULL OR invoice.invoice_number ILIKE $2
                    OR invoice.reference ILIKE $2 OR learner.display_name ILIKE $2
                    OR learner.learner_number ILIKE $2 OR billing.account_number ILIKE $2)
               AND ($3::TEXT IS NULL OR invoice.status = $3)
               AND ($4::UUID[] IS NULL OR invoice.billing_account_id IN (
                    SELECT id FROM fees_billing_accounts
                     WHERE tenant_id = $1 AND learner_id = ANY($4) AND deleted_at IS NULL
               ))
             GROUP BY invoice.id, billing.id, learner.id, year.id, term.id, currency.id, request.id
             ORDER BY invoice.invoice_date DESC, invoice.invoice_number DESC
             LIMIT $5 OFFSET $6
            "#,
            summary_select()
        ))
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(&learner_ids)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list Fees invoices")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
              FROM fees_invoices AS invoice
              JOIN fees_billing_accounts AS billing
                ON billing.id = invoice.billing_account_id AND billing.tenant_id = invoice.tenant_id
              JOIN learners AS learner
                ON learner.id = billing.learner_id AND learner.tenant_id = billing.tenant_id
             WHERE invoice.tenant_id = $1 AND invoice.deleted_at IS NULL
               AND billing.deleted_at IS NULL AND learner.deleted_at IS NULL
               AND ($2::TEXT IS NULL OR invoice.invoice_number ILIKE $2
                    OR invoice.reference ILIKE $2 OR learner.display_name ILIKE $2
                    OR learner.learner_number ILIKE $2 OR billing.account_number ILIKE $2)
               AND ($3::TEXT IS NULL OR invoice.status = $3)
               AND ($4::UUID[] IS NULL OR billing.learner_id = ANY($4))
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(&learner_ids)
        .fetch_one(pool)
        .await
        .context("Failed to count Fees invoices")?;
        Ok((rows, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        visible_learner_ids: Option<&[Uuid]>,
    ) -> Result<Option<InvoiceResponse>> {
        let learner_ids = visible_learner_ids.map(Vec::from);
        let invoice = sqlx::query_as::<_, InvoiceSummaryResponse>(&format!(
            r#"
            {} WHERE invoice.tenant_id = $1 AND invoice.id = $2 AND invoice.deleted_at IS NULL
               AND ($3::UUID[] IS NULL OR billing.learner_id = ANY($3))
             GROUP BY invoice.id, billing.id, learner.id, year.id, term.id, currency.id, request.id
            "#,
            summary_select()
        ))
        .bind(tenant_id)
        .bind(id)
        .bind(&learner_ids)
        .fetch_optional(pool)
        .await
        .context("Failed to load Fees invoice")?;
        let Some(invoice) = invoice else {
            return Ok(None);
        };
        let lines = load_lines(pool, tenant_id, id).await?;
        Ok(Some(InvoiceResponse { invoice, lines }))
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateInvoiceRequest,
    ) -> Result<InvoiceResponse> {
        request
            .validate()
            .map_err(|_| anyhow!("The invoice request is invalid"))?;
        let actor_id = actor_id(actor)?;
        let idempotency_key = required(&request.idempotency_key, "Idempotency key")?;
        let description = optional(request.description.as_deref());
        let reference = optional(request.reference.as_deref());
        let unique_ids = request
            .fee_structure_ids
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        if unique_ids.len() != request.fee_structure_ids.len()
            || unique_ids.is_empty()
            || unique_ids.len() > MAX_INVOICE_LINES
        {
            bail!("Choose between 1 and {MAX_INVOICE_LINES} different fee structures");
        }
        let billing =
            BillingAccountOps::get_by_id(pool, tenant_id, request.billing_account_id, None)
                .await?
                .ok_or_else(|| anyhow!("The billing account was not found"))?;
        if billing.status != "active" {
            bail!("Invoices require an active billing account");
        }
        if billing.opened_on > request.invoice_date {
            bail!("The invoice date cannot be before the billing account opened");
        }
        let learner_grade = learner_grade_for_year(
            pool,
            tenant_id,
            billing.learner_id,
            request.academic_year_id,
        )
        .await?;
        let mut structures = Vec::with_capacity(request.fee_structure_ids.len());
        for structure_id in &request.fee_structure_ids {
            let structure = FeeStructureOps::get_by_id(pool, tenant_id, *structure_id)
                .await?
                .ok_or_else(|| anyhow!("A selected fee structure was not found"))?;
            validate_structure_for_invoice(
                &structure,
                request.academic_year_id,
                request.academic_term_id,
                learner_grade,
            )?;
            structures.push(structure);
        }
        let currency_id = structures[0].currency_id;
        if structures
            .iter()
            .any(|structure| structure.currency_id != currency_id)
        {
            bail!("An invoice cannot mix fee-structure currencies");
        }
        let total_minor = structures.iter().try_fold(0_i64, |total, structure| {
            total
                .checked_add(structure.amount_minor)
                .ok_or_else(|| anyhow!("The invoice total is too large"))
        })?;
        let mut transaction = pool.begin().await.context("Failed to start Fees invoice")?;
        lock_tenant(&mut transaction, tenant_id).await?;
        if let Some(existing_id) = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM fees_invoices WHERE tenant_id = $1 AND idempotency_key = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(&idempotency_key)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to inspect invoice idempotency")?
        {
            transaction.rollback().await.ok();
            return Self::get_by_id(pool, tenant_id, existing_id, None)
                .await?
                .ok_or_else(|| anyhow!("The idempotent invoice could not be loaded"));
        }
        let sequence =
            next_invoice_number(&mut transaction, tenant_id, request.academic_year_id).await?;
        let invoice_number = format!("INV-{}-{sequence:06}", request.invoice_date.year());
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO fees_invoices (
                id, tenant_id, billing_account_id, academic_year_id, academic_term_id,
                currency_id, invoice_number, invoice_date, due_date, description,
                reference, total_minor, idempotency_key, created_by
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .bind(request.billing_account_id)
        .bind(request.academic_year_id)
        .bind(request.academic_term_id)
        .bind(currency_id)
        .bind(&invoice_number)
        .bind(request.invoice_date)
        .bind(request.due_date)
        .bind(description)
        .bind(reference)
        .bind(total_minor)
        .bind(&idempotency_key)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to create Fees invoice")?;
        for (index, structure) in structures.iter().enumerate() {
            sqlx::query(
                r#"
                INSERT INTO fees_invoice_lines (
                    tenant_id, invoice_id, fee_structure_id, receivable_account_id,
                    revenue_account_id, line_number, fee_code_snapshot, description, amount_minor
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
            )
            .bind(tenant_id)
            .bind(id)
            .bind(structure.id)
            .bind(structure.receivable_account_id)
            .bind(structure.revenue_account_id)
            .bind(i16::try_from(index + 1).context("Invoice line number overflow")?)
            .bind(&structure.code)
            .bind(&structure.name)
            .bind(structure.amount_minor)
            .execute(&mut *transaction)
            .await
            .context("Failed to create Fees invoice line")?;
        }
        append_invoice_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "fees.invoices.create",
            id,
            json!({
                "status": "draft",
                "invoice_number": invoice_number,
                "billing_account_id": request.billing_account_id,
                "currency_id": currency_id,
                "total_minor": total_minor,
                "line_count": structures.len()
            }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Fees invoice")?;
        Self::get_by_id(pool, tenant_id, id, None)
            .await?
            .ok_or_else(|| anyhow!("The invoice was not found after creation"))
    }

    pub async fn issue(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &IssueInvoiceRequest,
    ) -> Result<Option<InvoiceResponse>> {
        request
            .validate()
            .map_err(|_| anyhow!("The invoice issue request is invalid"))?;
        let actor_id = actor_id(actor)?;
        let Some(current) = Self::get_by_id(pool, tenant_id, id, None).await? else {
            return Ok(None);
        };
        if current.invoice.status == "issued" {
            return Ok(Some(current));
        }
        if current.invoice.status != "draft" || current.invoice.version != request.expected_version
        {
            bail!("The invoice changed or is no longer a draft");
        }
        let mut posting_lines = Vec::with_capacity(current.lines.len() * 2);
        for line in &current.lines {
            posting_lines.push(NewPostingRequestLine {
                account_id: line.receivable_account_id,
                description: Some(line.description.clone()),
                debit_minor: line.amount_minor,
                credit_minor: 0,
            });
            posting_lines.push(NewPostingRequestLine {
                account_id: line.revenue_account_id,
                description: Some(line.description.clone()),
                debit_minor: 0,
                credit_minor: line.amount_minor,
            });
        }
        let posting_request = PostingRequestOps::create_from_module(
            pool,
            tenant_id,
            actor,
            request_context,
            &NewPostingRequest {
                source: PostingRequestSource {
                    module_key: "fees".to_string(),
                    record_type: "invoice".to_string(),
                    record_id: id.to_string(),
                    event_key: "invoice_issue".to_string(),
                },
                posting_date: current.invoice.invoice_date,
                transaction_currency_id: current.invoice.currency_id,
                description: format!("Fees invoice {}", current.invoice.invoice_number),
                reference: current.invoice.reference.clone(),
                idempotency_key: format!("fees:invoice:{id}:issue"),
                operation_key: "fees.invoices.issue".to_string(),
                lines: posting_lines,
            },
        )
        .await?;
        let mut transaction = pool.begin().await.context("Failed to issue Fees invoice")?;
        let changed = sqlx::query(
            r#"
            UPDATE fees_invoices
               SET status = 'issued', version = version + 1, posting_request_id = $4,
                   issued_by = $5, issued_at = NOW()
             WHERE tenant_id = $1 AND id = $2 AND status = 'draft'
               AND version = $3 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(request.expected_version)
        .bind(posting_request.request.id)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to issue Fees invoice")?;
        if changed.rows_affected() == 0 {
            transaction.rollback().await.ok();
            let latest = Self::get_by_id(pool, tenant_id, id, None).await?;
            if latest
                .as_ref()
                .and_then(|invoice| invoice.invoice.posting_request_id)
                == Some(posting_request.request.id)
            {
                return Ok(latest);
            }
            bail!("The invoice changed. Reload it and try again");
        }
        append_invoice_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "fees.invoices.issue",
            id,
            json!({
                "status": "issued",
                "posting_request_id": posting_request.request.id,
                "total_minor": current.invoice.total_minor
            }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Fees invoice issue")?;
        Self::get_by_id(pool, tenant_id, id, None).await
    }

    pub async fn delete(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        expected_version: i32,
        actor: AuditActor,
        request_context: RequestContext,
    ) -> Result<InvoiceDeleteOutcome> {
        let _actor_id = actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to remove Fees invoice")?;
        let current = sqlx::query_as::<_, (String, i32)>(
            "SELECT status, version FROM fees_invoices WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL FOR UPDATE",
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to lock Fees invoice")?;
        let Some((status, version)) = current else {
            transaction.rollback().await.ok();
            return Ok(InvoiceDeleteOutcome::NotFound);
        };
        if status != "draft" {
            bail!("Only a draft invoice can be removed");
        }
        if version != expected_version {
            bail!("The invoice changed. Reload it and try again");
        }
        sqlx::query(
            "UPDATE fees_invoices SET deleted_at = NOW(), version = version + 1 WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to remove Fees invoice")?;
        append_invoice_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "fees.invoices.delete",
            id,
            json!({ "status": "deleted", "previous_version": version }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Fees invoice removal")?;
        Ok(InvoiceDeleteOutcome::Deleted)
    }
}

async fn learner_grade_for_year(
    pool: &PgPool,
    tenant_id: Uuid,
    learner_id: Uuid,
    academic_year_id: Uuid,
) -> Result<Option<Uuid>> {
    let (enrolments, _) = EnrolmentOps::list(
        pool,
        tenant_id,
        1,
        2,
        None,
        Some("active"),
        Some(academic_year_id),
        None,
        Some(learner_id),
    )
    .await?;
    if enrolments.len() > 1 {
        bail!("The learner has more than one active enrolment for the academic year");
    }
    let Some(enrolment) = enrolments.first() else {
        return Ok(None);
    };
    let class_group = ClassGroupOps::get_by_id(pool, tenant_id, enrolment.class_group_id)
        .await?
        .ok_or_else(|| anyhow!("The learner's class could not be loaded"))?;
    Ok(class_group.grade_level_id)
}

fn validate_structure_for_invoice(
    structure: &FeeStructureResponse,
    academic_year_id: Uuid,
    academic_term_id: Option<Uuid>,
    learner_grade_id: Option<Uuid>,
) -> Result<()> {
    if structure.status != "active" {
        bail!("Invoices require active fee structures");
    }
    if structure.academic_year_id != academic_year_id {
        bail!("Every fee structure must belong to the invoice academic year");
    }
    if structure.academic_term_id.is_some() && structure.academic_term_id != academic_term_id {
        bail!("A term fee structure must match the invoice term");
    }
    if let Some(grade_id) = structure.grade_level_id {
        match learner_grade_id {
            Some(learner_grade_id) if learner_grade_id == grade_id => {}
            Some(_) => bail!("A selected fee structure does not match the learner's grade"),
            None => bail!("A grade-specific fee requires an active learner enrolment"),
        }
    }
    Ok(())
}

async fn load_lines(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<Vec<InvoiceLineResponse>> {
    sqlx::query_as::<_, InvoiceLineResponse>(
        r#"
        SELECT id, line_number, fee_structure_id, receivable_account_id,
               revenue_account_id, fee_code_snapshot AS fee_code, description, amount_minor
          FROM fees_invoice_lines
         WHERE tenant_id = $1 AND invoice_id = $2
         ORDER BY line_number
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_all(pool)
    .await
    .context("Failed to load Fees invoice lines")
}

fn summary_select() -> &'static str {
    r#"
    SELECT invoice.id, invoice.billing_account_id,
           billing.account_number AS billing_account_number,
           billing.learner_id, learner.learner_number,
           learner.display_name AS learner_name,
           invoice.academic_year_id, year.name AS academic_year_name,
           invoice.academic_term_id, term.name AS academic_term_name,
           invoice.currency_id, currency.code AS currency_code,
           currency.minor_units AS currency_minor_units,
           invoice.posting_request_id, request.status AS posting_request_status,
           invoice.invoice_number, invoice.invoice_date, invoice.due_date,
           invoice.description, invoice.reference, invoice.total_minor,
           invoice.status, invoice.version, COUNT(line.id) AS line_count,
           invoice.created_by, invoice.issued_by, invoice.issued_at,
           invoice.created_at, invoice.updated_at
      FROM fees_invoices AS invoice
      JOIN fees_billing_accounts AS billing
        ON billing.id = invoice.billing_account_id AND billing.tenant_id = invoice.tenant_id
       AND billing.deleted_at IS NULL
      JOIN learners AS learner
        ON learner.id = billing.learner_id AND learner.tenant_id = billing.tenant_id
       AND learner.deleted_at IS NULL
      JOIN academic_years AS year
        ON year.id = invoice.academic_year_id AND year.tenant_id = invoice.tenant_id
       AND year.deleted_at IS NULL
      LEFT JOIN academic_terms AS term
        ON term.id = invoice.academic_term_id AND term.tenant_id = invoice.tenant_id
       AND term.deleted_at IS NULL
      JOIN finance_currencies AS currency
        ON currency.id = invoice.currency_id AND currency.tenant_id = invoice.tenant_id
       AND currency.deleted_at IS NULL
      LEFT JOIN finance_posting_requests AS request
        ON request.id = invoice.posting_request_id AND request.tenant_id = invoice.tenant_id
      LEFT JOIN fees_invoice_lines AS line
        ON line.invoice_id = invoice.id AND line.tenant_id = invoice.tenant_id
    "#
}

async fn next_invoice_number(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    academic_year_id: Uuid,
) -> Result<i64> {
    sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO fees_invoice_sequences (tenant_id, academic_year_id, last_number)
        VALUES ($1, $2, 1)
        ON CONFLICT (tenant_id, academic_year_id)
        DO UPDATE SET last_number = fees_invoice_sequences.last_number + 1
        RETURNING last_number
        "#,
    )
    .bind(tenant_id)
    .bind(academic_year_id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to allocate Fees invoice number")
}

async fn lock_tenant(transaction: &mut Transaction<'_, Postgres>, tenant_id: Uuid) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("fees-invoice:{tenant_id}"))
        .execute(&mut **transaction)
        .await
        .context("Failed to lock Fees invoice numbering")?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn append_invoice_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    operation: &str,
    id: Uuid,
    metadata: serde_json::Value,
) -> Result<()> {
    append_audit(
        &mut **transaction,
        &NewAuditEvent::new(
            tenant_id,
            actor,
            operation,
            AuditOutcome::Succeeded,
            request_context,
        )
        .with_target(AuditTarget::new("fees_invoice", id.to_string()))
        .with_redacted_metadata(metadata.as_object().cloned().unwrap_or_default()),
    )
    .await
    .context("Failed to audit Fees invoice")?;
    Ok(())
}

fn validate_status(status: Option<&str>) -> Result<()> {
    if status.is_some_and(|status| !matches!(status, "draft" | "issued")) {
        bail!("Invoice status filter is invalid");
    }
    Ok(())
}

fn actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("Authenticated person actor is required"))
}

fn required(value: &str, label: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{label} is required");
    }
    Ok(value.to_string())
}

fn optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invoice_dates_cannot_run_backwards() {
        let request = CreateInvoiceRequest {
            billing_account_id: Uuid::new_v4(),
            academic_year_id: Uuid::new_v4(),
            academic_term_id: None,
            invoice_date: NaiveDate::from_ymd_opt(2026, 8, 28).expect("valid date"),
            due_date: NaiveDate::from_ymd_opt(2026, 8, 27).expect("valid date"),
            description: None,
            reference: None,
            fee_structure_ids: vec![Uuid::new_v4()],
            idempotency_key: "invoice-1".to_string(),
        };
        assert!(request.validate().is_err());
    }

    #[test]
    fn grade_specific_fees_require_matching_enrolment() {
        let grade_id = Uuid::new_v4();
        let structure = FeeStructureResponse {
            id: Uuid::new_v4(),
            academic_year_id: Uuid::new_v4(),
            academic_term_id: None,
            grade_level_id: Some(grade_id),
            currency_id: Uuid::new_v4(),
            receivable_account_id: Uuid::new_v4(),
            revenue_account_id: Uuid::new_v4(),
            code: "TUITION".to_string(),
            name: "Tuition".to_string(),
            description: None,
            amount_minor: 100,
            status: "active".to_string(),
            version: 1,
            created_by: Uuid::new_v4(),
            activated_by: None,
            activated_at: None,
            retired_by: None,
            retired_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(
            validate_structure_for_invoice(
                &structure,
                structure.academic_year_id,
                None,
                Some(grade_id)
            )
            .is_ok()
        );
        assert!(
            validate_structure_for_invoice(&structure, structure.academic_year_id, None, None)
                .is_err()
        );
    }
}
