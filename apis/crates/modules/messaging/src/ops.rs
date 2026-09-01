//! Transactional Communication operations with frozen recipient snapshots.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, anyhow, bail};
use cp_academics::ops::CommunicationAudienceOps as AcademicAudienceOps;
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_hr_payroll::ops::CommunicationAudienceOps as HrAudienceOps;
use cp_sis::ops::CommunicationAudienceOps as SisAudienceOps;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::dtos::*;
use crate::models::{AnnouncementRow, AudienceTargetRow, LockedAnnouncement};

const DEFAULT_PAGE: i64 = 1;
const DEFAULT_PER_PAGE: i64 = 25;
const MAX_PER_PAGE: i64 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommunicationAccessScope {
    Campus,
    AssignedTo(Uuid),
    SelfFor(Uuid),
}

pub struct CommunicationOps;

impl CommunicationOps {
    pub async fn reference_data(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: CommunicationAccessScope,
    ) -> Result<CommunicationReferenceData> {
        let assigned = match scope {
            CommunicationAccessScope::AssignedTo(id) => Some(id),
            _ => None,
        };
        let classes = AcademicAudienceOps::class_references(pool, tenant_id, assigned).await?;
        if matches!(scope, CommunicationAccessScope::AssignedTo(_)) {
            return Ok(CommunicationReferenceData {
                classes,
                departments: Vec::new(),
                roles: Vec::new(),
                users: Vec::new(),
                campus_allowed: false,
            });
        }
        if !matches!(scope, CommunicationAccessScope::Campus) {
            return Ok(CommunicationReferenceData {
                classes: Vec::new(),
                departments: Vec::new(),
                roles: Vec::new(),
                users: Vec::new(),
                campus_allowed: false,
            });
        }
        let departments = HrAudienceOps::department_references(pool, tenant_id).await?;
        let roles = sqlx::query_as::<_, RoleReference>(
            "SELECT key, name FROM roles WHERE tenant_id = $1 AND deleted_at IS NULL ORDER BY name, key",
        ).bind(tenant_id).fetch_all(pool).await.context("Failed to list communication role references")?;
        let users = active_users(pool, tenant_id, None).await?;
        Ok(CommunicationReferenceData {
            classes,
            departments,
            roles,
            users,
            campus_allowed: true,
        })
    }

    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: CommunicationAccessScope,
        query: &AnnouncementListQuery,
    ) -> Result<(Vec<AnnouncementSummary>, i64)> {
        let (page, per_page) = bounded_page(query.page, query.per_page);
        let offset = (page - 1) * per_page;
        let creator = match scope {
            CommunicationAccessScope::AssignedTo(id) => Some(id),
            CommunicationAccessScope::SelfFor(_) => return Ok((Vec::new(), 0)),
            CommunicationAccessScope::Campus => None,
        };
        let status = query.status.map(AnnouncementStatus::as_str);
        let search = query
            .search
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(|v| format!("%{v}%"));
        let rows = sqlx::query_as::<_, AnnouncementRow>(&format!(
            "{} ORDER BY announcement.updated_at DESC LIMIT $6 OFFSET $7",
            announcement_select()
        ))
        .bind(tenant_id)
        .bind(creator)
        .bind(status)
        .bind(&search)
        .bind(Option::<Uuid>::None)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list announcements")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM communication_announcements AS announcement
                WHERE announcement.tenant_id = $1 AND announcement.deleted_at IS NULL
                  AND ($2::UUID IS NULL OR announcement.created_by = $2)
                  AND ($3::TEXT IS NULL OR announcement.status = $3)
                  AND ($4::TEXT IS NULL OR announcement.title ILIKE $4 OR announcement.body ILIKE $4)"#,
        ).bind(tenant_id).bind(creator).bind(status).bind(search).fetch_one(pool).await.context("Failed to count announcements")?;
        Ok((rows.into_iter().map(summary_from_row).collect(), total))
    }

    pub async fn get(
        pool: &PgPool,
        tenant_id: Uuid,
        announcement_id: Uuid,
        scope: CommunicationAccessScope,
    ) -> Result<Option<AnnouncementDetail>> {
        let row = sqlx::query_as::<_, AnnouncementRow>(announcement_select())
            .bind(tenant_id)
            .bind(Option::<Uuid>::None)
            .bind(Option::<&str>::None)
            .bind(Option::<String>::None)
            .bind(announcement_id)
            .fetch_optional(pool)
            .await
            .context("Failed to load announcement")?;
        let Some(row) = row else {
            return Ok(None);
        };
        if !scope_allows_announcement(scope, row.created_by) {
            return Ok(None);
        }
        let targets = load_targets(pool, tenant_id, announcement_id)
            .await?
            .into_iter()
            .map(target_from_row)
            .collect();
        Ok(Some(detail_from_row(row, targets)))
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: CommunicationAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateAnnouncementRequest,
    ) -> Result<AnnouncementDetail> {
        let actor_id = person_actor_id(actor)?;
        ensure_can_manage(scope, actor_id)?;
        let targets = validated_targets(pool, tenant_id, scope, &request.targets).await?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start announcement creation")?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"INSERT INTO communication_announcements
                (tenant_id, title, body, priority, created_by)
               VALUES ($1, $2, $3, $4, $5) RETURNING id"#,
        )
        .bind(tenant_id)
        .bind(required(&request.title, "Title")?)
        .bind(required(&request.body, "Message")?)
        .bind(request.priority.as_str())
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to create announcement")?;
        replace_targets(&mut transaction, tenant_id, id, &targets).await?;
        append_event(
            &mut transaction,
            tenant_id,
            id,
            "created",
            None,
            "draft",
            1,
            actor_id,
            None,
            json!({"target_count": targets.len()}),
        )
        .await?;
        append_communication_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "messaging.announcements.create",
            id,
            json!({"target_count": targets.len()}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit announcement creation")?;
        Self::get(pool, tenant_id, id, scope)
            .await?
            .context("Created announcement could not be reloaded")
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        announcement_id: Uuid,
        scope: CommunicationAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateAnnouncementRequest,
    ) -> Result<Option<AnnouncementDetail>> {
        let actor_id = person_actor_id(actor)?;
        let targets = validated_targets(pool, tenant_id, scope, &request.targets).await?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start announcement update")?;
        let Some(current) = lock_announcement(&mut transaction, tenant_id, announcement_id).await?
        else {
            return Ok(None);
        };
        ensure_managed_record(scope, actor_id, current.created_by)?;
        ensure_status(&current, "draft")?;
        ensure_version(&current, request.expected_version)?;
        replace_targets(&mut transaction, tenant_id, announcement_id, &targets).await?;
        let version = sqlx::query_scalar::<_, i32>(
            r#"UPDATE communication_announcements SET title = $3, body = $4, priority = $5,
                   version = version + 1 WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
               RETURNING version"#,
        )
        .bind(tenant_id)
        .bind(announcement_id)
        .bind(required(&request.title, "Title")?)
        .bind(required(&request.body, "Message")?)
        .bind(request.priority.as_str())
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to update announcement")?;
        append_event(
            &mut transaction,
            tenant_id,
            announcement_id,
            "updated",
            Some("draft"),
            "draft",
            version,
            actor_id,
            None,
            json!({"target_count": targets.len()}),
        )
        .await?;
        append_communication_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "messaging.announcements.update",
            announcement_id,
            json!({"target_count": targets.len()}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit announcement update")?;
        Self::get(pool, tenant_id, announcement_id, scope).await
    }

    pub async fn audience_preview(
        pool: &PgPool,
        tenant_id: Uuid,
        announcement_id: Uuid,
        scope: CommunicationAccessScope,
    ) -> Result<Option<AudiencePreview>> {
        let Some(detail) = Self::get(pool, tenant_id, announcement_id, scope).await? else {
            return Ok(None);
        };
        if matches!(
            detail.summary.status.as_str(),
            "submitted" | "published" | "cancelled"
        ) {
            let recipients = sqlx::query_as::<_, UserReference>(
                r#"SELECT recipient_user_id AS id, recipient_name_snapshot AS full_name,
                          ''::TEXT AS email
                     FROM communication_deliveries WHERE tenant_id = $1 AND announcement_id = $2
                      AND deleted_at IS NULL ORDER BY recipient_name_snapshot, recipient_user_id"#,
            )
            .bind(tenant_id)
            .bind(announcement_id)
            .fetch_all(pool)
            .await
            .context("Failed to load reviewed recipients")?;
            return Ok(Some(AudiencePreview {
                recipient_count: recipients.len() as i64,
                recipients,
            }));
        }
        let rows = load_targets(pool, tenant_id, announcement_id).await?;
        let recipients = resolve_recipients(pool, tenant_id, &rows).await?;
        Ok(Some(AudiencePreview {
            recipient_count: recipients.len() as i64,
            recipients,
        }))
    }

    pub async fn submit(
        pool: &PgPool,
        tenant_id: Uuid,
        announcement_id: Uuid,
        scope: CommunicationAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<AnnouncementDetail>> {
        let actor_id = person_actor_id(actor)?;
        let targets = load_targets(pool, tenant_id, announcement_id).await?;
        let recipients = resolve_recipients(pool, tenant_id, &targets).await?;
        if recipients.is_empty() {
            bail!("This audience has no active linked accounts");
        }
        let fingerprint = recipient_fingerprint(&recipients);
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start announcement submission")?;
        let Some(current) = lock_announcement(&mut transaction, tenant_id, announcement_id).await?
        else {
            return Ok(None);
        };
        ensure_managed_record(scope, actor_id, current.created_by)?;
        ensure_status(&current, "draft")?;
        ensure_version(&current, expected_version)?;
        sqlx::query("UPDATE communication_deliveries SET deleted_at = NOW() WHERE tenant_id = $1 AND announcement_id = $2 AND status = 'pending' AND deleted_at IS NULL")
            .bind(tenant_id).bind(announcement_id).execute(&mut *transaction).await.context("Failed to clear stale recipient review")?;
        for recipient in &recipients {
            sqlx::query(
                r#"INSERT INTO communication_deliveries
                (tenant_id, announcement_id, recipient_user_id, recipient_name_snapshot)
                VALUES ($1, $2, $3, $4)"#,
            )
            .bind(tenant_id)
            .bind(announcement_id)
            .bind(recipient.id)
            .bind(&recipient.full_name)
            .execute(&mut *transaction)
            .await
            .context("Failed to freeze announcement recipient")?;
        }
        let version = sqlx::query_scalar::<_, i32>(
            r#"UPDATE communication_announcements
            SET status = 'submitted', submitted_by = $3, submitted_at = NOW(),
                recipient_fingerprint = $4, version = version + 1
            WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL RETURNING version"#,
        )
        .bind(tenant_id)
        .bind(announcement_id)
        .bind(actor_id)
        .bind(&fingerprint)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to submit announcement")?;
        append_event(
            &mut transaction,
            tenant_id,
            announcement_id,
            "submitted",
            Some("draft"),
            "submitted",
            version,
            actor_id,
            None,
            json!({"recipient_count": recipients.len(), "recipient_fingerprint": fingerprint}),
        )
        .await?;
        append_communication_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "messaging.announcements.submit",
            announcement_id,
            json!({"recipient_count": recipients.len()}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit announcement submission")?;
        Self::get(pool, tenant_id, announcement_id, scope).await
    }

    pub async fn reopen(
        pool: &PgPool,
        tenant_id: Uuid,
        announcement_id: Uuid,
        scope: CommunicationAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ReasonedVersionRequest,
    ) -> Result<Option<AnnouncementDetail>> {
        let actor_id = person_actor_id(actor)?;
        let reason = required(&request.reason, "Reason")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start announcement reopen")?;
        let Some(current) = lock_announcement(&mut transaction, tenant_id, announcement_id).await?
        else {
            return Ok(None);
        };
        ensure_managed_record(scope, actor_id, current.created_by)?;
        ensure_status(&current, "submitted")?;
        ensure_version(&current, request.expected_version)?;
        sqlx::query("UPDATE communication_deliveries SET deleted_at = NOW() WHERE tenant_id = $1 AND announcement_id = $2 AND status = 'pending' AND deleted_at IS NULL")
            .bind(tenant_id).bind(announcement_id).execute(&mut *transaction).await.context("Failed to retire reviewed recipients")?;
        let version = sqlx::query_scalar::<_, i32>(
            r#"UPDATE communication_announcements SET status = 'draft',
            submitted_by = NULL, submitted_at = NULL, recipient_fingerprint = NULL,
            reopened_by = $3, reopened_at = NOW(), reopen_reason = $4, version = version + 1
            WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL RETURNING version"#,
        )
        .bind(tenant_id)
        .bind(announcement_id)
        .bind(actor_id)
        .bind(reason)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to reopen announcement")?;
        append_event(
            &mut transaction,
            tenant_id,
            announcement_id,
            "reopened",
            Some("submitted"),
            "draft",
            version,
            actor_id,
            Some(reason),
            json!({}),
        )
        .await?;
        append_communication_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "messaging.announcements.reopen",
            announcement_id,
            json!({"reason": reason}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit announcement reopen")?;
        Self::get(pool, tenant_id, announcement_id, scope).await
    }

    pub async fn publish(
        pool: &PgPool,
        tenant_id: Uuid,
        announcement_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<AnnouncementDetail>> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start announcement publication")?;
        let Some(current) = lock_announcement(&mut transaction, tenant_id, announcement_id).await?
        else {
            return Ok(None);
        };
        ensure_status(&current, "submitted")?;
        ensure_version(&current, expected_version)?;
        let delivered = sqlx::query(r#"UPDATE communication_deliveries SET status = 'delivered', delivered_at = NOW()
            WHERE tenant_id = $1 AND announcement_id = $2 AND status = 'pending' AND deleted_at IS NULL"#)
            .bind(tenant_id).bind(announcement_id).execute(&mut *transaction).await.context("Failed to publish in-app deliveries")?.rows_affected();
        if delivered == 0 {
            bail!("A reviewed recipient snapshot is required before publication");
        }
        let version = sqlx::query_scalar::<_, i32>(
            r#"UPDATE communication_announcements
            SET status = 'published', published_by = $3, published_at = NOW(), version = version + 1
            WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL RETURNING version"#,
        )
        .bind(tenant_id)
        .bind(announcement_id)
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to publish announcement")?;
        append_event(
            &mut transaction,
            tenant_id,
            announcement_id,
            "published",
            Some("submitted"),
            "published",
            version,
            actor_id,
            None,
            json!({"delivered_count": delivered}),
        )
        .await?;
        append_communication_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "messaging.announcements.publish",
            announcement_id,
            json!({"delivered_count": delivered}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit announcement publication")?;
        Self::get(
            pool,
            tenant_id,
            announcement_id,
            CommunicationAccessScope::Campus,
        )
        .await
    }

    pub async fn cancel(
        pool: &PgPool,
        tenant_id: Uuid,
        announcement_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
        reason: &str,
    ) -> Result<Option<AnnouncementDetail>> {
        let actor_id = person_actor_id(actor)?;
        let reason = required(reason, "Reason")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start announcement cancellation")?;
        let Some(current) = lock_announcement(&mut transaction, tenant_id, announcement_id).await?
        else {
            return Ok(None);
        };
        ensure_status(&current, "published")?;
        ensure_version(&current, expected_version)?;
        let version = sqlx::query_scalar::<_, i32>(r#"UPDATE communication_announcements
            SET status = 'cancelled', cancelled_by = $3, cancelled_at = NOW(), cancellation_reason = $4,
                version = version + 1 WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL RETURNING version"#)
            .bind(tenant_id).bind(announcement_id).bind(actor_id).bind(reason).fetch_one(&mut *transaction).await.context("Failed to cancel announcement")?;
        append_event(
            &mut transaction,
            tenant_id,
            announcement_id,
            "cancelled",
            Some("published"),
            "cancelled",
            version,
            actor_id,
            Some(reason),
            json!({}),
        )
        .await?;
        append_communication_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "messaging.announcements.cancel",
            announcement_id,
            json!({"reason": reason}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit announcement cancellation")?;
        Self::get(
            pool,
            tenant_id,
            announcement_id,
            CommunicationAccessScope::Campus,
        )
        .await
    }

    pub async fn delete(
        pool: &PgPool,
        tenant_id: Uuid,
        announcement_id: Uuid,
        scope: CommunicationAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<bool> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start announcement deletion")?;
        let Some(current) = lock_announcement(&mut transaction, tenant_id, announcement_id).await?
        else {
            return Ok(false);
        };
        ensure_managed_record(scope, actor_id, current.created_by)?;
        ensure_status(&current, "draft")?;
        ensure_version(&current, expected_version)?;
        sqlx::query("UPDATE communication_audience_targets SET deleted_at = NOW() WHERE tenant_id = $1 AND announcement_id = $2 AND deleted_at IS NULL")
            .bind(tenant_id).bind(announcement_id).execute(&mut *transaction).await.context("Failed to retire announcement targets")?;
        sqlx::query("UPDATE communication_announcements SET deleted_at = NOW(), version = version + 1 WHERE tenant_id = $1 AND id = $2")
            .bind(tenant_id).bind(announcement_id).execute(&mut *transaction).await.context("Failed to delete announcement")?;
        append_event(
            &mut transaction,
            tenant_id,
            announcement_id,
            "deleted",
            Some("draft"),
            "deleted",
            current.version + 1,
            actor_id,
            None,
            json!({}),
        )
        .await?;
        append_communication_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "messaging.announcements.delete",
            announcement_id,
            json!({}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit announcement deletion")?;
        Ok(true)
    }

    pub async fn deliveries(
        pool: &PgPool,
        tenant_id: Uuid,
        announcement_id: Uuid,
    ) -> Result<Vec<DeliveryRecord>> {
        sqlx::query_as::<_, DeliveryRecord>(
            r#"SELECT id, announcement_id, recipient_user_id,
            recipient_name_snapshot AS recipient_name, channel, status, delivered_at, read_at
            FROM communication_deliveries WHERE tenant_id = $1 AND announcement_id = $2
            AND deleted_at IS NULL ORDER BY recipient_name_snapshot, recipient_user_id"#,
        )
        .bind(tenant_id)
        .bind(announcement_id)
        .fetch_all(pool)
        .await
        .context("Failed to load delivery history")
    }

    pub async fn inbox(
        pool: &PgPool,
        tenant_id: Uuid,
        user_id: Uuid,
        query: &InboxListQuery,
    ) -> Result<(Vec<InboxItem>, i64)> {
        let (page, per_page) = bounded_page(query.page, query.per_page);
        let offset = (page - 1) * per_page;
        let unread = query.unread_only.unwrap_or(false);
        let messages = sqlx::query_as::<_, InboxItem>(r#"SELECT delivery.id AS delivery_id,
            announcement.id AS announcement_id, announcement.title, announcement.body,
            announcement.priority, announcement.status AS announcement_status,
            announcement.cancellation_reason, creator.full_name AS sender_name,
            announcement.published_at AS published_at, delivery.read_at
            FROM communication_deliveries AS delivery
            JOIN communication_announcements AS announcement ON announcement.id = delivery.announcement_id
                AND announcement.tenant_id = delivery.tenant_id AND announcement.deleted_at IS NULL
            JOIN users AS creator ON creator.id = announcement.created_by AND creator.tenant_id = announcement.tenant_id
            WHERE delivery.tenant_id = $1 AND delivery.recipient_user_id = $2
              AND delivery.status = 'delivered' AND delivery.deleted_at IS NULL
              AND ($3::BOOLEAN = FALSE OR delivery.read_at IS NULL)
            ORDER BY announcement.published_at DESC, delivery.id LIMIT $4 OFFSET $5"#)
            .bind(tenant_id).bind(user_id).bind(unread).bind(per_page).bind(offset)
            .fetch_all(pool).await.context("Failed to load communication inbox")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM communication_deliveries
            WHERE tenant_id = $1 AND recipient_user_id = $2 AND status = 'delivered'
              AND deleted_at IS NULL AND ($3::BOOLEAN = FALSE OR read_at IS NULL)"#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(unread)
        .fetch_one(pool)
        .await
        .context("Failed to count inbox messages")?;
        Ok((messages, total))
    }

    pub async fn inbox_message(
        pool: &PgPool,
        tenant_id: Uuid,
        user_id: Uuid,
        delivery_id: Uuid,
    ) -> Result<Option<InboxItem>> {
        sqlx::query_as::<_, InboxItem>(r#"SELECT delivery.id AS delivery_id,
            announcement.id AS announcement_id, announcement.title, announcement.body,
            announcement.priority, announcement.status AS announcement_status,
            announcement.cancellation_reason, creator.full_name AS sender_name,
            announcement.published_at AS published_at, delivery.read_at
            FROM communication_deliveries AS delivery
            JOIN communication_announcements AS announcement ON announcement.id = delivery.announcement_id
                AND announcement.tenant_id = delivery.tenant_id AND announcement.deleted_at IS NULL
            JOIN users AS creator ON creator.id = announcement.created_by AND creator.tenant_id = announcement.tenant_id
            WHERE delivery.tenant_id = $1 AND delivery.recipient_user_id = $2 AND delivery.id = $3
              AND delivery.status = 'delivered' AND delivery.deleted_at IS NULL"#)
            .bind(tenant_id).bind(user_id).bind(delivery_id).fetch_optional(pool).await.context("Failed to load inbox message")
    }

    pub async fn mark_read(
        pool: &PgPool,
        tenant_id: Uuid,
        user_id: Uuid,
        delivery_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
    ) -> Result<Option<InboxItem>> {
        let mut transaction = pool.begin().await.context("Failed to start read receipt")?;
        let updated = sqlx::query(
            r#"UPDATE communication_deliveries SET read_at = COALESCE(read_at, NOW())
            WHERE tenant_id = $1 AND recipient_user_id = $2 AND id = $3
              AND status = 'delivered' AND read_at IS NULL AND deleted_at IS NULL"#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(delivery_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to record read receipt")?
        .rows_affected();
        if updated == 0 {
            return Self::inbox_message(pool, tenant_id, user_id, delivery_id).await;
        }
        append_communication_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "messaging.inbox.read",
            delivery_id,
            json!({}),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit read receipt")?;
        Self::inbox_message(pool, tenant_id, user_id, delivery_id).await
    }
}

#[derive(Debug)]
struct ValidatedTarget {
    kind: &'static str,
    target_id: Option<Uuid>,
    target_key: Option<String>,
    label: String,
}

async fn validated_targets(
    pool: &PgPool,
    tenant_id: Uuid,
    scope: CommunicationAccessScope,
    inputs: &[AudienceTargetInput],
) -> Result<Vec<ValidatedTarget>> {
    let references = CommunicationOps::reference_data(pool, tenant_id, scope).await?;
    let class_map = references
        .classes
        .into_iter()
        .map(|v| (v.id, format!("{} ({})", v.name, v.code)))
        .collect::<BTreeMap<_, _>>();
    let department_map = references
        .departments
        .into_iter()
        .map(|v| (v.id, format!("{} ({})", v.name, v.code)))
        .collect::<BTreeMap<_, _>>();
    let role_map = references
        .roles
        .into_iter()
        .map(|v| (v.key, v.name))
        .collect::<BTreeMap<_, _>>();
    let user_map = references
        .users
        .into_iter()
        .map(|v| (v.id, v.full_name))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut values = Vec::new();
    for input in inputs {
        let (kind, id, key, label) = match input.kind {
            AudienceKind::Campus if references.campus_allowed => {
                ("campus", None, None, "Entire campus".to_string())
            }
            AudienceKind::Role => {
                let key = input
                    .target_key
                    .as_deref()
                    .map(str::trim)
                    .context("A role audience requires a role key")?;
                let label = role_map
                    .get(key)
                    .cloned()
                    .context("The selected role is unavailable")?;
                ("role", None, Some(key.to_string()), label)
            }
            AudienceKind::ClassGroup => {
                let id = input
                    .target_id
                    .context("A class audience requires a class")?;
                let label = class_map
                    .get(&id)
                    .cloned()
                    .context("The selected class is unavailable")?;
                ("class_group", Some(id), None, label)
            }
            AudienceKind::Department => {
                let id = input
                    .target_id
                    .context("A department audience requires a department")?;
                let label = department_map
                    .get(&id)
                    .cloned()
                    .context("The selected department is unavailable")?;
                ("department", Some(id), None, label)
            }
            AudienceKind::Individual => {
                let id = input
                    .target_id
                    .context("An individual audience requires an account")?;
                let label = user_map
                    .get(&id)
                    .cloned()
                    .context("The selected account is unavailable")?;
                ("individual", Some(id), None, label)
            }
            _ => bail!("This audience is outside your communication scope"),
        };
        let identity = format!(
            "{kind}:{}:{}",
            id.map(|v| v.to_string()).unwrap_or_default(),
            key.as_deref().unwrap_or_default()
        );
        if !seen.insert(identity) {
            bail!("Each audience may be selected only once");
        }
        values.push(ValidatedTarget {
            kind,
            target_id: id,
            target_key: key,
            label,
        });
    }
    if values.is_empty() {
        bail!("At least one audience is required");
    }
    Ok(values)
}

async fn replace_targets(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    announcement_id: Uuid,
    targets: &[ValidatedTarget],
) -> Result<()> {
    sqlx::query("UPDATE communication_audience_targets SET deleted_at = NOW() WHERE tenant_id = $1 AND announcement_id = $2 AND deleted_at IS NULL")
        .bind(tenant_id).bind(announcement_id).execute(&mut **transaction).await.context("Failed to replace announcement targets")?;
    for target in targets {
        sqlx::query(
            r#"INSERT INTO communication_audience_targets
            (tenant_id, announcement_id, target_kind, target_id, target_key, label_snapshot)
            VALUES ($1, $2, $3, $4, $5, $6)"#,
        )
        .bind(tenant_id)
        .bind(announcement_id)
        .bind(target.kind)
        .bind(target.target_id)
        .bind(&target.target_key)
        .bind(&target.label)
        .execute(&mut **transaction)
        .await
        .context("Failed to store announcement target")?;
    }
    Ok(())
}

async fn load_targets(
    pool: &PgPool,
    tenant_id: Uuid,
    announcement_id: Uuid,
) -> Result<Vec<AudienceTargetRow>> {
    sqlx::query_as::<_, AudienceTargetRow>(
        r#"SELECT id, target_kind, target_id, target_key, label_snapshot
        FROM communication_audience_targets WHERE tenant_id = $1 AND announcement_id = $2
        AND deleted_at IS NULL ORDER BY label_snapshot, id"#,
    )
    .bind(tenant_id)
    .bind(announcement_id)
    .fetch_all(pool)
    .await
    .context("Failed to load announcement targets")
}

async fn resolve_recipients(
    pool: &PgPool,
    tenant_id: Uuid,
    targets: &[AudienceTargetRow],
) -> Result<Vec<UserReference>> {
    let mut ids = BTreeSet::new();
    for target in targets {
        match target.target_kind.as_str() {
            "campus" => {
                for user in active_users(pool, tenant_id, None).await? {
                    ids.insert(user.id);
                }
            }
            "role" => {
                let key = target
                    .target_key
                    .as_deref()
                    .context("Stored role audience is invalid")?;
                let values = sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE tenant_id = $1 AND $2 = ANY(roles) AND is_active AND deleted_at IS NULL ORDER BY id")
                    .bind(tenant_id).bind(key).fetch_all(pool).await.context("Failed to resolve role recipients")?;
                ids.extend(values);
            }
            "class_group" => {
                let id = target
                    .target_id
                    .context("Stored class audience is invalid")?;
                ids.extend(
                    SisAudienceOps::class_recipient_accounts(pool, tenant_id, id)
                        .await?
                        .into_iter()
                        .map(|v| v.account_id),
                );
            }
            "department" => {
                let id = target
                    .target_id
                    .context("Stored department audience is invalid")?;
                ids.extend(
                    HrAudienceOps::department_recipient_accounts(pool, tenant_id, id)
                        .await?
                        .into_iter()
                        .map(|v| v.account_id),
                );
            }
            "individual" => {
                ids.insert(
                    target
                        .target_id
                        .context("Stored individual audience is invalid")?,
                );
            }
            _ => bail!("Stored communication audience is invalid"),
        }
    }
    active_users(pool, tenant_id, Some(&ids.into_iter().collect::<Vec<_>>())).await
}

async fn active_users(
    pool: &PgPool,
    tenant_id: Uuid,
    ids: Option<&[Uuid]>,
) -> Result<Vec<UserReference>> {
    sqlx::query_as::<_, UserReference>(
        r#"SELECT id, full_name, email FROM users
        WHERE tenant_id = $1 AND is_active AND deleted_at IS NULL
          AND ($2::UUID[] IS NULL OR id = ANY($2)) ORDER BY full_name, email, id"#,
    )
    .bind(tenant_id)
    .bind(ids)
    .fetch_all(pool)
    .await
    .context("Failed to resolve active communication accounts")
}

fn announcement_select() -> &'static str {
    r#"SELECT announcement.id, announcement.title, announcement.body,
    announcement.priority, announcement.status, announcement.version, announcement.created_by,
    creator.full_name AS creator_name, announcement.submitted_at, announcement.published_at,
    announcement.cancelled_at, announcement.cancellation_reason, announcement.reopened_at,
    announcement.reopen_reason, COUNT(delivery.id)::BIGINT AS recipient_count,
    COUNT(delivery.id) FILTER (WHERE delivery.read_at IS NOT NULL)::BIGINT AS read_count,
    announcement.created_at, announcement.updated_at
    FROM communication_announcements AS announcement
    JOIN users AS creator ON creator.id = announcement.created_by AND creator.tenant_id = announcement.tenant_id
    LEFT JOIN communication_deliveries AS delivery ON delivery.announcement_id = announcement.id
      AND delivery.tenant_id = announcement.tenant_id AND delivery.deleted_at IS NULL
    WHERE announcement.tenant_id = $1 AND announcement.deleted_at IS NULL
      AND ($2::UUID IS NULL OR announcement.created_by = $2)
      AND ($3::TEXT IS NULL OR announcement.status = $3)
      AND ($4::TEXT IS NULL OR announcement.title ILIKE $4 OR announcement.body ILIKE $4)
      AND ($5::UUID IS NULL OR announcement.id = $5)
    GROUP BY announcement.id, creator.full_name"#
}

fn summary_from_row(row: AnnouncementRow) -> AnnouncementSummary {
    AnnouncementSummary {
        id: row.id,
        title: row.title,
        priority: row.priority,
        status: row.status,
        version: row.version,
        creator_name: row.creator_name,
        recipient_count: row.recipient_count,
        read_count: row.read_count,
        created_at: row.created_at,
        updated_at: row.updated_at,
        published_at: row.published_at,
    }
}
fn detail_from_row(row: AnnouncementRow, targets: Vec<AudienceTarget>) -> AnnouncementDetail {
    let body = row.body.clone();
    let created_by = row.created_by;
    let submitted_at = row.submitted_at;
    let cancelled_at = row.cancelled_at;
    let cancellation_reason = row.cancellation_reason.clone();
    let reopened_at = row.reopened_at;
    let reopen_reason = row.reopen_reason.clone();
    AnnouncementDetail {
        summary: summary_from_row(row),
        body,
        created_by,
        targets,
        submitted_at,
        cancelled_at,
        cancellation_reason,
        reopened_at,
        reopen_reason,
    }
}
fn target_from_row(row: AudienceTargetRow) -> AudienceTarget {
    AudienceTarget {
        id: row.id,
        kind: row.target_kind,
        target_id: row.target_id,
        target_key: row.target_key,
        label: row.label_snapshot,
    }
}
fn scope_allows_announcement(scope: CommunicationAccessScope, created_by: Uuid) -> bool {
    match scope {
        CommunicationAccessScope::Campus => true,
        CommunicationAccessScope::AssignedTo(actor) => actor == created_by,
        CommunicationAccessScope::SelfFor(_) => false,
    }
}
fn ensure_can_manage(scope: CommunicationAccessScope, actor: Uuid) -> Result<()> {
    match scope {
        CommunicationAccessScope::Campus => Ok(()),
        CommunicationAccessScope::AssignedTo(id) if id == actor => Ok(()),
        _ => bail!("This communication workflow is unavailable"),
    }
}
fn ensure_managed_record(
    scope: CommunicationAccessScope,
    actor: Uuid,
    created_by: Uuid,
) -> Result<()> {
    ensure_can_manage(scope, actor)?;
    if !scope_allows_announcement(scope, created_by) {
        bail!("This announcement is outside your communication scope");
    }
    Ok(())
}
fn ensure_status(current: &LockedAnnouncement, expected: &str) -> Result<()> {
    if current.status != expected {
        bail!("This announcement is no longer {expected}");
    }
    Ok(())
}
fn ensure_version(current: &LockedAnnouncement, expected: i32) -> Result<()> {
    if current.version != expected {
        bail!("This announcement changed. Reload it before continuing");
    }
    Ok(())
}
fn person_actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("Authenticated person actor is required"))
}
fn required<'a>(value: &'a str, field: &str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{field} is required");
    }
    Ok(value)
}
fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(DEFAULT_PAGE).max(1),
        per_page.unwrap_or(DEFAULT_PER_PAGE).clamp(1, MAX_PER_PAGE),
    )
}
fn recipient_fingerprint(values: &[UserReference]) -> String {
    let canonical = values
        .iter()
        .map(|v| v.id.to_string())
        .collect::<Vec<_>>()
        .join("|");
    format!("{:x}", Sha256::digest(canonical.as_bytes()))
}

async fn lock_announcement(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<LockedAnnouncement>> {
    sqlx::query_as::<_, LockedAnnouncement>("SELECT status, version, created_by FROM communication_announcements WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL FOR UPDATE")
        .bind(tenant_id).bind(id).fetch_optional(&mut **transaction).await.context("Failed to lock announcement")
}

#[allow(clippy::too_many_arguments)]
async fn append_event(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
    event_type: &str,
    from_status: Option<&str>,
    to_status: &str,
    version: i32,
    actor_id: Uuid,
    reason: Option<&str>,
    metadata: Value,
) -> Result<()> {
    sqlx::query(r#"INSERT INTO communication_announcement_events
        (tenant_id, announcement_id, event_type, from_status, to_status, announcement_version, actor_id, reason, metadata)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)"#).bind(tenant_id).bind(id).bind(event_type).bind(from_status).bind(to_status).bind(version).bind(actor_id).bind(reason).bind(metadata)
        .execute(&mut **transaction).await.context("Failed to append communication history")?;
    Ok(())
}

async fn append_communication_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    action: &str,
    id: Uuid,
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
            "communication_announcement",
            id.to_string(),
        ))
        .with_redacted_metadata(metadata),
    )
    .await
    .context("Failed to append communication audit event")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        CommunicationAccessScope, LockedAnnouncement, UserReference, bounded_page,
        ensure_can_manage, ensure_managed_record, ensure_status, ensure_version,
        recipient_fingerprint, required, scope_allows_announcement,
    };
    use uuid::Uuid;

    #[test]
    fn announcement_scope_never_turns_self_access_into_authoring_access() {
        let author = Uuid::new_v4();
        let other = Uuid::new_v4();
        assert!(scope_allows_announcement(
            CommunicationAccessScope::Campus,
            author
        ));
        assert!(scope_allows_announcement(
            CommunicationAccessScope::AssignedTo(author),
            author
        ));
        assert!(!scope_allows_announcement(
            CommunicationAccessScope::AssignedTo(other),
            author
        ));
        assert!(!scope_allows_announcement(
            CommunicationAccessScope::SelfFor(author),
            author
        ));
        assert!(ensure_can_manage(CommunicationAccessScope::AssignedTo(author), author).is_ok());
        assert!(ensure_can_manage(CommunicationAccessScope::SelfFor(author), author).is_err());
        assert!(
            ensure_managed_record(CommunicationAccessScope::AssignedTo(other), other, author,)
                .is_err()
        );
    }

    #[test]
    fn stale_or_wrong_lifecycle_state_is_rejected() {
        let current = LockedAnnouncement {
            status: "draft".to_string(),
            version: 3,
            created_by: Uuid::new_v4(),
        };
        assert!(ensure_status(&current, "draft").is_ok());
        assert!(ensure_status(&current, "submitted").is_err());
        assert!(ensure_version(&current, 3).is_ok());
        assert!(ensure_version(&current, 2).is_err());
    }

    #[test]
    fn pagination_and_required_text_are_normalized() {
        assert_eq!(bounded_page(None, None), (1, 25));
        assert_eq!(bounded_page(Some(-4), Some(900)), (1, 100));
        assert_eq!(required("  notice  ", "Message").unwrap(), "notice");
        assert!(required("   ", "Message").is_err());
    }

    #[test]
    fn recipient_fingerprint_is_deterministic_for_the_frozen_order() {
        let first = UserReference {
            id: Uuid::new_v4(),
            full_name: "First".to_string(),
            email: "first@example.test".to_string(),
        };
        let second = UserReference {
            id: Uuid::new_v4(),
            full_name: "Second".to_string(),
            email: "second@example.test".to_string(),
        };
        let recipients = vec![first, second];
        let fingerprint = recipient_fingerprint(&recipients);
        assert_eq!(fingerprint.len(), 64);
        assert_eq!(fingerprint, recipient_fingerprint(&recipients));
    }
}
