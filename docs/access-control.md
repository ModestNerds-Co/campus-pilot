# Campus Pilot Access and Module Model

This is the canonical product and engineering reference for sign-in, roles, permissions, modules, and licensing.

## 1. Sign-in and landing

- Everyone uses the same sign-in screen.
- A successful sign-in opens the campus module launcher at `/home`; it never opens Administration directly.
- The launcher shows only modules that are both enabled for the campus and available to the signed-in user.
- Administration is a module, not the application shell. Its navigation contains administration concerns only.
- A future platform operator console is separate from campus Administration. Platform operators do not inherit access to school operational records.

## 2. Roles and permissions

- A user may have multiple roles.
- A role has an immutable, tenant-scoped `key` used by assignments and an editable `name` shown to people.
- Seeded roles and the core Home and Administration module records are provisioned atomically when a campus tenant is created. Their names, descriptions, and permissions may be edited, but seeded roles cannot be deleted.
- Custom roles may be created and deleted dynamically.
- `roles:create`, `roles:edit`, and `roles:assign` are explicit access-administration authorities. A School Administrator may create ordinary catalog-based roles and assign any non-wildcard seeded or custom role even when that role grants operational permissions the administrator does not personally use.
- Only a Campus Owner may create, edit, or assign a role containing `*`. The Campus Owner account cannot be edited, deactivated, or deleted from the user directory, and no operator may manage their own account there.
- An assigned custom role must be removed from every user before deletion. Role names are unique per campus without regard to case, while immutable role keys preserve assignments across renames.
- User and role drawers mirror these rules by hiding unavailable actions, but every mutation is authorized again by the API. Deactivation and deletion revoke refresh sessions; access tokens are rejected immediately because every request reloads the active user record.
- Authorization uses stable permission keys controlled by the application, never labels, URL paths, or substring matching.
- A permission is `<namespace>:<action>`, for example `users:view`, `fleet:create`, or `timetabling:manage`.
- `*` grants all permissions inside modules enabled for the campus. It does not bypass licensing.
- Fine-grained record scope such as `self`, `assigned classes`, or `department` belongs in policy/data-scope rules; it must not be simulated by renaming CRUD permissions.

The initial seeded campus roles are:

- Campus Owner — full access to every enabled campus module.
- School Administrator — manages people, access, configuration, and licensing without becoming a platform operator.
- Teacher — teaching, assigned learner, timetable, library, and communication access.
- Student — self-service learning, timetable, fee, library, and communication access.
- Registrar — admissions, enrolment, and learner-record access.
- Finance Officer — finance, fees, procurement, and reporting access.
- Librarian — library operations and learner lookup.
- Staff Member — employee self-service, timetable, communication, and library access.

## 3. Module registry

The application owns one module catalog. Each entry has a stable key, label, group, route, permission namespace, description, and availability state.

- Core modules (`home`, `administration`) are enabled during campus setup and do not require a commercial key.
- Licensed modules are enabled by an installation-bound signed entitlement lease scoped to a campus and explicit module keys.
- The API exchanges a one-time activation code, verifies the signed lease, stores its immutable claims and fingerprint plus a normalized current projection, and never persists the submitted activation code.
- Expired, revoked, or disabled modules disappear from the launcher and are rejected by the API even when a role contains matching permissions.
- Existing installations are grandfathered during the access-model migration so working modules are not silently removed before a renewable lease is connected. The first accepted signed lease becomes authoritative in the same transaction: granted legacy rows become lease-managed, omitted legacy rows are revoked, deliberate local-disable choices on still-granted modules survive, and core Home/Administration rows remain outside commercial takeover. New campuses start with core modules and require entitlements for licensed modules.

Initial school module catalog:

- Agent
- People and admissions
- Academics
- Timetabling
- Communication
- Finance
- Fees and billing
- Library
- HR and payroll
- Procurement
- Fleet
- Hostel
- Health services
- Assets and inventory
- Document registry
- Internal audit

The final three adopt useful LADS concepts in school language. LADS route-string authorization and unfiltered module walls are deliberately not carried forward.

### Commercial licensing and control plane

Commercial licensing is split across two deployables:

- **Campus Pilot control plane** is a separate vendor-operated Cloudflare service. It owns customer accounts, plans, prices, subscriptions, payments, installations, entitlement issuance, renewal, revocation, signing-key rotation, and vendor audit. It never receives learner, guardian, staff, health, payroll, finance, or other school operational records.
- **Campus entitlement runtime** remains inside each Campus Pilot installation. It stores and verifies signed leases, materializes effective entitlements, applies local module enablement, evaluates dependencies and limits, and continues operating without a synchronous control-plane call.

The control plane provides two distinct authenticated workspaces:

- The customer portal lets an authorized campus customer compare plans, purchase or renew a subscription, manage billing, register installations, create one-time activation codes, and download an offline license bundle.
- The owner portal is for platform operators only. It shows customers, subscriptions, payment state, installations, issued leases, entitlement changes, revocations, signing-key state, and append-only operator audit. Platform access never implies access to a school's operational application or data.

Assigning a subscription does not itself connect a campus server. A customer administrator creates a short-lived, one-time activation code under Customer portal → Installations. A Campus Owner enters that code under Campus Pilot Administration → Licensing. The campus server exchanges it for a signed lease and encrypted renewable installation credential. The owner portal may inspect or revoke the resulting installation, but it does not impersonate the customer or issue the customer's activation code.

Hosted payment checkout and billing management are used so Campus Pilot and the control plane do not collect or store card details. Payment-provider webhooks are signature-verified and idempotent. A client redirect or checkout success page is never authority to enable a module.

Billing is provider-neutral and multi-currency from the first contract. A plan may have several provider-specific price mappings in currencies such as ZWG, USD, or ZAR. Every money value uses an ISO 4217 currency code, an explicit currency exponent, and integer minor units; code never assumes two decimal places. Original, settlement, fee, and refund money remain distinct, and no report silently adds or converts unlike currencies. Stripe, PayPal, Paynow, Pesepay, and future providers integrate through isolated adapters rather than provider-named commercial columns.

### Signed entitlement lease v1

The canonical wire contract is versioned as `cp-license/v1` and carried in an Ed25519-signed JWS. The protected header includes `alg`, `typ`, and `kid`. Claims include:

- issuer, audience, tenant ID, installation ID, lease ID, catalog version, and monotonic lease sequence;
- issue time, not-before time, refresh-after time, active lease deadline, grace deadline, and final token expiry;
- entitled module, feature, capability, and meter keys;
- limits with unit, period, value, and enforcement mode;
- optional minimum and maximum supported Campus Pilot versions.

The signing private key exists only in the control plane. Campus installations select an exact trusted public key by the signed `kid` header and reject unknown identifiers before signature verification. `LICENSE_PUBLIC_KEY_BASE64` plus `LICENSE_PUBLIC_KEY_ID` supplies the current key; `LICENSE_TRUSTED_PUBLIC_KEYS_JSON` supplies an optional identifier-to-base64 keyring for overlap. Rotation provisions the new public key to campuses before the control plane begins signing with it, keeps both old and new keys trusted through the maximum old lease/offline/grace lifetime, then removes the old key only after evidence shows it is no longer in use. The public `/api/v1/keys` response aids provisioning but is never fetched synchronously by an operational authorization decision. Activation credentials and installation renewal credentials are write-only secrets and are never logged, returned after creation, or stored in plaintext.

The local decision is an intersection, never a union:

```text
trusted installation and signed lease
AND lease state permits the requested operation
AND module or feature is commercially entitled
AND the module is locally enabled
AND declared dependencies are satisfied
AND the person's exact permission and record scope allow it
AND policy, approval, and quota checks allow it
```

The pure operation evaluator returns stable reason codes for module, feature, local enablement, dependency, lease lifecycle, application version, permission, record scope, quota, and approval decisions. Operation catalog version 2 assigns stable, exact descriptors to the 43 currently implemented authenticated campus operations across licensing, campus settings, roles, users, fleet, vehicle logs, and timetabling. Actix route-pattern tests verify that every descriptor resolves to its intended route, and catalog tests reject duplicate keys, duplicate route identities, unknown modules, and unknown permissions. Catalogued permission-authoritative routes enforce the complete evaluator decision at the API boundary instead of inferring access from a URL namespace and HTTP method. This includes lease lifecycle and module state, corrects Vehicle Logs ownership under Fleet, keeps licensing repair under `licensing:edit`, and returns a generic denial without exposing commercial or authorization internals. Only launcher catalog and tenant-module discovery are authentication-only because every signed-in role needs them before entering a module. Licensing status and school settings are permission-authoritative. Agent discovery hides unlicensed capabilities, but every Agent execution evaluates licensing, permissions, scope, policy, approval, and quota again.

Each accepted signed lease is stored as immutable evidence and, in the same database transaction, replaces the tenant's normalized current entitlement projection. The projection records the source lease, catalog version, feature grants, capability limits, and minimum or maximum supported Campus Pilot versions. Runtime authorization computes application-version compatibility against the currently running binary instead of persisting a boolean that could become stale after an upgrade. Module dependency requirements remain versioned, code-owned operation-catalog rules and are evaluated against the projected module grants. Installations with a lease accepted before migration 009 temporarily read equivalent claims from that trusted lease until the next successful refresh creates the normalized projection. Grandfathered module rows remain usable only until the first signed lease; that lease atomically converts granted legacy rows to lease-managed rows and revokes any legacy module it does not grant. Hard-limit definitions are projected now; usage reservations and exhaustion meters remain a separate enforcement milestone and must not be inferred from a configured limit alone.

Lease lifecycle is `active -> refresh_due -> offline_lease -> grace -> restricted`; `revoked` and `invalid` override every other state. Restricted mode preserves sign-in, read access, backup/export, licensing repair, and explicitly documented safety-critical workflows. It blocks new high-risk writes, financial posting, destructive changes, and external Agent actions. Expiry or revocation never deletes, encrypts, or withholds a campus's own data.

Online installations refresh periodically. Offline installations may import a signed `.cp-license` bundle. A control-plane revocation takes effect on the next successful refresh or when the locally signed lease reaches its bounded validity deadline; the operational request path never depends on control-plane availability.

## 4. Module navigation

- The launcher is a quiet, searchable campus map: recent modules first, then grouped modules. It must not become a dense wall of identical cards.
- Each operational module owns its local navigation and provides a clear “All modules” return path.
- Administration navigation is limited to overview, users, roles and access, licensing, school settings, and the Agent management pages the signed-in administrator may open.
- Unimplemented licensed modules route to an honest setup/coming-soon state; there are no dead links or invented operational data.
- Secondary creation and editing flows use accessible right-side drawers. Centered modals are not used.

## 5. Security boundary

- The server is authoritative for both module enablement and permissions; hiding a launcher tile is never treated as authorization.
- The authenticated request context contains tenant ID, immutable role keys, effective permissions, and a freshly loaded entitlement snapshot. The snapshot distinguishes signed lease state, module state, projected feature grants, application-version compatibility, and future hard-limit exhaustion.
- Permission checks require both an enabled module and a matching permission. Campus Owner wildcard access still respects module enablement.
- License keys are never stored in plaintext or returned by the API.
- Support access to a campus, if added later, must be explicit, time-bound, and auditable.

## 6. Terminology

- Use “Sign in”, “All modules”, “Administration”, “Roles and access”, and “Licensing” consistently.
- “Campus Owner” is the campus-level super administrator. “Platform operator” is reserved for the future cross-campus platform console.
- Use “module” for a major product area and “permission” for an action inside a module.

## 7. Timetable lifecycle

- Timetabling follows `Rules and setup -> Generate draft -> Review -> Publish`.
- A generation run stores the exact configuration snapshot used for the result. Later setup edits never silently rewrite an existing draft or published timetable.
- Class, teacher, room, and teacher-unavailability collisions are hard constraints. Day balance and avoiding repeated same-subject days are quality preferences.
- Generation may return unresolved lessons, but a run with unresolved lessons cannot be published.
- Publication supersedes the prior published run for the same campus; it does not delete historical runs.
- The timetabling configuration is a self-contained scheduling source until Academics supplies equivalent verified records. Future synchronization must preserve snapshot and publication semantics.

## 8. Agent access

- Agent is a licensed module and follows the same login, role, permission, module-enablement, and record-scope rules as every other module.
- Agent has stable module key `agent`, route `/modules/agent`, and permission namespace `agent`.
- Agent capabilities never outrank the signed-in person. Effective access also applies campus Agent policy and any required human approval.
- Every server-owned product operation is classified for Agent as executable, approval-required, human-only, or prohibited. No operation may be left unclassified.
- Provider administration, routing, capability policy, limits, campus-wide usage, and run audit are Administration concerns with distinct permissions.
- Dynamic custom roles may receive Agent permissions and capability policy; seeded role names are not used as authorization shortcuts.
- New-campus School Administrator seeds receive Agent-administration permissions. Existing non-owner roles do not silently gain Agent access during migration.
- The canonical capability, provider, metering, and approval model is `docs/agent-platform.md`.
