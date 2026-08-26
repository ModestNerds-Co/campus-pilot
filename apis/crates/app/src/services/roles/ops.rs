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
                SELECT id, name, description, permissions, is_system, created_at, updated_at, deleted_at
                FROM roles
                WHERE deleted_at IS NULL
                  AND (name ILIKE $1 OR description ILIKE $1)
                ORDER BY created_at DESC
                LIMIT $2 OFFSET $3
                "#,
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
                WHERE deleted_at IS NULL
                  AND (name ILIKE $1 OR description ILIKE $1)
                "#,
                search_pattern
            )
            .fetch_one(pool)
            .await?;

            (roles, total)
        } else {
            let roles = sqlx::query_as!(
                Role,
                r#"
                SELECT id, name, description, permissions, is_system, created_at, updated_at, deleted_at
                FROM roles
                WHERE deleted_at IS NULL
                ORDER BY created_at DESC
                LIMIT $1 OFFSET $2
                "#,
                limit as i64,
                offset as i64
            )
            .fetch_all(pool)
            .await?;

            let total = sqlx::query_scalar!(
                r#"
                SELECT COUNT(*) as "count!"
                FROM roles
                WHERE deleted_at IS NULL
                "#
            )
            .fetch_one(pool)
            .await?;

            (roles, total)
        };

        Ok((roles, total))
    }

    pub async fn get_role_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Role>> {
        let role = sqlx::query_as!(
            Role,
            r#"
            SELECT id, name, description, permissions, is_system, created_at, updated_at, deleted_at
            FROM roles
            WHERE id = $1 AND deleted_at IS NULL
            "#,
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(role)
    }

    pub async fn get_role_by_name(pool: &PgPool, name: &str) -> Result<Option<Role>> {
        let role = sqlx::query_as!(
            Role,
            r#"
            SELECT id, name, description, permissions, is_system, created_at, updated_at, deleted_at
            FROM roles
            WHERE name = $1 AND deleted_at IS NULL
            "#,
            name
        )
        .fetch_optional(pool)
        .await?;

        Ok(role)
    }

    pub async fn create_role(pool: &PgPool, req: &CreateRoleRequest) -> Result<Role> {
        let role = sqlx::query_as!(
            Role,
            r#"
            INSERT INTO roles (name, description, permissions, is_system)
            VALUES ($1, $2, $3, false)
            RETURNING id, name, description, permissions, is_system, created_at, updated_at, deleted_at
            "#,
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
            WHERE id = $4 AND deleted_at IS NULL AND is_system = false
            RETURNING id, name, description, permissions, is_system, created_at, updated_at, deleted_at
            "#,
            req.name,
            req.description,
            req.permissions.as_deref(),
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(role)
    }

    pub async fn delete_role(pool: &PgPool, id: Uuid) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE roles
            SET deleted_at = NOW()
            WHERE id = $1 AND deleted_at IS NULL AND is_system = false
            "#,
            id
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}
