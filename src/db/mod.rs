//
//  campus-pilot-apis
//  mod.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use anyhow::Result;
use sqlx::PgPool;

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
        let functions_migration = include_str!("../../migrations/functions.sql");
        let triggers_migration = include_str!("../../migrations/triggers.sql");

        // Execute table creations
        sqlx::raw_sql(migration_001).execute(&self.pool).await?;

        // Execute function creations
        sqlx::raw_sql(functions_migration)
            .execute(&self.pool)
            .await?;

        // Execute trigger creations
        sqlx::raw_sql(triggers_migration)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
