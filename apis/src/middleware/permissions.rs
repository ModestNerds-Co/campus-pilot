//
//  campus-pilot-apis
//  permissions.rs
//
//  Created by Ngonidzashe Mangudya on 2025/10/02.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

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

use crate::{models::api_response::ApiResponse, services::auth::models::User};

pub struct RequirePermission {
    pub permission: String,
}

impl RequirePermission {
    pub fn new(permission: impl Into<String>) -> Self {
        Self {
            permission: permission.into(),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RequirePermission
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = RequirePermissionService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RequirePermissionService {
            service: Rc::new(service),
            permission: self.permission.clone(),
        }))
    }
}

pub struct RequirePermissionService<S> {
    service: Rc<S>,
    permission: String,
}

impl<S, B> Service<ServiceRequest> for RequirePermissionService<S>
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
        let permission = self.permission.clone();

        Box::pin(async move {
            // Get user from request extensions (set by AuthMiddleware)
            let user = req.extensions().get::<User>().cloned();

            match user {
                Some(user) => {
                    // Check if user has required permission
                    if has_permission(&user, &permission) {
                        // User has permission, continue
                        service.call(req).await.map(|res| res.map_into_left_body())
                    } else {
                        // User lacks permission
                        let response = ApiResponse::<()>::from_status(
                            actix_web::http::StatusCode::FORBIDDEN,
                            None,
                            Some(vec![format!(
                                "Insufficient permissions. Required: {}",
                                permission
                            )]),
                        );
                        Ok(req
                            .into_response(HttpResponse::Forbidden().json(response))
                            .map_into_right_body())
                    }
                }
                None => {
                    // User not authenticated (AuthMiddleware should have caught this)
                    let response = ApiResponse::<()>::from_status(
                        actix_web::http::StatusCode::UNAUTHORIZED,
                        None,
                        Some(vec!["Authentication required".to_string()]),
                    );
                    Ok(req
                        .into_response(HttpResponse::Unauthorized().json(response))
                        .map_into_right_body())
                }
            }
        })
    }
}

/// Check if user has a specific permission
/// For now, this checks if the user has "Super Admin" role or a role matching the permission prefix
/// TODO: Implement proper role-permission system with database
fn has_permission(user: &User, permission: &str) -> bool {
    // Super Admin has all permissions
    if user.roles.contains(&"Super Admin".to_string()) {
        return true;
    }

    // Parse permission format: "module:action" (e.g., "users:view", "users:create")
    let parts: Vec<&str> = permission.split(':').collect();
    if parts.len() != 2 {
        return false;
    }

    let module = parts[0];
    let _action = parts[1];

    // For now, check if user has a role that matches the module
    // Example: "users:view" requires "User Manager" or "users" role
    // This is a simplified implementation - proper RBAC will use database
    user.roles
        .iter()
        .any(|role| role.to_lowercase().contains(module))
}
