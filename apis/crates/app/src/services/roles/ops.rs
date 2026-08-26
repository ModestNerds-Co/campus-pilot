// Copyright (c) 2025-01-02 Codecraft Solutions
// Created: 2025-01-02
// Author: AI Assistant

use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::dtos::{CreateRoleRequest, UpdateRoleRequest};
use super::models::Role;

pub struct RoleOps;

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
            WHERE name = $1 AND tenant_id = $2 AND deleted_at IS NULL
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
    ) -> Result<Role> {
        let base_key = role_key_base(&req.name);
        let suffix = Uuid::new_v4().simple().to_string();
        let key = format!("{}_{}", base_key, &suffix[..8]);

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
        .fetch_one(pool)
        .await?;

        Ok(role)
    }

    pub async fn update_role(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        req: &UpdateRoleRequest,
    ) -> Result<Option<Role>> {
        let role = sqlx::query_as!(
            Role,
            r#"
            UPDATE roles
            SET name = COALESCE($1, name),
                description = COALESCE($2, description),
                permissions = COALESCE($3, permissions),
                updated_at = NOW()
            WHERE id = $4 AND tenant_id = $5 AND deleted_at IS NULL
            RETURNING id, tenant_id, key, name, description, permissions, is_system, created_at, updated_at, deleted_at
            "#,
            req.name,
            req.description,
            req.permissions.as_deref(),
            id,
            tenant_id
        )
        .fetch_optional(pool)
        .await?;

        Ok(role)
    }

    pub async fn delete_role(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<bool> {
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

        Ok(result.rows_affected() > 0)
    }

    pub async fn role_keys_exist(
        pool: &PgPool,
        tenant_id: Uuid,
        role_keys: &[String],
    ) -> Result<bool> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM roles
            WHERE tenant_id = $1
              AND key = ANY($2)
              AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(role_keys)
        .fetch_one(pool)
        .await?;

        Ok(count == role_keys.len() as i64)
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
