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
- Seeded roles are provisioned when a campus is configured. Their names, descriptions, and permissions may be edited, but seeded roles cannot be deleted.
- Custom roles may be created and deleted dynamically.
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
- Licensed modules are enabled by a signed entitlement key scoped to a campus and one or more module keys.
- The API verifies the entitlement signature and claims, stores only a key fingerprint, and records expiry when present.
- Expired, revoked, or disabled modules disappear from the launcher and are rejected by the API even when a role contains matching permissions.
- Existing installations are grandfathered during the access-model migration so working modules are not silently removed. New campuses start with core modules and require entitlements for licensed modules.

Initial school module catalog:

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

## 4. Module navigation

- The launcher is a quiet, searchable campus map: recent modules first, then grouped modules. It must not become a dense wall of identical cards.
- Each operational module owns its local navigation and provides a clear “All modules” return path.
- Administration navigation is limited to overview, users, roles and access, licensing, and school settings.
- Unimplemented licensed modules route to an honest setup/coming-soon state; there are no dead links or invented operational data.
- Secondary creation and editing flows use accessible right-side drawers. Centered modals are not used.

## 5. Security boundary

- The server is authoritative for both module enablement and permissions; hiding a launcher tile is never treated as authorization.
- The authenticated request context contains tenant ID, immutable role keys, effective permissions, and enabled module keys.
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
