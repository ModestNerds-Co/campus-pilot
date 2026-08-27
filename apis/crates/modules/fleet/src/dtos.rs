//
//  cp-fleet
//  dtos.rs
//
//  Created by Ngonidzashe Mangudya on 2026/08/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use super::models::{Driver, Vehicle};
use cp_hr_payroll::models::EmployeeReference;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateVehicleRequest {
    #[validate(length(min = 1, message = "Registration number is required"))]
    pub registration_number: String,
    #[validate(length(min = 1, message = "Make is required"))]
    pub make: String,
    #[validate(length(min = 1, message = "Model is required"))]
    pub model: String,
    pub year: Option<i32>,
    pub vehicle_type: Option<String>,
    pub capacity: Option<i32>,
    pub fuel_type: Option<String>,
    pub status: Option<String>,
    pub current_odometer: Option<i32>,
    pub insurance_expiry: Option<NaiveDate>,
    pub license_expiry: Option<NaiveDate>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateVehicleRequest {
    #[validate(length(min = 1, message = "Registration number cannot be empty"))]
    pub registration_number: Option<String>,
    #[validate(length(min = 1, message = "Make cannot be empty"))]
    pub make: Option<String>,
    #[validate(length(min = 1, message = "Model cannot be empty"))]
    pub model: Option<String>,
    pub year: Option<i32>,
    pub vehicle_type: Option<String>,
    pub capacity: Option<i32>,
    pub fuel_type: Option<String>,
    pub status: Option<String>,
    pub current_odometer: Option<i32>,
    pub insurance_expiry: Option<NaiveDate>,
    pub license_expiry: Option<NaiveDate>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct VehicleResponse {
    pub id: Uuid,
    pub registration_number: String,
    pub make: String,
    pub model: String,
    pub year: Option<i32>,
    pub vehicle_type: String,
    pub capacity: Option<i32>,
    pub fuel_type: String,
    pub status: String,
    pub current_odometer: i32,
    pub insurance_expiry: Option<NaiveDate>,
    pub license_expiry: Option<NaiveDate>,
    pub notes: Option<String>,
}

impl From<Vehicle> for VehicleResponse {
    fn from(v: Vehicle) -> Self {
        Self {
            id: v.id,
            registration_number: v.registration_number,
            make: v.make,
            model: v.model,
            year: v.year,
            vehicle_type: v.vehicle_type,
            capacity: v.capacity,
            fuel_type: v.fuel_type,
            status: v.status,
            current_odometer: v.current_odometer,
            insurance_expiry: v.insurance_expiry,
            license_expiry: v.license_expiry,
            notes: v.notes,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListVehiclesQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedVehiclesResponse {
    pub vehicles: Vec<VehicleResponse>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateDriverRequest {
    pub employee_id: Uuid,
    #[validate(length(min = 1, message = "License number is required"))]
    pub license_number: String,
    pub license_class: Option<String>,
    pub license_expiry: Option<NaiveDate>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateDriverRequest {
    #[validate(length(min = 1, message = "License number cannot be empty"))]
    pub license_number: Option<String>,
    pub license_class: Option<String>,
    pub license_expiry: Option<NaiveDate>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DriverResponse {
    pub id: Uuid,
    pub employee: EmployeeReference,
    pub license_number: String,
    pub license_class: Option<String>,
    pub license_expiry: Option<NaiveDate>,
    pub status: String,
}

impl From<Driver> for DriverResponse {
    fn from(d: Driver) -> Self {
        Self {
            id: d.id,
            employee: d.employee,
            license_number: d.license_number,
            license_class: d.license_class,
            license_expiry: d.license_expiry,
            status: d.status,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListDriversQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedDriversResponse {
    pub drivers: Vec<DriverResponse>,
}

#[derive(Debug, Serialize)]
pub struct DriverCandidatesResponse {
    pub employees: Vec<EmployeeReference>,
}

#[derive(Debug, Deserialize)]
pub struct DriverCandidatesQuery {
    pub search: Option<String>,
}
