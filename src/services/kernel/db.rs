use anyhow::Context;
use sqlx::PgPool;

use crate::{
    models::typedefs::ApiResult,
    services::kernel::{
        dtos::SetupSchoolRequest,
        models::{KernelStatus, SystemState},
    },
};

pub struct KernelDbOps {
    pool: PgPool,
}

impl KernelDbOps {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_kernel_status(&self) -> ApiResult<KernelStatus> {
        let state_str: String =
            sqlx::query_scalar("SELECT state::TEXT FROM system_state WHERE id='singleton'")
                .fetch_one(&self.pool)
                .await
                .unwrap_or_else(|_| "Uninitialized".to_string());

        Ok(KernelStatus {
            state: SystemState::from_str(&state_str),
        })
    }

    pub async fn update_system_state(&self, state: SystemState) -> ApiResult<()> {
        let state_str = state.to_string();
        sqlx::query(
            r#"
            UPDATE system_state
            SET state = $1::APP_STATE, updated_at = NOW()
            WHERE id = 'singleton'
            "#,
        )
        .bind(state_str)
        .execute(&self.pool)
        .await
        .context("Failed to update system state")?;

        Ok(())
    }

    pub async fn setup_school(&self, req: SetupSchoolRequest) -> ApiResult<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to start transaction")?;

        // Insert or update school profile
        sqlx::query!(
            r#"
            INSERT INTO school_profile (
                id, name, legal_name, emap_code, phone, email,
                address_line1, address_line2, city, province, country,
                timezone, locale, logo_light_url, logo_dark_url
            )
            VALUES (
                'singleton', $1, $2, $3, $4, $5,
                $6, $7, $8, $9, $10,
                $11, $12, $13, $14
            )
            ON CONFLICT (id) DO UPDATE SET
                name = EXCLUDED.name,
                legal_name = EXCLUDED.legal_name,
                emap_code = EXCLUDED.emap_code,
                phone = EXCLUDED.phone,
                email = EXCLUDED.email,
                address_line1 = EXCLUDED.address_line1,
                address_line2 = EXCLUDED.address_line2,
                city = EXCLUDED.city,
                province = EXCLUDED.province,
                country = EXCLUDED.country,
                timezone = EXCLUDED.timezone,
                locale = EXCLUDED.locale,
                logo_light_url = EXCLUDED.logo_light_url,
                logo_dark_url = EXCLUDED.logo_dark_url,
                updated_at = NOW()
            "#,
            req.name,
            req.legal_name,
            req.emap_code,
            req.phone,
            req.email,
            req.address_line1,
            req.address_line2,
            req.city,
            req.province,
            req.country.unwrap_or_else(|| "Zimbabwe".to_string()),
            req.timezone.unwrap_or_else(|| "Africa/Harare".to_string()),
            req.locale.unwrap_or_else(|| "en-ZW".to_string()),
            req.logo_light_url,
            req.logo_dark_url
        )
        .execute(&mut *tx)
        .await
        .context("Failed to insert school profile")?;

        tx.commit().await.context("Failed to commit transaction")?;

        // Update system state to SchoolConfigured (outside transaction)
        self.update_system_state(SystemState::SchoolConfigured)
            .await?;

        Ok(())
    }
}
