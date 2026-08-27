//! Actor-aware, append-only audit primitives shared by Campus Pilot modules.
//!
//! Callers provide already-redacted metadata and may append through a pool or
//! the same transaction that commits the consequential domain change.

use chrono::{DateTime, Utc};
use serde_json::{Map, Value};
use sqlx::{Executor, FromRow, Postgres, postgres::PgArguments, query::QueryAs};
use uuid::Uuid;

pub const REQUEST_ID_HEADER: &str = "x-request-id";
pub const CORRELATION_ID_HEADER: &str = "x-correlation-id";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestContext {
    request_id: Uuid,
    correlation_id: Uuid,
}

impl RequestContext {
    #[must_use]
    pub fn generate(incoming_correlation_id: Option<Uuid>) -> Self {
        let request_id = Uuid::new_v4();
        Self {
            request_id,
            correlation_id: incoming_correlation_id.unwrap_or(request_id),
        }
    }

    #[must_use]
    pub const fn from_ids(request_id: Uuid, correlation_id: Uuid) -> Self {
        Self {
            request_id,
            correlation_id,
        }
    }

    #[must_use]
    pub const fn request_id(self) -> Uuid {
        self.request_id
    }

    #[must_use]
    pub const fn correlation_id(self) -> Uuid {
        self.correlation_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditActorKind {
    Person,
    Agent,
    System,
}

impl AuditActorKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Agent => "agent",
            Self::System => "system",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditActor {
    kind: AuditActorKind,
    user_id: Option<Uuid>,
}

impl AuditActor {
    #[must_use]
    pub const fn person(user_id: Uuid) -> Self {
        Self {
            kind: AuditActorKind::Person,
            user_id: Some(user_id),
        }
    }

    /// Represents Agent acting for the authenticated requesting person.
    #[must_use]
    pub const fn agent(requester_user_id: Uuid) -> Self {
        Self {
            kind: AuditActorKind::Agent,
            user_id: Some(requester_user_id),
        }
    }

    #[must_use]
    pub const fn system() -> Self {
        Self {
            kind: AuditActorKind::System,
            user_id: None,
        }
    }

    #[must_use]
    pub const fn kind(self) -> AuditActorKind {
        self.kind
    }

    #[must_use]
    pub const fn user_id(self) -> Option<Uuid> {
        self.user_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditOutcome {
    Succeeded,
    Failed,
    Denied,
    Cancelled,
}

impl AuditOutcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Denied => "denied",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditTarget {
    kind: String,
    id: String,
}

impl AuditTarget {
    #[must_use]
    pub fn new(kind: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            id: id.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NewAuditEvent {
    tenant_id: Uuid,
    actor: AuditActor,
    action_key: String,
    target: Option<AuditTarget>,
    outcome: AuditOutcome,
    request_context: RequestContext,
    agent_run_id: Option<Uuid>,
    approval_id: Option<Uuid>,
    reason: Option<String>,
    redacted_metadata: Map<String, Value>,
}

impl NewAuditEvent {
    #[must_use]
    pub fn new(
        tenant_id: Uuid,
        actor: AuditActor,
        action_key: impl Into<String>,
        outcome: AuditOutcome,
        request_context: RequestContext,
    ) -> Self {
        Self {
            tenant_id,
            actor,
            action_key: action_key.into(),
            target: None,
            outcome,
            request_context,
            agent_run_id: None,
            approval_id: None,
            reason: None,
            redacted_metadata: Map::new(),
        }
    }

    #[must_use]
    pub fn with_target(mut self, target: AuditTarget) -> Self {
        self.target = Some(target);
        self
    }

    #[must_use]
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    #[must_use]
    pub fn with_agent_run_id(mut self, agent_run_id: Uuid) -> Self {
        self.agent_run_id = Some(agent_run_id);
        self
    }

    #[must_use]
    pub fn with_approval_id(mut self, approval_id: Uuid) -> Self {
        self.approval_id = Some(approval_id);
        self
    }

    /// Metadata must already be reduced and redacted by the owning domain.
    #[must_use]
    pub fn with_redacted_metadata(mut self, metadata: Map<String, Value>) -> Self {
        self.redacted_metadata = metadata;
        self
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct AuditEvent {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub actor_type: String,
    pub actor_user_id: Option<Uuid>,
    pub action_key: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub outcome: String,
    pub request_id: Uuid,
    pub correlation_id: Uuid,
    pub agent_run_id: Option<Uuid>,
    pub approval_id: Option<Uuid>,
    pub reason: Option<String>,
    pub redacted_metadata: Value,
    pub occurred_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Appends one audit event through either a pool or an open domain transaction.
pub async fn append<'e, E>(executor: E, event: &NewAuditEvent) -> Result<AuditEvent, sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    insert_query(event).fetch_one(executor).await
}

fn insert_query(event: &NewAuditEvent) -> QueryAs<'_, Postgres, AuditEvent, PgArguments> {
    let target_type = event.target.as_ref().map(|target| target.kind.as_str());
    let target_id = event.target.as_ref().map(|target| target.id.as_str());

    sqlx::query_as::<_, AuditEvent>(
        r#"
        INSERT INTO actor_audit_events (
            tenant_id, actor_type, actor_user_id, action_key, target_type,
            target_id, outcome, request_id, correlation_id, reason,
            agent_run_id, approval_id, redacted_metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        RETURNING id, tenant_id, actor_type, actor_user_id, action_key,
                  target_type, target_id, outcome, request_id, correlation_id,
                  agent_run_id, approval_id, reason, redacted_metadata,
                  occurred_at, created_at, updated_at, deleted_at
        "#,
    )
    .bind(event.tenant_id)
    .bind(event.actor.kind().as_str())
    .bind(event.actor.user_id())
    .bind(event.action_key.as_str())
    .bind(target_type)
    .bind(target_id)
    .bind(event.outcome.as_str())
    .bind(event.request_context.request_id())
    .bind(event.request_context.correlation_id())
    .bind(event.reason.as_deref())
    .bind(event.agent_run_id)
    .bind(event.approval_id)
    .bind(Value::Object(event.redacted_metadata.clone()))
}

#[cfg(test)]
mod tests {
    use super::{
        AuditActor, AuditActorKind, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext,
    };
    use serde_json::{Map, Value};
    use sqlx::Execute;
    use uuid::Uuid;

    #[test]
    fn request_context_uses_a_server_request_id_and_optional_correlation_id() {
        let standalone = RequestContext::generate(None);
        assert_eq!(standalone.request_id(), standalone.correlation_id());

        let incoming = Uuid::new_v4();
        let correlated = RequestContext::generate(Some(incoming));
        assert_eq!(correlated.correlation_id(), incoming);
        assert_ne!(correlated.request_id(), incoming);
    }

    #[test]
    fn actors_and_outcomes_have_stable_storage_values() {
        let user_id = Uuid::new_v4();
        let person = AuditActor::person(user_id);
        let agent = AuditActor::agent(user_id);
        let system = AuditActor::system();

        assert_eq!(person.kind(), AuditActorKind::Person);
        assert_eq!(person.kind().as_str(), "person");
        assert_eq!(person.user_id(), Some(user_id));
        assert_eq!(agent.kind().as_str(), "agent");
        assert_eq!(system.kind().as_str(), "system");
        assert_eq!(system.user_id(), None);
        assert_eq!(AuditOutcome::Succeeded.as_str(), "succeeded");
        assert_eq!(AuditOutcome::Failed.as_str(), "failed");
        assert_eq!(AuditOutcome::Denied.as_str(), "denied");
        assert_eq!(AuditOutcome::Cancelled.as_str(), "cancelled");
    }

    #[test]
    fn event_builder_keeps_target_reason_and_object_metadata_separate() {
        let request_context = RequestContext::from_ids(Uuid::new_v4(), Uuid::new_v4());
        let agent_run_id = Uuid::new_v4();
        let approval_id = Uuid::new_v4();
        let mut metadata = Map::new();
        metadata.insert("changed_fields".to_string(), Value::from(2));

        let event = NewAuditEvent::new(
            Uuid::new_v4(),
            AuditActor::system(),
            "administration.school_settings.update",
            AuditOutcome::Succeeded,
            request_context,
        )
        .with_target(AuditTarget::new("school_profile", "singleton"))
        .with_reason("Campus configuration updated")
        .with_agent_run_id(agent_run_id)
        .with_approval_id(approval_id)
        .with_redacted_metadata(metadata);

        assert_eq!(
            event.target.as_ref().map(|target| target.kind.as_str()),
            Some("school_profile")
        );
        assert_eq!(
            event.target.as_ref().map(|target| target.id.as_str()),
            Some("singleton")
        );
        assert_eq!(
            event.reason.as_deref(),
            Some("Campus configuration updated")
        );
        assert_eq!(event.agent_run_id, Some(agent_run_id));
        assert_eq!(event.approval_id, Some(approval_id));
        assert_eq!(
            event.redacted_metadata.get("changed_fields"),
            Some(&Value::from(2))
        );

        let query = super::insert_query(&event);
        assert!(query.sql().contains("INSERT INTO actor_audit_events"));
    }
}
