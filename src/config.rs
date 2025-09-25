//
//  campus-pilot-apis
//  config.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/30.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use anyhow::{Context, Result};
use std::env;
use urlencoding::encode;

#[derive(Debug, Clone)]
pub struct Config {
    pub app: AppConfig,
    pub database: DatabaseConfig,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub port: u16,
    pub sentry_dsn: String,
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let app = AppConfig::from_env()?;
        let database = DatabaseConfig::from_env()?;

        Ok(Config { app, database })
    }
}

impl AppConfig {
    fn from_env() -> Result<Self> {
        let port = env::var("APP_PORT")
            .context("APP_PORT must be set")?
            .parse::<u16>()
            .context("APP_PORT must be a valid port number")?;
        let sentry_dsn = env::var("SENTRY_DSN")
            .context("SENTRY_DSN must be set")?
            .parse::<String>()
            .context("SENTRY_DSN must be a valid port number")?;

        Ok(AppConfig { port, sentry_dsn })
    }
}

impl DatabaseConfig {
    fn from_env() -> Result<Self> {
        let user = std::env::var("DB_USER")?;
        let pass = std::env::var("DB_PASS")?;
        let host = std::env::var("DB_HOST")?;
        let port = std::env::var("DB_PORT")?;
        let db = std::env::var("DB_NAME")?;

        let url = format!(
            "postgresql://{}:{}@{}:{}/{}",
            encode(&user),
            encode(&pass),
            host,
            port,
            db
        );

        Ok(DatabaseConfig { url })
    }
}
