//! Owns controlled multi-currency journals and their immutable posting lifecycle.
//!
//! Other modules may eventually submit typed idempotent posting requests, but
//! only Finance validates, approves, posts, and reverses ledger journals.

use std::collections::HashMap;
use std::str::FromStr;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use rust_decimal::Decimal;
use rust_decimal::RoundingStrategy;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;
use validator::Validate;

const MAX_JOURNAL_LINES: usize = 100;
const MAX_LINE_MINOR_AMOUNT: i64 = 9_000_000_000_000_000;
const MAX_EXCHANGE_RATE: i64 = 1_000_000_000_000;

#[derive(Debug, Deserialize)]
pub struct JournalListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub starts_on: Option<NaiveDate>,
    pub ends_on: Option<NaiveDate>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct JournalSourceInput {
    #[validate(length(min = 1, max = 64))]
    pub module_key: String,
    #[validate(length(min = 1, max = 80))]
    pub record_type: String,
    #[validate(length(min = 1, max = 200))]
    pub record_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct JournalLineInput {
    pub account_id: Uuid,
    pub transaction_currency_id: Uuid,
    #[validate(length(max = 500))]
    pub description: Option<String>,
    #[validate(range(min = 0, max = 9_000_000_000_000_000_i64))]
    pub debit_minor: i64,
    #[validate(range(min = 0, max = 9_000_000_000_000_000_i64))]
    pub credit_minor: i64,
    #[validate(range(min = 0, max = 9_000_000_000_000_000_i64))]
    pub reporting_debit_minor: i64,
    #[validate(range(min = 0, max = 9_000_000_000_000_000_i64))]
    pub reporting_credit_minor: i64,
    #[validate(length(max = 40))]
    pub exchange_rate: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateJournalRequest {
    pub journal_date: NaiveDate,
    #[validate(length(min = 1, max = 1000))]
    pub description: String,
    #[validate(length(max = 160))]
    pub reference: Option<String>,
    #[validate(nested)]
    pub source: Option<JournalSourceInput>,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
    #[validate(length(min = 2, max = 100), nested)]
    pub lines: Vec<JournalLineInput>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateJournalRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    pub journal_date: NaiveDate,
    #[validate(length(min = 1, max = 1000))]
    pub description: String,
    #[validate(length(max = 160))]
    pub reference: Option<String>,
    #[validate(nested)]
    pub source: Option<JournalSourceInput>,
    #[validate(length(min = 2, max = 100), nested)]
    pub lines: Vec<JournalLineInput>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct JournalVersionRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct RejectJournalRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 1000))]
    pub reason: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ReverseJournalRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    pub journal_date: NaiveDate,
    #[validate(length(min = 1, max = 1000))]
    pub reason: String,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct DeleteJournalQuery {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct JournalSummaryResponse {
    pub id: Uuid,
    pub fiscal_year_id: Uuid,
    pub fiscal_year_name: String,
    pub accounting_period_id: Uuid,
    pub accounting_period_name: String,
    pub reporting_currency_id: Uuid,
    pub reporting_currency_code: String,
    pub reporting_currency_minor_units: i16,
    pub reversal_of_journal_id: Option<Uuid>,
    pub reversal_journal_id: Option<Uuid>,
    pub journal_number: String,
    pub journal_date: NaiveDate,
    pub description: String,
    pub reference: Option<String>,
    pub source_module_key: Option<String>,
    pub source_record_type: Option<String>,
    pub source_record_id: Option<String>,
    pub status: String,
    pub version: i32,
    pub line_count: i64,
    pub reporting_debit_minor: i64,
    pub reporting_credit_minor: i64,
    pub created_by: Uuid,
    pub submitted_by: Option<Uuid>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub rejected_by: Option<Uuid>,
    pub rejected_at: Option<DateTime<Utc>>,
    pub rejection_reason: Option<String>,
    pub posted_by: Option<Uuid>,
    pub posted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct JournalLineResponse {
    pub id: Uuid,
    pub line_number: i16,
    pub account_id: Uuid,
    pub account_code: String,
    pub account_name: String,
    pub transaction_currency_id: Uuid,
    pub transaction_currency_code: String,
    pub transaction_currency_minor_units: i16,
    pub description: Option<String>,
    pub debit_minor: i64,
    pub credit_minor: i64,
    pub reporting_debit_minor: i64,
    pub reporting_credit_minor: i64,
    pub exchange_rate: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JournalResponse {
    #[serde(flatten)]
    pub journal: JournalSummaryResponse,
    pub lines: Vec<JournalLineResponse>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedJournalsResponse {
    pub journals: Vec<JournalSummaryResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JournalValidationResponse {
    pub valid: bool,
    pub issues: Vec<String>,
    pub line_count: i64,
    pub reporting_debit_minor: i64,
    pub reporting_credit_minor: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalDeleteOutcome {
    Deleted,
    NotFound,
}

#[derive(Debug, FromRow)]
struct JournalContext {
    fiscal_year_id: Uuid,
    fiscal_year_starts_on: NaiveDate,
    accounting_period_id: Uuid,
    reporting_currency_id: Uuid,
    reporting_currency_minor_units: i16,
}

#[derive(Debug, FromRow)]
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
    minor_units: i16,
    status: String,
}

struct PreparedLine {
    account_id: Uuid,
    currency_id: Uuid,
    description: Option<String>,
    account_code: String,
    account_name: String,
    currency_code: String,
    currency_minor_units: i16,
    debit_minor: i64,
    credit_minor: i64,
    reporting_debit_minor: i64,
    reporting_credit_minor: i64,
    exchange_rate: Option<String>,
}

#[derive(Debug, FromRow)]
struct LockedJournal {
    fiscal_year_id: Uuid,
    journal_number: String,
    status: String,
    version: i32,
    created_by: Uuid,
    submitted_by: Option<Uuid>,
    reversal_of_journal_id: Option<Uuid>,
}

#[derive(Debug, FromRow)]
struct PersistedValidationRow {
    journal_status: String,
    line_count: i64,
    reporting_debit_minor: i64,
    reporting_credit_minor: i64,
    invalid_line_count: i64,
    conversion_mismatch_count: i64,
    fiscal_year_status: String,
    period_status: String,
    period_starts_on: NaiveDate,
    period_ends_on: NaiveDate,
    reporting_currency_is_current: bool,
}

struct NewJournalRecord<'a> {
    id: Uuid,
    context: &'a JournalContext,
    reversal_of: Option<Uuid>,
    number: &'a str,
    date: NaiveDate,
    description: &'a str,
    reference: Option<&'a str>,
    source: Option<&'a JournalSource>,
    idempotency_key: &'a str,
    created_by: Uuid,
}

pub struct JournalOps;

impl JournalOps {
    #[allow(clippy::too_many_arguments)]
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
        starts_on: Option<NaiveDate>,
        ends_on: Option<NaiveDate>,
    ) -> Result<(Vec<JournalSummaryResponse>, i64)> {
        validate_status_filter(status)?;
        if let (Some(start), Some(end)) = (starts_on, ends_on)
            && end < start
        {
            bail!("Journal date range end cannot be before its start");
        }
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, JournalSummaryResponse>(&format!(
            r#"
            {} WHERE journal.tenant_id = $1 AND journal.deleted_at IS NULL
               AND ($2::TEXT IS NULL OR journal.journal_number ILIKE $2
                    OR journal.description ILIKE $2 OR journal.reference ILIKE $2)
               AND ($3::TEXT IS NULL OR CASE
                    WHEN journal.status = 'posted' AND posted_reversal.id IS NOT NULL THEN 'reversed'
                    ELSE journal.status END = $3)
               AND ($4::DATE IS NULL OR journal.journal_date >= $4)
               AND ($5::DATE IS NULL OR journal.journal_date <= $5)
             GROUP BY journal.id, year.id, period.id, currency.id, posted_reversal.id
             ORDER BY journal.journal_date DESC, journal.journal_number DESC
             LIMIT $6 OFFSET $7
            "#,
            journal_select()
        ))
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(starts_on)
        .bind(ends_on)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list finance journals")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
              FROM finance_journals AS journal
              LEFT JOIN finance_journals AS posted_reversal
                ON posted_reversal.tenant_id = journal.tenant_id
               AND posted_reversal.reversal_of_journal_id = journal.id
               AND posted_reversal.status = 'posted'
               AND posted_reversal.deleted_at IS NULL
             WHERE journal.tenant_id = $1 AND journal.deleted_at IS NULL
               AND ($2::TEXT IS NULL OR journal.journal_number ILIKE $2
                    OR journal.description ILIKE $2 OR journal.reference ILIKE $2)
               AND ($3::TEXT IS NULL OR CASE
                    WHEN journal.status = 'posted' AND posted_reversal.id IS NOT NULL THEN 'reversed'
                    ELSE journal.status END = $3)
               AND ($4::DATE IS NULL OR journal.journal_date >= $4)
               AND ($5::DATE IS NULL OR journal.journal_date <= $5)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(starts_on)
        .bind(ends_on)
        .fetch_one(pool)
        .await
        .context("Failed to count finance journals")?;
        Ok((rows, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<JournalResponse>> {
        let Some(journal) = load_summary(pool, tenant_id, id).await? else {
            return Ok(None);
        };
        let lines = load_lines(pool, tenant_id, id).await?;
        Ok(Some(JournalResponse { journal, lines }))
    }

    pub async fn validation(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<JournalValidationResponse>> {
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start journal validation")?;
        let exists = lock_journal(&mut transaction, tenant_id, id).await?;
        let Some(_) = exists else {
            transaction.rollback().await.ok();
            return Ok(None);
        };
        let validation = validate_persisted(&mut transaction, tenant_id, id).await?;
        transaction.rollback().await.ok();
        Ok(Some(validation))
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateJournalRequest,
    ) -> Result<JournalResponse> {
        let actor_id = person_actor_id(actor)?;
        let idempotency_key = required(&request.idempotency_key, "Idempotency key")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start journal transaction")?;
        lock_tenant(&mut transaction, tenant_id).await?;
        if let Some(existing_id) =
            journal_id_for_idempotency(&mut transaction, tenant_id, &idempotency_key).await?
        {
            transaction.rollback().await.ok();
            return Self::get_by_id(pool, tenant_id, existing_id)
                .await?
                .ok_or_else(|| anyhow!("The idempotent journal could not be loaded"));
        }
        let context = journal_context(&mut transaction, tenant_id, request.journal_date).await?;
        let lines = prepare_lines(&mut transaction, tenant_id, &context, &request.lines).await?;
        let description = required(&request.description, "Journal description")?;
        let reference = optional(&request.reference);
        let source = normalized_source(request.source.as_ref())?;
        let sequence =
            next_journal_number(&mut transaction, tenant_id, context.fiscal_year_id).await?;
        let journal_number = format!("JRN-{}-{sequence:06}", context.fiscal_year_starts_on.year());
        let journal_id = Uuid::new_v4();
        insert_journal(
            &mut transaction,
            tenant_id,
            NewJournalRecord {
                id: journal_id,
                context: &context,
                reversal_of: None,
                number: &journal_number,
                date: request.journal_date,
                description: &description,
                reference: reference.as_deref(),
                source: source.as_ref(),
                idempotency_key: &idempotency_key,
                created_by: actor_id,
            },
        )
        .await?;
        insert_lines(&mut transaction, tenant_id, journal_id, &lines).await?;
        append_journal_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "finance.journals.create",
            journal_id,
            json!({ "status": "draft", "line_count": lines.len() }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit finance journal")?;
        Self::get_by_id(pool, tenant_id, journal_id)
            .await?
            .ok_or_else(|| anyhow!("The journal was not found after creation"))
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateJournalRequest,
    ) -> Result<Option<JournalResponse>> {
        let _actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start journal transaction")?;
        lock_tenant(&mut transaction, tenant_id).await?;
        let Some(current) = lock_journal(&mut transaction, tenant_id, id).await? else {
            transaction.rollback().await.ok();
            return Ok(None);
        };
        ensure_version(&current, request.expected_version)?;
        if current.status != "draft" && current.status != "rejected" {
            bail!("Only a draft or rejected journal can be edited");
        }
        let context = journal_context(&mut transaction, tenant_id, request.journal_date).await?;
        if context.fiscal_year_id != current.fiscal_year_id {
            bail!("A journal date cannot move to another fiscal year");
        }
        let lines = prepare_lines(&mut transaction, tenant_id, &context, &request.lines).await?;
        let description = required(&request.description, "Journal description")?;
        let reference = optional(&request.reference);
        let source = normalized_source(request.source.as_ref())?;
        let (source_module, source_type, source_id) = source_columns(source.as_ref());
        sqlx::query(
            r#"
            UPDATE finance_journals
               SET accounting_period_id = $3, reporting_currency_id = $4,
                   journal_date = $5, description = $6, reference = $7,
                   source_module_key = $8, source_record_type = $9, source_record_id = $10,
                   status = 'draft', version = version + 1,
                   submitted_by = NULL, submitted_at = NULL,
                   approved_by = NULL, approved_at = NULL,
                   rejected_by = NULL, rejected_at = NULL, rejection_reason = NULL,
                   posted_by = NULL, posted_at = NULL
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(context.accounting_period_id)
        .bind(context.reporting_currency_id)
        .bind(request.journal_date)
        .bind(description)
        .bind(reference)
        .bind(source_module)
        .bind(source_type)
        .bind(source_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to update finance journal")?;
        sqlx::query(
            "UPDATE finance_journal_lines SET deleted_at = NOW() WHERE tenant_id = $1 AND journal_id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to replace journal lines")?;
        insert_lines(&mut transaction, tenant_id, id, &lines).await?;
        append_journal_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "finance.journals.update",
            id,
            json!({ "status": "draft", "line_count": lines.len(), "previous_version": current.version }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit finance journal")?;
        Self::get_by_id(pool, tenant_id, id).await
    }

    pub async fn delete(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        expected_version: i32,
        actor: AuditActor,
        request_context: RequestContext,
    ) -> Result<JournalDeleteOutcome> {
        let _actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start journal transaction")?;
        let Some(current) = lock_journal(&mut transaction, tenant_id, id).await? else {
            transaction.rollback().await.ok();
            return Ok(JournalDeleteOutcome::NotFound);
        };
        ensure_version(&current, expected_version)?;
        if current.status != "draft" && current.status != "rejected" {
            bail!("Only a draft or rejected journal can be removed");
        }
        sqlx::query(
            "UPDATE finance_journal_lines SET deleted_at = NOW() WHERE tenant_id = $1 AND journal_id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to remove journal lines")?;
        sqlx::query(
            "UPDATE finance_journals SET deleted_at = NOW(), version = version + 1 WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to remove finance journal")?;
        append_journal_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "finance.journals.delete",
            id,
            json!({ "status": current.status, "previous_version": current.version }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit finance journal removal")?;
        Ok(JournalDeleteOutcome::Deleted)
    }

    pub async fn submit(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &JournalVersionRequest,
    ) -> Result<Option<JournalResponse>> {
        transition(
            pool,
            tenant_id,
            id,
            actor,
            request_context,
            request.expected_version,
            JournalTransition::Submit,
        )
        .await
    }

    pub async fn approve(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &JournalVersionRequest,
    ) -> Result<Option<JournalResponse>> {
        transition(
            pool,
            tenant_id,
            id,
            actor,
            request_context,
            request.expected_version,
            JournalTransition::Approve,
        )
        .await
    }

    pub async fn reject(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &RejectJournalRequest,
    ) -> Result<Option<JournalResponse>> {
        let reason = required(&request.reason, "Rejection reason")?;
        transition(
            pool,
            tenant_id,
            id,
            actor,
            request_context,
            request.expected_version,
            JournalTransition::Reject(reason),
        )
        .await
    }

    pub async fn post(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &JournalVersionRequest,
    ) -> Result<Option<JournalResponse>> {
        transition(
            pool,
            tenant_id,
            id,
            actor,
            request_context,
            request.expected_version,
            JournalTransition::Post,
        )
        .await
    }

    pub async fn reverse(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ReverseJournalRequest,
    ) -> Result<Option<JournalResponse>> {
        let actor_id = person_actor_id(actor)?;
        let reason = required(&request.reason, "Reversal reason")?;
        let idempotency_key = required(&request.idempotency_key, "Idempotency key")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start journal reversal")?;
        lock_tenant(&mut transaction, tenant_id).await?;
        let Some(original) = lock_journal(&mut transaction, tenant_id, id).await? else {
            transaction.rollback().await.ok();
            return Ok(None);
        };
        if let Some(existing_id) =
            journal_id_for_idempotency(&mut transaction, tenant_id, &idempotency_key).await?
        {
            transaction.rollback().await.ok();
            return Self::get_by_id(pool, tenant_id, existing_id).await;
        }
        ensure_version(&original, request.expected_version)?;
        if original.status != "posted" {
            bail!("Only a posted journal can be reversed");
        }
        if original.reversal_of_journal_id.is_some() {
            bail!("A reversal journal cannot itself be reversed");
        }
        let active_reversal = sqlx::query_scalar::<_, Option<Uuid>>(
            r#"
            SELECT id FROM finance_journals
             WHERE tenant_id = $1 AND reversal_of_journal_id = $2
               AND deleted_at IS NULL AND status <> 'rejected'
             LIMIT 1
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to inspect journal reversal")?
        .flatten();
        if active_reversal.is_some() {
            bail!("This journal already has an active reversal");
        }
        let context = journal_context(&mut transaction, tenant_id, request.journal_date).await?;
        let original_lines = load_lines_in_transaction(&mut transaction, tenant_id, id).await?;
        let reversed_inputs = original_lines
            .iter()
            .map(|line| JournalLineInput {
                account_id: line.account_id,
                transaction_currency_id: line.transaction_currency_id,
                description: line.description.clone(),
                debit_minor: line.credit_minor,
                credit_minor: line.debit_minor,
                reporting_debit_minor: line.reporting_credit_minor,
                reporting_credit_minor: line.reporting_debit_minor,
                exchange_rate: line.exchange_rate.clone(),
            })
            .collect::<Vec<_>>();
        let lines = prepare_lines(&mut transaction, tenant_id, &context, &reversed_inputs).await?;
        let sequence =
            next_journal_number(&mut transaction, tenant_id, context.fiscal_year_id).await?;
        let journal_number = format!("JRN-{}-{sequence:06}", context.fiscal_year_starts_on.year());
        let reversal_id = Uuid::new_v4();
        let description = format!("Reversal of {}: {reason}", original.journal_number);
        let source = JournalSource {
            module_key: "finance".to_string(),
            record_type: "journal_reversal".to_string(),
            record_id: id.to_string(),
        };
        insert_journal(
            &mut transaction,
            tenant_id,
            NewJournalRecord {
                id: reversal_id,
                context: &context,
                reversal_of: Some(id),
                number: &journal_number,
                date: request.journal_date,
                description: &description,
                reference: Some(original.journal_number.as_str()),
                source: Some(&source),
                idempotency_key: &idempotency_key,
                created_by: actor_id,
            },
        )
        .await?;
        insert_lines(&mut transaction, tenant_id, reversal_id, &lines).await?;
        append_journal_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "finance.journals.reverse",
            reversal_id,
            json!({ "status": "draft", "reversal_of_journal_id": id, "line_count": lines.len() }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit journal reversal")?;
        Self::get_by_id(pool, tenant_id, reversal_id).await
    }
}

enum JournalTransition {
    Submit,
    Approve,
    Reject(String),
    Post,
}

impl JournalTransition {
    const fn action_key(&self) -> &'static str {
        match self {
            Self::Submit => "finance.journals.submit",
            Self::Approve => "finance.journals.approve",
            Self::Reject(_) => "finance.journals.reject",
            Self::Post => "finance.journals.post",
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn transition(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    expected_version: i32,
    transition: JournalTransition,
) -> Result<Option<JournalResponse>> {
    let actor_id = person_actor_id(actor)?;
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to start journal lifecycle transaction")?;
    let Some(current) = lock_journal(&mut transaction, tenant_id, id).await? else {
        transaction.rollback().await.ok();
        return Ok(None);
    };
    let already_complete = matches!(
        (&transition, current.status.as_str()),
        (
            JournalTransition::Submit,
            "submitted" | "approved" | "posted"
        ) | (JournalTransition::Approve, "approved" | "posted")
            | (JournalTransition::Reject(_), "rejected")
            | (JournalTransition::Post, "posted")
    );
    if already_complete {
        transaction.rollback().await.ok();
        return JournalOps::get_by_id(pool, tenant_id, id).await;
    }
    ensure_version(&current, expected_version)?;
    let validation_required = !matches!(transition, JournalTransition::Reject(_));
    if validation_required {
        let validation = validate_persisted(&mut transaction, tenant_id, id).await?;
        if !validation.valid {
            bail!("Journal validation failed: {}", validation.issues.join(" "));
        }
    }
    let target_status = match &transition {
        JournalTransition::Submit => {
            if current.status != "draft" {
                bail!("Only a draft journal can be submitted");
            }
            sqlx::query(
                r#"
                UPDATE finance_journals
                   SET status = 'submitted', submitted_by = $3, submitted_at = NOW(),
                       version = version + 1
                 WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
                "#,
            )
            .bind(tenant_id)
            .bind(id)
            .bind(actor_id)
            .execute(&mut *transaction)
            .await
            .context("Failed to submit finance journal")?;
            "submitted"
        }
        JournalTransition::Approve => {
            if current.status != "submitted" {
                bail!("Only a submitted journal can be approved");
            }
            if current.created_by == actor_id || current.submitted_by == Some(actor_id) {
                bail!("A journal must be approved by another person");
            }
            sqlx::query(
                r#"
                UPDATE finance_journals
                   SET status = 'approved', approved_by = $3, approved_at = NOW(),
                       version = version + 1
                 WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
                "#,
            )
            .bind(tenant_id)
            .bind(id)
            .bind(actor_id)
            .execute(&mut *transaction)
            .await
            .context("Failed to approve finance journal")?;
            "approved"
        }
        JournalTransition::Reject(reason) => {
            if current.status != "submitted" {
                bail!("Only a submitted journal can be rejected");
            }
            if current.created_by == actor_id || current.submitted_by == Some(actor_id) {
                bail!("A journal must be reviewed by another person");
            }
            sqlx::query(
                r#"
                UPDATE finance_journals
                   SET status = 'rejected', rejected_by = $3, rejected_at = NOW(),
                       rejection_reason = $4, version = version + 1
                 WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
                "#,
            )
            .bind(tenant_id)
            .bind(id)
            .bind(actor_id)
            .bind(reason)
            .execute(&mut *transaction)
            .await
            .context("Failed to reject finance journal")?;
            "rejected"
        }
        JournalTransition::Post => {
            if current.status != "approved" {
                bail!("Only an approved journal can be posted");
            }
            if current.created_by == actor_id || current.submitted_by == Some(actor_id) {
                bail!("A journal must be posted by someone other than its preparer");
            }
            sqlx::query(
                r#"
                UPDATE finance_journals
                   SET status = 'posted', posted_by = $3, posted_at = NOW(),
                       version = version + 1
                 WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
                "#,
            )
            .bind(tenant_id)
            .bind(id)
            .bind(actor_id)
            .execute(&mut *transaction)
            .await
            .context("Failed to post finance journal")?;
            "posted"
        }
    };
    append_journal_audit(
        &mut transaction,
        tenant_id,
        actor,
        request_context,
        transition.action_key(),
        id,
        json!({ "status": target_status, "previous_version": current.version }),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit journal lifecycle change")?;
    JournalOps::get_by_id(pool, tenant_id, id).await
}

async fn prepare_lines(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    context: &JournalContext,
    inputs: &[JournalLineInput],
) -> Result<Vec<PreparedLine>> {
    if !(2..=MAX_JOURNAL_LINES).contains(&inputs.len()) {
        bail!("A journal requires between 2 and {MAX_JOURNAL_LINES} lines");
    }
    let account_ids = inputs
        .iter()
        .map(|line| line.account_id)
        .collect::<Vec<_>>();
    let currency_ids = inputs
        .iter()
        .map(|line| line.transaction_currency_id)
        .collect::<Vec<_>>();
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
    .context("Failed to load journal accounts")?
    .into_iter()
    .map(|value| (value.id, value))
    .collect::<HashMap<_, _>>();
    let currencies = sqlx::query_as::<_, CurrencyReference>(
        r#"
        SELECT id, code, minor_units, status
          FROM finance_currencies
         WHERE tenant_id = $1 AND id = ANY($2) AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(&currency_ids)
    .fetch_all(&mut **transaction)
    .await
    .context("Failed to load journal currencies")?
    .into_iter()
    .map(|value| (value.id, value))
    .collect::<HashMap<_, _>>();

    let mut prepared = Vec::with_capacity(inputs.len());
    let mut reporting_debit = 0_i64;
    let mut reporting_credit = 0_i64;
    for (index, input) in inputs.iter().enumerate() {
        input
            .validate()
            .map_err(|_| anyhow!("Journal line {} is invalid", index + 1))?;
        validate_sides(input, index)?;
        let account = accounts
            .get(&input.account_id)
            .ok_or_else(|| anyhow!("Journal line {} account was not found", index + 1))?;
        if account.status != "active" || !account.accepts_postings {
            bail!(
                "Journal line {} requires an active posting account",
                index + 1
            );
        }
        let currency = currencies
            .get(&input.transaction_currency_id)
            .ok_or_else(|| anyhow!("Journal line {} currency was not found", index + 1))?;
        if currency.status != "active" {
            bail!("Journal line {} requires an active currency", index + 1);
        }
        match account.currency_mode.as_str() {
            "reporting" if currency.id != context.reporting_currency_id => {
                bail!(
                    "Journal line {} account accepts only the reporting currency",
                    index + 1
                );
            }
            "single" if account.currency_id != Some(currency.id) => {
                bail!(
                    "Journal line {} account accepts only its configured currency",
                    index + 1
                );
            }
            "reporting" | "single" | "multi" => {}
            _ => bail!(
                "Journal line {} account currency mode is invalid",
                index + 1
            ),
        }
        let transaction_amount = input.debit_minor.max(input.credit_minor);
        let reporting_amount = input
            .reporting_debit_minor
            .max(input.reporting_credit_minor);
        if transaction_amount > MAX_LINE_MINOR_AMOUNT || reporting_amount > MAX_LINE_MINOR_AMOUNT {
            bail!("Journal line {} amount is too large", index + 1);
        }
        let exchange_rate = normalize_exchange_rate(
            input.exchange_rate.as_deref(),
            currency.id != context.reporting_currency_id,
        )?;
        if currency.id == context.reporting_currency_id {
            if transaction_amount != reporting_amount {
                bail!(
                    "Journal line {} reporting amount must equal its transaction amount",
                    index + 1
                );
            }
        } else {
            verify_conversion(
                index,
                transaction_amount,
                currency.minor_units,
                reporting_amount,
                context.reporting_currency_minor_units,
                exchange_rate.as_deref().unwrap_or_default(),
            )?;
        }
        reporting_debit = reporting_debit
            .checked_add(input.reporting_debit_minor)
            .ok_or_else(|| anyhow!("Journal debit total is too large"))?;
        reporting_credit = reporting_credit
            .checked_add(input.reporting_credit_minor)
            .ok_or_else(|| anyhow!("Journal credit total is too large"))?;
        prepared.push(PreparedLine {
            account_id: account.id,
            currency_id: currency.id,
            description: optional(&input.description),
            account_code: account.code.clone(),
            account_name: account.name.clone(),
            currency_code: currency.code.clone(),
            currency_minor_units: currency.minor_units,
            debit_minor: input.debit_minor,
            credit_minor: input.credit_minor,
            reporting_debit_minor: input.reporting_debit_minor,
            reporting_credit_minor: input.reporting_credit_minor,
            exchange_rate,
        });
    }
    if reporting_debit != reporting_credit {
        bail!("Journal reporting debits and credits must balance");
    }
    if reporting_debit == 0 {
        bail!("Journal total must be greater than zero");
    }
    Ok(prepared)
}

fn validate_sides(input: &JournalLineInput, index: usize) -> Result<()> {
    let debit = input.debit_minor > 0
        && input.credit_minor == 0
        && input.reporting_debit_minor > 0
        && input.reporting_credit_minor == 0;
    let credit = input.credit_minor > 0
        && input.debit_minor == 0
        && input.reporting_credit_minor > 0
        && input.reporting_debit_minor == 0;
    if !debit && !credit {
        bail!(
            "Journal line {} must contain one debit or one credit",
            index + 1
        );
    }
    Ok(())
}

fn normalize_exchange_rate(
    value: Option<&str>,
    required_for_foreign: bool,
) -> Result<Option<String>> {
    let normalized = value.map(str::trim).filter(|value| !value.is_empty());
    if !required_for_foreign {
        if normalized.is_some_and(|value| value != "1" && value != "1.0") {
            bail!("Reporting-currency journal lines cannot set an exchange rate");
        }
        return Ok(None);
    }
    let value = normalized
        .ok_or_else(|| anyhow!("Foreign-currency journal lines require an exchange rate"))?;
    let rate = Decimal::from_str(value).map_err(|_| anyhow!("Exchange rate is invalid"))?;
    if rate <= Decimal::ZERO || rate > Decimal::from(MAX_EXCHANGE_RATE) || rate.scale() > 18 {
        bail!("Exchange rate must be greater than zero with at most 18 decimal places");
    }
    Ok(Some(rate.normalize().to_string()))
}

fn verify_conversion(
    index: usize,
    transaction_minor: i64,
    transaction_minor_units: i16,
    reporting_minor: i64,
    reporting_minor_units: i16,
    exchange_rate: &str,
) -> Result<()> {
    let rate = Decimal::from_str(exchange_rate).map_err(|_| anyhow!("Exchange rate is invalid"))?;
    let transaction_major = Decimal::from_i128_with_scale(
        i128::from(transaction_minor),
        u32::try_from(transaction_minor_units).unwrap_or(0),
    );
    let reporting_factor =
        Decimal::from(10_i64.pow(u32::try_from(reporting_minor_units).unwrap_or(0)));
    let expected = transaction_major
        .checked_mul(rate)
        .and_then(|value| value.checked_mul(reporting_factor))
        .ok_or_else(|| {
            anyhow!(
                "Journal line {} currency conversion is too large",
                index + 1
            )
        })?
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_i64()
        .ok_or_else(|| {
            anyhow!(
                "Journal line {} currency conversion is too large",
                index + 1
            )
        })?;
    if expected.abs_diff(reporting_minor) > 1 {
        bail!(
            "Journal line {} reporting amount does not match its exchange rate",
            index + 1
        );
    }
    Ok(())
}

async fn validate_persisted(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<JournalValidationResponse> {
    let row = sqlx::query_as::<_, PersistedValidationRow>(
        r#"
        SELECT journal.status AS journal_status,
               COUNT(line.id) AS line_count,
               COALESCE(SUM(line.reporting_debit_minor), 0)::BIGINT AS reporting_debit_minor,
               COALESCE(SUM(line.reporting_credit_minor), 0)::BIGINT AS reporting_credit_minor,
               COUNT(line.id) FILTER (WHERE
                   account.id IS NULL OR account.status <> 'active' OR NOT account.accepts_postings
                   OR currency.id IS NULL OR currency.status <> 'active'
                   OR line.account_code_snapshot <> account.code
                   OR line.account_name_snapshot <> account.name
                   OR line.transaction_currency_code <> currency.code
                   OR line.transaction_currency_minor_units <> currency.minor_units
                   OR (account.currency_mode = 'reporting'
                       AND line.transaction_currency_id <> journal.reporting_currency_id)
                   OR (account.currency_mode = 'single'
                       AND line.transaction_currency_id <> account.currency_id)
                   OR (line.transaction_currency_id = journal.reporting_currency_id AND (
                       line.debit_minor <> line.reporting_debit_minor
                       OR line.credit_minor <> line.reporting_credit_minor
                       OR line.exchange_rate IS NOT NULL
                   ))
                   OR (line.transaction_currency_id <> journal.reporting_currency_id
                       AND line.exchange_rate IS NULL)
               ) AS invalid_line_count,
               COUNT(line.id) FILTER (WHERE
                   line.transaction_currency_id <> journal.reporting_currency_id
                   AND line.exchange_rate IS NOT NULL
                   AND ABS(
                       ROUND(
                           (GREATEST(line.debit_minor, line.credit_minor)::NUMERIC
                               / POWER(10::NUMERIC, line.transaction_currency_minor_units))
                           * line.exchange_rate
                           * POWER(10::NUMERIC, reporting.minor_units)
                       ) - GREATEST(line.reporting_debit_minor, line.reporting_credit_minor)
                   ) > 1
               ) AS conversion_mismatch_count,
               year.status AS fiscal_year_status,
               period.status AS period_status,
               period.starts_on AS period_starts_on,
               period.ends_on AS period_ends_on,
               reporting.is_reporting AND reporting.status = 'active'
                   AND reporting.deleted_at IS NULL AS reporting_currency_is_current
          FROM finance_journals AS journal
          JOIN finance_fiscal_years AS year
            ON year.id = journal.fiscal_year_id AND year.tenant_id = journal.tenant_id
          JOIN finance_accounting_periods AS period
            ON period.id = journal.accounting_period_id AND period.tenant_id = journal.tenant_id
          JOIN finance_currencies AS reporting
            ON reporting.id = journal.reporting_currency_id AND reporting.tenant_id = journal.tenant_id
          LEFT JOIN finance_journal_lines AS line
            ON line.journal_id = journal.id AND line.tenant_id = journal.tenant_id
           AND line.deleted_at IS NULL
          LEFT JOIN finance_accounts AS account
            ON account.id = line.account_id AND account.tenant_id = line.tenant_id
           AND account.deleted_at IS NULL
          LEFT JOIN finance_currencies AS currency
            ON currency.id = line.transaction_currency_id AND currency.tenant_id = line.tenant_id
           AND currency.deleted_at IS NULL
         WHERE journal.tenant_id = $1 AND journal.id = $2 AND journal.deleted_at IS NULL
         GROUP BY journal.id, year.id, period.id, reporting.id
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to validate finance journal")?;
    let journal_date = sqlx::query_scalar::<_, NaiveDate>(
        "SELECT journal_date FROM finance_journals WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to read journal date")?;
    let mut issues = Vec::new();
    if !(2..=i64::try_from(MAX_JOURNAL_LINES).unwrap_or(100)).contains(&row.line_count) {
        issues.push(format!(
            "A journal requires between 2 and {MAX_JOURNAL_LINES} lines."
        ));
    }
    if row.reporting_debit_minor <= 0 || row.reporting_debit_minor != row.reporting_credit_minor {
        issues.push("Reporting debits and credits must balance above zero.".to_string());
    }
    // A posted journal is historical evidence. Later account, currency, and
    // period changes must not make that immutable entry appear invalid.
    if row.journal_status != "posted" {
        if row.invalid_line_count > 0 {
            issues.push(
                "One or more lines no longer match an active account or currency.".to_string(),
            );
        }
        if row.conversion_mismatch_count > 0 {
            issues.push(
                "One or more foreign-currency amounts no longer match their exchange rate."
                    .to_string(),
            );
        }
        if row.fiscal_year_status != "open" || row.period_status != "open" {
            issues.push("The fiscal year and accounting period must both be open.".to_string());
        }
        if journal_date < row.period_starts_on || journal_date > row.period_ends_on {
            issues.push("The journal date must fall inside its accounting period.".to_string());
        }
        if !row.reporting_currency_is_current {
            issues.push("The journal reporting currency is no longer active.".to_string());
        }
    }
    Ok(JournalValidationResponse {
        valid: issues.is_empty(),
        issues,
        line_count: row.line_count,
        reporting_debit_minor: row.reporting_debit_minor,
        reporting_credit_minor: row.reporting_credit_minor,
    })
}

async fn journal_context(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    journal_date: NaiveDate,
) -> Result<JournalContext> {
    sqlx::query_as::<_, JournalContext>(
        r#"
        SELECT year.id AS fiscal_year_id, year.starts_on AS fiscal_year_starts_on,
               period.id AS accounting_period_id,
               currency.id AS reporting_currency_id,
               currency.minor_units AS reporting_currency_minor_units
          FROM finance_fiscal_years AS year
          JOIN finance_accounting_periods AS period
            ON period.fiscal_year_id = year.id AND period.tenant_id = year.tenant_id
           AND period.deleted_at IS NULL AND period.status = 'open'
           AND $2::DATE BETWEEN period.starts_on AND period.ends_on
          JOIN finance_currencies AS currency
            ON currency.tenant_id = year.tenant_id AND currency.is_reporting
           AND currency.status = 'active' AND currency.deleted_at IS NULL
         WHERE year.tenant_id = $1 AND year.deleted_at IS NULL AND year.status = 'open'
           AND $2::DATE BETWEEN year.starts_on AND year.ends_on
         FOR SHARE OF year, period, currency
        "#,
    )
    .bind(tenant_id)
    .bind(journal_date)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to resolve journal accounting period")?
    .ok_or_else(|| anyhow!("Journal date requires an open fiscal year and accounting period"))
}

async fn insert_journal(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    record: NewJournalRecord<'_>,
) -> Result<()> {
    let (source_module, source_type, source_id) = source_columns(record.source);
    sqlx::query(
        r#"
        INSERT INTO finance_journals (
            id, tenant_id, fiscal_year_id, accounting_period_id, reporting_currency_id,
            reversal_of_journal_id, journal_number, journal_date, description, reference,
            source_module_key, source_record_type, source_record_id, idempotency_key, created_by
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        "#,
    )
    .bind(record.id)
    .bind(tenant_id)
    .bind(record.context.fiscal_year_id)
    .bind(record.context.accounting_period_id)
    .bind(record.context.reporting_currency_id)
    .bind(record.reversal_of)
    .bind(record.number)
    .bind(record.date)
    .bind(record.description)
    .bind(record.reference)
    .bind(source_module)
    .bind(source_type)
    .bind(source_id)
    .bind(record.idempotency_key)
    .bind(record.created_by)
    .execute(&mut **transaction)
    .await
    .context("Failed to create finance journal")?;
    Ok(())
}

async fn insert_lines(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    journal_id: Uuid,
    lines: &[PreparedLine],
) -> Result<()> {
    for (index, line) in lines.iter().enumerate() {
        let line_number = i16::try_from(index + 1).context("Journal has too many lines")?;
        sqlx::query(
            r#"
            INSERT INTO finance_journal_lines (
                tenant_id, journal_id, account_id, transaction_currency_id, line_number,
                description, account_code_snapshot, account_name_snapshot,
                transaction_currency_code, transaction_currency_minor_units,
                debit_minor, credit_minor, reporting_debit_minor, reporting_credit_minor,
                exchange_rate
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15::NUMERIC
            )
            "#,
        )
        .bind(tenant_id)
        .bind(journal_id)
        .bind(line.account_id)
        .bind(line.currency_id)
        .bind(line_number)
        .bind(&line.description)
        .bind(&line.account_code)
        .bind(&line.account_name)
        .bind(&line.currency_code)
        .bind(line.currency_minor_units)
        .bind(line.debit_minor)
        .bind(line.credit_minor)
        .bind(line.reporting_debit_minor)
        .bind(line.reporting_credit_minor)
        .bind(&line.exchange_rate)
        .execute(&mut **transaction)
        .await
        .context("Failed to create finance journal line")?;
    }
    Ok(())
}

async fn next_journal_number(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    fiscal_year_id: Uuid,
) -> Result<i64> {
    sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO finance_journal_sequences (tenant_id, fiscal_year_id, last_number)
        VALUES ($1, $2, 1)
        ON CONFLICT (tenant_id, fiscal_year_id)
        DO UPDATE SET last_number = finance_journal_sequences.last_number + 1
        RETURNING last_number
        "#,
    )
    .bind(tenant_id)
    .bind(fiscal_year_id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to allocate journal number")
}

async fn journal_id_for_idempotency(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> Result<Option<Uuid>> {
    sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM finance_journals WHERE tenant_id = $1 AND idempotency_key = $2 AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .bind(idempotency_key)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to inspect journal idempotency")
}

async fn lock_journal(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<LockedJournal>> {
    sqlx::query_as::<_, LockedJournal>(
        r#"
        SELECT fiscal_year_id, journal_number, status, version, created_by,
               submitted_by, reversal_of_journal_id
          FROM finance_journals
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock finance journal")
}

fn ensure_version(journal: &LockedJournal, expected_version: i32) -> Result<()> {
    if journal.version != expected_version {
        bail!("Journal changed since it was loaded; reload it before continuing");
    }
    Ok(())
}

async fn load_summary<'e, E>(
    executor: E,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<JournalSummaryResponse>>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, JournalSummaryResponse>(&format!(
        r#"
        {} WHERE journal.tenant_id = $1 AND journal.id = $2 AND journal.deleted_at IS NULL
         GROUP BY journal.id, year.id, period.id, currency.id, posted_reversal.id
        "#,
        journal_select()
    ))
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(executor)
    .await
    .context("Failed to read finance journal")
}

async fn load_lines(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<Vec<JournalLineResponse>> {
    sqlx::query_as::<_, JournalLineResponse>(line_select())
        .bind(tenant_id)
        .bind(id)
        .fetch_all(pool)
        .await
        .context("Failed to read finance journal lines")
}

async fn load_lines_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Vec<JournalLineResponse>> {
    sqlx::query_as::<_, JournalLineResponse>(line_select())
        .bind(tenant_id)
        .bind(id)
        .fetch_all(&mut **transaction)
        .await
        .context("Failed to read finance journal lines")
}

fn journal_select() -> &'static str {
    r#"
    SELECT journal.id, journal.fiscal_year_id, year.name AS fiscal_year_name,
           journal.accounting_period_id, period.name AS accounting_period_name,
           journal.reporting_currency_id, currency.code AS reporting_currency_code,
           currency.minor_units AS reporting_currency_minor_units,
           journal.reversal_of_journal_id, posted_reversal.id AS reversal_journal_id,
           journal.journal_number, journal.journal_date, journal.description, journal.reference,
           journal.source_module_key, journal.source_record_type, journal.source_record_id,
           CASE WHEN journal.status = 'posted' AND posted_reversal.id IS NOT NULL
                THEN 'reversed' ELSE journal.status END AS status,
           journal.version, COUNT(line.id) AS line_count,
           COALESCE(SUM(line.reporting_debit_minor), 0)::BIGINT AS reporting_debit_minor,
           COALESCE(SUM(line.reporting_credit_minor), 0)::BIGINT AS reporting_credit_minor,
           journal.created_by, journal.submitted_by, journal.submitted_at,
           journal.approved_by, journal.approved_at, journal.rejected_by,
           journal.rejected_at, journal.rejection_reason, journal.posted_by,
           journal.posted_at, journal.created_at, journal.updated_at
      FROM finance_journals AS journal
      JOIN finance_fiscal_years AS year
        ON year.id = journal.fiscal_year_id AND year.tenant_id = journal.tenant_id
      JOIN finance_accounting_periods AS period
        ON period.id = journal.accounting_period_id AND period.tenant_id = journal.tenant_id
      JOIN finance_currencies AS currency
        ON currency.id = journal.reporting_currency_id AND currency.tenant_id = journal.tenant_id
      LEFT JOIN finance_journal_lines AS line
        ON line.journal_id = journal.id AND line.tenant_id = journal.tenant_id
       AND line.deleted_at IS NULL
      LEFT JOIN finance_journals AS posted_reversal
        ON posted_reversal.tenant_id = journal.tenant_id
       AND posted_reversal.reversal_of_journal_id = journal.id
       AND posted_reversal.status = 'posted' AND posted_reversal.deleted_at IS NULL
    "#
}

fn line_select() -> &'static str {
    r#"
    SELECT id, line_number, account_id, account_code_snapshot AS account_code,
           account_name_snapshot AS account_name, transaction_currency_id,
           transaction_currency_code, transaction_currency_minor_units,
           description, debit_minor, credit_minor, reporting_debit_minor,
           reporting_credit_minor, exchange_rate::TEXT AS exchange_rate
      FROM finance_journal_lines
     WHERE tenant_id = $1 AND journal_id = $2 AND deleted_at IS NULL
     ORDER BY line_number
    "#
}

#[derive(Debug)]
struct JournalSource {
    module_key: String,
    record_type: String,
    record_id: String,
}

fn normalized_source(value: Option<&JournalSourceInput>) -> Result<Option<JournalSource>> {
    let Some(value) = value else {
        return Ok(None);
    };
    value
        .validate()
        .map_err(|_| anyhow!("Journal source is invalid"))?;
    let module_key = required(&value.module_key, "Source module")?.to_ascii_lowercase();
    if !module_key.chars().enumerate().all(|(index, character)| {
        if index == 0 {
            character.is_ascii_lowercase()
        } else {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        }
    }) {
        bail!("Source module key is invalid");
    }
    Ok(Some(JournalSource {
        module_key,
        record_type: required(&value.record_type, "Source record type")?,
        record_id: required(&value.record_id, "Source record identifier")?,
    }))
}

fn source_columns(source: Option<&JournalSource>) -> (Option<&str>, Option<&str>, Option<&str>) {
    source.map_or((None, None, None), |source| {
        (
            Some(source.module_key.as_str()),
            Some(source.record_type.as_str()),
            Some(source.record_id.as_str()),
        )
    })
}

async fn append_journal_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    action: &str,
    journal_id: Uuid,
    metadata: serde_json::Value,
) -> Result<()> {
    let metadata = metadata.as_object().cloned().unwrap_or_default();
    append_audit(
        &mut **transaction,
        &NewAuditEvent::new(
            tenant_id,
            actor,
            action,
            AuditOutcome::Succeeded,
            request_context,
        )
        .with_target(AuditTarget::new("finance_journal", journal_id.to_string()))
        .with_redacted_metadata(metadata),
    )
    .await
    .context("Failed to audit finance journal operation")?;
    Ok(())
}

async fn lock_tenant(transaction: &mut Transaction<'_, Postgres>, tenant_id: Uuid) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::TEXT, 0))")
        .bind(tenant_id)
        .execute(&mut **transaction)
        .await
        .context("Failed to lock finance journal numbering")?;
    Ok(())
}

fn person_actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("Authenticated person actor is required"))
}

fn validate_status_filter(status: Option<&str>) -> Result<()> {
    if status.is_some_and(|status| {
        !matches!(
            status,
            "draft" | "submitted" | "approved" | "rejected" | "posted" | "reversed"
        )
    }) {
        bail!("Journal status filter is invalid");
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

fn optional(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::{
        JournalLineInput, normalize_exchange_rate, validate_sides, validate_status_filter,
        verify_conversion,
    };
    use uuid::Uuid;

    fn debit() -> JournalLineInput {
        JournalLineInput {
            account_id: Uuid::new_v4(),
            transaction_currency_id: Uuid::new_v4(),
            description: None,
            debit_minor: 1_000,
            credit_minor: 0,
            reporting_debit_minor: 1_000,
            reporting_credit_minor: 0,
            exchange_rate: None,
        }
    }

    #[test]
    fn journal_line_requires_one_matching_side() {
        assert!(validate_sides(&debit(), 0).is_ok());
        let mut invalid = debit();
        invalid.credit_minor = 1;
        assert!(validate_sides(&invalid, 0).is_err());
        let mut invalid_reporting = debit();
        invalid_reporting.reporting_debit_minor = 0;
        assert!(validate_sides(&invalid_reporting, 1).is_err());
    }

    #[test]
    fn exchange_rates_are_canonical_and_bounded() {
        assert_eq!(
            normalize_exchange_rate(Some(" 1.2500 "), true).unwrap(),
            Some("1.25".to_string())
        );
        assert!(normalize_exchange_rate(None, true).is_err());
        assert!(normalize_exchange_rate(Some("0"), true).is_err());
        assert_eq!(normalize_exchange_rate(Some("1"), false).unwrap(), None);
        assert!(normalize_exchange_rate(Some("1.1"), false).is_err());
    }

    #[test]
    fn foreign_conversion_allows_only_one_minor_unit_rounding_difference() {
        assert!(verify_conversion(0, 1_000, 2, 1_250, 2, "1.25").is_ok());
        assert!(verify_conversion(0, 1_000, 2, 1_251, 2, "1.25").is_ok());
        assert!(verify_conversion(0, 1_000, 2, 1_252, 2, "1.25").is_err());
    }

    #[test]
    fn status_filter_accepts_effective_reversal_state() {
        for status in [
            "draft",
            "submitted",
            "approved",
            "rejected",
            "posted",
            "reversed",
        ] {
            assert!(validate_status_filter(Some(status)).is_ok());
        }
        assert!(validate_status_filter(Some("pending")).is_err());
    }
}
