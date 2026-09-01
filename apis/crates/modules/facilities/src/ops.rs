//! Transactional Facilities workflows and scoped operational projections.
//!
//! Every mutation re-proves tenant and record scope under row locks, appends
//! reduced audit evidence in the same transaction, and retains completion and
//! inspection submissions as immutable records.

use std::collections::HashMap;

use anyhow::{Context, Result, anyhow, bail};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_hr_payroll::{models::EmployeeReference, ops::EmployeeOps};
use serde_json::{Map, Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    ArchiveFacilityLocationRequest, CreateFacilityLocationRequest, CreateFacilityServiceRequest,
    CreateFacilityWorkOrderRequest, FacilitiesRequestScope, FacilitiesWorkOrderScope,
    FacilityEventResponse, FacilityInspectionResponse, FacilityLocationQuery,
    FacilityLocationResponse, FacilityReferenceData, FacilityReferenceQuery, FacilityRequestQuery,
    FacilityServiceRequestRecord, FacilityServiceRequestSummary, FacilityTransitionRequest,
    FacilityWorkOrderQuery, FacilityWorkOrderRecord, FacilityWorkOrderSummary,
    FacilityWorkOrderTransitionRequest, InspectFacilityWorkOrderRequest,
    SubmitFacilityCompletionRequest, UpdateFacilityLocationRequest,
    models::{
        FacilityEventRow, FacilityInspectionRow, FacilityLocationRow, FacilityRequestRow,
        FacilityWorkOrderRow, LockedFacilityRequest, LockedFacilityWorkOrder,
    },
};

pub struct FacilitiesOps;

impl FacilitiesOps {
    pub async fn list_locations(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &FacilityLocationQuery,
    ) -> Result<Vec<FacilityLocationResponse>> {
        let search = search_pattern(query.search.as_deref());
        sqlx::query_as::<_, FacilityLocationRow>(LOCATION_LIST)
            .bind(tenant_id)
            .bind(query.parent_id)
            .bind(query.kind.map(|kind| kind.as_str()))
            .bind(query.status.as_deref())
            .bind(search.as_deref())
            .fetch_all(pool)
            .await
            .context("Failed to list Facilities locations")
            .map(|rows| rows.into_iter().map(location_response).collect())
    }

    pub async fn get_location(
        pool: &PgPool,
        tenant_id: Uuid,
        location_id: Uuid,
    ) -> Result<Option<FacilityLocationResponse>> {
        sqlx::query_as::<_, FacilityLocationRow>(LOCATION_BY_ID)
            .bind(tenant_id)
            .bind(location_id)
            .fetch_optional(pool)
            .await
            .context("Failed to read the Facilities location")
            .map(|row| row.map(location_response))
    }

    pub async fn reference_data(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &FacilityReferenceQuery,
    ) -> Result<FacilityReferenceData> {
        let locations = Self::list_locations(
            pool,
            tenant_id,
            &FacilityLocationQuery {
                search: query.search.clone(),
                status: Some("active".to_string()),
                ..FacilityLocationQuery::default()
            },
        )
        .await?;
        let employees = EmployeeOps::list_references(
            pool,
            tenant_id,
            query.search.as_deref(),
            Some("active"),
            100,
        )
        .await?;
        Ok(FacilityReferenceData {
            locations,
            employees,
        })
    }

    pub async fn create_location(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &CreateFacilityLocationRequest,
    ) -> Result<FacilityLocationResponse> {
        let actor_id = person_actor_id(actor)?;
        validate_capacity(request.capacity)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Facilities location creation")?;
        let location_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO facility_locations (
                id, tenant_id, parent_id, kind, code, name, capacity, notes,
                created_by, updated_by
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$9)
            "#,
        )
        .bind(location_id)
        .bind(tenant_id)
        .bind(request.parent_id)
        .bind(request.kind.as_str())
        .bind(trimmed_required(&request.code, "Location code")?)
        .bind(trimmed_required(&request.name, "Location name")?)
        .bind(request.capacity)
        .bind(trimmed_optional(request.notes.as_deref()))
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            location_database_error(error, "A location with this code already exists")
        })?;
        append_domain_audit(
            &mut transaction,
            tenant_id,
            actor,
            context,
            "facilities.locations.create",
            "facility_location",
            location_id,
            json!({"kind": request.kind.as_str(), "code": request.code.trim()}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Facilities location creation")?;
        Self::get_location(pool, tenant_id, location_id)
            .await?
            .ok_or_else(|| anyhow!("The Facilities location could not be reloaded"))
    }

    pub async fn update_location(
        pool: &PgPool,
        tenant_id: Uuid,
        location_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &UpdateFacilityLocationRequest,
    ) -> Result<Option<FacilityLocationResponse>> {
        let actor_id = person_actor_id(actor)?;
        validate_capacity(request.capacity)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Facilities location update")?;
        let current = sqlx::query_as::<_, (i32, String)>(
            "SELECT version, status FROM facility_locations WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL FOR UPDATE",
        )
        .bind(tenant_id)
        .bind(location_id)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to lock the Facilities location")?;
        let Some((version, status)) = current else {
            return Ok(None);
        };
        ensure_version(version, request.expected_version, "Facilities location")?;
        if status != "active" {
            bail!("An archived Facilities location cannot be changed");
        }
        sqlx::query(
            r#"
            UPDATE facility_locations
               SET parent_id=$1, kind=$2, code=$3, name=$4, capacity=$5, notes=$6,
                   updated_by=$7, version=version+1
             WHERE tenant_id=$8 AND id=$9 AND deleted_at IS NULL
            "#,
        )
        .bind(request.parent_id)
        .bind(request.kind.as_str())
        .bind(trimmed_required(&request.code, "Location code")?)
        .bind(trimmed_required(&request.name, "Location name")?)
        .bind(request.capacity)
        .bind(trimmed_optional(request.notes.as_deref()))
        .bind(actor_id)
        .bind(tenant_id)
        .bind(location_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            location_database_error(error, "A location with this code already exists")
        })?;
        append_domain_audit(
            &mut transaction,
            tenant_id,
            actor,
            context,
            "facilities.locations.update",
            "facility_location",
            location_id,
            json!({"expected_version": request.expected_version}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Facilities location update")?;
        Self::get_location(pool, tenant_id, location_id).await
    }

    pub async fn archive_location(
        pool: &PgPool,
        tenant_id: Uuid,
        location_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &ArchiveFacilityLocationRequest,
    ) -> Result<Option<FacilityLocationResponse>> {
        let actor_id = person_actor_id(actor)?;
        let reason = trimmed_required(&request.reason, "Archive reason")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Facilities location archive")?;
        let current = sqlx::query_as::<_, (i32, String)>(
            "SELECT version, status FROM facility_locations WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL FOR UPDATE",
        )
        .bind(tenant_id)
        .bind(location_id)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to lock the Facilities location")?;
        let Some((version, status)) = current else {
            return Ok(None);
        };
        ensure_version(version, request.expected_version, "Facilities location")?;
        if status != "active" {
            bail!("The Facilities location is already archived");
        }
        let has_dependencies = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM facility_locations
                 WHERE tenant_id=$1 AND parent_id=$2 AND status='active' AND deleted_at IS NULL
                UNION ALL
                SELECT 1 FROM facility_service_requests
                 WHERE tenant_id=$1 AND location_id=$2
                   AND status IN ('open','assigned','resolved') AND deleted_at IS NULL
                UNION ALL
                SELECT 1 FROM facility_work_orders
                 WHERE tenant_id=$1 AND location_id=$2
                   AND status IN ('assigned','in_progress','ready_for_inspection')
                   AND deleted_at IS NULL
            )
            "#,
        )
        .bind(tenant_id)
        .bind(location_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to check Facilities location usage")?;
        if has_dependencies {
            bail!(
                "Move active child locations and finish open Facilities work before archiving this location"
            );
        }
        sqlx::query(
            r#"
            UPDATE facility_locations
               SET status='archived', archived_by=$1, archived_at=NOW(), archive_reason=$2,
                   updated_by=$1, version=version+1
             WHERE tenant_id=$3 AND id=$4 AND deleted_at IS NULL
            "#,
        )
        .bind(actor_id)
        .bind(reason)
        .bind(tenant_id)
        .bind(location_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to archive the Facilities location")?;
        append_domain_audit(
            &mut transaction,
            tenant_id,
            actor,
            context,
            "facilities.locations.archive",
            "facility_location",
            location_id,
            json!({"reason": reason}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Facilities location archive")?;
        Self::get_location(pool, tenant_id, location_id).await
    }

    pub async fn list_requests(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: FacilitiesRequestScope,
        query: &FacilityRequestQuery,
    ) -> Result<(Vec<FacilityServiceRequestSummary>, i64)> {
        ensure_request_visibility(scope)?;
        let (page, per_page) = bounded_page(query.page, query.per_page);
        let offset = (page - 1) * per_page;
        let search = search_pattern(query.search.as_deref());
        let reporter_user_id = scope.reporter_user_id();
        let rows = sqlx::query_as::<_, FacilityRequestRow>(REQUEST_LIST)
            .bind(tenant_id)
            .bind(query.status.map(|status| status.as_str()))
            .bind(query.priority.map(|priority| priority.as_str()))
            .bind(query.location_id)
            .bind(search.as_deref())
            .bind(reporter_user_id)
            .bind(per_page)
            .bind(offset)
            .fetch_all(pool)
            .await
            .context("Failed to list Facilities service requests")?;
        let total = sqlx::query_scalar::<_, i64>(REQUEST_COUNT)
            .bind(tenant_id)
            .bind(query.status.map(|status| status.as_str()))
            .bind(query.priority.map(|priority| priority.as_str()))
            .bind(query.location_id)
            .bind(search.as_deref())
            .bind(reporter_user_id)
            .fetch_one(pool)
            .await
            .context("Failed to count Facilities service requests")?;
        Ok((rows.into_iter().map(request_summary).collect(), total))
    }

    pub async fn get_request(
        pool: &PgPool,
        tenant_id: Uuid,
        request_id: Uuid,
        scope: FacilitiesRequestScope,
    ) -> Result<Option<FacilityServiceRequestRecord>> {
        ensure_request_visibility(scope)?;
        let row = sqlx::query_as::<_, FacilityRequestRow>(REQUEST_BY_ID)
            .bind(tenant_id)
            .bind(request_id)
            .bind(scope.reporter_user_id())
            .fetch_optional(pool)
            .await
            .context("Failed to read the Facilities service request")?;
        let Some(row) = row else {
            return Ok(None);
        };
        let history = event_history(pool, tenant_id, Some(request_id), None).await?;
        Ok(Some(request_record(row, history)))
    }

    pub async fn create_request(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: FacilitiesRequestScope,
        actor: AuditActor,
        context: RequestContext,
        request: &CreateFacilityServiceRequest,
    ) -> Result<FacilityServiceRequestRecord> {
        ensure_request_visibility(scope)?;
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Facilities request creation")?;
        ensure_active_location(&mut transaction, tenant_id, request.location_id).await?;
        let reference =
            reserve_reference(&mut transaction, tenant_id, ReferenceKind::Request).await?;
        let request_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO facility_service_requests (
                id, tenant_id, reference, location_id, reporter_user_id,
                priority, summary, description, created_by, updated_by
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$5,$5)
            "#,
        )
        .bind(request_id)
        .bind(tenant_id)
        .bind(&reference)
        .bind(request.location_id)
        .bind(actor_id)
        .bind(request.priority.as_str())
        .bind(trimmed_required(&request.summary, "Request summary")?)
        .bind(trimmed_required(
            &request.description,
            "Request description",
        )?)
        .execute(&mut *transaction)
        .await
        .context("Failed to create the Facilities service request")?;
        append_facilities_evidence(
            &mut transaction,
            FacilitiesEvidence {
                tenant_id,
                service_request_id: Some(request_id),
                work_order_id: None,
                actor,
                context,
                event_type: "facilities.request.created",
                operation: "facilities.requests.create",
                target_kind: "facility_service_request",
                target_id: request_id,
                metadata: json!({"reference": reference, "priority": request.priority.as_str()}),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Facilities request creation")?;
        Self::get_request(pool, tenant_id, request_id, scope)
            .await?
            .ok_or_else(|| anyhow!("The Facilities service request could not be reloaded"))
    }

    pub async fn cancel_request(
        pool: &PgPool,
        tenant_id: Uuid,
        request_id: Uuid,
        scope: FacilitiesRequestScope,
        actor: AuditActor,
        context: RequestContext,
        request: &FacilityTransitionRequest,
    ) -> Result<Option<FacilityServiceRequestRecord>> {
        ensure_request_visibility(scope)?;
        let actor_id = person_actor_id(actor)?;
        let reason = trimmed_required(&request.reason, "Cancellation reason")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Facilities request cancellation")?;
        let Some(current) = lock_request(&mut transaction, tenant_id, request_id, scope).await?
        else {
            return Ok(None);
        };
        ensure_version(
            current.version,
            request.expected_version,
            "Facilities request",
        )?;
        if current.status != "open" {
            bail!("Only an open Facilities request without a work order can be cancelled here");
        }
        let has_work_order = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM facility_work_orders WHERE tenant_id=$1 AND service_request_id=$2 AND deleted_at IS NULL)",
        )
        .bind(tenant_id)
        .bind(request_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to check the Facilities work-order link")?;
        if has_work_order {
            bail!("Cancel the linked work order instead of cancelling this Facilities request");
        }
        sqlx::query(
            r#"
            UPDATE facility_service_requests
               SET status='cancelled', cancelled_by=$1, cancelled_at=NOW(),
                   cancellation_reason=$2, updated_by=$1, version=version+1
             WHERE tenant_id=$3 AND id=$4 AND deleted_at IS NULL
            "#,
        )
        .bind(actor_id)
        .bind(reason)
        .bind(tenant_id)
        .bind(request_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to cancel the Facilities request")?;
        append_facilities_evidence(
            &mut transaction,
            FacilitiesEvidence {
                tenant_id,
                service_request_id: Some(request_id),
                work_order_id: None,
                actor,
                context,
                event_type: "facilities.request.cancelled",
                operation: "facilities.requests.cancel",
                target_kind: "facility_service_request",
                target_id: request_id,
                metadata: json!({"reason": reason}),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Facilities request cancellation")?;
        Self::get_request(pool, tenant_id, request_id, scope).await
    }

    pub async fn close_request(
        pool: &PgPool,
        tenant_id: Uuid,
        request_id: Uuid,
        scope: FacilitiesRequestScope,
        actor: AuditActor,
        context: RequestContext,
        request: &FacilityTransitionRequest,
    ) -> Result<Option<FacilityServiceRequestRecord>> {
        ensure_request_management(scope)?;
        let actor_id = person_actor_id(actor)?;
        let reason = trimmed_required(&request.reason, "Closure reason")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Facilities request closure")?;
        let Some(current) = lock_request(&mut transaction, tenant_id, request_id, scope).await?
        else {
            return Ok(None);
        };
        ensure_version(
            current.version,
            request.expected_version,
            "Facilities request",
        )?;
        if current.status != "resolved" {
            bail!("Only a resolved Facilities request can be closed");
        }
        sqlx::query(
            r#"
            UPDATE facility_service_requests
               SET status='closed', closed_by=$1, closed_at=NOW(), closure_reason=$2,
                   updated_by=$1, version=version+1
             WHERE tenant_id=$3 AND id=$4 AND deleted_at IS NULL
            "#,
        )
        .bind(actor_id)
        .bind(reason)
        .bind(tenant_id)
        .bind(request_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to close the Facilities request")?;
        append_facilities_evidence(
            &mut transaction,
            FacilitiesEvidence {
                tenant_id,
                service_request_id: Some(request_id),
                work_order_id: None,
                actor,
                context,
                event_type: "facilities.request.closed",
                operation: "facilities.requests.close",
                target_kind: "facility_service_request",
                target_id: request_id,
                metadata: json!({"reason": reason}),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Facilities request closure")?;
        Self::get_request(pool, tenant_id, request_id, scope).await
    }

    pub async fn list_work_orders(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: FacilitiesWorkOrderScope,
        query: &FacilityWorkOrderQuery,
    ) -> Result<(Vec<FacilityWorkOrderSummary>, i64)> {
        let visibility = work_order_visibility(pool, tenant_id, scope).await?;
        let (page, per_page) = bounded_page(query.page, query.per_page);
        let offset = (page - 1) * per_page;
        let search = search_pattern(query.search.as_deref());
        let assigned_scope_id = visibility.assigned_employee_id();
        let rows = sqlx::query_as::<_, FacilityWorkOrderRow>(WORK_ORDER_LIST)
            .bind(tenant_id)
            .bind(query.status.map(|status| status.as_str()))
            .bind(query.assigned_employee_id)
            .bind(query.location_id)
            .bind(search.as_deref())
            .bind(assigned_scope_id)
            .bind(per_page)
            .bind(offset)
            .fetch_all(pool)
            .await
            .context("Failed to list Facilities work orders")?;
        let total = sqlx::query_scalar::<_, i64>(WORK_ORDER_COUNT)
            .bind(tenant_id)
            .bind(query.status.map(|status| status.as_str()))
            .bind(query.assigned_employee_id)
            .bind(query.location_id)
            .bind(search.as_deref())
            .bind(assigned_scope_id)
            .fetch_one(pool)
            .await
            .context("Failed to count Facilities work orders")?;
        Ok((
            hydrate_work_order_summaries(pool, tenant_id, rows).await?,
            total,
        ))
    }

    pub async fn get_work_order(
        pool: &PgPool,
        tenant_id: Uuid,
        work_order_id: Uuid,
        scope: FacilitiesWorkOrderScope,
    ) -> Result<Option<FacilityWorkOrderRecord>> {
        let visibility = work_order_visibility(pool, tenant_id, scope).await?;
        let row = sqlx::query_as::<_, FacilityWorkOrderRow>(WORK_ORDER_BY_ID)
            .bind(tenant_id)
            .bind(work_order_id)
            .bind(visibility.assigned_employee_id())
            .fetch_optional(pool)
            .await
            .context("Failed to read the Facilities work order")?;
        let Some(row) = row else {
            return Ok(None);
        };
        let employee = EmployeeOps::get_reference(pool, tenant_id, row.assigned_employee_id)
            .await?
            .ok_or_else(|| anyhow!("The Facilities work-order assignee is unavailable"))?;
        let inspections = sqlx::query_as::<_, FacilityInspectionRow>(
            r#"
            SELECT inspection.id, inspection.outcome, inspection.notes,
                   inspection.inspected_by, account.full_name AS inspector_name,
                   inspection.created_at
              FROM facility_work_order_inspections AS inspection
              JOIN users AS account
                ON account.id=inspection.inspected_by AND account.tenant_id=inspection.tenant_id
             WHERE inspection.tenant_id=$1 AND inspection.work_order_id=$2
             ORDER BY inspection.created_at DESC, inspection.id DESC
            "#,
        )
        .bind(tenant_id)
        .bind(work_order_id)
        .fetch_all(pool)
        .await
        .context("Failed to load Facilities inspections")?
        .into_iter()
        .map(inspection_response)
        .collect();
        let history = event_history(pool, tenant_id, None, Some(work_order_id)).await?;
        Ok(Some(work_order_record(row, employee, inspections, history)))
    }

    pub async fn create_work_order(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: FacilitiesWorkOrderScope,
        actor: AuditActor,
        context: RequestContext,
        request: &CreateFacilityWorkOrderRequest,
    ) -> Result<FacilityWorkOrderRecord> {
        ensure_work_order_management(scope)?;
        let actor_id = person_actor_id(actor)?;
        let employee = EmployeeOps::get_reference(pool, tenant_id, request.assigned_employee_id)
            .await?
            .filter(|employee| employee.employment_status == "active")
            .ok_or_else(|| anyhow!("The selected Facilities assignee is not an active employee"))?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Facilities work-order creation")?;
        let current = sqlx::query_as::<_, LockedFacilityRequest>(
            r#"
            SELECT reference, location_id, status, version
              FROM facility_service_requests
             WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL
             FOR UPDATE
            "#,
        )
        .bind(tenant_id)
        .bind(request.service_request_id)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to lock the Facilities service request")?
        .ok_or_else(|| anyhow!("The Facilities service request was not found"))?;
        if current.status != "open" {
            bail!("Only an open Facilities service request can receive a work order");
        }
        ensure_active_location(&mut transaction, tenant_id, current.location_id).await?;
        let reference =
            reserve_reference(&mut transaction, tenant_id, ReferenceKind::WorkOrder).await?;
        let work_order_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO facility_work_orders (
                id, tenant_id, reference, service_request_id, location_id,
                assigned_employee_id, title, instructions, target_date,
                created_by, updated_by
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$10)
            "#,
        )
        .bind(work_order_id)
        .bind(tenant_id)
        .bind(&reference)
        .bind(request.service_request_id)
        .bind(current.location_id)
        .bind(request.assigned_employee_id)
        .bind(trimmed_required(&request.title, "Work-order title")?)
        .bind(trimmed_optional(request.instructions.as_deref()))
        .bind(request.target_date)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(work_order_database_error)?;
        sqlx::query(
            "UPDATE facility_service_requests SET status='assigned', updated_by=$1, version=version+1 WHERE tenant_id=$2 AND id=$3",
        )
        .bind(actor_id)
        .bind(tenant_id)
        .bind(request.service_request_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to assign the Facilities service request")?;
        append_facilities_evidence(
            &mut transaction,
            FacilitiesEvidence {
                tenant_id,
                service_request_id: Some(request.service_request_id),
                work_order_id: Some(work_order_id),
                actor,
                context,
                event_type: "facilities.work_order.created",
                operation: "facilities.work_orders.create",
                target_kind: "facility_work_order",
                target_id: work_order_id,
                metadata: json!({
                    "reference": reference,
                    "request_reference": current.reference,
                    "assigned_employee_id": employee.id
                }),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Facilities work-order creation")?;
        Self::get_work_order(pool, tenant_id, work_order_id, scope)
            .await?
            .ok_or_else(|| anyhow!("The Facilities work order could not be reloaded"))
    }

    pub async fn start_work_order(
        pool: &PgPool,
        tenant_id: Uuid,
        work_order_id: Uuid,
        scope: FacilitiesWorkOrderScope,
        actor: AuditActor,
        context: RequestContext,
        request: &FacilityWorkOrderTransitionRequest,
    ) -> Result<Option<FacilityWorkOrderRecord>> {
        let actor_id = person_actor_id(actor)?;
        let visibility = work_order_visibility(pool, tenant_id, scope).await?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start the Facilities work-order transition")?;
        let Some(current) =
            lock_work_order(&mut transaction, tenant_id, work_order_id, visibility).await?
        else {
            return Ok(None);
        };
        ensure_version(
            current.version,
            request.expected_version,
            "Facilities work order",
        )?;
        if current.status != "assigned" {
            bail!("Only an assigned Facilities work order can be started");
        }
        sqlx::query(
            "UPDATE facility_work_orders SET status='in_progress', started_by=$1, started_at=NOW(), updated_by=$1, version=version+1 WHERE tenant_id=$2 AND id=$3",
        )
        .bind(actor_id)
        .bind(tenant_id)
        .bind(work_order_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to start the Facilities work order")?;
        append_facilities_evidence(
            &mut transaction,
            FacilitiesEvidence {
                tenant_id,
                service_request_id: Some(current.service_request_id),
                work_order_id: Some(work_order_id),
                actor,
                context,
                event_type: "facilities.work_order.started",
                operation: "facilities.work_orders.start",
                target_kind: "facility_work_order",
                target_id: work_order_id,
                metadata: json!({"reference": current.reference}),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Facilities work-order start")?;
        Self::get_work_order(pool, tenant_id, work_order_id, scope).await
    }

    pub async fn submit_completion(
        pool: &PgPool,
        tenant_id: Uuid,
        work_order_id: Uuid,
        scope: FacilitiesWorkOrderScope,
        actor: AuditActor,
        context: RequestContext,
        request: &SubmitFacilityCompletionRequest,
    ) -> Result<Option<FacilityWorkOrderRecord>> {
        let actor_id = person_actor_id(actor)?;
        let summary = trimmed_required(&request.summary, "Completion summary")?;
        let visibility = work_order_visibility(pool, tenant_id, scope).await?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Facilities completion submission")?;
        let Some(current) =
            lock_work_order(&mut transaction, tenant_id, work_order_id, visibility).await?
        else {
            return Ok(None);
        };
        ensure_version(
            current.version,
            request.expected_version,
            "Facilities work order",
        )?;
        if current.status != "in_progress" {
            bail!("Only an in-progress Facilities work order can be submitted for inspection");
        }
        sqlx::query(
            "INSERT INTO facility_work_order_completion_submissions (tenant_id, work_order_id, summary, submitted_by) VALUES ($1,$2,$3,$4)",
        )
        .bind(tenant_id)
        .bind(work_order_id)
        .bind(summary)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to retain the Facilities completion submission")?;
        sqlx::query(
            "UPDATE facility_work_orders SET status='ready_for_inspection', updated_by=$1, version=version+1 WHERE tenant_id=$2 AND id=$3",
        )
        .bind(actor_id)
        .bind(tenant_id)
        .bind(work_order_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to submit the Facilities work order")?;
        append_facilities_evidence(
            &mut transaction,
            FacilitiesEvidence {
                tenant_id,
                service_request_id: Some(current.service_request_id),
                work_order_id: Some(work_order_id),
                actor,
                context,
                event_type: "facilities.work_order.completion_submitted",
                operation: "facilities.work_orders.submit_completion",
                target_kind: "facility_work_order",
                target_id: work_order_id,
                metadata: json!({"reference": current.reference}),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Facilities completion submission")?;
        Self::get_work_order(pool, tenant_id, work_order_id, scope).await
    }

    pub async fn cancel_work_order(
        pool: &PgPool,
        tenant_id: Uuid,
        work_order_id: Uuid,
        scope: FacilitiesWorkOrderScope,
        actor: AuditActor,
        context: RequestContext,
        request: &FacilityTransitionRequest,
    ) -> Result<Option<FacilityWorkOrderRecord>> {
        ensure_work_order_management(scope)?;
        let actor_id = person_actor_id(actor)?;
        let reason = trimmed_required(&request.reason, "Cancellation reason")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Facilities work-order cancellation")?;
        let Some(current) = lock_work_order(
            &mut transaction,
            tenant_id,
            work_order_id,
            WorkOrderVisibility::Campus,
        )
        .await?
        else {
            return Ok(None);
        };
        ensure_version(
            current.version,
            request.expected_version,
            "Facilities work order",
        )?;
        if matches!(current.status.as_str(), "completed" | "cancelled") {
            bail!("A completed or cancelled Facilities work order cannot be cancelled");
        }
        sqlx::query(
            r#"
            UPDATE facility_work_orders
               SET status='cancelled', cancelled_by=$1, cancelled_at=NOW(),
                   cancellation_reason=$2, updated_by=$1, version=version+1
             WHERE tenant_id=$3 AND id=$4
            "#,
        )
        .bind(actor_id)
        .bind(reason)
        .bind(tenant_id)
        .bind(work_order_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to cancel the Facilities work order")?;
        sqlx::query(
            r#"
            UPDATE facility_service_requests
               SET status='cancelled', cancelled_by=$1, cancelled_at=NOW(),
                   cancellation_reason=$2, updated_by=$1, version=version+1
             WHERE tenant_id=$3 AND id=$4 AND status='assigned'
            "#,
        )
        .bind(actor_id)
        .bind(reason)
        .bind(tenant_id)
        .bind(current.service_request_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to cancel the linked Facilities service request")?;
        append_facilities_evidence(
            &mut transaction,
            FacilitiesEvidence {
                tenant_id,
                service_request_id: Some(current.service_request_id),
                work_order_id: Some(work_order_id),
                actor,
                context,
                event_type: "facilities.work_order.cancelled",
                operation: "facilities.work_orders.cancel",
                target_kind: "facility_work_order",
                target_id: work_order_id,
                metadata: json!({"reason": reason}),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Facilities work-order cancellation")?;
        Self::get_work_order(pool, tenant_id, work_order_id, scope).await
    }

    pub async fn inspect_work_order(
        pool: &PgPool,
        tenant_id: Uuid,
        work_order_id: Uuid,
        scope: FacilitiesWorkOrderScope,
        actor: AuditActor,
        context: RequestContext,
        request: &InspectFacilityWorkOrderRequest,
    ) -> Result<Option<FacilityWorkOrderRecord>> {
        ensure_work_order_management(scope)?;
        let actor_id = person_actor_id(actor)?;
        let notes = trimmed_required(&request.notes, "Inspection notes")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start the Facilities inspection")?;
        let Some(current) = lock_work_order(
            &mut transaction,
            tenant_id,
            work_order_id,
            WorkOrderVisibility::Campus,
        )
        .await?
        else {
            return Ok(None);
        };
        ensure_version(
            current.version,
            request.expected_version,
            "Facilities work order",
        )?;
        if current.status != "ready_for_inspection" {
            bail!("Only a Facilities work order awaiting inspection can be inspected");
        }
        let completion_summary = sqlx::query_scalar::<_, String>(
            r#"
            SELECT summary
              FROM facility_work_order_completion_submissions
             WHERE tenant_id=$1 AND work_order_id=$2
             ORDER BY created_at DESC, id DESC
             LIMIT 1
            "#,
        )
        .bind(tenant_id)
        .bind(work_order_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to read the Facilities completion submission")?;
        sqlx::query(
            "INSERT INTO facility_work_order_inspections (tenant_id, work_order_id, outcome, notes, inspected_by) VALUES ($1,$2,$3,$4,$5)",
        )
        .bind(tenant_id)
        .bind(work_order_id)
        .bind(request.outcome.as_str())
        .bind(notes)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to retain the Facilities inspection")?;
        let (next_status, event_type) = match request.outcome {
            crate::FacilityInspectionOutcome::Pass => {
                sqlx::query(
                    "UPDATE facility_work_orders SET status='completed', completed_by=$1, completed_at=NOW(), updated_by=$1, version=version+1 WHERE tenant_id=$2 AND id=$3",
                )
                .bind(actor_id)
                .bind(tenant_id)
                .bind(work_order_id)
                .execute(&mut *transaction)
                .await
                .context("Failed to complete the Facilities work order")?;
                sqlx::query(
                    r#"
                    UPDATE facility_service_requests
                       SET status='resolved', resolved_by=$1, resolved_at=NOW(),
                           resolution_summary=$2, updated_by=$1, version=version+1
                     WHERE tenant_id=$3 AND id=$4 AND status='assigned'
                    "#,
                )
                .bind(actor_id)
                .bind(&completion_summary)
                .bind(tenant_id)
                .bind(current.service_request_id)
                .execute(&mut *transaction)
                .await
                .context("Failed to resolve the Facilities service request")?;
                ("completed", "facilities.work_order.inspection_passed")
            }
            crate::FacilityInspectionOutcome::Fail => {
                sqlx::query(
                    "UPDATE facility_work_orders SET status='in_progress', updated_by=$1, version=version+1 WHERE tenant_id=$2 AND id=$3",
                )
                .bind(actor_id)
                .bind(tenant_id)
                .bind(work_order_id)
                .execute(&mut *transaction)
                .await
                .context("Failed to return the Facilities work order for correction")?;
                ("in_progress", "facilities.work_order.inspection_failed")
            }
        };
        append_facilities_evidence(
            &mut transaction,
            FacilitiesEvidence {
                tenant_id,
                service_request_id: Some(current.service_request_id),
                work_order_id: Some(work_order_id),
                actor,
                context,
                event_type,
                operation: "facilities.work_orders.inspect",
                target_kind: "facility_work_order",
                target_id: work_order_id,
                metadata: json!({
                    "outcome": request.outcome.as_str(),
                    "next_status": next_status
                }),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit the Facilities inspection")?;
        Self::get_work_order(pool, tenant_id, work_order_id, scope).await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkOrderVisibility {
    Campus,
    AssignedEmployee(Uuid),
}

impl WorkOrderVisibility {
    const fn assigned_employee_id(self) -> Option<Uuid> {
        match self {
            Self::Campus => None,
            Self::AssignedEmployee(employee_id) => Some(employee_id),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ReferenceKind {
    Request,
    WorkOrder,
}

struct FacilitiesEvidence<'a> {
    tenant_id: Uuid,
    service_request_id: Option<Uuid>,
    work_order_id: Option<Uuid>,
    actor: AuditActor,
    context: RequestContext,
    event_type: &'a str,
    operation: &'a str,
    target_kind: &'a str,
    target_id: Uuid,
    metadata: Value,
}

async fn work_order_visibility(
    pool: &PgPool,
    tenant_id: Uuid,
    scope: FacilitiesWorkOrderScope,
) -> Result<WorkOrderVisibility> {
    match scope {
        FacilitiesWorkOrderScope::Campus => Ok(WorkOrderVisibility::Campus),
        FacilitiesWorkOrderScope::AssignedAccount(account_id) => {
            let employee = EmployeeOps::active_reference_by_account(pool, tenant_id, account_id)
                .await?
                .ok_or_else(|| {
                    anyhow!("Facilities work-order access requires an active linked employee")
                })?;
            Ok(WorkOrderVisibility::AssignedEmployee(employee.id))
        }
        FacilitiesWorkOrderScope::Denied => {
            bail!("Facilities work-order access is outside your current assignment scope")
        }
    }
}

async fn lock_request(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    request_id: Uuid,
    scope: FacilitiesRequestScope,
) -> Result<Option<LockedFacilityRequest>> {
    sqlx::query_as::<_, LockedFacilityRequest>(
        r#"
        SELECT reference, location_id, status, version
          FROM facility_service_requests
         WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL
           AND ($3::UUID IS NULL OR reporter_user_id=$3)
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(request_id)
    .bind(scope.reporter_user_id())
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock the Facilities service request")
}

async fn lock_work_order(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    work_order_id: Uuid,
    visibility: WorkOrderVisibility,
) -> Result<Option<LockedFacilityWorkOrder>> {
    sqlx::query_as::<_, LockedFacilityWorkOrder>(
        r#"
        SELECT reference, service_request_id, status, version
          FROM facility_work_orders
         WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL
           AND ($3::UUID IS NULL OR assigned_employee_id=$3)
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(work_order_id)
    .bind(visibility.assigned_employee_id())
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock the Facilities work order")
}

async fn ensure_active_location(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    location_id: Uuid,
) -> Result<()> {
    let active = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM facility_locations WHERE tenant_id=$1 AND id=$2 AND status='active' AND deleted_at IS NULL)",
    )
    .bind(tenant_id)
    .bind(location_id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to check the Facilities location")?;
    if !active {
        bail!("The selected Facilities location is not active");
    }
    Ok(())
}

async fn reserve_reference(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    kind: ReferenceKind,
) -> Result<String> {
    let (prefix, sequence, padding) = match kind {
        ReferenceKind::Request => sqlx::query_as::<_, (String, i64, i16)>(
            r#"
                UPDATE facilities_numbering_policies
                   SET next_request_sequence=next_request_sequence+1, version=version+1
                 WHERE tenant_id=$1 AND deleted_at IS NULL
                 RETURNING request_prefix, next_request_sequence-1, padding
                "#,
        )
        .bind(tenant_id)
        .fetch_one(&mut **transaction)
        .await
        .context("Failed to reserve a Facilities request number")?,
        ReferenceKind::WorkOrder => sqlx::query_as::<_, (String, i64, i16)>(
            r#"
                UPDATE facilities_numbering_policies
                   SET next_work_order_sequence=next_work_order_sequence+1, version=version+1
                 WHERE tenant_id=$1 AND deleted_at IS NULL
                 RETURNING work_order_prefix, next_work_order_sequence-1, padding
                "#,
        )
        .bind(tenant_id)
        .fetch_one(&mut **transaction)
        .await
        .context("Failed to reserve a Facilities work-order number")?,
    };
    Ok(format!(
        "{prefix}{sequence:0width$}",
        width = padding as usize
    ))
}

async fn append_facilities_evidence(
    transaction: &mut Transaction<'_, Postgres>,
    evidence: FacilitiesEvidence<'_>,
) -> Result<()> {
    let actor_id = person_actor_id(evidence.actor)?;
    sqlx::query(
        r#"
        INSERT INTO facility_events (
            tenant_id, service_request_id, work_order_id, event_type, actor_id, metadata
        ) VALUES ($1,$2,$3,$4,$5,$6)
        "#,
    )
    .bind(evidence.tenant_id)
    .bind(evidence.service_request_id)
    .bind(evidence.work_order_id)
    .bind(evidence.event_type)
    .bind(actor_id)
    .bind(evidence.metadata.clone())
    .execute(&mut **transaction)
    .await
    .context("Failed to append Facilities lifecycle evidence")?;
    append_domain_audit(
        transaction,
        evidence.tenant_id,
        evidence.actor,
        evidence.context,
        evidence.operation,
        evidence.target_kind,
        evidence.target_id,
        evidence.metadata,
    )
    .await
}

#[expect(
    clippy::too_many_arguments,
    reason = "audit writes keep the complete domain target and request evidence explicit"
)]
async fn append_domain_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    context: RequestContext,
    operation: &str,
    target_kind: &str,
    target_id: Uuid,
    metadata: Value,
) -> Result<()> {
    append_audit(
        &mut **transaction,
        &NewAuditEvent::new(
            tenant_id,
            actor,
            operation,
            AuditOutcome::Succeeded,
            context,
        )
        .with_target(AuditTarget::new(target_kind, target_id.to_string()))
        .with_redacted_metadata(metadata.as_object().cloned().unwrap_or_else(Map::new)),
    )
    .await
    .map(|_| ())
    .context("Failed to append Facilities audit evidence")
}

async fn event_history(
    pool: &PgPool,
    tenant_id: Uuid,
    service_request_id: Option<Uuid>,
    work_order_id: Option<Uuid>,
) -> Result<Vec<FacilityEventResponse>> {
    sqlx::query_as::<_, FacilityEventRow>(
        r#"
        SELECT event.id, event.service_request_id, event.work_order_id,
               event.event_type, event.actor_id, account.full_name AS actor_name,
               event.metadata, event.created_at
          FROM facility_events AS event
          JOIN users AS account
            ON account.id=event.actor_id AND account.tenant_id=event.tenant_id
         WHERE event.tenant_id=$1
           AND ($2::UUID IS NULL OR event.service_request_id=$2)
           AND ($3::UUID IS NULL OR event.work_order_id=$3)
         ORDER BY event.created_at DESC, event.id DESC
         LIMIT 200
        "#,
    )
    .bind(tenant_id)
    .bind(service_request_id)
    .bind(work_order_id)
    .fetch_all(pool)
    .await
    .context("Failed to load Facilities lifecycle history")
    .map(|rows| rows.into_iter().map(event_response).collect())
}

async fn hydrate_work_order_summaries(
    pool: &PgPool,
    tenant_id: Uuid,
    rows: Vec<FacilityWorkOrderRow>,
) -> Result<Vec<FacilityWorkOrderSummary>> {
    let ids = rows
        .iter()
        .map(|row| row.assigned_employee_id)
        .collect::<Vec<_>>();
    let employees = EmployeeOps::references_by_ids(pool, tenant_id, &ids)
        .await?
        .into_iter()
        .map(|employee| (employee.id, employee))
        .collect::<HashMap<_, _>>();
    rows.into_iter()
        .map(|row| {
            let employee = employees
                .get(&row.assigned_employee_id)
                .ok_or_else(|| anyhow!("A Facilities work-order assignee is unavailable"))?;
            Ok(work_order_summary(row, employee))
        })
        .collect()
}

fn location_response(row: FacilityLocationRow) -> FacilityLocationResponse {
    FacilityLocationResponse {
        id: row.id,
        parent_id: row.parent_id,
        parent_name: row.parent_name,
        kind: row.kind,
        code: row.code,
        name: row.name,
        status: row.status,
        capacity: row.capacity,
        notes: row.notes,
        version: row.version,
        child_count: row.child_count,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn request_summary(row: FacilityRequestRow) -> FacilityServiceRequestSummary {
    FacilityServiceRequestSummary {
        id: row.id,
        reference: row.reference,
        location_id: row.location_id,
        location_name: row.location_name,
        reporter_user_id: row.reporter_user_id,
        reporter_name: row.reporter_name,
        priority: row.priority,
        summary: row.summary,
        status: row.status,
        version: row.version,
        work_order_id: row.work_order_id,
        work_order_reference: row.work_order_reference,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn request_record(
    row: FacilityRequestRow,
    history: Vec<FacilityEventResponse>,
) -> FacilityServiceRequestRecord {
    let description = row.description.clone();
    let resolution_summary = row.resolution_summary.clone();
    let resolved_at = row.resolved_at;
    let closure_reason = row.closure_reason.clone();
    let closed_at = row.closed_at;
    let cancellation_reason = row.cancellation_reason.clone();
    let cancelled_at = row.cancelled_at;
    FacilityServiceRequestRecord {
        request: request_summary(row),
        description,
        resolution_summary,
        resolved_at,
        closure_reason,
        closed_at,
        cancellation_reason,
        cancelled_at,
        history,
    }
}

fn work_order_summary(
    row: FacilityWorkOrderRow,
    employee: &EmployeeReference,
) -> FacilityWorkOrderSummary {
    FacilityWorkOrderSummary {
        id: row.id,
        reference: row.reference,
        service_request_id: row.service_request_id,
        service_request_reference: row.service_request_reference,
        location_id: row.location_id,
        location_name: row.location_name,
        assigned_employee_id: row.assigned_employee_id,
        assigned_employee_number: employee.employee_number.clone(),
        assigned_employee_name: employee.display_name.clone(),
        title: row.title,
        target_date: row.target_date,
        status: row.status,
        version: row.version,
        inspection_count: row.inspection_count,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn work_order_record(
    row: FacilityWorkOrderRow,
    employee: EmployeeReference,
    inspections: Vec<FacilityInspectionResponse>,
    history: Vec<FacilityEventResponse>,
) -> FacilityWorkOrderRecord {
    let instructions = row.instructions.clone();
    let started_at = row.started_at;
    let completion_summary = row.completion_summary.clone();
    let completion_submitted_at = row.completion_submitted_at;
    let completed_at = row.completed_at;
    let cancellation_reason = row.cancellation_reason.clone();
    let cancelled_at = row.cancelled_at;
    FacilityWorkOrderRecord {
        work_order: work_order_summary(row, &employee),
        instructions,
        started_at,
        completion_summary,
        completion_submitted_at,
        completed_at,
        cancellation_reason,
        cancelled_at,
        inspections,
        history,
    }
}

fn inspection_response(row: FacilityInspectionRow) -> FacilityInspectionResponse {
    FacilityInspectionResponse {
        id: row.id,
        outcome: row.outcome,
        notes: row.notes,
        inspected_by: row.inspected_by,
        inspector_name: row.inspector_name,
        created_at: row.created_at,
    }
}

fn event_response(row: FacilityEventRow) -> FacilityEventResponse {
    FacilityEventResponse {
        id: row.id,
        service_request_id: row.service_request_id,
        work_order_id: row.work_order_id,
        event_type: row.event_type,
        actor_id: row.actor_id,
        actor_name: row.actor_name,
        metadata: row.metadata,
        created_at: row.created_at,
    }
}

fn ensure_request_visibility(scope: FacilitiesRequestScope) -> Result<()> {
    if scope.is_denied() {
        bail!("Facilities request access is outside your current record scope");
    }
    Ok(())
}

fn ensure_request_management(scope: FacilitiesRequestScope) -> Result<()> {
    if !scope.is_campus() {
        bail!("Facilities request management requires campus scope");
    }
    Ok(())
}

fn ensure_work_order_management(scope: FacilitiesWorkOrderScope) -> Result<()> {
    if !scope.is_campus() {
        bail!("Facilities work-order management requires campus scope");
    }
    Ok(())
}

fn ensure_version(actual: i32, expected: i32, label: &str) -> Result<()> {
    if actual != expected {
        bail!("The {label} changed; reload before continuing");
    }
    Ok(())
}

fn validate_capacity(capacity: Option<i32>) -> Result<()> {
    if capacity.is_some_and(|value| value <= 0) {
        bail!("Location capacity must be greater than zero");
    }
    Ok(())
}

fn person_actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("Facilities requires a person actor"))
}

fn trimmed_required<'a>(value: &'a str, label: &str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{label} is required");
    }
    Ok(value)
}

fn trimmed_optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn search_pattern(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{value}%"))
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).max(1),
        per_page.unwrap_or(20).clamp(1, 100),
    )
}

fn location_database_error(error: sqlx::Error, duplicate_message: &str) -> anyhow::Error {
    if let sqlx::Error::Database(database) = &error {
        if database.constraint() == Some("idx_facility_locations_code") {
            return anyhow!(duplicate_message.to_string());
        }
        let message = database.message();
        if [
            "A facility location cannot be its own parent",
            "A site cannot have a parent location",
            "This facility location requires a parent",
            "The parent facility location was not found",
            "An archived facility location cannot be used as a parent",
            "The selected facility parent is not valid for this location type",
            "The facility hierarchy cannot contain a cycle",
        ]
        .contains(&message)
        {
            return anyhow!(message.to_string());
        }
    }
    anyhow!(error).context("Failed to persist the Facilities location")
}

fn work_order_database_error(error: sqlx::Error) -> anyhow::Error {
    if let sqlx::Error::Database(database) = &error
        && database.constraint() == Some("facility_work_orders_tenant_id_service_request_id_key")
    {
        return anyhow!("This Facilities request already has a work order");
    }
    anyhow!(error).context("Failed to create the Facilities work order")
}

const LOCATION_LIST: &str = r#"
    SELECT location.id, location.parent_id, parent.name AS parent_name,
           location.kind, location.code, location.name, location.status,
           location.capacity, location.notes, location.version,
           (SELECT COUNT(*) FROM facility_locations AS child
             WHERE child.tenant_id=location.tenant_id AND child.parent_id=location.id
               AND child.deleted_at IS NULL) AS child_count,
           location.created_at, location.updated_at
      FROM facility_locations AS location
      LEFT JOIN facility_locations AS parent
        ON parent.id=location.parent_id AND parent.tenant_id=location.tenant_id
     WHERE location.tenant_id=$1 AND location.deleted_at IS NULL
       AND ($2::UUID IS NULL OR location.parent_id=$2)
       AND ($3::TEXT IS NULL OR location.kind=$3)
       AND ($4::TEXT IS NULL OR location.status=$4)
       AND ($5::TEXT IS NULL OR location.code ILIKE $5 OR location.name ILIKE $5)
     ORDER BY
       CASE location.kind
         WHEN 'site' THEN 1 WHEN 'building' THEN 2 WHEN 'floor' THEN 3
         WHEN 'room' THEN 4 ELSE 5
       END,
       location.name, location.code
"#;

const LOCATION_BY_ID: &str = r#"
    SELECT location.id, location.parent_id, parent.name AS parent_name,
           location.kind, location.code, location.name, location.status,
           location.capacity, location.notes, location.version,
           (SELECT COUNT(*) FROM facility_locations AS child
             WHERE child.tenant_id=location.tenant_id AND child.parent_id=location.id
               AND child.deleted_at IS NULL) AS child_count,
           location.created_at, location.updated_at
      FROM facility_locations AS location
      LEFT JOIN facility_locations AS parent
        ON parent.id=location.parent_id AND parent.tenant_id=location.tenant_id
     WHERE location.tenant_id=$1 AND location.id=$2 AND location.deleted_at IS NULL
"#;

const REQUEST_LIST: &str = r#"
    SELECT service_request.id, service_request.reference, service_request.location_id,
           location.name AS location_name, service_request.reporter_user_id,
           reporter.full_name AS reporter_name, service_request.priority,
           service_request.summary, service_request.description, service_request.status,
           service_request.version, work_order.id AS work_order_id,
           work_order.reference AS work_order_reference,
           service_request.resolution_summary, service_request.resolved_at,
           service_request.closure_reason, service_request.closed_at,
           service_request.cancellation_reason, service_request.cancelled_at,
           service_request.created_at, service_request.updated_at
      FROM facility_service_requests AS service_request
      JOIN facility_locations AS location
        ON location.id=service_request.location_id AND location.tenant_id=service_request.tenant_id
      JOIN users AS reporter
        ON reporter.id=service_request.reporter_user_id AND reporter.tenant_id=service_request.tenant_id
      LEFT JOIN facility_work_orders AS work_order
        ON work_order.service_request_id=service_request.id
       AND work_order.tenant_id=service_request.tenant_id AND work_order.deleted_at IS NULL
     WHERE service_request.tenant_id=$1 AND service_request.deleted_at IS NULL
       AND ($2::TEXT IS NULL OR service_request.status=$2)
       AND ($3::TEXT IS NULL OR service_request.priority=$3)
       AND ($4::UUID IS NULL OR service_request.location_id=$4)
       AND ($5::TEXT IS NULL OR service_request.reference ILIKE $5
            OR service_request.summary ILIKE $5 OR location.name ILIKE $5)
       AND ($6::UUID IS NULL OR service_request.reporter_user_id=$6)
     ORDER BY
       CASE service_request.priority
         WHEN 'urgent' THEN 1 WHEN 'high' THEN 2 WHEN 'normal' THEN 3 ELSE 4
       END,
       service_request.created_at DESC, service_request.id DESC
     LIMIT $7 OFFSET $8
"#;

const REQUEST_COUNT: &str = r#"
    SELECT COUNT(*)
      FROM facility_service_requests AS service_request
      JOIN facility_locations AS location
        ON location.id=service_request.location_id AND location.tenant_id=service_request.tenant_id
     WHERE service_request.tenant_id=$1 AND service_request.deleted_at IS NULL
       AND ($2::TEXT IS NULL OR service_request.status=$2)
       AND ($3::TEXT IS NULL OR service_request.priority=$3)
       AND ($4::UUID IS NULL OR service_request.location_id=$4)
       AND ($5::TEXT IS NULL OR service_request.reference ILIKE $5
            OR service_request.summary ILIKE $5 OR location.name ILIKE $5)
       AND ($6::UUID IS NULL OR service_request.reporter_user_id=$6)
"#;

const REQUEST_BY_ID: &str = r#"
    SELECT service_request.id, service_request.reference, service_request.location_id,
           location.name AS location_name, service_request.reporter_user_id,
           reporter.full_name AS reporter_name, service_request.priority,
           service_request.summary, service_request.description, service_request.status,
           service_request.version, work_order.id AS work_order_id,
           work_order.reference AS work_order_reference,
           service_request.resolution_summary, service_request.resolved_at,
           service_request.closure_reason, service_request.closed_at,
           service_request.cancellation_reason, service_request.cancelled_at,
           service_request.created_at, service_request.updated_at
      FROM facility_service_requests AS service_request
      JOIN facility_locations AS location
        ON location.id=service_request.location_id AND location.tenant_id=service_request.tenant_id
      JOIN users AS reporter
        ON reporter.id=service_request.reporter_user_id AND reporter.tenant_id=service_request.tenant_id
      LEFT JOIN facility_work_orders AS work_order
        ON work_order.service_request_id=service_request.id
       AND work_order.tenant_id=service_request.tenant_id AND work_order.deleted_at IS NULL
     WHERE service_request.tenant_id=$1 AND service_request.id=$2
       AND service_request.deleted_at IS NULL
       AND ($3::UUID IS NULL OR service_request.reporter_user_id=$3)
"#;

const WORK_ORDER_LIST: &str = r#"
    SELECT work_order.id, work_order.reference, work_order.service_request_id,
           service_request.reference AS service_request_reference,
           work_order.location_id, location.name AS location_name,
           work_order.assigned_employee_id, work_order.title, work_order.instructions,
           work_order.target_date, work_order.status, work_order.version,
           (SELECT COUNT(*) FROM facility_work_order_inspections AS inspection
             WHERE inspection.tenant_id=work_order.tenant_id
               AND inspection.work_order_id=work_order.id) AS inspection_count,
           work_order.started_at, completion.summary AS completion_summary,
           completion.created_at AS completion_submitted_at,
           work_order.completed_at, work_order.cancellation_reason,
           work_order.cancelled_at, work_order.created_at, work_order.updated_at
      FROM facility_work_orders AS work_order
      JOIN facility_service_requests AS service_request
        ON service_request.id=work_order.service_request_id
       AND service_request.tenant_id=work_order.tenant_id
      JOIN facility_locations AS location
        ON location.id=work_order.location_id AND location.tenant_id=work_order.tenant_id
      LEFT JOIN LATERAL (
          SELECT submission.summary, submission.created_at
            FROM facility_work_order_completion_submissions AS submission
           WHERE submission.tenant_id=work_order.tenant_id
             AND submission.work_order_id=work_order.id
           ORDER BY submission.created_at DESC, submission.id DESC
           LIMIT 1
      ) AS completion ON TRUE
     WHERE work_order.tenant_id=$1 AND work_order.deleted_at IS NULL
       AND ($2::TEXT IS NULL OR work_order.status=$2)
       AND ($3::UUID IS NULL OR work_order.assigned_employee_id=$3)
       AND ($4::UUID IS NULL OR work_order.location_id=$4)
       AND ($5::TEXT IS NULL OR work_order.reference ILIKE $5
            OR work_order.title ILIKE $5 OR service_request.reference ILIKE $5
            OR location.name ILIKE $5)
       AND ($6::UUID IS NULL OR work_order.assigned_employee_id=$6)
     ORDER BY
       CASE work_order.status
         WHEN 'ready_for_inspection' THEN 1 WHEN 'in_progress' THEN 2
         WHEN 'assigned' THEN 3 WHEN 'completed' THEN 4 ELSE 5
       END,
       work_order.target_date NULLS LAST, work_order.created_at DESC
     LIMIT $7 OFFSET $8
"#;

const WORK_ORDER_COUNT: &str = r#"
    SELECT COUNT(*)
      FROM facility_work_orders AS work_order
      JOIN facility_service_requests AS service_request
        ON service_request.id=work_order.service_request_id
       AND service_request.tenant_id=work_order.tenant_id
      JOIN facility_locations AS location
        ON location.id=work_order.location_id AND location.tenant_id=work_order.tenant_id
     WHERE work_order.tenant_id=$1 AND work_order.deleted_at IS NULL
       AND ($2::TEXT IS NULL OR work_order.status=$2)
       AND ($3::UUID IS NULL OR work_order.assigned_employee_id=$3)
       AND ($4::UUID IS NULL OR work_order.location_id=$4)
       AND ($5::TEXT IS NULL OR work_order.reference ILIKE $5
            OR work_order.title ILIKE $5 OR service_request.reference ILIKE $5
            OR location.name ILIKE $5)
       AND ($6::UUID IS NULL OR work_order.assigned_employee_id=$6)
"#;

const WORK_ORDER_BY_ID: &str = r#"
    SELECT work_order.id, work_order.reference, work_order.service_request_id,
           service_request.reference AS service_request_reference,
           work_order.location_id, location.name AS location_name,
           work_order.assigned_employee_id, work_order.title, work_order.instructions,
           work_order.target_date, work_order.status, work_order.version,
           (SELECT COUNT(*) FROM facility_work_order_inspections AS inspection
             WHERE inspection.tenant_id=work_order.tenant_id
               AND inspection.work_order_id=work_order.id) AS inspection_count,
           work_order.started_at, completion.summary AS completion_summary,
           completion.created_at AS completion_submitted_at,
           work_order.completed_at, work_order.cancellation_reason,
           work_order.cancelled_at, work_order.created_at, work_order.updated_at
      FROM facility_work_orders AS work_order
      JOIN facility_service_requests AS service_request
        ON service_request.id=work_order.service_request_id
       AND service_request.tenant_id=work_order.tenant_id
      JOIN facility_locations AS location
        ON location.id=work_order.location_id AND location.tenant_id=work_order.tenant_id
      LEFT JOIN LATERAL (
          SELECT submission.summary, submission.created_at
            FROM facility_work_order_completion_submissions AS submission
           WHERE submission.tenant_id=work_order.tenant_id
             AND submission.work_order_id=work_order.id
           ORDER BY submission.created_at DESC, submission.id DESC
           LIMIT 1
      ) AS completion ON TRUE
     WHERE work_order.tenant_id=$1 AND work_order.id=$2
       AND work_order.deleted_at IS NULL
       AND ($3::UUID IS NULL OR work_order.assigned_employee_id=$3)
"#;

#[cfg(test)]
mod tests {
    use super::{
        FacilitiesRequestScope, FacilitiesWorkOrderScope, bounded_page, ensure_request_management,
        ensure_version, ensure_work_order_management, validate_capacity,
    };
    use uuid::Uuid;

    #[test]
    fn facilities_management_requires_campus_scope() {
        assert!(ensure_request_management(FacilitiesRequestScope::Campus).is_ok());
        assert!(
            ensure_request_management(FacilitiesRequestScope::SelfRecord(Uuid::new_v4())).is_err()
        );
        assert!(ensure_work_order_management(FacilitiesWorkOrderScope::Campus).is_ok());
        assert!(
            ensure_work_order_management(FacilitiesWorkOrderScope::AssignedAccount(Uuid::new_v4()))
                .is_err()
        );
    }

    #[test]
    fn optimistic_versions_and_capacity_are_strict() {
        assert!(ensure_version(3, 3, "record").is_ok());
        assert!(ensure_version(3, 2, "record").is_err());
        assert!(validate_capacity(None).is_ok());
        assert!(validate_capacity(Some(1)).is_ok());
        assert!(validate_capacity(Some(0)).is_err());
    }

    #[test]
    fn pagination_is_bounded() {
        assert_eq!(bounded_page(None, None), (1, 20));
        assert_eq!(bounded_page(Some(-2), Some(900)), (1, 100));
    }
}
