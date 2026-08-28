//! Defines wire-only requests for Agent provider/model routing.
//!
//! Routes parse these untrusted values into `cp-agent-runtime` commands before
//! storage access; target priority is derived from the bounded array order.

use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RouteTargetRequest {
    pub connection_id: Uuid,
    pub provider_model_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateRouteRequest {
    pub scope_kind: String,
    pub task_class: Option<String>,
    pub module_key: Option<String>,
    pub operation_class: Option<String>,
    pub capability_key: Option<String>,
    pub capability_version: Option<i32>,
    pub requires_tools: bool,
    pub targets: Vec<RouteTargetRequest>,
    pub audit_reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReplaceRouteRequest {
    pub expected_version: i64,
    pub requires_tools: bool,
    pub targets: Vec<RouteTargetRequest>,
    pub audit_reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ArchiveRouteRequest {
    pub expected_version: i64,
    pub audit_reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ResolveRouteRequest {
    pub task_class: String,
    pub module_key: Option<String>,
    pub operation_class: Option<String>,
    pub capability_key: Option<String>,
    pub capability_version: Option<i32>,
    pub requires_tools: bool,
}
