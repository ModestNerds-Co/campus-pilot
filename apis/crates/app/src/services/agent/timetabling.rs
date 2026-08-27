//! Exposes canonical Timetabling reads through the Agent broker.

use async_trait::async_trait;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityScope, DataSensitivity,
};
use cp_timetabling::{models::TimetableRun, ops::TimetablingOps};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;

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

fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}
