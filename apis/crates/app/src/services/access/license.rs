//
//  campus-pilot-apis
//  license.rs
//
//  Created by OpenAI Codex on 2026/08/26.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::config::LicenseConfig;

use super::catalog::{is_core_module, is_known_module};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseClaims {
    pub sub: String,
    pub modules: Vec<String>,
    pub iss: String,
    pub exp: usize,
    pub iat: Option<usize>,
    pub jti: Option<String>,
}

pub struct VerifiedLicense {
    pub claims: LicenseClaims,
    pub fingerprint: String,
    pub expires_at: DateTime<Utc>,
}

pub fn verify_license(
    key: &str,
    tenant_id: Uuid,
    config: &LicenseConfig,
) -> Result<VerifiedLicense> {
    let encoded_key = config
        .public_key_base64
        .as_deref()
        .context("License verification is not configured for this installation")?;
    let public_key = STANDARD
        .decode(encoded_key)
        .context("Configured license public key is invalid")?;

    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.set_issuer(&[config.issuer.as_str()]);
    validation.validate_exp = true;

    let token = decode::<LicenseClaims>(
        key.trim(),
        &DecodingKey::from_ed_pem(&public_key)
            .context("License public key is not valid Ed25519 PEM")?,
        &validation,
    )
    .context("License key signature or claims are invalid")?;

    if token.claims.sub != tenant_id.to_string() {
        bail!("License key belongs to a different campus");
    }
    if token.claims.modules.is_empty() {
        bail!("License key contains no module entitlements");
    }
    if token
        .claims
        .modules
        .iter()
        .any(|module| !is_known_module(module) || is_core_module(module))
    {
        bail!("License key contains an unknown or core module");
    }

    let expires_at = DateTime::from_timestamp(token.claims.exp as i64, 0)
        .context("License expiry is invalid")?;
    if expires_at <= Utc::now() {
        bail!("License key has expired");
    }

    let fingerprint = format!("{:x}", Sha256::digest(key.trim().as_bytes()));
    Ok(VerifiedLicense {
        claims: token.claims,
        fingerprint,
        expires_at,
    })
}
