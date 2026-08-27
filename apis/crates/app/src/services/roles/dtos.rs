//! Defines role-management wire contracts and response representations.
//!
//! Update descriptions preserve the difference between omitted and explicit null.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct UpdateRoleRequest {
    #[validate(length(
        min = 1,
        max = 255,
        message = "Name must be between 1 and 255 characters"
    ))]
    pub name: Option<String>,

    #[serde(default, deserialize_with = "deserialize_present")]
    pub description: Option<Option<String>>,

    pub permissions: Option<Vec<String>>,
}

fn deserialize_present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
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

#[cfg(test)]
mod tests {
    use super::UpdateRoleRequest;

    #[test]
    fn update_description_preserves_omitted_null_and_value() {
        let omitted: UpdateRoleRequest = serde_json::from_str("{}").unwrap();
        let cleared: UpdateRoleRequest = serde_json::from_str(r#"{"description":null}"#).unwrap();
        let changed: UpdateRoleRequest =
            serde_json::from_str(r#"{"description":"Heads a faculty"}"#).unwrap();

        assert_eq!(omitted.description, None);
        assert_eq!(cleared.description, Some(None));
        assert_eq!(
            changed.description,
            Some(Some("Heads a faculty".to_string()))
        );
    }

    #[test]
    fn role_updates_reject_unknown_fields() {
        assert!(serde_json::from_str::<UpdateRoleRequest>(r#"{"is_system":true}"#).is_err());
    }
}
