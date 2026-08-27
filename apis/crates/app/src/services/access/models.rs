//
//  campus-pilot-apis
//  models.rs
//
//  Created by OpenAI Codex on 2026/08/26.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct EffectiveAccess {
    pub role_names: Vec<String>,
    pub permissions: Vec<String>,
    pub enabled_modules: Vec<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct TenantModule {
    pub module_key: String,
    pub status: String,
    pub source: String,
    pub license_expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TenantModuleResponse {
    pub key: String,
    pub status: String,
    pub source: String,
    pub enabled: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub licensed: bool,
}

impl From<TenantModule> for TenantModuleResponse {
    fn from(module: TenantModule) -> Self {
        let enabled = module.status == "enabled"
            && module
                .license_expires_at
                .is_none_or(|expires_at| expires_at > Utc::now());
        Self {
            key: module.module_key,
            status: module.status,
            source: module.source.clone(),
            enabled,
            expires_at: module.license_expires_at,
            licensed: module.source == "license",
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct LicenseInstallation {
    pub id: Uuid,
    pub deployment_id: Uuid,
    pub remote_installation_id: Option<Uuid>,
    pub control_plane_url: Option<String>,
    pub credential_ciphertext: Option<String>,
    pub credential_nonce: Option<String>,
    pub credential_hint: Option<String>,
    pub status: String,
    pub latest_lease_sequence: i64,
    pub last_refresh_attempt_at: Option<DateTime<Utc>>,
    pub last_refresh_success_at: Option<DateTime<Utc>>,
    pub last_error_code: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct LicenseLease {
    pub lease_id: Uuid,
    pub catalog_version: String,
    pub claims: Value,
    pub status: String,
    pub issued_at: DateTime<Utc>,
    pub refresh_after: DateTime<Utc>,
    pub lease_expires_at: DateTime<Utc>,
    pub grace_until: DateTime<Utc>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseLimitResponse {
    pub key: String,
    pub unit: String,
    pub period: String,
    pub value: u64,
    pub enforcement: String,
}
