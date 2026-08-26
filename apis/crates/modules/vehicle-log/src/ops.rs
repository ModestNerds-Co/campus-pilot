//
//  cp-vehicle-log
//  ops.rs
//
//  Created by Ngonidzashe Mangudya on 2026/08/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use anyhow::{Context, Result as OpsResult};
use chrono::NaiveDate;
use sqlx::PgPool;
use uuid::Uuid;

use super::dtos::{CreateVehicleDailyLogRequest, UpdateVehicleDailyLogRequest};
use super::models::VehicleDailyLogWithDetails;

pub struct VehicleDailyLogOps;

impl VehicleDailyLogOps {
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        vehicle_id: Option<Uuid>,
        driver_id: Option<Uuid>,
        status: Option<&str>,
        from_date: Option<NaiveDate>,
        to_date: Option<NaiveDate>,
    ) -> OpsResult<(Vec<VehicleDailyLogWithDetails>, i64)> {
        let offset = (page - 1) * per_page;

        let logs = sqlx::query_as!(
            VehicleDailyLogWithDetails,
            r#"
            SELECT
                l.id, l.vehicle_id, v.registration_number AS vehicle_registration,
                l.driver_id, d.full_name AS driver_name,
                l.log_date, l.start_odometer, l.end_odometer, l.start_time, l.end_time,
                l.destination, l.purpose, l.fuel_added_liters, l.fuel_cost, l.status,
                l.created_at, l.updated_at
            FROM vehicle_daily_logs l
            JOIN vehicles v ON v.id = l.vehicle_id
            JOIN drivers d ON d.id = l.driver_id
            WHERE l.tenant_id = $1 AND l.deleted_at IS NULL
              AND ($2::UUID IS NULL OR l.vehicle_id = $2)
              AND ($3::UUID IS NULL OR l.driver_id = $3)
              AND ($4::TEXT IS NULL OR l.status = $4)
              AND ($5::DATE IS NULL OR l.log_date >= $5)
              AND ($6::DATE IS NULL OR l.log_date <= $6)
            ORDER BY l.log_date DESC, l.created_at DESC
            LIMIT $7 OFFSET $8
            "#,
            tenant_id,
            vehicle_id,
            driver_id,
            status,
            from_date,
            to_date,
            per_page,
            offset
        )
        .fetch_all(pool)
        .await
        .context("Failed to list vehicle daily logs")?;

        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM vehicle_daily_logs l
            WHERE l.tenant_id = $1 AND l.deleted_at IS NULL
              AND ($2::UUID IS NULL OR l.vehicle_id = $2)
              AND ($3::UUID IS NULL OR l.driver_id = $3)
              AND ($4::TEXT IS NULL OR l.status = $4)
              AND ($5::DATE IS NULL OR l.log_date >= $5)
              AND ($6::DATE IS NULL OR l.log_date <= $6)
            "#,
            tenant_id,
            vehicle_id,
            driver_id,
            status,
            from_date,
            to_date
        )
        .fetch_one(pool)
        .await
        .context("Failed to count vehicle daily logs")?;

        Ok((logs, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> OpsResult<Option<VehicleDailyLogWithDetails>> {
        let log = sqlx::query_as!(
            VehicleDailyLogWithDetails,
            r#"
            SELECT
                l.id, l.vehicle_id, v.registration_number AS vehicle_registration,
                l.driver_id, d.full_name AS driver_name,
                l.log_date, l.start_odometer, l.end_odometer, l.start_time, l.end_time,
                l.destination, l.purpose, l.fuel_added_liters, l.fuel_cost, l.status,
                l.created_at, l.updated_at
            FROM vehicle_daily_logs l
            JOIN vehicles v ON v.id = l.vehicle_id
            JOIN drivers d ON d.id = l.driver_id
            WHERE l.id = $1 AND l.tenant_id = $2 AND l.deleted_at IS NULL
            "#,
            id,
            tenant_id
        )
        .fetch_optional(pool)
        .await
        .context("Failed to fetch vehicle daily log")?;

        Ok(log)
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        req: &CreateVehicleDailyLogRequest,
    ) -> OpsResult<Uuid> {
        let id = sqlx::query_scalar!(
            r#"
            INSERT INTO vehicle_daily_logs (
                tenant_id, vehicle_id, driver_id, log_date, start_odometer, end_odometer,
                start_time, end_time, destination, purpose, fuel_added_liters, fuel_cost, status
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, COALESCE($13, 'draft'))
            RETURNING id
            "#,
            tenant_id,
            req.vehicle_id,
            req.driver_id,
            req.log_date,
            req.start_odometer,
            req.end_odometer,
            req.start_time,
            req.end_time,
            req.destination,
            req.purpose,
            req.fuel_added_liters,
            req.fuel_cost,
            req.status
        )
        .fetch_one(pool)
        .await
        .context("Failed to create vehicle daily log")?;

        Ok(id)
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        req: &UpdateVehicleDailyLogRequest,
    ) -> OpsResult<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE vehicle_daily_logs
            SET vehicle_id = COALESCE($1, vehicle_id),
                driver_id = COALESCE($2, driver_id),
                log_date = COALESCE($3, log_date),
                start_odometer = COALESCE($4, start_odometer),
                end_odometer = COALESCE($5, end_odometer),
                start_time = COALESCE($6, start_time),
                end_time = COALESCE($7, end_time),
                destination = COALESCE($8, destination),
                purpose = COALESCE($9, purpose),
                fuel_added_liters = COALESCE($10, fuel_added_liters),
                fuel_cost = COALESCE($11, fuel_cost),
                status = COALESCE($12, status),
                updated_at = NOW()
            WHERE id = $13 AND tenant_id = $14 AND deleted_at IS NULL
            "#,
            req.vehicle_id,
            req.driver_id,
            req.log_date,
            req.start_odometer,
            req.end_odometer,
            req.start_time,
            req.end_time,
            req.destination,
            req.purpose,
            req.fuel_added_liters,
            req.fuel_cost,
            req.status,
            id,
            tenant_id
        )
        .execute(pool)
        .await
        .context("Failed to update vehicle daily log")?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> OpsResult<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE vehicle_daily_logs
            SET deleted_at = NOW()
            WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
            "#,
            id,
            tenant_id
        )
        .execute(pool)
        .await
        .context("Failed to delete vehicle daily log")?;

        Ok(result.rows_affected() > 0)
    }
}
