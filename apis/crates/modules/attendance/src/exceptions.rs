//! Campus Attendance exception follow-up over immutable submission evidence.

use anyhow::{Context, Result, anyhow, bail};
use cp_academics::ops::ClassGroupOps;
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_sis::ops::EnrolmentOps;
use serde_json::{Map, Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::AttendanceOps;
use crate::dtos::{
    AcknowledgeAttendanceExceptionRequest, AttendanceAccessScope, AttendanceExceptionListQuery,
    AttendanceExceptionResponse, ReopenAttendanceExceptionRequest,
    ResolveAttendanceExceptionRequest,
};
use crate::models::AttendanceExceptionRow;

const DEFAULT_PAGE: i64 = 1;
const DEFAULT_PER_PAGE: i64 = 25;
const MAX_PER_PAGE: i64 = 100;

impl AttendanceOps {
    pub async fn list_exceptions(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &AttendanceExceptionListQuery,
        scope: AttendanceAccessScope,
    ) -> Result<(Vec<AttendanceExceptionResponse>, i64)> {
        require_campus_scope(scope)?;
        validate_date_range(query)?;
        let (page, per_page) = bounded_page(query.page, query.per_page);
        let offset = (page - 1) * per_page;
        let status = query.status.map(|value| value.as_str());
        let mark = query.mark.map(|value| value.as_str());
        let rows = sqlx::query_as::<_, AttendanceExceptionRow>(&format!(
            "{} ORDER BY exception.attendance_date DESC, exception.updated_at DESC, exception.id LIMIT $8 OFFSET $9",
            exception_select()
        ))
        .bind(tenant_id)
        .bind(query.date_from)
        .bind(query.date_to)
        .bind(query.class_group_id)
        .bind(status)
        .bind(mark)
        .bind(Option::<Uuid>::None)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list Attendance exceptions")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM attendance_exceptions AS exception
             WHERE exception.tenant_id = $1 AND exception.deleted_at IS NULL
               AND ($2::DATE IS NULL OR exception.attendance_date >= $2)
               AND ($3::DATE IS NULL OR exception.attendance_date <= $3)
               AND ($4::UUID IS NULL OR exception.class_group_id = $4)
               AND ($5::TEXT IS NULL OR exception.status = $5)
               AND ($6::TEXT IS NULL OR exception.mark = $6)
            "#,
        )
        .bind(tenant_id)
        .bind(query.date_from)
        .bind(query.date_to)
        .bind(query.class_group_id)
        .bind(status)
        .bind(mark)
        .fetch_one(pool)
        .await
        .context("Failed to count Attendance exceptions")?;
        Ok((hydrate_exceptions(pool, tenant_id, rows).await?, total))
    }

    pub async fn get_exception(
        pool: &PgPool,
        tenant_id: Uuid,
        exception_id: Uuid,
        scope: AttendanceAccessScope,
    ) -> Result<Option<AttendanceExceptionResponse>> {
        require_campus_scope(scope)?;
        let row = sqlx::query_as::<_, AttendanceExceptionRow>(exception_select())
            .bind(tenant_id)
            .bind(Option::<chrono::NaiveDate>::None)
            .bind(Option::<chrono::NaiveDate>::None)
            .bind(Option::<Uuid>::None)
            .bind(Option::<&str>::None)
            .bind(Option::<&str>::None)
            .bind(exception_id)
            .fetch_optional(pool)
            .await
            .context("Failed to load the Attendance exception")?;
        match row {
            Some(value) => Ok(Some(hydrate_exception(pool, tenant_id, value).await?)),
            None => Ok(None),
        }
    }

    pub async fn acknowledge_exception(
        pool: &PgPool,
        tenant_id: Uuid,
        exception_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &AcknowledgeAttendanceExceptionRequest,
        scope: AttendanceAccessScope,
    ) -> Result<Option<AttendanceExceptionResponse>> {
        require_campus_scope(scope)?;
        let actor_id = person_actor_id(actor)?;
        let note = required(&request.note, "Acknowledgement note")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Attendance exception acknowledgement")?;
        let Some(current) = lock_exception(&mut transaction, tenant_id, exception_id).await? else {
            return Ok(None);
        };
        ensure_status(&current, "open")?;
        ensure_version(&current, request.expected_version)?;
        let version = sqlx::query_scalar::<_, i32>(
            r#"
            UPDATE attendance_exceptions
               SET status = 'acknowledged', acknowledged_by = $3,
                   acknowledged_at = NOW(), acknowledgement_note = $4,
                   version = version + 1
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            RETURNING version
            "#,
        )
        .bind(tenant_id)
        .bind(exception_id)
        .bind(actor_id)
        .bind(note)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to acknowledge the Attendance exception")?;
        append_exception_event(
            &mut transaction,
            ExceptionEvent {
                tenant_id,
                exception_id,
                event_type: "acknowledged",
                from_status: Some("open"),
                to_status: "acknowledged",
                version,
                actor_id,
                reason: Some(note),
                metadata: json!({}),
            },
        )
        .await?;
        append_exception_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "attendance.exceptions.acknowledge",
            exception_id,
            json!({"note_recorded": true}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Attendance exception acknowledgement")?;
        Self::get_exception(pool, tenant_id, exception_id, scope).await
    }

    pub async fn resolve_exception(
        pool: &PgPool,
        tenant_id: Uuid,
        exception_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ResolveAttendanceExceptionRequest,
        scope: AttendanceAccessScope,
    ) -> Result<Option<AttendanceExceptionResponse>> {
        require_campus_scope(scope)?;
        let actor_id = person_actor_id(actor)?;
        let resolution = required(&request.resolution, "Resolution")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Attendance exception resolution")?;
        let Some(current) = lock_exception(&mut transaction, tenant_id, exception_id).await? else {
            return Ok(None);
        };
        if !matches!(current.status.as_str(), "open" | "acknowledged") {
            bail!("Only an open or acknowledged Attendance exception can be resolved");
        }
        ensure_version(&current, request.expected_version)?;
        let version = sqlx::query_scalar::<_, i32>(
            r#"
            UPDATE attendance_exceptions
               SET status = 'resolved', resolved_by = $3, resolved_at = NOW(),
                   resolution = $4, version = version + 1
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            RETURNING version
            "#,
        )
        .bind(tenant_id)
        .bind(exception_id)
        .bind(actor_id)
        .bind(resolution)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to resolve the Attendance exception")?;
        append_exception_event(
            &mut transaction,
            ExceptionEvent {
                tenant_id,
                exception_id,
                event_type: "resolved",
                from_status: Some(&current.status),
                to_status: "resolved",
                version,
                actor_id,
                reason: Some(resolution),
                metadata: json!({}),
            },
        )
        .await?;
        append_exception_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "attendance.exceptions.resolve",
            exception_id,
            json!({"resolution_recorded": true}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Attendance exception resolution")?;
        Self::get_exception(pool, tenant_id, exception_id, scope).await
    }

    pub async fn reopen_exception(
        pool: &PgPool,
        tenant_id: Uuid,
        exception_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ReopenAttendanceExceptionRequest,
        scope: AttendanceAccessScope,
    ) -> Result<Option<AttendanceExceptionResponse>> {
        require_campus_scope(scope)?;
        let actor_id = person_actor_id(actor)?;
        let reason = required(&request.reason, "Reopen reason")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Attendance exception reopen")?;
        let Some(current) = lock_exception(&mut transaction, tenant_id, exception_id).await? else {
            return Ok(None);
        };
        ensure_status(&current, "resolved")?;
        ensure_version(&current, request.expected_version)?;
        let version = sqlx::query_scalar::<_, i32>(
            r#"
            UPDATE attendance_exceptions
               SET status = 'open', acknowledged_by = NULL,
                   acknowledged_at = NULL, acknowledgement_note = NULL,
                   resolved_by = NULL, resolved_at = NULL, resolution = NULL,
                   reopened_by = $3, reopened_at = NOW(), reopen_reason = $4,
                   version = version + 1
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            RETURNING version
            "#,
        )
        .bind(tenant_id)
        .bind(exception_id)
        .bind(actor_id)
        .bind(reason)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to reopen the Attendance exception")?;
        append_exception_event(
            &mut transaction,
            ExceptionEvent {
                tenant_id,
                exception_id,
                event_type: "reopened",
                from_status: Some("resolved"),
                to_status: "open",
                version,
                actor_id,
                reason: Some(reason),
                metadata: json!({}),
            },
        )
        .await?;
        append_exception_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "attendance.exceptions.reopen",
            exception_id,
            json!({"reason_recorded": true}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Attendance exception reopen")?;
        Self::get_exception(pool, tenant_id, exception_id, scope).await
    }
}

/// Reconciles the current exception queue to one newly accepted register
/// version. The immutable submitted-mark table remains the source evidence.
pub(crate) async fn refresh_exceptions_for_submission(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    register_id: Uuid,
    register_version: i32,
    actor_id: Uuid,
) -> Result<()> {
    let marks = sqlx::query_as::<_, SubmissionMark>(
        r#"
        SELECT event.enrolment_id, event.learner_id, register.class_group_id,
               event.attendance_date, event.period, event.mark,
               event.minutes_late, event.note AS attendance_note,
               event.submitted_at
          FROM attendance_submission_mark_events event
          JOIN attendance_registers register
            ON register.id = event.register_id
           AND register.tenant_id = event.tenant_id
         WHERE event.tenant_id = $1 AND event.register_id = $2
           AND event.register_version = $3
         ORDER BY event.learner_id
        "#,
    )
    .bind(tenant_id)
    .bind(register_id)
    .bind(register_version)
    .fetch_all(&mut **transaction)
    .await
    .context("Failed to load submitted marks for Attendance exception reconciliation")?;

    for mark in marks {
        let current = sqlx::query_as::<_, CurrentException>(
            r#"
            SELECT id, status, version
              FROM attendance_exceptions
             WHERE tenant_id = $1 AND register_id = $2 AND learner_id = $3
               AND deleted_at IS NULL
             FOR UPDATE
            "#,
        )
        .bind(tenant_id)
        .bind(register_id)
        .bind(mark.learner_id)
        .fetch_optional(&mut **transaction)
        .await
        .context("Failed to lock an Attendance exception")?;
        if matches!(mark.mark.as_str(), "absent" | "late" | "excused") {
            if let Some(current) = current {
                let version = current.version + 1;
                sqlx::query(
                    r#"
                    UPDATE attendance_exceptions
                       SET source_register_version = $4, attendance_date = $5,
                           period = $6, mark = $7, minutes_late = $8,
                           attendance_note = $9, source_submitted_at = $10,
                           status = 'open', acknowledged_by = NULL,
                           acknowledged_at = NULL, acknowledgement_note = NULL,
                           resolved_by = NULL, resolved_at = NULL, resolution = NULL,
                           reopened_by = NULL, reopened_at = NULL, reopen_reason = NULL,
                           version = version + 1
                     WHERE tenant_id = $1 AND id = $2 AND register_id = $3
                       AND deleted_at IS NULL
                    "#,
                )
                .bind(tenant_id)
                .bind(current.id)
                .bind(register_id)
                .bind(register_version)
                .bind(mark.attendance_date)
                .bind(&mark.period)
                .bind(&mark.mark)
                .bind(mark.minutes_late)
                .bind(&mark.attendance_note)
                .bind(mark.submitted_at)
                .execute(&mut **transaction)
                .await
                .context("Failed to refresh an Attendance exception")?;
                append_exception_event(
                    transaction,
                    ExceptionEvent {
                        tenant_id,
                        exception_id: current.id,
                        event_type: "evidence_refreshed",
                        from_status: Some(&current.status),
                        to_status: "open",
                        version,
                        actor_id,
                        reason: None,
                        metadata: json!({
                            "register_id": register_id,
                            "register_version": register_version,
                            "mark": mark.mark
                        }),
                    },
                )
                .await?;
            } else {
                let exception_id = sqlx::query_scalar::<_, Uuid>(
                    r#"
                    INSERT INTO attendance_exceptions (
                        tenant_id, register_id, enrolment_id, learner_id,
                        class_group_id, source_register_version, attendance_date,
                        period, mark, minutes_late, attendance_note,
                        source_submitted_at
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                    RETURNING id
                    "#,
                )
                .bind(tenant_id)
                .bind(register_id)
                .bind(mark.enrolment_id)
                .bind(mark.learner_id)
                .bind(mark.class_group_id)
                .bind(register_version)
                .bind(mark.attendance_date)
                .bind(&mark.period)
                .bind(&mark.mark)
                .bind(mark.minutes_late)
                .bind(&mark.attendance_note)
                .bind(mark.submitted_at)
                .fetch_one(&mut **transaction)
                .await
                .context("Failed to create an Attendance exception")?;
                append_exception_event(
                    transaction,
                    ExceptionEvent {
                        tenant_id,
                        exception_id,
                        event_type: "created",
                        from_status: None,
                        to_status: "open",
                        version: 1,
                        actor_id,
                        reason: None,
                        metadata: json!({
                            "register_id": register_id,
                            "register_version": register_version,
                            "mark": mark.mark
                        }),
                    },
                )
                .await?;
            }
        } else if mark.mark == "present"
            && let Some(current) = current
            && current.status != "resolved"
        {
            let version = current.version + 1;
            let resolution = "Attendance corrected to present in the submitted register";
            sqlx::query(
                r#"
                UPDATE attendance_exceptions
                   SET source_register_version = $4, attendance_date = $5,
                       period = $6, source_submitted_at = $7,
                       status = 'resolved', resolved_by = $8, resolved_at = NOW(),
                       resolution = $9, version = version + 1
                 WHERE tenant_id = $1 AND id = $2 AND register_id = $3
                   AND deleted_at IS NULL
                "#,
            )
            .bind(tenant_id)
            .bind(current.id)
            .bind(register_id)
            .bind(register_version)
            .bind(mark.attendance_date)
            .bind(&mark.period)
            .bind(mark.submitted_at)
            .bind(actor_id)
            .bind(resolution)
            .execute(&mut **transaction)
            .await
            .context("Failed to resolve a corrected Attendance exception")?;
            append_exception_event(
                transaction,
                ExceptionEvent {
                    tenant_id,
                    exception_id: current.id,
                    event_type: "auto_resolved",
                    from_status: Some(&current.status),
                    to_status: "resolved",
                    version,
                    actor_id,
                    reason: Some(resolution),
                    metadata: json!({
                        "register_id": register_id,
                        "register_version": register_version
                    }),
                },
            )
            .await?;
        }
    }
    Ok(())
}

#[derive(Debug, sqlx::FromRow)]
struct SubmissionMark {
    enrolment_id: Uuid,
    learner_id: Uuid,
    class_group_id: Uuid,
    attendance_date: chrono::NaiveDate,
    period: String,
    mark: String,
    minutes_late: Option<i32>,
    attendance_note: Option<String>,
    submitted_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct CurrentException {
    id: Uuid,
    status: String,
    version: i32,
}

async fn hydrate_exceptions(
    pool: &PgPool,
    tenant_id: Uuid,
    rows: Vec<AttendanceExceptionRow>,
) -> Result<Vec<AttendanceExceptionResponse>> {
    let mut values = Vec::with_capacity(rows.len());
    for row in rows {
        values.push(hydrate_exception(pool, tenant_id, row).await?);
    }
    Ok(values)
}

async fn hydrate_exception(
    pool: &PgPool,
    tenant_id: Uuid,
    row: AttendanceExceptionRow,
) -> Result<AttendanceExceptionResponse> {
    let identity = EnrolmentOps::attendance_references_by_ids(pool, tenant_id, &[row.enrolment_id])
        .await?
        .into_iter()
        .next()
        .context("An Attendance exception learner is unavailable")?;
    let class = ClassGroupOps::get_by_id(pool, tenant_id, row.class_group_id)
        .await?
        .context("An Attendance exception class is unavailable")?;
    Ok(AttendanceExceptionResponse {
        id: row.id,
        register_id: row.register_id,
        enrolment_id: row.enrolment_id,
        learner_id: row.learner_id,
        learner_number: identity.learner_number,
        learner_name: identity.display_name,
        class_group_id: row.class_group_id,
        class_group_name: class.name,
        source_register_version: row.source_register_version,
        attendance_date: row.attendance_date,
        period: row.period,
        mark: row.mark,
        minutes_late: row.minutes_late,
        attendance_note: row.attendance_note,
        source_submitted_at: row.source_submitted_at,
        status: row.status,
        version: row.version,
        acknowledged_at: row.acknowledged_at,
        acknowledgement_note: row.acknowledgement_note,
        resolved_at: row.resolved_at,
        resolution: row.resolution,
        reopened_at: row.reopened_at,
        reopen_reason: row.reopen_reason,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn exception_select() -> &'static str {
    r#"
    SELECT exception.id, exception.register_id, exception.enrolment_id,
           exception.learner_id, exception.class_group_id,
           exception.source_register_version, exception.attendance_date,
           exception.period, exception.mark, exception.minutes_late,
           exception.attendance_note, exception.source_submitted_at,
           exception.status, exception.version, exception.acknowledged_at,
           exception.acknowledgement_note, exception.resolved_at,
           exception.resolution, exception.reopened_at, exception.reopen_reason,
           exception.created_at, exception.updated_at
      FROM attendance_exceptions AS exception
     WHERE exception.tenant_id = $1 AND exception.deleted_at IS NULL
       AND ($2::DATE IS NULL OR exception.attendance_date >= $2)
       AND ($3::DATE IS NULL OR exception.attendance_date <= $3)
       AND ($4::UUID IS NULL OR exception.class_group_id = $4)
       AND ($5::TEXT IS NULL OR exception.status = $5)
       AND ($6::TEXT IS NULL OR exception.mark = $6)
       AND ($7::UUID IS NULL OR exception.id = $7)
    "#
}

async fn lock_exception(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    exception_id: Uuid,
) -> Result<Option<AttendanceExceptionRow>> {
    sqlx::query_as::<_, AttendanceExceptionRow>(&format!("{} FOR UPDATE", exception_select()))
        .bind(tenant_id)
        .bind(Option::<chrono::NaiveDate>::None)
        .bind(Option::<chrono::NaiveDate>::None)
        .bind(Option::<Uuid>::None)
        .bind(Option::<&str>::None)
        .bind(Option::<&str>::None)
        .bind(exception_id)
        .fetch_optional(&mut **transaction)
        .await
        .context("Failed to lock the Attendance exception")
}

struct ExceptionEvent<'a> {
    tenant_id: Uuid,
    exception_id: Uuid,
    event_type: &'a str,
    from_status: Option<&'a str>,
    to_status: &'a str,
    version: i32,
    actor_id: Uuid,
    reason: Option<&'a str>,
    metadata: Value,
}

async fn append_exception_event(
    transaction: &mut Transaction<'_, Postgres>,
    event: ExceptionEvent<'_>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO attendance_exception_events (
            tenant_id, exception_id, event_type, from_status, to_status,
            exception_version, actor_id, reason, metadata
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(event.tenant_id)
    .bind(event.exception_id)
    .bind(event.event_type)
    .bind(event.from_status)
    .bind(event.to_status)
    .bind(event.version)
    .bind(event.actor_id)
    .bind(event.reason)
    .bind(event.metadata)
    .execute(&mut **transaction)
    .await
    .context("Failed to append Attendance exception history")?;
    Ok(())
}

async fn append_exception_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    action: &str,
    exception_id: Uuid,
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
        .with_target(AuditTarget::new(
            "attendance_exception",
            exception_id.to_string(),
        ))
        .with_redacted_metadata(metadata),
    )
    .await
    .context("Failed to append Attendance exception audit evidence")?;
    Ok(())
}

fn require_campus_scope(scope: AttendanceAccessScope) -> Result<()> {
    if !matches!(scope, AttendanceAccessScope::Campus) {
        bail!("Campus Attendance scope is required for exception follow-up");
    }
    Ok(())
}

fn validate_date_range(query: &AttendanceExceptionListQuery) -> Result<()> {
    if query
        .date_from
        .zip(query.date_to)
        .is_some_and(|(from, to)| to < from)
    {
        bail!("The Attendance exception date range is invalid");
    }
    Ok(())
}

fn ensure_status(row: &AttendanceExceptionRow, expected: &str) -> Result<()> {
    if row.status != expected {
        bail!("This Attendance exception is no longer {expected}");
    }
    Ok(())
}

fn ensure_version(row: &AttendanceExceptionRow, expected: i32) -> Result<()> {
    if row.version != expected {
        bail!("This Attendance exception changed. Reload it before continuing");
    }
    Ok(())
}

fn required<'a>(value: &'a str, label: &str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{label} is required");
    }
    Ok(value)
}

fn person_actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("Authenticated person actor is required"))
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(DEFAULT_PAGE).clamp(1, 1_000_000),
        per_page.unwrap_or(DEFAULT_PER_PAGE).clamp(1, MAX_PER_PAGE),
    )
}
