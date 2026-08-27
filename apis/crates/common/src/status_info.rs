//
//  cp-common
//  status_info.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/22.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use actix_web::http::StatusCode;

pub struct StatusInfo {
    pub meaning: &'static str,
    pub description: &'static str,
}

pub fn status_meaning(code: StatusCode) -> StatusInfo {
    match code {
        StatusCode::OK => StatusInfo {
            meaning: "OK",
            description: "Request was executed successfully",
        },
        StatusCode::CREATED => StatusInfo {
            meaning: "Created",
            description: "Resource was created successfully",
        },
        StatusCode::ACCEPTED => StatusInfo {
            meaning: "Accepted",
            description: "Request was accepted but not yet executed",
        },
        StatusCode::NO_CONTENT => StatusInfo {
            meaning: "No Content",
            description: "No content was returned",
        },
        StatusCode::BAD_REQUEST => StatusInfo {
            meaning: "Bad Request",
            description: "Request was malformed",
        },
        StatusCode::UNAUTHORIZED => StatusInfo {
            meaning: "Unauthorized",
            description: "Request was not authorized",
        },
        StatusCode::FORBIDDEN => StatusInfo {
            meaning: "Forbidden",
            description: "Request was forbidden",
        },
        StatusCode::NOT_FOUND => StatusInfo {
            meaning: "Not Found",
            description: "Resource was not found",
        },
        StatusCode::METHOD_NOT_ALLOWED => StatusInfo {
            meaning: "Method Not Allowed",
            description: "Method was not allowed",
        },
        StatusCode::CONFLICT => StatusInfo {
            meaning: "Conflict",
            description: "Request was not executed due to conflict",
        },
        StatusCode::INTERNAL_SERVER_ERROR => StatusInfo {
            meaning: "Server Error",
            description: "Server was unable to execute request",
        },
        _ => StatusInfo {
            meaning: "Unknown Status",
            description: "No mapped description available",
        },
    }
}

#[cfg(test)]
mod tests {
    use actix_web::http::StatusCode;

    use super::status_meaning;

    #[test]
    fn mapped_statuses_have_stable_meanings_and_descriptions() {
        for (status, meaning, description) in [
            (StatusCode::OK, "OK", "Request was executed successfully"),
            (
                StatusCode::CREATED,
                "Created",
                "Resource was created successfully",
            ),
            (
                StatusCode::ACCEPTED,
                "Accepted",
                "Request was accepted but not yet executed",
            ),
            (
                StatusCode::NO_CONTENT,
                "No Content",
                "No content was returned",
            ),
            (
                StatusCode::BAD_REQUEST,
                "Bad Request",
                "Request was malformed",
            ),
            (
                StatusCode::UNAUTHORIZED,
                "Unauthorized",
                "Request was not authorized",
            ),
            (StatusCode::FORBIDDEN, "Forbidden", "Request was forbidden"),
            (StatusCode::NOT_FOUND, "Not Found", "Resource was not found"),
            (
                StatusCode::METHOD_NOT_ALLOWED,
                "Method Not Allowed",
                "Method was not allowed",
            ),
            (
                StatusCode::CONFLICT,
                "Conflict",
                "Request was not executed due to conflict",
            ),
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Server Error",
                "Server was unable to execute request",
            ),
        ] {
            let info = status_meaning(status);
            assert_eq!(info.meaning, meaning);
            assert_eq!(info.description, description);
        }
    }

    #[test]
    fn unmapped_status_is_explicitly_unknown() {
        let info = status_meaning(StatusCode::IM_A_TEAPOT);
        assert_eq!(info.meaning, "Unknown Status");
        assert_eq!(info.description, "No mapped description available");
    }
}
