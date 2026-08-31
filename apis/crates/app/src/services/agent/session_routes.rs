//! Exposes owner-scoped durable Agent Sessions, messages, runs, and events.
//!
//! The route tree must be mounted under `AuthMiddleware` and
//! `RequirePermission("agent")`. Exact operation-catalog descriptors then
//! enforce the licensed Agent module and `agent:view`, `agent:history`, or
//! `agent:run` permission for each route.

use std::str::FromStr;

use actix_web::{
    HttpResponse, get,
    http::{StatusCode, header},
    patch, post,
    web::{self, ServiceConfig},
};
use cp_agent_runtime::{
    AgentSessionError, ArchiveSessionCommand, CreateSessionCommand, ListEventsQuery,
    ListMessagesQuery, ListRunsQuery, ListSessionsQuery, MessageCursor, RenameSessionCommand,
    RunCursor, SessionCursor, SubmitMessageCommand, TaskClass,
};
use cp_audit::RequestContext;
use cp_common::{AccessContext, ApiResponse, TenantId};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{services::auth::models::User, state::AppState};

use super::origin::{AttestedAgentOrigin, OriginAttestationError};
use super::session_dtos::{
    ArchiveSessionRequest, CreateSessionRequest, ListEventsRequest, ListMessagesRequest,
    ListRunsRequest, ListSessionsRequest, RenameSessionRequest, SubmitMessageRequest,
};

#[get("/sessions")]
async fn list_sessions(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    user: web::ReqData<User>,
    query: web::Query<ListSessionsRequest>,
) -> HttpResponse {
    let query = match list_sessions_query(query.into_inner()) {
        Ok(query) => query,
        Err(error) => return session_error(error),
    };
    respond(
        state
            .agent_session_ops
            .list_sessions(tenant.into_inner().0, user.id, query)
            .await,
        StatusCode::OK,
    )
}

#[post("/sessions")]
async fn create_session(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    user: web::ReqData<User>,
    request_context: web::ReqData<RequestContext>,
    body: web::Json<CreateSessionRequest>,
) -> HttpResponse {
    let request = body.into_inner();
    let command =
        match CreateSessionCommand::parse(request.title.as_deref(), request.idempotency_key) {
            Ok(command) => command,
            Err(error) => return session_error(error),
        };
    respond(
        state
            .agent_session_ops
            .create_session(
                tenant.into_inner().0,
                user.id,
                request_context.into_inner(),
                command,
            )
            .await,
        StatusCode::CREATED,
    )
}

#[get("/sessions/{session_id}")]
async fn read_session(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    user: web::ReqData<User>,
    session_id: web::Path<Uuid>,
) -> HttpResponse {
    respond(
        state
            .agent_session_ops
            .read_session(tenant.into_inner().0, user.id, session_id.into_inner())
            .await,
        StatusCode::OK,
    )
}

#[patch("/sessions/{session_id}")]
async fn rename_session(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    user: web::ReqData<User>,
    request_context: web::ReqData<RequestContext>,
    session_id: web::Path<Uuid>,
    body: web::Json<RenameSessionRequest>,
) -> HttpResponse {
    let request = body.into_inner();
    let command = match RenameSessionCommand::parse(&request.title, request.expected_version) {
        Ok(command) => command,
        Err(error) => return session_error(error),
    };
    respond(
        state
            .agent_session_ops
            .rename_session(
                tenant.into_inner().0,
                user.id,
                session_id.into_inner(),
                request_context.into_inner(),
                command,
            )
            .await,
        StatusCode::OK,
    )
}

#[post("/sessions/{session_id}/archive")]
async fn archive_session(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    user: web::ReqData<User>,
    request_context: web::ReqData<RequestContext>,
    session_id: web::Path<Uuid>,
    body: web::Json<ArchiveSessionRequest>,
) -> HttpResponse {
    let command = match ArchiveSessionCommand::parse(body.expected_version) {
        Ok(command) => command,
        Err(error) => return session_error(error),
    };
    respond(
        state
            .agent_session_ops
            .archive_session(
                tenant.into_inner().0,
                user.id,
                session_id.into_inner(),
                request_context.into_inner(),
                command,
            )
            .await,
        StatusCode::OK,
    )
}

#[get("/sessions/{session_id}/messages")]
async fn list_messages(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    user: web::ReqData<User>,
    session_id: web::Path<Uuid>,
    query: web::Query<ListMessagesRequest>,
) -> HttpResponse {
    let query = match list_messages_query(query.into_inner()) {
        Ok(query) => query,
        Err(error) => return session_error(error),
    };
    respond(
        state
            .agent_session_ops
            .list_messages(
                tenant.into_inner().0,
                user.id,
                session_id.into_inner(),
                query,
            )
            .await,
        StatusCode::OK,
    )
}

#[post("/sessions/{session_id}/messages")]
async fn submit_message(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    user: web::ReqData<User>,
    access: web::ReqData<AccessContext>,
    request_context: web::ReqData<RequestContext>,
    session_id: web::Path<Uuid>,
    body: web::Json<SubmitMessageRequest>,
) -> HttpResponse {
    let request = body.into_inner();
    let origin = match AttestedAgentOrigin::parse(
        &request.origin_module_key,
        &request.origin_route,
        &access,
    ) {
        Ok(origin) => origin,
        Err(error) => return origin_error(error),
    };
    let command = match submit_message_command(request, origin) {
        Ok(command) => command,
        Err(error) => return session_error(error),
    };
    if !state
        .agent_submission_gate
        .is_ready()
        .await
        .unwrap_or(false)
    {
        return HttpResponse::ServiceUnavailable()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(ApiResponse::<Value>::from_status(
                StatusCode::SERVICE_UNAVAILABLE,
                Some(json!({ "code": "agent_worker_unavailable" })),
                Some(vec!["Agent execution is not available".to_owned()]),
            ));
    }
    respond(
        state
            .agent_session_ops
            .submit_message(
                tenant.into_inner().0,
                user.id,
                session_id.into_inner(),
                request_context.into_inner(),
                command,
            )
            .await,
        StatusCode::ACCEPTED,
    )
}

#[get("/sessions/{session_id}/runs")]
async fn list_runs(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    user: web::ReqData<User>,
    session_id: web::Path<Uuid>,
    query: web::Query<ListRunsRequest>,
) -> HttpResponse {
    let query = match list_runs_query(query.into_inner()) {
        Ok(query) => query,
        Err(error) => return session_error(error),
    };
    respond(
        state
            .agent_session_ops
            .list_runs(
                tenant.into_inner().0,
                user.id,
                session_id.into_inner(),
                query,
            )
            .await,
        StatusCode::OK,
    )
}

#[get("/runs/{run_id}")]
async fn read_run(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    user: web::ReqData<User>,
    run_id: web::Path<Uuid>,
) -> HttpResponse {
    respond(
        state
            .agent_session_ops
            .read_run(tenant.into_inner().0, user.id, run_id.into_inner())
            .await,
        StatusCode::OK,
    )
}

#[post("/runs/{run_id}/cancel")]
async fn cancel_run(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    user: web::ReqData<User>,
    request_context: web::ReqData<RequestContext>,
    run_id: web::Path<Uuid>,
) -> HttpResponse {
    respond(
        state
            .agent_session_ops
            .cancel_run(
                tenant.into_inner().0,
                user.id,
                run_id.into_inner(),
                request_context.into_inner(),
            )
            .await,
        StatusCode::OK,
    )
}

#[get("/runs/{run_id}/events")]
async fn list_events(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    user: web::ReqData<User>,
    run_id: web::Path<Uuid>,
    query: web::Query<ListEventsRequest>,
) -> HttpResponse {
    let query = match ListEventsQuery::parse(query.limit, query.after.as_deref()) {
        Ok(query) => query,
        Err(error) => return session_error(error),
    };
    respond(
        state
            .agent_session_ops
            .list_events(tenant.into_inner().0, user.id, run_id.into_inner(), query)
            .await,
        StatusCode::OK,
    )
}

fn list_sessions_query(
    request: ListSessionsRequest,
) -> Result<ListSessionsQuery, AgentSessionError> {
    let cursor = paired_cursor(
        request.cursor_last_activity_at,
        request.cursor_session_id,
        |last_activity_at, session_id| SessionCursor {
            last_activity_at,
            session_id,
        },
        "invalid_session_cursor",
    )?;
    ListSessionsQuery::parse(
        request.limit,
        cursor,
        request.title_search.as_deref(),
        request.include_archived,
    )
}

fn list_messages_query(
    request: ListMessagesRequest,
) -> Result<ListMessagesQuery, AgentSessionError> {
    let cursor = paired_cursor(
        request.cursor_sequence,
        request.cursor_message_id,
        MessageCursor::parse,
        "invalid_message_cursor",
    )?
    .transpose()?;
    ListMessagesQuery::parse(request.limit, cursor)
}

fn list_runs_query(request: ListRunsRequest) -> Result<ListRunsQuery, AgentSessionError> {
    let cursor = paired_cursor(
        request.cursor_created_at,
        request.cursor_run_id,
        |created_at, run_id| RunCursor { created_at, run_id },
        "invalid_run_cursor",
    )?;
    ListRunsQuery::parse(request.limit, cursor)
}

fn submit_message_command(
    request: SubmitMessageRequest,
    origin: AttestedAgentOrigin,
) -> Result<SubmitMessageCommand, AgentSessionError> {
    let task_class = TaskClass::from_str(&request.task_class).map_err(|_| {
        AgentSessionError::invalid("invalid_task_class", "Choose a supported Agent task class")
    })?;
    SubmitMessageCommand::parse(
        &request.content,
        task_class,
        origin.module_key(),
        origin.route(),
        request.idempotency_key,
    )
}

fn origin_error(error: OriginAttestationError) -> HttpResponse {
    let status = match error {
        OriginAttestationError::UnknownRoute | OriginAttestationError::ModuleMismatch => {
            StatusCode::BAD_REQUEST
        }
        OriginAttestationError::AccessDenied => StatusCode::FORBIDDEN,
    };
    HttpResponse::build(status)
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(ApiResponse::<Value>::from_status(
            status,
            Some(json!({ "code": error.code() })),
            Some(vec![error.safe_message().to_owned()]),
        ))
}

fn paired_cursor<A, B, T>(
    left: Option<A>,
    right: Option<B>,
    build: impl FnOnce(A, B) -> T,
    code: &'static str,
) -> Result<Option<T>, AgentSessionError> {
    match (left, right) {
        (None, None) => Ok(None),
        (Some(left), Some(right)) => Ok(Some(build(left, right))),
        _ => Err(AgentSessionError::invalid(
            code,
            "Provide every cursor field together",
        )),
    }
}

fn respond<T: Serialize>(result: Result<T, AgentSessionError>, status: StatusCode) -> HttpResponse {
    match result {
        Ok(value) => HttpResponse::build(status)
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(ApiResponse::from_status(status, Some(value), None)),
        Err(error) => session_error(error),
    }
}

fn session_error(error: AgentSessionError) -> HttpResponse {
    let status = match &error {
        AgentSessionError::InvalidInput { .. } => StatusCode::BAD_REQUEST,
        AgentSessionError::SessionNotFound | AgentSessionError::RunNotFound => {
            StatusCode::NOT_FOUND
        }
        AgentSessionError::Conflict { .. } => StatusCode::CONFLICT,
        AgentSessionError::LeaseLost | AgentSessionError::Storage(_) => {
            StatusCode::SERVICE_UNAVAILABLE
        }
    };
    if matches!(error, AgentSessionError::Storage(_)) {
        log::error!("Agent Session operation failed: {error}");
    }
    HttpResponse::build(status)
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(ApiResponse::<Value>::from_status(
            status,
            Some(json!({ "code": error.code() })),
            Some(vec![error.safe_message()]),
        ))
}

/// Mounts only genuine owner APIs. Sharing, approval, and administration
/// endpoints remain absent until their durable services exist.
pub fn routes(cfg: &mut ServiceConfig) {
    cfg.service(list_sessions)
        .service(create_session)
        .service(read_session)
        .service(rename_session)
        .service(archive_session)
        .service(list_messages)
        .service(submit_message)
        .service(list_runs)
        .service(read_run)
        .service(cancel_run)
        .service(list_events);
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use cp_common::{AccessContext, EntitlementSnapshot, LeaseLifecycle, ModuleEntitlementState};
    use uuid::Uuid;

    use super::{
        ListMessagesRequest, ListRunsRequest, ListSessionsRequest, list_messages_query,
        list_runs_query, list_sessions_query, submit_message_command,
    };
    use crate::services::agent::origin::AttestedAgentOrigin;
    use crate::services::agent::session_dtos::SubmitMessageRequest;

    fn fleet_access() -> AccessContext {
        AccessContext {
            role_keys: vec!["test-role".to_owned()],
            permissions: vec!["fleet:view".to_owned()],
            enabled_modules: vec!["fleet".to_owned()],
            entitlements: EntitlementSnapshot::new(
                LeaseLifecycle::Active,
                [("fleet".to_owned(), ModuleEntitlementState::Enabled)],
                [],
            )
            .unwrap(),
        }
    }

    #[test]
    fn cursor_shapes_require_all_fields() {
        assert!(
            list_sessions_query(ListSessionsRequest {
                limit: None,
                cursor_last_activity_at: Some(Utc::now()),
                cursor_session_id: None,
                title_search: None,
                include_archived: false,
            })
            .is_err()
        );
        assert!(
            list_messages_query(ListMessagesRequest {
                limit: None,
                cursor_sequence: Some(1),
                cursor_message_id: Some(Uuid::new_v4()),
            })
            .is_ok()
        );
        assert!(
            list_runs_query(ListRunsRequest {
                limit: Some(100),
                cursor_created_at: Some(Utc::now()),
                cursor_run_id: Some(Uuid::new_v4()),
            })
            .is_ok()
        );
    }

    #[test]
    fn submission_parses_supported_task_and_rejects_unknown_task() {
        let request = |task_class: &str| SubmitMessageRequest {
            content: "Show the current fleet vehicles".to_owned(),
            task_class: task_class.to_owned(),
            origin_module_key: "fleet".to_owned(),
            origin_route: "/modules/fleet".to_owned(),
            idempotency_key: "agent-submit-0001".to_owned(),
        };
        let access = fleet_access();
        let origin = || AttestedAgentOrigin::parse("fleet", "/modules/fleet", &access).unwrap();
        assert!(submit_message_command(request("module_read_reporting"), origin()).is_ok());
        assert!(submit_message_command(request("unbounded"), origin()).is_err());
    }
}
