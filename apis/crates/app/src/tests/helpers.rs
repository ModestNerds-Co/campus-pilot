//
//  campus-pilot-apis
//  helpers.rs
//
//  Created by Ngonidzashe Mangudya on 2025/10/02.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use crate::config::Config;
use crate::state::AppState;
use actix_web::{App, web};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::sync::OnceCell;

static TEST_APP_STATE: OnceCell<Arc<AppState>> = OnceCell::const_new();

/// Create a test app state with a test database (singleton to avoid migration conflicts)
pub async fn create_test_app_state() -> Arc<AppState> {
    TEST_APP_STATE
        .get_or_init(|| async {
            dotenv::dotenv().ok();

            let config = Config::from_env().expect("Failed to load config");

            // Use a test database or the regular database
            let db_pool = PgPoolOptions::new()
                .max_connections(10)
                .connect(&config.database.url)
                .await
                .expect("Failed to connect to database");

            let app_state = Arc::new(AppState::init(db_pool, config));

            // Run migrations once
            app_state
                .db_ops
                .run_migrations()
                .await
                .expect("Failed to run migrations");

            app_state
        })
        .await
        .clone()
}

/// Create a test app with all routes configured
pub fn create_test_app(
    app_state: Arc<AppState>,
) -> App<
    impl actix_web::dev::ServiceFactory<
        actix_web::dev::ServiceRequest,
        Config = (),
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
        InitError = (),
    >,
> {
    let pool = app_state.db.clone();
    App::new()
        .app_data(web::Data::from(app_state))
        .app_data(web::Data::new(pool))
        .configure(crate::routes::init)
}
