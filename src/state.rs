//
//  campus-pilot-apis
//  state.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/22.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use aws_credential_types::Credentials;
use aws_sdk_s3::{Client as S3Client, config::Region};
use sqlx::PgPool;

use crate::config::Config;
use crate::db::DatabaseOperations;
use crate::services::kernel::db::KernelDbOps;
use crate::services::storage::ops::StorageOps;

use std::sync::Arc;

pub struct AppState {
    pub db: PgPool,
    pub db_ops: Arc<DatabaseOperations>,
    pub kernel_db: Arc<KernelDbOps>,
    pub storage_ops: Arc<StorageOps>,
    pub config: Arc<Config>,
}

impl AppState {
    pub fn init(pool: PgPool, config: Config) -> Self {
        let config_arc = Arc::new(config.clone());
        let db_ops = Arc::new(DatabaseOperations::new(pool.clone()));
        let kernel_db = Arc::new(KernelDbOps::new(pool.clone()));

        // Initialize MinIO/S3 client
        let credentials = Credentials::new(
            &config.storage.access_key,
            &config.storage.secret_key,
            None,
            None,
            "static",
        );

        let s3_config = aws_sdk_s3::config::Builder::new()
            .endpoint_url(&config.storage.endpoint)
            .region(Region::new(config.storage.region.clone()))
            .credentials_provider(credentials)
            .force_path_style(true)
            .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
            .build();

        let s3_client = S3Client::from_conf(s3_config);
        let storage_ops = Arc::new(StorageOps::new(s3_client, config.storage.bucket.clone()));

        Self {
            db: pool,
            db_ops,
            kernel_db,
            storage_ops,
            config: config_arc,
        }
    }
}
