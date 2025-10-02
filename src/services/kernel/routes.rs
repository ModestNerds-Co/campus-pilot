use actix_web::{HttpResponse, get, post, web, web::ServiceConfig};
use serde_json::json;
use validator::Validate;

use crate::{
    models::api_response::ApiResponse,
    services::kernel::dtos::{CreateAdminReq, SetupSchoolRequest},
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

pub fn init(cfg: &mut ServiceConfig) {
    cfg.service(get_kernel_status)
        .service(setup_school)
        .service(setup_admin);
}
