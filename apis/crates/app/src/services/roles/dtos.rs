// Copyright (c) 2025-01-02 Codecraft Solutions
// Created: 2025-01-02
// Author: AI Assistant

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateRoleRequest {
    #[validate(length(
        min = 1,
        max = 255,
        message = "Name must be between 1 and 255 characters"
    ))]
    pub name: String,

    #[validate(length(max = 1000, message = "Description must not exceed 1000 characters"))]
    pub description: Option<String>,

    pub permissions: Vec<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateRoleRequest {
    #[validate(length(
        min = 1,
        max = 255,
        message = "Name must be between 1 and 255 characters"
    ))]
    pub name: Option<String>,

    #[validate(length(max = 1000, message = "Description must not exceed 1000 characters"))]
    pub description: Option<String>,

    pub permissions: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct RoleResponse {
    pub id: Uuid,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub permissions: Vec<String>,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ListRolesQuery {
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub query: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListRolesResponse {
    pub roles: Vec<RoleResponse>,
}

impl From<super::models::Role> for RoleResponse {
    fn from(role: super::models::Role) -> Self {
        Self {
            id: role.id,
            key: role.key,
            name: role.name,
            description: role.description,
            permissions: role.permissions,
            is_system: role.is_system,
            created_at: role.created_at,
            updated_at: role.updated_at,
        }
    }
}
