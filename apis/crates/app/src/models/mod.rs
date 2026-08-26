//
//  campus-pilot-apis
//  mod.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//
//  These types now live in the `cp-common` crate (shared with every ERP
//  module crate); re-exported here under their old paths so existing
//  `crate::models::api_response::...` imports keep working unchanged.
//

pub use cp_common::api_response;
pub use cp_common::typedefs;

pub use cp_common::ApiResponse;
