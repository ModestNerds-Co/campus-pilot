# Agent Guidelines

## Meta Rules
- **Always update this AGENT.md file when discovering new patterns, rules, or conventions**
- **Always update the Postman collection file when adding or modifying API endpoints**
  - Location: `postman/collections/30029325-1c89067e-99af-49c8-aa89-dd466241d679.json`
  - Include proper folder structure, request bodies, and authorization headers
  - Add test scripts to auto-save tokens for auth endpoints

## Commands
- Build (whole workspace): `cargo build --workspace`
- Test all: `cargo test --workspace`
- Test single: `cargo test test_name`
- Check/lint: `cargo check --workspace`
- Run server: `cargo run -p campus-pilot`
- SQLx prepare (after query changes, from `apis/`): `DATABASE_URL=postgresql://127.0.0.1:5432/campus_pilot cargo sqlx prepare --workspace`
  - **IMPORTANT**: Always run this command (with `--workspace`, so every module crate's queries get checked) after adding or modifying `query!` or `query_as!` macros anywhere in the workspace
  - Commit the `.sqlx/` directory to version control

## Architecture
- Cargo workspace, one crate per ERP module, rooted at `apis/Cargo.toml`:
  - `crates/common` (package `cp-common`) — shared types with zero business logic: `ApiResponse`/`PaginationMeta`, `TenantId`, `Roles`, the `RequirePermission` middleware, `flatten_validation_errors`. Depends on nothing else in the workspace.
  - `crates/platform/audit` (package `cp-audit`) — shared request/correlation context, authenticated actor identity, append-only audit event types, and a transaction-compatible audit writer. Module crates may depend on it without depending on `app`; metadata passed to it must already be reduced and redacted by the owning domain.
  - `crates/agent` (package `cp-agent`) — provider-independent Agent capability descriptors, closed schemas, typed handler registry, proof-bearing broker inputs/scopes, fresh authority and record-scope enforcement, fail-closed actor-aware audit, and the diagnostic module coverage registry. Coverage joins module delivery stage, licensing boundary, workspace route, product operations, Agent classifications, and registered handlers; it never grants access. The executable registry is indexed against `cp-common` product operations and initially accepts only directly exposed read/export handlers; operational modules never depend on `cp-agent`.
  - `crates/app` (package `campus-pilot`, binary + lib) — the platform layer: kernel bootstrap, auth, users, roles, access/licensing, storage, `AuthMiddleware`, `AppState`, `main.rs`. Depends on `cp-common` and every module crate (it's the only crate that mounts everything into one `actix_web::App`).
  - Production Agent adapters for app-owned Administration services live in `crates/app/src/services/agent/`; they call the same typed service or pure catalogue source as HTTP routes and are assembled once into `AppState.agent_capabilities`. Never duplicate route logic or make private HTTP calls from a capability.
  - `crates/modules/<name>` (packages `cp-<name>`) — one ERP module each (`fleet`, `vehicle-log`, `sis`, `academics`, `finance`, `fees`, `hr-payroll`, `procurement`, `library`, `messaging`, `hostel`, `health`). Each follows the same `mod.rs`/`models.rs`/`dtos.rs`/`ops.rs`/`routes.rs` split as `app`'s own services. **Module crates depend on `cp-common` only, never on `app`** — this is what lets `app` depend on all of them without a cycle. A module crate MAY depend on a sibling module crate when there's a genuine domain relationship (e.g. `cp-vehicle-log` depends on `cp-fleet` to validate vehicle/driver IDs and reuse their read structs).
  - Module route handlers take `web::Data<sqlx::PgPool>` (registered as its own `app_data` in `main.rs`, alongside the full `AppState`) and `web::ReqData<TenantId>` — never `AppState` directly, so they stay decoupled from `app`.
- Auth vs. permissions split, mount order matters: `AuthMiddleware` (verifies the JWT, loads the `User` row, resolves effective role permissions and enabled modules, and inserts `User`/`TenantId`/`Roles`/`AccessContext` into request extensions) lives only in `app` and is applied at the OUTER scope when a module is mounted (see `crates/app/src/routes.rs`). `RequirePermission::new("<module>")` lives in `cp-common`, is applied by each module's own `routes()` on its resource scope(s), and derives the required `"<module>:<action>"` permission from the HTTP method (GET→view, POST→create, PUT/PATCH→edit, DELETE→delete) — so one `.wrap()` covers a whole CRUD resource. **When a scope needs both**, `AuthMiddleware` must be the LAST `.wrap()` call (outermost, runs first) so `RequirePermission` sees populated access context; e.g. `.wrap(RequirePermission::new("users")).wrap(AuthMiddleware)`.
- Role assignments store immutable `roles.key` values, never editable display names. Seeded system roles may be renamed and have their permissions changed, but cannot be deleted; custom roles cannot be deleted while assigned.
- A wildcard permission (`*`) grants every action only inside modules currently enabled for the tenant. It never bypasses module licensing.
- The canonical module and permission catalogue lives in `crates/app/src/services/access/catalog.rs`. Validate role permissions against this catalogue; do not add free-form permission strings in UI code or feature modules.
- Signed license activation stores only a key fingerprint and verified entitlement claims. Never persist, log, or return the original license key.
- **Never nest multiple `web::scope("")` (or any identically-patterned scopes) under the same parent scope** to apply different middleware per HTTP method — actix-web only honors the first such nested scope and silently 404s the routes registered in the others. This was a real, previously-shipped bug in `users`/`roles`; the fix was the single `RequirePermission` wrap described above.
- Entry point: `crates/app/src/main.rs` (binary), `crates/app/src/lib.rs` (library)
- Structure per service/module: `models.rs` (data types), `dtos.rs` (request/response types), `ops.rs` (business logic / queries), `routes.rs` (actix handlers + `routes(cfg)`)
- Database: PostgreSQL with SQLX migrations in `migrations/` (still centralized, not per-crate); reserved numbering: 001-003 core, 004-009 tenancy, 010-011 fleet/vehicle-log, 020s SIS, 030s academics, 040s finance, 050s fees, 060s HR/payroll, 070s procurement, 080s library, 090s messaging, 100s hostel, 110s health (see `ROADMAP.md`)
- Multi-tenancy: every ERP table carries `tenant_id`; a single-tenant on-prem install is just a deployment that only ever provisions one tenant (seeded by migration 004, used by kernel bootstrap) — same schema and code path as multi-tenant SaaS, no special-cased mode
- Testing: Integration tests in `crates/app/src/tests/`, unit tests use `#[actix_web::test]`. Each test runtime creates its own SQLx pool; `tests/helpers.rs` serializes idempotent migration passes with a PostgreSQL advisory lock so a fresh migration is safe under the default parallel test runner. Known baseline issues remain: setup and user tests reuse fixed bootstrap/user records in the same database across runs, and the activate/deactivate user tests still call stale HTTP methods.

## Code Style
- File headers: Include copyright header with creation date and author
- Imports: Group std, external crates, then local modules
- Error handling: Use `anyhow::Result` for main functions, custom errors for API responses
- Types: Use `serde` for JSON serialization, `sqlx` types for database
- Naming: snake_case for variables/functions, PascalCase for types/structs
- API responses: Wrap in `ApiResponse<T>` struct for consistency
- Routes: Use attribute macros (`#[get]`, `#[post]`, `#[put]`, `#[delete]`) on handler functions
- Middleware: Apply at scope level, not individual routes (e.g., `.wrap(AuthMiddleware)`)
- Authentication: All protected routes must use `AuthMiddleware` and appropriate `RequirePermission` middleware

## SQL Style (PostgreSQL)
- All SQL keywords should use UPPERCASE (CREATE, SELECT, INSERT, UPDATE, DELETE, etc.)
- All PostgreSQL data types should be UPPERCASE (TEXT, INTEGER, BOOLEAN, UUID, etc.)
- All built-in functions should be UPPERCASE (NOW(), LOWER(), TO_JSONB(), etc.)
- All control flow keywords should be UPPERCASE (BEGIN, END, IF, THEN, ELSE, etc.)
- All trigger/function keywords should be UPPERCASE (RETURNS, LANGUAGE, EXECUTE, etc.)
- Column names and table names should remain lowercase with underscores
- String literals and comments can remain as-is
- All create statements should include IF NOT EXISTS
- All tables should have deleted_at, created_at, and updated_at fields
