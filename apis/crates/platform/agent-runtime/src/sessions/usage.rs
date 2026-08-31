//! Transactional Agent hard-limit enforcement and immutable usage evidence.

use std::{str::FromStr, sync::Arc};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use cp_agent::{
    AuthenticatedAgentPrincipal, CapabilityExecutionProof, CapabilityScope,
    DurabilityProofRejected, PreparedCapabilityCallFacts, PreparedCapabilityCallVerifier,
};
use cp_audit::{AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext};
use serde_json::{Value, json};
use sqlx::{FromRow, PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use super::RunLease;
use super::usage_types::{
    AgentUsageDemand, AgentUsageError, AgentUsageMeter, AgentUsageReportCursor,
    AgentUsageReportDimension, AgentUsageReportPage, AgentUsageReportQuery, AgentUsageReportRow,
    AgentUsageReservationStatus, AgentUsageStage, AgentUsageTerminalAction, PrepareAgentUsage,
    PreparedAgentUsage,
};

const MAX_ROLE_KEYS: usize = 32;
const USAGE_DENIED_ACTION: &str = "agent.usage.denied";

#[derive(Clone)]
pub struct AgentUsageRuntime {
    ops: AgentUsageOps,
}

impl AgentUsageRuntime {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self {
            ops: AgentUsageOps { pool },
        }
    }

    #[must_use]
    pub fn prepared_capability_call_verifier(&self) -> Arc<dyn PreparedCapabilityCallVerifier> {
        Arc::new(UsageCapabilityVerifier {
            ops: self.ops.clone(),
        })
    }

    pub async fn prepare(
        &self,
        tenant_id: Uuid,
        actor_user_id: Uuid,
        command: PrepareAgentUsage,
    ) -> Result<PreparedAgentUsage, AgentUsageError> {
        self.ops.prepare(tenant_id, actor_user_id, command).await
    }

    pub async fn release_or_expire(
        &self,
        tenant_id: Uuid,
        reservation_id: Uuid,
        action: AgentUsageTerminalAction,
    ) -> Result<PreparedAgentUsage, AgentUsageError> {
        self.ops
            .release_or_expire(tenant_id, reservation_id, action)
            .await
    }

    /// Consumes the exact active queue lease before a provider request is sent.
    pub async fn claim_provider_attempt(
        &self,
        tenant_id: Uuid,
        actor_user_id: Uuid,
        reservation_id: Uuid,
        lease: &RunLease,
    ) -> Result<(), AgentUsageError> {
        self.ops
            .claim_provider_attempt(tenant_id, actor_user_id, reservation_id, lease)
            .await
    }

    pub async fn commit_terminal_usage(
        &self,
        tenant_id: Uuid,
        reservation_id: Uuid,
    ) -> Result<PreparedAgentUsage, AgentUsageError> {
        self.ops
            .commit_terminal_usage(tenant_id, reservation_id)
            .await
    }

    pub async fn report(
        &self,
        tenant_id: Uuid,
        query: AgentUsageReportQuery,
    ) -> Result<AgentUsageReportPage, AgentUsageError> {
        self.ops.report(tenant_id, query).await
    }
}

#[derive(Clone)]
struct AgentUsageOps {
    pool: PgPool,
}

#[derive(Clone)]
struct UsageCapabilityVerifier {
    ops: AgentUsageOps,
}

#[async_trait]
impl PreparedCapabilityCallVerifier for UsageCapabilityVerifier {
    async fn verify_and_consume(
        &self,
        principal: AuthenticatedAgentPrincipal,
        facts: &PreparedCapabilityCallFacts,
        proof: &CapabilityExecutionProof,
    ) -> Result<(), DurabilityProofRejected> {
        self.ops
            .verify_and_consume_capability(principal, facts, proof)
            .await
            .map_err(|_| DurabilityProofRejected)
    }
}

#[derive(Debug, FromRow)]
struct RunIdentityRow {
    requested_by: Uuid,
    origin_module_key: String,
    status: String,
}

#[derive(Debug, FromRow)]
struct StageIdentityRow {
    stage_sequence: i16,
    provider_attempt_id: Option<Uuid>,
    capability_call_id: Option<Uuid>,
    provider_key: Option<String>,
    provider_model_id: Option<String>,
    capability_module_key: Option<String>,
    capability_key: Option<String>,
    request_fingerprint: Vec<u8>,
}

#[derive(Debug, FromRow)]
struct ReservationRow {
    id: Uuid,
    run_id: Uuid,
    provider_attempt_id: Option<Uuid>,
    capability_call_id: Option<Uuid>,
    actor_user_id: Uuid,
    role_keys: Vec<String>,
    origin_module_key: String,
    capability_module_key: Option<String>,
    capability_key: Option<String>,
    provider_key: Option<String>,
    provider_model_id: Option<String>,
    stage_kind: String,
    stage_sequence: i16,
    idempotency_key: String,
    request_fingerprint: Vec<u8>,
    status: String,
    expires_at: Option<DateTime<Utc>>,
    claimed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
struct DefinitionRow {
    definition_kind: String,
    campus_rule_id: Option<Uuid>,
    source_lease_id: Option<Uuid>,
    entitlement_limit_key: Option<String>,
    definition_version: Option<i64>,
    scope_kind: String,
    scope_value: String,
    meter_key: String,
    unit: String,
    currency_code: Option<String>,
    currency_exponent: Option<i16>,
    period: String,
    limit_value: i64,
}

#[derive(Debug)]
struct PreparedDefinition {
    definition: DefinitionRow,
    bucket_id: Uuid,
    committed_before: i64,
    reserved_before: i64,
    period_start: DateTime<Utc>,
    period_end: Option<DateTime<Utc>>,
    demand: AgentUsageDemand,
    allowed: bool,
    entitlement_reservation_id: Option<Uuid>,
}

#[derive(Debug, FromRow)]
struct BucketRow {
    id: Uuid,
    period_start: DateTime<Utc>,
    period_end: Option<DateTime<Utc>>,
    committed_value: i64,
    reserved_value: i64,
}

#[derive(Debug, FromRow)]
struct ItemLifecycleRow {
    id: Uuid,
    bucket_id: Option<Uuid>,
    entitlement_bucket_id: Option<Uuid>,
    entitlement_reservation_id: Option<Uuid>,
    reserved_amount: i64,
    meter_key: String,
    period_start: DateTime<Utc>,
    period_end: Option<DateTime<Utc>>,
    source_lease_id: Option<Uuid>,
    entitlement_limit_key: Option<String>,
    unit: String,
    currency_code: Option<String>,
    currency_exponent: Option<i16>,
    pricing_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReconciledAmount {
    amount: i64,
    basis: &'static str,
}

#[derive(Debug, FromRow)]
struct UsageEventSourceRow {
    thread_id: Uuid,
    actor_user_id: Uuid,
    role_keys: Vec<String>,
    origin_module_key: String,
    task_class: String,
    request_id: Uuid,
    correlation_id: Uuid,
    event_kind: String,
    run_id: Uuid,
    provider_attempt_id: Option<Uuid>,
    provider_turn_index: Option<i16>,
    provider_attempt_index: Option<i16>,
    provider_connection_id: Option<Uuid>,
    provider_key: Option<String>,
    provider_model_id: Option<String>,
    provider_model_snapshot_id: Option<Uuid>,
    route_priority: Option<i16>,
    failure_origin: Option<String>,
    failure_category: Option<String>,
    capability_call_id: Option<Uuid>,
    capability_module_key: Option<String>,
    capability_key: Option<String>,
    capability_version: Option<i32>,
    approval_state: Option<String>,
    outcome: String,
    safe_failure_code: Option<String>,
    duration_ms: i64,
    occurred_at: DateTime<Utc>,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cached_tokens: Option<i64>,
    reasoning_tokens: Option<i64>,
    provider_reported_cost_amount: Option<i64>,
    provider_reported_cost_currency: Option<String>,
    provider_reported_cost_exponent: Option<i16>,
    provider_reported_pricing_version: Option<String>,
    estimated_cost_amount: Option<i64>,
    estimated_cost_currency: Option<String>,
    estimated_cost_exponent: Option<i16>,
    estimated_pricing_version: Option<String>,
}

#[derive(Debug, FromRow)]
struct ReportRow {
    event_id: Uuid,
    event_kind: String,
    outcome: String,
    run_id: Uuid,
    actor_user_id: Uuid,
    origin_module_key: String,
    capability_module_key: Option<String>,
    capability_key: Option<String>,
    provider_key: Option<String>,
    provider_model_id: Option<String>,
    meter_key: String,
    amount: Option<i64>,
    enforcement_amount: Option<i64>,
    enforcement_basis: Option<String>,
    currency_code: Option<String>,
    currency_exponent: Option<i16>,
    pricing_version: Option<String>,
    occurred_at: DateTime<Utc>,
}

impl AgentUsageOps {
    async fn prepare(
        &self,
        tenant_id: Uuid,
        actor_user_id: Uuid,
        command: PrepareAgentUsage,
    ) -> Result<PreparedAgentUsage, AgentUsageError> {
        if tenant_id.is_nil() || actor_user_id.is_nil() {
            return Err(AgentUsageError::NotFound);
        }
        let mut transaction = self.pool.begin().await.map_err(storage)?;
        advisory_lock(&mut transaction, tenant_id, &command.idempotency_key).await?;
        expire_stale_for_tenant(&mut transaction, tenant_id).await?;

        let run = lock_run(&mut transaction, tenant_id, command.run_id).await?;
        if run.requested_by != actor_user_id || !matches!(run.status.as_str(), "queued" | "running")
        {
            return Err(AgentUsageError::NotFound);
        }
        let role_keys = canonical_role_keys(&mut transaction, tenant_id, actor_user_id).await?;
        let stage = load_stage_identity(&mut transaction, tenant_id, &command).await?;

        if let Some(existing) = load_reservation_by_identity(
            &mut transaction,
            tenant_id,
            command.run_id,
            &command.idempotency_key,
            stage_kind(&command.stage),
            stage.stage_sequence,
        )
        .await?
        {
            verify_replay(&existing, actor_user_id, &role_keys, &run, &stage, &command)?;
            verify_replay_demands(&mut transaction, tenant_id, existing.id, &command).await?;
            let result = reservation_projection(&existing)?;
            transaction.commit().await.map_err(storage)?;
            return if result.status == AgentUsageReservationStatus::Denied {
                Err(AgentUsageError::Denied {
                    reservation_id: result.reservation_id,
                })
            } else {
                Ok(result)
            };
        }

        let reservation_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO agent_limit_reservations (
                id, tenant_id, run_id, provider_attempt_id, capability_call_id,
                actor_user_id, role_keys, origin_module_key,
                capability_module_key, capability_key, provider_key,
                provider_model_id, stage_kind, stage_sequence, idempotency_key,
                request_fingerprint
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                $13, $14, $15, $16
            )
            "#,
        )
        .bind(reservation_id)
        .bind(tenant_id)
        .bind(command.run_id)
        .bind(stage.provider_attempt_id)
        .bind(stage.capability_call_id)
        .bind(actor_user_id)
        .bind(&role_keys)
        .bind(&run.origin_module_key)
        .bind(&stage.capability_module_key)
        .bind(&stage.capability_key)
        .bind(&stage.provider_key)
        .bind(&stage.provider_model_id)
        .bind(stage_kind(&command.stage))
        .bind(stage.stage_sequence)
        .bind(&command.idempotency_key)
        .bind(&command.request_fingerprint[..])
        .execute(&mut *transaction)
        .await
        .map_err(storage)?;

        let definitions = matching_definitions(
            &mut transaction,
            tenant_id,
            actor_user_id,
            &role_keys,
            &run.origin_module_key,
            &stage,
            stage_kind(&command.stage),
        )
        .await?;
        let mut prepared = Vec::with_capacity(definitions.len());
        for definition in definitions {
            let meter = AgentUsageMeter::from_str(&definition.meter_key)?;
            let demand = command
                .demands
                .get(&meter)
                .cloned()
                .ok_or(AgentUsageError::MissingDemand)?;
            verify_money_tuple(&definition, &demand)?;
            let bucket = lock_definition_bucket(&mut transaction, tenant_id, &definition).await?;
            let occupied = bucket
                .committed_value
                .checked_add(bucket.reserved_value)
                .and_then(|value| value.checked_add(demand.amount))
                .ok_or_else(AgentUsageError::storage_contract)?;
            prepared.push(PreparedDefinition {
                allowed: occupied <= definition.limit_value,
                definition,
                bucket_id: bucket.id,
                committed_before: bucket.committed_value,
                reserved_before: bucket.reserved_value,
                period_start: bucket.period_start,
                period_end: bucket.period_end,
                demand,
                entitlement_reservation_id: None,
            });
        }

        let denied = prepared.iter().any(|item| !item.allowed);
        let expires_at = Utc::now()
            + chrono::Duration::from_std(command.ttl)
                .map_err(|_| AgentUsageError::invalid("invalid_reservation_ttl"))?;
        if !denied {
            reserve_all(
                &mut transaction,
                tenant_id,
                actor_user_id,
                reservation_id,
                &command.idempotency_key,
                expires_at,
                &mut prepared,
            )
            .await?;
        }
        insert_prepared_items(
            &mut transaction,
            tenant_id,
            command.run_id,
            reservation_id,
            denied,
            &prepared,
        )
        .await?;

        let status = if prepared.is_empty() {
            "not_limited"
        } else if denied {
            "denied"
        } else {
            "reserved"
        };
        let stored_expires_at = if status == "reserved" {
            Some(expires_at)
        } else {
            None
        };
        sqlx::query(
            r#"
            UPDATE agent_limit_reservations
            SET status = $3, expires_at = $4,
                denied_at = CASE WHEN $3 = 'denied' THEN STATEMENT_TIMESTAMP() END,
                updated_at = STATEMENT_TIMESTAMP()
            WHERE id = $1 AND tenant_id = $2 AND status = 'preparing'
            "#,
        )
        .bind(reservation_id)
        .bind(tenant_id)
        .bind(status)
        .bind(stored_expires_at)
        .execute(&mut *transaction)
        .await
        .map_err(storage)?;
        transaction.commit().await.map_err(storage)?;

        let projection = PreparedAgentUsage {
            reservation_id,
            status: AgentUsageReservationStatus::from_str(status)?,
            expires_at: stored_expires_at,
        };
        if denied {
            Err(AgentUsageError::Denied { reservation_id })
        } else {
            Ok(projection)
        }
    }

    async fn release_or_expire(
        &self,
        tenant_id: Uuid,
        reservation_id: Uuid,
        action: AgentUsageTerminalAction,
    ) -> Result<PreparedAgentUsage, AgentUsageError> {
        let mut transaction = self.pool.begin().await.map_err(storage)?;
        let reservation = lock_reservation(&mut transaction, tenant_id, reservation_id).await?;
        let current = AgentUsageReservationStatus::from_str(&reservation.status)?;
        let target = match action {
            AgentUsageTerminalAction::Release => "released",
            AgentUsageTerminalAction::Expire => "expired",
        };
        if current == AgentUsageReservationStatus::from_str(target)? {
            transaction.commit().await.map_err(storage)?;
            return reservation_projection(&reservation);
        }
        if current != AgentUsageReservationStatus::Reserved {
            return Err(AgentUsageError::InvalidTransition);
        }
        if reservation.claimed_at.is_some() {
            return Err(AgentUsageError::InvalidTransition);
        }
        transition_locked_reservation(&mut transaction, tenant_id, &reservation, target).await?;
        transaction.commit().await.map_err(storage)?;
        Ok(PreparedAgentUsage {
            reservation_id,
            status: AgentUsageReservationStatus::from_str(target)?,
            expires_at: reservation.expires_at,
        })
    }

    async fn commit_terminal_usage(
        &self,
        tenant_id: Uuid,
        reservation_id: Uuid,
    ) -> Result<PreparedAgentUsage, AgentUsageError> {
        let mut transaction = self.pool.begin().await.map_err(storage)?;
        let reservation = lock_reservation(&mut transaction, tenant_id, reservation_id).await?;
        let status = AgentUsageReservationStatus::from_str(&reservation.status)?;
        if status == AgentUsageReservationStatus::Committed {
            verify_committed_projection(&mut transaction, tenant_id, &reservation).await?;
            transaction.commit().await.map_err(storage)?;
            return reservation_projection(&reservation);
        }
        if status == AgentUsageReservationStatus::Denied {
            append_terminal_usage_event(&mut transaction, tenant_id, &reservation).await?;
            transaction.commit().await.map_err(storage)?;
            return Err(AgentUsageError::Denied { reservation_id });
        }
        if status == AgentUsageReservationStatus::NotLimited {
            append_terminal_usage_event(&mut transaction, tenant_id, &reservation).await?;
            transaction.commit().await.map_err(storage)?;
            return reservation_projection(&reservation);
        }
        if status != AgentUsageReservationStatus::Reserved {
            return Err(AgentUsageError::InvalidTransition);
        }
        let source = load_usage_source(&mut transaction, tenant_id, &reservation).await?;
        if !reserved_source_is_committable(
            &reservation.stage_kind,
            reservation.claimed_at.is_some(),
            source.failure_origin.as_deref(),
        ) {
            return Err(AgentUsageError::InvalidTransition);
        }
        commit_locked_reservation(&mut transaction, tenant_id, &reservation, &source).await?;
        let mut committed = reservation;
        committed.status = "committed".to_owned();
        append_terminal_usage_event(&mut transaction, tenant_id, &committed).await?;
        transaction.commit().await.map_err(storage)?;
        reservation_projection(&committed)
    }

    async fn report(
        &self,
        tenant_id: Uuid,
        query: AgentUsageReportQuery,
    ) -> Result<AgentUsageReportPage, AgentUsageError> {
        let (dimension_kind, dimension_primary, dimension_secondary) = match &query.dimension {
            AgentUsageReportDimension::Person(value) => ("person", value.to_string(), None),
            AgentUsageReportDimension::OriginModule(value) => {
                ("origin_module", value.clone(), None)
            }
            AgentUsageReportDimension::CapabilityModule(value) => {
                ("capability_module", value.clone(), None)
            }
            AgentUsageReportDimension::Capability(value) => ("capability", value.clone(), None),
            AgentUsageReportDimension::Provider(value) => ("provider", value.clone(), None),
            AgentUsageReportDimension::Model { provider, model } => {
                ("model", provider.clone(), Some(model.clone()))
            }
        };
        let (currency, exponent, pricing_version) = query
            .currency
            .clone()
            .map_or((None, None, None), |(code, exponent, pricing)| {
                (Some(code), Some(exponent), pricing)
            });
        let rows = sqlx::query_as::<_, ReportRow>(
            r#"
            SELECT event.id AS event_id, event.event_kind, event.outcome,
                   event.run_id, event.actor_user_id, event.origin_module_key,
                   event.capability_module_key, event.capability_key,
                   event.provider_key, event.provider_model_id,
                   measure.meter_key, measure.amount, measure.enforcement_amount,
                   measure.enforcement_basis, measure.currency_code,
                   measure.currency_exponent, measure.pricing_version,
                   event.occurred_at
            FROM agent_usage_events AS event
            INNER JOIN agent_usage_measures AS measure
              ON measure.usage_event_id = event.id
             AND measure.tenant_id = event.tenant_id
            WHERE event.tenant_id = $1
              AND event.deleted_at IS NULL
              AND measure.deleted_at IS NULL
              AND CASE $2
                    WHEN 'person' THEN event.actor_user_id::TEXT = $3
                    WHEN 'origin_module' THEN event.origin_module_key = $3
                    WHEN 'capability_module' THEN event.capability_module_key = $3
                    WHEN 'capability' THEN event.capability_key = $3
                    WHEN 'provider' THEN event.provider_key = $3
                    WHEN 'model' THEN event.provider_key = $3
                                      AND event.provider_model_id = $4
                  END
              AND ($5::TEXT IS NULL OR measure.meter_key = $5)
              AND ($6::TEXT IS NULL OR measure.currency_code = $6)
              AND ($7::SMALLINT IS NULL OR measure.currency_exponent = $7)
              AND ($6::TEXT IS NULL OR measure.pricing_version IS NOT DISTINCT FROM $8)
              AND (
                    $9::TIMESTAMPTZ IS NULL
                    OR event.occurred_at < $9
                    OR (event.occurred_at = $9 AND event.id < $10)
                    OR (event.occurred_at = $9 AND event.id = $10
                        AND measure.meter_key > $11)
                  )
            ORDER BY event.occurred_at DESC, event.id DESC, measure.meter_key
            LIMIT $12
            "#,
        )
        .bind(tenant_id)
        .bind(dimension_kind)
        .bind(dimension_primary)
        .bind(dimension_secondary)
        .bind(query.meter.map(AgentUsageMeter::as_str))
        .bind(currency)
        .bind(exponent)
        .bind(pricing_version)
        .bind(query.cursor.map(|cursor| cursor.occurred_at))
        .bind(query.cursor.map(|cursor| cursor.event_id))
        .bind(query.cursor.map(|cursor| cursor.meter.as_str()))
        .bind(query.limit + 1)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        page_report(rows, query.limit)
    }

    async fn claim_provider_attempt(
        &self,
        tenant_id: Uuid,
        actor_user_id: Uuid,
        reservation_id: Uuid,
        lease: &RunLease,
    ) -> Result<(), AgentUsageError> {
        if tenant_id.is_nil()
            || actor_user_id.is_nil()
            || reservation_id.is_nil()
            || lease.run_id.is_nil()
        {
            return Err(AgentUsageError::NotFound);
        }
        let mut transaction = self.pool.begin().await.map_err(storage)?;
        let queue_matches = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT TRUE FROM agent_run_queue
            WHERE tenant_id = $1 AND run_id = $2 AND state = 'leased'
              AND leased_by = $3 AND lease_token = $4 AND version = $5
              AND checkpoint = 'provider_in_flight'
              AND lease_expires_at > STATEMENT_TIMESTAMP()
              AND cancel_requested_at IS NULL AND deleted_at IS NULL
            FOR UPDATE
            "#,
        )
        .bind(tenant_id)
        .bind(lease.run_id)
        .bind(&lease.worker_id)
        .bind(lease.lease_token)
        .bind(lease.fence_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(storage)?;
        if queue_matches != Some(true) {
            return Err(AgentUsageError::NotFound);
        }
        let role_keys = canonical_role_keys(&mut transaction, tenant_id, actor_user_id).await?;
        if !reservation_definitions_are_current(&mut transaction, tenant_id, reservation_id).await?
        {
            return Err(AgentUsageError::NotFound);
        }
        let claimed = sqlx::query_scalar::<_, Uuid>(
            r#"
            UPDATE agent_limit_reservations AS reservation
            SET claimed_at = STATEMENT_TIMESTAMP(), claimed_by_worker_id = $5,
                claim_fence_version = $6, updated_at = STATEMENT_TIMESTAMP()
            FROM agent_provider_attempts AS attempt
            INNER JOIN agent_execution_steps AS step
              ON step.provider_attempt_id = attempt.id
             AND step.tenant_id = attempt.tenant_id
             AND step.run_id = attempt.run_id
             AND step.step_kind = 'provider_attempt'
            WHERE reservation.id = $1 AND reservation.tenant_id = $2
              AND reservation.run_id = $3 AND reservation.actor_user_id = $4
              AND reservation.role_keys = $7
              AND reservation.stage_kind = 'provider_attempt'
              AND reservation.provider_attempt_id = attempt.id
              AND reservation.capability_call_id IS NULL
              AND reservation.status IN ('reserved', 'not_limited')
              AND (reservation.status = 'not_limited'
                   OR reservation.expires_at > STATEMENT_TIMESTAMP())
              AND reservation.claimed_at IS NULL
              AND reservation.claimed_by_worker_id IS NULL
              AND reservation.claim_fence_version IS NULL
              AND attempt.tenant_id = reservation.tenant_id
              AND attempt.run_id = reservation.run_id
              AND attempt.status = 'running'
              AND reservation.stage_sequence =
                    ((attempt.turn_index - 1) * 3 + attempt.attempt_index)
              AND reservation.provider_key = attempt.provider_key
              AND reservation.provider_model_id = attempt.provider_model_id
              AND reservation.request_fingerprint = step.input_fingerprint
            RETURNING reservation.id
            "#,
        )
        .bind(reservation_id)
        .bind(tenant_id)
        .bind(lease.run_id)
        .bind(actor_user_id)
        .bind(&lease.worker_id)
        .bind(lease.fence_version)
        .bind(&role_keys)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(storage)?;
        if claimed != Some(reservation_id) {
            return Err(AgentUsageError::NotFound);
        }
        transaction.commit().await.map_err(storage)
    }

    async fn verify_and_consume_capability(
        &self,
        principal: AuthenticatedAgentPrincipal,
        facts: &PreparedCapabilityCallFacts,
        proof: &CapabilityExecutionProof,
    ) -> Result<(), AgentUsageError> {
        let run_id = facts.agent_run_id().ok_or(AgentUsageError::NotFound)?;
        if principal.tenant_id() != proof.tenant_id()
            || principal.user_id() != proof.user_id()
            || facts.capability_call_id() != proof.capability_call_id()
            || run_id != proof.run_id()
        {
            return Err(AgentUsageError::NotFound);
        }
        let mut transaction = self.pool.begin().await.map_err(storage)?;
        let queue_matches = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT TRUE FROM agent_run_queue
            WHERE tenant_id = $1 AND run_id = $2 AND state = 'leased'
              AND leased_by = $3 AND lease_token = $4 AND version = $5
              AND checkpoint = 'capability_in_flight'
              AND lease_expires_at > STATEMENT_TIMESTAMP()
              AND cancel_requested_at IS NULL AND deleted_at IS NULL
            FOR UPDATE
            "#,
        )
        .bind(principal.tenant_id())
        .bind(run_id)
        .bind(proof.worker_id())
        .bind(proof.lease_token())
        .bind(proof.fence_version())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(storage)?;
        if queue_matches != Some(true) {
            return Err(AgentUsageError::NotFound);
        }
        let role_keys =
            canonical_role_keys(&mut transaction, principal.tenant_id(), principal.user_id())
                .await?;
        if !reservation_definitions_are_current(
            &mut transaction,
            principal.tenant_id(),
            proof.usage_reservation_id(),
        )
        .await?
        {
            return Err(AgentUsageError::NotFound);
        }
        let (scope_kind, resource_references) = capability_scope_evidence(facts.scope());
        let updated = sqlx::query_scalar::<_, Uuid>(
            r#"
            UPDATE agent_limit_reservations AS reservation
            SET claimed_at = STATEMENT_TIMESTAMP(), claimed_by_worker_id = $4,
                claim_fence_version = $5, updated_at = STATEMENT_TIMESTAMP()
            FROM agent_capability_calls AS call
            WHERE reservation.id = $1
              AND reservation.tenant_id = $2
              AND reservation.run_id = $3
              AND reservation.actor_user_id = $6
              AND reservation.role_keys = $7
              AND reservation.stage_kind = 'capability_call'
              AND reservation.capability_call_id = $8
              AND reservation.provider_attempt_id IS NULL
              AND reservation.status IN ('reserved', 'not_limited')
              AND (reservation.status = 'not_limited'
                   OR reservation.expires_at > STATEMENT_TIMESTAMP())
              AND reservation.claimed_at IS NULL
              AND reservation.claimed_by_worker_id IS NULL
              AND reservation.claim_fence_version IS NULL
              AND reservation.request_fingerprint = $9
              AND EXISTS (
                  SELECT 1 FROM agent_runs AS run
                  WHERE run.id = reservation.run_id
                    AND run.tenant_id = reservation.tenant_id
                    AND run.requested_by = reservation.actor_user_id
                    AND run.status = 'running'
                    AND run.request_id = $15
                    AND run.correlation_id = $16
                    AND run.deleted_at IS NULL
              )
              AND call.id = reservation.capability_call_id
              AND call.tenant_id = reservation.tenant_id
              AND call.run_id = reservation.run_id
              AND call.call_sequence = reservation.stage_sequence
              AND call.status = 'running'
              AND call.capability_key = $10
              AND call.capability_version = $11
              AND call.product_operation_key = $12
              AND call.owning_module_key = $13
              AND call.required_permission = $14
              AND call.input_fingerprint = $9
              AND call.scope_kind = $17
              AND call.resource_references = $18
            RETURNING reservation.id
            "#,
        )
        .bind(proof.usage_reservation_id())
        .bind(principal.tenant_id())
        .bind(run_id)
        .bind(proof.worker_id())
        .bind(proof.fence_version())
        .bind(principal.user_id())
        .bind(&role_keys)
        .bind(facts.capability_call_id().as_uuid())
        .bind(&facts.input_binding_sha256()[..])
        .bind(facts.key().as_str())
        .bind(i32::from(facts.version().get()))
        .bind(facts.operation_key())
        .bind(facts.module_key())
        .bind(facts.required_permission())
        .bind(facts.request_context().request_id())
        .bind(facts.request_context().correlation_id())
        .bind(scope_kind)
        .bind(resource_references)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(storage)?;
        if updated != Some(proof.usage_reservation_id()) {
            return Err(AgentUsageError::NotFound);
        }
        transaction.commit().await.map_err(storage)
    }
}

async fn advisory_lock(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> Result<(), AgentUsageError> {
    sqlx::query("SELECT PG_ADVISORY_XACT_LOCK(HASHTEXT($1), HASHTEXT($2))")
        .bind(tenant_id.to_string())
        .bind(idempotency_key)
        .execute(&mut **transaction)
        .await
        .map_err(storage)?;
    Ok(())
}

async fn lock_run(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
) -> Result<RunIdentityRow, AgentUsageError> {
    sqlx::query_as::<_, RunIdentityRow>(
        r#"
        SELECT requested_by, origin_module_key, status
        FROM agent_runs
        WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(run_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(storage)?
    .ok_or(AgentUsageError::NotFound)
}

async fn canonical_role_keys(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<String>, AgentUsageError> {
    let roles = sqlx::query_scalar::<_, Vec<String>>(
        r#"
        SELECT ARRAY_AGG(DISTINCT role.key ORDER BY role.key)::TEXT[]
        FROM users AS person
        CROSS JOIN LATERAL UNNEST(person.roles) AS assigned(role_key)
        INNER JOIN roles AS role
          ON role.tenant_id = person.tenant_id
         AND role.key = assigned.role_key
         AND role.deleted_at IS NULL
        WHERE person.id = $1 AND person.tenant_id = $2
          AND person.is_active = TRUE AND person.deleted_at IS NULL
        GROUP BY person.id
        "#,
    )
    .bind(user_id)
    .bind(tenant_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(storage)?
    .ok_or(AgentUsageError::NotFound)?;
    if roles.is_empty() || roles.len() > MAX_ROLE_KEYS {
        return Err(AgentUsageError::NotFound);
    }
    Ok(roles)
}

fn stage_kind(stage: &AgentUsageStage) -> &'static str {
    match stage {
        AgentUsageStage::Run => "run",
        AgentUsageStage::ProviderAttempt { .. } => "provider_attempt",
        AgentUsageStage::CapabilityCall { .. } => "capability_call",
    }
}

async fn load_stage_identity(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    command: &PrepareAgentUsage,
) -> Result<StageIdentityRow, AgentUsageError> {
    match command.stage {
        AgentUsageStage::Run => Ok(StageIdentityRow {
            stage_sequence: 0,
            provider_attempt_id: None,
            capability_call_id: None,
            provider_key: None,
            provider_model_id: None,
            capability_module_key: None,
            capability_key: None,
            request_fingerprint: command.request_fingerprint.to_vec(),
        }),
        AgentUsageStage::ProviderAttempt { attempt_id } => sqlx::query_as::<_, StageIdentityRow>(
            r#"
                SELECT ((attempt.turn_index - 1) * 3 + attempt.attempt_index)::SMALLINT
                           AS stage_sequence,
                       attempt.id AS provider_attempt_id,
                       NULL::UUID AS capability_call_id,
                       attempt.provider_key, attempt.provider_model_id,
                       NULL::TEXT AS capability_module_key,
                       NULL::TEXT AS capability_key,
                       step.input_fingerprint AS request_fingerprint
                FROM agent_provider_attempts AS attempt
                INNER JOIN agent_execution_steps AS step
                  ON step.provider_attempt_id = attempt.id
                 AND step.tenant_id = attempt.tenant_id
                 AND step.run_id = attempt.run_id
                 AND step.step_kind = 'provider_attempt'
                WHERE attempt.id = $1 AND attempt.tenant_id = $2
                  AND attempt.run_id = $3 AND attempt.status = 'running'
                FOR UPDATE OF attempt, step
                "#,
        )
        .bind(attempt_id)
        .bind(tenant_id)
        .bind(command.run_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(storage)?
        .filter(|row| row.request_fingerprint == command.request_fingerprint)
        .ok_or(AgentUsageError::NotFound),
        AgentUsageStage::CapabilityCall { call_id } => sqlx::query_as::<_, StageIdentityRow>(
            r#"
                SELECT call_sequence AS stage_sequence,
                       NULL::UUID AS provider_attempt_id,
                       id AS capability_call_id,
                       NULL::TEXT AS provider_key,
                       NULL::TEXT AS provider_model_id,
                       owning_module_key AS capability_module_key,
                       capability_key,
                       input_fingerprint AS request_fingerprint
                FROM agent_capability_calls
                WHERE id = $1 AND tenant_id = $2 AND run_id = $3
                  AND status = 'running'
                FOR UPDATE
                "#,
        )
        .bind(call_id)
        .bind(tenant_id)
        .bind(command.run_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(storage)?
        .filter(|row| row.request_fingerprint == command.request_fingerprint)
        .ok_or(AgentUsageError::NotFound),
    }
}

async fn load_reservation_by_identity(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
    idempotency_key: &str,
    stage_kind: &str,
    stage_sequence: i16,
) -> Result<Option<ReservationRow>, AgentUsageError> {
    sqlx::query_as::<_, ReservationRow>(
        r#"
        SELECT id, run_id, provider_attempt_id, capability_call_id,
               actor_user_id, role_keys, origin_module_key,
               capability_module_key, capability_key, provider_key,
               provider_model_id, stage_kind, stage_sequence, idempotency_key,
               request_fingerprint, status, expires_at, claimed_at
        FROM agent_limit_reservations
        WHERE tenant_id = $1
          AND (idempotency_key = $2
               OR (run_id = $3 AND stage_kind = $4 AND stage_sequence = $5))
          AND deleted_at IS NULL
        ORDER BY (idempotency_key = $2) DESC
        FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(idempotency_key)
    .bind(run_id)
    .bind(stage_kind)
    .bind(stage_sequence)
    .fetch_all(&mut **transaction)
    .await
    .map_err(storage)
    .and_then(|rows| {
        let mut rows = rows.into_iter();
        let first = rows.next();
        if rows.next().is_some()
            || first
                .as_ref()
                .is_some_and(|row| row.idempotency_key != idempotency_key)
        {
            return Err(AgentUsageError::IdempotencyConflict);
        }
        Ok(first)
    })
}

fn capability_scope_evidence(scope: &CapabilityScope) -> (&'static str, Value) {
    match scope {
        CapabilityScope::TenantWide => ("tenant_wide", Value::Array(Vec::new())),
        CapabilityScope::Resources(resources) => (
            "resources",
            Value::Array(
                resources
                    .values()
                    .iter()
                    .map(|resource| json!({"kind": resource.kind(), "id": resource.id()}))
                    .collect(),
            ),
        ),
    }
}

async fn reservation_definitions_are_current(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    reservation_id: Uuid,
) -> Result<bool, AgentUsageError> {
    sqlx::query_scalar::<_, bool>(
        r#"
        SELECT NOT EXISTS (
            SELECT 1
            FROM agent_limit_reservations AS reservation
            INNER JOIN agent_limit_rules AS rule
              ON rule.tenant_id = reservation.tenant_id
             AND rule.deleted_at IS NULL
             AND rule.effective_from <= STATEMENT_TIMESTAMP()
             AND rule.enforcement = 'hard'
             AND agent_usage_stage_supports_meter(
                    reservation.stage_kind, rule.meter_key
                 )
             AND (
                 rule.scope_kind = 'campus'
                 OR (rule.scope_kind = 'person'
                     AND rule.person_user_id = reservation.actor_user_id)
                 OR (rule.scope_kind = 'role'
                     AND rule.role_key = ANY(reservation.role_keys))
                 OR (rule.scope_kind = 'origin_module'
                     AND rule.origin_module_key = reservation.origin_module_key)
                 OR (rule.scope_kind = 'capability_module'
                     AND rule.capability_module_key = reservation.capability_module_key)
                 OR (rule.scope_kind = 'capability'
                     AND rule.capability_key = reservation.capability_key)
                 OR (rule.scope_kind = 'provider'
                     AND rule.provider_key = reservation.provider_key)
                 OR (rule.scope_kind = 'model'
                     AND rule.provider_key = reservation.provider_key
                     AND rule.provider_model_id = reservation.provider_model_id)
             )
            WHERE reservation.id = $1 AND reservation.tenant_id = $2
              AND NOT EXISTS (
                  SELECT 1 FROM agent_limit_reservation_items AS item
                  WHERE item.reservation_id = reservation.id
                    AND item.tenant_id = reservation.tenant_id
                    AND item.definition_kind = 'local_rule'
                    AND item.campus_rule_id = rule.id
                    AND item.definition_version = rule.version
                    AND item.limit_value = rule.limit_value
                    AND item.meter_key = rule.meter_key
                    AND item.period = rule.period
                    AND item.currency_code IS NOT DISTINCT FROM rule.currency_code
                    AND item.currency_exponent IS NOT DISTINCT FROM rule.currency_exponent
              )
            UNION ALL
            SELECT 1
            FROM agent_limit_reservation_items AS item
            WHERE item.reservation_id = $1 AND item.tenant_id = $2
              AND item.definition_kind = 'local_rule'
              AND NOT EXISTS (
                  SELECT 1 FROM agent_limit_rules AS rule
                  WHERE rule.id = item.campus_rule_id
                    AND rule.tenant_id = item.tenant_id
                    AND rule.deleted_at IS NULL
                    AND rule.effective_from <= STATEMENT_TIMESTAMP()
                    AND rule.enforcement = 'hard'
                    AND rule.version = item.definition_version
                    AND rule.limit_value = item.limit_value
              )
            UNION ALL
            SELECT 1
            FROM agent_limit_reservations AS reservation
            INNER JOIN entitlement_limits AS entitlement
              ON entitlement.tenant_id = reservation.tenant_id
             AND entitlement.enforcement = 'hard'
             AND agent_usage_meter_unit(entitlement.limit_key) IS NOT NULL
             AND agent_usage_meter_unit(entitlement.limit_key) <> 'money'
             AND agent_usage_stage_supports_meter(
                    reservation.stage_kind, entitlement.limit_key
                 )
            WHERE reservation.id = $1 AND reservation.tenant_id = $2
              AND NOT EXISTS (
                  SELECT 1 FROM agent_limit_reservation_items AS item
                  WHERE item.reservation_id = reservation.id
                    AND item.tenant_id = reservation.tenant_id
                    AND item.definition_kind = 'signed_entitlement'
                    AND item.source_lease_id = entitlement.source_lease_id
                    AND item.entitlement_limit_key = entitlement.limit_key
                    AND item.unit = entitlement.unit
                    AND item.period = entitlement.period
                    AND item.limit_value = entitlement.limit_value
              )
            UNION ALL
            SELECT 1
            FROM agent_limit_reservation_items AS item
            WHERE item.reservation_id = $1 AND item.tenant_id = $2
              AND item.definition_kind = 'signed_entitlement'
              AND NOT EXISTS (
                  SELECT 1 FROM entitlement_limits AS entitlement
                  WHERE entitlement.tenant_id = item.tenant_id
                    AND entitlement.enforcement = 'hard'
                    AND entitlement.source_lease_id = item.source_lease_id
                    AND entitlement.limit_key = item.entitlement_limit_key
                    AND entitlement.unit = item.unit
                    AND entitlement.period = item.period
                    AND entitlement.limit_value = item.limit_value
              )
        )
        "#,
    )
    .bind(reservation_id)
    .bind(tenant_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(storage)
}

fn verify_replay(
    existing: &ReservationRow,
    actor_user_id: Uuid,
    role_keys: &[String],
    run: &RunIdentityRow,
    stage: &StageIdentityRow,
    command: &PrepareAgentUsage,
) -> Result<(), AgentUsageError> {
    if existing.run_id != command.run_id
        || existing.actor_user_id != actor_user_id
        || existing.role_keys != role_keys
        || existing.origin_module_key != run.origin_module_key
        || existing.provider_attempt_id != stage.provider_attempt_id
        || existing.capability_call_id != stage.capability_call_id
        || existing.provider_key != stage.provider_key
        || existing.provider_model_id != stage.provider_model_id
        || existing.capability_module_key != stage.capability_module_key
        || existing.capability_key != stage.capability_key
        || existing.stage_kind != stage_kind(&command.stage)
        || existing.stage_sequence != stage.stage_sequence
        || existing.request_fingerprint != command.request_fingerprint
    {
        return Err(AgentUsageError::IdempotencyConflict);
    }
    Ok(())
}

async fn verify_replay_demands(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    reservation_id: Uuid,
    command: &PrepareAgentUsage,
) -> Result<(), AgentUsageError> {
    let stored = sqlx::query(
        r#"
        SELECT meter_key, requested_amount, currency_code, currency_exponent,
               pricing_version
        FROM agent_limit_reservation_items
        WHERE tenant_id = $1 AND reservation_id = $2 AND deleted_at IS NULL
        ORDER BY item_sequence
        "#,
    )
    .bind(tenant_id)
    .bind(reservation_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(storage)?;
    for row in stored {
        let meter = AgentUsageMeter::from_str(row.get::<&str, _>("meter_key"))?;
        let demand = command
            .demands
            .get(&meter)
            .ok_or(AgentUsageError::IdempotencyConflict)?;
        let tuple_matches = demand.money.as_ref().map_or_else(
            || {
                row.get::<Option<String>, _>("currency_code").is_none()
                    && row.get::<Option<i16>, _>("currency_exponent").is_none()
                    && row.get::<Option<String>, _>("pricing_version").is_none()
            },
            |money| {
                row.get::<Option<String>, _>("currency_code").as_deref()
                    == Some(money.currency.as_str())
                    && row.get::<Option<i16>, _>("currency_exponent") == Some(money.exponent)
                    && row.get::<Option<String>, _>("pricing_version").as_deref()
                        == money.pricing_version.as_deref()
            },
        );
        if row.get::<i64, _>("requested_amount") != demand.amount || !tuple_matches {
            return Err(AgentUsageError::IdempotencyConflict);
        }
    }
    Ok(())
}

fn reservation_projection(row: &ReservationRow) -> Result<PreparedAgentUsage, AgentUsageError> {
    Ok(PreparedAgentUsage {
        reservation_id: row.id,
        status: AgentUsageReservationStatus::from_str(&row.status)?,
        expires_at: row.expires_at,
    })
}

async fn matching_definitions(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor_user_id: Uuid,
    role_keys: &[String],
    origin_module_key: &str,
    stage: &StageIdentityRow,
    stage_kind: &str,
) -> Result<Vec<DefinitionRow>, AgentUsageError> {
    sqlx::query_as::<_, DefinitionRow>(
        r#"
        SELECT 'local_rule'::TEXT AS definition_kind,
               rule.id AS campus_rule_id, NULL::UUID AS source_lease_id,
               NULL::TEXT AS entitlement_limit_key, rule.version AS definition_version,
               rule.scope_kind,
               CASE rule.scope_kind
                 WHEN 'campus' THEN rule.tenant_id::TEXT
                 WHEN 'person' THEN rule.person_user_id::TEXT
                 WHEN 'role' THEN rule.role_key
                 WHEN 'origin_module' THEN rule.origin_module_key
                 WHEN 'capability_module' THEN rule.capability_module_key
                 WHEN 'capability' THEN rule.capability_key
                 WHEN 'provider' THEN rule.provider_key
                 WHEN 'model' THEN rule.provider_model_id
               END AS scope_value,
               rule.meter_key, agent_usage_meter_unit(rule.meter_key) AS unit,
               rule.currency_code, rule.currency_exponent, rule.period,
               rule.limit_value
        FROM agent_limit_rules AS rule
        WHERE rule.tenant_id = $1 AND rule.deleted_at IS NULL
          AND rule.effective_from <= STATEMENT_TIMESTAMP()
          AND rule.enforcement = 'hard'
          AND agent_usage_stage_supports_meter($8, rule.meter_key)
          AND (
              rule.scope_kind = 'campus'
              OR (rule.scope_kind = 'person' AND rule.person_user_id = $2)
              OR (rule.scope_kind = 'role' AND rule.role_key = ANY($3))
              OR (rule.scope_kind = 'origin_module' AND rule.origin_module_key = $4)
              OR (rule.scope_kind = 'capability_module' AND rule.capability_module_key = $5)
              OR (rule.scope_kind = 'capability' AND rule.capability_key = $6)
              OR (rule.scope_kind = 'provider' AND rule.provider_key = $7)
              OR (rule.scope_kind = 'model' AND rule.provider_key = $7
                   AND rule.provider_model_id = $9)
          )
        UNION ALL
        SELECT 'signed_entitlement'::TEXT, NULL::UUID, entitlement.source_lease_id,
               entitlement.limit_key, NULL::BIGINT, 'campus'::TEXT, $1::TEXT,
               entitlement.limit_key, entitlement.unit, NULL::TEXT, NULL::SMALLINT,
               entitlement.period, entitlement.limit_value
        FROM entitlement_limits AS entitlement
        WHERE entitlement.tenant_id = $1 AND entitlement.enforcement = 'hard'
          AND agent_usage_meter_unit(entitlement.limit_key) IS NOT NULL
          AND agent_usage_meter_unit(entitlement.limit_key) <> 'money'
          AND agent_usage_stage_supports_meter($8, entitlement.limit_key)
        ORDER BY definition_kind, meter_key, scope_kind, scope_value
        "#,
    )
    .bind(tenant_id)
    .bind(actor_user_id)
    .bind(role_keys)
    .bind(origin_module_key)
    .bind(&stage.capability_module_key)
    .bind(&stage.capability_key)
    .bind(&stage.provider_key)
    .bind(stage_kind)
    .bind(&stage.provider_model_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(storage)
}

fn verify_money_tuple(
    definition: &DefinitionRow,
    demand: &AgentUsageDemand,
) -> Result<(), AgentUsageError> {
    match (
        &definition.currency_code,
        definition.currency_exponent,
        &demand.money,
    ) {
        (None, None, None) => Ok(()),
        (Some(code), Some(exponent), Some(money))
            if code == &money.currency && exponent == money.exponent =>
        {
            Ok(())
        }
        _ => Err(AgentUsageError::CurrencyMismatch),
    }
}

async fn lock_definition_bucket(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    definition: &DefinitionRow,
) -> Result<BucketRow, AgentUsageError> {
    if definition.definition_kind == "local_rule" {
        let rule_id = definition
            .campus_rule_id
            .ok_or_else(AgentUsageError::storage_contract)?;
        sqlx::query(
            r#"
            INSERT INTO agent_limit_buckets (
                tenant_id, campus_rule_id, meter_key, currency_code,
                currency_exponent, period, period_start, period_end
            )
            SELECT $1, $2, $3, $4, $5, $6,
                   CASE $6 WHEN 'none' THEN 'epoch'::TIMESTAMPTZ
                           WHEN 'day' THEN DATE_TRUNC('day', STATEMENT_TIMESTAMP(), 'UTC')
                           WHEN 'month' THEN DATE_TRUNC('month', STATEMENT_TIMESTAMP(), 'UTC')
                           WHEN 'year' THEN DATE_TRUNC('year', STATEMENT_TIMESTAMP(), 'UTC') END,
                   CASE $6 WHEN 'none' THEN NULL
                           WHEN 'day' THEN DATE_TRUNC('day', STATEMENT_TIMESTAMP(), 'UTC') + INTERVAL '1 day'
                           WHEN 'month' THEN DATE_TRUNC('month', STATEMENT_TIMESTAMP(), 'UTC') + INTERVAL '1 month'
                           WHEN 'year' THEN DATE_TRUNC('year', STATEMENT_TIMESTAMP(), 'UTC') + INTERVAL '1 year' END
            ON CONFLICT (tenant_id, campus_rule_id, period_start)
              WHERE deleted_at IS NULL DO NOTHING
            "#,
        )
        .bind(tenant_id)
        .bind(rule_id)
        .bind(&definition.meter_key)
        .bind(&definition.currency_code)
        .bind(definition.currency_exponent)
        .bind(&definition.period)
        .execute(&mut **transaction)
        .await
        .map_err(storage)?;
        sqlx::query_as::<_, BucketRow>(
            r#"
            SELECT id, period_start, period_end, committed_value, reserved_value
            FROM agent_limit_buckets
            WHERE tenant_id = $1 AND campus_rule_id = $2 AND deleted_at IS NULL
              AND period_start = CASE $3
                    WHEN 'none' THEN 'epoch'::TIMESTAMPTZ
                    WHEN 'day' THEN DATE_TRUNC('day', STATEMENT_TIMESTAMP(), 'UTC')
                    WHEN 'month' THEN DATE_TRUNC('month', STATEMENT_TIMESTAMP(), 'UTC')
                    WHEN 'year' THEN DATE_TRUNC('year', STATEMENT_TIMESTAMP(), 'UTC') END
            FOR UPDATE
            "#,
        )
        .bind(tenant_id)
        .bind(rule_id)
        .bind(&definition.period)
        .fetch_one(&mut **transaction)
        .await
        .map_err(storage)
    } else {
        let limit_key = definition
            .entitlement_limit_key
            .as_deref()
            .ok_or_else(AgentUsageError::storage_contract)?;
        sqlx::query(
            r#"
            INSERT INTO entitlement_meter_buckets (
                tenant_id, limit_key, period_start, period_end
            )
            SELECT $1, $2,
                   CASE $3 WHEN 'none' THEN 'epoch'::TIMESTAMPTZ
                           WHEN 'day' THEN DATE_TRUNC('day', STATEMENT_TIMESTAMP(), 'UTC')
                           WHEN 'month' THEN DATE_TRUNC('month', STATEMENT_TIMESTAMP(), 'UTC')
                           WHEN 'year' THEN DATE_TRUNC('year', STATEMENT_TIMESTAMP(), 'UTC') END,
                   CASE $3 WHEN 'none' THEN NULL
                           WHEN 'day' THEN DATE_TRUNC('day', STATEMENT_TIMESTAMP(), 'UTC') + INTERVAL '1 day'
                           WHEN 'month' THEN DATE_TRUNC('month', STATEMENT_TIMESTAMP(), 'UTC') + INTERVAL '1 month'
                           WHEN 'year' THEN DATE_TRUNC('year', STATEMENT_TIMESTAMP(), 'UTC') + INTERVAL '1 year' END
            ON CONFLICT (tenant_id, limit_key, period_start) DO NOTHING
            "#,
        )
        .bind(tenant_id)
        .bind(limit_key)
        .bind(&definition.period)
        .execute(&mut **transaction)
        .await
        .map_err(storage)?;
        sqlx::query_as::<_, BucketRow>(
            r#"
            SELECT id, period_start, period_end, committed_value, reserved_value
            FROM entitlement_meter_buckets
            WHERE tenant_id = $1 AND limit_key = $2 AND deleted_at IS NULL
              AND period_start = CASE $3
                    WHEN 'none' THEN 'epoch'::TIMESTAMPTZ
                    WHEN 'day' THEN DATE_TRUNC('day', STATEMENT_TIMESTAMP(), 'UTC')
                    WHEN 'month' THEN DATE_TRUNC('month', STATEMENT_TIMESTAMP(), 'UTC')
                    WHEN 'year' THEN DATE_TRUNC('year', STATEMENT_TIMESTAMP(), 'UTC') END
            FOR UPDATE
            "#,
        )
        .bind(tenant_id)
        .bind(limit_key)
        .bind(&definition.period)
        .fetch_one(&mut **transaction)
        .await
        .map_err(storage)
    }
}

#[allow(clippy::too_many_arguments)]
async fn reserve_all(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor_user_id: Uuid,
    agent_reservation_id: Uuid,
    agent_idempotency_key: &str,
    expires_at: DateTime<Utc>,
    prepared: &mut [PreparedDefinition],
) -> Result<(), AgentUsageError> {
    for (index, item) in prepared.iter_mut().enumerate() {
        if item.definition.definition_kind == "local_rule" {
            sqlx::query(
                r#"
                UPDATE agent_limit_buckets
                SET reserved_value = reserved_value + $3,
                    updated_at = STATEMENT_TIMESTAMP()
                WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
                "#,
            )
            .bind(item.bucket_id)
            .bind(tenant_id)
            .bind(item.demand.amount)
            .execute(&mut **transaction)
            .await
            .map_err(storage)?;
        } else {
            let canonical_id = Uuid::new_v4();
            let source_lease_id = item
                .definition
                .source_lease_id
                .ok_or_else(AgentUsageError::storage_contract)?;
            let limit_key = item
                .definition
                .entitlement_limit_key
                .as_deref()
                .ok_or_else(AgentUsageError::storage_contract)?;
            let canonical_key = format!("agent:{agent_reservation_id}:{index}");
            sqlx::query(
                r#"
                INSERT INTO entitlement_usage_reservations (
                    id, tenant_id, bucket_id, source_lease_id, limit_key,
                    unit, operation_key, actor_user_id, idempotency_key,
                    amount, expires_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                "#,
            )
            .bind(canonical_id)
            .bind(tenant_id)
            .bind(item.bucket_id)
            .bind(source_lease_id)
            .bind(limit_key)
            .bind(&item.definition.unit)
            .bind("agent.runtime")
            .bind(actor_user_id)
            .bind(canonical_key)
            .bind(item.demand.amount)
            .bind(expires_at)
            .execute(&mut **transaction)
            .await
            .map_err(storage)?;
            sqlx::query(
                r#"
                UPDATE entitlement_meter_buckets
                SET reserved_value = reserved_value + $3,
                    updated_at = STATEMENT_TIMESTAMP()
                WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
                "#,
            )
            .bind(item.bucket_id)
            .bind(tenant_id)
            .bind(item.demand.amount)
            .execute(&mut **transaction)
            .await
            .map_err(storage)?;
            item.entitlement_reservation_id = Some(canonical_id);
        }
    }
    let _ = agent_idempotency_key;
    Ok(())
}

async fn insert_prepared_items(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
    reservation_id: Uuid,
    denied: bool,
    items: &[PreparedDefinition],
) -> Result<(), AgentUsageError> {
    for (index, item) in items.iter().enumerate() {
        let is_signed = item.definition.definition_kind == "signed_entitlement";
        sqlx::query(
            r#"
            INSERT INTO agent_limit_reservation_items (
                tenant_id, reservation_id, run_id, item_sequence,
                bucket_id, entitlement_bucket_id, entitlement_reservation_id,
                definition_kind, campus_rule_id, source_lease_id,
                entitlement_limit_key, definition_version, scope_kind,
                scope_value, meter_key, unit, currency_code, currency_exponent,
                pricing_version, period, period_start, period_end, limit_value,
                committed_before, reserved_before, requested_amount,
                reserved_amount, decision
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                $13, $14, $15, $16, $17, $18, $19, $20, $21, $22,
                $23, $24, $25, $26, $27, $28
            )
            "#,
        )
        .bind(tenant_id)
        .bind(reservation_id)
        .bind(run_id)
        .bind(i16::try_from(index + 1).map_err(|_| AgentUsageError::storage_contract())?)
        .bind((!is_signed).then_some(item.bucket_id))
        .bind(is_signed.then_some(item.bucket_id))
        .bind(item.entitlement_reservation_id)
        .bind(&item.definition.definition_kind)
        .bind(item.definition.campus_rule_id)
        .bind(item.definition.source_lease_id)
        .bind(&item.definition.entitlement_limit_key)
        .bind(item.definition.definition_version)
        .bind(&item.definition.scope_kind)
        .bind(&item.definition.scope_value)
        .bind(&item.definition.meter_key)
        .bind(&item.definition.unit)
        .bind(&item.definition.currency_code)
        .bind(item.definition.currency_exponent)
        .bind(
            item.demand
                .money
                .as_ref()
                .and_then(|money| money.pricing_version.as_deref()),
        )
        .bind(&item.definition.period)
        .bind(item.period_start)
        .bind(item.period_end)
        .bind(item.definition.limit_value)
        .bind(item.committed_before)
        .bind(item.reserved_before)
        .bind(item.demand.amount)
        .bind(if denied { 0 } else { item.demand.amount })
        .bind(if item.allowed { "allowed" } else { "denied" })
        .execute(&mut **transaction)
        .await
        .map_err(storage)?;
    }
    Ok(())
}

async fn lock_reservation(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    reservation_id: Uuid,
) -> Result<ReservationRow, AgentUsageError> {
    sqlx::query_as::<_, ReservationRow>(
        r#"
        SELECT id, run_id, provider_attempt_id, capability_call_id,
               actor_user_id, role_keys, origin_module_key,
               capability_module_key, capability_key, provider_key,
               provider_model_id, stage_kind, stage_sequence, idempotency_key,
               request_fingerprint, status, expires_at, claimed_at
        FROM agent_limit_reservations
        WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
        FOR UPDATE
        "#,
    )
    .bind(reservation_id)
    .bind(tenant_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(storage)?
    .ok_or(AgentUsageError::NotFound)
}

async fn lifecycle_items(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    reservation_id: Uuid,
) -> Result<Vec<ItemLifecycleRow>, AgentUsageError> {
    sqlx::query_as::<_, ItemLifecycleRow>(
        r#"
        SELECT id, bucket_id, entitlement_bucket_id, entitlement_reservation_id,
               reserved_amount, meter_key, period_start,
               period_end, source_lease_id, entitlement_limit_key, unit,
               currency_code, currency_exponent, pricing_version
        FROM agent_limit_reservation_items
        WHERE tenant_id = $1 AND reservation_id = $2 AND deleted_at IS NULL
        ORDER BY item_sequence
        "#,
    )
    .bind(tenant_id)
    .bind(reservation_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(storage)
}

async fn verify_committed_projection(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    reservation: &ReservationRow,
) -> Result<(), AgentUsageError> {
    let valid = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT
          EXISTS (
            SELECT 1 FROM agent_usage_events AS event
            WHERE event.tenant_id = $1
              AND event.limit_reservation_id = $2
              AND event.run_id = $3
          )
          AND NOT EXISTS (
            SELECT 1
            FROM agent_limit_reservation_items AS item
            WHERE item.tenant_id = $1 AND item.reservation_id = $2
              AND NOT EXISTS (
                SELECT 1 FROM agent_limit_reconciliations AS reconciliation
                WHERE reconciliation.tenant_id = item.tenant_id
                  AND reconciliation.reservation_id = item.reservation_id
                  AND reconciliation.run_id = item.run_id
                  AND reconciliation.reservation_item_id = item.id
              )
          )
          AND NOT EXISTS (
            SELECT 1
            FROM agent_limit_reservation_items AS item
            INNER JOIN agent_limit_reconciliations AS reconciliation
              ON reconciliation.tenant_id = item.tenant_id
             AND reconciliation.reservation_id = item.reservation_id
             AND reconciliation.run_id = item.run_id
             AND reconciliation.reservation_item_id = item.id
            LEFT JOIN entitlement_usage_reservations AS source_reservation
              ON source_reservation.id = item.entitlement_reservation_id
             AND source_reservation.tenant_id = item.tenant_id
             AND source_reservation.bucket_id = item.entitlement_bucket_id
            LEFT JOIN entitlement_usage_events AS source_event
              ON source_event.reservation_id = source_reservation.id
             AND source_event.tenant_id = source_reservation.tenant_id
            WHERE item.tenant_id = $1 AND item.reservation_id = $2
              AND item.definition_kind = 'signed_entitlement'
              AND (
                source_reservation.amount IS DISTINCT FROM item.reserved_amount
                OR (
                  reconciliation.committed_amount > 0
                  AND (
                    source_reservation.status IS DISTINCT FROM 'committed'
                    OR source_event.amount IS DISTINCT FROM reconciliation.committed_amount
                  )
                )
                OR (
                  reconciliation.committed_amount = 0
                  AND (
                    source_reservation.status IS DISTINCT FROM 'released'
                    OR source_event.id IS NOT NULL
                  )
                )
              )
          )
          AND (
            SELECT COUNT(*)
            FROM agent_usage_measures AS measure
            INNER JOIN agent_usage_events AS event
              ON event.id = measure.usage_event_id
             AND event.tenant_id = measure.tenant_id
            WHERE event.tenant_id = $1 AND event.limit_reservation_id = $2
          ) = CASE WHEN $4 = 'provider_attempt' THEN 7 ELSE 1 END
        "#,
    )
    .bind(tenant_id)
    .bind(reservation.id)
    .bind(reservation.run_id)
    .bind(&reservation.stage_kind)
    .fetch_one(&mut **transaction)
    .await
    .map_err(storage)?;
    if valid {
        Ok(())
    } else {
        Err(AgentUsageError::storage_contract())
    }
}

async fn transition_locked_reservation(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    reservation: &ReservationRow,
    target: &str,
) -> Result<(), AgentUsageError> {
    for item in lifecycle_items(transaction, tenant_id, reservation.id).await? {
        if item.reserved_amount == 0 {
            continue;
        }
        if let Some(bucket_id) = item.bucket_id {
            sqlx::query(
                r#"
                UPDATE agent_limit_buckets
                SET reserved_value = reserved_value - $3,
                    updated_at = STATEMENT_TIMESTAMP()
                WHERE id = $1 AND tenant_id = $2
                  AND reserved_value >= $3 AND deleted_at IS NULL
                "#,
            )
            .bind(bucket_id)
            .bind(tenant_id)
            .bind(item.reserved_amount)
            .execute(&mut **transaction)
            .await
            .map_err(storage)?;
        } else if let (Some(bucket_id), Some(source_id)) =
            (item.entitlement_bucket_id, item.entitlement_reservation_id)
        {
            sqlx::query(
                r#"
                UPDATE entitlement_usage_reservations
                SET status = $3, released_at = STATEMENT_TIMESTAMP(),
                    updated_at = STATEMENT_TIMESTAMP()
                WHERE id = $1 AND tenant_id = $2 AND status = 'reserved'
                "#,
            )
            .bind(source_id)
            .bind(tenant_id)
            .bind(target)
            .execute(&mut **transaction)
            .await
            .map_err(storage)?;
            sqlx::query(
                r#"
                UPDATE entitlement_meter_buckets
                SET reserved_value = reserved_value - $3,
                    updated_at = STATEMENT_TIMESTAMP()
                WHERE id = $1 AND tenant_id = $2
                  AND reserved_value >= $3 AND deleted_at IS NULL
                "#,
            )
            .bind(bucket_id)
            .bind(tenant_id)
            .bind(item.reserved_amount)
            .execute(&mut **transaction)
            .await
            .map_err(storage)?;
        } else {
            return Err(AgentUsageError::storage_contract());
        }
    }
    sqlx::query(
        r#"
        UPDATE agent_limit_reservations
        SET status = $3, released_at = STATEMENT_TIMESTAMP(),
            updated_at = STATEMENT_TIMESTAMP()
        WHERE id = $1 AND tenant_id = $2 AND status = 'reserved'
        "#,
    )
    .bind(reservation.id)
    .bind(tenant_id)
    .bind(target)
    .execute(&mut **transaction)
    .await
    .map_err(storage)?;
    Ok(())
}

async fn expire_stale_for_tenant(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
) -> Result<(), AgentUsageError> {
    let stale = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id FROM agent_limit_reservations
        WHERE tenant_id = $1 AND status = 'reserved'
          AND expires_at <= STATEMENT_TIMESTAMP() AND deleted_at IS NULL
          AND claimed_at IS NULL
        ORDER BY expires_at, id
        FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(storage)?;
    for reservation_id in stale {
        let reservation = lock_reservation(transaction, tenant_id, reservation_id).await?;
        transition_locked_reservation(transaction, tenant_id, &reservation, "expired").await?;
    }
    Ok(())
}

async fn commit_locked_reservation(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    reservation: &ReservationRow,
    source: &UsageEventSourceRow,
) -> Result<(), AgentUsageError> {
    let items = lifecycle_items(transaction, tenant_id, reservation.id).await?;
    let mut agreed = std::collections::HashMap::<
        (String, Option<String>, Option<i16>, Option<String>),
        ReconciledAmount,
    >::new();
    for item in items {
        if item.reserved_amount == 0 {
            continue;
        }
        let reconciled = reconcile_item(&item, source)?;
        let agreement_key = (
            item_meter_key(&item)?.to_owned(),
            item.currency_code.clone(),
            item.currency_exponent,
            item.pricing_version.clone(),
        );
        if agreed
            .insert(agreement_key, reconciled.clone())
            .is_some_and(|existing| existing != reconciled)
        {
            return Err(AgentUsageError::storage_contract());
        }
        if let Some(bucket_id) = item.bucket_id {
            sqlx::query(
                r#"
                UPDATE agent_limit_buckets
                SET reserved_value = reserved_value - $3,
                    committed_value = committed_value + $4,
                    updated_at = STATEMENT_TIMESTAMP()
                WHERE id = $1 AND tenant_id = $2
                  AND reserved_value >= $3 AND deleted_at IS NULL
                "#,
            )
            .bind(bucket_id)
            .bind(tenant_id)
            .bind(item.reserved_amount)
            .bind(reconciled.amount)
            .execute(&mut **transaction)
            .await
            .map_err(storage)?;
        } else if let (Some(bucket_id), Some(source_id), Some(source_lease_id), Some(limit_key)) = (
            item.entitlement_bucket_id,
            item.entitlement_reservation_id,
            item.source_lease_id,
            item.entitlement_limit_key.as_deref(),
        ) {
            if reconciled.amount > 0 {
                sqlx::query(
                    r#"
                    UPDATE entitlement_usage_reservations
                    SET status = 'committed', committed_at = STATEMENT_TIMESTAMP(),
                        updated_at = STATEMENT_TIMESTAMP()
                    WHERE id = $1 AND tenant_id = $2 AND status = 'reserved'
                    "#,
                )
                .bind(source_id)
                .bind(tenant_id)
                .execute(&mut **transaction)
                .await
                .map_err(storage)?;
            } else {
                sqlx::query(
                    r#"
                    UPDATE entitlement_usage_reservations
                    SET status = 'released', released_at = STATEMENT_TIMESTAMP(),
                        updated_at = STATEMENT_TIMESTAMP()
                    WHERE id = $1 AND tenant_id = $2 AND status = 'reserved'
                    "#,
                )
                .bind(source_id)
                .bind(tenant_id)
                .execute(&mut **transaction)
                .await
                .map_err(storage)?;
            }
            sqlx::query(
                r#"
                UPDATE entitlement_meter_buckets
                SET reserved_value = reserved_value - $3,
                    committed_value = committed_value + $4,
                    updated_at = STATEMENT_TIMESTAMP()
                WHERE id = $1 AND tenant_id = $2
                  AND reserved_value >= $3 AND deleted_at IS NULL
                "#,
            )
            .bind(bucket_id)
            .bind(tenant_id)
            .bind(item.reserved_amount)
            .bind(reconciled.amount)
            .execute(&mut **transaction)
            .await
            .map_err(storage)?;
            if reconciled.amount > 0 {
                sqlx::query(
                    r#"
                    INSERT INTO entitlement_usage_events (
                        tenant_id, reservation_id, source_lease_id, limit_key,
                        unit, operation_key, actor_user_id, amount, period_start,
                        period_end, occurred_at
                    )
                    VALUES ($1, $2, $3, $4, $5, 'agent.runtime', $6, $7, $8, $9,
                            STATEMENT_TIMESTAMP())
                    "#,
                )
                .bind(tenant_id)
                .bind(source_id)
                .bind(source_lease_id)
                .bind(limit_key)
                .bind(&item.unit)
                .bind(reservation.actor_user_id)
                .bind(reconciled.amount)
                .bind(item.period_start)
                .bind(item.period_end)
                .execute(&mut **transaction)
                .await
                .map_err(storage)?;
            }
        } else {
            return Err(AgentUsageError::storage_contract());
        }
        sqlx::query(
            r#"
            INSERT INTO agent_limit_reconciliations (
                tenant_id, reservation_id, run_id, reservation_item_id,
                committed_amount, enforcement_basis
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(tenant_id)
        .bind(reservation.id)
        .bind(reservation.run_id)
        .bind(item.id)
        .bind(reconciled.amount)
        .bind(reconciled.basis)
        .execute(&mut **transaction)
        .await
        .map_err(storage)?;
    }
    sqlx::query(
        r#"
        UPDATE agent_limit_reservations
        SET status = 'committed', committed_at = STATEMENT_TIMESTAMP(),
            updated_at = STATEMENT_TIMESTAMP()
        WHERE id = $1 AND tenant_id = $2 AND status = 'reserved'
        "#,
    )
    .bind(reservation.id)
    .bind(tenant_id)
    .execute(&mut **transaction)
    .await
    .map_err(storage)?;
    Ok(())
}

fn item_meter_key(item: &ItemLifecycleRow) -> Result<&str, AgentUsageError> {
    if item.meter_key.is_empty() {
        Err(AgentUsageError::storage_contract())
    } else {
        Ok(&item.meter_key)
    }
}

fn reconcile_item(
    item: &ItemLifecycleRow,
    source: &UsageEventSourceRow,
) -> Result<ReconciledAmount, AgentUsageError> {
    let meter = AgentUsageMeter::from_str(item_meter_key(item)?)?;
    let (amount, basis) = match meter {
        AgentUsageMeter::Runs
        | AgentUsageMeter::ProviderAttempts
        | AgentUsageMeter::CapabilityCalls => (Some(1), "exact"),
        AgentUsageMeter::InputTokens => (source.input_tokens, "exact"),
        AgentUsageMeter::OutputTokens => (source.output_tokens, "exact"),
        AgentUsageMeter::EstimatedCost => (source.estimated_cost_amount, "estimated"),
        AgentUsageMeter::CachedInputTokens
        | AgentUsageMeter::ReasoningTokens
        | AgentUsageMeter::ProviderReportedCost => {
            return Err(AgentUsageError::storage_contract());
        }
    };
    let (amount, basis) = match amount {
        Some(amount) => (amount, basis),
        None if source.failure_origin.as_deref() == Some("preflight") => (0, basis),
        None => (item.reserved_amount, "upper_bound"),
    };
    if amount > item.reserved_amount {
        return Err(AgentUsageError::InvalidTransition);
    }
    Ok(ReconciledAmount { amount, basis })
}

async fn append_terminal_usage_event(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    reservation: &ReservationRow,
) -> Result<(), AgentUsageError> {
    let source = load_usage_source(transaction, tenant_id, reservation).await?;
    let denied_by_limit = reservation.status == "denied";
    let outcome = terminal_usage_outcome(&reservation.status, &source.outcome);
    let safe_failure_code =
        terminal_usage_failure_code(&reservation.status, source.safe_failure_code.as_deref());
    let event_id = Uuid::new_v4();
    let inserted = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO agent_usage_events (
            id, tenant_id, event_kind, run_id, thread_id, actor_user_id,
            role_keys, origin_module_key, task_class, provider_attempt_id,
            provider_turn_index, provider_attempt_index, provider_connection_id,
            provider_key, provider_model_id, provider_model_snapshot_id,
            route_priority, failure_origin, failure_category, capability_call_id,
            capability_module_key, capability_key, capability_version,
            approval_state, outcome, safe_failure_code, duration_ms, request_id,
            correlation_id, limit_reservation_id, occurred_at
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
            $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24,
            $25, $26, $27, $28, $29, $30, $31
        )
        ON CONFLICT (limit_reservation_id) WHERE limit_reservation_id IS NOT NULL
        DO NOTHING
        RETURNING id
        "#,
    )
    .bind(event_id)
    .bind(tenant_id)
    .bind(&source.event_kind)
    .bind(source.run_id)
    .bind(source.thread_id)
    .bind(source.actor_user_id)
    .bind(&source.role_keys)
    .bind(&source.origin_module_key)
    .bind(&source.task_class)
    .bind(source.provider_attempt_id)
    .bind(source.provider_turn_index)
    .bind(source.provider_attempt_index)
    .bind(source.provider_connection_id)
    .bind(&source.provider_key)
    .bind(&source.provider_model_id)
    .bind(source.provider_model_snapshot_id)
    .bind(source.route_priority)
    .bind(&source.failure_origin)
    .bind(&source.failure_category)
    .bind(source.capability_call_id)
    .bind(&source.capability_module_key)
    .bind(&source.capability_key)
    .bind(source.capability_version)
    .bind(&source.approval_state)
    .bind(outcome)
    .bind(safe_failure_code)
    .bind(source.duration_ms)
    .bind(source.request_id)
    .bind(source.correlation_id)
    .bind(reservation.id)
    .bind(source.occurred_at)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(storage)?;
    if let Some(event_id) = inserted {
        if denied_by_limit {
            let audit = NewAuditEvent::new(
                tenant_id,
                AuditActor::agent(source.actor_user_id),
                USAGE_DENIED_ACTION,
                AuditOutcome::Denied,
                RequestContext::from_ids(source.request_id, source.correlation_id),
            )
            .with_agent_run_id(source.run_id)
            .with_target(AuditTarget::new(
                "agent_limit_reservation",
                reservation.id.to_string(),
            ))
            .with_reason("hard_limit_denied");
            cp_audit::append(&mut **transaction, &audit)
                .await
                .map_err(storage)?;
        }
        insert_usage_measures(transaction, tenant_id, event_id, reservation.id, &source).await?;
    }
    Ok(())
}

fn reserved_source_is_committable(
    stage_kind: &str,
    claimed: bool,
    failure_origin: Option<&str>,
) -> bool {
    stage_kind == "run" || claimed || failure_origin == Some("preflight")
}

fn terminal_usage_outcome<'a>(reservation_status: &str, source_outcome: &'a str) -> &'a str {
    if reservation_status == "denied" {
        "denied"
    } else {
        source_outcome
    }
}

fn terminal_usage_failure_code<'a>(
    reservation_status: &str,
    source_failure_code: Option<&'a str>,
) -> Option<&'a str> {
    if reservation_status == "denied" {
        Some("hard_limit_denied")
    } else {
        source_failure_code
    }
}

async fn load_usage_source(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    reservation: &ReservationRow,
) -> Result<UsageEventSourceRow, AgentUsageError> {
    sqlx::query_as::<_, UsageEventSourceRow>(
        r#"
        SELECT run.thread_id, run.requested_by AS actor_user_id, $3::TEXT[] AS role_keys,
               run.origin_module_key, run.task_class, run.request_id,
               run.correlation_id, $4::TEXT AS event_kind, run.id AS run_id,
               attempt.id AS provider_attempt_id,
               attempt.turn_index AS provider_turn_index,
               attempt.attempt_index AS provider_attempt_index,
               attempt.connection_id AS provider_connection_id,
               attempt.provider_key, attempt.provider_model_id,
               attempt.model_snapshot_id AS provider_model_snapshot_id,
               route.priority AS route_priority, attempt.failure_origin,
               attempt.failure_category, call.id AS capability_call_id,
               call.owning_module_key AS capability_module_key,
               call.capability_key, call.capability_version,
               call.approval_state,
               CASE $4
                 WHEN 'run' THEN CASE run.status WHEN 'completed' THEN 'succeeded'
                                      WHEN 'cancelled' THEN 'cancelled'
                                      WHEN 'interrupted' THEN 'interrupted'
                                      ELSE 'failed' END
                 WHEN 'provider_attempt' THEN CASE attempt.status WHEN 'succeeded' THEN 'succeeded'
                                      WHEN 'cancelled' THEN 'cancelled'
                                      WHEN 'interrupted' THEN 'interrupted'
                                      ELSE 'failed' END
                 ELSE CASE call.status WHEN 'succeeded' THEN 'succeeded'
                                      WHEN 'denied' THEN 'denied'
                                      WHEN 'cancelled' THEN 'cancelled'
                                      WHEN 'interrupted' THEN 'interrupted'
                                      ELSE 'failed' END
               END AS outcome,
               CASE $4 WHEN 'run' THEN run.safe_failure_code
                       WHEN 'capability_call' THEN call.safe_failure_code
                       WHEN 'provider_attempt' THEN
                         CASE WHEN attempt.status IN ('failed', 'interrupted')
                              THEN COALESCE(attempt.failure_category, 'provider_interrupted') END
               END AS safe_failure_code,
               CASE $4 WHEN 'run' THEN
                    GREATEST(0, FLOOR(EXTRACT(EPOCH FROM (run.finished_at - COALESCE(run.started_at, run.created_at))) * 1000))::BIGINT
                 WHEN 'provider_attempt' THEN
                    GREATEST(0, FLOOR(EXTRACT(EPOCH FROM (attempt.finished_at - attempt.started_at)) * 1000))::BIGINT
                 ELSE call.duration_ms END AS duration_ms,
               CASE $4 WHEN 'run' THEN run.finished_at
                       WHEN 'provider_attempt' THEN attempt.finished_at
                       ELSE call.finished_at END AS occurred_at,
               attempt.input_tokens, attempt.output_tokens, attempt.cached_tokens,
               attempt.reasoning_tokens, attempt.provider_reported_cost_amount,
               attempt.provider_reported_cost_currency,
               attempt.provider_reported_cost_exponent,
               attempt.provider_reported_pricing_version,
               attempt.estimated_cost_amount, attempt.estimated_cost_currency,
               attempt.estimated_cost_exponent, attempt.estimated_pricing_version
        FROM agent_runs AS run
        LEFT JOIN agent_provider_attempts AS attempt
          ON attempt.id = $5 AND attempt.tenant_id = run.tenant_id
         AND attempt.run_id = run.id
        LEFT JOIN ai_task_routes AS route
          ON route.id = attempt.route_target_id AND route.tenant_id = attempt.tenant_id
        LEFT JOIN agent_capability_calls AS call
          ON call.id = $6 AND call.tenant_id = run.tenant_id
         AND call.run_id = run.id
        WHERE run.id = $1 AND run.tenant_id = $2
          AND (($4 = 'run' AND run.status IN ('completed','failed','cancelled','interrupted'))
            OR ($4 = 'provider_attempt' AND attempt.status <> 'running')
            OR ($4 = 'capability_call' AND call.status <> 'running'))
        "#,
    )
    .bind(reservation.run_id)
    .bind(tenant_id)
    .bind(&reservation.role_keys)
    .bind(&reservation.stage_kind)
    .bind(reservation.provider_attempt_id)
    .bind(reservation.capability_call_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(storage)?
    .ok_or(AgentUsageError::InvalidTransition)
}

async fn insert_usage_measures(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    event_id: Uuid,
    reservation_id: Uuid,
    source: &UsageEventSourceRow,
) -> Result<(), AgentUsageError> {
    let enforcement = sqlx::query(
        r#"
        SELECT item.meter_key, reconciliation.committed_amount,
               reconciliation.enforcement_basis
        FROM agent_limit_reconciliations AS reconciliation
        INNER JOIN agent_limit_reservation_items AS item
          ON item.id = reconciliation.reservation_item_id
         AND item.tenant_id = reconciliation.tenant_id
         AND item.reservation_id = reconciliation.reservation_id
         AND item.run_id = reconciliation.run_id
        WHERE reconciliation.tenant_id = $1
          AND reconciliation.reservation_id = $2
        "#,
    )
    .bind(tenant_id)
    .bind(reservation_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(storage)?
    .into_iter()
    .map(|row| {
        (
            row.get::<String, _>("meter_key"),
            (
                row.get::<i64, _>("committed_amount"),
                row.get::<String, _>("enforcement_basis"),
            ),
        )
    })
    .collect::<std::collections::HashMap<_, _>>();
    let mut measures = Vec::new();
    match source.event_kind.as_str() {
        "run" => measures.push((AgentUsageMeter::Runs, Some(1), None, None, None)),
        "capability_call" => {
            measures.push((AgentUsageMeter::CapabilityCalls, Some(1), None, None, None));
        }
        "provider_attempt" => {
            measures.extend([
                (AgentUsageMeter::ProviderAttempts, Some(1), None, None, None),
                (
                    AgentUsageMeter::InputTokens,
                    source.input_tokens,
                    None,
                    None,
                    None,
                ),
                (
                    AgentUsageMeter::OutputTokens,
                    source.output_tokens,
                    None,
                    None,
                    None,
                ),
                (
                    AgentUsageMeter::CachedInputTokens,
                    source.cached_tokens,
                    None,
                    None,
                    None,
                ),
                (
                    AgentUsageMeter::ReasoningTokens,
                    source.reasoning_tokens,
                    None,
                    None,
                    None,
                ),
                (
                    AgentUsageMeter::ProviderReportedCost,
                    source.provider_reported_cost_amount,
                    source.provider_reported_cost_currency.clone(),
                    source.provider_reported_cost_exponent,
                    source.provider_reported_pricing_version.clone(),
                ),
                (
                    AgentUsageMeter::EstimatedCost,
                    source.estimated_cost_amount,
                    source.estimated_cost_currency.clone(),
                    source.estimated_cost_exponent,
                    source.estimated_pricing_version.clone(),
                ),
            ]);
        }
        _ => return Err(AgentUsageError::storage_contract()),
    }
    for (meter, amount, currency, exponent, pricing) in measures {
        let (enforcement_amount, basis) = enforcement
            .get(meter.as_str())
            .map_or((None, None), |(amount, basis)| {
                (Some(*amount), Some(basis.as_str()))
            });
        let (currency, exponent, pricing) = if meter == AgentUsageMeter::EstimatedCost
            && amount.is_none()
            && enforcement_amount.is_some()
        {
            let tuple = sqlx::query(
                r#"
                SELECT currency_code, currency_exponent, pricing_version
                FROM agent_limit_reservation_items
                WHERE tenant_id = $1 AND reservation_id = $2 AND meter_key = $3
                LIMIT 1
                "#,
            )
            .bind(tenant_id)
            .bind(reservation_id)
            .bind(meter.as_str())
            .fetch_optional(&mut **transaction)
            .await
            .map_err(storage)?;
            tuple.map_or((currency, exponent, pricing), |row| {
                (
                    row.get("currency_code"),
                    row.get("currency_exponent"),
                    row.get("pricing_version"),
                )
            })
        } else {
            (currency, exponent, pricing)
        };
        sqlx::query(
            r#"
            INSERT INTO agent_usage_measures (
                tenant_id, usage_event_id, meter_key, amount,
                enforcement_amount, enforcement_basis, currency_code,
                currency_exponent, pricing_version
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(tenant_id)
        .bind(event_id)
        .bind(meter.as_str())
        .bind(amount)
        .bind(enforcement_amount)
        .bind(basis)
        .bind(currency)
        .bind(exponent)
        .bind(pricing)
        .execute(&mut **transaction)
        .await
        .map_err(storage)?;
    }
    Ok(())
}

fn page_report(rows: Vec<ReportRow>, limit: i64) -> Result<AgentUsageReportPage, AgentUsageError> {
    let mut items = rows
        .into_iter()
        .map(|row| {
            Ok(AgentUsageReportRow {
                event_id: row.event_id,
                event_kind: row.event_kind,
                outcome: row.outcome,
                run_id: row.run_id,
                actor_user_id: row.actor_user_id,
                origin_module_key: row.origin_module_key,
                capability_module_key: row.capability_module_key,
                capability_key: row.capability_key,
                provider_key: row.provider_key,
                provider_model_id: row.provider_model_id,
                meter: AgentUsageMeter::from_str(&row.meter_key)?,
                amount: row.amount,
                enforcement_amount: row.enforcement_amount,
                enforcement_basis: row.enforcement_basis,
                currency_code: row.currency_code,
                currency_exponent: row.currency_exponent,
                pricing_version: row.pricing_version,
                occurred_at: row.occurred_at,
            })
        })
        .collect::<Result<Vec<_>, AgentUsageError>>()?;
    let has_more = items.len() > usize::try_from(limit).map_err(|_| AgentUsageError::Storage)?;
    if has_more {
        items.truncate(usize::try_from(limit).map_err(|_| AgentUsageError::Storage)?);
    }
    let next_cursor = has_more.then(|| {
        items.last().map(|row| AgentUsageReportCursor {
            occurred_at: row.occurred_at,
            event_id: row.event_id,
            meter: row.meter,
        })
    });
    Ok(AgentUsageReportPage {
        items,
        next_cursor: next_cursor.flatten(),
    })
}

fn storage(error: sqlx::Error) -> AgentUsageError {
    #[cfg(test)]
    eprintln!("Agent usage test storage error: {error}");
    #[cfg(not(test))]
    let _ = error;
    AgentUsageError::Storage
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use sqlx::postgres::PgPoolOptions;

    use crate::{AgentUsageDemand, PrepareAgentUsage};

    const AGENT_USAGE_MIGRATION: &str =
        include_str!("../../../../../migrations/086_create_agent_usage_limits.sql");

    #[test]
    fn child_commit_requires_claim_except_for_true_preflight() {
        assert!(reserved_source_is_committable("run", false, None));
        assert!(reserved_source_is_committable(
            "provider_attempt",
            true,
            None
        ));
        assert!(reserved_source_is_committable(
            "provider_attempt",
            false,
            Some("preflight")
        ));
        assert!(!reserved_source_is_committable(
            "capability_call",
            false,
            None
        ));
    }

    #[test]
    fn hard_limit_denial_overrides_terminal_source_without_reusing_source_failure() {
        assert_eq!(terminal_usage_outcome("denied", "failed"), "denied");
        assert_eq!(
            terminal_usage_failure_code("denied", Some("run_failed")),
            Some("hard_limit_denied")
        );
        assert_eq!(
            terminal_usage_outcome("committed", "succeeded"),
            "succeeded"
        );
        assert_eq!(
            terminal_usage_failure_code("committed", Some("upstream_timeout")),
            Some("upstream_timeout")
        );
    }

    #[test]
    fn report_pages_are_bounded_and_use_the_last_visible_event() {
        let now = Utc::now();
        let rows = (0..3)
            .map(|index| ReportRow {
                event_id: Uuid::from_u128(index + 1),
                event_kind: "run".to_owned(),
                outcome: "succeeded".to_owned(),
                run_id: Uuid::new_v4(),
                actor_user_id: Uuid::new_v4(),
                origin_module_key: "sis".to_owned(),
                capability_module_key: None,
                capability_key: None,
                provider_key: None,
                provider_model_id: None,
                meter_key: "agent.runs".to_owned(),
                amount: Some(1),
                enforcement_amount: Some(1),
                enforcement_basis: Some("exact".to_owned()),
                currency_code: None,
                currency_exponent: None,
                pricing_version: None,
                occurred_at: now,
            })
            .collect();
        let page = page_report(rows, 2).unwrap();
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.next_cursor.unwrap().event_id, Uuid::from_u128(2));
    }

    #[test]
    fn reconciliation_uses_actual_zero_or_upper_bound_without_fx() {
        let mut source = usage_source();
        source.input_tokens = Some(3);
        let input = lifecycle_item("agent.input_tokens", 10);
        assert_eq!(
            reconcile_item(&input, &source).unwrap(),
            ReconciledAmount {
                amount: 3,
                basis: "exact"
            }
        );

        source.input_tokens = None;
        assert_eq!(
            reconcile_item(&input, &source).unwrap(),
            ReconciledAmount {
                amount: 10,
                basis: "upper_bound"
            }
        );

        source.failure_origin = Some("preflight".to_owned());
        assert_eq!(
            reconcile_item(&input, &source).unwrap(),
            ReconciledAmount {
                amount: 0,
                basis: "exact"
            }
        );

        source.estimated_cost_amount = Some(9);
        let estimate = lifecycle_item("agent.estimated_cost", 20);
        assert_eq!(
            reconcile_item(&estimate, &source).unwrap(),
            ReconciledAmount {
                amount: 9,
                basis: "estimated"
            }
        );
        source.estimated_cost_amount = None;
        assert_eq!(
            reconcile_item(&estimate, &source).unwrap(),
            ReconciledAmount {
                amount: 0,
                basis: "estimated"
            }
        );
    }

    #[test]
    fn reconciliation_rejects_unenforceable_or_over_reservation_evidence() {
        let mut source = usage_source();
        source.output_tokens = Some(11);
        assert!(matches!(
            reconcile_item(&lifecycle_item("agent.output_tokens", 10), &source),
            Err(AgentUsageError::InvalidTransition)
        ));
        assert!(matches!(
            reconcile_item(&lifecycle_item("agent.provider_reported_cost", 10), &source),
            Err(AgentUsageError::Storage)
        ));
        assert!(matches!(
            reconcile_item(&lifecycle_item("", 10), &source),
            Err(AgentUsageError::Storage)
        ));
        assert_eq!(
            reconcile_item(&lifecycle_item("agent.provider_attempts", 1), &source).unwrap(),
            ReconciledAmount {
                amount: 1,
                basis: "exact"
            }
        );
    }

    #[tokio::test]
    #[ignore = "requires a disposable fully migrated AGENT_USAGE_TEST_DATABASE_URL"]
    async fn postgres_contract_enforces_overlaps_replay_denial_and_exact_release() {
        let database_url = std::env::var("AGENT_USAGE_TEST_DATABASE_URL")
            .expect("AGENT_USAGE_TEST_DATABASE_URL must target a disposable migrated database");
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .connect(&database_url)
            .await
            .expect("Agent usage contract database must connect");
        sqlx::raw_sql(AGENT_USAGE_MIGRATION)
            .execute(&pool)
            .await
            .expect("migration 086 must replay before the service contract");

        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, $2, $3)")
            .bind(tenant_id)
            .bind(format!("usage-{}", &tenant_id.to_string()[..8]))
            .bind("Agent usage runtime contract")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            r#"
            INSERT INTO users (
                id, tenant_id, email, password_hash, full_name, roles
            ) VALUES ($1, $2, $3, 'test-only', 'Usage owner', ARRAY['campus_owner'])
            "#,
        )
        .bind(user_id)
        .bind(tenant_id)
        .bind(format!("usage-{user_id}@contract.test"))
        .execute(&pool)
        .await
        .unwrap();
        let rule_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO agent_limit_rules (
                id, tenant_id, scope_kind, meter_key, period, limit_value,
                enforcement, provenance_kind, configured_by, change_reason
            ) VALUES (
                $1, $2, 'campus', 'agent.runs', 'none', 2,
                'hard', 'campus_tightening', $3, 'Runtime overlap contract'
            )
            "#,
        )
        .bind(rule_id)
        .bind(tenant_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO entitlement_limits (
                tenant_id, limit_key, source_lease_id, unit, period,
                limit_value, enforcement
            ) VALUES ($1, 'agent.runs', $2, 'run', 'none', 3, 'hard')
            "#,
        )
        .bind(tenant_id)
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .unwrap();

        let first_run = seed_usage_run(&pool, tenant_id, user_id).await;
        let second_run = seed_usage_run(&pool, tenant_id, user_id).await;
        let third_run = seed_usage_run(&pool, tenant_id, user_id).await;
        let runtime = AgentUsageRuntime::new(pool.clone());
        let first = runtime
            .prepare(
                tenant_id,
                user_id,
                run_usage_command(first_run, "usage:runtime:first", [1; 32]),
            )
            .await
            .unwrap();
        assert_eq!(first.status, AgentUsageReservationStatus::Reserved);
        let replay = runtime
            .prepare(
                tenant_id,
                user_id,
                run_usage_command(first_run, "usage:runtime:first", [1; 32]),
            )
            .await
            .unwrap();
        assert_eq!(replay.reservation_id, first.reservation_id);
        let second = runtime
            .prepare(
                tenant_id,
                user_id,
                run_usage_command(second_run, "usage:runtime:second", [2; 32]),
            )
            .await
            .unwrap();
        assert_eq!(second.status, AgentUsageReservationStatus::Reserved);
        let denial = runtime
            .prepare(
                tenant_id,
                user_id,
                run_usage_command(third_run, "usage:runtime:third", [3; 32]),
            )
            .await
            .unwrap_err();
        assert!(matches!(denial, AgentUsageError::Denied { .. }));
        let (local_reserved, signed_reserved) = usage_reserved_counters(&pool, tenant_id).await;
        assert_eq!((local_reserved, signed_reserved), (2, 2));

        let released = runtime
            .release_or_expire(
                tenant_id,
                first.reservation_id,
                AgentUsageTerminalAction::Release,
            )
            .await
            .unwrap();
        assert_eq!(released.status, AgentUsageReservationStatus::Released);
        let (local_reserved, signed_reserved) = usage_reserved_counters(&pool, tenant_id).await;
        assert_eq!((local_reserved, signed_reserved), (1, 1));

        sqlx::query(
            r#"
            UPDATE agent_runs
            SET status = 'failed',
                safe_failure_code = 'usage_contract_failed',
                safe_failure_message = 'Usage contract terminal outcome',
                finished_at = CLOCK_TIMESTAMP(),
                version = version + 1,
                updated_at = CLOCK_TIMESTAMP()
            WHERE id = $1 AND tenant_id = $2 AND status = 'queued'
            "#,
        )
        .bind(second_run)
        .bind(tenant_id)
        .execute(&pool)
        .await
        .unwrap();
        let committed = runtime
            .commit_terminal_usage(tenant_id, second.reservation_id)
            .await
            .unwrap();
        assert_eq!(committed.status, AgentUsageReservationStatus::Committed);
        let committed_replay = runtime
            .commit_terminal_usage(tenant_id, second.reservation_id)
            .await
            .unwrap();
        assert_eq!(committed_replay, committed);
        assert_eq!(usage_reserved_counters(&pool, tenant_id).await, (0, 0));
        assert_eq!(usage_committed_counters(&pool, tenant_id).await, (1, 1));
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM agent_limit_reconciliations WHERE tenant_id = $1 AND reservation_id = $2",
            )
            .bind(tenant_id)
            .bind(second.reservation_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            2
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM agent_usage_events WHERE tenant_id = $1 AND limit_reservation_id = $2",
            )
            .bind(tenant_id)
            .bind(second.reservation_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COUNT(*)
                FROM agent_usage_measures AS measure
                INNER JOIN agent_usage_events AS event
                  ON event.id = measure.usage_event_id
                 AND event.tenant_id = measure.tenant_id
                WHERE event.tenant_id = $1 AND event.limit_reservation_id = $2
                "#,
            )
            .bind(tenant_id)
            .bind(second.reservation_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COUNT(*)
                FROM entitlement_usage_reservations AS source
                INNER JOIN entitlement_usage_events AS event
                  ON event.reservation_id = source.id
                 AND event.tenant_id = source.tenant_id
                WHERE source.tenant_id = $1
                  AND source.status = 'committed'
                  AND source.amount = 1
                  AND event.amount = 1
                "#,
            )
            .bind(tenant_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM agent_limit_reservation_items WHERE tenant_id = $1",
            )
            .bind(tenant_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            6
        );
    }

    fn run_usage_command(
        run_id: Uuid,
        idempotency_key: &str,
        fingerprint: [u8; 32],
    ) -> PrepareAgentUsage {
        PrepareAgentUsage::parse(
            run_id,
            AgentUsageStage::Run,
            idempotency_key,
            fingerprint,
            [AgentUsageDemand::count(AgentUsageMeter::Runs, 1).unwrap()],
            Duration::from_secs(60),
        )
        .unwrap()
    }

    fn lifecycle_item(meter_key: &str, reserved_amount: i64) -> ItemLifecycleRow {
        ItemLifecycleRow {
            id: Uuid::new_v4(),
            bucket_id: Some(Uuid::new_v4()),
            entitlement_bucket_id: None,
            entitlement_reservation_id: None,
            reserved_amount,
            meter_key: meter_key.to_owned(),
            period_start: Utc::now(),
            period_end: None,
            source_lease_id: None,
            entitlement_limit_key: None,
            unit: "token".to_owned(),
            currency_code: None,
            currency_exponent: None,
            pricing_version: None,
        }
    }

    fn usage_source() -> UsageEventSourceRow {
        UsageEventSourceRow {
            thread_id: Uuid::new_v4(),
            actor_user_id: Uuid::new_v4(),
            role_keys: vec!["campus_owner".to_owned()],
            origin_module_key: "sis".to_owned(),
            task_class: "module_read_reporting".to_owned(),
            request_id: Uuid::new_v4(),
            correlation_id: Uuid::new_v4(),
            event_kind: "provider_attempt".to_owned(),
            run_id: Uuid::new_v4(),
            provider_attempt_id: Some(Uuid::new_v4()),
            provider_turn_index: Some(1),
            provider_attempt_index: Some(1),
            provider_connection_id: Some(Uuid::new_v4()),
            provider_key: Some("openai".to_owned()),
            provider_model_id: Some("test-model".to_owned()),
            provider_model_snapshot_id: Some(Uuid::new_v4()),
            route_priority: Some(1),
            failure_origin: Some("upstream".to_owned()),
            failure_category: Some("rate_limited".to_owned()),
            capability_call_id: None,
            capability_module_key: None,
            capability_key: None,
            capability_version: None,
            approval_state: None,
            outcome: "failed".to_owned(),
            safe_failure_code: Some("rate_limited".to_owned()),
            duration_ms: 1,
            occurred_at: Utc::now(),
            input_tokens: None,
            output_tokens: None,
            cached_tokens: None,
            reasoning_tokens: None,
            provider_reported_cost_amount: None,
            provider_reported_cost_currency: None,
            provider_reported_cost_exponent: None,
            provider_reported_pricing_version: None,
            estimated_cost_amount: None,
            estimated_cost_currency: None,
            estimated_cost_exponent: None,
            estimated_pricing_version: None,
        }
    }

    async fn seed_usage_run(pool: &PgPool, tenant_id: Uuid, user_id: Uuid) -> Uuid {
        let thread_id = Uuid::new_v4();
        let message_id = Uuid::new_v4();
        let run_id = Uuid::new_v4();
        sqlx::query("INSERT INTO agent_threads (id, tenant_id, owner_user_id) VALUES ($1, $2, $3)")
            .bind(thread_id)
            .bind(tenant_id)
            .bind(user_id)
            .execute(pool)
            .await
            .unwrap();
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
        .unwrap();
        sqlx::query(
            r#"
            UPDATE agent_threads
            SET next_message_sequence = 2, version = 2,
                last_activity_at = last_activity_at + INTERVAL '1 second',
                updated_at = updated_at + INTERVAL '1 second'
            WHERE id = $1
            "#,
        )
        .bind(thread_id)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO agent_messages (
                id, tenant_id, thread_id, sequence, role, user_id, content
            ) VALUES ($1, $2, $3, 1, 'user', $4, 'Usage contract')
            "#,
        )
        .bind(message_id)
        .bind(tenant_id)
        .bind(thread_id)
        .bind(user_id)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO agent_runs (
                id, tenant_id, thread_id, request_message_id, requested_by,
                task_class, origin_module_key, origin_route, request_id,
                correlation_id
            ) VALUES (
                $1, $2, $3, $4, $5, 'module_read_reporting', 'sis',
                '/modules/sis', $6, $7
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
        .unwrap();
        run_id
    }

    async fn usage_reserved_counters(pool: &PgPool, tenant_id: Uuid) -> (i64, i64) {
        let local = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(SUM(reserved_value), 0)::BIGINT FROM agent_limit_buckets WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .fetch_one(pool)
        .await
        .unwrap();
        let signed = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(SUM(reserved_value), 0)::BIGINT FROM entitlement_meter_buckets WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .fetch_one(pool)
        .await
        .unwrap();
        (local, signed)
    }

    async fn usage_committed_counters(pool: &PgPool, tenant_id: Uuid) -> (i64, i64) {
        let local = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(SUM(committed_value), 0)::BIGINT FROM agent_limit_buckets WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .fetch_one(pool)
        .await
        .unwrap();
        let signed = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(SUM(committed_value), 0)::BIGINT FROM entitlement_meter_buckets WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .fetch_one(pool)
        .await
        .unwrap();
        (local, signed)
    }
}
