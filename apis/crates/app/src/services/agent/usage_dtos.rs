//! Defines untrusted query values for personal Agent usage history.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PersonalUsageRequest {
    pub meter: Option<String>,
    pub currency: Option<String>,
    pub currency_exponent: Option<u8>,
    pub pricing_version: Option<String>,
    pub cursor_occurred_at: Option<DateTime<Utc>>,
    pub cursor_event_id: Option<Uuid>,
    pub cursor_meter: Option<String>,
    pub limit: Option<u16>,
}
