//
//  cp-fleet
//  ops.rs
//
//  Created by Ngonidzashe Mangudya on 2026/08/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use std::collections::HashMap;

use anyhow::{Context, Result as OpsResult, bail};
use cp_hr_payroll::{models::EmployeeReference, ops::EmployeeOps};
use sqlx::PgPool;
use uuid::Uuid;

use super::dtos::{
    CreateDriverRequest, CreateVehicleRequest, UpdateDriverRequest, UpdateVehicleRequest,
};
use super::models::{Driver, DriverProfile, Vehicle};

pub struct VehicleOps;

impl VehicleOps {
    pub async fn references_by_ids(
        pool: &PgPool,
        tenant_id: Uuid,
        ids: &[Uuid],
    ) -> OpsResult<Vec<Vehicle>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as::<_, Vehicle>(
            r#"
            SELECT id, tenant_id, registration_number, make, model, year, vehicle_type,
                   capacity, fuel_type, status, current_odometer, insurance_expiry,
                   license_expiry, notes, created_at, updated_at, deleted_at
            FROM vehicles
            WHERE tenant_id = $1 AND id = ANY($2)
            "#,
        )
        .bind(tenant_id)
        .bind(ids)
        .fetch_all(pool)
        .await
        .context("Failed to load vehicle references")
    }

    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
    ) -> OpsResult<(Vec<Vehicle>, i64)> {
        let offset = (page - 1) * per_page;
        let search_pattern = search.map(|s| format!("%{}%", s));

        let vehicles = sqlx::query_as!(
            Vehicle,
            r#"
            SELECT id, tenant_id, registration_number, make, model, year, vehicle_type,
                   capacity, fuel_type, status, current_odometer, insurance_expiry,
                   license_expiry, notes, created_at, updated_at, deleted_at
            FROM vehicles
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND ($2::TEXT IS NULL OR registration_number ILIKE $2 OR make ILIKE $2 OR model ILIKE $2)
              AND ($3::TEXT IS NULL OR status = $3)
            ORDER BY created_at DESC
            LIMIT $4 OFFSET $5
            "#,
            tenant_id,
            search_pattern,
            status,
            per_page,
            offset
        )
        .fetch_all(pool)
        .await
        .context("Failed to list vehicles")?;

        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM vehicles
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND ($2::TEXT IS NULL OR registration_number ILIKE $2 OR make ILIKE $2 OR model ILIKE $2)
              AND ($3::TEXT IS NULL OR status = $3)
            "#,
            tenant_id,
            search_pattern,
            status
        )
        .fetch_one(pool)
        .await
        .context("Failed to count vehicles")?;

        Ok((vehicles, total))
    }

    pub async fn get_by_id(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> OpsResult<Option<Vehicle>> {
        let vehicle = sqlx::query_as!(
            Vehicle,
            r#"
            SELECT id, tenant_id, registration_number, make, model, year, vehicle_type,
                   capacity, fuel_type, status, current_odometer, insurance_expiry,
                   license_expiry, notes, created_at, updated_at, deleted_at
            FROM vehicles
            WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
            "#,
            id,
            tenant_id
        )
        .fetch_optional(pool)
        .await
        .context("Failed to fetch vehicle")?;

        Ok(vehicle)
    }

    pub async fn registration_exists(
        pool: &PgPool,
        tenant_id: Uuid,
        registration_number: &str,
        exclude_id: Option<Uuid>,
    ) -> OpsResult<bool> {
        let exists = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM vehicles
                WHERE tenant_id = $1 AND registration_number = $2
                  AND ($3::UUID IS NULL OR id != $3)
                  AND deleted_at IS NULL
            ) as "exists!"
            "#,
            tenant_id,
            registration_number,
            exclude_id
        )
        .fetch_one(pool)
        .await
        .context("Failed to check registration number existence")?;

        Ok(exists)
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        req: &CreateVehicleRequest,
    ) -> OpsResult<Vehicle> {
        let vehicle = sqlx::query_as!(
            Vehicle,
            r#"
            INSERT INTO vehicles (
                tenant_id, registration_number, make, model, year, vehicle_type,
                capacity, fuel_type, status, current_odometer, insurance_expiry,
                license_expiry, notes
            )
            VALUES (
                $1, $2, $3, $4, $5, COALESCE($6, 'bus'),
                $7, COALESCE($8, 'diesel'), COALESCE($9, 'active'), COALESCE($10, 0), $11,
                $12, $13
            )
            RETURNING id, tenant_id, registration_number, make, model, year, vehicle_type,
                      capacity, fuel_type, status, current_odometer, insurance_expiry,
                      license_expiry, notes, created_at, updated_at, deleted_at
            "#,
            tenant_id,
            req.registration_number,
            req.make,
            req.model,
            req.year,
            req.vehicle_type,
            req.capacity,
            req.fuel_type,
            req.status,
            req.current_odometer,
            req.insurance_expiry,
            req.license_expiry,
            req.notes
        )
        .fetch_one(pool)
        .await
        .context("Failed to create vehicle")?;

        Ok(vehicle)
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        req: &UpdateVehicleRequest,
    ) -> OpsResult<Option<Vehicle>> {
        let vehicle = sqlx::query_as!(
            Vehicle,
            r#"
            UPDATE vehicles
            SET registration_number = COALESCE($1, registration_number),
                make = COALESCE($2, make),
                model = COALESCE($3, model),
                year = COALESCE($4, year),
                vehicle_type = COALESCE($5, vehicle_type),
                capacity = COALESCE($6, capacity),
                fuel_type = COALESCE($7, fuel_type),
                status = COALESCE($8, status),
                current_odometer = COALESCE($9, current_odometer),
                insurance_expiry = COALESCE($10, insurance_expiry),
                license_expiry = COALESCE($11, license_expiry),
                notes = COALESCE($12, notes),
                updated_at = NOW()
            WHERE id = $13 AND tenant_id = $14 AND deleted_at IS NULL
            RETURNING id, tenant_id, registration_number, make, model, year, vehicle_type,
                      capacity, fuel_type, status, current_odometer, insurance_expiry,
                      license_expiry, notes, created_at, updated_at, deleted_at
            "#,
            req.registration_number,
            req.make,
            req.model,
            req.year,
            req.vehicle_type,
            req.capacity,
            req.fuel_type,
            req.status,
            req.current_odometer,
            req.insurance_expiry,
            req.license_expiry,
            req.notes,
            id,
            tenant_id
        )
        .fetch_optional(pool)
        .await
        .context("Failed to update vehicle")?;

        Ok(vehicle)
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> OpsResult<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE vehicles
            SET deleted_at = NOW()
            WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
            "#,
            id,
            tenant_id
        )
        .execute(pool)
        .await
        .context("Failed to delete vehicle")?;

        Ok(result.rows_affected() > 0)
    }
}
pub struct DriverOps;

impl DriverOps {
    pub async fn list_candidates(
        pool: &PgPool,
        tenant_id: Uuid,
        search: Option<&str>,
    ) -> OpsResult<Vec<EmployeeReference>> {
        let employees =
            EmployeeOps::list_references(pool, tenant_id, search, Some("active"), 100).await?;
        if employees.is_empty() {
            return Ok(Vec::new());
        }
        let employee_ids = employees
            .iter()
            .map(|employee| employee.id)
            .collect::<Vec<_>>();
        let assigned = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT employee_id
            FROM drivers
            WHERE tenant_id = $1 AND employee_id = ANY($2) AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(&employee_ids)
        .fetch_all(pool)
        .await
        .context("Failed to load assigned driver employees")?
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
        Ok(employees
            .into_iter()
            .filter(|employee| !assigned.contains(&employee.id))
            .collect())
    }

    pub async fn references_by_ids(
        pool: &PgPool,
        tenant_id: Uuid,
        ids: &[Uuid],
    ) -> OpsResult<Vec<Driver>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let profiles = sqlx::query_as::<_, DriverProfile>(
            r#"
            SELECT id, tenant_id, employee_id, license_number, license_class,
                   license_expiry, status, created_at, updated_at, deleted_at
            FROM drivers
            WHERE tenant_id = $1 AND id = ANY($2)
            "#,
        )
        .bind(tenant_id)
        .bind(ids)
        .fetch_all(pool)
        .await
        .context("Failed to load driver references")?;
        hydrate_drivers(pool, tenant_id, profiles).await
    }

    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
    ) -> OpsResult<(Vec<Driver>, i64)> {
        let offset = (page - 1) * per_page;
        let search_pattern = search.map(|s| format!("%{}%", s));
        let employee_ids = match search {
            Some(search) => EmployeeOps::search_reference_ids(pool, tenant_id, search).await?,
            None => Vec::new(),
        };

        let profiles = sqlx::query_as::<_, DriverProfile>(
            r#"
            SELECT id, tenant_id, employee_id, license_number, license_class,
                   license_expiry, status, created_at, updated_at, deleted_at
            FROM drivers
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND ($2::TEXT IS NULL OR license_number ILIKE $2 OR employee_id = ANY($3))
              AND ($4::TEXT IS NULL OR status = $4)
            ORDER BY created_at DESC
            LIMIT $5 OFFSET $6
            "#,
        )
        .bind(tenant_id)
        .bind(&search_pattern)
        .bind(&employee_ids)
        .bind(status)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list drivers")?;

        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM drivers
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND ($2::TEXT IS NULL OR license_number ILIKE $2 OR employee_id = ANY($3))
              AND ($4::TEXT IS NULL OR status = $4)
            "#,
        )
        .bind(tenant_id)
        .bind(&search_pattern)
        .bind(&employee_ids)
        .bind(status)
        .fetch_one(pool)
        .await
        .context("Failed to count drivers")?;

        Ok((hydrate_drivers(pool, tenant_id, profiles).await?, total))
    }

    pub async fn get_by_id(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> OpsResult<Option<Driver>> {
        let profile = sqlx::query_as::<_, DriverProfile>(
            r#"
            SELECT id, tenant_id, employee_id, license_number, license_class,
                   license_expiry, status, created_at, updated_at, deleted_at
            FROM drivers
            WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(pool)
        .await
        .context("Failed to fetch driver")?;

        match profile {
            Some(profile) => Ok(Some(hydrate_driver(pool, tenant_id, profile).await?)),
            None => Ok(None),
        }
    }

    pub async fn license_exists(
        pool: &PgPool,
        tenant_id: Uuid,
        license_number: &str,
        exclude_id: Option<Uuid>,
    ) -> OpsResult<bool> {
        let exists = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM drivers
                WHERE tenant_id = $1 AND license_number = $2
                  AND ($3::UUID IS NULL OR id != $3)
                  AND deleted_at IS NULL
            ) as "exists!"
            "#,
            tenant_id,
            license_number,
            exclude_id
        )
        .fetch_one(pool)
        .await
        .context("Failed to check license number existence")?;

        Ok(exists)
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        req: &CreateDriverRequest,
    ) -> OpsResult<Driver> {
        let employee = EmployeeOps::get_reference(pool, tenant_id, req.employee_id)
            .await?
            .context("Employee was not found for this campus")?;
        if employee.employment_status != "active" {
            bail!("Only an active employee can be assigned as a driver");
        }
        let profile = sqlx::query_as::<_, DriverProfile>(
            r#"
            INSERT INTO drivers (
                tenant_id, employee_id, license_number, license_class,
                license_expiry, status
            )
            VALUES ($1, $2, $3, $4, $5, COALESCE($6, 'active'))
            RETURNING id, tenant_id, employee_id, license_number, license_class,
                      license_expiry, status, created_at, updated_at, deleted_at
            "#,
        )
        .bind(tenant_id)
        .bind(req.employee_id)
        .bind(req.license_number.trim())
        .bind(req.license_class.as_deref().map(str::trim))
        .bind(req.license_expiry)
        .bind(req.status.as_deref())
        .fetch_one(pool)
        .await
        .context("Failed to create driver")?;

        Ok(Driver::from_profile(profile, employee))
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        req: &UpdateDriverRequest,
    ) -> OpsResult<Option<Driver>> {
        let current = match Self::get_by_id(pool, tenant_id, id).await? {
            Some(value) => value,
            None => return Ok(None),
        };
        if req.status.as_deref() == Some("active") && current.employee.employment_status != "active"
        {
            bail!("An inactive employee cannot have an active driver profile");
        }
        let profile = sqlx::query_as::<_, DriverProfile>(
            r#"
            UPDATE drivers
            SET license_number = COALESCE($1, license_number),
                license_class = COALESCE($2, license_class),
                license_expiry = COALESCE($3, license_expiry),
                status = COALESCE($4, status),
                updated_at = NOW()
            WHERE id = $5 AND tenant_id = $6 AND deleted_at IS NULL
            RETURNING id, tenant_id, employee_id, license_number, license_class,
                      license_expiry, status, created_at, updated_at, deleted_at
            "#,
        )
        .bind(req.license_number.as_deref().map(str::trim))
        .bind(req.license_class.as_deref().map(str::trim))
        .bind(req.license_expiry)
        .bind(req.status.as_deref())
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(pool)
        .await
        .context("Failed to update driver")?;

        Ok(profile.map(|profile| Driver::from_profile(profile, current.employee)))
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> OpsResult<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE drivers
            SET deleted_at = NOW()
            WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
            "#,
            id,
            tenant_id
        )
        .execute(pool)
        .await
        .context("Failed to delete driver")?;

        Ok(result.rows_affected() > 0)
    }
}

async fn hydrate_drivers(
    pool: &PgPool,
    tenant_id: Uuid,
    profiles: Vec<DriverProfile>,
) -> OpsResult<Vec<Driver>> {
    let ids = profiles
        .iter()
        .map(|profile| profile.employee_id)
        .collect::<Vec<_>>();
    let mut employees = EmployeeOps::references_by_ids(pool, tenant_id, &ids)
        .await?
        .into_iter()
        .map(|employee| (employee.id, employee))
        .collect::<HashMap<_, _>>();
    profiles
        .into_iter()
        .map(|profile| {
            let employee = employees
                .remove(&profile.employee_id)
                .context("Driver employee reference is unavailable")?;
            Ok(Driver::from_profile(profile, employee))
        })
        .collect()
}

async fn hydrate_driver(
    pool: &PgPool,
    tenant_id: Uuid,
    profile: DriverProfile,
) -> OpsResult<Driver> {
    let employee = EmployeeOps::get_reference(pool, tenant_id, profile.employee_id)
        .await?
        .context("Driver employee reference is unavailable")?;
    Ok(Driver::from_profile(profile, employee))
}
