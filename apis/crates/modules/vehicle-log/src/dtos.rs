//
//  cp-vehicle-log
//  dtos.rs
//
//  Created by Ngonidzashe Mangudya on 2026/08/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use super::models::VehicleDailyLogWithDetails;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateVehicleDailyLogRequest {
    pub vehicle_id: Uuid,
    pub driver_id: Uuid,
    pub log_date: NaiveDate,
    pub start_odometer: i32,
    pub end_odometer: Option<i32>,
    pub start_time: Option<NaiveTime>,
    pub end_time: Option<NaiveTime>,
    pub destination: Option<String>,
    #[validate(length(min = 1, message = "Purpose of trip is required"))]
    pub purpose: String,
    pub fuel_added_liters: Option<f64>,
    pub fuel_cost: Option<f64>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateVehicleDailyLogRequest {
    pub vehicle_id: Option<Uuid>,
    pub driver_id: Option<Uuid>,
    pub log_date: Option<NaiveDate>,
    pub start_odometer: Option<i32>,
    pub end_odometer: Option<i32>,
    pub start_time: Option<NaiveTime>,
    pub end_time: Option<NaiveTime>,
    pub destination: Option<String>,
    #[validate(length(min = 1, message = "Purpose of trip cannot be empty"))]
    pub purpose: Option<String>,
    pub fuel_added_liters: Option<f64>,
    pub fuel_cost: Option<f64>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct VehicleDailyLogResponse {
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
}

impl From<VehicleDailyLogWithDetails> for VehicleDailyLogResponse {
    fn from(l: VehicleDailyLogWithDetails) -> Self {
        Self {
            id: l.id,
            vehicle_id: l.vehicle_id,
            vehicle_registration: l.vehicle_registration,
            driver_id: l.driver_id,
            driver_name: l.driver_name,
            log_date: l.log_date,
            start_odometer: l.start_odometer,
            end_odometer: l.end_odometer,
            start_time: l.start_time,
            end_time: l.end_time,
            destination: l.destination,
            purpose: l.purpose,
            fuel_added_liters: l.fuel_added_liters,
            fuel_cost: l.fuel_cost,
            status: l.status,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListVehicleDailyLogsQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub vehicle_id: Option<Uuid>,
    pub driver_id: Option<Uuid>,
    pub status: Option<String>,
    pub from_date: Option<NaiveDate>,
    pub to_date: Option<NaiveDate>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedVehicleDailyLogsResponse {
    pub logs: Vec<VehicleDailyLogResponse>,
}
