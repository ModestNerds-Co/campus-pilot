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
        let migration_001 = include_str!("../../../../migrations/001_create_tables.sql");
        let functions_migration = include_str!("../../../../migrations/functions.sql");
        let triggers_migration = include_str!("../../../../migrations/triggers.sql");
        let migration_002 = include_str!("../../../../migrations/002_create_auth_tables.sql");
        let migration_003 = include_str!("../../../../migrations/003_create_roles_table.sql");
        let migration_004 = include_str!("../../../../migrations/004_create_tenants_table.sql");
        let migration_005 =
            include_str!("../../../../migrations/005_add_tenant_id_to_core_tables.sql");
        let migration_010 = include_str!("../../../../migrations/010_create_fleet_tables.sql");
        let migration_011 =
            include_str!("../../../../migrations/011_create_vehicle_daily_log_tables.sql");

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

        // Execute roles table migration
        sqlx::raw_sql(migration_003).execute(&self.pool).await?;

        // Execute tenancy migrations
        sqlx::raw_sql(migration_004).execute(&self.pool).await?;
        sqlx::raw_sql(migration_005).execute(&self.pool).await?;

        // Execute fleet + vehicle daily log migrations
        sqlx::raw_sql(migration_010).execute(&self.pool).await?;
        sqlx::raw_sql(migration_011).execute(&self.pool).await?;

        Ok(())
    }
}
