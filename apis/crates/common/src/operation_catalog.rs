//! Declares the versioned operation catalog for implemented API routes.
//!
//! Route matching is exact against Actix's resolved route pattern. The catalog
//! is code-owned so licensing, permissions, Agent capabilities, and audit can
//! share stable operation keys without trusting client-provided identifiers.

use std::sync::OnceLock;

use actix_web::http::Method;

use crate::{OperationEffect, ProductOperation};

/// Bump this when operation requirements change in a non-additive way.
pub const OPERATION_CATALOG_VERSION: u32 = 2;

/// Product-catalog identifier carried by signed entitlement leases.
///
/// The control plane and campus runtime must agree on this exact value before
/// lease claims can be accepted. Keep it aligned with
/// [`OPERATION_CATALOG_VERSION`].
pub const PRODUCT_CATALOG_VERSION: &str = "campus-pilot/2";

/// Product-catalog versions this campus binary can safely interpret.
///
/// A catalog upgrade may temporarily list both versions during a coordinated
/// rollout; the control plane still issues only [`PRODUCT_CATALOG_VERSION`].
pub const SUPPORTED_PRODUCT_CATALOG_VERSIONS: &[&str] = &[PRODUCT_CATALOG_VERSION];

/// Declares the authoritative access boundary for one routed operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteAuthority {
    /// A current campus identity is sufficient. This is reserved for shared
    /// launcher discovery needed before a user enters a module.
    Authenticated,
    /// The exact operation evaluator is authoritative.
    Permission,
}

/// Associates one resolved API route with its product operation.
#[derive(Debug)]
pub struct RoutedOperation {
    method: Method,
    route_pattern: &'static str,
    operation: ProductOperation,
    authority: RouteAuthority,
}

impl RoutedOperation {
    #[must_use]
    pub fn method(&self) -> &Method {
        &self.method
    }

    #[must_use]
    pub const fn route_pattern(&self) -> &'static str {
        self.route_pattern
    }

    #[must_use]
    pub const fn operation(&self) -> &ProductOperation {
        &self.operation
    }

    #[must_use]
    pub const fn authority(&self) -> RouteAuthority {
        self.authority
    }
}

/// Returns all product operations currently backed by working authenticated routes.
#[must_use]
pub fn operation_catalog() -> &'static [RoutedOperation] {
    static CATALOG: OnceLock<Vec<RoutedOperation>> = OnceLock::new();
    CATALOG.get_or_init(build_catalog)
}

/// Resolves an exact descriptor after Actix has matched the request route.
#[must_use]
pub fn operation_for_route(
    method: &Method,
    route_pattern: &str,
) -> Option<&'static ProductOperation> {
    operation_catalog()
        .iter()
        .find(|entry| entry.method() == method && entry.route_pattern() == route_pattern)
        .map(RoutedOperation::operation)
}

/// Resolves the route entry, including its authoritative access boundary.
#[must_use]
pub fn routed_operation_for_route(
    method: &Method,
    route_pattern: &str,
) -> Option<&'static RoutedOperation> {
    operation_catalog()
        .iter()
        .find(|entry| entry.method() == method && entry.route_pattern() == route_pattern)
}

fn build_catalog() -> Vec<RoutedOperation> {
    vec![
        // Authenticated launcher discovery used by every signed-in role.
        authenticated_route(
            Method::GET,
            "/api/1.0/access/catalog",
            "administration.catalog.read",
            "administration",
            "administration:view",
            OperationEffect::Read,
        ),
        authenticated_route(
            Method::GET,
            "/api/1.0/access/modules",
            "administration.modules.list",
            "administration",
            "administration:view",
            OperationEffect::Read,
        ),
        route(
            Method::GET,
            "/api/1.0/access/licensing",
            "administration.licensing.read",
            "administration",
            "licensing:view",
            OperationEffect::Read,
            false,
        ),
        route(
            Method::GET,
            "/api/1.0/kernel/school-profile",
            "administration.school_settings.read",
            "administration",
            "school_settings:view",
            OperationEffect::Read,
            false,
        ),
        route(
            Method::PUT,
            "/api/1.0/kernel/school-profile",
            "administration.school_settings.update",
            "administration",
            "school_settings:edit",
            OperationEffect::Write,
            false,
        ),
        route(
            Method::POST,
            "/api/1.0/kernel/school-profile/logo",
            "administration.school_settings.update_logo",
            "administration",
            "school_settings:edit",
            OperationEffect::Write,
            false,
        ),
        // Administration: roles.
        route(
            Method::GET,
            "/api/1.0/roles",
            "administration.roles.list",
            "administration",
            "roles:view",
            OperationEffect::Read,
            false,
        ),
        route(
            Method::GET,
            "/api/1.0/roles/{id}",
            "administration.roles.read",
            "administration",
            "roles:view",
            OperationEffect::Read,
            false,
        ),
        route(
            Method::POST,
            "/api/1.0/roles",
            "administration.roles.create",
            "administration",
            "roles:create",
            OperationEffect::Write,
            false,
        ),
        route(
            Method::PUT,
            "/api/1.0/roles/{id}",
            "administration.roles.update",
            "administration",
            "roles:edit",
            OperationEffect::Write,
            false,
        ),
        route(
            Method::DELETE,
            "/api/1.0/roles/{id}",
            "administration.roles.delete",
            "administration",
            "roles:delete",
            OperationEffect::Destructive,
            false,
        ),
        // Administration: users.
        route(
            Method::GET,
            "/api/1.0/users",
            "administration.users.list",
            "administration",
            "users:view",
            OperationEffect::Read,
            false,
        ),
        route(
            Method::GET,
            "/api/1.0/users/{id}",
            "administration.users.read",
            "administration",
            "users:view",
            OperationEffect::Read,
            false,
        ),
        route(
            Method::POST,
            "/api/1.0/users",
            "administration.users.create",
            "administration",
            "users:create",
            OperationEffect::Write,
            false,
        ),
        route(
            Method::PUT,
            "/api/1.0/users/{id}",
            "administration.users.update",
            "administration",
            "users:edit",
            OperationEffect::Write,
            false,
        ),
        route(
            Method::PUT,
            "/api/1.0/users/{id}/activate",
            "administration.users.activate",
            "administration",
            "users:edit",
            OperationEffect::Write,
            false,
        ),
        route(
            Method::PUT,
            "/api/1.0/users/{id}/deactivate",
            "administration.users.deactivate",
            "administration",
            "users:edit",
            OperationEffect::Write,
            false,
        ),
        route(
            Method::DELETE,
            "/api/1.0/users/{id}",
            "administration.users.delete",
            "administration",
            "users:delete",
            OperationEffect::Destructive,
            false,
        ),
        // Administration: license recovery and local module control.
        route(
            Method::PUT,
            "/api/1.0/access/licenses/activate",
            "administration.licensing.activate_legacy_key",
            "administration",
            "licensing:edit",
            OperationEffect::LicenseRepair,
            false,
        ),
        route(
            Method::PUT,
            "/api/1.0/access/licensing/connect",
            "administration.licensing.connect",
            "administration",
            "licensing:edit",
            OperationEffect::LicenseRepair,
            false,
        ),
        route(
            Method::POST,
            "/api/1.0/access/licensing/refresh",
            "administration.licensing.refresh",
            "administration",
            "licensing:edit",
            OperationEffect::LicenseRepair,
            false,
        ),
        route(
            Method::POST,
            "/api/1.0/access/licensing/import",
            "administration.licensing.import_offline_lease",
            "administration",
            "licensing:edit",
            OperationEffect::LicenseRepair,
            false,
        ),
        route(
            Method::DELETE,
            "/api/1.0/access/modules/{module_key}",
            "administration.licensing.disable_module",
            "administration",
            "licensing:delete",
            OperationEffect::Destructive,
            false,
        ),
        // Fleet: vehicles.
        route(
            Method::GET,
            "/api/1.0/fleet/vehicles",
            "fleet.vehicles.list",
            "fleet",
            "fleet:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/fleet/vehicles/{id}",
            "fleet.vehicles.read",
            "fleet",
            "fleet:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/fleet/vehicles",
            "fleet.vehicles.create",
            "fleet",
            "fleet:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/fleet/vehicles/{id}",
            "fleet.vehicles.update",
            "fleet",
            "fleet:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/fleet/vehicles/{id}",
            "fleet.vehicles.delete",
            "fleet",
            "fleet:delete",
            OperationEffect::Destructive,
            true,
        ),
        // Fleet: drivers.
        route(
            Method::GET,
            "/api/1.0/fleet/drivers",
            "fleet.drivers.list",
            "fleet",
            "fleet:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/fleet/drivers/{id}",
            "fleet.drivers.read",
            "fleet",
            "fleet:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/fleet/drivers",
            "fleet.drivers.create",
            "fleet",
            "fleet:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/fleet/drivers/{id}",
            "fleet.drivers.update",
            "fleet",
            "fleet:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/fleet/drivers/{id}",
            "fleet.drivers.delete",
            "fleet",
            "fleet:delete",
            OperationEffect::Destructive,
            true,
        ),
        // Fleet: vehicle daily logs.
        route(
            Method::GET,
            "/api/1.0/vehicle-logs",
            "fleet.vehicle_logs.list",
            "fleet",
            "fleet:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/vehicle-logs/{id}",
            "fleet.vehicle_logs.read",
            "fleet",
            "fleet:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/vehicle-logs",
            "fleet.vehicle_logs.create",
            "fleet",
            "fleet:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/vehicle-logs/{id}",
            "fleet.vehicle_logs.update",
            "fleet",
            "fleet:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/vehicle-logs/{id}",
            "fleet.vehicle_logs.delete",
            "fleet",
            "fleet:delete",
            OperationEffect::Destructive,
            true,
        ),
        // Timetabling.
        route(
            Method::GET,
            "/api/1.0/timetabling/configuration",
            "timetabling.configuration.read",
            "timetabling",
            "timetabling:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/timetabling/configuration",
            "timetabling.configuration.update",
            "timetabling",
            "timetabling:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/timetabling/generate",
            "timetabling.runs.generate",
            "timetabling",
            "timetabling:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/timetabling/runs/latest",
            "timetabling.runs.read_latest",
            "timetabling",
            "timetabling:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/timetabling/runs/{id}/publish",
            "timetabling.runs.publish",
            "timetabling",
            "timetabling:edit",
            OperationEffect::External,
            true,
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn route(
    method: Method,
    route_pattern: &'static str,
    key: &'static str,
    module_key: &'static str,
    permission: &'static str,
    effect: OperationEffect,
    license_required: bool,
) -> RoutedOperation {
    RoutedOperation {
        method,
        route_pattern,
        operation: ProductOperation::route(key, module_key, permission, effect, license_required),
        authority: RouteAuthority::Permission,
    }
}

fn authenticated_route(
    method: Method,
    route_pattern: &'static str,
    key: &'static str,
    module_key: &'static str,
    permission: &'static str,
    effect: OperationEffect,
) -> RoutedOperation {
    RoutedOperation {
        method,
        route_pattern,
        operation: ProductOperation::route(key, module_key, permission, effect, false),
        authority: RouteAuthority::Authenticated,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use actix_web::{App, HttpRequest, HttpResponse, http::Method, test as actix_test, web};

    use crate::{
        EntitlementSnapshot, LeaseLifecycle, ModuleEntitlementState, ProductOperation,
        RuntimeAccessChecks, evaluate_operation,
    };

    use super::{
        OPERATION_CATALOG_VERSION, PRODUCT_CATALOG_VERSION, RouteAuthority,
        SUPPORTED_PRODUCT_CATALOG_VERSIONS, operation_catalog, operation_for_route,
        routed_operation_for_route,
    };

    fn operation(key: &str) -> &'static ProductOperation {
        operation_catalog()
            .iter()
            .find(|entry| entry.operation().key() == key)
            .map(super::RoutedOperation::operation)
            .unwrap_or_else(|| unreachable!())
    }

    fn active_snapshot() -> EntitlementSnapshot {
        EntitlementSnapshot::new(
            LeaseLifecycle::Active,
            vec![
                (
                    "administration".to_string(),
                    ModuleEntitlementState::Enabled,
                ),
                ("fleet".to_string(), ModuleEntitlementState::Enabled),
                ("timetabling".to_string(), ModuleEntitlementState::Enabled),
            ],
            vec![],
        )
        .unwrap_or_else(|_| unreachable!())
    }

    fn allowed(key: &str, permissions: &[&str]) -> bool {
        evaluate_operation(
            operation(key),
            &active_snapshot(),
            &permissions
                .iter()
                .map(|permission| permission.to_string())
                .collect::<Vec<_>>(),
            RuntimeAccessChecks::default(),
        )
        .allowed
    }

    #[test]
    fn catalog_has_unique_stable_keys_and_route_identities() {
        assert_eq!(OPERATION_CATALOG_VERSION, 2);
        assert_eq!(
            PRODUCT_CATALOG_VERSION,
            format!("campus-pilot/{OPERATION_CATALOG_VERSION}")
        );
        assert!(SUPPORTED_PRODUCT_CATALOG_VERSIONS.contains(&PRODUCT_CATALOG_VERSION));
        assert_eq!(operation_catalog().len(), 43);

        let mut keys = BTreeSet::new();
        let mut routes = BTreeSet::new();
        for entry in operation_catalog() {
            let key = entry.operation().key();
            assert!(
                key.split('.').all(|part| {
                    !part.is_empty()
                        && part
                            .chars()
                            .all(|character| character.is_ascii_lowercase() || character == '_')
                }),
                "invalid operation key: {key}"
            );
            assert!(keys.insert(key), "duplicate operation key: {key}");

            let route = (entry.method().as_str(), entry.route_pattern());
            assert!(
                routes.insert(route),
                "duplicate routed operation: {route:?}"
            );
        }
    }

    #[test]
    fn only_launcher_discovery_uses_authenticated_authority() {
        let authenticated = operation_catalog()
            .iter()
            .filter(|entry| entry.authority() == RouteAuthority::Authenticated)
            .map(|entry| entry.operation().key())
            .collect::<Vec<_>>();
        assert_eq!(
            authenticated,
            vec!["administration.catalog.read", "administration.modules.list"]
        );

        for (method, pattern) in [
            (Method::GET, "/api/1.0/access/licensing"),
            (Method::GET, "/api/1.0/kernel/school-profile"),
            (Method::PUT, "/api/1.0/kernel/school-profile"),
            (Method::POST, "/api/1.0/kernel/school-profile/logo"),
        ] {
            assert_eq!(
                routed_operation_for_route(&method, pattern)
                    .unwrap_or_else(|| unreachable!())
                    .authority(),
                RouteAuthority::Permission
            );
        }
    }

    #[test]
    fn exact_route_resolution_distinguishes_collection_and_record_reads() {
        let list =
            operation_for_route(&Method::GET, "/api/1.0/users").unwrap_or_else(|| unreachable!());
        let read = operation_for_route(&Method::GET, "/api/1.0/users/{id}")
            .unwrap_or_else(|| unreachable!());

        assert_eq!(list.key(), "administration.users.list");
        assert_eq!(read.key(), "administration.users.read");
        assert!(operation_for_route(&Method::PATCH, "/api/1.0/users/{id}").is_none());
    }

    #[test]
    fn seeded_and_custom_role_permissions_intersect_exact_operations() {
        let school_administrator = [
            "administration:view",
            "users:view",
            "users:create",
            "users:edit",
            "roles:view",
            "roles:create",
            "roles:edit",
            "roles:assign",
            "licensing:view",
            "licensing:edit",
            "licensing:delete",
            "school_settings:view",
            "school_settings:edit",
        ];
        assert!(allowed("administration.users.list", &school_administrator));
        assert!(allowed(
            "administration.licensing.refresh",
            &school_administrator
        ));
        assert!(!allowed(
            "administration.users.delete",
            &school_administrator
        ));
        assert!(!allowed("fleet.vehicles.list", &school_administrator));

        let teacher = [
            "academics:view",
            "academics:edit",
            "sis:view",
            "timetabling:view",
            "messaging:view",
            "messaging:create",
            "library:view",
        ];
        assert!(allowed("timetabling.configuration.read", &teacher));
        assert!(!allowed("timetabling.runs.generate", &teacher));
        assert!(!allowed("administration.users.list", &teacher));

        let student = [
            "academics:view",
            "timetabling:view",
            "fees:view",
            "library:view",
            "messaging:view",
        ];
        assert!(allowed("timetabling.runs.read_latest", &student));
        assert!(!allowed("timetabling.configuration.update", &student));

        let fleet_viewer = ["fleet:view"];
        assert!(allowed("fleet.vehicles.list", &fleet_viewer));
        assert!(allowed("fleet.vehicle_logs.read", &fleet_viewer));
        assert!(!allowed("fleet.vehicles.create", &fleet_viewer));
        assert!(!allowed("fleet.vehicle_logs.delete", &fleet_viewer));
    }

    #[test]
    fn campus_owner_wildcard_still_requires_every_operation_module() {
        let snapshot = active_snapshot();
        for entry in operation_catalog() {
            let decision = evaluate_operation(
                entry.operation(),
                &snapshot,
                &["*".to_string()],
                RuntimeAccessChecks::default(),
            );
            assert!(
                decision.allowed,
                "owner unexpectedly denied operation {}: {}",
                entry.operation().key(),
                decision.reason.as_str()
            );
        }

        let missing_fleet = EntitlementSnapshot::new(
            LeaseLifecycle::Active,
            vec![(
                "administration".to_string(),
                ModuleEntitlementState::Enabled,
            )],
            vec![],
        )
        .unwrap_or_else(|_| unreachable!());
        let decision = evaluate_operation(
            operation("fleet.vehicles.list"),
            &missing_fleet,
            &["*".to_string()],
            RuntimeAccessChecks::default(),
        );
        assert!(!decision.allowed);
    }

    #[actix_web::test]
    async fn catalog_patterns_equal_actix_resolved_patterns() {
        async fn resolved_pattern(request: HttpRequest) -> HttpResponse {
            HttpResponse::Ok().body(
                request
                    .match_pattern()
                    .unwrap_or_else(|| "<unmatched>".to_string()),
            )
        }

        let app = actix_test::init_service(App::new().configure(|config| {
            for entry in operation_catalog() {
                config.route(
                    entry.route_pattern(),
                    web::method(entry.method().clone()).to(resolved_pattern),
                );
            }
        }))
        .await;

        for entry in operation_catalog() {
            let concrete_path = entry
                .route_pattern()
                .replace("{id}", "00000000-0000-0000-0000-000000000001")
                .replace("{module_key}", "fleet");
            let request = actix_test::TestRequest::default()
                .method(entry.method().clone())
                .uri(&concrete_path)
                .to_request();
            let response = actix_test::call_service(&app, request).await;
            assert!(
                response.status().is_success(),
                "unmatched route: {concrete_path}"
            );
            let body = actix_test::read_body(response).await;
            assert_eq!(body.as_ref(), entry.route_pattern().as_bytes());
        }
    }
}
