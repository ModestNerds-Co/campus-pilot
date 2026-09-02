//! Exposes record-scoped E-learning metadata through the Agent broker.

use async_trait::async_trait;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::{EffectiveRecordScope, PaginationMeta, RecordScopeFamilyKey};
use cp_learning::{
    LearningAccessScope, LearningAssignmentListQuery, LearningAssignmentResponse,
    LearningAssignmentStatus, LearningCompletionPage, LearningCompletionPolicyResponse,
    LearningOps, LearningProgressEntry, LearningQuizAttemptListQuery, LearningQuizAttemptResponse,
    LearningQuizAttemptStatus, LearningQuizListQuery, LearningQuizResponse, LearningQuizStatus,
    LearningReferenceData, LearningResourceFileQuery, LearningSettingsResponse,
    LearningSpaceListQuery, LearningSpaceResponse, LearningSpaceStatus, LearningSpaceSummary,
    LearningSubmissionListQuery, LearningSubmissionResponse, LearningSubmissionStatus,
};
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
pub(super) struct LearningSettingsOutput {
    settings: LearningSettingsResponse,
}

pub(super) struct LearningSettingsCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningSettingsCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.settings.read",
                "Read E-learning settings",
                "Returns the governed document series configured for E-learning resources.",
                json!({}),
                json!({ "settings": { "type": "object" } }),
                DataSensitivity::General,
                "learning.settings",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningSettingsCapability {
    type Input = EmptyInput;
    type Output = LearningSettingsOutput;

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
        let settings = LearningOps::settings(&self.pool, context.principal().tenant_id())
            .await
            .map_err(|_| dependency_failure("E-learning settings could not be loaded."))?;
        Ok(LearningSettingsOutput { settings })
    }
}

#[derive(Serialize)]
pub(super) struct LearningReferencesOutput {
    references: LearningReferenceData,
}

pub(super) struct LearningReferencesCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningReferencesCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.references.read",
                "Read E-learning references",
                "Returns the active term and teaching assignments visible to the current account.",
                json!({}),
                json!({ "references": { "type": "object" } }),
                DataSensitivity::Personal,
                "learning.references",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningReferencesCapability {
    type Input = EmptyInput;
    type Output = LearningReferencesOutput;

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
        let references = LearningOps::references(&self.pool, principal.tenant_id(), scope)
            .await
            .map_err(|_| dependency_failure("E-learning references could not be loaded."))?;
        Ok(LearningReferencesOutput { references })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LearningResourceFilesInput {
    search: Option<String>,
    limit: Option<i64>,
}

#[derive(Serialize)]
pub(super) struct LearningResourceFilesOutput {
    files: Vec<cp_document_registry::EvidenceFileReference>,
}

pub(super) struct LearningResourceFilesCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningResourceFilesCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.resource_files.list",
                "List E-learning resource files",
                "Returns governed, non-restricted file metadata available for a learning resource.",
                json!({
                    "search": { "type": ["string", "null"] },
                    "limit": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 }
                }),
                json!({ "files": { "type": "array" } }),
                DataSensitivity::Sensitive,
                "learning.resource_files",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningResourceFilesCapability {
    type Input = LearningResourceFilesInput;
    type Output = LearningResourceFilesOutput;

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
        let files = LearningOps::resource_file_candidates(
            &self.pool,
            context.principal().tenant_id(),
            &LearningResourceFileQuery {
                search: input.search,
                limit: input.limit.map(|limit| limit.clamp(1, 100)),
            },
        )
        .await
        .map_err(|_| dependency_failure("E-learning resource files could not be loaded."))?;
        Ok(LearningResourceFilesOutput { files })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LearningSpacesInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<LearningSpaceStatus>,
}

#[derive(Serialize)]
pub(super) struct LearningSpacesOutput {
    spaces: Vec<LearningSpaceSummary>,
    pagination: PaginationMeta,
}

pub(super) struct LearningSpacesCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningSpacesCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.spaces.list",
                "List E-learning spaces",
                "Returns only learning spaces visible through the current teaching assignment or learner enrolment.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "search": { "type": ["string", "null"] },
                    "status": { "type": ["string", "null"], "enum": ["draft", "published", "archived", null] }
                }),
                json!({ "spaces": { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::Personal,
                "learning.spaces",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningSpacesCapability {
    type Input = LearningSpacesInput;
    type Output = LearningSpacesOutput;

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
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let page = input.page.unwrap_or(1).max(1);
        let per_page = input.per_page.unwrap_or(25).clamp(1, 100);
        let (spaces, total) = LearningOps::list_spaces(
            &self.pool,
            principal.tenant_id(),
            scope,
            &LearningSpaceListQuery {
                page: Some(page),
                per_page: Some(per_page),
                search: input.search,
                status: input.status,
            },
        )
        .await
        .map_err(|_| dependency_failure("E-learning spaces could not be loaded."))?;
        Ok(LearningSpacesOutput {
            spaces,
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LearningSpaceInput {
    space_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct LearningSpaceOutput {
    space: LearningSpaceResponse,
}

pub(super) struct LearningSpaceCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningSpaceCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.spaces.read",
                "Read E-learning space",
                "Returns one visible learning space with its units and governed resource metadata.",
                json!({ "space_id": { "type": "string", "format": "uuid" } }),
                json!({ "space": { "type": "object" } }),
                DataSensitivity::Personal,
                "learning.spaces",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningSpaceCapability {
    type Input = LearningSpaceInput;
    type Output = LearningSpaceOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        CapabilityScope::resources([CapabilityResource::parse(
            "learning_space",
            input.space_id.to_string(),
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
        let space =
            LearningOps::get_space(&self.pool, principal.tenant_id(), input.space_id, scope)
                .await
                .map_err(|_| dependency_failure("The E-learning space could not be loaded."))?
                .ok_or_else(|| invalid_state("The E-learning space is unavailable."))?;
        Ok(LearningSpaceOutput { space })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LearningAssignmentsInput {
    space_id: Uuid,
    page: Option<i64>,
    per_page: Option<i64>,
    status: Option<LearningAssignmentStatus>,
}

#[derive(Serialize)]
pub(super) struct LearningAssignmentsOutput {
    assignments: Vec<LearningAssignmentResponse>,
    pagination: PaginationMeta,
}

pub(super) struct LearningAssignmentsCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningAssignmentsCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.assignments.list",
                "List E-learning assignments",
                "Returns bounded assignments visible through one current learning space.",
                json!({
                    "space_id": { "type": "string", "format": "uuid" },
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "status": { "type": ["string", "null"], "enum": ["draft", "published", "closed", null] }
                }),
                json!({ "assignments": { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::Personal,
                "learning.assignments",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningAssignmentsCapability {
    type Input = LearningAssignmentsInput;
    type Output = LearningAssignmentsOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("learning_space", input.space_id)
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
        let (assignments, total) = LearningOps::list_assignments(
            &self.pool,
            principal.tenant_id(),
            input.space_id,
            scope,
            &LearningAssignmentListQuery {
                page: Some(page),
                per_page: Some(per_page),
                status: input.status,
            },
        )
        .await
        .map_err(|_| dependency_failure("E-learning assignments could not be loaded."))?;
        Ok(LearningAssignmentsOutput {
            assignments,
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LearningAssignmentInput {
    assignment_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct LearningAssignmentOutput {
    assignment: LearningAssignmentResponse,
}

pub(super) struct LearningAssignmentCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningAssignmentCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.assignments.read",
                "Read E-learning assignment",
                "Returns one visible assignment with its immutable published rubric and lifecycle state.",
                json!({ "assignment_id": { "type": "string", "format": "uuid" } }),
                json!({ "assignment": { "type": "object" } }),
                DataSensitivity::Personal,
                "learning.assignments",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningAssignmentCapability {
    type Input = LearningAssignmentInput;
    type Output = LearningAssignmentOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("learning_assignment", input.assignment_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let assignment = LearningOps::get_assignment(
            &self.pool,
            principal.tenant_id(),
            input.assignment_id,
            scope,
        )
        .await
        .map_err(|_| dependency_failure("The E-learning assignment could not be loaded."))?
        .ok_or_else(|| invalid_state("The E-learning assignment is unavailable."))?;
        Ok(LearningAssignmentOutput { assignment })
    }
}

#[derive(Serialize)]
pub(super) struct LearningMineSubmissionOutput {
    submission: Option<LearningSubmissionResponse>,
}

pub(super) struct LearningMineSubmissionCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningMineSubmissionCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.submissions.mine.read",
                "Read my E-learning submission",
                "Returns only the authenticated learner's own draft, immutable attempts, and released feedback for one assignment.",
                json!({ "assignment_id": { "type": "string", "format": "uuid" } }),
                json!({ "submission": { "type": ["object", "null"] } }),
                DataSensitivity::Sensitive,
                "learning.submissions",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningMineSubmissionCapability {
    type Input = LearningAssignmentInput;
    type Output = LearningMineSubmissionOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("learning_assignment", input.assignment_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let submission = LearningOps::self_submission(
            &self.pool,
            principal.tenant_id(),
            input.assignment_id,
            scope,
        )
        .await
        .map_err(|_| dependency_failure("The learner submission could not be loaded."))?;
        Ok(LearningMineSubmissionOutput { submission })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LearningSubmissionsInput {
    assignment_id: Uuid,
    page: Option<i64>,
    per_page: Option<i64>,
    status: Option<LearningSubmissionStatus>,
}

#[derive(Serialize)]
pub(super) struct LearningSubmissionsOutput {
    submissions: Vec<LearningSubmissionResponse>,
    pagination: PaginationMeta,
}

pub(super) struct LearningSubmissionsCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningSubmissionsCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.submissions.list",
                "List E-learning submissions",
                "Returns bounded learner submission evidence for one assignment within assigned or campus teaching scope.",
                json!({
                    "assignment_id": { "type": "string", "format": "uuid" },
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "status": { "type": ["string", "null"], "enum": ["draft", "submitted", "revision_requested", "graded", null] }
                }),
                json!({ "submissions": { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "learning.submissions",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningSubmissionsCapability {
    type Input = LearningSubmissionsInput;
    type Output = LearningSubmissionsOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("learning_assignment", input.assignment_id)
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
        let (submissions, total) = LearningOps::list_submissions(
            &self.pool,
            principal.tenant_id(),
            input.assignment_id,
            scope,
            &LearningSubmissionListQuery {
                page: Some(page),
                per_page: Some(per_page),
                status: input.status,
            },
        )
        .await
        .map_err(|_| dependency_failure("E-learning submissions could not be loaded."))?;
        Ok(LearningSubmissionsOutput {
            submissions,
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LearningSubmissionInput {
    submission_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct LearningSubmissionOutput {
    submission: LearningSubmissionResponse,
}

pub(super) struct LearningSubmissionCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningSubmissionCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.submissions.read",
                "Read E-learning submission",
                "Returns one authorized learner submission with immutable attempts and visible feedback.",
                json!({ "submission_id": { "type": "string", "format": "uuid" } }),
                json!({ "submission": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "learning.submissions",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningSubmissionCapability {
    type Input = LearningSubmissionInput;
    type Output = LearningSubmissionOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("learning_submission", input.submission_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let submission = LearningOps::get_submission(
            &self.pool,
            principal.tenant_id(),
            input.submission_id,
            scope,
        )
        .await
        .map_err(|_| dependency_failure("The E-learning submission could not be loaded."))?
        .ok_or_else(|| invalid_state("The E-learning submission is unavailable."))?;
        Ok(LearningSubmissionOutput { submission })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LearningProgressInput {
    space_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct LearningMineProgressOutput {
    progress: LearningProgressEntry,
}

pub(super) struct LearningMineProgressCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningMineProgressCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.progress.mine.read",
                "Read my E-learning progress",
                "Returns derived assignment progress only for the authenticated learner in one visible space.",
                json!({ "space_id": { "type": "string", "format": "uuid" } }),
                json!({ "progress": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "learning.progress",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningMineProgressCapability {
    type Input = LearningProgressInput;
    type Output = LearningMineProgressOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("learning_space", input.space_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let progress =
            LearningOps::self_progress(&self.pool, principal.tenant_id(), input.space_id, scope)
                .await
                .map_err(|_| {
                    dependency_failure("Learner E-learning progress could not be loaded.")
                })?
                .ok_or_else(|| invalid_state("Learner E-learning progress is unavailable."))?;
        Ok(LearningMineProgressOutput { progress })
    }
}

#[derive(Serialize)]
pub(super) struct LearningProgressOutput {
    progress: Vec<LearningProgressEntry>,
}

pub(super) struct LearningProgressCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningProgressCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.progress.list",
                "List E-learning progress",
                "Returns derived learner progress within assigned or campus teaching scope for one space.",
                json!({ "space_id": { "type": "string", "format": "uuid" } }),
                json!({ "progress": { "type": "array" } }),
                DataSensitivity::Sensitive,
                "learning.progress",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningProgressCapability {
    type Input = LearningProgressInput;
    type Output = LearningProgressOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("learning_space", input.space_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let progress =
            LearningOps::list_progress(&self.pool, principal.tenant_id(), input.space_id, scope)
                .await
                .map_err(|_| dependency_failure("E-learning progress could not be loaded."))?;
        Ok(LearningProgressOutput { progress })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LearningQuizzesInput {
    space_id: Uuid,
    status: Option<LearningQuizStatus>,
    page: Option<i64>,
    per_page: Option<i64>,
}

#[derive(Serialize)]
pub(super) struct LearningQuizzesOutput {
    quizzes: Vec<LearningQuizResponse>,
    pagination: PaginationMeta,
}

pub(super) struct LearningQuizzesCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningQuizzesCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.quizzes.list",
                "List E-learning quizzes",
                "Returns quizzes in one visible Learning space. Answer keys are included only in assigned teacher or campus scope.",
                json!({
                    "space_id": { "type": "string", "format": "uuid" },
                    "status": { "type": ["string", "null"], "enum": ["draft", "published", "closed", null] },
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 }
                }),
                json!({ "quizzes": { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "learning.quizzes",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningQuizzesCapability {
    type Input = LearningQuizzesInput;
    type Output = LearningQuizzesOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("learning_space", input.space_id)
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
        let (quizzes, total) = LearningOps::list_quizzes(
            &self.pool,
            principal.tenant_id(),
            input.space_id,
            scope,
            &LearningQuizListQuery {
                page: Some(page),
                per_page: Some(per_page),
                status: input.status,
            },
        )
        .await
        .map_err(|_| dependency_failure("E-learning quizzes could not be loaded."))?;
        Ok(LearningQuizzesOutput {
            quizzes,
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LearningQuizInput {
    quiz_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct LearningQuizOutput {
    quiz: LearningQuizResponse,
}

pub(super) struct LearningQuizCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningQuizCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.quizzes.read",
                "Read an E-learning quiz",
                "Returns one visible quiz. Learner scope never receives correct-answer flags.",
                json!({ "quiz_id": { "type": "string", "format": "uuid" } }),
                json!({ "quiz": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "learning.quizzes",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningQuizCapability {
    type Input = LearningQuizInput;
    type Output = LearningQuizOutput;
    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("learning_quiz", input.quiz_id)
    }
    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let quiz = LearningOps::get_quiz(&self.pool, principal.tenant_id(), input.quiz_id, scope)
            .await
            .map_err(|_| dependency_failure("The E-learning quiz could not be loaded."))?
            .ok_or_else(|| invalid_state("The E-learning quiz is unavailable."))?;
        Ok(LearningQuizOutput { quiz })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LearningQuizAttemptsInput {
    quiz_id: Uuid,
    status: Option<LearningQuizAttemptStatus>,
    page: Option<i64>,
    per_page: Option<i64>,
}

#[derive(Serialize)]
pub(super) struct LearningQuizAttemptsOutput {
    attempts: Vec<LearningQuizAttemptResponse>,
    pagination: PaginationMeta,
}

pub(super) struct LearningQuizAttemptsCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningQuizAttemptsCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.quiz_attempts.list",
                "List E-learning quiz attempts",
                "Returns attempts within current learner-self, assigned-teacher, or campus scope.",
                json!({
                    "quiz_id": { "type": "string", "format": "uuid" },
                    "status": { "type": ["string", "null"], "enum": ["in_progress", "submitted", null] },
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 }
                }),
                json!({ "attempts": { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "learning.quiz_attempts",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningQuizAttemptsCapability {
    type Input = LearningQuizAttemptsInput;
    type Output = LearningQuizAttemptsOutput;
    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("learning_quiz", input.quiz_id)
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
        let (attempts, total) = LearningOps::list_quiz_attempts(
            &self.pool,
            principal.tenant_id(),
            input.quiz_id,
            scope,
            &LearningQuizAttemptListQuery {
                page: Some(page),
                per_page: Some(per_page),
                status: input.status,
            },
        )
        .await
        .map_err(|_| dependency_failure("E-learning quiz attempts could not be loaded."))?;
        Ok(LearningQuizAttemptsOutput {
            attempts,
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LearningQuizAttemptInput {
    attempt_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct LearningQuizAttemptOutput {
    attempt: LearningQuizAttemptResponse,
}

pub(super) struct LearningQuizAttemptCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningQuizAttemptCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.quiz_attempts.read",
                "Read an E-learning quiz attempt",
                "Returns one immutable or in-progress attempt through current record scope.",
                json!({ "attempt_id": { "type": "string", "format": "uuid" } }),
                json!({ "attempt": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "learning.quiz_attempts",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningQuizAttemptCapability {
    type Input = LearningQuizAttemptInput;
    type Output = LearningQuizAttemptOutput;
    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("learning_quiz_attempt", input.attempt_id)
    }
    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let attempt = LearningOps::get_quiz_attempt(
            &self.pool,
            principal.tenant_id(),
            input.attempt_id,
            scope,
        )
        .await
        .map_err(|_| dependency_failure("The E-learning quiz attempt could not be loaded."))?
        .ok_or_else(|| invalid_state("The E-learning quiz attempt is unavailable."))?;
        Ok(LearningQuizAttemptOutput { attempt })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LearningCompletionInput {
    space_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct LearningCompletionPolicyOutput {
    policy: LearningCompletionPolicyResponse,
}

pub(super) struct LearningCompletionPolicyCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningCompletionPolicyCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.completion_policy.read",
                "Read an E-learning completion policy",
                "Returns the editable draft for assigned staff, otherwise the current published completion policy.",
                json!({ "space_id": { "type": "string", "format": "uuid" } }),
                json!({ "policy": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "learning.completion",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningCompletionPolicyCapability {
    type Input = LearningCompletionInput;
    type Output = LearningCompletionPolicyOutput;
    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("learning_space", input.space_id)
    }
    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let policy = LearningOps::completion_policy(
            &self.pool,
            principal.tenant_id(),
            input.space_id,
            scope,
        )
        .await
        .map_err(|_| dependency_failure("The E-learning completion policy could not be loaded."))?
        .ok_or_else(|| invalid_state("No E-learning completion policy is available."))?;
        Ok(LearningCompletionPolicyOutput { policy })
    }
}

#[derive(Serialize)]
pub(super) struct LearningCompletionOutput {
    completion: LearningCompletionPage,
}

pub(super) struct LearningMineCompletionCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningMineCompletionCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.completion.mine.read",
                "Read my E-learning completion",
                "Returns derived completion for the authenticated learner against the published policy.",
                json!({ "space_id": { "type": "string", "format": "uuid" } }),
                json!({ "completion": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "learning.completion",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningMineCompletionCapability {
    type Input = LearningCompletionInput;
    type Output = LearningCompletionOutput;
    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("learning_space", input.space_id)
    }
    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let completion =
            LearningOps::self_completion(&self.pool, principal.tenant_id(), input.space_id, scope)
                .await
                .map_err(|_| {
                    dependency_failure("Learner E-learning completion could not be loaded.")
                })?;
        Ok(LearningCompletionOutput { completion })
    }
}

pub(super) struct LearningCompletionCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningCompletionCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.completion.list",
                "List E-learning completion",
                "Returns derived completion for the frozen class roster in assigned or campus scope.",
                json!({ "space_id": { "type": "string", "format": "uuid" } }),
                json!({ "completion": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "learning.completion",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningCompletionCapability {
    type Input = LearningCompletionInput;
    type Output = LearningCompletionOutput;
    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("learning_space", input.space_id)
    }
    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let completion =
            LearningOps::list_completion(&self.pool, principal.tenant_id(), input.space_id, scope)
                .await
                .map_err(|_| dependency_failure("E-learning completion could not be loaded."))?;
        Ok(LearningCompletionOutput { completion })
    }
}

fn resource_scope(kind: &'static str, id: Uuid) -> CapabilityScope {
    CapabilityScope::resources([CapabilityResource::parse(kind, id.to_string())
        .unwrap_or_else(|error| panic!("invalid built-in capability resource: {error}"))])
    .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
}

async fn current_scope(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<LearningAccessScope, CapabilityExecutionError> {
    let user = UserOps::get_user_by_id(pool, tenant_id, user_id)
        .await
        .map_err(|_| dependency_failure("Current E-learning authority could not be loaded."))?
        .filter(|user| user.is_active)
        .ok_or_else(|| invalid_state("The current E-learning account is unavailable."))?;
    let access = AccessOps::effective_access(pool, tenant_id, &user.roles)
        .await
        .map_err(|_| dependency_failure("Current E-learning access could not be loaded."))?;
    if access
        .permissions
        .iter()
        .any(|permission| permission == "*")
    {
        return Ok(LearningAccessScope::Campus);
    }
    let family = RecordScopeFamilyKey::parse("learning.spaces")
        .map_err(|_| invalid_state("The E-learning record scope is invalid."))?;
    match access.record_scopes.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Ok(LearningAccessScope::Campus),
        Some(EffectiveRecordScope::Assigned) => Ok(LearningAccessScope::AssignedTo(user_id)),
        Some(EffectiveRecordScope::SelfRecord) => Ok(LearningAccessScope::SelfFor(user_id)),
        Some(EffectiveRecordScope::SelfAndAssigned) => {
            Ok(LearningAccessScope::SelfAndAssigned(user_id))
        }
        None => Err(invalid_state("E-learning record scope is unavailable.")),
    }
}

fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}

fn invalid_state(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::InvalidState, message)
}

#[cfg(test)]
mod tests {
    use cp_agent::Capability as _;
    use sqlx::postgres::PgPoolOptions;

    use super::{
        LearningAssignmentCapability, LearningAssignmentInput, LearningAssignmentsCapability,
        LearningAssignmentsInput, LearningSpaceCapability, LearningSpaceInput,
        LearningSubmissionCapability, LearningSubmissionInput,
    };

    #[tokio::test]
    async fn exact_space_read_declares_the_requested_resource() {
        let capability = LearningSpaceCapability::new(
            PgPoolOptions::new()
                .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
                .unwrap_or_else(|_| unreachable!()),
        );
        let id = uuid::Uuid::new_v4();
        let scope = capability.scope(&LearningSpaceInput { space_id: id });
        let resource = scope.primary_resource().unwrap_or_else(|| unreachable!());
        assert_eq!(resource.kind(), "learning_space");
        assert_eq!(resource.id(), id.to_string());
    }

    #[tokio::test]
    async fn assignment_capabilities_declare_the_exact_parent_or_record() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
            .unwrap_or_else(|_| unreachable!());
        let space_id = uuid::Uuid::new_v4();
        let assignment_id = uuid::Uuid::new_v4();

        let list_scope =
            LearningAssignmentsCapability::new(pool.clone()).scope(&LearningAssignmentsInput {
                space_id,
                page: None,
                per_page: None,
                status: None,
            });
        let list_resource = list_scope
            .primary_resource()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(list_resource.kind(), "learning_space");
        assert_eq!(list_resource.id(), space_id.to_string());

        let read_scope = LearningAssignmentCapability::new(pool)
            .scope(&LearningAssignmentInput { assignment_id });
        let read_resource = read_scope
            .primary_resource()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(read_resource.kind(), "learning_assignment");
        assert_eq!(read_resource.id(), assignment_id.to_string());
    }

    #[tokio::test]
    async fn submission_read_declares_the_exact_submission() {
        let capability = LearningSubmissionCapability::new(
            PgPoolOptions::new()
                .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
                .unwrap_or_else(|_| unreachable!()),
        );
        let submission_id = uuid::Uuid::new_v4();
        let scope = capability.scope(&LearningSubmissionInput { submission_id });
        let resource = scope.primary_resource().unwrap_or_else(|| unreachable!());
        assert_eq!(resource.kind(), "learning_submission");
        assert_eq!(resource.id(), submission_id.to_string());
    }
}
