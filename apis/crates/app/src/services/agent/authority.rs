//! Reloads current person and tenant access for each Agent broker check.
//!
//! This adapter owns no authorization policy or persistence. It delegates user
//! and entitlement reads to their app services, and every failure is reduced to
//! the broker's stable, redacted authority-unavailable boundary.

use std::sync::Arc;

use async_trait::async_trait;
use cp_agent::{
    AuthenticatedAgentPrincipal, AuthorityLoadError, AuthorityLoader, CurrentAuthority,
};
use cp_common::AccessContext;
use sqlx::PgPool;
use uuid::Uuid;

use crate::services::{
    access::{models::EffectiveAccess, ops::AccessOps},
    users::ops::UserOps,
};

#[derive(Clone)]
struct ReloadedAgentUser {
    tenant_id: Uuid,
    role_keys: Vec<String>,
    is_active: bool,
}

#[derive(Debug, Clone, Copy)]
struct AuthorityDataError;

#[async_trait]
trait AuthorityDataSource: Send + Sync {
    async fn user(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<ReloadedAgentUser>, AuthorityDataError>;

    async fn effective_access(
        &self,
        tenant_id: Uuid,
        role_keys: &[String],
    ) -> Result<EffectiveAccess, AuthorityDataError>;
}

#[derive(Clone)]
struct PostgresAuthorityDataSource {
    pool: PgPool,
}

#[async_trait]
impl AuthorityDataSource for PostgresAuthorityDataSource {
    async fn user(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<ReloadedAgentUser>, AuthorityDataError> {
        UserOps::get_user_by_id(&self.pool, tenant_id, user_id)
            .await
            .map(|user| {
                user.map(|user| ReloadedAgentUser {
                    tenant_id: user.tenant_id,
                    role_keys: user.roles,
                    is_active: user.is_active,
                })
            })
            .map_err(|_| AuthorityDataError)
    }

    async fn effective_access(
        &self,
        tenant_id: Uuid,
        role_keys: &[String],
    ) -> Result<EffectiveAccess, AuthorityDataError> {
        AccessOps::effective_access(&self.pool, tenant_id, role_keys)
            .await
            .map_err(|_| AuthorityDataError)
    }
}

/// Production authority adapter for app-owned users, roles, and entitlements.
#[derive(Clone)]
pub struct AppAuthorityLoader {
    source: Arc<dyn AuthorityDataSource>,
}

impl AppAuthorityLoader {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self {
            source: Arc::new(PostgresAuthorityDataSource { pool }),
        }
    }

    #[cfg(test)]
    fn with_source(source: Arc<dyn AuthorityDataSource>) -> Self {
        Self { source }
    }
}

#[async_trait]
impl AuthorityLoader for AppAuthorityLoader {
    async fn load(
        &self,
        principal: AuthenticatedAgentPrincipal,
    ) -> Result<CurrentAuthority, AuthorityLoadError> {
        let user = self
            .source
            .user(principal.tenant_id(), principal.user_id())
            .await
            .map_err(|_| AuthorityLoadError)?
            .filter(|user| user.is_active && user.tenant_id == principal.tenant_id())
            .ok_or(AuthorityLoadError)?;

        let effective = self
            .source
            .effective_access(user.tenant_id, &user.role_keys)
            .await
            .map_err(|_| AuthorityLoadError)?;

        Ok(CurrentAuthority::from_reloaded_access(AccessContext {
            role_keys: user.role_keys,
            permissions: effective.permissions,
            enabled_modules: effective.enabled_modules,
            entitlements: effective.entitlements,
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        sync::{Arc, Mutex},
    };

    use async_trait::async_trait;
    use cp_agent::{
        AuthenticatedAgentPrincipal, AuthorizedRecordScope, BrokerAuditError, BrokerAuditRecord,
        BrokerAuditSink, BrokerError, BrokerErrorCode, CapabilityBroker, CapabilityCall,
        CapabilityCallId, CapabilityExecutionProof, CapabilityPreparationRejection,
        CapabilityWorkerLease, DurabilityProofRejected, PreparedCapabilityCall,
        PreparedCapabilityCallFacts, PreparedCapabilityCallVerifier, RecordScopeAuthorizer,
        RecordScopeDenied,
    };
    use cp_audit::RequestContext;
    use cp_common::{
        EntitlementSnapshot, LeaseLifecycle, ModuleEntitlementState, ProductOperation,
    };
    use serde_json::json;
    use sqlx::postgres::PgPoolOptions;
    use uuid::Uuid;

    use crate::{
        config::LicenseConfig,
        services::{access::models::EffectiveAccess, agent::build_capability_registry},
    };

    use super::{AppAuthorityLoader, AuthorityDataError, AuthorityDataSource, ReloadedAgentUser};

    const PROCUREMENT_CAPABILITY: &str = "procurement.suppliers.list";
    const ADMINISTRATION_CAPABILITY: &str = "administration.catalog.read";
    const ROLE_KEY: &str = "agent_operator";

    #[derive(Clone)]
    struct FakeAuthorityState {
        user_id: Uuid,
        user: Option<ReloadedAgentUser>,
        role_permissions: BTreeMap<String, Vec<String>>,
        lease: LeaseLifecycle,
        modules: Vec<(String, ModuleEntitlementState)>,
        exhausted_hard_limits: Vec<String>,
        app_version_supported: bool,
    }

    struct MutableAuthoritySource {
        state: Mutex<FakeAuthorityState>,
    }

    impl MutableAuthoritySource {
        fn update(&self, update: impl FnOnce(&mut FakeAuthorityState)) {
            update(
                &mut self
                    .state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()),
            );
        }
    }

    #[async_trait]
    impl AuthorityDataSource for MutableAuthoritySource {
        async fn user(
            &self,
            tenant_id: Uuid,
            user_id: Uuid,
        ) -> Result<Option<ReloadedAgentUser>, AuthorityDataError> {
            let state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            Ok(state
                .user
                .clone()
                .filter(|user| user.tenant_id == tenant_id && state.user_id == user_id))
        }

        async fn effective_access(
            &self,
            tenant_id: Uuid,
            role_keys: &[String],
        ) -> Result<EffectiveAccess, AuthorityDataError> {
            let state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(user) = state
                .user
                .as_ref()
                .filter(|user| user.tenant_id == tenant_id)
            else {
                return Err(AuthorityDataError);
            };
            if user.role_keys != role_keys {
                return Err(AuthorityDataError);
            }
            let permissions = role_keys
                .iter()
                .filter_map(|role_key| state.role_permissions.get(role_key))
                .flatten()
                .cloned()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let enabled_modules = state
                .modules
                .iter()
                .filter(|(_, module_state)| *module_state == ModuleEntitlementState::Enabled)
                .map(|(module_key, _)| module_key.clone())
                .collect();
            let entitlements =
                EntitlementSnapshot::new(state.lease, state.modules.clone(), Vec::<String>::new())
                    .map_err(|_| AuthorityDataError)?
                    .with_exhausted_hard_limits(state.exhausted_hard_limits.clone())
                    .with_app_version_supported(state.app_version_supported);
            Ok(EffectiveAccess {
                role_names: role_keys.to_vec(),
                permissions,
                enabled_modules,
                entitlements,
                record_scopes: cp_common::RecordScopeGrants::empty(),
            })
        }
    }

    struct AuthorityFixture {
        source: Arc<MutableAuthoritySource>,
        loader: Arc<AppAuthorityLoader>,
        principal: AuthenticatedAgentPrincipal,
    }

    impl AuthorityFixture {
        fn new(permissions: &[&str], modules: &[&str]) -> Self {
            let tenant_id = Uuid::new_v4();
            let user_id = Uuid::new_v4();
            let source = Arc::new(MutableAuthoritySource {
                state: Mutex::new(FakeAuthorityState {
                    user_id,
                    user: Some(ReloadedAgentUser {
                        tenant_id,
                        role_keys: vec![ROLE_KEY.to_string()],
                        is_active: true,
                    }),
                    role_permissions: BTreeMap::from([(
                        ROLE_KEY.to_string(),
                        permissions
                            .iter()
                            .map(|permission| (*permission).to_string())
                            .collect(),
                    )]),
                    lease: LeaseLifecycle::Active,
                    modules: modules
                        .iter()
                        .map(|module| ((*module).to_string(), ModuleEntitlementState::Enabled))
                        .collect(),
                    exhausted_hard_limits: Vec::new(),
                    app_version_supported: true,
                }),
            });
            let loader = Arc::new(AppAuthorityLoader::with_source(source.clone()));
            Self {
                source,
                loader,
                principal: AuthenticatedAgentPrincipal::from_authenticated_request(
                    tenant_id, user_id,
                ),
            }
        }

        fn broker(&self, audit: Arc<TestAudit>) -> CapabilityBroker {
            let pool = PgPoolOptions::new()
                .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
                .expect("test registry pool URL must be valid");
            CapabilityBroker::new(
                build_capability_registry(pool, license_config()),
                self.loader.clone(),
                Arc::new(TenantWideTestScope),
                Arc::new(MatchingProofVerifier),
                audit,
            )
        }
    }

    struct TenantWideTestScope;

    #[async_trait]
    impl RecordScopeAuthorizer for TenantWideTestScope {
        async fn authorize(
            &self,
            _principal: AuthenticatedAgentPrincipal,
            _authority: &cp_agent::CurrentAuthority,
            _operation: &ProductOperation,
            _scope: &cp_agent::CapabilityScope,
        ) -> Result<AuthorizedRecordScope, RecordScopeDenied> {
            Ok(AuthorizedRecordScope::granted())
        }
    }

    struct MatchingProofVerifier;

    #[async_trait]
    impl PreparedCapabilityCallVerifier for MatchingProofVerifier {
        async fn verify_and_consume(
            &self,
            principal: AuthenticatedAgentPrincipal,
            facts: &PreparedCapabilityCallFacts,
            proof: &CapabilityExecutionProof,
        ) -> Result<(), DurabilityProofRejected> {
            let matches = proof.tenant_id() == principal.tenant_id()
                && proof.user_id() == principal.user_id()
                && proof.capability_call_id() == facts.capability_call_id()
                && facts
                    .agent_run_id()
                    .is_some_and(|run_id| run_id == proof.run_id());
            if matches {
                Ok(())
            } else {
                Err(DurabilityProofRejected)
            }
        }
    }

    #[derive(Default)]
    struct TestAudit {
        records: Mutex<Vec<BrokerAuditRecord>>,
    }

    impl TestAudit {
        fn last_reason(&self) -> Option<&'static str> {
            self.records
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .last()
                .map(|record| record.reason)
        }
    }

    #[async_trait]
    impl BrokerAuditSink for TestAudit {
        async fn record(&self, record: BrokerAuditRecord) -> Result<(), BrokerAuditError> {
            self.records
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(record);
            Ok(())
        }
    }

    fn license_config() -> LicenseConfig {
        LicenseConfig {
            trusted_public_keys: BTreeMap::new(),
            issuer: "campus-pilot-control-plane".to_string(),
            audience: "campus-pilot".to_string(),
            control_plane_url: None,
            credential_key_base64: None,
            installation_name: "Agent authority test".to_string(),
        }
    }

    fn capability_call(key: &str) -> CapabilityCall {
        CapabilityCall::parse(
            key,
            1,
            json!({}),
            RequestContext::from_ids(Uuid::new_v4(), Uuid::new_v4()),
        )
        .expect("test capability call must match the canonical catalog")
        .with_agent_run_id(Uuid::new_v4())
    }

    async fn prepare(
        broker: &CapabilityBroker,
        principal: AuthenticatedAgentPrincipal,
        key: &str,
    ) -> Result<PreparedCapabilityCall, CapabilityPreparationRejection> {
        broker
            .prepare(
                principal,
                CapabilityCallId::from_trusted_runtime(Uuid::new_v4()),
                capability_call(key),
            )
            .await
    }

    async fn rejected_preparation(
        broker: &CapabilityBroker,
        principal: AuthenticatedAgentPrincipal,
        failure_message: &'static str,
    ) -> CapabilityPreparationRejection {
        match prepare(broker, principal, PROCUREMENT_CAPABILITY).await {
            Ok(_) => panic!("{failure_message}"),
            Err(error) => error,
        }
    }

    fn execution_proof(
        principal: AuthenticatedAgentPrincipal,
        prepared: &PreparedCapabilityCall,
    ) -> CapabilityExecutionProof {
        CapabilityExecutionProof::parse(
            principal,
            prepared.facts().capability_call_id(),
            prepared
                .facts()
                .agent_run_id()
                .expect("test call must carry an Agent run ID"),
            CapabilityWorkerLease::parse("agent-authority-test-worker", Uuid::new_v4(), 1)
                .expect("test worker lease must be valid"),
            Uuid::new_v4(),
        )
        .expect("test execution proof must be valid")
    }

    fn assert_denied(error: &BrokerError, audit: &TestAudit, reason: &'static str) {
        assert_eq!(error.code(), BrokerErrorCode::AccessDenied);
        assert_eq!(
            error.safe_message(),
            "This capability is not available for the current account."
        );
        assert_eq!(audit.last_reason(), Some(reason));
    }

    fn assert_preparation_denied(
        rejection: &CapabilityPreparationRejection,
        audit: &TestAudit,
        reason: &'static str,
    ) {
        assert_eq!(rejection.code(), BrokerErrorCode::AccessDenied);
        assert_eq!(
            rejection.safe_message(),
            "This capability is not available for the current account."
        );
        assert_eq!(rejection.reason_code(), reason);
        assert_eq!(audit.last_reason(), Some(reason));
    }

    #[tokio::test]
    async fn missing_agent_entitlement_fails_closed() {
        let fixture = AuthorityFixture::new(
            &["agent:run", "procurement:view"],
            &["procurement", "finance", "hr_payroll"],
        );
        let audit = Arc::new(TestAudit::default());
        let rejection = rejected_preparation(
            &fixture.broker(audit.clone()),
            fixture.principal,
            "Agent must be licensed before capability preparation",
        )
        .await;

        assert_preparation_denied(&rejection, &audit, "module_not_entitled");
    }

    #[tokio::test]
    async fn missing_target_dependency_fails_closed() {
        let fixture = AuthorityFixture::new(
            &["agent:run", "procurement:view"],
            &["agent", "procurement", "finance"],
        );
        let audit = Arc::new(TestAudit::default());
        let rejection = rejected_preparation(
            &fixture.broker(audit.clone()),
            fixture.principal,
            "every canonical target dependency must be entitled",
        )
        .await;

        assert_preparation_denied(&rejection, &audit, "dependency_missing");
    }

    #[tokio::test]
    async fn missing_target_entitlement_fails_closed() {
        let fixture = AuthorityFixture::new(
            &["agent:run", "procurement:view"],
            &["agent", "finance", "hr_payroll"],
        );
        let audit = Arc::new(TestAudit::default());
        let rejection = rejected_preparation(
            &fixture.broker(audit.clone()),
            fixture.principal,
            "the canonical target module must be entitled",
        )
        .await;

        assert_preparation_denied(&rejection, &audit, "module_not_entitled");
    }

    #[tokio::test]
    async fn missing_target_permission_fails_closed() {
        let fixture = AuthorityFixture::new(
            &["agent:run"],
            &["agent", "procurement", "finance", "hr_payroll"],
        );
        let audit = Arc::new(TestAudit::default());
        let rejection = rejected_preparation(
            &fixture.broker(audit.clone()),
            fixture.principal,
            "the target operation permission must be current",
        )
        .await;

        assert_preparation_denied(&rejection, &audit, "permission_denied");
    }

    #[tokio::test]
    async fn revoked_lease_is_rechecked_before_execution() {
        let fixture = AuthorityFixture::new(
            &["agent:run", "procurement:view"],
            &["agent", "procurement", "finance", "hr_payroll"],
        );
        let audit = Arc::new(TestAudit::default());
        let broker = fixture.broker(audit.clone());
        let mut prepared = prepare(&broker, fixture.principal, PROCUREMENT_CAPABILITY)
            .await
            .expect("current authority should allow preparation");
        fixture.source.update(|state| {
            state.lease = LeaseLifecycle::Revoked;
        });

        let proof = execution_proof(fixture.principal, &prepared);
        let error = broker
            .execute_prepared(&mut prepared, proof)
            .await
            .expect_err("a revoked lease must stop prepared execution");

        assert_denied(&error, &audit, "license_revoked");
    }

    #[tokio::test]
    async fn role_assignment_revocation_is_reloaded_before_execution() {
        let fixture = AuthorityFixture::new(
            &["agent:run", "procurement:view"],
            &["agent", "procurement", "finance", "hr_payroll"],
        );
        let audit = Arc::new(TestAudit::default());
        let broker = fixture.broker(audit.clone());
        let mut prepared = prepare(&broker, fixture.principal, PROCUREMENT_CAPABILITY)
            .await
            .expect("current role assignment should allow preparation");
        fixture.source.update(|state| {
            state
                .user
                .as_mut()
                .expect("test user must exist")
                .role_keys
                .clear();
        });

        let proof = execution_proof(fixture.principal, &prepared);
        let error = broker
            .execute_prepared(&mut prepared, proof)
            .await
            .expect_err("a revoked role assignment must stop prepared execution");

        assert_denied(&error, &audit, "permission_denied");
    }

    #[tokio::test]
    async fn inactive_user_fails_with_redacted_authority_reason() {
        let fixture = AuthorityFixture::new(
            &["agent:run", "procurement:view"],
            &["agent", "procurement", "finance", "hr_payroll"],
        );
        let audit = Arc::new(TestAudit::default());
        let broker = fixture.broker(audit.clone());
        let mut prepared = prepare(&broker, fixture.principal, PROCUREMENT_CAPABILITY)
            .await
            .expect("active user should allow preparation");
        fixture.source.update(|state| {
            state.user.as_mut().expect("test user must exist").is_active = false;
        });

        let proof = execution_proof(fixture.principal, &prepared);
        let error = broker
            .execute_prepared(&mut prepared, proof)
            .await
            .expect_err("an inactive user must stop prepared execution");

        assert_eq!(error.code(), BrokerErrorCode::AuthorityUnavailable);
        assert_eq!(error.safe_message(), "Current access could not be loaded.");
        assert_eq!(audit.last_reason(), Some("authority_unavailable"));
    }

    #[tokio::test]
    async fn current_authority_allows_canonical_capability_execution() {
        let fixture = AuthorityFixture::new(
            &["agent:run", "administration:view"],
            &["agent", "administration"],
        );
        let audit = Arc::new(TestAudit::default());
        let broker = fixture.broker(audit.clone());
        let mut prepared = prepare(&broker, fixture.principal, ADMINISTRATION_CAPABILITY)
            .await
            .expect("current authority should allow preparation");
        let proof = execution_proof(fixture.principal, &prepared);

        broker
            .execute_prepared(&mut prepared, proof)
            .await
            .expect("current authority should allow canonical capability execution");
        assert_eq!(audit.last_reason(), Some("completed"));
    }
}
