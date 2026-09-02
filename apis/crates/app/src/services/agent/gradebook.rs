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
    GradebookAccessScope, GradebookMarkImportOps, GradebookMarkImportPreview,
    GradebookMarkImportRecord, GradebookOps, GradebookReferenceData, GradebookSheetListQuery,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListMarkImportsInput {
    mark_sheet_id: Uuid,
    page: Option<i64>,
    per_page: Option<i64>,
}

#[derive(Serialize)]
pub(super) struct ListMarkImportsOutput {
    imports: Vec<AgentGradebookMarkImportRecord>,
    pagination: PaginationMeta,
}

#[derive(Debug, Serialize)]
struct AgentGradebookMarkImportRecord {
    id: Uuid,
    mark_sheet_id: Uuid,
    file_name: String,
    source_format: String,
    source_size_bytes: i64,
    source_row_count: i32,
    status: String,
    created_at: String,
    latest_preview_id: Option<Uuid>,
    mapping_version: Option<i32>,
    ready_rows: Option<i32>,
    invalid_rows: Option<i32>,
    duplicate_rows: Option<i32>,
    updated_rows: Option<i32>,
    skipped_rows: Option<i32>,
    failed_rows: Option<i32>,
    committed_at: Option<String>,
}

fn agent_mark_import(record: GradebookMarkImportRecord) -> AgentGradebookMarkImportRecord {
    AgentGradebookMarkImportRecord {
        id: record.id,
        mark_sheet_id: record.mark_sheet_id,
        file_name: record.file_name,
        source_format: record.source_format,
        source_size_bytes: record.source_size_bytes,
        source_row_count: record.source_row_count,
        status: record.status,
        created_at: record.created_at.to_rfc3339(),
        latest_preview_id: record.latest_preview_id,
        mapping_version: record.mapping_version,
        ready_rows: record.ready_rows,
        invalid_rows: record.invalid_rows,
        duplicate_rows: record.duplicate_rows,
        updated_rows: record.updated_rows,
        skipped_rows: record.skipped_rows,
        failed_rows: record.failed_rows,
        committed_at: record.committed_at.map(|value| value.to_rfc3339()),
    }
}

pub(super) struct GradebookMarkImportsListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl GradebookMarkImportsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "academics.gradebook.mark_imports.list",
                "List mark imports",
                "Returns retained import metadata and normalized preview totals for one mark sheet.",
                json!({
                    "mark_sheet_id": { "type": "string", "format": "uuid" },
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 }
                }),
                json!({ "imports": { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::Personal,
                "academics.gradebook",
            ),
        }
    }
}

#[async_trait]
impl Capability for GradebookMarkImportsListCapability {
    type Input = ListMarkImportsInput;
    type Output = ListMarkImportsOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        mark_import_scope(input.mark_sheet_id, None)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let page = input.page.unwrap_or(1).max(1);
        let per_page = input.per_page.unwrap_or(25).clamp(1, 100);
        let (imports, total) = GradebookMarkImportOps::list(
            &self.pool,
            context.principal().tenant_id(),
            input.mark_sheet_id,
            page,
            per_page,
        )
        .await
        .map_err(|_| dependency_failure("Mark imports could not be loaded."))?;
        Ok(ListMarkImportsOutput {
            imports: imports.into_iter().map(agent_mark_import).collect(),
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadMarkImportInput {
    mark_sheet_id: Uuid,
    import_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct ReadMarkImportOutput {
    mark_import: AgentGradebookMarkImportRecord,
}

pub(super) struct GradebookMarkImportReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl GradebookMarkImportReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "academics.gradebook.mark_imports.read",
                "Read mark import",
                "Returns retained import metadata without raw source bytes or source-row values.",
                json!({
                    "mark_sheet_id": { "type": "string", "format": "uuid" },
                    "import_id": { "type": "string", "format": "uuid" }
                }),
                json!({ "mark_import": { "type": "object" } }),
                DataSensitivity::Personal,
                "academics.gradebook",
            ),
        }
    }
}

#[async_trait]
impl Capability for GradebookMarkImportReadCapability {
    type Input = ReadMarkImportInput;
    type Output = ReadMarkImportOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        mark_import_scope(input.mark_sheet_id, Some(input.import_id))
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let mark_import = GradebookMarkImportOps::get(
            &self.pool,
            context.principal().tenant_id(),
            input.mark_sheet_id,
            input.import_id,
        )
        .await
        .map_err(|_| dependency_failure("The mark import could not be loaded."))?
        .ok_or_else(|| {
            CapabilityExecutionError::new(
                CapabilityExecutionErrorCode::InvalidState,
                "The mark import was not found.",
            )
        })?;
        Ok(ReadMarkImportOutput {
            mark_import: agent_mark_import(mark_import),
        })
    }
}

#[derive(Serialize)]
pub(super) struct ReadMarkImportPreviewOutput {
    preview: GradebookMarkImportPreview,
    pagination: PaginationMeta,
}

pub(super) struct GradebookMarkImportPreviewCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl GradebookMarkImportPreviewCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "academics.gradebook.mark_imports.preview.read",
                "Read mark import preview",
                "Returns normalized learner and mark values plus validation issues; raw source rows are excluded.",
                json!({
                    "mark_sheet_id": { "type": "string", "format": "uuid" },
                    "import_id": { "type": "string", "format": "uuid" },
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 }
                }),
                json!({ "preview": { "type": "object" }, "pagination": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "academics.gradebook",
            ),
        }
    }
}

#[async_trait]
impl Capability for GradebookMarkImportPreviewCapability {
    type Input = ListMarkImportsInputWithImport;
    type Output = ReadMarkImportPreviewOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        mark_import_scope(input.mark_sheet_id, Some(input.import_id))
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let page = input.page.unwrap_or(1).max(1);
        let per_page = input.per_page.unwrap_or(25).clamp(1, 100);
        let preview = GradebookMarkImportOps::preview(
            &self.pool,
            context.principal().tenant_id(),
            input.mark_sheet_id,
            input.import_id,
            page,
            per_page,
        )
        .await
        .map_err(|_| dependency_failure("The mark import preview could not be loaded."))?
        .ok_or_else(|| {
            CapabilityExecutionError::new(
                CapabilityExecutionErrorCode::InvalidState,
                "The mark import preview was not found.",
            )
        })?;
        Ok(ReadMarkImportPreviewOutput {
            pagination: PaginationMeta::new(page as u32, per_page as u32, preview.total_rows),
            preview,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListMarkImportsInputWithImport {
    mark_sheet_id: Uuid,
    import_id: Uuid,
    page: Option<i64>,
    per_page: Option<i64>,
}

fn mark_import_scope(mark_sheet_id: Uuid, import_id: Option<Uuid>) -> CapabilityScope {
    let mut resources = vec![
        CapabilityResource::parse("assessment_mark_sheet", mark_sheet_id.to_string())
            .unwrap_or_else(|error| panic!("invalid built-in capability resource: {error}")),
    ];
    if let Some(import_id) = import_id {
        resources.push(
            CapabilityResource::parse("data_import", import_id.to_string())
                .unwrap_or_else(|error| panic!("invalid built-in capability resource: {error}")),
        );
    }
    CapabilityScope::resources(resources)
        .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
}

fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::to_value;

    use super::*;

    #[test]
    fn agent_mark_import_metadata_omits_source_headers_and_content_type() {
        let projected = agent_mark_import(GradebookMarkImportRecord {
            id: Uuid::new_v4(),
            mark_sheet_id: Uuid::new_v4(),
            file_name: "marks.xlsx".to_string(),
            content_type: "application/private".to_string(),
            source_format: "xlsx".to_string(),
            source_size_bytes: 128,
            source_row_count: 2,
            source_headers: vec!["Private source heading".to_string()],
            status: "uploaded".to_string(),
            created_at: Utc::now(),
            latest_preview_id: None,
            mapping_version: None,
            ready_rows: None,
            invalid_rows: None,
            duplicate_rows: None,
            updated_rows: None,
            skipped_rows: None,
            failed_rows: None,
            committed_at: None,
        });
        let value =
            to_value(projected).unwrap_or_else(|error| panic!("projection failed: {error}"));
        assert!(value.get("source_headers").is_none());
        assert!(value.get("content_type").is_none());
        assert_eq!(value["source_format"], "xlsx");
    }
}
