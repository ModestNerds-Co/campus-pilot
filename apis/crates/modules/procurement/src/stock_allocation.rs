//! Exposes reduced posted-goods-receipt snapshots to the stock owner.
//!
//! Procurement retains receipt lifecycle ownership. Consumers may list stable
//! posted snapshots or lock one receipt and its lines inside their transaction.

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use chrono::NaiveDate;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

/// Reduced immutable receipt header needed by stock allocation workflows.
#[derive(Debug, Clone)]
pub struct PostedGoodsReceiptStockSource {
    pub id: Uuid,
    pub goods_receipt_number: String,
    pub purchase_order_id: Uuid,
    pub purchase_order_number: String,
    pub supplier_id: Uuid,
    pub supplier_number: String,
    pub supplier_name: String,
    pub received_on: NaiveDate,
    pub delivery_reference: Option<String>,
    pub lines: Vec<PostedGoodsReceiptLineStockSource>,
}

/// Reduced immutable receipt line needed to validate exact stock quantities.
#[derive(Debug, Clone, FromRow)]
pub struct PostedGoodsReceiptLineStockSource {
    pub id: Uuid,
    pub line_number: i32,
    pub description: String,
    pub unit_label: Option<String>,
    pub quantity_minor: i64,
    pub quantity_scale: i16,
}

#[derive(Debug, Clone, FromRow)]
struct PostedGoodsReceiptLineStockSourceRow {
    goods_receipt_id: Uuid,
    id: Uuid,
    line_number: i32,
    description: String,
    unit_label: Option<String>,
    quantity_minor: i64,
    quantity_scale: i16,
}

impl From<PostedGoodsReceiptLineStockSourceRow> for PostedGoodsReceiptLineStockSource {
    fn from(value: PostedGoodsReceiptLineStockSourceRow) -> Self {
        Self {
            id: value.id,
            line_number: value.line_number,
            description: value.description,
            unit_label: value.unit_label,
            quantity_minor: value.quantity_minor,
            quantity_scale: value.quantity_scale,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
struct PostedGoodsReceiptHeader {
    id: Uuid,
    goods_receipt_number: String,
    purchase_order_id: Uuid,
    purchase_order_number: String,
    supplier_id: Uuid,
    supplier_number: String,
    supplier_name: String,
    received_on: NaiveDate,
    delivery_reference: Option<String>,
}

impl PostedGoodsReceiptHeader {
    fn with_lines(
        self,
        lines: Vec<PostedGoodsReceiptLineStockSource>,
    ) -> PostedGoodsReceiptStockSource {
        PostedGoodsReceiptStockSource {
            id: self.id,
            goods_receipt_number: self.goods_receipt_number,
            purchase_order_id: self.purchase_order_id,
            purchase_order_number: self.purchase_order_number,
            supplier_id: self.supplier_id,
            supplier_number: self.supplier_number,
            supplier_name: self.supplier_name,
            received_on: self.received_on,
            delivery_reference: self.delivery_reference,
            lines,
        }
    }
}

/// Lists posted Procurement receipts without exposing draft lifecycle fields.
pub async fn list_posted_goods_receipt_stock_sources(
    pool: &PgPool,
    tenant_id: Uuid,
    page: i64,
    per_page: i64,
    search: Option<&str>,
    goods_receipt_id: Option<Uuid>,
) -> Result<(Vec<PostedGoodsReceiptStockSource>, i64)> {
    list_posted_goods_receipt_stock_sources_with_search_scope(
        pool,
        tenant_id,
        page,
        per_page,
        search,
        goods_receipt_id,
        false,
    )
    .await
}

/// Lists the same reduced sources while searching only fields returned to Agent callers.
pub async fn list_projected_posted_goods_receipt_stock_sources(
    pool: &PgPool,
    tenant_id: Uuid,
    page: i64,
    per_page: i64,
    search: Option<&str>,
    goods_receipt_id: Option<Uuid>,
) -> Result<(Vec<PostedGoodsReceiptStockSource>, i64)> {
    list_posted_goods_receipt_stock_sources_with_search_scope(
        pool,
        tenant_id,
        page,
        per_page,
        search,
        goods_receipt_id,
        true,
    )
    .await
}

async fn list_posted_goods_receipt_stock_sources_with_search_scope(
    pool: &PgPool,
    tenant_id: Uuid,
    page: i64,
    per_page: i64,
    search: Option<&str>,
    goods_receipt_id: Option<Uuid>,
    projected_fields_only: bool,
) -> Result<(Vec<PostedGoodsReceiptStockSource>, i64)> {
    let offset = (page - 1) * per_page;
    let search = search.map(|value| format!("%{value}%"));
    let headers = sqlx::query_as::<_, PostedGoodsReceiptHeader>(
        r#"
        SELECT id, goods_receipt_number, purchase_order_id, purchase_order_number,
               supplier_id, supplier_number, supplier_name, received_on,
               delivery_reference
          FROM procurement_goods_receipts
         WHERE tenant_id = $1 AND status = 'posted' AND deleted_at IS NULL
           AND ($2::TEXT IS NULL OR goods_receipt_number ILIKE $2
                OR purchase_order_number ILIKE $2
                OR (NOT $4 AND (supplier_number ILIKE $2
                    OR supplier_name ILIKE $2 OR delivery_reference ILIKE $2)))
           AND ($3::UUID IS NULL OR id = $3)
         ORDER BY received_on DESC, goods_receipt_number DESC
         LIMIT $5 OFFSET $6
        "#,
    )
    .bind(tenant_id)
    .bind(&search)
    .bind(goods_receipt_id)
    .bind(projected_fields_only)
    .bind(per_page)
    .bind(offset)
    .fetch_all(pool)
    .await
    .context("Failed to list posted Procurement receipt stock sources")?;
    let total = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
          FROM procurement_goods_receipts
         WHERE tenant_id = $1 AND status = 'posted' AND deleted_at IS NULL
           AND ($2::TEXT IS NULL OR goods_receipt_number ILIKE $2
                OR purchase_order_number ILIKE $2
                OR (NOT $4 AND (supplier_number ILIKE $2
                    OR supplier_name ILIKE $2 OR delivery_reference ILIKE $2)))
           AND ($3::UUID IS NULL OR id = $3)
        "#,
    )
    .bind(tenant_id)
    .bind(&search)
    .bind(goods_receipt_id)
    .bind(projected_fields_only)
    .fetch_one(pool)
    .await
    .context("Failed to count posted Procurement receipt stock sources")?;

    let header_ids = headers.iter().map(|header| header.id).collect::<Vec<_>>();
    let line_rows = if header_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as::<_, PostedGoodsReceiptLineStockSourceRow>(
            r#"
            SELECT goods_receipt_id, id, line_number, description, unit_label,
                   quantity_minor, quantity_scale
              FROM procurement_goods_receipt_lines
             WHERE tenant_id = $1 AND goods_receipt_id = ANY($2)
               AND deleted_at IS NULL
             ORDER BY goods_receipt_id, line_number
            "#,
        )
        .bind(tenant_id)
        .bind(&header_ids)
        .fetch_all(pool)
        .await
        .context("Failed to list posted Procurement receipt stock source lines")?
    };
    let mut lines_by_receipt = BTreeMap::<Uuid, Vec<PostedGoodsReceiptLineStockSource>>::new();
    for row in line_rows {
        lines_by_receipt
            .entry(row.goods_receipt_id)
            .or_default()
            .push(row.into());
    }
    let sources = headers
        .into_iter()
        .map(|header| {
            let lines = lines_by_receipt.remove(&header.id).unwrap_or_default();
            header.with_lines(lines)
        })
        .collect();
    Ok((sources, total))
}

/// Locks one posted receipt and its lines in canonical order for allocation.
///
/// The caller must keep the supplied SQL transaction open through its own
/// allocation-capacity check and stock post.
pub async fn lock_posted_goods_receipt_stock_source(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    goods_receipt_id: Uuid,
) -> Result<Option<PostedGoodsReceiptStockSource>> {
    let header = sqlx::query_as::<_, PostedGoodsReceiptHeader>(
        r#"
        SELECT id, goods_receipt_number, purchase_order_id, purchase_order_number,
               supplier_id, supplier_number, supplier_name, received_on,
               delivery_reference
          FROM procurement_goods_receipts
         WHERE tenant_id = $1 AND id = $2 AND status = 'posted'
           AND deleted_at IS NULL
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(goods_receipt_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock posted Procurement receipt stock source")?;
    let Some(header) = header else {
        return Ok(None);
    };
    let lines = sqlx::query_as::<_, PostedGoodsReceiptLineStockSource>(
        r#"
        SELECT id, line_number, description, unit_label,
               quantity_minor, quantity_scale
          FROM procurement_goods_receipt_lines
         WHERE tenant_id = $1 AND goods_receipt_id = $2
           AND deleted_at IS NULL
         ORDER BY id
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(header.id)
    .fetch_all(&mut **transaction)
    .await
    .context("Failed to lock posted Procurement receipt stock source lines")?;
    Ok(Some(header.with_lines(lines)))
}
