//! Exposes canonical Attendance reads through the Agent broker.

use async_trait::async_trait;
use chrono::NaiveDate;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_attendance::{
    AttendanceOps, AttendancePeriod, AttendanceReferenceData, AttendanceRegisterListQuery,
    AttendanceRegisterResponse, AttendanceRegisterStatus, AttendanceRegisterSummary,
};
use cp_common::PaginationMeta;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::administration::read_descriptor;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EmptyInput {}

#[derive(Serialize)]
pub(super) struct AttendanceReferencesOutput {
    references: Option<AttendanceReferenceData>,
}

pub(super) struct AttendanceReferencesCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl AttendanceReferencesCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "attendance.references.read",
                "Read attendance references",
                "Returns the active academic term and classes available for attendance registers.",
                json!({}),
                json!({ "references": { "type": ["object", "null"] } }),
                DataSensitivity::General,
                "attendance.references",
            ),
        }
    }
}

#[async_trait]
impl Capability for AttendanceReferencesCapability {
    type Input = EmptyInput;
    type Output = AttendanceReferencesOutput;

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
        let references = AttendanceOps::reference_data(&self.pool, context.principal().tenant_id())
            .await
            .map_err(|_| dependency_failure("Attendance references could not be loaded."))?;
        Ok(AttendanceReferencesOutput { references })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListAttendanceRegistersInput {
    page: Option<i64>,
    per_page: Option<i64>,
    date_from: Option<NaiveDate>,
    date_to: Option<NaiveDate>,
    class_group_id: Option<Uuid>,
    period: Option<AttendancePeriod>,
    status: Option<AttendanceRegisterStatus>,
}

#[derive(Serialize)]
pub(super) struct ListAttendanceRegistersOutput {
    registers: Vec<AttendanceRegisterSummary>,
    pagination: PaginationMeta,
}

pub(super) struct AttendanceRegistersListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl AttendanceRegistersListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "attendance.registers.list",
                "List attendance registers",
                "Returns attendance registers using bounded pagination and optional operational filters.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "date_from": { "type": ["string", "null"], "format": "date" },
                    "date_to": { "type": ["string", "null"], "format": "date" },
                    "class_group_id": { "type": ["string", "null"], "format": "uuid" },
                    "period": { "type": ["string", "null"], "enum": ["full_day", "morning", "afternoon", null] },
                    "status": { "type": ["string", "null"], "enum": ["draft", "submitted", null] }
                }),
                json!({ "registers": { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::Personal,
                "attendance.registers",
            ),
        }
    }
}

#[async_trait]
impl Capability for AttendanceRegistersListCapability {
    type Input = ListAttendanceRegistersInput;
    type Output = ListAttendanceRegistersOutput;

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
        let query = AttendanceRegisterListQuery {
            page: Some(page),
            per_page: Some(per_page),
            date_from: input.date_from,
            date_to: input.date_to,
            class_group_id: input.class_group_id,
            period: input.period,
            status: input.status,
        };
        let (registers, total) =
            AttendanceOps::list(&self.pool, context.principal().tenant_id(), &query)
                .await
                .map_err(|_| dependency_failure("Attendance registers could not be loaded."))?;
        Ok(ListAttendanceRegistersOutput {
            registers,
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadAttendanceRegisterInput {
    register_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct ReadAttendanceRegisterOutput {
    register: AttendanceRegisterResponse,
}

pub(super) struct AttendanceRegisterReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl AttendanceRegisterReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "attendance.registers.read",
                "Read attendance register",
                "Returns one register with its learner marks and current lifecycle state.",
                json!({ "register_id": { "type": "string", "format": "uuid" } }),
                json!({ "register": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "attendance.registers",
            ),
        }
    }
}

#[async_trait]
impl Capability for AttendanceRegisterReadCapability {
    type Input = ReadAttendanceRegisterInput;
    type Output = ReadAttendanceRegisterOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        CapabilityScope::resources([CapabilityResource::parse(
            "attendance_register",
            input.register_id.to_string(),
        )
        .unwrap_or_else(|error| panic!("invalid built-in capability resource: {error}"))])
        .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let register = AttendanceOps::get(
            &self.pool,
            context.principal().tenant_id(),
            input.register_id,
        )
        .await
        .map_err(|_| dependency_failure("The attendance register could not be loaded."))?
        .ok_or_else(|| {
            CapabilityExecutionError::new(
                CapabilityExecutionErrorCode::InvalidState,
                "The attendance register was not found.",
            )
        })?;
        Ok(ReadAttendanceRegisterOutput { register })
    }
}

fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}
