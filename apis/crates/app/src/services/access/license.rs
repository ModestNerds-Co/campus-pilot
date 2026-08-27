//! Verifies signed license material and protects renewable installation credentials.
//!
//! Raw activation material and signed envelopes remain write-only outside this module.

use std::collections::BTreeSet;

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use semver::Version;
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaseLimit {
    pub key: String,
    pub unit: String,
    pub period: String,
    pub value: u64,
    pub enforcement: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedLeaseClaims {
    pub contract_version: String,
    pub iss: String,
    pub aud: String,
    pub sub: String,
    pub installation_id: String,
    pub jti: String,
    pub sequence: i64,
    pub catalog_version: String,
    pub iat: i64,
    pub nbf: i64,
    pub refresh_after: i64,
    pub lease_expires_at: i64,
    pub grace_until: i64,
    pub exp: i64,
    pub modules: Vec<String>,
    pub features: Vec<String>,
    pub limits: Vec<LeaseLimit>,
    pub min_app_version: Option<String>,
    pub max_app_version: Option<String>,
}

pub struct VerifiedSignedLease {
    pub claims: SignedLeaseClaims,
    pub fingerprint: String,
    pub key_id: String,
    pub issued_at: DateTime<Utc>,
    pub refresh_after: DateTime<Utc>,
    pub lease_expires_at: DateTime<Utc>,
    pub grace_until: DateTime<Utc>,
    pub token_expires_at: DateTime<Utc>,
}

pub struct ProtectedCredential {
    pub ciphertext: String,
    pub nonce: String,
    pub hint: String,
}

#[derive(Debug, Deserialize)]
pub struct ControlPlaneActivationResponse {
    pub installation_id: String,
    pub installation_token: String,
    pub lease: String,
    pub claims: SignedLeaseClaims,
}

#[derive(Debug, Deserialize)]
pub struct ControlPlaneRenewalResponse {
    pub token: String,
    pub claims: SignedLeaseClaims,
}

#[derive(Debug, Deserialize)]
pub struct OfflineLeaseBundle {
    pub format: String,
    pub key_id: String,
    pub lease: String,
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

pub fn verify_signed_lease(
    token: &str,
    tenant_id: Uuid,
    expected_installation_id: Option<Uuid>,
    config: &LicenseConfig,
) -> Result<VerifiedSignedLease> {
    let token = token.trim();
    let encoded_key = config
        .public_key_base64
        .as_deref()
        .context("License verification is not configured for this installation")?;
    let public_key = STANDARD
        .decode(encoded_key)
        .context("Configured license public key is invalid")?;
    let header = decode_header(token).context("Signed lease header is invalid")?;
    if header.alg != Algorithm::EdDSA {
        bail!("Signed lease uses an unsupported algorithm");
    }
    let key_id = header
        .kid
        .filter(|value| !value.trim().is_empty())
        .context("Signed lease has no signing key identifier")?;

    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.set_issuer(&[config.issuer.as_str()]);
    validation.set_audience(&[config.audience.as_str()]);
    validation.validate_exp = true;
    validation.validate_nbf = true;
    validation.leeway = 30;
    let decoded = decode::<SignedLeaseClaims>(
        token,
        &DecodingKey::from_ed_pem(&public_key)
            .context("License public key is not valid Ed25519 PEM")?,
        &validation,
    )
    .context("Signed lease signature or registered claims are invalid")?;
    validate_signed_claims(&decoded.claims, tenant_id, expected_installation_id)?;

    Ok(VerifiedSignedLease {
        issued_at: timestamp("issued at", decoded.claims.iat)?,
        refresh_after: timestamp("refresh after", decoded.claims.refresh_after)?,
        lease_expires_at: timestamp("lease expiry", decoded.claims.lease_expires_at)?,
        grace_until: timestamp("grace deadline", decoded.claims.grace_until)?,
        token_expires_at: timestamp("token expiry", decoded.claims.exp)?,
        fingerprint: format!("{:x}", Sha256::digest(token.as_bytes())),
        key_id,
        claims: decoded.claims,
    })
}

fn validate_signed_claims(
    claims: &SignedLeaseClaims,
    tenant_id: Uuid,
    expected_installation_id: Option<Uuid>,
) -> Result<()> {
    if claims.contract_version != "cp-license/v1" {
        bail!("Signed lease contract version is not supported");
    }
    if claims.sub != tenant_id.to_string() {
        bail!("Signed lease belongs to a different campus");
    }
    let installation_id = Uuid::parse_str(&claims.installation_id)
        .context("Signed lease installation identifier is invalid")?;
    if expected_installation_id.is_some_and(|expected| expected != installation_id) {
        bail!("Signed lease belongs to a different installation");
    }
    Uuid::parse_str(&claims.jti).context("Signed lease identifier is invalid")?;
    if claims.sequence <= 0 {
        bail!("Signed lease sequence is invalid");
    }
    if claims.catalog_version.trim().is_empty() {
        bail!("Signed lease catalog version is missing");
    }
    if claims.modules.is_empty() {
        bail!("Signed lease contains no module entitlements");
    }
    if claims
        .modules
        .iter()
        .any(|module| !is_known_module(module) || is_core_module(module))
    {
        bail!("Signed lease contains an unknown or core module");
    }
    if !all_unique(claims.modules.iter()) {
        bail!("Signed lease contains duplicate module entitlements");
    }
    if claims
        .features
        .iter()
        .any(|feature| !valid_entitlement_key(feature))
        || !all_unique(claims.features.iter())
    {
        bail!("Signed lease contains an invalid feature entitlement");
    }
    if claims.limits.iter().any(|limit| !valid_limit(limit))
        || !all_unique(claims.limits.iter().map(|limit| &limit.key))
    {
        bail!("Signed lease contains an invalid capability limit");
    }
    validate_version_bounds(claims)?;
    if !(claims.nbf <= claims.iat
        && claims.iat <= claims.refresh_after
        && claims.refresh_after <= claims.lease_expires_at
        && claims.lease_expires_at <= claims.grace_until
        && claims.grace_until == claims.exp)
    {
        bail!("Signed lease lifecycle timestamps are invalid");
    }
    let now = Utc::now().timestamp();
    if claims.iat > now + Duration::minutes(5).num_seconds() {
        bail!("Signed lease was issued too far in the future");
    }
    Ok(())
}

fn validate_version_bounds(claims: &SignedLeaseClaims) -> Result<()> {
    parsed_version_bounds(
        claims.min_app_version.as_deref(),
        claims.max_app_version.as_deref(),
    )?;
    Ok(())
}

fn parsed_version_bounds(
    minimum: Option<&str>,
    maximum: Option<&str>,
) -> Result<(Option<Version>, Option<Version>)> {
    let minimum = minimum
        .map(Version::parse)
        .transpose()
        .context("Signed lease minimum application version is invalid")?;
    let maximum = maximum
        .map(Version::parse)
        .transpose()
        .context("Signed lease maximum application version is invalid")?;
    if minimum
        .as_ref()
        .zip(maximum.as_ref())
        .is_some_and(|(minimum, maximum)| minimum > maximum)
    {
        bail!("Signed lease application version bounds are invalid");
    }
    Ok((minimum, maximum))
}

pub(crate) fn app_version_is_supported(claims: &SignedLeaseClaims) -> Result<bool> {
    app_version_bounds_are_supported(
        claims.min_app_version.as_deref(),
        claims.max_app_version.as_deref(),
    )
}

pub(crate) fn app_version_bounds_are_supported(
    minimum: Option<&str>,
    maximum: Option<&str>,
) -> Result<bool> {
    let (minimum, maximum) = parsed_version_bounds(minimum, maximum)?;
    let current = Version::parse(env!("CARGO_PKG_VERSION"))
        .context("Campus Pilot application version is invalid")?;
    let minimum_matches = minimum.is_none_or(|minimum| current >= minimum);
    let maximum_matches = maximum.is_none_or(|maximum| current <= maximum);
    Ok(minimum_matches && maximum_matches)
}

fn valid_limit(limit: &LeaseLimit) -> bool {
    valid_entitlement_key(&limit.key)
        && !limit.unit.trim().is_empty()
        && matches!(limit.period.as_str(), "none" | "day" | "month" | "year")
        && matches!(limit.enforcement.as_str(), "report" | "hard")
        && i64::try_from(limit.value).is_ok()
}

fn valid_entitlement_key(key: &str) -> bool {
    let mut characters = key.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && characters.all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '_'
                || character == '.'
        })
}

fn all_unique<'a>(values: impl IntoIterator<Item = &'a String>) -> bool {
    let mut unique = BTreeSet::new();
    values.into_iter().all(|value| unique.insert(value))
}

fn timestamp(label: &str, value: i64) -> Result<DateTime<Utc>> {
    DateTime::from_timestamp(value, 0).with_context(|| format!("Signed lease {label} is invalid"))
}

pub fn protect_installation_credential(
    credential: &str,
    tenant_id: Uuid,
    deployment_id: Uuid,
    config: &LicenseConfig,
) -> Result<ProtectedCredential> {
    let cipher = credential_cipher(config)?;
    let mut nonce_bytes = [0_u8; 12];
    getrandom::fill(&mut nonce_bytes).context("Could not generate credential nonce")?;
    let aad = credential_aad(tenant_id, deployment_id);
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload {
                msg: credential.as_bytes(),
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| anyhow::anyhow!("Could not protect installation credential"))?;
    Ok(ProtectedCredential {
        ciphertext: STANDARD.encode(ciphertext),
        nonce: STANDARD.encode(nonce_bytes),
        hint: credential
            .chars()
            .rev()
            .take(8)
            .collect::<String>()
            .chars()
            .rev()
            .collect(),
    })
}

pub fn reveal_installation_credential(
    ciphertext: &str,
    nonce: &str,
    tenant_id: Uuid,
    deployment_id: Uuid,
    config: &LicenseConfig,
) -> Result<String> {
    let cipher = credential_cipher(config)?;
    let ciphertext = STANDARD
        .decode(ciphertext)
        .context("Stored installation credential is invalid")?;
    let nonce = STANDARD
        .decode(nonce)
        .context("Stored installation credential nonce is invalid")?;
    if nonce.len() != 12 {
        bail!("Stored installation credential nonce is invalid");
    }
    let aad = credential_aad(tenant_id, deployment_id);
    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &ciphertext,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| anyhow::anyhow!("Stored installation credential could not be opened"))?;
    String::from_utf8(plaintext).context("Stored installation credential is not valid text")
}

fn credential_cipher(config: &LicenseConfig) -> Result<Aes256Gcm> {
    let encoded_key = config
        .credential_key_base64
        .as_deref()
        .context("License credential encryption is not configured")?;
    let key = STANDARD
        .decode(encoded_key)
        .context("Configured license credential key is invalid")?;
    if key.len() != 32 {
        bail!("Configured license credential key must contain 32 bytes");
    }
    Aes256Gcm::new_from_slice(&key)
        .map_err(|_| anyhow::anyhow!("Configured license credential key is invalid"))
}

fn credential_aad(tenant_id: Uuid, deployment_id: Uuid) -> String {
    format!("cp-license-credential/v1:{tenant_id}:{deployment_id}")
}

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use chrono::Utc;
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
    use uuid::Uuid;

    use crate::config::LicenseConfig;

    use super::{
        LeaseLimit, SignedLeaseClaims, app_version_is_supported, protect_installation_credential,
        reveal_installation_credential, verify_signed_lease,
    };

    const PRIVATE_KEY: &str = "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEIL9PtNqTMRWH3/0tsQRAHSoduxipswZZSjKkMtpWweJd\n-----END PRIVATE KEY-----\n";
    const PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEAxbbyLLpJQoSoH8ia0Xw/lZTAUKtokEiy8l27VZND2zI=\n-----END PUBLIC KEY-----\n";

    fn config() -> LicenseConfig {
        LicenseConfig {
            public_key_base64: None,
            issuer: "campus-pilot-control-plane".to_string(),
            audience: "campus-pilot".to_string(),
            control_plane_url: Some("https://licenses.example.test".to_string()),
            credential_key_base64: Some(STANDARD.encode([7_u8; 32])),
            installation_name: "Test server".to_string(),
        }
    }

    #[test]
    fn credential_round_trip_is_bound_to_installation_identity() {
        let tenant_id = Uuid::new_v4();
        let deployment_id = Uuid::new_v4();
        let protected = protect_installation_credential(
            "cpinst_private_value",
            tenant_id,
            deployment_id,
            &config(),
        );
        assert!(protected.is_ok());
        let protected = protected.unwrap_or_else(|_| unreachable!());
        let opened = reveal_installation_credential(
            &protected.ciphertext,
            &protected.nonce,
            tenant_id,
            deployment_id,
            &config(),
        );
        assert!(opened.is_ok_and(|value| value == "cpinst_private_value"));
        assert_eq!(protected.hint, "te_value");
        assert!(
            reveal_installation_credential(
                &protected.ciphertext,
                &protected.nonce,
                Uuid::new_v4(),
                deployment_id,
                &config(),
            )
            .is_err()
        );
    }

    #[test]
    fn credential_key_must_be_exactly_32_bytes() {
        let mut invalid = config();
        invalid.credential_key_base64 = Some(STANDARD.encode([1_u8; 16]));
        assert!(
            protect_installation_credential(
                "cpinst_private_value",
                Uuid::new_v4(),
                Uuid::new_v4(),
                &invalid,
            )
            .is_err()
        );
    }

    #[test]
    fn signed_lease_verification_binds_registered_and_installation_claims() {
        let tenant_id = Uuid::new_v4();
        let installation_id = Uuid::new_v4();
        let now = Utc::now().timestamp();
        let claims = SignedLeaseClaims {
            contract_version: "cp-license/v1".to_string(),
            iss: "campus-pilot-control-plane".to_string(),
            aud: "campus-pilot".to_string(),
            sub: tenant_id.to_string(),
            installation_id: installation_id.to_string(),
            jti: Uuid::new_v4().to_string(),
            sequence: 7,
            catalog_version: "plans/complete/1".to_string(),
            iat: now,
            nbf: now - 30,
            refresh_after: now + 300,
            lease_expires_at: now + 600,
            grace_until: now + 900,
            exp: now + 900,
            modules: vec!["agent".to_string()],
            features: vec!["agent.sessions".to_string()],
            limits: vec![LeaseLimit {
                key: "agent.requests".to_string(),
                unit: "request".to_string(),
                period: "month".to_string(),
                value: 100,
                enforcement: "report".to_string(),
            }],
            min_app_version: None,
            max_app_version: None,
        };
        let header = Header {
            alg: Algorithm::EdDSA,
            kid: Some("test-key-1".to_string()),
            ..Header::default()
        };
        let token = encode(
            &header,
            &claims,
            &EncodingKey::from_ed_pem(PRIVATE_KEY.as_bytes()).unwrap_or_else(|_| unreachable!()),
        )
        .unwrap_or_else(|_| unreachable!());
        let mut license_config = config();
        license_config.public_key_base64 = Some(STANDARD.encode(PUBLIC_KEY.as_bytes()));

        let verified =
            verify_signed_lease(&token, tenant_id, Some(installation_id), &license_config);
        assert!(
            verified
                .is_ok_and(|lease| { lease.key_id == "test-key-1" && lease.claims.sequence == 7 })
        );
        assert!(
            verify_signed_lease(
                &token,
                Uuid::new_v4(),
                Some(installation_id),
                &license_config,
            )
            .is_err()
        );
        assert!(
            verify_signed_lease(&token, tenant_id, Some(Uuid::new_v4()), &license_config,).is_err()
        );

        let mut duplicate_modules = claims.clone();
        duplicate_modules.modules.push("agent".to_string());
        let mut duplicate_features = claims.clone();
        duplicate_features
            .features
            .push("agent.sessions".to_string());
        let mut duplicate_limits = claims.clone();
        duplicate_limits
            .limits
            .push(duplicate_limits.limits[0].clone());
        let mut invalid_feature = claims.clone();
        invalid_feature.features = vec!["Agent Sessions".to_string()];
        let mut oversized_limit = claims.clone();
        oversized_limit.limits[0].value = u64::MAX;
        for invalid_claims in [
            duplicate_modules,
            duplicate_features,
            duplicate_limits,
            invalid_feature,
            oversized_limit,
        ] {
            let invalid_token = encode(
                &header,
                &invalid_claims,
                &EncodingKey::from_ed_pem(PRIVATE_KEY.as_bytes())
                    .unwrap_or_else(|_| unreachable!()),
            )
            .unwrap_or_else(|_| unreachable!());
            assert!(
                verify_signed_lease(
                    &invalid_token,
                    tenant_id,
                    Some(installation_id),
                    &license_config,
                )
                .is_err()
            );
        }

        let mut unsupported = claims.clone();
        unsupported.min_app_version = Some("2.0.0".to_string());
        assert!(app_version_is_supported(&unsupported).is_ok_and(|supported| !supported));

        let mut invalid_bounds = claims;
        invalid_bounds.min_app_version = Some("2.0.0".to_string());
        invalid_bounds.max_app_version = Some("1.0.0".to_string());
        let invalid_token = encode(
            &header,
            &invalid_bounds,
            &EncodingKey::from_ed_pem(PRIVATE_KEY.as_bytes()).unwrap_or_else(|_| unreachable!()),
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(
            verify_signed_lease(
                &invalid_token,
                tenant_id,
                Some(installation_id),
                &license_config,
            )
            .is_err()
        );
    }
}
