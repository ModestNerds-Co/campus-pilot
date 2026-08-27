//
//  campus-pilot-apis
//  routes.rs
//
//  Created by OpenAI Codex on 2026/08/26.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use actix_web::http::StatusCode;
use std::time::Duration;

use actix_web::{HttpResponse, delete, get, post, put, web};
use anyhow::{Context, Result, bail};
use cp_common::{RequirePermission, TenantId};
use serde::Deserialize;
use validator::Validate;

use crate::{
    middleware::AuthMiddleware, models::api_response::ApiResponse, state::AppState,
    utils::flatten_validation_errors,
};

use super::{
    catalog::{administration_permissions, is_core_module, is_known_module, module_catalog},
    dtos::{
        ActivateLicenseRequest, ActivateLicenseResponse, ConnectLicenseRequest, ImportLeaseRequest,
        LeaseStateResponse, LicensingStateResponse, ModuleCatalogResponse, TenantModulesResponse,
    },
    license::{
        ControlPlaneActivationResponse, ControlPlaneRenewalResponse, OfflineLeaseBundle,
        SignedLeaseClaims, protect_installation_credential, reveal_installation_credential,
        verify_license, verify_signed_lease,
    },
    models::{LicenseLimitResponse, TenantModuleResponse},
    ops::AccessOps,
};

#[derive(Debug, Deserialize)]
struct ControlPlaneError {
    error: Option<String>,
    code: Option<String>,
}

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

#[get("/licensing")]
async fn licensing_state(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
) -> HttpResponse {
    let tenant_id = tenant.into_inner().into_inner();
    let installation = match AccessOps::ensure_license_installation(&state.db, tenant_id).await {
        Ok(value) => value,
        Err(error) => return internal_error("License status could not be loaded", error),
    };
    let lease = match AccessOps::latest_license_lease(&state.db, tenant_id).await {
        Ok(value) => value,
        Err(error) => return internal_error("License status could not be loaded", error),
    };
    let lease = match lease.map(|value| {
        let claims = serde_json::from_value::<SignedLeaseClaims>(value.claims)
            .context("Stored license claims are invalid")?;
        Ok::<_, anyhow::Error>(LeaseStateResponse {
            id: value.lease_id.to_string(),
            status: value.status,
            source: value.source,
            catalog_version: value.catalog_version,
            issued_at: value.issued_at,
            refresh_after: value.refresh_after,
            lease_expires_at: value.lease_expires_at,
            grace_until: value.grace_until,
            modules: claims.modules,
            features: claims.features,
            limits: claims
                .limits
                .into_iter()
                .map(|limit| LicenseLimitResponse {
                    key: limit.key,
                    unit: limit.unit,
                    period: limit.period,
                    value: limit.value,
                    enforcement: limit.enforcement,
                })
                .collect(),
        })
    }) {
        Some(Ok(value)) => Some(value),
        Some(Err(error)) => return internal_error("License status could not be loaded", error),
        None => None,
    };
    let configured = state.config.license.control_plane_url.is_some()
        && state.config.license.verification_is_configured()
        && state.config.license.credential_key_base64.is_some();
    let response = LicensingStateResponse {
        configured,
        connected: installation.remote_installation_id.is_some(),
        status: installation.status,
        deployment_id: installation.deployment_id.to_string(),
        installation_id: installation
            .remote_installation_id
            .map(|value| value.to_string()),
        credential_hint: installation.credential_hint,
        portal_url: state.config.license.control_plane_url.clone(),
        latest_sequence: installation.latest_lease_sequence,
        last_refresh_attempt_at: installation.last_refresh_attempt_at,
        last_refresh_success_at: installation.last_refresh_success_at,
        last_error_code: installation.last_error_code,
        lease,
    };
    HttpResponse::Ok().json(ApiResponse::from_status(
        StatusCode::OK,
        Some(response),
        None,
    ))
}

#[put("/licensing/connect")]
async fn connect_license(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<ConnectLicenseRequest>,
) -> HttpResponse {
    if let Err(error) = body.validate() {
        return validation_error(error);
    }
    let tenant_id = tenant.into_inner().into_inner();
    match connect_license_inner(&state, tenant_id, &body.activation_code).await {
        Ok(response) => HttpResponse::Ok().json(ApiResponse::from_status(
            StatusCode::OK,
            Some(response),
            None,
        )),
        Err(error) => {
            log::warn!(
                "License activation failed for tenant {}: {:#}",
                tenant_id,
                error
            );
            let _ = AccessOps::note_license_error(&state.db, tenant_id, "activation_failed").await;
            HttpResponse::BadRequest().json(ApiResponse::from_status(
                StatusCode::BAD_REQUEST,
                None::<()>,
                Some(vec!["The activation code could not be used".to_string()]),
            ))
        }
    }
}

#[post("/licensing/refresh")]
async fn refresh_license(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
) -> HttpResponse {
    let tenant_id = tenant.into_inner().into_inner();
    match refresh_license_inner(&state, tenant_id).await {
        Ok(response) => HttpResponse::Ok().json(ApiResponse::from_status(
            StatusCode::OK,
            Some(response),
            None,
        )),
        Err(error) => {
            log::warn!(
                "License refresh failed for tenant {}: {:#}",
                tenant_id,
                error
            );
            let _ = AccessOps::note_license_error(&state.db, tenant_id, "refresh_failed").await;
            HttpResponse::BadGateway().json(ApiResponse::from_status(
                StatusCode::BAD_GATEWAY,
                None::<()>,
                Some(vec!["The license could not be refreshed".to_string()]),
            ))
        }
    }
}

#[post("/licensing/import")]
async fn import_license(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<ImportLeaseRequest>,
) -> HttpResponse {
    if let Err(error) = body.validate() {
        return validation_error(error);
    }
    let tenant_id = tenant.into_inner().into_inner();
    match import_license_inner(&state, tenant_id, &body.bundle).await {
        Ok(response) => HttpResponse::Ok().json(ApiResponse::from_status(
            StatusCode::OK,
            Some(response),
            None,
        )),
        Err(error) => {
            log::warn!(
                "Offline license import failed for tenant {}: {:#}",
                tenant_id,
                error
            );
            HttpResponse::BadRequest().json(ApiResponse::from_status(
                StatusCode::BAD_REQUEST,
                None::<()>,
                Some(vec!["The license file could not be imported".to_string()]),
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
                    .service(licensing_state)
                    .service(activate_license)
                    .service(connect_license)
                    .service(refresh_license)
                    .service(import_license)
                    .service(disable_module),
            ),
    );
}

async fn connect_license_inner(
    state: &AppState,
    tenant_id: uuid::Uuid,
    activation_code: &str,
) -> Result<ActivateLicenseResponse> {
    let installation = AccessOps::ensure_license_installation(&state.db, tenant_id).await?;
    AccessOps::note_refresh_attempt(&state.db, tenant_id).await?;
    let control_plane_url = state
        .config
        .license
        .control_plane_url
        .as_deref()
        .context("License control plane is not configured")?;
    let url = format!("{control_plane_url}/api/v1/installations/activate");
    let client = license_http_client()?;
    let response = client
        .post(url)
        .json(&serde_json::json!({
            "activation_code": activation_code.trim(),
            "tenant_id": tenant_id,
            "deployment_id": installation.deployment_id,
            "name": state.config.license.installation_name,
        }))
        .send()
        .await
        .context("License control plane could not be reached")?;
    let output = parse_control_plane_response::<ControlPlaneActivationResponse>(response).await?;
    let remote_id = uuid::Uuid::parse_str(&output.installation_id)
        .context("Control plane returned an invalid installation identifier")?;
    let verified = verify_signed_lease(
        &output.lease,
        tenant_id,
        Some(remote_id),
        &state.config.license,
    )?;
    if verified.claims != output.claims {
        bail!("Control plane response claims do not match the signed lease");
    }
    let credential = protect_installation_credential(
        &output.installation_token,
        tenant_id,
        installation.deployment_id,
        &state.config.license,
    )?;
    AccessOps::apply_signed_lease(
        &state.db,
        tenant_id,
        remote_id,
        Some(control_plane_url),
        Some(&credential),
        &verified,
        "online_activation",
    )
    .await?;
    Ok(ActivateLicenseResponse {
        activated_modules: verified.claims.modules,
        expires_at: Some(verified.grace_until),
    })
}

pub(crate) async fn refresh_license_inner(
    state: &AppState,
    tenant_id: uuid::Uuid,
) -> Result<ActivateLicenseResponse> {
    let installation = AccessOps::ensure_license_installation(&state.db, tenant_id).await?;
    AccessOps::note_refresh_attempt(&state.db, tenant_id).await?;
    let remote_id = installation
        .remote_installation_id
        .context("This Campus Pilot server is not connected")?;
    let ciphertext = installation
        .credential_ciphertext
        .as_deref()
        .context("The installation credential is missing")?;
    let nonce = installation
        .credential_nonce
        .as_deref()
        .context("The installation credential nonce is missing")?;
    let credential = reveal_installation_credential(
        ciphertext,
        nonce,
        tenant_id,
        installation.deployment_id,
        &state.config.license,
    )?;
    let control_plane_url = installation
        .control_plane_url
        .as_deref()
        .or(state.config.license.control_plane_url.as_deref())
        .context("License control plane is not configured")?;
    let response = license_http_client()?
        .post(format!("{control_plane_url}/api/v1/leases/renew"))
        .bearer_auth(&credential)
        .send()
        .await
        .context("License control plane could not be reached")?;
    let output = parse_control_plane_response::<ControlPlaneRenewalResponse>(response).await?;
    let verified = verify_signed_lease(
        &output.token,
        tenant_id,
        Some(remote_id),
        &state.config.license,
    )?;
    if verified.claims != output.claims {
        bail!("Control plane response claims do not match the signed lease");
    }
    AccessOps::apply_signed_lease(
        &state.db,
        tenant_id,
        remote_id,
        Some(control_plane_url),
        None,
        &verified,
        "online_refresh",
    )
    .await?;
    Ok(ActivateLicenseResponse {
        activated_modules: verified.claims.modules,
        expires_at: Some(verified.grace_until),
    })
}

async fn import_license_inner(
    state: &AppState,
    tenant_id: uuid::Uuid,
    bundle: &str,
) -> Result<ActivateLicenseResponse> {
    let installation = AccessOps::ensure_license_installation(&state.db, tenant_id).await?;
    let remote_id = installation
        .remote_installation_id
        .context("Connect this Campus Pilot server before importing an offline license")?;
    let bundle = serde_json::from_str::<OfflineLeaseBundle>(bundle.trim())
        .context("Offline license bundle is invalid")?;
    if bundle.format != "cp-license-bundle/v1" {
        bail!("Offline license bundle version is not supported");
    }
    let verified = verify_signed_lease(
        &bundle.lease,
        tenant_id,
        Some(remote_id),
        &state.config.license,
    )?;
    if bundle.key_id != verified.key_id {
        bail!("Offline license signing key does not match the lease");
    }
    AccessOps::apply_signed_lease(
        &state.db,
        tenant_id,
        remote_id,
        installation.control_plane_url.as_deref(),
        None,
        &verified,
        "offline_import",
    )
    .await?;
    Ok(ActivateLicenseResponse {
        activated_modules: verified.claims.modules,
        expires_at: Some(verified.grace_until),
    })
}

fn license_http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .context("License HTTP client could not be created")
}

async fn parse_control_plane_response<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T> {
    let status = response.status();
    if status.is_success() {
        return response
            .json::<T>()
            .await
            .context("License control plane returned an invalid response");
    }
    let body = response.json::<ControlPlaneError>().await.ok();
    let code = body
        .as_ref()
        .and_then(|value| value.code.as_deref())
        .unwrap_or("control_plane_error");
    let message = body
        .as_ref()
        .and_then(|value| value.error.as_deref())
        .unwrap_or("License control plane rejected the request");
    bail!("{message} ({code}, HTTP {})", status.as_u16())
}

fn validation_error(error: validator::ValidationErrors) -> HttpResponse {
    HttpResponse::BadRequest().json(ApiResponse::from_status(
        StatusCode::BAD_REQUEST,
        None::<()>,
        Some(flatten_validation_errors(&error)),
    ))
}

fn internal_error(message: &str, error: anyhow::Error) -> HttpResponse {
    log::error!("{}: {:?}", message, error);
    HttpResponse::InternalServerError().json(ApiResponse::from_status(
        StatusCode::INTERNAL_SERVER_ERROR,
        None::<()>,
        Some(vec![message.to_string()]),
    ))
}

#[cfg(test)]
mod authority_tests {
    use actix_web::{http::StatusCode, test as actix_test};
    use uuid::Uuid;

    use crate::{
        tests::helpers::{create_test_app, create_test_app_state},
        utils::generate_access_token,
    };

    async fn token_for(role_key: &str) -> String {
        let state = create_test_app_state().await;
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let email = format!("authority-{role_key}-{user_id}@example.test");
        sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, $2, $3)")
            .bind(tenant_id)
            .bind(format!("authority-{tenant_id}"))
            .bind("Authority test")
            .execute(&state.db)
            .await
            .expect("failed to create authority test tenant");
        sqlx::query(
            r#"
            INSERT INTO users (
                id, tenant_id, email, full_name, password_hash, roles, is_active
            )
            VALUES ($1, $2, $3, 'Authority test', 'not-used', $4, TRUE)
            "#,
        )
        .bind(user_id)
        .bind(tenant_id)
        .bind(&email)
        .bind(vec![role_key.to_string()])
        .execute(&state.db)
        .await
        .expect("failed to create authority test user");
        generate_access_token(
            user_id,
            tenant_id,
            &email,
            vec![role_key.to_string()],
            &state.config.jwt.secret,
        )
        .unwrap_or_else(|_| unreachable!())
    }

    #[actix_web::test]
    async fn launcher_discovery_stays_shared_while_admin_reads_require_permission() {
        let state = create_test_app_state().await;
        let teacher_token = token_for("teacher").await;
        let app = actix_test::init_service(create_test_app(state.clone())).await;

        for path in ["/api/1.0/access/catalog", "/api/1.0/access/modules"] {
            let request = actix_test::TestRequest::get()
                .uri(path)
                .insert_header(("Authorization", format!("Bearer {teacher_token}")))
                .to_request();
            let response = actix_test::call_service(&app, request).await;
            assert_eq!(response.status(), StatusCode::OK, "{path}");
        }

        for path in [
            "/api/1.0/access/licensing",
            "/api/1.0/kernel/school-profile",
        ] {
            let request = actix_test::TestRequest::get()
                .uri(path)
                .insert_header(("Authorization", format!("Bearer {teacher_token}")))
                .to_request();
            let response = actix_test::call_service(&app, request).await;
            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{path}");
        }

        assert!(
            state.db.num_idle() > 0,
            "route test pool exhausted before administrator check: size={}, idle={}",
            state.db.size(),
            state.db.num_idle()
        );
        let admin_token = token_for("school_administrator").await;
        let request = actix_test::TestRequest::get()
            .uri("/api/1.0/access/licensing")
            .insert_header(("Authorization", format!("Bearer {admin_token}")))
            .to_request();
        let response = actix_test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);

        let request = actix_test::TestRequest::get()
            .uri("/api/1.0/access/catalog")
            .to_request();
        let response = actix_test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
