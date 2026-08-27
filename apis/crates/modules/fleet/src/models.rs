//
//  cp-fleet
//  models.rs
//
//  Created by Ngonidzashe Mangudya on 2026/08/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use cp_hr_payroll::models::EmployeeReference;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Vehicle {
    pub id: Uuid,
    pub tenant_id: Uuid,
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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DriverProfile {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub employee_id: Uuid,
    pub license_number: String,
    pub license_class: Option<String>,
    pub license_expiry: Option<NaiveDate>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Driver {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub employee: EmployeeReference,
    pub license_number: String,
    pub license_class: Option<String>,
    pub license_expiry: Option<NaiveDate>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Driver {
    pub fn from_profile(profile: DriverProfile, employee: EmployeeReference) -> Self {
        Self {
            id: profile.id,
            tenant_id: profile.tenant_id,
            employee,
            license_number: profile.license_number,
            license_class: profile.license_class,
            license_expiry: profile.license_expiry,
            status: profile.status,
            created_at: profile.created_at,
            updated_at: profile.updated_at,
        }
    }
}
