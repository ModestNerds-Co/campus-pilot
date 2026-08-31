//! Supervises tenant-fair Agent queue claims and usage cleanup.
//!
//! The supervisor owns no model or capability logic. A production executor
//! must perform the fenced provider/broker workflow and terminalize the run.
//! This layer prepares and reconciles the run counter, prevents dispatch when
//! tenant governance is not ready, and repairs usage after lease recovery.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use cp_agent_runtime::{
    AgentSessionError, AgentSessionOps, AgentUsageDemand, AgentUsageError, AgentUsageMeter,
    AgentUsageRuntime, AgentUsageStage, AgentUsageTerminalAction, ClaimRunsCommand, ClaimedRun,
    PrepareAgentUsage, RecoveryUsageAction, SafeRunFailure,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::worker_readiness::{
    AgentWorkerReadinessError, AgentWorkerReadinessOps, AgentWorkerReadinessReason,
};

const RUN_USAGE_TTL: Duration = Duration::from_secs(15 * 60);

/// Error from one executor invocation before it could terminalize the run.
///
/// The exact latest lease is retained so the supervisor can fail the run only
/// while it still owns the current fence. No provider detail is accepted.
#[derive(Debug)]
pub struct AgentExecutionFailure {
    pub lease: cp_agent_runtime::RunLease,
    pub code: &'static str,
    pub message: &'static str,
    pub(crate) disposition: AgentExecutionFailureDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentExecutionFailureDisposition {
    FailRun,
    CancellationAcknowledged,
    CancellationPending,
}

#[async_trait]
pub trait AgentRunExecutor: Send + Sync {
    /// Executes and terminalizes one claimed run, including heartbeats,
    /// provider/capability usage, artifacts, and cancellation acknowledgement.
    async fn execute(&self, run: ClaimedRun) -> Result<(), AgentExecutionFailure>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentWorkerTick {
    pub recovered: u64,
    pub claimed: usize,
    pub completed: usize,
    pub failed: usize,
}

#[derive(Debug, Error)]
pub enum AgentWorkerSupervisorError {
    #[error("Agent queue operation failed")]
    Session(#[source] AgentSessionError),
    #[error("Agent usage operation failed: {0}")]
    Usage(#[source] AgentUsageError),
    #[error("Agent worker readiness failed")]
    Readiness(#[source] AgentWorkerReadinessError),
}

#[derive(Clone)]
pub struct AgentWorkerSupervisor {
    sessions: AgentSessionOps,
    usage: AgentUsageRuntime,
    readiness: AgentWorkerReadinessOps,
    executor: Arc<dyn AgentRunExecutor>,
    claim: ClaimRunsCommand,
}

impl AgentWorkerSupervisor {
    pub fn new(
        sessions: AgentSessionOps,
        usage: AgentUsageRuntime,
        readiness: AgentWorkerReadinessOps,
        executor: Arc<dyn AgentRunExecutor>,
        worker_id: &str,
        batch_size: u16,
    ) -> Result<Self, AgentSessionError> {
        Ok(Self {
            sessions,
            usage,
            readiness,
            executor,
            claim: ClaimRunsCommand::parse(worker_id, batch_size)?,
        })
    }

    /// Recovers expired leases, reconciles their usage, then claims one fair batch.
    pub async fn run_once(&self) -> Result<AgentWorkerTick, AgentWorkerSupervisorError> {
        let recovery = self
            .sessions
            .recover_expired_runs_globally(100)
            .await
            .map_err(AgentWorkerSupervisorError::Session)?;
        for pending in recovery.pending_usage_reservations {
            match pending.action {
                RecoveryUsageAction::ExpireUnclaimed => {
                    self.usage
                        .release_or_expire(
                            pending.tenant_id,
                            pending.reservation_id,
                            AgentUsageTerminalAction::Expire,
                        )
                        .await
                        .map_err(AgentWorkerSupervisorError::Usage)?;
                }
                RecoveryUsageAction::CommitTerminal => {
                    self.usage
                        .commit_terminal_usage(pending.tenant_id, pending.reservation_id)
                        .await
                        .map_err(AgentWorkerSupervisorError::Usage)?;
                }
            }
        }

        let claimed = self
            .sessions
            .claim_runs_globally(self.claim.clone())
            .await
            .map_err(AgentWorkerSupervisorError::Session)?;
        let claimed_count = claimed.len();
        let mut completed = 0_usize;
        let mut failed = 0_usize;
        for run in claimed {
            if self.process(run).await? {
                completed += 1;
            } else {
                failed += 1;
            }
        }
        Ok(AgentWorkerTick {
            recovered: recovery.summary.requeued
                + recovery.summary.interrupted
                + recovery.summary.cancelled,
            claimed: claimed_count,
            completed,
            failed,
        })
    }

    async fn process(&self, run: ClaimedRun) -> Result<bool, AgentWorkerSupervisorError> {
        let run_usage = PrepareAgentUsage::parse(
            run.lease.run_id,
            AgentUsageStage::Run,
            &format!("agent:run:{}", run.lease.run_id),
            run_usage_fingerprint(run.tenant_id, run.lease.run_id),
            [AgentUsageDemand::count(AgentUsageMeter::Runs, 1)
                .map_err(AgentWorkerSupervisorError::Usage)?],
            RUN_USAGE_TTL,
        )
        .map_err(AgentWorkerSupervisorError::Usage)?;
        let reservation = match self
            .usage
            .prepare(run.tenant_id, run.requested_by, run_usage)
            .await
        {
            Ok(reservation) => reservation,
            Err(error @ AgentUsageError::Denied { reservation_id }) => {
                let failure = usage_preparation_failure(&error);
                self.sessions
                    .fail_run(run.tenant_id, &run.lease, failure)
                    .await
                    .map_err(AgentWorkerSupervisorError::Session)?;
                commit_denied_usage(&self.usage, run.tenant_id, reservation_id).await?;
                return Ok(false);
            }
            Err(error) => {
                let failure = usage_preparation_failure(&error);
                self.sessions
                    .fail_run(run.tenant_id, &run.lease, failure)
                    .await
                    .map_err(AgentWorkerSupervisorError::Session)?;
                return Ok(false);
            }
        };

        let readiness = self
            .readiness
            .tenant_readiness(run.tenant_id)
            .await
            .map_err(AgentWorkerSupervisorError::Readiness)?;
        if !readiness.ready {
            let failure = readiness_failure(readiness.reason)?;
            self.sessions
                .fail_run(run.tenant_id, &run.lease, failure)
                .await
                .map_err(AgentWorkerSupervisorError::Session)?;
            self.usage
                .commit_terminal_usage(run.tenant_id, reservation.reservation_id)
                .await
                .map_err(AgentWorkerSupervisorError::Usage)?;
            return Ok(false);
        }

        let tenant_id = run.tenant_id;
        let outcome = self.executor.execute(run).await;
        let succeeded = match outcome {
            Ok(()) => true,
            Err(failure)
                if failure.disposition == AgentExecutionFailureDisposition::CancellationPending =>
            {
                return Ok(false);
            }
            Err(failure)
                if failure.disposition
                    == AgentExecutionFailureDisposition::CancellationAcknowledged =>
            {
                false
            }
            Err(failure) => {
                let safe = SafeRunFailure::parse(failure.code, failure.message)
                    .map_err(AgentWorkerSupervisorError::Session)?;
                self.sessions
                    .fail_run(tenant_id, &failure.lease, safe)
                    .await
                    .map_err(AgentWorkerSupervisorError::Session)?;
                false
            }
        };
        self.usage
            .commit_terminal_usage(tenant_id, reservation.reservation_id)
            .await
            .map_err(AgentWorkerSupervisorError::Usage)?;
        Ok(succeeded)
    }
}

async fn commit_denied_usage(
    usage: &AgentUsageRuntime,
    tenant_id: uuid::Uuid,
    reservation_id: uuid::Uuid,
) -> Result<(), AgentWorkerSupervisorError> {
    match usage.commit_terminal_usage(tenant_id, reservation_id).await {
        Err(AgentUsageError::Denied {
            reservation_id: stored,
        }) if stored == reservation_id => Ok(()),
        Ok(_) => Err(AgentWorkerSupervisorError::Usage(
            AgentUsageError::InvalidTransition,
        )),
        Err(error) => Err(AgentWorkerSupervisorError::Usage(error)),
    }
}

fn run_usage_fingerprint(tenant_id: uuid::Uuid, run_id: uuid::Uuid) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"campus-pilot/agent-run-usage/v1");
    digest.update(tenant_id.as_bytes());
    digest.update(run_id.as_bytes());
    digest.finalize().into()
}

fn usage_preparation_failure(error: &AgentUsageError) -> SafeRunFailure {
    let (code, message) = usage_failure_facts(error);
    SafeRunFailure::parse(code, message).unwrap_or_else(|_| unreachable!())
}

fn usage_failure_facts(error: &AgentUsageError) -> (&'static str, &'static str) {
    match error {
        AgentUsageError::Denied { .. } => (
            "agent_usage_limit_denied",
            "This Agent run exceeds a configured usage limit",
        ),
        AgentUsageError::MissingDemand => (
            "agent_usage_limit_not_ready",
            "A configured Agent usage limit cannot be enforced for this run",
        ),
        AgentUsageError::CurrencyMismatch => (
            "agent_usage_currency_not_ready",
            "A configured Agent usage currency does not match this run",
        ),
        AgentUsageError::Invalid { .. }
        | AgentUsageError::IdempotencyConflict
        | AgentUsageError::NotFound
        | AgentUsageError::InvalidTransition
        | AgentUsageError::Storage => (
            "agent_usage_unavailable",
            "Agent usage checks are temporarily unavailable",
        ),
    }
}

fn readiness_failure(
    reason: AgentWorkerReadinessReason,
) -> Result<SafeRunFailure, AgentWorkerSupervisorError> {
    let (code, message) = match reason {
        AgentWorkerReadinessReason::EstimatedCostHardLimitRequiresPricing => (
            "agent_estimated_cost_limit_not_ready",
            "Estimated-cost limits require versioned provider pricing",
        ),
        AgentWorkerReadinessReason::Ready => {
            return Err(AgentWorkerSupervisorError::Usage(
                AgentUsageError::InvalidTransition,
            ));
        }
    };
    SafeRunFailure::parse(code, message).map_err(AgentWorkerSupervisorError::Session)
}

#[cfg(test)]
mod tests {
    use cp_agent_runtime::AgentUsageError;
    use uuid::Uuid;

    use super::{
        AgentWorkerReadinessReason, readiness_failure, run_usage_fingerprint, usage_failure_facts,
    };

    #[test]
    fn run_fingerprint_is_bound_to_tenant_and_run() {
        let tenant = Uuid::new_v4();
        let run = Uuid::new_v4();
        assert_eq!(
            run_usage_fingerprint(tenant, run),
            run_usage_fingerprint(tenant, run)
        );
        assert_ne!(
            run_usage_fingerprint(tenant, run),
            run_usage_fingerprint(Uuid::new_v4(), run)
        );
    }

    #[test]
    fn hard_limit_and_pricing_failures_are_safe_and_stable() {
        let denied = usage_failure_facts(&AgentUsageError::Denied {
            reservation_id: Uuid::new_v4(),
        });
        assert_eq!(denied.0, "agent_usage_limit_denied");
        assert!(
            readiness_failure(AgentWorkerReadinessReason::EstimatedCostHardLimitRequiresPricing)
                .is_ok()
        );
        assert!(readiness_failure(AgentWorkerReadinessReason::Ready).is_err());
    }
}
