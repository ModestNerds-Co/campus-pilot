//! Hostel transport contracts and closed boarding lifecycle values.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResidenceStatus {
    Active,
    Inactive,
}
impl ResidenceStatus {
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
pub enum RoomStatus {
    Available,
    Maintenance,
    Inactive,
}
impl RoomStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Maintenance => "maintenance",
            Self::Inactive => "inactive",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PastoralCategory {
    Wellbeing,
    Behaviour,
    Safeguarding,
    FamilyContact,
    Other,
}
impl PastoralCategory {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Wellbeing => "wellbeing",
            Self::Behaviour => "behaviour",
            Self::Safeguarding => "safeguarding",
            Self::FamilyContact => "family_contact",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PastoralSeverity {
    Low,
    Moderate,
    High,
    Critical,
}
impl PastoralSeverity {
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

#[derive(Debug, Deserialize)]
pub struct HostelListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub residence_id: Option<Uuid>,
    pub room_id: Option<Uuid>,
    pub learner_id: Option<Uuid>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum HostelAccessScope {
    Campus,
    SelfFor(Uuid),
}

#[derive(Debug, Clone, Serialize)]
pub struct HostelLearnerCandidate {
    pub id: Uuid,
    pub learner_number: String,
    pub display_name: String,
    pub status: String,
    pub has_current_allocation: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostelReferenceData {
    pub learners: Vec<HostelLearnerCandidate>,
    pub rooms: Vec<RoomResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResidenceResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub version: i32,
    pub room_count: i64,
    pub bed_capacity: i64,
    pub occupied_count: i64,
    pub available_beds: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoomResponse {
    pub id: Uuid,
    pub residence_id: Uuid,
    pub residence_code: String,
    pub residence_name: String,
    pub code: String,
    pub floor_label: Option<String>,
    pub capacity: i16,
    pub occupied_count: i64,
    pub available_beds: i64,
    pub status: String,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AllocationResponse {
    pub id: Uuid,
    pub learner_id: Uuid,
    pub learner_number: String,
    pub learner_name: String,
    pub learner_status: String,
    pub room_id: Uuid,
    pub room_code: String,
    pub residence_id: Uuid,
    pub residence_code: String,
    pub residence_name: String,
    pub starts_on: NaiveDate,
    pub expected_end_on: Option<NaiveDate>,
    pub ended_on: Option<NaiveDate>,
    pub status: String,
    pub version: i32,
    pub previous_allocation_id: Option<Uuid>,
    pub decision_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PastoralRecordResponse {
    pub id: Uuid,
    pub learner_id: Uuid,
    pub learner_number: String,
    pub learner_name: String,
    pub allocation_id: Option<Uuid>,
    pub residence_name: Option<String>,
    pub room_code: Option<String>,
    pub category: String,
    pub severity: String,
    pub subject: String,
    pub details: String,
    pub occurred_at: DateTime<Utc>,
    pub status: String,
    pub resolution: Option<String>,
    pub version: i32,
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AllocationPreviewResponse {
    pub learner_id: Uuid,
    pub learner_number: String,
    pub learner_name: String,
    pub room_id: Uuid,
    pub room_code: String,
    pub residence_name: String,
    pub room_version: i32,
    pub capacity: i16,
    pub occupied_count: i64,
    pub available_beds: i64,
    pub starts_on: NaiveDate,
    pub expected_end_on: Option<NaiveDate>,
    pub can_allocate: bool,
    pub issues: Vec<String>,
    pub fingerprint: String,
}

#[derive(Debug, Serialize)]
pub struct ResidencesPage {
    pub residences: Vec<ResidenceResponse>,
}
#[derive(Debug, Serialize)]
pub struct RoomsPage {
    pub rooms: Vec<RoomResponse>,
}
#[derive(Debug, Serialize)]
pub struct AllocationsPage {
    pub allocations: Vec<AllocationResponse>,
}
#[derive(Debug, Serialize)]
pub struct PastoralRecordsPage {
    pub pastoral_records: Vec<PastoralRecordResponse>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateResidenceRequest {
    #[validate(length(min = 1, max = 30))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    #[validate(length(max = 1000))]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateResidenceRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 30))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    #[validate(length(max = 1000))]
    pub description: Option<String>,
    pub status: ResidenceStatus,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateRoomRequest {
    pub residence_id: Uuid,
    #[validate(length(min = 1, max = 40))]
    pub code: String,
    #[validate(length(max = 80))]
    pub floor_label: Option<String>,
    #[validate(range(min = 1, max = 50))]
    pub capacity: i16,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateRoomRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 40))]
    pub code: String,
    #[validate(length(max = 80))]
    pub floor_label: Option<String>,
    #[validate(range(min = 1, max = 50))]
    pub capacity: i16,
    pub status: RoomStatus,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AllocationPreviewRequest {
    pub learner_id: Uuid,
    pub room_id: Uuid,
    pub starts_on: NaiveDate,
    pub expected_end_on: Option<NaiveDate>,
    pub replacing_allocation_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateAllocationRequest {
    pub learner_id: Uuid,
    pub room_id: Uuid,
    pub starts_on: NaiveDate,
    pub expected_end_on: Option<NaiveDate>,
    #[validate(length(min = 64, max = 64))]
    pub preview_fingerprint: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ActivateAllocationRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct EndAllocationRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    pub ended_on: NaiveDate,
    #[validate(length(min = 1, max = 1000))]
    pub reason: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CancelAllocationRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 1000))]
    pub reason: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct TransferAllocationPreviewRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    pub new_room_id: Uuid,
    pub effective_on: NaiveDate,
}

#[derive(Debug, Deserialize, Validate)]
pub struct TransferAllocationRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    pub new_room_id: Uuid,
    pub effective_on: NaiveDate,
    #[validate(length(min = 1, max = 1000))]
    pub reason: String,
    #[validate(length(min = 64, max = 64))]
    pub preview_fingerprint: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreatePastoralRecordRequest {
    pub learner_id: Uuid,
    pub allocation_id: Option<Uuid>,
    pub category: PastoralCategory,
    pub severity: PastoralSeverity,
    #[validate(length(min = 1, max = 200))]
    pub subject: String,
    #[validate(length(min = 1, max = 6000))]
    pub details: String,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdatePastoralRecordRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    pub category: PastoralCategory,
    pub severity: PastoralSeverity,
    #[validate(length(min = 1, max = 200))]
    pub subject: String,
    #[validate(length(min = 1, max = 6000))]
    pub details: String,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ResolvePastoralRecordRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 4000))]
    pub resolution: String,
}
