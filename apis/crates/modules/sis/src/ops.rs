//! Tenant-scoped SIS domain operations.
//!
//! SIS owns people and admissions state. Academics references are resolved
//! through typed Academics operations before any SIS write is performed.

use anyhow::{Context, Result, bail};
use chrono::{NaiveDate, Utc};
use cp_academics::ops::{AcademicGradeLevelOps, AcademicYearOps, ClassGroupOps};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    dtos::{
        AccountProfileKind, CreateApplicationRequest, CreateEnrolmentRequest,
        CreateGuardianRelationshipRequest, CreateGuardianRequest, CreateLearnerRequest,
        UpdateApplicationRequest, UpdateEnrolmentRequest, UpdateGuardianRelationshipRequest,
        UpdateGuardianRequest, UpdateLearnerRequest,
    },
    models::{
        AccountCandidate, Application, ApplicationWithDetails, AttendanceRosterEntry,
        ClassRosterEntry, Enrolment, EnrolmentWithDetails, GuardianRelationshipWithDetails,
        GuardianWithAccount, LearnerBillingReference, LearnerWithAccount,
    },
    numbering::allocate_learner_number,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteOutcome {
    Deleted,
    NotFound,
    InUse,
}

pub struct AccountCandidateOps;

impl AccountCandidateOps {
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        profile_kind: AccountProfileKind,
        profile_id: Option<Uuid>,
        search: Option<&str>,
    ) -> Result<Vec<AccountCandidate>> {
        let search = search.map(|value| format!("%{value}%"));
        let query = match profile_kind {
            AccountProfileKind::Learner => {
                r#"
                SELECT account.id, account.full_name, account.email
                FROM users AS account
                WHERE account.tenant_id = $1
                  AND account.is_active = TRUE
                  AND account.deleted_at IS NULL
                  AND ($2::TEXT IS NULL OR account.full_name ILIKE $2 OR account.email ILIKE $2)
                  AND NOT EXISTS (
                    SELECT 1 FROM learners AS learner
                    WHERE learner.tenant_id = $1
                      AND learner.account_id = account.id
                      AND learner.deleted_at IS NULL
                      AND ($3::UUID IS NULL OR learner.id <> $3)
                  )
                ORDER BY account.full_name, account.email
                LIMIT 100
                "#
            }
            AccountProfileKind::Guardian => {
                r#"
                SELECT account.id, account.full_name, account.email
                FROM users AS account
                WHERE account.tenant_id = $1
                  AND account.is_active = TRUE
                  AND account.deleted_at IS NULL
                  AND ($2::TEXT IS NULL OR account.full_name ILIKE $2 OR account.email ILIKE $2)
                  AND NOT EXISTS (
                    SELECT 1 FROM guardians AS guardian
                    WHERE guardian.tenant_id = $1
                      AND guardian.account_id = account.id
                      AND guardian.deleted_at IS NULL
                      AND ($3::UUID IS NULL OR guardian.id <> $3)
                  )
                ORDER BY account.full_name, account.email
                LIMIT 100
                "#
            }
        };
        sqlx::query_as::<_, AccountCandidate>(query)
            .bind(tenant_id)
            .bind(search)
            .bind(profile_id)
            .fetch_all(pool)
            .await
            .context("Failed to list account candidates")
    }
}

pub struct LearnerOps;

impl LearnerOps {
    /// Returns the minimum learner fields Fees needs to open or identify a
    /// billing account without granting SIS management access.
    pub async fn billing_references(
        pool: &PgPool,
        tenant_id: Uuid,
        search: Option<&str>,
        limit: i64,
    ) -> Result<Vec<LearnerBillingReference>> {
        let search = search.map(|value| format!("%{value}%"));
        sqlx::query_as::<_, LearnerBillingReference>(
            r#"
            SELECT id, learner_number, display_name, status
              FROM learners
             WHERE tenant_id = $1 AND deleted_at IS NULL
               AND ($2::TEXT IS NULL OR learner_number ILIKE $2 OR display_name ILIKE $2)
             ORDER BY display_name, learner_number
             LIMIT $3
            "#,
        )
        .bind(tenant_id)
        .bind(search)
        .bind(limit.clamp(1, 100))
        .fetch_all(pool)
        .await
        .context("Failed to list learner billing references")
    }

    /// Resolves learner identities linked to one authenticated account. Fees
    /// uses this for self-service account reads without joining SIS tables.
    pub async fn ids_for_linked_account(
        pool: &PgPool,
        tenant_id: Uuid,
        account_id: Uuid,
    ) -> Result<Vec<Uuid>> {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id FROM learners
             WHERE tenant_id = $1 AND account_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(account_id)
        .fetch_all(pool)
        .await
        .context("Failed to resolve account-linked learners")
    }

    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
    ) -> Result<(Vec<LearnerWithAccount>, i64)> {
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let learners = sqlx::query_as::<_, LearnerWithAccount>(
            r#"
            SELECT learner.id, learner.tenant_id, learner.account_id,
                   account.email AS account_email, learner.learner_number,
                   learner.display_name, learner.first_names, learner.surname,
                   learner.date_of_birth, learner.email, learner.phone, learner.status,
                   learner.created_at, learner.updated_at
            FROM learners AS learner
            LEFT JOIN users AS account
              ON account.id = learner.account_id AND account.tenant_id = learner.tenant_id
            WHERE learner.tenant_id = $1 AND learner.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR learner.learner_number ILIKE $2
                   OR learner.display_name ILIKE $2 OR learner.email ILIKE $2)
              AND ($3::TEXT IS NULL OR learner.status = $3)
            ORDER BY learner.display_name, learner.learner_number
            LIMIT $4 OFFSET $5
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list learners")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM learners
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND ($2::TEXT IS NULL OR learner_number ILIKE $2
                   OR display_name ILIKE $2 OR email ILIKE $2)
              AND ($3::TEXT IS NULL OR status = $3)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .fetch_one(pool)
        .await
        .context("Failed to count learners")?;
        Ok((learners, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<LearnerWithAccount>> {
        sqlx::query_as::<_, LearnerWithAccount>(
            r#"
            SELECT learner.id, learner.tenant_id, learner.account_id,
                   account.email AS account_email, learner.learner_number,
                   learner.display_name, learner.first_names, learner.surname,
                   learner.date_of_birth, learner.email, learner.phone, learner.status,
                   learner.created_at, learner.updated_at
            FROM learners AS learner
            LEFT JOIN users AS account
              ON account.id = learner.account_id AND account.tenant_id = learner.tenant_id
            WHERE learner.tenant_id = $1 AND learner.id = $2 AND learner.deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load learner")
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        request: &CreateLearnerRequest,
    ) -> Result<LearnerWithAccount> {
        validate_birth_date(request.date_of_birth)?;
        let display_name = required("Learner name", &request.display_name)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start learner creation")?;
        let learner_number = allocate_learner_number(&mut transaction, tenant_id).await?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO learners (
                tenant_id, learner_number, display_name, first_names, surname,
                date_of_birth, email, phone, status
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, COALESCE($9, 'prospective'))
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(&learner_number)
        .bind(display_name)
        .bind(optional_text(request.first_names.as_deref()))
        .bind(optional_text(request.surname.as_deref()))
        .bind(request.date_of_birth)
        .bind(normalized_email(request.email.as_deref()))
        .bind(optional_text(request.phone.as_deref()))
        .bind(request.status.map(|value| value.as_str()))
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to create learner")?;
        transaction
            .commit()
            .await
            .context("Failed to commit learner creation")?;
        Self::get_by_id(pool, tenant_id, id)
            .await?
            .context("Created learner could not be reloaded")
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        request: &UpdateLearnerRequest,
    ) -> Result<Option<LearnerWithAccount>> {
        validate_birth_date(request.date_of_birth)?;
        let display_name = required("Learner name", &request.display_name)?;
        let updated = sqlx::query(
            r#"
            UPDATE learners
            SET display_name = $1, first_names = $2, surname = $3,
                date_of_birth = $4, email = $5, phone = $6, status = $7,
                updated_at = NOW()
            WHERE tenant_id = $8 AND id = $9 AND deleted_at IS NULL
            "#,
        )
        .bind(display_name)
        .bind(optional_text(request.first_names.as_deref()))
        .bind(optional_text(request.surname.as_deref()))
        .bind(request.date_of_birth)
        .bind(normalized_email(request.email.as_deref()))
        .bind(optional_text(request.phone.as_deref()))
        .bind(request.status.as_str())
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to update learner")?;
        if updated.rows_affected() == 0 {
            return Ok(None);
        }
        Self::get_by_id(pool, tenant_id, id).await
    }

    pub async fn link_account(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        account_id: Option<Uuid>,
    ) -> Result<Option<LearnerWithAccount>> {
        validate_account(pool, tenant_id, account_id).await?;
        let updated = sqlx::query(
            "UPDATE learners SET account_id = $1, updated_at = NOW() WHERE tenant_id = $2 AND id = $3 AND deleted_at IS NULL",
        )
        .bind(account_id)
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to update learner account link")?;
        if updated.rows_affected() == 0 {
            return Ok(None);
        }
        Self::get_by_id(pool, tenant_id, id).await
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<DeleteOutcome> {
        if Self::get_by_id(pool, tenant_id, id).await?.is_none() {
            return Ok(DeleteOutcome::NotFound);
        }
        let in_use = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM learner_guardian_relationships
                WHERE tenant_id = $1 AND learner_id = $2 AND deleted_at IS NULL
            ) OR EXISTS(
                SELECT 1 FROM applications
                WHERE tenant_id = $1 AND learner_id = $2 AND deleted_at IS NULL
            ) OR EXISTS(
                SELECT 1 FROM enrolments
                WHERE tenant_id = $1 AND learner_id = $2 AND deleted_at IS NULL
            )
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_one(pool)
        .await
        .context("Failed to check learner references")?;
        if in_use {
            return Ok(DeleteOutcome::InUse);
        }
        sqlx::query(
            "UPDATE learners SET deleted_at = NOW(), account_id = NULL WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete learner")?;
        Ok(DeleteOutcome::Deleted)
    }
}

pub struct GuardianOps;

impl GuardianOps {
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
    ) -> Result<(Vec<GuardianWithAccount>, i64)> {
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let guardians = sqlx::query_as::<_, GuardianWithAccount>(
            r#"
            SELECT guardian.id, guardian.tenant_id, guardian.account_id,
                   account.email AS account_email, guardian.display_name,
                   guardian.first_names, guardian.surname, guardian.email,
                   guardian.phone, guardian.status, guardian.created_at, guardian.updated_at
            FROM guardians AS guardian
            LEFT JOIN users AS account
              ON account.id = guardian.account_id AND account.tenant_id = guardian.tenant_id
            WHERE guardian.tenant_id = $1 AND guardian.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR guardian.display_name ILIKE $2
                   OR guardian.email ILIKE $2 OR guardian.phone ILIKE $2)
              AND ($3::TEXT IS NULL OR guardian.status = $3)
            ORDER BY guardian.display_name
            LIMIT $4 OFFSET $5
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list guardians")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM guardians
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND ($2::TEXT IS NULL OR display_name ILIKE $2
                   OR email ILIKE $2 OR phone ILIKE $2)
              AND ($3::TEXT IS NULL OR status = $3)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .fetch_one(pool)
        .await
        .context("Failed to count guardians")?;
        Ok((guardians, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<GuardianWithAccount>> {
        sqlx::query_as::<_, GuardianWithAccount>(
            r#"
            SELECT guardian.id, guardian.tenant_id, guardian.account_id,
                   account.email AS account_email, guardian.display_name,
                   guardian.first_names, guardian.surname, guardian.email,
                   guardian.phone, guardian.status, guardian.created_at, guardian.updated_at
            FROM guardians AS guardian
            LEFT JOIN users AS account
              ON account.id = guardian.account_id AND account.tenant_id = guardian.tenant_id
            WHERE guardian.tenant_id = $1 AND guardian.id = $2 AND guardian.deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load guardian")
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        request: &CreateGuardianRequest,
    ) -> Result<GuardianWithAccount> {
        validate_guardian_contact(request.email.as_deref(), request.phone.as_deref())?;
        let display_name = required("Guardian name", &request.display_name)?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO guardians (
                tenant_id, display_name, first_names, surname, email, phone, status
            )
            VALUES ($1, $2, $3, $4, $5, $6, COALESCE($7, 'active'))
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(display_name)
        .bind(optional_text(request.first_names.as_deref()))
        .bind(optional_text(request.surname.as_deref()))
        .bind(normalized_email(request.email.as_deref()))
        .bind(optional_text(request.phone.as_deref()))
        .bind(request.status.map(|value| value.as_str()))
        .fetch_one(pool)
        .await
        .context("Failed to create guardian")?;
        Self::get_by_id(pool, tenant_id, id)
            .await?
            .context("Created guardian could not be reloaded")
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        request: &UpdateGuardianRequest,
    ) -> Result<Option<GuardianWithAccount>> {
        validate_guardian_contact(request.email.as_deref(), request.phone.as_deref())?;
        let display_name = required("Guardian name", &request.display_name)?;
        let updated = sqlx::query(
            r#"
            UPDATE guardians
            SET display_name = $1, first_names = $2, surname = $3, email = $4,
                phone = $5, status = $6, updated_at = NOW()
            WHERE tenant_id = $7 AND id = $8 AND deleted_at IS NULL
            "#,
        )
        .bind(display_name)
        .bind(optional_text(request.first_names.as_deref()))
        .bind(optional_text(request.surname.as_deref()))
        .bind(normalized_email(request.email.as_deref()))
        .bind(optional_text(request.phone.as_deref()))
        .bind(request.status.as_str())
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to update guardian")?;
        if updated.rows_affected() == 0 {
            return Ok(None);
        }
        Self::get_by_id(pool, tenant_id, id).await
    }

    pub async fn link_account(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        account_id: Option<Uuid>,
    ) -> Result<Option<GuardianWithAccount>> {
        validate_account(pool, tenant_id, account_id).await?;
        let updated = sqlx::query(
            "UPDATE guardians SET account_id = $1, updated_at = NOW() WHERE tenant_id = $2 AND id = $3 AND deleted_at IS NULL",
        )
        .bind(account_id)
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to update guardian account link")?;
        if updated.rows_affected() == 0 {
            return Ok(None);
        }
        Self::get_by_id(pool, tenant_id, id).await
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<DeleteOutcome> {
        if Self::get_by_id(pool, tenant_id, id).await?.is_none() {
            return Ok(DeleteOutcome::NotFound);
        }
        let in_use = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM learner_guardian_relationships WHERE tenant_id = $1 AND guardian_id = $2 AND deleted_at IS NULL)",
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_one(pool)
        .await
        .context("Failed to check guardian references")?;
        if in_use {
            return Ok(DeleteOutcome::InUse);
        }
        sqlx::query(
            "UPDATE guardians SET deleted_at = NOW(), account_id = NULL WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete guardian")?;
        Ok(DeleteOutcome::Deleted)
    }
}

pub struct GuardianRelationshipOps;

impl GuardianRelationshipOps {
    #[allow(clippy::too_many_arguments)]
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
        learner_id: Option<Uuid>,
        guardian_id: Option<Uuid>,
    ) -> Result<(Vec<GuardianRelationshipWithDetails>, i64)> {
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, GuardianRelationshipWithDetails>(
            r#"
            SELECT relationship.id, relationship.tenant_id, relationship.learner_id,
                   learner.display_name AS learner_name, learner.learner_number,
                   relationship.guardian_id, guardian.display_name AS guardian_name,
                   relationship.relationship_type, relationship.is_primary,
                   relationship.can_collect, relationship.receives_communications,
                   relationship.status, relationship.created_at, relationship.updated_at
            FROM learner_guardian_relationships AS relationship
            JOIN learners AS learner
              ON learner.id = relationship.learner_id AND learner.tenant_id = relationship.tenant_id
            JOIN guardians AS guardian
              ON guardian.id = relationship.guardian_id AND guardian.tenant_id = relationship.tenant_id
            WHERE relationship.tenant_id = $1 AND relationship.deleted_at IS NULL
              AND learner.deleted_at IS NULL AND guardian.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR learner.display_name ILIKE $2
                   OR learner.learner_number ILIKE $2 OR guardian.display_name ILIKE $2)
              AND ($3::TEXT IS NULL OR relationship.status = $3)
              AND ($4::UUID IS NULL OR relationship.learner_id = $4)
              AND ($5::UUID IS NULL OR relationship.guardian_id = $5)
            ORDER BY learner.display_name, relationship.is_primary DESC, guardian.display_name
            LIMIT $6 OFFSET $7
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(learner_id)
        .bind(guardian_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list guardian relationships")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM learner_guardian_relationships AS relationship
            JOIN learners AS learner
              ON learner.id = relationship.learner_id AND learner.tenant_id = relationship.tenant_id
            JOIN guardians AS guardian
              ON guardian.id = relationship.guardian_id AND guardian.tenant_id = relationship.tenant_id
            WHERE relationship.tenant_id = $1 AND relationship.deleted_at IS NULL
              AND learner.deleted_at IS NULL AND guardian.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR learner.display_name ILIKE $2
                   OR learner.learner_number ILIKE $2 OR guardian.display_name ILIKE $2)
              AND ($3::TEXT IS NULL OR relationship.status = $3)
              AND ($4::UUID IS NULL OR relationship.learner_id = $4)
              AND ($5::UUID IS NULL OR relationship.guardian_id = $5)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(learner_id)
        .bind(guardian_id)
        .fetch_one(pool)
        .await
        .context("Failed to count guardian relationships")?;
        Ok((rows, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<GuardianRelationshipWithDetails>> {
        sqlx::query_as::<_, GuardianRelationshipWithDetails>(
            r#"
            SELECT relationship.id, relationship.tenant_id, relationship.learner_id,
                   learner.display_name AS learner_name, learner.learner_number,
                   relationship.guardian_id, guardian.display_name AS guardian_name,
                   relationship.relationship_type, relationship.is_primary,
                   relationship.can_collect, relationship.receives_communications,
                   relationship.status, relationship.created_at, relationship.updated_at
            FROM learner_guardian_relationships AS relationship
            JOIN learners AS learner
              ON learner.id = relationship.learner_id AND learner.tenant_id = relationship.tenant_id
            JOIN guardians AS guardian
              ON guardian.id = relationship.guardian_id AND guardian.tenant_id = relationship.tenant_id
            WHERE relationship.tenant_id = $1 AND relationship.id = $2
              AND relationship.deleted_at IS NULL
              AND learner.deleted_at IS NULL AND guardian.deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load guardian relationship")
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        request: &CreateGuardianRelationshipRequest,
    ) -> Result<GuardianRelationshipWithDetails> {
        ensure_learner(pool, tenant_id, request.learner_id).await?;
        ensure_guardian(pool, tenant_id, request.guardian_id).await?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO learner_guardian_relationships (
                tenant_id, learner_id, guardian_id, relationship_type, is_primary,
                can_collect, receives_communications, status
            )
            VALUES ($1, $2, $3, $4, COALESCE($5, FALSE), COALESCE($6, FALSE),
                    COALESCE($7, TRUE), COALESCE($8, 'active'))
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(request.learner_id)
        .bind(request.guardian_id)
        .bind(request.relationship_type.as_str())
        .bind(request.is_primary)
        .bind(request.can_collect)
        .bind(request.receives_communications)
        .bind(request.status.map(|value| value.as_str()))
        .fetch_one(pool)
        .await
        .context("Failed to create guardian relationship")?;
        Self::get_by_id(pool, tenant_id, id)
            .await?
            .context("Created guardian relationship could not be reloaded")
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        request: &UpdateGuardianRelationshipRequest,
    ) -> Result<Option<GuardianRelationshipWithDetails>> {
        let updated = sqlx::query(
            r#"
            UPDATE learner_guardian_relationships
            SET relationship_type = $1, is_primary = $2, can_collect = $3,
                receives_communications = $4, status = $5, updated_at = NOW()
            WHERE tenant_id = $6 AND id = $7 AND deleted_at IS NULL
            "#,
        )
        .bind(request.relationship_type.as_str())
        .bind(request.is_primary)
        .bind(request.can_collect)
        .bind(request.receives_communications)
        .bind(request.status.as_str())
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to update guardian relationship")?;
        if updated.rows_affected() == 0 {
            return Ok(None);
        }
        Self::get_by_id(pool, tenant_id, id).await
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<DeleteOutcome> {
        let updated = sqlx::query(
            "UPDATE learner_guardian_relationships SET deleted_at = NOW() WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete guardian relationship")?;
        Ok(if updated.rows_affected() == 0 {
            DeleteOutcome::NotFound
        } else {
            DeleteOutcome::Deleted
        })
    }
}

pub struct ApplicationOps;

impl ApplicationOps {
    #[allow(clippy::too_many_arguments)]
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
        academic_year_id: Option<Uuid>,
        target_grade_level_id: Option<Uuid>,
        learner_id: Option<Uuid>,
    ) -> Result<(Vec<ApplicationWithDetails>, i64)> {
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, Application>(
            r#"
            SELECT application.id, application.tenant_id, application.application_number,
                   application.learner_id, application.academic_year_id,
                   application.target_grade_level_id, application.submitted_on,
                   application.status, application.notes, application.created_at,
                   application.updated_at
            FROM applications AS application
            JOIN learners AS learner
              ON learner.id = application.learner_id AND learner.tenant_id = application.tenant_id
            WHERE application.tenant_id = $1 AND application.deleted_at IS NULL
              AND learner.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR application.application_number ILIKE $2
                   OR learner.display_name ILIKE $2 OR learner.learner_number ILIKE $2)
              AND ($3::TEXT IS NULL OR application.status = $3)
              AND ($4::UUID IS NULL OR application.academic_year_id = $4)
              AND ($5::UUID IS NULL OR application.target_grade_level_id = $5)
              AND ($6::UUID IS NULL OR application.learner_id = $6)
            ORDER BY application.created_at DESC, application.application_number
            LIMIT $7 OFFSET $8
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(academic_year_id)
        .bind(target_grade_level_id)
        .bind(learner_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list applications")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM applications AS application
            JOIN learners AS learner
              ON learner.id = application.learner_id AND learner.tenant_id = application.tenant_id
            WHERE application.tenant_id = $1 AND application.deleted_at IS NULL
              AND learner.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR application.application_number ILIKE $2
                   OR learner.display_name ILIKE $2 OR learner.learner_number ILIKE $2)
              AND ($3::TEXT IS NULL OR application.status = $3)
              AND ($4::UUID IS NULL OR application.academic_year_id = $4)
              AND ($5::UUID IS NULL OR application.target_grade_level_id = $5)
              AND ($6::UUID IS NULL OR application.learner_id = $6)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(academic_year_id)
        .bind(target_grade_level_id)
        .bind(learner_id)
        .fetch_one(pool)
        .await
        .context("Failed to count applications")?;
        Ok((hydrate_applications(pool, tenant_id, rows).await?, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<ApplicationWithDetails>> {
        match Self::get_record_by_id(pool, tenant_id, id).await? {
            Some(row) => Ok(Some(hydrate_application(pool, tenant_id, row).await?)),
            None => Ok(None),
        }
    }

    async fn get_record_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<Application>> {
        sqlx::query_as::<_, Application>(
            r#"
            SELECT id, tenant_id, application_number, learner_id, academic_year_id,
                   target_grade_level_id, submitted_on, status, notes, created_at, updated_at
            FROM applications
            WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load application")
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        request: &CreateApplicationRequest,
    ) -> Result<ApplicationWithDetails> {
        ensure_learner(pool, tenant_id, request.learner_id).await?;
        validate_application_grade(
            pool,
            tenant_id,
            request.academic_year_id,
            request.target_grade_level_id,
            false,
        )
        .await?;
        let status = request
            .status
            .unwrap_or(crate::dtos::ApplicationStatus::Draft);
        validate_new_application_status(status)?;
        validate_application_state(status, request.submitted_on)?;
        let application_number = required("Application number", &request.application_number)?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO applications (
                tenant_id, application_number, learner_id, academic_year_id,
                target_grade_level_id, submitted_on, status, notes
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(application_number)
        .bind(request.learner_id)
        .bind(request.academic_year_id)
        .bind(request.target_grade_level_id)
        .bind(request.submitted_on)
        .bind(status.as_str())
        .bind(optional_text(request.notes.as_deref()))
        .fetch_one(pool)
        .await
        .context("Failed to create application")?;
        Self::get_by_id(pool, tenant_id, id)
            .await?
            .context("Created application could not be reloaded")
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        request: &UpdateApplicationRequest,
    ) -> Result<Option<ApplicationWithDetails>> {
        let Some(existing) = Self::get_record_by_id(pool, tenant_id, id).await? else {
            return Ok(None);
        };
        if existing.status != "draft"
            && (existing.learner_id != request.learner_id
                || existing.academic_year_id != request.academic_year_id
                || existing
                    .target_grade_level_id
                    .is_some_and(|target| target != request.target_grade_level_id))
        {
            bail!(
                "A submitted application cannot be moved to another learner, academic year, or grade level"
            );
        }
        ensure_learner(pool, tenant_id, request.learner_id).await?;
        validate_application_grade(
            pool,
            tenant_id,
            request.academic_year_id,
            request.target_grade_level_id,
            existing.academic_year_id == request.academic_year_id
                && existing
                    .target_grade_level_id
                    .is_none_or(|target| target == request.target_grade_level_id),
        )
        .await?;
        validate_application_transition(&existing.status, request.status)?;
        validate_application_state(request.status, request.submitted_on)?;
        let application_number = required("Application number", &request.application_number)?;
        let updated = sqlx::query(
            r#"
            UPDATE applications
            SET application_number = $1, learner_id = $2, academic_year_id = $3,
                target_grade_level_id = $4, submitted_on = $5, status = $6,
                notes = $7, updated_at = NOW()
            WHERE tenant_id = $8 AND id = $9 AND deleted_at IS NULL
            "#,
        )
        .bind(application_number)
        .bind(request.learner_id)
        .bind(request.academic_year_id)
        .bind(request.target_grade_level_id)
        .bind(request.submitted_on)
        .bind(request.status.as_str())
        .bind(optional_text(request.notes.as_deref()))
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to update application")?;
        if updated.rows_affected() == 0 {
            return Ok(None);
        }
        Self::get_by_id(pool, tenant_id, id).await
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<DeleteOutcome> {
        let Some(application) = Self::get_record_by_id(pool, tenant_id, id).await? else {
            return Ok(DeleteOutcome::NotFound);
        };
        let in_use = application.status != "draft"
            || sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM enrolments WHERE tenant_id = $1 AND source_application_id = $2 AND deleted_at IS NULL)",
            )
            .bind(tenant_id)
            .bind(id)
            .fetch_one(pool)
            .await
            .context("Failed to check application references")?;
        if in_use {
            return Ok(DeleteOutcome::InUse);
        }
        sqlx::query(
            "UPDATE applications SET deleted_at = NOW() WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete application")?;
        Ok(DeleteOutcome::Deleted)
    }
}

pub struct EnrolmentOps;

impl EnrolmentOps {
    /// Resolves learners the authenticated account may view as self-service.
    ///
    /// A direct learner account and active guardian relationships contribute to
    /// the same bounded identity set; callers still enforce their own domain
    /// record scope.
    pub async fn learner_ids_for_account(
        pool: &PgPool,
        tenant_id: Uuid,
        account_id: Uuid,
    ) -> Result<Vec<Uuid>> {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT learner.id
              FROM learners AS learner
             WHERE learner.tenant_id = $1
               AND learner.account_id = $2
               AND learner.deleted_at IS NULL
            UNION
            SELECT relationship.learner_id
              FROM guardians AS guardian
              JOIN learner_guardian_relationships AS relationship
                ON relationship.guardian_id = guardian.id
               AND relationship.tenant_id = guardian.tenant_id
               AND relationship.status = 'active'
               AND relationship.deleted_at IS NULL
             WHERE guardian.tenant_id = $1
               AND guardian.account_id = $2
               AND guardian.status = 'active'
               AND guardian.deleted_at IS NULL
             ORDER BY 1
            "#,
        )
        .bind(tenant_id)
        .bind(account_id)
        .fetch_all(pool)
        .await
        .context("Failed to resolve self-service learners")
    }

    /// Returns the date-effective SIS roster for one Academics-owned class.
    pub async fn class_roster_on(
        pool: &PgPool,
        tenant_id: Uuid,
        academic_year_id: Uuid,
        class_group_id: Uuid,
        effective_on: NaiveDate,
    ) -> Result<Vec<ClassRosterEntry>> {
        sqlx::query_as::<_, ClassRosterEntry>(
            r#"
            SELECT enrolment.id AS enrolment_id, learner.id AS learner_id,
                   learner.learner_number, learner.display_name
              FROM enrolments AS enrolment
              JOIN learners AS learner
                ON learner.id = enrolment.learner_id
               AND learner.tenant_id = enrolment.tenant_id
             WHERE enrolment.tenant_id = $1
               AND enrolment.academic_year_id = $2
               AND enrolment.class_group_id = $3
               AND enrolment.status = 'active'
               AND enrolment.starts_on <= $4
               AND (enrolment.ends_on IS NULL OR enrolment.ends_on >= $4)
               AND enrolment.deleted_at IS NULL
               AND learner.deleted_at IS NULL
             ORDER BY learner.display_name, learner.learner_number
            "#,
        )
        .bind(tenant_id)
        .bind(academic_year_id)
        .bind(class_group_id)
        .bind(effective_on)
        .fetch_all(pool)
        .await
        .context("Failed to load the SIS class roster")
    }

    /// Returns learners eligible for a class register on one date.
    pub async fn attendance_roster(
        pool: &PgPool,
        tenant_id: Uuid,
        academic_year_id: Uuid,
        class_group_id: Uuid,
        attendance_date: NaiveDate,
    ) -> Result<Vec<AttendanceRosterEntry>> {
        Self::class_roster_on(
            pool,
            tenant_id,
            academic_year_id,
            class_group_id,
            attendance_date,
        )
        .await
    }

    /// Resolves historical roster identities without applying current status.
    pub async fn roster_references_by_enrolment_ids(
        pool: &PgPool,
        tenant_id: Uuid,
        enrolment_ids: &[Uuid],
    ) -> Result<Vec<ClassRosterEntry>> {
        if enrolment_ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as::<_, ClassRosterEntry>(
            r#"
            SELECT enrolment.id AS enrolment_id, learner.id AS learner_id,
                   learner.learner_number, learner.display_name
              FROM enrolments AS enrolment
              JOIN learners AS learner
                ON learner.id = enrolment.learner_id
               AND learner.tenant_id = enrolment.tenant_id
             WHERE enrolment.tenant_id = $1
               AND enrolment.id = ANY($2)
             ORDER BY learner.display_name, learner.learner_number
            "#,
        )
        .bind(tenant_id)
        .bind(enrolment_ids)
        .fetch_all(pool)
        .await
        .context("Failed to resolve SIS roster identities")
    }

    /// Resolves historical register members without applying current status.
    pub async fn attendance_references_by_ids(
        pool: &PgPool,
        tenant_id: Uuid,
        enrolment_ids: &[Uuid],
    ) -> Result<Vec<AttendanceRosterEntry>> {
        Self::roster_references_by_enrolment_ids(pool, tenant_id, enrolment_ids).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
        academic_year_id: Option<Uuid>,
        class_group_id: Option<Uuid>,
        learner_id: Option<Uuid>,
    ) -> Result<(Vec<EnrolmentWithDetails>, i64)> {
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, Enrolment>(
            r#"
            SELECT enrolment.id, enrolment.tenant_id, enrolment.learner_id,
                   enrolment.academic_year_id, enrolment.class_group_id,
                   enrolment.source_application_id, enrolment.starts_on,
                   enrolment.ends_on, enrolment.status, enrolment.created_at,
                   enrolment.updated_at
            FROM enrolments AS enrolment
            JOIN learners AS learner
              ON learner.id = enrolment.learner_id AND learner.tenant_id = enrolment.tenant_id
            WHERE enrolment.tenant_id = $1 AND enrolment.deleted_at IS NULL
              AND learner.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR learner.display_name ILIKE $2
                   OR learner.learner_number ILIKE $2)
              AND ($3::TEXT IS NULL OR enrolment.status = $3)
              AND ($4::UUID IS NULL OR enrolment.academic_year_id = $4)
              AND ($5::UUID IS NULL OR enrolment.class_group_id = $5)
              AND ($6::UUID IS NULL OR enrolment.learner_id = $6)
            ORDER BY enrolment.starts_on DESC, learner.display_name
            LIMIT $7 OFFSET $8
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(academic_year_id)
        .bind(class_group_id)
        .bind(learner_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list enrolments")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM enrolments AS enrolment
            JOIN learners AS learner
              ON learner.id = enrolment.learner_id AND learner.tenant_id = enrolment.tenant_id
            WHERE enrolment.tenant_id = $1 AND enrolment.deleted_at IS NULL
              AND learner.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR learner.display_name ILIKE $2
                   OR learner.learner_number ILIKE $2)
              AND ($3::TEXT IS NULL OR enrolment.status = $3)
              AND ($4::UUID IS NULL OR enrolment.academic_year_id = $4)
              AND ($5::UUID IS NULL OR enrolment.class_group_id = $5)
              AND ($6::UUID IS NULL OR enrolment.learner_id = $6)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(academic_year_id)
        .bind(class_group_id)
        .bind(learner_id)
        .fetch_one(pool)
        .await
        .context("Failed to count enrolments")?;
        Ok((hydrate_enrolments(pool, tenant_id, rows).await?, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<EnrolmentWithDetails>> {
        let row = Self::get_record_by_id(pool, tenant_id, id).await?;
        match row {
            Some(row) => Ok(Some(hydrate_enrolment(pool, tenant_id, row).await?)),
            None => Ok(None),
        }
    }

    async fn get_record_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<Enrolment>> {
        sqlx::query_as::<_, Enrolment>(
            r#"
            SELECT id, tenant_id, learner_id, academic_year_id, class_group_id,
                   source_application_id, starts_on, ends_on, status, created_at, updated_at
            FROM enrolments
            WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load enrolment")
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        request: &CreateEnrolmentRequest,
    ) -> Result<EnrolmentWithDetails> {
        let status = request
            .status
            .unwrap_or(crate::dtos::EnrolmentStatus::Active);
        validate_enrolment_references(
            pool,
            tenant_id,
            request.learner_id,
            request.academic_year_id,
            request.class_group_id,
            request.source_application_id,
            status.as_str(),
            false,
        )
        .await?;
        let mut transaction = pool.begin().await.context("Failed to start enrolment")?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO enrolments (
                tenant_id, learner_id, academic_year_id, class_group_id,
                source_application_id, starts_on, ends_on, status
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(request.learner_id)
        .bind(request.academic_year_id)
        .bind(request.class_group_id)
        .bind(request.source_application_id)
        .bind(request.starts_on)
        .bind(request.ends_on)
        .bind(status.as_str())
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to create enrolment")?;
        if status.as_str() == "active" {
            sqlx::query(
                "UPDATE learners SET status = 'active', updated_at = NOW() WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
            )
            .bind(tenant_id)
            .bind(request.learner_id)
            .execute(&mut *transaction)
            .await
            .context("Failed to activate enrolled learner")?;
        }
        transaction
            .commit()
            .await
            .context("Failed to save enrolment")?;
        Self::get_by_id(pool, tenant_id, id)
            .await?
            .context("Created enrolment could not be reloaded")
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        request: &UpdateEnrolmentRequest,
    ) -> Result<Option<EnrolmentWithDetails>> {
        let Some(existing) = Self::get_record_by_id(pool, tenant_id, id).await? else {
            return Ok(None);
        };
        if existing.learner_id != request.learner_id {
            bail!("An enrolment cannot be moved to another learner");
        }
        validate_enrolment_references(
            pool,
            tenant_id,
            request.learner_id,
            request.academic_year_id,
            request.class_group_id,
            request.source_application_id,
            request.status.as_str(),
            existing.academic_year_id == request.academic_year_id
                && existing.class_group_id == request.class_group_id,
        )
        .await?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start enrolment update")?;
        let updated = sqlx::query(
            r#"
            UPDATE enrolments
            SET learner_id = $1, academic_year_id = $2, class_group_id = $3,
                source_application_id = $4, starts_on = $5, ends_on = $6,
                status = $7, updated_at = NOW()
            WHERE tenant_id = $8 AND id = $9 AND deleted_at IS NULL
            "#,
        )
        .bind(request.learner_id)
        .bind(request.academic_year_id)
        .bind(request.class_group_id)
        .bind(request.source_application_id)
        .bind(request.starts_on)
        .bind(request.ends_on)
        .bind(request.status.as_str())
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to update enrolment")?;
        if updated.rows_affected() == 0 {
            transaction
                .rollback()
                .await
                .context("Failed to close enrolment update")?;
            return Ok(None);
        }
        if request.status.as_str() == "active" {
            sqlx::query(
                "UPDATE learners SET status = 'active', updated_at = NOW() WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
            )
            .bind(tenant_id)
            .bind(request.learner_id)
            .execute(&mut *transaction)
            .await
            .context("Failed to activate enrolled learner")?;
        }
        transaction
            .commit()
            .await
            .context("Failed to save enrolment update")?;
        Self::get_by_id(pool, tenant_id, id).await
    }
}

async fn hydrate_applications(
    pool: &PgPool,
    tenant_id: Uuid,
    rows: Vec<Application>,
) -> Result<Vec<ApplicationWithDetails>> {
    let mut hydrated = Vec::with_capacity(rows.len());
    for row in rows {
        hydrated.push(hydrate_application(pool, tenant_id, row).await?);
    }
    Ok(hydrated)
}

async fn hydrate_application(
    pool: &PgPool,
    tenant_id: Uuid,
    row: Application,
) -> Result<ApplicationWithDetails> {
    let learner = LearnerOps::get_by_id(pool, tenant_id, row.learner_id)
        .await?
        .context("Application learner was not found")?;
    let academic_year = AcademicYearOps::get_by_id(pool, tenant_id, row.academic_year_id)
        .await?
        .context("Application academic year was not found")?;
    let target_grade_level = match row.target_grade_level_id {
        Some(id) => Some(
            AcademicGradeLevelOps::get_by_id(pool, tenant_id, id)
                .await?
                .context("Application target grade level was not found")?,
        ),
        None => None,
    };
    Ok(ApplicationWithDetails {
        id: row.id,
        tenant_id: row.tenant_id,
        application_number: row.application_number,
        learner_id: row.learner_id,
        learner_name: learner.display_name,
        learner_number: learner.learner_number,
        academic_year_id: row.academic_year_id,
        academic_year_name: academic_year.name,
        target_grade_level_id: row.target_grade_level_id,
        target_grade_level_name: target_grade_level.map(|grade| grade.name),
        submitted_on: row.submitted_on,
        status: row.status,
        notes: row.notes,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

async fn hydrate_enrolments(
    pool: &PgPool,
    tenant_id: Uuid,
    rows: Vec<Enrolment>,
) -> Result<Vec<EnrolmentWithDetails>> {
    let mut hydrated = Vec::with_capacity(rows.len());
    for row in rows {
        hydrated.push(hydrate_enrolment(pool, tenant_id, row).await?);
    }
    Ok(hydrated)
}

async fn hydrate_enrolment(
    pool: &PgPool,
    tenant_id: Uuid,
    row: Enrolment,
) -> Result<EnrolmentWithDetails> {
    let learner = LearnerOps::get_by_id(pool, tenant_id, row.learner_id)
        .await?
        .context("Enrolment learner was not found")?;
    let academic_year = AcademicYearOps::get_by_id(pool, tenant_id, row.academic_year_id)
        .await?
        .context("Enrolment academic year was not found")?;
    let class_group = ClassGroupOps::get_by_id(pool, tenant_id, row.class_group_id)
        .await?
        .context("Enrolment class was not found")?;
    let application_number = match row.source_application_id {
        Some(id) => ApplicationOps::get_record_by_id(pool, tenant_id, id)
            .await?
            .map(|application| application.application_number),
        None => None,
    };
    Ok(EnrolmentWithDetails {
        id: row.id,
        tenant_id: row.tenant_id,
        learner_id: row.learner_id,
        learner_name: learner.display_name,
        learner_number: learner.learner_number,
        academic_year_id: row.academic_year_id,
        academic_year_name: academic_year.name,
        class_group_id: row.class_group_id,
        class_group_name: class_group.name,
        source_application_id: row.source_application_id,
        application_number,
        starts_on: row.starts_on,
        ends_on: row.ends_on,
        status: row.status,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

async fn ensure_learner(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<LearnerWithAccount> {
    LearnerOps::get_by_id(pool, tenant_id, id)
        .await?
        .context("Learner was not found for this campus")
}

async fn ensure_guardian(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<GuardianWithAccount> {
    GuardianOps::get_by_id(pool, tenant_id, id)
        .await?
        .context("Guardian was not found for this campus")
}

async fn validate_academic_placement(
    pool: &PgPool,
    tenant_id: Uuid,
    academic_year_id: Uuid,
    class_group_id: Option<Uuid>,
    allow_existing_placement: bool,
) -> Result<()> {
    let academic_year = AcademicYearOps::get_by_id(pool, tenant_id, academic_year_id)
        .await?
        .context("Academic year was not found for this campus")?;
    if academic_year.status == "closed" && !allow_existing_placement {
        bail!("A closed academic year cannot receive new admissions records");
    }
    if let Some(class_group_id) = class_group_id {
        let class_group = ClassGroupOps::get_by_id(pool, tenant_id, class_group_id)
            .await?
            .context("Class was not found for this campus")?;
        if class_group.academic_year_id != academic_year_id {
            bail!("The class does not belong to the selected academic year");
        }
        if class_group.status != "active" && !allow_existing_placement {
            bail!("An inactive class cannot receive new admissions records");
        }
    }
    Ok(())
}

async fn validate_application_grade(
    pool: &PgPool,
    tenant_id: Uuid,
    academic_year_id: Uuid,
    grade_level_id: Uuid,
    allow_existing_placement: bool,
) -> Result<()> {
    validate_academic_placement(
        pool,
        tenant_id,
        academic_year_id,
        None,
        allow_existing_placement,
    )
    .await?;
    let grade_level = AcademicGradeLevelOps::get_by_id(pool, tenant_id, grade_level_id)
        .await?
        .context("Academic grade level was not found for this campus")?;
    if grade_level.status != "active" && !allow_existing_placement {
        bail!("An inactive grade level cannot receive new applications");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn validate_enrolment_references(
    pool: &PgPool,
    tenant_id: Uuid,
    learner_id: Uuid,
    academic_year_id: Uuid,
    class_group_id: Uuid,
    source_application_id: Option<Uuid>,
    status: &str,
    allow_existing_placement: bool,
) -> Result<()> {
    let learner = ensure_learner(pool, tenant_id, learner_id).await?;
    if status == "active" && matches!(learner.status.as_str(), "graduated" | "withdrawn") {
        bail!("A graduated or withdrawn learner cannot receive an active enrolment");
    }
    validate_academic_placement(
        pool,
        tenant_id,
        academic_year_id,
        Some(class_group_id),
        allow_existing_placement,
    )
    .await?;
    if let Some(application_id) = source_application_id {
        let application = ApplicationOps::get_record_by_id(pool, tenant_id, application_id)
            .await?
            .context("Source application was not found for this campus")?;
        if application.status != "accepted" {
            bail!("Only an accepted application can be used for enrolment");
        }
        let class_group = ClassGroupOps::get_by_id(pool, tenant_id, class_group_id)
            .await?
            .context("Class was not found for this campus")?;
        if application.learner_id != learner_id
            || application.academic_year_id != academic_year_id
            || application
                .target_grade_level_id
                .is_some_and(|target| class_group.grade_level_id != Some(target))
        {
            bail!("The source application does not match the selected learner and placement");
        }
    }
    Ok(())
}

fn validate_application_state(
    status: crate::dtos::ApplicationStatus,
    submitted_on: Option<chrono::NaiveDate>,
) -> Result<()> {
    if status.requires_submission_date() && submitted_on.is_none() {
        bail!("A submitted application requires a submission date");
    }
    Ok(())
}

fn validate_new_application_status(status: crate::dtos::ApplicationStatus) -> Result<()> {
    if !matches!(
        status,
        crate::dtos::ApplicationStatus::Draft | crate::dtos::ApplicationStatus::Submitted
    ) {
        bail!("A new application must be saved as a draft or submitted");
    }
    Ok(())
}

fn validate_application_transition(
    current: &str,
    next: crate::dtos::ApplicationStatus,
) -> Result<()> {
    let next = next.as_str();
    let allowed = current == next
        || matches!(
            (current, next),
            ("draft", "submitted")
                | ("submitted", "under_review" | "rejected" | "withdrawn")
                | (
                    "under_review",
                    "offered" | "accepted" | "rejected" | "withdrawn"
                )
                | ("offered", "accepted" | "rejected" | "withdrawn")
        );
    if !allowed {
        bail!("Application cannot move from {current} to {next}");
    }
    Ok(())
}

fn validate_birth_date(date_of_birth: chrono::NaiveDate) -> Result<()> {
    if date_of_birth > Utc::now().date_naive() {
        bail!("Date of birth cannot be in the future");
    }
    Ok(())
}

fn validate_guardian_contact(email: Option<&str>, phone: Option<&str>) -> Result<()> {
    if optional_text(email).is_none() && optional_text(phone).is_none() {
        bail!("A guardian requires an email address or phone number");
    }
    Ok(())
}

async fn validate_account(pool: &PgPool, tenant_id: Uuid, account_id: Option<Uuid>) -> Result<()> {
    let Some(account_id) = account_id else {
        return Ok(());
    };
    let exists = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM users
            WHERE tenant_id = $1 AND id = $2 AND is_active = TRUE AND deleted_at IS NULL
        )
        "#,
    )
    .bind(tenant_id)
    .bind(account_id)
    .fetch_one(pool)
    .await
    .context("Failed to validate account")?;
    if !exists {
        bail!("Account was not found for this campus");
    }
    Ok(())
}

fn required<'a>(label: &str, value: &'a str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{label} is required");
    }
    Ok(value)
}

fn optional_text(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn normalized_email(value: Option<&str>) -> Option<String> {
    optional_text(value).map(str::to_lowercase)
}

#[cfg(test)]
mod tests {
    use crate::dtos::ApplicationStatus;

    use super::{validate_application_transition, validate_new_application_status};

    #[test]
    fn new_applications_start_as_draft_or_submitted() {
        assert!(validate_new_application_status(ApplicationStatus::Draft).is_ok());
        assert!(validate_new_application_status(ApplicationStatus::Submitted).is_ok());
        assert!(validate_new_application_status(ApplicationStatus::Accepted).is_err());
    }

    #[test]
    fn application_transitions_move_forward_through_admissions() {
        assert!(validate_application_transition("draft", ApplicationStatus::Submitted).is_ok());
        assert!(
            validate_application_transition("submitted", ApplicationStatus::UnderReview).is_ok()
        );
        assert!(
            validate_application_transition("under_review", ApplicationStatus::Offered).is_ok()
        );
        assert!(validate_application_transition("offered", ApplicationStatus::Accepted).is_ok());
    }

    #[test]
    fn final_application_states_cannot_be_reopened_by_update() {
        assert!(
            validate_application_transition("accepted", ApplicationStatus::UnderReview).is_err()
        );
        assert!(validate_application_transition("rejected", ApplicationStatus::Submitted).is_err());
        assert!(
            validate_application_transition("withdrawn", ApplicationStatus::Submitted).is_err()
        );
    }
}
