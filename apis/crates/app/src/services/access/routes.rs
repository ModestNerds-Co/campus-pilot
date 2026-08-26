//
//  campus-pilot-apis
//  routes.rs
//
//  Created by OpenAI Codex on 2026/08/26.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, delete, get, put, web};
use cp_common::{RequirePermission, TenantId};
use validator::Validate;

use crate::{
    middleware::AuthMiddleware, models::api_response::ApiResponse, state::AppState,
    utils::flatten_validation_errors,
};

use super::{
    catalog::{administration_permissions, is_core_module, is_known_module, module_catalog},
    dtos::{
        ActivateLicenseRequest, ActivateLicenseResponse, ModuleCatalogResponse,
        TenantModulesResponse,
    },
    license::verify_license,
    models::TenantModuleResponse,
    ops::AccessOps,
};

#[get("/catalog")]
async fn catalog() -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::from_status(
        StatusCode::OK,
        Some(ModuleCatalogResponse {
            modules: module_catalog(),
            administration_permissions: administration_permissions(),
        }),
        None,
    ))
}

#[get("/modules")]
async fn modules(state: web::Data<AppState>, tenant: web::ReqData<TenantId>) -> HttpResponse {
    let tenant_id = tenant.into_inner().into_inner();
    match AccessOps::list_tenant_modules(&state.db, tenant_id).await {
        Ok(modules) => HttpResponse::Ok().json(ApiResponse::from_status(
            StatusCode::OK,
            Some(TenantModulesResponse {
                modules: modules
                    .into_iter()
                    .map(TenantModuleResponse::from)
                    .collect(),
            }),
            None,
        )),
        Err(error) => {
            log::error!("Failed to list tenant modules: {:?}", error);
            HttpResponse::InternalServerError().json(ApiResponse::from_status(
                StatusCode::INTERNAL_SERVER_ERROR,
                None::<()>,
                Some(vec!["Campus modules could not be loaded".to_string()]),
            ))
        }
    }
}

#[put("/licenses/activate")]
async fn activate_license(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<ActivateLicenseRequest>,
) -> HttpResponse {
    if let Err(error) = body.validate() {
        return HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(flatten_validation_errors(&error)),
        ));
    }

    let tenant_id = tenant.into_inner().into_inner();
    let verified = match verify_license(&body.license_key, tenant_id, &state.config.license) {
        Ok(verified) => verified,
        Err(error) => {
            return HttpResponse::BadRequest().json(ApiResponse::from_status(
                StatusCode::BAD_REQUEST,
                None::<()>,
                Some(vec![error.to_string()]),
            ));
        }
    };
    let claims_json = serde_json::to_value(&verified.claims).unwrap_or_default();

    if let Err(error) = AccessOps::activate_license(
        &state.db,
        tenant_id,
        &verified.fingerprint,
        &verified.claims.iss,
        verified.claims.jti.as_deref(),
        &verified.claims.modules,
        Some(verified.expires_at),
        &claims_json,
    )
    .await
    {
        log::error!("Failed to activate module license: {:?}", error);
        return HttpResponse::InternalServerError().json(ApiResponse::from_status(
            StatusCode::INTERNAL_SERVER_ERROR,
            None::<()>,
            Some(vec![
                "The license was valid but could not be activated".to_string(),
            ]),
        ));
    }

    HttpResponse::Ok().json(ApiResponse::from_status(
        StatusCode::OK,
        Some(ActivateLicenseResponse {
            activated_modules: verified.claims.modules,
            expires_at: Some(verified.expires_at),
        }),
        None,
    ))
}

#[delete("/modules/{module_key}")]
async fn disable_module(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    module_key: web::Path<String>,
) -> HttpResponse {
    let module_key = module_key.into_inner();
    if !is_known_module(&module_key) || is_core_module(&module_key) {
        return HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(vec!["This module cannot be disabled".to_string()]),
        ));
    }

    let tenant_id = tenant.into_inner().into_inner();
    match AccessOps::disable_module(&state.db, tenant_id, &module_key).await {
        Ok(true) => HttpResponse::Ok().json(ApiResponse::from_status(
            StatusCode::OK,
            Some(serde_json::json!({ "module_key": module_key, "status": "disabled" })),
            None,
        )),
        Ok(false) => HttpResponse::NotFound().json(ApiResponse::from_status(
            StatusCode::NOT_FOUND,
            None::<()>,
            Some(vec!["Module entitlement was not found".to_string()]),
        )),
        Err(error) => {
            log::error!("Failed to disable module: {:?}", error);
            HttpResponse::InternalServerError().json(ApiResponse::from_status(
                StatusCode::INTERNAL_SERVER_ERROR,
                None::<()>,
                Some(vec!["Module could not be disabled".to_string()]),
            ))
        }
    }
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/access")
            .wrap(AuthMiddleware)
            .service(catalog)
            .service(modules)
            .service(
                web::scope("")
                    .wrap(RequirePermission::new("licensing"))
                    .service(activate_license)
                    .service(disable_module),
            ),
    );
}
