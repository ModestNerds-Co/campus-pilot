//! Adapts canonical Academics reads to typed Agent capabilities.
//!
//! Handlers call the same Academics operations as HTTP routes and never accept
//! tenant or authenticated-person identity from model input.

use async_trait::async_trait;
use cp_academics::{
    dtos::{
        AcademicYearResponse, ClassGroupResponse, SubjectResponse, TeacherProfileResponse,
        TeachingAssignmentResponse,
    },
    ops::{AcademicYearOps, ClassGroupOps, SubjectOps, TeacherProfileOps, TeachingAssignmentOps},
};
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::PaginationMeta;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::administration::read_descriptor;

#[derive(Debug, Clone, Copy)]
pub(super) enum AcademicsListKind {
    AcademicYears,
    Subjects,
    Teachers,
    Classes,
    TeachingAssignments,
}

impl AcademicsListKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::AcademicYears => "academics.academic_years.list",
            Self::Subjects => "academics.subjects.list",
            Self::Teachers => "academics.teachers.list",
            Self::Classes => "academics.classes.list",
            Self::TeachingAssignments => "academics.teaching_assignments.list",
        }
    }

    const fn title(self) -> &'static str {
        match self {
            Self::AcademicYears => "List academic years",
            Self::Subjects => "List subjects",
            Self::Teachers => "List teachers",
            Self::Classes => "List classes",
            Self::TeachingAssignments => "List teaching assignments",
        }
    }

    const fn result_key(self) -> &'static str {
        match self {
            Self::AcademicYears => "academic_years",
            Self::Subjects => "subjects",
            Self::Teachers => "teachers",
            Self::Classes => "classes",
            Self::TeachingAssignments => "assignments",
        }
    }

    const fn sensitivity(self) -> DataSensitivity {
        match self {
            Self::Teachers | Self::TeachingAssignments => DataSensitivity::Personal,
            Self::AcademicYears | Self::Subjects | Self::Classes => DataSensitivity::General,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AcademicsListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<String>,
    academic_year_id: Option<Uuid>,
    class_group_id: Option<Uuid>,
    teacher_profile_id: Option<Uuid>,
}

pub(super) struct AcademicsListCapability {
    pool: PgPool,
    kind: AcademicsListKind,
    descriptor: CapabilityDescriptor,
}

impl AcademicsListCapability {
    pub(super) fn new(pool: PgPool, kind: AcademicsListKind) -> Self {
        let filter_properties = match kind {
            AcademicsListKind::AcademicYears => json!({
                "page": page_schema(), "per_page": per_page_schema(),
                "search": search_schema(),
                "status": { "type": ["string", "null"], "enum": ["planned", "active", "closed", null] }
            }),
            AcademicsListKind::Subjects | AcademicsListKind::Teachers => json!({
                "page": page_schema(), "per_page": per_page_schema(),
                "search": search_schema(),
                "status": active_status_schema()
            }),
            AcademicsListKind::Classes => json!({
                "page": page_schema(), "per_page": per_page_schema(),
                "search": search_schema(), "status": active_status_schema(),
                "academic_year_id": nullable_uuid_schema()
            }),
            AcademicsListKind::TeachingAssignments => json!({
                "page": page_schema(), "per_page": per_page_schema(),
                "status": active_status_schema(),
                "academic_year_id": nullable_uuid_schema(),
                "class_group_id": nullable_uuid_schema(),
                "teacher_profile_id": nullable_uuid_schema()
            }),
        };
        Self {
            pool,
            kind,
            descriptor: read_descriptor(
                kind.operation_key(),
                kind.title(),
                "Returns authorized canonical academic records using bounded filters.",
                filter_properties,
                json!({
                    kind.result_key(): { "type": "array" },
                    "pagination": { "type": "object" }
                }),
                kind.sensitivity(),
                match kind {
                    AcademicsListKind::AcademicYears => "academics.academic_years",
                    AcademicsListKind::Subjects => "academics.subjects",
                    AcademicsListKind::Teachers => "academics.teachers",
                    AcademicsListKind::Classes => "academics.classes",
                    AcademicsListKind::TeachingAssignments => "academics.teaching_assignments",
                },
            ),
        }
    }
}

#[async_trait]
impl Capability for AcademicsListCapability {
    type Input = AcademicsListInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        let resources = [
            input
                .academic_year_id
                .map(|id| resource("academic_year", id)),
            input.class_group_id.map(|id| resource("class", id)),
            input.teacher_profile_id.map(|id| resource("teacher", id)),
        ]
        .into_iter()
        .flatten()
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
        let tenant_id = context.principal().tenant_id();
        let (page, per_page) = bounded_page(input.page, input.per_page);
        let status = trimmed(input.status.as_deref());
        match self.kind {
            AcademicsListKind::AcademicYears => {
                let (rows, total) = AcademicYearOps::list(
                    &self.pool,
                    tenant_id,
                    page,
                    per_page,
                    trimmed(input.search.as_deref()),
                    status,
                )
                .await
                .map_err(|_| dependency_failure("Academic years could not be loaded."))?;
                Ok(list_output(
                    "academic_years",
                    rows.into_iter()
                        .map(AcademicYearResponse::from)
                        .collect::<Vec<_>>(),
                    page,
                    per_page,
                    total,
                ))
            }
            AcademicsListKind::Subjects => {
                let (rows, total) = SubjectOps::list(
                    &self.pool,
                    tenant_id,
                    page,
                    per_page,
                    trimmed(input.search.as_deref()),
                    status,
                )
                .await
                .map_err(|_| dependency_failure("Subjects could not be loaded."))?;
                Ok(list_output(
                    "subjects",
                    rows.into_iter()
                        .map(SubjectResponse::from)
                        .collect::<Vec<_>>(),
                    page,
                    per_page,
                    total,
                ))
            }
            AcademicsListKind::Teachers => {
                let (rows, total) = TeacherProfileOps::list(
                    &self.pool,
                    tenant_id,
                    page,
                    per_page,
                    trimmed(input.search.as_deref()),
                    status,
                )
                .await
                .map_err(|_| dependency_failure("Teachers could not be loaded."))?;
                Ok(list_output(
                    "teachers",
                    rows.into_iter()
                        .map(TeacherProfileResponse::from)
                        .collect::<Vec<_>>(),
                    page,
                    per_page,
                    total,
                ))
            }
            AcademicsListKind::Classes => {
                let (rows, total) = ClassGroupOps::list(
                    &self.pool,
                    tenant_id,
                    page,
                    per_page,
                    trimmed(input.search.as_deref()),
                    status,
                    input.academic_year_id,
                )
                .await
                .map_err(|_| dependency_failure("Classes could not be loaded."))?;
                Ok(list_output(
                    "classes",
                    rows.into_iter()
                        .map(ClassGroupResponse::from)
                        .collect::<Vec<_>>(),
                    page,
                    per_page,
                    total,
                ))
            }
            AcademicsListKind::TeachingAssignments => {
                let (rows, total) = TeachingAssignmentOps::list(
                    &self.pool,
                    tenant_id,
                    page,
                    per_page,
                    status,
                    input.academic_year_id,
                    input.class_group_id,
                    input.teacher_profile_id,
                )
                .await
                .map_err(|_| dependency_failure("Teaching assignments could not be loaded."))?;
                Ok(list_output(
                    "assignments",
                    rows.into_iter()
                        .map(TeachingAssignmentResponse::from)
                        .collect::<Vec<_>>(),
                    page,
                    per_page,
                    total,
                ))
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum AcademicsReadKind {
    AcademicYear,
    Subject,
    Teacher,
    Class,
    TeachingAssignment,
}

impl AcademicsReadKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::AcademicYear => "academics.academic_years.read",
            Self::Subject => "academics.subjects.read",
            Self::Teacher => "academics.teachers.read",
            Self::Class => "academics.classes.read",
            Self::TeachingAssignment => "academics.teaching_assignments.read",
        }
    }

    const fn title(self) -> &'static str {
        match self {
            Self::AcademicYear => "Read academic year",
            Self::Subject => "Read subject",
            Self::Teacher => "Read teacher",
            Self::Class => "Read class",
            Self::TeachingAssignment => "Read teaching assignment",
        }
    }

    const fn resource_kind(self) -> &'static str {
        match self {
            Self::AcademicYear => "academic_year",
            Self::Subject => "subject",
            Self::Teacher => "teacher",
            Self::Class => "class",
            Self::TeachingAssignment => "teaching_assignment",
        }
    }

    const fn sensitivity(self) -> DataSensitivity {
        match self {
            Self::Teacher | Self::TeachingAssignment => DataSensitivity::Personal,
            Self::AcademicYear | Self::Subject | Self::Class => DataSensitivity::General,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AcademicsReadInput {
    record_id: Uuid,
}

pub(super) struct AcademicsReadCapability {
    pool: PgPool,
    kind: AcademicsReadKind,
    descriptor: CapabilityDescriptor,
}

impl AcademicsReadCapability {
    pub(super) fn new(pool: PgPool, kind: AcademicsReadKind) -> Self {
        Self {
            pool,
            kind,
            descriptor: read_descriptor(
                kind.operation_key(),
                kind.title(),
                "Returns one authorized canonical academic record by stable identifier.",
                json!({ "record_id": { "type": "string", "format": "uuid" } }),
                json!({ "record": { "type": "object" } }),
                kind.sensitivity(),
                "academics.records",
            ),
        }
    }
}

#[async_trait]
impl Capability for AcademicsReadCapability {
    type Input = AcademicsReadInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        CapabilityScope::resources([resource(self.kind.resource_kind(), input.record_id)])
            .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let tenant_id = context.principal().tenant_id();
        let record = match self.kind {
            AcademicsReadKind::AcademicYear => {
                AcademicYearOps::get_by_id(&self.pool, tenant_id, input.record_id)
                    .await
                    .map_err(|_| dependency_failure("The academic year could not be loaded."))?
                    .map(AcademicYearResponse::from)
                    .map(|value| json!(value))
            }
            AcademicsReadKind::Subject => {
                SubjectOps::get_by_id(&self.pool, tenant_id, input.record_id)
                    .await
                    .map_err(|_| dependency_failure("The subject could not be loaded."))?
                    .map(SubjectResponse::from)
                    .map(|value| json!(value))
            }
            AcademicsReadKind::Teacher => {
                TeacherProfileOps::get_by_id(&self.pool, tenant_id, input.record_id)
                    .await
                    .map_err(|_| dependency_failure("The teacher could not be loaded."))?
                    .map(TeacherProfileResponse::from)
                    .map(|value| json!(value))
            }
            AcademicsReadKind::Class => {
                ClassGroupOps::get_by_id(&self.pool, tenant_id, input.record_id)
                    .await
                    .map_err(|_| dependency_failure("The class could not be loaded."))?
                    .map(ClassGroupResponse::from)
                    .map(|value| json!(value))
            }
            AcademicsReadKind::TeachingAssignment => {
                TeachingAssignmentOps::get_by_id(&self.pool, tenant_id, input.record_id)
                    .await
                    .map_err(|_| {
                        dependency_failure("The teaching assignment could not be loaded.")
                    })?
                    .map(TeachingAssignmentResponse::from)
                    .map(|value| json!(value))
            }
        }
        .ok_or_else(|| not_found("The academic record was not found."))?;
        Ok(json!({ "record": record }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TeacherCandidatesInput {
    search: Option<String>,
}

pub(super) struct TeacherCandidatesCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl TeacherCandidatesCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "academics.teacher_candidates.list",
                "List eligible teacher employees",
                "Returns active employees without an existing teacher profile.",
                json!({ "search": search_schema() }),
                json!({ "employees": { "type": "array" } }),
                DataSensitivity::Personal,
                "academics.teacher_candidates",
            ),
        }
    }
}

#[async_trait]
impl Capability for TeacherCandidatesCapability {
    type Input = TeacherCandidatesInput;
    type Output = Value;

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
        let employees = TeacherProfileOps::list_candidates(
            &self.pool,
            context.principal().tenant_id(),
            trimmed(input.search.as_deref()),
        )
        .await
        .map_err(|_| dependency_failure("Eligible teacher employees could not be loaded."))?;
        Ok(json!({ "employees": employees }))
    }
}

fn list_output<T: serde::Serialize>(
    key: &str,
    rows: Vec<T>,
    page: i64,
    per_page: i64,
    total: i64,
) -> Value {
    let mut value = serde_json::Map::new();
    value.insert(key.to_string(), json!(rows));
    value.insert(
        "pagination".to_string(),
        json!(PaginationMeta::new(page as u32, per_page as u32, total)),
    );
    Value::Object(value)
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).max(1),
        per_page.unwrap_or(25).clamp(1, 100),
    )
}

fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn resource(kind: &str, id: Uuid) -> CapabilityResource {
    CapabilityResource::parse(kind, id.to_string())
        .unwrap_or_else(|error| panic!("invalid built-in capability resource: {error}"))
}

fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}

fn not_found(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::InvalidState, message)
}

fn page_schema() -> Value {
    json!({ "type": ["integer", "null"], "minimum": 1 })
}

fn per_page_schema() -> Value {
    json!({ "type": ["integer", "null"], "minimum": 1, "maximum": 100 })
}

fn search_schema() -> Value {
    json!({ "type": ["string", "null"], "maxLength": 200 })
}

fn active_status_schema() -> Value {
    json!({ "type": ["string", "null"], "enum": ["active", "inactive", null] })
}

fn nullable_uuid_schema() -> Value {
    json!({ "type": ["string", "null"], "format": "uuid" })
}

#[cfg(test)]
mod tests {
    use super::{bounded_page, trimmed};

    #[test]
    fn filters_are_bounded_and_trimmed() {
        assert_eq!(bounded_page(Some(0), Some(500)), (1, 100));
        assert_eq!(trimmed(Some("  Science ")), Some("Science"));
        assert_eq!(trimmed(Some("")), None);
    }
}
