//! Tenant-scoped Document Registry operations and closed lifecycle transitions.

use anyhow::{Context, Result, anyhow, bail};
use chrono::{Months, NaiveDate, Utc};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    ActivityResponse, CloseFileRequest, CreateReviewRequest, CreateSeriesRequest, DocumentStorage,
    EvidenceFileReference, ExecuteDestructionRequest, FileResponse, NewRegistryFile,
    NumberingPolicyResponse, ReclassifyFileRequest, RegistryListQuery, ReviewDecisionRequest,
    ReviewResponse, SeriesResponse, UpdateFileRequest, UpdateNumberingPolicyRequest,
    UpdateSeriesRequest,
    models::{ActivityRow, FileRow, NumberingPolicyRow, ReviewRow, SeriesRow},
};

pub struct DocumentRegistryOps;

impl DocumentRegistryOps {
    /// Resolves the minimum current metadata required for a governed evidence link.
    /// Destroyed files and restricted files outside the caller's authority are absent.
    pub async fn evidence_reference<'e, E>(
        executor: E,
        tenant_id: Uuid,
        file_id: Uuid,
        can_view_restricted: bool,
    ) -> Result<Option<EvidenceFileReference>>
    where
        E: sqlx::Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query_as::<_, (Uuid, String, String, String, String)>(
            r#"
            SELECT id, reference, title, sensitivity, status
            FROM document_registry_files
            WHERE tenant_id = $1
              AND id = $2
              AND deleted_at IS NULL
              AND status <> 'destroyed'
              AND ($3 OR sensitivity <> 'restricted')
            FOR SHARE
            "#,
        )
        .bind(tenant_id)
        .bind(file_id)
        .bind(can_view_restricted)
        .fetch_optional(executor)
        .await
        .context("Failed to resolve governed document evidence")
        .map(|row| {
            row.map(
                |(id, reference, title, sensitivity, status)| EvidenceFileReference {
                    id,
                    reference,
                    title,
                    sensitivity,
                    status,
                },
            )
        })
    }

    pub async fn numbering_policy(
        pool: &PgPool,
        tenant_id: Uuid,
    ) -> Result<NumberingPolicyResponse> {
        let row = sqlx::query_as::<_, NumberingPolicyRow>(
            "SELECT prefix,padding,next_sequence,version,updated_at FROM document_registry_numbering_policies WHERE tenant_id=$1 AND deleted_at IS NULL",
        ).bind(tenant_id).fetch_one(pool).await.context("load Document Registry numbering")?;
        Ok(numbering_response(row))
    }

    pub async fn update_numbering_policy(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &UpdateNumberingPolicyRequest,
    ) -> Result<Option<NumberingPolicyResponse>> {
        let actor_id = person_actor_id(actor)?;
        let prefix = required("Number prefix", &request.prefix)?;
        let minimum = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT MAX((regexp_match(reference, '([0-9]+)$'))[1]::BIGINT)+1 FROM document_registry_files WHERE tenant_id=$1",
        ).bind(tenant_id).fetch_one(pool).await.context("resolve the used document number boundary")?.unwrap_or(1);
        if request.next_sequence < minimum {
            bail!("Next sequence cannot be below the current boundary of {minimum}");
        }
        let mut tx = pool
            .begin()
            .await
            .context("start Document Registry numbering update")?;
        let row = sqlx::query_as::<_, NumberingPolicyRow>(
            r#"UPDATE document_registry_numbering_policies SET prefix=$3,padding=$4,next_sequence=$5,
                   version=version+1 WHERE tenant_id=$1 AND version=$2 AND deleted_at IS NULL
               RETURNING prefix,padding,next_sequence,version,updated_at"#,
        ).bind(tenant_id).bind(request.version).bind(prefix).bind(request.padding)
          .bind(request.next_sequence).fetch_optional(&mut *tx).await.context("update Document Registry numbering")?;
        if row.is_some() {
            append_evidence(
                &mut tx,
                tenant_id,
                actor,
                context,
                "numbering_policy",
                tenant_id,
                None,
                "numbering_policy_updated",
                "document_registry.numbering_policy.update",
                json!({"actor_id": actor_id}),
            )
            .await?;
            tx.commit()
                .await
                .context("commit Document Registry numbering update")?;
        }
        Ok(row.map(numbering_response))
    }

    pub async fn list_series(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &RegistryListQuery,
    ) -> Result<(Vec<SeriesResponse>, i64)> {
        validate_optional(
            query.status.as_deref(),
            &["active", "inactive"],
            "classification status",
        )?;
        let (page, per_page) = bounded_page(query);
        let search = like_query(query.search.as_deref());
        let rows = sqlx::query_as::<_, SeriesRow>(SERIES_SELECT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(search.as_deref())
            .bind(per_page)
            .bind((page - 1) * per_page)
            .fetch_all(pool)
            .await
            .context("list Document Registry classifications")?;
        let total = sqlx::query_scalar::<_, i64>(SERIES_COUNT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(search.as_deref())
            .fetch_one(pool)
            .await
            .context("count Document Registry classifications")?;
        Ok((rows.into_iter().map(series_response).collect(), total))
    }

    pub async fn get_series(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<SeriesResponse>> {
        sqlx::query_as::<_, SeriesRow>(SERIES_BY_ID)
            .bind(tenant_id)
            .bind(id)
            .fetch_optional(pool)
            .await
            .context("load Document Registry classification")
            .map(|row| row.map(series_response))
    }

    pub async fn create_series(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &CreateSeriesRequest,
    ) -> Result<SeriesResponse> {
        let actor_id = person_actor_id(actor)?;
        validate_series(
            &request.retention_trigger,
            request.retention_period_months,
            &request.final_disposition,
            &request.default_sensitivity,
            None,
        )?;
        let id = Uuid::new_v4();
        let mut tx = pool
            .begin()
            .await
            .context("start classification creation")?;
        sqlx::query(
            r#"INSERT INTO document_registry_series
               (id,tenant_id,code,name,description,retention_trigger,retention_period_months,
                final_disposition,default_sensitivity,created_by,updated_by)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$10)"#,
        )
        .bind(id)
        .bind(tenant_id)
        .bind(required("Classification code", &request.code)?)
        .bind(required("Classification name", &request.name)?)
        .bind(clean(request.description.as_deref()))
        .bind(&request.retention_trigger)
        .bind(request.retention_period_months)
        .bind(&request.final_disposition)
        .bind(&request.default_sensitivity)
        .bind(actor_id)
        .execute(&mut *tx)
        .await
        .context("create Document Registry classification")?;
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            context,
            "series",
            id,
            None,
            "classification_created",
            "document_registry.series.create",
            json!({"code": request.code.trim()}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit classification creation")?;
        Self::get_series(pool, tenant_id, id)
            .await?
            .ok_or_else(|| anyhow!("The classification could not be reloaded"))
    }

    pub async fn update_series(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &UpdateSeriesRequest,
    ) -> Result<Option<SeriesResponse>> {
        let actor_id = person_actor_id(actor)?;
        validate_series(
            &request.retention_trigger,
            request.retention_period_months,
            &request.final_disposition,
            &request.default_sensitivity,
            Some(&request.status),
        )?;
        let mut tx = pool.begin().await.context("start classification update")?;
        let updated = sqlx::query_scalar::<_, Uuid>(
            r#"UPDATE document_registry_series SET code=$4,name=$5,description=$6,
                   retention_trigger=$7,retention_period_months=$8,final_disposition=$9,
                   default_sensitivity=$10,status=$11,version=version+1,updated_by=$12
               WHERE tenant_id=$1 AND id=$2 AND version=$3 AND deleted_at IS NULL RETURNING id"#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(request.version)
        .bind(required("Classification code", &request.code)?)
        .bind(required("Classification name", &request.name)?)
        .bind(clean(request.description.as_deref()))
        .bind(&request.retention_trigger)
        .bind(request.retention_period_months)
        .bind(&request.final_disposition)
        .bind(&request.default_sensitivity)
        .bind(&request.status)
        .bind(actor_id)
        .fetch_optional(&mut *tx)
        .await
        .context("update Document Registry classification")?;
        if updated.is_some() {
            append_evidence(
                &mut tx,
                tenant_id,
                actor,
                context,
                "series",
                id,
                None,
                "classification_updated",
                "document_registry.series.update",
                json!({"version": request.version}),
            )
            .await?;
            tx.commit().await.context("commit classification update")?;
            return Self::get_series(pool, tenant_id, id).await;
        }
        Ok(None)
    }

    pub async fn list_files(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &RegistryListQuery,
        can_view_restricted: bool,
    ) -> Result<(Vec<FileResponse>, i64)> {
        validate_optional(
            query.status.as_deref(),
            &["filed", "closed", "destroyed"],
            "document status",
        )?;
        validate_optional(
            query.sensitivity.as_deref(),
            SENSITIVITIES,
            "document sensitivity",
        )?;
        let (page, per_page) = bounded_page(query);
        let search = like_query(query.search.as_deref());
        let rows = sqlx::query_as::<_, FileRow>(FILE_SELECT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(query.series_id)
            .bind(query.sensitivity.as_deref())
            .bind(search.as_deref())
            .bind(can_view_restricted)
            .bind(per_page)
            .bind((page - 1) * per_page)
            .fetch_all(pool)
            .await
            .context("list Document Registry files")?;
        let total = sqlx::query_scalar::<_, i64>(FILE_COUNT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(query.series_id)
            .bind(query.sensitivity.as_deref())
            .bind(search.as_deref())
            .bind(can_view_restricted)
            .fetch_one(pool)
            .await
            .context("count Document Registry files")?;
        Ok((rows.into_iter().map(file_response).collect(), total))
    }

    pub async fn get_file(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        can_view_restricted: bool,
    ) -> Result<Option<FileResponse>> {
        sqlx::query_as::<_, FileRow>(FILE_BY_ID)
            .bind(tenant_id)
            .bind(id)
            .bind(can_view_restricted)
            .fetch_optional(pool)
            .await
            .context("load Document Registry file")
            .map(|row| row.map(file_response))
    }

    pub async fn create_file(
        pool: &PgPool,
        storage: &DocumentStorage,
        tenant_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: NewRegistryFile,
    ) -> Result<FileResponse> {
        let actor_id = person_actor_id(actor)?;
        let title = required("Document title", &request.title)?;
        validate_sensitivity(request.sensitivity.as_deref())?;
        let series = sqlx::query_as::<_, SeriesRow>(SERIES_BY_ID)
            .bind(tenant_id)
            .bind(request.series_id)
            .fetch_optional(pool)
            .await
            .context("load the selected classification")?
            .ok_or_else(|| anyhow!("The selected classification was not found"))?;
        if series.status != "active" {
            bail!("The selected classification is inactive");
        }
        let sensitivity = request
            .sensitivity
            .as_deref()
            .unwrap_or(&series.default_sensitivity)
            .to_string();
        let file_id = Uuid::new_v4();
        let sha256_hex = format!("{:x}", Sha256::digest(&request.bytes));
        let object_key = storage
            .scan_and_store(tenant_id, file_id, &request.bytes, &request.media_type)
            .await?;
        let stored_key = object_key.clone();
        let result = async {
            let mut tx = pool.begin().await.context("start document filing")?;
            let policy = sqlx::query_as::<_, NumberingPolicyRow>(
                "SELECT prefix,padding,next_sequence,version,updated_at FROM document_registry_numbering_policies WHERE tenant_id=$1 AND deleted_at IS NULL FOR UPDATE",
            ).bind(tenant_id).fetch_one(&mut *tx).await.context("lock Document Registry numbering")?;
            let reference = format!("{}{:0width$}", policy.prefix, policy.next_sequence, width=policy.padding as usize);
            sqlx::query("UPDATE document_registry_numbering_policies SET next_sequence=next_sequence+1,version=version+1 WHERE tenant_id=$1")
                .bind(tenant_id).execute(&mut *tx).await.context("reserve the document reference")?;
            let filed_on = Utc::now().date_naive();
            let retain_until = if series.retention_trigger == "filed" {
                retention_date(filed_on, series.retention_period_months)?
            } else { None };
            sqlx::query(
                r#"INSERT INTO document_registry_files
                   (id,tenant_id,reference,series_id,series_code_snapshot,series_name_snapshot,
                    retention_trigger_snapshot,retention_period_months_snapshot,final_disposition_snapshot,
                    sensitivity,title,description,document_date,filed_on,retain_until,original_file_name,
                    media_type,byte_size,sha256_hex,object_key,scanned_at,created_by,updated_by)
                   VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,NOW(),$21,$21)"#,
            ).bind(file_id).bind(tenant_id).bind(&reference).bind(series.id).bind(&series.code).bind(&series.name)
              .bind(&series.retention_trigger).bind(series.retention_period_months).bind(&series.final_disposition)
              .bind(&sensitivity).bind(title).bind(clean(request.description.as_deref())).bind(request.document_date)
              .bind(filed_on).bind(retain_until).bind(safe_file_name(&request.original_file_name)?)
              .bind(&request.media_type).bind(request.bytes.len() as i64).bind(&sha256_hex).bind(&object_key).bind(actor_id)
              .execute(&mut *tx).await.context("file the private document")?;
            append_evidence(&mut tx, tenant_id, actor, context, "file", file_id, Some(file_id),
                "document_filed", "document_registry.file.create",
                json!({"reference": reference,"series_code": series.code,"media_type": request.media_type,"byte_size": request.bytes.len()})).await?;
            tx.commit().await.context("commit document filing")?;
            Self::get_file(pool, tenant_id, file_id, true).await?.ok_or_else(|| anyhow!("The document could not be reloaded"))
        }.await;
        if result.is_err() {
            let _ = storage.delete(&stored_key).await;
        }
        result
    }

    pub async fn update_file(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &UpdateFileRequest,
    ) -> Result<Option<FileResponse>> {
        validate_sensitivity(Some(&request.sensitivity))?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("start document metadata update")?;
        let changed = sqlx::query_scalar::<_, Uuid>(
            r#"UPDATE document_registry_files SET title=$4,description=$5,document_date=$6,
                   sensitivity=$7,version=version+1,updated_by=$8
               WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status <> 'destroyed' AND deleted_at IS NULL RETURNING id"#,
        ).bind(tenant_id).bind(id).bind(request.version).bind(required("Document title", &request.title)?)
          .bind(clean(request.description.as_deref())).bind(request.document_date).bind(&request.sensitivity).bind(actor_id)
          .fetch_optional(&mut *tx).await.context("update document metadata")?;
        if changed.is_some() {
            append_evidence(
                &mut tx,
                tenant_id,
                actor,
                context,
                "file",
                id,
                Some(id),
                "document_metadata_updated",
                "document_registry.file.update",
                json!({"version": request.version}),
            )
            .await?;
            tx.commit()
                .await
                .context("commit document metadata update")?;
            return Self::get_file(pool, tenant_id, id, true).await;
        }
        Ok(None)
    }

    pub async fn reclassify_file(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &ReclassifyFileRequest,
    ) -> Result<Option<FileResponse>> {
        let actor_id = person_actor_id(actor)?;
        validate_sensitivity(request.sensitivity.as_deref())?;
        let series = sqlx::query_as::<_, SeriesRow>(SERIES_BY_ID)
            .bind(tenant_id)
            .bind(request.series_id)
            .fetch_optional(pool)
            .await
            .context("load replacement classification")?
            .ok_or_else(|| anyhow!("The selected classification was not found"))?;
        if series.status != "active" {
            bail!("The selected classification is inactive");
        }
        let sensitivity = request
            .sensitivity
            .as_deref()
            .unwrap_or(&series.default_sensitivity);
        let existing = Self::get_file(pool, tenant_id, id, true).await?;
        let Some(existing) = existing else {
            return Ok(None);
        };
        if existing.version != request.version || existing.status == "destroyed" {
            return Ok(None);
        }
        let base_date = if series.retention_trigger == "filed" {
            Some(existing.filed_on)
        } else {
            existing.closed_at.map(|value| value.date_naive())
        };
        let retain_until = match base_date {
            Some(date) => retention_date(date, series.retention_period_months)?,
            None => None,
        };
        let mut tx = pool
            .begin()
            .await
            .context("start document reclassification")?;
        let changed = sqlx::query_scalar::<_, Uuid>(
            r#"UPDATE document_registry_files SET series_id=$4,series_code_snapshot=$5,series_name_snapshot=$6,
                   retention_trigger_snapshot=$7,retention_period_months_snapshot=$8,final_disposition_snapshot=$9,
                   sensitivity=$10,retain_until=$11,version=version+1,updated_by=$12
               WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status <> 'destroyed' AND deleted_at IS NULL RETURNING id"#,
        ).bind(tenant_id).bind(id).bind(request.version).bind(series.id).bind(&series.code).bind(&series.name)
          .bind(&series.retention_trigger).bind(series.retention_period_months).bind(&series.final_disposition)
          .bind(sensitivity).bind(retain_until).bind(actor_id).fetch_optional(&mut *tx).await
          .context("reclassify the document")?;
        if changed.is_some() {
            append_evidence(&mut tx, tenant_id, actor, context, "file", id, Some(id),
                "document_reclassified", "document_registry.file.reclassify",
                json!({"from_series_id": existing.series_id,"to_series_id": series.id,"reason": request.reason.trim()})).await?;
            tx.commit()
                .await
                .context("commit document reclassification")?;
            return Self::get_file(pool, tenant_id, id, true).await;
        }
        Ok(None)
    }

    pub async fn close_file(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &CloseFileRequest,
    ) -> Result<Option<FileResponse>> {
        let actor_id = person_actor_id(actor)?;
        let current = Self::get_file(pool, tenant_id, id, true).await?;
        let Some(current) = current else {
            return Ok(None);
        };
        if current.version != request.version || current.status != "filed" {
            return Ok(None);
        }
        let closed_on = Utc::now().date_naive();
        let retain_until = if current.retention_trigger == "closed" {
            retention_date(closed_on, current.retention_period_months)?
        } else {
            current.retain_until
        };
        let mut tx = pool.begin().await.context("start document closure")?;
        let changed = sqlx::query_scalar::<_, Uuid>(
            r#"UPDATE document_registry_files SET status='closed',closed_by=$4,closed_at=NOW(),
                   close_reason=$5,retain_until=$6,version=version+1,updated_by=$4
               WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='filed' AND deleted_at IS NULL RETURNING id"#,
        ).bind(tenant_id).bind(id).bind(request.version).bind(actor_id).bind(required("Closure reason", &request.reason)?)
          .bind(retain_until).fetch_optional(&mut *tx).await.context("close the document")?;
        if changed.is_some() {
            append_evidence(
                &mut tx,
                tenant_id,
                actor,
                context,
                "file",
                id,
                Some(id),
                "document_closed",
                "document_registry.file.close",
                json!({"reason": request.reason.trim(),"retain_until": retain_until}),
            )
            .await?;
            tx.commit().await.context("commit document closure")?;
            return Self::get_file(pool, tenant_id, id, true).await;
        }
        Ok(None)
    }

    pub async fn activity(
        pool: &PgPool,
        tenant_id: Uuid,
        file_id: Uuid,
        can_view_restricted: bool,
    ) -> Result<Vec<ActivityResponse>> {
        if Self::get_file(pool, tenant_id, file_id, can_view_restricted)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }
        sqlx::query_as::<_, ActivityRow>(
            "SELECT id,aggregate_type,aggregate_id,file_id,event_type,actor_id,metadata,created_at FROM document_registry_activity_events WHERE tenant_id=$1 AND file_id=$2 ORDER BY created_at DESC,id DESC",
        ).bind(tenant_id).bind(file_id).fetch_all(pool).await.context("load document activity")
          .map(|rows| rows.into_iter().map(activity_response).collect())
    }

    pub async fn object_key(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        can_view_restricted: bool,
    ) -> Result<Option<String>> {
        sqlx::query_scalar::<_, String>(
            "SELECT object_key FROM document_registry_files WHERE tenant_id=$1 AND id=$2 AND status <> 'destroyed' AND object_key IS NOT NULL AND deleted_at IS NULL AND ($3 OR sensitivity <> 'restricted')",
        ).bind(tenant_id).bind(id).bind(can_view_restricted).fetch_optional(pool).await.context("authorize private document download")
    }

    pub async fn retention_due(
        pool: &PgPool,
        tenant_id: Uuid,
        can_view_restricted: bool,
    ) -> Result<Vec<FileResponse>> {
        sqlx::query_as::<_, FileRow>(
            &format!("{} AND file.status='closed' AND file.retain_until <= CURRENT_DATE AND file.final_disposition_snapshot <> 'permanent' AND NOT EXISTS (SELECT 1 FROM document_registry_disposition_reviews review WHERE review.tenant_id=file.tenant_id AND review.file_id=file.id AND review.deleted_at IS NULL AND review.status IN ('pending','approved')) ORDER BY file.retain_until,file.reference LIMIT 200", FILE_BASE),
        ).bind(tenant_id).bind(can_view_restricted).fetch_all(pool).await.context("load retention-due documents")
          .map(|rows| rows.into_iter().map(file_response).collect())
    }

    pub async fn list_reviews(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &RegistryListQuery,
        can_view_restricted: bool,
    ) -> Result<(Vec<ReviewResponse>, i64)> {
        validate_optional(
            query.status.as_deref(),
            &["pending", "approved", "rejected", "executed"],
            "review status",
        )?;
        let (page, per_page) = bounded_page(query);
        let rows = sqlx::query_as::<_, ReviewRow>(REVIEW_SELECT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(can_view_restricted)
            .bind(per_page)
            .bind((page - 1) * per_page)
            .fetch_all(pool)
            .await
            .context("list disposition reviews")?;
        let total = sqlx::query_scalar::<_, i64>(REVIEW_COUNT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(can_view_restricted)
            .fetch_one(pool)
            .await
            .context("count disposition reviews")?;
        Ok((rows.into_iter().map(review_response).collect(), total))
    }

    pub async fn get_review(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        can_view_restricted: bool,
    ) -> Result<Option<ReviewResponse>> {
        sqlx::query_as::<_, ReviewRow>(REVIEW_BY_ID)
            .bind(tenant_id)
            .bind(id)
            .bind(can_view_restricted)
            .fetch_optional(pool)
            .await
            .context("load disposition review")
            .map(|row| row.map(review_response))
    }

    pub async fn create_review(
        pool: &PgPool,
        tenant_id: Uuid,
        file_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &CreateReviewRequest,
    ) -> Result<ReviewResponse> {
        let actor_id = person_actor_id(actor)?;
        if !["retain", "destroy"].contains(&request.recommendation.as_str()) {
            bail!("Choose retain or destroy");
        }
        if request.recommendation == "retain" && request.proposed_retain_until.is_none() {
            bail!("Choose a new retention date");
        }
        if request.recommendation == "destroy" && request.proposed_retain_until.is_some() {
            bail!("A destruction request cannot include a new retention date");
        }
        let file = Self::get_file(pool, tenant_id, file_id, true)
            .await?
            .ok_or_else(|| anyhow!("The document was not found"))?;
        if file.version != request.file_version {
            bail!("The document changed since it was loaded");
        }
        if file.status != "closed"
            || file
                .retain_until
                .map(|d| d > Utc::now().date_naive())
                .unwrap_or(true)
        {
            bail!("The document is not due for disposition review");
        }
        if file.final_disposition == "permanent" {
            bail!("Permanent documents cannot enter disposition review");
        }
        if let Some(date) = request.proposed_retain_until
            && date <= Utc::now().date_naive()
        {
            bail!("The new retention date must be in the future");
        }
        let id = Uuid::new_v4();
        let mut tx = pool
            .begin()
            .await
            .context("start disposition review request")?;
        sqlx::query(
            r#"INSERT INTO document_registry_disposition_reviews
               (id,tenant_id,file_id,recommendation,proposed_retain_until,request_reason,requested_by)
               VALUES ($1,$2,$3,$4,$5,$6,$7)"#,
        ).bind(id).bind(tenant_id).bind(file_id).bind(&request.recommendation).bind(request.proposed_retain_until)
          .bind(required("Disposition reason", &request.reason)?).bind(actor_id).execute(&mut *tx).await.context("create disposition review")?;
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            context,
            "disposition_review",
            id,
            Some(file_id),
            "disposition_review_requested",
            "document_registry.disposition.request",
            json!({"recommendation": request.recommendation,"reason": request.reason.trim()}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit disposition review request")?;
        Self::get_review(pool, tenant_id, id, true)
            .await?
            .ok_or_else(|| anyhow!("The disposition review could not be reloaded"))
    }

    pub async fn decide_review(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &ReviewDecisionRequest,
        approve: bool,
    ) -> Result<Option<ReviewResponse>> {
        let actor_id = person_actor_id(actor)?;
        let review = Self::get_review(pool, tenant_id, id, true).await?;
        let Some(review) = review else {
            return Ok(None);
        };
        if review.version != request.version || review.status != "pending" {
            return Ok(None);
        }
        if approve && review.requested_by == actor_id {
            bail!("The requester cannot approve their own disposition review");
        }
        let mut tx = pool.begin().await.context("start disposition decision")?;
        if approve && review.recommendation == "retain" {
            sqlx::query("UPDATE document_registry_files SET retain_until=$4,version=version+1,updated_by=$3 WHERE tenant_id=$1 AND id=$2 AND status='closed'")
                .bind(tenant_id).bind(review.file_id).bind(actor_id).bind(review.proposed_retain_until)
                .execute(&mut *tx).await.context("apply the retention extension")?;
            sqlx::query("UPDATE document_registry_disposition_reviews SET status='executed',reviewed_by=$4,reviewed_at=NOW(),review_reason=$5,executed_by=$4,executed_at=NOW(),version=version+1 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='pending'")
                .bind(tenant_id).bind(id).bind(request.version).bind(actor_id).bind(required("Decision reason", &request.reason)?)
                .execute(&mut *tx).await.context("approve the retention extension")?;
        } else {
            let status = if approve { "approved" } else { "rejected" };
            sqlx::query("UPDATE document_registry_disposition_reviews SET status=$4,reviewed_by=$5,reviewed_at=NOW(),review_reason=$6,version=version+1 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='pending'")
                .bind(tenant_id).bind(id).bind(request.version).bind(status).bind(actor_id).bind(required("Decision reason", &request.reason)?)
                .execute(&mut *tx).await.context("record the disposition decision")?;
        }
        let event = if approve {
            "disposition_review_approved"
        } else {
            "disposition_review_rejected"
        };
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            context,
            "disposition_review",
            id,
            Some(review.file_id),
            event,
            if approve {
                "document_registry.disposition.approve"
            } else {
                "document_registry.disposition.reject"
            },
            json!({"reason": request.reason.trim()}),
        )
        .await?;
        tx.commit().await.context("commit disposition decision")?;
        Self::get_review(pool, tenant_id, id, true).await
    }

    pub async fn execute_destruction(
        pool: &PgPool,
        storage: &DocumentStorage,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &ExecuteDestructionRequest,
    ) -> Result<Option<ReviewResponse>> {
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("start approved document destruction")?;
        let file_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT file_id FROM document_registry_disposition_reviews WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='approved' AND recommendation='destroy' FOR UPDATE",
        ).bind(tenant_id).bind(id).bind(request.version).fetch_optional(&mut *tx).await.context("lock approved disposition review")?;
        let Some(file_id) = file_id else {
            return Ok(None);
        };
        let object_key = sqlx::query_scalar::<_, String>(
            "SELECT object_key FROM document_registry_files WHERE tenant_id=$1 AND id=$2 AND status='closed' AND object_key IS NOT NULL FOR UPDATE",
        ).bind(tenant_id).bind(file_id).fetch_optional(&mut *tx).await.context("lock document object for destruction")?
          .ok_or_else(|| anyhow!("The document is not available for destruction"))?;
        storage.delete(&object_key).await?;
        sqlx::query(
            "UPDATE document_registry_files SET status='destroyed',object_key=NULL,destroyed_by=$3,destroyed_at=NOW(),destruction_reason=$4,version=version+1,updated_by=$3 WHERE tenant_id=$1 AND id=$2 AND status='closed'",
        ).bind(tenant_id).bind(file_id).bind(actor_id).bind(required("Destruction reason", &request.reason)?)
          .execute(&mut *tx).await.context("retain the destruction evidence")?;
        sqlx::query(
            "UPDATE document_registry_disposition_reviews SET status='executed',executed_by=$4,executed_at=NOW(),version=version+1 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='approved'",
        ).bind(tenant_id).bind(id).bind(request.version).bind(actor_id).execute(&mut *tx).await.context("execute disposition review")?;
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            context,
            "disposition_review",
            id,
            Some(file_id),
            "document_destroyed",
            "document_registry.disposition.execute",
            json!({"reason": request.reason.trim()}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit approved document destruction")?;
        Self::get_review(pool, tenant_id, id, true).await
    }
}

fn numbering_response(row: NumberingPolicyRow) -> NumberingPolicyResponse {
    NumberingPolicyResponse {
        next_reference: format!(
            "{}{:0width$}",
            row.prefix,
            row.next_sequence,
            width = row.padding as usize
        ),
        prefix: row.prefix,
        padding: row.padding,
        next_sequence: row.next_sequence,
        version: row.version,
        updated_at: row.updated_at,
    }
}
fn series_response(row: SeriesRow) -> SeriesResponse {
    SeriesResponse {
        id: row.id,
        code: row.code,
        name: row.name,
        description: row.description,
        retention_trigger: row.retention_trigger,
        retention_period_months: row.retention_period_months,
        final_disposition: row.final_disposition,
        default_sensitivity: row.default_sensitivity,
        status: row.status,
        version: row.version,
        file_count: row.file_count,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}
fn file_response(row: FileRow) -> FileResponse {
    FileResponse {
        id: row.id,
        reference: row.reference,
        series_id: row.series_id,
        series_code: row.series_code_snapshot,
        series_name: row.series_name_snapshot,
        retention_trigger: row.retention_trigger_snapshot,
        retention_period_months: row.retention_period_months_snapshot,
        final_disposition: row.final_disposition_snapshot,
        sensitivity: row.sensitivity,
        title: row.title,
        description: row.description,
        document_date: row.document_date,
        filed_on: row.filed_on,
        retain_until: row.retain_until,
        status: row.status,
        original_file_name: row.original_file_name,
        media_type: row.media_type,
        byte_size: row.byte_size,
        sha256_hex: row.sha256_hex,
        scanned_at: row.scanned_at,
        version: row.version,
        closed_at: row.closed_at,
        close_reason: row.close_reason,
        destroyed_at: row.destroyed_at,
        destruction_reason: row.destruction_reason,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}
fn review_response(row: ReviewRow) -> ReviewResponse {
    ReviewResponse {
        id: row.id,
        file_id: row.file_id,
        file_reference: row.file_reference,
        file_title: row.file_title,
        recommendation: row.recommendation,
        proposed_retain_until: row.proposed_retain_until,
        request_reason: row.request_reason,
        status: row.status,
        version: row.version,
        requested_by: row.requested_by,
        reviewed_by: row.reviewed_by,
        reviewed_at: row.reviewed_at,
        review_reason: row.review_reason,
        executed_by: row.executed_by,
        executed_at: row.executed_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}
fn activity_response(row: ActivityRow) -> ActivityResponse {
    ActivityResponse {
        id: row.id,
        aggregate_type: row.aggregate_type,
        aggregate_id: row.aggregate_id,
        file_id: row.file_id,
        event_type: row.event_type,
        actor_id: row.actor_id,
        metadata: row.metadata,
        created_at: row.created_at,
    }
}

fn validate_series(
    trigger: &str,
    months: Option<i16>,
    disposition: &str,
    sensitivity: &str,
    status: Option<&str>,
) -> Result<()> {
    validate_choice(
        trigger,
        &["filed", "closed"],
        "Choose filed or closed as the retention trigger",
    )?;
    validate_choice(
        disposition,
        &["review", "destroy", "permanent"],
        "Choose review, destroy, or permanent as the final disposition",
    )?;
    validate_sensitivity(Some(sensitivity))?;
    if disposition == "permanent" && months.is_some() {
        bail!("Permanent classifications cannot have a retention period");
    }
    if disposition != "permanent" && !matches!(months, Some(1..=1200)) {
        bail!("Choose a retention period from 1 to 1200 months");
    }
    if let Some(status) = status {
        validate_choice(
            status,
            &["active", "inactive"],
            "Choose active or inactive classification status",
        )?;
    }
    Ok(())
}
fn validate_sensitivity(value: Option<&str>) -> Result<()> {
    if let Some(value) = value {
        validate_choice(value, SENSITIVITIES, "Choose a valid document sensitivity")?;
    }
    Ok(())
}
fn validate_choice(value: &str, allowed: &[&str], message: &str) -> Result<()> {
    if !allowed.contains(&value) {
        bail!(message.to_string())
    }
    Ok(())
}
fn validate_optional(value: Option<&str>, allowed: &[&str], label: &str) -> Result<()> {
    if let Some(value) = value
        && !allowed.contains(&value)
    {
        bail!("Choose a valid {label}")
    }
    Ok(())
}
fn retention_date(base: NaiveDate, months: Option<i16>) -> Result<Option<NaiveDate>> {
    months
        .map(|m| {
            base.checked_add_months(Months::new(m as u32))
                .ok_or_else(|| anyhow!("The retention date is outside the supported range"))
        })
        .transpose()
}
fn required<'a>(label: &str, value: &'a str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{label} is required")
    }
    Ok(value)
}
fn clean(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|v| !v.is_empty())
}
fn safe_file_name(value: &str) -> Result<String> {
    let name = value.rsplit(['/', '\\']).next().unwrap_or_default().trim();
    if name.is_empty() || name.len() > 255 {
        bail!("Choose a file with a valid name")
    }
    Ok(name.to_string())
}
fn like_query(value: Option<&str>) -> Option<String> {
    clean(value).map(|value| format!("%{value}%"))
}
fn bounded_page(query: &RegistryListQuery) -> (i64, i64) {
    (
        query.page.unwrap_or(1).max(1),
        query.per_page.unwrap_or(25).clamp(1, 100),
    )
}
fn person_actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("A person account is required for this Document Registry action"))
}

#[allow(clippy::too_many_arguments)]
async fn append_evidence(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    context: RequestContext,
    aggregate_type: &str,
    aggregate_id: Uuid,
    file_id: Option<Uuid>,
    event_type: &str,
    action: &str,
    metadata: Value,
) -> Result<()> {
    let actor_id = person_actor_id(actor)?;
    sqlx::query("INSERT INTO document_registry_activity_events (tenant_id,aggregate_type,aggregate_id,file_id,event_type,actor_id,metadata) VALUES ($1,$2,$3,$4,$5,$6,$7)")
        .bind(tenant_id).bind(aggregate_type).bind(aggregate_id).bind(file_id).bind(event_type).bind(actor_id).bind(metadata.clone())
        .execute(&mut **tx).await.context("append Document Registry activity evidence")?;
    append_audit(
        &mut **tx,
        &NewAuditEvent::new(tenant_id, actor, action, AuditOutcome::Succeeded, context)
            .with_target(AuditTarget::new(aggregate_type, aggregate_id.to_string()))
            .with_redacted_metadata(metadata.as_object().cloned().unwrap_or_else(Map::new)),
    )
    .await
    .context("append Document Registry audit evidence")?;
    Ok(())
}

const SENSITIVITIES: &[&str] = &["general", "internal", "confidential", "restricted"];
const SERIES_SELECT: &str = "SELECT series.id,series.code,series.name,series.description,series.retention_trigger,series.retention_period_months,series.final_disposition,series.default_sensitivity,series.status,series.version,(SELECT COUNT(*) FROM document_registry_files file WHERE file.tenant_id=series.tenant_id AND file.series_id=series.id AND file.deleted_at IS NULL) AS file_count,series.created_at,series.updated_at FROM document_registry_series series WHERE series.tenant_id=$1 AND series.deleted_at IS NULL AND ($2::TEXT IS NULL OR series.status=$2) AND ($3::TEXT IS NULL OR series.code ILIKE $3 OR series.name ILIKE $3) ORDER BY series.name LIMIT $4 OFFSET $5";
const SERIES_COUNT: &str = "SELECT COUNT(*) FROM document_registry_series series WHERE series.tenant_id=$1 AND series.deleted_at IS NULL AND ($2::TEXT IS NULL OR series.status=$2) AND ($3::TEXT IS NULL OR series.code ILIKE $3 OR series.name ILIKE $3)";
const SERIES_BY_ID: &str = "SELECT series.id,series.code,series.name,series.description,series.retention_trigger,series.retention_period_months,series.final_disposition,series.default_sensitivity,series.status,series.version,(SELECT COUNT(*) FROM document_registry_files file WHERE file.tenant_id=series.tenant_id AND file.series_id=series.id AND file.deleted_at IS NULL) AS file_count,series.created_at,series.updated_at FROM document_registry_series series WHERE series.tenant_id=$1 AND series.id=$2 AND series.deleted_at IS NULL";
const FILE_SELECT: &str = "SELECT file.id,file.reference,file.series_id,file.series_code_snapshot,file.series_name_snapshot,file.retention_trigger_snapshot,file.retention_period_months_snapshot,file.final_disposition_snapshot,file.sensitivity,file.title,file.description,file.document_date,file.filed_on,file.retain_until,file.status,file.original_file_name,file.media_type,file.byte_size,file.sha256_hex,file.object_key,file.scanned_at,file.version,file.closed_at,file.close_reason,file.destroyed_at,file.destruction_reason,file.created_at,file.updated_at FROM document_registry_files file WHERE file.tenant_id=$1 AND file.deleted_at IS NULL AND ($2::TEXT IS NULL OR file.status=$2) AND ($3::UUID IS NULL OR file.series_id=$3) AND ($4::TEXT IS NULL OR file.sensitivity=$4) AND ($5::TEXT IS NULL OR file.reference ILIKE $5 OR file.title ILIKE $5 OR file.series_name_snapshot ILIKE $5) AND ($6 OR file.sensitivity <> 'restricted') ORDER BY file.filed_on DESC,file.reference DESC LIMIT $7 OFFSET $8";
const FILE_COUNT: &str = "SELECT COUNT(*) FROM document_registry_files file WHERE file.tenant_id=$1 AND file.deleted_at IS NULL AND ($2::TEXT IS NULL OR file.status=$2) AND ($3::UUID IS NULL OR file.series_id=$3) AND ($4::TEXT IS NULL OR file.sensitivity=$4) AND ($5::TEXT IS NULL OR file.reference ILIKE $5 OR file.title ILIKE $5 OR file.series_name_snapshot ILIKE $5) AND ($6 OR file.sensitivity <> 'restricted')";
const FILE_BY_ID: &str = "SELECT file.id,file.reference,file.series_id,file.series_code_snapshot,file.series_name_snapshot,file.retention_trigger_snapshot,file.retention_period_months_snapshot,file.final_disposition_snapshot,file.sensitivity,file.title,file.description,file.document_date,file.filed_on,file.retain_until,file.status,file.original_file_name,file.media_type,file.byte_size,file.sha256_hex,file.object_key,file.scanned_at,file.version,file.closed_at,file.close_reason,file.destroyed_at,file.destruction_reason,file.created_at,file.updated_at FROM document_registry_files file WHERE file.tenant_id=$1 AND file.id=$2 AND file.deleted_at IS NULL AND ($3 OR file.sensitivity <> 'restricted')";
const FILE_BASE: &str = "SELECT file.id,file.reference,file.series_id,file.series_code_snapshot,file.series_name_snapshot,file.retention_trigger_snapshot,file.retention_period_months_snapshot,file.final_disposition_snapshot,file.sensitivity,file.title,file.description,file.document_date,file.filed_on,file.retain_until,file.status,file.original_file_name,file.media_type,file.byte_size,file.sha256_hex,file.object_key,file.scanned_at,file.version,file.closed_at,file.close_reason,file.destroyed_at,file.destruction_reason,file.created_at,file.updated_at FROM document_registry_files file WHERE file.tenant_id=$1 AND file.deleted_at IS NULL AND ($2 OR file.sensitivity <> 'restricted')";
const REVIEW_SELECT: &str = "SELECT review.id,review.file_id,file.reference AS file_reference,file.title AS file_title,review.recommendation,review.proposed_retain_until,review.request_reason,review.status,review.version,review.requested_by,review.reviewed_by,review.reviewed_at,review.review_reason,review.executed_by,review.executed_at,review.created_at,review.updated_at FROM document_registry_disposition_reviews review JOIN document_registry_files file ON file.id=review.file_id AND file.tenant_id=review.tenant_id WHERE review.tenant_id=$1 AND review.deleted_at IS NULL AND ($2::TEXT IS NULL OR review.status=$2) AND ($3 OR file.sensitivity <> 'restricted') ORDER BY review.created_at DESC LIMIT $4 OFFSET $5";
const REVIEW_COUNT: &str = "SELECT COUNT(*) FROM document_registry_disposition_reviews review JOIN document_registry_files file ON file.id=review.file_id AND file.tenant_id=review.tenant_id WHERE review.tenant_id=$1 AND review.deleted_at IS NULL AND ($2::TEXT IS NULL OR review.status=$2) AND ($3 OR file.sensitivity <> 'restricted')";
const REVIEW_BY_ID: &str = "SELECT review.id,review.file_id,file.reference AS file_reference,file.title AS file_title,review.recommendation,review.proposed_retain_until,review.request_reason,review.status,review.version,review.requested_by,review.reviewed_by,review.reviewed_at,review.review_reason,review.executed_by,review.executed_at,review.created_at,review.updated_at FROM document_registry_disposition_reviews review JOIN document_registry_files file ON file.id=review.file_id AND file.tenant_id=review.tenant_id WHERE review.tenant_id=$1 AND review.id=$2 AND review.deleted_at IS NULL AND ($3 OR file.sensitivity <> 'restricted')";

#[cfg(test)]
mod tests {
    use super::{retention_date, validate_series};
    use chrono::NaiveDate;
    #[test]
    fn retention_uses_calendar_months() {
        assert_eq!(
            retention_date(NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(), Some(1)).unwrap(),
            Some(NaiveDate::from_ymd_opt(2026, 2, 28).unwrap())
        );
    }
    #[test]
    fn permanent_series_has_no_duration() {
        assert!(validate_series("filed", None, "permanent", "internal", None).is_ok());
        assert!(validate_series("filed", Some(12), "permanent", "internal", None).is_err());
    }
}
