//
//  campus-pilot-apis
//  api_response.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/22.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use crate::utils::status_meaning;
use actix_web::http::StatusCode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub message: Option<String>,
    pub data: Option<T>,
    pub issues: Option<Vec<String>>,
    pub version: u8,
    pub by: String,
}

impl<T> ApiResponse<T> {
    pub fn from_status(code: StatusCode, data: Option<T>, issues: Option<Vec<String>>) -> Self {
        let status_info = status_meaning(code);

        let auto_msg = match (code.as_u16(), &issues) {
            (400, Some(errs)) if !errs.is_empty() => errs
                .first()
                .cloned()
                .unwrap_or(status_info.meaning.to_string()),
            _ => status_info.meaning.to_string(),
        };

        Self {
            success: code.is_success(),
            message: Some(auto_msg),
            data,
            issues,
            version: 1,
            by: "Codecraft Solutions".to_string(),
        }
    }
}
