use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AttachmentFile {
    pub filename: String,
    pub mime_type: String,
    pub data: String,
}
