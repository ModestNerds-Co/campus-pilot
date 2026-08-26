//
//  campus-pilot-apis
//  test_auth.rs
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
    async fn test_login_success() {
        let app_state = create_test_app_state().await;
        let app = create_test_app(app_state.clone());
        let app = test::init_service(app).await;

        // First, setup school and admin
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

        let setup_admin = test::TestRequest::post()
            .uri("/api/1.0/kernel/setup-admin")
            .set_json(json!({
                "full_name": "Admin User",
                "email": "admin@test.com",
                "password": "SecurePass123!",
                "phone": "+1234567890"
            }))
            .to_request();

        test::call_service(&app, setup_admin).await;

        // Now test login
        let req = test::TestRequest::post()
            .uri("/api/1.0/auth/login")
            .set_json(json!({
                "email": "admin@test.com",
                "password": "SecurePass123!"
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body: ApiResponse<serde_json::Value> = test::read_body_json(resp).await;
        assert!(body.success);
        assert!(body.data.is_some());

        let data = body.data.unwrap();
        assert!(data.get("access_token").is_some());
        assert!(data.get("refresh_token").is_some());
        assert!(data.get("user").is_some());
    }

    #[actix_web::test]
    async fn test_login_invalid_credentials() {
        let app_state = create_test_app_state().await;
        let app = create_test_app(app_state);
        let app = test::init_service(app).await;

        let req = test::TestRequest::post()
            .uri("/api/1.0/auth/login")
            .set_json(json!({
                "email": "nonexistent@test.com",
                "password": "WrongPassword123!"
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn test_login_missing_fields() {
        let app_state = create_test_app_state().await;
        let app = create_test_app(app_state);
        let app = test::init_service(app).await;

        let req = test::TestRequest::post()
            .uri("/api/1.0/auth/login")
            .set_json(json!({
                "email": "test@test.com"
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_me_without_token() {
        let app_state = create_test_app_state().await;
        let app = create_test_app(app_state);
        let app = test::init_service(app).await;

        let req = test::TestRequest::get()
            .uri("/api/1.0/auth/me")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn test_logout() {
        let app_state = create_test_app_state().await;
        let app = create_test_app(app_state);
        let app = test::init_service(app).await;

        let req = test::TestRequest::post()
            .uri("/api/1.0/auth/logout")
            .set_json(json!({
                "refresh_token": "some-token"
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
