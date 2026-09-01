//
//  cp-hr-payroll
//  ops.rs
//
//  Created by OpenAI Codex on 2026/08/27.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, NaiveDate, Utc};
use cp_audit::AuditActor;
use cp_common::{AccessContext, EffectiveRecordScope, RecordScopeFamilyKey, RecordScopeGrants};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    dtos::{
        CreateDepartmentRequest, CreateEmployeeAvailabilityRequest, CreateEmployeeRequest,
        CreateEmploymentEngagementRequest, CreatePositionRequest, UpdateDepartmentRequest,
        UpdateEmployeeAvailabilityRequest, UpdateEmployeeRequest,
        UpdateEmploymentEngagementRequest, UpdatePositionRequest,
    },
    models::{
        CommunicationDepartmentReference, CommunicationEmployeeAccountReference, Department,
        EmployeeAvailabilityReference, EmployeeAvailabilityWithDetails, EmployeeReference,
        EmployeeWithDetails, EmploymentEngagementWithDetails, Position,
        StockRequestDepartmentReference, StockRequestEmployeeReference,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteOutcome {
    Deleted,
    NotFound,
    InUse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HrRecordVisibility {
    Campus,
    SelfAccount(Uuid),
}

impl HrRecordVisibility {
    const fn account_filter(self) -> Option<Uuid> {
        match self {
            Self::Campus => None,
            Self::SelfAccount(account_id) => Some(account_id),
        }
    }
}

/// Proof that the current request may read HR employee records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmployeeReadScope(HrRecordVisibility);

impl EmployeeReadScope {
    /// Refines current request authority for the exact `hr.employees` family.
    pub fn from_authority(
        access: &AccessContext,
        grants: &RecordScopeGrants,
        actor: AuditActor,
    ) -> Option<Self> {
        employee_visibility(access, grants, actor, "hr.employees").map(Self)
    }

    const fn account_filter(self) -> Option<Uuid> {
        self.0.account_filter()
    }
}

/// Proof that the current request may read HR employment engagements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmploymentEngagementReadScope(HrRecordVisibility);

impl EmploymentEngagementReadScope {
    /// Refines current request authority for the exact `hr.engagements` family.
    pub fn from_authority(
        access: &AccessContext,
        grants: &RecordScopeGrants,
        actor: AuditActor,
    ) -> Option<Self> {
        employee_visibility(access, grants, actor, "hr.engagements").map(Self)
    }

    const fn account_filter(self) -> Option<Uuid> {
        self.0.account_filter()
    }
}

/// Proof that the current request may read HR availability periods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmployeeAvailabilityReadScope(HrRecordVisibility);

impl EmployeeAvailabilityReadScope {
    /// Refines current request authority for the exact `hr.availability` family.
    pub fn from_authority(
        access: &AccessContext,
        grants: &RecordScopeGrants,
        actor: AuditActor,
    ) -> Option<Self> {
        employee_visibility(access, grants, actor, "hr.availability").map(Self)
    }

    const fn account_filter(self) -> Option<Uuid> {
        self.0.account_filter()
    }
}

/// Proof that the current request has campus-wide HR import visibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HrImportReadScope(());

impl HrImportReadScope {
    /// Refines current request authority for the campus-only `hr.imports` family.
    pub fn from_authority(access: &AccessContext, grants: &RecordScopeGrants) -> Option<Self> {
        if access.has_permission("*") {
            return Some(Self(()));
        }
        let family = RecordScopeFamilyKey::parse("hr.imports").ok()?;
        matches!(
            grants.effective_scope(&family),
            Some(EffectiveRecordScope::Campus)
        )
        .then_some(Self(()))
    }
}

fn employee_visibility(
    access: &AccessContext,
    grants: &RecordScopeGrants,
    actor: AuditActor,
    family: &str,
) -> Option<HrRecordVisibility> {
    if access.has_permission("*") {
        return Some(HrRecordVisibility::Campus);
    }
    let family = RecordScopeFamilyKey::parse(family).ok()?;
    match grants.effective_scope(&family)? {
        EffectiveRecordScope::Campus => Some(HrRecordVisibility::Campus),
        EffectiveRecordScope::SelfRecord => actor.user_id().map(HrRecordVisibility::SelfAccount),
        EffectiveRecordScope::Assigned | EffectiveRecordScope::SelfAndAssigned => None,
    }
}

/// Typed HR boundary for department-based communication audiences.
pub struct CommunicationAudienceOps;

impl CommunicationAudienceOps {
    pub async fn department_references(
        pool: &PgPool,
        tenant_id: Uuid,
    ) -> Result<Vec<CommunicationDepartmentReference>> {
        sqlx::query_as::<_, CommunicationDepartmentReference>(
            r#"
            SELECT id, code, name FROM departments
             WHERE tenant_id = $1 AND status = 'active' AND deleted_at IS NULL
             ORDER BY name, code
            "#,
        )
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .context("Failed to list communication department references")
    }

    pub async fn department_recipient_accounts(
        pool: &PgPool,
        tenant_id: Uuid,
        department_id: Uuid,
    ) -> Result<Vec<CommunicationEmployeeAccountReference>> {
        sqlx::query_as::<_, CommunicationEmployeeAccountReference>(
            r#"
            SELECT DISTINCT employee.account_id
              FROM employees AS employee
              JOIN departments AS department
                ON department.id = employee.department_id
               AND department.tenant_id = employee.tenant_id
               AND department.status = 'active' AND department.deleted_at IS NULL
             WHERE employee.tenant_id = $1 AND employee.department_id = $2
               AND employee.employment_status = 'active'
               AND employee.deleted_at IS NULL AND employee.account_id IS NOT NULL
             ORDER BY employee.account_id
            "#,
        )
        .bind(tenant_id)
        .bind(department_id)
        .fetch_all(pool)
        .await
        .context("Failed to resolve HR communication recipients")
    }
}

pub struct DepartmentOps;

impl DepartmentOps {
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
    ) -> Result<(Vec<Department>, i64)> {
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let departments = sqlx::query_as::<_, Department>(
            r#"
            SELECT id, tenant_id, code, name, status, notes,
                   created_at, updated_at, deleted_at
            FROM departments
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
        .context("Failed to list departments")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM departments
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
        .context("Failed to count departments")?;
        Ok((departments, total))
    }

    pub async fn get_by_id(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<Option<Department>> {
        sqlx::query_as::<_, Department>(
            r#"
            SELECT id, tenant_id, code, name, status, notes,
                   created_at, updated_at, deleted_at
            FROM departments
            WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load department")
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        request: &CreateDepartmentRequest,
    ) -> Result<Department> {
        sqlx::query_as::<_, Department>(
            r#"
            INSERT INTO departments (tenant_id, code, name, status, notes)
            VALUES ($1, $2, $3, COALESCE($4, 'active'), $5)
            RETURNING id, tenant_id, code, name, status, notes,
                      created_at, updated_at, deleted_at
            "#,
        )
        .bind(tenant_id)
        .bind(request.code.trim())
        .bind(request.name.trim())
        .bind(request.status.map(|value| value.as_str()))
        .bind(request.notes.as_deref().map(str::trim))
        .fetch_one(pool)
        .await
        .context("Failed to create department")
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        request: &UpdateDepartmentRequest,
    ) -> Result<Option<Department>> {
        sqlx::query_as::<_, Department>(
            r#"
            UPDATE departments
            SET code = COALESCE($1, code),
                name = COALESCE($2, name),
                status = COALESCE($3, status),
                notes = COALESCE($4, notes),
                updated_at = NOW()
            WHERE tenant_id = $5 AND id = $6 AND deleted_at IS NULL
            RETURNING id, tenant_id, code, name, status, notes,
                      created_at, updated_at, deleted_at
            "#,
        )
        .bind(request.code.as_deref().map(str::trim))
        .bind(request.name.as_deref().map(str::trim))
        .bind(request.status.map(|value| value.as_str()))
        .bind(request.notes.as_deref().map(str::trim))
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to update department")
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<DeleteOutcome> {
        if Self::get_by_id(pool, tenant_id, id).await?.is_none() {
            return Ok(DeleteOutcome::NotFound);
        }
        let in_use = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM employees
                WHERE tenant_id = $1 AND department_id = $2 AND deleted_at IS NULL
                UNION ALL
                SELECT 1 FROM positions
                WHERE tenant_id = $1 AND department_id = $2 AND deleted_at IS NULL
            )
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_one(pool)
        .await
        .context("Failed to check department references")?;
        if in_use {
            return Ok(DeleteOutcome::InUse);
        }
        sqlx::query(
            "UPDATE departments SET deleted_at = NOW() WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete department")?;
        Ok(DeleteOutcome::Deleted)
    }
}

pub struct PositionOps;

impl PositionOps {
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
    ) -> Result<(Vec<Position>, i64)> {
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let positions = sqlx::query_as::<_, Position>(
            r#"
            SELECT id, tenant_id, department_id, code, title, status, notes,
                   created_at, updated_at, deleted_at
            FROM positions
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND ($2::TEXT IS NULL OR code ILIKE $2 OR title ILIKE $2)
              AND ($3::TEXT IS NULL OR status = $3)
            ORDER BY title, code
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
        .context("Failed to list positions")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM positions
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND ($2::TEXT IS NULL OR code ILIKE $2 OR title ILIKE $2)
              AND ($3::TEXT IS NULL OR status = $3)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .fetch_one(pool)
        .await
        .context("Failed to count positions")?;
        Ok((positions, total))
    }

    pub async fn get_by_id(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<Option<Position>> {
        sqlx::query_as::<_, Position>(
            r#"
            SELECT id, tenant_id, department_id, code, title, status, notes,
                   created_at, updated_at, deleted_at
            FROM positions
            WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load position")
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        request: &CreatePositionRequest,
    ) -> Result<Position> {
        ensure_department(pool, tenant_id, request.department_id).await?;
        sqlx::query_as::<_, Position>(
            r#"
            INSERT INTO positions (tenant_id, department_id, code, title, status, notes)
            VALUES ($1, $2, $3, $4, COALESCE($5, 'active'), $6)
            RETURNING id, tenant_id, department_id, code, title, status, notes,
                      created_at, updated_at, deleted_at
            "#,
        )
        .bind(tenant_id)
        .bind(request.department_id)
        .bind(request.code.trim())
        .bind(request.title.trim())
        .bind(request.status.map(|value| value.as_str()))
        .bind(request.notes.as_deref().map(str::trim))
        .fetch_one(pool)
        .await
        .context("Failed to create position")
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        request: &UpdatePositionRequest,
    ) -> Result<Option<Position>> {
        ensure_department(pool, tenant_id, request.department_id).await?;
        sqlx::query_as::<_, Position>(
            r#"
            UPDATE positions
            SET department_id = COALESCE($1, department_id),
                code = COALESCE($2, code),
                title = COALESCE($3, title),
                status = COALESCE($4, status),
                notes = COALESCE($5, notes),
                updated_at = NOW()
            WHERE tenant_id = $6 AND id = $7 AND deleted_at IS NULL
            RETURNING id, tenant_id, department_id, code, title, status, notes,
                      created_at, updated_at, deleted_at
            "#,
        )
        .bind(request.department_id)
        .bind(request.code.as_deref().map(str::trim))
        .bind(request.title.as_deref().map(str::trim))
        .bind(request.status.map(|value| value.as_str()))
        .bind(request.notes.as_deref().map(str::trim))
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to update position")
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<DeleteOutcome> {
        if Self::get_by_id(pool, tenant_id, id).await?.is_none() {
            return Ok(DeleteOutcome::NotFound);
        }
        let in_use = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM employees WHERE tenant_id = $1 AND position_id = $2 AND deleted_at IS NULL)",
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_one(pool)
        .await
        .context("Failed to check position references")?;
        if in_use {
            return Ok(DeleteOutcome::InUse);
        }
        sqlx::query(
            "UPDATE positions SET deleted_at = NOW() WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete position")?;
        Ok(DeleteOutcome::Deleted)
    }
}

pub struct EmployeeOps;

impl EmployeeOps {
    /// Resolves the active employee linked to one authenticated account.
    pub async fn active_reference_by_account(
        pool: &PgPool,
        tenant_id: Uuid,
        account_id: Uuid,
    ) -> Result<Option<EmployeeReference>> {
        sqlx::query_as::<_, EmployeeWithDetails>(
            r#"
            SELECT employee.id, employee.tenant_id, employee.account_id,
                   account.email AS account_email, employee.employee_number,
                   employee.display_name, employee.first_names, employee.surname,
                   employee.work_email, employee.phone, employee.department_id,
                   department.name AS department_name, employee.position_id,
                   position.title AS position_title, employee.employment_status,
                   employee.hire_date, employee.end_date,
                   employee.created_at, employee.updated_at
              FROM employees AS employee
              JOIN users AS account
                ON account.id = employee.account_id
               AND account.tenant_id = employee.tenant_id
               AND account.deleted_at IS NULL AND account.is_active
              LEFT JOIN departments AS department
                ON department.id = employee.department_id
               AND department.tenant_id = employee.tenant_id
               AND department.deleted_at IS NULL
              LEFT JOIN positions AS position
                ON position.id = employee.position_id
               AND position.tenant_id = employee.tenant_id
               AND position.deleted_at IS NULL
             WHERE employee.tenant_id = $1 AND employee.account_id = $2
               AND employee.employment_status = 'active'
               AND employee.deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(account_id)
        .fetch_optional(pool)
        .await
        .context("Failed to resolve the active employee account")
        .map(|employee| employee.map(EmployeeReference::from))
    }

    /// Returns a bounded, minimal workforce directory for typed cross-module use.
    pub async fn list_references(
        pool: &PgPool,
        tenant_id: Uuid,
        search: Option<&str>,
        status: Option<&str>,
        limit: i64,
    ) -> Result<Vec<EmployeeReference>> {
        let search = search.map(|value| format!("%{value}%"));
        let rows = sqlx::query_as::<_, EmployeeWithDetails>(
            r#"
            SELECT employee.id, employee.tenant_id, employee.account_id,
                   account.email AS account_email, employee.employee_number,
                   employee.display_name, employee.first_names, employee.surname,
                   employee.work_email, employee.phone, employee.department_id,
                   department.name AS department_name, employee.position_id,
                   position.title AS position_title, employee.employment_status,
                   employee.hire_date, employee.end_date,
                   employee.created_at, employee.updated_at
            FROM employees AS employee
            LEFT JOIN users AS account
              ON account.id = employee.account_id AND account.tenant_id = employee.tenant_id
            LEFT JOIN departments AS department
              ON department.id = employee.department_id
             AND department.tenant_id = employee.tenant_id
             AND department.deleted_at IS NULL
            LEFT JOIN positions AS position
              ON position.id = employee.position_id
             AND position.tenant_id = employee.tenant_id
             AND position.deleted_at IS NULL
            WHERE employee.tenant_id = $1 AND employee.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR employee.employee_number ILIKE $2
                   OR employee.display_name ILIKE $2 OR employee.work_email ILIKE $2)
              AND ($3::TEXT IS NULL OR employee.employment_status = $3)
            ORDER BY employee.display_name, employee.employee_number
            LIMIT $4
            "#,
        )
        .bind(tenant_id)
        .bind(search)
        .bind(status)
        .bind(limit.clamp(1, 100))
        .fetch_all(pool)
        .await
        .context("Failed to list employee references")?;
        Ok(rows.into_iter().map(EmployeeReference::from).collect())
    }

    pub async fn get_reference(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<EmployeeReference>> {
        Ok(Self::get_by_id(pool, tenant_id, id)
            .await?
            .map(EmployeeReference::from))
    }

    pub async fn references_by_ids(
        pool: &PgPool,
        tenant_id: Uuid,
        ids: &[Uuid],
    ) -> Result<Vec<EmployeeReference>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query_as::<_, EmployeeWithDetails>(
            r#"
            SELECT employee.id, employee.tenant_id, employee.account_id,
                   account.email AS account_email, employee.employee_number,
                   employee.display_name, employee.first_names, employee.surname,
                   employee.work_email, employee.phone, employee.department_id,
                   department.name AS department_name, employee.position_id,
                   position.title AS position_title, employee.employment_status,
                   employee.hire_date, employee.end_date,
                   employee.created_at, employee.updated_at
            FROM employees AS employee
            LEFT JOIN users AS account
              ON account.id = employee.account_id AND account.tenant_id = employee.tenant_id
            LEFT JOIN departments AS department
              ON department.id = employee.department_id
             AND department.tenant_id = employee.tenant_id
             AND department.deleted_at IS NULL
            LEFT JOIN positions AS position
              ON position.id = employee.position_id
             AND position.tenant_id = employee.tenant_id
             AND position.deleted_at IS NULL
            WHERE employee.tenant_id = $1
              AND employee.id = ANY($2)
              AND employee.deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(ids)
        .fetch_all(pool)
        .await
        .context("Failed to load employee references")?;
        Ok(rows.into_iter().map(EmployeeReference::from).collect())
    }

    pub async fn search_reference_ids(
        pool: &PgPool,
        tenant_id: Uuid,
        search: &str,
    ) -> Result<Vec<Uuid>> {
        let pattern = format!("%{}%", search.trim());
        sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id
            FROM employees
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND (employee_number ILIKE $2 OR display_name ILIKE $2 OR work_email ILIKE $2)
            "#,
        )
        .bind(tenant_id)
        .bind(pattern)
        .fetch_all(pool)
        .await
        .context("Failed to search employee references")
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
        department_id: Option<Uuid>,
        position_id: Option<Uuid>,
        account_linked: Option<bool>,
    ) -> Result<(Vec<EmployeeWithDetails>, i64)> {
        Self::list_with_account_scope(
            pool,
            tenant_id,
            page,
            per_page,
            search,
            status,
            department_id,
            position_id,
            account_linked,
            None,
        )
        .await
    }

    /// Lists only employee records visible through the refined request scope.
    #[allow(clippy::too_many_arguments)]
    pub async fn list_for_scope(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: EmployeeReadScope,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
        department_id: Option<Uuid>,
        position_id: Option<Uuid>,
        account_linked: Option<bool>,
    ) -> Result<(Vec<EmployeeWithDetails>, i64)> {
        Self::list_with_account_scope(
            pool,
            tenant_id,
            page,
            per_page,
            search,
            status,
            department_id,
            position_id,
            account_linked,
            scope.account_filter(),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn list_with_account_scope(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
        department_id: Option<Uuid>,
        position_id: Option<Uuid>,
        account_linked: Option<bool>,
        actor_account_id: Option<Uuid>,
    ) -> Result<(Vec<EmployeeWithDetails>, i64)> {
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let employees = sqlx::query_as::<_, EmployeeWithDetails>(
            r#"
            SELECT employee.id, employee.tenant_id, employee.account_id,
                   account.email AS account_email, employee.employee_number,
                   employee.display_name, employee.first_names, employee.surname,
                   employee.work_email, employee.phone, employee.department_id,
                   department.name AS department_name, employee.position_id,
                   position.title AS position_title, employee.employment_status,
                   employee.hire_date, employee.end_date,
                   employee.created_at, employee.updated_at
            FROM employees AS employee
            LEFT JOIN users AS account
              ON account.id = employee.account_id AND account.tenant_id = employee.tenant_id
            LEFT JOIN departments AS department
              ON department.id = employee.department_id
             AND department.tenant_id = employee.tenant_id
             AND department.deleted_at IS NULL
            LEFT JOIN positions AS position
              ON position.id = employee.position_id
             AND position.tenant_id = employee.tenant_id
             AND position.deleted_at IS NULL
            WHERE employee.tenant_id = $1 AND employee.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR employee.employee_number ILIKE $2
                   OR employee.display_name ILIKE $2 OR employee.work_email ILIKE $2)
              AND ($3::TEXT IS NULL OR employee.employment_status = $3)
              AND ($4::UUID IS NULL OR employee.department_id = $4)
              AND ($5::UUID IS NULL OR employee.position_id = $5)
              AND ($6::BOOLEAN IS NULL
                   OR ($6 = TRUE AND employee.account_id IS NOT NULL)
                   OR ($6 = FALSE AND employee.account_id IS NULL))
              AND ($7::UUID IS NULL OR employee.account_id = $7)
            ORDER BY employee.display_name, employee.employee_number
            LIMIT $8 OFFSET $9
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(department_id)
        .bind(position_id)
        .bind(account_linked)
        .bind(actor_account_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list employees")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM employees
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND ($2::TEXT IS NULL OR employee_number ILIKE $2
                   OR display_name ILIKE $2 OR work_email ILIKE $2)
              AND ($3::TEXT IS NULL OR employment_status = $3)
              AND ($4::UUID IS NULL OR department_id = $4)
              AND ($5::UUID IS NULL OR position_id = $5)
              AND ($6::BOOLEAN IS NULL
                   OR ($6 = TRUE AND account_id IS NOT NULL)
                   OR ($6 = FALSE AND account_id IS NULL))
              AND ($7::UUID IS NULL OR account_id = $7)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(department_id)
        .bind(position_id)
        .bind(account_linked)
        .bind(actor_account_id)
        .fetch_one(pool)
        .await
        .context("Failed to count employees")?;
        Ok((employees, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<EmployeeWithDetails>> {
        load_employee(pool, tenant_id, id, None).await
    }

    /// Loads one employee only when it is visible through the refined request scope.
    pub async fn get_by_id_for_scope(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: EmployeeReadScope,
        id: Uuid,
    ) -> Result<Option<EmployeeWithDetails>> {
        load_employee(pool, tenant_id, id, scope.account_filter()).await
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        request: &CreateEmployeeRequest,
    ) -> Result<EmployeeWithDetails> {
        validate_employee_references(
            pool,
            tenant_id,
            request.account_id,
            request.department_id,
            request.position_id,
        )
        .await?;
        validate_employment_dates(request.hire_date, request.end_date)?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO employees (
                tenant_id, account_id, employee_number, display_name, first_names,
                surname, work_email, phone, department_id, position_id,
                employment_status, hire_date, end_date
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                COALESCE($11, 'active'), $12, $13
            )
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(request.account_id)
        .bind(request.employee_number.trim())
        .bind(request.display_name.trim())
        .bind(request.first_names.as_deref().map(str::trim))
        .bind(request.surname.as_deref().map(str::trim))
        .bind(
            request
                .work_email
                .as_deref()
                .map(|value| value.trim().to_lowercase()),
        )
        .bind(request.phone.as_deref().map(str::trim))
        .bind(request.department_id)
        .bind(request.position_id)
        .bind(request.employment_status.map(|value| value.as_str()))
        .bind(request.hire_date)
        .bind(request.end_date)
        .fetch_one(pool)
        .await
        .context("Failed to create employee")?;
        Self::get_by_id(pool, tenant_id, id)
            .await?
            .context("Created employee could not be reloaded")
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        request: &UpdateEmployeeRequest,
    ) -> Result<Option<EmployeeWithDetails>> {
        validate_employee_references(
            pool,
            tenant_id,
            None,
            request.department_id,
            request.position_id,
        )
        .await?;
        let current = match Self::get_by_id(pool, tenant_id, id).await? {
            Some(value) => value,
            None => return Ok(None),
        };
        validate_employment_dates(
            request.hire_date.or(current.hire_date),
            request.end_date.or(current.end_date),
        )?;
        sqlx::query(
            r#"
            UPDATE employees
            SET employee_number = COALESCE($1, employee_number),
                display_name = COALESCE($2, display_name),
                first_names = COALESCE($3, first_names),
                surname = COALESCE($4, surname),
                work_email = COALESCE($5, work_email),
                phone = COALESCE($6, phone),
                department_id = COALESCE($7, department_id),
                position_id = COALESCE($8, position_id),
                employment_status = COALESCE($9, employment_status),
                hire_date = COALESCE($10, hire_date),
                end_date = COALESCE($11, end_date),
                updated_at = NOW()
            WHERE tenant_id = $12 AND id = $13 AND deleted_at IS NULL
            "#,
        )
        .bind(request.employee_number.as_deref().map(str::trim))
        .bind(request.display_name.as_deref().map(str::trim))
        .bind(request.first_names.as_deref().map(str::trim))
        .bind(request.surname.as_deref().map(str::trim))
        .bind(
            request
                .work_email
                .as_deref()
                .map(|value| value.trim().to_lowercase()),
        )
        .bind(request.phone.as_deref().map(str::trim))
        .bind(request.department_id)
        .bind(request.position_id)
        .bind(request.employment_status.map(|value| value.as_str()))
        .bind(request.hire_date)
        .bind(request.end_date)
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to update employee")?;
        Self::get_by_id(pool, tenant_id, id).await
    }

    pub async fn link_account(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        account_id: Option<Uuid>,
    ) -> Result<Option<EmployeeWithDetails>> {
        validate_employee_references(pool, tenant_id, account_id, None, None).await?;
        let updated = sqlx::query(
            r#"
            UPDATE employees
            SET account_id = $1, updated_at = NOW()
            WHERE tenant_id = $2 AND id = $3 AND deleted_at IS NULL
            "#,
        )
        .bind(account_id)
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to update employee account link")?;
        if updated.rows_affected() == 0 {
            return Ok(None);
        }
        Self::get_by_id(pool, tenant_id, id).await
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<DeleteOutcome> {
        if Self::get_by_id(pool, tenant_id, id).await?.is_none() {
            return Ok(DeleteOutcome::NotFound);
        }
        let in_use = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM drivers
                WHERE tenant_id = $1 AND employee_id = $2 AND deleted_at IS NULL
                UNION ALL
                SELECT 1 FROM teacher_profiles
                WHERE tenant_id = $1 AND employee_id = $2 AND deleted_at IS NULL
                UNION ALL
                SELECT 1 FROM employment_engagements
                WHERE tenant_id = $1 AND employee_id = $2 AND deleted_at IS NULL
                UNION ALL
                SELECT 1 FROM employee_availability_periods
                WHERE tenant_id = $1 AND employee_id = $2 AND deleted_at IS NULL
            )
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_one(pool)
        .await
        .context("Failed to check employee references")?;
        if in_use {
            return Ok(DeleteOutcome::InUse);
        }
        sqlx::query(
            "UPDATE employees SET deleted_at = NOW(), account_id = NULL WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete employee")?;
        Ok(DeleteOutcome::Deleted)
    }
}

/// Typed HR boundary for Assets-owned department stock requests.
///
/// HR supplies current identity and department membership only; request state,
/// approval, stock, and fulfilment remain Assets-owned.
pub struct StockRequestReferenceOps;

impl StockRequestReferenceOps {
    pub async fn requester_candidates(
        pool: &PgPool,
        tenant_id: Uuid,
        search: Option<&str>,
        department_id: Option<Uuid>,
        limit: i64,
    ) -> Result<Vec<StockRequestEmployeeReference>> {
        let search = search
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| format!("%{value}%"));
        sqlx::query_as::<_, StockRequestEmployeeReference>(
            r#"
            SELECT employee.id, employee.account_id, employee.employee_number,
                   employee.display_name, department.id AS department_id,
                   department.code AS department_code, department.name AS department_name
              FROM employees AS employee
              JOIN departments AS department
                ON department.id = employee.department_id
               AND department.tenant_id = employee.tenant_id
               AND department.status = 'active' AND department.deleted_at IS NULL
             WHERE employee.tenant_id = $1 AND employee.employment_status = 'active'
               AND employee.deleted_at IS NULL
               AND ($2::TEXT IS NULL OR employee.employee_number ILIKE $2
                    OR employee.display_name ILIKE $2)
               AND ($3::UUID IS NULL OR employee.department_id = $3)
             ORDER BY employee.display_name, employee.employee_number
             LIMIT $4
            "#,
        )
        .bind(tenant_id)
        .bind(search)
        .bind(department_id)
        .bind(limit.clamp(1, 100))
        .fetch_all(pool)
        .await
        .context("Failed to list stock request employee references")
    }

    pub async fn department_candidates(
        pool: &PgPool,
        tenant_id: Uuid,
        search: Option<&str>,
        limit: i64,
    ) -> Result<Vec<StockRequestDepartmentReference>> {
        let search = search
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| format!("%{value}%"));
        sqlx::query_as::<_, StockRequestDepartmentReference>(
            r#"
            SELECT id, code, name
              FROM departments
             WHERE tenant_id = $1 AND status = 'active' AND deleted_at IS NULL
               AND ($2::TEXT IS NULL OR code ILIKE $2 OR name ILIKE $2)
             ORDER BY name, code
             LIMIT $3
            "#,
        )
        .bind(tenant_id)
        .bind(search)
        .bind(limit.clamp(1, 100))
        .fetch_all(pool)
        .await
        .context("Failed to list stock request department references")
    }

    /// Locks and proves that the requester is active in the selected active
    /// department for the duration of the caller's transaction.
    pub async fn lock_active_requester_department(
        transaction: &mut Transaction<'_, Postgres>,
        tenant_id: Uuid,
        employee_id: Uuid,
        department_id: Uuid,
    ) -> Result<StockRequestEmployeeReference> {
        sqlx::query_as::<_, StockRequestEmployeeReference>(
            r#"
            SELECT employee.id, employee.account_id, employee.employee_number,
                   employee.display_name, department.id AS department_id,
                   department.code AS department_code, department.name AS department_name
              FROM employees AS employee
              JOIN departments AS department
                ON department.id = employee.department_id
               AND department.tenant_id = employee.tenant_id
             WHERE employee.tenant_id = $1 AND employee.id = $2
               AND employee.department_id = $3
               AND employee.employment_status = 'active'
               AND employee.deleted_at IS NULL
               AND department.status = 'active' AND department.deleted_at IS NULL
             FOR SHARE OF employee, department
            "#,
        )
        .bind(tenant_id)
        .bind(employee_id)
        .bind(department_id)
        .fetch_optional(&mut **transaction)
        .await
        .context("Failed to validate stock request HR ownership")?
        .ok_or_else(|| {
            anyhow!("Stock requester must be active in the selected active HR department")
        })
    }

    /// Locks the current HR identity for actor-separation checks without
    /// requiring the historical requester to remain actively employed.
    pub async fn lock_requester_identity(
        transaction: &mut Transaction<'_, Postgres>,
        tenant_id: Uuid,
        employee_id: Uuid,
    ) -> Result<StockRequestEmployeeReference> {
        sqlx::query_as::<_, StockRequestEmployeeReference>(
            r#"
            SELECT employee.id, employee.account_id, employee.employee_number,
                   employee.display_name, department.id AS department_id,
                   department.code AS department_code, department.name AS department_name
              FROM employees AS employee
              JOIN departments AS department
                ON department.id = employee.department_id
               AND department.tenant_id = employee.tenant_id
             WHERE employee.tenant_id = $1 AND employee.id = $2
             FOR SHARE OF employee, department
            "#,
        )
        .bind(tenant_id)
        .bind(employee_id)
        .fetch_optional(&mut **transaction)
        .await
        .context("Failed to lock stock request employee identity")?
        .ok_or_else(|| anyhow!("Stock requester HR identity is no longer available"))
    }

    pub async fn employee_references_by_ids(
        pool: &PgPool,
        tenant_id: Uuid,
        employee_ids: &[Uuid],
    ) -> Result<Vec<StockRequestEmployeeReference>> {
        if employee_ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as::<_, StockRequestEmployeeReference>(
            r#"
            SELECT employee.id, employee.account_id, employee.employee_number,
                   employee.display_name, department.id AS department_id,
                   department.code AS department_code, department.name AS department_name
              FROM employees AS employee
              JOIN departments AS department
                ON department.id = employee.department_id
               AND department.tenant_id = employee.tenant_id
             WHERE employee.tenant_id = $1 AND employee.id = ANY($2)
            "#,
        )
        .bind(tenant_id)
        .bind(employee_ids)
        .fetch_all(pool)
        .await
        .context("Failed to rehydrate stock request employee references")
    }

    pub async fn department_references_by_ids(
        pool: &PgPool,
        tenant_id: Uuid,
        department_ids: &[Uuid],
    ) -> Result<Vec<StockRequestDepartmentReference>> {
        if department_ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as::<_, StockRequestDepartmentReference>(
            "SELECT id, code, name FROM departments WHERE tenant_id = $1 AND id = ANY($2)",
        )
        .bind(tenant_id)
        .bind(department_ids)
        .fetch_all(pool)
        .await
        .context("Failed to rehydrate stock request department references")
    }
}

pub struct EmploymentEngagementOps;

impl EmploymentEngagementOps {
    #[allow(clippy::too_many_arguments)]
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        employee_id: Option<Uuid>,
        status: Option<&str>,
        employment_type: Option<&str>,
    ) -> Result<(Vec<EmploymentEngagementWithDetails>, i64)> {
        Self::list_with_account_scope(
            pool,
            tenant_id,
            page,
            per_page,
            search,
            employee_id,
            status,
            employment_type,
            None,
        )
        .await
    }

    /// Lists only engagements visible through the refined request scope.
    #[allow(clippy::too_many_arguments)]
    pub async fn list_for_scope(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: EmploymentEngagementReadScope,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        employee_id: Option<Uuid>,
        status: Option<&str>,
        employment_type: Option<&str>,
    ) -> Result<(Vec<EmploymentEngagementWithDetails>, i64)> {
        Self::list_with_account_scope(
            pool,
            tenant_id,
            page,
            per_page,
            search,
            employee_id,
            status,
            employment_type,
            scope.account_filter(),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn list_with_account_scope(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        employee_id: Option<Uuid>,
        status: Option<&str>,
        employment_type: Option<&str>,
        actor_account_id: Option<Uuid>,
    ) -> Result<(Vec<EmploymentEngagementWithDetails>, i64)> {
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, EmploymentEngagementWithDetails>(
            r#"
            SELECT engagement.id, engagement.tenant_id, engagement.employee_id,
                   employee.employee_number, employee.display_name AS employee_name,
                   engagement.reference, engagement.employment_type,
                   engagement.department_id, department.name AS department_name,
                   engagement.position_id, position.title AS position_title,
                   engagement.status, engagement.start_date, engagement.end_date,
                   engagement.workload_basis_points, engagement.notes,
                   engagement.created_at, engagement.updated_at
            FROM employment_engagements AS engagement
            JOIN employees AS employee
              ON employee.id = engagement.employee_id
             AND employee.tenant_id = engagement.tenant_id
             AND employee.deleted_at IS NULL
            LEFT JOIN departments AS department
              ON department.id = engagement.department_id
             AND department.tenant_id = engagement.tenant_id
             AND department.deleted_at IS NULL
            LEFT JOIN positions AS position
              ON position.id = engagement.position_id
             AND position.tenant_id = engagement.tenant_id
             AND position.deleted_at IS NULL
            WHERE engagement.tenant_id = $1 AND engagement.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR engagement.reference ILIKE $2
                   OR employee.employee_number ILIKE $2 OR employee.display_name ILIKE $2)
              AND ($3::UUID IS NULL OR engagement.employee_id = $3)
              AND ($4::TEXT IS NULL OR engagement.status = $4)
              AND ($5::TEXT IS NULL OR engagement.employment_type = $5)
              AND ($6::UUID IS NULL OR employee.account_id = $6)
            ORDER BY engagement.start_date DESC NULLS LAST, employee.display_name, engagement.created_at DESC
            LIMIT $7 OFFSET $8
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(employee_id)
        .bind(status)
        .bind(employment_type)
        .bind(actor_account_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list employment engagements")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM employment_engagements AS engagement
            JOIN employees AS employee
              ON employee.id = engagement.employee_id
             AND employee.tenant_id = engagement.tenant_id
             AND employee.deleted_at IS NULL
            WHERE engagement.tenant_id = $1 AND engagement.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR engagement.reference ILIKE $2
                   OR employee.employee_number ILIKE $2 OR employee.display_name ILIKE $2)
              AND ($3::UUID IS NULL OR engagement.employee_id = $3)
              AND ($4::TEXT IS NULL OR engagement.status = $4)
              AND ($5::TEXT IS NULL OR engagement.employment_type = $5)
              AND ($6::UUID IS NULL OR employee.account_id = $6)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(employee_id)
        .bind(status)
        .bind(employment_type)
        .bind(actor_account_id)
        .fetch_one(pool)
        .await
        .context("Failed to count employment engagements")?;
        Ok((rows, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<EmploymentEngagementWithDetails>> {
        load_engagement(pool, tenant_id, id, None).await
    }

    /// Loads one engagement only when it is visible through the refined request scope.
    pub async fn get_by_id_for_scope(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: EmploymentEngagementReadScope,
        id: Uuid,
    ) -> Result<Option<EmploymentEngagementWithDetails>> {
        load_engagement(pool, tenant_id, id, scope.account_filter()).await
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        request: &CreateEmploymentEngagementRequest,
    ) -> Result<EmploymentEngagementWithDetails> {
        ensure_employee(pool, tenant_id, request.employee_id).await?;
        validate_employee_references(
            pool,
            tenant_id,
            None,
            request.department_id,
            request.position_id,
        )
        .await?;
        let status = request.status.map_or("draft", |value| value.as_str());
        if !matches!(status, "draft" | "active") {
            bail!("A new employment engagement must be draft or active");
        }
        validate_engagement(
            request.employment_type.as_str(),
            status,
            Some(request.start_date),
            request.end_date,
        )?;

        let mut transaction = pool
            .begin()
            .await
            .context("Failed to begin employment change")?;
        lock_employee(&mut transaction, tenant_id, request.employee_id).await?;
        if status == "active" {
            ensure_no_other_active_engagement(
                &mut transaction,
                tenant_id,
                request.employee_id,
                None,
            )
            .await?;
        }
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO employment_engagements (
                tenant_id, employee_id, reference, employment_type,
                department_id, position_id, status, start_date, end_date,
                workload_basis_points, notes
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(request.employee_id)
        .bind(trimmed_owned(request.reference.as_deref()))
        .bind(request.employment_type.as_str())
        .bind(request.department_id)
        .bind(request.position_id)
        .bind(status)
        .bind(request.start_date)
        .bind(request.end_date)
        .bind(request.workload_basis_points.unwrap_or(10_000))
        .bind(trimmed_owned(request.notes.as_deref()))
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to create employment engagement")?;
        if status == "active" {
            sync_employee_projection(
                &mut transaction,
                tenant_id,
                request.employee_id,
                request.department_id,
                request.position_id,
                Some(request.start_date),
                request.end_date,
                "active",
            )
            .await?;
        }
        transaction
            .commit()
            .await
            .context("Failed to commit employment engagement")?;
        Self::get_by_id(pool, tenant_id, id)
            .await?
            .context("Created employment engagement could not be reloaded")
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        request: &UpdateEmploymentEngagementRequest,
    ) -> Result<Option<EmploymentEngagementWithDetails>> {
        validate_employee_references(
            pool,
            tenant_id,
            None,
            request.department_id,
            request.position_id,
        )
        .await?;
        validate_engagement(
            request.employment_type.as_str(),
            request.status.as_str(),
            Some(request.start_date),
            request.end_date,
        )?;

        let mut transaction = pool
            .begin()
            .await
            .context("Failed to begin employment change")?;
        let Some((employee_id, current_status)) = sqlx::query_as::<_, (Uuid, String)>(
            r#"
            SELECT employee_id, status
            FROM employment_engagements
            WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            FOR UPDATE
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to load employment engagement")?
        else {
            return Ok(None);
        };
        lock_employee(&mut transaction, tenant_id, employee_id).await?;
        if matches!(current_status.as_str(), "ended" | "cancelled") {
            bail!(
                "Ended and cancelled employment engagements are historical records and cannot be edited"
            );
        }
        validate_engagement_transition(&current_status, request.status.as_str())?;
        if request.status.as_str() == "active" {
            ensure_no_other_active_engagement(&mut transaction, tenant_id, employee_id, Some(id))
                .await?;
        }

        sqlx::query(
            r#"
            UPDATE employment_engagements
            SET reference = $1, employment_type = $2, department_id = $3,
                position_id = $4, status = $5, start_date = $6, end_date = $7,
                workload_basis_points = $8, notes = $9, updated_at = NOW()
            WHERE tenant_id = $10 AND id = $11 AND deleted_at IS NULL
            "#,
        )
        .bind(trimmed_owned(request.reference.as_deref()))
        .bind(request.employment_type.as_str())
        .bind(request.department_id)
        .bind(request.position_id)
        .bind(request.status.as_str())
        .bind(request.start_date)
        .bind(request.end_date)
        .bind(request.workload_basis_points)
        .bind(trimmed_owned(request.notes.as_deref()))
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to update employment engagement")?;

        if request.status.as_str() == "active" {
            sync_employee_projection(
                &mut transaction,
                tenant_id,
                employee_id,
                request.department_id,
                request.position_id,
                Some(request.start_date),
                request.end_date,
                "active",
            )
            .await?;
        } else if current_status == "active" && request.status.as_str() == "ended" {
            sync_employee_projection(
                &mut transaction,
                tenant_id,
                employee_id,
                request.department_id,
                request.position_id,
                Some(request.start_date),
                request.end_date,
                "terminated",
            )
            .await?;
        }
        transaction
            .commit()
            .await
            .context("Failed to commit employment engagement")?;
        Self::get_by_id(pool, tenant_id, id).await
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<DeleteOutcome> {
        let status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM employment_engagements WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load employment engagement")?;
        let Some(status) = status else {
            return Ok(DeleteOutcome::NotFound);
        };
        if status != "draft" {
            return Ok(DeleteOutcome::InUse);
        }
        sqlx::query(
            "UPDATE employment_engagements SET deleted_at = NOW() WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete employment engagement")?;
        Ok(DeleteOutcome::Deleted)
    }
}

pub struct EmployeeAvailabilityOps;

impl EmployeeAvailabilityOps {
    /// Returns approved scheduling constraints for typed cross-module use.
    /// Notes and decision metadata are deliberately excluded.
    pub async fn list_approved_for_window(
        pool: &PgPool,
        tenant_id: Uuid,
        employee_ids: &[Uuid],
        starts_at: DateTime<Utc>,
        ends_at: DateTime<Utc>,
    ) -> Result<Vec<EmployeeAvailabilityReference>> {
        validate_availability_times(starts_at, ends_at)?;
        if employee_ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as::<_, EmployeeAvailabilityReference>(
            r#"
            SELECT id, employee_id, kind, starts_at, ends_at
            FROM employee_availability_periods
            WHERE tenant_id = $1 AND employee_id = ANY($2)
              AND status = 'approved' AND deleted_at IS NULL
              AND starts_at < $4 AND ends_at > $3
            ORDER BY employee_id, starts_at, id
            "#,
        )
        .bind(tenant_id)
        .bind(employee_ids)
        .bind(starts_at)
        .bind(ends_at)
        .fetch_all(pool)
        .await
        .context("Failed to load approved employee availability")
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        employee_id: Option<Uuid>,
        status: Option<&str>,
        kind: Option<&str>,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
    ) -> Result<(Vec<EmployeeAvailabilityWithDetails>, i64)> {
        Self::list_with_account_scope(
            pool,
            tenant_id,
            page,
            per_page,
            search,
            employee_id,
            status,
            kind,
            from,
            to,
            None,
        )
        .await
    }

    /// Lists only availability periods visible through the refined request scope.
    #[allow(clippy::too_many_arguments)]
    pub async fn list_for_scope(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: EmployeeAvailabilityReadScope,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        employee_id: Option<Uuid>,
        status: Option<&str>,
        kind: Option<&str>,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
    ) -> Result<(Vec<EmployeeAvailabilityWithDetails>, i64)> {
        Self::list_with_account_scope(
            pool,
            tenant_id,
            page,
            per_page,
            search,
            employee_id,
            status,
            kind,
            from,
            to,
            scope.account_filter(),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn list_with_account_scope(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        employee_id: Option<Uuid>,
        status: Option<&str>,
        kind: Option<&str>,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
        actor_account_id: Option<Uuid>,
    ) -> Result<(Vec<EmployeeAvailabilityWithDetails>, i64)> {
        if let (Some(from), Some(to)) = (from, to)
            && to < from
        {
            bail!("Availability range end cannot be before its start");
        }
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, EmployeeAvailabilityWithDetails>(
            r#"
            SELECT availability.id, availability.tenant_id, availability.employee_id,
                   employee.employee_number, employee.display_name AS employee_name,
                   availability.kind, availability.starts_at, availability.ends_at,
                   availability.status, availability.notes, availability.decided_by,
                   decision_user.full_name AS decided_by_name, availability.decided_at,
                   availability.created_at, availability.updated_at
            FROM employee_availability_periods AS availability
            JOIN employees AS employee
              ON employee.id = availability.employee_id
             AND employee.tenant_id = availability.tenant_id
             AND employee.deleted_at IS NULL
            LEFT JOIN users AS decision_user
              ON decision_user.id = availability.decided_by
             AND decision_user.tenant_id = availability.tenant_id
            WHERE availability.tenant_id = $1 AND availability.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR employee.employee_number ILIKE $2
                   OR employee.display_name ILIKE $2)
              AND ($3::UUID IS NULL OR availability.employee_id = $3)
              AND ($4::TEXT IS NULL OR availability.status = $4)
              AND ($5::TEXT IS NULL OR availability.kind = $5)
              AND ($6::TIMESTAMPTZ IS NULL OR availability.ends_at >= $6)
              AND ($7::TIMESTAMPTZ IS NULL OR availability.starts_at <= $7)
              AND ($8::UUID IS NULL OR employee.account_id = $8)
            ORDER BY availability.starts_at DESC, employee.display_name
            LIMIT $9 OFFSET $10
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(employee_id)
        .bind(status)
        .bind(kind)
        .bind(from)
        .bind(to)
        .bind(actor_account_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list employee availability")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM employee_availability_periods AS availability
            JOIN employees AS employee
              ON employee.id = availability.employee_id
             AND employee.tenant_id = availability.tenant_id
             AND employee.deleted_at IS NULL
            WHERE availability.tenant_id = $1 AND availability.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR employee.employee_number ILIKE $2
                   OR employee.display_name ILIKE $2)
              AND ($3::UUID IS NULL OR availability.employee_id = $3)
              AND ($4::TEXT IS NULL OR availability.status = $4)
              AND ($5::TEXT IS NULL OR availability.kind = $5)
              AND ($6::TIMESTAMPTZ IS NULL OR availability.ends_at >= $6)
              AND ($7::TIMESTAMPTZ IS NULL OR availability.starts_at <= $7)
              AND ($8::UUID IS NULL OR employee.account_id = $8)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(employee_id)
        .bind(status)
        .bind(kind)
        .bind(from)
        .bind(to)
        .bind(actor_account_id)
        .fetch_one(pool)
        .await
        .context("Failed to count employee availability")?;
        Ok((rows, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<EmployeeAvailabilityWithDetails>> {
        load_availability(pool, tenant_id, id, None).await
    }

    /// Loads one availability period only when visible through the refined request scope.
    pub async fn get_by_id_for_scope(
        pool: &PgPool,
        tenant_id: Uuid,
        scope: EmployeeAvailabilityReadScope,
        id: Uuid,
    ) -> Result<Option<EmployeeAvailabilityWithDetails>> {
        load_availability(pool, tenant_id, id, scope.account_filter()).await
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        actor_user_id: Uuid,
        request: &CreateEmployeeAvailabilityRequest,
    ) -> Result<EmployeeAvailabilityWithDetails> {
        ensure_employee(pool, tenant_id, request.employee_id).await?;
        validate_availability_times(request.starts_at, request.ends_at)?;
        let status = request.status.map_or("draft", |value| value.as_str());
        if !matches!(status, "draft" | "submitted") {
            bail!("A new availability period must be draft or submitted");
        }
        ensure_actor(pool, tenant_id, actor_user_id).await?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO employee_availability_periods (
                tenant_id, employee_id, kind, starts_at, ends_at, status, notes
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(request.employee_id)
        .bind(request.kind.as_str())
        .bind(request.starts_at)
        .bind(request.ends_at)
        .bind(status)
        .bind(trimmed_owned(request.notes.as_deref()))
        .fetch_one(pool)
        .await
        .context("Failed to create employee availability")?;
        Self::get_by_id(pool, tenant_id, id)
            .await?
            .context("Created availability period could not be reloaded")
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        actor_user_id: Uuid,
        id: Uuid,
        request: &UpdateEmployeeAvailabilityRequest,
    ) -> Result<Option<EmployeeAvailabilityWithDetails>> {
        validate_availability_times(request.starts_at, request.ends_at)?;
        ensure_actor(pool, tenant_id, actor_user_id).await?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to begin availability change")?;
        let Some((employee_id, current_kind, current_starts_at, current_ends_at, current_status)) =
            sqlx::query_as::<_, (Uuid, String, DateTime<Utc>, DateTime<Utc>, String)>(
                r#"
                SELECT employee_id, kind, starts_at, ends_at, status
                FROM employee_availability_periods
                WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
                FOR UPDATE
                "#,
            )
            .bind(tenant_id)
            .bind(id)
            .fetch_optional(&mut *transaction)
            .await
            .context("Failed to load employee availability")?
        else {
            return Ok(None);
        };
        lock_employee(&mut transaction, tenant_id, employee_id).await?;
        if matches!(current_status.as_str(), "rejected" | "cancelled") {
            bail!(
                "Rejected and cancelled availability periods are historical decisions and cannot be edited"
            );
        }
        if current_status == "approved" && request.status.as_str() == "approved" {
            bail!("Approved availability is immutable; cancel it to record a replacement");
        }
        validate_availability_transition(&current_status, request.status.as_str())?;
        if current_status == "approved"
            && (request.kind.as_str() != current_kind
                || request.starts_at != current_starts_at
                || request.ends_at != current_ends_at)
        {
            bail!(
                "Approved availability dates and type cannot be rewritten; cancel and create a replacement"
            );
        }
        if request.status.as_str() == "approved" {
            ensure_no_approved_availability_overlap(
                &mut transaction,
                tenant_id,
                employee_id,
                id,
                request.starts_at,
                request.ends_at,
            )
            .await?;
        }
        let decision = matches!(request.status.as_str(), "approved" | "rejected")
            && !matches!(current_status.as_str(), "approved" | "rejected");
        sqlx::query(
            r#"
            UPDATE employee_availability_periods
            SET kind = $1, starts_at = $2, ends_at = $3, status = $4,
                notes = CASE WHEN status = 'approved' THEN notes ELSE $5 END,
                decided_by = CASE WHEN $6 THEN $7 ELSE decided_by END,
                decided_at = CASE WHEN $6 THEN NOW() ELSE decided_at END,
                updated_at = NOW()
            WHERE tenant_id = $8 AND id = $9 AND deleted_at IS NULL
            "#,
        )
        .bind(request.kind.as_str())
        .bind(request.starts_at)
        .bind(request.ends_at)
        .bind(request.status.as_str())
        .bind(trimmed_owned(request.notes.as_deref()))
        .bind(decision)
        .bind(actor_user_id)
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to update employee availability")?;
        transaction
            .commit()
            .await
            .context("Failed to commit employee availability")?;
        Self::get_by_id(pool, tenant_id, id).await
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<DeleteOutcome> {
        let status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM employee_availability_periods WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load employee availability")?;
        let Some(status) = status else {
            return Ok(DeleteOutcome::NotFound);
        };
        if status != "draft" {
            return Ok(DeleteOutcome::InUse);
        }
        sqlx::query(
            "UPDATE employee_availability_periods SET deleted_at = NOW() WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete employee availability")?;
        Ok(DeleteOutcome::Deleted)
    }
}

async fn load_employee(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    actor_account_id: Option<Uuid>,
) -> Result<Option<EmployeeWithDetails>> {
    sqlx::query_as::<_, EmployeeWithDetails>(
        r#"
        SELECT employee.id, employee.tenant_id, employee.account_id,
               account.email AS account_email, employee.employee_number,
               employee.display_name, employee.first_names, employee.surname,
               employee.work_email, employee.phone, employee.department_id,
               department.name AS department_name, employee.position_id,
               position.title AS position_title, employee.employment_status,
               employee.hire_date, employee.end_date,
               employee.created_at, employee.updated_at
        FROM employees AS employee
        LEFT JOIN users AS account
          ON account.id = employee.account_id AND account.tenant_id = employee.tenant_id
        LEFT JOIN departments AS department
          ON department.id = employee.department_id
         AND department.tenant_id = employee.tenant_id
         AND department.deleted_at IS NULL
        LEFT JOIN positions AS position
          ON position.id = employee.position_id
         AND position.tenant_id = employee.tenant_id
         AND position.deleted_at IS NULL
        WHERE employee.tenant_id = $1 AND employee.id = $2 AND employee.deleted_at IS NULL
          AND ($3::UUID IS NULL OR employee.account_id = $3)
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .bind(actor_account_id)
    .fetch_optional(pool)
    .await
    .context("Failed to load employee")
}

async fn load_engagement(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    actor_account_id: Option<Uuid>,
) -> Result<Option<EmploymentEngagementWithDetails>> {
    sqlx::query_as::<_, EmploymentEngagementWithDetails>(
        r#"
        SELECT engagement.id, engagement.tenant_id, engagement.employee_id,
               employee.employee_number, employee.display_name AS employee_name,
               engagement.reference, engagement.employment_type,
               engagement.department_id, department.name AS department_name,
               engagement.position_id, position.title AS position_title,
               engagement.status, engagement.start_date, engagement.end_date,
               engagement.workload_basis_points, engagement.notes,
               engagement.created_at, engagement.updated_at
        FROM employment_engagements AS engagement
        JOIN employees AS employee
          ON employee.id = engagement.employee_id
         AND employee.tenant_id = engagement.tenant_id
         AND employee.deleted_at IS NULL
        LEFT JOIN departments AS department
          ON department.id = engagement.department_id
         AND department.tenant_id = engagement.tenant_id
         AND department.deleted_at IS NULL
        LEFT JOIN positions AS position
          ON position.id = engagement.position_id
         AND position.tenant_id = engagement.tenant_id
         AND position.deleted_at IS NULL
        WHERE engagement.tenant_id = $1 AND engagement.id = $2
          AND engagement.deleted_at IS NULL
          AND ($3::UUID IS NULL OR employee.account_id = $3)
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .bind(actor_account_id)
    .fetch_optional(pool)
    .await
    .context("Failed to load employment engagement")
}

async fn load_availability(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    actor_account_id: Option<Uuid>,
) -> Result<Option<EmployeeAvailabilityWithDetails>> {
    sqlx::query_as::<_, EmployeeAvailabilityWithDetails>(
        r#"
        SELECT availability.id, availability.tenant_id, availability.employee_id,
               employee.employee_number, employee.display_name AS employee_name,
               availability.kind, availability.starts_at, availability.ends_at,
               availability.status, availability.notes, availability.decided_by,
               decision_user.full_name AS decided_by_name, availability.decided_at,
               availability.created_at, availability.updated_at
        FROM employee_availability_periods AS availability
        JOIN employees AS employee
          ON employee.id = availability.employee_id
         AND employee.tenant_id = availability.tenant_id
         AND employee.deleted_at IS NULL
        LEFT JOIN users AS decision_user
          ON decision_user.id = availability.decided_by
         AND decision_user.tenant_id = availability.tenant_id
        WHERE availability.tenant_id = $1 AND availability.id = $2
          AND availability.deleted_at IS NULL
          AND ($3::UUID IS NULL OR employee.account_id = $3)
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .bind(actor_account_id)
    .fetch_optional(pool)
    .await
    .context("Failed to load employee availability")
}

async fn lock_employee(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    employee_id: Uuid,
) -> Result<()> {
    let exists = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM employees WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(employee_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock employee")?;
    if exists.is_none() {
        bail!("Employee was not found for this campus");
    }
    Ok(())
}

async fn ensure_no_other_active_engagement(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    employee_id: Uuid,
    excluding_id: Option<Uuid>,
) -> Result<()> {
    let exists = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM employment_engagements
            WHERE tenant_id = $1 AND employee_id = $2 AND status = 'active'
              AND deleted_at IS NULL AND ($3::UUID IS NULL OR id <> $3)
        )
        "#,
    )
    .bind(tenant_id)
    .bind(employee_id)
    .bind(excluding_id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to check active employment")?;
    if exists {
        bail!("Employee already has an active employment engagement");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn sync_employee_projection(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    employee_id: Uuid,
    department_id: Option<Uuid>,
    position_id: Option<Uuid>,
    hire_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
    employment_status: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE employees
        SET department_id = $1, position_id = $2, hire_date = $3, end_date = $4,
            employment_status = $5, updated_at = NOW()
        WHERE tenant_id = $6 AND id = $7 AND deleted_at IS NULL
        "#,
    )
    .bind(department_id)
    .bind(position_id)
    .bind(hire_date)
    .bind(end_date)
    .bind(employment_status)
    .bind(tenant_id)
    .bind(employee_id)
    .execute(&mut **transaction)
    .await
    .context("Failed to update the employee employment projection")?;
    Ok(())
}

async fn ensure_no_approved_availability_overlap(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    employee_id: Uuid,
    id: Uuid,
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
) -> Result<()> {
    let exists = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM employee_availability_periods
            WHERE tenant_id = $1 AND employee_id = $2 AND id <> $3
              AND status = 'approved' AND deleted_at IS NULL
              AND starts_at < $5 AND ends_at > $4
        )
        "#,
    )
    .bind(tenant_id)
    .bind(employee_id)
    .bind(id)
    .bind(starts_at)
    .bind(ends_at)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to check approved availability")?;
    if exists {
        bail!("Approved availability periods cannot overlap for the same employee");
    }
    Ok(())
}

async fn ensure_employee(pool: &PgPool, tenant_id: Uuid, employee_id: Uuid) -> Result<()> {
    if EmployeeOps::get_by_id(pool, tenant_id, employee_id)
        .await?
        .is_none()
    {
        bail!("Employee was not found for this campus");
    }
    Ok(())
}

async fn ensure_actor(pool: &PgPool, tenant_id: Uuid, actor_user_id: Uuid) -> Result<()> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM users WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
    )
    .bind(tenant_id)
    .bind(actor_user_id)
    .fetch_one(pool)
    .await
    .context("Failed to validate the availability decision actor")?;
    if !exists {
        bail!("Availability decision actor was not found for this campus");
    }
    Ok(())
}

fn validate_engagement(
    employment_type: &str,
    status: &str,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
) -> Result<()> {
    if let (Some(start_date), Some(end_date)) = (start_date, end_date)
        && end_date < start_date
    {
        bail!("Employment end date cannot be before the start date");
    }
    if employment_type == "fixed_term" && end_date.is_none() {
        bail!("Fixed-term employment requires an end date");
    }
    if status == "ended" && end_date.is_none() {
        bail!("Ended employment requires an end date");
    }
    if matches!(status, "active" | "ended") && start_date.is_none() {
        bail!("Active and ended employment requires a start date");
    }
    Ok(())
}

fn validate_engagement_transition(current: &str, next: &str) -> Result<()> {
    let allowed = matches!(
        (current, next),
        ("draft", "draft" | "active" | "cancelled")
            | ("active", "active" | "ended")
            | ("ended", "ended")
            | ("cancelled", "cancelled")
    );
    if !allowed {
        bail!("Employment engagement cannot move from {current} to {next}");
    }
    Ok(())
}

fn validate_availability_times(starts_at: DateTime<Utc>, ends_at: DateTime<Utc>) -> Result<()> {
    if ends_at <= starts_at {
        bail!("Availability end must be after its start");
    }
    Ok(())
}

fn validate_availability_transition(current: &str, next: &str) -> Result<()> {
    let allowed = matches!(
        (current, next),
        ("draft", "draft" | "submitted" | "cancelled")
            | (
                "submitted",
                "submitted" | "approved" | "rejected" | "cancelled"
            )
            | ("approved", "approved" | "cancelled")
            | ("rejected", "rejected")
            | ("cancelled", "cancelled")
    );
    if !allowed {
        bail!("Availability period cannot move from {current} to {next}");
    }
    Ok(())
}

fn trimmed_owned(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

async fn ensure_department(pool: &PgPool, tenant_id: Uuid, id: Option<Uuid>) -> Result<()> {
    let Some(id) = id else {
        return Ok(());
    };
    if DepartmentOps::get_by_id(pool, tenant_id, id)
        .await?
        .is_none()
    {
        bail!("Department was not found for this campus");
    }
    Ok(())
}

async fn validate_employee_references(
    pool: &PgPool,
    tenant_id: Uuid,
    account_id: Option<Uuid>,
    department_id: Option<Uuid>,
    position_id: Option<Uuid>,
) -> Result<()> {
    ensure_department(pool, tenant_id, department_id).await?;
    if let Some(position_id) = position_id {
        let position = PositionOps::get_by_id(pool, tenant_id, position_id)
            .await?
            .context("Position was not found for this campus")?;
        if let (Some(department_id), Some(position_department_id)) =
            (department_id, position.department_id)
            && department_id != position_department_id
        {
            bail!("Position does not belong to the selected department");
        }
    }
    if let Some(account_id) = account_id {
        let exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM users
                WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            )
            "#,
        )
        .bind(tenant_id)
        .bind(account_id)
        .fetch_one(pool)
        .await
        .context("Failed to validate employee account")?;
        if !exists {
            bail!("Account was not found for this campus");
        }
    }
    Ok(())
}

fn validate_employment_dates(
    hire_date: Option<chrono::NaiveDate>,
    end_date: Option<chrono::NaiveDate>,
) -> Result<()> {
    if let (Some(hire_date), Some(end_date)) = (hire_date, end_date)
        && end_date < hire_date
    {
        bail!("Employment end date cannot be before the hire date");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, TimeZone, Utc};
    use cp_audit::AuditActor;
    use cp_common::{
        AccessContext, EntitlementSnapshot, LeaseLifecycle, ModuleEntitlementState,
        RecordScopeFamilyKey, RecordScopeGrant, RecordScopeGrants, RecordScopeKind,
    };
    use uuid::Uuid;

    use super::{
        DeleteOutcome, EmployeeAvailabilityReadScope, EmployeeReadScope,
        EmploymentEngagementReadScope, HrImportReadScope, validate_availability_times,
        validate_availability_transition, validate_employment_dates, validate_engagement,
        validate_engagement_transition,
    };

    fn access(permissions: &[&str]) -> AccessContext {
        AccessContext {
            role_keys: Vec::new(),
            permissions: permissions.iter().map(|value| value.to_string()).collect(),
            enabled_modules: vec!["hr_payroll".to_string()],
            entitlements: EntitlementSnapshot::new(
                LeaseLifecycle::Legacy,
                vec![("hr_payroll".to_string(), ModuleEntitlementState::Enabled)],
                Vec::new(),
            )
            .unwrap_or_else(|_| unreachable!()),
        }
    }

    fn grants(family: &str, kind: RecordScopeKind) -> RecordScopeGrants {
        let family = RecordScopeFamilyKey::parse(family).unwrap_or_else(|_| unreachable!());
        RecordScopeGrants::from_grants([RecordScopeGrant::new(family, kind)])
    }

    #[test]
    fn employment_dates_reject_reverse_ranges() {
        let hire = NaiveDate::from_ymd_opt(2026, 8, 27).unwrap_or_else(|| unreachable!());
        let end = NaiveDate::from_ymd_opt(2026, 8, 26).unwrap_or_else(|| unreachable!());
        assert!(validate_employment_dates(Some(hire), Some(end)).is_err());
        assert!(validate_employment_dates(Some(hire), None).is_ok());
        assert_eq!(DeleteOutcome::InUse, DeleteOutcome::InUse);
    }

    #[test]
    fn employment_lifecycle_is_forward_only() {
        let start = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap_or_else(|| unreachable!());
        let end = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap_or_else(|| unreachable!());
        assert!(validate_engagement("fixed_term", "active", Some(start), None).is_err());
        assert!(validate_engagement("fixed_term", "active", Some(start), Some(end)).is_ok());
        assert!(validate_engagement_transition("draft", "active").is_ok());
        assert!(validate_engagement_transition("active", "ended").is_ok());
        assert!(validate_engagement_transition("ended", "active").is_err());
        assert!(validate_engagement_transition("cancelled", "draft").is_err());
    }

    #[test]
    fn availability_lifecycle_preserves_decisions() {
        assert!(validate_availability_transition("draft", "submitted").is_ok());
        assert!(validate_availability_transition("submitted", "approved").is_ok());
        assert!(validate_availability_transition("approved", "cancelled").is_ok());
        assert!(validate_availability_transition("approved", "submitted").is_err());
        assert!(validate_availability_transition("rejected", "approved").is_err());

        let start = Utc
            .with_ymd_and_hms(2026, 8, 28, 8, 0, 0)
            .single()
            .unwrap_or_else(|| unreachable!());
        let end = Utc
            .with_ymd_and_hms(2026, 8, 28, 17, 0, 0)
            .single()
            .unwrap_or_else(|| unreachable!());
        assert!(validate_availability_times(start, end).is_ok());
        assert!(validate_availability_times(end, start).is_err());
    }

    #[test]
    fn self_read_scopes_bind_every_hr_family_to_the_actor_account() {
        let actor_id = Uuid::new_v4();
        let actor = AuditActor::person(actor_id);
        let access = access(&["hr_payroll:view"]);

        let employees = EmployeeReadScope::from_authority(
            &access,
            &grants("hr.employees", RecordScopeKind::SelfRecord),
            actor,
        )
        .unwrap_or_else(|| unreachable!());
        let engagements = EmploymentEngagementReadScope::from_authority(
            &access,
            &grants("hr.engagements", RecordScopeKind::SelfRecord),
            actor,
        )
        .unwrap_or_else(|| unreachable!());
        let availability = EmployeeAvailabilityReadScope::from_authority(
            &access,
            &grants("hr.availability", RecordScopeKind::SelfRecord),
            actor,
        )
        .unwrap_or_else(|| unreachable!());

        assert_eq!(employees.account_filter(), Some(actor_id));
        assert_eq!(engagements.account_filter(), Some(actor_id));
        assert_eq!(availability.account_filter(), Some(actor_id));
    }

    #[test]
    fn campus_and_wildcard_authority_keep_campus_visibility() {
        let actor = AuditActor::person(Uuid::new_v4());
        let campus = access(&["hr_payroll:view"]);
        let wildcard = access(&["*"]);

        let employees = EmployeeReadScope::from_authority(
            &campus,
            &grants("hr.employees", RecordScopeKind::Campus),
            actor,
        )
        .unwrap_or_else(|| unreachable!());
        let wildcard_availability = EmployeeAvailabilityReadScope::from_authority(
            &wildcard,
            &RecordScopeGrants::empty(),
            actor,
        )
        .unwrap_or_else(|| unreachable!());

        assert_eq!(employees.account_filter(), None);
        assert_eq!(wildcard_availability.account_filter(), None);
        assert!(
            HrImportReadScope::from_authority(
                &campus,
                &grants("hr.imports", RecordScopeKind::Campus),
            )
            .is_some()
        );
        assert!(
            HrImportReadScope::from_authority(&wildcard, &RecordScopeGrants::empty()).is_some()
        );
    }

    #[test]
    fn missing_mismatched_and_unsupported_hr_scopes_fail_closed() {
        let actor = AuditActor::person(Uuid::new_v4());
        let access = access(&["hr_payroll:view"]);

        assert!(
            EmployeeReadScope::from_authority(&access, &RecordScopeGrants::empty(), actor)
                .is_none()
        );
        assert!(
            EmploymentEngagementReadScope::from_authority(
                &access,
                &grants("hr.employees", RecordScopeKind::Campus),
                actor,
            )
            .is_none()
        );
        assert!(
            EmployeeAvailabilityReadScope::from_authority(
                &access,
                &grants("hr.availability", RecordScopeKind::Assigned),
                actor,
            )
            .is_none()
        );
        assert!(
            EmployeeReadScope::from_authority(
                &access,
                &grants("hr.employees", RecordScopeKind::SelfRecord),
                AuditActor::system(),
            )
            .is_none()
        );
        assert!(
            HrImportReadScope::from_authority(
                &access,
                &grants("hr.imports", RecordScopeKind::SelfRecord),
            )
            .is_none()
        );
    }
}
