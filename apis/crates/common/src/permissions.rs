//! Gates module routes and compares legacy checks with the operation evaluator.
//!
//! Shadow decisions are observable but do not change enforcement until drift is resolved.

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

use crate::{
    AccessContext, ApiResponse, OperationEffect, ProductOperation, RuntimeAccessChecks,
    module_key_for_namespace,
};

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

fn effect_for(method: &Method) -> OperationEffect {
    match *method {
        Method::GET => OperationEffect::Read,
        Method::DELETE => OperationEffect::Destructive,
        Method::POST | Method::PUT | Method::PATCH => OperationEffect::Write,
        _ => OperationEffect::Read,
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
        let action = action_for(req.method());
        let permission = format!("{}:{}", self.module, action);
        let module_key = module_key_for_namespace(&self.module).to_string();
        let operation = ProductOperation::route(
            format!("{}.{}", module_key, action),
            module_key.clone(),
            permission.clone(),
            effect_for(req.method()),
            !matches!(module_key.as_str(), "administration" | "home"),
        );

        Box::pin(async move {
            // AccessContext is set by AuthMiddleware (in the app crate) before
            // routing descends into a module's own scopes.
            let access = req.extensions().get::<AccessContext>().cloned();

            match access {
                Some(access) => {
                    let legacy_has_module = access.has_module(&module_key);
                    let legacy_has_permission = access.has_permission(&permission);
                    let shadow =
                        access.evaluate_operation(&operation, RuntimeAccessChecks::default());
                    let legacy_allowed = legacy_has_module && legacy_has_permission;
                    if shadow.allowed != legacy_allowed {
                        log::warn!(
                            "Operation entitlement shadow drift: operation={}, legacy_allowed={}, evaluator_allowed={}, evaluator_reason={}",
                            operation.key(),
                            legacy_allowed,
                            shadow.allowed,
                            shadow.reason.as_str(),
                        );
                    }

                    if !legacy_has_module {
                        let response = ApiResponse::<()>::from_status(
                            actix_web::http::StatusCode::FORBIDDEN,
                            None,
                            Some(vec![format!("Module is not enabled: {}", module_key)]),
                        );
                        return Ok(req
                            .into_response(HttpResponse::Forbidden().json(response))
                            .map_into_right_body());
                    }

                    if legacy_has_permission {
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
