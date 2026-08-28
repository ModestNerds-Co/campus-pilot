# Campus Pilot Agent Platform

This is the canonical product and engineering reference for Agent, contextual chat, AI providers, product capabilities, approvals, usage, limits, and auditability.

## 1. Product outcome

- Agent is a licensed, first-class campus module with stable key `agent` and canonical route `/modules/agent`.
- **Session** is the user-facing conversation container. A session contains messages, context references, approvals, and one or more Agent runs. **Run** means one queued or executing response inside a session.
- Its full workspace owns new-session creation, session search and history, resume, rename, archive, sharing, run status, approvals awaiting the person, and personal usage.
- The authenticated application shell exposes one persistent global Agent widget on Home, Administration, and operational modules when Agent is enabled and the signed-in person has `agent:view`. It is anchored to shared shell navigation, not rendered as a draggable chat bubble over operational content.
- The widget opens the right-side contextual drawer. It can start a session, resume the active or most recent session, show a short recent-session list, and display active-run or approval state.
- The contextual drawer is for focused work. It has one scrolling message region and a fixed composer. Complete history, long sessions, approvals, and run inspection open in the full Agent module.
- Provider setup, routing, capability policy, limits, campus-wide usage, and run audit live in Administration. They are not hidden inside the Agent conversation workspace.

“All functionality is available to Agent” has a precise meaning: every server-owned operation that reads or changes campus state has a stable operation key and an explicit Agent exposure classification. Every safe automatable operation is callable through the capability broker. Operations involving credentials, authentication, raw license keys, unrestricted privilege escalation, or unsupported irreversible effects remain human-only or prohibited, but still appear in the coverage registry with a reason. Full coverage means no operation is unclassified; it does not mean a model may execute unsafe operations.

Client-only presentation controls such as theme, local sorting, and opening navigation are not product operations and do not need Agent capabilities.

## 2. CCS foundation and Campus Pilot improvements

Reuse these proven patterns from `/Users/modestnerd/Developer/Projects/ccs`:

- a code-owned, typed tool catalogue with stable names and schemas;
- one domain implementation shared by first-party chat and other Agent transports;
- access derived from the person's current role on every request;
- tenant-scoped encrypted provider credentials and write-only secrets;
- ordered provider/model fallback routes with safe failure categories;
- durable threads, messages, jobs, provider attribution, audit events, and readable run trails;
- a full assistant workspace plus an authenticated-shell entry point;
- worker claiming, timeouts, interruption recovery, and idempotent job transitions.

Campus Pilot must improve on the current CCS implementation:

- capability coverage is per product operation and per module, not a partial central tool list;
- permissions are exact operation permissions, not coarse role scopes or HTTP-method inference;
- Agent is licensed and has its own role permissions;
- provider, routing, policy, usage, limits, and audit administration have distinct permissions;
- conversations run durably outside the request lifecycle;
- approvals are per immutable proposal, not a permanent token-level write flag;
- provider attempts and capability calls have normalized person/module/capability usage and cost events;
- sensitive learner, health, payroll, finance, and staff data has field-level context, trail, audit, and log redaction;
- one correlation ID links the message, run, provider attempts, capability calls, approvals, usage, audit, and result.

## 3. Product operation and capability catalogues

The application owns two related code catalogues:

- `ProductOperationCatalog` contains every server-owned domain operation, including human-only and prohibited operations.
- `AgentCapabilityCatalog` contains the executable subset and the typed handlers the broker may call.

Every mounted API route references a stable operation constant. A product operation is classified as `exposed`, `approval_required`, `human_only`, or `prohibited`. CI fails when an operation has no classification, when a capability key is duplicated, or when a human-only/prohibited entry has no reason.

Current implementation status: all 282 released server-owned operations have a code-owned Agent exposure classification. One hundred and fifteen read operations are exposed, 157 mutations require approval, and ten credential, destructive connection, role, account-deletion, and license-credential operations remain human-only with explicit reasons. This metadata is coverage policy only; it does not make an operation executable until the capability broker and typed capability handler exist.

Each operation and capability definition includes:

- stable operation and capability keys such as `fleet.vehicles.list` or `timetabling.publish`;
- owning module, label, version, and exact required permission keys;
- input and output schemas and the shared typed domain service;
- supported record and data scopes;
- effect: `read`, `propose`, `mutate`, or `external_side_effect`;
- reversibility: `not_applicable`, `reversible`, or `irreversible`;
- data sensitivity: `general`, `personal`, `sensitive`, or `highly_sensitive`;
- approval mode: none, requester confirmation, designated approver, dual control, or prohibited;
- idempotency, retry, resource-version, and stale-data strategy;
- field-level rules for model context, input, result, run trail, audit, and log redaction;
- allowed provider/data-residency classes and usage tags.

HTTP routes and Agent handlers call the same typed domain service. Agent code does not click the UI, construct private HTTP requests, issue arbitrary SQL, or bypass module transactions and validation.

## 4. Login, roles, modules, and permissions

- Everyone uses the existing sign-in and lands on `/home`.
- Agent has no separate login and no privileged service identity for interactive use.
- The launcher shows Agent only when the campus license enables it and the person has `agent:view`.
- Every request and every capability call reloads the tenant, person, current roles, effective permissions, enabled modules, record scope, Agent policy, and limits.
- A stored role or policy snapshot exists for reporting only; it is never authority for later execution.
- Custom roles receive Agent permissions and capability policy in the same dynamic role editor as seeded roles.
- A wildcard permission still respects licensing, record scope, policy, approvals, provider eligibility, and limits.

Agent permissions:

- `agent:view` — open the Agent module and contextual drawer.
- `agent:run` — submit runs and use capabilities allowed by the underlying permissions and policy.
- `agent:history` — view, search, resume, rename, and archive the person's own sessions and runs.
- `agent:share` — share a session with explicitly selected people.
- `agent:approve` — approve an eligible proposal when the approver also holds the underlying operation permission and record scope.

Administration permissions:

- `ai_providers:view` / `ai_providers:edit`;
- `ai_routing:view` / `ai_routing:edit`;
- `agent_policy:view` / `agent_policy:edit`;
- `agent_usage:view` / `agent_usage:export`;
- `agent_limits:view` / `agent_limits:edit`;
- `agent_audit:view`.

Campus Owner retains wildcard behaviour. Built-in School Administrator roles receive AI-provider and routing Administration permissions only; migration 084 backfills the four `ai_providers:*` and `ai_routing:*` permissions for existing system roles and leaves future-tenant provisioning intact. That backfill grants no `agent:view`, `agent:run`, `agent:history`, `agent:share`, or `agent:approve` permission and never creates an Agent module entitlement. Other existing non-owner roles must not silently gain Agent access; an administrator deliberately assigns it to seeded or custom roles.

## 5. Capability broker

The capability broker is the only Agent execution boundary. First-party chat, the contextual drawer, background runs, and any future MCP/API adapter all call the same broker and catalogue.

For every call the broker:

1. Resolves the authenticated tenant and person and reloads current access.
2. Confirms that Agent and the capability's owning module are enabled.
3. Resolves the exact capability version and validates structured input.
4. Applies record scope, sensitivity, campus policy, provider eligibility, approvals, and limits.
5. Creates the correlation, capability-call, usage, and audit records.
6. Invokes the shared typed domain service in its normal transaction boundary.
7. Records the redacted result, affected resources, duration, outcome, and final usage.

The broker rejects tenant or user IDs supplied by a model. Resource identifiers are always resolved again under the authenticated tenant and person.

Current implementation status: `cp-agent` provides the typed, provider-independent broker boundary. It owns versioned capability descriptors and closed input/output schemas, a registry indexed against the complete product-operation catalogue, authenticated principal and proof-bearing scope types, typed handler adapters, fresh authority and record-scope checks, reserved identity-field rejection, and a fail-closed `cp-audit` sink. The application assembles directly exposed reads over the same typed domain operations as the normal API. SIS includes a tenant-wide, General-sensitivity learner-numbering policy handler that returns only prefix, padding, next sequence, rendered preview, exhaustion status, and optimistic version; the approval-required policy mutation is not registered for execution. Procurement exposes active Finance currencies, minimum-field HR requester candidates, suppliers, requisitions, purchase orders, and goods receipts. Supplier, requisition, purchase-order, and receipt data is sensitive; those adapters remove HR login links and internal mutation-actor identifiers before results can enter model context. Purchase-order reads retain immutable requisition, requester, supplier, currency, and line snapshots plus receiving state. Receipt reads retain the Procurement-owned delivery record with its requisition, supplier, currency, and order-line snapshots; they do not expose a Finance posting because receiving creates none. Finance reads expose currency, chart-of-account structure, fiscal years, accounting periods, journal lists, journal records, journal validation, and module posting requests; journal and posting-request data is sensitive, and balances or statements are not yet exposed. Fees exposes learner candidates, billing accounts, fee structures, invoices, staged billing-account imports, and their Academics and Finance reference data. Learner candidates and import metadata are personal; billing-account, invoice, and normalized import-preview data is sensitive; and the adapter reloads current access so non-administrative reads remain restricted to learner records linked to the caller's login. HR employment and import reads are personal; availability and import-preview reads are sensitive, and the separate typed scheduling reference omits notes and decision metadata. The HTTP licensing route and Agent adapter share one safe licensing read model, so persisted installation and signed-lease interpretation cannot drift. Academics capabilities resolve teacher identity through HR-owned employee operations; term, grade-level, and assessment-cycle reads expose general academic structure, while assessment-component reads are personal because they resolve the assigned teacher. Timetabling capabilities resolve canonical academic records through Academics-owned operations, bind generated snapshots to the active term, and expose run-history reads classified as personal; and SIS capabilities resolve academic placements through Academics while keeping learner, guardian, and import-preview data explicitly classified as personal or sensitive. SIS, HR, and Fees import capabilities expose bounded metadata and normalized preview reads but never retained source bytes or unmapped source columns; upload, mapping/preview creation, and commit remain approval-required mutations. List inputs use bounded typed pagination/filter enums; record reads declare proof-bearing resource scopes; personal data is classified explicitly; and reserved authenticated identity fields cannot be supplied by model input. Catalogue, dependency, authority, and invalid-input paths are verified to fail safely. The first release deliberately registers only directly exposed read/export operations with no approval requirement; approval-required mutations, including every purchase-order or receiving state change, cannot be registered until immutable proposals and approval execution exist. The registry is held in application state. Provider connections and ordered route administration are available, but no provider execution runtime, durable session worker, or chat UI is enabled yet.

Assets and inventory exposes eight typed reads: item and store lists/records plus stock-balance list, stock-movement list/read, and the goods-receipt-allocation source list. The Agent handlers return reduced projections that retain generated references, exact scaled quantities, movement provenance, and the minimum allocation-source fields while omitting internal actor identifiers and unrelated Procurement data. Item/store create, update, and delete plus manual receipt, issue, transfer, adjustment, reversal, and goods-receipt allocation are 12 approval-required mutations. None is registered for direct execution until the immutable proposal and approval broker exists; a model cannot write the stock ledger through a read handler.

The current executable registry contains 115 reads: 16 Administration, 13 HR and payroll, seven Fleet, 19 Academics, four Timetabling, 15 SIS, 12 Finance, 11 Fees, ten Procurement, and eight Assets and inventory. The four AI-provider Administration reads expose the provider catalogue, reduced connection health, and cached models; they omit credentials, fingerprints, configuration attribution, wrapping metadata, and provider response bodies. The four routing reads list and inspect reduced route state, expose safe route-configuration choices, or resolve a route fail-closed; no provider credential or internal model-snapshot identifier is returned. Capability-aware resolution derives the canonical module and operation effect from the registered executable capability, rejects contradictory selectors, and preserves the correct module-route fallback when the caller supplies only a capability.

An optional remote MCP/API adapter may be added later. It is disabled by default and uses expiring, revocable, hashed personal tokens with per-capability allowlists. Token access is intersected with the person's current roles on every call; a token never becomes a service superuser.

## 6. Conversation and contextual execution

### Sessions, runs, and history

- The product calls the durable conversation container a **Session**. The internal persistence model may retain the `agent_threads` name, but the UI does not alternate between “thread”, “chat”, and “session”.
- A session belongs to one campus and one owner, with explicit membership for shared sessions. It retains its title, active or archived state, creation and last-activity times, message history, context references, approvals, and runs.
- A run is one execution caused by a submitted message. A session may contain many completed runs, but only one run may be `queued`, `running`, or `awaiting_approval` in that session at a time. Other sessions may run independently.
- Session history is durable across navigation, sign-out, browser reload, and supported devices. The full Agent module can search and filter the person's own and explicitly shared sessions by title or content, module context, status, participant, and date.
- People can start, resume, rename, and archive sessions. Archive removes a session from the default list without destroying its audit, usage, approval, or run records. Permanent deletion, if later supported, follows campus retention policy rather than behaving like casual chat deletion.
- The global widget shows only the active session and a short recent list. Full history and management remain in the Agent module.
- Navigating to another module or record never silently adds that context to the active session. The drawer shows the available context as a chip and requires the person to attach, replace, or dismiss it. Starting a new contextual session may attach the visible context explicitly.

### Global widget and contextual drawer

The widget is a shared authenticated-shell control, so it is available from Home, Administration, and operational modules without duplicating Agent state in each module. Its collapsed state may show queued, running, or approval attention, but it must not obscure operational controls or render conversation content over the page.

The contextual drawer sends only:

- `originModuleKey`;
- the current route;
- an optional allowlisted `{ recordType, recordId }`;
- an optional display label.

The browser never sends DOM text or a trusted record snapshot as context. The server rehydrates current record context through an authorized read capability. A removable context chip such as “Fleet · Vehicle ABC 123” shows exactly what will be supplied.

Every submitted message atomically creates the user message, an `agent_run`, a queue/lease record, and one correlation ID. A worker executes the run; the client follows durable state through SSE or polling. Runs support `queued`, `running`, `awaiting_approval`, `completed`, `failed`, `cancelled`, and `interrupted` states.

Workers use leases and heartbeats, accept cancel requests, and resume only from idempotent checkpoints. Non-idempotent external actions are never replayed automatically. Progress events are redacted and orphan messages/runs have an explicit recovery path.

Documents and records are untrusted data, not instructions. Tool results are schema-bound, size-bounded, and redacted before they enter model context.

## 7. Proposals and approvals

Read, propose, mutate, external side effect, reversible, irreversible, sensitive, and approval-required are independent policy dimensions. A permission to view a module never implies permission to change it through Agent.

An approval stores:

- exact capability key and version;
- canonical validated input and redacted preview;
- proposal hash;
- affected resource IDs and versions;
- requester, approver rules, expiry, and policy snapshot;
- decision, decision reason, and execution result.

Approval executes the saved proposal through the broker; the model does not reconstruct it. Immediately before execution, the broker reloads authorization, licensing, policy, limits, and resource versions. Changed source data marks the proposal stale and requires a new preview.

Provider credential entry, OAuth completion, API-key rotation, password changes, raw license-key entry, and unrestricted role escalation remain direct human workflows. Agent may read safe connection status, test a connection, or propose a route change for an authorized administrator, but secrets never enter messages or model-visible capability input/output.

## 8. Administration information architecture

Administration gets an `Agent management` navigation group with full pages for:

- **Overview** — provider readiness, recent run health, approvals, spend/usage summary, and policy alerts.
- **AI providers** — connections, account labels, status, last test/use, reconnect, rotate, and disconnect.
- **Routing** — task defaults, module/capability overrides, provider/model order, feature and sensitivity eligibility.
- **Capabilities and approvals** — the complete operation coverage matrix, policy, risk, approval mode, and availability by module.
- **Usage and limits** — filters, trends, budgets, limits, and export.
- **Runs and audit** — redacted step trails, provider attempts, capability calls, approvals, outcomes, and correlation search.

Usage tables, capability matrices, and run trails are full pages because they require comparison and sustained scrolling. Right-side drawers are reserved for focused connect, reconnect, route edit, policy edit, limit edit, approval decision, and disconnect workflows. Drawer header/footer remain fixed while their single content region scrolls.

The full Agent module shows personal usage only. Campus-wide administration is never mixed into an ordinary conversation.

## 9. Provider connections and routing

Current implementation status: migration 078 and the `cp-ai-providers` platform crate implement tenant-scoped OpenAI, Anthropic, and OpenRouter API-key connections. Administration can list supported providers; create, read, rename, test, rotate, and safely disconnect multiple connections; and read or refresh immutable model snapshots. Every mutation and provider test or refresh outcome writes reduced actor-aware audit evidence. Migration 083 and `cp-agent-runtime` add versioned provider/model route sets for tenant default, task class, module/operation, and exact capability scopes. A route contains one to three ordered targets, binds each target to a ready connection and its current immutable model snapshot, optionally requires confirmed tool support, and replaces or archives under optimistic version checks with actor-aware audit evidence. Resolution uses capability, module/operation, task class, then tenant-default precedence. A matched but unusable route fails closed and never falls through to a broader route. Provider attempts, normalized usage, and durable sessions remain part of the execution-worker release; they are not inferred from provider administration tests.

- A campus may configure multiple connections for the same provider.
- Support API-key and provider-supported OAuth/device-flow connections behind server-side adapters.
- Connections store tenant, provider, authentication method, account label, status, encrypted credential, fingerprint, configured-by person, credential version, last tested/use, safe failure category, and timestamps.
- Secrets require an application encryption key, are encrypted at rest, are write-only, and are never returned after save.
- Refreshed OAuth credentials are persisted with optimistic concurrency so a stale run cannot overwrite a newer credential.
- Model/provider catalogues are versioned and server-owned or safely refreshed. The server validates provider, model, reasoning mode, tool support, context limits, pricing version, and route eligibility on save and again at execution.

Provider credentials are app-managed write-only secrets. Deployment supplies `AI_PROVIDER_CREDENTIAL_KEYS_JSON`, a JSON key-ID to standard-base64 32-byte AES key map, and `AI_PROVIDER_CREDENTIAL_ACTIVE_KEY_ID`. AES-256-GCM uses a fresh nonce and versioned associated data binding the tenant, connection, provider, authentication method, and credential version. Old wrapping keys remain in the keyring until all active credentials using them have been rotated. When both variables are absent the service starts only while there are no active stored provider credentials; catalogue and empty-state reads remain available, while credential creation, rotation, testing, and model refresh return an explicit configuration error. Once active credentials exist, startup requires a keyring containing every stored key identifier, including retired rotation keys. Supplying only one variable, an invalid keyring, or a keyring that cannot decrypt the active credential set prevents startup. This keyring is distinct from license-signing and installation-credential keys.

Routing precedence is:

1. capability-specific override;
2. module and operation-class override;
3. task-class route;
4. tenant default.

Initial task classes are campus conversation/search, module read/reporting, document extraction, drafting/proposal, and approved operational action.

Route selection considers task class, sensitivity, module/capability policy, provider eligibility, health, required tool features, remaining budget, and ordered fallback. Only transient/provider failures may fall through. Policy denial, invalid input, stale proposal, approval rejection, or hard-limit denial stops immediately without contacting another provider. Every provider attempt is separately metered; raw upstream error bodies are never persisted.

## 10. Usage, cost, reporting, and limits

Usage distinguishes:

- `origin_module_key` — where the person opened Agent;
- `capability_module_key` — the module that owns the called capability.

This supports both “Agent usage from Fleet” and “usage of Fleet capabilities.”

Every provider attempt records nullable normalized values for input, output, cached, and reasoning tokens; provider-reported cost/currency; independently estimated cost/currency/pricing version; provider connection, provider, model, route priority, task class, duration, retries, failure category, and outcome. Unknown values remain `NULL`, never zero.

Every capability call records person, role snapshot, origin module, capability module, capability key/version, approval state, duration, outcome, affected resource references, run, thread, and correlation IDs.

Administration reporting supports usage per person, per module, and per capability, with totals, trends, and filters by:

- person and role snapshot;
- originating module and capability module;
- capability, provider connection, provider, model, and task class;
- success, failure, retry, cancellation, approval rejection, and limit rejection;
- date range and campus.

Export uses the exact active filters. Provider-reported, estimated, and unknown cost are visually distinct.

Limits may apply per campus, person, role, origin module, capability module, capability, provider, model, and reporting period. Any deny wins. A specific rule may tighten but never override a broader deny. Hard limits use transactional reservations/counters so concurrent runs cannot overspend. A rejected run records usage and audit events without calling a provider.

## 11. Data and audit boundaries

Keep these concerns separate:

- `ai_provider_connections`, OAuth/device-flow attempts, and provider model snapshots;
- `ai_task_routes`;
- code-owned product operation and Agent capability catalogues;
- `agent_policy_rules`, `agent_limits`, and transactional limit reservations/counters;
- `agent_threads` (presented as Sessions), explicit session membership, and user-visible messages;
- `agent_runs`, run steps, and capability calls;
- `agent_approvals`;
- append-only `agent_usage_events`;
- actor-aware, append-only audit events for consequential state changes.

The current trigger-based `event_log` records table changes but lacks the actor, request, approval, and Agent-run linkage required here. The first-class `actor_audit_events` ledger is the Agent audit boundary; remaining domain mutations must adopt it operation by operation before their Agent write capabilities ship. The legacy table remains table-change evidence during that adoption.

Current implementation status: the shared `cp-audit` platform crate defines server-owned request/correlation context, person/Agent/system actor identity, outcomes, targets, reduced redacted metadata, and a writer that can append through the same SQL transaction as a domain change. Every API response carries a fresh `x-request-id`; a valid incoming `x-correlation-id` is propagated, otherwise the request ID starts the correlation. Authenticated requests receive a person actor context. Migration 016 creates the indexed, append-only `actor_audit_events` ledger while retaining the legacy trigger log. Existing domain mutations still need to adopt the writer operation by operation; Agent execution is not enabled by this foundation.

Provider credentials never share tables with messages or usage. Usage is not the audit record, and a run trail is not the usage ledger.

The current generic object-storage route is unauthenticated and configures a public-read bucket. Agent must not use it for prompts, attachments, import sources, capability results, or generated documents. The initial runtime is text and typed-context-reference only. Attachments require a separate private, tenant-scoped, person-authorized object boundary with malware/content checks, retention controls, and broker-mediated reads.

## 12. Module capability coverage

The code catalogue contains the exact operation list. This planning map defines the capability families that must be inventoried; it is not permission to invent unavailable features.

Current implementation status: `cp-agent::ModuleCoverageRegistry` joins every catalogued module's delivery stage, core/licensed boundary, workspace route, routed product operations, Agent exposure counts, registered executable capabilities, and missing directly exposed handlers. The application catalogue test proves all 17 modules are represented and aligned. Administration exposes 16 executable reads; HR and payroll exposes 13; Fleet exposes seven; Academics exposes 19; Timetabling exposes four; SIS exposes 15; Finance exposes 12; Fees exposes 11; Procurement exposes ten; and Assets and inventory exposes eight. Every directly exposed operation now has a registered typed handler. AI-provider and routing Administration require the licensed Agent module plus exact provider or routing permissions. SIS, HR and payroll, Finance, Fees, Procurement, and Assets and inventory are marked `available` for their released operational slices. Finance exposes currency, chart-of-account, fiscal-year, accounting-period, journal, journal-validation, and module posting-request reads; its 23 mutations remain approval-required. Fees exposes learner billing accounts, versioned fee structures, immutable invoices, and staged billing-account imports; its 13 mutations remain approval-required and all 24 operations require licensed SIS, Academics, and Finance dependencies. Procurement exposes Finance currencies, HR requester candidates, suppliers, requisitions, purchase orders, and goods receipts; its 17 mutations remain approval-required and all 27 operations require licensed HR and payroll and Finance dependencies. Purchase-order issue/cancel use `procurement:approve`, receiving create/update/post use `procurement:receive`, and no Agent-exposed read grants either workflow authority. Assets and inventory has 20 routed operations, eight exposed reads, 12 approval-required writes, and eight registered executable reads. It owns the append-only movement ledger, balance projection, and goods-receipt allocation mapping; Procurement remains the immutable posted-receipt source. Exactly the allocation list and create operations depend on Procurement. Reversal and every other Assets operation remain Assets-only, with no Finance or HR and payroll dependency. Agent projections are reduced, and no Assets write is directly executable before the approval broker exists. Academics adds genuine assessment-cycle and component reads; its six assessment-structure mutations remain approval-required. This registry is diagnostic evidence, not an access grant.

| Module | Capability families | Initial policy emphasis |
| --- | --- | --- |
| Administration | users, roles, licensing, school settings, Agent governance | Secrets stay human-only; access escalation and license changes require strong approval. |
| People and admissions | applications, admissions, learners, guardians, enrolment | Personal data and assigned-record scopes. |
| Academics | subjects, classes, assessment structures, progression, reports | Publish/progression changes require preview and approval. |
| Timetabling | rules, generation, drafts, conflict review, publication | Generation is durable; publication executes an immutable reviewed draft. |
| Communication | drafts, audiences, announcements, sends, delivery history | Sending is an external side effect and approval-gated. |
| Finance | accounts, budgets, journals, controls, statements, reports | Propose before post; dual control where policy requires it. |
| Fees and billing | fee structures, invoices, receipts, balances, statements | Financial writes are idempotent and auditable. |
| Library | catalogue, circulation, reservations, fines, member lookup | Borrower scope and reversible circulation commands. |
| HR and payroll | staff, leave, contracts, payroll preparation and runs | Highly sensitive data; payroll execution needs dual control. |
| Procurement | requests, approvals, suppliers, orders, receiving | Existing approval chain is preserved; Agent cannot self-approve. |
| Fleet | vehicles, drivers, daily logs, trips, maintenance | Driver/vehicle scope, record versions, and reversible updates. |
| Hostel | residences, rooms, allocation, occupancy, pastoral records | Allocation changes need preview; pastoral data is sensitive. |
| Health services | visits, care records, medication, wellbeing follow-up | Highly sensitive; minimum context and strict role/record scope. |
| Assets and inventory | item master, stores, immutable movements, balances, and goods-receipt allocation | Assets owns the ledger, balance projection, and allocation mapping; Procurement owns the posted receipt source. Only allocation list/create depends on Procurement, reversal remains Assets-only, and every mutation requires approval. A later school workflow may turn a department or cost-centre inventory request into approval and store issue; the LADS concept is product insight, not reusable code or authorization. |
| Document registry | filing, classification, retention, retrieval | Documents are untrusted input; retention/destruction is restricted. |
| Internal audit | plans, findings, evidence, remediation | Preserve independence and immutable finding history. |
| Agent | sessions, sharing, runs, approvals, personal usage | No cross-person history without explicit sharing/report permission. |

## 13. Rust workspace integration

- `cp-common` owns the complete product-operation catalogue and shared access/licensing policy primitives; it contains no business logic.
- `cp-imports` owns bounded destination-neutral CSV/XLSX parsing and source fingerprinting. It never owns destination fields, permissions, deduplication, previews, or commits. Agent import reads expose normalized destination previews and issue summaries only; retained source bytes and unmapped source columns remain private.
- Each operational module owns typed domain services, its operation catalogue, and capability adapters beside those services.
- `cp-agent` owns Agent-specific capability descriptors, schemas, execution principal/context, registry, typed broker, redaction metadata, and broker audit adapter. It may depend on operational modules when real adapters are assembled; operational modules never depend on Agent.
- `cp-agent-runtime` owns provider routing first, then durable run orchestration, usage, and limits behind the same broker boundary. Approvals remain a later runtime concern. None belongs in `cp-common` or module domain services.
- A dedicated Agent worker binary/crate claims durable runs and handles recovery.
- The app crate mounts Agent APIs plus Administration provider, routing, policy, usage, limit, and audit APIs. `AuthMiddleware` remains outermost, followed by module, permission, scope, and broker checks.
- Chat and any future MCP/API adapter use the broker; there is no second dispatcher or duplicated business implementation.

## 14. Mandatory coverage and security tests

CI must prove:

- every mounted product operation has one unique stable key and an exposure classification;
- every exposed capability has schemas, module, exact permission, handler, redaction, idempotency, stale-data, and tests;
- every handler calls the same domain service as the normal API/UI path;
- no handler trusts tenant or person identity from model input;
- role revocation, scope change, module disablement, policy change, and limit change apply before the next call;
- credentials and protected learner, staff, health, payroll, and finance fields cannot enter messages, trails, usage, audit metadata, provider errors, or logs;
- fallback occurs only for eligible provider/transient failures;
- every provider attempt and capability call is metered, including failed fallbacks and denied runs;
- concurrent runs cannot exceed a hard limit and report rollups equal immutable usage events;
- unknown tokens/cost remain unknown;
- approval replay is idempotent and stale proposals cannot execute;
- contextual record hydration cannot read a record the person cannot open directly;
- desktop and mobile Agent workspace/global-widget drawer pass focus trap, Escape, focus return, background scroll lock, and single-scroll-region checks;
- session history survives reload and navigation, archived sessions leave default history without losing audit/run records, and module navigation never silently changes a session's attached context.

## 15. Delivery sequence

1. **Operation inventory and broker foundation** — classify every current server operation, add exact operation permissions and CI coverage, introduce actor-aware audit events, and build the broker with no provider execution.
2. **Provider administration and routing** — encrypted connections and model catalogues are complete; migration 083 adds server-validated tenant, task, module/operation, and capability routing. Migration 084 is the narrow forward backfill for existing built-in School Administrator AI-provider/routing Administration permissions; it grants no Agent-use permission or entitlement. Provider attempts and normalized usage ship with the execution worker rather than being fabricated by administration-only connection tests.
3. **Durable read-only Agent module** — migration 085 owns durable Sessions, owner membership, append-only messages, runs, queue leases, redacted events, provider-attempt and capability-call trails, and request idempotency. The licensed `/modules/agent`, history, worker-backed execution, canonical usage ledger, transactional licensing reservations, and genuine authorized reads release only when the worker can complete queued work; do not expose a queue that only accumulates runs.
4. **Global widget and contextual drawer** — one shared-shell widget, recent-session handoff, server-rehydrated module/record context, explicit context chips, full accessibility behaviour, and a clear path to the full Agent workspace.
5. **Governance and reporting** — capability/policy inventory, person/module/capability usage, transactional limits, run/audit inspection, and filtered export.
6. **Proposals, approvals, and executable coverage** — immutable proposals, stale checks, designated/dual approvals, then safe write capability expansion module by module.

Do not ship a generic chat box that implies broad campus control. A released capability must be real, currently authorized, auditable, metered, and accurately represented in the UI. Do not release a module operation without a coverage classification.
