//! Persists broker-preparation rejection evidence through the sole fenced SQL function.
//!
//! Raw worker lease tokens are bound in PostgreSQL but never retained in the
//! append-only rejection ledger. Unknown operation and scope facts remain NULL.

use cp_agent::{CapabilityPreparationRejection, CapabilityScope};
use serde_json::json;
use sqlx::error::DatabaseError;
use uuid::Uuid;

use super::{
    ops::AgentSessionOps,
    types::{AgentSessionError, CapabilityCallSequence, RunLease},
};

impl AgentSessionOps {
    /// Records a terminal non-executable call intent under the exact current run lease.
    pub async fn record_capability_rejection(
        &self,
        tenant_id: Uuid,
        lease: &RunLease,
        call_sequence: CapabilityCallSequence,
        rejection: &CapabilityPreparationRejection,
    ) -> Result<Uuid, AgentSessionError> {
        let call_id = rejection.capability_call_id().as_uuid();
        let principal = rejection.principal();
        let request_context = rejection.request_context();
        if principal.tenant_id() != tenant_id
            || rejection.agent_run_id() != Some(lease.run_id)
            || request_context.request_id() != call_id
        {
            return Err(AgentSessionError::conflict(
                "capability_rejection_identity_conflict",
                "The rejected capability intent does not match this Agent run",
            ));
        }

        let operation = rejection.operation_evidence();
        let (operation_key, module_key, required_permission) =
            operation.map_or((None, None, None), |evidence| {
                (
                    Some(evidence.operation_key()),
                    Some(evidence.module_key()),
                    Some(evidence.required_permission()),
                )
            });
        let (scope_kind, resource_references) = match rejection.scope_evidence() {
            None => (None, None),
            Some(CapabilityScope::TenantWide) => (Some("tenant_wide"), Some(json!([]))),
            Some(CapabilityScope::Resources(resources)) => (
                Some("resources"),
                Some(json!(
                    resources
                        .values()
                        .iter()
                        .map(|resource| json!({
                            "kind": resource.kind(),
                            "id": resource.id(),
                        }))
                        .collect::<Vec<_>>()
                )),
            ),
        };
        let digest = rejection
            .normalized_input_digest_sha256()
            .map(|value| value.to_vec());

        sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT record_agent_capability_rejection(
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22
            )
            "#,
        )
        .bind(call_id)
        .bind(tenant_id)
        .bind(lease.run_id)
        .bind(call_sequence.get())
        .bind(principal.user_id())
        .bind(request_context.request_id())
        .bind(request_context.correlation_id())
        .bind(rejection.key().as_str())
        .bind(i32::from(rejection.version().get()))
        .bind(digest)
        .bind(operation_key)
        .bind(module_key)
        .bind(required_permission)
        .bind(scope_kind)
        .bind(resource_references)
        .bind(rejection.outcome().as_str())
        .bind(rejection.code().as_str())
        .bind(rejection.reason_code())
        .bind(rejection.safe_message())
        .bind(&lease.worker_id)
        .bind(lease.lease_token)
        .bind(lease.fence_version)
        .fetch_one(&self.pool)
        .await
        .map_err(map_rejection_persistence_error)
    }
}

fn map_rejection_persistence_error(error: sqlx::Error) -> AgentSessionError {
    let message = error
        .as_database_error()
        .map(DatabaseError::message)
        .unwrap_or_default();
    if message.contains("exact current run lease") {
        AgentSessionError::LeaseLost
    } else if message.contains("idempotency conflict") {
        AgentSessionError::conflict(
            "capability_rejection_idempotency_conflict",
            "This capability rejection was already recorded with different evidence",
        )
    } else {
        AgentSessionError::Storage(error)
    }
}
