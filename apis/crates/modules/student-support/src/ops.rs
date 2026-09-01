//! Transactional Student Support workflows and restricted read projections.
//!
//! Every write re-locks the case through its current campus or case-team scope.
//! Actions and lifecycle evidence append in the same transaction as the change.

use std::collections::HashMap;

use anyhow::{Context, Result, anyhow, bail};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_sis::ops::LearnerOps;
use serde_json::{Map, Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    AssignTeamMemberRequest, CaseActionResponse, CaseEventResponse, CaseRecordResponse,
    CaseSummaryResponse, CaseTeamMemberResponse, CaseTransitionRequest,
    CaseWorkerCandidateResponse, CreateCaseActionRequest, CreateCaseRequest,
    LearnerCandidateResponse, ReferenceQuery, StudentSupportAccessScope, StudentSupportListQuery,
    StudentSupportReferenceData, UpdateCaseRequest,
    models::{ActionRow, CaseRow, EventRow, TeamMemberRow},
};

/// Tenant-scoped Student Support domain operations.
pub struct StudentSupportOps;

impl StudentSupportOps {
    pub async fn reference_data(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &ReferenceQuery,
        include_case_workers: bool,
    ) -> Result<StudentSupportReferenceData> {
        let learners =
            LearnerOps::student_support_references(pool, tenant_id, query.search.as_deref(), 100)
                .await?
                .into_iter()
                .map(|learner| LearnerCandidateResponse {
                    learner_id: learner.id,
                    learner_number: learner.learner_number,
                    display_name: learner.display_name,
                    status: learner.status,
                })
                .collect();
        let case_workers = if include_case_workers {
            case_worker_candidates(pool, tenant_id, query.search.as_deref()).await?
        } else {
            Vec::new()
        };
        Ok(StudentSupportReferenceData {
            learners,
            case_workers,
        })
    }

    pub async fn list_cases(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: StudentSupportAccessScope,
        query: &StudentSupportListQuery,
    ) -> Result<(Vec<CaseSummaryResponse>, i64)> {
        let (page, per_page) = bounded_page(query);
        let offset = (page - 1) * per_page;
        let search = search_pattern(query.search.as_deref());
        let learner_ids = if query
            .search
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        {
            Some(
                LearnerOps::student_support_references(
                    pool,
                    tenant_id,
                    query.search.as_deref(),
                    100,
                )
                .await?
                .into_iter()
                .map(|learner| learner.id)
                .collect::<Vec<_>>(),
            )
        } else {
            None
        };
        let assigned_user_id = scope.assigned_user_id();
        let rows = sqlx::query_as::<_, CaseRow>(CASE_LIST)
            .bind(tenant_id)
            .bind(query.status.map(|value| value.as_str()))
            .bind(query.category.map(|value| value.as_str()))
            .bind(query.severity.map(|value| value.as_str()))
            .bind(query.learner_id)
            .bind(search.as_deref())
            .bind(learner_ids.as_deref())
            .bind(assigned_user_id)
            .bind(per_page)
            .bind(offset)
            .fetch_all(pool)
            .await
            .context("Failed to list Student Support cases")?;
        let total = sqlx::query_scalar::<_, i64>(CASE_COUNT)
            .bind(tenant_id)
            .bind(query.status.map(|value| value.as_str()))
            .bind(query.category.map(|value| value.as_str()))
            .bind(query.severity.map(|value| value.as_str()))
            .bind(query.learner_id)
            .bind(search.as_deref())
            .bind(learner_ids.as_deref())
            .bind(assigned_user_id)
            .fetch_one(pool)
            .await
            .context("Failed to count Student Support cases")?;
        Ok((hydrate_case_summaries(pool, tenant_id, rows).await?, total))
    }

    pub async fn create_case(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: StudentSupportAccessScope,
        actor: AuditActor,
        context: RequestContext,
        request: &CreateCaseRequest,
    ) -> Result<CaseRecordResponse> {
        let actor_id = person_actor_id(actor)?;
        let lead_id = request.lead_case_worker_user_id.unwrap_or(actor_id);
        if !scope.is_campus() && lead_id != actor_id {
            bail!("A Case Worker can open a case only with themselves as lead");
        }
        validate_learner(pool, tenant_id, request.learner_id).await?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start Student Support case creation")?;
        validate_case_worker(&mut transaction, tenant_id, lead_id).await?;
        let reference = reserve_case_reference(&mut transaction, tenant_id).await?;
        let case_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO student_support_cases (
                id, tenant_id, reference, learner_id, lead_case_worker_user_id,
                category, severity, title, summary, occurred_on, created_by, updated_by
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$11)
            "#,
        )
        .bind(case_id)
        .bind(tenant_id)
        .bind(&reference)
        .bind(request.learner_id)
        .bind(lead_id)
        .bind(request.category.as_str())
        .bind(request.severity.as_str())
        .bind(trimmed_required(&request.title, "Case title")?)
        .bind(trimmed_required(&request.summary, "Case summary")?)
        .bind(request.occurred_on)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to create Student Support case")?;
        append_case_evidence(
            &mut transaction,
            CaseEvidence {
                tenant_id,
                case_id,
                actor,
                context,
                event_type: "student_support.case.created",
                operation: "student_support.cases.create",
                metadata: json!({
                    "reference": reference,
                    "learner_id": request.learner_id,
                    "category": request.category.as_str(),
                    "severity": request.severity.as_str()
                }),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit Student Support case creation")?;
        Self::get_case(pool, tenant_id, case_id, scope)
            .await?
            .ok_or_else(|| anyhow!("The Student Support case could not be reloaded"))
    }

    pub async fn get_case(
        pool: &PgPool,
        tenant_id: Uuid,
        case_id: Uuid,
        scope: StudentSupportAccessScope,
    ) -> Result<Option<CaseRecordResponse>> {
        let Some(row) = case_row_by_id(pool, tenant_id, case_id, scope).await? else {
            return Ok(None);
        };
        let mut summaries = hydrate_case_summaries(pool, tenant_id, vec![row]).await?;
        let summary = summaries
            .pop()
            .ok_or_else(|| anyhow!("The Student Support learner identity is unavailable"))?;
        let row = case_row_by_id(pool, tenant_id, case_id, scope)
            .await?
            .ok_or_else(|| anyhow!("The Student Support case changed while loading"))?;
        let mut team = vec![CaseTeamMemberResponse {
            user_id: row.lead_case_worker_user_id,
            full_name: row.lead_case_worker_name,
            email: row.lead_case_worker_email,
            member_role: "lead".to_string(),
            assigned_at: row.created_at,
        }];
        team.extend(
            sqlx::query_as::<_, TeamMemberRow>(
                r#"
                SELECT member.user_id, account.full_name, account.email,
                       member.member_role, member.created_at AS assigned_at
                  FROM student_support_case_team_members AS member
                  JOIN users AS account
                    ON account.id=member.user_id AND account.tenant_id=member.tenant_id
                 WHERE member.tenant_id=$1 AND member.case_id=$2
                   AND member.deleted_at IS NULL
                 ORDER BY member.member_role, account.full_name, account.email
                "#,
            )
            .bind(tenant_id)
            .bind(case_id)
            .fetch_all(pool)
            .await
            .context("Failed to load Student Support case team")?
            .into_iter()
            .map(team_member_response),
        );
        let history = sqlx::query_as::<_, EventRow>(
            r#"
            SELECT event.id, event.case_id, event.event_type, event.actor_id,
                   account.full_name AS actor_name, event.metadata, event.created_at
              FROM student_support_case_events AS event
              JOIN users AS account
                ON account.id=event.actor_id AND account.tenant_id=event.tenant_id
             WHERE event.tenant_id=$1 AND event.case_id=$2
             ORDER BY event.created_at DESC, event.id DESC
             LIMIT 200
            "#,
        )
        .bind(tenant_id)
        .bind(case_id)
        .fetch_all(pool)
        .await
        .context("Failed to load Student Support case history")?
        .into_iter()
        .map(event_response)
        .collect();
        Ok(Some(CaseRecordResponse {
            case: summary,
            summary: row.summary,
            escalation_reason: row.escalation_reason,
            escalated_at: row.escalated_at,
            resolution_summary: row.resolution_summary,
            resolved_at: row.resolved_at,
            closure_reason: row.closure_reason,
            closed_at: row.closed_at,
            team,
            history,
            created_at: row.created_at,
        }))
    }

    pub async fn update_case(
        pool: &PgPool,
        tenant_id: Uuid,
        case_id: Uuid,
        scope: StudentSupportAccessScope,
        actor: AuditActor,
        context: RequestContext,
        request: &UpdateCaseRequest,
    ) -> Result<Option<CaseRecordResponse>> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool.begin().await.context("Failed to start case update")?;
        let Some((status, version, _)) =
            lock_scoped_case(&mut transaction, tenant_id, case_id, scope).await?
        else {
            return Ok(None);
        };
        ensure_version(version, request.expected_version)?;
        if !matches!(status.as_str(), "open" | "active" | "escalated") {
            bail!("Resolved or closed cases cannot be edited");
        }
        let next_status = if status == "open" {
            "active"
        } else {
            status.as_str()
        };
        sqlx::query(
            r#"
            UPDATE student_support_cases
               SET category=$3, severity=$4, title=$5, summary=$6, occurred_on=$7,
                   status=$8, version=version+1, updated_by=$9
             WHERE tenant_id=$1 AND id=$2
            "#,
        )
        .bind(tenant_id)
        .bind(case_id)
        .bind(request.category.as_str())
        .bind(request.severity.as_str())
        .bind(trimmed_required(&request.title, "Case title")?)
        .bind(trimmed_required(&request.summary, "Case summary")?)
        .bind(request.occurred_on)
        .bind(next_status)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to update Student Support case")?;
        append_case_evidence(
            &mut transaction,
            CaseEvidence {
                tenant_id,
                case_id,
                actor,
                context,
                event_type: "student_support.case.updated",
                operation: "student_support.cases.update",
                metadata: json!({"version": version + 1, "status": next_status, "severity": request.severity.as_str()}),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit case update")?;
        Self::get_case(pool, tenant_id, case_id, scope).await
    }

    pub async fn list_actions(
        pool: &PgPool,
        tenant_id: Uuid,
        case_id: Uuid,
        scope: StudentSupportAccessScope,
    ) -> Result<Option<Vec<CaseActionResponse>>> {
        if case_row_by_id(pool, tenant_id, case_id, scope)
            .await?
            .is_none()
        {
            return Ok(None);
        }
        let actions = sqlx::query_as::<_, ActionRow>(ACTION_SELECT)
            .bind(tenant_id)
            .bind(case_id)
            .fetch_all(pool)
            .await
            .context("Failed to list Student Support actions")?
            .into_iter()
            .map(action_response)
            .collect();
        Ok(Some(actions))
    }

    pub async fn create_action(
        pool: &PgPool,
        tenant_id: Uuid,
        case_id: Uuid,
        scope: StudentSupportAccessScope,
        actor: AuditActor,
        context: RequestContext,
        request: &CreateCaseActionRequest,
    ) -> Result<Option<CaseActionResponse>> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start action creation")?;
        let Some((status, version, _)) =
            lock_scoped_case(&mut transaction, tenant_id, case_id, scope).await?
        else {
            return Ok(None);
        };
        ensure_version(version, request.expected_version)?;
        if !matches!(status.as_str(), "open" | "active" | "escalated") {
            bail!("Actions can be added only to open cases");
        }
        let action_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO student_support_case_actions (
                id, tenant_id, case_id, action_kind, summary, details, occurred_at, created_by
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
            "#,
        )
        .bind(action_id)
        .bind(tenant_id)
        .bind(case_id)
        .bind(request.action_kind.as_str())
        .bind(trimmed_required(&request.summary, "Action summary")?)
        .bind(trimmed_optional(request.details.as_deref()))
        .bind(request.occurred_at)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to create Student Support action")?;
        sqlx::query(
            "UPDATE student_support_cases SET status=CASE WHEN status='open' THEN 'active' ELSE status END, version=version+1, updated_by=$3 WHERE tenant_id=$1 AND id=$2",
        )
        .bind(tenant_id)
        .bind(case_id)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to advance Student Support case")?;
        append_case_evidence(
            &mut transaction,
            CaseEvidence {
                tenant_id,
                case_id,
                actor,
                context,
                event_type: "student_support.case.action_added",
                operation: "student_support.actions.create",
                metadata: json!({"action_id": action_id, "action_kind": request.action_kind.as_str(), "version": version + 1}),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit case action")?;
        let action = sqlx::query_as::<_, ActionRow>(ACTION_BY_ID)
            .bind(tenant_id)
            .bind(action_id)
            .fetch_one(pool)
            .await
            .context("Failed to reload Student Support action")?;
        Ok(Some(action_response(action)))
    }

    pub async fn assign_team_member(
        pool: &PgPool,
        tenant_id: Uuid,
        case_id: Uuid,
        scope: StudentSupportAccessScope,
        actor: AuditActor,
        context: RequestContext,
        request: &AssignTeamMemberRequest,
    ) -> Result<Option<CaseRecordResponse>> {
        ensure_manager_scope(scope)?;
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start team assignment")?;
        let Some((status, version, lead_id)) =
            lock_scoped_case(&mut transaction, tenant_id, case_id, scope).await?
        else {
            return Ok(None);
        };
        ensure_version(version, request.expected_version)?;
        if status == "closed" {
            bail!("A closed case team cannot be changed");
        }
        if request.user_id == lead_id {
            bail!("The lead Case Worker is already on the case team");
        }
        validate_case_worker(&mut transaction, tenant_id, request.user_id).await?;
        sqlx::query(
            r#"
            INSERT INTO student_support_case_team_members (
                tenant_id, case_id, user_id, member_role, assigned_by
            ) VALUES ($1,$2,$3,$4,$5)
            "#,
        )
        .bind(tenant_id)
        .bind(case_id)
        .bind(request.user_id)
        .bind(request.member_role.as_str())
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            database_error(error, "The selected account is already on this case team")
        })?;
        bump_case_version(&mut transaction, tenant_id, case_id, actor_id).await?;
        append_case_evidence(
            &mut transaction,
            CaseEvidence {
                tenant_id,
                case_id,
                actor,
                context,
                event_type: "student_support.case.team_member_assigned",
                operation: "student_support.case_team.assign",
                metadata: json!({"user_id": request.user_id, "member_role": request.member_role.as_str(), "version": version + 1}),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit team assignment")?;
        Self::get_case(pool, tenant_id, case_id, scope).await
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "removal authority and optimistic version are explicit"
    )]
    pub async fn remove_team_member(
        pool: &PgPool,
        tenant_id: Uuid,
        case_id: Uuid,
        user_id: Uuid,
        scope: StudentSupportAccessScope,
        actor: AuditActor,
        context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<CaseRecordResponse>> {
        ensure_manager_scope(scope)?;
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool.begin().await.context("Failed to start team removal")?;
        let Some((status, version, lead_id)) =
            lock_scoped_case(&mut transaction, tenant_id, case_id, scope).await?
        else {
            return Ok(None);
        };
        ensure_version(version, expected_version)?;
        if status == "closed" {
            bail!("A closed case team cannot be changed");
        }
        if user_id == lead_id {
            bail!("The case lead cannot be removed");
        }
        let changed = sqlx::query(
            r#"
            UPDATE student_support_case_team_members
               SET removed_by=$4, removed_at=NOW(), deleted_at=NOW()
             WHERE tenant_id=$1 AND case_id=$2 AND user_id=$3 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(case_id)
        .bind(user_id)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to remove Student Support case-team member")?;
        if changed.rows_affected() != 1 {
            bail!("The account is not an active member of this case team");
        }
        bump_case_version(&mut transaction, tenant_id, case_id, actor_id).await?;
        append_case_evidence(
            &mut transaction,
            CaseEvidence {
                tenant_id,
                case_id,
                actor,
                context,
                event_type: "student_support.case.team_member_removed",
                operation: "student_support.case_team.remove",
                metadata: json!({"user_id": user_id, "version": version + 1}),
            },
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit team removal")?;
        Self::get_case(pool, tenant_id, case_id, scope).await
    }

    pub async fn escalate_case(
        pool: &PgPool,
        tenant_id: Uuid,
        case_id: Uuid,
        scope: StudentSupportAccessScope,
        actor: AuditActor,
        context: RequestContext,
        request: &CaseTransitionRequest,
    ) -> Result<Option<CaseRecordResponse>> {
        transition_case(
            pool,
            tenant_id,
            case_id,
            scope,
            actor,
            context,
            request,
            Transition::Escalate,
        )
        .await
    }

    pub async fn resolve_case(
        pool: &PgPool,
        tenant_id: Uuid,
        case_id: Uuid,
        scope: StudentSupportAccessScope,
        actor: AuditActor,
        context: RequestContext,
        request: &CaseTransitionRequest,
    ) -> Result<Option<CaseRecordResponse>> {
        transition_case(
            pool,
            tenant_id,
            case_id,
            scope,
            actor,
            context,
            request,
            Transition::Resolve,
        )
        .await
    }

    pub async fn close_case(
        pool: &PgPool,
        tenant_id: Uuid,
        case_id: Uuid,
        scope: StudentSupportAccessScope,
        actor: AuditActor,
        context: RequestContext,
        request: &CaseTransitionRequest,
    ) -> Result<Option<CaseRecordResponse>> {
        transition_case(
            pool,
            tenant_id,
            case_id,
            scope,
            actor,
            context,
            request,
            Transition::Close,
        )
        .await
    }
}

#[derive(Debug, Clone, Copy)]
enum Transition {
    Escalate,
    Resolve,
    Close,
}

#[allow(
    clippy::too_many_arguments,
    reason = "the transition keeps authority and audit context explicit"
)]
async fn transition_case(
    pool: &PgPool,
    tenant_id: Uuid,
    case_id: Uuid,
    scope: StudentSupportAccessScope,
    actor: AuditActor,
    context: RequestContext,
    request: &CaseTransitionRequest,
    transition: Transition,
) -> Result<Option<CaseRecordResponse>> {
    ensure_manager_scope(scope)?;
    let actor_id = person_actor_id(actor)?;
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to start case transition")?;
    let Some((status, version, _)) =
        lock_scoped_case(&mut transaction, tenant_id, case_id, scope).await?
    else {
        return Ok(None);
    };
    ensure_version(version, request.expected_version)?;
    let reason = trimmed_required(&request.reason, "Transition reason")?;
    let (next_status, event_type, operation) = match transition {
        Transition::Escalate => {
            if !matches!(status.as_str(), "open" | "active") {
                bail!("Only an open or active case can be escalated");
            }
            sqlx::query(
                "UPDATE student_support_cases SET status='escalated', escalated_by=$3, escalated_at=NOW(), escalation_reason=$4, version=version+1, updated_by=$3 WHERE tenant_id=$1 AND id=$2",
            )
            .bind(tenant_id)
            .bind(case_id)
            .bind(actor_id)
            .bind(reason)
            .execute(&mut *transaction)
            .await
            .context("Failed to escalate Student Support case")?;
            (
                "escalated",
                "student_support.case.escalated",
                "student_support.cases.escalate",
            )
        }
        Transition::Resolve => {
            if !matches!(status.as_str(), "open" | "active" | "escalated") {
                bail!("Only an open case can be resolved");
            }
            sqlx::query(
                "UPDATE student_support_cases SET status='resolved', resolved_by=$3, resolved_at=NOW(), resolution_summary=$4, version=version+1, updated_by=$3 WHERE tenant_id=$1 AND id=$2",
            )
            .bind(tenant_id)
            .bind(case_id)
            .bind(actor_id)
            .bind(reason)
            .execute(&mut *transaction)
            .await
            .context("Failed to resolve Student Support case")?;
            (
                "resolved",
                "student_support.case.resolved",
                "student_support.cases.resolve",
            )
        }
        Transition::Close => {
            if status != "resolved" {
                bail!("Only a resolved case can be closed");
            }
            sqlx::query(
                "UPDATE student_support_cases SET status='closed', closed_by=$3, closed_at=NOW(), closure_reason=$4, version=version+1, updated_by=$3 WHERE tenant_id=$1 AND id=$2",
            )
            .bind(tenant_id)
            .bind(case_id)
            .bind(actor_id)
            .bind(reason)
            .execute(&mut *transaction)
            .await
            .context("Failed to close Student Support case")?;
            (
                "closed",
                "student_support.case.closed",
                "student_support.cases.close",
            )
        }
    };
    append_case_evidence(
        &mut transaction,
        CaseEvidence {
            tenant_id,
            case_id,
            actor,
            context,
            event_type,
            operation,
            metadata: json!({"status": next_status, "version": version + 1, "reason": reason}),
        },
    )
    .await?;
    transaction
        .commit()
        .await
        .context("Failed to commit case transition")?;
    StudentSupportOps::get_case(pool, tenant_id, case_id, scope).await
}

async fn validate_learner(pool: &PgPool, tenant_id: Uuid, learner_id: Uuid) -> Result<()> {
    let learner = LearnerOps::student_support_references_by_ids(pool, tenant_id, &[learner_id])
        .await?
        .pop()
        .ok_or_else(|| anyhow!("The learner was not found"))?;
    if learner.status != "active" {
        bail!("Student Support cases require an active learner");
    }
    Ok(())
}

async fn case_worker_candidates(
    pool: &PgPool,
    tenant_id: Uuid,
    search: Option<&str>,
) -> Result<Vec<CaseWorkerCandidateResponse>> {
    let search = search_pattern(search);
    sqlx::query_as::<_, (Uuid, String, String)>(
        r#"
        SELECT id, full_name, email
          FROM users
         WHERE tenant_id=$1 AND is_active AND deleted_at IS NULL
           AND roles && ARRAY['student_support_case_worker','student_support_manager']::TEXT[]
           AND ($2::TEXT IS NULL OR full_name ILIKE $2 OR email ILIKE $2)
         ORDER BY full_name, email
         LIMIT 100
        "#,
    )
    .bind(tenant_id)
    .bind(search.as_deref())
    .fetch_all(pool)
    .await
    .context("Failed to list Student Support Case Worker candidates")
    .map(|rows| {
        rows.into_iter()
            .map(|(user_id, full_name, email)| CaseWorkerCandidateResponse {
                user_id,
                full_name,
                email,
            })
            .collect()
    })
}

async fn validate_case_worker(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<()> {
    let eligible = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM users WHERE tenant_id=$1 AND id=$2 AND is_active AND deleted_at IS NULL AND roles && ARRAY['student_support_case_worker','student_support_manager']::TEXT[])",
    )
    .bind(tenant_id)
    .bind(user_id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to validate Student Support Case Worker")?;
    if !eligible {
        bail!("The selected account must have an active Student Support role");
    }
    Ok(())
}

async fn reserve_case_reference(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
) -> Result<String> {
    let (prefix, padding, sequence) = sqlx::query_as::<_, (String, i16, i64)>(
        r#"
        UPDATE student_support_numbering_policies
           SET next_case_sequence=next_case_sequence+1, version=version+1
         WHERE tenant_id=$1 AND deleted_at IS NULL
         RETURNING case_prefix, padding, next_case_sequence-1
        "#,
    )
    .bind(tenant_id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to reserve Student Support case reference")?;
    Ok(format!(
        "{prefix}{sequence:0width$}",
        width = padding as usize
    ))
}

async fn lock_scoped_case(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    case_id: Uuid,
    scope: StudentSupportAccessScope,
) -> Result<Option<(String, i32, Uuid)>> {
    sqlx::query_as::<_, (String, i32, Uuid)>(
        r#"
        SELECT support_case.status, support_case.version, support_case.lead_case_worker_user_id
          FROM student_support_cases AS support_case
         WHERE support_case.tenant_id=$1 AND support_case.id=$2
           AND support_case.deleted_at IS NULL
           AND (
               $3::UUID IS NULL
               OR support_case.lead_case_worker_user_id=$3
               OR EXISTS (
                   SELECT 1 FROM student_support_case_team_members AS member
                    WHERE member.tenant_id=support_case.tenant_id
                      AND member.case_id=support_case.id
                      AND member.user_id=$3 AND member.deleted_at IS NULL
               )
           )
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(case_id)
    .bind(scope.assigned_user_id())
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock Student Support case")
}

async fn bump_case_version(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    case_id: Uuid,
    actor_id: Uuid,
) -> Result<()> {
    sqlx::query(
        "UPDATE student_support_cases SET version=version+1, updated_by=$3 WHERE tenant_id=$1 AND id=$2",
    )
    .bind(tenant_id)
    .bind(case_id)
    .bind(actor_id)
    .execute(&mut **transaction)
    .await
    .context("Failed to update Student Support case version")?;
    Ok(())
}

async fn case_row_by_id(
    pool: &PgPool,
    tenant_id: Uuid,
    case_id: Uuid,
    scope: StudentSupportAccessScope,
) -> Result<Option<CaseRow>> {
    sqlx::query_as::<_, CaseRow>(CASE_BY_ID)
        .bind(tenant_id)
        .bind(case_id)
        .bind(scope.assigned_user_id())
        .fetch_optional(pool)
        .await
        .context("Failed to load Student Support case")
}

async fn hydrate_case_summaries(
    pool: &PgPool,
    tenant_id: Uuid,
    rows: Vec<CaseRow>,
) -> Result<Vec<CaseSummaryResponse>> {
    let learner_ids = rows.iter().map(|row| row.learner_id).collect::<Vec<_>>();
    let learners = LearnerOps::student_support_references_by_ids(pool, tenant_id, &learner_ids)
        .await?
        .into_iter()
        .map(|learner| (learner.id, learner))
        .collect::<HashMap<_, _>>();
    rows.into_iter()
        .map(|row| {
            let learner = learners
                .get(&row.learner_id)
                .ok_or_else(|| anyhow!("A Student Support learner identity is unavailable"))?;
            Ok(CaseSummaryResponse {
                id: row.id,
                reference: row.reference,
                learner_id: row.learner_id,
                learner_number: learner.learner_number.clone(),
                learner_name: learner.display_name.clone(),
                lead_case_worker_user_id: row.lead_case_worker_user_id,
                lead_case_worker_name: row.lead_case_worker_name,
                category: row.category,
                severity: row.severity,
                title: row.title,
                occurred_on: row.occurred_on,
                status: row.status,
                version: row.version,
                action_count: row.action_count,
                team_member_count: row.team_member_count + 1,
                updated_at: row.updated_at,
            })
        })
        .collect()
}

fn team_member_response(row: TeamMemberRow) -> CaseTeamMemberResponse {
    CaseTeamMemberResponse {
        user_id: row.user_id,
        full_name: row.full_name,
        email: row.email,
        member_role: row.member_role,
        assigned_at: row.assigned_at,
    }
}

fn action_response(row: ActionRow) -> CaseActionResponse {
    CaseActionResponse {
        id: row.id,
        case_id: row.case_id,
        action_kind: row.action_kind,
        summary: row.summary,
        details: row.details,
        occurred_at: row.occurred_at,
        created_by: row.created_by,
        created_by_name: row.created_by_name,
        created_at: row.created_at,
    }
}

fn event_response(row: EventRow) -> CaseEventResponse {
    CaseEventResponse {
        id: row.id,
        case_id: row.case_id,
        event_type: row.event_type,
        actor_id: row.actor_id,
        actor_name: row.actor_name,
        metadata: row.metadata,
        created_at: row.created_at,
    }
}

struct CaseEvidence<'a> {
    tenant_id: Uuid,
    case_id: Uuid,
    actor: AuditActor,
    context: RequestContext,
    event_type: &'a str,
    operation: &'a str,
    metadata: Value,
}

async fn append_case_evidence(
    transaction: &mut Transaction<'_, Postgres>,
    evidence: CaseEvidence<'_>,
) -> Result<()> {
    let actor_id = person_actor_id(evidence.actor)?;
    sqlx::query(
        "INSERT INTO student_support_case_events (tenant_id, case_id, event_type, actor_id, metadata) VALUES ($1,$2,$3,$4,$5)",
    )
    .bind(evidence.tenant_id)
    .bind(evidence.case_id)
    .bind(evidence.event_type)
    .bind(actor_id)
    .bind(evidence.metadata.clone())
    .execute(&mut **transaction)
    .await
    .context("Failed to append Student Support lifecycle evidence")?;
    append_audit(
        &mut **transaction,
        &NewAuditEvent::new(
            evidence.tenant_id,
            evidence.actor,
            evidence.operation,
            AuditOutcome::Succeeded,
            evidence.context,
        )
        .with_target(AuditTarget::new(
            "student_support_case",
            evidence.case_id.to_string(),
        ))
        .with_redacted_metadata(
            evidence
                .metadata
                .as_object()
                .cloned()
                .unwrap_or_else(Map::new),
        ),
    )
    .await
    .context("Failed to append Student Support audit evidence")?;
    Ok(())
}

fn ensure_manager_scope(scope: StudentSupportAccessScope) -> Result<()> {
    if !scope.is_campus() {
        bail!("Student Support management requires campus case scope");
    }
    Ok(())
}

fn ensure_version(actual: i32, expected: i32) -> Result<()> {
    if actual != expected {
        bail!("The Student Support case changed; reload before continuing");
    }
    Ok(())
}

fn person_actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("Student Support requires a person actor"))
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
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{value}%"))
}

fn bounded_page(query: &StudentSupportListQuery) -> (i64, i64) {
    (
        query.page.unwrap_or(1).max(1),
        query.per_page.unwrap_or(20).clamp(1, 100),
    )
}

fn database_error(error: sqlx::Error, duplicate_message: &str) -> anyhow::Error {
    if let sqlx::Error::Database(database) = &error
        && database.constraint() == Some("idx_student_support_case_team_active")
    {
        return anyhow!(duplicate_message.to_string());
    }
    anyhow!(error).context("Failed to update Student Support case team")
}

const CASE_LIST: &str = r#"
    SELECT support_case.id, support_case.reference, support_case.learner_id,
           support_case.lead_case_worker_user_id,
           lead.full_name AS lead_case_worker_name,
           lead.email AS lead_case_worker_email,
           support_case.category, support_case.severity, support_case.title,
           support_case.summary, support_case.occurred_on, support_case.status,
           support_case.version,
           (SELECT COUNT(*) FROM student_support_case_actions AS action
             WHERE action.tenant_id=support_case.tenant_id AND action.case_id=support_case.id) AS action_count,
           (SELECT COUNT(*) FROM student_support_case_team_members AS member
             WHERE member.tenant_id=support_case.tenant_id AND member.case_id=support_case.id
               AND member.deleted_at IS NULL) AS team_member_count,
           support_case.escalated_at, support_case.escalation_reason,
           support_case.resolved_at, support_case.resolution_summary,
           support_case.closed_at, support_case.closure_reason,
           support_case.created_at, support_case.updated_at
      FROM student_support_cases AS support_case
      JOIN users AS lead
        ON lead.id=support_case.lead_case_worker_user_id AND lead.tenant_id=support_case.tenant_id
     WHERE support_case.tenant_id=$1 AND support_case.deleted_at IS NULL
       AND ($2::TEXT IS NULL OR support_case.status=$2)
       AND ($3::TEXT IS NULL OR support_case.category=$3)
       AND ($4::TEXT IS NULL OR support_case.severity=$4)
       AND ($5::UUID IS NULL OR support_case.learner_id=$5)
       AND ($6::TEXT IS NULL OR support_case.reference ILIKE $6 OR support_case.title ILIKE $6
            OR support_case.learner_id=ANY(COALESCE($7::UUID[], ARRAY[]::UUID[])))
       AND ($8::UUID IS NULL OR support_case.lead_case_worker_user_id=$8 OR EXISTS (
            SELECT 1 FROM student_support_case_team_members AS scoped_member
             WHERE scoped_member.tenant_id=support_case.tenant_id
               AND scoped_member.case_id=support_case.id
               AND scoped_member.user_id=$8 AND scoped_member.deleted_at IS NULL
       ))
     ORDER BY CASE support_case.severity
                  WHEN 'critical' THEN 1 WHEN 'high' THEN 2
                  WHEN 'moderate' THEN 3 ELSE 4 END,
              support_case.updated_at DESC, support_case.reference DESC
     LIMIT $9 OFFSET $10
"#;

const CASE_COUNT: &str = r#"
    SELECT COUNT(*)
      FROM student_support_cases AS support_case
     WHERE support_case.tenant_id=$1 AND support_case.deleted_at IS NULL
       AND ($2::TEXT IS NULL OR support_case.status=$2)
       AND ($3::TEXT IS NULL OR support_case.category=$3)
       AND ($4::TEXT IS NULL OR support_case.severity=$4)
       AND ($5::UUID IS NULL OR support_case.learner_id=$5)
       AND ($6::TEXT IS NULL OR support_case.reference ILIKE $6 OR support_case.title ILIKE $6
            OR support_case.learner_id=ANY(COALESCE($7::UUID[], ARRAY[]::UUID[])))
       AND ($8::UUID IS NULL OR support_case.lead_case_worker_user_id=$8 OR EXISTS (
            SELECT 1 FROM student_support_case_team_members AS scoped_member
             WHERE scoped_member.tenant_id=support_case.tenant_id
               AND scoped_member.case_id=support_case.id
               AND scoped_member.user_id=$8 AND scoped_member.deleted_at IS NULL
       ))
"#;

const CASE_BY_ID: &str = r#"
    SELECT support_case.id, support_case.reference, support_case.learner_id,
           support_case.lead_case_worker_user_id,
           lead.full_name AS lead_case_worker_name,
           lead.email AS lead_case_worker_email,
           support_case.category, support_case.severity, support_case.title,
           support_case.summary, support_case.occurred_on, support_case.status,
           support_case.version,
           (SELECT COUNT(*) FROM student_support_case_actions AS action
             WHERE action.tenant_id=support_case.tenant_id AND action.case_id=support_case.id) AS action_count,
           (SELECT COUNT(*) FROM student_support_case_team_members AS member
             WHERE member.tenant_id=support_case.tenant_id AND member.case_id=support_case.id
               AND member.deleted_at IS NULL) AS team_member_count,
           support_case.escalated_at, support_case.escalation_reason,
           support_case.resolved_at, support_case.resolution_summary,
           support_case.closed_at, support_case.closure_reason,
           support_case.created_at, support_case.updated_at
      FROM student_support_cases AS support_case
      JOIN users AS lead
        ON lead.id=support_case.lead_case_worker_user_id AND lead.tenant_id=support_case.tenant_id
     WHERE support_case.tenant_id=$1 AND support_case.id=$2
       AND support_case.deleted_at IS NULL
       AND ($3::UUID IS NULL OR support_case.lead_case_worker_user_id=$3 OR EXISTS (
            SELECT 1 FROM student_support_case_team_members AS member
             WHERE member.tenant_id=support_case.tenant_id AND member.case_id=support_case.id
               AND member.user_id=$3 AND member.deleted_at IS NULL
       ))
"#;

const ACTION_SELECT: &str = r#"
    SELECT action.id, action.case_id, action.action_kind, action.summary,
           action.details, action.occurred_at, action.created_by,
           account.full_name AS created_by_name, action.created_at
      FROM student_support_case_actions AS action
      JOIN users AS account
        ON account.id=action.created_by AND account.tenant_id=action.tenant_id
     WHERE action.tenant_id=$1 AND action.case_id=$2
     ORDER BY action.occurred_at DESC, action.id DESC
     LIMIT 200
"#;

const ACTION_BY_ID: &str = r#"
    SELECT action.id, action.case_id, action.action_kind, action.summary,
           action.details, action.occurred_at, action.created_by,
           account.full_name AS created_by_name, action.created_at
      FROM student_support_case_actions AS action
      JOIN users AS account
        ON account.id=action.created_by AND account.tenant_id=action.tenant_id
     WHERE action.tenant_id=$1 AND action.id=$2
"#;

#[cfg(test)]
mod tests {
    use super::{StudentSupportAccessScope, ensure_manager_scope, ensure_version};
    use uuid::Uuid;

    #[test]
    fn case_team_scope_never_satisfies_management_authority() {
        assert!(ensure_manager_scope(StudentSupportAccessScope::CaseTeam(Uuid::new_v4())).is_err());
        assert!(ensure_manager_scope(StudentSupportAccessScope::Campus).is_ok());
    }

    #[test]
    fn optimistic_version_must_match_exactly() {
        assert!(ensure_version(4, 4).is_ok());
        assert!(ensure_version(4, 3).is_err());
    }
}
