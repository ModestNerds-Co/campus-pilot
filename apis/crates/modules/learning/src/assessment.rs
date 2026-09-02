//! Quiz attempts and derived completion rules for class-linked Learning spaces.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Utc};
use cp_audit::{AuditActor, RequestContext};
use cp_sis::ops::EnrolmentOps;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;
use validator::Validate;

use crate::dtos::{
    CreateLearningQuizQuestionRequest, CreateLearningQuizRequest,
    DeleteLearningQuizQuestionRequest, LearningAccessScope, LearningCompletionEntry,
    LearningCompletionPage, LearningCompletionPolicyResponse, LearningCompletionPolicyStatus,
    LearningCompletionRequirementInput, LearningCompletionRequirementResponse,
    LearningCompletionRequirementType, LearningQuizAttemptAnswerResponse,
    LearningQuizAttemptListQuery, LearningQuizAttemptResponse, LearningQuizAttemptStatus,
    LearningQuizChoiceInput, LearningQuizChoiceResponse, LearningQuizListQuery,
    LearningQuizQuestionResponse, LearningQuizResponse, LearningQuizStatus,
    ReasonedLearningTransitionRequest, SaveLearningCompletionPolicyRequest,
    SaveLearningQuizAttemptRequest, SubmitLearningQuizAttemptRequest,
    UpdateLearningQuizQuestionRequest, UpdateLearningQuizRequest,
};
use crate::models::LearningSpaceRow;
use crate::ops::{
    LearningOps, append_evidence, ensure_can_author_space, person_actor_id, scope_allows_space,
    space_row,
};

const DEFAULT_PAGE: i64 = 1;
const DEFAULT_PER_PAGE: i64 = 25;
const MAX_PER_PAGE: i64 = 100;

#[derive(Debug, FromRow)]
struct QuizRow {
    id: Uuid,
    learning_unit_id: Uuid,
    learning_space_id: Uuid,
    position: i32,
    title: String,
    instructions: Option<String>,
    opens_at: Option<DateTime<Utc>>,
    closes_at: Option<DateTime<Utc>>,
    attempt_limit: i32,
    pass_score_basis_points: i32,
    status: String,
    version: i32,
    recipient_count: i64,
    submitted_attempt_count: i64,
    published_at: Option<DateTime<Utc>>,
    closed_at: Option<DateTime<Utc>>,
    close_reason: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct QuestionRow {
    id: Uuid,
    position: i32,
    prompt: String,
    points: i32,
    version: i32,
}

#[derive(Debug, FromRow)]
struct ChoiceRow {
    id: Uuid,
    learning_quiz_question_id: Uuid,
    position: i32,
    label: String,
    is_correct: bool,
}

#[derive(Debug, FromRow)]
struct AttemptRow {
    id: Uuid,
    learning_quiz_id: Uuid,
    learner_id: Uuid,
    enrolment_id: Uuid,
    attempt_number: i32,
    status: String,
    version: i32,
    started_at: DateTime<Utc>,
    submitted_at: Option<DateTime<Utc>>,
    total_points_snapshot: Option<i32>,
    earned_points_snapshot: Option<i32>,
    score_basis_points: Option<i32>,
    passed: Option<bool>,
}

#[derive(Debug, FromRow)]
struct CompletionPolicyRow {
    id: Uuid,
    learning_space_id: Uuid,
    status: String,
    version: i32,
    recipient_count: i64,
    published_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct CompletionRequirementRow {
    id: Uuid,
    position: i32,
    requirement_type: String,
    source_id: Uuid,
    minimum_score_basis_points: i32,
}

impl LearningOps {
    /// Lists quizzes through the parent space's current record scope.
    pub async fn list_quizzes(
        pool: &PgPool,
        tenant_id: Uuid,
        space_id: Uuid,
        scope: LearningAccessScope,
        query: &LearningQuizListQuery,
    ) -> Result<(Vec<LearningQuizResponse>, i64)> {
        let (space, author) = visible_quiz_space(pool, tenant_id, space_id, scope)
            .await?
            .ok_or_else(|| anyhow!("The Learning space was not found"))?;
        let page = query.page.unwrap_or(DEFAULT_PAGE).max(1);
        let per_page = query
            .per_page
            .unwrap_or(DEFAULT_PER_PAGE)
            .clamp(1, MAX_PER_PAGE);
        let offset = (page - 1) * per_page;
        let requested_status = query.status.map(LearningQuizStatus::as_str);
        let rows = sqlx::query_as::<_, QuizRow>(&format!(
            "{QUIZ_SELECT} AND unit.learning_space_id=$2 AND ($3::TEXT IS NULL OR quiz.status=$3) AND ($4 OR quiz.status <> 'draft') ORDER BY quiz.position,quiz.created_at,quiz.id LIMIT $5 OFFSET $6"
        ))
        .bind(tenant_id)
        .bind(space.id)
        .bind(requested_status)
        .bind(author)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("list Learning quizzes")?;
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM learning_quizzes quiz JOIN learning_units unit ON unit.id=quiz.learning_unit_id AND unit.tenant_id=quiz.tenant_id WHERE quiz.tenant_id=$1 AND unit.learning_space_id=$2 AND quiz.deleted_at IS NULL AND unit.deleted_at IS NULL AND ($3::TEXT IS NULL OR quiz.status=$3) AND ($4 OR quiz.status <> 'draft')",
        )
        .bind(tenant_id)
        .bind(space.id)
        .bind(requested_status)
        .bind(author)
        .fetch_one(pool)
        .await
        .context("count Learning quizzes")?;
        let learner_id = if author {
            None
        } else {
            self_quiz_recipient(pool, tenant_id, &space, scope)
                .await?
                .map(|(_, learner_id)| learner_id)
        };
        let mut quizzes = Vec::with_capacity(rows.len());
        for row in rows {
            quizzes.push(quiz_response(pool, tenant_id, row, author, learner_id).await?);
        }
        Ok((quizzes, total))
    }

    pub async fn get_quiz(
        pool: &PgPool,
        tenant_id: Uuid,
        quiz_id: Uuid,
        scope: LearningAccessScope,
    ) -> Result<Option<LearningQuizResponse>> {
        let Some(row) = quiz_row(pool, tenant_id, quiz_id).await? else {
            return Ok(None);
        };
        let Some((space, author)) =
            visible_quiz_space(pool, tenant_id, row.learning_space_id, scope).await?
        else {
            return Ok(None);
        };
        if !author && row.status == "draft" {
            return Ok(None);
        }
        let learner_id = if author {
            None
        } else {
            self_quiz_recipient(pool, tenant_id, &space, scope)
                .await?
                .map(|(_, learner_id)| learner_id)
        };
        quiz_response(pool, tenant_id, row, author, learner_id)
            .await
            .map(Some)
    }

    pub async fn create_quiz(
        pool: &PgPool,
        tenant_id: Uuid,
        unit_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateLearningQuizRequest,
    ) -> Result<Option<LearningQuizResponse>> {
        validate_quiz_window(request.opens_at, request.closes_at)?;
        let Some(space_id) = quiz_unit_space(pool, tenant_id, unit_id).await? else {
            return Ok(None);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool.begin().await.context("start Learning quiz creation")?;
        require_quiz_parent(&mut tx, tenant_id, unit_id, space_id).await?;
        let quiz_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO learning_quizzes (tenant_id,learning_unit_id,position,title,instructions,opens_at,closes_at,attempt_limit,pass_score_basis_points,created_by,updated_by) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$10) RETURNING id",
        )
        .bind(tenant_id).bind(unit_id).bind(request.position).bind(required("Quiz title", &request.title)?)
        .bind(optional(&request.instructions)).bind(request.opens_at).bind(request.closes_at)
        .bind(request.attempt_limit).bind(request.pass_score_basis_points).bind(actor_id)
        .fetch_one(&mut *tx).await.map_err(|error| database_error(error, "create Learning quiz"))?;
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "quiz",
            quiz_id,
            Some(space_id),
            "learning_quiz_created",
            "learning.quizzes.create",
            json!({"unit_id": unit_id, "position": request.position}),
        )
        .await?;
        tx.commit().await.context("commit Learning quiz creation")?;
        quiz_response_by_id(pool, tenant_id, quiz_id, true, None).await
    }

    pub async fn update_quiz(
        pool: &PgPool,
        tenant_id: Uuid,
        quiz_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateLearningQuizRequest,
    ) -> Result<Option<LearningQuizResponse>> {
        validate_quiz_window(request.opens_at, request.closes_at)?;
        let Some((_, space_id)) = quiz_owner(pool, tenant_id, quiz_id).await? else {
            return Ok(None);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool.begin().await.context("start Learning quiz update")?;
        let changed = sqlx::query_scalar::<_, Uuid>(
            "UPDATE learning_quizzes SET position=$4,title=$5,instructions=$6,opens_at=$7,closes_at=$8,attempt_limit=$9,pass_score_basis_points=$10,version=version+1,updated_by=$11 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='draft' AND deleted_at IS NULL RETURNING id",
        )
        .bind(tenant_id).bind(quiz_id).bind(request.expected_version).bind(request.position)
        .bind(required("Quiz title", &request.title)?).bind(optional(&request.instructions))
        .bind(request.opens_at).bind(request.closes_at).bind(request.attempt_limit)
        .bind(request.pass_score_basis_points).bind(actor_id)
        .fetch_optional(&mut *tx).await.map_err(|error| database_error(error, "update Learning quiz"))?;
        if changed.is_none() {
            return Ok(None);
        }
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "quiz",
            quiz_id,
            Some(space_id),
            "learning_quiz_updated",
            "learning.quizzes.update",
            json!({"expected_version": request.expected_version}),
        )
        .await?;
        tx.commit().await.context("commit Learning quiz update")?;
        quiz_response_by_id(pool, tenant_id, quiz_id, true, None).await
    }

    pub async fn create_quiz_question(
        pool: &PgPool,
        tenant_id: Uuid,
        quiz_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateLearningQuizQuestionRequest,
    ) -> Result<Option<LearningQuizQuestionResponse>> {
        validate_choices(&request.choices)?;
        let Some((_, space_id)) = quiz_owner(pool, tenant_id, quiz_id).await? else {
            return Ok(None);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning quiz question creation")?;
        require_draft_quiz(&mut tx, tenant_id, quiz_id).await?;
        let question_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO learning_quiz_questions (tenant_id,learning_quiz_id,position,prompt,points,created_by,updated_by) VALUES ($1,$2,$3,$4,$5,$6,$6) RETURNING id",
        ).bind(tenant_id).bind(quiz_id).bind(request.position).bind(required("Question prompt", &request.prompt)?)
         .bind(request.points).bind(actor_id).fetch_one(&mut *tx).await.map_err(|error| database_error(error, "create Learning quiz question"))?;
        insert_choices(&mut tx, tenant_id, question_id, actor_id, &request.choices).await?;
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "quiz_question",
            question_id,
            Some(space_id),
            "learning_quiz_question_created",
            "learning.quiz_questions.create",
            json!({"quiz_id": quiz_id, "position": request.position}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit Learning quiz question creation")?;
        question_response_by_id(pool, tenant_id, question_id, true).await
    }

    pub async fn update_quiz_question(
        pool: &PgPool,
        tenant_id: Uuid,
        question_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateLearningQuizQuestionRequest,
    ) -> Result<Option<LearningQuizQuestionResponse>> {
        validate_choices(&request.choices)?;
        let Some((quiz_id, space_id)) = question_owner(pool, tenant_id, question_id).await? else {
            return Ok(None);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning quiz question update")?;
        require_draft_quiz(&mut tx, tenant_id, quiz_id).await?;
        let changed = sqlx::query_scalar::<_, Uuid>(
            "UPDATE learning_quiz_questions SET position=$4,prompt=$5,points=$6,version=version+1,updated_by=$7 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND deleted_at IS NULL RETURNING id",
        ).bind(tenant_id).bind(question_id).bind(request.expected_version).bind(request.position)
         .bind(required("Question prompt", &request.prompt)?).bind(request.points).bind(actor_id)
         .fetch_optional(&mut *tx).await.map_err(|error| database_error(error, "update Learning quiz question"))?;
        if changed.is_none() {
            return Ok(None);
        }
        sqlx::query("UPDATE learning_quiz_choices SET deleted_at=NOW(),deleted_by=$3,updated_by=$3 WHERE tenant_id=$1 AND learning_quiz_question_id=$2 AND deleted_at IS NULL")
            .bind(tenant_id).bind(question_id).bind(actor_id).execute(&mut *tx).await.context("replace Learning quiz choices")?;
        insert_choices(&mut tx, tenant_id, question_id, actor_id, &request.choices).await?;
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "quiz_question",
            question_id,
            Some(space_id),
            "learning_quiz_question_updated",
            "learning.quiz_questions.update",
            json!({"quiz_id": quiz_id, "expected_version": request.expected_version}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit Learning quiz question update")?;
        question_response_by_id(pool, tenant_id, question_id, true).await
    }

    pub async fn delete_quiz_question(
        pool: &PgPool,
        tenant_id: Uuid,
        question_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &DeleteLearningQuizQuestionRequest,
    ) -> Result<bool> {
        let Some((quiz_id, space_id)) = question_owner(pool, tenant_id, question_id).await? else {
            return Ok(false);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning quiz question removal")?;
        require_draft_quiz(&mut tx, tenant_id, quiz_id).await?;
        let changed = sqlx::query_scalar::<_, Uuid>(
            "UPDATE learning_quiz_questions SET deleted_at=NOW(),deleted_by=$4,updated_by=$4,version=version+1 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND deleted_at IS NULL RETURNING id",
        ).bind(tenant_id).bind(question_id).bind(request.expected_version).bind(actor_id)
         .fetch_optional(&mut *tx).await.context("remove Learning quiz question")?;
        if changed.is_none() {
            return Ok(false);
        }
        sqlx::query("UPDATE learning_quiz_choices SET deleted_at=NOW(),deleted_by=$3,updated_by=$3 WHERE tenant_id=$1 AND learning_quiz_question_id=$2 AND deleted_at IS NULL")
            .bind(tenant_id).bind(question_id).bind(actor_id).execute(&mut *tx).await.context("remove Learning quiz choices")?;
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "quiz_question",
            question_id,
            Some(space_id),
            "learning_quiz_question_removed",
            "learning.quiz_questions.delete",
            json!({"quiz_id": quiz_id, "expected_version": request.expected_version}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit Learning quiz question removal")?;
        Ok(true)
    }

    pub async fn publish_quiz(
        pool: &PgPool,
        tenant_id: Uuid,
        quiz_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<LearningQuizResponse>> {
        let Some((_, space_id)) = quiz_owner(pool, tenant_id, quiz_id).await? else {
            return Ok(None);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning quiz publication")?;
        let state = sqlx::query_as::<_, (String, String, String, Uuid, Uuid)>(
            "SELECT quiz.status,unit.status,space.status,space.academic_year_id,space.class_group_id FROM learning_quizzes quiz JOIN learning_units unit ON unit.id=quiz.learning_unit_id AND unit.tenant_id=quiz.tenant_id JOIN learning_spaces space ON space.id=unit.learning_space_id AND space.tenant_id=unit.tenant_id WHERE quiz.tenant_id=$1 AND quiz.id=$2 AND quiz.deleted_at IS NULL AND unit.deleted_at IS NULL AND space.deleted_at IS NULL FOR UPDATE OF quiz,unit,space",
        ).bind(tenant_id).bind(quiz_id).fetch_optional(&mut *tx).await.context("lock Learning quiz publication")?;
        let Some((status, unit_status, space_status, academic_year_id, class_group_id)) = state
        else {
            return Ok(None);
        };
        if status != "draft" || unit_status != "published" || space_status != "published" {
            bail!("Only a draft quiz in published Learning content can be published");
        }
        validate_quiz_questions_for_publication(&mut tx, tenant_id, quiz_id).await?;
        let roster = EnrolmentOps::class_roster_on(
            pool,
            tenant_id,
            academic_year_id,
            class_group_id,
            Utc::now().date_naive(),
        )
        .await?;
        if roster.is_empty() {
            bail!("The quiz cannot be published without eligible learners");
        }
        for recipient in &roster {
            sqlx::query("INSERT INTO learning_quiz_recipients (tenant_id,learning_quiz_id,enrolment_id,learner_id) VALUES ($1,$2,$3,$4)")
                .bind(tenant_id).bind(quiz_id).bind(recipient.enrolment_id).bind(recipient.learner_id)
                .execute(&mut *tx).await.context("snapshot Learning quiz recipient")?;
        }
        let changed = sqlx::query_scalar::<_, Uuid>(
            "UPDATE learning_quizzes SET status='published',published_by=$4,published_at=NOW(),version=version+1,updated_by=$4 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='draft' AND deleted_at IS NULL RETURNING id",
        ).bind(tenant_id).bind(quiz_id).bind(expected_version).bind(actor_id)
         .fetch_optional(&mut *tx).await.context("publish Learning quiz")?;
        if changed.is_none() {
            return Ok(None);
        }
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "quiz",
            quiz_id,
            Some(space_id),
            "learning_quiz_published",
            "learning.quizzes.publish",
            json!({"recipient_count": roster.len(), "expected_version": expected_version}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit Learning quiz publication")?;
        quiz_response_by_id(pool, tenant_id, quiz_id, true, None).await
    }

    pub async fn close_quiz(
        pool: &PgPool,
        tenant_id: Uuid,
        quiz_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ReasonedLearningTransitionRequest,
    ) -> Result<Option<LearningQuizResponse>> {
        let Some((_, space_id)) = quiz_owner(pool, tenant_id, quiz_id).await? else {
            return Ok(None);
        };
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let reason = required("Close reason", &request.reason)?;
        let mut tx = pool.begin().await.context("start Learning quiz closure")?;
        let changed = sqlx::query_scalar::<_, Uuid>(
            "UPDATE learning_quizzes SET status='closed',closed_by=$4,closed_at=NOW(),close_reason=$5,version=version+1,updated_by=$4 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='published' AND deleted_at IS NULL RETURNING id",
        ).bind(tenant_id).bind(quiz_id).bind(request.expected_version).bind(actor_id).bind(reason)
         .fetch_optional(&mut *tx).await.context("close Learning quiz")?;
        if changed.is_none() {
            return Ok(None);
        }
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "quiz",
            quiz_id,
            Some(space_id),
            "learning_quiz_closed",
            "learning.quizzes.close",
            json!({"reason": reason, "expected_version": request.expected_version}),
        )
        .await?;
        tx.commit().await.context("commit Learning quiz closure")?;
        quiz_response_by_id(pool, tenant_id, quiz_id, true, None).await
    }

    /// Starts or resumes the authenticated learner's current quiz attempt.
    pub async fn start_quiz_attempt(
        pool: &PgPool,
        tenant_id: Uuid,
        quiz_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
    ) -> Result<Option<LearningQuizAttemptResponse>> {
        let Some(row) = quiz_row(pool, tenant_id, quiz_id).await? else {
            return Ok(None);
        };
        let Some(space) = space_row(pool, tenant_id, row.learning_space_id).await? else {
            return Ok(None);
        };
        if !scope_allows_space(pool, tenant_id, &space, scope).await? {
            return Ok(None);
        }
        let Some((recipient_id, _)) =
            self_quiz_recipient_id(pool, tenant_id, quiz_id, &space, scope).await?
        else {
            return Ok(None);
        };
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool.begin().await.context("start Learning quiz attempt")?;
        let quiz_state = sqlx::query_as::<_, (String, Option<DateTime<Utc>>, Option<DateTime<Utc>>, i32)>(
            "SELECT status,opens_at,closes_at,attempt_limit FROM learning_quizzes WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL FOR UPDATE",
        ).bind(tenant_id).bind(quiz_id).fetch_optional(&mut *tx).await.context("lock Learning quiz attempt")?;
        let Some((status, opens_at, closes_at, attempt_limit)) = quiz_state else {
            return Ok(None);
        };
        let now = Utc::now();
        if status != "published"
            || opens_at.is_some_and(|value| value > now)
            || closes_at.is_some_and(|value| value <= now)
        {
            bail!("The Learning quiz is not open for attempts");
        }
        if let Some(existing_id) = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM learning_quiz_attempts WHERE tenant_id=$1 AND learning_quiz_id=$2 AND quiz_recipient_id=$3 AND status='in_progress' ORDER BY attempt_number DESC LIMIT 1",
        ).bind(tenant_id).bind(quiz_id).bind(recipient_id).fetch_optional(&mut *tx).await.context("resume Learning quiz attempt")? {
            tx.commit().await.context("finish Learning quiz attempt resume")?;
            return attempt_response_by_id(pool, tenant_id, existing_id).await;
        }
        let attempt_number = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM learning_quiz_attempts WHERE tenant_id=$1 AND learning_quiz_id=$2 AND quiz_recipient_id=$3",
        ).bind(tenant_id).bind(quiz_id).bind(recipient_id).fetch_one(&mut *tx).await.context("count Learning quiz attempts")? + 1;
        if attempt_number > i64::from(attempt_limit) {
            bail!("The Learning quiz attempt limit has been reached");
        }
        let attempt_number = i32::try_from(attempt_number)
            .context("Learning quiz attempt number exceeded its supported range")?;
        let attempt_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO learning_quiz_attempts (tenant_id,learning_quiz_id,quiz_recipient_id,attempt_number,started_by,updated_by) VALUES ($1,$2,$3,$4,$5,$5) RETURNING id",
        ).bind(tenant_id).bind(quiz_id).bind(recipient_id).bind(attempt_number).bind(actor_id)
         .fetch_one(&mut *tx).await.context("create Learning quiz attempt")?;
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "quiz_attempt",
            attempt_id,
            Some(space.id),
            "learning_quiz_attempt_started",
            "learning.quiz_attempts.start",
            json!({"quiz_id": quiz_id, "attempt_number": attempt_number}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit Learning quiz attempt start")?;
        attempt_response_by_id(pool, tenant_id, attempt_id).await
    }

    pub async fn save_quiz_attempt(
        pool: &PgPool,
        tenant_id: Uuid,
        attempt_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &SaveLearningQuizAttemptRequest,
    ) -> Result<Option<LearningQuizAttemptResponse>> {
        let Some((space, recipient_id)) =
            self_attempt_context(pool, tenant_id, attempt_id, scope).await?
        else {
            return Ok(None);
        };
        let actor_id = person_actor_id(actor)?;
        let mut unique = BTreeSet::new();
        for answer in &request.answers {
            if !unique.insert(answer.question_id) {
                bail!("A quiz question can only be answered once");
            }
        }
        let mut tx = pool
            .begin()
            .await
            .context("start Learning quiz attempt save")?;
        let quiz_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT learning_quiz_id FROM learning_quiz_attempts WHERE tenant_id=$1 AND id=$2 AND quiz_recipient_id=$3 AND version=$4 AND status='in_progress' FOR UPDATE",
        ).bind(tenant_id).bind(attempt_id).bind(recipient_id).bind(request.expected_version)
         .fetch_optional(&mut *tx).await.context("lock Learning quiz attempt save")?;
        let Some(quiz_id) = quiz_id else {
            return Ok(None);
        };
        validate_attempt_answers(&mut tx, tenant_id, quiz_id, &request.answers).await?;
        sqlx::query("DELETE FROM learning_quiz_attempt_answers WHERE tenant_id=$1 AND learning_quiz_attempt_id=$2")
            .bind(tenant_id).bind(attempt_id).execute(&mut *tx).await.context("replace Learning quiz answers")?;
        for answer in &request.answers {
            sqlx::query("INSERT INTO learning_quiz_attempt_answers (tenant_id,learning_quiz_attempt_id,learning_quiz_question_id,selected_choice_id,created_by,updated_by) VALUES ($1,$2,$3,$4,$5,$5)")
                .bind(tenant_id).bind(attempt_id).bind(answer.question_id).bind(answer.selected_choice_id).bind(actor_id)
                .execute(&mut *tx).await.context("save Learning quiz answer")?;
        }
        sqlx::query("UPDATE learning_quiz_attempts SET version=version+1,updated_by=$3 WHERE tenant_id=$1 AND id=$2")
            .bind(tenant_id).bind(attempt_id).bind(actor_id).execute(&mut *tx).await.context("advance Learning quiz attempt")?;
        append_evidence(&mut tx, tenant_id, actor, request_context, "quiz_attempt", attempt_id, Some(space.id),
            "learning_quiz_attempt_saved", "learning.quiz_attempts.save", json!({"answer_count": request.answers.len(), "expected_version": request.expected_version})).await?;
        tx.commit()
            .await
            .context("commit Learning quiz attempt save")?;
        attempt_response_by_id(pool, tenant_id, attempt_id).await
    }

    pub async fn submit_quiz_attempt(
        pool: &PgPool,
        tenant_id: Uuid,
        attempt_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &SubmitLearningQuizAttemptRequest,
    ) -> Result<Option<LearningQuizAttemptResponse>> {
        let Some((space, recipient_id)) =
            self_attempt_context(pool, tenant_id, attempt_id, scope).await?
        else {
            return Ok(None);
        };
        let fingerprint = attempt_fingerprint(attempt_id, request.expected_version);
        if let Some((stored_key, stored_fingerprint)) = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT idempotency_key,request_fingerprint FROM learning_quiz_attempts WHERE tenant_id=$1 AND id=$2 AND quiz_recipient_id=$3 AND status='submitted'",
        ).bind(tenant_id).bind(attempt_id).bind(recipient_id).fetch_optional(pool).await.context("reconcile Learning quiz attempt")? {
            if stored_key == request.idempotency_key && stored_fingerprint == fingerprint {
                return attempt_response_by_id(pool, tenant_id, attempt_id).await;
            }
            bail!("The Learning quiz attempt was already submitted with different evidence");
        }
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning quiz attempt submission")?;
        let state = sqlx::query_as::<_, (Uuid, i32, String, Option<DateTime<Utc>>)>(
            "SELECT attempt.learning_quiz_id,quiz.pass_score_basis_points,quiz.status,quiz.closes_at FROM learning_quiz_attempts attempt JOIN learning_quizzes quiz ON quiz.id=attempt.learning_quiz_id AND quiz.tenant_id=attempt.tenant_id WHERE attempt.tenant_id=$1 AND attempt.id=$2 AND attempt.quiz_recipient_id=$3 AND attempt.version=$4 AND attempt.status='in_progress' FOR UPDATE OF attempt,quiz",
        ).bind(tenant_id).bind(attempt_id).bind(recipient_id).bind(request.expected_version)
         .fetch_optional(&mut *tx).await.context("lock Learning quiz attempt submission")?;
        let Some((quiz_id, pass_score, quiz_status, closes_at)) = state else {
            return Ok(None);
        };
        if quiz_status != "published" || closes_at.is_some_and(|value| value <= Utc::now()) {
            bail!("The Learning quiz is no longer accepting attempts");
        }
        let (question_count, answered_count, total_points, earned_points) = sqlx::query_as::<_, (i64, i64, i64, i64)>(
            "SELECT COUNT(question.id),COUNT(answer.learning_quiz_question_id),COALESCE(SUM(question.points),0)::BIGINT,COALESCE(SUM(CASE WHEN choice.is_correct THEN question.points ELSE 0 END),0)::BIGINT FROM learning_quiz_questions question LEFT JOIN learning_quiz_attempt_answers answer ON answer.tenant_id=question.tenant_id AND answer.learning_quiz_question_id=question.id AND answer.learning_quiz_attempt_id=$3 LEFT JOIN learning_quiz_choices choice ON choice.tenant_id=answer.tenant_id AND choice.id=answer.selected_choice_id AND choice.deleted_at IS NULL WHERE question.tenant_id=$1 AND question.learning_quiz_id=$2 AND question.deleted_at IS NULL",
        ).bind(tenant_id).bind(quiz_id).bind(attempt_id).fetch_one(&mut *tx).await.context("score Learning quiz attempt")?;
        if question_count == 0 || answered_count != question_count {
            bail!("Answer every quiz question before submitting");
        }
        let total_points = i32::try_from(total_points)
            .context("Learning quiz points exceeded their supported range")?;
        let earned_points = i32::try_from(earned_points)
            .context("Learning quiz score exceeded its supported range")?;
        let score_basis_points = if total_points == 0 {
            0
        } else {
            (earned_points * 10_000) / total_points
        };
        let passed = score_basis_points >= pass_score;
        let changed = sqlx::query_scalar::<_, Uuid>(
            "UPDATE learning_quiz_attempts SET status='submitted',submitted_at=NOW(),total_points_snapshot=$5,earned_points_snapshot=$6,score_basis_points=$7,passed=$8,idempotency_key=$9,request_fingerprint=$10,version=version+1,updated_by=$11 WHERE tenant_id=$1 AND id=$2 AND quiz_recipient_id=$3 AND version=$4 AND status='in_progress' RETURNING id",
        ).bind(tenant_id).bind(attempt_id).bind(recipient_id).bind(request.expected_version)
         .bind(total_points).bind(earned_points).bind(score_basis_points).bind(passed)
         .bind(request.idempotency_key).bind(&fingerprint).bind(actor_id)
         .fetch_optional(&mut *tx).await.context("submit Learning quiz attempt")?;
        if changed.is_none() {
            return Ok(None);
        }
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "quiz_attempt",
            attempt_id,
            Some(space.id),
            "learning_quiz_attempt_submitted",
            "learning.quiz_attempts.submit",
            json!({"quiz_id": quiz_id, "score_basis_points": score_basis_points, "passed": passed}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit Learning quiz attempt submission")?;
        attempt_response_by_id(pool, tenant_id, attempt_id).await
    }

    pub async fn list_quiz_attempts(
        pool: &PgPool,
        tenant_id: Uuid,
        quiz_id: Uuid,
        scope: LearningAccessScope,
        query: &LearningQuizAttemptListQuery,
    ) -> Result<(Vec<LearningQuizAttemptResponse>, i64)> {
        let Some(quiz) = quiz_row(pool, tenant_id, quiz_id).await? else {
            return Ok((Vec::new(), 0));
        };
        let Some((space, author)) =
            visible_quiz_space(pool, tenant_id, quiz.learning_space_id, scope).await?
        else {
            return Ok((Vec::new(), 0));
        };
        let learner_id = if author {
            None
        } else {
            self_quiz_recipient(pool, tenant_id, &space, scope)
                .await?
                .map(|(_, id)| id)
        };
        let page = query.page.unwrap_or(DEFAULT_PAGE).max(1);
        let per_page = query
            .per_page
            .unwrap_or(DEFAULT_PER_PAGE)
            .clamp(1, MAX_PER_PAGE);
        let status = query.status.map(LearningQuizAttemptStatus::as_str);
        let rows = sqlx::query_as::<_, AttemptRow>(&format!(
            "{ATTEMPT_SELECT} AND attempt.learning_quiz_id=$2 AND ($3::UUID IS NULL OR recipient.learner_id=$3) AND ($4::TEXT IS NULL OR attempt.status=$4) ORDER BY attempt.started_at DESC,attempt.id DESC LIMIT $5 OFFSET $6"
        )).bind(tenant_id).bind(quiz_id).bind(learner_id).bind(status).bind(per_page).bind((page - 1) * per_page)
          .fetch_all(pool).await.context("list Learning quiz attempts")?;
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM learning_quiz_attempts attempt JOIN learning_quiz_recipients recipient ON recipient.id=attempt.quiz_recipient_id AND recipient.tenant_id=attempt.tenant_id WHERE attempt.tenant_id=$1 AND attempt.learning_quiz_id=$2 AND ($3::UUID IS NULL OR recipient.learner_id=$3) AND ($4::TEXT IS NULL OR attempt.status=$4)",
        ).bind(tenant_id).bind(quiz_id).bind(learner_id).bind(status).fetch_one(pool).await.context("count Learning quiz attempts")?;
        let mut attempts = Vec::with_capacity(rows.len());
        for row in rows {
            attempts.push(attempt_response(pool, tenant_id, row).await?);
        }
        Ok((attempts, total))
    }

    pub async fn get_quiz_attempt(
        pool: &PgPool,
        tenant_id: Uuid,
        attempt_id: Uuid,
        scope: LearningAccessScope,
    ) -> Result<Option<LearningQuizAttemptResponse>> {
        let Some(row) = attempt_row(pool, tenant_id, attempt_id).await? else {
            return Ok(None);
        };
        let Some(quiz) = quiz_row(pool, tenant_id, row.learning_quiz_id).await? else {
            return Ok(None);
        };
        let Some((space, author)) =
            visible_quiz_space(pool, tenant_id, quiz.learning_space_id, scope).await?
        else {
            return Ok(None);
        };
        if !author {
            let learner_id = self_quiz_recipient(pool, tenant_id, &space, scope)
                .await?
                .map(|(_, id)| id);
            if learner_id != Some(row.learner_id) {
                return Ok(None);
            }
        }
        attempt_response(pool, tenant_id, row).await.map(Some)
    }

    /// Loads the editable draft for teachers, otherwise the published policy.
    pub async fn completion_policy(
        pool: &PgPool,
        tenant_id: Uuid,
        space_id: Uuid,
        scope: LearningAccessScope,
    ) -> Result<Option<LearningCompletionPolicyResponse>> {
        let Some((_, author)) = visible_quiz_space(pool, tenant_id, space_id, scope).await? else {
            return Ok(None);
        };
        let row = completion_policy_row(pool, tenant_id, space_id, author).await?;
        match row {
            Some(row) => completion_policy_response(pool, tenant_id, row)
                .await
                .map(Some),
            None => Ok(None),
        }
    }

    pub async fn save_completion_policy(
        pool: &PgPool,
        tenant_id: Uuid,
        space_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        request: &SaveLearningCompletionPolicyRequest,
    ) -> Result<LearningCompletionPolicyResponse> {
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        validate_completion_inputs(pool, tenant_id, space_id, &request.requirements, false).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning completion policy save")?;
        let existing = sqlx::query_as::<_, (Uuid, i32)>(
            "SELECT id,version FROM learning_completion_policies WHERE tenant_id=$1 AND learning_space_id=$2 AND status='draft' FOR UPDATE",
        ).bind(tenant_id).bind(space_id).fetch_optional(&mut *tx).await.context("lock Learning completion policy")?;
        let policy_id = match (existing, request.expected_version) {
            (None, None) => sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO learning_completion_policies (tenant_id,learning_space_id,created_by,updated_by) VALUES ($1,$2,$3,$3) RETURNING id",
            ).bind(tenant_id).bind(space_id).bind(actor_id).fetch_one(&mut *tx).await.context("create Learning completion policy")?,
            (Some((id, version)), Some(expected)) if version == expected => {
                sqlx::query("UPDATE learning_completion_policies SET version=version+1,updated_by=$3 WHERE tenant_id=$1 AND id=$2")
                    .bind(tenant_id).bind(id).bind(actor_id).execute(&mut *tx).await.context("advance Learning completion policy")?;
                sqlx::query("DELETE FROM learning_completion_requirements WHERE tenant_id=$1 AND completion_policy_id=$2")
                    .bind(tenant_id).bind(id).execute(&mut *tx).await.context("replace Learning completion requirements")?;
                id
            }
            _ => bail!("The Learning completion policy changed before this update"),
        };
        insert_completion_requirements(
            &mut tx,
            tenant_id,
            policy_id,
            actor_id,
            &request.requirements,
        )
        .await?;
        append_evidence(&mut tx, tenant_id, actor, request_context, "completion_policy", policy_id, Some(space_id),
            "learning_completion_policy_saved", "learning.completion_policy.save", json!({"requirement_count": request.requirements.len(), "expected_version": request.expected_version})).await?;
        tx.commit()
            .await
            .context("commit Learning completion policy save")?;
        completion_policy_by_id(pool, tenant_id, policy_id)
            .await?
            .ok_or_else(|| anyhow!("The Learning completion policy could not be reloaded"))
    }

    pub async fn publish_completion_policy(
        pool: &PgPool,
        tenant_id: Uuid,
        space_id: Uuid,
        scope: LearningAccessScope,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<LearningCompletionPolicyResponse>> {
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let actor_id = person_actor_id(actor)?;
        let mut tx = pool
            .begin()
            .await
            .context("start Learning completion policy publication")?;
        let state = sqlx::query_as::<_, (Uuid, Uuid, Uuid)>(
            "SELECT policy.id,space.academic_year_id,space.class_group_id FROM learning_completion_policies policy JOIN learning_spaces space ON space.id=policy.learning_space_id AND space.tenant_id=policy.tenant_id WHERE policy.tenant_id=$1 AND policy.learning_space_id=$2 AND policy.status='draft' AND policy.version=$3 AND space.status='published' AND space.deleted_at IS NULL FOR UPDATE OF policy,space",
        ).bind(tenant_id).bind(space_id).bind(expected_version).fetch_optional(&mut *tx).await.context("lock Learning completion policy publication")?;
        let Some((policy_id, academic_year_id, class_group_id)) = state else {
            return Ok(None);
        };
        let requirements = completion_requirement_inputs(&mut tx, tenant_id, policy_id).await?;
        validate_completion_inputs(pool, tenant_id, space_id, &requirements, true).await?;
        let roster = EnrolmentOps::class_roster_on(
            pool,
            tenant_id,
            academic_year_id,
            class_group_id,
            Utc::now().date_naive(),
        )
        .await?;
        if roster.is_empty() {
            bail!("The completion policy cannot be published without eligible learners");
        }
        sqlx::query("UPDATE learning_completion_policies SET status='superseded',superseded_at=NOW(),updated_by=$3,version=version+1 WHERE tenant_id=$1 AND learning_space_id=$2 AND status='published'")
            .bind(tenant_id).bind(space_id).bind(actor_id).execute(&mut *tx).await.context("supersede Learning completion policy")?;
        for recipient in &roster {
            sqlx::query("INSERT INTO learning_completion_recipients (tenant_id,completion_policy_id,enrolment_id,learner_id) VALUES ($1,$2,$3,$4)")
                .bind(tenant_id).bind(policy_id).bind(recipient.enrolment_id).bind(recipient.learner_id)
                .execute(&mut *tx).await.context("snapshot Learning completion recipient")?;
        }
        let changed = sqlx::query_scalar::<_, Uuid>(
            "UPDATE learning_completion_policies SET status='published',published_by=$4,published_at=NOW(),version=version+1,updated_by=$4 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='draft' RETURNING id",
        ).bind(tenant_id).bind(policy_id).bind(expected_version).bind(actor_id).fetch_optional(&mut *tx).await.context("publish Learning completion policy")?;
        if changed.is_none() {
            return Ok(None);
        }
        append_evidence(
            &mut tx,
            tenant_id,
            actor,
            request_context,
            "completion_policy",
            policy_id,
            Some(space_id),
            "learning_completion_policy_published",
            "learning.completion_policy.publish",
            json!({"recipient_count": roster.len(), "requirement_count": requirements.len()}),
        )
        .await?;
        tx.commit()
            .await
            .context("commit Learning completion policy publication")?;
        completion_policy_by_id(pool, tenant_id, policy_id).await
    }

    pub async fn self_completion(
        pool: &PgPool,
        tenant_id: Uuid,
        space_id: Uuid,
        scope: LearningAccessScope,
    ) -> Result<LearningCompletionPage> {
        let Some((space, author)) = visible_quiz_space(pool, tenant_id, space_id, scope).await?
        else {
            return Ok(LearningCompletionPage {
                policy: None,
                progress: Vec::new(),
            });
        };
        if author {
            bail!("Learner self scope is required for personal completion");
        }
        let policy = published_completion_policy(pool, tenant_id, space_id).await?;
        let Some(policy) = policy else {
            return Ok(LearningCompletionPage {
                policy: None,
                progress: Vec::new(),
            });
        };
        let recipient = self_quiz_recipient(pool, tenant_id, &space, scope).await?;
        let progress = match recipient {
            Some((_, learner_id)) => {
                completion_entries(pool, tenant_id, policy.id, Some(learner_id)).await?
            }
            None => Vec::new(),
        };
        Ok(LearningCompletionPage {
            policy: Some(policy),
            progress,
        })
    }

    pub async fn list_completion(
        pool: &PgPool,
        tenant_id: Uuid,
        space_id: Uuid,
        scope: LearningAccessScope,
    ) -> Result<LearningCompletionPage> {
        ensure_can_author_space(pool, tenant_id, space_id, scope).await?;
        let policy = published_completion_policy(pool, tenant_id, space_id).await?;
        let Some(policy) = policy else {
            return Ok(LearningCompletionPage {
                policy: None,
                progress: Vec::new(),
            });
        };
        let progress = completion_entries(pool, tenant_id, policy.id, None).await?;
        Ok(LearningCompletionPage {
            policy: Some(policy),
            progress,
        })
    }
}

const QUIZ_SELECT: &str = r#"
SELECT quiz.id,quiz.learning_unit_id,unit.learning_space_id,quiz.position,quiz.title,
       quiz.instructions,quiz.opens_at,quiz.closes_at,quiz.attempt_limit,
       quiz.pass_score_basis_points,quiz.status,quiz.version,quiz.published_at,
       quiz.closed_at,quiz.close_reason,quiz.created_at,quiz.updated_at,
       (SELECT COUNT(*) FROM learning_quiz_recipients recipient
         WHERE recipient.tenant_id=quiz.tenant_id AND recipient.learning_quiz_id=quiz.id)::BIGINT AS recipient_count,
       (SELECT COUNT(*) FROM learning_quiz_attempts attempt
         WHERE attempt.tenant_id=quiz.tenant_id AND attempt.learning_quiz_id=quiz.id
           AND attempt.status='submitted')::BIGINT AS submitted_attempt_count
  FROM learning_quizzes quiz
  JOIN learning_units unit ON unit.id=quiz.learning_unit_id AND unit.tenant_id=quiz.tenant_id
 WHERE quiz.tenant_id=$1 AND quiz.deleted_at IS NULL AND unit.deleted_at IS NULL
"#;

const ATTEMPT_SELECT: &str = r#"
SELECT attempt.id,attempt.learning_quiz_id,recipient.learner_id,recipient.enrolment_id,
       attempt.attempt_number,attempt.status,attempt.version,attempt.started_at,
       attempt.submitted_at,attempt.total_points_snapshot,attempt.earned_points_snapshot,
       attempt.score_basis_points,attempt.passed
  FROM learning_quiz_attempts attempt
  JOIN learning_quiz_recipients recipient
    ON recipient.id=attempt.quiz_recipient_id AND recipient.tenant_id=attempt.tenant_id
 WHERE attempt.tenant_id=$1
"#;

async fn visible_quiz_space(
    pool: &PgPool,
    tenant_id: Uuid,
    space_id: Uuid,
    scope: LearningAccessScope,
) -> Result<Option<(LearningSpaceRow, bool)>> {
    let Some(space) = space_row(pool, tenant_id, space_id).await? else {
        return Ok(None);
    };
    if !scope_allows_space(pool, tenant_id, &space, scope).await? {
        return Ok(None);
    }
    let author = match scope {
        LearningAccessScope::Campus | LearningAccessScope::AssignedTo(_) => true,
        LearningAccessScope::SelfFor(_) => false,
        LearningAccessScope::SelfAndAssigned(_) => {
            ensure_can_author_space(pool, tenant_id, space_id, scope)
                .await
                .is_ok()
        }
    };
    Ok(Some((space, author)))
}

async fn quiz_unit_space(pool: &PgPool, tenant_id: Uuid, unit_id: Uuid) -> Result<Option<Uuid>> {
    sqlx::query_scalar("SELECT learning_space_id FROM learning_units WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL")
        .bind(tenant_id).bind(unit_id).fetch_optional(pool).await.context("resolve Learning quiz unit")
}

async fn quiz_owner(pool: &PgPool, tenant_id: Uuid, quiz_id: Uuid) -> Result<Option<(Uuid, Uuid)>> {
    sqlx::query_as("SELECT quiz.learning_unit_id,unit.learning_space_id FROM learning_quizzes quiz JOIN learning_units unit ON unit.id=quiz.learning_unit_id AND unit.tenant_id=quiz.tenant_id WHERE quiz.tenant_id=$1 AND quiz.id=$2 AND quiz.deleted_at IS NULL AND unit.deleted_at IS NULL")
        .bind(tenant_id).bind(quiz_id).fetch_optional(pool).await.context("resolve Learning quiz owner")
}

async fn question_owner(
    pool: &PgPool,
    tenant_id: Uuid,
    question_id: Uuid,
) -> Result<Option<(Uuid, Uuid)>> {
    sqlx::query_as("SELECT question.learning_quiz_id,unit.learning_space_id FROM learning_quiz_questions question JOIN learning_quizzes quiz ON quiz.id=question.learning_quiz_id AND quiz.tenant_id=question.tenant_id JOIN learning_units unit ON unit.id=quiz.learning_unit_id AND unit.tenant_id=quiz.tenant_id WHERE question.tenant_id=$1 AND question.id=$2 AND question.deleted_at IS NULL AND quiz.deleted_at IS NULL AND unit.deleted_at IS NULL")
        .bind(tenant_id).bind(question_id).fetch_optional(pool).await.context("resolve Learning quiz question owner")
}

async fn quiz_row(pool: &PgPool, tenant_id: Uuid, quiz_id: Uuid) -> Result<Option<QuizRow>> {
    sqlx::query_as::<_, QuizRow>(&format!("{QUIZ_SELECT} AND quiz.id=$2"))
        .bind(tenant_id)
        .bind(quiz_id)
        .fetch_optional(pool)
        .await
        .context("load Learning quiz")
}

async fn quiz_response_by_id(
    pool: &PgPool,
    tenant_id: Uuid,
    quiz_id: Uuid,
    answer_key: bool,
    learner_id: Option<Uuid>,
) -> Result<Option<LearningQuizResponse>> {
    match quiz_row(pool, tenant_id, quiz_id).await? {
        Some(row) => quiz_response(pool, tenant_id, row, answer_key, learner_id)
            .await
            .map(Some),
        None => Ok(None),
    }
}

async fn quiz_response(
    pool: &PgPool,
    tenant_id: Uuid,
    row: QuizRow,
    answer_key: bool,
    learner_id: Option<Uuid>,
) -> Result<LearningQuizResponse> {
    let question_rows = sqlx::query_as::<_, QuestionRow>(
        "SELECT id,position,prompt,points,version FROM learning_quiz_questions WHERE tenant_id=$1 AND learning_quiz_id=$2 AND deleted_at IS NULL ORDER BY position,created_at,id",
    ).bind(tenant_id).bind(row.id).fetch_all(pool).await.context("load Learning quiz questions")?;
    let mut questions = Vec::with_capacity(question_rows.len());
    for question in question_rows {
        questions.push(question_response(pool, tenant_id, question, answer_key).await?);
    }
    let my_attempt_count = match learner_id {
        Some(learner_id) => sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM learning_quiz_attempts attempt JOIN learning_quiz_recipients recipient ON recipient.id=attempt.quiz_recipient_id AND recipient.tenant_id=attempt.tenant_id WHERE attempt.tenant_id=$1 AND attempt.learning_quiz_id=$2 AND recipient.learner_id=$3",
        ).bind(tenant_id).bind(row.id).bind(learner_id).fetch_one(pool).await.context("count learner quiz attempts")?,
        None => 0,
    };
    Ok(LearningQuizResponse {
        id: row.id,
        learning_unit_id: row.learning_unit_id,
        learning_space_id: row.learning_space_id,
        position: row.position,
        title: row.title,
        instructions: row.instructions,
        opens_at: row.opens_at,
        closes_at: row.closes_at,
        attempt_limit: row.attempt_limit,
        pass_score_basis_points: row.pass_score_basis_points,
        status: parse_quiz_status(&row.status)?,
        version: row.version,
        recipient_count: row.recipient_count,
        submitted_attempt_count: row.submitted_attempt_count,
        my_attempt_count,
        questions,
        published_at: row.published_at,
        closed_at: row.closed_at,
        close_reason: row.close_reason,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

async fn question_response_by_id(
    pool: &PgPool,
    tenant_id: Uuid,
    question_id: Uuid,
    answer_key: bool,
) -> Result<Option<LearningQuizQuestionResponse>> {
    let row = sqlx::query_as::<_, QuestionRow>("SELECT id,position,prompt,points,version FROM learning_quiz_questions WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL")
        .bind(tenant_id).bind(question_id).fetch_optional(pool).await.context("load Learning quiz question")?;
    match row {
        Some(row) => question_response(pool, tenant_id, row, answer_key)
            .await
            .map(Some),
        None => Ok(None),
    }
}

async fn question_response(
    pool: &PgPool,
    tenant_id: Uuid,
    row: QuestionRow,
    answer_key: bool,
) -> Result<LearningQuizQuestionResponse> {
    let choices = sqlx::query_as::<_, ChoiceRow>("SELECT id,learning_quiz_question_id,position,label,is_correct FROM learning_quiz_choices WHERE tenant_id=$1 AND learning_quiz_question_id=$2 AND deleted_at IS NULL ORDER BY position,created_at,id")
        .bind(tenant_id).bind(row.id).fetch_all(pool).await.context("load Learning quiz choices")?
        .into_iter().map(|choice| {
            debug_assert_eq!(choice.learning_quiz_question_id, row.id);
            LearningQuizChoiceResponse { id: choice.id, position: choice.position, label: choice.label, is_correct: answer_key.then_some(choice.is_correct) }
        }).collect();
    Ok(LearningQuizQuestionResponse {
        id: row.id,
        position: row.position,
        prompt: row.prompt,
        points: row.points,
        version: row.version,
        choices,
    })
}

async fn self_quiz_recipient(
    pool: &PgPool,
    tenant_id: Uuid,
    space: &LearningSpaceRow,
    scope: LearningAccessScope,
) -> Result<Option<(Uuid, Uuid)>> {
    let account_id = match scope {
        LearningAccessScope::SelfFor(id) | LearningAccessScope::SelfAndAssigned(id) => id,
        LearningAccessScope::Campus | LearningAccessScope::AssignedTo(_) => return Ok(None),
    };
    let roster = EnrolmentOps::active_roster_entry_for_account(
        pool,
        tenant_id,
        account_id,
        space.academic_year_id,
        space.class_group_id,
    )
    .await?;
    Ok(roster.map(|entry| (entry.enrolment_id, entry.learner_id)))
}

async fn self_quiz_recipient_id(
    pool: &PgPool,
    tenant_id: Uuid,
    quiz_id: Uuid,
    space: &LearningSpaceRow,
    scope: LearningAccessScope,
) -> Result<Option<(Uuid, Uuid)>> {
    let Some((enrolment_id, learner_id)) =
        self_quiz_recipient(pool, tenant_id, space, scope).await?
    else {
        return Ok(None);
    };
    let recipient_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM learning_quiz_recipients WHERE tenant_id=$1 AND learning_quiz_id=$2 AND enrolment_id=$3 AND learner_id=$4",
    )
    .bind(tenant_id)
    .bind(quiz_id)
    .bind(enrolment_id)
    .bind(learner_id)
    .fetch_optional(pool)
    .await
    .context("resolve Learning quiz recipient")?;
    Ok(recipient_id.map(|id| (id, learner_id)))
}

async fn self_attempt_context(
    pool: &PgPool,
    tenant_id: Uuid,
    attempt_id: Uuid,
    scope: LearningAccessScope,
) -> Result<Option<(LearningSpaceRow, Uuid)>> {
    let Some(row) = attempt_row(pool, tenant_id, attempt_id).await? else {
        return Ok(None);
    };
    let Some(quiz) = quiz_row(pool, tenant_id, row.learning_quiz_id).await? else {
        return Ok(None);
    };
    let Some(space) = space_row(pool, tenant_id, quiz.learning_space_id).await? else {
        return Ok(None);
    };
    if !scope_allows_space(pool, tenant_id, &space, scope).await? {
        return Ok(None);
    }
    let Some((_, learner_id)) = self_quiz_recipient(pool, tenant_id, &space, scope).await? else {
        return Ok(None);
    };
    if learner_id != row.learner_id {
        return Ok(None);
    }
    let recipient_id = sqlx::query_scalar(
        "SELECT quiz_recipient_id FROM learning_quiz_attempts WHERE tenant_id=$1 AND id=$2",
    )
    .bind(tenant_id)
    .bind(attempt_id)
    .fetch_optional(pool)
    .await
    .context("resolve Learning quiz attempt recipient")?;
    Ok(recipient_id.map(|id| (space, id)))
}

async fn attempt_row(
    pool: &PgPool,
    tenant_id: Uuid,
    attempt_id: Uuid,
) -> Result<Option<AttemptRow>> {
    sqlx::query_as::<_, AttemptRow>(&format!("{ATTEMPT_SELECT} AND attempt.id=$2"))
        .bind(tenant_id)
        .bind(attempt_id)
        .fetch_optional(pool)
        .await
        .context("load Learning quiz attempt")
}

async fn attempt_response_by_id(
    pool: &PgPool,
    tenant_id: Uuid,
    attempt_id: Uuid,
) -> Result<Option<LearningQuizAttemptResponse>> {
    match attempt_row(pool, tenant_id, attempt_id).await? {
        Some(row) => attempt_response(pool, tenant_id, row).await.map(Some),
        None => Ok(None),
    }
}

async fn attempt_response(
    pool: &PgPool,
    tenant_id: Uuid,
    row: AttemptRow,
) -> Result<LearningQuizAttemptResponse> {
    let identity =
        EnrolmentOps::roster_references_by_enrolment_ids(pool, tenant_id, &[row.enrolment_id])
            .await?
            .into_iter()
            .find(|entry| entry.learner_id == row.learner_id)
            .ok_or_else(|| {
                anyhow!("The SIS learner identity for this quiz attempt is unavailable")
            })?;
    let answers = sqlx::query_as::<_, (Uuid, Uuid)>("SELECT learning_quiz_question_id,selected_choice_id FROM learning_quiz_attempt_answers WHERE tenant_id=$1 AND learning_quiz_attempt_id=$2 ORDER BY created_at,id")
        .bind(tenant_id).bind(row.id).fetch_all(pool).await.context("load Learning quiz answers")?
        .into_iter().map(|(question_id, selected_choice_id)| LearningQuizAttemptAnswerResponse { question_id, selected_choice_id }).collect();
    Ok(LearningQuizAttemptResponse {
        id: row.id,
        learning_quiz_id: row.learning_quiz_id,
        learner_id: row.learner_id,
        enrolment_id: row.enrolment_id,
        learner_name: identity.display_name,
        learner_number: identity.learner_number,
        attempt_number: row.attempt_number,
        status: parse_attempt_status(&row.status)?,
        version: row.version,
        answers,
        started_at: row.started_at,
        submitted_at: row.submitted_at,
        total_points: row.total_points_snapshot,
        earned_points: row.earned_points_snapshot,
        score_basis_points: row.score_basis_points,
        passed: row.passed,
    })
}

async fn require_quiz_parent(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    unit_id: Uuid,
    space_id: Uuid,
) -> Result<()> {
    let state = sqlx::query_as::<_, (String, String)>("SELECT unit.status,space.status FROM learning_units unit JOIN learning_spaces space ON space.id=unit.learning_space_id AND space.tenant_id=unit.tenant_id WHERE unit.tenant_id=$1 AND unit.id=$2 AND space.id=$3 AND unit.deleted_at IS NULL AND space.deleted_at IS NULL FOR UPDATE OF unit,space")
        .bind(tenant_id).bind(unit_id).bind(space_id).fetch_optional(&mut **tx).await.context("lock Learning quiz parent")?;
    match state {
        Some((unit, space)) if unit != "withdrawn" && space != "archived" => Ok(()),
        Some(_) => bail!("Quizzes cannot be added to withdrawn or archived Learning content"),
        None => bail!("The Learning quiz parent is unavailable"),
    }
}

async fn require_draft_quiz(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    quiz_id: Uuid,
) -> Result<()> {
    let status = sqlx::query_scalar::<_, String>("SELECT status FROM learning_quizzes WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL FOR UPDATE")
        .bind(tenant_id).bind(quiz_id).fetch_optional(&mut **tx).await.context("lock draft Learning quiz")?;
    match status.as_deref() {
        Some("draft") => Ok(()),
        Some(_) => bail!("A published Learning quiz is immutable"),
        None => bail!("The Learning quiz is unavailable"),
    }
}

async fn insert_choices(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    question_id: Uuid,
    actor_id: Uuid,
    choices: &[LearningQuizChoiceInput],
) -> Result<()> {
    for (index, choice) in choices.iter().enumerate() {
        let position = i32::try_from(index + 1)
            .context("Learning quiz choice position exceeded its supported range")?;
        sqlx::query("INSERT INTO learning_quiz_choices (tenant_id,learning_quiz_question_id,position,label,is_correct,created_by,updated_by) VALUES ($1,$2,$3,$4,$5,$6,$6)")
            .bind(tenant_id).bind(question_id).bind(position).bind(required("Choice label", &choice.label)?)
            .bind(choice.is_correct).bind(actor_id).execute(&mut **tx).await.context("create Learning quiz choice")?;
    }
    Ok(())
}

async fn validate_quiz_questions_for_publication(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    quiz_id: Uuid,
) -> Result<()> {
    let rows = sqlx::query_as::<_, (Uuid, i64, i64)>(
        "SELECT question.id,COUNT(choice.id)::BIGINT,COUNT(choice.id) FILTER (WHERE choice.is_correct)::BIGINT FROM learning_quiz_questions question LEFT JOIN learning_quiz_choices choice ON choice.learning_quiz_question_id=question.id AND choice.tenant_id=question.tenant_id AND choice.deleted_at IS NULL WHERE question.tenant_id=$1 AND question.learning_quiz_id=$2 AND question.deleted_at IS NULL GROUP BY question.id",
    ).bind(tenant_id).bind(quiz_id).fetch_all(&mut **tx).await.context("validate Learning quiz questions")?;
    if rows.is_empty() {
        bail!("Add at least one question before publishing the quiz");
    }
    if rows
        .iter()
        .any(|(_, choices, correct)| *choices < 2 || *correct != 1)
    {
        bail!("Every quiz question needs at least two choices and exactly one correct answer");
    }
    Ok(())
}

async fn validate_attempt_answers(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    quiz_id: Uuid,
    answers: &[crate::dtos::LearningQuizAnswerInput],
) -> Result<()> {
    for answer in answers {
        let valid = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM learning_quiz_questions question JOIN learning_quiz_choices choice ON choice.learning_quiz_question_id=question.id AND choice.tenant_id=question.tenant_id WHERE question.tenant_id=$1 AND question.learning_quiz_id=$2 AND question.id=$3 AND choice.id=$4 AND question.deleted_at IS NULL AND choice.deleted_at IS NULL)")
            .bind(tenant_id).bind(quiz_id).bind(answer.question_id).bind(answer.selected_choice_id)
            .fetch_one(&mut **tx).await.context("validate Learning quiz answer")?;
        if !valid {
            bail!("A selected quiz answer is unavailable");
        }
    }
    Ok(())
}

fn validate_choices(choices: &[LearningQuizChoiceInput]) -> Result<()> {
    if choices.len() < 2 || choices.len() > 8 {
        bail!("A quiz question needs between two and eight choices");
    }
    if choices.iter().filter(|choice| choice.is_correct).count() != 1 {
        bail!("A quiz question needs exactly one correct answer");
    }
    for choice in choices {
        choice.validate().context("A quiz choice is invalid")?;
        required("Choice label", &choice.label)?;
    }
    Ok(())
}

fn validate_quiz_window(
    opens_at: Option<DateTime<Utc>>,
    closes_at: Option<DateTime<Utc>>,
) -> Result<()> {
    if opens_at
        .zip(closes_at)
        .is_some_and(|(opens, closes)| opens >= closes)
    {
        bail!("Quiz closing time must be after its opening time");
    }
    Ok(())
}

fn attempt_fingerprint(attempt_id: Uuid, expected_version: i32) -> String {
    let mut digest = Sha256::new();
    digest.update(attempt_id.as_bytes());
    digest.update(expected_version.to_be_bytes());
    format!("{:x}", digest.finalize())
}

async fn completion_policy_row(
    pool: &PgPool,
    tenant_id: Uuid,
    space_id: Uuid,
    include_draft: bool,
) -> Result<Option<CompletionPolicyRow>> {
    sqlx::query_as::<_, CompletionPolicyRow>(
        "SELECT policy.id,policy.learning_space_id,policy.status,policy.version,policy.published_at,policy.created_at,policy.updated_at,(SELECT COUNT(*) FROM learning_completion_recipients recipient WHERE recipient.tenant_id=policy.tenant_id AND recipient.completion_policy_id=policy.id)::BIGINT AS recipient_count FROM learning_completion_policies policy WHERE policy.tenant_id=$1 AND policy.learning_space_id=$2 AND ((NOT $3 AND policy.status='published') OR ($3 AND policy.status IN ('draft','published'))) ORDER BY CASE policy.status WHEN 'draft' THEN 0 ELSE 1 END LIMIT 1",
    ).bind(tenant_id).bind(space_id).bind(include_draft).fetch_optional(pool).await.context("load Learning completion policy")
}

async fn completion_policy_by_id(
    pool: &PgPool,
    tenant_id: Uuid,
    policy_id: Uuid,
) -> Result<Option<LearningCompletionPolicyResponse>> {
    let row = sqlx::query_as::<_, CompletionPolicyRow>(
        "SELECT policy.id,policy.learning_space_id,policy.status,policy.version,policy.published_at,policy.created_at,policy.updated_at,(SELECT COUNT(*) FROM learning_completion_recipients recipient WHERE recipient.tenant_id=policy.tenant_id AND recipient.completion_policy_id=policy.id)::BIGINT AS recipient_count FROM learning_completion_policies policy WHERE policy.tenant_id=$1 AND policy.id=$2",
    ).bind(tenant_id).bind(policy_id).fetch_optional(pool).await.context("reload Learning completion policy")?;
    match row {
        Some(row) => completion_policy_response(pool, tenant_id, row)
            .await
            .map(Some),
        None => Ok(None),
    }
}

async fn published_completion_policy(
    pool: &PgPool,
    tenant_id: Uuid,
    space_id: Uuid,
) -> Result<Option<LearningCompletionPolicyResponse>> {
    match completion_policy_row(pool, tenant_id, space_id, false).await? {
        Some(row) => completion_policy_response(pool, tenant_id, row)
            .await
            .map(Some),
        None => Ok(None),
    }
}

async fn completion_policy_response(
    pool: &PgPool,
    tenant_id: Uuid,
    row: CompletionPolicyRow,
) -> Result<LearningCompletionPolicyResponse> {
    let requirements = completion_requirement_rows(pool, tenant_id, row.id).await?;
    let mut titles = BTreeMap::new();
    for requirement in &requirements {
        let title = match requirement.requirement_type.as_str() {
            "assignment" => sqlx::query_scalar::<_, String>("SELECT title FROM learning_assignments WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL").bind(tenant_id).bind(requirement.source_id).fetch_optional(pool).await.context("load completion assignment title")?,
            "quiz" => sqlx::query_scalar::<_, String>("SELECT title FROM learning_quizzes WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL").bind(tenant_id).bind(requirement.source_id).fetch_optional(pool).await.context("load completion quiz title")?,
            _ => None,
        }.unwrap_or_else(|| "Unavailable activity".to_string());
        titles.insert(requirement.id, title);
    }
    let requirements = requirements
        .into_iter()
        .map(|requirement| {
            Ok(LearningCompletionRequirementResponse {
                id: requirement.id,
                position: requirement.position,
                requirement_type: parse_requirement_type(&requirement.requirement_type)?,
                source_id: requirement.source_id,
                source_title: titles.remove(&requirement.id).unwrap_or_default(),
                minimum_score_basis_points: requirement.minimum_score_basis_points,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(LearningCompletionPolicyResponse {
        id: row.id,
        learning_space_id: row.learning_space_id,
        status: parse_completion_status(&row.status)?,
        version: row.version,
        requirements,
        recipient_count: row.recipient_count,
        published_at: row.published_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

async fn completion_requirement_rows(
    pool: &PgPool,
    tenant_id: Uuid,
    policy_id: Uuid,
) -> Result<Vec<CompletionRequirementRow>> {
    sqlx::query_as("SELECT id,position,requirement_type,source_id,minimum_score_basis_points FROM learning_completion_requirements WHERE tenant_id=$1 AND completion_policy_id=$2 ORDER BY position,id")
        .bind(tenant_id).bind(policy_id).fetch_all(pool).await.context("load Learning completion requirements")
}

async fn completion_requirement_inputs(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    policy_id: Uuid,
) -> Result<Vec<LearningCompletionRequirementInput>> {
    let rows = sqlx::query_as::<_, (String, Uuid, i32)>("SELECT requirement_type,source_id,minimum_score_basis_points FROM learning_completion_requirements WHERE tenant_id=$1 AND completion_policy_id=$2 ORDER BY position,id")
        .bind(tenant_id).bind(policy_id).fetch_all(&mut **tx).await.context("load completion requirements for publication")?;
    rows.into_iter()
        .map(|(kind, source_id, minimum)| {
            Ok(LearningCompletionRequirementInput {
                requirement_type: parse_requirement_type(&kind)?,
                source_id,
                minimum_score_basis_points: minimum,
            })
        })
        .collect()
}

async fn validate_completion_inputs(
    pool: &PgPool,
    tenant_id: Uuid,
    space_id: Uuid,
    requirements: &[LearningCompletionRequirementInput],
    require_published: bool,
) -> Result<()> {
    if requirements.is_empty() || requirements.len() > 100 {
        bail!("A completion policy needs between one and 100 requirements");
    }
    let mut unique = BTreeSet::new();
    for requirement in requirements {
        if !(0..=10_000).contains(&requirement.minimum_score_basis_points) {
            bail!("A completion threshold must be between 0 and 100 percent");
        }
        if !unique.insert((requirement.requirement_type.as_str(), requirement.source_id)) {
            bail!("A completion activity can only be required once");
        }
        let valid = match requirement.requirement_type {
            LearningCompletionRequirementType::Assignment => sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM learning_assignments assignment JOIN learning_units unit ON unit.id=assignment.learning_unit_id AND unit.tenant_id=assignment.tenant_id WHERE assignment.tenant_id=$1 AND assignment.id=$2 AND unit.learning_space_id=$3 AND assignment.deleted_at IS NULL AND unit.deleted_at IS NULL AND (NOT $4 OR assignment.status IN ('published','closed')))"),
            LearningCompletionRequirementType::Quiz => sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM learning_quizzes quiz JOIN learning_units unit ON unit.id=quiz.learning_unit_id AND unit.tenant_id=quiz.tenant_id WHERE quiz.tenant_id=$1 AND quiz.id=$2 AND unit.learning_space_id=$3 AND quiz.deleted_at IS NULL AND unit.deleted_at IS NULL AND (NOT $4 OR quiz.status IN ('published','closed')))"),
        }.bind(tenant_id).bind(requirement.source_id).bind(space_id).bind(require_published).fetch_one(pool).await.context("validate Learning completion activity")?;
        if !valid {
            bail!("A completion activity is unavailable or belongs to another Learning space");
        }
    }
    Ok(())
}

async fn insert_completion_requirements(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    policy_id: Uuid,
    actor_id: Uuid,
    requirements: &[LearningCompletionRequirementInput],
) -> Result<()> {
    for (index, requirement) in requirements.iter().enumerate() {
        let position = i32::try_from(index + 1)
            .context("Completion requirement position exceeded its supported range")?;
        sqlx::query("INSERT INTO learning_completion_requirements (tenant_id,completion_policy_id,position,requirement_type,source_id,minimum_score_basis_points,created_by) VALUES ($1,$2,$3,$4,$5,$6,$7)")
            .bind(tenant_id).bind(policy_id).bind(position).bind(requirement.requirement_type.as_str())
            .bind(requirement.source_id).bind(requirement.minimum_score_basis_points).bind(actor_id)
            .execute(&mut **tx).await.context("save Learning completion requirement")?;
    }
    Ok(())
}

async fn completion_entries(
    pool: &PgPool,
    tenant_id: Uuid,
    policy_id: Uuid,
    learner_filter: Option<Uuid>,
) -> Result<Vec<LearningCompletionEntry>> {
    let recipients = sqlx::query_as::<_, (Uuid, Uuid)>("SELECT learner_id,enrolment_id FROM learning_completion_recipients WHERE tenant_id=$1 AND completion_policy_id=$2 AND ($3::UUID IS NULL OR learner_id=$3) ORDER BY assigned_at,id")
        .bind(tenant_id).bind(policy_id).bind(learner_filter).fetch_all(pool).await.context("load Learning completion recipients")?;
    let required_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM learning_completion_requirements WHERE tenant_id=$1 AND completion_policy_id=$2")
        .bind(tenant_id).bind(policy_id).fetch_one(pool).await.context("count Learning completion requirements")?;
    let identities = EnrolmentOps::roster_references_by_enrolment_ids(
        pool,
        tenant_id,
        &recipients
            .iter()
            .map(|(_, enrolment_id)| *enrolment_id)
            .collect::<Vec<_>>(),
    )
    .await?;
    let mut entries = Vec::with_capacity(recipients.len());
    for (learner_id, enrolment_id) in recipients {
        let completed_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM learning_completion_requirements requirement WHERE requirement.tenant_id=$1 AND requirement.completion_policy_id=$2 AND ((requirement.requirement_type='assignment' AND EXISTS (SELECT 1 FROM learning_assignment_recipients recipient JOIN learning_assignments assignment ON assignment.id=recipient.learning_assignment_id AND assignment.tenant_id=recipient.tenant_id JOIN learning_submissions submission ON submission.assignment_recipient_id=recipient.id AND submission.tenant_id=recipient.tenant_id AND submission.current_submission_version_id IS NOT NULL JOIN learning_submission_reviews review ON review.submission_version_id=submission.current_submission_version_id AND review.tenant_id=submission.tenant_id AND review.status='released' AND review.outcome='graded' WHERE recipient.tenant_id=requirement.tenant_id AND recipient.learning_assignment_id=requirement.source_id AND recipient.learner_id=$3 AND (review.total_score_hundredths::BIGINT * 10000) / assignment.max_score_hundredths >= requirement.minimum_score_basis_points)) OR (requirement.requirement_type='quiz' AND EXISTS (SELECT 1 FROM learning_quiz_recipients recipient JOIN learning_quiz_attempts attempt ON attempt.quiz_recipient_id=recipient.id AND attempt.tenant_id=recipient.tenant_id AND attempt.status='submitted' WHERE recipient.tenant_id=requirement.tenant_id AND recipient.learning_quiz_id=requirement.source_id AND recipient.learner_id=$3 AND attempt.score_basis_points >= requirement.minimum_score_basis_points)))",
        ).bind(tenant_id).bind(policy_id).bind(learner_id).fetch_one(pool).await.context("derive Learning completion")?;
        let identity = identities
            .iter()
            .find(|entry| entry.learner_id == learner_id && entry.enrolment_id == enrolment_id)
            .ok_or_else(|| {
                anyhow!("The SIS learner identity for this completion record is unavailable")
            })?;
        let completion_percent = if required_count == 0 {
            0
        } else {
            i32::try_from((completed_count * 100) / required_count)
                .context("Learning completion percentage exceeded its supported range")?
        };
        entries.push(LearningCompletionEntry {
            learner_id,
            enrolment_id,
            learner_name: identity.display_name.clone(),
            learner_number: identity.learner_number.clone(),
            required_count,
            completed_count,
            completion_percent,
            complete: required_count > 0 && completed_count == required_count,
        });
    }
    Ok(entries)
}

fn parse_quiz_status(value: &str) -> Result<LearningQuizStatus> {
    match value {
        "draft" => Ok(LearningQuizStatus::Draft),
        "published" => Ok(LearningQuizStatus::Published),
        "closed" => Ok(LearningQuizStatus::Closed),
        _ => bail!("Stored Learning quiz status is invalid"),
    }
}
fn parse_attempt_status(value: &str) -> Result<LearningQuizAttemptStatus> {
    match value {
        "in_progress" => Ok(LearningQuizAttemptStatus::InProgress),
        "submitted" => Ok(LearningQuizAttemptStatus::Submitted),
        _ => bail!("Stored Learning quiz attempt status is invalid"),
    }
}
fn parse_completion_status(value: &str) -> Result<LearningCompletionPolicyStatus> {
    match value {
        "draft" => Ok(LearningCompletionPolicyStatus::Draft),
        "published" => Ok(LearningCompletionPolicyStatus::Published),
        "superseded" => Ok(LearningCompletionPolicyStatus::Superseded),
        _ => bail!("Stored Learning completion policy status is invalid"),
    }
}
fn parse_requirement_type(value: &str) -> Result<LearningCompletionRequirementType> {
    match value {
        "assignment" => Ok(LearningCompletionRequirementType::Assignment),
        "quiz" => Ok(LearningCompletionRequirementType::Quiz),
        _ => bail!("Stored Learning completion requirement is invalid"),
    }
}

fn required<'a>(label: &str, value: &'a str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{label} is required");
    }
    Ok(value)
}
fn optional(value: &Option<String>) -> Option<&str> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}
fn database_error(error: sqlx::Error, action: &str) -> anyhow::Error {
    if let sqlx::Error::Database(database) = &error {
        if database.is_unique_violation() {
            return anyhow!("A Learning quiz position or policy value already exists");
        }
    }
    anyhow!(error).context(action.to_string())
}
