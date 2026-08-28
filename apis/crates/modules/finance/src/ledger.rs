//! Multi-currency Finance references and chart-of-account structure.

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;
use validator::{Validate, ValidationError};

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordStatus {
    Active,
    Inactive,
}
impl RecordStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    Asset,
    Liability,
    Equity,
    Income,
    Expense,
}
impl AccountType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Asset => "asset",
            Self::Liability => "liability",
            Self::Equity => "equity",
            Self::Income => "income",
            Self::Expense => "expense",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurrencyMode {
    Reporting,
    Single,
    Multi,
}
impl CurrencyMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reporting => "reporting",
            Self::Single => "single",
            Self::Multi => "multi",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CurrencyListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateCurrencyRequest {
    #[validate(custom(function = "validate_currency_code"))]
    pub code: String,
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    #[validate(length(max = 8))]
    pub symbol: Option<String>,
    #[validate(range(min = 0, max = 4))]
    pub minor_units: i16,
    pub is_reporting: Option<bool>,
    pub status: Option<RecordStatus>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateCurrencyRequest {
    #[validate(custom(function = "validate_currency_code"))]
    pub code: String,
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    #[validate(length(max = 8))]
    pub symbol: Option<String>,
    #[validate(range(min = 0, max = 4))]
    pub minor_units: i16,
    pub is_reporting: bool,
    pub status: RecordStatus,
}

fn validate_currency_code(code: &str) -> std::result::Result<(), ValidationError> {
    let trimmed = code.trim();
    if trimmed.len() == 3
        && trimmed
            .chars()
            .all(|character| character.is_ascii_alphabetic())
    {
        Ok(())
    } else {
        Err(ValidationError::new("currency_code"))
    }
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CurrencyResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub symbol: Option<String>,
    pub minor_units: i16,
    pub is_reporting: bool,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedCurrenciesResponse {
    pub currencies: Vec<CurrencyResponse>,
}

pub struct CurrencyOps;
impl CurrencyOps {
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
    ) -> Result<(Vec<CurrencyResponse>, i64)> {
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, CurrencyResponse>(
            r#"
            SELECT id, code, name, symbol, minor_units, is_reporting, status, created_at, updated_at
              FROM finance_currencies
             WHERE tenant_id = $1 AND deleted_at IS NULL
               AND ($2::TEXT IS NULL OR code ILIKE $2 OR name ILIKE $2)
               AND ($3::TEXT IS NULL OR status = $3)
             ORDER BY is_reporting DESC, code ASC
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
        .context("Failed to list finance currencies")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM finance_currencies
             WHERE tenant_id = $1 AND deleted_at IS NULL
               AND ($2::TEXT IS NULL OR code ILIKE $2 OR name ILIKE $2)
               AND ($3::TEXT IS NULL OR status = $3)
        "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .fetch_one(pool)
        .await
        .context("Failed to count finance currencies")?;
        Ok((rows, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<CurrencyResponse>> {
        sqlx::query_as::<_, CurrencyResponse>(
            r#"
            SELECT id, code, name, symbol, minor_units, is_reporting, status, created_at, updated_at
              FROM finance_currencies WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to read finance currency")
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        request: &CreateCurrencyRequest,
    ) -> Result<CurrencyResponse> {
        let code = request.code.trim().to_ascii_uppercase();
        let name = required(&request.name, "Currency name")?;
        let symbol = optional(&request.symbol);
        let status = request.status.unwrap_or(RecordStatus::Active).as_str();
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start currency transaction")?;
        lock_tenant(&mut transaction, tenant_id).await?;
        let active_count = active_currency_count(&mut transaction, tenant_id).await?;
        let is_reporting = request
            .is_reporting
            .unwrap_or(active_count == 0 && status == "active");
        if is_reporting && status != "active" {
            bail!("Reporting currency must be active");
        }
        if active_count == 0 && status == "active" && !is_reporting {
            bail!("Finance requires the first active currency to be the reporting currency");
        }
        if is_reporting {
            unset_reporting_currency(&mut transaction, tenant_id, None).await?;
        }
        let row = sqlx::query_as::<_, CurrencyResponse>(r#"
            INSERT INTO finance_currencies (tenant_id, code, name, symbol, minor_units, is_reporting, status)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, code, name, symbol, minor_units, is_reporting, status, created_at, updated_at
        "#).bind(tenant_id).bind(code).bind(name).bind(symbol).bind(request.minor_units)
            .bind(is_reporting).bind(status).fetch_one(&mut *transaction).await
            .map_err(|error| finance_database_error(error, "Failed to create finance currency"))?;
        transaction
            .commit()
            .await
            .map_err(|error| finance_database_error(error, "Failed to commit finance currency"))?;
        Ok(row)
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        request: &UpdateCurrencyRequest,
    ) -> Result<Option<CurrencyResponse>> {
        let code = request.code.trim().to_ascii_uppercase();
        let name = required(&request.name, "Currency name")?;
        let symbol = optional(&request.symbol);
        let status = request.status.as_str();
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start currency transaction")?;
        lock_tenant(&mut transaction, tenant_id).await?;
        let current = sqlx::query_as::<_, (bool,)>(
            "SELECT is_reporting FROM finance_currencies WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL"
        ).bind(tenant_id).bind(id).fetch_optional(&mut *transaction).await.context("Failed to inspect finance currency")?;
        let Some((was_reporting,)) = current else {
            transaction.rollback().await.ok();
            return Ok(None);
        };
        if was_reporting && (!request.is_reporting || status != "active") {
            bail!("Choose another reporting currency before deactivating this one");
        }
        if request.is_reporting && status != "active" {
            bail!("Reporting currency must be active");
        }
        if request.is_reporting {
            unset_reporting_currency(&mut transaction, tenant_id, Some(id)).await?;
        }
        let row = sqlx::query_as::<_, CurrencyResponse>(r#"
            UPDATE finance_currencies
               SET code = $3, name = $4, symbol = $5, minor_units = $6,
                   is_reporting = $7, status = $8
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            RETURNING id, code, name, symbol, minor_units, is_reporting, status, created_at, updated_at
        "#).bind(tenant_id).bind(id).bind(code).bind(name).bind(symbol).bind(request.minor_units)
            .bind(request.is_reporting).bind(status).fetch_optional(&mut *transaction).await
            .map_err(|error| finance_database_error(error, "Failed to update finance currency"))?;
        transaction
            .commit()
            .await
            .map_err(|error| finance_database_error(error, "Failed to commit finance currency"))?;
        Ok(row)
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<DeleteOutcome> {
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start currency transaction")?;
        lock_tenant(&mut transaction, tenant_id).await?;
        let current = sqlx::query_as::<_, (bool,)>(
            "SELECT is_reporting FROM finance_currencies WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL"
        ).bind(tenant_id).bind(id).fetch_optional(&mut *transaction).await.context("Failed to inspect finance currency")?;
        let Some((is_reporting,)) = current else {
            transaction.rollback().await.ok();
            return Ok(DeleteOutcome::NotFound);
        };
        if is_reporting {
            bail!("Choose another reporting currency before removing this one");
        }
        let in_use = sqlx::query_scalar::<_, bool>(r#"
            SELECT EXISTS (SELECT 1 FROM finance_accounts WHERE tenant_id = $1 AND currency_id = $2 AND deleted_at IS NULL)
        "#).bind(tenant_id).bind(id).fetch_one(&mut *transaction).await.context("Failed to inspect currency usage")?;
        if in_use {
            transaction.rollback().await.ok();
            return Ok(DeleteOutcome::InUse);
        }
        sqlx::query(
            "UPDATE finance_currencies SET deleted_at = NOW() WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| finance_database_error(error, "Failed to remove finance currency"))?;
        transaction
            .commit()
            .await
            .map_err(|error| finance_database_error(error, "Failed to commit finance currency"))?;
        Ok(DeleteOutcome::Deleted)
    }
}

#[derive(Debug, Deserialize)]
pub struct AccountListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub account_type: Option<String>,
    pub currency_mode: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateAccountRequest {
    #[validate(length(min = 1, max = 40))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    #[validate(length(max = 1000))]
    pub description: Option<String>,
    pub account_type: AccountType,
    pub parent_account_id: Option<Uuid>,
    pub currency_mode: CurrencyMode,
    pub currency_id: Option<Uuid>,
    pub accepts_postings: bool,
    pub status: Option<RecordStatus>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateAccountRequest {
    #[validate(length(min = 1, max = 40))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    #[validate(length(max = 1000))]
    pub description: Option<String>,
    pub account_type: AccountType,
    pub parent_account_id: Option<Uuid>,
    pub currency_mode: CurrencyMode,
    pub currency_id: Option<Uuid>,
    pub accepts_postings: bool,
    pub status: RecordStatus,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AccountResponse {
    pub id: Uuid,
    pub parent_account_id: Option<Uuid>,
    pub parent_account_code: Option<String>,
    pub currency_id: Option<Uuid>,
    pub currency_code: Option<String>,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub account_type: String,
    pub normal_balance: String,
    pub currency_mode: String,
    pub accepts_postings: bool,
    pub status: String,
    pub child_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedAccountsResponse {
    pub accounts: Vec<AccountResponse>,
}

pub struct AccountOps;
impl AccountOps {
    #[allow(clippy::too_many_arguments)]
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
        account_type: Option<&str>,
        currency_mode: Option<&str>,
    ) -> Result<(Vec<AccountResponse>, i64)> {
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, AccountResponse>(&format!(
            r#"
            {} WHERE account.tenant_id = $1 AND account.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR account.code ILIKE $2 OR account.name ILIKE $2)
              AND ($3::TEXT IS NULL OR account.status = $3)
              AND ($4::TEXT IS NULL OR account.account_type = $4)
              AND ($5::TEXT IS NULL OR account.currency_mode = $5)
            GROUP BY account.id, parent.id, currency.id
            ORDER BY account.code ASC LIMIT $6 OFFSET $7
        "#,
            account_select()
        ))
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(account_type)
        .bind(currency_mode)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list finance accounts")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM finance_accounts AS account
             WHERE account.tenant_id = $1 AND account.deleted_at IS NULL
               AND ($2::TEXT IS NULL OR account.code ILIKE $2 OR account.name ILIKE $2)
               AND ($3::TEXT IS NULL OR account.status = $3)
               AND ($4::TEXT IS NULL OR account.account_type = $4)
               AND ($5::TEXT IS NULL OR account.currency_mode = $5)
        "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(account_type)
        .bind(currency_mode)
        .fetch_one(pool)
        .await
        .context("Failed to count finance accounts")?;
        Ok((rows, total))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<AccountResponse>> {
        sqlx::query_as::<_, AccountResponse>(&format!(
            r#"
            {} WHERE account.tenant_id = $1 AND account.id = $2 AND account.deleted_at IS NULL
            GROUP BY account.id, parent.id, currency.id
        "#,
            account_select()
        ))
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to read finance account")
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        request: &CreateAccountRequest,
    ) -> Result<AccountResponse> {
        ensure_reporting_currency(pool, tenant_id).await?;
        let code = required(&request.code, "Account code")?;
        let name = required(&request.name, "Account name")?;
        let description = optional(&request.description);
        let currency_id = normalized_currency_id(request.currency_mode, request.currency_id)?;
        let row = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO finance_accounts (
                tenant_id, parent_account_id, currency_id, code, name, description,
                account_type, currency_mode, accepts_postings, status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING id
        "#,
        )
        .bind(tenant_id)
        .bind(request.parent_account_id)
        .bind(currency_id)
        .bind(code)
        .bind(name)
        .bind(description)
        .bind(request.account_type.as_str())
        .bind(request.currency_mode.as_str())
        .bind(request.accepts_postings)
        .bind(request.status.unwrap_or(RecordStatus::Active).as_str())
        .fetch_one(pool)
        .await
        .map_err(|error| finance_database_error(error, "Failed to create finance account"))?;
        Self::get_by_id(pool, tenant_id, row)
            .await?
            .ok_or_else(|| anyhow!("The finance account was not found after creation"))
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        request: &UpdateAccountRequest,
    ) -> Result<Option<AccountResponse>> {
        ensure_reporting_currency(pool, tenant_id).await?;
        let code = required(&request.code, "Account code")?;
        let name = required(&request.name, "Account name")?;
        let description = optional(&request.description);
        let currency_id = normalized_currency_id(request.currency_mode, request.currency_id)?;
        let result = sqlx::query(
            r#"
            UPDATE finance_accounts SET parent_account_id = $3, currency_id = $4, code = $5,
                   name = $6, description = $7, account_type = $8, currency_mode = $9,
                   accepts_postings = $10, status = $11
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(request.parent_account_id)
        .bind(currency_id)
        .bind(code)
        .bind(name)
        .bind(description)
        .bind(request.account_type.as_str())
        .bind(request.currency_mode.as_str())
        .bind(request.accepts_postings)
        .bind(request.status.as_str())
        .execute(pool)
        .await
        .map_err(|error| finance_database_error(error, "Failed to update finance account"))?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        Self::get_by_id(pool, tenant_id, id).await
    }

    pub async fn delete(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<DeleteOutcome> {
        let exists = sqlx::query_scalar::<_, bool>(r#"
            SELECT EXISTS (SELECT 1 FROM finance_accounts WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)
        "#).bind(tenant_id).bind(id).fetch_one(pool).await.context("Failed to inspect finance account")?;
        if !exists {
            return Ok(DeleteOutcome::NotFound);
        }
        let has_children = sqlx::query_scalar::<_, bool>(r#"
            SELECT EXISTS (SELECT 1 FROM finance_accounts WHERE tenant_id = $1 AND parent_account_id = $2 AND deleted_at IS NULL)
        "#).bind(tenant_id).bind(id).fetch_one(pool).await.context("Failed to inspect child accounts")?;
        if has_children {
            return Ok(DeleteOutcome::InUse);
        }
        sqlx::query(
            "UPDATE finance_accounts SET deleted_at = NOW() WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to remove finance account")?;
        Ok(DeleteOutcome::Deleted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteOutcome {
    Deleted,
    NotFound,
    InUse,
}

fn account_select() -> &'static str {
    r#"
    SELECT account.id, account.parent_account_id, parent.code AS parent_account_code,
           account.currency_id, currency.code AS currency_code, account.code, account.name,
           account.description, account.account_type, account.normal_balance,
           account.currency_mode, account.accepts_postings, account.status,
           COUNT(child.id) AS child_count, account.created_at, account.updated_at
      FROM finance_accounts AS account
      LEFT JOIN finance_accounts AS parent
        ON parent.id = account.parent_account_id AND parent.tenant_id = account.tenant_id AND parent.deleted_at IS NULL
      LEFT JOIN finance_currencies AS currency
        ON currency.id = account.currency_id AND currency.tenant_id = account.tenant_id AND currency.deleted_at IS NULL
      LEFT JOIN finance_accounts AS child
        ON child.parent_account_id = account.id AND child.tenant_id = account.tenant_id AND child.deleted_at IS NULL
"#
}

fn required(value: &str, label: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{label} is required");
    }
    Ok(value.to_string())
}
fn optional(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
fn normalized_currency_id(mode: CurrencyMode, currency_id: Option<Uuid>) -> Result<Option<Uuid>> {
    match (mode, currency_id) {
        (CurrencyMode::Single, Some(id)) => Ok(Some(id)),
        (CurrencyMode::Single, None) => bail!("A single-currency account requires a currency"),
        (CurrencyMode::Reporting | CurrencyMode::Multi, None) => Ok(None),
        (CurrencyMode::Reporting | CurrencyMode::Multi, Some(_)) => {
            bail!("A currency is only selected for single-currency accounts")
        }
    }
}
async fn ensure_reporting_currency(pool: &PgPool, tenant_id: Uuid) -> Result<()> {
    let exists = sqlx::query_scalar::<_, bool>(r#"
        SELECT EXISTS (SELECT 1 FROM finance_currencies WHERE tenant_id = $1 AND is_reporting AND status = 'active' AND deleted_at IS NULL)
    "#).bind(tenant_id).fetch_one(pool).await.context("Failed to inspect reporting currency")?;
    if !exists {
        bail!("Finance requires an active reporting currency before accounts can be created");
    }
    Ok(())
}
async fn lock_tenant(transaction: &mut Transaction<'_, Postgres>, tenant_id: Uuid) -> Result<()> {
    let found = sqlx::query_scalar::<_, Uuid>("SELECT id FROM tenants WHERE id = $1 FOR UPDATE")
        .bind(tenant_id)
        .fetch_optional(&mut **transaction)
        .await
        .context("Failed to lock campus finance settings")?;
    if found.is_none() {
        bail!("The campus was not found");
    }
    Ok(())
}
async fn active_currency_count(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
) -> Result<i64> {
    sqlx::query_scalar::<_, i64>(r#"
        SELECT COUNT(*) FROM finance_currencies WHERE tenant_id = $1 AND status = 'active' AND deleted_at IS NULL
    "#).bind(tenant_id).fetch_one(&mut **transaction).await.context("Failed to count active finance currencies")
}
async fn unset_reporting_currency(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    except_id: Option<Uuid>,
) -> Result<()> {
    sqlx::query(r#"
        UPDATE finance_currencies SET is_reporting = FALSE
         WHERE tenant_id = $1 AND is_reporting AND deleted_at IS NULL AND ($2::UUID IS NULL OR id <> $2)
    "#).bind(tenant_id).bind(except_id).execute(&mut **transaction).await
        .context("Failed to change the reporting currency")?;
    Ok(())
}
fn finance_database_error(error: sqlx::Error, context: &'static str) -> anyhow::Error {
    if let sqlx::Error::Database(database) = &error {
        let message = database.message();
        for prefix in [
            "Finance requires",
            "A single-currency",
            "A parent",
            "A posting",
            "An account",
            "The parent",
            "Reporting currency",
        ] {
            if message.starts_with(prefix) {
                return anyhow!(message.to_string());
            }
        }
    }
    anyhow!(error).context(context)
}

#[cfg(test)]
mod tests {
    use super::{CurrencyMode, normalized_currency_id, validate_currency_code};
    use uuid::Uuid;

    #[test]
    fn currency_codes_are_three_ascii_letters() {
        assert!(validate_currency_code("USD").is_ok());
        assert!(validate_currency_code("zwg").is_ok());
        assert!(validate_currency_code("US").is_err());
        assert!(validate_currency_code("US1").is_err());
    }

    #[test]
    fn single_currency_accounts_require_one_currency() {
        let id = Uuid::new_v4();
        assert_eq!(
            normalized_currency_id(CurrencyMode::Single, Some(id)).ok(),
            Some(Some(id))
        );
        assert!(normalized_currency_id(CurrencyMode::Single, None).is_err());
        assert!(normalized_currency_id(CurrencyMode::Multi, Some(id)).is_err());
    }
}
