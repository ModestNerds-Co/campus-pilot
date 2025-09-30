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
    pub logo_light_b64: Option<String>,
    pub logo_dark_b64: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateAdminReq {
    pub full_name: String,
    pub email: String,
    pub phone: Option<String>,
    pub password: String,
}
