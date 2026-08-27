//! Persistence projections for the Academics-owned teaching structure.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AcademicYear {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Subject {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub code: String,
    pub name: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TeacherProfile {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub employee_id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeacherProfileWithEmployee {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub employee_id: Uuid,
    pub employee_number: String,
    pub display_name: String,
    pub work_email: Option<String>,
    pub phone: Option<String>,
    pub employment_status: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct TeachingAssignmentRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_year_name: String,
    pub class_group_id: Uuid,
    pub class_group_name: String,
    pub subject_id: Uuid,
    pub subject_name: String,
    pub teacher_profile_id: Uuid,
    pub employee_id: Uuid,
    pub periods_per_cycle: i16,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ClassGroupWithYear {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_year_name: String,
    pub code: String,
    pub name: String,
    pub grade_level: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TeachingAssignmentWithDetails {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_year_name: String,
    pub class_group_id: Uuid,
    pub class_group_name: String,
    pub subject_id: Uuid,
    pub subject_name: String,
    pub teacher_profile_id: Uuid,
    pub employee_id: Uuid,
    pub teacher_name: String,
    pub periods_per_cycle: i16,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Canonical Academics resources consumed by Timetabling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimetablingReferenceData {
    pub academic_year: AcademicYear,
    pub classes: Vec<ClassGroupWithYear>,
    pub subjects: Vec<Subject>,
    pub teachers: Vec<TeacherProfileWithEmployee>,
    pub assignments: Vec<TeachingAssignmentWithDetails>,
}
