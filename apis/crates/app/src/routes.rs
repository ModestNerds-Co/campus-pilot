//
//  campus-pilot-apis
//  routes.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use crate::middleware::AuthMiddleware;
use crate::models::api_response::ApiResponse;
use crate::services::{access, ai_providers, ai_routing, auth, kernel, roles, storage, users};
use actix_web::http::StatusCode;
use actix_web::web::{ServiceConfig, scope};
use actix_web::{HttpResponse, Responder, get};
use cp_common::RequirePermission;
use serde_json::json;
use sqlx::types::chrono::Utc;

#[get("/health-check")]
async fn health_check() -> impl Responder {
    let health_data = json!({
        "status": "healthy",
        "service": "campus-pilot",
        "version": "1.0.0",
        "timestamp": Utc::now().to_rfc3339()
    });

    let response = ApiResponse::from_status(StatusCode::OK, Some(health_data), None);
    HttpResponse::Ok().json(response)
}

pub fn init(cfg: &mut ServiceConfig) {
    cfg.service(
        scope("/api/1.0")
            .service(health_check)
            .configure(auth::routes)
            .configure(access::routes::routes)
            .configure(roles::routes::routes)
            .configure(users::routes::routes)
            .service(
                scope("/ai")
                    .wrap(RequirePermission::new("ai_providers"))
                    .wrap(AuthMiddleware)
                    .configure(ai_providers::routes::routes)
                    .configure(ai_routing::routes::routes),
            )
            .service(scope("/kernel").configure(kernel::routes::init))
            .service(scope("/storage").configure(storage::routes::init))
            // Operational module scopes mount the shared identity middleware;
            // exact licensing and permission checks run inside each module.
            .service(
                scope("/fleet")
                    .wrap(AuthMiddleware)
                    .configure(cp_fleet::routes::routes),
            )
            .service(
                scope("/vehicle-logs")
                    .wrap(AuthMiddleware)
                    .configure(cp_vehicle_log::routes::routes),
            )
            .service(
                scope("/sis")
                    .wrap(AuthMiddleware)
                    .configure(cp_sis::routes::routes),
            )
            .service(
                scope("/academics")
                    .wrap(AuthMiddleware)
                    .configure(cp_academics::routes::routes),
            )
            .service(
                scope("/timetabling")
                    .wrap(AuthMiddleware)
                    .configure(cp_timetabling::routes::routes),
            )
            .service(
                scope("/finance")
                    .wrap(AuthMiddleware)
                    .configure(cp_finance::routes::routes),
            )
            .service(
                scope("/fees")
                    .wrap(AuthMiddleware)
                    .configure(cp_fees::routes::routes),
            )
            .service(
                scope("/hr-payroll")
                    .wrap(AuthMiddleware)
                    .configure(cp_hr_payroll::routes::routes),
            )
            .service(
                scope("/procurement")
                    .wrap(AuthMiddleware)
                    .configure(cp_procurement::routes::routes),
            )
            .service(
                scope("/assets-inventory")
                    .wrap(AuthMiddleware)
                    .configure(cp_assets_inventory::routes::routes),
            )
            .service(scope("/library").configure(cp_library::routes::routes))
            .service(scope("/messaging").configure(cp_messaging::routes::routes))
            .service(scope("/hostel").configure(cp_hostel::routes::routes))
            .service(scope("/health-services").configure(cp_health::routes::routes)),
    );
}

#[cfg(test)]
mod route_wiring_tests {
    use actix_web::{
        App,
        dev::Service as _,
        http::{
            Method, StatusCode,
            header::{HeaderName, HeaderValue},
        },
        test as actix_test,
    };
    use cp_common::routed_operation_for_route;

    use super::init;

    const PATTERN_HEADER: &str = "x-test-route-pattern";
    const OPERATION_HEADER: &str = "x-test-operation-key";

    #[actix_web::test]
    async fn released_route_trees_resolve_concrete_paths_to_catalog_operations() {
        let app = actix_test::init_service(
            App::new()
                .wrap_fn(|request, service| {
                    let method = request.method().clone();
                    let pattern = request
                        .match_pattern()
                        .unwrap_or_else(|| "<unmatched>".to_string());
                    let operation_key = routed_operation_for_route(&method, &pattern)
                        .map(|route| route.operation().key().to_string())
                        .unwrap_or_else(|| "<uncatalogued>".to_string());
                    let response = service.call(request);

                    async move {
                        let mut response = response.await?;
                        response.headers_mut().insert(
                            HeaderName::from_static(PATTERN_HEADER),
                            HeaderValue::from_str(&pattern).unwrap_or_else(|error| {
                                panic!("invalid matched route-pattern header: {error}")
                            }),
                        );
                        response.headers_mut().insert(
                            HeaderName::from_static(OPERATION_HEADER),
                            HeaderValue::from_str(&operation_key).unwrap_or_else(|error| {
                                panic!("invalid operation-key header: {error}")
                            }),
                        );
                        Ok(response)
                    }
                })
                .configure(init),
        )
        .await;

        let record_id = "00000000-0000-0000-0000-000000000001";
        let route_cases = [
            (
                Method::GET,
                "/api/1.0/ai/providers".to_string(),
                "/api/1.0/ai/providers",
                "administration.ai_providers.catalog.list",
            ),
            (
                Method::GET,
                "/api/1.0/ai/connections".to_string(),
                "/api/1.0/ai/connections",
                "administration.ai_providers.connections.list",
            ),
            (
                Method::POST,
                "/api/1.0/ai/connections".to_string(),
                "/api/1.0/ai/connections",
                "administration.ai_providers.connections.create",
            ),
            (
                Method::GET,
                format!("/api/1.0/ai/connections/{record_id}"),
                "/api/1.0/ai/connections/{connection_id}",
                "administration.ai_providers.connections.read",
            ),
            (
                Method::PUT,
                format!("/api/1.0/ai/connections/{record_id}"),
                "/api/1.0/ai/connections/{connection_id}",
                "administration.ai_providers.connections.update",
            ),
            (
                Method::POST,
                format!("/api/1.0/ai/connections/{record_id}/credentials/rotate"),
                "/api/1.0/ai/connections/{connection_id}/credentials/rotate",
                "administration.ai_providers.credentials.rotate",
            ),
            (
                Method::POST,
                format!("/api/1.0/ai/connections/{record_id}/test"),
                "/api/1.0/ai/connections/{connection_id}/test",
                "administration.ai_providers.connections.test",
            ),
            (
                Method::GET,
                format!("/api/1.0/ai/connections/{record_id}/models"),
                "/api/1.0/ai/connections/{connection_id}/models",
                "administration.ai_providers.models.list",
            ),
            (
                Method::POST,
                format!("/api/1.0/ai/connections/{record_id}/models/refresh"),
                "/api/1.0/ai/connections/{connection_id}/models/refresh",
                "administration.ai_providers.models.refresh",
            ),
            (
                Method::DELETE,
                format!("/api/1.0/ai/connections/{record_id}"),
                "/api/1.0/ai/connections/{connection_id}",
                "administration.ai_providers.connections.disconnect",
            ),
            (
                Method::GET,
                "/api/1.0/ai/routes".to_string(),
                "/api/1.0/ai/routes",
                "administration.ai_routing.routes.list",
            ),
            (
                Method::GET,
                "/api/1.0/ai/routes/options".to_string(),
                "/api/1.0/ai/routes/options",
                "administration.ai_routing.routes.options",
            ),
            (
                Method::POST,
                "/api/1.0/ai/routes/resolve".to_string(),
                "/api/1.0/ai/routes/resolve",
                "administration.ai_routing.routes.resolve",
            ),
            (
                Method::POST,
                "/api/1.0/ai/routes".to_string(),
                "/api/1.0/ai/routes",
                "administration.ai_routing.routes.create",
            ),
            (
                Method::GET,
                format!("/api/1.0/ai/routes/{record_id}"),
                "/api/1.0/ai/routes/{route_set_id}",
                "administration.ai_routing.routes.read",
            ),
            (
                Method::PUT,
                format!("/api/1.0/ai/routes/{record_id}"),
                "/api/1.0/ai/routes/{route_set_id}",
                "administration.ai_routing.routes.update",
            ),
            (
                Method::DELETE,
                format!("/api/1.0/ai/routes/{record_id}"),
                "/api/1.0/ai/routes/{route_set_id}",
                "administration.ai_routing.routes.archive",
            ),
            (
                Method::GET,
                "/api/1.0/sis/learner-numbering".to_string(),
                "/api/1.0/sis/learner-numbering",
                "sis.learner_numbering.read",
            ),
            (
                Method::PUT,
                "/api/1.0/sis/learner-numbering".to_string(),
                "/api/1.0/sis/learner-numbering",
                "sis.learner_numbering.update",
            ),
            (
                Method::GET,
                "/api/1.0/procurement/reference-data".to_string(),
                "/api/1.0/procurement/reference-data",
                "procurement.reference_data.read",
            ),
            (
                Method::GET,
                "/api/1.0/procurement/requester-candidates".to_string(),
                "/api/1.0/procurement/requester-candidates",
                "procurement.requester_candidates.list",
            ),
            (
                Method::GET,
                "/api/1.0/procurement/suppliers".to_string(),
                "/api/1.0/procurement/suppliers",
                "procurement.suppliers.list",
            ),
            (
                Method::GET,
                format!("/api/1.0/procurement/suppliers/{record_id}"),
                "/api/1.0/procurement/suppliers/{id}",
                "procurement.suppliers.read",
            ),
            (
                Method::POST,
                "/api/1.0/procurement/suppliers".to_string(),
                "/api/1.0/procurement/suppliers",
                "procurement.suppliers.create",
            ),
            (
                Method::PUT,
                format!("/api/1.0/procurement/suppliers/{record_id}"),
                "/api/1.0/procurement/suppliers/{id}",
                "procurement.suppliers.update",
            ),
            (
                Method::DELETE,
                format!("/api/1.0/procurement/suppliers/{record_id}?expected_version=1"),
                "/api/1.0/procurement/suppliers/{id}",
                "procurement.suppliers.delete",
            ),
            (
                Method::GET,
                "/api/1.0/procurement/requisitions".to_string(),
                "/api/1.0/procurement/requisitions",
                "procurement.requisitions.list",
            ),
            (
                Method::GET,
                format!("/api/1.0/procurement/requisitions/{record_id}"),
                "/api/1.0/procurement/requisitions/{id}",
                "procurement.requisitions.read",
            ),
            (
                Method::POST,
                "/api/1.0/procurement/requisitions".to_string(),
                "/api/1.0/procurement/requisitions",
                "procurement.requisitions.create",
            ),
            (
                Method::PUT,
                format!("/api/1.0/procurement/requisitions/{record_id}"),
                "/api/1.0/procurement/requisitions/{id}",
                "procurement.requisitions.update",
            ),
            (
                Method::DELETE,
                format!("/api/1.0/procurement/requisitions/{record_id}?expected_version=1"),
                "/api/1.0/procurement/requisitions/{id}",
                "procurement.requisitions.delete",
            ),
            (
                Method::POST,
                format!("/api/1.0/procurement/requisitions/{record_id}/submit"),
                "/api/1.0/procurement/requisitions/{id}/submit",
                "procurement.requisitions.submit",
            ),
            (
                Method::POST,
                format!("/api/1.0/procurement/requisitions/{record_id}/approve"),
                "/api/1.0/procurement/requisitions/{id}/approve",
                "procurement.requisitions.approve",
            ),
            (
                Method::POST,
                format!("/api/1.0/procurement/requisitions/{record_id}/reject"),
                "/api/1.0/procurement/requisitions/{id}/reject",
                "procurement.requisitions.reject",
            ),
            (
                Method::POST,
                format!("/api/1.0/procurement/requisitions/{record_id}/cancel"),
                "/api/1.0/procurement/requisitions/{id}/cancel",
                "procurement.requisitions.cancel",
            ),
            (
                Method::GET,
                "/api/1.0/procurement/purchase-orders".to_string(),
                "/api/1.0/procurement/purchase-orders",
                "procurement.purchase_orders.list",
            ),
            (
                Method::GET,
                format!("/api/1.0/procurement/purchase-orders/{record_id}"),
                "/api/1.0/procurement/purchase-orders/{id}",
                "procurement.purchase_orders.read",
            ),
            (
                Method::POST,
                "/api/1.0/procurement/purchase-orders".to_string(),
                "/api/1.0/procurement/purchase-orders",
                "procurement.purchase_orders.create",
            ),
            (
                Method::PUT,
                format!("/api/1.0/procurement/purchase-orders/{record_id}"),
                "/api/1.0/procurement/purchase-orders/{id}",
                "procurement.purchase_orders.update",
            ),
            (
                Method::POST,
                format!("/api/1.0/procurement/purchase-orders/{record_id}/issue"),
                "/api/1.0/procurement/purchase-orders/{id}/issue",
                "procurement.purchase_orders.issue",
            ),
            (
                Method::POST,
                format!("/api/1.0/procurement/purchase-orders/{record_id}/cancel"),
                "/api/1.0/procurement/purchase-orders/{id}/cancel",
                "procurement.purchase_orders.cancel",
            ),
            (
                Method::GET,
                "/api/1.0/procurement/goods-receipts".to_string(),
                "/api/1.0/procurement/goods-receipts",
                "procurement.goods_receipts.list",
            ),
            (
                Method::GET,
                format!("/api/1.0/procurement/goods-receipts/{record_id}"),
                "/api/1.0/procurement/goods-receipts/{id}",
                "procurement.goods_receipts.read",
            ),
            (
                Method::POST,
                "/api/1.0/procurement/goods-receipts".to_string(),
                "/api/1.0/procurement/goods-receipts",
                "procurement.goods_receipts.create",
            ),
            (
                Method::PUT,
                format!("/api/1.0/procurement/goods-receipts/{record_id}"),
                "/api/1.0/procurement/goods-receipts/{id}",
                "procurement.goods_receipts.update",
            ),
            (
                Method::POST,
                format!("/api/1.0/procurement/goods-receipts/{record_id}/post"),
                "/api/1.0/procurement/goods-receipts/{id}/post",
                "procurement.goods_receipts.post",
            ),
            (
                Method::GET,
                "/api/1.0/assets-inventory/items".to_string(),
                "/api/1.0/assets-inventory/items",
                "assets_inventory.items.list",
            ),
            (
                Method::GET,
                format!("/api/1.0/assets-inventory/items/{record_id}"),
                "/api/1.0/assets-inventory/items/{id}",
                "assets_inventory.items.read",
            ),
            (
                Method::POST,
                "/api/1.0/assets-inventory/items".to_string(),
                "/api/1.0/assets-inventory/items",
                "assets_inventory.items.create",
            ),
            (
                Method::PUT,
                format!("/api/1.0/assets-inventory/items/{record_id}"),
                "/api/1.0/assets-inventory/items/{id}",
                "assets_inventory.items.update",
            ),
            (
                Method::DELETE,
                format!("/api/1.0/assets-inventory/items/{record_id}?expected_version=1"),
                "/api/1.0/assets-inventory/items/{id}",
                "assets_inventory.items.delete",
            ),
            (
                Method::GET,
                "/api/1.0/assets-inventory/stores".to_string(),
                "/api/1.0/assets-inventory/stores",
                "assets_inventory.stores.list",
            ),
            (
                Method::GET,
                format!("/api/1.0/assets-inventory/stores/{record_id}"),
                "/api/1.0/assets-inventory/stores/{id}",
                "assets_inventory.stores.read",
            ),
            (
                Method::POST,
                "/api/1.0/assets-inventory/stores".to_string(),
                "/api/1.0/assets-inventory/stores",
                "assets_inventory.stores.create",
            ),
            (
                Method::PUT,
                format!("/api/1.0/assets-inventory/stores/{record_id}"),
                "/api/1.0/assets-inventory/stores/{id}",
                "assets_inventory.stores.update",
            ),
            (
                Method::DELETE,
                format!("/api/1.0/assets-inventory/stores/{record_id}?expected_version=1"),
                "/api/1.0/assets-inventory/stores/{id}",
                "assets_inventory.stores.delete",
            ),
            (
                Method::GET,
                "/api/1.0/assets-inventory/stock-balances".to_string(),
                "/api/1.0/assets-inventory/stock-balances",
                "assets_inventory.stock_balances.list",
            ),
            (
                Method::GET,
                "/api/1.0/assets-inventory/stock-movements".to_string(),
                "/api/1.0/assets-inventory/stock-movements",
                "assets_inventory.stock_movements.list",
            ),
            (
                Method::GET,
                format!("/api/1.0/assets-inventory/stock-movements/{record_id}"),
                "/api/1.0/assets-inventory/stock-movements/{id}",
                "assets_inventory.stock_movements.read",
            ),
            (
                Method::POST,
                "/api/1.0/assets-inventory/manual-receipts".to_string(),
                "/api/1.0/assets-inventory/manual-receipts",
                "assets_inventory.manual_receipts.create",
            ),
            (
                Method::POST,
                "/api/1.0/assets-inventory/issues".to_string(),
                "/api/1.0/assets-inventory/issues",
                "assets_inventory.issues.create",
            ),
            (
                Method::POST,
                "/api/1.0/assets-inventory/transfers".to_string(),
                "/api/1.0/assets-inventory/transfers",
                "assets_inventory.transfers.create",
            ),
            (
                Method::POST,
                "/api/1.0/assets-inventory/adjustments".to_string(),
                "/api/1.0/assets-inventory/adjustments",
                "assets_inventory.adjustments.create",
            ),
            (
                Method::POST,
                format!("/api/1.0/assets-inventory/stock-movements/{record_id}/reverse"),
                "/api/1.0/assets-inventory/stock-movements/{id}/reverse",
                "assets_inventory.stock_movements.reverse",
            ),
            (
                Method::GET,
                "/api/1.0/assets-inventory/goods-receipt-allocations".to_string(),
                "/api/1.0/assets-inventory/goods-receipt-allocations",
                "assets_inventory.goods_receipt_allocations.list",
            ),
            (
                Method::POST,
                "/api/1.0/assets-inventory/goods-receipt-allocations".to_string(),
                "/api/1.0/assets-inventory/goods-receipt-allocations",
                "assets_inventory.goods_receipt_allocations.create",
            ),
        ];

        for (method, concrete_path, expected_pattern, expected_operation) in route_cases {
            let request = actix_test::TestRequest::default()
                .method(method)
                .uri(&concrete_path)
                .to_request();
            let response = actix_test::call_service(&app, request).await;

            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "protected route did not reach authentication: {concrete_path}",
            );
            assert_eq!(
                response
                    .headers()
                    .get(PATTERN_HEADER)
                    .and_then(|value| value.to_str().ok()),
                Some(expected_pattern),
                "resolved route pattern drifted: {concrete_path}",
            );
            assert_eq!(
                response
                    .headers()
                    .get(OPERATION_HEADER)
                    .and_then(|value| value.to_str().ok()),
                Some(expected_operation),
                "resolved operation drifted: {concrete_path}",
            );
        }
    }
}
