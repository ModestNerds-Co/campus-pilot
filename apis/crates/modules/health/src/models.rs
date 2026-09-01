//! Private persistence rows for Health services.

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub(crate) struct PatientRow {
    pub id: Uuid,
    pub status: String,
    pub version: i32,
    pub active_care_item_count: i64,
    pub open_visit_count: i64,
    pub active_medication_count: i64,
    pub open_follow_up_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Debug, Clone, FromRow)]
pub(crate) struct CareItemRow {
    pub id: Uuid,
    pub patient_id: Uuid,
    pub kind: String,
    pub title: String,
    pub details: Option<String>,
    pub severity: String,
    pub status: String,
    pub reviewed_on: Option<NaiveDate>,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Debug, Clone, FromRow)]
pub(crate) struct VisitRow {
    pub id: Uuid,
    pub patient_id: Uuid,
    pub checked_in_at: DateTime<Utc>,
    pub category: String,
    pub presenting_concern: String,
    pub assessment: Option<String>,
    pub care_given: Option<String>,
    pub disposition: Option<String>,
    pub status: String,
    pub version: i32,
    pub closed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Debug, Clone, FromRow)]
pub(crate) struct MedicationPlanRow {
    pub id: Uuid,
    pub patient_id: Uuid,
    pub medication_name: String,
    pub dosage: String,
    pub route: String,
    pub schedule: String,
    pub instructions: Option<String>,
    pub authorization_reference: String,
    pub starts_on: NaiveDate,
    pub ends_on: Option<NaiveDate>,
    pub status: String,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Debug, Clone, FromRow)]
pub(crate) struct MedicationAdministrationRow {
    pub id: Uuid,
    pub medication_plan_id: Uuid,
    pub patient_id: Uuid,
    pub medication_name: String,
    pub administered_at: DateTime<Utc>,
    pub dose: String,
    pub outcome: String,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Clone, FromRow)]
pub(crate) struct FollowUpRow {
    pub id: Uuid,
    pub patient_id: Uuid,
    pub visit_id: Option<Uuid>,
    pub assigned_employee_id: Option<Uuid>,
    pub due_on: NaiveDate,
    pub purpose: String,
    pub status: String,
    pub outcome: Option<String>,
    pub version: i32,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
