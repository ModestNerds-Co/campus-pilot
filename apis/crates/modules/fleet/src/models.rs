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
pub struct Driver {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub employee_id: Option<Uuid>,
    pub full_name: String,
    pub license_number: String,
    pub license_class: Option<String>,
    pub license_expiry: Option<NaiveDate>,
    pub phone: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}
