//
//  cp-fleet
//  routes.rs
//
//  Created by Ngonidzashe Mangudya on 2026/08/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//
//  Auth is applied by the app crate at the `/fleet` scope mount point;
//  this module only gates individual actions with `RequirePermission`.

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, delete, get, post, put, web};
use cp_common::{
    ApiResponse, PaginationMeta, RequirePermission, TenantId, flatten_validation_errors,
};
use uuid::Uuid;
use validator::Validate;

use crate::dtos::{
    CreateDriverRequest, CreateVehicleRequest, DriverCandidatesQuery, DriverCandidatesResponse,
    DriverResponse, ListDriversQuery, ListVehiclesQuery, PaginatedDriversResponse,
    PaginatedVehiclesResponse, UpdateDriverRequest, UpdateVehicleRequest, VehicleResponse,
};
use crate::ops::{DriverOps, VehicleOps};

// ---------------------------------------------------------------- vehicles

#[get("")]
async fn list_vehicles(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<ListVehiclesQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 100);

    let (vehicles, total) = match VehicleOps::list(
        &pool,
        tenant_id,
        page,
        per_page,
        query.search.as_deref(),
        query.status.as_deref(),
    )
    .await
    {
        Ok(result) => result,
        Err(e) => {
            log::error!("Failed to list vehicles: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    Some(vec!["Failed to fetch vehicles".to_string()]),
                )),
            );
        }
    };

    let pagination = PaginationMeta::new(page as u32, per_page as u32, total);
    let response = PaginatedVehiclesResponse {
        vehicles: vehicles.into_iter().map(VehicleResponse::from).collect(),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::with_pagination(
        StatusCode::OK,
        Some(response),
        pagination,
        None,
    )))
}

#[get("/{id}")]
async fn get_vehicle(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    let vehicle = match VehicleOps::get_by_id(&pool, tenant_id, path.into_inner()).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            return Ok(HttpResponse::NotFound().json(ApiResponse::from_status(
                StatusCode::NOT_FOUND,
                None::<()>,
                Some(vec!["Vehicle not found".to_string()]),
            )));
        }
        Err(e) => {
            log::error!("Failed to get vehicle: {:?}", e);
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
        Some(VehicleResponse::from(vehicle)),
        None,
    )))
}

#[post("")]
async fn create_vehicle(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<CreateVehicleRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();

    if let Err(e) = body.validate() {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(flatten_validation_errors(&e)),
        )));
    }

    match VehicleOps::registration_exists(&pool, tenant_id, &body.registration_number, None).await {
        Ok(true) => {
            return Ok(HttpResponse::Conflict().json(ApiResponse::from_status(
                StatusCode::CONFLICT,
                None::<()>,
                Some(vec![
                    "A vehicle with this registration number already exists".to_string(),
                ]),
            )));
        }
        Ok(false) => {}
        Err(e) => {
            log::error!("Failed to check registration existence: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    None,
                )),
            );
        }
    }

    let vehicle = match VehicleOps::create(&pool, tenant_id, &body).await {
        Ok(v) => v,
        Err(e) => {
            log::error!("Failed to create vehicle: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    Some(vec!["Failed to create vehicle".to_string()]),
                )),
            );
        }
    };

    Ok(HttpResponse::Created().json(ApiResponse::from_status(
        StatusCode::CREATED,
        Some(VehicleResponse::from(vehicle)),
        None,
    )))
}

#[put("/{id}")]
async fn update_vehicle(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateVehicleRequest>,
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

    if let Some(ref reg) = body.registration_number {
        match VehicleOps::registration_exists(&pool, tenant_id, reg, Some(id)).await {
            Ok(true) => {
                return Ok(HttpResponse::Conflict().json(ApiResponse::from_status(
                    StatusCode::CONFLICT,
                    None::<()>,
                    Some(vec![
                        "A vehicle with this registration number already exists".to_string(),
                    ]),
                )));
            }
            Ok(false) => {}
            Err(e) => {
                log::error!("Failed to check registration existence: {:?}", e);
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

    let vehicle = match VehicleOps::update(&pool, tenant_id, id, &body).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            return Ok(HttpResponse::NotFound().json(ApiResponse::from_status(
                StatusCode::NOT_FOUND,
                None::<()>,
                Some(vec!["Vehicle not found".to_string()]),
            )));
        }
        Err(e) => {
            log::error!("Failed to update vehicle: {:?}", e);
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
        Some(VehicleResponse::from(vehicle)),
        None,
    )))
}

#[delete("/{id}")]
async fn delete_vehicle(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    match VehicleOps::delete(&pool, tenant_id, path.into_inner()).await {
        Ok(true) => Ok(HttpResponse::Ok().json(ApiResponse::from_status(
            StatusCode::OK,
            Some(serde_json::json!({ "success": true })),
            None,
        ))),
        Ok(false) => Ok(HttpResponse::NotFound().json(ApiResponse::from_status(
            StatusCode::NOT_FOUND,
            None::<()>,
            Some(vec!["Vehicle not found".to_string()]),
        ))),
        Err(e) => {
            log::error!("Failed to delete vehicle: {:?}", e);
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

// ----------------------------------------------------------------- drivers

#[get("")]
async fn list_driver_candidates(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<DriverCandidatesQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let candidates = DriverOps::list_candidates(
        &pool,
        tenant.into_inner().into_inner(),
        query.search.as_deref(),
    )
    .await
    .map_err(|error| {
        log::error!("Failed to list driver candidates: {error:#}");
        actix_web::error::ErrorInternalServerError("Driver candidates could not be loaded")
    })?;
    Ok(HttpResponse::Ok().json(ApiResponse::from_status(
        StatusCode::OK,
        Some(DriverCandidatesResponse {
            employees: candidates,
        }),
        None,
    )))
}

#[get("")]
async fn list_drivers(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<ListDriversQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 100);

    let (drivers, total) = match DriverOps::list(
        &pool,
        tenant_id,
        page,
        per_page,
        query.search.as_deref(),
        query.status.as_deref(),
    )
    .await
    {
        Ok(result) => result,
        Err(e) => {
            log::error!("Failed to list drivers: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    Some(vec!["Failed to fetch drivers".to_string()]),
                )),
            );
        }
    };

    let pagination = PaginationMeta::new(page as u32, per_page as u32, total);
    let response = PaginatedDriversResponse {
        drivers: drivers.into_iter().map(DriverResponse::from).collect(),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::with_pagination(
        StatusCode::OK,
        Some(response),
        pagination,
        None,
    )))
}

#[get("/{id}")]
async fn get_driver(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    let driver = match DriverOps::get_by_id(&pool, tenant_id, path.into_inner()).await {
        Ok(Some(d)) => d,
        Ok(None) => {
            return Ok(HttpResponse::NotFound().json(ApiResponse::from_status(
                StatusCode::NOT_FOUND,
                None::<()>,
                Some(vec!["Driver not found".to_string()]),
            )));
        }
        Err(e) => {
            log::error!("Failed to get driver: {:?}", e);
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
        Some(DriverResponse::from(driver)),
        None,
    )))
}

#[post("")]
async fn create_driver(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<CreateDriverRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();

    if let Err(e) = body.validate() {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(flatten_validation_errors(&e)),
        )));
    }

    match DriverOps::license_exists(&pool, tenant_id, &body.license_number, None).await {
        Ok(true) => {
            return Ok(HttpResponse::Conflict().json(ApiResponse::from_status(
                StatusCode::CONFLICT,
                None::<()>,
                Some(vec![
                    "A driver with this license number already exists".to_string(),
                ]),
            )));
        }
        Ok(false) => {}
        Err(e) => {
            log::error!("Failed to check license existence: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    None,
                )),
            );
        }
    }

    let driver = match DriverOps::create(&pool, tenant_id, &body).await {
        Ok(d) => d,
        Err(e) => {
            log::error!("Failed to create driver: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    Some(vec!["Failed to create driver".to_string()]),
                )),
            );
        }
    };

    Ok(HttpResponse::Created().json(ApiResponse::from_status(
        StatusCode::CREATED,
        Some(DriverResponse::from(driver)),
        None,
    )))
}

#[put("/{id}")]
async fn update_driver(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateDriverRequest>,
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

    if let Some(ref license) = body.license_number {
        match DriverOps::license_exists(&pool, tenant_id, license, Some(id)).await {
            Ok(true) => {
                return Ok(HttpResponse::Conflict().json(ApiResponse::from_status(
                    StatusCode::CONFLICT,
                    None::<()>,
                    Some(vec![
                        "A driver with this license number already exists".to_string(),
                    ]),
                )));
            }
            Ok(false) => {}
            Err(e) => {
                log::error!("Failed to check license existence: {:?}", e);
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

    let driver = match DriverOps::update(&pool, tenant_id, id, &body).await {
        Ok(Some(d)) => d,
        Ok(None) => {
            return Ok(HttpResponse::NotFound().json(ApiResponse::from_status(
                StatusCode::NOT_FOUND,
                None::<()>,
                Some(vec!["Driver not found".to_string()]),
            )));
        }
        Err(e) => {
            log::error!("Failed to update driver: {:?}", e);
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
        Some(DriverResponse::from(driver)),
        None,
    )))
}

#[delete("/{id}")]
async fn delete_driver(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    match DriverOps::delete(&pool, tenant_id, path.into_inner()).await {
        Ok(true) => Ok(HttpResponse::Ok().json(ApiResponse::from_status(
            StatusCode::OK,
            Some(serde_json::json!({ "success": true })),
            None,
        ))),
        Ok(false) => Ok(HttpResponse::NotFound().json(ApiResponse::from_status(
            StatusCode::NOT_FOUND,
            None::<()>,
            Some(vec!["Driver not found".to_string()]),
        ))),
        Err(e) => {
            log::error!("Failed to delete driver: {:?}", e);
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
        web::scope("/vehicles")
            .wrap(RequirePermission::new("fleet"))
            .service(list_vehicles)
            .service(get_vehicle)
            .service(create_vehicle)
            .service(update_vehicle)
            .service(delete_vehicle),
    )
    .service(
        web::scope("/driver-candidates")
            .wrap(RequirePermission::new("fleet"))
            .service(list_driver_candidates),
    )
    .service(
        web::scope("/drivers")
            .wrap(RequirePermission::new("fleet"))
            .service(list_drivers)
            .service(get_driver)
            .service(create_driver)
            .service(update_driver)
            .service(delete_driver),
    );
}
