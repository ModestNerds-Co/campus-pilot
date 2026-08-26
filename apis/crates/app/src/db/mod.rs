//
//  campus-pilot-apis
//  mod.rs
//
//  Created by Ngonidzashe Mangudya on 2025/06/21.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use anyhow::Result;
use sqlx::PgPool;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

pub struct DatabaseOperations {
    pool: PgPool,
}

impl DatabaseOperations {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Run database migrations
    pub async fn run_migrations(&self) -> Result<()> {
        // Core migrations call these functions, so install them before the
        // tracked migrator runs. CREATE OR REPLACE keeps this safe on restart.
        let functions_migration = include_str!("../../../../migrations/functions.sql");
        sqlx::raw_sql(functions_migration)
            .execute(&self.pool)
            .await?;

        // Numbered migrations are applied exactly once and recorded in
        // _sqlx_migrations. This is essential for editable seeded roles: a
        // restart must never replay seed labels or permissions over user edits.
        MIGRATOR.run(&self.pool).await?;

        // Legacy audit trigger definitions are intentionally idempotent and
        // remain outside the numbered sequence.
        let triggers_migration = include_str!("../../../../migrations/triggers.sql");
        sqlx::raw_sql(triggers_migration)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
