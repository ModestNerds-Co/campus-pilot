//! Executes one fenced Agent run through provider and capability boundaries.
//!
//! Provider input is prepared before the durable attempt is marked in flight.
//! The sole outbound send happens only after token reservations and the exact
//! queue lease have been consumed. Capability handlers likewise run only after
//! their broker-derived binding and usage reservation are durably claimed.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use cp_agent::{
    AuthenticatedAgentPrincipal, AuthorityLoader, BrokerErrorCode, CapabilityBroker,
    CapabilityCall, CapabilityCallId, CapabilityExecutionProof, CapabilityScope,
    CapabilityWorkerLease, ProviderToolCatalog, ProviderToolSelectionContext,
    ProviderToolTaskClass,
};
use cp_agent_runtime::{
    AgentProviderKey, AgentSessionError, AgentSessionOps, AgentUsageDemand, AgentUsageError,
    AgentUsageMeter, AgentUsageRuntime, AgentUsageStage, ArtifactBinding, ArtifactKeyring,
    CapabilityCallDuration, CapabilityCallFailure, CapabilityCallPlan, CapabilityCallScope,
    CapabilityCallSequence, CapabilityFailureStatus, CapabilityResourceReference, ClaimedRun,
    FinalResponsePlaintext, NormalizedProviderUsage, PrepareAgentUsage, ProviderAttemptFailure,
    ProviderAttemptPlan, ProviderPreflightFailure, ProviderTurnIndex, ProviderUpstreamFailure,
    ResolveRouteCommand, RunCheckpoint, RunLease, TaskClass,
};
use cp_ai_providers::{
    AiProviderOps, ExecuteProviderCommand, ProviderExecutionError, ProviderExecutionResponse,
    ProviderExecutionTarget, ProviderFailureCategory, ProviderMessage, ProviderToolCall,
    ProviderToolDefinition,
};
use cp_audit::RequestContext;
use cp_common::{AgentExposure, RuntimeAccessChecks, operation_catalog};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    is_initial_worker_operation,
    worker_supervisor::{
        AgentExecutionFailure, AgentExecutionFailureDisposition, AgentRunExecutor,
    },
};

const RESERVATION_TTL: Duration = Duration::from_secs(15 * 60);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
const MAX_OUTPUT_TOKENS: u32 = 4_096;
const MAX_TURNS: u16 = 16;
const MAX_CAPABILITY_CALLS: u16 = 16;

#[derive(Clone)]
pub struct ProviderAgentRunExecutor {
    sessions: AgentSessionOps,
    usage: AgentUsageRuntime,
    providers: AiProviderOps,
    routing: cp_agent_runtime::AiRoutingOps,
    authority: Arc<dyn AuthorityLoader>,
    broker: Arc<CapabilityBroker>,
    keyring: ArtifactKeyring,
}

impl ProviderAgentRunExecutor {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        sessions: AgentSessionOps,
        usage: AgentUsageRuntime,
        providers: AiProviderOps,
        routing: cp_agent_runtime::AiRoutingOps,
        authority: Arc<dyn AuthorityLoader>,
        broker: Arc<CapabilityBroker>,
        keyring: ArtifactKeyring,
    ) -> Self {
        Self {
            sessions,
            usage,
            providers,
            routing,
            authority,
            broker,
            keyring,
        }
    }

    async fn execute_fresh(&self, run: ClaimedRun) -> Result<(), AgentExecutionFailure> {
        if !matches!(
            run.checkpoint,
            RunCheckpoint::Queued | RunCheckpoint::BeforeProvider
        ) {
            return self.resume_terminal_or_fail(run).await;
        }

        let principal = AuthenticatedAgentPrincipal::from_authenticated_request(
            run.tenant_id,
            run.requested_by,
        );
        let authority = self.authority.load(principal).await.map_err(|_| {
            failure(
                &run.lease,
                "agent_authority_unavailable",
                "Current Agent access could not be loaded",
            )
        })?;
        require_run_authority(&authority).map_err(|facts| failure(&run.lease, facts.0, facts.1))?;

        let authorized = self
            .broker
            .registry()
            .descriptors()
            .into_iter()
            .filter(|descriptor| {
                operation_catalog()
                    .iter()
                    .find(|entry| entry.operation().key() == descriptor.operation_key().as_str())
                    .is_some_and(|entry| {
                        entry.operation().agent_exposure() == AgentExposure::Exposed
                            && is_initial_worker_operation(entry.operation().key())
                            && authority
                                .access()
                                .evaluate_operation(
                                    entry.operation(),
                                    RuntimeAccessChecks::default(),
                                )
                                .allowed
                    })
            })
            .collect::<Vec<_>>();
        let tool_catalog = ProviderToolCatalog::from_authorized(authorized).map_err(|_| {
            failure(
                &run.lease,
                "agent_tool_catalog_invalid",
                "The Agent capability catalogue is unavailable",
            )
        })?;
        let task_class = provider_tool_task_class(run.task_class);
        let shortlisted = tool_catalog.shortlist(ProviderToolSelectionContext::new(
            task_class,
            &run.origin_module_key,
        ));
        let mut required_data_class = cp_common::ProviderDataClass::SensitiveDataApproved;
        let mut tool_definitions = Vec::new();
        for tool in shortlisted
            .into_iter()
            .take(usize::from(MAX_CAPABILITY_CALLS))
        {
            required_data_class = tighten_required_data_class(
                required_data_class,
                self.broker
                    .registry()
                    .descriptors()
                    .into_iter()
                    .find(|descriptor| {
                        descriptor.key() == tool.capability_key()
                            && descriptor.version() == tool.capability_version()
                    })
                    .map_or(
                        cp_common::ProviderDataClass::SensitiveDataApproved,
                        |descriptor| descriptor.policy().provider_data_class(),
                    ),
            );
            tool_definitions.push(
                ProviderToolDefinition::parse(
                    tool.provider_name(),
                    tool.description(),
                    tool.input_schema().value().clone(),
                )
                .map_err(|_| {
                    failure(
                        &run.lease,
                        "agent_tool_catalog_invalid",
                        "The Agent capability catalogue is unavailable",
                    )
                })?,
            );
        }

        let requires_tools = !tool_definitions.is_empty();
        let route_command = ResolveRouteCommand::parse(
            run.task_class.as_str(),
            Some(&run.origin_module_key),
            Some("read"),
            None,
            None,
            requires_tools,
        )
        .map_err(|_| {
            failure(
                &run.lease,
                "agent_route_invalid",
                "The Agent route request is invalid",
            )
        })?
        .requiring_provider_data_class(required_data_class);
        let route = self
            .routing
            .resolve_route(run.tenant_id, route_command)
            .await
            .map_err(|_| {
                failure(
                    &run.lease,
                    "agent_route_unavailable",
                    "No usable Agent route is available",
                )
            })?;

        let mut conversation = vec![ConversationMessage::User(run.request_message.clone())];
        let mut lease = run.lease.clone();
        let mut capability_sequence = 0_u16;

        for turn in 1..=MAX_TURNS {
            let mut completed_turn = false;
            for (target_offset, target) in route.targets.iter().enumerate() {
                let attempt = u8::try_from(target_offset + 1).map_err(|_| {
                    failure(
                        &lease,
                        "agent_route_invalid",
                        "The Agent route contains too many fallback targets",
                    )
                })?;
                let output_tokens = u32::try_from(target.max_output_tokens())
                    .unwrap_or(u32::MAX)
                    .min(MAX_OUTPUT_TOKENS);
                let execution_target = ProviderExecutionTarget::parse(
                    route.route_set_id,
                    route.route_version,
                    target.route_target_id(),
                    target.connection_id(),
                    target.expected_credential_version(),
                    target.model_snapshot_id(),
                    target.provider_data_approval_id(),
                    route.requires_tools,
                )
                .map_err(|_| {
                    failure(
                        &lease,
                        "agent_route_invalid",
                        "The Agent route target is invalid",
                    )
                })?;

                let command = ExecuteProviderCommand::parse(
                    execution_target,
                    run.task_class.as_str(),
                    target.provider_model_id(),
                    system_prompt(&run.origin_module_key),
                    materialize_messages(&conversation).map_err(|_| {
                        failure(
                            &lease,
                            "agent_message_invalid",
                            "The Agent message history is invalid",
                        )
                    })?,
                    rebuild_tool_definitions(&tool_catalog, task_class, &run.origin_module_key)
                        .map_err(|_| {
                            failure(
                                &lease,
                                "agent_tool_catalog_invalid",
                                "The Agent capability catalogue is unavailable",
                            )
                        })?,
                    output_tokens,
                )
                .map_err(|_| {
                    failure(
                        &lease,
                        "agent_provider_input_invalid",
                        "The Agent provider input is invalid",
                    )
                })?
                .requiring_provider_data_class(route.required_provider_data_class);
                let input_upper_bound = command.conservative_input_token_upper_bound();
                let preflight_fingerprint = provider_preflight_fingerprint(
                    run.tenant_id,
                    run.lease.run_id,
                    turn,
                    attempt,
                    target.route_target_id(),
                );
                let prepared = match self
                    .providers
                    .prepare_execution(run.tenant_id, command)
                    .await
                {
                    Ok(prepared) => prepared,
                    Err(error) => {
                        let provider_key = agent_provider_key(target.provider_key())
                            .map_err(|facts| failure(&lease, facts.0, facts.1))?;
                        let plan = provider_plan(
                            &route,
                            target,
                            turn,
                            attempt,
                            provider_key,
                            preflight_fingerprint,
                        )
                        .map_err(|_| {
                            failure(
                                &lease,
                                "agent_provider_attempt_invalid",
                                "The Agent provider attempt could not be prepared",
                            )
                        })?;
                        let persisted = self
                            .sessions
                            .prepare_provider_attempt(run.tenant_id, &lease, plan)
                            .await
                            .map_err(|error| session_failure(&lease, error))?;
                        lease = persisted.lease;
                        let reservation = self
                            .prepare_provider_usage(
                                &run,
                                persisted.identity.attempt_id,
                                preflight_fingerprint,
                                input_upper_bound,
                                output_tokens,
                            )
                            .await
                            .map_err(|error| usage_failure(&lease, error))?;
                        let normalized = map_preflight_error(error);
                        lease = self
                            .sessions
                            .persist_provider_failure(
                                run.tenant_id,
                                &lease,
                                persisted.identity,
                                ProviderAttemptFailure::Preflight(normalized),
                                NormalizedProviderUsage::unknown(),
                            )
                            .await
                            .map_err(|error| session_failure(&lease, error))?;
                        self.usage
                            .commit_terminal_usage(run.tenant_id, reservation.reservation_id)
                            .await
                            .map_err(|error| usage_failure(&lease, error))?;
                        return Err(failure(
                            &lease,
                            "agent_provider_preflight_failed",
                            "The Agent provider is not ready",
                        ));
                    }
                };

                let fingerprint = prepared.input_fingerprint_sha256();
                let provider_key = agent_provider_key(target.provider_key())
                    .map_err(|facts| failure(&lease, facts.0, facts.1))?;
                let plan = provider_plan(&route, target, turn, attempt, provider_key, fingerprint)
                    .map_err(|_| {
                        failure(
                            &lease,
                            "agent_provider_attempt_invalid",
                            "The Agent provider attempt could not be prepared",
                        )
                    })?;
                let persisted = self
                    .sessions
                    .prepare_provider_attempt(run.tenant_id, &lease, plan)
                    .await
                    .map_err(|error| session_failure(&lease, error))?;
                lease = persisted.lease;
                let reservation = self
                    .prepare_provider_usage(
                        &run,
                        persisted.identity.attempt_id,
                        fingerprint,
                        input_upper_bound,
                        output_tokens,
                    )
                    .await
                    .map_err(|error| usage_failure(&lease, error))?;
                self.revalidate_run_authority(&run, &lease).await?;
                lease = self
                    .sessions
                    .mark_provider_in_flight(run.tenant_id, &lease, persisted.identity)
                    .await
                    .map_err(|error| session_failure(&lease, error))?;
                self.usage
                    .claim_provider_attempt(
                        run.tenant_id,
                        run.requested_by,
                        reservation.reservation_id,
                        &lease,
                    )
                    .await
                    .map_err(|error| usage_failure(&lease, error))?;
                self.revalidate_run_authority(&run, &lease).await?;

                let send_outcome = self
                    .send_with_heartbeats(run.tenant_id, lease, prepared)
                    .await?;
                let response = match send_outcome {
                    InFlightOutcome::Completed {
                        lease: latest_lease,
                        result,
                    } => {
                        lease = latest_lease;
                        result
                    }
                    InFlightOutcome::CancellationRequested { lease: cancelled } => {
                        return Err(self
                            .acknowledge_claimed_cancellation(
                                run.tenant_id,
                                cancelled,
                                reservation.reservation_id,
                                "agent_cancel_requested_during_provider",
                                "The Agent run was cancelled while the provider request was in flight",
                            )
                            .await);
                    }
                };
                match response {
                    Ok(response) => {
                        let artifact_bytes = stored_provider_result(&response).map_err(|_| {
                            failure(
                                &lease,
                                "agent_provider_result_invalid",
                                "The Agent provider result could not be persisted",
                            )
                        })?;
                        let artifact = self
                            .keyring
                            .encrypt(
                                ArtifactBinding::provider_result(
                                    run.tenant_id,
                                    run.lease.run_id,
                                    persisted.identity.step_id,
                                ),
                                &artifact_bytes,
                            )
                            .map_err(|_| {
                                failure(
                                    &lease,
                                    "agent_artifact_unavailable",
                                    "The Agent continuation could not be protected",
                                )
                            })?;
                        let usage = normalized_provider_usage(&response)
                            .map_err(|error| session_failure(&lease, error))?;
                        let persisted_result = self
                            .sessions
                            .persist_provider_success(
                                run.tenant_id,
                                &lease,
                                persisted.identity,
                                usage,
                                artifact,
                            )
                            .await
                            .map_err(|error| session_failure(&lease, error))?;
                        lease = persisted_result.lease;
                        self.usage
                            .commit_terminal_usage(run.tenant_id, reservation.reservation_id)
                            .await
                            .map_err(|error| usage_failure(&lease, error))?;

                        if response.tool_calls.is_empty() {
                            let text = response.assistant_text.ok_or_else(|| {
                                failure(
                                    &lease,
                                    "agent_provider_result_empty",
                                    "The Agent provider returned no response",
                                )
                            })?;
                            return self.finalize(&run, lease, turn, text).await;
                        }
                        conversation.push(ConversationMessage::Assistant {
                            text: response.assistant_text.clone(),
                            tool_calls: response.tool_calls.clone(),
                        });
                        for tool_call in response.tool_calls {
                            capability_sequence =
                                capability_sequence.checked_add(1).ok_or_else(|| {
                                    failure(
                                        &lease,
                                        "agent_capability_limit_reached",
                                        "This Agent run requested too many capability calls",
                                    )
                                })?;
                            if capability_sequence > MAX_CAPABILITY_CALLS {
                                return Err(failure(
                                    &lease,
                                    "agent_capability_limit_reached",
                                    "This Agent run requested too many capability calls",
                                ));
                            }
                            let (next_lease, result_message) = self
                                .execute_capability(
                                    &run,
                                    &tool_catalog,
                                    lease,
                                    turn,
                                    capability_sequence,
                                    tool_call,
                                )
                                .await?;
                            lease = next_lease;
                            conversation.push(result_message);
                        }
                        completed_turn = true;
                        break;
                    }
                    Err(error) => {
                        if let Some(preflight) = map_send_preflight_error(&error) {
                            lease = self
                                .sessions
                                .persist_provider_failure(
                                    run.tenant_id,
                                    &lease,
                                    persisted.identity,
                                    ProviderAttemptFailure::Preflight(preflight),
                                    NormalizedProviderUsage::unknown(),
                                )
                                .await
                                .map_err(|failure_error| session_failure(&lease, failure_error))?;
                            self.usage
                                .commit_terminal_usage(run.tenant_id, reservation.reservation_id)
                                .await
                                .map_err(|usage_error| usage_failure(&lease, usage_error))?;
                            return Err(failure(
                                &lease,
                                "agent_provider_preflight_failed",
                                "The Agent provider changed before dispatch",
                            ));
                        }
                        let upstream = map_upstream_error(&error).ok_or_else(|| {
                            failure(
                                &lease,
                                "agent_provider_failed",
                                "The Agent provider request failed",
                            )
                        })?;
                        lease = self
                            .sessions
                            .persist_provider_failure(
                                run.tenant_id,
                                &lease,
                                persisted.identity,
                                ProviderAttemptFailure::Upstream(upstream),
                                NormalizedProviderUsage::unknown(),
                            )
                            .await
                            .map_err(|failure_error| session_failure(&lease, failure_error))?;
                        self.usage
                            .commit_terminal_usage(run.tenant_id, reservation.reservation_id)
                            .await
                            .map_err(|usage_error| usage_failure(&lease, usage_error))?;
                        if !fallback_eligible(upstream) {
                            return Err(failure(
                                &lease,
                                "agent_provider_failed",
                                "The Agent provider request failed",
                            ));
                        }
                    }
                }
            }
            if !completed_turn {
                return Err(failure(
                    &lease,
                    "agent_provider_chain_unavailable",
                    "No Agent provider target completed the request",
                ));
            }
        }
        Err(failure(
            &lease,
            "agent_turn_limit_reached",
            "This Agent run exceeded the provider turn limit",
        ))
    }

    async fn prepare_provider_usage(
        &self,
        run: &ClaimedRun,
        attempt_id: Uuid,
        fingerprint: [u8; 32],
        input_upper_bound: u64,
        output_upper_bound: u32,
    ) -> Result<cp_agent_runtime::PreparedAgentUsage, AgentUsageError> {
        let demands = [
            AgentUsageDemand::count(AgentUsageMeter::ProviderAttempts, 1)?,
            AgentUsageDemand::count(AgentUsageMeter::InputTokens, input_upper_bound)?,
            AgentUsageDemand::count(AgentUsageMeter::OutputTokens, u64::from(output_upper_bound))?,
            AgentUsageDemand::count(AgentUsageMeter::CachedInputTokens, input_upper_bound)?,
            AgentUsageDemand::count(
                AgentUsageMeter::ReasoningTokens,
                u64::from(output_upper_bound),
            )?,
        ];
        let command = PrepareAgentUsage::parse(
            run.lease.run_id,
            AgentUsageStage::ProviderAttempt { attempt_id },
            &format!("agent:provider:{attempt_id}"),
            fingerprint,
            demands,
            RESERVATION_TTL,
        )?;
        self.usage
            .prepare(run.tenant_id, run.requested_by, command)
            .await
    }

    async fn send_with_heartbeats(
        &self,
        tenant_id: Uuid,
        mut lease: RunLease,
        prepared: cp_ai_providers::PreparedProviderExecution,
    ) -> Result<
        InFlightOutcome<Result<ProviderExecutionResponse, ProviderExecutionError>>,
        AgentExecutionFailure,
    > {
        let send = self.providers.send_prepared_execution(prepared);
        tokio::pin!(send);
        let mut interval = tokio::time::interval(HEARTBEAT_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        interval.tick().await;
        loop {
            tokio::select! {
                response = &mut send => {
                    return Ok(InFlightOutcome::Completed { lease, result: response });
                }
                _ = interval.tick() => {
                    let heartbeat = self.sessions.heartbeat(tenant_id, &lease).await
                        .map_err(|error| session_failure(&lease, error))?;
                    lease = heartbeat.lease;
                    if heartbeat.cancel_requested {
                        return Ok(InFlightOutcome::CancellationRequested { lease });
                    }
                }
            }
        }
    }

    async fn revalidate_run_authority(
        &self,
        run: &ClaimedRun,
        lease: &RunLease,
    ) -> Result<(), AgentExecutionFailure> {
        let principal = AuthenticatedAgentPrincipal::from_authenticated_request(
            run.tenant_id,
            run.requested_by,
        );
        let authority = self.authority.load(principal).await.map_err(|_| {
            failure(
                lease,
                "agent_authority_unavailable",
                "Current Agent access could not be loaded",
            )
        })?;
        require_run_authority(&authority).map_err(|facts| failure(lease, facts.0, facts.1))
    }

    async fn acknowledge_claimed_cancellation(
        &self,
        tenant_id: Uuid,
        lease: RunLease,
        reservation_id: Uuid,
        code: &'static str,
        message: &'static str,
    ) -> AgentExecutionFailure {
        let acknowledged = self
            .sessions
            .acknowledge_cancellation(tenant_id, &lease)
            .await
            .is_ok();
        if acknowledged {
            // Cancellation terminalizes the in-flight child first. Reconciliation
            // can then commit the claimed upper bound without pretending that the
            // provider or capability returned exact usage.
            let _ = self
                .usage
                .commit_terminal_usage(tenant_id, reservation_id)
                .await;
        }
        cancellation_failure(&lease, code, message, acknowledged)
    }

    async fn commit_denied_usage(
        &self,
        tenant_id: Uuid,
        reservation_id: Uuid,
        lease: &RunLease,
    ) -> Result<(), AgentExecutionFailure> {
        match self
            .usage
            .commit_terminal_usage(tenant_id, reservation_id)
            .await
        {
            Err(AgentUsageError::Denied {
                reservation_id: stored,
            }) if stored == reservation_id => Ok(()),
            Ok(_) => Err(usage_failure(lease, AgentUsageError::InvalidTransition)),
            Err(error) => Err(usage_failure(lease, error)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_capability(
        &self,
        run: &ClaimedRun,
        catalog: &ProviderToolCatalog<'_>,
        lease: RunLease,
        turn: u16,
        sequence: u16,
        tool_call: ProviderToolCall,
    ) -> Result<(RunLease, ConversationMessage), AgentExecutionFailure> {
        let provider_name = tool_call.name().to_owned();
        let provider_call_id = tool_call.id().to_owned();
        let Some(tool) = catalog.resolve(&provider_name) else {
            return Err(failure(
                &lease,
                "agent_capability_not_authorized",
                "The provider requested an unavailable capability",
            ));
        };
        let call_id = deterministic_call_id(run.lease.run_id, turn, sequence, &provider_call_id);
        let principal = AuthenticatedAgentPrincipal::from_authenticated_request(
            run.tenant_id,
            run.requested_by,
        );
        let context = RequestContext::from_ids(call_id, run.correlation_id);
        let call = CapabilityCall::parse(
            tool.capability_key().as_str(),
            tool.capability_version().get(),
            tool_call.arguments().clone(),
            context,
        )
        .map_err(|_| {
            failure(
                &lease,
                "agent_capability_input_invalid",
                "The requested capability input is invalid",
            )
        })?
        .with_agent_run_id(run.lease.run_id);
        let mut prepared = match self
            .broker
            .prepare(
                principal,
                CapabilityCallId::from_trusted_runtime(call_id),
                call,
            )
            .await
        {
            Ok(prepared) => prepared,
            Err(rejection) => {
                let call_sequence = CapabilityCallSequence::parse(sequence)
                    .map_err(|error| session_failure(&lease, error))?;
                self.sessions
                    .record_capability_rejection(run.tenant_id, &lease, call_sequence, &rejection)
                    .await
                    .map_err(|error| session_failure(&lease, error))?;
                let content = serde_json::to_string(&json!({
                    "error": rejection.code().as_str(),
                    "message": rejection.safe_message(),
                }))
                .unwrap_or_else(|_| "{\"error\":\"capability_rejected\"}".to_owned());
                let message = ConversationMessage::ToolResult {
                    tool_call_id: provider_call_id,
                    name: provider_name,
                    content,
                    is_error: true,
                };
                return Ok((lease, message));
            }
        };
        let facts = prepared.facts();
        let plan = CapabilityCallPlan::parse(
            call_id,
            turn,
            sequence,
            facts.key().as_str(),
            i32::from(facts.version().get()),
            facts.operation_key(),
            facts.module_key(),
            facts.required_permission(),
            facts.input_binding_sha256(),
            runtime_scope(facts.scope()).map_err(|error| session_failure(&lease, error))?,
        )
        .map_err(|error| session_failure(&lease, error))?;
        let persisted = self
            .sessions
            .prepare_capability_call(run.tenant_id, &lease, plan)
            .await
            .map_err(|error| session_failure(&lease, error))?;
        let mut lease = persisted.lease;
        let reservation_command = PrepareAgentUsage::parse(
            run.lease.run_id,
            AgentUsageStage::CapabilityCall { call_id },
            &format!("agent:capability:{call_id}"),
            facts.input_binding_sha256(),
            [AgentUsageDemand::count(AgentUsageMeter::CapabilityCalls, 1)
                .map_err(|error| usage_failure(&lease, error))?],
            RESERVATION_TTL,
        )
        .map_err(|error| usage_failure(&lease, error))?;
        let reservation = match self
            .usage
            .prepare(run.tenant_id, run.requested_by, reservation_command)
            .await
        {
            Ok(reservation) => reservation,
            Err(AgentUsageError::Denied { reservation_id }) => {
                let content = json!({
                    "error": "agent_usage_limit_denied",
                    "message": "This capability exceeds a configured usage limit",
                });
                let artifact_bytes = serde_json::to_vec(&content).map_err(|_| {
                    failure(
                        &lease,
                        "agent_capability_result_invalid",
                        "The capability denial could not be persisted",
                    )
                })?;
                let artifact = self
                    .keyring
                    .encrypt(
                        ArtifactBinding::capability_result(
                            run.tenant_id,
                            run.lease.run_id,
                            persisted.identity.step_id,
                        ),
                        &artifact_bytes,
                    )
                    .map_err(|_| {
                        failure(
                            &lease,
                            "agent_artifact_unavailable",
                            "The Agent continuation could not be protected",
                        )
                    })?;
                lease = self
                    .sessions
                    .persist_capability_failure(
                        run.tenant_id,
                        &lease,
                        persisted.identity,
                        CapabilityCallFailure::parse(
                            CapabilityFailureStatus::Denied,
                            "agent_usage_limit_denied",
                            0,
                        )
                        .map_err(|error| session_failure(&lease, error))?,
                        artifact,
                    )
                    .await
                    .map_err(|error| session_failure(&lease, error))?
                    .lease;
                self.commit_denied_usage(run.tenant_id, reservation_id, &lease)
                    .await?;
                let message = ConversationMessage::ToolResult {
                    tool_call_id: provider_call_id,
                    name: provider_name,
                    content: serde_json::to_string(&content).unwrap_or_default(),
                    is_error: true,
                };
                return Ok((lease, message));
            }
            Err(error) => return Err(usage_failure(&lease, error)),
        };
        let worker_lease =
            CapabilityWorkerLease::parse(&lease.worker_id, lease.lease_token, lease.fence_version)
                .map_err(|_| {
                    failure(
                        &lease,
                        "agent_capability_proof_invalid",
                        "The capability execution proof is invalid",
                    )
                })?;
        let proof = CapabilityExecutionProof::parse(
            principal,
            CapabilityCallId::from_trusted_runtime(call_id),
            run.lease.run_id,
            worker_lease,
            reservation.reservation_id,
        )
        .map_err(|_| {
            failure(
                &lease,
                "agent_capability_proof_invalid",
                "The capability execution proof is invalid",
            )
        })?;
        let started = Instant::now();
        let capability_outcome = self
            .execute_capability_with_heartbeats(run.tenant_id, lease, &mut prepared, proof)
            .await?;
        let (mut lease, result) = match capability_outcome {
            InFlightOutcome::Completed {
                lease: latest_lease,
                result,
            } => (latest_lease, result),
            InFlightOutcome::CancellationRequested { lease: cancelled } => {
                return Err(self
                    .acknowledge_claimed_cancellation(
                        run.tenant_id,
                        cancelled,
                        reservation.reservation_id,
                        "agent_cancel_requested_during_capability",
                        "The Agent run was cancelled while a capability was in flight",
                    )
                    .await);
            }
        };
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let (content, is_error, status, failure_code) = match result {
            Ok(result) => (result.content().clone(), false, None, None),
            Err(error) => {
                let denied = matches!(
                    error.code(),
                    BrokerErrorCode::ApprovalRequired
                        | BrokerErrorCode::HumanOnly
                        | BrokerErrorCode::Prohibited
                        | BrokerErrorCode::AccessDenied
                        | BrokerErrorCode::RecordScopeDenied
                );
                (
                    json!({ "error": error.code().as_str(), "message": error.safe_message() }),
                    true,
                    Some(if denied {
                        CapabilityFailureStatus::Denied
                    } else {
                        CapabilityFailureStatus::Failed
                    }),
                    Some(error.code().as_str()),
                )
            }
        };
        let artifact_bytes = serde_json::to_vec(&content).map_err(|_| {
            failure(
                &lease,
                "agent_capability_result_invalid",
                "The capability result could not be persisted",
            )
        })?;
        let artifact = self
            .keyring
            .encrypt(
                ArtifactBinding::capability_result(
                    run.tenant_id,
                    run.lease.run_id,
                    persisted.identity.step_id,
                ),
                &artifact_bytes,
            )
            .map_err(|_| {
                failure(
                    &lease,
                    "agent_artifact_unavailable",
                    "The Agent continuation could not be protected",
                )
            })?;
        lease = match status {
            None => {
                self.sessions
                    .persist_capability_success(
                        run.tenant_id,
                        &lease,
                        persisted.identity,
                        CapabilityCallDuration::parse(duration_ms)
                            .map_err(|error| session_failure(&lease, error))?,
                        artifact,
                    )
                    .await
                    .map_err(|error| session_failure(&lease, error))?
                    .lease
            }
            Some(status) => {
                self.sessions
                    .persist_capability_failure(
                        run.tenant_id,
                        &lease,
                        persisted.identity,
                        CapabilityCallFailure::parse(
                            status,
                            failure_code.unwrap_or("capability_failed"),
                            duration_ms,
                        )
                        .map_err(|error| session_failure(&lease, error))?,
                        artifact,
                    )
                    .await
                    .map_err(|error| session_failure(&lease, error))?
                    .lease
            }
        };
        self.usage
            .commit_terminal_usage(run.tenant_id, reservation.reservation_id)
            .await
            .map_err(|error| usage_failure(&lease, error))?;
        let content = serde_json::to_string(&content).map_err(|_| {
            failure(
                &lease,
                "agent_capability_result_invalid",
                "The capability result could not be returned to the provider",
            )
        })?;
        let message = ConversationMessage::ToolResult {
            tool_call_id: provider_call_id,
            name: provider_name,
            content,
            is_error,
        };
        Ok((lease, message))
    }

    async fn finalize(
        &self,
        run: &ClaimedRun,
        lease: RunLease,
        turn: u16,
        text: String,
    ) -> Result<(), AgentExecutionFailure> {
        self.revalidate_run_authority(run, &lease).await?;
        let plaintext = FinalResponsePlaintext::parse(text.clone())
            .map_err(|error| session_failure(&lease, error))?;
        let artifact = self
            .keyring
            .encrypt(
                ArtifactBinding::final_response(run.tenant_id, run.lease.run_id),
                text.as_bytes(),
            )
            .map_err(|_| {
                failure(
                    &lease,
                    "agent_artifact_unavailable",
                    "The Agent response could not be protected",
                )
            })?;
        let persisted = self
            .sessions
            .persist_final_response(
                run.tenant_id,
                &lease,
                ProviderTurnIndex::parse(turn).map_err(|error| session_failure(&lease, error))?,
                artifact,
            )
            .await
            .map_err(|error| session_failure(&lease, error))?;
        self.revalidate_run_authority(run, &persisted.lease).await?;
        self.sessions
            .complete_run(
                run.tenant_id,
                &persisted.lease,
                persisted.artifact.id,
                plaintext,
            )
            .await
            .map_err(|error| session_failure(&persisted.lease, error))?;
        Ok(())
    }

    async fn execute_capability_with_heartbeats(
        &self,
        tenant_id: Uuid,
        mut lease: RunLease,
        prepared: &mut cp_agent::PreparedCapabilityCall,
        proof: CapabilityExecutionProof,
    ) -> Result<
        InFlightOutcome<Result<cp_agent::CapabilityResult, cp_agent::BrokerError>>,
        AgentExecutionFailure,
    > {
        let execution = self.broker.execute_prepared(prepared, proof);
        tokio::pin!(execution);
        let mut interval = tokio::time::interval(HEARTBEAT_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        interval.tick().await;
        loop {
            tokio::select! {
                result = &mut execution => {
                    return Ok(InFlightOutcome::Completed { lease, result });
                }
                _ = interval.tick() => {
                    let heartbeat = self.sessions.heartbeat(tenant_id, &lease).await
                        .map_err(|error| session_failure(&lease, error))?;
                    lease = heartbeat.lease;
                    if heartbeat.cancel_requested {
                        return Ok(InFlightOutcome::CancellationRequested { lease });
                    }
                }
            }
        }
    }

    async fn resume_terminal_or_fail(&self, run: ClaimedRun) -> Result<(), AgentExecutionFailure> {
        if run.checkpoint == RunCheckpoint::Finalizing {
            let snapshot = self
                .sessions
                .load_execution_snapshot(run.tenant_id, &run.lease)
                .await
                .map_err(|error| session_failure(&run.lease, error))?;
            if let Some(cp_agent_runtime::ExecutionStepSnapshot::Finalization(finalization)) =
                snapshot.steps.into_iter().rev().find(|step| {
                    matches!(
                        step,
                        cp_agent_runtime::ExecutionStepSnapshot::Finalization(_)
                    )
                })
            {
                let artifact = finalization.step.artifact.ok_or_else(|| {
                    failure(
                        &run.lease,
                        "agent_recovery_evidence_missing",
                        "The Agent response recovery evidence is unavailable",
                    )
                })?;
                let artifact_id = artifact.id;
                let decrypted = self
                    .keyring
                    .decrypt_loaded(run.tenant_id, run.lease.run_id, artifact)
                    .map_err(|_| {
                        failure(
                            &run.lease,
                            "agent_recovery_evidence_invalid",
                            "The Agent response recovery evidence is invalid",
                        )
                    })?;
                let text = String::from_utf8(decrypted.into_bytes()).map_err(|_| {
                    failure(
                        &run.lease,
                        "agent_recovery_evidence_invalid",
                        "The Agent response recovery evidence is invalid",
                    )
                })?;
                self.sessions
                    .complete_run(
                        run.tenant_id,
                        &run.lease,
                        artifact_id,
                        FinalResponsePlaintext::parse(text)
                            .map_err(|error| session_failure(&run.lease, error))?,
                    )
                    .await
                    .map_err(|error| session_failure(&run.lease, error))?;
                return Ok(());
            }
        }
        Err(failure(
            &run.lease,
            "agent_recovery_requires_reconciliation",
            "This Agent run requires manual recovery",
        ))
    }
}

#[async_trait]
impl AgentRunExecutor for ProviderAgentRunExecutor {
    async fn execute(&self, run: ClaimedRun) -> Result<(), AgentExecutionFailure> {
        self.execute_fresh(run).await
    }
}

#[derive(Serialize, Deserialize)]
struct StoredProviderResult {
    assistant_text: Option<String>,
    tool_calls: Vec<StoredToolCall>,
    usage: StoredUsage,
}

enum ConversationMessage {
    User(String),
    Assistant {
        text: Option<String>,
        tool_calls: Vec<ProviderToolCall>,
    },
    ToolResult {
        tool_call_id: String,
        name: String,
        content: String,
        is_error: bool,
    },
}

enum InFlightOutcome<T> {
    Completed { lease: RunLease, result: T },
    CancellationRequested { lease: RunLease },
}

fn materialize_messages(
    conversation: &[ConversationMessage],
) -> Result<Vec<ProviderMessage>, ProviderExecutionError> {
    conversation
        .iter()
        .map(|message| match message {
            ConversationMessage::User(content) => ProviderMessage::user(content.clone()),
            ConversationMessage::Assistant { text, tool_calls } => {
                ProviderMessage::assistant(text.clone(), tool_calls.clone())
            }
            ConversationMessage::ToolResult {
                tool_call_id,
                name,
                content,
                is_error,
            } => ProviderMessage::tool_result(
                tool_call_id.clone(),
                name.clone(),
                content.clone(),
                *is_error,
            ),
        })
        .collect()
}

#[derive(Serialize, Deserialize)]
struct StoredToolCall {
    id: String,
    name: String,
    arguments: Value,
}

#[derive(Serialize, Deserialize)]
struct StoredUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    total_tokens: Option<u64>,
    reasoning_tokens: Option<u64>,
    cached_input_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
}

fn stored_provider_result(
    response: &ProviderExecutionResponse,
) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(&StoredProviderResult {
        assistant_text: response.assistant_text.clone(),
        tool_calls: response
            .tool_calls
            .iter()
            .map(|call| StoredToolCall {
                id: call.id().to_owned(),
                name: call.name().to_owned(),
                arguments: call.arguments().clone(),
            })
            .collect(),
        usage: StoredUsage {
            input_tokens: response.usage.input_tokens,
            output_tokens: response.usage.output_tokens,
            total_tokens: response.usage.total_tokens,
            reasoning_tokens: response.usage.reasoning_tokens,
            cached_input_tokens: response.usage.cached_input_tokens,
            cache_creation_input_tokens: response.usage.cache_creation_input_tokens,
        },
    })
}

fn normalized_provider_usage(
    response: &ProviderExecutionResponse,
) -> Result<NormalizedProviderUsage, AgentSessionError> {
    let cached = combine_optional(
        response.usage.cached_input_tokens,
        response.usage.cache_creation_input_tokens,
    )?;
    NormalizedProviderUsage::parse(
        response.usage.input_tokens,
        response.usage.output_tokens,
        cached,
        response.usage.reasoning_tokens,
        None,
        None,
    )
}

fn combine_optional(
    left: Option<u64>,
    right: Option<u64>,
) -> Result<Option<u64>, AgentSessionError> {
    match (left, right) {
        (None, None) => Ok(None),
        (left, right) => left
            .unwrap_or(0)
            .checked_add(right.unwrap_or(0))
            .map(Some)
            .ok_or_else(|| {
                AgentSessionError::invalid(
                    "invalid_cached_tokens",
                    "Provider cached tokens overflowed",
                )
            }),
    }
}

fn provider_plan(
    route: &cp_agent_runtime::ResolvedAiRoute,
    target: &cp_agent_runtime::ResolvedAiRouteTarget,
    turn: u16,
    attempt: u8,
    provider_key: AgentProviderKey,
    fingerprint: [u8; 32],
) -> Result<ProviderAttemptPlan, AgentSessionError> {
    ProviderAttemptPlan::parse(
        turn,
        attempt,
        route.route_set_id,
        route.route_version,
        target.route_target_id(),
        target.connection_id(),
        target.expected_credential_version(),
        target.model_snapshot_id(),
        target.provider_data_approval_id(),
        route.required_provider_data_class,
        target.execution_environment_class(),
        provider_key,
        target.provider_model_id(),
        fingerprint,
    )
}

fn map_preflight_error(error: ProviderExecutionError) -> ProviderPreflightFailure {
    match error {
        ProviderExecutionError::ConnectionUnavailable => {
            ProviderPreflightFailure::ConnectionUnavailable
        }
        ProviderExecutionError::StaleCredential => ProviderPreflightFailure::StaleCredential,
        ProviderExecutionError::ProviderDataApprovalChanged => {
            ProviderPreflightFailure::ProviderDataApprovalChanged
        }
        ProviderExecutionError::ProviderDataNotApproved => {
            ProviderPreflightFailure::ProviderDataNotApproved
        }
        ProviderExecutionError::LocalExecutionRequired => {
            ProviderPreflightFailure::LocalExecutionRequired
        }
        ProviderExecutionError::StaleModel => ProviderPreflightFailure::StaleModel,
        ProviderExecutionError::ToolsUnsupported => ProviderPreflightFailure::ToolsUnsupported,
        ProviderExecutionError::ModelContextUnavailable => {
            ProviderPreflightFailure::ModelContextUnavailable
        }
        ProviderExecutionError::ModelOutputUnavailable => {
            ProviderPreflightFailure::ModelOutputUnavailable
        }
        ProviderExecutionError::ContextWindowExceeded => {
            ProviderPreflightFailure::ContextWindowExceeded
        }
        ProviderExecutionError::OutputBudgetExceeded => {
            ProviderPreflightFailure::OutputBudgetExceeded
        }
        ProviderExecutionError::CredentialUnavailable => {
            ProviderPreflightFailure::CredentialUnavailable
        }
        ProviderExecutionError::InvalidConfiguration => {
            ProviderPreflightFailure::InvalidConfiguration
        }
        ProviderExecutionError::InvalidInput { .. } => ProviderPreflightFailure::InvalidInput,
        ProviderExecutionError::Storage => ProviderPreflightFailure::StorageError,
        ProviderExecutionError::Provider(_) => ProviderPreflightFailure::StorageError,
    }
}

fn map_upstream_error(error: &ProviderExecutionError) -> Option<ProviderUpstreamFailure> {
    let failure = error.provider_failure()?;
    Some(match failure.category {
        ProviderFailureCategory::Authentication => ProviderUpstreamFailure::Authentication,
        ProviderFailureCategory::RateLimited => ProviderUpstreamFailure::RateLimited,
        ProviderFailureCategory::Unavailable => ProviderUpstreamFailure::Unavailable,
        ProviderFailureCategory::Timeout => ProviderUpstreamFailure::Timeout,
        ProviderFailureCategory::Network => ProviderUpstreamFailure::Network,
        ProviderFailureCategory::InvalidResponse => ProviderUpstreamFailure::InvalidResponse,
        ProviderFailureCategory::Unsupported => ProviderUpstreamFailure::Unsupported,
    })
}

fn map_send_preflight_error(error: &ProviderExecutionError) -> Option<ProviderPreflightFailure> {
    Some(match error {
        ProviderExecutionError::ConnectionUnavailable => {
            ProviderPreflightFailure::ConnectionUnavailable
        }
        ProviderExecutionError::StaleCredential => ProviderPreflightFailure::StaleCredential,
        ProviderExecutionError::ProviderDataApprovalChanged => {
            ProviderPreflightFailure::ProviderDataApprovalChanged
        }
        ProviderExecutionError::ProviderDataNotApproved => {
            ProviderPreflightFailure::ProviderDataNotApproved
        }
        ProviderExecutionError::LocalExecutionRequired => {
            ProviderPreflightFailure::LocalExecutionRequired
        }
        ProviderExecutionError::StaleModel => ProviderPreflightFailure::StaleModel,
        ProviderExecutionError::ToolsUnsupported => ProviderPreflightFailure::ToolsUnsupported,
        ProviderExecutionError::ModelContextUnavailable => {
            ProviderPreflightFailure::ModelContextUnavailable
        }
        ProviderExecutionError::ModelOutputUnavailable => {
            ProviderPreflightFailure::ModelOutputUnavailable
        }
        ProviderExecutionError::ContextWindowExceeded => {
            ProviderPreflightFailure::ContextWindowExceeded
        }
        ProviderExecutionError::OutputBudgetExceeded => {
            ProviderPreflightFailure::OutputBudgetExceeded
        }
        ProviderExecutionError::CredentialUnavailable => {
            ProviderPreflightFailure::CredentialUnavailable
        }
        ProviderExecutionError::InvalidConfiguration => {
            ProviderPreflightFailure::InvalidConfiguration
        }
        ProviderExecutionError::InvalidInput { .. } => ProviderPreflightFailure::InvalidInput,
        ProviderExecutionError::Storage => ProviderPreflightFailure::StorageError,
        ProviderExecutionError::Provider(_) => return None,
    })
}

const fn fallback_eligible(failure: ProviderUpstreamFailure) -> bool {
    matches!(
        failure,
        ProviderUpstreamFailure::RateLimited | ProviderUpstreamFailure::Unavailable
    )
}

fn runtime_scope(scope: &CapabilityScope) -> Result<CapabilityCallScope, AgentSessionError> {
    match scope {
        CapabilityScope::TenantWide => Ok(CapabilityCallScope::TenantWide),
        CapabilityScope::Resources(resources) => CapabilityCallScope::resources(
            resources
                .values()
                .iter()
                .map(|resource| CapabilityResourceReference::parse(resource.kind(), resource.id()))
                .collect::<Result<Vec<_>, _>>()?,
        ),
    }
}

fn rebuild_tool_definitions(
    catalog: &ProviderToolCatalog<'_>,
    task_class: ProviderToolTaskClass,
    origin_module_key: &str,
) -> Result<Vec<ProviderToolDefinition>, ProviderExecutionError> {
    catalog
        .shortlist(ProviderToolSelectionContext::new(
            task_class,
            origin_module_key,
        ))
        .into_iter()
        .take(usize::from(MAX_CAPABILITY_CALLS))
        .map(|tool| {
            ProviderToolDefinition::parse(
                tool.provider_name(),
                tool.description(),
                tool.input_schema().value().clone(),
            )
        })
        .collect()
}

fn provider_tool_task_class(task: TaskClass) -> ProviderToolTaskClass {
    match task {
        TaskClass::CampusConversation | TaskClass::CampusConversationSearch => {
            ProviderToolTaskClass::CampusConversationSearch
        }
        TaskClass::ModuleReadReporting => ProviderToolTaskClass::ModuleReadReporting,
        TaskClass::DocumentExtraction => ProviderToolTaskClass::DocumentExtraction,
        TaskClass::DraftingProposal => ProviderToolTaskClass::DraftingProposal,
        TaskClass::ApprovedOperationalAction => ProviderToolTaskClass::ApprovedOperationalAction,
    }
}

const fn tighten_required_data_class(
    current: cp_common::ProviderDataClass,
    declared_capability_output: cp_common::ProviderDataClass,
) -> cp_common::ProviderDataClass {
    current.max(declared_capability_output)
}

fn require_run_authority(
    authority: &cp_agent::CurrentAuthority,
) -> Result<(), (&'static str, &'static str)> {
    let operation = operation_catalog()
        .iter()
        .find(|entry| entry.operation().key() == "agent.messages.submit")
        .ok_or((
            "agent_operation_catalog_unavailable",
            "Agent execution policy is unavailable",
        ))?;
    if authority
        .access()
        .evaluate_operation(operation.operation(), RuntimeAccessChecks::default())
        .allowed
    {
        Ok(())
    } else {
        Err((
            "agent_access_revoked",
            "Current access no longer allows this Agent run",
        ))
    }
}

fn agent_provider_key(provider: &str) -> Result<AgentProviderKey, (&'static str, &'static str)> {
    match provider {
        "openai" => Ok(AgentProviderKey::OpenAi),
        "anthropic" => Ok(AgentProviderKey::Anthropic),
        "openrouter" => Ok(AgentProviderKey::OpenRouter),
        _ => Err((
            "agent_provider_unsupported",
            "The selected Agent provider is unsupported",
        )),
    }
}

fn provider_preflight_fingerprint(
    tenant_id: Uuid,
    run_id: Uuid,
    turn: u16,
    attempt: u8,
    route_target_id: Uuid,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"campus-pilot/agent-provider-preflight/v1");
    digest.update(tenant_id.as_bytes());
    digest.update(run_id.as_bytes());
    digest.update(turn.to_be_bytes());
    digest.update(attempt.to_be_bytes());
    digest.update(route_target_id.as_bytes());
    digest.finalize().into()
}

fn deterministic_call_id(run_id: Uuid, turn: u16, sequence: u16, provider_call_id: &str) -> Uuid {
    let mut digest = Sha256::new();
    digest.update(b"campus-pilot/agent-capability-call/v1");
    digest.update(run_id.as_bytes());
    digest.update(turn.to_be_bytes());
    digest.update(sequence.to_be_bytes());
    digest.update(provider_call_id.as_bytes());
    let digest = digest.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn system_prompt(origin_module_key: &str) -> String {
    format!(
        "You are Campus Pilot's operational assistant. Work within the user's current access. Use only offered tools for campus facts; never invent records. The current module is {origin_module_key}. Return concise operational answers."
    )
}

fn failure(lease: &RunLease, code: &'static str, message: &'static str) -> AgentExecutionFailure {
    AgentExecutionFailure {
        lease: lease.clone(),
        code,
        message,
        disposition: AgentExecutionFailureDisposition::FailRun,
    }
}

fn cancellation_failure(
    lease: &RunLease,
    code: &'static str,
    message: &'static str,
    acknowledged: bool,
) -> AgentExecutionFailure {
    AgentExecutionFailure {
        lease: lease.clone(),
        code,
        message,
        disposition: if acknowledged {
            AgentExecutionFailureDisposition::CancellationAcknowledged
        } else {
            AgentExecutionFailureDisposition::CancellationPending
        },
    }
}

fn session_failure(lease: &RunLease, error: AgentSessionError) -> AgentExecutionFailure {
    match error {
        AgentSessionError::LeaseLost => failure(
            lease,
            "agent_worker_lease_lost",
            "The Agent worker lease was lost",
        ),
        AgentSessionError::Conflict { .. } => failure(
            lease,
            "agent_execution_conflict",
            "The Agent run changed during execution",
        ),
        AgentSessionError::InvalidInput { .. }
        | AgentSessionError::SessionNotFound
        | AgentSessionError::RunNotFound
        | AgentSessionError::Storage(_) => failure(
            lease,
            "agent_execution_unavailable",
            "Agent execution is temporarily unavailable",
        ),
    }
}

fn usage_failure(lease: &RunLease, error: AgentUsageError) -> AgentExecutionFailure {
    match error {
        AgentUsageError::Denied { .. } => failure(
            lease,
            "agent_usage_limit_denied",
            "This Agent operation exceeds a configured usage limit",
        ),
        AgentUsageError::MissingDemand | AgentUsageError::CurrencyMismatch => failure(
            lease,
            "agent_usage_limit_not_ready",
            "A configured Agent usage limit cannot be enforced",
        ),
        AgentUsageError::Invalid { .. }
        | AgentUsageError::IdempotencyConflict
        | AgentUsageError::NotFound
        | AgentUsageError::InvalidTransition
        | AgentUsageError::Storage => failure(
            lease,
            "agent_usage_unavailable",
            "Agent usage checks are temporarily unavailable",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_is_only_explicit_backpressure_or_unavailability() {
        assert!(fallback_eligible(ProviderUpstreamFailure::RateLimited));
        assert!(fallback_eligible(ProviderUpstreamFailure::Unavailable));
        for failure in [
            ProviderUpstreamFailure::Authentication,
            ProviderUpstreamFailure::Timeout,
            ProviderUpstreamFailure::Network,
            ProviderUpstreamFailure::InvalidResponse,
            ProviderUpstreamFailure::Unsupported,
        ] {
            assert!(!fallback_eligible(failure));
        }
    }

    #[test]
    fn call_identity_is_stable_and_scoped() {
        let run_id = Uuid::new_v4();
        let first = deterministic_call_id(run_id, 1, 1, "call-1");
        assert_eq!(first, deterministic_call_id(run_id, 1, 1, "call-1"));
        assert_ne!(first, deterministic_call_id(run_id, 1, 2, "call-1"));
        assert_ne!(first, deterministic_call_id(run_id, 2, 1, "call-1"));
        assert_ne!(first, deterministic_call_id(run_id, 1, 1, "call-2"));
    }

    #[test]
    fn provider_keys_and_task_ranking_are_exhaustive() {
        assert_eq!(agent_provider_key("openai"), Ok(AgentProviderKey::OpenAi));
        assert!(agent_provider_key("raw").is_err());
        assert_eq!(
            provider_tool_task_class(TaskClass::CampusConversation),
            ProviderToolTaskClass::CampusConversationSearch,
        );
        assert_eq!(
            provider_tool_task_class(TaskClass::ApprovedOperationalAction),
            ProviderToolTaskClass::ApprovedOperationalAction,
        );
    }

    #[test]
    fn optional_cached_token_parts_preserve_unknown_and_sum_known() {
        assert_eq!(combine_optional(None, None).unwrap(), None);
        assert_eq!(combine_optional(Some(2), None).unwrap(), Some(2));
        assert_eq!(combine_optional(None, Some(3)).unwrap(), Some(3));
        assert_eq!(combine_optional(Some(2), Some(3)).unwrap(), Some(5));
        assert!(combine_optional(Some(u64::MAX), Some(1)).is_err());
    }

    #[test]
    fn preflight_fingerprint_binds_every_execution_dimension() {
        let tenant = Uuid::new_v4();
        let run = Uuid::new_v4();
        let target = Uuid::new_v4();
        let first = provider_preflight_fingerprint(tenant, run, 1, 1, target);
        assert_ne!(first, [0; 32]);
        assert_eq!(
            first,
            provider_preflight_fingerprint(tenant, run, 1, 1, target)
        );
        assert_ne!(
            first,
            provider_preflight_fingerprint(tenant, run, 1, 2, target)
        );
    }

    #[test]
    fn fallback_rebuilds_the_same_conversation_instead_of_consuming_it() {
        let conversation = vec![ConversationMessage::User("List vehicles".to_owned())];
        assert_eq!(materialize_messages(&conversation).unwrap().len(), 1);
        assert_eq!(materialize_messages(&conversation).unwrap().len(), 1);
        assert_eq!(conversation.len(), 1);
    }

    #[test]
    fn capability_declaration_is_a_fail_closed_provider_data_upper_bound() {
        use cp_common::ProviderDataClass::{CampusApproved, LocalOnly, SensitiveDataApproved};

        assert_eq!(
            tighten_required_data_class(CampusApproved, SensitiveDataApproved),
            SensitiveDataApproved
        );
        assert_eq!(
            tighten_required_data_class(SensitiveDataApproved, LocalOnly),
            LocalOnly
        );
        assert_eq!(
            tighten_required_data_class(LocalOnly, CampusApproved),
            LocalOnly
        );
    }

    #[test]
    fn send_time_policy_drift_remains_preflight_not_upstream() {
        assert_eq!(
            map_send_preflight_error(&ProviderExecutionError::StaleCredential),
            Some(ProviderPreflightFailure::StaleCredential)
        );
        assert_eq!(
            map_send_preflight_error(&ProviderExecutionError::ProviderDataApprovalChanged),
            Some(ProviderPreflightFailure::ProviderDataApprovalChanged)
        );
    }

    #[test]
    fn cancellation_disposition_prevents_the_supervisor_from_failing_a_cancelled_run() {
        let lease = RunLease::parse(Uuid::new_v4(), "worker-a", Uuid::new_v4(), 1).unwrap();
        assert_eq!(
            cancellation_failure(&lease, "cancelled", "Cancelled", true).disposition,
            AgentExecutionFailureDisposition::CancellationAcknowledged
        );
        assert_eq!(
            cancellation_failure(&lease, "cancelled", "Cancelled", false).disposition,
            AgentExecutionFailureDisposition::CancellationPending
        );
    }
}
