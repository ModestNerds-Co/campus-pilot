//! Transactional E-learning space, unit, and governed-resource operations.
//!
//! Every query applies the caller's current campus, assigned-teacher, or
//! learner-self scope before projection. Published records are immutable and
//! move only to an explicit retained terminal state.

use std::collections::BTreeSet;

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use cp_academics::ops::{AcademicTermOps, AcademicYearOps, TeachingAssignmentOps};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_document_registry::{DocumentRegistryOps, EvidenceFileReference};
use cp_sis::{models::ClassRosterEntry, ops::EnrolmentOps};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::dtos::{
    CreateLearningAssignmentRequest, CreateLearningResourceRequest,
    CreateLearningRubricCriterionRequest, CreateLearningSpaceRequest, CreateLearningUnitRequest,
    DeleteLearningRubricCriterionRequest, LearningAccessScope, LearningAssignmentListQuery,
    LearningAssignmentReference, LearningAssignmentResponse, LearningAssignmentStatus,
    LearningFeedbackResponse, LearningProgressEntry, LearningReferenceData,
    LearningResourceCreation, LearningResourceFileQuery, LearningResourceResponse,
    LearningResourceStatus, LearningReviewOutcome, LearningReviewScoreResponse,
    LearningRubricCriterionResponse, LearningSettingsResponse, LearningSpaceListQuery,
    LearningSpaceResponse, LearningSpaceStatus, LearningSpaceSummary, LearningSubmissionListQuery,
    LearningSubmissionResponse, LearningSubmissionStatus, LearningSubmissionVersionResponse,
    LearningTermReference, LearningUnitResponse, LearningUnitStatus,
    ReasonedLearningTransitionRequest, ReleaseLearningFeedbackRequest,
    SaveLearningSubmissionRequest, SubmitLearningSubmissionRequest,
    UpdateLearningAssignmentRequest, UpdateLearningFeedbackRequest, UpdateLearningResourceRequest,
    UpdateLearningRubricCriterionRequest, UpdateLearningSettingsRequest,
    UpdateLearningSpaceRequest, UpdateLearningUnitRequest,
};
use crate::models::{
    LearningAssignmentRow, LearningFeedbackRow, LearningProgressRow, LearningResourceRow,
    LearningReviewScoreRow, LearningRubricCriterionRow, LearningSettingsRow, LearningSpaceRow,
    LearningSubmissionRow, LearningSubmissionVersionRow, LearningUnitRow,
};

const DEFAULT_PAGE: i64 = 1;
const DEFAULT_PER_PAGE: i64 = 25;
const MAX_PER_PAGE: i64 = 100;

/// The complete Learning application-service boundary.
pub struct LearningOps;

/// Parsed context for one governed resource link or upload.
pub(crate) struct LearningResourceCreateCommand<'a> {
    pub tenant_id: Uuid,
    pub unit_id: Uuid,
    pub scope: LearningAccessScope,
    pub actor: AuditActor,
    pub request_context: RequestContext,
    pub request: &'a CreateLearningResourceRequest,
    pub creation: LearningResourceCreation,
}

impl LearningOps {
    /// Loads the selected governed filing series, if configured.
    pub async fn settings(pool: &PgPool, tenant_id: Uuid) -> Result<LearningSettingsResponse> {
        let row = sqlx::query_as::<_, LearningSettingsRow>(
            "SELECT document_series_id,version,updated_at FROM learning_settings WHERE tenant_id=$1 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .fetch_one(pool)
        .await
        .context("Failed to load Learning settings")?;
        let document_series_name = match row.document_series_id {
            Some(id) => DocumentRegistryOps::get_series(pool, tenant_id, id, false)
                .await?
                .map(|series| series.name),
            None => None,
        };
        Ok(LearningSettingsResponse {
            document_series_id: row.document_series_id,
            document_series_name,
            version: row.version,
            updated_at: row.updated_at,
        })
    }

    /// Changes the Document Registry series used for direct Learning uploads.
    pub async fn update_settings(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateLearningSettingsRequest,
    ) -> Result<Option<LearningSettingsResponse>> {
        if let Some(series_id) = request.document_series_id {
            let series = DocumentRegistryOps::get_series(pool, tenant_id, series_id, false)
                .await?
                .ok_or_else(|| anyhow!("The selected document classification was not found"))?;
            if series.status != "active" {
                bail!("The selected document classification is inactive");
            }
        }
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning settings update")?;
        let changed = sqlx::query_scalar::<_, Uuid>(
            r#"
            UPDATE learning_settings
               SET document_series_id=$3,version=version+1,updated_by=$4
             WHERE tenant_id=$1 AND version=$2 AND deleted_at IS NULL
             RETURNING tenant_id
            "#,
        )
        .bind(tenant_id)
        .bind(request.expected_version)
        .bind(request.document_series_id)
        .bind(actor_id)
        .fetch_optional(&mut *tx)
        .await
        .context("update Learning settings")?;
        if changed.is_none() {
            return Ok(None);
        }
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "settings",
            tenant_id,
            None,
            "learning_settings_updated",
            "learning.settings.update",
            json!({"document_series_id": request.document_series_id}),
        )
        .await?;
        tx.commit().await.context("commit Learning settings")?;
        Self::settings(pool, tenant_id).await.map(Some)
    }

    /// Returns the current term and teaching assignments visible for authoring.
    pub async fn references(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: LearningAccessScope,
    ) -> Result<LearningReferenceData> {
        let active_term = if let Some(year) = AcademicYearOps::get_active(pool, tenant_id).await? {
            AcademicTermOps::get_active_for_year(pool, tenant_id, year.id)
                .await?
                .map(|term| LearningTermReference {
                    id: term.id,
                    academic_year_id: term.academic_year_id,
                    academic_year_name: term.academic_year_name,
                    code: term.code,
                    name: term.name,
                    starts_on: term.starts_on,
                    ends_on: term.ends_on,
                })
        } else {
            None
        };
        let assignments = match scope {
            LearningAccessScope::Campus => {
                TeachingAssignmentOps::list(
                    pool,
                    tenant_id,
                    1,
                    1_000,
                    Some("active"),
                    None,
                    None,
                    None,
                )
                .await?
                .0
            }
            LearningAccessScope::AssignedTo(account_id) => {
                TeachingAssignmentOps::active_for_account(pool, tenant_id, account_id).await?
            }
            LearningAccessScope::SelfFor(_) => Vec::new(),
            LearningAccessScope::SelfAndAssigned(account_id) => {
                TeachingAssignmentOps::active_for_account(pool, tenant_id, account_id).await?
            }
        };
        Ok(LearningReferenceData {
            active_term,
            assignments: assignments
                .into_iter()
                .map(|assignment| LearningAssignmentReference {
                    id: assignment.id,
                    academic_year_id: assignment.academic_year_id,
                    academic_year_name: assignment.academic_year_name,
                    class_group_id: assignment.class_group_id,
                    class_group_name: assignment.class_group_name,
                    subject_id: assignment.subject_id,
                    subject_name: assignment.subject_name,
                    teacher_name: assignment.teacher_name,
                })
                .collect(),
        })
    }

    /// Lists safe Document Registry metadata for resource linking.
    pub async fn resource_file_candidates(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &LearningResourceFileQuery,
    ) -> Result<Vec<EvidenceFileReference>> {
        DocumentRegistryOps::linkable_references(
            pool,
            tenant_id,
            query.search.as_deref(),
            query.limit.unwrap_or(50),
        )
        .await
    }

    /// Lists Learning spaces after applying current record scope before limit
    /// and offset are evaluated.
    pub async fn list_spaces(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: LearningAccessScope,
        query: &LearningSpaceListQuery,
    ) -> Result<(Vec<LearningSpaceSummary>, i64)> {
        let visibility = visibility(pool, tenant_id, scope).await?;
        let (page, per_page) = bounded_page(query.page, query.per_page);
        let search = like_query(query.search.as_deref());
        let status = query.status.map(LearningSpaceStatus::as_str);
        let rows = sqlx::query_as::<_, LearningSpaceRow>(SPACE_LIST_SQL)
            .bind(tenant_id)
            .bind(status)
            .bind(search.as_deref())
            .bind(visibility.mode)
            .bind(visibility.assignment_ids.as_deref())
            .bind(visibility.academic_year_id)
            .bind(visibility.class_group_ids.as_deref())
            .bind(per_page)
            .bind((page - 1) * per_page)
            .fetch_all(pool)
            .await
            .context("Failed to list Learning spaces")?;
        let total = sqlx::query_scalar::<_, i64>(SPACE_COUNT_SQL)
            .bind(tenant_id)
            .bind(status)
            .bind(search.as_deref())
            .bind(visibility.mode)
            .bind(visibility.assignment_ids.as_deref())
            .bind(visibility.academic_year_id)
            .bind(visibility.class_group_ids.as_deref())
            .fetch_one(pool)
            .await
            .context("Failed to count Learning spaces")?;
        let mut spaces = Vec::with_capacity(rows.len());
        for row in rows {
            spaces.push(hydrate_space_summary(pool, tenant_id, row).await?);
        }
        Ok((spaces, total))
    }

    /// Loads one visible space with ordered units and resources.
    pub async fn get_space(
        pool: &PgPool,
        tenant_id: Uuid,
        space_id: Uuid,
        scope: LearningAccessScope,
    ) -> Result<Option<LearningSpaceResponse>> {
        let Some(row) = space_row(pool, tenant_id, space_id).await? else {
            return Ok(None);
        };
        if !scope_allows_space(pool, tenant_id, &row, scope).await? {
            return Ok(None);
        }
        let published_only = published_only_for_space(pool, tenant_id, &row, scope).await?;
        hydrate_space(pool, tenant_id, row, published_only)
            .await
            .map(Some)
    }

    /// Creates a draft space from one current Academics assignment and term.
    pub async fn create_space(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateLearningSpaceRequest,
    ) -> Result<LearningSpaceResponse> {
        let actor_id = person_actor_id(actor)?;
        let assignment =
            TeachingAssignmentOps::get_by_id(pool, tenant_id, request.teaching_assignment_id)
                .await?
                .ok_or_else(|| anyhow!("The selected teaching assignment was not found"))?;
        if assignment.status != "active" {
            bail!("The selected teaching assignment is inactive");
        }
        ensure_can_author_assignment(pool, tenant_id, assignment.id, scope).await?;
        let term = AcademicTermOps::get_by_id(pool, tenant_id, request.academic_term_id)
            .await?
            .ok_or_else(|| anyhow!("The selected academic term was not found"))?;
        if assignment.academic_year_id != term.academic_year_id {
            bail!("The selected term does not belong to the assignment academic year");
        }
        if term.status == "closed" {
            bail!("A Learning space cannot be created in a closed academic term");
        }
        let title = required("Space title", &request.title)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning space creation")?;
        let space_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO learning_spaces
                (tenant_id,teaching_assignment_id,academic_year_id,academic_term_id,
                 class_group_id,title,summary,created_by,updated_by)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$8)
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(assignment.id)
        .bind(assignment.academic_year_id)
        .bind(term.id)
        .bind(assignment.class_group_id)
        .bind(title)
        .bind(optional(request.summary.as_deref()))
        .bind(actor_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| database_error(error, "create Learning space"))?;
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "space",
            space_id,
            Some(space_id),
            "learning_space_created",
            "learning.spaces.create",
            json!({"teaching_assignment_id": assignment.id, "academic_term_id": term.id}),
        )
        .await?;
        tx.commit().await.context("commit Learning space")?;
        Self::get_space(pool, tenant_id, space_id, LearningAccessScope::Campus)
            .await?
            .ok_or_else(|| anyhow!("The Learning space could not be reloaded"))
    }

    /// Updates editable draft space content with optimistic concurrency.
    pub async fn update_space(
        pool: &PgPool,
        tenant_id: Uuid,
        space_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateLearningSpaceRequest,
    ) -> Result<Option<LearningSpaceResponse>> {
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool.begin().await.context("start Learning space update")?;
        let changed = sqlx::query_scalar::<_, Uuid>(
            r#"
            UPDATE learning_spaces SET title=$4,summary=$5,version=version+1,updated_by=$6
             WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='draft'
               AND deleted_at IS NULL RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(space_id)
        .bind(request.expected_version)
        .bind(required("Space title", &request.title)?)
        .bind(optional(request.summary.as_deref()))
        .bind(actor_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| database_error(error, "update Learning space"))?;
        if changed.is_none() {
            return Ok(None);
        }
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "space",
            space_id,
            Some(space_id),
            "learning_space_updated",
            "learning.spaces.update",
            json!({"previous_version": request.expected_version}),
        )
        .await?;
        tx.commit().await.context("commit Learning space update")?;
        Self::get_space(pool, tenant_id, space_id, scope).await
    }

    /// Publishes a draft space after at least one unit and resource are ready.
    pub async fn publish_space(
        pool: &PgPool,
        tenant_id: Uuid,
        space_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<LearningSpaceResponse>> {
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning space publication")?;
        let ready = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM learning_units AS unit
                WHERE unit.tenant_id=$1 AND unit.learning_space_id=$2
                  AND unit.status='published' AND unit.deleted_at IS NULL
                  AND EXISTS (
                    SELECT 1 FROM learning_resources AS resource
                    WHERE resource.tenant_id=unit.tenant_id
                      AND resource.learning_unit_id=unit.id
                      AND resource.status='published' AND resource.deleted_at IS NULL
                  )
            )
            "#,
        )
        .bind(tenant_id)
        .bind(space_id)
        .fetch_one(&mut *tx)
        .await
        .context("validate Learning publication content")?;
        if !ready {
            bail!("Publish at least one unit with a published resource first");
        }
        let changed = sqlx::query_scalar::<_, Uuid>(
            "UPDATE learning_spaces SET status='published',published_by=$4,published_at=NOW(),version=version+1,updated_by=$4 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='draft' AND deleted_at IS NULL RETURNING id",
        )
        .bind(tenant_id)
        .bind(space_id)
        .bind(expected_version)
        .bind(actor_id)
        .fetch_optional(&mut *tx)
        .await
        .context("publish Learning space")?;
        if changed.is_none() {
            return Ok(None);
        }
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "space",
            space_id,
            Some(space_id),
            "learning_space_published",
            "learning.spaces.publish",
            json!({"previous_version": expected_version}),
        )
        .await?;
        tx.commit().await.context("commit Learning publication")?;
        Self::get_space(pool, tenant_id, space_id, scope).await
    }

    /// Archives a published space while retaining all content and evidence.
    pub async fn archive_space(
        pool: &PgPool,
        tenant_id: Uuid,
        space_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ReasonedLearningTransitionRequest,
    ) -> Result<Option<LearningSpaceResponse>> {
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let reason = required("Archive reason", &request.reason)?;
        let mut tx = pool.begin().await.context("start Learning space archive")?;
        let changed = sqlx::query_scalar::<_, Uuid>(
            "UPDATE learning_spaces SET status='archived',archived_by=$4,archived_at=NOW(),archive_reason=$5,version=version+1,updated_by=$4 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='published' AND deleted_at IS NULL RETURNING id",
        )
        .bind(tenant_id).bind(space_id).bind(request.expected_version).bind(actor_id).bind(reason)
        .fetch_optional(&mut *tx).await.context("archive Learning space")?;
        if changed.is_none() {
            return Ok(None);
        }
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "space",
            space_id,
            Some(space_id),
            "learning_space_archived",
            "learning.spaces.archive",
            json!({"reason": reason, "previous_version": request.expected_version}),
        )
        .await?;
        tx.commit().await.context("commit Learning archive")?;
        Self::get_space(pool, tenant_id, space_id, scope).await
    }

    /// Adds an ordered draft unit while its parent space is still a draft.
    pub async fn create_unit(
        pool: &PgPool,
        tenant_id: Uuid,
        space_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateLearningUnitRequest,
    ) -> Result<Option<LearningUnitResponse>> {
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool.begin().await.context("start Learning unit creation")?;
        let space_status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM learning_spaces WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL FOR UPDATE",
        ).bind(tenant_id).bind(space_id).fetch_optional(&mut *tx).await.context("lock Learning space")?;
        let Some(space_status) = space_status else {
            return Ok(None);
        };
        if space_status != "draft" {
            bail!("A published Learning space is immutable");
        }
        let unit_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO learning_units (tenant_id,learning_space_id,position,title,summary,created_by,updated_by) VALUES ($1,$2,$3,$4,$5,$6,$6) RETURNING id",
        ).bind(tenant_id).bind(space_id).bind(request.position)
         .bind(required("Unit title", &request.title)?).bind(optional(request.summary.as_deref())).bind(actor_id)
         .fetch_one(&mut *tx).await.map_err(|error| database_error(error, "create Learning unit"))?;
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "unit",
            unit_id,
            Some(space_id),
            "learning_unit_created",
            "learning.units.create",
            json!({"position": request.position}),
        )
        .await?;
        tx.commit().await.context("commit Learning unit")?;
        unit_response(pool, tenant_id, unit_id, false).await
    }

    /// Proves that the current caller may author the unit before an upload
    /// performs any external storage work.
    pub async fn authorize_unit_for_write(
        pool: &PgPool,
        tenant_id: Uuid,
        unit_id: Uuid,
        scope: LearningAccessScope,
    ) -> Result<Option<Uuid>> {
        let Some(space_id) = unit_space_id(pool, tenant_id, unit_id).await? else {
            return Ok(None);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let states = sqlx::query_as::<_, (String, String)>(
            "SELECT unit.status,space.status FROM learning_units AS unit JOIN learning_spaces AS space ON space.tenant_id=unit.tenant_id AND space.id=unit.learning_space_id WHERE unit.tenant_id=$1 AND unit.id=$2 AND unit.deleted_at IS NULL AND space.deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(unit_id)
        .fetch_optional(pool)
        .await
        .context("validate Learning unit state")?;
        let Some((unit_status, space_status)) = states else {
            return Ok(None);
        };
        if unit_status != "draft" || space_status != "draft" {
            bail!("Published Learning content is immutable");
        }
        Ok(Some(space_id))
    }

    /// Updates an ordered draft unit.
    pub async fn update_unit(
        pool: &PgPool,
        tenant_id: Uuid,
        unit_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateLearningUnitRequest,
    ) -> Result<Option<LearningUnitResponse>> {
        let Some(space_id) = unit_space_id(pool, tenant_id, unit_id).await? else {
            return Ok(None);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool.begin().await.context("start Learning unit update")?;
        require_draft_space(&mut tx, tenant_id, space_id).await?;
        let changed = sqlx::query_scalar::<_, Uuid>(
            "UPDATE learning_units SET position=$4,title=$5,summary=$6,version=version+1,updated_by=$7 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='draft' AND deleted_at IS NULL RETURNING id",
        ).bind(tenant_id).bind(unit_id).bind(request.expected_version).bind(request.position)
         .bind(required("Unit title", &request.title)?).bind(optional(request.summary.as_deref())).bind(actor_id)
         .fetch_optional(&mut *tx).await.map_err(|error| database_error(error, "update Learning unit"))?;
        if changed.is_none() {
            return Ok(None);
        }
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "unit",
            unit_id,
            Some(space_id),
            "learning_unit_updated",
            "learning.units.update",
            json!({"position": request.position}),
        )
        .await?;
        tx.commit().await.context("commit Learning unit update")?;
        unit_response(pool, tenant_id, unit_id, false).await
    }

    /// Publishes a draft unit once one governed resource is published.
    pub async fn publish_unit(
        pool: &PgPool,
        tenant_id: Uuid,
        unit_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<LearningUnitResponse>> {
        let Some(space_id) = unit_space_id(pool, tenant_id, unit_id).await? else {
            return Ok(None);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning unit publication")?;
        require_draft_space(&mut tx, tenant_id, space_id).await?;
        let resource_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM learning_resources WHERE tenant_id=$1 AND learning_unit_id=$2 AND status='published' AND deleted_at IS NULL",
        ).bind(tenant_id).bind(unit_id).fetch_one(&mut *tx).await.context("validate Learning unit resources")?;
        if resource_count == 0 {
            bail!("Publish at least one resource before publishing this unit");
        }
        let changed = sqlx::query_scalar::<_, Uuid>(
            "UPDATE learning_units SET status='published',published_by=$4,published_at=NOW(),version=version+1,updated_by=$4 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='draft' AND deleted_at IS NULL RETURNING id",
        ).bind(tenant_id).bind(unit_id).bind(expected_version).bind(actor_id)
         .fetch_optional(&mut *tx).await.context("publish Learning unit")?;
        if changed.is_none() {
            return Ok(None);
        }
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "unit",
            unit_id,
            Some(space_id),
            "learning_unit_published",
            "learning.units.publish",
            json!({"previous_version": expected_version}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit Learning unit publication")?;
        unit_response(pool, tenant_id, unit_id, false).await
    }

    /// Withdraws a published unit with a retained reason.
    pub async fn withdraw_unit(
        pool: &PgPool,
        tenant_id: Uuid,
        unit_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ReasonedLearningTransitionRequest,
    ) -> Result<Option<LearningUnitResponse>> {
        let Some(space_id) = unit_space_id(pool, tenant_id, unit_id).await? else {
            return Ok(None);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let reason = required("Withdrawal reason", &request.reason)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning unit withdrawal")?;
        let changed = sqlx::query_scalar::<_, Uuid>(
            "UPDATE learning_units SET status='withdrawn',withdrawn_by=$4,withdrawn_at=NOW(),withdrawal_reason=$5,version=version+1,updated_by=$4 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='published' AND deleted_at IS NULL RETURNING id",
        ).bind(tenant_id).bind(unit_id).bind(request.expected_version).bind(actor_id).bind(reason)
         .fetch_optional(&mut *tx).await.context("withdraw Learning unit")?;
        if changed.is_none() {
            return Ok(None);
        }
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "unit",
            unit_id,
            Some(space_id),
            "learning_unit_withdrawn",
            "learning.units.withdraw",
            json!({"reason": reason}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit Learning unit withdrawal")?;
        unit_response(pool, tenant_id, unit_id, false).await
    }

    /// Links one current, non-restricted Document Registry file as a draft resource.
    pub(crate) async fn create_resource(
        pool: &PgPool,
        command: LearningResourceCreateCommand<'_>,
    ) -> Result<Option<LearningResourceResponse>> {
        let LearningResourceCreateCommand {
            tenant_id,
            unit_id,
            scope,
            actor,
            request_context,
            request,
            creation,
        } = command;
        let Some(space_id) = unit_space_id(pool, tenant_id, unit_id).await? else {
            return Ok(None);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool.begin().await.context("start Learning resource link")?;
        require_draft_space(&mut tx, tenant_id, space_id).await?;
        let unit_status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM learning_units WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL FOR UPDATE",
        ).bind(tenant_id).bind(unit_id).fetch_optional(&mut *tx).await.context("lock Learning unit")?;
        let Some(unit_status) = unit_status else {
            return Ok(None);
        };
        if unit_status != "draft" {
            bail!("A published Learning unit is immutable");
        }
        let document = DocumentRegistryOps::evidence_reference(
            &mut *tx,
            tenant_id,
            request.document_file_id,
            false,
        )
        .await?
        .ok_or_else(|| anyhow!("The selected governed file was not found or is restricted"))?;
        let resource_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO learning_resources (tenant_id,learning_unit_id,document_file_id,display_title,sensitivity_snapshot,position,created_by,updated_by) VALUES ($1,$2,$3,$4,$5,$6,$7,$7) RETURNING id",
        ).bind(tenant_id).bind(unit_id).bind(document.id)
         .bind(required("Resource title", &request.display_title)?).bind(&document.sensitivity)
         .bind(request.position).bind(actor_id).fetch_one(&mut *tx).await
         .map_err(|error| database_error(error, "link Learning resource"))?;
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "resource",
            resource_id,
            Some(space_id),
            "learning_resource_linked",
            creation.operation_key(),
            json!({"document_file_id": document.id, "document_reference": document.reference}),
        )
        .await?;
        tx.commit().await.context("commit Learning resource link")?;
        resource_response(pool, tenant_id, resource_id).await
    }

    /// Updates a draft resource label and position.
    pub async fn update_resource(
        pool: &PgPool,
        tenant_id: Uuid,
        resource_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateLearningResourceRequest,
    ) -> Result<Option<LearningResourceResponse>> {
        let Some((_, space_id)) = resource_owner(pool, tenant_id, resource_id).await? else {
            return Ok(None);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning resource update")?;
        require_draft_space(&mut tx, tenant_id, space_id).await?;
        let unit_status = sqlx::query_scalar::<_, String>(
            "SELECT unit.status FROM learning_resources AS resource JOIN learning_units AS unit ON unit.tenant_id=resource.tenant_id AND unit.id=resource.learning_unit_id WHERE resource.tenant_id=$1 AND resource.id=$2 AND resource.deleted_at IS NULL AND unit.deleted_at IS NULL FOR UPDATE OF unit",
        )
        .bind(tenant_id)
        .bind(resource_id)
        .fetch_optional(&mut *tx)
        .await
        .context("lock Learning resource unit")?;
        if unit_status.as_deref() != Some("draft") {
            bail!("A published Learning unit is immutable");
        }
        let changed = sqlx::query_scalar::<_, Uuid>(
            "UPDATE learning_resources SET display_title=$4,position=$5,version=version+1,updated_by=$6 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='draft' AND deleted_at IS NULL RETURNING id",
        ).bind(tenant_id).bind(resource_id).bind(request.expected_version)
         .bind(required("Resource title", &request.display_title)?).bind(request.position).bind(actor_id)
         .fetch_optional(&mut *tx).await.map_err(|error| database_error(error, "update Learning resource"))?;
        if changed.is_none() {
            return Ok(None);
        }
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "resource",
            resource_id,
            Some(space_id),
            "learning_resource_updated",
            "learning.resources.update",
            json!({"position": request.position}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit Learning resource update")?;
        resource_response(pool, tenant_id, resource_id).await
    }

    /// Publishes a draft resource after re-checking its current governed file.
    pub async fn publish_resource(
        pool: &PgPool,
        tenant_id: Uuid,
        resource_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<LearningResourceResponse>> {
        let Some((_, space_id)) = resource_owner(pool, tenant_id, resource_id).await? else {
            return Ok(None);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning resource publication")?;
        require_draft_space(&mut tx, tenant_id, space_id).await?;
        let unit_status = sqlx::query_scalar::<_, String>(
            "SELECT unit.status FROM learning_resources AS resource JOIN learning_units AS unit ON unit.tenant_id=resource.tenant_id AND unit.id=resource.learning_unit_id WHERE resource.tenant_id=$1 AND resource.id=$2 AND resource.deleted_at IS NULL AND unit.deleted_at IS NULL FOR UPDATE OF unit",
        )
        .bind(tenant_id)
        .bind(resource_id)
        .fetch_optional(&mut *tx)
        .await
        .context("lock Learning resource unit")?;
        if unit_status.as_deref() != Some("draft") {
            bail!("A published Learning unit is immutable");
        }
        let document_file_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT document_file_id FROM learning_resources WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='draft' AND deleted_at IS NULL FOR UPDATE",
        ).bind(tenant_id).bind(resource_id).bind(expected_version).fetch_optional(&mut *tx).await.context("lock Learning resource")?;
        let Some(document_file_id) = document_file_id else {
            return Ok(None);
        };
        DocumentRegistryOps::evidence_reference(&mut *tx, tenant_id, document_file_id, false)
            .await?
            .ok_or_else(|| anyhow!("The governed file is no longer available for publication"))?;
        let changed = sqlx::query_scalar::<_, Uuid>(
            "UPDATE learning_resources SET status='published',published_by=$4,published_at=NOW(),version=version+1,updated_by=$4 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='draft' AND deleted_at IS NULL RETURNING id",
        ).bind(tenant_id).bind(resource_id).bind(expected_version).bind(actor_id)
         .fetch_optional(&mut *tx).await.context("publish Learning resource")?;
        if changed.is_none() {
            return Ok(None);
        }
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "resource",
            resource_id,
            Some(space_id),
            "learning_resource_published",
            "learning.resources.publish",
            json!({"previous_version": expected_version}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit Learning resource publication")?;
        resource_response(pool, tenant_id, resource_id).await
    }

    /// Withdraws a published resource while retaining its governed reference.
    pub async fn withdraw_resource(
        pool: &PgPool,
        tenant_id: Uuid,
        resource_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ReasonedLearningTransitionRequest,
    ) -> Result<Option<LearningResourceResponse>> {
        let Some((_, space_id)) = resource_owner(pool, tenant_id, resource_id).await? else {
            return Ok(None);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let reason = required("Withdrawal reason", &request.reason)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning resource withdrawal")?;
        let changed = sqlx::query_scalar::<_, Uuid>(
            "UPDATE learning_resources SET status='withdrawn',withdrawn_by=$4,withdrawn_at=NOW(),withdrawal_reason=$5,version=version+1,updated_by=$4 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='published' AND deleted_at IS NULL RETURNING id",
        ).bind(tenant_id).bind(resource_id).bind(request.expected_version).bind(actor_id).bind(reason)
         .fetch_optional(&mut *tx).await.context("withdraw Learning resource")?;
        if changed.is_none() {
            return Ok(None);
        }
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "resource",
            resource_id,
            Some(space_id),
            "learning_resource_withdrawn",
            "learning.resources.withdraw",
            json!({"reason": reason}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit Learning resource withdrawal")?;
        resource_response(pool, tenant_id, resource_id).await
    }

    /// Resolves a private object key only after Learning visibility and current
    /// Document Registry state have both been checked. Agent adapters never call this.
    pub async fn authorized_resource_object_key(
        pool: &PgPool,
        tenant_id: Uuid,
        resource_id: Uuid,
        scope: LearningAccessScope,
    ) -> Result<Option<String>> {
        let Some((unit_id, space_id)) = resource_owner(pool, tenant_id, resource_id).await? else {
            return Ok(None);
        };
        let Some(space) = space_row(pool, tenant_id, space_id).await? else {
            return Ok(None);
        };
        if !scope_allows_space(pool, tenant_id, &space, scope).await? {
            return Ok(None);
        }
        let published_only = published_only_for_space(pool, tenant_id, &space, scope).await?;
        let document_file_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT resource.document_file_id
              FROM learning_resources AS resource
              JOIN learning_units AS unit ON unit.id=resource.learning_unit_id AND unit.tenant_id=resource.tenant_id
              JOIN learning_spaces AS space ON space.id=unit.learning_space_id AND space.tenant_id=unit.tenant_id
             WHERE resource.tenant_id=$1 AND resource.id=$2 AND resource.learning_unit_id=$3
               AND resource.deleted_at IS NULL AND resource.status <> 'withdrawn'
               AND unit.deleted_at IS NULL AND unit.status <> 'withdrawn'
               AND space.deleted_at IS NULL AND space.status <> 'archived'
               AND (NOT $4 OR (resource.status='published' AND unit.status='published' AND space.status='published'))
            "#,
        ).bind(tenant_id).bind(resource_id).bind(unit_id).bind(published_only)
         .fetch_optional(pool).await.context("authorize Learning resource download")?;
        match document_file_id {
            Some(file_id) => DocumentRegistryOps::object_key(pool, tenant_id, file_id, false).await,
            None => Ok(None),
        }
    }

    /// Lists assignments under one visible Learning space.
    pub async fn list_assignments(
        pool: &PgPool,
        tenant_id: Uuid,
        space_id: Uuid,
        scope: LearningAccessScope,
        query: &LearningAssignmentListQuery,
    ) -> Result<(Vec<LearningAssignmentResponse>, i64)> {
        let Some(space) = space_row(pool, tenant_id, space_id).await? else {
            return Ok((Vec::new(), 0));
        };
        if !scope_allows_space(pool, tenant_id, &space, scope).await? {
            return Ok((Vec::new(), 0));
        }
        let published_only = published_only_for_space(pool, tenant_id, &space, scope).await?;
        let (page, per_page) = bounded_page(query.page, query.per_page);
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, LearningAssignmentRow>(&format!(
            "{ASSIGNMENT_SELECT} AND unit.learning_space_id=$2 AND ($3::TEXT IS NULL OR assignment.status=$3) AND (NOT $4 OR assignment.status <> 'draft') ORDER BY assignment.position,assignment.created_at LIMIT $5 OFFSET $6"
        ))
        .bind(tenant_id)
        .bind(space_id)
        .bind(query.status.map(LearningAssignmentStatus::as_str))
        .bind(published_only)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("list Learning assignments")?;
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM learning_assignments assignment JOIN learning_units unit ON unit.id=assignment.learning_unit_id AND unit.tenant_id=assignment.tenant_id WHERE assignment.tenant_id=$1 AND unit.learning_space_id=$2 AND assignment.deleted_at IS NULL AND unit.deleted_at IS NULL AND ($3::TEXT IS NULL OR assignment.status=$3) AND (NOT $4 OR assignment.status <> 'draft')",
        )
        .bind(tenant_id)
        .bind(space_id)
        .bind(query.status.map(LearningAssignmentStatus::as_str))
        .bind(published_only)
        .fetch_one(pool)
        .await
        .context("count Learning assignments")?;
        let mut assignments = Vec::with_capacity(rows.len());
        for row in rows {
            assignments.push(assignment_response(pool, tenant_id, row).await?);
        }
        Ok((assignments, total))
    }

    /// Reads one assignment through its parent Learning-space scope.
    pub async fn get_assignment(
        pool: &PgPool,
        tenant_id: Uuid,
        assignment_id: Uuid,
        scope: LearningAccessScope,
    ) -> Result<Option<LearningAssignmentResponse>> {
        let Some(row) = assignment_row(pool, tenant_id, assignment_id).await? else {
            return Ok(None);
        };
        let Some(space) = space_row(pool, tenant_id, row.learning_space_id).await? else {
            return Ok(None);
        };
        if !scope_allows_space(pool, tenant_id, &space, scope).await?
            || (published_only_for_space(pool, tenant_id, &space, scope).await?
                && row.status == "draft")
        {
            return Ok(None);
        }
        assignment_response(pool, tenant_id, row).await.map(Some)
    }

    /// Creates a draft assignment under an active Learning unit.
    #[allow(
        clippy::too_many_arguments,
        reason = "actor and scope evidence stay explicit"
    )]
    pub async fn create_assignment(
        pool: &PgPool,
        tenant_id: Uuid,
        unit_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateLearningAssignmentRequest,
    ) -> Result<Option<LearningAssignmentResponse>> {
        let Some(space_id) = unit_space_id(pool, tenant_id, unit_id).await? else {
            return Ok(None);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning assignment creation")?;
        require_active_assignment_parent(&mut tx, tenant_id, unit_id, space_id).await?;
        let assignment_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO learning_assignments (tenant_id,learning_unit_id,position,title,instructions,due_at,max_score_hundredths,created_by,updated_by) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$8) RETURNING id",
        )
        .bind(tenant_id)
        .bind(unit_id)
        .bind(request.position)
        .bind(required("Assignment title", &request.title)?)
        .bind(required("Assignment instructions", &request.instructions)?)
        .bind(request.due_at)
        .bind(request.max_score_hundredths)
        .bind(actor_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| database_error(error, "create Learning assignment"))?;
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "assignment",
            assignment_id,
            Some(space_id),
            "learning_assignment_created",
            "learning.assignments.create",
            json!({"unit_id": unit_id, "position": request.position}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit Learning assignment creation")?;
        assignment_response_by_id(pool, tenant_id, assignment_id).await
    }

    /// Updates an assignment while it remains draft.
    #[allow(
        clippy::too_many_arguments,
        reason = "actor and scope evidence stay explicit"
    )]
    pub async fn update_assignment(
        pool: &PgPool,
        tenant_id: Uuid,
        assignment_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateLearningAssignmentRequest,
    ) -> Result<Option<LearningAssignmentResponse>> {
        let Some((_, space_id)) = assignment_owner(pool, tenant_id, assignment_id).await? else {
            return Ok(None);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning assignment update")?;
        let changed = sqlx::query_scalar::<_, Uuid>(
            "UPDATE learning_assignments SET position=$4,title=$5,instructions=$6,due_at=$7,max_score_hundredths=$8,version=version+1,updated_by=$9 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='draft' AND deleted_at IS NULL RETURNING id",
        )
        .bind(tenant_id)
        .bind(assignment_id)
        .bind(request.expected_version)
        .bind(request.position)
        .bind(required("Assignment title", &request.title)?)
        .bind(required("Assignment instructions", &request.instructions)?)
        .bind(request.due_at)
        .bind(request.max_score_hundredths)
        .bind(actor_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| database_error(error, "update Learning assignment"))?;
        if changed.is_none() {
            return Ok(None);
        }
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "assignment",
            assignment_id,
            Some(space_id),
            "learning_assignment_updated",
            "learning.assignments.update",
            json!({"position": request.position, "expected_version": request.expected_version}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit Learning assignment update")?;
        assignment_response_by_id(pool, tenant_id, assignment_id).await
    }

    /// Adds one draft rubric criterion.
    #[allow(
        clippy::too_many_arguments,
        reason = "actor and scope evidence stay explicit"
    )]
    pub async fn create_rubric_criterion(
        pool: &PgPool,
        tenant_id: Uuid,
        assignment_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateLearningRubricCriterionRequest,
    ) -> Result<Option<LearningRubricCriterionResponse>> {
        let Some((_, space_id)) = assignment_owner(pool, tenant_id, assignment_id).await? else {
            return Ok(None);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning rubric creation")?;
        require_draft_assignment(&mut tx, tenant_id, assignment_id).await?;
        let criterion_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO learning_assignment_rubric_criteria (tenant_id,learning_assignment_id,position,title,description,max_score_hundredths,created_by,updated_by) VALUES ($1,$2,$3,$4,$5,$6,$7,$7) RETURNING id",
        )
        .bind(tenant_id)
        .bind(assignment_id)
        .bind(request.position)
        .bind(required("Rubric title", &request.title)?)
        .bind(optional(request.description.as_deref()))
        .bind(request.max_score_hundredths)
        .bind(actor_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| database_error(error, "create Learning rubric criterion"))?;
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "assignment",
            assignment_id,
            Some(space_id),
            "learning_assignment_rubric_changed",
            "learning.rubric_criteria.create",
            json!({"criterion_id": criterion_id, "position": request.position}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit Learning rubric creation")?;
        rubric_criterion(pool, tenant_id, criterion_id).await
    }

    /// Updates one criterion while the assignment remains draft.
    #[allow(
        clippy::too_many_arguments,
        reason = "actor and scope evidence stay explicit"
    )]
    pub async fn update_rubric_criterion(
        pool: &PgPool,
        tenant_id: Uuid,
        criterion_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateLearningRubricCriterionRequest,
    ) -> Result<Option<LearningRubricCriterionResponse>> {
        let Some((assignment_id, space_id)) = rubric_owner(pool, tenant_id, criterion_id).await?
        else {
            return Ok(None);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool.begin().await.context("start Learning rubric update")?;
        require_draft_assignment(&mut tx, tenant_id, assignment_id).await?;
        let changed = sqlx::query_scalar::<_, Uuid>(
            "UPDATE learning_assignment_rubric_criteria SET position=$4,title=$5,description=$6,max_score_hundredths=$7,version=version+1,updated_by=$8 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND deleted_at IS NULL RETURNING id",
        )
        .bind(tenant_id)
        .bind(criterion_id)
        .bind(request.expected_version)
        .bind(request.position)
        .bind(required("Rubric title", &request.title)?)
        .bind(optional(request.description.as_deref()))
        .bind(request.max_score_hundredths)
        .bind(actor_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| database_error(error, "update Learning rubric criterion"))?;
        if changed.is_none() {
            return Ok(None);
        }
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "assignment",
            assignment_id,
            Some(space_id),
            "learning_assignment_rubric_changed",
            "learning.rubric_criteria.update",
            json!({"criterion_id": criterion_id, "expected_version": request.expected_version}),
        )
        .await?;
        tx.commit().await.context("commit Learning rubric update")?;
        rubric_criterion(pool, tenant_id, criterion_id).await
    }

    /// Soft-deletes one draft rubric criterion with optimistic concurrency.
    #[allow(
        clippy::too_many_arguments,
        reason = "actor and scope evidence stay explicit"
    )]
    pub async fn delete_rubric_criterion(
        pool: &PgPool,
        tenant_id: Uuid,
        criterion_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &DeleteLearningRubricCriterionRequest,
    ) -> Result<bool> {
        let Some((assignment_id, space_id)) = rubric_owner(pool, tenant_id, criterion_id).await?
        else {
            return Ok(false);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning rubric deletion")?;
        require_draft_assignment(&mut tx, tenant_id, assignment_id).await?;
        let changed = sqlx::query_scalar::<_, Uuid>(
            "UPDATE learning_assignment_rubric_criteria SET deleted_at=NOW(),deleted_by=$4,updated_by=$4,version=version+1 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND deleted_at IS NULL RETURNING id",
        )
        .bind(tenant_id)
        .bind(criterion_id)
        .bind(request.expected_version)
        .bind(actor_id)
        .fetch_optional(&mut *tx)
        .await
        .context("delete Learning rubric criterion")?;
        if changed.is_none() {
            return Ok(false);
        }
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "assignment",
            assignment_id,
            Some(space_id),
            "learning_assignment_rubric_changed",
            "learning.rubric_criteria.delete",
            json!({"criterion_id": criterion_id, "expected_version": request.expected_version}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit Learning rubric deletion")?;
        Ok(true)
    }

    /// Publishes an assignment with an immutable SIS recipient snapshot.
    pub async fn publish_assignment(
        pool: &PgPool,
        tenant_id: Uuid,
        assignment_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<LearningAssignmentResponse>> {
        let Some((_, space_id)) = assignment_owner(pool, tenant_id, assignment_id).await? else {
            return Ok(None);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning assignment publication")?;
        let state = sqlx::query_as::<_, (String, i32, String, String, Uuid, Uuid)>(
            r#"
            SELECT assignment.status,assignment.max_score_hundredths,
                   unit.status,space.status,space.academic_year_id,space.class_group_id
              FROM learning_assignments assignment
              JOIN learning_units unit
                ON unit.id=assignment.learning_unit_id AND unit.tenant_id=assignment.tenant_id
              JOIN learning_spaces space
                ON space.id=unit.learning_space_id AND space.tenant_id=unit.tenant_id
             WHERE assignment.tenant_id=$1 AND assignment.id=$2
               AND assignment.deleted_at IS NULL AND unit.deleted_at IS NULL
               AND space.deleted_at IS NULL
             FOR UPDATE OF assignment,unit,space
            "#,
        )
        .bind(tenant_id)
        .bind(assignment_id)
        .fetch_optional(&mut *tx)
        .await
        .context("lock Learning assignment publication")?;
        let Some((status, maximum, unit_status, space_status, academic_year_id, class_group_id)) =
            state
        else {
            return Ok(None);
        };
        if status != "draft" || unit_status != "published" || space_status != "published" {
            bail!("Only a draft assignment in published Learning content can be published");
        }
        let (criterion_count, rubric_total) = sqlx::query_as::<_, (i64, i64)>(
            "SELECT COUNT(*),COALESCE(SUM(max_score_hundredths),0)::BIGINT FROM learning_assignment_rubric_criteria WHERE tenant_id=$1 AND learning_assignment_id=$2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(assignment_id)
        .fetch_one(&mut *tx)
        .await
        .context("validate Learning assignment rubric")?;
        if criterion_count == 0 || rubric_total != i64::from(maximum) {
            bail!("The assignment rubric must contain criteria totalling the maximum score");
        }
        let roster = EnrolmentOps::class_roster_on(
            pool,
            tenant_id,
            academic_year_id,
            class_group_id,
            Utc::now().date_naive(),
        )
        .await?;
        if roster.is_empty() {
            bail!("The assignment cannot be published without eligible learners");
        }
        let changed = sqlx::query_scalar::<_, Uuid>(
            "UPDATE learning_assignments SET status='published',published_by=$4,published_at=NOW(),version=version+1,updated_by=$4 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='draft' AND deleted_at IS NULL RETURNING id",
        )
        .bind(tenant_id)
        .bind(assignment_id)
        .bind(expected_version)
        .bind(actor_id)
        .fetch_optional(&mut *tx)
        .await
        .context("publish Learning assignment")?;
        if changed.is_none() {
            return Ok(None);
        }
        for recipient in &roster {
            sqlx::query(
                "INSERT INTO learning_assignment_recipients (tenant_id,learning_assignment_id,enrolment_id,learner_id) VALUES ($1,$2,$3,$4)",
            )
            .bind(tenant_id)
            .bind(assignment_id)
            .bind(recipient.enrolment_id)
            .bind(recipient.learner_id)
            .execute(&mut *tx)
            .await
            .context("snapshot Learning assignment recipient")?;
        }
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "assignment",
            assignment_id,
            Some(space_id),
            "learning_assignment_published",
            "learning.assignments.publish",
            json!({"recipient_count": roster.len(), "expected_version": expected_version}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit Learning assignment publication")?;
        assignment_response_by_id(pool, tenant_id, assignment_id).await
    }

    /// Closes a published assignment and prevents further learner submissions.
    pub async fn close_assignment(
        pool: &PgPool,
        tenant_id: Uuid,
        assignment_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ReasonedLearningTransitionRequest,
    ) -> Result<Option<LearningAssignmentResponse>> {
        let Some((_, space_id)) = assignment_owner(pool, tenant_id, assignment_id).await? else {
            return Ok(None);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let reason = required("Close reason", &request.reason)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning assignment closure")?;
        let changed = sqlx::query_scalar::<_, Uuid>(
            "UPDATE learning_assignments SET status='closed',closed_by=$4,closed_at=NOW(),close_reason=$5,version=version+1,updated_by=$4 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='published' AND deleted_at IS NULL RETURNING id",
        )
        .bind(tenant_id)
        .bind(assignment_id)
        .bind(request.expected_version)
        .bind(actor_id)
        .bind(reason)
        .fetch_optional(&mut *tx)
        .await
        .context("close Learning assignment")?;
        if changed.is_none() {
            return Ok(None);
        }
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "assignment",
            assignment_id,
            Some(space_id),
            "learning_assignment_closed",
            "learning.assignments.close",
            json!({"reason": reason, "expected_version": request.expected_version}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit Learning assignment closure")?;
        assignment_response_by_id(pool, tenant_id, assignment_id).await
    }

    /// Reads the authenticated learner's own submission aggregate, if started.
    pub async fn self_submission(
        pool: &PgPool,
        tenant_id: Uuid,
        assignment_id: Uuid,
        scope: LearningAccessScope,
    ) -> Result<Option<LearningSubmissionResponse>> {
        let Some(context) = self_submission_context(pool, tenant_id, assignment_id, scope).await?
        else {
            return Ok(None);
        };
        let submission_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM learning_submissions WHERE tenant_id=$1 AND learning_assignment_id=$2 AND assignment_recipient_id=$3 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(assignment_id)
        .bind(context.recipient_id)
        .fetch_optional(pool)
        .await
        .context("load learner submission")?;
        match submission_id {
            Some(id) => submission_response_by_id(pool, tenant_id, id, true, false).await,
            None => Ok(None),
        }
    }

    /// Saves a text-only learner draft using self scope and optimistic concurrency.
    #[allow(
        clippy::too_many_arguments,
        reason = "actor and scope evidence stay explicit"
    )]
    pub async fn save_self_submission(
        pool: &PgPool,
        tenant_id: Uuid,
        assignment_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &SaveLearningSubmissionRequest,
    ) -> Result<Option<LearningSubmissionResponse>> {
        let Some(context) = self_submission_context(pool, tenant_id, assignment_id, scope).await?
        else {
            return Ok(None);
        };
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning submission save")?;
        require_open_assignment(&mut tx, tenant_id, assignment_id).await?;
        let existing = sqlx::query_as::<_, (Uuid, String, i32)>(
            "SELECT id,status,version FROM learning_submissions WHERE tenant_id=$1 AND learning_assignment_id=$2 AND assignment_recipient_id=$3 AND deleted_at IS NULL FOR UPDATE",
        )
        .bind(tenant_id)
        .bind(assignment_id)
        .bind(context.recipient_id)
        .fetch_optional(&mut *tx)
        .await
        .context("lock learner submission")?;
        let (submission_id, event_type) = match existing {
            None => {
                if request.expected_version.is_some() {
                    bail!("The learner submission changed; reload it before saving");
                }
                let id = sqlx::query_scalar::<_, Uuid>(
                    "INSERT INTO learning_submissions (tenant_id,learning_assignment_id,assignment_recipient_id,draft_body,created_by,updated_by) VALUES ($1,$2,$3,$4,$5,$5) RETURNING id",
                )
                .bind(tenant_id)
                .bind(assignment_id)
                .bind(context.recipient_id)
                .bind(&request.body)
                .bind(actor_id)
                .fetch_one(&mut *tx)
                .await
                .context("create learner submission")?;
                (id, "learning_submission_draft_created")
            }
            Some((id, status, version)) => {
                if request.expected_version != Some(version) {
                    bail!("The learner submission changed; reload it before saving");
                }
                if !matches!(status.as_str(), "draft" | "revision_requested") {
                    bail!("A submitted or graded attempt cannot be edited");
                }
                let changed = sqlx::query_scalar::<_, Uuid>(
                    "UPDATE learning_submissions SET draft_body=$4,status='draft',version=version+1,updated_by=$5 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status IN ('draft','revision_requested') AND deleted_at IS NULL RETURNING id",
                )
                .bind(tenant_id)
                .bind(id)
                .bind(version)
                .bind(&request.body)
                .bind(actor_id)
                .fetch_optional(&mut *tx)
                .await
                .context("save learner submission")?;
                if changed.is_none() {
                    bail!("The learner submission changed; reload it before saving");
                }
                (id, "learning_submission_draft_saved")
            }
        };
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "submission",
            submission_id,
            Some(context.space_id),
            event_type,
            "learning.submissions.save",
            json!({"assignment_id": assignment_id}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit Learning submission save")?;
        submission_response_by_id(pool, tenant_id, submission_id, true, false).await
    }

    /// Appends one immutable learner attempt and safely replays duplicate submits.
    #[allow(
        clippy::too_many_arguments,
        reason = "actor and scope evidence stay explicit"
    )]
    pub async fn submit_self_submission(
        pool: &PgPool,
        tenant_id: Uuid,
        assignment_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &SubmitLearningSubmissionRequest,
    ) -> Result<Option<LearningSubmissionResponse>> {
        let Some(context) = self_submission_context(pool, tenant_id, assignment_id, scope).await?
        else {
            return Ok(None);
        };
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool.begin().await.context("start Learning submission")?;
        let due_at = require_open_assignment(&mut tx, tenant_id, assignment_id).await?;
        let submission = sqlx::query_as::<_, (Uuid, String, i32, Option<String>)>(
            "SELECT id,status,version,draft_body FROM learning_submissions WHERE tenant_id=$1 AND learning_assignment_id=$2 AND assignment_recipient_id=$3 AND deleted_at IS NULL FOR UPDATE",
        )
        .bind(tenant_id)
        .bind(assignment_id)
        .bind(context.recipient_id)
        .fetch_optional(&mut *tx)
        .await
        .context("lock learner submission")?;
        let Some((submission_id, status, version, draft_body)) = submission else {
            bail!("Save a response before submitting this assignment");
        };
        let body = required(
            "Submission response",
            draft_body.as_deref().unwrap_or_default(),
        )?;
        let fingerprint = submission_fingerprint(submission_id, request.expected_version, body);
        if let Some((stored_fingerprint, _)) = sqlx::query_as::<_, (String, Uuid)>(
            "SELECT request_fingerprint,id FROM learning_submission_versions WHERE tenant_id=$1 AND learning_submission_id=$2 AND idempotency_key=$3",
        )
        .bind(tenant_id)
        .bind(submission_id)
        .bind(request.idempotency_key)
        .fetch_optional(&mut *tx)
        .await
        .context("check Learning submission replay")?
        {
            if stored_fingerprint != fingerprint {
                bail!("The submission idempotency key was already used for different content");
            }
            tx.rollback().await.context("finish Learning submission replay")?;
            return submission_response_by_id(pool, tenant_id, submission_id, true, false).await;
        }
        if status != "draft" || version != request.expected_version {
            bail!("The learner submission changed or is not ready to submit");
        }
        let revision_number = sqlx::query_scalar::<_, i32>(
            "SELECT COALESCE(MAX(revision_number),0)+1 FROM learning_submission_versions WHERE tenant_id=$1 AND learning_submission_id=$2",
        )
        .bind(tenant_id)
        .bind(submission_id)
        .fetch_one(&mut *tx)
        .await
        .context("allocate Learning submission revision")?;
        let late = Utc::now() > due_at;
        let submission_version_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO learning_submission_versions (tenant_id,learning_submission_id,revision_number,body_snapshot,submitted_by,late_snapshot,idempotency_key,request_fingerprint) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) RETURNING id",
        )
        .bind(tenant_id)
        .bind(submission_id)
        .bind(revision_number)
        .bind(body)
        .bind(actor_id)
        .bind(late)
        .bind(request.idempotency_key)
        .bind(&fingerprint)
        .fetch_one(&mut *tx)
        .await
        .context("append Learning submission version")?;
        sqlx::query(
            "UPDATE learning_submissions SET status='submitted',current_submission_version_id=$4,first_submitted_at=COALESCE(first_submitted_at,NOW()),last_submitted_at=NOW(),version=version+1,updated_by=$5 WHERE tenant_id=$1 AND id=$2 AND version=$3",
        )
        .bind(tenant_id)
        .bind(submission_id)
        .bind(request.expected_version)
        .bind(submission_version_id)
        .bind(actor_id)
        .execute(&mut *tx)
        .await
        .context("advance Learning submission")?;
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "submission",
            submission_id,
            Some(context.space_id),
            if revision_number == 1 { "learning_submission_submitted" } else { "learning_submission_resubmitted" },
            "learning.submissions.submit",
            json!({"assignment_id": assignment_id, "submission_version_id": submission_version_id, "revision_number": revision_number, "late": late}),
        )
        .await?;
        tx.commit().await.context("commit Learning submission")?;
        submission_response_by_id(pool, tenant_id, submission_id, true, false).await
    }

    /// Lists learner submissions for an assigned teacher or campus manager.
    pub async fn list_submissions(
        pool: &PgPool,
        tenant_id: Uuid,
        assignment_id: Uuid,
        scope: LearningAccessScope,
        query: &LearningSubmissionListQuery,
    ) -> Result<(Vec<LearningSubmissionResponse>, i64)> {
        let Some((_, space_id)) = assignment_owner(pool, tenant_id, assignment_id).await? else {
            return Ok((Vec::new(), 0));
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let (page, per_page) = bounded_page(query.page, query.per_page);
        let ids = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM learning_submissions WHERE tenant_id=$1 AND learning_assignment_id=$2 AND deleted_at IS NULL AND ($3::TEXT IS NULL OR status=$3) ORDER BY updated_at DESC,id LIMIT $4 OFFSET $5",
        )
        .bind(tenant_id)
        .bind(assignment_id)
        .bind(query.status.map(LearningSubmissionStatus::as_str))
        .bind(per_page)
        .bind((page - 1) * per_page)
        .fetch_all(pool)
        .await
        .context("list Learning submissions")?;
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM learning_submissions WHERE tenant_id=$1 AND learning_assignment_id=$2 AND deleted_at IS NULL AND ($3::TEXT IS NULL OR status=$3)",
        )
        .bind(tenant_id)
        .bind(assignment_id)
        .bind(query.status.map(LearningSubmissionStatus::as_str))
        .fetch_one(pool)
        .await
        .context("count Learning submissions")?;
        let mut submissions = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(submission) =
                submission_response_by_id(pool, tenant_id, id, false, true).await?
            {
                submissions.push(submission);
            }
        }
        Ok((submissions, total))
    }

    /// Reads one exact submission after self or assigned/campus authorization.
    pub async fn get_submission(
        pool: &PgPool,
        tenant_id: Uuid,
        submission_id: Uuid,
        scope: LearningAccessScope,
    ) -> Result<Option<LearningSubmissionResponse>> {
        let visibility = submission_visibility(pool, tenant_id, submission_id, scope).await?;
        if !visibility.allowed {
            return Ok(None);
        }
        submission_response_by_id(
            pool,
            tenant_id,
            submission_id,
            visibility.include_draft_body,
            visibility.include_draft_feedback,
        )
        .await
    }

    /// Creates or updates draft feedback for the current immutable submission version.
    #[allow(
        clippy::too_many_arguments,
        reason = "actor and scope evidence stay explicit"
    )]
    pub async fn update_feedback(
        pool: &PgPool,
        tenant_id: Uuid,
        submission_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateLearningFeedbackRequest,
    ) -> Result<Option<LearningFeedbackResponse>> {
        let Some((assignment_id, space_id)) =
            submission_owner(pool, tenant_id, submission_id).await?
        else {
            return Ok(None);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning feedback update")?;
        let submission = sqlx::query_as::<_, (String, Option<Uuid>)>(
            "SELECT status,current_submission_version_id FROM learning_submissions WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL FOR UPDATE",
        )
        .bind(tenant_id)
        .bind(submission_id)
        .fetch_optional(&mut *tx)
        .await
        .context("lock Learning submission for feedback")?;
        let Some((status, current_version_id)) = submission else {
            return Ok(None);
        };
        if status != "submitted" || current_version_id != Some(request.submission_version_id) {
            bail!("Feedback must target the current submitted attempt");
        }
        validate_review_scores(&mut tx, tenant_id, assignment_id, &request.scores).await?;
        let existing = sqlx::query_as::<_, (Uuid, String, i32)>(
            "SELECT id,status,version FROM learning_submission_reviews WHERE tenant_id=$1 AND submission_version_id=$2 FOR UPDATE",
        )
        .bind(tenant_id)
        .bind(request.submission_version_id)
        .fetch_optional(&mut *tx)
        .await
        .context("lock Learning feedback")?;
        let review_id = match existing {
            None => {
                if request.expected_review_version.is_some() {
                    bail!("The feedback changed; reload it before saving");
                }
                sqlx::query_scalar::<_, Uuid>(
                    "INSERT INTO learning_submission_reviews (tenant_id,submission_version_id,overall_feedback,reviewed_by,updated_by) VALUES ($1,$2,$3,$4,$4) RETURNING id",
                )
                .bind(tenant_id)
                .bind(request.submission_version_id)
                .bind(optional(request.overall_feedback.as_deref()))
                .bind(actor_id)
                .fetch_one(&mut *tx)
                .await
                .context("create Learning feedback")?
            }
            Some((id, review_status, version)) => {
                if review_status != "draft" || request.expected_review_version != Some(version) {
                    bail!("The feedback changed or was already released");
                }
                let changed = sqlx::query_scalar::<_, Uuid>(
                    "UPDATE learning_submission_reviews SET overall_feedback=$4,version=version+1,updated_by=$5 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='draft' RETURNING id",
                )
                .bind(tenant_id)
                .bind(id)
                .bind(version)
                .bind(optional(request.overall_feedback.as_deref()))
                .bind(actor_id)
                .fetch_optional(&mut *tx)
                .await
                .context("update Learning feedback")?;
                if changed.is_none() {
                    bail!("The feedback changed; reload it before saving");
                }
                id
            }
        };
        sqlx::query(
            "DELETE FROM learning_submission_review_scores WHERE tenant_id=$1 AND review_id=$2",
        )
        .bind(tenant_id)
        .bind(review_id)
        .execute(&mut *tx)
        .await
        .context("replace Learning feedback scores")?;
        for score in &request.scores {
            sqlx::query(
                "INSERT INTO learning_submission_review_scores (tenant_id,review_id,rubric_criterion_id,earned_score_hundredths,feedback) VALUES ($1,$2,$3,$4,$5)",
            )
            .bind(tenant_id)
            .bind(review_id)
            .bind(score.rubric_criterion_id)
            .bind(score.earned_score_hundredths)
            .bind(optional(score.feedback.as_deref()))
            .execute(&mut *tx)
            .await
            .context("store Learning feedback score")?;
        }
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "review",
            review_id,
            Some(space_id),
            "learning_feedback_draft_saved",
            "learning.feedback.update",
            json!({"submission_id": submission_id, "submission_version_id": request.submission_version_id, "score_count": request.scores.len()}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit Learning feedback update")?;
        feedback_response_by_id(pool, tenant_id, review_id).await
    }

    /// Releases feedback once, moving the current submission to its reviewed state.
    #[allow(
        clippy::too_many_arguments,
        reason = "actor and scope evidence stay explicit"
    )]
    pub async fn release_feedback(
        pool: &PgPool,
        tenant_id: Uuid,
        submission_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ReleaseLearningFeedbackRequest,
    ) -> Result<Option<LearningFeedbackResponse>> {
        let Some((assignment_id, space_id)) =
            submission_owner(pool, tenant_id, submission_id).await?
        else {
            return Ok(None);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning feedback release")?;
        let review = sqlx::query_as::<
            _,
            (
                Uuid,
                Uuid,
                String,
                i32,
                Option<String>,
                Option<Uuid>,
                Option<String>,
            ),
        >(
            r#"
            SELECT review.id,review.submission_version_id,review.status,review.version,
                   review.overall_feedback,review.release_idempotency_key,
                   review.release_request_fingerprint
              FROM learning_submission_reviews review
              JOIN learning_submission_versions version
                ON version.id=review.submission_version_id AND version.tenant_id=review.tenant_id
             WHERE review.tenant_id=$1 AND version.learning_submission_id=$2
             FOR UPDATE OF review
            "#,
        )
        .bind(tenant_id)
        .bind(submission_id)
        .fetch_optional(&mut *tx)
        .await
        .context("lock Learning feedback release")?;
        let Some((
            review_id,
            submission_version_id,
            status,
            version,
            overall_feedback,
            replay_key,
            replay_fingerprint,
        )) = review
        else {
            bail!("Save feedback before releasing it");
        };
        let fingerprint = feedback_release_fingerprint(
            review_id,
            request.expected_review_version,
            request.outcome,
        );
        if status == "released" {
            if replay_key == Some(request.idempotency_key)
                && replay_fingerprint.as_deref() == Some(fingerprint.as_str())
            {
                tx.rollback()
                    .await
                    .context("finish Learning feedback replay")?;
                return feedback_response_by_id(pool, tenant_id, review_id).await;
            }
            bail!("Feedback was already released");
        }
        if version != request.expected_review_version {
            bail!("The feedback changed; reload it before releasing");
        }
        let submission = sqlx::query_as::<_, (String, Option<Uuid>, i32)>(
            "SELECT status,current_submission_version_id,version FROM learning_submissions WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL FOR UPDATE",
        )
        .bind(tenant_id)
        .bind(submission_id)
        .fetch_optional(&mut *tx)
        .await
        .context("lock Learning submission review state")?;
        let Some((submission_status, current_version_id, submission_version)) = submission else {
            return Ok(None);
        };
        if submission_status != "submitted" || current_version_id != Some(submission_version_id) {
            bail!("Feedback must target the current submitted attempt");
        }
        let total_score = match request.outcome {
            LearningReviewOutcome::Graded => {
                let (criteria, scored, earned, maximum) =
                    sqlx::query_as::<_, (i64, i64, i64, i64)>(
                        r#"
                    SELECT COUNT(criterion.id),COUNT(score.id),
                           COALESCE(SUM(score.earned_score_hundredths),0)::BIGINT,
                           COALESCE(SUM(criterion.max_score_hundredths),0)::BIGINT
                      FROM learning_assignment_rubric_criteria criterion
                      LEFT JOIN learning_submission_review_scores score
                        ON score.tenant_id=criterion.tenant_id
                       AND score.rubric_criterion_id=criterion.id AND score.review_id=$3
                     WHERE criterion.tenant_id=$1
                       AND criterion.learning_assignment_id=$2 AND criterion.deleted_at IS NULL
                    "#,
                    )
                    .bind(tenant_id)
                    .bind(assignment_id)
                    .bind(review_id)
                    .fetch_one(&mut *tx)
                    .await
                    .context("validate complete Learning rubric feedback")?;
                if criteria == 0 || criteria != scored || earned > maximum {
                    bail!(
                        "Every rubric criterion must be scored within its maximum before grading"
                    );
                }
                Some(i32::try_from(earned).context("Learning score exceeded its supported range")?)
            }
            LearningReviewOutcome::RevisionRequested => {
                required(
                    "Revision feedback",
                    overall_feedback.as_deref().unwrap_or_default(),
                )?;
                None
            }
        };
        sqlx::query(
            "UPDATE learning_submission_reviews SET status='released',outcome=$4,total_score_hundredths=$5,released_by=$6,released_at=NOW(),release_idempotency_key=$7,release_request_fingerprint=$8,version=version+1,updated_by=$6 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='draft'",
        )
        .bind(tenant_id)
        .bind(review_id)
        .bind(request.expected_review_version)
        .bind(request.outcome.as_str())
        .bind(total_score)
        .bind(actor_id)
        .bind(request.idempotency_key)
        .bind(&fingerprint)
        .execute(&mut *tx)
        .await
        .context("release Learning feedback")?;
        sqlx::query(
            "UPDATE learning_submissions SET status=$4,graded_at=CASE WHEN $4='graded' THEN NOW() ELSE NULL END,version=version+1,updated_by=$5 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='submitted'",
        )
        .bind(tenant_id)
        .bind(submission_id)
        .bind(submission_version)
        .bind(request.outcome.as_str())
        .bind(actor_id)
        .execute(&mut *tx)
        .await
        .context("apply Learning feedback outcome")?;
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "review",
            review_id,
            Some(space_id),
            "learning_feedback_released",
            "learning.feedback.release",
            json!({"submission_id": submission_id, "submission_version_id": submission_version_id, "outcome": request.outcome.as_str(), "total_score_hundredths": total_score}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit Learning feedback release")?;
        feedback_response_by_id(pool, tenant_id, review_id).await
    }

    /// Returns the authenticated learner's derived progress in one space.
    pub async fn self_progress(
        pool: &PgPool,
        tenant_id: Uuid,
        space_id: Uuid,
        scope: LearningAccessScope,
    ) -> Result<Option<LearningProgressEntry>> {
        let Some(space) = space_row(pool, tenant_id, space_id).await? else {
            return Ok(None);
        };
        if !scope_allows_space(pool, tenant_id, &space, scope).await? {
            return Ok(None);
        }
        let Some(account_id) = self_account(scope) else {
            bail!("Learner self scope is required for personal Learning progress");
        };
        let Some(roster) = EnrolmentOps::active_roster_entry_for_account(
            pool,
            tenant_id,
            account_id,
            space.academic_year_id,
            space.class_group_id,
        )
        .await?
        else {
            return Ok(None);
        };
        let mut rows = progress_rows(pool, tenant_id, space_id, Some(roster.learner_id)).await?;
        match rows.pop() {
            Some(row) => progress_response(pool, tenant_id, row).await.map(Some),
            None => Ok(Some(empty_progress(roster))),
        }
    }

    /// Returns derived progress for the assigned class roster or campus manager.
    pub async fn list_progress(
        pool: &PgPool,
        tenant_id: Uuid,
        space_id: Uuid,
        scope: LearningAccessScope,
    ) -> Result<Vec<LearningProgressEntry>> {
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let rows = progress_rows(pool, tenant_id, space_id, None).await?;
        let mut progress = Vec::with_capacity(rows.len());
        for row in rows {
            progress.push(progress_response(pool, tenant_id, row).await?);
        }
        Ok(progress)
    }
}

#[derive(Debug)]
struct LearningVisibility {
    mode: &'static str,
    assignment_ids: Option<Vec<Uuid>>,
    academic_year_id: Option<Uuid>,
    class_group_ids: Option<Vec<Uuid>>,
}

async fn visibility(
    pool: &PgPool,
    tenant_id: Uuid,
    scope: LearningAccessScope,
) -> Result<LearningVisibility> {
    match scope {
        LearningAccessScope::Campus => Ok(LearningVisibility {
            mode: "campus",
            assignment_ids: None,
            academic_year_id: None,
            class_group_ids: None,
        }),
        LearningAccessScope::AssignedTo(account_id) => Ok(LearningVisibility {
            mode: "assigned",
            assignment_ids: Some(
                TeachingAssignmentOps::active_ids_for_account(pool, tenant_id, account_id).await?,
            ),
            academic_year_id: None,
            class_group_ids: None,
        }),
        LearningAccessScope::SelfFor(account_id) => {
            let academic_year_id = AcademicYearOps::get_active(pool, tenant_id)
                .await?
                .map(|year| year.id);
            let class_group_ids = match academic_year_id {
                Some(year_id) => {
                    EnrolmentOps::active_class_ids_for_account(pool, tenant_id, account_id, year_id)
                        .await?
                }
                None => Vec::new(),
            };
            Ok(LearningVisibility {
                mode: "self",
                assignment_ids: None,
                academic_year_id,
                class_group_ids: Some(class_group_ids),
            })
        }
        LearningAccessScope::SelfAndAssigned(account_id) => {
            let assignment_ids =
                TeachingAssignmentOps::active_ids_for_account(pool, tenant_id, account_id).await?;
            let academic_year_id = AcademicYearOps::get_active(pool, tenant_id)
                .await?
                .map(|year| year.id);
            let class_group_ids = match academic_year_id {
                Some(year_id) => {
                    EnrolmentOps::active_class_ids_for_account(pool, tenant_id, account_id, year_id)
                        .await?
                }
                None => Vec::new(),
            };
            Ok(LearningVisibility {
                mode: "self_and_assigned",
                assignment_ids: Some(assignment_ids),
                academic_year_id,
                class_group_ids: Some(class_group_ids),
            })
        }
    }
}

async fn scope_allows_space(
    pool: &PgPool,
    tenant_id: Uuid,
    row: &LearningSpaceRow,
    scope: LearningAccessScope,
) -> Result<bool> {
    match scope {
        LearningAccessScope::Campus => Ok(true),
        LearningAccessScope::AssignedTo(account_id) => {
            TeachingAssignmentOps::is_active_for_account(
                pool,
                tenant_id,
                row.teaching_assignment_id,
                account_id,
            )
            .await
        }
        LearningAccessScope::SelfFor(account_id) => {
            if row.status != "published" {
                return Ok(false);
            }
            EnrolmentOps::account_is_actively_enrolled(
                pool,
                tenant_id,
                account_id,
                row.academic_year_id,
                row.class_group_id,
            )
            .await
        }
        LearningAccessScope::SelfAndAssigned(account_id) => {
            if TeachingAssignmentOps::is_active_for_account(
                pool,
                tenant_id,
                row.teaching_assignment_id,
                account_id,
            )
            .await?
            {
                return Ok(true);
            }
            if row.status != "published" {
                return Ok(false);
            }
            EnrolmentOps::account_is_actively_enrolled(
                pool,
                tenant_id,
                account_id,
                row.academic_year_id,
                row.class_group_id,
            )
            .await
        }
    }
}

async fn ensure_can_author_assignment(
    pool: &PgPool,
    tenant_id: Uuid,
    assignment_id: Uuid,
    scope: LearningAccessScope,
) -> Result<()> {
    match scope {
        LearningAccessScope::Campus => Ok(()),
        LearningAccessScope::AssignedTo(account_id) => {
            if TeachingAssignmentOps::is_active_for_account(
                pool,
                tenant_id,
                assignment_id,
                account_id,
            )
            .await?
            {
                Ok(())
            } else {
                bail!("The teaching assignment is outside your current Learning access")
            }
        }
        LearningAccessScope::SelfFor(_) => bail!("Learner access cannot change Learning content"),
        LearningAccessScope::SelfAndAssigned(account_id) => {
            if TeachingAssignmentOps::is_active_for_account(
                pool,
                tenant_id,
                assignment_id,
                account_id,
            )
            .await?
            {
                Ok(())
            } else {
                bail!("The teaching assignment is outside your current Learning access")
            }
        }
    }
}

async fn published_only_for_space(
    pool: &PgPool,
    tenant_id: Uuid,
    row: &LearningSpaceRow,
    scope: LearningAccessScope,
) -> Result<bool> {
    match scope {
        LearningAccessScope::SelfFor(_) => Ok(true),
        LearningAccessScope::SelfAndAssigned(account_id) => {
            Ok(!TeachingAssignmentOps::is_active_for_account(
                pool,
                tenant_id,
                row.teaching_assignment_id,
                account_id,
            )
            .await?)
        }
        LearningAccessScope::Campus | LearningAccessScope::AssignedTo(_) => Ok(false),
    }
}

async fn ensure_can_author_space(
    pool: &PgPool,
    tenant_id: Uuid,
    space_id: Uuid,
    scope: LearningAccessScope,
) -> Result<()> {
    let row = space_row(pool, tenant_id, space_id)
        .await?
        .ok_or_else(|| anyhow!("The Learning space was not found"))?;
    ensure_can_author_assignment(pool, tenant_id, row.teaching_assignment_id, scope).await
}

async fn hydrate_space(
    pool: &PgPool,
    tenant_id: Uuid,
    row: LearningSpaceRow,
    published_only: bool,
) -> Result<LearningSpaceResponse> {
    let space_id = row.id;
    let summary = hydrate_space_summary(pool, tenant_id, row).await?;
    let unit_rows = sqlx::query_as::<_, LearningUnitRow>(
        r#"
        SELECT id,learning_space_id,position,title,summary,status,version,published_at,
               withdrawn_at,withdrawal_reason,created_at,updated_at
          FROM learning_units
         WHERE tenant_id=$1 AND learning_space_id=$2 AND deleted_at IS NULL
           AND (NOT $3 OR status='published')
         ORDER BY position,created_at,id
        "#,
    )
    .bind(tenant_id)
    .bind(space_id)
    .bind(published_only)
    .fetch_all(pool)
    .await
    .context("Failed to load Learning units")?;
    let mut units = Vec::with_capacity(unit_rows.len());
    for row in unit_rows {
        units.push(hydrate_unit(pool, tenant_id, row, published_only).await?);
    }
    Ok(LearningSpaceResponse { summary, units })
}

async fn hydrate_space_summary(
    pool: &PgPool,
    tenant_id: Uuid,
    row: LearningSpaceRow,
) -> Result<LearningSpaceSummary> {
    let assignment = TeachingAssignmentOps::get_by_id(pool, tenant_id, row.teaching_assignment_id)
        .await?
        .context("The Learning teaching assignment is unavailable")?;
    let term = AcademicTermOps::get_by_id(pool, tenant_id, row.academic_term_id)
        .await?
        .context("The Learning academic term is unavailable")?;
    Ok(LearningSpaceSummary {
        id: row.id,
        teaching_assignment_id: row.teaching_assignment_id,
        academic_year_id: row.academic_year_id,
        academic_year_name: assignment.academic_year_name,
        academic_term_id: row.academic_term_id,
        academic_term_name: term.name,
        class_group_id: row.class_group_id,
        class_group_name: assignment.class_group_name,
        subject_name: assignment.subject_name,
        teacher_name: assignment.teacher_name,
        title: row.title,
        summary: row.summary,
        status: parse_space_status(&row.status)?,
        version: row.version,
        unit_count: row.unit_count,
        published_unit_count: row.published_unit_count,
        published_at: row.published_at,
        archived_at: row.archived_at,
        archive_reason: row.archive_reason,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

async fn hydrate_unit(
    pool: &PgPool,
    tenant_id: Uuid,
    row: LearningUnitRow,
    published_only: bool,
) -> Result<LearningUnitResponse> {
    let resource_rows = sqlx::query_as::<_, LearningResourceRow>(
        r#"
        SELECT id,learning_unit_id,document_file_id,display_title,sensitivity_snapshot,
               position,status,version,published_at,withdrawn_at,withdrawal_reason,created_at,updated_at
          FROM learning_resources
         WHERE tenant_id=$1 AND learning_unit_id=$2 AND deleted_at IS NULL
           AND (NOT $3 OR status='published')
         ORDER BY position,created_at,id
        "#,
    ).bind(tenant_id).bind(row.id).bind(published_only).fetch_all(pool).await
     .context("Failed to load Learning resources")?;
    let mut resources = Vec::with_capacity(resource_rows.len());
    for resource in resource_rows {
        resources.push(hydrate_resource(pool, tenant_id, resource).await?);
    }
    Ok(LearningUnitResponse {
        id: row.id,
        learning_space_id: row.learning_space_id,
        position: row.position,
        title: row.title,
        summary: row.summary,
        status: parse_unit_status(&row.status)?,
        version: row.version,
        published_at: row.published_at,
        withdrawn_at: row.withdrawn_at,
        withdrawal_reason: row.withdrawal_reason,
        resources,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

async fn hydrate_resource(
    pool: &PgPool,
    tenant_id: Uuid,
    row: LearningResourceRow,
) -> Result<LearningResourceResponse> {
    let document =
        DocumentRegistryOps::evidence_reference(pool, tenant_id, row.document_file_id, false)
            .await?;
    Ok(LearningResourceResponse {
        id: row.id,
        learning_unit_id: row.learning_unit_id,
        document_file_id: row.document_file_id,
        document,
        display_title: row.display_title,
        sensitivity_snapshot: row.sensitivity_snapshot,
        position: row.position,
        status: parse_resource_status(&row.status)?,
        version: row.version,
        published_at: row.published_at,
        withdrawn_at: row.withdrawn_at,
        withdrawal_reason: row.withdrawal_reason,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

async fn unit_response(
    pool: &PgPool,
    tenant_id: Uuid,
    unit_id: Uuid,
    published_only: bool,
) -> Result<Option<LearningUnitResponse>> {
    let row = sqlx::query_as::<_, LearningUnitRow>(
        "SELECT id,learning_space_id,position,title,summary,status,version,published_at,withdrawn_at,withdrawal_reason,created_at,updated_at FROM learning_units WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL",
    ).bind(tenant_id).bind(unit_id).fetch_optional(pool).await.context("load Learning unit")?;
    match row {
        Some(row) => hydrate_unit(pool, tenant_id, row, published_only)
            .await
            .map(Some),
        None => Ok(None),
    }
}

async fn resource_response(
    pool: &PgPool,
    tenant_id: Uuid,
    resource_id: Uuid,
) -> Result<Option<LearningResourceResponse>> {
    let row = sqlx::query_as::<_, LearningResourceRow>(
        "SELECT id,learning_unit_id,document_file_id,display_title,sensitivity_snapshot,position,status,version,published_at,withdrawn_at,withdrawal_reason,created_at,updated_at FROM learning_resources WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL",
    ).bind(tenant_id).bind(resource_id).fetch_optional(pool).await.context("load Learning resource")?;
    match row {
        Some(row) => hydrate_resource(pool, tenant_id, row).await.map(Some),
        None => Ok(None),
    }
}

async fn space_row(
    pool: &PgPool,
    tenant_id: Uuid,
    space_id: Uuid,
) -> Result<Option<LearningSpaceRow>> {
    sqlx::query_as::<_, LearningSpaceRow>(SPACE_BY_ID_SQL)
        .bind(tenant_id)
        .bind(space_id)
        .fetch_optional(pool)
        .await
        .context("load Learning space")
}

async fn unit_space_id(pool: &PgPool, tenant_id: Uuid, unit_id: Uuid) -> Result<Option<Uuid>> {
    sqlx::query_scalar::<_, Uuid>(
        "SELECT learning_space_id FROM learning_units WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL",
    ).bind(tenant_id).bind(unit_id).fetch_optional(pool).await.context("resolve Learning unit owner")
}

async fn resource_owner(
    pool: &PgPool,
    tenant_id: Uuid,
    resource_id: Uuid,
) -> Result<Option<(Uuid, Uuid)>> {
    sqlx::query_as::<_, (Uuid, Uuid)>(
        "SELECT resource.learning_unit_id,unit.learning_space_id FROM learning_resources resource JOIN learning_units unit ON unit.id=resource.learning_unit_id AND unit.tenant_id=resource.tenant_id WHERE resource.tenant_id=$1 AND resource.id=$2 AND resource.deleted_at IS NULL AND unit.deleted_at IS NULL",
    ).bind(tenant_id).bind(resource_id).fetch_optional(pool).await.context("resolve Learning resource owner")
}

const ASSIGNMENT_SELECT: &str = r#"
SELECT assignment.id,assignment.learning_unit_id,unit.learning_space_id,
       assignment.position,assignment.title,assignment.instructions,assignment.due_at,
       assignment.max_score_hundredths,assignment.status,assignment.version,
       assignment.published_at,assignment.closed_at,assignment.close_reason,
       assignment.created_at,assignment.updated_at,
       (SELECT COUNT(*) FROM learning_assignment_recipients recipient
         WHERE recipient.tenant_id=assignment.tenant_id AND recipient.learning_assignment_id=assignment.id)::BIGINT AS recipient_count,
       (SELECT COUNT(*) FROM learning_submissions submission
         WHERE submission.tenant_id=assignment.tenant_id AND submission.learning_assignment_id=assignment.id
           AND submission.deleted_at IS NULL)::BIGINT AS submission_count
  FROM learning_assignments assignment
  JOIN learning_units unit ON unit.id=assignment.learning_unit_id AND unit.tenant_id=assignment.tenant_id
 WHERE assignment.tenant_id=$1 AND assignment.deleted_at IS NULL AND unit.deleted_at IS NULL
"#;

async fn assignment_row(
    pool: &PgPool,
    tenant_id: Uuid,
    assignment_id: Uuid,
) -> Result<Option<LearningAssignmentRow>> {
    sqlx::query_as::<_, LearningAssignmentRow>(&format!("{ASSIGNMENT_SELECT} AND assignment.id=$2"))
        .bind(tenant_id)
        .bind(assignment_id)
        .fetch_optional(pool)
        .await
        .context("load Learning assignment")
}

async fn assignment_response_by_id(
    pool: &PgPool,
    tenant_id: Uuid,
    assignment_id: Uuid,
) -> Result<Option<LearningAssignmentResponse>> {
    match assignment_row(pool, tenant_id, assignment_id).await? {
        Some(row) => assignment_response(pool, tenant_id, row).await.map(Some),
        None => Ok(None),
    }
}

async fn assignment_response(
    pool: &PgPool,
    tenant_id: Uuid,
    row: LearningAssignmentRow,
) -> Result<LearningAssignmentResponse> {
    let rubric_rows = sqlx::query_as::<_, LearningRubricCriterionRow>(
        "SELECT id,learning_assignment_id,position,title,description,max_score_hundredths,version FROM learning_assignment_rubric_criteria WHERE tenant_id=$1 AND learning_assignment_id=$2 AND deleted_at IS NULL ORDER BY position,created_at,id",
    ).bind(tenant_id).bind(row.id).fetch_all(pool).await.context("load Learning rubric")?;
    Ok(LearningAssignmentResponse {
        id: row.id,
        learning_unit_id: row.learning_unit_id,
        learning_space_id: row.learning_space_id,
        position: row.position,
        title: row.title,
        instructions: row.instructions,
        due_at: row.due_at,
        max_score_hundredths: row.max_score_hundredths,
        status: parse_assignment_status(&row.status)?,
        version: row.version,
        recipient_count: row.recipient_count,
        submission_count: row.submission_count,
        published_at: row.published_at,
        closed_at: row.closed_at,
        close_reason: row.close_reason,
        rubric: rubric_rows.into_iter().map(rubric_response).collect(),
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn rubric_response(row: LearningRubricCriterionRow) -> LearningRubricCriterionResponse {
    LearningRubricCriterionResponse {
        id: row.id,
        learning_assignment_id: row.learning_assignment_id,
        position: row.position,
        title: row.title,
        description: row.description,
        max_score_hundredths: row.max_score_hundredths,
        version: row.version,
    }
}

async fn rubric_criterion(
    pool: &PgPool,
    tenant_id: Uuid,
    criterion_id: Uuid,
) -> Result<Option<LearningRubricCriterionResponse>> {
    sqlx::query_as::<_, LearningRubricCriterionRow>(
        "SELECT id,learning_assignment_id,position,title,description,max_score_hundredths,version FROM learning_assignment_rubric_criteria WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL",
    ).bind(tenant_id).bind(criterion_id).fetch_optional(pool).await
     .context("load Learning rubric criterion").map(|row| row.map(rubric_response))
}

async fn assignment_owner(
    pool: &PgPool,
    tenant_id: Uuid,
    assignment_id: Uuid,
) -> Result<Option<(Uuid, Uuid)>> {
    sqlx::query_as::<_, (Uuid, Uuid)>(
        "SELECT assignment.learning_unit_id,unit.learning_space_id FROM learning_assignments assignment JOIN learning_units unit ON unit.id=assignment.learning_unit_id AND unit.tenant_id=assignment.tenant_id WHERE assignment.tenant_id=$1 AND assignment.id=$2 AND assignment.deleted_at IS NULL AND unit.deleted_at IS NULL",
    ).bind(tenant_id).bind(assignment_id).fetch_optional(pool).await.context("resolve Learning assignment owner")
}

async fn rubric_owner(
    pool: &PgPool,
    tenant_id: Uuid,
    criterion_id: Uuid,
) -> Result<Option<(Uuid, Uuid)>> {
    sqlx::query_as::<_, (Uuid, Uuid)>(
        "SELECT criterion.learning_assignment_id,unit.learning_space_id FROM learning_assignment_rubric_criteria criterion JOIN learning_assignments assignment ON assignment.id=criterion.learning_assignment_id AND assignment.tenant_id=criterion.tenant_id JOIN learning_units unit ON unit.id=assignment.learning_unit_id AND unit.tenant_id=assignment.tenant_id WHERE criterion.tenant_id=$1 AND criterion.id=$2 AND criterion.deleted_at IS NULL AND assignment.deleted_at IS NULL AND unit.deleted_at IS NULL",
    ).bind(tenant_id).bind(criterion_id).fetch_optional(pool).await.context("resolve Learning rubric owner")
}

async fn require_active_assignment_parent(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    unit_id: Uuid,
    space_id: Uuid,
) -> Result<()> {
    let state = sqlx::query_as::<_, (String, String)>(
        "SELECT unit.status,space.status FROM learning_units unit JOIN learning_spaces space ON space.id=unit.learning_space_id AND space.tenant_id=unit.tenant_id WHERE unit.tenant_id=$1 AND unit.id=$2 AND space.id=$3 AND unit.deleted_at IS NULL AND space.deleted_at IS NULL FOR UPDATE OF unit,space",
    ).bind(tenant_id).bind(unit_id).bind(space_id).fetch_optional(&mut **tx).await.context("lock Learning assignment parent")?;
    match state {
        Some((unit, space)) if unit != "withdrawn" && space != "archived" => Ok(()),
        Some(_) => bail!("Assignments cannot be added to withdrawn or archived Learning content"),
        None => bail!("The Learning assignment parent is unavailable"),
    }
}

async fn require_draft_assignment(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    assignment_id: Uuid,
) -> Result<()> {
    let status = sqlx::query_scalar::<_, String>(
        "SELECT status FROM learning_assignments WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL FOR UPDATE",
    ).bind(tenant_id).bind(assignment_id).fetch_optional(&mut **tx).await.context("lock draft Learning assignment")?;
    match status.as_deref() {
        Some("draft") => Ok(()),
        Some(_) => bail!("A published Learning assignment is immutable"),
        None => bail!("The Learning assignment is unavailable"),
    }
}

async fn require_open_assignment(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    assignment_id: Uuid,
) -> Result<chrono::DateTime<Utc>> {
    let state = sqlx::query_as::<_, (String, chrono::DateTime<Utc>)>(
        "SELECT status,due_at FROM learning_assignments WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL FOR UPDATE",
    ).bind(tenant_id).bind(assignment_id).fetch_optional(&mut **tx).await.context("lock open Learning assignment")?;
    match state {
        Some((status, due_at)) if status == "published" => Ok(due_at),
        Some(_) => bail!("The Learning assignment is not open for submissions"),
        None => bail!("The Learning assignment is unavailable"),
    }
}

struct SelfSubmissionContext {
    recipient_id: Uuid,
    space_id: Uuid,
}

fn self_account(scope: LearningAccessScope) -> Option<Uuid> {
    match scope {
        LearningAccessScope::SelfFor(id) | LearningAccessScope::SelfAndAssigned(id) => Some(id),
        LearningAccessScope::Campus | LearningAccessScope::AssignedTo(_) => None,
    }
}

async fn self_submission_context(
    pool: &PgPool,
    tenant_id: Uuid,
    assignment_id: Uuid,
    scope: LearningAccessScope,
) -> Result<Option<SelfSubmissionContext>> {
    let Some(account_id) = self_account(scope) else {
        bail!("Learner self scope is required for Learning participation")
    };
    let Some(assignment) = assignment_row(pool, tenant_id, assignment_id).await? else {
        return Ok(None);
    };
    if assignment.status == "draft" {
        return Ok(None);
    }
    let Some(space) = space_row(pool, tenant_id, assignment.learning_space_id).await? else {
        return Ok(None);
    };
    if !scope_allows_space(pool, tenant_id, &space, scope).await? {
        return Ok(None);
    }
    let Some(roster) = EnrolmentOps::active_roster_entry_for_account(
        pool,
        tenant_id,
        account_id,
        space.academic_year_id,
        space.class_group_id,
    )
    .await?
    else {
        return Ok(None);
    };
    let recipient_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM learning_assignment_recipients WHERE tenant_id=$1 AND learning_assignment_id=$2 AND enrolment_id=$3 AND learner_id=$4",
    ).bind(tenant_id).bind(assignment_id).bind(roster.enrolment_id).bind(roster.learner_id)
     .fetch_optional(pool).await.context("resolve Learning assignment recipient")?;
    Ok(recipient_id.map(|recipient_id| SelfSubmissionContext {
        recipient_id,
        space_id: assignment.learning_space_id,
    }))
}

fn submission_fingerprint(submission_id: Uuid, expected_version: i32, body: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(submission_id.as_bytes());
    digest.update(expected_version.to_be_bytes());
    digest.update(body.as_bytes());
    format!("{:x}", digest.finalize())
}

fn feedback_release_fingerprint(
    review_id: Uuid,
    expected_version: i32,
    outcome: LearningReviewOutcome,
) -> String {
    let mut digest = Sha256::new();
    digest.update(review_id.as_bytes());
    digest.update(expected_version.to_be_bytes());
    digest.update(outcome.as_str().as_bytes());
    format!("{:x}", digest.finalize())
}

async fn submission_owner(
    pool: &PgPool,
    tenant_id: Uuid,
    submission_id: Uuid,
) -> Result<Option<(Uuid, Uuid)>> {
    sqlx::query_as::<_, (Uuid, Uuid)>(
        "SELECT submission.learning_assignment_id,unit.learning_space_id FROM learning_submissions submission JOIN learning_assignments assignment ON assignment.id=submission.learning_assignment_id AND assignment.tenant_id=submission.tenant_id JOIN learning_units unit ON unit.id=assignment.learning_unit_id AND unit.tenant_id=assignment.tenant_id WHERE submission.tenant_id=$1 AND submission.id=$2 AND submission.deleted_at IS NULL AND assignment.deleted_at IS NULL AND unit.deleted_at IS NULL",
    ).bind(tenant_id).bind(submission_id).fetch_optional(pool).await.context("resolve Learning submission owner")
}

struct SubmissionVisibility {
    allowed: bool,
    include_draft_body: bool,
    include_draft_feedback: bool,
}

async fn submission_visibility(
    pool: &PgPool,
    tenant_id: Uuid,
    submission_id: Uuid,
    scope: LearningAccessScope,
) -> Result<SubmissionVisibility> {
    let relation = sqlx::query_as::<_, (Uuid, Uuid, Uuid, Uuid, Uuid)>(
        "SELECT space.teaching_assignment_id,space.academic_year_id,space.class_group_id,recipient.enrolment_id,recipient.learner_id FROM learning_submissions submission JOIN learning_assignment_recipients recipient ON recipient.id=submission.assignment_recipient_id AND recipient.tenant_id=submission.tenant_id JOIN learning_assignments assignment ON assignment.id=submission.learning_assignment_id AND assignment.tenant_id=submission.tenant_id JOIN learning_units unit ON unit.id=assignment.learning_unit_id AND unit.tenant_id=assignment.tenant_id JOIN learning_spaces space ON space.id=unit.learning_space_id AND space.tenant_id=unit.tenant_id WHERE submission.tenant_id=$1 AND submission.id=$2 AND submission.deleted_at IS NULL AND assignment.deleted_at IS NULL AND unit.deleted_at IS NULL AND space.deleted_at IS NULL",
    ).bind(tenant_id).bind(submission_id).fetch_optional(pool).await.context("authorize Learning submission")?;
    let Some((teaching_assignment_id, academic_year_id, class_group_id, enrolment_id, learner_id)) =
        relation
    else {
        return Ok(SubmissionVisibility {
            allowed: false,
            include_draft_body: false,
            include_draft_feedback: false,
        });
    };
    let is_self = |account_id| async move {
        Ok::<bool, anyhow::Error>(
            EnrolmentOps::active_roster_entry_for_account(
                pool,
                tenant_id,
                account_id,
                academic_year_id,
                class_group_id,
            )
            .await?
            .is_some_and(|entry| {
                entry.enrolment_id == enrolment_id && entry.learner_id == learner_id
            }),
        )
    };
    match scope {
        LearningAccessScope::Campus => Ok(SubmissionVisibility {
            allowed: true,
            include_draft_body: false,
            include_draft_feedback: true,
        }),
        LearningAccessScope::AssignedTo(account_id) => Ok(SubmissionVisibility {
            allowed: TeachingAssignmentOps::is_active_for_account(
                pool,
                tenant_id,
                teaching_assignment_id,
                account_id,
            )
            .await?,
            include_draft_body: false,
            include_draft_feedback: true,
        }),
        LearningAccessScope::SelfFor(account_id) => Ok(SubmissionVisibility {
            allowed: is_self(account_id).await?,
            include_draft_body: true,
            include_draft_feedback: false,
        }),
        LearningAccessScope::SelfAndAssigned(account_id) => {
            let assigned = TeachingAssignmentOps::is_active_for_account(
                pool,
                tenant_id,
                teaching_assignment_id,
                account_id,
            )
            .await?;
            Ok(SubmissionVisibility {
                allowed: assigned || is_self(account_id).await?,
                include_draft_body: !assigned,
                include_draft_feedback: assigned,
            })
        }
    }
}

async fn submission_response_by_id(
    pool: &PgPool,
    tenant_id: Uuid,
    submission_id: Uuid,
    include_draft_body: bool,
    include_draft_feedback: bool,
) -> Result<Option<LearningSubmissionResponse>> {
    let row = sqlx::query_as::<_, LearningSubmissionRow>(
        "SELECT submission.id,submission.learning_assignment_id,submission.assignment_recipient_id,recipient.learner_id,recipient.enrolment_id,submission.draft_body,submission.status,submission.version,submission.current_submission_version_id,submission.created_at,submission.updated_at FROM learning_submissions submission JOIN learning_assignment_recipients recipient ON recipient.id=submission.assignment_recipient_id AND recipient.tenant_id=submission.tenant_id WHERE submission.tenant_id=$1 AND submission.id=$2 AND submission.deleted_at IS NULL",
    ).bind(tenant_id).bind(submission_id).fetch_optional(pool).await.context("load Learning submission")?;
    let Some(row) = row else { return Ok(None) };
    let identity =
        EnrolmentOps::roster_references_by_enrolment_ids(pool, tenant_id, &[row.enrolment_id])
            .await?
            .into_iter()
            .find(|entry| entry.learner_id == row.learner_id)
            .ok_or_else(|| {
                anyhow!("The SIS learner identity for this submission is unavailable")
            })?;
    let versions = sqlx::query_as::<_, LearningSubmissionVersionRow>(
        "SELECT id,revision_number,body_snapshot,late_snapshot,submitted_at FROM learning_submission_versions WHERE tenant_id=$1 AND learning_submission_id=$2 ORDER BY revision_number",
    ).bind(tenant_id).bind(submission_id).fetch_all(pool).await.context("load Learning submission versions")?
     .into_iter().map(|version| LearningSubmissionVersionResponse { id: version.id, revision_number: version.revision_number,
        body: version.body_snapshot, late: version.late_snapshot, submitted_at: version.submitted_at }).collect();
    let feedback = match row.current_submission_version_id {
        Some(version_id) => {
            feedback_for_version(pool, tenant_id, version_id, include_draft_feedback).await?
        }
        None => None,
    };
    Ok(Some(LearningSubmissionResponse {
        id: row.id,
        learning_assignment_id: row.learning_assignment_id,
        assignment_recipient_id: row.assignment_recipient_id,
        learner_id: row.learner_id,
        enrolment_id: row.enrolment_id,
        learner_name: identity.display_name,
        learner_number: identity.learner_number,
        draft_body: visible_draft_body(row.draft_body, include_draft_body),
        status: parse_submission_status(&row.status)?,
        version: row.version,
        current_submission_version_id: row.current_submission_version_id,
        versions,
        feedback,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }))
}

fn visible_draft_body(body: Option<String>, include: bool) -> Option<String> {
    include.then_some(body).flatten()
}

async fn feedback_for_version(
    pool: &PgPool,
    tenant_id: Uuid,
    submission_version_id: Uuid,
    include_draft: bool,
) -> Result<Option<LearningFeedbackResponse>> {
    let id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM learning_submission_reviews WHERE tenant_id=$1 AND submission_version_id=$2 AND ($3 OR status='released')",
    ).bind(tenant_id).bind(submission_version_id).bind(include_draft).fetch_optional(pool).await.context("load Learning feedback reference")?;
    match id {
        Some(id) => feedback_response_by_id(pool, tenant_id, id).await,
        None => Ok(None),
    }
}

async fn feedback_response_by_id(
    pool: &PgPool,
    tenant_id: Uuid,
    review_id: Uuid,
) -> Result<Option<LearningFeedbackResponse>> {
    let row = sqlx::query_as::<_, LearningFeedbackRow>(
        "SELECT id,submission_version_id,status,outcome,overall_feedback,total_score_hundredths,version,released_at FROM learning_submission_reviews WHERE tenant_id=$1 AND id=$2",
    ).bind(tenant_id).bind(review_id).fetch_optional(pool).await.context("load Learning feedback")?;
    let Some(row) = row else { return Ok(None) };
    let scores = sqlx::query_as::<_, LearningReviewScoreRow>(
        "SELECT rubric_criterion_id,earned_score_hundredths,feedback FROM learning_submission_review_scores WHERE tenant_id=$1 AND review_id=$2 ORDER BY created_at,id",
    ).bind(tenant_id).bind(review_id).fetch_all(pool).await.context("load Learning feedback scores")?
     .into_iter().map(|score| LearningReviewScoreResponse { rubric_criterion_id: score.rubric_criterion_id,
        earned_score_hundredths: score.earned_score_hundredths, feedback: score.feedback }).collect();
    Ok(Some(LearningFeedbackResponse {
        id: row.id,
        submission_version_id: row.submission_version_id,
        status: row.status,
        outcome: row
            .outcome
            .as_deref()
            .map(parse_review_outcome)
            .transpose()?,
        overall_feedback: row.overall_feedback,
        total_score_hundredths: row.total_score_hundredths,
        version: row.version,
        scores,
        released_at: row.released_at,
    }))
}

async fn validate_review_scores(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    assignment_id: Uuid,
    scores: &[crate::dtos::LearningRubricScoreInput],
) -> Result<()> {
    let mut seen = BTreeSet::new();
    for score in scores {
        if score.earned_score_hundredths < 0 {
            bail!("Rubric scores cannot be negative");
        }
        if score
            .feedback
            .as_ref()
            .is_some_and(|value| value.len() > 4000)
        {
            bail!("Rubric feedback must use no more than 4000 characters");
        }
        if !seen.insert(score.rubric_criterion_id) {
            bail!("Each rubric criterion may be scored only once");
        }
        let maximum = sqlx::query_scalar::<_, i32>(
            "SELECT max_score_hundredths FROM learning_assignment_rubric_criteria WHERE tenant_id=$1 AND learning_assignment_id=$2 AND id=$3 AND deleted_at IS NULL",
        ).bind(tenant_id).bind(assignment_id).bind(score.rubric_criterion_id).fetch_optional(&mut **tx).await
         .context("validate Learning rubric score")?.ok_or_else(|| anyhow!("A rubric criterion is not part of this assignment"))?;
        if score.earned_score_hundredths > maximum {
            bail!("A rubric score exceeds its criterion maximum");
        }
    }
    Ok(())
}

async fn progress_rows(
    pool: &PgPool,
    tenant_id: Uuid,
    space_id: Uuid,
    learner_id: Option<Uuid>,
) -> Result<Vec<LearningProgressRow>> {
    sqlx::query_as::<_, LearningProgressRow>(r#"
        SELECT recipient.learner_id,recipient.enrolment_id,COUNT(*)::BIGINT AS total_assignments,
               COUNT(*) FILTER (WHERE submission.id IS NULL)::BIGINT AS not_started,
               COUNT(*) FILTER (WHERE submission.status='draft')::BIGINT AS drafts,
               COUNT(*) FILTER (WHERE submission.status='submitted')::BIGINT AS awaiting_feedback,
               COUNT(*) FILTER (WHERE submission.status='revision_requested')::BIGINT AS revision_requested,
               COUNT(*) FILTER (WHERE submission.status='graded')::BIGINT AS graded,
               COUNT(*) FILTER (WHERE assignment.due_at < NOW() AND (submission.id IS NULL OR submission.status IN ('draft','revision_requested')))::BIGINT AS overdue,
               COALESCE(SUM(review.total_score_hundredths) FILTER (WHERE review.status='released' AND review.outcome='graded'),0)::BIGINT AS earned_score_hundredths,
               COALESCE(SUM(assignment.max_score_hundredths) FILTER (WHERE review.status='released' AND review.outcome='graded'),0)::BIGINT AS possible_score_hundredths
          FROM learning_assignment_recipients recipient
          JOIN learning_assignments assignment ON assignment.id=recipient.learning_assignment_id AND assignment.tenant_id=recipient.tenant_id
          JOIN learning_units unit ON unit.id=assignment.learning_unit_id AND unit.tenant_id=assignment.tenant_id
          LEFT JOIN learning_submissions submission ON submission.tenant_id=recipient.tenant_id AND submission.learning_assignment_id=assignment.id AND submission.assignment_recipient_id=recipient.id AND submission.deleted_at IS NULL
          LEFT JOIN learning_submission_reviews review ON review.tenant_id=submission.tenant_id AND review.submission_version_id=submission.current_submission_version_id
         WHERE recipient.tenant_id=$1 AND unit.learning_space_id=$2 AND assignment.status IN ('published','closed')
           AND assignment.deleted_at IS NULL AND unit.deleted_at IS NULL AND ($3::UUID IS NULL OR recipient.learner_id=$3)
         GROUP BY recipient.learner_id,recipient.enrolment_id ORDER BY recipient.learner_id,recipient.enrolment_id
    "#).bind(tenant_id).bind(space_id).bind(learner_id).fetch_all(pool).await.context("calculate Learning progress")
}

async fn progress_response(
    pool: &PgPool,
    tenant_id: Uuid,
    row: LearningProgressRow,
) -> Result<LearningProgressEntry> {
    let identity =
        EnrolmentOps::roster_references_by_enrolment_ids(pool, tenant_id, &[row.enrolment_id])
            .await?
            .into_iter()
            .find(|entry| entry.learner_id == row.learner_id)
            .ok_or_else(|| {
                anyhow!("The SIS learner identity for this progress record is unavailable")
            })?;
    let completion_percent = if row.total_assignments == 0 {
        0
    } else {
        i32::try_from((row.graded * 100) / row.total_assignments)
            .context("Learning completion percentage exceeded its supported range")?
    };
    Ok(LearningProgressEntry {
        learner_id: row.learner_id,
        enrolment_id: row.enrolment_id,
        learner_name: identity.display_name,
        learner_number: identity.learner_number,
        total_assignments: row.total_assignments,
        not_started: row.not_started,
        drafts: row.drafts,
        awaiting_feedback: row.awaiting_feedback,
        revision_requested: row.revision_requested,
        graded: row.graded,
        overdue: row.overdue,
        completion_percent,
        earned_score_hundredths: row.earned_score_hundredths,
        possible_score_hundredths: row.possible_score_hundredths,
    })
}

fn empty_progress(identity: ClassRosterEntry) -> LearningProgressEntry {
    LearningProgressEntry {
        learner_id: identity.learner_id,
        enrolment_id: identity.enrolment_id,
        learner_name: identity.display_name,
        learner_number: identity.learner_number,
        total_assignments: 0,
        not_started: 0,
        drafts: 0,
        awaiting_feedback: 0,
        revision_requested: 0,
        graded: 0,
        overdue: 0,
        completion_percent: 0,
        earned_score_hundredths: 0,
        possible_score_hundredths: 0,
    }
}

async fn require_draft_space(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    space_id: Uuid,
) -> Result<()> {
    let status = sqlx::query_scalar::<_, String>(
        "SELECT status FROM learning_spaces WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(space_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("lock Learning space")?;
    match status.as_deref() {
        Some("draft") => Ok(()),
        Some(_) => bail!("A published Learning space is immutable"),
        None => bail!("The Learning space is unavailable"),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "domain and actor evidence are intentionally explicit"
)]
async fn append_evidence(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    aggregate_type: &str,
    aggregate_id: Uuid,
    learning_space_id: Option<Uuid>,
    event_type: &str,
    action: &str,
    metadata: Value,
) -> Result<()> {
    let actor_id = person_actor_id(actor)?;
    sqlx::query(
        "INSERT INTO learning_activity_events (tenant_id,aggregate_type,aggregate_id,learning_space_id,event_type,actor_id,metadata) VALUES ($1,$2,$3,$4,$5,$6,$7)",
    ).bind(tenant_id).bind(aggregate_type).bind(aggregate_id).bind(learning_space_id)
     .bind(event_type).bind(actor_id).bind(&metadata).execute(&mut **transaction).await
     .context("append Learning activity")?;
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
        .with_target(AuditTarget::new(aggregate_type, aggregate_id.to_string()))
        .with_redacted_metadata(metadata),
    )
    .await
    .context("append Learning audit evidence")?;
    Ok(())
}

fn parse_space_status(value: &str) -> Result<LearningSpaceStatus> {
    match value {
        "draft" => Ok(LearningSpaceStatus::Draft),
        "published" => Ok(LearningSpaceStatus::Published),
        "archived" => Ok(LearningSpaceStatus::Archived),
        _ => bail!("Stored Learning space status is invalid"),
    }
}

fn parse_unit_status(value: &str) -> Result<LearningUnitStatus> {
    match value {
        "draft" => Ok(LearningUnitStatus::Draft),
        "published" => Ok(LearningUnitStatus::Published),
        "withdrawn" => Ok(LearningUnitStatus::Withdrawn),
        _ => bail!("Stored Learning unit status is invalid"),
    }
}

fn parse_resource_status(value: &str) -> Result<LearningResourceStatus> {
    match value {
        "draft" => Ok(LearningResourceStatus::Draft),
        "published" => Ok(LearningResourceStatus::Published),
        "withdrawn" => Ok(LearningResourceStatus::Withdrawn),
        _ => bail!("Stored Learning resource status is invalid"),
    }
}

fn parse_assignment_status(value: &str) -> Result<LearningAssignmentStatus> {
    match value {
        "draft" => Ok(LearningAssignmentStatus::Draft),
        "published" => Ok(LearningAssignmentStatus::Published),
        "closed" => Ok(LearningAssignmentStatus::Closed),
        _ => bail!("Stored Learning assignment status is invalid"),
    }
}

fn parse_submission_status(value: &str) -> Result<LearningSubmissionStatus> {
    match value {
        "draft" => Ok(LearningSubmissionStatus::Draft),
        "submitted" => Ok(LearningSubmissionStatus::Submitted),
        "revision_requested" => Ok(LearningSubmissionStatus::RevisionRequested),
        "graded" => Ok(LearningSubmissionStatus::Graded),
        _ => bail!("Stored Learning submission status is invalid"),
    }
}

fn parse_review_outcome(value: &str) -> Result<LearningReviewOutcome> {
    match value {
        "graded" => Ok(LearningReviewOutcome::Graded),
        "revision_requested" => Ok(LearningReviewOutcome::RevisionRequested),
        _ => bail!("Stored Learning review outcome is invalid"),
    }
}

fn person_actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("Authenticated person actor is required"))
}

fn required<'a>(field: &str, value: &'a str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{field} is required");
    }
    Ok(value)
}

fn optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn like_query(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{}%", value.replace('%', "\\%").replace('_', "\\_")))
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(DEFAULT_PAGE).max(1),
        per_page.unwrap_or(DEFAULT_PER_PAGE).clamp(1, MAX_PER_PAGE),
    )
}

fn database_error(error: sqlx::Error, context: &str) -> anyhow::Error {
    if let sqlx::Error::Database(database) = &error {
        return match database.constraint() {
            Some("idx_learning_spaces_assignment_term") => {
                anyhow!("A Learning space already exists for this assignment and term")
            }
            Some("idx_learning_units_position") => {
                anyhow!("Another unit already uses this position")
            }
            Some("idx_learning_resources_file") => {
                anyhow!("This governed file is already linked to the unit")
            }
            Some("idx_learning_resources_position") => {
                anyhow!("Another resource already uses this position")
            }
            Some("idx_learning_assignments_position") => {
                anyhow!("Another assignment already uses this position")
            }
            Some("idx_learning_rubric_position") => {
                anyhow!("Another rubric criterion already uses this position")
            }
            _ => anyhow!("{context}: {error}"),
        };
    }
    anyhow!("{context}: {error}")
}

const SPACE_LIST_SQL: &str = concat!(
    "SELECT space.id,space.teaching_assignment_id,space.academic_year_id,space.academic_term_id,space.class_group_id,space.title,space.summary,space.status,space.version,space.published_at,space.archived_at,space.archive_reason,space.created_at,space.updated_at,COUNT(unit.id)::BIGINT AS unit_count,COUNT(unit.id) FILTER (WHERE unit.status='published')::BIGINT AS published_unit_count FROM learning_spaces AS space LEFT JOIN learning_units AS unit ON unit.tenant_id=space.tenant_id AND unit.learning_space_id=space.id AND unit.deleted_at IS NULL ",
    "WHERE space.tenant_id=$1 AND space.deleted_at IS NULL AND ($2::TEXT IS NULL OR space.status=$2) ",
    "AND ($3::TEXT IS NULL OR space.title ILIKE $3 ESCAPE '\\') AND ( ",
    "$4::TEXT='campus' OR ($4='assigned' AND space.teaching_assignment_id=ANY($5)) ",
    "OR ($4='self' AND space.status='published' AND space.academic_year_id=$6 AND space.class_group_id=ANY($7)) ",
    "OR ($4='self_and_assigned' AND (space.teaching_assignment_id=ANY($5) OR (space.status='published' AND space.academic_year_id=$6 AND space.class_group_id=ANY($7)))) ",
    ") ",
    "GROUP BY space.id ORDER BY space.updated_at DESC,space.id LIMIT $8 OFFSET $9"
);

const SPACE_COUNT_SQL: &str = concat!(
    "SELECT COUNT(*) FROM learning_spaces AS space WHERE space.tenant_id=$1 AND space.deleted_at IS NULL ",
    "AND ($2::TEXT IS NULL OR space.status=$2) AND ($3::TEXT IS NULL OR space.title ILIKE $3 ESCAPE '\\') AND ( ",
    "$4::TEXT='campus' OR ($4='assigned' AND space.teaching_assignment_id=ANY($5)) ",
    "OR ($4='self' AND space.status='published' AND space.academic_year_id=$6 AND space.class_group_id=ANY($7)) ",
    "OR ($4='self_and_assigned' AND (space.teaching_assignment_id=ANY($5) OR (space.status='published' AND space.academic_year_id=$6 AND space.class_group_id=ANY($7)))) ",
    ")"
);

const SPACE_BY_ID_SQL: &str = concat!(
    "SELECT space.id,space.teaching_assignment_id,space.academic_year_id,space.academic_term_id,space.class_group_id,space.title,space.summary,space.status,space.version,space.published_at,space.archived_at,space.archive_reason,space.created_at,space.updated_at,COUNT(unit.id)::BIGINT AS unit_count,COUNT(unit.id) FILTER (WHERE unit.status='published')::BIGINT AS published_unit_count FROM learning_spaces AS space LEFT JOIN learning_units AS unit ON unit.tenant_id=space.tenant_id AND unit.learning_space_id=space.id AND unit.deleted_at IS NULL ",
    "WHERE space.tenant_id=$1 AND space.id=$2 AND space.deleted_at IS NULL GROUP BY space.id"
);

#[cfg(test)]
mod tests {
    use super::{
        bounded_page, feedback_release_fingerprint, like_query, parse_assignment_status,
        parse_resource_status, parse_review_outcome, parse_space_status, parse_submission_status,
        parse_unit_status, submission_fingerprint, visible_draft_body,
    };
    use crate::{
        LearningAssignmentStatus, LearningResourceStatus, LearningReviewOutcome,
        LearningSpaceStatus, LearningSubmissionStatus, LearningUnitStatus,
    };
    use uuid::Uuid;

    #[test]
    fn status_parsers_reject_unknown_database_values() {
        assert_eq!(
            parse_space_status("draft").ok(),
            Some(LearningSpaceStatus::Draft)
        );
        assert_eq!(
            parse_unit_status("published").ok(),
            Some(LearningUnitStatus::Published)
        );
        assert_eq!(
            parse_resource_status("withdrawn").ok(),
            Some(LearningResourceStatus::Withdrawn)
        );
        assert!(parse_space_status("open").is_err());
        assert!(parse_unit_status("archived").is_err());
        assert!(parse_resource_status("deleted").is_err());
        assert_eq!(
            parse_assignment_status("closed").ok(),
            Some(LearningAssignmentStatus::Closed)
        );
        assert_eq!(
            parse_submission_status("revision_requested").ok(),
            Some(LearningSubmissionStatus::RevisionRequested)
        );
        assert_eq!(
            parse_review_outcome("graded").ok(),
            Some(LearningReviewOutcome::Graded)
        );
        assert!(parse_assignment_status("archived").is_err());
        assert!(parse_submission_status("withdrawn").is_err());
        assert!(parse_review_outcome("draft").is_err());
    }

    #[test]
    fn list_boundaries_are_bounded_and_search_is_escaped() {
        assert_eq!(bounded_page(None, None), (1, 25));
        assert_eq!(bounded_page(Some(0), Some(500)), (1, 100));
        assert_eq!(
            like_query(Some(" 100%_ready ")).as_deref(),
            Some("%100\\%\\_ready%")
        );
        assert_eq!(like_query(Some("  ")), None);
    }

    #[test]
    fn learner_draft_body_is_visible_only_to_self_hydration() {
        let body = Some("work in progress".to_string());
        assert_eq!(visible_draft_body(body.clone(), true), body);
        assert_eq!(visible_draft_body(body, false), None);
    }

    #[test]
    fn idempotency_fingerprints_are_deterministic_and_request_specific() {
        let aggregate_id = Uuid::from_u128(1);
        assert_eq!(
            submission_fingerprint(aggregate_id, 2, "answer"),
            submission_fingerprint(aggregate_id, 2, "answer")
        );
        assert_ne!(
            submission_fingerprint(aggregate_id, 2, "answer"),
            submission_fingerprint(aggregate_id, 3, "answer")
        );
        assert_ne!(
            feedback_release_fingerprint(aggregate_id, 2, LearningReviewOutcome::Graded),
            feedback_release_fingerprint(aggregate_id, 2, LearningReviewOutcome::RevisionRequested)
        );
    }
}
