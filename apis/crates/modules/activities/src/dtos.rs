//! Closed HTTP and Agent-facing contracts for Activities operations.

use chrono::{DateTime, NaiveDate, Utc};
use cp_hr_payroll::models::EmployeeReference;
use cp_sis::models::ActivityLearnerReference;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivitiesScope {
    Denied,
    SelfAccount(Uuid),
    AssignedAccount(Uuid),
    SelfAndAssigned(Uuid),
    Campus,
}

impl ActivitiesScope {
    #[must_use]
    pub const fn account_id(self) -> Option<Uuid> {
        match self {
            Self::SelfAccount(id) | Self::AssignedAccount(id) | Self::SelfAndAssigned(id) => {
                Some(id)
            }
            Self::Denied | Self::Campus => None,
        }
    }

    #[must_use]
    pub const fn includes_self(self) -> bool {
        matches!(self, Self::SelfAccount(_) | Self::SelfAndAssigned(_))
    }

    #[must_use]
    pub const fn includes_assigned(self) -> bool {
        matches!(self, Self::AssignedAccount(_) | Self::SelfAndAssigned(_))
    }

    #[must_use]
    pub const fn is_campus(self) -> bool {
        matches!(self, Self::Campus)
    }

    #[must_use]
    pub const fn is_denied(self) -> bool {
        matches!(self, Self::Denied)
    }
}

macro_rules! string_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }

        impl $name {
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $value),+ }
            }
        }
    };
}

string_enum!(ActivityCategory {
    Sport => "sport",
    Club => "club",
    Arts => "arts",
    Service => "service",
    Society => "society",
    AcademicEnrichment => "academic_enrichment",
    Other => "other",
});
string_enum!(ActivityCatalogStatus { Active => "active", Archived => "archived" });
string_enum!(ActivityGroupStatus {
    Draft => "draft",
    Active => "active",
    Closed => "closed",
    Cancelled => "cancelled",
});
string_enum!(ActivityLeaderRole { Lead => "lead", Leader => "leader", Assistant => "assistant" });
string_enum!(ActivityMembershipStatus { Active => "active", Ended => "ended", Withdrawn => "withdrawn" });
string_enum!(ActivityConsentStatus {
    NotRequired => "not_required",
    Pending => "pending",
    Granted => "granted",
    Declined => "declined",
});
string_enum!(ActivitySessionStatus { Scheduled => "scheduled", Completed => "completed", Cancelled => "cancelled" });
string_enum!(ActivityParticipationMark {
    Present => "present",
    Absent => "absent",
    Late => "late",
    Excused => "excused",
    NotRequired => "not_required",
});

#[derive(Debug, Deserialize)]
pub struct ActivityReferenceQuery {
    pub search: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ActivityCatalogQuery {
    pub search: Option<String>,
    pub category: Option<ActivityCategory>,
    pub status: Option<ActivityCatalogStatus>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateActivityCatalogItemRequest {
    #[validate(length(min = 1, max = 24))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    pub category: ActivityCategory,
    #[validate(length(max = 4000))]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateActivityCatalogItemRequest {
    #[validate(length(min = 1, max = 24))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    pub category: ActivityCategory,
    #[validate(length(max = 4000))]
    pub description: Option<String>,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ArchiveActivityCatalogItemRequest {
    #[validate(length(min = 1, max = 1000))]
    pub reason: String,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityCatalogItemResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub category: String,
    pub description: Option<String>,
    pub status: String,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ActivityGroupQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub activity_id: Option<Uuid>,
    pub status: Option<ActivityGroupStatus>,
    pub active_on: Option<NaiveDate>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateActivityGroupRequest {
    pub activity_id: Uuid,
    #[validate(length(min = 1, max = 24))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    #[validate(range(min = 1, max = 100000))]
    pub capacity: Option<i32>,
    pub consent_required: bool,
    #[validate(length(max = 3000))]
    pub consent_instructions: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateActivityGroupRequest {
    pub activity_id: Uuid,
    #[validate(length(min = 1, max = 24))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    #[validate(range(min = 1, max = 100000))]
    pub capacity: Option<i32>,
    pub consent_required: bool,
    #[validate(length(max = 3000))]
    pub consent_instructions: Option<String>,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ActivityTransitionRequest {
    #[validate(length(min = 1, max = 2000))]
    pub reason: Option<String>,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityGroupSummary {
    pub id: Uuid,
    pub activity_id: Uuid,
    pub activity_code: String,
    pub activity_name: String,
    pub code: String,
    pub name: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub capacity: Option<i32>,
    pub consent_required: bool,
    pub status: String,
    pub leader_count: i64,
    pub member_count: i64,
    pub session_count: i64,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityLeaderResponse {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub employee_number: String,
    pub employee_name: String,
    pub role: String,
    pub starts_on: NaiveDate,
    pub ends_on: Option<NaiveDate>,
    pub ended_at: Option<DateTime<Utc>>,
    pub end_reason: Option<String>,
    pub version: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityMembershipResponse {
    pub id: Uuid,
    pub learner_id: Uuid,
    pub learner_number: String,
    pub learner_name: String,
    pub joined_on: NaiveDate,
    pub ended_on: Option<NaiveDate>,
    pub status: String,
    pub consent_status: String,
    pub consent_recorded_at: Option<DateTime<Utc>>,
    pub consent_notes: Option<String>,
    pub version: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityLifecycleEventResponse {
    pub id: Uuid,
    pub event_type: String,
    pub actor_name: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityGroupRecord {
    #[serde(flatten)]
    pub group: ActivityGroupSummary,
    pub consent_instructions: Option<String>,
    pub leaders: Vec<ActivityLeaderResponse>,
    pub memberships: Vec<ActivityMembershipResponse>,
    pub history: Vec<ActivityLifecycleEventResponse>,
}

#[derive(Debug, Deserialize)]
pub struct AddActivityLeaderRequest {
    pub employee_id: Uuid,
    pub role: ActivityLeaderRole,
    pub starts_on: NaiveDate,
    pub ends_on: Option<NaiveDate>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct EndActivityLeaderRequest {
    pub ends_on: NaiveDate,
    #[validate(length(min = 1, max = 1000))]
    pub reason: String,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize)]
pub struct AddActivityMembershipRequest {
    pub learner_id: Uuid,
    pub joined_on: NaiveDate,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateActivityMembershipRequest {
    pub consent_status: ActivityConsentStatus,
    #[validate(length(max = 3000))]
    pub consent_notes: Option<String>,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct EndActivityMembershipRequest {
    pub ended_on: NaiveDate,
    pub outcome: ActivityMembershipStatus,
    #[validate(length(min = 1, max = 1000))]
    pub reason: String,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize)]
pub struct ActivitySessionQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub group_id: Option<Uuid>,
    pub status: Option<ActivitySessionStatus>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateActivitySessionRequest {
    pub group_id: Uuid,
    #[validate(length(min = 1, max = 180))]
    pub title: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    #[validate(length(max = 500))]
    pub location_note: Option<String>,
    #[validate(length(max = 4000))]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateActivitySessionRequest {
    #[validate(length(min = 1, max = 180))]
    pub title: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    #[validate(length(max = 500))]
    pub location_note: Option<String>,
    #[validate(length(max = 4000))]
    pub notes: Option<String>,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivitySessionSummary {
    pub id: Uuid,
    pub reference: String,
    pub group_id: Uuid,
    pub group_code: String,
    pub group_name: String,
    pub title: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub location_note: Option<String>,
    pub status: String,
    pub roster_count: i64,
    pub marked_count: i64,
    pub present_count: i64,
    pub absent_count: i64,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityParticipationResponse {
    pub membership_id: Uuid,
    pub learner_id: Uuid,
    pub learner_number: String,
    pub learner_name: String,
    pub mark: Option<String>,
    pub notes: Option<String>,
    pub version: Option<i32>,
    pub marked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivitySessionRecord {
    #[serde(flatten)]
    pub session: ActivitySessionSummary,
    pub notes: Option<String>,
    pub completion_summary: Option<String>,
    pub cancellation_reason: Option<String>,
    pub participation: Vec<ActivityParticipationResponse>,
    pub history: Vec<ActivityLifecycleEventResponse>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct MarkActivityParticipationRequest {
    pub mark: ActivityParticipationMark,
    #[validate(length(max = 2000))]
    pub notes: Option<String>,
    #[validate(range(min = 1))]
    pub expected_version: Option<i32>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CompleteActivitySessionRequest {
    #[validate(length(min = 1, max = 3000))]
    pub summary: String,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CancelActivitySessionRequest {
    #[validate(length(min = 1, max = 2000))]
    pub reason: String,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivitiesReferenceData {
    pub learners: Vec<ActivityLearnerReference>,
    pub employees: Vec<EmployeeReference>,
}

#[derive(Debug, Serialize)]
pub struct ActivityGroupsPage {
    pub groups: Vec<ActivityGroupSummary>,
}

#[derive(Debug, Serialize)]
pub struct ActivitySessionsPage {
    pub sessions: Vec<ActivitySessionSummary>,
}
