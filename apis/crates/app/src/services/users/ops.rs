//
//  campus-pilot-apis
//  ops.rs
//
//  Created by Ngonidzashe Mangudya on 2025/10/02.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use anyhow::{Context, Result as ApiResult};
use sqlx::PgPool;
use uuid::Uuid;

use crate::services::auth::models::User;

pub struct UserOps;

impl UserOps {
    /// List users with pagination and filtering, scoped to a tenant
    #[expect(
        clippy::too_many_arguments,
        reason = "the query boundary keeps independently optional filters explicit"
    )]
    pub async fn list_users(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        role: Option<&str>,
        status: Option<&str>,
        sort: Option<&str>,
    ) -> ApiResult<(Vec<User>, i64)> {
        let offset = (page - 1) * per_page;
        let sort_column = match sort {
            Some("email") => "email",
            Some("updated_at") => "updated_at",
            _ => "created_at",
        };

        // tenant_id is always $1; remaining filters get the next free placeholder,
        // bound in the same order below, so param numbering and bind order must stay in sync.
        let mut where_clause = "tenant_id = $1 AND deleted_at IS NULL".to_string();
        let mut next_param = 2;

        let search_param = search.map(|_| {
            let p = next_param;
            next_param += 1;
            p
        });
        if let Some(p) = search_param {
            where_clause.push_str(&format!(" AND (email ILIKE ${p} OR full_name ILIKE ${p})"));
        }

        let role_param = role.map(|_| {
            let p = next_param;
            next_param += 1;
            p
        });
        if let Some(p) = role_param {
            where_clause.push_str(&format!(" AND ${p} = ANY(roles)"));
        }

        let status_param = status.map(|_| {
            let p = next_param;
            next_param += 1;
            p
        });
        if let Some(p) = status_param {
            where_clause.push_str(&format!(" AND is_active = ${p}"));
        }

        let limit_param = next_param;
        let offset_param = next_param + 1;

        let query = format!(
            r#"
            SELECT id, tenant_id, email, full_name, phone, password_hash, roles, is_active,
                   last_login_at, last_login_ip, failed_login_attempts,
                   locked_until, created_at, updated_at, deleted_at
            FROM users
            WHERE {where_clause}
            ORDER BY {sort_column} DESC
            LIMIT ${limit_param} OFFSET ${offset_param}
            "#
        );

        let mut builder = sqlx::query_as::<_, User>(&query).bind(tenant_id);
        if let Some(term) = search {
            builder = builder.bind(format!("%{}%", term));
        }
        if let Some(r) = role {
            builder = builder.bind(r);
        }
        if let Some(s) = status {
            builder = builder.bind(s == "active");
        }
        builder = builder.bind(per_page).bind(offset);

        let users = builder
            .fetch_all(pool)
            .await
            .context("Failed to fetch users")?;

        // Count query mirrors the same WHERE clause, minus LIMIT/OFFSET.
        let count_query = format!("SELECT COUNT(*) FROM users WHERE {where_clause}");
        let mut count_builder = sqlx::query_as::<_, (i64,)>(&count_query).bind(tenant_id);
        if let Some(term) = search {
            count_builder = count_builder.bind(format!("%{}%", term));
        }
        if let Some(r) = role {
            count_builder = count_builder.bind(r);
        }
        if let Some(s) = status {
            count_builder = count_builder.bind(s == "active");
        }

        let total: (i64,) = count_builder
            .fetch_one(pool)
            .await
            .context("Failed to count users")?;

        Ok((users, total.0))
    }

    /// Get user by ID, scoped to a tenant
    pub async fn get_user_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> ApiResult<Option<User>> {
        let user = sqlx::query_as!(
            User,
            r#"
            SELECT id, tenant_id, email, full_name, phone, password_hash, roles, is_active,
                   last_login_at, last_login_ip, failed_login_attempts,
                   locked_until, created_at, updated_at, deleted_at
            FROM users
            WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
            "#,
            user_id,
            tenant_id
        )
        .fetch_optional(pool)
        .await
        .context("Failed to fetch user")?;

        Ok(user)
    }

    /// Create a new user within a tenant
    #[expect(
        clippy::too_many_arguments,
        reason = "the service boundary mirrors the validated user creation fields"
    )]
    pub async fn create_user(
        pool: &PgPool,
        tenant_id: Uuid,
        email: &str,
        full_name: &str,
        password_hash: &str,
        phone: Option<&str>,
        roles: Vec<String>,
        is_active: bool,
    ) -> ApiResult<User> {
        let user = sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (tenant_id, email, full_name, password_hash, phone, roles, is_active)
            VALUES ($1, LOWER($2), $3, $4, $5, $6, $7)
            RETURNING id, tenant_id, email, full_name, phone, password_hash, roles, is_active,
                      last_login_at, last_login_ip, failed_login_attempts,
                      locked_until, created_at, updated_at, deleted_at
            "#,
            tenant_id,
            email,
            full_name,
            password_hash,
            phone,
            &roles,
            is_active
        )
        .fetch_one(pool)
        .await
        .context("Failed to create user")?;

        Ok(user)
    }

    /// Update user, scoped to a tenant
    #[expect(
        clippy::too_many_arguments,
        reason = "the service boundary mirrors the independently optional user update fields"
    )]
    pub async fn update_user(
        pool: &PgPool,
        tenant_id: Uuid,
        user_id: Uuid,
        email: Option<&str>,
        full_name: Option<&str>,
        phone: Option<Option<&str>>,
        roles: Option<Vec<String>>,
        is_active: Option<bool>,
    ) -> ApiResult<User> {
        // Build dynamic update query. Column names below are a fixed, hardcoded set —
        // only the placeholder *numbers* are interpolated, values are always bound.
        let mut updates = Vec::new();
        let mut param_count = 1;

        if email.is_some() {
            updates.push(format!("email = LOWER(${})", param_count));
            param_count += 1;
        }
        if full_name.is_some() {
            updates.push(format!("full_name = ${}", param_count));
            param_count += 1;
        }
        if phone.is_some() {
            updates.push(format!("phone = ${}", param_count));
            param_count += 1;
        }
        if roles.is_some() {
            updates.push(format!("roles = ${}", param_count));
            param_count += 1;
        }
        if is_active.is_some() {
            updates.push(format!("is_active = ${}", param_count));
            param_count += 1;
        }

        if updates.is_empty() {
            return Self::get_user_by_id(pool, tenant_id, user_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("User not found"));
        }

        let id_param = param_count;
        let tenant_param = param_count + 1;

        let query = format!(
            r#"
            UPDATE users
            SET {}
            WHERE id = ${} AND tenant_id = ${} AND deleted_at IS NULL
            RETURNING id, tenant_id, email, full_name, phone, password_hash, roles, is_active,
                      last_login_at, last_login_ip, failed_login_attempts,
                      locked_until, created_at, updated_at, deleted_at
            "#,
            updates.join(", "),
            id_param,
            tenant_param
        );

        let mut query_builder = sqlx::query_as::<_, User>(&query);

        if let Some(e) = email {
            query_builder = query_builder.bind(e);
        }
        if let Some(f) = full_name {
            query_builder = query_builder.bind(f);
        }
        if let Some(p) = phone {
            query_builder = query_builder.bind(p);
        }
        if let Some(r) = roles {
            query_builder = query_builder.bind(r);
        }
        if let Some(a) = is_active {
            query_builder = query_builder.bind(a);
        }
        query_builder = query_builder.bind(user_id).bind(tenant_id);

        let user = query_builder
            .fetch_one(pool)
            .await
            .context("Failed to update user")?;

        Ok(user)
    }

    /// Soft delete user, scoped to a tenant
    pub async fn delete_user(pool: &PgPool, tenant_id: Uuid, user_id: Uuid) -> ApiResult<()> {
        sqlx::query!(
            r#"
            UPDATE users
            SET deleted_at = NOW()
            WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
            "#,
            user_id,
            tenant_id
        )
        .execute(pool)
        .await
        .context("Failed to delete user")?;

        Ok(())
    }

    /// Check if email exists within a tenant (excluding a specific user ID)
    pub async fn email_exists(
        pool: &PgPool,
        tenant_id: Uuid,
        email: &str,
        exclude_user_id: Option<Uuid>,
    ) -> ApiResult<bool> {
        let exists = if let Some(user_id) = exclude_user_id {
            sqlx::query_scalar!(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM users
                    WHERE tenant_id = $1
                    AND LOWER(email) = LOWER($2)
                    AND id != $3
                    AND deleted_at IS NULL
                ) as "exists!"
                "#,
                tenant_id,
                email,
                user_id
            )
            .fetch_one(pool)
            .await
            .context("Failed to check email existence")?
        } else {
            sqlx::query_scalar!(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM users
                    WHERE tenant_id = $1
                    AND LOWER(email) = LOWER($2)
                    AND deleted_at IS NULL
                ) as "exists!"
                "#,
                tenant_id,
                email
            )
            .fetch_one(pool)
            .await
            .context("Failed to check email existence")?
        };

        Ok(exists)
    }

    /// Activate user, scoped to a tenant
    pub async fn activate_user(pool: &PgPool, tenant_id: Uuid, user_id: Uuid) -> ApiResult<User> {
        let user = sqlx::query_as!(
            User,
            r#"
            UPDATE users
            SET is_active = TRUE
            WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
            RETURNING id, tenant_id, email, full_name, phone, password_hash, roles, is_active,
                      last_login_at, last_login_ip, failed_login_attempts,
                      locked_until, created_at, updated_at, deleted_at
            "#,
            user_id,
            tenant_id
        )
        .fetch_one(pool)
        .await
        .context("Failed to activate user")?;

        Ok(user)
    }

    /// Deactivate user, scoped to a tenant
    pub async fn deactivate_user(pool: &PgPool, tenant_id: Uuid, user_id: Uuid) -> ApiResult<User> {
        let user = sqlx::query_as!(
            User,
            r#"
            UPDATE users
            SET is_active = FALSE
            WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
            RETURNING id, tenant_id, email, full_name, phone, password_hash, roles, is_active,
                      last_login_at, last_login_ip, failed_login_attempts,
                      locked_until, created_at, updated_at, deleted_at
            "#,
            user_id,
            tenant_id
        )
        .fetch_one(pool)
        .await
        .context("Failed to deactivate user")?;

        Ok(user)
    }
}
