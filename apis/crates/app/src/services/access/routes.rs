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
        ModuleCatalogResponse, TenantModulesResponse,
    },
    license::{
        ControlPlaneActivationResponse, ControlPlaneRenewalResponse, OfflineLeaseBundle,
        protect_installation_credential, reveal_installation_credential, verify_license,
        verify_signed_lease,
    },
    models::TenantModuleResponse,
    ops::AccessOps,
    read_model::LicensingReadModel,
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
    let response = match LicensingReadModel::load(&state.db, tenant_id, &state.config.license).await
    {
        Ok(value) => value,
        Err(error) => return internal_error("License status could not be loaded", error),
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

/// Refreshes one tenant installation for the API route and the binary-owned scheduler.
pub async fn refresh_license_inner(
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

#[cfg(test)]
mod lifecycle_tests {
    use std::{
        net::TcpListener,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicI64, Ordering},
        },
    };

    use actix_web::{App, HttpRequest, HttpResponse, HttpServer, http::header, web};
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use chrono::{Duration, Utc};
    use cp_common::PRODUCT_CATALOG_VERSION;
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
    use serde::Deserialize;
    use serde_json::json;
    use sqlx::postgres::PgPoolOptions;
    use uuid::Uuid;

    use crate::{config::Config, services::access::license::SignedLeaseClaims, state::AppState};

    use super::{
        AccessOps, connect_license_inner, import_license_inner, refresh_license_inner,
        reveal_installation_credential,
    };

    const KEY_ID: &str = "lifecycle-test-key";
    const ACTIVATION_CODE: &str = "cpact_lifecycle_test_value";
    const INSTALLATION_CREDENTIAL: &str = "cpinst_lifecycle_test_value";
    const PRIVATE_KEY: &str = "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEIL9PtNqTMRWH3/0tsQRAHSoduxipswZZSjKkMtpWweJd\n-----END PRIVATE KEY-----\n";
    const PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEAxbbyLLpJQoSoH8ia0Xw/lZTAUKtokEiy8l27VZND2zI=\n-----END PUBLIC KEY-----\n";

    struct MockControlPlane {
        tenant_id: Uuid,
        installation_id: Uuid,
        revoked: AtomicBool,
        next_sequence: AtomicI64,
    }

    #[derive(Debug, Deserialize)]
    struct ActivationBody {
        activation_code: String,
        tenant_id: Uuid,
        deployment_id: Uuid,
        name: String,
    }

    async fn activate(
        state: web::Data<MockControlPlane>,
        body: web::Json<ActivationBody>,
    ) -> HttpResponse {
        if body.activation_code != ACTIVATION_CODE
            || body.tenant_id != state.tenant_id
            || body.deployment_id.is_nil()
            || body.name.trim().is_empty()
        {
            return HttpResponse::BadRequest().json(json!({
                "error": "The activation code is invalid or expired",
                "code": "activation_invalid",
            }));
        }
        let (lease, claims) = signed_lease(
            state.tenant_id,
            state.installation_id,
            1,
            &["agent", "fleet"],
            &["agent.sessions"],
        );
        HttpResponse::Ok().json(json!({
            "installation_id": state.installation_id,
            "installation_token": INSTALLATION_CREDENTIAL,
            "lease": lease,
            "claims": claims,
        }))
    }

    async fn renew(state: web::Data<MockControlPlane>, request: HttpRequest) -> HttpResponse {
        let authorization = request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok());
        if authorization != Some("Bearer cpinst_lifecycle_test_value") {
            return HttpResponse::Unauthorized().json(json!({
                "error": "Installation authentication failed",
                "code": "installation_unauthorized",
            }));
        }
        if state.revoked.load(Ordering::SeqCst) {
            return HttpResponse::Forbidden().json(json!({
                "error": "This installation is not active",
                "code": "installation_inactive",
            }));
        }
        let sequence = state.next_sequence.fetch_add(1, Ordering::SeqCst);
        let (token, claims) = signed_lease(
            state.tenant_id,
            state.installation_id,
            sequence,
            &["agent"],
            &["agent.history"],
        );
        HttpResponse::Ok().json(json!({ "token": token, "claims": claims }))
    }

    fn signed_lease(
        tenant_id: Uuid,
        installation_id: Uuid,
        sequence: i64,
        modules: &[&str],
        features: &[&str],
    ) -> (String, SignedLeaseClaims) {
        let now = Utc::now();
        let claims = SignedLeaseClaims {
            contract_version: "cp-license/v1".to_string(),
            iss: "campus-pilot-control-plane".to_string(),
            aud: "campus-pilot".to_string(),
            sub: tenant_id.to_string(),
            installation_id: installation_id.to_string(),
            jti: Uuid::new_v4().to_string(),
            sequence,
            catalog_version: PRODUCT_CATALOG_VERSION.to_string(),
            iat: now.timestamp(),
            nbf: (now - Duration::seconds(30)).timestamp(),
            refresh_after: (now + Duration::minutes(5)).timestamp(),
            lease_expires_at: (now + Duration::minutes(10)).timestamp(),
            grace_until: (now + Duration::minutes(15)).timestamp(),
            exp: (now + Duration::minutes(15)).timestamp(),
            modules: modules.iter().map(|value| (*value).to_string()).collect(),
            features: features.iter().map(|value| (*value).to_string()).collect(),
            limits: vec![],
            min_app_version: None,
            max_app_version: None,
        };
        let header = Header {
            alg: Algorithm::EdDSA,
            kid: Some(KEY_ID.to_string()),
            ..Header::default()
        };
        let token = encode(
            &header,
            &claims,
            &EncodingKey::from_ed_pem(PRIVATE_KEY.as_bytes()).unwrap_or_else(|_| unreachable!()),
        )
        .unwrap_or_else(|_| unreachable!());
        (token, claims)
    }

    #[actix_web::test]
    #[ignore = "requires a fresh disposable LICENSE_LIFECYCLE_TEST_DATABASE_URL"]
    async fn activation_refresh_offline_replay_revocation_and_recovery_are_coherent() {
        dotenv::dotenv().ok();
        let database_url = std::env::var("LICENSE_LIFECYCLE_TEST_DATABASE_URL")
            .expect("LICENSE_LIFECYCLE_TEST_DATABASE_URL must target a fresh disposable database");
        let config = Config::from_env().expect("test configuration must load");
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(&database_url)
            .await
            .expect("disposable license lifecycle database must connect");
        let base_state = Arc::new(AppState::init(pool, config));
        base_state
            .db_ops
            .run_migrations()
            .await
            .expect("disposable license lifecycle database must migrate");
        let tenant_id = Uuid::new_v4();
        sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, $2, 'Lifecycle test')")
            .bind(tenant_id)
            .bind(format!("license-lifecycle-{tenant_id}"))
            .execute(&base_state.db)
            .await
            .unwrap_or_else(|_| unreachable!());

        let installation_id = Uuid::new_v4();
        let mock_state = web::Data::new(MockControlPlane {
            tenant_id,
            installation_id,
            revoked: AtomicBool::new(false),
            next_sequence: AtomicI64::new(2),
        });
        let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|_| unreachable!());
        let address = listener.local_addr().unwrap_or_else(|_| unreachable!());
        let server_state = mock_state.clone();
        let server = HttpServer::new(move || {
            App::new()
                .app_data(server_state.clone())
                .route("/api/v1/installations/activate", web::post().to(activate))
                .route("/api/v1/leases/renew", web::post().to(renew))
        })
        .listen(listener)
        .unwrap_or_else(|_| unreachable!())
        .run();
        let server_handle = server.handle();
        actix_web::rt::spawn(server);

        let mut config = (*base_state.config).clone();
        config.license.control_plane_url = Some(format!("http://{address}"));
        config.license.credential_key_base64 = Some(STANDARD.encode([7_u8; 32]));
        config
            .license
            .trusted_public_keys
            .insert(KEY_ID.to_string(), STANDARD.encode(PUBLIC_KEY.as_bytes()));
        config.license.installation_name = "Lifecycle test server".to_string();
        let state = Arc::new(AppState::init(base_state.db.clone(), config));

        assert!(
            connect_license_inner(&state, tenant_id, "cpact_invalid_test_value")
                .await
                .is_err()
        );
        let activated = connect_license_inner(&state, tenant_id, ACTIVATION_CODE)
            .await
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(activated.activated_modules, vec!["agent", "fleet"]);

        let installation = AccessOps::ensure_license_installation(&state.db, tenant_id)
            .await
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(installation.remote_installation_id, Some(installation_id));
        assert_eq!(installation.latest_lease_sequence, 1);
        let stored_credential = reveal_installation_credential(
            installation
                .credential_ciphertext
                .as_deref()
                .unwrap_or_else(|| unreachable!()),
            installation
                .credential_nonce
                .as_deref()
                .unwrap_or_else(|| unreachable!()),
            tenant_id,
            installation.deployment_id,
            &state.config.license,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(stored_credential, INSTALLATION_CREDENTIAL);

        let refreshed = refresh_license_inner(&state, tenant_id)
            .await
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(refreshed.activated_modules, vec!["agent"]);
        assert_eq!(
            AccessOps::ensure_license_installation(&state.db, tenant_id)
                .await
                .unwrap_or_else(|_| unreachable!())
                .latest_lease_sequence,
            2
        );

        let (offline_token, _) = signed_lease(
            tenant_id,
            installation_id,
            3,
            &["agent"],
            &["agent.offline"],
        );
        let bundle = json!({
            "format": "cp-license-bundle/v1",
            "key_id": KEY_ID,
            "lease": offline_token,
        })
        .to_string();
        let imported = import_license_inner(&state, tenant_id, &bundle)
            .await
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(imported.activated_modules, vec!["agent"]);
        assert!(
            import_license_inner(&state, tenant_id, &bundle)
                .await
                .is_err()
        );
        let lease = AccessOps::latest_license_lease(&state.db, tenant_id)
            .await
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(lease.source, "offline_import");

        mock_state.revoked.store(true, Ordering::SeqCst);
        assert!(refresh_license_inner(&state, tenant_id).await.is_err());
        assert_eq!(
            AccessOps::ensure_license_installation(&state.db, tenant_id)
                .await
                .unwrap_or_else(|_| unreachable!())
                .latest_lease_sequence,
            3
        );

        mock_state.revoked.store(false, Ordering::SeqCst);
        mock_state.next_sequence.store(4, Ordering::SeqCst);
        refresh_license_inner(&state, tenant_id)
            .await
            .unwrap_or_else(|_| unreachable!());
        let recovered = AccessOps::ensure_license_installation(&state.db, tenant_id)
            .await
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(recovered.latest_lease_sequence, 4);
        assert_eq!(recovered.status, "active");
        assert!(recovered.last_refresh_success_at.is_some());

        server_handle.stop(true).await;
        sqlx::query(
            "UPDATE license_installations SET deleted_at = NOW() WHERE tenant_id = $1 AND deleted_at IS NULL",
        )
            .bind(tenant_id)
            .execute(&state.db)
            .await
            .unwrap_or_else(|_| unreachable!());
    }
}
