//! Exposes canonical Academic Progress and Reporting reads through the Agent broker.
//!
//! These adapters are classified and executable in the diagnostic registry,
//! but discovery remains withheld until broker authority carries the same
//! campus, assigned-teacher, and self scope used by the HTTP routes.

use async_trait::async_trait;
use cp_academic_reporting::{
    AcademicReportBatchListQuery, AcademicReportBatchResponse, AcademicReportBatchStatus,
    AcademicReportReferenceData, AcademicReportingAccessScope, AcademicReportingOps,
    AcademicTranscriptResponse, GradingSchemeResponse, PaginatedAcademicReportBatchesResponse,
};
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::PaginationMeta;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::administration::read_descriptor;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EmptyReportingInput {}

#[derive(Serialize)]
pub(super) struct ReportingReferencesOutput {
    references: AcademicReportReferenceData,
}

pub(super) struct ReportingReferencesCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl ReportingReferencesCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "academics.reporting.references.read",
                "Read reporting references",
                "Returns report-ready assessment cycles, classes, grading schemes, and grade levels.",
                json!({}),
                json!({ "references": { "type": "object" } }),
                DataSensitivity::Personal,
                "academics.reporting",
            ),
        }
    }
}

#[async_trait]
impl Capability for ReportingReferencesCapability {
    type Input = EmptyReportingInput;
    type Output = ReportingReferencesOutput;

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
        let references = AcademicReportingOps::reference_data(
            &self.pool,
            context.principal().tenant_id(),
            AcademicReportingAccessScope::Campus,
        )
        .await
        .map_err(|_| dependency_failure("Academic reporting references could not be loaded."))?;
        Ok(ReportingReferencesOutput { references })
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum GradingSchemeStatusFilter {
    Active,
    Retired,
}

impl GradingSchemeStatusFilter {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Retired => "retired",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListGradingSchemesInput {
    status: Option<GradingSchemeStatusFilter>,
}

#[derive(Serialize)]
pub(super) struct ListGradingSchemesOutput {
    grading_schemes: Vec<GradingSchemeResponse>,
}

pub(super) struct GradingSchemesListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl GradingSchemesListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "academics.reporting.grading_schemes.list",
                "List grading schemes",
                "Returns the campus grading policies and their percentage bands.",
                json!({ "status": { "type": ["string", "null"], "enum": ["active", "retired", null] } }),
                json!({ "grading_schemes": { "type": "array" } }),
                DataSensitivity::General,
                "academics.reporting",
            ),
        }
    }
}

#[async_trait]
impl Capability for GradingSchemesListCapability {
    type Input = ListGradingSchemesInput;
    type Output = ListGradingSchemesOutput;

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
        let status = input.status.map(GradingSchemeStatusFilter::as_str);
        let grading_schemes = AcademicReportingOps::list_grading_schemes(
            &self.pool,
            context.principal().tenant_id(),
            status,
        )
        .await
        .map_err(|_| dependency_failure("Grading schemes could not be loaded."))?;
        Ok(ListGradingSchemesOutput { grading_schemes })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadGradingSchemeInput {
    grading_scheme_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct ReadGradingSchemeOutput {
    grading_scheme: GradingSchemeResponse,
}

pub(super) struct GradingSchemeReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl GradingSchemeReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "academics.reporting.grading_schemes.read",
                "Read grading scheme",
                "Returns one campus grading policy with its ordered percentage bands.",
                json!({ "grading_scheme_id": { "type": "string", "format": "uuid" } }),
                json!({ "grading_scheme": { "type": "object" } }),
                DataSensitivity::General,
                "academics.reporting",
            ),
        }
    }
}

#[async_trait]
impl Capability for GradingSchemeReadCapability {
    type Input = ReadGradingSchemeInput;
    type Output = ReadGradingSchemeOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("academic_grading_scheme", input.grading_scheme_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let grading_scheme = AcademicReportingOps::get_grading_scheme(
            &self.pool,
            context.principal().tenant_id(),
            input.grading_scheme_id,
        )
        .await
        .map_err(|_| dependency_failure("The grading scheme could not be loaded."))?
        .ok_or_else(|| not_found("The grading scheme was not found."))?;
        Ok(ReadGradingSchemeOutput { grading_scheme })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListReportBatchesInput {
    page: Option<i64>,
    per_page: Option<i64>,
    status: Option<AcademicReportBatchStatus>,
}

#[derive(Serialize)]
pub(super) struct ListReportBatchesOutput {
    report_batches: PaginatedAcademicReportBatchesResponse,
    pagination: PaginationMeta,
}

pub(super) struct ReportBatchesListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl ReportBatchesListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "academics.reporting.report_batches.list",
                "List academic reports",
                "Returns bounded academic report batches and their review state.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "status": { "type": ["string", "null"], "enum": ["draft", "reviewed", "published", null] }
                }),
                json!({ "report_batches": { "type": "object" }, "pagination": { "type": "object" } }),
                DataSensitivity::Personal,
                "academics.reporting",
            ),
        }
    }
}

#[async_trait]
impl Capability for ReportBatchesListCapability {
    type Input = ListReportBatchesInput;
    type Output = ListReportBatchesOutput;

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
        let query = AcademicReportBatchListQuery {
            page: Some(page),
            per_page: Some(per_page),
            status: input.status,
        };
        let (report_batches, total) = AcademicReportingOps::list_report_batches(
            &self.pool,
            context.principal().tenant_id(),
            &query,
            AcademicReportingAccessScope::Campus,
        )
        .await
        .map_err(|_| dependency_failure("Academic reports could not be loaded."))?;
        Ok(ListReportBatchesOutput {
            report_batches,
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadReportBatchInput {
    report_batch_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct ReadReportBatchOutput {
    report_batch: AcademicReportBatchResponse,
}

pub(super) struct ReportBatchReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl ReportBatchReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "academics.reporting.report_batches.read",
                "Read academic report",
                "Returns one report batch with learner results, attendance, remarks, and progression state.",
                json!({ "report_batch_id": { "type": "string", "format": "uuid" } }),
                json!({ "report_batch": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "academics.reporting",
            ),
        }
    }
}

#[async_trait]
impl Capability for ReportBatchReadCapability {
    type Input = ReadReportBatchInput;
    type Output = ReadReportBatchOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("academic_report_batch", input.report_batch_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let report_batch = AcademicReportingOps::get_report_batch(
            &self.pool,
            context.principal().tenant_id(),
            input.report_batch_id,
            AcademicReportingAccessScope::Campus,
        )
        .await
        .map_err(|_| dependency_failure("The academic report could not be loaded."))?
        .ok_or_else(|| not_found("The academic report was not found."))?;
        Ok(ReadReportBatchOutput { report_batch })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadTranscriptInput {
    learner_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct ReadTranscriptOutput {
    transcript: AcademicTranscriptResponse,
}

pub(super) struct TranscriptReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl TranscriptReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "academics.reporting.transcripts.read",
                "Read learner transcript",
                "Returns one learner's published academic results across reporting periods.",
                json!({ "learner_id": { "type": "string", "format": "uuid" } }),
                json!({ "transcript": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "academics.reporting",
            ),
        }
    }
}

#[async_trait]
impl Capability for TranscriptReadCapability {
    type Input = ReadTranscriptInput;
    type Output = ReadTranscriptOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("learner", input.learner_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let transcript = AcademicReportingOps::learner_transcript(
            &self.pool,
            context.principal().tenant_id(),
            input.learner_id,
            AcademicReportingAccessScope::Campus,
        )
        .await
        .map_err(|_| dependency_failure("The learner transcript could not be loaded."))?
        .ok_or_else(|| not_found("The learner transcript was not found."))?;
        Ok(ReadTranscriptOutput { transcript })
    }
}

fn resource_scope(kind: &'static str, id: Uuid) -> CapabilityScope {
    CapabilityScope::resources([CapabilityResource::parse(kind, id.to_string())
        .unwrap_or_else(|error| panic!("invalid built-in reporting resource: {error}"))])
    .unwrap_or_else(|error| panic!("invalid built-in reporting scope: {error}"))
}

fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}

fn not_found(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::InvalidState, message)
}
