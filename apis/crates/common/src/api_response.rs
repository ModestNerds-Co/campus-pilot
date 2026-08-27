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

#[cfg(test)]
mod tests {
    use actix_web::http::StatusCode;

    use super::{ApiResponse, PaginationMeta};

    #[test]
    fn pagination_reports_page_boundaries() {
        let first = PaginationMeta::new(1, 10, 21);
        assert_eq!(first.total_pages, 3);
        assert!(first.has_next);
        assert!(!first.has_prev);

        let middle = PaginationMeta::new(2, 10, 21);
        assert!(middle.has_next);
        assert!(middle.has_prev);

        let last = PaginationMeta::new(3, 10, 21);
        assert!(!last.has_next);
        assert!(last.has_prev);

        let empty = PaginationMeta::new(1, 10, 0);
        assert_eq!(empty.total_pages, 0);
        assert!(!empty.has_next);
        assert!(!empty.has_prev);
    }

    #[test]
    fn response_uses_first_bad_request_issue_and_standard_status_messages() {
        let bad = ApiResponse::<()>::from_status(
            StatusCode::BAD_REQUEST,
            None,
            Some(vec![
                "Correct the email address".to_string(),
                "Ignored".to_string(),
            ]),
        );
        assert!(!bad.success);
        assert_eq!(bad.message.as_deref(), Some("Correct the email address"));
        assert_eq!(bad.version, 1);
        assert_eq!(bad.by, "Codecraft Solutions");

        let empty_issues =
            ApiResponse::<()>::from_status(StatusCode::BAD_REQUEST, None, Some(Vec::new()));
        assert_eq!(empty_issues.message.as_deref(), Some("Bad Request"));

        let created = ApiResponse::from_status(StatusCode::CREATED, Some("created"), None);
        assert!(created.success);
        assert_eq!(created.message.as_deref(), Some("Created"));
        assert_eq!(created.data, Some("created"));
    }

    #[test]
    fn response_can_include_pagination() {
        let pagination = PaginationMeta::new(2, 20, 45);
        let response =
            ApiResponse::with_pagination(StatusCode::OK, Some(vec![1, 2]), pagination, None);
        let pagination = response.pagination.expect("pagination should be present");
        assert_eq!(pagination.current_page, 2);
        assert_eq!(pagination.per_page, 20);
        assert_eq!(pagination.total, 45);
        assert_eq!(response.data, Some(vec![1, 2]));
    }
}
