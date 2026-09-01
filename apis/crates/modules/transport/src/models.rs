//! Private SQL projections for Transport persistence and immutable snapshots.

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub(crate) struct RouteRow {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub direction: String,
    pub status: String,
    pub notes: Option<String>,
    pub version: i32,
    pub stop_count: i64,
    pub active_rider_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct StopRow {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub stop_order: i32,
    pub planned_time: NaiveTime,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub version: i32,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct RiderRow {
    pub id: Uuid,
    pub learner_id: Uuid,
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

#[derive(Debug, Clone, FromRow)]
pub(crate) struct RunRow {
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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct RunStopRow {
    pub id: Uuid,
    pub source_stop_id: Uuid,
    pub code: String,
    pub name: String,
    pub stop_order: i32,
    pub planned_time: NaiveTime,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct ManifestRow {
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

#[derive(Debug, Clone, FromRow)]
pub(crate) struct EventRow {
    pub id: Uuid,
    pub event_type: String,
    pub manifest_entry_id: Option<Uuid>,
    pub actor_name: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}
