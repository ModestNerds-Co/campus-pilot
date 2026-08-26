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
