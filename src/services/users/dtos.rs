//
//  campus-pilot-apis
//  dtos.rs
//
//  Created by Ngonidzashe Mangudya on 2025/10/02.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::services::auth::models::User;

#[derive(Deserialize, Validate)]
pub struct CreateUserRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
    #[validate(length(min = 1, message = "Full name is required"))]
    pub full_name: String,
    #[validate(length(min = 10, message = "Password must be at least 10 characters"))]
    pub password: String,
    pub phone: Option<String>,
    #[validate(length(min = 1, message = "At least one role is required"))]
    pub roles: Vec<String>,
    pub is_active: Option<bool>,
}

#[derive(Deserialize, Validate)]
pub struct UpdateUserRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: Option<String>,
    pub full_name: Option<String>,
    pub phone: Option<String>,
    pub roles: Option<Vec<String>>,
    pub is_active: Option<bool>,
}

#[derive(Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub phone: Option<String>,
    pub roles: Vec<String>,
    pub is_active: bool,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct PaginatedUsersResponse {
    pub users: Vec<UserResponse>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
    pub total_pages: i64,
}

#[derive(Deserialize)]
pub struct ListUsersQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>, // "active" or "inactive"
    pub sort: Option<String>,   // "created_at", "updated_at", "email"
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            full_name: user.full_name,
            phone: user.phone,
            roles: user.roles,
            is_active: user.is_active,
            last_login_at: user.last_login_at,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}
