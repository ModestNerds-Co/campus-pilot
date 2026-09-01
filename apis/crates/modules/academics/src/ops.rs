//! Typed Academics operations shared by HTTP, Timetabling, and Agent adapters.
//!
//! SQL in this module touches Academics-owned tables only. Employee identity is
//! resolved through HR's typed `EmployeeOps` boundary.

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, bail};
use cp_hr_payroll::{models::EmployeeReference, ops::EmployeeOps};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    dtos::{
        CommunicationClassReference, CreateAcademicGradeLevelRequest, CreateAcademicTermRequest,
        CreateAcademicYearRequest, CreateClassGroupRequest, CreateSubjectRequest,
        CreateTeacherProfileRequest, CreateTeachingAssignmentRequest,
        UpdateAcademicGradeLevelRequest, UpdateAcademicTermRequest, UpdateAcademicYearRequest,
        UpdateClassGroupRequest, UpdateSubjectRequest, UpdateTeacherProfileRequest,
        UpdateTeachingAssignmentRequest,
    },
    models::{
        AcademicGradeLevel, AcademicTerm, AcademicYear, ClassGroupWithYear, Subject,
        TeacherProfile, TeacherProfileWithEmployee, TeachingAssignmentRow,
        TeachingAssignmentWithDetails, TimetablingReferenceData,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteOutcome {
    Deleted,
    NotFound,
    InUse,
}

/// Typed Academics boundary for class-based communication audiences.
pub struct CommunicationAudienceOps;

impl CommunicationAudienceOps {
    /// Lists active classes. When an account is supplied, only classes with a
    /// current teaching assignment linked to that employee account are shown.
    pub async fn class_references(
        pool: &PgPool,
        tenant_id: Uuid,
        assigned_account_id: Option<Uuid>,
    ) -> Result<Vec<CommunicationClassReference>> {
        let employee_id = if let Some(account_id) = assigned_account_id {
            EmployeeOps::active_reference_by_account(pool, tenant_id, account_id)
                .await?
                .map(|employee| employee.id)
        } else {
            None
        };
        if assigned_account_id.is_some() && employee_id.is_none() {
            return Ok(Vec::new());
        }
        sqlx::query_as::<_, CommunicationClassReference>(
            r#"
            SELECT DISTINCT class_group.id, class_group.code, class_group.name,
                   grade_level.name AS grade_level
              FROM class_groups AS class_group
              JOIN academic_years AS academic_year
                ON academic_year.id = class_group.academic_year_id
               AND academic_year.tenant_id = class_group.tenant_id
               AND academic_year.deleted_at IS NULL
              LEFT JOIN academic_grade_levels AS grade_level
                ON grade_level.id = class_group.grade_level_id
               AND grade_level.tenant_id = class_group.tenant_id
               AND grade_level.deleted_at IS NULL
             WHERE class_group.tenant_id = $1
               AND class_group.status = 'active'
               AND class_group.deleted_at IS NULL
               AND (
                   $2::UUID IS NULL
                   OR EXISTS (
                       SELECT 1
                         FROM teaching_assignments AS assignment
                         JOIN teacher_profiles AS teacher
                           ON teacher.id = assignment.teacher_profile_id
                          AND teacher.tenant_id = assignment.tenant_id
                          AND teacher.deleted_at IS NULL
                        WHERE assignment.tenant_id = class_group.tenant_id
                          AND assignment.class_group_id = class_group.id
                          AND assignment.status = 'active'
                          AND assignment.deleted_at IS NULL
                          AND teacher.employee_id = $2
                   )
               )
             ORDER BY class_group.name, class_group.code
            "#,
        )
        .bind(tenant_id)
        .bind(employee_id)
        .fetch_all(pool)
        .await
        .context("Failed to list communication class references")
    }
}

pub struct AcademicYearOps;

impl AcademicYearOps {
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
    ) -> Result<(Vec<AcademicYear>, i64)> {
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let years = sqlx::query_as::<_, AcademicYear>(
            r#"
            SELECT id, tenant_id, name, starts_on, ends_on, status,
                   created_at, updated_at, deleted_at
            FROM academic_years
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND ($2::TEXT IS NULL OR name ILIKE $2)
              AND ($3::TEXT IS NULL OR status = $3)
            ORDER BY starts_on DESC, name
            LIMIT $4 OFFSET $5
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list academic years")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM academic_years
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND ($2::TEXT IS NULL OR name ILIKE $2)
              AND ($3::TEXT IS NULL OR status = $3)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .fetch_one(pool)
        .await
        .context("Failed to count academic years")?;
        Ok((years, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<AcademicYear>> {
        sqlx::query_as::<_, AcademicYear>(
            r#"
            SELECT id, tenant_id, name, starts_on, ends_on, status,
                   created_at, updated_at, deleted_at
            FROM academic_years
            WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load academic year")
    }

    pub async fn get_active(pool: &PgPool, tenant_id: Uuid) -> Result<Option<AcademicYear>> {
        sqlx::query_as::<_, AcademicYear>(
            r#"
            SELECT id, tenant_id, name, starts_on, ends_on, status,
                   created_at, updated_at, deleted_at
            FROM academic_years
            WHERE tenant_id = $1 AND status = 'active' AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .fetch_optional(pool)
        .await
        .context("Failed to load the active academic year")
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        request: &CreateAcademicYearRequest,
    ) -> Result<AcademicYear> {
        sqlx::query_as::<_, AcademicYear>(
            r#"
            INSERT INTO academic_years (tenant_id, name, starts_on, ends_on, status)
            VALUES ($1, $2, $3, $4, COALESCE($5, 'planned'))
            RETURNING id, tenant_id, name, starts_on, ends_on, status,
                      created_at, updated_at, deleted_at
            "#,
        )
        .bind(tenant_id)
        .bind(request.name.trim())
        .bind(request.starts_on)
        .bind(request.ends_on)
        .bind(request.status.map(|value| value.as_str()))
        .fetch_one(pool)
        .await
        .context("Failed to create academic year")
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        request: &UpdateAcademicYearRequest,
    ) -> Result<Option<AcademicYear>> {
        if !request.dates_are_valid() {
            bail!("Academic year end date cannot be before its start date");
        }
        let mut tx = pool
            .begin()
            .await
            .context("Failed to start academic year update")?;
        let Some(current) = sqlx::query_as::<_, AcademicYear>(
            r#"
            SELECT id, tenant_id, name, starts_on, ends_on, status,
                   created_at, updated_at, deleted_at
            FROM academic_years
            WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            FOR UPDATE
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .context("Failed to lock academic year")?
        else {
            return Ok(None);
        };
        validate_year_transition(&current.status, request.status.as_str())?;
        let terms_fit = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT NOT EXISTS(
                SELECT 1 FROM academic_terms
                WHERE tenant_id = $1 AND academic_year_id = $2 AND deleted_at IS NULL
                  AND (starts_on < $3 OR ends_on > $4)
            )
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(request.starts_on)
        .bind(request.ends_on)
        .fetch_one(&mut *tx)
        .await
        .context("Failed to validate academic term boundaries")?;
        if !terms_fit {
            bail!("Academic year dates must contain every existing term");
        }
        if request.status.as_str() == "closed" {
            let has_open_terms = sqlx::query_scalar::<_, bool>(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM academic_terms
                    WHERE tenant_id = $1 AND academic_year_id = $2
                      AND deleted_at IS NULL AND status <> 'closed'
                )
                "#,
            )
            .bind(tenant_id)
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .context("Failed to validate academic term lifecycle")?;
            if has_open_terms {
                bail!("Every academic term must be closed before closing the academic year");
            }
        }
        let updated = sqlx::query_as::<_, AcademicYear>(
            r#"
            UPDATE academic_years
            SET name = $1, starts_on = $2, ends_on = $3, status = $4,
                updated_at = NOW()
            WHERE tenant_id = $5 AND id = $6 AND deleted_at IS NULL
            RETURNING id, tenant_id, name, starts_on, ends_on, status,
                      created_at, updated_at, deleted_at
            "#,
        )
        .bind(request.name.trim())
        .bind(request.starts_on)
        .bind(request.ends_on)
        .bind(request.status.as_str())
        .bind(tenant_id)
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .context("Failed to update academic year")?;
        tx.commit()
            .await
            .context("Failed to commit academic year update")?;
        Ok(Some(updated))
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<DeleteOutcome> {
        if Self::get_by_id(pool, tenant_id, id).await?.is_none() {
            return Ok(DeleteOutcome::NotFound);
        }
        let in_use = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM class_groups
                WHERE tenant_id = $1 AND academic_year_id = $2 AND deleted_at IS NULL
                UNION ALL
                SELECT 1 FROM academic_terms
                WHERE tenant_id = $1 AND academic_year_id = $2 AND deleted_at IS NULL
            )
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_one(pool)
        .await
        .context("Failed to check academic year references")?;
        if in_use {
            return Ok(DeleteOutcome::InUse);
        }
        sqlx::query(
            "UPDATE academic_years SET deleted_at = NOW() WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete academic year")?;
        Ok(DeleteOutcome::Deleted)
    }
}

pub struct AcademicTermOps;

impl AcademicTermOps {
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
        academic_year_id: Option<Uuid>,
    ) -> Result<(Vec<AcademicTerm>, i64)> {
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let terms = sqlx::query_as::<_, AcademicTerm>(
            r#"
            SELECT term.id, term.tenant_id, term.academic_year_id,
                   academic_year.name AS academic_year_name, term.code, term.name,
                   term.starts_on, term.ends_on, term.status, term.created_at,
                   term.updated_at, term.deleted_at
            FROM academic_terms AS term
            INNER JOIN academic_years AS academic_year
              ON academic_year.id = term.academic_year_id
             AND academic_year.tenant_id = term.tenant_id
            WHERE term.tenant_id = $1 AND term.deleted_at IS NULL
              AND academic_year.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR term.code ILIKE $2 OR term.name ILIKE $2)
              AND ($3::TEXT IS NULL OR term.status = $3)
              AND ($4::UUID IS NULL OR term.academic_year_id = $4)
            ORDER BY term.starts_on DESC, term.name
            LIMIT $5 OFFSET $6
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(academic_year_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list academic terms")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM academic_terms AS term
            INNER JOIN academic_years AS academic_year
              ON academic_year.id = term.academic_year_id
             AND academic_year.tenant_id = term.tenant_id
            WHERE term.tenant_id = $1 AND term.deleted_at IS NULL
              AND academic_year.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR term.code ILIKE $2 OR term.name ILIKE $2)
              AND ($3::TEXT IS NULL OR term.status = $3)
              AND ($4::UUID IS NULL OR term.academic_year_id = $4)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(academic_year_id)
        .fetch_one(pool)
        .await
        .context("Failed to count academic terms")?;
        Ok((terms, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<AcademicTerm>> {
        load_term(pool, tenant_id, id).await
    }

    pub async fn get_active_for_year(
        pool: &PgPool,
        tenant_id: Uuid,
        academic_year_id: Uuid,
    ) -> Result<Option<AcademicTerm>> {
        sqlx::query_as::<_, AcademicTerm>(
            r#"
            SELECT term.id, term.tenant_id, term.academic_year_id,
                   academic_year.name AS academic_year_name, term.code, term.name,
                   term.starts_on, term.ends_on, term.status, term.created_at,
                   term.updated_at, term.deleted_at
            FROM academic_terms AS term
            INNER JOIN academic_years AS academic_year
              ON academic_year.id = term.academic_year_id
             AND academic_year.tenant_id = term.tenant_id
            WHERE term.tenant_id = $1 AND term.academic_year_id = $2
              AND term.status = 'active' AND term.deleted_at IS NULL
              AND academic_year.deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(academic_year_id)
        .fetch_optional(pool)
        .await
        .context("Failed to load the active academic term")
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        request: &CreateAcademicTermRequest,
    ) -> Result<AcademicTerm> {
        if !request.dates_are_valid() {
            bail!("Academic term end date cannot be before its start date");
        }
        let mut tx = pool
            .begin()
            .await
            .context("Failed to start academic term creation")?;
        let year = lock_academic_year(&mut tx, tenant_id, request.academic_year_id).await?;
        validate_term_against_year(
            &year,
            request.starts_on,
            request.ends_on,
            request
                .status
                .map(|status| status.as_str())
                .unwrap_or("planned"),
        )?;
        ensure_no_term_overlap(
            &mut tx,
            tenant_id,
            request.academic_year_id,
            None,
            request.starts_on,
            request.ends_on,
        )
        .await?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO academic_terms
                (tenant_id, academic_year_id, code, name, starts_on, ends_on, status)
            VALUES ($1, $2, $3, $4, $5, $6, COALESCE($7, 'planned'))
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(request.academic_year_id)
        .bind(request.code.trim())
        .bind(request.name.trim())
        .bind(request.starts_on)
        .bind(request.ends_on)
        .bind(request.status.map(|status| status.as_str()))
        .fetch_one(&mut *tx)
        .await
        .context("Failed to create academic term")?;
        tx.commit()
            .await
            .context("Failed to commit academic term creation")?;
        load_term(pool, tenant_id, id)
            .await?
            .context("Created academic term could not be reloaded")
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        request: &UpdateAcademicTermRequest,
    ) -> Result<Option<AcademicTerm>> {
        if !request.dates_are_valid() {
            bail!("Academic term end date cannot be before its start date");
        }
        let mut tx = pool
            .begin()
            .await
            .context("Failed to start academic term update")?;
        let Some(current) = lock_term(&mut tx, tenant_id, id).await? else {
            return Ok(None);
        };
        validate_term_transition(&current.status, request.status.as_str())?;
        if current.status == "active"
            && (current.academic_year_id != request.academic_year_id
                || !current.code.eq_ignore_ascii_case(request.code.trim())
                || current.starts_on != request.starts_on
                || current.ends_on != request.ends_on)
        {
            bail!("An active academic term keeps its year, code, and dates");
        }
        let year = lock_academic_year(&mut tx, tenant_id, request.academic_year_id).await?;
        validate_term_against_year(
            &year,
            request.starts_on,
            request.ends_on,
            request.status.as_str(),
        )?;
        ensure_no_term_overlap(
            &mut tx,
            tenant_id,
            request.academic_year_id,
            Some(id),
            request.starts_on,
            request.ends_on,
        )
        .await?;
        sqlx::query(
            r#"
            UPDATE academic_terms
            SET academic_year_id = $1, code = $2, name = $3, starts_on = $4,
                ends_on = $5, status = $6, updated_at = NOW()
            WHERE tenant_id = $7 AND id = $8 AND deleted_at IS NULL
            "#,
        )
        .bind(request.academic_year_id)
        .bind(request.code.trim())
        .bind(request.name.trim())
        .bind(request.starts_on)
        .bind(request.ends_on)
        .bind(request.status.as_str())
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *tx)
        .await
        .context("Failed to update academic term")?;
        tx.commit()
            .await
            .context("Failed to commit academic term update")?;
        load_term(pool, tenant_id, id).await
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<DeleteOutcome> {
        let Some(term) = Self::get_by_id(pool, tenant_id, id).await? else {
            return Ok(DeleteOutcome::NotFound);
        };
        if term.status != "planned" {
            bail!("Only planned academic terms can be removed");
        }
        sqlx::query(
            "UPDATE academic_terms SET deleted_at = NOW() WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete academic term")?;
        Ok(DeleteOutcome::Deleted)
    }
}

pub struct SubjectOps;

impl SubjectOps {
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
    ) -> Result<(Vec<Subject>, i64)> {
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let subjects = sqlx::query_as::<_, Subject>(
            r#"
            SELECT id, tenant_id, code, name, status, created_at, updated_at, deleted_at
            FROM subjects
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND ($2::TEXT IS NULL OR code ILIKE $2 OR name ILIKE $2)
              AND ($3::TEXT IS NULL OR status = $3)
            ORDER BY name, code
            LIMIT $4 OFFSET $5
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list subjects")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM subjects
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND ($2::TEXT IS NULL OR code ILIKE $2 OR name ILIKE $2)
              AND ($3::TEXT IS NULL OR status = $3)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .fetch_one(pool)
        .await
        .context("Failed to count subjects")?;
        Ok((subjects, total))
    }

    pub async fn get_by_id(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<Option<Subject>> {
        sqlx::query_as::<_, Subject>(
            r#"
            SELECT id, tenant_id, code, name, status, created_at, updated_at, deleted_at
            FROM subjects
            WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load subject")
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        request: &CreateSubjectRequest,
    ) -> Result<Subject> {
        sqlx::query_as::<_, Subject>(
            r#"
            INSERT INTO subjects (tenant_id, code, name, status)
            VALUES ($1, $2, $3, COALESCE($4, 'active'))
            RETURNING id, tenant_id, code, name, status, created_at, updated_at, deleted_at
            "#,
        )
        .bind(tenant_id)
        .bind(request.code.trim())
        .bind(request.name.trim())
        .bind(request.status.map(|value| value.as_str()))
        .fetch_one(pool)
        .await
        .context("Failed to create subject")
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        request: &UpdateSubjectRequest,
    ) -> Result<Option<Subject>> {
        sqlx::query_as::<_, Subject>(
            r#"
            UPDATE subjects
            SET code = $1, name = $2, status = $3, updated_at = NOW()
            WHERE tenant_id = $4 AND id = $5 AND deleted_at IS NULL
            RETURNING id, tenant_id, code, name, status, created_at, updated_at, deleted_at
            "#,
        )
        .bind(request.code.trim())
        .bind(request.name.trim())
        .bind(request.status.as_str())
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to update subject")
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<DeleteOutcome> {
        if Self::get_by_id(pool, tenant_id, id).await?.is_none() {
            return Ok(DeleteOutcome::NotFound);
        }
        let in_use = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM teaching_assignments WHERE tenant_id = $1 AND subject_id = $2 AND deleted_at IS NULL)",
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_one(pool)
        .await
        .context("Failed to check subject references")?;
        if in_use {
            return Ok(DeleteOutcome::InUse);
        }
        sqlx::query(
            "UPDATE subjects SET deleted_at = NOW() WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete subject")?;
        Ok(DeleteOutcome::Deleted)
    }
}

pub struct AcademicGradeLevelOps;

impl AcademicGradeLevelOps {
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
    ) -> Result<(Vec<AcademicGradeLevel>, i64)> {
        let page = page.max(1);
        let per_page = per_page.clamp(1, 100);
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let grade_levels = sqlx::query_as::<_, AcademicGradeLevel>(
            r#"
            SELECT id, tenant_id, code, name, sequence_number, status,
                   created_at, updated_at, deleted_at
            FROM academic_grade_levels
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND ($2::TEXT IS NULL OR code ILIKE $2 OR name ILIKE $2)
              AND ($3::TEXT IS NULL OR status = $3)
            ORDER BY sequence_number, name, code
            LIMIT $4 OFFSET $5
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list academic grade levels")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM academic_grade_levels
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND ($2::TEXT IS NULL OR code ILIKE $2 OR name ILIKE $2)
              AND ($3::TEXT IS NULL OR status = $3)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .fetch_one(pool)
        .await
        .context("Failed to count academic grade levels")?;
        Ok((grade_levels, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<AcademicGradeLevel>> {
        sqlx::query_as::<_, AcademicGradeLevel>(
            r#"
            SELECT id, tenant_id, code, name, sequence_number, status,
                   created_at, updated_at, deleted_at
            FROM academic_grade_levels
            WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load academic grade level")
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        request: &CreateAcademicGradeLevelRequest,
    ) -> Result<AcademicGradeLevel> {
        validate_grade_level_names(&request.code, &request.name)?;
        sqlx::query_as::<_, AcademicGradeLevel>(
            r#"
            INSERT INTO academic_grade_levels
                (tenant_id, code, name, sequence_number, status)
            VALUES ($1, $2, $3, $4, COALESCE($5, 'active'))
            RETURNING id, tenant_id, code, name, sequence_number, status,
                      created_at, updated_at, deleted_at
            "#,
        )
        .bind(tenant_id)
        .bind(request.code.trim())
        .bind(request.name.trim())
        .bind(request.sequence_number)
        .bind(request.status.map(|value| value.as_str()))
        .fetch_one(pool)
        .await
        .context("Failed to create academic grade level")
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        request: &UpdateAcademicGradeLevelRequest,
    ) -> Result<Option<AcademicGradeLevel>> {
        validate_grade_level_names(&request.code, &request.name)?;
        sqlx::query_as::<_, AcademicGradeLevel>(
            r#"
            UPDATE academic_grade_levels
            SET code = $1, name = $2, sequence_number = $3, status = $4,
                updated_at = NOW()
            WHERE tenant_id = $5 AND id = $6 AND deleted_at IS NULL
            RETURNING id, tenant_id, code, name, sequence_number, status,
                      created_at, updated_at, deleted_at
            "#,
        )
        .bind(request.code.trim())
        .bind(request.name.trim())
        .bind(request.sequence_number)
        .bind(request.status.as_str())
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to update academic grade level")
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<DeleteOutcome> {
        if Self::get_by_id(pool, tenant_id, id).await?.is_none() {
            return Ok(DeleteOutcome::NotFound);
        }
        let in_use = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM class_groups WHERE tenant_id = $1 AND grade_level_id = $2 AND deleted_at IS NULL)",
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_one(pool)
        .await
        .context("Failed to check academic grade level references")?;
        if in_use {
            return Ok(DeleteOutcome::InUse);
        }
        sqlx::query(
            "UPDATE academic_grade_levels SET deleted_at = NOW() WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete academic grade level")?;
        Ok(DeleteOutcome::Deleted)
    }
}

pub struct TeacherProfileOps;

impl TeacherProfileOps {
    pub async fn list_candidates(
        pool: &PgPool,
        tenant_id: Uuid,
        search: Option<&str>,
    ) -> Result<Vec<EmployeeReference>> {
        let employees =
            EmployeeOps::list_references(pool, tenant_id, search, Some("active"), 100).await?;
        let assigned = sqlx::query_scalar::<_, Uuid>(
            "SELECT employee_id FROM teacher_profiles WHERE tenant_id = $1 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .context("Failed to load assigned teacher employees")?
        .into_iter()
        .collect::<HashSet<_>>();
        Ok(employees
            .into_iter()
            .filter(|employee| !assigned.contains(&employee.id))
            .collect())
    }

    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
    ) -> Result<(Vec<TeacherProfileWithEmployee>, i64)> {
        let employee_ids = match search {
            Some(value) => EmployeeOps::search_reference_ids(pool, tenant_id, value).await?,
            None => Vec::new(),
        };
        let offset = (page - 1) * per_page;
        let profiles = sqlx::query_as::<_, TeacherProfile>(
            r#"
            SELECT id, tenant_id, employee_id, status, created_at, updated_at, deleted_at
            FROM teacher_profiles
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND ($2::TEXT IS NULL OR employee_id = ANY($3))
              AND ($4::TEXT IS NULL OR status = $4)
            ORDER BY created_at DESC
            LIMIT $5 OFFSET $6
            "#,
        )
        .bind(tenant_id)
        .bind(search)
        .bind(&employee_ids)
        .bind(status)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list teacher profiles")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM teacher_profiles
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND ($2::TEXT IS NULL OR employee_id = ANY($3))
              AND ($4::TEXT IS NULL OR status = $4)
            "#,
        )
        .bind(tenant_id)
        .bind(search)
        .bind(&employee_ids)
        .bind(status)
        .fetch_one(pool)
        .await
        .context("Failed to count teacher profiles")?;
        Ok((hydrate_teachers(pool, tenant_id, profiles).await?, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<TeacherProfileWithEmployee>> {
        let profile = sqlx::query_as::<_, TeacherProfile>(
            r#"
            SELECT id, tenant_id, employee_id, status, created_at, updated_at, deleted_at
            FROM teacher_profiles
            WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load teacher profile")?;
        match profile {
            Some(value) => Ok(Some(hydrate_teacher(pool, tenant_id, value).await?)),
            None => Ok(None),
        }
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        request: &CreateTeacherProfileRequest,
    ) -> Result<TeacherProfileWithEmployee> {
        let employee = EmployeeOps::get_reference(pool, tenant_id, request.employee_id)
            .await?
            .context("Employee was not found for this campus")?;
        if employee.employment_status != "active" {
            bail!("Only an active employee can be assigned as a teacher");
        }
        let profile = sqlx::query_as::<_, TeacherProfile>(
            r#"
            INSERT INTO teacher_profiles (tenant_id, employee_id, status)
            VALUES ($1, $2, COALESCE($3, 'active'))
            RETURNING id, tenant_id, employee_id, status, created_at, updated_at, deleted_at
            "#,
        )
        .bind(tenant_id)
        .bind(request.employee_id)
        .bind(request.status.map(|value| value.as_str()))
        .fetch_one(pool)
        .await
        .context("Failed to create teacher profile")?;
        Ok(teacher_from_parts(profile, employee))
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        request: &UpdateTeacherProfileRequest,
    ) -> Result<Option<TeacherProfileWithEmployee>> {
        let Some(current) = Self::get_by_id(pool, tenant_id, id).await? else {
            return Ok(None);
        };
        if request.status.as_str() == "active" && current.employment_status != "active" {
            bail!("An inactive employee cannot have an active teacher profile");
        }
        let profile = sqlx::query_as::<_, TeacherProfile>(
            r#"
            UPDATE teacher_profiles SET status = $1, updated_at = NOW()
            WHERE tenant_id = $2 AND id = $3 AND deleted_at IS NULL
            RETURNING id, tenant_id, employee_id, status, created_at, updated_at, deleted_at
            "#,
        )
        .bind(request.status.as_str())
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to update teacher profile")?;
        Ok(profile.map(|value| TeacherProfileWithEmployee {
            status: value.status,
            updated_at: value.updated_at,
            ..current
        }))
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<DeleteOutcome> {
        if Self::get_by_id(pool, tenant_id, id).await?.is_none() {
            return Ok(DeleteOutcome::NotFound);
        }
        let in_use = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM teaching_assignments WHERE tenant_id = $1 AND teacher_profile_id = $2 AND deleted_at IS NULL)",
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_one(pool)
        .await
        .context("Failed to check teacher references")?;
        if in_use {
            return Ok(DeleteOutcome::InUse);
        }
        sqlx::query(
            "UPDATE teacher_profiles SET deleted_at = NOW() WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete teacher profile")?;
        Ok(DeleteOutcome::Deleted)
    }
}

pub struct ClassGroupOps;

impl ClassGroupOps {
    #[allow(clippy::too_many_arguments)]
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
        academic_year_id: Option<Uuid>,
        grade_level_id: Option<Uuid>,
    ) -> Result<(Vec<ClassGroupWithYear>, i64)> {
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let classes = sqlx::query_as::<_, ClassGroupWithYear>(
            r#"
            SELECT class_group.id, class_group.tenant_id, class_group.academic_year_id,
                   academic_year.name AS academic_year_name, class_group.code,
                   class_group.name, class_group.grade_level_id,
                   grade_level.name AS grade_level, class_group.status,
                   class_group.created_at, class_group.updated_at
            FROM class_groups AS class_group
            INNER JOIN academic_years AS academic_year
              ON academic_year.id = class_group.academic_year_id
             AND academic_year.tenant_id = class_group.tenant_id
            LEFT JOIN academic_grade_levels AS grade_level
              ON grade_level.id = class_group.grade_level_id
             AND grade_level.tenant_id = class_group.tenant_id
             AND grade_level.deleted_at IS NULL
            WHERE class_group.tenant_id = $1 AND class_group.deleted_at IS NULL
              AND academic_year.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR class_group.code ILIKE $2 OR class_group.name ILIKE $2)
              AND ($3::TEXT IS NULL OR class_group.status = $3)
              AND ($4::UUID IS NULL OR class_group.academic_year_id = $4)
              AND ($5::UUID IS NULL OR class_group.grade_level_id = $5)
            ORDER BY academic_year.starts_on DESC, grade_level.sequence_number,
                     class_group.name, class_group.code
            LIMIT $6 OFFSET $7
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(academic_year_id)
        .bind(grade_level_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list class groups")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM class_groups AS class_group
            WHERE class_group.tenant_id = $1 AND class_group.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR class_group.code ILIKE $2 OR class_group.name ILIKE $2)
              AND ($3::TEXT IS NULL OR class_group.status = $3)
              AND ($4::UUID IS NULL OR class_group.academic_year_id = $4)
              AND ($5::UUID IS NULL OR class_group.grade_level_id = $5)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(academic_year_id)
        .bind(grade_level_id)
        .fetch_one(pool)
        .await
        .context("Failed to count class groups")?;
        Ok((classes, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<ClassGroupWithYear>> {
        sqlx::query_as::<_, ClassGroupWithYear>(
            r#"
            SELECT class_group.id, class_group.tenant_id, class_group.academic_year_id,
                   academic_year.name AS academic_year_name, class_group.code,
                   class_group.name, class_group.grade_level_id,
                   grade_level.name AS grade_level, class_group.status,
                   class_group.created_at, class_group.updated_at
            FROM class_groups AS class_group
            INNER JOIN academic_years AS academic_year
              ON academic_year.id = class_group.academic_year_id
             AND academic_year.tenant_id = class_group.tenant_id
            LEFT JOIN academic_grade_levels AS grade_level
              ON grade_level.id = class_group.grade_level_id
             AND grade_level.tenant_id = class_group.tenant_id
             AND grade_level.deleted_at IS NULL
            WHERE class_group.tenant_id = $1 AND class_group.id = $2
              AND class_group.deleted_at IS NULL AND academic_year.deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load class group")
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        request: &CreateClassGroupRequest,
    ) -> Result<ClassGroupWithYear> {
        ensure_academic_year(pool, tenant_id, request.academic_year_id).await?;
        ensure_grade_level(pool, tenant_id, request.grade_level_id).await?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO class_groups
                (tenant_id, academic_year_id, code, name, grade_level_id, status)
            VALUES ($1, $2, $3, $4, $5, COALESCE($6, 'active'))
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(request.academic_year_id)
        .bind(request.code.trim())
        .bind(request.name.trim())
        .bind(request.grade_level_id)
        .bind(request.status.map(|value| value.as_str()))
        .fetch_one(pool)
        .await
        .context("Failed to create class group")?;
        Self::get_by_id(pool, tenant_id, id)
            .await?
            .context("Created class group could not be reloaded")
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        request: &UpdateClassGroupRequest,
    ) -> Result<Option<ClassGroupWithYear>> {
        if Self::get_by_id(pool, tenant_id, id).await?.is_none() {
            return Ok(None);
        }
        ensure_academic_year(pool, tenant_id, request.academic_year_id).await?;
        ensure_grade_level(pool, tenant_id, request.grade_level_id).await?;
        sqlx::query(
            r#"
            UPDATE class_groups
            SET academic_year_id = $1, code = $2, name = $3, grade_level_id = $4,
                status = $5, updated_at = NOW()
            WHERE tenant_id = $6 AND id = $7 AND deleted_at IS NULL
            "#,
        )
        .bind(request.academic_year_id)
        .bind(request.code.trim())
        .bind(request.name.trim())
        .bind(request.grade_level_id)
        .bind(request.status.as_str())
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to update class group")?;
        Self::get_by_id(pool, tenant_id, id).await
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<DeleteOutcome> {
        if Self::get_by_id(pool, tenant_id, id).await?.is_none() {
            return Ok(DeleteOutcome::NotFound);
        }
        let in_use = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM teaching_assignments WHERE tenant_id = $1 AND class_group_id = $2 AND deleted_at IS NULL)",
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_one(pool)
        .await
        .context("Failed to check class references")?;
        if in_use {
            return Ok(DeleteOutcome::InUse);
        }
        sqlx::query(
            "UPDATE class_groups SET deleted_at = NOW() WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete class group")?;
        Ok(DeleteOutcome::Deleted)
    }
}

pub struct TeachingAssignmentOps;

impl TeachingAssignmentOps {
    /// Returns the distinct active classes taught by the employee linked to
    /// one authenticated account. Attendance uses this typed boundary to
    /// enforce assigned-class visibility before it queries or mutates data.
    pub async fn active_class_ids_for_account(
        pool: &PgPool,
        tenant_id: Uuid,
        account_id: Uuid,
    ) -> Result<Vec<Uuid>> {
        let Some(employee) =
            EmployeeOps::active_reference_by_account(pool, tenant_id, account_id).await?
        else {
            return Ok(Vec::new());
        };
        sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT DISTINCT assignment.class_group_id
              FROM teaching_assignments AS assignment
              JOIN teacher_profiles AS teacher
                ON teacher.id = assignment.teacher_profile_id
               AND teacher.tenant_id = assignment.tenant_id
               AND teacher.deleted_at IS NULL
             WHERE assignment.tenant_id = $1
               AND teacher.employee_id = $2
               AND assignment.status = 'active'
               AND assignment.deleted_at IS NULL
             ORDER BY assignment.class_group_id
            "#,
        )
        .bind(tenant_id)
        .bind(employee.id)
        .fetch_all(pool)
        .await
        .context("Failed to resolve assigned teaching classes")
    }

    /// Returns active assignment identifiers owned by the employee linked to
    /// one authenticated account. Consumers use these identifiers to apply
    /// assigned-record scope before pagination.
    pub async fn active_ids_for_account(
        pool: &PgPool,
        tenant_id: Uuid,
        account_id: Uuid,
    ) -> Result<Vec<Uuid>> {
        let Some(employee) =
            EmployeeOps::active_reference_by_account(pool, tenant_id, account_id).await?
        else {
            return Ok(Vec::new());
        };
        sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT assignment.id
              FROM teaching_assignments AS assignment
              JOIN teacher_profiles AS teacher
                ON teacher.id = assignment.teacher_profile_id
               AND teacher.tenant_id = assignment.tenant_id
               AND teacher.deleted_at IS NULL
             WHERE assignment.tenant_id = $1
               AND teacher.employee_id = $2
               AND assignment.status = 'active'
               AND assignment.deleted_at IS NULL
             ORDER BY assignment.id
            "#,
        )
        .bind(tenant_id)
        .bind(employee.id)
        .fetch_all(pool)
        .await
        .context("Failed to resolve assigned teaching work")
    }

    /// Lists the active teaching assignments attached to one current account.
    pub async fn active_for_account(
        pool: &PgPool,
        tenant_id: Uuid,
        account_id: Uuid,
    ) -> Result<Vec<TeachingAssignmentWithDetails>> {
        let ids = Self::active_ids_for_account(pool, tenant_id, account_id).await?;
        let mut assignments = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(assignment) = Self::get_by_id(pool, tenant_id, id).await? {
                assignments.push(assignment);
            }
        }
        assignments.sort_by(|left, right| {
            left.class_group_name
                .cmp(&right.class_group_name)
                .then(left.subject_name.cmp(&right.subject_name))
        });
        Ok(assignments)
    }

    /// Re-checks that one assignment still belongs to the current account.
    pub async fn is_active_for_account(
        pool: &PgPool,
        tenant_id: Uuid,
        assignment_id: Uuid,
        account_id: Uuid,
    ) -> Result<bool> {
        Ok(Self::active_ids_for_account(pool, tenant_id, account_id)
            .await?
            .contains(&assignment_id))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        status: Option<&str>,
        academic_year_id: Option<Uuid>,
        class_group_id: Option<Uuid>,
        teacher_profile_id: Option<Uuid>,
    ) -> Result<(Vec<TeachingAssignmentWithDetails>, i64)> {
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, TeachingAssignmentRow>(
            r#"
            SELECT assignment.id, assignment.tenant_id, assignment.academic_year_id,
                   academic_year.name AS academic_year_name,
                   assignment.class_group_id, class_group.name AS class_group_name,
                   assignment.subject_id, subject.name AS subject_name,
                   assignment.teacher_profile_id, teacher.employee_id,
                   assignment.periods_per_cycle, assignment.status,
                   assignment.created_at, assignment.updated_at
            FROM teaching_assignments AS assignment
            INNER JOIN academic_years AS academic_year
              ON academic_year.id = assignment.academic_year_id
             AND academic_year.tenant_id = assignment.tenant_id
            INNER JOIN class_groups AS class_group
              ON class_group.id = assignment.class_group_id
             AND class_group.tenant_id = assignment.tenant_id
            INNER JOIN subjects AS subject
              ON subject.id = assignment.subject_id
             AND subject.tenant_id = assignment.tenant_id
            INNER JOIN teacher_profiles AS teacher
              ON teacher.id = assignment.teacher_profile_id
             AND teacher.tenant_id = assignment.tenant_id
            WHERE assignment.tenant_id = $1 AND assignment.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR assignment.status = $2)
              AND ($3::UUID IS NULL OR assignment.academic_year_id = $3)
              AND ($4::UUID IS NULL OR assignment.class_group_id = $4)
              AND ($5::UUID IS NULL OR assignment.teacher_profile_id = $5)
            ORDER BY class_group.name, subject.name, assignment.created_at
            LIMIT $6 OFFSET $7
            "#,
        )
        .bind(tenant_id)
        .bind(status)
        .bind(academic_year_id)
        .bind(class_group_id)
        .bind(teacher_profile_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list teaching assignments")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM teaching_assignments
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND ($2::TEXT IS NULL OR status = $2)
              AND ($3::UUID IS NULL OR academic_year_id = $3)
              AND ($4::UUID IS NULL OR class_group_id = $4)
              AND ($5::UUID IS NULL OR teacher_profile_id = $5)
            "#,
        )
        .bind(tenant_id)
        .bind(status)
        .bind(academic_year_id)
        .bind(class_group_id)
        .bind(teacher_profile_id)
        .fetch_one(pool)
        .await
        .context("Failed to count teaching assignments")?;
        Ok((hydrate_assignments(pool, tenant_id, rows).await?, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<TeachingAssignmentWithDetails>> {
        let row = sqlx::query_as::<_, TeachingAssignmentRow>(
            r#"
            SELECT assignment.id, assignment.tenant_id, assignment.academic_year_id,
                   academic_year.name AS academic_year_name,
                   assignment.class_group_id, class_group.name AS class_group_name,
                   assignment.subject_id, subject.name AS subject_name,
                   assignment.teacher_profile_id, teacher.employee_id,
                   assignment.periods_per_cycle, assignment.status,
                   assignment.created_at, assignment.updated_at
            FROM teaching_assignments AS assignment
            INNER JOIN academic_years AS academic_year
              ON academic_year.id = assignment.academic_year_id
             AND academic_year.tenant_id = assignment.tenant_id
            INNER JOIN class_groups AS class_group
              ON class_group.id = assignment.class_group_id
             AND class_group.tenant_id = assignment.tenant_id
            INNER JOIN subjects AS subject
              ON subject.id = assignment.subject_id
             AND subject.tenant_id = assignment.tenant_id
            INNER JOIN teacher_profiles AS teacher
              ON teacher.id = assignment.teacher_profile_id
             AND teacher.tenant_id = assignment.tenant_id
            WHERE assignment.tenant_id = $1 AND assignment.id = $2
              AND assignment.deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load teaching assignment")?;
        match row {
            Some(value) => Ok(Some(hydrate_assignment(pool, tenant_id, value).await?)),
            None => Ok(None),
        }
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        request: &CreateTeachingAssignmentRequest,
    ) -> Result<TeachingAssignmentWithDetails> {
        ensure_assignment_references(
            pool,
            tenant_id,
            request.academic_year_id,
            request.class_group_id,
            request.subject_id,
            request.teacher_profile_id,
        )
        .await?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO teaching_assignments (
                tenant_id, academic_year_id, class_group_id, subject_id,
                teacher_profile_id, periods_per_cycle, status
            )
            VALUES ($1, $2, $3, $4, $5, $6, COALESCE($7, 'active'))
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(request.academic_year_id)
        .bind(request.class_group_id)
        .bind(request.subject_id)
        .bind(request.teacher_profile_id)
        .bind(request.periods_per_cycle)
        .bind(request.status.map(|value| value.as_str()))
        .fetch_one(pool)
        .await
        .context("Failed to create teaching assignment")?;
        Self::get_by_id(pool, tenant_id, id)
            .await?
            .context("Created teaching assignment could not be reloaded")
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        request: &UpdateTeachingAssignmentRequest,
    ) -> Result<Option<TeachingAssignmentWithDetails>> {
        if Self::get_by_id(pool, tenant_id, id).await?.is_none() {
            return Ok(None);
        }
        ensure_assignment_references(
            pool,
            tenant_id,
            request.academic_year_id,
            request.class_group_id,
            request.subject_id,
            request.teacher_profile_id,
        )
        .await?;
        sqlx::query(
            r#"
            UPDATE teaching_assignments
            SET academic_year_id = $1, class_group_id = $2, subject_id = $3,
                teacher_profile_id = $4, periods_per_cycle = $5, status = $6,
                updated_at = NOW()
            WHERE tenant_id = $7 AND id = $8 AND deleted_at IS NULL
            "#,
        )
        .bind(request.academic_year_id)
        .bind(request.class_group_id)
        .bind(request.subject_id)
        .bind(request.teacher_profile_id)
        .bind(request.periods_per_cycle)
        .bind(request.status.as_str())
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to update teaching assignment")?;
        Self::get_by_id(pool, tenant_id, id).await
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE teaching_assignments SET deleted_at = NOW() WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete teaching assignment")?;
        Ok(result.rows_affected() > 0)
    }

    /// Returns the canonical active teaching structure consumed by Timetabling.
    pub async fn timetabling_reference_data(
        pool: &PgPool,
        tenant_id: Uuid,
    ) -> Result<Option<TimetablingReferenceData>> {
        let Some(academic_year) = AcademicYearOps::get_active(pool, tenant_id).await? else {
            return Ok(None);
        };
        let active_term =
            AcademicTermOps::get_active_for_year(pool, tenant_id, academic_year.id).await?;
        let (classes, _) = ClassGroupOps::list(
            pool,
            tenant_id,
            1,
            1_000,
            None,
            Some("active"),
            Some(academic_year.id),
            None,
        )
        .await?;
        let (subjects, _) =
            SubjectOps::list(pool, tenant_id, 1, 1_000, None, Some("active")).await?;
        let (teachers, _) =
            TeacherProfileOps::list(pool, tenant_id, 1, 1_000, None, Some("active")).await?;
        let (assignments, _) = Self::list(
            pool,
            tenant_id,
            1,
            5_000,
            Some("active"),
            Some(academic_year.id),
            None,
            None,
        )
        .await?;
        Ok(Some(TimetablingReferenceData {
            academic_year,
            active_term,
            classes,
            subjects,
            teachers,
            assignments,
        }))
    }
}

async fn ensure_academic_year(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<()> {
    AcademicYearOps::get_by_id(pool, tenant_id, id)
        .await?
        .context("Academic year was not found for this campus")?;
    Ok(())
}

async fn ensure_grade_level(pool: &PgPool, tenant_id: Uuid, id: Option<Uuid>) -> Result<()> {
    let Some(id) = id else {
        return Ok(());
    };
    let grade_level = AcademicGradeLevelOps::get_by_id(pool, tenant_id, id)
        .await?
        .context("Academic grade level was not found for this campus")?;
    if grade_level.status != "active" {
        bail!("Academic grade level must be active before assigning a class");
    }
    Ok(())
}

fn validate_grade_level_names(code: &str, name: &str) -> Result<()> {
    if code.trim().is_empty() || name.trim().is_empty() {
        bail!("Academic grade level code and name are required");
    }
    Ok(())
}

async fn load_term(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<Option<AcademicTerm>> {
    sqlx::query_as::<_, AcademicTerm>(
        r#"
        SELECT term.id, term.tenant_id, term.academic_year_id,
               academic_year.name AS academic_year_name, term.code, term.name,
               term.starts_on, term.ends_on, term.status, term.created_at,
               term.updated_at, term.deleted_at
        FROM academic_terms AS term
        INNER JOIN academic_years AS academic_year
          ON academic_year.id = term.academic_year_id
         AND academic_year.tenant_id = term.tenant_id
        WHERE term.tenant_id = $1 AND term.id = $2
          AND term.deleted_at IS NULL AND academic_year.deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("Failed to load academic term")
}

async fn lock_term(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<AcademicTerm>> {
    sqlx::query_as::<_, AcademicTerm>(
        r#"
        SELECT term.id, term.tenant_id, term.academic_year_id,
               academic_year.name AS academic_year_name, term.code, term.name,
               term.starts_on, term.ends_on, term.status, term.created_at,
               term.updated_at, term.deleted_at
        FROM academic_terms AS term
        INNER JOIN academic_years AS academic_year
          ON academic_year.id = term.academic_year_id
         AND academic_year.tenant_id = term.tenant_id
        WHERE term.tenant_id = $1 AND term.id = $2
          AND term.deleted_at IS NULL AND academic_year.deleted_at IS NULL
        FOR UPDATE OF term
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .context("Failed to lock academic term")
}

async fn lock_academic_year(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<AcademicYear> {
    sqlx::query_as::<_, AcademicYear>(
        r#"
        SELECT id, tenant_id, name, starts_on, ends_on, status,
               created_at, updated_at, deleted_at
        FROM academic_years
        WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .context("Failed to lock academic year")?
    .context("Academic year was not found for this campus")
}

async fn ensure_no_term_overlap(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    academic_year_id: Uuid,
    excluded_id: Option<Uuid>,
    starts_on: chrono::NaiveDate,
    ends_on: chrono::NaiveDate,
) -> Result<()> {
    let overlaps = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM academic_terms
            WHERE tenant_id = $1 AND academic_year_id = $2 AND deleted_at IS NULL
              AND ($3::UUID IS NULL OR id <> $3)
              AND NOT (ends_on < $4 OR starts_on > $5)
        )
        "#,
    )
    .bind(tenant_id)
    .bind(academic_year_id)
    .bind(excluded_id)
    .bind(starts_on)
    .bind(ends_on)
    .fetch_one(&mut **tx)
    .await
    .context("Failed to check academic term overlap")?;
    if overlaps {
        bail!("Academic term dates cannot overlap another term in the selected year");
    }
    Ok(())
}

fn validate_term_against_year(
    year: &AcademicYear,
    starts_on: chrono::NaiveDate,
    ends_on: chrono::NaiveDate,
    status: &str,
) -> Result<()> {
    if starts_on < year.starts_on || ends_on > year.ends_on {
        bail!("Academic term dates must fall within the selected academic year");
    }
    if status == "active" && year.status != "active" {
        bail!("Only a term in the active academic year can be activated");
    }
    if year.status == "closed" {
        bail!("A closed academic year cannot accept term changes");
    }
    Ok(())
}

fn validate_year_transition(current: &str, requested: &str) -> Result<()> {
    let allowed = matches!(
        (current, requested),
        ("planned", "planned")
            | ("planned", "active")
            | ("planned", "closed")
            | ("active", "active")
            | ("active", "closed")
    );
    if !allowed {
        bail!("Academic year lifecycle cannot move backwards or change after closure");
    }
    Ok(())
}

fn validate_term_transition(current: &str, requested: &str) -> Result<()> {
    let allowed = matches!(
        (current, requested),
        ("planned", "planned")
            | ("planned", "active")
            | ("planned", "closed")
            | ("active", "active")
            | ("active", "closed")
    );
    if !allowed {
        bail!("Academic term lifecycle cannot move backwards or change after closure");
    }
    Ok(())
}

async fn ensure_assignment_references(
    pool: &PgPool,
    tenant_id: Uuid,
    academic_year_id: Uuid,
    class_group_id: Uuid,
    subject_id: Uuid,
    teacher_profile_id: Uuid,
) -> Result<()> {
    let class_group = ClassGroupOps::get_by_id(pool, tenant_id, class_group_id)
        .await?
        .context("Class was not found for this campus")?;
    if class_group.academic_year_id != academic_year_id {
        bail!("The class does not belong to the selected academic year");
    }
    let subject = SubjectOps::get_by_id(pool, tenant_id, subject_id)
        .await?
        .context("Subject was not found for this campus")?;
    if subject.status != "active" {
        bail!("Only an active subject can be assigned");
    }
    let teacher = TeacherProfileOps::get_by_id(pool, tenant_id, teacher_profile_id)
        .await?
        .context("Teacher was not found for this campus")?;
    if teacher.status != "active" || teacher.employment_status != "active" {
        bail!("Only an active employed teacher can be assigned");
    }
    Ok(())
}

async fn hydrate_teachers(
    pool: &PgPool,
    tenant_id: Uuid,
    profiles: Vec<TeacherProfile>,
) -> Result<Vec<TeacherProfileWithEmployee>> {
    let employee_ids = profiles
        .iter()
        .map(|profile| profile.employee_id)
        .collect::<Vec<_>>();
    let mut employees = EmployeeOps::references_by_ids(pool, tenant_id, &employee_ids)
        .await?
        .into_iter()
        .map(|employee| (employee.id, employee))
        .collect::<HashMap<_, _>>();
    profiles
        .into_iter()
        .map(|profile| {
            let employee = employees
                .remove(&profile.employee_id)
                .context("Teacher employee reference is unavailable")?;
            Ok(teacher_from_parts(profile, employee))
        })
        .collect()
}

async fn hydrate_teacher(
    pool: &PgPool,
    tenant_id: Uuid,
    profile: TeacherProfile,
) -> Result<TeacherProfileWithEmployee> {
    let employee = EmployeeOps::get_reference(pool, tenant_id, profile.employee_id)
        .await?
        .context("Teacher employee reference is unavailable")?;
    Ok(teacher_from_parts(profile, employee))
}

fn teacher_from_parts(
    profile: TeacherProfile,
    employee: EmployeeReference,
) -> TeacherProfileWithEmployee {
    TeacherProfileWithEmployee {
        id: profile.id,
        tenant_id: profile.tenant_id,
        employee_id: profile.employee_id,
        employee_number: employee.employee_number,
        display_name: employee.display_name,
        work_email: employee.work_email,
        phone: employee.phone,
        employment_status: employee.employment_status,
        status: profile.status,
        created_at: profile.created_at,
        updated_at: profile.updated_at,
    }
}

async fn hydrate_assignments(
    pool: &PgPool,
    tenant_id: Uuid,
    rows: Vec<TeachingAssignmentRow>,
) -> Result<Vec<TeachingAssignmentWithDetails>> {
    let employee_ids = rows.iter().map(|row| row.employee_id).collect::<Vec<_>>();
    let employees = EmployeeOps::references_by_ids(pool, tenant_id, &employee_ids)
        .await?
        .into_iter()
        .map(|employee| (employee.id, employee))
        .collect::<HashMap<_, _>>();
    rows.into_iter()
        .map(|row| {
            let employee = employees
                .get(&row.employee_id)
                .context("Assignment teacher employee reference is unavailable")?;
            Ok(assignment_from_row(row, employee.display_name.clone()))
        })
        .collect()
}

async fn hydrate_assignment(
    pool: &PgPool,
    tenant_id: Uuid,
    row: TeachingAssignmentRow,
) -> Result<TeachingAssignmentWithDetails> {
    let employee = EmployeeOps::get_reference(pool, tenant_id, row.employee_id)
        .await?
        .context("Assignment teacher employee reference is unavailable")?;
    Ok(assignment_from_row(row, employee.display_name))
}

fn assignment_from_row(
    row: TeachingAssignmentRow,
    teacher_name: String,
) -> TeachingAssignmentWithDetails {
    TeachingAssignmentWithDetails {
        id: row.id,
        tenant_id: row.tenant_id,
        academic_year_id: row.academic_year_id,
        academic_year_name: row.academic_year_name,
        class_group_id: row.class_group_id,
        class_group_name: row.class_group_name,
        subject_id: row.subject_id,
        subject_name: row.subject_name,
        teacher_profile_id: row.teacher_profile_id,
        employee_id: row.employee_id,
        teacher_name,
        periods_per_cycle: row.periods_per_cycle,
        status: row.status,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DeleteOutcome, validate_grade_level_names, validate_term_transition,
        validate_year_transition,
    };

    #[test]
    fn delete_outcomes_are_distinct() {
        assert_ne!(DeleteOutcome::Deleted, DeleteOutcome::InUse);
        assert_ne!(DeleteOutcome::NotFound, DeleteOutcome::Deleted);
    }

    #[test]
    fn academic_lifecycles_only_move_forward() {
        assert!(validate_year_transition("planned", "active").is_ok());
        assert!(validate_year_transition("active", "closed").is_ok());
        assert!(validate_year_transition("closed", "active").is_err());
        assert!(validate_term_transition("planned", "closed").is_ok());
        assert!(validate_term_transition("active", "planned").is_err());
        assert!(validate_term_transition("closed", "closed").is_err());
    }

    #[test]
    fn grade_level_references_require_operational_names() {
        assert!(validate_grade_level_names("FORM-1", "Form 1").is_ok());
        assert!(validate_grade_level_names("   ", "Form 1").is_err());
        assert!(validate_grade_level_names("FORM-1", "\n").is_err());
    }
}
