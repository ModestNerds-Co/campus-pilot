//! Persistence projections for Assets-owned department stock requests.
//!
//! HR labels are deliberately absent: responses rehydrate current employee and
//! department identity through the typed HR boundary.

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub(crate) struct StockRequestRecord {
    pub(crate) request_number: String,
    pub(crate) requester_employee_id: Uuid,
    pub(crate) department_id: Uuid,
    pub(crate) purpose: String,
    pub(crate) status: String,
    pub(crate) version: i32,
    pub(crate) created_by: Uuid,
    pub(crate) submitted_by: Option<Uuid>,
    pub(crate) submitted_at: Option<DateTime<Utc>>,
    pub(crate) decided_by: Option<Uuid>,
    pub(crate) decided_at: Option<DateTime<Utc>>,
    pub(crate) decision_note: Option<String>,
    pub(crate) cancelled_at: Option<DateTime<Utc>>,
    pub(crate) cancellation_note: Option<String>,
    pub(crate) closed_at: Option<DateTime<Utc>>,
    pub(crate) closure_note: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct StockRequestSummaryRecord {
    pub(crate) id: Uuid,
    pub(crate) request_number: String,
    pub(crate) requester_employee_id: Uuid,
    pub(crate) department_id: Uuid,
    pub(crate) needed_by: Option<NaiveDate>,
    pub(crate) status: String,
    pub(crate) version: i32,
    pub(crate) line_count: i64,
    pub(crate) requested_quantity_minor: i64,
    pub(crate) approved_quantity_minor: i64,
    pub(crate) issued_quantity_minor: i64,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct StockRequestLineRecord {
    pub(crate) id: Uuid,
    pub(crate) line_number: i32,
    pub(crate) item_id: Uuid,
    pub(crate) item_number: String,
    pub(crate) item_name: String,
    pub(crate) unit_label: String,
    pub(crate) quantity_scale: i16,
    pub(crate) requested_quantity_minor: i64,
    pub(crate) approved_quantity_minor: Option<i64>,
    pub(crate) issued_quantity_minor: i64,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct StockRequestEventRecord {
    pub(crate) event_type: String,
    pub(crate) from_status: Option<String>,
    pub(crate) to_status: String,
    pub(crate) request_version: i32,
    pub(crate) note: Option<String>,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct StockRequestFulfilmentRecord {
    pub(crate) id: Uuid,
    pub(crate) movement_id: Uuid,
    pub(crate) movement_number: String,
    pub(crate) effective_on: NaiveDate,
    pub(crate) line_count: i64,
    pub(crate) quantity_minor: i64,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct StockRequestFulfilmentLineRecord {
    pub(crate) fulfilment_id: Uuid,
    pub(crate) request_line_id: Uuid,
    pub(crate) item_id: Uuid,
    pub(crate) item_number: String,
    pub(crate) item_name: String,
    pub(crate) store_id: Uuid,
    pub(crate) store_number: String,
    pub(crate) store_name: String,
    pub(crate) quantity_minor: i64,
    pub(crate) quantity_scale: i16,
    pub(crate) unit_label: String,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct FulfilmentLineState {
    pub(crate) item_id: Uuid,
    pub(crate) approved_quantity_minor: i64,
    pub(crate) issued_quantity_minor: i64,
}
