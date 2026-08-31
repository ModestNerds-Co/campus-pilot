//! Defines untrusted wire shapes for the owner-scoped Agent Session API.
//!
//! Every value is parsed into a refined `cp-agent-runtime` command before a
//! database operation. Tenant, person, correlation, and run identities never
//! come from these request bodies.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListSessionsRequest {
    pub limit: Option<u16>,
    pub cursor_last_activity_at: Option<DateTime<Utc>>,
    pub cursor_session_id: Option<Uuid>,
    pub title_search: Option<String>,
    #[serde(default)]
    pub include_archived: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateSessionRequest {
    pub title: Option<String>,
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RenameSessionRequest {
    pub title: String,
    pub expected_version: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ArchiveSessionRequest {
    pub expected_version: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListMessagesRequest {
    pub limit: Option<u16>,
    pub cursor_sequence: Option<i64>,
    pub cursor_message_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SubmitMessageRequest {
    pub content: String,
    pub task_class: String,
    pub origin_module_key: String,
    pub origin_route: String,
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListRunsRequest {
    pub limit: Option<u16>,
    pub cursor_created_at: Option<DateTime<Utc>>,
    pub cursor_run_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListEventsRequest {
    pub limit: Option<u16>,
    pub after: Option<String>,
}
