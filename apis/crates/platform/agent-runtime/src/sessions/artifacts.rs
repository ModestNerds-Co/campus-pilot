//! Encrypts private Agent continuation artifacts with versioned, execution-bound AAD.
//!
//! Provider and capability results are bound to their durable execution step. Final responses
//! are uniquely bound to the run because the Session repository allocates their step while it
//! persists the already-encrypted envelope. Plaintext and key material never implement `Debug`
//! or serialization.

use std::{collections::BTreeMap, fmt, sync::Arc};

use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, AeadCore, OsRng, Payload},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use super::types::{EncryptedExecutionArtifact, ExecutionArtifactKind, LoadedExecutionArtifact};

const AAD_DOMAIN: &[u8] = b"campus-pilot/agent-execution-artifact";
const ENVELOPE_VERSION: u8 = 1;
const AES_GCM_NONCE_LENGTH: usize = 12;
const MAX_KEY_ID_BYTES: usize = 128;
const MAX_PLAINTEXT_BYTES: usize = 65_536;

#[derive(Clone)]
struct ArtifactKey {
    version: i64,
    material: [u8; 32],
}

/// Parsed deployment keyring with one active Agent-artifact encryption key.
#[derive(Clone)]
pub struct ArtifactKeyring {
    keys: Arc<BTreeMap<String, ArtifactKey>>,
    active_key_id: String,
}

impl fmt::Debug for ArtifactKeyring {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ArtifactKeyring")
            .field("key_count", &self.keys.len())
            .field("active_key_id", &self.active_key_id)
            .finish_non_exhaustive()
    }
}

impl ArtifactKeyring {
    /// Parses standard-base64 AES-256 keys and positive per-key versions.
    ///
    /// The tuple is `(version, base64_key_material)`. A rotated deployment retains old entries
    /// until every artifact encrypted with them has expired from durable run history.
    pub fn from_base64(
        configured_keys: BTreeMap<String, (i64, String)>,
        active_key_id: impl Into<String>,
    ) -> Result<Self, ArtifactKeyringError> {
        let active_key_id = active_key_id.into();
        validate_key_id(&active_key_id)?;
        if configured_keys.is_empty() {
            return Err(ArtifactKeyringError::Empty);
        }

        let mut keys = BTreeMap::new();
        for (key_id, (version, encoded_material)) in configured_keys {
            validate_key_id(&key_id)?;
            if version <= 0 {
                return Err(ArtifactKeyringError::InvalidKeyVersion(key_id));
            }
            let decoded = STANDARD
                .decode(encoded_material.trim())
                .map_err(|_| ArtifactKeyringError::InvalidKeyMaterial(key_id.clone()))?;
            let material = decoded
                .try_into()
                .map_err(|_| ArtifactKeyringError::InvalidKeyMaterial(key_id.clone()))?;
            keys.insert(key_id, ArtifactKey { version, material });
        }
        if !keys.contains_key(&active_key_id) {
            return Err(ArtifactKeyringError::MissingActiveKey(active_key_id));
        }
        Ok(Self {
            keys: Arc::new(keys),
            active_key_id,
        })
    }

    /// Returns whether the keyring can decrypt the exact stored key identity and version.
    #[must_use]
    pub fn contains_key(&self, key_id: &str, key_version: i64) -> bool {
        self.keys
            .get(key_id)
            .is_some_and(|key| key.version == key_version)
    }

    /// Encrypts a bounded continuation value using fresh AES-256-GCM nonce material.
    pub fn encrypt(
        &self,
        binding: ArtifactBinding,
        plaintext: &[u8],
    ) -> Result<EncryptedExecutionArtifact, ArtifactKeyringError> {
        if plaintext.is_empty() || plaintext.len() > MAX_PLAINTEXT_BYTES {
            return Err(ArtifactKeyringError::InvalidPlaintextLength);
        }
        let key = self
            .keys
            .get(&self.active_key_id)
            .ok_or_else(|| ArtifactKeyringError::MissingActiveKey(self.active_key_id.clone()))?;
        let cipher =
            Aes256Gcm::new_from_slice(&key.material).map_err(|_| ArtifactKeyringError::Cipher)?;
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let plaintext_sha256: [u8; 32] = Sha256::digest(plaintext).into();
        let aad = binding.aad(
            &self.active_key_id,
            key.version,
            &plaintext_sha256,
            plaintext.len(),
        );
        let ciphertext = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| ArtifactKeyringError::Encrypt)?;
        EncryptedExecutionArtifact::parse(
            ciphertext,
            plaintext_sha256,
            nonce.to_vec(),
            &self.active_key_id,
            key.version,
            plaintext.len(),
        )
        .map_err(|_| ArtifactKeyringError::InvalidArtifact)
    }

    /// Decrypts a loaded envelope after deriving its exact durable execution binding.
    pub fn decrypt_loaded(
        &self,
        tenant_id: Uuid,
        run_id: Uuid,
        loaded: LoadedExecutionArtifact,
    ) -> Result<DecryptedExecutionArtifact, ArtifactKeyringError> {
        let binding = ArtifactBinding::for_loaded(tenant_id, run_id, loaded.step_id, loaded.kind);
        self.decrypt(binding, loaded.into_envelope())
    }

    /// Decrypts an envelope only under the same tenant, run, step, and kind binding used to
    /// create it. This is public for immediate lost-ack recovery before a fresh snapshot load.
    pub fn decrypt(
        &self,
        binding: ArtifactBinding,
        encrypted: EncryptedExecutionArtifact,
    ) -> Result<DecryptedExecutionArtifact, ArtifactKeyringError> {
        if encrypted.nonce().len() != AES_GCM_NONCE_LENGTH {
            return Err(ArtifactKeyringError::UnsupportedEnvelope);
        }
        let key = self
            .keys
            .get(encrypted.encryption_key_id())
            .ok_or_else(|| {
                ArtifactKeyringError::UnknownStoredKey(encrypted.encryption_key_id().to_owned())
            })?;
        if key.version != encrypted.encryption_key_version() {
            return Err(ArtifactKeyringError::UnsupportedEnvelope);
        }
        let plaintext_length = usize::try_from(encrypted.plaintext_length())
            .map_err(|_| ArtifactKeyringError::InvalidArtifact)?;
        let aad = binding.aad(
            encrypted.encryption_key_id(),
            encrypted.encryption_key_version(),
            encrypted.plaintext_sha256(),
            plaintext_length,
        );
        let cipher =
            Aes256Gcm::new_from_slice(&key.material).map_err(|_| ArtifactKeyringError::Cipher)?;
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(encrypted.nonce()),
                Payload {
                    msg: encrypted.ciphertext(),
                    aad: &aad,
                },
            )
            .map_err(|_| ArtifactKeyringError::Decrypt)?;
        if plaintext.len() != plaintext_length
            || <[u8; 32]>::from(Sha256::digest(&plaintext)) != *encrypted.plaintext_sha256()
        {
            return Err(ArtifactKeyringError::Decrypt);
        }
        Ok(DecryptedExecutionArtifact(plaintext))
    }
}

/// Authenticated identity for one encrypted continuation artifact.
#[derive(Clone, Copy)]
pub struct ArtifactBinding {
    tenant_id: Uuid,
    run_id: Uuid,
    step: ArtifactStepBinding,
}

#[derive(Clone, Copy)]
enum ArtifactStepBinding {
    ExecutionStep {
        step_id: Uuid,
        kind: ExecutionArtifactKind,
    },
    FinalResponse,
}

impl ArtifactBinding {
    #[must_use]
    pub const fn provider_result(tenant_id: Uuid, run_id: Uuid, step_id: Uuid) -> Self {
        Self {
            tenant_id,
            run_id,
            step: ArtifactStepBinding::ExecutionStep {
                step_id,
                kind: ExecutionArtifactKind::ProviderResult,
            },
        }
    }

    #[must_use]
    pub const fn capability_result(tenant_id: Uuid, run_id: Uuid, step_id: Uuid) -> Self {
        Self {
            tenant_id,
            run_id,
            step: ArtifactStepBinding::ExecutionStep {
                step_id,
                kind: ExecutionArtifactKind::CapabilityResult,
            },
        }
    }

    /// Finalization is run-bound because the repository allocates its unique step during write.
    #[must_use]
    pub const fn final_response(tenant_id: Uuid, run_id: Uuid) -> Self {
        Self {
            tenant_id,
            run_id,
            step: ArtifactStepBinding::FinalResponse,
        }
    }

    const fn for_loaded(
        tenant_id: Uuid,
        run_id: Uuid,
        step_id: Uuid,
        kind: ExecutionArtifactKind,
    ) -> Self {
        match kind {
            ExecutionArtifactKind::ProviderResult | ExecutionArtifactKind::CapabilityResult => {
                Self {
                    tenant_id,
                    run_id,
                    step: ArtifactStepBinding::ExecutionStep { step_id, kind },
                }
            }
            ExecutionArtifactKind::FinalResponse => Self::final_response(tenant_id, run_id),
        }
    }

    fn aad(
        self,
        key_id: &str,
        key_version: i64,
        plaintext_sha256: &[u8; 32],
        plaintext_length: usize,
    ) -> Vec<u8> {
        let mut aad = Vec::with_capacity(192);
        append_field(&mut aad, AAD_DOMAIN);
        aad.push(ENVELOPE_VERSION);
        aad.extend_from_slice(self.tenant_id.as_bytes());
        aad.extend_from_slice(self.run_id.as_bytes());
        match self.step {
            ArtifactStepBinding::ExecutionStep { step_id, kind } => {
                aad.push(1);
                aad.extend_from_slice(step_id.as_bytes());
                append_field(&mut aad, kind.as_str().as_bytes());
            }
            ArtifactStepBinding::FinalResponse => {
                aad.push(2);
                append_field(
                    &mut aad,
                    ExecutionArtifactKind::FinalResponse.as_str().as_bytes(),
                );
            }
        }
        append_field(&mut aad, key_id.as_bytes());
        aad.extend_from_slice(&key_version.to_be_bytes());
        aad.extend_from_slice(&(plaintext_length as u64).to_be_bytes());
        aad.extend_from_slice(plaintext_sha256);
        aad
    }
}

fn append_field(target: &mut Vec<u8>, value: &[u8]) {
    target.extend_from_slice(&(value.len() as u64).to_be_bytes());
    target.extend_from_slice(value);
}

fn validate_key_id(key_id: &str) -> Result<(), ArtifactKeyringError> {
    if key_id.is_empty()
        || key_id.len() > MAX_KEY_ID_BYTES
        || !key_id.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        return Err(ArtifactKeyringError::InvalidKeyId);
    }
    Ok(())
}

/// Decrypted worker-only artifact bytes.
///
/// This type intentionally implements neither `Debug`, `Clone`, nor serialization.
pub struct DecryptedExecutionArtifact(Vec<u8>);

impl DecryptedExecutionArtifact {
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

/// Deployment keyring, binding, and authenticated-encryption failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArtifactKeyringError {
    #[error("Agent artifact keyring is empty")]
    Empty,
    #[error("Agent artifact key identifier is invalid")]
    InvalidKeyId,
    #[error("Agent artifact key version is invalid for {0}")]
    InvalidKeyVersion(String),
    #[error("Agent artifact key material is invalid for {0}")]
    InvalidKeyMaterial(String),
    #[error("active Agent artifact key is missing: {0}")]
    MissingActiveKey(String),
    #[error("stored Agent artifact key is unavailable: {0}")]
    UnknownStoredKey(String),
    #[error("Agent artifact plaintext length is invalid")]
    InvalidPlaintextLength,
    #[error("Agent artifact envelope is invalid")]
    InvalidArtifact,
    #[error("Agent artifact envelope version is unsupported")]
    UnsupportedEnvelope,
    #[error("Agent artifact cipher initialization failed")]
    Cipher,
    #[error("Agent artifact encryption failed")]
    Encrypt,
    #[error("Agent artifact authentication failed")]
    Decrypt,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use sha2::{Digest, Sha256};
    use uuid::Uuid;

    use super::{
        ArtifactBinding, ArtifactKeyring, ArtifactKeyringError, ExecutionArtifactKind,
        LoadedExecutionArtifact,
    };

    fn keyring(active: &str) -> ArtifactKeyring {
        ArtifactKeyring::from_base64(
            BTreeMap::from([
                ("first".to_owned(), (1, STANDARD.encode([7_u8; 32]))),
                ("second".to_owned(), (2, STANDARD.encode([9_u8; 32]))),
            ]),
            active,
        )
        .unwrap()
    }

    fn expect_error<T>(result: Result<T, ArtifactKeyringError>) -> ArtifactKeyringError {
        match result {
            Ok(_) => panic!("expected Agent artifact operation to fail"),
            Err(error) => error,
        }
    }

    #[test]
    fn step_artifacts_round_trip_with_fresh_nonces_and_rotation_history() {
        let tenant_id = Uuid::new_v4();
        let run_id = Uuid::new_v4();
        let step_id = Uuid::new_v4();
        let binding = ArtifactBinding::provider_result(tenant_id, run_id, step_id);
        let plaintext = b"bounded provider result";
        let first_keyring = keyring("first");
        let first = first_keyring.encrypt(binding, plaintext).unwrap();
        let second = first_keyring.encrypt(binding, plaintext).unwrap();
        assert_ne!(first.nonce(), second.nonce());
        assert!(first_keyring.contains_key("first", 1));
        assert!(!first_keyring.contains_key("first", 2));

        let rotated = keyring("second");
        assert_eq!(
            rotated.decrypt(binding, first).unwrap().as_bytes(),
            plaintext
        );
    }

    #[test]
    fn authenticated_binding_rejects_cross_tenant_run_step_and_kind_moves() {
        let tenant_id = Uuid::new_v4();
        let run_id = Uuid::new_v4();
        let step_id = Uuid::new_v4();
        let keyring = keyring("first");
        let encrypt = || {
            keyring
                .encrypt(
                    ArtifactBinding::provider_result(tenant_id, run_id, step_id),
                    b"provider result",
                )
                .unwrap()
        };

        for mismatched in [
            ArtifactBinding::provider_result(Uuid::new_v4(), run_id, step_id),
            ArtifactBinding::provider_result(tenant_id, Uuid::new_v4(), step_id),
            ArtifactBinding::provider_result(tenant_id, run_id, Uuid::new_v4()),
            ArtifactBinding::capability_result(tenant_id, run_id, step_id),
            ArtifactBinding::final_response(tenant_id, run_id),
        ] {
            assert_eq!(
                expect_error(keyring.decrypt(mismatched, encrypt())),
                ArtifactKeyringError::Decrypt
            );
        }
    }

    #[test]
    fn loaded_artifacts_derive_the_same_step_or_final_binding() {
        let tenant_id = Uuid::new_v4();
        let run_id = Uuid::new_v4();
        let step_id = Uuid::new_v4();
        let keyring = keyring("first");

        for (kind, binding, plaintext) in [
            (
                ExecutionArtifactKind::CapabilityResult,
                ArtifactBinding::capability_result(tenant_id, run_id, step_id),
                b"capability result".as_slice(),
            ),
            (
                ExecutionArtifactKind::FinalResponse,
                ArtifactBinding::final_response(tenant_id, run_id),
                b"final response".as_slice(),
            ),
        ] {
            let envelope = keyring.encrypt(binding, plaintext).unwrap();
            let ciphertext_sha256 = Sha256::digest(envelope.ciphertext()).into();
            let loaded = LoadedExecutionArtifact::from_stored(
                Uuid::new_v4(),
                step_id,
                kind,
                1,
                envelope.ciphertext().to_vec(),
                ciphertext_sha256,
                *envelope.plaintext_sha256(),
                envelope.nonce().to_vec(),
                envelope.encryption_key_id().to_owned(),
                envelope.encryption_key_version(),
                usize::try_from(envelope.plaintext_length()).unwrap(),
            )
            .unwrap();
            assert_eq!(
                keyring
                    .decrypt_loaded(tenant_id, run_id, loaded)
                    .unwrap()
                    .into_bytes(),
                plaintext
            );
        }
    }

    #[test]
    fn tampered_metadata_or_ciphertext_fails_authentication() {
        let tenant_id = Uuid::new_v4();
        let run_id = Uuid::new_v4();
        let binding = ArtifactBinding::final_response(tenant_id, run_id);
        let keyring = keyring("first");

        let mut ciphertext = keyring.encrypt(binding, b"final response").unwrap();
        ciphertext.ciphertext[0] ^= 1;
        assert_eq!(
            expect_error(keyring.decrypt(binding, ciphertext)),
            ArtifactKeyringError::Decrypt
        );

        let mut digest = keyring.encrypt(binding, b"final response").unwrap();
        digest.plaintext_sha256[0] ^= 1;
        assert_eq!(
            expect_error(keyring.decrypt(binding, digest)),
            ArtifactKeyringError::Decrypt
        );

        let mut length = keyring.encrypt(binding, b"final response").unwrap();
        length.plaintext_length -= 1;
        assert_eq!(
            expect_error(keyring.decrypt(binding, length)),
            ArtifactKeyringError::Decrypt
        );
    }

    #[test]
    fn configuration_and_plaintext_bounds_fail_closed() {
        assert_eq!(
            ArtifactKeyring::from_base64(BTreeMap::new(), "first").unwrap_err(),
            ArtifactKeyringError::Empty
        );
        assert_eq!(
            ArtifactKeyring::from_base64(
                BTreeMap::from([("bad key".to_owned(), (1, STANDARD.encode([7_u8; 32])))]),
                "bad key",
            )
            .unwrap_err(),
            ArtifactKeyringError::InvalidKeyId
        );
        assert_eq!(
            ArtifactKeyring::from_base64(
                BTreeMap::from([("first".to_owned(), (0, STANDARD.encode([7_u8; 32])))]),
                "first",
            )
            .unwrap_err(),
            ArtifactKeyringError::InvalidKeyVersion("first".to_owned())
        );
        assert_eq!(
            ArtifactKeyring::from_base64(
                BTreeMap::from([("first".to_owned(), (1, STANDARD.encode([7_u8; 31])))]),
                "first",
            )
            .unwrap_err(),
            ArtifactKeyringError::InvalidKeyMaterial("first".to_owned())
        );
        assert_eq!(
            ArtifactKeyring::from_base64(
                BTreeMap::from([("first".to_owned(), (1, STANDARD.encode([7_u8; 32])))]),
                "second",
            )
            .unwrap_err(),
            ArtifactKeyringError::MissingActiveKey("second".to_owned())
        );

        let keyring = keyring("first");
        let binding = ArtifactBinding::final_response(Uuid::new_v4(), Uuid::new_v4());
        assert_eq!(
            expect_error(keyring.encrypt(binding, &[])),
            ArtifactKeyringError::InvalidPlaintextLength
        );
        assert_eq!(
            expect_error(keyring.encrypt(binding, &vec![0; 65_537])),
            ArtifactKeyringError::InvalidPlaintextLength
        );
    }
}
