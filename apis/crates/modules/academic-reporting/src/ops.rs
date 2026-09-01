//! Transactional academic reporting operations and exact result calculation.
//!
//! Generation consumes typed source projections before one snapshot transaction.
//! Lifecycle writes use optimistic versions and append actor-aware audit evidence.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::{Context, Result, anyhow, bail};
use cp_academics::ops::AcademicGradeLevelOps;
use cp_attendance::{AttendanceLearnerSummary, AttendanceOps};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_gradebook::{GradebookOps, GradebookReportingSource, PublishedAssessmentMark};
use cp_sis::{models::ClassRosterEntry, ops::EnrolmentOps};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::dtos::{
    AcademicAttendanceResponse, AcademicGradeLevelReference, AcademicReportBatchListQuery,
    AcademicReportBatchResponse, AcademicReportBatchSummary, AcademicReportCardResponse,
    AcademicReportReferenceData, AcademicSubjectResultResponse, AcademicTranscriptEntry,
    AcademicTranscriptResponse, CreateGradingSchemeRequest, GenerateAcademicReportRequest,
    GradingBandInput, GradingBandResponse, GradingSchemeResponse,
    PaginatedAcademicReportBatchesResponse, ReopenAcademicReportRequest, ReportingSourceBoundary,
    UpdateGradingSchemeRequest, UpdateReportCardReviewRequest,
    UpdateReportCardTeacherCommentRequest,
};
use crate::models::{
    AttendanceSnapshotRow, GradingBandRow, GradingSchemeRow, ReportBatchSummaryRow, ReportCardRow,
    SubjectResultRow,
};

const DEFAULT_PAGE: i64 = 1;
const DEFAULT_PER_PAGE: i64 = 25;
const MAX_PAGE: i64 = 1_000_000;
const MAX_PER_PAGE: i64 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcademicReportingAccessScope {
    Campus,
    AssignedTo(Uuid),
    SelfFor(Uuid),
    SelfAndAssigned(Uuid),
}

pub struct AcademicReportingOps;

impl AcademicReportingOps {
    /// Returns report-ready sources and configuration visible to the caller.
    pub async fn reference_data(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: AcademicReportingAccessScope,
    ) -> Result<AcademicReportReferenceData> {
        let mut sources = GradebookOps::reporting_sources(pool, tenant_id).await?;
        match scope {
            AcademicReportingAccessScope::Campus => {}
            AcademicReportingAccessScope::AssignedTo(user_id)
            | AcademicReportingAccessScope::SelfAndAssigned(user_id) => {
                sources.retain(|source| source.teacher_account_ids.contains(&user_id));
            }
            AcademicReportingAccessScope::SelfFor(_) => sources.clear(),
        }

        let configuration_visible = !matches!(scope, AcademicReportingAccessScope::SelfFor(_));
        let grading_schemes = if configuration_visible {
            Self::list_grading_schemes(pool, tenant_id, Some("active")).await?
        } else {
            Vec::new()
        };
        let grade_levels = if configuration_visible {
            AcademicGradeLevelOps::list(pool, tenant_id, 1, 100, None, Some("active"))
                .await?
                .0
                .into_iter()
                .map(|grade| AcademicGradeLevelReference {
                    id: grade.id,
                    code: grade.code,
                    name: grade.name,
                    sort_order: i32::from(grade.sequence_number),
                })
                .collect()
        } else {
            Vec::new()
        };
        Ok(AcademicReportReferenceData {
            sources,
            grading_schemes,
            grade_levels,
        })
    }

    pub async fn list_grading_schemes(
        pool: &PgPool,
        tenant_id: Uuid,
        status: Option<&str>,
    ) -> Result<Vec<GradingSchemeResponse>> {
        let rows = sqlx::query_as::<_, GradingSchemeRow>(
            r#"
            SELECT id, name, description, is_default, status, version,
                   created_at, updated_at
              FROM academic_grading_schemes
             WHERE tenant_id = $1
               AND deleted_at IS NULL
               AND ($2::TEXT IS NULL OR status = $2)
             ORDER BY is_default DESC, name, id
            "#,
        )
        .bind(tenant_id)
        .bind(status)
        .fetch_all(pool)
        .await
        .context("Failed to list academic grading schemes")?;
        let mut schemes = Vec::with_capacity(rows.len());
        for row in rows {
            schemes.push(hydrate_scheme(pool, tenant_id, row).await?);
        }
        Ok(schemes)
    }

    pub async fn get_grading_scheme(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<GradingSchemeResponse>> {
        let row = sqlx::query_as::<_, GradingSchemeRow>(
            r#"
            SELECT id, name, description, is_default, status, version,
                   created_at, updated_at
              FROM academic_grading_schemes
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load academic grading scheme")?;
        match row {
            Some(row) => Ok(Some(hydrate_scheme(pool, tenant_id, row).await?)),
            None => Ok(None),
        }
    }

    pub async fn create_grading_scheme(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateGradingSchemeRequest,
    ) -> Result<GradingSchemeResponse> {
        let actor_id = person_actor_id(actor)?;
        let name = trimmed_required(&request.name, "Grading scheme name")?;
        let description = optional_text(request.description.as_deref());
        let bands = normalized_bands(&request.bands)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start grading scheme creation")?;
        if request.is_default {
            clear_default_scheme(&mut transaction, tenant_id, actor_id, None).await?;
        }
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO academic_grading_schemes (
                tenant_id, name, description, is_default, created_by, updated_by
            ) VALUES ($1, $2, $3, $4, $5, $5)
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(name)
        .bind(description)
        .bind(request.is_default)
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "Failed to create academic grading scheme"))?;
        insert_bands(&mut transaction, tenant_id, id, &bands).await?;
        append_reporting_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "academics.reporting.grading_schemes.create",
            "academic_grading_scheme",
            id,
            json!({ "band_count": bands.len(), "is_default": request.is_default }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit grading scheme creation")?;
        Self::get_grading_scheme(pool, tenant_id, id)
            .await?
            .context("The created grading scheme is unavailable")
    }

    pub async fn update_grading_scheme(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateGradingSchemeRequest,
    ) -> Result<Option<GradingSchemeResponse>> {
        let actor_id = person_actor_id(actor)?;
        let name = trimmed_required(&request.name, "Grading scheme name")?;
        let description = optional_text(request.description.as_deref());
        let bands = normalized_bands(&request.bands)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start grading scheme update")?;
        if request.is_default {
            clear_default_scheme(&mut transaction, tenant_id, actor_id, Some(id)).await?;
        }
        let updated = sqlx::query(
            r#"
            UPDATE academic_grading_schemes
               SET name = $1, description = $2, is_default = $3,
                   updated_by = $4, version = version + 1, updated_at = NOW()
             WHERE tenant_id = $5 AND id = $6 AND version = $7
               AND status = 'active' AND deleted_at IS NULL
            "#,
        )
        .bind(name)
        .bind(description)
        .bind(request.is_default)
        .bind(actor_id)
        .bind(tenant_id)
        .bind(id)
        .bind(request.expected_version)
        .execute(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "Failed to update academic grading scheme"))?;
        if updated.rows_affected() == 0 {
            let exists = scheme_exists(&mut transaction, tenant_id, id).await?;
            transaction
                .rollback()
                .await
                .context("Failed to roll back grading scheme update")?;
            if exists {
                bail!("The grading scheme changed. Reload it and try again");
            }
            return Ok(None);
        }
        sqlx::query(
            "UPDATE academic_grading_bands SET deleted_at = NOW() WHERE tenant_id = $1 AND grading_scheme_id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to replace grading bands")?;
        insert_bands(&mut transaction, tenant_id, id, &bands).await?;
        append_reporting_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "academics.reporting.grading_schemes.update",
            "academic_grading_scheme",
            id,
            json!({ "band_count": bands.len(), "is_default": request.is_default }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit grading scheme update")?;
        Self::get_grading_scheme(pool, tenant_id, id).await
    }

    pub async fn retire_grading_scheme(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<GradingSchemeResponse>> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start grading scheme retirement")?;
        let updated = sqlx::query(
            r#"
            UPDATE academic_grading_schemes
               SET status = 'retired', is_default = FALSE, updated_by = $1,
                   version = version + 1, updated_at = NOW()
             WHERE tenant_id = $2 AND id = $3 AND version = $4
               AND status = 'active' AND deleted_at IS NULL
            "#,
        )
        .bind(actor_id)
        .bind(tenant_id)
        .bind(id)
        .bind(expected_version)
        .execute(&mut *transaction)
        .await
        .context("Failed to retire academic grading scheme")?;
        if updated.rows_affected() == 0 {
            let exists = scheme_exists(&mut transaction, tenant_id, id).await?;
            transaction
                .rollback()
                .await
                .context("Failed to roll back grading scheme retirement")?;
            if exists {
                bail!("The grading scheme changed or is already retired");
            }
            return Ok(None);
        }
        append_reporting_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "academics.reporting.grading_schemes.retire",
            "academic_grading_scheme",
            id,
            json!({}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit grading scheme retirement")?;
        Self::get_grading_scheme(pool, tenant_id, id).await
    }

    pub async fn delete_grading_scheme(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<bool> {
        let _actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start grading scheme deletion")?;
        let in_use = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM academic_report_batches WHERE tenant_id = $1 AND grading_scheme_id = $2)",
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to check grading scheme use")?;
        if in_use {
            bail!("This grading scheme is already used by an academic report");
        }
        let deleted = sqlx::query(
            r#"
            UPDATE academic_grading_schemes
               SET deleted_at = NOW(), is_default = FALSE
             WHERE tenant_id = $1 AND id = $2 AND version = $3 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(expected_version)
        .execute(&mut *transaction)
        .await
        .context("Failed to delete academic grading scheme")?;
        if deleted.rows_affected() == 0 {
            let exists = scheme_exists(&mut transaction, tenant_id, id).await?;
            transaction
                .rollback()
                .await
                .context("Failed to roll back grading scheme deletion")?;
            if exists {
                bail!("The grading scheme changed. Reload it and try again");
            }
            return Ok(false);
        }
        append_reporting_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "academics.reporting.grading_schemes.delete",
            "academic_grading_scheme",
            id,
            json!({}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit grading scheme deletion")?;
        Ok(true)
    }

    pub async fn list_report_batches(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &AcademicReportBatchListQuery,
        scope: AcademicReportingAccessScope,
    ) -> Result<(PaginatedAcademicReportBatchesResponse, i64)> {
        let (page, per_page) = bounded_page(query.page, query.per_page);
        let offset = (page - 1) * per_page;
        let status = query.status.map(|value| value.as_str());
        let (scope_kind, person_id) = scope_parts(scope);
        let rows = sqlx::query_as::<_, ReportBatchSummaryRow>(&batch_summary_query(
            "batch.tenant_id = $1 AND batch.deleted_at IS NULL AND ($2::TEXT IS NULL OR batch.status = $2) AND reporting_scope_allows(batch.tenant_id, batch.id, $3, $4) ORDER BY batch.created_at DESC, batch.id LIMIT $5 OFFSET $6",
        ))
        .bind(tenant_id)
        .bind(status)
        .bind(scope_kind)
        .bind(person_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list academic report batches")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
              FROM academic_report_batches AS batch
             WHERE batch.tenant_id = $1 AND batch.deleted_at IS NULL
               AND ($2::TEXT IS NULL OR batch.status = $2)
               AND reporting_scope_allows(batch.tenant_id, batch.id, $3, $4)
            "#,
        )
        .bind(tenant_id)
        .bind(status)
        .bind(scope_kind)
        .bind(person_id)
        .fetch_one(pool)
        .await
        .context("Failed to count academic report batches")?;
        Ok((
            PaginatedAcademicReportBatchesResponse {
                report_batches: rows.into_iter().map(summary_response).collect(),
            },
            total,
        ))
    }

    pub async fn get_report_batch(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        scope: AcademicReportingAccessScope,
    ) -> Result<Option<AcademicReportBatchResponse>> {
        if !can_access_batch(pool, tenant_id, id, scope).await? {
            return Ok(None);
        }
        let Some(row) = report_batch_row(pool, tenant_id, id).await? else {
            return Ok(None);
        };
        let summary = summary_response(row.clone());
        let self_learner_ids = self_learner_ids(pool, tenant_id, scope).await?;
        let cards =
            report_cards_for_batch(pool, tenant_id, id, self_learner_ids.as_deref()).await?;
        Ok(Some(AcademicReportBatchResponse {
            summary,
            cards,
            reopened_at: row.reopened_at,
            reopen_reason: row.reopen_reason,
        }))
    }

    pub async fn generate_report_batch(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        scope: AcademicReportingAccessScope,
        request: &GenerateAcademicReportRequest,
    ) -> Result<AcademicReportBatchResponse> {
        let actor_id = person_actor_id(actor)?;
        if matches!(scope, AcademicReportingAccessScope::SelfFor(_)) {
            bail!("Self-service access cannot generate academic reports");
        }
        let idempotency_key = trimmed_required(&request.idempotency_key, "Idempotency key")?;
        let sources = GradebookOps::reporting_sources(pool, tenant_id).await?;
        let source = sources
            .into_iter()
            .find(|source| {
                source.assessment_cycle_id == request.assessment_cycle_id
                    && source.class_group_id == request.class_group_id
            })
            .context("The selected closed assessment cycle and class are not report-ready")?;
        if !source_visible(&source, scope) {
            bail!("The selected reporting source is unavailable");
        }
        let scheme = Self::get_grading_scheme(pool, tenant_id, request.grading_scheme_id)
            .await?
            .context("The selected grading scheme was not found")?;
        if scheme.status != "active" {
            bail!("Only an active grading scheme can generate reports");
        }
        let marks = GradebookOps::published_results_for_cycle_class(
            pool,
            tenant_id,
            request.assessment_cycle_id,
            request.class_group_id,
        )
        .await?;
        if marks.is_empty() {
            bail!("The selected class has no published learner marks");
        }
        let end_roster = EnrolmentOps::class_roster_on(
            pool,
            tenant_id,
            source.academic_year_id,
            source.class_group_id,
            source.academic_term_ends_on,
        )
        .await?;
        let enrolment_ids = marks
            .iter()
            .map(|mark| mark.enrolment_id)
            .chain(end_roster.iter().map(|entry| entry.enrolment_id))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let roster_identities =
            EnrolmentOps::roster_references_by_enrolment_ids(pool, tenant_id, &enrolment_ids)
                .await?;
        let attendance = AttendanceOps::submitted_summaries_for_class(
            pool,
            tenant_id,
            source.class_group_id,
            source.academic_term_starts_on,
            source.academic_term_ends_on,
        )
        .await?;
        let source_fingerprint = source_fingerprint(&marks, &attendance, &end_roster);
        if let Some(existing) = report_by_idempotency(pool, tenant_id, idempotency_key).await? {
            if existing.assessment_cycle_id != request.assessment_cycle_id
                || existing.class_group_id != request.class_group_id
                || existing.grading_scheme_id != request.grading_scheme_id
                || existing.source_fingerprint != source_fingerprint
            {
                bail!("This idempotency key was already used for another academic report");
            }
            return Self::get_report_batch(pool, tenant_id, existing.id, scope)
                .await?
                .context("The existing academic report is unavailable");
        }
        let boundary = ReportingSourceBoundary {
            assessment_cycle_id: request.assessment_cycle_id,
            class_group_id: request.class_group_id,
            academic_year_id: source.academic_year_id,
            term_starts_on: source.academic_term_starts_on,
            term_ends_on: source.academic_term_ends_on,
        };
        let cards = calculate_report_cards(
            &marks,
            &attendance,
            &end_roster,
            &roster_identities,
            &scheme.bands,
        )?;
        if cards.is_empty() {
            bail!("The selected class has no learners to report");
        }
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start academic report generation")?;
        let batch_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO academic_report_batches (
                tenant_id, assessment_cycle_id, class_group_id,
                grading_scheme_id, grading_scheme_version,
                grading_scheme_name_snapshot, source_fingerprint,
                idempotency_key, generated_by
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(boundary.assessment_cycle_id)
        .bind(boundary.class_group_id)
        .bind(scheme.id)
        .bind(scheme.version)
        .bind(&scheme.name)
        .bind(&source_fingerprint)
        .bind(idempotency_key)
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "Failed to create academic report batch"))?;
        for card in &cards {
            insert_calculated_card(&mut transaction, tenant_id, batch_id, card).await?;
        }
        append_report_event(
            &mut transaction,
            tenant_id,
            batch_id,
            "generated",
            None,
            "draft",
            1,
            actor_id,
            None,
            json!({
                "learner_count": cards.len(),
                "assessment_cycle_id": boundary.assessment_cycle_id,
                "class_group_id": boundary.class_group_id,
                "academic_year_id": boundary.academic_year_id,
                "term_starts_on": boundary.term_starts_on,
                "term_ends_on": boundary.term_ends_on
            }),
        )
        .await?;
        append_reporting_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "academics.reporting.report_batches.generate",
            "academic_report_batch",
            batch_id,
            json!({ "learner_count": cards.len() }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit academic report generation")?;
        Self::get_report_batch(pool, tenant_id, batch_id, scope)
            .await?
            .context("The generated academic report is unavailable")
    }

    pub async fn update_teacher_comment(
        pool: &PgPool,
        tenant_id: Uuid,
        card_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        scope: AcademicReportingAccessScope,
        request: &UpdateReportCardTeacherCommentRequest,
    ) -> Result<Option<AcademicReportBatchResponse>> {
        if matches!(scope, AcademicReportingAccessScope::SelfFor(_)) {
            bail!("Self-service access cannot change report cards");
        }
        let actor_id = person_actor_id(actor)?;
        let Some(batch_id) = batch_for_card(pool, tenant_id, card_id).await? else {
            return Ok(None);
        };
        if !can_access_batch(pool, tenant_id, batch_id, scope).await? {
            return Ok(None);
        }
        let comment = optional_text(request.teacher_comment.as_deref());
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start report-card update")?;
        let updated = sqlx::query(
            r#"
            UPDATE academic_report_cards AS card
               SET teacher_comment = $1, version = version + 1, updated_at = NOW()
              FROM academic_report_batches AS batch
             WHERE card.tenant_id = $2 AND card.id = $3 AND card.version = $4
               AND card.deleted_at IS NULL
               AND batch.id = card.report_batch_id AND batch.tenant_id = card.tenant_id
               AND batch.status = 'draft' AND batch.deleted_at IS NULL
            "#,
        )
        .bind(comment)
        .bind(tenant_id)
        .bind(card_id)
        .bind(request.expected_version)
        .execute(&mut *transaction)
        .await
        .context("Failed to update teacher comment")?;
        if updated.rows_affected() == 0 {
            transaction
                .rollback()
                .await
                .context("Failed to roll back report-card update")?;
            bail!("The report card changed or is no longer a draft");
        }
        let batch_version = current_batch_version(&mut transaction, tenant_id, batch_id).await?;
        append_report_event(
            &mut transaction,
            tenant_id,
            batch_id,
            "remarks_updated",
            Some("draft"),
            "draft",
            batch_version,
            actor_id,
            None,
            json!({ "report_card_id": card_id, "field": "teacher_comment" }),
        )
        .await?;
        append_reporting_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "academics.reporting.report_cards.teacher_comment.update",
            "academic_report_card",
            card_id,
            json!({}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit report-card update")?;
        Self::get_report_batch(pool, tenant_id, batch_id, scope).await
    }

    pub async fn update_report_review(
        pool: &PgPool,
        tenant_id: Uuid,
        card_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateReportCardReviewRequest,
    ) -> Result<Option<AcademicReportBatchResponse>> {
        let actor_id = person_actor_id(actor)?;
        let Some(batch_id) = batch_for_card(pool, tenant_id, card_id).await? else {
            return Ok(None);
        };
        let reviewer_comment = optional_text(request.reviewer_comment.as_deref());
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start progression review")?;
        if let Some(target_id) = request.target_grade_level_id {
            let valid_target = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM academic_grade_levels WHERE tenant_id = $1 AND id = $2 AND status = 'active' AND deleted_at IS NULL)",
            )
            .bind(tenant_id)
            .bind(target_id)
            .fetch_one(&mut *transaction)
            .await
            .context("Failed to validate progression grade")?;
            if !valid_target {
                bail!("The selected progression grade is unavailable");
            }
        }
        let updated = sqlx::query(
            r#"
            UPDATE academic_report_cards AS card
               SET reviewer_comment = $1, progression_outcome = $2,
                   target_grade_level_id = $3, version = version + 1, updated_at = NOW()
              FROM academic_report_batches AS batch
             WHERE card.tenant_id = $4 AND card.id = $5 AND card.version = $6
               AND card.deleted_at IS NULL
               AND batch.id = card.report_batch_id AND batch.tenant_id = card.tenant_id
               AND batch.status = 'draft' AND batch.deleted_at IS NULL
            "#,
        )
        .bind(reviewer_comment)
        .bind(request.progression_outcome.as_str())
        .bind(request.target_grade_level_id)
        .bind(tenant_id)
        .bind(card_id)
        .bind(request.expected_version)
        .execute(&mut *transaction)
        .await
        .context("Failed to update progression review")?;
        if updated.rows_affected() == 0 {
            transaction
                .rollback()
                .await
                .context("Failed to roll back progression review")?;
            bail!("The report card changed or is no longer a draft");
        }
        let batch_version = current_batch_version(&mut transaction, tenant_id, batch_id).await?;
        append_report_event(
            &mut transaction,
            tenant_id,
            batch_id,
            "remarks_updated",
            Some("draft"),
            "draft",
            batch_version,
            actor_id,
            None,
            json!({
                "report_card_id": card_id,
                "field": "review_and_progression",
                "progression_outcome": request.progression_outcome.as_str()
            }),
        )
        .await?;
        append_reporting_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "academics.reporting.report_cards.review.update",
            "academic_report_card",
            card_id,
            json!({ "progression_outcome": request.progression_outcome.as_str() }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit progression review")?;
        Self::get_report_batch(
            pool,
            tenant_id,
            batch_id,
            AcademicReportingAccessScope::Campus,
        )
        .await
    }

    pub async fn review_report_batch(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<AcademicReportBatchResponse>> {
        transition_batch(
            pool,
            tenant_id,
            id,
            actor,
            request_context,
            expected_version,
            BatchTransition::Review,
        )
        .await
    }

    pub async fn publish_report_batch(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<AcademicReportBatchResponse>> {
        transition_batch(
            pool,
            tenant_id,
            id,
            actor,
            request_context,
            expected_version,
            BatchTransition::Publish,
        )
        .await
    }

    pub async fn reopen_report_batch(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ReopenAcademicReportRequest,
    ) -> Result<Option<AcademicReportBatchResponse>> {
        let reason = trimmed_required(&request.reason, "Reopen reason")?.to_string();
        transition_batch(
            pool,
            tenant_id,
            id,
            actor,
            request_context,
            request.expected_version,
            BatchTransition::Reopen(reason),
        )
        .await
    }

    pub async fn delete_report_batch(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<bool> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start report deletion")?;
        let deleted = sqlx::query(
            r#"
            UPDATE academic_report_batches
               SET deleted_at = NOW(), version = version + 1, updated_at = NOW()
             WHERE tenant_id = $1 AND id = $2 AND version = $3
               AND status = 'draft' AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(expected_version)
        .execute(&mut *transaction)
        .await
        .context("Failed to delete academic report")?;
        if deleted.rows_affected() == 0 {
            let exists = report_exists(&mut transaction, tenant_id, id).await?;
            transaction
                .rollback()
                .await
                .context("Failed to roll back report deletion")?;
            if exists {
                bail!("Only an unchanged draft academic report can be deleted");
            }
            return Ok(false);
        }
        append_report_event(
            &mut transaction,
            tenant_id,
            id,
            "deleted",
            Some("draft"),
            "deleted",
            expected_version + 1,
            actor_id,
            None,
            json!({}),
        )
        .await?;
        append_reporting_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "academics.reporting.report_batches.delete",
            "academic_report_batch",
            id,
            json!({}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit report deletion")?;
        Ok(true)
    }

    pub async fn learner_transcript(
        pool: &PgPool,
        tenant_id: Uuid,
        learner_id: Uuid,
        scope: AcademicReportingAccessScope,
    ) -> Result<Option<AcademicTranscriptResponse>> {
        if !can_access_learner_transcript(pool, tenant_id, learner_id, scope).await? {
            return Ok(None);
        }
        let rows = sqlx::query_as::<_, ReportBatchSummaryRow>(&batch_summary_query(
            "batch.tenant_id = $1 AND batch.status = 'published' AND batch.deleted_at IS NULL AND EXISTS (SELECT 1 FROM academic_report_cards transcript_card WHERE transcript_card.tenant_id = batch.tenant_id AND transcript_card.report_batch_id = batch.id AND transcript_card.learner_id = $2 AND transcript_card.deleted_at IS NULL) ORDER BY term.ends_on, batch.published_at, batch.id",
        ))
        .bind(tenant_id)
        .bind(learner_id)
        .fetch_all(pool)
        .await
        .context("Failed to load transcript periods")?;
        let mut entries = Vec::with_capacity(rows.len());
        let mut published_identity = None;
        for row in rows {
            let card = report_cards_for_batch(pool, tenant_id, row.id, Some(&[learner_id]))
                .await?
                .into_iter()
                .next()
                .context("A published transcript card is unavailable")?;
            published_identity = Some((card.learner_number.clone(), card.learner_name.clone()));
            entries.push(AcademicTranscriptEntry {
                report_batch_id: row.id,
                assessment_cycle_name: row.assessment_cycle_name,
                academic_term_name: row.academic_term_name,
                academic_year_name: row.academic_year_name,
                class_group_name: row.class_group_name,
                published_at: row
                    .published_at
                    .context("A published report is missing its publication time")?,
                overall_percentage_basis_points: card.overall_percentage_basis_points,
                overall_grade_code: card.overall_grade_code,
                overall_grade_label: card.overall_grade_label,
                progression_outcome: card.progression_outcome,
                subjects: card.subjects,
            });
        }
        let identity = if let Some(identity) = published_identity {
            Some(identity)
        } else {
            sqlx::query_as::<_, (String, String)>(
                "SELECT learner_number, display_name FROM learners WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
            )
            .bind(tenant_id)
            .bind(learner_id)
            .fetch_optional(pool)
            .await
            .context("Failed to load transcript learner")?
        };
        let Some((learner_number, learner_name)) = identity else {
            return Ok(None);
        };
        Ok(Some(AcademicTranscriptResponse {
            learner_id,
            learner_number,
            learner_name,
            entries,
        }))
    }
}

#[derive(Debug, Clone)]
struct CalculatedCard {
    enrolment_id: Uuid,
    learner_id: Uuid,
    learner_number: String,
    learner_name: String,
    overall_percentage_basis_points: Option<i16>,
    overall_band: Option<CalculatedBand>,
    subjects: Vec<CalculatedSubject>,
    attendance: CalculatedAttendance,
}

#[derive(Debug, Clone)]
struct CalculatedSubject {
    teaching_assignment_id: Uuid,
    subject_id: Uuid,
    subject_name: String,
    status: &'static str,
    percentage_basis_points: Option<i16>,
    band: Option<CalculatedBand>,
    scored_count: i32,
    absent_count: i32,
    exempt_count: i32,
}

#[derive(Debug, Clone)]
struct CalculatedBand {
    code: String,
    label: String,
    is_pass: bool,
}

#[derive(Debug, Clone, Copy)]
struct CalculatedAttendance {
    present_count: i32,
    absent_count: i32,
    late_count: i32,
    excused_count: i32,
    percentage_basis_points: Option<i16>,
}

#[derive(Debug, Clone)]
enum BatchTransition {
    Review,
    Publish,
    Reopen(String),
}

async fn transition_batch(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    expected_version: i32,
    transition: BatchTransition,
) -> Result<Option<AcademicReportBatchResponse>> {
    let actor_id = person_actor_id(actor)?;
    let Some(existing) = report_batch_row(pool, tenant_id, id).await? else {
        return Ok(None);
    };
    if matches!(&transition, BatchTransition::Review) {
        let current_fingerprint = current_source_fingerprint(pool, tenant_id, &existing).await?;
        if current_fingerprint != existing.source_fingerprint {
            bail!(
                "Source marks, attendance, or roster changed. Delete and generate this report again"
            );
        }
    }
    let (expected_status, next_status, event_type, action_key, reason) = match &transition {
        BatchTransition::Review => (
            "draft",
            "reviewed",
            "reviewed",
            "academics.reporting.report_batches.review",
            None,
        ),
        BatchTransition::Publish => (
            "reviewed",
            "published",
            "published",
            "academics.reporting.report_batches.publish",
            None,
        ),
        BatchTransition::Reopen(reason) => (
            existing.status.as_str(),
            "draft",
            "reopened",
            "academics.reporting.report_batches.reopen",
            Some(reason.as_str()),
        ),
    };
    if matches!(&transition, BatchTransition::Reopen(_))
        && !matches!(existing.status.as_str(), "reviewed" | "published")
    {
        bail!("Only a reviewed or published academic report can be reopened");
    }
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to start report transition")?;
    let updated = match &transition {
        BatchTransition::Review => {
            sqlx::query(
                r#"
            UPDATE academic_report_batches
               SET status = 'reviewed', reviewed_by = $1, reviewed_at = NOW(),
                   reopened_by = NULL, reopened_at = NULL, reopen_reason = NULL,
                   version = version + 1, updated_at = NOW()
             WHERE tenant_id = $2 AND id = $3 AND version = $4
               AND status = 'draft' AND deleted_at IS NULL
            "#,
            )
            .bind(actor_id)
            .bind(tenant_id)
            .bind(id)
            .bind(expected_version)
            .execute(&mut *transaction)
            .await
        }
        BatchTransition::Publish => {
            sqlx::query(
                r#"
            UPDATE academic_report_batches
               SET status = 'published', published_by = $1, published_at = NOW(),
                   version = version + 1, updated_at = NOW()
             WHERE tenant_id = $2 AND id = $3 AND version = $4
               AND status = 'reviewed' AND deleted_at IS NULL
            "#,
            )
            .bind(actor_id)
            .bind(tenant_id)
            .bind(id)
            .bind(expected_version)
            .execute(&mut *transaction)
            .await
        }
        BatchTransition::Reopen(reopen_reason) => {
            sqlx::query(
                r#"
            UPDATE academic_report_batches
               SET status = 'draft', reviewed_by = NULL, reviewed_at = NULL,
                   published_by = NULL, published_at = NULL,
                   reopened_by = $1, reopened_at = NOW(), reopen_reason = $2,
                   version = version + 1, updated_at = NOW()
             WHERE tenant_id = $3 AND id = $4 AND version = $5
               AND status IN ('reviewed', 'published') AND deleted_at IS NULL
            "#,
            )
            .bind(actor_id)
            .bind(reopen_reason)
            .bind(tenant_id)
            .bind(id)
            .bind(expected_version)
            .execute(&mut *transaction)
            .await
        }
    }
    .context("Failed to transition academic report")?;
    if updated.rows_affected() == 0 {
        transaction
            .rollback()
            .await
            .context("Failed to roll back report transition")?;
        bail!("The academic report changed or cannot move to the requested status");
    }
    append_report_event(
        &mut transaction,
        tenant_id,
        id,
        event_type,
        Some(expected_status),
        next_status,
        expected_version + 1,
        actor_id,
        reason,
        json!({}),
    )
    .await?;
    append_reporting_audit(
        &mut transaction,
        tenant_id,
        actor,
        request_context,
        action_key,
        "academic_report_batch",
        id,
        json!({ "from_status": expected_status, "to_status": next_status }),
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit report transition")?;
    AcademicReportingOps::get_report_batch(
        pool,
        tenant_id,
        id,
        AcademicReportingAccessScope::Campus,
    )
    .await
}

fn calculate_report_cards(
    marks: &[PublishedAssessmentMark],
    attendance: &[AttendanceLearnerSummary],
    end_roster: &[ClassRosterEntry],
    roster_identities: &[ClassRosterEntry],
    bands: &[GradingBandResponse],
) -> Result<Vec<CalculatedCard>> {
    let mut learners = BTreeMap::<Uuid, (Uuid, Vec<&PublishedAssessmentMark>)>::new();
    for mark in marks {
        learners
            .entry(mark.learner_id)
            .or_insert((mark.enrolment_id, Vec::new()))
            .1
            .push(mark);
    }
    for roster in end_roster {
        learners
            .entry(roster.learner_id)
            .or_insert((roster.enrolment_id, Vec::new()));
    }
    let assignments = marks
        .iter()
        .map(|mark| {
            (
                mark.teaching_assignment_id,
                mark.subject_id,
                mark.subject_name.clone(),
            )
        })
        .collect::<BTreeSet<_>>();
    let identities = roster_identities
        .iter()
        .map(|entry| (entry.enrolment_id, entry))
        .collect::<HashMap<_, _>>();
    let attendance_by_learner = attendance
        .iter()
        .map(|summary| (summary.learner_id, summary))
        .collect::<HashMap<_, _>>();
    let mut cards = Vec::with_capacity(learners.len());
    for (learner_id, (enrolment_id, learner_marks)) in learners {
        let identity = identities
            .get(&enrolment_id)
            .context("A report learner identity is unavailable from SIS")?;
        if identity.learner_id != learner_id {
            bail!("A report enrolment no longer belongs to its learner");
        }
        let mut subjects = Vec::with_capacity(assignments.len());
        for (assignment_id, subject_id, subject_name) in &assignments {
            let subject_marks = learner_marks
                .iter()
                .copied()
                .filter(|mark| mark.teaching_assignment_id == *assignment_id)
                .collect::<Vec<_>>();
            subjects.push(calculate_subject(
                *assignment_id,
                *subject_id,
                subject_name,
                &subject_marks,
                bands,
            )?);
        }
        let graded = subjects
            .iter()
            .filter_map(|subject| subject.percentage_basis_points)
            .map(i64::from)
            .collect::<Vec<_>>();
        let has_incomplete = subjects
            .iter()
            .any(|subject| subject.status == "incomplete");
        let overall = if has_incomplete || graded.is_empty() {
            None
        } else {
            Some(round_ratio(
                graded.iter().sum::<i64>(),
                i64::try_from(graded.len()).map_err(|_| anyhow!("Too many subject results"))?,
            )?)
        };
        let overall_band = overall
            .map(|value| band_for_score(value, bands))
            .transpose()?;
        let attendance = calculate_attendance(attendance_by_learner.get(&learner_id).copied())?;
        cards.push(CalculatedCard {
            enrolment_id,
            learner_id,
            learner_number: identity.learner_number.clone(),
            learner_name: identity.display_name.clone(),
            overall_percentage_basis_points: overall,
            overall_band,
            subjects,
            attendance,
        });
    }
    Ok(cards)
}

fn calculate_subject(
    teaching_assignment_id: Uuid,
    subject_id: Uuid,
    subject_name: &str,
    marks: &[&PublishedAssessmentMark],
    bands: &[GradingBandResponse],
) -> Result<CalculatedSubject> {
    if marks.is_empty() || marks.iter().any(|mark| mark.mark_status == "unmarked") {
        return Ok(CalculatedSubject {
            teaching_assignment_id,
            subject_id,
            subject_name: subject_name.to_string(),
            status: "incomplete",
            percentage_basis_points: None,
            band: None,
            scored_count: 0,
            absent_count: 0,
            exempt_count: 0,
        });
    }
    let mut weighted_score = 0_i64;
    let mut included_weight = 0_i64;
    let mut scored_count = 0_i32;
    let mut absent_count = 0_i32;
    let mut exempt_count = 0_i32;
    for mark in marks {
        match mark.mark_status.as_str() {
            "scored" => {
                let awarded = mark
                    .marks_awarded_hundredths
                    .context("A scored published mark has no value")?;
                let component_percentage = round_ratio(
                    awarded
                        .checked_mul(10_000)
                        .context("Mark calculation overflowed")?,
                    i64::from(mark.maximum_marks)
                        .checked_mul(100)
                        .context("Assessment maximum overflowed")?,
                )?;
                weighted_score = weighted_score
                    .checked_add(
                        i64::from(component_percentage)
                            .checked_mul(i64::from(mark.weight_basis_points))
                            .context("Subject result overflowed")?,
                    )
                    .context("Subject result overflowed")?;
                included_weight += i64::from(mark.weight_basis_points);
                scored_count += 1;
            }
            "absent" => {
                included_weight += i64::from(mark.weight_basis_points);
                absent_count += 1;
            }
            "exempt" => exempt_count += 1,
            _ => bail!("A published Gradebook mark has an unsupported status"),
        }
    }
    if included_weight == 0 {
        return Ok(CalculatedSubject {
            teaching_assignment_id,
            subject_id,
            subject_name: subject_name.to_string(),
            status: "exempt",
            percentage_basis_points: None,
            band: None,
            scored_count,
            absent_count,
            exempt_count,
        });
    }
    let percentage = round_ratio(weighted_score, included_weight)?;
    let band = band_for_score(percentage, bands)?;
    Ok(CalculatedSubject {
        teaching_assignment_id,
        subject_id,
        subject_name: subject_name.to_string(),
        status: "graded",
        percentage_basis_points: Some(percentage),
        band: Some(band),
        scored_count,
        absent_count,
        exempt_count,
    })
}

fn calculate_attendance(
    summary: Option<&AttendanceLearnerSummary>,
) -> Result<CalculatedAttendance> {
    let Some(summary) = summary else {
        return Ok(CalculatedAttendance {
            present_count: 0,
            absent_count: 0,
            late_count: 0,
            excused_count: 0,
            percentage_basis_points: None,
        });
    };
    let attended = summary.present_count + summary.late_count;
    let expected = attended + summary.absent_count;
    let percentage = if expected == 0 {
        None
    } else {
        Some(round_ratio(attended * 10_000, expected)?)
    };
    Ok(CalculatedAttendance {
        present_count: i32::try_from(summary.present_count)
            .context("Attendance total is too large")?,
        absent_count: i32::try_from(summary.absent_count)
            .context("Attendance total is too large")?,
        late_count: i32::try_from(summary.late_count).context("Attendance total is too large")?,
        excused_count: i32::try_from(summary.excused_count)
            .context("Attendance total is too large")?,
        percentage_basis_points: percentage,
    })
}

fn band_for_score(score: i16, bands: &[GradingBandResponse]) -> Result<CalculatedBand> {
    let band = bands
        .iter()
        .filter(|band| band.minimum_basis_points <= score)
        .max_by_key(|band| band.minimum_basis_points)
        .context("The grading scheme does not cover this score")?;
    Ok(CalculatedBand {
        code: band.code.clone(),
        label: band.label.clone(),
        is_pass: band.is_pass,
    })
}

fn round_ratio(numerator: i64, denominator: i64) -> Result<i16> {
    if denominator <= 0 || numerator < 0 {
        bail!("Academic result calculation received invalid values");
    }
    let rounded = numerator
        .checked_add(denominator / 2)
        .context("Academic result calculation overflowed")?
        / denominator;
    i16::try_from(rounded.clamp(0, 10_000))
        .context("Academic result is outside its basis-point range")
}

async fn insert_calculated_card(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    batch_id: Uuid,
    card: &CalculatedCard,
) -> Result<()> {
    let card_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO academic_report_cards (
            tenant_id, report_batch_id, enrolment_id, learner_id,
            learner_number_snapshot, learner_name_snapshot,
            overall_percentage_basis_points, overall_grade_code, overall_grade_label
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id
        "#,
    )
    .bind(tenant_id)
    .bind(batch_id)
    .bind(card.enrolment_id)
    .bind(card.learner_id)
    .bind(&card.learner_number)
    .bind(&card.learner_name)
    .bind(card.overall_percentage_basis_points)
    .bind(card.overall_band.as_ref().map(|band| band.code.as_str()))
    .bind(card.overall_band.as_ref().map(|band| band.label.as_str()))
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to insert calculated report card")?;
    for subject in &card.subjects {
        sqlx::query(
            r#"
            INSERT INTO academic_report_subject_results (
                tenant_id, report_card_id, teaching_assignment_id, subject_id,
                subject_name_snapshot,
                result_status, percentage_basis_points, grade_code, grade_label,
                is_pass, scored_component_count, absent_component_count,
                exempt_component_count
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
        )
        .bind(tenant_id)
        .bind(card_id)
        .bind(subject.teaching_assignment_id)
        .bind(subject.subject_id)
        .bind(&subject.subject_name)
        .bind(subject.status)
        .bind(subject.percentage_basis_points)
        .bind(subject.band.as_ref().map(|band| band.code.as_str()))
        .bind(subject.band.as_ref().map(|band| band.label.as_str()))
        .bind(subject.band.as_ref().map(|band| band.is_pass))
        .bind(subject.scored_count)
        .bind(subject.absent_count)
        .bind(subject.exempt_count)
        .execute(&mut **transaction)
        .await
        .context("Failed to insert calculated subject result")?;
    }
    sqlx::query(
        r#"
        INSERT INTO academic_report_attendance (
            tenant_id, report_card_id, present_count, absent_count,
            late_count, excused_count, attendance_percentage_basis_points
        ) VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(tenant_id)
    .bind(card_id)
    .bind(card.attendance.present_count)
    .bind(card.attendance.absent_count)
    .bind(card.attendance.late_count)
    .bind(card.attendance.excused_count)
    .bind(card.attendance.percentage_basis_points)
    .execute(&mut **transaction)
    .await
    .context("Failed to insert attendance report snapshot")?;
    Ok(())
}

async fn report_cards_for_batch(
    pool: &PgPool,
    tenant_id: Uuid,
    batch_id: Uuid,
    learner_ids: Option<&[Uuid]>,
) -> Result<Vec<AcademicReportCardResponse>> {
    let rows = sqlx::query_as::<_, ReportCardRow>(
        r#"
        SELECT card.id, card.enrolment_id, card.learner_id,
               card.learner_number_snapshot, card.learner_name_snapshot,
               card.overall_percentage_basis_points,
               card.overall_grade_code, card.overall_grade_label,
               card.teacher_comment, card.reviewer_comment,
               card.progression_outcome, card.target_grade_level_id,
               target_grade.name AS target_grade_level_name,
               card.version
          FROM academic_report_cards AS card
          LEFT JOIN academic_grade_levels AS target_grade
            ON target_grade.id = card.target_grade_level_id
           AND target_grade.tenant_id = card.tenant_id
         WHERE card.tenant_id = $1 AND card.report_batch_id = $2
           AND card.deleted_at IS NULL
           AND ($3::UUID[] IS NULL OR card.learner_id = ANY($3))
         ORDER BY card.created_at, card.id
        "#,
    )
    .bind(tenant_id)
    .bind(batch_id)
    .bind(learner_ids)
    .fetch_all(pool)
    .await
    .context("Failed to load academic report cards")?;
    let mut cards = Vec::with_capacity(rows.len());
    for row in rows {
        cards.push(AcademicReportCardResponse {
            id: row.id,
            enrolment_id: row.enrolment_id,
            learner_id: row.learner_id,
            learner_number: row.learner_number_snapshot,
            learner_name: row.learner_name_snapshot,
            overall_percentage_basis_points: row.overall_percentage_basis_points,
            overall_grade_code: row.overall_grade_code,
            overall_grade_label: row.overall_grade_label,
            teacher_comment: row.teacher_comment,
            reviewer_comment: row.reviewer_comment,
            progression_outcome: row.progression_outcome,
            target_grade_level_id: row.target_grade_level_id,
            target_grade_level_name: row.target_grade_level_name,
            version: row.version,
            subjects: subject_results(pool, tenant_id, row.id).await?,
            attendance: attendance_snapshot(pool, tenant_id, row.id).await?,
        });
    }
    cards.sort_by(|left, right| {
        left.learner_name
            .cmp(&right.learner_name)
            .then(left.learner_number.cmp(&right.learner_number))
    });
    Ok(cards)
}

async fn subject_results(
    pool: &PgPool,
    tenant_id: Uuid,
    card_id: Uuid,
) -> Result<Vec<AcademicSubjectResultResponse>> {
    sqlx::query_as::<_, SubjectResultRow>(
        r#"
        SELECT result.id, result.teaching_assignment_id, result.subject_id,
               result.subject_name_snapshot AS subject_name, result.result_status,
               result.percentage_basis_points, result.grade_code,
               result.grade_label, result.is_pass,
               result.scored_component_count, result.absent_component_count,
               result.exempt_component_count
          FROM academic_report_subject_results AS result
         WHERE result.tenant_id = $1 AND result.report_card_id = $2
           AND result.deleted_at IS NULL
         ORDER BY result.subject_name_snapshot, result.id
        "#,
    )
    .bind(tenant_id)
    .bind(card_id)
    .fetch_all(pool)
    .await
    .context("Failed to load academic subject results")
    .map(|rows| {
        rows.into_iter()
            .map(|row| AcademicSubjectResultResponse {
                id: row.id,
                teaching_assignment_id: row.teaching_assignment_id,
                subject_id: row.subject_id,
                subject_name: row.subject_name,
                result_status: row.result_status,
                percentage_basis_points: row.percentage_basis_points,
                grade_code: row.grade_code,
                grade_label: row.grade_label,
                is_pass: row.is_pass,
                scored_component_count: row.scored_component_count,
                absent_component_count: row.absent_component_count,
                exempt_component_count: row.exempt_component_count,
            })
            .collect()
    })
}

async fn attendance_snapshot(
    pool: &PgPool,
    tenant_id: Uuid,
    card_id: Uuid,
) -> Result<AcademicAttendanceResponse> {
    let row = sqlx::query_as::<_, AttendanceSnapshotRow>(
        r#"
        SELECT present_count, absent_count, late_count, excused_count,
               attendance_percentage_basis_points
          FROM academic_report_attendance
         WHERE tenant_id = $1 AND report_card_id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(card_id)
    .fetch_one(pool)
    .await
    .context("Failed to load attendance report snapshot")?;
    Ok(AcademicAttendanceResponse {
        present_count: row.present_count,
        absent_count: row.absent_count,
        late_count: row.late_count,
        excused_count: row.excused_count,
        attendance_percentage_basis_points: row.attendance_percentage_basis_points,
    })
}

fn batch_summary_query(predicate: &str) -> String {
    format!(
        r#"
        SELECT batch.id, batch.assessment_cycle_id,
               cycle.name AS assessment_cycle_name,
               term.id AS academic_term_id, term.name AS academic_term_name,
               academic_year.id AS academic_year_id,
               academic_year.name AS academic_year_name,
               batch.class_group_id, class_group.name AS class_group_name,
               batch.grading_scheme_id,
               batch.grading_scheme_name_snapshot AS grading_scheme_name,
               batch.grading_scheme_version, batch.status, batch.version,
               COUNT(DISTINCT card.id) AS learner_count,
               COUNT(DISTINCT result.id) FILTER (WHERE result.result_status = 'graded')
                   AS graded_subject_count,
               COUNT(DISTINCT result.id) FILTER (WHERE result.result_status = 'incomplete')
                   AS incomplete_subject_count,
               batch.created_at, batch.reviewed_at, batch.published_at,
               batch.reopened_at, batch.reopen_reason, batch.source_fingerprint
          FROM academic_report_batches AS batch
          JOIN assessment_cycles AS cycle
            ON cycle.id = batch.assessment_cycle_id AND cycle.tenant_id = batch.tenant_id
          JOIN academic_terms AS term
            ON term.id = cycle.academic_term_id AND term.tenant_id = cycle.tenant_id
          JOIN academic_years AS academic_year
            ON academic_year.id = term.academic_year_id AND academic_year.tenant_id = term.tenant_id
          JOIN class_groups AS class_group
            ON class_group.id = batch.class_group_id AND class_group.tenant_id = batch.tenant_id
          LEFT JOIN academic_report_cards AS card
            ON card.report_batch_id = batch.id AND card.tenant_id = batch.tenant_id
           AND card.deleted_at IS NULL
          LEFT JOIN academic_report_subject_results AS result
            ON result.report_card_id = card.id AND result.tenant_id = card.tenant_id
           AND result.deleted_at IS NULL
         WHERE {predicate}
         GROUP BY batch.id, cycle.name, term.id, term.name,
                  academic_year.id, academic_year.name, class_group.id, class_group.name
        "#,
    )
}

async fn report_batch_row(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<ReportBatchSummaryRow>> {
    sqlx::query_as::<_, ReportBatchSummaryRow>(&batch_summary_query(
        "batch.tenant_id = $1 AND batch.id = $2 AND batch.deleted_at IS NULL",
    ))
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("Failed to load academic report batch")
}

fn summary_response(row: ReportBatchSummaryRow) -> AcademicReportBatchSummary {
    AcademicReportBatchSummary {
        id: row.id,
        assessment_cycle_id: row.assessment_cycle_id,
        assessment_cycle_name: row.assessment_cycle_name,
        academic_term_id: row.academic_term_id,
        academic_term_name: row.academic_term_name,
        academic_year_id: row.academic_year_id,
        academic_year_name: row.academic_year_name,
        class_group_id: row.class_group_id,
        class_group_name: row.class_group_name,
        grading_scheme_id: row.grading_scheme_id,
        grading_scheme_name: row.grading_scheme_name,
        grading_scheme_version: row.grading_scheme_version,
        status: row.status,
        version: row.version,
        learner_count: row.learner_count,
        graded_subject_count: row.graded_subject_count,
        incomplete_subject_count: row.incomplete_subject_count,
        created_at: row.created_at,
        reviewed_at: row.reviewed_at,
        published_at: row.published_at,
    }
}

async fn hydrate_scheme(
    pool: &PgPool,
    tenant_id: Uuid,
    row: GradingSchemeRow,
) -> Result<GradingSchemeResponse> {
    let bands = sqlx::query_as::<_, GradingBandRow>(
        r#"
        SELECT id, code, label, minimum_basis_points, is_pass
          FROM academic_grading_bands
         WHERE tenant_id = $1 AND grading_scheme_id = $2 AND deleted_at IS NULL
         ORDER BY minimum_basis_points DESC, code
        "#,
    )
    .bind(tenant_id)
    .bind(row.id)
    .fetch_all(pool)
    .await
    .context("Failed to load academic grading bands")?
    .into_iter()
    .map(|band| GradingBandResponse {
        id: band.id,
        code: band.code,
        label: band.label,
        minimum_basis_points: band.minimum_basis_points,
        is_pass: band.is_pass,
    })
    .collect();
    Ok(GradingSchemeResponse {
        id: row.id,
        name: row.name,
        description: row.description,
        is_default: row.is_default,
        status: row.status,
        version: row.version,
        bands,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn normalized_bands(input: &[GradingBandInput]) -> Result<Vec<GradingBandInput>> {
    let mut bands = input
        .iter()
        .map(|band| {
            Ok(GradingBandInput {
                code: trimmed_required(&band.code, "Grade code")?.to_uppercase(),
                label: trimmed_required(&band.label, "Grade label")?.to_string(),
                minimum_basis_points: band.minimum_basis_points,
                is_pass: band.is_pass,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    bands.sort_by_key(|band| band.minimum_basis_points);
    if bands
        .first()
        .is_none_or(|band| band.minimum_basis_points != 0)
    {
        bail!("The grading scheme must start at 0%");
    }
    let unique_minimums = bands
        .iter()
        .map(|band| band.minimum_basis_points)
        .collect::<BTreeSet<_>>();
    let unique_codes = bands
        .iter()
        .map(|band| band.code.to_lowercase())
        .collect::<BTreeSet<_>>();
    if unique_minimums.len() != bands.len() || unique_codes.len() != bands.len() {
        bail!("Grade codes and minimum percentages must be unique");
    }
    Ok(bands)
}

async fn insert_bands(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    scheme_id: Uuid,
    bands: &[GradingBandInput],
) -> Result<()> {
    for band in bands {
        sqlx::query(
            r#"
            INSERT INTO academic_grading_bands (
                tenant_id, grading_scheme_id, code, label,
                minimum_basis_points, is_pass
            ) VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(tenant_id)
        .bind(scheme_id)
        .bind(&band.code)
        .bind(&band.label)
        .bind(band.minimum_basis_points)
        .bind(band.is_pass)
        .execute(&mut **transaction)
        .await
        .context("Failed to insert academic grading band")?;
    }
    Ok(())
}

async fn clear_default_scheme(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor_id: Uuid,
    excluded_scheme_id: Option<Uuid>,
) -> Result<()> {
    sqlx::query(
        "UPDATE academic_grading_schemes SET is_default = FALSE, updated_by = $2, version = version + 1, updated_at = NOW() WHERE tenant_id = $1 AND is_default = TRUE AND deleted_at IS NULL AND ($3::UUID IS NULL OR id <> $3)",
    )
    .bind(tenant_id)
    .bind(actor_id)
    .bind(excluded_scheme_id)
    .execute(&mut **transaction)
    .await
    .context("Failed to replace the default grading scheme")?;
    Ok(())
}

async fn scheme_exists(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<bool> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM academic_grading_schemes WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to check grading scheme")
}

async fn report_exists(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<bool> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM academic_report_batches WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to check academic report")
}

async fn batch_for_card(pool: &PgPool, tenant_id: Uuid, card_id: Uuid) -> Result<Option<Uuid>> {
    sqlx::query_scalar::<_, Uuid>(
        "SELECT report_batch_id FROM academic_report_cards WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .bind(card_id)
    .fetch_optional(pool)
    .await
    .context("Failed to resolve academic report card")
}

async fn current_batch_version(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    batch_id: Uuid,
) -> Result<i32> {
    sqlx::query_scalar::<_, i32>(
        "SELECT version FROM academic_report_batches WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .bind(batch_id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to load academic report version")
}

async fn report_by_idempotency(
    pool: &PgPool,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> Result<Option<ReportBatchSummaryRow>> {
    sqlx::query_as::<_, ReportBatchSummaryRow>(&batch_summary_query(
        "batch.tenant_id = $1 AND batch.idempotency_key = $2",
    ))
    .bind(tenant_id)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await
    .context("Failed to resolve academic report idempotency")
}

async fn can_access_batch(
    pool: &PgPool,
    tenant_id: Uuid,
    batch_id: Uuid,
    scope: AcademicReportingAccessScope,
) -> Result<bool> {
    let (kind, person_id) = scope_parts(scope);
    sqlx::query_scalar::<_, bool>("SELECT reporting_scope_allows($1, $2, $3, $4)")
        .bind(tenant_id)
        .bind(batch_id)
        .bind(kind)
        .bind(person_id)
        .fetch_one(pool)
        .await
        .context("Failed to evaluate academic report scope")
}

async fn can_access_learner_transcript(
    pool: &PgPool,
    tenant_id: Uuid,
    learner_id: Uuid,
    scope: AcademicReportingAccessScope,
) -> Result<bool> {
    match scope {
        AcademicReportingAccessScope::Campus => Ok(true),
        AcademicReportingAccessScope::SelfFor(user_id) => Ok(
            EnrolmentOps::learner_ids_for_account(pool, tenant_id, user_id)
                .await?
                .contains(&learner_id),
        ),
        AcademicReportingAccessScope::AssignedTo(user_id) => {
            assigned_transcript_access(pool, tenant_id, learner_id, user_id).await
        }
        AcademicReportingAccessScope::SelfAndAssigned(user_id) => {
            let self_access = EnrolmentOps::learner_ids_for_account(pool, tenant_id, user_id)
                .await?
                .contains(&learner_id);
            if self_access {
                Ok(true)
            } else {
                assigned_transcript_access(pool, tenant_id, learner_id, user_id).await
            }
        }
    }
}

async fn assigned_transcript_access(
    pool: &PgPool,
    tenant_id: Uuid,
    learner_id: Uuid,
    user_id: Uuid,
) -> Result<bool> {
    sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1
              FROM academic_report_cards AS card
              JOIN academic_report_batches AS batch
                ON batch.id = card.report_batch_id AND batch.tenant_id = card.tenant_id
               AND batch.status = 'published' AND batch.deleted_at IS NULL
              JOIN assessment_components AS component
                ON component.assessment_cycle_id = batch.assessment_cycle_id
               AND component.tenant_id = batch.tenant_id
               AND component.deleted_at IS NULL
              JOIN teaching_assignments AS assignment
                ON assignment.id = component.teaching_assignment_id
               AND assignment.tenant_id = component.tenant_id
               AND assignment.class_group_id = batch.class_group_id
               AND assignment.deleted_at IS NULL
              JOIN teacher_profiles AS teacher
                ON teacher.id = assignment.teacher_profile_id
               AND teacher.tenant_id = assignment.tenant_id
               AND teacher.deleted_at IS NULL
              JOIN employees AS employee
                ON employee.id = teacher.employee_id
               AND employee.tenant_id = teacher.tenant_id
               AND employee.account_id = $3
               AND employee.deleted_at IS NULL
             WHERE card.tenant_id = $1 AND card.learner_id = $2
               AND card.deleted_at IS NULL
        )
        "#,
    )
    .bind(tenant_id)
    .bind(learner_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Failed to evaluate assigned transcript scope")
}

async fn self_learner_ids(
    pool: &PgPool,
    tenant_id: Uuid,
    scope: AcademicReportingAccessScope,
) -> Result<Option<Vec<Uuid>>> {
    match scope {
        AcademicReportingAccessScope::SelfFor(user_id) => Ok(Some(
            EnrolmentOps::learner_ids_for_account(pool, tenant_id, user_id).await?,
        )),
        AcademicReportingAccessScope::SelfAndAssigned(_) => Ok(None),
        AcademicReportingAccessScope::Campus | AcademicReportingAccessScope::AssignedTo(_) => {
            Ok(None)
        }
    }
}

fn scope_parts(scope: AcademicReportingAccessScope) -> (&'static str, Option<Uuid>) {
    match scope {
        AcademicReportingAccessScope::Campus => ("campus", None),
        AcademicReportingAccessScope::AssignedTo(user_id) => ("assigned", Some(user_id)),
        AcademicReportingAccessScope::SelfFor(user_id) => ("self", Some(user_id)),
        AcademicReportingAccessScope::SelfAndAssigned(user_id) => {
            ("self_and_assigned", Some(user_id))
        }
    }
}

fn source_visible(source: &GradebookReportingSource, scope: AcademicReportingAccessScope) -> bool {
    match scope {
        AcademicReportingAccessScope::Campus => true,
        AcademicReportingAccessScope::AssignedTo(user_id)
        | AcademicReportingAccessScope::SelfAndAssigned(user_id) => {
            source.teacher_account_ids.contains(&user_id)
        }
        AcademicReportingAccessScope::SelfFor(_) => false,
    }
}

async fn current_source_fingerprint(
    pool: &PgPool,
    tenant_id: Uuid,
    batch: &ReportBatchSummaryRow,
) -> Result<String> {
    let marks = GradebookOps::published_results_for_cycle_class(
        pool,
        tenant_id,
        batch.assessment_cycle_id,
        batch.class_group_id,
    )
    .await?;
    let term = sqlx::query_as::<_, (chrono::NaiveDate, chrono::NaiveDate)>(
        "SELECT starts_on, ends_on FROM academic_terms WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .bind(batch.academic_term_id)
    .fetch_one(pool)
    .await
    .context("Failed to load report term boundary")?;
    let roster = EnrolmentOps::class_roster_on(
        pool,
        tenant_id,
        batch.academic_year_id,
        batch.class_group_id,
        term.1,
    )
    .await?;
    let attendance = AttendanceOps::submitted_summaries_for_class(
        pool,
        tenant_id,
        batch.class_group_id,
        term.0,
        term.1,
    )
    .await?;
    Ok(source_fingerprint(&marks, &attendance, &roster))
}

fn source_fingerprint(
    marks: &[PublishedAssessmentMark],
    attendance: &[AttendanceLearnerSummary],
    roster: &[ClassRosterEntry],
) -> String {
    let mut mark_parts = marks
        .iter()
        .map(|mark| {
            format!(
                "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
                mark.mark_sheet_id,
                mark.mark_sheet_version,
                mark.assessment_component_id,
                mark.subject_id,
                mark.subject_name,
                mark.enrolment_id,
                mark.learner_id,
                mark.mark_status,
                mark.marks_awarded_hundredths
                    .map_or_else(|| "null".to_string(), |value| value.to_string()),
                mark.maximum_marks,
                mark.weight_basis_points
            )
        })
        .collect::<Vec<_>>();
    mark_parts.sort_unstable();
    let mut attendance_parts = attendance
        .iter()
        .map(|summary| {
            format!(
                "{}:{}:{}:{}:{}:{}",
                summary.enrolment_id,
                summary.learner_id,
                summary.present_count,
                summary.absent_count,
                summary.late_count,
                summary.excused_count
            )
        })
        .collect::<Vec<_>>();
    attendance_parts.sort_unstable();
    let mut roster_parts = roster
        .iter()
        .map(|entry| {
            format!(
                "{}:{}:{}:{}",
                entry.enrolment_id, entry.learner_id, entry.learner_number, entry.display_name
            )
        })
        .collect::<Vec<_>>();
    roster_parts.sort_unstable();
    let canonical = format!(
        "marks={}|attendance={}|roster={}",
        mark_parts.join(","),
        attendance_parts.join(","),
        roster_parts.join(",")
    );
    format!("{:x}", Sha256::digest(canonical.as_bytes()))
}

#[allow(
    clippy::too_many_arguments,
    reason = "event evidence is intentionally explicit"
)]
async fn append_report_event(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    batch_id: Uuid,
    event_type: &str,
    from_status: Option<&str>,
    to_status: &str,
    version: i32,
    actor_id: Uuid,
    reason: Option<&str>,
    metadata: Value,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO academic_report_events (
            tenant_id, report_batch_id, event_type, from_status,
            to_status, report_batch_version, actor_id, reason, metadata
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(tenant_id)
    .bind(batch_id)
    .bind(event_type)
    .bind(from_status)
    .bind(to_status)
    .bind(version)
    .bind(actor_id)
    .bind(reason)
    .bind(metadata)
    .execute(&mut **transaction)
    .await
    .context("Failed to append academic report event")?;
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "audit evidence is intentionally explicit"
)]
async fn append_reporting_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    action: &str,
    target_kind: &str,
    target_id: Uuid,
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
        .with_target(AuditTarget::new(target_kind, target_id.to_string()))
        .with_redacted_metadata(metadata),
    )
    .await
    .context("Failed to append academic reporting audit event")?;
    Ok(())
}

fn person_actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .context("Academic reporting requires an authenticated person actor")
}

fn trimmed_required<'a>(value: &'a str, field: &str) -> Result<&'a str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("{field} is required");
    }
    Ok(trimmed)
}

fn optional_text(value: Option<&str>) -> Option<String> {
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

fn database_error(error: sqlx::Error, context: &str) -> anyhow::Error {
    if let sqlx::Error::Database(database) = &error {
        match database.constraint() {
            Some("idx_academic_grading_schemes_name") => {
                return anyhow!("A grading scheme with this name already exists");
            }
            Some("idx_academic_report_batches_source") => {
                return anyhow!("An academic report already exists for this cycle and class");
            }
            Some("idx_academic_report_batches_idempotency") => {
                return anyhow!("This idempotency key was already used");
            }
            _ => {}
        }
    }
    anyhow!("{context}: {error}")
}

#[cfg(test)]
mod tests {
    use cp_attendance::AttendanceLearnerSummary;
    use cp_gradebook::PublishedAssessmentMark;
    use cp_sis::models::ClassRosterEntry;
    use uuid::Uuid;

    use super::{
        AcademicReportingAccessScope, CalculatedBand, GradingBandResponse, band_for_score,
        calculate_report_cards, calculate_subject, round_ratio, source_fingerprint, source_visible,
    };
    use chrono::NaiveDate;
    use cp_gradebook::GradebookReportingSource;

    fn bands() -> Vec<GradingBandResponse> {
        vec![
            GradingBandResponse {
                id: Uuid::new_v4(),
                code: "F".to_string(),
                label: "Not achieved".to_string(),
                minimum_basis_points: 0,
                is_pass: false,
            },
            GradingBandResponse {
                id: Uuid::new_v4(),
                code: "P".to_string(),
                label: "Achieved".to_string(),
                minimum_basis_points: 5_000,
                is_pass: true,
            },
        ]
    }

    fn mark(status: &str, awarded: Option<i64>, weight: i16) -> PublishedAssessmentMark {
        PublishedAssessmentMark {
            mark_sheet_id: Uuid::new_v4(),
            mark_sheet_version: 1,
            assessment_component_id: Uuid::new_v4(),
            teaching_assignment_id: Uuid::from_u128(1),
            subject_id: Uuid::from_u128(2),
            subject_name: "Mathematics".to_string(),
            class_group_id: Uuid::from_u128(3),
            enrolment_id: Uuid::from_u128(4),
            learner_id: Uuid::from_u128(5),
            mark_status: status.to_string(),
            marks_awarded_hundredths: awarded,
            maximum_marks: 100,
            weight_basis_points: weight,
        }
    }

    fn identity(enrolment_id: u128, learner_id: u128, learner_number: &str) -> ClassRosterEntry {
        ClassRosterEntry {
            enrolment_id: Uuid::from_u128(enrolment_id),
            learner_id: Uuid::from_u128(learner_id),
            learner_number: learner_number.to_string(),
            display_name: format!("Learner {learner_number}"),
        }
    }

    #[test]
    fn half_values_round_up_in_basis_points() {
        assert_eq!(round_ratio(5, 2).unwrap_or_default(), 3);
    }

    #[test]
    fn grade_band_uses_highest_applicable_minimum() {
        let CalculatedBand { code, is_pass, .. } =
            band_for_score(5_000, &bands()).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(code, "P");
        assert!(is_pass);
    }

    #[test]
    fn exempt_components_are_removed_from_subject_denominator() {
        let scored = mark("scored", Some(6_000), 5_000);
        let exempt = mark("exempt", None, 5_000);
        let result = calculate_subject(
            scored.teaching_assignment_id,
            scored.subject_id,
            &scored.subject_name,
            &[&scored, &exempt],
            &bands(),
        )
        .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(result.percentage_basis_points, Some(6_000));
        assert_eq!(result.exempt_count, 1);
    }

    #[test]
    fn absence_contributes_zero_with_its_weight() {
        let scored = mark("scored", Some(10_000), 5_000);
        let absent = mark("absent", None, 5_000);
        let result = calculate_subject(
            scored.teaching_assignment_id,
            scored.subject_id,
            &scored.subject_name,
            &[&scored, &absent],
            &bands(),
        )
        .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(result.percentage_basis_points, Some(5_000));
        assert_eq!(result.absent_count, 1);
    }

    #[test]
    fn learner_without_assignment_marks_is_incomplete() {
        let roster = vec![identity(44, 55, "L-2")];
        let identities = vec![identity(4, 5, "L-1"), identity(44, 55, "L-2")];
        let result = calculate_report_cards(
            &[mark("scored", Some(8_000), 10_000)],
            &[],
            &roster,
            &identities,
            &bands(),
        )
        .unwrap_or_else(|error| panic!("{error}"));
        assert!(result.iter().any(|card| {
            card.learner_id == Uuid::from_u128(55)
                && card
                    .subjects
                    .iter()
                    .any(|subject| subject.status == "incomplete")
        }));
    }

    #[test]
    fn excused_attendance_is_excluded_from_rate_denominator() {
        let attendance = vec![AttendanceLearnerSummary {
            enrolment_id: Uuid::from_u128(4),
            learner_id: Uuid::from_u128(5),
            present_count: 8,
            absent_count: 2,
            late_count: 0,
            excused_count: 5,
        }];
        let result = calculate_report_cards(
            &[mark("scored", Some(8_000), 10_000)],
            &attendance,
            &[],
            &[identity(4, 5, "L-1")],
            &bands(),
        )
        .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(result[0].attendance.percentage_basis_points, Some(8_000));
    }

    #[test]
    fn source_fingerprint_changes_with_attendance() {
        let marks = vec![mark("scored", Some(8_000), 10_000)];
        let first = source_fingerprint(&marks, &[], &[]);
        let attendance = vec![AttendanceLearnerSummary {
            enrolment_id: Uuid::from_u128(4),
            learner_id: Uuid::from_u128(5),
            present_count: 1,
            absent_count: 0,
            late_count: 0,
            excused_count: 0,
        }];
        assert_ne!(first, source_fingerprint(&marks, &attendance, &[]));
    }

    #[test]
    fn assigned_source_is_bound_to_teacher_account() {
        let user_id = Uuid::new_v4();
        let source = GradebookReportingSource {
            assessment_cycle_id: Uuid::new_v4(),
            assessment_cycle_name: "Cycle".to_string(),
            academic_term_id: Uuid::new_v4(),
            academic_term_name: "Term".to_string(),
            academic_term_starts_on: NaiveDate::from_ymd_opt(2026, 1, 1)
                .unwrap_or_else(|| unreachable!()),
            academic_term_ends_on: NaiveDate::from_ymd_opt(2026, 3, 31)
                .unwrap_or_else(|| unreachable!()),
            academic_year_id: Uuid::new_v4(),
            academic_year_name: "2026".to_string(),
            class_group_id: Uuid::new_v4(),
            class_group_name: "Class".to_string(),
            component_count: 1,
            published_sheet_count: 1,
            teacher_account_ids: vec![user_id],
        };
        assert!(source_visible(
            &source,
            AcademicReportingAccessScope::AssignedTo(user_id)
        ));
        assert!(!source_visible(
            &source,
            AcademicReportingAccessScope::AssignedTo(Uuid::new_v4())
        ));
    }
}
