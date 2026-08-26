# Campus Pilot Agent Platform

This is the canonical product and engineering reference for the Agent module, contextual chat, AI providers, module capabilities, approvals, usage, and auditability.

## 1. Product structure

- Agent is a first-class campus module with its own route, navigation, conversation history, run history, and usage view.
- A compact Agent entry point is available from every authenticated module. It opens a right-side contextual drawer; the full Agent workspace remains the destination for long conversations, history, approvals, and administration.
- The contextual drawer carries the current module and record context only when the user deliberately opens it. It has one scrolling message region and a fixed composer; it must not place another scrolling form inside the conversation.
- Agent appears only when the module is enabled for the campus and the signed-in user has `agent:view`.
- AI provider setup, routing, capability policy, budgets, and campus-wide usage reporting live in Administration. They do not live in a provider-specific settings page inside the Agent module.
- Provider credentials, raw secrets, internal prompts, and hidden reasoning are never shown to the Agent or returned to a browser.

## 2. Principles

- The server, not the model or chat UI, owns authorization, validation, transactions, and audit records.
- Agent capabilities call typed domain operations. They do not click the UI, construct private HTTP requests, or bypass module services.
- Every supported product operation is deliberately classified as exposed, approval-required, or prohibited. A coverage test prevents a new API operation from being silently omitted from that classification.
- Effective capability access is always the intersection of campus licensing, the person's current roles and data scope, campus Agent policy, capability risk policy, and any required approval. Agent access never outranks the person using it.
- A wildcard role still respects licensing, record scope, Agent policy, and approval requirements.
- Read, propose, prepare, execute, and irreversible actions are different risk classes. A permission to view a module does not imply permission to execute changes through Agent.
- Consequential work is previewed before execution and produces a clear result or failure. Partial success is never presented as full completion.

## 3. Capability contract

Each module owns a typed capability catalog. Every capability definition includes:

- a stable key such as `fleet.vehicles.list` or `timetabling.draft.generate`;
- module key and human-readable label;
- input and output schemas;
- the existing application permission it requires;
- supported record/data scopes;
- risk class: `read`, `propose`, `write`, `restricted`, or `irreversible`;
- approval mode: none, confirm each run, designated approver, or prohibited;
- idempotency behavior and retry policy;
- redaction rules for input, output, logs, and model context;
- the typed domain handler used to perform the operation.

The capability broker performs this sequence for every call:

1. Resolve the authenticated user, tenant, roles, effective permissions, and record scope.
2. Confirm that the capability and its owning module are enabled for the tenant.
3. Apply campus capability policy, limits, and approval requirements.
4. Validate the structured input before any domain operation runs.
5. Record the call and correlation ID.
6. Execute the same domain operation used by the application.
7. Record the structured result, resource changes, usage, duration, and final status.

Password changes, authentication secrets, provider credentials, license keys, and unrestricted role escalation are prohibited model-visible capabilities. Administration may expose safe operations around them, but secrets are write-only and never enter conversation context or tool output.

## 4. Role and module access

- Everyone signs in through the existing login and lands on `/home`.
- Agent does not have a separate login or a privileged service identity for interactive use.
- A conversation run is attributed to the signed-in person and reevaluates their current roles and module access on every request.
- A person may use multiple assigned roles; Agent receives the same effective permission set as the application.
- Custom roles can receive Agent permissions and specific capability policy just like seeded roles.
- Campus Owners and School Administrators can manage provider connections and campus Agent policy only when they have the corresponding Administration permissions.
- Sensitive record scopes, especially learner, staff, finance, payroll, and health data, are enforced inside domain queries. The model must not receive records the person could not open directly.

Initial Agent permissions:

- `agent:view` — open Agent and use permitted read capabilities.
- `agent:run` — start Agent runs and use non-read capabilities allowed by policy.
- `agent:approve` — approve designated pending actions when the person also has the underlying module permission.
- `agent:history` — view the person's own conversations and runs.
- `ai_providers:view` / `ai_providers:edit` — review or manage provider connections and routing in Administration.
- `agent_policy:view` / `agent_policy:edit` — review or manage capability policy, limits, and approvals.
- `agent_usage:view` — view campus usage according to reporting scope.

## 5. Provider administration and routing

Adopt the useful CCS provider pattern and keep it tenant-scoped:

- Multiple encrypted provider connections may exist for one campus.
- Support subscription/OAuth connections and API-key connections without exposing stored credentials.
- Each connection stores provider, authentication method, account label, status, credential ciphertext, fingerprint, configured-by user, and timestamps.
- Provider secrets require an application encryption key, are encrypted at rest, are redacted from logs, and are never returned after save.
- Task routes define an ordered provider/model chain for a task class. A provider failure may fall through to the next configured route.
- Model IDs, reasoning modes, supported tool features, context limits, and pricing metadata are validated against a server-owned catalog or a safely refreshed provider catalog.
- Connection tests report operational status without echoing credentials or provider response bodies that may contain secrets.

Initial task classes should remain small and explicit:

- campus conversation and search;
- module read and reporting;
- document extraction;
- drafting and proposal generation;
- approved operational actions.

Provider setup is an Administration page with right-side drawers for connect, edit routing, reconnect, and disconnect confirmation. The Agent module reports when no usable route is available but does not expose provider controls to unauthorized users.

## 6. Conversations, runs, and approvals

- Threads belong to a tenant and creator. Sharing is explicit and auditable; it is not implied by role membership.
- Messages store user-visible content, provider/model attribution, timestamps, and the related run. Hidden reasoning is never persisted.
- Every turn creates an Agent run with status, task class, originating module, optional record context, request ID, idempotency key, and timing.
- Every model response and capability call is stored as a step trail with secrets and sensitive fields redacted.
- Long work runs outside the request lifecycle and can be resumed, cancelled, or recovered after worker interruption.
- Approval-required actions store an immutable preview, the exact proposed inputs, expiry, approver rules, and the decision. Approval executes the saved proposal; it does not ask the model to reconstruct it.
- If source data changes after a proposal is created, the server marks it stale and requires a fresh preview.
- Irreversible actions require explicit product authorization and stronger confirmation. They are prohibited by default.

## 7. Usage and cost reporting

Every provider attempt and capability call emits a normalized usage event. Store provider-reported values when available and leave unknown values unknown rather than estimating them as zero.

Usage dimensions:

- tenant, user, role snapshot, and conversation/run;
- originating module and capability key;
- provider connection, provider, model, and task class;
- input, output, cached, and reasoning tokens when reported;
- request count, tool/capability call count, duration, retries, and outcome;
- provider cost and currency when reported;
- catalog-estimated cost, pricing version, and currency when it can be calculated reliably.

Administration reporting must support:

- totals and trends by person, module, capability, provider, model, and task class;
- successful, failed, retried, and approval-rejected runs;
- the ability to open a run's redacted step and audit trail;
- filters by date range and campus;
- export of the currently filtered operational report;
- clear separation between provider-reported cost, estimated cost, and unknown cost.

Policy may define soft alerts or hard limits per tenant, user, role, module, capability, provider, and reporting period. A more specific limit cannot grant access that a broader policy denies. Limit rejections are recorded as usage/audit events without calling a provider.

## 8. Data model boundaries

The implementation should keep these concerns separate:

- `ai_provider_connections` and OAuth/device-flow attempts — encrypted provider access;
- `ai_task_routes` — ordered provider/model configuration per task class;
- `agent_capability_catalog` — code-owned capability definitions exposed through the API;
- `agent_policy_rules` and `agent_limits` — tenant configuration referencing stable capability and module keys;
- `agent_threads` and `agent_messages` — user-visible conversation state;
- `agent_runs`, `agent_run_steps`, and `agent_tool_calls` — durable execution and redacted trails;
- `agent_approvals` — immutable proposals and human decisions;
- `agent_usage_events` — append-only normalized metering;
- the existing audit event system — human, agent, and service accountability.

Provider credentials must not share tables with messages or usage. Usage events must not become the only audit record for state changes.

## 9. Rust workspace integration

- Add Agent as its own module crate. It may aggregate capability definitions from sibling module crates; operational module crates must not depend on Agent.
- Put the shared capability descriptor and execution context contracts in `cp-common` without adding business logic there.
- Each operational module exports its definitions and typed handlers beside its domain operations.
- The Agent broker aggregates those definitions and invokes handlers using the authenticated tenant/user context.
- The app crate mounts the Agent API and applies `AuthMiddleware` outermost, followed by the existing module and permission boundary.
- Provider management remains in the app/Administration layer because it owns tenant configuration, authentication flows, and encrypted secrets.
- Add coverage tests that compare routed product operations with the Agent classification registry and reject unknown capability keys in role/policy configuration.

## 10. Delivery sequence

1. Capability foundation: descriptor contracts, registry, coverage classification, run/audit schema, and no model execution.
2. Provider administration: encrypted connections, connection health, model catalog, task routing, and Administration drawers.
3. Read-only Agent: licensed module, full conversation workspace, contextual drawer, durable threads, and current-module read capabilities.
4. Metering and governance: normalized usage events, person/module/capability reports, limits, and redacted run trails.
5. Proposals and approvals: typed previews, stale-proposal checks, designated approvals, and idempotent execution.
6. Module coverage: add capabilities module by module, with tests proving every product operation is classified.

Do not ship a chat box that can only answer generic questions and imply that it controls the campus. Each released capability must be real, authorized, auditable, metered, and represented accurately in the UI.
