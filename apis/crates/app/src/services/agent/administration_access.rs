//! Adapts tenant-scoped role and user reads to typed Agent capabilities.

use async_trait::async_trait;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::PaginationMeta;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::services::{
    access::record_scopes::RoleRecordScopeOps,
    roles::{
        dtos::{ListRolesResponse, RoleResponse},
        ops::RoleOps,
    },
    users::{
        dtos::{PaginatedUsersResponse, UserResponse},
        ops::UserOps,
    },
};

use super::administration::read_descriptor;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListRolesInput {
    page: Option<u32>,
    limit: Option<u32>,
    query: Option<String>,
}

#[derive(Serialize)]
pub(super) struct ListRolesOutput {
    roles: ListRolesResponse,
    pagination: PaginationMeta,
}

pub(super) struct AdministrationRolesListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl AdministrationRolesListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "administration.roles.list",
                "List roles",
                "Returns tenant roles using bounded pagination and optional search.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "limit": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "query": { "type": ["string", "null"] }
                }),
                json!({
                    "roles": { "type": "object" },
                    "pagination": { "type": "object" }
                }),
                DataSensitivity::General,
                "administration.roles",
            ),
        }
    }
}

#[async_trait]
impl Capability for AdministrationRolesListCapability {
    type Input = ListRolesInput;
    type Output = ListRolesOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, _input: &Self::Input) -> CapabilityScope {
        CapabilityScope::TenantWide
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let page = input.page.unwrap_or(1).max(1);
        let limit = input.limit.unwrap_or(50).clamp(1, 100);
        let query = trimmed(input.query.as_deref());
        let (roles, total) = RoleOps::list_roles(
            &self.pool,
            context.principal().tenant_id(),
            page,
            limit,
            query,
        )
        .await
        .map_err(|_| dependency_failure("Roles could not be loaded."))?;
        let role_ids = roles.iter().map(|role| role.id).collect::<Vec<_>>();
        let mut scopes = RoleRecordScopeOps::for_role_ids(
            &self.pool,
            context.principal().tenant_id(),
            &role_ids,
        )
        .await
        .map_err(|_| dependency_failure("Role visibility could not be loaded."))?;
        Ok(ListRolesOutput {
            roles: ListRolesResponse {
                roles: roles
                    .into_iter()
                    .map(|role| {
                        let role_scopes = scopes.remove(&role.id).unwrap_or_default();
                        RoleResponse::from_role(role, role_scopes)
                    })
                    .collect(),
            },
            pagination: PaginationMeta::new(page, limit, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadRoleInput {
    role_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct ReadRoleOutput {
    role: RoleResponse,
}

pub(super) struct AdministrationRoleReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl AdministrationRoleReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "administration.roles.read",
                "Read role",
                "Returns one tenant role by its stable identifier.",
                json!({
                    "role_id": { "type": "string", "format": "uuid" }
                }),
                json!({ "role": { "type": "object" } }),
                DataSensitivity::General,
                "administration.roles",
            ),
        }
    }
}

#[async_trait]
impl Capability for AdministrationRoleReadCapability {
    type Input = ReadRoleInput;
    type Output = ReadRoleOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("role", input.role_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let role =
            RoleOps::get_role_by_id(&self.pool, context.principal().tenant_id(), input.role_id)
                .await
                .map_err(|_| dependency_failure("The role could not be loaded."))?
                .ok_or_else(|| not_found("The role was not found."))?;
        let mut scopes = RoleRecordScopeOps::for_role_ids(
            &self.pool,
            context.principal().tenant_id(),
            &[role.id],
        )
        .await
        .map_err(|_| dependency_failure("Role visibility could not be loaded."))?;
        let role_scopes = scopes.remove(&role.id).unwrap_or_default();
        Ok(ReadRoleOutput {
            role: RoleResponse::from_role(role, role_scopes),
        })
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum UserStatus {
    Active,
    Inactive,
}

impl UserStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum UserSort {
    CreatedAt,
    UpdatedAt,
    Email,
}

impl UserSort {
    const fn as_str(self) -> &'static str {
        match self {
            Self::CreatedAt => "created_at",
            Self::UpdatedAt => "updated_at",
            Self::Email => "email",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListUsersInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    role: Option<String>,
    status: Option<UserStatus>,
    sort: Option<UserSort>,
}

#[derive(Serialize)]
pub(super) struct ListUsersOutput {
    users: PaginatedUsersResponse,
    pagination: PaginationMeta,
}

pub(super) struct AdministrationUsersListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl AdministrationUsersListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "administration.users.list",
                "List users",
                "Returns tenant user accounts using bounded filters and pagination.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "search": { "type": ["string", "null"] },
                    "role": { "type": ["string", "null"] },
                    "status": { "type": ["string", "null"], "enum": ["active", "inactive", null] },
                    "sort": { "type": ["string", "null"], "enum": ["created_at", "updated_at", "email", null] }
                }),
                json!({
                    "users": { "type": "object" },
                    "pagination": { "type": "object" }
                }),
                DataSensitivity::Personal,
                "administration.users",
            ),
        }
    }
}

#[async_trait]
impl Capability for AdministrationUsersListCapability {
    type Input = ListUsersInput;
    type Output = ListUsersOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, _input: &Self::Input) -> CapabilityScope {
        CapabilityScope::TenantWide
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let page = input.page.unwrap_or(1).max(1);
        let per_page = input.per_page.unwrap_or(20).clamp(1, 100);
        let search = trimmed(input.search.as_deref());
        let role = trimmed(input.role.as_deref());
        let status = input.status.map(UserStatus::as_str);
        let sort = input.sort.map(UserSort::as_str);
        let (users, total) = UserOps::list_users(
            &self.pool,
            context.principal().tenant_id(),
            page,
            per_page,
            search,
            role,
            status,
            sort,
        )
        .await
        .map_err(|_| dependency_failure("Users could not be loaded."))?;
        Ok(ListUsersOutput {
            users: PaginatedUsersResponse {
                users: users.into_iter().map(UserResponse::from).collect(),
            },
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadUserInput {
    account_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct ReadUserOutput {
    user: UserResponse,
}

pub(super) struct AdministrationUserReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl AdministrationUserReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "administration.users.read",
                "Read user",
                "Returns one tenant user account by its stable identifier.",
                json!({
                    "account_id": { "type": "string", "format": "uuid" }
                }),
                json!({ "user": { "type": "object" } }),
                DataSensitivity::Personal,
                "administration.users",
            ),
        }
    }
}

#[async_trait]
impl Capability for AdministrationUserReadCapability {
    type Input = ReadUserInput;
    type Output = ReadUserOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("user", input.account_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let user = UserOps::get_user_by_id(
            &self.pool,
            context.principal().tenant_id(),
            input.account_id,
        )
        .await
        .map_err(|_| dependency_failure("The user could not be loaded."))?
        .ok_or_else(|| not_found("The user was not found."))?;
        Ok(ReadUserOutput {
            user: UserResponse::from(user),
        })
    }
}

fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn resource_scope(kind: &str, id: Uuid) -> CapabilityScope {
    CapabilityScope::resources([CapabilityResource::parse(kind, id.to_string())
        .unwrap_or_else(|error| panic!("invalid built-in capability resource: {error}"))])
    .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
}

fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}

fn not_found(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::InvalidState, message)
}

#[cfg(test)]
mod tests {
    use cp_agent::CapabilityExecutionErrorCode;

    use super::{UserSort, UserStatus, not_found};

    #[test]
    fn typed_filters_and_not_found_errors_have_stable_values() {
        assert_eq!(UserStatus::Inactive.as_str(), "inactive");
        assert_eq!(UserSort::CreatedAt.as_str(), "created_at");
        assert_eq!(UserSort::UpdatedAt.as_str(), "updated_at");
        let error = not_found("The record was not found.");
        assert_eq!(error.code(), CapabilityExecutionErrorCode::InvalidState);
        assert_eq!(error.safe_message(), "The record was not found.");
    }
}
