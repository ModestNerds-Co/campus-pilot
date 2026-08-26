//
//  cp-common
//  permissions.rs
//
//  Created by Ngonidzashe Mangudya on 2025/10/02.
//  Moved into cp-common on 2026/08/21 so every module crate can gate its
//  own routes without depending on the `app` crate. Redesigned the same day
//  to wrap a whole resource scope ONCE per module rather than nesting a
//  separate empty-prefixed `web::scope("")` per permission tier — actix-web
//  only honors the first of several identically-patterned nested scopes
//  under one parent, which silently 404'd every create/update/delete route
//  in `users`, `roles`, and the new ERP modules alike.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use actix_web::{
    Error, HttpMessage, HttpResponse,
    body::{EitherBody, MessageBody},
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::Method,
};
use futures_util::future::LocalBoxFuture;
use std::{
    future::{Ready, ready},
    rc::Rc,
};

use crate::{ApiResponse, roles::Roles};

/// Gates every request in a scope with a `"<module>:<action>"` permission,
/// deriving `<action>` from the HTTP method (GET -> view, POST -> create,
/// PUT/PATCH -> edit, DELETE -> delete) so one `.wrap()` on the resource
/// scope covers the whole CRUD set.
pub struct RequirePermission {
    module: String,
}

impl RequirePermission {
    pub fn new(module: impl Into<String>) -> Self {
        Self {
            module: module.into(),
        }
    }
}

fn action_for(method: &Method) -> &'static str {
    match *method {
        Method::GET => "view",
        Method::POST => "create",
        Method::PUT | Method::PATCH => "edit",
        Method::DELETE => "delete",
        _ => "view",
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
            module: self.module.clone(),
        }))
    }
}

pub struct RequirePermissionService<S> {
    service: Rc<S>,
    module: String,
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
        let permission = format!("{}:{}", self.module, action_for(req.method()));

        Box::pin(async move {
            // Roles are set by AuthMiddleware (in the app crate) before routing
            // descends into a module's own scopes.
            let roles = req.extensions().get::<Roles>().cloned();

            match roles {
                Some(roles) => {
                    if has_permission(&roles, &permission) {
                        service.call(req).await.map(|res| res.map_into_left_body())
                    } else {
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

/// Check if the caller's roles satisfy a "module:action" permission.
/// Super Admin has all permissions.
/// TODO: back this with a real role -> permission lookup once the roles
/// table's `permissions` column is consulted here instead of role-name matching.
fn has_permission(roles: &Roles, permission: &str) -> bool {
    if roles.contains("Super Admin") {
        return true;
    }

    let parts: Vec<&str> = permission.split(':').collect();
    if parts.len() != 2 {
        return false;
    }

    let module = parts[0];
    roles
        .0
        .iter()
        .any(|role| role.to_lowercase().contains(module))
}
