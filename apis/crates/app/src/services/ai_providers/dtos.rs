//! Defines wire-only requests for AI-provider Administration workflows.
//!
//! Routes parse these values into proof-bearing provider service commands before
//! any storage or network side effect occurs.
//!
//! Copyright (c) 2026 Codecraft Solutions. All rights reserved.

use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct CreateConnectionRequest {
    pub provider: String,
    pub auth_method: String,
    pub account_label: String,
    pub api_key: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateConnectionRequest {
    pub account_label: String,
    pub expected_version: i64,
}

#[derive(Deserialize)]
pub(super) struct RotateCredentialRequest {
    pub api_key: String,
    pub expected_version: i64,
}

#[derive(Debug, Deserialize)]
pub(super) struct VersionedActionRequest {
    pub expected_version: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DisconnectQuery {
    pub expected_version: i64,
}
