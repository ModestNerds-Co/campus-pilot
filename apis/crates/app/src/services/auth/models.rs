//
//  campus-pilot-apis
//  models.rs
//
//  Created by Ngonidzashe Mangudya on 2025/10/02.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use cp_common::EffectiveRecordScope;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub email: String,
    pub full_name: String,
    pub phone: Option<String>,
    pub password_hash: String,
    pub roles: Vec<String>,
    pub is_active: bool,
    pub last_login_at: Option<DateTime<Utc>>,
    pub last_login_ip: Option<String>,
    pub failed_login_attempts: i32,
    pub locked_until: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct RefreshToken {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub token: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub phone: Option<String>,
    pub roles: Vec<String>,
    pub role_names: Vec<String>,
    pub permissions: Vec<String>,
    pub modules: Vec<String>,
    pub record_scopes: BTreeMap<String, String>,
    pub is_active: bool,
    pub last_login_at: Option<DateTime<Utc>>,
}

impl UserInfo {
    pub fn with_access(
        user: User,
        access: crate::services::access::models::EffectiveAccess,
    ) -> Self {
        let record_scopes = access
            .record_scopes
            .families()
            .filter_map(|family| {
                access
                    .record_scopes
                    .effective_scope(family)
                    .map(|scope| (family.to_string(), effective_scope_key(scope).to_owned()))
            })
            .collect();
        UserInfo {
            id: user.id,
            email: user.email,
            full_name: user.full_name,
            phone: user.phone,
            roles: user.roles,
            role_names: access.role_names,
            permissions: access.permissions,
            modules: access.enabled_modules,
            record_scopes,
            is_active: user.is_active,
            last_login_at: user.last_login_at,
        }
    }
}

const fn effective_scope_key(scope: EffectiveRecordScope) -> &'static str {
    match scope {
        EffectiveRecordScope::SelfRecord => "self",
        EffectiveRecordScope::Assigned => "assigned",
        EffectiveRecordScope::SelfAndAssigned => "self_and_assigned",
        EffectiveRecordScope::Campus => "campus",
    }
}
