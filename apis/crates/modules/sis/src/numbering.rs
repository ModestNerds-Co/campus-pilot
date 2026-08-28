//! Owns tenant-scoped learner number policy and transactional allocation.
//!
//! Ordinary creates consume the configured prefix, padding, and sequence.
//! Imports retain legacy identities and only align values rendered by the
//! current managed policy. Policy changes are optimistic and forward-only.

use anyhow::{Context, Result, bail};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use serde::Serialize;
use serde_json::json;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::dtos::UpdateLearnerNumberingPolicyRequest;

const DEFAULT_PREFIX: &str = "LRN-";
const DEFAULT_PADDING: usize = 6;
const MAX_ISSUABLE_SEQUENCE: i64 = 99_999_999;
const EXHAUSTED_NEXT_SEQUENCE: i64 = MAX_ISSUABLE_SEQUENCE + 1;

#[derive(Debug, Clone, FromRow)]
struct LearnerNumberSequenceRow {
    id: Uuid,
    number_prefix: String,
    number_padding: i16,
    last_number: i64,
    version: i32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LearnerNumberingPolicy {
    pub number_prefix: String,
    pub number_padding: i16,
    pub next_sequence: i64,
    pub next_number_preview: Option<String>,
    pub exhausted: bool,
    pub version: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LearnerNumberingUpdateOutcome {
    Updated(LearnerNumberingPolicy),
    VersionConflict,
    NextSequenceBehind { minimum: i64 },
}

#[derive(Debug, Clone)]
struct ValidatedPolicy {
    prefix: String,
    padding: usize,
}

impl TryFrom<&LearnerNumberSequenceRow> for ValidatedPolicy {
    type Error = anyhow::Error;

    fn try_from(row: &LearnerNumberSequenceRow) -> Result<Self> {
        Self::new(&row.number_prefix, row.number_padding)
    }
}

impl ValidatedPolicy {
    fn new(prefix: &str, padding: i16) -> Result<Self> {
        let prefix = prefix.trim();
        let padding = usize::try_from(padding)
            .context("Learner number padding is outside the supported range")?;
        if prefix.is_empty() || prefix.chars().count() > 32 || prefix.chars().any(char::is_control)
        {
            bail!("Prefix must contain 1 to 32 printable characters.");
        }
        if !(1..=8).contains(&padding) {
            bail!("Padding must be between 1 and 8 digits.");
        }
        Ok(Self {
            prefix: prefix.to_string(),
            padding,
        })
    }

    fn render(&self, sequence: i64) -> Result<String> {
        if !(1..=MAX_ISSUABLE_SEQUENCE).contains(&sequence) {
            bail!("The learner number sequence is outside the issuable range.");
        }
        let learner_number = format!("{}{sequence:0width$}", self.prefix, width = self.padding);
        if learner_number.chars().count() > 80 {
            bail!("The learner number policy renders an invalid value.");
        }
        Ok(learner_number)
    }

    fn managed_sequence(&self, learner_number: &str) -> Option<i64> {
        let prefix = learner_number.get(..self.prefix.len())?;
        let digits = learner_number.get(self.prefix.len()..)?;
        if !prefix.eq_ignore_ascii_case(&self.prefix)
            || !(self.padding..=8).contains(&digits.len())
            || !digits.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        let sequence = digits.parse::<i64>().ok()?;
        (1..=MAX_ISSUABLE_SEQUENCE)
            .contains(&sequence)
            .then_some(sequence)
    }

    fn projection(&self, next_sequence: i64, version: i32) -> LearnerNumberingPolicy {
        LearnerNumberingPolicy {
            number_prefix: self.prefix.clone(),
            number_padding: i16::try_from(self.padding)
                .expect("validated learner number padding fits i16"),
            next_sequence,
            next_number_preview: self.render(next_sequence).ok(),
            exhausted: next_sequence > MAX_ISSUABLE_SEQUENCE,
            version,
        }
    }
}

pub struct LearnerNumberingPolicyOps;

impl LearnerNumberingPolicyOps {
    /// Reads the effective policy without creating a database row.
    pub async fn get(pool: &PgPool, tenant_id: Uuid) -> Result<LearnerNumberingPolicy> {
        let row = sqlx::query_as::<_, LearnerNumberSequenceRow>(
            r#"
            SELECT id, number_prefix, number_padding, last_number, version
            FROM sis_learner_number_sequences
            WHERE tenant_id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .fetch_optional(pool)
        .await
        .context("Failed to load learner number policy")?;
        match row {
            Some(row) => Ok(ValidatedPolicy::try_from(&row)?
                .projection(next_sequence(row.last_number), row.version)),
            None => Ok(default_policy()),
        }
    }

    /// Updates policy configuration while preserving the global sequence floor.
    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: UpdateLearnerNumberingPolicyRequest,
    ) -> Result<LearnerNumberingUpdateOutcome> {
        let policy = ValidatedPolicy::new(&request.number_prefix, request.number_padding)?;
        let reason = request.reason.trim();
        if reason.is_empty() || reason.chars().count() > 1_000 {
            bail!("Reason must contain 1 to 1000 characters.");
        }
        if !(1..=EXHAUSTED_NEXT_SEQUENCE).contains(&request.next_sequence) {
            bail!("Next sequence must be between 1 and {EXHAUSTED_NEXT_SEQUENCE}.");
        }
        if request.expected_version < 0 {
            bail!("Expected version cannot be negative.");
        }

        let mut transaction = pool
            .begin()
            .await
            .context("Failed to begin learner number policy update")?;
        let inserted = sqlx::query_as::<_, LearnerNumberSequenceRow>(
            r#"
            INSERT INTO sis_learner_number_sequences (tenant_id)
            VALUES ($1)
            ON CONFLICT (tenant_id) DO NOTHING
            RETURNING id, number_prefix, number_padding, last_number, version
            "#,
        )
        .bind(tenant_id)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to initialize learner number policy")?;
        let (current, logical_version) = match inserted {
            Some(row) => (row, 0),
            None => {
                let row = lock_policy(&mut transaction, tenant_id).await?;
                let version = row.version;
                (row, version)
            }
        };
        if logical_version != request.expected_version {
            return Ok(LearnerNumberingUpdateOutcome::VersionConflict);
        }

        let current_next = next_sequence(current.last_number);
        let namespace_last = highest_managed_sequence(&mut transaction, tenant_id, &policy).await?;
        let minimum_next = current_next.max(next_sequence(namespace_last));
        if request.next_sequence < minimum_next {
            return Ok(LearnerNumberingUpdateOutcome::NextSequenceBehind {
                minimum: minimum_next,
            });
        }

        let next_version = current.version + i32::from(logical_version > 0);
        let updated = sqlx::query_as::<_, LearnerNumberSequenceRow>(
            r#"
            UPDATE sis_learner_number_sequences
            SET number_prefix = $1,
                number_padding = $2,
                last_number = $3,
                version = $4,
                deleted_at = NULL
            WHERE id = $5 AND tenant_id = $6
            RETURNING id, number_prefix, number_padding, last_number, version
            "#,
        )
        .bind(&policy.prefix)
        .bind(request.number_padding)
        .bind(request.next_sequence - 1)
        .bind(next_version)
        .bind(current.id)
        .bind(tenant_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to update learner number policy")?;
        let result = policy.projection(request.next_sequence, updated.version);

        append_audit(
            &mut *transaction,
            &NewAuditEvent::new(
                tenant_id,
                actor,
                "sis.learner_numbering.update",
                AuditOutcome::Succeeded,
                request_context,
            )
            .with_target(AuditTarget::new(
                "sis_learner_number_policy",
                current.id.to_string(),
            ))
            .with_reason(reason)
            .with_redacted_metadata(
                json!({
                    "before": {
                        "number_prefix": current.number_prefix,
                        "number_padding": current.number_padding,
                        "next_sequence": current_next,
                        "version": logical_version,
                    },
                    "after": {
                        "number_prefix": result.number_prefix,
                        "number_padding": result.number_padding,
                        "next_sequence": result.next_sequence,
                        "version": result.version,
                    }
                })
                .as_object()
                .cloned()
                .unwrap_or_default(),
            ),
        )
        .await
        .context("Failed to audit learner number policy update")?;
        transaction
            .commit()
            .await
            .context("Failed to commit learner number policy update")?;
        Ok(LearnerNumberingUpdateOutcome::Updated(result))
    }
}

/// Allocates the next learner number while holding the tenant policy row.
///
/// The caller must create the learner in the same transaction. Rolling the
/// transaction back also returns the uncommitted allocation.
pub(crate) async fn allocate_learner_number(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
) -> Result<String> {
    let sequence = sqlx::query_as::<_, LearnerNumberSequenceRow>(
        r#"
        INSERT INTO sis_learner_number_sequences (tenant_id, last_number)
        VALUES ($1, 1)
        ON CONFLICT (tenant_id) DO UPDATE
           SET last_number = sis_learner_number_sequences.last_number + 1
         WHERE sis_learner_number_sequences.last_number < $2
        RETURNING id, number_prefix, number_padding, last_number, version
        "#,
    )
    .bind(tenant_id)
    .bind(MAX_ISSUABLE_SEQUENCE)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to allocate learner number")?
    .ok_or_else(|| anyhow::anyhow!("The learner number sequence is exhausted."))?;
    ValidatedPolicy::try_from(&sequence)?.render(sequence.last_number)
}

/// Advances the managed sequence for an explicitly imported learner number.
///
/// Arbitrary legacy formats are deliberately ignored and remain unchanged.
pub(crate) async fn align_imported_learner_number(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    learner_number: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO sis_learner_number_sequences (tenant_id, last_number)
        VALUES ($1, 0)
        ON CONFLICT (tenant_id) DO NOTHING
        "#,
    )
    .bind(tenant_id)
    .execute(&mut **transaction)
    .await
    .context("Failed to initialize learner number sequence")?;
    let current = lock_policy(transaction, tenant_id).await?;
    let policy = ValidatedPolicy::try_from(&current)?;
    let Some(sequence) = policy.managed_sequence(learner_number) else {
        return Ok(());
    };
    if sequence > current.last_number {
        sqlx::query(
            r#"
            UPDATE sis_learner_number_sequences
            SET last_number = $1
            WHERE tenant_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(sequence)
        .bind(tenant_id)
        .execute(&mut **transaction)
        .await
        .context("Failed to align imported learner number sequence")?;
    }
    Ok(())
}

async fn lock_policy(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
) -> Result<LearnerNumberSequenceRow> {
    sqlx::query_as::<_, LearnerNumberSequenceRow>(
        r#"
        SELECT id, number_prefix, number_padding, last_number, version
        FROM sis_learner_number_sequences
        WHERE tenant_id = $1 AND deleted_at IS NULL
        FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to lock learner number policy")
}

async fn highest_managed_sequence(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    policy: &ValidatedPolicy,
) -> Result<i64> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COALESCE(MAX(SUBSTRING(learner_number FROM CHAR_LENGTH($2) + 1)::BIGINT), 0)
        FROM learners
        WHERE tenant_id = $1
          AND LOWER(LEFT(learner_number, CHAR_LENGTH($2))) = LOWER($2)
          AND CHAR_LENGTH(SUBSTRING(learner_number FROM CHAR_LENGTH($2) + 1))
                BETWEEN $3 AND 8
          AND SUBSTRING(learner_number FROM CHAR_LENGTH($2) + 1) ~ '^[0-9]+$'
        "#,
    )
    .bind(tenant_id)
    .bind(&policy.prefix)
    .bind(i32::try_from(policy.padding).expect("validated padding fits i32"))
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to determine the learner number boundary")
}

const fn next_sequence(last_number: i64) -> i64 {
    if last_number >= MAX_ISSUABLE_SEQUENCE {
        EXHAUSTED_NEXT_SEQUENCE
    } else {
        last_number + 1
    }
}

fn default_policy() -> LearnerNumberingPolicy {
    ValidatedPolicy {
        prefix: DEFAULT_PREFIX.to_string(),
        padding: DEFAULT_PADDING,
    }
    .projection(1, 0)
}

#[cfg(test)]
mod tests {
    use super::{
        EXHAUSTED_NEXT_SEQUENCE, LearnerNumberingPolicyOps, ValidatedPolicy, default_policy,
        next_sequence,
    };

    fn managed_policy() -> ValidatedPolicy {
        ValidatedPolicy {
            prefix: "LRN-".to_string(),
            padding: 6,
        }
    }

    #[test]
    fn absent_policy_has_a_stable_preview_and_optimistic_version() {
        let policy = default_policy();
        assert_eq!(policy.next_number_preview.as_deref(), Some("LRN-000001"));
        assert_eq!(policy.next_sequence, 1);
        assert_eq!(policy.version, 0);
        assert!(!policy.exhausted);
    }

    #[test]
    fn generated_numbers_are_stable_and_zero_padded() {
        assert_eq!(managed_policy().render(1).unwrap(), "LRN-000001");
        assert_eq!(managed_policy().render(999_999).unwrap(), "LRN-999999");
        assert_eq!(managed_policy().render(1_000_000).unwrap(), "LRN-1000000");
    }

    #[test]
    fn generated_number_bounds_and_policy_fields_are_enforced() {
        assert!(managed_policy().render(0).is_err());
        assert!(managed_policy().render(100_000_000).is_err());
        assert!(ValidatedPolicy::new(" ", 6).is_err());
        assert!(ValidatedPolicy::new("LRN-", 0).is_err());
        assert!(ValidatedPolicy::new("LRN-", 9).is_err());
        assert!(ValidatedPolicy::new("LRN-\n", 6).is_ok());
        assert!(ValidatedPolicy::new(" LRN- ", 6).is_ok());
    }

    #[test]
    fn only_managed_import_numbers_advance_the_sequence() {
        assert_eq!(managed_policy().managed_sequence("LRN-000125"), Some(125));
        assert_eq!(
            managed_policy().managed_sequence("LRN-99999999"),
            Some(99_999_999)
        );
        assert_eq!(managed_policy().managed_sequence("STU-000125"), None);
        assert_eq!(managed_policy().managed_sequence("lrn-000125"), Some(125));
        assert_eq!(managed_policy().managed_sequence("LRN-00125"), None);
        assert_eq!(managed_policy().managed_sequence("LRN-100000000"), None);
    }

    #[test]
    fn tenant_policy_controls_prefix_and_padding() {
        let policy = ValidatedPolicy {
            prefix: "STUDENT/".to_string(),
            padding: 4,
        };
        assert_eq!(policy.render(17).unwrap(), "STUDENT/0017");
        assert_eq!(policy.managed_sequence("student/0017"), Some(17));
        assert_eq!(policy.managed_sequence("LRN-000017"), None);
    }

    #[test]
    fn exhausted_sequence_is_explicit_and_has_no_preview() {
        assert_eq!(next_sequence(99_999_999), EXHAUSTED_NEXT_SEQUENCE);
        let policy = managed_policy().projection(EXHAUSTED_NEXT_SEQUENCE, 4);
        assert!(policy.exhausted);
        assert_eq!(policy.next_number_preview, None);
    }

    #[test]
    fn policy_ops_is_a_public_typed_boundary() {
        let _ = LearnerNumberingPolicyOps;
    }
}
