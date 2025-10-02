//
//  campus-pilot-apis
//  routes.rs
//
//  Created by Ngonidzashe Mangudya on 2025/10/01.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use actix_web::{HttpResponse, post, web, web::ServiceConfig};
use nanoid::nanoid;
use validator::Validate;

use crate::{
    models::api_response::ApiResponse,
    services::storage::dtos::{GenerateUploadUrlRequest, GenerateUploadUrlResponse, UploadHeaders},
    state::AppState,
    utils::flatten_validation_errors,
};

#[post("generate-upload-url")]
pub async fn generate_upload_url(
    app_state: web::Data<AppState>,
    req: web::Json<GenerateUploadUrlRequest>,
) -> actix_web::Result<HttpResponse> {
    let request = req.into_inner();

    // Validate request
    if let Err(errors) = request.validate() {
        let issues = flatten_validation_errors(&errors);
        return Ok(
            HttpResponse::BadRequest().json(ApiResponse::<()>::from_status(
                actix_web::http::StatusCode::BAD_REQUEST,
                None,
                Some(issues),
            )),
        );
    }

    // Generate unique file key
    let file_extension = request.filename.split('.').last().unwrap_or("bin");
    let file_key = format!("uploads/{}.{}", nanoid!(16), file_extension);

    // Generate presigned URL (15 minutes expiry)
    let expires_in = 900u64;
    match app_state
        .storage_ops
        .generate_upload_url(&file_key, expires_in)
        .await
    {
        Ok(upload_url) => {
            let response = GenerateUploadUrlResponse {
                upload_url,
                file_key,
                expires_in,
                headers: UploadHeaders {
                    acl: "public-read".to_string(),
                },
            };

            Ok(HttpResponse::Ok().json(ApiResponse::from_status(
                actix_web::http::StatusCode::OK,
                Some(response),
                None,
            )))
        }
        Err(e) => Ok(
            HttpResponse::InternalServerError().json(ApiResponse::<()>::from_status(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                None,
                Some(vec![format!("Failed to generate upload URL: {}", e)]),
            )),
        ),
    }
}

pub fn init(cfg: &mut ServiceConfig) {
    cfg.service(generate_upload_url);
}
