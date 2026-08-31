//! Defines refined Session, run, queue, and event contracts for the durable Agent runtime.
//!
//! Commands contain no tenant or user identity: trusted callers provide those identities to
//! `AgentSessionOps`. Public history projections contain user-visible text and reduced state only,
//! while worker lease proofs are non-serializable and fence every background transition.

use std::str::FromStr;

use chrono::{DateTime, Utc};
use cp_common::{ProviderDataClass, ProviderExecutionEnvironmentClass};
use serde::{Serialize, Serializer};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::TaskClass;

const DEFAULT_PAGE_SIZE: u16 = 30;
const MAX_PAGE_SIZE: u16 = 100;
const MAX_SESSION_TITLE_LENGTH: usize = 120;
const MAX_MESSAGE_LENGTH: usize = 20_000;
const MAX_IDEMPOTENCY_KEY_LENGTH: usize = 200;
const MAX_MODULE_KEY_LENGTH: usize = 160;
const MAX_ORIGIN_ROUTE_LENGTH: usize = 500;
const MAX_WORKER_ID_LENGTH: usize = 120;
const MAX_SAFE_FAILURE_CODE_LENGTH: usize = 100;
const MAX_SAFE_FAILURE_MESSAGE_LENGTH: usize = 500;
const MAX_CLAIM_BATCH: u16 = 25;
const MAX_PROVIDER_TURNS: i16 = 16;
const MAX_PROVIDER_ATTEMPTS_PER_TURN: i16 = 3;
const MAX_CAPABILITY_CALLS: i16 = 16;
const MAX_PROVIDER_MODEL_ID_LENGTH: usize = 240;
const MAX_CAPABILITY_KEY_LENGTH: usize = 200;
const MAX_OPERATION_KEY_LENGTH: usize = 240;
const MAX_PERMISSION_KEY_LENGTH: usize = 200;
const MAX_RESOURCE_KIND_LENGTH: usize = 120;
const MAX_RESOURCE_ID_LENGTH: usize = 240;
const MAX_RESOURCE_REFERENCES: usize = 32;
const MAX_ENCRYPTION_KEY_ID_LENGTH: usize = 200;
const MAX_ARTIFACT_PLAINTEXT_BYTES: usize = 65_536;
const MAX_ARTIFACT_CIPHERTEXT_BYTES: usize = 65_552;
const MIN_ARTIFACT_NONCE_BYTES: usize = 12;
const MAX_ARTIFACT_NONCE_BYTES: usize = 32;
const MAX_JS_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

/// Bounded cursor-page request shared by Session, message, run, and event reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageLimit(u16);

impl PageLimit {
    pub fn parse(value: Option<u16>) -> Result<Self, AgentSessionError> {
        let value = value.unwrap_or(DEFAULT_PAGE_SIZE);
        if value == 0 || value > MAX_PAGE_SIZE {
            return Err(AgentSessionError::invalid(
                "invalid_page_limit",
                "Page size must be between 1 and 100",
            ));
        }
        Ok(Self(value))
    }

    pub(crate) const fn get(self) -> i64 {
        self.0 as i64
    }
}

/// Cursor for stable newest-first Session history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SessionCursor {
    pub last_activity_at: DateTime<Utc>,
    pub session_id: Uuid,
}

/// Cursor for stable ascending transcript reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MessageCursor {
    pub sequence: i64,
    pub message_id: Uuid,
}

impl MessageCursor {
    pub fn parse(sequence: i64, message_id: Uuid) -> Result<Self, AgentSessionError> {
        if sequence <= 0 {
            return Err(AgentSessionError::invalid(
                "invalid_message_cursor",
                "Message cursor sequence must be positive",
            ));
        }
        Ok(Self {
            sequence,
            message_id,
        })
    }
}

/// Cursor for stable newest-first run history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RunCursor {
    pub created_at: DateTime<Utc>,
    pub run_id: Uuid,
}

/// Cursor for append-only event replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventCursor(pub(crate) i64);

impl EventCursor {
    /// Parses an opaque decimal cursor without exposing PostgreSQL BIGINT as a JSON number.
    pub fn parse(value: &str) -> Result<Self, AgentSessionError> {
        let parsed = value.parse::<i64>().map_err(|_| {
            AgentSessionError::invalid(
                "invalid_event_cursor",
                "Event cursor must be a non-negative decimal string",
            )
        })?;
        if parsed < 0 {
            return Err(AgentSessionError::invalid(
                "invalid_event_cursor",
                "Event cursor must be a non-negative decimal string",
            ));
        }
        Ok(Self(parsed))
    }

    pub(crate) const fn get(self) -> i64 {
        self.0
    }
}

impl Serialize for EventCursor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

/// One cursor page and the cursor to request after its final record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CursorPage<T, C> {
    pub items: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<C>,
}

/// Session lifecycle visible to its owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Active,
    Archived,
}

impl FromStr for SessionStatus {
    type Err = AgentSessionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "active" => Ok(Self::Active),
            "archived" => Ok(Self::Archived),
            _ => Err(AgentSessionError::storage_contract()),
        }
    }
}

/// Owner-visible durable Session projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentSession {
    pub id: Uuid,
    pub title: String,
    pub status: SessionStatus,
    pub version: i64,
    pub last_activity_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Owner-visible transcript role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
}

impl FromStr for MessageRole {
    type Err = AgentSessionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "user" => Ok(Self::User),
            "assistant" => Ok(Self::Assistant),
            _ => Err(AgentSessionError::storage_contract()),
        }
    }
}

/// One user-visible, append-only Session message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentMessage {
    pub id: Uuid,
    pub session_id: Uuid,
    pub sequence: i64,
    pub role: MessageRole,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

/// Durable run state. Terminal states never transition again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    AwaitingApproval,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

impl RunStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::AwaitingApproval => "awaiting_approval",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::Interrupted
        )
    }
}

impl FromStr for RunStatus {
    type Err = AgentSessionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "awaiting_approval" => Ok(Self::AwaitingApproval),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "interrupted" => Ok(Self::Interrupted),
            _ => Err(AgentSessionError::storage_contract()),
        }
    }
}

/// Owner-visible run projection. Provider internals, role snapshots, and raw trail data are absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentRun {
    pub id: Uuid,
    pub session_id: Uuid,
    pub request_message_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_message_id: Option<Uuid>,
    pub task_class: TaskClass,
    pub origin_module_key: String,
    pub origin_route: String,
    pub status: RunStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safe_failure_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safe_failure_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Stable event kinds persisted for polling or SSE replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunEventType {
    Queued,
    Started,
    ProviderAttemptStarted,
    ProviderAttemptFinished,
    CapabilityCallStarted,
    CapabilityCallFinished,
    MessageCreated,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

impl RunEventType {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Started => "started",
            Self::ProviderAttemptStarted => "provider_attempt_started",
            Self::ProviderAttemptFinished => "provider_attempt_finished",
            Self::CapabilityCallStarted => "capability_call_started",
            Self::CapabilityCallFinished => "capability_call_finished",
            Self::MessageCreated => "message_created",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }
}

impl FromStr for RunEventType {
    type Err = AgentSessionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "queued" => Ok(Self::Queued),
            "started" => Ok(Self::Started),
            "provider_attempt_started" => Ok(Self::ProviderAttemptStarted),
            "provider_attempt_finished" => Ok(Self::ProviderAttemptFinished),
            "capability_call_started" => Ok(Self::CapabilityCallStarted),
            "capability_call_finished" => Ok(Self::CapabilityCallFinished),
            "message_created" => Ok(Self::MessageCreated),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "interrupted" => Ok(Self::Interrupted),
            _ => Err(AgentSessionError::storage_contract()),
        }
    }
}

/// Reduced replay event. Consumers reload the typed run or message projection for details.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AgentRunEvent {
    /// Opaque decimal replay cursor; it must never be converted to a JavaScript number.
    pub cursor: String,
    pub run_id: Uuid,
    pub event_type: RunEventType,
    pub created_at: DateTime<Utc>,
}

/// Queue checkpoint used to make safe and unsafe recovery decisions explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunCheckpoint {
    Queued,
    BeforeProvider,
    ProviderInFlight,
    ProviderResultPersisted,
    CapabilityInFlight,
    CapabilityResultPersisted,
    Finalizing,
}

impl RunCheckpoint {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::BeforeProvider => "before_provider",
            Self::ProviderInFlight => "provider_in_flight",
            Self::ProviderResultPersisted => "provider_result_persisted",
            Self::CapabilityInFlight => "capability_in_flight",
            Self::CapabilityResultPersisted => "capability_result_persisted",
            Self::Finalizing => "finalizing",
        }
    }

    #[must_use]
    pub const fn is_automatically_recoverable(self) -> bool {
        matches!(self, Self::Queued | Self::BeforeProvider)
    }

    pub(crate) fn can_advance_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Queued, Self::BeforeProvider)
                | (Self::BeforeProvider, Self::ProviderInFlight)
                | (Self::ProviderInFlight, Self::ProviderResultPersisted)
                | (Self::ProviderResultPersisted, Self::CapabilityInFlight)
                | (Self::ProviderResultPersisted, Self::Finalizing)
                | (Self::ProviderResultPersisted, Self::BeforeProvider)
                | (Self::CapabilityInFlight, Self::CapabilityResultPersisted)
                | (Self::CapabilityResultPersisted, Self::BeforeProvider)
                | (Self::CapabilityResultPersisted, Self::ProviderInFlight)
                | (Self::CapabilityResultPersisted, Self::Finalizing)
        )
    }
}

impl FromStr for RunCheckpoint {
    type Err = AgentSessionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "queued" => Ok(Self::Queued),
            "before_provider" => Ok(Self::BeforeProvider),
            "provider_in_flight" => Ok(Self::ProviderInFlight),
            "provider_result_persisted" => Ok(Self::ProviderResultPersisted),
            "capability_in_flight" => Ok(Self::CapabilityInFlight),
            "capability_result_persisted" => Ok(Self::CapabilityResultPersisted),
            "finalizing" => Ok(Self::Finalizing),
            _ => Err(AgentSessionError::storage_contract()),
        }
    }
}

/// One provider turn within a run. A turn may make up to three routed attempts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderTurnIndex(i16);

impl ProviderTurnIndex {
    pub fn parse(value: u16) -> Result<Self, AgentSessionError> {
        let value = i16::try_from(value).map_err(|_| invalid_provider_turn())?;
        if !(1..=MAX_PROVIDER_TURNS).contains(&value) {
            return Err(invalid_provider_turn());
        }
        Ok(Self(value))
    }

    pub(crate) const fn get(self) -> i16 {
        self.0
    }
}

/// One ordered provider fallback attempt inside a turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderAttemptIndex(i16);

impl ProviderAttemptIndex {
    pub fn parse(value: u8) -> Result<Self, AgentSessionError> {
        let value = i16::from(value);
        if !(1..=MAX_PROVIDER_ATTEMPTS_PER_TURN).contains(&value) {
            return Err(AgentSessionError::invalid(
                "invalid_provider_attempt_index",
                "Provider attempt index must be between 1 and 3",
            ));
        }
        Ok(Self(value))
    }

    pub(crate) const fn get(self) -> i16 {
        self.0
    }
}

/// Supported provider identity persisted with a durable attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentProviderKey {
    OpenAi,
    Anthropic,
    OpenRouter,
}

impl AgentProviderKey {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::OpenRouter => "openrouter",
        }
    }

    pub(crate) fn from_stored(value: &str) -> Result<Self, AgentSessionError> {
        match value {
            "openai" => Ok(Self::OpenAi),
            "anthropic" => Ok(Self::Anthropic),
            "openrouter" => Ok(Self::OpenRouter),
            _ => Err(AgentSessionError::storage_contract()),
        }
    }
}

/// Immutable route identity used to prepare one provider attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderAttemptPlan {
    pub(crate) turn_index: ProviderTurnIndex,
    pub(crate) attempt_index: ProviderAttemptIndex,
    pub(crate) route_set_id: Uuid,
    pub(crate) route_version: i64,
    pub(crate) route_target_id: Uuid,
    pub(crate) connection_id: Uuid,
    pub(crate) credential_version: i64,
    pub(crate) model_snapshot_id: Uuid,
    pub(crate) provider_data_approval_id: Uuid,
    pub(crate) required_provider_data_class: ProviderDataClass,
    pub(crate) execution_environment_class: ProviderExecutionEnvironmentClass,
    pub(crate) provider_key: AgentProviderKey,
    pub(crate) provider_model_id: String,
    pub(crate) input_fingerprint: [u8; 32],
}

impl ProviderAttemptPlan {
    #[allow(clippy::too_many_arguments)]
    pub fn parse(
        turn_index: u16,
        attempt_index: u8,
        route_set_id: Uuid,
        route_version: i64,
        route_target_id: Uuid,
        connection_id: Uuid,
        credential_version: i64,
        model_snapshot_id: Uuid,
        provider_data_approval_id: Uuid,
        required_provider_data_class: ProviderDataClass,
        execution_environment_class: ProviderExecutionEnvironmentClass,
        provider_key: AgentProviderKey,
        provider_model_id: &str,
        input_fingerprint: [u8; 32],
    ) -> Result<Self, AgentSessionError> {
        Ok(Self {
            turn_index: ProviderTurnIndex::parse(turn_index)?,
            attempt_index: ProviderAttemptIndex::parse(attempt_index)?,
            route_set_id,
            route_version: positive_version(route_version)?,
            route_target_id,
            connection_id,
            credential_version: positive_version(credential_version)?,
            model_snapshot_id,
            provider_data_approval_id,
            required_provider_data_class,
            execution_environment_class,
            provider_key,
            provider_model_id: parse_bounded_text(
                provider_model_id,
                MAX_PROVIDER_MODEL_ID_LENGTH,
                "invalid_provider_model_id",
                "Provider model ID must contain between 1 and 240 characters",
            )?,
            input_fingerprint,
        })
    }
}

/// Durable provider-attempt and execution-step identity returned by preparation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderAttemptIdentity {
    pub attempt_id: Uuid,
    pub step_id: Uuid,
    pub turn_index: ProviderTurnIndex,
    pub attempt_index: ProviderAttemptIndex,
}

/// Provider attempt preparation result carrying the next queue fence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedProviderAttempt {
    pub lease: RunLease,
    pub identity: ProviderAttemptIdentity,
}

/// Preflight failures happen before an upstream request can begin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderPreflightFailure {
    ConnectionUnavailable,
    StaleCredential,
    StaleModel,
    ToolsUnsupported,
    ModelContextUnavailable,
    ModelOutputUnavailable,
    ContextWindowExceeded,
    OutputBudgetExceeded,
    CredentialUnavailable,
    InvalidConfiguration,
    InvalidInput,
    StorageError,
    ProviderDataNotApproved,
    ProviderDataApprovalChanged,
    LocalExecutionRequired,
}

impl ProviderPreflightFailure {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ConnectionUnavailable => "connection_unavailable",
            Self::StaleCredential => "stale_credential",
            Self::StaleModel => "stale_model",
            Self::ToolsUnsupported => "tools_unsupported",
            Self::ModelContextUnavailable => "model_context_unavailable",
            Self::ModelOutputUnavailable => "model_output_unavailable",
            Self::ContextWindowExceeded => "context_window_exceeded",
            Self::OutputBudgetExceeded => "output_budget_exceeded",
            Self::CredentialUnavailable => "credential_unavailable",
            Self::InvalidConfiguration => "invalid_configuration",
            Self::InvalidInput => "invalid_input",
            Self::StorageError => "storage_error",
            Self::ProviderDataNotApproved => "provider_data_not_approved",
            Self::ProviderDataApprovalChanged => "provider_data_approval_changed",
            Self::LocalExecutionRequired => "local_execution_required",
        }
    }

    pub(crate) fn from_stored(value: &str) -> Result<Self, AgentSessionError> {
        match value {
            "connection_unavailable" => Ok(Self::ConnectionUnavailable),
            "stale_credential" => Ok(Self::StaleCredential),
            "stale_model" => Ok(Self::StaleModel),
            "tools_unsupported" => Ok(Self::ToolsUnsupported),
            "model_context_unavailable" => Ok(Self::ModelContextUnavailable),
            "model_output_unavailable" => Ok(Self::ModelOutputUnavailable),
            "context_window_exceeded" => Ok(Self::ContextWindowExceeded),
            "output_budget_exceeded" => Ok(Self::OutputBudgetExceeded),
            "credential_unavailable" => Ok(Self::CredentialUnavailable),
            "invalid_configuration" => Ok(Self::InvalidConfiguration),
            "invalid_input" => Ok(Self::InvalidInput),
            "storage_error" => Ok(Self::StorageError),
            "provider_data_not_approved" => Ok(Self::ProviderDataNotApproved),
            "provider_data_approval_changed" => Ok(Self::ProviderDataApprovalChanged),
            "local_execution_required" => Ok(Self::LocalExecutionRequired),
            _ => Err(AgentSessionError::storage_contract()),
        }
    }
}

/// Safe normalized failure returned after contacting an upstream provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderUpstreamFailure {
    Authentication,
    RateLimited,
    Unavailable,
    Timeout,
    Network,
    InvalidResponse,
    Unsupported,
}

impl ProviderUpstreamFailure {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Authentication => "authentication",
            Self::RateLimited => "rate_limited",
            Self::Unavailable => "unavailable",
            Self::Timeout => "timeout",
            Self::Network => "network",
            Self::InvalidResponse => "invalid_response",
            Self::Unsupported => "unsupported",
        }
    }

    pub(crate) fn from_stored(value: &str) -> Result<Self, AgentSessionError> {
        match value {
            "authentication" => Ok(Self::Authentication),
            "rate_limited" => Ok(Self::RateLimited),
            "unavailable" => Ok(Self::Unavailable),
            "timeout" => Ok(Self::Timeout),
            "network" => Ok(Self::Network),
            "invalid_response" => Ok(Self::InvalidResponse),
            "unsupported" => Ok(Self::Unsupported),
            _ => Err(AgentSessionError::storage_contract()),
        }
    }
}

/// Failure provenance persisted on a known failed provider attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderAttemptFailure {
    Preflight(ProviderPreflightFailure),
    Upstream(ProviderUpstreamFailure),
}

impl ProviderAttemptFailure {
    pub(crate) const fn origin(self) -> &'static str {
        match self {
            Self::Preflight(_) => "preflight",
            Self::Upstream(_) => "upstream",
        }
    }

    pub(crate) const fn category(self) -> &'static str {
        match self {
            Self::Preflight(failure) => failure.as_str(),
            Self::Upstream(failure) => failure.as_str(),
        }
    }

    pub(crate) fn from_stored(origin: &str, category: &str) -> Result<Self, AgentSessionError> {
        match origin {
            "preflight" => Ok(Self::Preflight(ProviderPreflightFailure::from_stored(
                category,
            )?)),
            "upstream" => Ok(Self::Upstream(ProviderUpstreamFailure::from_stored(
                category,
            )?)),
            _ => Err(AgentSessionError::storage_contract()),
        }
    }
}

/// One normalized currency amount. Unknown costs remain `None`, never zero-filled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedCost {
    pub(crate) amount: i64,
    pub(crate) currency: String,
    pub(crate) exponent: i16,
    pub(crate) pricing_version: Option<String>,
}

impl NormalizedCost {
    pub fn provider_reported(
        amount: u64,
        currency: &str,
        exponent: u8,
        pricing_version: Option<&str>,
    ) -> Result<Self, AgentSessionError> {
        Self::parse(amount, currency, exponent, pricing_version, false)
    }

    pub fn estimated(
        amount: u64,
        currency: &str,
        exponent: u8,
        pricing_version: &str,
    ) -> Result<Self, AgentSessionError> {
        Self::parse(amount, currency, exponent, Some(pricing_version), true)
    }

    fn parse(
        amount: u64,
        currency: &str,
        exponent: u8,
        pricing_version: Option<&str>,
        pricing_version_required: bool,
    ) -> Result<Self, AgentSessionError> {
        let amount = js_safe_counter(amount, "invalid_provider_cost")?;
        if currency.len() != 3 || !currency.bytes().all(|byte| byte.is_ascii_uppercase()) {
            return Err(AgentSessionError::invalid(
                "invalid_provider_cost_currency",
                "Provider cost currency must be a three-letter uppercase code",
            ));
        }
        if exponent > 9 {
            return Err(AgentSessionError::invalid(
                "invalid_provider_cost_exponent",
                "Provider cost exponent must be between 0 and 9",
            ));
        }
        let pricing_version = pricing_version
            .map(|value| {
                parse_bounded_text(
                    value,
                    100,
                    "invalid_pricing_version",
                    "Pricing version must contain between 1 and 100 characters",
                )
            })
            .transpose()?;
        if pricing_version_required && pricing_version.is_none() {
            return Err(AgentSessionError::invalid(
                "invalid_pricing_version",
                "Estimated provider cost requires a pricing version",
            ));
        }
        Ok(Self {
            amount,
            currency: currency.to_owned(),
            exponent: i16::from(exponent),
            pricing_version,
        })
    }
}

/// Nullable normalized usage for one provider attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedProviderUsage {
    pub(crate) input_tokens: Option<i64>,
    pub(crate) output_tokens: Option<i64>,
    pub(crate) cached_tokens: Option<i64>,
    pub(crate) reasoning_tokens: Option<i64>,
    pub(crate) provider_reported_cost: Option<NormalizedCost>,
    pub(crate) estimated_cost: Option<NormalizedCost>,
}

impl NormalizedProviderUsage {
    pub fn parse(
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        cached_tokens: Option<u64>,
        reasoning_tokens: Option<u64>,
        provider_reported_cost: Option<NormalizedCost>,
        estimated_cost: Option<NormalizedCost>,
    ) -> Result<Self, AgentSessionError> {
        if estimated_cost
            .as_ref()
            .is_some_and(|cost| cost.pricing_version.is_none())
        {
            return Err(AgentSessionError::invalid(
                "invalid_pricing_version",
                "Estimated provider cost requires a pricing version",
            ));
        }
        Ok(Self {
            input_tokens: optional_js_safe_counter(input_tokens, "invalid_input_tokens")?,
            output_tokens: optional_js_safe_counter(output_tokens, "invalid_output_tokens")?,
            cached_tokens: optional_js_safe_counter(cached_tokens, "invalid_cached_tokens")?,
            reasoning_tokens: optional_js_safe_counter(
                reasoning_tokens,
                "invalid_reasoning_tokens",
            )?,
            provider_reported_cost,
            estimated_cost,
        })
    }

    #[must_use]
    pub const fn unknown() -> Self {
        Self {
            input_tokens: None,
            output_tokens: None,
            cached_tokens: None,
            reasoning_tokens: None,
            provider_reported_cost: None,
            estimated_cost: None,
        }
    }
}

/// Ordered capability invocation sequence across a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityCallSequence(i16);

impl CapabilityCallSequence {
    pub fn parse(value: u16) -> Result<Self, AgentSessionError> {
        let value = i16::try_from(value).map_err(|_| invalid_capability_sequence())?;
        if !(1..=MAX_CAPABILITY_CALLS).contains(&value) {
            return Err(invalid_capability_sequence());
        }
        Ok(Self(value))
    }

    pub(crate) const fn get(self) -> i16 {
        self.0
    }
}

/// Minimum typed resource identity retained for a scoped capability call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityResourceReference {
    pub(crate) kind: String,
    pub(crate) id: String,
}

impl CapabilityResourceReference {
    pub fn parse(kind: &str, id: &str) -> Result<Self, AgentSessionError> {
        Ok(Self {
            kind: parse_bounded_text(
                kind,
                MAX_RESOURCE_KIND_LENGTH,
                "invalid_resource_reference",
                "Resource reference kind must contain between 1 and 120 characters",
            )?,
            id: parse_bounded_text(
                id,
                MAX_RESOURCE_ID_LENGTH,
                "invalid_resource_reference",
                "Resource reference ID must contain between 1 and 240 characters",
            )?,
        })
    }
}

/// Capability scope contains no tenant or person identity supplied by a model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityCallScope {
    TenantWide,
    Resources(Vec<CapabilityResourceReference>),
}

impl CapabilityCallScope {
    pub fn resources(
        references: Vec<CapabilityResourceReference>,
    ) -> Result<Self, AgentSessionError> {
        if references.is_empty() || references.len() > MAX_RESOURCE_REFERENCES {
            return Err(AgentSessionError::invalid(
                "invalid_resource_references",
                "Capability resource scope must contain between 1 and 32 references",
            ));
        }
        Ok(Self::Resources(references))
    }

    pub(crate) const fn kind(&self) -> &'static str {
        match self {
            Self::TenantWide => "tenant_wide",
            Self::Resources(_) => "resources",
        }
    }
}

/// Immutable typed identity used to prepare one capability call and execution step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityCallPlan {
    pub(crate) call_id: Uuid,
    pub(crate) turn_index: ProviderTurnIndex,
    pub(crate) call_sequence: CapabilityCallSequence,
    pub(crate) capability_key: String,
    pub(crate) capability_version: i32,
    pub(crate) product_operation_key: String,
    pub(crate) owning_module_key: String,
    pub(crate) required_permission: String,
    pub(crate) input_fingerprint: [u8; 32],
    pub(crate) scope: CapabilityCallScope,
}

impl CapabilityCallPlan {
    #[allow(clippy::too_many_arguments)]
    pub fn parse(
        call_id: Uuid,
        turn_index: u16,
        call_sequence: u16,
        capability_key: &str,
        capability_version: i32,
        product_operation_key: &str,
        owning_module_key: &str,
        required_permission: &str,
        input_fingerprint: [u8; 32],
        scope: CapabilityCallScope,
    ) -> Result<Self, AgentSessionError> {
        if capability_version <= 0 {
            return Err(AgentSessionError::invalid(
                "invalid_capability_version",
                "Capability version must be positive",
            ));
        }
        let required_permission = parse_permission_key(required_permission)?;
        Ok(Self {
            call_id,
            turn_index: ProviderTurnIndex::parse(turn_index)?,
            call_sequence: CapabilityCallSequence::parse(call_sequence)?,
            capability_key: parse_stable_key(
                capability_key,
                MAX_CAPABILITY_KEY_LENGTH,
                "invalid_capability_key",
            )?,
            capability_version,
            product_operation_key: parse_stable_key(
                product_operation_key,
                MAX_OPERATION_KEY_LENGTH,
                "invalid_product_operation_key",
            )?,
            owning_module_key: parse_stable_key(
                owning_module_key,
                MAX_MODULE_KEY_LENGTH,
                "invalid_owning_module_key",
            )?,
            required_permission,
            input_fingerprint,
            scope,
        })
    }
}

/// Durable capability-call and execution-step identity returned by preparation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityCallIdentity {
    pub call_id: Uuid,
    pub step_id: Uuid,
    pub turn_index: ProviderTurnIndex,
    pub call_sequence: CapabilityCallSequence,
}

/// Capability call preparation result carrying the next queue fence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedCapabilityCall {
    pub lease: RunLease,
    pub identity: CapabilityCallIdentity,
}

/// Bounded capability execution duration retained as a JavaScript-safe counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityCallDuration(i64);

impl CapabilityCallDuration {
    pub fn parse(duration_ms: u64) -> Result<Self, AgentSessionError> {
        Ok(Self(js_safe_counter(
            duration_ms,
            "invalid_capability_duration",
        )?))
    }

    pub(crate) const fn get(self) -> i64 {
        self.0
    }
}

/// Safe known capability outcome that did not produce a model-visible result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityCallFailure {
    pub(crate) status: CapabilityFailureStatus,
    pub(crate) safe_failure_code: String,
    pub(crate) duration_ms: i64,
}

impl CapabilityCallFailure {
    pub fn parse(
        status: CapabilityFailureStatus,
        safe_failure_code: &str,
        duration_ms: u64,
    ) -> Result<Self, AgentSessionError> {
        Ok(Self {
            status,
            safe_failure_code: parse_stable_failure_code(safe_failure_code)?,
            duration_ms: CapabilityCallDuration::parse(duration_ms)?.get(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityFailureStatus {
    Failed,
    Denied,
}

impl CapabilityFailureStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Failed => "failed",
            Self::Denied => "denied",
        }
    }
}

/// Artifact kind is derived from its execution step and cannot carry arbitrary labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionArtifactKind {
    ProviderResult,
    CapabilityResult,
    FinalResponse,
}

impl ExecutionArtifactKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ProviderResult => "provider_result",
            Self::CapabilityResult => "capability_result",
            Self::FinalResponse => "final_response",
        }
    }
}

impl FromStr for ExecutionArtifactKind {
    type Err = AgentSessionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "provider_result" => Ok(Self::ProviderResult),
            "capability_result" => Ok(Self::CapabilityResult),
            "final_response" => Ok(Self::FinalResponse),
            _ => Err(AgentSessionError::storage_contract()),
        }
    }
}

/// Opaque encrypted continuation envelope.
///
/// This type intentionally implements neither `Debug`, `Clone`, nor `Serialize` so ciphertext
/// and envelope metadata cannot drift into logs or API projections through derives.
pub struct EncryptedExecutionArtifact {
    pub(crate) ciphertext: Vec<u8>,
    pub(crate) ciphertext_sha256: [u8; 32],
    pub(crate) plaintext_sha256: [u8; 32],
    pub(crate) nonce: Vec<u8>,
    pub(crate) encryption_key_id: String,
    pub(crate) encryption_key_version: i64,
    pub(crate) plaintext_length: i32,
}

impl EncryptedExecutionArtifact {
    pub fn parse(
        ciphertext: Vec<u8>,
        plaintext_sha256: [u8; 32],
        nonce: Vec<u8>,
        encryption_key_id: &str,
        encryption_key_version: i64,
        plaintext_length: usize,
    ) -> Result<Self, AgentSessionError> {
        if ciphertext.is_empty() || ciphertext.len() > MAX_ARTIFACT_CIPHERTEXT_BYTES {
            return Err(AgentSessionError::invalid(
                "invalid_artifact_ciphertext",
                "Encrypted Agent artifact must contain at most 65,552 bytes",
            ));
        }
        if !(MIN_ARTIFACT_NONCE_BYTES..=MAX_ARTIFACT_NONCE_BYTES).contains(&nonce.len()) {
            return Err(AgentSessionError::invalid(
                "invalid_artifact_nonce",
                "Encrypted Agent artifact nonce must contain between 12 and 32 bytes",
            ));
        }
        if plaintext_length == 0 || plaintext_length > MAX_ARTIFACT_PLAINTEXT_BYTES {
            return Err(AgentSessionError::invalid(
                "invalid_artifact_plaintext_length",
                "Encrypted Agent artifact plaintext must contain at most 65,536 bytes",
            ));
        }
        let encryption_key_id = parse_bounded_text(
            encryption_key_id,
            MAX_ENCRYPTION_KEY_ID_LENGTH,
            "invalid_artifact_key_id",
            "Artifact encryption key ID must contain between 1 and 200 characters",
        )?;
        if encryption_key_id.chars().any(char::is_control) {
            return Err(AgentSessionError::invalid(
                "invalid_artifact_key_id",
                "Artifact encryption key ID cannot contain control characters",
            ));
        }
        let ciphertext_sha256 = Sha256::digest(&ciphertext).into();
        Ok(Self {
            ciphertext,
            ciphertext_sha256,
            plaintext_sha256,
            nonce,
            encryption_key_id,
            encryption_key_version: positive_version(encryption_key_version)?,
            plaintext_length: i32::try_from(plaintext_length)
                .map_err(|_| AgentSessionError::storage_contract())?,
        })
    }

    #[must_use]
    pub fn ciphertext(&self) -> &[u8] {
        &self.ciphertext
    }

    #[must_use]
    pub fn nonce(&self) -> &[u8] {
        &self.nonce
    }

    #[must_use]
    pub fn encryption_key_id(&self) -> &str {
        &self.encryption_key_id
    }

    #[must_use]
    pub const fn encryption_key_version(&self) -> i64 {
        self.encryption_key_version
    }

    #[must_use]
    pub const fn plaintext_sha256(&self) -> &[u8; 32] {
        &self.plaintext_sha256
    }

    #[must_use]
    pub const fn plaintext_length(&self) -> i32 {
        self.plaintext_length
    }
}

/// Persisted artifact identity returned after a fenced result write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PersistedExecutionArtifact {
    pub id: Uuid,
    pub step_id: Uuid,
    pub kind: ExecutionArtifactKind,
    pub sequence: i16,
}

/// Fenced result write and its durable artifact identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedExecutionResult {
    pub lease: RunLease,
    pub artifact: PersistedExecutionArtifact,
}

/// Loaded encrypted envelope for worker-only crash recovery.
///
/// The type deliberately has no `Debug`, `Clone`, or `Serialize` implementation.
pub struct LoadedExecutionArtifact {
    pub id: Uuid,
    pub step_id: Uuid,
    pub kind: ExecutionArtifactKind,
    pub sequence: i16,
    envelope: EncryptedExecutionArtifact,
}

/// Typed worker-only view of the durable execution trail for one fenced run.
///
/// This projection deliberately has no serialization implementation. It contains only bounded,
/// normalized execution facts and opaque encrypted envelopes, never provider bodies, capability
/// inputs, credentials, or decrypted results.
pub struct ExecutionSnapshot {
    pub run_id: Uuid,
    pub checkpoint: RunCheckpoint,
    pub steps: Vec<ExecutionStepSnapshot>,
}

/// One ordered execution step and its kind-specific reduced outcome.
pub enum ExecutionStepSnapshot {
    ProviderAttempt(ProviderAttemptSnapshot),
    CapabilityCall(CapabilityCallSnapshot),
    Finalization(FinalizationSnapshot),
}

/// Durable facts common to every private execution step.
pub struct ExecutionStepEvidence {
    pub step_id: Uuid,
    pub step_index: u16,
    pub turn_index: ProviderTurnIndex,
    pub input_fingerprint: [u8; 32],
    pub status: ExecutionStepStatus,
    pub safe_failure_code: Option<String>,
    pub artifact: Option<LoadedExecutionArtifact>,
}

/// Reduced provider-attempt facts needed to resume a fallback chain safely.
pub struct ProviderAttemptSnapshot {
    pub step: ExecutionStepEvidence,
    pub identity: ProviderAttemptIdentity,
    pub route_set_id: Uuid,
    pub route_version: i64,
    pub route_target_id: Uuid,
    pub connection_id: Uuid,
    pub credential_version: i64,
    pub model_snapshot_id: Uuid,
    pub provider_data_approval_id: Uuid,
    pub required_provider_data_class: ProviderDataClass,
    pub execution_environment_class: ProviderExecutionEnvironmentClass,
    pub provider_key: AgentProviderKey,
    pub provider_model_id: String,
    pub task_class: TaskClass,
    pub status: ProviderAttemptStatus,
    pub failure: Option<ProviderAttemptFailure>,
}

/// Reduced capability-call facts needed to replay its persisted model-visible result.
pub struct CapabilityCallSnapshot {
    pub step: ExecutionStepEvidence,
    pub identity: CapabilityCallIdentity,
    pub capability_key: String,
    pub capability_version: i32,
    pub product_operation_key: String,
    pub owning_module_key: String,
    pub required_permission: String,
    pub scope: CapabilityCallScope,
    pub status: CapabilityCallStatus,
    pub safe_failure_code: Option<String>,
    pub duration_ms: Option<u64>,
}

/// Finalization evidence. A succeeded finalization always carries the unique final artifact.
pub struct FinalizationSnapshot {
    pub step: ExecutionStepEvidence,
}

/// Lifecycle of a durable execution step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionStepStatus {
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

impl FromStr for ExecutionStepStatus {
    type Err = AgentSessionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "interrupted" => Ok(Self::Interrupted),
            _ => Err(AgentSessionError::storage_contract()),
        }
    }
}

/// Lifecycle of a provider attempt in the private execution snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderAttemptStatus {
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

impl FromStr for ProviderAttemptStatus {
    type Err = AgentSessionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "interrupted" => Ok(Self::Interrupted),
            _ => Err(AgentSessionError::storage_contract()),
        }
    }
}

/// Lifecycle of a capability call in the private execution snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityCallStatus {
    Running,
    Succeeded,
    Failed,
    Denied,
    Cancelled,
    Interrupted,
}

impl FromStr for CapabilityCallStatus {
    type Err = AgentSessionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "denied" => Ok(Self::Denied),
            "cancelled" => Ok(Self::Cancelled),
            "interrupted" => Ok(Self::Interrupted),
            _ => Err(AgentSessionError::storage_contract()),
        }
    }
}

impl LoadedExecutionArtifact {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_stored(
        id: Uuid,
        step_id: Uuid,
        kind: ExecutionArtifactKind,
        sequence: i16,
        ciphertext: Vec<u8>,
        stored_ciphertext_sha256: [u8; 32],
        plaintext_sha256: [u8; 32],
        nonce: Vec<u8>,
        encryption_key_id: String,
        encryption_key_version: i64,
        plaintext_length: usize,
    ) -> Result<Self, AgentSessionError> {
        let envelope = EncryptedExecutionArtifact::parse(
            ciphertext,
            plaintext_sha256,
            nonce,
            &encryption_key_id,
            encryption_key_version,
            plaintext_length,
        )?;
        if envelope.ciphertext_sha256 != stored_ciphertext_sha256 {
            return Err(AgentSessionError::storage_contract());
        }
        Ok(Self {
            id,
            step_id,
            kind,
            sequence,
            envelope,
        })
    }

    #[must_use]
    pub const fn envelope(&self) -> &EncryptedExecutionArtifact {
        &self.envelope
    }

    #[must_use]
    pub fn into_envelope(self) -> EncryptedExecutionArtifact {
        self.envelope
    }
}

/// Bounded final response plaintext supplied only after decrypting a durable final artifact.
///
/// This type intentionally implements neither `Debug`, `Clone`, nor `Serialize`.
pub struct FinalResponsePlaintext(String);

impl FinalResponsePlaintext {
    pub fn parse(value: String) -> Result<Self, AgentSessionError> {
        Ok(Self(parse_assistant_message(&value)?))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn sha256(&self) -> [u8; 32] {
        Sha256::digest(self.0.as_bytes()).into()
    }

    pub(crate) fn byte_len(&self) -> usize {
        self.0.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IdempotencyKey(String);

impl IdempotencyKey {
    fn parse(value: impl Into<String>) -> Result<Self, AgentSessionError> {
        let value = value.into().trim().to_owned();
        if value.is_empty() || value.chars().count() > MAX_IDEMPOTENCY_KEY_LENGTH {
            return Err(AgentSessionError::invalid(
                "invalid_idempotency_key",
                "Idempotency key must contain between 1 and 200 characters",
            ));
        }
        Ok(Self(value))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

/// Refined Session list input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListSessionsQuery {
    pub(crate) limit: PageLimit,
    pub(crate) cursor: Option<SessionCursor>,
    pub(crate) title_search: Option<String>,
    pub(crate) include_archived: bool,
}

impl ListSessionsQuery {
    pub fn parse(
        limit: Option<u16>,
        cursor: Option<SessionCursor>,
        title_search: Option<&str>,
        include_archived: bool,
    ) -> Result<Self, AgentSessionError> {
        let title_search = title_search
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        if title_search
            .as_ref()
            .is_some_and(|value| value.chars().count() > MAX_SESSION_TITLE_LENGTH)
        {
            return Err(AgentSessionError::invalid(
                "invalid_session_search",
                "Session title search cannot exceed 120 characters",
            ));
        }
        Ok(Self {
            limit: PageLimit::parse(limit)?,
            cursor,
            title_search,
            include_archived,
        })
    }
}

/// Refined Session create input with a canonical SHA-256 replay fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateSessionCommand {
    pub(crate) title: String,
    idempotency_key: IdempotencyKey,
    fingerprint: [u8; 32],
}

impl CreateSessionCommand {
    pub fn parse(
        title: Option<&str>,
        idempotency_key: impl Into<String>,
    ) -> Result<Self, AgentSessionError> {
        let title = parse_title(title.unwrap_or("New session"))?;
        let idempotency_key = IdempotencyKey::parse(idempotency_key)?;
        let fingerprint = canonical_fingerprint(&[("title", &title)]);
        Ok(Self {
            title,
            idempotency_key,
            fingerprint,
        })
    }

    pub(crate) fn idempotency_key(&self) -> &str {
        self.idempotency_key.as_str()
    }

    pub(crate) const fn fingerprint(&self) -> &[u8; 32] {
        &self.fingerprint
    }
}

/// Refined optimistic Session rename input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameSessionCommand {
    pub(crate) title: String,
    pub(crate) expected_version: i64,
}

impl RenameSessionCommand {
    pub fn parse(title: &str, expected_version: i64) -> Result<Self, AgentSessionError> {
        Ok(Self {
            title: parse_title(title)?,
            expected_version: positive_version(expected_version)?,
        })
    }
}

/// Refined optimistic archive input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveSessionCommand {
    pub(crate) expected_version: i64,
}

impl ArchiveSessionCommand {
    pub fn parse(expected_version: i64) -> Result<Self, AgentSessionError> {
        Ok(Self {
            expected_version: positive_version(expected_version)?,
        })
    }
}

/// Refined transcript list input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListMessagesQuery {
    pub(crate) limit: PageLimit,
    pub(crate) cursor: Option<MessageCursor>,
}

impl ListMessagesQuery {
    pub fn parse(
        limit: Option<u16>,
        cursor: Option<MessageCursor>,
    ) -> Result<Self, AgentSessionError> {
        Ok(Self {
            limit: PageLimit::parse(limit)?,
            cursor,
        })
    }
}

/// Refined user-message submission input with a canonical SHA-256 replay fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitMessageCommand {
    pub(crate) content: String,
    pub(crate) task_class: TaskClass,
    pub(crate) origin_module_key: String,
    pub(crate) origin_route: String,
    idempotency_key: IdempotencyKey,
    fingerprint: [u8; 32],
}

impl SubmitMessageCommand {
    pub fn parse(
        content: &str,
        task_class: TaskClass,
        origin_module_key: &str,
        origin_route: &str,
        idempotency_key: impl Into<String>,
    ) -> Result<Self, AgentSessionError> {
        let content = parse_bounded_text(
            content,
            MAX_MESSAGE_LENGTH,
            "invalid_message",
            "Message must contain between 1 and 20,000 characters",
        )?;
        let origin_module_key = stable_key(origin_module_key)?;
        let origin_route = parse_origin_route(origin_route)?;
        let idempotency_key = IdempotencyKey::parse(idempotency_key)?;
        let fingerprint = canonical_fingerprint(&[
            ("content", &content),
            ("task_class", task_class.as_str()),
            ("origin_module_key", &origin_module_key),
            ("origin_route", &origin_route),
        ]);
        Ok(Self {
            content,
            task_class,
            origin_module_key,
            origin_route,
            idempotency_key,
            fingerprint,
        })
    }

    pub(crate) fn idempotency_key(&self) -> &str {
        self.idempotency_key.as_str()
    }

    pub(crate) const fn fingerprint(&self) -> &[u8; 32] {
        &self.fingerprint
    }
}

/// Refined run list input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListRunsQuery {
    pub(crate) limit: PageLimit,
    pub(crate) cursor: Option<RunCursor>,
}

impl ListRunsQuery {
    pub fn parse(limit: Option<u16>, cursor: Option<RunCursor>) -> Result<Self, AgentSessionError> {
        Ok(Self {
            limit: PageLimit::parse(limit)?,
            cursor,
        })
    }
}

/// Refined event replay input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListEventsQuery {
    pub(crate) limit: PageLimit,
    pub(crate) after: EventCursor,
}

impl ListEventsQuery {
    pub fn parse(limit: Option<u16>, after: Option<&str>) -> Result<Self, AgentSessionError> {
        Ok(Self {
            limit: PageLimit::parse(limit)?,
            after: EventCursor::parse(after.unwrap_or("0"))?,
        })
    }
}

/// Bounded worker claim request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimRunsCommand {
    pub(crate) worker_id: String,
    pub(crate) batch_size: i64,
}

impl ClaimRunsCommand {
    pub fn parse(worker_id: &str, batch_size: u16) -> Result<Self, AgentSessionError> {
        let worker_id = parse_bounded_text(
            worker_id,
            MAX_WORKER_ID_LENGTH,
            "invalid_worker_id",
            "Worker ID must contain between 1 and 120 characters",
        )?;
        if batch_size == 0 || batch_size > MAX_CLAIM_BATCH {
            return Err(AgentSessionError::invalid(
                "invalid_claim_batch",
                "Worker claim batch must be between 1 and 25",
            ));
        }
        Ok(Self {
            worker_id,
            batch_size: i64::from(batch_size),
        })
    }
}

/// Non-serializable lease fence required for every claimed-run mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunLease {
    pub run_id: Uuid,
    pub worker_id: String,
    pub lease_token: Uuid,
    /// Monotonic queue-row fence. Every successful worker mutation returns the next value.
    pub fence_version: i64,
}

impl RunLease {
    pub fn parse(
        run_id: Uuid,
        worker_id: &str,
        lease_token: Uuid,
        fence_version: i64,
    ) -> Result<Self, AgentSessionError> {
        Ok(Self {
            run_id,
            worker_id: parse_bounded_text(
                worker_id,
                MAX_WORKER_ID_LENGTH,
                "invalid_worker_id",
                "Worker ID must contain between 1 and 120 characters",
            )?,
            lease_token,
            fence_version: positive_version(fence_version)?,
        })
    }
}

/// Successful heartbeat result carrying the next lease fence and cancellation signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseHeartbeat {
    pub lease: RunLease,
    pub cancel_requested: bool,
    pub lease_expires_at: DateTime<Utc>,
}

/// Claimed execution input. This type is deliberately not serializable to an API response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedRun {
    /// Owning campus. Global workers must pass this exact tenant to every fenced mutation.
    pub tenant_id: Uuid,
    pub lease: RunLease,
    pub session_id: Uuid,
    pub requested_by: Uuid,
    pub request_message_id: Uuid,
    pub request_message: String,
    pub task_class: TaskClass,
    pub origin_module_key: String,
    pub origin_route: String,
    pub correlation_id: Uuid,
    pub delivery_attempt: i16,
    pub checkpoint: RunCheckpoint,
    pub lease_expires_at: DateTime<Utc>,
}

/// Safe worker failure. Provider bodies and internal stack details cannot be supplied here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeRunFailure {
    pub(crate) code: String,
    pub(crate) message: String,
}

impl SafeRunFailure {
    pub fn parse(code: &str, message: &str) -> Result<Self, AgentSessionError> {
        Ok(Self {
            code: parse_stable_failure_code(code)?,
            message: parse_bounded_text(
                message,
                MAX_SAFE_FAILURE_MESSAGE_LENGTH,
                "invalid_safe_failure_message",
                "Safe failure message must contain between 1 and 500 characters",
            )?,
        })
    }
}

/// Recovery totals allow a worker supervisor to observe every expired-lease decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RecoverySummary {
    pub requeued: u64,
    pub interrupted: u64,
    pub cancelled: u64,
}

/// Durable outcome selected for one expired queue lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpiredLeaseRecoveryDisposition {
    Requeued,
    Interrupted,
    Cancelled,
}

/// One tenant-owned run processed by global expired-lease recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveredRun {
    pub tenant_id: Uuid,
    pub run_id: Uuid,
    pub disposition: ExpiredLeaseRecoveryDisposition,
}

/// Child execution stage whose reserved usage must be reconciled idempotently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryUsageStage {
    ProviderAttempt { attempt_id: Uuid },
    CapabilityCall { call_id: Uuid },
}

/// Idempotent usage transition required after recovery terminalizes a child stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryUsageAction {
    /// The child never consumed its reservation; expire it to return held capacity.
    ExpireUnclaimed,
    /// The child consumed its reservation; commit its now-terminal reduced usage evidence.
    CommitTerminal,
}

/// Replayable usage-reconciliation work exposed without direct worker SQL.
///
/// For `ExpireUnclaimed`, pass `tenant_id` and `reservation_id` to
/// `AgentUsageRuntime::release_or_expire` with the `Expire` action. For `CommitTerminal`, pass them
/// to `AgentUsageRuntime::commit_terminal_usage`. The reservation remains visible here until the
/// selected idempotent transition succeeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryUsageReservation {
    pub tenant_id: Uuid,
    pub run_id: Uuid,
    pub reservation_id: Uuid,
    pub stage: RecoveryUsageStage,
    pub action: RecoveryUsageAction,
}

/// Tenant-fair global recovery results plus replayable usage reconciliation work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalRecoveryBatch {
    pub summary: RecoverySummary,
    pub runs: Vec<RecoveredRun>,
    pub pending_usage_reservations: Vec<RecoveryUsageReservation>,
}

/// Stable Session/runtime errors mapped to HTTP or worker behavior by owning callers.
#[derive(Debug, Error)]
pub enum AgentSessionError {
    #[error("{message}")]
    InvalidInput { code: &'static str, message: String },
    #[error("Agent Session was not found")]
    SessionNotFound,
    #[error("Agent run was not found")]
    RunNotFound,
    #[error("{message}")]
    Conflict { code: &'static str, message: String },
    #[error("Agent worker lease is no longer valid")]
    LeaseLost,
    #[error("Agent Session persistence failed")]
    Storage(#[source] sqlx::Error),
}

impl AgentSessionError {
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

    pub(crate) fn storage_contract() -> Self {
        Self::conflict(
            "agent_runtime_storage_error",
            "Agent runtime data is not in a supported state",
        )
    }

    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { code, .. } | Self::Conflict { code, .. } => code,
            Self::SessionNotFound => "session_not_found",
            Self::RunNotFound => "run_not_found",
            Self::LeaseLost => "run_lease_lost",
            Self::Storage(_) => "agent_runtime_storage_error",
        }
    }

    #[must_use]
    pub fn safe_message(&self) -> String {
        match self {
            Self::InvalidInput { message, .. } | Self::Conflict { message, .. } => message.clone(),
            Self::SessionNotFound => "This Agent Session does not exist".to_owned(),
            Self::RunNotFound => "This Agent run does not exist".to_owned(),
            Self::LeaseLost => "This Agent run is no longer leased to this worker".to_owned(),
            Self::Storage(_) => "Agent Session history could not be loaded or saved".to_owned(),
        }
    }
}

impl From<sqlx::Error> for AgentSessionError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error)
    }
}

fn parse_title(value: &str) -> Result<String, AgentSessionError> {
    parse_bounded_text(
        value,
        MAX_SESSION_TITLE_LENGTH,
        "invalid_session_title",
        "Session title must contain between 1 and 120 characters",
    )
}

fn parse_assistant_message(value: &str) -> Result<String, AgentSessionError> {
    parse_bounded_text(
        value,
        MAX_MESSAGE_LENGTH,
        "invalid_assistant_message",
        "Assistant message must contain between 1 and 20,000 characters",
    )
}

fn parse_bounded_text(
    value: &str,
    maximum_length: usize,
    code: &'static str,
    message: &'static str,
) -> Result<String, AgentSessionError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > maximum_length {
        Err(AgentSessionError::invalid(code, message))
    } else {
        Ok(value.to_owned())
    }
}

fn stable_key(value: &str) -> Result<String, AgentSessionError> {
    let value = value.trim();
    let valid = !value.is_empty()
        && value.len() <= MAX_MODULE_KEY_LENGTH
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
        Err(AgentSessionError::invalid(
            "invalid_origin_module_key",
            "Origin module must be a stable lowercase key",
        ))
    }
}

fn parse_origin_route(value: &str) -> Result<String, AgentSessionError> {
    let value = parse_bounded_text(
        value,
        MAX_ORIGIN_ROUTE_LENGTH,
        "invalid_origin_route",
        "Origin route must contain between 1 and 500 characters",
    )?;
    if !value.starts_with('/') || value.chars().any(char::is_control) {
        return Err(AgentSessionError::invalid(
            "invalid_origin_route",
            "Origin route must be an application-relative path",
        ));
    }
    Ok(value)
}

fn parse_stable_failure_code(value: &str) -> Result<String, AgentSessionError> {
    let value = value.trim();
    let valid = !value.is_empty()
        && value.len() <= MAX_SAFE_FAILURE_CODE_LENGTH
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'.' | b'-')
        });
    if valid {
        Ok(value.to_owned())
    } else {
        Err(AgentSessionError::invalid(
            "invalid_safe_failure_code",
            "Safe failure code must use lowercase letters, numbers, or underscores",
        ))
    }
}

fn parse_stable_key(
    value: &str,
    maximum_length: usize,
    code: &'static str,
) -> Result<String, AgentSessionError> {
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
        Err(AgentSessionError::invalid(
            code,
            "Use a stable lowercase key containing letters, numbers, dots, hyphens, or underscores",
        ))
    }
}

fn parse_permission_key(value: &str) -> Result<String, AgentSessionError> {
    let value = value.trim();
    let valid = value.len() >= 3
        && value.len() <= MAX_PERMISSION_KEY_LENGTH
        && value.matches(':').count() == 1
        && value.split(':').all(|part| {
            part.bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_lowercase())
                && part.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'_' | b'-' | b'.')
                })
        });
    if valid {
        Ok(value.to_owned())
    } else {
        Err(AgentSessionError::invalid(
            "invalid_required_permission",
            "Required permission must contain one stable module and action separator",
        ))
    }
}

fn invalid_provider_turn() -> AgentSessionError {
    AgentSessionError::invalid(
        "invalid_provider_turn_index",
        "Provider turn index must be between 1 and 16",
    )
}

fn invalid_capability_sequence() -> AgentSessionError {
    AgentSessionError::invalid(
        "invalid_capability_call_sequence",
        "Capability call sequence must be between 1 and 16",
    )
}

fn js_safe_counter(value: u64, code: &'static str) -> Result<i64, AgentSessionError> {
    let value = i64::try_from(value)
        .ok()
        .filter(|value| *value <= MAX_JS_SAFE_INTEGER)
        .ok_or_else(|| {
            AgentSessionError::invalid(code, "Numeric value exceeds the supported safe range")
        })?;
    Ok(value)
}

fn optional_js_safe_counter(
    value: Option<u64>,
    code: &'static str,
) -> Result<Option<i64>, AgentSessionError> {
    value.map(|value| js_safe_counter(value, code)).transpose()
}

fn positive_version(value: i64) -> Result<i64, AgentSessionError> {
    if (1..=MAX_JS_SAFE_INTEGER).contains(&value) {
        Ok(value)
    } else {
        Err(AgentSessionError::invalid(
            "invalid_expected_version",
            "Expected version must be positive",
        ))
    }
}

fn canonical_fingerprint(fields: &[(&str, &str)]) -> [u8; 32] {
    let mut digest = Sha256::new();
    for (name, value) in fields {
        digest.update((name.len() as u64).to_be_bytes());
        digest.update(name.as_bytes());
        digest.update((value.len() as u64).to_be_bytes());
        digest.update(value.as_bytes());
    }
    digest.finalize().into()
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use cp_common::{ProviderDataClass, ProviderExecutionEnvironmentClass};
    use uuid::Uuid;

    use super::{
        AgentProviderKey, AgentSessionError, ArchiveSessionCommand, CapabilityCallDuration,
        CapabilityCallFailure, CapabilityCallPlan, CapabilityCallScope, CapabilityCallStatus,
        CapabilityFailureStatus, CapabilityResourceReference, ClaimRunsCommand,
        CreateSessionCommand, EncryptedExecutionArtifact, EventCursor, ExecutionArtifactKind,
        ExecutionStepStatus, FinalResponsePlaintext, ListEventsQuery, ListMessagesQuery,
        ListRunsQuery, ListSessionsQuery, MessageCursor, MessageRole, NormalizedCost,
        NormalizedProviderUsage, PageLimit, ProviderAttemptFailure, ProviderAttemptIndex,
        ProviderAttemptPlan, ProviderAttemptStatus, ProviderPreflightFailure, ProviderTurnIndex,
        ProviderUpstreamFailure, RenameSessionCommand, RunCheckpoint, RunEventType, RunLease,
        RunStatus, SafeRunFailure, SessionStatus, SubmitMessageCommand,
    };
    use crate::TaskClass;

    macro_rules! assert_not_impl {
        ($type:ty: $trait:path) => {
            const _: fn() = || {
                struct Invalid;
                trait AmbiguousIfImpl<A> {
                    fn marker() {}
                }
                impl<T: ?Sized> AmbiguousIfImpl<()> for T {}
                impl<T: ?Sized + $trait> AmbiguousIfImpl<Invalid> for T {}
                let _ = <$type as AmbiguousIfImpl<_>>::marker;
            };
        };
    }

    assert_not_impl!(super::EncryptedExecutionArtifact: std::fmt::Debug);
    assert_not_impl!(super::EncryptedExecutionArtifact: serde::Serialize);
    assert_not_impl!(super::LoadedExecutionArtifact: std::fmt::Debug);
    assert_not_impl!(super::LoadedExecutionArtifact: serde::Serialize);
    assert_not_impl!(super::ExecutionSnapshot: std::fmt::Debug);
    assert_not_impl!(super::ExecutionSnapshot: serde::Serialize);

    #[test]
    fn bounded_commands_normalize_and_reject_invalid_input() {
        assert!(PageLimit::parse(None).is_ok());
        assert!(PageLimit::parse(Some(0)).is_err());
        assert!(PageLimit::parse(Some(101)).is_err());
        assert!(ListSessionsQuery::parse(None, None, Some("  "), false).is_ok());
        assert!(ListSessionsQuery::parse(None, None, Some(&"x".repeat(121)), false).is_err());
        assert!(ListMessagesQuery::parse(Some(1), None).is_ok());
        assert!(ListRunsQuery::parse(Some(100), None).is_ok());
        assert!(ListEventsQuery::parse(None, Some("-1")).is_err());
        assert!(ListEventsQuery::parse(None, Some("not-a-cursor")).is_err());
        assert!(EventCursor::parse("0").is_ok());
        assert!(MessageCursor::parse(0, Uuid::new_v4()).is_err());

        let create = CreateSessionCommand::parse(Some("  Work  "), "request-1").unwrap();
        assert_eq!(create.title, "Work");
        assert_eq!(create.idempotency_key(), "request-1");
        assert!(CreateSessionCommand::parse(Some(" "), "request-1").is_err());
        assert!(CreateSessionCommand::parse(None, " ").is_err());
        assert!(RenameSessionCommand::parse("Updated", 1).is_ok());
        assert!(RenameSessionCommand::parse("Updated", 0).is_err());
        assert!(ArchiveSessionCommand::parse(-1).is_err());
    }

    #[test]
    fn submit_fingerprint_is_canonical_and_sensitive_to_every_execution_field() {
        let first = SubmitMessageCommand::parse(
            "  Show learners ",
            TaskClass::ModuleReadReporting,
            "sis",
            "/modules/sis",
            "submission-1",
        )
        .unwrap();
        let same = SubmitMessageCommand::parse(
            "Show learners",
            TaskClass::ModuleReadReporting,
            "sis",
            "/modules/sis",
            "submission-1",
        )
        .unwrap();
        let changed = SubmitMessageCommand::parse(
            "Show learners",
            TaskClass::CampusConversationSearch,
            "sis",
            "/modules/sis",
            "submission-1",
        )
        .unwrap();
        assert_eq!(first.fingerprint(), same.fingerprint());
        assert_ne!(first.fingerprint(), changed.fingerprint());
        assert!(
            SubmitMessageCommand::parse(
                " ",
                TaskClass::ModuleReadReporting,
                "sis",
                "/modules/sis",
                "key"
            )
            .is_err()
        );
        assert!(
            SubmitMessageCommand::parse(
                "message",
                TaskClass::ModuleReadReporting,
                "SIS",
                "/modules/sis",
                "key"
            )
            .is_err()
        );
        assert!(
            SubmitMessageCommand::parse(
                "message",
                TaskClass::ModuleReadReporting,
                "sis",
                "https://example.test",
                "key"
            )
            .is_err()
        );
    }

    #[test]
    fn worker_inputs_and_failure_details_are_bounded() {
        assert!(ClaimRunsCommand::parse("worker-a", 1).is_ok());
        assert!(ClaimRunsCommand::parse("worker-a", 0).is_err());
        assert!(ClaimRunsCommand::parse("worker-a", 26).is_err());
        assert!(RunLease::parse(Uuid::new_v4(), "worker-a", Uuid::new_v4(), 2).is_ok());
        assert!(RunLease::parse(Uuid::new_v4(), " ", Uuid::new_v4(), 2).is_err());
        assert!(RunLease::parse(Uuid::new_v4(), "worker-a", Uuid::new_v4(), 0).is_err());
        assert!(SafeRunFailure::parse("provider_unavailable", "Try again later").is_ok());
        assert!(SafeRunFailure::parse("Provider Failed", "safe").is_err());
        assert!(SafeRunFailure::parse("provider_failed", " ").is_err());
    }

    #[test]
    fn execution_inputs_are_typed_bounded_and_secret_safe() {
        assert!(MessageCursor::parse(1, Uuid::new_v4()).is_ok());
        for status in [
            RunStatus::Queued,
            RunStatus::Running,
            RunStatus::AwaitingApproval,
            RunStatus::Completed,
            RunStatus::Failed,
            RunStatus::Cancelled,
            RunStatus::Interrupted,
        ] {
            assert!(!status.as_str().is_empty());
        }
        assert!(!RunStatus::Running.is_terminal());
        assert!(RunStatus::Completed.is_terminal());
        assert_eq!(RunCheckpoint::Queued.as_str(), "queued");
        assert!(ProviderTurnIndex::parse(1).is_ok());
        assert!(ProviderTurnIndex::parse(0).is_err());
        assert!(ProviderTurnIndex::parse(17).is_err());
        assert!(ProviderAttemptIndex::parse(3).is_ok());
        assert!(ProviderAttemptIndex::parse(0).is_err());
        assert!(ProviderAttemptIndex::parse(4).is_err());
        let provider_plan = ProviderAttemptPlan::parse(
            1,
            1,
            Uuid::new_v4(),
            1,
            Uuid::new_v4(),
            Uuid::new_v4(),
            1,
            Uuid::new_v4(),
            Uuid::new_v4(),
            ProviderDataClass::SensitiveDataApproved,
            ProviderExecutionEnvironmentClass::ExternalManaged,
            AgentProviderKey::OpenAi,
            "gpt-test",
            [1; 32],
        )
        .unwrap();
        assert_eq!(provider_plan.provider_key.as_str(), "openai");
        assert_eq!(AgentProviderKey::Anthropic.as_str(), "anthropic");
        assert_eq!(AgentProviderKey::OpenRouter.as_str(), "openrouter");
        assert_eq!(
            AgentProviderKey::from_stored("anthropic").unwrap(),
            AgentProviderKey::Anthropic
        );
        assert_eq!(
            AgentProviderKey::from_stored("openrouter").unwrap(),
            AgentProviderKey::OpenRouter
        );
        assert!(AgentProviderKey::from_stored("raw-provider").is_err());
        assert!(
            ProviderAttemptPlan::parse(
                1,
                1,
                Uuid::new_v4(),
                0,
                Uuid::new_v4(),
                Uuid::new_v4(),
                1,
                Uuid::new_v4(),
                Uuid::new_v4(),
                ProviderDataClass::SensitiveDataApproved,
                ProviderExecutionEnvironmentClass::ExternalManaged,
                AgentProviderKey::OpenAi,
                "gpt-test",
                [1; 32],
            )
            .is_err()
        );
        assert_eq!(
            ProviderAttemptFailure::Preflight(ProviderPreflightFailure::StaleCredential).origin(),
            "preflight"
        );
        assert_eq!(
            ProviderAttemptFailure::Upstream(ProviderUpstreamFailure::RateLimited).category(),
            "rate_limited"
        );
        for failure in [
            ProviderPreflightFailure::ConnectionUnavailable,
            ProviderPreflightFailure::StaleCredential,
            ProviderPreflightFailure::StaleModel,
            ProviderPreflightFailure::ToolsUnsupported,
            ProviderPreflightFailure::ModelContextUnavailable,
            ProviderPreflightFailure::ModelOutputUnavailable,
            ProviderPreflightFailure::ContextWindowExceeded,
            ProviderPreflightFailure::OutputBudgetExceeded,
            ProviderPreflightFailure::CredentialUnavailable,
            ProviderPreflightFailure::InvalidConfiguration,
            ProviderPreflightFailure::InvalidInput,
            ProviderPreflightFailure::StorageError,
        ] {
            assert!(!failure.as_str().is_empty());
            assert_eq!(
                ProviderPreflightFailure::from_stored(failure.as_str()).unwrap(),
                failure
            );
        }
        assert!(ProviderPreflightFailure::from_stored("raw_failure").is_err());
        for failure in [
            ProviderUpstreamFailure::Authentication,
            ProviderUpstreamFailure::RateLimited,
            ProviderUpstreamFailure::Unavailable,
            ProviderUpstreamFailure::Timeout,
            ProviderUpstreamFailure::Network,
            ProviderUpstreamFailure::InvalidResponse,
            ProviderUpstreamFailure::Unsupported,
        ] {
            assert!(!failure.as_str().is_empty());
            assert_eq!(
                ProviderUpstreamFailure::from_stored(failure.as_str()).unwrap(),
                failure
            );
        }
        assert!(ProviderUpstreamFailure::from_stored("raw_failure").is_err());
        assert_eq!(
            ProviderAttemptFailure::from_stored("preflight", "invalid_input").unwrap(),
            ProviderAttemptFailure::Preflight(ProviderPreflightFailure::InvalidInput)
        );
        assert_eq!(
            ProviderAttemptFailure::from_stored("upstream", "network").unwrap(),
            ProviderAttemptFailure::Upstream(ProviderUpstreamFailure::Network)
        );
        assert!(ProviderAttemptFailure::from_stored("worker", "network").is_err());

        let reported = NormalizedCost::provider_reported(10, "USD", 2, None).unwrap();
        let estimated = NormalizedCost::estimated(11, "ZWL", 2, "catalog-1").unwrap();
        assert!(NormalizedCost::provider_reported(1, "usd", 2, None).is_err());
        assert!(NormalizedCost::provider_reported(1, "USD", 10, None).is_err());
        assert!(NormalizedCost::estimated(1, "USD", 2, " ").is_err());
        assert!(
            NormalizedProviderUsage::parse(
                Some(1),
                Some(2),
                None,
                None,
                Some(reported),
                Some(estimated),
            )
            .is_ok()
        );
        assert!(
            NormalizedProviderUsage::parse(
                Some(9_007_199_254_740_992),
                None,
                None,
                None,
                None,
                None,
            )
            .is_err()
        );

        let reference = CapabilityResourceReference::parse("learner", "learner-1").unwrap();
        assert!(CapabilityCallScope::resources(Vec::new()).is_err());
        let scope = CapabilityCallScope::resources(vec![reference]).unwrap();
        assert_eq!(scope.kind(), "resources");
        assert!(super::CapabilityCallSequence::parse(0).is_err());
        assert!(super::CapabilityCallSequence::parse(17).is_err());
        assert!(
            CapabilityCallPlan::parse(
                Uuid::new_v4(),
                1,
                1,
                "sis.learners.read",
                1,
                "sis.learners.read",
                "sis",
                "sis:view",
                [2; 32],
                scope,
            )
            .is_ok()
        );
        assert!(
            CapabilityCallPlan::parse(
                Uuid::new_v4(),
                1,
                1,
                "sis.learners.read",
                1,
                "sis.learners.read",
                "sis",
                "invalid",
                [2; 32],
                CapabilityCallScope::TenantWide,
            )
            .is_err()
        );
        assert!(CapabilityCallDuration::parse(25).is_ok());
        assert!(
            CapabilityCallPlan::parse(
                Uuid::new_v4(),
                1,
                1,
                "sis.learners.read",
                0,
                "sis.learners.read",
                "sis",
                "sis:view",
                [2; 32],
                CapabilityCallScope::TenantWide,
            )
            .is_err()
        );
        assert!(
            CapabilityCallFailure::parse(CapabilityFailureStatus::Denied, "policy_denied", 25,)
                .is_ok()
        );
        assert_eq!(CapabilityFailureStatus::Failed.as_str(), "failed");

        let artifact = EncryptedExecutionArtifact::parse(
            vec![1; 32],
            [3; 32],
            vec![2; 12],
            "artifact-key",
            1,
            16,
        )
        .unwrap();
        assert_eq!(artifact.ciphertext().len(), 32);
        assert_eq!(artifact.nonce().len(), 12);
        assert_eq!(artifact.encryption_key_id(), "artifact-key");
        assert_eq!(artifact.encryption_key_version(), 1);
        assert_eq!(artifact.plaintext_sha256(), &[3; 32]);
        assert_eq!(artifact.plaintext_length(), 16);
        assert!(
            EncryptedExecutionArtifact::parse(Vec::new(), [0; 32], vec![1; 12], "key", 1, 1,)
                .is_err()
        );
        assert!(
            EncryptedExecutionArtifact::parse(vec![1; 65_553], [0; 32], vec![1; 12], "key", 1, 1,)
                .is_err()
        );
        assert!(
            EncryptedExecutionArtifact::parse(vec![1], [0; 32], vec![1; 11], "key", 1, 1,).is_err()
        );
        assert!(
            EncryptedExecutionArtifact::parse(vec![1], [0; 32], vec![1; 12], "key", 1, 0,).is_err()
        );
        assert!(
            EncryptedExecutionArtifact::parse(vec![1], [0; 32], vec![1; 12], "key", 1, 65_537,)
                .is_err()
        );
        assert!(
            EncryptedExecutionArtifact::parse(vec![1], [0; 32], vec![1; 12], "bad\nkey", 1, 1,)
                .is_err()
        );
        assert!(
            EncryptedExecutionArtifact::parse(vec![1], [0; 32], vec![1; 12], "key", 0, 1,).is_err()
        );
        assert!(ExecutionArtifactKind::from_str("provider_result").is_ok());
        assert!(ExecutionArtifactKind::from_str("raw_body").is_err());
        for status in ["running", "succeeded", "failed", "cancelled", "interrupted"] {
            assert!(ExecutionStepStatus::from_str(status).is_ok());
            assert!(ProviderAttemptStatus::from_str(status).is_ok());
        }
        assert!(ExecutionStepStatus::from_str("denied").is_err());
        for status in [
            "running",
            "succeeded",
            "failed",
            "denied",
            "cancelled",
            "interrupted",
        ] {
            assert!(CapabilityCallStatus::from_str(status).is_ok());
        }
        assert!(CapabilityCallStatus::from_str("raw").is_err());
        assert!(FinalResponsePlaintext::parse("  Done  ".to_owned()).is_ok());
        assert!(FinalResponsePlaintext::parse(" ".to_owned()).is_err());
    }

    #[test]
    fn stored_enums_are_exhaustive_and_recovery_is_fail_closed() {
        assert_eq!(
            SessionStatus::from_str("active").unwrap(),
            SessionStatus::Active
        );
        assert_eq!(
            MessageRole::from_str("assistant").unwrap(),
            MessageRole::Assistant
        );
        assert_eq!(
            RunStatus::from_str("completed").unwrap(),
            RunStatus::Completed
        );
        assert!(RunStatus::Completed.is_terminal());
        assert!(!RunStatus::Running.is_terminal());
        assert_eq!(RunEventType::from_str("queued").unwrap().as_str(), "queued");
        assert!(SessionStatus::from_str("deleted").is_err());
        assert!(MessageRole::from_str("system").is_err());
        assert!(RunStatus::from_str("unknown").is_err());
        assert!(RunEventType::from_str("raw_provider_error").is_err());

        for checkpoint in [RunCheckpoint::Queued, RunCheckpoint::BeforeProvider] {
            assert!(checkpoint.is_automatically_recoverable());
        }
        for checkpoint in [
            RunCheckpoint::ProviderInFlight,
            RunCheckpoint::ProviderResultPersisted,
            RunCheckpoint::CapabilityInFlight,
            RunCheckpoint::CapabilityResultPersisted,
            RunCheckpoint::Finalizing,
        ] {
            assert!(!checkpoint.is_automatically_recoverable());
        }
        assert!(RunCheckpoint::Queued.can_advance_to(RunCheckpoint::BeforeProvider));
        assert!(!RunCheckpoint::Queued.can_advance_to(RunCheckpoint::ProviderResultPersisted));
        assert!(RunCheckpoint::from_str("unknown").is_err());
    }

    #[test]
    fn errors_have_stable_non_sensitive_contracts() {
        let invalid = AgentSessionError::invalid("invalid", "Fix the request");
        assert_eq!(invalid.code(), "invalid");
        assert_eq!(invalid.safe_message(), "Fix the request");
        assert_eq!(
            AgentSessionError::SessionNotFound.code(),
            "session_not_found"
        );
        assert_eq!(AgentSessionError::RunNotFound.code(), "run_not_found");
        assert_eq!(AgentSessionError::LeaseLost.code(), "run_lease_lost");
        let conflict = AgentSessionError::conflict("active_run_exists", "Wait for this run");
        assert_eq!(conflict.code(), "active_run_exists");
    }

    #[test]
    fn event_cursor_serializes_as_an_opaque_decimal_string() {
        let cursor = EventCursor::parse("9007199254740992").unwrap();
        assert_eq!(
            serde_json::to_value(cursor).unwrap(),
            serde_json::json!("9007199254740992")
        );
    }
}
