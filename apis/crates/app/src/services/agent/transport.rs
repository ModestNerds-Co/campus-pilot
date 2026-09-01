//! Exposes reduced Transport worklists and records through the Agent broker.

use async_trait::async_trait;
use chrono::NaiveDate;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::PaginationMeta;
use cp_transport::{
    ListRidersQuery, ListRoutesQuery, ListRunsQuery, RiderStatus, RouteDirection,
    RouteRecordResponse, RouteStatus, RunRecordResponse, RunStatus, TransportOps,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::administration::read_descriptor;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListRoutesInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<RouteStatus>,
    direction: Option<RouteDirection>,
}

#[derive(Serialize)]
pub(super) struct ListRoutesOutput {
    routes: Vec<Value>,
    pagination: PaginationMeta,
}

pub(super) struct TransportRoutesListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl TransportRoutesListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "transport.routes.list",
                "List transport routes",
                "Returns bounded route summaries and current rider counts.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "search": { "type": ["string", "null"] },
                    "status": { "type": ["string", "null"], "enum": ["active", "inactive", null] },
                    "direction": { "type": ["string", "null"], "enum": ["inbound", "outbound", null] }
                }),
                json!({ "routes": { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::General,
                "transport.routes",
            ),
        }
    }
}

#[async_trait]
impl Capability for TransportRoutesListCapability {
    type Input = ListRoutesInput;
    type Output = ListRoutesOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, _input: &Self::Input) -> CapabilityScope {
        CapabilityScope::TenantWide
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let page = input.page.unwrap_or(1).max(1);
        let per_page = input.per_page.unwrap_or(25).clamp(1, 100);
        let (routes, total) = TransportOps::list_routes(
            &self.pool,
            context.principal().tenant_id(),
            &ListRoutesQuery {
                page: Some(page),
                per_page: Some(per_page),
                search: input.search,
                status: input.status,
                direction: input.direction,
            },
        )
        .await
        .map_err(|_| dependency_failure("Transport routes could not be loaded."))?;
        Ok(ListRoutesOutput {
            routes: routes.into_iter().map(route_summary).collect(),
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadRouteInput {
    route_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct ReadRouteOutput {
    route: Value,
}

pub(super) struct TransportRouteReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl TransportRouteReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "transport.routes.read",
                "Read transport route",
                "Returns one route with its ordered stop plan.",
                json!({ "route_id": { "type": "string", "format": "uuid" } }),
                json!({ "route": { "type": "object" } }),
                DataSensitivity::General,
                "transport.routes",
            ),
        }
    }
}

#[async_trait]
impl Capability for TransportRouteReadCapability {
    type Input = ReadRouteInput;
    type Output = ReadRouteOutput;
    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("transport_route", input.route_id)
    }
    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let route =
            TransportOps::get_route(&self.pool, context.principal().tenant_id(), input.route_id)
                .await
                .map_err(|_| dependency_failure("The Transport route could not be loaded."))?
                .ok_or_else(|| invalid_state("The Transport route was not found."))?;
        Ok(ReadRouteOutput {
            route: route_record(route),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListRidersInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    route_id: Option<Uuid>,
    status: Option<RiderStatus>,
    on_date: Option<NaiveDate>,
}

#[derive(Serialize)]
pub(super) struct ListRidersOutput {
    riders: Vec<Value>,
    pagination: PaginationMeta,
}

pub(super) struct TransportRidersListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl TransportRidersListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "transport.riders.list",
                "List transport riders",
                "Returns dated rider assignments with learner numbers and planned stops.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "search": { "type": ["string", "null"] },
                    "route_id": { "type": ["string", "null"], "format": "uuid" },
                    "status": { "type": ["string", "null"], "enum": ["active", "ended", "cancelled", null] },
                    "on_date": { "type": ["string", "null"], "format": "date" }
                }),
                json!({ "riders": { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::Personal,
                "transport.riders",
            ),
        }
    }
}

#[async_trait]
impl Capability for TransportRidersListCapability {
    type Input = ListRidersInput;
    type Output = ListRidersOutput;
    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, _input: &Self::Input) -> CapabilityScope {
        CapabilityScope::TenantWide
    }
    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let page = input.page.unwrap_or(1).max(1);
        let per_page = input.per_page.unwrap_or(25).clamp(1, 100);
        let (riders, total) = TransportOps::list_riders(
            &self.pool,
            context.principal().tenant_id(),
            &ListRidersQuery {
                page: Some(page),
                per_page: Some(per_page),
                search: input.search,
                route_id: input.route_id,
                status: input.status,
                on_date: input.on_date,
            },
        )
        .await
        .map_err(|_| dependency_failure("Transport riders could not be loaded."))?;
        let riders = riders.into_iter().map(|rider| json!({
            "learner_number": rider.learner_number, "learner_name": rider.learner_name,
            "route_code": rider.route_code, "route_name": rider.route_name, "direction": rider.direction,
            "boarding_stop": rider.boarding_stop_name, "alighting_stop": rider.alighting_stop_name,
            "effective_from": rider.effective_from, "effective_until": rider.effective_until, "status": rider.status
        })).collect();
        Ok(ListRidersOutput {
            riders,
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListRunsInput {
    page: Option<i64>,
    per_page: Option<i64>,
    route_id: Option<Uuid>,
    status: Option<RunStatus>,
    date_from: Option<NaiveDate>,
    date_to: Option<NaiveDate>,
}

#[derive(Serialize)]
pub(super) struct ListRunsOutput {
    runs: Vec<Value>,
    pagination: PaginationMeta,
}

pub(super) struct TransportRunsListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl TransportRunsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "transport.runs.list",
                "List transport runs",
                "Returns bounded dated run summaries and manifest counts.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 }, "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "route_id": { "type": ["string", "null"], "format": "uuid" },
                    "status": { "type": ["string", "null"], "enum": ["draft", "boarding", "departed", "completed", "cancelled", null] },
                    "date_from": { "type": ["string", "null"], "format": "date" }, "date_to": { "type": ["string", "null"], "format": "date" }
                }),
                json!({ "runs": { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::Personal,
                "transport.runs",
            ),
        }
    }
}

#[async_trait]
impl Capability for TransportRunsListCapability {
    type Input = ListRunsInput;
    type Output = ListRunsOutput;
    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, _input: &Self::Input) -> CapabilityScope {
        CapabilityScope::TenantWide
    }
    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let page = input.page.unwrap_or(1).max(1);
        let per_page = input.per_page.unwrap_or(25).clamp(1, 100);
        let (runs, total) = TransportOps::list_runs(
            &self.pool,
            context.principal().tenant_id(),
            &ListRunsQuery {
                page: Some(page),
                per_page: Some(per_page),
                route_id: input.route_id,
                status: input.status,
                date_from: input.date_from,
                date_to: input.date_to,
            },
        )
        .await
        .map_err(|_| dependency_failure("Transport runs could not be loaded."))?;
        Ok(ListRunsOutput {
            runs: runs.into_iter().map(run_summary).collect(),
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadRunInput {
    run_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct ReadRunOutput {
    run: Value,
}

pub(super) struct TransportRunReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl TransportRunReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "transport.runs.read",
                "Read transport run",
                "Returns one run with stop, manifest, and lifecycle evidence.",
                json!({ "run_id": { "type": "string", "format": "uuid" } }),
                json!({ "run": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "transport.runs",
            ),
        }
    }
}

#[async_trait]
impl Capability for TransportRunReadCapability {
    type Input = ReadRunInput;
    type Output = ReadRunOutput;
    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("transport_run", input.run_id)
    }
    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let run = TransportOps::get_run(&self.pool, context.principal().tenant_id(), input.run_id)
            .await
            .map_err(|_| dependency_failure("The Transport run could not be loaded."))?
            .ok_or_else(|| invalid_state("The Transport run was not found."))?;
        Ok(ReadRunOutput {
            run: run_record(run),
        })
    }
}

fn route_summary(route: cp_transport::RouteSummaryResponse) -> Value {
    json!({
        "route_id": route.id, "code": route.code, "name": route.name, "direction": route.direction,
        "status": route.status, "stop_count": route.stop_count, "active_rider_count": route.active_rider_count
    })
}

fn route_record(route: RouteRecordResponse) -> Value {
    json!({
        "route": route_summary(route.route),
        "stops": route.stops.into_iter().map(|stop| json!({ "code": stop.code, "name": stop.name, "stop_order": stop.stop_order, "planned_time": stop.planned_time })).collect::<Vec<_>>()
    })
}

fn run_summary(run: cp_transport::RunSummaryResponse) -> Value {
    json!({
        "run_id": run.id, "reference": run.reference, "route_code": run.route_code, "route_name": run.route_name,
        "direction": run.direction, "service_date": run.service_date, "vehicle_registration": run.vehicle_registration,
        "driver_name": run.driver_name, "capacity": run.capacity, "status": run.status,
        "expected_count": run.expected_count, "boarded_count": run.boarded_count, "exception_count": run.exception_count
    })
}

fn run_record(run: RunRecordResponse) -> Value {
    json!({
        "run": run_summary(run.run),
        "stops": run.stops.into_iter().map(|stop| json!({ "code": stop.code, "name": stop.name, "stop_order": stop.stop_order, "planned_time": stop.planned_time })).collect::<Vec<_>>(),
        "manifest": run.manifest.into_iter().map(|entry| json!({
            "manifest_entry_id": entry.id, "learner_number": entry.learner_number, "learner_name": entry.learner_name,
            "boarding_stop": entry.boarding_stop_name, "alighting_stop": entry.alighting_stop_name,
            "status": entry.status, "exception_kind": entry.exception_kind, "note": entry.note, "marked_at": entry.marked_at
        })).collect::<Vec<_>>(),
        "history": run.history.into_iter().map(|event| json!({ "event_type": event.event_type, "actor_name": event.actor_name, "created_at": event.created_at })).collect::<Vec<_>>()
    })
}

fn resource_scope(resource_type: &str, id: Uuid) -> CapabilityScope {
    CapabilityScope::resources([CapabilityResource::parse(resource_type, id.to_string())
        .unwrap_or_else(|error| panic!("invalid built-in Transport resource: {error}"))])
    .unwrap_or_else(|error| panic!("invalid built-in Transport scope: {error}"))
}

fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}
fn invalid_state(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::InvalidState, message)
}
