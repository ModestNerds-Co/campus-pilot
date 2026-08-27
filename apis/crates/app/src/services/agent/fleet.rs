//! Adapts tenant-scoped Fleet and Vehicle Log reads to typed Agent capabilities.

use async_trait::async_trait;
use chrono::NaiveDate;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::PaginationMeta;
use cp_fleet::{
    dtos::{DriverResponse, PaginatedDriversResponse, PaginatedVehiclesResponse, VehicleResponse},
    ops::{DriverOps, VehicleOps},
};
use cp_hr_payroll::models::EmployeeReference;
use cp_vehicle_log::{
    dtos::{PaginatedVehicleDailyLogsResponse, VehicleDailyLogResponse},
    ops::VehicleDailyLogOps,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::administration::read_descriptor;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListDriverCandidatesInput {
    search: Option<String>,
}

#[derive(Serialize)]
pub(super) struct ListDriverCandidatesOutput {
    employees: Vec<EmployeeReference>,
}

pub(super) struct FleetDriverCandidatesListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl FleetDriverCandidatesListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "fleet.driver_candidates.list",
                "List eligible driver employees",
                "Returns active employees without an existing driver profile.",
                json!({ "search": { "type": ["string", "null"], "maxLength": 200 } }),
                json!({ "employees": { "type": "array" } }),
                DataSensitivity::Personal,
                "fleet.driver_candidates",
            ),
        }
    }
}

#[async_trait]
impl Capability for FleetDriverCandidatesListCapability {
    type Input = ListDriverCandidatesInput;
    type Output = ListDriverCandidatesOutput;

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
        let employees = DriverOps::list_candidates(
            &self.pool,
            context.principal().tenant_id(),
            trimmed(input.search.as_deref()),
        )
        .await
        .map_err(|_| dependency_failure("Eligible employees could not be loaded."))?;
        Ok(ListDriverCandidatesOutput { employees })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListFleetRecordsInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<String>,
}

#[derive(Serialize)]
pub(super) struct ListVehiclesOutput {
    vehicles: PaginatedVehiclesResponse,
    pagination: PaginationMeta,
}

pub(super) struct FleetVehiclesListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl FleetVehiclesListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: fleet_list_descriptor(
                "fleet.vehicles.list",
                "List vehicles",
                "Returns tenant vehicles using bounded pagination and optional filters.",
                "vehicles",
                DataSensitivity::General,
                "fleet.vehicles",
            ),
        }
    }
}

#[async_trait]
impl Capability for FleetVehiclesListCapability {
    type Input = ListFleetRecordsInput;
    type Output = ListVehiclesOutput;

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
        let (page, per_page) = bounded_page(input.page, input.per_page);
        let (vehicles, total) = VehicleOps::list(
            &self.pool,
            context.principal().tenant_id(),
            page,
            per_page,
            trimmed(input.search.as_deref()),
            trimmed(input.status.as_deref()),
        )
        .await
        .map_err(|_| dependency_failure("Vehicles could not be loaded."))?;
        Ok(ListVehiclesOutput {
            vehicles: PaginatedVehiclesResponse {
                vehicles: vehicles.into_iter().map(VehicleResponse::from).collect(),
            },
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadVehicleInput {
    vehicle_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct ReadVehicleOutput {
    vehicle: VehicleResponse,
}

pub(super) struct FleetVehicleReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl FleetVehicleReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "fleet.vehicles.read",
                "Read vehicle",
                "Returns one tenant vehicle by its stable identifier.",
                json!({ "vehicle_id": { "type": "string", "format": "uuid" } }),
                json!({ "vehicle": { "type": "object" } }),
                DataSensitivity::General,
                "fleet.vehicles",
            ),
        }
    }
}

#[async_trait]
impl Capability for FleetVehicleReadCapability {
    type Input = ReadVehicleInput;
    type Output = ReadVehicleOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("vehicle", input.vehicle_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let vehicle = VehicleOps::get_by_id(
            &self.pool,
            context.principal().tenant_id(),
            input.vehicle_id,
        )
        .await
        .map_err(|_| dependency_failure("The vehicle could not be loaded."))?
        .ok_or_else(|| not_found("The vehicle was not found."))?;
        Ok(ReadVehicleOutput {
            vehicle: VehicleResponse::from(vehicle),
        })
    }
}

#[derive(Serialize)]
pub(super) struct ListDriversOutput {
    drivers: PaginatedDriversResponse,
    pagination: PaginationMeta,
}

pub(super) struct FleetDriversListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl FleetDriversListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: fleet_list_descriptor(
                "fleet.drivers.list",
                "List drivers",
                "Returns tenant drivers using bounded pagination and optional filters.",
                "drivers",
                DataSensitivity::Personal,
                "fleet.drivers",
            ),
        }
    }
}

#[async_trait]
impl Capability for FleetDriversListCapability {
    type Input = ListFleetRecordsInput;
    type Output = ListDriversOutput;

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
        let (page, per_page) = bounded_page(input.page, input.per_page);
        let (drivers, total) = DriverOps::list(
            &self.pool,
            context.principal().tenant_id(),
            page,
            per_page,
            trimmed(input.search.as_deref()),
            trimmed(input.status.as_deref()),
        )
        .await
        .map_err(|_| dependency_failure("Drivers could not be loaded."))?;
        Ok(ListDriversOutput {
            drivers: PaginatedDriversResponse {
                drivers: drivers.into_iter().map(DriverResponse::from).collect(),
            },
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadDriverInput {
    driver_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct ReadDriverOutput {
    driver: DriverResponse,
}

pub(super) struct FleetDriverReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl FleetDriverReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "fleet.drivers.read",
                "Read driver",
                "Returns one tenant driver by its stable identifier.",
                json!({ "driver_id": { "type": "string", "format": "uuid" } }),
                json!({ "driver": { "type": "object" } }),
                DataSensitivity::Personal,
                "fleet.drivers",
            ),
        }
    }
}

#[async_trait]
impl Capability for FleetDriverReadCapability {
    type Input = ReadDriverInput;
    type Output = ReadDriverOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("driver", input.driver_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let driver =
            DriverOps::get_by_id(&self.pool, context.principal().tenant_id(), input.driver_id)
                .await
                .map_err(|_| dependency_failure("The driver could not be loaded."))?
                .ok_or_else(|| not_found("The driver was not found."))?;
        Ok(ReadDriverOutput {
            driver: DriverResponse::from(driver),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListVehicleLogsInput {
    page: Option<i64>,
    per_page: Option<i64>,
    vehicle_id: Option<Uuid>,
    driver_id: Option<Uuid>,
    status: Option<String>,
    from_date: Option<NaiveDate>,
    to_date: Option<NaiveDate>,
}

#[derive(Serialize)]
pub(super) struct ListVehicleLogsOutput {
    logs: PaginatedVehicleDailyLogsResponse,
    pagination: PaginationMeta,
}

pub(super) struct FleetVehicleLogsListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl FleetVehicleLogsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "fleet.vehicle_logs.list",
                "List vehicle logs",
                "Returns tenant vehicle logs using bounded operational filters.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "vehicle_id": { "type": ["string", "null"], "format": "uuid" },
                    "driver_id": { "type": ["string", "null"], "format": "uuid" },
                    "status": { "type": ["string", "null"], "maxLength": 50 },
                    "from_date": { "type": ["string", "null"], "format": "date" },
                    "to_date": { "type": ["string", "null"], "format": "date" }
                }),
                json!({
                    "logs": { "type": "object" },
                    "pagination": { "type": "object" }
                }),
                DataSensitivity::Sensitive,
                "fleet.vehicle_logs",
            ),
        }
    }
}

#[async_trait]
impl Capability for FleetVehicleLogsListCapability {
    type Input = ListVehicleLogsInput;
    type Output = ListVehicleLogsOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        let resources = [
            input.vehicle_id.map(|id| ("vehicle", id)),
            input.driver_id.map(|id| ("driver", id)),
        ]
        .into_iter()
        .flatten()
        .map(|(kind, id)| {
            CapabilityResource::parse(kind, id.to_string())
                .unwrap_or_else(|error| panic!("invalid built-in capability resource: {error}"))
        })
        .collect::<Vec<_>>();
        if resources.is_empty() {
            CapabilityScope::TenantWide
        } else {
            CapabilityScope::resources(resources)
                .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
        }
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let (page, per_page) = bounded_page(input.page, input.per_page);
        let (logs, total) = VehicleDailyLogOps::list(
            &self.pool,
            context.principal().tenant_id(),
            page,
            per_page,
            input.vehicle_id,
            input.driver_id,
            trimmed(input.status.as_deref()),
            input.from_date,
            input.to_date,
        )
        .await
        .map_err(|_| dependency_failure("Vehicle logs could not be loaded."))?;
        Ok(ListVehicleLogsOutput {
            logs: PaginatedVehicleDailyLogsResponse {
                logs: logs
                    .into_iter()
                    .map(VehicleDailyLogResponse::from)
                    .collect(),
            },
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadVehicleLogInput {
    log_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct ReadVehicleLogOutput {
    log: VehicleDailyLogResponse,
}

pub(super) struct FleetVehicleLogReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl FleetVehicleLogReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "fleet.vehicle_logs.read",
                "Read vehicle log",
                "Returns one tenant vehicle log by its stable identifier.",
                json!({ "log_id": { "type": "string", "format": "uuid" } }),
                json!({ "log": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "fleet.vehicle_logs",
            ),
        }
    }
}

#[async_trait]
impl Capability for FleetVehicleLogReadCapability {
    type Input = ReadVehicleLogInput;
    type Output = ReadVehicleLogOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("vehicle_log", input.log_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let log = VehicleDailyLogOps::get_by_id(
            &self.pool,
            context.principal().tenant_id(),
            input.log_id,
        )
        .await
        .map_err(|_| dependency_failure("The vehicle log could not be loaded."))?
        .ok_or_else(|| not_found("The vehicle log was not found."))?;
        Ok(ReadVehicleLogOutput {
            log: VehicleDailyLogResponse::from(log),
        })
    }
}

fn fleet_list_descriptor(
    key: &str,
    title: &str,
    description: &str,
    result_key: &str,
    sensitivity: DataSensitivity,
    usage_tag: &str,
) -> CapabilityDescriptor {
    read_descriptor(
        key,
        title,
        description,
        json!({
            "page": { "type": ["integer", "null"], "minimum": 1 },
            "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
            "search": { "type": ["string", "null"], "maxLength": 200 },
            "status": { "type": ["string", "null"], "maxLength": 50 }
        }),
        json!({
            result_key: { "type": "object" },
            "pagination": { "type": "object" }
        }),
        sensitivity,
        usage_tag,
    )
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).max(1),
        per_page.unwrap_or(20).clamp(1, 100),
    )
}

fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn resource_scope(kind: &str, id: Uuid) -> CapabilityScope {
    CapabilityScope::resources([CapabilityResource::parse(kind, id.to_string())
        .unwrap_or_else(|error| panic!("invalid built-in capability resource: {error}"))])
    .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
}

fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}

fn not_found(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::InvalidState, message)
}

#[cfg(test)]
mod tests {
    use super::{bounded_page, trimmed};

    #[test]
    fn fleet_filters_are_bounded_and_blank_values_are_ignored() {
        assert_eq!(bounded_page(Some(-2), Some(900)), (1, 100));
        assert_eq!(bounded_page(None, None), (1, 20));
        assert_eq!(trimmed(Some("  active  ")), Some("active"));
        assert_eq!(trimmed(Some("   ")), None);
    }
}
