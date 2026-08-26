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
    pub storage: StorageConfig,
    pub jwt: JwtConfig,
    pub license: LicenseConfig,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub port: u16,
    pub sentry_dsn: String,
}

#[derive(Debug, Clone)]
pub struct JwtConfig {
    pub secret: String,
}

#[derive(Debug, Clone)]
pub struct LicenseConfig {
    pub public_key_base64: Option<String>,
    pub issuer: String,
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub endpoint: String,
    pub public_endpoint: Option<String>,
    pub region: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let app = AppConfig::from_env()?;
        let database = DatabaseConfig::from_env()?;
        let storage = StorageConfig::from_env()?;
        let jwt = JwtConfig::from_env()?;
        let license = LicenseConfig::from_env();

        Ok(Config {
            app,
            database,
            storage,
            jwt,
            license,
        })
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

impl StorageConfig {
    fn from_env() -> Result<Self> {
        let endpoint = env::var("STORAGE_ENDPOINT").context("STORAGE_ENDPOINT must be set")?;
        let public_endpoint = env::var("STORAGE_PUBLIC_ENDPOINT").ok();
        let region = env::var("STORAGE_REGION").unwrap_or_else(|_| "us-east-1".to_string());
        let bucket = env::var("STORAGE_BUCKET").context("STORAGE_BUCKET must be set")?;
        let access_key =
            env::var("STORAGE_ACCESS_KEY").context("STORAGE_ACCESS_KEY must be set")?;
        let secret_key =
            env::var("STORAGE_SECRET_KEY").context("STORAGE_SECRET_KEY must be set")?;

        Ok(StorageConfig {
            endpoint,
            public_endpoint,
            region,
            bucket,
            access_key,
            secret_key,
        })
    }
}

impl JwtConfig {
    fn from_env() -> Result<Self> {
        let secret = env::var("JWT_SECRET").context("JWT_SECRET must be set")?;
        Ok(JwtConfig { secret })
    }
}

impl LicenseConfig {
    fn from_env() -> Self {
        Self {
            public_key_base64: env::var("LICENSE_PUBLIC_KEY_BASE64")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            issuer: env::var("LICENSE_ISSUER")
                .unwrap_or_else(|_| "campus-pilot-licensing".to_string()),
        }
    }
}
