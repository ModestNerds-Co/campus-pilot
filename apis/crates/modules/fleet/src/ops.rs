//
//  cp-fleet
//  ops.rs
//
//  Created by Ngonidzashe Mangudya on 2026/08/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use anyhow::{Context, Result as OpsResult};
use sqlx::PgPool;
use uuid::Uuid;

use super::dtos::{CreateDriverRequest, CreateVehicleRequest, UpdateDriverRequest, UpdateVehicleRequest};
use super::models::{Driver, Vehicle};

pub struct VehicleOps;

impl VehicleOps {
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

        let drivers = sqlx::query_as!(
            Driver,
            r#"
            SELECT id, tenant_id, employee_id, full_name, license_number, license_class,
                   license_expiry, phone, status, created_at, updated_at, deleted_at
            FROM drivers
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND ($2::TEXT IS NULL OR full_name ILIKE $2 OR license_number ILIKE $2)
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
        .context("Failed to list drivers")?;

        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM drivers
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND ($2::TEXT IS NULL OR full_name ILIKE $2 OR license_number ILIKE $2)
              AND ($3::TEXT IS NULL OR status = $3)
            "#,
            tenant_id,
            search_pattern,
            status
        )
        .fetch_one(pool)
        .await
        .context("Failed to count drivers")?;

        Ok((drivers, total))
    }

    pub async fn get_by_id(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> OpsResult<Option<Driver>> {
        let driver = sqlx::query_as!(
            Driver,
            r#"
            SELECT id, tenant_id, employee_id, full_name, license_number, license_class,
                   license_expiry, phone, status, created_at, updated_at, deleted_at
            FROM drivers
            WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
            "#,
            id,
            tenant_id
        )
        .fetch_optional(pool)
        .await
        .context("Failed to fetch driver")?;

        Ok(driver)
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
        let driver = sqlx::query_as!(
            Driver,
            r#"
            INSERT INTO drivers (
                tenant_id, employee_id, full_name, license_number, license_class,
                license_expiry, phone, status
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, COALESCE($8, 'active'))
            RETURNING id, tenant_id, employee_id, full_name, license_number, license_class,
                      license_expiry, phone, status, created_at, updated_at, deleted_at
            "#,
            tenant_id,
            req.employee_id,
            req.full_name,
            req.license_number,
            req.license_class,
            req.license_expiry,
            req.phone,
            req.status
        )
        .fetch_one(pool)
        .await
        .context("Failed to create driver")?;

        Ok(driver)
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        req: &UpdateDriverRequest,
    ) -> OpsResult<Option<Driver>> {
        let driver = sqlx::query_as!(
            Driver,
            r#"
            UPDATE drivers
            SET full_name = COALESCE($1, full_name),
                license_number = COALESCE($2, license_number),
                license_class = COALESCE($3, license_class),
                license_expiry = COALESCE($4, license_expiry),
                phone = COALESCE($5, phone),
                employee_id = COALESCE($6, employee_id),
                status = COALESCE($7, status),
                updated_at = NOW()
            WHERE id = $8 AND tenant_id = $9 AND deleted_at IS NULL
            RETURNING id, tenant_id, employee_id, full_name, license_number, license_class,
                      license_expiry, phone, status, created_at, updated_at, deleted_at
            "#,
            req.full_name,
            req.license_number,
            req.license_class,
            req.license_expiry,
            req.phone,
            req.employee_id,
            req.status,
            id,
            tenant_id
        )
        .fetch_optional(pool)
        .await
        .context("Failed to update driver")?;

        Ok(driver)
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

