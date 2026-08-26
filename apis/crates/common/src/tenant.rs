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
