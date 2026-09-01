//! Authenticated Library routes over licensed, exact, and record-scoped operations.

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
    AssessFineRequest, BorrowingListQuery, CheckoutRequest, CopiesPage, CopyListQuery,
    CreateCopyRequest, CreateMembershipRequest, CreateTitleRequest, DirectoryQuery, FinesPage,
    HoldsPage, LibraryAccessScope, LoansPage, MembershipsPage, PlaceHoldRequest, ReadyHoldRequest,
    ReasonedVersionRequest, RenewLoanRequest, ReturnLoanRequest, SubmitFineRequest, TitlesPage,
    UpdateCopyRequest, UpdateLibrarySettingsRequest, UpdateMembershipRequest, UpdateTitleRequest,
    VersionRequest, catalogue::LibraryCatalogueOps, circulation::LibraryCirculationOps,
    fines::LibraryFineOps, members::LibraryMemberOps, settings::LibrarySettingsOps,
};

type Authority = (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>);

#[get("/settings")]
async fn read_settings(pool: web::Data<PgPool>, tenant: web::ReqData<TenantId>) -> HttpResponse {
    value_or_error(LibrarySettingsOps::get(pool.get_ref(), tenant_id(tenant)).await)
}

#[put("/settings")]
async fn update_settings(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    body: web::Json<UpdateLibrarySettingsRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "library.members") {
        return forbidden();
    }
    value_or_error(
        LibrarySettingsOps::update(
            pool.get_ref(),
            tenant_id(tenant),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/references")]
async fn references(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    query: web::Query<DirectoryQuery>,
) -> HttpResponse {
    if !is_campus_scope(&authority, actor.clone().into_inner(), "library.members") {
        return forbidden();
    }
    value_or_error(
        LibraryMemberOps::reference_data(
            pool.get_ref(),
            tenant_id(tenant),
            trimmed(query.search.as_deref()),
        )
        .await,
    )
}

#[get("/titles")]
async fn list_titles(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<DirectoryQuery>,
) -> HttpResponse {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match LibraryCatalogueOps::list_titles(pool.get_ref(), tenant_id(tenant), &query).await {
        Ok((titles, total)) => paginated(TitlesPage { titles }, page, per_page, total),
        Err(error) => operation_error(error),
    }
}

#[post("/titles")]
async fn create_title(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    body: web::Json<CreateTitleRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    created_or_error(
        LibraryCatalogueOps::create_title(
            pool.get_ref(),
            tenant_id(tenant),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/titles/{id}")]
async fn read_title(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    found(
        LibraryCatalogueOps::get_title(pool.get_ref(), tenant_id(tenant), path.into_inner()).await,
        "Library title",
    )
}

#[put("/titles/{id}")]
async fn update_title(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateTitleRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        LibraryCatalogueOps::update_title(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
        "Library title",
    )
}

#[post("/titles/{id}/retire")]
async fn retire_title(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<VersionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        LibraryCatalogueOps::retire_title(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            body.expected_version,
        )
        .await,
        "Library title",
    )
}

#[get("/titles/{id}/copies")]
async fn list_copies(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    query: web::Query<CopyListQuery>,
) -> HttpResponse {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match LibraryCatalogueOps::list_copies(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        page,
        per_page,
        query.status.as_deref(),
    )
    .await
    {
        Ok((copies, total)) => paginated(CopiesPage { copies }, page, per_page, total),
        Err(error) => operation_error(error),
    }
}

#[post("/titles/{id}/copies")]
async fn create_copy(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CreateCopyRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    created_or_error(
        LibraryCatalogueOps::create_copy(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/copies/{id}")]
async fn read_copy(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    found(
        LibraryCatalogueOps::get_copy(pool.get_ref(), tenant_id(tenant), path.into_inner()).await,
        "Library copy",
    )
}

#[put("/copies/{id}")]
async fn update_copy(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateCopyRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        LibraryCatalogueOps::update_copy(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
        "Library copy",
    )
}

#[post("/copies/{id}/retire")]
async fn retire_copy(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<VersionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        LibraryCatalogueOps::retire_copy(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            body.expected_version,
        )
        .await,
        "Library copy",
    )
}

#[get("/members")]
async fn list_members(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    query: web::Query<DirectoryQuery>,
) -> HttpResponse {
    let Ok(scope) = library_scope(&authority, actor.clone().into_inner(), "library.members") else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match LibraryMemberOps::list(pool.get_ref(), tenant_id(tenant), scope, &query).await {
        Ok((memberships, total)) => {
            paginated(MembershipsPage { memberships }, page, per_page, total)
        }
        Err(error) => operation_error(error),
    }
}

#[post("/members")]
async fn create_member(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    body: web::Json<CreateMembershipRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "library.members") {
        return forbidden();
    }
    created_or_error(
        LibraryMemberOps::create(
            pool.get_ref(),
            tenant_id(tenant),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/members/{id}")]
async fn read_member(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Ok(scope) = library_scope(&authority, actor.clone().into_inner(), "library.members") else {
        return forbidden();
    };
    found(
        LibraryMemberOps::get(pool.get_ref(), tenant_id(tenant), path.into_inner(), scope).await,
        "Library membership",
    )
}

#[put("/members/{id}")]
async fn update_member(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateMembershipRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "library.members") {
        return forbidden();
    }
    updated_or_error(
        LibraryMemberOps::update(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
        "Library membership",
    )
}

#[get("/loans")]
async fn list_loans(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    query: web::Query<BorrowingListQuery>,
) -> HttpResponse {
    let Ok(scope) = library_scope(&authority, actor.clone().into_inner(), "library.borrowing")
    else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match LibraryCirculationOps::list_loans(pool.get_ref(), tenant_id(tenant), scope, &query).await
    {
        Ok((loans, total)) => paginated(LoansPage { loans }, page, per_page, total),
        Err(error) => operation_error(error),
    }
}

#[post("/loans")]
async fn checkout(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    body: web::Json<CheckoutRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "library.borrowing") {
        return forbidden();
    }
    created_or_error(
        LibraryCirculationOps::checkout(
            pool.get_ref(),
            tenant_id(tenant),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/loans/{id}")]
async fn read_loan(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Ok(scope) = library_scope(&authority, actor.clone().into_inner(), "library.borrowing")
    else {
        return forbidden();
    };
    found(
        LibraryCirculationOps::get_loan(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            scope,
        )
        .await,
        "Library loan",
    )
}

#[post("/loans/{id}/renew")]
async fn renew_loan(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<RenewLoanRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let Ok(scope) = library_scope(&authority, actor.clone().into_inner(), "library.borrowing")
    else {
        return forbidden();
    };
    updated_or_error(
        LibraryCirculationOps::renew(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
        "Library loan",
    )
}

#[post("/loans/{id}/return")]
async fn return_loan(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ReturnLoanRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "library.borrowing") {
        return forbidden();
    }
    updated_or_error(
        LibraryCirculationOps::return_loan(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
        "Library loan",
    )
}

#[post("/loans/{id}/lost")]
async fn mark_loan_lost(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ReasonedVersionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "library.borrowing") {
        return forbidden();
    }
    updated_or_error(
        LibraryCirculationOps::mark_lost(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
        "Library loan",
    )
}

#[get("/holds")]
async fn list_holds(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    query: web::Query<BorrowingListQuery>,
) -> HttpResponse {
    let Ok(scope) = library_scope(&authority, actor.clone().into_inner(), "library.borrowing")
    else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match LibraryCirculationOps::list_holds(pool.get_ref(), tenant_id(tenant), scope, &query).await
    {
        Ok((holds, total)) => paginated(HoldsPage { holds }, page, per_page, total),
        Err(error) => operation_error(error),
    }
}

#[post("/holds")]
async fn place_hold(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    body: web::Json<PlaceHoldRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let Ok(scope) = library_scope(&authority, actor.clone().into_inner(), "library.borrowing")
    else {
        return forbidden();
    };
    created_or_error(
        LibraryCirculationOps::place_hold(
            pool.get_ref(),
            tenant_id(tenant),
            scope,
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/holds/{id}")]
async fn read_hold(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Ok(scope) = library_scope(&authority, actor.clone().into_inner(), "library.borrowing")
    else {
        return forbidden();
    };
    found(
        LibraryCirculationOps::get_hold(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            scope,
        )
        .await,
        "Library hold",
    )
}

#[post("/holds/{id}/ready")]
async fn ready_hold(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ReadyHoldRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "library.borrowing") {
        return forbidden();
    }
    updated_or_error(
        LibraryCirculationOps::ready_hold(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
        "Library hold",
    )
}

#[post("/holds/{id}/cancel")]
async fn cancel_hold(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ReasonedVersionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let Ok(scope) = library_scope(&authority, actor.clone().into_inner(), "library.borrowing")
    else {
        return forbidden();
    };
    updated_or_error(
        LibraryCirculationOps::cancel_hold(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
        "Library hold",
    )
}

#[post("/holds/{id}/expire")]
async fn expire_hold(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ReasonedVersionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "library.borrowing") {
        return forbidden();
    }
    updated_or_error(
        LibraryCirculationOps::expire_hold(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
        "Library hold",
    )
}

#[get("/fines")]
async fn list_fines(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    query: web::Query<BorrowingListQuery>,
) -> HttpResponse {
    let Ok(scope) = library_scope(&authority, actor.clone().into_inner(), "library.borrowing")
    else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match LibraryFineOps::list(pool.get_ref(), tenant_id(tenant), scope, &query).await {
        Ok((fines, total)) => paginated(FinesPage { fines }, page, per_page, total),
        Err(error) => operation_error(error),
    }
}

#[get("/fines/{id}")]
async fn read_fine(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Ok(scope) = library_scope(&authority, actor.clone().into_inner(), "library.borrowing")
    else {
        return forbidden();
    };
    found(
        LibraryFineOps::get(pool.get_ref(), tenant_id(tenant), path.into_inner(), scope).await,
        "Library fine",
    )
}

#[post("/loans/{id}/fines")]
async fn assess_fine(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<AssessFineRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "library.borrowing") {
        return forbidden();
    }
    created_or_error(
        LibraryFineOps::assess(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
    )
}

#[post("/fines/{id}/submit-to-fees")]
async fn submit_fine(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<SubmitFineRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "library.borrowing") {
        return forbidden();
    }
    updated_or_error(
        LibraryFineOps::submit_to_fees(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
        "Library fine",
    )
}

#[post("/fines/{id}/waive")]
async fn waive_fine(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ReasonedVersionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "library.borrowing") {
        return forbidden();
    }
    updated_or_error(
        LibraryFineOps::waive(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
        "Library fine",
    )
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(RequirePermission::new("library"))
            .service(read_settings)
            .service(update_settings)
            .service(references)
            .service(list_titles)
            .service(create_title)
            .service(read_title)
            .service(update_title)
            .service(retire_title)
            .service(list_copies)
            .service(create_copy)
            .service(read_copy)
            .service(update_copy)
            .service(retire_copy)
            .service(list_members)
            .service(create_member)
            .service(read_member)
            .service(update_member)
            .service(list_loans)
            .service(checkout)
            .service(read_loan)
            .service(renew_loan)
            .service(return_loan)
            .service(mark_loan_lost)
            .service(list_holds)
            .service(place_hold)
            .service(read_hold)
            .service(ready_hold)
            .service(cancel_hold)
            .service(expire_hold)
            .service(list_fines)
            .service(read_fine)
            .service(assess_fine)
            .service(submit_fine)
            .service(waive_fine),
    );
}

fn library_scope(
    authority: &Authority,
    actor: AuditActor,
    family: &str,
) -> Result<LibraryAccessScope, ()> {
    if authority.0.has_permission("*") {
        return Ok(LibraryAccessScope::Campus);
    }
    let family = RecordScopeFamilyKey::parse(family).map_err(|_| ())?;
    let account_id = actor.user_id().ok_or(())?;
    match authority.1.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Ok(LibraryAccessScope::Campus),
        Some(
            EffectiveRecordScope::SelfRecord
            | EffectiveRecordScope::Assigned
            | EffectiveRecordScope::SelfAndAssigned,
        ) => Ok(LibraryAccessScope::SelfFor(account_id)),
        None => Err(()),
    }
}
fn is_campus_scope(authority: &Authority, actor: AuditActor, family: &str) -> bool {
    matches!(
        library_scope(authority, actor, family),
        Ok(LibraryAccessScope::Campus)
    )
}
fn tenant_id(value: web::ReqData<TenantId>) -> Uuid {
    value.into_inner().into_inner()
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
fn paginated<T: Serialize>(value: T, page: i64, per_page: i64, total: i64) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::with_pagination(
        StatusCode::OK,
        Some(value),
        PaginationMeta::new(page as u32, per_page as u32, total),
        None,
    ))
}
fn ok<T: Serialize>(value: T) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::from_status(StatusCode::OK, Some(value), None))
}
fn found<T: Serialize>(result: anyhow::Result<Option<T>>, label: &str) -> HttpResponse {
    match result {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found(label),
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
fn updated_or_error<T: Serialize>(result: anyhow::Result<Option<T>>, label: &str) -> HttpResponse {
    match result {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found(label),
        Err(error) => operation_error(error),
    }
}
fn value_or_error<T: Serialize>(result: anyhow::Result<T>) -> HttpResponse {
    result.map_or_else(operation_error, ok)
}
fn not_found(label: &str) -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::from_status(
        StatusCode::NOT_FOUND,
        None::<()>,
        Some(vec![format!("{label} was not found.")]),
    ))
}
fn forbidden() -> HttpResponse {
    HttpResponse::Forbidden().json(ApiResponse::from_status(
        StatusCode::FORBIDDEN,
        None::<()>,
        Some(vec![
            "This Library record is outside your assigned scope.".to_string(),
        ]),
    ))
}
fn internal_error() -> HttpResponse {
    HttpResponse::InternalServerError().json(ApiResponse::from_status(
        StatusCode::INTERNAL_SERVER_ERROR,
        None::<()>,
        Some(vec!["The Library record could not be loaded.".to_string()]),
    ))
}
fn operation_error(error: anyhow::Error) -> HttpResponse {
    let message = error.to_string();
    if message.contains("changed since it was loaded") {
        return HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec![message]),
        ));
    }
    let operational = [
        "A ",
        "An ",
        "Choose ",
        "Checkout ",
        "Copies ",
        "Employee ",
        "Holds ",
        "ISBN ",
        "Language ",
        "Only ",
        "Provide ",
        "Renew",
        "Replacement ",
        "Resolve ",
        "Retire ",
        "Set ",
        "The ",
        "This ",
        "Use ",
    ]
    .iter()
    .any(|prefix| message.starts_with(prefix));
    if operational {
        return HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(vec![message]),
        ));
    }
    internal_error()
}
