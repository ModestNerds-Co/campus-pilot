//! Closed API and Agent-facing contracts for school transport operations.

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use cp_fleet::models::{TransportDriverReference, TransportVehicleReference};
use cp_sis::models::TransportLearnerReference;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteDirection {
    Inbound,
    Outbound,
}

impl RouteDirection {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inbound => "inbound",
            Self::Outbound => "outbound",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteStatus {
    Active,
    Inactive,
}

impl RouteStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiderStatus {
    Active,
    Ended,
    Cancelled,
}

impl RiderStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Ended => "ended",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Draft,
    Boarding,
    Departed,
    Completed,
    Cancelled,
}

impl RunStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Boarding => "boarding",
            Self::Departed => "departed",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManifestStatus {
    Expected,
    Boarded,
    NoShow,
    Exception,
}

impl ManifestStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Expected => "expected",
            Self::Boarded => "boarded",
            Self::NoShow => "no_show",
            Self::Exception => "exception",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManifestExceptionKind {
    NotAtStop,
    Illness,
    TransportChange,
    Conduct,
    Safety,
    Other,
}

impl ManifestExceptionKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotAtStop => "not_at_stop",
            Self::Illness => "illness",
            Self::TransportChange => "transport_change",
            Self::Conduct => "conduct",
            Self::Safety => "safety",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ReferenceQuery {
    pub search: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListRoutesQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<RouteStatus>,
    pub direction: Option<RouteDirection>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateRouteRequest {
    #[validate(length(min = 1, max = 24))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    pub direction: RouteDirection,
    #[validate(length(max = 2000))]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateRouteRequest {
    #[validate(length(min = 1, max = 24))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    pub direction: RouteDirection,
    pub status: RouteStatus,
    #[validate(length(max = 2000))]
    pub notes: Option<String>,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateStopRequest {
    #[validate(length(min = 1, max = 24))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    #[validate(range(min = 1))]
    pub stop_order: i32,
    pub planned_time: NaiveTime,
    #[validate(range(min = -90.0, max = 90.0))]
    pub latitude: Option<f64>,
    #[validate(range(min = -180.0, max = 180.0))]
    pub longitude: Option<f64>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateStopRequest {
    #[validate(length(min = 1, max = 24))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    #[validate(range(min = 1))]
    pub stop_order: i32,
    pub planned_time: NaiveTime,
    #[validate(range(min = -90.0, max = 90.0))]
    pub latitude: Option<f64>,
    #[validate(range(min = -180.0, max = 180.0))]
    pub longitude: Option<f64>,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct RemoveStopRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize)]
pub struct ListRidersQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub route_id: Option<Uuid>,
    pub status: Option<RiderStatus>,
    pub on_date: Option<NaiveDate>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateRiderAssignmentRequest {
    pub learner_id: Uuid,
    pub route_id: Uuid,
    pub boarding_stop_id: Uuid,
    pub alighting_stop_id: Uuid,
    pub effective_from: NaiveDate,
    pub effective_until: Option<NaiveDate>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct EndRiderAssignmentRequest {
    pub effective_until: NaiveDate,
    #[validate(length(min = 1, max = 1000))]
    pub reason: String,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize)]
pub struct ListRunsQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub route_id: Option<Uuid>,
    pub status: Option<RunStatus>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRunRequest {
    pub route_id: Uuid,
    pub service_date: NaiveDate,
    pub vehicle_id: Uuid,
    pub driver_id: Uuid,
}

#[derive(Debug, Deserialize, Validate)]
pub struct RunTransitionRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CancelRunRequest {
    #[validate(length(min = 1, max = 1000))]
    pub reason: String,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct MarkManifestEntryRequest {
    pub status: ManifestStatus,
    pub exception_kind: Option<ManifestExceptionKind>,
    #[validate(length(max = 1000))]
    pub note: Option<String>,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteStopResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub stop_order: i32,
    pub planned_time: NaiveTime,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub version: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteSummaryResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub direction: String,
    pub status: String,
    pub notes: Option<String>,
    pub version: i32,
    pub stop_count: i64,
    pub active_rider_count: i64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteRecordResponse {
    #[serde(flatten)]
    pub route: RouteSummaryResponse,
    pub stops: Vec<RouteStopResponse>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct RoutesPage {
    pub routes: Vec<RouteSummaryResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RiderAssignmentResponse {
    pub id: Uuid,
    pub learner_id: Uuid,
    pub learner_number: String,
    pub learner_name: String,
    pub route_id: Uuid,
    pub route_code: String,
    pub route_name: String,
    pub direction: String,
    pub boarding_stop_id: Uuid,
    pub boarding_stop_name: String,
    pub alighting_stop_id: Uuid,
    pub alighting_stop_name: String,
    pub effective_from: NaiveDate,
    pub effective_until: Option<NaiveDate>,
    pub status: String,
    pub version: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct RidersPage {
    pub riders: Vec<RiderAssignmentResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunStopResponse {
    pub id: Uuid,
    pub source_stop_id: Uuid,
    pub code: String,
    pub name: String,
    pub stop_order: i32,
    pub planned_time: NaiveTime,
}

#[derive(Debug, Clone, Serialize)]
pub struct ManifestEntryResponse {
    pub id: Uuid,
    pub learner_id: Uuid,
    pub learner_number: String,
    pub learner_name: String,
    pub boarding_run_stop_id: Uuid,
    pub boarding_stop_name: String,
    pub alighting_run_stop_id: Uuid,
    pub alighting_stop_name: String,
    pub status: String,
    pub exception_kind: Option<String>,
    pub note: Option<String>,
    pub marked_at: Option<DateTime<Utc>>,
    pub version: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunEventResponse {
    pub id: Uuid,
    pub event_type: String,
    pub manifest_entry_id: Option<Uuid>,
    pub actor_name: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunSummaryResponse {
    pub id: Uuid,
    pub reference: String,
    pub route_id: Uuid,
    pub route_code: String,
    pub route_name: String,
    pub direction: String,
    pub service_date: NaiveDate,
    pub vehicle_id: Uuid,
    pub vehicle_registration: String,
    pub driver_id: Uuid,
    pub driver_name: String,
    pub capacity: i32,
    pub status: String,
    pub expected_count: i64,
    pub boarded_count: i64,
    pub exception_count: i64,
    pub version: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunRecordResponse {
    #[serde(flatten)]
    pub run: RunSummaryResponse,
    pub stops: Vec<RunStopResponse>,
    pub manifest: Vec<ManifestEntryResponse>,
    pub history: Vec<RunEventResponse>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct RunsPage {
    pub runs: Vec<RunSummaryResponse>,
}

#[derive(Debug, Serialize)]
pub struct TransportReferenceData {
    pub learners: Vec<TransportLearnerReference>,
    pub vehicles: Vec<TransportVehicleReference>,
    pub drivers: Vec<TransportDriverReference>,
    pub routes: Vec<RouteRecordResponse>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_transport_values_have_stable_wire_names() {
        assert_eq!(RouteDirection::Inbound.as_str(), "inbound");
        assert_eq!(RouteDirection::Outbound.as_str(), "outbound");
        assert_eq!(RouteStatus::Active.as_str(), "active");
        assert_eq!(RouteStatus::Inactive.as_str(), "inactive");
        assert_eq!(RiderStatus::Active.as_str(), "active");
        assert_eq!(RiderStatus::Ended.as_str(), "ended");
        assert_eq!(RiderStatus::Cancelled.as_str(), "cancelled");
        assert_eq!(RunStatus::Draft.as_str(), "draft");
        assert_eq!(RunStatus::Boarding.as_str(), "boarding");
        assert_eq!(RunStatus::Departed.as_str(), "departed");
        assert_eq!(RunStatus::Completed.as_str(), "completed");
        assert_eq!(RunStatus::Cancelled.as_str(), "cancelled");
        assert_eq!(ManifestStatus::Expected.as_str(), "expected");
        assert_eq!(ManifestStatus::Boarded.as_str(), "boarded");
        assert_eq!(ManifestStatus::NoShow.as_str(), "no_show");
        assert_eq!(ManifestStatus::Exception.as_str(), "exception");
        assert_eq!(ManifestExceptionKind::NotAtStop.as_str(), "not_at_stop");
        assert_eq!(ManifestExceptionKind::Illness.as_str(), "illness");
        assert_eq!(
            ManifestExceptionKind::TransportChange.as_str(),
            "transport_change"
        );
        assert_eq!(ManifestExceptionKind::Conduct.as_str(), "conduct");
        assert_eq!(ManifestExceptionKind::Safety.as_str(), "safety");
        assert_eq!(ManifestExceptionKind::Other.as_str(), "other");
    }
}
