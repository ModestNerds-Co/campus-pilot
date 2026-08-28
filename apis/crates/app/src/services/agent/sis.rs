//! Adapts SIS people and admissions reads to typed Agent capabilities.
//!
//! Handlers use the same tenant-scoped SIS operations as HTTP routes. Model
//! input can filter records but cannot supply tenant or authenticated identity.

use async_trait::async_trait;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::PaginationMeta;
use cp_sis::{
    dtos::{AccountProfileKind, GuardianResponse, LearnerResponse},
    ops::{
        AccountCandidateOps, ApplicationOps, EnrolmentOps, GuardianOps, GuardianRelationshipOps,
        LearnerOps,
    },
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::administration::read_descriptor;

#[derive(Debug, Clone, Copy)]
pub(super) enum SisListKind {
    Learners,
    Guardians,
    GuardianRelationships,
    Applications,
    Enrolments,
}

impl SisListKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::Learners => "sis.learners.list",
            Self::Guardians => "sis.guardians.list",
            Self::GuardianRelationships => "sis.guardian_relationships.list",
            Self::Applications => "sis.applications.list",
            Self::Enrolments => "sis.enrolments.list",
        }
    }

    const fn title(self) -> &'static str {
        match self {
            Self::Learners => "List learners",
            Self::Guardians => "List guardians",
            Self::GuardianRelationships => "List guardian relationships",
            Self::Applications => "List applications",
            Self::Enrolments => "List enrolments",
        }
    }

    const fn result_key(self) -> &'static str {
        match self {
            Self::Learners => "learners",
            Self::Guardians => "guardians",
            Self::GuardianRelationships => "relationships",
            Self::Applications => "applications",
            Self::Enrolments => "enrolments",
        }
    }

    const fn sensitivity(self) -> DataSensitivity {
        match self {
            Self::Applications | Self::Enrolments => DataSensitivity::Sensitive,
            Self::Learners | Self::Guardians | Self::GuardianRelationships => {
                DataSensitivity::Personal
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SisListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<String>,
    learner_id: Option<Uuid>,
    guardian_id: Option<Uuid>,
    academic_year_id: Option<Uuid>,
    target_grade_level_id: Option<Uuid>,
    class_group_id: Option<Uuid>,
}

pub(super) struct SisListCapability {
    pool: PgPool,
    kind: SisListKind,
    descriptor: CapabilityDescriptor,
}

impl SisListCapability {
    pub(super) fn new(pool: PgPool, kind: SisListKind) -> Self {
        let properties = match kind {
            SisListKind::Learners => json!({
                "page": page_schema(), "per_page": per_page_schema(), "search": search_schema(),
                "status": { "type": ["string", "null"], "enum": ["prospective", "active", "inactive", "graduated", "withdrawn", null] }
            }),
            SisListKind::Guardians => json!({
                "page": page_schema(), "per_page": per_page_schema(), "search": search_schema(),
                "status": active_status_schema()
            }),
            SisListKind::GuardianRelationships => json!({
                "page": page_schema(), "per_page": per_page_schema(), "search": search_schema(),
                "status": active_status_schema(), "learner_id": nullable_uuid_schema(),
                "guardian_id": nullable_uuid_schema()
            }),
            SisListKind::Applications => json!({
                "page": page_schema(), "per_page": per_page_schema(), "search": search_schema(),
                "status": { "type": ["string", "null"], "enum": ["draft", "submitted", "under_review", "offered", "accepted", "rejected", "withdrawn", null] },
                "academic_year_id": nullable_uuid_schema(),
                "target_grade_level_id": nullable_uuid_schema(),
                "learner_id": nullable_uuid_schema()
            }),
            SisListKind::Enrolments => json!({
                "page": page_schema(), "per_page": per_page_schema(), "search": search_schema(),
                "status": { "type": ["string", "null"], "enum": ["active", "completed", "withdrawn", null] },
                "academic_year_id": nullable_uuid_schema(), "class_group_id": nullable_uuid_schema(),
                "learner_id": nullable_uuid_schema()
            }),
        };
        Self {
            pool,
            kind,
            descriptor: read_descriptor(
                kind.operation_key(),
                kind.title(),
                "Returns authorized SIS records using bounded filters.",
                properties,
                json!({ kind.result_key(): { "type": "array" }, "pagination": { "type": "object" } }),
                kind.sensitivity(),
                match kind {
                    SisListKind::Learners => "sis.learners",
                    SisListKind::Guardians => "sis.guardians",
                    SisListKind::GuardianRelationships => "sis.guardian_relationships",
                    SisListKind::Applications => "sis.applications",
                    SisListKind::Enrolments => "sis.enrolments",
                },
            ),
        }
    }
}

#[async_trait]
impl Capability for SisListCapability {
    type Input = SisListInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        let resources = [
            input.learner_id.map(|id| resource("learner", id)),
            input.guardian_id.map(|id| resource("guardian", id)),
            input
                .academic_year_id
                .map(|id| resource("academic_year", id)),
            input.class_group_id.map(|id| resource("class", id)),
            input
                .target_grade_level_id
                .map(|id| resource("academic_grade_level", id)),
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
            SisListKind::Learners => {
                let (rows, total) = LearnerOps::list(
                    &self.pool,
                    tenant_id,
                    page,
                    per_page,
                    trimmed(input.search.as_deref()),
                    status,
                )
                .await
                .map_err(|_| dependency_failure("Learners could not be loaded."))?;
                Ok(list_output(
                    "learners",
                    rows.into_iter().map(LearnerResponse::from).collect(),
                    page,
                    per_page,
                    total,
                ))
            }
            SisListKind::Guardians => {
                let (rows, total) = GuardianOps::list(
                    &self.pool,
                    tenant_id,
                    page,
                    per_page,
                    trimmed(input.search.as_deref()),
                    status,
                )
                .await
                .map_err(|_| dependency_failure("Guardians could not be loaded."))?;
                Ok(list_output(
                    "guardians",
                    rows.into_iter().map(GuardianResponse::from).collect(),
                    page,
                    per_page,
                    total,
                ))
            }
            SisListKind::GuardianRelationships => {
                let (rows, total) = GuardianRelationshipOps::list(
                    &self.pool,
                    tenant_id,
                    page,
                    per_page,
                    trimmed(input.search.as_deref()),
                    status,
                    input.learner_id,
                    input.guardian_id,
                )
                .await
                .map_err(|_| dependency_failure("Guardian relationships could not be loaded."))?;
                Ok(list_output("relationships", rows, page, per_page, total))
            }
            SisListKind::Applications => {
                let (rows, total) = ApplicationOps::list(
                    &self.pool,
                    tenant_id,
                    page,
                    per_page,
                    trimmed(input.search.as_deref()),
                    status,
                    input.academic_year_id,
                    input.target_grade_level_id,
                    input.learner_id,
                )
                .await
                .map_err(|_| dependency_failure("Applications could not be loaded."))?;
                Ok(list_output("applications", rows, page, per_page, total))
            }
            SisListKind::Enrolments => {
                let (rows, total) = EnrolmentOps::list(
                    &self.pool,
                    tenant_id,
                    page,
                    per_page,
                    trimmed(input.search.as_deref()),
                    status,
                    input.academic_year_id,
                    input.class_group_id,
                    input.learner_id,
                )
                .await
                .map_err(|_| dependency_failure("Enrolments could not be loaded."))?;
                Ok(list_output("enrolments", rows, page, per_page, total))
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum SisReadKind {
    Learner,
    Guardian,
    GuardianRelationship,
    Application,
    Enrolment,
}

impl SisReadKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::Learner => "sis.learners.read",
            Self::Guardian => "sis.guardians.read",
            Self::GuardianRelationship => "sis.guardian_relationships.read",
            Self::Application => "sis.applications.read",
            Self::Enrolment => "sis.enrolments.read",
        }
    }

    const fn title(self) -> &'static str {
        match self {
            Self::Learner => "Read learner",
            Self::Guardian => "Read guardian",
            Self::GuardianRelationship => "Read guardian relationship",
            Self::Application => "Read application",
            Self::Enrolment => "Read enrolment",
        }
    }

    const fn resource_kind(self) -> &'static str {
        match self {
            Self::Learner => "learner",
            Self::Guardian => "guardian",
            Self::GuardianRelationship => "guardian_relationship",
            Self::Application => "application",
            Self::Enrolment => "enrolment",
        }
    }

    const fn sensitivity(self) -> DataSensitivity {
        match self {
            Self::Application | Self::Enrolment => DataSensitivity::Sensitive,
            Self::Learner | Self::Guardian | Self::GuardianRelationship => {
                DataSensitivity::Personal
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SisReadInput {
    record_id: Uuid,
}

pub(super) struct SisReadCapability {
    pool: PgPool,
    kind: SisReadKind,
    descriptor: CapabilityDescriptor,
}

impl SisReadCapability {
    pub(super) fn new(pool: PgPool, kind: SisReadKind) -> Self {
        Self {
            pool,
            kind,
            descriptor: read_descriptor(
                kind.operation_key(),
                kind.title(),
                "Returns one authorized SIS record by stable identifier.",
                json!({ "record_id": { "type": "string", "format": "uuid" } }),
                json!({ "record": { "type": "object" } }),
                kind.sensitivity(),
                "sis.records",
            ),
        }
    }
}

#[async_trait]
impl Capability for SisReadCapability {
    type Input = SisReadInput;
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
            SisReadKind::Learner => LearnerOps::get_by_id(&self.pool, tenant_id, input.record_id)
                .await
                .map_err(|_| dependency_failure("The learner could not be loaded."))?
                .map(LearnerResponse::from)
                .map(|value| json!(value)),
            SisReadKind::Guardian => GuardianOps::get_by_id(&self.pool, tenant_id, input.record_id)
                .await
                .map_err(|_| dependency_failure("The guardian could not be loaded."))?
                .map(GuardianResponse::from)
                .map(|value| json!(value)),
            SisReadKind::GuardianRelationship => {
                GuardianRelationshipOps::get_by_id(&self.pool, tenant_id, input.record_id)
                    .await
                    .map_err(|_| {
                        dependency_failure("The guardian relationship could not be loaded.")
                    })?
                    .map(|value| json!(value))
            }
            SisReadKind::Application => {
                ApplicationOps::get_by_id(&self.pool, tenant_id, input.record_id)
                    .await
                    .map_err(|_| dependency_failure("The application could not be loaded."))?
                    .map(|value| json!(value))
            }
            SisReadKind::Enrolment => {
                EnrolmentOps::get_by_id(&self.pool, tenant_id, input.record_id)
                    .await
                    .map_err(|_| dependency_failure("The enrolment could not be loaded."))?
                    .map(|value| json!(value))
            }
        }
        .ok_or_else(|| not_found("The SIS record was not found."))?;
        Ok(json!({ "record": record }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AccountCandidatesInput {
    profile_kind: AccountProfileKind,
    profile_id: Option<Uuid>,
    search: Option<String>,
}

pub(super) struct AccountCandidatesCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl AccountCandidatesCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "sis.account_candidates.list",
                "List SIS account candidates",
                "Returns active system accounts available for a learner or guardian link.",
                json!({
                    "profile_kind": { "type": "string", "enum": ["learner", "guardian"] },
                    "profile_id": nullable_uuid_schema(),
                    "search": search_schema()
                }),
                json!({ "accounts": { "type": "array" } }),
                DataSensitivity::Personal,
                "sis.account_candidates",
            ),
        }
    }
}

#[async_trait]
impl Capability for AccountCandidatesCapability {
    type Input = AccountCandidatesInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        input.profile_id.map_or(CapabilityScope::TenantWide, |id| {
            CapabilityScope::resources([resource(
                match input.profile_kind {
                    AccountProfileKind::Learner => "learner",
                    AccountProfileKind::Guardian => "guardian",
                },
                id,
            )])
            .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
        })
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let accounts = AccountCandidateOps::list(
            &self.pool,
            context.principal().tenant_id(),
            input.profile_kind,
            input.profile_id,
            trimmed(input.search.as_deref()),
        )
        .await
        .map_err(|_| dependency_failure("Account candidates could not be loaded."))?;
        Ok(json!({ "accounts": accounts }))
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
    use super::{SisListKind, bounded_page, trimmed};

    #[test]
    fn sis_capability_filters_are_bounded_and_keys_are_stable() {
        assert_eq!(bounded_page(Some(0), Some(500)), (1, 100));
        assert_eq!(trimmed(Some("  Applicant ")), Some("Applicant"));
        assert_eq!(
            SisListKind::Enrolments.operation_key(),
            "sis.enrolments.list"
        );
    }
}
