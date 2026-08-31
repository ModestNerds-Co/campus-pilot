//
//  campus-pilot-apis
//  test_kernel.rs
//
//  Created by Ngonidzashe Mangudya on 2025/10/02.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

#[cfg(test)]
mod tests {
    use crate::models::api_response::ApiResponse;
    use crate::services::kernel::{
        db::KernelSetupError,
        dtos::{SetupSchoolRequest, UpdateSchoolProfileRequest},
        models::SystemState,
    };
    use crate::tests::helpers::{create_test_app, create_test_app_state};
    use actix_web::{http::StatusCode, test};
    use serde_json::json;
    use uuid::Uuid;

    #[actix_web::test]
    async fn test_kernel_status() {
        let app_state = create_test_app_state().await;
        let app = create_test_app(app_state);
        let app = test::init_service(app).await;

        let req = test::TestRequest::get()
            .uri("/api/1.0/kernel/status")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body: ApiResponse<serde_json::Value> = test::read_body_json(resp).await;
        assert!(body.success);
        assert!(body.data.is_some());

        let data = body.data.unwrap();
        assert!(data.get("state").is_some());
    }

    #[actix_web::test]
    async fn test_setup_school_valid() {
        let app_state = create_test_app_state().await;
        let app = create_test_app(app_state);
        let app = test::init_service(app).await;

        let req = test::TestRequest::post()
            .uri("/api/1.0/kernel/setup-school")
            .set_json(json!({
                "name": "Test School",
                "legal_name": "Test School Ltd",
                "email": "school@test.com",
                "phone": "+1234567890",
                "address_line1": "123 School St",
                "city": "Test City",
                "province": "Test Province",
                "country": "Test Country"
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(
            resp.status() == StatusCode::OK || resp.status() == StatusCode::CONFLICT,
            "Expected OK or CONFLICT, got {}",
            resp.status()
        );
    }

    #[actix_web::test]
    async fn test_setup_school_missing_fields() {
        let app_state = create_test_app_state().await;
        let app = create_test_app(app_state);
        let app = test::init_service(app).await;

        let req = test::TestRequest::post()
            .uri("/api/1.0/kernel/setup-school")
            .set_json(json!({ "name": "" }))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_setup_admin_valid() {
        let app_state = create_test_app_state().await;
        let app = create_test_app(app_state.clone());
        let app = test::init_service(app).await;

        // Setup school first
        let setup_school = test::TestRequest::post()
            .uri("/api/1.0/kernel/setup-school")
            .set_json(json!({
                "name": "Test School",
                "legal_name": "Test School Ltd",
                "email": "admin@test.com",
                "phone": "+1234567890",
                "address_line1": "123 Test St",
                "city": "Test City",
                "province": "Test Province",
                "country": "Test Country"
            }))
            .to_request();

        test::call_service(&app, setup_school).await;

        // Now setup admin
        let req = test::TestRequest::post()
            .uri("/api/1.0/kernel/setup-admin")
            .set_json(json!({
                "full_name": "Admin User",
                "email": "admin@test.com",
                "password": "SecurePass123!",
                "phone": "+1234567890"
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(
            resp.status() == StatusCode::OK || resp.status() == StatusCode::CONFLICT,
            "Expected OK or CONFLICT, got {}",
            resp.status()
        );
    }

    #[actix_web::test]
    async fn test_setup_admin_weak_password() {
        let app_state = create_test_app_state().await;
        let app = create_test_app(app_state);
        let app = test::init_service(app).await;

        let req = test::TestRequest::post()
            .uri("/api/1.0/kernel/setup-admin")
            .set_json(json!({
                "full_name": "Admin User",
                "email": "admin@test.com",
                "password": "weak",
                "phone": "+1234567890"
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn school_profile_operations_are_isolated_between_tenants() {
        let app_state = create_test_app_state().await;
        let first_tenant = Uuid::new_v4();
        let second_tenant = Uuid::new_v4();

        for (tenant_id, slug, name) in [
            (
                first_tenant,
                format!("kernel-first-{first_tenant}"),
                "First Campus",
            ),
            (
                second_tenant,
                format!("kernel-second-{second_tenant}"),
                "Second Campus",
            ),
        ] {
            sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, $2, $3)")
                .bind(tenant_id)
                .bind(slug)
                .bind(name)
                .execute(&app_state.db)
                .await
                .unwrap_or_else(|error| panic!("tenant fixture must insert: {error}"));

            sqlx::query(
                "INSERT INTO school_profile (tenant_id, name, logo_light_url) VALUES ($1, $2, $3)",
            )
            .bind(tenant_id)
            .bind(name)
            .bind(format!("https://assets.test/{tenant_id}/original.png"))
            .execute(&app_state.db)
            .await
            .unwrap_or_else(|error| panic!("school profile fixture must insert: {error}"));
        }

        app_state
            .kernel_db
            .setup_school(SetupSchoolRequest {
                name: "Default Campus Updated".to_string(),
                legal_name: None,
                emap_code: None,
                phone: None,
                email: None,
                address_line1: None,
                address_line2: None,
                city: None,
                province: None,
                country: None,
                timezone: None,
                locale: None,
                logo_light_url: None,
                logo_dark_url: None,
            })
            .await
            .unwrap_or_else(|error| panic!("default tenant setup must upsert safely: {error}"));

        let second_after_setup = app_state
            .kernel_db
            .get_school_profile(second_tenant)
            .await
            .unwrap_or_else(|error| panic!("second tenant must survive setup unchanged: {error}"));
        assert_eq!(second_after_setup.name, "Second Campus");

        let updated_first = app_state
            .kernel_db
            .update_school_profile(
                first_tenant,
                UpdateSchoolProfileRequest {
                    name: Some("First Campus Updated".to_string()),
                    legal_name: None,
                    emap_code: None,
                    email: None,
                    phone: None,
                    address_line1: None,
                    address_line2: None,
                    city: None,
                    province: None,
                    country: None,
                    timezone: None,
                    locale: None,
                },
            )
            .await
            .unwrap_or_else(|error| panic!("first tenant update must succeed: {error}"));
        assert_eq!(updated_first.name, "First Campus Updated");

        let first_logo = format!("https://assets.test/{first_tenant}/updated.png");
        app_state
            .kernel_db
            .update_school_logos(first_tenant, Some(first_logo.clone()), None)
            .await
            .unwrap_or_else(|error| panic!("first tenant logo update must succeed: {error}"));

        let first_profile = app_state
            .kernel_db
            .get_school_profile(first_tenant)
            .await
            .unwrap_or_else(|error| panic!("first tenant profile must load: {error}"));
        let second_profile = app_state
            .kernel_db
            .get_school_profile(second_tenant)
            .await
            .unwrap_or_else(|error| panic!("second tenant profile must load: {error}"));

        assert_eq!(first_profile.name, "First Campus Updated");
        assert_eq!(
            first_profile.logo_light_url.as_deref(),
            Some(first_logo.as_str())
        );
        let second_logo = format!("https://assets.test/{second_tenant}/original.png");
        assert_eq!(second_profile.name, "Second Campus");
        assert_eq!(
            second_profile.logo_light_url.as_deref(),
            Some(second_logo.as_str())
        );

        let default_tenant = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM tenants WHERE slug = 'default' AND deleted_at IS NULL",
        )
        .fetch_one(&app_state.db)
        .await
        .unwrap_or_else(|error| panic!("default tenant fixture must exist: {error}"));
        let default_before_replay = app_state
            .kernel_db
            .get_school_profile(default_tenant)
            .await
            .unwrap_or_else(|error| panic!("default profile must exist: {error}"));
        app_state
            .kernel_db
            .update_system_state(SystemState::Ready)
            .await
            .unwrap_or_else(|error| panic!("fixture must reach Ready: {error}"));

        let replay = app_state
            .kernel_db
            .setup_school(SetupSchoolRequest {
                name: "Unauthorized replay".to_string(),
                legal_name: None,
                emap_code: None,
                phone: None,
                email: None,
                address_line1: None,
                address_line2: None,
                city: None,
                province: None,
                country: None,
                timezone: None,
                locale: None,
                logo_light_url: None,
                logo_dark_url: None,
            })
            .await;
        assert!(matches!(replay, Err(KernelSetupError::InvalidState)));

        let default_after_replay = app_state
            .kernel_db
            .get_school_profile(default_tenant)
            .await
            .unwrap_or_else(|error| panic!("default profile must survive replay: {error}"));
        assert_eq!(default_after_replay.name, default_before_replay.name);
        let status = app_state
            .kernel_db
            .get_kernel_status()
            .await
            .unwrap_or_else(|error| panic!("kernel status must remain readable: {error}"));
        assert!(matches!(status.state, SystemState::Ready));
    }
}
