//
//  campus-pilot-apis
//  mod.rs
//
//  Created by Ngonidzashe Mangudya on 2025/10/02.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

mod dtos;
pub mod models;
mod ops;
mod routes;

pub(crate) use ops::AuthOps;
pub use routes::routes;
