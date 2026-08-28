//! Exposes canonical Timetabling reads through the Agent broker.

use async_trait::async_trait;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::PaginationMeta;
use cp_timetabling::{
    models::{TimetableRun, TimetableRunSummary},
    ops::TimetablingOps,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::administration::read_descriptor;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EmptyInput {}

#[derive(Serialize)]
pub(super) struct TimetableConfigurationOutput {
    configuration: cp_timetabling::models::TimetableConfiguration,
}

pub(super) struct TimetableConfigurationCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl TimetableConfigurationCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "timetabling.configuration.read",
                "Read timetable configuration",
                "Returns scheduling settings hydrated from canonical Academics records.",
                json!({}),
                json!({ "configuration": { "type": "object" } }),
                DataSensitivity::Personal,
                "timetabling.configuration",
            ),
        }
    }
}

#[async_trait]
impl Capability for TimetableConfigurationCapability {
    type Input = EmptyInput;
    type Output = TimetableConfigurationOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, _input: &Self::Input) -> CapabilityScope {
        CapabilityScope::TenantWide
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        _input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let configuration =
            TimetablingOps::get_configuration(&self.pool, context.principal().tenant_id())
                .await
                .map_err(|_| dependency_failure("Timetable configuration could not be loaded."))?;
        Ok(TimetableConfigurationOutput { configuration })
    }
}

#[derive(Serialize)]
pub(super) struct LatestTimetableRunOutput {
    run: Option<TimetableRun>,
}

pub(super) struct LatestTimetableRunCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LatestTimetableRunCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "timetabling.runs.read_latest",
                "Read latest timetable run",
                "Returns the latest generated or published timetable snapshot.",
                json!({}),
                json!({ "run": { "type": ["object", "null"] } }),
                DataSensitivity::Personal,
                "timetabling.runs",
            ),
        }
    }
}

#[async_trait]
impl Capability for LatestTimetableRunCapability {
    type Input = EmptyInput;
    type Output = LatestTimetableRunOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, _input: &Self::Input) -> CapabilityScope {
        CapabilityScope::TenantWide
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        _input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let run = TimetablingOps::latest_run(&self.pool, context.principal().tenant_id())
            .await
            .map_err(|_| dependency_failure("The latest timetable run could not be loaded."))?;
        Ok(LatestTimetableRunOutput { run })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListTimetableRunsInput {
    page: Option<i64>,
    per_page: Option<i64>,
    status: Option<String>,
}

#[derive(Serialize)]
pub(super) struct ListTimetableRunsOutput {
    runs: Vec<TimetableRunSummary>,
    pagination: PaginationMeta,
}

pub(super) struct TimetableRunsListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl TimetableRunsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "timetabling.runs.list",
                "List timetable runs",
                "Returns generated timetable runs using bounded pagination and an optional status filter.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "status": { "type": ["string", "null"], "enum": ["draft", "published", "superseded", null] }
                }),
                json!({ "runs": { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::Personal,
                "timetabling.runs",
            ),
        }
    }
}

#[async_trait]
impl Capability for TimetableRunsListCapability {
    type Input = ListTimetableRunsInput;
    type Output = ListTimetableRunsOutput;

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
        let per_page = input.per_page.unwrap_or(20).clamp(1, 100);
        let status = input
            .status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let (runs, total) = TimetablingOps::list_runs(
            &self.pool,
            context.principal().tenant_id(),
            page,
            per_page,
            status,
        )
        .await
        .map_err(|_| dependency_failure("Timetable runs could not be loaded."))?;
        Ok(ListTimetableRunsOutput {
            runs,
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadTimetableRunInput {
    run_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct ReadTimetableRunOutput {
    run: TimetableRun,
}

pub(super) struct TimetableRunReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl TimetableRunReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "timetabling.runs.read",
                "Read timetable run",
                "Returns one immutable generated timetable snapshot by its stable identifier.",
                json!({ "run_id": { "type": "string", "format": "uuid" } }),
                json!({ "run": { "type": "object" } }),
                DataSensitivity::Personal,
                "timetabling.runs",
            ),
        }
    }
}

#[async_trait]
impl Capability for TimetableRunReadCapability {
    type Input = ReadTimetableRunInput;
    type Output = ReadTimetableRunOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        CapabilityScope::resources([CapabilityResource::parse(
            "timetable_run",
            input.run_id.to_string(),
        )
        .unwrap_or_else(|error| panic!("invalid built-in capability resource: {error}"))])
        .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let run =
            TimetablingOps::get_run(&self.pool, context.principal().tenant_id(), input.run_id)
                .await
                .map_err(|_| dependency_failure("The timetable run could not be loaded."))?
                .ok_or_else(|| {
                    CapabilityExecutionError::new(
                        CapabilityExecutionErrorCode::InvalidState,
                        "The timetable run was not found.",
                    )
                })?;
        Ok(ReadTimetableRunOutput { run })
    }
}

fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}
