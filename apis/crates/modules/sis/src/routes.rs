//! Authenticated SIS HTTP routes.
//!
//! The application mounts identity middleware outside this crate. This scope
//! applies SIS permissions while the operation evaluator enforces licensing.
//! Academic-placement operations also require Academics; learner numbering is
//! deliberately SIS-only.

use actix_multipart::Multipart;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, delete, get, post, put, web};
use cp_audit::{AuditActor, RequestContext};
use cp_common::{
    ApiResponse, PaginationMeta, RequirePermission, TenantId, flatten_validation_errors,
};
use cp_imports::{MAX_SOURCE_BYTES, parse_source};
use futures_util::StreamExt;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;

use crate::{
    dtos::{
        AccountCandidateQuery, AccountCandidatesResponse, ApplicationListQuery,
        CreateApplicationRequest, CreateEnrolmentRequest, CreateGuardianRelationshipRequest,
        CreateGuardianRequest, CreateLearnerRequest, DirectoryListQuery, EnrolmentListQuery,
        GuardianResponse, LearnerResponse, LinkAccountRequest, PaginatedApplicationsResponse,
        PaginatedEnrolmentsResponse, PaginatedGuardianRelationshipsResponse,
        PaginatedGuardiansResponse, PaginatedLearnersResponse, RelationshipListQuery,
        UpdateApplicationRequest, UpdateEnrolmentRequest, UpdateGuardianRelationshipRequest,
        UpdateGuardianRequest, UpdateLearnerNumberingPolicyRequest, UpdateLearnerRequest,
    },
    imports::{
        CommitImportRequest, ImportListQuery, NewSisImport, PreviewRowsQuery,
        SisImportListResponse, SisImportMapping, SisImportOps, SisImportTarget,
    },
    numbering::{LearnerNumberingPolicyOps, LearnerNumberingUpdateOutcome},
    ops::{
        AccountCandidateOps, ApplicationOps, DeleteOutcome, EnrolmentOps, GuardianOps,
        GuardianRelationshipOps, LearnerOps,
    },
};

#[get("/learner-numbering")]
async fn read_learner_numbering_policy(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
) -> HttpResponse {
    updated_value_or_error(LearnerNumberingPolicyOps::get(pool.get_ref(), tenant_id(tenant)).await)
}

#[put("/learner-numbering")]
async fn update_learner_numbering_policy(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    body: web::Json<UpdateLearnerNumberingPolicyRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    match LearnerNumberingPolicyOps::update(
        pool.get_ref(),
        tenant_id(tenant),
        actor.into_inner(),
        request_context.into_inner(),
        body.into_inner(),
    )
    .await
    {
        Ok(LearnerNumberingUpdateOutcome::Updated(policy)) => ok(policy),
        Ok(LearnerNumberingUpdateOutcome::VersionConflict) => {
            conflict("Learner numbering changed since it was loaded. Refresh and try again.")
        }
        Ok(LearnerNumberingUpdateOutcome::NextSequenceBehind { minimum }) => conflict(&format!(
            "Next sequence cannot be below the current boundary of {minimum}."
        )),
        Err(error) => operation_error(error),
    }
}

#[get("/imports")]
async fn list_imports(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<ImportListQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    let (imports, total) = SisImportOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        query.target,
    )
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(paginated(
        SisImportListResponse { imports },
        page,
        per_page,
        total,
    ))
}

#[post("/imports")]
async fn upload_import(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    mut payload: Multipart,
) -> HttpResponse {
    let mut target = None;
    let mut file_name = None;
    let mut content_type = None;
    let mut source_bytes = None;

    while let Some(item) = payload.next().await {
        let mut field = match item {
            Ok(field) => field,
            Err(_) => return bad_request("The import upload is malformed."),
        };
        let disposition = field.content_disposition();
        let field_name = disposition
            .and_then(|value| value.get_name())
            .unwrap_or_default()
            .to_string();
        if field_name == "target" {
            let bytes = match read_bounded_field(&mut field, 64).await {
                Ok(bytes) => bytes,
                Err(message) => return bad_request(message),
            };
            let value = match String::from_utf8(bytes) {
                Ok(value) => value,
                Err(_) => return bad_request("The import target is invalid."),
            };
            target = match SisImportTarget::parse(value.trim()) {
                Ok(target) => Some(target),
                Err(error) => return bad_request(&error.to_string()),
            };
        } else if field_name == "file" {
            if source_bytes.is_some() {
                return bad_request("Upload one import file at a time.");
            }
            file_name = disposition
                .and_then(|value| value.get_filename())
                .map(ToOwned::to_owned);
            content_type = field.content_type().map(ToString::to_string);
            source_bytes = match read_bounded_field(&mut field, MAX_SOURCE_BYTES).await {
                Ok(bytes) => Some(bytes),
                Err(message) => return bad_request(message),
            };
        } else {
            return bad_request("The import upload contains an unknown field.");
        }
    }

    let Some(target) = target else {
        return bad_request("Choose whether the file contains learners or guardians.");
    };
    let Some(file_name) = file_name else {
        return bad_request("Choose a CSV or XLSX file.");
    };
    let Some(source_bytes) = source_bytes else {
        return bad_request("Choose a CSV or XLSX file.");
    };
    let parse_name = file_name.clone();
    let parse_bytes = source_bytes.clone();
    let parsed = match web::block(move || parse_source(&parse_name, &parse_bytes)).await {
        Ok(Ok(parsed)) => parsed,
        Ok(Err(error)) => return bad_request(&error.to_string()),
        Err(_) => return internal_error("The import file could not be read."),
    };
    created_or_error(
        SisImportOps::create(
            pool.get_ref(),
            tenant_id(tenant),
            actor.into_inner(),
            request_context.into_inner(),
            NewSisImport {
                target,
                file_name,
                content_type: content_type
                    .unwrap_or_else(|| "application/octet-stream".to_string()),
                source_bytes,
                parsed,
            },
        )
        .await,
    )
}

#[get("/imports/{id}")]
async fn read_import(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let record = SisImportOps::get(pool.get_ref(), tenant_id(tenant), path.into_inner())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(found(record, "Import"))
}

#[put("/imports/{id}/mapping")]
async fn preview_import_mapping(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<SisImportMapping>,
) -> HttpResponse {
    let import_id = path.into_inner();
    let tenant_id = tenant_id(tenant);
    let source = match SisImportOps::retained_source(pool.get_ref(), tenant_id, import_id).await {
        Ok(Some(source)) => source,
        Ok(None) => return not_found("Import"),
        Err(_) => return internal_error("The retained import source could not be loaded."),
    };
    let parse_name = source.file_name.clone();
    let parse_bytes = source.source_bytes;
    let table = match web::block(move || parse_source(&parse_name, &parse_bytes)).await {
        Ok(Ok(parsed)) => parsed.table,
        Ok(Err(error)) => return bad_request(&error.to_string()),
        Err(_) => return internal_error("The retained import source could not be read."),
    };
    updated_value_or_error(
        SisImportOps::create_preview(
            pool.get_ref(),
            tenant_id,
            actor.into_inner(),
            request_context.into_inner(),
            import_id,
            body.into_inner(),
            &table,
        )
        .await,
    )
}

#[get("/imports/{id}/preview")]
async fn read_import_preview(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    query: web::Query<PreviewRowsQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    let preview = SisImportOps::preview(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        page,
        per_page,
    )
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(found(preview, "Import preview"))
}

#[post("/imports/{id}/commit")]
async fn commit_import(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CommitImportRequest>,
) -> HttpResponse {
    updated_value_or_error(
        SisImportOps::commit(
            pool.get_ref(),
            tenant_id(tenant),
            actor.into_inner(),
            request_context.into_inner(),
            path.into_inner(),
            body.preview_id,
        )
        .await,
    )
}

#[get("/account-candidates")]
async fn list_account_candidates(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<AccountCandidateQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let accounts = AccountCandidateOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        query.profile_kind,
        query.profile_id,
        trimmed(query.search.as_deref()),
    )
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(ok(AccountCandidatesResponse { accounts }))
}

#[get("/learners")]
async fn list_learners(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<DirectoryListQuery<crate::dtos::LearnerStatus>>,
) -> Result<HttpResponse, actix_web::Error> {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    let (learners, total) = LearnerOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        query.status.map(crate::dtos::LearnerStatus::as_str),
    )
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(paginated(
        PaginatedLearnersResponse {
            learners: learners.into_iter().map(LearnerResponse::from).collect(),
        },
        page,
        per_page,
        total,
    ))
}

#[get("/learners/{id}")]
async fn read_learner(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let learner = LearnerOps::get_by_id(pool.get_ref(), tenant_id(tenant), path.into_inner())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(found(learner.map(LearnerResponse::from), "Learner"))
}

#[post("/learners")]
async fn create_learner(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<CreateLearnerRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    created_or_error(
        LearnerOps::create(pool.get_ref(), tenant_id(tenant), &body)
            .await
            .map(LearnerResponse::from),
    )
}

#[put("/learners/{id}")]
async fn update_learner(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateLearnerRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    updated_or_error(
        LearnerOps::update(pool.get_ref(), tenant_id(tenant), path.into_inner(), &body)
            .await
            .map(|value| value.map(LearnerResponse::from)),
        "Learner",
    )
}

#[put("/learners/{id}/account")]
async fn link_learner_account(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<LinkAccountRequest>,
) -> HttpResponse {
    updated_or_error(
        LearnerOps::link_account(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            body.account_id,
        )
        .await
        .map(|value| value.map(LearnerResponse::from)),
        "Learner",
    )
}

#[delete("/learners/{id}")]
async fn delete_learner(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let outcome = LearnerOps::delete(pool.get_ref(), tenant_id(tenant), path.into_inner())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(delete_response(
        outcome,
        "Learner",
        "Remove the learner's relationships, applications, and enrolments first.",
    ))
}

#[get("/guardians")]
async fn list_guardians(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<DirectoryListQuery<crate::dtos::ActiveStatus>>,
) -> Result<HttpResponse, actix_web::Error> {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    let (guardians, total) = GuardianOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        query.status.map(crate::dtos::ActiveStatus::as_str),
    )
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(paginated(
        PaginatedGuardiansResponse {
            guardians: guardians.into_iter().map(GuardianResponse::from).collect(),
        },
        page,
        per_page,
        total,
    ))
}

#[get("/guardians/{id}")]
async fn read_guardian(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let guardian = GuardianOps::get_by_id(pool.get_ref(), tenant_id(tenant), path.into_inner())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(found(guardian.map(GuardianResponse::from), "Guardian"))
}

#[post("/guardians")]
async fn create_guardian(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<CreateGuardianRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    created_or_error(
        GuardianOps::create(pool.get_ref(), tenant_id(tenant), &body)
            .await
            .map(GuardianResponse::from),
    )
}

#[put("/guardians/{id}")]
async fn update_guardian(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateGuardianRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    updated_or_error(
        GuardianOps::update(pool.get_ref(), tenant_id(tenant), path.into_inner(), &body)
            .await
            .map(|value| value.map(GuardianResponse::from)),
        "Guardian",
    )
}

#[put("/guardians/{id}/account")]
async fn link_guardian_account(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<LinkAccountRequest>,
) -> HttpResponse {
    updated_or_error(
        GuardianOps::link_account(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            body.account_id,
        )
        .await
        .map(|value| value.map(GuardianResponse::from)),
        "Guardian",
    )
}

#[delete("/guardians/{id}")]
async fn delete_guardian(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let outcome = GuardianOps::delete(pool.get_ref(), tenant_id(tenant), path.into_inner())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(delete_response(
        outcome,
        "Guardian",
        "Remove this guardian's learner relationships first.",
    ))
}

#[get("/guardian-relationships")]
async fn list_guardian_relationships(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<RelationshipListQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    let (relationships, total) = GuardianRelationshipOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        query.status.map(crate::dtos::ActiveStatus::as_str),
        query.learner_id,
        query.guardian_id,
    )
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(paginated(
        PaginatedGuardianRelationshipsResponse { relationships },
        page,
        per_page,
        total,
    ))
}

#[get("/guardian-relationships/{id}")]
async fn read_guardian_relationship(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let relationship =
        GuardianRelationshipOps::get_by_id(pool.get_ref(), tenant_id(tenant), path.into_inner())
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(found(relationship, "Guardian relationship"))
}

#[post("/guardian-relationships")]
async fn create_guardian_relationship(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<CreateGuardianRelationshipRequest>,
) -> HttpResponse {
    created_or_error(
        GuardianRelationshipOps::create(pool.get_ref(), tenant_id(tenant), &body).await,
    )
}

#[put("/guardian-relationships/{id}")]
async fn update_guardian_relationship(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateGuardianRelationshipRequest>,
) -> HttpResponse {
    updated_or_error(
        GuardianRelationshipOps::update(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            &body,
        )
        .await,
        "Guardian relationship",
    )
}

#[delete("/guardian-relationships/{id}")]
async fn delete_guardian_relationship(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let outcome =
        GuardianRelationshipOps::delete(pool.get_ref(), tenant_id(tenant), path.into_inner())
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(delete_response(
        outcome,
        "Guardian relationship",
        "This relationship is still in use.",
    ))
}

#[get("/applications")]
async fn list_applications(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<ApplicationListQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    let (applications, total) = ApplicationOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        query.status.map(crate::dtos::ApplicationStatus::as_str),
        query.academic_year_id,
        query.target_grade_level_id,
        query.learner_id,
    )
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(paginated(
        PaginatedApplicationsResponse { applications },
        page,
        per_page,
        total,
    ))
}

#[get("/applications/{id}")]
async fn read_application(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let application =
        ApplicationOps::get_by_id(pool.get_ref(), tenant_id(tenant), path.into_inner())
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(found(application, "Application"))
}

#[post("/applications")]
async fn create_application(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<CreateApplicationRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    created_or_error(ApplicationOps::create(pool.get_ref(), tenant_id(tenant), &body).await)
}

#[put("/applications/{id}")]
async fn update_application(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateApplicationRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    updated_or_error(
        ApplicationOps::update(pool.get_ref(), tenant_id(tenant), path.into_inner(), &body).await,
        "Application",
    )
}

#[delete("/applications/{id}")]
async fn delete_application(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let outcome = ApplicationOps::delete(pool.get_ref(), tenant_id(tenant), path.into_inner())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(delete_response(
        outcome,
        "Application",
        "Only an unused draft application can be removed.",
    ))
}

#[get("/enrolments")]
async fn list_enrolments(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<EnrolmentListQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    let (enrolments, total) = EnrolmentOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        query.status.map(crate::dtos::EnrolmentStatus::as_str),
        query.academic_year_id,
        query.class_group_id,
        query.learner_id,
    )
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(paginated(
        PaginatedEnrolmentsResponse { enrolments },
        page,
        per_page,
        total,
    ))
}

#[get("/enrolments/{id}")]
async fn read_enrolment(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let enrolment = EnrolmentOps::get_by_id(pool.get_ref(), tenant_id(tenant), path.into_inner())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(found(enrolment, "Enrolment"))
}

#[post("/enrolments")]
async fn create_enrolment(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<CreateEnrolmentRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    created_or_error(EnrolmentOps::create(pool.get_ref(), tenant_id(tenant), &body).await)
}

#[put("/enrolments/{id}")]
async fn update_enrolment(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateEnrolmentRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    updated_or_error(
        EnrolmentOps::update(pool.get_ref(), tenant_id(tenant), path.into_inner(), &body).await,
        "Enrolment",
    )
}

fn tenant_id(tenant: web::ReqData<TenantId>) -> Uuid {
    tenant.into_inner().0
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).max(1),
        per_page.unwrap_or(25).clamp(1, 100),
    )
}

fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
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

fn ok<T: Serialize>(value: T) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::from_status(StatusCode::OK, Some(value), None))
}

fn paginated<T: Serialize>(value: T, page: i64, per_page: i64, total: i64) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::with_pagination(
        StatusCode::OK,
        Some(value),
        PaginationMeta::new(page as u32, per_page as u32, total),
        None,
    ))
}

fn found<T: Serialize>(value: Option<T>, label: &str) -> HttpResponse {
    value.map_or_else(|| not_found(label), ok)
}

fn not_found(label: &str) -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::from_status(
        StatusCode::NOT_FOUND,
        None::<()>,
        Some(vec![format!("{label} not found")]),
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

fn updated_or_error<T: Serialize>(result: anyhow::Result<Option<T>>, label: &str) -> HttpResponse {
    match result {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found(label),
        Err(error) => operation_error(error),
    }
}

fn operation_error(error: anyhow::Error) -> HttpResponse {
    if let Some(database) = error.root_cause().downcast_ref::<sqlx::Error>() {
        if let sqlx::Error::Database(database) = database
            && database.code().as_deref() == Some("23505")
        {
            return HttpResponse::Conflict().json(ApiResponse::from_status(
                StatusCode::CONFLICT,
                None::<()>,
                Some(vec![
                    "That SIS record conflicts with an existing record.".to_string(),
                ]),
            ));
        }
        return HttpResponse::InternalServerError().json(ApiResponse::from_status(
            StatusCode::INTERNAL_SERVER_ERROR,
            None::<()>,
            Some(vec!["The SIS record could not be saved.".to_string()]),
        ));
    }
    bad_request(&error.to_string())
}

fn updated_value_or_error<T: Serialize>(result: anyhow::Result<T>) -> HttpResponse {
    match result {
        Ok(value) => ok(value),
        Err(error) => operation_error(error),
    }
}

fn internal_error(message: &str) -> HttpResponse {
    HttpResponse::InternalServerError().json(ApiResponse::from_status(
        StatusCode::INTERNAL_SERVER_ERROR,
        None::<()>,
        Some(vec![message.to_string()]),
    ))
}

async fn read_bounded_field(
    field: &mut actix_multipart::Field,
    maximum_bytes: usize,
) -> Result<Vec<u8>, &'static str> {
    let mut bytes = Vec::new();
    while let Some(chunk) = field.next().await {
        let chunk = chunk.map_err(|_| "The import upload could not be read.")?;
        if bytes.len().saturating_add(chunk.len()) > maximum_bytes {
            return Err("The import file exceeds the 5 MB limit.");
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn delete_response(outcome: DeleteOutcome, label: &str, in_use_message: &str) -> HttpResponse {
    match outcome {
        DeleteOutcome::Deleted => ok(serde_json::json!({ "deleted": true })),
        DeleteOutcome::NotFound => not_found(label),
        DeleteOutcome::InUse => HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec![in_use_message.to_string()]),
        )),
    }
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(RequirePermission::new("sis"))
            .service(read_learner_numbering_policy)
            .service(update_learner_numbering_policy)
            .service(list_account_candidates)
            .service(list_imports)
            .service(upload_import)
            .service(read_import)
            .service(preview_import_mapping)
            .service(read_import_preview)
            .service(commit_import)
            .service(list_learners)
            .service(read_learner)
            .service(create_learner)
            .service(update_learner)
            .service(link_learner_account)
            .service(delete_learner)
            .service(list_guardians)
            .service(read_guardian)
            .service(create_guardian)
            .service(update_guardian)
            .service(link_guardian_account)
            .service(delete_guardian)
            .service(list_guardian_relationships)
            .service(read_guardian_relationship)
            .service(create_guardian_relationship)
            .service(update_guardian_relationship)
            .service(delete_guardian_relationship)
            .service(list_applications)
            .service(read_application)
            .service(create_application)
            .service(update_application)
            .service(delete_application)
            .service(list_enrolments)
            .service(read_enrolment)
            .service(create_enrolment)
            .service(update_enrolment),
    );
}

#[cfg(test)]
mod tests {
    use super::{bounded_page, trimmed};

    #[test]
    fn sis_filters_are_bounded_and_blank_search_is_ignored() {
        assert_eq!(bounded_page(Some(-2), Some(500)), (1, 100));
        assert_eq!(bounded_page(None, None), (1, 25));
        assert_eq!(trimmed(Some("  ")), None);
        assert_eq!(trimmed(Some(" Learner  ")), Some("Learner"));
    }
}
