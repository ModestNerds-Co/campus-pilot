//! Declares the versioned operation catalog for implemented API routes.
//!
//! Route matching is exact against Actix's resolved route pattern. The catalog
//! is code-owned so licensing, permissions, Agent capabilities, and audit can
//! share stable operation keys without trusting client-provided identifiers.

use std::sync::OnceLock;

use actix_web::http::Method;

use crate::{AgentExposure, OperationEffect, ProductOperation};

/// Bump this when operation requirements change in a non-additive way.
pub const OPERATION_CATALOG_VERSION: u32 = 2;

/// Product-catalog identifier carried by signed entitlement leases.
///
/// The control plane and campus runtime must agree on this exact value before
/// lease claims can be accepted. Keep it aligned with
/// [`OPERATION_CATALOG_VERSION`].
pub const PRODUCT_CATALOG_VERSION: &str = "campus-pilot/2";

/// Product-catalog versions this campus binary can safely interpret.
///
/// A catalog upgrade may temporarily list both versions during a coordinated
/// rollout; the control plane still issues only [`PRODUCT_CATALOG_VERSION`].
pub const SUPPORTED_PRODUCT_CATALOG_VERSIONS: &[&str] = &[PRODUCT_CATALOG_VERSION];

/// Declares the authoritative access boundary for one routed operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteAuthority {
    /// A current campus identity is sufficient. This is reserved for shared
    /// launcher discovery needed before a user enters a module.
    Authenticated,
    /// The exact operation evaluator is authoritative.
    Permission,
}

/// Associates one resolved API route with its product operation.
#[derive(Debug)]
pub struct RoutedOperation {
    method: Method,
    route_pattern: &'static str,
    operation: ProductOperation,
    authority: RouteAuthority,
}

impl RoutedOperation {
    #[must_use]
    pub fn method(&self) -> &Method {
        &self.method
    }

    #[must_use]
    pub const fn route_pattern(&self) -> &'static str {
        self.route_pattern
    }

    #[must_use]
    pub const fn operation(&self) -> &ProductOperation {
        &self.operation
    }

    #[must_use]
    pub const fn authority(&self) -> RouteAuthority {
        self.authority
    }
}

/// Returns all product operations currently backed by working authenticated routes.
#[must_use]
pub fn operation_catalog() -> &'static [RoutedOperation] {
    static CATALOG: OnceLock<Vec<RoutedOperation>> = OnceLock::new();
    CATALOG.get_or_init(build_catalog)
}

/// Resolves an exact descriptor after Actix has matched the request route.
#[must_use]
pub fn operation_for_route(
    method: &Method,
    route_pattern: &str,
) -> Option<&'static ProductOperation> {
    operation_catalog()
        .iter()
        .find(|entry| entry.method() == method && entry.route_pattern() == route_pattern)
        .map(RoutedOperation::operation)
}

/// Resolves the route entry, including its authoritative access boundary.
#[must_use]
pub fn routed_operation_for_route(
    method: &Method,
    route_pattern: &str,
) -> Option<&'static RoutedOperation> {
    operation_catalog()
        .iter()
        .find(|entry| entry.method() == method && entry.route_pattern() == route_pattern)
}

fn build_catalog() -> Vec<RoutedOperation> {
    vec![
        // Authenticated launcher discovery used by every signed-in role.
        authenticated_route(
            Method::GET,
            "/api/1.0/access/catalog",
            "administration.catalog.read",
            "administration",
            "administration:view",
            OperationEffect::Read,
        ),
        authenticated_route(
            Method::GET,
            "/api/1.0/access/modules",
            "administration.modules.list",
            "administration",
            "administration:view",
            OperationEffect::Read,
        ),
        route(
            Method::GET,
            "/api/1.0/access/licensing",
            "administration.licensing.read",
            "administration",
            "licensing:view",
            OperationEffect::Read,
            false,
        ),
        route(
            Method::GET,
            "/api/1.0/kernel/school-profile",
            "administration.school_settings.read",
            "administration",
            "school_settings:view",
            OperationEffect::Read,
            false,
        ),
        route(
            Method::PUT,
            "/api/1.0/kernel/school-profile",
            "administration.school_settings.update",
            "administration",
            "school_settings:edit",
            OperationEffect::Write,
            false,
        ),
        route(
            Method::POST,
            "/api/1.0/kernel/school-profile/logo",
            "administration.school_settings.update_logo",
            "administration",
            "school_settings:edit",
            OperationEffect::Write,
            false,
        ),
        // Administration: app-managed AI provider connections.
        route(
            Method::GET,
            "/api/1.0/ai/providers",
            "administration.ai_providers.catalog.list",
            "administration",
            "ai_providers:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/ai/connections",
            "administration.ai_providers.connections.list",
            "administration",
            "ai_providers:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/ai/connections",
            "administration.ai_providers.connections.create",
            "administration",
            "ai_providers:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/ai/connections/{connection_id}",
            "administration.ai_providers.connections.read",
            "administration",
            "ai_providers:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/ai/connections/{connection_id}",
            "administration.ai_providers.connections.update",
            "administration",
            "ai_providers:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/ai/connections/{connection_id}/data-approval",
            "administration.ai_providers.connections.data_approval.update",
            "administration",
            "ai_providers:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/ai/connections/{connection_id}/credentials/rotate",
            "administration.ai_providers.credentials.rotate",
            "administration",
            "ai_providers:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/ai/connections/{connection_id}/test",
            "administration.ai_providers.connections.test",
            "administration",
            "ai_providers:edit",
            OperationEffect::External,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/ai/connections/{connection_id}/models",
            "administration.ai_providers.models.list",
            "administration",
            "ai_providers:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/ai/connections/{connection_id}/models/refresh",
            "administration.ai_providers.models.refresh",
            "administration",
            "ai_providers:edit",
            OperationEffect::External,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/ai/connections/{connection_id}",
            "administration.ai_providers.connections.disconnect",
            "administration",
            "ai_providers:edit",
            OperationEffect::Destructive,
            true,
        ),
        // Administration: ordered Agent provider/model routing.
        route(
            Method::GET,
            "/api/1.0/ai/routes",
            "administration.ai_routing.routes.list",
            "administration",
            "ai_routing:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/ai/routes/options",
            "administration.ai_routing.routes.options",
            "administration",
            "ai_routing:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/ai/routes/resolve",
            "administration.ai_routing.routes.resolve",
            "administration",
            "ai_routing:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/ai/routes",
            "administration.ai_routing.routes.create",
            "administration",
            "ai_routing:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/ai/routes/{route_set_id}",
            "administration.ai_routing.routes.read",
            "administration",
            "ai_routing:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/ai/routes/{route_set_id}",
            "administration.ai_routing.routes.update",
            "administration",
            "ai_routing:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/ai/routes/{route_set_id}",
            "administration.ai_routing.routes.archive",
            "administration",
            "ai_routing:edit",
            OperationEffect::Write,
            true,
        ),
        // Administration: Agent governance, usage, and redacted run evidence.
        route(
            Method::GET,
            "/api/1.0/agent-governance/readiness",
            "administration.agent_governance.readiness",
            "administration",
            "agent_policy:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/agent-governance/capabilities",
            "administration.agent_governance.capabilities.list",
            "administration",
            "agent_policy:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/agent-governance/usage/options",
            "administration.agent_usage.options",
            "administration",
            "agent_usage:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/agent-governance/usage",
            "administration.agent_usage.report",
            "administration",
            "agent_usage:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/agent-governance/usage/export",
            "administration.agent_usage.export",
            "administration",
            "agent_usage:export",
            OperationEffect::Export,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/agent-governance/runs",
            "administration.agent_audit.runs.list",
            "administration",
            "agent_audit:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/agent-governance/runs/{run_id}",
            "administration.agent_audit.runs.read",
            "administration",
            "agent_audit:view",
            OperationEffect::Read,
            true,
        ),
        // Agent: owner-scoped Sessions, runs, history, and personal usage.
        route(
            Method::GET,
            "/api/1.0/agent/sessions",
            "agent.sessions.list",
            "agent",
            "agent:history",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/agent/sessions",
            "agent.sessions.create",
            "agent",
            "agent:run",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/agent/sessions/{session_id}",
            "agent.sessions.read",
            "agent",
            "agent:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PATCH,
            "/api/1.0/agent/sessions/{session_id}",
            "agent.sessions.update",
            "agent",
            "agent:history",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/agent/sessions/{session_id}/archive",
            "agent.sessions.archive",
            "agent",
            "agent:history",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/agent/sessions/{session_id}/messages",
            "agent.messages.list",
            "agent",
            "agent:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/agent/sessions/{session_id}/messages",
            "agent.messages.submit",
            "agent",
            "agent:run",
            OperationEffect::External,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/agent/sessions/{session_id}/runs",
            "agent.runs.list",
            "agent",
            "agent:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/agent/runs/{run_id}",
            "agent.runs.read",
            "agent",
            "agent:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/agent/runs/{run_id}/cancel",
            "agent.runs.cancel",
            "agent",
            "agent:run",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/agent/runs/{run_id}/events",
            "agent.runs.events.list",
            "agent",
            "agent:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/agent/usage/personal",
            "agent.usage.personal.read",
            "agent",
            "agent:view",
            OperationEffect::Read,
            true,
        ),
        // Administration: roles.
        route(
            Method::GET,
            "/api/1.0/roles",
            "administration.roles.list",
            "administration",
            "roles:view",
            OperationEffect::Read,
            false,
        ),
        route(
            Method::GET,
            "/api/1.0/roles/{id}",
            "administration.roles.read",
            "administration",
            "roles:view",
            OperationEffect::Read,
            false,
        ),
        route(
            Method::POST,
            "/api/1.0/roles",
            "administration.roles.create",
            "administration",
            "roles:create",
            OperationEffect::Write,
            false,
        ),
        route(
            Method::PUT,
            "/api/1.0/roles/{id}",
            "administration.roles.update",
            "administration",
            "roles:edit",
            OperationEffect::Write,
            false,
        ),
        route(
            Method::DELETE,
            "/api/1.0/roles/{id}",
            "administration.roles.delete",
            "administration",
            "roles:delete",
            OperationEffect::Destructive,
            false,
        ),
        // Administration: users.
        route(
            Method::GET,
            "/api/1.0/users",
            "administration.users.list",
            "administration",
            "users:view",
            OperationEffect::Read,
            false,
        ),
        route(
            Method::GET,
            "/api/1.0/users/{id}",
            "administration.users.read",
            "administration",
            "users:view",
            OperationEffect::Read,
            false,
        ),
        route(
            Method::POST,
            "/api/1.0/users",
            "administration.users.create",
            "administration",
            "users:create",
            OperationEffect::Write,
            false,
        ),
        route(
            Method::PUT,
            "/api/1.0/users/{id}",
            "administration.users.update",
            "administration",
            "users:edit",
            OperationEffect::Write,
            false,
        ),
        route(
            Method::PUT,
            "/api/1.0/users/{id}/activate",
            "administration.users.activate",
            "administration",
            "users:edit",
            OperationEffect::Write,
            false,
        ),
        route(
            Method::PUT,
            "/api/1.0/users/{id}/deactivate",
            "administration.users.deactivate",
            "administration",
            "users:edit",
            OperationEffect::Write,
            false,
        ),
        route(
            Method::DELETE,
            "/api/1.0/users/{id}",
            "administration.users.delete",
            "administration",
            "users:delete",
            OperationEffect::Destructive,
            false,
        ),
        // Administration: license recovery and local module control.
        route(
            Method::PUT,
            "/api/1.0/access/licenses/activate",
            "administration.licensing.activate_legacy_key",
            "administration",
            "licensing:edit",
            OperationEffect::LicenseRepair,
            false,
        ),
        route(
            Method::PUT,
            "/api/1.0/access/licensing/connect",
            "administration.licensing.connect",
            "administration",
            "licensing:edit",
            OperationEffect::LicenseRepair,
            false,
        ),
        route(
            Method::POST,
            "/api/1.0/access/licensing/refresh",
            "administration.licensing.refresh",
            "administration",
            "licensing:edit",
            OperationEffect::LicenseRepair,
            false,
        ),
        route(
            Method::POST,
            "/api/1.0/access/licensing/import",
            "administration.licensing.import_offline_lease",
            "administration",
            "licensing:edit",
            OperationEffect::LicenseRepair,
            false,
        ),
        route(
            Method::DELETE,
            "/api/1.0/access/modules/{module_key}",
            "administration.licensing.disable_module",
            "administration",
            "licensing:delete",
            OperationEffect::Destructive,
            false,
        ),
        // SIS: canonical people, admissions, and enrolment records.
        route(
            Method::GET,
            "/api/1.0/sis/learner-numbering",
            "sis.learner_numbering.read",
            "sis",
            "sis:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/sis/learner-numbering",
            "sis.learner_numbering.update",
            "sis",
            "sis:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/sis/account-candidates",
            "sis.account_candidates.list",
            "sis",
            "sis:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/sis/imports",
            "sis.imports.list",
            "sis",
            "sis:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/sis/imports",
            "sis.imports.upload",
            "sis",
            "sis:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/sis/imports/{id}",
            "sis.imports.read",
            "sis",
            "sis:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/sis/imports/{id}/mapping",
            "sis.imports.preview",
            "sis",
            "sis:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/sis/imports/{id}/preview",
            "sis.imports.preview.read",
            "sis",
            "sis:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/sis/imports/{id}/commit",
            "sis.imports.commit",
            "sis",
            "sis:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/sis/learners",
            "sis.learners.list",
            "sis",
            "sis:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/sis/learners/{id}",
            "sis.learners.read",
            "sis",
            "sis:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/sis/learners",
            "sis.learners.create",
            "sis",
            "sis:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/sis/learners/{id}",
            "sis.learners.update",
            "sis",
            "sis:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/sis/learners/{id}/account",
            "sis.learners.link_account",
            "sis",
            "sis:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/sis/learners/{id}",
            "sis.learners.delete",
            "sis",
            "sis:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/sis/guardians",
            "sis.guardians.list",
            "sis",
            "sis:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/sis/guardians/{id}",
            "sis.guardians.read",
            "sis",
            "sis:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/sis/guardians",
            "sis.guardians.create",
            "sis",
            "sis:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/sis/guardians/{id}",
            "sis.guardians.update",
            "sis",
            "sis:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/sis/guardians/{id}/account",
            "sis.guardians.link_account",
            "sis",
            "sis:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/sis/guardians/{id}",
            "sis.guardians.delete",
            "sis",
            "sis:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/sis/guardian-relationships",
            "sis.guardian_relationships.list",
            "sis",
            "sis:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/sis/guardian-relationships/{id}",
            "sis.guardian_relationships.read",
            "sis",
            "sis:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/sis/guardian-relationships",
            "sis.guardian_relationships.create",
            "sis",
            "sis:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/sis/guardian-relationships/{id}",
            "sis.guardian_relationships.update",
            "sis",
            "sis:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/sis/guardian-relationships/{id}",
            "sis.guardian_relationships.delete",
            "sis",
            "sis:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/sis/applications",
            "sis.applications.list",
            "sis",
            "sis:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/sis/applications/{id}",
            "sis.applications.read",
            "sis",
            "sis:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/sis/applications",
            "sis.applications.create",
            "sis",
            "sis:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/sis/applications/{id}",
            "sis.applications.update",
            "sis",
            "sis:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/sis/applications/{id}",
            "sis.applications.delete",
            "sis",
            "sis:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/sis/enrolments",
            "sis.enrolments.list",
            "sis",
            "sis:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/sis/enrolments/{id}",
            "sis.enrolments.read",
            "sis",
            "sis:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/sis/enrolments",
            "sis.enrolments.create",
            "sis",
            "sis:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/sis/enrolments/{id}",
            "sis.enrolments.update",
            "sis",
            "sis:edit",
            OperationEffect::Write,
            true,
        ),
        // Academics: canonical teaching structure.
        route(
            Method::GET,
            "/api/1.0/academics/academic-years",
            "academics.academic_years.list",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/academic-years/{id}",
            "academics.academic_years.read",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/academics/academic-years",
            "academics.academic_years.create",
            "academics",
            "academics:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/academics/academic-years/{id}",
            "academics.academic_years.update",
            "academics",
            "academics:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/academics/academic-years/{id}",
            "academics.academic_years.delete",
            "academics",
            "academics:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/terms",
            "academics.terms.list",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/terms/{id}",
            "academics.terms.read",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/academics/terms",
            "academics.terms.create",
            "academics",
            "academics:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/academics/terms/{id}",
            "academics.terms.update",
            "academics",
            "academics:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/academics/terms/{id}",
            "academics.terms.delete",
            "academics",
            "academics:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/subjects",
            "academics.subjects.list",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/subjects/{id}",
            "academics.subjects.read",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/academics/subjects",
            "academics.subjects.create",
            "academics",
            "academics:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/academics/subjects/{id}",
            "academics.subjects.update",
            "academics",
            "academics:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/academics/subjects/{id}",
            "academics.subjects.delete",
            "academics",
            "academics:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/grade-levels",
            "academics.grade_levels.list",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/grade-levels/{id}",
            "academics.grade_levels.read",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/academics/grade-levels",
            "academics.grade_levels.create",
            "academics",
            "academics:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/academics/grade-levels/{id}",
            "academics.grade_levels.update",
            "academics",
            "academics:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/academics/grade-levels/{id}",
            "academics.grade_levels.delete",
            "academics",
            "academics:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/teacher-candidates",
            "academics.teacher_candidates.list",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/teachers",
            "academics.teachers.list",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/teachers/{id}",
            "academics.teachers.read",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/academics/teachers",
            "academics.teachers.create",
            "academics",
            "academics:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/academics/teachers/{id}",
            "academics.teachers.update",
            "academics",
            "academics:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/academics/teachers/{id}",
            "academics.teachers.delete",
            "academics",
            "academics:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/classes",
            "academics.classes.list",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/classes/{id}",
            "academics.classes.read",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/academics/classes",
            "academics.classes.create",
            "academics",
            "academics:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/academics/classes/{id}",
            "academics.classes.update",
            "academics",
            "academics:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/academics/classes/{id}",
            "academics.classes.delete",
            "academics",
            "academics:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/teaching-assignments",
            "academics.teaching_assignments.list",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/teaching-assignments/{id}",
            "academics.teaching_assignments.read",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/academics/teaching-assignments",
            "academics.teaching_assignments.create",
            "academics",
            "academics:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/academics/teaching-assignments/{id}",
            "academics.teaching_assignments.update",
            "academics",
            "academics:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/academics/teaching-assignments/{id}",
            "academics.teaching_assignments.delete",
            "academics",
            "academics:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/assessment-cycles",
            "academics.assessment_cycles.list",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/assessment-cycles/{id}",
            "academics.assessment_cycles.read",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/academics/assessment-cycles",
            "academics.assessment_cycles.create",
            "academics",
            "academics:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/academics/assessment-cycles/{id}",
            "academics.assessment_cycles.update",
            "academics",
            "academics:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/academics/assessment-cycles/{id}",
            "academics.assessment_cycles.delete",
            "academics",
            "academics:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/assessment-cycles/{cycle_id}/components",
            "academics.assessment_components.list",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/assessment-components/{id}",
            "academics.assessment_components.read",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/academics/assessment-cycles/{cycle_id}/components",
            "academics.assessment_components.create",
            "academics",
            "academics:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/academics/assessment-components/{id}",
            "academics.assessment_components.update",
            "academics",
            "academics:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/academics/assessment-components/{id}",
            "academics.assessment_components.delete",
            "academics",
            "academics:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/gradebook/references",
            "academics.gradebook.references.read",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/gradebook/mark-sheets",
            "academics.gradebook.mark_sheets.list",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/academics/gradebook/mark-sheets",
            "academics.gradebook.mark_sheets.create",
            "academics",
            "academics:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/gradebook/mark-sheets/{id}",
            "academics.gradebook.mark_sheets.read",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/academics/gradebook/mark-sheets/{id}/marks",
            "academics.gradebook.mark_sheets.marks.update",
            "academics",
            "academics:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/academics/gradebook/mark-sheets/{id}/submit",
            "academics.gradebook.mark_sheets.submit",
            "academics",
            "academics:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/academics/gradebook/mark-sheets/{id}/publish",
            "academics.gradebook.mark_sheets.publish",
            "academics",
            "academics:manage",
            OperationEffect::External,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/academics/gradebook/mark-sheets/{id}/reopen",
            "academics.gradebook.mark_sheets.reopen",
            "academics",
            "academics:manage",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/academics/gradebook/mark-sheets/{id}",
            "academics.gradebook.mark_sheets.delete",
            "academics",
            "academics:delete",
            OperationEffect::Destructive,
            true,
        ),
        // Academic progress and reporting: grading policy, report lifecycle, and transcripts.
        route(
            Method::GET,
            "/api/1.0/academics/reporting/references",
            "academics.reporting.references.read",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/reporting/grading-schemes",
            "academics.reporting.grading_schemes.list",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/academics/reporting/grading-schemes",
            "academics.reporting.grading_schemes.create",
            "academics",
            "academics:manage",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/reporting/grading-schemes/{id}",
            "academics.reporting.grading_schemes.read",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/academics/reporting/grading-schemes/{id}",
            "academics.reporting.grading_schemes.update",
            "academics",
            "academics:manage",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/academics/reporting/grading-schemes/{id}/retire",
            "academics.reporting.grading_schemes.retire",
            "academics",
            "academics:manage",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/academics/reporting/grading-schemes/{id}",
            "academics.reporting.grading_schemes.delete",
            "academics",
            "academics:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/reporting/report-batches",
            "academics.reporting.report_batches.list",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/academics/reporting/report-batches",
            "academics.reporting.report_batches.generate",
            "academics",
            "academics:manage",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/reporting/report-batches/{id}",
            "academics.reporting.report_batches.read",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/academics/reporting/report-cards/{id}/teacher-comment",
            "academics.reporting.report_cards.teacher_comment.update",
            "academics",
            "academics:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/academics/reporting/report-cards/{id}/review",
            "academics.reporting.report_cards.review.update",
            "academics",
            "academics:manage",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/academics/reporting/report-batches/{id}/review",
            "academics.reporting.report_batches.review",
            "academics",
            "academics:manage",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/academics/reporting/report-batches/{id}/publish",
            "academics.reporting.report_batches.publish",
            "academics",
            "academics:manage",
            OperationEffect::External,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/academics/reporting/report-batches/{id}/reopen",
            "academics.reporting.report_batches.reopen",
            "academics",
            "academics:manage",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/academics/reporting/report-batches/{id}",
            "academics.reporting.report_batches.delete",
            "academics",
            "academics:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/academics/reporting/learners/{id}/transcript",
            "academics.reporting.transcripts.read",
            "academics",
            "academics:view",
            OperationEffect::Read,
            true,
        ),
        // Finance: currencies and chart-of-account structure.
        route(
            Method::GET,
            "/api/1.0/finance/currencies",
            "finance.currencies.list",
            "finance",
            "finance:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/finance/currencies/{id}",
            "finance.currencies.read",
            "finance",
            "finance:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/finance/currencies",
            "finance.currencies.create",
            "finance",
            "finance:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/finance/currencies/{id}",
            "finance.currencies.update",
            "finance",
            "finance:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/finance/currencies/{id}",
            "finance.currencies.delete",
            "finance",
            "finance:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/finance/accounts",
            "finance.accounts.list",
            "finance",
            "finance:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/finance/accounts/{id}",
            "finance.accounts.read",
            "finance",
            "finance:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/finance/accounts",
            "finance.accounts.create",
            "finance",
            "finance:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/finance/accounts/{id}",
            "finance.accounts.update",
            "finance",
            "finance:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/finance/accounts/{id}",
            "finance.accounts.delete",
            "finance",
            "finance:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/finance/fiscal-years",
            "finance.fiscal_years.list",
            "finance",
            "finance:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/finance/fiscal-years/{id}",
            "finance.fiscal_years.read",
            "finance",
            "finance:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/finance/fiscal-years",
            "finance.fiscal_years.create",
            "finance",
            "finance:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/finance/fiscal-years/{id}",
            "finance.fiscal_years.update",
            "finance",
            "finance:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/finance/fiscal-years/{id}",
            "finance.fiscal_years.delete",
            "finance",
            "finance:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/finance/fiscal-years/{id}/open",
            "finance.fiscal_years.open",
            "finance",
            "finance:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/finance/fiscal-years/{id}/close",
            "finance.fiscal_years.close",
            "finance",
            "finance:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/finance/fiscal-years/{id}/periods",
            "finance.accounting_periods.list",
            "finance",
            "finance:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/finance/periods/{id}/close",
            "finance.accounting_periods.close",
            "finance",
            "finance:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/finance/periods/{id}/reopen",
            "finance.accounting_periods.reopen",
            "finance",
            "finance:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/finance/posting-requests",
            "finance.posting_requests.list",
            "finance",
            "finance:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/finance/posting-requests/{id}",
            "finance.posting_requests.read",
            "finance",
            "finance:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/finance/posting-requests/{id}/convert",
            "finance.posting_requests.convert",
            "finance",
            "finance:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/finance/posting-requests/{id}/reject",
            "finance.posting_requests.reject",
            "finance",
            "finance:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/finance/journals",
            "finance.journals.list",
            "finance",
            "finance:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/finance/journals/{id}",
            "finance.journals.read",
            "finance",
            "finance:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/finance/journals/{id}/validation",
            "finance.journals.validation.read",
            "finance",
            "finance:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/finance/journals",
            "finance.journals.create",
            "finance",
            "finance:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/finance/journals/{id}",
            "finance.journals.update",
            "finance",
            "finance:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/finance/journals/{id}",
            "finance.journals.delete",
            "finance",
            "finance:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/finance/journals/{id}/submit",
            "finance.journals.submit",
            "finance",
            "finance:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/finance/journals/{id}/approve",
            "finance.journals.approve",
            "finance",
            "finance:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/finance/journals/{id}/reject",
            "finance.journals.reject",
            "finance",
            "finance:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/finance/journals/{id}/post",
            "finance.journals.post",
            "finance",
            "finance:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/finance/journals/{id}/reverse",
            "finance.journals.reverse",
            "finance",
            "finance:edit",
            OperationEffect::Write,
            true,
        ),
        // Fees and Billing: learner accounts and versioned fee structures.
        route(
            Method::GET,
            "/api/1.0/fees/reference-data",
            "fees.reference_data.read",
            "fees",
            "fees:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/fees/learner-candidates",
            "fees.learner_candidates.list",
            "fees",
            "fees:create",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/fees/imports",
            "fees.imports.list",
            "fees",
            "fees:create",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/fees/imports",
            "fees.imports.upload",
            "fees",
            "fees:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/fees/imports/{id}",
            "fees.imports.read",
            "fees",
            "fees:create",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/fees/imports/{id}/mapping",
            "fees.imports.preview",
            "fees",
            "fees:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/fees/imports/{id}/preview",
            "fees.imports.preview.read",
            "fees",
            "fees:create",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/fees/imports/{id}/commit",
            "fees.imports.commit",
            "fees",
            "fees:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/fees/billing-accounts",
            "fees.billing_accounts.list",
            "fees",
            "fees:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/fees/billing-accounts/{id}",
            "fees.billing_accounts.read",
            "fees",
            "fees:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/fees/billing-accounts",
            "fees.billing_accounts.create",
            "fees",
            "fees:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/fees/billing-accounts/{id}",
            "fees.billing_accounts.update",
            "fees",
            "fees:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/fees/fee-structures",
            "fees.fee_structures.list",
            "fees",
            "fees:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/fees/fee-structures/{id}",
            "fees.fee_structures.read",
            "fees",
            "fees:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/fees/fee-structures",
            "fees.fee_structures.create",
            "fees",
            "fees:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/fees/fee-structures/{id}",
            "fees.fee_structures.update",
            "fees",
            "fees:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/fees/fee-structures/{id}",
            "fees.fee_structures.delete",
            "fees",
            "fees:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/fees/fee-structures/{id}/activate",
            "fees.fee_structures.activate",
            "fees",
            "fees:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/fees/fee-structures/{id}/retire",
            "fees.fee_structures.retire",
            "fees",
            "fees:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/fees/invoices",
            "fees.invoices.list",
            "fees",
            "fees:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/fees/invoices/{id}",
            "fees.invoices.read",
            "fees",
            "fees:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/fees/invoices",
            "fees.invoices.create",
            "fees",
            "fees:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/fees/invoices/{id}/issue",
            "fees.invoices.issue",
            "fees",
            "fees:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/fees/invoices/{id}",
            "fees.invoices.delete",
            "fees",
            "fees:delete",
            OperationEffect::Destructive,
            true,
        ),
        // Procurement: Finance-backed reference data, employee requesters,
        // suppliers, controlled requisitions, purchase orders, and receiving.
        route(
            Method::GET,
            "/api/1.0/procurement/reference-data",
            "procurement.reference_data.read",
            "procurement",
            "procurement:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/procurement/requester-candidates",
            "procurement.requester_candidates.list",
            "procurement",
            "procurement:create",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/procurement/suppliers",
            "procurement.suppliers.list",
            "procurement",
            "procurement:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/procurement/suppliers/{id}",
            "procurement.suppliers.read",
            "procurement",
            "procurement:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/procurement/suppliers",
            "procurement.suppliers.create",
            "procurement",
            "procurement:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/procurement/suppliers/{id}",
            "procurement.suppliers.update",
            "procurement",
            "procurement:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/procurement/suppliers/{id}",
            "procurement.suppliers.delete",
            "procurement",
            "procurement:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/procurement/requisitions",
            "procurement.requisitions.list",
            "procurement",
            "procurement:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/procurement/requisitions/{id}",
            "procurement.requisitions.read",
            "procurement",
            "procurement:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/procurement/requisitions",
            "procurement.requisitions.create",
            "procurement",
            "procurement:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/procurement/requisitions/{id}",
            "procurement.requisitions.update",
            "procurement",
            "procurement:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/procurement/requisitions/{id}",
            "procurement.requisitions.delete",
            "procurement",
            "procurement:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/procurement/requisitions/{id}/submit",
            "procurement.requisitions.submit",
            "procurement",
            "procurement:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/procurement/requisitions/{id}/approve",
            "procurement.requisitions.approve",
            "procurement",
            "procurement:approve",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/procurement/requisitions/{id}/reject",
            "procurement.requisitions.reject",
            "procurement",
            "procurement:approve",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/procurement/requisitions/{id}/cancel",
            "procurement.requisitions.cancel",
            "procurement",
            "procurement:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/procurement/purchase-orders",
            "procurement.purchase_orders.list",
            "procurement",
            "procurement:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/procurement/purchase-orders/{id}",
            "procurement.purchase_orders.read",
            "procurement",
            "procurement:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/procurement/purchase-orders",
            "procurement.purchase_orders.create",
            "procurement",
            "procurement:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/procurement/purchase-orders/{id}",
            "procurement.purchase_orders.update",
            "procurement",
            "procurement:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/procurement/purchase-orders/{id}/issue",
            "procurement.purchase_orders.issue",
            "procurement",
            "procurement:approve",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/procurement/purchase-orders/{id}/cancel",
            "procurement.purchase_orders.cancel",
            "procurement",
            "procurement:approve",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/procurement/goods-receipts",
            "procurement.goods_receipts.list",
            "procurement",
            "procurement:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/procurement/goods-receipts/{id}",
            "procurement.goods_receipts.read",
            "procurement",
            "procurement:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/procurement/goods-receipts",
            "procurement.goods_receipts.create",
            "procurement",
            "procurement:receive",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/procurement/goods-receipts/{id}",
            "procurement.goods_receipts.update",
            "procurement",
            "procurement:receive",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/procurement/goods-receipts/{id}/post",
            "procurement.goods_receipts.post",
            "procurement",
            "procurement:receive",
            OperationEffect::Write,
            true,
        ),
        // Assets and inventory: item/store catalogues and immutable stock ledger.
        route(
            Method::GET,
            "/api/1.0/assets-inventory/items",
            "assets_inventory.items.list",
            "assets_inventory",
            "assets_inventory:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/assets-inventory/items/{id}",
            "assets_inventory.items.read",
            "assets_inventory",
            "assets_inventory:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/assets-inventory/items",
            "assets_inventory.items.create",
            "assets_inventory",
            "assets_inventory:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/assets-inventory/items/{id}",
            "assets_inventory.items.update",
            "assets_inventory",
            "assets_inventory:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/assets-inventory/items/{id}",
            "assets_inventory.items.delete",
            "assets_inventory",
            "assets_inventory:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/assets-inventory/stores",
            "assets_inventory.stores.list",
            "assets_inventory",
            "assets_inventory:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/assets-inventory/stores/{id}",
            "assets_inventory.stores.read",
            "assets_inventory",
            "assets_inventory:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/assets-inventory/stores",
            "assets_inventory.stores.create",
            "assets_inventory",
            "assets_inventory:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/assets-inventory/stores/{id}",
            "assets_inventory.stores.update",
            "assets_inventory",
            "assets_inventory:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/assets-inventory/stores/{id}",
            "assets_inventory.stores.delete",
            "assets_inventory",
            "assets_inventory:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/assets-inventory/stock-balances",
            "assets_inventory.stock_balances.list",
            "assets_inventory",
            "assets_inventory:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/assets-inventory/stock-movements",
            "assets_inventory.stock_movements.list",
            "assets_inventory",
            "assets_inventory:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/assets-inventory/stock-movements/{id}",
            "assets_inventory.stock_movements.read",
            "assets_inventory",
            "assets_inventory:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/assets-inventory/manual-receipts",
            "assets_inventory.manual_receipts.create",
            "assets_inventory",
            "assets_inventory:receive",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/assets-inventory/issues",
            "assets_inventory.issues.create",
            "assets_inventory",
            "assets_inventory:issue",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/assets-inventory/transfers",
            "assets_inventory.transfers.create",
            "assets_inventory",
            "assets_inventory:transfer",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/assets-inventory/adjustments",
            "assets_inventory.adjustments.create",
            "assets_inventory",
            "assets_inventory:adjust",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/assets-inventory/stock-movements/{id}/reverse",
            "assets_inventory.stock_movements.reverse",
            "assets_inventory",
            "assets_inventory:reverse",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/assets-inventory/goods-receipt-allocations",
            "assets_inventory.goods_receipt_allocations.list",
            "assets_inventory",
            "assets_inventory:receive",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/assets-inventory/goods-receipt-allocations",
            "assets_inventory.goods_receipt_allocations.create",
            "assets_inventory",
            "assets_inventory:receive",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/assets-inventory/stock-request-requesters",
            "assets_inventory.requester_candidates.list",
            "assets_inventory",
            "assets_inventory:request",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/assets-inventory/stock-request-departments",
            "assets_inventory.department_candidates.list",
            "assets_inventory",
            "assets_inventory:request",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/assets-inventory/stock-requests",
            "assets_inventory.stock_requests.list",
            "assets_inventory",
            "assets_inventory:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/assets-inventory/stock-requests/{id}",
            "assets_inventory.stock_requests.read",
            "assets_inventory",
            "assets_inventory:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/assets-inventory/stock-requests/{id}/fulfilment-preview",
            "assets_inventory.stock_requests.fulfilment_preview.read",
            "assets_inventory",
            "assets_inventory:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/assets-inventory/stock-requests",
            "assets_inventory.stock_requests.create",
            "assets_inventory",
            "assets_inventory:request",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/assets-inventory/stock-requests/{id}",
            "assets_inventory.stock_requests.update",
            "assets_inventory",
            "assets_inventory:request",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/assets-inventory/stock-requests/{id}",
            "assets_inventory.stock_requests.delete",
            "assets_inventory",
            "assets_inventory:request",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/assets-inventory/stock-requests/{id}/submit",
            "assets_inventory.stock_requests.submit",
            "assets_inventory",
            "assets_inventory:request",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/assets-inventory/stock-requests/{id}/cancel",
            "assets_inventory.stock_requests.cancel",
            "assets_inventory",
            "assets_inventory:request",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/assets-inventory/stock-requests/{id}/approve",
            "assets_inventory.stock_requests.approve",
            "assets_inventory",
            "assets_inventory:approve",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/assets-inventory/stock-requests/{id}/reject",
            "assets_inventory.stock_requests.reject",
            "assets_inventory",
            "assets_inventory:approve",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/assets-inventory/stock-requests/{id}/close",
            "assets_inventory.stock_requests.close",
            "assets_inventory",
            "assets_inventory:approve",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/assets-inventory/stock-requests/{id}/fulfilments",
            "assets_inventory.stock_request_fulfilments.create",
            "assets_inventory",
            "assets_inventory:issue",
            OperationEffect::Write,
            true,
        ),
        // HR and payroll: canonical workforce directory.
        route(
            Method::GET,
            "/api/1.0/hr-payroll/imports",
            "hr_payroll.imports.list",
            "hr_payroll",
            "hr_payroll:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/hr-payroll/imports",
            "hr_payroll.imports.upload",
            "hr_payroll",
            "hr_payroll:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/hr-payroll/imports/{id}",
            "hr_payroll.imports.read",
            "hr_payroll",
            "hr_payroll:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/hr-payroll/imports/{id}/mapping",
            "hr_payroll.imports.preview",
            "hr_payroll",
            "hr_payroll:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/hr-payroll/imports/{id}/preview",
            "hr_payroll.imports.preview.read",
            "hr_payroll",
            "hr_payroll:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/hr-payroll/imports/{id}/commit",
            "hr_payroll.imports.commit",
            "hr_payroll",
            "hr_payroll:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/hr-payroll/departments",
            "hr_payroll.departments.list",
            "hr_payroll",
            "hr_payroll:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/hr-payroll/departments/{id}",
            "hr_payroll.departments.read",
            "hr_payroll",
            "hr_payroll:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/hr-payroll/departments",
            "hr_payroll.departments.create",
            "hr_payroll",
            "hr_payroll:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/hr-payroll/departments/{id}",
            "hr_payroll.departments.update",
            "hr_payroll",
            "hr_payroll:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/hr-payroll/departments/{id}",
            "hr_payroll.departments.delete",
            "hr_payroll",
            "hr_payroll:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/hr-payroll/positions",
            "hr_payroll.positions.list",
            "hr_payroll",
            "hr_payroll:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/hr-payroll/positions/{id}",
            "hr_payroll.positions.read",
            "hr_payroll",
            "hr_payroll:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/hr-payroll/positions",
            "hr_payroll.positions.create",
            "hr_payroll",
            "hr_payroll:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/hr-payroll/positions/{id}",
            "hr_payroll.positions.update",
            "hr_payroll",
            "hr_payroll:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/hr-payroll/positions/{id}",
            "hr_payroll.positions.delete",
            "hr_payroll",
            "hr_payroll:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/hr-payroll/employees",
            "hr_payroll.employees.list",
            "hr_payroll",
            "hr_payroll:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/hr-payroll/employees/{id}",
            "hr_payroll.employees.read",
            "hr_payroll",
            "hr_payroll:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/hr-payroll/employees",
            "hr_payroll.employees.create",
            "hr_payroll",
            "hr_payroll:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/hr-payroll/employees/{id}",
            "hr_payroll.employees.update",
            "hr_payroll",
            "hr_payroll:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/hr-payroll/employees/{id}/account",
            "hr_payroll.employees.link_account",
            "hr_payroll",
            "hr_payroll:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/hr-payroll/employees/{id}",
            "hr_payroll.employees.delete",
            "hr_payroll",
            "hr_payroll:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/hr-payroll/employment-engagements",
            "hr_payroll.employment_engagements.list",
            "hr_payroll",
            "hr_payroll:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/hr-payroll/employment-engagements/{id}",
            "hr_payroll.employment_engagements.read",
            "hr_payroll",
            "hr_payroll:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/hr-payroll/employment-engagements",
            "hr_payroll.employment_engagements.create",
            "hr_payroll",
            "hr_payroll:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/hr-payroll/employment-engagements/{id}",
            "hr_payroll.employment_engagements.update",
            "hr_payroll",
            "hr_payroll:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/hr-payroll/employment-engagements/{id}",
            "hr_payroll.employment_engagements.delete",
            "hr_payroll",
            "hr_payroll:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/hr-payroll/availability",
            "hr_payroll.availability.list",
            "hr_payroll",
            "hr_payroll:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/hr-payroll/availability/{id}",
            "hr_payroll.availability.read",
            "hr_payroll",
            "hr_payroll:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/hr-payroll/availability",
            "hr_payroll.availability.create",
            "hr_payroll",
            "hr_payroll:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/hr-payroll/availability/{id}",
            "hr_payroll.availability.update",
            "hr_payroll",
            "hr_payroll:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/hr-payroll/availability/{id}",
            "hr_payroll.availability.delete",
            "hr_payroll",
            "hr_payroll:delete",
            OperationEffect::Destructive,
            true,
        ),
        // Fleet: vehicles.
        route(
            Method::GET,
            "/api/1.0/fleet/vehicles",
            "fleet.vehicles.list",
            "fleet",
            "fleet:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/fleet/vehicles/{id}",
            "fleet.vehicles.read",
            "fleet",
            "fleet:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/fleet/vehicles",
            "fleet.vehicles.create",
            "fleet",
            "fleet:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/fleet/vehicles/{id}",
            "fleet.vehicles.update",
            "fleet",
            "fleet:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/fleet/vehicles/{id}",
            "fleet.vehicles.delete",
            "fleet",
            "fleet:delete",
            OperationEffect::Destructive,
            true,
        ),
        // Fleet: drivers.
        route(
            Method::GET,
            "/api/1.0/fleet/driver-candidates",
            "fleet.driver_candidates.list",
            "fleet",
            "fleet:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/fleet/drivers",
            "fleet.drivers.list",
            "fleet",
            "fleet:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/fleet/drivers/{id}",
            "fleet.drivers.read",
            "fleet",
            "fleet:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/fleet/drivers",
            "fleet.drivers.create",
            "fleet",
            "fleet:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/fleet/drivers/{id}",
            "fleet.drivers.update",
            "fleet",
            "fleet:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/fleet/drivers/{id}",
            "fleet.drivers.delete",
            "fleet",
            "fleet:delete",
            OperationEffect::Destructive,
            true,
        ),
        // Fleet: vehicle daily logs.
        route(
            Method::GET,
            "/api/1.0/vehicle-logs",
            "fleet.vehicle_logs.list",
            "fleet",
            "fleet:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/vehicle-logs/{id}",
            "fleet.vehicle_logs.read",
            "fleet",
            "fleet:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/vehicle-logs",
            "fleet.vehicle_logs.create",
            "fleet",
            "fleet:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/vehicle-logs/{id}",
            "fleet.vehicle_logs.update",
            "fleet",
            "fleet:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/vehicle-logs/{id}",
            "fleet.vehicle_logs.delete",
            "fleet",
            "fleet:delete",
            OperationEffect::Destructive,
            true,
        ),
        // Timetabling.
        route(
            Method::GET,
            "/api/1.0/timetabling/configuration",
            "timetabling.configuration.read",
            "timetabling",
            "timetabling:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/timetabling/configuration",
            "timetabling.configuration.update",
            "timetabling",
            "timetabling:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/timetabling/generate",
            "timetabling.runs.generate",
            "timetabling",
            "timetabling:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/timetabling/runs/latest",
            "timetabling.runs.read_latest",
            "timetabling",
            "timetabling:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/timetabling/runs",
            "timetabling.runs.list",
            "timetabling",
            "timetabling:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/timetabling/runs/{id}",
            "timetabling.runs.read",
            "timetabling",
            "timetabling:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/timetabling/runs/{id}/publish",
            "timetabling.runs.publish",
            "timetabling",
            "timetabling:edit",
            OperationEffect::External,
            true,
        ),
        // Attendance: daily learner registers.
        route(
            Method::GET,
            "/api/1.0/attendance/references",
            "attendance.references.read",
            "attendance",
            "attendance:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/attendance/registers",
            "attendance.registers.list",
            "attendance",
            "attendance:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/attendance/registers",
            "attendance.registers.create",
            "attendance",
            "attendance:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/attendance/registers/{id}",
            "attendance.registers.read",
            "attendance",
            "attendance:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/attendance/registers/{id}/marks",
            "attendance.registers.marks.update",
            "attendance",
            "attendance:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/attendance/registers/{id}/submit",
            "attendance.registers.submit",
            "attendance",
            "attendance:submit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/attendance/registers/{id}/reopen",
            "attendance.registers.reopen",
            "attendance",
            "attendance:manage",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/attendance/registers/{id}",
            "attendance.registers.delete",
            "attendance",
            "attendance:delete",
            OperationEffect::Destructive,
            true,
        ),
        // Communication: reviewed announcements and personal in-app inbox.
        route(
            Method::GET,
            "/api/1.0/messaging/references",
            "messaging.references.read",
            "messaging",
            "messaging:create",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/messaging/announcements",
            "messaging.announcements.list",
            "messaging",
            "messaging:create",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/messaging/announcements",
            "messaging.announcements.create",
            "messaging",
            "messaging:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/messaging/announcements/{id}",
            "messaging.announcements.read",
            "messaging",
            "messaging:create",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/messaging/announcements/{id}",
            "messaging.announcements.update",
            "messaging",
            "messaging:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/messaging/announcements/{id}/audience-preview",
            "messaging.announcements.audience_preview.read",
            "messaging",
            "messaging:create",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/messaging/announcements/{id}/submit",
            "messaging.announcements.submit",
            "messaging",
            "messaging:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/messaging/announcements/{id}/reopen",
            "messaging.announcements.reopen",
            "messaging",
            "messaging:manage",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/messaging/announcements/{id}/publish",
            "messaging.announcements.publish",
            "messaging",
            "messaging:send",
            OperationEffect::External,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/messaging/announcements/{id}/cancel",
            "messaging.announcements.cancel",
            "messaging",
            "messaging:manage",
            OperationEffect::External,
            true,
        ),
        route(
            Method::DELETE,
            "/api/1.0/messaging/announcements/{id}",
            "messaging.announcements.delete",
            "messaging",
            "messaging:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/messaging/announcements/{id}/deliveries",
            "messaging.deliveries.list",
            "messaging",
            "messaging:send",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/messaging/inbox",
            "messaging.inbox.list",
            "messaging",
            "messaging:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/messaging/inbox/{id}",
            "messaging.inbox.read",
            "messaging",
            "messaging:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/messaging/inbox/{id}/read",
            "messaging.inbox.mark_read",
            "messaging",
            "messaging:view",
            OperationEffect::Write,
            true,
        ),
        // Hostel: residences, rooms, previewed allocations, occupancy, and pastoral care.
        route(
            Method::GET,
            "/api/1.0/hostel/references",
            "hostel.references.read",
            "hostel",
            "hostel:manage",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/hostel/residences",
            "hostel.residences.list",
            "hostel",
            "hostel:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/hostel/residences",
            "hostel.residences.create",
            "hostel",
            "hostel:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/hostel/residences/{id}",
            "hostel.residences.read",
            "hostel",
            "hostel:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/hostel/residences/{id}",
            "hostel.residences.update",
            "hostel",
            "hostel:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/hostel/rooms",
            "hostel.rooms.list",
            "hostel",
            "hostel:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/hostel/rooms",
            "hostel.rooms.create",
            "hostel",
            "hostel:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/hostel/rooms/{id}",
            "hostel.rooms.read",
            "hostel",
            "hostel:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/hostel/rooms/{id}",
            "hostel.rooms.update",
            "hostel",
            "hostel:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/hostel/allocations/preview",
            "hostel.allocations.preview",
            "hostel",
            "hostel:allocate",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/hostel/allocations",
            "hostel.allocations.list",
            "hostel",
            "hostel:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/hostel/allocations",
            "hostel.allocations.create",
            "hostel",
            "hostel:allocate",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/hostel/allocations/{id}",
            "hostel.allocations.read",
            "hostel",
            "hostel:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/hostel/allocations/{id}/activate",
            "hostel.allocations.activate",
            "hostel",
            "hostel:allocate",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/hostel/allocations/{id}/end",
            "hostel.allocations.end",
            "hostel",
            "hostel:allocate",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/hostel/allocations/{id}/cancel",
            "hostel.allocations.cancel",
            "hostel",
            "hostel:allocate",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/hostel/allocations/{id}/transfer-preview",
            "hostel.allocations.transfer_preview",
            "hostel",
            "hostel:allocate",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/hostel/allocations/{id}/transfer",
            "hostel.allocations.transfer",
            "hostel",
            "hostel:allocate",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/hostel/pastoral-records",
            "hostel.pastoral_records.list",
            "hostel",
            "hostel:pastoral",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/hostel/pastoral-records",
            "hostel.pastoral_records.create",
            "hostel",
            "hostel:pastoral",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/hostel/pastoral-records/{id}",
            "hostel.pastoral_records.read",
            "hostel",
            "hostel:pastoral",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/hostel/pastoral-records/{id}",
            "hostel.pastoral_records.update",
            "hostel",
            "hostel:pastoral",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/hostel/pastoral-records/{id}/resolve",
            "hostel.pastoral_records.resolve",
            "hostel",
            "hostel:pastoral",
            OperationEffect::Write,
            true,
        ),
        // Document Registry: private filing, classification, retention, and disposition evidence.
        route(
            Method::GET,
            "/api/1.0/document-registry/numbering-policy",
            "document_registry.numbering_policy.read",
            "document_registry",
            "document_registry:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/document-registry/numbering-policy",
            "document_registry.numbering_policy.update",
            "document_registry",
            "document_registry:manage",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/document-registry/series",
            "document_registry.series.list",
            "document_registry",
            "document_registry:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/document-registry/series",
            "document_registry.series.create",
            "document_registry",
            "document_registry:manage",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/document-registry/series/{id}",
            "document_registry.series.read",
            "document_registry",
            "document_registry:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/document-registry/series/{id}",
            "document_registry.series.update",
            "document_registry",
            "document_registry:manage",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/document-registry/files",
            "document_registry.files.list",
            "document_registry",
            "document_registry:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/document-registry/files",
            "document_registry.files.create",
            "document_registry",
            "document_registry:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/document-registry/files/{id}",
            "document_registry.files.read",
            "document_registry",
            "document_registry:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/document-registry/files/{id}",
            "document_registry.files.update",
            "document_registry",
            "document_registry:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/document-registry/files/{id}/reclassify",
            "document_registry.files.reclassify",
            "document_registry",
            "document_registry:classify",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/document-registry/files/{id}/close",
            "document_registry.files.close",
            "document_registry",
            "document_registry:close",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/document-registry/files/{id}/activity",
            "document_registry.files.activity.list",
            "document_registry",
            "document_registry:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/document-registry/files/{id}/download",
            "document_registry.files.download",
            "document_registry",
            "document_registry:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/document-registry/retention-due",
            "document_registry.retention_due.list",
            "document_registry",
            "document_registry:dispose",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/document-registry/disposition-reviews",
            "document_registry.disposition_reviews.list",
            "document_registry",
            "document_registry:dispose",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/document-registry/disposition-reviews/{id}",
            "document_registry.disposition_reviews.read",
            "document_registry",
            "document_registry:dispose",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/document-registry/files/{id}/disposition-reviews",
            "document_registry.disposition_reviews.create",
            "document_registry",
            "document_registry:dispose",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/document-registry/disposition-reviews/{id}/approve",
            "document_registry.disposition_reviews.approve",
            "document_registry",
            "document_registry:manage",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/document-registry/disposition-reviews/{id}/reject",
            "document_registry.disposition_reviews.reject",
            "document_registry",
            "document_registry:manage",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/document-registry/disposition-reviews/{id}/execute",
            "document_registry.disposition_reviews.execute",
            "document_registry",
            "document_registry:manage",
            OperationEffect::Write,
            true,
        ),
        // Health services: canonical patients, care, visits, medication, and follow-up.
        route(
            Method::GET,
            "/api/1.0/health/references",
            "health.references.read",
            "health",
            "health:manage",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/health/patients",
            "health.patients.list",
            "health",
            "health:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/health/patients",
            "health.patients.create",
            "health",
            "health:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/health/patients/{id}",
            "health.patients.read",
            "health",
            "health:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/health/patients/{id}",
            "health.patients.update",
            "health",
            "health:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/health/patients/{id}/care-items",
            "health.care_items.create",
            "health",
            "health:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/health/care-items/{id}",
            "health.care_items.update",
            "health",
            "health:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/health/visits",
            "health.visits.list",
            "health",
            "health:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/health/visits",
            "health.visits.create",
            "health",
            "health:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/health/visits/{id}",
            "health.visits.read",
            "health",
            "health:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/health/visits/{id}/close",
            "health.visits.close",
            "health",
            "health:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/health/medication-plans",
            "health.medication_plans.list",
            "health",
            "health:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/health/medication-plans",
            "health.medication_plans.create",
            "health",
            "health:medication",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/health/medication-plans/{id}",
            "health.medication_plans.update",
            "health",
            "health:medication",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/health/medication-administrations",
            "health.medication_administrations.list",
            "health",
            "health:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/health/medication-plans/{id}/administrations",
            "health.medication_administrations.create",
            "health",
            "health:medication",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/health/follow-ups",
            "health.follow_ups.list",
            "health",
            "health:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/health/follow-ups",
            "health.follow_ups.create",
            "health",
            "health:follow_up",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/health/follow-ups/{id}",
            "health.follow_ups.update",
            "health",
            "health:follow_up",
            OperationEffect::Write,
            true,
        ),
        // Library: catalogue, canonical members, circulation, holds, and fines.
        route(
            Method::GET,
            "/api/1.0/library/settings",
            "library.settings.read",
            "library",
            "library:manage",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/library/settings",
            "library.settings.update",
            "library",
            "library:manage",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/library/references",
            "library.references.read",
            "library",
            "library:manage",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/library/titles",
            "library.titles.list",
            "library",
            "library:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/library/titles",
            "library.titles.create",
            "library",
            "library:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/library/titles/{id}",
            "library.titles.read",
            "library",
            "library:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/library/titles/{id}",
            "library.titles.update",
            "library",
            "library:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/library/titles/{id}/retire",
            "library.titles.retire",
            "library",
            "library:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/library/titles/{id}/copies",
            "library.copies.list",
            "library",
            "library:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/library/titles/{id}/copies",
            "library.copies.create",
            "library",
            "library:create",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/library/copies/{id}",
            "library.copies.read",
            "library",
            "library:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/library/copies/{id}",
            "library.copies.update",
            "library",
            "library:edit",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/library/copies/{id}/retire",
            "library.copies.retire",
            "library",
            "library:delete",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/library/members",
            "library.members.list",
            "library",
            "library:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/library/members",
            "library.members.create",
            "library",
            "library:manage",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/library/members/{id}",
            "library.members.read",
            "library",
            "library:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::PUT,
            "/api/1.0/library/members/{id}",
            "library.members.update",
            "library",
            "library:manage",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/library/loans",
            "library.loans.list",
            "library",
            "library:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/library/loans",
            "library.loans.checkout",
            "library",
            "library:circulate",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/library/loans/{id}",
            "library.loans.read",
            "library",
            "library:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/library/loans/{id}/renew",
            "library.loans.renew",
            "library",
            "library:borrow",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/library/loans/{id}/return",
            "library.loans.return",
            "library",
            "library:circulate",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/library/loans/{id}/lost",
            "library.loans.mark_lost",
            "library",
            "library:circulate",
            OperationEffect::Destructive,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/library/loans/{id}/fines",
            "library.fines.assess",
            "library",
            "library:manage",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/library/holds",
            "library.holds.list",
            "library",
            "library:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/library/holds",
            "library.holds.place",
            "library",
            "library:borrow",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/library/holds/{id}",
            "library.holds.read",
            "library",
            "library:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/library/holds/{id}/ready",
            "library.holds.ready",
            "library",
            "library:circulate",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/library/holds/{id}/cancel",
            "library.holds.cancel",
            "library",
            "library:borrow",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/library/holds/{id}/expire",
            "library.holds.expire",
            "library",
            "library:circulate",
            OperationEffect::Write,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/library/fines",
            "library.fines.list",
            "library",
            "library:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::GET,
            "/api/1.0/library/fines/{id}",
            "library.fines.read",
            "library",
            "library:view",
            OperationEffect::Read,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/library/fines/{id}/submit-to-fees",
            "library.fines.submit_to_fees",
            "library",
            "library:manage",
            OperationEffect::External,
            true,
        ),
        route(
            Method::POST,
            "/api/1.0/library/fines/{id}/waive",
            "library.fines.waive",
            "library",
            "library:manage",
            OperationEffect::Write,
            true,
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn route(
    method: Method,
    route_pattern: &'static str,
    key: &'static str,
    module_key: &'static str,
    permission: &'static str,
    effect: OperationEffect,
    license_required: bool,
) -> RoutedOperation {
    let operation = ProductOperation::route(
        key,
        module_key,
        permission,
        effect,
        agent_exposure_for(key),
        license_required,
    );
    let operation = if key.starts_with("assets_inventory.goods_receipt_allocations.") {
        operation.requiring_modules(["procurement".to_string()])
    } else if key.starts_with("assets_inventory.stock_requests.")
        || key.starts_with("assets_inventory.stock_request_fulfilments.")
        || matches!(
            key,
            "assets_inventory.requester_candidates.list"
                | "assets_inventory.department_candidates.list"
        )
    {
        operation.requiring_modules(["hr_payroll".to_string()])
    } else if key.starts_with("administration.ai_providers.")
        || key.starts_with("administration.ai_routing.")
        || key.starts_with("administration.agent_governance.")
        || key.starts_with("administration.agent_usage.")
        || key.starts_with("administration.agent_audit.")
    {
        operation.requiring_modules(["agent".to_string()])
    } else if key.starts_with("sis.") && !key.starts_with("sis.learner_numbering.") {
        operation.requiring_modules(["academics".to_string()])
    } else if key.starts_with("academics.reporting.") {
        operation.requiring_modules([
            "attendance".to_string(),
            "hr_payroll".to_string(),
            "sis".to_string(),
        ])
    } else if key.starts_with("academics.gradebook.") {
        operation.requiring_modules(["sis".to_string(), "hr_payroll".to_string()])
    } else if key.starts_with("academics.teacher")
        || key.starts_with("academics.teaching_assignments")
        || key.starts_with("academics.assessment_components")
    {
        operation.requiring_modules(["hr_payroll".to_string()])
    } else if key.starts_with("timetabling.") {
        operation.requiring_modules(["academics".to_string(), "hr_payroll".to_string()])
    } else if key.starts_with("attendance.") {
        operation.requiring_modules(["academics".to_string(), "sis".to_string()])
    } else if key.starts_with("hostel.") {
        operation.requiring_modules(["sis".to_string()])
    } else if key.starts_with("health.") {
        operation.requiring_modules(["hr_payroll".to_string(), "sis".to_string()])
    } else if matches!(
        key,
        "messaging.references.read"
            | "messaging.announcements.create"
            | "messaging.announcements.update"
            | "messaging.announcements.audience_preview.read"
            | "messaging.announcements.submit"
    ) {
        operation.requiring_modules([
            "academics".to_string(),
            "sis".to_string(),
            "hr_payroll".to_string(),
        ])
    } else if key == "library.fines.submit_to_fees" {
        operation.requiring_modules([
            "fees".to_string(),
            "finance".to_string(),
            "hr_payroll".to_string(),
            "sis".to_string(),
        ])
    } else if key.starts_with("library.fines.") {
        operation.requiring_modules([
            "finance".to_string(),
            "hr_payroll".to_string(),
            "sis".to_string(),
        ])
    } else if key == "library.references.read" {
        operation.requiring_modules([
            "fees".to_string(),
            "finance".to_string(),
            "hr_payroll".to_string(),
            "sis".to_string(),
        ])
    } else if matches!(
        key,
        "library.settings.read"
            | "library.settings.update"
            | "library.titles.read"
            | "library.titles.create"
            | "library.titles.update"
    ) {
        operation.requiring_modules(["finance".to_string()])
    } else if key.starts_with("library.members.")
        || key.starts_with("library.loans.")
        || key.starts_with("library.holds.")
    {
        operation.requiring_modules(["hr_payroll".to_string(), "sis".to_string()])
    } else if key.starts_with("fees.") {
        operation.requiring_modules([
            "sis".to_string(),
            "academics".to_string(),
            "finance".to_string(),
        ])
    } else if key.starts_with("procurement.") {
        operation.requiring_modules(["hr_payroll".to_string(), "finance".to_string()])
    } else if key.starts_with("fleet.") {
        operation.requiring_modules(["hr_payroll".to_string()])
    } else {
        operation
    };
    RoutedOperation {
        method,
        route_pattern,
        operation,
        authority: RouteAuthority::Permission,
    }
}

fn authenticated_route(
    method: Method,
    route_pattern: &'static str,
    key: &'static str,
    module_key: &'static str,
    permission: &'static str,
    effect: OperationEffect,
) -> RoutedOperation {
    RoutedOperation {
        method,
        route_pattern,
        operation: ProductOperation::route(
            key,
            module_key,
            permission,
            effect,
            agent_exposure_for(key),
            false,
        ),
        authority: RouteAuthority::Authenticated,
    }
}

fn agent_exposure_for(key: &'static str) -> AgentExposure {
    match key {
        "administration.catalog.read"
        | "administration.modules.list"
        | "administration.licensing.read"
        | "administration.school_settings.read"
        | "administration.ai_providers.catalog.list"
        | "administration.ai_providers.connections.list"
        | "administration.ai_providers.connections.read"
        | "administration.ai_providers.models.list"
        | "administration.ai_routing.routes.list"
        | "administration.ai_routing.routes.options"
        | "administration.ai_routing.routes.read"
        | "administration.ai_routing.routes.resolve"
        | "administration.roles.list"
        | "administration.roles.read"
        | "administration.users.list"
        | "administration.users.read"
        | "sis.learner_numbering.read"
        | "sis.account_candidates.list"
        | "sis.imports.list"
        | "sis.imports.read"
        | "sis.imports.preview.read"
        | "sis.learners.list"
        | "sis.learners.read"
        | "sis.guardians.list"
        | "sis.guardians.read"
        | "sis.guardian_relationships.list"
        | "sis.guardian_relationships.read"
        | "sis.applications.list"
        | "sis.applications.read"
        | "sis.enrolments.list"
        | "sis.enrolments.read"
        | "academics.academic_years.list"
        | "academics.academic_years.read"
        | "academics.terms.list"
        | "academics.terms.read"
        | "academics.subjects.list"
        | "academics.subjects.read"
        | "academics.grade_levels.list"
        | "academics.grade_levels.read"
        | "academics.teacher_candidates.list"
        | "academics.teachers.list"
        | "academics.teachers.read"
        | "academics.classes.list"
        | "academics.classes.read"
        | "academics.teaching_assignments.list"
        | "academics.teaching_assignments.read"
        | "academics.assessment_cycles.list"
        | "academics.assessment_cycles.read"
        | "academics.assessment_components.list"
        | "academics.assessment_components.read"
        | "academics.gradebook.references.read"
        | "academics.gradebook.mark_sheets.list"
        | "academics.gradebook.mark_sheets.read"
        | "academics.reporting.references.read"
        | "academics.reporting.grading_schemes.list"
        | "academics.reporting.grading_schemes.read"
        | "academics.reporting.report_batches.list"
        | "academics.reporting.report_batches.read"
        | "academics.reporting.transcripts.read"
        | "finance.currencies.list"
        | "finance.currencies.read"
        | "finance.accounts.list"
        | "finance.accounts.read"
        | "finance.fiscal_years.list"
        | "finance.fiscal_years.read"
        | "finance.accounting_periods.list"
        | "finance.journals.list"
        | "finance.journals.read"
        | "finance.journals.validation.read"
        | "finance.posting_requests.list"
        | "finance.posting_requests.read"
        | "fees.reference_data.read"
        | "fees.learner_candidates.list"
        | "fees.imports.list"
        | "fees.imports.read"
        | "fees.imports.preview.read"
        | "fees.billing_accounts.list"
        | "fees.billing_accounts.read"
        | "fees.fee_structures.list"
        | "fees.fee_structures.read"
        | "fees.invoices.list"
        | "fees.invoices.read"
        | "procurement.reference_data.read"
        | "procurement.requester_candidates.list"
        | "procurement.suppliers.list"
        | "procurement.suppliers.read"
        | "procurement.requisitions.list"
        | "procurement.requisitions.read"
        | "procurement.purchase_orders.list"
        | "procurement.purchase_orders.read"
        | "procurement.goods_receipts.list"
        | "procurement.goods_receipts.read"
        | "assets_inventory.items.list"
        | "assets_inventory.items.read"
        | "assets_inventory.stores.list"
        | "assets_inventory.stores.read"
        | "assets_inventory.stock_balances.list"
        | "assets_inventory.stock_movements.list"
        | "assets_inventory.stock_movements.read"
        | "assets_inventory.goods_receipt_allocations.list"
        | "assets_inventory.requester_candidates.list"
        | "assets_inventory.department_candidates.list"
        | "assets_inventory.stock_requests.list"
        | "assets_inventory.stock_requests.read"
        | "assets_inventory.stock_requests.fulfilment_preview.read"
        | "hr_payroll.imports.list"
        | "hr_payroll.imports.read"
        | "hr_payroll.imports.preview.read"
        | "hr_payroll.departments.list"
        | "hr_payroll.departments.read"
        | "hr_payroll.positions.list"
        | "hr_payroll.positions.read"
        | "hr_payroll.employees.list"
        | "hr_payroll.employees.read"
        | "hr_payroll.employment_engagements.list"
        | "hr_payroll.employment_engagements.read"
        | "hr_payroll.availability.list"
        | "hr_payroll.availability.read"
        | "fleet.driver_candidates.list"
        | "fleet.vehicles.list"
        | "fleet.vehicles.read"
        | "fleet.drivers.list"
        | "fleet.drivers.read"
        | "fleet.vehicle_logs.list"
        | "fleet.vehicle_logs.read"
        | "timetabling.configuration.read"
        | "timetabling.runs.list"
        | "timetabling.runs.read"
        | "timetabling.runs.read_latest"
        | "attendance.references.read"
        | "attendance.registers.list"
        | "attendance.registers.read"
        | "messaging.references.read"
        | "messaging.announcements.list"
        | "messaging.announcements.read"
        | "messaging.announcements.audience_preview.read"
        | "messaging.deliveries.list"
        | "messaging.inbox.list"
        | "messaging.inbox.read"
        | "library.settings.read"
        | "library.references.read"
        | "library.titles.list"
        | "library.titles.read"
        | "library.copies.list"
        | "library.copies.read"
        | "library.members.list"
        | "library.members.read"
        | "library.loans.list"
        | "library.loans.read"
        | "library.holds.list"
        | "library.holds.read"
        | "library.fines.list"
        | "library.fines.read"
        | "health.references.read"
        | "health.patients.list"
        | "health.patients.read"
        | "health.visits.list"
        | "health.visits.read"
        | "health.medication_plans.list"
        | "health.medication_administrations.list"
        | "health.follow_ups.list" => AgentExposure::Exposed,
        "document_registry.numbering_policy.read"
        | "document_registry.series.list"
        | "document_registry.series.read"
        | "document_registry.files.list"
        | "document_registry.files.read"
        | "document_registry.files.activity.list"
        | "document_registry.retention_due.list"
        | "document_registry.disposition_reviews.list"
        | "document_registry.disposition_reviews.read" => AgentExposure::Exposed,
        "hostel.references.read"
        | "hostel.residences.list"
        | "hostel.residences.read"
        | "hostel.rooms.list"
        | "hostel.rooms.read"
        | "hostel.allocations.preview"
        | "hostel.allocations.list"
        | "hostel.allocations.read"
        | "hostel.allocations.transfer_preview"
        | "hostel.pastoral_records.list"
        | "hostel.pastoral_records.read" => AgentExposure::Exposed,
        "administration.school_settings.update"
        | "administration.school_settings.update_logo"
        | "administration.ai_providers.connections.update"
        | "administration.ai_providers.connections.test"
        | "administration.ai_providers.models.refresh"
        | "administration.ai_routing.routes.create"
        | "administration.ai_routing.routes.update"
        | "administration.ai_routing.routes.archive"
        | "administration.users.create"
        | "administration.users.update"
        | "administration.users.activate"
        | "administration.users.deactivate"
        | "administration.licensing.refresh"
        | "administration.licensing.disable_module"
        | "sis.learner_numbering.update"
        | "sis.learners.create"
        | "sis.imports.upload"
        | "sis.imports.preview"
        | "sis.imports.commit"
        | "sis.learners.update"
        | "sis.learners.link_account"
        | "sis.learners.delete"
        | "sis.guardians.create"
        | "sis.guardians.update"
        | "sis.guardians.link_account"
        | "sis.guardians.delete"
        | "sis.guardian_relationships.create"
        | "sis.guardian_relationships.update"
        | "sis.guardian_relationships.delete"
        | "sis.applications.create"
        | "sis.applications.update"
        | "sis.applications.delete"
        | "sis.enrolments.create"
        | "sis.enrolments.update"
        | "academics.academic_years.create"
        | "academics.academic_years.update"
        | "academics.academic_years.delete"
        | "academics.terms.create"
        | "academics.terms.update"
        | "academics.terms.delete"
        | "academics.subjects.create"
        | "academics.subjects.update"
        | "academics.subjects.delete"
        | "academics.grade_levels.create"
        | "academics.grade_levels.update"
        | "academics.grade_levels.delete"
        | "academics.teachers.create"
        | "academics.teachers.update"
        | "academics.teachers.delete"
        | "academics.classes.create"
        | "academics.classes.update"
        | "academics.classes.delete"
        | "academics.teaching_assignments.create"
        | "academics.teaching_assignments.update"
        | "academics.teaching_assignments.delete"
        | "academics.assessment_cycles.create"
        | "academics.assessment_cycles.update"
        | "academics.assessment_cycles.delete"
        | "academics.assessment_components.create"
        | "academics.assessment_components.update"
        | "academics.assessment_components.delete"
        | "academics.gradebook.mark_sheets.create"
        | "academics.gradebook.mark_sheets.marks.update"
        | "academics.gradebook.mark_sheets.submit"
        | "academics.gradebook.mark_sheets.publish"
        | "academics.gradebook.mark_sheets.reopen"
        | "academics.gradebook.mark_sheets.delete"
        | "academics.reporting.grading_schemes.create"
        | "academics.reporting.grading_schemes.update"
        | "academics.reporting.grading_schemes.retire"
        | "academics.reporting.grading_schemes.delete"
        | "academics.reporting.report_batches.generate"
        | "academics.reporting.report_cards.teacher_comment.update"
        | "academics.reporting.report_cards.review.update"
        | "academics.reporting.report_batches.review"
        | "academics.reporting.report_batches.publish"
        | "academics.reporting.report_batches.reopen"
        | "academics.reporting.report_batches.delete"
        | "finance.currencies.create"
        | "finance.currencies.update"
        | "finance.currencies.delete"
        | "finance.accounts.create"
        | "finance.accounts.update"
        | "finance.accounts.delete"
        | "finance.fiscal_years.create"
        | "finance.fiscal_years.update"
        | "finance.fiscal_years.delete"
        | "finance.fiscal_years.open"
        | "finance.fiscal_years.close"
        | "finance.accounting_periods.close"
        | "finance.accounting_periods.reopen"
        | "finance.journals.create"
        | "finance.journals.update"
        | "finance.journals.delete"
        | "finance.journals.submit"
        | "finance.journals.approve"
        | "finance.journals.reject"
        | "finance.journals.post"
        | "finance.journals.reverse"
        | "finance.posting_requests.convert"
        | "finance.posting_requests.reject"
        | "fees.imports.upload"
        | "fees.imports.preview"
        | "fees.imports.commit"
        | "fees.billing_accounts.create"
        | "fees.billing_accounts.update"
        | "fees.fee_structures.create"
        | "fees.fee_structures.update"
        | "fees.fee_structures.delete"
        | "fees.fee_structures.activate"
        | "fees.fee_structures.retire"
        | "fees.invoices.create"
        | "fees.invoices.issue"
        | "fees.invoices.delete"
        | "procurement.suppliers.create"
        | "procurement.suppliers.update"
        | "procurement.suppliers.delete"
        | "procurement.requisitions.create"
        | "procurement.requisitions.update"
        | "procurement.requisitions.delete"
        | "procurement.requisitions.submit"
        | "procurement.requisitions.approve"
        | "procurement.requisitions.reject"
        | "procurement.requisitions.cancel"
        | "procurement.purchase_orders.create"
        | "procurement.purchase_orders.update"
        | "procurement.purchase_orders.issue"
        | "procurement.purchase_orders.cancel"
        | "procurement.goods_receipts.create"
        | "procurement.goods_receipts.update"
        | "procurement.goods_receipts.post"
        | "assets_inventory.items.create"
        | "assets_inventory.items.update"
        | "assets_inventory.items.delete"
        | "assets_inventory.stores.create"
        | "assets_inventory.stores.update"
        | "assets_inventory.stores.delete"
        | "assets_inventory.manual_receipts.create"
        | "assets_inventory.issues.create"
        | "assets_inventory.transfers.create"
        | "assets_inventory.adjustments.create"
        | "assets_inventory.stock_movements.reverse"
        | "assets_inventory.goods_receipt_allocations.create"
        | "assets_inventory.stock_requests.create"
        | "assets_inventory.stock_requests.update"
        | "assets_inventory.stock_requests.delete"
        | "assets_inventory.stock_requests.submit"
        | "assets_inventory.stock_requests.cancel"
        | "assets_inventory.stock_requests.approve"
        | "assets_inventory.stock_requests.reject"
        | "assets_inventory.stock_requests.close"
        | "assets_inventory.stock_request_fulfilments.create"
        | "hr_payroll.imports.upload"
        | "hr_payroll.imports.preview"
        | "hr_payroll.imports.commit"
        | "hr_payroll.departments.create"
        | "hr_payroll.departments.update"
        | "hr_payroll.departments.delete"
        | "hr_payroll.positions.create"
        | "hr_payroll.positions.update"
        | "hr_payroll.positions.delete"
        | "hr_payroll.employees.create"
        | "hr_payroll.employees.update"
        | "hr_payroll.employees.link_account"
        | "hr_payroll.employees.delete"
        | "hr_payroll.employment_engagements.create"
        | "hr_payroll.employment_engagements.update"
        | "hr_payroll.employment_engagements.delete"
        | "hr_payroll.availability.create"
        | "hr_payroll.availability.update"
        | "hr_payroll.availability.delete"
        | "fleet.vehicles.create"
        | "fleet.vehicles.update"
        | "fleet.vehicles.delete"
        | "fleet.drivers.create"
        | "fleet.drivers.update"
        | "fleet.drivers.delete"
        | "fleet.vehicle_logs.create"
        | "fleet.vehicle_logs.update"
        | "fleet.vehicle_logs.delete"
        | "timetabling.configuration.update"
        | "timetabling.runs.generate"
        | "timetabling.runs.publish"
        | "attendance.registers.create"
        | "attendance.registers.marks.update"
        | "attendance.registers.submit"
        | "attendance.registers.reopen"
        | "attendance.registers.delete"
        | "messaging.announcements.create"
        | "messaging.announcements.update"
        | "messaging.announcements.submit"
        | "messaging.announcements.reopen"
        | "messaging.announcements.publish"
        | "messaging.announcements.cancel"
        | "messaging.announcements.delete"
        | "messaging.inbox.mark_read"
        | "library.settings.update"
        | "library.titles.create"
        | "library.titles.update"
        | "library.titles.retire"
        | "library.copies.create"
        | "library.copies.update"
        | "library.copies.retire"
        | "library.members.create"
        | "library.members.update"
        | "library.loans.checkout"
        | "library.loans.renew"
        | "library.loans.return"
        | "library.loans.mark_lost"
        | "library.holds.place"
        | "library.holds.ready"
        | "library.holds.cancel"
        | "library.holds.expire"
        | "library.fines.assess"
        | "library.fines.submit_to_fees"
        | "library.fines.waive"
        | "health.patients.create"
        | "health.patients.update"
        | "health.care_items.create"
        | "health.care_items.update"
        | "health.visits.create"
        | "health.visits.close"
        | "health.medication_plans.create"
        | "health.medication_plans.update"
        | "health.medication_administrations.create"
        | "health.follow_ups.create"
        | "health.follow_ups.update" => AgentExposure::ApprovalRequired,
        "document_registry.numbering_policy.update"
        | "document_registry.series.create"
        | "document_registry.series.update"
        | "document_registry.files.update"
        | "document_registry.files.reclassify"
        | "document_registry.files.close"
        | "document_registry.disposition_reviews.create"
        | "document_registry.disposition_reviews.approve"
        | "document_registry.disposition_reviews.reject"
        | "document_registry.disposition_reviews.execute" => AgentExposure::ApprovalRequired,
        "hostel.residences.create"
        | "hostel.residences.update"
        | "hostel.rooms.create"
        | "hostel.rooms.update"
        | "hostel.allocations.create"
        | "hostel.allocations.activate"
        | "hostel.allocations.end"
        | "hostel.allocations.cancel"
        | "hostel.allocations.transfer"
        | "hostel.pastoral_records.create"
        | "hostel.pastoral_records.update"
        | "hostel.pastoral_records.resolve" => AgentExposure::ApprovalRequired,
        "document_registry.files.create" => AgentExposure::HumanOnly {
            reason: "Private document bytes must be selected and security-scanned in a direct human workflow.",
        },
        "document_registry.files.download" => AgentExposure::HumanOnly {
            reason: "Private document bytes are never returned to an Agent provider.",
        },
        "administration.ai_providers.connections.create"
        | "administration.ai_providers.connections.data_approval.update"
        | "administration.ai_providers.credentials.rotate" => AgentExposure::HumanOnly {
            reason: "Provider credential and data-approval decisions remain direct human workflows.",
        },
        "administration.ai_providers.connections.disconnect" => AgentExposure::HumanOnly {
            reason: "Destructive provider disconnect and credential purge remain a direct human workflow.",
        },
        "administration.roles.create"
        | "administration.roles.update"
        | "administration.roles.delete" => AgentExposure::HumanOnly {
            reason: "Role definition changes remain a direct human workflow.",
        },
        "administration.users.delete" => AgentExposure::HumanOnly {
            reason: "Permanent account deletion remains a direct human workflow.",
        },
        "administration.licensing.activate_legacy_key"
        | "administration.licensing.connect"
        | "administration.licensing.import_offline_lease" => AgentExposure::HumanOnly {
            reason: "License credential entry remains a direct human workflow.",
        },
        "administration.agent_governance.readiness"
        | "administration.agent_governance.capabilities.list"
        | "administration.agent_usage.options"
        | "administration.agent_usage.report"
        | "administration.agent_usage.export"
        | "administration.agent_audit.runs.list"
        | "administration.agent_audit.runs.read" => AgentExposure::HumanOnly {
            reason: "Agent governance, campus usage, and run audit evidence remain direct human workflows.",
        },
        "agent.sessions.list"
        | "agent.sessions.create"
        | "agent.sessions.read"
        | "agent.sessions.update"
        | "agent.sessions.archive"
        | "agent.messages.list"
        | "agent.messages.submit"
        | "agent.runs.list"
        | "agent.runs.read"
        | "agent.runs.cancel"
        | "agent.runs.events.list"
        | "agent.usage.personal.read" => AgentExposure::Prohibited {
            reason: "Agent Session control-plane operations cannot invoke themselves as capabilities.",
        },
        _ => panic!("Product operation {key} has no Agent exposure classification"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use actix_web::{App, HttpRequest, HttpResponse, http::Method, test as actix_test, web};

    use crate::{
        AgentExposure, EntitlementSnapshot, LeaseLifecycle, ModuleEntitlementState,
        OperationEffect, ProductOperation, RuntimeAccessChecks, evaluate_operation,
    };

    use super::{
        OPERATION_CATALOG_VERSION, PRODUCT_CATALOG_VERSION, RouteAuthority,
        SUPPORTED_PRODUCT_CATALOG_VERSIONS, operation_catalog, operation_for_route,
        routed_operation_for_route,
    };

    fn operation(key: &str) -> &'static ProductOperation {
        operation_catalog()
            .iter()
            .find(|entry| entry.operation().key() == key)
            .map(super::RoutedOperation::operation)
            .unwrap_or_else(|| unreachable!())
    }

    fn routed_operation(key: &str) -> &'static super::RoutedOperation {
        operation_catalog()
            .iter()
            .find(|entry| entry.operation().key() == key)
            .unwrap_or_else(|| unreachable!())
    }

    fn active_snapshot() -> EntitlementSnapshot {
        EntitlementSnapshot::new(
            LeaseLifecycle::Active,
            vec![
                (
                    "administration".to_string(),
                    ModuleEntitlementState::Enabled,
                ),
                ("academics".to_string(), ModuleEntitlementState::Enabled),
                ("sis".to_string(), ModuleEntitlementState::Enabled),
                ("hr_payroll".to_string(), ModuleEntitlementState::Enabled),
                ("fleet".to_string(), ModuleEntitlementState::Enabled),
                ("timetabling".to_string(), ModuleEntitlementState::Enabled),
                ("attendance".to_string(), ModuleEntitlementState::Enabled),
                ("messaging".to_string(), ModuleEntitlementState::Enabled),
                ("library".to_string(), ModuleEntitlementState::Enabled),
                ("hostel".to_string(), ModuleEntitlementState::Enabled),
                ("health".to_string(), ModuleEntitlementState::Enabled),
                (
                    "document_registry".to_string(),
                    ModuleEntitlementState::Enabled,
                ),
                ("finance".to_string(), ModuleEntitlementState::Enabled),
                ("fees".to_string(), ModuleEntitlementState::Enabled),
                ("procurement".to_string(), ModuleEntitlementState::Enabled),
                (
                    "assets_inventory".to_string(),
                    ModuleEntitlementState::Enabled,
                ),
                ("agent".to_string(), ModuleEntitlementState::Enabled),
            ],
            vec![],
        )
        .unwrap_or_else(|_| unreachable!())
    }

    fn allowed(key: &str, permissions: &[&str]) -> bool {
        evaluate_operation(
            operation(key),
            &active_snapshot(),
            &permissions
                .iter()
                .map(|permission| permission.to_string())
                .collect::<Vec<_>>(),
            RuntimeAccessChecks::default(),
        )
        .allowed
    }

    #[test]
    fn catalog_has_unique_stable_keys_and_route_identities() {
        assert_eq!(OPERATION_CATALOG_VERSION, 2);
        assert_eq!(
            PRODUCT_CATALOG_VERSION,
            format!("campus-pilot/{OPERATION_CATALOG_VERSION}")
        );
        assert!(SUPPORTED_PRODUCT_CATALOG_VERSIONS.contains(&PRODUCT_CATALOG_VERSION));
        assert_eq!(operation_catalog().len(), 462);

        let mut keys = BTreeSet::new();
        let mut routes = BTreeSet::new();
        for entry in operation_catalog() {
            let key = entry.operation().key();
            assert!(
                key.split('.').all(|part| {
                    !part.is_empty()
                        && part
                            .chars()
                            .all(|character| character.is_ascii_lowercase() || character == '_')
                }),
                "invalid operation key: {key}"
            );
            assert!(keys.insert(key), "duplicate operation key: {key}");

            let route = (entry.method().as_str(), entry.route_pattern());
            assert!(
                routes.insert(route),
                "duplicate routed operation: {route:?}"
            );
        }
    }

    #[test]
    fn every_operation_has_an_explicit_agent_exposure_classification() {
        let mut counts = [0_u32; 4];

        for entry in operation_catalog() {
            match entry.operation().agent_exposure() {
                AgentExposure::Exposed => counts[0] += 1,
                AgentExposure::ApprovalRequired => counts[1] += 1,
                AgentExposure::HumanOnly { reason } => {
                    assert!(
                        !reason.trim().is_empty(),
                        "human-only operation {} must explain why",
                        entry.operation().key()
                    );
                    counts[2] += 1;
                }
                AgentExposure::Prohibited { reason } => {
                    assert!(
                        !reason.trim().is_empty(),
                        "prohibited operation {} must explain why",
                        entry.operation().key()
                    );
                    counts[3] += 1;
                }
            }
        }

        assert_eq!(counts, [181, 249, 20, 12]);
        assert_eq!(counts.iter().sum::<u32>(), operation_catalog().len() as u32);
    }

    #[test]
    #[should_panic(
        expected = "Product operation test.unclassified has no Agent exposure classification"
    )]
    fn unclassified_agent_operation_fails_the_code_owned_catalog() {
        super::agent_exposure_for("test.unclassified");
    }

    #[test]
    fn only_launcher_discovery_uses_authenticated_authority() {
        let authenticated = operation_catalog()
            .iter()
            .filter(|entry| entry.authority() == RouteAuthority::Authenticated)
            .map(|entry| entry.operation().key())
            .collect::<Vec<_>>();
        assert_eq!(
            authenticated,
            vec!["administration.catalog.read", "administration.modules.list"]
        );

        for (method, pattern) in [
            (Method::GET, "/api/1.0/access/licensing"),
            (Method::GET, "/api/1.0/kernel/school-profile"),
            (Method::PUT, "/api/1.0/kernel/school-profile"),
            (Method::POST, "/api/1.0/kernel/school-profile/logo"),
        ] {
            assert_eq!(
                routed_operation_for_route(&method, pattern)
                    .unwrap_or_else(|| unreachable!())
                    .authority(),
                RouteAuthority::Permission
            );
        }
    }

    #[test]
    fn exact_route_resolution_distinguishes_collection_and_record_reads() {
        let list =
            operation_for_route(&Method::GET, "/api/1.0/users").unwrap_or_else(|| unreachable!());
        let read = operation_for_route(&Method::GET, "/api/1.0/users/{id}")
            .unwrap_or_else(|| unreachable!());

        assert_eq!(list.key(), "administration.users.list");
        assert_eq!(read.key(), "administration.users.read");
        assert!(operation_for_route(&Method::PATCH, "/api/1.0/users/{id}").is_none());
    }

    #[test]
    fn seeded_and_custom_role_permissions_intersect_exact_operations() {
        let school_administrator = [
            "administration:view",
            "users:view",
            "users:create",
            "users:edit",
            "roles:view",
            "roles:create",
            "roles:edit",
            "roles:assign",
            "licensing:view",
            "licensing:edit",
            "licensing:delete",
            "school_settings:view",
            "school_settings:edit",
        ];
        assert!(allowed("administration.users.list", &school_administrator));
        assert!(allowed(
            "administration.licensing.refresh",
            &school_administrator
        ));
        assert!(!allowed(
            "administration.users.delete",
            &school_administrator
        ));
        assert!(!allowed("fleet.vehicles.list", &school_administrator));

        let teacher = [
            "academics:view",
            "academics:edit",
            "sis:view",
            "timetabling:view",
            "messaging:view",
            "messaging:create",
            "library:view",
        ];
        assert!(allowed("timetabling.configuration.read", &teacher));
        assert!(!allowed("timetabling.runs.generate", &teacher));
        assert!(!allowed("administration.users.list", &teacher));

        let student = [
            "academics:view",
            "timetabling:view",
            "fees:view",
            "library:view",
            "messaging:view",
        ];
        assert!(allowed("timetabling.runs.read_latest", &student));
        assert!(!allowed("timetabling.configuration.update", &student));

        let fleet_viewer = ["fleet:view"];
        assert!(allowed("fleet.vehicles.list", &fleet_viewer));
        assert!(allowed("fleet.vehicle_logs.read", &fleet_viewer));
        assert!(!allowed("fleet.vehicles.create", &fleet_viewer));
        assert!(!allowed("fleet.vehicle_logs.delete", &fleet_viewer));

        let hr_viewer = ["hr_payroll:view"];
        assert!(allowed("hr_payroll.employees.list", &hr_viewer));
        assert!(allowed("hr_payroll.departments.read", &hr_viewer));
        assert!(allowed(
            "hr_payroll.employment_engagements.list",
            &hr_viewer
        ));
        assert!(allowed("hr_payroll.availability.read", &hr_viewer));
        assert!(!allowed("hr_payroll.employees.link_account", &hr_viewer));

        let procurement_viewer = ["procurement:view"];
        assert!(allowed(
            "procurement.requisitions.list",
            &procurement_viewer
        ));
        assert!(allowed("procurement.suppliers.read", &procurement_viewer));
        assert!(allowed(
            "procurement.purchase_orders.list",
            &procurement_viewer
        ));
        assert!(allowed(
            "procurement.goods_receipts.read",
            &procurement_viewer
        ));
        assert!(!allowed(
            "procurement.requisitions.create",
            &procurement_viewer
        ));
    }

    #[test]
    fn run_capable_user_can_read_owned_work_without_history_management() {
        let permissions = ["agent:view", "agent:run"];
        for key in [
            "agent.sessions.create",
            "agent.sessions.read",
            "agent.messages.list",
            "agent.messages.submit",
            "agent.runs.list",
            "agent.runs.read",
            "agent.runs.cancel",
            "agent.runs.events.list",
            "agent.usage.personal.read",
        ] {
            assert!(allowed(key, &permissions), "{key} should be allowed");
        }
        for key in [
            "agent.sessions.list",
            "agent.sessions.update",
            "agent.sessions.archive",
        ] {
            assert!(!allowed(key, &permissions), "{key} requires agent:history");
        }
    }

    #[test]
    fn learner_numbering_is_sis_only_and_uses_exact_permissions() {
        let read = operation("sis.learner_numbering.read");
        let update = operation("sis.learner_numbering.update");
        assert_eq!(read.required_modules().count(), 0);
        assert_eq!(update.required_modules().count(), 0);
        assert!(read.license_required());
        assert!(update.license_required());
        assert!(allowed("sis.learner_numbering.read", &["sis:view"]));
        assert!(!allowed("sis.learner_numbering.update", &["sis:view"]));
        assert!(allowed("sis.learner_numbering.update", &["sis:edit"]));
        assert!(matches!(
            update.agent_exposure(),
            AgentExposure::ApprovalRequired
        ));
    }

    #[test]
    fn library_operations_are_fully_governed_and_dependency_typed() {
        let operations = operation_catalog()
            .iter()
            .filter(|entry| entry.operation().key().starts_with("library."))
            .collect::<Vec<_>>();
        assert_eq!(operations.len(), 34);
        for entry in operations {
            let operation = entry.operation();
            assert_eq!(operation.module_key(), "library", "{}", operation.key());
            assert!(operation.license_required(), "{}", operation.key());
        }

        assert_eq!(
            operation("library.references.read")
                .required_modules()
                .collect::<Vec<_>>(),
            vec!["fees", "finance", "hr_payroll", "sis"]
        );
        assert_eq!(
            operation("library.fines.submit_to_fees")
                .required_modules()
                .collect::<Vec<_>>(),
            vec!["fees", "finance", "hr_payroll", "sis"]
        );
        assert_eq!(
            operation("library.loans.renew").permission(),
            "library:borrow"
        );
        assert_eq!(
            operation("library.loans.checkout").permission(),
            "library:circulate"
        );
        assert_eq!(
            operation("library.titles.list").agent_exposure(),
            AgentExposure::Exposed
        );
        assert_eq!(
            operation("library.titles.create").agent_exposure(),
            AgentExposure::ApprovalRequired
        );
    }

    #[test]
    fn gradebook_requires_school_records_and_staff_entitlements() {
        let publish = operation("academics.gradebook.mark_sheets.publish");
        assert_eq!(publish.permission(), "academics:manage");
        assert_eq!(
            publish.required_modules().collect::<Vec<_>>(),
            vec!["hr_payroll", "sis"]
        );
        assert!(matches!(
            publish.agent_exposure(),
            AgentExposure::ApprovalRequired
        ));

        let academics_only = EntitlementSnapshot::new(
            LeaseLifecycle::Active,
            [("academics".to_string(), ModuleEntitlementState::Enabled)],
            Vec::<String>::new(),
        )
        .unwrap_or_else(|_| unreachable!());
        let denied = evaluate_operation(
            publish,
            &academics_only,
            &["academics:manage".to_string()],
            RuntimeAccessChecks::default(),
        );
        assert!(!denied.allowed);
        assert_eq!(denied.reason.as_str(), "dependency_missing");

        let complete = EntitlementSnapshot::new(
            LeaseLifecycle::Active,
            [
                ("academics".to_string(), ModuleEntitlementState::Enabled),
                ("hr_payroll".to_string(), ModuleEntitlementState::Enabled),
                ("sis".to_string(), ModuleEntitlementState::Enabled),
            ],
            Vec::<String>::new(),
        )
        .unwrap_or_else(|_| unreachable!());
        let allowed = evaluate_operation(
            publish,
            &complete,
            &["academics:manage".to_string()],
            RuntimeAccessChecks::default(),
        );
        assert!(allowed.allowed);
    }

    #[test]
    fn procurement_requires_hr_and_finance_entitlements() {
        let operation = operation("procurement.requisitions.list");
        assert_eq!(
            operation.required_modules().collect::<Vec<_>>(),
            vec!["finance", "hr_payroll"]
        );

        for modules in [
            vec![("procurement".to_string(), ModuleEntitlementState::Enabled)],
            vec![
                ("procurement".to_string(), ModuleEntitlementState::Enabled),
                ("hr_payroll".to_string(), ModuleEntitlementState::Enabled),
            ],
            vec![
                ("procurement".to_string(), ModuleEntitlementState::Enabled),
                ("finance".to_string(), ModuleEntitlementState::Enabled),
            ],
        ] {
            let snapshot = EntitlementSnapshot::new(LeaseLifecycle::Active, modules, vec![])
                .unwrap_or_else(|_| unreachable!());
            let decision = evaluate_operation(
                operation,
                &snapshot,
                &["procurement:view".to_string()],
                RuntimeAccessChecks::default(),
            );
            assert!(!decision.allowed);
            assert_eq!(decision.reason.as_str(), "dependency_missing");
        }
    }

    #[test]
    fn fleet_requires_the_hr_employee_system_of_record() {
        for key in [
            "fleet.vehicles.list",
            "fleet.vehicles.read",
            "fleet.vehicles.create",
            "fleet.vehicles.update",
            "fleet.vehicles.delete",
            "fleet.driver_candidates.list",
            "fleet.drivers.list",
            "fleet.drivers.read",
            "fleet.drivers.create",
            "fleet.drivers.update",
            "fleet.drivers.delete",
            "fleet.vehicle_logs.list",
            "fleet.vehicle_logs.read",
            "fleet.vehicle_logs.create",
            "fleet.vehicle_logs.update",
            "fleet.vehicle_logs.delete",
        ] {
            assert_eq!(
                operation(key).required_modules().collect::<Vec<_>>(),
                vec!["hr_payroll"],
                "{key}"
            );
        }

        let fleet_only = EntitlementSnapshot::new(
            LeaseLifecycle::Active,
            [("fleet".to_string(), ModuleEntitlementState::Enabled)],
            Vec::<String>::new(),
        )
        .unwrap_or_else(|_| unreachable!());
        let denied = evaluate_operation(
            operation("fleet.vehicles.list"),
            &fleet_only,
            &["fleet:view".to_string()],
            RuntimeAccessChecks::default(),
        );
        assert!(!denied.allowed);
        assert_eq!(denied.reason.as_str(), "dependency_missing");
    }

    #[test]
    fn procurement_purchase_and_receiving_operations_are_fully_governed() {
        for (key, exposure) in [
            ("procurement.purchase_orders.list", "exposed"),
            ("procurement.purchase_orders.read", "exposed"),
            ("procurement.purchase_orders.create", "approval_required"),
            ("procurement.purchase_orders.update", "approval_required"),
            ("procurement.purchase_orders.issue", "approval_required"),
            ("procurement.purchase_orders.cancel", "approval_required"),
            ("procurement.goods_receipts.list", "exposed"),
            ("procurement.goods_receipts.read", "exposed"),
            ("procurement.goods_receipts.create", "approval_required"),
            ("procurement.goods_receipts.update", "approval_required"),
            ("procurement.goods_receipts.post", "approval_required"),
        ] {
            let operation = operation(key);
            assert_eq!(operation.module_key(), "procurement", "{key}");
            assert!(operation.license_required(), "{key}");
            assert_eq!(
                operation.required_modules().collect::<Vec<_>>(),
                vec!["finance", "hr_payroll"],
                "{key}"
            );
            assert_eq!(operation.agent_exposure().as_str(), exposure, "{key}");
        }
    }

    #[test]
    fn procurement_workflow_permissions_override_http_verb_defaults() {
        for (key, permission) in [
            (
                "procurement.requester_candidates.list",
                "procurement:create",
            ),
            ("procurement.requisitions.create", "procurement:create"),
            ("procurement.requisitions.update", "procurement:edit"),
            ("procurement.requisitions.delete", "procurement:delete"),
            ("procurement.requisitions.submit", "procurement:edit"),
            ("procurement.requisitions.cancel", "procurement:edit"),
            ("procurement.requisitions.approve", "procurement:approve"),
            ("procurement.requisitions.reject", "procurement:approve"),
            ("procurement.purchase_orders.create", "procurement:create"),
            ("procurement.purchase_orders.update", "procurement:edit"),
            ("procurement.purchase_orders.issue", "procurement:approve"),
            ("procurement.purchase_orders.cancel", "procurement:approve"),
            ("procurement.goods_receipts.create", "procurement:receive"),
            ("procurement.goods_receipts.update", "procurement:receive"),
            ("procurement.goods_receipts.post", "procurement:receive"),
        ] {
            assert_eq!(operation(key).permission(), permission, "{key}");
        }

        // These transitions are POST routes, but exact operation permission is
        // authoritative; the generic POST -> create fallback must not grant them.
        for key in [
            "procurement.requisitions.submit",
            "procurement.requisitions.cancel",
        ] {
            assert!(!allowed(key, &["procurement:create"]), "{key}");
            assert!(allowed(key, &["procurement:edit"]), "{key}");
        }

        for key in [
            "procurement.requisitions.approve",
            "procurement.requisitions.reject",
            "procurement.purchase_orders.issue",
            "procurement.purchase_orders.cancel",
        ] {
            assert!(!allowed(key, &["procurement:edit"]), "{key}");
            assert!(allowed(key, &["procurement:approve"]), "{key}");
        }

        for key in [
            "procurement.goods_receipts.create",
            "procurement.goods_receipts.update",
            "procurement.goods_receipts.post",
        ] {
            assert!(!allowed(key, &["procurement:edit"]), "{key}");
            assert!(allowed(key, &["procurement:receive"]), "{key}");
        }
    }

    #[test]
    fn assets_inventory_operations_are_fully_governed() {
        for (key, permission, exposure) in [
            (
                "assets_inventory.items.list",
                "assets_inventory:view",
                "exposed",
            ),
            (
                "assets_inventory.items.read",
                "assets_inventory:view",
                "exposed",
            ),
            (
                "assets_inventory.items.create",
                "assets_inventory:create",
                "approval_required",
            ),
            (
                "assets_inventory.items.update",
                "assets_inventory:edit",
                "approval_required",
            ),
            (
                "assets_inventory.items.delete",
                "assets_inventory:delete",
                "approval_required",
            ),
            (
                "assets_inventory.stores.list",
                "assets_inventory:view",
                "exposed",
            ),
            (
                "assets_inventory.stores.read",
                "assets_inventory:view",
                "exposed",
            ),
            (
                "assets_inventory.stores.create",
                "assets_inventory:create",
                "approval_required",
            ),
            (
                "assets_inventory.stores.update",
                "assets_inventory:edit",
                "approval_required",
            ),
            (
                "assets_inventory.stores.delete",
                "assets_inventory:delete",
                "approval_required",
            ),
        ] {
            let operation = operation(key);
            assert_eq!(operation.module_key(), "assets_inventory", "{key}");
            assert_eq!(operation.permission(), permission, "{key}");
            assert!(operation.license_required(), "{key}");
            assert_eq!(operation.required_modules().count(), 0, "{key}");
            assert_eq!(operation.agent_exposure().as_str(), exposure, "{key}");
        }
    }

    #[test]
    fn assets_inventory_foundation_crud_succeeds_with_standalone_entitlement() {
        let snapshot = EntitlementSnapshot::new(
            LeaseLifecycle::Active,
            vec![(
                "assets_inventory".to_string(),
                ModuleEntitlementState::Enabled,
            )],
            vec![],
        )
        .unwrap_or_else(|_| unreachable!());
        for (key, permission) in [
            ("assets_inventory.items.list", "assets_inventory:view"),
            ("assets_inventory.items.read", "assets_inventory:view"),
            ("assets_inventory.items.create", "assets_inventory:create"),
            ("assets_inventory.items.update", "assets_inventory:edit"),
            ("assets_inventory.items.delete", "assets_inventory:delete"),
            ("assets_inventory.stores.list", "assets_inventory:view"),
            ("assets_inventory.stores.read", "assets_inventory:view"),
            ("assets_inventory.stores.create", "assets_inventory:create"),
            ("assets_inventory.stores.update", "assets_inventory:edit"),
            ("assets_inventory.stores.delete", "assets_inventory:delete"),
        ] {
            let operation = operation(key);
            let decision = evaluate_operation(
                operation,
                &snapshot,
                &[permission.to_string()],
                RuntimeAccessChecks::default(),
            );
            assert!(decision.allowed, "{key}: {}", decision.reason.as_str());
        }
    }

    #[test]
    fn assets_inventory_stock_operations_have_exact_access_contracts() {
        for (method, path, key, permission, effect, exposure, dependency) in [
            (
                Method::GET,
                "/api/1.0/assets-inventory/stock-balances",
                "assets_inventory.stock_balances.list",
                "assets_inventory:view",
                OperationEffect::Read,
                "exposed",
                None,
            ),
            (
                Method::GET,
                "/api/1.0/assets-inventory/stock-movements",
                "assets_inventory.stock_movements.list",
                "assets_inventory:view",
                OperationEffect::Read,
                "exposed",
                None,
            ),
            (
                Method::GET,
                "/api/1.0/assets-inventory/stock-movements/{id}",
                "assets_inventory.stock_movements.read",
                "assets_inventory:view",
                OperationEffect::Read,
                "exposed",
                None,
            ),
            (
                Method::POST,
                "/api/1.0/assets-inventory/manual-receipts",
                "assets_inventory.manual_receipts.create",
                "assets_inventory:receive",
                OperationEffect::Write,
                "approval_required",
                None,
            ),
            (
                Method::POST,
                "/api/1.0/assets-inventory/issues",
                "assets_inventory.issues.create",
                "assets_inventory:issue",
                OperationEffect::Write,
                "approval_required",
                None,
            ),
            (
                Method::POST,
                "/api/1.0/assets-inventory/transfers",
                "assets_inventory.transfers.create",
                "assets_inventory:transfer",
                OperationEffect::Write,
                "approval_required",
                None,
            ),
            (
                Method::POST,
                "/api/1.0/assets-inventory/adjustments",
                "assets_inventory.adjustments.create",
                "assets_inventory:adjust",
                OperationEffect::Write,
                "approval_required",
                None,
            ),
            (
                Method::POST,
                "/api/1.0/assets-inventory/stock-movements/{id}/reverse",
                "assets_inventory.stock_movements.reverse",
                "assets_inventory:reverse",
                OperationEffect::Write,
                "approval_required",
                None,
            ),
            (
                Method::GET,
                "/api/1.0/assets-inventory/goods-receipt-allocations",
                "assets_inventory.goods_receipt_allocations.list",
                "assets_inventory:receive",
                OperationEffect::Read,
                "exposed",
                Some("procurement"),
            ),
            (
                Method::POST,
                "/api/1.0/assets-inventory/goods-receipt-allocations",
                "assets_inventory.goods_receipt_allocations.create",
                "assets_inventory:receive",
                OperationEffect::Write,
                "approval_required",
                Some("procurement"),
            ),
        ] {
            let route = routed_operation(key);
            let operation = route.operation();
            assert_eq!(route.method(), &method, "{key}");
            assert_eq!(route.route_pattern(), path, "{key}");
            assert_eq!(operation.module_key(), "assets_inventory", "{key}");
            assert_eq!(operation.permission(), permission, "{key}");
            assert_eq!(operation.effect(), effect, "{key}");
            assert!(operation.license_required(), "{key}");
            assert_eq!(operation.agent_exposure().as_str(), exposure, "{key}");
            assert_eq!(
                operation.required_modules().collect::<Vec<_>>(),
                dependency.into_iter().collect::<Vec<_>>(),
                "{key}"
            );
        }
    }

    #[test]
    fn assets_inventory_manual_stock_operations_remain_standalone() {
        let operations = [
            (
                "assets_inventory.stock_balances.list",
                "assets_inventory:view",
            ),
            (
                "assets_inventory.stock_movements.list",
                "assets_inventory:view",
            ),
            (
                "assets_inventory.stock_movements.read",
                "assets_inventory:view",
            ),
            (
                "assets_inventory.manual_receipts.create",
                "assets_inventory:receive",
            ),
            ("assets_inventory.issues.create", "assets_inventory:issue"),
            (
                "assets_inventory.transfers.create",
                "assets_inventory:transfer",
            ),
            (
                "assets_inventory.adjustments.create",
                "assets_inventory:adjust",
            ),
            (
                "assets_inventory.stock_movements.reverse",
                "assets_inventory:reverse",
            ),
        ];
        let active = EntitlementSnapshot::new(
            LeaseLifecycle::Active,
            [(
                "assets_inventory".to_string(),
                ModuleEntitlementState::Enabled,
            )],
            Vec::<String>::new(),
        )
        .unwrap_or_else(|_| unreachable!());
        for (key, permission) in operations {
            let operation = operation(key);
            assert_eq!(operation.required_modules().count(), 0, "{key}");
            let decision = evaluate_operation(
                operation,
                &active,
                &[permission.to_string()],
                RuntimeAccessChecks::default(),
            );
            assert!(decision.allowed, "{key}: {}", decision.reason.as_str());
        }

        let restricted = EntitlementSnapshot::new(
            LeaseLifecycle::Restricted,
            [(
                "assets_inventory".to_string(),
                ModuleEntitlementState::Enabled,
            )],
            Vec::<String>::new(),
        )
        .unwrap_or_else(|_| unreachable!());
        for (key, permission) in operations {
            let decision = evaluate_operation(
                operation(key),
                &restricted,
                &[permission.to_string()],
                RuntimeAccessChecks::default(),
            );
            assert_eq!(
                decision.allowed,
                operation(key).effect() == OperationEffect::Read,
                "{key}: {}",
                decision.reason.as_str()
            );
        }

        for lifecycle in [LeaseLifecycle::Revoked, LeaseLifecycle::Invalid] {
            let snapshot = EntitlementSnapshot::new(
                lifecycle,
                [(
                    "assets_inventory".to_string(),
                    ModuleEntitlementState::Enabled,
                )],
                Vec::<String>::new(),
            )
            .unwrap_or_else(|_| unreachable!());
            for (key, permission) in operations {
                let decision = evaluate_operation(
                    operation(key),
                    &snapshot,
                    &[permission.to_string()],
                    RuntimeAccessChecks::default(),
                );
                assert!(!decision.allowed, "{lifecycle:?} allowed {key}");
            }
        }
    }

    #[test]
    fn assets_inventory_stock_requests_require_hr_and_exact_workflow_permissions() {
        for (method, path, key, permission, effect, exposure) in [
            (
                Method::GET,
                "/api/1.0/assets-inventory/stock-request-requesters",
                "assets_inventory.requester_candidates.list",
                "assets_inventory:request",
                OperationEffect::Read,
                "exposed",
            ),
            (
                Method::GET,
                "/api/1.0/assets-inventory/stock-request-departments",
                "assets_inventory.department_candidates.list",
                "assets_inventory:request",
                OperationEffect::Read,
                "exposed",
            ),
            (
                Method::GET,
                "/api/1.0/assets-inventory/stock-requests",
                "assets_inventory.stock_requests.list",
                "assets_inventory:view",
                OperationEffect::Read,
                "exposed",
            ),
            (
                Method::GET,
                "/api/1.0/assets-inventory/stock-requests/{id}",
                "assets_inventory.stock_requests.read",
                "assets_inventory:view",
                OperationEffect::Read,
                "exposed",
            ),
            (
                Method::GET,
                "/api/1.0/assets-inventory/stock-requests/{id}/fulfilment-preview",
                "assets_inventory.stock_requests.fulfilment_preview.read",
                "assets_inventory:view",
                OperationEffect::Read,
                "exposed",
            ),
            (
                Method::POST,
                "/api/1.0/assets-inventory/stock-requests",
                "assets_inventory.stock_requests.create",
                "assets_inventory:request",
                OperationEffect::Write,
                "approval_required",
            ),
            (
                Method::PUT,
                "/api/1.0/assets-inventory/stock-requests/{id}",
                "assets_inventory.stock_requests.update",
                "assets_inventory:request",
                OperationEffect::Write,
                "approval_required",
            ),
            (
                Method::DELETE,
                "/api/1.0/assets-inventory/stock-requests/{id}",
                "assets_inventory.stock_requests.delete",
                "assets_inventory:request",
                OperationEffect::Destructive,
                "approval_required",
            ),
            (
                Method::POST,
                "/api/1.0/assets-inventory/stock-requests/{id}/submit",
                "assets_inventory.stock_requests.submit",
                "assets_inventory:request",
                OperationEffect::Write,
                "approval_required",
            ),
            (
                Method::POST,
                "/api/1.0/assets-inventory/stock-requests/{id}/cancel",
                "assets_inventory.stock_requests.cancel",
                "assets_inventory:request",
                OperationEffect::Write,
                "approval_required",
            ),
            (
                Method::POST,
                "/api/1.0/assets-inventory/stock-requests/{id}/approve",
                "assets_inventory.stock_requests.approve",
                "assets_inventory:approve",
                OperationEffect::Write,
                "approval_required",
            ),
            (
                Method::POST,
                "/api/1.0/assets-inventory/stock-requests/{id}/reject",
                "assets_inventory.stock_requests.reject",
                "assets_inventory:approve",
                OperationEffect::Write,
                "approval_required",
            ),
            (
                Method::POST,
                "/api/1.0/assets-inventory/stock-requests/{id}/close",
                "assets_inventory.stock_requests.close",
                "assets_inventory:approve",
                OperationEffect::Write,
                "approval_required",
            ),
            (
                Method::POST,
                "/api/1.0/assets-inventory/stock-requests/{id}/fulfilments",
                "assets_inventory.stock_request_fulfilments.create",
                "assets_inventory:issue",
                OperationEffect::Write,
                "approval_required",
            ),
        ] {
            let route = routed_operation(key);
            let operation = route.operation();
            assert_eq!(route.method(), &method, "{key}");
            assert_eq!(route.route_pattern(), path, "{key}");
            assert_eq!(operation.module_key(), "assets_inventory", "{key}");
            assert_eq!(operation.permission(), permission, "{key}");
            assert_eq!(operation.effect(), effect, "{key}");
            assert_eq!(operation.agent_exposure().as_str(), exposure, "{key}");
            assert_eq!(
                operation.required_modules().collect::<Vec<_>>(),
                vec!["hr_payroll"],
                "{key}"
            );
        }
    }

    #[test]
    fn assets_inventory_goods_receipt_allocation_requires_exact_dependency() {
        let allocation_operations = [
            "assets_inventory.goods_receipt_allocations.list",
            "assets_inventory.goods_receipt_allocations.create",
        ];
        let enabled = EntitlementSnapshot::new(
            LeaseLifecycle::Active,
            [
                (
                    "assets_inventory".to_string(),
                    ModuleEntitlementState::Enabled,
                ),
                ("procurement".to_string(), ModuleEntitlementState::Enabled),
            ],
            Vec::<String>::new(),
        )
        .unwrap_or_else(|_| unreachable!());
        for key in allocation_operations {
            let operation = operation(key);
            assert_eq!(
                operation.required_modules().collect::<Vec<_>>(),
                vec!["procurement"],
                "{key}"
            );
            let decision = evaluate_operation(
                operation,
                &enabled,
                &["assets_inventory:receive".to_string()],
                RuntimeAccessChecks::default(),
            );
            assert!(decision.allowed, "{key}: {}", decision.reason.as_str());
        }

        for procurement_state in [
            None,
            Some(ModuleEntitlementState::LocallyDisabled),
            Some(ModuleEntitlementState::Expired),
            Some(ModuleEntitlementState::Revoked),
        ] {
            let mut modules = vec![(
                "assets_inventory".to_string(),
                ModuleEntitlementState::Enabled,
            )];
            if let Some(state) = procurement_state {
                modules.push(("procurement".to_string(), state));
            }
            let snapshot = EntitlementSnapshot::new(LeaseLifecycle::Active, modules, Vec::new())
                .unwrap_or_else(|_| unreachable!());
            for key in allocation_operations {
                let decision = evaluate_operation(
                    operation(key),
                    &snapshot,
                    &["assets_inventory:receive".to_string()],
                    RuntimeAccessChecks::default(),
                );
                assert!(!decision.allowed, "{procurement_state:?} allowed {key}");
                assert_eq!(decision.reason.as_str(), "dependency_missing", "{key}");
            }
        }

        let procurement_only = EntitlementSnapshot::new(
            LeaseLifecycle::Active,
            [("procurement".to_string(), ModuleEntitlementState::Enabled)],
            Vec::<String>::new(),
        )
        .unwrap_or_else(|_| unreachable!());
        for key in allocation_operations {
            let decision = evaluate_operation(
                operation(key),
                &procurement_only,
                &["assets_inventory:receive".to_string()],
                RuntimeAccessChecks::default(),
            );
            assert!(
                !decision.allowed,
                "missing Assets entitlement allowed {key}"
            );
            assert_eq!(decision.reason.as_str(), "module_not_entitled", "{key}");
        }

        let restricted = EntitlementSnapshot::new(
            LeaseLifecycle::Restricted,
            [
                (
                    "assets_inventory".to_string(),
                    ModuleEntitlementState::Enabled,
                ),
                ("procurement".to_string(), ModuleEntitlementState::Enabled),
            ],
            Vec::<String>::new(),
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(
            evaluate_operation(
                operation("assets_inventory.goods_receipt_allocations.list"),
                &restricted,
                &["assets_inventory:receive".to_string()],
                RuntimeAccessChecks::default(),
            )
            .allowed
        );
        assert!(
            !evaluate_operation(
                operation("assets_inventory.goods_receipt_allocations.create"),
                &restricted,
                &["assets_inventory:receive".to_string()],
                RuntimeAccessChecks::default(),
            )
            .allowed
        );

        for lifecycle in [LeaseLifecycle::Revoked, LeaseLifecycle::Invalid] {
            let snapshot = EntitlementSnapshot::new(
                lifecycle,
                [
                    (
                        "assets_inventory".to_string(),
                        ModuleEntitlementState::Enabled,
                    ),
                    ("procurement".to_string(), ModuleEntitlementState::Enabled),
                ],
                Vec::<String>::new(),
            )
            .unwrap_or_else(|_| unreachable!());
            for key in allocation_operations {
                assert!(
                    !evaluate_operation(
                        operation(key),
                        &snapshot,
                        &["assets_inventory:receive".to_string()],
                        RuntimeAccessChecks::default(),
                    )
                    .allowed,
                    "{lifecycle:?} allowed {key}"
                );
            }
        }
    }

    #[test]
    fn ai_provider_administration_requires_agent_and_exact_permissions() {
        let catalog = operation("administration.ai_providers.catalog.list");
        assert_eq!(catalog.permission(), "ai_providers:view");
        assert_eq!(
            catalog.required_modules().collect::<Vec<_>>(),
            vec!["agent"]
        );
        assert!(catalog.license_required());
        assert!(allowed(
            "administration.ai_providers.connections.list",
            &["ai_providers:view"]
        ));
        assert!(!allowed(
            "administration.ai_providers.connections.update",
            &["ai_providers:view"]
        ));
        assert!(allowed(
            "administration.ai_providers.connections.update",
            &["ai_providers:edit"]
        ));

        assert_eq!(
            operation("administration.ai_providers.connections.create")
                .agent_exposure()
                .as_str(),
            "human_only"
        );
        assert_eq!(
            operation("administration.ai_providers.connections.disconnect")
                .agent_exposure()
                .as_str(),
            "human_only"
        );
        assert_eq!(
            operation("administration.ai_providers.connections.test")
                .agent_exposure()
                .as_str(),
            "approval_required"
        );

        let without_agent = EntitlementSnapshot::new(
            LeaseLifecycle::Active,
            [(
                "administration".to_string(),
                ModuleEntitlementState::Enabled,
            )],
            vec![],
        )
        .unwrap_or_else(|_| unreachable!());
        let decision = evaluate_operation(
            catalog,
            &without_agent,
            &["ai_providers:view".to_string()],
            RuntimeAccessChecks::default(),
        );
        assert!(!decision.allowed);
        assert_eq!(decision.reason.as_str(), "dependency_missing");

        let enabled_modules = || {
            [
                (
                    "administration".to_string(),
                    ModuleEntitlementState::Enabled,
                ),
                ("agent".to_string(), ModuleEntitlementState::Enabled),
            ]
        };
        let restricted =
            EntitlementSnapshot::new(LeaseLifecycle::Restricted, enabled_modules(), vec![])
                .unwrap_or_else(|_| unreachable!());
        let restricted_read = evaluate_operation(
            catalog,
            &restricted,
            &["ai_providers:view".to_string()],
            RuntimeAccessChecks::default(),
        );
        assert!(restricted_read.allowed);

        let restricted_write = evaluate_operation(
            operation("administration.ai_providers.connections.update"),
            &restricted,
            &["ai_providers:edit".to_string()],
            RuntimeAccessChecks::default(),
        );
        assert!(!restricted_write.allowed);
        assert_eq!(restricted_write.reason.as_str(), "lease_expired");

        for (lifecycle, reason) in [
            (LeaseLifecycle::Revoked, "license_revoked"),
            (LeaseLifecycle::Invalid, "license_invalid"),
        ] {
            let snapshot = EntitlementSnapshot::new(lifecycle, enabled_modules(), vec![])
                .unwrap_or_else(|_| unreachable!());
            let decision = evaluate_operation(
                catalog,
                &snapshot,
                &["ai_providers:view".to_string()],
                RuntimeAccessChecks::default(),
            );
            assert!(!decision.allowed);
            assert_eq!(decision.reason.as_str(), reason);
        }
    }

    #[test]
    fn ai_routing_administration_requires_agent_and_exact_permissions() {
        let list = operation("administration.ai_routing.routes.list");
        assert_eq!(list.permission(), "ai_routing:view");
        assert_eq!(list.required_modules().collect::<Vec<_>>(), vec!["agent"]);
        assert!(list.license_required());
        assert_eq!(list.agent_exposure().as_str(), "exposed");
        let options = operation("administration.ai_routing.routes.options");
        assert_eq!(options.permission(), "ai_routing:view");
        assert_eq!(
            options.required_modules().collect::<Vec<_>>(),
            vec!["agent"]
        );
        assert!(options.license_required());
        assert_eq!(options.agent_exposure().as_str(), "exposed");
        assert_eq!(
            operation("administration.ai_routing.routes.resolve")
                .agent_exposure()
                .as_str(),
            "exposed"
        );

        for key in [
            "administration.ai_routing.routes.create",
            "administration.ai_routing.routes.update",
            "administration.ai_routing.routes.archive",
        ] {
            assert_eq!(
                operation(key).agent_exposure().as_str(),
                "approval_required",
                "{key} must not be directly executable"
            );
        }

        assert!(allowed(
            "administration.ai_routing.routes.read",
            &["ai_routing:view"]
        ));
        assert!(!allowed(
            "administration.ai_routing.routes.update",
            &["ai_routing:view"]
        ));
        assert!(allowed(
            "administration.ai_routing.routes.update",
            &["ai_routing:edit"]
        ));

        let without_agent = EntitlementSnapshot::new(
            LeaseLifecycle::Active,
            [(
                "administration".to_string(),
                ModuleEntitlementState::Enabled,
            )],
            vec![],
        )
        .unwrap_or_else(|_| unreachable!());
        let decision = evaluate_operation(
            list,
            &without_agent,
            &["ai_routing:view".to_string()],
            RuntimeAccessChecks::default(),
        );
        assert!(!decision.allowed);
        assert_eq!(decision.reason.as_str(), "dependency_missing");
    }

    #[test]
    fn agent_governance_requires_agent_and_separate_human_permissions() {
        for (key, permission) in [
            (
                "administration.agent_governance.readiness",
                "agent_policy:view",
            ),
            (
                "administration.agent_governance.capabilities.list",
                "agent_policy:view",
            ),
            ("administration.agent_usage.options", "agent_usage:view"),
            ("administration.agent_usage.report", "agent_usage:view"),
            ("administration.agent_usage.export", "agent_usage:export"),
            ("administration.agent_audit.runs.list", "agent_audit:view"),
            ("administration.agent_audit.runs.read", "agent_audit:view"),
        ] {
            let operation = operation(key);
            assert_eq!(operation.permission(), permission, "{key}");
            assert_eq!(
                operation.required_modules().collect::<Vec<_>>(),
                vec!["agent"],
                "{key}"
            );
            assert!(operation.license_required(), "{key}");
            assert_eq!(operation.agent_exposure().as_str(), "human_only", "{key}");
        }

        assert!(allowed(
            "administration.agent_governance.readiness",
            &["agent_policy:view"]
        ));
        assert!(!allowed(
            "administration.agent_usage.export",
            &["agent_usage:view"]
        ));
        assert!(allowed(
            "administration.agent_usage.export",
            &["agent_usage:export"]
        ));
    }

    #[test]
    fn campus_owner_wildcard_still_requires_every_operation_module() {
        let snapshot = active_snapshot();
        for entry in operation_catalog() {
            let decision = evaluate_operation(
                entry.operation(),
                &snapshot,
                &["*".to_string()],
                RuntimeAccessChecks::default(),
            );
            assert!(
                decision.allowed,
                "owner unexpectedly denied operation {}: {}",
                entry.operation().key(),
                decision.reason.as_str()
            );
        }

        let missing_fleet = EntitlementSnapshot::new(
            LeaseLifecycle::Active,
            vec![(
                "administration".to_string(),
                ModuleEntitlementState::Enabled,
            )],
            vec![],
        )
        .unwrap_or_else(|_| unreachable!());
        let decision = evaluate_operation(
            operation("fleet.vehicles.list"),
            &missing_fleet,
            &["*".to_string()],
            RuntimeAccessChecks::default(),
        );
        assert!(!decision.allowed);
    }

    #[actix_web::test]
    async fn catalog_patterns_equal_actix_resolved_patterns() {
        async fn resolved_pattern(request: HttpRequest) -> HttpResponse {
            HttpResponse::Ok().body(
                request
                    .match_pattern()
                    .unwrap_or_else(|| "<unmatched>".to_string()),
            )
        }

        let app = actix_test::init_service(App::new().configure(|config| {
            for entry in operation_catalog() {
                config.route(
                    entry.route_pattern(),
                    web::method(entry.method().clone()).to(resolved_pattern),
                );
            }
        }))
        .await;

        for entry in operation_catalog() {
            let concrete_path = entry
                .route_pattern()
                .replace("{id}", "00000000-0000-0000-0000-000000000001")
                .replace("{module_key}", "fleet");
            let request = actix_test::TestRequest::default()
                .method(entry.method().clone())
                .uri(&concrete_path)
                .to_request();
            let response = actix_test::call_service(&app, request).await;
            assert!(
                response.status().is_success(),
                "unmatched route: {concrete_path}"
            );
            let body = actix_test::read_body(response).await;
            assert_eq!(body.as_ref(), entry.route_pattern().as_bytes());
        }
    }
}
