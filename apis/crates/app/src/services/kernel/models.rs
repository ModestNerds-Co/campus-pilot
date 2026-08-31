use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum SystemState {
    Uninitialized,
    SchoolConfigured,
    Ready,
}

impl fmt::Display for SystemState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SystemState::Uninitialized => write!(f, "Uninitialized"),
            SystemState::SchoolConfigured => write!(f, "SchoolConfigured"),
            SystemState::Ready => write!(f, "Ready"),
        }
    }
}

impl SystemState {
    pub fn from_db_value(s: &str) -> Self {
        match s {
            "SchoolConfigured" => SystemState::SchoolConfigured,
            "Ready" => SystemState::Ready,
            _ => SystemState::Uninitialized,
        }
    }
}

#[derive(Serialize)]
pub struct SchoolInfo {
    pub name: String,
    pub legal_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub city: Option<String>,
    pub province: Option<String>,
    pub country: Option<String>,
    pub logo_light_url: Option<String>,
    pub logo_dark_url: Option<String>,
}

#[derive(Serialize)]
pub struct KernelStatus {
    pub state: SystemState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub school: Option<SchoolInfo>,
}
