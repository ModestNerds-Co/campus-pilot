//! Proves that deployment keys can authenticate every durable Agent artifact generation.
//!
//! Startup validation loads one encrypted, execution-bound sample for each stored key identity.
//! It never exposes key identifiers, ciphertext, or decrypted continuation data through Debug,
//! serialization, or error messages.

use std::str::FromStr;

use sqlx::FromRow;
use thiserror::Error;
use uuid::Uuid;

use super::{
    artifacts::{ArtifactKeyring, ArtifactKeyringError},
    ops::AgentSessionOps,
    types::{ExecutionArtifactKind, LoadedExecutionArtifact},
};

const MAX_STORED_ARTIFACT_KEY_IDENTITIES: usize = 256;

/// Proof that a configured keyring covers and authenticates current durable artifact history.
///
/// The strict API requires a parsed keyring even when no artifacts exist. A dedicated worker can
/// therefore make key configuration an unconditional startup dependency without exposing stored
/// key identities to its process wiring.
pub struct ValidatedArtifactKeyringCoverage {
    distinct_key_identity_count: usize,
}

impl ValidatedArtifactKeyringCoverage {
    /// Number of distinct stored key ID and version pairs that were authenticated.
    #[must_use]
    pub const fn distinct_key_identity_count(&self) -> usize {
        self.distinct_key_identity_count
    }
}

/// Safe startup failures for strict Agent artifact key coverage validation.
#[derive(Debug, Error)]
pub enum ArtifactKeyringCoverageError {
    #[error("Agent artifact keyring is missing a stored key identity")]
    MissingStoredKey,
    #[error("Agent artifact keyring cannot authenticate stored artifact history")]
    StoredArtifactAuthentication,
    #[error("stored Agent artifact history uses an unsupported envelope")]
    UnsupportedStoredArtifact,
    #[error("Agent artifact key coverage inspection failed")]
    Storage(#[source] sqlx::Error),
}

impl AgentSessionOps {
    /// Strictly validates configured coverage for all stored artifact key generations.
    ///
    /// One oldest bound sample is selected for each distinct `(key_id, key_version)` pair. Exact
    /// identity coverage is checked before AES-GCM authentication under the stored tenant, run,
    /// step, and artifact-kind binding. Replacing material without changing its identity therefore
    /// fails startup instead of surfacing later during crash recovery.
    pub async fn validate_artifact_keyring_coverage(
        &self,
        keyring: &ArtifactKeyring,
    ) -> Result<ValidatedArtifactKeyringCoverage, ArtifactKeyringCoverageError> {
        let rows = sqlx::query_as::<_, StoredArtifactKeySampleRow>(
            r#"
            SELECT DISTINCT ON (a.encryption_key_id, a.encryption_key_version)
                a.tenant_id,
                a.run_id,
                a.id AS artifact_id,
                a.step_id,
                a.artifact_sequence,
                a.artifact_kind,
                a.ciphertext,
                a.ciphertext_sha256,
                a.plaintext_sha256,
                a.nonce,
                a.encryption_key_id,
                a.encryption_key_version,
                a.plaintext_length
            FROM agent_execution_artifacts AS a
            WHERE a.deleted_at IS NULL
            ORDER BY
                a.encryption_key_id,
                a.encryption_key_version,
                a.created_at,
                a.id
            LIMIT $1
            "#,
        )
        .bind(
            i64::try_from(MAX_STORED_ARTIFACT_KEY_IDENTITIES + 1)
                .map_err(|_| ArtifactKeyringCoverageError::UnsupportedStoredArtifact)?,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(ArtifactKeyringCoverageError::Storage)?;

        if rows.len() > MAX_STORED_ARTIFACT_KEY_IDENTITIES {
            return Err(ArtifactKeyringCoverageError::UnsupportedStoredArtifact);
        }

        let samples = rows
            .into_iter()
            .map(StoredArtifactKeySample::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        validate_artifact_key_samples(keyring, samples)
    }
}

#[derive(FromRow)]
struct StoredArtifactKeySampleRow {
    tenant_id: Uuid,
    run_id: Uuid,
    artifact_id: Uuid,
    step_id: Uuid,
    artifact_sequence: i16,
    artifact_kind: String,
    ciphertext: Vec<u8>,
    ciphertext_sha256: Vec<u8>,
    plaintext_sha256: Vec<u8>,
    nonce: Vec<u8>,
    encryption_key_id: String,
    encryption_key_version: i64,
    plaintext_length: i32,
}

struct StoredArtifactKeySample {
    tenant_id: Uuid,
    run_id: Uuid,
    artifact: LoadedExecutionArtifact,
}

impl TryFrom<StoredArtifactKeySampleRow> for StoredArtifactKeySample {
    type Error = ArtifactKeyringCoverageError;

    fn try_from(row: StoredArtifactKeySampleRow) -> Result<Self, Self::Error> {
        let ciphertext_sha256 = row
            .ciphertext_sha256
            .try_into()
            .map_err(|_| ArtifactKeyringCoverageError::UnsupportedStoredArtifact)?;
        let plaintext_sha256 = row
            .plaintext_sha256
            .try_into()
            .map_err(|_| ArtifactKeyringCoverageError::UnsupportedStoredArtifact)?;
        let kind = ExecutionArtifactKind::from_str(&row.artifact_kind)
            .map_err(|_| ArtifactKeyringCoverageError::UnsupportedStoredArtifact)?;
        let plaintext_length = usize::try_from(row.plaintext_length)
            .map_err(|_| ArtifactKeyringCoverageError::UnsupportedStoredArtifact)?;
        let artifact = LoadedExecutionArtifact::from_stored(
            row.artifact_id,
            row.step_id,
            kind,
            row.artifact_sequence,
            row.ciphertext,
            ciphertext_sha256,
            plaintext_sha256,
            row.nonce,
            row.encryption_key_id,
            row.encryption_key_version,
            plaintext_length,
        )
        .map_err(|_| ArtifactKeyringCoverageError::UnsupportedStoredArtifact)?;
        Ok(Self {
            tenant_id: row.tenant_id,
            run_id: row.run_id,
            artifact,
        })
    }
}

fn validate_artifact_key_samples(
    keyring: &ArtifactKeyring,
    samples: impl IntoIterator<Item = StoredArtifactKeySample>,
) -> Result<ValidatedArtifactKeyringCoverage, ArtifactKeyringCoverageError> {
    let mut distinct_key_identity_count = 0_usize;
    for sample in samples {
        let envelope = sample.artifact.envelope();
        if !keyring.contains_key(
            envelope.encryption_key_id(),
            envelope.encryption_key_version(),
        ) {
            return Err(ArtifactKeyringCoverageError::MissingStoredKey);
        }
        keyring
            .decrypt_loaded(sample.tenant_id, sample.run_id, sample.artifact)
            .map_err(map_keyring_error)?;
        distinct_key_identity_count = distinct_key_identity_count
            .checked_add(1)
            .ok_or(ArtifactKeyringCoverageError::UnsupportedStoredArtifact)?;
    }
    Ok(ValidatedArtifactKeyringCoverage {
        distinct_key_identity_count,
    })
}

fn map_keyring_error(error: ArtifactKeyringError) -> ArtifactKeyringCoverageError {
    match error {
        ArtifactKeyringError::UnknownStoredKey(_) => ArtifactKeyringCoverageError::MissingStoredKey,
        ArtifactKeyringError::Decrypt => ArtifactKeyringCoverageError::StoredArtifactAuthentication,
        ArtifactKeyringError::Empty
        | ArtifactKeyringError::InvalidKeyId
        | ArtifactKeyringError::InvalidKeyVersion(_)
        | ArtifactKeyringError::InvalidKeyMaterial(_)
        | ArtifactKeyringError::MissingActiveKey(_)
        | ArtifactKeyringError::InvalidPlaintextLength
        | ArtifactKeyringError::InvalidArtifact
        | ArtifactKeyringError::UnsupportedEnvelope
        | ArtifactKeyringError::Cipher
        | ArtifactKeyringError::Encrypt => ArtifactKeyringCoverageError::UnsupportedStoredArtifact,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use sha2::{Digest, Sha256};
    use sqlx::{PgPool, postgres::PgPoolOptions};
    use uuid::Uuid;

    use super::{
        AgentSessionOps, ArtifactKeyring, ArtifactKeyringCoverageError, StoredArtifactKeySample,
        ValidatedArtifactKeyringCoverage, validate_artifact_key_samples,
    };
    use crate::{
        ArtifactBinding, EncryptedExecutionArtifact, ExecutionArtifactKind, LoadedExecutionArtifact,
    };

    macro_rules! assert_not_impl {
        ($type:ty: $trait:path) => {
            const _: fn() = || {
                struct Invalid;
                trait AmbiguousIfImpl<A> {
                    fn marker() {}
                }
                impl<T: ?Sized> AmbiguousIfImpl<()> for T {}
                impl<T: ?Sized + $trait> AmbiguousIfImpl<Invalid> for T {}
                let _ = <$type as AmbiguousIfImpl<_>>::marker;
            };
        };
    }

    assert_not_impl!(ValidatedArtifactKeyringCoverage: std::fmt::Debug);
    assert_not_impl!(ValidatedArtifactKeyringCoverage: serde::Serialize);
    assert_not_impl!(StoredArtifactKeySample: std::fmt::Debug);
    assert_not_impl!(StoredArtifactKeySample: serde::Serialize);

    fn keyring(active_key_id: &str, first_material: u8, include_second: bool) -> ArtifactKeyring {
        let mut keys = BTreeMap::from([(
            "first".to_owned(),
            (1, STANDARD.encode([first_material; 32])),
        )]);
        if include_second {
            keys.insert("second".to_owned(), (2, STANDARD.encode([9_u8; 32])));
        }
        ArtifactKeyring::from_base64(keys, active_key_id).unwrap()
    }

    fn sample(
        keyring: &ArtifactKeyring,
        tenant_id: Uuid,
        run_id: Uuid,
        step_id: Uuid,
        kind: ExecutionArtifactKind,
        plaintext: &[u8],
    ) -> StoredArtifactKeySample {
        let binding = match kind {
            ExecutionArtifactKind::ProviderResult => {
                ArtifactBinding::provider_result(tenant_id, run_id, step_id)
            }
            ExecutionArtifactKind::CapabilityResult => {
                ArtifactBinding::capability_result(tenant_id, run_id, step_id)
            }
            ExecutionArtifactKind::FinalResponse => {
                ArtifactBinding::final_response(tenant_id, run_id)
            }
        };
        let encrypted = keyring.encrypt(binding, plaintext).unwrap();
        loaded_sample(tenant_id, run_id, step_id, kind, encrypted)
    }

    fn loaded_sample(
        tenant_id: Uuid,
        run_id: Uuid,
        step_id: Uuid,
        kind: ExecutionArtifactKind,
        encrypted: EncryptedExecutionArtifact,
    ) -> StoredArtifactKeySample {
        let ciphertext_sha256 = Sha256::digest(encrypted.ciphertext()).into();
        let loaded = LoadedExecutionArtifact::from_stored(
            Uuid::new_v4(),
            step_id,
            kind,
            1,
            encrypted.ciphertext().to_vec(),
            ciphertext_sha256,
            *encrypted.plaintext_sha256(),
            encrypted.nonce().to_vec(),
            encrypted.encryption_key_id().to_owned(),
            encrypted.encryption_key_version(),
            usize::try_from(encrypted.plaintext_length()).unwrap(),
        )
        .unwrap();
        StoredArtifactKeySample {
            tenant_id,
            run_id,
            artifact: loaded,
        }
    }

    async fn seed_final_artifact(pool: &PgPool, keyring: &ArtifactKeyring, label: &str) {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let thread_id = Uuid::new_v4();
        let message_id = Uuid::new_v4();
        let run_id = Uuid::new_v4();
        let step_id = Uuid::new_v4();
        sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, $2, $3)")
            .bind(tenant_id)
            .bind(format!("artifact-coverage-{}", &tenant_id.to_string()[..8]))
            .bind(format!("Artifact coverage {label}"))
            .execute(pool)
            .await
            .expect("coverage fixture tenant must insert");
        sqlx::query(
            r#"
            INSERT INTO users (id, tenant_id, email, password_hash, full_name)
            VALUES ($1, $2, $3, 'test-only', 'Artifact Coverage Owner')
            "#,
        )
        .bind(user_id)
        .bind(tenant_id)
        .bind(format!("artifact-{user_id}@coverage.test"))
        .execute(pool)
        .await
        .expect("coverage fixture owner must insert");
        sqlx::query("INSERT INTO agent_threads (id, tenant_id, owner_user_id) VALUES ($1, $2, $3)")
            .bind(thread_id)
            .bind(tenant_id)
            .bind(user_id)
            .execute(pool)
            .await
            .expect("coverage fixture Session must insert");
        sqlx::query(
            r#"
            INSERT INTO agent_thread_members (
                tenant_id, thread_id, user_id, membership_role, added_by
            ) VALUES ($1, $2, $3, 'owner', $3)
            "#,
        )
        .bind(tenant_id)
        .bind(thread_id)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("coverage fixture owner membership must insert");
        sqlx::query(
            r#"
            UPDATE agent_threads
            SET next_message_sequence = 2,
                version = 2,
                last_activity_at = last_activity_at + INTERVAL '1 second',
                updated_at = updated_at + INTERVAL '1 second'
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(thread_id)
        .bind(tenant_id)
        .execute(pool)
        .await
        .expect("coverage fixture message sequence must allocate");
        sqlx::query(
            r#"
            INSERT INTO agent_messages (
                id, tenant_id, thread_id, sequence, role, user_id, content
            ) VALUES ($1, $2, $3, 1, 'user', $4, 'Validate artifact key coverage')
            "#,
        )
        .bind(message_id)
        .bind(tenant_id)
        .bind(thread_id)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("coverage fixture message must insert");
        sqlx::query(
            r#"
            INSERT INTO agent_runs (
                id, tenant_id, thread_id, request_message_id, requested_by, task_class,
                origin_module_key, origin_route, request_id, correlation_id
            ) VALUES (
                $1, $2, $3, $4, $5, 'module_read_reporting',
                'agent', '/modules/agent', $6, $7
            )
            "#,
        )
        .bind(run_id)
        .bind(tenant_id)
        .bind(thread_id)
        .bind(message_id)
        .bind(user_id)
        .bind(Uuid::new_v4())
        .bind(Uuid::new_v4())
        .execute(pool)
        .await
        .expect("coverage fixture run must insert");
        sqlx::query(
            r#"
            UPDATE agent_runs
            SET status = 'running',
                started_at = NOW(),
                version = 2,
                updated_at = updated_at + INTERVAL '1 second'
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(run_id)
        .bind(tenant_id)
        .execute(pool)
        .await
        .expect("coverage fixture run must start");
        sqlx::query(
            r#"
            INSERT INTO agent_execution_steps (
                id, tenant_id, run_id, step_index, turn_index, step_kind, input_fingerprint
            ) VALUES ($1, $2, $3, 1, 1, 'finalize', $4)
            "#,
        )
        .bind(step_id)
        .bind(tenant_id)
        .bind(run_id)
        .bind(vec![5_u8; 32])
        .execute(pool)
        .await
        .expect("coverage fixture finalization step must insert");

        let encrypted = keyring
            .encrypt(
                ArtifactBinding::final_response(tenant_id, run_id),
                label.as_bytes(),
            )
            .expect("coverage fixture artifact must encrypt");
        let ciphertext_sha256: [u8; 32] = Sha256::digest(encrypted.ciphertext()).into();
        sqlx::query(
            r#"
            INSERT INTO agent_execution_artifacts (
                tenant_id, run_id, step_id, artifact_sequence, artifact_kind,
                ciphertext, ciphertext_sha256, plaintext_sha256, nonce,
                encryption_key_id, encryption_key_version, plaintext_length
            ) VALUES ($1, $2, $3, 1, 'final_response', $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(tenant_id)
        .bind(run_id)
        .bind(step_id)
        .bind(encrypted.ciphertext())
        .bind(ciphertext_sha256.as_slice())
        .bind(encrypted.plaintext_sha256().as_slice())
        .bind(encrypted.nonce())
        .bind(encrypted.encryption_key_id())
        .bind(encrypted.encryption_key_version())
        .bind(encrypted.plaintext_length())
        .execute(pool)
        .await
        .expect("coverage fixture encrypted artifact must insert");
    }

    #[test]
    fn strict_coverage_authenticates_one_bound_sample_per_key_identity() {
        let tenant_id = Uuid::new_v4();
        let run_id = Uuid::new_v4();
        let first = keyring("first", 7, true);
        let second = keyring("second", 7, true);
        let samples = vec![
            sample(
                &first,
                tenant_id,
                run_id,
                Uuid::new_v4(),
                ExecutionArtifactKind::ProviderResult,
                b"provider result",
            ),
            sample(
                &second,
                tenant_id,
                run_id,
                Uuid::new_v4(),
                ExecutionArtifactKind::CapabilityResult,
                b"capability result",
            ),
        ];

        let coverage = validate_artifact_key_samples(&first, samples).unwrap();
        assert_eq!(coverage.distinct_key_identity_count(), 2);
        assert_eq!(
            validate_artifact_key_samples(&first, Vec::new())
                .unwrap()
                .distinct_key_identity_count(),
            0
        );
    }

    #[test]
    fn missing_version_and_replaced_material_fail_without_exposing_identity() {
        let tenant_id = Uuid::new_v4();
        let run_id = Uuid::new_v4();
        let step_id = Uuid::new_v4();
        let original = keyring("first", 7, true);
        let encrypted_with_second = sample(
            &keyring("second", 7, true),
            tenant_id,
            run_id,
            step_id,
            ExecutionArtifactKind::ProviderResult,
            b"second-generation result",
        );
        let missing_second = keyring("first", 7, false);
        let error = validate_artifact_key_samples(&missing_second, [encrypted_with_second])
            .err()
            .unwrap();
        assert!(matches!(
            error,
            ArtifactKeyringCoverageError::MissingStoredKey
        ));
        assert!(!error.to_string().contains("second"));

        let original_sample = sample(
            &original,
            tenant_id,
            run_id,
            step_id,
            ExecutionArtifactKind::ProviderResult,
            b"first-generation result",
        );
        let replaced_material = keyring("first", 8, false);
        let error = validate_artifact_key_samples(&replaced_material, [original_sample])
            .err()
            .unwrap();
        assert!(matches!(
            error,
            ArtifactKeyringCoverageError::StoredArtifactAuthentication
        ));
        assert!(!error.to_string().contains("first"));
    }

    #[test]
    fn changed_binding_and_unsupported_envelope_fail_closed() {
        let tenant_id = Uuid::new_v4();
        let run_id = Uuid::new_v4();
        let step_id = Uuid::new_v4();
        let keyring = keyring("first", 7, false);
        let mut mismatched = sample(
            &keyring,
            tenant_id,
            run_id,
            step_id,
            ExecutionArtifactKind::FinalResponse,
            b"final response",
        );
        mismatched.tenant_id = Uuid::new_v4();
        assert!(matches!(
            validate_artifact_key_samples(&keyring, [mismatched]),
            Err(ArtifactKeyringCoverageError::StoredArtifactAuthentication)
        ));

        let binding = ArtifactBinding::provider_result(tenant_id, run_id, step_id);
        let valid = keyring.encrypt(binding, b"provider result").unwrap();
        let unsupported = EncryptedExecutionArtifact::parse(
            valid.ciphertext().to_vec(),
            *valid.plaintext_sha256(),
            vec![3; 13],
            valid.encryption_key_id(),
            valid.encryption_key_version(),
            usize::try_from(valid.plaintext_length()).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            validate_artifact_key_samples(
                &keyring,
                [loaded_sample(
                    tenant_id,
                    run_id,
                    step_id,
                    ExecutionArtifactKind::ProviderResult,
                    unsupported,
                )],
            ),
            Err(ArtifactKeyringCoverageError::UnsupportedStoredArtifact)
        ));
    }

    #[tokio::test]
    #[ignore = "requires a fresh, disposable, migrated AGENT_ARTIFACT_COVERAGE_TEST_DATABASE_URL"]
    async fn postgres_contract_enumerates_distinct_identities_and_authenticates_samples() {
        let database_url = std::env::var("AGENT_ARTIFACT_COVERAGE_TEST_DATABASE_URL").expect(
            "AGENT_ARTIFACT_COVERAGE_TEST_DATABASE_URL must target a disposable migrated database",
        );
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(&database_url)
            .await
            .expect("Agent artifact coverage database must connect");
        let existing_artifacts =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM agent_execution_artifacts")
                .fetch_one(&pool)
                .await
                .expect("coverage contract must inspect its fresh database");
        assert_eq!(
            existing_artifacts, 0,
            "artifact coverage contract requires a fresh disposable database"
        );

        let first = keyring("first", 7, true);
        let second = keyring("second", 7, true);
        seed_final_artifact(&pool, &first, "first sample").await;
        seed_final_artifact(&pool, &first, "duplicate first identity").await;
        seed_final_artifact(&pool, &second, "second sample").await;

        let ops = AgentSessionOps::new(pool);
        let coverage = ops
            .validate_artifact_keyring_coverage(&first)
            .await
            .expect("the complete historical keyring must authenticate both identities");
        assert_eq!(coverage.distinct_key_identity_count(), 2);
        assert!(matches!(
            ops.validate_artifact_keyring_coverage(&keyring("first", 7, false))
                .await,
            Err(ArtifactKeyringCoverageError::MissingStoredKey)
        ));
        assert!(matches!(
            ops.validate_artifact_keyring_coverage(&keyring("first", 8, true))
                .await,
            Err(ArtifactKeyringCoverageError::StoredArtifactAuthentication)
        ));
    }
}
