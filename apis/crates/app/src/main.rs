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
use cp_audit::{CORRELATION_ID_HEADER, REQUEST_ID_HEADER};
use dotenv::dotenv;
use log::info;
use sentry::integrations::log::LogFilter;
use sentry_actix::Sentry;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use std::time::Duration;

mod config;
mod db;
mod middleware;
mod models;
mod routes;
mod services;
mod state;
mod utils;

use crate::config::Config;
use crate::middleware::RequestContextMiddleware;
use crate::models::ApiResponse;
use crate::services::access::{ops::AccessOps, routes::refresh_license_inner};
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

    let app_state = Arc::new(AppState::init(db_pool, config.clone()));
    info!(
        "Loaded {} executable Agent capabilities",
        app_state.agent_capabilities.descriptors().len()
    );

    // Run database migrations
    info!("Running database migrations... ⚙️");
    app_state.db_ops.run_migrations().await?;
    info!("Database migrations completed successfully 🍻");

    // Setup storage bucket
    info!("Setting up storage bucket... 🗄️");
    app_state.storage_ops.ensure_bucket_setup().await?;
    info!("Storage bucket configured successfully 📦");

    let refresh_state = Arc::clone(&app_state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60 * 60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            match AccessOps::due_license_tenants(&refresh_state.db).await {
                Ok(tenant_ids) => {
                    for tenant_id in tenant_ids {
                        if let Err(error) = refresh_license_inner(&refresh_state, tenant_id).await {
                            log::warn!(
                                "Scheduled license refresh failed for tenant {}: {:#}",
                                tenant_id,
                                error
                            );
                            let _ = AccessOps::note_license_error(
                                &refresh_state.db,
                                tenant_id,
                                "scheduled_refresh_failed",
                            )
                            .await;
                        }
                    }
                }
                Err(error) => log::error!("Scheduled license refresh scan failed: {:#}", error),
            }
        }
    });

    let addr = format!("0.0.0.0:{}", config.app.port);
    info!("Ready to rock and roll on {} 🚀", addr);

    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::from(app_state.clone()))
            // ERP module crates take a bare PgPool rather than the full
            // AppState, so they never need to depend on the `app` crate.
            .app_data(web::Data::new(app_state.db.clone()))
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
            .wrap(Cors::permissive().expose_headers([REQUEST_ID_HEADER, CORRELATION_ID_HEADER]))
            .wrap(Governor::new(
                &GovernorConfigBuilder::default()
                    .per_second(200)
                    .burst_size(200)
                    .finish()
                    .unwrap(),
            ))
            .wrap(Logger::default())
            .wrap(RequestContextMiddleware)
            .configure(routes::init)
    })
    .bind(addr.as_str())?
    .run()
    .await;

    Ok(server)
}
