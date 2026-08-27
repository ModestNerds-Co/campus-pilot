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

#[derive(Debug, Clone)]
pub struct CapabilityCall {
    key: CapabilityKey,
    version: CapabilityVersion,
    input: Value,
    request_context: RequestContext,
    agent_run_id: Option<Uuid>,
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

#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityResult {
    key: CapabilityKey,
    version: CapabilityVersion,
    content: Value,
    request_context: RequestContext,
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
    RecordScopeDenied,
    ExecutionFailed,
    AuditUnavailable,
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
            Self::RecordScopeDenied => "record_scope_denied",
            Self::ExecutionFailed => "execution_failed",
            Self::AuditUnavailable => "audit_unavailable",
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

#[cfg(test)]
mod tests {
    use cp_audit::RequestContext;
    use cp_common::{AccessContext, EntitlementSnapshot, LeaseLifecycle};
    use serde_json::json;
    use uuid::Uuid;

    use crate::descriptor::{CapabilityKey, CapabilityVersion};

    use super::{
        AuthenticatedAgentPrincipal, AuthorizedCapabilityContext, AuthorizedRecordScope,
        BrokerError, BrokerErrorCode, CapabilityCall, CapabilityExecutionError,
        CapabilityExecutionErrorCode, CapabilityResource, CapabilityResources, CapabilityResult,
        CapabilityScope, CurrentAuthority,
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
