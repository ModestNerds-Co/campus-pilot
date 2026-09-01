//! Persists tenant-scoped role definitions and resolves assignment authority.
//!
//! Role keys are immutable assignment identifiers. Built-in permission
//! baselines are fixed; dynamic custom roles remain editable and may be deleted
//! only while unassigned.

use std::collections::BTreeSet;

use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::services::access::record_scopes::{RoleRecordScopeAssignment, RoleRecordScopeOps};

use super::dtos::{CreateRoleRequest, UpdateRoleRequest};
use super::models::Role;

pub struct RoleOps;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteRoleOutcome {
    Deleted,
    NotFound,
    SystemRole,
    Assigned,
}

impl RoleOps {
    pub async fn list_roles(
        pool: &PgPool,
        tenant_id: Uuid,
        page: u32,
        limit: u32,
        query: Option<&str>,
    ) -> Result<(Vec<Role>, i64)> {
        let offset = (page - 1) * limit;

        let (roles, total) = if let Some(search) = query {
            let search_pattern = format!("%{}%", search);
            let roles = sqlx::query_as!(
                Role,
                r#"
                SELECT id, tenant_id, key, name, description, permissions, is_system, created_at, updated_at, deleted_at
                FROM roles
                WHERE tenant_id = $1 AND deleted_at IS NULL
                  AND (name ILIKE $2 OR description ILIKE $2)
                ORDER BY created_at DESC
                LIMIT $3 OFFSET $4
                "#,
                tenant_id,
                search_pattern,
                limit as i64,
                offset as i64
            )
            .fetch_all(pool)
            .await?;

            let total = sqlx::query_scalar!(
                r#"
                SELECT COUNT(*) as "count!"
                FROM roles
                WHERE tenant_id = $1 AND deleted_at IS NULL
                  AND (name ILIKE $2 OR description ILIKE $2)
                "#,
                tenant_id,
                search_pattern
            )
            .fetch_one(pool)
            .await?;

            (roles, total)
        } else {
            let roles = sqlx::query_as!(
                Role,
                r#"
                SELECT id, tenant_id, key, name, description, permissions, is_system, created_at, updated_at, deleted_at
                FROM roles
                WHERE tenant_id = $1 AND deleted_at IS NULL
                ORDER BY created_at DESC
                LIMIT $2 OFFSET $3
                "#,
                tenant_id,
                limit as i64,
                offset as i64
            )
            .fetch_all(pool)
            .await?;

            let total = sqlx::query_scalar!(
                r#"
                SELECT COUNT(*) as "count!"
                FROM roles
                WHERE tenant_id = $1 AND deleted_at IS NULL
                "#,
                tenant_id
            )
            .fetch_one(pool)
            .await?;

            (roles, total)
        };

        Ok((roles, total))
    }

    pub async fn get_role_by_id(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<Option<Role>> {
        let role = sqlx::query_as!(
            Role,
            r#"
            SELECT id, tenant_id, key, name, description, permissions, is_system, created_at, updated_at, deleted_at
            FROM roles
            WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
            "#,
            id,
            tenant_id
        )
        .fetch_optional(pool)
        .await?;

        Ok(role)
    }

    pub async fn get_role_by_name(
        pool: &PgPool,
        tenant_id: Uuid,
        name: &str,
    ) -> Result<Option<Role>> {
        let role = sqlx::query_as!(
            Role,
            r#"
            SELECT id, tenant_id, key, name, description, permissions, is_system, created_at, updated_at, deleted_at
            FROM roles
            WHERE LOWER(name) = LOWER($1) AND tenant_id = $2 AND deleted_at IS NULL
            "#,
            name,
            tenant_id
        )
        .fetch_optional(pool)
        .await?;

        Ok(role)
    }

    pub async fn create_role(
        pool: &PgPool,
        tenant_id: Uuid,
        req: &CreateRoleRequest,
        record_scopes: &[RoleRecordScopeAssignment],
    ) -> Result<Role> {
        let base_key = role_key_base(&req.name);
        let suffix = Uuid::new_v4().simple().to_string();
        let key = format!("{}_{}", base_key, &suffix[..8]);

        let mut transaction = pool.begin().await?;
        let role = sqlx::query_as!(
            Role,
            r#"
            INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
            VALUES ($1, $2, $3, $4, $5, FALSE)
            RETURNING id, tenant_id, key, name, description, permissions, is_system, created_at, updated_at, deleted_at
            "#,
            tenant_id,
            key,
            req.name,
            req.description,
            &req.permissions
        )
        .fetch_one(&mut *transaction)
        .await?;

        RoleRecordScopeOps::replace_for_role(&mut transaction, tenant_id, role.id, record_scopes)
            .await?;
        transaction.commit().await?;

        Ok(role)
    }

    pub async fn update_role(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        req: &UpdateRoleRequest,
        record_scopes: Option<&[RoleRecordScopeAssignment]>,
    ) -> Result<Option<Role>> {
        let mut transaction = pool.begin().await?;
        let role = sqlx::query_as!(
            Role,
            r#"
            UPDATE roles
            SET name = COALESCE($1, name),
                description = CASE WHEN $2 THEN $3 ELSE description END,
                permissions = COALESCE($4, permissions),
                updated_at = NOW()
            WHERE id = $5 AND tenant_id = $6 AND deleted_at IS NULL
            RETURNING id, tenant_id, key, name, description, permissions, is_system, created_at, updated_at, deleted_at
            "#,
            req.name,
            req.description.is_some(),
            req.description.as_ref().and_then(|value| value.as_deref()),
            req.permissions.as_deref(),
            id,
            tenant_id
        )
        .fetch_optional(&mut *transaction)
        .await?;

        if let (Some(role), Some(assignments)) = (role.as_ref(), record_scopes) {
            RoleRecordScopeOps::replace_for_role(&mut transaction, tenant_id, role.id, assignments)
                .await?;
        }
        transaction.commit().await?;

        Ok(role)
    }

    pub async fn delete_role(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<DeleteRoleOutcome> {
        let role = Self::get_role_by_id(pool, tenant_id, id).await?;
        let Some(role) = role else {
            return Ok(DeleteRoleOutcome::NotFound);
        };
        if role.is_system {
            return Ok(DeleteRoleOutcome::SystemRole);
        }

        let assigned = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM users
                WHERE tenant_id = $1
                  AND $2 = ANY(roles)
                  AND deleted_at IS NULL
            )
            "#,
        )
        .bind(tenant_id)
        .bind(&role.key)
        .fetch_one(pool)
        .await?;
        if assigned {
            return Ok(DeleteRoleOutcome::Assigned);
        }

        let result = sqlx::query!(
            r#"
            UPDATE roles
            SET deleted_at = NOW()
            WHERE id = $1
              AND tenant_id = $2
              AND deleted_at IS NULL
              AND is_system = FALSE
              AND NOT EXISTS (
                  SELECT 1
                  FROM users
                  WHERE users.tenant_id = $2
                    AND roles.key = ANY(users.roles)
                    AND users.deleted_at IS NULL
              )
            "#,
            id,
            tenant_id
        )
        .execute(pool)
        .await?;

        Ok(if result.rows_affected() == 1 {
            DeleteRoleOutcome::Deleted
        } else {
            DeleteRoleOutcome::Assigned
        })
    }

    pub async fn assignment_permissions(
        pool: &PgPool,
        tenant_id: Uuid,
        role_keys: &[String],
    ) -> Result<Option<Vec<String>>> {
        let requested: BTreeSet<String> = role_keys.iter().cloned().collect();
        if requested.is_empty() {
            return Ok(Some(Vec::new()));
        }
        let requested_keys: Vec<String> = requested.iter().cloned().collect();
        let roles = sqlx::query_as::<_, Role>(
            r#"
            SELECT id, tenant_id, key, name, description, permissions, is_system,
                   created_at, updated_at, deleted_at
            FROM roles
            WHERE tenant_id = $1
              AND key = ANY($2)
              AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(&requested_keys)
        .fetch_all(pool)
        .await?;
        if roles.len() != requested.len() {
            return Ok(None);
        }
        let permissions = roles
            .into_iter()
            .flat_map(|role| role.permissions)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok(Some(permissions))
    }
}

fn role_key_base(name: &str) -> String {
    let mut key = String::new();
    let mut last_was_separator = false;

    for character in name.trim().to_lowercase().chars() {
        if character.is_ascii_alphanumeric() {
            key.push(character);
            last_was_separator = false;
        } else if !last_was_separator && !key.is_empty() {
            key.push('_');
            last_was_separator = true;
        }
    }

    let trimmed = key.trim_matches('_');
    if trimmed.is_empty() {
        "custom_role".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::role_key_base;

    #[test]
    fn custom_role_keys_are_stable_slugs() {
        assert_eq!(role_key_base(" Head of Department "), "head_of_department");
        assert_eq!(role_key_base("---"), "custom_role");
    }
}
