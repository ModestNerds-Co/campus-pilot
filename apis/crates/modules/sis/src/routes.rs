//
//  cp-sis
//  routes.rs
//
//  Created by Ngonidzashe Mangudya on 2026/08/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, get, web};
use cp_common::ApiResponse;

#[get("/status")]
async fn status() -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::from_status(
        StatusCode::OK,
        Some(serde_json::json!({ "module": "sis", "status": "not_implemented" })),
        None,
    ))
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(status);
}
