//
//  cp-common
//  tenant.rs
//
//  Created by Ngonidzashe Mangudya on 2026/08/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//
//  Request-scoped tenant identity. `AuthMiddleware` (in the `app` crate)
//  inserts this into `req.extensions()` after loading the authenticated
//  user; every module crate reads it back with the actix-provided
//  `web::ReqData<TenantId>` extractor to scope its queries, without needing
//  to depend on `app` or know anything about how auth works.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TenantId(pub Uuid);

impl TenantId {
    pub fn into_inner(self) -> Uuid {
        self.0
    }
}

impl std::fmt::Display for TenantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::TenantId;

    #[test]
    fn tenant_id_preserves_and_displays_the_uuid() {
        let uuid = Uuid::parse_str("9ab4dca0-075a-4d26-b9c8-66d0671af522")
            .unwrap_or_else(|_| unreachable!());
        let tenant = TenantId(uuid);
        assert_eq!(tenant.to_string(), uuid.to_string());
        assert_eq!(tenant.into_inner(), uuid);
    }

    #[test]
    fn tenant_id_serializes_as_the_wrapped_uuid() {
        let tenant = TenantId(Uuid::nil());
        let json = serde_json::to_string(&tenant).unwrap_or_else(|_| unreachable!());
        let decoded: TenantId = serde_json::from_str(&json).unwrap_or_else(|_| unreachable!());
        assert_eq!(decoded, tenant);
    }
}
