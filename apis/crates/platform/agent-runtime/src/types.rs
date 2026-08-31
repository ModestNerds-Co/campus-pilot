//! Defines refined AI routing commands and secret-free route projections.
//!
//! Scope shapes and target chains are parsed once. Stored connection readiness
//! and immutable model-currentness remain runtime facts checked by `AiRoutingOps`.

use std::{collections::HashSet, str::FromStr};

use chrono::{DateTime, Utc};
use cp_common::{ProviderApprovalClass, ProviderDataClass, ProviderExecutionEnvironmentClass};
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

const MAX_ROUTE_TARGETS: usize = 3;
const MAX_MODULE_KEY_LENGTH: usize = 160;
const MAX_CAPABILITY_KEY_LENGTH: usize = 200;
const MAX_PROVIDER_MODEL_ID_LENGTH: usize = 240;
const MAX_AUDIT_REASON_LENGTH: usize = 500;

/// Stable Agent task classes understood by the initial routing release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskClass {
    CampusConversation,
    CampusConversationSearch,
    ModuleReadReporting,
    DocumentExtraction,
    DraftingProposal,
    ApprovedOperationalAction,
}

impl TaskClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CampusConversation => "campus_conversation",
            Self::CampusConversationSearch => "campus_conversation_search",
            Self::ModuleReadReporting => "module_read_reporting",
            Self::DocumentExtraction => "document_extraction",
            Self::DraftingProposal => "drafting_proposal",
            Self::ApprovedOperationalAction => "approved_operational_action",
        }
    }
}

impl FromStr for TaskClass {
    type Err = AiRoutingError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "campus_conversation" => Ok(Self::CampusConversation),
            "campus_conversation_search" => Ok(Self::CampusConversationSearch),
            "module_read_reporting" => Ok(Self::ModuleReadReporting),
            "document_extraction" => Ok(Self::DocumentExtraction),
            "drafting_proposal" => Ok(Self::DraftingProposal),
            "approved_operational_action" => Ok(Self::ApprovedOperationalAction),
            _ => Err(AiRoutingError::invalid(
                "invalid_task_class",
                "Choose a supported Agent task class",
            )),
        }
    }
}

/// Operation effects used by module-level routing overrides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationClass {
    Read,
    Propose,
    Mutate,
    ExternalSideEffect,
}

impl OperationClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Propose => "propose",
            Self::Mutate => "mutate",
            Self::ExternalSideEffect => "external_side_effect",
        }
    }
}

impl FromStr for OperationClass {
    type Err = AiRoutingError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "read" => Ok(Self::Read),
            "propose" => Ok(Self::Propose),
            "mutate" => Ok(Self::Mutate),
            "external_side_effect" => Ok(Self::ExternalSideEffect),
            _ => Err(AiRoutingError::invalid(
                "invalid_operation_class",
                "Choose read, propose, mutate, or external side effect",
            )),
        }
    }
}

/// One exact route scope. Variant fields make malformed scope combinations impossible.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(tag = "scope_kind", rename_all = "snake_case")]
pub enum AiRouteScope {
    TenantDefault,
    TaskClass {
        task_class: TaskClass,
    },
    ModuleOperation {
        module_key: String,
        operation_class: OperationClass,
    },
    Capability {
        capability_key: String,
        capability_version: i32,
    },
}

impl AiRouteScope {
    /// Parses the flat transport scope fields into one valid scope variant.
    #[allow(clippy::too_many_arguments)]
    pub fn parse(
        scope_kind: &str,
        task_class: Option<&str>,
        module_key: Option<&str>,
        operation_class: Option<&str>,
        capability_key: Option<&str>,
        capability_version: Option<i32>,
    ) -> Result<Self, AiRoutingError> {
        match scope_kind.trim() {
            "tenant_default"
                if task_class.is_none()
                    && module_key.is_none()
                    && operation_class.is_none()
                    && capability_key.is_none()
                    && capability_version.is_none() =>
            {
                Ok(Self::TenantDefault)
            }
            "task_class"
                if module_key.is_none()
                    && operation_class.is_none()
                    && capability_key.is_none()
                    && capability_version.is_none() =>
            {
                let task_class = task_class.ok_or_else(invalid_scope_shape)?;
                Ok(Self::TaskClass {
                    task_class: TaskClass::from_str(task_class)?,
                })
            }
            "module_operation"
                if task_class.is_none()
                    && capability_key.is_none()
                    && capability_version.is_none() =>
            {
                let module_key = stable_key(
                    module_key.ok_or_else(invalid_scope_shape)?,
                    "invalid_module_key",
                    MAX_MODULE_KEY_LENGTH,
                )?;
                let operation_class =
                    OperationClass::from_str(operation_class.ok_or_else(invalid_scope_shape)?)?;
                Ok(Self::ModuleOperation {
                    module_key,
                    operation_class,
                })
            }
            "capability"
                if task_class.is_none() && module_key.is_none() && operation_class.is_none() =>
            {
                let capability_key = stable_key(
                    capability_key.ok_or_else(invalid_scope_shape)?,
                    "invalid_capability_key",
                    MAX_CAPABILITY_KEY_LENGTH,
                )?;
                let capability_version = positive_i32(
                    capability_version.ok_or_else(invalid_scope_shape)?,
                    "invalid_capability_version",
                )?;
                Ok(Self::Capability {
                    capability_key,
                    capability_version,
                })
            }
            _ => Err(invalid_scope_shape()),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::TenantDefault => "tenant_default",
            Self::TaskClass { .. } => "task_class",
            Self::ModuleOperation { .. } => "module_operation",
            Self::Capability { .. } => "capability",
        }
    }

    pub(crate) const fn task_class(&self) -> Option<TaskClass> {
        match self {
            Self::TaskClass { task_class } => Some(*task_class),
            Self::TenantDefault | Self::ModuleOperation { .. } | Self::Capability { .. } => None,
        }
    }

    pub(crate) fn module_key(&self) -> Option<&str> {
        match self {
            Self::ModuleOperation { module_key, .. } => Some(module_key),
            Self::TenantDefault | Self::TaskClass { .. } | Self::Capability { .. } => None,
        }
    }

    pub(crate) const fn operation_class(&self) -> Option<OperationClass> {
        match self {
            Self::ModuleOperation {
                operation_class, ..
            } => Some(*operation_class),
            Self::TenantDefault | Self::TaskClass { .. } | Self::Capability { .. } => None,
        }
    }

    pub(crate) fn capability_key(&self) -> Option<&str> {
        match self {
            Self::Capability { capability_key, .. } => Some(capability_key),
            Self::TenantDefault | Self::TaskClass { .. } | Self::ModuleOperation { .. } => None,
        }
    }

    pub(crate) const fn capability_version(&self) -> Option<i32> {
        match self {
            Self::Capability {
                capability_version, ..
            } => Some(*capability_version),
            Self::TenantDefault | Self::TaskClass { .. } | Self::ModuleOperation { .. } => None,
        }
    }
}

/// One connection/model pair in an ordered fallback chain.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RouteTargetDraft {
    pub(crate) connection_id: Uuid,
    pub(crate) provider_model_id: String,
}

impl RouteTargetDraft {
    pub fn parse(
        connection_id: Uuid,
        provider_model_id: impl Into<String>,
    ) -> Result<Self, AiRoutingError> {
        let provider_model_id = provider_model_id.into().trim().to_owned();
        if provider_model_id.is_empty()
            || provider_model_id.chars().count() > MAX_PROVIDER_MODEL_ID_LENGTH
        {
            return Err(AiRoutingError::invalid(
                "invalid_provider_model_id",
                "Provider model ID must contain between 1 and 240 characters",
            ));
        }
        Ok(Self {
            connection_id,
            provider_model_id,
        })
    }
}

#[derive(Debug, Clone)]
struct RouteChain(Vec<RouteTargetDraft>);

impl RouteChain {
    fn parse(targets: Vec<RouteTargetDraft>) -> Result<Self, AiRoutingError> {
        if targets.is_empty() || targets.len() > MAX_ROUTE_TARGETS {
            return Err(AiRoutingError::invalid(
                "invalid_route_chain",
                "Provide between one and three ordered route targets",
            ));
        }
        let unique = targets
            .iter()
            .map(|target| target.connection_id)
            .collect::<HashSet<_>>();
        if unique.len() != targets.len() {
            return Err(AiRoutingError::invalid(
                "duplicate_route_target",
                "Each connection may appear only once in a route",
            ));
        }
        Ok(Self(targets))
    }

    pub(crate) fn as_slice(&self) -> &[RouteTargetDraft] {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AuditReason(String);

impl AuditReason {
    fn parse(value: impl Into<String>) -> Result<Self, AiRoutingError> {
        let value = value.into().trim().to_owned();
        if value.chars().count() < 3 || value.chars().count() > MAX_AUDIT_REASON_LENGTH {
            return Err(AiRoutingError::invalid(
                "invalid_audit_reason",
                "Audit reason must contain between 3 and 500 characters",
            ));
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// Parsed create command for one unique active scope.
#[derive(Debug, Clone)]
pub struct CreateRouteCommand {
    pub(crate) scope: AiRouteScope,
    pub(crate) requires_tools: bool,
    chain: RouteChain,
    reason: AuditReason,
}

impl CreateRouteCommand {
    pub fn parse(
        scope: AiRouteScope,
        requires_tools: bool,
        targets: Vec<RouteTargetDraft>,
        audit_reason: impl Into<String>,
    ) -> Result<Self, AiRoutingError> {
        Ok(Self {
            scope,
            requires_tools,
            chain: RouteChain::parse(targets)?,
            reason: AuditReason::parse(audit_reason)?,
        })
    }

    pub(crate) fn targets(&self) -> &[RouteTargetDraft] {
        self.chain.as_slice()
    }

    pub(crate) fn reason(&self) -> &str {
        self.reason.as_str()
    }
}

/// Parsed optimistic full-chain replacement command.
#[derive(Debug, Clone)]
pub struct ReplaceRouteCommand {
    pub(crate) expected_version: i64,
    pub(crate) requires_tools: bool,
    chain: RouteChain,
    reason: AuditReason,
}

impl ReplaceRouteCommand {
    pub fn parse(
        expected_version: i64,
        requires_tools: bool,
        targets: Vec<RouteTargetDraft>,
        audit_reason: impl Into<String>,
    ) -> Result<Self, AiRoutingError> {
        Ok(Self {
            expected_version: positive_i64(expected_version)?,
            requires_tools,
            chain: RouteChain::parse(targets)?,
            reason: AuditReason::parse(audit_reason)?,
        })
    }

    pub(crate) fn targets(&self) -> &[RouteTargetDraft] {
        self.chain.as_slice()
    }

    pub(crate) fn reason(&self) -> &str {
        self.reason.as_str()
    }
}

/// Parsed optimistic archive command.
#[derive(Debug, Clone)]
pub struct ArchiveRouteCommand {
    pub(crate) expected_version: i64,
    reason: AuditReason,
}

impl ArchiveRouteCommand {
    pub fn parse(
        expected_version: i64,
        audit_reason: impl Into<String>,
    ) -> Result<Self, AiRoutingError> {
        Ok(Self {
            expected_version: positive_i64(expected_version)?,
            reason: AuditReason::parse(audit_reason)?,
        })
    }

    pub(crate) fn reason(&self) -> &str {
        self.reason.as_str()
    }
}

/// Parsed inputs used to select the highest-precedence exact route scope.
#[derive(Debug, Clone)]
pub struct ResolveRouteCommand {
    pub(crate) task_class: TaskClass,
    pub(crate) module_operation: Option<(String, OperationClass)>,
    pub(crate) capability: Option<(String, i32)>,
    pub(crate) requires_tools: bool,
    pub(crate) required_provider_data_class: ProviderDataClass,
}

impl ResolveRouteCommand {
    #[allow(clippy::too_many_arguments)]
    pub fn parse(
        task_class: &str,
        module_key: Option<&str>,
        operation_class: Option<&str>,
        capability_key: Option<&str>,
        capability_version: Option<i32>,
        requires_tools: bool,
    ) -> Result<Self, AiRoutingError> {
        let module_operation = match (module_key, operation_class) {
            (Some(module_key), Some(operation_class)) => Some((
                stable_key(module_key, "invalid_module_key", MAX_MODULE_KEY_LENGTH)?,
                OperationClass::from_str(operation_class)?,
            )),
            (None, None) => None,
            _ => return Err(invalid_resolve_shape()),
        };
        let capability = match (capability_key, capability_version) {
            (Some(capability_key), Some(capability_version)) => Some((
                stable_key(
                    capability_key,
                    "invalid_capability_key",
                    MAX_CAPABILITY_KEY_LENGTH,
                )?,
                positive_i32(capability_version, "invalid_capability_version")?,
            )),
            (None, None) => None,
            _ => return Err(invalid_resolve_shape()),
        };
        Ok(Self {
            task_class: TaskClass::from_str(task_class)?,
            module_operation,
            capability,
            requires_tools,
            // User-authored turns may contain personal campus information even
            // before a typed capability adds stricter context.
            required_provider_data_class: ProviderDataClass::SensitiveDataApproved,
        })
    }

    /// Raises, but never lowers, the provider data requirement for hydrated context.
    #[must_use]
    pub fn requiring_provider_data_class(mut self, required: ProviderDataClass) -> Self {
        self.required_provider_data_class = self.required_provider_data_class.max(required);
        self
    }

    pub(crate) fn candidate_scopes(&self) -> Vec<AiRouteScope> {
        let mut scopes = Vec::with_capacity(4);
        if let Some((capability_key, capability_version)) = &self.capability {
            scopes.push(AiRouteScope::Capability {
                capability_key: capability_key.clone(),
                capability_version: *capability_version,
            });
        }
        if let Some((module_key, operation_class)) = &self.module_operation {
            scopes.push(AiRouteScope::ModuleOperation {
                module_key: module_key.clone(),
                operation_class: *operation_class,
            });
        }
        scopes.push(AiRouteScope::TaskClass {
            task_class: self.task_class,
        });
        scopes.push(AiRouteScope::TenantDefault);
        scopes
    }
}

/// Current readiness of a saved target without exposing provider failure detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteTargetReadiness {
    Ready,
    ConnectionUnavailable,
    StaleModel,
    ModelLimitsUnavailable,
    ToolsUnsupported,
    ProviderDataNotApproved,
    ProviderDataApprovalChanged,
    LocalExecutionRequired,
}

/// One secret-free target in route priority order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiRouteTarget {
    pub id: Uuid,
    pub priority: i16,
    pub connection_id: Uuid,
    #[serde(skip_serializing)]
    pub provider_data_approval_id: Uuid,
    pub provider_data_approval_version: i64,
    pub provider_data_approval_class: ProviderApprovalClass,
    pub execution_environment_class: ProviderExecutionEnvironmentClass,
    #[serde(skip_serializing)]
    pub model_id: Uuid,
    pub provider: String,
    pub account_label: String,
    pub provider_model_id: String,
    pub model_display_name: String,
    pub context_window_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub supports_tools: Option<bool>,
    pub readiness: RouteTargetReadiness,
}

/// One ready resolved target pinned to the immutable model credential snapshot.
///
/// Construction stays crate-private so worker code cannot accidentally rebuild
/// the execution identity from a mutable provider connection. The pin and model
/// snapshot UUID are deliberately absent from administration JSON projections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedAiRouteTarget {
    #[serde(flatten)]
    projection: AiRouteTarget,
    #[serde(skip_serializing)]
    expected_credential_version: i64,
    #[serde(skip_serializing)]
    max_output_tokens: i64,
}

impl ResolvedAiRouteTarget {
    pub(crate) fn from_ready_projection(
        projection: AiRouteTarget,
        expected_credential_version: i64,
    ) -> Option<Self> {
        if projection.readiness != RouteTargetReadiness::Ready
            || expected_credential_version <= 0
            || projection
                .context_window_tokens
                .is_none_or(|value| value <= 0)
        {
            return None;
        }
        let max_output_tokens = projection.max_output_tokens.filter(|value| *value > 0)?;
        Some(Self {
            projection,
            expected_credential_version,
            max_output_tokens,
        })
    }

    /// Stable route-target identity persisted with an execution attempt.
    #[must_use]
    pub const fn route_target_id(&self) -> Uuid {
        self.projection.id
    }

    /// One-based fallback priority selected by routing resolution.
    #[must_use]
    pub const fn priority(&self) -> i16 {
        self.projection.priority
    }

    /// Provider connection selected by routing resolution.
    #[must_use]
    pub const fn connection_id(&self) -> Uuid {
        self.projection.connection_id
    }

    /// Stable provider key used to construct the execution command.
    #[must_use]
    pub fn provider_key(&self) -> &str {
        &self.projection.provider
    }

    /// Provider-owned model identifier pinned by the model snapshot.
    #[must_use]
    pub fn provider_model_id(&self) -> &str {
        &self.projection.provider_model_id
    }

    /// Maximum output-token count proven by the current model snapshot.
    #[must_use]
    pub const fn max_output_tokens(&self) -> i64 {
        self.max_output_tokens
    }

    /// Credential version proven current when this route was resolved.
    #[must_use]
    pub const fn expected_credential_version(&self) -> i64 {
        self.expected_credential_version
    }

    /// Immutable provider-model snapshot selected by routing resolution.
    #[must_use]
    pub const fn model_snapshot_id(&self) -> Uuid {
        self.projection.model_id
    }

    /// Immutable provider data-approval version pinned by this route target.
    #[must_use]
    pub const fn provider_data_approval_id(&self) -> Uuid {
        self.projection.provider_data_approval_id
    }

    /// Required provider data class used for this resolution.
    #[must_use]
    pub const fn provider_approval_class(&self) -> ProviderApprovalClass {
        self.projection.provider_data_approval_class
    }

    /// Trusted adapter boundary selected for the provider request.
    #[must_use]
    pub const fn execution_environment_class(&self) -> ProviderExecutionEnvironmentClass {
        self.projection.execution_environment_class
    }
}

/// One active route scope and its complete ordered fallback chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiRouteSet {
    pub id: Uuid,
    pub scope: AiRouteScope,
    pub requires_tools: bool,
    pub targets: Vec<AiRouteTarget>,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Precedence that selected an effective route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutePrecedence {
    Capability,
    ModuleOperation,
    TaskClass,
    TenantDefault,
}

impl RoutePrecedence {
    #[must_use]
    pub const fn for_scope(scope: &AiRouteScope) -> Self {
        match scope {
            AiRouteScope::Capability { .. } => Self::Capability,
            AiRouteScope::ModuleOperation { .. } => Self::ModuleOperation,
            AiRouteScope::TaskClass { .. } => Self::TaskClass,
            AiRouteScope::TenantDefault => Self::TenantDefault,
        }
    }
}

/// Resolved route pinned to one route-set version and ordered ready chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedAiRoute {
    pub route_set_id: Uuid,
    pub matched_scope: AiRouteScope,
    pub precedence: RoutePrecedence,
    pub route_version: i64,
    pub requires_tools: bool,
    pub required_provider_data_class: ProviderDataClass,
    pub targets: Vec<ResolvedAiRouteTarget>,
}

/// Confirmation that an optimistic archive succeeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ArchivedAiRoute {
    pub archived_id: Uuid,
    pub version: i64,
}

/// Safe reason a matched route cannot be used. Resolution never falls through it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteUnusableReason {
    EmptyChain,
    ConnectionUnavailable,
    StaleModel,
    ModelLimitsUnavailable,
    ToolsUnsupported,
    ProviderDataNotApproved,
    ProviderDataApprovalChanged,
    LocalExecutionRequired,
}

/// Stable routing errors mapped to HTTP responses by the application crate.
#[derive(Debug, Error)]
pub enum AiRoutingError {
    #[error("{message}")]
    InvalidInput { code: &'static str, message: String },
    #[error("AI route was not found")]
    NotFound,
    #[error("{message}")]
    Conflict { code: &'static str, message: String },
    #[error("No AI route matches this task")]
    NoMatchingRoute,
    #[error("The matched AI route is not currently usable")]
    UnusableRoute {
        route_set_id: Uuid,
        reason: RouteUnusableReason,
    },
    #[error("AI routing persistence failed")]
    Storage(#[source] sqlx::Error),
}

impl AiRoutingError {
    #[must_use]
    pub fn invalid(code: &'static str, message: impl Into<String>) -> Self {
        Self::InvalidInput {
            code,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::Conflict {
            code,
            message: message.into(),
        }
    }

    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { code, .. } | Self::Conflict { code, .. } => code,
            Self::NotFound => "route_not_found",
            Self::NoMatchingRoute => "route_not_configured",
            Self::UnusableRoute { .. } => "route_unusable",
            Self::Storage(_) => "routing_storage_error",
        }
    }

    #[must_use]
    pub fn safe_message(&self) -> String {
        match self {
            Self::InvalidInput { message, .. } | Self::Conflict { message, .. } => message.clone(),
            Self::NotFound => "This AI route does not exist".to_owned(),
            Self::NoMatchingRoute => "No AI route is configured for this task".to_owned(),
            Self::UnusableRoute { reason, .. } => match reason {
                RouteUnusableReason::EmptyChain => {
                    "The matched AI route has no active targets".to_owned()
                }
                RouteUnusableReason::ConnectionUnavailable => {
                    "A connection in the matched AI route is not ready".to_owned()
                }
                RouteUnusableReason::StaleModel => {
                    "A model in the matched AI route is no longer current".to_owned()
                }
                RouteUnusableReason::ModelLimitsUnavailable => {
                    "A model in the matched AI route has no usable token limits".to_owned()
                }
                RouteUnusableReason::ToolsUnsupported => {
                    "A model in the matched AI route does not support tools".to_owned()
                }
                RouteUnusableReason::ProviderDataNotApproved => {
                    "A connection in the matched AI route is not approved for this data".to_owned()
                }
                RouteUnusableReason::ProviderDataApprovalChanged => {
                    "A provider data approval changed; save the route again".to_owned()
                }
                RouteUnusableReason::LocalExecutionRequired => {
                    "This data requires an installation-local provider".to_owned()
                }
            },
            Self::Storage(_) => "AI routing could not be loaded or saved".to_owned(),
        }
    }
}

impl From<sqlx::Error> for AiRoutingError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error)
    }
}

fn stable_key(
    value: &str,
    code: &'static str,
    maximum_length: usize,
) -> Result<String, AiRoutingError> {
    let value = value.trim();
    let valid = !value.is_empty()
        && value.len() <= maximum_length
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        });
    if valid {
        Ok(value.to_owned())
    } else {
        Err(AiRoutingError::invalid(
            code,
            "Use a stable lowercase key containing letters, numbers, dots, hyphens, or underscores",
        ))
    }
}

fn positive_i32(value: i32, code: &'static str) -> Result<i32, AiRoutingError> {
    if value > 0 {
        Ok(value)
    } else {
        Err(AiRoutingError::invalid(code, "Version must be positive"))
    }
}

fn positive_i64(value: i64) -> Result<i64, AiRoutingError> {
    if value > 0 {
        Ok(value)
    } else {
        Err(AiRoutingError::invalid(
            "invalid_expected_version",
            "Expected version must be positive",
        ))
    }
}

fn invalid_scope_shape() -> AiRoutingError {
    AiRoutingError::invalid(
        "invalid_route_scope",
        "Route scope fields do not match the selected scope kind",
    )
}

fn invalid_resolve_shape() -> AiRoutingError {
    AiRoutingError::invalid(
        "invalid_resolve_scope",
        "Module and capability selectors must be supplied as complete pairs",
    )
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use cp_common::{ProviderApprovalClass, ProviderExecutionEnvironmentClass};
    use serde_json::json;
    use uuid::Uuid;

    use super::{
        AiRouteScope, AiRouteTarget, AiRoutingError, ArchiveRouteCommand, CreateRouteCommand,
        OperationClass, ReplaceRouteCommand, ResolveRouteCommand, ResolvedAiRouteTarget,
        RoutePrecedence, RouteTargetDraft, RouteTargetReadiness, RouteUnusableReason, TaskClass,
    };

    #[test]
    fn scope_parser_accepts_only_exact_shapes() {
        assert_eq!(
            AiRouteScope::parse("tenant_default", None, None, None, None, None).unwrap(),
            AiRouteScope::TenantDefault
        );
        assert_eq!(
            AiRouteScope::parse(
                "module_operation",
                None,
                Some("finance"),
                Some("read"),
                None,
                None,
            )
            .unwrap(),
            AiRouteScope::ModuleOperation {
                module_key: "finance".to_owned(),
                operation_class: OperationClass::Read,
            }
        );
        assert!(
            AiRouteScope::parse(
                "task_class",
                Some("module_read_reporting"),
                Some("finance"),
                None,
                None,
                None,
            )
            .is_err()
        );
        assert!(
            AiRouteScope::parse("capability", None, None, None, Some("Bad Key"), Some(1)).is_err()
        );
    }

    #[test]
    fn scope_serialization_has_a_stable_tag() {
        let scope = AiRouteScope::Capability {
            capability_key: "finance.journals.list".to_owned(),
            capability_version: 2,
        };
        assert_eq!(
            serde_json::to_value(scope).unwrap(),
            json!({
                "scope_kind": "capability",
                "capability_key": "finance.journals.list",
                "capability_version": 2
            })
        );
    }

    #[test]
    fn route_target_serialization_keeps_execution_identity_internal() {
        let provider_data_approval_id = Uuid::new_v4();
        let target = AiRouteTarget {
            id: Uuid::new_v4(),
            priority: 1,
            connection_id: Uuid::new_v4(),
            provider_data_approval_id,
            provider_data_approval_version: 2,
            provider_data_approval_class: ProviderApprovalClass::SensitiveDataApproved,
            execution_environment_class: ProviderExecutionEnvironmentClass::ExternalManaged,
            model_id: Uuid::new_v4(),
            provider: "openai".to_owned(),
            account_label: "Campus account".to_owned(),
            provider_model_id: "gpt-test".to_owned(),
            model_display_name: "GPT Test".to_owned(),
            context_window_tokens: Some(128_000),
            max_output_tokens: Some(16_384),
            supports_tools: Some(true),
            readiness: RouteTargetReadiness::Ready,
        };
        let value = serde_json::to_value(&target).unwrap();
        assert!(value.get("model_id").is_none());
        assert!(value.get("provider_data_approval_id").is_none());
        assert_eq!(value["provider_model_id"], json!("gpt-test"));

        let resolved = ResolvedAiRouteTarget::from_ready_projection(target.clone(), 7).unwrap();
        assert_eq!(resolved.route_target_id(), target.id);
        assert_eq!(resolved.priority(), target.priority);
        assert_eq!(resolved.connection_id(), target.connection_id);
        assert_eq!(resolved.provider_key(), target.provider);
        assert_eq!(resolved.provider_model_id(), target.provider_model_id);
        assert_eq!(resolved.max_output_tokens(), 16_384);
        assert_eq!(resolved.expected_credential_version(), 7);
        assert_eq!(resolved.model_snapshot_id(), target.model_id);
        let value = serde_json::to_value(resolved).unwrap();
        assert!(value.get("model_id").is_none());
        assert!(value.get("expected_credential_version").is_none());
        assert_eq!(value["max_output_tokens"], json!(16_384));

        assert!(ResolvedAiRouteTarget::from_ready_projection(target.clone(), 0).is_none());
        let mut missing_limits = target.clone();
        missing_limits.max_output_tokens = None;
        assert!(ResolvedAiRouteTarget::from_ready_projection(missing_limits, 7).is_none());
        let mut invalid_context_limit = target.clone();
        invalid_context_limit.context_window_tokens = Some(0);
        assert!(ResolvedAiRouteTarget::from_ready_projection(invalid_context_limit, 7).is_none());
        let mut invalid_output_limit = target.clone();
        invalid_output_limit.max_output_tokens = Some(-1);
        assert!(ResolvedAiRouteTarget::from_ready_projection(invalid_output_limit, 7).is_none());
        let mut unready = target;
        unready.readiness = RouteTargetReadiness::StaleModel;
        assert!(ResolvedAiRouteTarget::from_ready_projection(unready, 7).is_none());
    }

    #[test]
    fn commands_require_bounded_unique_chains_reasons_and_versions() {
        let target = RouteTargetDraft::parse(Uuid::new_v4(), "gpt-test").unwrap();
        assert!(
            CreateRouteCommand::parse(AiRouteScope::TenantDefault, false, vec![], "setup").is_err()
        );
        assert!(
            CreateRouteCommand::parse(
                AiRouteScope::TenantDefault,
                false,
                vec![target.clone(), target.clone()],
                "setup",
            )
            .is_err()
        );
        assert!(
            ReplaceRouteCommand::parse(0, false, vec![target.clone()], "change route").is_err()
        );
        assert!(ArchiveRouteCommand::parse(1, "x").is_err());
        assert!(
            CreateRouteCommand::parse(
                AiRouteScope::TenantDefault,
                true,
                vec![target],
                "initial route",
            )
            .is_ok()
        );
    }

    #[test]
    fn resolve_candidates_have_exact_fail_closed_precedence() {
        let command = ResolveRouteCommand::parse(
            "module_read_reporting",
            Some("finance"),
            Some("read"),
            Some("finance.journals.list"),
            Some(1),
            true,
        )
        .unwrap();
        let scopes = command.candidate_scopes();
        assert!(matches!(scopes[0], AiRouteScope::Capability { .. }));
        assert!(matches!(scopes[1], AiRouteScope::ModuleOperation { .. }));
        assert_eq!(
            scopes[2],
            AiRouteScope::TaskClass {
                task_class: TaskClass::ModuleReadReporting
            }
        );
        assert_eq!(scopes[3], AiRouteScope::TenantDefault);
        assert_eq!(
            RoutePrecedence::for_scope(&scopes[0]),
            RoutePrecedence::Capability
        );
    }

    #[test]
    fn resolve_requires_complete_optional_selector_pairs() {
        assert!(
            ResolveRouteCommand::parse(
                "campus_conversation_search",
                Some("finance"),
                None,
                None,
                None,
                false,
            )
            .is_err()
        );
        assert!(
            ResolveRouteCommand::parse(
                "campus_conversation_search",
                None,
                None,
                Some("finance.journals.list"),
                None,
                false,
            )
            .is_err()
        );
    }

    #[test]
    fn task_and_operation_enums_cover_every_stable_value() {
        for (value, task_class) in [
            ("campus_conversation", TaskClass::CampusConversation),
            (
                "campus_conversation_search",
                TaskClass::CampusConversationSearch,
            ),
            ("module_read_reporting", TaskClass::ModuleReadReporting),
            ("document_extraction", TaskClass::DocumentExtraction),
            ("drafting_proposal", TaskClass::DraftingProposal),
            (
                "approved_operational_action",
                TaskClass::ApprovedOperationalAction,
            ),
        ] {
            assert_eq!(TaskClass::from_str(value).unwrap(), task_class);
            assert_eq!(task_class.as_str(), value);
        }
        assert!(TaskClass::from_str("unknown").is_err());

        for (value, operation_class) in [
            ("read", OperationClass::Read),
            ("propose", OperationClass::Propose),
            ("mutate", OperationClass::Mutate),
            ("external_side_effect", OperationClass::ExternalSideEffect),
        ] {
            assert_eq!(OperationClass::from_str(value).unwrap(), operation_class);
            assert_eq!(operation_class.as_str(), value);
        }
        assert!(OperationClass::from_str("delete").is_err());
    }

    #[test]
    fn every_scope_variant_exposes_only_its_own_fields() {
        let task = AiRouteScope::parse(
            "task_class",
            Some("document_extraction"),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(task.kind(), "task_class");
        assert_eq!(task.task_class(), Some(TaskClass::DocumentExtraction));
        assert_eq!(task.module_key(), None);
        assert_eq!(task.operation_class(), None);
        assert_eq!(task.capability_key(), None);
        assert_eq!(task.capability_version(), None);

        let module = AiRouteScope::parse(
            "module_operation",
            None,
            Some("hr_payroll"),
            Some("external_side_effect"),
            None,
            None,
        )
        .unwrap();
        assert_eq!(module.kind(), "module_operation");
        assert_eq!(module.task_class(), None);
        assert_eq!(module.module_key(), Some("hr_payroll"));
        assert_eq!(
            module.operation_class(),
            Some(OperationClass::ExternalSideEffect)
        );

        let capability = AiRouteScope::parse(
            "capability",
            None,
            None,
            None,
            Some("sis.learners.list"),
            Some(3),
        )
        .unwrap();
        assert_eq!(capability.kind(), "capability");
        assert_eq!(capability.capability_key(), Some("sis.learners.list"));
        assert_eq!(capability.capability_version(), Some(3));

        let default = AiRouteScope::TenantDefault;
        assert_eq!(default.task_class(), None);
        assert_eq!(default.module_key(), None);
        assert_eq!(default.operation_class(), None);
        assert_eq!(default.capability_key(), None);
        assert_eq!(default.capability_version(), None);
    }

    #[test]
    fn target_and_chain_parsers_cover_boundary_rejections() {
        assert!(RouteTargetDraft::parse(Uuid::new_v4(), " ").is_err());
        assert!(RouteTargetDraft::parse(Uuid::new_v4(), "x".repeat(241)).is_err());
        let targets = (0..4)
            .map(|index| RouteTargetDraft::parse(Uuid::new_v4(), format!("model-{index}")))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(
            CreateRouteCommand::parse(
                AiRouteScope::TenantDefault,
                false,
                targets,
                "Too many targets",
            )
            .is_err()
        );
        assert!(ArchiveRouteCommand::parse(2, "Archive route").is_ok());
        assert!(
            ResolveRouteCommand::parse(
                "campus_conversation_search",
                None,
                None,
                None,
                None,
                false,
            )
            .is_ok()
        );
    }

    #[test]
    fn every_public_error_has_a_stable_safe_projection() {
        let errors = [
            AiRoutingError::invalid("bad_route", "Correct the route"),
            AiRoutingError::NotFound,
            AiRoutingError::conflict("route_changed", "Reload the route"),
            AiRoutingError::NoMatchingRoute,
            AiRoutingError::UnusableRoute {
                route_set_id: Uuid::new_v4(),
                reason: RouteUnusableReason::EmptyChain,
            },
            AiRoutingError::UnusableRoute {
                route_set_id: Uuid::new_v4(),
                reason: RouteUnusableReason::ConnectionUnavailable,
            },
            AiRoutingError::UnusableRoute {
                route_set_id: Uuid::new_v4(),
                reason: RouteUnusableReason::ModelLimitsUnavailable,
            },
            AiRoutingError::UnusableRoute {
                route_set_id: Uuid::new_v4(),
                reason: RouteUnusableReason::ToolsUnsupported,
            },
            AiRoutingError::Storage(sqlx::Error::RowNotFound),
        ];
        for error in errors {
            assert!(!error.code().is_empty());
            assert!(!error.safe_message().is_empty());
        }
    }
}
