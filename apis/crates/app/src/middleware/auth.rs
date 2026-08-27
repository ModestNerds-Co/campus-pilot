//! Authenticates API requests and attaches tenant-scoped access evidence.
//!
//! Downstream handlers receive only active users with freshly loaded authority.

use actix_web::{
    Error, HttpMessage, HttpResponse,
    body::{EitherBody, MessageBody},
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
};
use futures_util::future::LocalBoxFuture;
use std::{
    future::{Ready, ready},
    rc::Rc,
};

use cp_audit::AuditActor;
use cp_common::{AccessContext, Roles, TenantId};

use crate::{
    models::api_response::ApiResponse,
    services::{access::ops::AccessOps, auth::models::User},
    state::AppState,
    utils::verify_access_token,
};

pub struct AuthMiddleware;

impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddlewareService {
            service: Rc::new(service),
        }))
    }
}

pub struct AuthMiddlewareService<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for AuthMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = Rc::clone(&self.service);

        Box::pin(async move {
            // Extract Authorization header
            let auth_header = req.headers().get("Authorization");

            let token = match auth_header {
                Some(header) => {
                    let header_str = match header.to_str() {
                        Ok(s) => s,
                        Err(_) => {
                            let response = ApiResponse::<()>::from_status(
                                actix_web::http::StatusCode::UNAUTHORIZED,
                                None,
                                Some(vec!["Invalid authorization header".to_string()]),
                            );
                            return Ok(req
                                .into_response(HttpResponse::Unauthorized().json(response))
                                .map_into_right_body());
                        }
                    };

                    // Extract token from "Bearer <token>"
                    if !header_str.starts_with("Bearer ") {
                        let response = ApiResponse::<()>::from_status(
                            actix_web::http::StatusCode::UNAUTHORIZED,
                            None,
                            Some(vec![
                                "Invalid authorization format. Expected 'Bearer <token>'"
                                    .to_string(),
                            ]),
                        );
                        return Ok(req
                            .into_response(HttpResponse::Unauthorized().json(response))
                            .map_into_right_body());
                    }

                    &header_str[7..]
                }
                None => {
                    let response = ApiResponse::<()>::from_status(
                        actix_web::http::StatusCode::UNAUTHORIZED,
                        None,
                        Some(vec!["Missing authorization token".to_string()]),
                    );
                    return Ok(req
                        .into_response(HttpResponse::Unauthorized().json(response))
                        .map_into_right_body());
                }
            };

            // Get app state
            let app_state = match req.app_data::<actix_web::web::Data<AppState>>() {
                Some(state) => state,
                None => {
                    let response = ApiResponse::<()>::from_status(
                        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                        None,
                        Some(vec!["Application state not available".to_string()]),
                    );
                    return Ok(req
                        .into_response(HttpResponse::InternalServerError().json(response))
                        .map_into_right_body());
                }
            };

            // Verify token
            let claims = match verify_access_token(token, &app_state.config.jwt.secret) {
                Ok(claims) => claims,
                Err(_) => {
                    let response = ApiResponse::<()>::from_status(
                        actix_web::http::StatusCode::UNAUTHORIZED,
                        None,
                        Some(vec!["Invalid or expired token".to_string()]),
                    );
                    return Ok(req
                        .into_response(HttpResponse::Unauthorized().json(response))
                        .map_into_right_body());
                }
            };

            // Load user from database
            let user = match sqlx::query_as!(
                User,
                r#"
                SELECT id, tenant_id, email, full_name, phone, password_hash, roles, is_active,
                       last_login_at, last_login_ip, failed_login_attempts,
                       locked_until, created_at, updated_at, deleted_at
                FROM users
                WHERE id = $1 AND deleted_at IS NULL
                "#,
                claims.sub
            )
            .fetch_optional(&app_state.db)
            .await
            {
                Ok(Some(user)) => user,
                Ok(None) => {
                    let response = ApiResponse::<()>::from_status(
                        actix_web::http::StatusCode::UNAUTHORIZED,
                        None,
                        Some(vec!["User not found".to_string()]),
                    );
                    return Ok(req
                        .into_response(HttpResponse::Unauthorized().json(response))
                        .map_into_right_body());
                }
                Err(_) => {
                    let response = ApiResponse::<()>::from_status(
                        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                        None,
                        Some(vec!["Failed to load user".to_string()]),
                    );
                    return Ok(req
                        .into_response(HttpResponse::InternalServerError().json(response))
                        .map_into_right_body());
                }
            };

            // Check if user is active
            if !user.is_active {
                let response = ApiResponse::<()>::from_status(
                    actix_web::http::StatusCode::FORBIDDEN,
                    None,
                    Some(vec!["User account is inactive".to_string()]),
                );
                return Ok(req
                    .into_response(HttpResponse::Forbidden().json(response))
                    .map_into_right_body());
            }

            let effective_access =
                match AccessOps::effective_access(&app_state.db, user.tenant_id, &user.roles).await
                {
                    Ok(access) => access,
                    Err(error) => {
                        log::error!("Failed to load authenticated access context: {:?}", error);
                        let response = ApiResponse::<()>::from_status(
                            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                            None,
                            Some(vec!["Account access could not be loaded".to_string()]),
                        );
                        return Ok(req
                            .into_response(HttpResponse::InternalServerError().json(response))
                            .map_into_right_body());
                    }
                };

            // Attach user (and their tenant + roles) to request extensions
            let tenant_id = TenantId(user.tenant_id);
            let roles = Roles(user.roles.clone());
            let access_context = AccessContext {
                role_keys: user.roles.clone(),
                permissions: effective_access.permissions,
                enabled_modules: effective_access.enabled_modules,
                entitlements: effective_access.entitlements,
            };
            req.extensions_mut().insert(user);
            req.extensions_mut().insert(AuditActor::person(claims.sub));
            req.extensions_mut().insert(tenant_id);
            req.extensions_mut().insert(roles);
            req.extensions_mut().insert(access_context);

            // Continue to next middleware/handler
            service.call(req).await.map(|res| res.map_into_left_body())
        })
    }
}
