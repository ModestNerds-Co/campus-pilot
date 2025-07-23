//
//  campus-pilot-apis
//  mod.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

mod utils;

pub use utils::load_email_template;
pub use utils::render_email_template;
pub use utils::send_email;
pub use utils::send_email_with_attachments;
pub use utils::status_meaning;
pub use utils::verify_turnstile;
