//
//  cp-hr-payroll
//  models.rs
//
//  Created by OpenAI Codex on 2026/08/27.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Department {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub code: String,
    pub name: String,
    pub status: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Position {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub department_id: Option<Uuid>,
    pub code: String,
    pub title: String,
    pub status: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EmployeeWithDetails {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub account_id: Option<Uuid>,
    pub account_email: Option<String>,
    pub employee_number: String,
    pub display_name: String,
    pub first_names: Option<String>,
    pub surname: Option<String>,
    pub work_email: Option<String>,
    pub phone: Option<String>,
    pub department_id: Option<Uuid>,
    pub department_name: Option<String>,
    pub position_id: Option<Uuid>,
    pub position_title: Option<String>,
    pub employment_status: String,
    pub hire_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeReference {
    pub id: Uuid,
    pub account_id: Option<Uuid>,
    pub employee_number: String,
    pub display_name: String,
    pub work_email: Option<String>,
    pub phone: Option<String>,
    pub employment_status: String,
}

impl From<EmployeeWithDetails> for EmployeeReference {
    fn from(value: EmployeeWithDetails) -> Self {
        Self {
            id: value.id,
            account_id: value.account_id,
            employee_number: value.employee_number,
            display_name: value.display_name,
            work_email: value.work_email,
            phone: value.phone,
            employment_status: value.employment_status,
        }
    }
}
