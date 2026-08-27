//! Records one correlated actor-aware event for every broker decision.
//!
//! Audit metadata contains only stable identifiers and outcome codes. Capability
//! inputs and outputs are deliberately excluded from this generic boundary.

use async_trait::async_trait;
use cp_audit::{AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append};
use serde_json::{Map, Value};
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use crate::types::{AuthenticatedAgentPrincipal, CapabilityResource};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerAuditOutcome {
    Succeeded,
    Failed,
    Denied,
}

#[derive(Debug, Clone)]
pub struct BrokerAuditRecord {
    pub principal: AuthenticatedAgentPrincipal,
    pub request_context: RequestContext,
    pub capability_call_id: Uuid,
    pub action_key: String,
    pub capability_version: u16,
    pub agent_run_id: Option<Uuid>,
    pub target: Option<CapabilityResource>,
    pub outcome: BrokerAuditOutcome,
    pub reason: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("broker audit evidence could not be persisted")]
pub struct BrokerAuditError;

#[async_trait]
pub trait BrokerAuditSink: Send + Sync {
    async fn record(&self, record: BrokerAuditRecord) -> Result<(), BrokerAuditError>;
}

#[derive(Clone)]
pub struct PostgresBrokerAuditSink {
    pool: PgPool,
}

impl PostgresBrokerAuditSink {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl BrokerAuditSink for PostgresBrokerAuditSink {
    async fn record(&self, record: BrokerAuditRecord) -> Result<(), BrokerAuditError> {
        let mut metadata = Map::new();
        metadata.insert(
            "capabilityCallId".to_string(),
            Value::String(record.capability_call_id.to_string()),
        );
        metadata.insert(
            "capabilityVersion".to_string(),
            Value::from(record.capability_version),
        );

        let outcome = match record.outcome {
            BrokerAuditOutcome::Succeeded => AuditOutcome::Succeeded,
            BrokerAuditOutcome::Failed => AuditOutcome::Failed,
            BrokerAuditOutcome::Denied => AuditOutcome::Denied,
        };
        let mut event = NewAuditEvent::new(
            record.principal.tenant_id(),
            AuditActor::agent(record.principal.user_id()),
            record.action_key,
            outcome,
            record.request_context,
        )
        .with_reason(record.reason)
        .with_redacted_metadata(metadata);
        if let Some(run_id) = record.agent_run_id {
            event = event.with_agent_run_id(run_id);
        }
        if let Some(target) = record.target {
            event = event.with_target(AuditTarget::new(target.kind(), target.id()));
        }

        append(&self.pool, &event)
            .await
            .map(|_| ())
            .map_err(|_| BrokerAuditError)
    }
}

#[cfg(test)]
mod tests {
    use cp_audit::RequestContext;
    use sqlx::postgres::PgPoolOptions;
    use uuid::Uuid;

    use crate::types::{AuthenticatedAgentPrincipal, CapabilityResource};

    use super::{BrokerAuditOutcome, BrokerAuditRecord, BrokerAuditSink, PostgresBrokerAuditSink};

    #[tokio::test]
    async fn postgres_sink_fails_closed_when_audit_storage_is_closed() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
            .unwrap_or_else(|_| unreachable!());
        pool.close().await;
        let sink = PostgresBrokerAuditSink::new(pool);
        let run_id = Uuid::new_v4();
        let record = BrokerAuditRecord {
            principal: AuthenticatedAgentPrincipal::from_authenticated_request(
                Uuid::new_v4(),
                Uuid::new_v4(),
            ),
            request_context: RequestContext::from_ids(Uuid::new_v4(), Uuid::new_v4()),
            capability_call_id: Uuid::new_v4(),
            action_key: "administration.catalog.read".to_string(),
            capability_version: 1,
            agent_run_id: Some(run_id),
            target: Some(
                CapabilityResource::parse("catalog", "roles").unwrap_or_else(|_| unreachable!()),
            ),
            outcome: BrokerAuditOutcome::Succeeded,
            reason: "completed",
        };

        assert!(sink.record(record).await.is_err());
    }

    #[tokio::test]
    async fn postgres_sink_maps_all_broker_outcomes_before_persistence() {
        for outcome in [
            BrokerAuditOutcome::Succeeded,
            BrokerAuditOutcome::Failed,
            BrokerAuditOutcome::Denied,
        ] {
            let pool = PgPoolOptions::new()
                .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
                .unwrap_or_else(|_| unreachable!());
            pool.close().await;
            let sink = PostgresBrokerAuditSink::new(pool);
            let record = BrokerAuditRecord {
                principal: AuthenticatedAgentPrincipal::from_authenticated_request(
                    Uuid::new_v4(),
                    Uuid::new_v4(),
                ),
                request_context: RequestContext::from_ids(Uuid::new_v4(), Uuid::new_v4()),
                capability_call_id: Uuid::new_v4(),
                action_key: "agent.capability.invoke".to_string(),
                capability_version: 1,
                agent_run_id: None,
                target: None,
                outcome,
                reason: "test",
            };

            assert!(sink.record(record).await.is_err());
        }
    }
}
