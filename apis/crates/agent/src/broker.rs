//! Enforces the single execution boundary for every Agent capability call.
//!
//! This foundation performs no model/provider work and registers no product
//! capabilities by default. Only typed read-only handlers can execute.

use std::sync::Arc;

use async_trait::async_trait;
use cp_common::{AgentExposure, ProductOperation, RuntimeAccessChecks};
use serde_json::Value;
use thiserror::Error;

use crate::{
    audit::{BrokerAuditError, BrokerAuditOutcome, BrokerAuditRecord, BrokerAuditSink},
    binding::{CapabilityBindingSource, canonical_input, input_binding, normalized_input_digest},
    handler::{ErasedCapability, ErasedCapabilityError, ParsedCapabilityInput},
    registry::CapabilityRegistry,
    types::{
        AuthenticatedAgentPrincipal, AuthorizedCapabilityContext, AuthorizedRecordScope,
        BrokerError, BrokerErrorCode, CapabilityCall, CapabilityCallId, CapabilityExecutionProof,
        CapabilityPreparationRejection, CapabilityPreparationRejectionParts,
        CapabilityRejectionOperationEvidence, CapabilityRejectionOutcome, CapabilityResult,
        CapabilityScope, CurrentAuthority, PreparedCapabilityCallFacts,
        PreparedCapabilityCallFactsParts,
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
    capability_call_id: CapabilityCallId,
    action_key: &'a str,
    outcome: BrokerAuditOutcome,
    reason: &'static str,
    target: Option<crate::types::CapabilityResource>,
}

#[derive(Clone, Copy)]
struct CallEvidence {
    version: crate::descriptor::CapabilityVersion,
    request_context: cp_audit::RequestContext,
    agent_run_id: Option<uuid::Uuid>,
}

impl CallEvidence {
    const fn from_call(call: &CapabilityCall) -> Self {
        Self {
            version: call.version(),
            request_context: call.request_context(),
            agent_run_id: call.agent_run_id(),
        }
    }

    const fn from_facts(facts: &PreparedCapabilityCallFacts) -> Self {
        Self {
            version: facts.version(),
            request_context: facts.request_context(),
            agent_run_id: facts.agent_run_id(),
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("durable capability execution proof was rejected")]
pub struct DurabilityProofRejected;

#[async_trait]
pub trait PreparedCapabilityCallVerifier: Send + Sync {
    /// Atomically verifies and consumes the runtime claim for this execution.
    ///
    /// A production implementation must prove the exact tenant, principal,
    /// call and run IDs, active worker lease/token/fence, persisted call row,
    /// input binding, operation/module/permission facts, and prepared usage
    /// reservation. Returning `Ok(())` more than once for the same persisted
    /// call is a contract violation.
    async fn verify_and_consume(
        &self,
        principal: AuthenticatedAgentPrincipal,
        facts: &PreparedCapabilityCallFacts,
        proof: &CapabilityExecutionProof,
    ) -> Result<(), DurabilityProofRejected>;
}

/// One-shot typed broker work produced by [`CapabilityBroker::prepare`].
///
/// The parsed handler input and handler identity remain opaque. Runtime code
/// can persist [`Self::facts`] before calling `execute_prepared`, while only the
/// broker can enter the typed domain handler.
pub struct PreparedCapabilityCall {
    facts: PreparedCapabilityCallFacts,
    principal: AuthenticatedAgentPrincipal,
    operation: ProductOperation,
    handler: Arc<dyn ErasedCapability>,
    parsed: Option<ParsedCapabilityInput>,
}

impl PreparedCapabilityCall {
    #[must_use]
    pub const fn facts(&self) -> &PreparedCapabilityCallFacts {
        &self.facts
    }
}

pub struct CapabilityBroker {
    registry: Arc<CapabilityRegistry>,
    authority_loader: Arc<dyn AuthorityLoader>,
    scope_authorizer: Arc<dyn RecordScopeAuthorizer>,
    durability_verifier: Arc<dyn PreparedCapabilityCallVerifier>,
    audit_sink: Arc<dyn BrokerAuditSink>,
}

impl CapabilityBroker {
    #[must_use]
    pub fn new(
        registry: CapabilityRegistry,
        authority_loader: Arc<dyn AuthorityLoader>,
        scope_authorizer: Arc<dyn RecordScopeAuthorizer>,
        durability_verifier: Arc<dyn PreparedCapabilityCallVerifier>,
        audit_sink: Arc<dyn BrokerAuditSink>,
    ) -> Self {
        Self {
            registry: Arc::new(registry),
            authority_loader,
            scope_authorizer,
            durability_verifier,
            audit_sink,
        }
    }

    #[must_use]
    pub fn registry(&self) -> &CapabilityRegistry {
        &self.registry
    }

    /// Rechecks current authority and resolves typed input and record scope
    /// without entering a domain handler.
    ///
    /// A durable worker must persist [`CapabilityPreparationRejection`] before
    /// finalizing a rejected call. That error is call-intent evidence, not an
    /// executable prepared call, and must never be passed to
    /// [`Self::execute_prepared`] or paired with an execution proof.
    pub async fn prepare(
        &self,
        principal: AuthenticatedAgentPrincipal,
        capability_call_id: CapabilityCallId,
        call: CapabilityCall,
    ) -> Result<PreparedCapabilityCall, CapabilityPreparationRejection> {
        let canonicalized_input = canonical_input(call.input()).ok();
        let normalized_input_digest_sha256 =
            canonicalized_input.as_deref().map(normalized_input_digest);
        let Some(operation) = self.registry.operation(call.key()) else {
            return Err(self
                .reject(
                    principal,
                    &call,
                    capability_call_id,
                    normalized_input_digest_sha256,
                    None,
                    BrokerFailure::denied(
                        INVOKE_ACTION,
                        BrokerErrorCode::UnknownCapability,
                        "The requested capability does not exist.",
                        "unknown_capability",
                    ),
                )
                .await);
        };

        match operation.agent_exposure() {
            AgentExposure::ApprovalRequired => {
                return Err(self
                    .reject(
                        principal,
                        &call,
                        capability_call_id,
                        normalized_input_digest_sha256,
                        None,
                        BrokerFailure::denied(
                            operation.key(),
                            BrokerErrorCode::ApprovalRequired,
                            "This capability requires an approved proposal.",
                            "approval_required",
                        ),
                    )
                    .await);
            }
            AgentExposure::HumanOnly { .. } => {
                return Err(self
                    .reject(
                        principal,
                        &call,
                        capability_call_id,
                        normalized_input_digest_sha256,
                        None,
                        BrokerFailure::denied(
                            operation.key(),
                            BrokerErrorCode::HumanOnly,
                            "This operation must be completed by a person.",
                            "human_only",
                        ),
                    )
                    .await);
            }
            AgentExposure::Prohibited { .. } => {
                return Err(self
                    .reject(
                        principal,
                        &call,
                        capability_call_id,
                        normalized_input_digest_sha256,
                        None,
                        BrokerFailure::denied(
                            operation.key(),
                            BrokerErrorCode::Prohibited,
                            "This operation is not available to Agent.",
                            "prohibited",
                        ),
                    )
                    .await);
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
            return Err(self
                .reject(
                    principal,
                    &call,
                    capability_call_id,
                    normalized_input_digest_sha256,
                    None,
                    BrokerFailure::denied(operation.key(), code, message, reason),
                )
                .await);
        };

        let authority = match self.authority_loader.load(principal).await {
            Ok(authority) => authority,
            Err(_) => {
                return Err(self
                    .reject(
                        principal,
                        &call,
                        capability_call_id,
                        normalized_input_digest_sha256,
                        None,
                        BrokerFailure::failed(
                            operation.key(),
                            BrokerErrorCode::AuthorityUnavailable,
                            "Current access could not be loaded.",
                            "authority_unavailable",
                        ),
                    )
                    .await);
            }
        };

        let agent_gate = agent_gate(operation);
        let agent_decision = authority
            .access()
            .evaluate_operation(&agent_gate, RuntimeAccessChecks::default());
        if !agent_decision.allowed {
            return Err(self
                .reject(
                    principal,
                    &call,
                    capability_call_id,
                    normalized_input_digest_sha256,
                    None,
                    BrokerFailure::denied(
                        operation.key(),
                        BrokerErrorCode::AccessDenied,
                        "This capability is not available for the current account.",
                        agent_decision.reason.as_str(),
                    ),
                )
                .await);
        }

        let operation_decision = authority
            .access()
            .evaluate_operation(operation, RuntimeAccessChecks::default());
        if !operation_decision.allowed {
            return Err(self
                .reject(
                    principal,
                    &call,
                    capability_call_id,
                    normalized_input_digest_sha256,
                    None,
                    BrokerFailure::denied(
                        operation.key(),
                        BrokerErrorCode::AccessDenied,
                        "This capability is not available for the current account.",
                        operation_decision.reason.as_str(),
                    ),
                )
                .await);
        }

        let Some(canonical_input) = canonicalized_input.as_deref() else {
            return Err(self
                .reject(
                    principal,
                    &call,
                    capability_call_id,
                    None,
                    None,
                    BrokerFailure::denied(
                        operation.key(),
                        BrokerErrorCode::InputTooLarge,
                        "Capability input exceeds the supported size.",
                        "input_too_large",
                    ),
                )
                .await);
        };

        if input_contains_reserved_identity(call.input()) {
            return Err(self
                .reject(
                    principal,
                    &call,
                    capability_call_id,
                    normalized_input_digest_sha256,
                    None,
                    BrokerFailure::denied(
                        operation.key(),
                        BrokerErrorCode::InvalidInput,
                        "Capability input contains a reserved identity field.",
                        "reserved_identity_input",
                    ),
                )
                .await);
        }

        let parsed = match handler.parse_input(call.input().clone()) {
            Ok(input) => input,
            Err(_) => {
                return Err(self
                    .reject(
                        principal,
                        &call,
                        capability_call_id,
                        normalized_input_digest_sha256,
                        None,
                        BrokerFailure::denied(
                            operation.key(),
                            BrokerErrorCode::InvalidInput,
                            "Capability input is invalid.",
                            "invalid_input",
                        ),
                    )
                    .await);
            }
        };
        let scope = match handler.scope(&parsed) {
            Ok(scope) => scope,
            Err(_) => {
                return Err(self
                    .reject(
                        principal,
                        &call,
                        capability_call_id,
                        normalized_input_digest_sha256,
                        None,
                        BrokerFailure::failed(
                            operation.key(),
                            BrokerErrorCode::ExecutionFailed,
                            "The capability could not be prepared.",
                            "handler_contract_failed",
                        ),
                    )
                    .await);
            }
        };
        let target = scope.primary_resource().cloned();
        match self
            .scope_authorizer
            .authorize(principal, &authority, operation, &scope)
            .await
        {
            Ok(_) => {}
            Err(_) => {
                return Err(self
                    .reject(
                        principal,
                        &call,
                        capability_call_id,
                        normalized_input_digest_sha256,
                        Some(&scope),
                        BrokerFailure::denied(
                            operation.key(),
                            BrokerErrorCode::RecordScopeDenied,
                            "The requested records are outside the current access scope.",
                            "record_scope_denied",
                        )
                        .with_target(target),
                    )
                    .await);
            }
        }

        let input_binding_sha256 = input_binding(CapabilityBindingSource {
            principal,
            capability_call_id,
            call: &call,
            operation,
            scope: &scope,
            canonical_input,
        });
        Ok(PreparedCapabilityCall {
            facts: PreparedCapabilityCallFacts::new(PreparedCapabilityCallFactsParts {
                capability_call_id,
                key: call.key().clone(),
                version: call.version(),
                operation_key: operation.key().to_string(),
                module_key: operation.module_key().to_string(),
                required_permission: operation.permission().to_string(),
                input_binding_sha256,
                request_context: call.request_context(),
                agent_run_id: call.agent_run_id(),
                scope,
            }),
            principal,
            operation: operation.clone(),
            handler,
            parsed: Some(parsed),
        })
    }

    /// Executes one prepared call after a second, immediate authority and
    /// record-scope freshness check. The prepared input is consumed before any
    /// handler can run, so a replay fails closed with the same stable call ID.
    pub async fn execute_prepared(
        &self,
        prepared: &mut PreparedCapabilityCall,
        proof: CapabilityExecutionProof,
    ) -> Result<CapabilityResult, BrokerError> {
        let evidence = CallEvidence::from_facts(&prepared.facts);
        let capability_call_id = prepared.facts.capability_call_id();
        let target = prepared.facts.scope().primary_resource().cloned();
        let Some(parsed) = prepared.parsed.take() else {
            return self
                .reject_prepared(
                    prepared.principal,
                    evidence,
                    capability_call_id,
                    BrokerFailure::failed(
                        prepared.operation.key(),
                        BrokerErrorCode::PreparedCallConsumed,
                        "This prepared capability call has already been consumed.",
                        "prepared_call_consumed",
                    )
                    .with_target(target),
                )
                .await;
        };

        let operation = &prepared.operation;

        let authority = match self.authority_loader.load(prepared.principal).await {
            Ok(authority) => authority,
            Err(_) => {
                return self
                    .reject_prepared(
                        prepared.principal,
                        evidence,
                        capability_call_id,
                        BrokerFailure::failed(
                            operation.key(),
                            BrokerErrorCode::AuthorityUnavailable,
                            "Current access could not be loaded.",
                            "authority_unavailable",
                        )
                        .with_target(target),
                    )
                    .await;
            }
        };

        let agent_decision = authority
            .access()
            .evaluate_operation(&agent_gate(operation), RuntimeAccessChecks::default());
        if !agent_decision.allowed {
            return self
                .reject_prepared(
                    prepared.principal,
                    evidence,
                    capability_call_id,
                    BrokerFailure::denied(
                        operation.key(),
                        BrokerErrorCode::AccessDenied,
                        "This capability is not available for the current account.",
                        agent_decision.reason.as_str(),
                    )
                    .with_target(target),
                )
                .await;
        }

        let operation_decision = authority
            .access()
            .evaluate_operation(operation, RuntimeAccessChecks::default());
        if !operation_decision.allowed {
            return self
                .reject_prepared(
                    prepared.principal,
                    evidence,
                    capability_call_id,
                    BrokerFailure::denied(
                        operation.key(),
                        BrokerErrorCode::AccessDenied,
                        "This capability is not available for the current account.",
                        operation_decision.reason.as_str(),
                    )
                    .with_target(target),
                )
                .await;
        }

        let scope_grant = match self
            .scope_authorizer
            .authorize(
                prepared.principal,
                &authority,
                operation,
                prepared.facts.scope(),
            )
            .await
        {
            Ok(grant) => grant,
            Err(_) => {
                return self
                    .reject_prepared(
                        prepared.principal,
                        evidence,
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

        if !proof_matches_prepared(prepared, &proof)
            || self
                .durability_verifier
                .verify_and_consume(prepared.principal, &prepared.facts, &proof)
                .await
                .is_err()
        {
            return self
                .reject_prepared(
                    prepared.principal,
                    evidence,
                    capability_call_id,
                    BrokerFailure::failed(
                        operation.key(),
                        BrokerErrorCode::DurabilityProofRejected,
                        "Durable capability execution proof could not be verified.",
                        "durability_proof_rejected",
                    )
                    .with_target(target),
                )
                .await;
        }

        let context = AuthorizedCapabilityContext::new(
            prepared.principal,
            prepared.facts.request_context(),
            prepared.facts.scope().clone(),
            scope_grant,
        );
        let content = match prepared.handler.execute(context, parsed).await {
            Ok(content) => content,
            Err(ErasedCapabilityError::Execution(error)) => {
                return self
                    .reject_prepared(
                        prepared.principal,
                        evidence,
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
                    .reject_prepared(
                        prepared.principal,
                        evidence,
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

        self.audit_evidence(
            prepared.principal,
            evidence,
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
                prepared.facts.request_context(),
            )
        })?;

        Ok(CapabilityResult::new(
            prepared.facts.key().clone(),
            prepared.facts.version(),
            content,
            prepared.facts.request_context(),
        ))
    }

    async fn reject(
        &self,
        principal: AuthenticatedAgentPrincipal,
        call: &CapabilityCall,
        capability_call_id: CapabilityCallId,
        normalized_input_digest_sha256: Option<[u8; 32]>,
        scope_evidence: Option<&CapabilityScope>,
        failure: BrokerFailure<'_>,
    ) -> CapabilityPreparationRejection {
        let evidence = CallEvidence::from_call(call);
        let audit_result = self
            .audit_evidence(
                principal,
                evidence,
                AuditDecision {
                    capability_call_id,
                    action_key: failure.action_key,
                    outcome: failure.outcome,
                    reason: failure.reason,
                    target: failure.target.clone(),
                },
            )
            .await;
        let (outcome, code, reason_code, safe_message) = if audit_result.is_err() {
            (
                CapabilityRejectionOutcome::Failed,
                BrokerErrorCode::AuditUnavailable,
                "audit_unavailable",
                "Capability audit evidence could not be recorded.",
            )
        } else {
            (
                rejection_outcome(failure.outcome),
                failure.code,
                failure.reason,
                failure.message,
            )
        };
        CapabilityPreparationRejection::new(CapabilityPreparationRejectionParts {
            principal,
            capability_call_id,
            key: call.key().clone(),
            version: call.version(),
            request_context: evidence.request_context,
            agent_run_id: evidence.agent_run_id,
            normalized_input_digest_sha256,
            operation_evidence: self.registry.operation(call.key()).map(|operation| {
                CapabilityRejectionOperationEvidence::new(
                    operation.key(),
                    operation.module_key(),
                    operation.permission(),
                )
            }),
            scope_evidence: scope_evidence.cloned(),
            outcome,
            code,
            reason_code,
            safe_message,
        })
    }

    async fn reject_prepared<T>(
        &self,
        principal: AuthenticatedAgentPrincipal,
        evidence: CallEvidence,
        capability_call_id: CapabilityCallId,
        failure: BrokerFailure<'_>,
    ) -> Result<T, BrokerError> {
        self.audit_evidence(
            principal,
            evidence,
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
                evidence.request_context,
            )
        })?;
        Err(BrokerError::new(
            failure.code,
            failure.message,
            evidence.request_context,
        ))
    }

    async fn audit_evidence(
        &self,
        principal: AuthenticatedAgentPrincipal,
        evidence: CallEvidence,
        decision: AuditDecision<'_>,
    ) -> Result<(), BrokerAuditError> {
        self.audit_sink
            .record(BrokerAuditRecord {
                principal,
                request_context: evidence.request_context,
                capability_call_id: decision.capability_call_id,
                action_key: decision.action_key.to_string(),
                capability_version: evidence.version.get(),
                agent_run_id: evidence.agent_run_id,
                target: decision.target,
                outcome: decision.outcome,
                reason: decision.reason,
            })
            .await
    }
}

fn proof_matches_prepared(
    prepared: &PreparedCapabilityCall,
    proof: &CapabilityExecutionProof,
) -> bool {
    prepared.facts.agent_run_id().is_some_and(|run_id| {
        proof.tenant_id() == prepared.principal.tenant_id()
            && proof.user_id() == prepared.principal.user_id()
            && proof.capability_call_id() == prepared.facts.capability_call_id()
            && proof.run_id() == run_id
    })
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

const fn rejection_outcome(outcome: BrokerAuditOutcome) -> CapabilityRejectionOutcome {
    match outcome {
        BrokerAuditOutcome::Denied => CapabilityRejectionOutcome::Denied,
        BrokerAuditOutcome::Failed | BrokerAuditOutcome::Succeeded => {
            CapabilityRejectionOutcome::Failed
        }
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
    use std::{
        collections::BTreeSet,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
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
        audit::{BrokerAuditError, BrokerAuditOutcome, BrokerAuditRecord, BrokerAuditSink},
        descriptor::{
            CapabilityDescriptor, CapabilityIdentity, CapabilityKey, CapabilityPolicy,
            CapabilityRedaction, CapabilitySchemas, CapabilityVersion, DataSensitivity,
            ObjectSchema, RedactionProjection,
        },
        handler::Capability,
        registry::CapabilityRegistry,
        types::{
            AuthenticatedAgentPrincipal, AuthorizedCapabilityContext, AuthorizedRecordScope,
            BrokerError, BrokerErrorCode, CapabilityCall, CapabilityCallId,
            CapabilityExecutionError, CapabilityExecutionErrorCode, CapabilityExecutionProof,
            CapabilityRejectionOutcome, CapabilityResource, CapabilityResult, CapabilityScope,
            CapabilityWorkerLease, CurrentAuthority, PreparedCapabilityCallFacts,
        },
    };

    use super::{
        AuthorityLoadError, AuthorityLoader, CapabilityBroker, DurabilityProofRejected,
        PreparedCapabilityCallVerifier, RecordScopeAuthorizer, RecordScopeDenied,
    };

    const READ_KEY: &str = "administration.catalog.read";

    fn test_run_id() -> Uuid {
        Uuid::from_u128(0x100)
    }

    fn test_lease_token() -> Uuid {
        Uuid::from_u128(0x200)
    }

    fn test_reservation_id() -> Uuid {
        Uuid::from_u128(0x300)
    }

    fn execution_proof(
        principal: AuthenticatedAgentPrincipal,
        capability_call_id: CapabilityCallId,
        run_id: Uuid,
    ) -> CapabilityExecutionProof {
        CapabilityExecutionProof::parse(
            principal,
            capability_call_id,
            run_id,
            CapabilityWorkerLease::parse("test-worker", test_lease_token(), 7)
                .unwrap_or_else(|_| unreachable!()),
            test_reservation_id(),
        )
        .unwrap_or_else(|_| unreachable!())
    }

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
        scope_resolutions: Arc<AtomicUsize>,
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
            self.scope_resolutions.fetch_add(1, Ordering::SeqCst);
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
        AvailableThenUnavailable(CurrentAuthority),
        Sequence(Arc<Vec<CurrentAuthority>>),
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
            let load_index = self.loads.fetch_add(1, Ordering::SeqCst);
            match &self.state {
                AuthorityState::Available(authority) => Ok(authority.clone()),
                AuthorityState::AvailableThenUnavailable(authority) if load_index == 0 => {
                    Ok(authority.clone())
                }
                AuthorityState::AvailableThenUnavailable(_) => Err(AuthorityLoadError),
                AuthorityState::Sequence(authorities) => authorities
                    .get(load_index)
                    .or_else(|| authorities.last())
                    .cloned()
                    .ok_or(AuthorityLoadError),
                AuthorityState::Unavailable => Err(AuthorityLoadError),
            }
        }
    }

    struct FakeScopeAuthorizer {
        allowed: Arc<AtomicBool>,
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
            if self.allowed.load(Ordering::SeqCst) {
                Ok(AuthorizedRecordScope::granted())
            } else {
                Err(RecordScopeDenied)
            }
        }
    }

    struct FakeDurabilityVerifier {
        consumed: Arc<Mutex<BTreeSet<CapabilityCallId>>>,
        checks: Arc<AtomicUsize>,
        persisted_binding: Arc<Mutex<Option<[u8; 32]>>>,
    }

    #[async_trait]
    impl PreparedCapabilityCallVerifier for FakeDurabilityVerifier {
        async fn verify_and_consume(
            &self,
            principal: AuthenticatedAgentPrincipal,
            facts: &PreparedCapabilityCallFacts,
            proof: &CapabilityExecutionProof,
        ) -> Result<(), DurabilityProofRejected> {
            self.checks.fetch_add(1, Ordering::SeqCst);
            let exact = facts.agent_run_id().is_some_and(|run_id| {
                proof.tenant_id() == principal.tenant_id()
                    && proof.user_id() == principal.user_id()
                    && proof.capability_call_id() == facts.capability_call_id()
                    && proof.run_id() == run_id
                    && proof.worker_id() == "test-worker"
                    && proof.lease_token() == test_lease_token()
                    && proof.fence_version() == 7
                    && proof.usage_reservation_id() == test_reservation_id()
                    && facts.operation_key() == READ_KEY
                    && facts.module_key() == "administration"
                    && facts.required_permission() == "administration:view"
                    && facts.input_binding_sha256() != [0; 32]
            });
            let persisted_binding = *self
                .persisted_binding
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !exact
                || persisted_binding.is_some_and(|binding| binding != facts.input_binding_sha256())
            {
                return Err(DurabilityProofRejected);
            }
            let mut consumed = self
                .consumed
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !consumed.insert(facts.capability_call_id()) {
                return Err(DurabilityProofRejected);
            }
            Ok(())
        }
    }

    #[async_trait]
    trait TestInvoke {
        async fn invoke(
            &self,
            principal: AuthenticatedAgentPrincipal,
            capability_call_id: CapabilityCallId,
            call: CapabilityCall,
        ) -> Result<CapabilityResult, BrokerError>;
    }

    #[async_trait]
    impl TestInvoke for CapabilityBroker {
        async fn invoke(
            &self,
            principal: AuthenticatedAgentPrincipal,
            capability_call_id: CapabilityCallId,
            call: CapabilityCall,
        ) -> Result<CapabilityResult, BrokerError> {
            let run_id = call.agent_run_id().unwrap_or_else(test_run_id);
            let mut prepared = self.prepare(principal, capability_call_id, call).await?;
            self.execute_prepared(
                &mut prepared,
                execution_proof(principal, capability_call_id, run_id),
            )
            .await
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
        scope_resolutions: Arc<AtomicUsize>,
        scope_allowed: Arc<AtomicBool>,
        persisted_binding: Arc<Mutex<Option<[u8; 32]>>>,
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
            CapabilityPolicy::read_only(
                DataSensitivity::General,
                crate::ProviderDataClass::CampusApproved,
            ),
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
        let scope_resolutions = Arc::new(AtomicUsize::new(0));
        let scope_allowed_state = Arc::new(AtomicBool::new(scope_allowed));
        let persisted_binding = Arc::new(Mutex::new(None));
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
                scope_resolutions: Arc::clone(&scope_resolutions),
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
                allowed: Arc::clone(&scope_allowed_state),
                checks: Arc::clone(&scope_checks),
            }),
            Arc::new(FakeDurabilityVerifier {
                consumed: Arc::new(Mutex::new(BTreeSet::new())),
                checks: Arc::new(AtomicUsize::new(0)),
                persisted_binding: Arc::clone(&persisted_binding),
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
            scope_resolutions,
            scope_allowed: scope_allowed_state,
            persisted_binding,
            audit_records,
        }
    }

    fn call(fixture: &BrokerFixture, key: &str, version: u16, input: Value) -> CapabilityCall {
        CapabilityCall::parse(key, version, input, fixture.request_context)
            .unwrap_or_else(|_| unreachable!())
            .with_agent_run_id(test_run_id())
    }

    fn call_id() -> CapabilityCallId {
        CapabilityCallId::from_trusted_runtime(Uuid::new_v4())
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
        let capability_call_id = call_id();
        let result = fixture
            .broker
            .invoke(fixture.principal, capability_call_id, capability_call)
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
        assert_eq!(fixture.loads.load(Ordering::SeqCst), 2);
        assert_eq!(fixture.scope_checks.load(Ordering::SeqCst), 2);
        assert_eq!(fixture.scope_resolutions.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.executions.load(Ordering::SeqCst), 1);

        let records = records(&fixture);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].action_key, READ_KEY);
        assert_eq!(records[0].reason, "completed");
        assert_eq!(records[0].agent_run_id, Some(run_id));
        assert_eq!(records[0].capability_call_id, capability_call_id);
        assert_eq!(
            records[0].target.as_ref().map(CapabilityResource::id),
            Some("record-1")
        );
    }

    #[tokio::test]
    async fn prepared_call_keeps_stable_id_resolves_scope_once_and_rejects_replay() {
        let fixture = fixture(available_authority(), true, false, None);
        let stable_uuid = Uuid::new_v4();
        let capability_call_id = CapabilityCallId::from_trusted_runtime(stable_uuid);
        let mut prepared = fixture
            .broker
            .prepare(
                fixture.principal,
                capability_call_id,
                call(
                    &fixture,
                    READ_KEY,
                    1,
                    json!({ "query": "modules", "recordId": "record-1" }),
                ),
            )
            .await
            .unwrap_or_else(|_| unreachable!());

        assert_eq!(prepared.facts().capability_call_id().as_uuid(), stable_uuid);
        assert_eq!(prepared.facts().key().as_str(), READ_KEY);
        assert_eq!(prepared.facts().version().get(), 1);
        assert_eq!(prepared.facts().operation_key(), READ_KEY);
        assert_eq!(prepared.facts().module_key(), "administration");
        assert_eq!(
            prepared.facts().required_permission(),
            "administration:view"
        );
        assert_ne!(prepared.facts().input_binding_sha256(), [0; 32]);
        assert_eq!(
            prepared
                .facts()
                .scope()
                .primary_resource()
                .map(CapabilityResource::id),
            Some("record-1")
        );
        assert_eq!(fixture.scope_resolutions.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.executions.load(Ordering::SeqCst), 0);

        let result = fixture
            .broker
            .execute_prepared(
                &mut prepared,
                execution_proof(fixture.principal, capability_call_id, test_run_id()),
            )
            .await;
        assert!(result.is_ok());
        let replay = fixture
            .broker
            .execute_prepared(
                &mut prepared,
                execution_proof(fixture.principal, capability_call_id, test_run_id()),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());

        assert_eq!(replay.code(), BrokerErrorCode::PreparedCallConsumed);
        assert_eq!(fixture.scope_resolutions.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.executions.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.loads.load(Ordering::SeqCst), 2);
        assert_eq!(fixture.scope_checks.load(Ordering::SeqCst), 2);
        let records = records(&fixture);
        assert_eq!(records.len(), 2);
        assert!(
            records
                .iter()
                .all(|record| record.capability_call_id == capability_call_id)
        );
        assert_eq!(records[0].outcome, BrokerAuditOutcome::Succeeded);
        assert_eq!(records[1].outcome, BrokerAuditOutcome::Failed);
        assert_eq!(records[1].reason, "prepared_call_consumed");
    }

    #[tokio::test]
    async fn durability_proof_replay_cannot_enter_handler_twice() {
        let fixture = fixture(available_authority(), true, false, None);
        let capability_call_id = call_id();
        let mut first = fixture
            .broker
            .prepare(
                fixture.principal,
                capability_call_id,
                call(&fixture, READ_KEY, 1, json!({ "query": "modules" })),
            )
            .await
            .unwrap_or_else(|_| unreachable!());
        let mut replay = fixture
            .broker
            .prepare(
                fixture.principal,
                capability_call_id,
                call(&fixture, READ_KEY, 1, json!({ "query": "modules" })),
            )
            .await
            .unwrap_or_else(|_| unreachable!());

        let first_result = fixture
            .broker
            .execute_prepared(
                &mut first,
                execution_proof(fixture.principal, capability_call_id, test_run_id()),
            )
            .await;
        let replay_error = fixture
            .broker
            .execute_prepared(
                &mut replay,
                execution_proof(fixture.principal, capability_call_id, test_run_id()),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());

        assert!(first_result.is_ok());
        assert_eq!(
            replay_error.code(),
            BrokerErrorCode::DurabilityProofRejected
        );
        assert_eq!(fixture.executions.load(Ordering::SeqCst), 1);
        let records = records(&fixture);
        assert_eq!(records.len(), 2);
        assert!(
            records
                .iter()
                .all(|record| record.capability_call_id == capability_call_id)
        );
        assert_eq!(records[0].outcome, BrokerAuditOutcome::Succeeded);
        assert_eq!(records[1].outcome, BrokerAuditOutcome::Failed);
        assert_eq!(records[1].reason, "durability_proof_rejected");
    }

    #[tokio::test]
    async fn persisted_input_binding_mismatch_fails_before_handler_entry() {
        let fixture = fixture(available_authority(), true, false, None);
        let capability_call_id = call_id();
        let mut prepared = fixture
            .broker
            .prepare(
                fixture.principal,
                capability_call_id,
                call(&fixture, READ_KEY, 1, json!({ "query": "modules" })),
            )
            .await
            .unwrap_or_else(|_| unreachable!());
        let actual_binding = prepared.facts().input_binding_sha256();
        let mut mismatched_binding = actual_binding;
        mismatched_binding[0] ^= 0xff;
        *fixture
            .persisted_binding
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(mismatched_binding);

        let error = fixture
            .broker
            .execute_prepared(
                &mut prepared,
                execution_proof(fixture.principal, capability_call_id, test_run_id()),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());

        assert_eq!(error.code(), BrokerErrorCode::DurabilityProofRejected);
        assert_eq!(fixture.executions.load(Ordering::SeqCst), 0);
        let records = records(&fixture);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].capability_call_id, capability_call_id);
        assert_eq!(records[0].reason, "durability_proof_rejected");
    }

    #[tokio::test]
    async fn mismatched_durability_fields_fail_before_handler_with_prepared_audit_id() {
        for mismatch in 0_u8..8 {
            let fixture = fixture(available_authority(), true, false, None);
            let capability_call_id = call_id();
            let mut prepared = fixture
                .broker
                .prepare(
                    fixture.principal,
                    capability_call_id,
                    call(&fixture, READ_KEY, 1, json!({ "query": "modules" })),
                )
                .await
                .unwrap_or_else(|_| unreachable!());
            let tenant_id = if mismatch == 0 {
                Uuid::new_v4()
            } else {
                fixture.principal.tenant_id()
            };
            let user_id = if mismatch == 1 {
                Uuid::new_v4()
            } else {
                fixture.principal.user_id()
            };
            let proof_call_id = if mismatch == 2 {
                call_id()
            } else {
                capability_call_id
            };
            let run_id = if mismatch == 3 {
                Uuid::new_v4()
            } else {
                test_run_id()
            };
            let worker_id = if mismatch == 4 {
                "other-worker"
            } else {
                "test-worker"
            };
            let lease_token = if mismatch == 5 {
                Uuid::new_v4()
            } else {
                test_lease_token()
            };
            let fence_version = if mismatch == 6 { 8 } else { 7 };
            let reservation_id = if mismatch == 7 {
                Uuid::new_v4()
            } else {
                test_reservation_id()
            };
            let proof = CapabilityExecutionProof::parse(
                AuthenticatedAgentPrincipal::from_authenticated_request(tenant_id, user_id),
                proof_call_id,
                run_id,
                CapabilityWorkerLease::parse(worker_id, lease_token, fence_version)
                    .unwrap_or_else(|_| unreachable!()),
                reservation_id,
            )
            .unwrap_or_else(|_| unreachable!());

            let error = fixture
                .broker
                .execute_prepared(&mut prepared, proof)
                .await
                .err()
                .unwrap_or_else(|| unreachable!());

            assert_eq!(
                error.code(),
                BrokerErrorCode::DurabilityProofRejected,
                "mismatch case {mismatch}"
            );
            assert_eq!(fixture.executions.load(Ordering::SeqCst), 0);
            let records = records(&fixture);
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].capability_call_id, capability_call_id);
            assert_eq!(records[0].outcome, BrokerAuditOutcome::Failed);
            assert_eq!(records[0].reason, "durability_proof_rejected");
        }
    }

    #[tokio::test]
    async fn rejected_preflight_keeps_stable_id_without_fabricating_execution() {
        let fixture = fixture(available_authority(), true, false, None);
        let capability_call_id = call_id();
        let error = fixture
            .broker
            .prepare(
                fixture.principal,
                capability_call_id,
                call(
                    &fixture,
                    READ_KEY,
                    1,
                    json!({ "query": "modules", "unknown": true }),
                ),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());

        assert_eq!(error.code(), BrokerErrorCode::InvalidInput);
        assert_eq!(fixture.executions.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.scope_resolutions.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.scope_checks.load(Ordering::SeqCst), 0);
        let records = records(&fixture);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].capability_call_id, capability_call_id);
        assert_eq!(records[0].outcome, BrokerAuditOutcome::Denied);
        assert_eq!(records[0].reason, "invalid_input");
    }

    #[tokio::test]
    async fn oversized_canonical_input_is_rejected_before_parse_or_scope_resolution() {
        let fixture = fixture(available_authority(), true, false, None);
        let capability_call_id = call_id();
        let error = fixture
            .broker
            .prepare(
                fixture.principal,
                capability_call_id,
                call(
                    &fixture,
                    READ_KEY,
                    1,
                    json!({ "query": "x".repeat(65_536) }),
                ),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());

        assert_eq!(error.code(), BrokerErrorCode::InputTooLarge);
        assert_eq!(error.code().as_str(), "input_too_large");
        assert_eq!(error.reason_code(), "input_too_large");
        assert_eq!(error.outcome(), CapabilityRejectionOutcome::Denied);
        assert!(error.normalized_input_digest_sha256().is_none());
        assert!(error.scope_evidence().is_none());
        assert_eq!(
            error.safe_message(),
            "Capability input exceeds the supported size."
        );
        assert_eq!(fixture.scope_resolutions.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.scope_checks.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.executions.load(Ordering::SeqCst), 0);
        let records = records(&fixture);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].reason, "input_too_large");
    }

    #[tokio::test]
    async fn execution_freshness_denial_happens_before_handler_entry() {
        let allowed = authority(
            &["agent:run", "administration:view"],
            &["agent", "administration"],
        );
        let revoked = authority(&["agent:run"], &["agent", "administration"]);
        let fixture = fixture(
            AuthorityState::Sequence(Arc::new(vec![allowed, revoked])),
            true,
            false,
            None,
        );
        let capability_call_id = call_id();
        let mut prepared = fixture
            .broker
            .prepare(
                fixture.principal,
                capability_call_id,
                call(&fixture, READ_KEY, 1, json!({ "query": "modules" })),
            )
            .await
            .unwrap_or_else(|_| unreachable!());

        let error = fixture
            .broker
            .execute_prepared(
                &mut prepared,
                execution_proof(fixture.principal, capability_call_id, test_run_id()),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());

        assert_eq!(error.code(), BrokerErrorCode::AccessDenied);
        assert_eq!(fixture.loads.load(Ordering::SeqCst), 2);
        assert_eq!(fixture.scope_resolutions.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.scope_checks.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.executions.load(Ordering::SeqCst), 0);
        let records = records(&fixture);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].capability_call_id, capability_call_id);
        assert_eq!(records[0].outcome, BrokerAuditOutcome::Denied);
    }

    #[tokio::test]
    async fn execution_rechecks_agent_gate_and_authority_availability() {
        let agent_revoked = fixture(
            AuthorityState::Sequence(Arc::new(vec![
                authority(
                    &["agent:run", "administration:view"],
                    &["agent", "administration"],
                ),
                authority(&["administration:view"], &["agent", "administration"]),
            ])),
            true,
            false,
            None,
        );
        let capability_call_id = call_id();
        let mut prepared = agent_revoked
            .broker
            .prepare(
                agent_revoked.principal,
                capability_call_id,
                call(&agent_revoked, READ_KEY, 1, json!({ "query": "modules" })),
            )
            .await
            .unwrap_or_else(|_| unreachable!());
        let denied = agent_revoked
            .broker
            .execute_prepared(
                &mut prepared,
                execution_proof(agent_revoked.principal, capability_call_id, test_run_id()),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(denied.code(), BrokerErrorCode::AccessDenied);
        assert_eq!(agent_revoked.executions.load(Ordering::SeqCst), 0);

        let unavailable = fixture(
            AuthorityState::AvailableThenUnavailable(authority(
                &["agent:run", "administration:view"],
                &["agent", "administration"],
            )),
            true,
            false,
            None,
        );
        let capability_call_id = call_id();
        let mut prepared = unavailable
            .broker
            .prepare(
                unavailable.principal,
                capability_call_id,
                call(&unavailable, READ_KEY, 1, json!({ "query": "modules" })),
            )
            .await
            .unwrap_or_else(|_| unreachable!());
        let failed = unavailable
            .broker
            .execute_prepared(
                &mut prepared,
                execution_proof(unavailable.principal, capability_call_id, test_run_id()),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(failed.code(), BrokerErrorCode::AuthorityUnavailable);
        assert_eq!(unavailable.executions.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn execution_rechecks_record_scope_without_resolving_scope_again() {
        let fixture = fixture(available_authority(), true, false, None);
        let capability_call_id = call_id();
        let mut prepared = fixture
            .broker
            .prepare(
                fixture.principal,
                capability_call_id,
                call(
                    &fixture,
                    READ_KEY,
                    1,
                    json!({ "query": "modules", "recordId": "record-1" }),
                ),
            )
            .await
            .unwrap_or_else(|_| unreachable!());
        fixture.scope_allowed.store(false, Ordering::SeqCst);

        let denied = fixture
            .broker
            .execute_prepared(
                &mut prepared,
                execution_proof(fixture.principal, capability_call_id, test_run_id()),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());

        assert_eq!(denied.code(), BrokerErrorCode::RecordScopeDenied);
        assert_eq!(fixture.scope_resolutions.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.scope_checks.load(Ordering::SeqCst), 2);
        assert_eq!(fixture.executions.load(Ordering::SeqCst), 0);
        let records = records(&fixture);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].capability_call_id, capability_call_id);
        assert_eq!(records[0].outcome, BrokerAuditOutcome::Denied);
        assert_eq!(records[0].reason, "record_scope_denied");
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
                .invoke(
                    fixture.principal,
                    call_id(),
                    call(&fixture, key, 1, json!({})),
                )
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
        let unknown_rejection = fixture
            .broker
            .prepare(
                fixture.principal,
                call_id(),
                call(
                    &fixture,
                    "administration.unknown.read",
                    1,
                    json!({ "query": "x" }),
                ),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());
        assert!(unknown_rejection.operation_evidence().is_none());
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
                    call_id(),
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
                allowed: Arc::new(AtomicBool::new(true)),
                checks: Arc::new(AtomicUsize::new(0)),
            }),
            Arc::new(FakeDurabilityVerifier {
                consumed: Arc::new(Mutex::new(BTreeSet::new())),
                checks: Arc::new(AtomicUsize::new(0)),
                persisted_binding: Arc::new(Mutex::new(None)),
            }),
            Arc::new(FakeAuditSink {
                fail: false,
                records: Arc::clone(&audit_records),
            }),
        );
        let error = broker
            .prepare(
                fixture.principal,
                call_id(),
                call(&fixture, "administration.users.read", 1, json!({})),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(error.code(), BrokerErrorCode::CapabilityUnavailable);
        let operation_evidence = error.operation_evidence().unwrap_or_else(|| unreachable!());
        assert_eq!(
            operation_evidence.operation_key(),
            "administration.users.read"
        );
        assert_eq!(operation_evidence.module_key(), "administration");
        assert_eq!(
            operation_evidence.required_permission(),
            "administration:view"
        );
        assert_eq!(
            format!("{operation_evidence:?}"),
            "CapabilityRejectionOperationEvidence([redacted])"
        );
    }

    #[tokio::test]
    async fn authority_agent_gate_and_underlying_operation_are_rechecked() {
        let unavailable = fixture(AuthorityState::Unavailable, true, false, None);
        let error = unavailable
            .broker
            .invoke(
                unavailable.principal,
                call_id(),
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
                    call_id(),
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
    async fn revoked_authority_rejection_binds_call_intent_without_fabricating_scope() {
        let revoked_entitlements = EntitlementSnapshot::new(
            LeaseLifecycle::Revoked,
            [
                ("agent".to_string(), ModuleEntitlementState::Enabled),
                (
                    "administration".to_string(),
                    ModuleEntitlementState::Enabled,
                ),
            ],
            Vec::<String>::new(),
        )
        .unwrap_or_else(|_| unreachable!());
        let revoked = CurrentAuthority::from_reloaded_access(AccessContext {
            role_keys: vec!["test_role".to_string()],
            permissions: vec!["agent:run".to_string(), "administration:view".to_string()],
            enabled_modules: vec!["agent".to_string(), "administration".to_string()],
            entitlements: revoked_entitlements,
        });
        let fixture = fixture(AuthorityState::Available(revoked), true, false, None);
        let capability_call_id = call_id();
        let rejection = fixture
            .broker
            .prepare(
                fixture.principal,
                capability_call_id,
                call(
                    &fixture,
                    READ_KEY,
                    1,
                    json!({ "query": "private-query", "recordId": "sensitive-record-id" }),
                ),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());

        assert_eq!(rejection.principal(), fixture.principal);
        assert_eq!(rejection.capability_call_id(), capability_call_id);
        assert_eq!(rejection.key().as_str(), READ_KEY);
        assert_eq!(rejection.version().get(), 1);
        assert_eq!(rejection.request_context(), fixture.request_context);
        assert_eq!(rejection.agent_run_id(), Some(test_run_id()));
        assert!(rejection.normalized_input_digest_sha256().is_some());
        let operation_evidence = rejection
            .operation_evidence()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(operation_evidence.operation_key(), READ_KEY);
        assert_eq!(operation_evidence.module_key(), "administration");
        assert_eq!(
            operation_evidence.required_permission(),
            "administration:view"
        );
        assert!(rejection.scope_evidence().is_none());
        assert_eq!(rejection.outcome(), CapabilityRejectionOutcome::Denied);
        assert_eq!(rejection.outcome().as_str(), "denied");
        assert_eq!(rejection.code(), BrokerErrorCode::AccessDenied);
        assert_eq!(rejection.reason_code(), "license_revoked");
        assert_eq!(fixture.executions.load(Ordering::SeqCst), 0);

        let debug = format!("{rejection:?}");
        let digest_debug = format!(
            "{:?}",
            rejection
                .normalized_input_digest_sha256()
                .unwrap_or_else(|| unreachable!())
        );
        assert!(debug.contains("has_normalized_input_digest: true"));
        for sensitive in [
            "private-query".to_string(),
            "sensitive-record-id".to_string(),
            fixture.principal.tenant_id().to_string(),
            fixture.principal.user_id().to_string(),
            capability_call_id.as_uuid().to_string(),
            fixture.request_context.request_id().to_string(),
            fixture.request_context.correlation_id().to_string(),
            test_run_id().to_string(),
            digest_debug,
        ] {
            assert!(!debug.contains(&sensitive));
        }
    }

    #[tokio::test]
    async fn invalid_input_and_scope_denial_return_distinct_reduced_evidence() {
        let invalid = fixture(available_authority(), true, false, None);
        let invalid_rejection = invalid
            .broker
            .prepare(
                invalid.principal,
                call_id(),
                call(
                    &invalid,
                    READ_KEY,
                    1,
                    json!({ "query": "raw-model-secret", "unknown": true }),
                ),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(invalid_rejection.code(), BrokerErrorCode::InvalidInput);
        assert_eq!(invalid_rejection.reason_code(), "invalid_input");
        assert_eq!(
            invalid_rejection.to_string(),
            "Capability input is invalid."
        );
        assert!(invalid_rejection.normalized_input_digest_sha256().is_some());
        assert!(invalid_rejection.scope_evidence().is_none());
        assert!(!format!("{invalid_rejection:?}").contains("raw-model-secret"));
        let compatibility_error: BrokerError = invalid_rejection.into();
        assert_eq!(compatibility_error.code(), BrokerErrorCode::InvalidInput);

        let denied = fixture(available_authority(), false, false, None);
        let denied_rejection = denied
            .broker
            .prepare(
                denied.principal,
                call_id(),
                call(
                    &denied,
                    READ_KEY,
                    1,
                    json!({ "query": "x", "recordId": "outside-sensitive-scope" }),
                ),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(denied_rejection.code(), BrokerErrorCode::RecordScopeDenied);
        assert_eq!(denied_rejection.reason_code(), "record_scope_denied");
        assert_eq!(
            denied_rejection.outcome(),
            CapabilityRejectionOutcome::Denied
        );
        assert_eq!(
            denied_rejection
                .scope_evidence()
                .and_then(CapabilityScope::primary_resource)
                .map(CapabilityResource::id),
            Some("outside-sensitive-scope")
        );
        let denied_debug = format!("{denied_rejection:?}");
        assert!(denied_debug.contains("scope_kind: \"resources\""));
        assert!(!denied_debug.contains("outside-sensitive-scope"));
        assert_eq!(denied.executions.load(Ordering::SeqCst), 0);
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
                    call_id(),
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
                call_id(),
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
                    call_id(),
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
            .prepare(
                denied.principal,
                call_id(),
                call(&denied, "administration.roles.create", 1, json!({})),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(error.code(), BrokerErrorCode::AuditUnavailable);
        assert_eq!(error.reason_code(), "audit_unavailable");
        assert_eq!(error.outcome(), CapabilityRejectionOutcome::Failed);
        assert_eq!(error.outcome().as_str(), "failed");
        assert!(error.normalized_input_digest_sha256().is_some());
        assert_eq!(denied.executions.load(Ordering::SeqCst), 0);

        let success = fixture(available_authority(), true, true, None);
        let error = success
            .broker
            .invoke(
                success.principal,
                call_id(),
                call(&success, READ_KEY, 1, json!({ "query": "x" })),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(error.code(), BrokerErrorCode::AuditUnavailable);
        assert_eq!(success.executions.load(Ordering::SeqCst), 1);
    }
}
