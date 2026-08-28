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
    let operation = if key.starts_with("sis.") {
        operation.requiring_modules(["academics".to_string()])
    } else if key.starts_with("academics.teacher")
        || key.starts_with("academics.teaching_assignments")
        || key.starts_with("academics.assessment_components")
    {
        operation.requiring_modules(["hr_payroll".to_string()])
    } else if key.starts_with("timetabling.") {
        operation.requiring_modules(["academics".to_string(), "hr_payroll".to_string()])
    } else if key.starts_with("fees.") {
        operation.requiring_modules([
            "sis".to_string(),
            "academics".to_string(),
            "finance".to_string(),
        ])
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
        | "administration.roles.list"
        | "administration.roles.read"
        | "administration.users.list"
        | "administration.users.read"
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
        | "timetabling.runs.read_latest" => AgentExposure::Exposed,
        "administration.school_settings.update"
        | "administration.school_settings.update_logo"
        | "administration.users.create"
        | "administration.users.update"
        | "administration.users.activate"
        | "administration.users.deactivate"
        | "administration.licensing.refresh"
        | "administration.licensing.disable_module"
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
        | "timetabling.runs.publish" => AgentExposure::ApprovalRequired,
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
        _ => panic!("Product operation {key} has no Agent exposure classification"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use actix_web::{App, HttpRequest, HttpResponse, http::Method, test as actix_test, web};

    use crate::{
        AgentExposure, EntitlementSnapshot, LeaseLifecycle, ModuleEntitlementState,
        ProductOperation, RuntimeAccessChecks, evaluate_operation,
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
                ("finance".to_string(), ModuleEntitlementState::Enabled),
                ("fees".to_string(), ModuleEntitlementState::Enabled),
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
        assert_eq!(operation_catalog().len(), 216);

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

        assert_eq!(counts, [88, 121, 7, 0]);
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
