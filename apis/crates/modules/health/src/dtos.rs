//! Health transport contracts and closed workflow values.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatientKind {
    Learner,
    Employee,
}
impl PatientKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Learner => "learner",
            Self::Employee => "employee",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatientStatus {
    Active,
    Inactive,
}
impl PatientStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CareItemKind {
    Allergy,
    Condition,
    Accommodation,
    ActionPlan,
}
impl CareItemKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allergy => "allergy",
            Self::Condition => "condition",
            Self::Accommodation => "accommodation",
            Self::ActionPlan => "action_plan",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CareSeverity {
    Low,
    Moderate,
    High,
    Critical,
}
impl CareSeverity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Moderate => "moderate",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisitCategory {
    Illness,
    Injury,
    Medication,
    Wellbeing,
    FollowUp,
    Other,
}
impl VisitCategory {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Illness => "illness",
            Self::Injury => "injury",
            Self::Medication => "medication",
            Self::Wellbeing => "wellbeing",
            Self::FollowUp => "follow_up",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisitDisposition {
    ReturnedToClass,
    SentHome,
    EmergencyReferral,
    GuardianCollection,
    StaffReleased,
    Other,
}
impl VisitDisposition {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReturnedToClass => "returned_to_class",
            Self::SentHome => "sent_home",
            Self::EmergencyReferral => "emergency_referral",
            Self::GuardianCollection => "guardian_collection",
            Self::StaffReleased => "staff_released",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MedicationPlanStatus {
    Active,
    Suspended,
    Ended,
}
impl MedicationPlanStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Suspended => "suspended",
            Self::Ended => "ended",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MedicationOutcome {
    Given,
    Refused,
    Missed,
    Held,
}
impl MedicationOutcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Given => "given",
            Self::Refused => "refused",
            Self::Missed => "missed",
            Self::Held => "held",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FollowUpStatus {
    Open,
    Completed,
    Cancelled,
}
impl FollowUpStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct HealthListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub patient_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PatientCandidate {
    pub kind: PatientKind,
    pub id: Uuid,
    pub number: String,
    pub display_name: String,
    pub source_status: String,
    pub already_patient: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct EmployeeCandidate {
    pub id: Uuid,
    pub number: String,
    pub display_name: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct HealthReferenceData {
    pub patients: Vec<PatientCandidate>,
    pub employees: Vec<EmployeeCandidate>,
}
#[derive(Debug, Clone, Serialize)]
pub struct GuardianContact {
    pub guardian_id: Uuid,
    pub display_name: String,
    pub relationship_type: String,
    pub is_primary: bool,
    pub can_collect: bool,
    pub phone: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PatientSummary {
    pub id: Uuid,
    pub person_kind: PatientKind,
    pub person_id: Uuid,
    pub person_number: String,
    pub person_name: String,
    pub source_status: String,
    pub status: String,
    pub version: i32,
    pub active_care_item_count: i64,
    pub open_visit_count: i64,
    pub active_medication_count: i64,
    pub open_follow_up_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize)]
pub struct CareItemResponse {
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
#[derive(Debug, Clone, Serialize)]
pub struct PatientRecord {
    #[serde(flatten)]
    pub patient: PatientSummary,
    pub guardian_contacts: Vec<GuardianContact>,
    pub care_items: Vec<CareItemResponse>,
}
#[derive(Debug, Clone, Serialize)]
pub struct VisitResponse {
    pub id: Uuid,
    pub patient_id: Uuid,
    pub patient_kind: PatientKind,
    pub patient_number: String,
    pub patient_name: String,
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
#[derive(Debug, Clone, Serialize)]
pub struct MedicationPlanResponse {
    pub id: Uuid,
    pub patient_id: Uuid,
    pub patient_kind: PatientKind,
    pub patient_number: String,
    pub patient_name: String,
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
#[derive(Debug, Clone, Serialize)]
pub struct MedicationAdministrationResponse {
    pub id: Uuid,
    pub medication_plan_id: Uuid,
    pub patient_id: Uuid,
    pub patient_number: String,
    pub patient_name: String,
    pub medication_name: String,
    pub administered_at: DateTime<Utc>,
    pub dose: String,
    pub outcome: String,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize)]
pub struct FollowUpResponse {
    pub id: Uuid,
    pub patient_id: Uuid,
    pub patient_kind: PatientKind,
    pub patient_number: String,
    pub patient_name: String,
    pub visit_id: Option<Uuid>,
    pub assigned_employee_id: Option<Uuid>,
    pub assigned_employee_name: Option<String>,
    pub due_on: NaiveDate,
    pub purpose: String,
    pub status: String,
    pub outcome: Option<String>,
    pub version: i32,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PatientsPage {
    pub patients: Vec<PatientSummary>,
}
#[derive(Debug, Serialize)]
pub struct VisitsPage {
    pub visits: Vec<VisitResponse>,
}
#[derive(Debug, Serialize)]
pub struct MedicationPlansPage {
    pub medication_plans: Vec<MedicationPlanResponse>,
}
#[derive(Debug, Serialize)]
pub struct MedicationAdministrationsPage {
    pub administrations: Vec<MedicationAdministrationResponse>,
}
#[derive(Debug, Serialize)]
pub struct FollowUpsPage {
    pub follow_ups: Vec<FollowUpResponse>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreatePatientRequest {
    pub person_kind: PatientKind,
    pub person_id: Uuid,
}
#[derive(Debug, Deserialize, Validate)]
pub struct UpdatePatientRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    pub status: PatientStatus,
}
#[derive(Debug, Deserialize, Validate)]
pub struct CreateCareItemRequest {
    pub kind: CareItemKind,
    #[validate(length(min = 1, max = 160))]
    pub title: String,
    #[validate(length(max = 4000))]
    pub details: Option<String>,
    pub severity: CareSeverity,
    pub reviewed_on: Option<NaiveDate>,
}
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateCareItemRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    pub kind: CareItemKind,
    #[validate(length(min = 1, max = 160))]
    pub title: String,
    #[validate(length(max = 4000))]
    pub details: Option<String>,
    pub severity: CareSeverity,
    pub reviewed_on: Option<NaiveDate>,
    pub status: String,
}
#[derive(Debug, Deserialize, Validate)]
pub struct CreateVisitRequest {
    pub patient_id: Uuid,
    pub checked_in_at: DateTime<Utc>,
    pub category: VisitCategory,
    #[validate(length(min = 1, max = 2000))]
    pub presenting_concern: String,
    #[validate(length(max = 4000))]
    pub assessment: Option<String>,
    #[validate(length(max = 4000))]
    pub care_given: Option<String>,
}
#[derive(Debug, Deserialize, Validate)]
pub struct CloseVisitRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    pub disposition: VisitDisposition,
    #[validate(length(max = 4000))]
    pub assessment: Option<String>,
    #[validate(length(max = 4000))]
    pub care_given: Option<String>,
}
#[derive(Debug, Deserialize, Validate)]
pub struct CreateMedicationPlanRequest {
    pub patient_id: Uuid,
    #[validate(length(min = 1, max = 200))]
    pub medication_name: String,
    #[validate(length(min = 1, max = 160))]
    pub dosage: String,
    #[validate(length(min = 1, max = 80))]
    pub route: String,
    #[validate(length(min = 1, max = 300))]
    pub schedule: String,
    #[validate(length(max = 2000))]
    pub instructions: Option<String>,
    #[validate(length(min = 1, max = 300))]
    pub authorization_reference: String,
    pub starts_on: NaiveDate,
    pub ends_on: Option<NaiveDate>,
}
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateMedicationPlanRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 200))]
    pub medication_name: String,
    #[validate(length(min = 1, max = 160))]
    pub dosage: String,
    #[validate(length(min = 1, max = 80))]
    pub route: String,
    #[validate(length(min = 1, max = 300))]
    pub schedule: String,
    #[validate(length(max = 2000))]
    pub instructions: Option<String>,
    #[validate(length(min = 1, max = 300))]
    pub authorization_reference: String,
    pub starts_on: NaiveDate,
    pub ends_on: Option<NaiveDate>,
    pub status: MedicationPlanStatus,
}
#[derive(Debug, Deserialize, Validate)]
pub struct RecordMedicationAdministrationRequest {
    pub administered_at: DateTime<Utc>,
    #[validate(length(min = 1, max = 160))]
    pub dose: String,
    pub outcome: MedicationOutcome,
    #[validate(length(max = 2000))]
    pub note: Option<String>,
}
#[derive(Debug, Deserialize, Validate)]
pub struct CreateFollowUpRequest {
    pub patient_id: Uuid,
    pub visit_id: Option<Uuid>,
    pub assigned_employee_id: Option<Uuid>,
    pub due_on: NaiveDate,
    #[validate(length(min = 1, max = 1000))]
    pub purpose: String,
}
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateFollowUpRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    pub assigned_employee_id: Option<Uuid>,
    pub due_on: NaiveDate,
    #[validate(length(min = 1, max = 1000))]
    pub purpose: String,
    pub status: FollowUpStatus,
    #[validate(length(max = 2000))]
    pub outcome: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum HealthAccessScope {
    Campus,
    SelfFor(Uuid),
}
