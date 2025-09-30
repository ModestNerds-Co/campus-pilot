use actix_web::{HttpResponse, get, post, web, web::ServiceConfig};
use validator::Validate;

use crate::{
    models::api_response::ApiResponse, services::kernel::dtos::SetupSchoolRequest, state::AppState,
    utils::flatten_validation_errors,
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

pub fn init(cfg: &mut ServiceConfig) {
    cfg.service(get_kernel_status).service(setup_school);
}
