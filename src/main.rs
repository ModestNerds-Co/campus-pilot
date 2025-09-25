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
use actix_web::{App, HttpResponse, HttpServer, web};
use campus_pilot::db::DatabaseOperations;
use dotenv::dotenv;
use log::info;
use sentry::integrations::log::LogFilter;
use sentry_actix::Sentry;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

mod config;
mod db;
mod dtos;
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

    // Load and validate configuration
    let config = Config::from_env()?;
    info!("Configuration loaded successfully 📦");

    // Initialize Sentry first for error tracking
    let _guard = if !config.app.sentry_dsn.is_empty() {
        info!("Initializing Sentry monitoring 📊");
        let guard = sentry::init((
            config.app.sentry_dsn.clone(),
            sentry::ClientOptions {
                release: sentry::release_name!(),
                enable_logs: true,
                ..Default::default()
            },
        ));

        Some(guard)
    } else {
        info!("Sentry not configured (SENTRY_DSN not set) 🚫");
        None
    };

    let logger = sentry::integrations::log::SentryLogger::with_dest(
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).build(),
    )
    .filter(|md| match md.level() {
        // Capture error and warning records as Sentry events
        log::Level::Error | log::Level::Warn => LogFilter::Event,
        // Ignore trace and debug level records, as they're too verbose
        log::Level::Trace | log::Level::Debug => LogFilter::Ignore,
        // Capture everything else as a breadcrumb
        _ => LogFilter::Breadcrumb,
    });
    log::set_boxed_logger(Box::new(logger))?;
    log::set_max_level(log::LevelFilter::Trace);

    info!("Booting API 🥱");

    let db_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database.url)
        .await?;

    let db_ops = DatabaseOperations::new(db_pool.clone());

    let app_state = Arc::new(AppState::init(db_pool, config.clone()));

    let addr = format!("0.0.0.0:{}", config.app.port);
    info!("Ready to rock and roll on {} 🚀", addr);

    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::from(app_state.clone()))
            .app_data(JsonConfig::default().error_handler(|err, _req| {
                let message: String = match &err {
                    actix_web::error::JsonPayloadError::ContentType => {
                        "Expected Content-Type: application/json".to_string()
                    }
                    actix_web::error::JsonPayloadError::Deserialize(e) => {
                        format!("Invalid JSON: {}", e)
                    }
                    _ => "Malformed JSON payload".to_string(),
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
            .wrap(Sentry::new())
            .wrap(Cors::permissive())
            .wrap(Governor::new(
                &GovernorConfigBuilder::default()
                    .per_second(200)
                    .burst_size(200)
                    .finish()
                    .unwrap(),
            ))
            .wrap(Logger::default())
            .configure(routes::init)
    })
    .bind(addr.as_str())?
    .run()
    .await;

    Ok(server)
}
