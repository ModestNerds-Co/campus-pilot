//
//  campus-pilot-apis
//  mod.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

mod jwt;
#[expect(
    clippy::module_inception,
    reason = "this legacy private module remains the password utility implementation behind stable re-exports"
)]
mod utils;

pub use jwt::{
    Claims, generate_access_token, generate_refresh_token, verify_access_token, verify_token,
};
pub use utils::{hash_password, verify_password};

pub use cp_common::flatten_validation_errors;
