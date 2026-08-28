//! Agent read adapters for Finance reference data.

use async_trait::async_trait;
use chrono::NaiveDate;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::PaginationMeta;
use cp_finance::journals::JournalOps;
use cp_finance::ledger::{AccountOps, CurrencyOps};
use cp_finance::periods::{AccountingPeriodOps, FiscalYearOps};
use cp_finance::posting_requests::PostingRequestOps;
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
    FiscalYears,
}
impl FinanceListKind {
    const fn operation_key(self) -> &'static str {
        match self {
            Self::Currencies => "finance.currencies.list",
            Self::Accounts => "finance.accounts.list",
            Self::FiscalYears => "finance.fiscal_years.list",
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
            FinanceListKind::FiscalYears => (
                "List fiscal years",
                "Returns the campus fiscal years and their accounting-period lifecycle summary.",
                "fiscal_years",
                "finance.fiscal_years",
            ),
        };
        let status_schema = match kind {
            FinanceListKind::FiscalYears => {
                json!({ "type": ["string", "null"], "enum": ["draft", "open", "closed", null] })
            }
            FinanceListKind::Currencies | FinanceListKind::Accounts => {
                json!({ "type": ["string", "null"], "enum": ["active", "inactive", null] })
            }
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
                    "status": status_schema,
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
            FinanceListKind::FiscalYears => {
                let (rows, total) = FiscalYearOps::list(
                    &self.pool,
                    context.principal().tenant_id(),
                    page,
                    per_page,
                    trimmed(input.search.as_deref()),
                    trimmed(input.status.as_deref()),
                )
                .await
                .map_err(|_| dependency_failure("Fiscal years could not be loaded."))?;
                Ok(
                    json!({ "fiscal_years": rows, "pagination": PaginationMeta::new(page as u32, per_page as u32, total) }),
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
    FiscalYear,
    Journal,
    PostingRequest,
}
impl FinanceReadKind {
    const fn operation_key(self) -> &'static str {
        match self {
            Self::Currency => "finance.currencies.read",
            Self::Account => "finance.accounts.read",
            Self::FiscalYear => "finance.fiscal_years.read",
            Self::Journal => "finance.journals.read",
            Self::PostingRequest => "finance.posting_requests.read",
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
        let (title, description, resource, sensitivity) = match kind {
            FinanceReadKind::Currency => (
                "Read finance currency",
                "Returns one campus currency by stable identifier.",
                "finance.currencies",
                DataSensitivity::General,
            ),
            FinanceReadKind::Account => (
                "Read finance account",
                "Returns one chart-of-account record without balances or journals.",
                "finance.accounts",
                DataSensitivity::General,
            ),
            FinanceReadKind::FiscalYear => (
                "Read fiscal year",
                "Returns one fiscal year and its accounting-period lifecycle summary.",
                "finance.fiscal_years",
                DataSensitivity::General,
            ),
            FinanceReadKind::Journal => (
                "Read finance journal",
                "Returns one journal with its controlled lifecycle and multi-currency lines.",
                "finance.journals",
                DataSensitivity::Sensitive,
            ),
            FinanceReadKind::PostingRequest => (
                "Read finance posting request",
                "Returns one immutable operational posting request and its Finance resolution state.",
                "finance.posting_requests",
                DataSensitivity::Sensitive,
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
                sensitivity,
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
            FinanceReadKind::FiscalYear => "finance_fiscal_year",
            FinanceReadKind::Journal => "finance_journal",
            FinanceReadKind::PostingRequest => "finance_posting_request",
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
            FinanceReadKind::FiscalYear => FiscalYearOps::get_by_id(
                &self.pool,
                context.principal().tenant_id(),
                input.record_id,
            )
            .await
            .map(|record| record.map(|value| json!(value))),
            FinanceReadKind::Journal => {
                JournalOps::get_by_id(&self.pool, context.principal().tenant_id(), input.record_id)
                    .await
                    .map(|record| record.map(|value| json!(value)))
            }
            FinanceReadKind::PostingRequest => PostingRequestOps::get_by_id(
                &self.pool,
                context.principal().tenant_id(),
                input.record_id,
            )
            .await
            .map(|record| record.map(|value| json!(value))),
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FinancePostingRequestsListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<String>,
    source_module: Option<String>,
}

pub(super) struct FinancePostingRequestsListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl FinancePostingRequestsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "finance.posting_requests.list",
                "List finance posting requests",
                "Returns balanced operational requests awaiting or carrying a Finance resolution.",
                json!({
                    "page": page_schema(),
                    "per_page": per_page_schema(),
                    "search": search_schema(),
                    "status": { "type": ["string", "null"], "enum": ["pending", "converted", "rejected", "cancelled", null] },
                    "source_module": { "type": ["string", "null"], "maxLength": 64 }
                }),
                json!({ "posting_requests": { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "finance.posting_requests",
            ),
        }
    }
}

#[async_trait]
impl Capability for FinancePostingRequestsListCapability {
    type Input = FinancePostingRequestsListInput;
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
        let (posting_requests, total) = PostingRequestOps::list(
            &self.pool,
            context.principal().tenant_id(),
            page,
            per_page,
            trimmed(input.search.as_deref()),
            trimmed(input.status.as_deref()),
            trimmed(input.source_module.as_deref()),
        )
        .await
        .map_err(|_| dependency_failure("Finance posting requests could not be loaded."))?;
        Ok(json!({
            "posting_requests": posting_requests,
            "pagination": PaginationMeta::new(page as u32, per_page as u32, total)
        }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FinanceJournalsListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<String>,
    starts_on: Option<NaiveDate>,
    ends_on: Option<NaiveDate>,
}

pub(super) struct FinanceJournalsListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl FinanceJournalsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "finance.journals.list",
                "List finance journals",
                "Returns journal headers, lifecycle state, source traceability, and reporting-currency totals.",
                json!({
                    "page": page_schema(),
                    "per_page": per_page_schema(),
                    "search": search_schema(),
                    "status": { "type": ["string", "null"], "enum": ["draft", "submitted", "approved", "rejected", "posted", "reversed", null] },
                    "starts_on": { "type": ["string", "null"], "format": "date" },
                    "ends_on": { "type": ["string", "null"], "format": "date" }
                }),
                json!({ "journals": { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "finance.journals",
            ),
        }
    }
}

#[async_trait]
impl Capability for FinanceJournalsListCapability {
    type Input = FinanceJournalsListInput;
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
        let (journals, total) = JournalOps::list(
            &self.pool,
            context.principal().tenant_id(),
            page,
            per_page,
            trimmed(input.search.as_deref()),
            trimmed(input.status.as_deref()),
            input.starts_on,
            input.ends_on,
        )
        .await
        .map_err(|_| dependency_failure("Finance journals could not be loaded."))?;
        Ok(json!({
            "journals": journals,
            "pagination": PaginationMeta::new(page as u32, per_page as u32, total)
        }))
    }
}

pub(super) struct FinanceJournalValidationCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl FinanceJournalValidationCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "finance.journals.validation.read",
                "Validate finance journal",
                "Checks whether a stored journal is currently balanced and eligible for its next controlled lifecycle step.",
                json!({ "record_id": { "type": "string", "format": "uuid" } }),
                json!({ "validation": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "finance.journals.validation",
            ),
        }
    }
}

#[async_trait]
impl Capability for FinanceJournalValidationCapability {
    type Input = FinanceRecordInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        CapabilityScope::resources([resource("finance_journal", input.record_id)])
            .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let validation =
            JournalOps::validation(&self.pool, context.principal().tenant_id(), input.record_id)
                .await
                .map_err(|_| dependency_failure("The finance journal could not be validated."))?
                .ok_or_else(|| {
                    CapabilityExecutionError::new(
                        CapabilityExecutionErrorCode::InvalidState,
                        "The finance journal was not found.",
                    )
                })?;
        Ok(json!({ "validation": validation }))
    }
}

pub(super) struct FinancePeriodsCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl FinancePeriodsCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "finance.accounting_periods.list",
                "List accounting periods",
                "Returns the dated accounting periods and lifecycle state for one fiscal year.",
                json!({ "record_id": { "type": "string", "format": "uuid" } }),
                json!({ "periods": { "type": "array" } }),
                DataSensitivity::General,
                "finance.accounting_periods",
            ),
        }
    }
}

#[async_trait]
impl Capability for FinancePeriodsCapability {
    type Input = FinanceRecordInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        CapabilityScope::resources([resource("finance_fiscal_year", input.record_id)])
            .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        FiscalYearOps::get_by_id(&self.pool, context.principal().tenant_id(), input.record_id)
            .await
            .map_err(|_| dependency_failure("The fiscal year could not be loaded."))?
            .ok_or_else(|| {
                CapabilityExecutionError::new(
                    CapabilityExecutionErrorCode::InvalidState,
                    "The fiscal year was not found.",
                )
            })?;
        let periods =
            AccountingPeriodOps::list(&self.pool, context.principal().tenant_id(), input.record_id)
                .await
                .map_err(|_| dependency_failure("Accounting periods could not be loaded."))?;
        Ok(json!({ "periods": periods }))
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
