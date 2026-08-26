// Copyright (c) 2025-01-02 Codecraft Solutions
// Created: 2025-01-02
// Author: AI Assistant

use actix_web::test;
use serde_json::json;
use std::sync::Arc;

use super::helpers::{create_test_app, create_test_app_state};
use crate::state::AppState;

async fn setup() -> (Arc<AppState>, String) {
    let app_state = create_test_app_state().await;

    // Setup school and admin
    let app = test::init_service(create_test_app(app_state.clone())).await;

    let school_req = test::TestRequest::post()
        .uri("/api/1.0/kernel/setup-school")
        .set_json(json!({
            "name": "Test University",
            "short_code": "TU",
            "domain": "testuniversity.edu"
        }))
        .to_request();
    test::call_service(&app, school_req).await;

    let admin_req = test::TestRequest::post()
        .uri("/api/1.0/kernel/setup-admin")
        .set_json(json!({
            "email": "admin@testuniversity.edu",
            "full_name": "Admin User",
            "password": "SecureP@ssw0rd123"
        }))
        .to_request();
    test::call_service(&app, admin_req).await;

    // Login to get token
    let login_req = test::TestRequest::post()
        .uri("/api/1.0/auth/login")
        .set_json(json!({
            "email": "admin@testuniversity.edu",
            "password": "SecureP@ssw0rd123"
        }))
        .to_request();
    let resp = test::call_service(&app, login_req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let token = body["data"]["access_token"].as_str().unwrap().to_string();

    (app_state, token)
}

#[actix_web::test]
async fn test_list_users_success() {
    let (app_state, token) = setup().await;
    let app = test::init_service(create_test_app(app_state.clone())).await;

    let req = test::TestRequest::get()
        .uri("/api/1.0/users")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["data"]["users"].is_array());
    assert!(body["data"]["total"].as_i64().unwrap() >= 1);
}

#[actix_web::test]
async fn test_list_users_unauthorized() {
    let app_state = create_test_app_state().await;
    let app = test::init_service(create_test_app(app_state.clone())).await;

    let req = test::TestRequest::get().uri("/api/1.0/users").to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 401);
}

#[actix_web::test]
async fn test_create_user_success() {
    let (app_state, token) = setup().await;
    let app = test::init_service(create_test_app(app_state.clone())).await;

    let req = test::TestRequest::post()
        .uri("/api/1.0/users")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(json!({
            "email": "newuser@testuniversity.edu",
            "full_name": "New User",
            "password": "SecureP@ssw0rd123",
            "phone": "+1234567890",
            "roles": ["Student"],
            "is_active": true
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["email"], "newuser@testuniversity.edu");
    assert_eq!(body["data"]["full_name"], "New User");
    assert!(body["data"]["is_active"].as_bool().unwrap());
}

#[actix_web::test]
async fn test_create_user_invalid_email() {
    let (app_state, token) = setup().await;
    let app = test::init_service(create_test_app(app_state.clone())).await;

    let req = test::TestRequest::post()
        .uri("/api/1.0/users")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(json!({
            "email": "invalid-email",
            "full_name": "Test User",
            "password": "SecureP@ssw0rd123",
            "roles": ["Student"]
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_client_error());
}

#[actix_web::test]
async fn test_create_user_weak_password() {
    let (app_state, token) = setup().await;
    let app = test::init_service(create_test_app(app_state.clone())).await;

    let req = test::TestRequest::post()
        .uri("/api/1.0/users")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(json!({
            "email": "user@testuniversity.edu",
            "full_name": "Test User",
            "password": "weak",
            "roles": ["Student"]
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_client_error());
}

#[actix_web::test]
async fn test_get_user_success() {
    let (app_state, token) = setup().await;
    let app = test::init_service(create_test_app(app_state.clone())).await;

    // Create a user first
    let create_req = test::TestRequest::post()
        .uri("/api/1.0/users")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(json!({
            "email": "getuser@testuniversity.edu",
            "full_name": "Get User Test",
            "password": "SecureP@ssw0rd123",
            "roles": ["Student"]
        }))
        .to_request();
    let create_resp = test::call_service(&app, create_req).await;
    let create_body: serde_json::Value = test::read_body_json(create_resp).await;
    let user_id = create_body["data"]["id"].as_str().unwrap();

    // Get the user
    let get_req = test::TestRequest::get()
        .uri(&format!("/api/1.0/users/{}", user_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let resp = test::call_service(&app, get_req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["id"], user_id);
    assert_eq!(body["data"]["email"], "getuser@testuniversity.edu");
}

#[actix_web::test]
async fn test_get_user_not_found() {
    let (app_state, token) = setup().await;
    let app = test::init_service(create_test_app(app_state.clone())).await;

    let req = test::TestRequest::get()
        .uri("/api/1.0/users/00000000-0000-0000-0000-000000000000")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 404);
}

#[actix_web::test]
async fn test_update_user_success() {
    let (app_state, token) = setup().await;
    let app = test::init_service(create_test_app(app_state.clone())).await;

    // Create a user first
    let create_req = test::TestRequest::post()
        .uri("/api/1.0/users")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(json!({
            "email": "updateuser@testuniversity.edu",
            "full_name": "Update User Test",
            "password": "SecureP@ssw0rd123",
            "roles": ["Student"]
        }))
        .to_request();
    let create_resp = test::call_service(&app, create_req).await;
    let create_body: serde_json::Value = test::read_body_json(create_resp).await;
    let user_id = create_body["data"]["id"].as_str().unwrap();

    // Update the user
    let update_req = test::TestRequest::put()
        .uri(&format!("/api/1.0/users/{}", user_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(json!({
            "full_name": "Updated Name",
            "phone": "+9876543210"
        }))
        .to_request();

    let resp = test::call_service(&app, update_req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["full_name"], "Updated Name");
    assert_eq!(body["data"]["phone"], "+9876543210");
}

#[actix_web::test]
async fn test_deactivate_user_success() {
    let (app_state, token) = setup().await;
    let app = test::init_service(create_test_app(app_state.clone())).await;

    // Create a user first
    let create_req = test::TestRequest::post()
        .uri("/api/1.0/users")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(json!({
            "email": "deactivate@testuniversity.edu",
            "full_name": "Deactivate Test",
            "password": "SecureP@ssw0rd123",
            "roles": ["Student"]
        }))
        .to_request();
    let create_resp = test::call_service(&app, create_req).await;
    let create_body: serde_json::Value = test::read_body_json(create_resp).await;
    let user_id = create_body["data"]["id"].as_str().unwrap();

    // Deactivate the user
    let deactivate_req = test::TestRequest::post()
        .uri(&format!("/api/1.0/users/{}/deactivate", user_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let resp = test::call_service(&app, deactivate_req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["is_active"], false);
}

#[actix_web::test]
async fn test_activate_user_success() {
    let (app_state, token) = setup().await;
    let app = test::init_service(create_test_app(app_state.clone())).await;

    // Create and deactivate a user first
    let create_req = test::TestRequest::post()
        .uri("/api/1.0/users")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(json!({
            "email": "activate@testuniversity.edu",
            "full_name": "Activate Test",
            "password": "SecureP@ssw0rd123",
            "roles": ["Student"],
            "is_active": false
        }))
        .to_request();
    let create_resp = test::call_service(&app, create_req).await;
    let create_body: serde_json::Value = test::read_body_json(create_resp).await;
    let user_id = create_body["data"]["id"].as_str().unwrap();

    // Activate the user
    let activate_req = test::TestRequest::post()
        .uri(&format!("/api/1.0/users/{}/activate", user_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let resp = test::call_service(&app, activate_req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["is_active"], true);
}

#[actix_web::test]
async fn test_delete_user_success() {
    let (app_state, token) = setup().await;
    let app = test::init_service(create_test_app(app_state.clone())).await;

    // Create a user first
    let create_req = test::TestRequest::post()
        .uri("/api/1.0/users")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(json!({
            "email": "delete@testuniversity.edu",
            "full_name": "Delete Test",
            "password": "SecureP@ssw0rd123",
            "roles": ["Student"]
        }))
        .to_request();
    let create_resp = test::call_service(&app, create_req).await;
    let create_body: serde_json::Value = test::read_body_json(create_resp).await;
    let user_id = create_body["data"]["id"].as_str().unwrap();

    // Delete the user
    let delete_req = test::TestRequest::delete()
        .uri(&format!("/api/1.0/users/{}", user_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let resp = test::call_service(&app, delete_req).await;
    assert!(resp.status().is_success());

    // Verify user is deleted (not found)
    let get_req = test::TestRequest::get()
        .uri(&format!("/api/1.0/users/{}", user_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let get_resp = test::call_service(&app, get_req).await;
    assert_eq!(get_resp.status().as_u16(), 404);
}
