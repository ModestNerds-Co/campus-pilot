//
//  campus-pilot-apis
//  config.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/30.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use anyhow::{Context, Result};
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub app: AppConfig,
    pub database: DatabaseConfig,
    pub email: EmailConfig,
    pub turnstile: TurnstileConfig,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub port: u16,
    pub frontend_base_url: String,
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct EmailConfig {
    pub user: String,
    pub password: String,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone)]
pub struct TurnstileConfig {
    pub secret: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let app = AppConfig::from_env()?;
        let database = DatabaseConfig::from_env()?;
        let email = EmailConfig::from_env()?;
        let turnstile = TurnstileConfig::from_env()?;

        Ok(Config {
            app,
            database,
            email,
            turnstile,
        })
    }
}

impl AppConfig {
    fn from_env() -> Result<Self> {
        let port = env::var("APP_PORT")
            .context("APP_PORT must be set")?
            .parse::<u16>()
            .context("APP_PORT must be a valid port number")?;
        let frontend_base_url =
            env::var("FRONTEND_BASE_URL").context("FRONTEND_BASE_URL must be set")?;

        Ok(AppConfig {
            port,
            frontend_base_url,
        })
    }
}

impl DatabaseConfig {
    fn from_env() -> Result<Self> {
        let url = env::var("DATABASE_URL").context("DATABASE_URL must be set")?;

        Ok(DatabaseConfig { url })
    }
}

impl EmailConfig {
    fn from_env() -> Result<Self> {
        let user = env::var("EMAIL_USER").context("EMAIL_USER must be set")?;
        let password = env::var("EMAIL_PASSWORD").context("EMAIL_PASSWORD must be set")?;
        let host = env::var("EMAIL_HOST").context("EMAIL_HOST must be set")?;
        let port = env::var("EMAIL_PORT")
            .context("EMAIL_PORT must be set")?
            .parse::<u16>()
            .context("EMAIL_PORT must be a valid port number")?;

        Ok(EmailConfig {
            user,
            password,
            host,
            port,
        })
    }
}

impl TurnstileConfig {
    fn from_env() -> Result<Self> {
        let secret = env::var("TURNSTILE_SECRET").context("TURNSTILE_SECRET must be set")?;

        Ok(TurnstileConfig { secret })
    }
}
