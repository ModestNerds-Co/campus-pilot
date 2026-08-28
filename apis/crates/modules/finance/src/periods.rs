//! Fiscal-year and accounting-period lifecycle for Finance.

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Days, Months, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeriodCadence {
    Monthly,
    Quarterly,
}

impl PeriodCadence {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Monthly => "monthly",
            Self::Quarterly => "quarterly",
        }
    }

    const fn months(self) -> u32 {
        match self {
            Self::Monthly => 1,
            Self::Quarterly => 3,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct FiscalYearListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateFiscalYearRequest {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub period_cadence: PeriodCadence,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateFiscalYearRequest {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct FiscalYearResponse {
    pub id: Uuid,
    pub name: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub period_cadence: String,
    pub status: String,
    pub opened_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub period_count: i64,
    pub open_period_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedFiscalYearsResponse {
    pub fiscal_years: Vec<FiscalYearResponse>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AccountingPeriodResponse {
    pub id: Uuid,
    pub fiscal_year_id: Uuid,
    pub period_number: i16,
    pub name: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub status: String,
    pub closed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AccountingPeriodsResponse {
    pub periods: Vec<AccountingPeriodResponse>,
}

#[derive(Debug, Clone)]
struct GeneratedPeriod {
    number: i16,
    name: String,
    starts_on: NaiveDate,
    ends_on: NaiveDate,
}

pub struct FiscalYearOps;

impl FiscalYearOps {
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
    ) -> Result<(Vec<FiscalYearResponse>, i64)> {
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let years = sqlx::query_as::<_, FiscalYearResponse>(&format!(
            r#"
            {} WHERE year.tenant_id = $1 AND year.deleted_at IS NULL
                 AND ($2::TEXT IS NULL OR year.name ILIKE $2)
                 AND ($3::TEXT IS NULL OR year.status = $3)
             GROUP BY year.id
             ORDER BY year.starts_on DESC, year.name
             LIMIT $4 OFFSET $5
            "#,
            fiscal_year_select()
        ))
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list fiscal years")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM finance_fiscal_years AS year
             WHERE year.tenant_id = $1 AND year.deleted_at IS NULL
               AND ($2::TEXT IS NULL OR year.name ILIKE $2)
               AND ($3::TEXT IS NULL OR year.status = $3)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .fetch_one(pool)
        .await
        .context("Failed to count fiscal years")?;
        Ok((years, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<FiscalYearResponse>> {
        sqlx::query_as::<_, FiscalYearResponse>(&format!(
            r#"
            {} WHERE year.tenant_id = $1 AND year.id = $2 AND year.deleted_at IS NULL
             GROUP BY year.id
            "#,
            fiscal_year_select()
        ))
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to read fiscal year")
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        request: &CreateFiscalYearRequest,
    ) -> Result<FiscalYearResponse> {
        let name = required(&request.name, "Fiscal year name")?;
        let periods = generate_periods(request.starts_on, request.ends_on, request.period_cadence)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start fiscal year transaction")?;
        lock_tenant(&mut transaction, tenant_id).await?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO finance_fiscal_years (
                tenant_id, name, starts_on, ends_on, period_cadence
            ) VALUES ($1, $2, $3, $4, $5)
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(name)
        .bind(request.starts_on)
        .bind(request.ends_on)
        .bind(request.period_cadence.as_str())
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to create fiscal year")?;
        for period in periods {
            sqlx::query(
                r#"
                INSERT INTO finance_accounting_periods (
                    tenant_id, fiscal_year_id, period_number, name, starts_on, ends_on
                ) VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(tenant_id)
            .bind(id)
            .bind(period.number)
            .bind(period.name)
            .bind(period.starts_on)
            .bind(period.ends_on)
            .execute(&mut *transaction)
            .await
            .context("Failed to create accounting periods")?;
        }
        transaction
            .commit()
            .await
            .context("Failed to commit fiscal year")?;
        Self::get_by_id(pool, tenant_id, id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("The fiscal year was not found after creation"))
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        request: &UpdateFiscalYearRequest,
    ) -> Result<Option<FiscalYearResponse>> {
        let name = required(&request.name, "Fiscal year name")?;
        let current = sqlx::query_scalar::<_, String>(
            "SELECT status FROM finance_fiscal_years WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to inspect fiscal year")?;
        let Some(status) = current else {
            return Ok(None);
        };
        if status != "draft" {
            bail!("Only a draft fiscal year can be edited");
        }
        sqlx::query(
            "UPDATE finance_fiscal_years SET name = $3 WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .context("Failed to update fiscal year")?;
        Self::get_by_id(pool, tenant_id, id).await
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<CalendarOutcome> {
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start fiscal year transaction")?;
        lock_tenant(&mut transaction, tenant_id).await?;
        let status = lock_fiscal_year(&mut transaction, tenant_id, id).await?;
        let Some(status) = status else {
            transaction.rollback().await.ok();
            return Ok(CalendarOutcome::NotFound);
        };
        if status != "draft" {
            bail!("Only a draft fiscal year can be removed");
        }
        sqlx::query(
            "UPDATE finance_accounting_periods SET deleted_at = NOW() WHERE tenant_id = $1 AND fiscal_year_id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to remove accounting periods")?;
        sqlx::query(
            "UPDATE finance_fiscal_years SET deleted_at = NOW() WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to remove fiscal year")?;
        transaction
            .commit()
            .await
            .context("Failed to commit fiscal year removal")?;
        Ok(CalendarOutcome::Changed)
    }

    pub async fn open(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<FiscalYearResponse>> {
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start fiscal year transaction")?;
        lock_tenant(&mut transaction, tenant_id).await?;
        let status = lock_fiscal_year(&mut transaction, tenant_id, id).await?;
        let Some(status) = status else {
            transaction.rollback().await.ok();
            return Ok(None);
        };
        if status != "draft" {
            bail!("Only a draft fiscal year can be opened");
        }
        ensure_complete_period_coverage(&mut transaction, tenant_id, id).await?;
        sqlx::query(
            "UPDATE finance_fiscal_years SET status = 'open', opened_at = NOW() WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to open fiscal year")?;
        sqlx::query(
            "UPDATE finance_accounting_periods SET status = 'open' WHERE tenant_id = $1 AND fiscal_year_id = $2 AND status = 'planned' AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to open accounting periods")?;
        transaction
            .commit()
            .await
            .context("Failed to commit fiscal year opening")?;
        Self::get_by_id(pool, tenant_id, id).await
    }

    pub async fn close(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<FiscalYearResponse>> {
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start fiscal year transaction")?;
        lock_tenant(&mut transaction, tenant_id).await?;
        let status = lock_fiscal_year(&mut transaction, tenant_id, id).await?;
        let Some(status) = status else {
            transaction.rollback().await.ok();
            return Ok(None);
        };
        if status != "open" {
            bail!("Only an open fiscal year can be closed");
        }
        let open_periods = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM finance_accounting_periods WHERE tenant_id = $1 AND fiscal_year_id = $2 AND deleted_at IS NULL AND status <> 'closed'",
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to inspect accounting periods")?;
        if open_periods > 0 {
            bail!("Every accounting period must be closed before the fiscal year can close");
        }
        sqlx::query(
            "UPDATE finance_fiscal_years SET status = 'closed', closed_at = NOW() WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to close fiscal year")?;
        transaction
            .commit()
            .await
            .context("Failed to commit fiscal year closure")?;
        Self::get_by_id(pool, tenant_id, id).await
    }
}

pub struct AccountingPeriodOps;

impl AccountingPeriodOps {
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        fiscal_year_id: Uuid,
    ) -> Result<Vec<AccountingPeriodResponse>> {
        sqlx::query_as::<_, AccountingPeriodResponse>(
            r#"
            SELECT id, fiscal_year_id, period_number, name, starts_on, ends_on, status,
                   closed_at, created_at, updated_at
              FROM finance_accounting_periods
             WHERE tenant_id = $1 AND fiscal_year_id = $2 AND deleted_at IS NULL
             ORDER BY period_number
            "#,
        )
        .bind(tenant_id)
        .bind(fiscal_year_id)
        .fetch_all(pool)
        .await
        .context("Failed to list accounting periods")
    }

    pub async fn close(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<AccountingPeriodResponse>> {
        change_period_status(pool, tenant_id, id, "closed").await
    }

    pub async fn reopen(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<AccountingPeriodResponse>> {
        change_period_status(pool, tenant_id, id, "open").await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarOutcome {
    Changed,
    NotFound,
}

async fn change_period_status(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    target: &str,
) -> Result<Option<AccountingPeriodResponse>> {
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to start accounting period transaction")?;
    lock_tenant(&mut transaction, tenant_id).await?;
    let current = sqlx::query_as::<_, (String, String)>(
        r#"
        SELECT period.status, year.status
          FROM finance_accounting_periods AS period
          JOIN finance_fiscal_years AS year
            ON year.id = period.fiscal_year_id AND year.tenant_id = period.tenant_id
         WHERE period.tenant_id = $1 AND period.id = $2
           AND period.deleted_at IS NULL AND year.deleted_at IS NULL
         FOR UPDATE OF period, year
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(&mut *transaction)
    .await
    .context("Failed to lock accounting period")?;
    let Some((current_status, year_status)) = current else {
        transaction.rollback().await.ok();
        return Ok(None);
    };
    if year_status != "open" {
        bail!("Accounting periods can change only while the fiscal year is open");
    }
    if current_status == "planned" {
        bail!("Open the fiscal year before changing its accounting periods");
    }
    if current_status != target {
        let closed_at = if target == "closed" { "NOW()" } else { "NULL" };
        sqlx::query(&format!(
            "UPDATE finance_accounting_periods SET status = $3, closed_at = {closed_at} WHERE tenant_id = $1 AND id = $2"
        ))
        .bind(tenant_id)
        .bind(id)
        .bind(target)
        .execute(&mut *transaction)
        .await
        .context("Failed to update accounting period")?;
    }
    transaction
        .commit()
        .await
        .context("Failed to commit accounting period")?;
    sqlx::query_as::<_, AccountingPeriodResponse>(
        r#"
        SELECT id, fiscal_year_id, period_number, name, starts_on, ends_on, status,
               closed_at, created_at, updated_at
          FROM finance_accounting_periods
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("Failed to read accounting period")
}

fn fiscal_year_select() -> &'static str {
    r#"
    SELECT year.id, year.name, year.starts_on, year.ends_on, year.period_cadence,
           year.status, year.opened_at, year.closed_at,
           COUNT(period.id) AS period_count,
           COUNT(period.id) FILTER (WHERE period.status = 'open') AS open_period_count,
           year.created_at, year.updated_at
      FROM finance_fiscal_years AS year
      LEFT JOIN finance_accounting_periods AS period
        ON period.fiscal_year_id = year.id
       AND period.tenant_id = year.tenant_id
       AND period.deleted_at IS NULL
    "#
}

fn generate_periods(
    starts_on: NaiveDate,
    ends_on: NaiveDate,
    cadence: PeriodCadence,
) -> Result<Vec<GeneratedPeriod>> {
    if ends_on < starts_on {
        bail!("Fiscal year end date cannot be before its start date");
    }
    if ends_on.signed_duration_since(starts_on).num_days() > 3_660 {
        bail!("A fiscal year cannot span more than ten years");
    }
    let mut periods = Vec::new();
    let mut period_start = starts_on;
    let mut number = 1_u32;
    while period_start <= ends_on {
        let months = number
            .checked_mul(cadence.months())
            .ok_or_else(|| anyhow::anyhow!("Fiscal year period count is too large"))?;
        let next_start = starts_on
            .checked_add_months(Months::new(months))
            .ok_or_else(|| anyhow::anyhow!("Fiscal year period dates are out of range"))?;
        let candidate_end = next_start
            .checked_sub_days(Days::new(1))
            .ok_or_else(|| anyhow::anyhow!("Fiscal year period dates are out of range"))?;
        let period_end = candidate_end.min(ends_on);
        let period_number = i16::try_from(number)
            .map_err(|_| anyhow::anyhow!("Fiscal year period count is too large"))?;
        periods.push(GeneratedPeriod {
            number: period_number,
            name: format!("Period {number:02}"),
            starts_on: period_start,
            ends_on: period_end,
        });
        if period_end == ends_on {
            break;
        }
        period_start = next_start;
        number += 1;
    }
    Ok(periods)
}

async fn ensure_complete_period_coverage(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    fiscal_year_id: Uuid,
) -> Result<()> {
    let complete = sqlx::query_scalar::<_, bool>(
        r#"
        WITH ordered AS (
            SELECT starts_on, ends_on,
                   LAG(ends_on) OVER (ORDER BY period_number) AS previous_end
              FROM finance_accounting_periods
             WHERE tenant_id = $1 AND fiscal_year_id = $2 AND deleted_at IS NULL
        ), year_bounds AS (
            SELECT starts_on, ends_on
              FROM finance_fiscal_years
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        )
        SELECT EXISTS (SELECT 1 FROM ordered)
           AND (SELECT MIN(starts_on) FROM ordered) = (SELECT starts_on FROM year_bounds)
           AND (SELECT MAX(ends_on) FROM ordered) = (SELECT ends_on FROM year_bounds)
           AND NOT EXISTS (
               SELECT 1 FROM ordered
                WHERE previous_end IS NOT NULL AND starts_on <> previous_end + 1
           )
        "#,
    )
    .bind(tenant_id)
    .bind(fiscal_year_id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to validate accounting period coverage")?;
    if !complete {
        bail!("Accounting periods must cover the full fiscal year without gaps");
    }
    Ok(())
}

async fn lock_fiscal_year(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<String>> {
    sqlx::query_scalar::<_, String>(
        "SELECT status FROM finance_fiscal_years WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock fiscal year")
}

async fn lock_tenant(transaction: &mut Transaction<'_, Postgres>, tenant_id: Uuid) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::TEXT, 0))")
        .bind(tenant_id)
        .execute(&mut **transaction)
        .await
        .context("Failed to lock finance configuration")?;
    Ok(())
}

fn required(value: &str, label: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{label} is required");
    }
    Ok(value.to_string())
}

#[cfg(test)]
mod tests {
    use chrono::{Days, NaiveDate};

    use super::{PeriodCadence, generate_periods};

    #[test]
    fn monthly_periods_cover_a_non_calendar_fiscal_year() {
        let periods = generate_periods(
            NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
            NaiveDate::from_ymd_opt(2027, 6, 30).unwrap(),
            PeriodCadence::Monthly,
        )
        .unwrap();
        assert_eq!(periods.len(), 12);
        assert_eq!(periods.first().unwrap().starts_on.to_string(), "2026-07-01");
        assert_eq!(periods.last().unwrap().ends_on.to_string(), "2027-06-30");
        for pair in periods.windows(2) {
            assert_eq!(
                pair[0].ends_on.checked_add_days(Days::new(1)).unwrap(),
                pair[1].starts_on
            );
        }
    }

    #[test]
    fn quarterly_periods_clip_the_final_period() {
        let periods = generate_periods(
            NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            NaiveDate::from_ymd_opt(2026, 11, 30).unwrap(),
            PeriodCadence::Quarterly,
        )
        .unwrap();
        assert_eq!(periods.len(), 4);
        assert_eq!(periods.last().unwrap().ends_on.to_string(), "2026-11-30");
    }

    #[test]
    fn reversed_dates_are_rejected() {
        assert!(
            generate_periods(
                NaiveDate::from_ymd_opt(2027, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                PeriodCadence::Monthly,
            )
            .is_err()
        );
    }
}
