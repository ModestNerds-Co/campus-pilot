//! Assembles production Agent capability adapters over existing domain services.

mod administration;

use cp_agent::CapabilityRegistry;
use sqlx::PgPool;

use administration::{AdministrationCatalogCapability, AdministrationModulesCapability};

#[must_use]
pub fn build_capability_registry(pool: PgPool) -> CapabilityRegistry {
    let mut registry = CapabilityRegistry::from_product_catalog()
        .unwrap_or_else(|error| panic!("invalid product operation catalogue: {error}"));
    registry
        .register(AdministrationCatalogCapability::new())
        .unwrap_or_else(|error| panic!("invalid Administration catalogue capability: {error}"));
    registry
        .register(AdministrationModulesCapability::new(pool))
        .unwrap_or_else(|error| panic!("invalid Administration modules capability: {error}"));
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
    use uuid::Uuid;

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
            permissions: vec!["agent:run".to_string(), "administration:view".to_string()],
            enabled_modules: vec!["agent".to_string(), "administration".to_string()],
            entitlements: EntitlementSnapshot::new(
                LeaseLifecycle::Active,
                [
                    ("agent".to_string(), ModuleEntitlementState::Enabled),
                    (
                        "administration".to_string(),
                        ModuleEntitlementState::Enabled,
                    ),
                ],
                Vec::<String>::new(),
            )
            .unwrap_or_else(|_| unreachable!()),
        })
    }

    #[tokio::test]
    async fn production_registry_executes_the_real_administration_catalogue_read() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
            .unwrap_or_else(|_| unreachable!());
        let registry = build_capability_registry(pool);
        let keys = registry
            .descriptors()
            .into_iter()
            .map(|descriptor| descriptor.key().as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            keys,
            vec!["administration.catalog.read", "administration.modules.list"]
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
            build_capability_registry(pool),
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
}
