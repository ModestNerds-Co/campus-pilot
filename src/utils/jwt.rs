//
//  campus-pilot-apis
//  jwt.rs
//
//  Created by Ngonidzashe Mangudya on 2025/10/02.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use crate::models::typedefs::ApiResult;
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,          // Subject (user ID)
    pub email: String,      // User email
    pub roles: Vec<String>, // User roles
    pub exp: i64,           // Expiration timestamp
    pub iat: i64,           // Issued at timestamp
    pub jti: String,        // JWT ID (unique identifier)
}

/// Generate an access token (15 minutes expiry)
pub fn generate_access_token(
    user_id: Uuid,
    email: &str,
    roles: Vec<String>,
    secret: &str,
) -> ApiResult<String> {
    let now = Utc::now();
    let expiration = now + Duration::minutes(15);

    let claims = Claims {
        sub: user_id,
        email: email.to_string(),
        roles,
        exp: expiration.timestamp(),
        iat: now.timestamp(),
        jti: Uuid::new_v4().to_string(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| anyhow::anyhow!("Failed to generate access token: {}", e))
}

/// Generate a refresh token (7 days expiry)
pub fn generate_refresh_token(
    user_id: Uuid,
    email: &str,
    roles: Vec<String>,
    secret: &str,
) -> ApiResult<String> {
    let now = Utc::now();
    let expiration = now + Duration::days(7);

    let claims = Claims {
        sub: user_id,
        email: email.to_string(),
        roles,
        exp: expiration.timestamp(),
        iat: now.timestamp(),
        jti: Uuid::new_v4().to_string(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| anyhow::anyhow!("Failed to generate refresh token: {}", e))
}

/// Verify and decode a JWT token
pub fn verify_token(token: &str, secret: &str) -> ApiResult<Claims> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| anyhow::anyhow!("Failed to verify token: {}", e))?;

    Ok(token_data.claims)
}

/// Verify and decode an access token (alias for verify_token)
pub fn verify_access_token(token: &str, secret: &str) -> ApiResult<Claims> {
    verify_token(token, secret)
}
