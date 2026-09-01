//! Attests Agent submission origins against the code-owned client route map.
//!
//! Browser-supplied module keys and paths are untrusted. Only an exact current
//! client route with current tenant entitlement and caller access can become a
//! persisted Agent run origin.

use cp_common::{AccessContext, ModuleEntitlementState};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OriginAccess {
    module_key: &'static str,
    required_module: &'static str,
    additional_module: Option<&'static str>,
    required_permission: Option<&'static str>,
    additional_permission: Option<&'static str>,
}

impl OriginAccess {
    const fn module(module_key: &'static str, permission: &'static str) -> Self {
        Self {
            module_key,
            required_module: module_key,
            additional_module: None,
            required_permission: Some(permission),
            additional_permission: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ExactRoute {
    path: &'static str,
    access: OriginAccess,
}

#[derive(Debug, Clone, Copy)]
struct UuidRoute {
    prefix: &'static str,
    access: OriginAccess,
}

const HOME: OriginAccess = OriginAccess {
    module_key: "home",
    required_module: "home",
    additional_module: None,
    required_permission: None,
    additional_permission: None,
};
const ADMINISTRATION: OriginAccess = OriginAccess::module("administration", "administration:view");
const ADMINISTRATION_USERS: OriginAccess = OriginAccess {
    module_key: "administration",
    required_module: "administration",
    additional_module: None,
    required_permission: Some("administration:view"),
    additional_permission: Some("users:view"),
};
const ADMINISTRATION_ROLES: OriginAccess = OriginAccess {
    module_key: "administration",
    required_module: "administration",
    additional_module: None,
    required_permission: Some("administration:view"),
    additional_permission: Some("roles:view"),
};
const ADMINISTRATION_LICENSING: OriginAccess = OriginAccess {
    module_key: "administration",
    required_module: "administration",
    additional_module: None,
    required_permission: Some("administration:view"),
    additional_permission: Some("licensing:view"),
};
const ADMINISTRATION_SETTINGS: OriginAccess = OriginAccess {
    module_key: "administration",
    required_module: "administration",
    additional_module: None,
    required_permission: Some("administration:view"),
    additional_permission: Some("school_settings:view"),
};
const ADMINISTRATION_AGENT_POLICY: OriginAccess = OriginAccess {
    module_key: "administration",
    required_module: "administration",
    additional_module: Some("agent"),
    required_permission: Some("administration:view"),
    additional_permission: Some("agent_policy:view"),
};
const ADMINISTRATION_AI_PROVIDERS: OriginAccess = OriginAccess {
    module_key: "administration",
    required_module: "administration",
    additional_module: Some("agent"),
    required_permission: Some("administration:view"),
    additional_permission: Some("ai_providers:view"),
};
const ADMINISTRATION_AI_ROUTING: OriginAccess = OriginAccess {
    module_key: "administration",
    required_module: "administration",
    additional_module: Some("agent"),
    required_permission: Some("administration:view"),
    additional_permission: Some("ai_routing:view"),
};
const ADMINISTRATION_AGENT_USAGE: OriginAccess = OriginAccess {
    module_key: "administration",
    required_module: "administration",
    additional_module: Some("agent"),
    required_permission: Some("administration:view"),
    additional_permission: Some("agent_usage:view"),
};
const ADMINISTRATION_AGENT_AUDIT: OriginAccess = OriginAccess {
    module_key: "administration",
    required_module: "administration",
    additional_module: Some("agent"),
    required_permission: Some("administration:view"),
    additional_permission: Some("agent_audit:view"),
};

const SIS: OriginAccess = OriginAccess::module("sis", "sis:view");
const ACADEMICS: OriginAccess = OriginAccess::module("academics", "academics:view");
const ATTENDANCE: OriginAccess = OriginAccess::module("attendance", "attendance:view");
const TIMETABLING: OriginAccess = OriginAccess::module("timetabling", "timetabling:view");
const MESSAGING: OriginAccess = OriginAccess::module("messaging", "messaging:view");
const MESSAGING_MANAGE: OriginAccess = OriginAccess::module("messaging", "messaging:create");
const MESSAGING_SEND: OriginAccess = OriginAccess::module("messaging", "messaging:send");
const FINANCE: OriginAccess = OriginAccess::module("finance", "finance:view");
const FEES: OriginAccess = OriginAccess::module("fees", "fees:view");
const FEES_IMPORTS: OriginAccess = OriginAccess::module("fees", "fees:create");
const LIBRARY: OriginAccess = OriginAccess::module("library", "library:view");
const HR_PAYROLL: OriginAccess = OriginAccess::module("hr_payroll", "hr_payroll:view");
const PROCUREMENT: OriginAccess = OriginAccess::module("procurement", "procurement:view");
const FLEET: OriginAccess = OriginAccess::module("fleet", "fleet:view");
const HOSTEL: OriginAccess = OriginAccess::module("hostel", "hostel:view");
const HEALTH: OriginAccess = OriginAccess::module("health", "health:view");
const ASSETS_INVENTORY: OriginAccess =
    OriginAccess::module("assets_inventory", "assets_inventory:view");
const ASSETS_RECEIVING: OriginAccess =
    OriginAccess::module("assets_inventory", "assets_inventory:receive");
const DOCUMENT_REGISTRY: OriginAccess =
    OriginAccess::module("document_registry", "document_registry:view");
const INTERNAL_AUDIT: OriginAccess = OriginAccess::module("internal_audit", "internal_audit:view");
const AGENT: OriginAccess = OriginAccess::module("agent", "agent:view");

const EXACT_ROUTES: &[ExactRoute] = &[
    ExactRoute {
        path: "/home",
        access: HOME,
    },
    ExactRoute {
        path: "/admin",
        access: ADMINISTRATION,
    },
    ExactRoute {
        path: "/admin/classes",
        access: ADMINISTRATION,
    },
    ExactRoute {
        path: "/admin/departments",
        access: ADMINISTRATION,
    },
    ExactRoute {
        path: "/admin/fees",
        access: ADMINISTRATION,
    },
    ExactRoute {
        path: "/admin/finance",
        access: ADMINISTRATION,
    },
    ExactRoute {
        path: "/admin/fleet",
        access: ADMINISTRATION,
    },
    ExactRoute {
        path: "/admin/fleet/daily-log",
        access: ADMINISTRATION,
    },
    ExactRoute {
        path: "/admin/fleet/drivers",
        access: ADMINISTRATION,
    },
    ExactRoute {
        path: "/admin/health",
        access: ADMINISTRATION,
    },
    ExactRoute {
        path: "/admin/hostel",
        access: ADMINISTRATION,
    },
    ExactRoute {
        path: "/admin/hr-payroll",
        access: ADMINISTRATION,
    },
    ExactRoute {
        path: "/admin/library",
        access: ADMINISTRATION,
    },
    ExactRoute {
        path: "/admin/messaging",
        access: ADMINISTRATION,
    },
    ExactRoute {
        path: "/admin/procurement",
        access: ADMINISTRATION,
    },
    ExactRoute {
        path: "/admin/staff",
        access: ADMINISTRATION,
    },
    ExactRoute {
        path: "/admin/students",
        access: ADMINISTRATION,
    },
    ExactRoute {
        path: "/admin/subjects",
        access: ADMINISTRATION,
    },
    ExactRoute {
        path: "/admin/users",
        access: ADMINISTRATION_USERS,
    },
    ExactRoute {
        path: "/admin/roles",
        access: ADMINISTRATION_ROLES,
    },
    ExactRoute {
        path: "/admin/licensing",
        access: ADMINISTRATION_LICENSING,
    },
    ExactRoute {
        path: "/admin/settings",
        access: ADMINISTRATION_SETTINGS,
    },
    ExactRoute {
        path: "/admin/agent",
        access: ADMINISTRATION_AGENT_POLICY,
    },
    ExactRoute {
        path: "/admin/agent/capabilities",
        access: ADMINISTRATION_AGENT_POLICY,
    },
    ExactRoute {
        path: "/admin/agent/providers",
        access: ADMINISTRATION_AI_PROVIDERS,
    },
    ExactRoute {
        path: "/admin/agent/routing",
        access: ADMINISTRATION_AI_ROUTING,
    },
    ExactRoute {
        path: "/admin/agent/usage",
        access: ADMINISTRATION_AGENT_USAGE,
    },
    ExactRoute {
        path: "/admin/agent/runs",
        access: ADMINISTRATION_AGENT_AUDIT,
    },
    ExactRoute {
        path: "/modules/sis",
        access: SIS,
    },
    ExactRoute {
        path: "/modules/sis/applications",
        access: SIS,
    },
    ExactRoute {
        path: "/modules/sis/enrolments",
        access: SIS,
    },
    ExactRoute {
        path: "/modules/sis/guardian-relationships",
        access: SIS,
    },
    ExactRoute {
        path: "/modules/sis/guardians",
        access: SIS,
    },
    ExactRoute {
        path: "/modules/sis/imports",
        access: SIS,
    },
    ExactRoute {
        path: "/modules/sis/learners",
        access: SIS,
    },
    ExactRoute {
        path: "/modules/sis/settings",
        access: SIS,
    },
    ExactRoute {
        path: "/modules/academics",
        access: ACADEMICS,
    },
    ExactRoute {
        path: "/modules/academics/academic-years",
        access: ACADEMICS,
    },
    ExactRoute {
        path: "/modules/academics/assessments",
        access: ACADEMICS,
    },
    ExactRoute {
        path: "/modules/academics/classes",
        access: ACADEMICS,
    },
    ExactRoute {
        path: "/modules/academics/grade-levels",
        access: ACADEMICS,
    },
    ExactRoute {
        path: "/modules/academics/gradebook",
        access: ACADEMICS,
    },
    ExactRoute {
        path: "/modules/academics/reporting",
        access: ACADEMICS,
    },
    ExactRoute {
        path: "/modules/academics/subjects",
        access: ACADEMICS,
    },
    ExactRoute {
        path: "/modules/academics/teachers",
        access: ACADEMICS,
    },
    ExactRoute {
        path: "/modules/academics/teaching-assignments",
        access: ACADEMICS,
    },
    ExactRoute {
        path: "/modules/academics/terms",
        access: ACADEMICS,
    },
    ExactRoute {
        path: "/modules/attendance",
        access: ATTENDANCE,
    },
    ExactRoute {
        path: "/modules/attendance/registers",
        access: ATTENDANCE,
    },
    ExactRoute {
        path: "/modules/timetabling",
        access: TIMETABLING,
    },
    ExactRoute {
        path: "/modules/messaging",
        access: MESSAGING,
    },
    ExactRoute {
        path: "/modules/messaging/inbox",
        access: MESSAGING,
    },
    ExactRoute {
        path: "/modules/messaging/delivery-history",
        access: MESSAGING_SEND,
    },
    ExactRoute {
        path: "/modules/finance",
        access: FINANCE,
    },
    ExactRoute {
        path: "/modules/finance/accounting-periods",
        access: FINANCE,
    },
    ExactRoute {
        path: "/modules/finance/chart-of-accounts",
        access: FINANCE,
    },
    ExactRoute {
        path: "/modules/finance/currencies",
        access: FINANCE,
    },
    ExactRoute {
        path: "/modules/finance/journals",
        access: FINANCE,
    },
    ExactRoute {
        path: "/modules/finance/posting-requests",
        access: FINANCE,
    },
    ExactRoute {
        path: "/modules/fees",
        access: FEES,
    },
    ExactRoute {
        path: "/modules/fees/billing-accounts",
        access: FEES,
    },
    ExactRoute {
        path: "/modules/fees/fee-structures",
        access: FEES,
    },
    ExactRoute {
        path: "/modules/fees/imports",
        access: FEES_IMPORTS,
    },
    ExactRoute {
        path: "/modules/fees/invoices",
        access: FEES,
    },
    ExactRoute {
        path: "/modules/library",
        access: LIBRARY,
    },
    ExactRoute {
        path: "/modules/hr-payroll",
        access: HR_PAYROLL,
    },
    ExactRoute {
        path: "/modules/hr-payroll/availability",
        access: HR_PAYROLL,
    },
    ExactRoute {
        path: "/modules/hr-payroll/departments",
        access: HR_PAYROLL,
    },
    ExactRoute {
        path: "/modules/hr-payroll/employees",
        access: HR_PAYROLL,
    },
    ExactRoute {
        path: "/modules/hr-payroll/employment",
        access: HR_PAYROLL,
    },
    ExactRoute {
        path: "/modules/hr-payroll/imports",
        access: HR_PAYROLL,
    },
    ExactRoute {
        path: "/modules/hr-payroll/positions",
        access: HR_PAYROLL,
    },
    ExactRoute {
        path: "/modules/procurement",
        access: PROCUREMENT,
    },
    ExactRoute {
        path: "/modules/procurement/goods-receipts",
        access: PROCUREMENT,
    },
    ExactRoute {
        path: "/modules/procurement/purchase-orders",
        access: PROCUREMENT,
    },
    ExactRoute {
        path: "/modules/procurement/requisitions",
        access: PROCUREMENT,
    },
    ExactRoute {
        path: "/modules/procurement/suppliers",
        access: PROCUREMENT,
    },
    ExactRoute {
        path: "/modules/fleet",
        access: FLEET,
    },
    ExactRoute {
        path: "/modules/fleet/daily-log",
        access: FLEET,
    },
    ExactRoute {
        path: "/modules/fleet/drivers",
        access: FLEET,
    },
    ExactRoute {
        path: "/modules/fleet/vehicles",
        access: FLEET,
    },
    ExactRoute {
        path: "/modules/hostel",
        access: HOSTEL,
    },
    ExactRoute {
        path: "/modules/health",
        access: HEALTH,
    },
    ExactRoute {
        path: "/modules/assets-inventory",
        access: ASSETS_INVENTORY,
    },
    ExactRoute {
        path: "/modules/assets-inventory/items",
        access: ASSETS_INVENTORY,
    },
    ExactRoute {
        path: "/modules/assets-inventory/movements",
        access: ASSETS_INVENTORY,
    },
    ExactRoute {
        path: "/modules/assets-inventory/procurement-receipts",
        access: ASSETS_RECEIVING,
    },
    ExactRoute {
        path: "/modules/assets-inventory/requests",
        access: ASSETS_INVENTORY,
    },
    ExactRoute {
        path: "/modules/assets-inventory/stock",
        access: ASSETS_INVENTORY,
    },
    ExactRoute {
        path: "/modules/assets-inventory/stores",
        access: ASSETS_INVENTORY,
    },
    ExactRoute {
        path: "/modules/document-registry",
        access: DOCUMENT_REGISTRY,
    },
    ExactRoute {
        path: "/modules/internal-audit",
        access: INTERNAL_AUDIT,
    },
    ExactRoute {
        path: "/modules/agent",
        access: AGENT,
    },
    ExactRoute {
        path: "/modules/agent/usage",
        access: AGENT,
    },
];

const UUID_ROUTES: &[UuidRoute] = &[
    UuidRoute {
        prefix: "/modules/academics/reporting/report-batches/",
        access: ACADEMICS,
    },
    UuidRoute {
        prefix: "/modules/academics/reporting/transcripts/",
        access: ACADEMICS,
    },
    UuidRoute {
        prefix: "/modules/academics/gradebook/mark-sheets/",
        access: ACADEMICS,
    },
    UuidRoute {
        prefix: "/modules/attendance/registers/",
        access: ATTENDANCE,
    },
    UuidRoute {
        prefix: "/modules/messaging/announcements/",
        access: MESSAGING_MANAGE,
    },
    UuidRoute {
        prefix: "/modules/sis/applications/",
        access: SIS,
    },
    UuidRoute {
        prefix: "/modules/sis/learners/",
        access: SIS,
    },
    UuidRoute {
        prefix: "/modules/academics/classes/",
        access: ACADEMICS,
    },
    UuidRoute {
        prefix: "/modules/procurement/purchase-orders/",
        access: PROCUREMENT,
    },
    UuidRoute {
        prefix: "/modules/procurement/requisitions/",
        access: PROCUREMENT,
    },
    UuidRoute {
        prefix: "/modules/assets-inventory/movements/",
        access: ASSETS_INVENTORY,
    },
    UuidRoute {
        prefix: "/modules/assets-inventory/requests/",
        access: ASSETS_INVENTORY,
    },
    UuidRoute {
        prefix: "/modules/agent/sessions/",
        access: AGENT,
    },
];

/// A request-scoped proof that the submitted origin is a current client route
/// which the caller can open in this tenant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AttestedAgentOrigin {
    module_key: &'static str,
    route: String,
}

impl AttestedAgentOrigin {
    pub(super) fn parse(
        claimed_module_key: &str,
        route: &str,
        access: &AccessContext,
    ) -> Result<Self, OriginAttestationError> {
        let route_access = route_access(route).ok_or(OriginAttestationError::UnknownRoute)?;
        if claimed_module_key != route_access.module_key {
            return Err(OriginAttestationError::ModuleMismatch);
        }
        let entitled = [
            Some(route_access.required_module),
            route_access.additional_module,
        ]
        .into_iter()
        .flatten()
        .all(|module_key| {
            access.entitlements.module_state(module_key) == Some(ModuleEntitlementState::Enabled)
                && access.has_module(module_key)
        });
        let permitted = [
            route_access.required_permission,
            route_access.additional_permission,
        ]
        .into_iter()
        .flatten()
        .all(|permission| access.has_permission(permission));
        if !entitled || !permitted {
            return Err(OriginAttestationError::AccessDenied);
        }
        Ok(Self {
            module_key: route_access.module_key,
            route: route.to_owned(),
        })
    }

    pub(super) const fn module_key(&self) -> &'static str {
        self.module_key
    }

    pub(super) fn route(&self) -> &str {
        &self.route
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OriginAttestationError {
    UnknownRoute,
    ModuleMismatch,
    AccessDenied,
}

impl OriginAttestationError {
    pub(super) const fn code(self) -> &'static str {
        match self {
            Self::UnknownRoute => "invalid_origin_route",
            Self::ModuleMismatch => "origin_route_module_mismatch",
            Self::AccessDenied => "origin_access_denied",
        }
    }

    pub(super) const fn safe_message(self) -> &'static str {
        match self {
            Self::UnknownRoute => "Origin route is not a recognized Campus Pilot page",
            Self::ModuleMismatch => "Origin module does not match the origin route",
            Self::AccessDenied => "Origin page is not available to this account",
        }
    }
}

fn route_access(route: &str) -> Option<OriginAccess> {
    if route.trim() != route
        || route.contains(['?', '#', '%'])
        || route.contains("//")
        || route
            .split('/')
            .any(|segment| matches!(segment, "." | ".."))
    {
        return None;
    }
    if let Some(entry) = EXACT_ROUTES.iter().find(|entry| entry.path == route) {
        return Some(entry.access);
    }
    UUID_ROUTES.iter().find_map(|entry| {
        let identifier = route.strip_prefix(entry.prefix)?;
        let parsed = Uuid::parse_str(identifier).ok()?;
        (parsed.hyphenated().to_string() == identifier).then_some(entry.access)
    })
}

#[cfg(test)]
mod tests {
    use cp_common::{AccessContext, EntitlementSnapshot, LeaseLifecycle, ModuleEntitlementState};
    use uuid::Uuid;

    use super::{AttestedAgentOrigin, OriginAttestationError, route_access};
    use crate::services::access::catalog::module_catalog;

    fn access(permissions: &[&str], modules: &[&str]) -> AccessContext {
        AccessContext {
            role_keys: vec!["test-role".to_owned()],
            permissions: permissions
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            enabled_modules: modules.iter().map(|value| (*value).to_owned()).collect(),
            entitlements: EntitlementSnapshot::new(
                LeaseLifecycle::Active,
                modules
                    .iter()
                    .map(|value| ((*value).to_owned(), ModuleEntitlementState::Enabled)),
                [],
            )
            .unwrap(),
        }
    }

    #[test]
    fn every_catalog_module_root_has_one_canonical_origin_mapping() {
        for module in module_catalog() {
            let mapped = route_access(module.route)
                .unwrap_or_else(|| panic!("missing Agent origin route for {}", module.route));
            assert_eq!(mapped.module_key, module.key);
            assert!(
                mapped.required_module == module.key
                    || mapped.additional_module == Some(module.key)
            );
        }
    }

    #[test]
    fn canonical_static_and_uuid_routes_resolve_to_their_client_module() {
        let identifier = Uuid::new_v4();
        let cases = [
            ("/home".to_owned(), "home"),
            ("/admin/users".to_owned(), "administration"),
            ("/admin/agent/providers".to_owned(), "administration"),
            ("/modules/fleet/vehicles".to_owned(), "fleet"),
            ("/modules/fees/imports".to_owned(), "fees"),
            (format!("/modules/sis/learners/{identifier}"), "sis"),
            (
                format!("/modules/assets-inventory/requests/{identifier}"),
                "assets_inventory",
            ),
            (format!("/modules/agent/sessions/{identifier}"), "agent"),
        ];
        for (route, expected_module) in cases {
            assert_eq!(
                route_access(&route).map(|access| access.module_key),
                Some(expected_module)
            );
        }
    }

    #[test]
    fn route_parser_rejects_unknown_noncanonical_and_non_uuid_paths() {
        for route in [
            "https://campus.example/modules/fleet",
            "/modules/fleet/unknown",
            "/modules/fleet?source=widget",
            "/modules//fleet",
            "/modules/fleet/../sis",
            "/modules/agent/sessions/not-a-uuid",
            "/modules/agent/sessions/550E8400-E29B-41D4-A716-446655440000",
            " /home",
        ] {
            assert_eq!(
                route_access(route),
                None,
                "route should be rejected: {route}"
            );
        }
    }

    #[test]
    fn attestation_binds_route_module_entitlement_and_exact_route_access() {
        let fleet_access = access(&["fleet:view"], &["fleet"]);
        assert!(
            AttestedAgentOrigin::parse("fleet", "/modules/fleet/vehicles", &fleet_access).is_ok()
        );
        assert_eq!(
            AttestedAgentOrigin::parse("sis", "/modules/fleet/vehicles", &fleet_access),
            Err(OriginAttestationError::ModuleMismatch)
        );
        assert_eq!(
            AttestedAgentOrigin::parse(
                "fleet",
                "/modules/fleet/vehicles",
                &access(&[], &["fleet"])
            ),
            Err(OriginAttestationError::AccessDenied)
        );
        assert_eq!(
            AttestedAgentOrigin::parse(
                "fleet",
                "/modules/fleet/vehicles",
                &access(&["fleet:view"], &[])
            ),
            Err(OriginAttestationError::AccessDenied)
        );

        let mut locally_disabled = access(&["fleet:view"], &["fleet"]);
        locally_disabled.entitlements = EntitlementSnapshot::new(
            LeaseLifecycle::Active,
            [("fleet".to_owned(), ModuleEntitlementState::LocallyDisabled)],
            [],
        )
        .unwrap();
        assert_eq!(
            AttestedAgentOrigin::parse("fleet", "/modules/fleet/vehicles", &locally_disabled),
            Err(OriginAttestationError::AccessDenied)
        );
    }

    #[test]
    fn attestation_failures_have_stable_transport_codes_and_safe_messages() {
        let cases = [
            (
                OriginAttestationError::UnknownRoute,
                "invalid_origin_route",
                "Origin route is not a recognized Campus Pilot page",
            ),
            (
                OriginAttestationError::ModuleMismatch,
                "origin_route_module_mismatch",
                "Origin module does not match the origin route",
            ),
            (
                OriginAttestationError::AccessDenied,
                "origin_access_denied",
                "Origin page is not available to this account",
            ),
        ];
        for (error, code, message) in cases {
            assert_eq!(error.code(), code);
            assert_eq!(error.safe_message(), message);
        }
    }

    #[test]
    fn global_widget_origins_preserve_home_and_require_admin_agent_access() {
        assert!(AttestedAgentOrigin::parse("home", "/home", &access(&[], &["home"])).is_ok());

        let administration_agent = access(
            &["administration:view", "ai_providers:view"],
            &["administration", "agent"],
        );
        assert!(
            AttestedAgentOrigin::parse(
                "administration",
                "/admin/agent/providers",
                &administration_agent,
            )
            .is_ok()
        );
        assert_eq!(
            AttestedAgentOrigin::parse(
                "administration",
                "/admin/agent/providers",
                &access(
                    &["administration:view", "ai_providers:view"],
                    &["administration"]
                ),
            ),
            Err(OriginAttestationError::AccessDenied)
        );
    }

    #[test]
    fn wildcard_permission_never_bypasses_origin_module_entitlement() {
        assert!(
            AttestedAgentOrigin::parse(
                "assets_inventory",
                "/modules/assets-inventory/stock",
                &access(&["*"], &["assets_inventory"]),
            )
            .is_ok()
        );
        assert_eq!(
            AttestedAgentOrigin::parse(
                "assets_inventory",
                "/modules/assets-inventory/stock",
                &access(&["*"], &[]),
            ),
            Err(OriginAttestationError::AccessDenied)
        );
    }
}
