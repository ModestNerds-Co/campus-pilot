use actix_web::{HttpResponse, get, web, web::ServiceConfig};
use sqlx::PgPool;

use crate::{models::api_response::ApiResponse, services::kernel::db::KernelDbOps};

#[get("status")]
pub async fn get_kernel_status(db: web::Data<PgPool>) -> actix_web::Result<HttpResponse> {
    let kernel_db_ops = KernelDbOps::new(db.get_ref().clone());
    let status = kernel_db_ops.get_kernel_status().await.unwrap();
    let response = HttpResponse::Ok().json(ApiResponse::from_status(
        actix_web::http::StatusCode::OK,
        Some(status),
        None,
    ));
    Ok(response)
}

pub fn init(cfg: &mut ServiceConfig) {
    cfg.service(get_kernel_status);
}
