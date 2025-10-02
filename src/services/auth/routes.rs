//
//  campus-pilot-apis
//  routes.rs
//
//  Created by Ngonidzashe Mangudya on 2025/10/02.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use actix_web::{HttpRequest, HttpResponse, get, post, web};
use validator::Validate;

use crate::{
    models::api_response::ApiResponse,
    state::AppState,
    utils::{
        flatten_validation_errors, generate_access_token, generate_refresh_token, verify_password,
        verify_token,
    },
};
use actix_web::http::StatusCode;

use super::{
    dtos::{LoginRequest, LoginResponse, LogoutRequest, RefreshRequest, RefreshResponse},
    models::UserInfo,
    ops::AuthOps,
};

#[post("login")]
async fn login(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<LoginRequest>,
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

    // Find user by email
    let user = match AuthOps::find_user_by_email(&state.db, &body.email).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Ok(HttpResponse::Unauthorized().json(ApiResponse::from_status(
                StatusCode::UNAUTHORIZED,
                None::<()>,
                None,
            )));
        }
        Err(e) => {
            log::error!("Failed to find user: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    None,
                )),
            );
        }
    };

    // Check if account is locked
    if AuthOps::is_account_locked(&user) {
        return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
            StatusCode::FORBIDDEN,
            None::<()>,
            None,
        )));
    }

    // Check if user is active
    if !user.is_active {
        return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
            StatusCode::FORBIDDEN,
            None::<()>,
            None,
        )));
    }

    // Verify password
    let password_valid = match verify_password(&body.password, &user.password_hash) {
        Ok(valid) => valid,
        Err(e) => {
            log::error!("Failed to verify password: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    None,
                )),
            );
        }
    };

    if !password_valid {
        // Increment failed login attempts
        if let Err(e) = AuthOps::increment_failed_login(&state.db, user.id).await {
            log::error!("Failed to increment failed login: {:?}", e);
        }

        return Ok(HttpResponse::Unauthorized().json(ApiResponse::from_status(
            StatusCode::UNAUTHORIZED,
            None::<()>,
            None,
        )));
    }

    // Generate tokens
    let access_token = match generate_access_token(
        user.id,
        &user.email,
        user.roles.clone(),
        &state.config.jwt.secret,
    ) {
        Ok(token) => token,
        Err(e) => {
            log::error!("Failed to generate access token: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    None,
                )),
            );
        }
    };

    let refresh_token = match generate_refresh_token(
        user.id,
        &user.email,
        user.roles.clone(),
        &state.config.jwt.secret,
    ) {
        Ok(token) => token,
        Err(e) => {
            log::error!("Failed to generate refresh token: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    None,
                )),
            );
        }
    };

    // Get IP address
    let ip_address = req
        .connection_info()
        .realip_remote_addr()
        .map(|s| s.to_string());

    // Get user agent
    let user_agent = req
        .headers()
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    // Store refresh token
    if let Err(e) = AuthOps::store_refresh_token(
        &state.db,
        user.id,
        &refresh_token,
        ip_address.as_deref(),
        user_agent.as_deref(),
    )
    .await
    {
        log::error!("Failed to store refresh token: {:?}", e);
        return Ok(
            HttpResponse::InternalServerError().json(ApiResponse::from_status(
                StatusCode::INTERNAL_SERVER_ERROR,
                None::<()>,
                None,
            )),
        );
    }

    // Update login info
    if let Err(e) = AuthOps::update_login_info(&state.db, user.id, ip_address.as_deref()).await {
        log::error!("Failed to update login info: {:?}", e);
    }

    let response = LoginResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: 900, // 15 minutes in seconds
        user: user.into(),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::from_status(
        StatusCode::OK,
        Some(response),
        None,
    )))
}

#[post("refresh")]
async fn refresh(
    state: web::Data<AppState>,
    body: web::Json<RefreshRequest>,
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

    // Verify refresh token JWT
    let claims = match verify_token(&body.refresh_token, &state.config.jwt.secret) {
        Ok(claims) => claims,
        Err(e) => {
            log::warn!("Invalid refresh token: {:?}", e);
            return Ok(HttpResponse::Unauthorized().json(ApiResponse::from_status(
                StatusCode::UNAUTHORIZED,
                None::<()>,
                None,
            )));
        }
    };

    // Find refresh token in database
    let db_token = match AuthOps::find_refresh_token(&state.db, &body.refresh_token).await {
        Ok(Some(token)) => token,
        Ok(None) => {
            return Ok(HttpResponse::Unauthorized().json(ApiResponse::from_status(
                StatusCode::UNAUTHORIZED,
                None::<()>,
                None,
            )));
        }
        Err(e) => {
            log::error!("Failed to find refresh token: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    None,
                )),
            );
        }
    };

    // Validate refresh token (not revoked, not expired)
    if let Err(e) = AuthOps::validate_refresh_token(&db_token) {
        return Ok(HttpResponse::Unauthorized().json(ApiResponse::from_status(
            StatusCode::UNAUTHORIZED,
            None::<()>,
            Some(vec![e.to_string()]),
        )));
    }

    // Get user
    let user = match AuthOps::find_user_by_id(&state.db, claims.sub).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Ok(HttpResponse::Unauthorized().json(ApiResponse::from_status(
                StatusCode::UNAUTHORIZED,
                None::<()>,
                None,
            )));
        }
        Err(e) => {
            log::error!("Failed to find user: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    None,
                )),
            );
        }
    };

    // Check if user is active
    if !user.is_active {
        return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
            StatusCode::FORBIDDEN,
            None::<()>,
            None,
        )));
    }

    // Generate new tokens
    let new_access_token = match generate_access_token(
        user.id,
        &user.email,
        user.roles.clone(),
        &state.config.jwt.secret,
    ) {
        Ok(token) => token,
        Err(e) => {
            log::error!("Failed to generate access token: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    None,
                )),
            );
        }
    };

    let new_refresh_token = match generate_refresh_token(
        user.id,
        &user.email,
        user.roles.clone(),
        &state.config.jwt.secret,
    ) {
        Ok(token) => token,
        Err(e) => {
            log::error!("Failed to generate refresh token: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    None,
                )),
            );
        }
    };

    // Revoke old refresh token and store new one
    if let Err(e) = AuthOps::revoke_refresh_token(&state.db, &body.refresh_token).await {
        log::error!("Failed to revoke old refresh token: {:?}", e);
    }

    if let Err(e) = AuthOps::store_refresh_token(
        &state.db,
        user.id,
        &new_refresh_token,
        db_token.ip_address.as_deref(),
        db_token.user_agent.as_deref(),
    )
    .await
    {
        log::error!("Failed to store new refresh token: {:?}", e);
        return Ok(
            HttpResponse::InternalServerError().json(ApiResponse::from_status(
                StatusCode::INTERNAL_SERVER_ERROR,
                None::<()>,
                None,
            )),
        );
    }

    let response = RefreshResponse {
        access_token: new_access_token,
        refresh_token: new_refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: 900, // 15 minutes in seconds
    };

    Ok(HttpResponse::Ok().json(ApiResponse::from_status(
        StatusCode::OK,
        Some(response),
        None,
    )))
}

#[post("logout")]
async fn logout(
    state: web::Data<AppState>,
    body: web::Json<LogoutRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(refresh_token) = &body.refresh_token {
        if let Err(e) = AuthOps::revoke_refresh_token(&state.db, refresh_token).await {
            log::error!("Failed to revoke refresh token: {:?}", e);
        }
    }

    Ok(HttpResponse::Ok().json(ApiResponse::from_status(
        StatusCode::OK,
        Some(serde_json::json!({ "success": true })),
        None,
    )))
}

#[get("me")]
async fn me(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    // Extract token from Authorization header
    let auth_header = match req.headers().get("Authorization") {
        Some(header) => header,
        None => {
            return Ok(HttpResponse::Unauthorized().json(ApiResponse::from_status(
                StatusCode::UNAUTHORIZED,
                None::<()>,
                None,
            )));
        }
    };

    let token = match auth_header.to_str() {
        Ok(s) if s.starts_with("Bearer ") => &s[7..],
        _ => {
            return Ok(HttpResponse::Unauthorized().json(ApiResponse::from_status(
                StatusCode::UNAUTHORIZED,
                None::<()>,
                None,
            )));
        }
    };

    // Verify token
    let claims = match verify_token(token, &state.config.jwt.secret) {
        Ok(claims) => claims,
        Err(e) => {
            log::warn!("Invalid token: {:?}", e);
            return Ok(HttpResponse::Unauthorized().json(ApiResponse::from_status(
                StatusCode::UNAUTHORIZED,
                None::<()>,
                None,
            )));
        }
    };

    // Get user
    let user = match AuthOps::find_user_by_id(&state.db, claims.sub).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Ok(HttpResponse::Unauthorized().json(ApiResponse::from_status(
                StatusCode::UNAUTHORIZED,
                None::<()>,
                None,
            )));
        }
        Err(e) => {
            log::error!("Failed to find user: {:?}", e);
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::from_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    None::<()>,
                    None,
                )),
            );
        }
    };

    // Check if user is active
    if !user.is_active {
        return Ok(HttpResponse::Forbidden().json(ApiResponse::from_status(
            StatusCode::FORBIDDEN,
            None::<()>,
            None,
        )));
    }

    let user_info: UserInfo = user.into();

    Ok(HttpResponse::Ok().json(ApiResponse::from_status(
        StatusCode::OK,
        Some(user_info),
        None,
    )))
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .service(login)
            .service(refresh)
            .service(logout)
            .service(me),
    );
}
