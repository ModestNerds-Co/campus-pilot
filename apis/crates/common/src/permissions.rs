//! Enforces catalogued permission routes through the operation evaluator.

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
    AccessContext, ApiResponse, RouteAuthority, RuntimeAccessChecks, module_key_for_namespace,
    routed_operation_for_route,
};

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
        let routed_operation = matched_pattern
            .as_deref()
            .and_then(|pattern| routed_operation_for_route(req.method(), pattern));

        Box::pin(async move {
            // AccessContext is set by AuthMiddleware (in the app crate) before
            // routing descends into a module's own scopes.
            let access = req.extensions().get::<AccessContext>().cloned();

            match access {
                Some(access) => {
                    match routed_operation {
                        Some(route) if route.authority() == RouteAuthority::Permission => {
                            let operation = route.operation();
                            let decision = access
                                .evaluate_operation(operation, RuntimeAccessChecks::default());
                            if decision.allowed {
                                return service
                                    .call(req)
                                    .await
                                    .map(|response| response.map_into_left_body());
                            }

                            log::warn!(
                                "Operation access denied: operation={}, reason={}",
                                operation.key(),
                                decision.reason.as_str(),
                            );
                            let response = ApiResponse::<()>::from_status(
                                actix_web::http::StatusCode::FORBIDDEN,
                                None,
                                Some(vec![
                                    "This operation is not available for your account or campus"
                                        .to_string(),
                                ]),
                            );
                            return Ok(req
                                .into_response(HttpResponse::Forbidden().json(response))
                                .map_into_right_body());
                        }
                        Some(route) => {
                            log::error!(
                                "Operation gate mismatch: operation={}, expected=permission, actual=authenticated",
                                route.operation().key(),
                            );
                            let response = ApiResponse::<()>::from_status(
                                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                                None,
                                Some(vec!["Operation access could not be checked".to_string()]),
                            );
                            return Ok(req
                                .into_response(HttpResponse::InternalServerError().json(response))
                                .map_into_right_body());
                        }
                        None => {
                            log::error!(
                                "Missing operation descriptor: method={}, route_pattern={}",
                                req.method(),
                                matched_pattern.as_deref().unwrap_or("<unmatched>"),
                            );
                        }
                    }

                    if !access.has_module(&fallback_module_key) {
                        let response = ApiResponse::<()>::from_status(
                            actix_web::http::StatusCode::FORBIDDEN,
                            None,
                            Some(vec![format!(
                                "Module is not enabled: {fallback_module_key}"
                            )]),
                        );
                        return Ok(req
                            .into_response(HttpResponse::Forbidden().json(response))
                            .map_into_right_body());
                    }

                    if access.has_permission(&fallback_permission) {
                        service.call(req).await.map(|res| res.map_into_left_body())
                    } else {
                        let response = ApiResponse::<()>::from_status(
                            actix_web::http::StatusCode::FORBIDDEN,
                            None,
                            Some(vec![format!(
                                "Insufficient permissions. Required: {fallback_permission}"
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
    use std::{collections::BTreeSet, sync::Once};

    use actix_web::{
        App, HttpMessage, HttpResponse,
        dev::Service as _,
        http::{Method, StatusCode},
        test as actix_test, web,
    };

    use crate::{
        AccessContext, EntitlementSnapshot, LeaseLifecycle, ModuleEntitlementState,
        RequirePermission,
    };

    use super::action_for;

    struct TestLogger;

    impl log::Log for TestLogger {
        fn enabled(&self, _metadata: &log::Metadata<'_>) -> bool {
            true
        }

        fn log(&self, _record: &log::Record<'_>) {}

        fn flush(&self) {}
    }

    static LOGGER: TestLogger = TestLogger;
    static INIT_LOGGER: Once = Once::new();

    fn enable_test_logging() {
        INIT_LOGGER.call_once(|| {
            let _ = log::set_logger(&LOGGER);
            log::set_max_level(log::LevelFilter::Trace);
        });
    }

    fn access(permissions: &[&str], enabled_modules: &[&str]) -> AccessContext {
        access_with_lifecycle(permissions, enabled_modules, LeaseLifecycle::Legacy)
    }

    fn access_with_lifecycle(
        permissions: &[&str],
        enabled_modules: &[&str],
        lifecycle: LeaseLifecycle,
    ) -> AccessContext {
        enable_test_logging();
        let entitlement_modules: Vec<_> = enabled_modules
            .iter()
            .copied()
            .chain(std::iter::once("administration"))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|module_key| (module_key.to_string(), ModuleEntitlementState::Enabled))
            .collect();
        AccessContext {
            role_keys: vec!["test-role".to_string()],
            permissions: permissions.iter().map(|value| value.to_string()).collect(),
            enabled_modules: enabled_modules
                .iter()
                .map(|value| value.to_string())
                .collect(),
            entitlements: EntitlementSnapshot::new(lifecycle, entitlement_modules, vec![])
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

        let exact_access = access(&["fleet:view"], &["fleet", "vehicle-logs", "hr_payroll"]);
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
    async fn evaluator_denial_is_authoritative_for_cataloged_permission_routes() {
        let mut access = access(&["fleet:view"], &["fleet"]);
        access.entitlements = EntitlementSnapshot::new(
            LeaseLifecycle::Legacy,
            vec![
                (
                    "administration".to_string(),
                    ModuleEntitlementState::Enabled,
                ),
                ("fleet".to_string(), ModuleEntitlementState::LocallyDisabled),
            ],
            vec![],
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
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body: crate::ApiResponse<serde_json::Value> =
            actix_test::read_body_json(response).await;
        assert_eq!(
            body.issues,
            Some(vec![
                "This operation is not available for your account or campus".to_string()
            ])
        );
    }

    #[actix_web::test]
    async fn lease_lifecycle_matrix_is_enforced_at_the_route_boundary() {
        let restricted = access_with_lifecycle(
            &["fleet:view", "fleet:create"],
            &["fleet", "hr_payroll"],
            LeaseLifecycle::Restricted,
        );
        let app = actix_test::init_service(
            App::new()
                .wrap_fn(move |request, service| {
                    request.extensions_mut().insert(restricted.clone());
                    service.call(request)
                })
                .service(
                    web::scope("/api/1.0/fleet/vehicles")
                        .wrap(RequirePermission::new("fleet"))
                        .route("", web::get().to(ok))
                        .route("", web::post().to(ok)),
                ),
        )
        .await;

        let read = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/api/1.0/fleet/vehicles")
                .to_request(),
        )
        .await;
        assert_eq!(read.status(), StatusCode::OK);
        let write = actix_test::call_service(
            &app,
            actix_test::TestRequest::post()
                .uri("/api/1.0/fleet/vehicles")
                .to_request(),
        )
        .await;
        assert_eq!(write.status(), StatusCode::FORBIDDEN);

        let revoked = access_with_lifecycle(
            &["fleet:view", "roles:view"],
            &["fleet", "administration"],
            LeaseLifecycle::Revoked,
        );
        let app = actix_test::init_service(
            App::new()
                .wrap_fn(move |request, service| {
                    request.extensions_mut().insert(revoked.clone());
                    service.call(request)
                })
                .service(
                    web::scope("/api/1.0")
                        .service(
                            web::scope("/fleet/vehicles")
                                .wrap(RequirePermission::new("fleet"))
                                .route("", web::get().to(ok)),
                        )
                        .service(
                            web::scope("/roles")
                                .wrap(RequirePermission::new("roles"))
                                .route("", web::get().to(ok)),
                        ),
                ),
        )
        .await;
        let licensed_read = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/api/1.0/fleet/vehicles")
                .to_request(),
        )
        .await;
        assert_eq!(licensed_read.status(), StatusCode::FORBIDDEN);
        let core_read = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/api/1.0/roles")
                .to_request(),
        )
        .await;
        assert_eq!(core_read.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn permission_middleware_rejects_an_authenticated_only_catalog_entry() {
        let access = access(&["administration:view"], &["administration"]);
        let app = actix_test::init_service(
            App::new()
                .wrap_fn(move |request, service| {
                    request.extensions_mut().insert(access.clone());
                    service.call(request)
                })
                .service(
                    web::scope("/api/1.0/access")
                        .wrap(RequirePermission::new("administration"))
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
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[actix_web::test]
    async fn permission_gate_preserves_module_and_authentication_denials() {
        let mut disabled_access = access(&["fleet:view"], &[]);
        disabled_access.entitlements = EntitlementSnapshot::new(
            LeaseLifecycle::Active,
            vec![("fleet".to_string(), ModuleEntitlementState::LocallyDisabled)],
            vec![],
        )
        .unwrap_or_else(|_| unreachable!());
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
    async fn unclassified_permission_route_preserves_fallback_denials() {
        let disabled = access(&["fleet:view"], &[]);
        let app = actix_test::init_service(
            App::new()
                .wrap_fn(move |request, service| {
                    request.extensions_mut().insert(disabled.clone());
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
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body: crate::ApiResponse<serde_json::Value> =
            actix_test::read_body_json(response).await;
        assert_eq!(
            body.issues,
            Some(vec!["Module is not enabled: fleet".to_string()])
        );

        let unauthorized = access(&[], &["fleet"]);
        let app = actix_test::init_service(
            App::new()
                .wrap_fn(move |request, service| {
                    request.extensions_mut().insert(unauthorized.clone());
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
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body: crate::ApiResponse<serde_json::Value> =
            actix_test::read_body_json(response).await;
        assert_eq!(
            body.issues,
            Some(vec![
                "Insufficient permissions. Required: fleet:view".to_string()
            ])
        );
    }
}
