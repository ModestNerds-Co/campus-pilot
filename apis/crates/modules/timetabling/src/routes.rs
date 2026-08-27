//! Timetabling HTTP transport over shared typed operations.

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, get, post, put, web};
use cp_common::{ApiResponse, RequirePermission, TenantId};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{models::TimetableConfiguration, ops::TimetablingOps};

#[get("/configuration")]
async fn get_configuration(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
) -> Result<HttpResponse, actix_web::Error> {
    let configuration =
        TimetablingOps::get_configuration(pool.get_ref(), tenant.into_inner().into_inner())
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(ok(Some(configuration)))
}

#[put("/configuration")]
async fn save_configuration(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<TimetableConfiguration>,
) -> HttpResponse {
    match TimetablingOps::save_configuration(
        pool.get_ref(),
        tenant.into_inner().into_inner(),
        body.into_inner(),
    )
    .await
    {
        Ok(configuration) => ok(Some(configuration)),
        Err(error) => bad_request_or_internal(error),
    }
}

#[post("/generate")]
async fn generate_timetable(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
) -> HttpResponse {
    match TimetablingOps::generate(pool.get_ref(), tenant.into_inner().into_inner()).await {
        Ok(run) => HttpResponse::Created().json(ApiResponse::from_status(
            StatusCode::CREATED,
            Some(run),
            None,
        )),
        Err(error) => bad_request_or_internal(error),
    }
}

#[get("/runs/latest")]
async fn latest_run(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
) -> Result<HttpResponse, actix_web::Error> {
    let run = TimetablingOps::latest_run(pool.get_ref(), tenant.into_inner().into_inner())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(ok(run))
}

#[put("/runs/{id}/publish")]
async fn publish_run(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    match TimetablingOps::publish(
        pool.get_ref(),
        tenant.into_inner().into_inner(),
        path.into_inner(),
    )
    .await
    {
        Ok(Some(run)) => ok(Some(run)),
        Ok(None) => HttpResponse::NotFound().json(ApiResponse::from_status(
            StatusCode::NOT_FOUND,
            None::<()>,
            Some(vec!["Timetable run not found".to_string()]),
        )),
        Err(error) => bad_request_or_internal(error),
    }
}

fn ok<T: serde::Serialize>(value: Option<T>) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::from_status(StatusCode::OK, value, None))
}

fn bad_request_or_internal(error: anyhow::Error) -> HttpResponse {
    let message = error.to_string();
    let operational = message.starts_with("Add ")
        || message.starts_with("Resolve ")
        || message.contains('\n')
        || message.contains("required")
        || message.contains("Configure ")
        || message.contains("references an unknown");
    if operational {
        HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(message.lines().map(str::to_string).collect()),
        ))
    } else {
        HttpResponse::InternalServerError().json(ApiResponse::from_status(
            StatusCode::INTERNAL_SERVER_ERROR,
            None::<()>,
            Some(vec![
                "Timetabling could not complete the request.".to_string(),
            ]),
        ))
    }
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(RequirePermission::new("timetabling"))
            .service(get_configuration)
            .service(save_configuration)
            .service(generate_timetable)
            .service(latest_run)
            .service(publish_run),
    );
}
