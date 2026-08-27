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

use super::{
    license::{ProtectedCredential, VerifiedSignedLease},
    models::{EffectiveAccess, LicenseInstallation, LicenseLease, TenantModule},
};

pub struct AccessOps;

impl AccessOps {
    pub async fn ensure_license_installation(
        pool: &PgPool,
        tenant_id: Uuid,
    ) -> Result<LicenseInstallation> {
        sqlx::query_as::<_, LicenseInstallation>(
            r#"
            INSERT INTO license_installations (tenant_id)
            VALUES ($1)
            ON CONFLICT (tenant_id) WHERE deleted_at IS NULL
            DO UPDATE SET tenant_id = EXCLUDED.tenant_id
            RETURNING id, deployment_id, remote_installation_id,
                      control_plane_url, credential_ciphertext, credential_nonce,
                      credential_hint, status, latest_lease_sequence,
                      last_refresh_attempt_at, last_refresh_success_at, last_error_code
            "#,
        )
        .bind(tenant_id)
        .fetch_one(pool)
        .await
        .context("Failed to prepare the license installation")
    }

    pub async fn latest_license_lease(
        pool: &PgPool,
        tenant_id: Uuid,
    ) -> Result<Option<LicenseLease>> {
        sqlx::query_as::<_, LicenseLease>(
            r#"
            SELECT lease_id, catalog_version, claims, status, issued_at, refresh_after,
                   lease_expires_at, grace_until, source
            FROM license_leases
            WHERE tenant_id = $1 AND deleted_at IS NULL
            ORDER BY sequence DESC
            LIMIT 1
            "#,
        )
        .bind(tenant_id)
        .fetch_optional(pool)
        .await
        .context("Failed to load the current license lease")
    }

    pub async fn note_refresh_attempt(pool: &PgPool, tenant_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE license_installations
            SET last_refresh_attempt_at = NOW(), last_error_code = NULL, updated_at = NOW()
            WHERE tenant_id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .execute(pool)
        .await
        .context("Failed to record the license refresh attempt")?;
        Ok(())
    }

    pub async fn note_license_error(
        pool: &PgPool,
        tenant_id: Uuid,
        error_code: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE license_installations
            SET last_error_code = $2, updated_at = NOW()
            WHERE tenant_id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(error_code)
        .execute(pool)
        .await
        .context("Failed to record the license error")?;
        Ok(())
    }

    pub async fn due_license_tenants(pool: &PgPool) -> Result<Vec<Uuid>> {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT installation.tenant_id
            FROM license_installations AS installation
            JOIN LATERAL (
                SELECT refresh_after
                FROM license_leases
                WHERE tenant_id = installation.tenant_id
                  AND status = 'active'
                  AND deleted_at IS NULL
                ORDER BY sequence DESC
                LIMIT 1
            ) AS lease ON TRUE
            WHERE installation.deleted_at IS NULL
              AND installation.status = 'active'
              AND installation.remote_installation_id IS NOT NULL
              AND installation.credential_ciphertext IS NOT NULL
              AND installation.credential_nonce IS NOT NULL
              AND lease.refresh_after <= NOW()
            ORDER BY installation.updated_at
            "#,
        )
        .fetch_all(pool)
        .await
        .context("Failed to load licenses due for refresh")
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn apply_signed_lease(
        pool: &PgPool,
        tenant_id: Uuid,
        remote_installation_id: Uuid,
        control_plane_url: Option<&str>,
        credential: Option<&ProtectedCredential>,
        verified: &VerifiedSignedLease,
        source: &str,
    ) -> Result<()> {
        let lease_id = Uuid::parse_str(&verified.claims.jti)
            .context("Verified lease identifier is invalid")?;
        let claims = serde_json::to_value(&verified.claims)
            .context("Verified lease claims could not be stored")?;
        let mut transaction = pool.begin().await?;
        let installation = sqlx::query_as::<_, LicenseInstallation>(
            r#"
            SELECT id, deployment_id, remote_installation_id,
                   control_plane_url, credential_ciphertext, credential_nonce,
                   credential_hint, status, latest_lease_sequence,
                   last_refresh_attempt_at, last_refresh_success_at, last_error_code
            FROM license_installations
            WHERE tenant_id = $1 AND deleted_at IS NULL
            FOR UPDATE
            "#,
        )
        .bind(tenant_id)
        .fetch_one(&mut *transaction)
        .await
        .context("License installation is not prepared")?;
        if installation
            .remote_installation_id
            .is_some_and(|current| current != remote_installation_id)
        {
            anyhow::bail!("Signed lease belongs to a different remote installation");
        }
        if verified.claims.sequence <= installation.latest_lease_sequence {
            anyhow::bail!("Signed lease sequence is not newer than the installed lease");
        }

        sqlx::query(
            r#"
            UPDATE license_leases
            SET status = 'superseded', updated_at = NOW()
            WHERE tenant_id = $1 AND status = 'active' AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .execute(&mut *transaction)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO license_leases (
                tenant_id, remote_installation_id, lease_id, sequence, key_id,
                token_fingerprint, catalog_version, claims, status, issued_at,
                refresh_after, lease_expires_at, grace_until, token_expires_at, source
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'active', $9, $10, $11, $12, $13, $14)
            "#,
        )
        .bind(tenant_id)
        .bind(remote_installation_id)
        .bind(lease_id)
        .bind(verified.claims.sequence)
        .bind(&verified.key_id)
        .bind(&verified.fingerprint)
        .bind(&verified.claims.catalog_version)
        .bind(&claims)
        .bind(verified.issued_at)
        .bind(verified.refresh_after)
        .bind(verified.lease_expires_at)
        .bind(verified.grace_until)
        .bind(verified.token_expires_at)
        .bind(source)
        .execute(&mut *transaction)
        .await?;

        sqlx::query(
            r#"
            UPDATE license_installations
            SET remote_installation_id = $2,
                control_plane_url = COALESCE($3, control_plane_url),
                credential_ciphertext = COALESCE($4, credential_ciphertext),
                credential_nonce = COALESCE($5, credential_nonce),
                credential_hint = COALESCE($6, credential_hint),
                status = 'active', latest_lease_sequence = $7,
                last_refresh_success_at = NOW(), last_error_code = NULL,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(installation.id)
        .bind(remote_installation_id)
        .bind(control_plane_url)
        .bind(credential.map(|value| value.ciphertext.as_str()))
        .bind(credential.map(|value| value.nonce.as_str()))
        .bind(credential.map(|value| value.hint.as_str()))
        .bind(verified.claims.sequence)
        .execute(&mut *transaction)
        .await?;

        sqlx::query(
            r#"
            UPDATE tenant_modules
            SET status = 'revoked', license_fingerprint = $2,
                license_expires_at = $3, updated_at = NOW()
            WHERE tenant_id = $1 AND source = 'license' AND deleted_at IS NULL
              AND NOT (module_key = ANY($4))
            "#,
        )
        .bind(tenant_id)
        .bind(&verified.fingerprint)
        .bind(verified.grace_until)
        .bind(&verified.claims.modules)
        .execute(&mut *transaction)
        .await?;

        for module_key in &verified.claims.modules {
            sqlx::query(
                r#"
                INSERT INTO tenant_modules (
                    tenant_id, module_key, status, source,
                    license_fingerprint, license_expires_at
                )
                VALUES ($1, $2, 'enabled', 'license', $3, $4)
                ON CONFLICT (tenant_id, module_key) WHERE deleted_at IS NULL
                DO UPDATE SET
                    status = CASE
                        WHEN tenant_modules.source != 'license' THEN tenant_modules.status
                        WHEN tenant_modules.status = 'disabled' THEN 'disabled'
                        ELSE 'enabled'
                    END,
                    license_fingerprint = CASE
                        WHEN tenant_modules.source = 'license' THEN EXCLUDED.license_fingerprint
                        ELSE tenant_modules.license_fingerprint
                    END,
                    license_expires_at = CASE
                        WHEN tenant_modules.source = 'license' THEN EXCLUDED.license_expires_at
                        ELSE tenant_modules.license_expires_at
                    END,
                    updated_at = NOW()
                "#,
            )
            .bind(tenant_id)
            .bind(module_key)
            .bind(&verified.fingerprint)
            .bind(verified.grace_until)
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;
        Ok(())
    }

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
            SELECT module_key, status, source, license_expires_at
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
