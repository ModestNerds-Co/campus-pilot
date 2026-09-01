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

/// Minimum SIS-owned learner identity used by Library membership workflows.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LibraryLearnerReference {
    pub id: Uuid,
    pub account_id: Option<Uuid>,
    pub learner_number: String,
    pub display_name: String,
    pub status: String,
}

/// Current SIS-owned emergency contact projection for school health workflows.
///
/// Health stores no guardian contact copy; this projection is resolved when a
/// patient record is read so current SIS data remains authoritative.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct HealthGuardianContactReference {
    pub learner_id: Uuid,
    pub guardian_id: Uuid,
    pub display_name: String,
    pub relationship_type: String,
    pub is_primary: bool,
    pub can_collect: bool,
    pub phone: Option<String>,
    pub email: Option<String>,
}

/// Minimum SIS-owned learner identity used by Hostel boarding workflows.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct HostelLearnerReference {
    pub id: Uuid,
    pub account_id: Option<Uuid>,
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
/// Minimum class-roster identity shared with authorised operational modules.
///
/// Consumers store only stable identifiers. Names and learner numbers are
/// resolved from SIS whenever an operational record is read.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ClassRosterEntry {
    pub enrolment_id: Uuid,
    pub learner_id: Uuid,
    pub learner_number: String,
    pub display_name: String,
}

/// Backwards-compatible name retained for Attendance callers.
pub type AttendanceRosterEntry = ClassRosterEntry;

/// Minimum SIS-owned linked account reference used by Communication.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CommunicationRecipientReference {
    pub account_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AccountCandidate {
    pub id: Uuid,
    pub full_name: String,
    pub email: String,
}
