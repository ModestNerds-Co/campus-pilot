//! Builds safe licensing read models shared by HTTP and Agent entry points.

use anyhow::{Context, Result};
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::LicenseConfig;

use super::{
    dtos::{LeaseStateResponse, LicensingStateResponse},
    license::SignedLeaseClaims,
    models::LicenseLimitResponse,
    ops::AccessOps,
};

pub struct LicensingReadModel;

impl LicensingReadModel {
    pub async fn load(
        pool: &PgPool,
        tenant_id: Uuid,
        config: &LicenseConfig,
    ) -> Result<LicensingStateResponse> {
        let installation = AccessOps::ensure_license_installation(pool, tenant_id).await?;
        let lease = AccessOps::latest_license_lease(pool, tenant_id)
            .await?
            .map(|value| {
                let claims = serde_json::from_value::<SignedLeaseClaims>(value.claims)
                    .context("Stored license claims are invalid")?;
                Ok::<_, anyhow::Error>(LeaseStateResponse {
                    id: value.lease_id.to_string(),
                    status: value.status,
                    source: value.source,
                    catalog_version: value.catalog_version,
                    issued_at: value.issued_at,
                    refresh_after: value.refresh_after,
                    lease_expires_at: value.lease_expires_at,
                    grace_until: value.grace_until,
                    modules: claims.modules,
                    features: claims.features,
                    limits: claims
                        .limits
                        .into_iter()
                        .map(|limit| LicenseLimitResponse {
                            key: limit.key,
                            unit: limit.unit,
                            period: limit.period,
                            value: limit.value,
                            enforcement: limit.enforcement,
                        })
                        .collect(),
                })
            })
            .transpose()?;

        Ok(LicensingStateResponse {
            configured: config.control_plane_url.is_some()
                && config.verification_is_configured()
                && config.credential_key_base64.is_some(),
            connected: installation.remote_installation_id.is_some(),
            status: installation.status,
            deployment_id: installation.deployment_id.to_string(),
            installation_id: installation
                .remote_installation_id
                .map(|value| value.to_string()),
            credential_hint: installation.credential_hint,
            portal_url: config.control_plane_url.clone(),
            latest_sequence: installation.latest_lease_sequence,
            last_refresh_attempt_at: installation.last_refresh_attempt_at,
            last_refresh_success_at: installation.last_refresh_success_at,
            last_error_code: installation.last_error_code,
            lease,
        })
    }
}
