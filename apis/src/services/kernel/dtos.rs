// Copyright (c) 2025-01-02 Codecraft Solutions
// Created: 2025-01-02
// Author: AI Assistant

use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Deserialize, Validate)]
pub struct SetupSchoolRequest {
    #[validate(length(min = 1, message = "School name is required"))]
    pub name: String,
    pub legal_name: Option<String>,
    pub emap_code: Option<String>,
    pub phone: Option<String>,
    #[validate(email(message = "Invalid email format"))]
    pub email: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub city: Option<String>,
    pub province: Option<String>,
    pub country: Option<String>,
    pub timezone: Option<String>,
    pub locale: Option<String>,
    #[validate(url(message = "Invalid logo light URL"))]
    pub logo_light_url: Option<String>,
    #[validate(url(message = "Invalid logo dark URL"))]
    pub logo_dark_url: Option<String>,
}

#[derive(Deserialize, Validate)]
pub struct CreateAdminReq {
    #[validate(length(min = 1, message = "Full name is required"))]
    pub full_name: String,
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
    pub phone: Option<String>,
    #[validate(length(min = 10, message = "Password must be at least 10 characters"))]
    pub password: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateSchoolProfileRequest {
    #[validate(length(
        min = 1,
        max = 255,
        message = "Name must be between 1 and 255 characters"
    ))]
    pub name: Option<String>,

    #[validate(length(max = 255, message = "Legal name must not exceed 255 characters"))]
    pub legal_name: Option<String>,

    #[validate(length(max = 50, message = "EMAP code must not exceed 50 characters"))]
    pub emap_code: Option<String>,

    #[validate(email(message = "Invalid email address"))]
    pub email: Option<String>,

    #[validate(length(max = 50, message = "Phone must not exceed 50 characters"))]
    pub phone: Option<String>,

    #[validate(length(max = 255, message = "Address line 1 must not exceed 255 characters"))]
    pub address_line1: Option<String>,

    #[validate(length(max = 255, message = "Address line 2 must not exceed 255 characters"))]
    pub address_line2: Option<String>,

    #[validate(length(max = 100, message = "City must not exceed 100 characters"))]
    pub city: Option<String>,

    #[validate(length(max = 100, message = "Province must not exceed 100 characters"))]
    pub province: Option<String>,

    #[validate(length(max = 100, message = "Country must not exceed 100 characters"))]
    pub country: Option<String>,

    #[validate(length(max = 50, message = "Timezone must not exceed 50 characters"))]
    pub timezone: Option<String>,

    #[validate(length(max = 10, message = "Locale must not exceed 10 characters"))]
    pub locale: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SchoolProfileResponse {
    pub id: String,
    pub name: String,
    pub legal_name: Option<String>,
    pub emap_code: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub city: Option<String>,
    pub province: Option<String>,
    pub country: Option<String>,
    pub timezone: Option<String>,
    pub locale: Option<String>,
    pub logo_light_url: Option<String>,
    pub logo_dark_url: Option<String>,
}

#[derive(Debug, Serialize, Validate)]
pub struct LogoUploadResponse {
    #[validate(length(min = 1, message = "Full name is required"))]
    pub logo_light_url: Option<String>,
    #[validate(length(min = 10, message = "Password must be at least 10 characters"))]
    pub logo_dark_url: Option<String>,
}
