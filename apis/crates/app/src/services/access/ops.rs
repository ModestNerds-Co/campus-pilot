//
//  campus-pilot-apis
//  ops.rs
//
//  Created by OpenAI Codex on 2026/08/26.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use std::collections::BTreeSet;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use super::models::{EffectiveAccess, TenantModule};

pub struct AccessOps;

impl AccessOps {
    pub async fn effective_access(
        pool: &PgPool,
        tenant_id: Uuid,
        role_keys: &[String],
    ) -> Result<EffectiveAccess> {
        let role_rows = sqlx::query_as::<_, (String, Vec<String>)>(
            r#"
            SELECT name, permissions
            FROM roles
            WHERE tenant_id = $1
              AND key = ANY($2)
              AND deleted_at IS NULL
            ORDER BY name
            "#,
        )
        .bind(tenant_id)
        .bind(role_keys)
        .fetch_all(pool)
        .await
        .context("Failed to load role permissions")?;

        let mut role_names = Vec::new();
        let mut permissions = BTreeSet::new();
        for (name, role_permissions) in role_rows {
            role_names.push(name);
            permissions.extend(role_permissions);
        }

        let enabled_modules = sqlx::query_scalar::<_, String>(
            r#"
            SELECT module_key
            FROM tenant_modules
            WHERE tenant_id = $1
              AND deleted_at IS NULL
              AND status = 'enabled'
              AND (license_expires_at IS NULL OR license_expires_at > NOW())
            ORDER BY module_key
            "#,
        )
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .context("Failed to load enabled modules")?;

        Ok(EffectiveAccess {
            role_names,
            permissions: permissions.into_iter().collect(),
            enabled_modules,
        })
    }

    pub async fn list_tenant_modules(pool: &PgPool, tenant_id: Uuid) -> Result<Vec<TenantModule>> {
        sqlx::query_as::<_, TenantModule>(
            r#"
            SELECT id, tenant_id, module_key, status, source, license_fingerprint,
                   license_expires_at, created_at, updated_at
            FROM tenant_modules
            WHERE tenant_id = $1 AND deleted_at IS NULL
            ORDER BY module_key
            "#,
        )
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .context("Failed to load tenant modules")
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn activate_license(
        pool: &PgPool,
        tenant_id: Uuid,
        fingerprint: &str,
        issuer: &str,
        entitlement_id: Option<&str>,
        module_keys: &[String],
        expires_at: Option<DateTime<Utc>>,
        claims: &Value,
    ) -> Result<()> {
        let mut transaction = pool.begin().await?;

        sqlx::query(
            r#"
            INSERT INTO module_license_activations (
                tenant_id, fingerprint, issuer, entitlement_id, module_keys, expires_at, claims
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (tenant_id, fingerprint) WHERE deleted_at IS NULL
            DO UPDATE SET
                issuer = EXCLUDED.issuer,
                entitlement_id = EXCLUDED.entitlement_id,
                module_keys = EXCLUDED.module_keys,
                expires_at = EXCLUDED.expires_at,
                claims = EXCLUDED.claims,
                updated_at = NOW()
            "#,
        )
        .bind(tenant_id)
        .bind(fingerprint)
        .bind(issuer)
        .bind(entitlement_id)
        .bind(module_keys)
        .bind(expires_at)
        .bind(claims)
        .execute(&mut *transaction)
        .await?;

        for module_key in module_keys {
            sqlx::query(
                r#"
                INSERT INTO tenant_modules (
                    tenant_id, module_key, status, source, license_fingerprint, license_expires_at
                )
                VALUES ($1, $2, 'enabled', 'license', $3, $4)
                ON CONFLICT (tenant_id, module_key) WHERE deleted_at IS NULL
                DO UPDATE SET
                    status = 'enabled',
                    source = 'license',
                    license_fingerprint = EXCLUDED.license_fingerprint,
                    license_expires_at = EXCLUDED.license_expires_at,
                    updated_at = NOW()
                "#,
            )
            .bind(tenant_id)
            .bind(module_key)
            .bind(fingerprint)
            .bind(expires_at)
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;
        Ok(())
    }

    pub async fn disable_module(pool: &PgPool, tenant_id: Uuid, module_key: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE tenant_modules
            SET status = 'disabled', updated_at = NOW()
            WHERE tenant_id = $1 AND module_key = $2 AND deleted_at IS NULL AND source != 'core'
            "#,
        )
        .bind(tenant_id)
        .bind(module_key)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
