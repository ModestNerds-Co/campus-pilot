//
//  cp-procurement
//  requisitions.rs
//
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//
//! Owns currency-safe requisitions and their approval lifecycle.

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_finance::ledger::{CurrencyOps, CurrencyResponse};
use cp_hr_payroll::{models::EmployeeReference, ops::EmployeeOps};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;
use validator::Validate;

use crate::suppliers::{SupplierOps, SupplierSnapshot};

const MAX_MONEY_MINOR: i64 = 9_000_000_000_000_000;
const MAX_REQUISITION_LINES: usize = 200;

#[derive(Debug, Deserialize)]
pub struct RequisitionListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub requester_employee_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct RequesterCandidateQuery {
    pub search: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct RequisitionLineInput {
    #[validate(length(min = 1, max = 500))]
    pub description: String,
    #[validate(range(min = 1, max = 1_000_000_000))]
    pub quantity: i32,
    #[validate(length(max = 40))]
    pub unit_label: Option<String>,
    #[validate(range(min = 0))]
    pub estimated_unit_amount_minor: i64,
    pub preferred_supplier_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateRequisitionRequest {
    pub requester_employee_id: Uuid,
    pub currency_id: Uuid,
    #[validate(length(min = 1, max = 180))]
    pub title: String,
    #[validate(length(max = 2000))]
    pub purpose: Option<String>,
    pub needed_by: Option<NaiveDate>,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
    #[validate(length(min = 1, max = 200), nested)]
    pub lines: Vec<RequisitionLineInput>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateRequisitionRequest {
    pub requester_employee_id: Uuid,
    pub currency_id: Uuid,
    #[validate(length(min = 1, max = 180))]
    pub title: String,
    #[validate(length(max = 2000))]
    pub purpose: Option<String>,
    pub needed_by: Option<NaiveDate>,
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 200), nested)]
    pub lines: Vec<RequisitionLineInput>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct VersionRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct DecisionRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(max = 1000))]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct RequisitionSummary {
    pub id: Uuid,
    pub requisition_number: String,
    pub requester_employee_id: Uuid,
    pub requester_account_id: Option<Uuid>,
    pub requester_employee_number: String,
    pub requester_name: String,
    pub currency_id: Uuid,
    pub currency_code: String,
    pub currency_minor_units: i16,
    pub title: String,
    pub purpose: Option<String>,
    pub needed_by: Option<NaiveDate>,
    pub status: String,
    pub version: i32,
    pub total_minor: i64,
    pub line_count: i64,
    pub created_by: Uuid,
    pub submitted_by: Option<Uuid>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub decided_by: Option<Uuid>,
    pub decided_at: Option<DateTime<Utc>>,
    pub decision_note: Option<String>,
    pub cancelled_by: Option<Uuid>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub cancellation_note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct RequisitionLineResponse {
    pub id: Uuid,
    pub line_number: i32,
    pub description: String,
    pub quantity: i32,
    pub unit_label: Option<String>,
    pub estimated_unit_amount_minor: i64,
    pub estimated_line_amount_minor: i64,
    pub preferred_supplier_id: Option<Uuid>,
    pub preferred_supplier_number: Option<String>,
    pub preferred_supplier_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequisitionResponse {
    #[serde(flatten)]
    pub summary: RequisitionSummary,
    pub lines: Vec<RequisitionLineResponse>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedRequisitionsResponse {
    pub requisitions: Vec<RequisitionSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcurementCurrencyReference {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub symbol: Option<String>,
    pub minor_units: i16,
    pub is_reporting: bool,
}

#[derive(Debug, Serialize)]
pub struct ProcurementReferenceData {
    pub currencies: Vec<ProcurementCurrencyReference>,
}

#[derive(Debug, Serialize)]
pub struct RequesterCandidatesResponse {
    pub employees: Vec<EmployeeReference>,
}

pub struct ProcurementReferenceOps;

impl ProcurementReferenceOps {
    pub async fn currencies(pool: &PgPool, tenant_id: Uuid) -> Result<ProcurementReferenceData> {
        let (currencies, _) = CurrencyOps::list(pool, tenant_id, 1, 100, None, Some("active"))
            .await
            .context("Failed to load Procurement currencies")?;
        Ok(ProcurementReferenceData {
            currencies: currencies
                .into_iter()
                .map(|currency| ProcurementCurrencyReference {
                    id: currency.id,
                    code: currency.code,
                    name: currency.name,
                    symbol: currency.symbol,
                    minor_units: currency.minor_units,
                    is_reporting: currency.is_reporting,
                })
                .collect(),
        })
    }

    pub async fn requester_candidates(
        pool: &PgPool,
        tenant_id: Uuid,
        search: Option<&str>,
    ) -> Result<RequesterCandidatesResponse> {
        Ok(RequesterCandidatesResponse {
            employees: EmployeeOps::list_references(pool, tenant_id, search, Some("active"), 50)
                .await
                .context("Failed to load Procurement requester candidates")?,
        })
    }
}

pub struct RequisitionOps;

impl RequisitionOps {
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
        requester_employee_id: Option<Uuid>,
    ) -> Result<(Vec<RequisitionSummary>, i64)> {
        validate_status(status)?;
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, RequisitionSummary>(&format!(
            "{} ORDER BY requisition.created_at DESC LIMIT $6 OFFSET $7",
            summary_query()
        ))
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(requester_employee_id)
        .bind(Option::<Uuid>::None)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list Procurement requisitions")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
              FROM procurement_requisitions AS requisition
             WHERE requisition.tenant_id = $1 AND requisition.deleted_at IS NULL
               AND ($2::TEXT IS NULL OR requisition.requisition_number ILIKE $2
                    OR requisition.title ILIKE $2 OR requisition.requester_name ILIKE $2
                    OR requisition.requester_employee_number ILIKE $2)
               AND ($3::TEXT IS NULL OR requisition.status = $3)
               AND ($4::UUID IS NULL OR requisition.requester_employee_id = $4)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(requester_employee_id)
        .fetch_one(pool)
        .await
        .context("Failed to count Procurement requisitions")?;
        Ok((rows, total))
    }

    pub async fn get(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<RequisitionResponse>> {
        let summary = sqlx::query_as::<_, RequisitionSummary>(summary_query())
            .bind(tenant_id)
            .bind(Option::<String>::None)
            .bind(Option::<String>::None)
            .bind(Option::<Uuid>::None)
            .bind(Some(id))
            .fetch_optional(pool)
            .await
            .context("Failed to read Procurement requisition")?;
        let Some(summary) = summary else {
            return Ok(None);
        };
        let lines = sqlx::query_as::<_, RequisitionLineResponse>(
            r#"
            SELECT id, line_number, description, quantity, unit_label,
                   estimated_unit_amount_minor, estimated_line_amount_minor,
                   preferred_supplier_id, preferred_supplier_number, preferred_supplier_name
              FROM procurement_requisition_lines
             WHERE tenant_id = $1 AND requisition_id = $2 AND deleted_at IS NULL
             ORDER BY line_number
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_all(pool)
        .await
        .context("Failed to read Procurement requisition lines")?;
        Ok(Some(RequisitionResponse { summary, lines }))
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateRequisitionRequest,
    ) -> Result<RequisitionResponse> {
        let actor_id = actor_id(actor)?;
        let values = RequisitionValues::from_create(request)?;
        let references = resolve_references(pool, tenant_id, &values).await?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start requisition transaction")?;
        lock_tenant(&mut transaction, tenant_id).await?;
        if let Some(existing) = id_for_idempotency(
            &mut transaction,
            tenant_id,
            values.idempotency_key.as_deref().unwrap_or_default(),
        )
        .await?
        {
            transaction.rollback().await.ok();
            let existing = Self::get(pool, tenant_id, existing)
                .await?
                .ok_or_else(|| anyhow!("The idempotent requisition could not be loaded"))?;
            if !values.matches(&existing) {
                bail!("Idempotency key already belongs to another requisition request");
            }
            return Ok(existing);
        }
        let requisition_number = next_requisition_number(&mut transaction, tenant_id).await?;
        let requisition_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO procurement_requisitions (
                id, tenant_id, requisition_number, requester_employee_id,
                requester_account_id, requester_employee_number, requester_name,
                currency_id, currency_code, currency_minor_units, title, purpose,
                needed_by, idempotency_key, created_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            "#,
        )
        .bind(requisition_id)
        .bind(tenant_id)
        .bind(&requisition_number)
        .bind(references.requester.id)
        .bind(references.requester.account_id)
        .bind(&references.requester.employee_number)
        .bind(&references.requester.display_name)
        .bind(references.currency.id)
        .bind(&references.currency.code)
        .bind(references.currency.minor_units)
        .bind(&values.title)
        .bind(&values.purpose)
        .bind(values.needed_by)
        .bind(values.idempotency_key.as_deref())
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "Failed to create Procurement requisition"))?;
        insert_lines(
            &mut transaction,
            tenant_id,
            requisition_id,
            &values.lines,
            &references.suppliers,
        )
        .await?;
        append_requisition_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            RequisitionAudit {
                operation: "procurement.requisitions.create",
                id: requisition_id,
                reason: None,
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit requisition transaction")?;
        Self::get(pool, tenant_id, requisition_id)
            .await?
            .ok_or_else(|| anyhow!("The created requisition could not be loaded"))
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateRequisitionRequest,
    ) -> Result<Option<RequisitionResponse>> {
        let values = RequisitionValues::from_update(request)?;
        let references = resolve_references(pool, tenant_id, &values).await?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start requisition transaction")?;
        let current = lock_requisition(&mut transaction, tenant_id, id).await?;
        let Some(current) = current else {
            return Ok(None);
        };
        ensure_version(current.version, request.expected_version, "Requisition")?;
        ensure_state(
            &current.status,
            "draft",
            "Only a draft requisition can be edited",
        )?;
        if current.currency_id != values.currency_id {
            bail!(
                "A requisition currency is fixed after creation; remove the draft and create it again"
            );
        }
        sqlx::query(
            r#"
            UPDATE procurement_requisitions
               SET requester_employee_id = $3, requester_account_id = $4,
                   requester_employee_number = $5, requester_name = $6,
                   currency_id = $7, currency_code = $8, currency_minor_units = $9,
                   title = $10, purpose = $11, needed_by = $12, version = version + 1
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(references.requester.id)
        .bind(references.requester.account_id)
        .bind(&references.requester.employee_number)
        .bind(&references.requester.display_name)
        .bind(references.currency.id)
        .bind(&references.currency.code)
        .bind(references.currency.minor_units)
        .bind(&values.title)
        .bind(&values.purpose)
        .bind(values.needed_by)
        .execute(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "Failed to update Procurement requisition"))?;
        sqlx::query(
            "DELETE FROM procurement_requisition_lines WHERE tenant_id = $1 AND requisition_id = $2",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to replace requisition lines")?;
        insert_lines(
            &mut transaction,
            tenant_id,
            id,
            &values.lines,
            &references.suppliers,
        )
        .await?;
        append_requisition_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            RequisitionAudit {
                operation: "procurement.requisitions.update",
                id,
                reason: None,
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit requisition transaction")?;
        Self::get(pool, tenant_id, id).await
    }

    pub async fn submit(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<RequisitionResponse>> {
        let actor_id = actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start requisition transaction")?;
        let current = lock_requisition(&mut transaction, tenant_id, id).await?;
        let Some(current) = current else {
            return Ok(None);
        };
        ensure_version(current.version, expected_version, "Requisition")?;
        ensure_state(
            &current.status,
            "draft",
            "Only a draft requisition can be submitted",
        )?;
        let (line_count, total_minor) = requisition_totals(&mut transaction, tenant_id, id).await?;
        if line_count == 0 {
            bail!("A requisition requires at least one line");
        }
        if total_minor <= 0 {
            bail!("A requisition total must be greater than zero");
        }
        sqlx::query(
            r#"
            UPDATE procurement_requisitions
               SET status = 'submitted', submitted_by = $3, submitted_at = NOW(),
                   version = version + 1
             WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to submit Procurement requisition")?;
        append_requisition_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            RequisitionAudit {
                operation: "procurement.requisitions.submit",
                id,
                reason: None,
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit requisition transaction")?;
        Self::get(pool, tenant_id, id).await
    }

    pub async fn approve(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &DecisionRequest,
    ) -> Result<Option<RequisitionResponse>> {
        decide(
            pool,
            tenant_id,
            id,
            actor,
            request_context,
            request,
            DecisionKind::Approve,
        )
        .await
    }

    pub async fn reject(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &DecisionRequest,
    ) -> Result<Option<RequisitionResponse>> {
        decide(
            pool,
            tenant_id,
            id,
            actor,
            request_context,
            request,
            DecisionKind::Reject,
        )
        .await
    }

    pub async fn cancel(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &DecisionRequest,
    ) -> Result<Option<RequisitionResponse>> {
        let actor_id = actor_id(actor)?;
        let note = optional(request.note.as_deref());
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start requisition transaction")?;
        let current = lock_requisition(&mut transaction, tenant_id, id).await?;
        let Some(current) = current else {
            return Ok(None);
        };
        ensure_version(current.version, request.expected_version, "Requisition")?;
        if !matches!(current.status.as_str(), "draft" | "submitted") {
            bail!("Only a draft or submitted requisition can be cancelled");
        }
        if current.created_by != actor_id && current.requester_account_id != Some(actor_id) {
            bail!("Only the requisition creator or requester can cancel it");
        }
        sqlx::query(
            r#"
            UPDATE procurement_requisitions
               SET status = 'cancelled', cancelled_by = $3, cancelled_at = NOW(),
                   cancellation_note = $4, version = version + 1
             WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(actor_id)
        .bind(&note)
        .execute(&mut *transaction)
        .await
        .context("Failed to cancel Procurement requisition")?;
        append_requisition_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            RequisitionAudit {
                operation: "procurement.requisitions.cancel",
                id,
                reason: note.as_deref(),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit requisition transaction")?;
        Self::get(pool, tenant_id, id).await
    }

    pub async fn delete(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<bool> {
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start requisition transaction")?;
        let current = lock_requisition(&mut transaction, tenant_id, id).await?;
        let Some(current) = current else {
            return Ok(false);
        };
        ensure_version(current.version, expected_version, "Requisition")?;
        ensure_state(
            &current.status,
            "draft",
            "Only a draft requisition can be removed",
        )?;
        sqlx::query(
            "DELETE FROM procurement_requisition_lines WHERE tenant_id = $1 AND requisition_id = $2",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to remove draft requisition lines")?;
        sqlx::query(
            "UPDATE procurement_requisitions SET deleted_at = NOW(), version = version + 1 WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to remove draft Procurement requisition")?;
        append_requisition_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            RequisitionAudit {
                operation: "procurement.requisitions.delete",
                id,
                reason: None,
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit requisition transaction")?;
        Ok(true)
    }
}

#[derive(Debug)]
struct RequisitionValues {
    requester_employee_id: Uuid,
    currency_id: Uuid,
    title: String,
    purpose: Option<String>,
    needed_by: Option<NaiveDate>,
    idempotency_key: Option<String>,
    lines: Vec<LineValues>,
}

impl RequisitionValues {
    fn from_create(request: &CreateRequisitionRequest) -> Result<Self> {
        Self::new(
            request.requester_employee_id,
            request.currency_id,
            &request.title,
            request.purpose.as_deref(),
            request.needed_by,
            Some(request.idempotency_key.as_str()),
            &request.lines,
        )
    }

    fn from_update(request: &UpdateRequisitionRequest) -> Result<Self> {
        Self::new(
            request.requester_employee_id,
            request.currency_id,
            &request.title,
            request.purpose.as_deref(),
            request.needed_by,
            None,
            &request.lines,
        )
    }

    fn new(
        requester_employee_id: Uuid,
        currency_id: Uuid,
        title: &str,
        purpose: Option<&str>,
        needed_by: Option<NaiveDate>,
        idempotency_key: Option<&str>,
        lines: &[RequisitionLineInput],
    ) -> Result<Self> {
        if lines.is_empty() || lines.len() > MAX_REQUISITION_LINES {
            bail!("A requisition requires between 1 and {MAX_REQUISITION_LINES} lines");
        }
        let lines = lines
            .iter()
            .map(LineValues::parse)
            .collect::<Result<Vec<_>>>()?;
        checked_total(&lines)?;
        Ok(Self {
            requester_employee_id,
            currency_id,
            title: required(title, "Requisition title")?,
            purpose: optional(purpose),
            needed_by,
            idempotency_key: idempotency_key
                .map(|value| required(value, "Idempotency key"))
                .transpose()?,
            lines,
        })
    }

    fn matches(&self, existing: &RequisitionResponse) -> bool {
        self.requester_employee_id == existing.summary.requester_employee_id
            && self.currency_id == existing.summary.currency_id
            && self.title == existing.summary.title
            && self.purpose == existing.summary.purpose
            && self.needed_by == existing.summary.needed_by
            && self.lines.len() == existing.lines.len()
            && self
                .lines
                .iter()
                .zip(&existing.lines)
                .all(|(requested, stored)| {
                    requested.description == stored.description
                        && requested.quantity == stored.quantity
                        && requested.unit_label == stored.unit_label
                        && requested.estimated_unit_amount_minor
                            == stored.estimated_unit_amount_minor
                        && requested.preferred_supplier_id == stored.preferred_supplier_id
                })
    }
}

#[derive(Debug)]
struct LineValues {
    description: String,
    quantity: i32,
    unit_label: Option<String>,
    estimated_unit_amount_minor: i64,
    estimated_line_amount_minor: i64,
    preferred_supplier_id: Option<Uuid>,
}

impl LineValues {
    fn parse(input: &RequisitionLineInput) -> Result<Self> {
        if input.quantity <= 0 {
            bail!("Requisition line quantity must be greater than zero");
        }
        if !(0..=MAX_MONEY_MINOR).contains(&input.estimated_unit_amount_minor) {
            bail!("Requisition line amount is outside the supported range");
        }
        let estimated_line_amount_minor = i64::from(input.quantity)
            .checked_mul(input.estimated_unit_amount_minor)
            .filter(|amount| *amount <= MAX_MONEY_MINOR)
            .ok_or_else(|| anyhow!("Requisition line total is too large"))?;
        Ok(Self {
            description: required(&input.description, "Requisition line description")?,
            quantity: input.quantity,
            unit_label: optional(input.unit_label.as_deref()),
            estimated_unit_amount_minor: input.estimated_unit_amount_minor,
            estimated_line_amount_minor,
            preferred_supplier_id: input.preferred_supplier_id,
        })
    }
}

struct ResolvedReferences {
    requester: EmployeeReference,
    currency: CurrencyResponse,
    suppliers: HashMap<Uuid, SupplierSnapshot>,
}

async fn resolve_references(
    pool: &PgPool,
    tenant_id: Uuid,
    values: &RequisitionValues,
) -> Result<ResolvedReferences> {
    let requester = EmployeeOps::get_reference(pool, tenant_id, values.requester_employee_id)
        .await?
        .ok_or_else(|| anyhow!("Requisition requester was not found in HR"))?;
    if requester.employment_status != "active" {
        bail!("Requisition requester must be an active HR employee");
    }
    let currency = CurrencyOps::get_by_id(pool, tenant_id, values.currency_id)
        .await?
        .ok_or_else(|| anyhow!("Requisition currency was not found in Finance"))?;
    if currency.status != "active" {
        bail!("Requisition currency must be active in Finance");
    }
    let supplier_ids = values
        .lines
        .iter()
        .filter_map(|line| line.preferred_supplier_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let suppliers = SupplierOps::active_snapshots(pool, tenant_id, &supplier_ids)
        .await?
        .into_iter()
        .map(|supplier| (supplier.id, supplier))
        .collect::<HashMap<_, _>>();
    if suppliers.len() != supplier_ids.len() {
        bail!("Every preferred supplier must be active in Procurement");
    }
    Ok(ResolvedReferences {
        requester,
        currency,
        suppliers,
    })
}

#[derive(Debug, FromRow)]
struct RequisitionState {
    requester_account_id: Option<Uuid>,
    currency_id: Uuid,
    status: String,
    version: i32,
    created_by: Uuid,
    submitted_by: Option<Uuid>,
}

async fn lock_requisition(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<RequisitionState>> {
    sqlx::query_as::<_, RequisitionState>(
        r#"
        SELECT requester_account_id, currency_id, status, version,
               created_by, submitted_by
          FROM procurement_requisitions
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock Procurement requisition")
}

async fn insert_lines(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    requisition_id: Uuid,
    lines: &[LineValues],
    suppliers: &HashMap<Uuid, SupplierSnapshot>,
) -> Result<()> {
    for (index, line) in lines.iter().enumerate() {
        let supplier = line.preferred_supplier_id.and_then(|id| suppliers.get(&id));
        sqlx::query(
            r#"
            INSERT INTO procurement_requisition_lines (
                tenant_id, requisition_id, line_number, description, quantity,
                unit_label, estimated_unit_amount_minor, estimated_line_amount_minor,
                preferred_supplier_id, preferred_supplier_number, preferred_supplier_name
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(tenant_id)
        .bind(requisition_id)
        .bind(i32::try_from(index + 1).context("Too many requisition lines")?)
        .bind(&line.description)
        .bind(line.quantity)
        .bind(&line.unit_label)
        .bind(line.estimated_unit_amount_minor)
        .bind(line.estimated_line_amount_minor)
        .bind(supplier.map(|value| value.id))
        .bind(supplier.map(|value| value.supplier_number.as_str()))
        .bind(supplier.map(|value| value.legal_name.as_str()))
        .execute(&mut **transaction)
        .await
        .context("Failed to save Procurement requisition line")?;
    }
    Ok(())
}

enum DecisionKind {
    Approve,
    Reject,
}

impl DecisionKind {
    const fn status(&self) -> &'static str {
        match self {
            Self::Approve => "approved",
            Self::Reject => "rejected",
        }
    }

    const fn operation(&self) -> &'static str {
        match self {
            Self::Approve => "procurement.requisitions.approve",
            Self::Reject => "procurement.requisitions.reject",
        }
    }
}

async fn decide(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    request: &DecisionRequest,
    decision: DecisionKind,
) -> Result<Option<RequisitionResponse>> {
    let actor_id = actor_id(actor)?;
    let note = optional(request.note.as_deref());
    if matches!(decision, DecisionKind::Reject) && note.is_none() {
        bail!("A rejection reason is required");
    }
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to start requisition transaction")?;
    let current = lock_requisition(&mut transaction, tenant_id, id).await?;
    let Some(current) = current else {
        return Ok(None);
    };
    ensure_version(current.version, request.expected_version, "Requisition")?;
    ensure_state(
        &current.status,
        "submitted",
        "Only a submitted requisition can be decided",
    )?;
    if current.created_by == actor_id
        || current.requester_account_id == Some(actor_id)
        || current.submitted_by == Some(actor_id)
    {
        bail!("A requisition creator, requester, or submitter cannot decide their own request");
    }
    sqlx::query(
        r#"
        UPDATE procurement_requisitions
           SET status = $3, decided_by = $4, decided_at = NOW(), decision_note = $5,
               version = version + 1
         WHERE tenant_id = $1 AND id = $2
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .bind(decision.status())
    .bind(actor_id)
    .bind(&note)
    .execute(&mut *transaction)
    .await
    .context("Failed to decide Procurement requisition")?;
    append_requisition_audit(
        &mut transaction,
        tenant_id,
        actor,
        request_context,
        RequisitionAudit {
            operation: decision.operation(),
            id,
            reason: note.as_deref(),
        },
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit requisition transaction")?;
    RequisitionOps::get(pool, tenant_id, id).await
}

async fn id_for_idempotency(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    key: &str,
) -> Result<Option<Uuid>> {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id FROM procurement_requisitions
         WHERE tenant_id = $1 AND idempotency_key = $2 AND deleted_at IS NULL
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(key)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to resolve requisition idempotency")
}

async fn next_requisition_number(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
) -> Result<String> {
    let year = Utc::now().year();
    let number = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO procurement_requisition_sequences (tenant_id, calendar_year, last_number)
        VALUES ($1, $2, 1)
        ON CONFLICT (tenant_id, calendar_year)
        DO UPDATE SET last_number = procurement_requisition_sequences.last_number + 1,
                      deleted_at = NULL
        RETURNING last_number
        "#,
    )
    .bind(tenant_id)
    .bind(year)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to allocate requisition number")?;
    Ok(format!("REQ-{year}-{number:06}"))
}

async fn lock_tenant(transaction: &mut Transaction<'_, Postgres>, tenant_id: Uuid) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("procurement-requisition:{tenant_id}"))
        .execute(&mut **transaction)
        .await
        .context("Failed to lock Procurement requisition numbering")?;
    Ok(())
}

async fn requisition_totals(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<(i64, i64)> {
    #[derive(FromRow)]
    struct Totals {
        line_count: i64,
        total_minor: i64,
    }
    let totals = sqlx::query_as::<_, Totals>(
        r#"
        SELECT COUNT(*) AS line_count,
               COALESCE(SUM(estimated_line_amount_minor), 0)::BIGINT AS total_minor
          FROM procurement_requisition_lines
         WHERE tenant_id = $1 AND requisition_id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to total Procurement requisition")?;
    Ok((totals.line_count, totals.total_minor))
}

fn summary_query() -> &'static str {
    r#"
    SELECT requisition.id, requisition.requisition_number,
           requisition.requester_employee_id, requisition.requester_account_id,
           requisition.requester_employee_number, requisition.requester_name,
           requisition.currency_id, requisition.currency_code,
           requisition.currency_minor_units, requisition.title, requisition.purpose,
           requisition.needed_by, requisition.status, requisition.version,
           COALESCE(SUM(line.estimated_line_amount_minor), 0)::BIGINT AS total_minor,
           COUNT(line.id)::BIGINT AS line_count, requisition.created_by,
           requisition.submitted_by, requisition.submitted_at, requisition.decided_by,
           requisition.decided_at, requisition.decision_note, requisition.cancelled_by,
           requisition.cancelled_at, requisition.cancellation_note,
           requisition.created_at, requisition.updated_at
      FROM procurement_requisitions AS requisition
      LEFT JOIN procurement_requisition_lines AS line
        ON line.tenant_id = requisition.tenant_id
       AND line.requisition_id = requisition.id AND line.deleted_at IS NULL
     WHERE requisition.tenant_id = $1 AND requisition.deleted_at IS NULL
       AND ($2::TEXT IS NULL OR requisition.requisition_number ILIKE $2
            OR requisition.title ILIKE $2 OR requisition.requester_name ILIKE $2
            OR requisition.requester_employee_number ILIKE $2)
       AND ($3::TEXT IS NULL OR requisition.status = $3)
       AND ($4::UUID IS NULL OR requisition.requester_employee_id = $4)
       AND ($5::UUID IS NULL OR requisition.id = $5)
     GROUP BY requisition.id
    "#
}

struct RequisitionAudit<'a> {
    operation: &'static str,
    id: Uuid,
    reason: Option<&'a str>,
}

#[derive(FromRow)]
struct RequisitionAuditDetails {
    requisition_number: String,
    status: String,
    requester_employee_id: Uuid,
    currency_code: String,
    total_minor: i64,
    line_count: i64,
}

async fn append_requisition_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    audit: RequisitionAudit<'_>,
) -> Result<()> {
    let details = sqlx::query_as::<_, RequisitionAuditDetails>(
        r#"
        SELECT requisition.requisition_number, requisition.status,
               requisition.requester_employee_id, requisition.currency_code,
               COALESCE(SUM(line.estimated_line_amount_minor), 0)::BIGINT AS total_minor,
               COUNT(line.id)::BIGINT AS line_count
          FROM procurement_requisitions AS requisition
          LEFT JOIN procurement_requisition_lines AS line
            ON line.tenant_id = requisition.tenant_id
           AND line.requisition_id = requisition.id AND line.deleted_at IS NULL
         WHERE requisition.tenant_id = $1 AND requisition.id = $2
         GROUP BY requisition.id
        "#,
    )
    .bind(tenant_id)
    .bind(audit.id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to load Procurement requisition audit details")?;
    let outcome = if audit.operation.ends_with(".cancel") {
        AuditOutcome::Cancelled
    } else {
        AuditOutcome::Succeeded
    };
    let mut event = NewAuditEvent::new(tenant_id, actor, audit.operation, outcome, request_context)
        .with_target(AuditTarget::new(
            "procurement_requisition",
            audit.id.to_string(),
        ))
        .with_redacted_metadata(
            json!({
                "requisition_number": details.requisition_number,
                "status": details.status,
                "requester_employee_id": details.requester_employee_id,
                "currency_code": details.currency_code,
                "total_minor": details.total_minor,
                "line_count": details.line_count,
            })
            .as_object()
            .cloned()
            .unwrap_or_default(),
        );
    if let Some(reason) = audit.reason {
        event = event.with_reason(reason);
    }
    append_audit(&mut **transaction, &event)
        .await
        .context("Failed to audit Procurement requisition")?;
    Ok(())
}

fn checked_total(lines: &[LineValues]) -> Result<i64> {
    lines.iter().try_fold(0_i64, |total, line| {
        total
            .checked_add(line.estimated_line_amount_minor)
            .filter(|amount| *amount <= MAX_MONEY_MINOR)
            .ok_or_else(|| anyhow!("Requisition total is too large"))
    })
}

fn actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("Authenticated person actor is required"))
}

fn validate_status(status: Option<&str>) -> Result<()> {
    if status.is_some_and(|value| {
        !matches!(
            value,
            "draft" | "submitted" | "approved" | "rejected" | "cancelled"
        )
    }) {
        bail!("Requisition status filter is invalid");
    }
    Ok(())
}

fn ensure_version(actual: i32, expected: i32, label: &str) -> Result<()> {
    if actual != expected {
        bail!("{label} changed since it was loaded");
    }
    Ok(())
}

fn ensure_state(actual: &str, expected: &str, message: &str) -> Result<()> {
    if actual != expected {
        bail!("{message}");
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

fn database_error(error: sqlx::Error, context: &'static str) -> anyhow::Error {
    if let sqlx::Error::Database(database) = &error
        && database.code().as_deref() == Some("23505")
    {
        return anyhow!("A requisition with that identity already exists");
    }
    anyhow::Error::new(error).context(context)
}

#[cfg(test)]
mod tests {
    use super::{LineValues, RequisitionLineInput, checked_total, validate_status};

    fn line(quantity: i32, unit_amount: i64) -> RequisitionLineInput {
        RequisitionLineInput {
            description: "Exercise books".to_string(),
            quantity,
            unit_label: Some("each".to_string()),
            estimated_unit_amount_minor: unit_amount,
            preferred_supplier_id: None,
        }
    }

    #[test]
    fn line_totals_use_checked_minor_unit_arithmetic() {
        let value = LineValues::parse(&line(5, 125)).unwrap();
        assert_eq!(value.estimated_line_amount_minor, 625);
        assert_eq!(checked_total(&[value]).unwrap(), 625);
        assert!(LineValues::parse(&line(i32::MAX, i64::MAX)).is_err());
    }

    #[test]
    fn requisition_status_filters_are_closed_sets() {
        assert!(validate_status(Some("submitted")).is_ok());
        assert!(validate_status(Some("paid")).is_err());
    }
}
