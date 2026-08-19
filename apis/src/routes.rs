//
//  campus-pilot-apis
//  routes.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use crate::models::api_response::ApiResponse;
use crate::services::{auth, kernel, roles, storage, users};
use actix_web::http::StatusCode;
use actix_web::web::{ServiceConfig, scope};
use actix_web::{HttpResponse, Responder, get};
use serde_json::json;
use sqlx::types::chrono::Utc;

#[get("/health-check")]
async fn health_check() -> impl Responder {
    let health_data = json!({
        "status": "healthy",
        "service": "campus-pilot",
        "version": "1.0.0",
        "timestamp": Utc::now().to_rfc3339()
    });

    let response = ApiResponse::from_status(StatusCode::OK, Some(health_data), None);
    HttpResponse::Ok().json(response)
}

pub fn init(cfg: &mut ServiceConfig) {
    cfg.service(
        scope("/api/1.0")
            .service(health_check)
            .configure(auth::routes)
            .configure(roles::routes::routes)
            .configure(users::routes::routes)
            .service(scope("/kernel").configure(kernel::routes::init))
            .service(scope("/storage").configure(storage::routes::init)),
    );
}
