//
//  cp-common
//  access.rs
//
//  Created by OpenAI Codex on 2026/08/26.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

#[derive(Debug, Clone)]
pub struct AccessContext {
    pub role_keys: Vec<String>,
    pub permissions: Vec<String>,
    pub enabled_modules: Vec<String>,
}

impl AccessContext {
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions
            .iter()
            .any(|item| item == "*" || item == permission)
    }

    pub fn has_module(&self, module_key: &str) -> bool {
        self.enabled_modules.iter().any(|item| item == module_key)
    }

    /// A dedicated role-management permission authorizes catalog access
    /// administration. Only wildcard access remains owner-delegable.
    pub fn can_delegate_permissions(&self, requested: &[String]) -> bool {
        self.has_permission("*") || requested.iter().all(|permission| permission != "*")
    }
}

pub fn module_key_for_namespace(namespace: &str) -> &str {
    match namespace {
        "users" | "roles" | "licensing" | "school_settings" | "kernel" | "storage" => {
            "administration"
        }
        "vehicles" | "drivers" | "vehicle_log" | "vehicle_logs" => "fleet",
        "health_services" => "health",
        "hr-payroll" => "hr_payroll",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::AccessContext;

    fn access(permissions: &[&str]) -> AccessContext {
        AccessContext {
            role_keys: Vec::new(),
            permissions: permissions.iter().map(|value| value.to_string()).collect(),
            enabled_modules: vec!["administration".to_string()],
        }
    }

    #[test]
    fn role_administrator_can_delegate_catalog_permissions_but_not_wildcard() {
        let administrator = access(&["users:view", "users:edit"]);
        assert!(administrator.can_delegate_permissions(&["users:view".to_string()]));
        assert!(administrator.can_delegate_permissions(&["roles:edit".to_string()]));
        assert!(!administrator.can_delegate_permissions(&["*".to_string()]));
    }

    #[test]
    fn wildcard_holder_can_delegate_any_catalog_permission() {
        assert!(access(&["*"]).can_delegate_permissions(&["*".to_string()]));
    }
}
