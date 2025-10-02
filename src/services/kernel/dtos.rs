use serde::Deserialize;
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
