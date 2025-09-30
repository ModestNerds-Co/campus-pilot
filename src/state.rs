//
//  campus-pilot-apis
//  state.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/22.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use sqlx::PgPool;

use crate::config::Config;
use crate::db::DatabaseOperations;
use crate::services::kernel::db::KernelDbOps;

use std::sync::Arc;

pub struct AppState {
    pub db_ops: Arc<DatabaseOperations>,
    pub kernel_db: Arc<KernelDbOps>,
    pub config: Arc<Config>,
}

impl AppState {
    pub fn init(pool: PgPool, config: Config) -> Self {
        let config = Arc::new(config);
        let db_ops = Arc::new(DatabaseOperations::new(pool.clone()));
        let kernel_db = Arc::new(KernelDbOps::new(pool.clone()));

        Self {
            db_ops,
            kernel_db,
            config,
        }
    }
}
