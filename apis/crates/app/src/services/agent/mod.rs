//! Assembles production Agent capability adapters over existing domain services.

mod administration;
mod administration_access;
mod fleet;
mod hr;

use cp_agent::CapabilityRegistry;
use sqlx::PgPool;

use crate::config::LicenseConfig;

use administration::{
    AdministrationCatalogCapability, AdministrationLicensingCapability,
    AdministrationModulesCapability, AdministrationSchoolSettingsCapability,
};
use administration_access::{
    AdministrationRoleReadCapability, AdministrationRolesListCapability,
    AdministrationUserReadCapability, AdministrationUsersListCapability,
};
use fleet::{
    FleetDriverCandidatesListCapability, FleetDriverReadCapability, FleetDriversListCapability,
    FleetVehicleLogReadCapability, FleetVehicleLogsListCapability, FleetVehicleReadCapability,
    FleetVehiclesListCapability,
};
use hr::{
    HrDepartmentReadCapability, HrDepartmentsListCapability, HrEmployeeReadCapability,
    HrEmployeesListCapability, HrPositionReadCapability, HrPositionsListCapability,
};

#[must_use]
pub fn build_capability_registry(
    pool: PgPool,
    license_config: LicenseConfig,
) -> CapabilityRegistry {
    let mut registry = CapabilityRegistry::from_product_catalog()
        .unwrap_or_else(|error| panic!("invalid product operation catalogue: {error}"));
    registry
        .register(AdministrationCatalogCapability::new())
        .unwrap_or_else(|error| panic!("invalid Administration catalogue capability: {error}"));
    registry
        .register(AdministrationModulesCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Administration modules capability: {error}"));
    registry
        .register(AdministrationRolesListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Administration roles-list capability: {error}"));
    registry
        .register(AdministrationRoleReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Administration role-read capability: {error}"));
    registry
        .register(AdministrationUsersListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Administration users-list capability: {error}"));
    registry
        .register(AdministrationUserReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Administration user-read capability: {error}"));
    registry
        .register(AdministrationSchoolSettingsCapability::new(pool.clone()))
        .unwrap_or_else(|error| {
            panic!("invalid Administration school-settings capability: {error}")
        });
    registry
        .register(AdministrationLicensingCapability::new(
            pool.clone(),
            license_config,
        ))
        .unwrap_or_else(|error| panic!("invalid Administration licensing capability: {error}"));
    registry
        .register(HrDepartmentsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid HR departments-list capability: {error}"));
    registry
        .register(HrDepartmentReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid HR department-read capability: {error}"));
    registry
        .register(HrPositionsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid HR positions-list capability: {error}"));
    registry
        .register(HrPositionReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid HR position-read capability: {error}"));
    registry
        .register(HrEmployeesListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid HR employees-list capability: {error}"));
    registry
        .register(HrEmployeeReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid HR employee-read capability: {error}"));
    registry
        .register(FleetDriverCandidatesListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Fleet driver-candidates capability: {error}"));
    registry
        .register(FleetVehiclesListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Fleet vehicles-list capability: {error}"));
    registry
        .register(FleetVehicleReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Fleet vehicle-read capability: {error}"));
    registry
        .register(FleetDriversListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Fleet drivers-list capability: {error}"));
    registry
        .register(FleetDriverReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Fleet driver-read capability: {error}"));
    registry
        .register(FleetVehicleLogsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Fleet vehicle-logs-list capability: {error}"));
    registry
        .register(FleetVehicleLogReadCapability::new(pool))
        .unwrap_or_else(|error| panic!("invalid Fleet vehicle-log-read capability: {error}"));
    registry
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use cp_agent::{
        AuthenticatedAgentPrincipal, AuthorityLoadError, AuthorityLoader, AuthorizedRecordScope,
        BrokerAuditError, BrokerAuditRecord, BrokerAuditSink, BrokerErrorCode, CapabilityBroker,
        CapabilityCall, CurrentAuthority, RecordScopeAuthorizer, RecordScopeDenied,
    };
    use cp_audit::RequestContext;
    use cp_common::{
        AccessContext, EntitlementSnapshot, LeaseLifecycle, ModuleEntitlementState,
        ProductOperation,
    };
    use serde_json::json;
    use sqlx::postgres::PgPoolOptions;
    use std::collections::BTreeMap;
    use uuid::Uuid;

    use crate::config::LicenseConfig;

    use super::build_capability_registry;

    struct TestAuthorityLoader(CurrentAuthority);

    #[async_trait]
    impl AuthorityLoader for TestAuthorityLoader {
        async fn load(
            &self,
            _principal: AuthenticatedAgentPrincipal,
        ) -> Result<CurrentAuthority, AuthorityLoadError> {
            Ok(self.0.clone())
        }
    }

    struct TenantWideScope;

    #[async_trait]
    impl RecordScopeAuthorizer for TenantWideScope {
        async fn authorize(
            &self,
            _principal: AuthenticatedAgentPrincipal,
            _authority: &CurrentAuthority,
            _operation: &ProductOperation,
            _scope: &cp_agent::CapabilityScope,
        ) -> Result<AuthorizedRecordScope, RecordScopeDenied> {
            Ok(AuthorizedRecordScope::granted())
        }
    }

    struct TestAudit;

    #[async_trait]
    impl BrokerAuditSink for TestAudit {
        async fn record(&self, _record: BrokerAuditRecord) -> Result<(), BrokerAuditError> {
            Ok(())
        }
    }

    fn authority() -> CurrentAuthority {
        CurrentAuthority::from_reloaded_access(AccessContext {
            role_keys: vec!["campus_owner".to_string()],
            permissions: vec![
                "agent:run".to_string(),
                "administration:view".to_string(),
                "roles:view".to_string(),
                "users:view".to_string(),
                "school_settings:view".to_string(),
                "licensing:view".to_string(),
                "hr_payroll:view".to_string(),
                "fleet:view".to_string(),
            ],
            enabled_modules: vec![
                "agent".to_string(),
                "administration".to_string(),
                "hr_payroll".to_string(),
                "fleet".to_string(),
            ],
            entitlements: EntitlementSnapshot::new(
                LeaseLifecycle::Active,
                [
                    ("agent".to_string(), ModuleEntitlementState::Enabled),
                    (
                        "administration".to_string(),
                        ModuleEntitlementState::Enabled,
                    ),
                    ("hr_payroll".to_string(), ModuleEntitlementState::Enabled),
                    ("fleet".to_string(), ModuleEntitlementState::Enabled),
                ],
                Vec::<String>::new(),
            )
            .unwrap_or_else(|_| unreachable!()),
        })
    }

    fn license_config() -> LicenseConfig {
        LicenseConfig {
            trusted_public_keys: BTreeMap::new(),
            issuer: "campus-pilot-control-plane".to_string(),
            audience: "campus-pilot".to_string(),
            control_plane_url: Some("https://licensing.invalid".to_string()),
            credential_key_base64: Some("test-key".to_string()),
            installation_name: "Test installation".to_string(),
        }
    }

    #[tokio::test]
    async fn production_registry_executes_the_real_administration_catalogue_read() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
            .unwrap_or_else(|_| unreachable!());
        let registry = build_capability_registry(pool, license_config());
        let keys = registry
            .descriptors()
            .into_iter()
            .map(|descriptor| descriptor.key().as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            keys,
            vec![
                "administration.catalog.read",
                "administration.licensing.read",
                "administration.modules.list",
                "administration.roles.list",
                "administration.roles.read",
                "administration.school_settings.read",
                "administration.users.list",
                "administration.users.read",
                "fleet.driver_candidates.list",
                "fleet.drivers.list",
                "fleet.drivers.read",
                "fleet.vehicle_logs.list",
                "fleet.vehicle_logs.read",
                "fleet.vehicles.list",
                "fleet.vehicles.read",
                "hr_payroll.departments.list",
                "hr_payroll.departments.read",
                "hr_payroll.employees.list",
                "hr_payroll.employees.read",
                "hr_payroll.positions.list",
                "hr_payroll.positions.read"
            ]
        );
        let broker = CapabilityBroker::new(
            registry,
            Arc::new(TestAuthorityLoader(authority())),
            Arc::new(TenantWideScope),
            Arc::new(TestAudit),
        );
        let principal =
            AuthenticatedAgentPrincipal::from_authenticated_request(Uuid::new_v4(), Uuid::new_v4());
        let request_context = RequestContext::from_ids(Uuid::new_v4(), Uuid::new_v4());
        let result = broker
            .invoke(
                principal,
                CapabilityCall::parse("administration.catalog.read", 1, json!({}), request_context)
                    .unwrap_or_else(|_| unreachable!()),
            )
            .await
            .unwrap_or_else(|_| unreachable!());

        assert_eq!(
            result.content()["modules"].as_array().map(Vec::len),
            Some(17)
        );
        assert!(result.content()["administration_permissions"].is_array());
    }

    #[tokio::test]
    async fn production_modules_capability_returns_a_safe_domain_failure() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
            .unwrap_or_else(|_| unreachable!());
        pool.close().await;
        let broker = CapabilityBroker::new(
            build_capability_registry(pool, license_config()),
            Arc::new(TestAuthorityLoader(authority())),
            Arc::new(TenantWideScope),
            Arc::new(TestAudit),
        );
        let request_context = RequestContext::from_ids(Uuid::new_v4(), Uuid::new_v4());
        let error = broker
            .invoke(
                AuthenticatedAgentPrincipal::from_authenticated_request(
                    Uuid::new_v4(),
                    Uuid::new_v4(),
                ),
                CapabilityCall::parse("administration.modules.list", 1, json!({}), request_context)
                    .unwrap_or_else(|_| unreachable!()),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());

        assert_eq!(error.code(), BrokerErrorCode::ExecutionFailed);
        assert_eq!(error.request_context(), request_context);
        assert_eq!(
            error.safe_message(),
            "The capability could not be completed."
        );
    }

    #[tokio::test]
    async fn production_role_and_user_capabilities_are_typed_and_fail_safely() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
            .unwrap_or_else(|_| unreachable!());
        pool.close().await;
        let broker = CapabilityBroker::new(
            build_capability_registry(pool, license_config()),
            Arc::new(TestAuthorityLoader(authority())),
            Arc::new(TenantWideScope),
            Arc::new(TestAudit),
        );
        let principal =
            AuthenticatedAgentPrincipal::from_authenticated_request(Uuid::new_v4(), Uuid::new_v4());
        let request_context = RequestContext::from_ids(Uuid::new_v4(), Uuid::new_v4());
        for (key, input) in [
            ("administration.roles.list", json!({})),
            (
                "administration.roles.read",
                json!({ "role_id": Uuid::new_v4() }),
            ),
            (
                "administration.users.list",
                json!({ "status": "active", "sort": "email" }),
            ),
            (
                "administration.users.read",
                json!({ "account_id": Uuid::new_v4() }),
            ),
        ] {
            let error = broker
                .invoke(
                    principal,
                    CapabilityCall::parse(key, 1, input, request_context)
                        .unwrap_or_else(|_| unreachable!()),
                )
                .await
                .err()
                .unwrap_or_else(|| unreachable!());
            assert_eq!(
                error.code(),
                BrokerErrorCode::ExecutionFailed,
                "unexpected broker result for {key}"
            );
        }

        let invalid = broker
            .invoke(
                principal,
                CapabilityCall::parse(
                    "administration.users.list",
                    1,
                    json!({ "status": "maybe" }),
                    request_context,
                )
                .unwrap_or_else(|_| unreachable!()),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(invalid.code(), BrokerErrorCode::InvalidInput);
    }

    #[tokio::test]
    async fn production_school_and_licensing_capabilities_fail_safely() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
            .unwrap_or_else(|_| unreachable!());
        pool.close().await;
        let broker = CapabilityBroker::new(
            build_capability_registry(pool, license_config()),
            Arc::new(TestAuthorityLoader(authority())),
            Arc::new(TenantWideScope),
            Arc::new(TestAudit),
        );
        let principal =
            AuthenticatedAgentPrincipal::from_authenticated_request(Uuid::new_v4(), Uuid::new_v4());
        for key in [
            "administration.school_settings.read",
            "administration.licensing.read",
        ] {
            let error = broker
                .invoke(
                    principal,
                    CapabilityCall::parse(
                        key,
                        1,
                        json!({}),
                        RequestContext::from_ids(Uuid::new_v4(), Uuid::new_v4()),
                    )
                    .unwrap_or_else(|_| unreachable!()),
                )
                .await
                .err()
                .unwrap_or_else(|| unreachable!());
            assert_eq!(error.code(), BrokerErrorCode::ExecutionFailed, "{key}");
        }
    }
}
