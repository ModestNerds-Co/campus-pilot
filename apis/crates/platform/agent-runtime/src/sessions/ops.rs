//! Implements owner-scoped Session history and fenced durable run orchestration in PostgreSQL.
//!
//! User-message submission and worker completion use the schema's canonical lock order. Every
//! worker mutation matches tenant, run, worker, random lease token, and monotonic queue version;
//! expired non-idempotent checkpoints are interrupted rather than replayed.

use std::str::FromStr;

use chrono::{DateTime, Utc};
use cp_audit::{AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext};
use serde_json::{Map, Value};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::execution::{ExecutionTerminal, terminalize_running_children};
use super::types::{
    AgentMessage, AgentRun, AgentRunEvent, AgentSession, AgentSessionError, ArchiveSessionCommand,
    ClaimRunsCommand, ClaimedRun, CreateSessionCommand, CursorPage, EventCursor,
    ExpiredLeaseRecoveryDisposition, FinalResponsePlaintext, GlobalRecoveryBatch, LeaseHeartbeat,
    ListEventsQuery, ListMessagesQuery, ListRunsQuery, ListSessionsQuery, MessageCursor,
    MessageRole, RecoveredRun, RecoverySummary, RecoveryUsageAction, RecoveryUsageReservation,
    RecoveryUsageStage, RenameSessionCommand, RunCheckpoint, RunCursor, RunEventType, RunLease,
    RunStatus, SafeRunFailure, SessionCursor, SessionStatus, SubmitMessageCommand,
};
use crate::TaskClass;

const CREATE_SESSION_OPERATION: &str = "agent.sessions.create";
const SUBMIT_MESSAGE_OPERATION: &str = "agent.messages.submit";
const MAX_DELIVERY_ATTEMPTS: i16 = 3;

/// Shared durable Session service used by owner APIs and the dedicated Agent worker.
#[derive(Debug, Clone)]
pub struct AgentSessionOps {
    pub(super) pool: PgPool,
}

impl AgentSessionOps {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Lists only Sessions owned by the trusted person identity.
    pub async fn list_sessions(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        query: ListSessionsQuery,
    ) -> Result<CursorPage<AgentSession, SessionCursor>, AgentSessionError> {
        let cursor_at = query.cursor.map(|cursor| cursor.last_activity_at);
        let cursor_id = query.cursor.map(|cursor| cursor.session_id);
        let fetch_limit = query.limit.get() + 1;
        let rows = sqlx::query_as::<_, SessionRow>(
            r#"
            SELECT t.id, t.title, t.status, t.version, t.last_activity_at,
                   t.created_at, t.updated_at
            FROM agent_threads t
            JOIN agent_thread_members m
              ON m.tenant_id = t.tenant_id
             AND m.thread_id = t.id
             AND m.user_id = $2
             AND m.membership_role = 'owner'
             AND m.deleted_at IS NULL
            WHERE t.tenant_id = $1
              AND t.owner_user_id = $2
              AND t.deleted_at IS NULL
              AND ($3 OR t.status = 'active')
              AND (
                    $4::TEXT IS NULL
                    OR POSITION(LOWER($4) IN LOWER(t.title)) > 0
              )
              AND (
                    $5::TIMESTAMPTZ IS NULL
                    OR (t.last_activity_at, t.id) < ($5, $6)
              )
            ORDER BY t.last_activity_at DESC, t.id DESC
            LIMIT $7
            "#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(query.include_archived)
        .bind(query.title_search.as_deref())
        .bind(cursor_at)
        .bind(cursor_id)
        .bind(fetch_limit)
        .fetch_all(&self.pool)
        .await?;
        page_sessions(rows, query.limit.get() as usize)
    }

    /// Creates a Session and owner membership atomically, replaying an identical request.
    pub async fn create_session(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        request_context: RequestContext,
        command: CreateSessionCommand,
    ) -> Result<AgentSession, AgentSessionError> {
        let mut transaction = self.pool.begin().await?;
        lock_idempotency_key(
            &mut transaction,
            tenant_id,
            user_id,
            CREATE_SESSION_OPERATION,
            None,
            command.idempotency_key(),
        )
        .await?;
        if let Some(result_id) = resolve_idempotency(
            &mut transaction,
            tenant_id,
            user_id,
            CREATE_SESSION_OPERATION,
            None,
            command.idempotency_key(),
            command.fingerprint(),
            "thread",
        )
        .await?
        {
            transaction.commit().await?;
            return self.read_session(tenant_id, user_id, result_id).await;
        }

        let session_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO agent_threads (id, tenant_id, owner_user_id, title)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(session_id)
        .bind(tenant_id)
        .bind(user_id)
        .bind(&command.title)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO agent_thread_members (
                tenant_id, thread_id, user_id, membership_role, added_by
            )
            VALUES ($1, $2, $3, 'owner', $3)
            "#,
        )
        .bind(tenant_id)
        .bind(session_id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
        insert_idempotency(
            &mut transaction,
            tenant_id,
            user_id,
            CREATE_SESSION_OPERATION,
            None,
            command.idempotency_key(),
            command.fingerprint(),
            "thread",
            session_id,
        )
        .await?;
        append_person_audit(
            &mut transaction,
            tenant_id,
            user_id,
            request_context,
            CREATE_SESSION_OPERATION,
            "agent_thread",
            session_id,
            None,
            Map::new(),
        )
        .await?;
        transaction.commit().await?;
        self.read_session(tenant_id, user_id, session_id).await
    }

    /// Reads one Session only when the trusted person is its explicit owner member.
    pub async fn read_session(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        session_id: Uuid,
    ) -> Result<AgentSession, AgentSessionError> {
        let row = sqlx::query_as::<_, SessionRow>(
            r#"
            SELECT t.id, t.title, t.status, t.version, t.last_activity_at,
                   t.created_at, t.updated_at
            FROM agent_threads t
            JOIN agent_thread_members m
              ON m.tenant_id = t.tenant_id
             AND m.thread_id = t.id
             AND m.user_id = $2
             AND m.membership_role = 'owner'
             AND m.deleted_at IS NULL
            WHERE t.tenant_id = $1
              AND t.id = $3
              AND t.owner_user_id = $2
              AND t.deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AgentSessionError::SessionNotFound)?;
        row.try_into()
    }

    /// Renames an active Session under its optimistic version.
    pub async fn rename_session(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        session_id: Uuid,
        request_context: RequestContext,
        command: RenameSessionCommand,
    ) -> Result<AgentSession, AgentSessionError> {
        let mut transaction = self.pool.begin().await?;
        let locked = lock_owned_session(&mut transaction, tenant_id, user_id, session_id).await?;
        require_active_session(&locked)?;
        if locked.version != command.expected_version {
            return Err(stale_session());
        }
        let updated = sqlx::query(
            r#"
            UPDATE agent_threads
            SET title = $1,
                version = version + 1,
                updated_at = CLOCK_TIMESTAMP()
            WHERE tenant_id = $2
              AND id = $3
              AND owner_user_id = $4
              AND status = 'active'
              AND version = $5
              AND deleted_at IS NULL
            "#,
        )
        .bind(&command.title)
        .bind(tenant_id)
        .bind(session_id)
        .bind(user_id)
        .bind(command.expected_version)
        .execute(&mut *transaction)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(stale_session());
        }
        append_person_audit(
            &mut transaction,
            tenant_id,
            user_id,
            request_context,
            "agent.sessions.update",
            "agent_thread",
            session_id,
            None,
            version_metadata(command.expected_version + 1),
        )
        .await?;
        transaction.commit().await?;
        self.read_session(tenant_id, user_id, session_id).await
    }

    /// Archives an active Session without deleting transcript, run, event, or audit history.
    pub async fn archive_session(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        session_id: Uuid,
        request_context: RequestContext,
        command: ArchiveSessionCommand,
    ) -> Result<AgentSession, AgentSessionError> {
        let mut transaction = self.pool.begin().await?;
        let locked = lock_owned_session(&mut transaction, tenant_id, user_id, session_id).await?;
        require_active_session(&locked)?;
        if locked.version != command.expected_version {
            return Err(stale_session());
        }
        if active_run_exists(&mut transaction, tenant_id, session_id).await? {
            return Err(AgentSessionError::conflict(
                "active_run_exists",
                "Cancel or finish the active run before archiving this Session",
            ));
        }
        let updated = sqlx::query(
            r#"
            UPDATE agent_threads
            SET status = 'archived',
                version = version + 1,
                updated_at = CLOCK_TIMESTAMP()
            WHERE tenant_id = $1
              AND id = $2
              AND owner_user_id = $3
              AND status = 'active'
              AND version = $4
              AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(session_id)
        .bind(user_id)
        .bind(command.expected_version)
        .execute(&mut *transaction)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(stale_session());
        }
        append_person_audit(
            &mut transaction,
            tenant_id,
            user_id,
            request_context,
            "agent.sessions.archive",
            "agent_thread",
            session_id,
            None,
            version_metadata(command.expected_version + 1),
        )
        .await?;
        transaction.commit().await?;
        self.read_session(tenant_id, user_id, session_id).await
    }

    /// Lists the owner-visible transcript in immutable sequence order.
    pub async fn list_messages(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        session_id: Uuid,
        query: ListMessagesQuery,
    ) -> Result<CursorPage<AgentMessage, MessageCursor>, AgentSessionError> {
        ensure_owned_session(&self.pool, tenant_id, user_id, session_id).await?;
        let cursor_sequence = query.cursor.map(|cursor| cursor.sequence);
        let cursor_id = query.cursor.map(|cursor| cursor.message_id);
        let rows = sqlx::query_as::<_, MessageRow>(
            r#"
            SELECT id, thread_id, sequence, role, content, created_at
            FROM agent_messages
            WHERE tenant_id = $1
              AND thread_id = $2
              AND deleted_at IS NULL
              AND (
                    $3::BIGINT IS NULL
                    OR (sequence, id) > ($3, $4)
              )
            ORDER BY sequence, id
            LIMIT $5
            "#,
        )
        .bind(tenant_id)
        .bind(session_id)
        .bind(cursor_sequence)
        .bind(cursor_id)
        .bind(query.limit.get() + 1)
        .fetch_all(&self.pool)
        .await?;
        page_messages(rows, query.limit.get() as usize)
    }

    /// Atomically persists one user message, queued run, queue row, event, replay evidence, and audit.
    pub async fn submit_message(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        session_id: Uuid,
        request_context: RequestContext,
        command: SubmitMessageCommand,
    ) -> Result<AgentRun, AgentSessionError> {
        let mut transaction = self.pool.begin().await?;
        let locked = lock_owned_session(&mut transaction, tenant_id, user_id, session_id).await?;
        require_active_session(&locked)?;
        if let Some(result_id) = resolve_idempotency(
            &mut transaction,
            tenant_id,
            user_id,
            SUBMIT_MESSAGE_OPERATION,
            Some(session_id),
            command.idempotency_key(),
            command.fingerprint(),
            "run",
        )
        .await?
        {
            transaction.commit().await?;
            return self.read_run(tenant_id, user_id, result_id).await;
        }
        if active_run_exists(&mut transaction, tenant_id, session_id).await? {
            return Err(AgentSessionError::conflict(
                "active_run_exists",
                "Wait for or cancel the active run before submitting another message",
            ));
        }

        let sequence = locked.next_message_sequence;
        let message_id = Uuid::new_v4();
        let run_id = Uuid::new_v4();
        let session_update = sqlx::query(
            r#"
            UPDATE agent_threads
            SET next_message_sequence = next_message_sequence + 1,
                version = version + 1,
                last_activity_at = CLOCK_TIMESTAMP(),
                updated_at = CLOCK_TIMESTAMP()
            WHERE tenant_id = $1
              AND id = $2
              AND owner_user_id = $3
              AND status = 'active'
              AND version = $4
              AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(session_id)
        .bind(user_id)
        .bind(locked.version)
        .execute(&mut *transaction)
        .await?;
        if session_update.rows_affected() != 1 {
            return Err(stale_session());
        }
        sqlx::query(
            r#"
            INSERT INTO agent_messages (
                id, tenant_id, thread_id, sequence, role, user_id, content
            )
            VALUES ($1, $2, $3, $4, 'user', $5, $6)
            "#,
        )
        .bind(message_id)
        .bind(tenant_id)
        .bind(session_id)
        .bind(sequence)
        .bind(user_id)
        .bind(&command.content)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO agent_runs (
                id, tenant_id, thread_id, request_message_id, requested_by,
                task_class, origin_module_key, origin_route, request_id, correlation_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(run_id)
        .bind(tenant_id)
        .bind(session_id)
        .bind(message_id)
        .bind(user_id)
        .bind(command.task_class.as_str())
        .bind(&command.origin_module_key)
        .bind(&command.origin_route)
        .bind(request_context.request_id())
        .bind(request_context.correlation_id())
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO agent_run_queue (run_id, tenant_id)
            VALUES ($1, $2)
            "#,
        )
        .bind(run_id)
        .bind(tenant_id)
        .execute(&mut *transaction)
        .await?;
        append_run_event(&mut transaction, tenant_id, run_id, RunEventType::Queued).await?;
        insert_idempotency(
            &mut transaction,
            tenant_id,
            user_id,
            SUBMIT_MESSAGE_OPERATION,
            Some(session_id),
            command.idempotency_key(),
            command.fingerprint(),
            "run",
            run_id,
        )
        .await?;
        append_person_audit(
            &mut transaction,
            tenant_id,
            user_id,
            request_context,
            SUBMIT_MESSAGE_OPERATION,
            "agent_run",
            run_id,
            Some(run_id),
            sequence_metadata(sequence),
        )
        .await?;
        transaction.commit().await?;
        self.read_run(tenant_id, user_id, run_id).await
    }

    /// Lists runs for one owned Session, newest first.
    pub async fn list_runs(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        session_id: Uuid,
        query: ListRunsQuery,
    ) -> Result<CursorPage<AgentRun, RunCursor>, AgentSessionError> {
        ensure_owned_session(&self.pool, tenant_id, user_id, session_id).await?;
        let cursor_at = query.cursor.map(|cursor| cursor.created_at);
        let cursor_id = query.cursor.map(|cursor| cursor.run_id);
        let rows = sqlx::query_as::<_, RunRow>(
            r#"
            SELECT id, thread_id, request_message_id, response_message_id, task_class,
                   origin_module_key, origin_route, status, safe_failure_code,
                   safe_failure_message, started_at, finished_at, version,
                   created_at, updated_at
            FROM agent_runs
            WHERE tenant_id = $1
              AND thread_id = $2
              AND deleted_at IS NULL
              AND (
                    $3::TIMESTAMPTZ IS NULL
                    OR (created_at, id) < ($3, $4)
              )
            ORDER BY created_at DESC, id DESC
            LIMIT $5
            "#,
        )
        .bind(tenant_id)
        .bind(session_id)
        .bind(cursor_at)
        .bind(cursor_id)
        .bind(query.limit.get() + 1)
        .fetch_all(&self.pool)
        .await?;
        page_runs(rows, query.limit.get() as usize)
    }

    /// Reads one run through its owning Session authorization boundary.
    pub async fn read_run(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        run_id: Uuid,
    ) -> Result<AgentRun, AgentSessionError> {
        let row = sqlx::query_as::<_, RunRow>(
            r#"
            SELECT r.id, r.thread_id, r.request_message_id, r.response_message_id,
                   r.task_class, r.origin_module_key, r.origin_route, r.status,
                   r.safe_failure_code, r.safe_failure_message, r.started_at,
                   r.finished_at, r.version, r.created_at, r.updated_at
            FROM agent_runs r
            JOIN agent_threads t
              ON t.tenant_id = r.tenant_id AND t.id = r.thread_id
            JOIN agent_thread_members m
              ON m.tenant_id = t.tenant_id
             AND m.thread_id = t.id
             AND m.user_id = $2
             AND m.membership_role = 'owner'
             AND m.deleted_at IS NULL
            WHERE r.tenant_id = $1
              AND r.id = $3
              AND r.requested_by = $2
              AND t.owner_user_id = $2
              AND r.deleted_at IS NULL
              AND t.deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AgentSessionError::RunNotFound)?;
        row.try_into()
    }

    /// Cancels available work immediately or requests cooperative cancellation from its lease owner.
    pub async fn cancel_run(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        run_id: Uuid,
        request_context: RequestContext,
    ) -> Result<AgentRun, AgentSessionError> {
        let mut transaction = self.pool.begin().await?;
        let queue = lock_owned_queue(&mut transaction, tenant_id, user_id, run_id).await?;
        let run = lock_run(&mut transaction, tenant_id, run_id).await?;
        let status = RunStatus::from_str(&run.status)?;
        if status == RunStatus::Cancelled {
            transaction.commit().await?;
            return self.read_run(tenant_id, user_id, run_id).await;
        }
        if status.is_terminal() {
            return Err(AgentSessionError::conflict(
                "run_already_finished",
                "This Agent run has already finished",
            ));
        }

        if queue.cancel_requested_at.is_some() {
            transaction.commit().await?;
            return self.read_run(tenant_id, user_id, run_id).await;
        }

        let cancellation_request = sqlx::query(
            r#"
            UPDATE agent_run_queue
            SET cancel_requested_at = STATEMENT_TIMESTAMP(),
                cancel_requested_by = $1,
                updated_at = STATEMENT_TIMESTAMP()
            WHERE tenant_id = $2
              AND run_id = $3
              AND state <> 'finished'
              AND cancel_requested_at IS NULL
              AND version = $4
              AND deleted_at IS NULL
            "#,
        )
        .bind(user_id)
        .bind(tenant_id)
        .bind(run_id)
        .bind(queue.version)
        .execute(&mut *transaction)
        .await?;
        if cancellation_request.rows_affected() != 1 {
            return Err(AgentSessionError::conflict(
                "run_state_changed",
                "This Agent run changed while cancellation was requested",
            ));
        }

        if queue.state == "leased" {
            append_person_audit(
                &mut transaction,
                tenant_id,
                user_id,
                request_context,
                "agent.runs.cancel.request",
                "agent_run",
                run_id,
                Some(run_id),
                Map::new(),
            )
            .await?;
            transaction.commit().await?;
            return self.read_run(tenant_id, user_id, run_id).await;
        }
        if queue.state != "available" {
            return Err(AgentSessionError::storage_contract());
        }
        terminalize_running_children(
            &mut transaction,
            tenant_id,
            run_id,
            ExecutionTerminal::Cancelled,
        )
        .await?;
        transition_run_to_cancelled(&mut transaction, tenant_id, run_id, &run).await?;
        finish_available_queue(&mut transaction, tenant_id, run_id, queue.version).await?;
        append_run_event(&mut transaction, tenant_id, run_id, RunEventType::Cancelled).await?;
        append_person_audit(
            &mut transaction,
            tenant_id,
            user_id,
            request_context,
            "agent.runs.cancel",
            "agent_run",
            run_id,
            Some(run_id),
            Map::new(),
        )
        .await?;
        transaction.commit().await?;
        self.read_run(tenant_id, user_id, run_id).await
    }

    /// Cooperatively acknowledges a cancellation request under the exact current lease fence.
    pub async fn acknowledge_cancellation(
        &self,
        tenant_id: Uuid,
        lease: &RunLease,
    ) -> Result<AgentRun, AgentSessionError> {
        let mut transaction = self.pool.begin().await?;
        let queue = lock_leased_queue(&mut transaction, tenant_id, lease).await?;
        if queue.cancel_requested_at.is_none() {
            return Err(AgentSessionError::conflict(
                "run_cancellation_not_requested",
                "This Agent run has no cancellation request to acknowledge",
            ));
        }
        let run = lock_run(&mut transaction, tenant_id, lease.run_id).await?;
        if RunStatus::from_str(&run.status)? != RunStatus::Running {
            return Err(AgentSessionError::conflict(
                "run_not_running",
                "Only a running Agent run can acknowledge cancellation",
            ));
        }
        terminalize_running_children(
            &mut transaction,
            tenant_id,
            run.id,
            ExecutionTerminal::Cancelled,
        )
        .await?;
        transition_run_to_cancelled(&mut transaction, tenant_id, run.id, &run).await?;
        finish_queue(&mut transaction, tenant_id, lease).await?;
        append_run_event(&mut transaction, tenant_id, run.id, RunEventType::Cancelled).await?;
        append_worker_audit(
            &mut transaction,
            tenant_id,
            run.requested_by,
            RequestContext::from_ids(run.request_id, run.correlation_id),
            "agent.runs.cancel.acknowledge",
            run.id,
            AuditOutcome::Succeeded,
            None,
        )
        .await?;
        transaction.commit().await?;
        self.read_run(tenant_id, run.requested_by, run.id).await
    }

    /// Reads reduced run events after a monotonically increasing replay cursor.
    pub async fn list_events(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        run_id: Uuid,
        query: ListEventsQuery,
    ) -> Result<CursorPage<AgentRunEvent, EventCursor>, AgentSessionError> {
        self.read_run(tenant_id, user_id, run_id).await?;
        let rows = sqlx::query_as::<_, EventRow>(
            r#"
            SELECT id, run_id, event_type, created_at
            FROM agent_run_events
            WHERE tenant_id = $1
              AND run_id = $2
              AND id > $3
              AND deleted_at IS NULL
            ORDER BY id
            LIMIT $4
            "#,
        )
        .bind(tenant_id)
        .bind(run_id)
        .bind(query.after.get())
        .bind(query.limit.get() + 1)
        .fetch_all(&self.pool)
        .await?;
        page_events(rows, query.limit.get() as usize)
    }

    /// Claims bounded available work with `SKIP LOCKED` and starts queued runs atomically.
    pub async fn claim_runs(
        &self,
        tenant_id: Uuid,
        command: ClaimRunsCommand,
    ) -> Result<Vec<ClaimedRun>, AgentSessionError> {
        let mut transaction = self.pool.begin().await?;
        let candidates = sqlx::query_as::<_, QueueCandidateRow>(
            r#"
            SELECT tenant_id, run_id, version
            FROM agent_run_queue
            WHERE tenant_id = $1
              AND state = 'available'
              AND available_at <= STATEMENT_TIMESTAMP()
              AND delivery_attempt < 3
              AND cancel_requested_at IS NULL
              AND deleted_at IS NULL
            ORDER BY available_at, run_id
            FOR UPDATE SKIP LOCKED
            LIMIT $2
            "#,
        )
        .bind(tenant_id)
        .bind(command.batch_size)
        .fetch_all(&mut *transaction)
        .await?;
        let claimed = claim_candidates(&mut transaction, &command, candidates).await?;
        transaction.commit().await?;
        Ok(claimed)
    }

    /// Claims a bounded tenant-fair batch without requiring the worker to enumerate campuses.
    pub async fn claim_runs_globally(
        &self,
        command: ClaimRunsCommand,
    ) -> Result<Vec<ClaimedRun>, AgentSessionError> {
        let mut transaction = self.pool.begin().await?;
        let candidates = sqlx::query_as::<_, QueueCandidateRow>(
            r#"
            WITH ranked AS MATERIALIZED (
                SELECT tenant_id, run_id, version, available_at,
                       ROW_NUMBER() OVER (
                           PARTITION BY tenant_id
                           ORDER BY available_at, run_id
                       ) AS tenant_rank
                FROM agent_run_queue
                WHERE state = 'available'
                  AND available_at <= STATEMENT_TIMESTAMP()
                  AND delivery_attempt < 3
                  AND cancel_requested_at IS NULL
                  AND deleted_at IS NULL
            )
            SELECT queue.tenant_id, queue.run_id, queue.version
            FROM ranked
            INNER JOIN agent_run_queue AS queue
              ON queue.tenant_id = ranked.tenant_id
             AND queue.run_id = ranked.run_id
             AND queue.version = ranked.version
            ORDER BY ranked.tenant_rank, ranked.available_at,
                     ranked.tenant_id, ranked.run_id
            FOR UPDATE OF queue SKIP LOCKED
            LIMIT $1
            "#,
        )
        .bind(command.batch_size)
        .fetch_all(&mut *transaction)
        .await?;
        let claimed = claim_candidates(&mut transaction, &command, candidates).await?;
        transaction.commit().await?;
        Ok(claimed)
    }

    /// Extends a valid lease by exactly 30 seconds and returns the next monotonic fence.
    pub async fn heartbeat(
        &self,
        tenant_id: Uuid,
        lease: &RunLease,
    ) -> Result<LeaseHeartbeat, AgentSessionError> {
        let row = sqlx::query_as::<_, HeartbeatRow>(
            r#"
            WITH lease_clock AS (
                SELECT STATEMENT_TIMESTAMP() AS captured_at
            )
            UPDATE agent_run_queue q
            SET heartbeat_at = lease_clock.captured_at,
                lease_expires_at = lease_clock.captured_at + INTERVAL '30 seconds',
                version = version + 1,
                updated_at = lease_clock.captured_at
            FROM lease_clock
            WHERE q.tenant_id = $1
              AND q.run_id = $2
              AND q.state = 'leased'
              AND q.leased_by = $3
              AND q.lease_token = $4
              AND q.version = $5
              AND q.lease_expires_at > STATEMENT_TIMESTAMP()
              AND q.deleted_at IS NULL
            RETURNING version, lease_expires_at, cancel_requested_at IS NOT NULL AS cancel_requested
            "#,
        )
        .bind(tenant_id)
        .bind(lease.run_id)
        .bind(&lease.worker_id)
        .bind(lease.lease_token)
        .bind(lease.fence_version)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AgentSessionError::LeaseLost)?;
        Ok(LeaseHeartbeat {
            lease: next_lease(lease, row.version),
            cancel_requested: row.cancel_requested,
            lease_expires_at: row.lease_expires_at,
        })
    }

    /// Test-only seam for adversarial checkpoint recovery; production workers use atomic methods.
    #[cfg(test)]
    async fn checkpoint(
        &self,
        tenant_id: Uuid,
        lease: &RunLease,
        next: RunCheckpoint,
    ) -> Result<RunLease, AgentSessionError> {
        let current = sqlx::query_as::<_, CheckpointRow>(
            r#"
            SELECT checkpoint, cancel_requested_at IS NOT NULL AS cancel_requested
            FROM agent_run_queue
            WHERE tenant_id = $1
              AND run_id = $2
              AND state = 'leased'
              AND leased_by = $3
              AND lease_token = $4
              AND version = $5
              AND lease_expires_at > STATEMENT_TIMESTAMP()
              AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(lease.run_id)
        .bind(&lease.worker_id)
        .bind(lease.lease_token)
        .bind(lease.fence_version)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AgentSessionError::LeaseLost)?;
        if current.cancel_requested {
            return Err(AgentSessionError::conflict(
                "run_cancel_requested",
                "Cancellation was requested for this Agent run",
            ));
        }
        let previous = RunCheckpoint::from_str(&current.checkpoint)?;
        if !previous.can_advance_to(next) {
            return Err(AgentSessionError::conflict(
                "invalid_run_checkpoint",
                "This Agent run cannot advance to that checkpoint",
            ));
        }
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
              AND cancel_requested_at IS NULL
              AND deleted_at IS NULL
            RETURNING version
            "#,
        )
        .bind(next.as_str())
        .bind(tenant_id)
        .bind(lease.run_id)
        .bind(&lease.worker_id)
        .bind(lease.lease_token)
        .bind(lease.fence_version)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AgentSessionError::LeaseLost)?;
        Ok(next_lease(lease, version))
    }

    /// Completes from the unique durable final-response artifact; lost acknowledgements are safe.
    pub async fn complete_run(
        &self,
        tenant_id: Uuid,
        lease: &RunLease,
        artifact_id: Uuid,
        assistant_message: FinalResponsePlaintext,
    ) -> Result<AgentRun, AgentSessionError> {
        let mut transaction = self.pool.begin().await?;
        let queue = lock_completion_queue(&mut transaction, tenant_id, lease.run_id).await?;
        let run = lock_run(&mut transaction, tenant_id, lease.run_id).await?;
        if queue.state == "finished" && RunStatus::from_str(&run.status)? == RunStatus::Completed {
            verify_final_response_evidence(
                &mut transaction,
                tenant_id,
                run.id,
                artifact_id,
                &assistant_message,
            )
            .await?;
            let response_message_id = run
                .response_message_id
                .ok_or_else(AgentSessionError::storage_contract)?;
            let message_matches = sqlx::query_scalar::<_, bool>(
                r#"
                SELECT EXISTS (
                    SELECT 1 FROM agent_messages
                    WHERE tenant_id = $1
                      AND thread_id = $2
                      AND id = $3
                      AND role = 'assistant'
                      AND content = $4
                      AND deleted_at IS NULL
                )
                "#,
            )
            .bind(tenant_id)
            .bind(run.thread_id)
            .bind(response_message_id)
            .bind(assistant_message.as_str())
            .fetch_one(&mut *transaction)
            .await?;
            if !message_matches {
                return Err(AgentSessionError::conflict(
                    "final_response_conflict",
                    "This Agent run was completed from different final response evidence",
                ));
            }
            transaction.commit().await?;
            return self.read_run(tenant_id, run.requested_by, run.id).await;
        }
        if queue.state != "leased"
            || queue.leased_by.as_deref() != Some(lease.worker_id.as_str())
            || queue.lease_token != Some(lease.lease_token)
            || queue.version != lease.fence_version
            || !queue.lease_current
        {
            return Err(AgentSessionError::LeaseLost);
        }
        if queue.cancel_requested_at.is_some() {
            return Err(AgentSessionError::conflict(
                "run_cancel_requested",
                "Cancellation was requested for this Agent run",
            ));
        }
        if RunCheckpoint::from_str(&queue.checkpoint)? != RunCheckpoint::Finalizing {
            return Err(AgentSessionError::conflict(
                "run_not_finalizing",
                "The Agent run must reach its finalizing checkpoint before completion",
            ));
        }
        if RunStatus::from_str(&run.status)? != RunStatus::Running {
            return Err(AgentSessionError::conflict(
                "run_not_running",
                "Only a running Agent run can complete",
            ));
        }
        verify_final_response_evidence(
            &mut transaction,
            tenant_id,
            run.id,
            artifact_id,
            &assistant_message,
        )
        .await?;
        let thread = lock_thread_for_worker(&mut transaction, tenant_id, run.thread_id).await?;
        require_active_session(&thread)?;
        let response_message_id = Uuid::new_v4();
        allocate_message_sequence(&mut transaction, tenant_id, run.thread_id, thread.version)
            .await?;
        sqlx::query(
            r#"
            INSERT INTO agent_messages (
                id, tenant_id, thread_id, sequence, role, user_id, content
            )
            VALUES ($1, $2, $3, $4, 'assistant', NULL, $5)
            "#,
        )
        .bind(response_message_id)
        .bind(tenant_id)
        .bind(run.thread_id)
        .bind(thread.next_message_sequence)
        .bind(assistant_message.as_str())
        .execute(&mut *transaction)
        .await?;
        let run_update = sqlx::query(
            r#"
            UPDATE agent_runs
            SET status = 'completed',
                response_message_id = $1,
                finished_at = CLOCK_TIMESTAMP(),
                version = version + 1,
                updated_at = CLOCK_TIMESTAMP()
            WHERE tenant_id = $2
              AND id = $3
              AND status = 'running'
              AND version = $4
            "#,
        )
        .bind(response_message_id)
        .bind(tenant_id)
        .bind(run.id)
        .bind(run.version)
        .execute(&mut *transaction)
        .await?;
        if run_update.rows_affected() != 1 {
            return Err(AgentSessionError::LeaseLost);
        }
        finish_queue(&mut transaction, tenant_id, lease).await?;
        append_run_event(
            &mut transaction,
            tenant_id,
            run.id,
            RunEventType::MessageCreated,
        )
        .await?;
        append_run_event(&mut transaction, tenant_id, run.id, RunEventType::Completed).await?;
        append_worker_audit(
            &mut transaction,
            tenant_id,
            run.requested_by,
            RequestContext::from_ids(run.request_id, run.correlation_id),
            "agent.runs.complete",
            run.id,
            AuditOutcome::Succeeded,
            None,
        )
        .await?;
        transaction.commit().await?;
        self.read_run(tenant_id, run.requested_by, run.id).await
    }

    /// Fails a fenced run with only a bounded, user-safe category and message.
    pub async fn fail_run(
        &self,
        tenant_id: Uuid,
        lease: &RunLease,
        failure: SafeRunFailure,
    ) -> Result<AgentRun, AgentSessionError> {
        let mut transaction = self.pool.begin().await?;
        let queue = lock_leased_queue(&mut transaction, tenant_id, lease).await?;
        if queue.cancel_requested_at.is_some() {
            return Err(AgentSessionError::conflict(
                "run_cancel_requested",
                "Cancellation was requested for this Agent run",
            ));
        }
        let run = lock_run(&mut transaction, tenant_id, lease.run_id).await?;
        if RunStatus::from_str(&run.status)? != RunStatus::Running {
            return Err(AgentSessionError::conflict(
                "run_not_running",
                "Only a running Agent run can fail",
            ));
        }
        terminalize_running_children(
            &mut transaction,
            tenant_id,
            run.id,
            ExecutionTerminal::Interrupted(&failure.code),
        )
        .await?;
        transition_run_to_failure(
            &mut transaction,
            tenant_id,
            &run,
            RunStatus::Failed,
            &failure.code,
            &failure.message,
        )
        .await?;
        finish_queue(&mut transaction, tenant_id, lease).await?;
        append_run_event(&mut transaction, tenant_id, run.id, RunEventType::Failed).await?;
        append_worker_audit(
            &mut transaction,
            tenant_id,
            run.requested_by,
            RequestContext::from_ids(run.request_id, run.correlation_id),
            "agent.runs.fail",
            run.id,
            AuditOutcome::Failed,
            Some(failure.code),
        )
        .await?;
        transaction.commit().await?;
        self.read_run(tenant_id, run.requested_by, run.id).await
    }

    /// Recovers expired leases, replaying only idempotent checkpoints and interrupting all others.
    pub async fn recover_expired_runs(
        &self,
        tenant_id: Uuid,
        limit: u16,
    ) -> Result<RecoverySummary, AgentSessionError> {
        let limit = recovery_limit(limit)?;
        let mut transaction = self.pool.begin().await?;
        let expired = sqlx::query_as::<_, ExpiredQueueRow>(
            r#"
            SELECT tenant_id, run_id, checkpoint, delivery_attempt,
                   cancel_requested_at IS NOT NULL AS cancel_requested, version
            FROM agent_run_queue
            WHERE tenant_id = $1
              AND state = 'leased'
              AND lease_expires_at <= STATEMENT_TIMESTAMP()
              AND deleted_at IS NULL
            ORDER BY lease_expires_at, run_id
            FOR UPDATE SKIP LOCKED
            LIMIT $2
            "#,
        )
        .bind(tenant_id)
        .bind(limit)
        .fetch_all(&mut *transaction)
        .await?;
        let (summary, _) = recover_candidates(&mut transaction, expired).await?;
        transaction.commit().await?;
        Ok(summary)
    }

    /// Recovers a tenant-fair global batch and returns replayable usage cleanup work.
    pub async fn recover_expired_runs_globally(
        &self,
        limit: u16,
    ) -> Result<GlobalRecoveryBatch, AgentSessionError> {
        let limit = recovery_limit(limit)?;
        let mut transaction = self.pool.begin().await?;
        let expired = sqlx::query_as::<_, ExpiredQueueRow>(
            r#"
            WITH ranked AS MATERIALIZED (
                SELECT tenant_id, run_id, checkpoint, delivery_attempt,
                       cancel_requested_at IS NOT NULL AS cancel_requested,
                       version, lease_expires_at,
                       ROW_NUMBER() OVER (
                           PARTITION BY tenant_id
                           ORDER BY lease_expires_at, run_id
                       ) AS tenant_rank
                FROM agent_run_queue
                WHERE state = 'leased'
                  AND lease_expires_at <= STATEMENT_TIMESTAMP()
                  AND deleted_at IS NULL
            )
            SELECT queue.tenant_id, queue.run_id, queue.checkpoint,
                   queue.delivery_attempt,
                   queue.cancel_requested_at IS NOT NULL AS cancel_requested,
                   queue.version
            FROM ranked
            INNER JOIN agent_run_queue AS queue
              ON queue.tenant_id = ranked.tenant_id
             AND queue.run_id = ranked.run_id
             AND queue.version = ranked.version
            ORDER BY ranked.tenant_rank, ranked.lease_expires_at,
                     ranked.tenant_id, ranked.run_id
            FOR UPDATE OF queue SKIP LOCKED
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&mut *transaction)
        .await?;
        let (summary, runs) = recover_candidates(&mut transaction, expired).await?;
        let pending_usage_reservations =
            load_pending_recovery_usage(&mut transaction, limit).await?;
        transaction.commit().await?;
        Ok(GlobalRecoveryBatch {
            summary,
            runs,
            pending_usage_reservations,
        })
    }
}

fn recovery_limit(limit: u16) -> Result<i64, AgentSessionError> {
    if limit == 0 || limit > 100 {
        return Err(AgentSessionError::invalid(
            "invalid_recovery_limit",
            "Recovery batch must be between 1 and 100",
        ));
    }
    Ok(i64::from(limit))
}

async fn recover_candidates(
    transaction: &mut Transaction<'_, Postgres>,
    expired: Vec<ExpiredQueueRow>,
) -> Result<(RecoverySummary, Vec<RecoveredRun>), AgentSessionError> {
    let mut summary = RecoverySummary {
        requeued: 0,
        interrupted: 0,
        cancelled: 0,
    };
    let mut recovered = Vec::with_capacity(expired.len());
    for queue in expired {
        let tenant_id = queue.tenant_id;
        let checkpoint = RunCheckpoint::from_str(&queue.checkpoint)?;
        if queue.cancel_requested {
            let run = lock_run(transaction, tenant_id, queue.run_id).await?;
            if RunStatus::from_str(&run.status)? != RunStatus::Running {
                return Err(AgentSessionError::storage_contract());
            }
            terminalize_running_children(
                transaction,
                tenant_id,
                run.id,
                ExecutionTerminal::Cancelled,
            )
            .await?;
            transition_run_to_cancelled(transaction, tenant_id, run.id, &run).await?;
            finish_expired_queue(transaction, tenant_id, run.id, queue.version).await?;
            append_run_event(transaction, tenant_id, run.id, RunEventType::Cancelled).await?;
            append_system_cancellation_audit(
                transaction,
                tenant_id,
                RequestContext::from_ids(run.request_id, run.correlation_id),
                run.id,
            )
            .await?;
            summary.cancelled += 1;
            recovered.push(RecoveredRun {
                tenant_id,
                run_id: run.id,
                disposition: ExpiredLeaseRecoveryDisposition::Cancelled,
            });
            continue;
        }
        let has_finalizing_evidence = checkpoint == RunCheckpoint::Finalizing
            && finalizing_evidence_exists(transaction, tenant_id, queue.run_id).await?;
        if (checkpoint.is_automatically_recoverable() || has_finalizing_evidence)
            && queue.delivery_attempt < MAX_DELIVERY_ATTEMPTS
        {
            let update = sqlx::query(
                r#"
                UPDATE agent_run_queue
                SET state = 'available',
                    available_at = CLOCK_TIMESTAMP(),
                    lease_token = NULL,
                    leased_by = NULL,
                    lease_expires_at = NULL,
                    heartbeat_at = NULL,
                    version = version + 1,
                    updated_at = CLOCK_TIMESTAMP()
                WHERE tenant_id = $1
                  AND run_id = $2
                  AND state = 'leased'
                  AND version = $3
                  AND lease_expires_at <= STATEMENT_TIMESTAMP()
                "#,
            )
            .bind(tenant_id)
            .bind(queue.run_id)
            .bind(queue.version)
            .execute(&mut **transaction)
            .await?;
            if update.rows_affected() != 1 {
                return Err(AgentSessionError::storage_contract());
            }
            summary.requeued += 1;
            recovered.push(RecoveredRun {
                tenant_id,
                run_id: queue.run_id,
                disposition: ExpiredLeaseRecoveryDisposition::Requeued,
            });
            continue;
        }

        let run = lock_run(transaction, tenant_id, queue.run_id).await?;
        let status = RunStatus::from_str(&run.status)?;
        if status.is_terminal() {
            return Err(AgentSessionError::storage_contract());
        }
        let (failure_code, failure_message) = if queue.delivery_attempt >= MAX_DELIVERY_ATTEMPTS {
            (
                "delivery_attempts_exhausted",
                "This Agent run could not be recovered after three deliveries",
            )
        } else {
            (
                "unsafe_checkpoint_interrupted",
                "This Agent run stopped during an operation that cannot be replayed safely",
            )
        };
        terminalize_running_children(
            transaction,
            tenant_id,
            run.id,
            ExecutionTerminal::Interrupted(failure_code),
        )
        .await?;
        transition_run_to_failure(
            transaction,
            tenant_id,
            &run,
            RunStatus::Interrupted,
            failure_code,
            failure_message,
        )
        .await?;
        let queue_update = sqlx::query(
            r#"
            UPDATE agent_run_queue
            SET state = 'finished',
                lease_token = NULL,
                leased_by = NULL,
                lease_expires_at = NULL,
                heartbeat_at = NULL,
                finished_at = CLOCK_TIMESTAMP(),
                version = version + 1,
                updated_at = CLOCK_TIMESTAMP()
            WHERE tenant_id = $1
              AND run_id = $2
              AND state = 'leased'
              AND version = $3
              AND lease_expires_at <= STATEMENT_TIMESTAMP()
            "#,
        )
        .bind(tenant_id)
        .bind(run.id)
        .bind(queue.version)
        .execute(&mut **transaction)
        .await?;
        if queue_update.rows_affected() != 1 {
            return Err(AgentSessionError::storage_contract());
        }
        append_run_event(transaction, tenant_id, run.id, RunEventType::Interrupted).await?;
        append_system_audit(
            transaction,
            tenant_id,
            RequestContext::from_ids(run.request_id, run.correlation_id),
            run.id,
            failure_code,
        )
        .await?;
        summary.interrupted += 1;
        recovered.push(RecoveredRun {
            tenant_id,
            run_id: run.id,
            disposition: ExpiredLeaseRecoveryDisposition::Interrupted,
        });
    }
    Ok((summary, recovered))
}

async fn load_pending_recovery_usage(
    transaction: &mut Transaction<'_, Postgres>,
    limit: i64,
) -> Result<Vec<RecoveryUsageReservation>, AgentSessionError> {
    let rows = sqlx::query_as::<_, PendingRecoveryUsageRow>(
        r#"
        WITH eligible AS MATERIALIZED (
            SELECT reservation.tenant_id, reservation.id, reservation.created_at,
                   ROW_NUMBER() OVER (
                       PARTITION BY reservation.tenant_id
                       ORDER BY reservation.created_at, reservation.id
                   ) AS tenant_rank
            FROM agent_limit_reservations AS reservation
            INNER JOIN agent_runs AS run
              ON run.tenant_id = reservation.tenant_id
             AND run.id = reservation.run_id
             AND run.status IN ('cancelled', 'interrupted')
             AND run.deleted_at IS NULL
            LEFT JOIN agent_provider_attempts AS attempt
              ON attempt.tenant_id = reservation.tenant_id
             AND attempt.run_id = reservation.run_id
             AND attempt.id = reservation.provider_attempt_id
            LEFT JOIN agent_capability_calls AS call
              ON call.tenant_id = reservation.tenant_id
             AND call.run_id = reservation.run_id
             AND call.id = reservation.capability_call_id
            WHERE reservation.status IN ('reserved', 'not_limited')
              AND reservation.deleted_at IS NULL
              AND (
                  reservation.status = 'reserved'
                  OR NOT EXISTS (
                      SELECT 1
                      FROM agent_usage_events AS usage_event
                      WHERE usage_event.tenant_id = reservation.tenant_id
                        AND usage_event.limit_reservation_id = reservation.id
                  )
              )
              AND (
                  (reservation.stage_kind = 'provider_attempt'
                   AND attempt.status IN ('cancelled', 'interrupted'))
                  OR
                  (reservation.stage_kind = 'capability_call'
                   AND call.status IN ('cancelled', 'interrupted'))
              )
        )
        SELECT reservation.tenant_id, reservation.run_id,
               reservation.id AS reservation_id, reservation.stage_kind,
               reservation.provider_attempt_id, reservation.capability_call_id,
               reservation.status AS reservation_status,
               reservation.claimed_at IS NOT NULL AS claimed
        FROM eligible
        INNER JOIN agent_limit_reservations AS reservation
          ON reservation.tenant_id = eligible.tenant_id
         AND reservation.id = eligible.id
         AND reservation.status IN ('reserved', 'not_limited')
         AND reservation.deleted_at IS NULL
        ORDER BY eligible.tenant_rank, eligible.created_at,
                 eligible.tenant_id, eligible.id
        FOR UPDATE OF reservation SKIP LOCKED
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(&mut **transaction)
    .await?;
    rows.into_iter().map(TryInto::try_into).collect()
}

async fn claim_candidates(
    transaction: &mut Transaction<'_, Postgres>,
    command: &ClaimRunsCommand,
    candidates: Vec<QueueCandidateRow>,
) -> Result<Vec<ClaimedRun>, AgentSessionError> {
    let mut claimed = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let token = Uuid::new_v4();
        let queue = sqlx::query_as::<_, ClaimedQueueRow>(
            r#"
            WITH lease_clock AS (
                SELECT STATEMENT_TIMESTAMP() AS captured_at
            )
            UPDATE agent_run_queue q
            SET state = 'leased',
                lease_token = $1,
                leased_by = $2,
                heartbeat_at = lease_clock.captured_at,
                lease_expires_at = lease_clock.captured_at + INTERVAL '30 seconds',
                delivery_attempt = delivery_attempt + 1,
                version = version + 1,
                updated_at = lease_clock.captured_at
            FROM lease_clock
            WHERE q.tenant_id = $3
              AND q.run_id = $4
              AND q.state = 'available'
              AND q.version = $5
              AND q.deleted_at IS NULL
            RETURNING run_id, lease_expires_at, delivery_attempt, checkpoint, version
            "#,
        )
        .bind(token)
        .bind(&command.worker_id)
        .bind(candidate.tenant_id)
        .bind(candidate.run_id)
        .bind(candidate.version)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or(AgentSessionError::LeaseLost)?;

        let run = sqlx::query_as::<_, WorkerRunRow>(
            r#"
            SELECT r.id, r.thread_id, r.request_message_id, r.requested_by,
                   m.content AS request_message, r.task_class, r.origin_module_key,
                   r.origin_route, r.correlation_id, r.status
            FROM agent_runs r
            JOIN agent_messages m
              ON m.tenant_id = r.tenant_id
             AND m.thread_id = r.thread_id
             AND m.id = r.request_message_id
            WHERE r.tenant_id = $1
              AND r.id = $2
              AND r.deleted_at IS NULL
              AND m.deleted_at IS NULL
            FOR UPDATE OF r
            "#,
        )
        .bind(candidate.tenant_id)
        .bind(candidate.run_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(AgentSessionError::storage_contract)?;
        let run_status = RunStatus::from_str(&run.status)?;
        if run_status == RunStatus::Queued {
            let transition = sqlx::query(
                r#"
                UPDATE agent_runs
                SET status = 'running',
                    started_at = CLOCK_TIMESTAMP(),
                    version = version + 1,
                    updated_at = CLOCK_TIMESTAMP()
                WHERE tenant_id = $1 AND id = $2 AND status = 'queued'
                "#,
            )
            .bind(candidate.tenant_id)
            .bind(run.id)
            .execute(&mut **transaction)
            .await?;
            if transition.rows_affected() != 1 {
                return Err(AgentSessionError::storage_contract());
            }
            append_run_event(
                transaction,
                candidate.tenant_id,
                run.id,
                RunEventType::Started,
            )
            .await?;
        } else if run_status != RunStatus::Running {
            return Err(AgentSessionError::storage_contract());
        }
        claimed.push(ClaimedRun {
            tenant_id: candidate.tenant_id,
            lease: RunLease {
                run_id: run.id,
                worker_id: command.worker_id.clone(),
                lease_token: token,
                fence_version: queue.version,
            },
            session_id: run.thread_id,
            requested_by: run.requested_by,
            request_message_id: run.request_message_id,
            request_message: run.request_message,
            task_class: parse_task_class(&run.task_class)?,
            origin_module_key: run.origin_module_key,
            origin_route: run.origin_route,
            correlation_id: run.correlation_id,
            delivery_attempt: queue.delivery_attempt,
            checkpoint: RunCheckpoint::from_str(&queue.checkpoint)?,
            lease_expires_at: queue.lease_expires_at,
        });
    }
    Ok(claimed)
}

#[derive(Debug, FromRow)]
struct SessionRow {
    id: Uuid,
    title: String,
    status: String,
    version: i64,
    last_activity_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<SessionRow> for AgentSession {
    type Error = AgentSessionError;

    fn try_from(row: SessionRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            title: row.title,
            status: SessionStatus::from_str(&row.status)?,
            version: row.version,
            last_activity_at: row.last_activity_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

#[derive(Debug, FromRow)]
struct MessageRow {
    id: Uuid,
    thread_id: Uuid,
    sequence: i64,
    role: String,
    content: String,
    created_at: DateTime<Utc>,
}

impl TryFrom<MessageRow> for AgentMessage {
    type Error = AgentSessionError;

    fn try_from(row: MessageRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            session_id: row.thread_id,
            sequence: row.sequence,
            role: MessageRole::from_str(&row.role)?,
            content: row.content,
            created_at: row.created_at,
        })
    }
}

#[derive(Debug, Clone, FromRow)]
struct RunRow {
    id: Uuid,
    thread_id: Uuid,
    request_message_id: Uuid,
    response_message_id: Option<Uuid>,
    task_class: String,
    origin_module_key: String,
    origin_route: String,
    status: String,
    safe_failure_code: Option<String>,
    safe_failure_message: Option<String>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    version: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<RunRow> for AgentRun {
    type Error = AgentSessionError;

    fn try_from(row: RunRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            session_id: row.thread_id,
            request_message_id: row.request_message_id,
            response_message_id: row.response_message_id,
            task_class: parse_task_class(&row.task_class)?,
            origin_module_key: row.origin_module_key,
            origin_route: row.origin_route,
            status: RunStatus::from_str(&row.status)?,
            safe_failure_code: row.safe_failure_code,
            safe_failure_message: row.safe_failure_message,
            started_at: row.started_at,
            finished_at: row.finished_at,
            version: row.version,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

#[derive(Debug, FromRow)]
struct EventRow {
    id: i64,
    run_id: Uuid,
    event_type: String,
    created_at: DateTime<Utc>,
}

impl TryFrom<EventRow> for AgentRunEvent {
    type Error = AgentSessionError;

    fn try_from(row: EventRow) -> Result<Self, Self::Error> {
        Ok(Self {
            cursor: row.id.to_string(),
            run_id: row.run_id,
            event_type: RunEventType::from_str(&row.event_type)?,
            created_at: row.created_at,
        })
    }
}

#[derive(Debug, FromRow)]
struct LockedThreadRow {
    status: String,
    next_message_sequence: i64,
    version: i64,
}

#[derive(Debug, FromRow)]
struct IdempotencyRow {
    request_fingerprint: Vec<u8>,
    result_kind: String,
    result_id: Uuid,
}

#[derive(Debug, FromRow)]
struct QueueLockRow {
    state: String,
    cancel_requested_at: Option<DateTime<Utc>>,
    version: i64,
}

#[derive(Debug, FromRow)]
struct LockedRunRow {
    id: Uuid,
    thread_id: Uuid,
    response_message_id: Option<Uuid>,
    requested_by: Uuid,
    request_id: Uuid,
    correlation_id: Uuid,
    status: String,
    version: i64,
}

#[derive(Debug, FromRow)]
struct CompletionQueueRow {
    state: String,
    checkpoint: String,
    cancel_requested_at: Option<DateTime<Utc>>,
    lease_token: Option<Uuid>,
    leased_by: Option<String>,
    version: i64,
    lease_current: bool,
}

#[derive(Debug, FromRow)]
struct FinalResponseEvidenceRow {
    plaintext_sha256: Vec<u8>,
    plaintext_length: i32,
}

#[derive(Debug, FromRow)]
struct QueueCandidateRow {
    tenant_id: Uuid,
    run_id: Uuid,
    version: i64,
}

#[derive(Debug, FromRow)]
struct ClaimedQueueRow {
    lease_expires_at: DateTime<Utc>,
    delivery_attempt: i16,
    checkpoint: String,
    version: i64,
}

#[derive(Debug, FromRow)]
struct WorkerRunRow {
    id: Uuid,
    thread_id: Uuid,
    request_message_id: Uuid,
    requested_by: Uuid,
    request_message: String,
    task_class: String,
    origin_module_key: String,
    origin_route: String,
    correlation_id: Uuid,
    status: String,
}

#[derive(Debug, FromRow)]
struct HeartbeatRow {
    version: i64,
    lease_expires_at: DateTime<Utc>,
    cancel_requested: bool,
}

#[cfg(test)]
#[derive(Debug, FromRow)]
struct CheckpointRow {
    checkpoint: String,
    cancel_requested: bool,
}

#[derive(Debug, FromRow)]
struct ExpiredQueueRow {
    tenant_id: Uuid,
    run_id: Uuid,
    checkpoint: String,
    delivery_attempt: i16,
    cancel_requested: bool,
    version: i64,
}

#[derive(Debug, FromRow)]
struct PendingRecoveryUsageRow {
    tenant_id: Uuid,
    run_id: Uuid,
    reservation_id: Uuid,
    stage_kind: String,
    provider_attempt_id: Option<Uuid>,
    capability_call_id: Option<Uuid>,
    reservation_status: String,
    claimed: bool,
}

impl TryFrom<PendingRecoveryUsageRow> for RecoveryUsageReservation {
    type Error = AgentSessionError;

    fn try_from(row: PendingRecoveryUsageRow) -> Result<Self, Self::Error> {
        let stage = match (
            row.stage_kind.as_str(),
            row.provider_attempt_id,
            row.capability_call_id,
        ) {
            ("provider_attempt", Some(attempt_id), None) => {
                RecoveryUsageStage::ProviderAttempt { attempt_id }
            }
            ("capability_call", None, Some(call_id)) => {
                RecoveryUsageStage::CapabilityCall { call_id }
            }
            _ => return Err(AgentSessionError::storage_contract()),
        };
        let action = match (row.reservation_status.as_str(), row.claimed) {
            ("reserved", false) => RecoveryUsageAction::ExpireUnclaimed,
            ("reserved", true) | ("not_limited", _) => RecoveryUsageAction::CommitTerminal,
            _ => return Err(AgentSessionError::storage_contract()),
        };
        Ok(Self {
            tenant_id: row.tenant_id,
            run_id: row.run_id,
            reservation_id: row.reservation_id,
            stage,
            action,
        })
    }
}

fn page_sessions(
    mut rows: Vec<SessionRow>,
    limit: usize,
) -> Result<CursorPage<AgentSession, SessionCursor>, AgentSessionError> {
    let has_more = rows.len() > limit;
    rows.truncate(limit);
    let next_cursor = has_more.then(|| {
        rows.last().map(|row| SessionCursor {
            last_activity_at: row.last_activity_at,
            session_id: row.id,
        })
    });
    Ok(CursorPage {
        items: rows
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?,
        next_cursor: next_cursor.flatten(),
    })
}

fn page_messages(
    mut rows: Vec<MessageRow>,
    limit: usize,
) -> Result<CursorPage<AgentMessage, MessageCursor>, AgentSessionError> {
    let has_more = rows.len() > limit;
    rows.truncate(limit);
    let next_cursor = has_more.then(|| {
        rows.last().map(|row| MessageCursor {
            sequence: row.sequence,
            message_id: row.id,
        })
    });
    Ok(CursorPage {
        items: rows
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?,
        next_cursor: next_cursor.flatten(),
    })
}

fn page_runs(
    mut rows: Vec<RunRow>,
    limit: usize,
) -> Result<CursorPage<AgentRun, RunCursor>, AgentSessionError> {
    let has_more = rows.len() > limit;
    rows.truncate(limit);
    let next_cursor = has_more.then(|| {
        rows.last().map(|row| RunCursor {
            created_at: row.created_at,
            run_id: row.id,
        })
    });
    Ok(CursorPage {
        items: rows
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?,
        next_cursor: next_cursor.flatten(),
    })
}

fn page_events(
    mut rows: Vec<EventRow>,
    limit: usize,
) -> Result<CursorPage<AgentRunEvent, EventCursor>, AgentSessionError> {
    let has_more = rows.len() > limit;
    rows.truncate(limit);
    let next_cursor = has_more.then(|| rows.last().map(|row| EventCursor(row.id)));
    Ok(CursorPage {
        items: rows
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?,
        next_cursor: next_cursor.flatten(),
    })
}

async fn ensure_owned_session(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
    session_id: Uuid,
) -> Result<(), AgentSessionError> {
    let exists = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM agent_threads t
            JOIN agent_thread_members m
              ON m.tenant_id = t.tenant_id
             AND m.thread_id = t.id
             AND m.user_id = $2
             AND m.membership_role = 'owner'
             AND m.deleted_at IS NULL
            WHERE t.tenant_id = $1
              AND t.id = $3
              AND t.owner_user_id = $2
              AND t.deleted_at IS NULL
        )
        "#,
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(session_id)
    .fetch_one(pool)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(AgentSessionError::SessionNotFound)
    }
}

async fn lock_owned_session(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    user_id: Uuid,
    session_id: Uuid,
) -> Result<LockedThreadRow, AgentSessionError> {
    sqlx::query_as::<_, LockedThreadRow>(
        r#"
        SELECT t.status, t.next_message_sequence, t.version
        FROM agent_threads t
        JOIN agent_thread_members m
          ON m.tenant_id = t.tenant_id
         AND m.thread_id = t.id
         AND m.user_id = $2
         AND m.membership_role = 'owner'
         AND m.deleted_at IS NULL
        WHERE t.tenant_id = $1
          AND t.id = $3
          AND t.owner_user_id = $2
          AND t.deleted_at IS NULL
        FOR UPDATE OF t
        "#,
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(session_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(AgentSessionError::SessionNotFound)
}

async fn lock_thread_for_worker(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    session_id: Uuid,
) -> Result<LockedThreadRow, AgentSessionError> {
    sqlx::query_as::<_, LockedThreadRow>(
        r#"
        SELECT status, next_message_sequence, version
        FROM agent_threads
        WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(session_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(AgentSessionError::storage_contract)
}

fn require_active_session(thread: &LockedThreadRow) -> Result<(), AgentSessionError> {
    match SessionStatus::from_str(&thread.status)? {
        SessionStatus::Active => Ok(()),
        SessionStatus::Archived => Err(AgentSessionError::conflict(
            "session_archived",
            "Archived Agent Sessions cannot be changed",
        )),
    }
}

async fn active_run_exists(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    session_id: Uuid,
) -> Result<bool, AgentSessionError> {
    Ok(sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM agent_runs
            WHERE tenant_id = $1
              AND thread_id = $2
              AND status IN ('queued', 'running', 'awaiting_approval')
              AND deleted_at IS NULL
        )
        "#,
    )
    .bind(tenant_id)
    .bind(session_id)
    .fetch_one(&mut **transaction)
    .await?)
}

async fn lock_idempotency_key(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    user_id: Uuid,
    operation_key: &str,
    scope_id: Option<Uuid>,
    idempotency_key: &str,
) -> Result<(), AgentSessionError> {
    let lock_key = format!(
        "{tenant_id}:{user_id}:{operation_key}:{}:{idempotency_key}",
        scope_id.unwrap_or(Uuid::nil())
    );
    sqlx::query("SELECT PG_ADVISORY_XACT_LOCK(HASHTEXTEXTENDED($1, 0::BIGINT))")
        .bind(lock_key)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn resolve_idempotency(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    user_id: Uuid,
    operation_key: &str,
    scope_id: Option<Uuid>,
    idempotency_key: &str,
    fingerprint: &[u8; 32],
    result_kind: &str,
) -> Result<Option<Uuid>, AgentSessionError> {
    let row = sqlx::query_as::<_, IdempotencyRow>(
        r#"
        SELECT request_fingerprint, result_kind, result_id
        FROM agent_request_idempotency
        WHERE tenant_id = $1
          AND user_id = $2
          AND operation_key = $3
          AND scope_id IS NOT DISTINCT FROM $4
          AND idempotency_key = $5
          AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(operation_key)
    .bind(scope_id)
    .bind(idempotency_key)
    .fetch_optional(&mut **transaction)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    if row.request_fingerprint.as_slice() != fingerprint || row.result_kind != result_kind {
        return Err(AgentSessionError::conflict(
            "idempotency_conflict",
            "This idempotency key was already used for a different request",
        ));
    }
    Ok(Some(row.result_id))
}

#[allow(clippy::too_many_arguments)]
async fn insert_idempotency(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    user_id: Uuid,
    operation_key: &str,
    scope_id: Option<Uuid>,
    idempotency_key: &str,
    fingerprint: &[u8; 32],
    result_kind: &str,
    result_id: Uuid,
) -> Result<(), AgentSessionError> {
    sqlx::query(
        r#"
        INSERT INTO agent_request_idempotency (
            tenant_id, user_id, operation_key, scope_id, idempotency_key,
            request_fingerprint, result_kind, result_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(operation_key)
    .bind(scope_id)
    .bind(idempotency_key)
    .bind(fingerprint.as_slice())
    .bind(result_kind)
    .bind(result_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn append_run_event(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
    event_type: RunEventType,
) -> Result<(), AgentSessionError> {
    sqlx::query(
        r#"
        INSERT INTO agent_run_events (tenant_id, run_id, event_type, payload)
        VALUES ($1, $2, $3, '{}'::JSONB)
        "#,
    )
    .bind(tenant_id)
    .bind(run_id)
    .bind(event_type.as_str())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn append_person_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    user_id: Uuid,
    request_context: RequestContext,
    action_key: &'static str,
    target_kind: &'static str,
    target_id: Uuid,
    run_id: Option<Uuid>,
    metadata: Map<String, Value>,
) -> Result<(), AgentSessionError> {
    let mut event = NewAuditEvent::new(
        tenant_id,
        AuditActor::person(user_id),
        action_key,
        AuditOutcome::Succeeded,
        request_context,
    )
    .with_target(AuditTarget::new(target_kind, target_id))
    .with_redacted_metadata(metadata);
    if let Some(run_id) = run_id {
        event = event.with_agent_run_id(run_id);
    }
    cp_audit::append(&mut **transaction, &event).await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn append_worker_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    requested_by: Uuid,
    request_context: RequestContext,
    action_key: &'static str,
    run_id: Uuid,
    outcome: AuditOutcome,
    reason: Option<String>,
) -> Result<(), AgentSessionError> {
    let mut event = NewAuditEvent::new(
        tenant_id,
        AuditActor::agent(requested_by),
        action_key,
        outcome,
        request_context,
    )
    .with_target(AuditTarget::new("agent_run", run_id))
    .with_agent_run_id(run_id);
    if let Some(reason) = reason {
        event = event.with_reason(reason);
    }
    cp_audit::append(&mut **transaction, &event).await?;
    Ok(())
}

async fn append_system_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    request_context: RequestContext,
    run_id: Uuid,
    reason: &str,
) -> Result<(), AgentSessionError> {
    let event = NewAuditEvent::new(
        tenant_id,
        AuditActor::system(),
        "agent.runs.interrupt",
        AuditOutcome::Failed,
        request_context,
    )
    .with_target(AuditTarget::new("agent_run", run_id))
    .with_agent_run_id(run_id)
    .with_reason(reason);
    cp_audit::append(&mut **transaction, &event).await?;
    Ok(())
}

async fn append_system_cancellation_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    request_context: RequestContext,
    run_id: Uuid,
) -> Result<(), AgentSessionError> {
    let event = NewAuditEvent::new(
        tenant_id,
        AuditActor::system(),
        "agent.runs.cancel.recover",
        AuditOutcome::Succeeded,
        request_context,
    )
    .with_target(AuditTarget::new("agent_run", run_id))
    .with_agent_run_id(run_id)
    .with_reason("cancelled_lease_expired");
    cp_audit::append(&mut **transaction, &event).await?;
    Ok(())
}

async fn lock_owned_queue(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    user_id: Uuid,
    run_id: Uuid,
) -> Result<QueueLockRow, AgentSessionError> {
    sqlx::query_as::<_, QueueLockRow>(
        r#"
        SELECT q.state, q.cancel_requested_at, q.version
        FROM agent_run_queue q
        JOIN agent_runs r
          ON r.tenant_id = q.tenant_id AND r.id = q.run_id
        JOIN agent_threads t
          ON t.tenant_id = r.tenant_id AND t.id = r.thread_id
        JOIN agent_thread_members m
          ON m.tenant_id = t.tenant_id
         AND m.thread_id = t.id
         AND m.user_id = $2
         AND m.membership_role = 'owner'
         AND m.deleted_at IS NULL
        WHERE q.tenant_id = $1
          AND q.run_id = $3
          AND r.requested_by = $2
          AND t.owner_user_id = $2
          AND q.deleted_at IS NULL
          AND r.deleted_at IS NULL
          AND t.deleted_at IS NULL
        FOR UPDATE OF q
        "#,
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(run_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(AgentSessionError::RunNotFound)
}

async fn lock_leased_queue(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    lease: &RunLease,
) -> Result<QueueLockRow, AgentSessionError> {
    sqlx::query_as::<_, QueueLockRow>(
        r#"
        SELECT state, cancel_requested_at, version
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

async fn lock_completion_queue(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
) -> Result<CompletionQueueRow, AgentSessionError> {
    sqlx::query_as::<_, CompletionQueueRow>(
        r#"
        SELECT state, checkpoint, cancel_requested_at, lease_token, leased_by, version,
               COALESCE(lease_expires_at > STATEMENT_TIMESTAMP(), FALSE) AS lease_current
        FROM agent_run_queue
        WHERE tenant_id = $1 AND run_id = $2 AND deleted_at IS NULL
        FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(run_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(AgentSessionError::LeaseLost)
}

async fn verify_final_response_evidence(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
    artifact_id: Uuid,
    plaintext: &FinalResponsePlaintext,
) -> Result<(), AgentSessionError> {
    let evidence = sqlx::query_as::<_, FinalResponseEvidenceRow>(
        r#"
        SELECT a.plaintext_sha256, a.plaintext_length
        FROM agent_execution_artifacts a
        JOIN agent_execution_steps s
          ON s.tenant_id = a.tenant_id
         AND s.run_id = a.run_id
         AND s.id = a.step_id
        WHERE a.tenant_id = $1
          AND a.run_id = $2
          AND a.id = $3
          AND a.artifact_kind = 'final_response'
          AND s.step_kind = 'finalize'
          AND s.status = 'succeeded'
          AND a.deleted_at IS NULL
          AND s.deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(run_id)
    .bind(artifact_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| {
        AgentSessionError::conflict(
            "final_response_conflict",
            "This final response does not match the run's durable evidence",
        )
    })?;
    let expected_hash = plaintext.sha256();
    let expected_length =
        i32::try_from(plaintext.byte_len()).map_err(|_| AgentSessionError::storage_contract())?;
    if evidence.plaintext_sha256.as_slice() == expected_hash
        && evidence.plaintext_length == expected_length
    {
        Ok(())
    } else {
        Err(AgentSessionError::conflict(
            "final_response_conflict",
            "This final response does not match the run's durable evidence",
        ))
    }
}

async fn lock_run(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
) -> Result<LockedRunRow, AgentSessionError> {
    sqlx::query_as::<_, LockedRunRow>(
        r#"
        SELECT id, thread_id, response_message_id, requested_by, request_id,
               correlation_id, status, version
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

async fn transition_run_to_cancelled(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
    run: &LockedRunRow,
) -> Result<(), AgentSessionError> {
    let update = sqlx::query(
        r#"
        UPDATE agent_runs
        SET status = 'cancelled',
            finished_at = CLOCK_TIMESTAMP(),
            version = version + 1,
            updated_at = CLOCK_TIMESTAMP()
        WHERE tenant_id = $1
          AND id = $2
          AND status IN ('queued', 'running', 'awaiting_approval')
          AND version = $3
        "#,
    )
    .bind(tenant_id)
    .bind(run_id)
    .bind(run.version)
    .execute(&mut **transaction)
    .await?;
    if update.rows_affected() == 1 {
        Ok(())
    } else {
        Err(AgentSessionError::conflict(
            "run_state_changed",
            "This Agent run changed while cancellation was requested",
        ))
    }
}

async fn transition_run_to_failure(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run: &LockedRunRow,
    status: RunStatus,
    failure_code: &str,
    failure_message: &str,
) -> Result<(), AgentSessionError> {
    if !matches!(status, RunStatus::Failed | RunStatus::Interrupted) {
        return Err(AgentSessionError::storage_contract());
    }
    let update = sqlx::query(
        r#"
        UPDATE agent_runs
        SET status = $1,
            safe_failure_code = $2,
            safe_failure_message = $3,
            finished_at = CLOCK_TIMESTAMP(),
            version = version + 1,
            updated_at = CLOCK_TIMESTAMP()
        WHERE tenant_id = $4
          AND id = $5
          AND status IN ('queued', 'running', 'awaiting_approval')
          AND version = $6
        "#,
    )
    .bind(status.as_str())
    .bind(failure_code)
    .bind(failure_message)
    .bind(tenant_id)
    .bind(run.id)
    .bind(run.version)
    .execute(&mut **transaction)
    .await?;
    if update.rows_affected() == 1 {
        Ok(())
    } else {
        Err(AgentSessionError::storage_contract())
    }
}

async fn finish_queue(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    lease: &RunLease,
) -> Result<(), AgentSessionError> {
    let update = sqlx::query(
        r#"
        UPDATE agent_run_queue
        SET state = 'finished',
            lease_token = NULL,
            leased_by = NULL,
            lease_expires_at = NULL,
            heartbeat_at = NULL,
            finished_at = CLOCK_TIMESTAMP(),
            version = version + 1,
            updated_at = CLOCK_TIMESTAMP()
        WHERE tenant_id = $1
          AND run_id = $2
          AND state = 'leased'
          AND leased_by = $3
          AND lease_token = $4
          AND version = $5
          AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(lease.run_id)
    .bind(&lease.worker_id)
    .bind(lease.lease_token)
    .bind(lease.fence_version)
    .execute(&mut **transaction)
    .await?;
    if update.rows_affected() == 1 {
        Ok(())
    } else {
        Err(AgentSessionError::LeaseLost)
    }
}

async fn finish_available_queue(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
    expected_fence: i64,
) -> Result<(), AgentSessionError> {
    let update = sqlx::query(
        r#"
        UPDATE agent_run_queue
        SET state = 'finished',
            finished_at = CLOCK_TIMESTAMP(),
            version = version + 1,
            updated_at = CLOCK_TIMESTAMP()
        WHERE tenant_id = $1
          AND run_id = $2
          AND state = 'available'
          AND cancel_requested_at IS NOT NULL
          AND version = $3
          AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(run_id)
    .bind(expected_fence)
    .execute(&mut **transaction)
    .await?;
    if update.rows_affected() == 1 {
        Ok(())
    } else {
        Err(AgentSessionError::conflict(
            "run_state_changed",
            "This Agent run changed while cancellation was requested",
        ))
    }
}

async fn finish_expired_queue(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
    expected_fence: i64,
) -> Result<(), AgentSessionError> {
    let update = sqlx::query(
        r#"
        UPDATE agent_run_queue
        SET state = 'finished',
            lease_token = NULL,
            leased_by = NULL,
            lease_expires_at = NULL,
            heartbeat_at = NULL,
            finished_at = CLOCK_TIMESTAMP(),
            version = version + 1,
            updated_at = CLOCK_TIMESTAMP()
        WHERE tenant_id = $1
          AND run_id = $2
          AND state = 'leased'
          AND version = $3
          AND lease_expires_at <= STATEMENT_TIMESTAMP()
          AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(run_id)
    .bind(expected_fence)
    .execute(&mut **transaction)
    .await?;
    if update.rows_affected() == 1 {
        Ok(())
    } else {
        Err(AgentSessionError::storage_contract())
    }
}

async fn finalizing_evidence_exists(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    run_id: Uuid,
) -> Result<bool, AgentSessionError> {
    Ok(sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM agent_execution_steps s
            JOIN agent_execution_artifacts a
              ON a.tenant_id = s.tenant_id
             AND a.run_id = s.run_id
             AND a.step_id = s.id
            WHERE s.tenant_id = $1
              AND s.run_id = $2
              AND s.step_kind = 'finalize'
              AND s.status = 'succeeded'
              AND a.artifact_kind = 'final_response'
        )
        "#,
    )
    .bind(tenant_id)
    .bind(run_id)
    .fetch_one(&mut **transaction)
    .await?)
}

async fn allocate_message_sequence(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    session_id: Uuid,
    expected_version: i64,
) -> Result<(), AgentSessionError> {
    let update = sqlx::query(
        r#"
        UPDATE agent_threads
        SET next_message_sequence = next_message_sequence + 1,
            last_activity_at = CLOCK_TIMESTAMP(),
            version = version + 1,
            updated_at = CLOCK_TIMESTAMP()
        WHERE tenant_id = $1
          AND id = $2
          AND status = 'active'
          AND version = $3
          AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(session_id)
    .bind(expected_version)
    .execute(&mut **transaction)
    .await?;
    if update.rows_affected() == 1 {
        Ok(())
    } else {
        Err(stale_session())
    }
}

fn next_lease(lease: &RunLease, fence_version: i64) -> RunLease {
    RunLease {
        run_id: lease.run_id,
        worker_id: lease.worker_id.clone(),
        lease_token: lease.lease_token,
        fence_version,
    }
}

fn parse_task_class(value: &str) -> Result<TaskClass, AgentSessionError> {
    TaskClass::from_str(value).map_err(|_| AgentSessionError::storage_contract())
}

fn stale_session() -> AgentSessionError {
    AgentSessionError::conflict(
        "stale_session_version",
        "This Agent Session changed; reload it before trying again",
    )
}

fn version_metadata(version: i64) -> Map<String, Value> {
    let mut metadata = Map::new();
    metadata.insert("session_version".to_owned(), Value::from(version));
    metadata
}

fn sequence_metadata(sequence: i64) -> Map<String, Value> {
    let mut metadata = Map::new();
    metadata.insert("message_sequence".to_owned(), Value::from(sequence));
    metadata
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use chrono::Utc;
    use cp_audit::RequestContext;
    use sha2::{Digest, Sha256};
    use sqlx::{PgPool, postgres::PgPoolOptions};
    use uuid::Uuid;

    use super::{
        EventRow, MessageRow, PendingRecoveryUsageRow, RunRow, SessionRow, page_events,
        page_messages, page_runs, page_sessions, parse_task_class, recovery_limit,
    };
    use crate::{
        AgentProviderKey, AgentSessionError, AgentSessionOps, ArchiveSessionCommand,
        CapabilityCallDuration, CapabilityCallFailure, CapabilityCallPlan, CapabilityCallScope,
        CapabilityCallStatus, CapabilityFailureStatus, ClaimRunsCommand, CreateSessionCommand,
        EncryptedExecutionArtifact, ExecutionStepSnapshot, FinalResponsePlaintext, ListEventsQuery,
        ListMessagesQuery, ListRunsQuery, ListSessionsQuery, MessageRole, NormalizedProviderUsage,
        ProviderAttemptFailure, ProviderAttemptPlan, ProviderAttemptStatus,
        ProviderPreflightFailure, ProviderTurnIndex, ProviderUpstreamFailure, RecoveryUsageAction,
        RecoveryUsageReservation, RecoveryUsageStage, RenameSessionCommand, RunCheckpoint,
        RunEventType, RunStatus, SafeRunFailure, SessionStatus, SubmitMessageCommand, TaskClass,
    };

    fn session_row(index: i64) -> SessionRow {
        SessionRow {
            id: Uuid::from_u128(index as u128 + 1),
            title: format!("Session {index}"),
            status: "active".to_owned(),
            version: 1,
            last_activity_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn run_row(index: i64) -> RunRow {
        RunRow {
            id: Uuid::from_u128(index as u128 + 20),
            thread_id: Uuid::new_v4(),
            request_message_id: Uuid::new_v4(),
            response_message_id: None,
            task_class: "module_read_reporting".to_owned(),
            origin_module_key: "sis".to_owned(),
            origin_route: "/modules/sis".to_owned(),
            status: "queued".to_owned(),
            safe_failure_code: None,
            safe_failure_message: None,
            started_at: None,
            finished_at: None,
            version: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn projection_rows_parse_only_supported_states() {
        let session = crate::AgentSession::try_from(session_row(1)).unwrap();
        assert_eq!(session.status, SessionStatus::Active);
        let message = crate::AgentMessage::try_from(MessageRow {
            id: Uuid::new_v4(),
            thread_id: Uuid::new_v4(),
            sequence: 1,
            role: "user".to_owned(),
            content: "Hello".to_owned(),
            created_at: Utc::now(),
        })
        .unwrap();
        assert_eq!(message.role, MessageRole::User);
        let run = crate::AgentRun::try_from(run_row(1)).unwrap();
        assert_eq!(run.status, RunStatus::Queued);
        assert_eq!(run.task_class, TaskClass::ModuleReadReporting);
        assert!(parse_task_class("unknown").is_err());
    }

    #[test]
    fn cursor_pages_return_only_a_bounded_page_and_last_visible_cursor() {
        let sessions = page_sessions(vec![session_row(1), session_row(2)], 1).unwrap();
        assert_eq!(sessions.items.len(), 1);
        assert!(sessions.next_cursor.is_some());
        let messages = page_messages(
            vec![
                MessageRow {
                    id: Uuid::new_v4(),
                    thread_id: Uuid::new_v4(),
                    sequence: 1,
                    role: "user".to_owned(),
                    content: "One".to_owned(),
                    created_at: Utc::now(),
                },
                MessageRow {
                    id: Uuid::new_v4(),
                    thread_id: Uuid::new_v4(),
                    sequence: 2,
                    role: "assistant".to_owned(),
                    content: "Two".to_owned(),
                    created_at: Utc::now(),
                },
            ],
            1,
        )
        .unwrap();
        assert_eq!(messages.items.len(), 1);
        assert!(messages.next_cursor.is_some());
        let runs = page_runs(vec![run_row(1), run_row(2)], 1).unwrap();
        assert_eq!(runs.items.len(), 1);
        assert!(runs.next_cursor.is_some());
        let events = page_events(
            vec![
                EventRow {
                    id: 1,
                    run_id: Uuid::new_v4(),
                    event_type: "queued".to_owned(),
                    created_at: Utc::now(),
                },
                EventRow {
                    id: 2,
                    run_id: Uuid::new_v4(),
                    event_type: "started".to_owned(),
                    created_at: Utc::now(),
                },
            ],
            1,
        )
        .unwrap();
        assert_eq!(events.items[0].event_type, RunEventType::Queued);
        assert!(events.next_cursor.is_some());
    }

    #[test]
    fn empty_and_exact_pages_have_no_next_cursor() {
        assert!(page_sessions(Vec::new(), 10).unwrap().next_cursor.is_none());
        assert!(
            page_runs(vec![run_row(1)], 1)
                .unwrap()
                .next_cursor
                .is_none()
        );
    }

    #[test]
    fn malformed_event_type_and_messages_fail_safely() {
        let error = page_events(
            vec![EventRow {
                id: 1,
                run_id: Uuid::new_v4(),
                event_type: "raw_provider_error".to_owned(),
                created_at: Utc::now(),
            }],
            1,
        )
        .unwrap_err();
        assert_eq!(error.code(), "agent_runtime_storage_error");
    }

    #[test]
    fn storage_errors_are_non_sensitive() {
        let error = AgentSessionError::storage_contract();
        assert_eq!(error.code(), "agent_runtime_storage_error");
        assert!(!error.safe_message().is_empty());
    }

    #[test]
    fn recovery_batches_are_bounded_and_reconciliation_rows_are_strictly_shaped() {
        assert_eq!(recovery_limit(1).unwrap(), 1);
        assert_eq!(recovery_limit(100).unwrap(), 100);
        assert_eq!(
            recovery_limit(0).unwrap_err().code(),
            "invalid_recovery_limit"
        );
        assert_eq!(
            recovery_limit(101).unwrap_err().code(),
            "invalid_recovery_limit"
        );

        let tenant_id = Uuid::new_v4();
        let run_id = Uuid::new_v4();
        let reservation_id = Uuid::new_v4();
        let attempt_id = Uuid::new_v4();
        let provider = RecoveryUsageReservation::try_from(PendingRecoveryUsageRow {
            tenant_id,
            run_id,
            reservation_id,
            stage_kind: "provider_attempt".to_owned(),
            provider_attempt_id: Some(attempt_id),
            capability_call_id: None,
            reservation_status: "reserved".to_owned(),
            claimed: true,
        })
        .unwrap();
        assert_eq!(provider.tenant_id, tenant_id);
        assert_eq!(provider.run_id, run_id);
        assert_eq!(provider.reservation_id, reservation_id);
        assert_eq!(
            provider.stage,
            RecoveryUsageStage::ProviderAttempt { attempt_id }
        );
        assert_eq!(provider.action, RecoveryUsageAction::CommitTerminal);

        let call_id = Uuid::new_v4();
        let capability = RecoveryUsageReservation::try_from(PendingRecoveryUsageRow {
            tenant_id,
            run_id,
            reservation_id,
            stage_kind: "capability_call".to_owned(),
            provider_attempt_id: None,
            capability_call_id: Some(call_id),
            reservation_status: "reserved".to_owned(),
            claimed: false,
        })
        .unwrap();
        assert_eq!(
            capability.stage,
            RecoveryUsageStage::CapabilityCall { call_id }
        );
        assert_eq!(capability.action, RecoveryUsageAction::ExpireUnclaimed);

        let not_limited = RecoveryUsageReservation::try_from(PendingRecoveryUsageRow {
            tenant_id,
            run_id,
            reservation_id,
            stage_kind: "capability_call".to_owned(),
            provider_attempt_id: None,
            capability_call_id: Some(call_id),
            reservation_status: "not_limited".to_owned(),
            claimed: false,
        })
        .unwrap();
        assert_eq!(not_limited.action, RecoveryUsageAction::CommitTerminal);

        for malformed in [
            PendingRecoveryUsageRow {
                tenant_id,
                run_id,
                reservation_id,
                stage_kind: "run".to_owned(),
                provider_attempt_id: None,
                capability_call_id: None,
                reservation_status: "reserved".to_owned(),
                claimed: false,
            },
            PendingRecoveryUsageRow {
                tenant_id,
                run_id,
                reservation_id,
                stage_kind: "provider_attempt".to_owned(),
                provider_attempt_id: Some(attempt_id),
                capability_call_id: Some(call_id),
                reservation_status: "reserved".to_owned(),
                claimed: true,
            },
            PendingRecoveryUsageRow {
                tenant_id,
                run_id,
                reservation_id,
                stage_kind: "provider_attempt".to_owned(),
                provider_attempt_id: Some(attempt_id),
                capability_call_id: None,
                reservation_status: "unknown".to_owned(),
                claimed: true,
            },
        ] {
            assert_eq!(
                RecoveryUsageReservation::try_from(malformed)
                    .unwrap_err()
                    .code(),
                "agent_runtime_storage_error"
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires a disposable fully migrated AGENT_RUNTIME_TEST_DATABASE_URL"]
    async fn postgres_service_contract_covers_ownership_replay_fencing_completion_and_recovery() {
        let database_url = std::env::var("AGENT_RUNTIME_TEST_DATABASE_URL")
            .expect("AGENT_RUNTIME_TEST_DATABASE_URL must target a disposable migrated database");
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .connect(&database_url)
            .await
            .expect("Agent runtime contract database must connect");
        let first = seed_runtime_tenant(&pool, "runtime-a").await;
        let second = seed_runtime_tenant(&pool, "runtime-b").await;
        let other_user = seed_runtime_user(&pool, first.tenant_id, "other").await;
        let ops = AgentSessionOps::new(pool.clone());

        let other_session = ops
            .create_session(
                first.tenant_id,
                other_user,
                RequestContext::generate(None),
                CreateSessionCommand::parse(Some("Other owner"), "other-create").unwrap(),
            )
            .await
            .expect("a trusted caller derives the new Session owner from its user parameter");
        assert!(matches!(
            ops.read_session(first.tenant_id, first.user_id, other_session.id)
                .await,
            Err(AgentSessionError::SessionNotFound)
        ));

        let create_context = RequestContext::generate(None);
        let created = ops
            .create_session(
                first.tenant_id,
                first.user_id,
                create_context,
                CreateSessionCommand::parse(Some("Learner report"), "create-session-1").unwrap(),
            )
            .await
            .expect("owner Session must create");
        let replay = ops
            .create_session(
                first.tenant_id,
                first.user_id,
                RequestContext::generate(None),
                CreateSessionCommand::parse(Some("Learner report"), "create-session-1").unwrap(),
            )
            .await
            .expect("identical Session create must replay");
        assert_eq!(created.id, replay.id);
        let conflicting_create = ops
            .create_session(
                first.tenant_id,
                first.user_id,
                RequestContext::generate(None),
                CreateSessionCommand::parse(Some("Different title"), "create-session-1").unwrap(),
            )
            .await
            .unwrap_err();
        assert_eq!(conflicting_create.code(), "idempotency_conflict");

        assert!(matches!(
            ops.read_session(first.tenant_id, other_user, created.id)
                .await,
            Err(AgentSessionError::SessionNotFound)
        ));
        sqlx::query(
            r#"
            INSERT INTO agent_thread_members (
                tenant_id, thread_id, user_id, membership_role, added_by
            )
            VALUES ($1, $2, $3, 'member', $4)
            "#,
        )
        .bind(first.tenant_id)
        .bind(created.id)
        .bind(other_user)
        .bind(first.user_id)
        .execute(&pool)
        .await
        .expect("explicit non-owner membership must insert for owner-only contract proof");
        assert!(matches!(
            ops.read_session(first.tenant_id, other_user, created.id)
                .await,
            Err(AgentSessionError::SessionNotFound)
        ));
        assert!(matches!(
            ops.read_session(second.tenant_id, second.user_id, created.id)
                .await,
            Err(AgentSessionError::SessionNotFound)
        ));
        assert_eq!(
            ops.list_sessions(
                first.tenant_id,
                first.user_id,
                ListSessionsQuery::parse(Some(10), None, Some("learner"), false).unwrap(),
            )
            .await
            .unwrap()
            .items
            .len(),
            1
        );

        let stale_rename = ops
            .rename_session(
                first.tenant_id,
                first.user_id,
                created.id,
                RequestContext::generate(None),
                RenameSessionCommand::parse("Updated", created.version + 1).unwrap(),
            )
            .await
            .unwrap_err();
        assert_eq!(stale_rename.code(), "stale_session_version");
        let renamed = ops
            .rename_session(
                first.tenant_id,
                first.user_id,
                created.id,
                RequestContext::generate(None),
                RenameSessionCommand::parse("Updated learner report", created.version).unwrap(),
            )
            .await
            .expect("current version must rename");
        assert_eq!(renamed.version, created.version + 1);

        let submit = SubmitMessageCommand::parse(
            "List the active learners",
            TaskClass::ModuleReadReporting,
            "sis",
            "/modules/sis",
            "submit-1",
        )
        .unwrap();
        let queued = ops
            .submit_message(
                first.tenant_id,
                first.user_id,
                created.id,
                RequestContext::generate(None),
                submit.clone(),
            )
            .await
            .expect("message submission must queue atomically");
        assert_eq!(queued.status, RunStatus::Queued);
        assert!(matches!(
            ops.read_run(first.tenant_id, other_user, queued.id).await,
            Err(AgentSessionError::RunNotFound)
        ));
        assert!(matches!(
            ops.submit_message(
                first.tenant_id,
                other_user,
                created.id,
                RequestContext::generate(None),
                SubmitMessageCommand::parse(
                    "Member must not submit",
                    TaskClass::ModuleReadReporting,
                    "sis",
                    "/modules/sis",
                    "member-submit",
                )
                .unwrap(),
            )
            .await,
            Err(AgentSessionError::SessionNotFound)
        ));
        let replayed_run = ops
            .submit_message(
                first.tenant_id,
                first.user_id,
                created.id,
                RequestContext::generate(None),
                submit,
            )
            .await
            .expect("identical submission must return the existing run");
        assert_eq!(queued.id, replayed_run.id);
        let conflicting_submit = ops
            .submit_message(
                first.tenant_id,
                first.user_id,
                created.id,
                RequestContext::generate(None),
                SubmitMessageCommand::parse(
                    "Different request",
                    TaskClass::ModuleReadReporting,
                    "sis",
                    "/modules/sis",
                    "submit-1",
                )
                .unwrap(),
            )
            .await
            .unwrap_err();
        assert_eq!(conflicting_submit.code(), "idempotency_conflict");
        let active_conflict = ops
            .submit_message(
                first.tenant_id,
                first.user_id,
                created.id,
                RequestContext::generate(None),
                SubmitMessageCommand::parse(
                    "Another request",
                    TaskClass::ModuleReadReporting,
                    "sis",
                    "/modules/sis",
                    "submit-2",
                )
                .unwrap(),
            )
            .await
            .unwrap_err();
        assert_eq!(active_conflict.code(), "active_run_exists");
        assert_eq!(
            ops.list_messages(
                first.tenant_id,
                first.user_id,
                created.id,
                ListMessagesQuery::parse(Some(10), None).unwrap(),
            )
            .await
            .unwrap()
            .items
            .len(),
            1
        );

        let claimed = ops
            .claim_runs(
                first.tenant_id,
                ClaimRunsCommand::parse("worker-a", 1).unwrap(),
            )
            .await
            .expect("worker must claim available work");
        assert_eq!(claimed.len(), 1);
        let original_lease = claimed[0].lease.clone();
        let heartbeat = ops
            .heartbeat(first.tenant_id, &original_lease)
            .await
            .expect("current fence must heartbeat");
        assert!(heartbeat.lease.fence_version > original_lease.fence_version);
        assert!(matches!(
            ops.heartbeat(first.tenant_id, &original_lease).await,
            Err(AgentSessionError::LeaseLost)
        ));
        assert_eq!(
            ops.checkpoint(
                first.tenant_id,
                &heartbeat.lease,
                RunCheckpoint::CapabilityInFlight,
            )
            .await
            .unwrap_err()
            .code(),
            "invalid_run_checkpoint"
        );
        let first_provider_plan = provider_plan(&first, 1, 1, [1; 32]);
        let provider = ops
            .prepare_provider_attempt(
                first.tenant_id,
                &heartbeat.lease,
                first_provider_plan.clone(),
            )
            .await
            .unwrap();
        let duplicated_provider = ops
            .prepare_provider_attempt(first.tenant_id, &provider.lease, first_provider_plan)
            .await
            .expect("duplicate provider preparation delivery must return its durable identity");
        assert_eq!(duplicated_provider.identity, provider.identity);
        assert_eq!(
            duplicated_provider.lease.fence_version,
            provider.lease.fence_version
        );
        assert_eq!(
            ops.persist_provider_failure(
                first.tenant_id,
                &duplicated_provider.lease,
                provider.identity,
                ProviderAttemptFailure::Preflight(ProviderPreflightFailure::InvalidInput),
                NormalizedProviderUsage::parse(Some(1), None, None, None, None, None).unwrap(),
            )
            .await
            .unwrap_err()
            .code(),
            "invalid_preflight_usage"
        );
        assert_eq!(
            ops.persist_provider_success(
                first.tenant_id,
                &duplicated_provider.lease,
                provider.identity,
                NormalizedProviderUsage::unknown(),
                encrypted_artifact(b"premature-provider-result"),
            )
            .await
            .unwrap_err()
            .code(),
            "provider_attempt_not_in_flight"
        );
        let lease = ops
            .mark_provider_in_flight(
                first.tenant_id,
                &duplicated_provider.lease,
                provider.identity,
            )
            .await
            .unwrap();
        let provider_result = ops
            .persist_provider_success(
                first.tenant_id,
                &lease,
                provider.identity,
                NormalizedProviderUsage::unknown(),
                encrypted_artifact(b"provider-result"),
            )
            .await
            .unwrap();
        let first_capability_plan = CapabilityCallPlan::parse(
            Uuid::new_v4(),
            1,
            1,
            "sis.learners.list",
            1,
            "sis.learners.list",
            "sis",
            "sis:view",
            [2; 32],
            CapabilityCallScope::TenantWide,
        )
        .unwrap();
        let capability = ops
            .prepare_capability_call(
                first.tenant_id,
                &provider_result.lease,
                first_capability_plan.clone(),
            )
            .await
            .unwrap();
        let duplicated_capability = ops
            .prepare_capability_call(first.tenant_id, &capability.lease, first_capability_plan)
            .await
            .expect("duplicate capability preparation must return its durable identity");
        assert_eq!(duplicated_capability.identity, capability.identity);
        assert_eq!(
            duplicated_capability.lease.fence_version,
            capability.lease.fence_version
        );
        let capability_result = ops
            .persist_capability_success(
                first.tenant_id,
                &duplicated_capability.lease,
                capability.identity,
                CapabilityCallDuration::parse(12).unwrap(),
                encrypted_artifact(b"capability-result"),
            )
            .await
            .unwrap();
        assert_eq!(
            ops.prepare_capability_call(
                first.tenant_id,
                &capability_result.lease,
                CapabilityCallPlan::parse(
                    Uuid::new_v4(),
                    1,
                    2,
                    "sis.learners.read",
                    1,
                    "sis.learners.read",
                    "sis",
                    "sis:view",
                    [22; 32],
                    CapabilityCallScope::TenantWide,
                )
                .unwrap(),
            )
            .await
            .unwrap_err()
            .code(),
            "capability_call_not_ready"
        );
        let second_turn = ops
            .prepare_provider_attempt(
                first.tenant_id,
                &capability_result.lease,
                provider_plan(&first, 2, 1, [3; 32]),
            )
            .await
            .unwrap();
        let second_turn_lease = ops
            .mark_provider_in_flight(first.tenant_id, &second_turn.lease, second_turn.identity)
            .await
            .unwrap();
        let second_turn_result = ops
            .persist_provider_success(
                first.tenant_id,
                &second_turn_lease,
                second_turn.identity,
                NormalizedProviderUsage::unknown(),
                encrypted_artifact(b"second-provider-result"),
            )
            .await
            .unwrap();
        let final_text = "There are no active learners.";
        let final_result = ops
            .persist_final_response(
                first.tenant_id,
                &second_turn_result.lease,
                ProviderTurnIndex::parse(2).unwrap(),
                encrypted_artifact(final_text.as_bytes()),
            )
            .await
            .unwrap();
        let execution_snapshot = ops
            .load_execution_snapshot(first.tenant_id, &final_result.lease)
            .await
            .unwrap();
        assert_eq!(execution_snapshot.steps.len(), 4);
        assert_eq!(execution_snapshot.checkpoint, RunCheckpoint::Finalizing);
        assert!(matches!(
            &execution_snapshot.steps[0],
            ExecutionStepSnapshot::ProviderAttempt(step) if step.step.artifact.is_some()
        ));
        let completed = ops
            .complete_run(
                first.tenant_id,
                &final_result.lease,
                final_result.artifact.id,
                FinalResponsePlaintext::parse(final_text.to_owned()).unwrap(),
            )
            .await
            .expect("finalizing run must complete with one assistant message");
        assert_eq!(completed.status, RunStatus::Completed);
        assert!(completed.response_message_id.is_some());
        assert_eq!(
            ops.complete_run(
                first.tenant_id,
                &final_result.lease,
                final_result.artifact.id,
                FinalResponsePlaintext::parse(final_text.to_owned()).unwrap(),
            )
            .await
            .expect("lost completion acknowledgement must be idempotent")
            .response_message_id,
            completed.response_message_id
        );
        assert_eq!(
            ops.complete_run(
                first.tenant_id,
                &final_result.lease,
                Uuid::new_v4(),
                FinalResponsePlaintext::parse(final_text.to_owned()).unwrap(),
            )
            .await
            .unwrap_err()
            .code(),
            "final_response_conflict"
        );
        assert_eq!(
            ops.cancel_run(
                first.tenant_id,
                first.user_id,
                completed.id,
                RequestContext::generate(None),
            )
            .await
            .unwrap_err()
            .code(),
            "run_already_finished"
        );
        assert!(
            ops.list_events(
                first.tenant_id,
                first.user_id,
                completed.id,
                ListEventsQuery::parse(Some(2), None).unwrap(),
            )
            .await
            .unwrap()
            .items[0]
                .cursor
                .parse::<i64>()
                .unwrap()
                > 0
        );
        assert_eq!(
            ops.list_runs(
                first.tenant_id,
                first.user_id,
                created.id,
                ListRunsQuery::parse(Some(10), None).unwrap(),
            )
            .await
            .unwrap()
            .items
            .len(),
            1
        );

        let queued_cancel = ops
            .submit_message(
                first.tenant_id,
                first.user_id,
                created.id,
                RequestContext::generate(None),
                SubmitMessageCommand::parse(
                    "Cancel before claim",
                    TaskClass::ModuleReadReporting,
                    "sis",
                    "/modules/sis",
                    "queued-cancel",
                )
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            ops.cancel_run(
                first.tenant_id,
                first.user_id,
                queued_cancel.id,
                RequestContext::generate(None),
            )
            .await
            .expect("available cancellation must finish immediately")
            .status,
            RunStatus::Cancelled
        );

        let safe_run =
            submit_and_claim(&ops, first.tenant_id, first.user_id, created.id, "safe").await;
        let safe_lease = ops
            .checkpoint(
                first.tenant_id,
                &safe_run.lease,
                RunCheckpoint::BeforeProvider,
            )
            .await
            .unwrap();
        expire_lease(&pool, first.tenant_id, &safe_lease).await;
        let recovered = ops
            .recover_expired_runs(first.tenant_id, 10)
            .await
            .expect("idempotent checkpoint must requeue");
        assert_eq!(recovered.requeued, 1);
        assert_eq!(recovered.interrupted, 0);
        let reclaimed = ops
            .claim_runs(
                first.tenant_id,
                ClaimRunsCommand::parse("worker-b", 1).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(reclaimed[0].delivery_attempt, 2);
        let cancelled = ops
            .cancel_run(
                first.tenant_id,
                first.user_id,
                reclaimed[0].lease.run_id,
                RequestContext::generate(None),
            )
            .await
            .expect("owner must cancel reclaimed work");
        assert_eq!(cancelled.status, RunStatus::Running);
        assert_eq!(
            ops.cancel_run(
                first.tenant_id,
                first.user_id,
                reclaimed[0].lease.run_id,
                RequestContext::generate(None),
            )
            .await
            .expect("repeated cancellation must return current state")
            .status,
            RunStatus::Running
        );
        let cancellation_heartbeat = ops
            .heartbeat(first.tenant_id, &reclaimed[0].lease)
            .await
            .expect("cancellation request must preserve the current worker fence");
        assert!(cancellation_heartbeat.cancel_requested);
        let foreign_lease = crate::RunLease::parse(
            reclaimed[0].lease.run_id,
            "foreign-worker",
            reclaimed[0].lease.lease_token,
            reclaimed[0].lease.fence_version,
        )
        .unwrap();
        assert!(matches!(
            ops.heartbeat(first.tenant_id, &foreign_lease).await,
            Err(AgentSessionError::LeaseLost)
        ));
        assert_eq!(
            ops.acknowledge_cancellation(first.tenant_id, &cancellation_heartbeat.lease)
                .await
                .expect("the fenced lease owner must acknowledge cancellation")
                .status,
            RunStatus::Cancelled
        );
        assert_eq!(
            ops.cancel_run(
                first.tenant_id,
                first.user_id,
                reclaimed[0].lease.run_id,
                RequestContext::generate(None),
            )
            .await
            .expect("cancellation remains idempotent after acknowledgement")
            .status,
            RunStatus::Cancelled
        );

        let in_flight_cancel = submit_and_claim(
            &ops,
            first.tenant_id,
            first.user_id,
            created.id,
            "in-flight-cancel",
        )
        .await;
        let in_flight_attempt = ops
            .prepare_provider_attempt(
                first.tenant_id,
                &in_flight_cancel.lease,
                provider_plan(&first, 1, 1, [6; 32]),
            )
            .await
            .unwrap();
        let in_flight_lease = ops
            .mark_provider_in_flight(
                first.tenant_id,
                &in_flight_attempt.lease,
                in_flight_attempt.identity,
            )
            .await
            .unwrap();
        assert_eq!(
            ops.cancel_run(
                first.tenant_id,
                first.user_id,
                in_flight_lease.run_id,
                RequestContext::generate(None),
            )
            .await
            .unwrap()
            .status,
            RunStatus::Running
        );
        let in_flight_heartbeat = ops
            .heartbeat(first.tenant_id, &in_flight_lease)
            .await
            .unwrap();
        assert!(in_flight_heartbeat.cancel_requested);
        let persisted_cancelled_outcome = ops
            .persist_provider_failure(
                first.tenant_id,
                &in_flight_heartbeat.lease,
                in_flight_attempt.identity,
                ProviderAttemptFailure::Upstream(ProviderUpstreamFailure::Unavailable),
                NormalizedProviderUsage::unknown(),
            )
            .await
            .expect("in-flight outcome must persist after cancellation request");
        assert_eq!(
            ops.acknowledge_cancellation(first.tenant_id, &persisted_cancelled_outcome)
                .await
                .unwrap()
                .status,
            RunStatus::Cancelled
        );

        let provider_failed_run = submit_and_claim(
            &ops,
            first.tenant_id,
            first.user_id,
            created.id,
            "provider-failed",
        )
        .await;
        let preflight = ops
            .prepare_provider_attempt(
                first.tenant_id,
                &provider_failed_run.lease,
                provider_plan(&first, 1, 1, [4; 32]),
            )
            .await
            .unwrap();
        let fallback_lease = ops
            .persist_provider_failure(
                first.tenant_id,
                &ops.mark_provider_in_flight(first.tenant_id, &preflight.lease, preflight.identity)
                    .await
                    .unwrap(),
                preflight.identity,
                ProviderAttemptFailure::Upstream(ProviderUpstreamFailure::RateLimited),
                NormalizedProviderUsage::unknown(),
            )
            .await
            .unwrap();
        let fallback = ops
            .prepare_provider_attempt(
                first.tenant_id,
                &fallback_lease,
                provider_plan(&first, 1, 2, [4; 32]),
            )
            .await
            .unwrap();
        let fallback_in_flight = ops
            .mark_provider_in_flight(first.tenant_id, &fallback.lease, fallback.identity)
            .await
            .unwrap();
        let fallback_failed = ops
            .persist_provider_failure(
                first.tenant_id,
                &fallback_in_flight,
                fallback.identity,
                ProviderAttemptFailure::Upstream(ProviderUpstreamFailure::Unavailable),
                NormalizedProviderUsage::unknown(),
            )
            .await
            .unwrap();
        assert_eq!(
            ops.fail_run(
                first.tenant_id,
                &fallback_failed,
                SafeRunFailure::parse("provider_unavailable", "The provider is unavailable")
                    .unwrap(),
            )
            .await
            .unwrap()
            .status,
            RunStatus::Failed
        );

        let lost_failure_ack = submit_and_claim(
            &ops,
            first.tenant_id,
            first.user_id,
            created.id,
            "lost-provider-failure-ack",
        )
        .await;
        let lost_failure_attempt = ops
            .prepare_provider_attempt(
                first.tenant_id,
                &lost_failure_ack.lease,
                provider_plan(&first, 1, 1, [31; 32]),
            )
            .await
            .unwrap();
        let lost_failure_in_flight = ops
            .mark_provider_in_flight(
                first.tenant_id,
                &lost_failure_attempt.lease,
                lost_failure_attempt.identity,
            )
            .await
            .unwrap();
        let lost_failure_result = ops
            .persist_provider_failure(
                first.tenant_id,
                &lost_failure_in_flight,
                lost_failure_attempt.identity,
                ProviderAttemptFailure::Upstream(ProviderUpstreamFailure::Timeout),
                NormalizedProviderUsage::unknown(),
            )
            .await
            .unwrap();
        expire_lease(&pool, first.tenant_id, &lost_failure_result).await;
        assert_eq!(
            ops.recover_expired_runs(first.tenant_id, 10)
                .await
                .unwrap()
                .requeued,
            1
        );
        let reclaimed_failure = ops
            .claim_runs(
                first.tenant_id,
                ClaimRunsCommand::parse("worker-lost-provider-failure", 1).unwrap(),
            )
            .await
            .unwrap()
            .remove(0);
        let failure_snapshot = ops
            .load_execution_snapshot(first.tenant_id, &reclaimed_failure.lease)
            .await
            .unwrap();
        assert!(matches!(
            &failure_snapshot.steps[0],
            ExecutionStepSnapshot::ProviderAttempt(step)
                if step.status == ProviderAttemptStatus::Failed
                    && step.failure
                        == Some(ProviderAttemptFailure::Upstream(
                            ProviderUpstreamFailure::Timeout
                        ))
                    && step.step.artifact.is_none()
        ));
        let ambiguous_fallback = ops
            .prepare_provider_attempt(
                first.tenant_id,
                &reclaimed_failure.lease,
                provider_plan(&first, 1, 2, [31; 32]),
            )
            .await
            .expect_err("a timeout after dispatch must not advance to a fallback attempt");
        assert_eq!(
            ambiguous_fallback.code(),
            "invalid_provider_attempt_sequence"
        );
        assert_eq!(
            ops.fail_run(
                first.tenant_id,
                &reclaimed_failure.lease,
                SafeRunFailure::parse(
                    "provider_outcome_ambiguous",
                    "The provider outcome could not be determined",
                )
                .unwrap(),
            )
            .await
            .unwrap()
            .status,
            RunStatus::Failed
        );

        for (suffix, failure_status, failure_code, expected_status, result_text) in [
            (
                "capability-failed-replay",
                CapabilityFailureStatus::Failed,
                "capability_unavailable",
                CapabilityCallStatus::Failed,
                "The learner service is temporarily unavailable.",
            ),
            (
                "capability-denied-replay",
                CapabilityFailureStatus::Denied,
                "permission_denied",
                CapabilityCallStatus::Denied,
                "You do not have permission to view those learners.",
            ),
        ] {
            let capability_replay =
                submit_and_claim(&ops, first.tenant_id, first.user_id, created.id, suffix).await;
            let replay_provider = ops
                .prepare_provider_attempt(
                    first.tenant_id,
                    &capability_replay.lease,
                    provider_plan(&first, 1, 1, [32; 32]),
                )
                .await
                .unwrap();
            let replay_provider_in_flight = ops
                .mark_provider_in_flight(
                    first.tenant_id,
                    &replay_provider.lease,
                    replay_provider.identity,
                )
                .await
                .unwrap();
            let replay_provider_result = ops
                .persist_provider_success(
                    first.tenant_id,
                    &replay_provider_in_flight,
                    replay_provider.identity,
                    NormalizedProviderUsage::unknown(),
                    encrypted_artifact(b"provider-capability-call"),
                )
                .await
                .unwrap();
            let replay_call = ops
                .prepare_capability_call(
                    first.tenant_id,
                    &replay_provider_result.lease,
                    CapabilityCallPlan::parse(
                        Uuid::new_v4(),
                        1,
                        1,
                        "sis.learners.list",
                        1,
                        "sis.learners.list",
                        "sis",
                        "sis:view",
                        [33; 32],
                        CapabilityCallScope::TenantWide,
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
            let replay_result = ops
                .persist_capability_failure(
                    first.tenant_id,
                    &replay_call.lease,
                    replay_call.identity,
                    CapabilityCallFailure::parse(failure_status, failure_code, 19).unwrap(),
                    encrypted_artifact(result_text.as_bytes()),
                )
                .await
                .expect("failed and denied capability results must persist atomically");
            expire_lease(&pool, first.tenant_id, &replay_result.lease).await;
            assert_eq!(
                ops.recover_expired_runs(first.tenant_id, 10)
                    .await
                    .unwrap()
                    .requeued,
                1
            );
            let reclaimed = ops
                .claim_runs(
                    first.tenant_id,
                    ClaimRunsCommand::parse(&format!("worker-{suffix}-reclaim"), 1).unwrap(),
                )
                .await
                .unwrap()
                .remove(0);
            let snapshot = ops
                .load_execution_snapshot(first.tenant_id, &reclaimed.lease)
                .await
                .unwrap();
            assert_eq!(
                snapshot.checkpoint,
                RunCheckpoint::CapabilityResultPersisted
            );
            let capability_step = snapshot
                .steps
                .iter()
                .find_map(|step| match step {
                    ExecutionStepSnapshot::CapabilityCall(step) => Some(step),
                    _ => None,
                })
                .expect("reclaimed snapshot must include the capability outcome");
            assert_eq!(capability_step.status, expected_status);
            assert_eq!(
                capability_step.safe_failure_code.as_deref(),
                Some(failure_code)
            );
            let replay_artifact = capability_step
                .step
                .artifact
                .as_ref()
                .expect("failed and denied capability calls must retain replay evidence");
            assert!(
                replay_artifact
                    .envelope()
                    .ciphertext()
                    .starts_with(result_text.as_bytes())
            );
            assert_eq!(
                ops.fail_run(
                    first.tenant_id,
                    &reclaimed.lease,
                    SafeRunFailure::parse(
                        "capability_result_replayed",
                        "Capability result replay contract completed",
                    )
                    .unwrap(),
                )
                .await
                .unwrap()
                .status,
                RunStatus::Failed
            );
        }

        let failed_run =
            submit_and_claim(&ops, first.tenant_id, first.user_id, created.id, "failed").await;
        let failed = ops
            .fail_run(
                first.tenant_id,
                &failed_run.lease,
                SafeRunFailure::parse("provider_unavailable", "The provider is unavailable")
                    .unwrap(),
            )
            .await
            .expect("a fenced worker may persist a safe failure");
        assert_eq!(failed.status, RunStatus::Failed);
        assert_eq!(
            failed.safe_failure_code.as_deref(),
            Some("provider_unavailable")
        );

        let finalizing_crash = submit_and_claim(
            &ops,
            first.tenant_id,
            first.user_id,
            created.id,
            "finalizing-crash",
        )
        .await;
        let crash_attempt = ops
            .prepare_provider_attempt(
                first.tenant_id,
                &finalizing_crash.lease,
                provider_plan(&first, 1, 1, [5; 32]),
            )
            .await
            .unwrap();
        let crash_in_flight = ops
            .mark_provider_in_flight(
                first.tenant_id,
                &crash_attempt.lease,
                crash_attempt.identity,
            )
            .await
            .unwrap();
        let crash_result = ops
            .persist_provider_success(
                first.tenant_id,
                &crash_in_flight,
                crash_attempt.identity,
                NormalizedProviderUsage::unknown(),
                encrypted_artifact(b"crash-provider-result"),
            )
            .await
            .unwrap();
        let crash_text = "Recovered from durable final evidence.";
        let crash_final = ops
            .persist_final_response(
                first.tenant_id,
                &crash_result.lease,
                ProviderTurnIndex::parse(1).unwrap(),
                encrypted_artifact(crash_text.as_bytes()),
            )
            .await
            .unwrap();
        expire_lease(&pool, first.tenant_id, &crash_final.lease).await;
        let recovered_finalizing = ops.recover_expired_runs(first.tenant_id, 10).await.unwrap();
        assert_eq!(recovered_finalizing.requeued, 1);
        assert_eq!(recovered_finalizing.interrupted, 0);
        let reclaimed_finalizing = ops
            .claim_runs(
                first.tenant_id,
                ClaimRunsCommand::parse("worker-finalizing-recovery", 1).unwrap(),
            )
            .await
            .unwrap()
            .remove(0);
        assert_eq!(reclaimed_finalizing.checkpoint, RunCheckpoint::Finalizing);
        assert_eq!(
            ops.complete_run(
                first.tenant_id,
                &reclaimed_finalizing.lease,
                crash_final.artifact.id,
                FinalResponsePlaintext::parse(crash_text.to_owned()).unwrap(),
            )
            .await
            .unwrap()
            .status,
            RunStatus::Completed
        );

        let exhausted = submit_and_claim(
            &ops,
            first.tenant_id,
            first.user_id,
            created.id,
            "exhausted",
        )
        .await;
        let exhausted_lease = ops
            .checkpoint(
                first.tenant_id,
                &exhausted.lease,
                RunCheckpoint::BeforeProvider,
            )
            .await
            .unwrap();
        expire_lease(&pool, first.tenant_id, &exhausted_lease).await;
        assert_eq!(
            ops.recover_expired_runs(first.tenant_id, 10)
                .await
                .unwrap()
                .requeued,
            1
        );
        let second_delivery = ops
            .claim_runs(
                first.tenant_id,
                ClaimRunsCommand::parse("worker-exhausted-2", 1).unwrap(),
            )
            .await
            .unwrap()
            .remove(0);
        assert_eq!(second_delivery.delivery_attempt, 2);
        expire_lease(&pool, first.tenant_id, &second_delivery.lease).await;
        assert_eq!(
            ops.recover_expired_runs(first.tenant_id, 10)
                .await
                .unwrap()
                .requeued,
            1
        );
        let third_delivery = ops
            .claim_runs(
                first.tenant_id,
                ClaimRunsCommand::parse("worker-exhausted-3", 1).unwrap(),
            )
            .await
            .unwrap()
            .remove(0);
        assert_eq!(third_delivery.delivery_attempt, 3);
        expire_lease(&pool, first.tenant_id, &third_delivery.lease).await;
        let exhausted_recovery = ops.recover_expired_runs(first.tenant_id, 10).await.unwrap();
        assert_eq!(exhausted_recovery.requeued, 0);
        assert_eq!(exhausted_recovery.interrupted, 1);
        let exhausted_run = ops
            .read_run(first.tenant_id, first.user_id, third_delivery.lease.run_id)
            .await
            .unwrap();
        assert_eq!(exhausted_run.status, RunStatus::Interrupted);
        assert_eq!(
            exhausted_run.safe_failure_code.as_deref(),
            Some("delivery_attempts_exhausted")
        );
        let exhausted_queue_state = sqlx::query_scalar::<_, String>(
            "SELECT state FROM agent_run_queue WHERE tenant_id = $1 AND run_id = $2",
        )
        .bind(first.tenant_id)
        .bind(third_delivery.lease.run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(exhausted_queue_state, "finished");
        let exhausted_event_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM agent_run_events WHERE tenant_id = $1 AND run_id = $2 AND event_type = 'interrupted'",
        )
        .bind(first.tenant_id)
        .bind(third_delivery.lease.run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(exhausted_event_count, 1);
        let exhausted_audit_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM actor_audit_events WHERE tenant_id = $1 AND agent_run_id = $2 AND action_key = 'agent.runs.interrupt'",
        )
        .bind(first.tenant_id)
        .bind(third_delivery.lease.run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(exhausted_audit_count, 1);

        let unsafe_run =
            submit_and_claim(&ops, first.tenant_id, first.user_id, created.id, "unsafe").await;
        let unsafe_lease = ops
            .checkpoint(
                first.tenant_id,
                &unsafe_run.lease,
                RunCheckpoint::BeforeProvider,
            )
            .await
            .unwrap();
        let unsafe_lease = ops
            .checkpoint(
                first.tenant_id,
                &unsafe_lease,
                RunCheckpoint::ProviderInFlight,
            )
            .await
            .unwrap();
        expire_lease(&pool, first.tenant_id, &unsafe_lease).await;
        let recovered = ops.recover_expired_runs(first.tenant_id, 10).await.unwrap();
        assert_eq!(recovered.interrupted, 1);
        assert_eq!(
            ops.read_run(first.tenant_id, first.user_id, unsafe_lease.run_id)
                .await
                .unwrap()
                .status,
            RunStatus::Interrupted
        );

        let latest = ops
            .read_session(first.tenant_id, first.user_id, created.id)
            .await
            .unwrap();
        let archived = ops
            .archive_session(
                first.tenant_id,
                first.user_id,
                created.id,
                RequestContext::generate(None),
                ArchiveSessionCommand::parse(latest.version).unwrap(),
            )
            .await
            .expect("Session with no active run must archive");
        assert_eq!(archived.status, SessionStatus::Archived);

        let audit_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM actor_audit_events WHERE tenant_id = $1 AND agent_run_id IS NOT NULL",
        )
        .bind(first.tenant_id)
        .fetch_one(&pool)
        .await
        .expect("run audit events must be queryable");
        assert!(audit_count >= 6);
    }

    #[tokio::test]
    #[ignore = "requires an exclusive disposable fully migrated AGENT_RUNTIME_TEST_DATABASE_URL"]
    async fn postgres_global_worker_claim_and_recovery_are_tenant_fair_and_skip_locked() {
        let database_url = std::env::var("AGENT_RUNTIME_TEST_DATABASE_URL")
            .expect("AGENT_RUNTIME_TEST_DATABASE_URL must target a disposable migrated database");
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .connect(&database_url)
            .await
            .expect("Agent runtime contract database must connect");
        let first = seed_session_tenant(&pool, "global-runtime-a").await;
        let second = seed_session_tenant(&pool, "global-runtime-b").await;
        let ops = AgentSessionOps::new(pool.clone());
        let mut first_sessions = Vec::new();
        for index in 1..=3 {
            first_sessions.push(
                ops.create_session(
                    first.tenant_id,
                    first.user_id,
                    RequestContext::generate(None),
                    CreateSessionCommand::parse(
                        Some(&format!("Global first {index}")),
                        format!("global-first-session-{index}"),
                    )
                    .unwrap(),
                )
                .await
                .unwrap(),
            );
        }
        let second_session = ops
            .create_session(
                second.tenant_id,
                second.user_id,
                RequestContext::generate(None),
                CreateSessionCommand::parse(Some("Global second"), "global-second-session")
                    .unwrap(),
            )
            .await
            .unwrap();

        for (session, suffix) in first_sessions.iter().zip(["a-1", "a-2", "a-3"]) {
            submit_queued_run(&ops, first.tenant_id, first.user_id, session.id, suffix).await;
        }
        let second_run = submit_queued_run(
            &ops,
            second.tenant_id,
            second.user_id,
            second_session.id,
            "b-1",
        )
        .await;

        let ordered_first = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT run_id
            FROM agent_run_queue
            WHERE tenant_id = $1 AND state = 'available'
            ORDER BY available_at, run_id
            "#,
        )
        .bind(first.tenant_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(ordered_first.len(), 3);

        let mut blocker = pool.begin().await.unwrap();
        let blocked_run = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT run_id
            FROM agent_run_queue
            WHERE tenant_id = $1 AND run_id = $2
            FOR UPDATE
            "#,
        )
        .bind(first.tenant_id)
        .bind(ordered_first[0])
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
        assert_eq!(blocked_run, ordered_first[0]);

        let claimed = ops
            .claim_runs_globally(ClaimRunsCommand::parse("global-worker-a", 2).unwrap())
            .await
            .expect("global claim must skip the independently locked oldest row");
        assert_eq!(claimed.len(), 2);
        assert!(claimed.iter().all(|run| run.lease.fence_version > 1));
        assert!(claimed.iter().all(|run| !run.lease.lease_token.is_nil()));
        assert_eq!(
            claimed
                .iter()
                .map(|run| run.lease.lease_token)
                .collect::<HashSet<_>>()
                .len(),
            2
        );
        assert!(
            claimed
                .iter()
                .any(|run| run.tenant_id == first.tenant_id
                    && run.lease.run_id == ordered_first[1])
        );
        assert!(
            claimed
                .iter()
                .any(|run| run.tenant_id == second.tenant_id && run.lease.run_id == second_run.id)
        );
        assert!(claimed.iter().all(|run| run.lease.run_id != blocked_run));
        blocker.rollback().await.unwrap();

        for run in &claimed {
            expire_lease(&pool, run.tenant_id, &run.lease).await;
        }
        let recovered = ops
            .recover_expired_runs_globally(2)
            .await
            .expect("global recovery must process one expired lease from each campus");
        assert_eq!(recovered.summary.requeued, 2);
        assert_eq!(recovered.summary.interrupted, 0);
        assert_eq!(recovered.summary.cancelled, 0);
        assert_eq!(recovered.runs.len(), 2);
        assert!(recovered.pending_usage_reservations.is_empty());
        assert!(
            recovered
                .runs
                .iter()
                .any(|run| run.tenant_id == first.tenant_id)
        );
        assert!(
            recovered
                .runs
                .iter()
                .any(|run| run.tenant_id == second.tenant_id)
        );
        assert!(
            recovered
                .runs
                .iter()
                .all(|run| { run.disposition == crate::ExpiredLeaseRecoveryDisposition::Requeued })
        );

        let replay = ops.recover_expired_runs_globally(2).await.unwrap();
        assert_eq!(replay.summary.requeued, 0);
        assert_eq!(replay.summary.interrupted, 0);
        assert_eq!(replay.summary.cancelled, 0);
        assert!(replay.runs.is_empty());

        let next = ops
            .claim_runs_globally(ClaimRunsCommand::parse("global-worker-b", 2).unwrap())
            .await
            .unwrap();
        assert_eq!(next.len(), 2);
        assert!(next.iter().any(|run| run.tenant_id == first.tenant_id));
        assert!(next.iter().any(|run| run.tenant_id == second.tenant_id));

        let claimed_reservation = seed_terminal_recovery_reservation(&pool, &next[0], true).await;
        let unclaimed_reservation =
            seed_terminal_recovery_reservation(&pool, &next[1], false).await;
        let usage_recovery = ops.recover_expired_runs_globally(2).await.unwrap();
        assert!(usage_recovery.runs.is_empty());
        assert_eq!(usage_recovery.pending_usage_reservations.len(), 2);
        assert!(
            usage_recovery
                .pending_usage_reservations
                .iter()
                .any(|item| {
                    item.tenant_id == next[0].tenant_id
                        && item.run_id == next[0].lease.run_id
                        && item.reservation_id == claimed_reservation.0
                        && item.stage
                            == RecoveryUsageStage::CapabilityCall {
                                call_id: claimed_reservation.1,
                            }
                        && item.action == RecoveryUsageAction::CommitTerminal
                })
        );
        assert!(
            usage_recovery
                .pending_usage_reservations
                .iter()
                .any(|item| {
                    item.tenant_id == next[1].tenant_id
                        && item.run_id == next[1].lease.run_id
                        && item.reservation_id == unclaimed_reservation.0
                        && item.stage
                            == RecoveryUsageStage::CapabilityCall {
                                call_id: unclaimed_reservation.1,
                            }
                        && item.action == RecoveryUsageAction::ExpireUnclaimed
                })
        );

        let usage_replay = ops.recover_expired_runs_globally(2).await.unwrap();
        assert_eq!(
            usage_replay
                .pending_usage_reservations
                .iter()
                .map(|item| item.reservation_id)
                .collect::<HashSet<_>>(),
            HashSet::from([claimed_reservation.0, unclaimed_reservation.0])
        );
    }

    #[derive(Debug, Clone)]
    struct RuntimeSeed {
        tenant_id: Uuid,
        user_id: Uuid,
        route_set_id: Uuid,
        route_target_id: Uuid,
        connection_id: Uuid,
        model_id: Uuid,
        provider_data_approval_id: Uuid,
        provider_model_id: String,
    }

    #[derive(Debug, Clone, Copy)]
    struct SessionTenantSeed {
        tenant_id: Uuid,
        user_id: Uuid,
    }

    async fn seed_session_tenant(pool: &PgPool, prefix: &str) -> SessionTenantSeed {
        let tenant_id = Uuid::new_v4();
        let suffix = tenant_id.simple();
        sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, $2, $3)")
            .bind(tenant_id)
            .bind(format!("{prefix}-{suffix}"))
            .bind(format!("{prefix} Agent runtime contract"))
            .execute(pool)
            .await
            .expect("contract tenant must insert");
        let user_id = seed_runtime_user(pool, tenant_id, prefix).await;
        SessionTenantSeed { tenant_id, user_id }
    }

    async fn seed_runtime_tenant(pool: &PgPool, prefix: &str) -> RuntimeSeed {
        let session_seed = seed_session_tenant(pool, prefix).await;
        let tenant_id = session_seed.tenant_id;
        let user_id = session_seed.user_id;
        let (
            route_set_id,
            route_target_id,
            connection_id,
            model_id,
            provider_data_approval_id,
            provider_model_id,
        ) = seed_runtime_provider(pool, tenant_id, user_id).await;
        RuntimeSeed {
            tenant_id,
            user_id,
            route_set_id,
            route_target_id,
            connection_id,
            model_id,
            provider_data_approval_id,
            provider_model_id,
        }
    }

    async fn seed_runtime_user(pool: &PgPool, tenant_id: Uuid, prefix: &str) -> Uuid {
        let user_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO users (id, tenant_id, email, password_hash, full_name) VALUES ($1, $2, $3, 'not-a-login', 'Agent Runtime Contract')",
        )
        .bind(user_id)
        .bind(tenant_id)
        .bind(format!("{prefix}-{user_id}@example.invalid"))
        .execute(pool)
        .await
        .expect("contract user must insert");
        user_id
    }

    async fn seed_runtime_provider(
        pool: &PgPool,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> (Uuid, Uuid, Uuid, Uuid, Uuid, String) {
        let connection_id = Uuid::new_v4();
        let model_id = Uuid::new_v4();
        let route_set_id = Uuid::new_v4();
        let route_target_id = Uuid::new_v4();
        let default_approval_id = Uuid::new_v4();
        let provider_data_approval_id = Uuid::new_v4();
        let provider_model_id = format!("runtime-model-{connection_id}");
        let mut transaction = pool.begin().await.unwrap();
        sqlx::query(
            r#"
            INSERT INTO ai_provider_connections (
                id, tenant_id, provider, auth_method, account_label, status,
                credential_ciphertext, credential_nonce, credential_key_id,
                credential_version, credential_fingerprint, configured_by,
                model_catalog_version, model_catalog_refreshed_at
            )
            VALUES ($1, $2, 'openai', 'api_key', 'Runtime contract', 'ready',
                    $3, $4, 'runtime-key', 1, $5, $6, 1, NOW())
            "#,
        )
        .bind(connection_id)
        .bind(tenant_id)
        .bind(vec![7_u8; 16])
        .bind(vec![9_u8; 12])
        .bind(format!("sha256:{connection_id}"))
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO ai_provider_data_approval_versions (
                id, tenant_id, connection_id, approval_version, approval_class,
                change_source, changed_by, change_reason
            )
            VALUES
                ($1, $2, $3, 1, 'unapproved', 'system_default', NULL,
                 'Runtime default approval'),
                ($4, $2, $3, 2, 'sensitive_data_approved', 'administrator', $5,
                 'Runtime sensitive data approval')
            "#,
        )
        .bind(default_approval_id)
        .bind(tenant_id)
        .bind(connection_id)
        .bind(provider_data_approval_id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO ai_provider_models (
                id, tenant_id, connection_id, credential_version, catalog_version,
                provider_model_id, display_name, context_window_tokens,
                max_output_tokens, supports_tools, refreshed_at
            )
            VALUES ($1, $2, $3, 1, 1, $4, 'Runtime Test Model', 100000, 16384, TRUE, NOW())
            "#,
        )
        .bind(model_id)
        .bind(tenant_id)
        .bind(connection_id)
        .bind(&provider_model_id)
        .execute(&mut *transaction)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO ai_route_sets (
                id, tenant_id, scope_kind, task_class, configured_by, change_reason
            )
            VALUES ($1, $2, 'task_class', 'module_read_reporting', $3, 'Runtime contract route')
            "#,
        )
        .bind(route_set_id)
        .bind(tenant_id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO ai_task_routes (
                id, tenant_id, route_set_id, priority, connection_id,
                model_id, provider_data_approval_id, requires_tools, created_by
            )
            VALUES ($1, $2, $3, 1, $4, $5, $6, TRUE, $7)
            "#,
        )
        .bind(route_target_id)
        .bind(tenant_id)
        .bind(route_set_id)
        .bind(connection_id)
        .bind(model_id)
        .bind(provider_data_approval_id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .unwrap();
        transaction.commit().await.unwrap();
        (
            route_set_id,
            route_target_id,
            connection_id,
            model_id,
            provider_data_approval_id,
            provider_model_id,
        )
    }

    fn provider_plan(
        seed: &RuntimeSeed,
        turn_index: u16,
        attempt_index: u8,
        input_fingerprint: [u8; 32],
    ) -> ProviderAttemptPlan {
        ProviderAttemptPlan::parse(
            turn_index,
            attempt_index,
            seed.route_set_id,
            1,
            seed.route_target_id,
            seed.connection_id,
            1,
            seed.model_id,
            seed.provider_data_approval_id,
            cp_common::ProviderDataClass::SensitiveDataApproved,
            cp_common::ProviderExecutionEnvironmentClass::ExternalManaged,
            AgentProviderKey::OpenAi,
            &seed.provider_model_id,
            input_fingerprint,
        )
        .unwrap()
    }

    fn encrypted_artifact(plaintext: &[u8]) -> EncryptedExecutionArtifact {
        let mut ciphertext = plaintext.to_vec();
        ciphertext.extend_from_slice(&[0xA5; 16]);
        EncryptedExecutionArtifact::parse(
            ciphertext,
            Sha256::digest(plaintext).into(),
            vec![3; 12],
            "runtime-test-key",
            1,
            plaintext.len(),
        )
        .unwrap()
    }

    async fn submit_and_claim(
        ops: &AgentSessionOps,
        tenant_id: Uuid,
        user_id: Uuid,
        session_id: Uuid,
        suffix: &str,
    ) -> crate::ClaimedRun {
        submit_queued_run(ops, tenant_id, user_id, session_id, suffix).await;
        let claimed = ops
            .claim_runs(
                tenant_id,
                ClaimRunsCommand::parse(&format!("worker-{suffix}"), 1).unwrap(),
            )
            .await
            .expect("queued request must claim")
            .into_iter()
            .next()
            .expect("one queued request must exist");
        assert_eq!(claimed.tenant_id, tenant_id);
        claimed
    }

    async fn submit_queued_run(
        ops: &AgentSessionOps,
        tenant_id: Uuid,
        user_id: Uuid,
        session_id: Uuid,
        suffix: &str,
    ) -> crate::AgentRun {
        ops.submit_message(
            tenant_id,
            user_id,
            session_id,
            RequestContext::generate(None),
            SubmitMessageCommand::parse(
                &format!("Request {suffix}"),
                TaskClass::ModuleReadReporting,
                "sis",
                "/modules/sis",
                format!("submit-{suffix}"),
            )
            .unwrap(),
        )
        .await
        .expect("next request must queue")
    }

    async fn seed_terminal_recovery_reservation(
        pool: &PgPool,
        claimed: &crate::ClaimedRun,
        was_claimed: bool,
    ) -> (Uuid, Uuid) {
        // Fault injection is confined to this disposable contract database. It simulates the
        // post-recovery/pre-usage handoff crash so replay can be proven without mutating usage.rs.
        let reservation_id = Uuid::new_v4();
        let call_id = Uuid::new_v4();
        let mut transaction = pool.begin().await.unwrap();
        sqlx::query("SET LOCAL session_replication_role = 'replica'")
            .execute(&mut *transaction)
            .await
            .unwrap();
        sqlx::query(
            r#"
            UPDATE agent_runs
            SET status = 'interrupted',
                safe_failure_code = 'worker_recovery_fixture',
                safe_failure_message = 'Disposable recovery contract fixture',
                finished_at = CLOCK_TIMESTAMP(),
                version = version + 1,
                updated_at = CLOCK_TIMESTAMP()
            WHERE tenant_id = $1 AND id = $2 AND status = 'running'
            "#,
        )
        .bind(claimed.tenant_id)
        .bind(claimed.lease.run_id)
        .execute(&mut *transaction)
        .await
        .unwrap();
        sqlx::query(
            r#"
            UPDATE agent_run_queue
            SET state = 'finished', lease_token = NULL, leased_by = NULL,
                lease_expires_at = NULL, heartbeat_at = NULL,
                finished_at = CLOCK_TIMESTAMP(), version = version + 1,
                updated_at = CLOCK_TIMESTAMP()
            WHERE tenant_id = $1 AND run_id = $2 AND state = 'leased'
            "#,
        )
        .bind(claimed.tenant_id)
        .bind(claimed.lease.run_id)
        .execute(&mut *transaction)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO agent_capability_calls (
                id, tenant_id, run_id, call_sequence, capability_key,
                capability_version, product_operation_key, owning_module_key,
                required_permission, input_fingerprint, scope_kind,
                resource_references, status, safe_failure_code, duration_ms,
                finished_at
            )
            VALUES (
                $1, $2, $3, 1, 'sis.report', 1, 'sis.report', 'sis',
                'sis:read', $4, 'tenant_wide', '[]'::JSONB, 'interrupted',
                'worker_recovery_fixture', 0, CLOCK_TIMESTAMP()
            )
            "#,
        )
        .bind(call_id)
        .bind(claimed.tenant_id)
        .bind(claimed.lease.run_id)
        .bind(vec![17_u8; 32])
        .execute(&mut *transaction)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO agent_limit_reservations (
                id, tenant_id, run_id, capability_call_id, actor_user_id,
                role_keys, origin_module_key, capability_module_key,
                capability_key, stage_kind, stage_sequence, idempotency_key,
                request_fingerprint, status, expires_at, claimed_at,
                claimed_by_worker_id, claim_fence_version
            )
            VALUES (
                $1, $2, $3, $4, $5, ARRAY['worker'], 'sis', 'sis',
                'sis.report', 'capability_call', 1, $6, $7, 'reserved',
                CLOCK_TIMESTAMP() + INTERVAL '1 hour',
                CASE WHEN $8 THEN CLOCK_TIMESTAMP() END,
                CASE WHEN $8 THEN $9 END,
                CASE WHEN $8 THEN $10 END
            )
            "#,
        )
        .bind(reservation_id)
        .bind(claimed.tenant_id)
        .bind(claimed.lease.run_id)
        .bind(call_id)
        .bind(claimed.requested_by)
        .bind(format!("recovery-fixture-{reservation_id}"))
        .bind(vec![17_u8; 32])
        .bind(was_claimed)
        .bind(&claimed.lease.worker_id)
        .bind(claimed.lease.fence_version)
        .execute(&mut *transaction)
        .await
        .unwrap();
        transaction.commit().await.unwrap();
        (reservation_id, call_id)
    }

    async fn expire_lease(pool: &PgPool, tenant_id: Uuid, lease: &crate::RunLease) {
        // Fault injection is confined to the disposable contract database. Production lifecycle
        // triggers correctly reject a caller attempting to rewrite a live lease into the past.
        let mut transaction = pool.begin().await.expect("fault transaction must begin");
        sqlx::query("SET LOCAL session_replication_role = 'replica'")
            .execute(&mut *transaction)
            .await
            .expect("contract database must permit isolated trigger bypass");
        let updated = sqlx::query(
            r#"
            UPDATE agent_run_queue
            SET heartbeat_at = NOW() - INTERVAL '31 seconds',
                lease_expires_at = NOW() - INTERVAL '1 second',
                version = version + 1,
                updated_at = CLOCK_TIMESTAMP()
            WHERE tenant_id = $1
              AND run_id = $2
              AND leased_by = $3
              AND lease_token = $4
              AND version = $5
            "#,
        )
        .bind(tenant_id)
        .bind(lease.run_id)
        .bind(&lease.worker_id)
        .bind(lease.lease_token)
        .bind(lease.fence_version)
        .execute(&mut *transaction)
        .await
        .expect("contract lease must be made expired");
        assert_eq!(updated.rows_affected(), 1);
        transaction
            .commit()
            .await
            .expect("fault-injected lease expiry must commit");
    }
}
