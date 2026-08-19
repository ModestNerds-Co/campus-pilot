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
    /// List users with pagination and filtering
    pub async fn list_users(
        pool: &PgPool,
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

        let mut query = format!(
            r#"
            SELECT id, email, full_name, phone, password_hash, roles, is_active,
                   last_login_at, last_login_ip, failed_login_attempts,
                   locked_until, created_at, updated_at, deleted_at
            FROM users
            WHERE deleted_at IS NULL
            "#
        );

        let mut conditions = Vec::new();

        if let Some(search_term) = search {
            conditions.push(format!(
                "(email ILIKE '%{}%' OR full_name ILIKE '%{}%')",
                search_term, search_term
            ));
        }

        if let Some(role_filter) = role {
            conditions.push(format!("'{}' = ANY(roles)", role_filter));
        }

        if let Some(status_filter) = status {
            let is_active = status_filter == "active";
            conditions.push(format!("is_active = {}", is_active));
        }

        if !conditions.is_empty() {
            query.push_str(" AND ");
            query.push_str(&conditions.join(" AND "));
        }

        query.push_str(&format!(" ORDER BY {} DESC", sort_column));
        query.push_str(&format!(" LIMIT {} OFFSET {}", per_page, offset));

        let users = sqlx::query_as::<_, User>(&query)
            .fetch_all(pool)
            .await
            .context("Failed to fetch users")?;

        // Get total count
        let mut count_query = "SELECT COUNT(*) FROM users WHERE deleted_at IS NULL".to_string();
        if !conditions.is_empty() {
            count_query.push_str(" AND ");
            count_query.push_str(&conditions.join(" AND "));
        }

        let total: (i64,) = sqlx::query_as(&count_query)
            .fetch_one(pool)
            .await
            .context("Failed to count users")?;

        Ok((users, total.0))
    }

    /// Get user by ID
    pub async fn get_user_by_id(pool: &PgPool, user_id: Uuid) -> ApiResult<Option<User>> {
        let user = sqlx::query_as!(
            User,
            r#"
            SELECT id, email, full_name, phone, password_hash, roles, is_active,
                   last_login_at, last_login_ip, failed_login_attempts,
                   locked_until, created_at, updated_at, deleted_at
            FROM users
            WHERE id = $1 AND deleted_at IS NULL
            "#,
            user_id
        )
        .fetch_optional(pool)
        .await
        .context("Failed to fetch user")?;

        Ok(user)
    }

    /// Create a new user
    pub async fn create_user(
        pool: &PgPool,
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
            INSERT INTO users (email, full_name, password_hash, phone, roles, is_active)
            VALUES (LOWER($1), $2, $3, $4, $5, $6)
            RETURNING id, email, full_name, phone, password_hash, roles, is_active,
                      last_login_at, last_login_ip, failed_login_attempts,
                      locked_until, created_at, updated_at, deleted_at
            "#,
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

    /// Update user
    pub async fn update_user(
        pool: &PgPool,
        user_id: Uuid,
        email: Option<&str>,
        full_name: Option<&str>,
        phone: Option<&str>,
        roles: Option<Vec<String>>,
        is_active: Option<bool>,
    ) -> ApiResult<User> {
        // Build dynamic update query
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
            return Self::get_user_by_id(pool, user_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("User not found"));
        }

        let query = format!(
            r#"
            UPDATE users
            SET {}
            WHERE id = ${} AND deleted_at IS NULL
            RETURNING id, email, full_name, phone, password_hash, roles, is_active,
                      last_login_at, last_login_ip, failed_login_attempts,
                      locked_until, created_at, updated_at, deleted_at
            "#,
            updates.join(", "),
            param_count
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
        query_builder = query_builder.bind(user_id);

        let user = query_builder
            .fetch_one(pool)
            .await
            .context("Failed to update user")?;

        Ok(user)
    }

    /// Soft delete user
    pub async fn delete_user(pool: &PgPool, user_id: Uuid) -> ApiResult<()> {
        sqlx::query!(
            r#"
            UPDATE users
            SET deleted_at = NOW()
            WHERE id = $1 AND deleted_at IS NULL
            "#,
            user_id
        )
        .execute(pool)
        .await
        .context("Failed to delete user")?;

        Ok(())
    }

    /// Check if email exists (excluding specific user ID)
    pub async fn email_exists(
        pool: &PgPool,
        email: &str,
        exclude_user_id: Option<Uuid>,
    ) -> ApiResult<bool> {
        let exists = if let Some(user_id) = exclude_user_id {
            sqlx::query_scalar!(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM users
                    WHERE LOWER(email) = LOWER($1)
                    AND id != $2
                    AND deleted_at IS NULL
                ) as "exists!"
                "#,
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
                    WHERE LOWER(email) = LOWER($1)
                    AND deleted_at IS NULL
                ) as "exists!"
                "#,
                email
            )
            .fetch_one(pool)
            .await
            .context("Failed to check email existence")?
        };

        Ok(exists)
    }

    /// Activate user
    pub async fn activate_user(pool: &PgPool, user_id: Uuid) -> ApiResult<User> {
        let user = sqlx::query_as!(
            User,
            r#"
            UPDATE users
            SET is_active = TRUE
            WHERE id = $1 AND deleted_at IS NULL
            RETURNING id, email, full_name, phone, password_hash, roles, is_active,
                      last_login_at, last_login_ip, failed_login_attempts,
                      locked_until, created_at, updated_at, deleted_at
            "#,
            user_id
        )
        .fetch_one(pool)
        .await
        .context("Failed to activate user")?;

        Ok(user)
    }

    /// Deactivate user
    pub async fn deactivate_user(pool: &PgPool, user_id: Uuid) -> ApiResult<User> {
        let user = sqlx::query_as!(
            User,
            r#"
            UPDATE users
            SET is_active = FALSE
            WHERE id = $1 AND deleted_at IS NULL
            RETURNING id, email, full_name, phone, password_hash, roles, is_active,
                      last_login_at, last_login_ip, failed_login_attempts,
                      locked_until, created_at, updated_at, deleted_at
            "#,
            user_id
        )
        .fetch_one(pool)
        .await
        .context("Failed to deactivate user")?;

        Ok(user)
    }
}
