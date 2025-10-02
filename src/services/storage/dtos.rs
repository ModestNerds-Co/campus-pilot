//
//  campus-pilot-apis
//  dtos.rs
//
//  Created by Ngonidzashe Mangudya on 2025/10/01.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Deserialize, Validate)]
pub struct GenerateUploadUrlRequest {
    #[validate(length(min = 1, message = "Filename is required"))]
    pub filename: String,
    #[validate(length(min = 1, message = "File type is required"))]
    pub file_type: String,
}

#[derive(Serialize)]
pub struct GenerateUploadUrlResponse {
    pub upload_url: String,
    pub file_key: String,
    pub expires_in: u64,
    pub headers: UploadHeaders,
}

#[derive(Serialize)]
pub struct UploadHeaders {
    #[serde(rename = "x-amz-acl")]
    pub acl: String,
}
