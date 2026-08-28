//! Agent read adapters for Finance reference data.

use async_trait::async_trait;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::PaginationMeta;
use cp_finance::ledger::{AccountOps, CurrencyOps};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::administration::read_descriptor;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FinanceListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<String>,
    account_type: Option<String>,
    currency_mode: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum FinanceListKind {
    Currencies,
    Accounts,
}
impl FinanceListKind {
    const fn operation_key(self) -> &'static str {
        match self {
            Self::Currencies => "finance.currencies.list",
            Self::Accounts => "finance.accounts.list",
        }
    }
}

pub(super) struct FinanceListCapability {
    pool: PgPool,
    kind: FinanceListKind,
    descriptor: CapabilityDescriptor,
}
impl FinanceListCapability {
    pub(super) fn new(pool: PgPool, kind: FinanceListKind) -> Self {
        let (title, description, output_key, resource) = match kind {
            FinanceListKind::Currencies => (
                "List finance currencies",
                "Returns the campus currency register and reporting-currency designation.",
                "currencies",
                "finance.currencies",
            ),
            FinanceListKind::Accounts => (
                "List chart of accounts",
                "Returns the controlled chart-of-account structure without balances or journal data.",
                "accounts",
                "finance.accounts",
            ),
        };
        Self {
            pool,
            kind,
            descriptor: read_descriptor(
                kind.operation_key(),
                title,
                description,
                json!({
                    "page": page_schema(), "per_page": per_page_schema(), "search": search_schema(),
                    "status": { "type": ["string", "null"], "enum": ["active", "inactive", null] },
                    "account_type": { "type": ["string", "null"], "enum": ["asset", "liability", "equity", "income", "expense", null] },
                    "currency_mode": { "type": ["string", "null"], "enum": ["reporting", "single", "multi", null] }
                }),
                json!({ (output_key): { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::General,
                resource,
            ),
        }
    }
}

#[async_trait]
impl Capability for FinanceListCapability {
    type Input = FinanceListInput;
    type Output = Value;
    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, _input: &Self::Input) -> CapabilityScope {
        CapabilityScope::TenantWide
    }
    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let (page, per_page) = bounded_page(input.page, input.per_page);
        match self.kind {
            FinanceListKind::Currencies => {
                let (rows, total) = CurrencyOps::list(
                    &self.pool,
                    context.principal().tenant_id(),
                    page,
                    per_page,
                    trimmed(input.search.as_deref()),
                    trimmed(input.status.as_deref()),
                )
                .await
                .map_err(|_| dependency_failure("Finance currencies could not be loaded."))?;
                Ok(
                    json!({ "currencies": rows, "pagination": PaginationMeta::new(page as u32, per_page as u32, total) }),
                )
            }
            FinanceListKind::Accounts => {
                let (rows, total) = AccountOps::list(
                    &self.pool,
                    context.principal().tenant_id(),
                    page,
                    per_page,
                    trimmed(input.search.as_deref()),
                    trimmed(input.status.as_deref()),
                    trimmed(input.account_type.as_deref()),
                    trimmed(input.currency_mode.as_deref()),
                )
                .await
                .map_err(|_| dependency_failure("The chart of accounts could not be loaded."))?;
                Ok(
                    json!({ "accounts": rows, "pagination": PaginationMeta::new(page as u32, per_page as u32, total) }),
                )
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FinanceRecordInput {
    record_id: Uuid,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum FinanceReadKind {
    Currency,
    Account,
}
impl FinanceReadKind {
    const fn operation_key(self) -> &'static str {
        match self {
            Self::Currency => "finance.currencies.read",
            Self::Account => "finance.accounts.read",
        }
    }
}

pub(super) struct FinanceReadCapability {
    pool: PgPool,
    kind: FinanceReadKind,
    descriptor: CapabilityDescriptor,
}
impl FinanceReadCapability {
    pub(super) fn new(pool: PgPool, kind: FinanceReadKind) -> Self {
        let (title, description, resource) = match kind {
            FinanceReadKind::Currency => (
                "Read finance currency",
                "Returns one campus currency by stable identifier.",
                "finance.currencies",
            ),
            FinanceReadKind::Account => (
                "Read finance account",
                "Returns one chart-of-account record without balances or journals.",
                "finance.accounts",
            ),
        };
        Self {
            pool,
            kind,
            descriptor: read_descriptor(
                kind.operation_key(),
                title,
                description,
                json!({ "record_id": { "type": "string", "format": "uuid" } }),
                json!({ "record": { "type": "object" } }),
                DataSensitivity::General,
                resource,
            ),
        }
    }
}

#[async_trait]
impl Capability for FinanceReadCapability {
    type Input = FinanceRecordInput;
    type Output = Value;
    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        let kind = match self.kind {
            FinanceReadKind::Currency => "finance_currency",
            FinanceReadKind::Account => "finance_account",
        };
        CapabilityScope::resources([resource(kind, input.record_id)])
            .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
    }
    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let record = match self.kind {
            FinanceReadKind::Currency => {
                CurrencyOps::get_by_id(&self.pool, context.principal().tenant_id(), input.record_id)
                    .await
                    .map(|record| record.map(|value| json!(value)))
            }
            FinanceReadKind::Account => {
                AccountOps::get_by_id(&self.pool, context.principal().tenant_id(), input.record_id)
                    .await
                    .map(|record| record.map(|value| json!(value)))
            }
        }
        .map_err(|_| dependency_failure("The finance record could not be loaded."))?
        .ok_or_else(|| {
            CapabilityExecutionError::new(
                CapabilityExecutionErrorCode::InvalidState,
                "The finance record was not found.",
            )
        })?;
        Ok(json!({ "record": record }))
    }
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).max(1),
        per_page.unwrap_or(25).clamp(1, 100),
    )
}
fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}
fn resource(kind: &str, id: Uuid) -> CapabilityResource {
    CapabilityResource::parse(kind, id.to_string())
        .unwrap_or_else(|error| panic!("invalid built-in capability resource: {error}"))
}
fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}
fn page_schema() -> Value {
    json!({ "type": ["integer", "null"], "minimum": 1 })
}
fn per_page_schema() -> Value {
    json!({ "type": ["integer", "null"], "minimum": 1, "maximum": 100 })
}
fn search_schema() -> Value {
    json!({ "type": ["string", "null"], "maxLength": 200 })
}
