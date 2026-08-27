use actix_multipart::Multipart;
use actix_web::{HttpResponse, get, post, put, web, web::ServiceConfig};
use cp_common::RequirePermission;
use futures_util::StreamExt;
use serde_json::json;
use validator::Validate;

use crate::{
    middleware::AuthMiddleware,
    models::api_response::ApiResponse,
    services::kernel::dtos::{
        CreateAdminReq, LogoUploadResponse, SetupSchoolRequest, UpdateSchoolProfileRequest,
    },
    state::AppState,
    utils::{flatten_validation_errors, hash_password},
};

#[get("status")]
pub async fn get_kernel_status(app_state: web::Data<AppState>) -> actix_web::Result<HttpResponse> {
    let status = app_state.kernel_db.get_kernel_status().await.unwrap();

    let response = HttpResponse::Ok().json(ApiResponse::from_status(
        actix_web::http::StatusCode::OK,
        Some(status),
        None,
    ));
    Ok(response)
}

#[post("setup-school")]
pub async fn setup_school(
    app_state: web::Data<AppState>,
    req: web::Json<SetupSchoolRequest>,
) -> actix_web::Result<HttpResponse> {
    let setup_req = req.into_inner();

    // Validate using validator crate
    if let Err(errors) = setup_req.validate() {
        let issues = flatten_validation_errors(&errors);
        return Ok(
            HttpResponse::BadRequest().json(ApiResponse::<()>::from_status(
                actix_web::http::StatusCode::BAD_REQUEST,
                None,
                Some(issues),
            )),
        );
    }

    // Attempt to setup school
    match app_state.kernel_db.setup_school(setup_req).await {
        Ok(_) => Ok(HttpResponse::Ok().json(ApiResponse::<()>::from_status(
            actix_web::http::StatusCode::OK,
            None,
            None,
        ))),
        Err(e) => Ok(
            HttpResponse::InternalServerError().json(ApiResponse::<()>::from_status(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                None,
                Some(vec![format!("Failed to setup school: {}", e)]),
            )),
        ),
    }
}

#[post("setup-admin")]
pub async fn setup_admin(
    app_state: web::Data<AppState>,
    req: web::Json<CreateAdminReq>,
) -> actix_web::Result<HttpResponse> {
    let admin_req = req.into_inner();

    // Validate using validator crate
    if let Err(errors) = admin_req.validate() {
        let issues = flatten_validation_errors(&errors);
        return Ok(
            HttpResponse::BadRequest().json(ApiResponse::<()>::from_status(
                actix_web::http::StatusCode::BAD_REQUEST,
                None,
                Some(issues),
            )),
        );
    }

    // Check system state - must be SchoolConfigured
    let status = app_state.kernel_db.get_kernel_status().await;
    match status {
        Ok(kernel_status) if kernel_status.state.to_string() == "SchoolConfigured" => {
            // Proceed with admin creation
        }
        Ok(_) => {
            return Ok(
                HttpResponse::BadRequest().json(ApiResponse::<()>::from_status(
                    actix_web::http::StatusCode::BAD_REQUEST,
                    None,
                    Some(vec![
                        "System must be in SchoolConfigured state to create admin user".to_string(),
                    ]),
                )),
            );
        }
        Err(e) => {
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::<()>::from_status(
                    actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                    None,
                    Some(vec![format!("Failed to check system status: {}", e)]),
                )),
            );
        }
    }

    // Hash password
    let password_hash = match hash_password(&admin_req.password) {
        Ok(hash) => hash,
        Err(e) => {
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::<()>::from_status(
                    actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                    None,
                    Some(vec![format!("Failed to hash password: {}", e)]),
                )),
            );
        }
    };

    // Create admin user
    match app_state
        .kernel_db
        .create_admin_user(
            &admin_req.full_name,
            &admin_req.email,
            admin_req.phone.as_deref(),
            &password_hash,
        )
        .await
    {
        Ok(user_id) => Ok(HttpResponse::Ok().json(ApiResponse::from_status(
            actix_web::http::StatusCode::OK,
            Some(json!({
                "user_id": user_id,
                "email": admin_req.email,
                "full_name": admin_req.full_name,
                "message": "Admin user created successfully. System is now Ready."
            })),
            None,
        ))),
        Err(e) => Ok(
            HttpResponse::InternalServerError().json(ApiResponse::<()>::from_status(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                None,
                Some(vec![format!("Failed to create admin user: {}", e)]),
            )),
        ),
    }
}

#[get("school-profile")]
pub async fn get_school_profile(app_state: web::Data<AppState>) -> actix_web::Result<HttpResponse> {
    match app_state.kernel_db.get_school_profile().await {
        Ok(profile) => Ok(HttpResponse::Ok().json(ApiResponse::from_status(
            actix_web::http::StatusCode::OK,
            Some(profile),
            None,
        ))),
        Err(e) => {
            log::error!("Failed to get school profile: {:?}", e);
            Ok(
                HttpResponse::InternalServerError().json(ApiResponse::<()>::from_status(
                    actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                    None,
                    Some(vec![format!("Failed to get school profile: {}", e)]),
                )),
            )
        }
    }
}

#[put("school-profile")]
pub async fn update_school_profile(
    app_state: web::Data<AppState>,
    req: web::Json<UpdateSchoolProfileRequest>,
) -> actix_web::Result<HttpResponse> {
    // Validate request
    if let Err(errors) = req.validate() {
        let issues = flatten_validation_errors(&errors);
        return Ok(
            HttpResponse::BadRequest().json(ApiResponse::<()>::from_status(
                actix_web::http::StatusCode::BAD_REQUEST,
                None,
                Some(issues),
            )),
        );
    }

    match app_state
        .kernel_db
        .update_school_profile(req.into_inner())
        .await
    {
        Ok(profile) => Ok(HttpResponse::Ok().json(ApiResponse::from_status(
            actix_web::http::StatusCode::OK,
            Some(profile),
            None,
        ))),
        Err(e) => {
            log::error!("Failed to update school profile: {:?}", e);
            Ok(
                HttpResponse::InternalServerError().json(ApiResponse::<()>::from_status(
                    actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                    None,
                    Some(vec![format!("Failed to update school profile: {}", e)]),
                )),
            )
        }
    }
}

#[post("school-profile/logo")]
pub async fn upload_school_logo(
    app_state: web::Data<AppState>,
    mut payload: Multipart,
) -> actix_web::Result<HttpResponse> {
    let mut logo_light_url: Option<String> = None;
    let mut logo_dark_url: Option<String> = None;

    // Process multipart form data
    while let Some(item) = payload.next().await {
        let mut field = match item {
            Ok(field) => field,
            Err(e) => {
                log::error!("Failed to read multipart field: {:?}", e);
                return Ok(
                    HttpResponse::BadRequest().json(ApiResponse::<()>::from_status(
                        actix_web::http::StatusCode::BAD_REQUEST,
                        None,
                        Some(vec![format!("Invalid multipart data: {}", e)]),
                    )),
                );
            }
        };

        let content_disp = field.content_disposition();
        let field_name = content_disp
            .and_then(|cd| cd.get_name().map(|n| n.to_string()))
            .unwrap_or_else(|| "unknown".to_string());
        let filename = content_disp
            .and_then(|cd| cd.get_filename().map(|f| f.to_string()))
            .unwrap_or_else(|| "logo.png".to_string());

        // Read file data
        let mut file_data = Vec::new();
        while let Some(chunk) = field.next().await {
            let data = match chunk {
                Ok(data) => data,
                Err(e) => {
                    log::error!("Failed to read chunk: {:?}", e);
                    return Ok(
                        HttpResponse::BadRequest().json(ApiResponse::<()>::from_status(
                            actix_web::http::StatusCode::BAD_REQUEST,
                            None,
                            Some(vec![format!("Failed to read file data: {}", e)]),
                        )),
                    );
                }
            };
            file_data.extend_from_slice(&data);
        }

        // Determine file extension from content type or filename
        let extension = filename.rsplit('.').next().unwrap_or("png");

        // Generate unique filename
        let unique_filename = format!(
            "school_{}_{}.{}",
            field_name,
            uuid::Uuid::new_v4(),
            extension
        );

        // Upload to storage
        match app_state
            .storage_ops
            .upload_file(
                &unique_filename,
                &file_data,
                &format!("image/{}", extension),
            )
            .await
        {
            Ok(url) => {
                if field_name == "logo_light" {
                    logo_light_url = Some(url);
                } else if field_name == "logo_dark" {
                    logo_dark_url = Some(url);
                }
            }
            Err(e) => {
                log::error!("Failed to upload logo: {:?}", e);
                return Ok(HttpResponse::InternalServerError().json(
                    ApiResponse::<()>::from_status(
                        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                        None,
                        Some(vec![format!("Failed to upload logo: {}", e)]),
                    ),
                ));
            }
        }
    }

    // Update database with new logo URLs
    match app_state
        .kernel_db
        .update_school_logos(logo_light_url.clone(), logo_dark_url.clone())
        .await
    {
        Ok((light, dark)) => Ok(HttpResponse::Ok().json(ApiResponse::from_status(
            actix_web::http::StatusCode::OK,
            Some(LogoUploadResponse {
                logo_light_url: light,
                logo_dark_url: dark,
            }),
            None,
        ))),
        Err(e) => {
            log::error!("Failed to update logo URLs in database: {:?}", e);
            Ok(
                HttpResponse::InternalServerError().json(ApiResponse::<()>::from_status(
                    actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                    None,
                    Some(vec![format!("Failed to update logo URLs: {}", e)]),
                )),
            )
        }
    }
}

pub fn init(cfg: &mut ServiceConfig) {
    cfg.service(get_kernel_status)
        .service(setup_school)
        .service(setup_admin)
        .service(
            web::scope("")
                .wrap(RequirePermission::new("school_settings"))
                .wrap(AuthMiddleware)
                .service(get_school_profile)
                .service(update_school_profile)
                .service(upload_school_logo),
        );
}
