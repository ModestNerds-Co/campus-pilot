//! Term-scoped assessment structures.
//!
//! Components reference canonical teaching assignments. Learner marks are a
//! separate boundary because SIS remains authoritative for enrolment.

use anyhow::{Context, Result, bail};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;
use validator::{Validate, ValidationError};

use crate::ops::DeleteOutcome;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentCycleStatus {
    Draft,
    Open,
    Closed,
}

impl AssessmentCycleStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Open => "open",
            Self::Closed => "closed",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentComponentStatus {
    Active,
    Inactive,
}

impl AssessmentComponentStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentKind {
    Assignment,
    Quiz,
    Test,
    Project,
    Exam,
    Practical,
    Other,
}

impl AssessmentKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Assignment => "assignment",
            Self::Quiz => "quiz",
            Self::Test => "test",
            Self::Project => "project",
            Self::Exam => "exam",
            Self::Practical => "practical",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AssessmentCycleListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<AssessmentCycleStatus>,
    pub academic_term_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateAssessmentCycleRequest {
    pub academic_term_id: Uuid,
    #[validate(length(min = 1, max = 40))]
    pub code: String,
    #[validate(length(min = 1, max = 120))]
    pub name: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateAssessmentCycleRequest {
    pub academic_term_id: Uuid,
    #[validate(length(min = 1, max = 40))]
    pub code: String,
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    pub status: AssessmentCycleStatus,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AssessmentCycleResponse {
    pub id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_term_code: String,
    pub academic_term_name: String,
    pub academic_year_id: Uuid,
    pub academic_year_name: String,
    pub code: String,
    pub name: String,
    pub status: String,
    pub component_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedAssessmentCyclesResponse {
    pub assessment_cycles: Vec<AssessmentCycleResponse>,
}

#[derive(Debug, Deserialize)]
pub struct AssessmentComponentListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub status: Option<AssessmentComponentStatus>,
    pub teaching_assignment_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate)]
#[validate(schema(function = "validate_component_request"))]
pub struct CreateAssessmentComponentRequest {
    pub teaching_assignment_id: Uuid,
    #[validate(length(min = 1, max = 40))]
    pub code: String,
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    pub assessment_kind: AssessmentKind,
    #[validate(range(min = 1, max = 100000))]
    pub maximum_marks: i32,
    #[validate(range(min = 1, max = 10000))]
    pub weight_basis_points: i16,
    pub occurs_on: Option<NaiveDate>,
    pub status: Option<AssessmentComponentStatus>,
}

fn validate_component_request(
    request: &CreateAssessmentComponentRequest,
) -> std::result::Result<(), ValidationError> {
    if request.code.trim().is_empty() || request.name.trim().is_empty() {
        return Err(ValidationError::new("assessment_component_name"));
    }
    Ok(())
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateAssessmentComponentRequest {
    pub teaching_assignment_id: Uuid,
    #[validate(length(min = 1, max = 40))]
    pub code: String,
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    pub assessment_kind: AssessmentKind,
    #[validate(range(min = 1, max = 100000))]
    pub maximum_marks: i32,
    #[validate(range(min = 1, max = 10000))]
    pub weight_basis_points: i16,
    pub occurs_on: Option<NaiveDate>,
    pub status: AssessmentComponentStatus,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AssessmentComponentResponse {
    pub id: Uuid,
    pub assessment_cycle_id: Uuid,
    pub assessment_cycle_name: String,
    pub teaching_assignment_id: Uuid,
    pub class_group_id: Uuid,
    pub class_group_name: String,
    pub subject_id: Uuid,
    pub subject_name: String,
    pub teacher_profile_id: Uuid,
    pub teacher_name: String,
    pub code: String,
    pub name: String,
    pub assessment_kind: String,
    pub maximum_marks: i32,
    pub weight_basis_points: i16,
    pub occurs_on: Option<NaiveDate>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedAssessmentComponentsResponse {
    pub assessment_components: Vec<AssessmentComponentResponse>,
}

#[derive(Debug, FromRow)]
struct CycleState {
    academic_term_id: Uuid,
    status: String,
}

#[derive(Debug, FromRow)]
struct TermBoundary {
    academic_year_id: Uuid,
    starts_on: NaiveDate,
    ends_on: NaiveDate,
    status: String,
}

#[derive(Debug, FromRow)]
struct ComponentState {
    assessment_cycle_id: Uuid,
}

pub struct AssessmentCycleOps;

impl AssessmentCycleOps {
    #[allow(clippy::too_many_arguments)]
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
        academic_term_id: Option<Uuid>,
    ) -> Result<(Vec<AssessmentCycleResponse>, i64)> {
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, AssessmentCycleResponse>(
            r#"
            SELECT cycle.id, cycle.academic_term_id,
                   term.code AS academic_term_code, term.name AS academic_term_name,
                   term.academic_year_id, academic_year.name AS academic_year_name,
                   cycle.code, cycle.name, cycle.status,
                   COUNT(component.id) AS component_count,
                   cycle.created_at, cycle.updated_at
            FROM assessment_cycles AS cycle
            INNER JOIN academic_terms AS term
              ON term.id = cycle.academic_term_id
             AND term.tenant_id = cycle.tenant_id
             AND term.deleted_at IS NULL
            INNER JOIN academic_years AS academic_year
              ON academic_year.id = term.academic_year_id
             AND academic_year.tenant_id = term.tenant_id
             AND academic_year.deleted_at IS NULL
            LEFT JOIN assessment_components AS component
              ON component.assessment_cycle_id = cycle.id
             AND component.tenant_id = cycle.tenant_id
             AND component.deleted_at IS NULL
            WHERE cycle.tenant_id = $1 AND cycle.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR cycle.code ILIKE $2 OR cycle.name ILIKE $2)
              AND ($3::TEXT IS NULL OR cycle.status = $3)
              AND ($4::UUID IS NULL OR cycle.academic_term_id = $4)
            GROUP BY cycle.id, term.id, academic_year.id
            ORDER BY academic_year.starts_on DESC, term.starts_on DESC, cycle.created_at DESC
            LIMIT $5 OFFSET $6
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(academic_term_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list assessment cycles")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM assessment_cycles
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND ($2::TEXT IS NULL OR code ILIKE $2 OR name ILIKE $2)
              AND ($3::TEXT IS NULL OR status = $3)
              AND ($4::UUID IS NULL OR academic_term_id = $4)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(academic_term_id)
        .fetch_one(pool)
        .await
        .context("Failed to count assessment cycles")?;
        Ok((rows, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<AssessmentCycleResponse>> {
        sqlx::query_as::<_, AssessmentCycleResponse>(
            r#"
            SELECT cycle.id, cycle.academic_term_id,
                   term.code AS academic_term_code, term.name AS academic_term_name,
                   term.academic_year_id, academic_year.name AS academic_year_name,
                   cycle.code, cycle.name, cycle.status,
                   COUNT(component.id) AS component_count,
                   cycle.created_at, cycle.updated_at
            FROM assessment_cycles AS cycle
            INNER JOIN academic_terms AS term
              ON term.id = cycle.academic_term_id
             AND term.tenant_id = cycle.tenant_id
             AND term.deleted_at IS NULL
            INNER JOIN academic_years AS academic_year
              ON academic_year.id = term.academic_year_id
             AND academic_year.tenant_id = term.tenant_id
             AND academic_year.deleted_at IS NULL
            LEFT JOIN assessment_components AS component
              ON component.assessment_cycle_id = cycle.id
             AND component.tenant_id = cycle.tenant_id
             AND component.deleted_at IS NULL
            WHERE cycle.tenant_id = $1 AND cycle.id = $2 AND cycle.deleted_at IS NULL
            GROUP BY cycle.id, term.id, academic_year.id
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load assessment cycle")
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        request: &CreateAssessmentCycleRequest,
    ) -> Result<AssessmentCycleResponse> {
        ensure_term_accepts_assessments(pool, tenant_id, request.academic_term_id).await?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO assessment_cycles (tenant_id, academic_term_id, code, name)
            VALUES ($1, $2, $3, $4)
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(request.academic_term_id)
        .bind(request.code.trim())
        .bind(request.name.trim())
        .fetch_one(pool)
        .await
        .context("Failed to create assessment cycle")?;
        Self::get_by_id(pool, tenant_id, id)
            .await?
            .context("Created assessment cycle could not be reloaded")
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        request: &UpdateAssessmentCycleRequest,
    ) -> Result<Option<AssessmentCycleResponse>> {
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to begin assessment update")?;
        let Some(current) = lock_cycle(&mut transaction, tenant_id, id).await? else {
            return Ok(None);
        };
        validate_cycle_transition(&current.status, request.status.as_str())?;
        if current.academic_term_id != request.academic_term_id {
            ensure_term_accepts_assessments_tx(
                &mut transaction,
                tenant_id,
                request.academic_term_id,
            )
            .await?;
            let has_components = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM assessment_components WHERE tenant_id = $1 AND assessment_cycle_id = $2 AND deleted_at IS NULL)",
            )
            .bind(tenant_id)
            .bind(id)
            .fetch_one(&mut *transaction)
            .await
            .context("Failed to check assessment components")?;
            if has_components {
                bail!("Remove assessment components before changing the academic term");
            }
        }
        if current.status != "draft"
            && (current.academic_term_id != request.academic_term_id
                || request.code.trim().is_empty()
                || request.name.trim().is_empty())
        {
            bail!("Only a draft assessment cycle can change its details");
        }
        if current.status != "draft" {
            let stored = sqlx::query_as::<_, (String, String)>(
                "SELECT code, name FROM assessment_cycles WHERE tenant_id = $1 AND id = $2",
            )
            .bind(tenant_id)
            .bind(id)
            .fetch_one(&mut *transaction)
            .await
            .context("Failed to read assessment cycle details")?;
            if stored.0 != request.code.trim() || stored.1 != request.name.trim() {
                bail!("Only a draft assessment cycle can change its details");
            }
        }
        if current.status == "draft" && request.status == AssessmentCycleStatus::Open {
            ensure_term_accepts_assessments_tx(
                &mut transaction,
                tenant_id,
                current.academic_term_id,
            )
            .await?;
            validate_cycle_weights(&mut transaction, tenant_id, id).await?;
        }
        sqlx::query(
            r#"
            UPDATE assessment_cycles
            SET academic_term_id = $1, code = $2, name = $3, status = $4
            WHERE tenant_id = $5 AND id = $6 AND deleted_at IS NULL
            "#,
        )
        .bind(request.academic_term_id)
        .bind(request.code.trim())
        .bind(request.name.trim())
        .bind(request.status.as_str())
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to update assessment cycle")?;
        transaction
            .commit()
            .await
            .context("Failed to commit assessment update")?;
        Self::get_by_id(pool, tenant_id, id).await
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<DeleteOutcome> {
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to begin assessment removal")?;
        let Some(current) = lock_cycle(&mut transaction, tenant_id, id).await? else {
            return Ok(DeleteOutcome::NotFound);
        };
        if current.status != "draft" {
            bail!("Only a draft assessment cycle can be removed");
        }
        let in_use = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM assessment_components WHERE tenant_id = $1 AND assessment_cycle_id = $2 AND deleted_at IS NULL)",
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to check assessment components")?;
        if in_use {
            return Ok(DeleteOutcome::InUse);
        }
        sqlx::query(
            "UPDATE assessment_cycles SET deleted_at = NOW() WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to remove assessment cycle")?;
        transaction
            .commit()
            .await
            .context("Failed to commit assessment removal")?;
        Ok(DeleteOutcome::Deleted)
    }
}

pub struct AssessmentComponentOps;

impl AssessmentComponentOps {
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        cycle_id: Uuid,
        page: i64,
        per_page: i64,
        status: Option<&str>,
        teaching_assignment_id: Option<Uuid>,
    ) -> Result<(Vec<AssessmentComponentResponse>, i64)> {
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, AssessmentComponentResponse>(&component_select(
            "component.assessment_cycle_id = $2 AND ($3::TEXT IS NULL OR component.status = $3) AND ($4::UUID IS NULL OR component.teaching_assignment_id = $4) ORDER BY class_group.name, subject.name, component.occurs_on NULLS LAST, component.name LIMIT $5 OFFSET $6",
        ))
        .bind(tenant_id)
        .bind(cycle_id)
        .bind(status)
        .bind(teaching_assignment_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list assessment components")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM assessment_components
            WHERE tenant_id = $1 AND assessment_cycle_id = $2 AND deleted_at IS NULL
              AND ($3::TEXT IS NULL OR status = $3)
              AND ($4::UUID IS NULL OR teaching_assignment_id = $4)
            "#,
        )
        .bind(tenant_id)
        .bind(cycle_id)
        .bind(status)
        .bind(teaching_assignment_id)
        .fetch_one(pool)
        .await
        .context("Failed to count assessment components")?;
        Ok((rows, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<AssessmentComponentResponse>> {
        sqlx::query_as::<_, AssessmentComponentResponse>(&component_select("component.id = $2"))
            .bind(tenant_id)
            .bind(id)
            .fetch_optional(pool)
            .await
            .context("Failed to load assessment component")
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        cycle_id: Uuid,
        request: &CreateAssessmentComponentRequest,
    ) -> Result<AssessmentComponentResponse> {
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to begin component creation")?;
        let cycle = require_draft_cycle(&mut transaction, tenant_id, cycle_id).await?;
        let status = request
            .status
            .unwrap_or(AssessmentComponentStatus::Active)
            .as_str();
        validate_component_reference(
            &mut transaction,
            tenant_id,
            &cycle,
            request.teaching_assignment_id,
            request.occurs_on,
            status,
        )
        .await?;
        validate_weight_limit(
            &mut transaction,
            tenant_id,
            cycle_id,
            request.teaching_assignment_id,
            None,
            status,
            request.weight_basis_points,
        )
        .await?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO assessment_components
                (tenant_id, assessment_cycle_id, teaching_assignment_id, code, name,
                 assessment_kind, maximum_marks, weight_basis_points, occurs_on, status)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(cycle_id)
        .bind(request.teaching_assignment_id)
        .bind(request.code.trim())
        .bind(request.name.trim())
        .bind(request.assessment_kind.as_str())
        .bind(request.maximum_marks)
        .bind(request.weight_basis_points)
        .bind(request.occurs_on)
        .bind(status)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to create assessment component")?;
        transaction
            .commit()
            .await
            .context("Failed to commit component creation")?;
        Self::get_by_id(pool, tenant_id, id)
            .await?
            .context("Created assessment component could not be reloaded")
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        request: &UpdateAssessmentComponentRequest,
    ) -> Result<Option<AssessmentComponentResponse>> {
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to begin component update")?;
        let Some(current) = lock_component(&mut transaction, tenant_id, id).await? else {
            return Ok(None);
        };
        let cycle =
            require_draft_cycle(&mut transaction, tenant_id, current.assessment_cycle_id).await?;
        validate_component_reference(
            &mut transaction,
            tenant_id,
            &cycle,
            request.teaching_assignment_id,
            request.occurs_on,
            request.status.as_str(),
        )
        .await?;
        validate_weight_limit(
            &mut transaction,
            tenant_id,
            current.assessment_cycle_id,
            request.teaching_assignment_id,
            Some(id),
            request.status.as_str(),
            request.weight_basis_points,
        )
        .await?;
        sqlx::query(
            r#"
            UPDATE assessment_components
            SET teaching_assignment_id = $1, code = $2, name = $3,
                assessment_kind = $4, maximum_marks = $5, weight_basis_points = $6,
                occurs_on = $7, status = $8
            WHERE tenant_id = $9 AND id = $10 AND deleted_at IS NULL
            "#,
        )
        .bind(request.teaching_assignment_id)
        .bind(request.code.trim())
        .bind(request.name.trim())
        .bind(request.assessment_kind.as_str())
        .bind(request.maximum_marks)
        .bind(request.weight_basis_points)
        .bind(request.occurs_on)
        .bind(request.status.as_str())
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to update assessment component")?;
        transaction
            .commit()
            .await
            .context("Failed to commit component update")?;
        Self::get_by_id(pool, tenant_id, id).await
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<DeleteOutcome> {
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to begin component removal")?;
        let Some(current) = lock_component(&mut transaction, tenant_id, id).await? else {
            return Ok(DeleteOutcome::NotFound);
        };
        require_draft_cycle(&mut transaction, tenant_id, current.assessment_cycle_id).await?;
        sqlx::query(
            "UPDATE assessment_components SET deleted_at = NOW() WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to remove assessment component")?;
        transaction
            .commit()
            .await
            .context("Failed to commit component removal")?;
        Ok(DeleteOutcome::Deleted)
    }
}

fn component_select(predicate: &str) -> String {
    format!(
        r#"
        SELECT component.id, component.assessment_cycle_id,
               cycle.name AS assessment_cycle_name,
               component.teaching_assignment_id,
               assignment.class_group_id, class_group.name AS class_group_name,
               assignment.subject_id, subject.name AS subject_name,
               assignment.teacher_profile_id, employee.display_name AS teacher_name,
               component.code, component.name, component.assessment_kind,
               component.maximum_marks, component.weight_basis_points,
               component.occurs_on, component.status,
               component.created_at, component.updated_at
        FROM assessment_components AS component
        INNER JOIN assessment_cycles AS cycle
          ON cycle.id = component.assessment_cycle_id
         AND cycle.tenant_id = component.tenant_id
         AND cycle.deleted_at IS NULL
        INNER JOIN teaching_assignments AS assignment
          ON assignment.id = component.teaching_assignment_id
         AND assignment.tenant_id = component.tenant_id
         AND assignment.deleted_at IS NULL
        INNER JOIN class_groups AS class_group
          ON class_group.id = assignment.class_group_id
         AND class_group.tenant_id = assignment.tenant_id
         AND class_group.deleted_at IS NULL
        INNER JOIN subjects AS subject
          ON subject.id = assignment.subject_id
         AND subject.tenant_id = assignment.tenant_id
         AND subject.deleted_at IS NULL
        INNER JOIN teacher_profiles AS teacher
          ON teacher.id = assignment.teacher_profile_id
         AND teacher.tenant_id = assignment.tenant_id
         AND teacher.deleted_at IS NULL
        INNER JOIN employees AS employee
          ON employee.id = teacher.employee_id
         AND employee.tenant_id = teacher.tenant_id
         AND employee.deleted_at IS NULL
        WHERE component.tenant_id = $1 AND component.deleted_at IS NULL AND {predicate}
        "#,
    )
}

async fn ensure_term_accepts_assessments(
    pool: &PgPool,
    tenant_id: Uuid,
    term_id: Uuid,
) -> Result<TermBoundary> {
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to validate academic term")?;
    let term = ensure_term_accepts_assessments_tx(&mut transaction, tenant_id, term_id).await?;
    transaction
        .commit()
        .await
        .context("Failed to finish academic term validation")?;
    Ok(term)
}

async fn ensure_term_accepts_assessments_tx(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    term_id: Uuid,
) -> Result<TermBoundary> {
    let term = sqlx::query_as::<_, TermBoundary>(
        r#"
        SELECT academic_year_id, starts_on, ends_on, status
        FROM academic_terms
        WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(term_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to load academic term")?
    .context("Academic term was not found for this campus")?;
    if term.status == "closed" {
        bail!("A closed academic term cannot accept a new assessment cycle");
    }
    Ok(term)
}

async fn lock_cycle(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    cycle_id: Uuid,
) -> Result<Option<CycleState>> {
    sqlx::query_as::<_, CycleState>(
        r#"
        SELECT academic_term_id, status
        FROM assessment_cycles
        WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(cycle_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock assessment cycle")
}

async fn require_draft_cycle(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    cycle_id: Uuid,
) -> Result<CycleState> {
    let cycle = lock_cycle(transaction, tenant_id, cycle_id)
        .await?
        .context("Assessment cycle was not found for this campus")?;
    if cycle.status != "draft" {
        bail!("Assessment components can only change while the cycle is draft");
    }
    Ok(cycle)
}

async fn lock_component(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<ComponentState>> {
    sqlx::query_as::<_, ComponentState>(
        r#"
        SELECT assessment_cycle_id
        FROM assessment_components
        WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock assessment component")
}

async fn validate_cycle_weights(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    cycle_id: Uuid,
) -> Result<()> {
    let component_ids = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM assessment_components WHERE tenant_id = $1 AND assessment_cycle_id = $2 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(cycle_id)
    .fetch_all(&mut **transaction)
    .await
    .context("Failed to lock assessment components")?;
    if component_ids.is_empty() {
        bail!("Add at least one active assessment component before opening the cycle");
    }
    let totals = sqlx::query_as::<_, (Uuid, i64)>(
        r#"
        SELECT teaching_assignment_id, SUM(weight_basis_points)::BIGINT
        FROM assessment_components
        WHERE tenant_id = $1 AND assessment_cycle_id = $2
          AND status = 'active' AND deleted_at IS NULL
        GROUP BY teaching_assignment_id
        "#,
    )
    .bind(tenant_id)
    .bind(cycle_id)
    .fetch_all(&mut **transaction)
    .await
    .context("Failed to total assessment weights")?;
    if totals.is_empty() {
        bail!("Add at least one active assessment component before opening the cycle");
    }
    if totals.iter().any(|(_, total)| *total != 10_000) {
        bail!("Active assessment component weights must total 100% for every teaching assignment");
    }
    Ok(())
}

async fn validate_component_reference(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    cycle: &CycleState,
    teaching_assignment_id: Uuid,
    occurs_on: Option<NaiveDate>,
    component_status: &str,
) -> Result<()> {
    let term =
        ensure_term_accepts_assessments_tx(transaction, tenant_id, cycle.academic_term_id).await?;
    let assignment = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT academic_year_id, status
        FROM teaching_assignments
        WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(teaching_assignment_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to load teaching assignment")?
    .context("Teaching assignment was not found for this campus")?;
    if assignment.0 != term.academic_year_id {
        bail!("Teaching assignment must belong to the assessment term academic year");
    }
    if component_status == "active" && assignment.1 != "active" {
        bail!("An active assessment component requires an active teaching assignment");
    }
    if occurs_on.is_some_and(|date| date < term.starts_on || date > term.ends_on) {
        bail!("Assessment date must fall within the academic term");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn validate_weight_limit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    cycle_id: Uuid,
    teaching_assignment_id: Uuid,
    excluded_id: Option<Uuid>,
    status: &str,
    weight_basis_points: i16,
) -> Result<()> {
    if status != "active" {
        return Ok(());
    }
    let current = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COALESCE(SUM(weight_basis_points), 0)::BIGINT
        FROM assessment_components
        WHERE tenant_id = $1 AND assessment_cycle_id = $2
          AND teaching_assignment_id = $3 AND status = 'active'
          AND deleted_at IS NULL AND ($4::UUID IS NULL OR id <> $4)
        "#,
    )
    .bind(tenant_id)
    .bind(cycle_id)
    .bind(teaching_assignment_id)
    .bind(excluded_id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to total assessment weights")?;
    if current + i64::from(weight_basis_points) > 10_000 {
        bail!("Active assessment component weights cannot exceed 100% for a teaching assignment");
    }
    Ok(())
}

fn validate_cycle_transition(current: &str, requested: &str) -> Result<()> {
    let allowed = matches!(
        (current, requested),
        ("draft", "draft")
            | ("draft", "open")
            | ("open", "open")
            | ("open", "closed")
            | ("closed", "closed")
    );
    if !allowed {
        bail!("Assessment cycles move forward from draft to open to closed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_cycle_transition;

    #[test]
    fn assessment_cycle_state_machine_only_moves_forward() {
        assert!(validate_cycle_transition("draft", "open").is_ok());
        assert!(validate_cycle_transition("open", "closed").is_ok());
        assert!(validate_cycle_transition("closed", "closed").is_ok());
        assert!(validate_cycle_transition("draft", "closed").is_err());
        assert!(validate_cycle_transition("open", "draft").is_err());
        assert!(validate_cycle_transition("closed", "open").is_err());
    }
}
