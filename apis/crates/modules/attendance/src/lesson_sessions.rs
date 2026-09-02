//! Timetable-linked Attendance lesson-session operations.

use anyhow::{Context, Result, anyhow, bail};
use chrono::{Datelike, NaiveDate, Weekday};
use cp_academics::ops::{AcademicTermOps, TeachingAssignmentOps};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_sis::ops::EnrolmentOps;
use cp_timetabling::ops::TimetablingOps;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::AttendanceOps;
use crate::dtos::{
    AttendanceAccessScope, AttendanceLessonSessionListQuery, AttendanceLessonSessionSummary,
    CancelAttendanceLessonSessionRequest, OpenAttendanceLessonSessionRequest,
    SyncAttendanceLessonSessionsRequest, SyncAttendanceLessonSessionsResponse,
};
use crate::models::AttendanceLessonSessionRow;

const DEFAULT_PAGE: i64 = 1;
const DEFAULT_PER_PAGE: i64 = 25;
const MAX_PER_PAGE: i64 = 100;
const MAX_SYNC_DAYS: i64 = 31;

impl AttendanceOps {
    pub async fn list_lesson_sessions(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &AttendanceLessonSessionListQuery,
        scope: AttendanceAccessScope,
    ) -> Result<(Vec<AttendanceLessonSessionSummary>, i64)> {
        validate_date_range(query.date_from, query.date_to, None)?;
        let (page, per_page) = bounded_page(query.page, query.per_page);
        let offset = (page - 1) * per_page;
        let assignment_ids = scope_assignment_ids(pool, tenant_id, scope).await?;
        let campus_scope = assignment_ids.is_none();
        let assignment_ids = assignment_ids.unwrap_or_default();
        let status = query.status.map(|value| value.as_str());
        let rows = sqlx::query_as::<_, AttendanceLessonSessionRow>(
            r#"
            SELECT id, academic_term_id, class_group_id, teaching_assignment_id,
                   timetable_run_id, session_date,
                   day_key, period_key, status, version, register_id,
                   cancellation_reason, opened_at, completed_at, cancelled_at,
                   created_at
              FROM attendance_lesson_sessions
             WHERE tenant_id = $1 AND deleted_at IS NULL
               AND ($2::DATE IS NULL OR session_date >= $2)
               AND ($3::DATE IS NULL OR session_date <= $3)
               AND ($4::UUID IS NULL OR class_group_id = $4)
               AND ($5::TEXT IS NULL OR status = $5)
               AND ($6 OR teaching_assignment_id = ANY($7))
             ORDER BY session_date, period_key, class_group_id, id
             LIMIT $8 OFFSET $9
            "#,
        )
        .bind(tenant_id)
        .bind(query.date_from)
        .bind(query.date_to)
        .bind(query.class_group_id)
        .bind(status)
        .bind(campus_scope)
        .bind(&assignment_ids)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list Attendance lesson sessions")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
              FROM attendance_lesson_sessions
             WHERE tenant_id = $1 AND deleted_at IS NULL
               AND ($2::DATE IS NULL OR session_date >= $2)
               AND ($3::DATE IS NULL OR session_date <= $3)
               AND ($4::UUID IS NULL OR class_group_id = $4)
               AND ($5::TEXT IS NULL OR status = $5)
               AND ($6 OR teaching_assignment_id = ANY($7))
            "#,
        )
        .bind(tenant_id)
        .bind(query.date_from)
        .bind(query.date_to)
        .bind(query.class_group_id)
        .bind(status)
        .bind(campus_scope)
        .bind(&assignment_ids)
        .fetch_one(pool)
        .await
        .context("Failed to count Attendance lesson sessions")?;
        Ok((hydrate_sessions(pool, tenant_id, rows).await?, total))
    }

    pub async fn get_lesson_session(
        pool: &PgPool,
        tenant_id: Uuid,
        lesson_session_id: Uuid,
        scope: AttendanceAccessScope,
    ) -> Result<Option<AttendanceLessonSessionSummary>> {
        let Some(row) = lesson_session_by_id(pool, tenant_id, lesson_session_id).await? else {
            return Ok(None);
        };
        if !scope_allows_assignment(pool, tenant_id, row.teaching_assignment_id, scope).await? {
            return Ok(None);
        }
        Ok(Some(hydrate_session(pool, tenant_id, row).await?))
    }

    pub async fn sync_lesson_sessions(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &SyncAttendanceLessonSessionsRequest,
        scope: AttendanceAccessScope,
    ) -> Result<SyncAttendanceLessonSessionsResponse> {
        if !matches!(scope, AttendanceAccessScope::Campus) {
            bail!("Campus Attendance scope is required to sync lesson sessions");
        }
        validate_date_range(
            Some(request.date_from),
            Some(request.date_to),
            Some(MAX_SYNC_DAYS),
        )?;
        let actor_id = person_actor_id(actor)?;
        let run = TimetablingOps::latest_published_run(pool, tenant_id)
            .await?
            .context("Publish a timetable before syncing Attendance lesson sessions")?;
        let period = run
            .configuration
            .academic_period
            .as_ref()
            .context("The published timetable has no academic period")?;
        if request.date_from < period.starts_on || request.date_to > period.ends_on {
            bail!("The sync range must stay inside the published timetable's academic term");
        }

        let mut occurrences = Vec::new();
        let mut date = request.date_from;
        while date <= request.date_to {
            for entry in &run.entries {
                let configured_day = run
                    .configuration
                    .days
                    .iter()
                    .find(|day| day.key == entry.day_key)
                    .context("A published timetable entry references an unknown day")?;
                if date.weekday() != parse_weekday(&configured_day.label)? {
                    continue;
                }
                if !run
                    .configuration
                    .periods
                    .iter()
                    .any(|configured| configured.key == entry.period_key)
                {
                    bail!("A published timetable entry references an unknown period");
                }
                let assignment_id = parse_uuid(&entry.requirement_id, "teaching assignment")?;
                let class_id = parse_uuid(&entry.class_id, "class")?;
                let subject_id = parse_uuid(&entry.subject_id, "subject")?;
                let assignment = TeachingAssignmentOps::get_by_id(pool, tenant_id, assignment_id)
                    .await?
                    .context("A published timetable teaching assignment is unavailable")?;
                if assignment.status != "active"
                    || assignment.class_group_id != class_id
                    || assignment.subject_id != subject_id
                {
                    bail!(
                        "A published timetable entry no longer matches its active teaching assignment"
                    );
                }
                occurrences.push((
                    assignment_id,
                    class_id,
                    entry.requirement_id.clone(),
                    date,
                    entry.day_key.clone(),
                    entry.period_key.clone(),
                ));
            }
            date = date
                .succ_opt()
                .context("The Attendance sync date range is invalid")?;
        }

        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Attendance lesson session sync")?;
        let mut created_count = 0_u64;
        let mut existing_count = 0_u64;
        for (assignment_id, class_id, requirement_id, session_date, day_key, period_key) in
            occurrences
        {
            let inserted = sqlx::query_scalar::<_, Uuid>(
                r#"
                INSERT INTO attendance_lesson_sessions (
                    tenant_id, academic_term_id, class_group_id,
                    teaching_assignment_id, timetable_run_id,
                    timetable_requirement_id, session_date, day_key, period_key,
                    created_by
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                ON CONFLICT (tenant_id, timetable_run_id, session_date, day_key,
                             period_key, timetable_requirement_id)
                    WHERE deleted_at IS NULL DO NOTHING
                RETURNING id
                "#,
            )
            .bind(tenant_id)
            .bind(period.academic_term_id)
            .bind(class_id)
            .bind(assignment_id)
            .bind(run.id)
            .bind(requirement_id)
            .bind(session_date)
            .bind(day_key)
            .bind(period_key)
            .bind(actor_id)
            .fetch_optional(&mut *transaction)
            .await
            .context("Failed to materialise an Attendance lesson session")?;
            if let Some(lesson_session_id) = inserted {
                created_count += 1;
                append_lesson_session_event(
                    &mut transaction,
                    LessonSessionEvent {
                        tenant_id,
                        lesson_session_id,
                        event_type: "scheduled",
                        from_status: None,
                        to_status: "scheduled",
                        version: 1,
                        actor_id,
                        reason: None,
                        metadata: json!({"timetable_run_id": run.id}),
                    },
                )
                .await?;
            } else {
                existing_count += 1;
            }
        }
        append_attendance_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "attendance.lesson_sessions.sync",
            AuditTarget::new("timetable_run", run.id.to_string()),
            json!({
                "date_from": request.date_from,
                "date_to": request.date_to,
                "created_count": created_count,
                "existing_count": existing_count
            }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Attendance lesson session sync")?;
        Ok(SyncAttendanceLessonSessionsResponse {
            timetable_run_id: run.id,
            date_from: request.date_from,
            date_to: request.date_to,
            created_count,
            existing_count,
        })
    }

    pub async fn open_lesson_session(
        pool: &PgPool,
        tenant_id: Uuid,
        lesson_session_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &OpenAttendanceLessonSessionRequest,
        scope: AttendanceAccessScope,
    ) -> Result<Option<AttendanceLessonSessionSummary>> {
        let actor_id = person_actor_id(actor)?;
        let idempotency_key = required(&request.idempotency_key, "Idempotency key")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Attendance lesson session opening")?;
        let Some(session) =
            lock_lesson_session(&mut transaction, tenant_id, lesson_session_id).await?
        else {
            return Ok(None);
        };
        if !scope_allows_assignment(pool, tenant_id, session.teaching_assignment_id, scope).await? {
            return Ok(None);
        }
        ensure_session_status(&session, "scheduled")?;
        ensure_session_version(&session, request.expected_version)?;
        let assignment =
            TeachingAssignmentOps::get_by_id(pool, tenant_id, session.teaching_assignment_id)
                .await?
                .context("The lesson's teaching assignment is unavailable")?;
        if assignment.status != "active" {
            bail!("The lesson's teaching assignment is no longer active");
        }
        let term = AcademicTermOps::get_by_id(pool, tenant_id, session.academic_term_id)
            .await?
            .context("The lesson's academic term is unavailable")?;
        if term.status != "active" {
            bail!("Lesson attendance may be opened only in the active academic term");
        }
        let roster = EnrolmentOps::attendance_roster(
            pool,
            tenant_id,
            term.academic_year_id,
            session.class_group_id,
            session.session_date,
        )
        .await?;
        if roster.is_empty() {
            bail!("This lesson has no active learners on the scheduled date");
        }
        let period = format!("lesson:{}", session.period_key);
        let fingerprint = format!(
            "{:x}",
            Sha256::digest(
                format!(
                    "{}|{}|{}|{}",
                    session.id, session.academic_term_id, session.session_date, period
                )
                .as_bytes()
            )
        );
        let register_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO attendance_registers (
                tenant_id, academic_term_id, class_group_id, attendance_date,
                period, idempotency_key, create_request_fingerprint, created_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(session.academic_term_id)
        .bind(session.class_group_id)
        .bind(session.session_date)
        .bind(&period)
        .bind(idempotency_key)
        .bind(fingerprint)
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(lesson_register_database_error)?;
        for learner in &roster {
            sqlx::query(
                r#"
                INSERT INTO attendance_marks (
                    tenant_id, register_id, enrolment_id, learner_id
                ) VALUES ($1, $2, $3, $4)
                "#,
            )
            .bind(tenant_id)
            .bind(register_id)
            .bind(learner.enrolment_id)
            .bind(learner.learner_id)
            .execute(&mut *transaction)
            .await
            .context("Failed to create the lesson Attendance roster")?;
        }
        append_register_event(
            &mut transaction,
            tenant_id,
            register_id,
            "created",
            None,
            "draft",
            1,
            actor_id,
            json!({
                "learner_count": roster.len(),
                "lesson_session_id": lesson_session_id
            }),
        )
        .await?;
        let version = sqlx::query_scalar::<_, i32>(
            r#"
            UPDATE attendance_lesson_sessions
               SET status = 'open', register_id = $3, opened_by = $4,
                   opened_at = NOW(), version = version + 1
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            RETURNING version
            "#,
        )
        .bind(tenant_id)
        .bind(lesson_session_id)
        .bind(register_id)
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to open the Attendance lesson session")?;
        append_lesson_session_event(
            &mut transaction,
            LessonSessionEvent {
                tenant_id,
                lesson_session_id,
                event_type: "opened",
                from_status: Some("scheduled"),
                to_status: "open",
                version,
                actor_id,
                reason: None,
                metadata: json!({"register_id": register_id, "learner_count": roster.len()}),
            },
        )
        .await?;
        append_attendance_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "attendance.lesson_sessions.open",
            AuditTarget::new("attendance_lesson_session", lesson_session_id.to_string()),
            json!({"register_id": register_id, "learner_count": roster.len()}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Attendance lesson session opening")?;
        Self::get_lesson_session(pool, tenant_id, lesson_session_id, scope).await
    }

    pub async fn cancel_lesson_session(
        pool: &PgPool,
        tenant_id: Uuid,
        lesson_session_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CancelAttendanceLessonSessionRequest,
        scope: AttendanceAccessScope,
    ) -> Result<Option<AttendanceLessonSessionSummary>> {
        if !matches!(scope, AttendanceAccessScope::Campus) {
            bail!("Campus Attendance scope is required to cancel lesson sessions");
        }
        let actor_id = person_actor_id(actor)?;
        let reason = required(&request.reason, "Cancellation reason")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Attendance lesson cancellation")?;
        let Some(session) =
            lock_lesson_session(&mut transaction, tenant_id, lesson_session_id).await?
        else {
            return Ok(None);
        };
        ensure_session_status(&session, "scheduled")?;
        ensure_session_version(&session, request.expected_version)?;
        let version = sqlx::query_scalar::<_, i32>(
            r#"
            UPDATE attendance_lesson_sessions
               SET status = 'cancelled', cancelled_by = $3, cancelled_at = NOW(),
                   cancellation_reason = $4, version = version + 1
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            RETURNING version
            "#,
        )
        .bind(tenant_id)
        .bind(lesson_session_id)
        .bind(actor_id)
        .bind(reason)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to cancel the Attendance lesson session")?;
        append_lesson_session_event(
            &mut transaction,
            LessonSessionEvent {
                tenant_id,
                lesson_session_id,
                event_type: "cancelled",
                from_status: Some("scheduled"),
                to_status: "cancelled",
                version,
                actor_id,
                reason: Some(reason),
                metadata: json!({}),
            },
        )
        .await?;
        append_attendance_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "attendance.lesson_sessions.cancel",
            AuditTarget::new("attendance_lesson_session", lesson_session_id.to_string()),
            json!({"reason_recorded": true}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Attendance lesson cancellation")?;
        Self::get_lesson_session(pool, tenant_id, lesson_session_id, scope).await
    }
}

pub(crate) async fn complete_session_for_register(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    register_id: Uuid,
    actor_id: Uuid,
) -> Result<()> {
    let Some((lesson_session_id, current_version)) = sqlx::query_as::<_, (Uuid, i32)>(
        r#"
        SELECT id, version FROM attendance_lesson_sessions
         WHERE tenant_id = $1 AND register_id = $2
           AND status = 'open' AND deleted_at IS NULL
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(register_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock the lesson session for Attendance submission")?
    else {
        return Ok(());
    };
    let version = current_version + 1;
    sqlx::query(
        r#"
        UPDATE attendance_lesson_sessions
           SET status = 'completed', completed_by = $3, completed_at = NOW(),
               version = version + 1
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(lesson_session_id)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await
    .context("Failed to complete the Attendance lesson session")?;
    append_lesson_session_event(
        transaction,
        LessonSessionEvent {
            tenant_id,
            lesson_session_id,
            event_type: "completed",
            from_status: Some("open"),
            to_status: "completed",
            version,
            actor_id,
            reason: None,
            metadata: json!({"register_id": register_id}),
        },
    )
    .await
}

pub(crate) async fn reopen_session_for_register(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    register_id: Uuid,
    actor_id: Uuid,
    reason: &str,
) -> Result<()> {
    let Some((lesson_session_id, current_version)) = sqlx::query_as::<_, (Uuid, i32)>(
        r#"
        SELECT id, version FROM attendance_lesson_sessions
         WHERE tenant_id = $1 AND register_id = $2
           AND status = 'completed' AND deleted_at IS NULL
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(register_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock the lesson session for Attendance reopen")?
    else {
        return Ok(());
    };
    let version = current_version + 1;
    sqlx::query(
        r#"
        UPDATE attendance_lesson_sessions
           SET status = 'open', completed_by = NULL, completed_at = NULL,
               version = version + 1
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(lesson_session_id)
    .execute(&mut **transaction)
    .await
    .context("Failed to reopen the Attendance lesson session")?;
    append_lesson_session_event(
        transaction,
        LessonSessionEvent {
            tenant_id,
            lesson_session_id,
            event_type: "reopened",
            from_status: Some("completed"),
            to_status: "open",
            version,
            actor_id,
            reason: Some(reason),
            metadata: json!({"register_id": register_id}),
        },
    )
    .await
}

pub(crate) async fn detach_deleted_register_session(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    register_id: Uuid,
    actor_id: Uuid,
) -> Result<()> {
    let Some((lesson_session_id, current_version)) = sqlx::query_as::<_, (Uuid, i32)>(
        r#"
        SELECT id, version FROM attendance_lesson_sessions
         WHERE tenant_id = $1 AND register_id = $2
           AND status = 'open' AND deleted_at IS NULL
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(register_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock the lesson session for register deletion")?
    else {
        return Ok(());
    };
    let version = current_version + 1;
    sqlx::query(
        r#"
        UPDATE attendance_lesson_sessions
           SET status = 'scheduled', register_id = NULL,
               opened_by = NULL, opened_at = NULL, version = version + 1
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(lesson_session_id)
    .execute(&mut **transaction)
    .await
    .context("Failed to detach the deleted Attendance register")?;
    append_lesson_session_event(
        transaction,
        LessonSessionEvent {
            tenant_id,
            lesson_session_id,
            event_type: "register_deleted",
            from_status: Some("open"),
            to_status: "scheduled",
            version,
            actor_id,
            reason: None,
            metadata: json!({"register_id": register_id}),
        },
    )
    .await
}

async fn hydrate_sessions(
    pool: &PgPool,
    tenant_id: Uuid,
    rows: Vec<AttendanceLessonSessionRow>,
) -> Result<Vec<AttendanceLessonSessionSummary>> {
    let mut sessions = Vec::with_capacity(rows.len());
    for row in rows {
        sessions.push(hydrate_session(pool, tenant_id, row).await?);
    }
    Ok(sessions)
}

async fn hydrate_session(
    pool: &PgPool,
    tenant_id: Uuid,
    row: AttendanceLessonSessionRow,
) -> Result<AttendanceLessonSessionSummary> {
    let assignment = TeachingAssignmentOps::get_by_id(pool, tenant_id, row.teaching_assignment_id)
        .await?
        .context("An Attendance lesson teaching assignment is unavailable")?;
    let term = AcademicTermOps::get_by_id(pool, tenant_id, row.academic_term_id)
        .await?
        .context("An Attendance lesson academic term is unavailable")?;
    Ok(AttendanceLessonSessionSummary {
        id: row.id,
        academic_term_id: row.academic_term_id,
        academic_term_name: term.name,
        class_group_id: row.class_group_id,
        class_group_name: assignment.class_group_name,
        teaching_assignment_id: row.teaching_assignment_id,
        subject_id: assignment.subject_id,
        subject_name: assignment.subject_name,
        teacher_name: assignment.teacher_name,
        timetable_run_id: row.timetable_run_id,
        session_date: row.session_date,
        day_key: row.day_key,
        period_key: row.period_key,
        status: row.status,
        version: row.version,
        register_id: row.register_id,
        cancellation_reason: row.cancellation_reason,
        opened_at: row.opened_at,
        completed_at: row.completed_at,
        cancelled_at: row.cancelled_at,
        created_at: row.created_at,
    })
}

async fn lesson_session_by_id(
    pool: &PgPool,
    tenant_id: Uuid,
    lesson_session_id: Uuid,
) -> Result<Option<AttendanceLessonSessionRow>> {
    sqlx::query_as::<_, AttendanceLessonSessionRow>(
        r#"
        SELECT id, academic_term_id, class_group_id, teaching_assignment_id,
               timetable_run_id, session_date,
               day_key, period_key, status, version, register_id,
               cancellation_reason, opened_at, completed_at, cancelled_at,
               created_at
          FROM attendance_lesson_sessions
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(lesson_session_id)
    .fetch_optional(pool)
    .await
    .context("Failed to load the Attendance lesson session")
}

async fn lock_lesson_session(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    lesson_session_id: Uuid,
) -> Result<Option<AttendanceLessonSessionRow>> {
    sqlx::query_as::<_, AttendanceLessonSessionRow>(
        r#"
        SELECT id, academic_term_id, class_group_id, teaching_assignment_id,
               timetable_run_id, session_date,
               day_key, period_key, status, version, register_id,
               cancellation_reason, opened_at, completed_at, cancelled_at,
               created_at
          FROM attendance_lesson_sessions
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(lesson_session_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock the Attendance lesson session")
}

async fn scope_assignment_ids(
    pool: &PgPool,
    tenant_id: Uuid,
    scope: AttendanceAccessScope,
) -> Result<Option<Vec<Uuid>>> {
    match scope {
        AttendanceAccessScope::Campus => Ok(None),
        AttendanceAccessScope::AssignedTo(account_id) => Ok(Some(
            TeachingAssignmentOps::active_for_account(pool, tenant_id, account_id)
                .await?
                .into_iter()
                .map(|assignment| assignment.id)
                .collect(),
        )),
    }
}

async fn scope_allows_assignment(
    pool: &PgPool,
    tenant_id: Uuid,
    assignment_id: Uuid,
    scope: AttendanceAccessScope,
) -> Result<bool> {
    match scope {
        AttendanceAccessScope::Campus => Ok(true),
        AttendanceAccessScope::AssignedTo(account_id) => {
            TeachingAssignmentOps::is_active_for_account(pool, tenant_id, assignment_id, account_id)
                .await
        }
    }
}

fn validate_date_range(
    date_from: Option<NaiveDate>,
    date_to: Option<NaiveDate>,
    maximum_days: Option<i64>,
) -> Result<()> {
    if let Some((from, to)) = date_from.zip(date_to) {
        if to < from {
            bail!("The Attendance lesson-session date range is invalid");
        }
        if maximum_days.is_some_and(|maximum| (to - from).num_days() + 1 > maximum) {
            bail!("Attendance lesson sessions may be synced for at most 31 days at a time");
        }
    }
    Ok(())
}

fn parse_weekday(value: &str) -> Result<Weekday> {
    match value.trim().to_ascii_lowercase().as_str() {
        "monday" | "mon" => Ok(Weekday::Mon),
        "tuesday" | "tue" | "tues" => Ok(Weekday::Tue),
        "wednesday" | "wed" => Ok(Weekday::Wed),
        "thursday" | "thu" | "thur" | "thurs" => Ok(Weekday::Thu),
        "friday" | "fri" => Ok(Weekday::Fri),
        "saturday" | "sat" => Ok(Weekday::Sat),
        "sunday" | "sun" => Ok(Weekday::Sun),
        _ => bail!("Published timetable days must use weekday labels"),
    }
}

fn parse_uuid(value: &str, label: &str) -> Result<Uuid> {
    Uuid::parse_str(value).with_context(|| format!("The published timetable {label} is invalid"))
}

fn ensure_session_status(session: &AttendanceLessonSessionRow, expected: &str) -> Result<()> {
    if session.status != expected {
        bail!("This Attendance lesson session is no longer {expected}");
    }
    Ok(())
}

fn ensure_session_version(session: &AttendanceLessonSessionRow, expected: i32) -> Result<()> {
    if session.version != expected {
        bail!("This Attendance lesson session changed. Reload it before continuing");
    }
    Ok(())
}

struct LessonSessionEvent<'a> {
    tenant_id: Uuid,
    lesson_session_id: Uuid,
    event_type: &'a str,
    from_status: Option<&'a str>,
    to_status: &'a str,
    version: i32,
    actor_id: Uuid,
    reason: Option<&'a str>,
    metadata: Value,
}

async fn append_lesson_session_event(
    transaction: &mut Transaction<'_, Postgres>,
    event: LessonSessionEvent<'_>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO attendance_lesson_session_events (
            tenant_id, lesson_session_id, event_type, from_status, to_status,
            session_version, actor_id, reason, metadata
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(event.tenant_id)
    .bind(event.lesson_session_id)
    .bind(event.event_type)
    .bind(event.from_status)
    .bind(event.to_status)
    .bind(event.version)
    .bind(event.actor_id)
    .bind(event.reason)
    .bind(event.metadata)
    .execute(&mut **transaction)
    .await
    .context("Failed to append Attendance lesson-session history")?;
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "Attendance audit evidence is explicit"
)]
async fn append_register_event(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    register_id: Uuid,
    event_type: &str,
    from_status: Option<&str>,
    to_status: &str,
    version: i32,
    actor_id: Uuid,
    metadata: Value,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO attendance_register_events (
            tenant_id, register_id, event_type, from_status, to_status,
            register_version, actor_id, metadata
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(tenant_id)
    .bind(register_id)
    .bind(event_type)
    .bind(from_status)
    .bind(to_status)
    .bind(version)
    .bind(actor_id)
    .bind(metadata)
    .execute(&mut **transaction)
    .await
    .context("Failed to append lesson Attendance register history")?;
    Ok(())
}

async fn append_attendance_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    action: &str,
    target: AuditTarget,
    metadata: Value,
) -> Result<()> {
    let metadata = match metadata {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    append_audit(
        &mut **transaction,
        &NewAuditEvent::new(
            tenant_id,
            actor,
            action,
            AuditOutcome::Succeeded,
            request_context,
        )
        .with_target(target)
        .with_redacted_metadata(metadata),
    )
    .await
    .context("Failed to append Attendance audit evidence")?;
    Ok(())
}

fn person_actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("Authenticated person actor is required"))
}

fn required<'a>(value: &'a str, label: &str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{label} is required");
    }
    Ok(value)
}

fn lesson_register_database_error(error: sqlx::Error) -> anyhow::Error {
    if let sqlx::Error::Database(database) = &error {
        if database.constraint() == Some("idx_attendance_registers_scope") {
            return anyhow!(
                "A lesson attendance register already exists for this class and period"
            );
        }
        if database.constraint() == Some("idx_attendance_registers_idempotency") {
            return anyhow!("This lesson attendance request has already been processed");
        }
    }
    anyhow!("Failed to create the lesson Attendance register: {error}")
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(DEFAULT_PAGE).clamp(1, 1_000_000),
        per_page.unwrap_or(DEFAULT_PER_PAGE).clamp(1, MAX_PER_PAGE),
    )
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, Weekday};

    use super::{parse_weekday, validate_date_range};

    #[test]
    fn weekday_parser_accepts_school_day_labels() {
        assert_eq!(
            parse_weekday("Monday").unwrap_or_else(|_| unreachable!()),
            Weekday::Mon
        );
        assert_eq!(
            parse_weekday("Thurs").unwrap_or_else(|_| unreachable!()),
            Weekday::Thu
        );
        assert!(parse_weekday("Day one").is_err());
    }

    #[test]
    fn session_sync_range_is_bounded() {
        let from = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap_or_else(|| unreachable!());
        let thirty_one = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap_or_else(|| unreachable!());
        let thirty_two = NaiveDate::from_ymd_opt(2026, 10, 2).unwrap_or_else(|| unreachable!());
        assert!(validate_date_range(Some(from), Some(thirty_one), Some(31)).is_ok());
        assert!(validate_date_range(Some(from), Some(thirty_two), Some(31)).is_err());
        assert!(validate_date_range(Some(thirty_two), Some(from), Some(31)).is_err());
    }
}
