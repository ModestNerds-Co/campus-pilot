//! Owns encrypted campus AI-provider connections and provider model discovery.
//!
//! Routing, Agent sessions, and usage are separate boundaries. Credential
//! plaintext never crosses this crate's public read models.
//!
//! Copyright (c) 2026 Codecraft Solutions. All rights reserved.

mod client;
mod crypto;
mod service;
mod types;

pub use client::{ProviderEndpoints, ProviderHttpClient};
pub use crypto::{CredentialKeyring, KeyringError};
pub use service::AiProviderOps;
pub use types::{
    AiProviderConnection, AuthMethod, ConnectionModelSnapshot, ConnectionStatus,
    ConnectionTestOutcome, ConnectionTestResult, CreateConnectionCommand, DisconnectedConnection,
    ProviderCatalogEntry, ProviderFailureCategory, ProviderKey, ProviderModel,
    RotateCredentialCommand, ServiceError, UpdateConnectionCommand, provider_catalog,
};
