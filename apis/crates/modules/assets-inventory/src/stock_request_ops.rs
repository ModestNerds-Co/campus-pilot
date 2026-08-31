//! Assets-owned department stock-request workflow and atomic fulfilment.
//!
//! HR supplies only current employee/department references. Approval records
//! quantities without reserving stock; fulfilment verifies live balances and
//! posts the linked immutable issue inside the same database transaction.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, anyhow, bail};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_hr_payroll::{models::StockRequestEmployeeReference, ops::StockRequestReferenceOps};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::stock_dtos::{IssueStockRequest, StockQuantityLineInput};
use crate::stock_ops::StockMovementOps;
use crate::stock_request_dtos::{
    ApproveStockRequest, CloseStockRequest, CreateStockRequest, FulfilStockRequest,
    FulfilStockRequestResponse, PaginatedStockRequestsResponse, StockRequestBalancePreview,
    StockRequestDepartmentResponse, StockRequestDepartmentsResponse, StockRequestEventResponse,
    StockRequestFulfilmentLineResponse, StockRequestFulfilmentPreviewResponse,
    StockRequestFulfilmentResponse, StockRequestLineInput, StockRequestLineResponse,
    StockRequestReasonCommand, StockRequestResponse, StockRequestSummaryResponse,
    StockRequestVersionCommand, StockRequesterCandidateResponse, StockRequesterCandidatesResponse,
    UpdateStockRequest,
};
use crate::stock_request_models::{
    FulfilmentLineState, StockRequestEventRecord, StockRequestFulfilmentLineRecord,
    StockRequestFulfilmentRecord, StockRequestLineRecord, StockRequestRecord,
    StockRequestSummaryRecord,
};

const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
const MAX_REQUEST_LINES: usize = 200;
const MAX_SEARCH_LENGTH: usize = 200;

/// Minimum-field candidate reads for the department request workflow.
pub struct StockRequestCandidateOps;

impl StockRequestCandidateOps {
    pub async fn requesters(
        pool: &PgPool,
        tenant_id: Uuid,
        search: Option<&str>,
        department_id: Option<Uuid>,
    ) -> Result<StockRequesterCandidatesResponse> {
        let search = normalized_search(search)?;
        let employees = StockRequestReferenceOps::requester_candidates(
            pool,
            tenant_id,
            search.as_deref(),
            department_id,
            100,
        )
        .await?;
        Ok(StockRequesterCandidatesResponse {
            employees: employees
                .into_iter()
                .map(StockRequesterCandidateResponse::from)
                .collect(),
        })
    }

    pub async fn departments(
        pool: &PgPool,
        tenant_id: Uuid,
        search: Option<&str>,
    ) -> Result<StockRequestDepartmentsResponse> {
        let search = normalized_search(search)?;
        let departments = StockRequestReferenceOps::department_candidates(
            pool,
            tenant_id,
            search.as_deref(),
            100,
        )
        .await?;
        Ok(StockRequestDepartmentsResponse {
            departments: departments
                .into_iter()
                .map(StockRequestDepartmentResponse::from)
                .collect(),
        })
    }
}

/// Read and lifecycle operations for Assets-owned stock requests.
pub struct StockRequestOps;

impl StockRequestOps {
    #[allow(
        clippy::too_many_arguments,
        reason = "mirrors the bounded request worklist filters"
    )]
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
        requester_employee_id: Option<Uuid>,
        department_id: Option<Uuid>,
    ) -> Result<(PaginatedStockRequestsResponse, i64)> {
        let (page, per_page) = bounded_page(page, per_page);
        let search = search_pattern(search)?;
        let status = parse_status_filter(status)?;
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, StockRequestSummaryRecord>(&format!(
            "{} ORDER BY request.created_at DESC LIMIT $7 OFFSET $8",
            summary_query()
        ))
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(requester_employee_id)
        .bind(department_id)
        .bind(Option::<Uuid>::None)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list stock requests")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
              FROM assets_inventory_stock_requests AS request
             WHERE request.tenant_id = $1 AND request.deleted_at IS NULL
               AND ($2::TEXT IS NULL OR request.request_number ILIKE $2
                    OR request.purpose ILIKE $2)
               AND ($3::TEXT IS NULL OR request.status = $3)
               AND ($4::UUID IS NULL OR request.requester_employee_id = $4)
               AND ($5::UUID IS NULL OR request.department_id = $5)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(requester_employee_id)
        .bind(department_id)
        .fetch_one(pool)
        .await
        .context("Failed to count stock requests")?;
        let requests = hydrate_summaries(pool, tenant_id, rows).await?;
        Ok((PaginatedStockRequestsResponse { requests }, total))
    }

    pub async fn get(
        pool: &PgPool,
        tenant_id: Uuid,
        request_id: Uuid,
    ) -> Result<Option<StockRequestResponse>> {
        load_response(pool, tenant_id, request_id).await
    }

    pub async fn fulfilment_preview(
        pool: &PgPool,
        tenant_id: Uuid,
        request_id: Uuid,
    ) -> Result<Option<StockRequestFulfilmentPreviewResponse>> {
        let Some(request) = load_response(pool, tenant_id, request_id).await? else {
            return Ok(None);
        };
        if !matches!(
            request.summary.status.as_str(),
            "approved" | "partially_fulfilled"
        ) {
            bail!("Only approved or partially fulfilled stock requests can be fulfilled");
        }
        let item_ids = request
            .lines
            .iter()
            .filter(|line| line.remaining_quantity_minor > 0)
            .map(|line| line.item_id)
            .collect::<Vec<_>>();
        let balances = if item_ids.is_empty() {
            Vec::new()
        } else {
            sqlx::query_as::<_, StockRequestBalancePreview>(
                r#"
                SELECT balance.item_id, balance.store_id, store.store_number,
                       store.name AS store_name, balance.on_hand_minor,
                       balance.quantity_scale, balance.unit_label, balance.version
                  FROM assets_inventory_stock_balances AS balance
                  JOIN assets_inventory_stores AS store
                    ON store.id = balance.store_id AND store.tenant_id = balance.tenant_id
                 WHERE balance.tenant_id = $1 AND balance.item_id = ANY($2)
                   AND balance.deleted_at IS NULL AND balance.on_hand_minor > 0
                   AND store.status = 'active' AND store.deleted_at IS NULL
                 ORDER BY balance.item_id, store.name, store.store_number
                "#,
            )
            .bind(tenant_id)
            .bind(&item_ids)
            .fetch_all(pool)
            .await
            .context("Failed to load stock request fulfilment balances")?
        };
        Ok(Some(StockRequestFulfilmentPreviewResponse {
            request,
            balances,
        }))
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateStockRequest,
    ) -> Result<StockRequestResponse> {
        let actor_id = actor_user_id(actor)?;
        let values = StockRequestValues::parse(
            request.requester_employee_id,
            request.department_id,
            &request.purpose,
            request.needed_by,
            &request.lines,
        )?;
        let idempotency_key = required(&request.idempotency_key, 200, "Idempotency key")?;
        let fingerprint = request_fingerprint("create", None, &values)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start stock request creation")?;
        if let Some((existing_id, stored_fingerprint, deleted_at)) =
            replay_created_request(&mut transaction, tenant_id, &idempotency_key).await?
        {
            if stored_fingerprint != fingerprint {
                bail!("Idempotency key already belongs to another stock request");
            }
            if deleted_at.is_some() {
                bail!("Idempotent stock request has been removed");
            }
            transaction
                .rollback()
                .await
                .context("Failed to close replayed stock request creation")?;
            return load_response(pool, tenant_id, existing_id)
                .await?
                .ok_or_else(|| anyhow!("Idempotent stock request could not be loaded"));
        }
        StockRequestReferenceOps::lock_active_requester_department(
            &mut transaction,
            tenant_id,
            values.requester_employee_id,
            values.department_id,
        )
        .await?;
        let request_number = next_request_number(&mut transaction, tenant_id).await?;
        let request_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO assets_inventory_stock_requests (
                id, tenant_id, request_number, requester_employee_id, department_id,
                purpose, needed_by, status, version, idempotency_key,
                create_request_fingerprint, created_by
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, 'draft', 1, $8, $9, $10)
            "#,
        )
        .bind(request_id)
        .bind(tenant_id)
        .bind(&request_number)
        .bind(values.requester_employee_id)
        .bind(values.department_id)
        .bind(&values.purpose)
        .bind(values.needed_by)
        .bind(&idempotency_key)
        .bind(&fingerprint)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to create stock request")?;
        replace_draft_lines(&mut transaction, tenant_id, request_id, &values.lines).await?;
        append_request_event(
            &mut transaction,
            NewRequestEvent {
                tenant_id,
                request_id,
                event_type: "created",
                from_status: None,
                to_status: "draft",
                request_version: 1,
                actor_id,
                note: None,
                idempotency_key: None,
                fingerprint: None,
            },
        )
        .await?;
        append_request_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "create",
            request_id,
            &request_number,
            "draft",
            values.lines.len(),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit stock request creation")?;
        load_response(pool, tenant_id, request_id)
            .await?
            .ok_or_else(|| anyhow!("Created stock request could not be loaded"))
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        request_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateStockRequest,
    ) -> Result<Option<StockRequestResponse>> {
        let actor_id = actor_user_id(actor)?;
        let values = StockRequestValues::parse(
            request.requester_employee_id,
            request.department_id,
            &request.purpose,
            request.needed_by,
            &request.lines,
        )?;
        let idempotency_key = required(&request.idempotency_key, 200, "Idempotency key")?;
        let fingerprint = request_fingerprint(
            "update",
            Some(request_id),
            &json!({ "expected_version": request.expected_version, "values": &values }),
        )?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start stock request update")?;
        if replay_event(
            &mut transaction,
            tenant_id,
            request_id,
            &idempotency_key,
            &fingerprint,
        )
        .await?
        {
            transaction.rollback().await.ok();
            return load_response(pool, tenant_id, request_id).await;
        }
        let Some(current) = lock_request(&mut transaction, tenant_id, request_id).await? else {
            return Ok(None);
        };
        ensure_expected_version(&current, request.expected_version)?;
        ensure_status(&current, &["draft"])?;
        let requester = StockRequestReferenceOps::lock_requester_identity(
            &mut transaction,
            tenant_id,
            current.requester_employee_id,
        )
        .await?;
        ensure_requester_actor(&current, &requester, actor_id)?;
        StockRequestReferenceOps::lock_active_requester_department(
            &mut transaction,
            tenant_id,
            values.requester_employee_id,
            values.department_id,
        )
        .await?;
        let next_version = current.version + 1;
        sqlx::query(
            r#"
            UPDATE assets_inventory_stock_requests
               SET requester_employee_id = $3, department_id = $4, purpose = $5,
                   needed_by = $6, version = $7
             WHERE tenant_id = $1 AND id = $2 AND status = 'draft'
               AND version = $8 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(request_id)
        .bind(values.requester_employee_id)
        .bind(values.department_id)
        .bind(&values.purpose)
        .bind(values.needed_by)
        .bind(next_version)
        .bind(request.expected_version)
        .execute(&mut *transaction)
        .await
        .context("Failed to update stock request")?;
        soft_delete_draft_lines(&mut transaction, tenant_id, request_id).await?;
        replace_draft_lines(&mut transaction, tenant_id, request_id, &values.lines).await?;
        append_transition_event(
            &mut transaction,
            tenant_id,
            request_id,
            "updated",
            "draft",
            "draft",
            next_version,
            actor_id,
            None,
            &idempotency_key,
            &fingerprint,
        )
        .await?;
        append_request_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "update",
            request_id,
            &current.request_number,
            "draft",
            values.lines.len(),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit stock request update")?;
        load_response(pool, tenant_id, request_id).await
    }

    pub async fn delete(
        pool: &PgPool,
        tenant_id: Uuid,
        request_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        command: &StockRequestVersionCommand,
    ) -> Result<bool> {
        let actor_id = actor_user_id(actor)?;
        let idempotency_key = required(&command.idempotency_key, 200, "Idempotency key")?;
        let fingerprint = request_fingerprint("delete", Some(request_id), command)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start stock request removal")?;
        if replay_event(
            &mut transaction,
            tenant_id,
            request_id,
            &idempotency_key,
            &fingerprint,
        )
        .await?
        {
            transaction.rollback().await.ok();
            return Ok(true);
        }
        let Some(current) = lock_request(&mut transaction, tenant_id, request_id).await? else {
            return Ok(false);
        };
        ensure_expected_version(&current, command.expected_version)?;
        ensure_status(&current, &["draft"])?;
        let requester = StockRequestReferenceOps::lock_requester_identity(
            &mut transaction,
            tenant_id,
            current.requester_employee_id,
        )
        .await?;
        ensure_requester_actor(&current, &requester, actor_id)?;
        soft_delete_draft_lines(&mut transaction, tenant_id, request_id).await?;
        let next_version = current.version + 1;
        sqlx::query(
            r#"
            UPDATE assets_inventory_stock_requests
               SET deleted_at = NOW(), version = $3
             WHERE tenant_id = $1 AND id = $2 AND status = 'draft'
               AND version = $4 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(request_id)
        .bind(next_version)
        .bind(command.expected_version)
        .execute(&mut *transaction)
        .await
        .context("Failed to remove stock request")?;
        append_transition_event(
            &mut transaction,
            tenant_id,
            request_id,
            "deleted",
            "draft",
            "draft",
            next_version,
            actor_id,
            None,
            &idempotency_key,
            &fingerprint,
        )
        .await?;
        append_request_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "delete",
            request_id,
            &current.request_number,
            "draft",
            0,
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit stock request removal")?;
        Ok(true)
    }
}

impl StockRequestOps {
    pub async fn submit(
        pool: &PgPool,
        tenant_id: Uuid,
        request_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        command: &StockRequestVersionCommand,
    ) -> Result<Option<StockRequestResponse>> {
        let actor_id = actor_user_id(actor)?;
        let idempotency_key = required(&command.idempotency_key, 200, "Idempotency key")?;
        let fingerprint = request_fingerprint("submit", Some(request_id), command)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start stock request submission")?;
        if replay_event(
            &mut transaction,
            tenant_id,
            request_id,
            &idempotency_key,
            &fingerprint,
        )
        .await?
        {
            transaction.rollback().await.ok();
            return load_response(pool, tenant_id, request_id).await;
        }
        let Some(current) = lock_request(&mut transaction, tenant_id, request_id).await? else {
            return Ok(None);
        };
        ensure_expected_version(&current, command.expected_version)?;
        ensure_status(&current, &["draft"])?;
        let requester = StockRequestReferenceOps::lock_requester_identity(
            &mut transaction,
            tenant_id,
            current.requester_employee_id,
        )
        .await?;
        ensure_requester_actor(&current, &requester, actor_id)?;
        StockRequestReferenceOps::lock_active_requester_department(
            &mut transaction,
            tenant_id,
            current.requester_employee_id,
            current.department_id,
        )
        .await?;
        let next_version = current.version + 1;
        sqlx::query(
            r#"
            UPDATE assets_inventory_stock_requests
               SET status = 'submitted', submitted_by = $3, submitted_at = NOW(), version = $4
             WHERE tenant_id = $1 AND id = $2 AND status = 'draft'
               AND version = $5 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(request_id)
        .bind(actor_id)
        .bind(next_version)
        .bind(command.expected_version)
        .execute(&mut *transaction)
        .await
        .context("Failed to submit stock request")?;
        append_transition_event(
            &mut transaction,
            tenant_id,
            request_id,
            "submitted",
            "draft",
            "submitted",
            next_version,
            actor_id,
            None,
            &idempotency_key,
            &fingerprint,
        )
        .await?;
        let line_count = request_line_count(&mut transaction, tenant_id, request_id).await?;
        append_request_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "submit",
            request_id,
            &current.request_number,
            "submitted",
            line_count,
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit stock request submission")?;
        load_response(pool, tenant_id, request_id).await
    }

    pub async fn approve(
        pool: &PgPool,
        tenant_id: Uuid,
        request_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        command: &ApproveStockRequest,
    ) -> Result<Option<StockRequestResponse>> {
        let actor_id = actor_user_id(actor)?;
        let note = optional(command.note.as_deref(), 1000, "Approval note")?;
        let idempotency_key = required(&command.idempotency_key, 200, "Idempotency key")?;
        let fingerprint = request_fingerprint("approve", Some(request_id), command)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start stock request approval")?;
        if replay_event(
            &mut transaction,
            tenant_id,
            request_id,
            &idempotency_key,
            &fingerprint,
        )
        .await?
        {
            transaction.rollback().await.ok();
            return load_response(pool, tenant_id, request_id).await;
        }
        let Some(current) = lock_request(&mut transaction, tenant_id, request_id).await? else {
            return Ok(None);
        };
        ensure_expected_version(&current, command.expected_version)?;
        ensure_status(&current, &["submitted"])?;
        let requester = StockRequestReferenceOps::lock_requester_identity(
            &mut transaction,
            tenant_id,
            current.requester_employee_id,
        )
        .await?;
        ensure_approver_actor(&current, &requester, actor_id)?;
        let approvals =
            validate_approval_lines(&mut transaction, tenant_id, request_id, &command.lines)
                .await?;
        let next_version = current.version + 1;
        sqlx::query(
            r#"
            UPDATE assets_inventory_stock_requests
               SET status = 'approved', decided_by = $3, decided_at = NOW(),
                   decision_note = $4, version = $5
             WHERE tenant_id = $1 AND id = $2 AND status = 'submitted'
               AND version = $6 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(request_id)
        .bind(actor_id)
        .bind(&note)
        .bind(next_version)
        .bind(command.expected_version)
        .execute(&mut *transaction)
        .await
        .context("Failed to approve stock request")?;
        for (line_id, quantity) in approvals {
            sqlx::query(
                r#"
                UPDATE assets_inventory_stock_request_lines
                   SET approved_quantity_minor = $4
                 WHERE tenant_id = $1 AND request_id = $2 AND id = $3
                   AND deleted_at IS NULL AND approved_quantity_minor IS NULL
                "#,
            )
            .bind(tenant_id)
            .bind(request_id)
            .bind(line_id)
            .bind(quantity)
            .execute(&mut *transaction)
            .await
            .context("Failed to record stock request approval quantity")?;
        }
        append_transition_event(
            &mut transaction,
            tenant_id,
            request_id,
            "approved",
            "submitted",
            "approved",
            next_version,
            actor_id,
            note.as_deref(),
            &idempotency_key,
            &fingerprint,
        )
        .await?;
        let line_count = request_line_count(&mut transaction, tenant_id, request_id).await?;
        append_request_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "approve",
            request_id,
            &current.request_number,
            "approved",
            line_count,
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit stock request approval")?;
        load_response(pool, tenant_id, request_id).await
    }

    pub async fn reject(
        pool: &PgPool,
        tenant_id: Uuid,
        request_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        command: &StockRequestReasonCommand,
    ) -> Result<Option<StockRequestResponse>> {
        let actor_id = actor_user_id(actor)?;
        let reason = required(&command.reason, 1000, "Rejection reason")?;
        let idempotency_key = required(&command.idempotency_key, 200, "Idempotency key")?;
        let fingerprint = request_fingerprint("reject", Some(request_id), command)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start stock request rejection")?;
        if replay_event(
            &mut transaction,
            tenant_id,
            request_id,
            &idempotency_key,
            &fingerprint,
        )
        .await?
        {
            transaction.rollback().await.ok();
            return load_response(pool, tenant_id, request_id).await;
        }
        let Some(current) = lock_request(&mut transaction, tenant_id, request_id).await? else {
            return Ok(None);
        };
        ensure_expected_version(&current, command.expected_version)?;
        ensure_status(&current, &["submitted"])?;
        let requester = StockRequestReferenceOps::lock_requester_identity(
            &mut transaction,
            tenant_id,
            current.requester_employee_id,
        )
        .await?;
        ensure_approver_actor(&current, &requester, actor_id)?;
        let next_version = current.version + 1;
        sqlx::query(
            r#"
            UPDATE assets_inventory_stock_requests
               SET status = 'rejected', decided_by = $3, decided_at = NOW(),
                   decision_note = $4, version = $5
             WHERE tenant_id = $1 AND id = $2 AND status = 'submitted'
               AND version = $6 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(request_id)
        .bind(actor_id)
        .bind(&reason)
        .bind(next_version)
        .bind(command.expected_version)
        .execute(&mut *transaction)
        .await
        .context("Failed to reject stock request")?;
        sqlx::query(
            r#"
            UPDATE assets_inventory_stock_request_lines
               SET approved_quantity_minor = 0
             WHERE tenant_id = $1 AND request_id = $2 AND deleted_at IS NULL
               AND approved_quantity_minor IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(request_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to close rejected request quantities")?;
        append_transition_event(
            &mut transaction,
            tenant_id,
            request_id,
            "rejected",
            "submitted",
            "rejected",
            next_version,
            actor_id,
            Some(&reason),
            &idempotency_key,
            &fingerprint,
        )
        .await?;
        let line_count = request_line_count(&mut transaction, tenant_id, request_id).await?;
        append_request_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "reject",
            request_id,
            &current.request_number,
            "rejected",
            line_count,
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit stock request rejection")?;
        load_response(pool, tenant_id, request_id).await
    }

    pub async fn cancel(
        pool: &PgPool,
        tenant_id: Uuid,
        request_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        command: &StockRequestReasonCommand,
    ) -> Result<Option<StockRequestResponse>> {
        let actor_id = actor_user_id(actor)?;
        let reason = required(&command.reason, 1000, "Cancellation reason")?;
        let idempotency_key = required(&command.idempotency_key, 200, "Idempotency key")?;
        let fingerprint = request_fingerprint("cancel", Some(request_id), command)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start stock request cancellation")?;
        if replay_event(
            &mut transaction,
            tenant_id,
            request_id,
            &idempotency_key,
            &fingerprint,
        )
        .await?
        {
            transaction.rollback().await.ok();
            return load_response(pool, tenant_id, request_id).await;
        }
        let Some(current) = lock_request(&mut transaction, tenant_id, request_id).await? else {
            return Ok(None);
        };
        ensure_expected_version(&current, command.expected_version)?;
        ensure_status(&current, &["submitted", "approved"])?;
        let requester = StockRequestReferenceOps::lock_requester_identity(
            &mut transaction,
            tenant_id,
            current.requester_employee_id,
        )
        .await?;
        ensure_requester_actor(&current, &requester, actor_id)?;
        if current.status == "approved"
            && request_has_fulfilments(&mut transaction, tenant_id, request_id).await?
        {
            bail!("Approved stock requests cannot be cancelled after an issue");
        }
        let next_version = current.version + 1;
        sqlx::query(
            r#"
            UPDATE assets_inventory_stock_requests
               SET status = 'cancelled', cancelled_by = $3, cancelled_at = NOW(),
                   cancellation_note = $4, version = $5
             WHERE tenant_id = $1 AND id = $2 AND status = $6
               AND version = $7 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(request_id)
        .bind(actor_id)
        .bind(&reason)
        .bind(next_version)
        .bind(&current.status)
        .bind(command.expected_version)
        .execute(&mut *transaction)
        .await
        .context("Failed to cancel stock request")?;
        append_transition_event(
            &mut transaction,
            tenant_id,
            request_id,
            "cancelled",
            &current.status,
            "cancelled",
            next_version,
            actor_id,
            Some(&reason),
            &idempotency_key,
            &fingerprint,
        )
        .await?;
        let line_count = request_line_count(&mut transaction, tenant_id, request_id).await?;
        append_request_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "cancel",
            request_id,
            &current.request_number,
            "cancelled",
            line_count,
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit stock request cancellation")?;
        load_response(pool, tenant_id, request_id).await
    }

    pub async fn close(
        pool: &PgPool,
        tenant_id: Uuid,
        request_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        command: &CloseStockRequest,
    ) -> Result<Option<StockRequestResponse>> {
        let actor_id = actor_user_id(actor)?;
        let note = optional(command.note.as_deref(), 1000, "Closure note")?;
        let idempotency_key = required(&command.idempotency_key, 200, "Idempotency key")?;
        let fingerprint = request_fingerprint("close", Some(request_id), command)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start stock request closure")?;
        if replay_event(
            &mut transaction,
            tenant_id,
            request_id,
            &idempotency_key,
            &fingerprint,
        )
        .await?
        {
            transaction.rollback().await.ok();
            return load_response(pool, tenant_id, request_id).await;
        }
        let Some(current) = lock_request(&mut transaction, tenant_id, request_id).await? else {
            return Ok(None);
        };
        ensure_expected_version(&current, command.expected_version)?;
        ensure_status(&current, &["partially_fulfilled"])?;
        let requester = StockRequestReferenceOps::lock_requester_identity(
            &mut transaction,
            tenant_id,
            current.requester_employee_id,
        )
        .await?;
        ensure_approver_actor(&current, &requester, actor_id)?;
        let next_version = current.version + 1;
        sqlx::query(
            r#"
            UPDATE assets_inventory_stock_requests
               SET status = 'closed', closed_by = $3, closed_at = NOW(),
                   closure_note = $4, version = $5
             WHERE tenant_id = $1 AND id = $2 AND status = 'partially_fulfilled'
               AND version = $6 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(request_id)
        .bind(actor_id)
        .bind(&note)
        .bind(next_version)
        .bind(command.expected_version)
        .execute(&mut *transaction)
        .await
        .context("Failed to close stock request")?;
        append_transition_event(
            &mut transaction,
            tenant_id,
            request_id,
            "closed",
            "partially_fulfilled",
            "closed",
            next_version,
            actor_id,
            note.as_deref(),
            &idempotency_key,
            &fingerprint,
        )
        .await?;
        let line_count = request_line_count(&mut transaction, tenant_id, request_id).await?;
        append_request_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "close",
            request_id,
            &current.request_number,
            "closed",
            line_count,
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit stock request closure")?;
        load_response(pool, tenant_id, request_id).await
    }

    pub async fn fulfil(
        pool: &PgPool,
        tenant_id: Uuid,
        request_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        command: &FulfilStockRequest,
    ) -> Result<Option<FulfilStockRequestResponse>> {
        let actor_id = actor_user_id(actor)?;
        let reason = optional(command.reason.as_deref(), 2000, "Issue reason")?;
        let idempotency_key = required(&command.idempotency_key, 200, "Idempotency key")?;
        let fingerprint = request_fingerprint("fulfil", Some(request_id), command)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start stock request fulfilment")?;
        if let Some((existing_request_id, movement_id, movement_number, stored_fingerprint)) =
            replay_fulfilment(&mut transaction, tenant_id, &idempotency_key).await?
        {
            if existing_request_id != request_id || stored_fingerprint != fingerprint {
                bail!("Idempotency key already belongs to another stock request fulfilment");
            }
            transaction.rollback().await.ok();
            let request = load_response(pool, tenant_id, request_id)
                .await?
                .ok_or_else(|| anyhow!("Idempotent stock request could not be loaded"))?;
            return Ok(Some(FulfilStockRequestResponse {
                request,
                movement_id,
                movement_number,
            }));
        }
        let Some(current) = lock_request(&mut transaction, tenant_id, request_id).await? else {
            return Ok(None);
        };
        ensure_expected_version(&current, command.expected_request_version)?;
        ensure_status(&current, &["approved", "partially_fulfilled"])?;
        let requester = StockRequestReferenceOps::lock_requester_identity(
            &mut transaction,
            tenant_id,
            current.requester_employee_id,
        )
        .await?;
        ensure_issuer_actor(&current, &requester, actor_id)?;
        let issue_lines =
            validate_fulfilment_lines(&mut transaction, tenant_id, request_id, &command.lines)
                .await?;
        let movement_request = IssueStockRequest {
            effective_on: command.effective_on,
            reference: Some(current.request_number.clone()),
            reason,
            idempotency_key: movement_idempotency_key(request_id, &idempotency_key),
            lines: issue_lines
                .iter()
                .map(|line| StockQuantityLineInput {
                    item_id: line.item_id,
                    store_id: line.store_id,
                    quantity_minor: line.quantity_minor,
                })
                .collect(),
        };
        let movement = StockMovementOps::issue_in_transaction(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            &movement_request,
        )
        .await?;
        let movement_lines = movement
            .lines
            .iter()
            .map(|line| ((line.item_id, line.store_id), line))
            .collect::<BTreeMap<_, _>>();
        if movement_lines.len() != issue_lines.len() {
            bail!("Stock issue lines do not match the request fulfilment");
        }
        let next_version = current.version + 1;
        let fulfilment_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO assets_inventory_stock_request_fulfilments (
                id, tenant_id, request_id, movement_id, request_version, issued_by,
                idempotency_key, create_request_fingerprint
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(fulfilment_id)
        .bind(tenant_id)
        .bind(request_id)
        .bind(movement.summary.id)
        .bind(next_version)
        .bind(actor_id)
        .bind(&idempotency_key)
        .bind(&fingerprint)
        .execute(&mut *transaction)
        .await
        .context("Failed to link stock request fulfilment")?;
        for line in &issue_lines {
            let movement_line = movement_lines
                .get(&(line.item_id, line.store_id))
                .ok_or_else(|| anyhow!("Stock issue line could not be linked to the request"))?;
            if movement_line.quantity_delta_minor != -line.quantity_minor {
                bail!("Stock issue quantity does not match the request fulfilment");
            }
            sqlx::query(
                r#"
                INSERT INTO assets_inventory_stock_request_fulfilment_lines (
                    id, tenant_id, fulfilment_id, request_line_id, movement_line_id,
                    quantity_minor
                ) VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(tenant_id)
            .bind(fulfilment_id)
            .bind(line.request_line_id)
            .bind(movement_line.id)
            .bind(line.quantity_minor)
            .execute(&mut *transaction)
            .await
            .context("Failed to link stock request fulfilment line")?;
        }
        let fulfilled = request_is_fully_issued(&mut transaction, tenant_id, request_id).await?;
        let next_status = if fulfilled {
            "fulfilled"
        } else {
            "partially_fulfilled"
        };
        sqlx::query(
            r#"
            UPDATE assets_inventory_stock_requests
               SET status = $3, version = $4
             WHERE tenant_id = $1 AND id = $2 AND status = $5
               AND version = $6 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(request_id)
        .bind(next_status)
        .bind(next_version)
        .bind(&current.status)
        .bind(command.expected_request_version)
        .execute(&mut *transaction)
        .await
        .context("Failed to advance fulfilled stock request")?;
        append_request_event(
            &mut transaction,
            NewRequestEvent {
                tenant_id,
                request_id,
                event_type: next_status,
                from_status: Some(&current.status),
                to_status: next_status,
                request_version: next_version,
                actor_id,
                note: None,
                idempotency_key: None,
                fingerprint: None,
            },
        )
        .await?;
        let line_count = request_line_count(&mut transaction, tenant_id, request_id).await?;
        append_request_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "fulfil",
            request_id,
            &current.request_number,
            next_status,
            line_count,
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit stock request fulfilment")?;
        let request = load_response(pool, tenant_id, request_id)
            .await?
            .ok_or_else(|| anyhow!("Fulfilled stock request could not be loaded"))?;
        Ok(Some(FulfilStockRequestResponse {
            request,
            movement_id: movement.summary.id,
            movement_number: movement.summary.movement_number,
        }))
    }
}

#[derive(Debug, Clone)]
struct PreparedFulfilmentLine {
    request_line_id: Uuid,
    item_id: Uuid,
    store_id: Uuid,
    quantity_minor: i64,
}

#[derive(Debug, Serialize)]
struct StockRequestValues {
    requester_employee_id: Uuid,
    department_id: Uuid,
    purpose: String,
    needed_by: Option<chrono::NaiveDate>,
    lines: Vec<StockRequestLineInput>,
}

impl StockRequestValues {
    fn parse(
        requester_employee_id: Uuid,
        department_id: Uuid,
        purpose: &str,
        needed_by: Option<chrono::NaiveDate>,
        lines: &[StockRequestLineInput],
    ) -> Result<Self> {
        if !(1..=MAX_REQUEST_LINES).contains(&lines.len()) {
            bail!("Stock request requires between one and {MAX_REQUEST_LINES} lines");
        }
        let mut item_ids = BTreeSet::new();
        let mut total = 0i64;
        for line in lines {
            if !item_ids.insert(line.item_id) {
                bail!("Stock request items must be unique");
            }
            ensure_positive_quantity(line.requested_quantity_minor)?;
            total = total
                .checked_add(line.requested_quantity_minor)
                .filter(|value| *value <= MAX_SAFE_INTEGER)
                .ok_or_else(|| anyhow!("Stock request total quantity is too large"))?;
        }
        Ok(Self {
            requester_employee_id,
            department_id,
            purpose: required(purpose, 2000, "Stock request purpose")?,
            needed_by,
            lines: lines.to_vec(),
        })
    }
}

fn bounded_page(page: i64, per_page: i64) -> (i64, i64) {
    (page.clamp(1, 1_000_000), per_page.clamp(1, 100))
}

fn parse_status_filter(value: Option<&str>) -> Result<Option<&str>> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        None => Ok(None),
        Some(
            value @ ("draft"
            | "submitted"
            | "approved"
            | "rejected"
            | "cancelled"
            | "partially_fulfilled"
            | "fulfilled"
            | "closed"),
        ) => Ok(Some(value)),
        Some(_) => bail!("Stock request status filter is invalid"),
    }
}

fn normalized_search(value: Option<&str>) -> Result<Option<String>> {
    let value = value.map(str::trim).filter(|value| !value.is_empty());
    if value.is_some_and(|value| value.chars().count() > MAX_SEARCH_LENGTH) {
        bail!("Stock request search is too long");
    }
    Ok(value.map(str::to_string))
}

fn search_pattern(value: Option<&str>) -> Result<Option<String>> {
    Ok(normalized_search(value)?.map(|value| format!("%{value}%")))
}

fn required(value: &str, max: usize, label: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{label} is required");
    }
    if value.chars().count() > max {
        bail!("{label} is too long");
    }
    Ok(value.to_string())
}

fn optional(value: Option<&str>, max: usize, label: &str) -> Result<Option<String>> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            if value.chars().count() > max {
                bail!("{label} is too long");
            }
            Ok(value.to_string())
        })
        .transpose()
}

fn ensure_positive_quantity(value: i64) -> Result<()> {
    if !(1..=MAX_SAFE_INTEGER).contains(&value) {
        bail!("Stock request quantity must be positive and exactly representable");
    }
    Ok(())
}

fn actor_user_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("Authenticated person or Agent actor is required"))
}

fn summary_query() -> &'static str {
    r#"
    SELECT request.id, request.request_number, request.requester_employee_id,
           request.department_id, request.needed_by, request.status, request.version,
           COALESCE(lines.line_count, 0)::BIGINT AS line_count,
           COALESCE(lines.requested_quantity_minor, 0)::BIGINT AS requested_quantity_minor,
           COALESCE(lines.approved_quantity_minor, 0)::BIGINT AS approved_quantity_minor,
           COALESCE(issued.issued_quantity_minor, 0)::BIGINT AS issued_quantity_minor,
           request.created_at, request.updated_at
      FROM assets_inventory_stock_requests AS request
      LEFT JOIN LATERAL (
          SELECT COUNT(*)::BIGINT AS line_count,
                 COALESCE(SUM(line.requested_quantity_minor), 0)::BIGINT AS requested_quantity_minor,
                 COALESCE(SUM(line.approved_quantity_minor), 0)::BIGINT AS approved_quantity_minor
            FROM assets_inventory_stock_request_lines AS line
           WHERE line.tenant_id = request.tenant_id AND line.request_id = request.id
             AND line.deleted_at IS NULL
      ) AS lines ON TRUE
      LEFT JOIN LATERAL (
          SELECT COALESCE(SUM(fulfilment_line.quantity_minor), 0)::BIGINT AS issued_quantity_minor
            FROM assets_inventory_stock_request_fulfilment_lines AS fulfilment_line
            JOIN assets_inventory_stock_request_fulfilments AS fulfilment
              ON fulfilment.id = fulfilment_line.fulfilment_id
             AND fulfilment.tenant_id = fulfilment_line.tenant_id
           WHERE fulfilment.tenant_id = request.tenant_id
             AND fulfilment.request_id = request.id
             AND fulfilment.deleted_at IS NULL AND fulfilment_line.deleted_at IS NULL
      ) AS issued ON TRUE
     WHERE request.tenant_id = $1 AND request.deleted_at IS NULL
       AND ($2::TEXT IS NULL OR request.request_number ILIKE $2 OR request.purpose ILIKE $2)
       AND ($3::TEXT IS NULL OR request.status = $3)
       AND ($4::UUID IS NULL OR request.requester_employee_id = $4)
       AND ($5::UUID IS NULL OR request.department_id = $5)
       AND ($6::UUID IS NULL OR request.id = $6)
    "#
}

async fn hydrate_summaries(
    pool: &PgPool,
    tenant_id: Uuid,
    rows: Vec<StockRequestSummaryRecord>,
) -> Result<Vec<StockRequestSummaryResponse>> {
    let employee_ids = rows
        .iter()
        .map(|row| row.requester_employee_id)
        .collect::<Vec<_>>();
    let department_ids = rows.iter().map(|row| row.department_id).collect::<Vec<_>>();
    let employees =
        StockRequestReferenceOps::employee_references_by_ids(pool, tenant_id, &employee_ids)
            .await?
            .into_iter()
            .map(|reference| (reference.id, reference))
            .collect::<BTreeMap<_, _>>();
    let departments =
        StockRequestReferenceOps::department_references_by_ids(pool, tenant_id, &department_ids)
            .await?
            .into_iter()
            .map(|reference| (reference.id, reference))
            .collect::<BTreeMap<_, _>>();
    Ok(rows
        .into_iter()
        .map(|row| {
            let employee = employees.get(&row.requester_employee_id);
            let department = departments.get(&row.department_id);
            StockRequestSummaryResponse {
                id: row.id,
                request_number: row.request_number,
                requester_employee_id: row.requester_employee_id,
                requester_employee_number: employee.map(|value| value.employee_number.clone()),
                requester_name: employee.map(|value| value.display_name.clone()),
                department_id: row.department_id,
                department_code: department.map(|value| value.code.clone()),
                department_name: department.map(|value| value.name.clone()),
                needed_by: row.needed_by,
                status: row.status,
                version: row.version,
                line_count: row.line_count,
                requested_quantity_minor: row.requested_quantity_minor,
                approved_quantity_minor: row.approved_quantity_minor,
                issued_quantity_minor: row.issued_quantity_minor,
                created_at: row.created_at,
                updated_at: row.updated_at,
            }
        })
        .collect())
}

async fn load_response(
    pool: &PgPool,
    tenant_id: Uuid,
    request_id: Uuid,
) -> Result<Option<StockRequestResponse>> {
    let record = sqlx::query_as::<_, StockRequestRecord>(
        r#"
        SELECT request_number, requester_employee_id, department_id, purpose,
               status, version, created_by, submitted_by, submitted_at,
               decided_by, decided_at, decision_note, cancelled_at, cancellation_note,
               closed_at, closure_note
          FROM assets_inventory_stock_requests
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(request_id)
    .fetch_optional(pool)
    .await
    .context("Failed to read stock request")?;
    let Some(record) = record else {
        return Ok(None);
    };
    let summaries = sqlx::query_as::<_, StockRequestSummaryRecord>(summary_query())
        .bind(tenant_id)
        .bind(Option::<String>::None)
        .bind(Option::<String>::None)
        .bind(Option::<Uuid>::None)
        .bind(Option::<Uuid>::None)
        .bind(Some(request_id))
        .fetch_all(pool)
        .await
        .context("Failed to read stock request summary")?;
    let summary = hydrate_summaries(pool, tenant_id, summaries)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("Stock request summary could not be loaded"))?;
    let line_rows = sqlx::query_as::<_, StockRequestLineRecord>(
        r#"
        SELECT line.id, line.line_number, line.item_id, item.item_number,
               item.name AS item_name, item.unit_label, item.quantity_scale,
               line.requested_quantity_minor, line.approved_quantity_minor,
               COALESCE(issued.issued_quantity_minor, 0)::BIGINT AS issued_quantity_minor
          FROM assets_inventory_stock_request_lines AS line
          JOIN assets_inventory_items AS item
            ON item.id = line.item_id AND item.tenant_id = line.tenant_id
          LEFT JOIN LATERAL (
              SELECT COALESCE(SUM(fulfilment_line.quantity_minor), 0)::BIGINT AS issued_quantity_minor
                FROM assets_inventory_stock_request_fulfilment_lines AS fulfilment_line
                JOIN assets_inventory_stock_request_fulfilments AS fulfilment
                  ON fulfilment.id = fulfilment_line.fulfilment_id
                 AND fulfilment.tenant_id = fulfilment_line.tenant_id
               WHERE fulfilment_line.tenant_id = line.tenant_id
                 AND fulfilment_line.request_line_id = line.id
                 AND fulfilment.deleted_at IS NULL AND fulfilment_line.deleted_at IS NULL
          ) AS issued ON TRUE
         WHERE line.tenant_id = $1 AND line.request_id = $2 AND line.deleted_at IS NULL
         ORDER BY line.line_number
        "#,
    )
    .bind(tenant_id).bind(request_id)
    .fetch_all(pool).await.context("Failed to read stock request lines")?;
    let lines = line_rows
        .into_iter()
        .map(|line| {
            let approved = line.approved_quantity_minor.unwrap_or(0);
            let remaining = approved
                .checked_sub(line.issued_quantity_minor)
                .filter(|value| *value >= 0)
                .ok_or_else(|| anyhow!("Stock request fulfilment quantities are inconsistent"))?;
            Ok(StockRequestLineResponse {
                id: line.id,
                line_number: line.line_number,
                item_id: line.item_id,
                item_number: line.item_number,
                item_name: line.item_name,
                unit_label: line.unit_label,
                quantity_scale: line.quantity_scale,
                requested_quantity_minor: line.requested_quantity_minor,
                approved_quantity_minor: line.approved_quantity_minor,
                issued_quantity_minor: line.issued_quantity_minor,
                remaining_quantity_minor: remaining,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let event_rows = sqlx::query_as::<_, StockRequestEventRecord>(
        r#"
        SELECT event_type, from_status, to_status, request_version, note, created_at
          FROM assets_inventory_stock_request_events
         WHERE tenant_id = $1 AND request_id = $2 AND deleted_at IS NULL
         ORDER BY created_at, id
        "#,
    )
    .bind(tenant_id)
    .bind(request_id)
    .fetch_all(pool)
    .await
    .context("Failed to read stock request events")?;
    let events = event_rows
        .into_iter()
        .map(|event| StockRequestEventResponse {
            event_type: event.event_type,
            from_status: event.from_status,
            to_status: event.to_status,
            request_version: event.request_version,
            note: event.note,
            created_at: event.created_at,
        })
        .collect();
    let fulfilment_rows = sqlx::query_as::<_, StockRequestFulfilmentRecord>(
        r#"
        SELECT fulfilment.id, fulfilment.movement_id, movement.movement_number,
               movement.effective_on, COUNT(line.id)::BIGINT AS line_count,
               COALESCE(SUM(line.quantity_minor), 0)::BIGINT AS quantity_minor,
               fulfilment.created_at
          FROM assets_inventory_stock_request_fulfilments AS fulfilment
          JOIN assets_inventory_stock_movements AS movement
            ON movement.id = fulfilment.movement_id AND movement.tenant_id = fulfilment.tenant_id
          LEFT JOIN assets_inventory_stock_request_fulfilment_lines AS line
            ON line.fulfilment_id = fulfilment.id AND line.tenant_id = fulfilment.tenant_id
           AND line.deleted_at IS NULL
         WHERE fulfilment.tenant_id = $1 AND fulfilment.request_id = $2
           AND fulfilment.deleted_at IS NULL
         GROUP BY fulfilment.id, movement.id
         ORDER BY fulfilment.created_at, fulfilment.id
        "#,
    )
    .bind(tenant_id)
    .bind(request_id)
    .fetch_all(pool)
    .await
    .context("Failed to read stock request fulfilments")?;
    let fulfilment_line_rows = sqlx::query_as::<_, StockRequestFulfilmentLineRecord>(
        r#"
        SELECT link.fulfilment_id, link.request_line_id, movement_line.item_id,
               movement_line.item_number, movement_line.item_name, movement_line.store_id,
               movement_line.store_number, movement_line.store_name, link.quantity_minor,
               movement_line.quantity_scale, movement_line.unit_label
          FROM assets_inventory_stock_request_fulfilment_lines AS link
          JOIN assets_inventory_stock_request_fulfilments AS fulfilment
            ON fulfilment.id = link.fulfilment_id AND fulfilment.tenant_id = link.tenant_id
          JOIN assets_inventory_stock_movement_lines AS movement_line
            ON movement_line.id = link.movement_line_id AND movement_line.tenant_id = link.tenant_id
         WHERE fulfilment.tenant_id = $1 AND fulfilment.request_id = $2
           AND fulfilment.deleted_at IS NULL AND link.deleted_at IS NULL
         ORDER BY fulfilment.created_at, movement_line.line_number
        "#,
    )
    .bind(tenant_id)
    .bind(request_id)
    .fetch_all(pool)
    .await
    .context("Failed to read stock request fulfilment lines")?;
    let mut fulfilment_lines = BTreeMap::<Uuid, Vec<StockRequestFulfilmentLineResponse>>::new();
    for line in fulfilment_line_rows {
        fulfilment_lines
            .entry(line.fulfilment_id)
            .or_default()
            .push(StockRequestFulfilmentLineResponse {
                request_line_id: line.request_line_id,
                item_id: line.item_id,
                item_number: line.item_number,
                item_name: line.item_name,
                store_id: line.store_id,
                store_number: line.store_number,
                store_name: line.store_name,
                quantity_minor: line.quantity_minor,
                quantity_scale: line.quantity_scale,
                unit_label: line.unit_label,
            });
    }
    let fulfilments = fulfilment_rows
        .into_iter()
        .map(|fulfilment| {
            let response_lines = fulfilment_lines.remove(&fulfilment.id).unwrap_or_default();
            debug_assert_eq!(fulfilment.line_count, response_lines.len() as i64);
            StockRequestFulfilmentResponse {
                id: fulfilment.id,
                movement_id: fulfilment.movement_id,
                movement_number: fulfilment.movement_number,
                effective_on: fulfilment.effective_on,
                quantity_minor: fulfilment.quantity_minor,
                created_at: fulfilment.created_at,
                lines: response_lines,
            }
        })
        .collect();
    Ok(Some(StockRequestResponse {
        summary,
        purpose: record.purpose,
        submitted_at: record.submitted_at,
        decided_at: record.decided_at,
        decision_note: record.decision_note,
        cancelled_at: record.cancelled_at,
        cancellation_note: record.cancellation_note,
        closed_at: record.closed_at,
        closure_note: record.closure_note,
        lines,
        events,
        fulfilments,
    }))
}

async fn next_request_number(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
) -> Result<String> {
    let number = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO assets_inventory_stock_request_sequences (tenant_id, last_number)
        VALUES ($1, 1)
        ON CONFLICT (tenant_id) DO UPDATE
            SET last_number = assets_inventory_stock_request_sequences.last_number + 1
        RETURNING last_number
        "#,
    )
    .bind(tenant_id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to allocate stock request number")?;
    if !(1..=999_999).contains(&number) {
        bail!("Stock request number sequence is exhausted");
    }
    Ok(format!("SRQ-{number:06}"))
}

async fn replay_created_request(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> Result<Option<(Uuid, String, Option<chrono::DateTime<chrono::Utc>>)>> {
    sqlx::query_as::<_, (Uuid, String, Option<chrono::DateTime<chrono::Utc>>)>(
        r#"
        SELECT id, create_request_fingerprint, deleted_at
          FROM assets_inventory_stock_requests
         WHERE tenant_id = $1 AND idempotency_key = $2
        "#,
    )
    .bind(tenant_id)
    .bind(idempotency_key)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to inspect stock request idempotency")
}

async fn replay_event(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    request_id: Uuid,
    idempotency_key: &str,
    fingerprint: &str,
) -> Result<bool> {
    let existing = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT request_id, request_fingerprint
          FROM assets_inventory_stock_request_events
         WHERE tenant_id = $1 AND idempotency_key = $2
        "#,
    )
    .bind(tenant_id)
    .bind(idempotency_key)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to inspect stock request event idempotency")?;
    let Some((existing_request_id, existing_fingerprint)) = existing else {
        return Ok(false);
    };
    if existing_request_id != request_id || existing_fingerprint != fingerprint {
        bail!("Idempotency key already belongs to another stock request operation");
    }
    Ok(true)
}

async fn replay_fulfilment(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> Result<Option<(Uuid, Uuid, String, String)>> {
    sqlx::query_as::<_, (Uuid, Uuid, String, String)>(
        r#"
        SELECT fulfilment.request_id, fulfilment.movement_id, movement.movement_number,
               fulfilment.create_request_fingerprint
          FROM assets_inventory_stock_request_fulfilments AS fulfilment
          JOIN assets_inventory_stock_movements AS movement
            ON movement.id = fulfilment.movement_id AND movement.tenant_id = fulfilment.tenant_id
         WHERE fulfilment.tenant_id = $1 AND fulfilment.idempotency_key = $2
        "#,
    )
    .bind(tenant_id)
    .bind(idempotency_key)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to inspect stock request fulfilment idempotency")
}

fn request_fingerprint<T: Serialize>(
    kind: &str,
    request_id: Option<Uuid>,
    value: &T,
) -> Result<String> {
    let canonical = serde_json::to_vec(&json!({
        "kind": kind,
        "request_id": request_id,
        "payload": value,
    }))
    .context("Failed to fingerprint stock request operation")?;
    Ok(format!("{:x}", Sha256::digest(canonical)))
}

fn movement_idempotency_key(request_id: Uuid, fulfilment_key: &str) -> String {
    let digest = format!("{:x}", Sha256::digest(fulfilment_key.as_bytes()));
    format!("stock-request:{request_id}:{}", &digest[..32])
}

async fn replace_draft_lines(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    request_id: Uuid,
    lines: &[StockRequestLineInput],
) -> Result<()> {
    let item_ids = lines.iter().map(|line| line.item_id).collect::<Vec<_>>();
    let active_ids = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id
          FROM assets_inventory_items
         WHERE tenant_id = $1 AND id = ANY($2) AND status = 'active' AND deleted_at IS NULL
         FOR SHARE
        "#,
    )
    .bind(tenant_id)
    .bind(&item_ids)
    .fetch_all(&mut **transaction)
    .await
    .context("Failed to validate stock request items")?;
    if active_ids.len() != lines.len() {
        bail!("Stock request items must be active Assets catalogue items");
    }
    for (index, line) in lines.iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO assets_inventory_stock_request_lines (
                id, tenant_id, request_id, line_number, item_id, requested_quantity_minor
            ) VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(tenant_id)
        .bind(request_id)
        .bind(i32::try_from(index + 1).context("Stock request line number overflow")?)
        .bind(line.item_id)
        .bind(line.requested_quantity_minor)
        .execute(&mut **transaction)
        .await
        .context("Failed to create stock request line")?;
    }
    Ok(())
}

async fn soft_delete_draft_lines(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    request_id: Uuid,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE assets_inventory_stock_request_lines
           SET deleted_at = NOW()
         WHERE tenant_id = $1 AND request_id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(request_id)
    .execute(&mut **transaction)
    .await
    .context("Failed to replace stock request lines")?;
    Ok(())
}

async fn lock_request(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    request_id: Uuid,
) -> Result<Option<StockRequestRecord>> {
    sqlx::query_as::<_, StockRequestRecord>(
        r#"
        SELECT request_number, requester_employee_id, department_id, purpose,
               status, version, created_by, submitted_by, submitted_at,
               decided_by, decided_at, decision_note, cancelled_at, cancellation_note,
               closed_at, closure_note
          FROM assets_inventory_stock_requests
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(request_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock stock request")
}

fn ensure_expected_version(current: &StockRequestRecord, expected: i32) -> Result<()> {
    if current.version != expected {
        bail!("Stock request changed since it was loaded");
    }
    Ok(())
}

fn ensure_status(current: &StockRequestRecord, allowed: &[&str]) -> Result<()> {
    if !allowed.contains(&current.status.as_str()) {
        bail!("Stock request is not in a valid state for this operation");
    }
    Ok(())
}

fn ensure_requester_actor(
    request: &StockRequestRecord,
    requester: &StockRequestEmployeeReference,
    actor_id: Uuid,
) -> Result<()> {
    if actor_id != request.created_by && requester.account_id != Some(actor_id) {
        bail!("Stock request can only be changed by its requester");
    }
    Ok(())
}

fn ensure_approver_actor(
    request: &StockRequestRecord,
    requester: &StockRequestEmployeeReference,
    actor_id: Uuid,
) -> Result<()> {
    if actor_id == request.created_by
        || request.submitted_by == Some(actor_id)
        || requester.account_id == Some(actor_id)
    {
        bail!("Stock request approver must be separate from the requester");
    }
    Ok(())
}

fn ensure_issuer_actor(
    request: &StockRequestRecord,
    requester: &StockRequestEmployeeReference,
    actor_id: Uuid,
) -> Result<()> {
    if actor_id == request.created_by
        || request.submitted_by == Some(actor_id)
        || request.decided_by == Some(actor_id)
        || requester.account_id == Some(actor_id)
    {
        bail!("Stock issuer must be separate from the requester and approver");
    }
    Ok(())
}

async fn request_line_count(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    request_id: Uuid,
) -> Result<usize> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)::BIGINT FROM assets_inventory_stock_request_lines WHERE tenant_id = $1 AND request_id = $2 AND deleted_at IS NULL",
    )
    .bind(tenant_id).bind(request_id)
    .fetch_one(&mut **transaction).await.context("Failed to count stock request lines")?;
    usize::try_from(count).context("Stock request line count is invalid")
}

async fn request_has_fulfilments(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    request_id: Uuid,
) -> Result<bool> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM assets_inventory_stock_request_fulfilments WHERE tenant_id = $1 AND request_id = $2 AND deleted_at IS NULL)",
    )
    .bind(tenant_id).bind(request_id)
    .fetch_one(&mut **transaction).await.context("Failed to inspect stock request issues")
}

struct NewRequestEvent<'a> {
    tenant_id: Uuid,
    request_id: Uuid,
    event_type: &'a str,
    from_status: Option<&'a str>,
    to_status: &'a str,
    request_version: i32,
    actor_id: Uuid,
    note: Option<&'a str>,
    idempotency_key: Option<&'a str>,
    fingerprint: Option<&'a str>,
}

async fn append_request_event(
    transaction: &mut Transaction<'_, Postgres>,
    event: NewRequestEvent<'_>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO assets_inventory_stock_request_events (
            id, tenant_id, request_id, event_type, from_status, to_status,
            request_version, actor_id, note, idempotency_key, request_fingerprint
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(event.tenant_id)
    .bind(event.request_id)
    .bind(event.event_type)
    .bind(event.from_status)
    .bind(event.to_status)
    .bind(event.request_version)
    .bind(event.actor_id)
    .bind(event.note)
    .bind(event.idempotency_key)
    .bind(event.fingerprint)
    .execute(&mut **transaction)
    .await
    .context("Failed to append stock request event")?;
    Ok(())
}

#[allow(clippy::too_many_arguments, reason = "transition evidence is explicit")]
async fn append_transition_event(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    request_id: Uuid,
    event_type: &str,
    from_status: &str,
    to_status: &str,
    request_version: i32,
    actor_id: Uuid,
    note: Option<&str>,
    idempotency_key: &str,
    fingerprint: &str,
) -> Result<()> {
    append_request_event(
        transaction,
        NewRequestEvent {
            tenant_id,
            request_id,
            event_type,
            from_status: Some(from_status),
            to_status,
            request_version,
            actor_id,
            note,
            idempotency_key: Some(idempotency_key),
            fingerprint: Some(fingerprint),
        },
    )
    .await
}

#[allow(
    clippy::too_many_arguments,
    reason = "audit evidence is intentionally explicit"
)]
async fn append_request_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    action: &str,
    request_id: Uuid,
    request_number: &str,
    status: &str,
    line_count: usize,
) -> Result<()> {
    append_audit(
        &mut **transaction,
        &NewAuditEvent::new(
            tenant_id,
            actor,
            format!("assets_inventory.stock_requests.{action}"),
            AuditOutcome::Succeeded,
            request_context,
        )
        .with_target(AuditTarget::new(
            "assets_inventory_stock_request",
            request_id.to_string(),
        ))
        .with_redacted_metadata(serde_json::Map::from_iter([
            ("request_number".to_string(), json!(request_number)),
            ("status".to_string(), json!(status)),
            ("line_count".to_string(), json!(line_count)),
        ])),
    )
    .await
    .context("Failed to append stock request audit event")?;
    Ok(())
}

async fn validate_approval_lines(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    request_id: Uuid,
    inputs: &[crate::stock_request_dtos::StockRequestApprovalLineInput],
) -> Result<Vec<(Uuid, i64)>> {
    if !(1..=MAX_REQUEST_LINES).contains(&inputs.len()) {
        bail!("Stock request approval must include every request line");
    }
    let rows = sqlx::query_as::<_, (Uuid, i64)>(
        r#"
        SELECT id, requested_quantity_minor
          FROM assets_inventory_stock_request_lines
         WHERE tenant_id = $1 AND request_id = $2 AND deleted_at IS NULL
         ORDER BY line_number
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(request_id)
    .fetch_all(&mut **transaction)
    .await
    .context("Failed to lock stock request approval lines")?;
    if rows.len() != inputs.len() {
        bail!("Stock request approval must include every request line");
    }
    let requested = rows.into_iter().collect::<BTreeMap<_, _>>();
    approval_values(&requested, inputs)
}

fn approval_values(
    requested: &BTreeMap<Uuid, i64>,
    inputs: &[crate::stock_request_dtos::StockRequestApprovalLineInput],
) -> Result<Vec<(Uuid, i64)>> {
    if requested.len() != inputs.len() {
        bail!("Stock request approval must include every request line");
    }
    let mut approved = Vec::with_capacity(inputs.len());
    let mut seen = BTreeSet::new();
    let mut positive = false;
    for input in inputs {
        if !seen.insert(input.request_line_id) {
            bail!("Stock request approval lines must be unique");
        }
        let requested_quantity = requested
            .get(&input.request_line_id)
            .ok_or_else(|| anyhow!("Stock request approval line is not part of the request"))?;
        if input.approved_quantity_minor < 0
            || input.approved_quantity_minor > *requested_quantity
            || input.approved_quantity_minor > MAX_SAFE_INTEGER
        {
            bail!("Stock request approved quantity exceeds the requested quantity");
        }
        positive |= input.approved_quantity_minor > 0;
        approved.push((input.request_line_id, input.approved_quantity_minor));
    }
    if !positive {
        bail!("Stock request approval requires at least one positive quantity");
    }
    Ok(approved)
}

async fn validate_fulfilment_lines(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    request_id: Uuid,
    inputs: &[crate::stock_request_dtos::FulfilStockRequestLineInput],
) -> Result<Vec<PreparedFulfilmentLine>> {
    if !(1..=MAX_REQUEST_LINES).contains(&inputs.len()) {
        bail!("Stock request fulfilment requires between one and {MAX_REQUEST_LINES} lines");
    }
    let locked_rows = sqlx::query_as::<_, (Uuid, Uuid, i64)>(
        r#"
        SELECT line.id, line.item_id, line.approved_quantity_minor
          FROM assets_inventory_stock_request_lines AS line
         WHERE line.tenant_id = $1 AND line.request_id = $2 AND line.deleted_at IS NULL
           AND line.approved_quantity_minor IS NOT NULL
         ORDER BY line.line_number
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(request_id)
    .fetch_all(&mut **transaction)
    .await
    .context("Failed to lock stock request fulfilment lines")?;
    let issued_rows = sqlx::query_as::<_, (Uuid, i64)>(
        r#"
        SELECT line.id, COALESCE(SUM(link.quantity_minor), 0)::BIGINT
          FROM assets_inventory_stock_request_lines AS line
          LEFT JOIN assets_inventory_stock_request_fulfilment_lines AS link
            ON link.request_line_id = line.id AND link.tenant_id = line.tenant_id
           AND link.deleted_at IS NULL
         WHERE line.tenant_id = $1 AND line.request_id = $2 AND line.deleted_at IS NULL
         GROUP BY line.id
        "#,
    )
    .bind(tenant_id)
    .bind(request_id)
    .fetch_all(&mut **transaction)
    .await
    .context("Failed to count prior stock request fulfilments")?
    .into_iter()
    .collect::<BTreeMap<_, _>>();
    let state = locked_rows
        .into_iter()
        .map(|(id, item_id, approved_quantity_minor)| {
            (
                id,
                FulfilmentLineState {
                    item_id,
                    approved_quantity_minor,
                    issued_quantity_minor: *issued_rows.get(&id).unwrap_or(&0),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut pairs = BTreeSet::new();
    let mut requested_by_line = BTreeMap::<Uuid, i64>::new();
    for input in inputs {
        ensure_positive_quantity(input.quantity_minor)?;
        if !pairs.insert((input.request_line_id, input.store_id)) {
            bail!("Stock request fulfilment line and store pairs must be unique");
        }
        let total = requested_by_line.entry(input.request_line_id).or_default();
        *total = total
            .checked_add(input.quantity_minor)
            .filter(|value| *value <= MAX_SAFE_INTEGER)
            .ok_or_else(|| anyhow!("Stock request fulfilment quantity is too large"))?;
    }
    for (line_id, quantity) in &requested_by_line {
        let line = state
            .get(line_id)
            .ok_or_else(|| anyhow!("Stock request fulfilment line is not approved"))?;
        let remaining = line
            .approved_quantity_minor
            .checked_sub(line.issued_quantity_minor)
            .filter(|value| *value >= 0)
            .ok_or_else(|| anyhow!("Stock request fulfilment quantities are inconsistent"))?;
        if *quantity > remaining {
            bail!("Stock request fulfilment exceeds the remaining approved quantity");
        }
    }
    let mut balance_keys = inputs
        .iter()
        .map(|line| Ok((line.item_id_from(&state)?, line.store_id)))
        .collect::<Result<Vec<_>>>()?;
    balance_keys.sort_unstable();
    balance_keys.dedup();
    let mut balances = BTreeMap::<(Uuid, Uuid), (i64, i16, i32)>::new();
    for (item_id, store_id) in balance_keys {
        let balance = sqlx::query_as::<_, (i64, i16, i32)>(
            r#"
            SELECT on_hand_minor, quantity_scale, version
              FROM assets_inventory_stock_balances
             WHERE tenant_id = $1 AND item_id = $2 AND store_id = $3 AND deleted_at IS NULL
             FOR UPDATE
            "#,
        )
        .bind(tenant_id)
        .bind(item_id)
        .bind(store_id)
        .fetch_optional(&mut **transaction)
        .await
        .context("Failed to lock stock request balance")?
        .ok_or_else(|| anyhow!("Stock request fulfilment store has no stock balance"))?;
        balances.insert((item_id, store_id), balance);
    }
    let mut required_by_balance = BTreeMap::<(Uuid, Uuid), i64>::new();
    let mut prepared = Vec::with_capacity(inputs.len());
    for input in inputs {
        let line = state
            .get(&input.request_line_id)
            .ok_or_else(|| anyhow!("Stock request fulfilment line is not approved"))?;
        let balance = balances
            .get(&(line.item_id, input.store_id))
            .ok_or_else(|| anyhow!("Stock request fulfilment balance could not be loaded"))?;
        if balance.2 != input.expected_balance_version {
            bail!("Stock balance changed since the fulfilment was prepared");
        }
        let total = required_by_balance
            .entry((line.item_id, input.store_id))
            .or_default();
        *total = total
            .checked_add(input.quantity_minor)
            .ok_or_else(|| anyhow!("Stock request fulfilment quantity is unsafe"))?;
        if *total > balance.0 {
            bail!("Stock request fulfilment exceeds on-hand stock");
        }
        prepared.push(PreparedFulfilmentLine {
            request_line_id: input.request_line_id,
            item_id: line.item_id,
            store_id: input.store_id,
            quantity_minor: input.quantity_minor,
        });
    }
    Ok(prepared)
}

trait FulfilInputItem {
    fn item_id_from(&self, state: &BTreeMap<Uuid, FulfilmentLineState>) -> Result<Uuid>;
}

impl FulfilInputItem for crate::stock_request_dtos::FulfilStockRequestLineInput {
    fn item_id_from(&self, state: &BTreeMap<Uuid, FulfilmentLineState>) -> Result<Uuid> {
        state
            .get(&self.request_line_id)
            .map(|line| line.item_id)
            .ok_or_else(|| anyhow!("Stock request fulfilment line is not approved"))
    }
}

async fn request_is_fully_issued(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    request_id: Uuid,
) -> Result<bool> {
    sqlx::query_scalar::<_, bool>(
        r#"
        SELECT COALESCE(BOOL_AND(line.approved_quantity_minor = COALESCE(issued.quantity_minor, 0)), FALSE)
          FROM assets_inventory_stock_request_lines AS line
          LEFT JOIN LATERAL (
              SELECT SUM(link.quantity_minor)::BIGINT AS quantity_minor
                FROM assets_inventory_stock_request_fulfilment_lines AS link
               WHERE link.tenant_id = line.tenant_id AND link.request_line_id = line.id
                 AND link.deleted_at IS NULL
          ) AS issued ON TRUE
         WHERE line.tenant_id = $1 AND line.request_id = $2 AND line.deleted_at IS NULL
        "#,
    )
    .bind(tenant_id).bind(request_id)
    .fetch_one(&mut **transaction).await.context("Failed to derive stock request fulfilment state")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stock_request_dtos::{
        StockRequestApprovalLineInput, StockRequestLineInput, StockRequestVersionCommand,
    };

    fn request_record(
        created_by: Uuid,
        submitted_by: Uuid,
        decided_by: Uuid,
    ) -> StockRequestRecord {
        StockRequestRecord {
            request_number: "SRQ-000001".to_string(),
            requester_employee_id: Uuid::new_v4(),
            department_id: Uuid::new_v4(),
            purpose: "Classroom supplies".to_string(),
            status: "approved".to_string(),
            version: 3,
            created_by,
            submitted_by: Some(submitted_by),
            submitted_at: Some(chrono::Utc::now()),
            decided_by: Some(decided_by),
            decided_at: Some(chrono::Utc::now()),
            decision_note: None,
            cancelled_at: None,
            cancellation_note: None,
            closed_at: None,
            closure_note: None,
        }
    }

    fn requester(account_id: Option<Uuid>) -> StockRequestEmployeeReference {
        StockRequestEmployeeReference {
            id: Uuid::new_v4(),
            account_id,
            employee_number: "EMP-001".to_string(),
            display_name: "Test Requester".to_string(),
            department_id: Uuid::new_v4(),
            department_code: "SCI".to_string(),
            department_name: "Science".to_string(),
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "explicit PostgreSQL lifecycle fixture"
    )]
    async fn create_submitted_request(
        pool: &PgPool,
        tenant_id: Uuid,
        requester: AuditActor,
        employee_id: Uuid,
        department_id: Uuid,
        item_id: Uuid,
        quantity_minor: i64,
        suffix: &str,
    ) -> StockRequestResponse {
        let created = StockRequestOps::create(
            pool,
            tenant_id,
            requester,
            RequestContext::generate(None),
            &CreateStockRequest {
                requester_employee_id: employee_id,
                department_id,
                purpose: format!("Lifecycle fixture {suffix}"),
                needed_by: None,
                idempotency_key: format!("stock-request-{suffix}-create-{tenant_id}"),
                lines: vec![StockRequestLineInput {
                    item_id,
                    requested_quantity_minor: quantity_minor,
                }],
            },
        )
        .await
        .expect("lifecycle request creates");
        StockRequestOps::submit(
            pool,
            tenant_id,
            created.summary.id,
            requester,
            RequestContext::generate(None),
            &StockRequestVersionCommand {
                expected_version: created.summary.version,
                idempotency_key: format!("stock-request-{suffix}-submit-{tenant_id}"),
            },
        )
        .await
        .expect("lifecycle request submits")
        .expect("lifecycle request exists")
    }

    #[test]
    fn draft_values_require_unique_exact_positive_lines() {
        let first = Uuid::new_v4();
        let valid = StockRequestValues::parse(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "  Classroom supplies  ",
            None,
            &[StockRequestLineInput {
                item_id: first,
                requested_quantity_minor: 2,
            }],
        )
        .expect("valid stock request");
        assert_eq!(valid.purpose, "Classroom supplies");

        let duplicate = StockRequestValues::parse(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Supplies",
            None,
            &[
                StockRequestLineInput {
                    item_id: first,
                    requested_quantity_minor: 1,
                },
                StockRequestLineInput {
                    item_id: first,
                    requested_quantity_minor: 2,
                },
            ],
        );
        assert!(duplicate.unwrap_err().to_string().contains("unique"));

        let unsafe_total = StockRequestValues::parse(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Supplies",
            None,
            &[
                StockRequestLineInput {
                    item_id: Uuid::new_v4(),
                    requested_quantity_minor: MAX_SAFE_INTEGER,
                },
                StockRequestLineInput {
                    item_id: Uuid::new_v4(),
                    requested_quantity_minor: 1,
                },
            ],
        );
        assert!(unsafe_total.unwrap_err().to_string().contains("too large"));
    }

    #[test]
    fn approval_supports_partial_quantities_but_requires_positive_total() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let requested = BTreeMap::from([(first, 10), (second, 5)]);
        let partial = approval_values(
            &requested,
            &[
                StockRequestApprovalLineInput {
                    request_line_id: first,
                    approved_quantity_minor: 4,
                },
                StockRequestApprovalLineInput {
                    request_line_id: second,
                    approved_quantity_minor: 0,
                },
            ],
        )
        .expect("partial approval");
        assert_eq!(partial, vec![(first, 4), (second, 0)]);

        let zero = approval_values(
            &requested,
            &[
                StockRequestApprovalLineInput {
                    request_line_id: first,
                    approved_quantity_minor: 0,
                },
                StockRequestApprovalLineInput {
                    request_line_id: second,
                    approved_quantity_minor: 0,
                },
            ],
        );
        assert!(zero.unwrap_err().to_string().contains("positive"));

        let excessive = approval_values(
            &requested,
            &[
                StockRequestApprovalLineInput {
                    request_line_id: first,
                    approved_quantity_minor: 11,
                },
                StockRequestApprovalLineInput {
                    request_line_id: second,
                    approved_quantity_minor: 0,
                },
            ],
        );
        assert!(excessive.unwrap_err().to_string().contains("exceeds"));
    }

    #[test]
    fn actor_separation_includes_the_requesters_linked_account() {
        let creator = Uuid::new_v4();
        let submitter = Uuid::new_v4();
        let approver = Uuid::new_v4();
        let linked = Uuid::new_v4();
        let issuer = Uuid::new_v4();
        let request = request_record(creator, submitter, approver);
        let requester = requester(Some(linked));

        assert!(ensure_requester_actor(&request, &requester, linked).is_ok());
        assert!(ensure_approver_actor(&request, &requester, creator).is_err());
        assert!(ensure_approver_actor(&request, &requester, linked).is_err());
        assert!(ensure_approver_actor(&request, &requester, approver).is_ok());
        assert!(ensure_issuer_actor(&request, &requester, approver).is_err());
        assert!(ensure_issuer_actor(&request, &requester, issuer).is_ok());
    }

    #[test]
    fn filters_and_idempotency_fingerprints_are_stable_and_scoped() {
        assert_eq!(
            parse_status_filter(Some(" approved ")).unwrap(),
            Some("approved")
        );
        assert!(parse_status_filter(Some("pending")).is_err());
        let command = StockRequestVersionCommand {
            expected_version: 2,
            idempotency_key: "request-2-submit".to_string(),
        };
        let id = Uuid::new_v4();
        let first = request_fingerprint("submit", Some(id), &command).unwrap();
        let replay = request_fingerprint("submit", Some(id), &command).unwrap();
        let other = request_fingerprint("cancel", Some(id), &command).unwrap();
        assert_eq!(first, replay);
        assert_ne!(first, other);
        assert_eq!(first.len(), 64);
    }

    #[actix_web::test]
    #[ignore = "requires STOCK_REQUEST_TEST_DATABASE_URL with migrations through 088"]
    async fn postgres_request_approval_and_partial_fulfilment_share_one_atomic_ledger() {
        use sqlx::postgres::PgPoolOptions;

        use crate::dtos::{CreateItemRequest, CreateStoreRequest};
        use crate::ops::{ItemOps, StoreOps};
        use crate::stock_dtos::{ManualReceiptRequest, StockQuantityLineInput};
        use crate::stock_ops::{StockBalanceOps, StockMovementOps};
        use crate::stock_request_dtos::{
            ApproveStockRequest, CreateStockRequest, FulfilStockRequest,
            FulfilStockRequestLineInput, StockRequestApprovalLineInput, StockRequestVersionCommand,
        };

        let database_url = std::env::var("STOCK_REQUEST_TEST_DATABASE_URL")
            .expect("STOCK_REQUEST_TEST_DATABASE_URL must target a disposable database");
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .connect(&database_url)
            .await
            .expect("disposable PostgreSQL database must be available");
        let tenant_id = Uuid::new_v4();
        let requester_id = Uuid::new_v4();
        let approver_id = Uuid::new_v4();
        let issuer_id = Uuid::new_v4();
        let department_id = Uuid::new_v4();
        let employee_id = Uuid::new_v4();
        sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, $2, 'Stock request test')")
            .bind(tenant_id)
            .bind(format!("stock-request-{tenant_id}"))
            .execute(&pool)
            .await
            .expect("tenant fixture");
        for (id, label) in [
            (requester_id, "requester"),
            (approver_id, "approver"),
            (issuer_id, "issuer"),
        ] {
            sqlx::query(
                "INSERT INTO users (id, tenant_id, email, password_hash, full_name) VALUES ($1, $2, $3, 'x', $4)",
            )
            .bind(id)
            .bind(tenant_id)
            .bind(format!("{label}-{id}@example.test"))
            .bind(label)
            .execute(&pool)
            .await
            .expect("user fixture");
        }
        sqlx::query(
            "INSERT INTO departments (id, tenant_id, code, name) VALUES ($1, $2, 'SCI', 'Science')",
        )
        .bind(department_id)
        .bind(tenant_id)
        .execute(&pool)
        .await
        .expect("department fixture");
        sqlx::query(
            "INSERT INTO employees (id, tenant_id, account_id, employee_number, display_name, department_id) VALUES ($1, $2, $3, 'EMP-001', 'Test Requester', $4)",
        )
        .bind(employee_id)
        .bind(tenant_id)
        .bind(requester_id)
        .bind(department_id)
        .execute(&pool)
        .await
        .expect("employee fixture");
        let issuer = AuditActor::person(issuer_id);
        let item = ItemOps::create(
            &pool,
            tenant_id,
            issuer,
            RequestContext::generate(None),
            &CreateItemRequest {
                name: "Exercise book".into(),
                description: None,
                barcode: None,
                unit_label: "each".into(),
                quantity_scale: 0,
                reorder_level_minor: None,
                idempotency_key: format!("stock-request-item-{tenant_id}"),
            },
        )
        .await
        .expect("item fixture");
        let store = StoreOps::create(
            &pool,
            tenant_id,
            issuer,
            RequestContext::generate(None),
            &CreateStoreRequest {
                name: "Main store".into(),
                location_label: None,
                notes: None,
                idempotency_key: format!("stock-request-store-{tenant_id}"),
            },
        )
        .await
        .expect("store fixture");
        StockMovementOps::create_manual_receipt(
            &pool,
            tenant_id,
            issuer,
            RequestContext::generate(None),
            &ManualReceiptRequest {
                effective_on: chrono::Utc::now().date_naive(),
                reference: Some("Opening balance".into()),
                reason: None,
                idempotency_key: format!("stock-request-opening-{tenant_id}"),
                lines: vec![StockQuantityLineInput {
                    item_id: item.id,
                    store_id: store.id,
                    quantity_minor: 10,
                }],
            },
        )
        .await
        .expect("opening balance");
        let requester = AuditActor::person(requester_id);
        let created = StockRequestOps::create(
            &pool,
            tenant_id,
            requester,
            RequestContext::generate(None),
            &CreateStockRequest {
                requester_employee_id: employee_id,
                department_id,
                purpose: "Grade 5 exercise books".into(),
                needed_by: None,
                idempotency_key: format!("stock-request-create-{tenant_id}"),
                lines: vec![StockRequestLineInput {
                    item_id: item.id,
                    requested_quantity_minor: 8,
                }],
            },
        )
        .await
        .expect("request creates");
        assert!(
            StockRequestOps::get(&pool, Uuid::new_v4(), created.summary.id)
                .await
                .expect("cross-tenant request read")
                .is_none(),
            "stock requests must remain tenant-isolated"
        );
        let candidates = StockRequestCandidateOps::requesters(
            &pool,
            tenant_id,
            Some("Test Requester"),
            Some(department_id),
        )
        .await
        .expect("requester candidates");
        assert_eq!(candidates.employees.len(), 1);
        let candidate_json = serde_json::to_value(candidates).expect("candidate JSON");
        assert!(candidate_json.get("account_id").is_none());
        assert!(!candidate_json.to_string().contains("account_id"));
        let departments = StockRequestCandidateOps::departments(&pool, tenant_id, Some("Science"))
            .await
            .expect("department candidates");
        assert_eq!(departments.departments.len(), 1);
        let (listed, total) = StockRequestOps::list(
            &pool,
            tenant_id,
            1,
            25,
            Some("Grade 5"),
            Some("draft"),
            Some(employee_id),
            Some(department_id),
        )
        .await
        .expect("stock request worklist");
        assert_eq!(total, 1);
        assert_eq!(listed.requests[0].id, created.summary.id);
        let created = StockRequestOps::update(
            &pool,
            tenant_id,
            created.summary.id,
            requester,
            RequestContext::generate(None),
            &UpdateStockRequest {
                requester_employee_id: employee_id,
                department_id,
                purpose: "Grade 5 exercise books for term two".into(),
                needed_by: None,
                expected_version: created.summary.version,
                idempotency_key: format!("stock-request-update-{tenant_id}"),
                lines: vec![StockRequestLineInput {
                    item_id: item.id,
                    requested_quantity_minor: 8,
                }],
            },
        )
        .await
        .expect("request updates")
        .expect("request exists");
        let submitted = StockRequestOps::submit(
            &pool,
            tenant_id,
            created.summary.id,
            requester,
            RequestContext::generate(None),
            &StockRequestVersionCommand {
                expected_version: created.summary.version,
                idempotency_key: format!("stock-request-submit-{tenant_id}"),
            },
        )
        .await
        .expect("request submits")
        .expect("request exists");
        let approved = StockRequestOps::approve(
            &pool,
            tenant_id,
            submitted.summary.id,
            AuditActor::person(approver_id),
            RequestContext::generate(None),
            &ApproveStockRequest {
                expected_version: submitted.summary.version,
                note: None,
                idempotency_key: format!("stock-request-approve-{tenant_id}"),
                lines: vec![StockRequestApprovalLineInput {
                    request_line_id: submitted.lines[0].id,
                    approved_quantity_minor: 8,
                }],
            },
        )
        .await
        .expect("request approves")
        .expect("request exists");
        let (balances, _) =
            StockBalanceOps::list(&pool, tenant_id, 1, 25, None, Some(item.id), Some(store.id))
                .await
                .expect("balance after approval");
        assert_eq!(
            balances[0].on_hand_minor, 10,
            "approval must not reserve stock"
        );
        let first_preview =
            StockRequestOps::fulfilment_preview(&pool, tenant_id, approved.summary.id)
                .await
                .expect("first preview")
                .expect("request exists");
        let first_command = FulfilStockRequest {
            expected_request_version: approved.summary.version,
            effective_on: chrono::Utc::now().date_naive(),
            reason: None,
            idempotency_key: format!("stock-request-first-issue-{tenant_id}"),
            lines: vec![FulfilStockRequestLineInput {
                request_line_id: approved.lines[0].id,
                store_id: store.id,
                quantity_minor: 3,
                expected_balance_version: first_preview.balances[0].version,
            }],
        };
        let separated = StockRequestOps::fulfil(
            &pool,
            tenant_id,
            approved.summary.id,
            AuditActor::person(approver_id),
            RequestContext::generate(None),
            &first_command,
        )
        .await;
        assert!(separated.unwrap_err().to_string().contains("separate"));
        let partial = StockRequestOps::fulfil(
            &pool,
            tenant_id,
            approved.summary.id,
            issuer,
            RequestContext::generate(None),
            &first_command,
        )
        .await
        .expect("first issue")
        .expect("request exists");
        assert_eq!(partial.request.summary.status, "partially_fulfilled");
        let replay = StockRequestOps::fulfil(
            &pool,
            tenant_id,
            approved.summary.id,
            issuer,
            RequestContext::generate(None),
            &first_command,
        )
        .await
        .expect("first issue replays")
        .expect("request exists");
        assert_eq!(partial.movement_id, replay.movement_id);
        let second_preview =
            StockRequestOps::fulfilment_preview(&pool, tenant_id, approved.summary.id)
                .await
                .expect("second preview")
                .expect("request exists");
        let fulfilled = StockRequestOps::fulfil(
            &pool,
            tenant_id,
            approved.summary.id,
            issuer,
            RequestContext::generate(None),
            &FulfilStockRequest {
                expected_request_version: partial.request.summary.version,
                effective_on: chrono::Utc::now().date_naive(),
                reason: None,
                idempotency_key: format!("stock-request-second-issue-{tenant_id}"),
                lines: vec![FulfilStockRequestLineInput {
                    request_line_id: approved.lines[0].id,
                    store_id: store.id,
                    quantity_minor: 5,
                    expected_balance_version: second_preview.balances[0].version,
                }],
            },
        )
        .await
        .expect("second issue")
        .expect("request exists");
        assert_eq!(fulfilled.request.summary.status, "fulfilled");
        assert_eq!(fulfilled.request.summary.issued_quantity_minor, 8);
        let (balances, _) =
            StockBalanceOps::list(&pool, tenant_id, 1, 25, None, Some(item.id), Some(store.id))
                .await
                .expect("final balance");
        assert_eq!(balances[0].on_hand_minor, 2);

        let removable = StockRequestOps::create(
            &pool,
            tenant_id,
            requester,
            RequestContext::generate(None),
            &CreateStockRequest {
                requester_employee_id: employee_id,
                department_id,
                purpose: "Draft to remove".into(),
                needed_by: None,
                idempotency_key: format!("stock-request-removable-{tenant_id}"),
                lines: vec![StockRequestLineInput {
                    item_id: item.id,
                    requested_quantity_minor: 1,
                }],
            },
        )
        .await
        .expect("removable request");
        assert!(
            StockRequestOps::fulfilment_preview(&pool, tenant_id, removable.summary.id)
                .await
                .is_err()
        );
        assert!(
            StockRequestOps::delete(
                &pool,
                tenant_id,
                removable.summary.id,
                requester,
                RequestContext::generate(None),
                &StockRequestVersionCommand {
                    expected_version: removable.summary.version,
                    idempotency_key: format!("stock-request-remove-{tenant_id}"),
                },
            )
            .await
            .expect("draft removal")
        );
        assert!(
            StockRequestOps::get(&pool, tenant_id, removable.summary.id)
                .await
                .expect("removed request read")
                .is_none()
        );

        let rejected = create_submitted_request(
            &pool,
            tenant_id,
            requester,
            employee_id,
            department_id,
            item.id,
            1,
            "rejected",
        )
        .await;
        let rejected = StockRequestOps::reject(
            &pool,
            tenant_id,
            rejected.summary.id,
            AuditActor::person(approver_id),
            RequestContext::generate(None),
            &StockRequestReasonCommand {
                expected_version: rejected.summary.version,
                reason: "Not required this term".into(),
                idempotency_key: format!("stock-request-reject-{tenant_id}"),
            },
        )
        .await
        .expect("request rejection")
        .expect("request exists");
        assert_eq!(rejected.summary.status, "rejected");
        assert_eq!(rejected.lines[0].approved_quantity_minor, Some(0));

        let cancelled = create_submitted_request(
            &pool,
            tenant_id,
            requester,
            employee_id,
            department_id,
            item.id,
            1,
            "cancelled-submitted",
        )
        .await;
        let cancelled = StockRequestOps::cancel(
            &pool,
            tenant_id,
            cancelled.summary.id,
            requester,
            RequestContext::generate(None),
            &StockRequestReasonCommand {
                expected_version: cancelled.summary.version,
                reason: "Department no longer needs this".into(),
                idempotency_key: format!("stock-request-cancel-submitted-{tenant_id}"),
            },
        )
        .await
        .expect("submitted request cancellation")
        .expect("request exists");
        assert_eq!(cancelled.summary.status, "cancelled");

        let approved_to_cancel = create_submitted_request(
            &pool,
            tenant_id,
            requester,
            employee_id,
            department_id,
            item.id,
            1,
            "cancelled-approved",
        )
        .await;
        let approved_to_cancel = StockRequestOps::approve(
            &pool,
            tenant_id,
            approved_to_cancel.summary.id,
            AuditActor::person(approver_id),
            RequestContext::generate(None),
            &ApproveStockRequest {
                expected_version: approved_to_cancel.summary.version,
                note: Some("Approved before cancellation".into()),
                idempotency_key: format!("stock-request-approve-to-cancel-{tenant_id}"),
                lines: vec![StockRequestApprovalLineInput {
                    request_line_id: approved_to_cancel.lines[0].id,
                    approved_quantity_minor: 1,
                }],
            },
        )
        .await
        .expect("request approval before cancellation")
        .expect("request exists");
        let approved_to_cancel = StockRequestOps::cancel(
            &pool,
            tenant_id,
            approved_to_cancel.summary.id,
            requester,
            RequestContext::generate(None),
            &StockRequestReasonCommand {
                expected_version: approved_to_cancel.summary.version,
                reason: "Cancelled before store issue".into(),
                idempotency_key: format!("stock-request-cancel-approved-{tenant_id}"),
            },
        )
        .await
        .expect("approved request cancellation")
        .expect("request exists");
        assert_eq!(approved_to_cancel.summary.status, "cancelled");

        let closable = create_submitted_request(
            &pool,
            tenant_id,
            requester,
            employee_id,
            department_id,
            item.id,
            2,
            "closable",
        )
        .await;
        let closable = StockRequestOps::approve(
            &pool,
            tenant_id,
            closable.summary.id,
            AuditActor::person(approver_id),
            RequestContext::generate(None),
            &ApproveStockRequest {
                expected_version: closable.summary.version,
                note: None,
                idempotency_key: format!("stock-request-closable-approve-{tenant_id}"),
                lines: vec![StockRequestApprovalLineInput {
                    request_line_id: closable.lines[0].id,
                    approved_quantity_minor: 2,
                }],
            },
        )
        .await
        .expect("closable approval")
        .expect("request exists");
        let preview = StockRequestOps::fulfilment_preview(&pool, tenant_id, closable.summary.id)
            .await
            .expect("closable preview")
            .expect("request exists");
        let closable = StockRequestOps::fulfil(
            &pool,
            tenant_id,
            closable.summary.id,
            issuer,
            RequestContext::generate(None),
            &FulfilStockRequest {
                expected_request_version: closable.summary.version,
                effective_on: chrono::Utc::now().date_naive(),
                reason: None,
                idempotency_key: format!("stock-request-closable-issue-{tenant_id}"),
                lines: vec![FulfilStockRequestLineInput {
                    request_line_id: closable.lines[0].id,
                    store_id: store.id,
                    quantity_minor: 1,
                    expected_balance_version: preview.balances[0].version,
                }],
            },
        )
        .await
        .expect("closable partial issue")
        .expect("request exists");
        assert_eq!(closable.request.summary.status, "partially_fulfilled");
        let closed = StockRequestOps::close(
            &pool,
            tenant_id,
            closable.request.summary.id,
            AuditActor::person(approver_id),
            RequestContext::generate(None),
            &CloseStockRequest {
                expected_version: closable.request.summary.version,
                note: Some("Close remaining quantity".into()),
                idempotency_key: format!("stock-request-close-{tenant_id}"),
            },
        )
        .await
        .expect("partial request closure")
        .expect("request exists");
        assert_eq!(closed.summary.status, "closed");
    }
}
