//
//  cp-common
//  api_response.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/22.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use crate::status_info::status_meaning;
use actix_web::http::StatusCode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginationMeta {
    pub current_page: u32,
    pub per_page: u32,
    pub total: i64,
    pub total_pages: u32,
    pub has_next: bool,
    pub has_prev: bool,
}

impl PaginationMeta {
    pub fn new(current_page: u32, per_page: u32, total: i64) -> Self {
        let total_pages = ((total as f64) / (per_page as f64)).ceil() as u32;
        Self {
            current_page,
            per_page,
            total,
            total_pages,
            has_next: current_page < total_pages,
            has_prev: current_page > 1,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub message: Option<String>,
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<PaginationMeta>,
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
            pagination: None,
            issues,
            version: 1,
            by: "Codecraft Solutions".to_string(),
        }
    }

    pub fn with_pagination(
        code: StatusCode,
        data: Option<T>,
        pagination: PaginationMeta,
        issues: Option<Vec<String>>,
    ) -> Self {
        let mut response = Self::from_status(code, data, issues);
        response.pagination = Some(pagination);
        response
    }
}
