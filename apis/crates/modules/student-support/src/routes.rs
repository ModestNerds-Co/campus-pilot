//! Authenticated, licensed, permission-authoritative Student Support routes.

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, get, post, put, web};
use cp_audit::{AuditActor, RequestContext};
use cp_common::{
    AccessContext, ApiResponse, EffectiveRecordScope, PaginationMeta, RecordScopeFamilyKey,
    RecordScopeGrants, RequirePermission, TenantId, flatten_validation_errors,
};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;

use crate::{
    AssignTeamMemberRequest, CaseActionsPage, CaseTransitionRequest, CasesPage,
    CreateCaseActionRequest, CreateCaseRequest, ReferenceQuery, RemoveTeamMemberRequest,
    StudentSupportAccessScope, StudentSupportListQuery, StudentSupportOps, UpdateCaseRequest,
};

type Authority = (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>);

#[get("/references")]
async fn reference_data(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    query: web::Query<ReferenceQuery>,
) -> HttpResponse {
    if access_scope(&authority.0, &authority.1, *actor).is_none() {
        return forbidden();
    }
    let include_case_workers =
        authority.0.has_permission("*") || authority.0.has_permission("student_support:manage");
    value_or_error(
        StudentSupportOps::reference_data(&pool, tenant_id(tenant), &query, include_case_workers)
            .await,
    )
}

#[get("/cases")]
async fn list_cases(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    query: web::Query<StudentSupportListQuery>,
) -> HttpResponse {
    let Some(scope) = access_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(&query);
    match StudentSupportOps::list_cases(&pool, tenant_id(tenant), scope, &query).await {
        Ok((cases, total)) => paginated(CasesPage { cases }, page, per_page, total),
        Err(error) => operation_error(error),
    }
}

#[post("/cases")]
async fn create_case(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<CreateCaseRequest>,
) -> HttpResponse {
    let Some(scope) = access_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    created_or_error(
        StudentSupportOps::create_case(
            &pool,
            tenant_id(tenant),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/cases/{id}")]
async fn read_case(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Some(scope) = access_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    found(
        StudentSupportOps::get_case(&pool, tenant_id(tenant), path.into_inner(), scope).await,
        "Student Support case",
    )
}

#[put("/cases/{id}")]
async fn update_case(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateCaseRequest>,
) -> HttpResponse {
    let Some(scope) = access_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        StudentSupportOps::update_case(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Student Support case",
    )
}

#[get("/cases/{id}/actions")]
async fn list_actions(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Some(scope) = access_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    match StudentSupportOps::list_actions(&pool, tenant_id(tenant), path.into_inner(), scope).await
    {
        Ok(Some(actions)) => ok(CaseActionsPage { actions }),
        Ok(None) => not_found("Student Support case"),
        Err(error) => operation_error(error),
    }
}

#[post("/cases/{id}/actions")]
async fn create_action(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CreateCaseActionRequest>,
) -> HttpResponse {
    let Some(scope) = access_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    match StudentSupportOps::create_action(
        &pool,
        tenant_id(tenant),
        path.into_inner(),
        scope,
        actor.into_inner(),
        context.into_inner(),
        &body,
    )
    .await
    {
        Ok(Some(action)) => created(action),
        Ok(None) => not_found("Student Support case"),
        Err(error) => operation_error(error),
    }
}

#[post("/cases/{id}/team")]
async fn assign_team_member(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<AssignTeamMemberRequest>,
) -> HttpResponse {
    let Some(scope) = access_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        StudentSupportOps::assign_team_member(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Student Support case",
    )
}

#[post("/cases/{case_id}/team/{user_id}/remove")]
async fn remove_team_member(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<(Uuid, Uuid)>,
    query: web::Query<RemoveTeamMemberRequest>,
) -> HttpResponse {
    let Some(scope) = access_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    if let Some(response) = validation_response(&query.0) {
        return response;
    }
    let (case_id, user_id) = path.into_inner();
    updated_or_error(
        StudentSupportOps::remove_team_member(
            &pool,
            tenant_id(tenant),
            case_id,
            user_id,
            scope,
            actor.into_inner(),
            context.into_inner(),
            query.expected_version,
        )
        .await,
        "Student Support case",
    )
}

#[post("/cases/{id}/escalate")]
async fn escalate_case(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CaseTransitionRequest>,
) -> HttpResponse {
    transition_response(
        pool, tenant, authority, actor, context, path, body, "escalate",
    )
    .await
}

#[post("/cases/{id}/resolve")]
async fn resolve_case(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CaseTransitionRequest>,
) -> HttpResponse {
    transition_response(
        pool, tenant, authority, actor, context, path, body, "resolve",
    )
    .await
}

#[post("/cases/{id}/close")]
async fn close_case(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CaseTransitionRequest>,
) -> HttpResponse {
    transition_response(pool, tenant, authority, actor, context, path, body, "close").await
}

#[allow(
    clippy::too_many_arguments,
    reason = "the shared route retains all proof-bearing extractors"
)]
async fn transition_response(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CaseTransitionRequest>,
    action: &str,
) -> HttpResponse {
    let Some(scope) = access_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let case_id = path.into_inner();
    let result = match action {
        "escalate" => {
            StudentSupportOps::escalate_case(
                &pool,
                tenant_id(tenant),
                case_id,
                scope,
                actor.into_inner(),
                context.into_inner(),
                &body,
            )
            .await
        }
        "resolve" => {
            StudentSupportOps::resolve_case(
                &pool,
                tenant_id(tenant),
                case_id,
                scope,
                actor.into_inner(),
                context.into_inner(),
                &body,
            )
            .await
        }
        _ => {
            StudentSupportOps::close_case(
                &pool,
                tenant_id(tenant),
                case_id,
                scope,
                actor.into_inner(),
                context.into_inner(),
                &body,
            )
            .await
        }
    };
    updated_or_error(result, "Student Support case")
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(RequirePermission::new("student_support"))
            .service(reference_data)
            .service(list_cases)
            .service(create_case)
            .service(read_case)
            .service(update_case)
            .service(list_actions)
            .service(create_action)
            .service(assign_team_member)
            .service(remove_team_member)
            .service(escalate_case)
            .service(resolve_case)
            .service(close_case),
    );
}

fn access_scope(
    access: &AccessContext,
    grants: &RecordScopeGrants,
    actor: AuditActor,
) -> Option<StudentSupportAccessScope> {
    if access.has_permission("*") {
        return Some(StudentSupportAccessScope::Campus);
    }
    let family = RecordScopeFamilyKey::parse("student_support.cases").ok()?;
    match grants.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Some(StudentSupportAccessScope::Campus),
        Some(EffectiveRecordScope::Assigned | EffectiveRecordScope::SelfAndAssigned) => {
            actor.user_id().map(StudentSupportAccessScope::CaseTeam)
        }
        Some(EffectiveRecordScope::SelfRecord) | None => None,
    }
}

fn tenant_id(value: web::ReqData<TenantId>) -> Uuid {
    value.into_inner().into_inner()
}

fn bounded_page(query: &StudentSupportListQuery) -> (i64, i64) {
    (
        query.page.unwrap_or(1).clamp(1, 1_000_000),
        query.per_page.unwrap_or(20).clamp(1, 100),
    )
}

fn validation_response<T: Validate>(value: &T) -> Option<HttpResponse> {
    value.validate().err().map(|errors| {
        HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(flatten_validation_errors(&errors)),
        ))
    })
}

fn paginated<T: Serialize>(value: T, page: i64, per_page: i64, total: i64) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::with_pagination(
        StatusCode::OK,
        Some(value),
        PaginationMeta::new(page as u32, per_page as u32, total),
        None,
    ))
}

fn value_or_error<T: Serialize>(result: anyhow::Result<T>) -> HttpResponse {
    match result {
        Ok(value) => ok(value),
        Err(error) => operation_error(error),
    }
}

fn created_or_error<T: Serialize>(result: anyhow::Result<T>) -> HttpResponse {
    match result {
        Ok(value) => created(value),
        Err(error) => operation_error(error),
    }
}

fn updated_or_error<T: Serialize>(result: anyhow::Result<Option<T>>, label: &str) -> HttpResponse {
    match result {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found(label),
        Err(error) => operation_error(error),
    }
}

fn found<T: Serialize>(result: anyhow::Result<Option<T>>, label: &str) -> HttpResponse {
    updated_or_error(result, label)
}

fn ok<T: Serialize>(value: T) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::from_status(StatusCode::OK, Some(value), None))
}

fn created<T: Serialize>(value: T) -> HttpResponse {
    HttpResponse::Created().json(ApiResponse::from_status(
        StatusCode::CREATED,
        Some(value),
        None,
    ))
}

fn forbidden() -> HttpResponse {
    HttpResponse::Forbidden().json(ApiResponse::from_status(
        StatusCode::FORBIDDEN,
        None::<()>,
        Some(vec![
            "Student Support access is outside your current case-team scope".to_string(),
        ]),
    ))
}

fn not_found(label: &str) -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::from_status(
        StatusCode::NOT_FOUND,
        None::<()>,
        Some(vec![format!("{label} not found")]),
    ))
}

fn operation_error(error: anyhow::Error) -> HttpResponse {
    let message = error.to_string();
    if message.contains("changed") || message.contains("already") {
        return HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec![message]),
        ));
    }
    let operational = [
        "A ",
        "Only ",
        "Resolved ",
        "Actions ",
        "The learner ",
        "The selected ",
        "The account ",
        "Student Support ",
    ]
    .iter()
    .any(|prefix| message.starts_with(prefix));
    if operational {
        HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(vec![message]),
        ))
    } else {
        HttpResponse::InternalServerError().json(ApiResponse::from_status(
            StatusCode::INTERNAL_SERVER_ERROR,
            None::<()>,
            Some(vec![
                "Student Support could not complete the request".to_string(),
            ]),
        ))
    }
}

#[cfg(test)]
mod tests {
    use cp_audit::AuditActor;
    use cp_common::{
        AccessContext, EntitlementSnapshot, LeaseLifecycle, ModuleEntitlementState,
        RecordScopeFamilyKey, RecordScopeGrant, RecordScopeGrants, RecordScopeKind,
    };
    use uuid::Uuid;

    use super::access_scope;

    fn access(permissions: &[&str]) -> AccessContext {
        AccessContext {
            role_keys: vec![],
            permissions: permissions
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            enabled_modules: vec!["student_support".to_string()],
            entitlements: EntitlementSnapshot::new(
                LeaseLifecycle::Active,
                [(
                    "student_support".to_string(),
                    ModuleEntitlementState::Enabled,
                )],
                [],
            )
            .unwrap_or_else(|_| unreachable!()),
        }
    }

    fn grants(kind: RecordScopeKind) -> RecordScopeGrants {
        RecordScopeGrants::from_grants([RecordScopeGrant::new(
            RecordScopeFamilyKey::parse("student_support.cases").unwrap_or_else(|_| unreachable!()),
            kind,
        )])
    }

    #[test]
    fn assigned_scope_is_bound_to_the_current_person() {
        let user_id = Uuid::new_v4();
        assert_eq!(
            access_scope(
                &access(&["student_support:view"]),
                &grants(RecordScopeKind::Assigned),
                AuditActor::person(user_id),
            ),
            Some(crate::StudentSupportAccessScope::CaseTeam(user_id))
        );
    }

    #[test]
    fn missing_or_self_scope_fails_closed() {
        let user_id = Uuid::new_v4();
        assert_eq!(
            access_scope(
                &access(&["student_support:view"]),
                &RecordScopeGrants::empty(),
                AuditActor::person(user_id),
            ),
            None
        );
        assert_eq!(
            access_scope(
                &access(&["student_support:view"]),
                &grants(RecordScopeKind::SelfRecord),
                AuditActor::person(user_id),
            ),
            None
        );
    }
}
