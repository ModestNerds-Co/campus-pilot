//! Defines validated AI-provider commands and secret-free administration views.
//!
//! API keys are write-only secret values; connection and model responses cannot
//! represent ciphertext, nonces, encryption-key identifiers, or plaintext.
//!
//! Copyright (c) 2026 Codecraft Solutions. All rights reserved.

use std::{fmt, str::FromStr};

use chrono::{DateTime, Utc};
use cp_common::{ProviderApprovalClass, ProviderExecutionEnvironmentClass};
use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use sqlx::FromRow;
use thiserror::Error;
use uuid::Uuid;

const MAX_API_KEY_LENGTH: usize = 4096;
const MIN_API_KEY_LENGTH: usize = 16;
const MAX_ACCOUNT_LABEL_LENGTH: usize = 100;

/// A provider supported by the first app-managed credential release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKey {
    OpenAi,
    Anthropic,
    OpenRouter,
}

impl ProviderKey {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::OpenRouter => "openrouter",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::OpenRouter => "OpenRouter",
        }
    }
}

impl FromStr for ProviderKey {
    type Err = ServiceError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "openai" => Ok(Self::OpenAi),
            "anthropic" => Ok(Self::Anthropic),
            "openrouter" => Ok(Self::OpenRouter),
            _ => Err(ServiceError::invalid(
                "unsupported_provider",
                "Choose OpenAI, Anthropic, or OpenRouter",
            )),
        }
    }
}

/// Authentication methods supported by the current provider adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    ApiKey,
}

impl AuthMethod {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        "api_key"
    }
}

impl FromStr for AuthMethod {
    type Err = ServiceError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.trim().eq_ignore_ascii_case("api_key") {
            Ok(Self::ApiKey)
        } else {
            Err(ServiceError::invalid(
                "unsupported_auth_method",
                "The selected provider supports API-key authentication",
            ))
        }
    }
}

#[derive(Clone)]
pub(crate) struct ApiKey(SecretString);

impl fmt::Debug for ApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApiKey([REDACTED])")
    }
}

impl ApiKey {
    pub(crate) fn parse(value: impl Into<String>) -> Result<Self, ServiceError> {
        let value = value.into();
        let normalized = value.trim();
        if normalized.len() < MIN_API_KEY_LENGTH || normalized.len() > MAX_API_KEY_LENGTH {
            return Err(ServiceError::invalid(
                "invalid_api_key",
                "The API key length is invalid",
            ));
        }
        if normalized
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
        {
            return Err(ServiceError::invalid(
                "invalid_api_key",
                "The API key cannot contain whitespace or control characters",
            ));
        }
        Ok(Self(SecretString::from(normalized.to_owned())))
    }

    pub(crate) fn expose(&self) -> &str {
        self.0.expose_secret()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountLabel(String);

impl AccountLabel {
    fn parse(value: impl Into<String>) -> Result<Self, ServiceError> {
        let normalized = value.into().trim().to_owned();
        if normalized.is_empty() || normalized.chars().count() > MAX_ACCOUNT_LABEL_LENGTH {
            return Err(ServiceError::invalid(
                "invalid_account_label",
                "Account label must contain between 1 and 100 characters",
            ));
        }
        Ok(Self(normalized))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// A parsed command for creating one tenant-scoped provider connection.
#[derive(Debug, Clone)]
pub struct CreateConnectionCommand {
    pub(crate) provider: ProviderKey,
    pub(crate) auth_method: AuthMethod,
    pub(crate) account_label: AccountLabel,
    pub(crate) api_key: ApiKey,
}

impl CreateConnectionCommand {
    pub fn parse(
        provider: &str,
        auth_method: &str,
        account_label: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Result<Self, ServiceError> {
        Ok(Self {
            provider: ProviderKey::from_str(provider)?,
            auth_method: AuthMethod::from_str(auth_method)?,
            account_label: AccountLabel::parse(account_label)?,
            api_key: ApiKey::parse(api_key)?,
        })
    }
}

/// A version-checked command for renaming a connection.
#[derive(Debug, Clone)]
pub struct UpdateConnectionCommand {
    pub(crate) account_label: AccountLabel,
    pub(crate) expected_version: i64,
}

impl UpdateConnectionCommand {
    pub fn parse(
        account_label: impl Into<String>,
        expected_version: i64,
    ) -> Result<Self, ServiceError> {
        Ok(Self {
            account_label: AccountLabel::parse(account_label)?,
            expected_version: positive_version(expected_version)?,
        })
    }
}

/// A version-checked write-only credential rotation command.
#[derive(Debug, Clone)]
pub struct RotateCredentialCommand {
    pub(crate) api_key: ApiKey,
    pub(crate) expected_version: i64,
}

/// Optimistically versioned administrator decision for provider data handling.
#[derive(Debug, Clone)]
pub struct SetProviderDataApprovalCommand {
    pub(crate) approval_class: ProviderApprovalClass,
    pub(crate) expected_approval_version: i64,
    pub(crate) change_reason: String,
}

impl SetProviderDataApprovalCommand {
    pub fn parse(
        approval_class: &str,
        expected_approval_version: i64,
        change_reason: impl Into<String>,
    ) -> Result<Self, ServiceError> {
        let approval_class = ProviderApprovalClass::from_str(approval_class).map_err(|_| {
            ServiceError::invalid(
                "invalid_provider_data_approval_class",
                "Choose unapproved, campus approved, or sensitive-data approved",
            )
        })?;
        let change_reason = change_reason.into().trim().to_owned();
        if !(3..=500).contains(&change_reason.chars().count()) {
            return Err(ServiceError::invalid(
                "invalid_provider_data_approval_reason",
                "Change reason must contain between 3 and 500 characters",
            ));
        }
        Ok(Self {
            approval_class,
            expected_approval_version: positive_version(expected_approval_version)?,
            change_reason,
        })
    }
}

impl RotateCredentialCommand {
    pub fn parse(api_key: impl Into<String>, expected_version: i64) -> Result<Self, ServiceError> {
        Ok(Self {
            api_key: ApiKey::parse(api_key)?,
            expected_version: positive_version(expected_version)?,
        })
    }
}

pub(crate) fn positive_version(value: i64) -> Result<i64, ServiceError> {
    if value > 0 {
        Ok(value)
    } else {
        Err(ServiceError::invalid(
            "invalid_expected_version",
            "Expected version must be a positive integer",
        ))
    }
}

/// Safe catalogue metadata for a code-owned provider adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProviderCatalogEntry {
    pub key: &'static str,
    pub label: &'static str,
    pub auth_methods: &'static [&'static str],
    pub credential_hint: &'static str,
    pub supports_connection_test: bool,
    pub supports_model_refresh: bool,
    pub execution_environment_class: ProviderExecutionEnvironmentClass,
}

const API_KEY_AUTH: &[&str] = &["api_key"];
const PROVIDER_CATALOG: &[ProviderCatalogEntry] = &[
    ProviderCatalogEntry {
        key: "openai",
        label: "OpenAI",
        auth_methods: API_KEY_AUTH,
        credential_hint: "OpenAI API key",
        supports_connection_test: true,
        supports_model_refresh: true,
        execution_environment_class: ProviderExecutionEnvironmentClass::ExternalManaged,
    },
    ProviderCatalogEntry {
        key: "anthropic",
        label: "Anthropic",
        auth_methods: API_KEY_AUTH,
        credential_hint: "Anthropic API key",
        supports_connection_test: true,
        supports_model_refresh: true,
        execution_environment_class: ProviderExecutionEnvironmentClass::ExternalManaged,
    },
    ProviderCatalogEntry {
        key: "openrouter",
        label: "OpenRouter",
        auth_methods: API_KEY_AUTH,
        credential_hint: "OpenRouter API key",
        supports_connection_test: true,
        supports_model_refresh: true,
        execution_environment_class: ProviderExecutionEnvironmentClass::ExternalManaged,
    },
];

#[must_use]
pub const fn provider_catalog() -> &'static [ProviderCatalogEntry] {
    PROVIDER_CATALOG
}

/// Secret-free current state returned by list, read, and mutation endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, FromRow)]
pub struct AiProviderConnection {
    pub id: Uuid,
    pub provider: String,
    pub provider_label: String,
    pub auth_method: String,
    pub account_label: String,
    pub status: String,
    pub credential_fingerprint: String,
    pub credential_version: i64,
    pub version: i64,
    pub configured_by_name: String,
    pub last_tested_at: Option<DateTime<Utc>>,
    pub last_test_status: Option<String>,
    pub last_failure_category: Option<String>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub model_count: i64,
    pub model_catalog_refreshed_at: Option<DateTime<Utc>>,
    pub provider_data_approval_id: Uuid,
    pub provider_data_approval_version: i64,
    pub provider_data_approval_class: String,
    pub execution_environment_class: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Audited administrator projection of one immutable provider approval version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, FromRow)]
pub struct ProviderDataApproval {
    pub id: Uuid,
    pub connection_id: Uuid,
    pub approval_version: i64,
    pub approval_class: String,
    pub execution_environment_class: String,
    pub change_source: String,
    pub changed_by_name: Option<String>,
    pub change_reason: String,
    pub created_at: DateTime<Utc>,
}

/// Persisted connection lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Untested,
    Ready,
    Error,
}

impl ConnectionStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Untested => "untested",
            Self::Ready => "ready",
            Self::Error => "error",
        }
    }
}

/// Safe, normalized upstream failure categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFailureCategory {
    Authentication,
    RateLimited,
    Unavailable,
    Timeout,
    Network,
    InvalidResponse,
    Unsupported,
}

impl ProviderFailureCategory {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderFailure {
    pub category: ProviderFailureCategory,
}

/// Result of a server-side provider credential test.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConnectionTestOutcome {
    pub status: String,
    pub failure_category: Option<String>,
    pub tested_at: DateTime<Utc>,
}

/// Updated safe connection state and its test outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConnectionTestResult {
    pub connection: AiProviderConnection,
    pub outcome: ConnectionTestOutcome,
}

/// One provider-reported model normalized into a stable safe shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, FromRow)]
pub struct ProviderModel {
    #[sqlx(rename = "provider_model_id")]
    pub id: String,
    pub display_name: String,
    pub context_window_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub supports_tools: Option<bool>,
    pub source: String,
}

/// Current immutable provider-model snapshot for one connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConnectionModelSnapshot {
    pub connection_id: Uuid,
    pub provider: String,
    pub credential_version: i64,
    pub refreshed_at: Option<DateTime<Utc>>,
    pub models: Vec<ProviderModel>,
}

/// Confirmation returned after a credential has been purged and disconnected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DisconnectedConnection {
    pub disconnected_id: Uuid,
}

/// Stable service failures mapped to non-sensitive HTTP responses by the app.
#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("{message}")]
    InvalidInput { code: &'static str, message: String },
    #[error("AI provider connection was not found")]
    NotFound,
    #[error("{message}")]
    Conflict { code: &'static str, message: String },
    #[error("AI provider credential encryption is not configured")]
    CredentialStorageUnavailable,
    #[error("AI provider credential could not be opened")]
    CredentialUnavailable,
    #[error("AI provider request failed")]
    ProviderFailed(ProviderFailureCategory),
    #[error("AI provider persistence failed")]
    Storage(#[source] sqlx::Error),
}

impl ServiceError {
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
            Self::NotFound => "connection_not_found",
            Self::CredentialStorageUnavailable => "credential_storage_unavailable",
            Self::CredentialUnavailable => "credential_unavailable",
            Self::ProviderFailed(category) => category.as_str(),
            Self::Storage(_) => "provider_storage_error",
        }
    }

    #[must_use]
    pub fn safe_message(&self) -> String {
        match self {
            Self::InvalidInput { message, .. } | Self::Conflict { message, .. } => message.clone(),
            Self::NotFound => "This provider connection does not exist".to_owned(),
            Self::CredentialStorageUnavailable => {
                "AI provider credential storage is not configured".to_owned()
            }
            Self::CredentialUnavailable => {
                "The stored provider credential could not be opened".to_owned()
            }
            Self::ProviderFailed(category) => format!(
                "The provider request failed ({})",
                category.as_str().replace('_', " ")
            ),
            Self::Storage(_) => "AI provider settings could not be saved".to_owned(),
        }
    }
}

impl From<sqlx::Error> for ServiceError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use cp_common::{ProviderApprovalClass, ProviderExecutionEnvironmentClass};

    use super::{
        ApiKey, AuthMethod, CreateConnectionCommand, ProviderKey, RotateCredentialCommand,
        ServiceError, SetProviderDataApprovalCommand, UpdateConnectionCommand, provider_catalog,
    };

    #[test]
    fn provider_and_auth_parsing_accept_only_supported_values() {
        assert_eq!(
            ProviderKey::from_str(" OpenAI ").unwrap(),
            ProviderKey::OpenAi
        );
        assert_eq!(
            ProviderKey::from_str("openrouter").unwrap(),
            ProviderKey::OpenRouter
        );
        assert!(ProviderKey::from_str("custom").is_err());
        assert_eq!(AuthMethod::from_str("API_KEY").unwrap(), AuthMethod::ApiKey);
        assert!(AuthMethod::from_str("oauth").is_err());
    }

    #[test]
    fn commands_parse_secrets_labels_and_positive_versions_once() {
        let create = CreateConnectionCommand::parse(
            "anthropic",
            "api_key",
            "  Production  ",
            "secret-key-material-123",
        )
        .unwrap();
        assert_eq!(create.account_label.as_str(), "Production");
        assert_eq!(create.provider, ProviderKey::Anthropic);

        assert!(UpdateConnectionCommand::parse("", 1).is_err());
        assert!(UpdateConnectionCommand::parse("Valid", 0).is_err());
        assert!(RotateCredentialCommand::parse("short", 1).is_err());
        assert!(ApiKey::parse("secret key with spaces").is_err());

        let approval = SetProviderDataApprovalCommand::parse(
            "sensitive_data_approved",
            2,
            "Approve student records for this provider.",
        )
        .unwrap();
        assert_eq!(
            approval.approval_class,
            ProviderApprovalClass::SensitiveDataApproved
        );
        assert_eq!(approval.expected_approval_version, 2);
        assert!(SetProviderDataApprovalCommand::parse("local_only", 2, "Not a toggle").is_err());
        assert!(SetProviderDataApprovalCommand::parse("campus_approved", 0, "Approve").is_err());
        assert!(SetProviderDataApprovalCommand::parse("campus_approved", 1, "no").is_err());
    }

    #[test]
    fn public_catalog_is_provider_neutral_and_api_key_only() {
        assert_eq!(provider_catalog().len(), 3);
        assert!(
            provider_catalog()
                .iter()
                .all(|entry| entry.auth_methods == ["api_key"])
        );
        assert!(provider_catalog().iter().all(|entry| {
            entry.execution_environment_class == ProviderExecutionEnvironmentClass::ExternalManaged
        }));
    }

    #[test]
    fn errors_expose_stable_codes_and_safe_messages() {
        let invalid = ServiceError::invalid("bad", "Correct the value");
        assert_eq!(invalid.code(), "bad");
        assert_eq!(invalid.safe_message(), "Correct the value");
        assert_eq!(ServiceError::NotFound.code(), "connection_not_found");
    }
}
