//! Transactional operations for the immutable stock ledger and its projection.
//!
//! Every mutation allocates one tenant-local movement, posts it atomically, and
//! appends actor-aware audit evidence. Procurement is consumed only through its
//! reduced typed posted-receipt boundary.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, anyhow, bail};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_procurement::stock_allocation::{
    PostedGoodsReceiptLineStockSource, PostedGoodsReceiptStockSource,
    list_posted_goods_receipt_stock_sources, list_projected_posted_goods_receipt_stock_sources,
    lock_posted_goods_receipt_stock_source,
};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::stock_dtos::{
    AdjustStockRequest, AllocateGoodsReceiptRequest, GoodsReceiptAllocationLineResponse,
    GoodsReceiptAllocationResponse, IssueStockRequest, ManualReceiptRequest,
    ReverseStockMovementRequest, StockBalanceResponse, StockMovementLineResponse,
    StockMovementResponse, StockMovementSummaryResponse, TransferStockRequest,
};
use crate::stock_models::{
    ItemStockSnapshot, OriginalMovementLineRecord, OriginalMovementRecord, StockBalanceRecord,
    StockMovementLineRecord, StockMovementSummaryRecord, StoreStockSnapshot,
};

const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
const MAX_REQUEST_LINES: usize = 200;
const MAX_POSTED_LINES: usize = MAX_REQUEST_LINES * 2;
const MAX_SEARCH_LENGTH: usize = 200;
pub const MAX_GOODS_RECEIPT_ALLOCATION_RECEIPTS_PER_PAGE: i64 = 5;

/// Read operations over the guarded current-balance projection.
pub struct StockBalanceOps;

impl StockBalanceOps {
    #[allow(
        clippy::too_many_arguments,
        reason = "mirrors the bounded HTTP filters"
    )]
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        item_id: Option<Uuid>,
        store_id: Option<Uuid>,
    ) -> Result<(Vec<StockBalanceResponse>, i64)> {
        let (page, per_page) = bounded_page(page, per_page);
        let search = search_pattern(search)?;
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, StockBalanceRecord>(
            r#"
            SELECT balance.item_id, item.item_number, item.name AS item_name,
                   balance.store_id, store.store_number, store.name AS store_name,
                   balance.on_hand_minor, balance.quantity_scale, balance.unit_label,
                   balance.version, balance.updated_at
              FROM assets_inventory_stock_balances AS balance
              JOIN assets_inventory_items AS item
                ON item.id = balance.item_id AND item.tenant_id = balance.tenant_id
              JOIN assets_inventory_stores AS store
                ON store.id = balance.store_id AND store.tenant_id = balance.tenant_id
             WHERE balance.tenant_id = $1 AND balance.deleted_at IS NULL
               AND ($2::TEXT IS NULL OR item.item_number ILIKE $2 OR item.name ILIKE $2
                    OR store.store_number ILIKE $2 OR store.name ILIKE $2)
               AND ($3::UUID IS NULL OR balance.item_id = $3)
               AND ($4::UUID IS NULL OR balance.store_id = $4)
             ORDER BY item.name, item.item_number, store.name, store.store_number
             LIMIT $5 OFFSET $6
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(item_id)
        .bind(store_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list stock balances")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
              FROM assets_inventory_stock_balances AS balance
              JOIN assets_inventory_items AS item
                ON item.id = balance.item_id AND item.tenant_id = balance.tenant_id
              JOIN assets_inventory_stores AS store
                ON store.id = balance.store_id AND store.tenant_id = balance.tenant_id
             WHERE balance.tenant_id = $1 AND balance.deleted_at IS NULL
               AND ($2::TEXT IS NULL OR item.item_number ILIKE $2 OR item.name ILIKE $2
                    OR store.store_number ILIKE $2 OR store.name ILIKE $2)
               AND ($3::UUID IS NULL OR balance.item_id = $3)
               AND ($4::UUID IS NULL OR balance.store_id = $4)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(item_id)
        .bind(store_id)
        .fetch_one(pool)
        .await
        .context("Failed to count stock balances")?;
        Ok((
            rows.into_iter().map(StockBalanceResponse::from).collect(),
            total,
        ))
    }
}

/// Read and mutation operations over posted stock movements.
pub struct StockMovementOps;

impl StockMovementOps {
    #[allow(
        clippy::too_many_arguments,
        reason = "mirrors the bounded HTTP filters"
    )]
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        kind: Option<&str>,
        item_id: Option<Uuid>,
        store_id: Option<Uuid>,
    ) -> Result<(Vec<StockMovementSummaryResponse>, i64)> {
        let (page, per_page) = bounded_page(page, per_page);
        let search = search_pattern(search)?;
        let kind = parse_kind_filter(kind)?;
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, StockMovementSummaryRecord>(&format!(
            "{} ORDER BY movement.effective_on DESC, movement.created_at DESC LIMIT $7 OFFSET $8",
            movement_summary_query()
        ))
        .bind(tenant_id)
        .bind(&search)
        .bind(kind)
        .bind(item_id)
        .bind(store_id)
        .bind(Option::<Uuid>::None)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list stock movements")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
              FROM assets_inventory_stock_movements AS movement
             WHERE movement.tenant_id = $1 AND movement.status = 'posted'
               AND movement.deleted_at IS NULL
               AND ($2::TEXT IS NULL OR movement.movement_number ILIKE $2
                    OR movement.reference ILIKE $2 OR movement.reason ILIKE $2
                    OR movement.source_goods_receipt_number ILIKE $2)
               AND ($3::TEXT IS NULL OR movement.kind = $3)
               AND ($4::UUID IS NULL OR EXISTS (
                    SELECT 1 FROM assets_inventory_stock_movement_lines AS line
                     WHERE line.tenant_id = movement.tenant_id
                       AND line.movement_id = movement.id AND line.item_id = $4
                       AND line.deleted_at IS NULL
               ))
               AND ($5::UUID IS NULL OR EXISTS (
                    SELECT 1 FROM assets_inventory_stock_movement_lines AS line
                     WHERE line.tenant_id = movement.tenant_id
                       AND line.movement_id = movement.id AND line.store_id = $5
                       AND line.deleted_at IS NULL
               ))
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(kind)
        .bind(item_id)
        .bind(store_id)
        .fetch_one(pool)
        .await
        .context("Failed to count stock movements")?;
        Ok((
            rows.into_iter()
                .map(StockMovementSummaryResponse::from)
                .collect(),
            total,
        ))
    }

    pub async fn get(
        pool: &PgPool,
        tenant_id: Uuid,
        movement_id: Uuid,
    ) -> Result<Option<StockMovementResponse>> {
        load_movement_pool(pool, tenant_id, movement_id).await
    }

    pub async fn create_manual_receipt(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ManualReceiptRequest,
    ) -> Result<StockMovementResponse> {
        let actor_id = actor_id(actor)?;
        let header = MovementHeaderValues::parse(
            "manual_receipt",
            request.effective_on,
            request.reference.as_deref(),
            request.reason.as_deref(),
            &request.idempotency_key,
            request,
        )?;
        ensure_line_count(request.lines.len())?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start stock receipt")?;
        let movement_number = next_movement_number(&mut transaction, tenant_id).await?;
        if let Some(replayed_id) = replay_movement(
            &mut transaction,
            tenant_id,
            &header.idempotency_key,
            &header.fingerprint,
        )
        .await?
        {
            return finish_replayed_movement(transaction, pool, tenant_id, replayed_id).await;
        }
        let inputs = request
            .lines
            .iter()
            .map(|line| (line.item_id, line.store_id, line.quantity_minor))
            .collect::<Vec<_>>();
        let lines = prepare_quantity_lines(&mut transaction, tenant_id, &inputs, 1).await?;
        post_movement(
            transaction,
            tenant_id,
            actor,
            request_context,
            actor_id,
            movement_number,
            header,
            None,
            None,
            lines,
        )
        .await
    }

    pub async fn issue(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &IssueStockRequest,
    ) -> Result<StockMovementResponse> {
        let actor_id = actor_id(actor)?;
        let header = MovementHeaderValues::parse(
            "issue",
            request.effective_on,
            request.reference.as_deref(),
            request.reason.as_deref(),
            &request.idempotency_key,
            request,
        )?;
        ensure_line_count(request.lines.len())?;
        let mut transaction = pool.begin().await.context("Failed to start stock issue")?;
        let movement_number = next_movement_number(&mut transaction, tenant_id).await?;
        if let Some(replayed_id) = replay_movement(
            &mut transaction,
            tenant_id,
            &header.idempotency_key,
            &header.fingerprint,
        )
        .await?
        {
            return finish_replayed_movement(transaction, pool, tenant_id, replayed_id).await;
        }
        let inputs = request
            .lines
            .iter()
            .map(|line| (line.item_id, line.store_id, line.quantity_minor))
            .collect::<Vec<_>>();
        let lines = prepare_quantity_lines(&mut transaction, tenant_id, &inputs, -1).await?;
        post_movement(
            transaction,
            tenant_id,
            actor,
            request_context,
            actor_id,
            movement_number,
            header,
            None,
            None,
            lines,
        )
        .await
    }

    pub async fn transfer(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &TransferStockRequest,
    ) -> Result<StockMovementResponse> {
        let actor_id = actor_id(actor)?;
        let header = MovementHeaderValues::parse(
            "transfer",
            request.effective_on,
            request.reference.as_deref(),
            request.reason.as_deref(),
            &request.idempotency_key,
            request,
        )?;
        ensure_line_count(request.lines.len())?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start stock transfer")?;
        let movement_number = next_movement_number(&mut transaction, tenant_id).await?;
        if let Some(replayed_id) = replay_movement(
            &mut transaction,
            tenant_id,
            &header.idempotency_key,
            &header.fingerprint,
        )
        .await?
        {
            return finish_replayed_movement(transaction, pool, tenant_id, replayed_id).await;
        }
        let lines = prepare_transfer_lines(&mut transaction, tenant_id, request).await?;
        post_movement(
            transaction,
            tenant_id,
            actor,
            request_context,
            actor_id,
            movement_number,
            header,
            None,
            None,
            lines,
        )
        .await
    }

    pub async fn adjust(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &AdjustStockRequest,
    ) -> Result<StockMovementResponse> {
        let actor_id = actor_id(actor)?;
        let header = MovementHeaderValues::parse(
            "adjustment",
            request.effective_on,
            request.reference.as_deref(),
            Some(&request.reason),
            &request.idempotency_key,
            request,
        )?;
        ensure_line_count(request.lines.len())?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start stock adjustment")?;
        let movement_number = next_movement_number(&mut transaction, tenant_id).await?;
        if let Some(replayed_id) = replay_movement(
            &mut transaction,
            tenant_id,
            &header.idempotency_key,
            &header.fingerprint,
        )
        .await?
        {
            return finish_replayed_movement(transaction, pool, tenant_id, replayed_id).await;
        }
        let lines = prepare_adjustment_lines(&mut transaction, tenant_id, request).await?;
        post_movement(
            transaction,
            tenant_id,
            actor,
            request_context,
            actor_id,
            movement_number,
            header,
            None,
            None,
            lines,
        )
        .await
    }

    pub async fn reverse(
        pool: &PgPool,
        tenant_id: Uuid,
        movement_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &ReverseStockMovementRequest,
    ) -> Result<Option<StockMovementResponse>> {
        let actor_id = actor_id(actor)?;
        let header = MovementHeaderValues::parse(
            "reversal",
            request.effective_on,
            None,
            Some(&request.reason),
            &request.idempotency_key,
            &json!({
                "movement_id": movement_id,
                "effective_on": request.effective_on,
                "reason": request.reason,
                "idempotency_key": request.idempotency_key,
            }),
        )?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start stock reversal")?;
        let movement_number = next_movement_number(&mut transaction, tenant_id).await?;
        if let Some(replayed_id) = replay_movement(
            &mut transaction,
            tenant_id,
            &header.idempotency_key,
            &header.fingerprint,
        )
        .await?
        {
            return finish_replayed_movement(transaction, pool, tenant_id, replayed_id)
                .await
                .map(Some);
        }
        let original = sqlx::query_as::<_, OriginalMovementRecord>(
            r#"
            SELECT id, movement_number, kind, source_goods_receipt_id,
                   source_goods_receipt_number
              FROM assets_inventory_stock_movements
             WHERE tenant_id = $1 AND id = $2 AND status = 'posted'
               AND deleted_at IS NULL
             FOR UPDATE
            "#,
        )
        .bind(tenant_id)
        .bind(movement_id)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to lock original stock movement")?;
        let Some(original) = original else {
            transaction.rollback().await.ok();
            return Ok(None);
        };
        if original.kind == "reversal" {
            bail!("Reversal movements cannot themselves be reversed");
        }
        if sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM assets_inventory_stock_movements
                 WHERE tenant_id = $1 AND reverses_movement_id = $2
                   AND status = 'posted' AND deleted_at IS NULL
            )
            "#,
        )
        .bind(tenant_id)
        .bind(movement_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to inspect stock movement reversal state")?
        {
            bail!("Stock movement has already been reversed");
        }
        let originals = sqlx::query_as::<_, OriginalMovementLineRecord>(
            r#"
            SELECT id, item_id, item_number, item_name, store_id, store_number,
                   store_name, quantity_delta_minor, quantity_scale, unit_label,
                   source_goods_receipt_line_id, source_goods_receipt_line_number,
                   source_goods_receipt_description
              FROM assets_inventory_stock_movement_lines
             WHERE tenant_id = $1 AND movement_id = $2 AND deleted_at IS NULL
             ORDER BY item_id, store_id, line_number DESC
             FOR SHARE
            "#,
        )
        .bind(tenant_id)
        .bind(movement_id)
        .fetch_all(&mut *transaction)
        .await
        .context("Failed to load original stock movement lines")?;
        ensure_posted_line_count(originals.len())?;
        let mut lines = Vec::with_capacity(originals.len());
        for line in originals {
            lines.push(PreparedMovementLine {
                item_id: line.item_id,
                item_number: line.item_number,
                item_name: line.item_name,
                store_id: line.store_id,
                store_number: line.store_number,
                store_name: line.store_name,
                quantity_delta_minor: line
                    .quantity_delta_minor
                    .checked_neg()
                    .ok_or_else(|| anyhow!("Original stock quantity cannot be reversed safely"))?,
                quantity_scale: line.quantity_scale,
                unit_label: line.unit_label,
                source_goods_receipt_line_id: line.source_goods_receipt_line_id,
                source_goods_receipt_line_number: line.source_goods_receipt_line_number,
                source_goods_receipt_description: line.source_goods_receipt_description,
                reverses_movement_line_id: Some(line.id),
            });
        }
        let source_receipt = original.source_goods_receipt_id.map(|id| {
            (
                id,
                original
                    .source_goods_receipt_number
                    .clone()
                    .unwrap_or_default(),
            )
        });
        let result = post_movement(
            transaction,
            tenant_id,
            actor,
            request_context,
            actor_id,
            movement_number,
            header,
            source_receipt,
            Some((original.id, original.movement_number)),
            lines,
        )
        .await?;
        Ok(Some(result))
    }
}

/// Procurement receipt allocation reads and atomic stock posts.
pub struct GoodsReceiptAllocationOps;

impl GoodsReceiptAllocationOps {
    #[allow(
        clippy::too_many_arguments,
        reason = "mirrors the bounded HTTP filters"
    )]
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        goods_receipt_id: Option<Uuid>,
    ) -> Result<(Vec<GoodsReceiptAllocationResponse>, i64)> {
        Self::list_with_search_scope(
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

    pub async fn list_for_agent(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        goods_receipt_id: Option<Uuid>,
    ) -> Result<(Vec<GoodsReceiptAllocationResponse>, i64)> {
        Self::list_with_search_scope(
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

    #[allow(
        clippy::too_many_arguments,
        reason = "keeps human and Agent search fields explicit at the typed boundary"
    )]
    async fn list_with_search_scope(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        goods_receipt_id: Option<Uuid>,
        projected_fields_only: bool,
    ) -> Result<(Vec<GoodsReceiptAllocationResponse>, i64)> {
        let (page, per_page) = bounded_goods_receipt_allocation_page(page, per_page);
        let search = normalized_search(search)?;
        let (sources, total) = if projected_fields_only {
            list_projected_posted_goods_receipt_stock_sources(
                pool,
                tenant_id,
                page,
                per_page,
                search.as_deref(),
                goods_receipt_id,
            )
            .await?
        } else {
            list_posted_goods_receipt_stock_sources(
                pool,
                tenant_id,
                page,
                per_page,
                search.as_deref(),
                goods_receipt_id,
            )
            .await?
        };
        let source_line_ids = sources
            .iter()
            .flat_map(|source| source.lines.iter().map(|line| line.id))
            .collect::<Vec<_>>();
        let states = historical_allocation_states(pool, tenant_id, &source_line_ids).await?;
        let responses = sources
            .into_iter()
            .map(|source| allocation_response(source, &states))
            .collect::<Result<Vec<_>>>()?;
        Ok((responses, total))
    }

    pub async fn allocate(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &AllocateGoodsReceiptRequest,
    ) -> Result<StockMovementResponse> {
        let actor_id = actor_id(actor)?;
        let header = MovementHeaderValues::parse(
            "goods_receipt_allocation",
            request.effective_on,
            None,
            request.reason.as_deref(),
            &request.idempotency_key,
            request,
        )?;
        ensure_line_count(request.lines.len())?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start goods receipt allocation")?;
        let movement_number = next_movement_number(&mut transaction, tenant_id).await?;
        if let Some(replayed_id) = replay_movement(
            &mut transaction,
            tenant_id,
            &header.idempotency_key,
            &header.fingerprint,
        )
        .await?
        {
            return finish_replayed_movement(transaction, pool, tenant_id, replayed_id).await;
        }
        let source = lock_posted_goods_receipt_stock_source(
            &mut transaction,
            tenant_id,
            request.goods_receipt_id,
        )
        .await?
        .ok_or_else(|| anyhow!("Goods receipt allocation requires a posted Procurement receipt"))?;
        let lines =
            prepare_goods_receipt_lines(&mut transaction, tenant_id, request, &source).await?;
        post_movement(
            transaction,
            tenant_id,
            actor,
            request_context,
            actor_id,
            movement_number,
            header,
            Some((source.id, source.goods_receipt_number)),
            None,
            lines,
        )
        .await
    }
}

impl From<StockBalanceRecord> for StockBalanceResponse {
    fn from(value: StockBalanceRecord) -> Self {
        Self {
            item_id: value.item_id,
            item_number: value.item_number,
            item_name: value.item_name,
            store_id: value.store_id,
            store_number: value.store_number,
            store_name: value.store_name,
            on_hand_minor: value.on_hand_minor,
            quantity_scale: value.quantity_scale,
            unit_label: value.unit_label,
            version: value.version,
            updated_at: value.updated_at,
        }
    }
}

impl From<StockMovementSummaryRecord> for StockMovementSummaryResponse {
    fn from(value: StockMovementSummaryRecord) -> Self {
        Self {
            id: value.id,
            movement_number: value.movement_number,
            kind: value.kind,
            effective_on: value.effective_on,
            reference: value.reference,
            reason: value.reason,
            source_goods_receipt_id: value.source_goods_receipt_id,
            source_goods_receipt_number: value.source_goods_receipt_number,
            reverses_movement_id: value.reverses_movement_id,
            reverses_movement_number: value.reverses_movement_number,
            reversed_by_movement_id: value.reversed_by_movement_id,
            reversed_by_movement_number: value.reversed_by_movement_number,
            status: value.status,
            version: value.version,
            line_count: value.line_count,
            created_by: value.created_by,
            posted_by: value.posted_by,
            posted_at: value.posted_at,
            created_at: value.created_at,
        }
    }
}

impl From<StockMovementLineRecord> for StockMovementLineResponse {
    fn from(value: StockMovementLineRecord) -> Self {
        Self {
            id: value.id,
            line_number: value.line_number,
            item_id: value.item_id,
            item_number: value.item_number,
            item_name: value.item_name,
            store_id: value.store_id,
            store_number: value.store_number,
            store_name: value.store_name,
            quantity_delta_minor: value.quantity_delta_minor,
            quantity_scale: value.quantity_scale,
            unit_label: value.unit_label,
            on_hand_before_minor: value.on_hand_before_minor,
            on_hand_after_minor: value.on_hand_after_minor,
            source_goods_receipt_line_id: value.source_goods_receipt_line_id,
            source_goods_receipt_line_number: value.source_goods_receipt_line_number,
            source_goods_receipt_description: value.source_goods_receipt_description,
        }
    }
}

fn movement_summary_query() -> &'static str {
    r#"
    SELECT movement.id, movement.movement_number, movement.kind,
           movement.effective_on, movement.reference, movement.reason,
           movement.source_goods_receipt_id, movement.source_goods_receipt_number,
           movement.reverses_movement_id, movement.reverses_movement_number,
           reversed.id AS reversed_by_movement_id,
           reversed.movement_number AS reversed_by_movement_number,
           movement.status, movement.version,
           (SELECT COUNT(*) FROM assets_inventory_stock_movement_lines AS counted
             WHERE counted.tenant_id = movement.tenant_id
               AND counted.movement_id = movement.id AND counted.deleted_at IS NULL
           ) AS line_count,
           movement.created_by, movement.posted_by, movement.posted_at,
           movement.created_at
      FROM assets_inventory_stock_movements AS movement
      LEFT JOIN assets_inventory_stock_movements AS reversed
        ON reversed.tenant_id = movement.tenant_id
       AND reversed.reverses_movement_id = movement.id
       AND reversed.status = 'posted' AND reversed.deleted_at IS NULL
     WHERE movement.tenant_id = $1 AND movement.status = 'posted'
       AND movement.deleted_at IS NULL
       AND ($2::TEXT IS NULL OR movement.movement_number ILIKE $2
            OR movement.reference ILIKE $2 OR movement.reason ILIKE $2
            OR movement.source_goods_receipt_number ILIKE $2)
       AND ($3::TEXT IS NULL OR movement.kind = $3)
       AND ($4::UUID IS NULL OR EXISTS (
            SELECT 1 FROM assets_inventory_stock_movement_lines AS line
             WHERE line.tenant_id = movement.tenant_id
               AND line.movement_id = movement.id AND line.item_id = $4
               AND line.deleted_at IS NULL
       ))
       AND ($5::UUID IS NULL OR EXISTS (
            SELECT 1 FROM assets_inventory_stock_movement_lines AS line
             WHERE line.tenant_id = movement.tenant_id
               AND line.movement_id = movement.id AND line.store_id = $5
               AND line.deleted_at IS NULL
       ))
       AND ($6::UUID IS NULL OR movement.id = $6)
    "#
}

async fn load_movement_pool(
    pool: &PgPool,
    tenant_id: Uuid,
    movement_id: Uuid,
) -> Result<Option<StockMovementResponse>> {
    let summary = sqlx::query_as::<_, StockMovementSummaryRecord>(movement_summary_query())
        .bind(tenant_id)
        .bind(Option::<String>::None)
        .bind(Option::<String>::None)
        .bind(Option::<Uuid>::None)
        .bind(Option::<Uuid>::None)
        .bind(Some(movement_id))
        .fetch_optional(pool)
        .await
        .context("Failed to read stock movement")?;
    let Some(summary) = summary else {
        return Ok(None);
    };
    let lines = load_movement_lines_pool(pool, tenant_id, movement_id).await?;
    Ok(Some(StockMovementResponse {
        summary: summary.into(),
        lines,
    }))
}

async fn load_movement_lines_pool(
    pool: &PgPool,
    tenant_id: Uuid,
    movement_id: Uuid,
) -> Result<Vec<StockMovementLineResponse>> {
    let rows = sqlx::query_as::<_, StockMovementLineRecord>(movement_lines_query())
        .bind(tenant_id)
        .bind(movement_id)
        .fetch_all(pool)
        .await
        .context("Failed to read stock movement lines")?;
    Ok(rows
        .into_iter()
        .map(StockMovementLineResponse::from)
        .collect())
}

fn movement_lines_query() -> &'static str {
    r#"
    SELECT id, line_number, item_id, item_number, item_name, store_id,
           store_number, store_name, quantity_delta_minor, quantity_scale,
           unit_label, on_hand_before_minor, on_hand_after_minor,
           source_goods_receipt_line_id, source_goods_receipt_line_number,
           source_goods_receipt_description
      FROM assets_inventory_stock_movement_lines
     WHERE tenant_id = $1 AND movement_id = $2 AND deleted_at IS NULL
     ORDER BY line_number
    "#
}

#[derive(Debug)]
struct MovementHeaderValues {
    kind: &'static str,
    effective_on: chrono::NaiveDate,
    reference: Option<String>,
    reason: Option<String>,
    idempotency_key: String,
    fingerprint: String,
}

impl MovementHeaderValues {
    fn parse<T: Serialize>(
        kind: &'static str,
        effective_on: chrono::NaiveDate,
        reference: Option<&str>,
        reason: Option<&str>,
        idempotency_key: &str,
        fingerprint_source: &T,
    ) -> Result<Self> {
        let reference = clean_optional(reference, 200, "Stock movement reference")?;
        let reason = clean_optional(reason, 2000, "Stock movement reason")?;
        if matches!(kind, "adjustment" | "reversal") && reason.is_none() {
            bail!("Stock {kind} requires a reason");
        }
        let idempotency_key =
            clean_required(idempotency_key, 200, "Stock movement idempotency key")?;
        let fingerprint = fingerprint(
            kind,
            effective_on,
            reference.as_deref(),
            reason.as_deref(),
            &idempotency_key,
            fingerprint_source,
        )?;
        Ok(Self {
            kind,
            effective_on,
            reference,
            reason,
            idempotency_key,
            fingerprint,
        })
    }
}

#[derive(Debug, Clone)]
struct PreparedMovementLine {
    item_id: Uuid,
    item_number: String,
    item_name: String,
    store_id: Uuid,
    store_number: String,
    store_name: String,
    quantity_delta_minor: i64,
    quantity_scale: i16,
    unit_label: String,
    source_goods_receipt_line_id: Option<Uuid>,
    source_goods_receipt_line_number: Option<i32>,
    source_goods_receipt_description: Option<String>,
    reverses_movement_line_id: Option<Uuid>,
}

async fn prepare_quantity_lines(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    inputs: &[(Uuid, Uuid, i64)],
    sign: i64,
) -> Result<Vec<PreparedMovementLine>> {
    ensure_line_count(inputs.len())?;
    let mut pairs = BTreeSet::new();
    for (item_id, store_id, quantity) in inputs {
        ensure_positive_quantity(*quantity)?;
        if !pairs.insert((*item_id, *store_id)) {
            bail!("Stock movement item and store lines must be unique");
        }
    }
    let items = load_items(transaction, tenant_id, inputs.iter().map(|value| value.0)).await?;
    let stores = load_stores(transaction, tenant_id, inputs.iter().map(|value| value.1)).await?;
    inputs
        .iter()
        .map(|(item_id, store_id, quantity)| {
            prepared_line(
                items
                    .get(item_id)
                    .ok_or_else(|| anyhow!("Stock movement item is not active"))?,
                stores
                    .get(store_id)
                    .ok_or_else(|| anyhow!("Stock movement store is not active"))?,
                quantity
                    .checked_mul(sign)
                    .ok_or_else(|| anyhow!("Stock movement quantity is unsafe"))?,
            )
        })
        .collect()
}

async fn prepare_transfer_lines(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    request: &TransferStockRequest,
) -> Result<Vec<PreparedMovementLine>> {
    let mut triples = BTreeSet::new();
    for line in &request.lines {
        ensure_positive_quantity(line.quantity_minor)?;
        if line.from_store_id == line.to_store_id {
            bail!("Stock transfers require different source and destination stores");
        }
        if !triples.insert((line.item_id, line.from_store_id, line.to_store_id)) {
            bail!("Stock transfer lines must be unique");
        }
    }
    let items = load_items(
        transaction,
        tenant_id,
        request.lines.iter().map(|line| line.item_id),
    )
    .await?;
    let stores = load_stores(
        transaction,
        tenant_id,
        request
            .lines
            .iter()
            .flat_map(|line| [line.from_store_id, line.to_store_id]),
    )
    .await?;
    let mut prepared = Vec::with_capacity(request.lines.len() * 2);
    for line in &request.lines {
        let item = items
            .get(&line.item_id)
            .ok_or_else(|| anyhow!("Stock transfer item is not active"))?;
        let source = stores
            .get(&line.from_store_id)
            .ok_or_else(|| anyhow!("Stock transfer source store is not active"))?;
        let destination = stores
            .get(&line.to_store_id)
            .ok_or_else(|| anyhow!("Stock transfer destination store is not active"))?;
        prepared.push(prepared_line(item, source, -line.quantity_minor)?);
        prepared.push(prepared_line(item, destination, line.quantity_minor)?);
    }
    Ok(prepared)
}

async fn prepare_adjustment_lines(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    request: &AdjustStockRequest,
) -> Result<Vec<PreparedMovementLine>> {
    let mut pairs = BTreeSet::new();
    for line in &request.lines {
        ensure_nonnegative_quantity(line.expected_on_hand_minor)?;
        ensure_nonnegative_quantity(line.counted_on_hand_minor)?;
        if !pairs.insert((line.item_id, line.store_id)) {
            bail!("Stock adjustment item and store lines must be unique");
        }
        if line.expected_on_hand_minor == line.counted_on_hand_minor {
            bail!("Stock adjustments must change the counted balance");
        }
    }
    let items = load_items(
        transaction,
        tenant_id,
        request.lines.iter().map(|line| line.item_id),
    )
    .await?;
    let stores = load_stores(
        transaction,
        tenant_id,
        request.lines.iter().map(|line| line.store_id),
    )
    .await?;
    let mut prepared = Vec::with_capacity(request.lines.len());
    for line in &request.lines {
        let current = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT on_hand_minor
              FROM assets_inventory_stock_balances
             WHERE tenant_id = $1 AND item_id = $2 AND store_id = $3
               AND deleted_at IS NULL
             FOR UPDATE
            "#,
        )
        .bind(tenant_id)
        .bind(line.item_id)
        .bind(line.store_id)
        .fetch_optional(&mut **transaction)
        .await
        .context("Failed to lock the counted stock balance")?
        .unwrap_or(0);
        if current != line.expected_on_hand_minor {
            bail!("Stock balance changed since the adjustment was counted");
        }
        let delta = line
            .counted_on_hand_minor
            .checked_sub(line.expected_on_hand_minor)
            .ok_or_else(|| anyhow!("Stock adjustment quantity is unsafe"))?;
        prepared.push(prepared_line(
            items
                .get(&line.item_id)
                .ok_or_else(|| anyhow!("Stock adjustment item is not active"))?,
            stores
                .get(&line.store_id)
                .ok_or_else(|| anyhow!("Stock adjustment store is not active"))?,
            delta,
        )?);
    }
    Ok(prepared)
}

async fn prepare_goods_receipt_lines(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    request: &AllocateGoodsReceiptRequest,
    source: &PostedGoodsReceiptStockSource,
) -> Result<Vec<PreparedMovementLine>> {
    let source_lines = source
        .lines
        .iter()
        .map(|line| (line.id, line))
        .collect::<BTreeMap<_, _>>();
    let mut triples = BTreeSet::new();
    let mut request_mapping = BTreeMap::<Uuid, Uuid>::new();
    let mut requested = BTreeMap::<Uuid, i64>::new();
    for line in &request.lines {
        ensure_positive_quantity(line.quantity_minor)?;
        if !triples.insert((line.goods_receipt_line_id, line.item_id, line.store_id)) {
            bail!("Goods receipt allocation lines must be unique");
        }
        if let Some(mapped) = request_mapping.insert(line.goods_receipt_line_id, line.item_id)
            && mapped != line.item_id
        {
            bail!("A goods receipt line cannot be allocated to different items");
        }
        let total = requested.entry(line.goods_receipt_line_id).or_default();
        *total = total
            .checked_add(line.quantity_minor)
            .ok_or_else(|| anyhow!("Goods receipt allocation quantity is unsafe"))?;
    }
    let items = load_items(
        transaction,
        tenant_id,
        request.lines.iter().map(|line| line.item_id),
    )
    .await?;
    let stores = load_stores(
        transaction,
        tenant_id,
        request.lines.iter().map(|line| line.store_id),
    )
    .await?;

    for (source_line_id, requested_quantity) in &requested {
        let source_line = source_lines
            .get(source_line_id)
            .ok_or_else(|| anyhow!("Goods receipt allocation line is not part of the receipt"))?;
        if source_line.quantity_minor < 1 || source_line.quantity_minor > MAX_SAFE_INTEGER {
            bail!("Goods receipt quantity is outside the exact stock boundary");
        }
        let historical =
            historical_allocation_state(transaction, tenant_id, *source_line_id).await?;
        let available = source_line
            .quantity_minor
            .checked_sub(historical.allocated_quantity_minor)
            .ok_or_else(|| anyhow!("Goods receipt allocation state is inconsistent"))?;
        if *requested_quantity > available {
            bail!("Goods receipt allocation exceeds the remaining received quantity");
        }
        if let Some(historical_item_id) = historical.mapped_item_id
            && request_mapping.get(source_line_id) != Some(&historical_item_id)
        {
            bail!("A goods receipt line cannot be remapped to another item");
        }
    }

    let mut prepared = Vec::with_capacity(request.lines.len());
    for line in &request.lines {
        let source_line = source_lines
            .get(&line.goods_receipt_line_id)
            .ok_or_else(|| anyhow!("Goods receipt allocation line is not part of the receipt"))?;
        let item = items
            .get(&line.item_id)
            .ok_or_else(|| anyhow!("Goods receipt allocation item is not active"))?;
        let store = stores
            .get(&line.store_id)
            .ok_or_else(|| anyhow!("Goods receipt allocation store is not active"))?;
        validate_receipt_item_compatibility(source_line, item)?;
        let mut value = prepared_line(item, store, line.quantity_minor)?;
        value.source_goods_receipt_line_id = Some(source_line.id);
        value.source_goods_receipt_line_number = Some(source_line.line_number);
        value.source_goods_receipt_description = Some(source_line.description.clone());
        prepared.push(value);
    }
    Ok(prepared)
}

fn validate_receipt_item_compatibility(
    source: &PostedGoodsReceiptLineStockSource,
    item: &ItemStockSnapshot,
) -> Result<()> {
    let source_unit = source
        .unit_label
        .as_deref()
        .ok_or_else(|| anyhow!("Goods receipt allocation requires a source unit"))?;
    if source.quantity_scale != item.quantity_scale
        || normalize_unit(source_unit) != normalize_unit(&item.unit_label)
    {
        bail!("Goods receipt unit and quantity scale must match the selected item");
    }
    Ok(())
}

fn prepared_line(
    item: &ItemStockSnapshot,
    store: &StoreStockSnapshot,
    quantity_delta_minor: i64,
) -> Result<PreparedMovementLine> {
    if quantity_delta_minor == 0
        || !(-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&quantity_delta_minor)
    {
        bail!("Stock movement quantity must be non-zero and exact");
    }
    Ok(PreparedMovementLine {
        item_id: item.id,
        item_number: item.item_number.clone(),
        item_name: item.name.clone(),
        store_id: store.id,
        store_number: store.store_number.clone(),
        store_name: store.name.clone(),
        quantity_delta_minor,
        quantity_scale: item.quantity_scale,
        unit_label: item.unit_label.clone(),
        source_goods_receipt_line_id: None,
        source_goods_receipt_line_number: None,
        source_goods_receipt_description: None,
        reverses_movement_line_id: None,
    })
}

async fn load_items(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    ids: impl Iterator<Item = Uuid>,
) -> Result<BTreeMap<Uuid, ItemStockSnapshot>> {
    let ids = ids.collect::<BTreeSet<_>>();
    let mut items = BTreeMap::new();
    for id in ids {
        let item = sqlx::query_as::<_, ItemStockSnapshot>(
            r#"
            SELECT id, item_number, name, unit_label, quantity_scale
              FROM assets_inventory_items
             WHERE tenant_id = $1 AND id = $2 AND status = 'active'
               AND deleted_at IS NULL
             FOR SHARE
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(&mut **transaction)
        .await
        .context("Failed to lock stock movement item")?
        .ok_or_else(|| anyhow!("Stock movement item is not active"))?;
        items.insert(id, item);
    }
    Ok(items)
}

async fn load_stores(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    ids: impl Iterator<Item = Uuid>,
) -> Result<BTreeMap<Uuid, StoreStockSnapshot>> {
    let ids = ids.collect::<BTreeSet<_>>();
    let mut stores = BTreeMap::new();
    for id in ids {
        let store = sqlx::query_as::<_, StoreStockSnapshot>(
            r#"
            SELECT id, store_number, name
              FROM assets_inventory_stores
             WHERE tenant_id = $1 AND id = $2 AND status = 'active'
               AND deleted_at IS NULL
             FOR SHARE
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(&mut **transaction)
        .await
        .context("Failed to lock stock movement store")?
        .ok_or_else(|| anyhow!("Stock movement store is not active"))?;
        stores.insert(id, store);
    }
    Ok(stores)
}

#[allow(
    clippy::too_many_arguments,
    reason = "one explicit atomic movement boundary"
)]
async fn post_movement(
    mut transaction: Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    actor_id: Uuid,
    movement_number: String,
    header: MovementHeaderValues,
    source_receipt: Option<(Uuid, String)>,
    reverses_movement: Option<(Uuid, String)>,
    lines: Vec<PreparedMovementLine>,
) -> Result<StockMovementResponse> {
    if lines.is_empty() || lines.len() > MAX_POSTED_LINES {
        bail!("Stock movement line count is outside its bounded range");
    }
    let movement_id = Uuid::new_v4();
    let (source_receipt_id, source_receipt_number) = source_receipt.unzip();
    let (reverses_movement_id, reverses_movement_number) = reverses_movement.unzip();
    sqlx::query(
        r#"
        INSERT INTO assets_inventory_stock_movements (
            id, tenant_id, movement_number, kind, effective_on, reference,
            reason, source_goods_receipt_id, source_goods_receipt_number,
            reverses_movement_id, reverses_movement_number, status, version,
            idempotency_key, create_request_fingerprint, created_by
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
            'draft', 1, $12, $13, $14
        )
        "#,
    )
    .bind(movement_id)
    .bind(tenant_id)
    .bind(&movement_number)
    .bind(header.kind)
    .bind(header.effective_on)
    .bind(&header.reference)
    .bind(&header.reason)
    .bind(source_receipt_id)
    .bind(&source_receipt_number)
    .bind(reverses_movement_id)
    .bind(&reverses_movement_number)
    .bind(&header.idempotency_key)
    .bind(&header.fingerprint)
    .bind(actor_id)
    .execute(&mut *transaction)
    .await
    .context("Failed to create draft stock movement")?;

    for (index, line) in lines.iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO assets_inventory_stock_movement_lines (
                id, tenant_id, movement_id, line_number, item_id, item_number,
                item_name, store_id, store_number, store_name,
                quantity_delta_minor, quantity_scale, unit_label,
                source_goods_receipt_line_id, source_goods_receipt_line_number,
                source_goods_receipt_description, reverses_movement_line_id
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17
            )
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(tenant_id)
        .bind(movement_id)
        .bind(i32::try_from(index + 1).context("Stock movement line number overflow")?)
        .bind(line.item_id)
        .bind(&line.item_number)
        .bind(&line.item_name)
        .bind(line.store_id)
        .bind(&line.store_number)
        .bind(&line.store_name)
        .bind(line.quantity_delta_minor)
        .bind(line.quantity_scale)
        .bind(&line.unit_label)
        .bind(line.source_goods_receipt_line_id)
        .bind(line.source_goods_receipt_line_number)
        .bind(&line.source_goods_receipt_description)
        .bind(line.reverses_movement_line_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to create stock movement line")?;
    }

    sqlx::query(
        r#"
        UPDATE assets_inventory_stock_movements
           SET status = 'posted', version = version + 1,
               posted_by = $3, posted_at = NOW()
         WHERE tenant_id = $1 AND id = $2 AND status = 'draft'
        "#,
    )
    .bind(tenant_id)
    .bind(movement_id)
    .bind(actor_id)
    .execute(&mut *transaction)
    .await
    .context("Failed to post stock movement")?;

    append_stock_audit(
        &mut transaction,
        tenant_id,
        actor,
        request_context,
        header.kind,
        movement_id,
        &movement_number,
        lines.len(),
        source_receipt_number.as_deref(),
        reverses_movement_number.as_deref(),
    )
    .await?;
    let response = load_movement_transaction(&mut transaction, tenant_id, movement_id)
        .await?
        .ok_or_else(|| anyhow!("Posted stock movement could not be loaded"))?;
    transaction
        .commit()
        .await
        .context("Failed to commit stock movement")?;
    Ok(response)
}

async fn load_movement_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    movement_id: Uuid,
) -> Result<Option<StockMovementResponse>> {
    let summary = sqlx::query_as::<_, StockMovementSummaryRecord>(movement_summary_query())
        .bind(tenant_id)
        .bind(Option::<String>::None)
        .bind(Option::<String>::None)
        .bind(Option::<Uuid>::None)
        .bind(Option::<Uuid>::None)
        .bind(Some(movement_id))
        .fetch_optional(&mut **transaction)
        .await
        .context("Failed to read posted stock movement")?;
    let Some(summary) = summary else {
        return Ok(None);
    };
    let rows = sqlx::query_as::<_, StockMovementLineRecord>(movement_lines_query())
        .bind(tenant_id)
        .bind(movement_id)
        .fetch_all(&mut **transaction)
        .await
        .context("Failed to read posted stock movement lines")?;
    Ok(Some(StockMovementResponse {
        summary: summary.into(),
        lines: rows
            .into_iter()
            .map(StockMovementLineResponse::from)
            .collect(),
    }))
}

async fn next_movement_number(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
) -> Result<String> {
    let number = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO assets_inventory_movement_sequences (tenant_id, last_number)
        VALUES ($1, 1)
        ON CONFLICT (tenant_id) DO UPDATE
            SET last_number = assets_inventory_movement_sequences.last_number + 1
        RETURNING last_number
        "#,
    )
    .bind(tenant_id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to allocate stock movement number")?;
    if !(1..=999_999).contains(&number) {
        bail!("Stock movement number sequence is exhausted");
    }
    Ok(format!("MOV-{number:06}"))
}

async fn replay_movement(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    idempotency_key: &str,
    fingerprint: &str,
) -> Result<Option<Uuid>> {
    let existing = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT id, create_request_fingerprint
          FROM assets_inventory_stock_movements
         WHERE tenant_id = $1 AND idempotency_key = $2
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(idempotency_key)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to inspect stock movement idempotency")?;
    let Some((movement_id, stored_fingerprint)) = existing else {
        return Ok(None);
    };
    if stored_fingerprint != fingerprint {
        bail!("Idempotency key already belongs to another stock movement request");
    }
    Ok(Some(movement_id))
}

async fn finish_replayed_movement(
    transaction: Transaction<'_, Postgres>,
    pool: &PgPool,
    tenant_id: Uuid,
    movement_id: Uuid,
) -> Result<StockMovementResponse> {
    transaction
        .rollback()
        .await
        .context("Failed to close replayed stock movement transaction")?;
    load_movement_pool(pool, tenant_id, movement_id)
        .await?
        .ok_or_else(|| anyhow!("The idempotent stock movement could not be loaded"))
}

#[allow(
    clippy::too_many_arguments,
    reason = "audit evidence is intentionally explicit"
)]
async fn append_stock_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    kind: &str,
    movement_id: Uuid,
    movement_number: &str,
    line_count: usize,
    source_receipt_number: Option<&str>,
    reverses_movement_number: Option<&str>,
) -> Result<()> {
    let mut metadata = serde_json::Map::from_iter([
        ("movement_number".to_string(), json!(movement_number)),
        ("kind".to_string(), json!(kind)),
        ("line_count".to_string(), json!(line_count)),
    ]);
    if let Some(value) = source_receipt_number {
        metadata.insert("goods_receipt_number".to_string(), json!(value));
    }
    if let Some(value) = reverses_movement_number {
        metadata.insert("reverses_movement_number".to_string(), json!(value));
    }
    append_audit(
        &mut **transaction,
        &NewAuditEvent::new(
            tenant_id,
            actor,
            format!("assets_inventory.stock.{kind}"),
            AuditOutcome::Succeeded,
            request_context,
        )
        .with_target(AuditTarget::new(
            "assets_inventory_stock_movement",
            movement_id.to_string(),
        ))
        .with_redacted_metadata(metadata),
    )
    .await
    .context("Failed to append stock movement audit event")?;
    Ok(())
}

#[derive(Debug, FromRow)]
struct HistoricalAllocationState {
    allocated_quantity_minor: i64,
    mapped_item_id: Option<Uuid>,
    mapped_item_number: Option<String>,
    mapped_item_name: Option<String>,
}

#[derive(Debug, FromRow)]
struct HistoricalAllocationStateRow {
    source_goods_receipt_line_id: Uuid,
    allocated_quantity_minor: i64,
    mapped_item_id: Option<Uuid>,
    mapped_item_number: Option<String>,
    mapped_item_name: Option<String>,
}

async fn historical_allocation_state(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    source_line_id: Uuid,
) -> Result<HistoricalAllocationState> {
    sqlx::query_as::<_, HistoricalAllocationState>(
        r#"
        SELECT COALESCE(SUM(CASE WHEN movement.status = 'posted'
                                 THEN line.quantity_delta_minor ELSE 0 END), 0)::BIGINT
                   AS allocated_quantity_minor,
               (ARRAY_AGG(line.item_id ORDER BY movement.created_at)
                    FILTER (WHERE movement.status = 'posted'))[1] AS mapped_item_id,
               (ARRAY_AGG(line.item_number ORDER BY movement.created_at)
                    FILTER (WHERE movement.status = 'posted'))[1] AS mapped_item_number,
               (ARRAY_AGG(line.item_name ORDER BY movement.created_at)
                    FILTER (WHERE movement.status = 'posted'))[1] AS mapped_item_name
          FROM assets_inventory_stock_movement_lines AS line
          JOIN assets_inventory_stock_movements AS movement
            ON movement.id = line.movement_id AND movement.tenant_id = line.tenant_id
         WHERE line.tenant_id = $1 AND line.source_goods_receipt_line_id = $2
           AND line.deleted_at IS NULL AND movement.deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(source_line_id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to inspect goods receipt allocation state")
}

async fn historical_allocation_states(
    pool: &PgPool,
    tenant_id: Uuid,
    source_line_ids: &[Uuid],
) -> Result<BTreeMap<Uuid, HistoricalAllocationState>> {
    if source_line_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let rows = sqlx::query_as::<_, HistoricalAllocationStateRow>(
        r#"
        SELECT stock.source_goods_receipt_line_id,
               SUM(CASE WHEN movement.status = 'posted'
                        THEN stock.quantity_delta_minor ELSE 0 END)::BIGINT
                   AS allocated_quantity_minor,
               (ARRAY_AGG(stock.item_id ORDER BY movement.created_at)
                    FILTER (WHERE movement.status = 'posted'))[1] AS mapped_item_id,
               (ARRAY_AGG(stock.item_number ORDER BY movement.created_at)
                    FILTER (WHERE movement.status = 'posted'))[1] AS mapped_item_number,
               (ARRAY_AGG(stock.item_name ORDER BY movement.created_at)
                    FILTER (WHERE movement.status = 'posted'))[1] AS mapped_item_name
          FROM assets_inventory_stock_movement_lines AS stock
          JOIN assets_inventory_stock_movements AS movement
            ON movement.id = stock.movement_id AND movement.tenant_id = stock.tenant_id
         WHERE stock.tenant_id = $1
           AND stock.source_goods_receipt_line_id = ANY($2)
           AND stock.deleted_at IS NULL AND movement.deleted_at IS NULL
         GROUP BY stock.source_goods_receipt_line_id
        "#,
    )
    .bind(tenant_id)
    .bind(source_line_ids)
    .fetch_all(pool)
    .await
    .context("Failed to batch goods receipt allocation state")?;
    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.source_goods_receipt_line_id,
                HistoricalAllocationState {
                    allocated_quantity_minor: row.allocated_quantity_minor,
                    mapped_item_id: row.mapped_item_id,
                    mapped_item_number: row.mapped_item_number,
                    mapped_item_name: row.mapped_item_name,
                },
            )
        })
        .collect())
}

fn allocation_response(
    source: PostedGoodsReceiptStockSource,
    states: &BTreeMap<Uuid, HistoricalAllocationState>,
) -> Result<GoodsReceiptAllocationResponse> {
    let mut lines = Vec::with_capacity(source.lines.len());
    for line in source.lines {
        let state = states.get(&line.id);
        let allocated_quantity_minor = state
            .map(|state| state.allocated_quantity_minor)
            .unwrap_or_default();
        if allocated_quantity_minor < 0 || allocated_quantity_minor > line.quantity_minor {
            bail!("Goods receipt allocation state is inconsistent");
        }
        lines.push(GoodsReceiptAllocationLineResponse {
            id: line.id,
            line_number: line.line_number,
            description: line.description,
            unit_label: line.unit_label,
            quantity_minor: line.quantity_minor,
            quantity_scale: line.quantity_scale,
            allocated_quantity_minor,
            remaining_quantity_minor: line.quantity_minor - allocated_quantity_minor,
            mapped_item_id: state.and_then(|state| state.mapped_item_id),
            mapped_item_number: state.and_then(|state| state.mapped_item_number.clone()),
            mapped_item_name: state.and_then(|state| state.mapped_item_name.clone()),
        });
    }
    Ok(GoodsReceiptAllocationResponse {
        id: source.id,
        goods_receipt_number: source.goods_receipt_number,
        purchase_order_id: source.purchase_order_id,
        purchase_order_number: source.purchase_order_number,
        supplier_id: source.supplier_id,
        supplier_number: source.supplier_number,
        supplier_name: source.supplier_name,
        received_on: source.received_on,
        delivery_reference: source.delivery_reference,
        lines,
    })
}

fn bounded_page(page: i64, per_page: i64) -> (i64, i64) {
    (page.clamp(1, 1_000_000), per_page.clamp(1, 100))
}

pub fn bounded_goods_receipt_allocation_page(page: i64, per_page: i64) -> (i64, i64) {
    let (page, per_page) = bounded_page(page, per_page);
    (
        page,
        per_page.min(MAX_GOODS_RECEIPT_ALLOCATION_RECEIPTS_PER_PAGE),
    )
}

fn parse_kind_filter(kind: Option<&str>) -> Result<Option<&str>> {
    match kind.map(str::trim).filter(|value| !value.is_empty()) {
        None => Ok(None),
        Some(
            value @ ("manual_receipt"
            | "issue"
            | "transfer"
            | "adjustment"
            | "goods_receipt_allocation"
            | "reversal"),
        ) => Ok(Some(value)),
        Some(_) => bail!("Stock movement kind filter is invalid"),
    }
}

fn normalized_search(search: Option<&str>) -> Result<Option<String>> {
    let search = search.map(str::trim).filter(|value| !value.is_empty());
    if search.is_some_and(|value| value.chars().count() > MAX_SEARCH_LENGTH) {
        bail!("Stock search is too long");
    }
    Ok(search.map(str::to_string))
}

fn search_pattern(search: Option<&str>) -> Result<Option<String>> {
    Ok(normalized_search(search)?.map(|value| format!("%{value}%")))
}

fn clean_optional(value: Option<&str>, max: usize, label: &str) -> Result<Option<String>> {
    let value = value.map(str::trim).filter(|value| !value.is_empty());
    if value.is_some_and(|value| value.chars().count() > max) {
        bail!("{label} is too long");
    }
    Ok(value.map(str::to_string))
}

fn clean_required(value: &str, max: usize, label: &str) -> Result<String> {
    clean_optional(Some(value), max, label)?.ok_or_else(|| anyhow!("{label} is required"))
}

fn ensure_line_count(count: usize) -> Result<()> {
    if !(1..=MAX_REQUEST_LINES).contains(&count) {
        bail!("Stock movement requires between one and {MAX_REQUEST_LINES} input lines");
    }
    Ok(())
}

fn ensure_posted_line_count(count: usize) -> Result<()> {
    if !(1..=MAX_POSTED_LINES).contains(&count) {
        bail!("Posted stock movement requires between one and {MAX_POSTED_LINES} lines");
    }
    Ok(())
}

fn ensure_positive_quantity(value: i64) -> Result<()> {
    if !(1..=MAX_SAFE_INTEGER).contains(&value) {
        bail!("Stock quantity must be positive and exactly representable");
    }
    Ok(())
}

fn ensure_nonnegative_quantity(value: i64) -> Result<()> {
    if !(0..=MAX_SAFE_INTEGER).contains(&value) {
        bail!("Stock quantity must be non-negative and exactly representable");
    }
    Ok(())
}

fn normalize_unit(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn fingerprint<T: Serialize>(
    kind: &str,
    effective_on: chrono::NaiveDate,
    reference: Option<&str>,
    reason: Option<&str>,
    idempotency_key: &str,
    value: &T,
) -> Result<String> {
    let mut request =
        serde_json::to_value(value).context("Failed to fingerprint stock movement request")?;
    if let Some(object) = request.as_object_mut() {
        object.remove("effective_on");
        object.remove("reference");
        object.remove("reason");
        object.remove("idempotency_key");
    }
    let bytes = serde_json::to_vec(&json!({
        "kind": kind,
        "effective_on": effective_on,
        "reference": reference,
        "reason": reason,
        "idempotency_key": idempotency_key,
        "request": request,
    }))
    .context("Failed to encode stock movement fingerprint")?;
    let mut digest = Sha256::new();
    digest.update(bytes);
    Ok(format!("{:x}", digest.finalize()))
}

fn actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("Authenticated person or Agent actor is required"))
}

#[cfg(test)]
mod tests {
    use actix_web::rt;
    use cp_audit::{AuditActor, RequestContext};
    use cp_procurement::goods_receipts::{
        CreateGoodsReceiptRequest, GoodsReceiptLineInput, GoodsReceiptOps,
    };
    use cp_procurement::purchase_orders::{
        CreatePurchaseOrderRequest, PurchaseOrderLineInput, PurchaseOrderOps,
    };
    use cp_procurement::requisitions::{
        CreateRequisitionRequest, DecisionRequest, RequisitionLineInput, RequisitionOps,
    };
    use cp_procurement::suppliers::{CreateSupplierRequest, SupplierOps};
    use sqlx::postgres::PgPoolOptions;
    use uuid::Uuid;

    use crate::dtos::{CreateItemRequest, CreateStoreRequest, UpdateItemRequest};
    use crate::stock_dtos::{
        AdjustStockLineInput, AdjustStockRequest, AllocateGoodsReceiptLineInput,
        AllocateGoodsReceiptRequest, IssueStockRequest, ManualReceiptRequest,
        ReverseStockMovementRequest, StockQuantityLineInput, TransferStockLineInput,
        TransferStockRequest,
    };
    use crate::{AssetStatus, ItemOps, StoreOps};

    use super::{
        GoodsReceiptAllocationOps, MAX_SAFE_INTEGER, StockBalanceOps, StockMovementOps,
        bounded_goods_receipt_allocation_page, ensure_line_count, ensure_nonnegative_quantity,
        ensure_positive_quantity, ensure_posted_line_count, normalize_unit, parse_kind_filter,
        search_pattern,
    };

    #[test]
    fn quantity_and_line_boundaries_are_exact() {
        assert!(ensure_positive_quantity(1).is_ok());
        assert!(ensure_positive_quantity(MAX_SAFE_INTEGER).is_ok());
        assert!(ensure_positive_quantity(0).is_err());
        assert!(ensure_positive_quantity(MAX_SAFE_INTEGER + 1).is_err());
        assert!(ensure_nonnegative_quantity(0).is_ok());
        assert!(ensure_nonnegative_quantity(-1).is_err());
        assert!(ensure_line_count(1).is_ok());
        assert!(ensure_line_count(200).is_ok());
        assert!(ensure_line_count(0).is_err());
        assert!(ensure_line_count(201).is_err());
        assert!(ensure_posted_line_count(200).is_ok());
        assert!(ensure_posted_line_count(400).is_ok());
        assert!(ensure_posted_line_count(0).is_err());
        assert!(ensure_posted_line_count(401).is_err());
        assert_eq!(bounded_goods_receipt_allocation_page(1, 100), (1, 5));
    }

    #[test]
    fn filters_and_units_are_canonical() {
        assert_eq!(parse_kind_filter(Some(" issue ")).unwrap(), Some("issue"));
        assert!(parse_kind_filter(Some("delete")).is_err());
        assert_eq!(
            search_pattern(Some(" chalk ")).unwrap(),
            Some("%chalk%".into())
        );
        assert_eq!(search_pattern(Some("  ")).unwrap(), None);
        assert_eq!(normalize_unit(" Box   EACH "), "box each");
    }

    #[actix_web::test]
    #[ignore = "requires STOCK_LEDGER_TEST_DATABASE_URL with migrations through 082"]
    async fn postgres_ledger_serializes_stock_and_rejects_direct_tampering() {
        let database_url = std::env::var("STOCK_LEDGER_TEST_DATABASE_URL")
            .expect("STOCK_LEDGER_TEST_DATABASE_URL must target a disposable database");
        let pool = PgPoolOptions::new()
            .max_connections(12)
            .connect(&database_url)
            .await
            .expect("disposable PostgreSQL database must be available");
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, $2, 'Stock test')")
            .bind(tenant_id)
            .bind(format!("stock-{tenant_id}"))
            .execute(&pool)
            .await
            .expect("tenant fixture");
        sqlx::query(
            "INSERT INTO users (id, tenant_id, email, password_hash, full_name) VALUES ($1, $2, $3, 'x', 'Stock Tester')",
        )
        .bind(actor_id)
        .bind(tenant_id)
        .bind(format!("stock-{actor_id}@example.test"))
        .execute(&pool)
        .await
        .expect("user fixture");
        let actor = AuditActor::person(actor_id);
        let item = ItemOps::create(
            &pool,
            tenant_id,
            actor,
            RequestContext::generate(None),
            &CreateItemRequest {
                name: "Exercise book".into(),
                description: None,
                barcode: None,
                unit_label: "each".into(),
                quantity_scale: 0,
                reorder_level_minor: None,
                idempotency_key: format!("item-{tenant_id}"),
            },
        )
        .await
        .expect("item fixture");
        let store = StoreOps::create(
            &pool,
            tenant_id,
            actor,
            RequestContext::generate(None),
            &CreateStoreRequest {
                name: "Main store".into(),
                location_label: None,
                notes: None,
                idempotency_key: format!("store-{tenant_id}"),
            },
        )
        .await
        .expect("store fixture");
        let item_id = item.id;
        let store_id = store.id;
        let today = chrono::Utc::now().date_naive();
        let receipt_request = ManualReceiptRequest {
            effective_on: today,
            reference: Some("Opening count".into()),
            reason: None,
            idempotency_key: format!("receipt-{tenant_id}"),
            lines: vec![StockQuantityLineInput {
                item_id,
                store_id,
                quantity_minor: 10,
            }],
        };
        let receipt = StockMovementOps::create_manual_receipt(
            &pool,
            tenant_id,
            actor,
            RequestContext::generate(None),
            &receipt_request,
        )
        .await
        .expect("receipt posts");
        let replay = StockMovementOps::create_manual_receipt(
            &pool,
            tenant_id,
            actor,
            RequestContext::generate(None),
            &receipt_request,
        )
        .await
        .expect("receipt replays");
        assert_eq!(receipt.summary.id, replay.summary.id);

        let issue = |pool: sqlx::PgPool, key: String| async move {
            StockMovementOps::issue(
                &pool,
                tenant_id,
                actor,
                RequestContext::generate(None),
                &IssueStockRequest {
                    effective_on: today,
                    reference: None,
                    reason: Some("Concurrent issue".into()),
                    idempotency_key: key,
                    lines: vec![StockQuantityLineInput {
                        item_id,
                        store_id,
                        quantity_minor: 7,
                    }],
                },
            )
            .await
        };
        let first = rt::spawn(issue(pool.clone(), format!("issue-a-{tenant_id}")));
        let second = rt::spawn(issue(pool.clone(), format!("issue-b-{tenant_id}")));
        let outcomes = [
            first.await.expect("first issue task"),
            second.await.expect("second issue task"),
        ];
        assert_eq!(outcomes.iter().filter(|value| value.is_ok()).count(), 1);
        assert_eq!(outcomes.iter().filter(|value| value.is_err()).count(), 1);
        let (balances, _) =
            StockBalanceOps::list(&pool, tenant_id, 1, 25, None, Some(item_id), Some(store_id))
                .await
                .expect("balance loads");
        assert_eq!(balances[0].on_hand_minor, 3);

        let destination = StoreOps::create(
            &pool,
            tenant_id,
            actor,
            RequestContext::generate(None),
            &CreateStoreRequest {
                name: "Destination store".into(),
                location_label: None,
                notes: None,
                idempotency_key: format!("destination-{tenant_id}"),
            },
        )
        .await
        .expect("destination store fixture");
        let transfer = StockMovementOps::transfer(
            &pool,
            tenant_id,
            actor,
            RequestContext::generate(None),
            &TransferStockRequest {
                effective_on: today,
                reference: None,
                reason: None,
                idempotency_key: format!("transfer-{tenant_id}"),
                lines: vec![TransferStockLineInput {
                    item_id,
                    from_store_id: store_id,
                    to_store_id: destination.id,
                    quantity_minor: 2,
                }],
            },
        )
        .await
        .expect("transfer posts");
        assert_eq!(transfer.lines.len(), 2);
        assert_eq!(
            transfer
                .lines
                .iter()
                .map(|line| line.quantity_delta_minor)
                .sum::<i64>(),
            0
        );

        let adjustment = StockMovementOps::adjust(
            &pool,
            tenant_id,
            actor,
            RequestContext::generate(None),
            &AdjustStockRequest {
                effective_on: today,
                reference: None,
                reason: "Counted four".into(),
                idempotency_key: format!("adjust-{tenant_id}"),
                lines: vec![AdjustStockLineInput {
                    item_id,
                    store_id: destination.id,
                    expected_on_hand_minor: 2,
                    counted_on_hand_minor: 3,
                }],
            },
        )
        .await
        .expect("adjustment posts");
        assert!(
            StockMovementOps::adjust(
                &pool,
                tenant_id,
                actor,
                RequestContext::generate(None),
                &AdjustStockRequest {
                    effective_on: today,
                    reference: None,
                    reason: "Stale counted balance".into(),
                    idempotency_key: format!("stale-adjust-{tenant_id}"),
                    lines: vec![AdjustStockLineInput {
                        item_id,
                        store_id: destination.id,
                        expected_on_hand_minor: 2,
                        counted_on_hand_minor: 4,
                    }],
                },
            )
            .await
            .is_err()
        );
        let reversal_request = ReverseStockMovementRequest {
            effective_on: today,
            reason: "Correct count source".into(),
            idempotency_key: format!("reverse-{tenant_id}"),
        };
        let reversal = StockMovementOps::reverse(
            &pool,
            tenant_id,
            adjustment.summary.id,
            actor,
            RequestContext::generate(None),
            &reversal_request,
        )
        .await
        .expect("reversal operation")
        .expect("original movement");
        assert_eq!(reversal.lines[0].quantity_delta_minor, -1);
        let reversal_replay = StockMovementOps::reverse(
            &pool,
            tenant_id,
            adjustment.summary.id,
            actor,
            RequestContext::generate(None),
            &reversal_request,
        )
        .await
        .expect("reversal replay")
        .expect("original movement");
        assert_eq!(reversal.summary.id, reversal_replay.summary.id);

        let mut bulk_items = Vec::with_capacity(101);
        for index in 1..=101 {
            bulk_items.push(
                ItemOps::create(
                    &pool,
                    tenant_id,
                    actor,
                    RequestContext::generate(None),
                    &CreateItemRequest {
                        name: format!("Bulk transfer item {index}"),
                        description: None,
                        barcode: None,
                        unit_label: "each".into(),
                        quantity_scale: 0,
                        reorder_level_minor: None,
                        idempotency_key: format!("bulk-transfer-item-{index}-{tenant_id}"),
                    },
                )
                .await
                .expect("bulk transfer item fixture"),
            );
        }
        StockMovementOps::create_manual_receipt(
            &pool,
            tenant_id,
            actor,
            RequestContext::generate(None),
            &ManualReceiptRequest {
                effective_on: today,
                reference: Some("Large reversible transfer stock".into()),
                reason: None,
                idempotency_key: format!("bulk-transfer-receipt-{tenant_id}"),
                lines: bulk_items
                    .iter()
                    .map(|bulk_item| StockQuantityLineInput {
                        item_id: bulk_item.id,
                        store_id,
                        quantity_minor: 1,
                    })
                    .collect(),
            },
        )
        .await
        .expect("large transfer stock receipt");
        let large_transfer = StockMovementOps::transfer(
            &pool,
            tenant_id,
            actor,
            RequestContext::generate(None),
            &TransferStockRequest {
                effective_on: today,
                reference: Some("Large reversible transfer".into()),
                reason: None,
                idempotency_key: format!("large-transfer-{tenant_id}"),
                lines: bulk_items
                    .iter()
                    .map(|bulk_item| TransferStockLineInput {
                        item_id: bulk_item.id,
                        from_store_id: store_id,
                        to_store_id: destination.id,
                        quantity_minor: 1,
                    })
                    .collect(),
            },
        )
        .await
        .expect("101-line transfer posts");
        assert_eq!(large_transfer.lines.len(), 202);
        let large_reversal = StockMovementOps::reverse(
            &pool,
            tenant_id,
            large_transfer.summary.id,
            actor,
            RequestContext::generate(None),
            &ReverseStockMovementRequest {
                effective_on: today,
                reason: "Reverse large transfer".into(),
                idempotency_key: format!("reverse-large-transfer-{tenant_id}"),
            },
        )
        .await
        .expect("large transfer reversal operation")
        .expect("large transfer movement");
        assert_eq!(large_reversal.lines.len(), 202);
        let original_large_lines = large_transfer
            .lines
            .iter()
            .map(|line| (line.item_id, line.store_id, line.quantity_delta_minor))
            .collect::<std::collections::BTreeSet<_>>();
        let reversed_large_lines = large_reversal
            .lines
            .iter()
            .map(|line| (line.item_id, line.store_id, -line.quantity_delta_minor))
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(original_large_lines, reversed_large_lines);

        let chain_item = ItemOps::create(
            &pool,
            tenant_id,
            actor,
            RequestContext::generate(None),
            &CreateItemRequest {
                name: "Chained transfer item".into(),
                description: None,
                barcode: None,
                unit_label: "each".into(),
                quantity_scale: 0,
                reorder_level_minor: None,
                idempotency_key: format!("chain-item-{tenant_id}"),
            },
        )
        .await
        .expect("chained transfer item fixture");
        let chain_destination = StoreOps::create(
            &pool,
            tenant_id,
            actor,
            RequestContext::generate(None),
            &CreateStoreRequest {
                name: "Chained destination store".into(),
                location_label: None,
                notes: None,
                idempotency_key: format!("chain-destination-{tenant_id}"),
            },
        )
        .await
        .expect("chained destination fixture");
        StockMovementOps::create_manual_receipt(
            &pool,
            tenant_id,
            actor,
            RequestContext::generate(None),
            &ManualReceiptRequest {
                effective_on: today,
                reference: Some("Chained transfer stock".into()),
                reason: None,
                idempotency_key: format!("chain-receipt-{tenant_id}"),
                lines: vec![StockQuantityLineInput {
                    item_id: chain_item.id,
                    store_id,
                    quantity_minor: 10,
                }],
            },
        )
        .await
        .expect("chained transfer stock receipt");
        let chained_transfer = StockMovementOps::transfer(
            &pool,
            tenant_id,
            actor,
            RequestContext::generate(None),
            &TransferStockRequest {
                effective_on: today,
                reference: Some("Chained transfer".into()),
                reason: None,
                idempotency_key: format!("chain-transfer-{tenant_id}"),
                lines: vec![
                    TransferStockLineInput {
                        item_id: chain_item.id,
                        from_store_id: store_id,
                        to_store_id: destination.id,
                        quantity_minor: 10,
                    },
                    TransferStockLineInput {
                        item_id: chain_item.id,
                        from_store_id: destination.id,
                        to_store_id: chain_destination.id,
                        quantity_minor: 10,
                    },
                ],
            },
        )
        .await
        .expect("chained transfer posts");
        StockMovementOps::reverse(
            &pool,
            tenant_id,
            chained_transfer.summary.id,
            actor,
            RequestContext::generate(None),
            &ReverseStockMovementRequest {
                effective_on: today,
                reason: "Reverse chained transfer".into(),
                idempotency_key: format!("reverse-chain-transfer-{tenant_id}"),
            },
        )
        .await
        .expect("chained transfer reversal operation")
        .expect("chained transfer movement");
        let (chain_balances, _) =
            StockBalanceOps::list(&pool, tenant_id, 1, 25, None, Some(chain_item.id), None)
                .await
                .expect("chained transfer balances");
        let chain_balances = chain_balances
            .into_iter()
            .map(|balance| (balance.store_id, balance.on_hand_minor))
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(chain_balances.get(&store_id), Some(&10));
        assert_eq!(chain_balances.get(&destination.id), Some(&0));
        assert_eq!(chain_balances.get(&chain_destination.id), Some(&0));

        let direct = sqlx::query(
            "UPDATE assets_inventory_stock_balances SET on_hand_minor = on_hand_minor + 1, version = version + 1 WHERE tenant_id = $1 AND item_id = $2 AND store_id = $3",
        )
        .bind(tenant_id)
        .bind(item_id)
        .bind(store_id)
        .execute(&pool)
        .await;
        assert!(direct.is_err());
        let mut spoof = pool.begin().await.expect("spoof transaction");
        sqlx::query("SELECT SET_CONFIG('campus_pilot.stock_posting_movement_id', $1, TRUE)")
            .bind(receipt.summary.id.to_string())
            .execute(&mut *spoof)
            .await
            .expect("set local custom value");
        let spoofed = sqlx::query(
            "UPDATE assets_inventory_stock_balances SET on_hand_minor = on_hand_minor + 1, version = version + 1 WHERE tenant_id = $1 AND item_id = $2 AND store_id = $3",
        )
        .bind(tenant_id)
        .bind(item_id)
        .bind(store_id)
        .execute(&mut *spoof)
        .await;
        assert!(spoofed.is_err());
        spoof.rollback().await.ok();

        let mut sequence = pool.begin().await.expect("sequence transaction");
        sqlx::query("UPDATE assets_inventory_movement_sequences SET last_number = last_number + 1 WHERE tenant_id = $1")
            .bind(tenant_id)
            .execute(&mut *sequence)
            .await
            .expect("sequence update reaches deferred guard");
        assert!(sequence.commit().await.is_err());

        let current_sequence: i64 = sqlx::query_scalar(
            "SELECT last_number FROM assets_inventory_movement_sequences WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .fetch_one(&pool)
        .await
        .expect("current movement sequence");
        let mut skipped_sequence = pool.begin().await.expect("skipped sequence transaction");
        sqlx::query(
            "UPDATE assets_inventory_movement_sequences SET last_number = last_number + 1 WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .execute(&mut *skipped_sequence)
        .await
        .expect("first sequence advance");
        sqlx::query(
            "UPDATE assets_inventory_movement_sequences SET last_number = last_number + 1 WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .execute(&mut *skipped_sequence)
        .await
        .expect("second sequence advance");
        sqlx::query(
            r#"
            INSERT INTO assets_inventory_stock_movements (
                tenant_id, movement_number, kind, effective_on,
                idempotency_key, create_request_fingerprint, created_by
            ) VALUES ($1, $2, 'manual_receipt', $3, $4, $5, $6)
            "#,
        )
        .bind(tenant_id)
        .bind(format!("MOV-{:06}", current_sequence + 2))
        .bind(today)
        .bind(format!("skipped-sequence-{tenant_id}"))
        .bind("0".repeat(64))
        .bind(actor_id)
        .execute(&mut *skipped_sequence)
        .await
        .expect("only the final movement number exists");
        let skipped_reference_guard = sqlx::query(
            "SET CONSTRAINTS assets_inventory_movement_sequence_reference_guard IMMEDIATE",
        )
        .execute(&mut *skipped_sequence)
        .await;
        assert!(
            skipped_reference_guard.is_err(),
            "the named sequence reference constraint must reject every skipped movement number"
        );
        skipped_sequence.rollback().await.ok();

        let inactivate = ItemOps::update(
            &pool,
            tenant_id,
            item_id,
            actor,
            RequestContext::generate(None),
            &UpdateItemRequest {
                name: item.name,
                description: item.description,
                barcode: item.barcode,
                reorder_level_minor: item.reorder_level_minor,
                status: AssetStatus::Inactive,
                expected_version: item.version,
            },
        )
        .await;
        assert!(inactivate.is_err());
    }

    #[actix_web::test]
    #[ignore = "requires STOCK_LEDGER_TEST_DATABASE_URL with migrations through 082"]
    async fn postgres_goods_receipt_allocation_is_exact_and_reversible() {
        let database_url = std::env::var("STOCK_LEDGER_TEST_DATABASE_URL")
            .expect("STOCK_LEDGER_TEST_DATABASE_URL must target a disposable database");
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .connect(&database_url)
            .await
            .expect("disposable PostgreSQL database must be available");
        let tenant_id = Uuid::new_v4();
        let preparer_id = Uuid::new_v4();
        let reviewer_id = Uuid::new_v4();
        let employee_id = Uuid::new_v4();
        let currency_id = Uuid::new_v4();
        sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, $2, 'Allocation test')")
            .bind(tenant_id)
            .bind(format!("allocation-{tenant_id}"))
            .execute(&pool)
            .await
            .expect("tenant fixture");
        for (id, label) in [(preparer_id, "prepare"), (reviewer_id, "review")] {
            sqlx::query(
                "INSERT INTO users (id, tenant_id, email, password_hash, full_name) VALUES ($1, $2, $3, 'x', $4)",
            )
            .bind(id)
            .bind(tenant_id)
            .bind(format!("{label}-{id}@example.test"))
            .bind(label)
            .execute(&pool)
            .await
            .expect("user fixture");
        }
        sqlx::query(
            "INSERT INTO employees (id, tenant_id, account_id, employee_number, display_name) VALUES ($1, $2, $3, 'EMP-TEST', 'Procurement Requester')",
        )
        .bind(employee_id)
        .bind(tenant_id)
        .bind(preparer_id)
        .execute(&pool)
        .await
        .expect("employee fixture");
        sqlx::query(
            "INSERT INTO finance_currencies (id, tenant_id, code, name, minor_units, is_reporting) VALUES ($1, $2, 'USD', 'US Dollar', 2, TRUE)",
        )
        .bind(currency_id)
        .bind(tenant_id)
        .execute(&pool)
        .await
        .expect("currency fixture");
        let preparer = AuditActor::person(preparer_id);
        let reviewer = AuditActor::person(reviewer_id);
        let supplier = SupplierOps::create(
            &pool,
            tenant_id,
            preparer,
            RequestContext::generate(None),
            &CreateSupplierRequest {
                legal_name: "Test Stationery".into(),
                trading_name: None,
                registration_number: None,
                tax_number: None,
                email: None,
                phone: None,
                address: None,
                idempotency_key: format!("supplier-{tenant_id}"),
            },
        )
        .await
        .expect("supplier fixture");
        let requisition = RequisitionOps::create(
            &pool,
            tenant_id,
            preparer,
            RequestContext::generate(None),
            &CreateRequisitionRequest {
                requester_employee_id: employee_id,
                currency_id,
                title: "Exercise books".into(),
                purpose: None,
                needed_by: None,
                idempotency_key: format!("req-{tenant_id}"),
                lines: vec![RequisitionLineInput {
                    description: "A5 exercise book".into(),
                    quantity: 10,
                    unit_label: Some("each".into()),
                    estimated_unit_amount_minor: 100,
                    preferred_supplier_id: Some(supplier.id),
                }],
            },
        )
        .await
        .expect("requisition fixture");
        let submitted = RequisitionOps::submit(
            &pool,
            tenant_id,
            requisition.summary.id,
            preparer,
            RequestContext::generate(None),
            requisition.summary.version,
        )
        .await
        .expect("requisition submits")
        .expect("requisition exists");
        let approved = RequisitionOps::approve(
            &pool,
            tenant_id,
            submitted.summary.id,
            reviewer,
            RequestContext::generate(None),
            &DecisionRequest {
                expected_version: submitted.summary.version,
                note: None,
            },
        )
        .await
        .expect("requisition approves")
        .expect("requisition exists");
        let order = PurchaseOrderOps::create(
            &pool,
            tenant_id,
            preparer,
            RequestContext::generate(None),
            &CreatePurchaseOrderRequest {
                requisition_id: approved.summary.id,
                supplier_id: supplier.id,
                delivery_date: None,
                notes: None,
                idempotency_key: format!("po-{tenant_id}"),
                lines: vec![PurchaseOrderLineInput {
                    requisition_line_id: approved.lines[0].id,
                    quantity_minor: 10,
                    quantity_scale: 0,
                    unit_amount_minor: 100,
                }],
            },
        )
        .await
        .expect("purchase order fixture");
        let issued = PurchaseOrderOps::issue(
            &pool,
            tenant_id,
            order.summary.id,
            reviewer,
            RequestContext::generate(None),
            order.summary.version,
        )
        .await
        .expect("purchase order issues")
        .expect("purchase order exists");
        let receipt = GoodsReceiptOps::create(
            &pool,
            tenant_id,
            preparer,
            RequestContext::generate(None),
            &CreateGoodsReceiptRequest {
                purchase_order_id: issued.summary.id,
                received_on: chrono::Utc::now().date_naive(),
                delivery_reference: None,
                notes: None,
                idempotency_key: format!("grn-{tenant_id}"),
                lines: vec![GoodsReceiptLineInput {
                    purchase_order_line_id: issued.lines[0].id,
                    quantity_minor: 10,
                    quantity_scale: 0,
                }],
            },
        )
        .await
        .expect("goods receipt fixture");
        let posted = GoodsReceiptOps::post(
            &pool,
            tenant_id,
            receipt.summary.id,
            reviewer,
            RequestContext::generate(None),
            receipt.summary.version,
        )
        .await
        .expect("goods receipt posts")
        .expect("goods receipt exists");
        let (human_supplier_search, _) =
            GoodsReceiptAllocationOps::list(&pool, tenant_id, 1, 5, Some("Test Stationery"), None)
                .await
                .expect("human allocation source search");
        assert_eq!(human_supplier_search.len(), 1);
        let (agent_supplier_search, _) = GoodsReceiptAllocationOps::list_for_agent(
            &pool,
            tenant_id,
            1,
            2,
            Some("Test Stationery"),
            None,
        )
        .await
        .expect("Agent allocation source search");
        assert!(agent_supplier_search.is_empty());
        let (agent_receipt_search, _) = GoodsReceiptAllocationOps::list_for_agent(
            &pool,
            tenant_id,
            1,
            2,
            Some(&posted.summary.goods_receipt_number),
            None,
        )
        .await
        .expect("Agent projected-field search");
        assert_eq!(agent_receipt_search.len(), 1);
        let item = ItemOps::create(
            &pool,
            tenant_id,
            preparer,
            RequestContext::generate(None),
            &CreateItemRequest {
                name: "A5 exercise book".into(),
                description: None,
                barcode: None,
                unit_label: "each".into(),
                quantity_scale: 0,
                reorder_level_minor: None,
                idempotency_key: format!("item-{tenant_id}"),
            },
        )
        .await
        .expect("item fixture");
        let incompatible = ItemOps::create(
            &pool,
            tenant_id,
            preparer,
            RequestContext::generate(None),
            &CreateItemRequest {
                name: "Box item".into(),
                description: None,
                barcode: None,
                unit_label: "box".into(),
                quantity_scale: 0,
                reorder_level_minor: None,
                idempotency_key: format!("box-{tenant_id}"),
            },
        )
        .await
        .expect("incompatible item fixture");
        let compatible_other = ItemOps::create(
            &pool,
            tenant_id,
            preparer,
            RequestContext::generate(None),
            &CreateItemRequest {
                name: "Another each item".into(),
                description: None,
                barcode: None,
                unit_label: "EACH".into(),
                quantity_scale: 0,
                reorder_level_minor: None,
                idempotency_key: format!("other-{tenant_id}"),
            },
        )
        .await
        .expect("compatible item fixture");
        let mut stores = Vec::new();
        for label in ["Main", "Classroom"] {
            stores.push(
                StoreOps::create(
                    &pool,
                    tenant_id,
                    preparer,
                    RequestContext::generate(None),
                    &CreateStoreRequest {
                        name: format!("{label} store"),
                        location_label: None,
                        notes: None,
                        idempotency_key: format!("store-{label}-{tenant_id}"),
                    },
                )
                .await
                .expect("store fixture"),
            );
        }
        let allocation_request =
            |key: &str, item_id: Uuid, quantity: i64| AllocateGoodsReceiptRequest {
                goods_receipt_id: posted.summary.id,
                effective_on: chrono::Utc::now().date_naive(),
                reason: None,
                idempotency_key: format!("{key}-{tenant_id}"),
                lines: vec![AllocateGoodsReceiptLineInput {
                    goods_receipt_line_id: posted.lines[0].id,
                    item_id,
                    store_id: stores[0].id,
                    quantity_minor: quantity,
                }],
            };
        assert!(
            GoodsReceiptAllocationOps::allocate(
                &pool,
                tenant_id,
                preparer,
                RequestContext::generate(None),
                &allocation_request("over", item.id, 11),
            )
            .await
            .is_err()
        );
        assert!(
            GoodsReceiptAllocationOps::allocate(
                &pool,
                tenant_id,
                preparer,
                RequestContext::generate(None),
                &allocation_request("unit", incompatible.id, 1),
            )
            .await
            .is_err()
        );
        let allocation = GoodsReceiptAllocationOps::allocate(
            &pool,
            tenant_id,
            preparer,
            RequestContext::generate(None),
            &AllocateGoodsReceiptRequest {
                goods_receipt_id: posted.summary.id,
                effective_on: chrono::Utc::now().date_naive(),
                reason: None,
                idempotency_key: format!("allocate-{tenant_id}"),
                lines: vec![
                    AllocateGoodsReceiptLineInput {
                        goods_receipt_line_id: posted.lines[0].id,
                        item_id: item.id,
                        store_id: stores[0].id,
                        quantity_minor: 2,
                    },
                    AllocateGoodsReceiptLineInput {
                        goods_receipt_line_id: posted.lines[0].id,
                        item_id: item.id,
                        store_id: stores[1].id,
                        quantity_minor: 1,
                    },
                ],
            },
        )
        .await
        .expect("split allocation posts");
        assert!(
            GoodsReceiptAllocationOps::allocate(
                &pool,
                tenant_id,
                preparer,
                RequestContext::generate(None),
                &allocation_request("remap", compatible_other.id, 1),
            )
            .await
            .is_err()
        );
        StockMovementOps::reverse(
            &pool,
            tenant_id,
            allocation.summary.id,
            preparer,
            RequestContext::generate(None),
            &ReverseStockMovementRequest {
                effective_on: chrono::Utc::now().date_naive(),
                reason: "Return allocation capacity".into(),
                idempotency_key: format!("reverse-allocation-{tenant_id}"),
            },
        )
        .await
        .expect("allocation reversal")
        .expect("allocation movement exists");
        let (states, _) =
            GoodsReceiptAllocationOps::list(&pool, tenant_id, 1, 25, None, Some(posted.summary.id))
                .await
                .expect("allocation state loads");
        assert_eq!(states[0].lines[0].allocated_quantity_minor, 0);
        assert_eq!(states[0].lines[0].remaining_quantity_minor, 10);
    }
}
