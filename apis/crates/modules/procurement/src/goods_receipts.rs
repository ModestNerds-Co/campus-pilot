//! Owns goods receipts against issued purchase orders.
//!
//! Draft receipts may be corrected, but posting is immutable, requires a
//! different person actor, and serializes cumulative quantity checks per order.

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

const MAX_QUANTITY_MINOR: i64 = 9_000_000_000_000_000;
const MAX_GOODS_RECEIPT_LINES: usize = 200;

#[derive(Debug, Deserialize)]
pub struct GoodsReceiptListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub purchase_order_id: Option<Uuid>,
    pub supplier_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct GoodsReceiptLineInput {
    pub purchase_order_line_id: Uuid,
    #[validate(range(min = 1, max = 9_000_000_000_000_000_i64))]
    pub quantity_minor: i64,
    #[validate(range(max = 9))]
    pub quantity_scale: i16,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateGoodsReceiptRequest {
    pub purchase_order_id: Uuid,
    pub received_on: NaiveDate,
    #[validate(length(max = 200))]
    pub delivery_reference: Option<String>,
    #[validate(length(max = 2000))]
    pub notes: Option<String>,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
    #[validate(length(min = 1, max = 200), nested)]
    pub lines: Vec<GoodsReceiptLineInput>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateGoodsReceiptRequest {
    pub received_on: NaiveDate,
    #[validate(length(max = 200))]
    pub delivery_reference: Option<String>,
    #[validate(length(max = 2000))]
    pub notes: Option<String>,
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 200), nested)]
    pub lines: Vec<GoodsReceiptLineInput>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct GoodsReceiptPostRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct GoodsReceiptSummary {
    pub id: Uuid,
    pub goods_receipt_number: String,
    pub purchase_order_id: Uuid,
    pub purchase_order_number: String,
    pub requisition_id: Uuid,
    pub requisition_number: String,
    pub supplier_id: Uuid,
    pub supplier_number: String,
    pub supplier_name: String,
    pub currency_id: Uuid,
    pub currency_code: String,
    pub currency_minor_units: i16,
    pub received_on: NaiveDate,
    pub delivery_reference: Option<String>,
    pub notes: Option<String>,
    pub status: String,
    pub version: i32,
    pub line_count: i64,
    pub created_by: Uuid,
    pub prepared_by: Uuid,
    pub posted_by: Option<Uuid>,
    pub posted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct GoodsReceiptLineResponse {
    pub id: Uuid,
    pub line_number: i32,
    pub purchase_order_line_id: Uuid,
    pub purchase_order_line_number: i32,
    pub requisition_line_id: Uuid,
    pub description: String,
    pub unit_label: Option<String>,
    pub quantity_minor: i64,
    pub quantity_scale: i16,
}

#[derive(Debug, Clone, Serialize)]
pub struct GoodsReceiptResponse {
    #[serde(flatten)]
    pub summary: GoodsReceiptSummary,
    pub lines: Vec<GoodsReceiptLineResponse>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedGoodsReceiptsResponse {
    pub goods_receipts: Vec<GoodsReceiptSummary>,
}

/// Transactional goods-receipt workflows with serialized cumulative posting.
pub struct GoodsReceiptOps;

impl GoodsReceiptOps {
    #[allow(clippy::too_many_arguments)]
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
        purchase_order_id: Option<Uuid>,
        supplier_id: Option<Uuid>,
    ) -> Result<(Vec<GoodsReceiptSummary>, i64)> {
        validate_status(status)?;
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, GoodsReceiptSummary>(&format!(
            "{} ORDER BY receipt.created_at DESC LIMIT $7 OFFSET $8",
            summary_query()
        ))
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(purchase_order_id)
        .bind(supplier_id)
        .bind(Option::<Uuid>::None)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list Procurement goods receipts")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
              FROM procurement_goods_receipts AS receipt
             WHERE receipt.tenant_id = $1 AND receipt.deleted_at IS NULL
               AND ($2::TEXT IS NULL OR receipt.goods_receipt_number ILIKE $2
                    OR receipt.purchase_order_number ILIKE $2
                    OR receipt.requisition_number ILIKE $2
                    OR receipt.supplier_number ILIKE $2
                    OR receipt.supplier_name ILIKE $2
                    OR receipt.delivery_reference ILIKE $2)
               AND ($3::TEXT IS NULL OR receipt.status = $3)
               AND ($4::UUID IS NULL OR receipt.purchase_order_id = $4)
               AND ($5::UUID IS NULL OR receipt.supplier_id = $5)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .bind(purchase_order_id)
        .bind(supplier_id)
        .fetch_one(pool)
        .await
        .context("Failed to count Procurement goods receipts")?;
        Ok((rows, total))
    }

    pub async fn get(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<GoodsReceiptResponse>> {
        let summary = sqlx::query_as::<_, GoodsReceiptSummary>(summary_query())
            .bind(tenant_id)
            .bind(Option::<String>::None)
            .bind(Option::<String>::None)
            .bind(Option::<Uuid>::None)
            .bind(Option::<Uuid>::None)
            .bind(Some(id))
            .fetch_optional(pool)
            .await
            .context("Failed to read Procurement goods receipt")?;
        let Some(summary) = summary else {
            return Ok(None);
        };
        let lines = load_lines(pool, tenant_id, id).await?;
        Ok(Some(GoodsReceiptResponse { summary, lines }))
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateGoodsReceiptRequest,
    ) -> Result<GoodsReceiptResponse> {
        let actor_id = actor_id(actor)?;
        let values = GoodsReceiptValues::from_create(request)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start goods receipt transaction")?;
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
                .ok_or_else(|| anyhow!("The idempotent goods receipt could not be loaded"))?;
            if !values.matches(&existing) {
                bail!("Idempotency key already belongs to another goods receipt request");
            }
            return Ok(existing);
        }
        let order = lock_open_purchase_order(&mut transaction, tenant_id, values.purchase_order_id)
            .await?
            .ok_or_else(|| anyhow!("Goods receipts require an open issued purchase order"))?;
        let source_lines =
            lock_order_lines(&mut transaction, tenant_id, order.id, Some(&values.lines)).await?;
        validate_requested_lines(&values.lines, &source_lines)?;
        validate_draft_capacity(
            &mut transaction,
            tenant_id,
            order.id,
            &values.lines,
            &source_lines,
        )
        .await?;
        let goods_receipt_number = next_goods_receipt_number(&mut transaction, tenant_id).await?;
        let receipt_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO procurement_goods_receipts (
                id, tenant_id, goods_receipt_number, purchase_order_id,
                purchase_order_number, requisition_id, requisition_number,
                supplier_id, supplier_number, supplier_name, currency_id,
                currency_code, currency_minor_units, received_on, delivery_reference,
                notes, idempotency_key, created_by, prepared_by
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9,
                $10, $11, $12, $13, $14, $15, $16, $17, $18, $19
            )
            "#,
        )
        .bind(receipt_id)
        .bind(tenant_id)
        .bind(&goods_receipt_number)
        .bind(order.id)
        .bind(&order.purchase_order_number)
        .bind(order.requisition_id)
        .bind(&order.requisition_number)
        .bind(order.supplier_id)
        .bind(&order.supplier_number)
        .bind(&order.supplier_name)
        .bind(order.currency_id)
        .bind(&order.currency_code)
        .bind(order.currency_minor_units)
        .bind(values.received_on)
        .bind(&values.delivery_reference)
        .bind(&values.notes)
        .bind(values.idempotency_key.as_deref())
        .bind(actor_id)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "Failed to create Procurement goods receipt"))?;
        insert_lines(
            &mut transaction,
            tenant_id,
            receipt_id,
            &values.lines,
            &source_lines,
        )
        .await?;
        append_goods_receipt_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            receipt_id,
            "procurement.goods_receipts.create",
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit goods receipt transaction")?;
        Self::get(pool, tenant_id, receipt_id)
            .await?
            .ok_or_else(|| anyhow!("The created goods receipt could not be loaded"))
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateGoodsReceiptRequest,
    ) -> Result<Option<GoodsReceiptResponse>> {
        let values = GoodsReceiptValues::from_update(request)?;
        let actor_id = actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start goods receipt transaction")?;
        let Some(current) = lock_goods_receipt(&mut transaction, tenant_id, id).await? else {
            return Ok(None);
        };
        ensure_version(current.version, request.expected_version, "Goods receipt")?;
        ensure_state(
            &current.status,
            "draft",
            "Only a draft goods receipt can be edited",
        )?;
        let order =
            lock_open_purchase_order(&mut transaction, tenant_id, current.purchase_order_id)
                .await?
                .ok_or_else(|| anyhow!("Goods receipts require an open issued purchase order"))?;
        let source_lines =
            lock_order_lines(&mut transaction, tenant_id, order.id, Some(&values.lines)).await?;
        validate_requested_lines(&values.lines, &source_lines)?;
        validate_draft_capacity(
            &mut transaction,
            tenant_id,
            order.id,
            &values.lines,
            &source_lines,
        )
        .await?;
        sqlx::query(
            r#"
            UPDATE procurement_goods_receipts
               SET received_on = $3, delivery_reference = $4, notes = $5,
                   prepared_by = $6, version = version + 1
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(values.received_on)
        .bind(&values.delivery_reference)
        .bind(&values.notes)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to update Procurement goods receipt")?;
        sqlx::query(
            "DELETE FROM procurement_goods_receipt_lines WHERE tenant_id = $1 AND goods_receipt_id = $2",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to replace goods receipt lines")?;
        insert_lines(
            &mut transaction,
            tenant_id,
            id,
            &values.lines,
            &source_lines,
        )
        .await?;
        append_goods_receipt_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            id,
            "procurement.goods_receipts.update",
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit goods receipt transaction")?;
        Self::get(pool, tenant_id, id).await
    }

    pub async fn post(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<GoodsReceiptResponse>> {
        let actor_id = actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start goods receipt transaction")?;
        let Some(current) = lock_goods_receipt(&mut transaction, tenant_id, id).await? else {
            return Ok(None);
        };
        ensure_version(current.version, expected_version, "Goods receipt")?;
        ensure_state(
            &current.status,
            "draft",
            "Only a draft goods receipt can be posted",
        )?;
        if current.created_by == actor_id || current.prepared_by == actor_id {
            bail!("A different actor must post the goods receipt");
        }
        let order =
            lock_open_purchase_order(&mut transaction, tenant_id, current.purchase_order_id)
                .await?
                .ok_or_else(|| anyhow!("Goods receipts require an open issued purchase order"))?;
        let order_lines = lock_order_lines(&mut transaction, tenant_id, order.id, None).await?;
        let receipt_lines = load_lines_in_transaction(&mut transaction, tenant_id, id).await?;
        if receipt_lines.is_empty() {
            bail!("A goods receipt requires at least one line");
        }
        let posting = evaluate_posting(
            &mut transaction,
            tenant_id,
            order.id,
            id,
            &order_lines,
            &receipt_lines,
        )
        .await?;
        if posting.exceeds_order {
            bail!("Posted receipt quantities cannot exceed the purchase order");
        }
        let expected_order_status = if posting.fully_received {
            "received"
        } else {
            "partially_received"
        };
        sqlx::query(
            r#"
            UPDATE procurement_goods_receipts
               SET status = 'posted', posted_by = $3, posted_at = NOW(),
                   version = version + 1
             WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to post Procurement goods receipt")?;
        let actual_order_status = sqlx::query_scalar::<_, String>(
            r#"
            SELECT status FROM procurement_purchase_orders
             WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(order.id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to verify purchase order receipt status")?;
        if actual_order_status != expected_order_status {
            bail!("Goods receipt could not update its purchase order status");
        }
        append_goods_receipt_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            id,
            "procurement.goods_receipts.post",
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit goods receipt transaction")?;
        Self::get(pool, tenant_id, id).await
    }
}

#[derive(Debug)]
struct GoodsReceiptValues {
    purchase_order_id: Uuid,
    received_on: NaiveDate,
    delivery_reference: Option<String>,
    notes: Option<String>,
    idempotency_key: Option<String>,
    lines: Vec<GoodsReceiptLineValues>,
}

impl GoodsReceiptValues {
    fn from_create(request: &CreateGoodsReceiptRequest) -> Result<Self> {
        Self::new(
            request.purchase_order_id,
            request.received_on,
            request.delivery_reference.as_deref(),
            request.notes.as_deref(),
            Some(request.idempotency_key.as_str()),
            &request.lines,
        )
    }

    fn from_update(request: &UpdateGoodsReceiptRequest) -> Result<Self> {
        Self::new(
            Uuid::nil(),
            request.received_on,
            request.delivery_reference.as_deref(),
            request.notes.as_deref(),
            None,
            &request.lines,
        )
    }

    fn new(
        purchase_order_id: Uuid,
        received_on: NaiveDate,
        delivery_reference: Option<&str>,
        notes: Option<&str>,
        idempotency_key: Option<&str>,
        lines: &[GoodsReceiptLineInput],
    ) -> Result<Self> {
        if lines.is_empty() || lines.len() > MAX_GOODS_RECEIPT_LINES {
            bail!("A goods receipt requires between 1 and {MAX_GOODS_RECEIPT_LINES} lines");
        }
        let lines = lines
            .iter()
            .map(GoodsReceiptLineValues::parse)
            .collect::<Result<Vec<_>>>()?;
        ensure_unique_line_references(&lines)?;
        Ok(Self {
            purchase_order_id,
            received_on,
            delivery_reference: optional(delivery_reference),
            notes: optional(notes),
            idempotency_key: idempotency_key
                .map(|value| required(value, "Idempotency key"))
                .transpose()?,
            lines,
        })
    }

    fn matches(&self, existing: &GoodsReceiptResponse) -> bool {
        self.purchase_order_id == existing.summary.purchase_order_id
            && self.received_on == existing.summary.received_on
            && self.delivery_reference == existing.summary.delivery_reference
            && self.notes == existing.summary.notes
            && self.lines.len() == existing.lines.len()
            && self.lines.iter().zip(&existing.lines).all(|(left, right)| {
                left.purchase_order_line_id == right.purchase_order_line_id
                    && left.quantity_minor == right.quantity_minor
                    && left.quantity_scale == right.quantity_scale
            })
    }
}

#[derive(Debug)]
struct GoodsReceiptLineValues {
    purchase_order_line_id: Uuid,
    quantity_minor: i64,
    quantity_scale: i16,
}

impl GoodsReceiptLineValues {
    fn parse(input: &GoodsReceiptLineInput) -> Result<Self> {
        if !(1..=MAX_QUANTITY_MINOR).contains(&input.quantity_minor) {
            bail!("Goods receipt line quantity is outside the supported range");
        }
        if !(0..=9).contains(&input.quantity_scale) {
            bail!("Goods receipt line quantity scale is outside the supported range");
        }
        Ok(Self {
            purchase_order_line_id: input.purchase_order_line_id,
            quantity_minor: input.quantity_minor,
            quantity_scale: input.quantity_scale,
        })
    }
}

#[derive(Debug, FromRow)]
struct PurchaseOrderSnapshot {
    id: Uuid,
    purchase_order_number: String,
    requisition_id: Uuid,
    requisition_number: String,
    supplier_id: Uuid,
    supplier_number: String,
    supplier_name: String,
    currency_id: Uuid,
    currency_code: String,
    currency_minor_units: i16,
}

#[derive(Debug, Clone, FromRow)]
struct PurchaseOrderLineSnapshot {
    id: Uuid,
    line_number: i32,
    requisition_line_id: Uuid,
    description: String,
    unit_label: Option<String>,
    quantity_minor: i64,
    quantity_scale: i16,
}

#[derive(Debug, FromRow)]
struct GoodsReceiptState {
    purchase_order_id: Uuid,
    status: String,
    version: i32,
    created_by: Uuid,
    prepared_by: Uuid,
}

async fn lock_goods_receipt(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<GoodsReceiptState>> {
    sqlx::query_as::<_, GoodsReceiptState>(
        r#"
        SELECT purchase_order_id, status, version, created_by, prepared_by
          FROM procurement_goods_receipts
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock Procurement goods receipt")
}

async fn lock_open_purchase_order(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<PurchaseOrderSnapshot>> {
    sqlx::query_as::<_, PurchaseOrderSnapshot>(
        r#"
        SELECT id, purchase_order_number, requisition_id, requisition_number,
               supplier_id, supplier_number, supplier_name, currency_id,
               currency_code, currency_minor_units
          FROM procurement_purchase_orders
         WHERE tenant_id = $1 AND id = $2
           AND status IN ('issued', 'partially_received') AND deleted_at IS NULL
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock open Procurement purchase order")
}

async fn lock_order_lines(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    purchase_order_id: Uuid,
    requested: Option<&[GoodsReceiptLineValues]>,
) -> Result<HashMap<Uuid, PurchaseOrderLineSnapshot>> {
    let ids = requested.map(|lines| {
        lines
            .iter()
            .map(|line| line.purchase_order_line_id)
            .collect::<Vec<_>>()
    });
    let lines = sqlx::query_as::<_, PurchaseOrderLineSnapshot>(
        r#"
        SELECT id, line_number, requisition_line_id, description, unit_label,
               quantity_minor, quantity_scale
          FROM procurement_purchase_order_lines
         WHERE tenant_id = $1 AND purchase_order_id = $2
           AND ($3::UUID[] IS NULL OR id = ANY($3)) AND deleted_at IS NULL
         ORDER BY id
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(purchase_order_id)
    .bind(ids.as_deref())
    .fetch_all(&mut **transaction)
    .await
    .context("Failed to lock Procurement purchase order lines")?;
    if let Some(ids) = ids
        && lines.len() != ids.len()
    {
        bail!("Every goods receipt line must belong to the purchase order");
    }
    Ok(lines.into_iter().map(|line| (line.id, line)).collect())
}

fn validate_requested_lines(
    requested: &[GoodsReceiptLineValues],
    source_lines: &HashMap<Uuid, PurchaseOrderLineSnapshot>,
) -> Result<()> {
    for requested in requested {
        let source = source_lines
            .get(&requested.purchase_order_line_id)
            .ok_or_else(|| anyhow!("Goods receipt purchase order line was not found"))?;
        if requested.quantity_scale != source.quantity_scale {
            bail!("Goods receipt quantity scale must match the purchase order line");
        }
        if requested.quantity_minor > source.quantity_minor {
            bail!("Goods receipt quantity cannot exceed the purchase order line");
        }
    }
    Ok(())
}

async fn validate_draft_capacity(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    purchase_order_id: Uuid,
    requested: &[GoodsReceiptLineValues],
    source_lines: &HashMap<Uuid, PurchaseOrderLineSnapshot>,
) -> Result<()> {
    #[derive(FromRow)]
    struct PostedQuantity {
        purchase_order_line_id: Uuid,
        quantity_minor: i64,
    }
    let posted = sqlx::query_as::<_, PostedQuantity>(
        r#"
        SELECT receipt_line.purchase_order_line_id,
               SUM(receipt_line.quantity_minor)::BIGINT AS quantity_minor
          FROM procurement_goods_receipt_lines AS receipt_line
          JOIN procurement_goods_receipts AS receipt
            ON receipt.id = receipt_line.goods_receipt_id
           AND receipt.tenant_id = receipt_line.tenant_id
         WHERE receipt.tenant_id = $1 AND receipt.purchase_order_id = $2
           AND receipt.status = 'posted' AND receipt.deleted_at IS NULL
           AND receipt_line.deleted_at IS NULL
         GROUP BY receipt_line.purchase_order_line_id
        "#,
    )
    .bind(tenant_id)
    .bind(purchase_order_id)
    .fetch_all(&mut **transaction)
    .await
    .context("Failed to load posted purchase order quantities")?
    .into_iter()
    .map(|row| (row.purchase_order_line_id, row.quantity_minor))
    .collect::<HashMap<_, _>>();
    for requested in requested {
        let source = source_lines
            .get(&requested.purchase_order_line_id)
            .ok_or_else(|| anyhow!("Goods receipt purchase order line was not found"))?;
        let cumulative = posted
            .get(&requested.purchase_order_line_id)
            .copied()
            .unwrap_or_default()
            .checked_add(requested.quantity_minor)
            .ok_or_else(|| anyhow!("Goods receipt cumulative quantity is too large"))?;
        if cumulative > source.quantity_minor {
            bail!("Goods receipt quantity exceeds the unreceived purchase order quantity");
        }
    }
    Ok(())
}

async fn insert_lines(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    goods_receipt_id: Uuid,
    requested: &[GoodsReceiptLineValues],
    source_lines: &HashMap<Uuid, PurchaseOrderLineSnapshot>,
) -> Result<()> {
    for (index, line) in requested.iter().enumerate() {
        let source = source_lines
            .get(&line.purchase_order_line_id)
            .ok_or_else(|| anyhow!("Goods receipt purchase order line was not found"))?;
        sqlx::query(
            r#"
            INSERT INTO procurement_goods_receipt_lines (
                tenant_id, goods_receipt_id, line_number, purchase_order_line_id,
                purchase_order_line_number, requisition_line_id, description,
                unit_label, quantity_minor, quantity_scale
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(tenant_id)
        .bind(goods_receipt_id)
        .bind(i32::try_from(index + 1).context("Too many goods receipt lines")?)
        .bind(source.id)
        .bind(source.line_number)
        .bind(source.requisition_line_id)
        .bind(&source.description)
        .bind(&source.unit_label)
        .bind(line.quantity_minor)
        .bind(line.quantity_scale)
        .execute(&mut **transaction)
        .await
        .context("Failed to save Procurement goods receipt line")?;
    }
    Ok(())
}

async fn load_lines<'e, E>(
    executor: E,
    tenant_id: Uuid,
    goods_receipt_id: Uuid,
) -> Result<Vec<GoodsReceiptLineResponse>>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, GoodsReceiptLineResponse>(
        r#"
        SELECT id, line_number, purchase_order_line_id, purchase_order_line_number,
               requisition_line_id, description, unit_label, quantity_minor, quantity_scale
          FROM procurement_goods_receipt_lines
         WHERE tenant_id = $1 AND goods_receipt_id = $2 AND deleted_at IS NULL
         ORDER BY line_number
        "#,
    )
    .bind(tenant_id)
    .bind(goods_receipt_id)
    .fetch_all(executor)
    .await
    .context("Failed to read Procurement goods receipt lines")
}

async fn load_lines_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    goods_receipt_id: Uuid,
) -> Result<Vec<GoodsReceiptLineResponse>> {
    load_lines(&mut **transaction, tenant_id, goods_receipt_id).await
}

#[derive(Debug, FromRow)]
struct PostingEvaluation {
    exceeds_order: bool,
    fully_received: bool,
}

async fn evaluate_posting(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    purchase_order_id: Uuid,
    goods_receipt_id: Uuid,
    order_lines: &HashMap<Uuid, PurchaseOrderLineSnapshot>,
    receipt_lines: &[GoodsReceiptLineResponse],
) -> Result<PostingEvaluation> {
    for receipt_line in receipt_lines {
        let order_line = order_lines
            .get(&receipt_line.purchase_order_line_id)
            .ok_or_else(|| anyhow!("Goods receipt line does not belong to its purchase order"))?;
        if receipt_line.quantity_scale != order_line.quantity_scale {
            bail!("Goods receipt quantity scale must match the purchase order line");
        }
    }
    sqlx::query_as::<_, PostingEvaluation>(
        r#"
        WITH posted AS (
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
        ), current_receipt AS (
            SELECT purchase_order_line_id, SUM(quantity_minor)::NUMERIC AS quantity_minor
              FROM procurement_goods_receipt_lines
             WHERE tenant_id = $1 AND goods_receipt_id = $3 AND deleted_at IS NULL
             GROUP BY purchase_order_line_id
        ), totals AS (
            SELECT order_line.quantity_minor::NUMERIC AS ordered_quantity_minor,
                   COALESCE(posted.quantity_minor, 0)
                       + COALESCE(current_receipt.quantity_minor, 0) AS received_quantity_minor
              FROM procurement_purchase_order_lines AS order_line
              LEFT JOIN posted ON posted.purchase_order_line_id = order_line.id
              LEFT JOIN current_receipt
                ON current_receipt.purchase_order_line_id = order_line.id
             WHERE order_line.tenant_id = $1 AND order_line.purchase_order_id = $2
               AND order_line.deleted_at IS NULL
        )
        SELECT COALESCE(BOOL_OR(received_quantity_minor > ordered_quantity_minor), FALSE)
                   AS exceeds_order,
               COALESCE(BOOL_AND(received_quantity_minor = ordered_quantity_minor), FALSE)
                   AS fully_received
          FROM totals
        "#,
    )
    .bind(tenant_id)
    .bind(purchase_order_id)
    .bind(goods_receipt_id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to evaluate cumulative goods receipt quantities")
}

async fn id_for_idempotency(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    key: &str,
) -> Result<Option<Uuid>> {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id FROM procurement_goods_receipts
         WHERE tenant_id = $1 AND idempotency_key = $2
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(key)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to resolve goods receipt idempotency")
}

async fn next_goods_receipt_number(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
) -> Result<String> {
    let number = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO procurement_goods_receipt_sequences (tenant_id, last_number)
        VALUES ($1, 1)
        ON CONFLICT (tenant_id)
        DO UPDATE SET last_number = procurement_goods_receipt_sequences.last_number + 1,
                      deleted_at = NULL
        RETURNING last_number
        "#,
    )
    .bind(tenant_id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to allocate goods receipt number")?;
    Ok(format!("GRN-{number:06}"))
}

async fn lock_numbering(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("procurement-goods-receipt:{tenant_id}"))
        .execute(&mut **transaction)
        .await
        .context("Failed to lock Procurement goods receipt numbering")?;
    Ok(())
}

fn summary_query() -> &'static str {
    r#"
    SELECT receipt.id, receipt.goods_receipt_number, receipt.purchase_order_id,
           receipt.purchase_order_number, receipt.requisition_id,
           receipt.requisition_number, receipt.supplier_id, receipt.supplier_number,
           receipt.supplier_name, receipt.currency_id, receipt.currency_code,
           receipt.currency_minor_units, receipt.received_on, receipt.delivery_reference,
           receipt.notes, receipt.status, receipt.version, COUNT(line.id)::BIGINT AS line_count,
           receipt.created_by, receipt.prepared_by, receipt.posted_by, receipt.posted_at,
           receipt.created_at, receipt.updated_at
      FROM procurement_goods_receipts AS receipt
      LEFT JOIN procurement_goods_receipt_lines AS line
        ON line.tenant_id = receipt.tenant_id
       AND line.goods_receipt_id = receipt.id AND line.deleted_at IS NULL
     WHERE receipt.tenant_id = $1 AND receipt.deleted_at IS NULL
       AND ($2::TEXT IS NULL OR receipt.goods_receipt_number ILIKE $2
            OR receipt.purchase_order_number ILIKE $2
            OR receipt.requisition_number ILIKE $2
            OR receipt.supplier_number ILIKE $2
            OR receipt.supplier_name ILIKE $2
            OR receipt.delivery_reference ILIKE $2)
       AND ($3::TEXT IS NULL OR receipt.status = $3)
       AND ($4::UUID IS NULL OR receipt.purchase_order_id = $4)
       AND ($5::UUID IS NULL OR receipt.supplier_id = $5)
       AND ($6::UUID IS NULL OR receipt.id = $6)
     GROUP BY receipt.id
    "#
}

#[derive(FromRow)]
struct GoodsReceiptAuditDetails {
    goods_receipt_number: String,
    purchase_order_id: Uuid,
    supplier_id: Uuid,
    status: String,
    line_count: i64,
}

async fn append_goods_receipt_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    id: Uuid,
    operation: &'static str,
) -> Result<()> {
    let details = sqlx::query_as::<_, GoodsReceiptAuditDetails>(
        r#"
        SELECT receipt.goods_receipt_number, receipt.purchase_order_id,
               receipt.supplier_id, receipt.status, COUNT(line.id)::BIGINT AS line_count
          FROM procurement_goods_receipts AS receipt
          LEFT JOIN procurement_goods_receipt_lines AS line
            ON line.tenant_id = receipt.tenant_id
           AND line.goods_receipt_id = receipt.id AND line.deleted_at IS NULL
         WHERE receipt.tenant_id = $1 AND receipt.id = $2
         GROUP BY receipt.id
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to load goods receipt audit details")?;
    append_audit(
        &mut **transaction,
        &NewAuditEvent::new(
            tenant_id,
            actor,
            operation,
            AuditOutcome::Succeeded,
            request_context,
        )
        .with_target(AuditTarget::new(
            "procurement_goods_receipt",
            id.to_string(),
        ))
        .with_redacted_metadata(
            json!({
                "goods_receipt_number": details.goods_receipt_number,
                "purchase_order_id": details.purchase_order_id,
                "supplier_id": details.supplier_id,
                "status": details.status,
                "line_count": details.line_count,
            })
            .as_object()
            .cloned()
            .unwrap_or_default(),
        ),
    )
    .await
    .context("Failed to audit Procurement goods receipt")?;
    Ok(())
}

fn ensure_unique_line_references(lines: &[GoodsReceiptLineValues]) -> Result<()> {
    let unique = lines
        .iter()
        .map(|line| line.purchase_order_line_id)
        .collect::<HashSet<_>>();
    if unique.len() != lines.len() {
        bail!("A purchase order line can appear only once on a goods receipt");
    }
    Ok(())
}

fn actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("Authenticated person actor is required"))
}

fn validate_status(status: Option<&str>) -> Result<()> {
    if status.is_some_and(|value| !matches!(value, "draft" | "posted")) {
        bail!("Goods receipt status filter is invalid");
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
        return anyhow!("A goods receipt with that identity already exists");
    }
    anyhow::Error::new(error).context(context)
}

#[cfg(test)]
mod tests {
    use super::{
        GoodsReceiptLineInput, GoodsReceiptLineValues, ensure_unique_line_references,
        validate_status,
    };
    use sqlx::postgres::PgPoolOptions;
    use uuid::Uuid;

    fn line(id: Uuid, quantity_minor: i64, quantity_scale: i16) -> GoodsReceiptLineInput {
        GoodsReceiptLineInput {
            purchase_order_line_id: id,
            quantity_minor,
            quantity_scale,
        }
    }

    #[test]
    fn receipt_quantities_are_positive_scaled_values() {
        assert!(GoodsReceiptLineValues::parse(&line(Uuid::new_v4(), 1, 0)).is_ok());
        assert!(GoodsReceiptLineValues::parse(&line(Uuid::new_v4(), 0, 0)).is_err());
        assert!(GoodsReceiptLineValues::parse(&line(Uuid::new_v4(), 1, 10)).is_err());
    }

    #[test]
    fn receipt_line_references_and_statuses_are_closed() {
        let id = Uuid::new_v4();
        let values = vec![
            GoodsReceiptLineValues::parse(&line(id, 1, 0)).unwrap(),
            GoodsReceiptLineValues::parse(&line(id, 2, 0)).unwrap(),
        ];
        assert!(ensure_unique_line_references(&values).is_err());
        assert!(validate_status(Some("posted")).is_ok());
        assert!(validate_status(Some("reversed")).is_err());
    }

    #[actix_web::test]
    #[ignore = "requires a migrated PostgreSQL DATABASE_URL"]
    async fn database_enforces_order_and_receipt_contracts() {
        let database_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL is required for the Procurement database contract test");
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("Procurement database contract test could not connect");
        let mut transaction = pool
            .begin()
            .await
            .expect("Procurement database contract transaction could not start");
        let migration =
            include_str!("../../../../migrations/079_create_procurement_orders_and_receipts.sql");
        sqlx::raw_sql(migration)
            .execute(&mut *transaction)
            .await
            .expect("Migration 079 must apply over the current Procurement schema");
        sqlx::raw_sql(migration)
            .execute(&mut *transaction)
            .await
            .expect("Migration 079 must be safe to replay");
        sqlx::raw_sql(
            r#"
            DO $$
            DECLARE
                test_tenant UUID;
                creator UUID := '79000000-0000-0000-0000-000000000001';
                reviewer UUID := '79000000-0000-0000-0000-000000000002';
                preparer UUID := '79000000-0000-0000-0000-000000000016';
                employee UUID := '79000000-0000-0000-0000-000000000003';
                currency UUID := '79000000-0000-0000-0000-000000000004';
                supplier UUID := '79000000-0000-0000-0000-000000000005';
                requisition UUID := '79000000-0000-0000-0000-000000000006';
                requisition_line UUID := '79000000-0000-0000-0000-000000000007';
                first_order UUID := '79000000-0000-0000-0000-000000000008';
                first_order_line UUID := '79000000-0000-0000-0000-000000000009';
                second_order UUID := '79000000-0000-0000-0000-000000000010';
                second_order_line UUID := '79000000-0000-0000-0000-000000000011';
                first_receipt UUID := '79000000-0000-0000-0000-000000000012';
                first_receipt_line UUID := '79000000-0000-0000-0000-000000000013';
                second_receipt UUID := '79000000-0000-0000-0000-000000000014';
                second_receipt_line UUID := '79000000-0000-0000-0000-000000000015';
                bypass_order UUID := '79000000-0000-0000-0000-000000000017';
                bypass_receipt UUID := '79000000-0000-0000-0000-000000000018';
                error_message TEXT;
            BEGIN
                SELECT id INTO test_tenant FROM tenants WHERE deleted_at IS NULL ORDER BY created_at LIMIT 1;
                IF test_tenant IS NULL THEN
                    RAISE EXCEPTION 'The Procurement database contract test requires one tenant';
                END IF;

                INSERT INTO users (id, tenant_id, email, password_hash, full_name)
                VALUES
                    (creator, test_tenant, 'procurement-contract-creator@example.invalid',
                        'not-a-login', 'Contract Creator'),
                    (reviewer, test_tenant, 'procurement-contract-reviewer@example.invalid',
                        'not-a-login', 'Contract Reviewer'),
                    (preparer, test_tenant, 'procurement-contract-preparer@example.invalid',
                        'not-a-login', 'Contract Preparer');
                INSERT INTO employees (
                    id, tenant_id, account_id, employee_number, display_name, employment_status
                ) VALUES (
                    employee, test_tenant, creator, 'EMP-CONTRACT-079',
                    'Contract Requester', 'active'
                );
                INSERT INTO finance_currencies (
                    id, tenant_id, code, name, minor_units, is_reporting, status
                ) VALUES (currency, test_tenant, 'QZZ', 'Contract Currency', 2, FALSE, 'active');
                INSERT INTO procurement_suppliers (
                    id, tenant_id, supplier_number, legal_name, status,
                    idempotency_key, created_by
                ) VALUES (
                    supplier, test_tenant, 'SUP-CONTRACT-079', 'Contract Supplier',
                    'active', 'supplier-contract-079', creator
                );
                INSERT INTO procurement_requisitions (
                    id, tenant_id, requisition_number, requester_employee_id,
                    requester_account_id, requester_employee_number, requester_name,
                    currency_id, currency_code, currency_minor_units, title,
                    idempotency_key, created_by
                ) VALUES (
                    requisition, test_tenant, 'REQ-CONTRACT-079', employee, creator,
                    'EMP-CONTRACT-079', 'Contract Requester', currency, 'QZZ', 2,
                    'Contract requisition', 'requisition-contract-079', creator
                );
                INSERT INTO procurement_requisition_lines (
                    id, tenant_id, requisition_id, line_number, description, quantity,
                    unit_label, estimated_unit_amount_minor, estimated_line_amount_minor
                ) VALUES (
                    requisition_line, test_tenant, requisition, 1,
                    'Contract exercise books', 10, 'each', 100, 1000
                );
                UPDATE procurement_requisitions
                   SET status = 'submitted', submitted_by = creator, submitted_at = NOW(),
                       version = version + 1
                 WHERE id = requisition;
                UPDATE procurement_requisitions
                   SET status = 'approved', decided_by = reviewer, decided_at = NOW(),
                       version = version + 1
                 WHERE id = requisition;

                BEGIN
                    INSERT INTO procurement_purchase_orders (
                        id, tenant_id, purchase_order_number, requisition_id,
                        requisition_number, requisition_title, requester_employee_id,
                        requester_account_id, requester_employee_number, requester_name,
                        supplier_id, supplier_number, supplier_name, currency_id,
                        currency_code, currency_minor_units, status, idempotency_key,
                        created_by, prepared_by, issued_by, issued_at
                    ) VALUES (
                        bypass_order, test_tenant, 'PO-799999', requisition,
                        'REQ-CONTRACT-079', 'Contract requisition', employee, creator,
                        'EMP-CONTRACT-079', 'Contract Requester', supplier,
                        'SUP-CONTRACT-079', 'Contract Supplier', currency, 'QZZ', 2,
                        'received', 'purchase-order-contract-079-bypass', creator,
                        creator, reviewer, NOW()
                    );
                    RAISE EXCEPTION 'Terminal purchase order insert guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Purchase orders must begin in draft at version one' THEN
                        RAISE;
                    END IF;
                END;

                INSERT INTO procurement_purchase_orders (
                    id, tenant_id, purchase_order_number, requisition_id, requisition_number,
                    requisition_title, requester_employee_id, requester_account_id,
                    requester_employee_number, requester_name, supplier_id, supplier_number,
                    supplier_name, currency_id, currency_code, currency_minor_units,
                    idempotency_key, created_by, prepared_by
                ) VALUES (
                    first_order, test_tenant, 'PO-790001', requisition, 'REQ-CONTRACT-079',
                    'Contract requisition', employee, creator, 'EMP-CONTRACT-079',
                    'Contract Requester', supplier, 'SUP-CONTRACT-079', 'Contract Supplier',
                    currency, 'QZZ', 2, 'purchase-order-contract-079-1', creator, creator
                );
                INSERT INTO procurement_purchase_order_lines (
                    id, tenant_id, purchase_order_id, line_number, requisition_line_id,
                    requisition_line_number, description, unit_label,
                    requisition_quantity_minor, quantity_minor, quantity_scale,
                    unit_amount_minor, line_amount_minor
                ) VALUES (
                    first_order_line, test_tenant, first_order, 1, requisition_line, 1,
                    'Contract exercise books', 'each', 10, 6, 0, 100, 600
                );
                UPDATE procurement_purchase_orders
                   SET prepared_by = preparer, notes = 'Prepared by another actor'
                 WHERE id = first_order;
                BEGIN
                    UPDATE procurement_purchase_orders
                       SET status = 'issued', issued_by = preparer,
                           issued_at = NOW(), version = 2
                     WHERE id = first_order;
                    RAISE EXCEPTION 'Purchase order preparer separation guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'A different actor must issue the purchase order' THEN
                        RAISE;
                    END IF;
                END;
                UPDATE procurement_purchase_orders
                   SET status = 'issued', issued_by = reviewer, issued_at = NOW(), version = 2
                 WHERE id = first_order;
                BEGIN
                    UPDATE procurement_purchase_orders SET status = 'partially_received'
                     WHERE id = first_order;
                    RAISE EXCEPTION 'Unproven partial receipt status guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <>
                        'Purchase order partial receipt status requires posted partial receipts' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    UPDATE procurement_purchase_orders SET status = 'received'
                     WHERE id = first_order;
                    RAISE EXCEPTION 'Unproven received status guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <>
                        'Purchase order received status requires fully posted receipts' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    UPDATE procurement_purchase_orders SET prepared_by = creator
                     WHERE id = first_order;
                    RAISE EXCEPTION 'Purchase order preparer immutability guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Purchase order preparer is immutable after draft' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    INSERT INTO procurement_goods_receipts (
                        id, tenant_id, goods_receipt_number, purchase_order_id,
                        purchase_order_number, requisition_id, requisition_number,
                        supplier_id, supplier_number, supplier_name, currency_id,
                        currency_code, currency_minor_units, received_on, status,
                        idempotency_key, created_by, prepared_by, posted_by, posted_at
                    ) VALUES (
                        bypass_receipt, test_tenant, 'GRN-799999', first_order,
                        'PO-790001', requisition, 'REQ-CONTRACT-079', supplier,
                        'SUP-CONTRACT-079', 'Contract Supplier', currency, 'QZZ', 2,
                        CURRENT_DATE, 'posted', 'goods-receipt-contract-079-bypass',
                        creator, creator, reviewer, NOW()
                    );
                    RAISE EXCEPTION 'Posted goods receipt insert guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Goods receipts must begin in draft at version one' THEN
                        RAISE;
                    END IF;
                END;

                INSERT INTO procurement_purchase_orders (
                    id, tenant_id, purchase_order_number, requisition_id, requisition_number,
                    requisition_title, requester_employee_id, requester_account_id,
                    requester_employee_number, requester_name, supplier_id, supplier_number,
                    supplier_name, currency_id, currency_code, currency_minor_units,
                    idempotency_key, created_by, prepared_by
                ) VALUES (
                    second_order, test_tenant, 'PO-790002', requisition, 'REQ-CONTRACT-079',
                    'Contract requisition', employee, creator, 'EMP-CONTRACT-079',
                    'Contract Requester', supplier, 'SUP-CONTRACT-079', 'Contract Supplier',
                    currency, 'QZZ', 2, 'purchase-order-contract-079-2', creator, creator
                );
                INSERT INTO procurement_purchase_order_lines (
                    id, tenant_id, purchase_order_id, line_number, requisition_line_id,
                    requisition_line_number, description, unit_label,
                    requisition_quantity_minor, quantity_minor, quantity_scale,
                    unit_amount_minor, line_amount_minor
                ) VALUES (
                    second_order_line, test_tenant, second_order, 1, requisition_line, 1,
                    'Contract exercise books', 'each', 10, 5, 0, 100, 500
                );
                BEGIN
                    UPDATE procurement_purchase_orders
                       SET status = 'issued', issued_by = reviewer, issued_at = NOW(), version = 2
                     WHERE id = second_order;
                    RAISE EXCEPTION 'Aggregate requisition quantity guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Issued purchase order quantities cannot exceed the requisition' THEN
                        RAISE;
                    END IF;
                END;
                UPDATE procurement_purchase_order_lines
                   SET quantity_minor = 4, line_amount_minor = 400
                 WHERE id = second_order_line;
                UPDATE procurement_purchase_orders
                   SET status = 'issued', issued_by = reviewer, issued_at = NOW(), version = 2
                 WHERE id = second_order;

                BEGIN
                    UPDATE procurement_purchase_orders SET currency_code = 'ABC'
                     WHERE id = first_order;
                    RAISE EXCEPTION 'Purchase order snapshot guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Purchase order source snapshots are immutable' THEN
                        RAISE;
                    END IF;
                END;

                INSERT INTO procurement_goods_receipts (
                    id, tenant_id, goods_receipt_number, purchase_order_id,
                    purchase_order_number, requisition_id, requisition_number,
                    supplier_id, supplier_number, supplier_name, currency_id,
                    currency_code, currency_minor_units, received_on,
                    idempotency_key, created_by, prepared_by
                ) VALUES (
                    first_receipt, test_tenant, 'GRN-790001', first_order, 'PO-790001',
                    requisition, 'REQ-CONTRACT-079', supplier, 'SUP-CONTRACT-079',
                    'Contract Supplier', currency, 'QZZ', 2, CURRENT_DATE,
                    'goods-receipt-contract-079-1', creator, creator
                );
                INSERT INTO procurement_goods_receipt_lines (
                    id, tenant_id, goods_receipt_id, line_number, purchase_order_line_id,
                    purchase_order_line_number, requisition_line_id, description,
                    unit_label, quantity_minor, quantity_scale
                ) VALUES (
                    first_receipt_line, test_tenant, first_receipt, 1, first_order_line,
                    1, requisition_line, 'Contract exercise books', 'each', 7, 0
                );
                UPDATE procurement_goods_receipts
                   SET prepared_by = preparer, notes = 'Prepared by another actor'
                 WHERE id = first_receipt;
                BEGIN
                    UPDATE procurement_goods_receipts
                       SET status = 'posted', posted_by = preparer,
                           posted_at = NOW(), version = 2
                     WHERE id = first_receipt;
                    RAISE EXCEPTION 'Goods receipt preparer separation guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'A different actor must post the goods receipt' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    UPDATE procurement_goods_receipts
                       SET status = 'posted', posted_by = reviewer, posted_at = NOW(), version = 2
                     WHERE id = first_receipt;
                    RAISE EXCEPTION 'Cumulative receipt quantity guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Posted receipt quantities cannot exceed the purchase order' THEN
                        RAISE;
                    END IF;
                END;
                IF (SELECT status FROM procurement_purchase_orders WHERE id = first_order) <> 'issued' THEN
                    RAISE EXCEPTION 'Failed receipt posting changed purchase order status';
                END IF;
                UPDATE procurement_goods_receipt_lines SET quantity_minor = 2
                 WHERE id = first_receipt_line;
                UPDATE procurement_goods_receipts
                   SET status = 'posted', posted_by = reviewer, posted_at = NOW(), version = 2
                 WHERE id = first_receipt;
                IF (SELECT status FROM procurement_purchase_orders WHERE id = first_order)
                    <> 'partially_received' THEN
                    RAISE EXCEPTION 'Partial receipt did not atomically update purchase order status';
                END IF;
                BEGIN
                    UPDATE procurement_purchase_orders SET status = 'received'
                     WHERE id = first_order;
                    RAISE EXCEPTION 'Premature received status guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <>
                        'Purchase order received status requires fully posted receipts' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    UPDATE procurement_goods_receipts SET prepared_by = creator
                     WHERE id = first_receipt;
                    RAISE EXCEPTION 'Goods receipt preparer immutability guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Goods receipt preparer is immutable after draft' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    UPDATE procurement_goods_receipt_lines SET quantity_minor = 1
                     WHERE id = first_receipt_line;
                    RAISE EXCEPTION 'Posted receipt line immutability guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Only draft goods receipt lines can change' THEN
                        RAISE;
                    END IF;
                END;

                INSERT INTO procurement_goods_receipts (
                    id, tenant_id, goods_receipt_number, purchase_order_id,
                    purchase_order_number, requisition_id, requisition_number,
                    supplier_id, supplier_number, supplier_name, currency_id,
                    currency_code, currency_minor_units, received_on,
                    idempotency_key, created_by, prepared_by
                ) VALUES (
                    second_receipt, test_tenant, 'GRN-790002', first_order, 'PO-790001',
                    requisition, 'REQ-CONTRACT-079', supplier, 'SUP-CONTRACT-079',
                    'Contract Supplier', currency, 'QZZ', 2, CURRENT_DATE,
                    'goods-receipt-contract-079-2', creator, creator
                );
                INSERT INTO procurement_goods_receipt_lines (
                    id, tenant_id, goods_receipt_id, line_number, purchase_order_line_id,
                    purchase_order_line_number, requisition_line_id, description,
                    unit_label, quantity_minor, quantity_scale
                ) VALUES (
                    second_receipt_line, test_tenant, second_receipt, 1, first_order_line,
                    1, requisition_line, 'Contract exercise books', 'each', 4, 0
                );
                UPDATE procurement_goods_receipts
                   SET status = 'posted', posted_by = reviewer, posted_at = NOW(), version = 2
                 WHERE id = second_receipt;
                IF (SELECT status FROM procurement_purchase_orders WHERE id = first_order)
                    <> 'received' THEN
                    RAISE EXCEPTION 'Full receipt did not atomically complete purchase order';
                END IF;
            END;
            $$;
            "#,
        )
        .execute(&mut *transaction)
        .await
        .expect("Procurement database lifecycle contract failed");
        transaction
            .rollback()
            .await
            .expect("Procurement database contract transaction did not roll back");
    }
}
