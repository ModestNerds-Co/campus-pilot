//
//  cp-common
//  lib.rs
//
//  Created by Ngonidzashe Mangudya on 2026/08/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//
//  Shared, dependency-free types used by the `app` crate and every ERP
//  module crate. This crate never depends on `app` or on any module crate.
//

pub mod api_response;
pub mod attachment_file;
pub mod permissions;
pub mod roles;
pub mod status_info;
pub mod tenant;
pub mod typedefs;
pub mod validation;

pub use api_response::{ApiResponse, PaginationMeta};
pub use attachment_file::AttachmentFile;
pub use permissions::RequirePermission;
pub use roles::Roles;
pub use status_info::{StatusInfo, status_meaning};
pub use tenant::TenantId;
pub use typedefs::ApiResult;
pub use validation::flatten_validation_errors;
