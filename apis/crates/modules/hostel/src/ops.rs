//! Tenant-scoped Hostel operations with previewed allocations and SIS hydration.

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, anyhow, bail};
use chrono::{NaiveDate, Utc};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_sis::ops::{EnrolmentOps, LearnerOps};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    ActivateAllocationRequest, AllocationPreviewRequest, AllocationPreviewResponse,
    AllocationResponse, CancelAllocationRequest, CreateAllocationRequest,
    CreatePastoralRecordRequest, CreateResidenceRequest, CreateRoomRequest, EndAllocationRequest,
    HostelAccessScope, HostelLearnerCandidate, HostelListQuery, HostelReferenceData,
    PastoralRecordResponse, ResidenceResponse, RoomResponse, TransferAllocationPreviewRequest,
    TransferAllocationRequest, UpdatePastoralRecordRequest, UpdateResidenceRequest,
    UpdateRoomRequest,
    dtos::ResolvePastoralRecordRequest,
    models::{AllocationRow, PastoralRecordRow, PreviewRoomRow, ResidenceRow, RoomRow},
};

/// Boarding workflows over stable learner and room identifiers.
pub struct HostelOps;

impl HostelOps {
    pub async fn reference_data(
        pool: &PgPool,
        tenant_id: Uuid,
        search: Option<&str>,
    ) -> Result<HostelReferenceData> {
        let learners = LearnerOps::hostel_references(pool, tenant_id, search, 100).await?;
        let current = sqlx::query_scalar::<_, Uuid>(
            "SELECT learner_id FROM hostel_allocations WHERE tenant_id=$1 AND status IN ('planned','active')",
        )
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .context("Failed to resolve current Hostel allocations")?
        .into_iter()
        .collect::<HashSet<_>>();
        let rooms = Self::list_rooms(
            pool,
            tenant_id,
            &HostelListQuery {
                page: Some(1),
                per_page: Some(100),
                search: None,
                status: Some("available".to_string()),
                residence_id: None,
                room_id: None,
                learner_id: None,
                category: None,
            },
        )
        .await?
        .0;
        Ok(HostelReferenceData {
            learners: learners
                .into_iter()
                .map(|learner| HostelLearnerCandidate {
                    id: learner.id,
                    learner_number: learner.learner_number,
                    display_name: learner.display_name,
                    status: learner.status,
                    has_current_allocation: current.contains(&learner.id),
                })
                .collect(),
            rooms: rooms
                .into_iter()
                .filter(|room| room.available_beds > 0)
                .collect(),
        })
    }

    pub async fn list_residences(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &HostelListQuery,
    ) -> Result<(Vec<ResidenceResponse>, i64)> {
        validate_status(
            query.status.as_deref(),
            &["active", "inactive"],
            "residence",
        )?;
        let (page, per_page) = bounded_page(query);
        let search = like(query.search.as_deref());
        let rows = sqlx::query_as::<_, ResidenceRow>(RESIDENCE_SELECT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(search.as_deref())
            .bind(per_page)
            .bind((page - 1) * per_page)
            .fetch_all(pool)
            .await
            .context("Failed to list Hostel residences")?;
        let total = sqlx::query_scalar::<_, i64>(RESIDENCE_COUNT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(search.as_deref())
            .fetch_one(pool)
            .await
            .context("Failed to count Hostel residences")?;
        Ok((rows.into_iter().map(residence_response).collect(), total))
    }

    pub async fn get_residence(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<ResidenceResponse>> {
        sqlx::query_as::<_, ResidenceRow>(RESIDENCE_BY_ID)
            .bind(tenant_id)
            .bind(id)
            .fetch_optional(pool)
            .await
            .context("Failed to load Hostel residence")
            .map(|row| row.map(residence_response))
    }

    pub async fn create_residence(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateResidenceRequest,
    ) -> Result<ResidenceResponse> {
        let actor_id = person_actor_id(actor)?;
        let code = required("Residence code", &request.code)?;
        let name = required("Residence name", &request.name)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start residence creation")?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"INSERT INTO hostel_residences (
                   tenant_id, code, name, description, created_by, updated_by
               ) VALUES ($1,$2,$3,$4,$5,$5) RETURNING id"#,
        )
        .bind(tenant_id)
        .bind(code)
        .bind(name)
        .bind(optional_text(request.description.as_deref()))
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_unique(
            "A residence with this code or name already exists",
        ))?;
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "residence",
            id,
            None,
            "created",
            "hostel.residences.create",
            json!({ "code": code }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit residence creation")?;
        Self::get_residence(pool, tenant_id, id)
            .await?
            .ok_or_else(|| anyhow!("The residence could not be reloaded"))
    }

    pub async fn update_residence(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateResidenceRequest,
    ) -> Result<Option<ResidenceResponse>> {
        let actor_id = person_actor_id(actor)?;
        let code = required("Residence code", &request.code)?;
        let name = required("Residence name", &request.name)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start residence update")?;
        if request.status.as_str() == "inactive" {
            let current = sqlx::query_scalar::<_, i64>(
                r#"SELECT COUNT(*) FROM hostel_allocations allocation
                   JOIN hostel_rooms room ON room.id=allocation.room_id AND room.tenant_id=allocation.tenant_id
                  WHERE allocation.tenant_id=$1 AND room.residence_id=$2
                    AND allocation.status IN ('planned','active')"#,
            )
            .bind(tenant_id)
            .bind(id)
            .fetch_one(&mut *transaction)
            .await?;
            if current > 0 {
                bail!("A residence with current allocations cannot be made inactive");
            }
        }
        let changed = sqlx::query_scalar::<_, Uuid>(
            r#"UPDATE hostel_residences
                  SET code=$3, name=$4, description=$5, status=$6,
                      version=version+1, updated_by=$7, updated_at=NOW()
                WHERE tenant_id=$1 AND id=$2 AND version=$8 RETURNING id"#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(code)
        .bind(name)
        .bind(optional_text(request.description.as_deref()))
        .bind(request.status.as_str())
        .bind(actor_id)
        .bind(request.expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(map_unique(
            "A residence with this code or name already exists",
        ))?;
        if changed.is_none() {
            return versioned_not_found(&mut transaction, tenant_id, "hostel_residences", id).await;
        }
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "residence",
            id,
            None,
            "updated",
            "hostel.residences.update",
            json!({ "status": request.status.as_str() }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit residence update")?;
        Self::get_residence(pool, tenant_id, id).await
    }

    pub async fn list_rooms(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &HostelListQuery,
    ) -> Result<(Vec<RoomResponse>, i64)> {
        validate_status(
            query.status.as_deref(),
            &["available", "maintenance", "inactive"],
            "room",
        )?;
        let (page, per_page) = bounded_page(query);
        let search = like(query.search.as_deref());
        let rows = sqlx::query_as::<_, RoomRow>(ROOM_SELECT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(query.residence_id)
            .bind(search.as_deref())
            .bind(per_page)
            .bind((page - 1) * per_page)
            .fetch_all(pool)
            .await
            .context("Failed to list Hostel rooms")?;
        let total = sqlx::query_scalar::<_, i64>(ROOM_COUNT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(query.residence_id)
            .bind(search.as_deref())
            .fetch_one(pool)
            .await
            .context("Failed to count Hostel rooms")?;
        Ok((rows.into_iter().map(room_response).collect(), total))
    }

    pub async fn get_room(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<RoomResponse>> {
        sqlx::query_as::<_, RoomRow>(ROOM_BY_ID)
            .bind(tenant_id)
            .bind(id)
            .fetch_optional(pool)
            .await
            .context("Failed to load Hostel room")
            .map(|row| row.map(room_response))
    }

    pub async fn create_room(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateRoomRequest,
    ) -> Result<RoomResponse> {
        let actor_id = person_actor_id(actor)?;
        ensure_active_residence(pool, tenant_id, request.residence_id).await?;
        let code = required("Room code", &request.code)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start room creation")?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"INSERT INTO hostel_rooms (
                   tenant_id, residence_id, code, floor_label, capacity, created_by, updated_by
               ) VALUES ($1,$2,$3,$4,$5,$6,$6) RETURNING id"#,
        )
        .bind(tenant_id)
        .bind(request.residence_id)
        .bind(code)
        .bind(optional_text(request.floor_label.as_deref()))
        .bind(request.capacity)
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_unique(
            "This residence already has a room with that code",
        ))?;
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "room",
            id,
            None,
            "created",
            "hostel.rooms.create",
            json!({ "residence_id": request.residence_id, "capacity": request.capacity }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit room creation")?;
        Self::get_room(pool, tenant_id, id)
            .await?
            .ok_or_else(|| anyhow!("The room could not be reloaded"))
    }

    pub async fn update_room(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateRoomRequest,
    ) -> Result<Option<RoomResponse>> {
        let actor_id = person_actor_id(actor)?;
        let code = required("Room code", &request.code)?;
        let mut transaction = pool.begin().await.context("Failed to start room update")?;
        let occupied = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM hostel_allocations WHERE tenant_id=$1 AND room_id=$2 AND status IN ('planned','active')",
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_one(&mut *transaction)
        .await?;
        if i64::from(request.capacity) < occupied {
            bail!("Room capacity cannot be lower than current occupancy");
        }
        if request.status.as_str() != "available" && occupied > 0 {
            bail!("A room with current allocations cannot be moved out of service");
        }
        let changed = sqlx::query_scalar::<_, Uuid>(
            r#"UPDATE hostel_rooms
                  SET code=$3, floor_label=$4, capacity=$5, status=$6,
                      version=version+1, updated_by=$7, updated_at=NOW()
                WHERE tenant_id=$1 AND id=$2 AND version=$8 RETURNING id"#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(code)
        .bind(optional_text(request.floor_label.as_deref()))
        .bind(request.capacity)
        .bind(request.status.as_str())
        .bind(actor_id)
        .bind(request.expected_version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(map_unique(
            "This residence already has a room with that code",
        ))?;
        if changed.is_none() {
            return versioned_not_found(&mut transaction, tenant_id, "hostel_rooms", id).await;
        }
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "room",
            id,
            None,
            "updated",
            "hostel.rooms.update",
            json!({ "status": request.status.as_str(), "capacity": request.capacity }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit room update")?;
        Self::get_room(pool, tenant_id, id).await
    }

    pub async fn allocation_preview(
        pool: &PgPool,
        tenant_id: Uuid,
        request: &AllocationPreviewRequest,
    ) -> Result<AllocationPreviewResponse> {
        validate_allocation_dates(request.starts_on, request.expected_end_on)?;
        let learner = LearnerOps::hostel_references_by_ids(pool, tenant_id, &[request.learner_id])
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("The selected learner is unavailable"))?;
        let room = preview_room(
            pool,
            tenant_id,
            request.room_id,
            request.starts_on,
            request.expected_end_on,
            request.replacing_allocation_id,
        )
        .await?;
        let existing = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM hostel_allocations
                WHERE tenant_id=$1 AND learner_id=$2 AND status IN ('planned','active')
                  AND ($3::UUID IS NULL OR id <> $3)"#,
        )
        .bind(tenant_id)
        .bind(request.learner_id)
        .bind(request.replacing_allocation_id)
        .fetch_one(pool)
        .await
        .context("Failed to check the learner allocation")?;
        let mut issues = Vec::new();
        if learner.status != "active" {
            issues.push("The learner is not active in SIS.".to_string());
        }
        if request.starts_on < Utc::now().date_naive() {
            issues.push("The allocation start date cannot be in the past.".to_string());
        }
        if room.status != "available" {
            issues.push("The selected room is not available.".to_string());
        }
        if room.occupied_count >= i64::from(room.capacity) {
            issues.push("The selected room has no available bed for these dates.".to_string());
        }
        if existing > 0 {
            issues.push("The learner already has a current allocation.".to_string());
        }
        let fingerprint = allocation_fingerprint(
            tenant_id,
            request.learner_id,
            &room,
            request.starts_on,
            request.expected_end_on,
            &issues,
        );
        Ok(AllocationPreviewResponse {
            learner_id: learner.id,
            learner_number: learner.learner_number,
            learner_name: learner.display_name,
            room_id: room.id,
            room_code: room.code,
            residence_name: room.residence_name,
            room_version: room.version,
            capacity: room.capacity,
            occupied_count: room.occupied_count,
            available_beds: (i64::from(room.capacity) - room.occupied_count).max(0),
            starts_on: request.starts_on,
            expected_end_on: request.expected_end_on,
            can_allocate: issues.is_empty(),
            issues,
            fingerprint,
        })
    }

    pub async fn list_allocations(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: HostelAccessScope,
        query: &HostelListQuery,
    ) -> Result<(Vec<AllocationResponse>, i64)> {
        validate_status(
            query.status.as_deref(),
            &["planned", "active", "ended", "cancelled"],
            "allocation",
        )?;
        let (page, per_page) = bounded_page(query);
        let visible_ids = visible_learner_filter(pool, tenant_id, scope).await?;
        let search_ids = search_learner_ids(pool, tenant_id, query.search.as_deref()).await?;
        let rows = sqlx::query_as::<_, AllocationRow>(ALLOCATION_SELECT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(query.residence_id)
            .bind(query.room_id)
            .bind(query.learner_id)
            .bind(visible_ids.as_deref())
            .bind(search_ids.as_deref())
            .bind(per_page)
            .bind((page - 1) * per_page)
            .fetch_all(pool)
            .await
            .context("Failed to list Hostel allocations")?;
        let total = sqlx::query_scalar::<_, i64>(ALLOCATION_COUNT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(query.residence_id)
            .bind(query.room_id)
            .bind(query.learner_id)
            .bind(visible_ids.as_deref())
            .bind(search_ids.as_deref())
            .fetch_one(pool)
            .await
            .context("Failed to count Hostel allocations")?;
        Ok((hydrate_allocations(pool, tenant_id, rows).await?, total))
    }

    pub async fn get_allocation(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        scope: HostelAccessScope,
    ) -> Result<Option<AllocationResponse>> {
        let visible_ids = visible_learner_filter(pool, tenant_id, scope).await?;
        let row = sqlx::query_as::<_, AllocationRow>(ALLOCATION_BY_ID)
            .bind(tenant_id)
            .bind(id)
            .bind(visible_ids.as_deref())
            .fetch_optional(pool)
            .await
            .context("Failed to load Hostel allocation")?;
        let Some(row) = row else { return Ok(None) };
        Ok(hydrate_allocations(pool, tenant_id, vec![row]).await?.pop())
    }

    pub async fn create_allocation(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateAllocationRequest,
    ) -> Result<AllocationResponse> {
        let preview_request = AllocationPreviewRequest {
            learner_id: request.learner_id,
            room_id: request.room_id,
            starts_on: request.starts_on,
            expected_end_on: request.expected_end_on,
            replacing_allocation_id: None,
        };
        let preview = Self::allocation_preview(pool, tenant_id, &preview_request).await?;
        require_preview(&preview, &request.preview_fingerprint)?;
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start allocation creation")?;
        lock_room_and_revalidate(
            &mut transaction,
            tenant_id,
            request.room_id,
            request.starts_on,
            request.expected_end_on,
            None,
        )
        .await?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"INSERT INTO hostel_allocations (
                   tenant_id, learner_id, room_id, starts_on, expected_end_on,
                   created_by, updated_by
               ) VALUES ($1,$2,$3,$4,$5,$6,$6) RETURNING id"#,
        )
        .bind(tenant_id)
        .bind(request.learner_id)
        .bind(request.room_id)
        .bind(request.starts_on)
        .bind(request.expected_end_on)
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(map_unique("The learner already has a current allocation"))?;
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "allocation",
            id,
            Some(request.learner_id),
            "planned",
            "hostel.allocations.create",
            json!({ "room_id": request.room_id, "starts_on": request.starts_on }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit allocation creation")?;
        Self::get_allocation(pool, tenant_id, id, HostelAccessScope::Campus)
            .await?
            .ok_or_else(|| anyhow!("The allocation could not be reloaded"))
    }

    pub async fn activate_allocation(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ActivateAllocationRequest,
    ) -> Result<Option<AllocationResponse>> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start allocation check-in")?;
        let allocation =
            sqlx::query_as::<_, (Uuid, Uuid, NaiveDate, Option<NaiveDate>, String, i32)>(
                r#"SELECT learner_id, room_id, starts_on, expected_end_on, status, version
                 FROM hostel_allocations WHERE tenant_id=$1 AND id=$2 FOR UPDATE"#,
            )
            .bind(tenant_id)
            .bind(id)
            .fetch_optional(&mut *transaction)
            .await?;
        let Some((learner_id, room_id, starts_on, expected_end_on, status, version)) = allocation
        else {
            return Ok(None);
        };
        if version != request.expected_version {
            bail!("The allocation changed; reload it before saving");
        }
        if status != "planned" {
            bail!("Only a planned allocation can be checked in");
        }
        if starts_on > Utc::now().date_naive() {
            bail!("The allocation cannot be checked in before its start date");
        }
        lock_room_and_revalidate(
            &mut transaction,
            tenant_id,
            room_id,
            starts_on,
            expected_end_on,
            Some(id),
        )
        .await?;
        sqlx::query(
            "UPDATE hostel_allocations SET status='active', version=version+1, updated_by=$3, updated_at=NOW() WHERE tenant_id=$1 AND id=$2",
        )
        .bind(tenant_id)
        .bind(id)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await?;
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "allocation",
            id,
            Some(learner_id),
            "checked_in",
            "hostel.allocations.activate",
            json!({}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit allocation check-in")?;
        Self::get_allocation(pool, tenant_id, id, HostelAccessScope::Campus).await
    }

    pub async fn end_allocation(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &EndAllocationRequest,
    ) -> Result<Option<AllocationResponse>> {
        let actor_id = person_actor_id(actor)?;
        let reason = required("End reason", &request.reason)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start allocation checkout")?;
        let row = sqlx::query_as::<_, (Uuid, NaiveDate, String, i32)>(
            "SELECT learner_id, starts_on, status, version FROM hostel_allocations WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((learner_id, starts_on, status, version)) = row else {
            return Ok(None);
        };
        if version != request.expected_version {
            bail!("The allocation changed; reload it before saving");
        }
        if status != "active" {
            bail!("Only an active allocation can be checked out");
        }
        if request.ended_on < starts_on || request.ended_on > Utc::now().date_naive() {
            bail!("The checkout date must be between the start date and today");
        }
        sqlx::query(
            r#"UPDATE hostel_allocations
                  SET status='ended', ended_on=$3, decision_reason=$4,
                      ended_by=$5, ended_at=NOW(), updated_by=$5,
                      version=version+1, updated_at=NOW()
                WHERE tenant_id=$1 AND id=$2"#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(request.ended_on)
        .bind(reason)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await?;
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "allocation",
            id,
            Some(learner_id),
            "checked_out",
            "hostel.allocations.end",
            json!({ "ended_on": request.ended_on, "reason": reason }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit allocation checkout")?;
        Self::get_allocation(pool, tenant_id, id, HostelAccessScope::Campus).await
    }

    pub async fn cancel_allocation(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CancelAllocationRequest,
    ) -> Result<Option<AllocationResponse>> {
        let actor_id = person_actor_id(actor)?;
        let reason = required("Cancellation reason", &request.reason)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start allocation cancellation")?;
        let changed = sqlx::query_as::<_, (Uuid, Uuid)>(
            r#"UPDATE hostel_allocations
                  SET status='cancelled', decision_reason=$4, ended_by=$5, ended_at=NOW(),
                      updated_by=$5, version=version+1, updated_at=NOW()
                WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='planned'
                RETURNING id, learner_id"#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(request.expected_version)
        .bind(reason)
        .bind(actor_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((_, learner_id)) = changed else {
            return allocation_transition_not_found(&mut transaction, tenant_id, id, "cancel")
                .await;
        };
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "allocation",
            id,
            Some(learner_id),
            "cancelled",
            "hostel.allocations.cancel",
            json!({ "reason": reason }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit allocation cancellation")?;
        Self::get_allocation(pool, tenant_id, id, HostelAccessScope::Campus).await
    }

    pub async fn transfer_preview(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        request: &TransferAllocationPreviewRequest,
    ) -> Result<AllocationPreviewResponse> {
        let current = sqlx::query_as::<_, (Uuid, String, i32, Option<NaiveDate>)>(
            "SELECT learner_id, status, version, expected_end_on FROM hostel_allocations WHERE tenant_id=$1 AND id=$2",
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load allocation for transfer")?
        .ok_or_else(|| anyhow!("The allocation was not found"))?;
        if current.1 != "active" {
            bail!("Only an active allocation can be transferred");
        }
        if current.2 != request.expected_version {
            bail!("The allocation changed; reload it before saving");
        }
        if request.effective_on != Utc::now().date_naive() {
            bail!("A room transfer must take effect today");
        }
        Self::allocation_preview(
            pool,
            tenant_id,
            &AllocationPreviewRequest {
                learner_id: current.0,
                room_id: request.new_room_id,
                starts_on: request.effective_on,
                expected_end_on: current.3,
                replacing_allocation_id: Some(id),
            },
        )
        .await
    }

    pub async fn transfer_allocation(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &TransferAllocationRequest,
    ) -> Result<Option<AllocationResponse>> {
        let preview = Self::transfer_preview(
            pool,
            tenant_id,
            id,
            &TransferAllocationPreviewRequest {
                expected_version: request.expected_version,
                new_room_id: request.new_room_id,
                effective_on: request.effective_on,
            },
        )
        .await?;
        require_preview(&preview, &request.preview_fingerprint)?;
        let actor_id = person_actor_id(actor)?;
        let reason = required("Transfer reason", &request.reason)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start room transfer")?;
        let current = sqlx::query_as::<_, (Uuid, Uuid, String, i32, Option<NaiveDate>)>(
            "SELECT learner_id, room_id, status, version, expected_end_on FROM hostel_allocations WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((learner_id, old_room_id, status, version, expected_end_on)) = current else {
            return Ok(None);
        };
        if status != "active" || version != request.expected_version {
            bail!("The allocation changed; reload it before saving");
        }
        if old_room_id == request.new_room_id {
            bail!("Choose a different room for the transfer");
        }
        lock_rooms(
            &mut transaction,
            tenant_id,
            old_room_id,
            request.new_room_id,
        )
        .await?;
        revalidate_room_capacity(
            &mut transaction,
            tenant_id,
            request.new_room_id,
            request.effective_on,
            expected_end_on,
            Some(id),
        )
        .await?;
        sqlx::query(
            r#"UPDATE hostel_allocations
                  SET status='ended', ended_on=$3, decision_reason=$4,
                      ended_by=$5, ended_at=NOW(), updated_by=$5,
                      version=version+1, updated_at=NOW()
                WHERE tenant_id=$1 AND id=$2"#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(request.effective_on)
        .bind(reason)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await?;
        let new_id = sqlx::query_scalar::<_, Uuid>(
            r#"INSERT INTO hostel_allocations (
                   tenant_id, learner_id, room_id, starts_on, expected_end_on, status,
                   previous_allocation_id, decision_reason, created_by, updated_by
               ) VALUES ($1,$2,$3,$4,$5,'active',$6,$7,$8,$8) RETURNING id"#,
        )
        .bind(tenant_id)
        .bind(learner_id)
        .bind(request.new_room_id)
        .bind(request.effective_on)
        .bind(expected_end_on)
        .bind(id)
        .bind(reason)
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await?;
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "allocation",
            id,
            Some(learner_id),
            "transferred_out",
            "hostel.allocations.transfer",
            json!({ "new_allocation_id": new_id, "reason": reason }),
        )
        .await?;
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "allocation",
            new_id,
            Some(learner_id),
            "transferred_in",
            "hostel.allocations.transfer",
            json!({ "previous_allocation_id": id, "reason": reason }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit room transfer")?;
        Self::get_allocation(pool, tenant_id, new_id, HostelAccessScope::Campus).await
    }

    pub async fn list_pastoral_records(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &HostelListQuery,
    ) -> Result<(Vec<PastoralRecordResponse>, i64)> {
        validate_status(
            query.status.as_deref(),
            &["open", "resolved"],
            "pastoral record",
        )?;
        validate_status(
            query.category.as_deref(),
            &[
                "wellbeing",
                "behaviour",
                "safeguarding",
                "family_contact",
                "other",
            ],
            "pastoral category",
        )?;
        let (page, per_page) = bounded_page(query);
        let search_ids = search_learner_ids(pool, tenant_id, query.search.as_deref()).await?;
        let rows = sqlx::query_as::<_, PastoralRecordRow>(PASTORAL_SELECT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(query.category.as_deref())
            .bind(query.learner_id)
            .bind(search_ids.as_deref())
            .bind(per_page)
            .bind((page - 1) * per_page)
            .fetch_all(pool)
            .await
            .context("Failed to list pastoral records")?;
        let total = sqlx::query_scalar::<_, i64>(PASTORAL_COUNT)
            .bind(tenant_id)
            .bind(query.status.as_deref())
            .bind(query.category.as_deref())
            .bind(query.learner_id)
            .bind(search_ids.as_deref())
            .fetch_one(pool)
            .await
            .context("Failed to count pastoral records")?;
        Ok((hydrate_pastoral(pool, tenant_id, rows).await?, total))
    }

    pub async fn get_pastoral_record(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<PastoralRecordResponse>> {
        let row = sqlx::query_as::<_, PastoralRecordRow>(PASTORAL_BY_ID)
            .bind(tenant_id)
            .bind(id)
            .fetch_optional(pool)
            .await
            .context("Failed to load pastoral record")?;
        let Some(row) = row else { return Ok(None) };
        Ok(hydrate_pastoral(pool, tenant_id, vec![row]).await?.pop())
    }

    pub async fn create_pastoral_record(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreatePastoralRecordRequest,
    ) -> Result<PastoralRecordResponse> {
        ensure_learner(pool, tenant_id, request.learner_id).await?;
        ensure_allocation_learner(pool, tenant_id, request.allocation_id, request.learner_id)
            .await?;
        let actor_id = person_actor_id(actor)?;
        let subject = required("Subject", &request.subject)?;
        let details = required("Details", &request.details)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start pastoral record creation")?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"INSERT INTO hostel_pastoral_records (
                   tenant_id, learner_id, allocation_id, category, severity, subject,
                   details, occurred_at, recorded_by, updated_by
               ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$9) RETURNING id"#,
        )
        .bind(tenant_id)
        .bind(request.learner_id)
        .bind(request.allocation_id)
        .bind(request.category.as_str())
        .bind(request.severity.as_str())
        .bind(subject)
        .bind(details)
        .bind(request.occurred_at)
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await?;
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "pastoral_record",
            id,
            Some(request.learner_id),
            "created",
            "hostel.pastoral_records.create",
            json!({ "category": request.category.as_str(), "severity": request.severity.as_str() }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit pastoral record creation")?;
        Self::get_pastoral_record(pool, tenant_id, id)
            .await?
            .ok_or_else(|| anyhow!("The pastoral record could not be reloaded"))
    }

    pub async fn update_pastoral_record(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdatePastoralRecordRequest,
    ) -> Result<Option<PastoralRecordResponse>> {
        let actor_id = person_actor_id(actor)?;
        let subject = required("Subject", &request.subject)?;
        let details = required("Details", &request.details)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start pastoral record update")?;
        let changed = sqlx::query_as::<_, (Uuid, Uuid)>(
            r#"UPDATE hostel_pastoral_records
                  SET category=$3, severity=$4, subject=$5, details=$6, occurred_at=$7,
                      updated_by=$8, version=version+1, updated_at=NOW()
                WHERE tenant_id=$1 AND id=$2 AND version=$9 AND status='open'
                RETURNING id, learner_id"#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(request.category.as_str())
        .bind(request.severity.as_str())
        .bind(subject)
        .bind(details)
        .bind(request.occurred_at)
        .bind(actor_id)
        .bind(request.expected_version)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((_, learner_id)) = changed else {
            return pastoral_transition_not_found(&mut transaction, tenant_id, id, "edit").await;
        };
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "pastoral_record",
            id,
            Some(learner_id),
            "updated",
            "hostel.pastoral_records.update",
            json!({ "category": request.category.as_str(), "severity": request.severity.as_str() }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit pastoral record update")?;
        Self::get_pastoral_record(pool, tenant_id, id).await
    }

    pub async fn resolve_pastoral_record(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ResolvePastoralRecordRequest,
    ) -> Result<Option<PastoralRecordResponse>> {
        let actor_id = person_actor_id(actor)?;
        let resolution = required("Resolution", &request.resolution)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start pastoral resolution")?;
        let changed = sqlx::query_as::<_, (Uuid, Uuid)>(
            r#"UPDATE hostel_pastoral_records
                  SET status='resolved', resolution=$3, resolved_by=$4, resolved_at=NOW(),
                      updated_by=$4, version=version+1, updated_at=NOW()
                WHERE tenant_id=$1 AND id=$2 AND version=$5 AND status='open'
                RETURNING id, learner_id"#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(resolution)
        .bind(actor_id)
        .bind(request.expected_version)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((_, learner_id)) = changed else {
            return pastoral_transition_not_found(&mut transaction, tenant_id, id, "resolve").await;
        };
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "pastoral_record",
            id,
            Some(learner_id),
            "resolved",
            "hostel.pastoral_records.resolve",
            json!({ "resolution_recorded": true }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit pastoral resolution")?;
        Self::get_pastoral_record(pool, tenant_id, id).await
    }
}

async fn ensure_active_residence(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<()> {
    let active = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM hostel_residences WHERE tenant_id=$1 AND id=$2 AND status='active')",
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_one(pool)
    .await?;
    if !active {
        bail!("The selected residence is not active")
    }
    Ok(())
}

async fn ensure_learner(pool: &PgPool, tenant_id: Uuid, learner_id: Uuid) -> Result<()> {
    if LearnerOps::hostel_references_by_ids(pool, tenant_id, &[learner_id])
        .await?
        .into_iter()
        .any(|value| value.status == "active")
    {
        Ok(())
    } else {
        bail!("The selected learner is unavailable")
    }
}

async fn ensure_allocation_learner(
    pool: &PgPool,
    tenant_id: Uuid,
    allocation_id: Option<Uuid>,
    learner_id: Uuid,
) -> Result<()> {
    let Some(allocation_id) = allocation_id else {
        return Ok(());
    };
    let matches = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM hostel_allocations WHERE tenant_id=$1 AND id=$2 AND learner_id=$3)",
    )
    .bind(tenant_id)
    .bind(allocation_id)
    .bind(learner_id)
    .fetch_one(pool)
    .await?;
    if !matches {
        bail!("The selected allocation does not belong to this learner")
    }
    Ok(())
}

async fn preview_room(
    pool: &PgPool,
    tenant_id: Uuid,
    room_id: Uuid,
    starts_on: NaiveDate,
    expected_end_on: Option<NaiveDate>,
    excluding: Option<Uuid>,
) -> Result<PreviewRoomRow> {
    sqlx::query_as::<_, PreviewRoomRow>(PREVIEW_ROOM_SELECT)
        .bind(tenant_id)
        .bind(room_id)
        .bind(starts_on)
        .bind(expected_end_on)
        .bind(excluding)
        .fetch_optional(pool)
        .await
        .context("Failed to load room availability")?
        .ok_or_else(|| anyhow!("The selected room was not found"))
}

async fn lock_room_and_revalidate(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    room_id: Uuid,
    starts_on: NaiveDate,
    expected_end_on: Option<NaiveDate>,
    excluding: Option<Uuid>,
) -> Result<()> {
    sqlx::query("SELECT id FROM hostel_rooms WHERE tenant_id=$1 AND id=$2 FOR UPDATE")
        .bind(tenant_id)
        .bind(room_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| anyhow!("The selected room was not found"))?;
    revalidate_room_capacity(
        transaction,
        tenant_id,
        room_id,
        starts_on,
        expected_end_on,
        excluding,
    )
    .await
}

async fn lock_rooms(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    first: Uuid,
    second: Uuid,
) -> Result<()> {
    let rows = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM hostel_rooms WHERE tenant_id=$1 AND id=ANY($2) ORDER BY id FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(vec![first, second])
    .fetch_all(&mut **transaction)
    .await?;
    if rows.len() != 2 {
        bail!("One of the selected rooms was not found")
    }
    Ok(())
}

async fn revalidate_room_capacity(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    room_id: Uuid,
    starts_on: NaiveDate,
    expected_end_on: Option<NaiveDate>,
    excluding: Option<Uuid>,
) -> Result<()> {
    let room = sqlx::query_as::<_, (i16, String, String)>(
        r#"SELECT room.capacity, room.status, residence.status
             FROM hostel_rooms room
             JOIN hostel_residences residence
               ON residence.id=room.residence_id AND residence.tenant_id=room.tenant_id
            WHERE room.tenant_id=$1 AND room.id=$2"#,
    )
    .bind(tenant_id)
    .bind(room_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| anyhow!("The selected room was not found"))?;
    if room.1 != "available" || room.2 != "active" {
        bail!("The selected room is not available");
    }
    let occupied = overlapping_allocation_count(
        transaction,
        tenant_id,
        room_id,
        starts_on,
        expected_end_on,
        excluding,
    )
    .await?;
    if occupied >= i64::from(room.0) {
        bail!("The selected room has no available bed for these dates")
    }
    Ok(())
}

async fn overlapping_allocation_count(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    room_id: Uuid,
    starts_on: NaiveDate,
    expected_end_on: Option<NaiveDate>,
    excluding: Option<Uuid>,
) -> Result<i64> {
    sqlx::query_scalar::<_, i64>(OVERLAPPING_COUNT)
        .bind(tenant_id)
        .bind(room_id)
        .bind(starts_on)
        .bind(expected_end_on)
        .bind(excluding)
        .fetch_one(&mut **transaction)
        .await
        .context("Failed to recheck room capacity")
}

fn require_preview(preview: &AllocationPreviewResponse, fingerprint: &str) -> Result<()> {
    if !preview.can_allocate {
        bail!(preview.issues.join(" "));
    }
    if preview.fingerprint != fingerprint {
        bail!("The allocation preview changed; preview it again before saving");
    }
    Ok(())
}

fn allocation_fingerprint(
    tenant_id: Uuid,
    learner_id: Uuid,
    room: &PreviewRoomRow,
    starts_on: NaiveDate,
    expected_end_on: Option<NaiveDate>,
    issues: &[String],
) -> String {
    let canonical = json!({
        "tenant_id": tenant_id,
        "learner_id": learner_id,
        "room_id": room.id,
        "room_version": room.version,
        "reserved_count": room.occupied_count,
        "starts_on": starts_on,
        "expected_end_on": expected_end_on,
        "issues": issues,
    });
    format!("{:x}", Sha256::digest(canonical.to_string().as_bytes()))
}

fn validate_allocation_dates(
    starts_on: NaiveDate,
    expected_end_on: Option<NaiveDate>,
) -> Result<()> {
    if expected_end_on.is_some_and(|value| value < starts_on) {
        bail!("The expected end date cannot be before the start date");
    }
    Ok(())
}

async fn visible_learner_filter(
    pool: &PgPool,
    tenant_id: Uuid,
    scope: HostelAccessScope,
) -> Result<Option<Vec<Uuid>>> {
    match scope {
        HostelAccessScope::Campus => Ok(None),
        HostelAccessScope::SelfFor(account_id) => Ok(Some(
            EnrolmentOps::learner_ids_for_account(pool, tenant_id, account_id).await?,
        )),
    }
}

async fn search_learner_ids(
    pool: &PgPool,
    tenant_id: Uuid,
    search: Option<&str>,
) -> Result<Option<Vec<Uuid>>> {
    let Some(search) = trimmed(search) else {
        return Ok(None);
    };
    Ok(Some(
        LearnerOps::hostel_references(pool, tenant_id, Some(search), 100)
            .await?
            .into_iter()
            .map(|value| value.id)
            .collect(),
    ))
}

async fn hydrate_allocations(
    pool: &PgPool,
    tenant_id: Uuid,
    rows: Vec<AllocationRow>,
) -> Result<Vec<AllocationResponse>> {
    let identities = learner_identity_map(
        pool,
        tenant_id,
        &rows.iter().map(|row| row.learner_id).collect::<Vec<_>>(),
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            let learner = identities
                .get(&row.learner_id)
                .ok_or_else(|| anyhow!("The allocation learner is unavailable"))?;
            Ok(AllocationResponse {
                id: row.id,
                learner_id: row.learner_id,
                learner_number: learner.learner_number.clone(),
                learner_name: learner.display_name.clone(),
                learner_status: learner.status.clone(),
                room_id: row.room_id,
                room_code: row.room_code,
                residence_id: row.residence_id,
                residence_code: row.residence_code,
                residence_name: row.residence_name,
                starts_on: row.starts_on,
                expected_end_on: row.expected_end_on,
                ended_on: row.ended_on,
                status: row.status,
                version: row.version,
                previous_allocation_id: row.previous_allocation_id,
                decision_reason: row.decision_reason,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
        })
        .collect()
}

async fn hydrate_pastoral(
    pool: &PgPool,
    tenant_id: Uuid,
    rows: Vec<PastoralRecordRow>,
) -> Result<Vec<PastoralRecordResponse>> {
    let identities = learner_identity_map(
        pool,
        tenant_id,
        &rows.iter().map(|row| row.learner_id).collect::<Vec<_>>(),
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            let learner = identities
                .get(&row.learner_id)
                .ok_or_else(|| anyhow!("The pastoral learner is unavailable"))?;
            Ok(PastoralRecordResponse {
                id: row.id,
                learner_id: row.learner_id,
                learner_number: learner.learner_number.clone(),
                learner_name: learner.display_name.clone(),
                allocation_id: row.allocation_id,
                residence_name: row.residence_name,
                room_code: row.room_code,
                category: row.category,
                severity: row.severity,
                subject: row.subject,
                details: row.details,
                occurred_at: row.occurred_at,
                status: row.status,
                resolution: row.resolution,
                version: row.version,
                resolved_at: row.resolved_at,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
        })
        .collect()
}

async fn learner_identity_map(
    pool: &PgPool,
    tenant_id: Uuid,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, cp_sis::models::HostelLearnerReference>> {
    let unique = ids
        .iter()
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    Ok(
        LearnerOps::hostel_references_by_ids(pool, tenant_id, &unique)
            .await?
            .into_iter()
            .map(|value| (value.id, value))
            .collect(),
    )
}

fn residence_response(row: ResidenceRow) -> ResidenceResponse {
    ResidenceResponse {
        id: row.id,
        code: row.code,
        name: row.name,
        description: row.description,
        status: row.status,
        version: row.version,
        room_count: row.room_count,
        bed_capacity: row.bed_capacity,
        occupied_count: row.occupied_count,
        available_beds: (row.bed_capacity - row.occupied_count).max(0),
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn room_response(row: RoomRow) -> RoomResponse {
    RoomResponse {
        id: row.id,
        residence_id: row.residence_id,
        residence_code: row.residence_code,
        residence_name: row.residence_name,
        code: row.code,
        floor_label: row.floor_label,
        capacity: row.capacity,
        occupied_count: row.occupied_count,
        available_beds: (i64::from(row.capacity) - row.occupied_count).max(0),
        status: row.status,
        version: row.version,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn bounded_page(query: &HostelListQuery) -> (i64, i64) {
    (
        query.page.unwrap_or(1).max(1),
        query.per_page.unwrap_or(25).clamp(1, 100),
    )
}
fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}
fn like(value: Option<&str>) -> Option<String> {
    trimmed(value).map(|value| format!("%{value}%"))
}
fn required<'a>(label: &str, value: &'a str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{label} is required")
    }
    Ok(value)
}
fn optional_text(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}
fn validate_status(value: Option<&str>, allowed: &[&str], label: &str) -> Result<()> {
    if let Some(value) = value.filter(|value| *value != "all")
        && !allowed.contains(&value)
    {
        bail!("The {label} filter is invalid")
    }
    Ok(())
}
fn person_actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("Authenticated person actor is required"))
}
fn map_unique(message: &'static str) -> impl FnOnce(sqlx::Error) -> anyhow::Error {
    move |error| match &error {
        sqlx::Error::Database(database) if database.is_unique_violation() => anyhow!(message),
        _ => anyhow!(error),
    }
}

async fn versioned_not_found<T>(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    table: &str,
    id: Uuid,
) -> Result<Option<T>> {
    let query = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE tenant_id=$1 AND id=$2)");
    if sqlx::query_scalar::<_, bool>(&query)
        .bind(tenant_id)
        .bind(id)
        .fetch_one(&mut **transaction)
        .await?
    {
        bail!("The record changed; reload it before saving");
    }
    Ok(None)
}

async fn allocation_transition_not_found<T>(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
    action: &str,
) -> Result<Option<T>> {
    let status = sqlx::query_scalar::<_, String>(
        "SELECT status FROM hostel_allocations WHERE tenant_id=$1 AND id=$2",
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(&mut **transaction)
    .await?;
    match status {
        None => Ok(None),
        Some(status) => bail!("The allocation cannot be {action} while it is {status}; reload it"),
    }
}

async fn pastoral_transition_not_found<T>(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
    action: &str,
) -> Result<Option<T>> {
    let status = sqlx::query_scalar::<_, String>(
        "SELECT status FROM hostel_pastoral_records WHERE tenant_id=$1 AND id=$2",
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(&mut **transaction)
    .await?;
    match status {
        None => Ok(None),
        Some(status) => {
            bail!("The pastoral record cannot be {action} while it is {status}; reload it")
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Hostel evidence keeps aggregate and learner scope explicit"
)]
async fn append_evidence(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    aggregate_type: &str,
    aggregate_id: Uuid,
    learner_id: Option<Uuid>,
    event_type: &str,
    action: &str,
    metadata: Value,
) -> Result<()> {
    let actor_id = person_actor_id(actor)?;
    sqlx::query(
        r#"INSERT INTO hostel_activity_events (
               tenant_id, aggregate_type, aggregate_id, learner_id, event_type, actor_id, metadata
           ) VALUES ($1,$2,$3,$4,$5,$6,$7)"#,
    )
    .bind(tenant_id)
    .bind(aggregate_type)
    .bind(aggregate_id)
    .bind(learner_id)
    .bind(event_type)
    .bind(actor_id)
    .bind(metadata.clone())
    .execute(&mut **transaction)
    .await
    .context("Failed to append Hostel activity evidence")?;
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
        .with_redacted_metadata(metadata.as_object().cloned().unwrap_or_else(Map::new)),
    )
    .await
    .context("Failed to append Hostel audit evidence")?;
    Ok(())
}

const RESIDENCE_SELECT: &str = r#"
    SELECT residence.id, residence.code, residence.name, residence.description,
           residence.status, residence.version,
           (SELECT COUNT(*) FROM hostel_rooms room WHERE room.tenant_id=residence.tenant_id AND room.residence_id=residence.id) AS room_count,
           (SELECT COALESCE(SUM(room.capacity),0)::BIGINT FROM hostel_rooms room WHERE room.tenant_id=residence.tenant_id AND room.residence_id=residence.id AND room.status <> 'inactive') AS bed_capacity,
           (SELECT COUNT(*) FROM hostel_allocations allocation JOIN hostel_rooms room ON room.id=allocation.room_id AND room.tenant_id=allocation.tenant_id WHERE allocation.tenant_id=residence.tenant_id AND room.residence_id=residence.id AND allocation.status='active') AS occupied_count,
           residence.created_at, residence.updated_at
      FROM hostel_residences residence
     WHERE residence.tenant_id=$1 AND ($2::TEXT IS NULL OR residence.status=$2)
       AND ($3::TEXT IS NULL OR residence.code ILIKE $3 OR residence.name ILIKE $3)
     ORDER BY residence.name LIMIT $4 OFFSET $5
"#;
const RESIDENCE_COUNT: &str = r#"
    SELECT COUNT(*) FROM hostel_residences residence
     WHERE residence.tenant_id=$1 AND ($2::TEXT IS NULL OR residence.status=$2)
       AND ($3::TEXT IS NULL OR residence.code ILIKE $3 OR residence.name ILIKE $3)
"#;
const RESIDENCE_BY_ID: &str = r#"
    SELECT residence.id, residence.code, residence.name, residence.description,
           residence.status, residence.version,
           (SELECT COUNT(*) FROM hostel_rooms room WHERE room.tenant_id=residence.tenant_id AND room.residence_id=residence.id) AS room_count,
           (SELECT COALESCE(SUM(room.capacity),0)::BIGINT FROM hostel_rooms room WHERE room.tenant_id=residence.tenant_id AND room.residence_id=residence.id AND room.status <> 'inactive') AS bed_capacity,
           (SELECT COUNT(*) FROM hostel_allocations allocation JOIN hostel_rooms room ON room.id=allocation.room_id AND room.tenant_id=allocation.tenant_id WHERE allocation.tenant_id=residence.tenant_id AND room.residence_id=residence.id AND allocation.status='active') AS occupied_count,
           residence.created_at, residence.updated_at
      FROM hostel_residences residence WHERE residence.tenant_id=$1 AND residence.id=$2
"#;
const ROOM_SELECT: &str = r#"
    SELECT room.id, room.residence_id, residence.code AS residence_code,
           residence.name AS residence_name, room.code, room.floor_label,
           room.capacity, (SELECT COUNT(*) FROM hostel_allocations allocation WHERE allocation.tenant_id=room.tenant_id AND allocation.room_id=room.id AND allocation.status='active') AS occupied_count,
           room.status, room.version, room.created_at, room.updated_at
      FROM hostel_rooms room JOIN hostel_residences residence
        ON residence.id=room.residence_id AND residence.tenant_id=room.tenant_id
     WHERE room.tenant_id=$1 AND ($2::TEXT IS NULL OR room.status=$2)
       AND ($3::UUID IS NULL OR room.residence_id=$3)
       AND ($4::TEXT IS NULL OR room.code ILIKE $4 OR residence.name ILIKE $4 OR residence.code ILIKE $4)
     ORDER BY residence.name, room.code LIMIT $5 OFFSET $6
"#;
const ROOM_COUNT: &str = r#"
    SELECT COUNT(*) FROM hostel_rooms room JOIN hostel_residences residence
      ON residence.id=room.residence_id AND residence.tenant_id=room.tenant_id
     WHERE room.tenant_id=$1 AND ($2::TEXT IS NULL OR room.status=$2)
       AND ($3::UUID IS NULL OR room.residence_id=$3)
       AND ($4::TEXT IS NULL OR room.code ILIKE $4 OR residence.name ILIKE $4 OR residence.code ILIKE $4)
"#;
const ROOM_BY_ID: &str = r#"
    SELECT room.id, room.residence_id, residence.code AS residence_code,
           residence.name AS residence_name, room.code, room.floor_label,
           room.capacity, (SELECT COUNT(*) FROM hostel_allocations allocation WHERE allocation.tenant_id=room.tenant_id AND allocation.room_id=room.id AND allocation.status='active') AS occupied_count,
           room.status, room.version, room.created_at, room.updated_at
      FROM hostel_rooms room JOIN hostel_residences residence
        ON residence.id=room.residence_id AND residence.tenant_id=room.tenant_id
     WHERE room.tenant_id=$1 AND room.id=$2
"#;
const PREVIEW_ROOM_SELECT: &str = r#"
    SELECT room.id, residence.name AS residence_name,
           room.code, room.capacity,
           CASE WHEN residence.status='active' THEN room.status ELSE 'inactive' END AS status,
           room.version,
           (SELECT COUNT(*) FROM hostel_allocations allocation
             WHERE allocation.tenant_id=room.tenant_id AND allocation.room_id=room.id
               AND allocation.status IN ('planned','active')
               AND ($5::UUID IS NULL OR allocation.id <> $5)
               AND allocation.starts_on <= COALESCE($4::DATE, 'infinity'::DATE)
               AND COALESCE(allocation.expected_end_on, 'infinity'::DATE) >= $3) AS occupied_count
      FROM hostel_rooms room JOIN hostel_residences residence
        ON residence.id=room.residence_id AND residence.tenant_id=room.tenant_id
     WHERE room.tenant_id=$1 AND room.id=$2
"#;
const OVERLAPPING_COUNT: &str = r#"
    SELECT COUNT(*) FROM hostel_allocations allocation
     WHERE allocation.tenant_id=$1 AND allocation.room_id=$2
       AND allocation.status IN ('planned','active')
       AND ($5::UUID IS NULL OR allocation.id <> $5)
       AND allocation.starts_on <= COALESCE($4::DATE, 'infinity'::DATE)
       AND COALESCE(allocation.expected_end_on, 'infinity'::DATE) >= $3
"#;
const ALLOCATION_SELECT: &str = r#"
    SELECT allocation.id, allocation.learner_id, allocation.room_id,
           residence.id AS residence_id, residence.code AS residence_code,
           residence.name AS residence_name, room.code AS room_code,
           allocation.starts_on, allocation.expected_end_on, allocation.ended_on,
           allocation.status, allocation.version, allocation.previous_allocation_id,
           allocation.decision_reason, allocation.created_at, allocation.updated_at
      FROM hostel_allocations allocation
      JOIN hostel_rooms room ON room.id=allocation.room_id AND room.tenant_id=allocation.tenant_id
      JOIN hostel_residences residence ON residence.id=room.residence_id AND residence.tenant_id=room.tenant_id
     WHERE allocation.tenant_id=$1 AND ($2::TEXT IS NULL OR allocation.status=$2)
       AND ($3::UUID IS NULL OR room.residence_id=$3) AND ($4::UUID IS NULL OR allocation.room_id=$4)
       AND ($5::UUID IS NULL OR allocation.learner_id=$5)
       AND ($6::UUID[] IS NULL OR allocation.learner_id=ANY($6))
       AND ($7::UUID[] IS NULL OR allocation.learner_id=ANY($7))
     ORDER BY CASE allocation.status WHEN 'active' THEN 0 WHEN 'planned' THEN 1 ELSE 2 END,
              allocation.starts_on DESC, allocation.created_at DESC
     LIMIT $8 OFFSET $9
"#;
const ALLOCATION_COUNT: &str = r#"
    SELECT COUNT(*) FROM hostel_allocations allocation
      JOIN hostel_rooms room ON room.id=allocation.room_id AND room.tenant_id=allocation.tenant_id
     WHERE allocation.tenant_id=$1 AND ($2::TEXT IS NULL OR allocation.status=$2)
       AND ($3::UUID IS NULL OR room.residence_id=$3) AND ($4::UUID IS NULL OR allocation.room_id=$4)
       AND ($5::UUID IS NULL OR allocation.learner_id=$5)
       AND ($6::UUID[] IS NULL OR allocation.learner_id=ANY($6))
       AND ($7::UUID[] IS NULL OR allocation.learner_id=ANY($7))
"#;
const ALLOCATION_BY_ID: &str = r#"
    SELECT allocation.id, allocation.learner_id, allocation.room_id,
           residence.id AS residence_id, residence.code AS residence_code,
           residence.name AS residence_name, room.code AS room_code,
           allocation.starts_on, allocation.expected_end_on, allocation.ended_on,
           allocation.status, allocation.version, allocation.previous_allocation_id,
           allocation.decision_reason, allocation.created_at, allocation.updated_at
      FROM hostel_allocations allocation
      JOIN hostel_rooms room ON room.id=allocation.room_id AND room.tenant_id=allocation.tenant_id
      JOIN hostel_residences residence ON residence.id=room.residence_id AND residence.tenant_id=room.tenant_id
     WHERE allocation.tenant_id=$1 AND allocation.id=$2
       AND ($3::UUID[] IS NULL OR allocation.learner_id=ANY($3))
"#;
const PASTORAL_SELECT: &str = r#"
    SELECT record.id, record.learner_id, record.allocation_id,
           residence.name AS residence_name, room.code AS room_code,
           record.category, record.severity, record.subject, record.details,
           record.occurred_at, record.status, record.resolution, record.version,
           record.resolved_at, record.created_at, record.updated_at
      FROM hostel_pastoral_records record
      LEFT JOIN hostel_allocations allocation ON allocation.id=record.allocation_id AND allocation.tenant_id=record.tenant_id
      LEFT JOIN hostel_rooms room ON room.id=allocation.room_id AND room.tenant_id=allocation.tenant_id
      LEFT JOIN hostel_residences residence ON residence.id=room.residence_id AND residence.tenant_id=room.tenant_id
     WHERE record.tenant_id=$1 AND ($2::TEXT IS NULL OR record.status=$2)
       AND ($3::TEXT IS NULL OR record.category=$3) AND ($4::UUID IS NULL OR record.learner_id=$4)
       AND ($5::UUID[] IS NULL OR record.learner_id=ANY($5))
     ORDER BY CASE record.status WHEN 'open' THEN 0 ELSE 1 END,
              CASE record.severity WHEN 'critical' THEN 0 WHEN 'high' THEN 1 WHEN 'moderate' THEN 2 ELSE 3 END,
              record.occurred_at DESC LIMIT $6 OFFSET $7
"#;
const PASTORAL_COUNT: &str = r#"
    SELECT COUNT(*) FROM hostel_pastoral_records record
     WHERE record.tenant_id=$1 AND ($2::TEXT IS NULL OR record.status=$2)
       AND ($3::TEXT IS NULL OR record.category=$3) AND ($4::UUID IS NULL OR record.learner_id=$4)
       AND ($5::UUID[] IS NULL OR record.learner_id=ANY($5))
"#;
const PASTORAL_BY_ID: &str = r#"
    SELECT record.id, record.learner_id, record.allocation_id,
           residence.name AS residence_name, room.code AS room_code,
           record.category, record.severity, record.subject, record.details,
           record.occurred_at, record.status, record.resolution, record.version,
           record.resolved_at, record.created_at, record.updated_at
      FROM hostel_pastoral_records record
      LEFT JOIN hostel_allocations allocation ON allocation.id=record.allocation_id AND allocation.tenant_id=record.tenant_id
      LEFT JOIN hostel_rooms room ON room.id=allocation.room_id AND room.tenant_id=allocation.tenant_id
      LEFT JOIN hostel_residences residence ON residence.id=room.residence_id AND residence.tenant_id=room.tenant_id
     WHERE record.tenant_id=$1 AND record.id=$2
"#;

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use uuid::Uuid;

    use crate::models::PreviewRoomRow;

    use super::{allocation_fingerprint, validate_allocation_dates};

    #[test]
    fn allocation_dates_reject_an_end_before_the_start() {
        let start = NaiveDate::from_ymd_opt(2026, 9, 2).unwrap_or_else(|| unreachable!());
        let end = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap_or_else(|| unreachable!());
        assert!(validate_allocation_dates(start, Some(end)).is_err());
    }

    #[test]
    fn preview_fingerprint_changes_with_capacity_evidence() {
        let room = PreviewRoomRow {
            id: Uuid::new_v4(),
            residence_name: "North House".to_string(),
            code: "N-01".to_string(),
            capacity: 4,
            status: "available".to_string(),
            version: 1,
            occupied_count: 2,
        };
        let date = NaiveDate::from_ymd_opt(2026, 9, 2).unwrap_or_else(|| unreachable!());
        let tenant_id = Uuid::new_v4();
        let learner_id = Uuid::new_v4();
        let first = allocation_fingerprint(tenant_id, learner_id, &room, date, None, &[]);
        let mut changed = room.clone();
        changed.occupied_count = 3;
        let second = allocation_fingerprint(tenant_id, learner_id, &changed, date, None, &[]);
        assert_ne!(first, second);
    }
}
