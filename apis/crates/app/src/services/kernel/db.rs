//! Owns the single-install bootstrap lifecycle and tenant-scoped school profile persistence.
//!
//! Bootstrap is closed permanently once the default campus has an active Campus Owner;
//! a stale mutable system-state row must never reopen unauthenticated setup.

use anyhow::Context;
use sqlx::PgPool;
use thiserror::Error;
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

#[derive(Debug, Error)]
pub enum KernelSetupError {
    #[error("Campus Pilot setup has already started or is complete")]
    InvalidState,
    #[error("Campus Pilot setup could not be saved")]
    Storage(#[source] anyhow::Error),
}

impl KernelSetupError {
    fn storage(error: impl Into<anyhow::Error>) -> Self {
        Self::Storage(error.into())
    }
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
        let (state_str, has_active_campus_owner): (String, bool) = sqlx::query_as(
            r#"
            SELECT state.state::TEXT,
                   EXISTS (
                       SELECT 1
                       FROM tenants AS tenant
                       JOIN users AS campus_owner ON campus_owner.tenant_id = tenant.id
                       WHERE tenant.slug = 'default'
                         AND tenant.deleted_at IS NULL
                         AND campus_owner.deleted_at IS NULL
                         AND campus_owner.is_active = TRUE
                         AND 'campus_owner' = ANY(campus_owner.roles)
                   )
            FROM system_state AS state
            WHERE state.id = 'singleton' AND state.deleted_at IS NULL
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to load bootstrap state")?;

        // The durable lifecycle is monotonic. Existing owner authority is
        // canonical evidence that bootstrap already completed, even if a
        // legacy migration or manual repair left the status row behind.
        let state = SystemState::from_bootstrap_facts(&state_str, has_active_campus_owner);

        // If system is Ready or SchoolConfigured, fetch school info
        let school = if matches!(state, SystemState::Ready | SystemState::SchoolConfigured) {
            match self.default_tenant_id().await {
                Ok(tenant_id) => self.get_school_info(tenant_id).await.ok(),
                Err(_) => None,
            }
        } else {
            None
        };

        Ok(KernelStatus { state, school })
    }

    async fn get_school_info(&self, tenant_id: Uuid) -> ApiResult<SchoolInfo> {
        let school = sqlx::query_as!(
            SchoolInfo,
            r#"
            SELECT name, legal_name, email, phone,
                   address_line1, address_line2, city, province, country,
                   logo_light_url, logo_dark_url
            FROM school_profile
            WHERE tenant_id = $1 AND deleted_at IS NULL
            "#,
            tenant_id
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

    pub async fn setup_school(&self, req: SetupSchoolRequest) -> Result<(), KernelSetupError> {
        let timezone = req.timezone.unwrap_or_else(|| "Africa/Harare".to_string());

        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to start school setup transaction")
            .map_err(KernelSetupError::storage)?;

        // Acquire the bootstrap transition in the database. The predicate and
        // row lock make concurrent/replayed unauthenticated setup requests fail
        // closed without leaving a persistent lock after rollback or process exit.
        let acquired = sqlx::query_scalar::<_, bool>(
            r#"
            UPDATE system_state
            SET kernel_lock = TRUE, updated_at = NOW()
            WHERE id = 'singleton'
              AND state = 'Uninitialized'
              AND kernel_lock = FALSE
              AND deleted_at IS NULL
              AND NOT EXISTS (
                  SELECT 1
                  FROM tenants AS tenant
                  JOIN users AS campus_owner ON campus_owner.tenant_id = tenant.id
                  WHERE tenant.slug = 'default'
                    AND tenant.deleted_at IS NULL
                    AND campus_owner.deleted_at IS NULL
                    AND campus_owner.is_active = TRUE
                    AND 'campus_owner' = ANY(campus_owner.roles)
              )
            RETURNING TRUE
            "#,
        )
        .fetch_optional(&mut *tx)
        .await
        .context("Failed to acquire school setup transition")
        .map_err(KernelSetupError::storage)?;
        if acquired.is_none() {
            return Err(KernelSetupError::InvalidState);
        }

        let tenant_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM tenants WHERE slug = 'default' AND deleted_at IS NULL",
        )
        .fetch_one(&mut *tx)
        .await
        .context("Failed to load the default tenant for school setup")
        .map_err(KernelSetupError::storage)?;

        // Keep the tenant record's own name/timezone in sync with the school profile.
        sqlx::query!(
            r#"UPDATE tenants SET name = $1, timezone = $2, updated_at = NOW() WHERE id = $3"#,
            req.name,
            timezone,
            tenant_id
        )
        .execute(&mut *tx)
        .await
        .context("Failed to update tenant record")
        .map_err(KernelSetupError::storage)?;

        // Insert or update only the default tenant's school profile. The partial
        // unique index guarantees one active profile per tenant without reviving
        // or overwriting another tenant's row.
        sqlx::query!(
            r#"
            INSERT INTO school_profile (
                tenant_id, name, legal_name, emap_code, phone, email,
                address_line1, address_line2, city, province, country,
                timezone, locale, logo_light_url, logo_dark_url
            )
            VALUES (
                $1, $2, $3, $4, $5, $6,
                $7, $8, $9, $10, $11,
                $12, $13, $14, $15
            )
            ON CONFLICT (tenant_id) WHERE deleted_at IS NULL DO UPDATE SET
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
                logo_light_url = COALESCE(EXCLUDED.logo_light_url, school_profile.logo_light_url),
                logo_dark_url = COALESCE(EXCLUDED.logo_dark_url, school_profile.logo_dark_url),
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
        .context("Failed to insert school profile")
        .map_err(KernelSetupError::storage)?;

        sqlx::query(
            r#"
            UPDATE system_state
            SET state = 'SchoolConfigured', kernel_lock = FALSE, updated_at = NOW()
            WHERE id = 'singleton' AND kernel_lock = TRUE
            "#,
        )
        .execute(&mut *tx)
        .await
        .context("Failed to complete school setup transition")
        .map_err(KernelSetupError::storage)?;

        tx.commit()
            .await
            .context("Failed to commit school setup transaction")
            .map_err(KernelSetupError::storage)?;

        Ok(())
    }

    pub async fn create_admin_user(
        &self,
        full_name: &str,
        email: &str,
        phone: Option<&str>,
        password_hash: &str,
    ) -> Result<Uuid, KernelSetupError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to start administrator setup transaction")
            .map_err(KernelSetupError::storage)?;

        let acquired = sqlx::query_scalar::<_, bool>(
            r#"
            UPDATE system_state
            SET kernel_lock = TRUE, updated_at = NOW()
            WHERE id = 'singleton'
              AND state = 'SchoolConfigured'
              AND kernel_lock = FALSE
              AND deleted_at IS NULL
              AND NOT EXISTS (
                  SELECT 1
                  FROM tenants AS tenant
                  JOIN users AS campus_owner ON campus_owner.tenant_id = tenant.id
                  WHERE tenant.slug = 'default'
                    AND tenant.deleted_at IS NULL
                    AND campus_owner.deleted_at IS NULL
                    AND campus_owner.is_active = TRUE
                    AND 'campus_owner' = ANY(campus_owner.roles)
              )
            RETURNING TRUE
            "#,
        )
        .fetch_optional(&mut *tx)
        .await
        .context("Failed to acquire administrator setup transition")
        .map_err(KernelSetupError::storage)?;
        if acquired.is_none() {
            return Err(KernelSetupError::InvalidState);
        }

        let tenant_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM tenants WHERE slug = 'default' AND deleted_at IS NULL",
        )
        .fetch_one(&mut *tx)
        .await
        .context("Failed to load the default tenant for administrator setup")
        .map_err(KernelSetupError::storage)?;

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
        .context("Failed to check for existing user")
        .map_err(KernelSetupError::storage)?;

        if existing_user.is_some() {
            return Err(KernelSetupError::InvalidState);
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
        .context("Failed to create admin user")
        .map_err(KernelSetupError::storage)?;

        sqlx::query(
            r#"
            UPDATE system_state
            SET state = 'Ready', kernel_lock = FALSE, updated_at = NOW()
            WHERE id = 'singleton' AND kernel_lock = TRUE
            "#,
        )
        .execute(&mut *tx)
        .await
        .context("Failed to complete administrator setup transition")
        .map_err(KernelSetupError::storage)?;

        tx.commit()
            .await
            .context("Failed to commit administrator setup transaction")
            .map_err(KernelSetupError::storage)?;

        Ok(user_id)
    }

    pub async fn get_school_profile(&self, tenant_id: Uuid) -> ApiResult<SchoolProfileResponse> {
        let profile = sqlx::query_as!(
            SchoolProfileResponse,
            r#"
            SELECT id, name, legal_name, emap_code, email, phone,
                   address_line1, address_line2, city, province, country,
                   timezone, locale, logo_light_url, logo_dark_url
            FROM school_profile
            WHERE tenant_id = $1 AND deleted_at IS NULL
            "#,
            tenant_id
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to fetch school profile")?;

        Ok(profile)
    }

    pub async fn update_school_profile(
        &self,
        tenant_id: Uuid,
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
            WHERE tenant_id = $13 AND deleted_at IS NULL
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
            req.locale,
            tenant_id
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to update school profile")?;

        Ok(profile)
    }

    pub async fn update_school_logos(
        &self,
        tenant_id: Uuid,
        logo_light_url: Option<String>,
        logo_dark_url: Option<String>,
    ) -> ApiResult<(Option<String>, Option<String>)> {
        let result = sqlx::query!(
            r#"
            UPDATE school_profile
            SET logo_light_url = COALESCE($1, logo_light_url),
                logo_dark_url = COALESCE($2, logo_dark_url),
                updated_at = NOW()
            WHERE tenant_id = $3 AND deleted_at IS NULL
            RETURNING logo_light_url, logo_dark_url
            "#,
            logo_light_url,
            logo_dark_url,
            tenant_id
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to update school logos")?;

        Ok((result.logo_light_url, result.logo_dark_url))
    }
}
