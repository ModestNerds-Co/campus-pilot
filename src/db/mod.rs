//
//  campus-pilot-apis
//  mod.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use crate::models::operation_status::OperationStatus;
use crate::models::payment_mode::PaymentMode;
use crate::models::typedefs::ApiResult;
use crate::models::OrderUpdate;
use crate::models::{ContactMessageStatus, DonorInfo, Order, PaymentOption};
use anyhow::Result;
use rust_decimal::prelude::FromPrimitive;
use sqlx::types::{uuid, BigDecimal};
use sqlx::{PgPool, QueryBuilder};

pub struct DatabaseOperations {
    pool: PgPool,
}

impl DatabaseOperations {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Run database migrations
    pub async fn run_migrations(&self) -> Result<()> {
        // Read and execute the migration files
        let migration_001 = include_str!("../../migrations/001_create_tables.sql");

        sqlx::raw_sql(migration_001).execute(&self.pool).await?;
        Ok(())
    }
}
