//! Agent read adapters for Procurement suppliers and requisitions.
//!
//! These handlers call Procurement-owned operations and emit reduced
//! projections. HR login links and internal mutation-actor identifiers never
//! enter model-visible results.

use async_trait::async_trait;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::PaginationMeta;
use cp_hr_payroll::models::EmployeeReference;
use cp_procurement::{
    goods_receipts::{GoodsReceiptOps, GoodsReceiptResponse, GoodsReceiptSummary},
    purchase_orders::{PurchaseOrderOps, PurchaseOrderResponse, PurchaseOrderSummary},
    requisitions::{
        ProcurementReferenceOps, RequisitionOps, RequisitionResponse, RequisitionSummary,
    },
    suppliers::{SupplierOps, SupplierResponse},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::administration::read_descriptor;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EmptyInput {}

pub(super) struct ProcurementReferenceDataCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl ProcurementReferenceDataCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "procurement.reference_data.read",
                "Read Procurement reference data",
                "Returns active Finance-owned currencies available for Procurement requests.",
                json!({}),
                json!({ "currencies": { "type": "array" } }),
                DataSensitivity::General,
                "procurement.reference_data",
            ),
        }
    }
}

#[async_trait]
impl Capability for ProcurementReferenceDataCapability {
    type Input = EmptyInput;
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
        _input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let references =
            ProcurementReferenceOps::currencies(&self.pool, context.principal().tenant_id())
                .await
                .map_err(|_| dependency_failure("Procurement currencies could not be loaded."))?;
        Ok(json!({ "currencies": references.currencies }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RequesterCandidatesInput {
    search: Option<String>,
}

pub(super) struct ProcurementRequesterCandidatesCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl ProcurementRequesterCandidatesCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "procurement.requester_candidates.list",
                "List requisition requester candidates",
                "Returns a bounded minimum-field projection of active HR employees eligible to request supplies.",
                json!({ "search": search_schema() }),
                json!({ "employees": { "type": "array" } }),
                DataSensitivity::Personal,
                "procurement.requester_candidates",
            ),
        }
    }
}

#[async_trait]
impl Capability for ProcurementRequesterCandidatesCapability {
    type Input = RequesterCandidatesInput;
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
        let candidates = ProcurementReferenceOps::requester_candidates(
            &self.pool,
            context.principal().tenant_id(),
            trimmed(input.search.as_deref()),
        )
        .await
        .map_err(|_| dependency_failure("Requisition requesters could not be loaded."))?;
        Ok(json!({
            "employees": candidates
                .employees
                .iter()
                .map(requester_projection)
                .collect::<Vec<_>>()
        }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProcurementSuppliersListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<String>,
}

pub(super) struct ProcurementSuppliersListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl ProcurementSuppliersListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "procurement.suppliers.list",
                "List Procurement suppliers",
                "Returns supplier identity, contact, and lifecycle data without internal actor identifiers.",
                json!({
                    "page": page_schema(),
                    "per_page": per_page_schema(),
                    "search": search_schema(),
                    "status": { "type": ["string", "null"], "maxLength": 40 }
                }),
                json!({
                    "suppliers": { "type": "array" },
                    "pagination": { "type": "object" }
                }),
                DataSensitivity::Sensitive,
                "procurement.suppliers",
            ),
        }
    }
}

#[async_trait]
impl Capability for ProcurementSuppliersListCapability {
    type Input = ProcurementSuppliersListInput;
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
        let (suppliers, total) = SupplierOps::list(
            &self.pool,
            context.principal().tenant_id(),
            page,
            per_page,
            trimmed(input.search.as_deref()),
            trimmed(input.status.as_deref()),
        )
        .await
        .map_err(|_| dependency_failure("Procurement suppliers could not be loaded."))?;
        Ok(json!({
            "suppliers": suppliers.iter().map(supplier_projection).collect::<Vec<_>>(),
            "pagination": PaginationMeta::new(page as u32, per_page as u32, total)
        }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProcurementRequisitionsListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<String>,
    requester_employee_id: Option<Uuid>,
}

pub(super) struct ProcurementRequisitionsListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl ProcurementRequisitionsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "procurement.requisitions.list",
                "List Procurement requisitions",
                "Returns requisition headers, requester references, multi-currency totals, and approval state without login-link identifiers.",
                json!({
                    "page": page_schema(),
                    "per_page": per_page_schema(),
                    "search": search_schema(),
                    "status": { "type": ["string", "null"], "maxLength": 40 },
                    "requester_employee_id": { "type": ["string", "null"], "format": "uuid" }
                }),
                json!({
                    "requisitions": { "type": "array" },
                    "pagination": { "type": "object" }
                }),
                DataSensitivity::Sensitive,
                "procurement.requisitions",
            ),
        }
    }
}

#[async_trait]
impl Capability for ProcurementRequisitionsListCapability {
    type Input = ProcurementRequisitionsListInput;
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
        let (requisitions, total) = RequisitionOps::list(
            &self.pool,
            context.principal().tenant_id(),
            page,
            per_page,
            trimmed(input.search.as_deref()),
            trimmed(input.status.as_deref()),
            input.requester_employee_id,
        )
        .await
        .map_err(|_| dependency_failure("Procurement requisitions could not be loaded."))?;
        Ok(json!({
            "requisitions": requisitions
                .iter()
                .map(requisition_summary_projection)
                .collect::<Vec<_>>(),
            "pagination": PaginationMeta::new(page as u32, per_page as u32, total)
        }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProcurementPurchaseOrdersListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<String>,
    requisition_id: Option<Uuid>,
    supplier_id: Option<Uuid>,
}

pub(super) struct ProcurementPurchaseOrdersListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl ProcurementPurchaseOrdersListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "procurement.purchase_orders.list",
                "List Procurement purchase orders",
                "Returns bounded purchase-order snapshots and receiving state without internal actor or login-link identifiers.",
                json!({
                    "page": page_schema(),
                    "per_page": per_page_schema(),
                    "search": search_schema(),
                    "status": { "type": ["string", "null"], "maxLength": 40 },
                    "requisition_id": { "type": ["string", "null"], "format": "uuid" },
                    "supplier_id": { "type": ["string", "null"], "format": "uuid" }
                }),
                json!({
                    "purchase_orders": { "type": "array" },
                    "pagination": { "type": "object" }
                }),
                DataSensitivity::Sensitive,
                "procurement.purchase_orders",
            ),
        }
    }
}

#[async_trait]
impl Capability for ProcurementPurchaseOrdersListCapability {
    type Input = ProcurementPurchaseOrdersListInput;
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
        let (purchase_orders, total) = PurchaseOrderOps::list(
            &self.pool,
            context.principal().tenant_id(),
            page,
            per_page,
            trimmed(input.search.as_deref()),
            trimmed(input.status.as_deref()),
            input.requisition_id,
            input.supplier_id,
        )
        .await
        .map_err(|_| dependency_failure("Procurement purchase orders could not be loaded."))?;
        Ok(json!({
            "purchase_orders": purchase_orders
                .iter()
                .map(purchase_order_summary_projection)
                .collect::<Vec<_>>(),
            "pagination": PaginationMeta::new(page as u32, per_page as u32, total)
        }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProcurementGoodsReceiptsListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<String>,
    purchase_order_id: Option<Uuid>,
    supplier_id: Option<Uuid>,
}

pub(super) struct ProcurementGoodsReceiptsListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl ProcurementGoodsReceiptsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "procurement.goods_receipts.list",
                "List Procurement goods receipts",
                "Returns bounded receipt snapshots and posting state without internal actor identifiers.",
                json!({
                    "page": page_schema(),
                    "per_page": per_page_schema(),
                    "search": search_schema(),
                    "status": { "type": ["string", "null"], "maxLength": 40 },
                    "purchase_order_id": { "type": ["string", "null"], "format": "uuid" },
                    "supplier_id": { "type": ["string", "null"], "format": "uuid" }
                }),
                json!({
                    "goods_receipts": { "type": "array" },
                    "pagination": { "type": "object" }
                }),
                DataSensitivity::Sensitive,
                "procurement.goods_receipts",
            ),
        }
    }
}

#[async_trait]
impl Capability for ProcurementGoodsReceiptsListCapability {
    type Input = ProcurementGoodsReceiptsListInput;
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
        let (goods_receipts, total) = GoodsReceiptOps::list(
            &self.pool,
            context.principal().tenant_id(),
            page,
            per_page,
            trimmed(input.search.as_deref()),
            trimmed(input.status.as_deref()),
            input.purchase_order_id,
            input.supplier_id,
        )
        .await
        .map_err(|_| dependency_failure("Procurement goods receipts could not be loaded."))?;
        Ok(json!({
            "goods_receipts": goods_receipts
                .iter()
                .map(goods_receipt_summary_projection)
                .collect::<Vec<_>>(),
            "pagination": PaginationMeta::new(page as u32, per_page as u32, total)
        }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProcurementRecordInput {
    record_id: Uuid,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ProcurementReadKind {
    Supplier,
    Requisition,
    PurchaseOrder,
    GoodsReceipt,
}

impl ProcurementReadKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::Supplier => "procurement.suppliers.read",
            Self::Requisition => "procurement.requisitions.read",
            Self::PurchaseOrder => "procurement.purchase_orders.read",
            Self::GoodsReceipt => "procurement.goods_receipts.read",
        }
    }
}

pub(super) struct ProcurementReadCapability {
    pool: PgPool,
    kind: ProcurementReadKind,
    descriptor: CapabilityDescriptor,
}

impl ProcurementReadCapability {
    pub(super) fn new(pool: PgPool, kind: ProcurementReadKind) -> Self {
        let (title, description, resource) = match kind {
            ProcurementReadKind::Supplier => (
                "Read Procurement supplier",
                "Returns one supplier without internal mutation-actor identifiers.",
                "procurement.suppliers",
            ),
            ProcurementReadKind::Requisition => (
                "Read Procurement requisition",
                "Returns one requisition with its lines and controlled approval state without login-link identifiers.",
                "procurement.requisitions",
            ),
            ProcurementReadKind::PurchaseOrder => (
                "Read Procurement purchase order",
                "Returns one purchase order with immutable source snapshots and lines without internal actor or login-link identifiers.",
                "procurement.purchase_orders",
            ),
            ProcurementReadKind::GoodsReceipt => (
                "Read Procurement goods receipt",
                "Returns one goods receipt with immutable source snapshots and lines without internal actor identifiers.",
                "procurement.goods_receipts",
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
                DataSensitivity::Sensitive,
                resource,
            ),
        }
    }
}

#[async_trait]
impl Capability for ProcurementReadCapability {
    type Input = ProcurementRecordInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        let resource_kind = match self.kind {
            ProcurementReadKind::Supplier => "procurement_supplier",
            ProcurementReadKind::Requisition => "procurement_requisition",
            ProcurementReadKind::PurchaseOrder => "procurement_purchase_order",
            ProcurementReadKind::GoodsReceipt => "procurement_goods_receipt",
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
            ProcurementReadKind::Supplier => {
                SupplierOps::get(&self.pool, tenant_id, input.record_id)
                    .await
                    .map_err(|_| {
                        dependency_failure("The Procurement supplier could not be loaded.")
                    })?
                    .map(|supplier| supplier_projection(&supplier))
            }
            ProcurementReadKind::Requisition => {
                RequisitionOps::get(&self.pool, tenant_id, input.record_id)
                    .await
                    .map_err(|_| {
                        dependency_failure("The Procurement requisition could not be loaded.")
                    })?
                    .map(|requisition| requisition_projection(&requisition))
            }
            ProcurementReadKind::PurchaseOrder => {
                PurchaseOrderOps::get(&self.pool, tenant_id, input.record_id)
                    .await
                    .map_err(|_| {
                        dependency_failure("The Procurement purchase order could not be loaded.")
                    })?
                    .map(|purchase_order| purchase_order_projection(&purchase_order))
            }
            ProcurementReadKind::GoodsReceipt => {
                GoodsReceiptOps::get(&self.pool, tenant_id, input.record_id)
                    .await
                    .map_err(|_| {
                        dependency_failure("The Procurement goods receipt could not be loaded.")
                    })?
                    .map(|goods_receipt| goods_receipt_projection(&goods_receipt))
            }
        }
        .ok_or_else(|| {
            CapabilityExecutionError::new(
                CapabilityExecutionErrorCode::InvalidState,
                "The Procurement record was not found.",
            )
        })?;
        Ok(json!({ "record": record }))
    }
}

fn requester_projection(employee: &EmployeeReference) -> Value {
    json!({
        "id": employee.id,
        "employee_number": employee.employee_number,
        "display_name": employee.display_name,
        "work_email": employee.work_email,
        "employment_status": employee.employment_status
    })
}

fn supplier_projection(supplier: &SupplierResponse) -> Value {
    json!({
        "id": supplier.id,
        "supplier_number": supplier.supplier_number,
        "legal_name": supplier.legal_name,
        "trading_name": supplier.trading_name,
        "registration_number": supplier.registration_number,
        "tax_number": supplier.tax_number,
        "email": supplier.email,
        "phone": supplier.phone,
        "address": supplier.address,
        "status": supplier.status,
        "version": supplier.version,
        "created_at": supplier.created_at,
        "updated_at": supplier.updated_at
    })
}

fn requisition_summary_projection(requisition: &RequisitionSummary) -> Value {
    json!({
        "id": requisition.id,
        "requisition_number": requisition.requisition_number,
        "requester_employee_id": requisition.requester_employee_id,
        "requester_employee_number": requisition.requester_employee_number,
        "requester_name": requisition.requester_name,
        "currency_id": requisition.currency_id,
        "currency_code": requisition.currency_code,
        "currency_minor_units": requisition.currency_minor_units,
        "title": requisition.title,
        "purpose": requisition.purpose,
        "needed_by": requisition.needed_by,
        "status": requisition.status,
        "version": requisition.version,
        "total_minor": requisition.total_minor,
        "line_count": requisition.line_count,
        "submitted_at": requisition.submitted_at,
        "decided_at": requisition.decided_at,
        "decision_note": requisition.decision_note,
        "cancelled_at": requisition.cancelled_at,
        "cancellation_note": requisition.cancellation_note,
        "created_at": requisition.created_at,
        "updated_at": requisition.updated_at
    })
}

fn requisition_projection(requisition: &RequisitionResponse) -> Value {
    json!({
        "summary": requisition_summary_projection(&requisition.summary),
        "lines": requisition.lines
    })
}

fn purchase_order_summary_projection(purchase_order: &PurchaseOrderSummary) -> Value {
    json!({
        "id": purchase_order.id,
        "purchase_order_number": purchase_order.purchase_order_number,
        "requisition_id": purchase_order.requisition_id,
        "requisition_number": purchase_order.requisition_number,
        "requisition_title": purchase_order.requisition_title,
        "requisition_purpose": purchase_order.requisition_purpose,
        "requisition_needed_by": purchase_order.requisition_needed_by,
        "requester_employee_id": purchase_order.requester_employee_id,
        "requester_employee_number": purchase_order.requester_employee_number,
        "requester_name": purchase_order.requester_name,
        "supplier_id": purchase_order.supplier_id,
        "supplier_number": purchase_order.supplier_number,
        "supplier_name": purchase_order.supplier_name,
        "currency_id": purchase_order.currency_id,
        "currency_code": purchase_order.currency_code,
        "currency_minor_units": purchase_order.currency_minor_units,
        "delivery_date": purchase_order.delivery_date,
        "notes": purchase_order.notes,
        "status": purchase_order.status,
        "version": purchase_order.version,
        "total_minor": purchase_order.total_minor,
        "line_count": purchase_order.line_count,
        "issued_at": purchase_order.issued_at,
        "cancelled_at": purchase_order.cancelled_at,
        "cancellation_note": purchase_order.cancellation_note,
        "created_at": purchase_order.created_at,
        "updated_at": purchase_order.updated_at
    })
}

fn purchase_order_projection(purchase_order: &PurchaseOrderResponse) -> Value {
    json!({
        "summary": purchase_order_summary_projection(&purchase_order.summary),
        "lines": purchase_order.lines
    })
}

fn goods_receipt_summary_projection(goods_receipt: &GoodsReceiptSummary) -> Value {
    json!({
        "id": goods_receipt.id,
        "goods_receipt_number": goods_receipt.goods_receipt_number,
        "purchase_order_id": goods_receipt.purchase_order_id,
        "purchase_order_number": goods_receipt.purchase_order_number,
        "requisition_id": goods_receipt.requisition_id,
        "requisition_number": goods_receipt.requisition_number,
        "supplier_id": goods_receipt.supplier_id,
        "supplier_number": goods_receipt.supplier_number,
        "supplier_name": goods_receipt.supplier_name,
        "currency_id": goods_receipt.currency_id,
        "currency_code": goods_receipt.currency_code,
        "currency_minor_units": goods_receipt.currency_minor_units,
        "received_on": goods_receipt.received_on,
        "delivery_reference": goods_receipt.delivery_reference,
        "notes": goods_receipt.notes,
        "status": goods_receipt.status,
        "version": goods_receipt.version,
        "line_count": goods_receipt.line_count,
        "posted_at": goods_receipt.posted_at,
        "created_at": goods_receipt.created_at,
        "updated_at": goods_receipt.updated_at
    })
}

fn goods_receipt_projection(goods_receipt: &GoodsReceiptResponse) -> Value {
    json!({
        "summary": goods_receipt_summary_projection(&goods_receipt.summary),
        "lines": goods_receipt.lines
    })
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

fn resource_scope(kind: &str, id: Uuid) -> CapabilityScope {
    CapabilityScope::resources([CapabilityResource::parse(kind, id.to_string())
        .unwrap_or_else(|error| panic!("invalid built-in capability resource: {error}"))])
    .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
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

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use cp_hr_payroll::models::EmployeeReference;
    use cp_procurement::{
        goods_receipts::GoodsReceiptSummary, purchase_orders::PurchaseOrderSummary,
        requisitions::RequisitionSummary, suppliers::SupplierResponse,
    };
    use uuid::Uuid;

    use super::{
        bounded_page, goods_receipt_summary_projection, purchase_order_summary_projection,
        requester_projection, requisition_summary_projection, supplier_projection, trimmed,
    };

    #[test]
    fn pagination_and_search_inputs_are_bounded_before_domain_reads() {
        assert_eq!(bounded_page(None, None), (1, 25));
        assert_eq!(bounded_page(Some(-5), Some(999)), (1, 100));
        assert_eq!(trimmed(Some("  supplier  ")), Some("supplier"));
        assert_eq!(trimmed(Some("   ")), None);
    }

    #[test]
    fn projections_omit_login_links_and_internal_actor_ids() {
        let account_id = Uuid::new_v4();
        let employee = EmployeeReference {
            id: Uuid::new_v4(),
            account_id: Some(account_id),
            employee_number: "EMP-0001".to_string(),
            display_name: "Sam Requester".to_string(),
            work_email: Some("sam@example.test".to_string()),
            phone: Some("+263000000000".to_string()),
            employment_status: "active".to_string(),
        };
        let employee_json = requester_projection(&employee);
        assert!(employee_json.get("account_id").is_none());
        assert!(employee_json.get("phone").is_none());
        assert_eq!(employee_json["employee_number"], "EMP-0001");

        let actor_id = Uuid::new_v4();
        let now = Utc::now();
        let supplier = SupplierResponse {
            id: Uuid::new_v4(),
            supplier_number: "SUP-0001".to_string(),
            legal_name: "Stationery Supplier".to_string(),
            trading_name: None,
            registration_number: None,
            tax_number: None,
            email: None,
            phone: None,
            address: None,
            status: "active".to_string(),
            version: 1,
            created_by: actor_id,
            created_at: now,
            updated_at: now,
        };
        let supplier_json = supplier_projection(&supplier);
        assert!(supplier_json.get("created_by").is_none());
        assert_eq!(supplier_json["supplier_number"], "SUP-0001");

        let requisition = RequisitionSummary {
            id: Uuid::new_v4(),
            requisition_number: "REQ-2026-000001".to_string(),
            requester_employee_id: employee.id,
            requester_account_id: Some(account_id),
            requester_employee_number: employee.employee_number,
            requester_name: employee.display_name,
            currency_id: Uuid::new_v4(),
            currency_code: "USD".to_string(),
            currency_minor_units: 2,
            title: "Classroom supplies".to_string(),
            purpose: None,
            needed_by: None,
            status: "submitted".to_string(),
            version: 2,
            total_minor: 10_000,
            line_count: 1,
            created_by: actor_id,
            submitted_by: Some(actor_id),
            submitted_at: Some(now),
            decided_by: None,
            decided_at: None,
            decision_note: None,
            cancelled_by: None,
            cancelled_at: None,
            cancellation_note: None,
            created_at: now,
            updated_at: now,
        };
        let requisition_json = requisition_summary_projection(&requisition);
        for omitted in [
            "requester_account_id",
            "created_by",
            "submitted_by",
            "decided_by",
            "cancelled_by",
        ] {
            assert!(requisition_json.get(omitted).is_none(), "{omitted}");
        }
        assert_eq!(requisition_json["currency_code"], "USD");
        assert_eq!(requisition_json["total_minor"], 10_000);

        let purchase_order = PurchaseOrderSummary {
            id: Uuid::new_v4(),
            purchase_order_number: "PO-000001".to_string(),
            requisition_id: requisition.id,
            requisition_number: requisition.requisition_number,
            requisition_title: requisition.title,
            requisition_purpose: None,
            requisition_needed_by: None,
            requester_employee_id: employee.id,
            requester_account_id: Some(account_id),
            requester_employee_number: "EMP-0001".to_string(),
            requester_name: "Sam Requester".to_string(),
            supplier_id: supplier.id,
            supplier_number: supplier.supplier_number,
            supplier_name: supplier.legal_name,
            currency_id: Uuid::new_v4(),
            currency_code: "USD".to_string(),
            currency_minor_units: 2,
            delivery_date: None,
            notes: None,
            status: "issued".to_string(),
            version: 2,
            total_minor: 10_000,
            line_count: 1,
            created_by: actor_id,
            prepared_by: actor_id,
            issued_by: Some(Uuid::new_v4()),
            issued_at: Some(now),
            cancelled_by: None,
            cancelled_at: None,
            cancellation_note: None,
            created_at: now,
            updated_at: now,
        };
        let purchase_order_json = purchase_order_summary_projection(&purchase_order);
        for omitted in [
            "requester_account_id",
            "created_by",
            "prepared_by",
            "issued_by",
            "cancelled_by",
        ] {
            assert!(purchase_order_json.get(omitted).is_none(), "{omitted}");
        }
        assert_eq!(purchase_order_json["purchase_order_number"], "PO-000001");

        let goods_receipt = GoodsReceiptSummary {
            id: Uuid::new_v4(),
            goods_receipt_number: "GRN-000001".to_string(),
            purchase_order_id: purchase_order.id,
            purchase_order_number: purchase_order.purchase_order_number,
            requisition_id: purchase_order.requisition_id,
            requisition_number: purchase_order.requisition_number,
            supplier_id: purchase_order.supplier_id,
            supplier_number: purchase_order.supplier_number,
            supplier_name: purchase_order.supplier_name,
            currency_id: purchase_order.currency_id,
            currency_code: purchase_order.currency_code,
            currency_minor_units: purchase_order.currency_minor_units,
            received_on: now.date_naive(),
            delivery_reference: None,
            notes: None,
            status: "posted".to_string(),
            version: 2,
            line_count: 1,
            created_by: actor_id,
            prepared_by: actor_id,
            posted_by: Some(Uuid::new_v4()),
            posted_at: Some(now),
            created_at: now,
            updated_at: now,
        };
        let goods_receipt_json = goods_receipt_summary_projection(&goods_receipt);
        for omitted in ["created_by", "prepared_by", "posted_by"] {
            assert!(goods_receipt_json.get(omitted).is_none(), "{omitted}");
        }
        assert_eq!(goods_receipt_json["goods_receipt_number"], "GRN-000001");
    }
}
