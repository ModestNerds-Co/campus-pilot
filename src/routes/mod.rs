//
//  campus-pilot-apis
//  mod.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use actix_web::web::{ServiceConfig, scope};

pub mod health;

pub fn init(cfg: &mut ServiceConfig) {
    cfg.service(scope("/api/1.0").configure(health::init));
}
