//
//  cp-common
//  roles.rs
//
//  Created by Ngonidzashe Mangudya on 2026/08/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//
//  The authenticated user's role names for this request, inserted into
//  `req.extensions()` by `app`'s `AuthMiddleware` alongside `TenantId`.
//  Module crates read this via `web::ReqData<Roles>` — through
//  `RequirePermission` below, or directly — without depending on `app`'s
//  full `User` model.

#[derive(Debug, Clone)]
pub struct Roles(pub Vec<String>);

impl Roles {
    pub fn contains(&self, role: &str) -> bool {
        self.0.iter().any(|r| r == role)
    }
}

#[cfg(test)]
mod tests {
    use super::Roles;

    #[test]
    fn role_membership_is_exact() {
        let roles = Roles(vec!["campus_owner".to_string(), "teacher".to_string()]);
        assert!(roles.contains("campus_owner"));
        assert!(roles.contains("teacher"));
        assert!(!roles.contains("student"));
        assert!(!roles.contains("Teacher"));
    }
}
