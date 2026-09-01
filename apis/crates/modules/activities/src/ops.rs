//! Transactional Activities workflows and record-scoped projections.
//!
//! Every mutation re-proves tenant and assignment scope under row locks. SIS
//! and HR identities are resolved through typed module operations, while
//! completed session rosters are retained as immutable evidence.

use std::collections::HashMap;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, NaiveDate, Utc};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_hr_payroll::ops::EmployeeOps;
use cp_sis::ops::{EnrolmentOps, LearnerOps};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    ActivitiesReferenceData, ActivitiesScope, ActivityCatalogItemResponse, ActivityCatalogQuery,
    ActivityConsentStatus, ActivityGroupQuery, ActivityGroupRecord, ActivityGroupSummary,
    ActivityLeaderResponse, ActivityLifecycleEventResponse, ActivityMembershipResponse,
    ActivityMembershipStatus, ActivityParticipationResponse, ActivityReferenceQuery,
    ActivitySessionQuery, ActivitySessionRecord, ActivitySessionSummary, ActivityTransitionRequest,
    AddActivityLeaderRequest, AddActivityMembershipRequest, ArchiveActivityCatalogItemRequest,
    CancelActivitySessionRequest, CompleteActivitySessionRequest, CreateActivityCatalogItemRequest,
    CreateActivityGroupRequest, CreateActivitySessionRequest, EndActivityLeaderRequest,
    EndActivityMembershipRequest, MarkActivityParticipationRequest,
    UpdateActivityCatalogItemRequest, UpdateActivityGroupRequest, UpdateActivityMembershipRequest,
    UpdateActivitySessionRequest,
    models::{
        CatalogRow, EventRow, GroupRow, LeaderRow, LockedGroup, LockedSession, MembershipRow,
        ParticipationRow, SessionRow,
    },
};

pub struct ActivitiesOps;

impl ActivitiesOps {
    pub async fn reference_data(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &ActivityReferenceQuery,
    ) -> Result<ActivitiesReferenceData> {
        let learners =
            LearnerOps::activity_references(pool, tenant_id, query.search.as_deref(), 100).await?;
        let employees = EmployeeOps::list_references(
            pool,
            tenant_id,
            query.search.as_deref(),
            Some("active"),
            100,
        )
        .await?;
        Ok(ActivitiesReferenceData {
            learners,
            employees,
        })
    }

    pub async fn list_catalog(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &ActivityCatalogQuery,
    ) -> Result<Vec<ActivityCatalogItemResponse>> {
        let search = search_pattern(query.search.as_deref());
        sqlx::query_as::<_, CatalogRow>(CATALOG_LIST)
            .bind(tenant_id)
            .bind(search.as_deref())
            .bind(query.category.map(|value| value.as_str()))
            .bind(query.status.map(|value| value.as_str()))
            .fetch_all(pool)
            .await
            .context("Failed to list the Activities catalog")
            .map(|rows| rows.into_iter().map(catalog_response).collect())
    }

    pub async fn get_catalog_item(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<ActivityCatalogItemResponse>> {
        sqlx::query_as::<_, CatalogRow>(CATALOG_BY_ID)
            .bind(tenant_id)
            .bind(id)
            .fetch_optional(pool)
            .await
            .context("Failed to read the Activities catalog item")
            .map(|row| row.map(catalog_response))
    }

    pub async fn create_catalog_item(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &CreateActivityCatalogItemRequest,
    ) -> Result<ActivityCatalogItemResponse> {
        let actor_id = person_actor_id(actor)?;
        let id = Uuid::new_v4();
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Activities catalog creation")?;
        sqlx::query(
            r#"
            INSERT INTO activity_catalog_items (
                id, tenant_id, code, name, category, description, created_by, updated_by
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$7)
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .bind(trimmed_required(&request.code, "Activity code")?)
        .bind(trimmed_required(&request.name, "Activity name")?)
        .bind(request.category.as_str())
        .bind(trimmed_optional(request.description.as_deref()))
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "An activity with this code already exists"))?;
        append_domain_audit(
            &mut transaction,
            tenant_id,
            actor,
            context,
            "activities.catalog.create",
            "activity_catalog_item",
            id,
            json!({"code": request.code.trim(), "category": request.category.as_str()}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Activities catalog creation")?;
        Self::get_catalog_item(pool, tenant_id, id)
            .await?
            .ok_or_else(|| anyhow!("The Activities catalog item could not be reloaded"))
    }

    pub async fn update_catalog_item(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &UpdateActivityCatalogItemRequest,
    ) -> Result<Option<ActivityCatalogItemResponse>> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Activities catalog update")?;
        let current = sqlx::query_as::<_, (i32, String)>(
            "SELECT version, status FROM activity_catalog_items WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL FOR UPDATE",
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to lock the Activities catalog item")?;
        let Some((version, status)) = current else {
            return Ok(None);
        };
        ensure_version(version, request.expected_version, "Activities catalog item")?;
        if status != "active" {
            bail!("An archived activity cannot be changed");
        }
        sqlx::query(
            r#"
            UPDATE activity_catalog_items
               SET code=$1, name=$2, category=$3, description=$4,
                   updated_by=$5, version=version+1, updated_at=NOW()
             WHERE tenant_id=$6 AND id=$7 AND deleted_at IS NULL
            "#,
        )
        .bind(trimmed_required(&request.code, "Activity code")?)
        .bind(trimmed_required(&request.name, "Activity name")?)
        .bind(request.category.as_str())
        .bind(trimmed_optional(request.description.as_deref()))
        .bind(actor_id)
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "An activity with this code already exists"))?;
        append_domain_audit(
            &mut transaction,
            tenant_id,
            actor,
            context,
            "activities.catalog.update",
            "activity_catalog_item",
            id,
            json!({"expected_version": request.expected_version}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Activities catalog update")?;
        Self::get_catalog_item(pool, tenant_id, id).await
    }

    pub async fn archive_catalog_item(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &ArchiveActivityCatalogItemRequest,
    ) -> Result<Option<ActivityCatalogItemResponse>> {
        let actor_id = person_actor_id(actor)?;
        let reason = trimmed_required(&request.reason, "Archive reason")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Activities catalog archive")?;
        let current = sqlx::query_as::<_, (i32, String)>(
            "SELECT version, status FROM activity_catalog_items WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL FOR UPDATE",
        ).bind(tenant_id).bind(id).fetch_optional(&mut *transaction).await
            .context("Failed to lock the Activities catalog item")?;
        let Some((version, status)) = current else {
            return Ok(None);
        };
        ensure_version(version, request.expected_version, "Activities catalog item")?;
        if status != "active" {
            bail!("The activity is already archived");
        }
        let active_groups = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM activity_groups WHERE tenant_id=$1 AND activity_id=$2 AND status IN ('draft','active') AND deleted_at IS NULL",
        ).bind(tenant_id).bind(id).fetch_one(&mut *transaction).await
            .context("Failed to check activity groups")?;
        if active_groups > 0 {
            bail!("Close or cancel every current group before archiving this activity");
        }
        sqlx::query(
            "UPDATE activity_catalog_items SET status='archived', archived_at=NOW(), archived_by=$1, archive_reason=$2, updated_by=$1, version=version+1, updated_at=NOW() WHERE tenant_id=$3 AND id=$4 AND deleted_at IS NULL",
        ).bind(actor_id).bind(reason).bind(tenant_id).bind(id).execute(&mut *transaction).await
            .context("Failed to archive the activity")?;
        append_domain_audit(
            &mut transaction,
            tenant_id,
            actor,
            context,
            "activities.catalog.archive",
            "activity_catalog_item",
            id,
            json!({"reason": reason}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Activities catalog archive")?;
        Self::get_catalog_item(pool, tenant_id, id).await
    }

    pub async fn list_groups(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: ActivitiesScope,
        query: &ActivityGroupQuery,
    ) -> Result<(Vec<ActivityGroupSummary>, i64)> {
        let visibility = resolve_visibility(pool, tenant_id, scope).await?;
        let (page, per_page) = bounded_page(query.page, query.per_page);
        let search = search_pattern(query.search.as_deref());
        let offset = (page - 1) * per_page;
        let list_sql = format!(
            "{GROUP_PROJECTION} AND ($5::TEXT IS NULL OR activity_group.code ILIKE $5 OR activity_group.name ILIKE $5 OR activity.name ILIKE $5) AND ($6::UUID IS NULL OR activity_group.activity_id=$6) AND ($7::TEXT IS NULL OR activity_group.status=$7) AND ($8::DATE IS NULL OR (activity_group.starts_on <= $8 AND activity_group.ends_on >= $8)) ORDER BY activity_group.status='active' DESC, activity_group.starts_on DESC, activity_group.name LIMIT $9 OFFSET $10"
        );
        let rows = sqlx::query_as::<_, GroupRow>(&list_sql)
            .bind(tenant_id)
            .bind(visibility.campus)
            .bind(visibility.employee_id)
            .bind(&visibility.learner_ids)
            .bind(search.as_deref())
            .bind(query.activity_id)
            .bind(query.status.map(|value| value.as_str()))
            .bind(query.active_on)
            .bind(per_page)
            .bind(offset)
            .fetch_all(pool)
            .await
            .context("Failed to list Activities groups")?;
        let count_sql = format!(
            "SELECT COUNT(*) FROM ({GROUP_PROJECTION} AND ($5::TEXT IS NULL OR activity_group.code ILIKE $5 OR activity_group.name ILIKE $5 OR activity.name ILIKE $5) AND ($6::UUID IS NULL OR activity_group.activity_id=$6) AND ($7::TEXT IS NULL OR activity_group.status=$7) AND ($8::DATE IS NULL OR (activity_group.starts_on <= $8 AND activity_group.ends_on >= $8))) AS visible_group"
        );
        let total = sqlx::query_scalar::<_, i64>(&count_sql)
            .bind(tenant_id)
            .bind(visibility.campus)
            .bind(visibility.employee_id)
            .bind(&visibility.learner_ids)
            .bind(search.as_deref())
            .bind(query.activity_id)
            .bind(query.status.map(|value| value.as_str()))
            .bind(query.active_on)
            .fetch_one(pool)
            .await
            .context("Failed to count Activities groups")?;
        Ok((rows.into_iter().map(group_summary).collect(), total))
    }

    pub async fn get_group(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: ActivitiesScope,
        id: Uuid,
    ) -> Result<Option<ActivityGroupRecord>> {
        let visibility = resolve_visibility(pool, tenant_id, scope).await?;
        let by_id_sql = format!("{GROUP_PROJECTION} AND activity_group.id=$5");
        let row = sqlx::query_as::<_, GroupRow>(&by_id_sql)
            .bind(tenant_id)
            .bind(visibility.campus)
            .bind(visibility.employee_id)
            .bind(&visibility.learner_ids)
            .bind(id)
            .fetch_optional(pool)
            .await
            .context("Failed to read the Activities group")?;
        let Some(row) = row else {
            return Ok(None);
        };
        let can_read_roster = can_read_group_roster(pool, tenant_id, &visibility, id).await?;
        let leaders = group_leaders(pool, tenant_id, id).await?;
        let memberships = if can_read_roster {
            group_memberships(pool, tenant_id, id, None).await?
        } else {
            group_memberships(pool, tenant_id, id, Some(&visibility.learner_ids)).await?
        };
        let history = if can_read_roster {
            event_history(pool, tenant_id, Some(id), None).await?
        } else {
            Vec::new()
        };
        Ok(Some(ActivityGroupRecord {
            consent_instructions: row.consent_instructions.clone(),
            group: group_summary(row),
            leaders,
            memberships,
            history,
        }))
    }

    pub async fn create_group(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &CreateActivityGroupRequest,
    ) -> Result<ActivityGroupRecord> {
        validate_group_dates(request.starts_on, request.ends_on)?;
        validate_consent(
            request.consent_required,
            request.consent_instructions.as_deref(),
        )?;
        let actor_id = person_actor_id(actor)?;
        let id = Uuid::new_v4();
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Activities group creation")?;
        ensure_active_catalog(&mut transaction, tenant_id, request.activity_id).await?;
        sqlx::query(
            r#"
            INSERT INTO activity_groups (
                id, tenant_id, activity_id, code, name, starts_on, ends_on, capacity,
                consent_required, consent_instructions, created_by, updated_by
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$11)
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .bind(request.activity_id)
        .bind(trimmed_required(&request.code, "Group code")?)
        .bind(trimmed_required(&request.name, "Group name")?)
        .bind(request.starts_on)
        .bind(request.ends_on)
        .bind(request.capacity)
        .bind(request.consent_required)
        .bind(trimmed_optional(request.consent_instructions.as_deref()))
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            database_error(error, "An Activities group with this code already exists")
        })?;
        append_activity_evidence(
            &mut transaction,
            ActivityEvidence {
                tenant_id,
                group_id: Some(id),
                session_id: None,
                actor,
                context,
                event_type: "activities.group.created",
                operation: "activities.groups.create",
                target_kind: "activity_group",
                target_id: id,
                metadata: json!({"activity_id": request.activity_id, "code": request.code.trim()}),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Activities group creation")?;
        Self::get_group(pool, tenant_id, ActivitiesScope::Campus, id)
            .await?
            .ok_or_else(|| anyhow!("The Activities group could not be reloaded"))
    }

    pub async fn update_group(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &UpdateActivityGroupRequest,
    ) -> Result<Option<ActivityGroupRecord>> {
        validate_group_dates(request.starts_on, request.ends_on)?;
        validate_consent(
            request.consent_required,
            request.consent_instructions.as_deref(),
        )?;
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Activities group update")?;
        let Some(current) = lock_group(&mut transaction, tenant_id, id, None).await? else {
            return Ok(None);
        };
        ensure_version(
            current.version,
            request.expected_version,
            "Activities group",
        )?;
        if current.status != "draft" {
            bail!("Only a draft Activities group can be changed");
        }
        ensure_active_catalog(&mut transaction, tenant_id, request.activity_id).await?;
        ensure_roster_inside_dates(
            &mut transaction,
            tenant_id,
            id,
            request.starts_on,
            request.ends_on,
        )
        .await?;
        sqlx::query(
            r#"
            UPDATE activity_groups
               SET activity_id=$1, code=$2, name=$3, starts_on=$4, ends_on=$5,
                   capacity=$6, consent_required=$7, consent_instructions=$8,
                   updated_by=$9, version=version+1, updated_at=NOW()
             WHERE tenant_id=$10 AND id=$11 AND deleted_at IS NULL
            "#,
        )
        .bind(request.activity_id)
        .bind(trimmed_required(&request.code, "Group code")?)
        .bind(trimmed_required(&request.name, "Group name")?)
        .bind(request.starts_on)
        .bind(request.ends_on)
        .bind(request.capacity)
        .bind(request.consent_required)
        .bind(trimmed_optional(request.consent_instructions.as_deref()))
        .bind(actor_id)
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            database_error(error, "An Activities group with this code already exists")
        })?;
        append_activity_evidence(
            &mut transaction,
            ActivityEvidence {
                tenant_id,
                group_id: Some(id),
                session_id: None,
                actor,
                context,
                event_type: "activities.group.updated",
                operation: "activities.groups.update",
                target_kind: "activity_group",
                target_id: id,
                metadata: json!({"expected_version": request.expected_version}),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Activities group update")?;
        Self::get_group(pool, tenant_id, ActivitiesScope::Campus, id).await
    }

    pub async fn transition_group(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &ActivityTransitionRequest,
        transition: GroupTransition,
    ) -> Result<Option<ActivityGroupRecord>> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Activities group transition")?;
        let Some(current) = lock_group(&mut transaction, tenant_id, id, None).await? else {
            return Ok(None);
        };
        ensure_version(
            current.version,
            request.expected_version,
            "Activities group",
        )?;
        let (status, event_type, operation, reason) = match transition {
            GroupTransition::Activate => {
                if current.status != "draft" {
                    bail!("Only a draft Activities group can be activated");
                }
                ensure_active_catalog(&mut transaction, tenant_id, current.activity_id).await?;
                let leader_count = active_leader_count(&mut transaction, tenant_id, id).await?;
                let member_count = active_member_count(&mut transaction, tenant_id, id).await?;
                if leader_count == 0 {
                    bail!("Assign an active employee leader before activating the group");
                }
                if member_count == 0 {
                    bail!("Add at least one learner before activating the group");
                }
                if current
                    .capacity
                    .is_some_and(|capacity| member_count > i64::from(capacity))
                {
                    bail!("The active roster exceeds the group capacity");
                }
                sqlx::query("UPDATE activity_groups SET status='active', activated_at=NOW(), activated_by=$1, updated_by=$1, version=version+1, updated_at=NOW() WHERE tenant_id=$2 AND id=$3 AND deleted_at IS NULL")
                    .bind(actor_id).bind(tenant_id).bind(id).execute(&mut *transaction).await
                    .context("Failed to activate the Activities group")?;
                (
                    "active",
                    "activities.group.activated",
                    "activities.groups.activate",
                    None,
                )
            }
            GroupTransition::Close => {
                if current.status != "active" {
                    bail!("Only an active Activities group can be closed");
                }
                let reason = required_reason(request.reason.as_deref(), "Closure reason")?;
                let scheduled = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM activity_sessions WHERE tenant_id=$1 AND group_id=$2 AND status='scheduled' AND deleted_at IS NULL")
                    .bind(tenant_id).bind(id).fetch_one(&mut *transaction).await
                    .context("Failed to check scheduled activity sessions")?;
                if scheduled > 0 {
                    bail!("Complete or cancel every scheduled session before closing this group");
                }
                sqlx::query("UPDATE activity_groups SET status='closed', closed_at=NOW(), closed_by=$1, closure_reason=$2, updated_by=$1, version=version+1, updated_at=NOW() WHERE tenant_id=$3 AND id=$4 AND deleted_at IS NULL")
                    .bind(actor_id).bind(reason).bind(tenant_id).bind(id).execute(&mut *transaction).await
                    .context("Failed to close the Activities group")?;
                (
                    "closed",
                    "activities.group.closed",
                    "activities.groups.close",
                    Some(reason.to_string()),
                )
            }
            GroupTransition::Cancel => {
                if !matches!(current.status.as_str(), "draft" | "active") {
                    bail!("Only a draft or active Activities group can be cancelled");
                }
                let reason = required_reason(request.reason.as_deref(), "Cancellation reason")?;
                sqlx::query("UPDATE activity_sessions SET status='cancelled', cancelled_at=NOW(), cancelled_by=$1, cancellation_reason=$2, updated_by=$1, version=version+1, updated_at=NOW() WHERE tenant_id=$3 AND group_id=$4 AND status='scheduled' AND deleted_at IS NULL")
                    .bind(actor_id).bind(reason).bind(tenant_id).bind(id).execute(&mut *transaction).await
                    .context("Failed to cancel scheduled activity sessions")?;
                sqlx::query("UPDATE activity_groups SET status='cancelled', cancelled_at=NOW(), cancelled_by=$1, cancellation_reason=$2, updated_by=$1, version=version+1, updated_at=NOW() WHERE tenant_id=$3 AND id=$4 AND deleted_at IS NULL")
                    .bind(actor_id).bind(reason).bind(tenant_id).bind(id).execute(&mut *transaction).await
                    .context("Failed to cancel the Activities group")?;
                (
                    "cancelled",
                    "activities.group.cancelled",
                    "activities.groups.cancel",
                    Some(reason.to_string()),
                )
            }
        };
        append_activity_evidence(
            &mut transaction,
            ActivityEvidence {
                tenant_id,
                group_id: Some(id),
                session_id: None,
                actor,
                context,
                event_type,
                operation,
                target_kind: "activity_group",
                target_id: id,
                metadata: json!({"status": status, "reason": reason}),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Activities group transition")?;
        Self::get_group(pool, tenant_id, ActivitiesScope::Campus, id).await
    }
}

#[derive(Debug, Clone, Copy)]
pub enum GroupTransition {
    Activate,
    Close,
    Cancel,
}

impl ActivitiesOps {
    pub async fn add_leader(
        pool: &PgPool,
        tenant_id: Uuid,
        group_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &AddActivityLeaderRequest,
    ) -> Result<Option<ActivityGroupRecord>> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start activity leader assignment")?;
        let Some(group) = lock_group(&mut transaction, tenant_id, group_id, None).await? else {
            return Ok(None);
        };
        if !matches!(group.status.as_str(), "draft" | "active") {
            bail!("Leaders can only be assigned to a draft or active group");
        }
        validate_effective_dates(
            request.starts_on,
            request.ends_on,
            group.starts_on,
            group.ends_on,
            "Leader dates",
        )?;
        let employee = EmployeeOps::get_reference(pool, tenant_id, request.employee_id)
            .await?
            .filter(|employee| employee.employment_status == "active")
            .ok_or_else(|| anyhow!("The selected employee is not active"))?;
        let leader_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO activity_group_leaders (
                   id, tenant_id, group_id, employee_id, leader_role, starts_on, ends_on,
                   created_by, updated_by
               ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$8)"#,
        )
        .bind(leader_id)
        .bind(tenant_id)
        .bind(group_id)
        .bind(request.employee_id)
        .bind(request.role.as_str())
        .bind(request.starts_on)
        .bind(request.ends_on)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            database_error(
                error,
                "This employee is already an active leader for the group",
            )
        })?;
        append_activity_evidence(
            &mut transaction,
            ActivityEvidence {
                tenant_id,
                group_id: Some(group_id),
                session_id: None,
                actor,
                context,
                event_type: "activities.group.leader_assigned",
                operation: "activities.groups.leaders.assign",
                target_kind: "activity_group_leader",
                target_id: leader_id,
                metadata: json!({"employee_id": employee.id, "role": request.role.as_str()}),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit activity leader assignment")?;
        Self::get_group(pool, tenant_id, ActivitiesScope::Campus, group_id).await
    }

    pub async fn end_leader(
        pool: &PgPool,
        tenant_id: Uuid,
        group_id: Uuid,
        leader_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &EndActivityLeaderRequest,
    ) -> Result<Option<ActivityGroupRecord>> {
        let actor_id = person_actor_id(actor)?;
        let reason = trimmed_required(&request.reason, "Leader end reason")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start activity leader end")?;
        let Some(group) = lock_group(&mut transaction, tenant_id, group_id, None).await? else {
            return Ok(None);
        };
        if !matches!(group.status.as_str(), "draft" | "active") {
            bail!("Leaders can only be ended on a draft or active group");
        }
        let current = sqlx::query_as::<_, (i32, NaiveDate, Option<chrono::DateTime<Utc>>)>(
            "SELECT version, starts_on, ended_at FROM activity_group_leaders WHERE tenant_id=$1 AND group_id=$2 AND id=$3 AND deleted_at IS NULL FOR UPDATE",
        ).bind(tenant_id).bind(group_id).bind(leader_id).fetch_optional(&mut *transaction).await
            .context("Failed to lock the activity leader")?;
        let Some((version, starts_on, ended_at)) = current else {
            return Ok(None);
        };
        ensure_version(version, request.expected_version, "Activity leader")?;
        if ended_at.is_some() {
            bail!("The activity leader assignment has already ended");
        }
        validate_effective_dates(
            starts_on,
            Some(request.ends_on),
            group.starts_on,
            group.ends_on,
            "Leader dates",
        )?;
        if group.status == "active"
            && active_leader_count(&mut transaction, tenant_id, group_id).await? <= 1
        {
            bail!("Assign another active leader before ending the group's final leader");
        }
        sqlx::query("UPDATE activity_group_leaders SET ends_on=$1, ended_at=NOW(), ended_by=$2, end_reason=$3, updated_by=$2, version=version+1, updated_at=NOW() WHERE tenant_id=$4 AND group_id=$5 AND id=$6 AND deleted_at IS NULL")
            .bind(request.ends_on).bind(actor_id).bind(reason).bind(tenant_id).bind(group_id).bind(leader_id)
            .execute(&mut *transaction).await.context("Failed to end the activity leader")?;
        append_activity_evidence(
            &mut transaction,
            ActivityEvidence {
                tenant_id,
                group_id: Some(group_id),
                session_id: None,
                actor,
                context,
                event_type: "activities.group.leader_ended",
                operation: "activities.groups.leaders.end",
                target_kind: "activity_group_leader",
                target_id: leader_id,
                metadata: json!({"ends_on": request.ends_on, "reason": reason}),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit activity leader end")?;
        Self::get_group(pool, tenant_id, ActivitiesScope::Campus, group_id).await
    }

    pub async fn add_membership(
        pool: &PgPool,
        tenant_id: Uuid,
        group_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &AddActivityMembershipRequest,
    ) -> Result<Option<ActivityGroupRecord>> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start activity membership creation")?;
        let Some(group) = lock_group(&mut transaction, tenant_id, group_id, None).await? else {
            return Ok(None);
        };
        if !matches!(group.status.as_str(), "draft" | "active") {
            bail!("Learners can only be added to a draft or active group");
        }
        validate_effective_dates(
            request.joined_on,
            None,
            group.starts_on,
            group.ends_on,
            "Membership date",
        )?;
        let learner =
            LearnerOps::activity_references_by_ids(pool, tenant_id, &[request.learner_id])
                .await?
                .into_iter()
                .find(|learner| learner.status == "active")
                .ok_or_else(|| anyhow!("The selected learner is not active"))?;
        let member_count = active_member_count(&mut transaction, tenant_id, group_id).await?;
        if group
            .capacity
            .is_some_and(|capacity| member_count >= i64::from(capacity))
        {
            bail!("The activity group has reached its capacity");
        }
        let membership_id = Uuid::new_v4();
        let consent_status = if group.consent_required {
            "pending"
        } else {
            "not_required"
        };
        sqlx::query(
            r#"INSERT INTO activity_group_memberships (
                   id, tenant_id, group_id, learner_id, joined_on, consent_status,
                   created_by, updated_by
               ) VALUES ($1,$2,$3,$4,$5,$6,$7,$7)"#,
        )
        .bind(membership_id)
        .bind(tenant_id)
        .bind(group_id)
        .bind(request.learner_id)
        .bind(request.joined_on)
        .bind(consent_status)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            database_error(
                error,
                "This learner is already an active member of the group",
            )
        })?;
        append_activity_evidence(
            &mut transaction,
            ActivityEvidence {
                tenant_id,
                group_id: Some(group_id),
                session_id: None,
                actor,
                context,
                event_type: "activities.group.member_added",
                operation: "activities.groups.members.add",
                target_kind: "activity_group_membership",
                target_id: membership_id,
                metadata: json!({"learner_id": learner.id, "consent_status": consent_status}),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit activity membership creation")?;
        Self::get_group(pool, tenant_id, ActivitiesScope::Campus, group_id).await
    }

    pub async fn update_membership(
        pool: &PgPool,
        tenant_id: Uuid,
        group_id: Uuid,
        membership_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &UpdateActivityMembershipRequest,
    ) -> Result<Option<ActivityGroupRecord>> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start activity membership update")?;
        let Some(group) = lock_group(&mut transaction, tenant_id, group_id, None).await? else {
            return Ok(None);
        };
        if !matches!(group.status.as_str(), "draft" | "active") {
            bail!("Membership consent can only change on a draft or active group");
        }
        let current = sqlx::query_as::<_, (i32, String)>(
            "SELECT version, status FROM activity_group_memberships WHERE tenant_id=$1 AND group_id=$2 AND id=$3 AND deleted_at IS NULL FOR UPDATE",
        ).bind(tenant_id).bind(group_id).bind(membership_id).fetch_optional(&mut *transaction).await
            .context("Failed to lock the activity membership")?;
        let Some((version, status)) = current else {
            return Ok(None);
        };
        ensure_version(version, request.expected_version, "Activity membership")?;
        if status != "active" {
            bail!("An ended activity membership cannot be changed");
        }
        if group.consent_required && request.consent_status == ActivityConsentStatus::NotRequired {
            bail!("Consent cannot be marked not required for this group");
        }
        if !group.consent_required && request.consent_status != ActivityConsentStatus::NotRequired {
            bail!("This group does not require consent");
        }
        let recorded = matches!(
            request.consent_status,
            ActivityConsentStatus::Granted | ActivityConsentStatus::Declined
        );
        sqlx::query(
            r#"UPDATE activity_group_memberships
                  SET consent_status=$1, consent_recorded_at=CASE WHEN $2 THEN NOW() ELSE NULL END,
                      consent_recorded_by=CASE WHEN $2 THEN $3 ELSE NULL END,
                      consent_notes=$4, updated_by=$3, version=version+1, updated_at=NOW()
                WHERE tenant_id=$5 AND group_id=$6 AND id=$7 AND deleted_at IS NULL"#,
        )
        .bind(request.consent_status.as_str())
        .bind(recorded)
        .bind(actor_id)
        .bind(trimmed_optional(request.consent_notes.as_deref()))
        .bind(tenant_id)
        .bind(group_id)
        .bind(membership_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to update activity membership consent")?;
        append_activity_evidence(
            &mut transaction,
            ActivityEvidence {
                tenant_id,
                group_id: Some(group_id),
                session_id: None,
                actor,
                context,
                event_type: "activities.group.member_consent_updated",
                operation: "activities.groups.members.update",
                target_kind: "activity_group_membership",
                target_id: membership_id,
                metadata: json!({"consent_status": request.consent_status.as_str()}),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit activity membership update")?;
        Self::get_group(pool, tenant_id, ActivitiesScope::Campus, group_id).await
    }

    pub async fn end_membership(
        pool: &PgPool,
        tenant_id: Uuid,
        group_id: Uuid,
        membership_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &EndActivityMembershipRequest,
    ) -> Result<Option<ActivityGroupRecord>> {
        if request.outcome == ActivityMembershipStatus::Active {
            bail!("An ended membership must be ended or withdrawn");
        }
        let actor_id = person_actor_id(actor)?;
        let reason = trimmed_required(&request.reason, "Membership end reason")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start activity membership end")?;
        let Some(group) = lock_group(&mut transaction, tenant_id, group_id, None).await? else {
            return Ok(None);
        };
        if !matches!(group.status.as_str(), "draft" | "active") {
            bail!("Memberships can only end on a draft or active group");
        }
        let current = sqlx::query_as::<_, (i32, NaiveDate, String)>(
            "SELECT version, joined_on, status FROM activity_group_memberships WHERE tenant_id=$1 AND group_id=$2 AND id=$3 AND deleted_at IS NULL FOR UPDATE",
        ).bind(tenant_id).bind(group_id).bind(membership_id).fetch_optional(&mut *transaction).await
            .context("Failed to lock the activity membership")?;
        let Some((version, joined_on, status)) = current else {
            return Ok(None);
        };
        ensure_version(version, request.expected_version, "Activity membership")?;
        if status != "active" {
            bail!("The activity membership has already ended");
        }
        validate_effective_dates(
            joined_on,
            Some(request.ended_on),
            group.starts_on,
            group.ends_on,
            "Membership dates",
        )?;
        sqlx::query("UPDATE activity_group_memberships SET ended_on=$1, status=$2, ended_at=NOW(), ended_by=$3, end_reason=$4, updated_by=$3, version=version+1, updated_at=NOW() WHERE tenant_id=$5 AND group_id=$6 AND id=$7 AND deleted_at IS NULL")
            .bind(request.ended_on).bind(request.outcome.as_str()).bind(actor_id).bind(reason)
            .bind(tenant_id).bind(group_id).bind(membership_id).execute(&mut *transaction).await
            .context("Failed to end the activity membership")?;
        append_activity_evidence(&mut transaction, ActivityEvidence {
            tenant_id, group_id: Some(group_id), session_id: None, actor, context,
            event_type: "activities.group.member_ended", operation: "activities.groups.members.end",
            target_kind: "activity_group_membership", target_id: membership_id,
            metadata: json!({"status": request.outcome.as_str(), "ended_on": request.ended_on, "reason": reason}),
        }).await?;
        transaction
            .commit()
            .await
            .context("Failed to commit activity membership end")?;
        Self::get_group(pool, tenant_id, ActivitiesScope::Campus, group_id).await
    }

    pub async fn list_sessions(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: ActivitiesScope,
        query: &ActivitySessionQuery,
    ) -> Result<(Vec<ActivitySessionSummary>, i64)> {
        let visibility = resolve_visibility(pool, tenant_id, scope).await?;
        let (page, per_page) = bounded_page(query.page, query.per_page);
        let offset = (page - 1) * per_page;
        let search = search_pattern(query.search.as_deref());
        let list_sql = format!(
            "{SESSION_PROJECTION} AND ($5::TEXT IS NULL OR session.reference ILIKE $5 OR session.title ILIKE $5 OR activity_group.name ILIKE $5) AND ($6::UUID IS NULL OR session.group_id=$6) AND ($7::TEXT IS NULL OR session.status=$7) AND ($8::DATE IS NULL OR session.starts_at::DATE >= $8) AND ($9::DATE IS NULL OR session.starts_at::DATE <= $9) ORDER BY session.starts_at DESC, session.reference DESC LIMIT $10 OFFSET $11"
        );
        let rows = sqlx::query_as::<_, SessionRow>(&list_sql)
            .bind(tenant_id)
            .bind(visibility.campus)
            .bind(visibility.employee_id)
            .bind(&visibility.learner_ids)
            .bind(search.as_deref())
            .bind(query.group_id)
            .bind(query.status.map(|value| value.as_str()))
            .bind(query.date_from)
            .bind(query.date_to)
            .bind(per_page)
            .bind(offset)
            .fetch_all(pool)
            .await
            .context("Failed to list Activities sessions")?;
        let count_sql = format!(
            "SELECT COUNT(*) FROM ({SESSION_PROJECTION} AND ($5::TEXT IS NULL OR session.reference ILIKE $5 OR session.title ILIKE $5 OR activity_group.name ILIKE $5) AND ($6::UUID IS NULL OR session.group_id=$6) AND ($7::TEXT IS NULL OR session.status=$7) AND ($8::DATE IS NULL OR session.starts_at::DATE >= $8) AND ($9::DATE IS NULL OR session.starts_at::DATE <= $9)) AS visible_session"
        );
        let total = sqlx::query_scalar::<_, i64>(&count_sql)
            .bind(tenant_id)
            .bind(visibility.campus)
            .bind(visibility.employee_id)
            .bind(&visibility.learner_ids)
            .bind(search.as_deref())
            .bind(query.group_id)
            .bind(query.status.map(|value| value.as_str()))
            .bind(query.date_from)
            .bind(query.date_to)
            .fetch_one(pool)
            .await
            .context("Failed to count Activities sessions")?;
        Ok((rows.into_iter().map(session_summary).collect(), total))
    }

    pub async fn get_session(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: ActivitiesScope,
        session_id: Uuid,
    ) -> Result<Option<ActivitySessionRecord>> {
        let visibility = resolve_visibility(pool, tenant_id, scope).await?;
        let by_id_sql = format!("{SESSION_PROJECTION} AND session.id=$5");
        let row = sqlx::query_as::<_, SessionRow>(&by_id_sql)
            .bind(tenant_id)
            .bind(visibility.campus)
            .bind(visibility.employee_id)
            .bind(&visibility.learner_ids)
            .bind(session_id)
            .fetch_optional(pool)
            .await
            .context("Failed to read the Activities session")?;
        let Some(row) = row else {
            return Ok(None);
        };
        let can_read_roster =
            can_read_group_roster(pool, tenant_id, &visibility, row.group_id).await?;
        let mut participation = session_participation(pool, tenant_id, &row).await?;
        if !can_read_roster {
            participation.retain(|item| visibility.learner_ids.contains(&item.learner_id));
        }
        let history = if can_read_roster {
            event_history(pool, tenant_id, None, Some(session_id)).await?
        } else {
            Vec::new()
        };
        Ok(Some(ActivitySessionRecord {
            notes: row.notes.clone(),
            completion_summary: row.completion_summary.clone(),
            cancellation_reason: row.cancellation_reason.clone(),
            session: session_summary(row),
            participation,
            history,
        }))
    }

    pub async fn create_session(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: ActivitiesScope,
        actor: AuditActor,
        context: RequestContext,
        request: &CreateActivitySessionRequest,
    ) -> Result<ActivitySessionRecord> {
        validate_session_times(request.starts_at, request.ends_at)?;
        let actor_id = person_actor_id(actor)?;
        let visibility = resolve_visibility(pool, tenant_id, scope).await?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Activities session creation")?;
        let group = lock_group(
            &mut transaction,
            tenant_id,
            request.group_id,
            Some(&visibility),
        )
        .await?
        .ok_or_else(|| anyhow!("The Activities group was not found in your current scope"))?;
        if group.status != "active" {
            bail!("Sessions can only be created for an active Activities group");
        }
        validate_session_inside_group(
            request.starts_at,
            request.ends_at,
            group.starts_on,
            group.ends_on,
        )?;
        let reference = reserve_session_reference(&mut transaction, tenant_id).await?;
        let session_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO activity_sessions (
                   id, tenant_id, group_id, reference, title, starts_at, ends_at,
                   location_note, notes, created_by, updated_by
               ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$10)"#,
        )
        .bind(session_id)
        .bind(tenant_id)
        .bind(request.group_id)
        .bind(&reference)
        .bind(trimmed_required(&request.title, "Session title")?)
        .bind(request.starts_at)
        .bind(request.ends_at)
        .bind(trimmed_optional(request.location_note.as_deref()))
        .bind(trimmed_optional(request.notes.as_deref()))
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to create the Activities session")?;
        append_activity_evidence(
            &mut transaction,
            ActivityEvidence {
                tenant_id,
                group_id: Some(request.group_id),
                session_id: Some(session_id),
                actor,
                context,
                event_type: "activities.session.created",
                operation: "activities.sessions.create",
                target_kind: "activity_session",
                target_id: session_id,
                metadata: json!({"reference": reference, "starts_at": request.starts_at}),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Activities session creation")?;
        Self::get_session(pool, tenant_id, ActivitiesScope::Campus, session_id)
            .await?
            .ok_or_else(|| anyhow!("The Activities session could not be reloaded"))
    }

    pub async fn update_session(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: ActivitiesScope,
        session_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &UpdateActivitySessionRequest,
    ) -> Result<Option<ActivitySessionRecord>> {
        validate_session_times(request.starts_at, request.ends_at)?;
        let actor_id = person_actor_id(actor)?;
        let visibility = resolve_visibility(pool, tenant_id, scope).await?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Activities session update")?;
        let Some(session) =
            lock_session(&mut transaction, tenant_id, session_id, &visibility).await?
        else {
            return Ok(None);
        };
        ensure_version(
            session.version,
            request.expected_version,
            "Activities session",
        )?;
        if session.status != "scheduled" {
            bail!("Only a scheduled Activities session can be changed");
        }
        let group = lock_group(
            &mut transaction,
            tenant_id,
            session.group_id,
            Some(&visibility),
        )
        .await?
        .ok_or_else(|| anyhow!("The Activities group was not found in your current scope"))?;
        if group.status != "active" {
            bail!("The Activities group is no longer active");
        }
        validate_session_inside_group(
            request.starts_at,
            request.ends_at,
            group.starts_on,
            group.ends_on,
        )?;
        sqlx::query("UPDATE activity_sessions SET title=$1, starts_at=$2, ends_at=$3, location_note=$4, notes=$5, updated_by=$6, version=version+1, updated_at=NOW() WHERE tenant_id=$7 AND id=$8 AND deleted_at IS NULL")
            .bind(trimmed_required(&request.title, "Session title")?).bind(request.starts_at).bind(request.ends_at)
            .bind(trimmed_optional(request.location_note.as_deref())).bind(trimmed_optional(request.notes.as_deref()))
            .bind(actor_id).bind(tenant_id).bind(session_id).execute(&mut *transaction).await
            .context("Failed to update the Activities session")?;
        append_activity_evidence(
            &mut transaction,
            ActivityEvidence {
                tenant_id,
                group_id: Some(session.group_id),
                session_id: Some(session_id),
                actor,
                context,
                event_type: "activities.session.updated",
                operation: "activities.sessions.update",
                target_kind: "activity_session",
                target_id: session_id,
                metadata: json!({"expected_version": request.expected_version}),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Activities session update")?;
        Self::get_session(pool, tenant_id, ActivitiesScope::Campus, session_id).await
    }

    pub async fn mark_participation(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: ActivitiesScope,
        session_id: Uuid,
        membership_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &MarkActivityParticipationRequest,
    ) -> Result<Option<ActivitySessionRecord>> {
        let actor_id = person_actor_id(actor)?;
        let visibility = resolve_visibility(pool, tenant_id, scope).await?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Activities participation update")?;
        let Some(session) =
            lock_session(&mut transaction, tenant_id, session_id, &visibility).await?
        else {
            return Ok(None);
        };
        if session.status != "scheduled" {
            bail!("Participation can only be marked on a scheduled session");
        }
        let membership = sqlx::query_as::<_, (Uuid, Uuid)>(
            r#"SELECT id, learner_id FROM activity_group_memberships
                WHERE tenant_id=$1 AND id=$2 AND group_id=$3 AND joined_on <= $4::DATE
                  AND (ended_on IS NULL OR ended_on >= $4::DATE) AND deleted_at IS NULL FOR UPDATE"#,
        ).bind(tenant_id).bind(membership_id).bind(session.group_id).bind(session.starts_at)
            .fetch_optional(&mut *transaction).await.context("Failed to lock the Activities membership")?
            .ok_or_else(|| anyhow!("The learner is not on this session's effective roster"))?;
        let current = sqlx::query_as::<_, (Uuid, i32)>(
            "SELECT id, version FROM activity_session_participation WHERE tenant_id=$1 AND session_id=$2 AND membership_id=$3 AND deleted_at IS NULL FOR UPDATE",
        ).bind(tenant_id).bind(session_id).bind(membership_id).fetch_optional(&mut *transaction).await
            .context("Failed to lock Activities participation")?;
        match (current, request.expected_version) {
            (None, None) => {
                sqlx::query("INSERT INTO activity_session_participation (tenant_id, session_id, group_id, membership_id, learner_id, mark, notes, marked_by) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)")
                    .bind(tenant_id).bind(session_id).bind(session.group_id).bind(membership_id).bind(membership.1)
                    .bind(request.mark.as_str()).bind(trimmed_optional(request.notes.as_deref())).bind(actor_id)
                    .execute(&mut *transaction).await.context("Failed to mark Activities participation")?;
            }
            (Some((id, version)), Some(expected)) => {
                ensure_version(version, expected, "Activities participation")?;
                sqlx::query("UPDATE activity_session_participation SET mark=$1, notes=$2, marked_by=$3, marked_at=NOW(), version=version+1, updated_at=NOW() WHERE tenant_id=$4 AND id=$5 AND deleted_at IS NULL")
                    .bind(request.mark.as_str()).bind(trimmed_optional(request.notes.as_deref())).bind(actor_id)
                    .bind(tenant_id).bind(id).execute(&mut *transaction).await.context("Failed to update Activities participation")?;
            }
            (None, Some(_)) => {
                bail!("The participation record no longer exists; refresh and try again")
            }
            (Some(_), None) => {
                bail!("The participation record already exists; refresh and try again")
            }
        }
        append_activity_evidence(
            &mut transaction,
            ActivityEvidence {
                tenant_id,
                group_id: Some(session.group_id),
                session_id: Some(session_id),
                actor,
                context,
                event_type: "activities.session.participation_marked",
                operation: "activities.sessions.participation.mark",
                target_kind: "activity_session",
                target_id: session_id,
                metadata: json!({"membership_id": membership_id, "mark": request.mark.as_str()}),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Activities participation update")?;
        Self::get_session(pool, tenant_id, ActivitiesScope::Campus, session_id).await
    }

    pub async fn complete_session(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: ActivitiesScope,
        session_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &CompleteActivitySessionRequest,
    ) -> Result<Option<ActivitySessionRecord>> {
        let actor_id = person_actor_id(actor)?;
        let summary = trimmed_required(&request.summary, "Completion summary")?;
        let visibility = resolve_visibility(pool, tenant_id, scope).await?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Activities session completion")?;
        let Some(session) =
            lock_session(&mut transaction, tenant_id, session_id, &visibility).await?
        else {
            return Ok(None);
        };
        ensure_version(
            session.version,
            request.expected_version,
            "Activities session",
        )?;
        if session.status != "scheduled" {
            bail!("Only a scheduled Activities session can be completed");
        }
        let roster = sqlx::query_as::<_, (Uuid, Uuid, String, String, Option<String>, Option<String>)>(
            r#"SELECT membership.id, membership.learner_id, learner.learner_number,
                      learner.display_name, participation.mark, participation.notes
                 FROM activity_group_memberships AS membership
                 JOIN learners AS learner ON learner.id=membership.learner_id AND learner.tenant_id=membership.tenant_id
                 LEFT JOIN activity_session_participation AS participation
                   ON participation.tenant_id=membership.tenant_id AND participation.session_id=$3
                  AND participation.membership_id=membership.id AND participation.deleted_at IS NULL
                WHERE membership.tenant_id=$1 AND membership.group_id=$2
                  AND membership.joined_on <= $4::DATE
                  AND (membership.ended_on IS NULL OR membership.ended_on >= $4::DATE)
                  AND membership.deleted_at IS NULL
                ORDER BY membership.id FOR UPDATE OF membership"#,
        ).bind(tenant_id).bind(session.group_id).bind(session_id).bind(session.starts_at)
            .fetch_all(&mut *transaction).await.context("Failed to lock the Activities session roster")?;
        if roster.is_empty() {
            bail!("A session cannot be completed without an effective learner roster");
        }
        if roster.iter().any(|entry| entry.4.is_none()) {
            bail!("Mark every learner on the effective roster before completing the session");
        }
        let fingerprint_input = roster
            .iter()
            .map(|entry| {
                format!(
                    "{}:{}:{}",
                    entry.0,
                    entry.1,
                    entry.4.as_deref().unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("|");
        let fingerprint = format!("{:x}", Sha256::digest(fingerprint_input.as_bytes()));
        let snapshot_id = Uuid::new_v4();
        sqlx::query("INSERT INTO activity_session_completion_snapshots (id, tenant_id, session_id, group_id, roster_count, roster_fingerprint, summary, completed_by) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)")
            .bind(snapshot_id).bind(tenant_id).bind(session_id).bind(session.group_id)
            .bind(i32::try_from(roster.len()).context("Activities roster is too large")?)
            .bind(&fingerprint).bind(summary).bind(actor_id)
            .execute(&mut *transaction).await.context("Failed to create the Activities completion snapshot")?;
        for (membership_id, learner_id, learner_number, learner_name, mark, notes) in &roster {
            let mark = mark
                .as_deref()
                .ok_or_else(|| anyhow!("Every roster member requires a participation mark"))?;
            sqlx::query("INSERT INTO activity_session_completion_members (tenant_id, snapshot_id, session_id, group_id, membership_id, learner_id, learner_number_snapshot, learner_name_snapshot, mark, notes) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
                .bind(tenant_id).bind(snapshot_id).bind(session_id).bind(session.group_id)
                .bind(membership_id).bind(learner_id).bind(learner_number).bind(learner_name).bind(mark).bind(notes)
                .execute(&mut *transaction).await.context("Failed to snapshot Activities participation")?;
        }
        sqlx::query("UPDATE activity_sessions SET status='completed', completed_at=NOW(), completed_by=$1, completion_summary=$2, updated_by=$1, version=version+1, updated_at=NOW() WHERE tenant_id=$3 AND id=$4 AND deleted_at IS NULL")
            .bind(actor_id).bind(summary).bind(tenant_id).bind(session_id).execute(&mut *transaction).await
            .context("Failed to complete the Activities session")?;
        append_activity_evidence(&mut transaction, ActivityEvidence {
            tenant_id, group_id: Some(session.group_id), session_id: Some(session_id), actor, context,
            event_type: "activities.session.completed", operation: "activities.sessions.complete",
            target_kind: "activity_session", target_id: session_id,
            metadata: json!({"reference": session.reference, "roster_count": roster.len(), "roster_fingerprint": fingerprint}),
        }).await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Activities session completion")?;
        Self::get_session(pool, tenant_id, ActivitiesScope::Campus, session_id).await
    }

    pub async fn cancel_session(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: ActivitiesScope,
        session_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &CancelActivitySessionRequest,
    ) -> Result<Option<ActivitySessionRecord>> {
        let actor_id = person_actor_id(actor)?;
        let reason = trimmed_required(&request.reason, "Cancellation reason")?;
        let visibility = resolve_visibility(pool, tenant_id, scope).await?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Activities session cancellation")?;
        let Some(session) =
            lock_session(&mut transaction, tenant_id, session_id, &visibility).await?
        else {
            return Ok(None);
        };
        ensure_version(
            session.version,
            request.expected_version,
            "Activities session",
        )?;
        if session.status != "scheduled" {
            bail!("Only a scheduled Activities session can be cancelled");
        }
        sqlx::query("UPDATE activity_sessions SET status='cancelled', cancelled_at=NOW(), cancelled_by=$1, cancellation_reason=$2, updated_by=$1, version=version+1, updated_at=NOW() WHERE tenant_id=$3 AND id=$4 AND deleted_at IS NULL")
            .bind(actor_id).bind(reason).bind(tenant_id).bind(session_id).execute(&mut *transaction).await
            .context("Failed to cancel the Activities session")?;
        append_activity_evidence(
            &mut transaction,
            ActivityEvidence {
                tenant_id,
                group_id: Some(session.group_id),
                session_id: Some(session_id),
                actor,
                context,
                event_type: "activities.session.cancelled",
                operation: "activities.sessions.cancel",
                target_kind: "activity_session",
                target_id: session_id,
                metadata: json!({"reference": session.reference, "reason": reason}),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Activities session cancellation")?;
        Self::get_session(pool, tenant_id, ActivitiesScope::Campus, session_id).await
    }
}

#[derive(Debug)]
struct ActivityVisibility {
    campus: bool,
    employee_id: Option<Uuid>,
    learner_ids: Vec<Uuid>,
}

async fn resolve_visibility(
    pool: &PgPool,
    tenant_id: Uuid,
    scope: ActivitiesScope,
) -> Result<ActivityVisibility> {
    if scope.is_denied() {
        bail!("Activities records are outside your current scope");
    }
    if scope.is_campus() {
        return Ok(ActivityVisibility {
            campus: true,
            employee_id: None,
            learner_ids: Vec::new(),
        });
    }
    let account_id = scope
        .account_id()
        .ok_or_else(|| anyhow!("Activities account scope is unavailable"))?;
    let employee_id = if scope.includes_assigned() {
        Some(
            EmployeeOps::active_reference_by_account(pool, tenant_id, account_id)
                .await?
                .ok_or_else(|| {
                    anyhow!("Activity Leader access requires an active linked employee")
                })?
                .id,
        )
    } else {
        None
    };
    let learner_ids = if scope.includes_self() {
        EnrolmentOps::learner_ids_for_account(pool, tenant_id, account_id).await?
    } else {
        Vec::new()
    };
    Ok(ActivityVisibility {
        campus: false,
        employee_id,
        learner_ids,
    })
}

async fn group_leaders(
    pool: &PgPool,
    tenant_id: Uuid,
    group_id: Uuid,
) -> Result<Vec<ActivityLeaderResponse>> {
    let rows = sqlx::query_as::<_, LeaderRow>(
        r#"SELECT id, employee_id, leader_role, starts_on, ends_on, ended_at, end_reason, version
             FROM activity_group_leaders WHERE tenant_id=$1 AND group_id=$2 AND deleted_at IS NULL
            ORDER BY ended_at NULLS FIRST, leader_role, starts_on, id"#,
    )
    .bind(tenant_id)
    .bind(group_id)
    .fetch_all(pool)
    .await
    .context("Failed to list activity leaders")?;
    let ids = rows.iter().map(|row| row.employee_id).collect::<Vec<_>>();
    let references = EmployeeOps::references_by_ids(pool, tenant_id, &ids).await?;
    let by_id = references
        .into_iter()
        .map(|item| (item.id, item))
        .collect::<HashMap<_, _>>();
    rows.into_iter()
        .map(|row| {
            let employee = by_id
                .get(&row.employee_id)
                .ok_or_else(|| anyhow!("An activity leader no longer has an HR record"))?;
            Ok(ActivityLeaderResponse {
                id: row.id,
                employee_id: row.employee_id,
                employee_number: employee.employee_number.clone(),
                employee_name: employee.display_name.clone(),
                role: row.leader_role,
                starts_on: row.starts_on,
                ends_on: row.ends_on,
                ended_at: row.ended_at,
                end_reason: row.end_reason,
                version: row.version,
            })
        })
        .collect()
}

async fn group_memberships(
    pool: &PgPool,
    tenant_id: Uuid,
    group_id: Uuid,
    learner_ids: Option<&[Uuid]>,
) -> Result<Vec<ActivityMembershipResponse>> {
    let rows = sqlx::query_as::<_, MembershipRow>(
        r#"SELECT id, learner_id, joined_on, ended_on, status, consent_status,
                  consent_recorded_at, consent_notes, version
             FROM activity_group_memberships
            WHERE tenant_id=$1 AND group_id=$2 AND deleted_at IS NULL
              AND ($3::UUID[] IS NULL OR learner_id=ANY($3))
            ORDER BY status='active' DESC, joined_on, id"#,
    )
    .bind(tenant_id)
    .bind(group_id)
    .bind(learner_ids)
    .fetch_all(pool)
    .await
    .context("Failed to list activity memberships")?;
    let ids = rows.iter().map(|row| row.learner_id).collect::<Vec<_>>();
    let references = LearnerOps::activity_references_by_ids(pool, tenant_id, &ids).await?;
    let by_id = references
        .into_iter()
        .map(|item| (item.id, item))
        .collect::<HashMap<_, _>>();
    rows.into_iter()
        .map(|row| {
            let learner = by_id
                .get(&row.learner_id)
                .ok_or_else(|| anyhow!("An activity member no longer has an SIS record"))?;
            Ok(ActivityMembershipResponse {
                id: row.id,
                learner_id: row.learner_id,
                learner_number: learner.learner_number.clone(),
                learner_name: learner.display_name.clone(),
                joined_on: row.joined_on,
                ended_on: row.ended_on,
                status: row.status,
                consent_status: row.consent_status,
                consent_recorded_at: row.consent_recorded_at,
                consent_notes: row.consent_notes,
                version: row.version,
            })
        })
        .collect()
}

async fn can_read_group_roster(
    pool: &PgPool,
    tenant_id: Uuid,
    visibility: &ActivityVisibility,
    group_id: Uuid,
) -> Result<bool> {
    if visibility.campus {
        return Ok(true);
    }
    let Some(employee_id) = visibility.employee_id else {
        return Ok(false);
    };
    sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(
               SELECT 1 FROM activity_group_leaders
                WHERE tenant_id=$1 AND group_id=$2 AND employee_id=$3
                  AND ended_at IS NULL AND deleted_at IS NULL
           )"#,
    )
    .bind(tenant_id)
    .bind(group_id)
    .bind(employee_id)
    .fetch_one(pool)
    .await
    .context("Failed to check activity roster access")
}

async fn event_history(
    pool: &PgPool,
    tenant_id: Uuid,
    group_id: Option<Uuid>,
    session_id: Option<Uuid>,
) -> Result<Vec<ActivityLifecycleEventResponse>> {
    sqlx::query_as::<_, EventRow>(
        r#"SELECT event.id, event.event_type, account.full_name AS actor_name,
                  event.metadata, event.created_at
             FROM activity_lifecycle_events AS event
             JOIN users AS account ON account.id=event.actor_id AND account.tenant_id=event.tenant_id
            WHERE event.tenant_id=$1
              AND ($2::UUID IS NULL OR event.group_id=$2)
              AND ($3::UUID IS NULL OR event.session_id=$3)
            ORDER BY event.created_at DESC, event.id DESC LIMIT 200"#,
    ).bind(tenant_id).bind(group_id).bind(session_id).fetch_all(pool).await
        .context("Failed to load Activities lifecycle history")
        .map(|rows| rows.into_iter().map(|row| ActivityLifecycleEventResponse {
            id: row.id, event_type: row.event_type, actor_name: row.actor_name,
            metadata: row.metadata, created_at: row.created_at,
        }).collect())
}

async fn lock_group(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    group_id: Uuid,
    visibility: Option<&ActivityVisibility>,
) -> Result<Option<LockedGroup>> {
    let (campus, employee_id, learner_ids) = visibility.map_or((true, None, Vec::new()), |value| {
        (value.campus, value.employee_id, value.learner_ids.clone())
    });
    sqlx::query_as::<_, LockedGroup>(
        r#"SELECT activity_id, starts_on, ends_on, capacity, consent_required, status, version
             FROM activity_groups AS activity_group
            WHERE activity_group.tenant_id=$1 AND activity_group.id=$2 AND activity_group.deleted_at IS NULL
              AND ($3 OR ($4::UUID IS NOT NULL AND EXISTS (
                    SELECT 1 FROM activity_group_leaders AS leader
                     WHERE leader.tenant_id=activity_group.tenant_id AND leader.group_id=activity_group.id
                       AND leader.employee_id=$4 AND leader.ended_at IS NULL AND leader.deleted_at IS NULL
                  )) OR (COALESCE(array_length($5::UUID[],1),0)>0 AND EXISTS (
                    SELECT 1 FROM activity_group_memberships AS membership
                     WHERE membership.tenant_id=activity_group.tenant_id AND membership.group_id=activity_group.id
                       AND membership.learner_id=ANY($5) AND membership.status='active' AND membership.deleted_at IS NULL
                  )))
            FOR UPDATE"#,
    ).bind(tenant_id).bind(group_id).bind(campus).bind(employee_id).bind(learner_ids)
        .fetch_optional(&mut **transaction).await.context("Failed to lock the Activities group")
}

async fn lock_session(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    session_id: Uuid,
    visibility: &ActivityVisibility,
) -> Result<Option<LockedSession>> {
    sqlx::query_as::<_, LockedSession>(
        r#"SELECT session.group_id, session.reference, session.starts_at, session.status, session.version
             FROM activity_sessions AS session
             JOIN activity_groups AS activity_group
               ON activity_group.id=session.group_id AND activity_group.tenant_id=session.tenant_id
            WHERE session.tenant_id=$1 AND session.id=$2 AND session.deleted_at IS NULL
              AND ($3 OR ($4::UUID IS NOT NULL AND EXISTS (
                    SELECT 1 FROM activity_group_leaders AS leader
                     WHERE leader.tenant_id=activity_group.tenant_id AND leader.group_id=activity_group.id
                       AND leader.employee_id=$4 AND leader.ended_at IS NULL AND leader.deleted_at IS NULL
                  )) OR (COALESCE(array_length($5::UUID[],1),0)>0 AND EXISTS (
                    SELECT 1 FROM activity_group_memberships AS membership
                     WHERE membership.tenant_id=activity_group.tenant_id AND membership.group_id=activity_group.id
                       AND membership.learner_id=ANY($5) AND membership.status='active' AND membership.deleted_at IS NULL
                  )))
            FOR UPDATE OF session"#,
    )
    .bind(tenant_id)
    .bind(session_id)
    .bind(visibility.campus)
    .bind(visibility.employee_id)
    .bind(&visibility.learner_ids)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock the Activities session")
}

async fn session_participation(
    pool: &PgPool,
    tenant_id: Uuid,
    session: &SessionRow,
) -> Result<Vec<ActivityParticipationResponse>> {
    let rows = if session.status == "completed" {
        sqlx::query_as::<_, ParticipationRow>(
            r#"SELECT member.membership_id, member.learner_id,
                      member.learner_number_snapshot AS learner_number,
                      member.learner_name_snapshot AS learner_name,
                      member.mark AS mark, member.notes, NULL::INTEGER AS version,
                      snapshot.completed_at AS marked_at
                 FROM activity_session_completion_members AS member
                 JOIN activity_session_completion_snapshots AS snapshot
                   ON snapshot.id=member.snapshot_id AND snapshot.tenant_id=member.tenant_id
                WHERE member.tenant_id=$1 AND member.session_id=$2 AND member.deleted_at IS NULL
                ORDER BY member.learner_name_snapshot, member.learner_number_snapshot"#,
        )
        .bind(tenant_id)
        .bind(session.id)
        .fetch_all(pool)
        .await
        .context("Failed to read completed Activities participation")?
    } else {
        sqlx::query_as::<_, ParticipationRow>(
            r#"SELECT membership.id AS membership_id, membership.learner_id,
                      learner.learner_number, learner.display_name AS learner_name,
                      participation.mark, participation.notes, participation.version,
                      participation.marked_at
                 FROM activity_group_memberships AS membership
                 JOIN learners AS learner
                   ON learner.id=membership.learner_id AND learner.tenant_id=membership.tenant_id
                 LEFT JOIN activity_session_participation AS participation
                   ON participation.tenant_id=membership.tenant_id
                  AND participation.session_id=$2 AND participation.membership_id=membership.id
                  AND participation.deleted_at IS NULL
                WHERE membership.tenant_id=$1 AND membership.group_id=$3
                  AND membership.joined_on <= $4::DATE
                  AND (membership.ended_on IS NULL OR membership.ended_on >= $4::DATE)
                  AND membership.deleted_at IS NULL
                ORDER BY learner.display_name, learner.learner_number"#,
        )
        .bind(tenant_id)
        .bind(session.id)
        .bind(session.group_id)
        .bind(session.starts_at)
        .fetch_all(pool)
        .await
        .context("Failed to read Activities participation")?
    };
    Ok(rows
        .into_iter()
        .map(|row| ActivityParticipationResponse {
            membership_id: row.membership_id,
            learner_id: row.learner_id,
            learner_number: row.learner_number,
            learner_name: row.learner_name,
            mark: row.mark,
            notes: row.notes,
            version: row.version,
            marked_at: row.marked_at,
        })
        .collect())
}

async fn reserve_session_reference(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
) -> Result<String> {
    let (prefix, sequence, padding) = sqlx::query_as::<_, (String, i64, i32)>(
        r#"UPDATE activity_session_numbering_policies
               SET next_sequence=next_sequence+1, updated_at=NOW()
             WHERE tenant_id=$1 AND deleted_at IS NULL
         RETURNING prefix, next_sequence-1, padding"#,
    )
    .bind(tenant_id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to reserve an Activities session number")?;
    Ok(format!(
        "{prefix}{sequence:0width$}",
        width = padding as usize
    ))
}

async fn ensure_active_catalog(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    activity_id: Uuid,
) -> Result<()> {
    let active = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM activity_catalog_items WHERE tenant_id=$1 AND id=$2 AND status='active' AND deleted_at IS NULL)")
        .bind(tenant_id).bind(activity_id).fetch_one(&mut **transaction).await.context("Failed to check the activity catalog item")?;
    if !active {
        bail!("The selected activity is not active");
    }
    Ok(())
}

async fn ensure_roster_inside_dates(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    group_id: Uuid,
    starts_on: NaiveDate,
    ends_on: NaiveDate,
) -> Result<()> {
    let outside = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(
               SELECT 1 FROM activity_group_leaders WHERE tenant_id=$1 AND group_id=$2 AND deleted_at IS NULL
                AND (starts_on < $3 OR COALESCE(ends_on, $4) > $4)
               UNION ALL
               SELECT 1 FROM activity_group_memberships WHERE tenant_id=$1 AND group_id=$2 AND deleted_at IS NULL
                AND (joined_on < $3 OR COALESCE(ended_on, $4) > $4)
               UNION ALL
               SELECT 1 FROM activity_sessions WHERE tenant_id=$1 AND group_id=$2 AND deleted_at IS NULL
                AND (starts_at::DATE < $3 OR ends_at::DATE > $4)
            )"#,
    ).bind(tenant_id).bind(group_id).bind(starts_on).bind(ends_on).fetch_one(&mut **transaction).await
        .context("Failed to validate activity group dates")?;
    if outside {
        bail!("The new dates exclude an existing leader, member, or session");
    }
    Ok(())
}

async fn active_leader_count(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    group_id: Uuid,
) -> Result<i64> {
    sqlx::query_scalar("SELECT COUNT(*) FROM activity_group_leaders WHERE tenant_id=$1 AND group_id=$2 AND ended_at IS NULL AND deleted_at IS NULL")
        .bind(tenant_id).bind(group_id).fetch_one(&mut **transaction).await.context("Failed to count activity leaders")
}

async fn active_member_count(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    group_id: Uuid,
) -> Result<i64> {
    sqlx::query_scalar("SELECT COUNT(*) FROM activity_group_memberships WHERE tenant_id=$1 AND group_id=$2 AND status='active' AND deleted_at IS NULL")
        .bind(tenant_id).bind(group_id).fetch_one(&mut **transaction).await.context("Failed to count activity members")
}

struct ActivityEvidence<'a> {
    tenant_id: Uuid,
    group_id: Option<Uuid>,
    session_id: Option<Uuid>,
    actor: AuditActor,
    context: RequestContext,
    event_type: &'a str,
    operation: &'a str,
    target_kind: &'a str,
    target_id: Uuid,
    metadata: Value,
}

async fn append_activity_evidence(
    transaction: &mut Transaction<'_, Postgres>,
    evidence: ActivityEvidence<'_>,
) -> Result<()> {
    let actor_id = person_actor_id(evidence.actor)?;
    sqlx::query("INSERT INTO activity_lifecycle_events (tenant_id, group_id, session_id, event_type, actor_id, metadata) VALUES ($1,$2,$3,$4,$5,$6)")
        .bind(evidence.tenant_id).bind(evidence.group_id).bind(evidence.session_id)
        .bind(evidence.event_type).bind(actor_id).bind(evidence.metadata.clone())
        .execute(&mut **transaction).await.context("Failed to append Activities lifecycle evidence")?;
    append_domain_audit(
        transaction,
        evidence.tenant_id,
        evidence.actor,
        evidence.context,
        evidence.operation,
        evidence.target_kind,
        evidence.target_id,
        evidence.metadata,
    )
    .await
}

#[expect(
    clippy::too_many_arguments,
    reason = "audit writes keep complete domain target and request evidence explicit"
)]
async fn append_domain_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    context: RequestContext,
    operation: &str,
    target_kind: &str,
    target_id: Uuid,
    metadata: Value,
) -> Result<()> {
    append_audit(
        &mut **transaction,
        &NewAuditEvent::new(
            tenant_id,
            actor,
            operation,
            AuditOutcome::Succeeded,
            context,
        )
        .with_target(AuditTarget::new(target_kind, target_id.to_string()))
        .with_redacted_metadata(metadata.as_object().cloned().unwrap_or_else(Map::new)),
    )
    .await
    .map(|_| ())
    .context("Failed to append Activities audit evidence")
}

fn catalog_response(row: CatalogRow) -> ActivityCatalogItemResponse {
    ActivityCatalogItemResponse {
        id: row.id,
        code: row.code,
        name: row.name,
        category: row.category,
        description: row.description,
        status: row.status,
        version: row.version,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn group_summary(row: GroupRow) -> ActivityGroupSummary {
    ActivityGroupSummary {
        id: row.id,
        activity_id: row.activity_id,
        activity_code: row.activity_code,
        activity_name: row.activity_name,
        code: row.code,
        name: row.name,
        starts_on: row.starts_on,
        ends_on: row.ends_on,
        capacity: row.capacity,
        consent_required: row.consent_required,
        status: row.status,
        leader_count: row.leader_count,
        member_count: row.member_count,
        session_count: row.session_count,
        version: row.version,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn session_summary(row: SessionRow) -> ActivitySessionSummary {
    ActivitySessionSummary {
        id: row.id,
        reference: row.reference,
        group_id: row.group_id,
        group_code: row.group_code,
        group_name: row.group_name,
        title: row.title,
        starts_at: row.starts_at,
        ends_at: row.ends_at,
        location_note: row.location_note,
        status: row.status,
        roster_count: row.roster_count,
        marked_count: row.marked_count,
        present_count: row.present_count,
        absent_count: row.absent_count,
        version: row.version,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn validate_group_dates(starts_on: NaiveDate, ends_on: NaiveDate) -> Result<()> {
    if starts_on > ends_on {
        bail!("The group start date must not be after its end date");
    }
    Ok(())
}

fn validate_consent(required: bool, instructions: Option<&str>) -> Result<()> {
    if !required && trimmed_optional(instructions).is_some() {
        bail!("Consent instructions require consent to be enabled");
    }
    Ok(())
}

fn validate_effective_dates(
    starts_on: NaiveDate,
    ends_on: Option<NaiveDate>,
    group_starts_on: NaiveDate,
    group_ends_on: NaiveDate,
    label: &str,
) -> Result<()> {
    if starts_on < group_starts_on || starts_on > group_ends_on {
        bail!("{label} must start within the group dates");
    }
    if let Some(ends_on) = ends_on {
        if ends_on < starts_on || ends_on > group_ends_on {
            bail!("{label} must end on or after its start and within the group dates");
        }
    }
    Ok(())
}

fn validate_session_times(starts_at: DateTime<Utc>, ends_at: DateTime<Utc>) -> Result<()> {
    if starts_at >= ends_at {
        bail!("The session end time must be after its start time");
    }
    Ok(())
}

fn validate_session_inside_group(
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
    group_starts_on: NaiveDate,
    group_ends_on: NaiveDate,
) -> Result<()> {
    if starts_at.date_naive() < group_starts_on || ends_at.date_naive() > group_ends_on {
        bail!("The session must take place within the group dates");
    }
    Ok(())
}

fn ensure_version(actual: i32, expected: i32, label: &str) -> Result<()> {
    if actual != expected {
        bail!("{label} changed after it was loaded; refresh and try again");
    }
    Ok(())
}

fn required_reason<'a>(value: Option<&'a str>, label: &str) -> Result<&'a str> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("{label} is required"))
}

fn person_actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("Activities changes require an authenticated person"))
}

fn trimmed_required<'a>(value: &'a str, label: &str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{label} is required");
    }
    Ok(value)
}

fn trimmed_optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn search_pattern(value: Option<&str>) -> Option<String> {
    trimmed_optional(value).map(|value| format!("%{value}%"))
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).max(1),
        per_page.unwrap_or(20).clamp(1, 100),
    )
}

fn database_error(error: sqlx::Error, duplicate_message: &str) -> anyhow::Error {
    if let sqlx::Error::Database(database) = &error {
        if database.code().as_deref() == Some("23505") {
            return anyhow!(duplicate_message.to_string());
        }
        if database.code().as_deref() == Some("23514") {
            return anyhow!("The Activities record violates its lifecycle rules");
        }
    }
    anyhow!(error).context("The Activities record could not be saved")
}

const CATALOG_LIST: &str = r#"
SELECT id, code, name, category, description, status, version, created_at, updated_at
  FROM activity_catalog_items
 WHERE tenant_id=$1 AND deleted_at IS NULL
   AND ($2::TEXT IS NULL OR code ILIKE $2 OR name ILIKE $2)
   AND ($3::TEXT IS NULL OR category=$3)
   AND ($4::TEXT IS NULL OR status=$4)
 ORDER BY status='active' DESC, name, code
"#;

const CATALOG_BY_ID: &str = r#"
SELECT id, code, name, category, description, status, version, created_at, updated_at
  FROM activity_catalog_items WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL
"#;

const GROUP_PROJECTION: &str = r#"
SELECT activity_group.id, activity_group.activity_id,
       activity.code AS activity_code, activity.name AS activity_name,
       activity_group.code, activity_group.name, activity_group.starts_on,
       activity_group.ends_on, activity_group.capacity, activity_group.consent_required,
       activity_group.consent_instructions, activity_group.status,
       (SELECT COUNT(*) FROM activity_group_leaders AS leader
         WHERE leader.tenant_id=activity_group.tenant_id AND leader.group_id=activity_group.id
           AND leader.ended_at IS NULL AND leader.deleted_at IS NULL) AS leader_count,
       (SELECT COUNT(*) FROM activity_group_memberships AS membership
         WHERE membership.tenant_id=activity_group.tenant_id AND membership.group_id=activity_group.id
           AND membership.status='active' AND membership.deleted_at IS NULL) AS member_count,
       (SELECT COUNT(*) FROM activity_sessions AS session
         WHERE session.tenant_id=activity_group.tenant_id AND session.group_id=activity_group.id
           AND session.deleted_at IS NULL) AS session_count,
       activity_group.version, activity_group.created_at, activity_group.updated_at
  FROM activity_groups AS activity_group
  JOIN activity_catalog_items AS activity
    ON activity.id=activity_group.activity_id AND activity.tenant_id=activity_group.tenant_id
 WHERE activity_group.tenant_id=$1 AND activity_group.deleted_at IS NULL
   AND ($2 OR ($3::UUID IS NOT NULL AND EXISTS (
         SELECT 1 FROM activity_group_leaders AS leader
          WHERE leader.tenant_id=activity_group.tenant_id AND leader.group_id=activity_group.id
            AND leader.employee_id=$3 AND leader.ended_at IS NULL AND leader.deleted_at IS NULL
       )) OR (COALESCE(array_length($4::UUID[],1),0)>0 AND EXISTS (
         SELECT 1 FROM activity_group_memberships AS membership
          WHERE membership.tenant_id=activity_group.tenant_id AND membership.group_id=activity_group.id
            AND membership.learner_id=ANY($4) AND membership.deleted_at IS NULL
       )))
"#;

const SESSION_PROJECTION: &str = r#"
SELECT session.id, session.reference, session.group_id,
       activity_group.code AS group_code, activity_group.name AS group_name,
       session.title, session.starts_at, session.ends_at, session.location_note,
       session.notes, session.status, session.completion_summary, session.cancellation_reason,
       CASE WHEN session.status='completed' THEN COALESCE((
            SELECT snapshot.roster_count::BIGINT FROM activity_session_completion_snapshots AS snapshot
             WHERE snapshot.tenant_id=session.tenant_id AND snapshot.session_id=session.id
               AND snapshot.deleted_at IS NULL
       ),0) ELSE (
            SELECT COUNT(*) FROM activity_group_memberships AS membership
             WHERE membership.tenant_id=session.tenant_id AND membership.group_id=session.group_id
               AND membership.joined_on <= session.starts_at::DATE
               AND (membership.ended_on IS NULL OR membership.ended_on >= session.starts_at::DATE)
               AND membership.deleted_at IS NULL
       ) END AS roster_count,
       CASE WHEN session.status='completed' THEN COALESCE((
            SELECT snapshot.roster_count::BIGINT FROM activity_session_completion_snapshots AS snapshot
             WHERE snapshot.tenant_id=session.tenant_id AND snapshot.session_id=session.id
               AND snapshot.deleted_at IS NULL
       ),0) ELSE (
            SELECT COUNT(*) FROM activity_session_participation AS participation
             WHERE participation.tenant_id=session.tenant_id AND participation.session_id=session.id
               AND participation.deleted_at IS NULL
       ) END AS marked_count,
       CASE WHEN session.status='completed' THEN (
            SELECT COUNT(*) FROM activity_session_completion_members AS member
             WHERE member.tenant_id=session.tenant_id AND member.session_id=session.id
               AND member.mark IN ('present','late') AND member.deleted_at IS NULL
       ) ELSE (
            SELECT COUNT(*) FROM activity_session_participation AS participation
             WHERE participation.tenant_id=session.tenant_id AND participation.session_id=session.id
               AND participation.mark IN ('present','late') AND participation.deleted_at IS NULL
       ) END AS present_count,
       CASE WHEN session.status='completed' THEN (
            SELECT COUNT(*) FROM activity_session_completion_members AS member
             WHERE member.tenant_id=session.tenant_id AND member.session_id=session.id
               AND member.mark='absent' AND member.deleted_at IS NULL
       ) ELSE (
            SELECT COUNT(*) FROM activity_session_participation AS participation
             WHERE participation.tenant_id=session.tenant_id AND participation.session_id=session.id
               AND participation.mark='absent' AND participation.deleted_at IS NULL
       ) END AS absent_count,
       session.version, session.created_at, session.updated_at
  FROM activity_sessions AS session
  JOIN activity_groups AS activity_group
    ON activity_group.id=session.group_id AND activity_group.tenant_id=session.tenant_id
 WHERE session.tenant_id=$1 AND session.deleted_at IS NULL
   AND ($2 OR ($3::UUID IS NOT NULL AND EXISTS (
         SELECT 1 FROM activity_group_leaders AS leader
          WHERE leader.tenant_id=activity_group.tenant_id AND leader.group_id=activity_group.id
            AND leader.employee_id=$3 AND leader.ended_at IS NULL AND leader.deleted_at IS NULL
       )) OR (COALESCE(array_length($4::UUID[],1),0)>0 AND EXISTS (
         SELECT 1 FROM activity_group_memberships AS membership
          WHERE membership.tenant_id=activity_group.tenant_id AND membership.group_id=activity_group.id
            AND membership.learner_id=ANY($4)
            AND membership.joined_on <= session.starts_at::DATE
            AND (membership.ended_on IS NULL OR membership.ended_on >= session.starts_at::DATE)
            AND membership.deleted_at IS NULL
       )))
"#;
