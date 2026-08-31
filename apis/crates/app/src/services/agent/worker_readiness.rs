//! Evaluates tenant-specific Agent worker readiness without inventing pricing.
//!
//! The first execution release can enforce run, attempt, capability-call, and
//! token limits. Versioned model pricing does not exist yet, so an active hard
//! `agent.estimated_cost` rule must stop provider dispatch for that campus.

use cp_agent_runtime::{
    AgentSessionOps, ArtifactKeyring, ArtifactKeyringCoverageError,
    ValidatedArtifactKeyringCoverage,
};
use cp_ai_providers::CredentialKeyring;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentWorkerReadinessReason {
    Ready,
    EstimatedCostHardLimitRequiresPricing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AgentWorkerReadiness {
    pub ready: bool,
    pub reason: AgentWorkerReadinessReason,
}

#[derive(Debug, Error)]
pub enum AgentWorkerReadinessError {
    #[error("Agent artifact keyring does not cover durable history")]
    ArtifactCoverage(#[source] ArtifactKeyringCoverageError),
    #[error("AI provider credential keyring does not cover active credentials")]
    ProviderCredentialCoverage,
    #[error("Agent worker readiness could not be loaded")]
    Storage(#[source] sqlx::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentWorkerCoverageProof {
    pub artifact_keys: [u8; 32],
    pub provider_keys: [u8; 32],
    pub provider_routes: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentWorkerInstance {
    pub id: Uuid,
    pub worker_key: String,
    pub version: i64,
    pub coverage: Option<AgentWorkerCoverageProof>,
}

#[derive(Debug, Clone)]
pub struct AgentWorkerReadinessOps {
    pool: PgPool,
    sessions: AgentSessionOps,
}

impl AgentWorkerReadinessOps {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self {
            sessions: AgentSessionOps::new(pool.clone()),
            pool,
        }
    }

    /// Validates every stored Agent continuation key before a worker starts.
    pub async fn validate_artifact_coverage(
        &self,
        keyring: &ArtifactKeyring,
    ) -> Result<ValidatedArtifactKeyringCoverage, AgentWorkerReadinessError> {
        self.sessions
            .validate_artifact_keyring_coverage(keyring)
            .await
            .map_err(AgentWorkerReadinessError::ArtifactCoverage)
    }

    /// Revalidates every non-secret startup inventory and returns canonical digests.
    pub async fn startup_coverage(
        &self,
        artifact_keyring: &ArtifactKeyring,
        provider_keyring: Option<&CredentialKeyring>,
    ) -> Result<AgentWorkerCoverageProof, AgentWorkerReadinessError> {
        self.validate_artifact_coverage(artifact_keyring).await?;
        let artifact_rows = sqlx::query_as::<_, (String, i64)>(
            r#"
            SELECT DISTINCT encryption_key_id, encryption_key_version
            FROM agent_execution_artifacts
            WHERE deleted_at IS NULL
            ORDER BY encryption_key_id, encryption_key_version
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AgentWorkerReadinessError::Storage)?;
        let provider_rows = sqlx::query_as::<_, (Uuid, String, i64)>(
            r#"
            SELECT id, credential_key_id, credential_version
            FROM ai_provider_connections
            WHERE deleted_at IS NULL AND credential_key_id IS NOT NULL
            ORDER BY id
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AgentWorkerReadinessError::Storage)?;
        if provider_rows.iter().any(|(_, key_id, _)| {
            provider_keyring.is_none_or(|keyring| !keyring.contains_key_id(key_id))
        }) {
            return Err(AgentWorkerReadinessError::ProviderCredentialCoverage);
        }
        let route_rows = sqlx::query_as::<_, (Uuid, i64, Uuid, Uuid, Uuid, i64)>(
            r#"
            SELECT route.id, route.version, target.id, target.connection_id,
                   target.model_id, target.priority::BIGINT
            FROM ai_route_sets AS route
            INNER JOIN ai_task_routes AS target
              ON target.tenant_id = route.tenant_id
             AND target.route_set_id = route.id
             AND target.deleted_at IS NULL
            WHERE route.deleted_at IS NULL
            ORDER BY route.id, target.priority, target.id
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AgentWorkerReadinessError::Storage)?;

        Ok(AgentWorkerCoverageProof {
            artifact_keys: digest_inventory(
                b"campus-pilot/agent-worker/artifact-coverage/v1",
                artifact_rows
                    .iter()
                    .map(|(key_id, version)| format!("{key_id}:{version}")),
            ),
            provider_keys: digest_inventory(
                b"campus-pilot/agent-worker/provider-key-coverage/v1",
                provider_rows
                    .iter()
                    .map(|(id, key_id, version)| format!("{id}:{key_id}:{version}")),
            ),
            provider_routes: digest_inventory(
                b"campus-pilot/agent-worker/provider-route-coverage/v1",
                route_rows.iter().map(|row| {
                    format!(
                        "{}:{}:{}:{}:{}:{}",
                        row.0, row.1, row.2, row.3, row.4, row.5
                    )
                }),
            ),
        })
    }

    pub async fn register_worker(
        &self,
        worker_key: &str,
    ) -> Result<AgentWorkerInstance, AgentWorkerReadinessError> {
        let id = Uuid::new_v4();
        let version = sqlx::query_scalar::<_, i64>(
            r#"
            WITH clock AS (SELECT STATEMENT_TIMESTAMP() AS now)
            INSERT INTO agent_worker_instances (
                id, worker_key, started_at, status_changed_at,
                heartbeat_at, heartbeat_expires_at, created_at, updated_at
            )
            SELECT $1, $2, now, now, now, now + INTERVAL '45 seconds', now, now
            FROM clock
            RETURNING version
            "#,
        )
        .bind(id)
        .bind(worker_key)
        .fetch_one(&self.pool)
        .await
        .map_err(AgentWorkerReadinessError::Storage)?;
        Ok(AgentWorkerInstance {
            id,
            worker_key: worker_key.to_owned(),
            version,
            coverage: None,
        })
    }

    pub async fn mark_ready(
        &self,
        worker: &mut AgentWorkerInstance,
        coverage: AgentWorkerCoverageProof,
    ) -> Result<(), AgentWorkerReadinessError> {
        let version = sqlx::query_scalar::<_, i64>(
            r#"
            UPDATE agent_worker_instances
            SET status = 'ready',
                artifact_key_coverage_sha256 = $3,
                provider_key_coverage_sha256 = $4,
                provider_route_coverage_sha256 = $5,
                startup_coverage_completed_at = STATEMENT_TIMESTAMP(),
                status_changed_at = STATEMENT_TIMESTAMP(),
                heartbeat_at = STATEMENT_TIMESTAMP(),
                heartbeat_expires_at = STATEMENT_TIMESTAMP() + INTERVAL '45 seconds',
                version = version + 1,
                updated_at = STATEMENT_TIMESTAMP()
            WHERE id = $1 AND version = $2 AND status = 'starting'
              AND deleted_at IS NULL
            RETURNING version
            "#,
        )
        .bind(worker.id)
        .bind(worker.version)
        .bind(coverage.artifact_keys.as_slice())
        .bind(coverage.provider_keys.as_slice())
        .bind(coverage.provider_routes.as_slice())
        .fetch_optional(&self.pool)
        .await
        .map_err(AgentWorkerReadinessError::Storage)?
        .ok_or_else(stale_worker)?;
        worker.version = version;
        worker.coverage = Some(coverage);
        Ok(())
    }

    /// Revalidates coverage before extending a ready worker's durable lease.
    pub async fn heartbeat_ready(
        &self,
        worker: &mut AgentWorkerInstance,
        current: AgentWorkerCoverageProof,
    ) -> Result<(), AgentWorkerReadinessError> {
        if worker.coverage != Some(current) {
            self.mark_unavailable(worker, "startup_coverage_changed")
                .await?;
            return Err(AgentWorkerReadinessError::ProviderCredentialCoverage);
        }
        let version = sqlx::query_scalar::<_, i64>(
            r#"
            UPDATE agent_worker_instances
            SET heartbeat_at = STATEMENT_TIMESTAMP(),
                heartbeat_expires_at = STATEMENT_TIMESTAMP() + INTERVAL '45 seconds',
                version = version + 1,
                updated_at = STATEMENT_TIMESTAMP()
            WHERE id = $1 AND version = $2 AND status = 'ready'
              AND deleted_at IS NULL
            RETURNING version
            "#,
        )
        .bind(worker.id)
        .bind(worker.version)
        .fetch_optional(&self.pool)
        .await
        .map_err(AgentWorkerReadinessError::Storage)?
        .ok_or_else(stale_worker)?;
        worker.version = version;
        Ok(())
    }

    pub async fn mark_draining(
        &self,
        worker: &mut AgentWorkerInstance,
        reason: &str,
    ) -> Result<(), AgentWorkerReadinessError> {
        self.transition(worker, "draining", reason, &["starting", "ready"])
            .await
    }

    pub async fn mark_unavailable(
        &self,
        worker: &mut AgentWorkerInstance,
        reason: &str,
    ) -> Result<(), AgentWorkerReadinessError> {
        self.transition(
            worker,
            "unavailable",
            reason,
            &["starting", "ready", "draining"],
        )
        .await
    }

    async fn transition(
        &self,
        worker: &mut AgentWorkerInstance,
        status: &str,
        reason: &str,
        allowed: &[&str],
    ) -> Result<(), AgentWorkerReadinessError> {
        let version = sqlx::query_scalar::<_, i64>(
            r#"
            UPDATE agent_worker_instances
            SET status = $3, status_reason_code = $4,
                status_changed_at = STATEMENT_TIMESTAMP(),
                version = version + 1, updated_at = STATEMENT_TIMESTAMP()
            WHERE id = $1 AND version = $2 AND status = ANY($5)
              AND deleted_at IS NULL
            RETURNING version
            "#,
        )
        .bind(worker.id)
        .bind(worker.version)
        .bind(status)
        .bind(reason)
        .bind(allowed)
        .fetch_optional(&self.pool)
        .await
        .map_err(AgentWorkerReadinessError::Storage)?
        .ok_or_else(stale_worker)?;
        worker.version = version;
        Ok(())
    }

    pub async fn cleanup_workers(&self) -> Result<(i64, i64), AgentWorkerReadinessError> {
        let expired = sqlx::query_scalar::<_, i64>("SELECT expire_agent_worker_instances()")
            .fetch_one(&self.pool)
            .await
            .map_err(AgentWorkerReadinessError::Storage)?;
        let retired = sqlx::query_scalar::<_, i64>("SELECT retire_agent_worker_instances()")
            .fetch_one(&self.pool)
            .await
            .map_err(AgentWorkerReadinessError::Storage)?;
        Ok((expired, retired))
    }

    /// Returns whether this tenant can safely begin provider execution.
    pub async fn tenant_readiness(
        &self,
        tenant_id: Uuid,
    ) -> Result<AgentWorkerReadiness, AgentWorkerReadinessError> {
        let unsupported_cost_limit = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM agent_limit_rules AS rule
                WHERE rule.tenant_id = $1
                  AND rule.meter_key = 'agent.estimated_cost'
                  AND rule.enforcement = 'hard'
                  AND rule.effective_from <= STATEMENT_TIMESTAMP()
                  AND rule.deleted_at IS NULL
                UNION ALL
                SELECT 1
                FROM entitlement_limits AS entitlement
                WHERE entitlement.tenant_id = $1
                  AND entitlement.limit_key = 'agent.estimated_cost'
                  AND entitlement.enforcement = 'hard'
            )
            "#,
        )
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await
        .map_err(AgentWorkerReadinessError::Storage)?;
        Ok(if unsupported_cost_limit {
            AgentWorkerReadiness {
                ready: false,
                reason: AgentWorkerReadinessReason::EstimatedCostHardLimitRequiresPricing,
            }
        } else {
            AgentWorkerReadiness {
                ready: true,
                reason: AgentWorkerReadinessReason::Ready,
            }
        })
    }
}

fn stale_worker() -> AgentWorkerReadinessError {
    AgentWorkerReadinessError::Storage(sqlx::Error::RowNotFound)
}

fn digest_inventory(domain: &[u8], values: impl IntoIterator<Item = String>) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    for value in values {
        digest.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
        digest.update(value.as_bytes());
    }
    digest.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::{AgentWorkerReadiness, AgentWorkerReadinessReason, digest_inventory};

    #[test]
    fn readiness_reason_is_explicit_and_non_marketing() {
        let readiness = AgentWorkerReadiness {
            ready: false,
            reason: AgentWorkerReadinessReason::EstimatedCostHardLimitRequiresPricing,
        };
        assert!(!readiness.ready);
        assert_eq!(
            serde_json::to_value(readiness.reason).unwrap_or_else(|_| unreachable!()),
            "estimated_cost_hard_limit_requires_pricing"
        );
    }

    #[test]
    fn coverage_digest_is_ordered_and_domain_separated() {
        let first = digest_inventory(b"first", ["a".to_owned(), "b".to_owned()]);
        assert_eq!(
            first,
            digest_inventory(b"first", ["a".to_owned(), "b".to_owned()])
        );
        assert_ne!(
            first,
            digest_inventory(b"first", ["b".to_owned(), "a".to_owned()])
        );
        assert_ne!(
            first,
            digest_inventory(b"second", ["a".to_owned(), "b".to_owned()])
        );
    }
}
