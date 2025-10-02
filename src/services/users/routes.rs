//
//  campus-pilot-apis
//  routes.rs
//
//  Created by Ngonidzashe Mangudya on 2025/10/02.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use actix_web::http::StatusCode;
use actix_web::{HttpMessage, HttpRequest, HttpResponse, delete, get, post, put, web};
use uuid::Uuid;
use validator::Validate;

use crate::{
    middleware::{AuthMiddleware, RequirePermission},
    models::api_response::{ApiResponse, PaginationMeta},
    services::auth::models::User,
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
    query: web::Query<ListUsersQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 100);

    let (users, total) = match UserOps::list_users(
        &state.db,
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
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = path.into_inner();

    let user = match UserOps::get_user_by_id(&state.db, user_id).await {
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
    body: web::Json<CreateUserRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    // Validate request
    if let Err(e) = body.validate() {
        let errors = flatten_validation_errors(&e);
        return Ok(HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(errors),
        )));
    }

    // Check if email already exists
    match UserOps::email_exists(&state.db, &body.email, None).await {
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
        &body.email,
        &body.full_name,
        &password_hash,
        body.phone.as_deref(),
        body.roles.clone(),
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
    path: web::Path<Uuid>,
    body: web::Json<UpdateUserRequest>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
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

    // Check if user exists
    if UserOps::get_user_by_id(&state.db, user_id)
        .await
        .map_err(|e| {
            log::error!("Failed to check user existence: {:?}", e);
            actix_web::error::ErrorInternalServerError("Failed to check user")
        })?
        .is_none()
    {
        return Ok(HttpResponse::NotFound().json(ApiResponse::from_status(
            StatusCode::NOT_FOUND,
            None::<()>,
            Some(vec!["User not found".to_string()]),
        )));
    }

    // Check if email is being updated and already exists
    if let Some(ref email) = body.email {
        match UserOps::email_exists(&state.db, email, Some(user_id)).await {
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

    // Prevent user from modifying their own account
    if let Some(current_user) = req.extensions().get::<User>() {
        if current_user.id == user_id {
            return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
                StatusCode::FORBIDDEN,
                None::<()>,
                Some(vec!["Cannot modify your own account".to_string()]),
            )));
        }
    }

    // Update user
    let user = match UserOps::update_user(
        &state.db,
        user_id,
        body.email.as_deref(),
        body.full_name.as_deref(),
        body.phone.as_deref(),
        body.roles.clone(),
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
    path: web::Path<Uuid>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = path.into_inner();

    // Check if user exists
    let target_user = match UserOps::get_user_by_id(&state.db, user_id).await {
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
    if let Some(current_user) = req.extensions().get::<User>() {
        if current_user.id == user_id {
            return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
                StatusCode::FORBIDDEN,
                None::<()>,
                Some(vec!["Cannot delete your own account".to_string()]),
            )));
        }
    }

    // Prevent deleting Super Admin
    if target_user.roles.contains(&"Super Admin".to_string()) {
        return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
            StatusCode::FORBIDDEN,
            None::<()>,
            Some(vec!["Cannot delete Super Admin account".to_string()]),
        )));
    }

    // Delete user
    if let Err(e) = UserOps::delete_user(&state.db, user_id).await {
        log::error!("Failed to delete user: {:?}", e);
        return Ok(
            HttpResponse::InternalServerError().json(ApiResponse::from_status(
                StatusCode::INTERNAL_SERVER_ERROR,
                None::<()>,
                Some(vec!["Failed to delete user".to_string()]),
            )),
        );
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
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = path.into_inner();

    let user = match UserOps::activate_user(&state.db, user_id).await {
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
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = path.into_inner();

    let user = match UserOps::deactivate_user(&state.db, user_id).await {
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

    // TODO: Revoke all user sessions

    let response: UserResponse = user.into();

    Ok(HttpResponse::Ok().json(ApiResponse::from_status(
        StatusCode::OK,
        Some(response),
        None,
    )))
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/users")
            .wrap(AuthMiddleware)
            .service(
                web::scope("")
                    .wrap(RequirePermission::new("users:view"))
                    .service(list_users)
                    .service(get_user),
            )
            .service(
                web::scope("")
                    .wrap(RequirePermission::new("users:create"))
                    .service(create_user),
            )
            .service(
                web::scope("")
                    .wrap(RequirePermission::new("users:edit"))
                    .service(update_user)
                    .service(activate_user)
                    .service(deactivate_user),
            )
            .service(
                web::scope("")
                    .wrap(RequirePermission::new("users:delete"))
                    .service(delete_user),
            ),
    );
}
