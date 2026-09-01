//
//  campus-pilot-apis
//  dtos.rs
//
//  Created by OpenAI Codex on 2026/08/26.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

use super::{
    catalog::ModuleDefinition,
    models::{LicenseLimitResponse, TenantModuleResponse},
    record_scopes::RecordScopeFamilyCatalogItem,
};

#[derive(Debug, Serialize)]
pub struct ModuleCatalogResponse {
    pub modules: Vec<ModuleDefinition>,
    pub administration_permissions: Vec<super::catalog::PermissionDefinition>,
    pub record_scope_families: Vec<RecordScopeFamilyCatalogItem>,
}

#[derive(Debug, Serialize)]
pub struct TenantModulesResponse {
    pub modules: Vec<TenantModuleResponse>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ActivateLicenseRequest {
    #[validate(length(min = 20, message = "License key is incomplete"))]
    pub license_key: String,
}

#[derive(Debug, Serialize)]
pub struct ActivateLicenseResponse {
    pub activated_modules: Vec<String>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ConnectLicenseRequest {
    #[validate(length(min = 12, max = 200, message = "Activation code is incomplete"))]
    pub activation_code: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ImportLeaseRequest {
    #[validate(length(min = 20, message = "License bundle is incomplete"))]
    pub bundle: String,
}

#[derive(Debug, Serialize)]
pub struct LicensingStateResponse {
    pub configured: bool,
    pub connected: bool,
    pub status: String,
    pub deployment_id: String,
    pub installation_id: Option<String>,
    pub credential_hint: Option<String>,
    pub portal_url: Option<String>,
    pub latest_sequence: i64,
    pub last_refresh_attempt_at: Option<DateTime<Utc>>,
    pub last_refresh_success_at: Option<DateTime<Utc>>,
    pub last_error_code: Option<String>,
    pub lease: Option<LeaseStateResponse>,
}

#[derive(Debug, Serialize)]
pub struct LeaseStateResponse {
    pub id: String,
    pub status: String,
    pub source: String,
    pub catalog_version: String,
    pub issued_at: DateTime<Utc>,
    pub refresh_after: DateTime<Utc>,
    pub lease_expires_at: DateTime<Utc>,
    pub grace_until: DateTime<Utc>,
    pub modules: Vec<String>,
    pub features: Vec<String>,
    pub limits: Vec<LicenseLimitResponse>,
}
