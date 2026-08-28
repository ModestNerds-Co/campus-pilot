//! Authenticated Fees and Billing HTTP routes.
//!
//! The application mounts identity middleware outside this crate. The shared
//! operation evaluator enforces exact permissions, licensing, and module
//! dependencies before these handlers run.

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, delete, get, post, put, web};
use cp_audit::{AuditActor, RequestContext};
use cp_common::{
    AccessContext, ApiResponse, PaginationMeta, RequirePermission, TenantId,
    flatten_validation_errors,
};
use cp_sis::ops::LearnerOps;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;

use crate::foundation::{
    BillingAccountOps, CreateBillingAccountRequest, CreateFeeStructureRequest, DeleteOutcome,
    DirectoryQuery, FeeStructureOps, FeesReferenceOps, LearnerCandidateQuery,
    LearnerCandidatesResponse, PaginatedBillingAccountsResponse, PaginatedFeeStructuresResponse,
    UpdateBillingAccountRequest, UpdateFeeStructureRequest, VersionRequest,
};
use crate::invoices::{
    CreateInvoiceRequest, InvoiceDeleteOutcome, InvoiceListQuery, InvoiceOps, IssueInvoiceRequest,
    PaginatedInvoicesResponse,
};

#[get("/reference-data")]
async fn read_reference_data(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
) -> HttpResponse {
    value_or_error(FeesReferenceOps::load(pool.get_ref(), tenant_id(tenant)).await)
}

#[get("/learner-candidates")]
async fn list_learner_candidates(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<LearnerCandidateQuery>,
) -> HttpResponse {
    value_or_error(
        LearnerOps::billing_references(
            pool.get_ref(),
            tenant_id(tenant),
            trimmed(query.search.as_deref()),
            100,
        )
        .await
        .map(|learners| LearnerCandidatesResponse { learners }),
    )
}

#[get("/billing-accounts")]
async fn list_billing_accounts(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    access: web::ReqData<AccessContext>,
    query: web::Query<DirectoryQuery>,
) -> HttpResponse {
    let tenant_id = tenant_id(tenant);
    let visible_learner_ids = match billing_scope(
        pool.get_ref(),
        tenant_id,
        actor.into_inner(),
        access.into_inner(),
    )
    .await
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match BillingAccountOps::list(
        pool.get_ref(),
        tenant_id,
        page,
        per_page,
        trimmed(query.search.as_deref()),
        trimmed(query.status.as_deref()),
        visible_learner_ids.as_deref(),
    )
    .await
    {
        Ok((billing_accounts, total)) => paginated(
            PaginatedBillingAccountsResponse { billing_accounts },
            page,
            per_page,
            total,
        ),
        Err(error) => operation_error(error),
    }
}

#[get("/billing-accounts/{id}")]
async fn read_billing_account(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    access: web::ReqData<AccessContext>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let tenant_id = tenant_id(tenant);
    let visible_learner_ids = match billing_scope(
        pool.get_ref(),
        tenant_id,
        actor.into_inner(),
        access.into_inner(),
    )
    .await
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    match BillingAccountOps::get_by_id(
        pool.get_ref(),
        tenant_id,
        path.into_inner(),
        visible_learner_ids.as_deref(),
    )
    .await
    {
        Ok(value) => found(value, "Billing account"),
        Err(error) => operation_error(error),
    }
}

#[post("/billing-accounts")]
async fn create_billing_account(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    body: web::Json<CreateBillingAccountRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    created_or_error(
        BillingAccountOps::create(
            pool.get_ref(),
            tenant_id(tenant),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
    )
}

#[put("/billing-accounts/{id}")]
async fn update_billing_account(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateBillingAccountRequest>,
) -> HttpResponse {
    updated_or_error(
        BillingAccountOps::update(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
        "Billing account",
    )
}

#[get("/fee-structures")]
async fn list_fee_structures(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<DirectoryQuery>,
) -> HttpResponse {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match FeeStructureOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        trimmed(query.status.as_deref()),
    )
    .await
    {
        Ok((fee_structures, total)) => paginated(
            PaginatedFeeStructuresResponse { fee_structures },
            page,
            per_page,
            total,
        ),
        Err(error) => operation_error(error),
    }
}

#[get("/fee-structures/{id}")]
async fn read_fee_structure(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    match FeeStructureOps::get_by_id(pool.get_ref(), tenant_id(tenant), path.into_inner()).await {
        Ok(value) => found(value, "Fee structure"),
        Err(error) => operation_error(error),
    }
}

#[post("/fee-structures")]
async fn create_fee_structure(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    body: web::Json<CreateFeeStructureRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    created_or_error(
        FeeStructureOps::create(
            pool.get_ref(),
            tenant_id(tenant),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
    )
}

#[put("/fee-structures/{id}")]
async fn update_fee_structure(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateFeeStructureRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    updated_or_error(
        FeeStructureOps::update(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
        "Fee structure",
    )
}

#[delete("/fee-structures/{id}")]
async fn delete_fee_structure(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    query: web::Query<VersionRequest>,
) -> HttpResponse {
    match FeeStructureOps::delete(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor.into_inner(),
        request_context.into_inner(),
        query.expected_version,
    )
    .await
    {
        Ok(DeleteOutcome::Deleted) => ok(serde_json::json!({ "deleted": true })),
        Ok(DeleteOutcome::NotFound) => not_found("Fee structure"),
        Err(error) => operation_error(error),
    }
}

#[post("/fee-structures/{id}/activate")]
async fn activate_fee_structure(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<VersionRequest>,
) -> HttpResponse {
    updated_or_error(
        FeeStructureOps::activate(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            body.expected_version,
        )
        .await,
        "Fee structure",
    )
}

#[post("/fee-structures/{id}/retire")]
async fn retire_fee_structure(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<VersionRequest>,
) -> HttpResponse {
    updated_or_error(
        FeeStructureOps::retire(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            body.expected_version,
        )
        .await,
        "Fee structure",
    )
}

#[get("/invoices")]
async fn list_invoices(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    access: web::ReqData<AccessContext>,
    query: web::Query<InvoiceListQuery>,
) -> HttpResponse {
    let tenant_id = tenant_id(tenant);
    let visible_learner_ids = match billing_scope(
        pool.get_ref(),
        tenant_id,
        actor.into_inner(),
        access.into_inner(),
    )
    .await
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match InvoiceOps::list(
        pool.get_ref(),
        tenant_id,
        page,
        per_page,
        trimmed(query.search.as_deref()),
        trimmed(query.status.as_deref()),
        visible_learner_ids.as_deref(),
    )
    .await
    {
        Ok((invoices, total)) => paginated(
            PaginatedInvoicesResponse { invoices },
            page,
            per_page,
            total,
        ),
        Err(error) => operation_error(error),
    }
}

#[get("/invoices/{id}")]
async fn read_invoice(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    access: web::ReqData<AccessContext>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let tenant_id = tenant_id(tenant);
    let visible_learner_ids = match billing_scope(
        pool.get_ref(),
        tenant_id,
        actor.into_inner(),
        access.into_inner(),
    )
    .await
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    match InvoiceOps::get_by_id(
        pool.get_ref(),
        tenant_id,
        path.into_inner(),
        visible_learner_ids.as_deref(),
    )
    .await
    {
        Ok(value) => found(value, "Invoice"),
        Err(error) => operation_error(error),
    }
}

#[post("/invoices")]
async fn create_invoice(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    body: web::Json<CreateInvoiceRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    created_or_error(
        InvoiceOps::create(
            pool.get_ref(),
            tenant_id(tenant),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
    )
}

#[post("/invoices/{id}/issue")]
async fn issue_invoice(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<IssueInvoiceRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    updated_or_error(
        InvoiceOps::issue(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
        "Invoice",
    )
}

#[delete("/invoices/{id}")]
async fn delete_invoice(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    query: web::Query<VersionRequest>,
) -> HttpResponse {
    match InvoiceOps::delete(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        query.expected_version,
        actor.into_inner(),
        request_context.into_inner(),
    )
    .await
    {
        Ok(InvoiceDeleteOutcome::Deleted) => ok(serde_json::json!({ "deleted": true })),
        Ok(InvoiceDeleteOutcome::NotFound) => not_found("Invoice"),
        Err(error) => operation_error(error),
    }
}

async fn billing_scope(
    pool: &PgPool,
    tenant_id: Uuid,
    actor: AuditActor,
    access: AccessContext,
) -> Result<Option<Vec<Uuid>>, HttpResponse> {
    if access.has_permission("*")
        || access.has_permission("fees:create")
        || access.has_permission("fees:edit")
    {
        return Ok(None);
    }
    let Some(account_id) = actor.user_id() else {
        return Err(HttpResponse::Unauthorized().json(ApiResponse::from_status(
            StatusCode::UNAUTHORIZED,
            None::<()>,
            Some(vec!["Authenticated account is required.".to_string()]),
        )));
    };
    LearnerOps::ids_for_linked_account(pool, tenant_id, account_id)
        .await
        .map(Some)
        .map_err(|_| internal_error())
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

fn found<T: Serialize>(value: Option<T>, label: &str) -> HttpResponse {
    value.map_or_else(|| not_found(label), ok)
}

fn ok<T: Serialize>(value: T) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::from_status(StatusCode::OK, Some(value), None))
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

fn value_or_error<T: Serialize>(result: anyhow::Result<T>) -> HttpResponse {
    result.map_or_else(operation_error, ok)
}

fn updated_or_error<T: Serialize>(result: anyhow::Result<Option<T>>, label: &str) -> HttpResponse {
    match result {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found(label),
        Err(error) => operation_error(error),
    }
}

fn not_found(label: &str) -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::from_status(
        StatusCode::NOT_FOUND,
        None::<()>,
        Some(vec![format!("{label} was not found.")]),
    ))
}

fn internal_error() -> HttpResponse {
    HttpResponse::InternalServerError().json(ApiResponse::from_status(
        StatusCode::INTERNAL_SERVER_ERROR,
        None::<()>,
        Some(vec!["The fees record could not be loaded.".to_string()]),
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
    if let Some(database) = error.root_cause().downcast_ref::<sqlx::Error>()
        && let sqlx::Error::Database(database) = database
        && database.code().as_deref() == Some("23505")
    {
        return HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec!["That fees record already exists.".to_string()]),
        ));
    }
    let is_operational = [
        "A ",
        "An ",
        "Billing ",
        "Fee ",
        "Idempotency ",
        "Only ",
        "Receivable ",
        "Revenue ",
        "The ",
        "This learner ",
    ]
    .iter()
    .any(|prefix| message.starts_with(prefix));
    if is_operational {
        return HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(vec![message]),
        ));
    }
    internal_error()
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(RequirePermission::new("fees"))
            .service(read_reference_data)
            .service(list_learner_candidates)
            .service(list_billing_accounts)
            .service(read_billing_account)
            .service(create_billing_account)
            .service(update_billing_account)
            .service(list_fee_structures)
            .service(read_fee_structure)
            .service(create_fee_structure)
            .service(update_fee_structure)
            .service(delete_fee_structure)
            .service(activate_fee_structure)
            .service(retire_fee_structure)
            .service(list_invoices)
            .service(read_invoice)
            .service(create_invoice)
            .service(issue_invoice)
            .service(delete_invoice),
    );
}
