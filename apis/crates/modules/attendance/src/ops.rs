//! Transactional Attendance register operations.
//!
//! Writes use optimistic versions, preserve an immutable event trail, and
//! append actor-aware audit evidence in the same transaction. Submitted
//! registers are immutable until an explicit, reasoned reopen transition.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::{Context, Result, anyhow, bail};
use cp_academics::ops::{AcademicTermOps, AcademicYearOps, ClassGroupOps};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_sis::ops::EnrolmentOps;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::dtos::{
    AttendanceClassReference, AttendanceLearnerSummary, AttendanceMarkInput,
    AttendanceMarkResponse, AttendanceMarkStatus, AttendanceReferenceData,
    AttendanceRegisterListQuery, AttendanceRegisterResponse, AttendanceRegisterSummary,
    AttendanceTermReference, CreateAttendanceRegisterRequest, ReopenAttendanceRegisterRequest,
    UpdateAttendanceMarksRequest,
};
use crate::models::{AttendanceMarkRow, AttendanceRegisterRow, AttendanceRegisterSummaryRow};

const DEFAULT_PAGE: i64 = 1;
const DEFAULT_PER_PAGE: i64 = 25;
const MAX_PAGE: i64 = 1_000_000;
const MAX_PER_PAGE: i64 = 100;

pub struct AttendanceOps;

impl AttendanceOps {
    /// Aggregates submitted class attendance within one inclusive term range.
    /// Draft registers never enter academic reports.
    pub async fn submitted_summaries_for_class(
        pool: &PgPool,
        tenant_id: Uuid,
        class_group_id: Uuid,
        starts_on: chrono::NaiveDate,
        ends_on: chrono::NaiveDate,
    ) -> Result<Vec<AttendanceLearnerSummary>> {
        sqlx::query_as::<_, AttendanceLearnerSummary>(
            r#"
            SELECT mark.enrolment_id,
                   mark.learner_id,
                   COUNT(*) FILTER (WHERE mark.mark_status = 'present') AS present_count,
                   COUNT(*) FILTER (WHERE mark.mark_status = 'absent') AS absent_count,
                   COUNT(*) FILTER (WHERE mark.mark_status = 'late') AS late_count,
                   COUNT(*) FILTER (WHERE mark.mark_status = 'excused') AS excused_count
              FROM attendance_registers AS register
              JOIN attendance_marks AS mark
                ON mark.attendance_register_id = register.id
               AND mark.tenant_id = register.tenant_id
               AND mark.deleted_at IS NULL
             WHERE register.tenant_id = $1
               AND register.class_group_id = $2
               AND register.attendance_date BETWEEN $3 AND $4
               AND register.status = 'submitted'
               AND register.deleted_at IS NULL
             GROUP BY mark.enrolment_id, mark.learner_id
             ORDER BY mark.learner_id
            "#,
        )
        .bind(tenant_id)
        .bind(class_group_id)
        .bind(starts_on)
        .bind(ends_on)
        .fetch_all(pool)
        .await
        .context("Failed to load submitted attendance summaries")
    }

    /// Returns the active term and active classes that may own a register.
    pub async fn reference_data(
        pool: &PgPool,
        tenant_id: Uuid,
    ) -> Result<Option<AttendanceReferenceData>> {
        let Some(year) = AcademicYearOps::get_active(pool, tenant_id).await? else {
            return Ok(None);
        };
        let Some(term) = AcademicTermOps::get_active_for_year(pool, tenant_id, year.id).await?
        else {
            return Ok(None);
        };
        let (classes, _) = ClassGroupOps::list(
            pool,
            tenant_id,
            1,
            1_000,
            None,
            Some("active"),
            Some(year.id),
            None,
        )
        .await?;
        Ok(Some(AttendanceReferenceData {
            term: AttendanceTermReference {
                id: term.id,
                academic_year_id: term.academic_year_id,
                academic_year_name: term.academic_year_name,
                code: term.code,
                name: term.name,
                starts_on: term.starts_on,
                ends_on: term.ends_on,
            },
            classes: classes
                .into_iter()
                .map(|class| AttendanceClassReference {
                    id: class.id,
                    code: class.code,
                    name: class.name,
                    grade_level: class.grade_level,
                })
                .collect(),
        }))
    }

    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &AttendanceRegisterListQuery,
    ) -> Result<(Vec<AttendanceRegisterSummary>, i64)> {
        let (page, per_page) = bounded_page(query.page, query.per_page);
        if query
            .date_from
            .zip(query.date_to)
            .is_some_and(|(from, to)| to < from)
        {
            bail!("The attendance date range is invalid");
        }
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, AttendanceRegisterSummaryRow>(
            r#"
            SELECT register.id, register.academic_term_id, register.class_group_id,
                   register.attendance_date, register.period, register.status,
                   register.version, register.created_at, register.submitted_at,
                   COUNT(mark.id)::BIGINT AS learner_count,
                   COUNT(mark.id) FILTER (WHERE mark.mark = 'present')::BIGINT AS present_count,
                   COUNT(mark.id) FILTER (WHERE mark.mark = 'absent')::BIGINT AS absent_count,
                   COUNT(mark.id) FILTER (WHERE mark.mark = 'late')::BIGINT AS late_count,
                   COUNT(mark.id) FILTER (WHERE mark.mark = 'excused')::BIGINT AS excused_count,
                   COUNT(mark.id) FILTER (WHERE mark.mark = 'unmarked')::BIGINT AS unmarked_count
              FROM attendance_registers AS register
              LEFT JOIN attendance_marks AS mark
                ON mark.tenant_id = register.tenant_id
               AND mark.register_id = register.id
               AND mark.deleted_at IS NULL
             WHERE register.tenant_id = $1 AND register.deleted_at IS NULL
               AND ($2::DATE IS NULL OR register.attendance_date >= $2)
               AND ($3::DATE IS NULL OR register.attendance_date <= $3)
               AND ($4::UUID IS NULL OR register.class_group_id = $4)
               AND ($5::TEXT IS NULL OR register.period = $5)
               AND ($6::TEXT IS NULL OR register.status = $6)
             GROUP BY register.id
             ORDER BY register.attendance_date DESC, register.created_at DESC, register.id
             LIMIT $7 OFFSET $8
            "#,
        )
        .bind(tenant_id)
        .bind(query.date_from)
        .bind(query.date_to)
        .bind(query.class_group_id)
        .bind(query.period.map(|period| period.as_str()))
        .bind(query.status.map(|status| status.as_str()))
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list attendance registers")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
              FROM attendance_registers AS register
             WHERE register.tenant_id = $1 AND register.deleted_at IS NULL
               AND ($2::DATE IS NULL OR register.attendance_date >= $2)
               AND ($3::DATE IS NULL OR register.attendance_date <= $3)
               AND ($4::UUID IS NULL OR register.class_group_id = $4)
               AND ($5::TEXT IS NULL OR register.period = $5)
               AND ($6::TEXT IS NULL OR register.status = $6)
            "#,
        )
        .bind(tenant_id)
        .bind(query.date_from)
        .bind(query.date_to)
        .bind(query.class_group_id)
        .bind(query.period.map(|period| period.as_str()))
        .bind(query.status.map(|status| status.as_str()))
        .fetch_one(pool)
        .await
        .context("Failed to count attendance registers")?;
        Ok((hydrate_summaries(pool, tenant_id, rows).await?, total))
    }

    pub async fn get(
        pool: &PgPool,
        tenant_id: Uuid,
        register_id: Uuid,
    ) -> Result<Option<AttendanceRegisterResponse>> {
        let Some(register) = register_by_id(pool, tenant_id, register_id).await? else {
            return Ok(None);
        };
        let summary_row = summary_row_by_id(pool, tenant_id, register_id)
            .await?
            .context("Attendance register summary is unavailable")?;
        let summary = hydrate_summary(pool, tenant_id, summary_row).await?;
        let rows = sqlx::query_as::<_, AttendanceMarkRow>(
            r#"
            SELECT id, enrolment_id, learner_id, mark, minutes_late, note,
                   version, marked_at
              FROM attendance_marks
             WHERE tenant_id = $1 AND register_id = $2 AND deleted_at IS NULL
             ORDER BY created_at, id
            "#,
        )
        .bind(tenant_id)
        .bind(register_id)
        .fetch_all(pool)
        .await
        .context("Failed to load attendance marks")?;
        let enrolment_ids = rows.iter().map(|row| row.enrolment_id).collect::<Vec<_>>();
        let identities =
            EnrolmentOps::attendance_references_by_ids(pool, tenant_id, &enrolment_ids)
                .await?
                .into_iter()
                .map(|entry| (entry.enrolment_id, entry))
                .collect::<HashMap<_, _>>();
        let mut marks = Vec::with_capacity(rows.len());
        for row in rows {
            let identity = identities
                .get(&row.enrolment_id)
                .context("A learner referenced by this register is unavailable")?;
            marks.push(AttendanceMarkResponse {
                id: row.id,
                enrolment_id: row.enrolment_id,
                learner_id: row.learner_id,
                learner_number: identity.learner_number.clone(),
                learner_name: identity.display_name.clone(),
                mark: row.mark,
                minutes_late: row.minutes_late,
                note: row.note,
                version: row.version,
                marked_at: row.marked_at,
            });
        }
        marks.sort_by(|left, right| {
            left.learner_name
                .cmp(&right.learner_name)
                .then(left.learner_number.cmp(&right.learner_number))
        });
        Ok(Some(AttendanceRegisterResponse {
            summary,
            marks,
            reopened_at: register.reopened_at,
            reopen_reason: register.reopen_reason,
        }))
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateAttendanceRegisterRequest,
    ) -> Result<AttendanceRegisterResponse> {
        let actor_id = person_actor_id(actor)?;
        let idempotency_key = trimmed_required(&request.idempotency_key, "Idempotency key")?;
        let fingerprint = create_fingerprint(request);
        if let Some((existing_id, existing_fingerprint)) =
            register_by_idempotency(pool, tenant_id, idempotency_key).await?
        {
            if existing_fingerprint != fingerprint {
                bail!("This idempotency key was already used for another attendance register");
            }
            return Self::get(pool, tenant_id, existing_id)
                .await?
                .context("The existing attendance register is unavailable");
        }
        let (term, class) = validate_register_references(pool, tenant_id, request).await?;
        let roster = EnrolmentOps::attendance_roster(
            pool,
            tenant_id,
            term.academic_year_id,
            class.id,
            request.attendance_date,
        )
        .await?;
        if roster.is_empty() {
            bail!("This class has no active learners for the selected date");
        }

        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start attendance register creation")?;
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
        .bind(term.id)
        .bind(class.id)
        .bind(request.attendance_date)
        .bind(request.period.as_str())
        .bind(idempotency_key)
        .bind(&fingerprint)
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "Failed to create attendance register"))?;
        for learner in &roster {
            sqlx::query(
                r#"
                INSERT INTO attendance_marks (
                    tenant_id, register_id, enrolment_id, learner_id
                )
                VALUES ($1, $2, $3, $4)
                "#,
            )
            .bind(tenant_id)
            .bind(register_id)
            .bind(learner.enrolment_id)
            .bind(learner.learner_id)
            .execute(&mut *transaction)
            .await
            .context("Failed to create attendance roster")?;
        }
        append_register_event(
            &mut transaction,
            RegisterEvent {
                tenant_id,
                register_id,
                event_type: "created",
                from_status: None,
                to_status: "draft",
                version: 1,
                actor_id,
                reason: None,
                metadata: json!({ "learner_count": roster.len() }),
            },
        )
        .await?;
        append_register_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "attendance.registers.create",
            register_id,
            json!({
                "attendance_date": request.attendance_date,
                "period": request.period.as_str(),
                "learner_count": roster.len()
            }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit attendance register")?;
        Self::get(pool, tenant_id, register_id)
            .await?
            .context("Created attendance register could not be reloaded")
    }

    pub async fn update_marks(
        pool: &PgPool,
        tenant_id: Uuid,
        register_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateAttendanceMarksRequest,
    ) -> Result<Option<AttendanceRegisterResponse>> {
        let actor_id = person_actor_id(actor)?;
        let parsed = parse_marks(&request.marks)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start attendance update")?;
        let Some(register) = lock_register(&mut transaction, tenant_id, register_id).await? else {
            return Ok(None);
        };
        ensure_draft(&register)?;
        ensure_version(&register, request.expected_version)?;
        let current_marks = sqlx::query_as::<_, AttendanceMarkRow>(
            r#"
            SELECT id, enrolment_id, learner_id, mark, minutes_late, note,
                   version, marked_at
              FROM attendance_marks
             WHERE tenant_id = $1 AND register_id = $2 AND deleted_at IS NULL
             FOR UPDATE
            "#,
        )
        .bind(tenant_id)
        .bind(register_id)
        .fetch_all(&mut *transaction)
        .await
        .context("Failed to lock attendance roster")?;
        let current_ids = current_marks
            .iter()
            .map(|mark| mark.learner_id)
            .collect::<BTreeSet<_>>();
        let submitted_ids = parsed.keys().copied().collect::<BTreeSet<_>>();
        if current_ids != submitted_ids {
            bail!("The submitted roster no longer matches this attendance register");
        }
        for row in current_marks {
            let value = parsed
                .get(&row.learner_id)
                .context("Attendance mark is missing from the parsed roster")?;
            let marked = value.mark != AttendanceMarkStatus::Unmarked;
            sqlx::query(
                r#"
                UPDATE attendance_marks
                   SET mark = $4, minutes_late = $5, note = $6,
                       marked_by = $7, marked_at = $8, version = version + 1
                 WHERE tenant_id = $1 AND register_id = $2 AND id = $3
                   AND deleted_at IS NULL
                "#,
            )
            .bind(tenant_id)
            .bind(register_id)
            .bind(row.id)
            .bind(value.mark.as_str())
            .bind(value.minutes_late)
            .bind(&value.note)
            .bind(marked.then_some(actor_id))
            .bind(marked.then(chrono::Utc::now))
            .execute(&mut *transaction)
            .await
            .context("Failed to update attendance mark")?;
        }
        let version = sqlx::query_scalar::<_, i32>(
            r#"
            UPDATE attendance_registers
               SET version = version + 1
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            RETURNING version
            "#,
        )
        .bind(tenant_id)
        .bind(register_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to version attendance register")?;
        let counts = mark_counts(parsed.values().map(|mark| mark.mark));
        append_register_event(
            &mut transaction,
            RegisterEvent {
                tenant_id,
                register_id,
                event_type: "marks_updated",
                from_status: Some("draft"),
                to_status: "draft",
                version,
                actor_id,
                reason: None,
                metadata: counts.clone(),
            },
        )
        .await?;
        append_register_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "attendance.registers.marks.update",
            register_id,
            counts,
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit attendance marks")?;
        Self::get(pool, tenant_id, register_id).await
    }

    pub async fn submit(
        pool: &PgPool,
        tenant_id: Uuid,
        register_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<AttendanceRegisterResponse>> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start attendance submission")?;
        let Some(register) = lock_register(&mut transaction, tenant_id, register_id).await? else {
            return Ok(None);
        };
        ensure_draft(&register)?;
        ensure_version(&register, expected_version)?;
        let (learner_count, unmarked_count) = sqlx::query_as::<_, (i64, i64)>(
            r#"
            SELECT COUNT(*)::BIGINT,
                   COUNT(*) FILTER (WHERE mark = 'unmarked')::BIGINT
              FROM attendance_marks
             WHERE tenant_id = $1 AND register_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(register_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to validate attendance submission")?;
        if learner_count == 0 {
            bail!("An empty attendance register cannot be submitted");
        }
        if unmarked_count > 0 {
            bail!("Mark every learner before submitting this register");
        }
        let version = sqlx::query_scalar::<_, i32>(
            r#"
            UPDATE attendance_registers
               SET status = 'submitted', submitted_by = $3, submitted_at = NOW(),
                   version = version + 1
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            RETURNING version
            "#,
        )
        .bind(tenant_id)
        .bind(register_id)
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to submit attendance register")?;
        append_register_event(
            &mut transaction,
            RegisterEvent {
                tenant_id,
                register_id,
                event_type: "submitted",
                from_status: Some("draft"),
                to_status: "submitted",
                version,
                actor_id,
                reason: None,
                metadata: json!({ "learner_count": learner_count }),
            },
        )
        .await?;
        append_register_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "attendance.registers.submit",
            register_id,
            json!({ "learner_count": learner_count }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit attendance submission")?;
        Self::get(pool, tenant_id, register_id).await
    }

    pub async fn reopen(
        pool: &PgPool,
        tenant_id: Uuid,
        register_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ReopenAttendanceRegisterRequest,
    ) -> Result<Option<AttendanceRegisterResponse>> {
        let actor_id = person_actor_id(actor)?;
        let reason = trimmed_required(&request.reason, "Reopen reason")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start attendance reopen")?;
        let Some(register) = lock_register(&mut transaction, tenant_id, register_id).await? else {
            return Ok(None);
        };
        if register.status != "submitted" {
            bail!("Only a submitted attendance register can be reopened");
        }
        ensure_version(&register, request.expected_version)?;
        let version = sqlx::query_scalar::<_, i32>(
            r#"
            UPDATE attendance_registers
               SET status = 'draft', submitted_by = NULL, submitted_at = NULL,
                   reopened_by = $3, reopened_at = NOW(), reopen_reason = $4,
                   version = version + 1
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            RETURNING version
            "#,
        )
        .bind(tenant_id)
        .bind(register_id)
        .bind(actor_id)
        .bind(reason)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to reopen attendance register")?;
        append_register_event(
            &mut transaction,
            RegisterEvent {
                tenant_id,
                register_id,
                event_type: "reopened",
                from_status: Some("submitted"),
                to_status: "draft",
                version,
                actor_id,
                reason: Some(reason),
                metadata: json!({}),
            },
        )
        .await?;
        append_register_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "attendance.registers.reopen",
            register_id,
            json!({ "reason_recorded": true }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit attendance reopen")?;
        Self::get(pool, tenant_id, register_id).await
    }

    pub async fn delete(
        pool: &PgPool,
        tenant_id: Uuid,
        register_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<bool> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start attendance deletion")?;
        let Some(register) = lock_register(&mut transaction, tenant_id, register_id).await? else {
            return Ok(false);
        };
        ensure_draft(&register)?;
        ensure_version(&register, expected_version)?;
        append_register_event(
            &mut transaction,
            RegisterEvent {
                tenant_id,
                register_id,
                event_type: "deleted",
                from_status: Some("draft"),
                to_status: "deleted",
                version: register.version + 1,
                actor_id,
                reason: None,
                metadata: json!({}),
            },
        )
        .await?;
        sqlx::query(
            r#"
            UPDATE attendance_marks
               SET deleted_at = NOW(), version = version + 1
             WHERE tenant_id = $1 AND register_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(register_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to remove attendance roster")?;
        sqlx::query(
            r#"
            UPDATE attendance_registers
               SET deleted_at = NOW(), version = version + 1
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(register_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to remove attendance register")?;
        append_register_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "attendance.registers.delete",
            register_id,
            json!({ "status": "deleted" }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit attendance deletion")?;
        Ok(true)
    }
}

async fn validate_register_references(
    pool: &PgPool,
    tenant_id: Uuid,
    request: &CreateAttendanceRegisterRequest,
) -> Result<(
    cp_academics::models::AcademicTerm,
    cp_academics::models::ClassGroupWithYear,
)> {
    let term = AcademicTermOps::get_by_id(pool, tenant_id, request.academic_term_id)
        .await?
        .context("The selected academic term was not found")?;
    if term.status != "active" {
        bail!("Attendance registers require the active academic term");
    }
    if request.attendance_date < term.starts_on || request.attendance_date > term.ends_on {
        bail!("The attendance date must fall inside the active academic term");
    }
    let class = ClassGroupOps::get_by_id(pool, tenant_id, request.class_group_id)
        .await?
        .context("The selected class was not found")?;
    if class.status != "active" {
        bail!("The selected class is not active");
    }
    if class.academic_year_id != term.academic_year_id {
        bail!("The selected class does not belong to the active academic year");
    }
    Ok((term, class))
}

async fn hydrate_summaries(
    pool: &PgPool,
    tenant_id: Uuid,
    rows: Vec<AttendanceRegisterSummaryRow>,
) -> Result<Vec<AttendanceRegisterSummary>> {
    let mut summaries = Vec::with_capacity(rows.len());
    for row in rows {
        summaries.push(hydrate_summary(pool, tenant_id, row).await?);
    }
    Ok(summaries)
}

async fn hydrate_summary(
    pool: &PgPool,
    tenant_id: Uuid,
    row: AttendanceRegisterSummaryRow,
) -> Result<AttendanceRegisterSummary> {
    let term = AcademicTermOps::get_by_id(pool, tenant_id, row.academic_term_id)
        .await?
        .context("The attendance register academic term is unavailable")?;
    let class = ClassGroupOps::get_by_id(pool, tenant_id, row.class_group_id)
        .await?
        .context("The attendance register class is unavailable")?;
    Ok(AttendanceRegisterSummary {
        id: row.id,
        academic_term_id: row.academic_term_id,
        academic_term_name: term.name,
        class_group_id: row.class_group_id,
        class_group_name: class.name,
        attendance_date: row.attendance_date,
        period: row.period,
        status: row.status,
        version: row.version,
        learner_count: row.learner_count,
        present_count: row.present_count,
        absent_count: row.absent_count,
        late_count: row.late_count,
        excused_count: row.excused_count,
        unmarked_count: row.unmarked_count,
        created_at: row.created_at,
        submitted_at: row.submitted_at,
    })
}

async fn register_by_id(
    pool: &PgPool,
    tenant_id: Uuid,
    register_id: Uuid,
) -> Result<Option<AttendanceRegisterRow>> {
    sqlx::query_as::<_, AttendanceRegisterRow>(
        r#"
        SELECT status, version, reopened_at, reopen_reason
          FROM attendance_registers
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(register_id)
    .fetch_optional(pool)
    .await
    .context("Failed to load attendance register")
}

async fn summary_row_by_id(
    pool: &PgPool,
    tenant_id: Uuid,
    register_id: Uuid,
) -> Result<Option<AttendanceRegisterSummaryRow>> {
    sqlx::query_as::<_, AttendanceRegisterSummaryRow>(
        r#"
        SELECT register.id, register.academic_term_id, register.class_group_id,
               register.attendance_date, register.period, register.status,
               register.version, register.created_at, register.submitted_at,
               COUNT(mark.id)::BIGINT AS learner_count,
               COUNT(mark.id) FILTER (WHERE mark.mark = 'present')::BIGINT AS present_count,
               COUNT(mark.id) FILTER (WHERE mark.mark = 'absent')::BIGINT AS absent_count,
               COUNT(mark.id) FILTER (WHERE mark.mark = 'late')::BIGINT AS late_count,
               COUNT(mark.id) FILTER (WHERE mark.mark = 'excused')::BIGINT AS excused_count,
               COUNT(mark.id) FILTER (WHERE mark.mark = 'unmarked')::BIGINT AS unmarked_count
          FROM attendance_registers AS register
          LEFT JOIN attendance_marks AS mark
            ON mark.tenant_id = register.tenant_id
           AND mark.register_id = register.id
           AND mark.deleted_at IS NULL
         WHERE register.tenant_id = $1 AND register.id = $2
           AND register.deleted_at IS NULL
         GROUP BY register.id
        "#,
    )
    .bind(tenant_id)
    .bind(register_id)
    .fetch_optional(pool)
    .await
    .context("Failed to load attendance summary")
}

async fn lock_register(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    register_id: Uuid,
) -> Result<Option<AttendanceRegisterRow>> {
    sqlx::query_as::<_, AttendanceRegisterRow>(
        r#"
        SELECT status, version, reopened_at, reopen_reason
          FROM attendance_registers
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(register_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock attendance register")
}

async fn register_by_idempotency(
    pool: &PgPool,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> Result<Option<(Uuid, String)>> {
    sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT id, create_request_fingerprint
          FROM attendance_registers
         WHERE tenant_id = $1 AND idempotency_key = $2
        "#,
    )
    .bind(tenant_id)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await
    .context("Failed to check attendance idempotency")
}

fn ensure_draft(register: &AttendanceRegisterRow) -> Result<()> {
    if register.status != "draft" {
        bail!("Submitted attendance registers are locked");
    }
    Ok(())
}

fn ensure_version(register: &AttendanceRegisterRow, expected_version: i32) -> Result<()> {
    if register.version != expected_version {
        bail!("This attendance register changed. Reload it before continuing");
    }
    Ok(())
}

#[derive(Debug)]
struct ParsedMark {
    mark: AttendanceMarkStatus,
    minutes_late: Option<i32>,
    note: Option<String>,
}

fn parse_marks(values: &[AttendanceMarkInput]) -> Result<BTreeMap<Uuid, ParsedMark>> {
    let mut parsed = BTreeMap::new();
    for value in values {
        if value
            .minutes_late
            .is_some_and(|minutes| !(0..=1440).contains(&minutes))
        {
            bail!("Minutes late must be between 0 and 1440");
        }
        if value.mark != AttendanceMarkStatus::Late && value.minutes_late.is_some() {
            bail!("Minutes late can be recorded only for a late learner");
        }
        let note = trimmed_optional(value.note.as_deref());
        if value.mark == AttendanceMarkStatus::Unmarked && note.is_some() {
            bail!("An unmarked learner cannot have an attendance note");
        }
        if parsed
            .insert(
                value.learner_id,
                ParsedMark {
                    mark: value.mark,
                    minutes_late: value.minutes_late,
                    note,
                },
            )
            .is_some()
        {
            bail!("Each learner may appear only once in an attendance update");
        }
    }
    if parsed.is_empty() {
        bail!("Attendance marks are required");
    }
    Ok(parsed)
}

fn mark_counts(values: impl Iterator<Item = AttendanceMarkStatus>) -> Value {
    let mut counts = BTreeMap::from([
        ("unmarked", 0_u64),
        ("present", 0),
        ("absent", 0),
        ("late", 0),
        ("excused", 0),
    ]);
    for value in values {
        if let Some(count) = counts.get_mut(value.as_str()) {
            *count += 1;
        }
    }
    json!(counts)
}

struct RegisterEvent<'a> {
    tenant_id: Uuid,
    register_id: Uuid,
    event_type: &'a str,
    from_status: Option<&'a str>,
    to_status: &'a str,
    version: i32,
    actor_id: Uuid,
    reason: Option<&'a str>,
    metadata: Value,
}

async fn append_register_event(
    transaction: &mut Transaction<'_, Postgres>,
    event: RegisterEvent<'_>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO attendance_register_events (
            tenant_id, register_id, event_type, from_status, to_status,
            register_version, actor_id, reason, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(event.tenant_id)
    .bind(event.register_id)
    .bind(event.event_type)
    .bind(event.from_status)
    .bind(event.to_status)
    .bind(event.version)
    .bind(event.actor_id)
    .bind(event.reason)
    .bind(event.metadata)
    .execute(&mut **transaction)
    .await
    .context("Failed to append attendance register history")?;
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "audit evidence is intentionally explicit"
)]
async fn append_register_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    action: &str,
    register_id: Uuid,
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
            "attendance_register",
            register_id.to_string(),
        ))
        .with_redacted_metadata(metadata),
    )
    .await
    .context("Failed to append attendance audit event")?;
    Ok(())
}

fn create_fingerprint(request: &CreateAttendanceRegisterRequest) -> String {
    let canonical = format!(
        "{}|{}|{}|{}",
        request.academic_term_id,
        request.class_group_id,
        request.attendance_date,
        request.period.as_str()
    );
    format!("{:x}", Sha256::digest(canonical.as_bytes()))
}

fn database_error(error: sqlx::Error, context: &str) -> anyhow::Error {
    if let sqlx::Error::Database(database) = &error {
        if database.constraint() == Some("idx_attendance_registers_scope") {
            return anyhow!(
                "An attendance register already exists for this class, date, and period"
            );
        }
        if database.constraint() == Some("idx_attendance_registers_idempotency") {
            return anyhow!("This attendance request has already been processed");
        }
    }
    anyhow!("{context}: {error}")
}

fn person_actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("Authenticated person actor is required"))
}

fn trimmed_required<'a>(value: &'a str, field: &str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{field} is required");
    }
    Ok(value)
}

fn trimmed_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE),
        per_page.unwrap_or(DEFAULT_PER_PAGE).clamp(1, MAX_PER_PAGE),
    )
}

#[cfg(test)]
mod tests {
    use super::{AttendanceMarkInput, AttendanceMarkStatus, mark_counts, parse_marks};
    use uuid::Uuid;

    #[test]
    fn mark_parser_rejects_duplicate_learners() {
        let learner_id = Uuid::new_v4();
        let values = vec![
            AttendanceMarkInput {
                learner_id,
                mark: AttendanceMarkStatus::Present,
                minutes_late: None,
                note: None,
            },
            AttendanceMarkInput {
                learner_id,
                mark: AttendanceMarkStatus::Absent,
                minutes_late: None,
                note: None,
            },
        ];
        assert!(parse_marks(&values).is_err());
    }

    #[test]
    fn mark_parser_rejects_lateness_on_non_late_marks() {
        let values = vec![AttendanceMarkInput {
            learner_id: Uuid::new_v4(),
            mark: AttendanceMarkStatus::Present,
            minutes_late: Some(5),
            note: None,
        }];
        assert!(parse_marks(&values).is_err());
    }

    #[test]
    fn mark_counts_cover_every_closed_status() {
        let counts = mark_counts(
            [
                AttendanceMarkStatus::Present,
                AttendanceMarkStatus::Present,
                AttendanceMarkStatus::Absent,
                AttendanceMarkStatus::Late,
                AttendanceMarkStatus::Excused,
                AttendanceMarkStatus::Unmarked,
            ]
            .into_iter(),
        );
        assert_eq!(counts["present"], 2);
        assert_eq!(counts["absent"], 1);
        assert_eq!(counts["late"], 1);
        assert_eq!(counts["excused"], 1);
        assert_eq!(counts["unmarked"], 1);
    }
}
