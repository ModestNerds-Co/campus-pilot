//! Exposes canonical Attendance reads through the Agent broker.

use async_trait::async_trait;
use chrono::NaiveDate;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_attendance::{
    AttendanceAccessScope, AttendanceExceptionListQuery, AttendanceExceptionMark,
    AttendanceExceptionResponse, AttendanceExceptionStatus, AttendanceLessonSessionListQuery,
    AttendanceLessonSessionStatus, AttendanceLessonSessionSummary, AttendanceOps, AttendancePeriod,
    AttendanceReferenceData, AttendanceRegisterListQuery, AttendanceRegisterResponse,
    AttendanceRegisterStatus, AttendanceRegisterSummary, LearnerAttendanceHistoryQuery,
    LearnerAttendanceHistoryResponse,
};
use cp_common::{EffectiveRecordScope, PaginationMeta, RecordScopeFamilyKey};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::services::{access::ops::AccessOps, users::ops::UserOps};

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
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let references = AttendanceOps::reference_data(&self.pool, principal.tenant_id(), scope)
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
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let (registers, total) =
            AttendanceOps::list(&self.pool, principal.tenant_id(), &query, scope)
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
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let register =
            AttendanceOps::get(&self.pool, principal.tenant_id(), input.register_id, scope)
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadLearnerAttendanceHistoryInput {
    learner_id: Uuid,
    page: Option<i64>,
    per_page: Option<i64>,
    date_from: Option<NaiveDate>,
    date_to: Option<NaiveDate>,
}

#[derive(Serialize)]
pub(super) struct ReadLearnerAttendanceHistoryOutput {
    history: LearnerAttendanceHistoryResponse,
    pagination: PaginationMeta,
}

pub(super) struct AttendanceLearnerHistoryCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl AttendanceLearnerHistoryCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "attendance.learners.history.read",
                "Read learner attendance history",
                "Returns accepted submitted attendance for one learner within the current account's class scope.",
                json!({
                    "learner_id": { "type": "string", "format": "uuid" },
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "date_from": { "type": ["string", "null"], "format": "date" },
                    "date_to": { "type": ["string", "null"], "format": "date" }
                }),
                json!({ "history": { "type": "object" }, "pagination": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "attendance.learners",
            ),
        }
    }
}

#[async_trait]
impl Capability for AttendanceLearnerHistoryCapability {
    type Input = ReadLearnerAttendanceHistoryInput;
    type Output = ReadLearnerAttendanceHistoryOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        CapabilityScope::resources([CapabilityResource::parse(
            "learner",
            input.learner_id.to_string(),
        )
        .unwrap_or_else(|error| panic!("invalid built-in capability resource: {error}"))])
        .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let page = input.page.unwrap_or(1).max(1);
        let per_page = input.per_page.unwrap_or(25).clamp(1, 100);
        let query = LearnerAttendanceHistoryQuery {
            page: Some(page),
            per_page: Some(per_page),
            date_from: input.date_from,
            date_to: input.date_to,
        };
        let (history, total) = AttendanceOps::learner_history(
            &self.pool,
            principal.tenant_id(),
            input.learner_id,
            &query,
            scope,
        )
        .await
        .map_err(|_| dependency_failure("Learner attendance history could not be loaded."))?
        .ok_or_else(|| invalid_state("Learner attendance history is unavailable."))?;
        Ok(ReadLearnerAttendanceHistoryOutput {
            history,
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListAttendanceLessonSessionsInput {
    page: Option<i64>,
    per_page: Option<i64>,
    date_from: Option<NaiveDate>,
    date_to: Option<NaiveDate>,
    class_group_id: Option<Uuid>,
    status: Option<AttendanceLessonSessionStatus>,
}

#[derive(Serialize)]
pub(super) struct ListAttendanceLessonSessionsOutput {
    sessions: Vec<AttendanceLessonSessionSummary>,
    pagination: PaginationMeta,
}

pub(super) struct AttendanceLessonSessionsListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl AttendanceLessonSessionsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "attendance.lesson_sessions.list",
                "List Attendance lesson sessions",
                "Returns timetable-linked lesson sessions within the current account's Attendance scope.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "date_from": { "type": ["string", "null"], "format": "date" },
                    "date_to": { "type": ["string", "null"], "format": "date" },
                    "class_group_id": { "type": ["string", "null"], "format": "uuid" },
                    "status": { "type": ["string", "null"], "enum": ["scheduled", "open", "completed", "cancelled", null] }
                }),
                json!({ "sessions": { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::Personal,
                "attendance.lesson_sessions",
            ),
        }
    }
}

#[async_trait]
impl Capability for AttendanceLessonSessionsListCapability {
    type Input = ListAttendanceLessonSessionsInput;
    type Output = ListAttendanceLessonSessionsOutput;

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
        let query = AttendanceLessonSessionListQuery {
            page: Some(page),
            per_page: Some(per_page),
            date_from: input.date_from,
            date_to: input.date_to,
            class_group_id: input.class_group_id,
            status: input.status,
        };
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let (sessions, total) =
            AttendanceOps::list_lesson_sessions(&self.pool, principal.tenant_id(), &query, scope)
                .await
                .map_err(|_| {
                    dependency_failure("Attendance lesson sessions could not be loaded.")
                })?;
        Ok(ListAttendanceLessonSessionsOutput {
            sessions,
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadAttendanceLessonSessionInput {
    lesson_session_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct ReadAttendanceLessonSessionOutput {
    session: AttendanceLessonSessionSummary,
}

pub(super) struct AttendanceLessonSessionReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl AttendanceLessonSessionReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "attendance.lesson_sessions.read",
                "Read Attendance lesson session",
                "Returns one timetable-linked lesson session and its register state.",
                json!({ "lesson_session_id": { "type": "string", "format": "uuid" } }),
                json!({ "session": { "type": "object" } }),
                DataSensitivity::Personal,
                "attendance.lesson_sessions",
            ),
        }
    }
}

#[async_trait]
impl Capability for AttendanceLessonSessionReadCapability {
    type Input = ReadAttendanceLessonSessionInput;
    type Output = ReadAttendanceLessonSessionOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        CapabilityScope::resources([CapabilityResource::parse(
            "attendance_lesson_session",
            input.lesson_session_id.to_string(),
        )
        .unwrap_or_else(|error| panic!("invalid built-in capability resource: {error}"))])
        .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let session = AttendanceOps::get_lesson_session(
            &self.pool,
            principal.tenant_id(),
            input.lesson_session_id,
            scope,
        )
        .await
        .map_err(|_| dependency_failure("The Attendance lesson session could not be loaded."))?
        .ok_or_else(|| invalid_state("The Attendance lesson session is unavailable."))?;
        Ok(ReadAttendanceLessonSessionOutput { session })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListAttendanceExceptionsInput {
    page: Option<i64>,
    per_page: Option<i64>,
    date_from: Option<NaiveDate>,
    date_to: Option<NaiveDate>,
    class_group_id: Option<Uuid>,
    status: Option<AttendanceExceptionStatus>,
    mark: Option<AttendanceExceptionMark>,
}

#[derive(Serialize)]
pub(super) struct ListAttendanceExceptionsOutput {
    exceptions: Vec<AttendanceExceptionResponse>,
    pagination: PaginationMeta,
}

pub(super) struct AttendanceExceptionsListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl AttendanceExceptionsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "attendance.exceptions.list",
                "List Attendance exceptions",
                "Returns the campus absence and lateness follow-up queue. It does not claim that notifications were sent.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "date_from": { "type": ["string", "null"], "format": "date" },
                    "date_to": { "type": ["string", "null"], "format": "date" },
                    "class_group_id": { "type": ["string", "null"], "format": "uuid" },
                    "status": { "type": ["string", "null"], "enum": ["open", "acknowledged", "resolved", null] },
                    "mark": { "type": ["string", "null"], "enum": ["absent", "late", "excused", null] }
                }),
                json!({ "exceptions": { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "attendance.exceptions",
            ),
        }
    }
}

#[async_trait]
impl Capability for AttendanceExceptionsListCapability {
    type Input = ListAttendanceExceptionsInput;
    type Output = ListAttendanceExceptionsOutput;

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
        let query = AttendanceExceptionListQuery {
            page: Some(page),
            per_page: Some(per_page),
            date_from: input.date_from,
            date_to: input.date_to,
            class_group_id: input.class_group_id,
            status: input.status,
            mark: input.mark,
        };
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let (exceptions, total) =
            AttendanceOps::list_exceptions(&self.pool, principal.tenant_id(), &query, scope)
                .await
                .map_err(|_| dependency_failure("Attendance exceptions could not be loaded."))?;
        Ok(ListAttendanceExceptionsOutput {
            exceptions,
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadAttendanceExceptionInput {
    exception_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct ReadAttendanceExceptionOutput {
    exception: AttendanceExceptionResponse,
}

pub(super) struct AttendanceExceptionReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl AttendanceExceptionReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "attendance.exceptions.read",
                "Read Attendance exception",
                "Returns one campus Attendance exception with its current follow-up state.",
                json!({ "exception_id": { "type": "string", "format": "uuid" } }),
                json!({ "exception": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "attendance.exceptions",
            ),
        }
    }
}

#[async_trait]
impl Capability for AttendanceExceptionReadCapability {
    type Input = ReadAttendanceExceptionInput;
    type Output = ReadAttendanceExceptionOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        CapabilityScope::resources([CapabilityResource::parse(
            "attendance_exception",
            input.exception_id.to_string(),
        )
        .unwrap_or_else(|error| panic!("invalid built-in capability resource: {error}"))])
        .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let exception = AttendanceOps::get_exception(
            &self.pool,
            principal.tenant_id(),
            input.exception_id,
            scope,
        )
        .await
        .map_err(|_| dependency_failure("The Attendance exception could not be loaded."))?
        .ok_or_else(|| invalid_state("The Attendance exception is unavailable."))?;
        Ok(ReadAttendanceExceptionOutput { exception })
    }
}

async fn current_scope(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<AttendanceAccessScope, CapabilityExecutionError> {
    let user = UserOps::get_user_by_id(pool, tenant_id, user_id)
        .await
        .map_err(|_| dependency_failure("Current Attendance authority could not be loaded."))?
        .filter(|user| user.is_active)
        .ok_or_else(|| invalid_state("The current Attendance account is unavailable."))?;
    let access = AccessOps::effective_access(pool, tenant_id, &user.roles)
        .await
        .map_err(|_| dependency_failure("Current Attendance access could not be loaded."))?;
    let family = RecordScopeFamilyKey::parse("attendance.registers")
        .map_err(|_| invalid_state("The Attendance record scope is invalid."))?;
    match access.record_scopes.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Ok(AttendanceAccessScope::Campus),
        Some(EffectiveRecordScope::Assigned | EffectiveRecordScope::SelfAndAssigned) => {
            Ok(AttendanceAccessScope::AssignedTo(user_id))
        }
        Some(EffectiveRecordScope::SelfRecord) | None => {
            Err(invalid_state("Attendance record scope is unavailable."))
        }
    }
}

fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}

fn invalid_state(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::InvalidState, message)
}
