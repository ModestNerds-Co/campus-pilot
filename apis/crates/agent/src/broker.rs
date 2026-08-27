//! Enforces the single execution boundary for every Agent capability call.
//!
//! This foundation performs no model/provider work and registers no product
//! capabilities by default. Only typed read-only handlers can execute.

use std::sync::Arc;

use async_trait::async_trait;
use cp_common::{AgentExposure, ProductOperation, RuntimeAccessChecks};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    audit::{BrokerAuditError, BrokerAuditOutcome, BrokerAuditRecord, BrokerAuditSink},
    handler::ErasedCapabilityError,
    registry::CapabilityRegistry,
    types::{
        AuthenticatedAgentPrincipal, AuthorizedCapabilityContext, AuthorizedRecordScope,
        BrokerError, BrokerErrorCode, CapabilityCall, CapabilityResult, CapabilityScope,
        CurrentAuthority,
    },
};

const INVOKE_ACTION: &str = "agent.capabilities.invoke";

struct BrokerFailure<'a> {
    outcome: BrokerAuditOutcome,
    action_key: &'a str,
    code: BrokerErrorCode,
    message: &'static str,
    reason: &'static str,
    target: Option<crate::types::CapabilityResource>,
}

impl<'a> BrokerFailure<'a> {
    const fn denied(
        action_key: &'a str,
        code: BrokerErrorCode,
        message: &'static str,
        reason: &'static str,
    ) -> Self {
        Self {
            outcome: BrokerAuditOutcome::Denied,
            action_key,
            code,
            message,
            reason,
            target: None,
        }
    }

    const fn failed(
        action_key: &'a str,
        code: BrokerErrorCode,
        message: &'static str,
        reason: &'static str,
    ) -> Self {
        Self {
            outcome: BrokerAuditOutcome::Failed,
            action_key,
            code,
            message,
            reason,
            target: None,
        }
    }

    fn with_target(mut self, target: Option<crate::types::CapabilityResource>) -> Self {
        self.target = target;
        self
    }
}

struct AuditDecision<'a> {
    capability_call_id: Uuid,
    action_key: &'a str,
    outcome: BrokerAuditOutcome,
    reason: &'static str,
    target: Option<crate::types::CapabilityResource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("current Agent authority could not be loaded")]
pub struct AuthorityLoadError;

#[async_trait]
pub trait AuthorityLoader: Send + Sync {
    /// Reloads the active person, current roles, permissions, modules, lease,
    /// and hard-limit evidence for this exact call.
    async fn load(
        &self,
        principal: AuthenticatedAgentPrincipal,
    ) -> Result<CurrentAuthority, AuthorityLoadError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("record scope does not allow this capability call")]
pub struct RecordScopeDenied;

#[async_trait]
pub trait RecordScopeAuthorizer: Send + Sync {
    async fn authorize(
        &self,
        principal: AuthenticatedAgentPrincipal,
        authority: &CurrentAuthority,
        operation: &ProductOperation,
        scope: &CapabilityScope,
    ) -> Result<AuthorizedRecordScope, RecordScopeDenied>;
}

pub struct CapabilityBroker {
    registry: Arc<CapabilityRegistry>,
    authority_loader: Arc<dyn AuthorityLoader>,
    scope_authorizer: Arc<dyn RecordScopeAuthorizer>,
    audit_sink: Arc<dyn BrokerAuditSink>,
}

impl CapabilityBroker {
    #[must_use]
    pub fn new(
        registry: CapabilityRegistry,
        authority_loader: Arc<dyn AuthorityLoader>,
        scope_authorizer: Arc<dyn RecordScopeAuthorizer>,
        audit_sink: Arc<dyn BrokerAuditSink>,
    ) -> Self {
        Self {
            registry: Arc::new(registry),
            authority_loader,
            scope_authorizer,
            audit_sink,
        }
    }

    #[must_use]
    pub fn registry(&self) -> &CapabilityRegistry {
        &self.registry
    }

    pub async fn invoke(
        &self,
        principal: AuthenticatedAgentPrincipal,
        call: CapabilityCall,
    ) -> Result<CapabilityResult, BrokerError> {
        let capability_call_id = Uuid::new_v4();
        let Some(operation) = self.registry.operation(call.key()) else {
            return self
                .reject(
                    principal,
                    &call,
                    capability_call_id,
                    BrokerFailure::denied(
                        INVOKE_ACTION,
                        BrokerErrorCode::UnknownCapability,
                        "The requested capability does not exist.",
                        "unknown_capability",
                    ),
                )
                .await;
        };

        match operation.agent_exposure() {
            AgentExposure::ApprovalRequired => {
                return self
                    .reject(
                        principal,
                        &call,
                        capability_call_id,
                        BrokerFailure::denied(
                            operation.key(),
                            BrokerErrorCode::ApprovalRequired,
                            "This capability requires an approved proposal.",
                            "approval_required",
                        ),
                    )
                    .await;
            }
            AgentExposure::HumanOnly { .. } => {
                return self
                    .reject(
                        principal,
                        &call,
                        capability_call_id,
                        BrokerFailure::denied(
                            operation.key(),
                            BrokerErrorCode::HumanOnly,
                            "This operation must be completed by a person.",
                            "human_only",
                        ),
                    )
                    .await;
            }
            AgentExposure::Prohibited { .. } => {
                return self
                    .reject(
                        principal,
                        &call,
                        capability_call_id,
                        BrokerFailure::denied(
                            operation.key(),
                            BrokerErrorCode::Prohibited,
                            "This operation is not available to Agent.",
                            "prohibited",
                        ),
                    )
                    .await;
            }
            AgentExposure::Exposed => {}
        }

        let Some(handler) = self.registry.handler(call.key(), call.version()) else {
            let (code, message, reason) = if self.registry.has_any_version(call.key()) {
                (
                    BrokerErrorCode::UnsupportedVersion,
                    "The requested capability version is not supported.",
                    "unsupported_version",
                )
            } else {
                (
                    BrokerErrorCode::CapabilityUnavailable,
                    "This capability has not been released.",
                    "capability_unavailable",
                )
            };
            return self
                .reject(
                    principal,
                    &call,
                    capability_call_id,
                    BrokerFailure::denied(operation.key(), code, message, reason),
                )
                .await;
        };

        let authority = match self.authority_loader.load(principal).await {
            Ok(authority) => authority,
            Err(_) => {
                return self
                    .reject(
                        principal,
                        &call,
                        capability_call_id,
                        BrokerFailure::failed(
                            operation.key(),
                            BrokerErrorCode::AuthorityUnavailable,
                            "Current access could not be loaded.",
                            "authority_unavailable",
                        ),
                    )
                    .await;
            }
        };

        let agent_gate = agent_gate(operation);
        let agent_decision = authority
            .access()
            .evaluate_operation(&agent_gate, RuntimeAccessChecks::default());
        if !agent_decision.allowed {
            return self
                .reject(
                    principal,
                    &call,
                    capability_call_id,
                    BrokerFailure::denied(
                        operation.key(),
                        BrokerErrorCode::AccessDenied,
                        "This capability is not available for the current account.",
                        agent_decision.reason.as_str(),
                    ),
                )
                .await;
        }

        let operation_decision = authority
            .access()
            .evaluate_operation(operation, RuntimeAccessChecks::default());
        if !operation_decision.allowed {
            return self
                .reject(
                    principal,
                    &call,
                    capability_call_id,
                    BrokerFailure::denied(
                        operation.key(),
                        BrokerErrorCode::AccessDenied,
                        "This capability is not available for the current account.",
                        operation_decision.reason.as_str(),
                    ),
                )
                .await;
        }

        if input_contains_reserved_identity(call.input()) {
            return self
                .reject(
                    principal,
                    &call,
                    capability_call_id,
                    BrokerFailure::denied(
                        operation.key(),
                        BrokerErrorCode::InvalidInput,
                        "Capability input contains a reserved identity field.",
                        "reserved_identity_input",
                    ),
                )
                .await;
        }

        let parsed = match handler.parse_input(call.input().clone()) {
            Ok(input) => input,
            Err(_) => {
                return self
                    .reject(
                        principal,
                        &call,
                        capability_call_id,
                        BrokerFailure::denied(
                            operation.key(),
                            BrokerErrorCode::InvalidInput,
                            "Capability input is invalid.",
                            "invalid_input",
                        ),
                    )
                    .await;
            }
        };
        let scope = match handler.scope(&parsed) {
            Ok(scope) => scope,
            Err(_) => {
                return self
                    .reject(
                        principal,
                        &call,
                        capability_call_id,
                        BrokerFailure::failed(
                            operation.key(),
                            BrokerErrorCode::ExecutionFailed,
                            "The capability could not be prepared.",
                            "handler_contract_failed",
                        ),
                    )
                    .await;
            }
        };
        let target = scope.primary_resource().cloned();
        let scope_grant = match self
            .scope_authorizer
            .authorize(principal, &authority, operation, &scope)
            .await
        {
            Ok(grant) => grant,
            Err(_) => {
                return self
                    .reject(
                        principal,
                        &call,
                        capability_call_id,
                        BrokerFailure::denied(
                            operation.key(),
                            BrokerErrorCode::RecordScopeDenied,
                            "The requested records are outside the current access scope.",
                            "record_scope_denied",
                        )
                        .with_target(target),
                    )
                    .await;
            }
        };

        let context =
            AuthorizedCapabilityContext::new(principal, call.request_context(), scope, scope_grant);
        let content = match handler.execute(context, parsed).await {
            Ok(content) => content,
            Err(ErasedCapabilityError::Execution(error)) => {
                return self
                    .reject(
                        principal,
                        &call,
                        capability_call_id,
                        BrokerFailure::failed(
                            operation.key(),
                            BrokerErrorCode::ExecutionFailed,
                            "The capability could not be completed.",
                            execution_reason(error.code().as_str()),
                        )
                        .with_target(target),
                    )
                    .await;
            }
            Err(ErasedCapabilityError::InvalidInput | ErasedCapabilityError::Contract) => {
                return self
                    .reject(
                        principal,
                        &call,
                        capability_call_id,
                        BrokerFailure::failed(
                            operation.key(),
                            BrokerErrorCode::ExecutionFailed,
                            "The capability could not be completed.",
                            "handler_contract_failed",
                        )
                        .with_target(target),
                    )
                    .await;
            }
        };

        self.audit(
            principal,
            &call,
            AuditDecision {
                capability_call_id,
                action_key: operation.key(),
                outcome: BrokerAuditOutcome::Succeeded,
                reason: "completed",
                target,
            },
        )
        .await
        .map_err(|_| {
            BrokerError::new(
                BrokerErrorCode::AuditUnavailable,
                "Capability audit evidence could not be recorded.",
                call.request_context(),
            )
        })?;

        Ok(CapabilityResult::new(
            call.key().clone(),
            call.version(),
            content,
            call.request_context(),
        ))
    }

    async fn reject<T>(
        &self,
        principal: AuthenticatedAgentPrincipal,
        call: &CapabilityCall,
        capability_call_id: Uuid,
        failure: BrokerFailure<'_>,
    ) -> Result<T, BrokerError> {
        self.audit(
            principal,
            call,
            AuditDecision {
                capability_call_id,
                action_key: failure.action_key,
                outcome: failure.outcome,
                reason: failure.reason,
                target: failure.target,
            },
        )
        .await
        .map_err(|_| {
            BrokerError::new(
                BrokerErrorCode::AuditUnavailable,
                "Capability audit evidence could not be recorded.",
                call.request_context(),
            )
        })?;
        Err(BrokerError::new(
            failure.code,
            failure.message,
            call.request_context(),
        ))
    }

    async fn audit(
        &self,
        principal: AuthenticatedAgentPrincipal,
        call: &CapabilityCall,
        decision: AuditDecision<'_>,
    ) -> Result<(), BrokerAuditError> {
        self.audit_sink
            .record(BrokerAuditRecord {
                principal,
                request_context: call.request_context(),
                capability_call_id: decision.capability_call_id,
                action_key: decision.action_key.to_string(),
                capability_version: call.version().get(),
                agent_run_id: call.agent_run_id(),
                target: decision.target,
                outcome: decision.outcome,
                reason: decision.reason,
            })
            .await
    }
}

fn agent_gate(operation: &ProductOperation) -> ProductOperation {
    ProductOperation::route(
        INVOKE_ACTION,
        "agent",
        "agent:run",
        operation.effect(),
        AgentExposure::Exposed,
        true,
    )
}

fn input_contains_reserved_identity(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            matches!(
                key.as_str(),
                "tenantId"
                    | "tenant_id"
                    | "userId"
                    | "user_id"
                    | "actorUserId"
                    | "actor_user_id"
                    | "requesterUserId"
                    | "requester_user_id"
            ) || input_contains_reserved_identity(value)
        }),
        Value::Array(values) => values.iter().any(input_contains_reserved_identity),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => false,
    }
}

fn execution_reason(code: &str) -> &'static str {
    match code {
        "dependency_unavailable" => "handler_dependency_unavailable",
        "conflict" => "handler_conflict",
        "invalid_state" => "handler_invalid_state",
        _ => "handler_internal",
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use async_trait::async_trait;
    use cp_audit::RequestContext;
    use cp_common::{
        AccessContext, AgentExposure, EntitlementSnapshot, LeaseLifecycle, ModuleEntitlementState,
        OperationEffect, ProductOperation,
    };
    use serde::{Deserialize, Serialize};
    use serde_json::{Value, json};
    use uuid::Uuid;

    use crate::{
        audit::{BrokerAuditError, BrokerAuditRecord, BrokerAuditSink},
        descriptor::{
            CapabilityDescriptor, CapabilityIdentity, CapabilityKey, CapabilityPolicy,
            CapabilityRedaction, CapabilitySchemas, CapabilityVersion, DataSensitivity,
            ObjectSchema, RedactionProjection,
        },
        handler::Capability,
        registry::CapabilityRegistry,
        types::{
            AuthenticatedAgentPrincipal, AuthorizedCapabilityContext, AuthorizedRecordScope,
            BrokerErrorCode, CapabilityCall, CapabilityExecutionError,
            CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, CurrentAuthority,
        },
    };

    use super::{
        AuthorityLoadError, AuthorityLoader, CapabilityBroker, RecordScopeAuthorizer,
        RecordScopeDenied,
    };

    const READ_KEY: &str = "administration.catalog.read";

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    struct ReadInput {
        query: String,
        record_id: Option<String>,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ReadOutput {
        echoed: String,
        tenant_id: Uuid,
    }

    struct ReadCapability {
        descriptor: CapabilityDescriptor,
        executions: Arc<AtomicUsize>,
        failure: Option<CapabilityExecutionErrorCode>,
    }

    #[async_trait]
    impl Capability for ReadCapability {
        type Input = ReadInput;
        type Output = ReadOutput;

        fn descriptor(&self) -> &CapabilityDescriptor {
            &self.descriptor
        }

        fn scope(&self, input: &Self::Input) -> CapabilityScope {
            input
                .record_id
                .as_ref()
                .map_or(CapabilityScope::TenantWide, |id| {
                    CapabilityScope::resources([CapabilityResource::parse("catalog_record", id)
                        .unwrap_or_else(|_| unreachable!())])
                    .unwrap_or_else(|_| unreachable!())
                })
        }

        async fn execute(
            &self,
            context: AuthorizedCapabilityContext,
            input: Self::Input,
        ) -> Result<Self::Output, CapabilityExecutionError> {
            self.executions.fetch_add(1, Ordering::SeqCst);
            if let Some(code) = self.failure {
                return Err(CapabilityExecutionError::new(
                    code,
                    "The test capability could not complete.",
                ));
            }
            Ok(ReadOutput {
                echoed: input.query,
                tenant_id: context.principal().tenant_id(),
            })
        }
    }

    #[derive(Clone)]
    enum AuthorityState {
        Available(CurrentAuthority),
        Unavailable,
    }

    struct FakeAuthorityLoader {
        state: AuthorityState,
        loads: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl AuthorityLoader for FakeAuthorityLoader {
        async fn load(
            &self,
            _principal: AuthenticatedAgentPrincipal,
        ) -> Result<CurrentAuthority, AuthorityLoadError> {
            self.loads.fetch_add(1, Ordering::SeqCst);
            match &self.state {
                AuthorityState::Available(authority) => Ok(authority.clone()),
                AuthorityState::Unavailable => Err(AuthorityLoadError),
            }
        }
    }

    struct FakeScopeAuthorizer {
        allowed: bool,
        checks: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl RecordScopeAuthorizer for FakeScopeAuthorizer {
        async fn authorize(
            &self,
            _principal: AuthenticatedAgentPrincipal,
            _authority: &CurrentAuthority,
            _operation: &ProductOperation,
            _scope: &CapabilityScope,
        ) -> Result<AuthorizedRecordScope, RecordScopeDenied> {
            self.checks.fetch_add(1, Ordering::SeqCst);
            if self.allowed {
                Ok(AuthorizedRecordScope::granted())
            } else {
                Err(RecordScopeDenied)
            }
        }
    }

    struct FakeAuditSink {
        fail: bool,
        records: Arc<Mutex<Vec<BrokerAuditRecord>>>,
    }

    #[async_trait]
    impl BrokerAuditSink for FakeAuditSink {
        async fn record(&self, record: BrokerAuditRecord) -> Result<(), BrokerAuditError> {
            if self.fail {
                return Err(BrokerAuditError);
            }
            self.records
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(record);
            Ok(())
        }
    }

    struct BrokerFixture {
        broker: CapabilityBroker,
        principal: AuthenticatedAgentPrincipal,
        request_context: RequestContext,
        executions: Arc<AtomicUsize>,
        loads: Arc<AtomicUsize>,
        scope_checks: Arc<AtomicUsize>,
        audit_records: Arc<Mutex<Vec<BrokerAuditRecord>>>,
    }

    fn operation(key: &str, effect: OperationEffect, exposure: AgentExposure) -> ProductOperation {
        ProductOperation::route(
            key,
            "administration",
            "administration:view",
            effect,
            exposure,
            false,
        )
    }

    fn schema(properties: Value) -> ObjectSchema {
        ObjectSchema::try_from(json!({
            "type": "object",
            "properties": properties,
            "additionalProperties": false
        }))
        .unwrap_or_else(|_| unreachable!())
    }

    fn descriptor() -> CapabilityDescriptor {
        let key = CapabilityKey::try_from(READ_KEY).unwrap_or_else(|_| unreachable!());
        CapabilityDescriptor::new(
            CapabilityIdentity::new(
                key.clone(),
                CapabilityVersion::try_from(1).unwrap_or_else(|_| unreachable!()),
                key,
                "Read catalog",
                "Read the current code-owned catalog.",
            )
            .unwrap_or_else(|_| unreachable!()),
            CapabilitySchemas::new(
                schema(json!({
                    "query": { "type": "string" },
                    "recordId": { "type": ["string", "null"] }
                })),
                schema(json!({
                    "echoed": { "type": "string" },
                    "tenantId": { "type": "string", "format": "uuid" }
                })),
            ),
            CapabilityPolicy::read_only(DataSensitivity::General),
            CapabilityRedaction::new(
                RedactionProjection::AllowlistedFields,
                RedactionProjection::AllowlistedFields,
                RedactionProjection::SummaryOnly,
                RedactionProjection::SummaryOnly,
                RedactionProjection::Omitted,
            ),
            ["administration.catalog".to_string()],
        )
        .unwrap_or_else(|_| unreachable!())
    }

    fn authority(permissions: &[&str], modules: &[&str]) -> CurrentAuthority {
        let entitlements = EntitlementSnapshot::new(
            LeaseLifecycle::Active,
            modules
                .iter()
                .map(|module| ((*module).to_string(), ModuleEntitlementState::Enabled)),
            Vec::<String>::new(),
        )
        .unwrap_or_else(|_| unreachable!());
        CurrentAuthority::from_reloaded_access(AccessContext {
            role_keys: vec!["test_role".to_string()],
            permissions: permissions
                .iter()
                .map(|permission| (*permission).to_string())
                .collect(),
            enabled_modules: modules.iter().map(|module| (*module).to_string()).collect(),
            entitlements,
        })
    }

    fn fixture(
        authority_state: AuthorityState,
        scope_allowed: bool,
        audit_fails: bool,
        failure: Option<CapabilityExecutionErrorCode>,
    ) -> BrokerFixture {
        let executions = Arc::new(AtomicUsize::new(0));
        let loads = Arc::new(AtomicUsize::new(0));
        let scope_checks = Arc::new(AtomicUsize::new(0));
        let audit_records = Arc::new(Mutex::new(Vec::new()));
        let mut registry = CapabilityRegistry::from_operations([
            operation(READ_KEY, OperationEffect::Read, AgentExposure::Exposed),
            operation(
                "administration.users.create",
                OperationEffect::Write,
                AgentExposure::ApprovalRequired,
            ),
            operation(
                "administration.roles.create",
                OperationEffect::Write,
                AgentExposure::HumanOnly {
                    reason: "Role changes remain human-only.",
                },
            ),
            operation(
                "administration.secrets.reveal",
                OperationEffect::Read,
                AgentExposure::Prohibited {
                    reason: "Secrets are never exposed.",
                },
            ),
        ])
        .unwrap_or_else(|_| unreachable!());
        registry
            .register(ReadCapability {
                descriptor: descriptor(),
                executions: Arc::clone(&executions),
                failure,
            })
            .unwrap_or_else(|_| unreachable!());

        let principal =
            AuthenticatedAgentPrincipal::from_authenticated_request(Uuid::new_v4(), Uuid::new_v4());
        let request_context = RequestContext::from_ids(Uuid::new_v4(), Uuid::new_v4());
        let broker = CapabilityBroker::new(
            registry,
            Arc::new(FakeAuthorityLoader {
                state: authority_state,
                loads: Arc::clone(&loads),
            }),
            Arc::new(FakeScopeAuthorizer {
                allowed: scope_allowed,
                checks: Arc::clone(&scope_checks),
            }),
            Arc::new(FakeAuditSink {
                fail: audit_fails,
                records: Arc::clone(&audit_records),
            }),
        );
        BrokerFixture {
            broker,
            principal,
            request_context,
            executions,
            loads,
            scope_checks,
            audit_records,
        }
    }

    fn call(fixture: &BrokerFixture, key: &str, version: u16, input: Value) -> CapabilityCall {
        CapabilityCall::parse(key, version, input, fixture.request_context)
            .unwrap_or_else(|_| unreachable!())
    }

    fn available_authority() -> AuthorityState {
        AuthorityState::Available(authority(
            &["agent:run", "administration:view"],
            &["agent", "administration"],
        ))
    }

    fn records(fixture: &BrokerFixture) -> Vec<BrokerAuditRecord> {
        fixture
            .audit_records
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    #[tokio::test]
    async fn successful_call_reloads_authority_checks_scope_executes_and_audits() {
        let fixture = fixture(available_authority(), true, false, None);
        assert_eq!(fixture.broker.registry().descriptors().len(), 1);
        let run_id = Uuid::new_v4();
        let capability_call = call(
            &fixture,
            READ_KEY,
            1,
            json!({ "query": "modules", "recordId": "record-1" }),
        )
        .with_agent_run_id(run_id);
        let result = fixture
            .broker
            .invoke(fixture.principal, capability_call)
            .await
            .unwrap_or_else(|_| unreachable!());

        assert_eq!(result.key().as_str(), READ_KEY);
        assert_eq!(result.version().get(), 1);
        assert_eq!(result.content()["echoed"], "modules");
        assert_eq!(
            result.content()["tenantId"],
            fixture.principal.tenant_id().to_string()
        );
        assert_eq!(result.request_context(), fixture.request_context);
        assert_eq!(fixture.loads.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.scope_checks.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.executions.load(Ordering::SeqCst), 1);

        let records = records(&fixture);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].action_key, READ_KEY);
        assert_eq!(records[0].reason, "completed");
        assert_eq!(records[0].agent_run_id, Some(run_id));
        assert_eq!(
            records[0].target.as_ref().map(CapabilityResource::id),
            Some("record-1")
        );
    }

    #[tokio::test]
    async fn operation_exposure_blocks_approval_human_only_and_prohibited_calls() {
        let fixture = fixture(available_authority(), true, false, None);
        for (key, expected) in [
            (
                "administration.users.create",
                BrokerErrorCode::ApprovalRequired,
            ),
            ("administration.roles.create", BrokerErrorCode::HumanOnly),
            ("administration.secrets.reveal", BrokerErrorCode::Prohibited),
        ] {
            let error = fixture
                .broker
                .invoke(fixture.principal, call(&fixture, key, 1, json!({})))
                .await
                .err()
                .unwrap_or_else(|| unreachable!());
            assert_eq!(error.code(), expected);
            assert_eq!(error.request_context(), fixture.request_context);
        }
        assert_eq!(fixture.loads.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.executions.load(Ordering::SeqCst), 0);
        assert_eq!(records(&fixture).len(), 3);
    }

    #[tokio::test]
    async fn unknown_unreleased_and_wrong_version_capabilities_are_distinct() {
        let fixture = fixture(available_authority(), true, false, None);
        let cases = [
            (
                "administration.unknown.read",
                1,
                BrokerErrorCode::UnknownCapability,
            ),
            (
                "administration.users.read",
                1,
                BrokerErrorCode::UnknownCapability,
            ),
            (READ_KEY, 2, BrokerErrorCode::UnsupportedVersion),
        ];
        for (key, version, expected) in cases {
            let error = fixture
                .broker
                .invoke(
                    fixture.principal,
                    call(&fixture, key, version, json!({ "query": "x" })),
                )
                .await
                .err()
                .unwrap_or_else(|| unreachable!());
            assert_eq!(error.code(), expected);
        }

        let unreleased_operation = operation(
            "administration.users.read",
            OperationEffect::Read,
            AgentExposure::Exposed,
        );
        let registry = CapabilityRegistry::from_operations([unreleased_operation])
            .unwrap_or_else(|_| unreachable!());
        let audit_records = Arc::new(Mutex::new(Vec::new()));
        let broker = CapabilityBroker::new(
            registry,
            Arc::new(FakeAuthorityLoader {
                state: available_authority(),
                loads: Arc::new(AtomicUsize::new(0)),
            }),
            Arc::new(FakeScopeAuthorizer {
                allowed: true,
                checks: Arc::new(AtomicUsize::new(0)),
            }),
            Arc::new(FakeAuditSink {
                fail: false,
                records: Arc::clone(&audit_records),
            }),
        );
        let error = broker
            .invoke(
                fixture.principal,
                call(&fixture, "administration.users.read", 1, json!({})),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(error.code(), BrokerErrorCode::CapabilityUnavailable);
    }

    #[tokio::test]
    async fn authority_agent_gate_and_underlying_operation_are_rechecked() {
        let unavailable = fixture(AuthorityState::Unavailable, true, false, None);
        let error = unavailable
            .broker
            .invoke(
                unavailable.principal,
                call(&unavailable, READ_KEY, 1, json!({ "query": "x" })),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(error.code(), BrokerErrorCode::AuthorityUnavailable);

        for current in [
            authority(&["administration:view"], &["agent", "administration"]),
            authority(&["agent:run", "administration:view"], &["administration"]),
            authority(&["agent:run"], &["agent", "administration"]),
        ] {
            let fixture = fixture(AuthorityState::Available(current), true, false, None);
            let error = fixture
                .broker
                .invoke(
                    fixture.principal,
                    call(&fixture, READ_KEY, 1, json!({ "query": "x" })),
                )
                .await
                .err()
                .unwrap_or_else(|| unreachable!());
            assert_eq!(error.code(), BrokerErrorCode::AccessDenied);
            assert_eq!(fixture.executions.load(Ordering::SeqCst), 0);
        }
    }

    #[tokio::test]
    async fn reserved_identity_invalid_input_and_scope_denial_never_execute() {
        let invalid_fixture = fixture(available_authority(), true, false, None);
        for input in [
            json!({ "query": "x", "tenantId": Uuid::new_v4() }),
            json!({ "query": "x", "filters": [{ "user_id": Uuid::new_v4() }] }),
            json!({ "unknown": "field" }),
        ] {
            let error = invalid_fixture
                .broker
                .invoke(
                    invalid_fixture.principal,
                    call(&invalid_fixture, READ_KEY, 1, input),
                )
                .await
                .err()
                .unwrap_or_else(|| unreachable!());
            assert_eq!(error.code(), BrokerErrorCode::InvalidInput);
        }
        assert_eq!(invalid_fixture.executions.load(Ordering::SeqCst), 0);

        let denied = fixture(available_authority(), false, false, None);
        let error = denied
            .broker
            .invoke(
                denied.principal,
                call(
                    &denied,
                    READ_KEY,
                    1,
                    json!({ "query": "x", "recordId": "outside" }),
                ),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(error.code(), BrokerErrorCode::RecordScopeDenied);
        assert_eq!(denied.executions.load(Ordering::SeqCst), 0);
        assert_eq!(
            records(&denied)[0]
                .target
                .as_ref()
                .map(CapabilityResource::id),
            Some("outside")
        );
    }

    #[tokio::test]
    async fn handler_failures_use_stable_safe_errors_and_audit_reasons() {
        for (code, reason) in [
            (
                CapabilityExecutionErrorCode::DependencyUnavailable,
                "handler_dependency_unavailable",
            ),
            (CapabilityExecutionErrorCode::Conflict, "handler_conflict"),
            (
                CapabilityExecutionErrorCode::InvalidState,
                "handler_invalid_state",
            ),
            (CapabilityExecutionErrorCode::Internal, "handler_internal"),
        ] {
            let fixture = fixture(available_authority(), true, false, Some(code));
            let error = fixture
                .broker
                .invoke(
                    fixture.principal,
                    call(&fixture, READ_KEY, 1, json!({ "query": "x" })),
                )
                .await
                .err()
                .unwrap_or_else(|| unreachable!());
            assert_eq!(error.code(), BrokerErrorCode::ExecutionFailed);
            assert_eq!(
                error.safe_message(),
                "The capability could not be completed."
            );
            assert_eq!(records(&fixture)[0].reason, reason);
        }
    }

    #[tokio::test]
    async fn missing_audit_evidence_fails_closed_before_or_after_read_execution() {
        let denied = fixture(available_authority(), true, true, None);
        let error = denied
            .broker
            .invoke(
                denied.principal,
                call(&denied, "administration.roles.create", 1, json!({})),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(error.code(), BrokerErrorCode::AuditUnavailable);
        assert_eq!(denied.executions.load(Ordering::SeqCst), 0);

        let success = fixture(available_authority(), true, true, None);
        let error = success
            .broker
            .invoke(
                success.principal,
                call(&success, READ_KEY, 1, json!({ "query": "x" })),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(error.code(), BrokerErrorCode::AuditUnavailable);
        assert_eq!(success.executions.load(Ordering::SeqCst), 1);
    }
}
