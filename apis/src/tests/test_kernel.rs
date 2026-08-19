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
    use crate::tests::helpers::{create_test_app, create_test_app_state};
    use actix_web::{http::StatusCode, test};
    use serde_json::json;

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
            .set_json(json!({
                "name": "Test School"
            }))
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
}
