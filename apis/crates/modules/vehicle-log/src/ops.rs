//
//  cp-vehicle-log
//  ops.rs
//
//  Created by Ngonidzashe Mangudya on 2026/08/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use anyhow::{Context, Result as OpsResult};
use chrono::NaiveDate;
use cp_fleet::ops::{DriverOps, VehicleOps};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use super::dtos::{CreateVehicleDailyLogRequest, UpdateVehicleDailyLogRequest};
use super::models::{VehicleDailyLog, VehicleDailyLogWithDetails};

pub struct VehicleDailyLogOps;

impl VehicleDailyLogOps {
    #[allow(clippy::too_many_arguments)]
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

        let logs = sqlx::query_as::<_, VehicleDailyLog>(
            r#"
            SELECT id, tenant_id, vehicle_id, driver_id, log_date, start_odometer,
                   end_odometer, start_time, end_time, destination, purpose,
                   fuel_added_liters, fuel_cost, status, created_at, updated_at, deleted_at
            FROM vehicle_daily_logs
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND ($2::UUID IS NULL OR vehicle_id = $2)
              AND ($3::UUID IS NULL OR driver_id = $3)
              AND ($4::TEXT IS NULL OR status = $4)
              AND ($5::DATE IS NULL OR log_date >= $5)
              AND ($6::DATE IS NULL OR log_date <= $6)
            ORDER BY log_date DESC, created_at DESC
            LIMIT $7 OFFSET $8
            "#,
        )
        .bind(tenant_id)
        .bind(vehicle_id)
        .bind(driver_id)
        .bind(status)
        .bind(from_date)
        .bind(to_date)
        .bind(per_page)
        .bind(offset)
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

        Ok((hydrate_logs(pool, tenant_id, logs).await?, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> OpsResult<Option<VehicleDailyLogWithDetails>> {
        let log = sqlx::query_as::<_, VehicleDailyLog>(
            r#"
            SELECT id, tenant_id, vehicle_id, driver_id, log_date, start_odometer,
                   end_odometer, start_time, end_time, destination, purpose,
                   fuel_added_liters, fuel_cost, status, created_at, updated_at, deleted_at
            FROM vehicle_daily_logs
            WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(pool)
        .await
        .context("Failed to fetch vehicle daily log")?;

        match log {
            Some(log) => Ok(hydrate_logs(pool, tenant_id, vec![log])
                .await?
                .into_iter()
                .next()),
            None => Ok(None),
        }
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

async fn hydrate_logs(
    pool: &PgPool,
    tenant_id: Uuid,
    logs: Vec<VehicleDailyLog>,
) -> OpsResult<Vec<VehicleDailyLogWithDetails>> {
    let vehicle_ids = logs.iter().map(|log| log.vehicle_id).collect::<Vec<_>>();
    let driver_ids = logs.iter().map(|log| log.driver_id).collect::<Vec<_>>();
    let vehicles = VehicleOps::references_by_ids(pool, tenant_id, &vehicle_ids)
        .await?
        .into_iter()
        .map(|vehicle| (vehicle.id, vehicle.registration_number))
        .collect::<HashMap<_, _>>();
    let drivers = DriverOps::references_by_ids(pool, tenant_id, &driver_ids)
        .await?
        .into_iter()
        .map(|driver| (driver.id, driver.employee.display_name))
        .collect::<HashMap<_, _>>();
    logs.into_iter()
        .map(|log| {
            let vehicle_registration = vehicles
                .get(&log.vehicle_id)
                .cloned()
                .context("Vehicle reference is unavailable")?;
            let driver_name = drivers
                .get(&log.driver_id)
                .cloned()
                .context("Driver reference is unavailable")?;
            Ok(VehicleDailyLogWithDetails {
                id: log.id,
                vehicle_id: log.vehicle_id,
                vehicle_registration,
                driver_id: log.driver_id,
                driver_name,
                log_date: log.log_date,
                start_odometer: log.start_odometer,
                end_odometer: log.end_odometer,
                start_time: log.start_time,
                end_time: log.end_time,
                destination: log.destination,
                purpose: log.purpose,
                fuel_added_liters: log.fuel_added_liters,
                fuel_cost: log.fuel_cost,
                status: log.status,
                created_at: log.created_at,
                updated_at: log.updated_at,
            })
        })
        .collect()
}
