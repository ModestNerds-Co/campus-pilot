//! Reviewed Learning score proposals applied through the Gradebook owner.
//!
//! Teachers prepare immutable assignment or quiz evidence. A different campus
//! reviewer may apply only ready rows to an unchanged draft Gradebook sheet.

use std::collections::HashMap;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Utc};
use cp_audit::{AuditActor, RequestContext};
use cp_gradebook::{
    ApplyGradebookScoreTransfer, GradebookAccessScope, GradebookOps, GradebookScoreTransferMark,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::dtos::{
    ApplyLearningScoreTransferRequest, CreateLearningScoreTransferRequest, LearningAccessScope,
    LearningScoreTransferListQuery, LearningScoreTransferResponse,
    LearningScoreTransferRowResponse, LearningScoreTransferSourceType, LearningScoreTransferStatus,
    LearningScoreTransferSummary, RejectLearningScoreTransferRequest,
};
use crate::ops::{
    LearningOps, append_evidence, ensure_can_author_space, person_actor_id, scope_allows_space,
    space_row,
};

const DEFAULT_PAGE: i64 = 1;
const DEFAULT_PER_PAGE: i64 = 25;
const MAX_PER_PAGE: i64 = 100;

#[derive(Debug, FromRow)]
struct ProposalSummaryRow {
    id: Uuid,
    learning_space_id: Uuid,
    learning_space_title: String,
    class_group_name: String,
    subject_name: String,
    source_type: String,
    source_id: Uuid,
    source_title_snapshot: String,
    source_version: i32,
    target_mark_sheet_id: Uuid,
    target_mark_sheet_version: i32,
    target_assessment_name: String,
    target_maximum_marks: i32,
    status: String,
    version: i32,
    ready_count: i64,
    missing_source_count: i64,
    target_already_marked_count: i64,
    proposed_by_id: Uuid,
    proposed_by_name: String,
    proposed_at: DateTime<Utc>,
    reviewed_by_name: Option<String>,
    reviewed_at: Option<DateTime<Utc>>,
    review_reason: Option<String>,
    applied_mark_sheet_version: Option<i32>,
}

#[derive(Debug, FromRow)]
struct ProposalRow {
    id: Uuid,
    learning_space_id: Uuid,
    source_type: String,
    source_id: Uuid,
    source_version: i32,
    target_mark_sheet_id: Uuid,
    target_mark_sheet_version: i32,
    status: String,
    version: i32,
    proposed_by: Uuid,
}

#[derive(Debug, FromRow)]
struct TransferRow {
    id: Uuid,
    target_mark_id: Uuid,
    enrolment_id: Uuid,
    learner_id: Uuid,
    learner_number_snapshot: String,
    learner_name_snapshot: String,
    target_mark_version: i32,
    source_evidence_id: Option<Uuid>,
    source_evidence_version: Option<i32>,
    source_score_basis_points: Option<i32>,
    proposed_marks_hundredths: Option<i64>,
    outcome: String,
}

#[derive(Debug)]
struct SourceHeader {
    space_id: Uuid,
    version: i32,
    title: String,
}

#[derive(Debug, Clone)]
struct SourceEvidence {
    evidence_id: Uuid,
    evidence_version: i32,
    score_basis_points: i32,
}

#[derive(Debug)]
struct ProposedRow {
    target_mark_id: Uuid,
    enrolment_id: Uuid,
    learner_id: Uuid,
    learner_number: String,
    learner_name: String,
    target_mark_version: i32,
    source: Option<SourceEvidence>,
    proposed_marks_hundredths: Option<i64>,
    outcome: &'static str,
}

impl LearningOps {
    pub async fn list_score_transfers(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: LearningAccessScope,
        query: &LearningScoreTransferListQuery,
    ) -> Result<(Vec<LearningScoreTransferSummary>, i64)> {
        let (campus, account_id) = author_filter(scope);
        let Some((campus, account_id)) = campus.zip(account_id) else {
            return Ok((Vec::new(), 0));
        };
        let page = query.page.unwrap_or(DEFAULT_PAGE).max(1);
        let per_page = query
            .per_page
            .unwrap_or(DEFAULT_PER_PAGE)
            .clamp(1, MAX_PER_PAGE);
        let offset = (page - 1) * per_page;
        let status = query.status.map(LearningScoreTransferStatus::as_str);
        let rows = sqlx::query_as::<_, ProposalSummaryRow>(&format!(
            "{PROPOSAL_SUMMARY_SELECT} AND ($2 OR employee.account_id=$3) AND ($4::TEXT IS NULL OR proposal.status=$4) ORDER BY proposal.proposed_at DESC,proposal.id LIMIT $5 OFFSET $6"
        ))
        .bind(tenant_id)
        .bind(campus)
        .bind(account_id)
        .bind(status)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list Learning score transfers")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)::BIGINT
              FROM learning_score_transfer_proposals proposal
              JOIN learning_spaces space
                ON space.id=proposal.learning_space_id AND space.tenant_id=proposal.tenant_id
              JOIN teaching_assignments assignment
                ON assignment.id=space.teaching_assignment_id AND assignment.tenant_id=space.tenant_id
              JOIN teacher_profiles teacher
                ON teacher.id=assignment.teacher_profile_id AND teacher.tenant_id=assignment.tenant_id
              JOIN employees employee
                ON employee.id=teacher.employee_id AND employee.tenant_id=teacher.tenant_id
             WHERE proposal.tenant_id=$1 AND ($2 OR employee.account_id=$3)
               AND ($4::TEXT IS NULL OR proposal.status=$4)
            "#,
        )
        .bind(tenant_id)
        .bind(campus)
        .bind(account_id)
        .bind(status)
        .fetch_one(pool)
        .await
        .context("Failed to count Learning score transfers")?;
        Ok((
            rows.into_iter()
                .map(summary_response)
                .collect::<Result<Vec<_>>>()?,
            total,
        ))
    }

    pub async fn get_score_transfer(
        pool: &PgPool,
        tenant_id: Uuid,
        proposal_id: Uuid,
        scope: LearningAccessScope,
    ) -> Result<Option<LearningScoreTransferResponse>> {
        let Some(summary) = summary_row(pool, tenant_id, proposal_id).await? else {
            return Ok(None);
        };
        let Some(space) = space_row(pool, tenant_id, summary.learning_space_id).await? else {
            return Ok(None);
        };
        if !scope_allows_space(pool, tenant_id, &space, scope).await?
            || matches!(scope, LearningAccessScope::SelfFor(_))
        {
            return Ok(None);
        }
        let rows = transfer_rows(pool, tenant_id, proposal_id).await?;
        Ok(Some(LearningScoreTransferResponse {
            summary: summary_response(summary)?,
            rows: rows.into_iter().map(row_response).collect(),
        }))
    }

    pub async fn create_score_transfer(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateLearningScoreTransferRequest,
    ) -> Result<LearningScoreTransferResponse> {
        let actor_id = person_actor_id(actor)?;
        let source = source_header(pool, tenant_id, request.source_type, request.source_id)
            .await?
            .ok_or_else(|| anyhow!("The selected Learning score source was not found"))?;
        ensure_can_author_space(pool, tenant_id, source.space_id, scope).await?;
        let space = space_row(pool, tenant_id, source.space_id)
            .await?
            .context("The source Learning space is unavailable")?;
        let target = GradebookOps::get(
            pool,
            tenant_id,
            request.target_mark_sheet_id,
            GradebookAccessScope::Campus,
        )
        .await?
        .context("The target Gradebook mark sheet was not found")?;
        if target.summary.status != "draft" {
            bail!("Learning scores can be proposed only for a draft mark sheet");
        }
        if target.summary.teaching_assignment_id != space.teaching_assignment_id {
            bail!("The Learning source and Gradebook target must use the same teaching assignment");
        }

        let evidence =
            source_evidence(pool, tenant_id, request.source_type, request.source_id).await?;
        let target_maximum_hundredths = i64::from(target.summary.maximum_marks) * 100;
        let mut proposed_rows = target
            .marks
            .into_iter()
            .map(|mark| {
                let source = evidence.get(&mark.learner_id).cloned();
                let (outcome, proposed_marks_hundredths) = if mark.mark_status != "unmarked" {
                    ("target_already_marked", None)
                } else if let Some(source) = &source {
                    (
                        "ready",
                        Some(rounded_ratio(
                            i64::from(source.score_basis_points),
                            10_000,
                            target_maximum_hundredths,
                        )),
                    )
                } else {
                    ("missing_source", None)
                };
                ProposedRow {
                    target_mark_id: mark.id,
                    enrolment_id: mark.enrolment_id,
                    learner_id: mark.learner_id,
                    learner_number: mark.learner_number,
                    learner_name: mark.learner_name,
                    target_mark_version: mark.version,
                    source,
                    proposed_marks_hundredths,
                    outcome,
                }
            })
            .collect::<Vec<_>>();
        proposed_rows.sort_by_key(|row| row.learner_id);
        if !proposed_rows.iter().any(|row| row.outcome == "ready") {
            bail!("The Learning source has no ready scores for this mark sheet");
        }
        let fingerprint = proposal_fingerprint(
            request,
            source.version,
            target.summary.version,
            &proposed_rows,
        );
        if let Some((existing_id, existing_fingerprint)) = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id,request_fingerprint FROM learning_score_transfer_proposals WHERE tenant_id=$1 AND idempotency_key=$2",
        )
        .bind(tenant_id)
        .bind(request.idempotency_key)
        .fetch_optional(pool)
        .await
        .context("Failed to check score-transfer idempotency")?
        {
            if existing_fingerprint != fingerprint {
                bail!("This idempotency key was already used for another score transfer");
            }
            return Self::get_score_transfer(pool, tenant_id, existing_id, scope)
                .await?
                .context("The existing Learning score transfer is unavailable");
        }

        let proposal_id = Uuid::new_v4();
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start the Learning score transfer")?;
        sqlx::query(
            r#"
            INSERT INTO learning_score_transfer_proposals (
                id,tenant_id,learning_space_id,source_type,source_id,source_version,
                source_title_snapshot,target_mark_sheet_id,target_mark_sheet_version,
                target_maximum_marks,idempotency_key,request_fingerprint,proposed_by
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
            "#,
        )
        .bind(proposal_id)
        .bind(tenant_id)
        .bind(source.space_id)
        .bind(request.source_type.as_str())
        .bind(request.source_id)
        .bind(source.version)
        .bind(&source.title)
        .bind(request.target_mark_sheet_id)
        .bind(target.summary.version)
        .bind(target.summary.maximum_marks)
        .bind(request.idempotency_key)
        .bind(&fingerprint)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to create the Learning score-transfer proposal")?;
        for row in &proposed_rows {
            insert_transfer_row(&mut transaction, tenant_id, proposal_id, row).await?;
        }
        let ready_count = proposed_rows
            .iter()
            .filter(|row| row.outcome == "ready")
            .count();
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "score_transfer",
            proposal_id,
            Some(source.space_id),
            "learning_score_transfer_proposed",
            "learning.score_transfers.create",
            json!({
                "source_type": request.source_type.as_str(),
                "source_id": request.source_id,
                "target_mark_sheet_id": request.target_mark_sheet_id,
                "ready_count": ready_count
            }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit the Learning score transfer")?;
        Self::get_score_transfer(pool, tenant_id, proposal_id, scope)
            .await?
            .context("The created Learning score transfer could not be reloaded")
    }

    pub async fn apply_score_transfer(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: LearningAccessScope,
        proposal_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ApplyLearningScoreTransferRequest,
    ) -> Result<Option<LearningScoreTransferResponse>> {
        ensure_campus_review(scope)?;
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start score-transfer review")?;
        let Some(proposal) = lock_proposal(&mut transaction, tenant_id, proposal_id).await? else {
            return Ok(None);
        };
        ensure_pending_review(&proposal, request.expected_version, actor_id)?;
        verify_source_evidence(&mut transaction, tenant_id, &proposal).await?;
        let rows = transfer_rows_transaction(&mut transaction, tenant_id, proposal_id).await?;
        let ready = rows
            .iter()
            .filter(|row| row.outcome == "ready")
            .map(|row| {
                Ok(GradebookScoreTransferMark {
                    mark_id: row.target_mark_id,
                    learner_id: row.learner_id,
                    expected_mark_version: row.target_mark_version,
                    marks_awarded_hundredths: row
                        .proposed_marks_hundredths
                        .context("A ready score-transfer row has no proposed mark")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let gradebook_version = GradebookOps::apply_learning_score_transfer(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            &ApplyGradebookScoreTransfer {
                proposal_id,
                mark_sheet_id: proposal.target_mark_sheet_id,
                expected_sheet_version: proposal.target_mark_sheet_version,
                source_type: proposal.source_type.clone(),
                marks: ready,
            },
        )
        .await?;
        sqlx::query(
            "UPDATE learning_score_transfer_proposals SET status='applied',version=version+1,reviewed_by=$4,reviewed_at=NOW(),applied_mark_sheet_version=$5 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='pending'",
        )
        .bind(tenant_id)
        .bind(proposal_id)
        .bind(request.expected_version)
        .bind(actor_id)
        .bind(gradebook_version)
        .execute(&mut *transaction)
        .await
        .context("Failed to apply the Learning score-transfer decision")?;
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "score_transfer",
            proposal_id,
            Some(proposal.learning_space_id),
            "learning_score_transfer_applied",
            "learning.score_transfers.apply",
            json!({
                "target_mark_sheet_id": proposal.target_mark_sheet_id,
                "applied_mark_sheet_version": gradebook_version,
                "transferred_count": rows.iter().filter(|row| row.outcome == "ready").count()
            }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit the score-transfer review")?;
        Self::get_score_transfer(pool, tenant_id, proposal_id, scope).await
    }

    pub async fn reject_score_transfer(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: LearningAccessScope,
        proposal_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &RejectLearningScoreTransferRequest,
    ) -> Result<Option<LearningScoreTransferResponse>> {
        ensure_campus_review(scope)?;
        let actor_id = person_actor_id(actor)?;
        let reason = request.reason.trim();
        if reason.is_empty() {
            bail!("A rejection reason is required");
        }
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start score-transfer rejection")?;
        let Some(proposal) = lock_proposal(&mut transaction, tenant_id, proposal_id).await? else {
            return Ok(None);
        };
        ensure_pending_review(&proposal, request.expected_version, actor_id)?;
        sqlx::query(
            "UPDATE learning_score_transfer_proposals SET status='rejected',version=version+1,reviewed_by=$4,reviewed_at=NOW(),review_reason=$5 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='pending'",
        )
        .bind(tenant_id)
        .bind(proposal_id)
        .bind(request.expected_version)
        .bind(actor_id)
        .bind(reason)
        .execute(&mut *transaction)
        .await
        .context("Failed to reject the Learning score transfer")?;
        append_evidence(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "score_transfer",
            proposal_id,
            Some(proposal.learning_space_id),
            "learning_score_transfer_rejected",
            "learning.score_transfers.reject",
            json!({ "reason": reason }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit the score-transfer rejection")?;
        Self::get_score_transfer(pool, tenant_id, proposal_id, scope).await
    }
}

fn author_filter(scope: LearningAccessScope) -> (Option<bool>, Option<Option<Uuid>>) {
    match scope {
        LearningAccessScope::Campus => (Some(true), Some(None)),
        LearningAccessScope::AssignedTo(account_id)
        | LearningAccessScope::SelfAndAssigned(account_id) => (Some(false), Some(Some(account_id))),
        LearningAccessScope::SelfFor(_) => (None, None),
    }
}

async fn source_header(
    pool: &PgPool,
    tenant_id: Uuid,
    source_type: LearningScoreTransferSourceType,
    source_id: Uuid,
) -> Result<Option<SourceHeader>> {
    match source_type {
        LearningScoreTransferSourceType::Assignment => sqlx::query_as::<_, (Uuid, i32, String)>(
            r#"
            SELECT unit.learning_space_id,assignment.version,assignment.title
              FROM learning_assignments assignment
              JOIN learning_units unit
                ON unit.id=assignment.learning_unit_id AND unit.tenant_id=assignment.tenant_id
             WHERE assignment.tenant_id=$1 AND assignment.id=$2
               AND assignment.status IN ('published','closed')
               AND assignment.deleted_at IS NULL AND unit.deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(source_id)
        .fetch_optional(pool)
        .await
        .context("Failed to load the Learning assignment score source"),
        LearningScoreTransferSourceType::Quiz => sqlx::query_as::<_, (Uuid, i32, String)>(
            r#"
            SELECT unit.learning_space_id,quiz.version,quiz.title
              FROM learning_quizzes quiz
              JOIN learning_units unit
                ON unit.id=quiz.learning_unit_id AND unit.tenant_id=quiz.tenant_id
             WHERE quiz.tenant_id=$1 AND quiz.id=$2
               AND quiz.status IN ('published','closed')
               AND quiz.deleted_at IS NULL AND unit.deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(source_id)
        .fetch_optional(pool)
        .await
        .context("Failed to load the Learning quiz score source"),
    }
    .map(|row| {
        row.map(|(space_id, version, title)| SourceHeader {
            space_id,
            version,
            title,
        })
    })
}

async fn source_evidence(
    pool: &PgPool,
    tenant_id: Uuid,
    source_type: LearningScoreTransferSourceType,
    source_id: Uuid,
) -> Result<HashMap<Uuid, SourceEvidence>> {
    let rows = match source_type {
        LearningScoreTransferSourceType::Assignment => {
            sqlx::query_as::<_, (Uuid, Uuid, i32, i32, i32)>(
                r#"
                SELECT recipient.learner_id,review.id,review.version,
                       review.total_score_hundredths,assignment.max_score_hundredths
                  FROM learning_assignment_recipients recipient
                  JOIN learning_assignments assignment
                    ON assignment.id=recipient.learning_assignment_id
                   AND assignment.tenant_id=recipient.tenant_id
                  JOIN learning_submissions submission
                    ON submission.assignment_recipient_id=recipient.id
                   AND submission.tenant_id=recipient.tenant_id
                  JOIN learning_submission_reviews review
                    ON review.submission_version_id=submission.current_submission_version_id
                   AND review.tenant_id=submission.tenant_id
                 WHERE recipient.tenant_id=$1 AND recipient.learning_assignment_id=$2
                   AND review.status='released' AND review.outcome='graded'
                "#,
            )
            .bind(tenant_id)
            .bind(source_id)
            .fetch_all(pool)
            .await
            .context("Failed to load released Learning assignment scores")?
            .into_iter()
            .map(|(learner_id, evidence_id, version, score, maximum)| {
                (
                    learner_id,
                    evidence_id,
                    version,
                    rounded_ratio(i64::from(score), i64::from(maximum), 10_000) as i32,
                )
            })
            .collect::<Vec<_>>()
        }
        LearningScoreTransferSourceType::Quiz => sqlx::query_as::<_, (Uuid, Uuid, i32, i32)>(
            r#"
            SELECT DISTINCT ON (recipient.learner_id)
                   recipient.learner_id,attempt.id,attempt.version,attempt.score_basis_points
              FROM learning_quiz_recipients recipient
              JOIN learning_quiz_attempts attempt
                ON attempt.quiz_recipient_id=recipient.id
               AND attempt.tenant_id=recipient.tenant_id
             WHERE recipient.tenant_id=$1 AND recipient.learning_quiz_id=$2
               AND attempt.status='submitted'
             ORDER BY recipient.learner_id,attempt.score_basis_points DESC,
                      attempt.submitted_at DESC,attempt.attempt_number DESC,attempt.id
            "#,
        )
        .bind(tenant_id)
        .bind(source_id)
        .fetch_all(pool)
        .await
        .context("Failed to load submitted Learning quiz scores")?,
    };
    Ok(rows
        .into_iter()
        .map(
            |(learner_id, evidence_id, evidence_version, score_basis_points)| {
                (
                    learner_id,
                    SourceEvidence {
                        evidence_id,
                        evidence_version,
                        score_basis_points,
                    },
                )
            },
        )
        .collect())
}

async fn insert_transfer_row(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    proposal_id: Uuid,
    row: &ProposedRow,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO learning_score_transfer_rows (
            tenant_id,proposal_id,target_mark_id,enrolment_id,learner_id,
            learner_number_snapshot,learner_name_snapshot,target_mark_version,
            source_evidence_id,source_evidence_version,source_score_basis_points,
            proposed_marks_hundredths,outcome
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
        "#,
    )
    .bind(tenant_id)
    .bind(proposal_id)
    .bind(row.target_mark_id)
    .bind(row.enrolment_id)
    .bind(row.learner_id)
    .bind(&row.learner_number)
    .bind(&row.learner_name)
    .bind(row.target_mark_version)
    .bind(row.source.as_ref().map(|source| source.evidence_id))
    .bind(row.source.as_ref().map(|source| source.evidence_version))
    .bind(row.source.as_ref().map(|source| source.score_basis_points))
    .bind(row.proposed_marks_hundredths)
    .bind(row.outcome)
    .execute(&mut **transaction)
    .await
    .context("Failed to retain a score-transfer learner row")?;
    Ok(())
}

fn proposal_fingerprint(
    request: &CreateLearningScoreTransferRequest,
    source_version: i32,
    target_version: i32,
    rows: &[ProposedRow],
) -> String {
    let mut digest = Sha256::new();
    digest.update(request.source_type.as_str().as_bytes());
    digest.update(request.source_id.as_bytes());
    digest.update(source_version.to_be_bytes());
    digest.update(request.target_mark_sheet_id.as_bytes());
    digest.update(target_version.to_be_bytes());
    for row in rows {
        digest.update(row.target_mark_id.as_bytes());
        digest.update(row.learner_id.as_bytes());
        digest.update(row.target_mark_version.to_be_bytes());
        digest.update(row.outcome.as_bytes());
        if let Some(source) = &row.source {
            digest.update(source.evidence_id.as_bytes());
            digest.update(source.evidence_version.to_be_bytes());
            digest.update(source.score_basis_points.to_be_bytes());
        }
        digest.update(
            row.proposed_marks_hundredths
                .unwrap_or_default()
                .to_be_bytes(),
        );
    }
    format!("{:x}", digest.finalize())
}

fn rounded_ratio(numerator: i64, denominator: i64, scale: i64) -> i64 {
    ((i128::from(numerator) * i128::from(scale) + i128::from(denominator) / 2)
        / i128::from(denominator)) as i64
}

fn ensure_campus_review(scope: LearningAccessScope) -> Result<()> {
    if !matches!(scope, LearningAccessScope::Campus) {
        bail!("Campus Learning management scope is required to review score transfers");
    }
    Ok(())
}

fn ensure_pending_review(
    proposal: &ProposalRow,
    expected_version: i32,
    actor_id: Uuid,
) -> Result<()> {
    if proposal.status != "pending" || proposal.version != expected_version {
        bail!("The Learning score transfer changed before this review");
    }
    if proposal.proposed_by == actor_id {
        bail!("A score-transfer proposer cannot review their own proposal");
    }
    Ok(())
}

async fn verify_source_evidence(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    proposal: &ProposalRow,
) -> Result<()> {
    let header_version = match proposal.source_type.as_str() {
        "assignment" => sqlx::query_scalar::<_, i32>(
            "SELECT version FROM learning_assignments WHERE tenant_id=$1 AND id=$2 AND status IN ('published','closed') AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(proposal.source_id)
        .fetch_optional(&mut **transaction)
        .await
        .context("Failed to verify the Learning assignment source")?,
        "quiz" => sqlx::query_scalar::<_, i32>(
            "SELECT version FROM learning_quizzes WHERE tenant_id=$1 AND id=$2 AND status IN ('published','closed') AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(proposal.source_id)
        .fetch_optional(&mut **transaction)
        .await
        .context("Failed to verify the Learning quiz source")?,
        _ => bail!("The stored Learning score source type is invalid"),
    };
    if header_version != Some(proposal.source_version) {
        bail!("The Learning score source changed before review");
    }
    let ready_rows = transfer_rows_transaction(transaction, tenant_id, proposal.id)
        .await?
        .into_iter()
        .filter(|row| row.outcome == "ready")
        .collect::<Vec<_>>();
    for row in ready_rows {
        let evidence_id = row
            .source_evidence_id
            .context("A ready score-transfer row has no source evidence")?;
        let evidence_version = row
            .source_evidence_version
            .context("A ready score-transfer row has no source version")?;
        let current = match proposal.source_type.as_str() {
            "assignment" => sqlx::query_as::<_, (i32, i32, i32)>(
                r#"
                SELECT review.version,review.total_score_hundredths,assignment.max_score_hundredths
                  FROM learning_submission_reviews review
                  JOIN learning_submission_versions submission_version
                    ON submission_version.id=review.submission_version_id
                   AND submission_version.tenant_id=review.tenant_id
                  JOIN learning_submissions submission
                    ON submission.id=submission_version.learning_submission_id
                   AND submission.tenant_id=submission_version.tenant_id
                  JOIN learning_assignments assignment
                    ON assignment.id=submission.learning_assignment_id
                   AND assignment.tenant_id=submission.tenant_id
                 WHERE review.tenant_id=$1 AND review.id=$2
                   AND assignment.id=$3 AND review.status='released'
                   AND review.outcome='graded'
                "#,
            )
            .bind(tenant_id)
            .bind(evidence_id)
            .bind(proposal.source_id)
            .fetch_optional(&mut **transaction)
            .await
            .context("Failed to verify released Learning feedback")?
            .map(|(version, score, maximum)| {
                (version, rounded_ratio(i64::from(score), i64::from(maximum), 10_000) as i32)
            }),
            "quiz" => sqlx::query_as::<_, (i32, i32)>(
                "SELECT version,score_basis_points FROM learning_quiz_attempts WHERE tenant_id=$1 AND id=$2 AND learning_quiz_id=$3 AND status='submitted'",
            )
            .bind(tenant_id)
            .bind(evidence_id)
            .bind(proposal.source_id)
            .fetch_optional(&mut **transaction)
            .await
            .context("Failed to verify the submitted Learning quiz attempt")?,
            _ => None,
        };
        if current
            != Some((
                evidence_version,
                row.source_score_basis_points
                    .context("A ready score-transfer row has no source score")?,
            ))
        {
            bail!("Learning score evidence changed before review");
        }
    }
    Ok(())
}

async fn lock_proposal(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    proposal_id: Uuid,
) -> Result<Option<ProposalRow>> {
    sqlx::query_as::<_, ProposalRow>(
        r#"
        SELECT id,learning_space_id,source_type,source_id,source_version,
               target_mark_sheet_id,target_mark_sheet_version,status,version,
               proposed_by
          FROM learning_score_transfer_proposals
         WHERE tenant_id=$1 AND id=$2 FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(proposal_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock the Learning score transfer")
}

async fn summary_row(
    pool: &PgPool,
    tenant_id: Uuid,
    proposal_id: Uuid,
) -> Result<Option<ProposalSummaryRow>> {
    sqlx::query_as::<_, ProposalSummaryRow>(&format!(
        "{PROPOSAL_SUMMARY_SELECT} AND proposal.id=$2"
    ))
    .bind(tenant_id)
    .bind(proposal_id)
    .fetch_optional(pool)
    .await
    .context("Failed to load the Learning score transfer")
}

async fn transfer_rows(
    pool: &PgPool,
    tenant_id: Uuid,
    proposal_id: Uuid,
) -> Result<Vec<TransferRow>> {
    sqlx::query_as::<_, TransferRow>(TRANSFER_ROWS_SELECT)
        .bind(tenant_id)
        .bind(proposal_id)
        .fetch_all(pool)
        .await
        .context("Failed to load Learning score-transfer rows")
}

async fn transfer_rows_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    proposal_id: Uuid,
) -> Result<Vec<TransferRow>> {
    sqlx::query_as::<_, TransferRow>(TRANSFER_ROWS_SELECT)
        .bind(tenant_id)
        .bind(proposal_id)
        .fetch_all(&mut **transaction)
        .await
        .context("Failed to load Learning score-transfer rows")
}

fn summary_response(row: ProposalSummaryRow) -> Result<LearningScoreTransferSummary> {
    Ok(LearningScoreTransferSummary {
        id: row.id,
        learning_space_id: row.learning_space_id,
        learning_space_title: row.learning_space_title,
        class_group_name: row.class_group_name,
        subject_name: row.subject_name,
        source_type: parse_source_type(&row.source_type)?,
        source_id: row.source_id,
        source_title: row.source_title_snapshot,
        source_version: row.source_version,
        target_mark_sheet_id: row.target_mark_sheet_id,
        target_mark_sheet_version: row.target_mark_sheet_version,
        target_assessment_name: row.target_assessment_name,
        target_maximum_marks: row.target_maximum_marks,
        status: parse_transfer_status(&row.status)?,
        version: row.version,
        ready_count: row.ready_count,
        missing_source_count: row.missing_source_count,
        target_already_marked_count: row.target_already_marked_count,
        proposed_by_id: row.proposed_by_id,
        proposed_by_name: row.proposed_by_name,
        proposed_at: row.proposed_at,
        reviewed_by_name: row.reviewed_by_name,
        reviewed_at: row.reviewed_at,
        review_reason: row.review_reason,
        applied_mark_sheet_version: row.applied_mark_sheet_version,
    })
}

fn row_response(row: TransferRow) -> LearningScoreTransferRowResponse {
    LearningScoreTransferRowResponse {
        id: row.id,
        target_mark_id: row.target_mark_id,
        enrolment_id: row.enrolment_id,
        learner_id: row.learner_id,
        learner_number: row.learner_number_snapshot,
        learner_name: row.learner_name_snapshot,
        target_mark_version: row.target_mark_version,
        source_score_basis_points: row.source_score_basis_points,
        proposed_marks_hundredths: row.proposed_marks_hundredths,
        outcome: row.outcome,
    }
}

fn parse_source_type(value: &str) -> Result<LearningScoreTransferSourceType> {
    match value {
        "assignment" => Ok(LearningScoreTransferSourceType::Assignment),
        "quiz" => Ok(LearningScoreTransferSourceType::Quiz),
        _ => bail!("Stored Learning score-transfer source type is invalid"),
    }
}

fn parse_transfer_status(value: &str) -> Result<LearningScoreTransferStatus> {
    match value {
        "pending" => Ok(LearningScoreTransferStatus::Pending),
        "applied" => Ok(LearningScoreTransferStatus::Applied),
        "rejected" => Ok(LearningScoreTransferStatus::Rejected),
        _ => bail!("Stored Learning score-transfer status is invalid"),
    }
}

const TRANSFER_ROWS_SELECT: &str = r#"
SELECT id,target_mark_id,enrolment_id,learner_id,learner_number_snapshot,
       learner_name_snapshot,target_mark_version,source_evidence_id,
       source_evidence_version,source_score_basis_points,
       proposed_marks_hundredths,outcome
  FROM learning_score_transfer_rows
 WHERE tenant_id=$1 AND proposal_id=$2
 ORDER BY learner_name_snapshot,learner_number_snapshot,id
"#;

const PROPOSAL_SUMMARY_SELECT: &str = r#"
SELECT proposal.id,proposal.learning_space_id,space.title AS learning_space_title,
       class_group.name AS class_group_name,subject.name AS subject_name,
       proposal.source_type,proposal.source_id,proposal.source_title_snapshot,
       proposal.source_version,proposal.target_mark_sheet_id,
       proposal.target_mark_sheet_version,component.name AS target_assessment_name,
       proposal.target_maximum_marks,proposal.status,proposal.version,
       (SELECT COUNT(*) FROM learning_score_transfer_rows row
         WHERE row.tenant_id=proposal.tenant_id AND row.proposal_id=proposal.id
           AND row.outcome='ready')::BIGINT AS ready_count,
       (SELECT COUNT(*) FROM learning_score_transfer_rows row
         WHERE row.tenant_id=proposal.tenant_id AND row.proposal_id=proposal.id
           AND row.outcome='missing_source')::BIGINT AS missing_source_count,
       (SELECT COUNT(*) FROM learning_score_transfer_rows row
         WHERE row.tenant_id=proposal.tenant_id AND row.proposal_id=proposal.id
           AND row.outcome='target_already_marked')::BIGINT AS target_already_marked_count,
       proposal.proposed_by AS proposed_by_id,proposer.full_name AS proposed_by_name,
       proposal.proposed_at,
       reviewer.full_name AS reviewed_by_name,proposal.reviewed_at,
       proposal.review_reason,proposal.applied_mark_sheet_version
  FROM learning_score_transfer_proposals proposal
  JOIN learning_spaces space
    ON space.id=proposal.learning_space_id AND space.tenant_id=proposal.tenant_id
  JOIN teaching_assignments assignment
    ON assignment.id=space.teaching_assignment_id AND assignment.tenant_id=space.tenant_id
  JOIN class_groups class_group
    ON class_group.id=assignment.class_group_id AND class_group.tenant_id=assignment.tenant_id
  JOIN subjects subject
    ON subject.id=assignment.subject_id AND subject.tenant_id=assignment.tenant_id
  JOIN teacher_profiles teacher
    ON teacher.id=assignment.teacher_profile_id AND teacher.tenant_id=assignment.tenant_id
  JOIN employees employee
    ON employee.id=teacher.employee_id AND employee.tenant_id=teacher.tenant_id
  JOIN assessment_mark_sheets sheet
    ON sheet.id=proposal.target_mark_sheet_id AND sheet.tenant_id=proposal.tenant_id
  JOIN assessment_components component
    ON component.id=sheet.assessment_component_id AND component.tenant_id=sheet.tenant_id
  JOIN users proposer
    ON proposer.id=proposal.proposed_by AND proposer.tenant_id=proposal.tenant_id
  LEFT JOIN users reviewer
    ON reviewer.id=proposal.reviewed_by AND reviewer.tenant_id=proposal.tenant_id
 WHERE proposal.tenant_id=$1
"#;

#[cfg(test)]
mod tests {
    use super::rounded_ratio;

    #[test]
    fn exact_score_scaling_rounds_half_up_without_floats() {
        assert_eq!(rounded_ratio(8_333, 10_000, 5_000), 4_167);
        assert_eq!(rounded_ratio(2_500, 10_000, 10_000), 2_500);
        assert_eq!(rounded_ratio(1, 3, 100), 33);
    }
}
