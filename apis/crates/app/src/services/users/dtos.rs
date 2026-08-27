//! Defines user-administration wire contracts and response representations.
//!
//! Update phone values preserve the difference between omitted and explicit null.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::services::auth::models::User;

#[derive(Deserialize, Validate)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct UpdateUserRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: Option<String>,
    pub full_name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_present")]
    pub phone: Option<Option<String>>,
    pub roles: Option<Vec<String>>,
    pub is_active: Option<bool>,
}

fn deserialize_present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
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

#[cfg(test)]
mod tests {
    use super::UpdateUserRequest;

    #[test]
    fn update_phone_preserves_omitted_null_and_value() {
        let omitted: UpdateUserRequest = serde_json::from_str("{}").unwrap();
        let cleared: UpdateUserRequest = serde_json::from_str(r#"{"phone":null}"#).unwrap();
        let changed: UpdateUserRequest =
            serde_json::from_str(r#"{"phone":"+263 77 000 0000"}"#).unwrap();

        assert_eq!(omitted.phone, None);
        assert_eq!(cleared.phone, Some(None));
        assert_eq!(changed.phone, Some(Some("+263 77 000 0000".to_string())));
    }

    #[test]
    fn user_updates_reject_ignored_password_changes() {
        assert!(serde_json::from_str::<UpdateUserRequest>(r#"{"password":"ignored"}"#).is_err());
    }
}
