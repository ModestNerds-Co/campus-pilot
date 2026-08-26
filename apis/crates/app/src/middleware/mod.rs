//
//  campus-pilot-apis
//  mod.rs
//
//  Created by Ngonidzashe Mangudya on 2025/10/02.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

pub mod auth;
pub mod rate_limit;

pub use auth::AuthMiddleware;
pub use rate_limit::{auth_rate_limiter, refresh_rate_limiter};

// RequirePermission now lives in cp-common so module crates can gate their
// own routes without depending on `app`; re-exported here so existing
// `crate::middleware::RequirePermission` call sites keep working.
pub use cp_common::RequirePermission;
