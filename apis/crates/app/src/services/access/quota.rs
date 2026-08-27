//
//  quota.rs
//  Campus Pilot APIs
//
//  Created by Codex on 2026/08/27.
//

//! Reserves and records licensed hard-limit usage without trusting client counters.
//!
//! Product operations own meter keys. This service only enforces a hard limit
//! when the current signed lease projects the same key with `hard` enforcement.

use std::num::NonZeroU64;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use cp_common::ProductOperation;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

const MAX_RESERVATION_TTL: Duration = Duration::hours(24);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaReservationState {
    Reserved,
    Committed,
    Released,
    Expired,
}

impl TryFrom<&str> for QuotaReservationState {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "reserved" => Ok(Self::Reserved),
            "committed" => Ok(Self::Committed),
            "released" => Ok(Self::Released),
            "expired" => Ok(Self::Expired),
            other => bail!("Stored quota reservation status is invalid: {other}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaReservation {
    pub id: Uuid,
    pub state: QuotaReservationState,
    pub limit_key: String,
    pub amount: u64,
    pub period_start: DateTime<Utc>,
    pub period_end: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaDenial {
    pub limit_key: String,
    pub limit_value: u64,
    pub committed_value: u64,
    pub reserved_value: u64,
    pub requested_value: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuotaReserveOutcome {
    /// The operation declares no hard-limit key, or the current lease projects
    /// no hard-enforced definition for it.
    NotLimited,
    Reserved(QuotaReservation),
    /// The tenant-scoped idempotency key was already used. Callers must branch
    /// on the returned state and must not repeat their business mutation.
    Existing(QuotaReservation),
    Denied(QuotaDenial),
}

#[derive(Debug, FromRow)]
struct HardLimitRow {
    source_lease_id: Uuid,
    unit: String,
    period: String,
    limit_value: i64,
}

#[derive(Debug, FromRow)]
struct BucketRow {
    id: Uuid,
    period_start: DateTime<Utc>,
    period_end: Option<DateTime<Utc>>,
    committed_value: i64,
    reserved_value: i64,
}

#[derive(Debug, FromRow)]
struct ReservationRow {
    id: Uuid,
    bucket_id: Uuid,
    source_lease_id: Uuid,
    limit_key: String,
    unit: String,
    operation_key: String,
    actor_user_id: Option<Uuid>,
    amount: i64,
    status: String,
    expires_at: DateTime<Utc>,
    period_start: DateTime<Utc>,
    period_end: Option<DateTime<Utc>>,
}

pub struct QuotaOps;

impl QuotaOps {
    #[allow(clippy::too_many_arguments)]
    pub async fn reserve_operation(
        pool: &PgPool,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        operation: &ProductOperation,
        amount: NonZeroU64,
        idempotency_key: &str,
        ttl: Duration,
    ) -> Result<QuotaReserveOutcome> {
        Self::reserve_operation_at(
            pool,
            tenant_id,
            actor_user_id,
            operation,
            amount,
            idempotency_key,
            ttl,
            Utc::now(),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn reserve_operation_at(
        pool: &PgPool,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        operation: &ProductOperation,
        amount: NonZeroU64,
        idempotency_key: &str,
        ttl: Duration,
        now: DateTime<Utc>,
    ) -> Result<QuotaReserveOutcome> {
        let Some(limit_key) = operation.hard_limit_key() else {
            return Ok(QuotaReserveOutcome::NotLimited);
        };
        validate_idempotency_key(idempotency_key)?;
        if ttl < Duration::seconds(1) || ttl > MAX_RESERVATION_TTL {
            bail!("Quota reservation lifetime must be between one second and 24 hours");
        }
        let amount_i64 = i64::try_from(amount.get()).context("Quota amount is too large")?;
        let mut transaction = pool.begin().await?;

        if let Some(existing) =
            load_reservation_by_idempotency(&mut transaction, tenant_id, idempotency_key).await?
        {
            verify_replayed_reservation(&existing, operation, limit_key, amount_i64)?;
            transaction.commit().await?;
            return Ok(QuotaReserveOutcome::Existing(reservation_value(existing)?));
        }

        let limit = sqlx::query_as::<_, HardLimitRow>(
            r#"
            SELECT source_lease_id, unit, period, limit_value
            FROM entitlement_limits
            WHERE tenant_id = $1 AND limit_key = $2 AND enforcement = 'hard'
            FOR UPDATE
            "#,
        )
        .bind(tenant_id)
        .bind(limit_key)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to load the current hard limit")?;
        let Some(limit) = limit else {
            transaction.commit().await?;
            return Ok(QuotaReserveOutcome::NotLimited);
        };
        // A same-key reservation may have committed while this transaction
        // waited for the current limit row. Recheck before capacity math so a
        // concurrent retry resolves to the original reservation, not a false
        // quota denial.
        if let Some(existing) =
            load_reservation_by_idempotency(&mut transaction, tenant_id, idempotency_key).await?
        {
            verify_replayed_reservation(&existing, operation, limit_key, amount_i64)?;
            transaction.commit().await?;
            return Ok(QuotaReserveOutcome::Existing(reservation_value(existing)?));
        }
        let (period_start, period_end) = period_bounds(&limit.period, now)?;

        sqlx::query(
            r#"
            INSERT INTO entitlement_meter_buckets (
                tenant_id, limit_key, period_start, period_end
            )
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (tenant_id, limit_key, period_start) DO NOTHING
            "#,
        )
        .bind(tenant_id)
        .bind(limit_key)
        .bind(period_start)
        .bind(period_end)
        .execute(&mut *transaction)
        .await
        .context("Failed to prepare the entitlement meter bucket")?;

        let mut bucket = sqlx::query_as::<_, BucketRow>(
            r#"
            SELECT id, period_start, period_end, committed_value, reserved_value
            FROM entitlement_meter_buckets
            WHERE tenant_id = $1 AND limit_key = $2 AND period_start = $3
              AND deleted_at IS NULL
            FOR UPDATE
            "#,
        )
        .bind(tenant_id)
        .bind(limit_key)
        .bind(period_start)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to lock the entitlement meter bucket")?;

        let expired_value = sqlx::query_scalar::<_, i64>(
            r#"
            WITH expired AS (
                UPDATE entitlement_usage_reservations
                SET status = 'expired', released_at = $2, updated_at = $2
                WHERE bucket_id = $1 AND status = 'reserved' AND expires_at <= $2
                  AND deleted_at IS NULL
                RETURNING amount
            )
            SELECT COALESCE(SUM(amount), 0)::BIGINT FROM expired
            "#,
        )
        .bind(bucket.id)
        .bind(now)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to expire abandoned quota reservations")?;
        if expired_value > 0 {
            let updated = sqlx::query_scalar::<_, i64>(
                r#"
                UPDATE entitlement_meter_buckets
                SET reserved_value = reserved_value - $2, updated_at = $3
                WHERE id = $1 AND reserved_value >= $2 AND deleted_at IS NULL
                RETURNING reserved_value
                "#,
            )
            .bind(bucket.id)
            .bind(expired_value)
            .bind(now)
            .fetch_optional(&mut *transaction)
            .await?
            .context("Stored quota reservation counters are inconsistent")?;
            bucket.reserved_value = updated;
        }

        let occupied = bucket
            .committed_value
            .checked_add(bucket.reserved_value)
            .and_then(|value| value.checked_add(amount_i64))
            .context("Stored quota counters overflowed")?;
        if occupied > limit.limit_value {
            let denial = QuotaDenial {
                limit_key: limit_key.to_string(),
                limit_value: unsigned(limit.limit_value, "limit")?,
                committed_value: unsigned(bucket.committed_value, "committed counter")?,
                reserved_value: unsigned(bucket.reserved_value, "reserved counter")?,
                requested_value: amount.get(),
            };
            transaction.commit().await?;
            return Ok(QuotaReserveOutcome::Denied(denial));
        }

        let reservation_id = Uuid::new_v4();
        let expires_at = now + ttl;
        let inserted = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO entitlement_usage_reservations (
                id, tenant_id, bucket_id, source_lease_id, limit_key, unit,
                operation_key, actor_user_id, idempotency_key, amount, expires_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (tenant_id, idempotency_key) DO NOTHING
            RETURNING id
            "#,
        )
        .bind(reservation_id)
        .bind(tenant_id)
        .bind(bucket.id)
        .bind(limit.source_lease_id)
        .bind(limit_key)
        .bind(&limit.unit)
        .bind(operation.key())
        .bind(actor_user_id)
        .bind(idempotency_key)
        .bind(amount_i64)
        .bind(expires_at)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to create the quota reservation")?;

        if inserted.is_none() {
            let existing =
                load_reservation_by_idempotency(&mut transaction, tenant_id, idempotency_key)
                    .await?
                    .context("Conflicting quota idempotency key could not be reloaded")?;
            verify_replayed_reservation(&existing, operation, limit_key, amount_i64)?;
            transaction.commit().await?;
            return Ok(QuotaReserveOutcome::Existing(reservation_value(existing)?));
        }

        sqlx::query(
            r#"
            UPDATE entitlement_meter_buckets
            SET reserved_value = reserved_value + $2, updated_at = $3
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(bucket.id)
        .bind(amount_i64)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .context("Failed to increment the reserved quota counter")?;
        transaction.commit().await?;

        Ok(QuotaReserveOutcome::Reserved(QuotaReservation {
            id: reservation_id,
            state: QuotaReservationState::Reserved,
            limit_key: limit_key.to_string(),
            amount: amount.get(),
            period_start: bucket.period_start,
            period_end: bucket.period_end,
            expires_at,
        }))
    }

    pub async fn commit(
        pool: &PgPool,
        tenant_id: Uuid,
        reservation_id: Uuid,
    ) -> Result<QuotaReservation> {
        let now = Utc::now();
        let mut transaction = pool.begin().await?;
        let bucket_id = reservation_bucket_id(&mut transaction, tenant_id, reservation_id).await?;
        lock_bucket(&mut transaction, bucket_id).await?;
        let reservation = load_reservation_by_id(&mut transaction, tenant_id, reservation_id)
            .await?
            .context("Quota reservation was not found")?;
        match QuotaReservationState::try_from(reservation.status.as_str())? {
            QuotaReservationState::Committed => {
                transaction.commit().await?;
                return reservation_value(reservation);
            }
            QuotaReservationState::Released | QuotaReservationState::Expired => {
                bail!("Closed quota reservation cannot be committed");
            }
            QuotaReservationState::Reserved => {}
        }
        if reservation.expires_at <= now {
            expire_locked_reservation(&mut transaction, &reservation, now).await?;
            transaction.commit().await?;
            bail!("Expired quota reservation cannot be committed");
        }

        move_reserved_value(&mut transaction, bucket_id, reservation.amount, true, now).await?;
        sqlx::query(
            r#"
            UPDATE entitlement_usage_reservations
            SET status = 'committed', committed_at = $2, updated_at = $2
            WHERE id = $1 AND status = 'reserved'
            "#,
        )
        .bind(reservation.id)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .context("Failed to commit the quota reservation")?;
        sqlx::query(
            r#"
            INSERT INTO entitlement_usage_events (
                tenant_id, reservation_id, source_lease_id, limit_key, unit,
                operation_key, actor_user_id, amount, period_start, period_end, occurred_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (reservation_id) DO NOTHING
            "#,
        )
        .bind(tenant_id)
        .bind(reservation.id)
        .bind(reservation.source_lease_id)
        .bind(&reservation.limit_key)
        .bind(&reservation.unit)
        .bind(&reservation.operation_key)
        .bind(reservation.actor_user_id)
        .bind(reservation.amount)
        .bind(reservation.period_start)
        .bind(reservation.period_end)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .context("Failed to record committed entitlement usage")?;
        transaction.commit().await?;

        Ok(QuotaReservation {
            state: QuotaReservationState::Committed,
            ..reservation_value(reservation)?
        })
    }

    pub async fn release(
        pool: &PgPool,
        tenant_id: Uuid,
        reservation_id: Uuid,
    ) -> Result<QuotaReservation> {
        let now = Utc::now();
        let mut transaction = pool.begin().await?;
        let bucket_id = reservation_bucket_id(&mut transaction, tenant_id, reservation_id).await?;
        lock_bucket(&mut transaction, bucket_id).await?;
        let reservation = load_reservation_by_id(&mut transaction, tenant_id, reservation_id)
            .await?
            .context("Quota reservation was not found")?;
        match QuotaReservationState::try_from(reservation.status.as_str())? {
            QuotaReservationState::Released | QuotaReservationState::Expired => {
                transaction.commit().await?;
                return reservation_value(reservation);
            }
            QuotaReservationState::Committed => {
                bail!("Committed quota usage cannot be released");
            }
            QuotaReservationState::Reserved => {}
        }

        move_reserved_value(&mut transaction, bucket_id, reservation.amount, false, now).await?;
        sqlx::query(
            r#"
            UPDATE entitlement_usage_reservations
            SET status = 'released', released_at = $2, updated_at = $2
            WHERE id = $1 AND status = 'reserved'
            "#,
        )
        .bind(reservation.id)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .context("Failed to release the quota reservation")?;
        transaction.commit().await?;

        Ok(QuotaReservation {
            state: QuotaReservationState::Released,
            ..reservation_value(reservation)?
        })
    }

    pub async fn exhausted_hard_limits(pool: &PgPool, tenant_id: Uuid) -> Result<Vec<String>> {
        sqlx::query_scalar::<_, String>(
            r#"
            WITH current_limits AS (
                SELECT limit_key, limit_value,
                       CASE period
                           WHEN 'none' THEN TIMESTAMPTZ '1970-01-01 00:00:00+00'
                           WHEN 'day' THEN DATE_TRUNC('day', NOW() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC'
                           WHEN 'month' THEN DATE_TRUNC('month', NOW() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC'
                           WHEN 'year' THEN DATE_TRUNC('year', NOW() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC'
                       END AS period_start
                FROM entitlement_limits
                WHERE tenant_id = $1 AND enforcement = 'hard'
            )
            SELECT hard_limit.limit_key
            FROM current_limits AS hard_limit
            LEFT JOIN entitlement_meter_buckets AS bucket
              ON bucket.tenant_id = $1
             AND bucket.limit_key = hard_limit.limit_key
             AND bucket.period_start = hard_limit.period_start
             AND bucket.deleted_at IS NULL
            LEFT JOIN entitlement_usage_reservations AS reservation
              ON reservation.bucket_id = bucket.id
             AND reservation.status = 'reserved'
             AND reservation.expires_at > NOW()
             AND reservation.deleted_at IS NULL
            GROUP BY hard_limit.limit_key, hard_limit.limit_value, bucket.committed_value
            HAVING COALESCE(bucket.committed_value, 0)
                   + COALESCE(SUM(reservation.amount), 0) >= hard_limit.limit_value
            ORDER BY hard_limit.limit_key
            "#,
        )
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .context("Failed to load exhausted hard limits")
    }
}

fn validate_idempotency_key(value: &str) -> Result<()> {
    let value = value.trim();
    if (8..=200).contains(&value.len())
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':')
        })
    {
        Ok(())
    } else {
        bail!("Quota idempotency key has an invalid shape")
    }
}

fn period_bounds(
    period: &str,
    now: DateTime<Utc>,
) -> Result<(DateTime<Utc>, Option<DateTime<Utc>>)> {
    let date = now.date_naive();
    let start_of = |date: NaiveDate| {
        date.and_hms_opt(0, 0, 0)
            .map(|value| value.and_utc())
            .context("Quota period boundary is invalid")
    };
    match period {
        "none" => DateTime::from_timestamp(0, 0)
            .map(|start| (start, None))
            .context("Quota lifetime boundary is invalid"),
        "day" => {
            let start = start_of(date)?;
            Ok((start, Some(start + Duration::days(1))))
        }
        "month" => {
            let start_date = NaiveDate::from_ymd_opt(date.year(), date.month(), 1)
                .context("Quota month boundary is invalid")?;
            let next_date = if date.month() == 12 {
                NaiveDate::from_ymd_opt(date.year() + 1, 1, 1)
            } else {
                NaiveDate::from_ymd_opt(date.year(), date.month() + 1, 1)
            }
            .context("Quota next-month boundary is invalid")?;
            Ok((start_of(start_date)?, Some(start_of(next_date)?)))
        }
        "year" => {
            let start_date = NaiveDate::from_ymd_opt(date.year(), 1, 1)
                .context("Quota year boundary is invalid")?;
            let next_date = NaiveDate::from_ymd_opt(date.year() + 1, 1, 1)
                .context("Quota next-year boundary is invalid")?;
            Ok((start_of(start_date)?, Some(start_of(next_date)?)))
        }
        other => bail!("Stored entitlement limit period is invalid: {other}"),
    }
}

async fn load_reservation_by_idempotency(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> Result<Option<ReservationRow>> {
    sqlx::query_as::<_, ReservationRow>(
        r#"
        SELECT reservation.id, reservation.bucket_id, reservation.source_lease_id,
               reservation.limit_key, reservation.unit, reservation.operation_key,
               reservation.actor_user_id, reservation.amount, reservation.status,
               reservation.expires_at, bucket.period_start, bucket.period_end
        FROM entitlement_usage_reservations AS reservation
        INNER JOIN entitlement_meter_buckets AS bucket ON bucket.id = reservation.bucket_id
        WHERE reservation.tenant_id = $1
          AND reservation.idempotency_key = $2
          AND reservation.deleted_at IS NULL
          AND bucket.deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(idempotency_key)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to load the quota reservation")
}

async fn load_reservation_by_id(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    reservation_id: Uuid,
) -> Result<Option<ReservationRow>> {
    sqlx::query_as::<_, ReservationRow>(
        r#"
        SELECT reservation.id, reservation.bucket_id, reservation.source_lease_id,
               reservation.limit_key, reservation.unit, reservation.operation_key,
               reservation.actor_user_id, reservation.amount, reservation.status,
               reservation.expires_at, bucket.period_start, bucket.period_end
        FROM entitlement_usage_reservations AS reservation
        INNER JOIN entitlement_meter_buckets AS bucket ON bucket.id = reservation.bucket_id
        WHERE reservation.tenant_id = $1
          AND reservation.id = $2
          AND reservation.deleted_at IS NULL
          AND bucket.deleted_at IS NULL
        FOR UPDATE OF reservation
        "#,
    )
    .bind(tenant_id)
    .bind(reservation_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to load the quota reservation")
}

async fn reservation_bucket_id(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    reservation_id: Uuid,
) -> Result<Uuid> {
    sqlx::query_scalar::<_, Uuid>(
        "SELECT bucket_id FROM entitlement_usage_reservations WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .bind(reservation_id)
    .fetch_optional(&mut **transaction)
    .await?
    .context("Quota reservation was not found")
}

async fn lock_bucket(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    bucket_id: Uuid,
) -> Result<()> {
    sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM entitlement_meter_buckets WHERE id = $1 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(bucket_id)
    .fetch_optional(&mut **transaction)
    .await?
    .context("Quota reservation bucket was not found")?;
    Ok(())
}

async fn move_reserved_value(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    bucket_id: Uuid,
    amount: i64,
    commit: bool,
    now: DateTime<Utc>,
) -> Result<()> {
    let committed_increment = if commit { amount } else { 0 };
    let updated = sqlx::query_scalar::<_, Uuid>(
        r#"
        UPDATE entitlement_meter_buckets
        SET reserved_value = reserved_value - $2,
            committed_value = committed_value + $3,
            updated_at = $4
        WHERE id = $1 AND reserved_value >= $2 AND deleted_at IS NULL
        RETURNING id
        "#,
    )
    .bind(bucket_id)
    .bind(amount)
    .bind(committed_increment)
    .bind(now)
    .fetch_optional(&mut **transaction)
    .await?;
    if updated.is_none() {
        bail!("Stored quota reservation counters are inconsistent");
    }
    Ok(())
}

async fn expire_locked_reservation(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    reservation: &ReservationRow,
    now: DateTime<Utc>,
) -> Result<()> {
    move_reserved_value(
        transaction,
        reservation.bucket_id,
        reservation.amount,
        false,
        now,
    )
    .await?;
    sqlx::query(
        r#"
        UPDATE entitlement_usage_reservations
        SET status = 'expired', released_at = $2, updated_at = $2
        WHERE id = $1 AND status = 'reserved'
        "#,
    )
    .bind(reservation.id)
    .bind(now)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn verify_replayed_reservation(
    reservation: &ReservationRow,
    operation: &ProductOperation,
    limit_key: &str,
    amount: i64,
) -> Result<()> {
    if reservation.operation_key == operation.key()
        && reservation.limit_key == limit_key
        && reservation.amount == amount
    {
        Ok(())
    } else {
        bail!("Quota idempotency key was already used for a different operation")
    }
}

fn reservation_value(row: ReservationRow) -> Result<QuotaReservation> {
    Ok(QuotaReservation {
        id: row.id,
        state: QuotaReservationState::try_from(row.status.as_str())?,
        limit_key: row.limit_key,
        amount: unsigned(row.amount, "reservation amount")?,
        period_start: row.period_start,
        period_end: row.period_end,
        expires_at: row.expires_at,
    })
}

fn unsigned(value: i64, label: &str) -> Result<u64> {
    u64::try_from(value).with_context(|| format!("Stored quota {label} is negative"))
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use chrono::{Duration, TimeZone, Utc};
    use cp_common::{AgentExposure, OperationEffect, ProductOperation};
    use uuid::Uuid;

    use crate::tests::helpers::create_test_app_state;

    use super::{QuotaOps, QuotaReservationState, QuotaReserveOutcome, period_bounds};

    fn metered_operation(limit_key: &str) -> ProductOperation {
        ProductOperation::route(
            "agent.runs.execute",
            "agent",
            "agent:use",
            OperationEffect::External,
            AgentExposure::ApprovalRequired,
            true,
        )
        .consuming_hard_limit(limit_key)
    }

    async fn tenant_with_limit(limit_key: &str, value: i64) -> (sqlx::PgPool, Uuid) {
        let state = create_test_app_state().await;
        let tenant_id = Uuid::new_v4();
        sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, $2, 'Quota test')")
            .bind(tenant_id)
            .bind(format!("quota-test-{tenant_id}"))
            .execute(&state.db)
            .await
            .unwrap_or_else(|_| unreachable!());
        sqlx::query(
            r#"
            INSERT INTO entitlement_limits (
                tenant_id, limit_key, source_lease_id, unit, period, limit_value, enforcement
            )
            VALUES ($1, $2, $3, 'run', 'month', $4, 'hard')
            "#,
        )
        .bind(tenant_id)
        .bind(limit_key)
        .bind(Uuid::new_v4())
        .bind(value)
        .execute(&state.db)
        .await
        .unwrap_or_else(|_| unreachable!());
        (state.db.clone(), tenant_id)
    }

    #[test]
    fn quota_periods_are_utc_and_non_overlapping() {
        let now = Utc
            .with_ymd_and_hms(2028, 12, 31, 23, 59, 59)
            .single()
            .unwrap_or_else(|| unreachable!());
        let (day_start, day_end) = period_bounds("day", now).unwrap_or_else(|_| unreachable!());
        let (month_start, month_end) =
            period_bounds("month", now).unwrap_or_else(|_| unreachable!());
        let (year_start, year_end) = period_bounds("year", now).unwrap_or_else(|_| unreachable!());
        assert_eq!(day_start.to_rfc3339(), "2028-12-31T00:00:00+00:00");
        assert_eq!(
            day_end.map(|value| value.to_rfc3339()).as_deref(),
            Some("2029-01-01T00:00:00+00:00")
        );
        assert_eq!(month_start.to_rfc3339(), "2028-12-01T00:00:00+00:00");
        assert_eq!(month_end, day_end);
        assert_eq!(year_start.to_rfc3339(), "2028-01-01T00:00:00+00:00");
        assert_eq!(year_end, day_end);
        let (lifetime_start, lifetime_end) =
            period_bounds("none", now).unwrap_or_else(|_| unreachable!());
        assert_eq!(lifetime_start.timestamp(), 0);
        assert_eq!(lifetime_end, None);
        assert!(period_bounds("rolling", now).is_err());
        assert_eq!(
            QuotaReservationState::try_from("released").unwrap_or_else(|_| unreachable!()),
            QuotaReservationState::Released
        );
        assert_eq!(
            QuotaReservationState::try_from("expired").unwrap_or_else(|_| unreachable!()),
            QuotaReservationState::Expired
        );
        assert!(QuotaReservationState::try_from("unknown").is_err());
    }

    #[actix_web::test]
    async fn concurrent_reservations_cannot_overspend_and_commit_is_idempotent() {
        let limit_key = "agent.runs";
        let (pool, tenant_id) = tenant_with_limit(limit_key, 1).await;
        let operation = metered_operation(limit_key);
        let amount = NonZeroU64::new(1).unwrap_or_else(|| unreachable!());
        let first = QuotaOps::reserve_operation(
            &pool,
            tenant_id,
            None,
            &operation,
            amount,
            "quota-test-first",
            Duration::minutes(5),
        );
        let second = QuotaOps::reserve_operation(
            &pool,
            tenant_id,
            None,
            &operation,
            amount,
            "quota-test-second",
            Duration::minutes(5),
        );
        let (first, second) = tokio::join!(first, second);
        let outcomes = [
            ("quota-test-first", first.unwrap_or_else(|_| unreachable!())),
            (
                "quota-test-second",
                second.unwrap_or_else(|_| unreachable!()),
            ),
        ];
        let reserved = outcomes
            .iter()
            .find_map(|(key, outcome)| match outcome {
                QuotaReserveOutcome::Reserved(value) => Some((*key, value.clone())),
                _ => None,
            })
            .unwrap_or_else(|| unreachable!());
        assert_eq!(
            outcomes
                .iter()
                .filter(|(_, outcome)| matches!(outcome, QuotaReserveOutcome::Denied(_)))
                .count(),
            1
        );

        let replay = QuotaOps::reserve_operation(
            &pool,
            tenant_id,
            None,
            &operation,
            amount,
            reserved.0,
            Duration::minutes(5),
        )
        .await
        .unwrap_or_else(|_| unreachable!());
        assert!(
            matches!(replay, QuotaReserveOutcome::Existing(ref value) if value.id == reserved.1.id)
        );

        let committed = QuotaOps::commit(&pool, tenant_id, reserved.1.id)
            .await
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(committed.state, QuotaReservationState::Committed);
        let committed_again = QuotaOps::commit(&pool, tenant_id, reserved.1.id)
            .await
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(committed_again.state, QuotaReservationState::Committed);
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM entitlement_usage_events WHERE tenant_id = $1"
            )
            .bind(tenant_id)
            .fetch_one(&pool)
            .await
            .unwrap_or_default(),
            1
        );
        assert!(
            sqlx::query(
                "UPDATE entitlement_usage_events SET amount = amount + 1 WHERE tenant_id = $1"
            )
            .bind(tenant_id)
            .execute(&pool)
            .await
            .is_err()
        );
        assert_eq!(
            QuotaOps::exhausted_hard_limits(&pool, tenant_id)
                .await
                .unwrap_or_default(),
            vec![limit_key.to_string()]
        );
        assert!(
            QuotaOps::release(&pool, tenant_id, reserved.1.id)
                .await
                .is_err()
        );
    }

    #[actix_web::test]
    async fn concurrent_idempotent_retries_resolve_to_one_reservation() {
        let limit_key = "agent.runs";
        let (pool, tenant_id) = tenant_with_limit(limit_key, 1).await;
        let operation = metered_operation(limit_key);
        let amount = NonZeroU64::new(1).unwrap_or_else(|| unreachable!());
        let first = QuotaOps::reserve_operation(
            &pool,
            tenant_id,
            None,
            &operation,
            amount,
            "quota-same-request",
            Duration::minutes(5),
        );
        let second = QuotaOps::reserve_operation(
            &pool,
            tenant_id,
            None,
            &operation,
            amount,
            "quota-same-request",
            Duration::minutes(5),
        );
        let (first, second) = tokio::join!(first, second);
        let first = first.unwrap_or_else(|_| unreachable!());
        let second = second.unwrap_or_else(|_| unreachable!());
        let reservation_id = match (&first, &second) {
            (QuotaReserveOutcome::Reserved(reserved), QuotaReserveOutcome::Existing(existing))
            | (QuotaReserveOutcome::Existing(existing), QuotaReserveOutcome::Reserved(reserved)) => {
                assert_eq!(reserved.id, existing.id);
                reserved.id
            }
            _ => unreachable!(),
        };
        QuotaOps::release(&pool, tenant_id, reservation_id)
            .await
            .unwrap_or_else(|_| unreachable!());
    }

    #[actix_web::test]
    async fn released_and_expired_reservations_return_capacity() {
        let limit_key = "agent.tokens";
        let (pool, tenant_id) = tenant_with_limit(limit_key, 1).await;
        let operation = metered_operation(limit_key);
        let amount = NonZeroU64::new(1).unwrap_or_else(|| unreachable!());
        let unmetered_operation = ProductOperation::route(
            "agent.sessions.read",
            "agent",
            "agent:view",
            OperationEffect::Read,
            AgentExposure::Exposed,
            true,
        );
        assert_eq!(
            QuotaOps::reserve_operation(
                &pool,
                tenant_id,
                None,
                &unmetered_operation,
                amount,
                "quota-not-declared",
                Duration::minutes(5),
            )
            .await
            .unwrap_or_else(|_| unreachable!()),
            QuotaReserveOutcome::NotLimited
        );
        assert!(
            QuotaOps::reserve_operation(
                &pool,
                tenant_id,
                None,
                &operation,
                amount,
                "short",
                Duration::minutes(5),
            )
            .await
            .is_err()
        );
        assert!(
            QuotaOps::reserve_operation(
                &pool,
                tenant_id,
                None,
                &operation,
                amount,
                "quota-invalid-ttl",
                Duration::zero(),
            )
            .await
            .is_err()
        );
        let first = QuotaOps::reserve_operation(
            &pool,
            tenant_id,
            None,
            &operation,
            amount,
            "quota-release-first",
            Duration::minutes(5),
        )
        .await
        .unwrap_or_else(|_| unreachable!());
        let first = match first {
            QuotaReserveOutcome::Reserved(value) => value,
            _ => unreachable!(),
        };
        let released = QuotaOps::release(&pool, tenant_id, first.id)
            .await
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(released.state, QuotaReservationState::Released);
        assert_eq!(
            QuotaOps::release(&pool, tenant_id, first.id)
                .await
                .unwrap_or_else(|_| unreachable!())
                .state,
            QuotaReservationState::Released
        );
        assert!(QuotaOps::commit(&pool, tenant_id, first.id).await.is_err());
        assert!(
            QuotaOps::reserve_operation(
                &pool,
                tenant_id,
                None,
                &operation,
                NonZeroU64::new(2).unwrap_or_else(|| unreachable!()),
                "quota-release-first",
                Duration::minutes(5),
            )
            .await
            .is_err()
        );

        let second = QuotaOps::reserve_operation(
            &pool,
            tenant_id,
            None,
            &operation,
            amount,
            "quota-release-second",
            Duration::minutes(5),
        )
        .await
        .unwrap_or_else(|_| unreachable!());
        let second = match second {
            QuotaReserveOutcome::Reserved(value) => value,
            _ => unreachable!(),
        };
        sqlx::query(
            "UPDATE entitlement_usage_reservations SET expires_at = NOW() - INTERVAL '1 second' WHERE id = $1",
        )
        .bind(second.id)
        .execute(&pool)
        .await
        .unwrap_or_else(|_| unreachable!());
        assert!(QuotaOps::commit(&pool, tenant_id, second.id).await.is_err());
        assert_eq!(
            QuotaOps::release(&pool, tenant_id, second.id)
                .await
                .unwrap_or_else(|_| unreachable!())
                .state,
            QuotaReservationState::Expired
        );
        let third = QuotaOps::reserve_operation(
            &pool,
            tenant_id,
            None,
            &operation,
            amount,
            "quota-release-third",
            Duration::minutes(5),
        )
        .await
        .unwrap_or_else(|_| unreachable!());
        let third = match third {
            QuotaReserveOutcome::Reserved(value) => value,
            _ => unreachable!(),
        };
        sqlx::query(
            "UPDATE entitlement_usage_reservations SET expires_at = NOW() - INTERVAL '1 second' WHERE id = $1",
        )
        .bind(third.id)
        .execute(&pool)
        .await
        .unwrap_or_else(|_| unreachable!());
        let fourth = QuotaOps::reserve_operation(
            &pool,
            tenant_id,
            None,
            &operation,
            amount,
            "quota-cleanup-fourth",
            Duration::minutes(5),
        )
        .await
        .unwrap_or_else(|_| unreachable!());
        let fourth = match fourth {
            QuotaReserveOutcome::Reserved(value) => value,
            _ => unreachable!(),
        };
        QuotaOps::release(&pool, tenant_id, fourth.id)
            .await
            .unwrap_or_else(|_| unreachable!());

        sqlx::query(
            "UPDATE entitlement_limits SET enforcement = 'report' WHERE tenant_id = $1 AND limit_key = $2",
        )
        .bind(tenant_id)
        .bind(limit_key)
        .execute(&pool)
        .await
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            QuotaOps::reserve_operation(
                &pool,
                tenant_id,
                None,
                &operation,
                amount,
                "quota-report-only",
                Duration::minutes(5),
            )
            .await
            .unwrap_or_else(|_| unreachable!()),
            QuotaReserveOutcome::NotLimited
        );
    }
}
