//
//  campus-pilot-apis
//  models.rs
//
//  Created by OpenAI Codex on 2026/08/26.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use chrono::{DateTime, Utc};
use serde::Serialize;
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
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub module_key: String,
    pub status: String,
    pub source: String,
    pub license_fingerprint: Option<String>,
    pub license_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
