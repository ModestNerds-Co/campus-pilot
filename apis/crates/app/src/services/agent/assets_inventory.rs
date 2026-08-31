//! Agent read adapters for Assets and inventory catalogue and stock reads.
//!
//! These handlers call the module-owned operations and expose reduced records;
//! internal actors, idempotency identifiers, and raw Procurement data never
//! enter model-visible data.

use async_trait::async_trait;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_assets_inventory::{
    AssetStatus, GoodsReceiptAllocationOps, GoodsReceiptAllocationResponse, ItemOps, ItemResponse,
    StockBalanceOps, StockBalanceResponse, StockMovementOps, StockMovementResponse,
    StockMovementSummaryResponse, StockRequestCandidateOps, StockRequestOps, StoreOps,
    StoreResponse,
};
use cp_common::PaginationMeta;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::administration::read_descriptor;

const MAX_AGENT_GOODS_RECEIPTS_PER_PAGE: i64 = 2;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AssetsInventoryListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<AssetStatus>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum AssetsInventoryListKind {
    Items,
    Stores,
}

impl AssetsInventoryListKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::Items => "assets_inventory.items.list",
            Self::Stores => "assets_inventory.stores.list",
        }
    }
}

pub(super) struct AssetsInventoryListCapability {
    pool: PgPool,
    kind: AssetsInventoryListKind,
    descriptor: CapabilityDescriptor,
}

impl AssetsInventoryListCapability {
    pub(super) fn new(pool: PgPool, kind: AssetsInventoryListKind) -> Self {
        let (title, description, collection, resource) = match kind {
            AssetsInventoryListKind::Items => (
                "List inventory items",
                "Returns bounded item catalogue records without internal actor or idempotency identifiers.",
                "items",
                "assets_inventory.items",
            ),
            AssetsInventoryListKind::Stores => (
                "List inventory stores",
                "Returns bounded store catalogue records without internal actor or idempotency identifiers.",
                "stores",
                "assets_inventory.stores",
            ),
        };
        let output_schema = if collection == "items" {
            json!({
                "items": { "type": "array" },
                "pagination": { "type": "object" }
            })
        } else {
            json!({
                "stores": { "type": "array" },
                "pagination": { "type": "object" }
            })
        };
        Self {
            pool,
            kind,
            descriptor: read_descriptor(
                kind.operation_key(),
                title,
                description,
                json!({
                    "page": page_schema(),
                    "per_page": per_page_schema(),
                    "search": search_schema(),
                    "status": {
                        "type": ["string", "null"],
                        "enum": ["active", "inactive", null]
                    }
                }),
                output_schema,
                DataSensitivity::General,
                resource,
            ),
        }
    }
}

#[async_trait]
impl Capability for AssetsInventoryListCapability {
    type Input = AssetsInventoryListInput;
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
        let tenant_id = context.principal().tenant_id();
        match self.kind {
            AssetsInventoryListKind::Items => {
                let (items, total) = ItemOps::list(
                    &self.pool,
                    tenant_id,
                    page,
                    per_page,
                    trimmed(input.search.as_deref()),
                    input.status.map(AssetStatus::as_str),
                )
                .await
                .map_err(|_| dependency_failure("Inventory items could not be loaded."))?;
                Ok(json!({
                    "items": items.iter().map(item_projection).collect::<Vec<_>>(),
                    "pagination": PaginationMeta::new(page as u32, per_page as u32, total)
                }))
            }
            AssetsInventoryListKind::Stores => {
                let (stores, total) = StoreOps::list(
                    &self.pool,
                    tenant_id,
                    page,
                    per_page,
                    trimmed(input.search.as_deref()),
                    input.status.map(AssetStatus::as_str),
                )
                .await
                .map_err(|_| dependency_failure("Inventory stores could not be loaded."))?;
                Ok(json!({
                    "stores": stores.iter().map(store_projection).collect::<Vec<_>>(),
                    "pagination": PaginationMeta::new(page as u32, per_page as u32, total)
                }))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AssetsInventoryRecordInput {
    record_id: Uuid,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum AssetsInventoryReadKind {
    Item,
    Store,
}

impl AssetsInventoryReadKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::Item => "assets_inventory.items.read",
            Self::Store => "assets_inventory.stores.read",
        }
    }
}

pub(super) struct AssetsInventoryReadCapability {
    pool: PgPool,
    kind: AssetsInventoryReadKind,
    descriptor: CapabilityDescriptor,
}

impl AssetsInventoryReadCapability {
    pub(super) fn new(pool: PgPool, kind: AssetsInventoryReadKind) -> Self {
        let (title, description, resource) = match kind {
            AssetsInventoryReadKind::Item => (
                "Read inventory item",
                "Returns one item without internal actor or idempotency identifiers.",
                "assets_inventory.items",
            ),
            AssetsInventoryReadKind::Store => (
                "Read inventory store",
                "Returns one store without internal actor or idempotency identifiers.",
                "assets_inventory.stores",
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
impl Capability for AssetsInventoryReadCapability {
    type Input = AssetsInventoryRecordInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        let resource_kind = match self.kind {
            AssetsInventoryReadKind::Item => "assets_inventory_item",
            AssetsInventoryReadKind::Store => "assets_inventory_store",
        };
        resource_scope(resource_kind, input.record_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let tenant_id = context.principal().tenant_id();
        let record = match self.kind {
            AssetsInventoryReadKind::Item => ItemOps::get(&self.pool, tenant_id, input.record_id)
                .await
                .map_err(|_| dependency_failure("The inventory item could not be loaded."))?
                .map(|item| item_projection(&item)),
            AssetsInventoryReadKind::Store => StoreOps::get(&self.pool, tenant_id, input.record_id)
                .await
                .map_err(|_| dependency_failure("The inventory store could not be loaded."))?
                .map(|store| store_projection(&store)),
        }
        .ok_or_else(|| {
            CapabilityExecutionError::new(
                CapabilityExecutionErrorCode::InvalidState,
                "The Assets and inventory record was not found.",
            )
        })?;
        Ok(json!({ "record": record }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StockBalancesListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    item_id: Option<Uuid>,
    store_id: Option<Uuid>,
}

pub(super) struct StockBalancesListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl StockBalancesListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "assets_inventory.stock_balances.list",
                "List stock balances",
                "Returns exact on-hand quantities by item and store without internal projection versions.",
                json!({
                    "page": page_schema(),
                    "per_page": per_page_schema(),
                    "search": search_schema(),
                    "item_id": nullable_uuid_schema(),
                    "store_id": nullable_uuid_schema()
                }),
                json!({
                    "balances": { "type": "array" },
                    "pagination": { "type": "object" }
                }),
                DataSensitivity::Sensitive,
                "assets_inventory.stock_balances",
            ),
        }
    }
}

#[async_trait]
impl Capability for StockBalancesListCapability {
    type Input = StockBalancesListInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        filtered_resource_scope([
            ("assets_inventory_item", input.item_id),
            ("assets_inventory_store", input.store_id),
        ])
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let (page, per_page) = bounded_page(input.page, input.per_page);
        let (balances, total) = StockBalanceOps::list(
            &self.pool,
            context.principal().tenant_id(),
            page,
            per_page,
            trimmed(input.search.as_deref()),
            input.item_id,
            input.store_id,
        )
        .await
        .map_err(|_| dependency_failure("Stock balances could not be loaded."))?;
        Ok(json!({
            "balances": balances
                .iter()
                .map(stock_balance_projection)
                .collect::<Vec<_>>(),
            "pagination": PaginationMeta::new(page as u32, per_page as u32, total)
        }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StockMovementsListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    kind: Option<StockMovementKind>,
    item_id: Option<Uuid>,
    store_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StockMovementKind {
    ManualReceipt,
    Issue,
    Transfer,
    Adjustment,
    GoodsReceiptAllocation,
    Reversal,
}

impl StockMovementKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ManualReceipt => "manual_receipt",
            Self::Issue => "issue",
            Self::Transfer => "transfer",
            Self::Adjustment => "adjustment",
            Self::GoodsReceiptAllocation => "goods_receipt_allocation",
            Self::Reversal => "reversal",
        }
    }
}

pub(super) struct StockMovementsListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl StockMovementsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "assets_inventory.stock_movements.list",
                "List stock movements",
                "Returns immutable posted stock-movement summaries without actor or internal version identifiers.",
                json!({
                    "page": page_schema(),
                    "per_page": per_page_schema(),
                    "search": search_schema(),
                    "kind": movement_kind_schema(),
                    "item_id": nullable_uuid_schema(),
                    "store_id": nullable_uuid_schema()
                }),
                json!({
                    "movements": { "type": "array" },
                    "pagination": { "type": "object" }
                }),
                DataSensitivity::Sensitive,
                "assets_inventory.stock_movements",
            ),
        }
    }
}

#[async_trait]
impl Capability for StockMovementsListCapability {
    type Input = StockMovementsListInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        filtered_resource_scope([
            ("assets_inventory_item", input.item_id),
            ("assets_inventory_store", input.store_id),
        ])
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let (page, per_page) = bounded_page(input.page, input.per_page);
        let (movements, total) = StockMovementOps::list(
            &self.pool,
            context.principal().tenant_id(),
            page,
            per_page,
            trimmed(input.search.as_deref()),
            input.kind.map(StockMovementKind::as_str),
            input.item_id,
            input.store_id,
        )
        .await
        .map_err(|_| dependency_failure("Stock movements could not be loaded."))?;
        Ok(json!({
            "movements": movements
                .iter()
                .map(stock_movement_summary_projection)
                .collect::<Vec<_>>(),
            "pagination": PaginationMeta::new(page as u32, per_page as u32, total)
        }))
    }
}

pub(super) struct StockMovementReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl StockMovementReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "assets_inventory.stock_movements.read",
                "Read stock movement",
                "Returns one immutable posted stock movement and its exact quantity lines without actor identifiers.",
                json!({ "record_id": { "type": "string", "format": "uuid" } }),
                json!({ "record": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "assets_inventory.stock_movements",
            ),
        }
    }
}

#[async_trait]
impl Capability for StockMovementReadCapability {
    type Input = AssetsInventoryRecordInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("assets_inventory_stock_movement", input.record_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let movement =
            StockMovementOps::get(&self.pool, context.principal().tenant_id(), input.record_id)
                .await
                .map_err(|_| dependency_failure("The stock movement could not be loaded."))?
                .ok_or_else(|| {
                    CapabilityExecutionError::new(
                        CapabilityExecutionErrorCode::InvalidState,
                        "The stock movement was not found.",
                    )
                })?;
        Ok(json!({ "record": stock_movement_projection(&movement) }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GoodsReceiptAllocationsListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    goods_receipt_id: Option<Uuid>,
}

pub(super) struct GoodsReceiptAllocationsListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl GoodsReceiptAllocationsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "assets_inventory.goods_receipt_allocations.list",
                "List goods-receipt allocations",
                "Returns reduced posted-GRN allocation quantities and item mappings without supplier, delivery, or Procurement actor data.",
                json!({
                    "page": page_schema(),
                    "per_page": goods_receipt_per_page_schema(),
                    "search": search_schema(),
                    "goods_receipt_id": nullable_uuid_schema()
                }),
                json!({
                    "goods_receipts": { "type": "array" },
                    "pagination": { "type": "object" }
                }),
                DataSensitivity::Sensitive,
                "assets_inventory.goods_receipt_allocations",
            ),
        }
    }
}

#[async_trait]
impl Capability for GoodsReceiptAllocationsListCapability {
    type Input = GoodsReceiptAllocationsListInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        filtered_resource_scope([("procurement_goods_receipt", input.goods_receipt_id)])
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let (page, per_page) = bounded_page(input.page, input.per_page);
        let per_page = per_page.min(MAX_AGENT_GOODS_RECEIPTS_PER_PAGE);
        let (goods_receipts, total) = GoodsReceiptAllocationOps::list_for_agent(
            &self.pool,
            context.principal().tenant_id(),
            page,
            per_page,
            trimmed(input.search.as_deref()),
            input.goods_receipt_id,
        )
        .await
        .map_err(|_| dependency_failure("Goods-receipt allocations could not be loaded."))?;
        Ok(json!({
            "goods_receipts": goods_receipts
                .iter()
                .map(goods_receipt_allocation_projection)
                .collect::<Vec<_>>(),
            "pagination": PaginationMeta::new(page as u32, per_page as u32, total)
        }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StockRequestCandidatesInput {
    search: Option<String>,
    department_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum StockRequestCandidateKind {
    Requesters,
    Departments,
}

impl StockRequestCandidateKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::Requesters => "assets_inventory.requester_candidates.list",
            Self::Departments => "assets_inventory.department_candidates.list",
        }
    }
}

pub(super) struct StockRequestCandidatesCapability {
    pool: PgPool,
    kind: StockRequestCandidateKind,
    descriptor: CapabilityDescriptor,
}

impl StockRequestCandidatesCapability {
    pub(super) fn new(pool: PgPool, kind: StockRequestCandidateKind) -> Self {
        let (title, description, resource) = match kind {
            StockRequestCandidateKind::Requesters => (
                "List stock-request employees",
                "Returns bounded active employee references available to the stock-request workflow.",
                "assets_inventory.stock_request_requesters",
            ),
            StockRequestCandidateKind::Departments => (
                "List stock-request departments",
                "Returns bounded active department references available to the stock-request workflow.",
                "assets_inventory.stock_request_departments",
            ),
        };
        let output_schema = match kind {
            StockRequestCandidateKind::Requesters => {
                json!({ "employees": { "type": "array" } })
            }
            StockRequestCandidateKind::Departments => {
                json!({ "departments": { "type": "array" } })
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
                    "search": search_schema(),
                    "department_id": nullable_uuid_schema()
                }),
                output_schema,
                DataSensitivity::Sensitive,
                resource,
            ),
        }
    }
}

#[async_trait]
impl Capability for StockRequestCandidatesCapability {
    type Input = StockRequestCandidatesInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        filtered_resource_scope([("hr_department", input.department_id)])
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let tenant_id = context.principal().tenant_id();
        match self.kind {
            StockRequestCandidateKind::Requesters => {
                let response = StockRequestCandidateOps::requesters(
                    &self.pool,
                    tenant_id,
                    trimmed(input.search.as_deref()),
                    input.department_id,
                )
                .await
                .map_err(|_| dependency_failure("Stock-request employees could not be loaded."))?;
                serde_json::to_value(response).map_err(|_| {
                    dependency_failure("Stock-request employees could not be projected.")
                })
            }
            StockRequestCandidateKind::Departments => {
                let response = StockRequestCandidateOps::departments(
                    &self.pool,
                    tenant_id,
                    trimmed(input.search.as_deref()),
                )
                .await
                .map_err(|_| {
                    dependency_failure("Stock-request departments could not be loaded.")
                })?;
                serde_json::to_value(response).map_err(|_| {
                    dependency_failure("Stock-request departments could not be projected.")
                })
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StockRequestsListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<String>,
    requester_employee_id: Option<Uuid>,
    department_id: Option<Uuid>,
}

pub(super) struct StockRequestsListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl StockRequestsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "assets_inventory.stock_requests.list",
                "List stock requests",
                "Returns a bounded stock-request worklist with current quantities and workflow states.",
                json!({
                    "page": page_schema(),
                    "per_page": per_page_schema(),
                    "search": search_schema(),
                    "status": { "type": ["string", "null"], "maxLength": 40 },
                    "requester_employee_id": nullable_uuid_schema(),
                    "department_id": nullable_uuid_schema()
                }),
                json!({
                    "requests": { "type": "array" },
                    "pagination": { "type": "object" }
                }),
                DataSensitivity::Sensitive,
                "assets_inventory.stock_requests",
            ),
        }
    }
}

#[async_trait]
impl Capability for StockRequestsListCapability {
    type Input = StockRequestsListInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        filtered_resource_scope([
            ("hr_employee", input.requester_employee_id),
            ("hr_department", input.department_id),
        ])
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let (page, per_page) = bounded_page(input.page, input.per_page);
        let (response, total) = StockRequestOps::list(
            &self.pool,
            context.principal().tenant_id(),
            page,
            per_page,
            trimmed(input.search.as_deref()),
            trimmed(input.status.as_deref()),
            input.requester_employee_id,
            input.department_id,
        )
        .await
        .map_err(|_| dependency_failure("Stock requests could not be loaded."))?;
        Ok(json!({
            "requests": response.requests,
            "pagination": PaginationMeta::new(page as u32, per_page as u32, total)
        }))
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum StockRequestReadKind {
    Request,
    FulfilmentPreview,
}

impl StockRequestReadKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::Request => "assets_inventory.stock_requests.read",
            Self::FulfilmentPreview => "assets_inventory.stock_requests.fulfilment_preview.read",
        }
    }
}

pub(super) struct StockRequestReadCapability {
    pool: PgPool,
    kind: StockRequestReadKind,
    descriptor: CapabilityDescriptor,
}

impl StockRequestReadCapability {
    pub(super) fn new(pool: PgPool, kind: StockRequestReadKind) -> Self {
        let (title, description) = match kind {
            StockRequestReadKind::Request => (
                "Read stock request",
                "Returns one stock request with its lines, status history, and fulfilments.",
            ),
            StockRequestReadKind::FulfilmentPreview => (
                "Read stock-request fulfilment preview",
                "Returns one approved request with current positive store balances for its remaining lines.",
            ),
        };
        let output_schema = match kind {
            StockRequestReadKind::Request => json!({ "record": { "type": "object" } }),
            StockRequestReadKind::FulfilmentPreview => {
                json!({ "preview": { "type": "object" } })
            }
        };
        Self {
            pool,
            kind,
            descriptor: read_descriptor(
                kind.operation_key(),
                title,
                description,
                json!({ "record_id": { "type": "string", "format": "uuid" } }),
                output_schema,
                DataSensitivity::Sensitive,
                "assets_inventory.stock_requests",
            ),
        }
    }
}

#[async_trait]
impl Capability for StockRequestReadCapability {
    type Input = AssetsInventoryRecordInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("assets_inventory_stock_request", input.record_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let tenant_id = context.principal().tenant_id();
        match self.kind {
            StockRequestReadKind::Request => {
                let record = StockRequestOps::get(&self.pool, tenant_id, input.record_id)
                    .await
                    .map_err(|_| dependency_failure("The stock request could not be loaded."))?
                    .ok_or_else(|| {
                        CapabilityExecutionError::new(
                            CapabilityExecutionErrorCode::InvalidState,
                            "The stock request was not found.",
                        )
                    })?;
                Ok(json!({ "record": record }))
            }
            StockRequestReadKind::FulfilmentPreview => {
                let preview =
                    StockRequestOps::fulfilment_preview(&self.pool, tenant_id, input.record_id)
                        .await
                        .map_err(|_| {
                            dependency_failure(
                                "The stock-request fulfilment preview could not be loaded.",
                            )
                        })?
                        .ok_or_else(|| {
                            CapabilityExecutionError::new(
                                CapabilityExecutionErrorCode::InvalidState,
                                "The stock request was not found.",
                            )
                        })?;
                Ok(json!({ "preview": preview }))
            }
        }
    }
}

fn item_projection(item: &ItemResponse) -> Value {
    json!({
        "id": item.id,
        "item_number": item.item_number,
        "name": item.name,
        "description": item.description,
        "barcode": item.barcode,
        "unit_label": item.unit_label,
        "quantity_scale": item.quantity_scale,
        "reorder_level_minor": item.reorder_level_minor,
        "status": item.status,
        "version": item.version,
        "created_at": item.created_at,
        "updated_at": item.updated_at
    })
}

fn store_projection(store: &StoreResponse) -> Value {
    json!({
        "id": store.id,
        "store_number": store.store_number,
        "name": store.name,
        "location_label": store.location_label,
        "notes": store.notes,
        "status": store.status,
        "version": store.version,
        "created_at": store.created_at,
        "updated_at": store.updated_at
    })
}

fn stock_balance_projection(balance: &StockBalanceResponse) -> Value {
    json!({
        "item_id": balance.item_id,
        "item_number": balance.item_number,
        "item_name": balance.item_name,
        "store_id": balance.store_id,
        "store_number": balance.store_number,
        "store_name": balance.store_name,
        "on_hand_minor": balance.on_hand_minor,
        "quantity_scale": balance.quantity_scale,
        "unit_label": balance.unit_label,
        "updated_at": balance.updated_at
    })
}

fn stock_movement_summary_projection(movement: &StockMovementSummaryResponse) -> Value {
    json!({
        "id": movement.id,
        "movement_number": movement.movement_number,
        "kind": movement.kind,
        "effective_on": movement.effective_on,
        "reference": movement.reference,
        "reason": movement.reason,
        "source_goods_receipt_id": movement.source_goods_receipt_id,
        "source_goods_receipt_number": movement.source_goods_receipt_number,
        "reverses_movement_id": movement.reverses_movement_id,
        "reverses_movement_number": movement.reverses_movement_number,
        "reversed_by_movement_id": movement.reversed_by_movement_id,
        "reversed_by_movement_number": movement.reversed_by_movement_number,
        "status": movement.status,
        "line_count": movement.line_count,
        "posted_at": movement.posted_at,
        "created_at": movement.created_at
    })
}

fn stock_movement_projection(movement: &StockMovementResponse) -> Value {
    json!({
        "summary": stock_movement_summary_projection(&movement.summary),
        "lines": movement.lines.iter().map(|line| json!({
            "line_number": line.line_number,
            "item_id": line.item_id,
            "item_number": line.item_number,
            "item_name": line.item_name,
            "store_id": line.store_id,
            "store_number": line.store_number,
            "store_name": line.store_name,
            "quantity_delta_minor": line.quantity_delta_minor,
            "quantity_scale": line.quantity_scale,
            "unit_label": line.unit_label,
            "on_hand_before_minor": line.on_hand_before_minor,
            "on_hand_after_minor": line.on_hand_after_minor,
            "source_goods_receipt_line_number": line.source_goods_receipt_line_number,
            "source_goods_receipt_description": line.source_goods_receipt_description
        })).collect::<Vec<_>>()
    })
}

fn goods_receipt_allocation_projection(receipt: &GoodsReceiptAllocationResponse) -> Value {
    json!({
        "goods_receipt_id": receipt.id,
        "goods_receipt_number": receipt.goods_receipt_number,
        "purchase_order_number": receipt.purchase_order_number,
        "received_on": receipt.received_on,
        "lines": receipt.lines.iter().map(|line| json!({
            "line_number": line.line_number,
            "description": line.description,
            "unit_label": line.unit_label,
            "quantity_minor": line.quantity_minor,
            "quantity_scale": line.quantity_scale,
            "allocated_quantity_minor": line.allocated_quantity_minor,
            "remaining_quantity_minor": line.remaining_quantity_minor,
            "mapped_item_id": line.mapped_item_id,
            "mapped_item_number": line.mapped_item_number,
            "mapped_item_name": line.mapped_item_name
        })).collect::<Vec<_>>()
    })
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).clamp(1, 1_000_000),
        per_page.unwrap_or(25).clamp(1, 100),
    )
}

fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn resource_scope(kind: &str, id: Uuid) -> CapabilityScope {
    CapabilityScope::resources([CapabilityResource::parse(kind, id.to_string())
        .unwrap_or_else(|error| panic!("invalid built-in capability resource: {error}"))])
    .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
}

fn filtered_resource_scope<const N: usize>(filters: [(&str, Option<Uuid>); N]) -> CapabilityScope {
    let resources = filters
        .into_iter()
        .filter_map(|(kind, id)| {
            id.map(|id| {
                CapabilityResource::parse(kind, id.to_string())
                    .unwrap_or_else(|error| panic!("invalid built-in capability resource: {error}"))
            })
        })
        .collect::<Vec<_>>();
    if resources.is_empty() {
        CapabilityScope::TenantWide
    } else {
        CapabilityScope::resources(resources)
            .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
    }
}

fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}

fn page_schema() -> Value {
    json!({ "type": ["integer", "null"], "minimum": 1, "maximum": 1_000_000 })
}

fn per_page_schema() -> Value {
    json!({ "type": ["integer", "null"], "minimum": 1, "maximum": 100 })
}

fn goods_receipt_per_page_schema() -> Value {
    json!({
        "type": ["integer", "null"],
        "minimum": 1,
        "maximum": MAX_AGENT_GOODS_RECEIPTS_PER_PAGE
    })
}

fn search_schema() -> Value {
    json!({ "type": ["string", "null"], "maxLength": 200 })
}

fn nullable_uuid_schema() -> Value {
    json!({ "type": ["string", "null"], "format": "uuid" })
}

fn movement_kind_schema() -> Value {
    json!({
        "type": ["string", "null"],
        "enum": [
            "manual_receipt",
            "issue",
            "transfer",
            "adjustment",
            "goods_receipt_allocation",
            "reversal",
            null
        ]
    })
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, Utc};
    use cp_agent::{Capability, CapabilityScope, DataSensitivity};
    use cp_assets_inventory::{
        GoodsReceiptAllocationLineResponse, GoodsReceiptAllocationResponse, ItemResponse,
        StockBalanceResponse, StockMovementLineResponse, StockMovementResponse,
        StockMovementSummaryResponse, StoreResponse,
    };
    use serde_json::json;
    use sqlx::postgres::PgPoolOptions;
    use uuid::Uuid;

    use super::{
        GoodsReceiptAllocationsListCapability, GoodsReceiptAllocationsListInput,
        MAX_AGENT_GOODS_RECEIPTS_PER_PAGE, StockBalancesListCapability, StockBalancesListInput,
        StockMovementReadCapability, StockMovementsListCapability, StockMovementsListInput,
        bounded_page, goods_receipt_allocation_projection, item_projection,
        stock_balance_projection, stock_movement_projection, store_projection, trimmed,
    };

    #[test]
    fn list_inputs_are_bounded_and_trimmed_before_domain_reads() {
        assert_eq!(bounded_page(None, None), (1, 25));
        assert_eq!(bounded_page(Some(-4), Some(900)), (1, 100));
        assert_eq!(bounded_page(Some(i64::MAX), Some(25)), (1_000_000, 25));
        assert_eq!(trimmed(Some("  science  ")), Some("science"));
        assert_eq!(trimmed(Some("   ")), None);
    }

    #[test]
    fn projections_omit_actor_and_idempotency_identifiers() {
        let now = Utc::now();
        let item = ItemResponse {
            id: Uuid::new_v4(),
            item_number: "ITM-000001".to_string(),
            name: "Science beaker".to_string(),
            description: None,
            barcode: Some("BEAKER-001".to_string()),
            unit_label: "each".to_string(),
            quantity_scale: 0,
            reorder_level_minor: Some(10),
            status: "active".to_string(),
            version: 1,
            created_by: Uuid::new_v4(),
            updated_by: Uuid::new_v4(),
            created_at: now,
            updated_at: now,
        };
        let item_json = item_projection(&item);
        for omitted in ["created_by", "updated_by", "idempotency_key"] {
            assert!(item_json.get(omitted).is_none(), "{omitted}");
        }
        assert_eq!(item_json["item_number"], "ITM-000001");

        let store = StoreResponse {
            id: Uuid::new_v4(),
            store_number: "STR-000001".to_string(),
            name: "Main store".to_string(),
            location_label: Some("Block A".to_string()),
            notes: None,
            status: "active".to_string(),
            version: 1,
            created_by: Uuid::new_v4(),
            updated_by: Uuid::new_v4(),
            created_at: now,
            updated_at: now,
        };
        let store_json = store_projection(&store);
        for omitted in ["created_by", "updated_by", "idempotency_key"] {
            assert!(store_json.get(omitted).is_none(), "{omitted}");
        }
        assert_eq!(store_json["store_number"], "STR-000001");
    }

    #[tokio::test]
    async fn stock_read_descriptors_are_explicitly_sensitive() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            StockBalancesListCapability::new(pool.clone())
                .descriptor()
                .policy()
                .data_sensitivity(),
            DataSensitivity::Sensitive
        );
        assert_eq!(
            StockMovementsListCapability::new(pool.clone())
                .descriptor()
                .policy()
                .data_sensitivity(),
            DataSensitivity::Sensitive
        );
        assert_eq!(
            StockMovementReadCapability::new(pool.clone())
                .descriptor()
                .policy()
                .data_sensitivity(),
            DataSensitivity::Sensitive
        );
        assert_eq!(
            GoodsReceiptAllocationsListCapability::new(pool)
                .descriptor()
                .policy()
                .data_sensitivity(),
            DataSensitivity::Sensitive
        );
    }

    #[tokio::test]
    async fn stock_list_filters_narrow_capability_scopes() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
            .unwrap_or_else(|_| unreachable!());
        let item_id = Uuid::new_v4();
        let store_id = Uuid::new_v4();
        let item_id_string = item_id.to_string();
        let store_id_string = store_id.to_string();
        let balance_capability = StockBalancesListCapability::new(pool.clone());
        let balance_scope = balance_capability.scope(&StockBalancesListInput {
            page: None,
            per_page: None,
            search: None,
            item_id: Some(item_id),
            store_id: Some(store_id),
        });
        let CapabilityScope::Resources(resources) = balance_scope else {
            panic!("filtered balance read must use resource scope");
        };
        assert_eq!(
            resources
                .values()
                .iter()
                .map(|resource| (resource.kind(), resource.id()))
                .collect::<Vec<_>>(),
            vec![
                ("assets_inventory_item", item_id_string.as_str()),
                ("assets_inventory_store", store_id_string.as_str()),
            ]
        );

        let movement_capability = StockMovementsListCapability::new(pool.clone());
        let movement_scope = movement_capability.scope(&StockMovementsListInput {
            page: None,
            per_page: None,
            search: None,
            kind: None,
            item_id: Some(item_id),
            store_id: None,
        });
        assert_eq!(
            movement_scope
                .primary_resource()
                .map(|resource| (resource.kind(), resource.id())),
            Some(("assets_inventory_item", item_id_string.as_str()))
        );

        let receipt_id = Uuid::new_v4();
        let receipt_id_string = receipt_id.to_string();
        let receipt_capability = GoodsReceiptAllocationsListCapability::new(pool);
        let receipt_scope = receipt_capability.scope(&GoodsReceiptAllocationsListInput {
            page: None,
            per_page: None,
            search: None,
            goods_receipt_id: Some(receipt_id),
        });
        assert_eq!(
            receipt_scope
                .primary_resource()
                .map(|resource| (resource.kind(), resource.id())),
            Some(("procurement_goods_receipt", receipt_id_string.as_str()))
        );

        assert_eq!(
            balance_capability.scope(&StockBalancesListInput {
                page: None,
                per_page: None,
                search: None,
                item_id: None,
                store_id: None,
            }),
            CapabilityScope::TenantWide
        );
    }

    #[test]
    fn stock_projections_omit_internal_and_raw_procurement_fields() {
        let now = Utc::now();
        let balance = StockBalanceResponse {
            item_id: Uuid::new_v4(),
            item_number: "ITM-000001".to_string(),
            item_name: "Exercise book".to_string(),
            store_id: Uuid::new_v4(),
            store_number: "STR-000001".to_string(),
            store_name: "Main store".to_string(),
            on_hand_minor: 125,
            quantity_scale: 0,
            unit_label: "each".to_string(),
            version: 7,
            updated_at: now,
        };
        let balance_json = stock_balance_projection(&balance);
        assert!(balance_json.get("version").is_none());
        assert_eq!(balance_json["on_hand_minor"], 125);

        let movement = StockMovementResponse {
            summary: StockMovementSummaryResponse {
                id: Uuid::new_v4(),
                movement_number: "MOV-000001".to_string(),
                kind: "goods_receipt_allocation".to_string(),
                effective_on: NaiveDate::from_ymd_opt(2026, 8, 28)
                    .unwrap_or_else(|| unreachable!()),
                reference: None,
                reason: Some("Initial allocation".to_string()),
                source_goods_receipt_id: Some(Uuid::new_v4()),
                source_goods_receipt_number: Some("GRN-000001".to_string()),
                reverses_movement_id: None,
                reverses_movement_number: None,
                reversed_by_movement_id: None,
                reversed_by_movement_number: None,
                status: "posted".to_string(),
                version: 3,
                line_count: 1,
                created_by: Uuid::new_v4(),
                posted_by: Uuid::new_v4(),
                posted_at: now,
                created_at: now,
            },
            lines: vec![StockMovementLineResponse {
                id: Uuid::new_v4(),
                line_number: 1,
                item_id: balance.item_id,
                item_number: balance.item_number.clone(),
                item_name: balance.item_name.clone(),
                store_id: balance.store_id,
                store_number: balance.store_number.clone(),
                store_name: balance.store_name.clone(),
                quantity_delta_minor: 25,
                quantity_scale: 0,
                unit_label: "each".to_string(),
                on_hand_before_minor: 100,
                on_hand_after_minor: 125,
                source_goods_receipt_line_id: Some(Uuid::new_v4()),
                source_goods_receipt_line_number: Some(1),
                source_goods_receipt_description: Some("Exercise book".to_string()),
            }],
        };
        let movement_json = stock_movement_projection(&movement);
        let summary = &movement_json["summary"];
        for omitted in ["created_by", "posted_by", "version", "idempotency_key"] {
            assert!(summary.get(omitted).is_none(), "{omitted}");
        }
        assert!(movement_json["lines"][0].get("id").is_none());
        assert!(
            movement_json["lines"][0]
                .get("source_goods_receipt_line_id")
                .is_none()
        );

        let receipt = GoodsReceiptAllocationResponse {
            id: Uuid::new_v4(),
            goods_receipt_number: "GRN-000001".to_string(),
            purchase_order_id: Uuid::new_v4(),
            purchase_order_number: "PO-000001".to_string(),
            supplier_id: Uuid::new_v4(),
            supplier_number: "SUP-000001".to_string(),
            supplier_name: "Private supplier".to_string(),
            received_on: NaiveDate::from_ymd_opt(2026, 8, 27).unwrap_or_else(|| unreachable!()),
            delivery_reference: Some("private-delivery-reference".to_string()),
            lines: vec![GoodsReceiptAllocationLineResponse {
                id: Uuid::new_v4(),
                line_number: 1,
                description: "Exercise book".to_string(),
                unit_label: Some("each".to_string()),
                quantity_minor: 100,
                quantity_scale: 0,
                allocated_quantity_minor: 25,
                remaining_quantity_minor: 75,
                mapped_item_id: Some(balance.item_id),
                mapped_item_number: Some(balance.item_number),
                mapped_item_name: Some(balance.item_name),
            }],
        };
        let receipt_json = goods_receipt_allocation_projection(&receipt);
        for omitted in [
            "purchase_order_id",
            "supplier_id",
            "supplier_number",
            "supplier_name",
            "delivery_reference",
            "idempotency_key",
        ] {
            assert!(receipt_json.get(omitted).is_none(), "{omitted}");
        }
        assert!(receipt_json["lines"][0].get("id").is_none());
        assert_eq!(receipt_json["goods_receipt_number"], "GRN-000001");
    }

    #[test]
    fn goods_receipt_agent_page_has_a_bounded_maximum_projection() {
        let description = "x".repeat(500);
        let receipts = (1..=MAX_AGENT_GOODS_RECEIPTS_PER_PAGE)
            .map(|receipt_number| GoodsReceiptAllocationResponse {
                id: Uuid::new_v4(),
                goods_receipt_number: format!("GRN-{receipt_number:06}"),
                purchase_order_id: Uuid::new_v4(),
                purchase_order_number: format!("PO-{receipt_number:06}"),
                supplier_id: Uuid::new_v4(),
                supplier_number: "SUP-999999".to_string(),
                supplier_name: "Hidden supplier".to_string(),
                received_on: NaiveDate::from_ymd_opt(2026, 8, 27).unwrap_or_else(|| unreachable!()),
                delivery_reference: Some("hidden-delivery-reference".to_string()),
                lines: (1..=200)
                    .map(|line_number| GoodsReceiptAllocationLineResponse {
                        id: Uuid::new_v4(),
                        line_number,
                        description: description.clone(),
                        unit_label: Some("each".to_string()),
                        quantity_minor: 1,
                        quantity_scale: 0,
                        allocated_quantity_minor: 0,
                        remaining_quantity_minor: 1,
                        mapped_item_id: None,
                        mapped_item_number: None,
                        mapped_item_name: None,
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        let projection = json!(
            receipts
                .iter()
                .map(goods_receipt_allocation_projection)
                .collect::<Vec<_>>()
        );
        let line_count = projection
            .as_array()
            .into_iter()
            .flatten()
            .map(|receipt| {
                receipt["lines"]
                    .as_array()
                    .map(Vec::len)
                    .unwrap_or_default()
            })
            .sum::<usize>();
        assert_eq!(line_count, 400);
        assert!(
            serde_json::to_vec(&projection)
                .unwrap_or_else(|error| panic!("serialize maximum projection: {error}"))
                .len()
                < 320_000
        );
    }
}
