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
        // Read and execute the migration files in order
        let migration_001 = include_str!("../../migrations/001_create_tables.sql");
        let functions_migration = include_str!("../../migrations/functions.sql");
        let triggers_migration = include_str!("../../migrations/triggers.sql");
        let migration_002 = include_str!("../../migrations/002_create_auth_tables.sql");

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

        // Execute auth tables migration
        sqlx::raw_sql(migration_002).execute(&self.pool).await?;

        Ok(())
    }
}
