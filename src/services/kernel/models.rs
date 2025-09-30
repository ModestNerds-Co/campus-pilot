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
    pub fn from_str(s: &str) -> Self {
        match s {
            "SchoolConfigured" => SystemState::SchoolConfigured,
            "Ready" => SystemState::Ready,
            _ => SystemState::Uninitialized,
        }
    }
}

#[derive(Serialize)]
pub struct KernelStatus {
    pub state: SystemState,
}
