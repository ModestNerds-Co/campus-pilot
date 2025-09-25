//
//  campus-pilot-apis
//  attachment_file.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/22.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AttachmentFile {
    pub filename: String,
    pub mime_type: String,
    pub data: String,
}
