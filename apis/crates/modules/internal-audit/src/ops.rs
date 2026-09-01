//! Transactional Internal Audit workflows and read projections.
//!
//! Lifecycle transitions lock and re-read authority-bearing rows. Every accepted
//! write appends module history and the shared actor-aware audit event in the same
//! transaction; source-module and Document Registry records remain read-only.

use anyhow::{Context, Result, anyhow, bail};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_document_registry::DocumentRegistryOps;
use serde_json::{Map, Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::dtos::{
    AuditorCandidateResponse, CloseRequest, CreateEngagementRequest, CreateFindingRequest,
    CreatePlanRequest, EngagementResponse, EvidenceResponse, FindingRating, FindingResponse,
    InternalAuditAccessScope, InternalAuditListQuery, LinkEvidenceRequest, NumberingPolicyResponse,
    PlanResponse, UpdateEngagementRequest, UpdateFindingRequest, UpdateNumberingPolicyRequest,
    UpdatePlanRequest,
};
use crate::models::{EngagementRow, EvidenceRow, FindingRow, NumberingPolicyRow, PlanRow};

pub struct InternalAuditOps;

impl InternalAuditOps {
    pub async fn numbering_policy(
        pool: &PgPool,
        tenant_id: Uuid,
    ) -> Result<NumberingPolicyResponse> {
        let row = sqlx::query_as::<_, NumberingPolicyRow>(NUMBERING_SELECT)
            .bind(tenant_id)
            .fetch_one(pool)
            .await
            .context("Failed to load Internal Audit numbering")?;
        Ok(numbering_response(row))
    }

    pub async fn update_numbering_policy(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &UpdateNumberingPolicyRequest,
    ) -> Result<NumberingPolicyResponse> {
        let actor_id = actor_user_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("Failed to start numbering update")?;
        let row = sqlx::query_as::<_, NumberingPolicyRow>(
            r#"
            UPDATE internal_audit_numbering_policies
            SET plan_prefix = $3,
                engagement_prefix = $4,
                finding_prefix = $5,
                padding = $6,
                next_plan_sequence = $7,
                next_engagement_sequence = $8,
                next_finding_sequence = $9,
                version = version + 1
            WHERE tenant_id = $1
              AND version = $2
              AND next_plan_sequence <= $7
              AND next_engagement_sequence <= $8
              AND next_finding_sequence <= $9
              AND deleted_at IS NULL
            RETURNING plan_prefix, engagement_prefix, finding_prefix, padding,
                      next_plan_sequence, next_engagement_sequence, next_finding_sequence,
                      version, updated_at
            "#,
        )
        .bind(tenant_id)
        .bind(request.version)
        .bind(trimmed_required(&request.plan_prefix, "Plan prefix")?)
        .bind(trimmed_required(
            &request.engagement_prefix,
            "Engagement prefix",
        )?)
        .bind(trimmed_required(&request.finding_prefix, "Finding prefix")?)
        .bind(request.padding)
        .bind(request.next_plan_sequence)
        .bind(request.next_engagement_sequence)
        .bind(request.next_finding_sequence)
        .fetch_optional(&mut *tx)
        .await
        .context("Failed to update Internal Audit numbering")?
        .ok_or_else(|| {
            anyhow!("Internal Audit numbering changed or a sequence would move backwards")
        })?;
        append_event(
            &mut tx,
            tenant_id,
            "numbering_policy",
            tenant_id,
            None,
            "internal_audit.numbering.updated",
            actor_id,
            json!({"version": row.version}),
        )
        .await?;
        append_shared_audit(
            &mut tx,
            tenant_id,
            actor,
            context,
            "internal_audit.numbering_policy.update",
            "internal_audit_numbering_policy",
            tenant_id,
            None,
            json!({"version": row.version}),
        )
        .await?;
        tx.commit()
            .await
            .context("Failed to commit numbering update")?;
        Ok(numbering_response(row))
    }

    pub async fn list_plans(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &InternalAuditListQuery,
    ) -> Result<(Vec<PlanResponse>, i64)> {
        let (limit, offset) = page_bounds(query);
        let search = search_pattern(query.search.as_deref());
        let status = normalized_filter(query.status.as_deref());
        let rows = sqlx::query_as::<_, PlanRow>(PLAN_LIST)
            .bind(tenant_id)
            .bind(status.as_deref())
            .bind(search.as_deref())
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .context("Failed to list audit plans")?;
        let total = sqlx::query_scalar::<_, i64>(PLAN_COUNT)
            .bind(tenant_id)
            .bind(status.as_deref())
            .bind(search.as_deref())
            .fetch_one(pool)
            .await
            .context("Failed to count audit plans")?;
        Ok((rows.into_iter().map(plan_response).collect(), total))
    }

    pub async fn get_plan(
        pool: &PgPool,
        tenant_id: Uuid,
        plan_id: Uuid,
    ) -> Result<Option<PlanResponse>> {
        sqlx::query_as::<_, PlanRow>(PLAN_BY_ID)
            .bind(tenant_id)
            .bind(plan_id)
            .fetch_optional(pool)
            .await
            .context("Failed to load audit plan")
            .map(|row| row.map(plan_response))
    }

    pub async fn create_plan(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &CreatePlanRequest,
    ) -> Result<PlanResponse> {
        validate_dates(request.period_start, request.period_end, "Audit plan")?;
        let actor_id = actor_user_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("Failed to start audit plan creation")?;
        let reference = reserve_reference(&mut tx, tenant_id, SequenceKind::Plan).await?;
        let plan_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO internal_audit_plans (
                id, tenant_id, reference, title, objective, risk_summary,
                period_start, period_end, created_by, updated_by
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$9)
            "#,
        )
        .bind(plan_id)
        .bind(tenant_id)
        .bind(&reference)
        .bind(trimmed_required(&request.title, "Plan title")?)
        .bind(trimmed_required(&request.objective, "Plan objective")?)
        .bind(trimmed_optional(request.risk_summary.as_deref()))
        .bind(request.period_start)
        .bind(request.period_end)
        .bind(actor_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| database_error(error, "Failed to create audit plan"))?;
        append_event(
            &mut tx,
            tenant_id,
            "plan",
            plan_id,
            None,
            "internal_audit.plan.created",
            actor_id,
            json!({"reference": reference}),
        )
        .await?;
        append_shared_audit(
            &mut tx,
            tenant_id,
            actor,
            context,
            "internal_audit.plans.create",
            "internal_audit_plan",
            plan_id,
            None,
            json!({"reference": reference}),
        )
        .await?;
        tx.commit().await.context("Failed to commit audit plan")?;
        Self::get_plan(pool, tenant_id, plan_id)
            .await?
            .context("Created audit plan could not be reloaded")
    }

    pub async fn update_plan(
        pool: &PgPool,
        tenant_id: Uuid,
        plan_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &UpdatePlanRequest,
    ) -> Result<Option<PlanResponse>> {
        validate_dates(request.period_start, request.period_end, "Audit plan")?;
        let actor_id = actor_user_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("Failed to start audit plan update")?;
        let current = lock_plan(&mut tx, tenant_id, plan_id).await?;
        let Some((status, version)) = current else {
            return Ok(None);
        };
        ensure_status(&status, "draft", "Only a draft audit plan can be changed")?;
        ensure_version(version, request.expected_version, "Audit plan")?;
        sqlx::query(
            r#"
            UPDATE internal_audit_plans
            SET title=$3, objective=$4, risk_summary=$5, period_start=$6,
                period_end=$7, updated_by=$8, version=version+1
            WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(plan_id)
        .bind(trimmed_required(&request.title, "Plan title")?)
        .bind(trimmed_required(&request.objective, "Plan objective")?)
        .bind(trimmed_optional(request.risk_summary.as_deref()))
        .bind(request.period_start)
        .bind(request.period_end)
        .bind(actor_id)
        .execute(&mut *tx)
        .await
        .context("Failed to update audit plan")?;
        append_domain_write(
            &mut tx,
            tenant_id,
            actor,
            context,
            "plan",
            plan_id,
            None,
            "internal_audit.plan.updated",
            "internal_audit.plans.update",
            json!({"version": version + 1}),
        )
        .await?;
        tx.commit()
            .await
            .context("Failed to commit audit plan update")?;
        Self::get_plan(pool, tenant_id, plan_id).await
    }

    pub async fn approve_plan(
        pool: &PgPool,
        tenant_id: Uuid,
        plan_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<PlanResponse>> {
        let actor_id = actor_user_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("Failed to start plan approval")?;
        let Some((status, version)) = lock_plan(&mut tx, tenant_id, plan_id).await? else {
            return Ok(None);
        };
        ensure_status(&status, "draft", "Only a draft audit plan can be approved")?;
        ensure_version(version, expected_version, "Audit plan")?;
        sqlx::query(
            "UPDATE internal_audit_plans SET status='approved', approved_by=$3, approved_at=NOW(), updated_by=$3, version=version+1 WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(plan_id)
        .bind(actor_id)
        .execute(&mut *tx)
        .await
        .context("Failed to approve audit plan")?;
        append_domain_write(
            &mut tx,
            tenant_id,
            actor,
            context,
            "plan",
            plan_id,
            None,
            "internal_audit.plan.approved",
            "internal_audit.plans.approve",
            json!({"version": version + 1}),
        )
        .await?;
        tx.commit()
            .await
            .context("Failed to commit plan approval")?;
        Self::get_plan(pool, tenant_id, plan_id).await
    }

    pub async fn close_plan(
        pool: &PgPool,
        tenant_id: Uuid,
        plan_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &CloseRequest,
    ) -> Result<Option<PlanResponse>> {
        let actor_id = actor_user_id(actor)?;
        let mut tx = pool.begin().await.context("Failed to start plan closure")?;
        let Some((status, version)) = lock_plan(&mut tx, tenant_id, plan_id).await? else {
            return Ok(None);
        };
        ensure_status(
            &status,
            "approved",
            "Only an approved audit plan can be closed",
        )?;
        ensure_version(version, request.expected_version, "Audit plan")?;
        let (engagement_count, open_count) = sqlx::query_as::<_, (i64, i64)>(
            "SELECT COUNT(*), COUNT(*) FILTER (WHERE status <> 'closed') FROM internal_audit_engagements WHERE tenant_id=$1 AND plan_id=$2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(plan_id)
        .fetch_one(&mut *tx)
        .await
        .context("Failed to validate audit plan closure")?;
        if engagement_count == 0 {
            bail!("An audit plan requires at least one engagement before closure");
        }
        if open_count > 0 {
            bail!("Close every engagement before closing the audit plan");
        }
        let summary = trimmed_required(&request.summary, "Closure summary")?;
        sqlx::query(
            "UPDATE internal_audit_plans SET status='closed', closed_by=$3, closed_at=NOW(), close_summary=$4, updated_by=$3, version=version+1 WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(plan_id)
        .bind(actor_id)
        .bind(summary)
        .execute(&mut *tx)
        .await
        .context("Failed to close audit plan")?;
        append_domain_write(
            &mut tx,
            tenant_id,
            actor,
            context,
            "plan",
            plan_id,
            None,
            "internal_audit.plan.closed",
            "internal_audit.plans.close",
            json!({"version": version + 1}),
        )
        .await?;
        tx.commit().await.context("Failed to commit plan closure")?;
        Self::get_plan(pool, tenant_id, plan_id).await
    }

    pub async fn delete_plan(
        pool: &PgPool,
        tenant_id: Uuid,
        plan_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        expected_version: i32,
    ) -> Result<bool> {
        let actor_id = actor_user_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("Failed to start audit plan deletion")?;
        let Some((status, version)) = lock_plan(&mut tx, tenant_id, plan_id).await? else {
            return Ok(false);
        };
        ensure_status(&status, "draft", "Only a draft audit plan can be deleted")?;
        ensure_version(version, expected_version, "Audit plan")?;
        let has_engagements = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM internal_audit_engagements WHERE tenant_id=$1 AND plan_id=$2 AND deleted_at IS NULL)",
        )
        .bind(tenant_id)
        .bind(plan_id)
        .fetch_one(&mut *tx)
        .await
        .context("Failed to validate audit plan deletion")?;
        if has_engagements {
            bail!("An audit plan with engagements cannot be deleted");
        }
        sqlx::query(
            "UPDATE internal_audit_plans SET deleted_at=NOW(), updated_by=$3, version=version+1 WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(plan_id)
        .bind(actor_id)
        .execute(&mut *tx)
        .await
        .context("Failed to delete audit plan")?;
        append_domain_write(
            &mut tx,
            tenant_id,
            actor,
            context,
            "plan",
            plan_id,
            None,
            "internal_audit.plan.deleted",
            "internal_audit.plans.delete",
            json!({"version": version + 1}),
        )
        .await?;
        tx.commit()
            .await
            .context("Failed to commit audit plan deletion")?;
        Ok(true)
    }

    pub async fn auditor_candidates(
        pool: &PgPool,
        tenant_id: Uuid,
        search: Option<&str>,
    ) -> Result<Vec<AuditorCandidateResponse>> {
        let search = search_pattern(search);
        sqlx::query_as::<_, (Uuid, String, String)>(
            r#"
            SELECT id, full_name, email
            FROM users
            WHERE tenant_id=$1 AND is_active AND deleted_at IS NULL
              AND roles && ARRAY['internal_auditor','audit_manager']::TEXT[]
              AND ($2::TEXT IS NULL OR full_name ILIKE $2 OR email ILIKE $2)
            ORDER BY full_name, email
            LIMIT 100
            "#,
        )
        .bind(tenant_id)
        .bind(search.as_deref())
        .fetch_all(pool)
        .await
        .context("Failed to load auditor candidates")
        .map(|rows| {
            rows.into_iter()
                .map(|(user_id, full_name, email)| AuditorCandidateResponse {
                    user_id,
                    full_name,
                    email,
                })
                .collect()
        })
    }

    pub async fn list_engagements(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: InternalAuditAccessScope,
        query: &InternalAuditListQuery,
    ) -> Result<(Vec<EngagementResponse>, i64)> {
        let (limit, offset) = page_bounds(query);
        let search = search_pattern(query.search.as_deref());
        let status = normalized_filter(query.status.as_deref());
        let assigned_user_id = scope.assigned_user_id();
        let rows = sqlx::query_as::<_, EngagementRow>(ENGAGEMENT_LIST)
            .bind(tenant_id)
            .bind(status.as_deref())
            .bind(query.plan_id)
            .bind(search.as_deref())
            .bind(assigned_user_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .context("Failed to list audit engagements")?;
        let total = sqlx::query_scalar::<_, i64>(ENGAGEMENT_COUNT)
            .bind(tenant_id)
            .bind(status.as_deref())
            .bind(query.plan_id)
            .bind(search.as_deref())
            .bind(assigned_user_id)
            .fetch_one(pool)
            .await
            .context("Failed to count audit engagements")?;
        Ok((rows.into_iter().map(engagement_response).collect(), total))
    }

    pub async fn get_engagement(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: InternalAuditAccessScope,
        engagement_id: Uuid,
    ) -> Result<Option<EngagementResponse>> {
        sqlx::query_as::<_, EngagementRow>(ENGAGEMENT_BY_ID)
            .bind(tenant_id)
            .bind(engagement_id)
            .bind(scope.assigned_user_id())
            .fetch_optional(pool)
            .await
            .context("Failed to load audit engagement")
            .map(|row| row.map(engagement_response))
    }

    pub async fn create_engagement(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: InternalAuditAccessScope,
        actor: AuditActor,
        context: RequestContext,
        request: &CreateEngagementRequest,
    ) -> Result<EngagementResponse> {
        validate_dates(request.starts_on, request.due_on, "Audit engagement")?;
        let actor_id = actor_user_id(actor)?;
        enforce_lead_assignment(scope, actor_id, request.lead_auditor_user_id)?;
        let mut tx = pool
            .begin()
            .await
            .context("Failed to start audit engagement creation")?;
        validate_plan_for_engagement(
            &mut tx,
            tenant_id,
            request.plan_id,
            request.starts_on,
            request.due_on,
        )
        .await?;
        validate_auditor(&mut tx, tenant_id, request.lead_auditor_user_id).await?;
        let reference = reserve_reference(&mut tx, tenant_id, SequenceKind::Engagement).await?;
        let engagement_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO internal_audit_engagements (
                id, tenant_id, plan_id, reference, title, objective, scope_text,
                lead_auditor_user_id, starts_on, due_on, created_by, updated_by
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$11)
            "#,
        )
        .bind(engagement_id)
        .bind(tenant_id)
        .bind(request.plan_id)
        .bind(&reference)
        .bind(trimmed_required(&request.title, "Engagement title")?)
        .bind(trimmed_required(
            &request.objective,
            "Engagement objective",
        )?)
        .bind(trimmed_required(&request.scope_text, "Engagement scope")?)
        .bind(request.lead_auditor_user_id)
        .bind(request.starts_on)
        .bind(request.due_on)
        .bind(actor_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| database_error(error, "Failed to create audit engagement"))?;
        append_domain_write(
            &mut tx,
            tenant_id,
            actor,
            context,
            "engagement",
            engagement_id,
            Some(engagement_id),
            "internal_audit.engagement.created",
            "internal_audit.engagements.create",
            json!({"reference": reference,"plan_id":request.plan_id,"lead_auditor_user_id":request.lead_auditor_user_id}),
        )
        .await?;
        tx.commit()
            .await
            .context("Failed to commit audit engagement")?;
        Self::get_engagement(pool, tenant_id, scope, engagement_id)
            .await?
            .context("Created audit engagement could not be reloaded")
    }
}

#[derive(Debug)]
struct LockedEngagement {
    plan_id: Uuid,
    status: String,
    version: i32,
}

#[derive(Debug)]
struct LockedFinding {
    engagement_id: Uuid,
    status: String,
    version: i32,
}

#[derive(Debug, Clone, Copy)]
enum SequenceKind {
    Plan,
    Engagement,
    Finding,
}

#[derive(Debug, Clone, Copy)]
enum EngagementTransition {
    Start,
    BeginReporting,
    Close,
}

#[allow(clippy::too_many_arguments)]
async fn transition_engagement(
    pool: &PgPool,
    tenant_id: Uuid,
    scope: InternalAuditAccessScope,
    engagement_id: Uuid,
    actor: AuditActor,
    context: RequestContext,
    expected_version: i32,
    transition: EngagementTransition,
    close_summary: Option<String>,
) -> Result<Option<EngagementResponse>> {
    let actor_id = actor_user_id(actor)?;
    let mut tx = pool
        .begin()
        .await
        .context("Failed to start engagement transition")?;
    let Some(current) = lock_engagement(&mut tx, tenant_id, scope, engagement_id).await? else {
        return Ok(None);
    };
    ensure_version(current.version, expected_version, "Audit engagement")?;
    let (expected_status, next_status, event_type, operation_key) = match transition {
        EngagementTransition::Start => (
            "planned",
            "fieldwork",
            "internal_audit.engagement.started",
            "internal_audit.engagements.start",
        ),
        EngagementTransition::BeginReporting => (
            "fieldwork",
            "reporting",
            "internal_audit.engagement.reporting_started",
            "internal_audit.engagements.begin_reporting",
        ),
        EngagementTransition::Close => (
            "reporting",
            "closed",
            "internal_audit.engagement.closed",
            "internal_audit.engagements.close",
        ),
    };
    ensure_status(
        &current.status,
        expected_status,
        &format!("Only a {expected_status} audit engagement can move to {next_status}"),
    )?;
    if matches!(transition, EngagementTransition::BeginReporting) {
        let has_evidence = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM internal_audit_evidence WHERE tenant_id=$1 AND engagement_id=$2)",
        )
        .bind(tenant_id)
        .bind(engagement_id)
        .fetch_one(&mut *tx)
        .await
        .context("Failed to validate engagement evidence")?;
        if !has_evidence {
            bail!("Link governed evidence before moving the engagement to reporting");
        }
    }
    if matches!(transition, EngagementTransition::Close) {
        let draft_findings = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM internal_audit_findings WHERE tenant_id=$1 AND engagement_id=$2 AND status='draft' AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(engagement_id)
        .fetch_one(&mut *tx)
        .await
        .context("Failed to validate engagement findings")?;
        if draft_findings > 0 {
            bail!("Issue or delete every draft finding before closing the engagement");
        }
    }
    match transition {
        EngagementTransition::Start => {
            sqlx::query(
                "UPDATE internal_audit_engagements SET status='fieldwork', started_by=$3, started_at=NOW(), updated_by=$3, version=version+1 WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL",
            )
            .bind(tenant_id)
            .bind(engagement_id)
            .bind(actor_id)
            .execute(&mut *tx)
            .await
            .context("Failed to start audit engagement")?;
        }
        EngagementTransition::BeginReporting => {
            sqlx::query(
                "UPDATE internal_audit_engagements SET status='reporting', reporting_by=$3, reporting_at=NOW(), updated_by=$3, version=version+1 WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL",
            )
            .bind(tenant_id)
            .bind(engagement_id)
            .bind(actor_id)
            .execute(&mut *tx)
            .await
            .context("Failed to start audit engagement reporting")?;
        }
        EngagementTransition::Close => {
            let summary = close_summary.context("Engagement closure requires a summary")?;
            sqlx::query(
                "UPDATE internal_audit_engagements SET status='closed', closed_by=$3, closed_at=NOW(), close_summary=$4, updated_by=$3, version=version+1 WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL",
            )
            .bind(tenant_id)
            .bind(engagement_id)
            .bind(actor_id)
            .bind(summary)
            .execute(&mut *tx)
            .await
            .context("Failed to close audit engagement")?;
        }
    }
    append_domain_write(
        &mut tx,
        tenant_id,
        actor,
        context,
        "engagement",
        engagement_id,
        Some(engagement_id),
        event_type,
        operation_key,
        json!({"from":expected_status,"to":next_status,"version":current.version + 1}),
    )
    .await?;
    tx.commit()
        .await
        .context("Failed to commit engagement transition")?;
    InternalAuditOps::get_engagement(pool, tenant_id, scope, engagement_id).await
}

async fn lock_plan(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    plan_id: Uuid,
) -> Result<Option<(String, i32)>> {
    sqlx::query_as::<_, (String, i32)>(
        "SELECT status, version FROM internal_audit_plans WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(plan_id)
    .fetch_optional(&mut **tx)
    .await
    .context("Failed to lock audit plan")
}

async fn lock_engagement(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    scope: InternalAuditAccessScope,
    engagement_id: Uuid,
) -> Result<Option<LockedEngagement>> {
    sqlx::query_as::<_, (Uuid, String, i32)>(
        r#"
        SELECT plan_id, status, version
        FROM internal_audit_engagements
        WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL
          AND ($3::UUID IS NULL OR lead_auditor_user_id=$3)
        FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(engagement_id)
    .bind(scope.assigned_user_id())
    .fetch_optional(&mut **tx)
    .await
    .context("Failed to lock audit engagement")
    .map(|row| {
        row.map(|(plan_id, status, version)| LockedEngagement {
            plan_id,
            status,
            version,
        })
    })
}

async fn lock_finding(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    scope: InternalAuditAccessScope,
    finding_id: Uuid,
) -> Result<Option<LockedFinding>> {
    sqlx::query_as::<_, (Uuid, String, i32)>(
        r#"
        SELECT finding.engagement_id, finding.status, finding.version
        FROM internal_audit_findings AS finding
        INNER JOIN internal_audit_engagements AS engagement
            ON engagement.id=finding.engagement_id AND engagement.tenant_id=finding.tenant_id
        WHERE finding.tenant_id=$1 AND finding.id=$2 AND finding.deleted_at IS NULL
          AND engagement.deleted_at IS NULL
          AND ($3::UUID IS NULL OR engagement.lead_auditor_user_id=$3)
        FOR UPDATE OF finding
        "#,
    )
    .bind(tenant_id)
    .bind(finding_id)
    .bind(scope.assigned_user_id())
    .fetch_optional(&mut **tx)
    .await
    .context("Failed to lock audit finding")
    .map(|row| {
        row.map(|(engagement_id, status, version)| LockedFinding {
            engagement_id,
            status,
            version,
        })
    })
}

async fn engagement_exists(
    pool: &PgPool,
    tenant_id: Uuid,
    scope: InternalAuditAccessScope,
    engagement_id: Uuid,
) -> Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM internal_audit_engagements WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL AND ($3::UUID IS NULL OR lead_auditor_user_id=$3))",
    )
    .bind(tenant_id)
    .bind(engagement_id)
    .bind(scope.assigned_user_id())
    .fetch_one(pool)
    .await
    .context("Failed to validate audit engagement access")
}

async fn validate_plan_for_engagement(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    plan_id: Uuid,
    starts_on: chrono::NaiveDate,
    due_on: chrono::NaiveDate,
) -> Result<()> {
    let plan = sqlx::query_as::<_, (String, chrono::NaiveDate, chrono::NaiveDate)>(
        "SELECT status, period_start, period_end FROM internal_audit_plans WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL FOR SHARE",
    )
    .bind(tenant_id)
    .bind(plan_id)
    .fetch_optional(&mut **tx)
    .await
    .context("Failed to validate engagement audit plan")?
    .context("The selected audit plan is unavailable")?;
    ensure_status(
        &plan.0,
        "approved",
        "Engagements require an approved audit plan",
    )?;
    if starts_on < plan.1 || due_on > plan.2 {
        bail!("Engagement dates must fall inside the audit plan period");
    }
    Ok(())
}

async fn validate_auditor(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<()> {
    let eligible = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM users WHERE tenant_id=$1 AND id=$2 AND is_active AND deleted_at IS NULL AND roles && ARRAY['internal_auditor','audit_manager']::TEXT[])",
    )
    .bind(tenant_id)
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await
    .context("Failed to validate engagement lead auditor")?;
    if !eligible {
        bail!("The selected lead must have an active Internal Auditor or Audit Manager role");
    }
    Ok(())
}

fn enforce_lead_assignment(
    scope: InternalAuditAccessScope,
    actor_id: Uuid,
    lead_auditor_user_id: Uuid,
) -> Result<()> {
    if matches!(scope, InternalAuditAccessScope::AssignedTo(_)) && lead_auditor_user_id != actor_id
    {
        bail!("An Internal Auditor can assign only themselves as engagement lead");
    }
    Ok(())
}

async fn reserve_reference(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    kind: SequenceKind,
) -> Result<String> {
    let row = sqlx::query_as::<_, NumberingPolicyRow>(
        r#"
        SELECT plan_prefix, engagement_prefix, finding_prefix, padding,
               next_plan_sequence, next_engagement_sequence, next_finding_sequence,
               version, updated_at
        FROM internal_audit_numbering_policies
        WHERE tenant_id=$1 AND deleted_at IS NULL
        FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .fetch_one(&mut **tx)
    .await
    .context("Failed to lock Internal Audit numbering")?;
    let (prefix, sequence, column) = match kind {
        SequenceKind::Plan => (
            &row.plan_prefix,
            row.next_plan_sequence,
            "next_plan_sequence",
        ),
        SequenceKind::Engagement => (
            &row.engagement_prefix,
            row.next_engagement_sequence,
            "next_engagement_sequence",
        ),
        SequenceKind::Finding => (
            &row.finding_prefix,
            row.next_finding_sequence,
            "next_finding_sequence",
        ),
    };
    let statement = format!(
        "UPDATE internal_audit_numbering_policies SET {column}={column}+1, version=version+1 WHERE tenant_id=$1 AND deleted_at IS NULL"
    );
    sqlx::query(&statement)
        .bind(tenant_id)
        .execute(&mut **tx)
        .await
        .context("Failed to reserve Internal Audit reference")?;
    Ok(format!(
        "{}{:0width$}",
        prefix.trim(),
        sequence,
        width = row.padding as usize
    ))
}

#[allow(clippy::too_many_arguments)]
async fn append_domain_write(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    context: RequestContext,
    aggregate_type: &str,
    aggregate_id: Uuid,
    engagement_id: Option<Uuid>,
    event_type: &str,
    operation_key: &str,
    metadata: Value,
) -> Result<()> {
    let actor_id = actor_user_id(actor)?;
    append_event(
        tx,
        tenant_id,
        aggregate_type,
        aggregate_id,
        engagement_id,
        event_type,
        actor_id,
        metadata.clone(),
    )
    .await?;
    append_shared_audit(
        tx,
        tenant_id,
        actor,
        context,
        operation_key,
        &format!("internal_audit_{aggregate_type}"),
        aggregate_id,
        None,
        metadata,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn append_event(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    aggregate_type: &str,
    aggregate_id: Uuid,
    engagement_id: Option<Uuid>,
    event_type: &str,
    actor_id: Uuid,
    metadata: Value,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO internal_audit_events (
            tenant_id, aggregate_type, aggregate_id, engagement_id,
            event_type, actor_id, metadata
        ) VALUES ($1,$2,$3,$4,$5,$6,$7)
        "#,
    )
    .bind(tenant_id)
    .bind(aggregate_type)
    .bind(aggregate_id)
    .bind(engagement_id)
    .bind(event_type)
    .bind(actor_id)
    .bind(metadata)
    .execute(&mut **tx)
    .await
    .context("Failed to append Internal Audit history")?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn append_shared_audit(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    context: RequestContext,
    operation_key: &str,
    target_kind: &str,
    target_id: Uuid,
    reason: Option<&str>,
    metadata: Value,
) -> Result<()> {
    let mut event = NewAuditEvent::new(
        tenant_id,
        actor,
        operation_key,
        AuditOutcome::Succeeded,
        context,
    )
    .with_target(AuditTarget::new(target_kind, target_id.to_string()))
    .with_redacted_metadata(redacted_object(metadata));
    if let Some(reason) = reason {
        event = event.with_reason(reason);
    }
    append_audit(&mut **tx, &event)
        .await
        .context("Failed to append shared audit evidence")?;
    Ok(())
}

fn redacted_object(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

fn actor_user_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .context("Internal Audit writes require an accountable user")
}

fn validate_dates(
    starts_on: chrono::NaiveDate,
    ends_on: chrono::NaiveDate,
    label: &str,
) -> Result<()> {
    if ends_on < starts_on {
        bail!("{label} end date cannot be before its start date");
    }
    Ok(())
}

fn ensure_status(current: &str, expected: &str, message: &str) -> Result<()> {
    if current != expected {
        bail!(message.to_owned());
    }
    Ok(())
}

fn ensure_version(current: i32, expected: i32, label: &str) -> Result<()> {
    if current != expected {
        bail!("{label} changed. Reload it before continuing");
    }
    Ok(())
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

fn normalized_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "all")
        .map(str::to_lowercase)
}

fn search_pattern(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{}%", value.chars().take(240).collect::<String>()))
}

fn page_bounds(query: &InternalAuditListQuery) -> (i64, i64) {
    let page = query.page.unwrap_or(1).clamp(1, 1_000_000);
    let per_page = query.per_page.unwrap_or(25).clamp(1, 100);
    (per_page, (page - 1) * per_page)
}

fn numbering_response(row: NumberingPolicyRow) -> NumberingPolicyResponse {
    NumberingPolicyResponse {
        next_plan_reference: format_reference(
            &row.plan_prefix,
            row.next_plan_sequence,
            row.padding,
        ),
        next_engagement_reference: format_reference(
            &row.engagement_prefix,
            row.next_engagement_sequence,
            row.padding,
        ),
        next_finding_reference: format_reference(
            &row.finding_prefix,
            row.next_finding_sequence,
            row.padding,
        ),
        plan_prefix: row.plan_prefix,
        engagement_prefix: row.engagement_prefix,
        finding_prefix: row.finding_prefix,
        padding: row.padding,
        next_plan_sequence: row.next_plan_sequence,
        next_engagement_sequence: row.next_engagement_sequence,
        next_finding_sequence: row.next_finding_sequence,
        version: row.version,
        updated_at: row.updated_at,
    }
}

fn format_reference(prefix: &str, sequence: i64, padding: i16) -> String {
    format!(
        "{}{:0width$}",
        prefix.trim(),
        sequence,
        width = padding as usize
    )
}

fn plan_response(row: PlanRow) -> PlanResponse {
    PlanResponse {
        id: row.id,
        reference: row.reference,
        title: row.title,
        objective: row.objective,
        risk_summary: row.risk_summary,
        period_start: row.period_start,
        period_end: row.period_end,
        status: row.status,
        version: row.version,
        engagement_count: row.engagement_count,
        approved_at: row.approved_at,
        closed_at: row.closed_at,
        close_summary: row.close_summary,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn engagement_response(row: EngagementRow) -> EngagementResponse {
    EngagementResponse {
        id: row.id,
        plan_id: row.plan_id,
        plan_reference: row.plan_reference,
        plan_title: row.plan_title,
        reference: row.reference,
        title: row.title,
        objective: row.objective,
        scope_text: row.scope_text,
        lead_auditor_user_id: row.lead_auditor_user_id,
        lead_auditor_name: row.lead_auditor_name,
        lead_auditor_email: row.lead_auditor_email,
        starts_on: row.starts_on,
        due_on: row.due_on,
        status: row.status,
        version: row.version,
        finding_count: row.finding_count,
        evidence_count: row.evidence_count,
        started_at: row.started_at,
        reporting_at: row.reporting_at,
        closed_at: row.closed_at,
        close_summary: row.close_summary,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn evidence_response(row: EvidenceRow) -> EvidenceResponse {
    EvidenceResponse {
        id: row.id,
        engagement_id: row.engagement_id,
        document_file_id: row.document_file_id,
        document_reference: row.document_reference,
        document_title: row.document_title,
        document_sensitivity: row.document_sensitivity,
        purpose: row.purpose,
        linked_at: row.linked_at,
    }
}

fn finding_response(row: FindingRow) -> Result<FindingResponse> {
    let rating = match row.rating.as_str() {
        "low" => FindingRating::Low,
        "moderate" => FindingRating::Moderate,
        "high" => FindingRating::High,
        "critical" => FindingRating::Critical,
        value => bail!("Stored Internal Audit finding rating is invalid: {value}"),
    };
    Ok(FindingResponse {
        id: row.id,
        engagement_id: row.engagement_id,
        engagement_reference: row.engagement_reference,
        engagement_title: row.engagement_title,
        reference: row.reference,
        title: row.title,
        rating,
        criteria: row.criteria,
        condition: row.condition,
        risk_effect: row.risk_effect,
        recommendation: row.recommendation,
        status: row.status,
        version: row.version,
        issued_at: row.issued_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn database_error(error: sqlx::Error, fallback: &str) -> anyhow::Error {
    if let sqlx::Error::Database(database) = &error {
        return match database.constraint() {
            Some("internal_audit_plans_tenant_id_reference_key") => {
                anyhow!("This audit plan reference already exists")
            }
            Some("internal_audit_engagements_tenant_id_reference_key") => {
                anyhow!("This audit engagement reference already exists")
            }
            Some("internal_audit_findings_tenant_id_reference_key") => {
                anyhow!("This audit finding reference already exists")
            }
            Some("internal_audit_evidence_tenant_id_engagement_id_document_file_id_key") => {
                anyhow!("This document is already linked to the audit engagement")
            }
            _ => anyhow!(fallback.to_owned()),
        };
    }
    anyhow!(fallback.to_owned())
}

const NUMBERING_SELECT: &str = r#"
    SELECT plan_prefix, engagement_prefix, finding_prefix, padding,
           next_plan_sequence, next_engagement_sequence, next_finding_sequence,
           version, updated_at
    FROM internal_audit_numbering_policies
    WHERE tenant_id=$1 AND deleted_at IS NULL
"#;

const PLAN_LIST: &str = r#"
    SELECT plan.id, plan.reference, plan.title, plan.objective, plan.risk_summary,
           plan.period_start, plan.period_end, plan.status, plan.version,
           (SELECT COUNT(*) FROM internal_audit_engagements AS engagement
             WHERE engagement.tenant_id=plan.tenant_id AND engagement.plan_id=plan.id
               AND engagement.deleted_at IS NULL) AS engagement_count,
           plan.approved_at, plan.closed_at, plan.close_summary,
           plan.created_at, plan.updated_at
    FROM internal_audit_plans AS plan
    WHERE plan.tenant_id=$1 AND plan.deleted_at IS NULL
      AND ($2::TEXT IS NULL OR plan.status=$2)
      AND ($3::TEXT IS NULL OR plan.reference ILIKE $3 OR plan.title ILIKE $3)
    ORDER BY plan.period_start DESC, plan.reference DESC
    LIMIT $4 OFFSET $5
"#;

const PLAN_COUNT: &str = r#"
    SELECT COUNT(*) FROM internal_audit_plans AS plan
    WHERE plan.tenant_id=$1 AND plan.deleted_at IS NULL
      AND ($2::TEXT IS NULL OR plan.status=$2)
      AND ($3::TEXT IS NULL OR plan.reference ILIKE $3 OR plan.title ILIKE $3)
"#;

const PLAN_BY_ID: &str = r#"
    SELECT plan.id, plan.reference, plan.title, plan.objective, plan.risk_summary,
           plan.period_start, plan.period_end, plan.status, plan.version,
           (SELECT COUNT(*) FROM internal_audit_engagements AS engagement
             WHERE engagement.tenant_id=plan.tenant_id AND engagement.plan_id=plan.id
               AND engagement.deleted_at IS NULL) AS engagement_count,
           plan.approved_at, plan.closed_at, plan.close_summary,
           plan.created_at, plan.updated_at
    FROM internal_audit_plans AS plan
    WHERE plan.tenant_id=$1 AND plan.id=$2 AND plan.deleted_at IS NULL
"#;

const ENGAGEMENT_LIST: &str = r#"
    SELECT engagement.id, engagement.plan_id, plan.reference AS plan_reference,
           plan.title AS plan_title, engagement.reference, engagement.title,
           engagement.objective, engagement.scope_text, engagement.lead_auditor_user_id,
           auditor.full_name AS lead_auditor_name, auditor.email AS lead_auditor_email,
           engagement.starts_on, engagement.due_on, engagement.status, engagement.version,
           (SELECT COUNT(*) FROM internal_audit_findings AS finding
             WHERE finding.tenant_id=engagement.tenant_id
               AND finding.engagement_id=engagement.id AND finding.deleted_at IS NULL) AS finding_count,
           (SELECT COUNT(*) FROM internal_audit_evidence AS evidence
             WHERE evidence.tenant_id=engagement.tenant_id
               AND evidence.engagement_id=engagement.id) AS evidence_count,
           engagement.started_at, engagement.reporting_at, engagement.closed_at,
           engagement.close_summary, engagement.created_at, engagement.updated_at
    FROM internal_audit_engagements AS engagement
    INNER JOIN internal_audit_plans AS plan
        ON plan.id=engagement.plan_id AND plan.tenant_id=engagement.tenant_id
    INNER JOIN users AS auditor
        ON auditor.id=engagement.lead_auditor_user_id AND auditor.tenant_id=engagement.tenant_id
    WHERE engagement.tenant_id=$1 AND engagement.deleted_at IS NULL
      AND ($2::TEXT IS NULL OR engagement.status=$2)
      AND ($3::UUID IS NULL OR engagement.plan_id=$3)
      AND ($4::TEXT IS NULL OR engagement.reference ILIKE $4 OR engagement.title ILIKE $4
           OR plan.reference ILIKE $4 OR auditor.full_name ILIKE $4)
      AND ($5::UUID IS NULL OR engagement.lead_auditor_user_id=$5)
    ORDER BY engagement.due_on, engagement.reference
    LIMIT $6 OFFSET $7
"#;

const ENGAGEMENT_COUNT: &str = r#"
    SELECT COUNT(*)
    FROM internal_audit_engagements AS engagement
    INNER JOIN internal_audit_plans AS plan
        ON plan.id=engagement.plan_id AND plan.tenant_id=engagement.tenant_id
    INNER JOIN users AS auditor
        ON auditor.id=engagement.lead_auditor_user_id AND auditor.tenant_id=engagement.tenant_id
    WHERE engagement.tenant_id=$1 AND engagement.deleted_at IS NULL
      AND ($2::TEXT IS NULL OR engagement.status=$2)
      AND ($3::UUID IS NULL OR engagement.plan_id=$3)
      AND ($4::TEXT IS NULL OR engagement.reference ILIKE $4 OR engagement.title ILIKE $4
           OR plan.reference ILIKE $4 OR auditor.full_name ILIKE $4)
      AND ($5::UUID IS NULL OR engagement.lead_auditor_user_id=$5)
"#;

const ENGAGEMENT_BY_ID: &str = r#"
    SELECT engagement.id, engagement.plan_id, plan.reference AS plan_reference,
           plan.title AS plan_title, engagement.reference, engagement.title,
           engagement.objective, engagement.scope_text, engagement.lead_auditor_user_id,
           auditor.full_name AS lead_auditor_name, auditor.email AS lead_auditor_email,
           engagement.starts_on, engagement.due_on, engagement.status, engagement.version,
           (SELECT COUNT(*) FROM internal_audit_findings AS finding
             WHERE finding.tenant_id=engagement.tenant_id
               AND finding.engagement_id=engagement.id AND finding.deleted_at IS NULL) AS finding_count,
           (SELECT COUNT(*) FROM internal_audit_evidence AS evidence
             WHERE evidence.tenant_id=engagement.tenant_id
               AND evidence.engagement_id=engagement.id) AS evidence_count,
           engagement.started_at, engagement.reporting_at, engagement.closed_at,
           engagement.close_summary, engagement.created_at, engagement.updated_at
    FROM internal_audit_engagements AS engagement
    INNER JOIN internal_audit_plans AS plan
        ON plan.id=engagement.plan_id AND plan.tenant_id=engagement.tenant_id
    INNER JOIN users AS auditor
        ON auditor.id=engagement.lead_auditor_user_id AND auditor.tenant_id=engagement.tenant_id
    WHERE engagement.tenant_id=$1 AND engagement.id=$2 AND engagement.deleted_at IS NULL
      AND ($3::UUID IS NULL OR engagement.lead_auditor_user_id=$3)
"#;

const FINDING_LIST: &str = r#"
    SELECT finding.id, finding.engagement_id,
           engagement.reference AS engagement_reference,
           engagement.title AS engagement_title,
           finding.reference, finding.title, finding.rating, finding.criteria,
           finding.condition, finding.risk_effect, finding.recommendation,
           finding.status, finding.version, finding.issued_at,
           finding.created_at, finding.updated_at
    FROM internal_audit_findings AS finding
    INNER JOIN internal_audit_engagements AS engagement
        ON engagement.id=finding.engagement_id AND engagement.tenant_id=finding.tenant_id
    WHERE finding.tenant_id=$1 AND finding.deleted_at IS NULL
      AND engagement.deleted_at IS NULL
      AND ($2::TEXT IS NULL OR finding.status=$2)
      AND ($3::TEXT IS NULL OR finding.rating=$3)
      AND ($4::UUID IS NULL OR finding.engagement_id=$4)
      AND ($5::TEXT IS NULL OR finding.reference ILIKE $5 OR finding.title ILIKE $5
           OR engagement.reference ILIKE $5 OR engagement.title ILIKE $5)
      AND ($6::UUID IS NULL OR engagement.lead_auditor_user_id=$6)
    ORDER BY CASE finding.rating WHEN 'critical' THEN 1 WHEN 'high' THEN 2
             WHEN 'moderate' THEN 3 ELSE 4 END,
             finding.created_at DESC
    LIMIT $7 OFFSET $8
"#;

const FINDING_COUNT: &str = r#"
    SELECT COUNT(*)
    FROM internal_audit_findings AS finding
    INNER JOIN internal_audit_engagements AS engagement
        ON engagement.id=finding.engagement_id AND engagement.tenant_id=finding.tenant_id
    WHERE finding.tenant_id=$1 AND finding.deleted_at IS NULL
      AND engagement.deleted_at IS NULL
      AND ($2::TEXT IS NULL OR finding.status=$2)
      AND ($3::TEXT IS NULL OR finding.rating=$3)
      AND ($4::UUID IS NULL OR finding.engagement_id=$4)
      AND ($5::TEXT IS NULL OR finding.reference ILIKE $5 OR finding.title ILIKE $5
           OR engagement.reference ILIKE $5 OR engagement.title ILIKE $5)
      AND ($6::UUID IS NULL OR engagement.lead_auditor_user_id=$6)
"#;

const FINDING_BY_ID: &str = r#"
    SELECT finding.id, finding.engagement_id,
           engagement.reference AS engagement_reference,
           engagement.title AS engagement_title,
           finding.reference, finding.title, finding.rating, finding.criteria,
           finding.condition, finding.risk_effect, finding.recommendation,
           finding.status, finding.version, finding.issued_at,
           finding.created_at, finding.updated_at
    FROM internal_audit_findings AS finding
    INNER JOIN internal_audit_engagements AS engagement
        ON engagement.id=finding.engagement_id AND engagement.tenant_id=finding.tenant_id
    WHERE finding.tenant_id=$1 AND finding.id=$2 AND finding.deleted_at IS NULL
      AND engagement.deleted_at IS NULL
      AND ($3::UUID IS NULL OR engagement.lead_auditor_user_id=$3)
"#;

impl InternalAuditOps {
    pub async fn update_engagement(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: InternalAuditAccessScope,
        engagement_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &UpdateEngagementRequest,
    ) -> Result<Option<EngagementResponse>> {
        validate_dates(request.starts_on, request.due_on, "Audit engagement")?;
        let actor_id = actor_user_id(actor)?;
        enforce_lead_assignment(scope, actor_id, request.lead_auditor_user_id)?;
        let mut tx = pool
            .begin()
            .await
            .context("Failed to start engagement update")?;
        let Some(current) = lock_engagement(&mut tx, tenant_id, scope, engagement_id).await? else {
            return Ok(None);
        };
        ensure_status(
            &current.status,
            "planned",
            "Only a planned audit engagement can be changed",
        )?;
        ensure_version(
            current.version,
            request.expected_version,
            "Audit engagement",
        )?;
        validate_plan_for_engagement(
            &mut tx,
            tenant_id,
            current.plan_id,
            request.starts_on,
            request.due_on,
        )
        .await?;
        validate_auditor(&mut tx, tenant_id, request.lead_auditor_user_id).await?;
        sqlx::query(
            r#"
            UPDATE internal_audit_engagements
            SET title=$3, objective=$4, scope_text=$5, lead_auditor_user_id=$6,
                starts_on=$7, due_on=$8, updated_by=$9, version=version+1
            WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(engagement_id)
        .bind(trimmed_required(&request.title, "Engagement title")?)
        .bind(trimmed_required(
            &request.objective,
            "Engagement objective",
        )?)
        .bind(trimmed_required(&request.scope_text, "Engagement scope")?)
        .bind(request.lead_auditor_user_id)
        .bind(request.starts_on)
        .bind(request.due_on)
        .bind(actor_id)
        .execute(&mut *tx)
        .await
        .context("Failed to update audit engagement")?;
        append_domain_write(
            &mut tx,
            tenant_id,
            actor,
            context,
            "engagement",
            engagement_id,
            Some(engagement_id),
            "internal_audit.engagement.updated",
            "internal_audit.engagements.update",
            json!({"version": current.version + 1,"lead_auditor_user_id":request.lead_auditor_user_id}),
        )
        .await?;
        tx.commit()
            .await
            .context("Failed to commit engagement update")?;
        Self::get_engagement(pool, tenant_id, scope, engagement_id).await
    }

    pub async fn start_engagement(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: InternalAuditAccessScope,
        engagement_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<EngagementResponse>> {
        transition_engagement(
            pool,
            tenant_id,
            scope,
            engagement_id,
            actor,
            context,
            expected_version,
            EngagementTransition::Start,
            None,
        )
        .await
    }

    pub async fn begin_reporting(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: InternalAuditAccessScope,
        engagement_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<EngagementResponse>> {
        transition_engagement(
            pool,
            tenant_id,
            scope,
            engagement_id,
            actor,
            context,
            expected_version,
            EngagementTransition::BeginReporting,
            None,
        )
        .await
    }

    pub async fn close_engagement(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: InternalAuditAccessScope,
        engagement_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &CloseRequest,
    ) -> Result<Option<EngagementResponse>> {
        transition_engagement(
            pool,
            tenant_id,
            scope,
            engagement_id,
            actor,
            context,
            request.expected_version,
            EngagementTransition::Close,
            Some(trimmed_required(&request.summary, "Closure summary")?.to_owned()),
        )
        .await
    }

    pub async fn delete_engagement(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: InternalAuditAccessScope,
        engagement_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        expected_version: i32,
    ) -> Result<bool> {
        let actor_id = actor_user_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("Failed to start engagement deletion")?;
        let Some(current) = lock_engagement(&mut tx, tenant_id, scope, engagement_id).await? else {
            return Ok(false);
        };
        ensure_status(
            &current.status,
            "planned",
            "Only a planned audit engagement can be deleted",
        )?;
        ensure_version(current.version, expected_version, "Audit engagement")?;
        let has_children = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM internal_audit_findings WHERE tenant_id=$1 AND engagement_id=$2 AND deleted_at IS NULL) OR EXISTS(SELECT 1 FROM internal_audit_evidence WHERE tenant_id=$1 AND engagement_id=$2)",
        )
        .bind(tenant_id)
        .bind(engagement_id)
        .fetch_one(&mut *tx)
        .await
        .context("Failed to validate engagement deletion")?;
        if has_children {
            bail!("An audit engagement with findings or evidence cannot be deleted");
        }
        sqlx::query(
            "UPDATE internal_audit_engagements SET deleted_at=NOW(), updated_by=$3, version=version+1 WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(engagement_id)
        .bind(actor_id)
        .execute(&mut *tx)
        .await
        .context("Failed to delete audit engagement")?;
        append_domain_write(
            &mut tx,
            tenant_id,
            actor,
            context,
            "engagement",
            engagement_id,
            Some(engagement_id),
            "internal_audit.engagement.deleted",
            "internal_audit.engagements.delete",
            json!({"version": current.version + 1}),
        )
        .await?;
        tx.commit()
            .await
            .context("Failed to commit engagement deletion")?;
        Ok(true)
    }

    pub async fn list_evidence(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: InternalAuditAccessScope,
        engagement_id: Uuid,
    ) -> Result<Vec<EvidenceResponse>> {
        if !engagement_exists(pool, tenant_id, scope, engagement_id).await? {
            return Ok(Vec::new());
        }
        sqlx::query_as::<_, EvidenceRow>(
            r#"
            SELECT id, engagement_id, document_file_id,
                   document_reference_snapshot AS document_reference,
                   document_title_snapshot AS document_title,
                   document_sensitivity_snapshot AS document_sensitivity,
                   purpose, created_at AS linked_at
            FROM internal_audit_evidence
            WHERE tenant_id=$1 AND engagement_id=$2
            ORDER BY created_at DESC, id
            "#,
        )
        .bind(tenant_id)
        .bind(engagement_id)
        .fetch_all(pool)
        .await
        .context("Failed to list audit evidence")
        .map(|rows| rows.into_iter().map(evidence_response).collect())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn link_evidence(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: InternalAuditAccessScope,
        can_view_restricted: bool,
        engagement_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &LinkEvidenceRequest,
    ) -> Result<Option<EvidenceResponse>> {
        let actor_id = actor_user_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("Failed to start evidence link")?;
        let Some(current) = lock_engagement(&mut tx, tenant_id, scope, engagement_id).await? else {
            return Ok(None);
        };
        if !matches!(current.status.as_str(), "fieldwork" | "reporting") {
            bail!("Evidence can be linked only during fieldwork or reporting");
        }
        let document = DocumentRegistryOps::evidence_reference(
            &mut *tx,
            tenant_id,
            request.document_file_id,
            can_view_restricted,
        )
        .await?
        .context("The governed document is unavailable for evidence linking")?;
        let evidence_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO internal_audit_evidence (
                id, tenant_id, engagement_id, document_file_id,
                document_reference_snapshot, document_title_snapshot,
                document_sensitivity_snapshot, purpose, linked_by
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
            "#,
        )
        .bind(evidence_id)
        .bind(tenant_id)
        .bind(engagement_id)
        .bind(document.id)
        .bind(&document.reference)
        .bind(&document.title)
        .bind(&document.sensitivity)
        .bind(trimmed_required(&request.purpose, "Evidence purpose")?)
        .bind(actor_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| database_error(error, "Failed to link audit evidence"))?;
        append_domain_write(
            &mut tx,
            tenant_id,
            actor,
            context,
            "evidence",
            evidence_id,
            Some(engagement_id),
            "internal_audit.evidence.linked",
            "internal_audit.evidence.create",
            json!({"document_file_id":document.id,"document_reference":document.reference}),
        )
        .await?;
        tx.commit()
            .await
            .context("Failed to commit evidence link")?;
        sqlx::query_as::<_, EvidenceRow>(
            r#"
            SELECT id, engagement_id, document_file_id,
                   document_reference_snapshot AS document_reference,
                   document_title_snapshot AS document_title,
                   document_sensitivity_snapshot AS document_sensitivity,
                   purpose, created_at AS linked_at
            FROM internal_audit_evidence
            WHERE tenant_id=$1 AND id=$2
            "#,
        )
        .bind(tenant_id)
        .bind(evidence_id)
        .fetch_optional(pool)
        .await
        .context("Failed to reload linked audit evidence")
        .map(|row| row.map(evidence_response))
    }

    pub async fn list_findings(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: InternalAuditAccessScope,
        query: &InternalAuditListQuery,
    ) -> Result<(Vec<FindingResponse>, i64)> {
        let (limit, offset) = page_bounds(query);
        let search = search_pattern(query.search.as_deref());
        let status = normalized_filter(query.status.as_deref());
        let rating = query.rating.map(FindingRating::as_str);
        let assigned_user_id = scope.assigned_user_id();
        let rows = sqlx::query_as::<_, FindingRow>(FINDING_LIST)
            .bind(tenant_id)
            .bind(status.as_deref())
            .bind(rating)
            .bind(query.engagement_id)
            .bind(search.as_deref())
            .bind(assigned_user_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .context("Failed to list audit findings")?;
        let total = sqlx::query_scalar::<_, i64>(FINDING_COUNT)
            .bind(tenant_id)
            .bind(status.as_deref())
            .bind(rating)
            .bind(query.engagement_id)
            .bind(search.as_deref())
            .bind(assigned_user_id)
            .fetch_one(pool)
            .await
            .context("Failed to count audit findings")?;
        let findings = rows
            .into_iter()
            .map(finding_response)
            .collect::<Result<Vec<_>>>()?;
        Ok((findings, total))
    }

    pub async fn get_finding(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: InternalAuditAccessScope,
        finding_id: Uuid,
    ) -> Result<Option<FindingResponse>> {
        let row = sqlx::query_as::<_, FindingRow>(FINDING_BY_ID)
            .bind(tenant_id)
            .bind(finding_id)
            .bind(scope.assigned_user_id())
            .fetch_optional(pool)
            .await
            .context("Failed to load audit finding")?;
        row.map(finding_response).transpose()
    }

    pub async fn create_finding(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: InternalAuditAccessScope,
        engagement_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &CreateFindingRequest,
    ) -> Result<Option<FindingResponse>> {
        let actor_id = actor_user_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("Failed to start finding creation")?;
        let Some(current) = lock_engagement(&mut tx, tenant_id, scope, engagement_id).await? else {
            return Ok(None);
        };
        if !matches!(current.status.as_str(), "fieldwork" | "reporting") {
            bail!("Findings can be drafted only during fieldwork or reporting");
        }
        let reference = reserve_reference(&mut tx, tenant_id, SequenceKind::Finding).await?;
        let finding_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO internal_audit_findings (
                id, tenant_id, engagement_id, reference, title, rating, criteria,
                condition, risk_effect, recommendation, created_by, updated_by
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$11)
            "#,
        )
        .bind(finding_id)
        .bind(tenant_id)
        .bind(engagement_id)
        .bind(&reference)
        .bind(trimmed_required(&request.title, "Finding title")?)
        .bind(request.rating.as_str())
        .bind(trimmed_required(&request.criteria, "Finding criteria")?)
        .bind(trimmed_required(&request.condition, "Finding condition")?)
        .bind(trimmed_required(&request.risk_effect, "Risk effect")?)
        .bind(trimmed_required(
            &request.recommendation,
            "Finding recommendation",
        )?)
        .bind(actor_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| database_error(error, "Failed to create audit finding"))?;
        append_domain_write(
            &mut tx,
            tenant_id,
            actor,
            context,
            "finding",
            finding_id,
            Some(engagement_id),
            "internal_audit.finding.created",
            "internal_audit.findings.create",
            json!({"reference":reference,"rating":request.rating.as_str()}),
        )
        .await?;
        tx.commit()
            .await
            .context("Failed to commit audit finding")?;
        Self::get_finding(pool, tenant_id, scope, finding_id).await
    }

    pub async fn update_finding(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: InternalAuditAccessScope,
        finding_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        request: &UpdateFindingRequest,
    ) -> Result<Option<FindingResponse>> {
        let actor_id = actor_user_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("Failed to start finding update")?;
        let Some(current) = lock_finding(&mut tx, tenant_id, scope, finding_id).await? else {
            return Ok(None);
        };
        ensure_status(
            &current.status,
            "draft",
            "Only a draft audit finding can be changed",
        )?;
        ensure_version(current.version, request.expected_version, "Audit finding")?;
        sqlx::query(
            r#"
            UPDATE internal_audit_findings
            SET title=$3, rating=$4, criteria=$5, condition=$6, risk_effect=$7,
                recommendation=$8, updated_by=$9, version=version+1
            WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(finding_id)
        .bind(trimmed_required(&request.title, "Finding title")?)
        .bind(request.rating.as_str())
        .bind(trimmed_required(&request.criteria, "Finding criteria")?)
        .bind(trimmed_required(&request.condition, "Finding condition")?)
        .bind(trimmed_required(&request.risk_effect, "Risk effect")?)
        .bind(trimmed_required(
            &request.recommendation,
            "Finding recommendation",
        )?)
        .bind(actor_id)
        .execute(&mut *tx)
        .await
        .context("Failed to update audit finding")?;
        append_domain_write(
            &mut tx,
            tenant_id,
            actor,
            context,
            "finding",
            finding_id,
            Some(current.engagement_id),
            "internal_audit.finding.updated",
            "internal_audit.findings.update",
            json!({"version":current.version + 1,"rating":request.rating.as_str()}),
        )
        .await?;
        tx.commit()
            .await
            .context("Failed to commit finding update")?;
        Self::get_finding(pool, tenant_id, scope, finding_id).await
    }

    pub async fn issue_finding(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: InternalAuditAccessScope,
        finding_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<FindingResponse>> {
        let actor_id = actor_user_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("Failed to start finding issuance")?;
        let Some(current) = lock_finding(&mut tx, tenant_id, scope, finding_id).await? else {
            return Ok(None);
        };
        ensure_status(
            &current.status,
            "draft",
            "Only a draft finding can be issued",
        )?;
        ensure_version(current.version, expected_version, "Audit finding")?;
        let engagement_status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM internal_audit_engagements WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL FOR UPDATE",
        )
        .bind(tenant_id)
        .bind(current.engagement_id)
        .fetch_one(&mut *tx)
        .await
        .context("Failed to validate finding engagement")?;
        ensure_status(
            &engagement_status,
            "reporting",
            "Findings can be issued only while the engagement is in reporting",
        )?;
        sqlx::query(
            "UPDATE internal_audit_findings SET status='issued', issued_by=$3, issued_at=NOW(), updated_by=$3, version=version+1 WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(finding_id)
        .bind(actor_id)
        .execute(&mut *tx)
        .await
        .context("Failed to issue audit finding")?;
        append_domain_write(
            &mut tx,
            tenant_id,
            actor,
            context,
            "finding",
            finding_id,
            Some(current.engagement_id),
            "internal_audit.finding.issued",
            "internal_audit.findings.issue",
            json!({"version":current.version + 1}),
        )
        .await?;
        tx.commit()
            .await
            .context("Failed to commit finding issuance")?;
        Self::get_finding(pool, tenant_id, scope, finding_id).await
    }

    pub async fn delete_finding(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: InternalAuditAccessScope,
        finding_id: Uuid,
        actor: AuditActor,
        context: RequestContext,
        expected_version: i32,
    ) -> Result<bool> {
        let actor_id = actor_user_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("Failed to start finding deletion")?;
        let Some(current) = lock_finding(&mut tx, tenant_id, scope, finding_id).await? else {
            return Ok(false);
        };
        ensure_status(
            &current.status,
            "draft",
            "Only a draft audit finding can be deleted",
        )?;
        ensure_version(current.version, expected_version, "Audit finding")?;
        sqlx::query(
            "UPDATE internal_audit_findings SET deleted_at=NOW(), updated_by=$3, version=version+1 WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(finding_id)
        .bind(actor_id)
        .execute(&mut *tx)
        .await
        .context("Failed to delete audit finding")?;
        append_domain_write(
            &mut tx,
            tenant_id,
            actor,
            context,
            "finding",
            finding_id,
            Some(current.engagement_id),
            "internal_audit.finding.deleted",
            "internal_audit.findings.delete",
            json!({"version":current.version + 1}),
        )
        .await?;
        tx.commit()
            .await
            .context("Failed to commit finding deletion")?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::{FindingRating, ensure_version, format_reference, validate_dates};

    #[test]
    fn references_use_tenant_padding_without_copying_display_state() {
        assert_eq!(format_reference("AUD-", 42, 6), "AUD-000042");
    }

    #[test]
    fn date_ranges_and_versions_fail_closed() {
        assert!(
            validate_dates(
                NaiveDate::from_ymd_opt(2026, 9, 2).unwrap_or_else(|| unreachable!()),
                NaiveDate::from_ymd_opt(2026, 9, 1).unwrap_or_else(|| unreachable!()),
                "Audit plan"
            )
            .is_err()
        );
        assert!(ensure_version(3, 2, "Audit plan").is_err());
    }

    #[test]
    fn ratings_serialize_to_the_persisted_closed_vocabulary() {
        assert_eq!(FindingRating::Critical.as_str(), "critical");
        assert_eq!(FindingRating::Moderate.as_str(), "moderate");
    }
}
