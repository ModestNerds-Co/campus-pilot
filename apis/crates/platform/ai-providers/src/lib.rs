//! Owns encrypted campus AI-provider connections and provider model discovery.
//!
//! Routing, Agent sessions, and usage are separate boundaries. Credential
//! plaintext never crosses this crate's public read models.
//!
//! Copyright (c) 2026 Codecraft Solutions. All rights reserved.

mod client;
mod crypto;
mod execution;
mod execution_binding;
mod execution_client;
mod execution_types;
mod service;
mod types;

pub use client::{ProviderEndpoints, ProviderHttpClient};
pub use crypto::{CredentialKeyring, KeyringError};
pub use execution::PreparedProviderExecution;
pub use execution_types::{
    ExecuteProviderCommand, ProviderExecutionError, ProviderExecutionFailure,
    ProviderExecutionResponse, ProviderExecutionTarget, ProviderMessage, ProviderToolCall,
    ProviderToolDefinition, ProviderUsage,
};
pub use service::AiProviderOps;
pub use types::{
    AiProviderConnection, AuthMethod, ConnectionModelSnapshot, ConnectionStatus,
    ConnectionTestOutcome, ConnectionTestResult, CreateConnectionCommand, DisconnectedConnection,
    ProviderCatalogEntry, ProviderDataApproval, ProviderFailureCategory, ProviderKey,
    ProviderModel, RotateCredentialCommand, ServiceError, SetProviderDataApprovalCommand,
    UpdateConnectionCommand, provider_catalog,
};
