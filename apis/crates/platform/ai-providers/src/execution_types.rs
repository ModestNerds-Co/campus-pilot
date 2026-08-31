//! Defines bounded, provider-neutral Agent execution contracts.
//!
//! Model input is deliberately non-serializable and non-debuggable. Construction
//! proves history, tool schemas, tool results, and output budgets fit hard limits.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use cp_common::ProviderDataClass;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::types::ProviderFailureCategory;

const MAX_SYSTEM_PROMPT_BYTES: usize = 32 * 1024;
const MAX_MESSAGE_BYTES: usize = 64 * 1024;
const MAX_HISTORY_MESSAGES: usize = 64;
const MAX_HISTORY_BYTES: usize = 256 * 1024;
const MAX_TOOLS: usize = 32;
const MAX_TOOL_NAME_BYTES: usize = 64;
const MAX_TOOL_DESCRIPTION_BYTES: usize = 4 * 1024;
const MAX_TOOL_SCHEMA_BYTES: usize = 32 * 1024;
const MAX_TOOL_CALL_ID_BYTES: usize = 128;
const MAX_TOOL_ARGUMENT_BYTES: usize = 32 * 1024;
const MAX_JSON_DEPTH: usize = 16;
const MAX_OUTPUT_TOKENS: u32 = 32_768;

const SUPPORTED_TASK_CLASSES: [&str; 6] = [
    "campus_conversation",
    "campus_conversation_search",
    "module_read_reporting",
    "document_extraction",
    "drafting_proposal",
    "approved_operational_action",
];

/// Exact route target the worker asks this crate to revalidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderExecutionTarget {
    pub(crate) route_set_id: Uuid,
    pub(crate) route_version: i64,
    pub(crate) route_target_id: Uuid,
    pub(crate) connection_id: Uuid,
    pub(crate) expected_credential_version: i64,
    pub(crate) model_snapshot_id: Uuid,
    pub(crate) provider_data_approval_id: Uuid,
    pub(crate) requires_tools: bool,
}

impl ProviderExecutionTarget {
    /// Proves route and credential versions are current-style positive versions.
    #[allow(clippy::too_many_arguments)]
    pub fn parse(
        route_set_id: Uuid,
        route_version: i64,
        route_target_id: Uuid,
        connection_id: Uuid,
        expected_credential_version: i64,
        model_snapshot_id: Uuid,
        provider_data_approval_id: Uuid,
        requires_tools: bool,
    ) -> Result<Self, ProviderExecutionError> {
        if route_version <= 0 {
            return Err(ProviderExecutionError::invalid(
                "invalid_route_version",
                "Route version must be a positive integer",
            ));
        }
        if expected_credential_version <= 0 {
            return Err(ProviderExecutionError::invalid(
                "invalid_credential_version",
                "Credential version must be a positive integer",
            ));
        }
        Ok(Self {
            route_set_id,
            route_version,
            route_target_id,
            connection_id,
            expected_credential_version,
            model_snapshot_id,
            provider_data_approval_id,
            requires_tools,
        })
    }

    #[must_use]
    pub const fn route_set_id(self) -> Uuid {
        self.route_set_id
    }

    #[must_use]
    pub const fn route_version(self) -> i64 {
        self.route_version
    }

    #[must_use]
    pub const fn route_target_id(self) -> Uuid {
        self.route_target_id
    }

    #[must_use]
    pub const fn connection_id(self) -> Uuid {
        self.connection_id
    }

    #[must_use]
    pub const fn expected_credential_version(self) -> i64 {
        self.expected_credential_version
    }

    #[must_use]
    pub const fn model_snapshot_id(self) -> Uuid {
        self.model_snapshot_id
    }

    #[must_use]
    pub const fn provider_data_approval_id(self) -> Uuid {
        self.provider_data_approval_id
    }

    #[must_use]
    pub const fn requires_tools(self) -> bool {
        self.requires_tools
    }
}

/// One normalized function tool offered to a provider.
///
/// Fields remain private so name and schema bounds cannot be bypassed.
pub struct ProviderToolDefinition {
    name: String,
    description: String,
    input_schema: Value,
}

impl ProviderToolDefinition {
    pub fn parse(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
    ) -> Result<Self, ProviderExecutionError> {
        let name = parse_tool_name(name.into())?;
        let description = parse_text(
            description.into(),
            1,
            MAX_TOOL_DESCRIPTION_BYTES,
            "invalid_tool_description",
            "Tool descriptions must contain between 1 and 4096 bytes",
        )?;
        validate_json_object(
            &input_schema,
            MAX_TOOL_SCHEMA_BYTES,
            "invalid_tool_schema",
            "Tool input schemas must be bounded JSON objects",
        )?;
        Ok(Self {
            name,
            description,
            input_schema,
        })
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn description(&self) -> &str {
        &self.description
    }

    pub(crate) fn input_schema(&self) -> &Value {
        &self.input_schema
    }
}

/// One normalized model-requested tool call.
#[derive(Clone, PartialEq)]
pub struct ProviderToolCall {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) arguments: Value,
}

impl ProviderToolCall {
    pub fn parse(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: Value,
    ) -> Result<Self, ProviderExecutionError> {
        let id = parse_tool_call_id(id.into())?;
        let name = parse_tool_name(name.into())?;
        validate_json_object(
            &arguments,
            MAX_TOOL_ARGUMENT_BYTES,
            "invalid_tool_arguments",
            "Tool arguments must be bounded JSON objects",
        )?;
        Ok(Self {
            id,
            name,
            arguments,
        })
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn arguments(&self) -> &Value {
        &self.arguments
    }

    #[must_use]
    pub fn into_parts(self) -> (String, String, Value) {
        (self.id, self.name, self.arguments)
    }
}

impl fmt::Debug for ProviderToolCall {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderToolCall")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("arguments", &"[REDACTED]")
            .finish()
    }
}

/// One bounded provider-neutral history message.
///
/// The inner representation is private and the type intentionally implements
/// neither `Debug` nor `Serialize` because content can contain campus data.
pub struct ProviderMessage(MessageKind);

enum MessageKind {
    User(String),
    Assistant {
        text: Option<String>,
        tool_calls: Vec<ProviderToolCall>,
    },
    ToolResult {
        tool_call_id: String,
        name: String,
        content: String,
        is_error: bool,
    },
}

impl ProviderMessage {
    pub fn user(content: impl Into<String>) -> Result<Self, ProviderExecutionError> {
        Ok(Self(MessageKind::User(parse_message_text(content.into())?)))
    }

    pub fn assistant(
        text: Option<String>,
        tool_calls: Vec<ProviderToolCall>,
    ) -> Result<Self, ProviderExecutionError> {
        let text = text
            .map(parse_message_text)
            .transpose()?
            .filter(|value| !value.is_empty());
        if text.is_none() && tool_calls.is_empty() {
            return Err(ProviderExecutionError::invalid(
                "invalid_assistant_message",
                "Assistant history must contain text or tool calls",
            ));
        }
        validate_tool_call_set(&tool_calls)?;
        let message = Self(MessageKind::Assistant { text, tool_calls });
        if message.byte_len() > MAX_MESSAGE_BYTES {
            return Err(ProviderExecutionError::invalid(
                "assistant_message_too_large",
                "Assistant text and tool calls exceed the 65536-byte message limit",
            ));
        }
        Ok(message)
    }

    pub fn tool_result(
        tool_call_id: impl Into<String>,
        name: impl Into<String>,
        content: impl Into<String>,
        is_error: bool,
    ) -> Result<Self, ProviderExecutionError> {
        let message = Self(MessageKind::ToolResult {
            tool_call_id: parse_tool_call_id(tool_call_id.into())?,
            name: parse_tool_name(name.into())?,
            content: parse_message_text(content.into())?,
            is_error,
        });
        if message.byte_len() > MAX_MESSAGE_BYTES {
            return Err(ProviderExecutionError::invalid(
                "tool_result_message_too_large",
                "Tool result ID, name, and content exceed the 65536-byte message limit",
            ));
        }
        Ok(message)
    }

    fn byte_len(&self) -> usize {
        match &self.0 {
            MessageKind::User(content) => content.len(),
            MessageKind::Assistant { text, tool_calls } => {
                text.as_deref().map_or(0, str::len)
                    + tool_calls
                        .iter()
                        .map(|call| {
                            call.id.len()
                                + call.name.len()
                                + serde_json::to_vec(&call.arguments)
                                    .map_or(usize::MAX, |v| v.len())
                        })
                        .sum::<usize>()
            }
            MessageKind::ToolResult {
                tool_call_id,
                name,
                content,
                ..
            } => tool_call_id.len() + name.len() + content.len(),
        }
    }

    fn encoded_input_upper_bound(&self) -> usize {
        match &self.0 {
            MessageKind::User(content) => encoded_string_len(content).saturating_add(128),
            MessageKind::Assistant { text, tool_calls } => text
                .as_deref()
                .map_or(0, encoded_string_len)
                .saturating_add(
                    tool_calls
                        .iter()
                        .map(|call| {
                            encoded_string_len(&call.id)
                                .saturating_add(encoded_string_len(&call.name))
                                .saturating_add(
                                    serde_json::to_vec(&call.arguments)
                                        .map_or(usize::MAX, |value| value.len()),
                                )
                                .saturating_add(192)
                        })
                        .sum::<usize>(),
                )
                .saturating_add(128),
            MessageKind::ToolResult {
                tool_call_id,
                content,
                ..
            } => encoded_string_len(tool_call_id)
                .saturating_add(encoded_string_len(content))
                .saturating_add(160),
        }
    }
}

/// Bounded provider execution command created by the durable worker.
///
/// This type intentionally implements neither `Debug` nor `Serialize`.
pub struct ExecuteProviderCommand {
    target: ProviderExecutionTarget,
    task_class: String,
    provider_model_id: String,
    system_prompt: String,
    messages: Vec<ProviderMessage>,
    tools: Vec<ProviderToolDefinition>,
    required_provider_data_class: ProviderDataClass,
    max_output_tokens: u32,
    history_bytes: usize,
    conservative_input_token_upper_bound: u64,
}

impl ExecuteProviderCommand {
    pub fn parse(
        target: ProviderExecutionTarget,
        task_class: impl Into<String>,
        provider_model_id: impl Into<String>,
        system_prompt: impl Into<String>,
        messages: Vec<ProviderMessage>,
        tools: Vec<ProviderToolDefinition>,
        max_output_tokens: u32,
    ) -> Result<Self, ProviderExecutionError> {
        let task_class = task_class.into();
        if !SUPPORTED_TASK_CLASSES.contains(&task_class.as_str()) {
            return Err(ProviderExecutionError::invalid(
                "invalid_task_class",
                "Choose a supported Agent task class",
            ));
        }
        let provider_model_id = parse_text(
            provider_model_id.into(),
            1,
            240,
            "invalid_provider_model_id",
            "Provider model ID must contain between 1 and 240 bytes",
        )?;
        let system_prompt = parse_text(
            system_prompt.into(),
            1,
            MAX_SYSTEM_PROMPT_BYTES,
            "invalid_system_prompt",
            "System prompt must contain between 1 and 32768 bytes",
        )?;
        if messages.is_empty() || messages.len() > MAX_HISTORY_MESSAGES {
            return Err(ProviderExecutionError::invalid(
                "invalid_message_history",
                "Provide between 1 and 64 history messages",
            ));
        }
        if tools.len() > MAX_TOOLS {
            return Err(ProviderExecutionError::invalid(
                "invalid_tool_count",
                "Provide no more than 32 tools",
            ));
        }
        if !(1..=MAX_OUTPUT_TOKENS).contains(&max_output_tokens) {
            return Err(ProviderExecutionError::invalid(
                "invalid_output_budget",
                "Output token budget must be between 1 and 32768",
            ));
        }

        validate_tool_names(&tools)?;
        validate_history(&messages, &tools)?;
        let history_bytes = messages.iter().try_fold(0_usize, |total, message| {
            total.checked_add(message.byte_len())
        });
        if history_bytes.is_none_or(|bytes| bytes > MAX_HISTORY_BYTES) {
            return Err(ProviderExecutionError::invalid(
                "message_history_too_large",
                "Message history exceeds the 262144-byte limit",
            ));
        }
        let history_bytes = history_bytes.unwrap_or_default();
        let encoded_messages = messages.iter().fold(0_usize, |total, message| {
            total.saturating_add(message.encoded_input_upper_bound())
        });
        let encoded_tools = tools.iter().fold(0_usize, |total, tool| {
            total
                .saturating_add(encoded_string_len(tool.name()))
                .saturating_add(encoded_string_len(tool.description()))
                .saturating_add(
                    serde_json::to_vec(tool.input_schema()).map_or(usize::MAX, |value| value.len()),
                )
                // Conservatively covers request framing and provider-added
                // function-tool prompt overhead.
                .saturating_add(1_024)
        });
        let conservative_input_token_upper_bound = encoded_string_len(&system_prompt)
            .saturating_add(encoded_string_len(&provider_model_id))
            .saturating_add(encoded_messages)
            .saturating_add(encoded_tools)
            .saturating_add(4_096);
        let conservative_input_token_upper_bound =
            u64::try_from(conservative_input_token_upper_bound).map_err(|_| {
                ProviderExecutionError::invalid(
                    "provider_input_too_large",
                    "Provider input token estimate overflowed",
                )
            })?;

        Ok(Self {
            target,
            task_class,
            provider_model_id,
            system_prompt,
            messages,
            tools,
            // Unstructured campus user turns can contain personal or otherwise
            // sensitive records even when no capability tool is selected.
            required_provider_data_class: ProviderDataClass::SensitiveDataApproved,
            max_output_tokens,
            history_bytes,
            conservative_input_token_upper_bound,
        })
    }

    pub(crate) const fn target(&self) -> ProviderExecutionTarget {
        self.target
    }

    pub(crate) fn task_class(&self) -> &str {
        &self.task_class
    }

    pub(crate) fn provider_model_id(&self) -> &str {
        &self.provider_model_id
    }

    pub(crate) fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    pub(crate) fn messages(&self) -> &[ProviderMessage] {
        &self.messages
    }

    pub(crate) fn tools(&self) -> &[ProviderToolDefinition] {
        &self.tools
    }

    /// Raises the provider handling requirement for capability/context data.
    /// A later stage cannot weaken the baseline selected during parsing.
    #[must_use]
    pub fn requiring_provider_data_class(mut self, required: ProviderDataClass) -> Self {
        self.required_provider_data_class = self.required_provider_data_class.max(required);
        self
    }

    #[must_use]
    pub const fn required_provider_data_class(&self) -> ProviderDataClass {
        self.required_provider_data_class
    }

    pub(crate) const fn max_output_tokens(&self) -> u32 {
        self.max_output_tokens
    }

    /// Returns the fail-closed input-token reservation derived from the entire
    /// bounded command before dispatch. Durable workers use this public value
    /// for hard-limit admission; it is deliberately an upper bound rather than
    /// a provider-reported or tokenizer-specific estimate.
    #[must_use]
    pub const fn conservative_input_token_upper_bound(&self) -> u64 {
        self.conservative_input_token_upper_bound
    }

    pub(crate) fn validate_replayable_assistant(
        &self,
        text: &Option<String>,
        tool_calls: &[ProviderToolCall],
    ) -> Result<(), ProviderExecutionError> {
        let response = ProviderMessage::assistant(text.clone(), tool_calls.to_vec())?;
        let required_message_slots = 1_usize.saturating_add(tool_calls.len());
        if self
            .messages
            .len()
            .checked_add(required_message_slots)
            .is_none_or(|count| count > MAX_HISTORY_MESSAGES)
        {
            return Err(ProviderExecutionError::invalid(
                "message_history_too_long",
                "The assistant response and its tool results cannot fit in bounded replay history",
            ));
        }
        let future_tool_result_bytes =
            tool_calls
                .len()
                .checked_mul(MAX_MESSAGE_BYTES)
                .ok_or_else(|| {
                    ProviderExecutionError::invalid(
                        "message_history_too_large",
                        "Future tool results exceed bounded replay history",
                    )
                })?;
        if self
            .history_bytes
            .checked_add(response.byte_len())
            .and_then(|bytes| bytes.checked_add(future_tool_result_bytes))
            .is_none_or(|bytes| bytes > MAX_HISTORY_BYTES)
        {
            return Err(ProviderExecutionError::invalid(
                "message_history_too_large",
                "The assistant response cannot fit in bounded replay history",
            ));
        }
        Ok(())
    }
}

/// Provider-reported token counters. Missing provider fields remain `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
}

/// One normalized, bounded provider response.
#[derive(Clone, PartialEq)]
pub struct ProviderExecutionResponse {
    pub assistant_text: Option<String>,
    pub tool_calls: Vec<ProviderToolCall>,
    pub usage: ProviderUsage,
}

impl fmt::Debug for ProviderExecutionResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderExecutionResponse")
            .field(
                "assistant_text",
                &self.assistant_text.as_ref().map(|_| "[REDACTED]"),
            )
            .field("tool_call_count", &self.tool_calls.len())
            .field("usage", &self.usage)
            .finish()
    }
}

/// Safe provider failure evidence; raw bodies, headers, and request IDs are absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderExecutionFailure {
    pub category: ProviderFailureCategory,
    pub retryable: bool,
}

impl ProviderExecutionFailure {
    pub(crate) const fn from_category(category: ProviderFailureCategory) -> Self {
        Self {
            category,
            // A timeout or connection loss after dispatch has ambiguous delivery.
            // Only explicit upstream back-pressure/unavailability is retryable here.
            retryable: matches!(
                category,
                ProviderFailureCategory::RateLimited | ProviderFailureCategory::Unavailable
            ),
        }
    }

    /// Whether a durable worker may advance to the next ordered route target.
    /// Ambiguous delivery, authentication, malformed responses, and unsupported
    /// adapters must stop for reconciliation or administrator action.
    #[must_use]
    pub const fn fallback_eligible(self) -> bool {
        matches!(
            self.category,
            ProviderFailureCategory::RateLimited | ProviderFailureCategory::Unavailable
        )
    }
}

/// Stable, non-sensitive failures returned to the durable worker.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProviderExecutionError {
    #[error("{message}")]
    InvalidInput { code: &'static str, message: String },
    #[error("AI provider connection is unavailable")]
    ConnectionUnavailable,
    #[error("AI provider credential changed")]
    StaleCredential,
    #[error("AI provider data approval changed")]
    ProviderDataApprovalChanged,
    #[error("AI provider connection is not approved for this data")]
    ProviderDataNotApproved,
    #[error("AI provider execution must remain installation-local")]
    LocalExecutionRequired,
    #[error("AI provider model snapshot changed")]
    StaleModel,
    #[error("AI provider model does not support tools")]
    ToolsUnsupported,
    #[error("AI provider model context capacity is unavailable")]
    ModelContextUnavailable,
    #[error("AI provider model output capacity is unavailable")]
    ModelOutputUnavailable,
    #[error("AI provider input and output exceed the model context capacity")]
    ContextWindowExceeded,
    #[error("AI provider output budget exceeds the model capacity")]
    OutputBudgetExceeded,
    #[error("AI provider credential is unavailable")]
    CredentialUnavailable,
    #[error("AI provider configuration is invalid")]
    InvalidConfiguration,
    #[error("AI provider request failed")]
    Provider(ProviderExecutionFailure),
    #[error("AI provider execution state could not be loaded")]
    Storage,
}

impl ProviderExecutionError {
    #[must_use]
    pub fn invalid(code: &'static str, message: impl Into<String>) -> Self {
        Self::InvalidInput {
            code,
            message: message.into(),
        }
    }

    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { code, .. } => code,
            Self::ConnectionUnavailable => "provider_connection_unavailable",
            Self::StaleCredential => "provider_credential_changed",
            Self::ProviderDataApprovalChanged => "provider_data_approval_changed",
            Self::ProviderDataNotApproved => "provider_data_not_approved",
            Self::LocalExecutionRequired => "local_execution_required",
            Self::StaleModel => "provider_model_changed",
            Self::ToolsUnsupported => "provider_tools_unsupported",
            Self::ModelContextUnavailable => "provider_model_context_unavailable",
            Self::ModelOutputUnavailable => "provider_model_output_unavailable",
            Self::ContextWindowExceeded => "provider_context_window_exceeded",
            Self::OutputBudgetExceeded => "provider_output_budget_exceeded",
            Self::CredentialUnavailable => "provider_credential_unavailable",
            Self::InvalidConfiguration => "provider_configuration_invalid",
            Self::Provider(failure) => failure.category.as_str(),
            Self::Storage => "provider_execution_storage_error",
        }
    }

    #[must_use]
    pub const fn provider_failure(&self) -> Option<ProviderExecutionFailure> {
        match self {
            Self::Provider(failure) => Some(*failure),
            _ => None,
        }
    }
}

pub(crate) enum ProviderMessageRef<'a> {
    User(&'a str),
    Assistant {
        text: Option<&'a str>,
        tool_calls: &'a [ProviderToolCall],
    },
    ToolResult {
        tool_call_id: &'a str,
        name: &'a str,
        content: &'a str,
        is_error: bool,
    },
}

impl MessageKind {
    fn as_ref(&self) -> ProviderMessageRef<'_> {
        match self {
            Self::User(content) => ProviderMessageRef::User(content),
            Self::Assistant { text, tool_calls } => ProviderMessageRef::Assistant {
                text: text.as_deref(),
                tool_calls,
            },
            Self::ToolResult {
                tool_call_id,
                name,
                content,
                is_error,
            } => ProviderMessageRef::ToolResult {
                tool_call_id,
                name,
                content,
                is_error: *is_error,
            },
        }
    }
}

impl ProviderMessage {
    pub(crate) fn as_ref(&self) -> ProviderMessageRef<'_> {
        self.0.as_ref()
    }
}

fn parse_message_text(value: String) -> Result<String, ProviderExecutionError> {
    parse_text(
        value,
        1,
        MAX_MESSAGE_BYTES,
        "invalid_message_content",
        "Message content must contain between 1 and 65536 bytes",
    )
}

fn encoded_string_len(value: &str) -> usize {
    serde_json::to_vec(value).map_or(usize::MAX, |encoded| encoded.len())
}

fn parse_text(
    value: String,
    min_bytes: usize,
    max_bytes: usize,
    code: &'static str,
    message: &'static str,
) -> Result<String, ProviderExecutionError> {
    let value = value.trim().to_owned();
    if value.len() < min_bytes || value.len() > max_bytes || value.contains('\0') {
        return Err(ProviderExecutionError::invalid(code, message));
    }
    Ok(value)
}

fn parse_tool_name(value: String) -> Result<String, ProviderExecutionError> {
    if value.is_empty()
        || value.len() > MAX_TOOL_NAME_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(ProviderExecutionError::invalid(
            "invalid_tool_name",
            "Tool names must use 1 to 64 letters, numbers, underscores, or hyphens",
        ));
    }
    Ok(value)
}

fn parse_tool_call_id(value: String) -> Result<String, ProviderExecutionError> {
    if value.is_empty()
        || value.len() > MAX_TOOL_CALL_ID_BYTES
        || !value.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(ProviderExecutionError::invalid(
            "invalid_tool_call_id",
            "Tool call IDs must contain between 1 and 128 visible ASCII bytes",
        ));
    }
    Ok(value)
}

fn validate_json_object(
    value: &Value,
    max_bytes: usize,
    code: &'static str,
    message: &'static str,
) -> Result<(), ProviderExecutionError> {
    if !value.is_object()
        || json_depth(value) > MAX_JSON_DEPTH
        || serde_json::to_vec(value).map_or(true, |encoded| encoded.len() > max_bytes)
    {
        return Err(ProviderExecutionError::invalid(code, message));
    }
    Ok(())
}

fn json_depth(value: &Value) -> usize {
    match value {
        Value::Array(values) => 1 + values.iter().map(json_depth).max().unwrap_or(0),
        Value::Object(values) => 1 + values.values().map(json_depth).max().unwrap_or(0),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => 1,
    }
}

fn validate_tool_call_set(calls: &[ProviderToolCall]) -> Result<(), ProviderExecutionError> {
    if calls.len() > MAX_TOOLS {
        return Err(ProviderExecutionError::invalid(
            "invalid_tool_call_count",
            "One assistant message cannot contain more than 32 tool calls",
        ));
    }
    let unique = calls
        .iter()
        .map(|call| call.id.as_str())
        .collect::<BTreeSet<_>>();
    if unique.len() != calls.len() {
        return Err(ProviderExecutionError::invalid(
            "duplicate_tool_call_id",
            "Tool call IDs must be unique",
        ));
    }
    Ok(())
}

fn validate_tool_names(tools: &[ProviderToolDefinition]) -> Result<(), ProviderExecutionError> {
    let unique = tools
        .iter()
        .map(ProviderToolDefinition::name)
        .collect::<BTreeSet<_>>();
    if unique.len() != tools.len() {
        return Err(ProviderExecutionError::invalid(
            "duplicate_tool_name",
            "Tool names must be unique",
        ));
    }
    Ok(())
}

fn validate_history(
    messages: &[ProviderMessage],
    tools: &[ProviderToolDefinition],
) -> Result<(), ProviderExecutionError> {
    let allowed_tools = tools
        .iter()
        .map(|tool| tool.name())
        .collect::<BTreeSet<_>>();
    let mut pending_calls = BTreeMap::<&str, &str>::new();
    let mut seen_calls = BTreeSet::<&str>::new();
    for message in messages {
        match message.as_ref() {
            ProviderMessageRef::Assistant { tool_calls, .. } => {
                if !pending_calls.is_empty() {
                    return Err(incomplete_tool_results());
                }
                for call in tool_calls {
                    if !allowed_tools.contains(call.name.as_str()) {
                        return Err(ProviderExecutionError::invalid(
                            "unknown_tool_call",
                            "Assistant history references a tool that is not available",
                        ));
                    }
                    if !seen_calls.insert(&call.id)
                        || pending_calls.insert(&call.id, &call.name).is_some()
                    {
                        return Err(ProviderExecutionError::invalid(
                            "duplicate_tool_call_id",
                            "Tool call IDs must be unique",
                        ));
                    }
                }
            }
            ProviderMessageRef::ToolResult {
                tool_call_id, name, ..
            } => {
                if pending_calls.remove(tool_call_id) != Some(name) {
                    return Err(ProviderExecutionError::invalid(
                        "invalid_tool_result",
                        "Tool results must match the immediately preceding assistant tool calls",
                    ));
                }
            }
            ProviderMessageRef::User(_) if !pending_calls.is_empty() => {
                return Err(incomplete_tool_results());
            }
            ProviderMessageRef::User(_) => {}
        }
    }
    if !pending_calls.is_empty() {
        return Err(incomplete_tool_results());
    }
    Ok(())
}

fn incomplete_tool_results() -> ProviderExecutionError {
    ProviderExecutionError::invalid(
        "incomplete_tool_results",
        "Every assistant tool call must be followed immediately by one matching tool result",
    )
}

#[cfg(test)]
mod tests {
    use cp_common::ProviderDataClass;
    use serde_json::json;
    use uuid::Uuid;

    use super::{
        ExecuteProviderCommand, ProviderExecutionError, ProviderExecutionFailure,
        ProviderExecutionResponse, ProviderExecutionTarget, ProviderMessage, ProviderToolCall,
        ProviderToolDefinition, ProviderUsage,
    };
    use crate::ProviderFailureCategory;

    fn target() -> ProviderExecutionTarget {
        ProviderExecutionTarget::parse(
            Uuid::new_v4(),
            1,
            Uuid::new_v4(),
            Uuid::new_v4(),
            2,
            Uuid::new_v4(),
            Uuid::new_v4(),
            true,
        )
        .unwrap()
    }

    fn tool() -> ProviderToolDefinition {
        ProviderToolDefinition::parse(
            "lookup_learner",
            "Look up one learner.",
            json!({"type":"object","properties":{"id":{"type":"string"}}}),
        )
        .unwrap()
    }

    #[test]
    fn bounded_command_accepts_complete_tool_history() {
        let call = ProviderToolCall::parse("call_1", "lookup_learner", json!({"id":"7"})).unwrap();
        let command = ExecuteProviderCommand::parse(
            target(),
            "module_read_reporting",
            "claude-sonnet-4",
            "Answer from campus records only.",
            vec![
                ProviderMessage::user("Find the learner").unwrap(),
                ProviderMessage::assistant(None, vec![call]).unwrap(),
                ProviderMessage::tool_result("call_1", "lookup_learner", "Found", false).unwrap(),
            ],
            vec![tool()],
            1024,
        )
        .unwrap();
        assert_eq!(command.provider_model_id(), "claude-sonnet-4");
        assert_eq!(command.messages().len(), 3);
        assert_eq!(
            command.required_provider_data_class(),
            ProviderDataClass::SensitiveDataApproved
        );
        let command = command.requiring_provider_data_class(ProviderDataClass::CampusApproved);
        assert_eq!(
            command.required_provider_data_class(),
            ProviderDataClass::SensitiveDataApproved
        );
        let command = command.requiring_provider_data_class(ProviderDataClass::LocalOnly);
        assert_eq!(
            command.required_provider_data_class(),
            ProviderDataClass::LocalOnly
        );
    }

    #[test]
    fn command_rejects_unbounded_and_invalid_shapes() {
        assert!(
            ProviderExecutionTarget::parse(
                Uuid::nil(),
                0,
                Uuid::nil(),
                Uuid::nil(),
                0,
                Uuid::nil(),
                Uuid::nil(),
                false,
            )
            .is_err()
        );
        assert!(ProviderMessage::user("\0").is_err());
        assert!(ProviderMessage::assistant(None, Vec::new()).is_err());
        assert!(ProviderToolDefinition::parse("bad name", "description", json!({})).is_err());
        assert!(ProviderToolDefinition::parse("tool", "description", json!([])).is_err());
        assert!(ProviderToolCall::parse("bad id", "tool", json!({})).is_err());
        assert!(ProviderMessage::user("x".repeat(65 * 1024)).is_err());
        assert_eq!(
            ExecuteProviderCommand::parse(
                target(),
                "unknown_task",
                "model",
                "prompt",
                vec![ProviderMessage::user("hello").unwrap()],
                Vec::new(),
                1,
            )
            .err()
            .unwrap()
            .code(),
            "invalid_task_class"
        );
        assert!(
            ExecuteProviderCommand::parse(
                target(),
                "module_read_reporting",
                "model",
                "prompt",
                vec![ProviderMessage::user("hello").unwrap()],
                Vec::new(),
                0,
            )
            .is_err()
        );
    }

    #[test]
    fn campus_conversation_task_class_is_supported() {
        let command = ExecuteProviderCommand::parse(
            target(),
            "campus_conversation",
            "model",
            "Answer from the current campus session.",
            vec![ProviderMessage::user("Hello").unwrap()],
            Vec::new(),
            256,
        )
        .unwrap();

        assert_eq!(command.task_class(), "campus_conversation");
        assert_eq!(
            command.required_provider_data_class(),
            ProviderDataClass::SensitiveDataApproved
        );
    }

    #[test]
    fn assistant_responses_must_fit_message_and_remaining_history_bounds() {
        let tool_result_content_limit =
            super::MAX_MESSAGE_BYTES - "call_1".len() - "lookup_learner".len();
        assert!(
            ProviderMessage::tool_result(
                "call_1",
                "lookup_learner",
                "x".repeat(tool_result_content_limit),
                false,
            )
            .is_ok()
        );
        assert_eq!(
            ProviderMessage::tool_result(
                "call_1",
                "lookup_learner",
                "x".repeat(tool_result_content_limit + 1),
                false,
            )
            .err()
            .unwrap()
            .code(),
            "tool_result_message_too_large"
        );

        let oversized_calls = (0..3)
            .map(|index| {
                ProviderToolCall::parse(
                    format!("call_{index}"),
                    "lookup_learner",
                    json!({"padding":"x".repeat(22_000)}),
                )
                .unwrap()
            })
            .collect();
        assert_eq!(
            ProviderMessage::assistant(None, oversized_calls)
                .err()
                .unwrap()
                .code(),
            "assistant_message_too_large"
        );

        let command = ExecuteProviderCommand::parse(
            target(),
            "module_read_reporting",
            "model",
            "prompt",
            (0..4)
                .map(|_| ProviderMessage::user("x".repeat(60 * 1024)).unwrap())
                .collect(),
            Vec::new(),
            100,
        )
        .unwrap();
        assert!(
            command
                .validate_replayable_assistant(&Some("r".repeat(16 * 1024)), &[])
                .is_ok()
        );
        assert_eq!(
            command
                .validate_replayable_assistant(&Some("r".repeat(16 * 1024 + 1)), &[])
                .unwrap_err()
                .code(),
            "message_history_too_large"
        );
        assert!(command.conservative_input_token_upper_bound() > 240 * 1024);

        let full_count = ExecuteProviderCommand::parse(
            target(),
            "module_read_reporting",
            "model",
            "prompt",
            (0..64)
                .map(|index| ProviderMessage::user(format!("message {index}")).unwrap())
                .collect(),
            Vec::new(),
            100,
        )
        .unwrap();
        assert_eq!(
            full_count
                .validate_replayable_assistant(&Some("response".to_owned()), &[])
                .unwrap_err()
                .code(),
            "message_history_too_long"
        );

        let reserved_call = ProviderToolCall::parse("call_1", "lookup_learner", json!({})).unwrap();
        let reservation_fits = ExecuteProviderCommand::parse(
            target(),
            "module_read_reporting",
            "model",
            "prompt",
            (0..3)
                .map(|_| ProviderMessage::user("x".repeat(60 * 1024)).unwrap())
                .collect(),
            vec![tool()],
            100,
        )
        .unwrap();
        assert!(
            reservation_fits
                .validate_replayable_assistant(&None, std::slice::from_ref(&reserved_call))
                .is_ok()
        );
        let reservation_overflows = ExecuteProviderCommand::parse(
            target(),
            "module_read_reporting",
            "model",
            "prompt",
            (0..4)
                .map(|_| ProviderMessage::user("x".repeat(48 * 1024)).unwrap())
                .collect(),
            vec![tool()],
            100,
        )
        .unwrap();
        assert_eq!(
            reservation_overflows
                .validate_replayable_assistant(&None, &[reserved_call])
                .unwrap_err()
                .code(),
            "message_history_too_large"
        );
    }

    #[test]
    fn history_rejects_unknown_duplicate_or_incomplete_tool_calls() {
        let call = ProviderToolCall::parse("call_1", "lookup_learner", json!({})).unwrap();
        let incomplete = ExecuteProviderCommand::parse(
            target(),
            "module_read_reporting",
            "model",
            "prompt",
            vec![ProviderMessage::assistant(None, vec![call]).unwrap()],
            vec![tool()],
            100,
        );
        assert_eq!(incomplete.err().unwrap().code(), "incomplete_tool_results");

        let unknown = ProviderToolCall::parse("call_2", "unknown", json!({})).unwrap();
        let unknown = ExecuteProviderCommand::parse(
            target(),
            "module_read_reporting",
            "model",
            "prompt",
            vec![ProviderMessage::assistant(None, vec![unknown]).unwrap()],
            vec![tool()],
            100,
        );
        assert_eq!(unknown.err().unwrap().code(), "unknown_tool_call");

        let duplicate_tools = ExecuteProviderCommand::parse(
            target(),
            "module_read_reporting",
            "model",
            "prompt",
            vec![ProviderMessage::user("hello").unwrap()],
            vec![tool(), tool()],
            100,
        );
        assert_eq!(duplicate_tools.err().unwrap().code(), "duplicate_tool_name");
    }

    #[test]
    fn response_failure_exposes_only_category_and_retryability() {
        let rate_limit =
            ProviderExecutionFailure::from_category(ProviderFailureCategory::RateLimited);
        let timeout = ProviderExecutionFailure::from_category(ProviderFailureCategory::Timeout);
        assert!(rate_limit.retryable);
        assert!(rate_limit.fallback_eligible());
        assert!(!timeout.retryable);
        assert!(!timeout.fallback_eligible());
        let error = ProviderExecutionError::Provider(timeout);
        assert_eq!(error.code(), "timeout");
        assert_eq!(error.provider_failure(), Some(timeout));
        assert!(!format!("{error:?}").contains("secret"));
    }

    #[test]
    fn fallback_is_explicitly_limited_to_safe_transient_categories() {
        for (category, eligible) in [
            (ProviderFailureCategory::Authentication, false),
            (ProviderFailureCategory::RateLimited, true),
            (ProviderFailureCategory::Unavailable, true),
            (ProviderFailureCategory::Timeout, false),
            (ProviderFailureCategory::Network, false),
            (ProviderFailureCategory::InvalidResponse, false),
            (ProviderFailureCategory::Unsupported, false),
        ] {
            let failure = ProviderExecutionFailure::from_category(category);
            assert_eq!(failure.retryable, eligible, "{category:?}");
            assert_eq!(failure.fallback_eligible(), eligible, "{category:?}");
        }
    }

    #[test]
    fn opaque_target_and_tool_call_expose_only_explicit_accessors() {
        let connection_id = Uuid::new_v4();
        let model_id = Uuid::new_v4();
        let route_set_id = Uuid::new_v4();
        let route_target_id = Uuid::new_v4();
        let target = ProviderExecutionTarget::parse(
            route_set_id,
            3,
            route_target_id,
            connection_id,
            7,
            model_id,
            Uuid::new_v4(),
            true,
        )
        .unwrap();
        assert_eq!(target.route_set_id(), route_set_id);
        assert_eq!(target.route_version(), 3);
        assert_eq!(target.route_target_id(), route_target_id);
        assert_eq!(target.connection_id(), connection_id);
        assert_eq!(target.expected_credential_version(), 7);
        assert_eq!(target.model_snapshot_id(), model_id);
        assert_ne!(target.provider_data_approval_id(), Uuid::nil());
        assert!(target.requires_tools());

        let call = ProviderToolCall::parse(
            "call_1",
            "lookup_learner",
            json!({"private":"learner record"}),
        )
        .unwrap();
        assert_eq!(call.id(), "call_1");
        assert_eq!(call.name(), "lookup_learner");
        assert_eq!(call.arguments()["private"], "learner record");
        let debug = format!("{call:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("learner record"));
        let (id, name, arguments) = call.into_parts();
        assert_eq!((id.as_str(), name.as_str()), ("call_1", "lookup_learner"));
        assert_eq!(arguments["private"], "learner record");
    }

    #[test]
    fn successful_response_debug_redacts_model_content() {
        let response = ProviderExecutionResponse {
            assistant_text: Some("private learner answer".to_owned()),
            tool_calls: vec![
                ProviderToolCall::parse("call_1", "lookup_learner", json!({"private":"arguments"}))
                    .unwrap(),
            ],
            usage: ProviderUsage {
                input_tokens: Some(10),
                output_tokens: Some(5),
                total_tokens: Some(15),
                reasoning_tokens: None,
                cached_input_tokens: None,
                cache_creation_input_tokens: None,
            },
        };
        let debug = format!("{response:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(debug.contains("tool_call_count: 1"));
        assert!(!debug.contains("private learner answer"));
        assert!(!debug.contains("arguments"));
    }

    #[test]
    fn every_command_collection_and_byte_limit_fails_closed() {
        let empty = ExecuteProviderCommand::parse(
            target(),
            "module_read_reporting",
            "model",
            "prompt",
            Vec::new(),
            Vec::new(),
            1,
        );
        assert_eq!(empty.err().unwrap().code(), "invalid_message_history");

        let too_many_messages = ExecuteProviderCommand::parse(
            target(),
            "module_read_reporting",
            "model",
            "prompt",
            (0..65)
                .map(|_| ProviderMessage::user("x").unwrap())
                .collect(),
            Vec::new(),
            1,
        );
        assert_eq!(
            too_many_messages.err().unwrap().code(),
            "invalid_message_history"
        );

        let too_many_tools = ExecuteProviderCommand::parse(
            target(),
            "module_read_reporting",
            "model",
            "prompt",
            vec![ProviderMessage::user("x").unwrap()],
            (0..33)
                .map(|index| {
                    ProviderToolDefinition::parse(format!("tool_{index}"), "description", json!({}))
                        .unwrap()
                })
                .collect(),
            1,
        );
        assert_eq!(too_many_tools.err().unwrap().code(), "invalid_tool_count");

        let too_many_bytes = ExecuteProviderCommand::parse(
            target(),
            "module_read_reporting",
            "model",
            "prompt",
            (0..5)
                .map(|_| ProviderMessage::user("x".repeat(60 * 1024)).unwrap())
                .collect(),
            Vec::new(),
            1,
        );
        assert_eq!(
            too_many_bytes.err().unwrap().code(),
            "message_history_too_large"
        );
        for (model, prompt, tokens, expected) in [
            ("", "prompt", 1, "invalid_provider_model_id"),
            ("model", "", 1, "invalid_system_prompt"),
            ("model", "prompt", 32_769, "invalid_output_budget"),
        ] {
            let error = ExecuteProviderCommand::parse(
                target(),
                "module_read_reporting",
                model,
                prompt,
                vec![ProviderMessage::user("x").unwrap()],
                Vec::new(),
                tokens,
            )
            .err()
            .unwrap();
            assert_eq!(error.code(), expected);
        }
    }

    #[test]
    fn tool_schema_argument_and_history_edges_are_rejected() {
        assert!(ProviderToolDefinition::parse("tool", "", json!({})).is_err());
        assert!(ProviderToolDefinition::parse("tool", "x".repeat(4097), json!({})).is_err());
        assert!(
            ProviderToolDefinition::parse(
                "tool",
                "description",
                json!({"padding":"x".repeat(33 * 1024)})
            )
            .is_err()
        );
        let mut nested = json!({});
        for _ in 0..17 {
            nested = json!({"nested":nested});
        }
        assert!(ProviderToolDefinition::parse("tool", "description", nested.clone()).is_err());
        assert!(ProviderToolCall::parse("call", "tool", json!([])).is_err());
        assert!(ProviderToolCall::parse("call", "tool", nested).is_err());
        assert!(ProviderToolCall::parse("x".repeat(129), "tool", json!({})).is_err());
        assert!(ProviderToolCall::parse("call", "x".repeat(65), json!({})).is_err());

        let calls = (0..33)
            .map(|index| {
                ProviderToolCall::parse(format!("call_{index}"), "tool", json!({})).unwrap()
            })
            .collect();
        assert!(ProviderMessage::assistant(None, calls).is_err());
        let duplicate = ProviderMessage::assistant(
            None,
            vec![
                ProviderToolCall::parse("call", "tool", json!({})).unwrap(),
                ProviderToolCall::parse("call", "tool", json!({})).unwrap(),
            ],
        );
        assert!(duplicate.is_err());

        let call = || ProviderToolCall::parse("call", "lookup_learner", json!({})).unwrap();
        let user_before_result = ExecuteProviderCommand::parse(
            target(),
            "module_read_reporting",
            "model",
            "prompt",
            vec![
                ProviderMessage::assistant(None, vec![call()]).unwrap(),
                ProviderMessage::user("next").unwrap(),
            ],
            vec![tool()],
            10,
        );
        assert_eq!(
            user_before_result.err().unwrap().code(),
            "incomplete_tool_results"
        );
        let wrong_result = ExecuteProviderCommand::parse(
            target(),
            "module_read_reporting",
            "model",
            "prompt",
            vec![
                ProviderMessage::assistant(None, vec![call()]).unwrap(),
                ProviderMessage::tool_result("call", "wrong", "result", false).unwrap(),
            ],
            vec![tool()],
            10,
        );
        assert_eq!(wrong_result.err().unwrap().code(), "invalid_tool_result");
        let reused_call = ExecuteProviderCommand::parse(
            target(),
            "module_read_reporting",
            "model",
            "prompt",
            vec![
                ProviderMessage::assistant(None, vec![call()]).unwrap(),
                ProviderMessage::tool_result("call", "lookup_learner", "one", false).unwrap(),
                ProviderMessage::assistant(None, vec![call()]).unwrap(),
                ProviderMessage::tool_result("call", "lookup_learner", "two", false).unwrap(),
            ],
            vec![tool()],
            10,
        );
        assert_eq!(reused_call.err().unwrap().code(), "duplicate_tool_call_id");
    }

    #[test]
    fn all_safe_local_failure_codes_are_stable() {
        for (error, code) in [
            (
                ProviderExecutionError::ConnectionUnavailable,
                "provider_connection_unavailable",
            ),
            (
                ProviderExecutionError::StaleCredential,
                "provider_credential_changed",
            ),
            (
                ProviderExecutionError::ProviderDataApprovalChanged,
                "provider_data_approval_changed",
            ),
            (
                ProviderExecutionError::ProviderDataNotApproved,
                "provider_data_not_approved",
            ),
            (
                ProviderExecutionError::LocalExecutionRequired,
                "local_execution_required",
            ),
            (ProviderExecutionError::StaleModel, "provider_model_changed"),
            (
                ProviderExecutionError::ToolsUnsupported,
                "provider_tools_unsupported",
            ),
            (
                ProviderExecutionError::ModelContextUnavailable,
                "provider_model_context_unavailable",
            ),
            (
                ProviderExecutionError::ModelOutputUnavailable,
                "provider_model_output_unavailable",
            ),
            (
                ProviderExecutionError::ContextWindowExceeded,
                "provider_context_window_exceeded",
            ),
            (
                ProviderExecutionError::OutputBudgetExceeded,
                "provider_output_budget_exceeded",
            ),
            (
                ProviderExecutionError::CredentialUnavailable,
                "provider_credential_unavailable",
            ),
            (
                ProviderExecutionError::InvalidConfiguration,
                "provider_configuration_invalid",
            ),
            (
                ProviderExecutionError::Storage,
                "provider_execution_storage_error",
            ),
        ] {
            assert_eq!(error.code(), code);
            assert_eq!(error.provider_failure(), None);
        }
    }
}
