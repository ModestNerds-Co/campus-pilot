//! Persistence projections for SIS-owned people and admissions records.
//!
//! Academic labels are hydrated through Academics operations; these rows keep
//! only stable foreign identifiers for that module.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LearnerWithAccount {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub account_id: Option<Uuid>,
    pub account_email: Option<String>,
    pub learner_number: String,
    pub display_name: String,
    pub first_names: Option<String>,
    pub surname: Option<String>,
    pub date_of_birth: NaiveDate,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Minimum SIS-owned projection for authorised billing workflows.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LearnerBillingReference {
    pub id: Uuid,
    pub learner_number: String,
    pub display_name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GuardianWithAccount {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub account_id: Option<Uuid>,
    pub account_email: Option<String>,
    pub display_name: String,
    pub first_names: Option<String>,
    pub surname: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GuardianRelationshipWithDetails {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub learner_id: Uuid,
    pub learner_name: String,
    pub learner_number: String,
    pub guardian_id: Uuid,
    pub guardian_name: String,
    pub relationship_type: String,
    pub is_primary: bool,
    pub can_collect: bool,
    pub receives_communications: bool,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Application {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub application_number: String,
    pub learner_id: Uuid,
    pub academic_year_id: Uuid,
    pub target_grade_level_id: Option<Uuid>,
    pub submitted_on: Option<NaiveDate>,
    pub status: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationWithDetails {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub application_number: String,
    pub learner_id: Uuid,
    pub learner_name: String,
    pub learner_number: String,
    pub academic_year_id: Uuid,
    pub academic_year_name: String,
    pub target_grade_level_id: Option<Uuid>,
    pub target_grade_level_name: Option<String>,
    pub submitted_on: Option<NaiveDate>,
    pub status: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Enrolment {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub learner_id: Uuid,
    pub academic_year_id: Uuid,
    pub class_group_id: Uuid,
    pub source_application_id: Option<Uuid>,
    pub starts_on: NaiveDate,
    pub ends_on: Option<NaiveDate>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrolmentWithDetails {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub learner_id: Uuid,
    pub learner_name: String,
    pub learner_number: String,
    pub academic_year_id: Uuid,
    pub academic_year_name: String,
    pub class_group_id: Uuid,
    pub class_group_name: String,
    pub source_application_id: Option<Uuid>,
    pub application_number: Option<String>,
    pub starts_on: NaiveDate,
    pub ends_on: Option<NaiveDate>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Minimum SIS-owned identity and placement projection used by Attendance.
///
/// Attendance stores only stable identifiers. Names and learner numbers are
/// resolved from SIS whenever a register is read.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AttendanceRosterEntry {
    pub enrolment_id: Uuid,
    pub learner_id: Uuid,
    pub learner_number: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AccountCandidate {
    pub id: Uuid,
    pub full_name: String,
    pub email: String,
}
