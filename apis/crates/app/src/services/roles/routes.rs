//! Exposes tenant-scoped role management with delegation-safe authorization.
//!
//! The authenticated operator may only create, edit, or delete role access
//! that is contained within their own effective permissions.

use std::collections::BTreeSet;

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, delete, get, post, put, web};
use cp_common::{AccessContext, TenantId};
use validator::Validate;

use crate::services::access::catalog::all_permission_keys;
use crate::{
    middleware::{AuthMiddleware, RequirePermission},
    models::api_response::{ApiResponse, PaginationMeta},
    state::AppState,
    utils::flatten_validation_errors,
};

use super::{
    dtos::{CreateRoleRequest, ListRolesQuery, ListRolesResponse, RoleResponse, UpdateRoleRequest},
    ops::{DeleteRoleOutcome, RoleOps},
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
    access: web::ReqData<AccessContext>,
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

    let mut request = body.into_inner();
    request.name = request.name.trim().to_string();
    request.description = normalize_description(request.description);
    request.permissions = canonical_values(request.permissions);
    if request.name.is_empty() || request.permissions.is_empty() {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(vec![
                "A role name and at least one permission are required".to_string(),
            ]),
        )));
    }

    let known_permissions = all_permission_keys();
    let invalid_permissions: Vec<&String> = request
        .permissions
        .iter()
        .filter(|permission| !known_permissions.contains(permission))
        .collect();
    if !invalid_permissions.is_empty() {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(vec![
                "One or more permissions are not part of the Campus Pilot catalog".to_string(),
            ]),
        )));
    }
    if !access.can_delegate_permissions(&request.permissions) {
        return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
            StatusCode::FORBIDDEN,
            None::<()>,
            Some(vec![
                "Only the Campus Owner can grant full access".to_string(),
            ]),
        )));
    }

    // Check if role with same name already exists
    match RoleOps::get_role_by_name(&state.db, tenant_id, &request.name).await {
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
    let role = match RoleOps::create_role(&state.db, tenant_id, &request).await {
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
    access: web::ReqData<AccessContext>,
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

    let role_id = *id;
    let current_role = match RoleOps::get_role_by_id(&state.db, tenant_id, role_id).await {
        Ok(Some(role)) => role,
        Ok(None) => {
            return Ok(HttpResponse::NotFound().json(ApiResponse::from_status(
                StatusCode::NOT_FOUND,
                None::<()>,
                Some(vec!["Role was not found".to_string()]),
            )));
        }
        Err(error) => {
            log::error!("Failed to load role before update: {:?}", error);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    Some(vec!["Role access could not be checked".to_string()]),
                )),
            );
        }
    };
    if !access.can_delegate_permissions(&current_role.permissions) {
        return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
            StatusCode::FORBIDDEN,
            None::<()>,
            Some(vec![
                "You cannot change a role with access beyond your own".to_string(),
            ]),
        )));
    }

    let mut request = body.into_inner();
    request.name = request.name.map(|name| name.trim().to_string());
    request.description = normalize_optional_description(request.description);
    request.permissions = request.permissions.map(canonical_values);
    if request.name.as_ref().is_some_and(String::is_empty)
        || request.permissions.as_ref().is_some_and(Vec::is_empty)
        || request
            .description
            .as_ref()
            .and_then(|value| value.as_ref())
            .is_some_and(|value| value.len() > 1000)
    {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(vec!["Role details are invalid".to_string()]),
        )));
    }

    if let Some(permissions) = request.permissions.as_ref() {
        let known_permissions = all_permission_keys();
        if permissions
            .iter()
            .any(|permission| !known_permissions.contains(permission))
        {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
                StatusCode::BAD_REQUEST,
                None::<()>,
                Some(vec![
                    "One or more permissions are not part of the Campus Pilot catalog".to_string(),
                ]),
            )));
        }
        if !access.can_delegate_permissions(permissions) {
            return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
                StatusCode::FORBIDDEN,
                None::<()>,
                Some(vec![
                    "Only the Campus Owner can grant full access".to_string(),
                ]),
            )));
        }
    }

    // Check if updating name and if new name already exists
    if let Some(ref new_name) = request.name {
        match RoleOps::get_role_by_name(&state.db, tenant_id, new_name).await {
            Ok(Some(existing_role)) if existing_role.id != role_id => {
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
    let role = match RoleOps::update_role(&state.db, tenant_id, role_id, &request).await {
        Ok(Some(role)) => role,
        Ok(None) => {
            return Ok(HttpResponse::NotFound().json(ApiResponse::from_status(
                StatusCode::NOT_FOUND,
                None::<()>,
                Some(vec!["Role was not found".to_string()]),
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
    access: web::ReqData<AccessContext>,
    id: web::Path<uuid::Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    let role_id = *id;
    let role = match RoleOps::get_role_by_id(&state.db, tenant_id, role_id).await {
        Ok(Some(role)) => role,
        Ok(None) => {
            return Ok(HttpResponse::NotFound().json(ApiResponse::from_status(
                StatusCode::NOT_FOUND,
                None::<()>,
                Some(vec!["Role was not found".to_string()]),
            )));
        }
        Err(error) => {
            log::error!("Failed to load role before deletion: {:?}", error);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    Some(vec!["Role access could not be checked".to_string()]),
                )),
            );
        }
    };
    if !access.can_delegate_permissions(&role.permissions) {
        return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
            StatusCode::FORBIDDEN,
            None::<()>,
            Some(vec![
                "You cannot delete a role with access beyond your own".to_string(),
            ]),
        )));
    }

    match RoleOps::delete_role(&state.db, tenant_id, role_id).await {
        Ok(DeleteRoleOutcome::Deleted) => Ok(HttpResponse::Ok().json(ApiResponse::from_status(
            StatusCode::OK,
            Some(serde_json::json!({ "success": true })),
            None,
        ))),
        Ok(DeleteRoleOutcome::NotFound) => {
            Ok(HttpResponse::NotFound().json(ApiResponse::from_status(
                StatusCode::NOT_FOUND,
                None::<()>,
                Some(vec!["Role was not found".to_string()]),
            )))
        }
        Ok(DeleteRoleOutcome::SystemRole) => {
            Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
                StatusCode::FORBIDDEN,
                None::<()>,
                Some(vec!["Protected roles cannot be deleted".to_string()]),
            )))
        }
        Ok(DeleteRoleOutcome::Assigned) => {
            Ok(HttpResponse::Conflict().json(ApiResponse::from_status(
                StatusCode::CONFLICT,
                None::<()>,
                Some(vec![
                    "Remove this role from every user before deleting it".to_string(),
                ]),
            )))
        }
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

fn canonical_values(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn normalize_description(description: Option<String>) -> Option<String> {
    description.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn normalize_optional_description(description: Option<Option<String>>) -> Option<Option<String>> {
    description.map(normalize_description)
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
