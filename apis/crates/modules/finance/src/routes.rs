//! Authenticated Finance reference, accounting-calendar, and journal routes.

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, delete, get, post, put, web};
use cp_audit::{AuditActor, RequestContext};
use cp_common::{
    ApiResponse, PaginationMeta, RequirePermission, TenantId, flatten_validation_errors,
};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;

use crate::journals::{
    CreateJournalRequest, DeleteJournalQuery, JournalDeleteOutcome, JournalListQuery, JournalOps,
    JournalVersionRequest, PaginatedJournalsResponse, RejectJournalRequest, ReverseJournalRequest,
    UpdateJournalRequest,
};
use crate::ledger::{
    AccountListQuery, AccountOps, CreateAccountRequest, CreateCurrencyRequest, CurrencyListQuery,
    CurrencyOps, DeleteOutcome, PaginatedAccountsResponse, PaginatedCurrenciesResponse,
    UpdateAccountRequest, UpdateCurrencyRequest,
};
use crate::periods::{
    AccountingPeriodOps, AccountingPeriodsResponse, CalendarOutcome, CreateFiscalYearRequest,
    FiscalYearListQuery, FiscalYearOps, PaginatedFiscalYearsResponse, UpdateFiscalYearRequest,
};
use crate::posting_requests::{
    ConvertPostingRequestRequest, PaginatedPostingRequestsResponse, PostingRequestListQuery,
    PostingRequestOps, RejectPostingRequestRequest,
};

#[get("/currencies")]
async fn list_currencies(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<CurrencyListQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    let (currencies, total) = CurrencyOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        query.status.as_deref(),
    )
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(paginated(
        PaginatedCurrenciesResponse { currencies },
        page,
        per_page,
        total,
    ))
}

#[get("/posting-requests")]
async fn list_posting_requests(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<PostingRequestListQuery>,
) -> HttpResponse {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match PostingRequestOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        trimmed(query.status.as_deref()),
        trimmed(query.source_module.as_deref()),
    )
    .await
    {
        Ok((posting_requests, total)) => paginated(
            PaginatedPostingRequestsResponse { posting_requests },
            page,
            per_page,
            total,
        ),
        Err(error) => operation_error(error),
    }
}

#[get("/posting-requests/{id}")]
async fn read_posting_request(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    match PostingRequestOps::get_by_id(pool.get_ref(), tenant_id(tenant), path.into_inner()).await {
        Ok(value) => found(value, "Posting request"),
        Err(error) => operation_error(error),
    }
}

#[post("/posting-requests/{id}/convert")]
async fn convert_posting_request(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ConvertPostingRequestRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    updated_or_error(
        PostingRequestOps::convert_to_journal(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
        "Posting request",
    )
}

#[post("/posting-requests/{id}/reject")]
async fn reject_posting_request(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<RejectPostingRequestRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    updated_or_error(
        PostingRequestOps::reject(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
        "Posting request",
    )
}

#[get("/currencies/{id}")]
async fn read_currency(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let value = CurrencyOps::get_by_id(pool.get_ref(), tenant_id(tenant), path.into_inner())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(found(value, "Currency"))
}

#[post("/currencies")]
async fn create_currency(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<CreateCurrencyRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    created_or_error(CurrencyOps::create(pool.get_ref(), tenant_id(tenant), &body).await)
}

#[put("/currencies/{id}")]
async fn update_currency(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateCurrencyRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    updated_or_error(
        CurrencyOps::update(pool.get_ref(), tenant_id(tenant), path.into_inner(), &body).await,
        "Currency",
    )
}

#[delete("/currencies/{id}")]
async fn delete_currency(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    delete_or_error(
        CurrencyOps::delete(pool.get_ref(), tenant_id(tenant), path.into_inner()).await,
        "Currency",
        "This currency is used by a finance account.",
    )
}

#[get("/accounts")]
async fn list_accounts(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<AccountListQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    let (accounts, total) = AccountOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        query.status.as_deref(),
        query.account_type.as_deref(),
        query.currency_mode.as_deref(),
    )
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(paginated(
        PaginatedAccountsResponse { accounts },
        page,
        per_page,
        total,
    ))
}

#[get("/accounts/{id}")]
async fn read_account(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let value = AccountOps::get_by_id(pool.get_ref(), tenant_id(tenant), path.into_inner())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(found(value, "Finance account"))
}

#[post("/accounts")]
async fn create_account(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<CreateAccountRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    created_or_error(AccountOps::create(pool.get_ref(), tenant_id(tenant), &body).await)
}

#[put("/accounts/{id}")]
async fn update_account(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateAccountRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    updated_or_error(
        AccountOps::update(pool.get_ref(), tenant_id(tenant), path.into_inner(), &body).await,
        "Finance account",
    )
}

#[delete("/accounts/{id}")]
async fn delete_account(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    delete_or_error(
        AccountOps::delete(pool.get_ref(), tenant_id(tenant), path.into_inner()).await,
        "Finance account",
        "Remove or move its child accounts first.",
    )
}

#[get("/fiscal-years")]
async fn list_fiscal_years(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<FiscalYearListQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    let (fiscal_years, total) = FiscalYearOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        query.status.as_deref(),
    )
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(paginated(
        PaginatedFiscalYearsResponse { fiscal_years },
        page,
        per_page,
        total,
    ))
}

#[get("/fiscal-years/{id}")]
async fn read_fiscal_year(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let value = FiscalYearOps::get_by_id(pool.get_ref(), tenant_id(tenant), path.into_inner())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(found(value, "Fiscal year"))
}

#[post("/fiscal-years")]
async fn create_fiscal_year(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<CreateFiscalYearRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    created_or_error(FiscalYearOps::create(pool.get_ref(), tenant_id(tenant), &body).await)
}

#[put("/fiscal-years/{id}")]
async fn update_fiscal_year(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateFiscalYearRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    updated_or_error(
        FiscalYearOps::update(pool.get_ref(), tenant_id(tenant), path.into_inner(), &body).await,
        "Fiscal year",
    )
}

#[delete("/fiscal-years/{id}")]
async fn delete_fiscal_year(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    match FiscalYearOps::delete(pool.get_ref(), tenant_id(tenant), path.into_inner()).await {
        Ok(CalendarOutcome::Changed) => ok(serde_json::json!({ "deleted": true })),
        Ok(CalendarOutcome::NotFound) => not_found("Fiscal year"),
        Err(error) => operation_error(error),
    }
}

#[post("/fiscal-years/{id}/open")]
async fn open_fiscal_year(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    updated_or_error(
        FiscalYearOps::open(pool.get_ref(), tenant_id(tenant), path.into_inner()).await,
        "Fiscal year",
    )
}

#[post("/fiscal-years/{id}/close")]
async fn close_fiscal_year(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    updated_or_error(
        FiscalYearOps::close(pool.get_ref(), tenant_id(tenant), path.into_inner()).await,
        "Fiscal year",
    )
}

#[get("/fiscal-years/{id}/periods")]
async fn list_accounting_periods(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let periods = AccountingPeriodOps::list(pool.get_ref(), tenant_id(tenant), path.into_inner())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(ok(AccountingPeriodsResponse { periods }))
}

#[post("/periods/{id}/close")]
async fn close_accounting_period(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    updated_or_error(
        AccountingPeriodOps::close(pool.get_ref(), tenant_id(tenant), path.into_inner()).await,
        "Accounting period",
    )
}

#[post("/periods/{id}/reopen")]
async fn reopen_accounting_period(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    updated_or_error(
        AccountingPeriodOps::reopen(pool.get_ref(), tenant_id(tenant), path.into_inner()).await,
        "Accounting period",
    )
}

#[get("/journals")]
async fn list_journals(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<JournalListQuery>,
) -> HttpResponse {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match JournalOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        query.status.as_deref(),
        query.starts_on,
        query.ends_on,
    )
    .await
    {
        Ok((journals, total)) => paginated(
            PaginatedJournalsResponse { journals },
            page,
            per_page,
            total,
        ),
        Err(error) => operation_error(error),
    }
}

#[get("/journals/{id}")]
async fn read_journal(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    match JournalOps::get_by_id(pool.get_ref(), tenant_id(tenant), path.into_inner()).await {
        Ok(value) => found(value, "Journal"),
        Err(error) => operation_error(error),
    }
}

#[get("/journals/{id}/validation")]
async fn validate_journal(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    match JournalOps::validation(pool.get_ref(), tenant_id(tenant), path.into_inner()).await {
        Ok(value) => found(value, "Journal"),
        Err(error) => operation_error(error),
    }
}

#[post("/journals")]
async fn create_journal(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    body: web::Json<CreateJournalRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    created_or_error(
        JournalOps::create(
            pool.get_ref(),
            tenant_id(tenant),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
    )
}

#[put("/journals/{id}")]
async fn update_journal(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateJournalRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    updated_or_error(
        JournalOps::update(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
        "Journal",
    )
}

#[delete("/journals/{id}")]
async fn delete_journal(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    query: web::Query<DeleteJournalQuery>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*query) {
        return response;
    }
    match JournalOps::delete(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        query.expected_version,
        actor.into_inner(),
        request_context.into_inner(),
    )
    .await
    {
        Ok(JournalDeleteOutcome::Deleted) => ok(serde_json::json!({ "deleted": true })),
        Ok(JournalDeleteOutcome::NotFound) => not_found("Journal"),
        Err(error) => operation_error(error),
    }
}

#[post("/journals/{id}/submit")]
async fn submit_journal(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<JournalVersionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    updated_or_error(
        JournalOps::submit(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
        "Journal",
    )
}

#[post("/journals/{id}/approve")]
async fn approve_journal(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<JournalVersionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    updated_or_error(
        JournalOps::approve(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
        "Journal",
    )
}

#[post("/journals/{id}/reject")]
async fn reject_journal(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<RejectJournalRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    updated_or_error(
        JournalOps::reject(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
        "Journal",
    )
}

#[post("/journals/{id}/post")]
async fn post_journal(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<JournalVersionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    updated_or_error(
        JournalOps::post(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
        "Journal",
    )
}

#[post("/journals/{id}/reverse")]
async fn reverse_journal(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ReverseJournalRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&*body) {
        return response;
    }
    updated_or_error(
        JournalOps::reverse(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body,
        )
        .await,
        "Journal",
    )
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
fn updated_or_error<T: Serialize>(result: anyhow::Result<Option<T>>, label: &str) -> HttpResponse {
    match result {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found(label),
        Err(error) => operation_error(error),
    }
}
fn delete_or_error(
    result: anyhow::Result<DeleteOutcome>,
    label: &str,
    in_use_message: &str,
) -> HttpResponse {
    match result {
        Ok(DeleteOutcome::Deleted) => ok(serde_json::json!({ "deleted": true })),
        Ok(DeleteOutcome::NotFound) => not_found(label),
        Ok(DeleteOutcome::InUse) => HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec![in_use_message.to_string()]),
        )),
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
fn operation_error(error: anyhow::Error) -> HttpResponse {
    let safe_message = error.to_string();
    if safe_message.starts_with("Journal changed") {
        return HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec![safe_message]),
        ));
    }
    if let Some(database) = error.root_cause().downcast_ref::<sqlx::Error>()
        && let sqlx::Error::Database(database) = database
        && database.code().as_deref() == Some("23505")
    {
        return HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec!["That finance record already exists.".to_string()]),
        ));
    }
    if safe_message.starts_with("Finance requires")
        || safe_message.starts_with("Choose another")
        || safe_message.starts_with("A single-currency")
        || safe_message.starts_with("A currency")
        || safe_message.starts_with("A parent")
        || safe_message.starts_with("A posting")
        || safe_message.starts_with("An account")
        || safe_message.starts_with("The parent")
        || safe_message.starts_with("Reporting currency")
        || safe_message.starts_with("Account ")
        || safe_message.starts_with("Currency ")
        || safe_message.starts_with("Fiscal year")
        || safe_message.starts_with("Only a")
        || safe_message.starts_with("Every accounting")
        || safe_message.starts_with("Accounting periods")
        || safe_message.starts_with("Open the fiscal")
        || safe_message.starts_with("Journal ")
        || safe_message.starts_with("Only a draft")
        || safe_message.starts_with("Only a submitted")
        || safe_message.starts_with("Only an approved")
        || safe_message.starts_with("Only a posted")
        || safe_message.starts_with("A journal")
        || safe_message.starts_with("A rejected journal")
        || safe_message.starts_with("A submitted journal")
        || safe_message.starts_with("A reporting-currency")
        || safe_message.starts_with("Foreign-currency")
        || safe_message.starts_with("Exchange rate")
        || safe_message.starts_with("This journal")
        || safe_message.starts_with("Rejection reason")
        || safe_message.starts_with("Reversal reason")
        || safe_message.starts_with("Idempotency key")
        || safe_message.starts_with("Source ")
        || safe_message.starts_with("Posting request")
        || safe_message.starts_with("The posting")
        || safe_message.starts_with("Every posting")
    {
        return HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(vec![safe_message]),
        ));
    }
    HttpResponse::InternalServerError().json(ApiResponse::from_status(
        StatusCode::INTERNAL_SERVER_ERROR,
        None::<()>,
        Some(vec!["The finance record could not be saved.".to_string()]),
    ))
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(RequirePermission::new("finance"))
            .service(list_currencies)
            .service(read_currency)
            .service(create_currency)
            .service(update_currency)
            .service(delete_currency)
            .service(list_accounts)
            .service(read_account)
            .service(create_account)
            .service(update_account)
            .service(delete_account)
            .service(list_fiscal_years)
            .service(read_fiscal_year)
            .service(create_fiscal_year)
            .service(update_fiscal_year)
            .service(delete_fiscal_year)
            .service(open_fiscal_year)
            .service(close_fiscal_year)
            .service(list_accounting_periods)
            .service(close_accounting_period)
            .service(reopen_accounting_period)
            .service(list_posting_requests)
            .service(read_posting_request)
            .service(convert_posting_request)
            .service(reject_posting_request)
            .service(list_journals)
            .service(read_journal)
            .service(validate_journal)
            .service(create_journal)
            .service(update_journal)
            .service(delete_journal)
            .service(submit_journal)
            .service(approve_journal)
            .service(reject_journal)
            .service(post_journal)
            .service(reverse_journal),
    );
}

#[cfg(test)]
mod tests {
    use super::{bounded_page, trimmed};
    #[test]
    fn filters_are_bounded_and_blank_search_is_ignored() {
        assert_eq!(bounded_page(Some(-2), Some(500)), (1, 100));
        assert_eq!(trimmed(Some("  ")), None);
        assert_eq!(trimmed(Some("USD")), Some("USD"));
    }
}
