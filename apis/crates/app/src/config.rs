//
//  campus-pilot-apis
//  config.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/30.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use cp_agent_runtime::ArtifactKeyring;
use cp_ai_providers::{CredentialKeyring, ProviderHttpClient};
use jsonwebtoken::DecodingKey;
use serde::Deserialize;
use std::{collections::BTreeMap, env};
use urlencoding::encode;

#[derive(Debug, Clone)]
pub struct Config {
    pub app: AppConfig,
    pub database: DatabaseConfig,
    pub storage: StorageConfig,
    pub jwt: JwtConfig,
    pub license: LicenseConfig,
    pub ai_providers: AiProviderConfig,
    pub agent: AgentConfig,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub port: u16,
    pub sentry_dsn: String,
}

#[derive(Debug, Clone)]
pub struct JwtConfig {
    pub secret: String,
}

#[derive(Debug, Clone)]
pub struct LicenseConfig {
    pub trusted_public_keys: BTreeMap<String, String>,
    pub issuer: String,
    pub audience: String,
    pub control_plane_url: Option<String>,
    pub credential_key_base64: Option<String>,
    pub installation_name: String,
}

#[derive(Debug, Clone)]
pub struct AiProviderConfig {
    pub credential_keyring: Option<CredentialKeyring>,
    pub http_client: ProviderHttpClient,
}

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub artifact_keyring: Option<ArtifactKeyring>,
}

#[derive(Deserialize)]
struct ConfiguredAgentArtifactKey {
    version: i64,
    key_base64: String,
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub endpoint: String,
    pub public_endpoint: Option<String>,
    pub region: String,
    pub bucket: String,
    pub private_bucket: String,
    pub document_scanner_address: String,
    pub access_key: String,
    pub secret_key: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let app = AppConfig::from_env()?;
        let database = DatabaseConfig::from_env()?;
        let storage = StorageConfig::from_env()?;
        let jwt = JwtConfig::from_env()?;
        let license = LicenseConfig::from_env()?;
        let ai_providers = AiProviderConfig::from_env()?;
        let agent = AgentConfig::from_env()?;

        Ok(Config {
            app,
            database,
            storage,
            jwt,
            license,
            ai_providers,
            agent,
        })
    }
}

impl AgentConfig {
    fn from_env() -> Result<Self> {
        let configured_keys = env::var("AGENT_ARTIFACT_KEYS_JSON")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let active_key_id = env::var("AGENT_ARTIFACT_ACTIVE_KEY_ID")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let artifact_keyring =
            agent_artifact_keyring(configured_keys.as_deref(), active_key_id.as_deref())?;
        Ok(Self { artifact_keyring })
    }
}

fn agent_artifact_keyring(
    configured_keys: Option<&str>,
    active_key_id: Option<&str>,
) -> Result<Option<ArtifactKeyring>> {
    match (configured_keys, active_key_id) {
        (None, None) => Ok(None),
        (Some(configured_keys), Some(active_key_id)) => {
            let configured = serde_json::from_str::<BTreeMap<String, ConfiguredAgentArtifactKey>>(
                configured_keys,
            )
            .context("AGENT_ARTIFACT_KEYS_JSON must be a JSON object")?;
            let keys = configured
                .into_iter()
                .map(|(key_id, key)| (key_id, (key.version, key.key_base64)))
                .collect();
            ArtifactKeyring::from_base64(keys, active_key_id)
                .context("Agent artifact keyring is invalid")
                .map(Some)
        }
        (Some(_), None) => {
            bail!("AGENT_ARTIFACT_ACTIVE_KEY_ID must be set with the Agent artifact keyring")
        }
        (None, Some(_)) => {
            bail!("AGENT_ARTIFACT_KEYS_JSON must be set with the active key identifier")
        }
    }
}

impl AiProviderConfig {
    fn from_env() -> Result<Self> {
        let configured_keys = env::var("AI_PROVIDER_CREDENTIAL_KEYS_JSON")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let active_key_id = env::var("AI_PROVIDER_CREDENTIAL_ACTIVE_KEY_ID")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let credential_keyring =
            ai_provider_credential_keyring(configured_keys.as_deref(), active_key_id.as_deref())?;
        let http_client = ProviderHttpClient::new()
            .context("build the bounded AI provider administration HTTP client")?;
        Ok(Self {
            credential_keyring,
            http_client,
        })
    }
}

fn ai_provider_credential_keyring(
    configured_keys: Option<&str>,
    active_key_id: Option<&str>,
) -> Result<Option<CredentialKeyring>> {
    match (configured_keys, active_key_id) {
        (None, None) => Ok(None),
        (Some(configured_keys), Some(active_key_id)) => {
            let keys = serde_json::from_str::<BTreeMap<String, String>>(configured_keys)
                .context("AI_PROVIDER_CREDENTIAL_KEYS_JSON must be a JSON object")?;
            CredentialKeyring::from_base64(keys, active_key_id)
                .context("AI provider credential keyring is invalid")
                .map(Some)
        }
        (Some(_), None) => {
            bail!("AI_PROVIDER_CREDENTIAL_ACTIVE_KEY_ID must be set with the credential keyring")
        }
        (None, Some(_)) => {
            bail!("AI_PROVIDER_CREDENTIAL_KEYS_JSON must be set with the active key identifier")
        }
    }
}

impl AppConfig {
    fn from_env() -> Result<Self> {
        let port = env::var("APP_PORT")
            .context("APP_PORT must be set")?
            .parse::<u16>()
            .context("APP_PORT must be a valid port number")?;
        let sentry_dsn = env::var("SENTRY_DSN")
            .context("SENTRY_DSN must be set")?
            .parse::<String>()
            .context("SENTRY_DSN must be a valid port number")?;

        Ok(AppConfig { port, sentry_dsn })
    }
}

impl DatabaseConfig {
    fn from_env() -> Result<Self> {
        let user = std::env::var("DB_USER")?;
        let pass = std::env::var("DB_PASS")?;
        let host = std::env::var("DB_HOST")?;
        let port = std::env::var("DB_PORT")?;
        let db = std::env::var("DB_NAME")?;

        let url = format!(
            "postgresql://{}:{}@{}:{}/{}",
            encode(&user),
            encode(&pass),
            host,
            port,
            db
        );

        Ok(DatabaseConfig { url })
    }
}

impl StorageConfig {
    fn from_env() -> Result<Self> {
        let endpoint = env::var("STORAGE_ENDPOINT").context("STORAGE_ENDPOINT must be set")?;
        let public_endpoint = env::var("STORAGE_PUBLIC_ENDPOINT").ok();
        let region = env::var("STORAGE_REGION").unwrap_or_else(|_| "us-east-1".to_string());
        let bucket = env::var("STORAGE_BUCKET").context("STORAGE_BUCKET must be set")?;
        let private_bucket =
            env::var("STORAGE_PRIVATE_BUCKET").unwrap_or_else(|_| format!("{bucket}-private"));
        if private_bucket == bucket {
            bail!("STORAGE_PRIVATE_BUCKET must differ from the public STORAGE_BUCKET");
        }
        let document_scanner_address =
            env::var("DOCUMENT_SCANNER_ADDRESS").unwrap_or_else(|_| "clamav:3310".to_string());
        let access_key =
            env::var("STORAGE_ACCESS_KEY").context("STORAGE_ACCESS_KEY must be set")?;
        let secret_key =
            env::var("STORAGE_SECRET_KEY").context("STORAGE_SECRET_KEY must be set")?;

        Ok(StorageConfig {
            endpoint,
            public_endpoint,
            region,
            bucket,
            private_bucket,
            document_scanner_address,
            access_key,
            secret_key,
        })
    }
}

impl JwtConfig {
    fn from_env() -> Result<Self> {
        let secret = env::var("JWT_SECRET").context("JWT_SECRET must be set")?;
        Ok(JwtConfig { secret })
    }
}

impl LicenseConfig {
    fn from_env() -> Result<Self> {
        let legacy_public_key = env::var("LICENSE_PUBLIC_KEY_BASE64")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let legacy_public_key_id = env::var("LICENSE_PUBLIC_KEY_ID")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let configured_keyring = env::var("LICENSE_TRUSTED_PUBLIC_KEYS_JSON")
            .ok()
            .filter(|value| !value.trim().is_empty());
        Ok(Self {
            trusted_public_keys: trusted_public_keys(
                configured_keyring.as_deref(),
                legacy_public_key_id.as_deref(),
                legacy_public_key.as_deref(),
            )?,
            issuer: env::var("LICENSE_ISSUER")
                .unwrap_or_else(|_| "campus-pilot-control-plane".to_string()),
            audience: env::var("LICENSE_AUDIENCE").unwrap_or_else(|_| "campus-pilot".to_string()),
            control_plane_url: env::var("LICENSE_CONTROL_PLANE_URL")
                .ok()
                .map(|value| value.trim_end_matches('/').to_string())
                .filter(|value| !value.is_empty()),
            credential_key_base64: env::var("LICENSE_CREDENTIAL_KEY_BASE64")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            installation_name: env::var("LICENSE_INSTALLATION_NAME")
                .unwrap_or_else(|_| "Campus Pilot server".to_string()),
        })
    }

    #[must_use]
    pub fn verification_is_configured(&self) -> bool {
        !self.trusted_public_keys.is_empty()
    }
}

fn trusted_public_keys(
    configured_keyring: Option<&str>,
    legacy_key_id: Option<&str>,
    legacy_key: Option<&str>,
) -> Result<BTreeMap<String, String>> {
    let mut keys = configured_keyring.map_or_else(
        || Ok(BTreeMap::new()),
        |value| {
            serde_json::from_str::<BTreeMap<String, String>>(value)
                .context("LICENSE_TRUSTED_PUBLIC_KEYS_JSON must be a JSON object")
        },
    )?;
    for (key_id, public_key) in &keys {
        validate_key_entry(key_id, public_key)?;
    }

    if let Some(public_key) = legacy_key {
        let key_id = legacy_key_id.context(
            "LICENSE_PUBLIC_KEY_ID must be set when LICENSE_PUBLIC_KEY_BASE64 is configured",
        )?;
        validate_key_entry(key_id, public_key)?;
        match keys.get(key_id) {
            Some(configured) if configured != public_key => {
                bail!("License public key identifier is configured with conflicting keys")
            }
            Some(_) => {}
            None => {
                keys.insert(key_id.to_string(), public_key.to_string());
            }
        }
    } else if legacy_key_id.is_some() {
        bail!("LICENSE_PUBLIC_KEY_BASE64 must be set when LICENSE_PUBLIC_KEY_ID is configured");
    }

    Ok(keys)
}

fn validate_key_entry(key_id: &str, public_key: &str) -> Result<()> {
    if key_id.is_empty()
        || key_id.len() > 128
        || !key_id.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        bail!("License public key identifier is invalid");
    }
    if public_key.trim().is_empty() {
        bail!("License public key is empty");
    }
    let public_key = STANDARD
        .decode(public_key)
        .context("Configured license public key is invalid")?;
    DecodingKey::from_ed_pem(&public_key)
        .context("Configured license public key is not valid Ed25519 PEM")?;
    Ok(())
}

#[cfg(test)]
mod license_tests {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    use super::trusted_public_keys;

    const PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEAxbbyLLpJQoSoH8ia0Xw/lZTAUKtokEiy8l27VZND2zI=\n-----END PUBLIC KEY-----\n";
    const ROTATED_PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEArVSef1/+8dsF8OsxRrBs6q6+hRI7leppr00NTz3n2NA=\n-----END PUBLIC KEY-----\n";

    #[test]
    fn trusted_keyring_merges_the_legacy_active_key_without_ambiguity() {
        let previous = STANDARD.encode(PUBLIC_KEY.as_bytes());
        let active = STANDARD.encode(ROTATED_PUBLIC_KEY.as_bytes());
        let configured = serde_json::to_string(&std::collections::BTreeMap::from([(
            "production-0",
            previous.as_str(),
        )]))
        .unwrap_or_else(|_| unreachable!());
        let keys = trusted_public_keys(Some(&configured), Some("production-1"), Some(&active))
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            keys.get("production-0").map(String::as_str),
            Some(previous.as_str())
        );
        assert_eq!(
            keys.get("production-1").map(String::as_str),
            Some(active.as_str())
        );

        let conflicting = serde_json::to_string(&std::collections::BTreeMap::from([(
            "production-1",
            previous.as_str(),
        )]))
        .unwrap_or_else(|_| unreachable!());
        assert!(
            trusted_public_keys(Some(&conflicting), Some("production-1"), Some(&active),).is_err()
        );
        assert!(trusted_public_keys(None, None, Some(&active)).is_err());
        assert!(trusted_public_keys(Some("[]"), None, None).is_err());
        assert!(trusted_public_keys(None, Some("production-1"), Some("not-base64")).is_err());
    }
}

#[cfg(test)]
mod ai_provider_tests {
    use std::collections::BTreeMap;

    use base64::{Engine as _, engine::general_purpose::STANDARD};

    use super::ai_provider_credential_keyring;

    #[test]
    fn provider_keyring_is_optional_but_cannot_be_partially_configured() {
        assert!(
            ai_provider_credential_keyring(None, None)
                .unwrap_or_else(|_| unreachable!())
                .is_none()
        );
        assert!(ai_provider_credential_keyring(Some("{}"), None).is_err());
        assert!(ai_provider_credential_keyring(None, Some("production-1")).is_err());
    }

    #[test]
    fn provider_keyring_requires_a_32_byte_active_key() {
        let key = STANDARD.encode([7_u8; 32]);
        let configured = serde_json::to_string(&BTreeMap::from([("production-1", key)]))
            .unwrap_or_else(|_| unreachable!());
        assert!(
            ai_provider_credential_keyring(Some(&configured), Some("production-1"))
                .unwrap_or_else(|_| unreachable!())
                .is_some()
        );
        assert!(ai_provider_credential_keyring(Some(&configured), Some("missing")).is_err());
        assert!(ai_provider_credential_keyring(Some("[]"), Some("production-1")).is_err());
    }
}

#[cfg(test)]
mod agent_artifact_tests {
    use std::collections::BTreeMap;

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use serde_json::json;

    use super::agent_artifact_keyring;

    #[test]
    fn artifact_keyring_is_optional_but_cannot_be_partially_configured() {
        assert!(
            agent_artifact_keyring(None, None)
                .unwrap_or_else(|_| unreachable!())
                .is_none()
        );
        assert!(agent_artifact_keyring(Some("{}"), None).is_err());
        assert!(agent_artifact_keyring(None, Some("production-1")).is_err());
    }

    #[test]
    fn artifact_keyring_requires_versioned_32_byte_active_keys() {
        let configured = serde_json::to_string(&BTreeMap::from([(
            "production-1",
            json!({
                "version": 1,
                "key_base64": STANDARD.encode([7_u8; 32]),
            }),
        )]))
        .unwrap_or_else(|_| unreachable!());
        let keyring = agent_artifact_keyring(Some(&configured), Some("production-1"))
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert!(keyring.contains_key("production-1", 1));
        assert!(agent_artifact_keyring(Some(&configured), Some("missing")).is_err());

        let zero_version = configured.replace("\"version\":1", "\"version\":0");
        assert!(agent_artifact_keyring(Some(&zero_version), Some("production-1")).is_err());
        assert!(agent_artifact_keyring(Some("[]"), Some("production-1")).is_err());
    }
}
