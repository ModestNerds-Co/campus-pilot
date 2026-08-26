//
//  cp-vehicle-log
//  routes.rs
//
//  Created by Ngonidzashe Mangudya on 2026/08/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//
//  Auth is applied by the app crate at the `/vehicle-logs` scope mount
//  point; this module only gates individual actions with `RequirePermission`.

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, delete, get, post, put, web};
use cp_common::{
    ApiResponse, PaginationMeta, RequirePermission, TenantId, flatten_validation_errors,
};
use cp_fleet::ops::{DriverOps, VehicleOps};
use uuid::Uuid;
use validator::Validate;

use crate::dtos::{
    CreateVehicleDailyLogRequest, ListVehicleDailyLogsQuery, PaginatedVehicleDailyLogsResponse,
    UpdateVehicleDailyLogRequest, VehicleDailyLogResponse,
};
use crate::ops::VehicleDailyLogOps;

/// `None` when there's nothing to check (end reading absent), `Some(issue)`
/// when an end odometer reading is present and lower than the start reading.
fn odometer_issue(start: i32, end: Option<i32>) -> Option<Vec<String>> {
    match end {
        Some(end) if end < start => Some(vec![
            "End odometer can't be less than the start odometer".to_string(),
        ]),
        _ => None,
    }
}

/// Confirms a vehicle_id / driver_id pair belongs to this tenant before a
/// log entry is written against them — the one place this module reaches
/// into cp-fleet rather than just trusting the foreign key.
async fn validate_vehicle_and_driver(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    vehicle_id: Uuid,
    driver_id: Uuid,
) -> Result<Option<Vec<String>>, anyhow::Error> {
    let mut issues = Vec::new();

    if VehicleOps::get_by_id(pool, tenant_id, vehicle_id)
        .await?
        .is_none()
    {
        issues.push("Vehicle not found for this school".to_string());
    }
    if DriverOps::get_by_id(pool, tenant_id, driver_id)
        .await?
        .is_none()
    {
        issues.push("Driver not found for this school".to_string());
    }

    Ok(if issues.is_empty() {
        None
    } else {
        Some(issues)
    })
}

#[get("")]
async fn list_logs(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<ListVehicleDailyLogsQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 100);

    let (logs, total) = match VehicleDailyLogOps::list(
        &pool,
        tenant_id,
        page,
        per_page,
        query.vehicle_id,
        query.driver_id,
        query.status.as_deref(),
        query.from_date,
        query.to_date,
    )
    .await
    {
        Ok(result) => result,
        Err(e) => {
            log::error!("Failed to list vehicle daily logs: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    Some(vec!["Failed to fetch daily logs".to_string()]),
                )),
            );
        }
    };

    let pagination = PaginationMeta::new(page as u32, per_page as u32, total);
    let response = PaginatedVehicleDailyLogsResponse {
        logs: logs
            .into_iter()
            .map(VehicleDailyLogResponse::from)
            .collect(),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::with_pagination(
        StatusCode::OK,
        Some(response),
        pagination,
        None,
    )))
}

#[get("/{id}")]
async fn get_log(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    let log_entry = match VehicleDailyLogOps::get_by_id(&pool, tenant_id, path.into_inner()).await {
        Ok(Some(l)) => l,
        Ok(None) => {
            return Ok(HttpResponse::NotFound().json(ApiResponse::from_status(
                StatusCode::NOT_FOUND,
                None::<()>,
                Some(vec!["Daily log entry not found".to_string()]),
            )));
        }
        Err(e) => {
            log::error!("Failed to get vehicle daily log: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    None,
                )),
            );
        }
    };

    Ok(HttpResponse::Ok().json(ApiResponse::from_status(
        StatusCode::OK,
        Some(VehicleDailyLogResponse::from(log_entry)),
        None,
    )))
}

#[post("")]
async fn create_log(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<CreateVehicleDailyLogRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();

    if let Err(e) = body.validate() {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(flatten_validation_errors(&e)),
        )));
    }

    if let Some(issues) = odometer_issue(body.start_odometer, body.end_odometer) {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(issues),
        )));
    }

    match validate_vehicle_and_driver(&pool, tenant_id, body.vehicle_id, body.driver_id).await {
        Ok(Some(issues)) => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
                StatusCode::BAD_REQUEST,
                None::<()>,
                Some(issues),
            )));
        }
        Ok(None) => {}
        Err(e) => {
            log::error!("Failed to validate vehicle/driver: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    None,
                )),
            );
        }
    }

    let id = match VehicleDailyLogOps::create(&pool, tenant_id, &body).await {
        Ok(id) => id,
        Err(e) => {
            log::error!("Failed to create vehicle daily log: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    Some(vec!["Failed to create daily log entry".to_string()]),
                )),
            );
        }
    };

    let created = match VehicleDailyLogOps::get_by_id(&pool, tenant_id, id).await {
        Ok(Some(l)) => l,
        _ => {
            return Ok(HttpResponse::Created().json(ApiResponse::from_status(
                StatusCode::CREATED,
                Some(serde_json::json!({ "id": id })),
                None,
            )));
        }
    };

    Ok(HttpResponse::Created().json(ApiResponse::from_status(
        StatusCode::CREATED,
        Some(VehicleDailyLogResponse::from(created)),
        None,
    )))
}

#[put("/{id}")]
async fn update_log(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateVehicleDailyLogRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    let id = path.into_inner();

    if let Err(e) = body.validate() {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(flatten_validation_errors(&e)),
        )));
    }

    // Only checked when this request touches both readings — a partial update
    // that only sends one of the two can't be validated without the other,
    // already-stored value.
    if let Some(start) = body.start_odometer {
        if let Some(issues) = odometer_issue(start, body.end_odometer) {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
                StatusCode::BAD_REQUEST,
                None::<()>,
                Some(issues),
            )));
        }
    }

    if let (Some(vehicle_id), Some(driver_id)) = (body.vehicle_id, body.driver_id) {
        match validate_vehicle_and_driver(&pool, tenant_id, vehicle_id, driver_id).await {
            Ok(Some(issues)) => {
                return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
                    StatusCode::BAD_REQUEST,
                    None::<()>,
                    Some(issues),
                )));
            }
            Ok(None) => {}
            Err(e) => {
                log::error!("Failed to validate vehicle/driver: {:?}", e);
                return Ok(
                    HttpResponse::InternalServerError().json(ApiResponse::from_status(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        None::<()>,
                        None,
                    )),
                );
            }
        }
    }

    match VehicleDailyLogOps::update(&pool, tenant_id, id, &body).await {
        Ok(true) => {}
        Ok(false) => {
            return Ok(HttpResponse::NotFound().json(ApiResponse::from_status(
                StatusCode::NOT_FOUND,
                None::<()>,
                Some(vec!["Daily log entry not found".to_string()]),
            )));
        }
        Err(e) => {
            log::error!("Failed to update vehicle daily log: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    None,
                )),
            );
        }
    };

    match VehicleDailyLogOps::get_by_id(&pool, tenant_id, id).await {
        Ok(Some(l)) => Ok(HttpResponse::Ok().json(ApiResponse::from_status(
            StatusCode::OK,
            Some(VehicleDailyLogResponse::from(l)),
            None,
        ))),
        _ => Ok(HttpResponse::Ok().json(ApiResponse::from_status(
            StatusCode::OK,
            Some(serde_json::json!({ "success": true })),
            None,
        ))),
    }
}

#[delete("/{id}")]
async fn delete_log(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    match VehicleDailyLogOps::delete(&pool, tenant_id, path.into_inner()).await {
        Ok(true) => Ok(HttpResponse::Ok().json(ApiResponse::from_status(
            StatusCode::OK,
            Some(serde_json::json!({ "success": true })),
            None,
        ))),
        Ok(false) => Ok(HttpResponse::NotFound().json(ApiResponse::from_status(
            StatusCode::NOT_FOUND,
            None::<()>,
            Some(vec!["Daily log entry not found".to_string()]),
        ))),
        Err(e) => {
            log::error!("Failed to delete vehicle daily log: {:?}", e);
            Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    None,
                )),
            )
        }
    }
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(RequirePermission::new("vehicle-logs"))
            .service(list_logs)
            .service(get_log)
            .service(create_log)
            .service(update_log)
            .service(delete_log),
    );
}
