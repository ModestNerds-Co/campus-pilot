//
//  campus-pilot-apis
//  dtos.rs
//
//  Created by OpenAI Codex on 2026/08/26.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use serde::{Deserialize, Serialize};
use validator::Validate;

use super::{catalog::ModuleDefinition, models::TenantModuleResponse};

#[derive(Debug, Serialize)]
pub struct ModuleCatalogResponse {
    pub modules: Vec<ModuleDefinition>,
    pub administration_permissions: Vec<super::catalog::PermissionDefinition>,
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
