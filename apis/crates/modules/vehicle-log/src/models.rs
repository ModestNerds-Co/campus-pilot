//
//  cp-vehicle-log
//  models.rs
//
//  Created by Ngonidzashe Mangudya on 2026/08/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct VehicleDailyLog {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub vehicle_id: Uuid,
    pub driver_id: Uuid,
    pub log_date: NaiveDate,
    pub start_odometer: i32,
    pub end_odometer: Option<i32>,
    pub start_time: Option<NaiveTime>,
    pub end_time: Option<NaiveTime>,
    pub destination: Option<String>,
    pub purpose: String,
    pub fuel_added_liters: Option<f64>,
    pub fuel_cost: Option<f64>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Same row, with the vehicle/driver display fields joined in — what list
/// and detail endpoints actually return, so the client never needs a
/// second round-trip to show "which vehicle, which driver".
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct VehicleDailyLogWithDetails {
    pub id: Uuid,
    pub vehicle_id: Uuid,
    pub vehicle_registration: String,
    pub driver_id: Uuid,
    pub driver_name: String,
    pub log_date: NaiveDate,
    pub start_odometer: i32,
    pub end_odometer: Option<i32>,
    pub start_time: Option<NaiveTime>,
    pub end_time: Option<NaiveTime>,
    pub destination: Option<String>,
    pub purpose: String,
    pub fuel_added_liters: Option<f64>,
    pub fuel_cost: Option<f64>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
