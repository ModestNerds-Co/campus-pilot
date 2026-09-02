//! Authenticated Learning routes over typed, record-scoped domain operations.
//!
//! The application mounts authentication. This scope applies exact licensed
//! operation gates and derives campus, assigned, or learner-self visibility
//! from current record-scope grants.

use actix_multipart::Multipart;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, delete, get, post, put, web};
use cp_audit::{AuditActor, RequestContext};
use cp_common::{
    AccessContext, ApiResponse, EffectiveRecordScope, PaginationMeta, RecordScopeFamilyKey,
    RecordScopeGrants, RequirePermission, TenantId, flatten_validation_errors,
};
use cp_document_registry::{DocumentRegistryOps, DocumentStorage, NewRegistryFile};
use futures_util::StreamExt as _;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;

use crate::ops::LearningResourceCreateCommand;
use crate::{
    CreateLearningAssignmentRequest, CreateLearningQuizQuestionRequest, CreateLearningQuizRequest,
    CreateLearningResourceRequest, CreateLearningRubricCriterionRequest,
    CreateLearningSpaceRequest, CreateLearningUnitRequest, DeleteLearningQuizQuestionRequest,
    DeleteLearningRubricCriterionRequest, LearningAccessScope, LearningAssignmentListQuery,
    LearningAssignmentsPage, LearningDownloadResponse, LearningOps, LearningProgressPage,
    LearningQuizAttemptListQuery, LearningQuizAttemptsPage, LearningQuizListQuery,
    LearningQuizzesPage, LearningResourceCreation, LearningResourceFileQuery,
    LearningResourceFilesResponse, LearningSpaceListQuery, LearningSpacesPage,
    LearningSubmissionListQuery, LearningSubmissionsPage, ReasonedLearningTransitionRequest,
    ReleaseLearningFeedbackRequest, SaveLearningCompletionPolicyRequest,
    SaveLearningQuizAttemptRequest, SaveLearningSubmissionRequest,
    SubmitLearningQuizAttemptRequest, SubmitLearningSubmissionRequest,
    UpdateLearningAssignmentRequest, UpdateLearningFeedbackRequest,
    UpdateLearningQuizQuestionRequest, UpdateLearningQuizRequest, UpdateLearningResourceRequest,
    UpdateLearningRubricCriterionRequest, UpdateLearningSettingsRequest,
    UpdateLearningSpaceRequest, UpdateLearningUnitRequest, VersionedLearningRequest,
};

const MAX_RESOURCE_BYTES: usize = 15 * 1024 * 1024;

type LearningAuthority = (
    web::ReqData<AuditActor>,
    web::ReqData<AccessContext>,
    web::ReqData<RecordScopeGrants>,
);

#[get("/settings")]
async fn read_settings(pool: web::Data<PgPool>, tenant: web::ReqData<TenantId>) -> HttpResponse {
    value_or_error(LearningOps::settings(&pool, tenant_id(tenant)).await)
}

#[put("/settings")]
async fn update_settings(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<UpdateLearningSettingsRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    optional_or_conflict(
        LearningOps::update_settings(
            &pool,
            tenant_id(tenant),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Learning settings changed before this update",
    )
}

#[get("/references")]
async fn references(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
) -> HttpResponse {
    let Some(scope) = route_scope(authority) else {
        return forbidden();
    };
    value_or_error(LearningOps::references(&pool, tenant_id(tenant), scope).await)
}

#[get("/resource-files")]
async fn resource_files(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<LearningResourceFileQuery>,
) -> HttpResponse {
    match LearningOps::resource_file_candidates(&pool, tenant_id(tenant), &query).await {
        Ok(files) => ok(LearningResourceFilesResponse { files }),
        Err(error) => operation_error(error),
    }
}

#[get("/spaces")]
async fn list_spaces(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    query: web::Query<LearningSpaceListQuery>,
) -> HttpResponse {
    let Some(scope) = route_scope(authority) else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match LearningOps::list_spaces(&pool, tenant_id(tenant), scope, &query).await {
        Ok((spaces, total)) => HttpResponse::Ok().json(ApiResponse::with_pagination(
            StatusCode::OK,
            Some(LearningSpacesPage { spaces }),
            PaginationMeta::new(page as u32, per_page as u32, total),
            None,
        )),
        Err(error) => operation_error(error),
    }
}

#[post("/spaces")]
async fn create_space(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    body: web::Json<CreateLearningSpaceRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    created_or_error(
        LearningOps::create_space(
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

#[get("/spaces/{id}")]
async fn read_space(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Some(scope) = route_scope(authority) else {
        return forbidden();
    };
    optional_or_not_found(
        LearningOps::get_space(&pool, tenant_id(tenant), path.into_inner(), scope).await,
    )
}

#[put("/spaces/{id}")]
async fn update_space(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateLearningSpaceRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::update_space(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "The Learning space changed or is no longer a draft",
    )
}

#[post("/spaces/{id}/publish")]
async fn publish_space(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<VersionedLearningRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::publish_space(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            body.expected_version,
        )
        .await,
        "The Learning space changed or cannot be published",
    )
}

#[post("/spaces/{id}/archive")]
async fn archive_space(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ReasonedLearningTransitionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::archive_space(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "The Learning space changed or cannot be archived",
    )
}

#[post("/spaces/{id}/units")]
async fn create_unit(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CreateLearningUnitRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_not_found(
        LearningOps::create_unit(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[put("/units/{id}")]
async fn update_unit(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateLearningUnitRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::update_unit(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "The Learning unit changed or is no longer a draft",
    )
}

#[post("/units/{id}/publish")]
async fn publish_unit(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<VersionedLearningRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::publish_unit(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            body.expected_version,
        )
        .await,
        "The Learning unit changed or cannot be published",
    )
}

#[post("/units/{id}/withdraw")]
async fn withdraw_unit(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ReasonedLearningTransitionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::withdraw_unit(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "The Learning unit changed or cannot be withdrawn",
    )
}

#[post("/units/{id}/resources")]
async fn create_resource(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CreateLearningResourceRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_not_found(
        LearningOps::create_resource(
            &pool,
            LearningResourceCreateCommand {
                tenant_id: tenant_id(tenant),
                unit_id: path.into_inner(),
                scope,
                actor: actor.into_inner(),
                request_context: context.into_inner(),
                request: &body,
                creation: LearningResourceCreation::Link,
            },
        )
        .await,
    )
}

#[post("/units/{id}/resources/upload")]
async fn upload_resource(
    pool: web::Data<PgPool>,
    storage: web::Data<DocumentStorage>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    mut payload: Multipart,
) -> HttpResponse {
    let tenant_id = tenant_id(tenant);
    let unit_id = path.into_inner();
    let (actor, access, grants) = authority;
    let actor_value = actor.into_inner();
    let Some(scope) = access_scope(&access, &grants, actor_value) else {
        return forbidden();
    };
    match LearningOps::authorize_unit_for_write(&pool, tenant_id, unit_id, scope).await {
        Ok(Some(_)) => {}
        Ok(None) => return not_found(),
        Err(error) => return operation_error(error),
    }
    let settings = match LearningOps::settings(&pool, tenant_id).await {
        Ok(settings) => settings,
        Err(error) => return operation_error(error),
    };
    let Some(series_id) = settings.document_series_id else {
        return bad_request(
            "Choose a document classification in Learning settings before uploading resources",
        );
    };
    let mut display_title = None;
    let mut position = None;
    let mut description = None;
    let mut original_file_name = None;
    let mut media_type = None;
    let mut bytes = None;
    while let Some(item) = payload.next().await {
        let mut field = match item {
            Ok(field) => field,
            Err(_) => return bad_request("The resource upload is malformed"),
        };
        let disposition = field.content_disposition();
        let name = disposition
            .and_then(|value| value.get_name())
            .unwrap_or_default()
            .to_string();
        if name == "file" {
            if bytes.is_some() {
                return bad_request("Upload one resource at a time");
            }
            original_file_name = disposition
                .and_then(|value| value.get_filename())
                .map(ToOwned::to_owned);
            media_type = field.content_type().map(ToString::to_string);
            bytes = match read_field(&mut field, MAX_RESOURCE_BYTES).await {
                Ok(bytes) => Some(bytes),
                Err(message) => return bad_request(message),
            };
            continue;
        }
        let raw = match read_field(&mut field, 4096).await {
            Ok(value) => value,
            Err(message) => return bad_request(message),
        };
        let value = match String::from_utf8(raw) {
            Ok(value) => value.trim().to_string(),
            Err(_) => return bad_request("The resource details are invalid"),
        };
        match name.as_str() {
            "display_title" => display_title = non_empty(value),
            "position" => position = value.parse::<i32>().ok().filter(|value| *value > 0),
            "description" => description = non_empty(value),
            _ => return bad_request("The resource upload contains an unknown field"),
        }
    }
    let Some(display_title) = display_title else {
        return bad_request("Resource title is required");
    };
    let Some(position) = position else {
        return bad_request("Resource position must be a positive number");
    };
    let Some(original_file_name) = original_file_name else {
        return bad_request("Choose a PDF, JPEG, or PNG resource");
    };
    let Some(media_type) = media_type else {
        return bad_request("The resource file type is missing");
    };
    let Some(bytes) = bytes else {
        return bad_request("Choose a PDF, JPEG, or PNG resource");
    };
    let document = match DocumentRegistryOps::create_file(
        &pool,
        &storage,
        tenant_id,
        actor_value,
        context.clone().into_inner(),
        NewRegistryFile {
            series_id,
            title: display_title.clone(),
            description,
            document_date: None,
            sensitivity: None,
            original_file_name,
            media_type,
            bytes,
        },
    )
    .await
    {
        Ok(document) => document,
        Err(error) => return operation_error(error),
    };
    let request = CreateLearningResourceRequest {
        document_file_id: document.id,
        display_title,
        position,
    };
    match LearningOps::create_resource(
        &pool,
        LearningResourceCreateCommand {
            tenant_id,
            unit_id,
            scope,
            actor: actor_value,
            request_context: context.into_inner(),
            request: &request,
            creation: LearningResourceCreation::Upload,
        },
    )
    .await
    {
        Ok(Some(resource)) => HttpResponse::Created().json(ApiResponse::from_status(
            StatusCode::CREATED,
            Some(resource),
            None,
        )),
        Ok(None) => recoverable_upload_error(
            &document.reference,
            "The Learning unit is no longer available",
        ),
        Err(error) => recoverable_upload_error(&document.reference, &error.to_string()),
    }
}

#[put("/resources/{id}")]
async fn update_resource(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateLearningResourceRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::update_resource(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "The Learning resource changed or is no longer a draft",
    )
}

#[post("/resources/{id}/publish")]
async fn publish_resource(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<VersionedLearningRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::publish_resource(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            body.expected_version,
        )
        .await,
        "The Learning resource changed or cannot be published",
    )
}

#[post("/resources/{id}/withdraw")]
async fn withdraw_resource(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ReasonedLearningTransitionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::withdraw_resource(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "The Learning resource changed or cannot be withdrawn",
    )
}

#[get("/resources/{id}/download")]
async fn download_resource(
    pool: web::Data<PgPool>,
    storage: web::Data<DocumentStorage>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Some(scope) = route_scope(authority) else {
        return forbidden();
    };
    match LearningOps::authorized_resource_object_key(
        &pool,
        tenant_id(tenant),
        path.into_inner(),
        scope,
    )
    .await
    {
        Ok(Some(key)) => match storage.download_url(&key, 60).await {
            Ok(url) => ok(LearningDownloadResponse {
                url,
                expires_in_seconds: 60,
            }),
            Err(_) => internal_error(),
        },
        Ok(None) => not_found(),
        Err(error) => operation_error(error),
    }
}

#[get("/spaces/{id}/assignments")]
async fn list_assignments(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    path: web::Path<Uuid>,
    query: web::Query<LearningAssignmentListQuery>,
) -> HttpResponse {
    let Some(scope) = route_scope(authority) else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match LearningOps::list_assignments(&pool, tenant_id(tenant), path.into_inner(), scope, &query)
        .await
    {
        Ok((assignments, total)) => HttpResponse::Ok().json(ApiResponse::with_pagination(
            StatusCode::OK,
            Some(LearningAssignmentsPage { assignments }),
            PaginationMeta::new(page as u32, per_page as u32, total),
            None,
        )),
        Err(error) => operation_error(error),
    }
}

#[get("/assignments/{id}")]
async fn read_assignment(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Some(scope) = route_scope(authority) else {
        return forbidden();
    };
    optional_or_not_found(
        LearningOps::get_assignment(&pool, tenant_id(tenant), path.into_inner(), scope).await,
    )
}

#[post("/units/{id}/assignments")]
async fn create_assignment(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CreateLearningAssignmentRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_created_or_not_found(
        LearningOps::create_assignment(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[put("/assignments/{id}")]
async fn update_assignment(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateLearningAssignmentRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::update_assignment(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "The Learning assignment changed or is no longer a draft",
    )
}

#[post("/assignments/{id}/publish")]
async fn publish_assignment(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<VersionedLearningRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::publish_assignment(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            body.expected_version,
        )
        .await,
        "The Learning assignment changed or cannot be published",
    )
}

#[post("/assignments/{id}/close")]
async fn close_assignment(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ReasonedLearningTransitionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::close_assignment(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "The Learning assignment changed or cannot be closed",
    )
}

#[post("/assignments/{id}/rubric-criteria")]
async fn create_rubric_criterion(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CreateLearningRubricCriterionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_created_or_not_found(
        LearningOps::create_rubric_criterion(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[put("/rubric-criteria/{id}")]
async fn update_rubric_criterion(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateLearningRubricCriterionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::update_rubric_criterion(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "The rubric criterion changed or is no longer editable",
    )
}

#[delete("/rubric-criteria/{id}")]
async fn delete_rubric_criterion(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<DeleteLearningRubricCriterionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    match LearningOps::delete_rubric_criterion(
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
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => conflict("The rubric criterion changed or is no longer editable"),
        Err(error) => operation_error(error),
    }
}

#[get("/assignments/{id}/submission")]
async fn read_self_submission(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Some(scope) = route_scope(authority) else {
        return forbidden();
    };
    optional_or_not_found(
        LearningOps::self_submission(&pool, tenant_id(tenant), path.into_inner(), scope).await,
    )
}

#[put("/assignments/{id}/submission")]
async fn save_self_submission(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<SaveLearningSubmissionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::save_self_submission(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "The learner submission changed or is unavailable",
    )
}

#[post("/assignments/{id}/submission/submit")]
async fn submit_self_submission(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<SubmitLearningSubmissionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::submit_self_submission(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "The learner submission changed or cannot be submitted",
    )
}

#[get("/assignments/{id}/submissions")]
async fn list_submissions(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    path: web::Path<Uuid>,
    query: web::Query<LearningSubmissionListQuery>,
) -> HttpResponse {
    let Some(scope) = route_scope(authority) else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match LearningOps::list_submissions(&pool, tenant_id(tenant), path.into_inner(), scope, &query)
        .await
    {
        Ok((submissions, total)) => HttpResponse::Ok().json(ApiResponse::with_pagination(
            StatusCode::OK,
            Some(LearningSubmissionsPage { submissions }),
            PaginationMeta::new(page as u32, per_page as u32, total),
            None,
        )),
        Err(error) => operation_error(error),
    }
}

#[get("/submissions/{id}")]
async fn read_submission(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Some(scope) = route_scope(authority) else {
        return forbidden();
    };
    optional_or_not_found(
        LearningOps::get_submission(&pool, tenant_id(tenant), path.into_inner(), scope).await,
    )
}

#[put("/submissions/{id}/feedback")]
async fn update_feedback(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateLearningFeedbackRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::update_feedback(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "The feedback changed or cannot be saved",
    )
}

#[post("/submissions/{id}/feedback/release")]
async fn release_feedback(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ReleaseLearningFeedbackRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::release_feedback(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "The feedback changed or cannot be released",
    )
}

#[get("/spaces/{id}/quizzes")]
async fn list_quizzes(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    path: web::Path<Uuid>,
    query: web::Query<LearningQuizListQuery>,
) -> HttpResponse {
    let Some(scope) = route_scope(authority) else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match LearningOps::list_quizzes(&pool, tenant_id(tenant), path.into_inner(), scope, &query)
        .await
    {
        Ok((quizzes, total)) => HttpResponse::Ok().json(ApiResponse::with_pagination(
            StatusCode::OK,
            Some(LearningQuizzesPage { quizzes }),
            PaginationMeta::new(page as u32, per_page as u32, total),
            None,
        )),
        Err(error) => operation_error(error),
    }
}

#[post("/units/{id}/quizzes")]
async fn create_quiz(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CreateLearningQuizRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_created_or_not_found(
        LearningOps::create_quiz(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/quizzes/{id}")]
async fn read_quiz(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Some(scope) = route_scope(authority) else {
        return forbidden();
    };
    optional_or_not_found(
        LearningOps::get_quiz(&pool, tenant_id(tenant), path.into_inner(), scope).await,
    )
}

#[put("/quizzes/{id}")]
async fn update_quiz(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateLearningQuizRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::update_quiz(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "The Learning quiz changed or is no longer a draft",
    )
}

#[post("/quizzes/{id}/questions")]
async fn create_quiz_question(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CreateLearningQuizQuestionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_created_or_not_found(
        LearningOps::create_quiz_question(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[put("/quiz-questions/{id}")]
async fn update_quiz_question(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateLearningQuizQuestionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::update_quiz_question(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "The quiz question changed or is no longer editable",
    )
}

#[delete("/quiz-questions/{id}")]
async fn delete_quiz_question(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<DeleteLearningQuizQuestionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    match LearningOps::delete_quiz_question(
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
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => conflict("The quiz question changed or is no longer editable"),
        Err(error) => operation_error(error),
    }
}

#[post("/quizzes/{id}/publish")]
async fn publish_quiz(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<VersionedLearningRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::publish_quiz(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            body.expected_version,
        )
        .await,
        "The Learning quiz changed or cannot be published",
    )
}

#[post("/quizzes/{id}/close")]
async fn close_quiz(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ReasonedLearningTransitionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::close_quiz(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "The Learning quiz changed or cannot be closed",
    )
}

#[post("/quizzes/{id}/attempts")]
async fn start_quiz_attempt(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_created_or_not_found(
        LearningOps::start_quiz_attempt(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
        )
        .await,
    )
}

#[get("/quizzes/{id}/attempts")]
async fn list_quiz_attempts(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    path: web::Path<Uuid>,
    query: web::Query<LearningQuizAttemptListQuery>,
) -> HttpResponse {
    let Some(scope) = route_scope(authority) else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match LearningOps::list_quiz_attempts(
        &pool,
        tenant_id(tenant),
        path.into_inner(),
        scope,
        &query,
    )
    .await
    {
        Ok((attempts, total)) => HttpResponse::Ok().json(ApiResponse::with_pagination(
            StatusCode::OK,
            Some(LearningQuizAttemptsPage { attempts }),
            PaginationMeta::new(page as u32, per_page as u32, total),
            None,
        )),
        Err(error) => operation_error(error),
    }
}

#[get("/quiz-attempts/{id}")]
async fn read_quiz_attempt(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Some(scope) = route_scope(authority) else {
        return forbidden();
    };
    optional_or_not_found(
        LearningOps::get_quiz_attempt(&pool, tenant_id(tenant), path.into_inner(), scope).await,
    )
}

#[put("/quiz-attempts/{id}")]
async fn save_quiz_attempt(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<SaveLearningQuizAttemptRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::save_quiz_attempt(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "The Learning quiz attempt changed or is unavailable",
    )
}

#[post("/quiz-attempts/{id}/submit")]
async fn submit_quiz_attempt(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<SubmitLearningQuizAttemptRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::submit_quiz_attempt(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "The Learning quiz attempt changed or cannot be submitted",
    )
}

#[get("/spaces/{id}/completion-policy")]
async fn read_completion_policy(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Some(scope) = route_scope(authority) else {
        return forbidden();
    };
    optional_or_not_found(
        LearningOps::completion_policy(&pool, tenant_id(tenant), path.into_inner(), scope).await,
    )
}

#[put("/spaces/{id}/completion-policy")]
async fn save_completion_policy(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<SaveLearningCompletionPolicyRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    value_or_error(
        LearningOps::save_completion_policy(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[post("/spaces/{id}/completion-policy/publish")]
async fn publish_completion_policy(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<VersionedLearningRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let Some(scope) = access_scope(&access, &grants, actor.clone().into_inner()) else {
        return forbidden();
    };
    optional_or_conflict(
        LearningOps::publish_completion_policy(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            body.expected_version,
        )
        .await,
        "The Learning completion policy changed or cannot be published",
    )
}

#[get("/spaces/{id}/completion/me")]
async fn read_self_completion(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Some(scope) = route_scope(authority) else {
        return forbidden();
    };
    value_or_error(
        LearningOps::self_completion(&pool, tenant_id(tenant), path.into_inner(), scope).await,
    )
}

#[get("/spaces/{id}/completion")]
async fn list_completion(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Some(scope) = route_scope(authority) else {
        return forbidden();
    };
    value_or_error(
        LearningOps::list_completion(&pool, tenant_id(tenant), path.into_inner(), scope).await,
    )
}

#[get("/spaces/{id}/progress/me")]
async fn read_self_progress(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Some(scope) = route_scope(authority) else {
        return forbidden();
    };
    optional_or_not_found(
        LearningOps::self_progress(&pool, tenant_id(tenant), path.into_inner(), scope).await,
    )
}

#[get("/spaces/{id}/progress")]
async fn list_progress(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: LearningAuthority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Some(scope) = route_scope(authority) else {
        return forbidden();
    };
    match LearningOps::list_progress(&pool, tenant_id(tenant), path.into_inner(), scope).await {
        Ok(progress) => ok(LearningProgressPage { progress }),
        Err(error) => operation_error(error),
    }
}

/// Registers every released Learning route beneath `/api/1.0/learning`.
pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(RequirePermission::new("learning"))
            .service(read_settings)
            .service(update_settings)
            .service(references)
            .service(resource_files)
            .service(list_spaces)
            .service(create_space)
            .service(read_space)
            .service(update_space)
            .service(publish_space)
            .service(archive_space)
            .service(create_unit)
            .service(update_unit)
            .service(publish_unit)
            .service(withdraw_unit)
            .service(create_resource)
            .service(upload_resource)
            .service(update_resource)
            .service(publish_resource)
            .service(withdraw_resource)
            .service(download_resource)
            .service(list_assignments)
            .service(read_assignment)
            .service(create_assignment)
            .service(update_assignment)
            .service(publish_assignment)
            .service(close_assignment)
            .service(create_rubric_criterion)
            .service(update_rubric_criterion)
            .service(delete_rubric_criterion)
            .service(read_self_submission)
            .service(save_self_submission)
            .service(submit_self_submission)
            .service(list_submissions)
            .service(read_submission)
            .service(update_feedback)
            .service(release_feedback)
            .service(list_quizzes)
            .service(create_quiz)
            .service(read_quiz)
            .service(update_quiz)
            .service(create_quiz_question)
            .service(update_quiz_question)
            .service(delete_quiz_question)
            .service(publish_quiz)
            .service(close_quiz)
            .service(start_quiz_attempt)
            .service(list_quiz_attempts)
            .service(read_quiz_attempt)
            .service(save_quiz_attempt)
            .service(submit_quiz_attempt)
            .service(read_completion_policy)
            .service(save_completion_policy)
            .service(publish_completion_policy)
            .service(read_self_completion)
            .service(list_completion)
            .service(read_self_progress)
            .service(list_progress),
    );
}

fn route_scope(authority: LearningAuthority) -> Option<LearningAccessScope> {
    let (actor, access, grants) = authority;
    access_scope(&access, &grants, actor.into_inner())
}

fn access_scope(
    access: &AccessContext,
    grants: &RecordScopeGrants,
    actor: AuditActor,
) -> Option<LearningAccessScope> {
    if access.has_permission("*") {
        return Some(LearningAccessScope::Campus);
    }
    let family = RecordScopeFamilyKey::parse("learning.spaces").ok()?;
    let user_id = actor.user_id()?;
    match grants.effective_scope(&family)? {
        EffectiveRecordScope::Campus => Some(LearningAccessScope::Campus),
        EffectiveRecordScope::Assigned => Some(LearningAccessScope::AssignedTo(user_id)),
        EffectiveRecordScope::SelfRecord => Some(LearningAccessScope::SelfFor(user_id)),
        EffectiveRecordScope::SelfAndAssigned => {
            Some(LearningAccessScope::SelfAndAssigned(user_id))
        }
    }
}

fn tenant_id(tenant: web::ReqData<TenantId>) -> Uuid {
    tenant.into_inner().into_inner()
}
fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).max(1),
        per_page.unwrap_or(25).clamp(1, 100),
    )
}
fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

async fn read_field(
    field: &mut actix_multipart::Field,
    maximum: usize,
) -> Result<Vec<u8>, &'static str> {
    let mut bytes = Vec::new();
    while let Some(chunk) = field.next().await {
        let chunk = chunk.map_err(|_| "The resource upload could not be read")?;
        if bytes.len().saturating_add(chunk.len()) > maximum {
            return Err("The resource upload exceeds 15 MB");
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn ok<T: Serialize>(value: T) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::from_status(StatusCode::OK, Some(value), None))
}
fn value_or_error<T: Serialize>(result: anyhow::Result<T>) -> HttpResponse {
    match result {
        Ok(value) => ok(value),
        Err(error) => operation_error(error),
    }
}
fn created_or_error<T: Serialize>(result: anyhow::Result<T>) -> HttpResponse {
    match result {
        Ok(value) => HttpResponse::Created().json(ApiResponse::from_status(
            StatusCode::CREATED,
            Some(value),
            None,
        )),
        Err(error) => operation_error(error),
    }
}
fn optional_created_or_not_found<T: Serialize>(result: anyhow::Result<Option<T>>) -> HttpResponse {
    match result {
        Ok(Some(value)) => HttpResponse::Created().json(ApiResponse::from_status(
            StatusCode::CREATED,
            Some(value),
            None,
        )),
        Ok(None) => not_found(),
        Err(error) => operation_error(error),
    }
}
fn optional_or_not_found<T: Serialize>(result: anyhow::Result<Option<T>>) -> HttpResponse {
    match result {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found(),
        Err(error) => operation_error(error),
    }
}
fn optional_or_conflict<T: Serialize>(
    result: anyhow::Result<Option<T>>,
    message: &str,
) -> HttpResponse {
    match result {
        Ok(Some(value)) => ok(value),
        Ok(None) => conflict(message),
        Err(error) => operation_error(error),
    }
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
fn not_found() -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::from_status(
        StatusCode::NOT_FOUND,
        None::<()>,
        Some(vec!["Learning record not found".to_string()]),
    ))
}
fn forbidden() -> HttpResponse {
    HttpResponse::Forbidden().json(ApiResponse::from_status(
        StatusCode::FORBIDDEN,
        None::<()>,
        Some(vec!["Learning record scope is unavailable".to_string()]),
    ))
}
fn bad_request(message: &str) -> HttpResponse {
    HttpResponse::BadRequest().json(ApiResponse::from_status(
        StatusCode::BAD_REQUEST,
        None::<()>,
        Some(vec![message.to_string()]),
    ))
}
fn conflict(message: &str) -> HttpResponse {
    HttpResponse::Conflict().json(ApiResponse::from_status(
        StatusCode::CONFLICT,
        None::<()>,
        Some(vec![message.to_string()]),
    ))
}
fn recoverable_upload_error(reference: &str, detail: &str) -> HttpResponse {
    conflict(&format!(
        "The file was stored as {reference}, but it could not be linked. {detail}"
    ))
}
fn operation_error(error: anyhow::Error) -> HttpResponse {
    let message = error.to_string();
    if message.contains("already") || message.contains("changed") || message.contains("position") {
        return conflict(&message);
    }
    if ["A ", "An ", "The ", "Publish ", "Learner "]
        .iter()
        .any(|prefix| message.starts_with(prefix))
    {
        return bad_request(&message);
    }
    internal_error()
}
fn internal_error() -> HttpResponse {
    HttpResponse::InternalServerError().json(ApiResponse::from_status(
        StatusCode::INTERNAL_SERVER_ERROR,
        None::<()>,
        Some(vec!["Learning could not complete the request".to_string()]),
    ))
}

#[cfg(test)]
mod tests {
    use super::{LearningAccessScope, access_scope};
    use cp_audit::AuditActor;
    use cp_common::{
        AccessContext, EntitlementSnapshot, LeaseLifecycle, ModuleEntitlementState,
        RecordScopeFamilyKey, RecordScopeGrant, RecordScopeGrants, RecordScopeKind,
    };
    use uuid::Uuid;

    fn access(permissions: &[&str]) -> AccessContext {
        AccessContext {
            role_keys: vec![],
            permissions: permissions
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            enabled_modules: vec!["learning".to_string()],
            entitlements: EntitlementSnapshot::new(
                LeaseLifecycle::Active,
                vec![("learning".to_string(), ModuleEntitlementState::Enabled)],
                vec![],
            )
            .unwrap_or_else(|_| unreachable!()),
        }
    }
    fn grants(kind: RecordScopeKind) -> RecordScopeGrants {
        let mut grants = RecordScopeGrants::empty();
        grants.insert(RecordScopeGrant::new(
            RecordScopeFamilyKey::parse("learning.spaces").unwrap_or_else(|_| unreachable!()),
            kind,
        ));
        grants
    }
    #[test]
    fn role_scope_maps_to_the_authenticated_person() {
        let user_id = Uuid::new_v4();
        assert_eq!(
            access_scope(
                &access(&["learning:view"]),
                &grants(RecordScopeKind::Assigned),
                AuditActor::person(user_id)
            ),
            Some(LearningAccessScope::AssignedTo(user_id))
        );
        assert_eq!(
            access_scope(
                &access(&["learning:view"]),
                &grants(RecordScopeKind::SelfRecord),
                AuditActor::person(user_id)
            ),
            Some(LearningAccessScope::SelfFor(user_id))
        );
        assert_eq!(
            access_scope(
                &access(&["*"]),
                &RecordScopeGrants::empty(),
                AuditActor::person(user_id)
            ),
            Some(LearningAccessScope::Campus)
        );
    }
    #[test]
    fn missing_scope_fails_closed() {
        assert_eq!(
            access_scope(
                &access(&["learning:view"]),
                &RecordScopeGrants::empty(),
                AuditActor::person(Uuid::new_v4())
            ),
            None
        );
    }
}
