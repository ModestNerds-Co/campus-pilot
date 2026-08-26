//
//  campus-pilot-apis
//  ops.rs
//
//  Created by Ngonidzashe Mangudya on 2025/10/02.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use crate::models::typedefs::ApiResult;
use anyhow;
use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::models::{RefreshToken, User};

pub struct AuthOps;

impl AuthOps {
    /// Find user by email (case-insensitive)
    pub async fn find_user_by_email(pool: &PgPool, email: &str) -> ApiResult<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, tenant_id, email, full_name, phone, password_hash, roles, is_active,
                   last_login_at, last_login_ip, failed_login_attempts, locked_until,
                   created_at, updated_at, deleted_at
            FROM users
            WHERE LOWER(email) = LOWER($1) AND deleted_at IS NULL
            "#,
        )
        .bind(email)
        .fetch_optional(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch user by email: {}", e))?;

        Ok(user)
    }

    /// Find user by ID
    pub async fn find_user_by_id(pool: &PgPool, user_id: Uuid) -> ApiResult<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, tenant_id, email, full_name, phone, password_hash, roles, is_active,
                   last_login_at, last_login_ip, failed_login_attempts, locked_until,
                   created_at, updated_at, deleted_at
            FROM users
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch user by ID: {}", e))?;

        Ok(user)
    }

    /// Update user login info (last login timestamp, IP, reset failed attempts)
    pub async fn update_login_info(
        pool: &PgPool,
        user_id: Uuid,
        ip_address: Option<&str>,
    ) -> ApiResult<()> {
        sqlx::query(
            r#"
            UPDATE users
            SET last_login_at = NOW(),
                last_login_ip = $2,
                failed_login_attempts = 0,
                locked_until = NULL
            WHERE id = $1
            "#,
        )
        .bind(user_id)
        .bind(ip_address)
        .execute(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to update login info: {}", e))?;

        Ok(())
    }

    /// Increment failed login attempts
    pub async fn increment_failed_login(pool: &PgPool, user_id: Uuid) -> ApiResult<i32> {
        let result = sqlx::query_scalar::<_, i32>(
            r#"
            UPDATE users
            SET failed_login_attempts = failed_login_attempts + 1,
                locked_until = CASE
                    WHEN failed_login_attempts + 1 >= 5 THEN NOW() + INTERVAL '15 minutes'
                    ELSE NULL
                END
            WHERE id = $1
            RETURNING failed_login_attempts
            "#,
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to increment failed login attempts: {}", e))?;

        Ok(result)
    }

    /// Store refresh token
    pub async fn store_refresh_token(
        pool: &PgPool,
        tenant_id: Uuid,
        user_id: Uuid,
        token: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> ApiResult<RefreshToken> {
        let expires_at = Utc::now() + Duration::days(7);

        let refresh_token = sqlx::query_as::<_, RefreshToken>(
            r#"
            INSERT INTO refresh_tokens (tenant_id, user_id, token, ip_address, user_agent, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, tenant_id, user_id, token, ip_address, user_agent, expires_at, revoked_at, created_at, updated_at
            "#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(token)
        .bind(ip_address)
        .bind(user_agent)
        .bind(expires_at)
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to store refresh token: {}", e))?;

        Ok(refresh_token)
    }

    /// Find refresh token
    pub async fn find_refresh_token(pool: &PgPool, token: &str) -> ApiResult<Option<RefreshToken>> {
        let refresh_token = sqlx::query_as::<_, RefreshToken>(
            r#"
            SELECT id, tenant_id, user_id, token, ip_address, user_agent, expires_at, revoked_at, created_at, updated_at
            FROM refresh_tokens
            WHERE token = $1
            "#,
        )
        .bind(token)
        .fetch_optional(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch refresh token: {}", e))?;

        Ok(refresh_token)
    }

    /// Revoke refresh token
    pub async fn revoke_refresh_token(pool: &PgPool, token: &str) -> ApiResult<()> {
        sqlx::query(
            r#"
            UPDATE refresh_tokens
            SET revoked_at = NOW()
            WHERE token = $1
            "#,
        )
        .bind(token)
        .execute(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to revoke refresh token: {}", e))?;

        Ok(())
    }

    /// Revoke all user refresh tokens
    pub async fn revoke_all_user_tokens(pool: &PgPool, user_id: Uuid) -> ApiResult<()> {
        sqlx::query(
            r#"
            UPDATE refresh_tokens
            SET revoked_at = NOW()
            WHERE user_id = $1 AND revoked_at IS NULL
            "#,
        )
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to revoke all user tokens: {}", e))?;

        Ok(())
    }

    /// Validate refresh token (not expired, not revoked)
    pub fn validate_refresh_token(token: &RefreshToken) -> ApiResult<()> {
        if token.revoked_at.is_some() {
            return Err(anyhow::anyhow!("Refresh token has been revoked"));
        }

        if token.expires_at < Utc::now() {
            return Err(anyhow::anyhow!("Refresh token has expired"));
        }

        Ok(())
    }

    /// Check if user account is locked
    pub fn is_account_locked(user: &User) -> bool {
        if let Some(locked_until) = user.locked_until {
            locked_until > Utc::now()
        } else {
            false
        }
    }
}
