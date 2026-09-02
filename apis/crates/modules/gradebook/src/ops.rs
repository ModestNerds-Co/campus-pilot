//! Transactional Gradebook operations.
//!
//! Writes use optimistic versions, immutable lifecycle evidence, and actor-aware
//! audit in the same transaction. Published results require a reasoned reopen
//! before any learner mark can change.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::{Context, Result, anyhow, bail};
use cp_academics::assessments::{
    AssessmentComponentOps, AssessmentCycleOps, GradebookAssessmentReference,
};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_sis::ops::EnrolmentOps;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::dtos::{
    CreateMarkSheetRequest, GradebookComponentReference, GradebookMarkInput, GradebookMarkResponse,
    GradebookMarkStatus, GradebookReferenceData, GradebookReportingSource, GradebookSheetListQuery,
    GradebookSheetResponse, GradebookSheetSummary, PaginatedGradebookSheetsResponse,
    PublishedAssessmentMark, ReopenMarkSheetRequest, UpdateGradebookMarksRequest,
};
use crate::models::{MarkRow, MarkSheetRow, MarkSheetSummaryRow};

const DEFAULT_PAGE: i64 = 1;
const DEFAULT_PER_PAGE: i64 = 25;
const MAX_PAGE: i64 = 1_000_000;
const MAX_PER_PAGE: i64 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradebookAccessScope {
    Campus,
    AssignedTo(Uuid),
}

/// One exact, version-bound mark prepared by another owning module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GradebookScoreTransferMark {
    pub mark_id: Uuid,
    pub learner_id: Uuid,
    pub expected_mark_version: i32,
    pub marks_awarded_hundredths: i64,
}

/// Typed cross-module command for applying reviewed Learning evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyGradebookScoreTransfer {
    pub proposal_id: Uuid,
    pub mark_sheet_id: Uuid,
    pub expected_sheet_version: i32,
    pub source_type: String,
    pub marks: Vec<GradebookScoreTransferMark>,
}

pub struct GradebookOps;

impl GradebookOps {
    /// Applies a reviewed Learning proposal to unmarked rows in one draft sheet.
    ///
    /// The caller owns the proposal transaction. Gradebook locks and verifies
    /// every destination row, writes the formal marks, and appends its own
    /// lifecycle and audit evidence in that same transaction.
    pub async fn apply_learning_score_transfer(
        transaction: &mut Transaction<'_, Postgres>,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ApplyGradebookScoreTransfer,
    ) -> Result<i32> {
        let actor_id = person_actor_id(actor)?;
        if request.marks.is_empty() {
            bail!("A score transfer needs at least one ready learner mark");
        }
        let Some(sheet) = lock_sheet(transaction, tenant_id, request.mark_sheet_id).await? else {
            bail!("The target mark sheet is unavailable");
        };
        ensure_draft(&sheet)?;
        ensure_version(&sheet, request.expected_sheet_version)?;
        let maximum_hundredths = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT component.maximum_marks::BIGINT * 100
              FROM assessment_components component
             WHERE component.tenant_id = $1 AND component.id = $2
               AND component.deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(sheet.assessment_component_id)
        .fetch_optional(&mut **transaction)
        .await
        .context("Failed to load the score-transfer mark limit")?
        .context("The score-transfer assessment component is unavailable")?;

        let requested_ids = request
            .marks
            .iter()
            .map(|mark| mark.mark_id)
            .collect::<BTreeSet<_>>();
        if requested_ids.len() != request.marks.len() {
            bail!("The score transfer contains a duplicate target mark");
        }
        let current = sqlx::query_as::<_, MarkRow>(
            r#"
            SELECT id, enrolment_id, learner_id, mark_status,
                   marks_awarded_hundredths, note, version, marked_at
              FROM assessment_marks
             WHERE tenant_id = $1 AND mark_sheet_id = $2
               AND id = ANY($3) AND deleted_at IS NULL
             FOR UPDATE
            "#,
        )
        .bind(tenant_id)
        .bind(request.mark_sheet_id)
        .bind(requested_ids.iter().copied().collect::<Vec<_>>())
        .fetch_all(&mut **transaction)
        .await
        .context("Failed to lock score-transfer target marks")?;
        if current.len() != request.marks.len() {
            bail!("The score-transfer target roster changed before review");
        }
        let current_by_id = current
            .into_iter()
            .map(|mark| (mark.id, mark))
            .collect::<HashMap<_, _>>();
        for proposed in &request.marks {
            ensure_transfer_mark_range(proposed.marks_awarded_hundredths, maximum_hundredths)?;
            let target = current_by_id
                .get(&proposed.mark_id)
                .context("A score-transfer target mark is unavailable")?;
            if target.learner_id != proposed.learner_id
                || target.version != proposed.expected_mark_version
                || target.mark_status != "unmarked"
            {
                bail!("The score-transfer target changed before review");
            }
            let updated = sqlx::query(
                r#"
                UPDATE assessment_marks
                   SET mark_status = 'scored', marks_awarded_hundredths = $4,
                       note = $5, marked_by = $6, marked_at = NOW(),
                       version = version + 1
                 WHERE tenant_id = $1 AND mark_sheet_id = $2 AND id = $3
                   AND version = $7 AND mark_status = 'unmarked'
                   AND deleted_at IS NULL
                "#,
            )
            .bind(tenant_id)
            .bind(request.mark_sheet_id)
            .bind(proposed.mark_id)
            .bind(proposed.marks_awarded_hundredths)
            .bind(format!(
                "Transferred from E-learning {} proposal {}",
                request.source_type, request.proposal_id
            ))
            .bind(actor_id)
            .bind(proposed.expected_mark_version)
            .execute(&mut **transaction)
            .await
            .context("Failed to apply a reviewed Learning score")?;
            if updated.rows_affected() != 1 {
                bail!("The score-transfer target changed before review");
            }
        }

        let version =
            increment_sheet_version(transaction, tenant_id, request.mark_sheet_id).await?;
        let metadata = json!({
            "learning_score_transfer_proposal_id": request.proposal_id,
            "source_type": request.source_type,
            "transferred_count": request.marks.len()
        });
        append_sheet_event(
            transaction,
            SheetEvent {
                tenant_id,
                mark_sheet_id: request.mark_sheet_id,
                event_type: "marks_transferred",
                from_status: Some("draft"),
                to_status: "draft",
                version,
                actor_id,
                reason: None,
                metadata: metadata.clone(),
            },
        )
        .await?;
        append_sheet_audit(
            transaction,
            tenant_id,
            actor,
            request_context,
            "academics.gradebook.mark_sheets.learning_transfer.apply",
            request.mark_sheet_id,
            metadata,
        )
        .await?;
        Ok(version)
    }

    /// Lists closed assessment-cycle classes whose active components all have
    /// published sheets. A reporting batch may only use these exact sources.
    pub async fn reporting_sources(
        pool: &PgPool,
        tenant_id: Uuid,
    ) -> Result<Vec<GradebookReportingSource>> {
        sqlx::query_as::<_, GradebookReportingSource>(
            r#"
            SELECT cycle.id AS assessment_cycle_id,
                   cycle.name AS assessment_cycle_name,
                   term.id AS academic_term_id,
                   term.name AS academic_term_name,
                   term.starts_on AS academic_term_starts_on,
                   term.ends_on AS academic_term_ends_on,
                   academic_year.id AS academic_year_id,
                   academic_year.name AS academic_year_name,
                   class_group.id AS class_group_id,
                   class_group.name AS class_group_name,
                   COUNT(DISTINCT component.id) AS component_count,
                   COUNT(DISTINCT sheet.id) AS published_sheet_count,
                   COALESCE(
                       ARRAY_AGG(DISTINCT employee.account_id)
                           FILTER (WHERE employee.account_id IS NOT NULL),
                       ARRAY[]::UUID[]
                   ) AS teacher_account_ids
              FROM assessment_cycles AS cycle
              JOIN academic_terms AS term
                ON term.id = cycle.academic_term_id
               AND term.tenant_id = cycle.tenant_id
               AND term.deleted_at IS NULL
              JOIN academic_years AS academic_year
                ON academic_year.id = term.academic_year_id
               AND academic_year.tenant_id = term.tenant_id
               AND academic_year.deleted_at IS NULL
              JOIN assessment_components AS component
                ON component.assessment_cycle_id = cycle.id
               AND component.tenant_id = cycle.tenant_id
               AND component.status = 'active'
               AND component.deleted_at IS NULL
              JOIN teaching_assignments AS assignment
                ON assignment.id = component.teaching_assignment_id
               AND assignment.tenant_id = component.tenant_id
               AND assignment.deleted_at IS NULL
              JOIN class_groups AS class_group
                ON class_group.id = assignment.class_group_id
               AND class_group.tenant_id = assignment.tenant_id
               AND class_group.deleted_at IS NULL
              JOIN teacher_profiles AS teacher
                ON teacher.id = assignment.teacher_profile_id
               AND teacher.tenant_id = assignment.tenant_id
               AND teacher.deleted_at IS NULL
              JOIN employees AS employee
                ON employee.id = teacher.employee_id
               AND employee.tenant_id = teacher.tenant_id
               AND employee.deleted_at IS NULL
              LEFT JOIN assessment_mark_sheets AS sheet
                ON sheet.assessment_component_id = component.id
               AND sheet.tenant_id = component.tenant_id
               AND sheet.status = 'published'
               AND sheet.deleted_at IS NULL
             WHERE cycle.tenant_id = $1
               AND cycle.status = 'closed'
               AND cycle.deleted_at IS NULL
             GROUP BY cycle.id, cycle.name, term.id, term.name,
                      term.starts_on, term.ends_on, academic_year.id,
                      academic_year.name, class_group.id, class_group.name
            HAVING COUNT(DISTINCT component.id) = COUNT(DISTINCT sheet.id)
             ORDER BY term.ends_on DESC, cycle.name, class_group.name
            "#,
        )
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .context("Failed to load Gradebook reporting sources")
    }

    /// Loads exact published marks for one closed-cycle class.
    pub async fn published_results_for_cycle_class(
        pool: &PgPool,
        tenant_id: Uuid,
        assessment_cycle_id: Uuid,
        class_group_id: Uuid,
    ) -> Result<Vec<PublishedAssessmentMark>> {
        sqlx::query_as::<_, PublishedAssessmentMark>(
            r#"
            SELECT sheet.id AS mark_sheet_id,
                   sheet.version AS mark_sheet_version,
                   component.id AS assessment_component_id,
                   assignment.id AS teaching_assignment_id,
                   assignment.subject_id,
                   subject.name AS subject_name,
                   assignment.class_group_id,
                   mark.enrolment_id,
                   mark.learner_id,
                   mark.mark_status,
                   mark.marks_awarded_hundredths,
                   component.maximum_marks,
                   component.weight_basis_points
              FROM assessment_cycles AS cycle
              JOIN assessment_components AS component
                ON component.assessment_cycle_id = cycle.id
               AND component.tenant_id = cycle.tenant_id
               AND component.status = 'active'
               AND component.deleted_at IS NULL
              JOIN teaching_assignments AS assignment
                ON assignment.id = component.teaching_assignment_id
               AND assignment.tenant_id = component.tenant_id
               AND assignment.deleted_at IS NULL
              JOIN subjects AS subject
                ON subject.id = assignment.subject_id
               AND subject.tenant_id = assignment.tenant_id
               AND subject.deleted_at IS NULL
              JOIN assessment_mark_sheets AS sheet
                ON sheet.assessment_component_id = component.id
               AND sheet.tenant_id = component.tenant_id
               AND sheet.status = 'published'
               AND sheet.deleted_at IS NULL
              JOIN assessment_marks AS mark
                ON mark.mark_sheet_id = sheet.id
               AND mark.tenant_id = sheet.tenant_id
               AND mark.deleted_at IS NULL
             WHERE cycle.tenant_id = $1
               AND cycle.id = $2
               AND cycle.status = 'closed'
               AND cycle.deleted_at IS NULL
               AND assignment.class_group_id = $3
             ORDER BY mark.learner_id, assignment.subject_id, component.id
            "#,
        )
        .bind(tenant_id)
        .bind(assessment_cycle_id)
        .bind(class_group_id)
        .fetch_all(pool)
        .await
        .context("Failed to load published Gradebook results")
    }

    /// Returns open and closed assessment components with current mark-sheet state.
    pub async fn reference_data(
        pool: &PgPool,
        tenant_id: Uuid,
        access_scope: GradebookAccessScope,
    ) -> Result<GradebookReferenceData> {
        let (cycles, _) =
            AssessmentCycleOps::list(pool, tenant_id, 1, 100, None, None, None).await?;
        let mut references = Vec::new();
        for cycle in cycles
            .into_iter()
            .filter(|cycle| matches!(cycle.status.as_str(), "open" | "closed"))
        {
            let (components, _) = AssessmentComponentOps::list(
                pool,
                tenant_id,
                cycle.id,
                1,
                100,
                Some("active"),
                None,
            )
            .await?;
            for component in components {
                let reference =
                    AssessmentComponentOps::gradebook_reference(pool, tenant_id, component.id)
                        .await?
                        .context("The assessment component is unavailable")?;
                if reference_is_visible(&reference, access_scope) {
                    references.push(reference);
                }
            }
        }
        let component_ids = references
            .iter()
            .map(|reference| reference.assessment_component_id)
            .collect::<Vec<_>>();
        let sheet_states = sheet_states_by_component(pool, tenant_id, &component_ids).await?;
        let mut components = references
            .into_iter()
            .map(|reference| {
                let state = sheet_states.get(&reference.assessment_component_id);
                component_reference(
                    reference,
                    state.map(|value| value.0),
                    state.map(|value| value.1.clone()),
                    state.map(|value| value.2),
                )
            })
            .collect::<Vec<_>>();
        components.sort_by(|left, right| {
            left.academic_term_name
                .cmp(&right.academic_term_name)
                .then(left.class_group_name.cmp(&right.class_group_name))
                .then(left.subject_name.cmp(&right.subject_name))
                .then(
                    left.assessment_component_name
                        .cmp(&right.assessment_component_name),
                )
        });
        Ok(GradebookReferenceData { components })
    }

    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &GradebookSheetListQuery,
        access_scope: GradebookAccessScope,
    ) -> Result<(PaginatedGradebookSheetsResponse, i64)> {
        let (page, per_page) = bounded_page(query.page, query.per_page);
        let offset = (page - 1) * per_page;
        let status = query.status.map(|value| value.as_str());
        let (rows, total) = match access_scope {
            GradebookAccessScope::Campus => {
                let rows = sqlx::query_as::<_, MarkSheetSummaryRow>(&summary_select(
                    "sheet.tenant_id = $1 AND sheet.deleted_at IS NULL AND ($2::TEXT IS NULL OR sheet.status = $2)",
                    "ORDER BY sheet.roster_on DESC, sheet.created_at DESC, sheet.id LIMIT $3 OFFSET $4",
                ))
                .bind(tenant_id)
                .bind(status)
                .bind(per_page)
                .bind(offset)
                .fetch_all(pool)
                .await
                .context("Failed to list assessment mark sheets")?;
                let total = count_sheets(pool, tenant_id, status, None).await?;
                (rows, total)
            }
            GradebookAccessScope::AssignedTo(user_id) => {
                let rows = sqlx::query_as::<_, MarkSheetSummaryRow>(&summary_select(
                    "sheet.tenant_id = $1 AND sheet.deleted_at IS NULL AND ($2::TEXT IS NULL OR sheet.status = $2) AND employee.account_id = $3",
                    "ORDER BY sheet.roster_on DESC, sheet.created_at DESC, sheet.id LIMIT $4 OFFSET $5",
                ))
                .bind(tenant_id)
                .bind(status)
                .bind(user_id)
                .bind(per_page)
                .bind(offset)
                .fetch_all(pool)
                .await
                .context("Failed to list assigned assessment mark sheets")?;
                let total = count_sheets(pool, tenant_id, status, Some(user_id)).await?;
                (rows, total)
            }
        };
        let mut mark_sheets = Vec::with_capacity(rows.len());
        for row in rows {
            mark_sheets.push(hydrate_summary(pool, tenant_id, row).await?);
        }
        Ok((PaginatedGradebookSheetsResponse { mark_sheets }, total))
    }

    pub async fn get(
        pool: &PgPool,
        tenant_id: Uuid,
        mark_sheet_id: Uuid,
        access_scope: GradebookAccessScope,
    ) -> Result<Option<GradebookSheetResponse>> {
        let Some(sheet) = sheet_by_id(pool, tenant_id, mark_sheet_id).await? else {
            return Ok(None);
        };
        if !Self::can_access_component(pool, tenant_id, sheet.assessment_component_id, access_scope)
            .await?
        {
            return Ok(None);
        }
        let summary_row = summary_row_by_id(pool, tenant_id, mark_sheet_id)
            .await?
            .context("The assessment mark sheet summary is unavailable")?;
        let summary = hydrate_summary(pool, tenant_id, summary_row).await?;
        let rows = sqlx::query_as::<_, MarkRow>(
            r#"
            SELECT id, enrolment_id, learner_id, mark_status,
                   marks_awarded_hundredths, note, version, marked_at
              FROM assessment_marks
             WHERE tenant_id = $1 AND mark_sheet_id = $2 AND deleted_at IS NULL
             ORDER BY created_at, id
            "#,
        )
        .bind(tenant_id)
        .bind(mark_sheet_id)
        .fetch_all(pool)
        .await
        .context("Failed to load assessment marks")?;
        let enrolment_ids = rows.iter().map(|row| row.enrolment_id).collect::<Vec<_>>();
        let identities =
            EnrolmentOps::roster_references_by_enrolment_ids(pool, tenant_id, &enrolment_ids)
                .await?
                .into_iter()
                .map(|entry| (entry.enrolment_id, entry))
                .collect::<HashMap<_, _>>();
        let mut marks = Vec::with_capacity(rows.len());
        for row in rows {
            let identity = identities
                .get(&row.enrolment_id)
                .context("A learner referenced by this mark sheet is unavailable")?;
            let percentage = row
                .marks_awarded_hundredths
                .map(|marks| percentage_basis_points(marks, summary.maximum_marks));
            let weighted = row.marks_awarded_hundredths.map(|marks| {
                weighted_score_basis_points(
                    marks,
                    summary.maximum_marks,
                    summary.weight_basis_points,
                )
            });
            marks.push(GradebookMarkResponse {
                id: row.id,
                enrolment_id: row.enrolment_id,
                learner_id: row.learner_id,
                learner_number: identity.learner_number.clone(),
                learner_name: identity.display_name.clone(),
                mark_status: row.mark_status,
                marks_awarded_hundredths: row.marks_awarded_hundredths,
                percentage_basis_points: percentage,
                weighted_score_basis_points: weighted,
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
        Ok(Some(GradebookSheetResponse {
            summary,
            marks,
            reopened_at: sheet.reopened_at,
            reopen_reason: sheet.reopen_reason,
        }))
    }

    pub async fn can_access_component(
        pool: &PgPool,
        tenant_id: Uuid,
        component_id: Uuid,
        access_scope: GradebookAccessScope,
    ) -> Result<bool> {
        Ok(
            AssessmentComponentOps::gradebook_reference(pool, tenant_id, component_id)
                .await?
                .is_some_and(|reference| reference_is_visible(&reference, access_scope)),
        )
    }

    pub async fn can_access_mark_sheet(
        pool: &PgPool,
        tenant_id: Uuid,
        mark_sheet_id: Uuid,
        access_scope: GradebookAccessScope,
    ) -> Result<bool> {
        let Some(sheet) = sheet_by_id(pool, tenant_id, mark_sheet_id).await? else {
            return Ok(false);
        };
        Self::can_access_component(pool, tenant_id, sheet.assessment_component_id, access_scope)
            .await
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateMarkSheetRequest,
    ) -> Result<GradebookSheetResponse> {
        let actor_id = person_actor_id(actor)?;
        let idempotency_key = trimmed_required(&request.idempotency_key, "Idempotency key")?;
        let fingerprint = create_fingerprint(request);
        if let Some((existing_id, existing_fingerprint)) =
            sheet_by_idempotency(pool, tenant_id, idempotency_key).await?
        {
            if existing_fingerprint != fingerprint {
                bail!("This idempotency key was already used for another mark sheet");
            }
            return Self::get(pool, tenant_id, existing_id, GradebookAccessScope::Campus)
                .await?
                .context("The existing assessment mark sheet is unavailable");
        }
        let reference = AssessmentComponentOps::gradebook_reference(
            pool,
            tenant_id,
            request.assessment_component_id,
        )
        .await?
        .context("The selected assessment component was not found")?;
        validate_new_sheet_reference(&reference, request.roster_on)?;
        let roster = EnrolmentOps::class_roster_on(
            pool,
            tenant_id,
            reference.academic_year_id,
            reference.class_group_id,
            request.roster_on,
        )
        .await?;
        if roster.is_empty() {
            bail!("This class has no active learners for the selected roster date");
        }

        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start mark-sheet creation")?;
        let mark_sheet_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO assessment_mark_sheets (
                tenant_id, assessment_component_id, roster_on, idempotency_key,
                create_request_fingerprint, created_by
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(request.assessment_component_id)
        .bind(request.roster_on)
        .bind(idempotency_key)
        .bind(&fingerprint)
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "Failed to create assessment mark sheet"))?;
        for learner in &roster {
            sqlx::query(
                r#"
                INSERT INTO assessment_marks (
                    tenant_id, mark_sheet_id, enrolment_id, learner_id
                )
                VALUES ($1, $2, $3, $4)
                "#,
            )
            .bind(tenant_id)
            .bind(mark_sheet_id)
            .bind(learner.enrolment_id)
            .bind(learner.learner_id)
            .execute(&mut *transaction)
            .await
            .context("Failed to create the assessment roster")?;
        }
        append_sheet_event(
            &mut transaction,
            SheetEvent {
                tenant_id,
                mark_sheet_id,
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
        append_sheet_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "academics.gradebook.mark_sheets.create",
            mark_sheet_id,
            json!({
                "assessment_component_id": request.assessment_component_id,
                "roster_on": request.roster_on,
                "learner_count": roster.len()
            }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit assessment mark sheet")?;
        Self::get(pool, tenant_id, mark_sheet_id, GradebookAccessScope::Campus)
            .await?
            .context("Created assessment mark sheet could not be reloaded")
    }

    pub async fn update_marks(
        pool: &PgPool,
        tenant_id: Uuid,
        mark_sheet_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateGradebookMarksRequest,
    ) -> Result<Option<GradebookSheetResponse>> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start assessment mark update")?;
        let Some(sheet) = lock_sheet(&mut transaction, tenant_id, mark_sheet_id).await? else {
            return Ok(None);
        };
        ensure_draft(&sheet)?;
        ensure_version(&sheet, request.expected_version)?;
        let reference = AssessmentComponentOps::gradebook_reference(
            pool,
            tenant_id,
            sheet.assessment_component_id,
        )
        .await?
        .context("The assessment component is unavailable")?;
        let parsed = parse_marks(&request.marks, reference.maximum_marks)?;
        let current_marks = sqlx::query_as::<_, MarkRow>(
            r#"
            SELECT id, enrolment_id, learner_id, mark_status,
                   marks_awarded_hundredths, note, version, marked_at
              FROM assessment_marks
             WHERE tenant_id = $1 AND mark_sheet_id = $2 AND deleted_at IS NULL
             FOR UPDATE
            "#,
        )
        .bind(tenant_id)
        .bind(mark_sheet_id)
        .fetch_all(&mut *transaction)
        .await
        .context("Failed to lock the assessment roster")?;
        let current_ids = current_marks
            .iter()
            .map(|mark| mark.learner_id)
            .collect::<BTreeSet<_>>();
        let submitted_ids = parsed.keys().copied().collect::<BTreeSet<_>>();
        if current_ids != submitted_ids {
            bail!("The submitted roster no longer matches this assessment mark sheet");
        }
        for row in current_marks {
            let value = parsed
                .get(&row.learner_id)
                .context("A learner mark is missing from the parsed roster")?;
            let marked = value.mark_status != GradebookMarkStatus::Unmarked;
            sqlx::query(
                r#"
                UPDATE assessment_marks
                   SET mark_status = $4, marks_awarded_hundredths = $5,
                       note = $6, marked_by = $7, marked_at = $8,
                       version = version + 1
                 WHERE tenant_id = $1 AND mark_sheet_id = $2 AND id = $3
                   AND deleted_at IS NULL
                "#,
            )
            .bind(tenant_id)
            .bind(mark_sheet_id)
            .bind(row.id)
            .bind(value.mark_status.as_str())
            .bind(value.marks_awarded_hundredths)
            .bind(&value.note)
            .bind(marked.then_some(actor_id))
            .bind(marked.then(chrono::Utc::now))
            .execute(&mut *transaction)
            .await
            .context("Failed to update an assessment mark")?;
        }
        let version = increment_sheet_version(&mut transaction, tenant_id, mark_sheet_id).await?;
        let counts = mark_counts(parsed.values().map(|mark| mark.mark_status));
        append_sheet_event(
            &mut transaction,
            SheetEvent {
                tenant_id,
                mark_sheet_id,
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
        append_sheet_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "academics.gradebook.mark_sheets.marks.update",
            mark_sheet_id,
            counts,
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit assessment marks")?;
        Self::get(pool, tenant_id, mark_sheet_id, GradebookAccessScope::Campus).await
    }

    pub async fn submit(
        pool: &PgPool,
        tenant_id: Uuid,
        mark_sheet_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<GradebookSheetResponse>> {
        transition_sheet(
            pool,
            tenant_id,
            mark_sheet_id,
            actor,
            request_context,
            expected_version,
            "draft",
            "submitted",
            "submitted",
            "academics.gradebook.mark_sheets.submit",
        )
        .await
    }

    pub async fn publish(
        pool: &PgPool,
        tenant_id: Uuid,
        mark_sheet_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<GradebookSheetResponse>> {
        transition_sheet(
            pool,
            tenant_id,
            mark_sheet_id,
            actor,
            request_context,
            expected_version,
            "submitted",
            "published",
            "published",
            "academics.gradebook.mark_sheets.publish",
        )
        .await
    }

    pub async fn reopen(
        pool: &PgPool,
        tenant_id: Uuid,
        mark_sheet_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ReopenMarkSheetRequest,
    ) -> Result<Option<GradebookSheetResponse>> {
        let actor_id = person_actor_id(actor)?;
        let reason = trimmed_required(&request.reason, "Reopen reason")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start mark-sheet reopen")?;
        let Some(sheet) = lock_sheet(&mut transaction, tenant_id, mark_sheet_id).await? else {
            return Ok(None);
        };
        if !matches!(sheet.status.as_str(), "submitted" | "published") {
            bail!("Only a submitted or published mark sheet can be reopened");
        }
        ensure_version(&sheet, request.expected_version)?;
        let from_status = sheet.status.clone();
        let version = sqlx::query_scalar::<_, i32>(
            r#"
            UPDATE assessment_mark_sheets
               SET status = 'draft', submitted_by = NULL, submitted_at = NULL,
                   published_by = NULL, published_at = NULL,
                   reopened_by = $3, reopened_at = NOW(), reopen_reason = $4,
                   version = version + 1
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            RETURNING version
            "#,
        )
        .bind(tenant_id)
        .bind(mark_sheet_id)
        .bind(actor_id)
        .bind(reason)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to reopen assessment mark sheet")?;
        append_sheet_event(
            &mut transaction,
            SheetEvent {
                tenant_id,
                mark_sheet_id,
                event_type: "reopened",
                from_status: Some(&from_status),
                to_status: "draft",
                version,
                actor_id,
                reason: Some(reason),
                metadata: json!({}),
            },
        )
        .await?;
        append_sheet_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "academics.gradebook.mark_sheets.reopen",
            mark_sheet_id,
            json!({ "prior_status": from_status, "reason_recorded": true }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit mark-sheet reopen")?;
        Self::get(pool, tenant_id, mark_sheet_id, GradebookAccessScope::Campus).await
    }

    pub async fn delete(
        pool: &PgPool,
        tenant_id: Uuid,
        mark_sheet_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<bool> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start mark-sheet deletion")?;
        let Some(sheet) = lock_sheet(&mut transaction, tenant_id, mark_sheet_id).await? else {
            return Ok(false);
        };
        ensure_draft(&sheet)?;
        ensure_version(&sheet, expected_version)?;
        append_sheet_event(
            &mut transaction,
            SheetEvent {
                tenant_id,
                mark_sheet_id,
                event_type: "deleted",
                from_status: Some("draft"),
                to_status: "deleted",
                version: sheet.version + 1,
                actor_id,
                reason: None,
                metadata: json!({}),
            },
        )
        .await?;
        sqlx::query(
            r#"
            UPDATE assessment_marks
               SET deleted_at = NOW(), version = version + 1
             WHERE tenant_id = $1 AND mark_sheet_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(mark_sheet_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to remove assessment marks")?;
        sqlx::query(
            r#"
            UPDATE assessment_mark_sheets
               SET deleted_at = NOW(), version = version + 1
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(mark_sheet_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to remove assessment mark sheet")?;
        append_sheet_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "academics.gradebook.mark_sheets.delete",
            mark_sheet_id,
            json!({ "status": "deleted" }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit mark-sheet deletion")?;
        Ok(true)
    }
}

#[allow(clippy::too_many_arguments)]
async fn transition_sheet(
    pool: &PgPool,
    tenant_id: Uuid,
    mark_sheet_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    expected_version: i32,
    expected_status: &str,
    target_status: &str,
    event_type: &str,
    action_key: &str,
) -> Result<Option<GradebookSheetResponse>> {
    let actor_id = person_actor_id(actor)?;
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to start mark-sheet transition")?;
    let Some(sheet) = lock_sheet(&mut transaction, tenant_id, mark_sheet_id).await? else {
        return Ok(None);
    };
    if sheet.status != expected_status {
        bail!("This mark sheet is not ready for the requested transition");
    }
    ensure_version(&sheet, expected_version)?;
    let (learner_count, unmarked_count) =
        mark_completion_counts(&mut transaction, tenant_id, mark_sheet_id).await?;
    if expected_status == "draft" {
        if learner_count == 0 {
            bail!("An empty assessment mark sheet cannot be submitted");
        }
        if unmarked_count > 0 {
            bail!("Mark every learner before submitting this mark sheet");
        }
    }
    let query = if target_status == "submitted" {
        r#"
        UPDATE assessment_mark_sheets
           SET status = 'submitted', submitted_by = $3, submitted_at = NOW(),
               version = version + 1
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        RETURNING version
        "#
    } else {
        r#"
        UPDATE assessment_mark_sheets
           SET status = 'published', published_by = $3, published_at = NOW(),
               version = version + 1
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        RETURNING version
        "#
    };
    let version = sqlx::query_scalar::<_, i32>(query)
        .bind(tenant_id)
        .bind(mark_sheet_id)
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .with_context(|| format!("Failed to {target_status} assessment mark sheet"))?;
    append_sheet_event(
        &mut transaction,
        SheetEvent {
            tenant_id,
            mark_sheet_id,
            event_type,
            from_status: Some(expected_status),
            to_status: target_status,
            version,
            actor_id,
            reason: None,
            metadata: json!({ "learner_count": learner_count }),
        },
    )
    .await?;
    append_sheet_audit(
        &mut transaction,
        tenant_id,
        actor,
        request_context,
        action_key,
        mark_sheet_id,
        json!({ "learner_count": learner_count }),
    )
    .await?;
    transaction
        .commit()
        .await
        .with_context(|| format!("Failed to commit mark-sheet {target_status}"))?;
    GradebookOps::get(pool, tenant_id, mark_sheet_id, GradebookAccessScope::Campus).await
}

fn validate_new_sheet_reference(
    reference: &GradebookAssessmentReference,
    roster_on: chrono::NaiveDate,
) -> Result<()> {
    if reference.assessment_cycle_status != "open" {
        bail!("Mark sheets require an open assessment cycle");
    }
    if reference.assessment_component_status != "active" {
        bail!("Mark sheets require an active assessment component");
    }
    if roster_on < reference.academic_term_starts_on || roster_on > reference.academic_term_ends_on
    {
        bail!("The roster date must fall inside the assessment term");
    }
    if reference.occurs_on.is_some_and(|date| date != roster_on) {
        bail!("The roster date must match the configured assessment date");
    }
    Ok(())
}

fn component_reference(
    reference: GradebookAssessmentReference,
    mark_sheet_id: Option<Uuid>,
    mark_sheet_status: Option<String>,
    mark_sheet_version: Option<i32>,
) -> GradebookComponentReference {
    GradebookComponentReference {
        assessment_component_id: reference.assessment_component_id,
        assessment_component_code: reference.assessment_component_code,
        assessment_component_name: reference.assessment_component_name,
        assessment_kind: reference.assessment_kind,
        maximum_marks: reference.maximum_marks,
        weight_basis_points: reference.weight_basis_points,
        occurs_on: reference.occurs_on,
        assessment_cycle_id: reference.assessment_cycle_id,
        assessment_cycle_name: reference.assessment_cycle_name,
        assessment_cycle_status: reference.assessment_cycle_status,
        academic_term_id: reference.academic_term_id,
        academic_term_name: reference.academic_term_name,
        academic_term_starts_on: reference.academic_term_starts_on,
        academic_term_ends_on: reference.academic_term_ends_on,
        academic_year_id: reference.academic_year_id,
        academic_year_name: reference.academic_year_name,
        teaching_assignment_id: reference.teaching_assignment_id,
        class_group_id: reference.class_group_id,
        class_group_name: reference.class_group_name,
        subject_id: reference.subject_id,
        subject_name: reference.subject_name,
        teacher_profile_id: reference.teacher_profile_id,
        teacher_name: reference.teacher_name,
        mark_sheet_id,
        mark_sheet_status,
        mark_sheet_version,
    }
}

async fn hydrate_summary(
    pool: &PgPool,
    tenant_id: Uuid,
    row: MarkSheetSummaryRow,
) -> Result<GradebookSheetSummary> {
    let reference =
        AssessmentComponentOps::gradebook_reference(pool, tenant_id, row.assessment_component_id)
            .await?
            .context("The assessment structure for this mark sheet is unavailable")?;
    Ok(GradebookSheetSummary {
        id: row.id,
        assessment_component_id: row.assessment_component_id,
        assessment_component_code: reference.assessment_component_code,
        assessment_component_name: reference.assessment_component_name,
        assessment_kind: reference.assessment_kind,
        maximum_marks: reference.maximum_marks,
        weight_basis_points: reference.weight_basis_points,
        assessment_cycle_id: reference.assessment_cycle_id,
        assessment_cycle_name: reference.assessment_cycle_name,
        academic_term_id: reference.academic_term_id,
        academic_term_name: reference.academic_term_name,
        academic_year_id: reference.academic_year_id,
        academic_year_name: reference.academic_year_name,
        teaching_assignment_id: reference.teaching_assignment_id,
        class_group_id: reference.class_group_id,
        class_group_name: reference.class_group_name,
        subject_id: reference.subject_id,
        subject_name: reference.subject_name,
        teacher_profile_id: reference.teacher_profile_id,
        teacher_name: reference.teacher_name,
        roster_on: row.roster_on,
        status: row.status,
        version: row.version,
        learner_count: row.learner_count,
        scored_count: row.scored_count,
        absent_count: row.absent_count,
        exempt_count: row.exempt_count,
        unmarked_count: row.unmarked_count,
        average_percentage_basis_points: row.average_percentage_basis_points,
        created_at: row.created_at,
        submitted_at: row.submitted_at,
        published_at: row.published_at,
    })
}

fn summary_select(predicate: &str, trailing_clause: &str) -> String {
    format!(
        r#"
        SELECT sheet.id, sheet.assessment_component_id, sheet.roster_on,
               sheet.status, sheet.version, sheet.created_at, sheet.submitted_at,
               sheet.published_at,
               COUNT(mark.id)::BIGINT AS learner_count,
               COUNT(mark.id) FILTER (WHERE mark.mark_status = 'scored')::BIGINT AS scored_count,
               COUNT(mark.id) FILTER (WHERE mark.mark_status = 'absent')::BIGINT AS absent_count,
               COUNT(mark.id) FILTER (WHERE mark.mark_status = 'exempt')::BIGINT AS exempt_count,
               COUNT(mark.id) FILTER (WHERE mark.mark_status = 'unmarked')::BIGINT AS unmarked_count,
               CASE
                   WHEN COUNT(mark.id) FILTER (WHERE mark.mark_status = 'scored') = 0 THEN NULL
                   ELSE ROUND(
                       SUM(mark.marks_awarded_hundredths)
                           FILTER (WHERE mark.mark_status = 'scored') * 10000.0
                       / (COUNT(mark.id) FILTER (WHERE mark.mark_status = 'scored')
                           * component.maximum_marks * 100)
                   )::BIGINT
               END AS average_percentage_basis_points
          FROM assessment_mark_sheets AS sheet
          JOIN assessment_components AS component
            ON component.id = sheet.assessment_component_id
           AND component.tenant_id = sheet.tenant_id
           AND component.deleted_at IS NULL
          JOIN teaching_assignments AS assignment
            ON assignment.id = component.teaching_assignment_id
           AND assignment.tenant_id = component.tenant_id
           AND assignment.deleted_at IS NULL
          JOIN teacher_profiles AS teacher
            ON teacher.id = assignment.teacher_profile_id
           AND teacher.tenant_id = assignment.tenant_id
           AND teacher.deleted_at IS NULL
          JOIN employees AS employee
            ON employee.id = teacher.employee_id
           AND employee.tenant_id = teacher.tenant_id
           AND employee.deleted_at IS NULL
          LEFT JOIN assessment_marks AS mark
            ON mark.tenant_id = sheet.tenant_id
           AND mark.mark_sheet_id = sheet.id
           AND mark.deleted_at IS NULL
         WHERE {predicate}
         GROUP BY sheet.id, component.maximum_marks
         {trailing_clause}
        "#
    )
}

async fn count_sheets(
    pool: &PgPool,
    tenant_id: Uuid,
    status: Option<&str>,
    assigned_user_id: Option<Uuid>,
) -> Result<i64> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
          FROM assessment_mark_sheets AS sheet
          JOIN assessment_components AS component
            ON component.id = sheet.assessment_component_id
           AND component.tenant_id = sheet.tenant_id
           AND component.deleted_at IS NULL
          JOIN teaching_assignments AS assignment
            ON assignment.id = component.teaching_assignment_id
           AND assignment.tenant_id = component.tenant_id
           AND assignment.deleted_at IS NULL
          JOIN teacher_profiles AS teacher
            ON teacher.id = assignment.teacher_profile_id
           AND teacher.tenant_id = assignment.tenant_id
           AND teacher.deleted_at IS NULL
          JOIN employees AS employee
            ON employee.id = teacher.employee_id
           AND employee.tenant_id = teacher.tenant_id
           AND employee.deleted_at IS NULL
         WHERE sheet.tenant_id = $1
           AND sheet.deleted_at IS NULL
           AND ($2::TEXT IS NULL OR sheet.status = $2)
           AND ($3::UUID IS NULL OR employee.account_id = $3)
        "#,
    )
    .bind(tenant_id)
    .bind(status)
    .bind(assigned_user_id)
    .fetch_one(pool)
    .await
    .context("Failed to count assessment mark sheets")
}

fn reference_is_visible(
    reference: &GradebookAssessmentReference,
    access_scope: GradebookAccessScope,
) -> bool {
    match access_scope {
        GradebookAccessScope::Campus => true,
        GradebookAccessScope::AssignedTo(user_id) => reference.teacher_account_id == Some(user_id),
    }
}

async fn summary_row_by_id(
    pool: &PgPool,
    tenant_id: Uuid,
    mark_sheet_id: Uuid,
) -> Result<Option<MarkSheetSummaryRow>> {
    sqlx::query_as::<_, MarkSheetSummaryRow>(&summary_select(
        "sheet.tenant_id = $1 AND sheet.id = $2 AND sheet.deleted_at IS NULL",
        "",
    ))
    .bind(tenant_id)
    .bind(mark_sheet_id)
    .fetch_optional(pool)
    .await
    .context("Failed to load assessment mark-sheet summary")
}

async fn sheet_by_id(
    pool: &PgPool,
    tenant_id: Uuid,
    mark_sheet_id: Uuid,
) -> Result<Option<MarkSheetRow>> {
    sqlx::query_as::<_, MarkSheetRow>(
        r#"
        SELECT assessment_component_id, status, version,
               reopened_at, reopen_reason
          FROM assessment_mark_sheets
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(mark_sheet_id)
    .fetch_optional(pool)
    .await
    .context("Failed to load assessment mark sheet")
}

async fn lock_sheet(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    mark_sheet_id: Uuid,
) -> Result<Option<MarkSheetRow>> {
    sqlx::query_as::<_, MarkSheetRow>(
        r#"
        SELECT assessment_component_id, status, version,
               reopened_at, reopen_reason
          FROM assessment_mark_sheets
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(mark_sheet_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock assessment mark sheet")
}

async fn sheet_by_idempotency(
    pool: &PgPool,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> Result<Option<(Uuid, String)>> {
    sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT id, create_request_fingerprint
          FROM assessment_mark_sheets
         WHERE tenant_id = $1 AND idempotency_key = $2
        "#,
    )
    .bind(tenant_id)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await
    .context("Failed to check mark-sheet idempotency")
}

async fn sheet_states_by_component(
    pool: &PgPool,
    tenant_id: Uuid,
    component_ids: &[Uuid],
) -> Result<HashMap<Uuid, (Uuid, String, i32)>> {
    if component_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query_as::<_, (Uuid, Uuid, String, i32)>(
        r#"
        SELECT assessment_component_id, id, status, version
          FROM assessment_mark_sheets
         WHERE tenant_id = $1
           AND assessment_component_id = ANY($2)
           AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(component_ids)
    .fetch_all(pool)
    .await
    .context("Failed to load mark-sheet state")?;
    Ok(rows
        .into_iter()
        .map(|(component_id, sheet_id, status, version)| {
            (component_id, (sheet_id, status, version))
        })
        .collect())
}

async fn mark_completion_counts(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    mark_sheet_id: Uuid,
) -> Result<(i64, i64)> {
    sqlx::query_as::<_, (i64, i64)>(
        r#"
        SELECT COUNT(*)::BIGINT,
               COUNT(*) FILTER (WHERE mark_status = 'unmarked')::BIGINT
          FROM assessment_marks
         WHERE tenant_id = $1 AND mark_sheet_id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(mark_sheet_id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to validate mark-sheet completion")
}

async fn increment_sheet_version(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    mark_sheet_id: Uuid,
) -> Result<i32> {
    sqlx::query_scalar::<_, i32>(
        r#"
        UPDATE assessment_mark_sheets
           SET version = version + 1
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        RETURNING version
        "#,
    )
    .bind(tenant_id)
    .bind(mark_sheet_id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to version assessment mark sheet")
}

fn ensure_draft(sheet: &MarkSheetRow) -> Result<()> {
    if sheet.status != "draft" {
        bail!("Submitted or published mark sheets are locked");
    }
    Ok(())
}

fn ensure_version(sheet: &MarkSheetRow, expected_version: i32) -> Result<()> {
    if sheet.version != expected_version {
        bail!("This mark sheet changed. Reload it before continuing");
    }
    Ok(())
}

#[derive(Debug)]
struct ParsedMark {
    mark_status: GradebookMarkStatus,
    marks_awarded_hundredths: Option<i64>,
    note: Option<String>,
}

fn parse_marks(
    values: &[GradebookMarkInput],
    maximum_marks: i32,
) -> Result<BTreeMap<Uuid, ParsedMark>> {
    let maximum_hundredths = i64::from(maximum_marks) * 100;
    let mut parsed = BTreeMap::new();
    for value in values {
        let awarded = value.marks_awarded_hundredths;
        if (value.mark_status == GradebookMarkStatus::Scored) != awarded.is_some() {
            bail!("A scored learner requires a mark and other statuses cannot carry one");
        }
        if awarded.is_some_and(|marks| marks < 0 || marks > maximum_hundredths) {
            bail!("An awarded mark must be between zero and the assessment maximum");
        }
        let note = trimmed_optional(value.note.as_deref());
        if value.mark_status == GradebookMarkStatus::Unmarked && note.is_some() {
            bail!("An unmarked learner cannot have an assessment note");
        }
        if parsed
            .insert(
                value.learner_id,
                ParsedMark {
                    mark_status: value.mark_status,
                    marks_awarded_hundredths: awarded,
                    note,
                },
            )
            .is_some()
        {
            bail!("Each learner may appear only once in an assessment update");
        }
    }
    if parsed.is_empty() {
        bail!("Assessment marks are required");
    }
    Ok(parsed)
}

fn percentage_basis_points(marks_hundredths: i64, maximum_marks: i32) -> i64 {
    rounded_ratio(marks_hundredths, i64::from(maximum_marks) * 100, 10_000)
}

fn weighted_score_basis_points(
    marks_hundredths: i64,
    maximum_marks: i32,
    weight_basis_points: i16,
) -> i64 {
    rounded_ratio(
        marks_hundredths,
        i64::from(maximum_marks) * 100,
        i64::from(weight_basis_points),
    )
}

fn rounded_ratio(numerator: i64, denominator: i64, scale: i64) -> i64 {
    (numerator * scale + denominator / 2) / denominator
}

fn mark_counts(values: impl Iterator<Item = GradebookMarkStatus>) -> Value {
    let mut counts = BTreeMap::from([
        ("unmarked", 0_u64),
        ("scored", 0),
        ("absent", 0),
        ("exempt", 0),
    ]);
    for value in values {
        if let Some(count) = counts.get_mut(value.as_str()) {
            *count += 1;
        }
    }
    json!(counts)
}

struct SheetEvent<'a> {
    tenant_id: Uuid,
    mark_sheet_id: Uuid,
    event_type: &'a str,
    from_status: Option<&'a str>,
    to_status: &'a str,
    version: i32,
    actor_id: Uuid,
    reason: Option<&'a str>,
    metadata: Value,
}

async fn append_sheet_event(
    transaction: &mut Transaction<'_, Postgres>,
    event: SheetEvent<'_>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO assessment_mark_sheet_events (
            tenant_id, mark_sheet_id, event_type, from_status, to_status,
            mark_sheet_version, actor_id, reason, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(event.tenant_id)
    .bind(event.mark_sheet_id)
    .bind(event.event_type)
    .bind(event.from_status)
    .bind(event.to_status)
    .bind(event.version)
    .bind(event.actor_id)
    .bind(event.reason)
    .bind(event.metadata)
    .execute(&mut **transaction)
    .await
    .context("Failed to append mark-sheet history")?;
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "audit evidence is intentionally explicit"
)]
async fn append_sheet_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    action: &str,
    mark_sheet_id: Uuid,
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
            "assessment_mark_sheet",
            mark_sheet_id.to_string(),
        ))
        .with_redacted_metadata(metadata),
    )
    .await
    .context("Failed to append Gradebook audit event")?;
    Ok(())
}

fn create_fingerprint(request: &CreateMarkSheetRequest) -> String {
    let canonical = format!("{}|{}", request.assessment_component_id, request.roster_on);
    format!("{:x}", Sha256::digest(canonical.as_bytes()))
}

fn database_error(error: sqlx::Error, context: &str) -> anyhow::Error {
    if let sqlx::Error::Database(database) = &error {
        if database.constraint() == Some("idx_assessment_mark_sheets_component") {
            return anyhow!("A mark sheet already exists for this assessment component");
        }
        if database.constraint() == Some("idx_assessment_mark_sheets_idempotency") {
            return anyhow!("This mark-sheet request has already been processed");
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

fn ensure_transfer_mark_range(value: i64, maximum_hundredths: i64) -> Result<()> {
    if !(0..=maximum_hundredths).contains(&value) {
        bail!("A transferred mark is outside the assessment maximum");
    }
    Ok(())
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE),
        per_page.unwrap_or(DEFAULT_PER_PAGE).clamp(1, MAX_PER_PAGE),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        GradebookMarkInput, GradebookMarkStatus, ensure_transfer_mark_range, mark_counts,
        parse_marks, percentage_basis_points, summary_select, weighted_score_basis_points,
    };
    use uuid::Uuid;

    #[test]
    fn mark_parser_rejects_duplicate_learners() {
        let learner_id = Uuid::new_v4();
        let values = vec![
            GradebookMarkInput {
                learner_id,
                mark_status: GradebookMarkStatus::Scored,
                marks_awarded_hundredths: Some(7_500),
                note: None,
            },
            GradebookMarkInput {
                learner_id,
                mark_status: GradebookMarkStatus::Absent,
                marks_awarded_hundredths: None,
                note: None,
            },
        ];
        assert!(parse_marks(&values, 100).is_err());
    }

    #[test]
    fn mark_parser_rejects_values_above_the_component_maximum() {
        let values = vec![GradebookMarkInput {
            learner_id: Uuid::new_v4(),
            mark_status: GradebookMarkStatus::Scored,
            marks_awarded_hundredths: Some(5_001),
            note: None,
        }];
        assert!(parse_marks(&values, 50).is_err());
    }

    #[test]
    fn score_transfer_rechecks_the_gradebook_maximum() {
        assert!(ensure_transfer_mark_range(5_000, 5_000).is_ok());
        assert!(ensure_transfer_mark_range(-1, 5_000).is_err());
        assert!(ensure_transfer_mark_range(5_001, 5_000).is_err());
    }

    #[test]
    fn exact_scores_are_rounded_to_basis_points() {
        assert_eq!(percentage_basis_points(1_750, 20), 8_750);
        assert_eq!(weighted_score_basis_points(1_750, 20, 4_000), 3_500);
    }

    #[test]
    fn mark_counts_cover_every_status() {
        let counts = mark_counts(
            [
                GradebookMarkStatus::Scored,
                GradebookMarkStatus::Scored,
                GradebookMarkStatus::Absent,
                GradebookMarkStatus::Exempt,
                GradebookMarkStatus::Unmarked,
            ]
            .into_iter(),
        );
        assert_eq!(counts["scored"], 2);
        assert_eq!(counts["absent"], 1);
        assert_eq!(counts["exempt"], 1);
        assert_eq!(counts["unmarked"], 1);
    }

    #[test]
    fn summary_query_groups_before_ordering_and_pagination() {
        let query = summary_select(
            "sheet.tenant_id = $1 AND sheet.deleted_at IS NULL",
            "ORDER BY sheet.created_at DESC LIMIT $2 OFFSET $3",
        );
        let group_position = query
            .find("GROUP BY")
            .expect("summary query must aggregate mark rows");
        let order_position = query
            .find("ORDER BY")
            .expect("summary query must retain the caller's stable ordering");
        let limit_position = query
            .find("LIMIT")
            .expect("summary query must retain pagination");

        assert!(group_position < order_position);
        assert!(order_position < limit_position);
    }
}
