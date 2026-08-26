use anyhow::Context;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    models::typedefs::ApiResult,
    services::kernel::{
        dtos::{SchoolProfileResponse, SetupSchoolRequest, UpdateSchoolProfileRequest},
        models::{KernelStatus, SchoolInfo, SystemState},
    },
};

pub struct KernelDbOps {
    pool: PgPool,
}

impl KernelDbOps {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// The single tenant every on-prem / not-yet-multi-tenant install provisions
    /// during bootstrap (seeded by migration 004). A future "create additional
    /// school" flow will provision further tenants outside this bootstrap path.
    async fn default_tenant_id(&self) -> ApiResult<Uuid> {
        let id = sqlx::query_scalar!(
            r#"SELECT id FROM tenants WHERE slug = 'default' AND deleted_at IS NULL"#
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to load default tenant")?;

        Ok(id)
    }

    pub async fn get_kernel_status(&self) -> ApiResult<KernelStatus> {
        let state_str: String =
            sqlx::query_scalar("SELECT state::TEXT FROM system_state WHERE id='singleton'")
                .fetch_one(&self.pool)
                .await
                .unwrap_or_else(|_| "Uninitialized".to_string());

        let state = SystemState::from_str(&state_str);

        // If system is Ready or SchoolConfigured, fetch school info
        let school = if matches!(state, SystemState::Ready | SystemState::SchoolConfigured) {
            self.get_school_info().await.ok()
        } else {
            None
        };

        Ok(KernelStatus { state, school })
    }

    async fn get_school_info(&self) -> ApiResult<SchoolInfo> {
        let school = sqlx::query_as!(
            SchoolInfo,
            r#"
            SELECT name, legal_name, email, phone,
                   address_line1, address_line2, city, province, country,
                   logo_light_url, logo_dark_url
            FROM school_profile
            WHERE id = 'singleton'
            "#
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to fetch school info")?;

        Ok(school)
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
        let tenant_id = self.default_tenant_id().await?;
        let timezone = req.timezone.unwrap_or_else(|| "Africa/Harare".to_string());

        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to start transaction")?;

        // Keep the tenant record's own name/timezone in sync with the school profile.
        sqlx::query!(
            r#"UPDATE tenants SET name = $1, timezone = $2, updated_at = NOW() WHERE id = $3"#,
            req.name,
            timezone,
            tenant_id
        )
        .execute(&mut *tx)
        .await
        .context("Failed to update tenant record")?;

        // Insert or update school profile
        sqlx::query!(
            r#"
            INSERT INTO school_profile (
                id, tenant_id, name, legal_name, emap_code, phone, email,
                address_line1, address_line2, city, province, country,
                timezone, locale, logo_light_url, logo_dark_url
            )
            VALUES (
                'singleton', $1, $2, $3, $4, $5, $6,
                $7, $8, $9, $10, $11,
                $12, $13, $14, $15
            )
            ON CONFLICT (id) DO UPDATE SET
                tenant_id = EXCLUDED.tenant_id,
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
            tenant_id,
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
            timezone,
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

    pub async fn create_admin_user(
        &self,
        full_name: &str,
        email: &str,
        phone: Option<&str>,
        password_hash: &str,
    ) -> ApiResult<Uuid> {
        let tenant_id = self.default_tenant_id().await?;

        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to start transaction")?;

        // Check if user already exists (within this tenant)
        let existing_user: Option<(Uuid,)> = sqlx::query_as(
            r#"
            SELECT id FROM users
            WHERE tenant_id = $1 AND LOWER(email) = LOWER($2) AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(email)
        .fetch_optional(&mut *tx)
        .await
        .context("Failed to check for existing user")?;

        if existing_user.is_some() {
            return Err(anyhow::anyhow!("User with this email already exists"));
        }

        // Create the campus owner. Role assignments use immutable keys so the
        // human-facing seeded role name can be edited later without breaking access.
        let user_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO users (tenant_id, full_name, email, phone, password_hash, roles, is_active)
            VALUES ($1, $2, $3, $4, $5, $6, true)
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(full_name)
        .bind(email)
        .bind(phone)
        .bind(password_hash)
        .bind(vec!["campus_owner"])
        .fetch_one(&mut *tx)
        .await
        .context("Failed to create admin user")?;

        tx.commit().await.context("Failed to commit transaction")?;

        // Update system state to Ready (outside transaction)
        self.update_system_state(SystemState::Ready).await?;

        Ok(user_id)
    }

    pub async fn get_school_profile(&self) -> ApiResult<SchoolProfileResponse> {
        let profile = sqlx::query_as!(
            SchoolProfileResponse,
            r#"
            SELECT id, name, legal_name, emap_code, email, phone,
                   address_line1, address_line2, city, province, country,
                   timezone, locale, logo_light_url, logo_dark_url
            FROM school_profile
            WHERE id = 'singleton'
            "#
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to fetch school profile")?;

        Ok(profile)
    }

    pub async fn update_school_profile(
        &self,
        req: UpdateSchoolProfileRequest,
    ) -> ApiResult<SchoolProfileResponse> {
        let profile = sqlx::query_as!(
            SchoolProfileResponse,
            r#"
            UPDATE school_profile
            SET name = COALESCE($1, name),
                legal_name = COALESCE($2, legal_name),
                emap_code = COALESCE($3, emap_code),
                email = COALESCE($4, email),
                phone = COALESCE($5, phone),
                address_line1 = COALESCE($6, address_line1),
                address_line2 = COALESCE($7, address_line2),
                city = COALESCE($8, city),
                province = COALESCE($9, province),
                country = COALESCE($10, country),
                timezone = COALESCE($11, timezone),
                locale = COALESCE($12, locale),
                updated_at = NOW()
            WHERE id = 'singleton'
            RETURNING id, name, legal_name, emap_code, email, phone,
                      address_line1, address_line2, city, province, country,
                      timezone, locale, logo_light_url, logo_dark_url
            "#,
            req.name,
            req.legal_name,
            req.emap_code,
            req.email,
            req.phone,
            req.address_line1,
            req.address_line2,
            req.city,
            req.province,
            req.country,
            req.timezone,
            req.locale
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to update school profile")?;

        Ok(profile)
    }

    pub async fn update_school_logos(
        &self,
        logo_light_url: Option<String>,
        logo_dark_url: Option<String>,
    ) -> ApiResult<(Option<String>, Option<String>)> {
        let result = sqlx::query!(
            r#"
            UPDATE school_profile
            SET logo_light_url = COALESCE($1, logo_light_url),
                logo_dark_url = COALESCE($2, logo_dark_url),
                updated_at = NOW()
            WHERE id = 'singleton'
            RETURNING logo_light_url, logo_dark_url
            "#,
            logo_light_url,
            logo_dark_url
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to update school logos")?;

        Ok((result.logo_light_url, result.logo_dark_url))
    }
}
