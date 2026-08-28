//! Learner billing-account and fee-structure foundation.
//!
//! This module owns Fees records only. Learner, academic, currency, and account
//! references are resolved through typed owning-module operations.

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, NaiveDate, Utc};
use cp_academics::ops::{AcademicGradeLevelOps, AcademicTermOps, AcademicYearOps};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_finance::ledger::{AccountOps, CurrencyOps};
use cp_sis::{models::LearnerBillingReference, ops::LearnerOps};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;
use validator::Validate;

const MAX_AMOUNT_MINOR: i64 = 9_000_000_000_000_000;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillingAccountStatus {
    Active,
    OnHold,
    Closed,
}

impl BillingAccountStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::OnHold => "on_hold",
            Self::Closed => "closed",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeeStructureStatus {
    Draft,
    Active,
    Retired,
}

impl FeeStructureStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Retired => "retired",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DirectoryQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LearnerCandidateQuery {
    pub search: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateBillingAccountRequest {
    pub learner_id: Uuid,
    pub opened_on: NaiveDate,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBillingAccountRequest {
    pub status: BillingAccountStatus,
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateFeeStructureRequest {
    pub academic_year_id: Uuid,
    pub academic_term_id: Option<Uuid>,
    pub grade_level_id: Option<Uuid>,
    pub currency_id: Uuid,
    pub receivable_account_id: Uuid,
    pub revenue_account_id: Uuid,
    #[validate(length(min = 1, max = 40))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    #[validate(length(max = 1000))]
    pub description: Option<String>,
    #[validate(range(min = 1_i64, max = 9_000_000_000_000_000_i64))]
    pub amount_minor: i64,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateFeeStructureRequest {
    pub academic_year_id: Uuid,
    pub academic_term_id: Option<Uuid>,
    pub grade_level_id: Option<Uuid>,
    pub currency_id: Uuid,
    pub receivable_account_id: Uuid,
    pub revenue_account_id: Uuid,
    #[validate(length(min = 1, max = 40))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    #[validate(length(max = 1000))]
    pub description: Option<String>,
    #[validate(range(min = 1_i64, max = 9_000_000_000_000_000_i64))]
    pub amount_minor: i64,
    pub expected_version: i32,
}

#[derive(Debug, Deserialize)]
pub struct VersionRequest {
    pub expected_version: i32,
}

#[derive(Debug, Serialize)]
pub struct LearnerCandidatesResponse {
    pub learners: Vec<LearnerBillingReference>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct BillingAccountRecord {
    pub id: Uuid,
    pub learner_id: Uuid,
    pub account_number: String,
    pub opened_on: NaiveDate,
    pub status: String,
    pub version: i32,
    pub created_by: Uuid,
    pub closed_by: Option<Uuid>,
    pub closed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BillingAccountResponse {
    pub id: Uuid,
    pub learner_id: Uuid,
    pub learner_number: String,
    pub learner_name: String,
    pub learner_status: String,
    pub account_number: String,
    pub opened_on: NaiveDate,
    pub status: String,
    pub version: i32,
    pub created_by: Uuid,
    pub closed_by: Option<Uuid>,
    pub closed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedBillingAccountsResponse {
    pub billing_accounts: Vec<BillingAccountResponse>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct FeeStructureResponse {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_term_id: Option<Uuid>,
    pub grade_level_id: Option<Uuid>,
    pub currency_id: Uuid,
    pub receivable_account_id: Uuid,
    pub revenue_account_id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub amount_minor: i64,
    pub status: String,
    pub version: i32,
    pub created_by: Uuid,
    pub activated_by: Option<Uuid>,
    pub activated_at: Option<DateTime<Utc>>,
    pub retired_by: Option<Uuid>,
    pub retired_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedFeeStructuresResponse {
    pub fee_structures: Vec<FeeStructureResponse>,
}

#[derive(Debug, Serialize)]
pub struct FeeCurrencyReference {
    pub id: Uuid,
    pub code: String,
    pub minor_units: i16,
    pub is_reporting: bool,
}

#[derive(Debug, Serialize)]
pub struct FeeAccountReference {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub currency_mode: String,
    pub currency_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct FeeAcademicYearReference {
    pub id: Uuid,
    pub name: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct FeeAcademicTermReference {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub code: String,
    pub name: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct FeeGradeLevelReference {
    pub id: Uuid,
    pub code: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct FeesReferenceDataResponse {
    pub currencies: Vec<FeeCurrencyReference>,
    pub receivable_accounts: Vec<FeeAccountReference>,
    pub revenue_accounts: Vec<FeeAccountReference>,
    pub academic_years: Vec<FeeAcademicYearReference>,
    pub academic_terms: Vec<FeeAcademicTermReference>,
    pub grade_levels: Vec<FeeGradeLevelReference>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteOutcome {
    Deleted,
    NotFound,
}

pub struct FeesReferenceOps;

impl FeesReferenceOps {
    pub async fn load(pool: &PgPool, tenant_id: Uuid) -> Result<FeesReferenceDataResponse> {
        let (currencies, _) = CurrencyOps::list(pool, tenant_id, 1, 100, None, Some("active"))
            .await
            .context("Failed to load fee currencies")?;
        let (receivables, _) = AccountOps::list(
            pool,
            tenant_id,
            1,
            100,
            None,
            Some("active"),
            Some("asset"),
            None,
        )
        .await
        .context("Failed to load receivable accounts")?;
        let (revenue, _) = AccountOps::list(
            pool,
            tenant_id,
            1,
            100,
            None,
            Some("active"),
            Some("income"),
            None,
        )
        .await
        .context("Failed to load revenue accounts")?;
        let (years, _) = AcademicYearOps::list(pool, tenant_id, 1, 100, None, None)
            .await
            .context("Failed to load academic years")?;
        let (terms, _) = AcademicTermOps::list(pool, tenant_id, 1, 100, None, None, None)
            .await
            .context("Failed to load academic terms")?;
        let (grades, _) =
            AcademicGradeLevelOps::list(pool, tenant_id, 1, 100, None, Some("active"))
                .await
                .context("Failed to load grade levels")?;
        Ok(FeesReferenceDataResponse {
            currencies: currencies
                .into_iter()
                .map(|value| FeeCurrencyReference {
                    id: value.id,
                    code: value.code,
                    minor_units: value.minor_units,
                    is_reporting: value.is_reporting,
                })
                .collect(),
            receivable_accounts: receivables
                .into_iter()
                .filter(|value| value.accepts_postings)
                .map(account_reference)
                .collect(),
            revenue_accounts: revenue
                .into_iter()
                .filter(|value| value.accepts_postings)
                .map(account_reference)
                .collect(),
            academic_years: years
                .into_iter()
                .filter(|value| value.status != "closed")
                .map(|value| FeeAcademicYearReference {
                    id: value.id,
                    name: value.name,
                    status: value.status,
                })
                .collect(),
            academic_terms: terms
                .into_iter()
                .filter(|value| value.status != "closed")
                .map(|value| FeeAcademicTermReference {
                    id: value.id,
                    academic_year_id: value.academic_year_id,
                    code: value.code,
                    name: value.name,
                    status: value.status,
                })
                .collect(),
            grade_levels: grades
                .into_iter()
                .map(|value| FeeGradeLevelReference {
                    id: value.id,
                    code: value.code,
                    name: value.name,
                })
                .collect(),
        })
    }
}

pub struct BillingAccountOps;

impl BillingAccountOps {
    #[allow(clippy::too_many_arguments)]
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
        visible_learner_ids: Option<&[Uuid]>,
    ) -> Result<(Vec<BillingAccountResponse>, i64)> {
        validate_billing_status_filter(status)?;
        if visible_learner_ids.is_some_and(|values| values.is_empty()) {
            return Ok((Vec::new(), 0));
        }
        let page = page.max(1);
        let per_page = per_page.clamp(1, 100);
        let offset = (page - 1) * per_page;
        let search = search.map(|value| format!("%{value}%"));
        let learner_ids = visible_learner_ids.map(ToOwned::to_owned);
        let records = sqlx::query_as::<_, BillingAccountRecord>(
            r#"
            SELECT id, learner_id, account_number, opened_on, status, version,
                   created_by, closed_by, closed_at, created_at, updated_at
              FROM fees_billing_accounts
             WHERE tenant_id = $1 AND deleted_at IS NULL
               AND ($2::TEXT IS NULL OR account_number ILIKE $2)
               AND ($3::TEXT IS NULL OR status = $3)
               AND ($4::UUID[] IS NULL OR learner_id = ANY($4))
             ORDER BY account_number
             LIMIT $5 OFFSET $6
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(&learner_ids)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list billing accounts")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM fees_billing_accounts
             WHERE tenant_id = $1 AND deleted_at IS NULL
               AND ($2::TEXT IS NULL OR account_number ILIKE $2)
               AND ($3::TEXT IS NULL OR status = $3)
               AND ($4::UUID[] IS NULL OR learner_id = ANY($4))
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(&learner_ids)
        .fetch_one(pool)
        .await
        .context("Failed to count billing accounts")?;
        let mut accounts = Vec::with_capacity(records.len());
        for record in records {
            accounts.push(hydrate_billing_account(pool, tenant_id, record).await?);
        }
        Ok((accounts, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        visible_learner_ids: Option<&[Uuid]>,
    ) -> Result<Option<BillingAccountResponse>> {
        let record = load_billing_account(pool, tenant_id, id).await?;
        let Some(record) = record else {
            return Ok(None);
        };
        if visible_learner_ids.is_some_and(|values| !values.contains(&record.learner_id)) {
            return Ok(None);
        }
        Ok(Some(
            hydrate_billing_account(pool, tenant_id, record).await?,
        ))
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateBillingAccountRequest,
    ) -> Result<BillingAccountResponse> {
        request
            .validate()
            .map_err(|_| anyhow!("Billing account request is invalid"))?;
        let actor_id = actor_id(actor)?;
        let idempotency_key = required(&request.idempotency_key, "Idempotency key")?;
        let learner = LearnerOps::get_by_id(pool, tenant_id, request.learner_id)
            .await?
            .ok_or_else(|| anyhow!("The learner is not available for this campus"))?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start billing account transaction")?;
        lock_tenant(&mut transaction, tenant_id, "fees-account").await?;
        if let Some(existing_id) = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM fees_billing_accounts WHERE tenant_id = $1 AND idempotency_key = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(&idempotency_key)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to inspect billing account idempotency")?
        {
            transaction.rollback().await.ok();
            return Self::get_by_id(pool, tenant_id, existing_id, None)
                .await?
                .ok_or_else(|| anyhow!("The idempotent billing account could not be loaded"));
        }
        if sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM fees_billing_accounts WHERE tenant_id = $1 AND learner_id = $2 AND deleted_at IS NULL)",
        )
        .bind(tenant_id)
        .bind(request.learner_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to inspect learner billing account")?
        {
            bail!("This learner already has a billing account");
        }
        let sequence = sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO fees_billing_account_sequences (tenant_id, last_number)
            VALUES ($1, 1)
            ON CONFLICT (tenant_id) DO UPDATE SET last_number = fees_billing_account_sequences.last_number + 1
            RETURNING last_number
            "#,
        )
        .bind(tenant_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to allocate billing account number")?;
        let id = Uuid::new_v4();
        let account_number = format!("BIL-{sequence:06}");
        sqlx::query(
            r#"
            INSERT INTO fees_billing_accounts (
                id, tenant_id, learner_id, account_number, opened_on,
                idempotency_key, created_by
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .bind(request.learner_id)
        .bind(&account_number)
        .bind(request.opened_on)
        .bind(&idempotency_key)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| fees_database_error(error, "Failed to create billing account"))?;
        append_fees_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "fees.billing_accounts.create",
            "fees_billing_account",
            id,
            json!({ "learner_id": learner.id, "account_number": account_number }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit billing account")?;
        Self::get_by_id(pool, tenant_id, id, None)
            .await?
            .ok_or_else(|| anyhow!("Created billing account could not be loaded"))
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateBillingAccountRequest,
    ) -> Result<Option<BillingAccountResponse>> {
        if request.expected_version <= 0 {
            bail!("Billing account version is invalid");
        }
        let actor_id = actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start billing account update")?;
        let current = sqlx::query_as::<_, BillingAccountRecord>(
            r#"
            SELECT id, learner_id, account_number, opened_on, status, version,
                   created_by, closed_by, closed_at, created_at, updated_at
              FROM fees_billing_accounts
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
             FOR UPDATE
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to lock billing account")?;
        let Some(current) = current else {
            transaction.rollback().await.ok();
            return Ok(None);
        };
        ensure_version(current.version, request.expected_version)?;
        if current.status == "closed" {
            bail!("A closed billing account is immutable");
        }
        let status = request.status.as_str();
        sqlx::query(
            r#"
            UPDATE fees_billing_accounts
               SET status = $3, version = version + 1,
                   closed_by = CASE WHEN $3 = 'closed' THEN $4 ELSE NULL END,
                   closed_at = CASE WHEN $3 = 'closed' THEN NOW() ELSE NULL END
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(status)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| fees_database_error(error, "Failed to update billing account"))?;
        append_fees_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "fees.billing_accounts.update",
            "fees_billing_account",
            id,
            json!({ "previous_status": current.status, "status": status, "previous_version": current.version }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit billing account update")?;
        Self::get_by_id(pool, tenant_id, id, None).await
    }
}

pub struct FeeStructureOps;

impl FeeStructureOps {
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
    ) -> Result<(Vec<FeeStructureResponse>, i64)> {
        validate_structure_status_filter(status)?;
        let page = page.max(1);
        let per_page = per_page.clamp(1, 100);
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, FeeStructureResponse>(&format!(
            "{} WHERE tenant_id = $1 AND deleted_at IS NULL
                    AND ($2::TEXT IS NULL OR code ILIKE $2 OR name ILIKE $2)
                    AND ($3::TEXT IS NULL OR status = $3)
                  ORDER BY status, code LIMIT $4 OFFSET $5",
            fee_structure_select()
        ))
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list fee structures")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM fees_fee_structures
             WHERE tenant_id = $1 AND deleted_at IS NULL
               AND ($2::TEXT IS NULL OR code ILIKE $2 OR name ILIKE $2)
               AND ($3::TEXT IS NULL OR status = $3)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .fetch_one(pool)
        .await
        .context("Failed to count fee structures")?;
        Ok((rows, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<FeeStructureResponse>> {
        sqlx::query_as::<_, FeeStructureResponse>(&format!(
            "{} WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
            fee_structure_select()
        ))
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load fee structure")
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateFeeStructureRequest,
    ) -> Result<FeeStructureResponse> {
        request
            .validate()
            .map_err(|_| anyhow!("Fee structure request is invalid"))?;
        validate_structure_input(pool, tenant_id, StructureInput::from_create(request)).await?;
        let actor_id = actor_id(actor)?;
        let idempotency_key = required(&request.idempotency_key, "Idempotency key")?;
        let code = required(&request.code, "Fee code")?.to_ascii_uppercase();
        let name = required(&request.name, "Fee name")?;
        let description = optional(request.description.as_deref());
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start fee structure transaction")?;
        lock_tenant(&mut transaction, tenant_id, "fee-structure").await?;
        if let Some(existing_id) = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM fees_fee_structures WHERE tenant_id = $1 AND idempotency_key = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(&idempotency_key)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to inspect fee structure idempotency")?
        {
            transaction.rollback().await.ok();
            return Self::get_by_id(pool, tenant_id, existing_id)
                .await?
                .ok_or_else(|| anyhow!("The idempotent fee structure could not be loaded"));
        }
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO fees_fee_structures (
                id, tenant_id, academic_year_id, academic_term_id, grade_level_id,
                currency_id, receivable_account_id, revenue_account_id, code,
                name, description, amount_minor, idempotency_key, created_by
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .bind(request.academic_year_id)
        .bind(request.academic_term_id)
        .bind(request.grade_level_id)
        .bind(request.currency_id)
        .bind(request.receivable_account_id)
        .bind(request.revenue_account_id)
        .bind(&code)
        .bind(&name)
        .bind(description)
        .bind(request.amount_minor)
        .bind(&idempotency_key)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| fees_database_error(error, "Failed to create fee structure"))?;
        append_fees_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "fees.fee_structures.create",
            "fees_fee_structure",
            id,
            json!({ "code": code, "amount_minor": request.amount_minor, "currency_id": request.currency_id }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit fee structure")?;
        Self::get_by_id(pool, tenant_id, id)
            .await?
            .ok_or_else(|| anyhow!("Created fee structure could not be loaded"))
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateFeeStructureRequest,
    ) -> Result<Option<FeeStructureResponse>> {
        request
            .validate()
            .map_err(|_| anyhow!("Fee structure request is invalid"))?;
        validate_structure_input(pool, tenant_id, StructureInput::from_update(request)).await?;
        if request.expected_version <= 0 {
            bail!("Fee structure version is invalid");
        }
        let code = required(&request.code, "Fee code")?.to_ascii_uppercase();
        let name = required(&request.name, "Fee name")?;
        let description = optional(request.description.as_deref());
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start fee structure update")?;
        let current = lock_structure(&mut transaction, tenant_id, id).await?;
        let Some((status, version)) = current else {
            transaction.rollback().await.ok();
            return Ok(None);
        };
        ensure_version(version, request.expected_version)?;
        if status != "draft" {
            bail!("Only a draft fee structure can be edited");
        }
        sqlx::query(
            r#"
            UPDATE fees_fee_structures
               SET academic_year_id = $3, academic_term_id = $4, grade_level_id = $5,
                   currency_id = $6, receivable_account_id = $7, revenue_account_id = $8,
                   code = $9, name = $10, description = $11, amount_minor = $12,
                   version = version + 1
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(request.academic_year_id)
        .bind(request.academic_term_id)
        .bind(request.grade_level_id)
        .bind(request.currency_id)
        .bind(request.receivable_account_id)
        .bind(request.revenue_account_id)
        .bind(&code)
        .bind(&name)
        .bind(description)
        .bind(request.amount_minor)
        .execute(&mut *transaction)
        .await
        .map_err(|error| fees_database_error(error, "Failed to update fee structure"))?;
        append_fees_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "fees.fee_structures.update",
            "fees_fee_structure",
            id,
            json!({ "previous_version": version, "code": code }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit fee structure update")?;
        Self::get_by_id(pool, tenant_id, id).await
    }

    pub async fn delete(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<DeleteOutcome> {
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start fee structure removal")?;
        let current = lock_structure(&mut transaction, tenant_id, id).await?;
        let Some((status, version)) = current else {
            transaction.rollback().await.ok();
            return Ok(DeleteOutcome::NotFound);
        };
        ensure_version(version, expected_version)?;
        if status != "draft" {
            bail!("Only a draft fee structure can be removed");
        }
        sqlx::query(
            "UPDATE fees_fee_structures SET deleted_at = NOW(), version = version + 1 WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to remove fee structure")?;
        append_fees_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "fees.fee_structures.delete",
            "fees_fee_structure",
            id,
            json!({ "previous_version": version }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit fee structure removal")?;
        Ok(DeleteOutcome::Deleted)
    }

    pub async fn activate(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<FeeStructureResponse>> {
        transition_structure(
            pool,
            tenant_id,
            id,
            actor,
            request_context,
            expected_version,
            "draft",
            "active",
            "fees.fee_structures.activate",
        )
        .await
    }

    pub async fn retire(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<FeeStructureResponse>> {
        transition_structure(
            pool,
            tenant_id,
            id,
            actor,
            request_context,
            expected_version,
            "active",
            "retired",
            "fees.fee_structures.retire",
        )
        .await
    }
}

struct StructureInput {
    academic_year_id: Uuid,
    academic_term_id: Option<Uuid>,
    grade_level_id: Option<Uuid>,
    currency_id: Uuid,
    receivable_account_id: Uuid,
    revenue_account_id: Uuid,
    amount_minor: i64,
}

impl StructureInput {
    fn from_create(value: &CreateFeeStructureRequest) -> Self {
        Self {
            academic_year_id: value.academic_year_id,
            academic_term_id: value.academic_term_id,
            grade_level_id: value.grade_level_id,
            currency_id: value.currency_id,
            receivable_account_id: value.receivable_account_id,
            revenue_account_id: value.revenue_account_id,
            amount_minor: value.amount_minor,
        }
    }

    fn from_update(value: &UpdateFeeStructureRequest) -> Self {
        Self {
            academic_year_id: value.academic_year_id,
            academic_term_id: value.academic_term_id,
            grade_level_id: value.grade_level_id,
            currency_id: value.currency_id,
            receivable_account_id: value.receivable_account_id,
            revenue_account_id: value.revenue_account_id,
            amount_minor: value.amount_minor,
        }
    }
}

async fn validate_structure_input(
    pool: &PgPool,
    tenant_id: Uuid,
    input: StructureInput,
) -> Result<()> {
    if !(1..=MAX_AMOUNT_MINOR).contains(&input.amount_minor) {
        bail!("Fee amount is invalid");
    }
    let year = AcademicYearOps::get_by_id(pool, tenant_id, input.academic_year_id)
        .await?
        .ok_or_else(|| anyhow!("The academic year is not available"))?;
    if year.status == "closed" {
        bail!("A closed academic year cannot receive a fee structure");
    }
    if let Some(term_id) = input.academic_term_id {
        let term = AcademicTermOps::get_by_id(pool, tenant_id, term_id)
            .await?
            .ok_or_else(|| anyhow!("The academic term is not available"))?;
        if term.academic_year_id != input.academic_year_id || term.status == "closed" {
            bail!("The fee term must be available inside its academic year");
        }
    }
    if let Some(grade_id) = input.grade_level_id {
        let grade = AcademicGradeLevelOps::get_by_id(pool, tenant_id, grade_id)
            .await?
            .ok_or_else(|| anyhow!("The grade level is not available"))?;
        if grade.status != "active" {
            bail!("The fee grade level must be active");
        }
    }
    let currency = CurrencyOps::get_by_id(pool, tenant_id, input.currency_id)
        .await?
        .ok_or_else(|| anyhow!("The fee currency is not available"))?;
    if currency.status != "active" {
        bail!("The fee currency must be active");
    }
    let receivable = AccountOps::get_by_id(pool, tenant_id, input.receivable_account_id)
        .await?
        .ok_or_else(|| anyhow!("The receivable account is not available"))?;
    let revenue = AccountOps::get_by_id(pool, tenant_id, input.revenue_account_id)
        .await?
        .ok_or_else(|| anyhow!("The revenue account is not available"))?;
    validate_fee_account(
        &receivable,
        "asset",
        input.currency_id,
        currency.is_reporting,
    )?;
    validate_fee_account(&revenue, "income", input.currency_id, currency.is_reporting)?;
    if receivable.id == revenue.id {
        bail!("Receivable and revenue accounts must be different");
    }
    Ok(())
}

fn validate_fee_account(
    account: &cp_finance::ledger::AccountResponse,
    account_type: &str,
    currency_id: Uuid,
    currency_is_reporting: bool,
) -> Result<()> {
    if account.account_type != account_type
        || account.status != "active"
        || !account.accepts_postings
    {
        bail!("Fee structures require active posting {account_type} accounts");
    }
    match account.currency_mode.as_str() {
        "reporting" if !currency_is_reporting => {
            bail!("The selected fee currency is not allowed by a reporting-currency account")
        }
        "single" if account.currency_id != Some(currency_id) => {
            bail!("The selected fee currency is not allowed by a single-currency account")
        }
        "reporting" | "single" | "multi" => Ok(()),
        _ => bail!("The Finance account currency policy is invalid"),
    }
}

#[allow(clippy::too_many_arguments)]
async fn transition_structure(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    expected_version: i32,
    from_status: &str,
    to_status: &str,
    action: &str,
) -> Result<Option<FeeStructureResponse>> {
    let actor_id = actor_id(actor)?;
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to start fee structure transition")?;
    let current = lock_structure(&mut transaction, tenant_id, id).await?;
    let Some((status, version)) = current else {
        transaction.rollback().await.ok();
        return Ok(None);
    };
    ensure_version(version, expected_version)?;
    if status == to_status {
        transaction.rollback().await.ok();
        return FeeStructureOps::get_by_id(pool, tenant_id, id).await;
    }
    if status != from_status {
        bail!("Fee structure cannot move from {status} to {to_status}");
    }
    let (actor_column, time_column) = if to_status == "active" {
        ("activated_by", "activated_at")
    } else {
        ("retired_by", "retired_at")
    };
    let query = format!(
        "UPDATE fees_fee_structures SET status = $3, version = version + 1, {actor_column} = $4, {time_column} = NOW() WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL"
    );
    sqlx::query(&query)
        .bind(tenant_id)
        .bind(id)
        .bind(to_status)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| fees_database_error(error, "Failed to transition fee structure"))?;
    append_fees_audit(
        &mut transaction,
        tenant_id,
        actor,
        request_context,
        action,
        "fees_fee_structure",
        id,
        json!({ "previous_status": status, "status": to_status, "previous_version": version }),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit fee structure transition")?;
    FeeStructureOps::get_by_id(pool, tenant_id, id).await
}

async fn load_billing_account(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<BillingAccountRecord>> {
    sqlx::query_as::<_, BillingAccountRecord>(
        r#"
        SELECT id, learner_id, account_number, opened_on, status, version,
               created_by, closed_by, closed_at, created_at, updated_at
          FROM fees_billing_accounts
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("Failed to load billing account")
}

async fn hydrate_billing_account(
    pool: &PgPool,
    tenant_id: Uuid,
    record: BillingAccountRecord,
) -> Result<BillingAccountResponse> {
    let learner = LearnerOps::get_by_id(pool, tenant_id, record.learner_id)
        .await?
        .ok_or_else(|| anyhow!("The billing account learner is unavailable"))?;
    Ok(BillingAccountResponse {
        id: record.id,
        learner_id: record.learner_id,
        learner_number: learner.learner_number,
        learner_name: learner.display_name,
        learner_status: learner.status,
        account_number: record.account_number,
        opened_on: record.opened_on,
        status: record.status,
        version: record.version,
        created_by: record.created_by,
        closed_by: record.closed_by,
        closed_at: record.closed_at,
        created_at: record.created_at,
        updated_at: record.updated_at,
    })
}

async fn lock_structure(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<(String, i32)>> {
    sqlx::query_as::<_, (String, i32)>(
        "SELECT status, version FROM fees_fee_structures WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock fee structure")
}

fn fee_structure_select() -> &'static str {
    r#"
    SELECT id, academic_year_id, academic_term_id, grade_level_id,
           currency_id, receivable_account_id, revenue_account_id,
           code, name, description, amount_minor, status, version,
           created_by, activated_by, activated_at, retired_by, retired_at,
           created_at, updated_at
      FROM fees_fee_structures
    "#
}

fn account_reference(value: cp_finance::ledger::AccountResponse) -> FeeAccountReference {
    FeeAccountReference {
        id: value.id,
        code: value.code,
        name: value.name,
        currency_mode: value.currency_mode,
        currency_id: value.currency_id,
    }
}

async fn lock_tenant(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    scope: &str,
) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("{scope}:{tenant_id}"))
        .execute(&mut **transaction)
        .await
        .context("Failed to lock Fees numbering")?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn append_fees_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    action: &str,
    target_kind: &str,
    target_id: Uuid,
    metadata: serde_json::Value,
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
        .with_target(AuditTarget::new(target_kind, target_id.to_string()))
        .with_redacted_metadata(metadata.as_object().cloned().unwrap_or_default()),
    )
    .await
    .context("Failed to audit Fees operation")?;
    Ok(())
}

fn actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("Authenticated person actor is required"))
}

fn ensure_version(current: i32, expected: i32) -> Result<()> {
    if current != expected {
        bail!("The record changed after it was loaded; refresh and try again");
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
        .map(ToOwned::to_owned)
}

fn validate_billing_status_filter(status: Option<&str>) -> Result<()> {
    if status.is_some_and(|value| !matches!(value, "active" | "on_hold" | "closed")) {
        bail!("Billing account status filter is invalid");
    }
    Ok(())
}

fn validate_structure_status_filter(status: Option<&str>) -> Result<()> {
    if status.is_some_and(|value| !matches!(value, "draft" | "active" | "retired")) {
        bail!("Fee structure status filter is invalid");
    }
    Ok(())
}

fn fees_database_error(error: sqlx::Error, context: &str) -> anyhow::Error {
    if let sqlx::Error::Database(database) = &error {
        let message = database.message();
        if message.contains("duplicate key") {
            return anyhow!("A Fees record already uses these details");
        }
        if message.contains("fee")
            || message.contains("billing")
            || message.contains("academic")
            || message.contains("Finance")
        {
            return anyhow!(message.to_string());
        }
    }
    anyhow!("{context}: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_values_are_stable() {
        assert_eq!(BillingAccountStatus::OnHold.as_str(), "on_hold");
        assert_eq!(FeeStructureStatus::Retired.as_str(), "retired");
    }

    #[test]
    fn filters_reject_unknown_values() {
        assert!(validate_billing_status_filter(Some("active")).is_ok());
        assert!(validate_billing_status_filter(Some("pending")).is_err());
        assert!(validate_structure_status_filter(Some("draft")).is_ok());
        assert!(validate_structure_status_filter(Some("posted")).is_err());
    }

    #[test]
    fn fee_amount_bounds_are_integer_minor_units() {
        assert!((1..=MAX_AMOUNT_MINOR).contains(&1));
        assert!(!(1..=MAX_AMOUNT_MINOR).contains(&0));
        assert!(!(1..=MAX_AMOUNT_MINOR).contains(&(MAX_AMOUNT_MINOR + 1)));
    }
}
