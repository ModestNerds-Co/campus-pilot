//
//  cp-hr-payroll
//  ops.rs
//
//  Created by OpenAI Codex on 2026/08/27.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use anyhow::{Context, Result, bail};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    dtos::{
        CreateDepartmentRequest, CreateEmployeeRequest, CreatePositionRequest,
        UpdateDepartmentRequest, UpdateEmployeeRequest, UpdatePositionRequest,
    },
    models::{Department, EmployeeReference, EmployeeWithDetails, Position},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteOutcome {
    Deleted,
    NotFound,
    InUse,
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
            ORDER BY employee.display_name, employee.employee_number
            LIMIT $7 OFFSET $8
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(department_id)
        .bind(position_id)
        .bind(account_linked)
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
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(department_id)
        .bind(position_id)
        .bind(account_linked)
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
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load employee")
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
            "SELECT EXISTS(SELECT 1 FROM drivers WHERE tenant_id = $1 AND employee_id = $2 AND deleted_at IS NULL)",
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
    use chrono::NaiveDate;

    use super::{DeleteOutcome, validate_employment_dates};

    #[test]
    fn employment_dates_reject_reverse_ranges() {
        let hire = NaiveDate::from_ymd_opt(2026, 8, 27).unwrap_or_else(|| unreachable!());
        let end = NaiveDate::from_ymd_opt(2026, 8, 26).unwrap_or_else(|| unreachable!());
        assert!(validate_employment_dates(Some(hire), Some(end)).is_err());
        assert!(validate_employment_dates(Some(hire), None).is_ok());
        assert_eq!(DeleteOutcome::InUse, DeleteOutcome::InUse);
    }
}
