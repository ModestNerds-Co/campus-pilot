//! Transactional school transport workflows.
//!
//! Transport references identities owned by SIS and resources owned by Fleet.
//! A service run snapshots every mutable operational label required to preserve
//! an honest historical manifest.

use std::collections::HashMap;

use anyhow::{Context, Result, anyhow, bail};
use chrono::NaiveDate;
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_fleet::ops::{DriverOps, VehicleOps};
use cp_sis::ops::LearnerOps;
use serde_json::{Map, Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    CancelRunRequest, CreateRiderAssignmentRequest, CreateRouteRequest, CreateRunRequest,
    CreateStopRequest, EndRiderAssignmentRequest, ListRidersQuery, ListRoutesQuery, ListRunsQuery,
    ManifestEntryResponse, ManifestStatus, MarkManifestEntryRequest, ReferenceQuery,
    RiderAssignmentResponse, RouteRecordResponse, RouteStopResponse, RouteSummaryResponse,
    RunEventResponse, RunRecordResponse, RunStopResponse, RunSummaryResponse, RunTransitionRequest,
    TransportReferenceData, UpdateRouteRequest, UpdateStopRequest,
    models::{EventRow, ManifestRow, RiderRow, RouteRow, RunRow, RunStopRow, StopRow},
};

/// Tenant-scoped Transport operations.
pub struct TransportOps;

impl TransportOps {
    pub async fn reference_data(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &ReferenceQuery,
    ) -> Result<TransportReferenceData> {
        let search = query.search.as_deref();
        let learners = LearnerOps::transport_references(pool, tenant_id, search, 100).await?;
        let vehicles = VehicleOps::transport_references(pool, tenant_id, search, 100).await?;
        let drivers = DriverOps::transport_references(pool, tenant_id, search, 100).await?;
        let routes = active_routes(pool, tenant_id).await?;
        Ok(TransportReferenceData {
            learners,
            vehicles,
            drivers,
            routes,
        })
    }

    pub async fn list_routes(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &ListRoutesQuery,
    ) -> Result<(Vec<RouteSummaryResponse>, i64)> {
        let (page, per_page) = bounded_page(query.page, query.per_page);
        let offset = (page - 1) * per_page;
        let search = search_pattern(query.search.as_deref());
        let rows = sqlx::query_as::<_, RouteRow>(ROUTE_LIST)
            .bind(tenant_id)
            .bind(search.as_deref())
            .bind(query.status.map(|value| value.as_str()))
            .bind(query.direction.map(|value| value.as_str()))
            .bind(per_page)
            .bind(offset)
            .fetch_all(pool)
            .await
            .context("Failed to list Transport routes")?;
        let total = sqlx::query_scalar::<_, i64>(ROUTE_COUNT)
            .bind(tenant_id)
            .bind(search.as_deref())
            .bind(query.status.map(|value| value.as_str()))
            .bind(query.direction.map(|value| value.as_str()))
            .fetch_one(pool)
            .await
            .context("Failed to count Transport routes")?;
        Ok((rows.into_iter().map(route_summary).collect(), total))
    }

    pub async fn get_route(
        pool: &PgPool,
        tenant_id: Uuid,
        route_id: Uuid,
    ) -> Result<Option<RouteRecordResponse>> {
        let Some(row) = route_row_by_id(pool, tenant_id, route_id).await? else {
            return Ok(None);
        };
        let stops = route_stops(pool, tenant_id, route_id).await?;
        Ok(Some(route_record(row, stops)))
    }

    pub async fn create_route(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &CreateRouteRequest,
    ) -> Result<RouteRecordResponse> {
        let actor_id = person_actor_id(actor)?;
        let route_id = Uuid::new_v4();
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start route creation")?;
        sqlx::query(
            "INSERT INTO transport_routes (id,tenant_id,code,name,direction,notes,created_by,updated_by) VALUES ($1,$2,$3,$4,$5,$6,$7,$7)",
        )
        .bind(route_id)
        .bind(tenant_id)
        .bind(trimmed_required(&request.code, "Route code")?)
        .bind(trimmed_required(&request.name, "Route name")?)
        .bind(request.direction.as_str())
        .bind(trimmed_optional(request.notes.as_deref()))
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "A route with this code already exists"))?;
        append_domain_audit(
            &mut transaction,
            tenant_id,
            actor,
            context,
            "transport.routes.create",
            "transport_route",
            route_id,
            json!({"code": request.code.trim(), "direction": request.direction.as_str()}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit route creation")?;
        Self::get_route(pool, tenant_id, route_id)
            .await?
            .ok_or_else(|| anyhow!("The Transport route could not be reloaded"))
    }

    pub async fn update_route(
        pool: &PgPool,
        tenant_id: Uuid,
        route_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &UpdateRouteRequest,
    ) -> Result<Option<RouteRecordResponse>> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool.begin().await.context("Failed to start route update")?;
        let Some(current) = lock_route(&mut transaction, tenant_id, route_id).await? else {
            return Ok(None);
        };
        ensure_version(current.version, request.expected_version, "route")?;
        if current.active_rider_count > 0
            && (current.direction != request.direction.as_str()
                || (current.status == "active" && request.status.as_str() == "inactive"))
        {
            bail!("End active rider assignments before changing this route's direction or status");
        }
        sqlx::query(
            "UPDATE transport_routes SET code=$3,name=$4,direction=$5,status=$6,notes=$7,version=version+1,updated_by=$8 WHERE tenant_id=$1 AND id=$2",
        )
        .bind(tenant_id)
        .bind(route_id)
        .bind(trimmed_required(&request.code, "Route code")?)
        .bind(trimmed_required(&request.name, "Route name")?)
        .bind(request.direction.as_str())
        .bind(request.status.as_str())
        .bind(trimmed_optional(request.notes.as_deref()))
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "A route with this code already exists"))?;
        append_domain_audit(
            &mut transaction,
            tenant_id,
            actor,
            context,
            "transport.routes.update",
            "transport_route",
            route_id,
            json!({"version": request.expected_version, "status": request.status.as_str()}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit route update")?;
        Self::get_route(pool, tenant_id, route_id).await
    }

    pub async fn create_stop(
        pool: &PgPool,
        tenant_id: Uuid,
        route_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &CreateStopRequest,
    ) -> Result<Option<RouteRecordResponse>> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start stop creation")?;
        if lock_route(&mut transaction, tenant_id, route_id)
            .await?
            .is_none()
        {
            return Ok(None);
        }
        let stop_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO transport_route_stops (id,tenant_id,route_id,code,name,stop_order,planned_time,latitude,longitude,created_by,updated_by) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$10)",
        )
        .bind(stop_id)
        .bind(tenant_id)
        .bind(route_id)
        .bind(trimmed_required(&request.code, "Stop code")?)
        .bind(trimmed_required(&request.name, "Stop name")?)
        .bind(request.stop_order)
        .bind(request.planned_time)
        .bind(request.latitude)
        .bind(request.longitude)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "This route already has that stop code or order"))?;
        append_domain_audit(
            &mut transaction,
            tenant_id,
            actor,
            context,
            "transport.stops.create",
            "transport_route_stop",
            stop_id,
            json!({"route_id": route_id, "stop_order": request.stop_order}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit stop creation")?;
        Self::get_route(pool, tenant_id, route_id).await
    }

    pub async fn update_stop(
        pool: &PgPool,
        tenant_id: Uuid,
        route_id: Uuid,
        stop_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &UpdateStopRequest,
    ) -> Result<Option<RouteRecordResponse>> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool.begin().await.context("Failed to start stop update")?;
        let Some(version) = lock_stop(&mut transaction, tenant_id, route_id, stop_id).await? else {
            return Ok(None);
        };
        ensure_version(version, request.expected_version, "stop")?;
        ensure_stop_not_in_active_use(&mut transaction, tenant_id, stop_id).await?;
        sqlx::query(
            "UPDATE transport_route_stops SET code=$4,name=$5,stop_order=$6,planned_time=$7,latitude=$8,longitude=$9,version=version+1,updated_by=$10 WHERE tenant_id=$1 AND route_id=$2 AND id=$3",
        )
        .bind(tenant_id)
        .bind(route_id)
        .bind(stop_id)
        .bind(trimmed_required(&request.code, "Stop code")?)
        .bind(trimmed_required(&request.name, "Stop name")?)
        .bind(request.stop_order)
        .bind(request.planned_time)
        .bind(request.latitude)
        .bind(request.longitude)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "This route already has that stop code or order"))?;
        append_domain_audit(
            &mut transaction,
            tenant_id,
            actor,
            context,
            "transport.stops.update",
            "transport_route_stop",
            stop_id,
            json!({"route_id": route_id, "version": request.expected_version}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit stop update")?;
        Self::get_route(pool, tenant_id, route_id).await
    }

    pub async fn remove_stop(
        pool: &PgPool,
        tenant_id: Uuid,
        route_id: Uuid,
        stop_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<RouteRecordResponse>> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool.begin().await.context("Failed to start stop removal")?;
        let Some(version) = lock_stop(&mut transaction, tenant_id, route_id, stop_id).await? else {
            return Ok(None);
        };
        ensure_version(version, expected_version, "stop")?;
        ensure_stop_not_in_active_use(&mut transaction, tenant_id, stop_id).await?;
        sqlx::query(
            "UPDATE transport_route_stops SET deleted_at=NOW(),version=version+1,updated_by=$4 WHERE tenant_id=$1 AND route_id=$2 AND id=$3",
        )
        .bind(tenant_id)
        .bind(route_id)
        .bind(stop_id)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to remove Transport stop")?;
        append_domain_audit(
            &mut transaction,
            tenant_id,
            actor,
            context,
            "transport.stops.remove",
            "transport_route_stop",
            stop_id,
            json!({"route_id": route_id, "version": expected_version}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit stop removal")?;
        Self::get_route(pool, tenant_id, route_id).await
    }
}

const RUN_SELECT: &str = r#"
SELECT run.id,run.reference,run.route_id,run.route_code_snapshot AS route_code,
       run.route_name_snapshot AS route_name,run.direction_snapshot AS direction,
       run.service_date,run.vehicle_id,run.vehicle_registration_snapshot AS vehicle_registration,
       run.driver_id,run.driver_name_snapshot AS driver_name,run.capacity_snapshot AS capacity,
       run.status,
       COUNT(entry.id) FILTER (WHERE entry.status='expected')::BIGINT AS expected_count,
       COUNT(entry.id) FILTER (WHERE entry.status='boarded')::BIGINT AS boarded_count,
       COUNT(entry.id) FILTER (WHERE entry.status IN ('no_show','exception'))::BIGINT AS exception_count,
       run.version,run.created_at,run.updated_at
  FROM transport_service_runs run
  LEFT JOIN transport_manifest_entries entry ON entry.tenant_id=run.tenant_id AND entry.run_id=run.id AND entry.deleted_at IS NULL
"#;

const RUN_LIST: &str = r#"
SELECT run.id,run.reference,run.route_id,run.route_code_snapshot AS route_code,
       run.route_name_snapshot AS route_name,run.direction_snapshot AS direction,
       run.service_date,run.vehicle_id,run.vehicle_registration_snapshot AS vehicle_registration,
       run.driver_id,run.driver_name_snapshot AS driver_name,run.capacity_snapshot AS capacity,
       run.status,
       COUNT(entry.id) FILTER (WHERE entry.status='expected')::BIGINT AS expected_count,
       COUNT(entry.id) FILTER (WHERE entry.status='boarded')::BIGINT AS boarded_count,
       COUNT(entry.id) FILTER (WHERE entry.status IN ('no_show','exception'))::BIGINT AS exception_count,
       run.version,run.created_at,run.updated_at
  FROM transport_service_runs run
  LEFT JOIN transport_manifest_entries entry ON entry.tenant_id=run.tenant_id AND entry.run_id=run.id AND entry.deleted_at IS NULL
 WHERE run.tenant_id=$1 AND run.deleted_at IS NULL
   AND ($2::UUID IS NULL OR run.route_id=$2)
   AND ($3::TEXT IS NULL OR run.status=$3)
   AND ($4::DATE IS NULL OR run.service_date >= $4)
   AND ($5::DATE IS NULL OR run.service_date <= $5)
 GROUP BY run.id
 ORDER BY run.service_date DESC,run.reference DESC
 LIMIT $6 OFFSET $7
"#;

const RUN_COUNT: &str = r#"
SELECT COUNT(*) FROM transport_service_runs run
 WHERE run.tenant_id=$1 AND run.deleted_at IS NULL
   AND ($2::UUID IS NULL OR run.route_id=$2)
   AND ($3::TEXT IS NULL OR run.status=$3)
   AND ($4::DATE IS NULL OR run.service_date >= $4)
   AND ($5::DATE IS NULL OR run.service_date <= $5)
"#;

#[derive(Debug, Clone, Copy)]
enum RunTransition {
    StartBoarding,
    Depart,
    Complete,
    Cancel,
}

#[allow(
    clippy::too_many_arguments,
    reason = "the transition retains tenant, authority, audit, concurrency, and requested state"
)]
async fn transition_run(
    pool: &PgPool,
    tenant_id: Uuid,
    run_id: Uuid,
    actor: AuditActor,
    context: RequestContext,
    expected_version: i32,
    transition: RunTransition,
    reason: Option<&str>,
) -> Result<Option<RunRecordResponse>> {
    let actor_id = person_actor_id(actor)?;
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to start run transition")?;
    let Some((status, version)) = lock_run(&mut transaction, tenant_id, run_id).await? else {
        return Ok(None);
    };
    ensure_version(version, expected_version, "run")?;
    let (event_type, operation, next_status) = match transition {
        RunTransition::StartBoarding => {
            if status != "draft" {
                bail!("Only a draft Transport run can start boarding");
            }
            sqlx::query(
                "UPDATE transport_service_runs SET status='boarding',boarding_started_by=$3,boarding_started_at=NOW(),updated_by=$3,version=version+1 WHERE tenant_id=$1 AND id=$2",
            )
            .bind(tenant_id)
            .bind(run_id)
            .bind(actor_id)
            .execute(&mut *transaction)
            .await
            .context("Failed to start Transport boarding")?;
            (
                "transport.run.boarding_started",
                "transport.runs.start_boarding",
                "boarding",
            )
        }
        RunTransition::Depart => {
            if status != "boarding" {
                bail!("Only a boarding Transport run can depart");
            }
            let expected = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM transport_manifest_entries WHERE tenant_id=$1 AND run_id=$2 AND status='expected' AND deleted_at IS NULL",
            )
            .bind(tenant_id)
            .bind(run_id)
            .fetch_one(&mut *transaction)
            .await
            .context("Failed to validate Transport manifest completion")?;
            if expected > 0 {
                bail!("Mark every expected rider before the Transport run departs");
            }
            sqlx::query(
                "UPDATE transport_service_runs SET status='departed',departed_by=$3,departed_at=NOW(),updated_by=$3,version=version+1 WHERE tenant_id=$1 AND id=$2",
            )
            .bind(tenant_id)
            .bind(run_id)
            .bind(actor_id)
            .execute(&mut *transaction)
            .await
            .context("Failed to depart Transport run")?;
            (
                "transport.run.departed",
                "transport.runs.depart",
                "departed",
            )
        }
        RunTransition::Complete => {
            if status != "departed" {
                bail!("Only a departed Transport run can be completed");
            }
            sqlx::query(
                "UPDATE transport_service_runs SET status='completed',completed_by=$3,completed_at=NOW(),updated_by=$3,version=version+1 WHERE tenant_id=$1 AND id=$2",
            )
            .bind(tenant_id)
            .bind(run_id)
            .bind(actor_id)
            .execute(&mut *transaction)
            .await
            .context("Failed to complete Transport run")?;
            (
                "transport.run.completed",
                "transport.runs.complete",
                "completed",
            )
        }
        RunTransition::Cancel => {
            if !matches!(status.as_str(), "draft" | "boarding") {
                bail!("Only a draft or boarding Transport run can be cancelled");
            }
            let reason = reason.ok_or_else(|| anyhow!("A cancellation reason is required"))?;
            sqlx::query(
                "UPDATE transport_service_runs SET status='cancelled',cancelled_by=$3,cancelled_at=NOW(),cancellation_reason=$4,updated_by=$3,version=version+1 WHERE tenant_id=$1 AND id=$2",
            )
            .bind(tenant_id)
            .bind(run_id)
            .bind(actor_id)
            .bind(reason)
            .execute(&mut *transaction)
            .await
            .context("Failed to cancel Transport run")?;
            (
                "transport.run.cancelled",
                "transport.runs.cancel",
                "cancelled",
            )
        }
    };
    append_run_evidence(
        &mut transaction,
        RunEvidence {
            tenant_id,
            run_id,
            manifest_entry_id: None,
            actor,
            context,
            event_type,
            operation,
            metadata: json!({"previous_status": status, "status": next_status, "reason": reason}),
        },
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit run transition")?;
    TransportOps::get_run(pool, tenant_id, run_id).await
}

fn validate_manifest_mark(request: &MarkManifestEntryRequest) -> Result<()> {
    match request.status {
        ManifestStatus::Exception => {
            if request.exception_kind.is_none() {
                bail!("A Transport exception type is required");
            }
            trimmed_required(
                request.note.as_deref().unwrap_or_default(),
                "Transport exception note",
            )?;
        }
        ManifestStatus::Expected | ManifestStatus::Boarded | ManifestStatus::NoShow => {
            if request.exception_kind.is_some() {
                bail!("An exception type can be recorded only for a Transport exception");
            }
        }
    }
    Ok(())
}

async fn active_routes(pool: &PgPool, tenant_id: Uuid) -> Result<Vec<RouteRecordResponse>> {
    let rows = sqlx::query_as::<_, RouteRow>(
        r#"
        SELECT route.id,route.code,route.name,route.direction,route.status,route.notes,route.version,
               COUNT(DISTINCT stop.id)::BIGINT AS stop_count,
               COUNT(DISTINCT rider.id)::BIGINT AS active_rider_count,
               route.created_at,route.updated_at
          FROM transport_routes route
          LEFT JOIN transport_route_stops stop ON stop.tenant_id=route.tenant_id AND stop.route_id=route.id AND stop.deleted_at IS NULL
          LEFT JOIN transport_rider_assignments rider ON rider.tenant_id=route.tenant_id AND rider.route_id=route.id AND rider.status='active' AND rider.deleted_at IS NULL
         WHERE route.tenant_id=$1 AND route.status='active' AND route.deleted_at IS NULL
         GROUP BY route.id ORDER BY route.direction,route.name,route.code LIMIT 100
        "#,
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .context("Failed to list active Transport routes")?;
    let mut routes = Vec::with_capacity(rows.len());
    for row in rows {
        let stops = route_stops(pool, tenant_id, row.id).await?;
        routes.push(route_record(row, stops));
    }
    Ok(routes)
}

async fn route_row_by_id(
    pool: &PgPool,
    tenant_id: Uuid,
    route_id: Uuid,
) -> Result<Option<RouteRow>> {
    sqlx::query_as::<_, RouteRow>(ROUTE_BY_ID)
        .bind(tenant_id)
        .bind(route_id)
        .fetch_optional(pool)
        .await
        .context("Failed to load Transport route")
}

const ROUTE_BY_ID: &str = r#"
SELECT route.id,route.code,route.name,route.direction,route.status,route.notes,route.version,
       COUNT(DISTINCT stop.id)::BIGINT AS stop_count,
       COUNT(DISTINCT rider.id)::BIGINT AS active_rider_count,
       route.created_at,route.updated_at
  FROM transport_routes route
  LEFT JOIN transport_route_stops stop ON stop.tenant_id=route.tenant_id AND stop.route_id=route.id AND stop.deleted_at IS NULL
  LEFT JOIN transport_rider_assignments rider ON rider.tenant_id=route.tenant_id AND rider.route_id=route.id AND rider.status='active' AND rider.deleted_at IS NULL
 WHERE route.tenant_id=$1 AND route.id=$2 AND route.deleted_at IS NULL
 GROUP BY route.id
"#;

async fn lock_route(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    route_id: Uuid,
) -> Result<Option<RouteRow>> {
    let locked = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM transport_routes WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(route_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock Transport route")?;
    if locked.is_none() {
        return Ok(None);
    }
    sqlx::query_as::<_, RouteRow>(ROUTE_BY_ID)
        .bind(tenant_id)
        .bind(route_id)
        .fetch_optional(&mut **transaction)
        .await
        .context("Failed to reload locked Transport route")
}

async fn route_stops(pool: &PgPool, tenant_id: Uuid, route_id: Uuid) -> Result<Vec<StopRow>> {
    sqlx::query_as::<_, StopRow>(
        "SELECT id,code,name,stop_order,planned_time,latitude,longitude,version FROM transport_route_stops WHERE tenant_id=$1 AND route_id=$2 AND deleted_at IS NULL ORDER BY stop_order,code",
    )
    .bind(tenant_id)
    .bind(route_id)
    .fetch_all(pool)
    .await
    .context("Failed to load Transport route stops")
}

async fn lock_stop(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    route_id: Uuid,
    stop_id: Uuid,
) -> Result<Option<i32>> {
    sqlx::query_scalar::<_, i32>(
        "SELECT version FROM transport_route_stops WHERE tenant_id=$1 AND route_id=$2 AND id=$3 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(route_id)
    .bind(stop_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock Transport stop")
}

async fn ensure_stop_not_in_active_use(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    stop_id: Uuid,
) -> Result<()> {
    let in_use = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM transport_rider_assignments WHERE tenant_id=$1 AND status='active' AND deleted_at IS NULL AND (boarding_stop_id=$2 OR alighting_stop_id=$2))",
    )
    .bind(tenant_id)
    .bind(stop_id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to validate Transport stop use")?;
    if in_use {
        bail!("End active rider assignments before changing or removing this stop");
    }
    Ok(())
}

async fn learner_search_ids(
    pool: &PgPool,
    tenant_id: Uuid,
    search: Option<&str>,
) -> Result<Option<Vec<Uuid>>> {
    let Some(search) = search.filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    Ok(Some(
        LearnerOps::transport_references(pool, tenant_id, Some(search), 100)
            .await?
            .into_iter()
            .map(|learner| learner.id)
            .collect(),
    ))
}

async fn rider_rows_by_ids(pool: &PgPool, tenant_id: Uuid, ids: &[Uuid]) -> Result<Vec<RiderRow>> {
    sqlx::query_as::<_, RiderRow>(
        r#"
        SELECT assignment.id,assignment.learner_id,assignment.route_id,route.code AS route_code,route.name AS route_name,route.direction,
               assignment.boarding_stop_id,boarding.name AS boarding_stop_name,
               assignment.alighting_stop_id,alighting.name AS alighting_stop_name,
               assignment.effective_from,assignment.effective_until,assignment.status,assignment.version,assignment.updated_at
          FROM transport_rider_assignments assignment
          JOIN transport_routes route ON route.id=assignment.route_id AND route.tenant_id=assignment.tenant_id
          JOIN transport_route_stops boarding ON boarding.id=assignment.boarding_stop_id AND boarding.tenant_id=assignment.tenant_id
          JOIN transport_route_stops alighting ON alighting.id=assignment.alighting_stop_id AND alighting.tenant_id=assignment.tenant_id
         WHERE assignment.tenant_id=$1 AND assignment.id=ANY($2) AND assignment.deleted_at IS NULL
         ORDER BY assignment.created_at,assignment.id
        "#,
    )
    .bind(tenant_id)
    .bind(ids)
    .fetch_all(pool)
    .await
    .context("Failed to reload Transport rider assignments")
}

async fn hydrate_riders(
    pool: &PgPool,
    tenant_id: Uuid,
    rows: Vec<RiderRow>,
) -> Result<Vec<RiderAssignmentResponse>> {
    let learner_ids = rows.iter().map(|row| row.learner_id).collect::<Vec<_>>();
    let learners = LearnerOps::transport_references_by_ids(pool, tenant_id, &learner_ids)
        .await?
        .into_iter()
        .map(|learner| (learner.id, learner))
        .collect::<HashMap<_, _>>();
    rows.into_iter()
        .map(|row| {
            let learner = learners.get(&row.learner_id).ok_or_else(|| {
                anyhow!("A Transport rider assignment references an unavailable SIS learner")
            })?;
            Ok(RiderAssignmentResponse {
                id: row.id,
                learner_id: row.learner_id,
                learner_number: learner.learner_number.clone(),
                learner_name: learner.display_name.clone(),
                route_id: row.route_id,
                route_code: row.route_code,
                route_name: row.route_name,
                direction: row.direction,
                boarding_stop_id: row.boarding_stop_id,
                boarding_stop_name: row.boarding_stop_name,
                alighting_stop_id: row.alighting_stop_id,
                alighting_stop_name: row.alighting_stop_name,
                effective_from: row.effective_from,
                effective_until: row.effective_until,
                status: row.status,
                version: row.version,
                updated_at: row.updated_at,
            })
        })
        .collect()
}

async fn run_row_by_id(pool: &PgPool, tenant_id: Uuid, run_id: Uuid) -> Result<Option<RunRow>> {
    let query = format!(
        "{RUN_SELECT} WHERE run.tenant_id=$1 AND run.id=$2 AND run.deleted_at IS NULL GROUP BY run.id"
    );
    sqlx::query_as::<_, RunRow>(&query)
        .bind(tenant_id)
        .bind(run_id)
        .fetch_optional(pool)
        .await
        .context("Failed to load Transport run")
}

async fn lock_run(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
) -> Result<Option<(String, i32)>> {
    sqlx::query_as::<_, (String, i32)>(
        "SELECT status,version FROM transport_service_runs WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(run_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock Transport run")
}

async fn reserve_run_reference(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
) -> Result<String> {
    let (prefix, padding, sequence) = sqlx::query_as::<_, (String, i16, i64)>(
        "SELECT run_prefix,padding,next_run_sequence FROM transport_numbering_policies WHERE tenant_id=$1 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(tenant_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock Transport numbering policy")?
    .ok_or_else(|| anyhow!("Transport numbering is not configured for this campus"))?;
    sqlx::query(
        "UPDATE transport_numbering_policies SET next_run_sequence=next_run_sequence+1,version=version+1 WHERE tenant_id=$1",
    )
    .bind(tenant_id)
    .execute(&mut **transaction)
    .await
    .context("Failed to advance Transport run sequence")?;
    let width = usize::try_from(padding).context("Invalid Transport sequence padding")?;
    Ok(format!("{prefix}{sequence:0width$}"))
}

fn route_summary(row: RouteRow) -> RouteSummaryResponse {
    RouteSummaryResponse {
        id: row.id,
        code: row.code,
        name: row.name,
        direction: row.direction,
        status: row.status,
        notes: row.notes,
        version: row.version,
        stop_count: row.stop_count,
        active_rider_count: row.active_rider_count,
        updated_at: row.updated_at,
    }
}

fn route_record(row: RouteRow, stops: Vec<StopRow>) -> RouteRecordResponse {
    let created_at = row.created_at;
    RouteRecordResponse {
        route: route_summary(row),
        stops: stops.into_iter().map(stop_response).collect(),
        created_at,
    }
}

fn stop_response(row: StopRow) -> RouteStopResponse {
    RouteStopResponse {
        id: row.id,
        code: row.code,
        name: row.name,
        stop_order: row.stop_order,
        planned_time: row.planned_time,
        latitude: row.latitude,
        longitude: row.longitude,
        version: row.version,
    }
}

fn run_summary(row: RunRow) -> RunSummaryResponse {
    RunSummaryResponse {
        id: row.id,
        reference: row.reference,
        route_id: row.route_id,
        route_code: row.route_code,
        route_name: row.route_name,
        direction: row.direction,
        service_date: row.service_date,
        vehicle_id: row.vehicle_id,
        vehicle_registration: row.vehicle_registration,
        driver_id: row.driver_id,
        driver_name: row.driver_name,
        capacity: row.capacity,
        status: row.status,
        expected_count: row.expected_count,
        boarded_count: row.boarded_count,
        exception_count: row.exception_count,
        version: row.version,
        updated_at: row.updated_at,
    }
}

fn run_stop_response(row: RunStopRow) -> RunStopResponse {
    RunStopResponse {
        id: row.id,
        source_stop_id: row.source_stop_id,
        code: row.code,
        name: row.name,
        stop_order: row.stop_order,
        planned_time: row.planned_time,
    }
}

fn manifest_response(row: ManifestRow) -> ManifestEntryResponse {
    ManifestEntryResponse {
        id: row.id,
        learner_id: row.learner_id,
        learner_number: row.learner_number,
        learner_name: row.learner_name,
        boarding_run_stop_id: row.boarding_run_stop_id,
        boarding_stop_name: row.boarding_stop_name,
        alighting_run_stop_id: row.alighting_run_stop_id,
        alighting_stop_name: row.alighting_stop_name,
        status: row.status,
        exception_kind: row.exception_kind,
        note: row.note,
        marked_at: row.marked_at,
        version: row.version,
    }
}

fn event_response(row: EventRow) -> RunEventResponse {
    RunEventResponse {
        id: row.id,
        event_type: row.event_type,
        manifest_entry_id: row.manifest_entry_id,
        actor_name: row.actor_name,
        metadata: row.metadata,
        created_at: row.created_at,
    }
}

struct RunEvidence<'a> {
    tenant_id: Uuid,
    run_id: Uuid,
    manifest_entry_id: Option<Uuid>,
    actor: AuditActor,
    context: RequestContext,
    event_type: &'a str,
    operation: &'a str,
    metadata: Value,
}

async fn append_run_evidence(
    transaction: &mut Transaction<'_, Postgres>,
    evidence: RunEvidence<'_>,
) -> Result<()> {
    let actor_id = person_actor_id(evidence.actor)?;
    sqlx::query(
        "INSERT INTO transport_run_events (tenant_id,run_id,manifest_entry_id,event_type,actor_id,metadata) VALUES ($1,$2,$3,$4,$5,$6)",
    )
    .bind(evidence.tenant_id)
    .bind(evidence.run_id)
    .bind(evidence.manifest_entry_id)
    .bind(evidence.event_type)
    .bind(actor_id)
    .bind(evidence.metadata.clone())
    .execute(&mut **transaction)
    .await
    .context("Failed to append Transport lifecycle evidence")?;
    append_domain_audit(
        transaction,
        evidence.tenant_id,
        evidence.actor,
        evidence.context,
        evidence.operation,
        "transport_run",
        evidence.run_id,
        evidence.metadata,
    )
    .await
}

#[allow(
    clippy::too_many_arguments,
    reason = "audit evidence keeps the actor, context, target, operation, and metadata explicit"
)]
async fn append_domain_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    context: RequestContext,
    operation: &str,
    target_type: &str,
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
        .with_target(AuditTarget::new(target_type, target_id.to_string()))
        .with_redacted_metadata(metadata.as_object().cloned().unwrap_or_else(Map::new)),
    )
    .await
    .context("Failed to append Transport audit evidence")?;
    Ok(())
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).clamp(1, 1_000_000),
        per_page.unwrap_or(20).clamp(1, 100),
    )
}

fn ensure_version(actual: i32, expected: i32, label: &str) -> Result<()> {
    if actual != expected {
        bail!("The Transport {label} changed; reload before continuing");
    }
    Ok(())
}

fn person_actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("Transport writes require an authenticated campus user"))
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

fn database_error(error: sqlx::Error, duplicate_message: &str) -> anyhow::Error {
    if error
        .as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .is_some_and(|code| code == "23505")
    {
        anyhow!(duplicate_message.to_string())
    } else {
        anyhow!(error).context("Transport persistence failed")
    }
}

const ROUTE_LIST: &str = r#"
SELECT route.id,route.code,route.name,route.direction,route.status,route.notes,route.version,
       COUNT(DISTINCT stop.id)::BIGINT AS stop_count,
       COUNT(DISTINCT rider.id)::BIGINT AS active_rider_count,
       route.created_at,route.updated_at
  FROM transport_routes route
  LEFT JOIN transport_route_stops stop ON stop.tenant_id=route.tenant_id AND stop.route_id=route.id AND stop.deleted_at IS NULL
  LEFT JOIN transport_rider_assignments rider ON rider.tenant_id=route.tenant_id AND rider.route_id=route.id AND rider.status='active' AND rider.deleted_at IS NULL
 WHERE route.tenant_id=$1 AND route.deleted_at IS NULL
   AND ($2::TEXT IS NULL OR route.code ILIKE $2 OR route.name ILIKE $2)
   AND ($3::TEXT IS NULL OR route.status=$3)
   AND ($4::TEXT IS NULL OR route.direction=$4)
 GROUP BY route.id
 ORDER BY route.status,route.direction,route.name,route.code
 LIMIT $5 OFFSET $6
"#;

const ROUTE_COUNT: &str = r#"
SELECT COUNT(*) FROM transport_routes route
 WHERE route.tenant_id=$1 AND route.deleted_at IS NULL
   AND ($2::TEXT IS NULL OR route.code ILIKE $2 OR route.name ILIKE $2)
   AND ($3::TEXT IS NULL OR route.status=$3)
   AND ($4::TEXT IS NULL OR route.direction=$4)
"#;

impl TransportOps {
    pub async fn list_riders(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &ListRidersQuery,
    ) -> Result<(Vec<RiderAssignmentResponse>, i64)> {
        let (page, per_page) = bounded_page(query.page, query.per_page);
        let offset = (page - 1) * per_page;
        let learner_ids = learner_search_ids(pool, tenant_id, query.search.as_deref()).await?;
        let rows = sqlx::query_as::<_, RiderRow>(RIDER_LIST)
            .bind(tenant_id)
            .bind(query.route_id)
            .bind(query.status.map(|value| value.as_str()))
            .bind(query.on_date)
            .bind(learner_ids.as_deref())
            .bind(per_page)
            .bind(offset)
            .fetch_all(pool)
            .await
            .context("Failed to list Transport rider assignments")?;
        let total = sqlx::query_scalar::<_, i64>(RIDER_COUNT)
            .bind(tenant_id)
            .bind(query.route_id)
            .bind(query.status.map(|value| value.as_str()))
            .bind(query.on_date)
            .bind(learner_ids.as_deref())
            .fetch_one(pool)
            .await
            .context("Failed to count Transport rider assignments")?;
        Ok((hydrate_riders(pool, tenant_id, rows).await?, total))
    }

    pub async fn create_rider_assignment(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &CreateRiderAssignmentRequest,
    ) -> Result<RiderAssignmentResponse> {
        if request
            .effective_until
            .is_some_and(|until| until < request.effective_from)
        {
            bail!("The rider assignment end date cannot be before its start date");
        }
        let learner =
            LearnerOps::transport_references_by_ids(pool, tenant_id, &[request.learner_id])
                .await?
                .into_iter()
                .next()
                .filter(|value| value.status == "active")
                .ok_or_else(|| anyhow!("The selected learner is not active in SIS"))?;
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start rider assignment")?;
        let route = lock_route(&mut transaction, tenant_id, request.route_id)
            .await?
            .ok_or_else(|| anyhow!("The selected Transport route does not exist"))?;
        if route.status != "active" {
            bail!("The selected Transport route is not active");
        }
        let stops = sqlx::query_as::<_, StopRow>(
            "SELECT id,code,name,stop_order,planned_time,latitude,longitude,version FROM transport_route_stops WHERE tenant_id=$1 AND route_id=$2 AND id=ANY($3) AND deleted_at IS NULL FOR SHARE",
        )
        .bind(tenant_id)
        .bind(request.route_id)
        .bind(vec![request.boarding_stop_id, request.alighting_stop_id])
        .fetch_all(&mut *transaction)
        .await
        .context("Failed to validate Transport rider stops")?;
        if stops.len() != 2 {
            bail!("Both rider stops must belong to the selected route");
        }
        let boarding_order = stops
            .iter()
            .find(|stop| stop.id == request.boarding_stop_id)
            .map(|stop| stop.stop_order)
            .ok_or_else(|| anyhow!("The selected boarding stop is unavailable"))?;
        let alighting_order = stops
            .iter()
            .find(|stop| stop.id == request.alighting_stop_id)
            .map(|stop| stop.stop_order)
            .ok_or_else(|| anyhow!("The selected alighting stop is unavailable"))?;
        if boarding_order >= alighting_order {
            bail!("The boarding stop must come before the alighting stop on this route");
        }
        let overlap = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM transport_rider_assignments assignment
                  JOIN transport_routes route ON route.id=assignment.route_id AND route.tenant_id=assignment.tenant_id
                 WHERE assignment.tenant_id=$1 AND assignment.learner_id=$2
                   AND assignment.status='active' AND assignment.deleted_at IS NULL
                   AND route.direction=$3
                   AND assignment.effective_from <= COALESCE($5::DATE,'infinity'::DATE)
                   AND COALESCE(assignment.effective_until,'infinity'::DATE) >= $4
            )
            "#,
        )
        .bind(tenant_id)
        .bind(request.learner_id)
        .bind(&route.direction)
        .bind(request.effective_from)
        .bind(request.effective_until)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to check rider assignment dates")?;
        if overlap {
            bail!("This learner already has an overlapping route for the same direction");
        }
        let assignment_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO transport_rider_assignments (id,tenant_id,learner_id,route_id,boarding_stop_id,alighting_stop_id,effective_from,effective_until,created_by,updated_by) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$9)",
        )
        .bind(assignment_id)
        .bind(tenant_id)
        .bind(request.learner_id)
        .bind(request.route_id)
        .bind(request.boarding_stop_id)
        .bind(request.alighting_stop_id)
        .bind(request.effective_from)
        .bind(request.effective_until)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to create Transport rider assignment")?;
        append_domain_audit(
            &mut transaction,
            tenant_id,
            actor,
            context,
            "transport.riders.assign",
            "transport_rider_assignment",
            assignment_id,
            json!({
                "learner_number": learner.learner_number,
                "route_code": route.code,
                "effective_from": request.effective_from,
                "effective_until": request.effective_until
            }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit rider assignment")?;
        let rows = rider_rows_by_ids(pool, tenant_id, &[assignment_id]).await?;
        hydrate_riders(pool, tenant_id, rows)
            .await?
            .pop()
            .ok_or_else(|| anyhow!("The Transport rider assignment could not be reloaded"))
    }

    pub async fn end_rider_assignment(
        pool: &PgPool,
        tenant_id: Uuid,
        assignment_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &EndRiderAssignmentRequest,
    ) -> Result<Option<RiderAssignmentResponse>> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool.begin().await.context("Failed to start rider update")?;
        let current = sqlx::query_as::<_, (NaiveDate, String, i32)>(
            "SELECT effective_from,status,version FROM transport_rider_assignments WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL FOR UPDATE",
        )
        .bind(tenant_id)
        .bind(assignment_id)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to lock Transport rider assignment")?;
        let Some((effective_from, status, version)) = current else {
            return Ok(None);
        };
        ensure_version(version, request.expected_version, "rider assignment")?;
        if status != "active" {
            bail!("Only an active rider assignment can be ended");
        }
        if request.effective_until < effective_from {
            bail!("The rider assignment end date cannot be before its start date");
        }
        sqlx::query(
            "UPDATE transport_rider_assignments SET effective_until=$3,status='ended',ended_by=$4,ended_at=NOW(),end_reason=$5,updated_by=$4,version=version+1 WHERE tenant_id=$1 AND id=$2",
        )
        .bind(tenant_id)
        .bind(assignment_id)
        .bind(request.effective_until)
        .bind(actor_id)
        .bind(trimmed_required(&request.reason, "End reason")?)
        .execute(&mut *transaction)
        .await
        .context("Failed to end Transport rider assignment")?;
        append_domain_audit(
            &mut transaction,
            tenant_id,
            actor,
            context,
            "transport.riders.end",
            "transport_rider_assignment",
            assignment_id,
            json!({"effective_until": request.effective_until, "reason": request.reason.trim()}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit rider update")?;
        let rows = rider_rows_by_ids(pool, tenant_id, &[assignment_id]).await?;
        Ok(hydrate_riders(pool, tenant_id, rows).await?.pop())
    }
}

const RIDER_LIST: &str = r#"
SELECT assignment.id,assignment.learner_id,assignment.route_id,route.code AS route_code,route.name AS route_name,route.direction,
       assignment.boarding_stop_id,boarding.name AS boarding_stop_name,
       assignment.alighting_stop_id,alighting.name AS alighting_stop_name,
       assignment.effective_from,assignment.effective_until,assignment.status,assignment.version,assignment.updated_at
  FROM transport_rider_assignments assignment
  JOIN transport_routes route ON route.id=assignment.route_id AND route.tenant_id=assignment.tenant_id
  JOIN transport_route_stops boarding ON boarding.id=assignment.boarding_stop_id AND boarding.tenant_id=assignment.tenant_id
  JOIN transport_route_stops alighting ON alighting.id=assignment.alighting_stop_id AND alighting.tenant_id=assignment.tenant_id
 WHERE assignment.tenant_id=$1 AND assignment.deleted_at IS NULL
   AND ($2::UUID IS NULL OR assignment.route_id=$2)
   AND ($3::TEXT IS NULL OR assignment.status=$3)
   AND ($4::DATE IS NULL OR assignment.effective_from <= $4 AND COALESCE(assignment.effective_until,'infinity'::DATE) >= $4)
   AND ($5::UUID[] IS NULL OR assignment.learner_id=ANY($5))
 ORDER BY assignment.status,route.direction,route.name,assignment.effective_from DESC
 LIMIT $6 OFFSET $7
"#;

const RIDER_COUNT: &str = r#"
SELECT COUNT(*) FROM transport_rider_assignments assignment
 WHERE assignment.tenant_id=$1 AND assignment.deleted_at IS NULL
   AND ($2::UUID IS NULL OR assignment.route_id=$2)
   AND ($3::TEXT IS NULL OR assignment.status=$3)
   AND ($4::DATE IS NULL OR assignment.effective_from <= $4 AND COALESCE(assignment.effective_until,'infinity'::DATE) >= $4)
   AND ($5::UUID[] IS NULL OR assignment.learner_id=ANY($5))
"#;

impl TransportOps {
    pub async fn list_runs(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &ListRunsQuery,
    ) -> Result<(Vec<RunSummaryResponse>, i64)> {
        if query
            .date_to
            .zip(query.date_from)
            .is_some_and(|(to, from)| to < from)
        {
            bail!("The Transport run end date cannot be before its start date");
        }
        let (page, per_page) = bounded_page(query.page, query.per_page);
        let rows = sqlx::query_as::<_, RunRow>(RUN_LIST)
            .bind(tenant_id)
            .bind(query.route_id)
            .bind(query.status.map(|value| value.as_str()))
            .bind(query.date_from)
            .bind(query.date_to)
            .bind(per_page)
            .bind((page - 1) * per_page)
            .fetch_all(pool)
            .await
            .context("Failed to list Transport runs")?;
        let total = sqlx::query_scalar::<_, i64>(RUN_COUNT)
            .bind(tenant_id)
            .bind(query.route_id)
            .bind(query.status.map(|value| value.as_str()))
            .bind(query.date_from)
            .bind(query.date_to)
            .fetch_one(pool)
            .await
            .context("Failed to count Transport runs")?;
        Ok((rows.into_iter().map(run_summary).collect(), total))
    }

    pub async fn get_run(
        pool: &PgPool,
        tenant_id: Uuid,
        run_id: Uuid,
    ) -> Result<Option<RunRecordResponse>> {
        let Some(row) = run_row_by_id(pool, tenant_id, run_id).await? else {
            return Ok(None);
        };
        let stops = sqlx::query_as::<_, RunStopRow>(
            "SELECT id,source_stop_id,code_snapshot AS code,name_snapshot AS name,stop_order,planned_time_snapshot AS planned_time FROM transport_run_stops WHERE tenant_id=$1 AND run_id=$2 ORDER BY stop_order",
        )
        .bind(tenant_id)
        .bind(run_id)
        .fetch_all(pool)
        .await
        .context("Failed to load Transport run stops")?
        .into_iter()
        .map(run_stop_response)
        .collect();
        let manifest = sqlx::query_as::<_, ManifestRow>(
            r#"
            SELECT entry.id,entry.learner_id,entry.learner_number_snapshot AS learner_number,
                   entry.learner_name_snapshot AS learner_name,
                   entry.boarding_run_stop_id,boarding.name_snapshot AS boarding_stop_name,
                   entry.alighting_run_stop_id,alighting.name_snapshot AS alighting_stop_name,
                   entry.status,entry.exception_kind,entry.note,entry.marked_at,entry.version
              FROM transport_manifest_entries entry
              JOIN transport_run_stops boarding ON boarding.id=entry.boarding_run_stop_id AND boarding.tenant_id=entry.tenant_id
              JOIN transport_run_stops alighting ON alighting.id=entry.alighting_run_stop_id AND alighting.tenant_id=entry.tenant_id
             WHERE entry.tenant_id=$1 AND entry.run_id=$2 AND entry.deleted_at IS NULL
             ORDER BY boarding.stop_order,entry.learner_name_snapshot,entry.learner_number_snapshot
            "#,
        )
        .bind(tenant_id)
        .bind(run_id)
        .fetch_all(pool)
        .await
        .context("Failed to load Transport manifest")?
        .into_iter()
        .map(manifest_response)
        .collect();
        let history = sqlx::query_as::<_, EventRow>(
            r#"
            SELECT event.id,event.event_type,event.manifest_entry_id,account.full_name AS actor_name,event.metadata,event.created_at
              FROM transport_run_events event
              JOIN users account ON account.id=event.actor_id AND account.tenant_id=event.tenant_id
             WHERE event.tenant_id=$1 AND event.run_id=$2
             ORDER BY event.created_at,event.id
            "#,
        )
        .bind(tenant_id)
        .bind(run_id)
        .fetch_all(pool)
        .await
        .context("Failed to load Transport run history")?
        .into_iter()
        .map(event_response)
        .collect();
        let created_at = row.created_at;
        Ok(Some(RunRecordResponse {
            run: run_summary(row),
            stops,
            manifest,
            history,
            created_at,
        }))
    }

    pub async fn create_run(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &CreateRunRequest,
    ) -> Result<RunRecordResponse> {
        let vehicle = VehicleOps::transport_reference_by_id(pool, tenant_id, request.vehicle_id)
            .await?
            .filter(|value| value.status == "active")
            .ok_or_else(|| anyhow!("The selected Fleet vehicle is not active"))?;
        let driver = DriverOps::transport_reference_by_id(pool, tenant_id, request.driver_id)
            .await?
            .filter(|value| value.status == "active")
            .ok_or_else(|| anyhow!("The selected Fleet driver is not active"))?;
        if driver
            .license_expiry
            .is_some_and(|expiry| expiry < request.service_date)
        {
            bail!("The selected driver's licence expires before the service date");
        }
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Transport run")?;
        let route = lock_route(&mut transaction, tenant_id, request.route_id)
            .await?
            .ok_or_else(|| anyhow!("The selected Transport route does not exist"))?;
        if route.status != "active" {
            bail!("The selected Transport route is not active");
        }
        let stops = sqlx::query_as::<_, StopRow>(
            "SELECT id,code,name,stop_order,planned_time,latitude,longitude,version FROM transport_route_stops WHERE tenant_id=$1 AND route_id=$2 AND deleted_at IS NULL ORDER BY stop_order FOR SHARE",
        )
        .bind(tenant_id)
        .bind(request.route_id)
        .fetch_all(&mut *transaction)
        .await
        .context("Failed to snapshot Transport route stops")?;
        if stops.len() < 2 {
            bail!("A Transport route needs at least two stops before a run can be created");
        }
        let assignments = sqlx::query_as::<_, (Uuid, Uuid, Uuid, Uuid)>(
            r#"
            SELECT id,learner_id,boarding_stop_id,alighting_stop_id
              FROM transport_rider_assignments
             WHERE tenant_id=$1 AND route_id=$2 AND status='active' AND deleted_at IS NULL
               AND effective_from <= $3 AND COALESCE(effective_until,'infinity'::DATE) >= $3
             ORDER BY created_at,id FOR SHARE
            "#,
        )
        .bind(tenant_id)
        .bind(request.route_id)
        .bind(request.service_date)
        .fetch_all(&mut *transaction)
        .await
        .context("Failed to snapshot Transport riders")?;
        if assignments.is_empty() {
            bail!("This route has no active riders for the selected service date");
        }
        let learner_ids = assignments.iter().map(|row| row.1).collect::<Vec<_>>();
        let learners = LearnerOps::transport_references_by_ids(pool, tenant_id, &learner_ids)
            .await?
            .into_iter()
            .filter(|learner| learner.status == "active")
            .map(|learner| (learner.id, learner))
            .collect::<HashMap<_, _>>();
        if learners.len() != assignments.len() {
            bail!("Every active rider must still have an active SIS learner record");
        }
        let rider_count =
            i32::try_from(assignments.len()).context("Too many riders for one run")?;
        if rider_count > vehicle.capacity {
            bail!("The selected vehicle does not have enough seats for this route's riders");
        }
        let reference = reserve_run_reference(&mut transaction, tenant_id).await?;
        let run_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO transport_service_runs (
                id,tenant_id,reference,route_id,service_date,vehicle_id,driver_id,
                route_code_snapshot,route_name_snapshot,direction_snapshot,
                vehicle_registration_snapshot,driver_name_snapshot,capacity_snapshot,
                created_by,updated_by
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$14)
            "#,
        )
        .bind(run_id)
        .bind(tenant_id)
        .bind(&reference)
        .bind(request.route_id)
        .bind(request.service_date)
        .bind(vehicle.id)
        .bind(driver.id)
        .bind(&route.code)
        .bind(&route.name)
        .bind(&route.direction)
        .bind(&vehicle.registration_number)
        .bind(&driver.display_name)
        .bind(vehicle.capacity)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "A run already exists for this route and date"))?;
        let mut run_stop_ids = HashMap::new();
        for stop in &stops {
            let run_stop_id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO transport_run_stops (id,tenant_id,run_id,source_stop_id,stop_order,code_snapshot,name_snapshot,planned_time_snapshot) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
            )
            .bind(run_stop_id)
            .bind(tenant_id)
            .bind(run_id)
            .bind(stop.id)
            .bind(stop.stop_order)
            .bind(&stop.code)
            .bind(&stop.name)
            .bind(stop.planned_time)
            .execute(&mut *transaction)
            .await
            .context("Failed to snapshot a Transport route stop")?;
            run_stop_ids.insert(stop.id, run_stop_id);
        }
        for (assignment_id, learner_id, boarding_stop_id, alighting_stop_id) in assignments {
            let learner = learners
                .get(&learner_id)
                .ok_or_else(|| anyhow!("A Transport rider changed while the run was created"))?;
            let boarding_run_stop_id = run_stop_ids
                .get(&boarding_stop_id)
                .ok_or_else(|| anyhow!("A rider's boarding stop is not on this route"))?;
            let alighting_run_stop_id = run_stop_ids
                .get(&alighting_stop_id)
                .ok_or_else(|| anyhow!("A rider's alighting stop is not on this route"))?;
            sqlx::query(
                r#"
                INSERT INTO transport_manifest_entries (
                    tenant_id,run_id,source_assignment_id,learner_id,
                    learner_number_snapshot,learner_name_snapshot,
                    boarding_run_stop_id,alighting_run_stop_id
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
                "#,
            )
            .bind(tenant_id)
            .bind(run_id)
            .bind(assignment_id)
            .bind(learner_id)
            .bind(&learner.learner_number)
            .bind(&learner.display_name)
            .bind(boarding_run_stop_id)
            .bind(alighting_run_stop_id)
            .execute(&mut *transaction)
            .await
            .context("Failed to create a Transport manifest entry")?;
        }
        append_run_evidence(
            &mut transaction,
            RunEvidence {
                tenant_id,
                run_id,
                manifest_entry_id: None,
                actor,
                context,
                event_type: "transport.run.created",
                operation: "transport.runs.create",
                metadata: json!({
                    "reference": reference,
                    "route_code": route.code,
                    "service_date": request.service_date,
                    "vehicle_registration": vehicle.registration_number,
                    "driver_name": driver.display_name,
                    "rider_count": rider_count
                }),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Transport run")?;
        Self::get_run(pool, tenant_id, run_id)
            .await?
            .ok_or_else(|| anyhow!("The Transport run could not be reloaded"))
    }

    pub async fn start_boarding(
        pool: &PgPool,
        tenant_id: Uuid,
        run_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &RunTransitionRequest,
    ) -> Result<Option<RunRecordResponse>> {
        transition_run(
            pool,
            tenant_id,
            run_id,
            actor,
            context,
            request.expected_version,
            RunTransition::StartBoarding,
            None,
        )
        .await
    }

    pub async fn depart_run(
        pool: &PgPool,
        tenant_id: Uuid,
        run_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &RunTransitionRequest,
    ) -> Result<Option<RunRecordResponse>> {
        transition_run(
            pool,
            tenant_id,
            run_id,
            actor,
            context,
            request.expected_version,
            RunTransition::Depart,
            None,
        )
        .await
    }

    pub async fn complete_run(
        pool: &PgPool,
        tenant_id: Uuid,
        run_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &RunTransitionRequest,
    ) -> Result<Option<RunRecordResponse>> {
        transition_run(
            pool,
            tenant_id,
            run_id,
            actor,
            context,
            request.expected_version,
            RunTransition::Complete,
            None,
        )
        .await
    }

    pub async fn cancel_run(
        pool: &PgPool,
        tenant_id: Uuid,
        run_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &CancelRunRequest,
    ) -> Result<Option<RunRecordResponse>> {
        transition_run(
            pool,
            tenant_id,
            run_id,
            actor,
            context,
            request.expected_version,
            RunTransition::Cancel,
            Some(trimmed_required(&request.reason, "Cancellation reason")?),
        )
        .await
    }

    pub async fn mark_manifest_entry(
        pool: &PgPool,
        tenant_id: Uuid,
        run_id: Uuid,
        entry_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &MarkManifestEntryRequest,
    ) -> Result<Option<RunRecordResponse>> {
        validate_manifest_mark(request)?;
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start manifest update")?;
        let Some((run_status, _)) = lock_run(&mut transaction, tenant_id, run_id).await? else {
            return Ok(None);
        };
        if run_status != "boarding" {
            bail!("Manifest entries can be marked only while boarding is open");
        }
        let current = sqlx::query_as::<_, (i32, String)>(
            "SELECT version,status FROM transport_manifest_entries WHERE tenant_id=$1 AND run_id=$2 AND id=$3 AND deleted_at IS NULL FOR UPDATE",
        )
        .bind(tenant_id)
        .bind(run_id)
        .bind(entry_id)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to lock Transport manifest entry")?;
        let Some((version, previous_status)) = current else {
            bail!("The selected manifest entry does not exist on this run");
        };
        ensure_version(version, request.expected_version, "manifest entry")?;
        let (exception_kind, note, marked_by) = if request.status == ManifestStatus::Expected {
            (None, None, None)
        } else {
            (
                request.exception_kind.map(|value| value.as_str()),
                trimmed_optional(request.note.as_deref()),
                Some(actor_id),
            )
        };
        sqlx::query(
            "UPDATE transport_manifest_entries SET status=$4,exception_kind=$5,note=$6,marked_by=$7,marked_at=CASE WHEN $7::UUID IS NULL THEN NULL ELSE NOW() END,version=version+1 WHERE tenant_id=$1 AND run_id=$2 AND id=$3",
        )
        .bind(tenant_id)
        .bind(run_id)
        .bind(entry_id)
        .bind(request.status.as_str())
        .bind(exception_kind)
        .bind(note)
        .bind(marked_by)
        .execute(&mut *transaction)
        .await
        .context("Failed to update Transport manifest entry")?;
        append_run_evidence(
            &mut transaction,
            RunEvidence {
                tenant_id,
                run_id,
                manifest_entry_id: Some(entry_id),
                actor,
                context,
                event_type: "transport.manifest.marked",
                operation: "transport.manifest.mark",
                metadata: json!({
                    "previous_status": previous_status,
                    "status": request.status.as_str(),
                    "exception_kind": exception_kind
                }),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit manifest update")?;
        Self::get_run(pool, tenant_id, run_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;
    use sqlx::postgres::PgPoolOptions;

    #[test]
    fn exception_marks_require_type_and_note() {
        let request = MarkManifestEntryRequest {
            status: ManifestStatus::Exception,
            exception_kind: None,
            note: Some("Safety concern".to_string()),
            expected_version: 1,
        };
        assert!(validate_manifest_mark(&request).is_err());
    }

    #[test]
    fn route_filters_are_bounded() {
        assert_eq!(bounded_page(Some(0), Some(500)), (1, 100));
    }

    #[test]
    fn transport_value_helpers_cover_closed_states() {
        assert_eq!(bounded_page(None, None), (1, 20));
        assert!(ensure_version(2, 2, "route").is_ok());
        assert!(ensure_version(2, 1, "route").is_err());
        assert!(person_actor_id(AuditActor::system()).is_err());
        assert_eq!(trimmed_required("  Route  ", "Route").unwrap(), "Route");
        assert!(trimmed_required("  ", "Route").is_err());
        assert_eq!(trimmed_optional(Some(" note ")), Some("note"));
        assert_eq!(trimmed_optional(Some("  ")), None);
        assert_eq!(search_pattern(Some(" bus ")), Some("%bus%".to_string()));
        assert_eq!(search_pattern(Some("  ")), None);

        for status in [
            ManifestStatus::Expected,
            ManifestStatus::Boarded,
            ManifestStatus::NoShow,
        ] {
            assert!(
                validate_manifest_mark(&MarkManifestEntryRequest {
                    status,
                    exception_kind: None,
                    note: None,
                    expected_version: 1,
                })
                .is_ok()
            );
        }
        assert!(
            validate_manifest_mark(&MarkManifestEntryRequest {
                status: ManifestStatus::Boarded,
                exception_kind: Some(crate::ManifestExceptionKind::Safety),
                note: None,
                expected_version: 1,
            })
            .is_err()
        );
        assert!(
            validate_manifest_mark(&MarkManifestEntryRequest {
                status: ManifestStatus::Exception,
                exception_kind: Some(crate::ManifestExceptionKind::Safety),
                note: Some("  Driver reported a safety concern.  ".to_string()),
                expected_version: 1,
            })
            .is_ok()
        );
    }

    /// Exercises the complete public Transport lifecycle against a caller-owned,
    /// disposable database. The explicit database-name guard prevents a test
    /// invocation from ever migrating or mutating a normal Campus Pilot database.
    #[actix_web::test]
    async fn transport_lifecycle_is_tenant_scoped_and_audited() {
        let Ok(database_url) = std::env::var("CP_TRANSPORT_TEST_DATABASE_URL") else {
            return;
        };
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(&database_url)
            .await
            .expect("the disposable Transport test database must connect");
        let database_name = sqlx::query_scalar::<_, String>("SELECT current_database()")
            .fetch_one(&pool)
            .await
            .expect("the disposable database name must be readable");
        assert!(
            database_name.starts_with("campus_pilot_transport_"),
            "refusing to run Transport integration tests against {database_name}"
        );
        sqlx::raw_sql(include_str!("../../../../migrations/functions.sql"))
            .execute(&pool)
            .await
            .expect("shared migration functions must install");
        sqlx::migrate!("../../../migrations")
            .run(&pool)
            .await
            .expect("Transport integration migrations must apply");

        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let learner_id = Uuid::new_v4();
        let employee_id = Uuid::new_v4();
        let vehicle_id = Uuid::new_v4();
        let driver_id = Uuid::new_v4();
        let suffix = Uuid::new_v4().simple().to_string();
        sqlx::query("INSERT INTO tenants (id,slug,name) VALUES ($1,$2,$3)")
            .bind(tenant_id)
            .bind(format!("transport-test-{suffix}"))
            .bind("Transport lifecycle test")
            .execute(&pool)
            .await
            .expect("tenant fixture must insert");
        sqlx::query(
            "INSERT INTO users (id,tenant_id,email,password_hash,full_name,roles) VALUES ($1,$2,$3,'test','Transport Operator',ARRAY['campus_owner'])",
        )
        .bind(actor_id)
        .bind(tenant_id)
        .bind(format!("transport-{suffix}@example.test"))
        .execute(&pool)
        .await
        .expect("operator fixture must insert");
        sqlx::query(
            "INSERT INTO learners (id,tenant_id,learner_number,display_name,first_names,surname,date_of_birth,status) VALUES ($1,$2,'TRN-001','Tariro Moyo','Tariro','Moyo','2012-01-01','active')",
        )
        .bind(learner_id)
        .bind(tenant_id)
        .execute(&pool)
        .await
        .expect("learner fixture must insert");
        sqlx::query(
            "INSERT INTO employees (id,tenant_id,employee_number,display_name,first_names,surname,employment_status) VALUES ($1,$2,'EMP-TRN-1','Bus Driver','Bus','Driver','active')",
        )
        .bind(employee_id)
        .bind(tenant_id)
        .execute(&pool)
        .await
        .expect("employee fixture must insert");
        sqlx::query(
            "INSERT INTO vehicles (id,tenant_id,registration_number,make,model,capacity,status) VALUES ($1,$2,'TRN-001','Test','Bus',20,'active')",
        )
        .bind(vehicle_id)
        .bind(tenant_id)
        .execute(&pool)
        .await
        .expect("vehicle fixture must insert");
        sqlx::query(
            "INSERT INTO drivers (id,tenant_id,employee_id,license_number,license_expiry,status) VALUES ($1,$2,$3,'LIC-TRN-1','2030-01-01','active')",
        )
        .bind(driver_id)
        .bind(tenant_id)
        .bind(employee_id)
        .execute(&pool)
        .await
        .expect("driver fixture must insert");

        let actor = AuditActor::person(actor_id);
        let context = RequestContext::generate(None);
        let service_date = NaiveDate::from_ymd_opt(2026, 9, 2).unwrap();
        let route = TransportOps::create_route(
            &pool,
            tenant_id,
            actor,
            context,
            &CreateRouteRequest {
                code: " AM-01 ".to_string(),
                name: " Morning route ".to_string(),
                direction: crate::RouteDirection::Inbound,
                notes: Some("  First run  ".to_string()),
            },
        )
        .await
        .expect("route creation must succeed");
        assert_eq!(route.route.code, "AM-01");
        assert!(
            TransportOps::create_route(
                &pool,
                tenant_id,
                actor,
                context,
                &CreateRouteRequest {
                    code: "AM-01".to_string(),
                    name: "Duplicate".to_string(),
                    direction: crate::RouteDirection::Inbound,
                    notes: None,
                },
            )
            .await
            .unwrap_err()
            .to_string()
            .contains("already exists")
        );
        assert!(
            TransportOps::update_route(
                &pool,
                tenant_id,
                route.route.id,
                actor,
                context,
                &UpdateRouteRequest {
                    code: "AM-01".to_string(),
                    name: "Morning route".to_string(),
                    direction: crate::RouteDirection::Inbound,
                    status: crate::RouteStatus::Active,
                    notes: None,
                    expected_version: 99,
                },
            )
            .await
            .is_err()
        );
        let route = TransportOps::update_route(
            &pool,
            tenant_id,
            route.route.id,
            actor,
            context,
            &UpdateRouteRequest {
                code: "AM-01".to_string(),
                name: "Morning collection".to_string(),
                direction: crate::RouteDirection::Inbound,
                status: crate::RouteStatus::Active,
                notes: None,
                expected_version: 1,
            },
        )
        .await
        .expect("route update must succeed")
        .expect("route must remain visible");
        assert_eq!(route.route.version, 2);

        let missing_id = Uuid::new_v4();
        assert!(
            TransportOps::get_route(&pool, tenant_id, missing_id)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            TransportOps::create_stop(
                &pool,
                tenant_id,
                missing_id,
                actor,
                context,
                &CreateStopRequest {
                    code: "X".to_string(),
                    name: "Missing route".to_string(),
                    stop_order: 1,
                    planned_time: NaiveTime::from_hms_opt(6, 30, 0).unwrap(),
                    latitude: None,
                    longitude: None,
                },
            )
            .await
            .unwrap()
            .is_none()
        );

        let mut route_record = route;
        for (code, name, order, hour) in [
            ("A", "First stop", 1, 7),
            ("B", "Campus", 2, 8),
            ("C", "Spare stop", 3, 9),
        ] {
            route_record = TransportOps::create_stop(
                &pool,
                tenant_id,
                route_record.route.id,
                actor,
                context,
                &CreateStopRequest {
                    code: code.to_string(),
                    name: name.to_string(),
                    stop_order: order,
                    planned_time: NaiveTime::from_hms_opt(hour, 0, 0).unwrap(),
                    latitude: Some(-17.8),
                    longitude: Some(31.0),
                },
            )
            .await
            .expect("stop creation must succeed")
            .expect("route must remain visible");
        }
        let first_stop = route_record.stops[0].clone();
        let second_stop = route_record.stops[1].clone();
        let spare_stop = route_record.stops[2].clone();
        route_record = TransportOps::update_stop(
            &pool,
            tenant_id,
            route_record.route.id,
            spare_stop.id,
            actor,
            context,
            &UpdateStopRequest {
                code: "C".to_string(),
                name: "Updated spare".to_string(),
                stop_order: 3,
                planned_time: NaiveTime::from_hms_opt(9, 15, 0).unwrap(),
                latitude: None,
                longitude: None,
                expected_version: spare_stop.version,
            },
        )
        .await
        .expect("stop update must succeed")
        .expect("route must remain visible");
        let spare_stop = route_record.stops[2].clone();
        route_record = TransportOps::remove_stop(
            &pool,
            tenant_id,
            route_record.route.id,
            spare_stop.id,
            actor,
            context,
            spare_stop.version,
        )
        .await
        .expect("unused stop removal must succeed")
        .expect("route must remain visible");
        assert_eq!(route_record.stops.len(), 2);

        let invalid_assignment = CreateRiderAssignmentRequest {
            learner_id,
            route_id: route_record.route.id,
            boarding_stop_id: second_stop.id,
            alighting_stop_id: first_stop.id,
            effective_from: service_date,
            effective_until: None,
        };
        assert!(
            TransportOps::create_rider_assignment(
                &pool,
                tenant_id,
                actor,
                context,
                &invalid_assignment,
            )
            .await
            .is_err()
        );
        let assignment = TransportOps::create_rider_assignment(
            &pool,
            tenant_id,
            actor,
            context,
            &CreateRiderAssignmentRequest {
                learner_id,
                route_id: route_record.route.id,
                boarding_stop_id: first_stop.id,
                alighting_stop_id: second_stop.id,
                effective_from: service_date,
                effective_until: None,
            },
        )
        .await
        .expect("rider assignment must succeed");
        assert!(
            TransportOps::update_stop(
                &pool,
                tenant_id,
                route_record.route.id,
                first_stop.id,
                actor,
                context,
                &UpdateStopRequest {
                    code: first_stop.code.clone(),
                    name: first_stop.name.clone(),
                    stop_order: first_stop.stop_order,
                    planned_time: first_stop.planned_time,
                    latitude: first_stop.latitude,
                    longitude: first_stop.longitude,
                    expected_version: first_stop.version,
                },
            )
            .await
            .is_err()
        );
        assert!(
            TransportOps::create_rider_assignment(
                &pool,
                tenant_id,
                actor,
                context,
                &CreateRiderAssignmentRequest {
                    learner_id,
                    route_id: route_record.route.id,
                    boarding_stop_id: first_stop.id,
                    alighting_stop_id: second_stop.id,
                    effective_from: service_date,
                    effective_until: None,
                },
            )
            .await
            .is_err()
        );
        let (riders, rider_total) = TransportOps::list_riders(
            &pool,
            tenant_id,
            &ListRidersQuery {
                page: Some(1),
                per_page: Some(10),
                search: Some("Tariro".to_string()),
                route_id: Some(route_record.route.id),
                status: Some(crate::RiderStatus::Active),
                on_date: Some(service_date),
            },
        )
        .await
        .expect("rider list must load");
        assert_eq!(rider_total, 1);
        assert_eq!(riders[0].id, assignment.id);

        let references = TransportOps::reference_data(
            &pool,
            tenant_id,
            &ReferenceQuery {
                search: Some("TRN".to_string()),
            },
        )
        .await
        .expect("Transport references must load");
        assert_eq!(references.learners.len(), 1);
        assert_eq!(references.vehicles.len(), 1);
        assert_eq!(references.drivers.len(), 1);
        assert_eq!(references.routes.len(), 1);
        let (routes, route_total) = TransportOps::list_routes(
            &pool,
            tenant_id,
            &ListRoutesQuery {
                page: Some(1),
                per_page: Some(10),
                search: Some("Morning".to_string()),
                status: Some(crate::RouteStatus::Active),
                direction: Some(crate::RouteDirection::Inbound),
            },
        )
        .await
        .expect("route list must load");
        assert_eq!(route_total, 1);
        assert_eq!(routes[0].id, route_record.route.id);
        assert!(
            TransportOps::list_runs(
                &pool,
                tenant_id,
                &ListRunsQuery {
                    page: None,
                    per_page: None,
                    route_id: None,
                    status: None,
                    date_from: Some(service_date),
                    date_to: Some(service_date.pred_opt().unwrap()),
                },
            )
            .await
            .is_err()
        );

        let mut run = TransportOps::create_run(
            &pool,
            tenant_id,
            actor,
            context,
            &CreateRunRequest {
                route_id: route_record.route.id,
                service_date,
                vehicle_id,
                driver_id,
            },
        )
        .await
        .expect("run creation must succeed");
        assert_eq!(run.run.reference, "TRN-000001");
        assert_eq!(run.manifest.len(), 1);
        let manifest_id = run.manifest[0].id;
        assert!(
            TransportOps::depart_run(
                &pool,
                tenant_id,
                run.run.id,
                actor,
                context,
                &RunTransitionRequest {
                    expected_version: run.run.version,
                },
            )
            .await
            .is_err()
        );
        run = TransportOps::start_boarding(
            &pool,
            tenant_id,
            run.run.id,
            actor,
            context,
            &RunTransitionRequest {
                expected_version: run.run.version,
            },
        )
        .await
        .expect("boarding transition must succeed")
        .expect("run must remain visible");
        assert!(
            TransportOps::depart_run(
                &pool,
                tenant_id,
                run.run.id,
                actor,
                context,
                &RunTransitionRequest {
                    expected_version: run.run.version,
                },
            )
            .await
            .is_err()
        );
        run = TransportOps::mark_manifest_entry(
            &pool,
            tenant_id,
            run.run.id,
            manifest_id,
            actor,
            context,
            &MarkManifestEntryRequest {
                status: ManifestStatus::Boarded,
                exception_kind: None,
                note: None,
                expected_version: 1,
            },
        )
        .await
        .expect("manifest marking must succeed")
        .expect("run must remain visible");
        run = TransportOps::depart_run(
            &pool,
            tenant_id,
            run.run.id,
            actor,
            context,
            &RunTransitionRequest {
                expected_version: run.run.version,
            },
        )
        .await
        .expect("departure transition must succeed")
        .expect("run must remain visible");
        run = TransportOps::complete_run(
            &pool,
            tenant_id,
            run.run.id,
            actor,
            context,
            &RunTransitionRequest {
                expected_version: run.run.version,
            },
        )
        .await
        .expect("completion transition must succeed")
        .expect("run must remain visible");
        assert_eq!(run.run.status, "completed");
        assert!(
            TransportOps::mark_manifest_entry(
                &pool,
                tenant_id,
                run.run.id,
                manifest_id,
                actor,
                context,
                &MarkManifestEntryRequest {
                    status: ManifestStatus::Expected,
                    exception_kind: None,
                    note: None,
                    expected_version: 2,
                },
            )
            .await
            .is_err()
        );

        let second_date = service_date.succ_opt().unwrap();
        let cancel_run = TransportOps::create_run(
            &pool,
            tenant_id,
            actor,
            context,
            &CreateRunRequest {
                route_id: route_record.route.id,
                service_date: second_date,
                vehicle_id,
                driver_id,
            },
        )
        .await
        .expect("second run creation must succeed");
        let cancel_run = TransportOps::cancel_run(
            &pool,
            tenant_id,
            cancel_run.run.id,
            actor,
            context,
            &CancelRunRequest {
                reason: "Vehicle unavailable".to_string(),
                expected_version: cancel_run.run.version,
            },
        )
        .await
        .expect("run cancellation must succeed")
        .expect("cancelled run must remain visible");
        assert_eq!(cancel_run.run.status, "cancelled");
        let (runs, run_total) = TransportOps::list_runs(
            &pool,
            tenant_id,
            &ListRunsQuery {
                page: Some(1),
                per_page: Some(10),
                route_id: Some(route_record.route.id),
                status: None,
                date_from: Some(service_date),
                date_to: Some(second_date),
            },
        )
        .await
        .expect("run list must load");
        assert_eq!(run_total, 2);
        assert_eq!(runs.len(), 2);

        let ended = TransportOps::end_rider_assignment(
            &pool,
            tenant_id,
            assignment.id,
            actor,
            context,
            &EndRiderAssignmentRequest {
                effective_until: second_date,
                reason: "Route changed".to_string(),
                expected_version: assignment.version,
            },
        )
        .await
        .expect("rider assignment must end")
        .expect("ended assignment must remain visible");
        assert_eq!(ended.status, "ended");
        let inactive = TransportOps::update_route(
            &pool,
            tenant_id,
            route_record.route.id,
            actor,
            context,
            &UpdateRouteRequest {
                code: "AM-01".to_string(),
                name: "Morning collection".to_string(),
                direction: crate::RouteDirection::Outbound,
                status: crate::RouteStatus::Inactive,
                notes: None,
                expected_version: route_record.route.version,
            },
        )
        .await
        .expect("unused route can be retired")
        .expect("retired route must remain visible");
        assert_eq!(inactive.route.status, "inactive");

        let evidence_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM actor_audit_events WHERE tenant_id=$1 AND action_key LIKE 'transport.%'",
        )
        .bind(tenant_id)
        .fetch_one(&pool)
        .await
        .expect("Transport audit evidence must be countable");
        assert!(evidence_count >= 15);
    }
}
