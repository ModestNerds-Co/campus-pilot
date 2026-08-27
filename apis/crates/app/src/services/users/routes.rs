//! Exposes tenant-scoped user administration with delegation-safe role changes.
//!
//! Operators cannot manage their own account, the Campus Owner account, or a
//! user whose existing role access exceeds their own effective permissions.

use std::collections::BTreeSet;

use actix_web::http::StatusCode;
use actix_web::{HttpMessage, HttpRequest, HttpResponse, delete, get, post, put, web};
use cp_common::{AccessContext, TenantId};
use uuid::Uuid;
use validator::Validate;

use crate::{
    middleware::{AuthMiddleware, RequirePermission},
    models::api_response::{ApiResponse, PaginationMeta},
    services::{
        auth::{AuthOps, models::User},
        roles::ops::RoleOps,
    },
    state::AppState,
    utils::{flatten_validation_errors, hash_password},
};

use super::{
    dtos::{
        CreateUserRequest, ListUsersQuery, PaginatedUsersResponse, UpdateUserRequest, UserResponse,
    },
    ops::UserOps,
};

#[get("")]
async fn list_users(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<ListUsersQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 100);

    let (users, total) = match UserOps::list_users(
        &state.db,
        tenant_id,
        page,
        per_page,
        query.search.as_deref(),
        query.role.as_deref(),
        query.status.as_deref(),
        query.sort.as_deref(),
    )
    .await
    {
        Ok(result) => result,
        Err(e) => {
            log::error!("Failed to list users: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    Some(vec!["Failed to fetch users".to_string()]),
                )),
            );
        }
    };

    let pagination = PaginationMeta::new(page as u32, per_page as u32, total);

    let response = PaginatedUsersResponse {
        users: users.into_iter().map(|u| u.into()).collect(),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::with_pagination(
        StatusCode::OK,
        Some(response),
        pagination,
        None,
    )))
}

#[get("/{id}")]
async fn get_user(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    let user_id = path.into_inner();

    let user = match UserOps::get_user_by_id(&state.db, tenant_id, user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Ok(HttpResponse::NotFound().json(ApiResponse::from_status(
                StatusCode::NOT_FOUND,
                None::<()>,
                Some(vec!["User not found".to_string()]),
            )));
        }
        Err(e) => {
            log::error!("Failed to get user: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    Some(vec!["Failed to fetch user".to_string()]),
                )),
            );
        }
    };

    let response: UserResponse = user.into();

    Ok(HttpResponse::Ok().json(ApiResponse::from_status(
        StatusCode::OK,
        Some(response),
        None,
    )))
}

#[post("")]
async fn create_user(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    body: web::Json<CreateUserRequest>,
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

    let email = body.email.trim();
    let full_name = body.full_name.trim();
    let phone = body
        .phone
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if full_name.is_empty() {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(vec!["Full name is required".to_string()]),
        )));
    }

    let role_keys = canonical_values(body.roles.clone());
    if role_keys.is_empty() {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(vec!["At least one role is required".to_string()]),
        )));
    }

    if !access.has_permission("roles:assign") {
        return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
            StatusCode::FORBIDDEN,
            None::<()>,
            Some(vec!["Role assignment permission is required".to_string()]),
        )));
    }
    match RoleOps::assignment_permissions(&state.db, tenant_id, &role_keys).await {
        Ok(Some(permissions)) if access.can_delegate_permissions(&permissions) => {}
        Ok(Some(_)) => {
            return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
                StatusCode::FORBIDDEN,
                None::<()>,
                Some(vec![
                    "Only the Campus Owner can assign a full-access role".to_string(),
                ]),
            )));
        }
        Ok(None) => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
                StatusCode::BAD_REQUEST,
                None::<()>,
                Some(vec![
                    "One or more selected roles are no longer available".to_string(),
                ]),
            )));
        }
        Err(error) => {
            log::error!("Failed to validate role assignments: {:?}", error);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    Some(vec!["Role assignments could not be validated".to_string()]),
                )),
            );
        }
    }

    // Check if email already exists
    match UserOps::email_exists(&state.db, tenant_id, email, None).await {
        Ok(true) => {
            return Ok(HttpResponse::Conflict().json(ApiResponse::from_status(
                StatusCode::CONFLICT,
                None::<()>,
                Some(vec!["Email already exists".to_string()]),
            )));
        }
        Ok(false) => {}
        Err(e) => {
            log::error!("Failed to check email existence: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    Some(vec!["Failed to validate email".to_string()]),
                )),
            );
        }
    }

    // Hash password
    let password_hash = match hash_password(&body.password) {
        Ok(hash) => hash,
        Err(e) => {
            log::error!("Failed to hash password: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    Some(vec!["Failed to process password".to_string()]),
                )),
            );
        }
    };

    // Create user
    let user = match UserOps::create_user(
        &state.db,
        tenant_id,
        email,
        full_name,
        &password_hash,
        phone,
        role_keys,
        body.is_active.unwrap_or(true),
    )
    .await
    {
        Ok(user) => user,
        Err(e) => {
            log::error!("Failed to create user: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    Some(vec!["Failed to create user".to_string()]),
                )),
            );
        }
    };

    let response: UserResponse = user.into();

    Ok(HttpResponse::Created().json(ApiResponse::from_status(
        StatusCode::CREATED,
        Some(response),
        None,
    )))
}

#[put("/{id}")]
async fn update_user(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateUserRequest>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    let user_id = path.into_inner();

    // Validate request
    if let Err(e) = body.validate() {
        let errors = flatten_validation_errors(&e);
        return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(errors),
        )));
    }

    let target_user = match UserOps::get_user_by_id(&state.db, tenant_id, user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Ok(HttpResponse::NotFound().json(ApiResponse::from_status(
                StatusCode::NOT_FOUND,
                None::<()>,
                Some(vec!["User not found".to_string()]),
            )));
        }
        Err(error) => {
            log::error!("Failed to load user before update: {:?}", error);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    Some(vec!["User access could not be checked".to_string()]),
                )),
            );
        }
    };

    if req
        .extensions()
        .get::<User>()
        .is_some_and(|current_user| current_user.id == user_id)
    {
        return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
            StatusCode::FORBIDDEN,
            None::<()>,
            Some(vec![
                "Use your account settings to change your own details".to_string(),
            ]),
        )));
    }
    if target_user.roles.iter().any(|role| role == "campus_owner") {
        return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
            StatusCode::FORBIDDEN,
            None::<()>,
            Some(vec![
                "The Campus Owner account is managed through account settings".to_string(),
            ]),
        )));
    }
    if !can_manage_user(&state, tenant_id, &access, &target_user).await? {
        return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
            StatusCode::FORBIDDEN,
            None::<()>,
            Some(vec![
                "You cannot change a user with access beyond your own".to_string(),
            ]),
        )));
    }

    let normalized_role_keys = body.roles.clone().map(canonical_values);
    if normalized_role_keys.as_ref().is_some_and(Vec::is_empty) {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(vec!["At least one role is required".to_string()]),
        )));
    }

    let email = body.email.as_deref().map(str::trim);
    let full_name = body.full_name.as_deref().map(str::trim);
    if full_name.is_some_and(str::is_empty) {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(vec!["Full name cannot be empty".to_string()]),
        )));
    }
    let phone = body.phone.as_ref().map(|phone| {
        phone
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    });

    if let Some(role_keys) = normalized_role_keys.as_ref() {
        if !access.has_permission("roles:assign") {
            return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
                StatusCode::FORBIDDEN,
                None::<()>,
                Some(vec!["Role assignment permission is required".to_string()]),
            )));
        }
        match RoleOps::assignment_permissions(&state.db, tenant_id, role_keys).await {
            Ok(Some(permissions)) if access.can_delegate_permissions(&permissions) => {}
            Ok(Some(_)) => {
                return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
                    StatusCode::FORBIDDEN,
                    None::<()>,
                    Some(vec![
                        "Only the Campus Owner can assign a full-access role".to_string(),
                    ]),
                )));
            }
            Ok(None) => {
                return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
                    StatusCode::BAD_REQUEST,
                    None::<()>,
                    Some(vec![
                        "One or more selected roles are no longer available".to_string(),
                    ]),
                )));
            }
            Err(error) => {
                log::error!("Failed to validate role assignments: {:?}", error);
                return Ok(
                    HttpResponse::InternalServerError().json(ApiResponse::from_status(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        None::<()>,
                        Some(vec!["Role assignments could not be validated".to_string()]),
                    )),
                );
            }
        }
    }

    // Check if email is being updated and already exists
    if let Some(email) = email {
        match UserOps::email_exists(&state.db, tenant_id, email, Some(user_id)).await {
            Ok(true) => {
                return Ok(HttpResponse::Conflict().json(ApiResponse::from_status(
                    StatusCode::CONFLICT,
                    None::<()>,
                    Some(vec!["Email already exists".to_string()]),
                )));
            }
            Ok(false) => {}
            Err(e) => {
                log::error!("Failed to check email existence: {:?}", e);
                return Ok(
                    HttpResponse::InternalServerError().json(ApiResponse::from_status(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        None::<()>,
                        Some(vec!["Failed to validate email".to_string()]),
                    )),
                );
            }
        }
    }

    // Update user
    let user = match UserOps::update_user(
        &state.db,
        tenant_id,
        user_id,
        email,
        full_name,
        phone.as_ref().map(|phone| phone.as_deref()),
        normalized_role_keys,
        body.is_active,
    )
    .await
    {
        Ok(user) => user,
        Err(e) => {
            log::error!("Failed to update user: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    Some(vec!["Failed to update user".to_string()]),
                )),
            );
        }
    };

    let response: UserResponse = user.into();

    Ok(HttpResponse::Ok().json(ApiResponse::from_status(
        StatusCode::OK,
        Some(response),
        None,
    )))
}

#[delete("/{id}")]
async fn delete_user(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    path: web::Path<Uuid>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    let user_id = path.into_inner();

    // Check if user exists
    let target_user = match UserOps::get_user_by_id(&state.db, tenant_id, user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Ok(HttpResponse::NotFound().json(ApiResponse::from_status(
                StatusCode::NOT_FOUND,
                None::<()>,
                Some(vec!["User not found".to_string()]),
            )));
        }
        Err(e) => {
            log::error!("Failed to get user: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    Some(vec!["Failed to fetch user".to_string()]),
                )),
            );
        }
    };

    // Prevent deleting own account
    if req
        .extensions()
        .get::<User>()
        .is_some_and(|current_user| current_user.id == user_id)
    {
        return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
            StatusCode::FORBIDDEN,
            None::<()>,
            Some(vec!["Cannot delete your own account".to_string()]),
        )));
    }

    // The campus must always retain its owner account.
    if target_user.roles.contains(&"campus_owner".to_string()) {
        return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
            StatusCode::FORBIDDEN,
            None::<()>,
            Some(vec![
                "The Campus Owner account cannot be deleted".to_string(),
            ]),
        )));
    }
    if !can_manage_user(&state, tenant_id, &access, &target_user).await? {
        return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
            StatusCode::FORBIDDEN,
            None::<()>,
            Some(vec![
                "You cannot delete a user with access beyond your own".to_string(),
            ]),
        )));
    }

    // Delete user
    if let Err(e) = UserOps::delete_user(&state.db, tenant_id, user_id).await {
        log::error!("Failed to delete user: {:?}", e);
        return Ok(
            HttpResponse::InternalServerError().json(ApiResponse::from_status(
                StatusCode::INTERNAL_SERVER_ERROR,
                None::<()>,
                Some(vec!["Failed to delete user".to_string()]),
            )),
        );
    }
    if let Err(error) = AuthOps::revoke_all_user_tokens(&state.db, user_id).await {
        log::error!("Failed to revoke deleted user's sessions: {:?}", error);
    }

    Ok(HttpResponse::Ok().json(ApiResponse::from_status(
        StatusCode::OK,
        Some(serde_json::json!({ "success": true })),
        None,
    )))
}

#[put("/{id}/activate")]
async fn activate_user(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    path: web::Path<Uuid>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    let user_id = path.into_inner();

    let target_user = match managed_target_user(&state, tenant_id, user_id, &access, &req).await? {
        ManagedTargetUser::Allowed(user) => user,
        ManagedTargetUser::NotFound => return Ok(manage_user_not_found()),
        ManagedTargetUser::Forbidden => return Ok(manage_user_forbidden()),
    };
    if target_user.roles.iter().any(|role| role == "campus_owner") {
        return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
            StatusCode::FORBIDDEN,
            None::<()>,
            Some(vec![
                "The Campus Owner account is always active".to_string(),
            ]),
        )));
    }

    let user = match UserOps::activate_user(&state.db, tenant_id, user_id).await {
        Ok(user) => user,
        Err(e) => {
            log::error!("Failed to activate user: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    Some(vec!["Failed to activate user".to_string()]),
                )),
            );
        }
    };

    let response: UserResponse = user.into();

    Ok(HttpResponse::Ok().json(ApiResponse::from_status(
        StatusCode::OK,
        Some(response),
        None,
    )))
}

#[put("/{id}/deactivate")]
async fn deactivate_user(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    path: web::Path<Uuid>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let tenant_id = tenant.into_inner().into_inner();
    let user_id = path.into_inner();

    let target_user = match managed_target_user(&state, tenant_id, user_id, &access, &req).await? {
        ManagedTargetUser::Allowed(user) => user,
        ManagedTargetUser::NotFound => return Ok(manage_user_not_found()),
        ManagedTargetUser::Forbidden => return Ok(manage_user_forbidden()),
    };
    if target_user.roles.iter().any(|role| role == "campus_owner") {
        return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
            StatusCode::FORBIDDEN,
            None::<()>,
            Some(vec![
                "The Campus Owner account cannot be deactivated".to_string(),
            ]),
        )));
    }

    let user = match UserOps::deactivate_user(&state.db, tenant_id, user_id).await {
        Ok(user) => user,
        Err(e) => {
            log::error!("Failed to deactivate user: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    Some(vec!["Failed to deactivate user".to_string()]),
                )),
            );
        }
    };

    if let Err(error) = AuthOps::revoke_all_user_tokens(&state.db, user_id).await {
        log::error!("Failed to revoke deactivated user's sessions: {:?}", error);
    }

    let response: UserResponse = user.into();

    Ok(HttpResponse::Ok().json(ApiResponse::from_status(
        StatusCode::OK,
        Some(response),
        None,
    )))
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

async fn can_manage_user(
    state: &web::Data<AppState>,
    tenant_id: Uuid,
    access: &AccessContext,
    target_user: &User,
) -> Result<bool, actix_web::Error> {
    RoleOps::assignment_permissions(&state.db, tenant_id, &target_user.roles)
        .await
        .map(|permissions| {
            permissions.is_some_and(|permissions| access.can_delegate_permissions(&permissions))
        })
        .map_err(|error| {
            log::error!("Failed to resolve target user access: {:?}", error);
            actix_web::error::ErrorInternalServerError("User access could not be checked")
        })
}

async fn managed_target_user(
    state: &web::Data<AppState>,
    tenant_id: Uuid,
    user_id: Uuid,
    access: &AccessContext,
    req: &HttpRequest,
) -> Result<ManagedTargetUser, actix_web::Error> {
    if req
        .extensions()
        .get::<User>()
        .is_some_and(|current_user| current_user.id == user_id)
    {
        return Ok(ManagedTargetUser::Forbidden);
    }
    let target_user = UserOps::get_user_by_id(&state.db, tenant_id, user_id)
        .await
        .map_err(|error| {
            log::error!("Failed to load managed user: {:?}", error);
            actix_web::error::ErrorInternalServerError("User access could not be checked")
        })?;
    let Some(target_user) = target_user else {
        return Ok(ManagedTargetUser::NotFound);
    };
    Ok(
        if can_manage_user(state, tenant_id, access, &target_user).await? {
            ManagedTargetUser::Allowed(Box::new(target_user))
        } else {
            ManagedTargetUser::Forbidden
        },
    )
}

enum ManagedTargetUser {
    Allowed(Box<User>),
    NotFound,
    Forbidden,
}

fn manage_user_forbidden() -> HttpResponse {
    HttpResponse::Forbidden().json(ApiResponse::from_status(
        StatusCode::FORBIDDEN,
        None::<()>,
        Some(vec!["You cannot change this account".to_string()]),
    ))
}

fn manage_user_not_found() -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::from_status(
        StatusCode::NOT_FOUND,
        None::<()>,
        Some(vec!["User not found".to_string()]),
    ))
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/users")
            // actix composes `.wrap()` calls outside-in in reverse registration
            // order, so AuthMiddleware (which populates Roles) must be
            // registered LAST to actually run FIRST, ahead of RequirePermission.
            .wrap(RequirePermission::new("users"))
            .wrap(AuthMiddleware)
            .service(list_users)
            .service(get_user)
            .service(create_user)
            .service(update_user)
            .service(activate_user)
            .service(deactivate_user)
            .service(delete_user),
    );
}
