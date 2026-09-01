//! Transactional E-learning space, unit, and governed-resource operations.
//!
//! Every query applies the caller's current campus, assigned-teacher, or
//! learner-self scope before projection. Published records are immutable and
//! move only to an explicit retained terminal state.

use anyhow::{Context, Result, anyhow, bail};
use cp_academics::ops::{AcademicTermOps, AcademicYearOps, TeachingAssignmentOps};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_document_registry::{DocumentRegistryOps, EvidenceFileReference};
use cp_sis::ops::EnrolmentOps;
use serde_json::{Map, Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::dtos::{
    CreateLearningResourceRequest, CreateLearningSpaceRequest, CreateLearningUnitRequest,
    LearningAccessScope, LearningAssignmentReference, LearningReferenceData,
    LearningResourceCreation, LearningResourceFileQuery, LearningResourceResponse,
    LearningResourceStatus, LearningSettingsResponse, LearningSpaceListQuery,
    LearningSpaceResponse, LearningSpaceStatus, LearningSpaceSummary, LearningTermReference,
    LearningUnitResponse, LearningUnitStatus, ReasonedLearningTransitionRequest,
    UpdateLearningResourceRequest, UpdateLearningSettingsRequest, UpdateLearningSpaceRequest,
    UpdateLearningUnitRequest,
};
use crate::models::{LearningResourceRow, LearningSettingsRow, LearningSpaceRow, LearningUnitRow};

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
            Some(id) => DocumentRegistryOps::get_series(pool, tenant_id, id)
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
            let series = DocumentRegistryOps::get_series(pool, tenant_id, series_id)
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
        bounded_page, like_query, parse_resource_status, parse_space_status, parse_unit_status,
    };
    use crate::{LearningResourceStatus, LearningSpaceStatus, LearningUnitStatus};

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
}
