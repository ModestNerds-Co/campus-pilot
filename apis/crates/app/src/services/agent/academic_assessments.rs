//! Agent read adapters for Academics assessment structures.

use async_trait::async_trait;
use cp_academics::assessments::{AssessmentComponentOps, AssessmentCycleOps};
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AssessmentCyclesListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<String>,
    academic_term_id: Option<Uuid>,
}

pub(super) struct AssessmentCyclesListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl AssessmentCyclesListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "academics.assessment_cycles.list",
                "List assessment cycles",
                "Returns term-scoped assessment cycles using bounded filters.",
                json!({
                    "page": page_schema(),
                    "per_page": per_page_schema(),
                    "search": search_schema(),
                    "status": { "type": ["string", "null"], "enum": ["draft", "open", "closed", null] },
                    "academic_term_id": nullable_uuid_schema()
                }),
                json!({
                    "assessment_cycles": { "type": "array" },
                    "pagination": { "type": "object" }
                }),
                DataSensitivity::General,
                "academics.assessment_cycles",
            ),
        }
    }
}

#[async_trait]
impl Capability for AssessmentCyclesListCapability {
    type Input = AssessmentCyclesListInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        input
            .academic_term_id
            .map_or(CapabilityScope::TenantWide, |id| {
                resource_scope("academic_term", id)
            })
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let (page, per_page) = bounded_page(input.page, input.per_page);
        let (rows, total) = AssessmentCycleOps::list(
            &self.pool,
            context.principal().tenant_id(),
            page,
            per_page,
            trimmed(input.search.as_deref()),
            trimmed(input.status.as_deref()),
            input.academic_term_id,
        )
        .await
        .map_err(|_| dependency_failure("Assessment cycles could not be loaded."))?;
        Ok(json!({
            "assessment_cycles": rows,
            "pagination": PaginationMeta::new(page as u32, per_page as u32, total)
        }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AssessmentRecordInput {
    record_id: Uuid,
}

pub(super) struct AssessmentCycleReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl AssessmentCycleReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "academics.assessment_cycles.read",
                "Read assessment cycle",
                "Returns one authorized assessment cycle by stable identifier.",
                json!({ "record_id": { "type": "string", "format": "uuid" } }),
                json!({ "record": { "type": "object" } }),
                DataSensitivity::General,
                "academics.assessment_cycles",
            ),
        }
    }
}

#[async_trait]
impl Capability for AssessmentCycleReadCapability {
    type Input = AssessmentRecordInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("assessment_cycle", input.record_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let record = AssessmentCycleOps::get_by_id(
            &self.pool,
            context.principal().tenant_id(),
            input.record_id,
        )
        .await
        .map_err(|_| dependency_failure("The assessment cycle could not be loaded."))?
        .ok_or_else(|| not_found("The assessment cycle was not found."))?;
        Ok(json!({ "record": record }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AssessmentComponentsListInput {
    assessment_cycle_id: Uuid,
    page: Option<i64>,
    per_page: Option<i64>,
    status: Option<String>,
    teaching_assignment_id: Option<Uuid>,
}

pub(super) struct AssessmentComponentsListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl AssessmentComponentsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "academics.assessment_components.list",
                "List assessment components",
                "Returns the weighted components for one assessment cycle.",
                json!({
                    "assessment_cycle_id": { "type": "string", "format": "uuid" },
                    "page": page_schema(),
                    "per_page": per_page_schema(),
                    "status": { "type": ["string", "null"], "enum": ["active", "inactive", null] },
                    "teaching_assignment_id": nullable_uuid_schema()
                }),
                json!({
                    "assessment_components": { "type": "array" },
                    "pagination": { "type": "object" }
                }),
                DataSensitivity::Personal,
                "academics.assessment_components",
            ),
        }
    }
}

#[async_trait]
impl Capability for AssessmentComponentsListCapability {
    type Input = AssessmentComponentsListInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        let mut resources = vec![resource("assessment_cycle", input.assessment_cycle_id)];
        if let Some(id) = input.teaching_assignment_id {
            resources.push(resource("teaching_assignment", id));
        }
        CapabilityScope::resources(resources)
            .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let (page, per_page) = bounded_page(input.page, input.per_page);
        let (rows, total) = AssessmentComponentOps::list(
            &self.pool,
            context.principal().tenant_id(),
            input.assessment_cycle_id,
            page,
            per_page,
            trimmed(input.status.as_deref()),
            input.teaching_assignment_id,
        )
        .await
        .map_err(|_| dependency_failure("Assessment components could not be loaded."))?;
        Ok(json!({
            "assessment_components": rows,
            "pagination": PaginationMeta::new(page as u32, per_page as u32, total)
        }))
    }
}

pub(super) struct AssessmentComponentReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl AssessmentComponentReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "academics.assessment_components.read",
                "Read assessment component",
                "Returns one authorized assessment component by stable identifier.",
                json!({ "record_id": { "type": "string", "format": "uuid" } }),
                json!({ "record": { "type": "object" } }),
                DataSensitivity::Personal,
                "academics.assessment_components",
            ),
        }
    }
}

#[async_trait]
impl Capability for AssessmentComponentReadCapability {
    type Input = AssessmentRecordInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("assessment_component", input.record_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let record = AssessmentComponentOps::get_by_id(
            &self.pool,
            context.principal().tenant_id(),
            input.record_id,
        )
        .await
        .map_err(|_| dependency_failure("The assessment component could not be loaded."))?
        .ok_or_else(|| not_found("The assessment component was not found."))?;
        Ok(json!({ "record": record }))
    }
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

fn resource_scope(kind: &str, id: Uuid) -> CapabilityScope {
    CapabilityScope::resources([resource(kind, id)])
        .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
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

fn nullable_uuid_schema() -> Value {
    json!({ "type": ["string", "null"], "format": "uuid" })
}
