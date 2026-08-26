// Copyright (c) 2025-01-02 Codecraft Solutions
// Created: 2025-01-02
// Author: AI Assistant

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, delete, get, post, put, web};
use cp_common::TenantId;
use validator::Validate;

use crate::{
    middleware::{AuthMiddleware, RequirePermission},
    models::api_response::{ApiResponse, PaginationMeta},
    state::AppState,
    utils::flatten_validation_errors,
};

use super::{
    dtos::{CreateRoleRequest, ListRolesQuery, ListRolesResponse, RoleResponse, UpdateRoleRequest},
    ops::RoleOps,
};

#[get("")]
async fn list_roles(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<ListRolesQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(50).clamp(1, 100);

    let (roles, total) = match RoleOps::list_roles(
        &state.db,
        tenant_id,
        page,
        limit,
        query.query.as_deref(),
    )
    .await
    {
            Ok(data) => data,
            Err(e) => {
                log::error!("Failed to list roles: {:?}", e);
                return Ok(
                    HttpResponse::InternalServerError().json(ApiResponse::from_status(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        None::<()>,
                        None,
                    )),
                );
            }
        };

    let pagination = PaginationMeta::new(page, limit, total);

    let response = ListRolesResponse {
        roles: roles.into_iter().map(RoleResponse::from).collect(),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::with_pagination(
        StatusCode::OK,
        Some(response),
        pagination,
        None,
    )))
}

#[get("{id}")]
async fn get_role(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    id: web::Path<uuid::Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    let role = match RoleOps::get_role_by_id(&state.db, tenant_id, *id).await {
        Ok(Some(role)) => role,
        Ok(None) => {
            return Ok(HttpResponse::NotFound().json(ApiResponse::from_status(
                StatusCode::NOT_FOUND,
                None::<()>,
                None,
            )));
        }
        Err(e) => {
            log::error!("Failed to get role: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    None,
                )),
            );
        }
    };

    let response: RoleResponse = role.into();

    Ok(HttpResponse::Ok().json(ApiResponse::from_status(
        StatusCode::OK,
        Some(response),
        None,
    )))
}

#[post("")]
async fn create_role(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<CreateRoleRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();

    // Validate request
    if let Err(e) = body.validate() {
        let errors = flatten_validation_errors(&e);
        return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(errors),
        )));
    }

    // Check if role with same name already exists
    match RoleOps::get_role_by_name(&state.db, tenant_id, &body.name).await {
        Ok(Some(_)) => {
            return Ok(HttpResponse::Conflict().json(ApiResponse::from_status(
                StatusCode::CONFLICT,
                None::<()>,
                Some(vec!["Role with this name already exists".to_string()]),
            )));
        }
        Ok(None) => {}
        Err(e) => {
            log::error!("Failed to check role existence: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    None,
                )),
            );
        }
    }

    // Create role
    let role = match RoleOps::create_role(&state.db, tenant_id, &body).await {
        Ok(role) => role,
        Err(e) => {
            log::error!("Failed to create role: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    None,
                )),
            );
        }
    };

    let response: RoleResponse = role.into();

    Ok(HttpResponse::Created().json(ApiResponse::from_status(
        StatusCode::CREATED,
        Some(response),
        None,
    )))
}

#[put("{id}")]
async fn update_role(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    id: web::Path<uuid::Uuid>,
    body: web::Json<UpdateRoleRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();

    // Validate request
    if let Err(e) = body.validate() {
        let errors = flatten_validation_errors(&e);
        return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(errors),
        )));
    }

    // Check if updating name and if new name already exists
    if let Some(ref new_name) = body.name {
        match RoleOps::get_role_by_name(&state.db, tenant_id, new_name).await {
            Ok(Some(existing_role)) if existing_role.id != *id => {
                return Ok(HttpResponse::Conflict().json(ApiResponse::from_status(
                    StatusCode::CONFLICT,
                    None::<()>,
                    Some(vec!["Role with this name already exists".to_string()]),
                )));
            }
            Ok(_) => {}
            Err(e) => {
                log::error!("Failed to check role existence: {:?}", e);
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

    // Update role
    let role = match RoleOps::update_role(&state.db, tenant_id, *id, &body).await {
        Ok(Some(role)) => role,
        Ok(None) => {
            return Ok(HttpResponse::NotFound().json(ApiResponse::from_status(
                StatusCode::NOT_FOUND,
                None::<()>,
                Some(vec![
                    "Role not found or is a system role that cannot be modified".to_string(),
                ]),
            )));
        }
        Err(e) => {
            log::error!("Failed to update role: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    None,
                )),
            );
        }
    };

    let response: RoleResponse = role.into();

    Ok(HttpResponse::Ok().json(ApiResponse::from_status(
        StatusCode::OK,
        Some(response),
        None,
    )))
}

#[delete("{id}")]
async fn delete_role(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    id: web::Path<uuid::Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    match RoleOps::delete_role(&state.db, tenant_id, *id).await {
        Ok(true) => Ok(HttpResponse::Ok().json(ApiResponse::from_status(
            StatusCode::OK,
            Some(serde_json::json!({ "success": true })),
            None,
        ))),
        Ok(false) => Ok(HttpResponse::NotFound().json(ApiResponse::from_status(
            StatusCode::NOT_FOUND,
            None::<()>,
            Some(vec![
                "Role not found or is a system role that cannot be deleted".to_string(),
            ]),
        ))),
        Err(e) => {
            log::error!("Failed to delete role: {:?}", e);
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
        web::scope("/roles")
            // See users::routes::routes — AuthMiddleware must be registered
            // LAST so it runs FIRST (outermost), ahead of RequirePermission.
            .wrap(RequirePermission::new("roles"))
            .wrap(AuthMiddleware)
            .service(list_roles)
            .service(get_role)
            .service(create_role)
            .service(update_role)
            .service(delete_role),
    );
}
