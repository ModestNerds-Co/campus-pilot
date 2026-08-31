//! Carries proof-bearing broker inputs, scopes, results, and stable failures.
//!
//! Model-provided input never contains tenant or person authority; those values
//! enter through an authenticated principal created by the application layer.

use std::fmt;

use cp_audit::RequestContext;
use cp_common::AccessContext;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::descriptor::{CapabilityKey, CapabilityVersion};

/// Stable identity assigned by the trusted Agent runtime before broker work.
///
/// This type deliberately has no string parser or serde implementation. HTTP
/// and model input must never be able to choose the identity used to correlate
/// runtime, usage, and actor-audit records.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CapabilityCallId(Uuid);

impl CapabilityCallId {
    /// Wraps an identity created by the trusted Agent runtime or worker.
    #[must_use]
    pub const fn from_trusted_runtime(value: Uuid) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Debug for CapabilityCallId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CapabilityCallId([redacted])")
    }
}

const MAX_WORKER_ID_LENGTH: usize = 120;

/// Bounded, non-serializable worker lease evidence carried to the verifier.
#[derive(Clone, PartialEq, Eq)]
pub struct CapabilityWorkerLease {
    worker_id: String,
    lease_token: Uuid,
    fence_version: i64,
}

impl CapabilityWorkerLease {
    pub fn parse(
        worker_id: &str,
        lease_token: Uuid,
        fence_version: i64,
    ) -> Result<Self, CapabilityExecutionProofError> {
        let worker_id = worker_id.trim();
        if worker_id.is_empty()
            || worker_id.len() > MAX_WORKER_ID_LENGTH
            || worker_id.chars().any(char::is_control)
            || lease_token.is_nil()
            || fence_version <= 0
        {
            return Err(CapabilityExecutionProofError);
        }
        Ok(Self {
            worker_id: worker_id.to_string(),
            lease_token,
            fence_version,
        })
    }
}

impl fmt::Debug for CapabilityWorkerLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CapabilityWorkerLease")
            .field("worker_id_length", &self.worker_id.len())
            .field("has_positive_fence", &(self.fence_version > 0))
            .finish_non_exhaustive()
    }
}

/// Untrusted execution evidence supplied by the Agent runtime.
///
/// Construction only bounds the shape. The broker-owned durability verifier
/// must prove every field against persisted runtime state immediately before a
/// handler may execute. This type deliberately implements neither serde trait.
#[derive(Clone, PartialEq, Eq)]
pub struct CapabilityExecutionProof {
    tenant_id: Uuid,
    user_id: Uuid,
    capability_call_id: CapabilityCallId,
    run_id: Uuid,
    worker_lease: CapabilityWorkerLease,
    usage_reservation_id: Uuid,
}

impl CapabilityExecutionProof {
    pub fn parse(
        principal: AuthenticatedAgentPrincipal,
        capability_call_id: CapabilityCallId,
        run_id: Uuid,
        worker_lease: CapabilityWorkerLease,
        usage_reservation_id: Uuid,
    ) -> Result<Self, CapabilityExecutionProofError> {
        if principal.tenant_id().is_nil()
            || principal.user_id().is_nil()
            || capability_call_id.as_uuid().is_nil()
            || run_id.is_nil()
            || usage_reservation_id.is_nil()
        {
            return Err(CapabilityExecutionProofError);
        }
        Ok(Self {
            tenant_id: principal.tenant_id(),
            user_id: principal.user_id(),
            capability_call_id,
            run_id,
            worker_lease,
            usage_reservation_id,
        })
    }

    #[must_use]
    pub const fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    #[must_use]
    pub const fn user_id(&self) -> Uuid {
        self.user_id
    }

    #[must_use]
    pub const fn capability_call_id(&self) -> CapabilityCallId {
        self.capability_call_id
    }

    #[must_use]
    pub const fn run_id(&self) -> Uuid {
        self.run_id
    }

    #[must_use]
    pub fn worker_id(&self) -> &str {
        &self.worker_lease.worker_id
    }

    #[must_use]
    pub const fn lease_token(&self) -> Uuid {
        self.worker_lease.lease_token
    }

    #[must_use]
    pub const fn fence_version(&self) -> i64 {
        self.worker_lease.fence_version
    }

    #[must_use]
    pub const fn usage_reservation_id(&self) -> Uuid {
        self.usage_reservation_id
    }
}

impl fmt::Debug for CapabilityExecutionProof {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CapabilityExecutionProof")
            .field("worker_lease", &self.worker_lease)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("capability execution proof is invalid")]
pub struct CapabilityExecutionProofError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthenticatedAgentPrincipal {
    tenant_id: Uuid,
    user_id: Uuid,
}

impl AuthenticatedAgentPrincipal {
    /// Creates a principal only after application authentication has resolved
    /// the active person and tenant. Never call this with model input.
    #[must_use]
    pub const fn from_authenticated_request(tenant_id: Uuid, user_id: Uuid) -> Self {
        Self { tenant_id, user_id }
    }

    #[must_use]
    pub const fn tenant_id(self) -> Uuid {
        self.tenant_id
    }

    #[must_use]
    pub const fn user_id(self) -> Uuid {
        self.user_id
    }
}

#[derive(Clone)]
pub struct CapabilityCall {
    key: CapabilityKey,
    version: CapabilityVersion,
    input: Value,
    request_context: RequestContext,
    agent_run_id: Option<Uuid>,
}

impl fmt::Debug for CapabilityCall {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CapabilityCall")
            .field("key", &self.key)
            .field("version", &self.version)
            .field("has_agent_run_id", &self.agent_run_id.is_some())
            .finish_non_exhaustive()
    }
}

impl CapabilityCall {
    pub fn parse(
        key: &str,
        version: u16,
        input: Value,
        request_context: RequestContext,
    ) -> Result<Self, crate::descriptor::DescriptorError> {
        Ok(Self {
            key: CapabilityKey::try_from(key)?,
            version: CapabilityVersion::try_from(version)?,
            input,
            request_context,
            agent_run_id: None,
        })
    }

    #[must_use]
    pub fn with_agent_run_id(mut self, agent_run_id: Uuid) -> Self {
        self.agent_run_id = Some(agent_run_id);
        self
    }

    #[must_use]
    pub const fn key(&self) -> &CapabilityKey {
        &self.key
    }

    #[must_use]
    pub const fn version(&self) -> CapabilityVersion {
        self.version
    }

    #[must_use]
    pub const fn input(&self) -> &Value {
        &self.input
    }

    #[must_use]
    pub const fn request_context(&self) -> RequestContext {
        self.request_context
    }

    #[must_use]
    pub const fn agent_run_id(&self) -> Option<Uuid> {
        self.agent_run_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityResource {
    kind: String,
    id: String,
}

impl CapabilityResource {
    pub fn parse(
        kind: impl Into<String>,
        id: impl Into<String>,
    ) -> Result<Self, CapabilityResourceError> {
        let kind = kind.into();
        let id = id.into();
        if kind.trim().is_empty() || id.trim().is_empty() {
            return Err(CapabilityResourceError);
        }
        Ok(Self { kind, id })
    }

    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("capability resource kind and identifier must not be empty")]
pub struct CapabilityResourceError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityResources {
    values: Vec<CapabilityResource>,
}

impl CapabilityResources {
    pub fn parse(
        resources: impl IntoIterator<Item = CapabilityResource>,
    ) -> Result<Self, CapabilityResourceError> {
        let values = resources.into_iter().collect::<Vec<_>>();
        if values.is_empty() {
            return Err(CapabilityResourceError);
        }
        Ok(Self { values })
    }

    #[must_use]
    pub fn values(&self) -> &[CapabilityResource] {
        &self.values
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityScope {
    TenantWide,
    Resources(CapabilityResources),
}

/// Reduced, persistence-safe facts produced by broker preflight.
///
/// Provider input and parsed handler input are intentionally absent. The
/// runtime can persist these facts before execution without gaining access to
/// the broker's opaque typed input or authorization proof.
#[derive(Clone, PartialEq, Eq)]
pub struct PreparedCapabilityCallFacts {
    capability_call_id: CapabilityCallId,
    key: CapabilityKey,
    version: CapabilityVersion,
    operation_key: String,
    module_key: String,
    required_permission: String,
    input_binding_sha256: [u8; 32],
    request_context: RequestContext,
    agent_run_id: Option<Uuid>,
    scope: CapabilityScope,
}

pub(crate) struct PreparedCapabilityCallFactsParts {
    pub capability_call_id: CapabilityCallId,
    pub key: CapabilityKey,
    pub version: CapabilityVersion,
    pub operation_key: String,
    pub module_key: String,
    pub required_permission: String,
    pub input_binding_sha256: [u8; 32],
    pub request_context: RequestContext,
    pub agent_run_id: Option<Uuid>,
    pub scope: CapabilityScope,
}

impl fmt::Debug for PreparedCapabilityCallFacts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (scope_kind, resource_count) = match &self.scope {
            CapabilityScope::TenantWide => ("tenant_wide", 0),
            CapabilityScope::Resources(resources) => ("resources", resources.values().len()),
        };
        formatter
            .debug_struct("PreparedCapabilityCallFacts")
            .field("key", &self.key)
            .field("version", &self.version)
            .field("has_agent_run_id", &self.agent_run_id.is_some())
            .field("scope_kind", &scope_kind)
            .field("resource_count", &resource_count)
            .finish_non_exhaustive()
    }
}

impl PreparedCapabilityCallFacts {
    pub(crate) fn new(parts: PreparedCapabilityCallFactsParts) -> Self {
        Self {
            capability_call_id: parts.capability_call_id,
            key: parts.key,
            version: parts.version,
            operation_key: parts.operation_key,
            module_key: parts.module_key,
            required_permission: parts.required_permission,
            input_binding_sha256: parts.input_binding_sha256,
            request_context: parts.request_context,
            agent_run_id: parts.agent_run_id,
            scope: parts.scope,
        }
    }

    #[must_use]
    pub const fn capability_call_id(&self) -> CapabilityCallId {
        self.capability_call_id
    }

    #[must_use]
    pub const fn key(&self) -> &CapabilityKey {
        &self.key
    }

    #[must_use]
    pub const fn version(&self) -> CapabilityVersion {
        self.version
    }

    #[must_use]
    pub fn operation_key(&self) -> &str {
        &self.operation_key
    }

    #[must_use]
    pub fn module_key(&self) -> &str {
        &self.module_key
    }

    #[must_use]
    pub fn required_permission(&self) -> &str {
        &self.required_permission
    }

    /// Broker-derived binding the runtime must persist and compare exactly.
    #[must_use]
    pub const fn input_binding_sha256(&self) -> [u8; 32] {
        self.input_binding_sha256
    }

    #[must_use]
    pub const fn request_context(&self) -> RequestContext {
        self.request_context
    }

    #[must_use]
    pub const fn agent_run_id(&self) -> Option<Uuid> {
        self.agent_run_id
    }

    #[must_use]
    pub const fn scope(&self) -> &CapabilityScope {
        &self.scope
    }
}

impl CapabilityScope {
    pub fn resources(
        resources: impl IntoIterator<Item = CapabilityResource>,
    ) -> Result<Self, CapabilityResourceError> {
        CapabilityResources::parse(resources).map(Self::Resources)
    }

    #[must_use]
    pub fn primary_resource(&self) -> Option<&CapabilityResource> {
        match self {
            Self::TenantWide => None,
            Self::Resources(resources) => resources.values().first(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CurrentAuthority {
    access: AccessContext,
}

impl CurrentAuthority {
    /// Wraps access reloaded for this exact broker call by a trusted loader.
    #[must_use]
    pub const fn from_reloaded_access(access: AccessContext) -> Self {
        Self { access }
    }

    #[must_use]
    pub const fn access(&self) -> &AccessContext {
        &self.access
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthorizedRecordScope(());

impl AuthorizedRecordScope {
    /// Created only by a record-scope authorizer after checking the parsed input.
    #[must_use]
    pub const fn granted() -> Self {
        Self(())
    }
}

#[derive(Debug, Clone)]
pub struct AuthorizedCapabilityContext {
    principal: AuthenticatedAgentPrincipal,
    request_context: RequestContext,
    scope: CapabilityScope,
    _scope_grant: AuthorizedRecordScope,
}

impl AuthorizedCapabilityContext {
    pub(crate) const fn new(
        principal: AuthenticatedAgentPrincipal,
        request_context: RequestContext,
        scope: CapabilityScope,
        scope_grant: AuthorizedRecordScope,
    ) -> Self {
        Self {
            principal,
            request_context,
            scope,
            _scope_grant: scope_grant,
        }
    }

    #[must_use]
    pub const fn principal(&self) -> AuthenticatedAgentPrincipal {
        self.principal
    }

    #[must_use]
    pub const fn request_context(&self) -> RequestContext {
        self.request_context
    }

    #[must_use]
    pub const fn scope(&self) -> &CapabilityScope {
        &self.scope
    }
}

#[derive(Clone, PartialEq)]
pub struct CapabilityResult {
    key: CapabilityKey,
    version: CapabilityVersion,
    content: Value,
    request_context: RequestContext,
}

impl fmt::Debug for CapabilityResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CapabilityResult")
            .field("key", &self.key)
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

impl CapabilityResult {
    pub(crate) const fn new(
        key: CapabilityKey,
        version: CapabilityVersion,
        content: Value,
        request_context: RequestContext,
    ) -> Self {
        Self {
            key,
            version,
            content,
            request_context,
        }
    }

    #[must_use]
    pub const fn key(&self) -> &CapabilityKey {
        &self.key
    }

    #[must_use]
    pub const fn version(&self) -> CapabilityVersion {
        self.version
    }

    #[must_use]
    pub const fn content(&self) -> &Value {
        &self.content
    }

    #[must_use]
    pub const fn request_context(&self) -> RequestContext {
        self.request_context
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityExecutionErrorCode {
    DependencyUnavailable,
    Conflict,
    InvalidState,
    Internal,
}

impl CapabilityExecutionErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DependencyUnavailable => "dependency_unavailable",
            Self::Conflict => "conflict",
            Self::InvalidState => "invalid_state",
            Self::Internal => "internal",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityExecutionError {
    code: CapabilityExecutionErrorCode,
    safe_message: String,
}

impl CapabilityExecutionError {
    #[must_use]
    pub fn new(code: CapabilityExecutionErrorCode, safe_message: impl Into<String>) -> Self {
        Self {
            code,
            safe_message: safe_message.into(),
        }
    }

    #[must_use]
    pub const fn code(&self) -> CapabilityExecutionErrorCode {
        self.code
    }

    #[must_use]
    pub fn safe_message(&self) -> &str {
        &self.safe_message
    }
}

impl fmt::Display for CapabilityExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.safe_message)
    }
}

impl std::error::Error for CapabilityExecutionError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerErrorCode {
    UnknownCapability,
    UnsupportedVersion,
    CapabilityUnavailable,
    ApprovalRequired,
    HumanOnly,
    Prohibited,
    AuthorityUnavailable,
    AccessDenied,
    InvalidInput,
    InputTooLarge,
    RecordScopeDenied,
    ExecutionFailed,
    AuditUnavailable,
    PreparedCallConsumed,
    DurabilityProofRejected,
}

impl BrokerErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnknownCapability => "unknown_capability",
            Self::UnsupportedVersion => "unsupported_version",
            Self::CapabilityUnavailable => "capability_unavailable",
            Self::ApprovalRequired => "approval_required",
            Self::HumanOnly => "human_only",
            Self::Prohibited => "prohibited",
            Self::AuthorityUnavailable => "authority_unavailable",
            Self::AccessDenied => "access_denied",
            Self::InvalidInput => "invalid_input",
            Self::InputTooLarge => "input_too_large",
            Self::RecordScopeDenied => "record_scope_denied",
            Self::ExecutionFailed => "execution_failed",
            Self::AuditUnavailable => "audit_unavailable",
            Self::PreparedCallConsumed => "prepared_call_consumed",
            Self::DurabilityProofRejected => "durability_proof_rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerError {
    code: BrokerErrorCode,
    safe_message: &'static str,
    request_context: RequestContext,
}

impl BrokerError {
    pub(crate) const fn new(
        code: BrokerErrorCode,
        safe_message: &'static str,
        request_context: RequestContext,
    ) -> Self {
        Self {
            code,
            safe_message,
            request_context,
        }
    }

    #[must_use]
    pub const fn code(&self) -> BrokerErrorCode {
        self.code
    }

    #[must_use]
    pub const fn safe_message(&self) -> &'static str {
        self.safe_message
    }

    #[must_use]
    pub const fn request_context(&self) -> RequestContext {
        self.request_context
    }
}

impl fmt::Display for BrokerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.safe_message)
    }
}

impl std::error::Error for BrokerError {}

/// Durable classification for a broker call that could not be prepared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityRejectionOutcome {
    Denied,
    Failed,
}

impl CapabilityRejectionOutcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Denied => "denied",
            Self::Failed => "failed",
        }
    }
}

/// Code-owned operation metadata resolved before a preparation rejection.
///
/// Absence means the requested capability did not resolve to a catalogued
/// operation; callers must not infer or fabricate these values.
#[derive(Clone, PartialEq, Eq)]
pub struct CapabilityRejectionOperationEvidence {
    operation_key: String,
    module_key: String,
    required_permission: String,
}

impl CapabilityRejectionOperationEvidence {
    pub(crate) fn new(
        operation_key: impl Into<String>,
        module_key: impl Into<String>,
        required_permission: impl Into<String>,
    ) -> Self {
        Self {
            operation_key: operation_key.into(),
            module_key: module_key.into(),
            required_permission: required_permission.into(),
        }
    }

    #[must_use]
    pub fn operation_key(&self) -> &str {
        &self.operation_key
    }

    #[must_use]
    pub fn module_key(&self) -> &str {
        &self.module_key
    }

    #[must_use]
    pub fn required_permission(&self) -> &str {
        &self.required_permission
    }
}

impl fmt::Debug for CapabilityRejectionOperationEvidence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CapabilityRejectionOperationEvidence([redacted])")
    }
}

/// Reduced evidence for a call intent rejected during broker preparation.
///
/// This is deliberately distinct from [`PreparedCapabilityCallFacts`]: it
/// proves that no executable prepared call exists. It implements neither serde
/// trait, and its `Debug` output omits principals, request/call/run IDs, input
/// digests, and resource identifiers.
#[derive(Clone, PartialEq, Eq)]
pub struct CapabilityPreparationRejection {
    principal: AuthenticatedAgentPrincipal,
    capability_call_id: CapabilityCallId,
    key: CapabilityKey,
    version: CapabilityVersion,
    request_context: RequestContext,
    agent_run_id: Option<Uuid>,
    normalized_input_digest_sha256: Option<[u8; 32]>,
    operation_evidence: Option<CapabilityRejectionOperationEvidence>,
    scope_evidence: Option<CapabilityScope>,
    outcome: CapabilityRejectionOutcome,
    code: BrokerErrorCode,
    reason_code: &'static str,
    safe_message: &'static str,
}

pub(crate) struct CapabilityPreparationRejectionParts {
    pub principal: AuthenticatedAgentPrincipal,
    pub capability_call_id: CapabilityCallId,
    pub key: CapabilityKey,
    pub version: CapabilityVersion,
    pub request_context: RequestContext,
    pub agent_run_id: Option<Uuid>,
    pub normalized_input_digest_sha256: Option<[u8; 32]>,
    pub operation_evidence: Option<CapabilityRejectionOperationEvidence>,
    pub scope_evidence: Option<CapabilityScope>,
    pub outcome: CapabilityRejectionOutcome,
    pub code: BrokerErrorCode,
    pub reason_code: &'static str,
    pub safe_message: &'static str,
}

impl CapabilityPreparationRejection {
    pub(crate) fn new(parts: CapabilityPreparationRejectionParts) -> Self {
        Self {
            principal: parts.principal,
            capability_call_id: parts.capability_call_id,
            key: parts.key,
            version: parts.version,
            request_context: parts.request_context,
            agent_run_id: parts.agent_run_id,
            normalized_input_digest_sha256: parts.normalized_input_digest_sha256,
            operation_evidence: parts.operation_evidence,
            scope_evidence: parts.scope_evidence,
            outcome: parts.outcome,
            code: parts.code,
            reason_code: parts.reason_code,
            safe_message: parts.safe_message,
        }
    }

    #[must_use]
    pub const fn principal(&self) -> AuthenticatedAgentPrincipal {
        self.principal
    }

    #[must_use]
    pub const fn capability_call_id(&self) -> CapabilityCallId {
        self.capability_call_id
    }

    #[must_use]
    pub const fn key(&self) -> &CapabilityKey {
        &self.key
    }

    #[must_use]
    pub const fn version(&self) -> CapabilityVersion {
        self.version
    }

    #[must_use]
    pub const fn request_context(&self) -> RequestContext {
        self.request_context
    }

    #[must_use]
    pub const fn agent_run_id(&self) -> Option<Uuid> {
        self.agent_run_id
    }

    /// SHA-256 of bounded canonical JSON, or `None` when input exceeded the
    /// canonical input ceiling and therefore could not be retained safely.
    #[must_use]
    pub const fn normalized_input_digest_sha256(&self) -> Option<[u8; 32]> {
        self.normalized_input_digest_sha256
    }

    /// Exact code-owned operation metadata when registry resolution completed.
    #[must_use]
    pub const fn operation_evidence(&self) -> Option<&CapabilityRejectionOperationEvidence> {
        self.operation_evidence.as_ref()
    }

    /// Parsed resource scope when input parsing and scope resolution completed
    /// before the rejection. Resource identifiers remain sensitive evidence.
    #[must_use]
    pub const fn scope_evidence(&self) -> Option<&CapabilityScope> {
        self.scope_evidence.as_ref()
    }

    #[must_use]
    pub const fn outcome(&self) -> CapabilityRejectionOutcome {
        self.outcome
    }

    #[must_use]
    pub const fn code(&self) -> BrokerErrorCode {
        self.code
    }

    #[must_use]
    pub const fn reason_code(&self) -> &'static str {
        self.reason_code
    }

    #[must_use]
    pub const fn safe_message(&self) -> &'static str {
        self.safe_message
    }

    #[must_use]
    pub(crate) fn into_broker_error(self) -> BrokerError {
        BrokerError::new(self.code, self.safe_message, self.request_context)
    }
}

impl fmt::Debug for CapabilityPreparationRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (scope_kind, resource_count) = match &self.scope_evidence {
            None => ("unavailable", 0),
            Some(CapabilityScope::TenantWide) => ("tenant_wide", 0),
            Some(CapabilityScope::Resources(resources)) => ("resources", resources.values().len()),
        };
        formatter
            .debug_struct("CapabilityPreparationRejection")
            .field("key", &self.key)
            .field("version", &self.version)
            .field("outcome", &self.outcome)
            .field("code", &self.code)
            .field("reason_code", &self.reason_code)
            .field(
                "has_normalized_input_digest",
                &self.normalized_input_digest_sha256.is_some(),
            )
            .field("has_operation_evidence", &self.operation_evidence.is_some())
            .field("has_agent_run_id", &self.agent_run_id.is_some())
            .field("scope_kind", &scope_kind)
            .field("resource_count", &resource_count)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for CapabilityPreparationRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.safe_message)
    }
}

impl std::error::Error for CapabilityPreparationRejection {}

impl From<CapabilityPreparationRejection> for BrokerError {
    fn from(rejection: CapabilityPreparationRejection) -> Self {
        rejection.into_broker_error()
    }
}

#[cfg(test)]
mod tests {
    use cp_audit::RequestContext;
    use cp_common::{AccessContext, EntitlementSnapshot, LeaseLifecycle};
    use serde_json::json;
    use uuid::Uuid;

    use crate::descriptor::{CapabilityKey, CapabilityVersion};

    use super::{
        AuthenticatedAgentPrincipal, AuthorizedCapabilityContext, AuthorizedRecordScope,
        BrokerError, BrokerErrorCode, CapabilityCall, CapabilityCallId, CapabilityExecutionError,
        CapabilityExecutionErrorCode, CapabilityExecutionProof, CapabilityResource,
        CapabilityResources, CapabilityResult, CapabilityScope, CapabilityWorkerLease,
        CurrentAuthority, PreparedCapabilityCallFacts, PreparedCapabilityCallFactsParts,
    };

    fn request_context() -> RequestContext {
        RequestContext::from_ids(Uuid::new_v4(), Uuid::new_v4())
    }

    fn access() -> AccessContext {
        AccessContext {
            role_keys: vec!["campus_owner".to_string()],
            permissions: vec!["agent:run".to_string()],
            enabled_modules: vec!["agent".to_string()],
            entitlements: EntitlementSnapshot::new(
                LeaseLifecycle::Legacy,
                Vec::<(String, cp_common::ModuleEntitlementState)>::new(),
                Vec::<String>::new(),
            )
            .unwrap_or_else(|_| unreachable!()),
        }
    }

    #[test]
    fn authenticated_identity_and_call_metadata_are_server_owned() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let run_id = Uuid::new_v4();
        let request_context = request_context();
        let principal = AuthenticatedAgentPrincipal::from_authenticated_request(tenant_id, user_id);
        let call = CapabilityCall::parse(
            "administration.catalog.read",
            1,
            json!({"query": "roles"}),
            request_context,
        )
        .unwrap_or_else(|_| unreachable!())
        .with_agent_run_id(run_id);

        assert_eq!(principal.tenant_id(), tenant_id);
        assert_eq!(principal.user_id(), user_id);
        assert_eq!(call.key().as_str(), "administration.catalog.read");
        assert_eq!(call.version().get(), 1);
        assert_eq!(call.input(), &json!({"query": "roles"}));
        assert_eq!(call.request_context(), request_context);
        assert_eq!(call.agent_run_id(), Some(run_id));
        assert!(CapabilityCall::parse("bad", 1, json!({}), request_context).is_err());
        assert!(
            CapabilityCall::parse("administration.catalog.read", 0, json!({}), request_context)
                .is_err()
        );
    }

    #[test]
    fn agent_call_debug_output_is_allowlisted_and_redacted() {
        let request_id = Uuid::new_v4();
        let correlation_id = Uuid::new_v4();
        let run_id = Uuid::new_v4();
        let capability_call_uuid = Uuid::new_v4();
        let request_context = RequestContext::from_ids(request_id, correlation_id);
        let call = CapabilityCall::parse(
            "administration.catalog.read",
            1,
            json!({ "query": "raw-model-input-secret" }),
            request_context,
        )
        .unwrap_or_else(|_| unreachable!())
        .with_agent_run_id(run_id);
        let call_debug = format!("{call:?}");

        assert!(call_debug.contains("administration.catalog.read"));
        assert!(call_debug.contains("has_agent_run_id: true"));
        for sensitive in [
            "raw-model-input-secret".to_string(),
            request_id.to_string(),
            correlation_id.to_string(),
            run_id.to_string(),
        ] {
            assert!(!call_debug.contains(&sensitive));
        }

        let capability_call_id = CapabilityCallId::from_trusted_runtime(capability_call_uuid);
        let resource = CapabilityResource::parse("student", "resource-id-secret")
            .unwrap_or_else(|_| unreachable!());
        let scope = CapabilityScope::resources([resource]).unwrap_or_else(|_| unreachable!());
        let facts = PreparedCapabilityCallFacts::new(PreparedCapabilityCallFactsParts {
            capability_call_id,
            key: CapabilityKey::try_from("administration.catalog.read")
                .unwrap_or_else(|_| unreachable!()),
            version: CapabilityVersion::try_from(1).unwrap_or_else(|_| unreachable!()),
            operation_key: "administration.catalog.read".to_string(),
            module_key: "administration".to_string(),
            required_permission: "administration:view".to_string(),
            input_binding_sha256: [0x7e; 32],
            request_context,
            agent_run_id: Some(run_id),
            scope,
        });
        let facts_debug = format!("{facts:?}");
        let call_id_debug = format!("{capability_call_id:?}");

        assert!(facts_debug.contains("administration.catalog.read"));
        assert!(facts_debug.contains("scope_kind: \"resources\""));
        assert!(facts_debug.contains("resource_count: 1"));
        assert_eq!(facts.operation_key(), "administration.catalog.read");
        assert_eq!(facts.module_key(), "administration");
        assert_eq!(facts.required_permission(), "administration:view");
        assert_eq!(facts.input_binding_sha256(), [0x7e; 32]);
        assert_eq!(call_id_debug, "CapabilityCallId([redacted])");
        for sensitive in [
            "resource-id-secret".to_string(),
            "126, 126, 126".to_string(),
            capability_call_uuid.to_string(),
            request_id.to_string(),
            correlation_id.to_string(),
            run_id.to_string(),
        ] {
            assert!(!facts_debug.contains(&sensitive));
            assert!(!call_id_debug.contains(&sensitive));
        }

        let lease_token = Uuid::new_v4();
        let reservation_id = Uuid::new_v4();
        let proof = CapabilityExecutionProof::parse(
            AuthenticatedAgentPrincipal::from_authenticated_request(Uuid::new_v4(), Uuid::new_v4()),
            capability_call_id,
            run_id,
            CapabilityWorkerLease::parse("worker-identity-secret", lease_token, 91)
                .unwrap_or_else(|_| unreachable!()),
            reservation_id,
        )
        .unwrap_or_else(|_| unreachable!());
        let proof_debug = format!("{proof:?}");
        assert!(proof_debug.contains("worker_id_length"));
        for sensitive in [
            "worker-identity-secret".to_string(),
            capability_call_uuid.to_string(),
            run_id.to_string(),
            lease_token.to_string(),
            reservation_id.to_string(),
            "91".to_string(),
        ] {
            assert!(!proof_debug.contains(&sensitive));
        }

        let result = CapabilityResult::new(
            CapabilityKey::try_from("administration.catalog.read")
                .unwrap_or_else(|_| unreachable!()),
            CapabilityVersion::try_from(1).unwrap_or_else(|_| unreachable!()),
            json!({ "result": "raw-capability-result-secret" }),
            request_context,
        );
        let result_debug = format!("{result:?}");
        assert!(result_debug.contains("administration.catalog.read"));
        for sensitive in [
            "raw-capability-result-secret".to_string(),
            request_id.to_string(),
            correlation_id.to_string(),
        ] {
            assert!(!result_debug.contains(&sensitive));
        }
    }

    #[test]
    fn execution_proof_parser_rejects_unbounded_or_missing_evidence() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let call_id = CapabilityCallId::from_trusted_runtime(Uuid::new_v4());
        let run_id = Uuid::new_v4();
        let lease_token = Uuid::new_v4();
        let reservation_id = Uuid::new_v4();
        let principal = AuthenticatedAgentPrincipal::from_authenticated_request(tenant_id, user_id);
        let valid = |worker_id: &str, fence_version: i64| {
            CapabilityExecutionProof::parse(
                principal,
                call_id,
                run_id,
                CapabilityWorkerLease::parse(worker_id, lease_token, fence_version)?,
                reservation_id,
            )
        };

        assert!(valid(" worker-1 ", 1).is_ok());
        assert!(valid("", 1).is_err());
        assert!(valid("worker\n1", 1).is_err());
        assert!(valid(&"w".repeat(121), 1).is_err());
        assert!(valid("worker-1", 0).is_err());
        assert!(
            CapabilityExecutionProof::parse(
                AuthenticatedAgentPrincipal::from_authenticated_request(Uuid::nil(), user_id),
                call_id,
                run_id,
                CapabilityWorkerLease::parse("worker-1", lease_token, 1)
                    .unwrap_or_else(|_| unreachable!()),
                reservation_id,
            )
            .is_err()
        );
    }

    #[test]
    fn resource_scopes_require_a_concrete_nonempty_target() {
        assert!(CapabilityResource::parse("", "1").is_err());
        assert!(CapabilityResource::parse("student", " ").is_err());
        assert!(CapabilityResources::parse(Vec::new()).is_err());
        assert!(CapabilityScope::resources(Vec::new()).is_err());

        let resource =
            CapabilityResource::parse("student", "student-1").unwrap_or_else(|_| unreachable!());
        let resources =
            CapabilityResources::parse([resource.clone()]).unwrap_or_else(|_| unreachable!());
        let scope = CapabilityScope::Resources(resources.clone());

        assert_eq!(resource.kind(), "student");
        assert_eq!(resource.id(), "student-1");
        assert_eq!(resources.values(), std::slice::from_ref(&resource));
        assert_eq!(scope.primary_resource(), Some(&resource));
        assert_eq!(CapabilityScope::TenantWide.primary_resource(), None);
        assert_eq!(
            CapabilityScope::resources([resource.clone()]).unwrap_or_else(|_| unreachable!()),
            scope
        );
    }

    #[test]
    fn authorized_context_authority_and_result_preserve_broker_proof() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let principal = AuthenticatedAgentPrincipal::from_authenticated_request(tenant_id, user_id);
        let request_context = request_context();
        let scope = CapabilityScope::TenantWide;
        let authority = CurrentAuthority::from_reloaded_access(access());
        let context = AuthorizedCapabilityContext::new(
            principal,
            request_context,
            scope.clone(),
            AuthorizedRecordScope::granted(),
        );
        let key = CapabilityKey::try_from("administration.catalog.read")
            .unwrap_or_else(|_| unreachable!());
        let version = CapabilityVersion::try_from(1).unwrap_or_else(|_| unreachable!());
        let result =
            CapabilityResult::new(key.clone(), version, json!({"items": []}), request_context);

        assert!(authority.access().has_permission("agent:run"));
        assert_eq!(context.principal(), principal);
        assert_eq!(context.request_context(), request_context);
        assert_eq!(context.scope(), &scope);
        assert_eq!(result.key(), &key);
        assert_eq!(result.version(), version);
        assert_eq!(result.content(), &json!({"items": []}));
        assert_eq!(result.request_context(), request_context);
    }

    #[test]
    fn public_errors_expose_only_stable_codes_and_safe_messages() {
        let execution_codes = [
            (
                CapabilityExecutionErrorCode::DependencyUnavailable,
                "dependency_unavailable",
            ),
            (CapabilityExecutionErrorCode::Conflict, "conflict"),
            (CapabilityExecutionErrorCode::InvalidState, "invalid_state"),
            (CapabilityExecutionErrorCode::Internal, "internal"),
        ];
        for (code, expected) in execution_codes {
            assert_eq!(code.as_str(), expected);
        }
        let execution = CapabilityExecutionError::new(
            CapabilityExecutionErrorCode::Conflict,
            "The record changed.",
        );
        assert_eq!(execution.code(), CapabilityExecutionErrorCode::Conflict);
        assert_eq!(execution.safe_message(), "The record changed.");
        assert_eq!(execution.to_string(), "The record changed.");

        let broker_codes = [
            (BrokerErrorCode::UnknownCapability, "unknown_capability"),
            (BrokerErrorCode::UnsupportedVersion, "unsupported_version"),
            (
                BrokerErrorCode::CapabilityUnavailable,
                "capability_unavailable",
            ),
            (BrokerErrorCode::ApprovalRequired, "approval_required"),
            (BrokerErrorCode::HumanOnly, "human_only"),
            (BrokerErrorCode::Prohibited, "prohibited"),
            (
                BrokerErrorCode::AuthorityUnavailable,
                "authority_unavailable",
            ),
            (BrokerErrorCode::AccessDenied, "access_denied"),
            (BrokerErrorCode::InvalidInput, "invalid_input"),
            (BrokerErrorCode::RecordScopeDenied, "record_scope_denied"),
            (BrokerErrorCode::ExecutionFailed, "execution_failed"),
            (BrokerErrorCode::AuditUnavailable, "audit_unavailable"),
            (
                BrokerErrorCode::PreparedCallConsumed,
                "prepared_call_consumed",
            ),
            (
                BrokerErrorCode::DurabilityProofRejected,
                "durability_proof_rejected",
            ),
        ];
        for (code, expected) in broker_codes {
            assert_eq!(code.as_str(), expected);
        }

        let request_context = request_context();
        let broker = BrokerError::new(
            BrokerErrorCode::AccessDenied,
            "Access denied.",
            request_context,
        );
        assert_eq!(broker.code(), BrokerErrorCode::AccessDenied);
        assert_eq!(broker.safe_message(), "Access denied.");
        assert_eq!(broker.request_context(), request_context);
        assert_eq!(broker.to_string(), "Access denied.");
    }
}
