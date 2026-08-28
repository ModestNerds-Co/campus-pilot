//! Encrypts provider credentials with versioned tenant- and connection-bound AAD.
//!
//! Deployment supplies only a keyring and active key identifier. Each rotation
//! uses a fresh AES-256-GCM nonce, and old key identifiers remain decryptable.
//!
//! Copyright (c) 2026 Codecraft Solutions. All rights reserved.

use std::{collections::BTreeMap, fmt, sync::Arc};

use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, AeadCore, OsRng, Payload},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use thiserror::Error;
use uuid::Uuid;

use crate::types::{ApiKey, AuthMethod, ProviderKey};

const ENVELOPE_VERSION: u8 = 1;
const AES_GCM_NONCE_LENGTH: usize = 12;

/// Parsed deployment keyring with one active encryption key.
#[derive(Clone)]
pub struct CredentialKeyring {
    keys: Arc<BTreeMap<String, [u8; 32]>>,
    active_key_id: String,
}

impl fmt::Debug for CredentialKeyring {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialKeyring")
            .field("key_count", &self.keys.len())
            .field("active_key_id", &self.active_key_id)
            .finish_non_exhaustive()
    }
}

impl CredentialKeyring {
    /// Parses standard-base64 32-byte AES keys and proves the active key exists.
    pub fn from_base64(
        configured_keys: BTreeMap<String, String>,
        active_key_id: impl Into<String>,
    ) -> Result<Self, KeyringError> {
        let active_key_id = active_key_id.into();
        validate_key_id(&active_key_id)?;
        if configured_keys.is_empty() {
            return Err(KeyringError::Empty);
        }

        let mut keys = BTreeMap::new();
        for (key_id, encoded_key) in configured_keys {
            validate_key_id(&key_id)?;
            let decoded = STANDARD
                .decode(encoded_key.trim())
                .map_err(|_| KeyringError::InvalidKeyMaterial(key_id.clone()))?;
            let key: [u8; 32] = decoded
                .try_into()
                .map_err(|_| KeyringError::InvalidKeyMaterial(key_id.clone()))?;
            keys.insert(key_id, key);
        }
        if !keys.contains_key(&active_key_id) {
            return Err(KeyringError::MissingActiveKey(active_key_id));
        }
        Ok(Self {
            keys: Arc::new(keys),
            active_key_id,
        })
    }

    /// Returns whether this deployment can decrypt envelopes using `key_id`.
    #[must_use]
    pub fn contains_key_id(&self, key_id: &str) -> bool {
        self.keys.contains_key(key_id)
    }

    pub(crate) fn encrypt(
        &self,
        context: CredentialContext<'_>,
        api_key: &ApiKey,
    ) -> Result<EncryptedCredential, KeyringError> {
        let key = self
            .keys
            .get(&self.active_key_id)
            .ok_or_else(|| KeyringError::MissingActiveKey(self.active_key_id.clone()))?;
        let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| KeyringError::Cipher)?;
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let aad = context.aad();
        let ciphertext = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: api_key.expose().as_bytes(),
                    aad: &aad,
                },
            )
            .map_err(|_| KeyringError::Encrypt)?;
        Ok(EncryptedCredential {
            ciphertext,
            nonce: nonce.to_vec(),
            key_id: self.active_key_id.clone(),
            envelope_version: i16::from(ENVELOPE_VERSION),
        })
    }

    pub(crate) fn decrypt(
        &self,
        context: CredentialContext<'_>,
        encrypted: &EncryptedCredential,
    ) -> Result<ApiKey, KeyringError> {
        if encrypted.envelope_version != i16::from(ENVELOPE_VERSION)
            || encrypted.nonce.len() != AES_GCM_NONCE_LENGTH
        {
            return Err(KeyringError::UnsupportedEnvelope);
        }
        let key = self
            .keys
            .get(&encrypted.key_id)
            .ok_or_else(|| KeyringError::UnknownStoredKey(encrypted.key_id.clone()))?;
        let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| KeyringError::Cipher)?;
        let aad = context.aad();
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(&encrypted.nonce),
                Payload {
                    msg: &encrypted.ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|_| KeyringError::Decrypt)?;
        let plaintext = String::from_utf8(plaintext).map_err(|_| KeyringError::Decrypt)?;
        ApiKey::parse(plaintext).map_err(|_| KeyringError::Decrypt)
    }
}

fn validate_key_id(key_id: &str) -> Result<(), KeyringError> {
    if key_id.is_empty()
        || key_id.len() > 128
        || !key_id.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        return Err(KeyringError::InvalidKeyId);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CredentialContext<'a> {
    pub tenant_id: Uuid,
    pub connection_id: Uuid,
    pub provider: ProviderKey,
    pub auth_method: AuthMethod,
    pub credential_version: i64,
    pub domain: &'a str,
}

impl CredentialContext<'_> {
    fn aad(self) -> Vec<u8> {
        let mut aad = Vec::with_capacity(128);
        aad.extend_from_slice(self.domain.as_bytes());
        aad.push(0);
        aad.push(ENVELOPE_VERSION);
        aad.extend_from_slice(self.tenant_id.as_bytes());
        aad.extend_from_slice(self.connection_id.as_bytes());
        aad.extend_from_slice(self.provider.as_str().as_bytes());
        aad.push(0);
        aad.extend_from_slice(self.auth_method.as_str().as_bytes());
        aad.push(0);
        aad.extend_from_slice(&self.credential_version.to_be_bytes());
        aad
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EncryptedCredential {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub key_id: String,
    pub envelope_version: i16,
}

/// Deployment keyring and authenticated-encryption failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum KeyringError {
    #[error("credential keyring is empty")]
    Empty,
    #[error("credential key identifier is invalid")]
    InvalidKeyId,
    #[error("credential key material is invalid for {0}")]
    InvalidKeyMaterial(String),
    #[error("active credential key is missing: {0}")]
    MissingActiveKey(String),
    #[error("stored credential key is unavailable: {0}")]
    UnknownStoredKey(String),
    #[error("credential envelope version is unsupported")]
    UnsupportedEnvelope,
    #[error("credential cipher initialization failed")]
    Cipher,
    #[error("credential encryption failed")]
    Encrypt,
    #[error("credential authentication failed")]
    Decrypt,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use uuid::Uuid;

    use crate::types::{ApiKey, AuthMethod, ProviderKey};

    use super::{CredentialContext, CredentialKeyring, KeyringError};

    const DOMAIN: &str = "campus-pilot/ai-provider-credential";

    fn keyring(active: &str) -> CredentialKeyring {
        CredentialKeyring::from_base64(
            BTreeMap::from([
                ("first".to_owned(), STANDARD.encode([7_u8; 32])),
                ("second".to_owned(), STANDARD.encode([9_u8; 32])),
            ]),
            active,
        )
        .unwrap()
    }

    fn context(tenant_id: Uuid, connection_id: Uuid, version: i64) -> CredentialContext<'static> {
        CredentialContext {
            tenant_id,
            connection_id,
            provider: ProviderKey::OpenAi,
            auth_method: AuthMethod::ApiKey,
            credential_version: version,
            domain: DOMAIN,
        }
    }

    #[test]
    fn credentials_round_trip_with_fresh_nonces_and_old_keys() {
        let tenant_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();
        let secret = ApiKey::parse("secret-key-material-123").unwrap();
        let first_keyring = keyring("first");
        let first = first_keyring
            .encrypt(context(tenant_id, connection_id, 1), &secret)
            .unwrap();
        let second = first_keyring
            .encrypt(context(tenant_id, connection_id, 1), &secret)
            .unwrap();
        assert_ne!(first.nonce, second.nonce);

        let rotated_keyring = keyring("second");
        assert_eq!(
            rotated_keyring
                .decrypt(context(tenant_id, connection_id, 1), &first)
                .unwrap()
                .expose(),
            secret.expose()
        );
    }

    #[test]
    fn aad_rejects_cross_tenant_connection_provider_and_version_replay() {
        let tenant_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();
        let secret = ApiKey::parse("secret-key-material-123").unwrap();
        let keyring = keyring("first");
        let encrypted = keyring
            .encrypt(context(tenant_id, connection_id, 4), &secret)
            .unwrap();

        assert!(
            keyring
                .decrypt(context(Uuid::new_v4(), connection_id, 4), &encrypted)
                .is_err()
        );
        assert!(
            keyring
                .decrypt(context(tenant_id, Uuid::new_v4(), 4), &encrypted)
                .is_err()
        );
        assert!(
            keyring
                .decrypt(context(tenant_id, connection_id, 5), &encrypted)
                .is_err()
        );

        let provider_mismatch = CredentialContext {
            provider: ProviderKey::Anthropic,
            ..context(tenant_id, connection_id, 4)
        };
        assert!(keyring.decrypt(provider_mismatch, &encrypted).is_err());
    }

    #[test]
    fn keyring_configuration_rejects_invalid_keys_and_active_ids() {
        assert_eq!(
            CredentialKeyring::from_base64(BTreeMap::new(), "first").unwrap_err(),
            KeyringError::Empty
        );
        assert!(
            CredentialKeyring::from_base64(
                BTreeMap::from([("first".to_owned(), STANDARD.encode([1_u8; 31]))]),
                "first"
            )
            .is_err()
        );
        assert!(
            CredentialKeyring::from_base64(
                BTreeMap::from([("first".to_owned(), STANDARD.encode([1_u8; 32]))]),
                "missing"
            )
            .is_err()
        );
        assert!(
            CredentialKeyring::from_base64(
                BTreeMap::from([("bad key".to_owned(), STANDARD.encode([1_u8; 32]))]),
                "bad key"
            )
            .is_err()
        );
    }
}
