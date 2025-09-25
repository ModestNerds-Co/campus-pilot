//
//  campus-pilot-apis
//  utils.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/22.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use crate::models::status_info::StatusInfo;
use actix_web::http::StatusCode;

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
