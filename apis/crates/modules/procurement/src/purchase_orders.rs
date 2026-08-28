//! Owns supplier purchase orders derived from approved requisitions.
//!
//! Requisition, requester, currency, supplier, and line references are copied
//! once; issue freezes all order details and requires a different person actor.

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, NaiveDate, Utc};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;
use validator::Validate;

const MAX_MONEY_MINOR: i64 = 9_000_000_000_000_000;
const MAX_QUANTITY_MINOR: i64 = 9_000_000_000_000_000;
const MAX_PURCHASE_ORDER_LINES: usize = 200;

#[derive(Debug, Deserialize)]
pub struct PurchaseOrderListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub requisition_id: Option<Uuid>,
    pub supplier_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct PurchaseOrderLineInput {
    pub requisition_line_id: Uuid,
    #[validate(range(min = 1, max = 9_000_000_000_000_000_i64))]
    pub quantity_minor: i64,
    #[validate(range(max = 9))]
    pub quantity_scale: i16,
    #[validate(range(min = 0, max = 9_000_000_000_000_000_i64))]
    pub unit_amount_minor: i64,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreatePurchaseOrderRequest {
    pub requisition_id: Uuid,
    pub supplier_id: Uuid,
    pub delivery_date: Option<NaiveDate>,
    #[validate(length(max = 2000))]
    pub notes: Option<String>,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
    #[validate(length(min = 1, max = 200), nested)]
    pub lines: Vec<PurchaseOrderLineInput>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdatePurchaseOrderRequest {
    pub delivery_date: Option<NaiveDate>,
    #[validate(length(max = 2000))]
    pub notes: Option<String>,
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 200), nested)]
    pub lines: Vec<PurchaseOrderLineInput>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct PurchaseOrderTransitionRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(max = 1000))]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PurchaseOrderSummary {
    pub id: Uuid,
    pub purchase_order_number: String,
    pub requisition_id: Uuid,
    pub requisition_number: String,
    pub requisition_title: String,
    pub requisition_purpose: Option<String>,
    pub requisition_needed_by: Option<NaiveDate>,
    pub requester_employee_id: Uuid,
    pub requester_account_id: Option<Uuid>,
    pub requester_employee_number: String,
    pub requester_name: String,
    pub supplier_id: Uuid,
    pub supplier_number: String,
    pub supplier_name: String,
    pub currency_id: Uuid,
    pub currency_code: String,
    pub currency_minor_units: i16,
    pub delivery_date: Option<NaiveDate>,
    pub notes: Option<String>,
    pub status: String,
    pub version: i32,
    pub total_minor: i64,
    pub line_count: i64,
    pub created_by: Uuid,
    pub prepared_by: Uuid,
    pub issued_by: Option<Uuid>,
    pub issued_at: Option<DateTime<Utc>>,
    pub cancelled_by: Option<Uuid>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub cancellation_note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PurchaseOrderLineResponse {
    pub id: Uuid,
    pub line_number: i32,
    pub requisition_line_id: Uuid,
    pub requisition_line_number: i32,
    pub description: String,
    pub unit_label: Option<String>,
    pub requisition_quantity_minor: i64,
    pub quantity_minor: i64,
    pub quantity_scale: i16,
    pub unit_amount_minor: i64,
    pub line_amount_minor: i64,
    pub received_quantity_minor: i64,
    pub remaining_quantity_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PurchaseOrderResponse {
    #[serde(flatten)]
    pub summary: PurchaseOrderSummary,
    pub lines: Vec<PurchaseOrderLineResponse>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedPurchaseOrdersResponse {
    pub purchase_orders: Vec<PurchaseOrderSummary>,
}

/// Transactional purchase-order workflows over immutable source snapshots.
pub struct PurchaseOrderOps;

impl PurchaseOrderOps {
    #[allow(clippy::too_many_arguments)]
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
        requisition_id: Option<Uuid>,
        supplier_id: Option<Uuid>,
    ) -> Result<(Vec<PurchaseOrderSummary>, i64)> {
        validate_status(status)?;
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, PurchaseOrderSummary>(&format!(
            "{} ORDER BY purchase_order.created_at DESC LIMIT $7 OFFSET $8",
            summary_query()
        ))
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(requisition_id)
        .bind(supplier_id)
        .bind(Option::<Uuid>::None)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list Procurement purchase orders")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
              FROM procurement_purchase_orders AS purchase_order
             WHERE purchase_order.tenant_id = $1 AND purchase_order.deleted_at IS NULL
               AND ($2::TEXT IS NULL OR purchase_order.purchase_order_number ILIKE $2
                    OR purchase_order.requisition_number ILIKE $2
                    OR purchase_order.supplier_number ILIKE $2
                    OR purchase_order.supplier_name ILIKE $2
                    OR purchase_order.requester_name ILIKE $2)
               AND ($3::TEXT IS NULL OR purchase_order.status = $3)
               AND ($4::UUID IS NULL OR purchase_order.requisition_id = $4)
               AND ($5::UUID IS NULL OR purchase_order.supplier_id = $5)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(requisition_id)
        .bind(supplier_id)
        .fetch_one(pool)
        .await
        .context("Failed to count Procurement purchase orders")?;
        Ok((rows, total))
    }

    pub async fn get(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<PurchaseOrderResponse>> {
        let summary = sqlx::query_as::<_, PurchaseOrderSummary>(summary_query())
            .bind(tenant_id)
            .bind(Option::<String>::None)
            .bind(Option::<String>::None)
            .bind(Option::<Uuid>::None)
            .bind(Option::<Uuid>::None)
            .bind(Some(id))
            .fetch_optional(pool)
            .await
            .context("Failed to read Procurement purchase order")?;
        let Some(summary) = summary else {
            return Ok(None);
        };
        let lines = load_lines(pool, tenant_id, id).await?;
        Ok(Some(PurchaseOrderResponse { summary, lines }))
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreatePurchaseOrderRequest,
    ) -> Result<PurchaseOrderResponse> {
        let actor_id = actor_id(actor)?;
        let values = PurchaseOrderValues::from_create(request)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start purchase order transaction")?;
        lock_numbering(&mut transaction, tenant_id).await?;
        if let Some(existing_id) = id_for_idempotency(
            &mut transaction,
            tenant_id,
            values.idempotency_key.as_deref().unwrap_or_default(),
        )
        .await?
        {
            transaction.rollback().await.ok();
            let existing = Self::get(pool, tenant_id, existing_id)
                .await?
                .ok_or_else(|| anyhow!("The idempotent purchase order could not be loaded"))?;
            if !values.matches(&existing) {
                bail!("Idempotency key already belongs to another purchase order request");
            }
            return Ok(existing);
        }
        let requisition =
            lock_approved_requisition(&mut transaction, tenant_id, values.requisition_id)
                .await?
                .ok_or_else(|| anyhow!("Purchase orders require an approved requisition"))?;
        let supplier = lock_active_supplier(&mut transaction, tenant_id, values.supplier_id)
            .await?
            .ok_or_else(|| anyhow!("Purchase orders require an active supplier"))?;
        let source_lines = load_source_lines(
            &mut transaction,
            tenant_id,
            values.requisition_id,
            &values.lines,
        )
        .await?;
        let purchase_order_number = next_purchase_order_number(&mut transaction, tenant_id).await?;
        let purchase_order_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO procurement_purchase_orders (
                id, tenant_id, purchase_order_number, requisition_id, requisition_number,
                requisition_title, requisition_purpose, requisition_needed_by,
                requester_employee_id, requester_account_id, requester_employee_number,
                requester_name, supplier_id, supplier_number, supplier_name, currency_id,
                currency_code, currency_minor_units, delivery_date, notes,
                idempotency_key, created_by, prepared_by
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23
            )
            "#,
        )
        .bind(purchase_order_id)
        .bind(tenant_id)
        .bind(&purchase_order_number)
        .bind(requisition.id)
        .bind(&requisition.requisition_number)
        .bind(&requisition.title)
        .bind(&requisition.purpose)
        .bind(requisition.needed_by)
        .bind(requisition.requester_employee_id)
        .bind(requisition.requester_account_id)
        .bind(&requisition.requester_employee_number)
        .bind(&requisition.requester_name)
        .bind(supplier.id)
        .bind(&supplier.supplier_number)
        .bind(&supplier.legal_name)
        .bind(requisition.currency_id)
        .bind(&requisition.currency_code)
        .bind(requisition.currency_minor_units)
        .bind(values.delivery_date)
        .bind(&values.notes)
        .bind(values.idempotency_key.as_deref())
        .bind(actor_id)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "Failed to create Procurement purchase order"))?;
        insert_lines(
            &mut transaction,
            tenant_id,
            purchase_order_id,
            &values.lines,
            &source_lines,
        )
        .await?;
        append_purchase_order_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            purchase_order_id,
            "procurement.purchase_orders.create",
            None,
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit purchase order transaction")?;
        Self::get(pool, tenant_id, purchase_order_id)
            .await?
            .ok_or_else(|| anyhow!("The created purchase order could not be loaded"))
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdatePurchaseOrderRequest,
    ) -> Result<Option<PurchaseOrderResponse>> {
        let values = PurchaseOrderValues::from_update(request)?;
        let actor_id = actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start purchase order transaction")?;
        let Some(current) = lock_purchase_order(&mut transaction, tenant_id, id).await? else {
            return Ok(None);
        };
        ensure_version(current.version, request.expected_version, "Purchase order")?;
        ensure_state(
            &current.status,
            "draft",
            "Only a draft purchase order can be edited",
        )?;
        let stored_lines = load_lines_in_transaction(&mut transaction, tenant_id, id).await?;
        ensure_same_line_references(&values.lines, &stored_lines)?;
        sqlx::query(
            r#"
            UPDATE procurement_purchase_orders
               SET delivery_date = $3, notes = $4, prepared_by = $5,
                   version = version + 1
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(values.delivery_date)
        .bind(&values.notes)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to update Procurement purchase order")?;
        for line in &values.lines {
            sqlx::query(
                r#"
                UPDATE procurement_purchase_order_lines
                   SET quantity_minor = $4, quantity_scale = $5,
                       unit_amount_minor = $6, line_amount_minor = $7
                 WHERE tenant_id = $1 AND purchase_order_id = $2
                   AND requisition_line_id = $3 AND deleted_at IS NULL
                "#,
            )
            .bind(tenant_id)
            .bind(id)
            .bind(line.requisition_line_id)
            .bind(line.quantity_minor)
            .bind(line.quantity_scale)
            .bind(line.unit_amount_minor)
            .bind(line.line_amount_minor)
            .execute(&mut *transaction)
            .await
            .context("Failed to update Procurement purchase order line")?;
        }
        append_purchase_order_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            id,
            "procurement.purchase_orders.update",
            None,
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit purchase order transaction")?;
        Self::get(pool, tenant_id, id).await
    }

    pub async fn issue(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<PurchaseOrderResponse>> {
        let actor_id = actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start purchase order transaction")?;
        let Some(current) = lock_purchase_order(&mut transaction, tenant_id, id).await? else {
            return Ok(None);
        };
        ensure_version(current.version, expected_version, "Purchase order")?;
        ensure_state(
            &current.status,
            "draft",
            "Only a draft purchase order can be issued",
        )?;
        if current.created_by == actor_id || current.prepared_by == actor_id {
            bail!("A different actor must issue the purchase order");
        }
        if lock_active_supplier(&mut transaction, tenant_id, current.supplier_id)
            .await?
            .is_none()
        {
            bail!("Purchase orders require an active supplier when issued");
        }
        ensure_requisition_capacity_at_issue(
            &mut transaction,
            tenant_id,
            id,
            current.requisition_id,
        )
        .await?;
        let line_count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM procurement_purchase_order_lines
             WHERE tenant_id = $1 AND purchase_order_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to count Procurement purchase order lines")?;
        if line_count == 0 {
            bail!("A purchase order requires at least one line");
        }
        sqlx::query(
            r#"
            UPDATE procurement_purchase_orders
               SET status = 'issued', issued_by = $3, issued_at = NOW(), version = version + 1
             WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to issue Procurement purchase order")?;
        append_purchase_order_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            id,
            "procurement.purchase_orders.issue",
            None,
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit purchase order transaction")?;
        Self::get(pool, tenant_id, id).await
    }

    pub async fn cancel(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &PurchaseOrderTransitionRequest,
    ) -> Result<Option<PurchaseOrderResponse>> {
        let actor_id = actor_id(actor)?;
        let note = optional(request.note.as_deref());
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start purchase order transaction")?;
        let Some(current) = lock_purchase_order(&mut transaction, tenant_id, id).await? else {
            return Ok(None);
        };
        ensure_version(current.version, request.expected_version, "Purchase order")?;
        if !matches!(current.status.as_str(), "draft" | "issued") {
            bail!("Only a draft or unreceived issued purchase order can be cancelled");
        }
        let has_receipts = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM procurement_goods_receipts
                 WHERE tenant_id = $1 AND purchase_order_id = $2 AND deleted_at IS NULL
            )
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to check purchase order receipts")?;
        if has_receipts {
            bail!("A purchase order with receipts cannot be cancelled");
        }
        sqlx::query(
            r#"
            UPDATE procurement_purchase_orders
               SET status = 'cancelled', cancelled_by = $3, cancelled_at = NOW(),
                   cancellation_note = $4, version = version + 1
             WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(actor_id)
        .bind(&note)
        .execute(&mut *transaction)
        .await
        .context("Failed to cancel Procurement purchase order")?;
        append_purchase_order_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            id,
            "procurement.purchase_orders.cancel",
            note.as_deref(),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit purchase order transaction")?;
        Self::get(pool, tenant_id, id).await
    }
}

#[derive(Debug)]
struct PurchaseOrderValues {
    requisition_id: Uuid,
    supplier_id: Uuid,
    delivery_date: Option<NaiveDate>,
    notes: Option<String>,
    idempotency_key: Option<String>,
    lines: Vec<PurchaseOrderLineValues>,
}

impl PurchaseOrderValues {
    fn from_create(request: &CreatePurchaseOrderRequest) -> Result<Self> {
        Self::new(
            request.requisition_id,
            request.supplier_id,
            request.delivery_date,
            request.notes.as_deref(),
            Some(request.idempotency_key.as_str()),
            &request.lines,
        )
    }

    fn from_update(request: &UpdatePurchaseOrderRequest) -> Result<Self> {
        Self::new(
            Uuid::nil(),
            Uuid::nil(),
            request.delivery_date,
            request.notes.as_deref(),
            None,
            &request.lines,
        )
    }

    fn new(
        requisition_id: Uuid,
        supplier_id: Uuid,
        delivery_date: Option<NaiveDate>,
        notes: Option<&str>,
        idempotency_key: Option<&str>,
        lines: &[PurchaseOrderLineInput],
    ) -> Result<Self> {
        if lines.is_empty() || lines.len() > MAX_PURCHASE_ORDER_LINES {
            bail!("A purchase order requires between 1 and {MAX_PURCHASE_ORDER_LINES} lines");
        }
        let lines = lines
            .iter()
            .map(PurchaseOrderLineValues::parse)
            .collect::<Result<Vec<_>>>()?;
        ensure_unique_line_references(&lines)?;
        checked_total(&lines)?;
        Ok(Self {
            requisition_id,
            supplier_id,
            delivery_date,
            notes: optional(notes),
            idempotency_key: idempotency_key
                .map(|value| required(value, "Idempotency key"))
                .transpose()?,
            lines,
        })
    }

    fn matches(&self, existing: &PurchaseOrderResponse) -> bool {
        self.requisition_id == existing.summary.requisition_id
            && self.supplier_id == existing.summary.supplier_id
            && self.delivery_date == existing.summary.delivery_date
            && self.notes == existing.summary.notes
            && self.lines.len() == existing.lines.len()
            && self.lines.iter().zip(&existing.lines).all(|(left, right)| {
                left.requisition_line_id == right.requisition_line_id
                    && left.quantity_minor == right.quantity_minor
                    && left.quantity_scale == right.quantity_scale
                    && left.unit_amount_minor == right.unit_amount_minor
            })
    }
}

#[derive(Debug)]
struct PurchaseOrderLineValues {
    requisition_line_id: Uuid,
    quantity_minor: i64,
    quantity_scale: i16,
    unit_amount_minor: i64,
    line_amount_minor: i64,
}

impl PurchaseOrderLineValues {
    fn parse(input: &PurchaseOrderLineInput) -> Result<Self> {
        if !(1..=MAX_QUANTITY_MINOR).contains(&input.quantity_minor) {
            bail!("Purchase order line quantity is outside the supported range");
        }
        if !(0..=9).contains(&input.quantity_scale) {
            bail!("Purchase order line quantity scale is outside the supported range");
        }
        if !(0..=MAX_MONEY_MINOR).contains(&input.unit_amount_minor) {
            bail!("Purchase order unit amount is outside the supported range");
        }
        Ok(Self {
            requisition_line_id: input.requisition_line_id,
            quantity_minor: input.quantity_minor,
            quantity_scale: input.quantity_scale,
            unit_amount_minor: input.unit_amount_minor,
            line_amount_minor: scaled_line_amount(
                input.quantity_minor,
                input.quantity_scale,
                input.unit_amount_minor,
            )?,
        })
    }
}

#[derive(Debug, FromRow)]
struct RequisitionSnapshot {
    id: Uuid,
    requisition_number: String,
    title: String,
    purpose: Option<String>,
    needed_by: Option<NaiveDate>,
    requester_employee_id: Uuid,
    requester_account_id: Option<Uuid>,
    requester_employee_number: String,
    requester_name: String,
    currency_id: Uuid,
    currency_code: String,
    currency_minor_units: i16,
}

#[derive(Debug, FromRow)]
struct SupplierSnapshot {
    id: Uuid,
    supplier_number: String,
    legal_name: String,
}

#[derive(Debug, FromRow)]
struct RequisitionLineSnapshot {
    id: Uuid,
    line_number: i32,
    description: String,
    unit_label: Option<String>,
    quantity_minor: i64,
    quantity_scale: i16,
}

#[derive(Debug, FromRow)]
struct PurchaseOrderState {
    requisition_id: Uuid,
    supplier_id: Uuid,
    status: String,
    version: i32,
    created_by: Uuid,
    prepared_by: Uuid,
}

async fn lock_purchase_order(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<PurchaseOrderState>> {
    sqlx::query_as::<_, PurchaseOrderState>(
        r#"
        SELECT requisition_id, supplier_id, status, version, created_by, prepared_by
          FROM procurement_purchase_orders
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock Procurement purchase order")
}

async fn lock_approved_requisition(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<RequisitionSnapshot>> {
    sqlx::query_as::<_, RequisitionSnapshot>(
        r#"
        SELECT id, requisition_number, title, purpose, needed_by,
               requester_employee_id, requester_account_id, requester_employee_number,
               requester_name, currency_id, currency_code, currency_minor_units
          FROM procurement_requisitions
         WHERE tenant_id = $1 AND id = $2 AND status = 'approved' AND deleted_at IS NULL
         FOR SHARE
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock approved Procurement requisition")
}

async fn lock_active_supplier(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<SupplierSnapshot>> {
    sqlx::query_as::<_, SupplierSnapshot>(
        r#"
        SELECT id, supplier_number, legal_name
          FROM procurement_suppliers
         WHERE tenant_id = $1 AND id = $2 AND status = 'active' AND deleted_at IS NULL
         FOR SHARE
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock active Procurement supplier")
}

async fn load_source_lines(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    requisition_id: Uuid,
    requested: &[PurchaseOrderLineValues],
) -> Result<HashMap<Uuid, RequisitionLineSnapshot>> {
    let ids = requested
        .iter()
        .map(|line| line.requisition_line_id)
        .collect::<Vec<_>>();
    let lines = sqlx::query_as::<_, RequisitionLineSnapshot>(
        r#"
        SELECT id, line_number, description, unit_label, quantity_minor, quantity_scale
          FROM procurement_requisition_lines
         WHERE tenant_id = $1 AND requisition_id = $2 AND id = ANY($3)
           AND deleted_at IS NULL
         ORDER BY line_number
         FOR SHARE
        "#,
    )
    .bind(tenant_id)
    .bind(requisition_id)
    .bind(&ids)
    .fetch_all(&mut **transaction)
    .await
    .context("Failed to load purchase order requisition lines")?;
    if lines.len() != ids.len() {
        bail!("Every purchase order line must belong to the approved requisition");
    }
    let lines = lines
        .into_iter()
        .map(|line| (line.id, line))
        .collect::<HashMap<_, _>>();
    for requested in requested {
        let source = lines
            .get(&requested.requisition_line_id)
            .ok_or_else(|| anyhow!("Purchase order requisition line was not found"))?;
        if requested.quantity_scale != source.quantity_scale {
            bail!("Purchase order quantity scale must match the requisition line");
        }
        if requested.quantity_minor > source.quantity_minor {
            bail!("Purchase order quantity cannot exceed the requisition quantity");
        }
    }
    Ok(lines)
}

async fn ensure_requisition_capacity_at_issue(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    purchase_order_id: Uuid,
    requisition_id: Uuid,
) -> Result<()> {
    let requisition_status = sqlx::query_scalar::<_, String>(
        r#"
        SELECT status FROM procurement_requisitions
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(requisition_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock purchase order requisition capacity")?;
    if requisition_status.as_deref() != Some("approved") {
        bail!("Purchase orders require an approved requisition when issued");
    }
    let exceeds = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1
              FROM procurement_purchase_order_lines AS current_line
              JOIN procurement_requisition_lines AS requisition_line
                ON requisition_line.id = current_line.requisition_line_id
               AND requisition_line.tenant_id = current_line.tenant_id
              LEFT JOIN (
                    SELECT other_line.requisition_line_id,
                           SUM(other_line.quantity_minor)::NUMERIC AS quantity_minor
                      FROM procurement_purchase_order_lines AS other_line
                      JOIN procurement_purchase_orders AS other_order
                        ON other_order.id = other_line.purchase_order_id
                       AND other_order.tenant_id = other_line.tenant_id
                     WHERE other_order.tenant_id = $1
                       AND other_order.requisition_id = $3
                       AND other_order.id <> $2
                       AND other_order.status IN ('issued', 'partially_received', 'received')
                       AND other_order.deleted_at IS NULL AND other_line.deleted_at IS NULL
                     GROUP BY other_line.requisition_line_id
              ) AS ordered ON ordered.requisition_line_id = current_line.requisition_line_id
             WHERE current_line.tenant_id = $1 AND current_line.purchase_order_id = $2
               AND current_line.deleted_at IS NULL
               AND COALESCE(ordered.quantity_minor, 0) + current_line.quantity_minor::NUMERIC
                   > requisition_line.quantity_minor::NUMERIC
        )
        "#,
    )
    .bind(tenant_id)
    .bind(purchase_order_id)
    .bind(requisition_id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to check cumulative requisition order quantities")?;
    if exceeds {
        bail!("Issued purchase order quantities cannot exceed the requisition");
    }
    Ok(())
}

async fn insert_lines(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    purchase_order_id: Uuid,
    requested: &[PurchaseOrderLineValues],
    source_lines: &HashMap<Uuid, RequisitionLineSnapshot>,
) -> Result<()> {
    for (index, line) in requested.iter().enumerate() {
        let source = source_lines
            .get(&line.requisition_line_id)
            .ok_or_else(|| anyhow!("Purchase order requisition line was not found"))?;
        sqlx::query(
            r#"
            INSERT INTO procurement_purchase_order_lines (
                tenant_id, purchase_order_id, line_number, requisition_line_id,
                requisition_line_number, description, unit_label,
                requisition_quantity_minor, quantity_minor, quantity_scale,
                unit_amount_minor, line_amount_minor
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
        )
        .bind(tenant_id)
        .bind(purchase_order_id)
        .bind(i32::try_from(index + 1).context("Too many purchase order lines")?)
        .bind(source.id)
        .bind(source.line_number)
        .bind(&source.description)
        .bind(&source.unit_label)
        .bind(source.quantity_minor)
        .bind(line.quantity_minor)
        .bind(line.quantity_scale)
        .bind(line.unit_amount_minor)
        .bind(line.line_amount_minor)
        .execute(&mut **transaction)
        .await
        .context("Failed to save Procurement purchase order line")?;
    }
    Ok(())
}

async fn load_lines<'e, E>(
    executor: E,
    tenant_id: Uuid,
    purchase_order_id: Uuid,
) -> Result<Vec<PurchaseOrderLineResponse>>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, PurchaseOrderLineResponse>(
        r#"
        SELECT order_line.id, order_line.line_number, order_line.requisition_line_id,
               order_line.requisition_line_number, order_line.description,
               order_line.unit_label, order_line.requisition_quantity_minor,
               order_line.quantity_minor, order_line.quantity_scale,
               order_line.unit_amount_minor, order_line.line_amount_minor,
               COALESCE(received.quantity_minor, 0)::BIGINT AS received_quantity_minor,
               (order_line.quantity_minor::NUMERIC
                   - COALESCE(received.quantity_minor, 0))::BIGINT AS remaining_quantity_minor
          FROM procurement_purchase_order_lines AS order_line
          LEFT JOIN (
                SELECT receipt_line.purchase_order_line_id,
                       SUM(receipt_line.quantity_minor)::NUMERIC AS quantity_minor
                  FROM procurement_goods_receipt_lines AS receipt_line
                  JOIN procurement_goods_receipts AS receipt
                    ON receipt.id = receipt_line.goods_receipt_id
                   AND receipt.tenant_id = receipt_line.tenant_id
                 WHERE receipt.tenant_id = $1 AND receipt.purchase_order_id = $2
                   AND receipt.status = 'posted' AND receipt.deleted_at IS NULL
                   AND receipt_line.deleted_at IS NULL
                 GROUP BY receipt_line.purchase_order_line_id
          ) AS received ON received.purchase_order_line_id = order_line.id
         WHERE order_line.tenant_id = $1 AND order_line.purchase_order_id = $2
           AND order_line.deleted_at IS NULL
         ORDER BY order_line.line_number
        "#,
    )
    .bind(tenant_id)
    .bind(purchase_order_id)
    .fetch_all(executor)
    .await
    .context("Failed to read Procurement purchase order lines")
}

async fn load_lines_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    purchase_order_id: Uuid,
) -> Result<Vec<PurchaseOrderLineResponse>> {
    load_lines(&mut **transaction, tenant_id, purchase_order_id).await
}

fn ensure_same_line_references(
    requested: &[PurchaseOrderLineValues],
    stored: &[PurchaseOrderLineResponse],
) -> Result<()> {
    let requested = requested
        .iter()
        .map(|line| line.requisition_line_id)
        .collect::<HashSet<_>>();
    let stored = stored
        .iter()
        .map(|line| line.requisition_line_id)
        .collect::<HashSet<_>>();
    if requested != stored {
        bail!("Purchase order line references are immutable");
    }
    Ok(())
}

async fn id_for_idempotency(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    key: &str,
) -> Result<Option<Uuid>> {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id FROM procurement_purchase_orders
         WHERE tenant_id = $1 AND idempotency_key = $2
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(key)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to resolve purchase order idempotency")
}

async fn next_purchase_order_number(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
) -> Result<String> {
    let number = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO procurement_purchase_order_sequences (tenant_id, last_number)
        VALUES ($1, 1)
        ON CONFLICT (tenant_id)
        DO UPDATE SET last_number = procurement_purchase_order_sequences.last_number + 1,
                      deleted_at = NULL
        RETURNING last_number
        "#,
    )
    .bind(tenant_id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to allocate purchase order number")?;
    Ok(format!("PO-{number:06}"))
}

async fn lock_numbering(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("procurement-purchase-order:{tenant_id}"))
        .execute(&mut **transaction)
        .await
        .context("Failed to lock Procurement purchase order numbering")?;
    Ok(())
}

fn summary_query() -> &'static str {
    r#"
    SELECT purchase_order.id, purchase_order.purchase_order_number,
           purchase_order.requisition_id, purchase_order.requisition_number,
           purchase_order.requisition_title, purchase_order.requisition_purpose,
           purchase_order.requisition_needed_by, purchase_order.requester_employee_id,
           purchase_order.requester_account_id, purchase_order.requester_employee_number,
           purchase_order.requester_name, purchase_order.supplier_id,
           purchase_order.supplier_number, purchase_order.supplier_name,
           purchase_order.currency_id, purchase_order.currency_code,
           purchase_order.currency_minor_units, purchase_order.delivery_date,
           purchase_order.notes, purchase_order.status, purchase_order.version,
           COALESCE(SUM(line.line_amount_minor), 0)::BIGINT AS total_minor,
           COUNT(line.id)::BIGINT AS line_count, purchase_order.created_by,
           purchase_order.prepared_by,
           purchase_order.issued_by, purchase_order.issued_at,
           purchase_order.cancelled_by, purchase_order.cancelled_at,
           purchase_order.cancellation_note, purchase_order.created_at,
           purchase_order.updated_at
      FROM procurement_purchase_orders AS purchase_order
      LEFT JOIN procurement_purchase_order_lines AS line
        ON line.tenant_id = purchase_order.tenant_id
       AND line.purchase_order_id = purchase_order.id AND line.deleted_at IS NULL
     WHERE purchase_order.tenant_id = $1 AND purchase_order.deleted_at IS NULL
       AND ($2::TEXT IS NULL OR purchase_order.purchase_order_number ILIKE $2
            OR purchase_order.requisition_number ILIKE $2
            OR purchase_order.supplier_number ILIKE $2
            OR purchase_order.supplier_name ILIKE $2
            OR purchase_order.requester_name ILIKE $2)
       AND ($3::TEXT IS NULL OR purchase_order.status = $3)
       AND ($4::UUID IS NULL OR purchase_order.requisition_id = $4)
       AND ($5::UUID IS NULL OR purchase_order.supplier_id = $5)
       AND ($6::UUID IS NULL OR purchase_order.id = $6)
     GROUP BY purchase_order.id
    "#
}

#[derive(FromRow)]
struct PurchaseOrderAuditDetails {
    purchase_order_number: String,
    requisition_id: Uuid,
    supplier_id: Uuid,
    currency_code: String,
    status: String,
    total_minor: i64,
    line_count: i64,
}

#[allow(clippy::too_many_arguments)]
async fn append_purchase_order_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    id: Uuid,
    operation: &'static str,
    reason: Option<&str>,
) -> Result<()> {
    let details = sqlx::query_as::<_, PurchaseOrderAuditDetails>(
        r#"
        SELECT purchase_order.purchase_order_number, purchase_order.requisition_id,
               purchase_order.supplier_id, purchase_order.currency_code,
               purchase_order.status,
               COALESCE(SUM(line.line_amount_minor), 0)::BIGINT AS total_minor,
               COUNT(line.id)::BIGINT AS line_count
          FROM procurement_purchase_orders AS purchase_order
          LEFT JOIN procurement_purchase_order_lines AS line
            ON line.tenant_id = purchase_order.tenant_id
           AND line.purchase_order_id = purchase_order.id AND line.deleted_at IS NULL
         WHERE purchase_order.tenant_id = $1 AND purchase_order.id = $2
         GROUP BY purchase_order.id
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to load purchase order audit details")?;
    let outcome = if operation.ends_with(".cancel") {
        AuditOutcome::Cancelled
    } else {
        AuditOutcome::Succeeded
    };
    let mut event = NewAuditEvent::new(tenant_id, actor, operation, outcome, request_context)
        .with_target(AuditTarget::new(
            "procurement_purchase_order",
            id.to_string(),
        ))
        .with_redacted_metadata(
            json!({
                "purchase_order_number": details.purchase_order_number,
                "requisition_id": details.requisition_id,
                "supplier_id": details.supplier_id,
                "currency_code": details.currency_code,
                "status": details.status,
                "total_minor": details.total_minor,
                "line_count": details.line_count,
            })
            .as_object()
            .cloned()
            .unwrap_or_default(),
        );
    if let Some(reason) = reason {
        event = event.with_reason(reason);
    }
    append_audit(&mut **transaction, &event)
        .await
        .context("Failed to audit Procurement purchase order")?;
    Ok(())
}

fn scaled_line_amount(quantity_minor: i64, scale: i16, unit_amount_minor: i64) -> Result<i64> {
    let divisor = 10_i128
        .checked_pow(u32::try_from(scale).context("Quantity scale cannot be negative")?)
        .ok_or_else(|| anyhow!("Purchase order quantity scale is too large"))?;
    let product = i128::from(quantity_minor)
        .checked_mul(i128::from(unit_amount_minor))
        .ok_or_else(|| anyhow!("Purchase order line total is too large"))?;
    if product % divisor != 0 {
        bail!("Purchase order line total must resolve to exact currency minor units");
    }
    let amount = product / divisor;
    if amount > i128::from(MAX_MONEY_MINOR) {
        bail!("Purchase order line total is too large");
    }
    i64::try_from(amount).context("Purchase order line total is outside the supported range")
}

fn checked_total(lines: &[PurchaseOrderLineValues]) -> Result<i64> {
    lines.iter().try_fold(0_i64, |total, line| {
        total
            .checked_add(line.line_amount_minor)
            .filter(|amount| *amount <= MAX_MONEY_MINOR)
            .ok_or_else(|| anyhow!("Purchase order total is too large"))
    })
}

fn ensure_unique_line_references(lines: &[PurchaseOrderLineValues]) -> Result<()> {
    let unique = lines
        .iter()
        .map(|line| line.requisition_line_id)
        .collect::<HashSet<_>>();
    if unique.len() != lines.len() {
        bail!("A requisition line can appear only once on a purchase order");
    }
    Ok(())
}

fn actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("Authenticated person actor is required"))
}

fn validate_status(status: Option<&str>) -> Result<()> {
    if status.is_some_and(|value| {
        !matches!(
            value,
            "draft" | "issued" | "partially_received" | "received" | "cancelled"
        )
    }) {
        bail!("Purchase order status filter is invalid");
    }
    Ok(())
}

fn ensure_version(actual: i32, expected: i32, label: &str) -> Result<()> {
    if actual != expected {
        bail!("{label} changed since it was loaded");
    }
    Ok(())
}

fn ensure_state(actual: &str, expected: &str, message: &str) -> Result<()> {
    if actual != expected {
        bail!("{message}");
    }
    Ok(())
}

fn required(value: &str, label: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{label} is required");
    }
    Ok(value.to_string())
}

fn optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn database_error(error: sqlx::Error, context: &'static str) -> anyhow::Error {
    if let sqlx::Error::Database(database) = &error
        && database.code().as_deref() == Some("23505")
    {
        return anyhow!("A purchase order with that identity already exists");
    }
    anyhow::Error::new(error).context(context)
}

#[cfg(test)]
mod tests {
    use super::{
        PurchaseOrderLineInput, PurchaseOrderLineValues, ensure_unique_line_references,
        scaled_line_amount, validate_status,
    };
    use uuid::Uuid;

    fn line(
        id: Uuid,
        quantity_minor: i64,
        quantity_scale: i16,
        unit_amount_minor: i64,
    ) -> PurchaseOrderLineInput {
        PurchaseOrderLineInput {
            requisition_line_id: id,
            quantity_minor,
            quantity_scale,
            unit_amount_minor,
        }
    }

    #[test]
    fn scaled_quantities_require_exact_currency_minor_units() {
        assert_eq!(scaled_line_amount(125, 2, 400).unwrap(), 500);
        assert!(scaled_line_amount(1, 2, 1).is_err());
        assert!(scaled_line_amount(i64::MAX, 0, i64::MAX).is_err());
    }

    #[test]
    fn line_references_and_statuses_are_closed() {
        let id = Uuid::new_v4();
        let values = vec![
            PurchaseOrderLineValues::parse(&line(id, 1, 0, 100)).unwrap(),
            PurchaseOrderLineValues::parse(&line(id, 2, 0, 100)).unwrap(),
        ];
        assert!(ensure_unique_line_references(&values).is_err());
        assert!(validate_status(Some("partially_received")).is_ok());
        assert!(validate_status(Some("paid")).is_err());
    }
}
