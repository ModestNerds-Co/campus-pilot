//! Exposes canonical Gradebook reads through the Agent broker.
//!
//! Exact mark-sheet reads are sensitive and carry a tenant-proven resource
//! scope. The normal HTTP route applies assigned-teacher visibility; Agent
//! discovery remains withheld until broker authority carries the same current
//! role-scope evidence into these handlers.

use async_trait::async_trait;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::PaginationMeta;
use cp_gradebook::{
    GradebookAccessScope, GradebookOps, GradebookReferenceData, GradebookSheetListQuery,
    GradebookSheetResponse, GradebookSheetStatus, GradebookSheetSummary,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::administration::read_descriptor;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EmptyGradebookInput {}

#[derive(Serialize)]
pub(super) struct GradebookReferencesOutput {
    references: GradebookReferenceData,
}

pub(super) struct GradebookReferencesCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl GradebookReferencesCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "academics.gradebook.references.read",
                "Read Gradebook references",
                "Returns assessment components and their current mark-sheet state.",
                json!({}),
                json!({ "references": { "type": "object" } }),
                DataSensitivity::Personal,
                "academics.gradebook",
            ),
        }
    }
}

#[async_trait]
impl Capability for GradebookReferencesCapability {
    type Input = EmptyGradebookInput;
    type Output = GradebookReferencesOutput;

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
        let references = GradebookOps::reference_data(
            &self.pool,
            context.principal().tenant_id(),
            GradebookAccessScope::Campus,
        )
        .await
        .map_err(|_| dependency_failure("Gradebook references could not be loaded."))?;
        Ok(GradebookReferencesOutput { references })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListMarkSheetsInput {
    page: Option<i64>,
    per_page: Option<i64>,
    status: Option<GradebookSheetStatus>,
}

#[derive(Serialize)]
pub(super) struct ListMarkSheetsOutput {
    mark_sheets: Vec<GradebookSheetSummary>,
    pagination: PaginationMeta,
}

pub(super) struct GradebookMarkSheetsListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl GradebookMarkSheetsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "academics.gradebook.mark_sheets.list",
                "List assessment mark sheets",
                "Returns bounded mark-sheet summaries and their review state.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "status": { "type": ["string", "null"], "enum": ["draft", "submitted", "published", null] }
                }),
                json!({ "mark_sheets": { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::Personal,
                "academics.gradebook",
            ),
        }
    }
}

#[async_trait]
impl Capability for GradebookMarkSheetsListCapability {
    type Input = ListMarkSheetsInput;
    type Output = ListMarkSheetsOutput;

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
        let query = GradebookSheetListQuery {
            page: Some(page),
            per_page: Some(per_page),
            status: input.status,
        };
        let (data, total) = GradebookOps::list(
            &self.pool,
            context.principal().tenant_id(),
            &query,
            GradebookAccessScope::Campus,
        )
        .await
        .map_err(|_| dependency_failure("Assessment mark sheets could not be loaded."))?;
        Ok(ListMarkSheetsOutput {
            mark_sheets: data.mark_sheets,
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadMarkSheetInput {
    mark_sheet_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct ReadMarkSheetOutput {
    mark_sheet: GradebookSheetResponse,
}

pub(super) struct GradebookMarkSheetReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl GradebookMarkSheetReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "academics.gradebook.mark_sheets.read",
                "Read assessment mark sheet",
                "Returns one mark sheet with learner marks and publication state.",
                json!({ "mark_sheet_id": { "type": "string", "format": "uuid" } }),
                json!({ "mark_sheet": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "academics.gradebook",
            ),
        }
    }
}

#[async_trait]
impl Capability for GradebookMarkSheetReadCapability {
    type Input = ReadMarkSheetInput;
    type Output = ReadMarkSheetOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        CapabilityScope::resources([CapabilityResource::parse(
            "assessment_mark_sheet",
            input.mark_sheet_id.to_string(),
        )
        .unwrap_or_else(|error| panic!("invalid built-in capability resource: {error}"))])
        .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let mark_sheet = GradebookOps::get(
            &self.pool,
            context.principal().tenant_id(),
            input.mark_sheet_id,
            GradebookAccessScope::Campus,
        )
        .await
        .map_err(|_| dependency_failure("The assessment mark sheet could not be loaded."))?
        .ok_or_else(|| {
            CapabilityExecutionError::new(
                CapabilityExecutionErrorCode::InvalidState,
                "The assessment mark sheet was not found.",
            )
        })?;
        Ok(ReadMarkSheetOutput { mark_sheet })
    }
}

fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}
