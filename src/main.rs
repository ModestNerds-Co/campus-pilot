//
//  campus-pilot-apis
//  main.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use actix_cors::Cors;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::http::StatusCode;
use actix_web::middleware::Logger;
use actix_web::web::JsonConfig;
use actix_web::{web, App, HttpResponse, HttpServer};
use dotenv::dotenv;
use log::info;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

mod config;
mod db;
mod dtos;
mod handlers;
mod models;
mod routes;
mod services;
mod state;
mod utils;

use crate::config::Config;
use crate::models::ApiResponse;
use state::AppState;

#[actix_web::main]
async fn main() -> anyhow::Result<std::io::Result<()>> {
    dotenv().ok();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("Booting API 🥱");

    // Load and validate configuration
    let config = Config::from_env()?;
    info!("Configuration loaded successfully 📦");

    let db_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database.url)
        .await?;

    // Run database migrations
    info!("Running database migrations... ⚙️");
    let db_ops = crate::db::DatabaseOperations::new(db_pool.clone());
    db_ops.run_migrations().await?;
    info!("Database migrations completed successfully 🍻");

    let app_state = Arc::new(AppState::init(db_pool, config.clone()));

    info!("Ready to rock and roll on port {} 🚀", config.app.port);
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::from(app_state.clone()))
            .app_data(JsonConfig::default().error_handler(|err, _req| {
                let message = match &err {
                    actix_web::error::JsonPayloadError::ContentType => {
                        "Expected Content-Type: application/json"
                    }
                    actix_web::error::JsonPayloadError::Deserialize(_) => {
                        "Invalid JSON: unable to parse request body"
                    }
                    _ => "Malformed JSON payload",
                };

                let res = ApiResponse::<()>::from_status(
                    StatusCode::BAD_REQUEST,
                    None,
                    Some(vec![message.to_string()]),
                );

                actix_web::error::InternalError::from_response(
                    err,
                    HttpResponse::BadRequest().json(res),
                )
                .into()
            }))
            .wrap(Cors::permissive())
            .wrap(Governor::new(
                &GovernorConfigBuilder::default()
                    .per_second(1)
                    .burst_size(5)
                    .finish()
                    .unwrap(),
            ))
            .wrap(Logger::default())
            .configure(routes::init)
    })
    .bind(("127.0.0.1", config.app.port))?
    .run()
    .await;

    Ok(server)
}
