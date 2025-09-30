use serde::Serialize;

#[derive(Serialize)]
pub struct KernelStatus {
    pub state: String,
}
