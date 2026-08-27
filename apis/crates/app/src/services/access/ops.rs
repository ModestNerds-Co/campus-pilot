//! Owns tenant access projections and atomic signed-lease installation.
//!
//! Commercial entitlements and role permissions stay separate until evaluation.

use std::collections::BTreeSet;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use cp_common::{EntitlementSnapshot, LeaseLifecycle, ModuleEntitlementState};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    license::{
        ProtectedCredential, SignedLeaseClaims, VerifiedSignedLease,
        app_version_bounds_are_supported, app_version_is_supported,
    },
    models::{EffectiveAccess, LicenseInstallation, LicenseLease, TenantModule},
};

pub struct AccessOps;

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntitlementProjectionEvidence {
    source_lease_id: Uuid,
    features: Vec<String>,
    app_version_supported: bool,
}

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

    async fn current_entitlement_projection(
        pool: &PgPool,
        tenant_id: Uuid,
    ) -> Result<Option<EntitlementProjectionEvidence>> {
        let row = sqlx::query_as::<_, (Uuid, Option<String>, Option<String>, Vec<String>)>(
            r#"
            SELECT entitlement.source_lease_id,
                   entitlement.min_app_version,
                   entitlement.max_app_version,
                   COALESCE(
                       ARRAY_AGG(feature.feature_key ORDER BY feature.feature_key)
                           FILTER (WHERE feature.feature_key IS NOT NULL),
                       ARRAY[]::TEXT[]
                   ) AS features
            FROM tenant_entitlements AS entitlement
            LEFT JOIN tenant_entitlement_features AS feature
              ON feature.tenant_id = entitlement.tenant_id
             AND feature.source_lease_id = entitlement.source_lease_id
            WHERE entitlement.tenant_id = $1
            GROUP BY entitlement.source_lease_id,
                     entitlement.min_app_version,
                     entitlement.max_app_version
            "#,
        )
        .bind(tenant_id)
        .fetch_optional(pool)
        .await
        .context("Failed to load the current entitlement projection")?;
        row.map(|(source_lease_id, minimum, maximum, features)| {
            Ok(EntitlementProjectionEvidence {
                source_lease_id,
                features,
                app_version_supported: app_version_bounds_are_supported(
                    minimum.as_deref(),
                    maximum.as_deref(),
                )?,
            })
        })
        .transpose()
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
        app_version_is_supported(&verified.claims)?;
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
            INSERT INTO tenant_entitlements (
                tenant_id, source_lease_id, lease_sequence, catalog_version,
                min_app_version, max_app_version, projected_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, NOW())
            ON CONFLICT (tenant_id)
            DO UPDATE SET
                source_lease_id = EXCLUDED.source_lease_id,
                lease_sequence = EXCLUDED.lease_sequence,
                catalog_version = EXCLUDED.catalog_version,
                min_app_version = EXCLUDED.min_app_version,
                max_app_version = EXCLUDED.max_app_version,
                projected_at = NOW(),
                updated_at = NOW()
            "#,
        )
        .bind(tenant_id)
        .bind(lease_id)
        .bind(verified.claims.sequence)
        .bind(&verified.claims.catalog_version)
        .bind(&verified.claims.min_app_version)
        .bind(&verified.claims.max_app_version)
        .execute(&mut *transaction)
        .await?;

        sqlx::query("DELETE FROM tenant_entitlement_features WHERE tenant_id = $1")
            .bind(tenant_id)
            .execute(&mut *transaction)
            .await?;
        for feature_key in &verified.claims.features {
            sqlx::query(
                r#"
                INSERT INTO tenant_entitlement_features (
                    tenant_id, feature_key, source_lease_id
                )
                VALUES ($1, $2, $3)
                "#,
            )
            .bind(tenant_id)
            .bind(feature_key)
            .bind(lease_id)
            .execute(&mut *transaction)
            .await?;
        }

        sqlx::query("DELETE FROM entitlement_limits WHERE tenant_id = $1")
            .bind(tenant_id)
            .execute(&mut *transaction)
            .await?;
        for limit in &verified.claims.limits {
            let value = i64::try_from(limit.value)
                .context("Verified lease capability limit is too large")?;
            sqlx::query(
                r#"
                INSERT INTO entitlement_limits (
                    tenant_id, limit_key, source_lease_id, unit,
                    period, limit_value, enforcement
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
            )
            .bind(tenant_id)
            .bind(&limit.key)
            .bind(lease_id)
            .bind(&limit.unit)
            .bind(&limit.period)
            .bind(value)
            .bind(&limit.enforcement)
            .execute(&mut *transaction)
            .await?;
        }

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

        let module_rows = sqlx::query_as::<_, TenantModule>(
            r#"
            SELECT module_key, status, source, license_expires_at
            FROM tenant_modules
            WHERE tenant_id = $1
              AND deleted_at IS NULL
            ORDER BY module_key
            "#,
        )
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .context("Failed to load module entitlements")?;
        let latest_lease = Self::latest_license_lease(pool, tenant_id).await?;
        let entitlement_projection = Self::current_entitlement_projection(pool, tenant_id).await?;
        let installation_status = sqlx::query_scalar::<_, String>(
            r#"
            SELECT status
            FROM license_installations
            WHERE tenant_id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .fetch_optional(pool)
        .await
        .context("Failed to load license installation state")?;
        let evaluated_at = Utc::now();
        let entitlements = entitlement_snapshot(
            &module_rows,
            latest_lease.as_ref(),
            entitlement_projection.as_ref(),
            installation_status.as_deref(),
            evaluated_at,
        )?;
        let enabled_modules = module_rows
            .iter()
            .filter(|module| {
                module.status == "enabled"
                    && module
                        .license_expires_at
                        .is_none_or(|expires_at| expires_at > evaluated_at)
            })
            .map(|module| module.module_key.clone())
            .collect();

        Ok(EffectiveAccess {
            role_names,
            permissions: permissions.into_iter().collect(),
            enabled_modules,
            entitlements,
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

fn entitlement_snapshot(
    modules: &[TenantModule],
    lease: Option<&LicenseLease>,
    projection: Option<&EntitlementProjectionEvidence>,
    installation_status: Option<&str>,
    now: DateTime<Utc>,
) -> Result<EntitlementSnapshot> {
    let (mut lifecycle, features, app_version_supported) = match lease {
        Some(lease) => {
            let lifecycle = match lease.status.as_str() {
                "revoked" => LeaseLifecycle::Revoked,
                "invalid" => LeaseLifecycle::Invalid,
                "expired" => LeaseLifecycle::Restricted,
                "active" | "superseded" if now < lease.refresh_after => LeaseLifecycle::Active,
                "active" | "superseded" if now < lease.lease_expires_at => {
                    LeaseLifecycle::RefreshDue
                }
                "active" | "superseded" if now < lease.grace_until => LeaseLifecycle::Grace,
                "active" | "superseded" => LeaseLifecycle::Restricted,
                other => anyhow::bail!("Stored license lease status is invalid: {other}"),
            };
            let evidence = projection_evidence(lease, projection)?;
            (lifecycle, evidence.features, evidence.app_version_supported)
        }
        None => (LeaseLifecycle::Legacy, Vec::new(), true),
    };

    lifecycle = match installation_status {
        Some("revoked") => LeaseLifecycle::Revoked,
        Some("suspended") => LeaseLifecycle::Restricted,
        Some("error") => LeaseLifecycle::Invalid,
        Some("unconfigured" | "active") | None => lifecycle,
        Some(other) => anyhow::bail!("Stored license installation status is invalid: {other}"),
    };

    let module_states = modules.iter().map(|module| {
        let state = match module.status.as_str() {
            "enabled"
                if lease.is_none()
                    && module
                        .license_expires_at
                        .is_some_and(|expires_at| expires_at <= now) =>
            {
                ModuleEntitlementState::Expired
            }
            "enabled" => ModuleEntitlementState::Enabled,
            "disabled" => ModuleEntitlementState::LocallyDisabled,
            "expired" => ModuleEntitlementState::Expired,
            "revoked" => ModuleEntitlementState::Revoked,
            other => anyhow::bail!("Stored module entitlement status is invalid: {other}"),
        };
        Ok((module.module_key.clone(), state))
    });
    let module_states = module_states.collect::<Result<Vec<_>>>()?;
    EntitlementSnapshot::new(lifecycle, module_states, features)
        .map(|snapshot| snapshot.with_app_version_supported(app_version_supported))
        .context("Stored module entitlement projection is invalid")
}

fn projection_evidence(
    lease: &LicenseLease,
    projection: Option<&EntitlementProjectionEvidence>,
) -> Result<EntitlementProjectionEvidence> {
    if let Some(projection) = projection.filter(|value| value.source_lease_id == lease.lease_id) {
        return Ok(projection.clone());
    }

    // A lease accepted before migration 009 has no normalized projection. Keep
    // it usable until its next successful refresh creates one transactionally.
    let claims = serde_json::from_value::<SignedLeaseClaims>(lease.claims.clone())
        .context("Stored license claims are invalid")?;
    Ok(EntitlementProjectionEvidence {
        source_lease_id: lease.lease_id,
        features: claims.features.clone(),
        app_version_supported: app_version_is_supported(&claims)?,
    })
}

#[cfg(test)]
mod entitlement_tests {
    use chrono::{Duration, Utc};
    use serde_json::json;
    use uuid::Uuid;

    use cp_common::{LeaseLifecycle, ModuleEntitlementState};

    use crate::tests::helpers::create_test_app_state;

    use super::{
        AccessOps, EntitlementProjectionEvidence, LicenseLease, TenantModule, entitlement_snapshot,
        projection_evidence,
    };
    use crate::services::access::license::{LeaseLimit, SignedLeaseClaims, VerifiedSignedLease};

    fn module(status: &str, expires_at: Option<chrono::DateTime<Utc>>) -> TenantModule {
        TenantModule {
            module_key: "fleet".to_string(),
            status: status.to_string(),
            source: "license".to_string(),
            license_expires_at: expires_at,
        }
    }

    fn lease(
        now: chrono::DateTime<Utc>,
        refresh_after: chrono::DateTime<Utc>,
        lease_expires_at: chrono::DateTime<Utc>,
        grace_until: chrono::DateTime<Utc>,
    ) -> LicenseLease {
        LicenseLease {
            lease_id: Uuid::new_v4(),
            catalog_version: "plans/complete/1".to_string(),
            claims: json!({
                "contract_version": "cp-license/v1",
                "iss": "campus-pilot-control-plane",
                "aud": "campus-pilot",
                "sub": Uuid::new_v4().to_string(),
                "installation_id": Uuid::new_v4().to_string(),
                "jti": Uuid::new_v4().to_string(),
                "sequence": 1,
                "catalog_version": "plans/complete/1",
                "iat": now.timestamp(),
                "nbf": (now - Duration::seconds(30)).timestamp(),
                "refresh_after": refresh_after.timestamp(),
                "lease_expires_at": lease_expires_at.timestamp(),
                "grace_until": grace_until.timestamp(),
                "exp": grace_until.timestamp(),
                "modules": ["fleet"],
                "features": ["fleet.trips"],
                "limits": [],
                "min_app_version": "1.0.0",
                "max_app_version": "1.9.0"
            }),
            status: "active".to_string(),
            issued_at: now,
            refresh_after,
            lease_expires_at,
            grace_until,
            source: "online_activation".to_string(),
        }
    }

    fn verified_lease(
        tenant_id: Uuid,
        installation_id: Uuid,
        sequence: i64,
        modules: Vec<String>,
        features: Vec<String>,
        limits: Vec<LeaseLimit>,
    ) -> VerifiedSignedLease {
        let issued_at = Utc::now();
        let refresh_after = issued_at + Duration::hours(1);
        let lease_expires_at = issued_at + Duration::hours(2);
        let grace_until = issued_at + Duration::hours(3);
        VerifiedSignedLease {
            claims: SignedLeaseClaims {
                contract_version: "cp-license/v1".to_string(),
                iss: "campus-pilot-control-plane".to_string(),
                aud: "campus-pilot".to_string(),
                sub: tenant_id.to_string(),
                installation_id: installation_id.to_string(),
                jti: Uuid::new_v4().to_string(),
                sequence,
                catalog_version: "plans/test/1".to_string(),
                iat: issued_at.timestamp(),
                nbf: (issued_at - Duration::seconds(30)).timestamp(),
                refresh_after: refresh_after.timestamp(),
                lease_expires_at: lease_expires_at.timestamp(),
                grace_until: grace_until.timestamp(),
                exp: grace_until.timestamp(),
                modules,
                features,
                limits,
                min_app_version: Some("1.0.0".to_string()),
                max_app_version: Some("1.9.0".to_string()),
            },
            fingerprint: format!("test-fingerprint-{sequence}"),
            key_id: "test-key".to_string(),
            issued_at,
            refresh_after,
            lease_expires_at,
            grace_until,
            token_expires_at: grace_until,
        }
    }

    #[test]
    fn lease_lifecycle_is_derived_from_trusted_deadlines() {
        let now = Utc::now();
        let active = lease(
            now,
            now + Duration::hours(1),
            now + Duration::hours(2),
            now + Duration::hours(3),
        );
        let refresh = lease(
            now,
            now - Duration::minutes(1),
            now + Duration::hours(1),
            now + Duration::hours(2),
        );
        let grace = lease(
            now,
            now - Duration::hours(2),
            now - Duration::minutes(1),
            now + Duration::hours(1),
        );
        let restricted = lease(
            now,
            now - Duration::hours(3),
            now - Duration::hours(2),
            now - Duration::hours(1),
        );
        for (lease, expected) in [
            (active, LeaseLifecycle::Active),
            (refresh, LeaseLifecycle::RefreshDue),
            (grace, LeaseLifecycle::Grace),
            (restricted, LeaseLifecycle::Restricted),
        ] {
            let snapshot =
                entitlement_snapshot(&[module("enabled", None)], Some(&lease), None, None, now)
                    .unwrap_or_else(|_| unreachable!());
            assert_eq!(snapshot.lease(), expected);
            assert!(snapshot.has_feature("fleet.trips"));
        }

        let restricted = lease(
            now,
            now - Duration::hours(3),
            now - Duration::hours(2),
            now - Duration::hours(1),
        );
        let snapshot = entitlement_snapshot(
            &[module("enabled", Some(now - Duration::hours(1)))],
            Some(&restricted),
            None,
            None,
            now,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            snapshot.module_state("fleet"),
            Some(ModuleEntitlementState::Enabled)
        );
    }

    #[test]
    fn installation_revocation_overrides_an_active_lease() {
        let now = Utc::now();
        let lease = lease(
            now,
            now + Duration::hours(1),
            now + Duration::hours(2),
            now + Duration::hours(3),
        );
        let snapshot = entitlement_snapshot(
            &[module("enabled", None)],
            Some(&lease),
            None,
            Some("revoked"),
            now,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(snapshot.lease(), LeaseLifecycle::Revoked);
    }

    #[test]
    fn local_and_expired_module_states_remain_distinct() {
        let now = Utc::now();
        let disabled = entitlement_snapshot(
            &[module("disabled", None)],
            None,
            None,
            Some("unconfigured"),
            now,
        )
        .unwrap_or_else(|_| unreachable!());
        let expired = entitlement_snapshot(
            &[module("enabled", Some(now - Duration::seconds(1)))],
            None,
            None,
            None,
            now,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            disabled.module_state("fleet"),
            Some(ModuleEntitlementState::LocallyDisabled)
        );
        assert_eq!(
            expired.module_state("fleet"),
            Some(ModuleEntitlementState::Expired)
        );
    }

    #[test]
    fn persisted_status_values_are_exhaustive_and_reject_unknowns() {
        let now = Utc::now();
        let base = lease(
            now,
            now + Duration::hours(1),
            now + Duration::hours(2),
            now + Duration::hours(3),
        );
        for (status, expected) in [
            ("revoked", LeaseLifecycle::Revoked),
            ("invalid", LeaseLifecycle::Invalid),
            ("expired", LeaseLifecycle::Restricted),
        ] {
            let mut lease = base.clone();
            lease.status = status.to_string();
            let snapshot =
                entitlement_snapshot(&[module("enabled", None)], Some(&lease), None, None, now)
                    .unwrap_or_else(|_| unreachable!());
            assert_eq!(snapshot.lease(), expected);
        }
        for (status, expected) in [
            ("suspended", LeaseLifecycle::Restricted),
            ("error", LeaseLifecycle::Invalid),
        ] {
            let snapshot = entitlement_snapshot(
                &[module("enabled", None)],
                Some(&base),
                None,
                Some(status),
                now,
            )
            .unwrap_or_else(|_| unreachable!());
            assert_eq!(snapshot.lease(), expected);
        }
        for (status, expected) in [
            ("expired", ModuleEntitlementState::Expired),
            ("revoked", ModuleEntitlementState::Revoked),
        ] {
            let snapshot = entitlement_snapshot(&[module(status, None)], None, None, None, now)
                .unwrap_or_else(|_| unreachable!());
            assert_eq!(snapshot.module_state("fleet"), Some(expected));
        }

        let mut invalid_lease = base;
        invalid_lease.status = "unknown".to_string();
        assert!(
            entitlement_snapshot(
                &[module("enabled", None)],
                Some(&invalid_lease),
                None,
                None,
                now,
            )
            .is_err()
        );
        assert!(
            entitlement_snapshot(&[module("enabled", None)], None, None, Some("unknown"), now,)
                .is_err()
        );
        assert!(entitlement_snapshot(&[module("unknown", None)], None, None, None, now).is_err());
    }

    #[test]
    fn normalized_projection_is_authoritative_for_its_source_lease() {
        let now = Utc::now();
        let lease = lease(
            now,
            now + Duration::hours(1),
            now + Duration::hours(2),
            now + Duration::hours(3),
        );
        let projection = EntitlementProjectionEvidence {
            source_lease_id: lease.lease_id,
            features: vec!["fleet.projected".to_string()],
            app_version_supported: false,
        };
        let evidence =
            projection_evidence(&lease, Some(&projection)).unwrap_or_else(|_| unreachable!());
        assert_eq!(evidence, projection);

        let snapshot = entitlement_snapshot(
            &[module("enabled", None)],
            Some(&lease),
            Some(&projection),
            None,
            now,
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(snapshot.has_feature("fleet.projected"));
        assert!(!snapshot.has_feature("fleet.trips"));

        let stale = EntitlementProjectionEvidence {
            source_lease_id: Uuid::new_v4(),
            features: vec!["fleet.stale".to_string()],
            app_version_supported: false,
        };
        let fallback = projection_evidence(&lease, Some(&stale)).unwrap_or_else(|_| unreachable!());
        assert_eq!(fallback.source_lease_id, lease.lease_id);
        assert_eq!(fallback.features, vec!["fleet.trips"]);
        assert!(fallback.app_version_supported);
    }

    #[actix_web::test]
    async fn accepted_lease_replaces_normalized_projection_atomically() {
        let state = create_test_app_state().await;
        let tenant_id = Uuid::new_v4();
        let slug = format!("projection-test-{tenant_id}");
        sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, $2, 'Projection test')")
            .bind(tenant_id)
            .bind(slug)
            .execute(&state.db)
            .await
            .unwrap_or_else(|_| unreachable!());

        let installation = AccessOps::ensure_license_installation(&state.db, tenant_id)
            .await
            .unwrap_or_else(|_| unreachable!());
        let remote_installation_id = Uuid::new_v4();
        let first = verified_lease(
            tenant_id,
            remote_installation_id,
            1,
            vec!["fleet".to_string(), "agent".to_string()],
            vec!["agent.sessions".to_string()],
            vec![LeaseLimit {
                key: "agent.runs".to_string(),
                unit: "run".to_string(),
                period: "month".to_string(),
                value: 100,
                enforcement: "report".to_string(),
            }],
        );
        AccessOps::apply_signed_lease(
            &state.db,
            tenant_id,
            remote_installation_id,
            Some("https://licenses.example.test"),
            None,
            &first,
            "online_activation",
        )
        .await
        .unwrap_or_else(|_| unreachable!());

        let projection = AccessOps::current_entitlement_projection(&state.db, tenant_id)
            .await
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(projection.features, vec!["agent.sessions"]);
        assert!(projection.app_version_supported);
        let limit = sqlx::query_as::<_, (String, i64, String)>(
            "SELECT limit_key, limit_value, enforcement FROM entitlement_limits WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(limit, ("agent.runs".to_string(), 100, "report".to_string()));

        let second = verified_lease(
            tenant_id,
            remote_installation_id,
            2,
            vec!["agent".to_string()],
            vec!["agent.history".to_string()],
            vec![],
        );
        AccessOps::apply_signed_lease(
            &state.db,
            tenant_id,
            remote_installation_id,
            None,
            None,
            &second,
            "online_refresh",
        )
        .await
        .unwrap_or_else(|_| unreachable!());

        let projection = AccessOps::current_entitlement_projection(&state.db, tenant_id)
            .await
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(projection.features, vec!["agent.history"]);
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM entitlement_limits WHERE tenant_id = $1"
            )
            .bind(tenant_id)
            .fetch_one(&state.db)
            .await
            .unwrap_or_default(),
            0
        );
        let states = sqlx::query_as::<_, (String, String)>(
            "SELECT module_key, status FROM tenant_modules WHERE tenant_id = $1 ORDER BY module_key",
        )
        .bind(tenant_id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();
        assert_eq!(
            states,
            vec![
                ("administration".to_string(), "enabled".to_string()),
                ("agent".to_string(), "enabled".to_string()),
                ("fleet".to_string(), "revoked".to_string()),
                ("home".to_string(), "enabled".to_string()),
            ]
        );
        assert_eq!(installation.latest_lease_sequence, 0);
    }
}
