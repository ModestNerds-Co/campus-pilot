//! Typed posting requests submitted by operational modules.
//!
//! Source modules own their operational records. Finance validates balanced
//! request lines and is the only module that may convert a request into a
//! controlled journal draft.

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, NaiveDate, Utc};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;
use validator::Validate;

use crate::journals::{CreateJournalRequest, JournalLineInput, JournalOps, JournalSourceInput};

const MAX_LINES: usize = 100;
const MAX_MINOR_AMOUNT: i64 = 9_000_000_000_000_000;

#[derive(Debug, Deserialize)]
pub struct PostingRequestListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub source_module: Option<String>,
}

#[derive(Debug, Clone, Serialize, Validate)]
pub struct PostingRequestSource {
    #[validate(length(min = 1, max = 64))]
    pub module_key: String,
    #[validate(length(min = 1, max = 80))]
    pub record_type: String,
    #[validate(length(min = 1, max = 200))]
    pub record_id: String,
    #[validate(length(min = 1, max = 80))]
    pub event_key: String,
}

#[derive(Debug, Clone, Serialize, Validate)]
pub struct NewPostingRequestLine {
    pub account_id: Uuid,
    #[validate(length(max = 500))]
    pub description: Option<String>,
    #[validate(range(min = 0, max = 9_000_000_000_000_000_i64))]
    pub debit_minor: i64,
    #[validate(range(min = 0, max = 9_000_000_000_000_000_i64))]
    pub credit_minor: i64,
}

#[derive(Debug, Clone, Validate)]
pub struct NewPostingRequest {
    #[validate(nested)]
    pub source: PostingRequestSource,
    pub posting_date: NaiveDate,
    pub transaction_currency_id: Uuid,
    #[validate(length(min = 1, max = 1000))]
    pub description: String,
    #[validate(length(max = 160))]
    pub reference: Option<String>,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
    #[validate(length(min = 1, max = 160))]
    pub operation_key: String,
    #[validate(length(min = 2, max = 100), nested)]
    pub lines: Vec<NewPostingRequestLine>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PostingRequestSummaryResponse {
    pub id: Uuid,
    pub source_module_key: String,
    pub source_record_type: String,
    pub source_record_id: String,
    pub source_event_key: String,
    pub posting_date: NaiveDate,
    pub transaction_currency_id: Uuid,
    pub transaction_currency_code: String,
    pub transaction_currency_minor_units: i16,
    pub description: String,
    pub reference: Option<String>,
    pub status: String,
    pub version: i32,
    pub journal_id: Option<Uuid>,
    pub line_count: i64,
    pub debit_minor: i64,
    pub credit_minor: i64,
    pub created_by: Uuid,
    pub resolved_by: Option<Uuid>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolution_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PostingRequestLineResponse {
    pub id: Uuid,
    pub line_number: i16,
    pub account_id: Uuid,
    pub account_code: String,
    pub account_name: String,
    pub description: Option<String>,
    pub debit_minor: i64,
    pub credit_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PostingRequestResponse {
    #[serde(flatten)]
    pub request: PostingRequestSummaryResponse,
    pub lines: Vec<PostingRequestLineResponse>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedPostingRequestsResponse {
    pub posting_requests: Vec<PostingRequestSummaryResponse>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct PostingRequestConversionLine {
    pub line_id: Uuid,
    #[validate(range(min = 0, max = 9_000_000_000_000_000_i64))]
    pub reporting_debit_minor: i64,
    #[validate(range(min = 0, max = 9_000_000_000_000_000_i64))]
    pub reporting_credit_minor: i64,
    #[validate(length(max = 40))]
    pub exchange_rate: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ConvertPostingRequestRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
    #[validate(length(min = 2, max = 100), nested)]
    pub lines: Vec<PostingRequestConversionLine>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct RejectPostingRequestRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 1000))]
    pub reason: String,
}

#[derive(Debug, Clone, FromRow)]
struct AccountReference {
    id: Uuid,
    code: String,
    name: String,
    status: String,
    accepts_postings: bool,
    currency_mode: String,
    currency_id: Option<Uuid>,
}

#[derive(Debug, FromRow)]
struct CurrencyReference {
    id: Uuid,
    code: String,
    status: String,
    is_reporting: bool,
}

pub struct PostingRequestOps;

impl PostingRequestOps {
    #[allow(clippy::too_many_arguments)]
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
        source_module: Option<&str>,
    ) -> Result<(Vec<PostingRequestSummaryResponse>, i64)> {
        validate_status(status)?;
        validate_source_module(source_module)?;
        let page = page.max(1);
        let per_page = per_page.clamp(1, 100);
        let offset = (page - 1) * per_page;
        let search = search.map(|value| format!("%{value}%"));
        let rows = sqlx::query_as::<_, PostingRequestSummaryResponse>(&format!(
            r#"
            {} WHERE request.tenant_id = $1
               AND ($2::TEXT IS NULL OR request.description ILIKE $2
                    OR request.reference ILIKE $2 OR request.source_record_id ILIKE $2)
               AND ($3::TEXT IS NULL OR request.status = $3)
               AND ($4::TEXT IS NULL OR request.source_module_key = $4)
             GROUP BY request.id, currency.id
             ORDER BY request.posting_date DESC, request.created_at DESC
             LIMIT $5 OFFSET $6
            "#,
            summary_select()
        ))
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(source_module)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list Finance posting requests")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM finance_posting_requests
             WHERE tenant_id = $1
               AND ($2::TEXT IS NULL OR description ILIKE $2
                    OR reference ILIKE $2 OR source_record_id ILIKE $2)
               AND ($3::TEXT IS NULL OR status = $3)
               AND ($4::TEXT IS NULL OR source_module_key = $4)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(source_module)
        .fetch_one(pool)
        .await
        .context("Failed to count Finance posting requests")?;
        Ok((rows, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<PostingRequestResponse>> {
        let request = sqlx::query_as::<_, PostingRequestSummaryResponse>(&format!(
            "{} WHERE request.tenant_id = $1 AND request.id = $2 GROUP BY request.id, currency.id",
            summary_select()
        ))
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load Finance posting request")?;
        let Some(request) = request else {
            return Ok(None);
        };
        let lines = sqlx::query_as::<_, PostingRequestLineResponse>(
            r#"
            SELECT line.id, line.line_number, line.account_id,
                   line.account_code_snapshot AS account_code,
                   line.account_name_snapshot AS account_name,
                   line.description, line.debit_minor, line.credit_minor
              FROM finance_posting_request_lines AS line
             WHERE line.tenant_id = $1 AND line.posting_request_id = $2
             ORDER BY line.line_number
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_all(pool)
        .await
        .context("Failed to load Finance posting request lines")?;
        Ok(Some(PostingRequestResponse { request, lines }))
    }

    pub async fn create_from_module(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &NewPostingRequest,
    ) -> Result<PostingRequestResponse> {
        request
            .validate()
            .map_err(|_| anyhow!("The posting request is invalid"))?;
        validate_source(&request.source)?;
        let actor_id = person_actor_id(actor)?;
        let description = required(&request.description, "Description")?;
        let reference = optional(request.reference.as_deref());
        let idempotency_key = required(&request.idempotency_key, "Idempotency key")?;
        let operation_key = required(&request.operation_key, "Operation key")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Finance posting request")?;
        lock_tenant(&mut transaction, tenant_id).await?;
        if let Some(existing_id) = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM finance_posting_requests WHERE tenant_id = $1 AND idempotency_key = $2",
        )
        .bind(tenant_id)
        .bind(&idempotency_key)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to inspect posting-request idempotency")?
        {
            transaction.rollback().await.ok();
            let existing = Self::get_by_id(pool, tenant_id, existing_id)
                .await?
                .ok_or_else(|| anyhow!("The idempotent posting request could not be loaded"))?;
            ensure_idempotent_match(&existing, request)?;
            return Ok(existing);
        }

        let prepared = prepare_lines(
            &mut transaction,
            tenant_id,
            request.transaction_currency_id,
            &request.lines,
        )
        .await?;
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO finance_posting_requests (
                id, tenant_id, source_module_key, source_record_type, source_record_id,
                source_event_key, posting_date, transaction_currency_id, description,
                reference, idempotency_key, created_by
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .bind(request.source.module_key.trim())
        .bind(request.source.record_type.trim())
        .bind(request.source.record_id.trim())
        .bind(request.source.event_key.trim())
        .bind(request.posting_date)
        .bind(request.transaction_currency_id)
        .bind(&description)
        .bind(reference)
        .bind(&idempotency_key)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to create Finance posting request")?;
        for (index, (input, account)) in prepared.iter().enumerate() {
            sqlx::query(
                r#"
                INSERT INTO finance_posting_request_lines (
                    tenant_id, posting_request_id, account_id, line_number, description,
                    account_code_snapshot, account_name_snapshot, debit_minor, credit_minor
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
            )
            .bind(tenant_id)
            .bind(id)
            .bind(input.account_id)
            .bind(i16::try_from(index + 1).context("Posting request line number overflow")?)
            .bind(optional(input.description.as_deref()))
            .bind(&account.code)
            .bind(&account.name)
            .bind(input.debit_minor)
            .bind(input.credit_minor)
            .execute(&mut *transaction)
            .await
            .context("Failed to create Finance posting request line")?;
        }
        append_posting_request_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            &operation_key,
            id,
            json!({
                "status": "pending",
                "source_module": request.source.module_key,
                "source_record_type": request.source.record_type,
                "source_record_id": request.source.record_id,
                "source_event": request.source.event_key,
                "line_count": request.lines.len()
            }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Finance posting request")?;
        Self::get_by_id(pool, tenant_id, id)
            .await?
            .ok_or_else(|| anyhow!("The posting request was not found after creation"))
    }

    pub async fn convert_to_journal(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ConvertPostingRequestRequest,
    ) -> Result<Option<PostingRequestResponse>> {
        request
            .validate()
            .map_err(|_| anyhow!("The posting conversion is invalid"))?;
        let actor_id = person_actor_id(actor)?;
        let Some(current) = Self::get_by_id(pool, tenant_id, id).await? else {
            return Ok(None);
        };
        if current.request.status == "converted" {
            return Ok(Some(current));
        }
        ensure_pending_version(&current.request, request.expected_version)?;
        let conversions = conversion_map(request, &current.lines)?;
        let journal_lines = current
            .lines
            .iter()
            .map(|line| {
                let conversion = conversions.get(&line.id).ok_or_else(|| {
                    anyhow!("Every posting request line requires conversion values")
                })?;
                Ok(JournalLineInput {
                    account_id: line.account_id,
                    transaction_currency_id: current.request.transaction_currency_id,
                    description: line.description.clone(),
                    debit_minor: line.debit_minor,
                    credit_minor: line.credit_minor,
                    reporting_debit_minor: conversion.reporting_debit_minor,
                    reporting_credit_minor: conversion.reporting_credit_minor,
                    exchange_rate: optional(conversion.exchange_rate.as_deref()),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let journal = JournalOps::create(
            pool,
            tenant_id,
            actor,
            request_context,
            &CreateJournalRequest {
                journal_date: current.request.posting_date,
                description: current.request.description.clone(),
                reference: current.request.reference.clone(),
                source: Some(JournalSourceInput {
                    module_key: "finance".to_string(),
                    record_type: "posting_request".to_string(),
                    record_id: id.to_string(),
                }),
                idempotency_key: required(&request.idempotency_key, "Idempotency key")?,
                lines: journal_lines,
            },
        )
        .await?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to resolve Finance posting request")?;
        let changed = sqlx::query(
            r#"
            UPDATE finance_posting_requests
               SET status = 'converted', version = version + 1, journal_id = $4,
                   resolved_by = $5, resolved_at = NOW(), resolution_reason = NULL
             WHERE tenant_id = $1 AND id = $2 AND status = 'pending' AND version = $3
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(request.expected_version)
        .bind(journal.journal.id)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to convert Finance posting request")?;
        if changed.rows_affected() == 0 {
            transaction.rollback().await.ok();
            let latest = Self::get_by_id(pool, tenant_id, id).await?;
            if latest.as_ref().and_then(|value| value.request.journal_id)
                == Some(journal.journal.id)
            {
                return Ok(latest);
            }
            bail!("The posting request changed. Reload it and try again");
        }
        append_posting_request_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "finance.posting_requests.convert",
            id,
            json!({ "status": "converted", "journal_id": journal.journal.id }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Finance posting request conversion")?;
        Self::get_by_id(pool, tenant_id, id).await
    }

    pub async fn reject(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &RejectPostingRequestRequest,
    ) -> Result<Option<PostingRequestResponse>> {
        request
            .validate()
            .map_err(|_| anyhow!("The posting rejection is invalid"))?;
        let actor_id = person_actor_id(actor)?;
        let reason = required(&request.reason, "Reason")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to reject Finance posting request")?;
        let changed = sqlx::query(
            r#"
            UPDATE finance_posting_requests
               SET status = 'rejected', version = version + 1,
                   resolved_by = $4, resolved_at = NOW(), resolution_reason = $5
             WHERE tenant_id = $1 AND id = $2 AND status = 'pending' AND version = $3
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(request.expected_version)
        .bind(actor_id)
        .bind(&reason)
        .execute(&mut *transaction)
        .await
        .context("Failed to reject Finance posting request")?;
        if changed.rows_affected() == 0 {
            let exists = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM finance_posting_requests WHERE tenant_id = $1 AND id = $2)",
            )
            .bind(tenant_id)
            .bind(id)
            .fetch_one(&mut *transaction)
            .await
            .context("Failed to inspect Finance posting request")?;
            transaction.rollback().await.ok();
            if !exists {
                return Ok(None);
            }
            bail!("The posting request changed or is no longer pending");
        }
        append_posting_request_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "finance.posting_requests.reject",
            id,
            json!({ "status": "rejected", "reason": reason }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Finance posting request rejection")?;
        Self::get_by_id(pool, tenant_id, id).await
    }
}

async fn prepare_lines<'a>(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    currency_id: Uuid,
    lines: &'a [NewPostingRequestLine],
) -> Result<Vec<(&'a NewPostingRequestLine, AccountReference)>> {
    if !(2..=MAX_LINES).contains(&lines.len()) {
        bail!("A posting request requires between 2 and {MAX_LINES} lines");
    }
    let currency = sqlx::query_as::<_, CurrencyReference>(
        "SELECT id, code, status, is_reporting FROM finance_currencies WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .bind(currency_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to load posting-request currency")?
    .ok_or_else(|| anyhow!("The posting-request currency was not found"))?;
    if currency.status != "active" {
        bail!("Posting requests require an active currency");
    }
    let _currency_identity = (currency.id, currency.code.as_str());
    let account_ids = lines.iter().map(|line| line.account_id).collect::<Vec<_>>();
    let accounts = sqlx::query_as::<_, AccountReference>(
        r#"
        SELECT id, code, name, status, accepts_postings, currency_mode, currency_id
          FROM finance_accounts
         WHERE tenant_id = $1 AND id = ANY($2) AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(&account_ids)
    .fetch_all(&mut **transaction)
    .await
    .context("Failed to load posting-request accounts")?
    .into_iter()
    .map(|account| (account.id, account))
    .collect::<HashMap<_, _>>();
    let mut debit_total = 0_i64;
    let mut credit_total = 0_i64;
    let mut prepared = Vec::with_capacity(lines.len());
    for (index, line) in lines.iter().enumerate() {
        line.validate()
            .map_err(|_| anyhow!("Posting request line {} is invalid", index + 1))?;
        if !((line.debit_minor > 0 && line.credit_minor == 0)
            || (line.credit_minor > 0 && line.debit_minor == 0))
        {
            bail!(
                "Posting request line {} requires one amount side",
                index + 1
            );
        }
        debit_total = debit_total
            .checked_add(line.debit_minor)
            .ok_or_else(|| anyhow!("Posting request debit total is too large"))?;
        credit_total = credit_total
            .checked_add(line.credit_minor)
            .ok_or_else(|| anyhow!("Posting request credit total is too large"))?;
        let account = accounts
            .get(&line.account_id)
            .ok_or_else(|| anyhow!("Posting request line {} account was not found", index + 1))?;
        if account.status != "active" || !account.accepts_postings {
            bail!(
                "Posting request line {} requires an active posting account",
                index + 1
            );
        }
        match account.currency_mode.as_str() {
            "reporting" if !currency.is_reporting => {
                bail!(
                    "Posting request line {} account accepts only the reporting currency",
                    index + 1
                );
            }
            "single" if account.currency_id != Some(currency_id) => {
                bail!(
                    "Posting request line {} account accepts only its configured currency",
                    index + 1
                );
            }
            "reporting" | "single" | "multi" => {}
            _ => bail!(
                "Posting request line {} account currency mode is invalid",
                index + 1
            ),
        }
        prepared.push((line, account.clone()));
    }
    if debit_total <= 0 || debit_total != credit_total || debit_total > MAX_MINOR_AMOUNT {
        bail!("A posting request must balance in its transaction currency");
    }
    Ok(prepared)
}

fn conversion_map<'a>(
    request: &'a ConvertPostingRequestRequest,
    lines: &[PostingRequestLineResponse],
) -> Result<HashMap<Uuid, &'a PostingRequestConversionLine>> {
    if request.lines.len() != lines.len() {
        bail!("Every posting request line requires conversion values");
    }
    let valid_ids = lines.iter().map(|line| line.id).collect::<HashSet<_>>();
    let mut result = HashMap::with_capacity(request.lines.len());
    for conversion in &request.lines {
        if !valid_ids.contains(&conversion.line_id)
            || result.insert(conversion.line_id, conversion).is_some()
        {
            bail!("Posting conversion line identifiers are invalid");
        }
    }
    Ok(result)
}

fn ensure_idempotent_match(
    existing: &PostingRequestResponse,
    request: &NewPostingRequest,
) -> Result<()> {
    let header_matches = existing.request.source_module_key == request.source.module_key.trim()
        && existing.request.source_record_type == request.source.record_type.trim()
        && existing.request.source_record_id == request.source.record_id.trim()
        && existing.request.source_event_key == request.source.event_key.trim()
        && existing.request.posting_date == request.posting_date
        && existing.request.transaction_currency_id == request.transaction_currency_id
        && existing.request.description == request.description.trim()
        && existing.request.reference.as_deref()
            == optional(request.reference.as_deref()).as_deref()
        && existing.lines.len() == request.lines.len();
    let lines_match = header_matches
        && existing
            .lines
            .iter()
            .zip(&request.lines)
            .all(|(stored, input)| {
                stored.account_id == input.account_id
                    && stored.description.as_deref()
                        == optional(input.description.as_deref()).as_deref()
                    && stored.debit_minor == input.debit_minor
                    && stored.credit_minor == input.credit_minor
            });
    if !lines_match {
        bail!("The idempotency key belongs to a different posting request");
    }
    Ok(())
}

fn ensure_pending_version(
    request: &PostingRequestSummaryResponse,
    expected_version: i32,
) -> Result<()> {
    if request.status != "pending" {
        bail!("Only a pending posting request can be converted");
    }
    if request.version != expected_version {
        bail!("The posting request changed. Reload it and try again");
    }
    Ok(())
}

fn summary_select() -> &'static str {
    r#"
    SELECT request.id, request.source_module_key, request.source_record_type,
           request.source_record_id, request.source_event_key, request.posting_date,
           request.transaction_currency_id,
           currency.code AS transaction_currency_code,
           currency.minor_units AS transaction_currency_minor_units,
           request.description, request.reference, request.status, request.version,
           request.journal_id, COUNT(line.id) AS line_count,
           COALESCE(SUM(line.debit_minor), 0)::BIGINT AS debit_minor,
           COALESCE(SUM(line.credit_minor), 0)::BIGINT AS credit_minor,
           request.created_by, request.resolved_by, request.resolved_at,
           request.resolution_reason, request.created_at, request.updated_at
      FROM finance_posting_requests AS request
      JOIN finance_currencies AS currency
        ON currency.id = request.transaction_currency_id AND currency.tenant_id = request.tenant_id
      LEFT JOIN finance_posting_request_lines AS line
        ON line.posting_request_id = request.id AND line.tenant_id = request.tenant_id
    "#
}

fn validate_status(value: Option<&str>) -> Result<()> {
    if let Some(value) = value
        && !matches!(value, "pending" | "converted" | "rejected" | "cancelled")
    {
        bail!("Posting request status filter is invalid");
    }
    Ok(())
}

fn validate_source_module(value: Option<&str>) -> Result<()> {
    if let Some(value) = value {
        validate_key(value, "Source module")?;
    }
    Ok(())
}

fn validate_source(source: &PostingRequestSource) -> Result<()> {
    validate_key(&source.module_key, "Source module")?;
    validate_key(&source.event_key, "Source event")?;
    required(&source.record_type, "Source record type")?;
    required(&source.record_id, "Source record identifier")?;
    Ok(())
}

fn validate_key(value: &str, label: &str) -> Result<()> {
    let value = required(value, label)?;
    let mut characters = value.chars();
    let valid = characters
        .next()
        .is_some_and(|character| character.is_ascii_lowercase())
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        });
    if !valid {
        bail!("{label} is invalid");
    }
    Ok(())
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

fn person_actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("A signed-in person is required for this Finance operation"))
}

async fn lock_tenant(transaction: &mut Transaction<'_, Postgres>, tenant_id: Uuid) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::TEXT, 0))")
        .bind(format!("finance-posting-request:{tenant_id}"))
        .execute(&mut **transaction)
        .await
        .context("Failed to lock Finance posting request sequencing")?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn append_posting_request_audit(
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
        .with_target(AuditTarget::new("finance_posting_request", id.to_string()))
        .with_redacted_metadata(metadata.as_object().cloned().unwrap_or_default()),
    )
    .await
    .context("Failed to audit Finance posting request")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_keys_are_stable_machine_keys() {
        assert!(validate_key("invoice_issue", "Source event").is_ok());
        assert!(validate_key("Invoice issue", "Source event").is_err());
    }

    #[test]
    fn conversion_requires_every_line_once() {
        let line_id = Uuid::new_v4();
        let lines = vec![PostingRequestLineResponse {
            id: line_id,
            line_number: 1,
            account_id: Uuid::new_v4(),
            account_code: "1000".to_string(),
            account_name: "Receivables".to_string(),
            description: None,
            debit_minor: 100,
            credit_minor: 0,
        }];
        let request = ConvertPostingRequestRequest {
            expected_version: 1,
            idempotency_key: "convert-1".to_string(),
            lines: vec![PostingRequestConversionLine {
                line_id,
                reporting_debit_minor: 100,
                reporting_credit_minor: 0,
                exchange_rate: None,
            }],
        };
        assert!(conversion_map(&request, &lines).is_ok());
    }
}
