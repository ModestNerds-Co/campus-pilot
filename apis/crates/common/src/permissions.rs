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
    AccessContext, ApiResponse, LegacyRouteGate, OperationEffect, ProductOperation,
    RuntimeAccessChecks, module_key_for_namespace, operation_for_route, routed_operation_for_route,
};

/// Observes exact evaluator decisions for routes whose compatibility gate is
/// authentication only. It never changes the response during shadow rollout.
#[derive(Debug, Clone, Copy, Default)]
pub struct ObserveOperationAccess;

impl<S, B> Transform<S, ServiceRequest> for ObserveOperationAccess
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = ObserveOperationAccessService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(ObserveOperationAccessService {
            service: Rc::new(service),
        }))
    }
}

pub struct ObserveOperationAccessService<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for ObserveOperationAccessService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = Rc::clone(&self.service);
        let matched_pattern = req.match_pattern();
        let routed_operation = matched_pattern
            .as_deref()
            .and_then(|pattern| routed_operation_for_route(req.method(), pattern));

        Box::pin(async move {
            match routed_operation {
                Some(route) if route.legacy_gate() == LegacyRouteGate::Authenticated => {
                    if let Some(access) = req.extensions().get::<AccessContext>() {
                        let shadow = access
                            .evaluate_operation(route.operation(), RuntimeAccessChecks::default());
                        if !shadow.allowed {
                            log::warn!(
                                "Operation entitlement shadow drift: operation={}, legacy_allowed=true, evaluator_allowed=false, evaluator_reason={}",
                                route.operation().key(),
                                shadow.reason.as_str(),
                            );
                        }
                    }
                }
                Some(_) => {}
                None => {
                    log::error!(
                        "Missing operation descriptor: method={}, route_pattern={}",
                        req.method(),
                        matched_pattern.as_deref().unwrap_or("<unmatched>"),
                    );
                }
            }

            service.call(req).await
        })
    }
}

/// Gates a route with its exact catalog module and permission.
///
/// An uncatalogued compatibility route falls back to `"<module>:<action>"`,
/// deriving the action from its HTTP method until the route is classified.
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
        let fallback_permission = format!("{}:{}", self.module, action);
        let fallback_module_key = module_key_for_namespace(&self.module).to_string();
        let matched_pattern = req.match_pattern();
        let catalog_operation = matched_pattern
            .as_deref()
            .and_then(|pattern| operation_for_route(req.method(), pattern));
        let fallback_operation = catalog_operation.is_none().then(|| {
            ProductOperation::route(
                format!("{}.{}", fallback_module_key, action),
                fallback_module_key.clone(),
                fallback_permission.clone(),
                effect_for(req.method()),
                !matches!(fallback_module_key.as_str(), "administration" | "home"),
            )
        });

        Box::pin(async move {
            // AccessContext is set by AuthMiddleware (in the app crate) before
            // routing descends into a module's own scopes.
            let access = req.extensions().get::<AccessContext>().cloned();

            match access {
                Some(access) => {
                    let operation = match catalog_operation {
                        Some(operation) => operation,
                        None => {
                            log::error!(
                                "Missing operation descriptor: method={}, route_pattern={}",
                                req.method(),
                                matched_pattern.as_deref().unwrap_or("<unmatched>"),
                            );
                            fallback_operation
                                .as_ref()
                                .unwrap_or_else(|| unreachable!())
                        }
                    };
                    let module_key = operation.module_key();
                    let permission = operation.permission();
                    let legacy_has_module = access.has_module(module_key);
                    let legacy_has_permission = access.has_permission(permission);
                    let shadow =
                        access.evaluate_operation(operation, RuntimeAccessChecks::default());
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
                            Some(vec![format!("Module is not enabled: {module_key}")]),
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
                                "Insufficient permissions. Required: {permission}"
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

#[cfg(test)]
mod tests {
    use actix_web::{
        App, HttpMessage, HttpResponse,
        dev::Service as _,
        http::{Method, StatusCode},
        test as actix_test, web,
    };

    use crate::{
        AccessContext, EntitlementSnapshot, LeaseLifecycle, ModuleEntitlementState,
        ObserveOperationAccess, RequirePermission,
    };

    use super::{OperationEffect, action_for, effect_for};

    fn access(permissions: &[&str], enabled_modules: &[&str]) -> AccessContext {
        AccessContext {
            role_keys: vec!["test-role".to_string()],
            permissions: permissions.iter().map(|value| value.to_string()).collect(),
            enabled_modules: enabled_modules
                .iter()
                .map(|value| value.to_string())
                .collect(),
            entitlements: EntitlementSnapshot::new(
                LeaseLifecycle::Legacy,
                [
                    (
                        "administration".to_string(),
                        ModuleEntitlementState::Enabled,
                    ),
                    ("fleet".to_string(), ModuleEntitlementState::Enabled),
                ],
                [],
            )
            .unwrap_or_else(|_| unreachable!()),
        }
    }

    async fn ok() -> HttpResponse {
        HttpResponse::Ok().finish()
    }

    #[test]
    fn compatibility_method_mapping_remains_stable() {
        assert_eq!(action_for(&Method::GET), "view");
        assert_eq!(action_for(&Method::POST), "create");
        assert_eq!(action_for(&Method::PUT), "edit");
        assert_eq!(action_for(&Method::PATCH), "edit");
        assert_eq!(action_for(&Method::DELETE), "delete");
        assert_eq!(action_for(&Method::OPTIONS), "view");

        assert_eq!(effect_for(&Method::GET), OperationEffect::Read);
        assert_eq!(effect_for(&Method::DELETE), OperationEffect::Destructive);
        assert_eq!(effect_for(&Method::POST), OperationEffect::Write);
        assert_eq!(effect_for(&Method::PUT), OperationEffect::Write);
        assert_eq!(effect_for(&Method::PATCH), OperationEffect::Write);
        assert_eq!(effect_for(&Method::OPTIONS), OperationEffect::Read);
    }

    #[actix_web::test]
    async fn exact_descriptor_replaces_coarse_namespace_and_method_permissions() {
        let legacy_access = access(&["vehicle-logs:view"], &["fleet", "vehicle-logs"]);
        let app = actix_test::init_service(
            App::new()
                .wrap_fn(move |request, service| {
                    request.extensions_mut().insert(legacy_access.clone());
                    service.call(request)
                })
                .service(
                    web::scope("/api/1.0/vehicle-logs")
                        .wrap(RequirePermission::new("vehicle-logs"))
                        .route("", web::get().to(ok)),
                ),
        )
        .await;

        let response = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/api/1.0/vehicle-logs")
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let exact_access = access(&["fleet:view"], &["fleet", "vehicle-logs"]);
        let app = actix_test::init_service(
            App::new()
                .wrap_fn(move |request, service| {
                    request.extensions_mut().insert(exact_access.clone());
                    service.call(request)
                })
                .service(
                    web::scope("/api/1.0/vehicle-logs")
                        .wrap(RequirePermission::new("vehicle-logs"))
                        .route("", web::get().to(ok)),
                ),
        )
        .await;

        let response = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/api/1.0/vehicle-logs")
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);

        let licensing_access = access(&["licensing:edit"], &["administration"]);
        let app = actix_test::init_service(
            App::new()
                .wrap_fn(move |request, service| {
                    request.extensions_mut().insert(licensing_access.clone());
                    service.call(request)
                })
                .service(
                    web::scope("/api/1.0/access")
                        .wrap(RequirePermission::new("licensing"))
                        .route("/licensing/refresh", web::post().to(ok)),
                ),
        )
        .await;

        let response = actix_test::call_service(
            &app,
            actix_test::TestRequest::post()
                .uri("/api/1.0/access/licensing/refresh")
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn evaluator_result_remains_observational_during_shadow_rollout() {
        let mut access = access(&["fleet:view"], &["fleet"]);
        access.entitlements = EntitlementSnapshot::new(
            LeaseLifecycle::Legacy,
            [
                (
                    "administration".to_string(),
                    ModuleEntitlementState::Enabled,
                ),
                ("fleet".to_string(), ModuleEntitlementState::LocallyDisabled),
            ],
            [],
        )
        .unwrap_or_else(|_| unreachable!());

        let app = actix_test::init_service(
            App::new()
                .wrap_fn(move |request, service| {
                    request.extensions_mut().insert(access.clone());
                    service.call(request)
                })
                .service(
                    web::scope("/api/1.0/fleet/vehicles")
                        .wrap(RequirePermission::new("fleet"))
                        .route("", web::get().to(ok)),
                ),
        )
        .await;

        let response = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/api/1.0/fleet/vehicles")
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn permission_gate_preserves_module_and_authentication_denials() {
        let disabled_access = access(&["fleet:view"], &[]);
        let app = actix_test::init_service(
            App::new()
                .wrap_fn(move |request, service| {
                    request.extensions_mut().insert(disabled_access.clone());
                    service.call(request)
                })
                .service(
                    web::scope("/api/1.0/fleet/vehicles")
                        .wrap(RequirePermission::new("fleet"))
                        .route("", web::get().to(ok)),
                ),
        )
        .await;
        let response = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/api/1.0/fleet/vehicles")
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let app = actix_test::init_service(
            App::new().service(
                web::scope("/api/1.0/fleet/vehicles")
                    .wrap(RequirePermission::new("fleet"))
                    .route("", web::get().to(ok)),
            ),
        )
        .await;
        let response = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/api/1.0/fleet/vehicles")
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn unclassified_permission_route_uses_compatibility_descriptor() {
        let access = access(&["fleet:view"], &["fleet"]);
        let app = actix_test::init_service(
            App::new()
                .wrap_fn(move |request, service| {
                    request.extensions_mut().insert(access.clone());
                    service.call(request)
                })
                .service(
                    web::scope("/api/1.0/fleet")
                        .wrap(RequirePermission::new("fleet"))
                        .route("/unclassified", web::get().to(ok)),
                ),
        )
        .await;

        let response = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/api/1.0/fleet/unclassified")
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn authenticated_operation_observer_never_changes_legacy_response() {
        let access = access(&[], &["administration"]);
        let app = actix_test::init_service(
            App::new()
                .wrap_fn(move |request, service| {
                    request.extensions_mut().insert(access.clone());
                    service.call(request)
                })
                .service(
                    web::scope("/api/1.0/access")
                        .wrap(ObserveOperationAccess)
                        .route("/catalog", web::get().to(ok))
                        .route("/licensing/connect", web::put().to(ok))
                        .route("/unknown", web::get().to(ok)),
                ),
        )
        .await;

        for (method, path) in [
            (Method::GET, "/api/1.0/access/catalog"),
            (Method::PUT, "/api/1.0/access/licensing/connect"),
            (Method::GET, "/api/1.0/access/unknown"),
        ] {
            let response = actix_test::call_service(
                &app,
                actix_test::TestRequest::default()
                    .method(method)
                    .uri(path)
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
        }

        let app = actix_test::init_service(
            App::new().service(
                web::scope("/api/1.0/access")
                    .wrap(ObserveOperationAccess)
                    .route("/catalog", web::get().to(ok)),
            ),
        )
        .await;
        let response = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/api/1.0/access/catalog")
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }
}
