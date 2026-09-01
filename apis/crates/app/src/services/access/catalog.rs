//
//  campus-pilot-apis
//  catalog.rs
//
//  Created by OpenAI Codex on 2026/08/26.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PermissionDefinition {
    pub key: String,
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleDefinition {
    pub key: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub description: &'static str,
    pub route: &'static str,
    pub permission_namespace: &'static str,
    pub core: bool,
    pub stage: &'static str,
    pub dependencies: Vec<&'static str>,
    pub permissions: Vec<PermissionDefinition>,
}

pub fn module_catalog() -> Vec<ModuleDefinition> {
    vec![
        module(
            "administration",
            "Administration",
            "Campus management",
            "Manage people, roles, licensing, and campus configuration.",
            "/admin",
            "administration",
            true,
            "available",
            &["view"],
        ),
        module(
            "sis",
            "People and admissions",
            "People and learning",
            "Manage applications, enrolment, learner records, and guardians.",
            "/modules/sis",
            "sis",
            false,
            "available",
            &["view", "create", "edit", "delete"],
        ),
        module(
            "academics",
            "Academics",
            "People and learning",
            "Plan teaching structures, subjects, classes, assessment, and progression.",
            "/modules/academics",
            "academics",
            false,
            "available",
            &["view", "create", "edit", "delete"],
        ),
        module(
            "timetabling",
            "Timetabling",
            "People and learning",
            "Generate and publish conflict-aware class and staff timetables.",
            "/modules/timetabling",
            "timetabling",
            false,
            "available",
            &["view", "create", "edit", "manage"],
        ),
        module(
            "attendance",
            "Attendance",
            "People and learning",
            "Prepare, submit, and review daily learner attendance registers.",
            "/modules/attendance",
            "attendance",
            false,
            "available",
            &["view", "create", "edit", "delete", "submit", "manage"],
        ),
        module(
            "messaging",
            "Communication",
            "Campus operations",
            "Coordinate announcements, targeted messages, and school notices.",
            "/modules/messaging",
            "messaging",
            false,
            "foundation",
            &["view", "create", "edit", "delete"],
        ),
        module(
            "finance",
            "Finance",
            "Finance and resources",
            "Run the general ledger, budgets, reporting, and financial controls.",
            "/modules/finance",
            "finance",
            false,
            "available",
            &["view", "create", "edit", "delete"],
        ),
        module(
            "fees",
            "Fees and billing",
            "Finance and resources",
            "Manage fee structures, invoices, receipts, and account balances.",
            "/modules/fees",
            "fees",
            false,
            "available",
            &["view", "create", "edit", "delete"],
        ),
        module(
            "library",
            "Library",
            "People and learning",
            "Manage catalogue, circulation, reservations, and learning resources.",
            "/modules/library",
            "library",
            false,
            "foundation",
            &["view", "create", "edit", "delete"],
        ),
        module(
            "hr_payroll",
            "HR and payroll",
            "Campus operations",
            "Manage staff records, leave, contracts, and payroll operations.",
            "/modules/hr-payroll",
            "hr_payroll",
            false,
            "available",
            &["view", "create", "edit", "delete"],
        ),
        module(
            "procurement",
            "Procurement",
            "Finance and resources",
            "Manage requisitions, approval decisions, purchase orders, receiving, and supplier records.",
            "/modules/procurement",
            "procurement",
            false,
            "available",
            &["view", "create", "edit", "delete", "approve", "receive"],
        ),
        module(
            "fleet",
            "Fleet",
            "Campus operations",
            "Manage vehicles, drivers, daily logs, trips, and maintenance readiness.",
            "/modules/fleet",
            "fleet",
            false,
            "available",
            &["view", "create", "edit", "delete"],
        ),
        module(
            "hostel",
            "Hostel",
            "Student services",
            "Manage residences, rooms, allocation, occupancy, and pastoral records.",
            "/modules/hostel",
            "hostel",
            false,
            "foundation",
            &["view", "create", "edit", "delete"],
        ),
        module(
            "health",
            "Health services",
            "Student services",
            "Manage clinic visits, care records, medication, and wellbeing follow-up.",
            "/modules/health",
            "health",
            false,
            "foundation",
            &["view", "create", "edit", "delete"],
        ),
        module(
            "assets_inventory",
            "Assets and inventory",
            "Finance and resources",
            "Maintain item and store catalogues, stock balances, and immutable movements.",
            "/modules/assets-inventory",
            "assets_inventory",
            false,
            "available",
            &[
                "view", "create", "edit", "delete", "request", "approve", "receive", "issue",
                "transfer", "adjust", "reverse",
            ],
        ),
        module(
            "document_registry",
            "Document registry",
            "Campus operations",
            "File, classify, retain, and retrieve official campus documents.",
            "/modules/document-registry",
            "document_registry",
            false,
            "planned",
            &["view", "create", "edit", "delete"],
        ),
        module(
            "internal_audit",
            "Internal audit",
            "Campus management",
            "Plan audits, record findings, and follow remediation to closure.",
            "/modules/internal-audit",
            "internal_audit",
            false,
            "planned",
            &["view", "create", "edit", "delete"],
        ),
        module(
            "agent",
            "Agent",
            "Campus tools",
            "Work with authorized campus capabilities through durable Agent sessions.",
            "/modules/agent",
            "agent",
            false, // Licensed module; not part of the core entitlement.
            "planned",
            &["view", "run", "history", "share", "approve"],
        ),
    ]
}

pub fn administration_permissions() -> Vec<PermissionDefinition> {
    [
        (
            "administration:view",
            "Open Administration",
            "Open the campus Administration module.",
        ),
        (
            "users:view",
            "View users",
            "Read the campus user directory.",
        ),
        (
            "users:create",
            "Create users",
            "Create campus user accounts.",
        ),
        (
            "users:edit",
            "Edit users",
            "Change user details, status, and role assignments.",
        ),
        (
            "users:delete",
            "Delete users",
            "Delete eligible campus user accounts.",
        ),
        (
            "roles:view",
            "View roles",
            "Read role definitions and access rules.",
        ),
        (
            "roles:create",
            "Create roles",
            "Create custom campus roles.",
        ),
        (
            "roles:edit",
            "Edit roles",
            "Change seeded or custom role labels and permissions.",
        ),
        (
            "roles:assign",
            "Assign roles",
            "Assign roles within your own access authority.",
        ),
        (
            "roles:delete",
            "Delete roles",
            "Delete unassigned custom roles.",
        ),
        (
            "licensing:view",
            "View licensing",
            "See module entitlement and expiry states.",
        ),
        (
            "licensing:edit",
            "Activate licenses",
            "Activate or change signed module entitlements.",
        ),
        (
            "licensing:delete",
            "Disable modules",
            "Disable non-core campus modules.",
        ),
        (
            "school_settings:view",
            "View school settings",
            "Read campus configuration.",
        ),
        (
            "school_settings:edit",
            "Edit school settings",
            "Change campus configuration.",
        ),
        (
            "ai_providers:view",
            "View AI providers",
            "Read provider connections, status, and cached model catalogues.",
        ),
        (
            "ai_providers:edit",
            "Manage AI providers",
            "Connect, test, rotate, refresh, and disconnect AI providers.",
        ),
        (
            "ai_routing:view",
            "View AI routing",
            "Read Agent provider and model routing rules.",
        ),
        (
            "ai_routing:edit",
            "Manage AI routing",
            "Create, change, and archive Agent provider and model routing rules.",
        ),
        (
            "agent_policy:view",
            "View Agent policy",
            "Read Agent readiness, capability coverage, and governance state.",
        ),
        (
            "agent_policy:edit",
            "Manage Agent policy",
            "Change reviewed Agent governance policy.",
        ),
        (
            "agent_usage:view",
            "View Agent usage",
            "Read campus Agent usage totals and trends.",
        ),
        (
            "agent_usage:export",
            "Export Agent usage",
            "Export bounded campus Agent usage evidence.",
        ),
        (
            "agent_limits:view",
            "View Agent limits",
            "Read configured Agent usage limits.",
        ),
        (
            "agent_limits:edit",
            "Manage Agent limits",
            "Change reviewed Agent usage limits.",
        ),
        (
            "agent_audit:view",
            "View Agent audit",
            "Read redacted Agent run and audit evidence.",
        ),
    ]
    .into_iter()
    .map(|(key, label, description)| PermissionDefinition {
        key: key.to_string(),
        label: label.to_string(),
        description: description.to_string(),
    })
    .collect()
}

pub fn all_permission_keys() -> Vec<String> {
    let mut permissions: Vec<String> = administration_permissions()
        .into_iter()
        .map(|permission| permission.key)
        .collect();
    for module in module_catalog() {
        permissions.extend(
            module
                .permissions
                .into_iter()
                .map(|permission| permission.key),
        );
    }
    permissions.push("*".to_string());
    permissions.sort();
    permissions.dedup();
    permissions
}

pub fn is_known_module(module_key: &str) -> bool {
    module_catalog()
        .iter()
        .any(|module| module.key == module_key)
}

pub fn is_core_module(module_key: &str) -> bool {
    matches!(module_key, "home" | "administration")
}

pub fn module_dependencies(module_key: &str) -> &'static [&'static str] {
    match module_key {
        "fees" => &["academics", "finance", "sis"],
        "fleet" => &["hr_payroll"],
        "procurement" => &["finance", "hr_payroll"],
        "timetabling" => &["academics", "hr_payroll"],
        "attendance" => &["academics", "sis"],
        _ => &[],
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "the catalogue helper keeps each static module definition auditable at its call site"
)]
fn module(
    key: &'static str,
    label: &'static str,
    group: &'static str,
    description: &'static str,
    route: &'static str,
    permission_namespace: &'static str,
    core: bool,
    stage: &'static str,
    actions: &[&str],
) -> ModuleDefinition {
    let permissions = actions
        .iter()
        .map(|action| PermissionDefinition {
            key: format!("{}:{}", permission_namespace, action),
            label: action_label(action),
            description: format!("{} access in {}.", action_label(action), label),
        })
        .collect();

    ModuleDefinition {
        key,
        label,
        group,
        description,
        route,
        permission_namespace,
        core,
        stage,
        dependencies: module_dependencies(key).to_vec(),
        permissions,
    }
}

fn action_label(action: &str) -> String {
    let mut characters = action.chars();
    match characters.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), characters.as_str()),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use cp_agent::{ModuleCoverageRegistry, ModuleCoverageSource};
    use cp_common::{ProductOperation, operation_catalog};

    use crate::config::LicenseConfig;
    use crate::services::agent::build_capability_registry;

    fn license_config() -> LicenseConfig {
        LicenseConfig {
            trusted_public_keys: Default::default(),
            issuer: "campus-pilot-control-plane".to_string(),
            audience: "campus-pilot".to_string(),
            control_plane_url: None,
            credential_key_base64: None,
            installation_name: "Test installation".to_string(),
        }
    }

    use super::{all_permission_keys, module_catalog};

    const PROCUREMENT_WORKFLOW_PERMISSION_MIGRATION: &str =
        include_str!("../../../../../migrations/080_grant_procurement_workflow_permissions.sql");
    const AI_ROUTING_MIGRATION: &str =
        include_str!("../../../../../migrations/083_create_ai_task_routing.sql");

    #[test]
    fn procurement_permissions_separate_approval_and_receiving_authority() {
        let procurement = module_catalog()
            .into_iter()
            .find(|module| module.key == "procurement")
            .unwrap_or_else(|| unreachable!());
        let permissions = procurement
            .permissions
            .into_iter()
            .map(|permission| permission.key)
            .collect::<BTreeSet<_>>();

        assert!(permissions.contains("procurement:approve"));
        assert!(permissions.contains("procurement:receive"));
        assert!(PROCUREMENT_WORKFLOW_PERMISSION_MIGRATION.contains("key = 'finance_officer'"));
        assert_eq!(
            PROCUREMENT_WORKFLOW_PERMISSION_MIGRATION
                .matches("ARRAY['procurement:approve', 'procurement:receive']")
                .count(),
            2
        );
        assert!(
            PROCUREMENT_WORKFLOW_PERMISSION_MIGRATION
                .contains("zz_grant_new_tenant_procurement_workflow_permissions")
        );
    }

    #[test]
    fn assets_inventory_permissions_separate_stock_authority() {
        let assets_inventory = module_catalog()
            .into_iter()
            .find(|module| module.key == "assets_inventory")
            .unwrap_or_else(|| unreachable!());
        let permissions = assets_inventory
            .permissions
            .into_iter()
            .map(|permission| permission.key)
            .collect::<BTreeSet<_>>();

        assert_eq!(
            permissions,
            [
                "assets_inventory:adjust",
                "assets_inventory:approve",
                "assets_inventory:create",
                "assets_inventory:delete",
                "assets_inventory:edit",
                "assets_inventory:issue",
                "assets_inventory:receive",
                "assets_inventory:request",
                "assets_inventory:reverse",
                "assets_inventory:transfer",
                "assets_inventory:view",
            ]
            .into_iter()
            .map(str::to_string)
            .collect()
        );
    }

    #[test]
    fn agent_routing_permissions_are_code_owned_administration_permissions() {
        let permissions = all_permission_keys().into_iter().collect::<BTreeSet<_>>();

        assert!(permissions.contains("ai_routing:view"));
        assert!(permissions.contains("ai_routing:edit"));
        assert!(AI_ROUTING_MIGRATION.contains("key = 'school_administrator'"));
        assert!(
            AI_ROUTING_MIGRATION.contains("ARRAY['ai_routing:view', 'ai_routing:edit']::TEXT[]")
        );
        assert!(AI_ROUTING_MIGRATION.contains("zz_grant_new_tenant_ai_routing_permissions"));
        assert!(!AI_ROUTING_MIGRATION.contains("UPDATE roles\nSET permissions"));
    }

    #[test]
    fn agent_governance_permissions_are_separate_and_unseeded() {
        let permissions = all_permission_keys().into_iter().collect::<BTreeSet<_>>();

        for permission in [
            "agent_policy:view",
            "agent_policy:edit",
            "agent_usage:view",
            "agent_usage:export",
            "agent_limits:view",
            "agent_limits:edit",
            "agent_audit:view",
        ] {
            assert!(permissions.contains(permission), "missing {permission}");
        }

        assert!(!AI_ROUTING_MIGRATION.contains("agent_policy:view"));
        assert!(!AI_ROUTING_MIGRATION.contains("agent_usage:view"));
        assert!(!AI_ROUTING_MIGRATION.contains("agent_audit:view"));
    }

    #[test]
    fn operation_catalog_references_known_modules_and_permissions() {
        let modules: BTreeSet<_> = module_catalog()
            .into_iter()
            .map(|module| module.key)
            .collect();
        let permissions: BTreeSet<_> = all_permission_keys().into_iter().collect();

        for route in operation_catalog() {
            let operation = route.operation();
            assert!(
                modules.contains(operation.module_key()),
                "unknown operation module: {}",
                operation.module_key()
            );
            assert!(
                permissions.contains(operation.permission()),
                "unknown operation permission: {}",
                operation.permission()
            );
            for dependency in operation.required_modules() {
                assert!(
                    modules.contains(dependency),
                    "unknown operation dependency: {dependency}"
                );
            }
        }
    }

    #[test]
    fn module_dependencies_are_known_and_apply_to_every_module_operation() {
        let modules = module_catalog();
        let known = modules
            .iter()
            .map(|module| module.key)
            .collect::<BTreeSet<_>>();
        let operations = operation_catalog();

        for module in modules {
            for dependency in &module.dependencies {
                assert!(
                    known.contains(dependency),
                    "unknown dependency: {dependency}"
                );
            }
            if module.dependencies.is_empty() {
                continue;
            }
            let module_operations = operations
                .iter()
                .filter(|entry| entry.operation().module_key() == module.key)
                .collect::<Vec<_>>();
            assert!(
                !module_operations.is_empty(),
                "{} has no operations",
                module.key
            );
            for entry in module_operations {
                let required = entry
                    .operation()
                    .required_modules()
                    .collect::<BTreeSet<_>>();
                for dependency in &module.dependencies {
                    assert!(
                        required.contains(dependency),
                        "{} does not require {dependency}",
                        entry.operation().key()
                    );
                }
            }
        }
    }

    #[tokio::test]
    async fn module_coverage_exposes_current_release_and_agent_gaps() {
        let capability_registry = build_capability_registry(
            sqlx::postgres::PgPoolOptions::new()
                .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
                .unwrap_or_else(|_| unreachable!()),
            license_config(),
        );
        let coverage = ModuleCoverageRegistry::build(
            module_catalog().into_iter().map(|module| {
                ModuleCoverageSource::parse(module.key, module.stage, module.core, module.route)
                    .unwrap_or_else(|_| unreachable!())
            }),
            operation_catalog()
                .iter()
                .map(|entry| entry.operation().clone())
                .collect::<Vec<ProductOperation>>(),
            &capability_registry,
        )
        .unwrap_or_else(|_| unreachable!());

        assert_eq!(coverage.entries().len(), module_catalog().len());
        assert_eq!(coverage.missing_executable_capability_count(), 0);
        for module_key in [
            "administration",
            "academics",
            "attendance",
            "fleet",
            "hr_payroll",
            "sis",
            "timetabling",
            "finance",
            "fees",
            "procurement",
            "assets_inventory",
        ] {
            let module = coverage.entry(module_key).unwrap_or_else(|| unreachable!());
            assert!(module.stage_aligned(), "{module_key} stage is not aligned");
            assert!(
                module.licensing_aligned(),
                "{module_key} licensing is not aligned"
            );
            if module_key == "administration" {
                assert!(module.release_ready());
                assert_eq!(module.executable_capabilities(), 16);
            } else if module_key == "academics" {
                assert!(module.release_ready());
                assert_eq!(module.executable_capabilities(), 19);
            } else if module_key == "attendance" {
                assert!(module.release_ready());
                assert_eq!(module.routed_operations(), 8);
                assert_eq!(module.exposed_operations(), 3);
                assert_eq!(module.approval_required_operations(), 5);
                assert_eq!(module.executable_capabilities(), 3);
            } else if module_key == "fleet" {
                assert!(module.release_ready());
                assert_eq!(module.executable_capabilities(), 7);
            } else if module_key == "hr_payroll" {
                assert!(module.release_ready());
                assert_eq!(module.executable_capabilities(), 13);
            } else if module_key == "sis" {
                assert!(module.release_ready());
                assert_eq!(module.executable_capabilities(), 15);
            } else if module_key == "finance" {
                assert!(module.release_ready());
                assert_eq!(module.executable_capabilities(), 12);
            } else if module_key == "fees" {
                assert!(module.release_ready());
                assert_eq!(module.executable_capabilities(), 11);
            } else if module_key == "procurement" {
                assert!(module.release_ready());
                assert_eq!(module.executable_capabilities(), 10);
            } else if module_key == "assets_inventory" {
                assert!(module.release_ready());
                assert_eq!(module.routed_operations(), 34);
                assert_eq!(module.exposed_operations(), 13);
                assert_eq!(module.approval_required_operations(), 21);
                assert_eq!(module.executable_capabilities(), 13);
            } else {
                assert!(module.release_ready());
                assert_eq!(module.executable_capabilities(), 4);
            }
        }
    }
}
