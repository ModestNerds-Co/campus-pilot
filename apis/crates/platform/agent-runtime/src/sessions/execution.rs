//! Persists fenced provider/capability execution evidence and encrypted continuation artifacts.
//!
//! Provider HTTP execution, capability dispatch, and artifact encryption/decryption remain outside
//! this repository. Only normalized identities, safe outcomes, and opaque encrypted envelopes cross
//! this boundary.

use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde_json::{Map, Value, json};
use sqlx::{FromRow, Postgres, Transaction};
use uuid::Uuid;

use super::{
    AgentSessionOps,
    types::{
        AgentSessionError, CapabilityCallDuration, CapabilityCallFailure, CapabilityCallIdentity,
        CapabilityCallPlan, CapabilityCallScope, CapabilityCallSnapshot, CapabilityCallStatus,
        CapabilityResourceReference, EncryptedExecutionArtifact, ExecutionArtifactKind,
        ExecutionSnapshot, ExecutionStepEvidence, ExecutionStepSnapshot, ExecutionStepStatus,
        FinalizationSnapshot, LoadedExecutionArtifact, NormalizedProviderUsage,
        PersistedExecutionArtifact, PersistedExecutionResult, PreparedCapabilityCall,
        PreparedProviderAttempt, ProviderAttemptFailure, ProviderAttemptIdentity,
        ProviderAttemptPlan, ProviderAttemptSnapshot, ProviderAttemptStatus, ProviderTurnIndex,
        ProviderUpstreamFailure, RunCheckpoint, RunEventType, RunLease, RunStatus,
    },
};

const MAX_EXECUTION_STEPS: i16 = 65;
const MAX_EXECUTION_ARTIFACTS: i16 = 33;

impl AgentSessionOps {
    /// Persists one normalized provider attempt and its running step before any provider call.
    pub async fn prepare_provider_attempt(
        &self,
        tenant_id: Uuid,
        lease: &RunLease,
        plan: ProviderAttemptPlan,
    ) -> Result<PreparedProviderAttempt, AgentSessionError> {
        let mut transaction = self.pool.begin().await?;
        let queue = lock_execution_queue(&mut transaction, tenant_id, lease).await?;
        reject_cancelled_queue(&queue)?;
        let checkpoint = RunCheckpoint::from_str(&queue.checkpoint)?;
        if checkpoint != RunCheckpoint::BeforeProvider
            && !checkpoint.can_advance_to(RunCheckpoint::BeforeProvider)
        {
            return Err(AgentSessionError::conflict(
                "provider_attempt_not_ready",
                "This Agent run is not ready to prepare a provider attempt",
            ));
        }
        let run = lock_execution_run(&mut transaction, tenant_id, lease.run_id).await?;
        require_running_run(&run)?;

        if let Some(existing) = find_provider_attempt(
            &mut transaction,
            tenant_id,
            lease.run_id,
            plan.turn_index.get(),
            plan.attempt_index.get(),
        )
        .await?
        {
            ensure_matching_provider_attempt(&existing, &plan, &run.task_class)?;
            if checkpoint != RunCheckpoint::BeforeProvider {
                return Err(AgentSessionError::conflict(
                    "provider_attempt_already_prepared",
                    "This provider attempt was already prepared at a different run checkpoint",
                ));
            }
            transaction.commit().await?;
            return Ok(PreparedProviderAttempt {
                lease: lease.clone(),
                identity: provider_identity(&existing)?,
            });
        }

        validate_provider_attempt_sequence(&mut transaction, tenant_id, lease.run_id, &plan)
            .await?;
        let attempt_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO agent_provider_attempts (
                id, tenant_id, run_id, turn_index, attempt_index, route_set_id,
                route_version, route_target_id, connection_id, credential_version,
                model_snapshot_id, provider_data_approval_id,
                required_provider_data_class, execution_environment_class,
                provider_key, provider_model_id, task_class
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                $13, $14, $15, $16, $17
            )
            "#,
        )
        .bind(attempt_id)
        .bind(tenant_id)
        .bind(lease.run_id)
        .bind(plan.turn_index.get())
        .bind(plan.attempt_index.get())
        .bind(plan.route_set_id)
        .bind(plan.route_version)
        .bind(plan.route_target_id)
        .bind(plan.connection_id)
        .bind(plan.credential_version)
        .bind(plan.model_snapshot_id)
        .bind(plan.provider_data_approval_id)
        .bind(plan.required_provider_data_class.as_str())
        .bind(plan.execution_environment_class.as_str())
        .bind(plan.provider_key.as_str())
        .bind(&plan.provider_model_id)
        .bind(&run.task_class)
        .execute(&mut *transaction)
        .await?;
        let step_id = Uuid::new_v4();
        let step_index = next_step_index(&mut transaction, tenant_id, lease.run_id).await?;
        sqlx::query(
            r#"
            INSERT INTO agent_execution_steps (
                id, tenant_id, run_id, step_index, turn_index, step_kind,
                provider_attempt_id, capability_call_id, input_fingerprint
            )
            VALUES ($1, $2, $3, $4, $5, 'provider_attempt', $6, NULL, $7)
            "#,
        )
        .bind(step_id)
        .bind(tenant_id)
        .bind(lease.run_id)
        .bind(step_index)
        .bind(plan.turn_index.get())
        .bind(attempt_id)
        .bind(plan.input_fingerprint.as_slice())
        .execute(&mut *transaction)
        .await?;
        let next_fence = advance_queue_checkpoint(
            &mut transaction,
            tenant_id,
            lease,
            RunCheckpoint::BeforeProvider,
            true,
        )
        .await?;
        append_execution_event(
            &mut transaction,
            tenant_id,
            lease.run_id,
            RunEventType::ProviderAttemptStarted,
        )
        .await?;
        transaction.commit().await?;
        Ok(PreparedProviderAttempt {
            lease: next_lease(lease, next_fence),
            identity: ProviderAttemptIdentity {
                attempt_id,
                step_id,
                turn_index: plan.turn_index,
                attempt_index: plan.attempt_index,
            },
        })
    }

    /// Marks a prepared provider attempt in-flight under the current random token and fence.
    pub async fn mark_provider_in_flight(
        &self,
        tenant_id: Uuid,
        lease: &RunLease,
        identity: ProviderAttemptIdentity,
    ) -> Result<RunLease, AgentSessionError> {
        let mut transaction = self.pool.begin().await?;
        let queue = lock_execution_queue(&mut transaction, tenant_id, lease).await?;
        reject_cancelled_queue(&queue)?;
        if RunCheckpoint::from_str(&queue.checkpoint)? != RunCheckpoint::BeforeProvider {
            return Err(AgentSessionError::conflict(
                "provider_attempt_not_prepared",
                "This provider attempt is not ready to start",
            ));
        }
        ensure_running_provider_identity(&mut transaction, tenant_id, lease.run_id, identity)
            .await?;
        let next_fence = advance_queue_checkpoint(
            &mut transaction,
            tenant_id,
            lease,
            RunCheckpoint::ProviderInFlight,
            true,
        )
        .await?;
        transaction.commit().await?;
        Ok(next_lease(lease, next_fence))
    }

    /// Persists one normalized provider result and encrypted continuation envelope atomically.
    pub async fn persist_provider_success(
        &self,
        tenant_id: Uuid,
        lease: &RunLease,
        identity: ProviderAttemptIdentity,
        usage: NormalizedProviderUsage,
        artifact: EncryptedExecutionArtifact,
    ) -> Result<PersistedExecutionResult, AgentSessionError> {
        let mut transaction = self.pool.begin().await?;
        let queue = lock_execution_queue(&mut transaction, tenant_id, lease).await?;
        if RunCheckpoint::from_str(&queue.checkpoint)? != RunCheckpoint::ProviderInFlight {
            return Err(AgentSessionError::conflict(
                "provider_attempt_not_in_flight",
                "This provider attempt is not in flight",
            ));
        }
        let run = lock_execution_run(&mut transaction, tenant_id, lease.run_id).await?;
        require_running_run(&run)?;
        ensure_running_provider_identity(&mut transaction, tenant_id, lease.run_id, identity)
            .await?;
        let persisted = insert_artifact(
            &mut transaction,
            tenant_id,
            lease.run_id,
            identity.step_id,
            ExecutionArtifactKind::ProviderResult,
            artifact,
        )
        .await?;
        terminalize_provider_attempt_success(
            &mut transaction,
            tenant_id,
            lease.run_id,
            identity.attempt_id,
            &usage,
        )
        .await?;
        terminalize_step(
            &mut transaction,
            tenant_id,
            lease.run_id,
            identity.step_id,
            "succeeded",
            None,
        )
        .await?;
        let next_fence = advance_queue_checkpoint(
            &mut transaction,
            tenant_id,
            lease,
            RunCheckpoint::ProviderResultPersisted,
            false,
        )
        .await?;
        append_execution_event(
            &mut transaction,
            tenant_id,
            lease.run_id,
            RunEventType::ProviderAttemptFinished,
        )
        .await?;
        transaction.commit().await?;
        Ok(PersistedExecutionResult {
            lease: next_lease(lease, next_fence),
            artifact: persisted,
        })
    }

    /// Persists one known provider failure with exact preflight/upstream provenance.
    pub async fn persist_provider_failure(
        &self,
        tenant_id: Uuid,
        lease: &RunLease,
        identity: ProviderAttemptIdentity,
        failure: ProviderAttemptFailure,
        usage: NormalizedProviderUsage,
    ) -> Result<RunLease, AgentSessionError> {
        if matches!(failure, ProviderAttemptFailure::Preflight(_)) && !usage_is_empty(&usage) {
            return Err(AgentSessionError::invalid(
                "invalid_preflight_usage",
                "Provider preflight failures cannot contain usage or cost",
            ));
        }
        let mut transaction = self.pool.begin().await?;
        let queue = lock_execution_queue(&mut transaction, tenant_id, lease).await?;
        let checkpoint = RunCheckpoint::from_str(&queue.checkpoint)?;
        let checkpoint_matches = provider_failure_checkpoint_matches(checkpoint, failure);
        if !checkpoint_matches {
            return Err(AgentSessionError::conflict(
                "provider_attempt_failure_checkpoint_mismatch",
                "This provider failure does not match the durable run checkpoint",
            ));
        }
        let run = lock_execution_run(&mut transaction, tenant_id, lease.run_id).await?;
        require_running_run(&run)?;
        ensure_running_provider_identity(&mut transaction, tenant_id, lease.run_id, identity)
            .await?;
        terminalize_provider_attempt_failure(
            &mut transaction,
            tenant_id,
            lease.run_id,
            identity.attempt_id,
            failure,
            &usage,
        )
        .await?;
        terminalize_step(
            &mut transaction,
            tenant_id,
            lease.run_id,
            identity.step_id,
            "failed",
            Some(failure.category()),
        )
        .await?;
        let next_checkpoint = match failure {
            ProviderAttemptFailure::Preflight(_) => RunCheckpoint::BeforeProvider,
            ProviderAttemptFailure::Upstream(_) => RunCheckpoint::ProviderResultPersisted,
        };
        let next_fence =
            advance_queue_checkpoint(&mut transaction, tenant_id, lease, next_checkpoint, false)
                .await?;
        append_execution_event(
            &mut transaction,
            tenant_id,
            lease.run_id,
            RunEventType::ProviderAttemptFinished,
        )
        .await?;
        transaction.commit().await?;
        Ok(next_lease(lease, next_fence))
    }

    /// Persists one typed capability call and moves the queue into its in-flight checkpoint.
    pub async fn prepare_capability_call(
        &self,
        tenant_id: Uuid,
        lease: &RunLease,
        plan: CapabilityCallPlan,
    ) -> Result<PreparedCapabilityCall, AgentSessionError> {
        let mut transaction = self.pool.begin().await?;
        let queue = lock_execution_queue(&mut transaction, tenant_id, lease).await?;
        reject_cancelled_queue(&queue)?;
        let checkpoint = RunCheckpoint::from_str(&queue.checkpoint)?;
        if checkpoint != RunCheckpoint::CapabilityInFlight
            && !checkpoint.can_advance_to(RunCheckpoint::CapabilityInFlight)
        {
            return Err(AgentSessionError::conflict(
                "capability_call_not_ready",
                "This Agent run is not ready to prepare a capability call",
            ));
        }
        let run = lock_execution_run(&mut transaction, tenant_id, lease.run_id).await?;
        require_running_run(&run)?;
        if let Some(existing) = find_capability_call(
            &mut transaction,
            tenant_id,
            lease.run_id,
            plan.call_id,
            plan.call_sequence.get(),
        )
        .await?
        {
            ensure_matching_capability_call(&existing, &plan)?;
            if checkpoint != RunCheckpoint::CapabilityInFlight {
                return Err(AgentSessionError::conflict(
                    "capability_call_already_prepared",
                    "This capability call was already prepared at a different run checkpoint",
                ));
            }
            transaction.commit().await?;
            return Ok(PreparedCapabilityCall {
                lease: lease.clone(),
                identity: capability_identity(&existing)?,
            });
        }
        validate_capability_sequence(
            &mut transaction,
            tenant_id,
            lease.run_id,
            plan.turn_index.get(),
            plan.call_sequence.get(),
        )
        .await?;
        let resource_references = capability_resource_json(&plan.scope);
        sqlx::query(
            r#"
            INSERT INTO agent_capability_calls (
                id, tenant_id, run_id, call_sequence, capability_key,
                capability_version, product_operation_key, owning_module_key,
                required_permission, input_fingerprint, scope_kind, resource_references
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
        )
        .bind(plan.call_id)
        .bind(tenant_id)
        .bind(lease.run_id)
        .bind(plan.call_sequence.get())
        .bind(&plan.capability_key)
        .bind(plan.capability_version)
        .bind(&plan.product_operation_key)
        .bind(&plan.owning_module_key)
        .bind(&plan.required_permission)
        .bind(plan.input_fingerprint.as_slice())
        .bind(plan.scope.kind())
        .bind(resource_references)
        .execute(&mut *transaction)
        .await?;
        let step_id = Uuid::new_v4();
        let step_index = next_step_index(&mut transaction, tenant_id, lease.run_id).await?;
        sqlx::query(
            r#"
            INSERT INTO agent_execution_steps (
                id, tenant_id, run_id, step_index, turn_index, step_kind,
                provider_attempt_id, capability_call_id, input_fingerprint
            )
            VALUES ($1, $2, $3, $4, $5, 'capability_call', NULL, $6, $7)
            "#,
        )
        .bind(step_id)
        .bind(tenant_id)
        .bind(lease.run_id)
        .bind(step_index)
        .bind(plan.turn_index.get())
        .bind(plan.call_id)
        .bind(plan.input_fingerprint.as_slice())
        .execute(&mut *transaction)
        .await?;
        let next_fence = advance_queue_checkpoint(
            &mut transaction,
            tenant_id,
            lease,
            RunCheckpoint::CapabilityInFlight,
            true,
        )
        .await?;
        append_execution_event(
            &mut transaction,
            tenant_id,
            lease.run_id,
            RunEventType::CapabilityCallStarted,
        )
        .await?;
        transaction.commit().await?;
        Ok(PreparedCapabilityCall {
            lease: next_lease(lease, next_fence),
            identity: CapabilityCallIdentity {
                call_id: plan.call_id,
                step_id,
                turn_index: plan.turn_index,
                call_sequence: plan.call_sequence,
            },
        })
    }

    /// Persists a redacted encrypted capability result and terminal trail facts atomically.
    pub async fn persist_capability_success(
        &self,
        tenant_id: Uuid,
        lease: &RunLease,
        identity: CapabilityCallIdentity,
        duration: CapabilityCallDuration,
        artifact: EncryptedExecutionArtifact,
    ) -> Result<PersistedExecutionResult, AgentSessionError> {
        let mut transaction = self.pool.begin().await?;
        let queue = lock_execution_queue(&mut transaction, tenant_id, lease).await?;
        if RunCheckpoint::from_str(&queue.checkpoint)? != RunCheckpoint::CapabilityInFlight {
            return Err(AgentSessionError::conflict(
                "capability_call_not_in_flight",
                "This capability call is not in flight",
            ));
        }
        let run = lock_execution_run(&mut transaction, tenant_id, lease.run_id).await?;
        require_running_run(&run)?;
        ensure_running_capability_identity(&mut transaction, tenant_id, lease.run_id, identity)
            .await?;
        let persisted = insert_artifact(
            &mut transaction,
            tenant_id,
            lease.run_id,
            identity.step_id,
            ExecutionArtifactKind::CapabilityResult,
            artifact,
        )
        .await?;
        terminalize_capability_call(
            &mut transaction,
            tenant_id,
            lease.run_id,
            identity.call_id,
            "succeeded",
            None,
            duration.get(),
        )
        .await?;
        terminalize_step(
            &mut transaction,
            tenant_id,
            lease.run_id,
            identity.step_id,
            "succeeded",
            None,
        )
        .await?;
        let next_fence = advance_queue_checkpoint(
            &mut transaction,
            tenant_id,
            lease,
            RunCheckpoint::CapabilityResultPersisted,
            false,
        )
        .await?;
        append_execution_event(
            &mut transaction,
            tenant_id,
            lease.run_id,
            RunEventType::CapabilityCallFinished,
        )
        .await?;
        transaction.commit().await?;
        Ok(PersistedExecutionResult {
            lease: next_lease(lease, next_fence),
            artifact: persisted,
        })
    }

    /// Persists a known capability failure or denial and its redacted model-visible result.
    pub async fn persist_capability_failure(
        &self,
        tenant_id: Uuid,
        lease: &RunLease,
        identity: CapabilityCallIdentity,
        failure: CapabilityCallFailure,
        artifact: EncryptedExecutionArtifact,
    ) -> Result<PersistedExecutionResult, AgentSessionError> {
        let mut transaction = self.pool.begin().await?;
        let queue = lock_execution_queue(&mut transaction, tenant_id, lease).await?;
        if RunCheckpoint::from_str(&queue.checkpoint)? != RunCheckpoint::CapabilityInFlight {
            return Err(AgentSessionError::conflict(
                "capability_call_not_in_flight",
                "This capability call is not in flight",
            ));
        }
        let run = lock_execution_run(&mut transaction, tenant_id, lease.run_id).await?;
        require_running_run(&run)?;
        ensure_running_capability_identity(&mut transaction, tenant_id, lease.run_id, identity)
            .await?;
        let persisted = insert_artifact(
            &mut transaction,
            tenant_id,
            lease.run_id,
            identity.step_id,
            ExecutionArtifactKind::CapabilityResult,
            artifact,
        )
        .await?;
        terminalize_capability_call(
            &mut transaction,
            tenant_id,
            lease.run_id,
            identity.call_id,
            failure.status.as_str(),
            Some(&failure.safe_failure_code),
            failure.duration_ms,
        )
        .await?;
        terminalize_step(
            &mut transaction,
            tenant_id,
            lease.run_id,
            identity.step_id,
            "failed",
            Some(&failure.safe_failure_code),
        )
        .await?;
        let next_fence = advance_queue_checkpoint(
            &mut transaction,
            tenant_id,
            lease,
            RunCheckpoint::CapabilityResultPersisted,
            false,
        )
        .await?;
        append_execution_event(
            &mut transaction,
            tenant_id,
            lease.run_id,
            RunEventType::CapabilityCallFinished,
        )
        .await?;
        transaction.commit().await?;
        Ok(PersistedExecutionResult {
            lease: next_lease(lease, next_fence),
            artifact: persisted,
        })
    }

    /// Persists the unique encrypted final response and its succeeded finalize step.
    pub async fn persist_final_response(
        &self,
        tenant_id: Uuid,
        lease: &RunLease,
        turn_index: ProviderTurnIndex,
        artifact: EncryptedExecutionArtifact,
    ) -> Result<PersistedExecutionResult, AgentSessionError> {
        let mut transaction = self.pool.begin().await?;
        let queue = lock_execution_queue(&mut transaction, tenant_id, lease).await?;
        reject_cancelled_queue(&queue)?;
        let checkpoint = RunCheckpoint::from_str(&queue.checkpoint)?;
        if checkpoint != RunCheckpoint::Finalizing
            && !checkpoint.can_advance_to(RunCheckpoint::Finalizing)
        {
            return Err(AgentSessionError::conflict(
                "final_response_not_ready",
                "This Agent run is not ready to persist its final response",
            ));
        }
        let run = lock_execution_run(&mut transaction, tenant_id, lease.run_id).await?;
        require_running_run(&run)?;
        if let Some(existing) =
            find_final_artifact(&mut transaction, tenant_id, lease.run_id).await?
        {
            if existing.step_status != "succeeded"
                || checkpoint != RunCheckpoint::Finalizing
                || existing.plaintext_sha256.as_slice() != artifact.plaintext_sha256
                || existing.plaintext_length != artifact.plaintext_length
            {
                return Err(AgentSessionError::conflict(
                    "final_response_conflict",
                    "This Agent run already has different final response evidence",
                ));
            }
            transaction.commit().await?;
            return Ok(PersistedExecutionResult {
                lease: lease.clone(),
                artifact: PersistedExecutionArtifact {
                    id: existing.artifact_id,
                    step_id: existing.step_id,
                    kind: ExecutionArtifactKind::FinalResponse,
                    sequence: existing.artifact_sequence,
                },
            });
        }
        let step_id = Uuid::new_v4();
        let step_index = next_step_index(&mut transaction, tenant_id, lease.run_id).await?;
        sqlx::query(
            r#"
            INSERT INTO agent_execution_steps (
                id, tenant_id, run_id, step_index, turn_index, step_kind,
                provider_attempt_id, capability_call_id, input_fingerprint
            )
            VALUES ($1, $2, $3, $4, $5, 'finalize', NULL, NULL, $6)
            "#,
        )
        .bind(step_id)
        .bind(tenant_id)
        .bind(lease.run_id)
        .bind(step_index)
        .bind(turn_index.get())
        .bind(artifact.plaintext_sha256.as_slice())
        .execute(&mut *transaction)
        .await?;
        let persisted = insert_artifact(
            &mut transaction,
            tenant_id,
            lease.run_id,
            step_id,
            ExecutionArtifactKind::FinalResponse,
            artifact,
        )
        .await?;
        terminalize_step(
            &mut transaction,
            tenant_id,
            lease.run_id,
            step_id,
            "succeeded",
            None,
        )
        .await?;
        let next_fence = advance_queue_checkpoint(
            &mut transaction,
            tenant_id,
            lease,
            RunCheckpoint::Finalizing,
            true,
        )
        .await?;
        transaction.commit().await?;
        Ok(PersistedExecutionResult {
            lease: next_lease(lease, next_fence),
            artifact: persisted,
        })
    }

    /// Loads the bounded private execution trail for the currently fenced worker.
    pub async fn load_execution_snapshot(
        &self,
        tenant_id: Uuid,
        lease: &RunLease,
    ) -> Result<ExecutionSnapshot, AgentSessionError> {
        let mut transaction = self.pool.begin().await?;
        let queue = lock_execution_queue(&mut transaction, tenant_id, lease).await?;
        let rows = sqlx::query_as::<_, ExecutionSnapshotRow>(
            r#"
            SELECT
                s.id AS step_id,
                s.step_index,
                s.turn_index,
                s.step_kind,
                s.input_fingerprint AS step_input_fingerprint,
                s.status AS step_status,
                s.safe_failure_code AS step_safe_failure_code,
                p.id AS provider_attempt_id,
                p.attempt_index AS provider_attempt_index,
                p.route_set_id,
                p.route_version,
                p.route_target_id,
                p.connection_id,
                p.credential_version,
                p.model_snapshot_id,
                p.provider_data_approval_id,
                p.required_provider_data_class,
                p.execution_environment_class,
                p.provider_key,
                p.provider_model_id,
                p.task_class,
                p.status AS provider_status,
                p.failure_origin AS provider_failure_origin,
                p.failure_category AS provider_failure_category,
                c.id AS capability_call_id,
                c.call_sequence AS capability_call_sequence,
                c.capability_key,
                c.capability_version,
                c.product_operation_key,
                c.owning_module_key,
                c.required_permission,
                c.scope_kind AS capability_scope_kind,
                c.resource_references AS capability_resource_references,
                c.status AS capability_status,
                c.safe_failure_code AS capability_safe_failure_code,
                c.duration_ms AS capability_duration_ms,
                a.id AS artifact_id,
                a.artifact_sequence,
                a.artifact_kind,
                a.ciphertext,
                a.ciphertext_sha256,
                a.plaintext_sha256,
                a.nonce,
                a.encryption_key_id,
                a.encryption_key_version,
                a.plaintext_length
            FROM agent_execution_steps s
            LEFT JOIN agent_provider_attempts p
              ON p.id = s.provider_attempt_id
             AND p.tenant_id = s.tenant_id
             AND p.run_id = s.run_id
            LEFT JOIN agent_capability_calls c
              ON c.id = s.capability_call_id
             AND c.tenant_id = s.tenant_id
             AND c.run_id = s.run_id
            LEFT JOIN agent_execution_artifacts a
              ON a.step_id = s.id
             AND a.tenant_id = s.tenant_id
             AND a.run_id = s.run_id
             AND a.deleted_at IS NULL
            WHERE s.tenant_id = $1 AND s.run_id = $2 AND s.deleted_at IS NULL
            ORDER BY s.step_index
            "#,
        )
        .bind(tenant_id)
        .bind(lease.run_id)
        .fetch_all(&mut *transaction)
        .await?;
        let steps = rows
            .into_iter()
            .map(ExecutionStepSnapshot::try_from)
            .collect::<Result<_, _>>()?;
        transaction.commit().await?;
        Ok(ExecutionSnapshot {
            run_id: lease.run_id,
            checkpoint: RunCheckpoint::from_str(&queue.checkpoint)?,
            steps,
        })
    }
}

fn provider_failure_checkpoint_matches(
    checkpoint: RunCheckpoint,
    failure: ProviderAttemptFailure,
) -> bool {
    match failure {
        // A second, version-bound provider preflight runs immediately before
        // network I/O. It is durably after the dispatch claim, yet is still
        // preflight evidence because no upstream request was sent.
        ProviderAttemptFailure::Preflight(_) => matches!(
            checkpoint,
            RunCheckpoint::BeforeProvider | RunCheckpoint::ProviderInFlight
        ),
        ProviderAttemptFailure::Upstream(_) => checkpoint == RunCheckpoint::ProviderInFlight,
    }
}

#[derive(FromRow)]
struct ExecutionQueueRow {
    checkpoint: String,
    cancel_requested_at: Option<DateTime<Utc>>,
}

#[derive(FromRow)]
struct ExecutionRunRow {
    task_class: String,
    status: String,
}

#[derive(FromRow)]
struct ExistingProviderAttemptRow {
    attempt_id: Uuid,
    step_id: Uuid,
    turn_index: i16,
    attempt_index: i16,
    route_set_id: Uuid,
    route_version: i64,
    route_target_id: Uuid,
    connection_id: Uuid,
    credential_version: i64,
    model_snapshot_id: Uuid,
    provider_data_approval_id: Uuid,
    required_provider_data_class: String,
    execution_environment_class: String,
    provider_key: String,
    provider_model_id: String,
    task_class: String,
    attempt_status: String,
    step_status: String,
    input_fingerprint: Vec<u8>,
}

#[derive(FromRow)]
struct LastProviderAttemptRow {
    turn_index: i16,
    attempt_index: i16,
    status: String,
    failure_origin: Option<String>,
    failure_category: Option<String>,
}

#[derive(FromRow)]
struct ExistingCapabilityCallRow {
    call_id: Uuid,
    step_id: Uuid,
    turn_index: i16,
    call_sequence: i16,
    capability_key: String,
    capability_version: i32,
    product_operation_key: String,
    owning_module_key: String,
    required_permission: String,
    input_fingerprint: Vec<u8>,
    scope_kind: String,
    resource_references: Value,
    call_status: String,
    step_status: String,
}

#[derive(FromRow)]
struct FinalArtifactRow {
    artifact_id: Uuid,
    step_id: Uuid,
    artifact_sequence: i16,
    plaintext_sha256: Vec<u8>,
    plaintext_length: i32,
    step_status: String,
}

#[derive(FromRow)]
struct StoredArtifactRow {
    id: Uuid,
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

#[derive(FromRow)]
struct ExecutionSnapshotRow {
    step_id: Uuid,
    step_index: i16,
    turn_index: i16,
    step_kind: String,
    step_input_fingerprint: Vec<u8>,
    step_status: String,
    step_safe_failure_code: Option<String>,
    provider_attempt_id: Option<Uuid>,
    provider_attempt_index: Option<i16>,
    route_set_id: Option<Uuid>,
    route_version: Option<i64>,
    route_target_id: Option<Uuid>,
    connection_id: Option<Uuid>,
    credential_version: Option<i64>,
    model_snapshot_id: Option<Uuid>,
    provider_data_approval_id: Option<Uuid>,
    required_provider_data_class: Option<String>,
    execution_environment_class: Option<String>,
    provider_key: Option<String>,
    provider_model_id: Option<String>,
    task_class: Option<String>,
    provider_status: Option<String>,
    provider_failure_origin: Option<String>,
    provider_failure_category: Option<String>,
    capability_call_id: Option<Uuid>,
    capability_call_sequence: Option<i16>,
    capability_key: Option<String>,
    capability_version: Option<i32>,
    product_operation_key: Option<String>,
    owning_module_key: Option<String>,
    required_permission: Option<String>,
    capability_scope_kind: Option<String>,
    capability_resource_references: Option<Value>,
    capability_status: Option<String>,
    capability_safe_failure_code: Option<String>,
    capability_duration_ms: Option<i64>,
    artifact_id: Option<Uuid>,
    artifact_sequence: Option<i16>,
    artifact_kind: Option<String>,
    ciphertext: Option<Vec<u8>>,
    ciphertext_sha256: Option<Vec<u8>>,
    plaintext_sha256: Option<Vec<u8>>,
    nonce: Option<Vec<u8>>,
    encryption_key_id: Option<String>,
    encryption_key_version: Option<i64>,
    plaintext_length: Option<i32>,
}

impl TryFrom<StoredArtifactRow> for LoadedExecutionArtifact {
    type Error = AgentSessionError;

    fn try_from(row: StoredArtifactRow) -> Result<Self, Self::Error> {
        let ciphertext_sha256 = row
            .ciphertext_sha256
            .try_into()
            .map_err(|_| AgentSessionError::storage_contract())?;
        let plaintext_sha256 = row
            .plaintext_sha256
            .try_into()
            .map_err(|_| AgentSessionError::storage_contract())?;
        LoadedExecutionArtifact::from_stored(
            row.id,
            row.step_id,
            ExecutionArtifactKind::from_str(&row.artifact_kind)?,
            row.artifact_sequence,
            row.ciphertext,
            ciphertext_sha256,
            plaintext_sha256,
            row.nonce,
            row.encryption_key_id,
            row.encryption_key_version,
            usize::try_from(row.plaintext_length)
                .map_err(|_| AgentSessionError::storage_contract())?,
        )
    }
}

impl TryFrom<ExecutionSnapshotRow> for ExecutionStepSnapshot {
    type Error = AgentSessionError;

    fn try_from(mut row: ExecutionSnapshotRow) -> Result<Self, Self::Error> {
        let artifact = take_snapshot_artifact(&mut row)?;
        let step_status = ExecutionStepStatus::from_str(&row.step_status)?;
        let step_id = row.step_id;
        let turn_index = ProviderTurnIndex::parse(
            u16::try_from(row.turn_index).map_err(|_| AgentSessionError::storage_contract())?,
        )?;
        let step = ExecutionStepEvidence {
            step_id,
            step_index: u16::try_from(row.step_index)
                .map_err(|_| AgentSessionError::storage_contract())?,
            turn_index,
            input_fingerprint: row
                .step_input_fingerprint
                .as_slice()
                .try_into()
                .map_err(|_| AgentSessionError::storage_contract())?,
            status: step_status,
            safe_failure_code: row.step_safe_failure_code.take(),
            artifact,
        };
        match row.step_kind.as_str() {
            "provider_attempt" => provider_snapshot(row, step, step_id, turn_index),
            "capability_call" => capability_snapshot(row, step, step_id, turn_index),
            "finalize" => {
                ensure_finalization_snapshot_shape(&step)?;
                Ok(Self::Finalization(FinalizationSnapshot { step }))
            }
            _ => Err(AgentSessionError::storage_contract()),
        }
    }
}

fn take_snapshot_artifact(
    row: &mut ExecutionSnapshotRow,
) -> Result<Option<LoadedExecutionArtifact>, AgentSessionError> {
    let any_present = row.artifact_id.is_some()
        || row.artifact_sequence.is_some()
        || row.artifact_kind.is_some()
        || row.ciphertext.is_some()
        || row.ciphertext_sha256.is_some()
        || row.plaintext_sha256.is_some()
        || row.nonce.is_some()
        || row.encryption_key_id.is_some()
        || row.encryption_key_version.is_some()
        || row.plaintext_length.is_some();
    if !any_present {
        return Ok(None);
    }
    StoredArtifactRow {
        id: row
            .artifact_id
            .take()
            .ok_or_else(AgentSessionError::storage_contract)?,
        step_id: row.step_id,
        artifact_sequence: row
            .artifact_sequence
            .take()
            .ok_or_else(AgentSessionError::storage_contract)?,
        artifact_kind: row
            .artifact_kind
            .take()
            .ok_or_else(AgentSessionError::storage_contract)?,
        ciphertext: row
            .ciphertext
            .take()
            .ok_or_else(AgentSessionError::storage_contract)?,
        ciphertext_sha256: row
            .ciphertext_sha256
            .take()
            .ok_or_else(AgentSessionError::storage_contract)?,
        plaintext_sha256: row
            .plaintext_sha256
            .take()
            .ok_or_else(AgentSessionError::storage_contract)?,
        nonce: row
            .nonce
            .take()
            .ok_or_else(AgentSessionError::storage_contract)?,
        encryption_key_id: row
            .encryption_key_id
            .take()
            .ok_or_else(AgentSessionError::storage_contract)?,
        encryption_key_version: row
            .encryption_key_version
            .take()
            .ok_or_else(AgentSessionError::storage_contract)?,
        plaintext_length: row
            .plaintext_length
            .take()
            .ok_or_else(AgentSessionError::storage_contract)?,
    }
    .try_into()
    .map(Some)
}

fn provider_snapshot(
    row: ExecutionSnapshotRow,
    step: ExecutionStepEvidence,
    step_id: Uuid,
    turn_index: ProviderTurnIndex,
) -> Result<ExecutionStepSnapshot, AgentSessionError> {
    let status = ProviderAttemptStatus::from_str(
        row.provider_status
            .as_deref()
            .ok_or_else(AgentSessionError::storage_contract)?,
    )?;
    let failure = match (
        row.provider_failure_origin.as_deref(),
        row.provider_failure_category.as_deref(),
    ) {
        (Some(origin), Some(category)) => {
            Some(ProviderAttemptFailure::from_stored(origin, category)?)
        }
        (None, None) => None,
        _ => return Err(AgentSessionError::storage_contract()),
    };
    if (status == ProviderAttemptStatus::Succeeded) != step.artifact.is_some()
        || step
            .artifact
            .as_ref()
            .is_some_and(|value| value.kind != ExecutionArtifactKind::ProviderResult)
        || (status == ProviderAttemptStatus::Failed) != failure.is_some()
    {
        return Err(AgentSessionError::storage_contract());
    }
    Ok(ExecutionStepSnapshot::ProviderAttempt(
        ProviderAttemptSnapshot {
            identity: ProviderAttemptIdentity {
                attempt_id: row
                    .provider_attempt_id
                    .ok_or_else(AgentSessionError::storage_contract)?,
                step_id,
                turn_index,
                attempt_index: super::types::ProviderAttemptIndex::parse(
                    u8::try_from(
                        row.provider_attempt_index
                            .ok_or_else(AgentSessionError::storage_contract)?,
                    )
                    .map_err(|_| AgentSessionError::storage_contract())?,
                )?,
            },
            route_set_id: row
                .route_set_id
                .ok_or_else(AgentSessionError::storage_contract)?,
            route_version: row
                .route_version
                .ok_or_else(AgentSessionError::storage_contract)?,
            route_target_id: row
                .route_target_id
                .ok_or_else(AgentSessionError::storage_contract)?,
            connection_id: row
                .connection_id
                .ok_or_else(AgentSessionError::storage_contract)?,
            credential_version: row
                .credential_version
                .ok_or_else(AgentSessionError::storage_contract)?,
            model_snapshot_id: row
                .model_snapshot_id
                .ok_or_else(AgentSessionError::storage_contract)?,
            provider_data_approval_id: row
                .provider_data_approval_id
                .ok_or_else(AgentSessionError::storage_contract)?,
            required_provider_data_class: cp_common::ProviderDataClass::from_str(
                row.required_provider_data_class
                    .as_deref()
                    .ok_or_else(AgentSessionError::storage_contract)?,
            )
            .map_err(|_| AgentSessionError::storage_contract())?,
            execution_environment_class: cp_common::ProviderExecutionEnvironmentClass::from_str(
                row.execution_environment_class
                    .as_deref()
                    .ok_or_else(AgentSessionError::storage_contract)?,
            )
            .map_err(|_| AgentSessionError::storage_contract())?,
            provider_key: super::types::AgentProviderKey::from_stored(
                row.provider_key
                    .as_deref()
                    .ok_or_else(AgentSessionError::storage_contract)?,
            )?,
            provider_model_id: row
                .provider_model_id
                .ok_or_else(AgentSessionError::storage_contract)?,
            task_class: crate::TaskClass::from_str(
                row.task_class
                    .as_deref()
                    .ok_or_else(AgentSessionError::storage_contract)?,
            )
            .map_err(|_| AgentSessionError::storage_contract())?,
            status,
            failure,
            step,
        },
    ))
}

fn capability_snapshot(
    row: ExecutionSnapshotRow,
    step: ExecutionStepEvidence,
    step_id: Uuid,
    turn_index: ProviderTurnIndex,
) -> Result<ExecutionStepSnapshot, AgentSessionError> {
    let status = CapabilityCallStatus::from_str(
        row.capability_status
            .as_deref()
            .ok_or_else(AgentSessionError::storage_contract)?,
    )?;
    let should_have_artifact = matches!(
        status,
        CapabilityCallStatus::Succeeded
            | CapabilityCallStatus::Failed
            | CapabilityCallStatus::Denied
    );
    if should_have_artifact != step.artifact.is_some()
        || step
            .artifact
            .as_ref()
            .is_some_and(|value| value.kind != ExecutionArtifactKind::CapabilityResult)
    {
        return Err(AgentSessionError::storage_contract());
    }
    let duration_ms = row
        .capability_duration_ms
        .map(|value| u64::try_from(value).map_err(|_| AgentSessionError::storage_contract()))
        .transpose()?;
    Ok(ExecutionStepSnapshot::CapabilityCall(
        CapabilityCallSnapshot {
            identity: CapabilityCallIdentity {
                call_id: row
                    .capability_call_id
                    .ok_or_else(AgentSessionError::storage_contract)?,
                step_id,
                turn_index,
                call_sequence: super::types::CapabilityCallSequence::parse(
                    u16::try_from(
                        row.capability_call_sequence
                            .ok_or_else(AgentSessionError::storage_contract)?,
                    )
                    .map_err(|_| AgentSessionError::storage_contract())?,
                )?,
            },
            capability_key: row
                .capability_key
                .ok_or_else(AgentSessionError::storage_contract)?,
            capability_version: row
                .capability_version
                .ok_or_else(AgentSessionError::storage_contract)?,
            product_operation_key: row
                .product_operation_key
                .ok_or_else(AgentSessionError::storage_contract)?,
            owning_module_key: row
                .owning_module_key
                .ok_or_else(AgentSessionError::storage_contract)?,
            required_permission: row
                .required_permission
                .ok_or_else(AgentSessionError::storage_contract)?,
            scope: capability_scope_from_stored(
                row.capability_scope_kind
                    .as_deref()
                    .ok_or_else(AgentSessionError::storage_contract)?,
                row.capability_resource_references
                    .ok_or_else(AgentSessionError::storage_contract)?,
            )?,
            status,
            safe_failure_code: row.capability_safe_failure_code,
            duration_ms,
            step,
        },
    ))
}

fn capability_scope_from_stored(
    kind: &str,
    resources: Value,
) -> Result<CapabilityCallScope, AgentSessionError> {
    let resources = resources
        .as_array()
        .ok_or_else(AgentSessionError::storage_contract)?;
    match kind {
        "tenant_wide" if resources.is_empty() => Ok(CapabilityCallScope::TenantWide),
        "resources" => CapabilityCallScope::resources(
            resources
                .iter()
                .map(|value| {
                    let object = value
                        .as_object()
                        .filter(|object| object.len() == 2)
                        .ok_or_else(AgentSessionError::storage_contract)?;
                    CapabilityResourceReference::parse(
                        object
                            .get("kind")
                            .and_then(Value::as_str)
                            .ok_or_else(AgentSessionError::storage_contract)?,
                        object
                            .get("id")
                            .and_then(Value::as_str)
                            .ok_or_else(AgentSessionError::storage_contract)?,
                    )
                })
                .collect::<Result<_, _>>()?,
        ),
        _ => Err(AgentSessionError::storage_contract()),
    }
}

fn ensure_finalization_snapshot_shape(
    step: &ExecutionStepEvidence,
) -> Result<(), AgentSessionError> {
    if step
        .artifact
        .as_ref()
        .is_some_and(|value| value.kind != ExecutionArtifactKind::FinalResponse)
        || (step.status == ExecutionStepStatus::Succeeded) != step.artifact.is_some()
    {
        Err(AgentSessionError::storage_contract())
    } else {
        Ok(())
    }
}

async fn lock_execution_queue(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    lease: &RunLease,
) -> Result<ExecutionQueueRow, AgentSessionError> {
    sqlx::query_as::<_, ExecutionQueueRow>(
        r#"
        SELECT checkpoint, cancel_requested_at
        FROM agent_run_queue
        WHERE tenant_id = $1
          AND run_id = $2
          AND state = 'leased'
          AND leased_by = $3
          AND lease_token = $4
          AND version = $5
          AND lease_expires_at > STATEMENT_TIMESTAMP()
          AND deleted_at IS NULL
        FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(lease.run_id)
    .bind(&lease.worker_id)
    .bind(lease.lease_token)
    .bind(lease.fence_version)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(AgentSessionError::LeaseLost)
}

async fn lock_execution_run(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
) -> Result<ExecutionRunRow, AgentSessionError> {
    sqlx::query_as::<_, ExecutionRunRow>(
        r#"
        SELECT task_class, status
        FROM agent_runs
        WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(run_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(AgentSessionError::RunNotFound)
}

fn require_running_run(run: &ExecutionRunRow) -> Result<(), AgentSessionError> {
    if RunStatus::from_str(&run.status)? == RunStatus::Running {
        Ok(())
    } else {
        Err(AgentSessionError::conflict(
            "run_not_running",
            "Only a running Agent run can persist execution work",
        ))
    }
}

fn reject_cancelled_queue(queue: &ExecutionQueueRow) -> Result<(), AgentSessionError> {
    if queue.cancel_requested_at.is_some() {
        Err(AgentSessionError::conflict(
            "run_cancel_requested",
            "Cancellation was requested for this Agent run",
        ))
    } else {
        Ok(())
    }
}

async fn find_provider_attempt(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
    turn_index: i16,
    attempt_index: i16,
) -> Result<Option<ExistingProviderAttemptRow>, AgentSessionError> {
    Ok(sqlx::query_as::<_, ExistingProviderAttemptRow>(
        r#"
        SELECT a.id AS attempt_id, s.id AS step_id, a.turn_index, a.attempt_index,
               a.route_set_id, a.route_version, a.route_target_id, a.connection_id,
               a.credential_version, a.model_snapshot_id,
               a.provider_data_approval_id, a.required_provider_data_class,
               a.execution_environment_class, a.provider_key,
               a.provider_model_id, a.task_class, a.status AS attempt_status,
               s.status AS step_status, s.input_fingerprint
        FROM agent_provider_attempts a
        JOIN agent_execution_steps s
          ON s.tenant_id = a.tenant_id
         AND s.run_id = a.run_id
         AND s.provider_attempt_id = a.id
        WHERE a.tenant_id = $1
          AND a.run_id = $2
          AND a.turn_index = $3
          AND a.attempt_index = $4
        FOR UPDATE OF a, s
        "#,
    )
    .bind(tenant_id)
    .bind(run_id)
    .bind(turn_index)
    .bind(attempt_index)
    .fetch_optional(&mut **transaction)
    .await?)
}

fn ensure_matching_provider_attempt(
    existing: &ExistingProviderAttemptRow,
    plan: &ProviderAttemptPlan,
    task_class: &str,
) -> Result<(), AgentSessionError> {
    let matches = existing.route_set_id == plan.route_set_id
        && existing.route_version == plan.route_version
        && existing.route_target_id == plan.route_target_id
        && existing.connection_id == plan.connection_id
        && existing.credential_version == plan.credential_version
        && existing.model_snapshot_id == plan.model_snapshot_id
        && existing.provider_data_approval_id == plan.provider_data_approval_id
        && existing.required_provider_data_class == plan.required_provider_data_class.as_str()
        && existing.execution_environment_class == plan.execution_environment_class.as_str()
        && existing.provider_key == plan.provider_key.as_str()
        && existing.provider_model_id == plan.provider_model_id
        && existing.task_class == task_class
        && existing.input_fingerprint.as_slice() == plan.input_fingerprint
        && existing.attempt_status == "running"
        && existing.step_status == "running";
    if matches {
        Ok(())
    } else {
        Err(AgentSessionError::conflict(
            "provider_attempt_identity_conflict",
            "This provider turn and attempt already has different durable facts",
        ))
    }
}

fn provider_identity(
    existing: &ExistingProviderAttemptRow,
) -> Result<ProviderAttemptIdentity, AgentSessionError> {
    Ok(ProviderAttemptIdentity {
        attempt_id: existing.attempt_id,
        step_id: existing.step_id,
        turn_index: super::types::ProviderTurnIndex::parse(
            u16::try_from(existing.turn_index)
                .map_err(|_| AgentSessionError::storage_contract())?,
        )?,
        attempt_index: super::types::ProviderAttemptIndex::parse(
            u8::try_from(existing.attempt_index)
                .map_err(|_| AgentSessionError::storage_contract())?,
        )?,
    })
}

async fn validate_provider_attempt_sequence(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
    plan: &ProviderAttemptPlan,
) -> Result<(), AgentSessionError> {
    let last = sqlx::query_as::<_, LastProviderAttemptRow>(
        r#"
        SELECT turn_index, attempt_index, status, failure_origin, failure_category
        FROM agent_provider_attempts
        WHERE tenant_id = $1 AND run_id = $2
        ORDER BY turn_index DESC, attempt_index DESC
        LIMIT 1
        "#,
    )
    .bind(tenant_id)
    .bind(run_id)
    .fetch_optional(&mut **transaction)
    .await?;
    let valid = match last {
        None => plan.turn_index.get() == 1 && plan.attempt_index.get() == 1,
        Some(last) if plan.turn_index.get() == last.turn_index => {
            provider_attempt_allows_fallback(&last)?
                && plan.attempt_index.get() == last.attempt_index + 1
        }
        Some(last) => {
            last.status == "succeeded"
                && plan.turn_index.get() == last.turn_index + 1
                && plan.attempt_index.get() == 1
        }
    };
    if valid {
        Ok(())
    } else {
        Err(AgentSessionError::conflict(
            "invalid_provider_attempt_sequence",
            "Provider turns and fallback attempts must advance in order",
        ))
    }
}

fn provider_attempt_allows_fallback(
    attempt: &LastProviderAttemptRow,
) -> Result<bool, AgentSessionError> {
    if attempt.status != "failed" {
        return Ok(false);
    }
    let (origin, category) = match (
        attempt.failure_origin.as_deref(),
        attempt.failure_category.as_deref(),
    ) {
        (Some(origin), Some(category)) => (origin, category),
        _ => return Err(AgentSessionError::storage_contract()),
    };
    let failure = ProviderAttemptFailure::from_stored(origin, category)?;
    Ok(matches!(
        failure,
        ProviderAttemptFailure::Upstream(
            ProviderUpstreamFailure::RateLimited | ProviderUpstreamFailure::Unavailable
        )
    ))
}

async fn ensure_running_provider_identity(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
    identity: ProviderAttemptIdentity,
) -> Result<(), AgentSessionError> {
    let valid = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM agent_provider_attempts a
            JOIN agent_execution_steps s
              ON s.tenant_id = a.tenant_id
             AND s.run_id = a.run_id
             AND s.provider_attempt_id = a.id
            WHERE a.tenant_id = $1
              AND a.run_id = $2
              AND a.id = $3
              AND s.id = $4
              AND a.turn_index = $5
              AND a.attempt_index = $6
              AND a.status = 'running'
              AND s.status = 'running'
        )
        "#,
    )
    .bind(tenant_id)
    .bind(run_id)
    .bind(identity.attempt_id)
    .bind(identity.step_id)
    .bind(identity.turn_index.get())
    .bind(identity.attempt_index.get())
    .fetch_one(&mut **transaction)
    .await?;
    if valid {
        Ok(())
    } else {
        Err(AgentSessionError::conflict(
            "provider_attempt_not_running",
            "This provider attempt is not in the expected running state",
        ))
    }
}

async fn find_capability_call(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
    call_id: Uuid,
    call_sequence: i16,
) -> Result<Option<ExistingCapabilityCallRow>, AgentSessionError> {
    let mut rows = sqlx::query_as::<_, ExistingCapabilityCallRow>(
        r#"
        SELECT c.id AS call_id, s.id AS step_id, s.turn_index, c.call_sequence,
               c.capability_key, c.capability_version, c.product_operation_key,
               c.owning_module_key, c.required_permission, c.input_fingerprint,
               c.scope_kind, c.resource_references, c.status AS call_status,
               s.status AS step_status
        FROM agent_capability_calls c
        JOIN agent_execution_steps s
          ON s.tenant_id = c.tenant_id
         AND s.run_id = c.run_id
         AND s.capability_call_id = c.id
        WHERE c.tenant_id = $1
          AND c.run_id = $2
          AND (c.id = $3 OR c.call_sequence = $4)
        ORDER BY c.call_sequence
        FOR UPDATE OF c, s
        "#,
    )
    .bind(tenant_id)
    .bind(run_id)
    .bind(call_id)
    .bind(call_sequence)
    .fetch_all(&mut **transaction)
    .await?;
    if rows.len() > 1 {
        return Err(AgentSessionError::conflict(
            "capability_call_identity_conflict",
            "Capability call ID and sequence refer to different durable calls",
        ));
    }
    Ok(rows.pop())
}

fn ensure_matching_capability_call(
    existing: &ExistingCapabilityCallRow,
    plan: &CapabilityCallPlan,
) -> Result<(), AgentSessionError> {
    let matches = existing.call_id == plan.call_id
        && existing.turn_index == plan.turn_index.get()
        && existing.call_sequence == plan.call_sequence.get()
        && existing.capability_key == plan.capability_key
        && existing.capability_version == plan.capability_version
        && existing.product_operation_key == plan.product_operation_key
        && existing.owning_module_key == plan.owning_module_key
        && existing.required_permission == plan.required_permission
        && existing.input_fingerprint.as_slice() == plan.input_fingerprint
        && existing.scope_kind == plan.scope.kind()
        && existing.resource_references == capability_resource_json(&plan.scope)
        && existing.call_status == "running"
        && existing.step_status == "running";
    if matches {
        Ok(())
    } else {
        Err(AgentSessionError::conflict(
            "capability_call_identity_conflict",
            "This capability call ID or sequence already has different durable facts",
        ))
    }
}

fn capability_identity(
    existing: &ExistingCapabilityCallRow,
) -> Result<CapabilityCallIdentity, AgentSessionError> {
    Ok(CapabilityCallIdentity {
        call_id: existing.call_id,
        step_id: existing.step_id,
        turn_index: super::types::ProviderTurnIndex::parse(
            u16::try_from(existing.turn_index)
                .map_err(|_| AgentSessionError::storage_contract())?,
        )?,
        call_sequence: super::types::CapabilityCallSequence::parse(
            u16::try_from(existing.call_sequence)
                .map_err(|_| AgentSessionError::storage_contract())?,
        )?,
    })
}

async fn validate_capability_sequence(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
    turn_index: i16,
    call_sequence: i16,
) -> Result<(), AgentSessionError> {
    let (last, turn_exists) = sqlx::query_as::<_, (i16, bool)>(
        r#"
        SELECT COALESCE(MAX(c.call_sequence), 0)::SMALLINT,
               EXISTS (
                   SELECT 1
                   FROM agent_execution_steps s
                   WHERE s.tenant_id = $1
                     AND s.run_id = $2
                     AND s.turn_index = $3
                     AND s.step_kind = 'capability_call'
               )
        FROM agent_capability_calls c
        WHERE c.tenant_id = $1 AND c.run_id = $2
        "#,
    )
    .bind(tenant_id)
    .bind(run_id)
    .bind(turn_index)
    .fetch_one(&mut **transaction)
    .await?;
    if !turn_exists && call_sequence == last + 1 {
        Ok(())
    } else {
        Err(AgentSessionError::conflict(
            "invalid_capability_call_sequence",
            "Capability calls must advance in order with at most one call per provider turn",
        ))
    }
}

async fn find_final_artifact(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
) -> Result<Option<FinalArtifactRow>, AgentSessionError> {
    Ok(sqlx::query_as::<_, FinalArtifactRow>(
        r#"
        SELECT a.id AS artifact_id, a.step_id, a.artifact_sequence,
               a.plaintext_sha256, a.plaintext_length, s.status AS step_status
        FROM agent_execution_artifacts a
        JOIN agent_execution_steps s
          ON s.id = a.step_id
         AND s.tenant_id = a.tenant_id
         AND s.run_id = a.run_id
        WHERE a.tenant_id = $1
          AND a.run_id = $2
          AND a.artifact_kind = 'final_response'
        FOR UPDATE OF s
        "#,
    )
    .bind(tenant_id)
    .bind(run_id)
    .fetch_optional(&mut **transaction)
    .await?)
}

fn capability_resource_json(scope: &CapabilityCallScope) -> Value {
    match scope {
        CapabilityCallScope::TenantWide => Value::Array(Vec::new()),
        CapabilityCallScope::Resources(references) => Value::Array(
            references
                .iter()
                .map(|reference| {
                    let mut value = Map::new();
                    value.insert("kind".to_owned(), json!(reference.kind));
                    value.insert("id".to_owned(), json!(reference.id));
                    Value::Object(value)
                })
                .collect(),
        ),
    }
}

async fn ensure_running_capability_identity(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
    identity: CapabilityCallIdentity,
) -> Result<(), AgentSessionError> {
    let valid = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM agent_capability_calls c
            JOIN agent_execution_steps s
              ON s.tenant_id = c.tenant_id
             AND s.run_id = c.run_id
             AND s.capability_call_id = c.id
            WHERE c.tenant_id = $1
              AND c.run_id = $2
              AND c.id = $3
              AND s.id = $4
              AND s.turn_index = $5
              AND c.call_sequence = $6
              AND c.status = 'running'
              AND s.status = 'running'
        )
        "#,
    )
    .bind(tenant_id)
    .bind(run_id)
    .bind(identity.call_id)
    .bind(identity.step_id)
    .bind(identity.turn_index.get())
    .bind(identity.call_sequence.get())
    .fetch_one(&mut **transaction)
    .await?;
    if valid {
        Ok(())
    } else {
        Err(AgentSessionError::conflict(
            "capability_call_not_running",
            "This capability call is not in the expected running state",
        ))
    }
}

async fn next_step_index(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
) -> Result<i16, AgentSessionError> {
    let current = sqlx::query_scalar::<_, i16>(
        "SELECT COALESCE(MAX(step_index), 0)::SMALLINT FROM agent_execution_steps WHERE tenant_id = $1 AND run_id = $2",
    )
    .bind(tenant_id)
    .bind(run_id)
    .fetch_one(&mut **transaction)
    .await?;
    current
        .checked_add(1)
        .filter(|next| *next <= MAX_EXECUTION_STEPS)
        .ok_or_else(|| {
            AgentSessionError::conflict(
                "execution_step_limit_reached",
                "This Agent run reached its execution-step limit",
            )
        })
}

async fn insert_artifact(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
    step_id: Uuid,
    kind: ExecutionArtifactKind,
    artifact: EncryptedExecutionArtifact,
) -> Result<PersistedExecutionArtifact, AgentSessionError> {
    let sequence = next_artifact_sequence(transaction, tenant_id, run_id).await?;
    let artifact_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO agent_execution_artifacts (
            id, tenant_id, run_id, step_id, artifact_sequence, artifact_kind,
            ciphertext, ciphertext_sha256, plaintext_sha256, nonce,
            encryption_key_id, encryption_key_version, plaintext_length
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        "#,
    )
    .bind(artifact_id)
    .bind(tenant_id)
    .bind(run_id)
    .bind(step_id)
    .bind(sequence)
    .bind(kind.as_str())
    .bind(artifact.ciphertext)
    .bind(artifact.ciphertext_sha256.as_slice())
    .bind(artifact.plaintext_sha256.as_slice())
    .bind(artifact.nonce)
    .bind(artifact.encryption_key_id)
    .bind(artifact.encryption_key_version)
    .bind(artifact.plaintext_length)
    .execute(&mut **transaction)
    .await?;
    Ok(PersistedExecutionArtifact {
        id: artifact_id,
        step_id,
        kind,
        sequence,
    })
}

async fn next_artifact_sequence(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
) -> Result<i16, AgentSessionError> {
    let current = sqlx::query_scalar::<_, i16>(
        "SELECT COUNT(*)::SMALLINT FROM agent_execution_artifacts WHERE tenant_id = $1 AND run_id = $2",
    )
    .bind(tenant_id)
    .bind(run_id)
    .fetch_one(&mut **transaction)
    .await?;
    current
        .checked_add(1)
        .filter(|next| *next <= MAX_EXECUTION_ARTIFACTS)
        .ok_or_else(|| {
            AgentSessionError::conflict(
                "execution_artifact_limit_reached",
                "This Agent run reached its encrypted artifact limit",
            )
        })
}

async fn terminalize_provider_attempt_success(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
    attempt_id: Uuid,
    usage: &NormalizedProviderUsage,
) -> Result<(), AgentSessionError> {
    let provider_cost = usage.provider_reported_cost.as_ref();
    let estimated_cost = usage.estimated_cost.as_ref();
    let updated = sqlx::query(
        r#"
        UPDATE agent_provider_attempts
        SET status = 'succeeded',
            input_tokens = $1,
            output_tokens = $2,
            cached_tokens = $3,
            reasoning_tokens = $4,
            provider_reported_cost_amount = $5,
            provider_reported_cost_currency = $6,
            provider_reported_cost_exponent = $7,
            provider_reported_pricing_version = $8,
            estimated_cost_amount = $9,
            estimated_cost_currency = $10,
            estimated_cost_exponent = $11,
            estimated_pricing_version = $12,
            finished_at = CLOCK_TIMESTAMP(),
            updated_at = CLOCK_TIMESTAMP()
        WHERE tenant_id = $13
          AND run_id = $14
          AND id = $15
          AND status = 'running'
        "#,
    )
    .bind(usage.input_tokens)
    .bind(usage.output_tokens)
    .bind(usage.cached_tokens)
    .bind(usage.reasoning_tokens)
    .bind(provider_cost.map(|cost| cost.amount))
    .bind(provider_cost.map(|cost| cost.currency.as_str()))
    .bind(provider_cost.map(|cost| cost.exponent))
    .bind(provider_cost.and_then(|cost| cost.pricing_version.as_deref()))
    .bind(estimated_cost.map(|cost| cost.amount))
    .bind(estimated_cost.map(|cost| cost.currency.as_str()))
    .bind(estimated_cost.map(|cost| cost.exponent))
    .bind(estimated_cost.and_then(|cost| cost.pricing_version.as_deref()))
    .bind(tenant_id)
    .bind(run_id)
    .bind(attempt_id)
    .execute(&mut **transaction)
    .await?;
    ensure_single_execution_update(updated.rows_affected())
}

async fn terminalize_provider_attempt_failure(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
    attempt_id: Uuid,
    failure: ProviderAttemptFailure,
    usage: &NormalizedProviderUsage,
) -> Result<(), AgentSessionError> {
    let provider_cost = usage.provider_reported_cost.as_ref();
    let estimated_cost = usage.estimated_cost.as_ref();
    let updated = sqlx::query(
        r#"
        UPDATE agent_provider_attempts
        SET status = 'failed',
            failure_origin = $1,
            failure_category = $2,
            input_tokens = $3,
            output_tokens = $4,
            cached_tokens = $5,
            reasoning_tokens = $6,
            provider_reported_cost_amount = $7,
            provider_reported_cost_currency = $8,
            provider_reported_cost_exponent = $9,
            provider_reported_pricing_version = $10,
            estimated_cost_amount = $11,
            estimated_cost_currency = $12,
            estimated_cost_exponent = $13,
            estimated_pricing_version = $14,
            finished_at = CLOCK_TIMESTAMP(),
            updated_at = CLOCK_TIMESTAMP()
        WHERE tenant_id = $15
          AND run_id = $16
          AND id = $17
          AND status = 'running'
        "#,
    )
    .bind(failure.origin())
    .bind(failure.category())
    .bind(usage.input_tokens)
    .bind(usage.output_tokens)
    .bind(usage.cached_tokens)
    .bind(usage.reasoning_tokens)
    .bind(provider_cost.map(|cost| cost.amount))
    .bind(provider_cost.map(|cost| cost.currency.as_str()))
    .bind(provider_cost.map(|cost| cost.exponent))
    .bind(provider_cost.and_then(|cost| cost.pricing_version.as_deref()))
    .bind(estimated_cost.map(|cost| cost.amount))
    .bind(estimated_cost.map(|cost| cost.currency.as_str()))
    .bind(estimated_cost.map(|cost| cost.exponent))
    .bind(estimated_cost.and_then(|cost| cost.pricing_version.as_deref()))
    .bind(tenant_id)
    .bind(run_id)
    .bind(attempt_id)
    .execute(&mut **transaction)
    .await?;
    ensure_single_execution_update(updated.rows_affected())
}

async fn terminalize_step(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
    step_id: Uuid,
    status: &str,
    safe_failure_code: Option<&str>,
) -> Result<(), AgentSessionError> {
    let updated = sqlx::query(
        r#"
        UPDATE agent_execution_steps
        SET status = $1,
            safe_failure_code = $2,
            finished_at = CLOCK_TIMESTAMP(),
            updated_at = CLOCK_TIMESTAMP()
        WHERE tenant_id = $3
          AND run_id = $4
          AND id = $5
          AND status = 'running'
        "#,
    )
    .bind(status)
    .bind(safe_failure_code)
    .bind(tenant_id)
    .bind(run_id)
    .bind(step_id)
    .execute(&mut **transaction)
    .await?;
    ensure_single_execution_update(updated.rows_affected())
}

async fn terminalize_capability_call(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
    call_id: Uuid,
    status: &str,
    safe_failure_code: Option<&str>,
    duration_ms: i64,
) -> Result<(), AgentSessionError> {
    let updated = sqlx::query(
        r#"
        UPDATE agent_capability_calls
        SET status = $1,
            safe_failure_code = $2,
            duration_ms = $3,
            finished_at = CLOCK_TIMESTAMP(),
            updated_at = CLOCK_TIMESTAMP()
        WHERE tenant_id = $4
          AND run_id = $5
          AND id = $6
          AND status = 'running'
        "#,
    )
    .bind(status)
    .bind(safe_failure_code)
    .bind(duration_ms)
    .bind(tenant_id)
    .bind(run_id)
    .bind(call_id)
    .execute(&mut **transaction)
    .await?;
    ensure_single_execution_update(updated.rows_affected())
}

fn usage_is_empty(usage: &NormalizedProviderUsage) -> bool {
    usage.input_tokens.is_none()
        && usage.output_tokens.is_none()
        && usage.cached_tokens.is_none()
        && usage.reasoning_tokens.is_none()
        && usage.provider_reported_cost.is_none()
        && usage.estimated_cost.is_none()
}

fn ensure_single_execution_update(rows_affected: u64) -> Result<(), AgentSessionError> {
    if rows_affected == 1 {
        Ok(())
    } else {
        Err(AgentSessionError::conflict(
            "execution_state_changed",
            "This Agent execution step changed before it could be persisted",
        ))
    }
}

pub(super) enum ExecutionTerminal<'a> {
    Cancelled,
    Interrupted(&'a str),
}

/// Terminalizes every still-running child before the owning run and queue become terminal.
pub(super) async fn terminalize_running_children(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
    terminal: ExecutionTerminal<'_>,
) -> Result<(), AgentSessionError> {
    let (status, safe_failure_code) = match terminal {
        ExecutionTerminal::Cancelled => ("cancelled", None),
        ExecutionTerminal::Interrupted(code) => ("interrupted", Some(code)),
    };
    sqlx::query(
        r#"
        UPDATE agent_provider_attempts
        SET status = $1,
            finished_at = CLOCK_TIMESTAMP(),
            updated_at = CLOCK_TIMESTAMP()
        WHERE tenant_id = $2 AND run_id = $3 AND status = 'running'
        "#,
    )
    .bind(status)
    .bind(tenant_id)
    .bind(run_id)
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        r#"
        UPDATE agent_capability_calls
        SET status = $1,
            safe_failure_code = $2,
            duration_ms = LEAST(
                9007199254740991,
                GREATEST(
                    0,
                    FLOOR(EXTRACT(EPOCH FROM (CLOCK_TIMESTAMP() - started_at)) * 1000)::BIGINT
                )
            ),
            finished_at = CLOCK_TIMESTAMP(),
            updated_at = CLOCK_TIMESTAMP()
        WHERE tenant_id = $3 AND run_id = $4 AND status = 'running'
        "#,
    )
    .bind(status)
    .bind(safe_failure_code)
    .bind(tenant_id)
    .bind(run_id)
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        r#"
        UPDATE agent_execution_steps
        SET status = $1,
            safe_failure_code = $2,
            finished_at = CLOCK_TIMESTAMP(),
            updated_at = CLOCK_TIMESTAMP()
        WHERE tenant_id = $3 AND run_id = $4 AND status = 'running'
        "#,
    )
    .bind(status)
    .bind(safe_failure_code)
    .bind(tenant_id)
    .bind(run_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn advance_queue_checkpoint(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    lease: &RunLease,
    checkpoint: RunCheckpoint,
    reject_cancellation: bool,
) -> Result<i64, AgentSessionError> {
    let version = sqlx::query_scalar::<_, i64>(
        r#"
        UPDATE agent_run_queue
        SET checkpoint = $1,
            version = version + 1,
            updated_at = CLOCK_TIMESTAMP()
        WHERE tenant_id = $2
          AND run_id = $3
          AND state = 'leased'
          AND leased_by = $4
          AND lease_token = $5
          AND version = $6
          AND lease_expires_at > STATEMENT_TIMESTAMP()
          AND ($7::BOOLEAN = FALSE OR cancel_requested_at IS NULL)
          AND deleted_at IS NULL
        RETURNING version
        "#,
    )
    .bind(checkpoint.as_str())
    .bind(tenant_id)
    .bind(lease.run_id)
    .bind(&lease.worker_id)
    .bind(lease.lease_token)
    .bind(lease.fence_version)
    .bind(reject_cancellation)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(AgentSessionError::LeaseLost)?;
    Ok(version)
}

async fn append_execution_event(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
    event_type: RunEventType,
) -> Result<(), AgentSessionError> {
    sqlx::query(
        "INSERT INTO agent_run_events (tenant_id, run_id, event_type, payload) VALUES ($1, $2, $3, '{}'::JSONB)",
    )
    .bind(tenant_id)
    .bind(run_id)
    .bind(event_type.as_str())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn next_lease(lease: &RunLease, fence_version: i64) -> RunLease {
    RunLease {
        run_id: lease.run_id,
        worker_id: lease.worker_id.clone(),
        lease_token: lease.lease_token,
        fence_version,
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;
    use sha2::{Digest, Sha256};

    use super::*;
    use crate::{
        AgentProviderKey, CapabilityFailureStatus, ProviderAttemptIndex, ProviderPreflightFailure,
    };

    #[test]
    fn send_time_preflight_is_valid_after_dispatch_claim_without_becoming_upstream() {
        let preflight = ProviderAttemptFailure::Preflight(
            ProviderPreflightFailure::ProviderDataApprovalChanged,
        );
        assert!(provider_failure_checkpoint_matches(
            RunCheckpoint::BeforeProvider,
            preflight
        ));
        assert!(provider_failure_checkpoint_matches(
            RunCheckpoint::ProviderInFlight,
            preflight
        ));
        assert!(!provider_failure_checkpoint_matches(
            RunCheckpoint::ProviderResultPersisted,
            preflight
        ));
        assert!(!provider_failure_checkpoint_matches(
            RunCheckpoint::BeforeProvider,
            ProviderAttemptFailure::Upstream(ProviderUpstreamFailure::Unavailable)
        ));
    }

    fn snapshot_row(kind: &str, status: &str) -> ExecutionSnapshotRow {
        ExecutionSnapshotRow {
            step_id: Uuid::new_v4(),
            step_index: 1,
            turn_index: 1,
            step_kind: kind.to_owned(),
            step_input_fingerprint: vec![1; 32],
            step_status: status.to_owned(),
            step_safe_failure_code: None,
            provider_attempt_id: None,
            provider_attempt_index: None,
            route_set_id: None,
            route_version: None,
            route_target_id: None,
            connection_id: None,
            credential_version: None,
            model_snapshot_id: None,
            provider_data_approval_id: None,
            required_provider_data_class: None,
            execution_environment_class: None,
            provider_key: None,
            provider_model_id: None,
            task_class: None,
            provider_status: None,
            provider_failure_origin: None,
            provider_failure_category: None,
            capability_call_id: None,
            capability_call_sequence: None,
            capability_key: None,
            capability_version: None,
            product_operation_key: None,
            owning_module_key: None,
            required_permission: None,
            capability_scope_kind: None,
            capability_resource_references: None,
            capability_status: None,
            capability_safe_failure_code: None,
            capability_duration_ms: None,
            artifact_id: None,
            artifact_sequence: None,
            artifact_kind: None,
            ciphertext: None,
            ciphertext_sha256: None,
            plaintext_sha256: None,
            nonce: None,
            encryption_key_id: None,
            encryption_key_version: None,
            plaintext_length: None,
        }
    }

    fn add_artifact(row: &mut ExecutionSnapshotRow, kind: &str) {
        let ciphertext = vec![7; 32];
        row.artifact_id = Some(Uuid::new_v4());
        row.artifact_sequence = Some(1);
        row.artifact_kind = Some(kind.to_owned());
        row.ciphertext_sha256 = Some(Sha256::digest(&ciphertext).to_vec());
        row.ciphertext = Some(ciphertext);
        row.plaintext_sha256 = Some(vec![8; 32]);
        row.nonce = Some(vec![9; 12]);
        row.encryption_key_id = Some("unit-test-key".to_owned());
        row.encryption_key_version = Some(1);
        row.plaintext_length = Some(16);
    }

    fn loaded_artifact(kind: ExecutionArtifactKind) -> LoadedExecutionArtifact {
        let ciphertext = vec![7; 32];
        LoadedExecutionArtifact::from_stored(
            Uuid::new_v4(),
            Uuid::new_v4(),
            kind,
            1,
            ciphertext.clone(),
            Sha256::digest(&ciphertext).into(),
            [8; 32],
            vec![9; 12],
            "unit-test-key".to_owned(),
            1,
            16,
        )
        .unwrap()
    }

    fn step_evidence(
        status: ExecutionStepStatus,
        artifact: Option<LoadedExecutionArtifact>,
    ) -> ExecutionStepEvidence {
        ExecutionStepEvidence {
            step_id: Uuid::new_v4(),
            step_index: 1,
            turn_index: ProviderTurnIndex::parse(1).unwrap(),
            input_fingerprint: [1; 32],
            status,
            safe_failure_code: None,
            artifact,
        }
    }

    fn provider_row(status: &str) -> ExecutionSnapshotRow {
        let mut row = snapshot_row("provider_attempt", status);
        row.provider_attempt_id = Some(Uuid::new_v4());
        row.provider_attempt_index = Some(1);
        row.route_set_id = Some(Uuid::new_v4());
        row.route_version = Some(1);
        row.route_target_id = Some(Uuid::new_v4());
        row.connection_id = Some(Uuid::new_v4());
        row.credential_version = Some(1);
        row.model_snapshot_id = Some(Uuid::new_v4());
        row.provider_data_approval_id = Some(Uuid::new_v4());
        row.required_provider_data_class = Some("sensitive_data_approved".to_owned());
        row.execution_environment_class = Some("external_managed".to_owned());
        row.provider_key = Some("openai".to_owned());
        row.provider_model_id = Some("gpt-test".to_owned());
        row.task_class = Some("module_read_reporting".to_owned());
        row.provider_status = Some(status.to_owned());
        row
    }

    fn capability_row(status: &str) -> ExecutionSnapshotRow {
        let mut row = snapshot_row(
            "capability_call",
            if status == "denied" { "failed" } else { status },
        );
        row.capability_call_id = Some(Uuid::new_v4());
        row.capability_call_sequence = Some(1);
        row.capability_key = Some("sis.learners.read".to_owned());
        row.capability_version = Some(1);
        row.product_operation_key = Some("sis.learners.read".to_owned());
        row.owning_module_key = Some("sis".to_owned());
        row.required_permission = Some("sis:view".to_owned());
        row.capability_scope_kind = Some("tenant_wide".to_owned());
        row.capability_resource_references = Some(json!([]));
        row.capability_status = Some(status.to_owned());
        row.capability_duration_ms = Some(10);
        row
    }

    #[test]
    fn private_snapshot_projection_fails_closed_on_malformed_storage() {
        assert!(matches!(
            ExecutionStepSnapshot::try_from(snapshot_row("finalize", "running")).unwrap(),
            ExecutionStepSnapshot::Finalization(_)
        ));
        assert!(ExecutionStepSnapshot::try_from(snapshot_row("finalize", "succeeded")).is_err());
        assert!(ExecutionStepSnapshot::try_from(snapshot_row("unknown", "running")).is_err());
        assert!(ExecutionStepSnapshot::try_from(snapshot_row("finalize", "unknown")).is_err());

        let mut invalid_turn = snapshot_row("finalize", "running");
        invalid_turn.turn_index = 0;
        assert!(ExecutionStepSnapshot::try_from(invalid_turn).is_err());
        let mut invalid_fingerprint = snapshot_row("finalize", "running");
        invalid_fingerprint.step_input_fingerprint.pop();
        assert!(ExecutionStepSnapshot::try_from(invalid_fingerprint).is_err());

        let mut succeeded_final = snapshot_row("finalize", "succeeded");
        add_artifact(&mut succeeded_final, "final_response");
        assert!(matches!(
            ExecutionStepSnapshot::try_from(succeeded_final).unwrap(),
            ExecutionStepSnapshot::Finalization(_)
        ));
        let wrong_final = step_evidence(
            ExecutionStepStatus::Succeeded,
            Some(loaded_artifact(ExecutionArtifactKind::ProviderResult)),
        );
        assert!(ensure_finalization_snapshot_shape(&wrong_final).is_err());

        for missing_column in 0..10 {
            let mut partial = snapshot_row("finalize", "succeeded");
            add_artifact(&mut partial, "final_response");
            match missing_column {
                0 => partial.artifact_id = None,
                1 => partial.artifact_sequence = None,
                2 => partial.artifact_kind = None,
                3 => partial.ciphertext = None,
                4 => partial.ciphertext_sha256 = None,
                5 => partial.plaintext_sha256 = None,
                6 => partial.nonce = None,
                7 => partial.encryption_key_id = None,
                8 => partial.encryption_key_version = None,
                _ => partial.plaintext_length = None,
            }
            assert!(take_snapshot_artifact(&mut partial).is_err());
        }

        let invalid_hash = StoredArtifactRow {
            id: Uuid::new_v4(),
            step_id: Uuid::new_v4(),
            artifact_sequence: 1,
            artifact_kind: "provider_result".to_owned(),
            ciphertext: vec![1; 16],
            ciphertext_sha256: vec![0; 32],
            plaintext_sha256: vec![2; 32],
            nonce: vec![3; 12],
            encryption_key_id: "key".to_owned(),
            encryption_key_version: 1,
            plaintext_length: 8,
        };
        assert!(LoadedExecutionArtifact::try_from(invalid_hash).is_err());
        let invalid_hash_size = StoredArtifactRow {
            id: Uuid::new_v4(),
            step_id: Uuid::new_v4(),
            artifact_sequence: 1,
            artifact_kind: "provider_result".to_owned(),
            ciphertext: vec![1; 16],
            ciphertext_sha256: vec![0; 31],
            plaintext_sha256: vec![2; 32],
            nonce: vec![3; 12],
            encryption_key_id: "key".to_owned(),
            encryption_key_version: 1,
            plaintext_length: 8,
        };
        assert!(LoadedExecutionArtifact::try_from(invalid_hash_size).is_err());
    }

    #[test]
    fn reduced_provider_and_capability_outcomes_are_typed_and_bounded() {
        let mut failed_provider = provider_row("failed");
        failed_provider.provider_failure_origin = Some("upstream".to_owned());
        failed_provider.provider_failure_category = Some("timeout".to_owned());
        assert!(matches!(
            ExecutionStepSnapshot::try_from(failed_provider).unwrap(),
            ExecutionStepSnapshot::ProviderAttempt(step)
                if step.status == ProviderAttemptStatus::Failed
        ));
        let mut missing_provider_status = provider_row("running");
        missing_provider_status.provider_status = None;
        assert!(ExecutionStepSnapshot::try_from(missing_provider_status).is_err());
        let mut partial_failure = provider_row("failed");
        partial_failure.provider_failure_origin = Some("upstream".to_owned());
        assert!(ExecutionStepSnapshot::try_from(partial_failure).is_err());
        let mut invalid_attempt = provider_row("running");
        invalid_attempt.provider_attempt_index = Some(0);
        assert!(ExecutionStepSnapshot::try_from(invalid_attempt).is_err());
        let mut invalid_provider_key = provider_row("running");
        invalid_provider_key.provider_key = Some("raw".to_owned());
        assert!(ExecutionStepSnapshot::try_from(invalid_provider_key).is_err());
        let mut invalid_task = provider_row("running");
        invalid_task.task_class = Some("raw".to_owned());
        assert!(ExecutionStepSnapshot::try_from(invalid_task).is_err());

        for (status, step_status) in [
            ("succeeded", "succeeded"),
            ("failed", "failed"),
            ("denied", "failed"),
        ] {
            let mut capability = capability_row(status);
            capability.step_status = step_status.to_owned();
            add_artifact(&mut capability, "capability_result");
            assert!(matches!(
                ExecutionStepSnapshot::try_from(capability).unwrap(),
                ExecutionStepSnapshot::CapabilityCall(_)
            ));
        }
        let mut missing_capability_status = capability_row("running");
        missing_capability_status.capability_status = None;
        assert!(ExecutionStepSnapshot::try_from(missing_capability_status).is_err());
        let mut negative_duration = capability_row("running");
        negative_duration.capability_duration_ms = Some(-1);
        assert!(ExecutionStepSnapshot::try_from(negative_duration).is_err());
        let mut missing_call = capability_row("running");
        missing_call.capability_call_id = None;
        assert!(ExecutionStepSnapshot::try_from(missing_call).is_err());
        let mut missing_key = capability_row("running");
        missing_key.capability_key = None;
        assert!(ExecutionStepSnapshot::try_from(missing_key).is_err());

        let resource_scope = capability_scope_from_stored(
            "resources",
            json!([{"kind": "learner", "id": "learner-1"}]),
        )
        .unwrap();
        assert!(matches!(resource_scope, CapabilityCallScope::Resources(_)));
        assert!(capability_scope_from_stored("resources", json!([{"kind": "learner"}])).is_err());
        assert!(capability_scope_from_stored("tenant_wide", json!([1])).is_err());
        assert!(capability_scope_from_stored("resources", json!({})).is_err());
        assert!(capability_scope_from_stored("raw", json!([])).is_err());
        assert!(matches!(
            capability_resource_json(&resource_scope),
            Value::Array(values) if values.len() == 1
        ));
    }

    #[test]
    fn pure_execution_guards_reject_stale_or_mismatched_facts() {
        let running = ExecutionRunRow {
            task_class: "module_read_reporting".to_owned(),
            status: "running".to_owned(),
        };
        assert!(require_running_run(&running).is_ok());
        assert!(
            require_running_run(&ExecutionRunRow {
                task_class: running.task_class.clone(),
                status: "failed".to_owned(),
            })
            .is_err()
        );
        assert!(
            reject_cancelled_queue(&ExecutionQueueRow {
                checkpoint: "queued".to_owned(),
                cancel_requested_at: None,
            })
            .is_ok()
        );
        assert!(
            reject_cancelled_queue(&ExecutionQueueRow {
                checkpoint: "queued".to_owned(),
                cancel_requested_at: Some(Utc::now()),
            })
            .is_err()
        );
        assert!(usage_is_empty(&NormalizedProviderUsage::unknown()));
        assert!(!usage_is_empty(
            &NormalizedProviderUsage::parse(Some(1), None, None, None, None, None).unwrap()
        ));
        assert!(ensure_single_execution_update(1).is_ok());
        assert!(ensure_single_execution_update(0).is_err());

        let lease = RunLease::parse(Uuid::new_v4(), "worker", Uuid::new_v4(), 1).unwrap();
        assert_eq!(next_lease(&lease, 2).fence_version, 2);

        let plan = ProviderAttemptPlan::parse(
            1,
            1,
            Uuid::new_v4(),
            1,
            Uuid::new_v4(),
            Uuid::new_v4(),
            1,
            Uuid::new_v4(),
            Uuid::new_v4(),
            cp_common::ProviderDataClass::SensitiveDataApproved,
            cp_common::ProviderExecutionEnvironmentClass::ExternalManaged,
            AgentProviderKey::OpenAi,
            "gpt-test",
            [4; 32],
        )
        .unwrap();
        let mut existing_provider = ExistingProviderAttemptRow {
            attempt_id: Uuid::new_v4(),
            step_id: Uuid::new_v4(),
            turn_index: 1,
            attempt_index: 1,
            route_set_id: plan.route_set_id,
            route_version: plan.route_version,
            route_target_id: plan.route_target_id,
            connection_id: plan.connection_id,
            credential_version: plan.credential_version,
            model_snapshot_id: plan.model_snapshot_id,
            provider_data_approval_id: plan.provider_data_approval_id,
            required_provider_data_class: plan.required_provider_data_class.as_str().to_owned(),
            execution_environment_class: plan.execution_environment_class.as_str().to_owned(),
            provider_key: "openai".to_owned(),
            provider_model_id: plan.provider_model_id.clone(),
            task_class: "module_read_reporting".to_owned(),
            attempt_status: "running".to_owned(),
            step_status: "running".to_owned(),
            input_fingerprint: vec![4; 32],
        };
        assert!(
            ensure_matching_provider_attempt(&existing_provider, &plan, "module_read_reporting")
                .is_ok()
        );
        assert_eq!(
            provider_identity(&existing_provider).unwrap().attempt_index,
            ProviderAttemptIndex::parse(1).unwrap()
        );
        existing_provider.step_status = "failed".to_owned();
        assert!(
            ensure_matching_provider_attempt(&existing_provider, &plan, "module_read_reporting")
                .is_err()
        );

        for (category, allowed) in [
            ("rate_limited", true),
            ("unavailable", true),
            ("timeout", false),
            ("network", false),
            ("authentication", false),
            ("invalid_response", false),
            ("unsupported", false),
        ] {
            assert_eq!(
                provider_attempt_allows_fallback(&LastProviderAttemptRow {
                    turn_index: 1,
                    attempt_index: 1,
                    status: "failed".to_owned(),
                    failure_origin: Some("upstream".to_owned()),
                    failure_category: Some(category.to_owned()),
                })
                .unwrap(),
                allowed
            );
        }
        assert!(
            !provider_attempt_allows_fallback(&LastProviderAttemptRow {
                turn_index: 1,
                attempt_index: 1,
                status: "succeeded".to_owned(),
                failure_origin: None,
                failure_category: None,
            })
            .unwrap()
        );
        assert!(
            provider_attempt_allows_fallback(&LastProviderAttemptRow {
                turn_index: 1,
                attempt_index: 1,
                status: "failed".to_owned(),
                failure_origin: Some("upstream".to_owned()),
                failure_category: None,
            })
            .is_err()
        );

        let capability_plan = CapabilityCallPlan::parse(
            Uuid::new_v4(),
            1,
            1,
            "sis.learners.read",
            1,
            "sis.learners.read",
            "sis",
            "sis:view",
            [5; 32],
            CapabilityCallScope::TenantWide,
        )
        .unwrap();
        let mut existing_capability = ExistingCapabilityCallRow {
            call_id: capability_plan.call_id,
            step_id: Uuid::new_v4(),
            turn_index: 1,
            call_sequence: 1,
            capability_key: capability_plan.capability_key.clone(),
            capability_version: 1,
            product_operation_key: capability_plan.product_operation_key.clone(),
            owning_module_key: capability_plan.owning_module_key.clone(),
            required_permission: capability_plan.required_permission.clone(),
            input_fingerprint: vec![5; 32],
            scope_kind: "tenant_wide".to_owned(),
            resource_references: json!([]),
            call_status: "running".to_owned(),
            step_status: "running".to_owned(),
        };
        assert!(ensure_matching_capability_call(&existing_capability, &capability_plan).is_ok());
        assert_eq!(
            capability_identity(&existing_capability).unwrap().call_id,
            capability_plan.call_id
        );
        existing_capability.required_permission = "sis:edit".to_owned();
        assert!(ensure_matching_capability_call(&existing_capability, &capability_plan).is_err());
        assert_eq!(CapabilityFailureStatus::Denied.as_str(), "denied");
    }
}
