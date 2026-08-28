//! Transactional item and store catalogue operations.
//!
//! Writes allocate tenant-local references lazily, enforce optimistic versions,
//! and append actor-aware audit evidence in the same database transaction.

use std::collections::BTreeMap;

use anyhow::{Context, Result, anyhow, bail};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::dtos::{
    CreateItemRequest, CreateStoreRequest, ItemResponse, StoreResponse, UpdateItemRequest,
    UpdateStoreRequest,
};
use crate::models::{ItemRecord, StoreRecord};

const DEFAULT_PAGE: i64 = 1;
const DEFAULT_PER_PAGE: i64 = 25;
const MAX_PAGE: i64 = 1_000_000;
const MAX_PER_PAGE: i64 = 100;
const MAX_SEARCH_LENGTH: usize = 200;
const MAX_REFERENCE_NUMBER: i64 = 999_999;
const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

pub struct ItemOps;

impl ItemOps {
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
    ) -> Result<(Vec<ItemResponse>, i64)> {
        let (page, per_page) = bounded_page(Some(page), Some(per_page));
        let status = parse_status_filter(status, "Item")?;
        let search = search_pattern(search)?;
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, ItemRecord>(
            r#"
            SELECT id, item_number, name, description, barcode, unit_label,
                   quantity_scale, reorder_level_minor, status, version,
                   create_request_fingerprint, created_by, updated_by, deleted_at,
                   created_at, updated_at
              FROM assets_inventory_items
             WHERE tenant_id = $1 AND deleted_at IS NULL
               AND ($2::TEXT IS NULL OR item_number ILIKE $2 OR name ILIKE $2
                    OR description ILIKE $2 OR barcode ILIKE $2 OR unit_label ILIKE $2)
               AND ($3::TEXT IS NULL OR status = $3)
             ORDER BY name, item_number
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
        .context("Failed to list Assets and inventory items")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
              FROM assets_inventory_items
             WHERE tenant_id = $1 AND deleted_at IS NULL
               AND ($2::TEXT IS NULL OR item_number ILIKE $2 OR name ILIKE $2
                    OR description ILIKE $2 OR barcode ILIKE $2 OR unit_label ILIKE $2)
               AND ($3::TEXT IS NULL OR status = $3)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .fetch_one(pool)
        .await
        .context("Failed to count Assets and inventory items")?;
        Ok((rows.into_iter().map(ItemResponse::from).collect(), total))
    }

    pub async fn get(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<Option<ItemResponse>> {
        item_by_id(pool, tenant_id, id)
            .await
            .map(|item| item.map(ItemResponse::from))
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateItemRequest,
    ) -> Result<ItemResponse> {
        let actor_id = actor_id(actor)?;
        let values = CreateItemValues::parse(request)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start item transaction")?;
        let item = match create_item_in_transaction(&mut transaction, tenant_id, actor_id, &values)
            .await?
        {
            CreateOutcome::Created(item) => item,
            CreateOutcome::Replayed(item) => {
                transaction
                    .rollback()
                    .await
                    .context("Failed to close replayed item transaction")?;
                return Ok(ItemResponse::from(item));
            }
        };
        append_catalog_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            CatalogAudit::new(
                "assets_inventory.items.create",
                "assets_inventory_item",
                item.id,
                reference_metadata("item_number", &item.item_number, &item.status),
            ),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit item transaction")?;
        Ok(ItemResponse::from(item))
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateItemRequest,
    ) -> Result<Option<ItemResponse>> {
        let actor_id = actor_id(actor)?;
        let values = UpdateItemValues::parse(request)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start item transaction")?;
        let Some(current) = lock_item(&mut transaction, tenant_id, id).await? else {
            return Ok(None);
        };
        ensure_version(current.version, request.expected_version, "Item")?;
        let item = sqlx::query_as::<_, ItemRecord>(
            r#"
            UPDATE assets_inventory_items
               SET name = $3, description = $4, barcode = $5,
                   reorder_level_minor = $6, status = $7, updated_by = $8,
                   version = version + 1
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            RETURNING id, item_number, name, description, barcode, unit_label,
                      quantity_scale, reorder_level_minor, status, version,
                      create_request_fingerprint, created_by, updated_by, deleted_at,
                      created_at, updated_at
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(&values.name)
        .bind(&values.description)
        .bind(&values.barcode)
        .bind(values.reorder_level_minor)
        .bind(request.status.as_str())
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "Failed to update Assets and inventory item"))?;
        append_catalog_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            CatalogAudit::new(
                "assets_inventory.items.update",
                "assets_inventory_item",
                item.id,
                reference_metadata("item_number", &item.item_number, &item.status),
            ),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit item transaction")?;
        Ok(Some(ItemResponse::from(item)))
    }

    pub async fn delete(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<bool> {
        let actor_id = actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start item transaction")?;
        let Some(current) = lock_item(&mut transaction, tenant_id, id).await? else {
            return Ok(false);
        };
        ensure_version(current.version, expected_version, "Item")?;
        if current.status != "inactive" {
            bail!("Only an inactive asset inventory item can be removed");
        }
        let result = sqlx::query(
            r#"
            UPDATE assets_inventory_items
               SET deleted_at = NOW(), deleted_by = $3, updated_by = $3,
                   version = version + 1
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to remove Assets and inventory item")?;
        ensure_one_row(result.rows_affected(), "Item")?;
        append_catalog_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            CatalogAudit::new(
                "assets_inventory.items.delete",
                "assets_inventory_item",
                current.id,
                reference_metadata("item_number", &current.item_number, &current.status),
            ),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit item transaction")?;
        Ok(true)
    }
}

pub struct StoreOps;

impl StoreOps {
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
    ) -> Result<(Vec<StoreResponse>, i64)> {
        let (page, per_page) = bounded_page(Some(page), Some(per_page));
        let status = parse_status_filter(status, "Store")?;
        let search = search_pattern(search)?;
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, StoreRecord>(
            r#"
            SELECT id, store_number, name, location_label, notes, status, version,
                   create_request_fingerprint, created_by, updated_by, deleted_at,
                   created_at, updated_at
              FROM assets_inventory_stores
             WHERE tenant_id = $1 AND deleted_at IS NULL
               AND ($2::TEXT IS NULL OR store_number ILIKE $2 OR name ILIKE $2
                    OR location_label ILIKE $2 OR notes ILIKE $2)
               AND ($3::TEXT IS NULL OR status = $3)
             ORDER BY name, store_number
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
        .context("Failed to list Assets and inventory stores")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
              FROM assets_inventory_stores
             WHERE tenant_id = $1 AND deleted_at IS NULL
               AND ($2::TEXT IS NULL OR store_number ILIKE $2 OR name ILIKE $2
                    OR location_label ILIKE $2 OR notes ILIKE $2)
               AND ($3::TEXT IS NULL OR status = $3)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .fetch_one(pool)
        .await
        .context("Failed to count Assets and inventory stores")?;
        Ok((rows.into_iter().map(StoreResponse::from).collect(), total))
    }

    pub async fn get(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<Option<StoreResponse>> {
        store_by_id(pool, tenant_id, id)
            .await
            .map(|store| store.map(StoreResponse::from))
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateStoreRequest,
    ) -> Result<StoreResponse> {
        let actor_id = actor_id(actor)?;
        let values = CreateStoreValues::parse(request)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start store transaction")?;
        let store =
            match create_store_in_transaction(&mut transaction, tenant_id, actor_id, &values)
                .await?
            {
                CreateOutcome::Created(store) => store,
                CreateOutcome::Replayed(store) => {
                    transaction
                        .rollback()
                        .await
                        .context("Failed to close replayed store transaction")?;
                    return Ok(StoreResponse::from(store));
                }
            };
        append_catalog_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            CatalogAudit::new(
                "assets_inventory.stores.create",
                "assets_inventory_store",
                store.id,
                reference_metadata("store_number", &store.store_number, &store.status),
            ),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit store transaction")?;
        Ok(StoreResponse::from(store))
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateStoreRequest,
    ) -> Result<Option<StoreResponse>> {
        let actor_id = actor_id(actor)?;
        let values = UpdateStoreValues::parse(request)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start store transaction")?;
        let Some(current) = lock_store(&mut transaction, tenant_id, id).await? else {
            return Ok(None);
        };
        ensure_version(current.version, request.expected_version, "Store")?;
        let store = sqlx::query_as::<_, StoreRecord>(
            r#"
            UPDATE assets_inventory_stores
               SET name = $3, location_label = $4, notes = $5, status = $6,
                   updated_by = $7, version = version + 1
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            RETURNING id, store_number, name, location_label, notes, status, version,
                      create_request_fingerprint, created_by, updated_by, deleted_at,
                      created_at, updated_at
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(&values.name)
        .bind(&values.location_label)
        .bind(&values.notes)
        .bind(request.status.as_str())
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "Failed to update Assets and inventory store"))?;
        append_catalog_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            CatalogAudit::new(
                "assets_inventory.stores.update",
                "assets_inventory_store",
                store.id,
                reference_metadata("store_number", &store.store_number, &store.status),
            ),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit store transaction")?;
        Ok(Some(StoreResponse::from(store)))
    }

    pub async fn delete(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<bool> {
        let actor_id = actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start store transaction")?;
        let Some(current) = lock_store(&mut transaction, tenant_id, id).await? else {
            return Ok(false);
        };
        ensure_version(current.version, expected_version, "Store")?;
        if current.status != "inactive" {
            bail!("Only an inactive asset inventory store can be removed");
        }
        let result = sqlx::query(
            r#"
            UPDATE assets_inventory_stores
               SET deleted_at = NOW(), deleted_by = $3, updated_by = $3,
                   version = version + 1
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to remove Assets and inventory store")?;
        ensure_one_row(result.rows_affected(), "Store")?;
        append_catalog_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            CatalogAudit::new(
                "assets_inventory.stores.delete",
                "assets_inventory_store",
                current.id,
                reference_metadata("store_number", &current.store_number, &current.status),
            ),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit store transaction")?;
        Ok(true)
    }
}

struct CreateItemValues {
    name: String,
    description: Option<String>,
    barcode: Option<String>,
    unit_label: String,
    quantity_scale: i16,
    reorder_level_minor: Option<i64>,
    idempotency_key: String,
}

impl CreateItemValues {
    fn parse(request: &CreateItemRequest) -> Result<Self> {
        if !(0..=6).contains(&request.quantity_scale) {
            bail!("Item quantity scale must be between zero and six");
        }
        validate_reorder_level(request.reorder_level_minor)?;
        Ok(Self {
            name: required(&request.name, "Item name")?,
            description: optional(request.description.as_deref()),
            barcode: optional(request.barcode.as_deref()),
            unit_label: required(&request.unit_label, "Item unit label")?,
            quantity_scale: request.quantity_scale,
            reorder_level_minor: request.reorder_level_minor,
            idempotency_key: required(&request.idempotency_key, "Idempotency key")?,
        })
    }

    fn fingerprint(&self) -> String {
        let mut fingerprint = CreateFingerprint::new("assets_inventory:item:create:v1");
        fingerprint.text(&self.name);
        fingerprint.optional_text(self.description.as_deref());
        fingerprint.optional_text(self.barcode.as_deref());
        fingerprint.text(&self.unit_label);
        fingerprint.i16(self.quantity_scale);
        fingerprint.optional_i64(self.reorder_level_minor);
        fingerprint.finish()
    }
}

struct UpdateItemValues {
    name: String,
    description: Option<String>,
    barcode: Option<String>,
    reorder_level_minor: Option<i64>,
}

impl UpdateItemValues {
    fn parse(request: &UpdateItemRequest) -> Result<Self> {
        validate_reorder_level(request.reorder_level_minor)?;
        Ok(Self {
            name: required(&request.name, "Item name")?,
            description: optional(request.description.as_deref()),
            barcode: optional(request.barcode.as_deref()),
            reorder_level_minor: request.reorder_level_minor,
        })
    }
}

struct CreateStoreValues {
    name: String,
    location_label: Option<String>,
    notes: Option<String>,
    idempotency_key: String,
}

impl CreateStoreValues {
    fn parse(request: &CreateStoreRequest) -> Result<Self> {
        Ok(Self {
            name: required(&request.name, "Store name")?,
            location_label: optional(request.location_label.as_deref()),
            notes: optional(request.notes.as_deref()),
            idempotency_key: required(&request.idempotency_key, "Idempotency key")?,
        })
    }

    fn fingerprint(&self) -> String {
        let mut fingerprint = CreateFingerprint::new("assets_inventory:store:create:v1");
        fingerprint.text(&self.name);
        fingerprint.optional_text(self.location_label.as_deref());
        fingerprint.optional_text(self.notes.as_deref());
        fingerprint.finish()
    }
}

struct UpdateStoreValues {
    name: String,
    location_label: Option<String>,
    notes: Option<String>,
}

impl UpdateStoreValues {
    fn parse(request: &UpdateStoreRequest) -> Result<Self> {
        Ok(Self {
            name: required(&request.name, "Store name")?,
            location_label: optional(request.location_label.as_deref()),
            notes: optional(request.notes.as_deref()),
        })
    }
}

#[derive(Debug)]
enum CreateOutcome<T> {
    Created(T),
    Replayed(T),
}

async fn create_item_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor_id: Uuid,
    values: &CreateItemValues,
) -> Result<CreateOutcome<ItemRecord>> {
    lock_catalog(transaction, tenant_id, "item").await?;
    if let Some(existing) =
        item_by_idempotency(transaction, tenant_id, &values.idempotency_key).await?
    {
        if existing.deleted_at.is_some() {
            bail!("Idempotency key belongs to a deleted asset inventory item");
        }
        ensure_replay_fingerprint(
            &existing.create_request_fingerprint,
            &values.fingerprint(),
            "item",
        )?;
        return Ok(CreateOutcome::Replayed(existing));
    }
    let item_number = next_item_number(transaction, tenant_id).await?;
    let item = sqlx::query_as::<_, ItemRecord>(
        r#"
        INSERT INTO assets_inventory_items (
            tenant_id, item_number, name, description, barcode, unit_label,
            quantity_scale, reorder_level_minor, idempotency_key,
            create_request_fingerprint, created_by, updated_by
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $11)
        RETURNING id, item_number, name, description, barcode, unit_label,
                  quantity_scale, reorder_level_minor, status, version,
                  create_request_fingerprint, created_by, updated_by, deleted_at,
                  created_at, updated_at
        "#,
    )
    .bind(tenant_id)
    .bind(&item_number)
    .bind(&values.name)
    .bind(&values.description)
    .bind(&values.barcode)
    .bind(&values.unit_label)
    .bind(values.quantity_scale)
    .bind(values.reorder_level_minor)
    .bind(&values.idempotency_key)
    .bind(values.fingerprint())
    .bind(actor_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|error| database_error(error, "Failed to create Assets and inventory item"))?;
    Ok(CreateOutcome::Created(item))
}

async fn create_store_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor_id: Uuid,
    values: &CreateStoreValues,
) -> Result<CreateOutcome<StoreRecord>> {
    lock_catalog(transaction, tenant_id, "store").await?;
    if let Some(existing) =
        store_by_idempotency(transaction, tenant_id, &values.idempotency_key).await?
    {
        if existing.deleted_at.is_some() {
            bail!("Idempotency key belongs to a deleted asset inventory store");
        }
        ensure_replay_fingerprint(
            &existing.create_request_fingerprint,
            &values.fingerprint(),
            "store",
        )?;
        return Ok(CreateOutcome::Replayed(existing));
    }
    let store_number = next_store_number(transaction, tenant_id).await?;
    let store = sqlx::query_as::<_, StoreRecord>(
        r#"
        INSERT INTO assets_inventory_stores (
            tenant_id, store_number, name, location_label, notes,
            idempotency_key, create_request_fingerprint, created_by, updated_by
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)
        RETURNING id, store_number, name, location_label, notes, status, version,
                  create_request_fingerprint, created_by, updated_by, deleted_at,
                  created_at, updated_at
        "#,
    )
    .bind(tenant_id)
    .bind(&store_number)
    .bind(&values.name)
    .bind(&values.location_label)
    .bind(&values.notes)
    .bind(&values.idempotency_key)
    .bind(values.fingerprint())
    .bind(actor_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|error| database_error(error, "Failed to create Assets and inventory store"))?;
    Ok(CreateOutcome::Created(store))
}

async fn item_by_id(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<Option<ItemRecord>> {
    sqlx::query_as::<_, ItemRecord>(
        r#"
        SELECT id, item_number, name, description, barcode, unit_label,
               quantity_scale, reorder_level_minor, status, version,
               create_request_fingerprint, created_by, updated_by, deleted_at,
               created_at, updated_at
          FROM assets_inventory_items
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("Failed to read Assets and inventory item")
}

async fn store_by_id(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<Option<StoreRecord>> {
    sqlx::query_as::<_, StoreRecord>(
        r#"
        SELECT id, store_number, name, location_label, notes, status, version,
               create_request_fingerprint, created_by, updated_by, deleted_at,
               created_at, updated_at
          FROM assets_inventory_stores
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("Failed to read Assets and inventory store")
}

async fn lock_item(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<ItemRecord>> {
    sqlx::query_as::<_, ItemRecord>(
        r#"
        SELECT id, item_number, name, description, barcode, unit_label,
               quantity_scale, reorder_level_minor, status, version,
               create_request_fingerprint, created_by, updated_by, deleted_at,
               created_at, updated_at
          FROM assets_inventory_items
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock Assets and inventory item")
}

async fn lock_store(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<StoreRecord>> {
    sqlx::query_as::<_, StoreRecord>(
        r#"
        SELECT id, store_number, name, location_label, notes, status, version,
               create_request_fingerprint, created_by, updated_by, deleted_at,
               created_at, updated_at
          FROM assets_inventory_stores
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock Assets and inventory store")
}

async fn item_by_idempotency(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    key: &str,
) -> Result<Option<ItemRecord>> {
    sqlx::query_as::<_, ItemRecord>(
        r#"
        SELECT id, item_number, name, description, barcode, unit_label,
               quantity_scale, reorder_level_minor, status, version,
               create_request_fingerprint, created_by, updated_by, deleted_at,
               created_at, updated_at
          FROM assets_inventory_items
         WHERE tenant_id = $1 AND idempotency_key = $2
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(key)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to resolve item idempotency")
}

async fn store_by_idempotency(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    key: &str,
) -> Result<Option<StoreRecord>> {
    sqlx::query_as::<_, StoreRecord>(
        r#"
        SELECT id, store_number, name, location_label, notes, status, version,
               create_request_fingerprint, created_by, updated_by, deleted_at,
               created_at, updated_at
          FROM assets_inventory_stores
         WHERE tenant_id = $1 AND idempotency_key = $2
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(key)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to resolve store idempotency")
}

async fn next_item_number(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
) -> Result<String> {
    let number = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO assets_inventory_item_sequences (tenant_id, last_number)
        VALUES ($1, 1)
        ON CONFLICT (tenant_id)
        DO UPDATE SET last_number = assets_inventory_item_sequences.last_number + 1,
                      deleted_at = NULL
        WHERE assets_inventory_item_sequences.last_number < 999999
        RETURNING last_number
        "#,
    )
    .bind(tenant_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to allocate item number")?
    .ok_or_else(|| anyhow!("Item number sequence is exhausted"))?;
    format_reference("ITM", number)
}

async fn next_store_number(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
) -> Result<String> {
    let number = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO assets_inventory_store_sequences (tenant_id, last_number)
        VALUES ($1, 1)
        ON CONFLICT (tenant_id)
        DO UPDATE SET last_number = assets_inventory_store_sequences.last_number + 1,
                      deleted_at = NULL
        WHERE assets_inventory_store_sequences.last_number < 999999
        RETURNING last_number
        "#,
    )
    .bind(tenant_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to allocate store number")?
    .ok_or_else(|| anyhow!("Store number sequence is exhausted"))?;
    format_reference("STR", number)
}

async fn lock_catalog(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    catalogue: &str,
) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("assets-inventory:{catalogue}:{tenant_id}"))
        .execute(&mut **transaction)
        .await
        .context("Failed to lock Assets and inventory catalogue")?;
    Ok(())
}

struct CreateFingerprint(Sha256);

impl CreateFingerprint {
    fn new(domain: &str) -> Self {
        let mut fingerprint = Self(Sha256::new());
        fingerprint.text(domain);
        fingerprint
    }

    fn text(&mut self, value: &str) {
        self.0.update(b"text");
        self.0.update((value.len() as u64).to_be_bytes());
        self.0.update(value.as_bytes());
    }

    fn optional_text(&mut self, value: Option<&str>) {
        match value {
            Some(value) => {
                self.0.update(b"some");
                self.text(value);
            }
            None => self.0.update(b"none"),
        }
    }

    fn i16(&mut self, value: i16) {
        self.0.update(b"i16");
        self.0.update(value.to_be_bytes());
    }

    fn optional_i64(&mut self, value: Option<i64>) {
        match value {
            Some(value) => {
                self.0.update(b"some");
                self.0.update(b"i64");
                self.0.update(value.to_be_bytes());
            }
            None => self.0.update(b"none"),
        }
    }

    fn finish(self) -> String {
        format!("{:x}", self.0.finalize())
    }
}

fn ensure_replay_fingerprint(stored: &str, requested: &str, catalogue: &str) -> Result<()> {
    if stored != requested {
        bail!("Idempotency key already belongs to another {catalogue} request");
    }
    Ok(())
}

struct CatalogAudit<'a> {
    action: &'a str,
    target_type: &'a str,
    target_id: Uuid,
    metadata: BTreeMap<String, Value>,
}

impl<'a> CatalogAudit<'a> {
    fn new(
        action: &'a str,
        target_type: &'a str,
        target_id: Uuid,
        metadata: BTreeMap<String, Value>,
    ) -> Self {
        Self {
            action,
            target_type,
            target_id,
            metadata,
        }
    }
}

async fn append_catalog_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    audit: CatalogAudit<'_>,
) -> Result<()> {
    append_audit(
        &mut **transaction,
        &NewAuditEvent::new(
            tenant_id,
            actor,
            audit.action,
            AuditOutcome::Succeeded,
            request_context,
        )
        .with_target(AuditTarget::new(
            audit.target_type,
            audit.target_id.to_string(),
        ))
        .with_redacted_metadata(audit.metadata.into_iter().collect()),
    )
    .await
    .context("Failed to append Assets and inventory audit event")?;
    Ok(())
}

fn reference_metadata(field: &str, reference: &str, status: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([
        (field.to_string(), json!(reference)),
        ("status".to_string(), json!(status)),
    ])
}

fn actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("Authenticated person actor is required"))
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE),
        per_page.unwrap_or(DEFAULT_PER_PAGE).clamp(1, MAX_PER_PAGE),
    )
}

fn parse_status_filter<'a>(status: Option<&'a str>, label: &str) -> Result<Option<&'a str>> {
    let status = status.map(str::trim).filter(|value| !value.is_empty());
    if status.is_some_and(|value| !matches!(value, "active" | "inactive")) {
        bail!("{label} status filter is invalid");
    }
    Ok(status)
}

fn search_pattern(search: Option<&str>) -> Result<Option<String>> {
    let search = search.map(str::trim).filter(|value| !value.is_empty());
    if search.is_some_and(|value| value.chars().count() > MAX_SEARCH_LENGTH) {
        bail!("Catalogue search cannot exceed 200 characters");
    }
    Ok(search.map(|value| format!("%{value}%")))
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

fn ensure_version(actual: i32, expected: i32, label: &str) -> Result<()> {
    if actual != expected {
        bail!("{label} changed since it was loaded");
    }
    Ok(())
}

fn validate_reorder_level(value: Option<i64>) -> Result<()> {
    if value.is_some_and(|value| !(0..=MAX_SAFE_INTEGER).contains(&value)) {
        bail!("Item reorder level must be between zero and 9007199254740991");
    }
    Ok(())
}

fn ensure_one_row(rows: u64, label: &str) -> Result<()> {
    if rows != 1 {
        bail!("{label} changed since it was loaded");
    }
    Ok(())
}

fn format_reference(prefix: &str, number: i64) -> Result<String> {
    if !(1..=MAX_REFERENCE_NUMBER).contains(&number) {
        bail!("Catalogue reference sequence is exhausted");
    }
    Ok(format!("{prefix}-{number:06}"))
}

fn database_error(error: sqlx::Error, context: &'static str) -> anyhow::Error {
    if let sqlx::Error::Database(database) = &error
        && database.code().as_deref() == Some("23505")
    {
        return anyhow!("An Assets and inventory identity already exists");
    }
    anyhow::Error::new(error).context(context)
}

#[cfg(test)]
mod tests {
    use super::{
        CreateItemValues, CreateOutcome, CreateStoreValues, MAX_SAFE_INTEGER, bounded_page,
        create_item_in_transaction, create_store_in_transaction, ensure_replay_fingerprint,
        ensure_version, format_reference, optional, parse_status_filter, search_pattern,
        validate_reorder_level,
    };
    use crate::dtos::{AssetStatus, CreateItemRequest, CreateStoreRequest, UpdateItemRequest};
    use validator::Validate;

    #[test]
    fn catalogue_boundaries_are_canonical_and_bounded() {
        assert_eq!(bounded_page(None, None), (1, 25));
        assert_eq!(bounded_page(Some(-3), Some(900)), (1, 100));
        assert_eq!(bounded_page(Some(i64::MAX), Some(25)), (1_000_000, 25));
        assert_eq!(
            search_pattern(Some("  chalk  ")).unwrap().as_deref(),
            Some("%chalk%")
        );
        assert_eq!(search_pattern(Some("   ")).unwrap(), None);
        assert!(search_pattern(Some(&"x".repeat(201))).is_err());
        assert_eq!(
            optional(Some("  Main store  ")).as_deref(),
            Some("Main store")
        );
        assert_eq!(optional(Some("  ")), None);
    }

    #[test]
    fn status_versions_and_references_are_closed() {
        assert_eq!(
            parse_status_filter(Some("active"), "Item").unwrap(),
            Some("active")
        );
        assert!(parse_status_filter(Some("retired"), "Item").is_err());
        assert!(ensure_version(2, 2, "Item").is_ok());
        assert!(ensure_version(2, 1, "Item").is_err());
        assert_eq!(format_reference("ITM", 1).unwrap(), "ITM-000001");
        assert_eq!(format_reference("STR", 999_999).unwrap(), "STR-999999");
        assert!(format_reference("ITM", 0).is_err());
        assert!(format_reference("ITM", 1_000_000).is_err());
    }

    #[test]
    fn reorder_levels_stay_within_the_exact_json_integer_boundary() {
        assert!(validate_reorder_level(Some(MAX_SAFE_INTEGER)).is_ok());
        assert!(validate_reorder_level(Some(MAX_SAFE_INTEGER + 1)).is_err());
        assert!(validate_reorder_level(Some(-1)).is_err());

        let create = CreateItemRequest {
            name: "Boundary item".to_string(),
            description: None,
            barcode: None,
            unit_label: "each".to_string(),
            quantity_scale: 0,
            reorder_level_minor: Some(MAX_SAFE_INTEGER),
            idempotency_key: "boundary-create".to_string(),
        };
        assert!(create.validate().is_ok());
        let invalid_create = CreateItemRequest {
            reorder_level_minor: Some(MAX_SAFE_INTEGER + 1),
            ..create
        };
        assert!(invalid_create.validate().is_err());

        let update = UpdateItemRequest {
            name: "Boundary item".to_string(),
            description: None,
            barcode: None,
            reorder_level_minor: Some(MAX_SAFE_INTEGER),
            status: AssetStatus::Active,
            expected_version: 1,
        };
        assert!(update.validate().is_ok());
        let invalid_update = UpdateItemRequest {
            reorder_level_minor: Some(MAX_SAFE_INTEGER + 1),
            ..update
        };
        assert!(invalid_update.validate().is_err());
    }

    #[test]
    fn idempotency_uses_the_immutable_normalized_create_fingerprint() {
        let item = CreateItemValues::parse(&CreateItemRequest {
            name: "  Exercise book  ".to_string(),
            description: Some("  A4 ruled  ".to_string()),
            barcode: Some("  BOOK-A4  ".to_string()),
            unit_label: "  each  ".to_string(),
            quantity_scale: 0,
            reorder_level_minor: Some(10),
            idempotency_key: "item-key".to_string(),
        })
        .unwrap();
        let original = item.fingerprint();
        assert_eq!(original.len(), 64);
        assert!(ensure_replay_fingerprint(&original, &item.fingerprint(), "item").is_ok());

        let changed_item = CreateItemValues::parse(&CreateItemRequest {
            name: "Different book".to_string(),
            description: Some("A4 ruled".to_string()),
            barcode: Some("BOOK-A4".to_string()),
            unit_label: "each".to_string(),
            quantity_scale: 0,
            reorder_level_minor: Some(10),
            idempotency_key: "item-key".to_string(),
        })
        .unwrap();
        assert!(ensure_replay_fingerprint(&original, &changed_item.fingerprint(), "item").is_err());

        let store = CreateStoreValues::parse(&CreateStoreRequest {
            name: "  Main store  ".to_string(),
            location_label: Some("  Block A  ".to_string()),
            notes: None,
            idempotency_key: "store-key".to_string(),
        })
        .unwrap();
        assert!(
            ensure_replay_fingerprint(&store.fingerprint(), &store.fingerprint(), "store").is_ok()
        );
        let changed_store = CreateStoreValues::parse(&CreateStoreRequest {
            name: "Other store".to_string(),
            location_label: Some("Block A".to_string()),
            notes: None,
            idempotency_key: "store-key".to_string(),
        })
        .unwrap();
        assert!(
            ensure_replay_fingerprint(&store.fingerprint(), &changed_store.fingerprint(), "store")
                .is_err()
        );
    }

    #[actix_web::test]
    #[ignore = "requires a migrated PostgreSQL DATABASE_URL"]
    async fn service_replays_original_create_commands_after_catalogue_edits() {
        let database_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL is required for the Assets and inventory service contract");
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("Assets and inventory service contract could not connect");
        let mut transaction = pool
            .begin()
            .await
            .expect("Assets and inventory service contract transaction could not start");
        let migration =
            include_str!("../../../../migrations/081_create_assets_inventory_catalogue.sql");
        sqlx::raw_sql(migration)
            .execute(&mut *transaction)
            .await
            .expect("Migration 081 must apply for the service contract");
        sqlx::raw_sql(migration)
            .execute(&mut *transaction)
            .await
            .expect("Migration 081 must replay for the service contract");
        let tenant_id = uuid::Uuid::parse_str("81100000-0000-0000-0000-000000000001")
            .expect("static service contract tenant UUID must parse");
        let actor_id = uuid::Uuid::parse_str("81100000-0000-0000-0000-000000000002")
            .expect("static service contract actor UUID must parse");
        sqlx::query(
            "INSERT INTO tenants (id, slug, name) VALUES ($1, 'assets-service-081', 'Assets Service Contract')",
        )
        .bind(tenant_id)
        .execute(&mut *transaction)
        .await
        .expect("service contract tenant must insert");
        sqlx::query(
            r#"
            INSERT INTO users (id, tenant_id, email, password_hash, full_name)
            VALUES ($1, $2, 'assets-service@example.invalid', 'not-a-login',
                    'Assets Service Contract')
            "#,
        )
        .bind(actor_id)
        .bind(tenant_id)
        .execute(&mut *transaction)
        .await
        .expect("service contract actor must insert");

        let item_request = CreateItemRequest {
            name: "Exercise book".to_string(),
            description: Some("A4 ruled".to_string()),
            barcode: Some("SERVICE-BOOK-081".to_string()),
            unit_label: "each".to_string(),
            quantity_scale: 0,
            reorder_level_minor: Some(10),
            idempotency_key: "assets-service-item-081".to_string(),
        };
        let item_values = CreateItemValues::parse(&item_request).unwrap();
        let created_item =
            match create_item_in_transaction(&mut transaction, tenant_id, actor_id, &item_values)
                .await
                .unwrap()
            {
                CreateOutcome::Created(item) => item,
                CreateOutcome::Replayed(_) => panic!("first item command cannot be a replay"),
            };
        sqlx::query(
            r#"
            UPDATE assets_inventory_items
               SET name = 'Edited exercise book', description = 'Edited after creation',
                   status = 'inactive', version = version + 1, updated_by = $2
             WHERE id = $1
            "#,
        )
        .bind(created_item.id)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .unwrap();
        let replayed_item =
            match create_item_in_transaction(&mut transaction, tenant_id, actor_id, &item_values)
                .await
                .unwrap()
            {
                CreateOutcome::Replayed(item) => item,
                CreateOutcome::Created(_) => panic!("original item command must replay"),
            };
        assert_eq!(replayed_item.id, created_item.id);
        assert_eq!(replayed_item.name, "Edited exercise book");
        assert_eq!(replayed_item.status, "inactive");
        assert_eq!(replayed_item.version, 2);
        let changed_item_values = CreateItemValues::parse(&CreateItemRequest {
            name: "Different original item".to_string(),
            description: item_request.description.clone(),
            barcode: item_request.barcode.clone(),
            unit_label: item_request.unit_label.clone(),
            quantity_scale: item_request.quantity_scale,
            reorder_level_minor: item_request.reorder_level_minor,
            idempotency_key: item_request.idempotency_key.clone(),
        })
        .unwrap();
        let item_conflict =
            create_item_in_transaction(&mut transaction, tenant_id, actor_id, &changed_item_values)
                .await
                .unwrap_err();
        assert!(item_conflict.to_string().contains("another item request"));

        let store_request = CreateStoreRequest {
            name: "Main store".to_string(),
            location_label: Some("Block A".to_string()),
            notes: Some("Original notes".to_string()),
            idempotency_key: "assets-service-store-081".to_string(),
        };
        let store_values = CreateStoreValues::parse(&store_request).unwrap();
        let created_store =
            match create_store_in_transaction(&mut transaction, tenant_id, actor_id, &store_values)
                .await
                .unwrap()
            {
                CreateOutcome::Created(store) => store,
                CreateOutcome::Replayed(_) => panic!("first store command cannot be a replay"),
            };
        sqlx::query(
            r#"
            UPDATE assets_inventory_stores
               SET name = 'Edited main store', notes = 'Edited after creation',
                   status = 'inactive', version = version + 1, updated_by = $2
             WHERE id = $1
            "#,
        )
        .bind(created_store.id)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .unwrap();
        let replayed_store =
            match create_store_in_transaction(&mut transaction, tenant_id, actor_id, &store_values)
                .await
                .unwrap()
            {
                CreateOutcome::Replayed(store) => store,
                CreateOutcome::Created(_) => panic!("original store command must replay"),
            };
        assert_eq!(replayed_store.id, created_store.id);
        assert_eq!(replayed_store.name, "Edited main store");
        assert_eq!(replayed_store.status, "inactive");
        assert_eq!(replayed_store.version, 2);
        let changed_store_values = CreateStoreValues::parse(&CreateStoreRequest {
            name: "Different original store".to_string(),
            location_label: store_request.location_label.clone(),
            notes: store_request.notes.clone(),
            idempotency_key: store_request.idempotency_key.clone(),
        })
        .unwrap();
        let store_conflict = create_store_in_transaction(
            &mut transaction,
            tenant_id,
            actor_id,
            &changed_store_values,
        )
        .await
        .unwrap_err();
        assert!(store_conflict.to_string().contains("another store request"));
        transaction
            .rollback()
            .await
            .expect("Assets and inventory service contract did not roll back");
    }

    #[actix_web::test]
    #[ignore = "requires a migrated PostgreSQL DATABASE_URL"]
    async fn database_enforces_catalogue_contracts() {
        let database_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL is required for the Assets and inventory database contract");
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("Assets and inventory database contract could not connect");
        let mut transaction = pool
            .begin()
            .await
            .expect("Assets and inventory database contract transaction could not start");
        let migration =
            include_str!("../../../../migrations/081_create_assets_inventory_catalogue.sql");
        sqlx::raw_sql(migration)
            .execute(&mut *transaction)
            .await
            .expect("Migration 081 must apply over the current schema");
        sqlx::raw_sql(
            r#"
            ALTER TABLE assets_inventory_items
                DROP CONSTRAINT assets_inventory_items_reorder_level_minor_check;
            ALTER TABLE assets_inventory_items
                ADD CONSTRAINT assets_inventory_items_reorder_level_minor_check
                CHECK (reorder_level_minor IS NULL OR reorder_level_minor >= 0);
            "#,
        )
        .execute(&mut *transaction)
        .await
        .expect("Database contract must recreate the legacy weak reorder constraint");
        sqlx::raw_sql(migration)
            .execute(&mut *transaction)
            .await
            .expect("Migration 081 must repair the legacy weak reorder constraint on replay");
        sqlx::raw_sql(
            r#"
            DO $$
            DECLARE
                first_tenant UUID := '81000000-0000-0000-0000-000000000001';
                second_tenant UUID := '81000000-0000-0000-0000-000000000002';
                missing_sequence_tenant UUID := '81000000-0000-0000-0000-000000000011';
                first_actor UUID := '81000000-0000-0000-0000-000000000003';
                second_actor UUID := '81000000-0000-0000-0000-000000000004';
                missing_sequence_actor UUID := '81000000-0000-0000-0000-000000000012';
                alternate_first_actor UUID := '81000000-0000-0000-0000-000000000008';
                first_item UUID := '81000000-0000-0000-0000-000000000005';
                second_item UUID := '81000000-0000-0000-0000-000000000006';
                first_store UUID := '81000000-0000-0000-0000-000000000007';
                boundary_item UUID := '81000000-0000-0000-0000-000000000010';
                sequence_value BIGINT;
                affected_rows BIGINT;
                error_message TEXT;
                guard_case INTEGER;
                invalid_status TEXT;
                invalid_version INTEGER;
                invalid_updated_by UUID;
                invalid_deleted_at TIMESTAMPTZ;
                invalid_deleted_by UUID;
            BEGIN
                INSERT INTO tenants (id, slug, name)
                VALUES
                    (first_tenant, 'assets-contract-081-a', 'Assets Contract A'),
                    (second_tenant, 'assets-contract-081-b', 'Assets Contract B'),
                    (missing_sequence_tenant, 'assets-contract-081-no-sequence',
                        'Assets Contract Without Sequence');
                INSERT INTO users (id, tenant_id, email, password_hash, full_name)
                VALUES
                    (first_actor, first_tenant, 'assets-contract-a@example.invalid',
                        'not-a-login', 'Assets Contract A'),
                    (alternate_first_actor, first_tenant,
                        'assets-contract-a-alternate@example.invalid',
                        'not-a-login', 'Assets Contract A Alternate'),
                    (second_actor, second_tenant, 'assets-contract-b@example.invalid',
                        'not-a-login', 'Assets Contract B'),
                    (missing_sequence_actor, missing_sequence_tenant,
                        'assets-contract-no-sequence@example.invalid',
                        'not-a-login', 'Assets Contract Without Sequence');

                INSERT INTO assets_inventory_item_sequences (tenant_id, last_number)
                VALUES (first_tenant, 1)
                ON CONFLICT (tenant_id)
                DO UPDATE SET last_number = assets_inventory_item_sequences.last_number + 1
                RETURNING last_number INTO sequence_value;
                IF sequence_value <> 1 THEN
                    RAISE EXCEPTION 'First tenant item sequence did not begin at one';
                END IF;
                INSERT INTO assets_inventory_item_sequences (tenant_id, last_number)
                VALUES (first_tenant, 1)
                ON CONFLICT (tenant_id)
                DO UPDATE SET last_number = assets_inventory_item_sequences.last_number + 1
                RETURNING last_number INTO sequence_value;
                IF sequence_value <> 2 THEN
                    RAISE EXCEPTION 'First tenant item sequence did not advance';
                END IF;
                INSERT INTO assets_inventory_item_sequences (tenant_id, last_number)
                VALUES (second_tenant, 1)
                ON CONFLICT (tenant_id)
                DO UPDATE SET last_number = assets_inventory_item_sequences.last_number + 1
                RETURNING last_number INTO sequence_value;
                IF sequence_value <> 1 THEN
                    RAISE EXCEPTION 'Second tenant item sequence was not isolated';
                END IF;
                INSERT INTO assets_inventory_store_sequences (tenant_id, last_number)
                VALUES (first_tenant, 1)
                ON CONFLICT (tenant_id)
                DO UPDATE SET last_number = assets_inventory_store_sequences.last_number + 1
                RETURNING last_number INTO sequence_value;
                IF sequence_value <> 1 THEN
                    RAISE EXCEPTION 'First tenant store sequence did not begin at one';
                END IF;
                INSERT INTO assets_inventory_store_sequences (tenant_id, last_number)
                VALUES (first_tenant, 1)
                ON CONFLICT (tenant_id)
                DO UPDATE SET last_number = assets_inventory_store_sequences.last_number + 1
                RETURNING last_number INTO sequence_value;
                IF sequence_value <> 2 THEN
                    RAISE EXCEPTION 'First tenant store sequence did not advance';
                END IF;
                INSERT INTO assets_inventory_store_sequences (tenant_id, last_number)
                VALUES (second_tenant, 1)
                ON CONFLICT (tenant_id)
                DO UPDATE SET last_number = assets_inventory_store_sequences.last_number + 1
                RETURNING last_number INTO sequence_value;
                IF sequence_value <> 1 THEN
                    RAISE EXCEPTION 'Second tenant store sequence was not isolated';
                END IF;

                BEGIN
                    INSERT INTO assets_inventory_item_sequences (tenant_id, last_number)
                    VALUES ('81000000-0000-0000-0000-000000000009', 0);
                    RAISE EXCEPTION 'Item sequence initial-state guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Asset inventory item sequence must begin at one' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    INSERT INTO assets_inventory_store_sequences (tenant_id, last_number)
                    VALUES ('81000000-0000-0000-0000-000000000009', 0);
                    RAISE EXCEPTION 'Store sequence initial-state guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Asset inventory store sequence must begin at one' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    UPDATE assets_inventory_item_sequences
                       SET last_number = last_number - 1
                     WHERE tenant_id = first_tenant;
                    RAISE EXCEPTION 'Item sequence decrement guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Asset inventory item sequence must advance by one' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    UPDATE assets_inventory_item_sequences SET last_number = 1
                     WHERE tenant_id = first_tenant;
                    RAISE EXCEPTION 'Item sequence reset guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Asset inventory item sequence must advance by one' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    UPDATE assets_inventory_item_sequences
                       SET last_number = last_number + 1,
                           created_at = created_at - INTERVAL '1 day'
                     WHERE tenant_id = first_tenant;
                    RAISE EXCEPTION 'Item sequence source guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <>
                        'Asset inventory item sequence source fields are immutable' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    DELETE FROM assets_inventory_item_sequences
                     WHERE tenant_id = first_tenant;
                    RAISE EXCEPTION 'Item sequence delete guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Asset inventory item sequence rows cannot be deleted' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    UPDATE assets_inventory_store_sequences
                       SET last_number = last_number - 1
                     WHERE tenant_id = first_tenant;
                    RAISE EXCEPTION 'Store sequence decrement guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Asset inventory store sequence must advance by one' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    UPDATE assets_inventory_store_sequences SET last_number = 1
                     WHERE tenant_id = first_tenant;
                    RAISE EXCEPTION 'Store sequence reset guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Asset inventory store sequence must advance by one' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    UPDATE assets_inventory_store_sequences
                       SET last_number = last_number + 1, deleted_at = NOW()
                     WHERE tenant_id = first_tenant;
                    RAISE EXCEPTION 'Store sequence deletion-state guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <>
                        'Asset inventory store sequence source fields are immutable' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    DELETE FROM assets_inventory_store_sequences
                     WHERE tenant_id = first_tenant;
                    RAISE EXCEPTION 'Store sequence delete guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Asset inventory store sequence rows cannot be deleted' THEN
                        RAISE;
                    END IF;
                END;

                FOR guard_case IN 1..4 LOOP
                    invalid_status := 'active';
                    invalid_version := 1;
                    invalid_updated_by := first_actor;
                    invalid_deleted_at := NULL;
                    invalid_deleted_by := NULL;
                    IF guard_case = 1 THEN
                        invalid_status := 'inactive';
                    ELSIF guard_case = 2 THEN
                        invalid_version := 2;
                    ELSIF guard_case = 3 THEN
                        invalid_updated_by := alternate_first_actor;
                    ELSE
                        invalid_deleted_at := NOW();
                        invalid_deleted_by := first_actor;
                    END IF;
                    BEGIN
                        INSERT INTO assets_inventory_items (
                            tenant_id, item_number, name, unit_label, quantity_scale,
                            status, version, idempotency_key, create_request_fingerprint,
                            created_by, updated_by, deleted_at, deleted_by
                        ) VALUES (
                            first_tenant, 'ITM-000098', 'Invalid initial item', 'each', 0,
                            invalid_status, invalid_version, 'assets-invalid-initial-item',
                            REPEAT('e', 64), first_actor, invalid_updated_by,
                            invalid_deleted_at, invalid_deleted_by
                        );
                        RAISE EXCEPTION 'Item initial-state guard case % did not fire', guard_case;
                    EXCEPTION WHEN OTHERS THEN
                        GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                        IF error_message <>
                            'Asset inventory items must begin active at version one' THEN
                            RAISE;
                        END IF;
                    END;
                END LOOP;
                FOR guard_case IN 1..4 LOOP
                    invalid_status := 'active';
                    invalid_version := 1;
                    invalid_updated_by := first_actor;
                    invalid_deleted_at := NULL;
                    invalid_deleted_by := NULL;
                    IF guard_case = 1 THEN
                        invalid_status := 'inactive';
                    ELSIF guard_case = 2 THEN
                        invalid_version := 2;
                    ELSIF guard_case = 3 THEN
                        invalid_updated_by := alternate_first_actor;
                    ELSE
                        invalid_deleted_at := NOW();
                        invalid_deleted_by := first_actor;
                    END IF;
                    BEGIN
                        INSERT INTO assets_inventory_stores (
                            tenant_id, store_number, name, status, version, idempotency_key,
                            create_request_fingerprint, created_by, updated_by,
                            deleted_at, deleted_by
                        ) VALUES (
                            first_tenant, 'STR-000098', 'Invalid initial store',
                            invalid_status, invalid_version, 'assets-invalid-initial-store',
                            REPEAT('f', 64), first_actor, invalid_updated_by,
                            invalid_deleted_at, invalid_deleted_by
                        );
                        RAISE EXCEPTION 'Store initial-state guard case % did not fire', guard_case;
                    EXCEPTION WHEN OTHERS THEN
                        GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                        IF error_message <>
                            'Asset inventory stores must begin active at version one' THEN
                            RAISE;
                        END IF;
                    END;
                END LOOP;

                BEGIN
                    INSERT INTO assets_inventory_items (
                        tenant_id, item_number, name, unit_label, quantity_scale,
                        idempotency_key, create_request_fingerprint, created_by, updated_by
                    ) VALUES (
                        first_tenant, 'ITM-000099', 'Wrong allocated item number', 'each', 0,
                        'assets-contract-item-wrong-sequence', REPEAT('1', 64),
                        first_actor, first_actor
                    );
                    RAISE EXCEPTION 'Wrong item sequence reference guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <>
                        'Asset inventory item number must match the allocated tenant sequence' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    INSERT INTO assets_inventory_items (
                        tenant_id, item_number, name, unit_label, quantity_scale,
                        idempotency_key, create_request_fingerprint, created_by, updated_by
                    ) VALUES (
                        missing_sequence_tenant, 'ITM-000001',
                        'Item without allocated sequence', 'each', 0,
                        'assets-contract-item-missing-sequence', REPEAT('2', 64),
                        missing_sequence_actor, missing_sequence_actor
                    );
                    RAISE EXCEPTION 'Missing item sequence guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <>
                        'Asset inventory item reference requires an allocated tenant sequence' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    INSERT INTO assets_inventory_items (
                        tenant_id, item_number, name, unit_label, quantity_scale,
                        idempotency_key, create_request_fingerprint, created_by, updated_by
                    ) VALUES (
                        first_tenant, 'ITM-000001', 'Cross-tenant sequence item', 'each', 0,
                        'assets-contract-item-cross-tenant-sequence', REPEAT('3', 64),
                        first_actor, first_actor
                    );
                    RAISE EXCEPTION 'Cross-tenant item sequence guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <>
                        'Asset inventory item number must match the allocated tenant sequence' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    INSERT INTO assets_inventory_stores (
                        tenant_id, store_number, name, idempotency_key,
                        create_request_fingerprint, created_by, updated_by
                    ) VALUES (
                        first_tenant, 'STR-000099', 'Wrong allocated store number',
                        'assets-contract-store-wrong-sequence', REPEAT('4', 64),
                        first_actor, first_actor
                    );
                    RAISE EXCEPTION 'Wrong store sequence reference guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <>
                        'Asset inventory store number must match the allocated tenant sequence' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    INSERT INTO assets_inventory_stores (
                        tenant_id, store_number, name, idempotency_key,
                        create_request_fingerprint, created_by, updated_by
                    ) VALUES (
                        missing_sequence_tenant, 'STR-000001',
                        'Store without allocated sequence',
                        'assets-contract-store-missing-sequence', REPEAT('5', 64),
                        missing_sequence_actor, missing_sequence_actor
                    );
                    RAISE EXCEPTION 'Missing store sequence guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <>
                        'Asset inventory store reference requires an allocated tenant sequence' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    INSERT INTO assets_inventory_stores (
                        tenant_id, store_number, name, idempotency_key,
                        create_request_fingerprint, created_by, updated_by
                    ) VALUES (
                        first_tenant, 'STR-000001', 'Cross-tenant sequence store',
                        'assets-contract-store-cross-tenant-sequence', REPEAT('6', 64),
                        first_actor, first_actor
                    );
                    RAISE EXCEPTION 'Cross-tenant store sequence guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <>
                        'Asset inventory store number must match the allocated tenant sequence' THEN
                        RAISE;
                    END IF;
                END;

                BEGIN
                    INSERT INTO assets_inventory_items (
                        tenant_id, item_number, name, unit_label, quantity_scale,
                        idempotency_key, create_request_fingerprint, created_by, updated_by
                    ) VALUES (
                        first_tenant, 'ITM-000002', 'Wrong tenant actor', 'each', 0,
                        'assets-contract-cross-tenant', REPEAT('d', 64),
                        second_actor, second_actor
                    );
                    RAISE EXCEPTION 'Cross-tenant item actor guard did not fire';
                EXCEPTION WHEN foreign_key_violation THEN
                    NULL;
                END;

                INSERT INTO assets_inventory_items (
                    id, tenant_id, item_number, name, description, barcode, unit_label,
                    quantity_scale, reorder_level_minor, idempotency_key,
                    create_request_fingerprint, created_by, updated_by
                ) VALUES (
                    first_item, first_tenant, 'ITM-000002', 'Exercise book',
                    'A4 ruled book', 'BOOK-A4-081', 'each', 0, 10,
                    'assets-contract-item-a', REPEAT('a', 64), first_actor, first_actor
                );
                INSERT INTO assets_inventory_items (
                    id, tenant_id, item_number, name, unit_label, quantity_scale,
                    idempotency_key, create_request_fingerprint, created_by, updated_by
                ) VALUES (
                    second_item, second_tenant, 'ITM-000001', 'Exercise book',
                    'each', 0, 'assets-contract-item-b', REPEAT('b', 64),
                    second_actor, second_actor
                );
                INSERT INTO assets_inventory_item_sequences (tenant_id, last_number)
                VALUES (second_tenant, 1)
                ON CONFLICT (tenant_id)
                DO UPDATE SET last_number = assets_inventory_item_sequences.last_number + 1
                RETURNING last_number INTO sequence_value;
                IF sequence_value <> 2 THEN
                    RAISE EXCEPTION 'Second tenant item sequence did not advance for boundary item';
                END IF;
                INSERT INTO assets_inventory_items (
                    id, tenant_id, item_number, name, unit_label, quantity_scale,
                    reorder_level_minor, idempotency_key, create_request_fingerprint,
                    created_by, updated_by
                ) VALUES (
                    boundary_item, second_tenant, 'ITM-000002', 'Maximum safe item',
                    'each', 0, 9007199254740991, 'assets-contract-item-max-safe',
                    REPEAT('9', 64), second_actor, second_actor
                );
                IF (SELECT reorder_level_minor FROM assets_inventory_items
                     WHERE id = boundary_item) <> 9007199254740991 THEN
                    RAISE EXCEPTION 'Maximum safe reorder level was not stored exactly';
                END IF;
                INSERT INTO assets_inventory_item_sequences (tenant_id, last_number)
                VALUES (second_tenant, 1)
                ON CONFLICT (tenant_id)
                DO UPDATE SET last_number = assets_inventory_item_sequences.last_number + 1
                RETURNING last_number INTO sequence_value;
                IF sequence_value <> 3 THEN
                    RAISE EXCEPTION 'Second tenant item sequence did not advance for unsafe item';
                END IF;
                BEGIN
                    INSERT INTO assets_inventory_items (
                        tenant_id, item_number, name, unit_label, quantity_scale,
                        reorder_level_minor, idempotency_key, create_request_fingerprint,
                        created_by, updated_by
                    ) VALUES (
                        second_tenant, 'ITM-000003', 'Unsafe integer item', 'each', 0,
                        9007199254740992, 'assets-contract-item-unsafe-integer',
                        REPEAT('8', 64), second_actor, second_actor
                    );
                    RAISE EXCEPTION 'Unsafe reorder level guard did not fire';
                EXCEPTION WHEN check_violation THEN
                    NULL;
                END;
                INSERT INTO assets_inventory_stores (
                    id, tenant_id, store_number, name, location_label, idempotency_key,
                    create_request_fingerprint, created_by, updated_by
                ) VALUES (
                    first_store, first_tenant, 'STR-000002', 'Main store', 'Block A',
                    'assets-contract-store-a', REPEAT('c', 64), first_actor, first_actor
                );

                BEGIN
                    DELETE FROM assets_inventory_items WHERE id = first_item;
                    RAISE EXCEPTION 'Item hard delete guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Asset inventory items must be soft deleted' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    DELETE FROM assets_inventory_stores WHERE id = first_store;
                    RAISE EXCEPTION 'Store hard delete guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Asset inventory stores must be soft deleted' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    UPDATE assets_inventory_items
                       SET create_request_fingerprint = REPEAT('e', 64),
                           version = version + 1
                     WHERE id = first_item;
                    RAISE EXCEPTION 'Item creation fingerprint immutability guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Asset inventory item source fields are immutable' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    UPDATE assets_inventory_stores
                       SET create_request_fingerprint = REPEAT('e', 64),
                           version = version + 1
                     WHERE id = first_store;
                    RAISE EXCEPTION 'Store creation fingerprint immutability guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Asset inventory store source fields are immutable' THEN
                        RAISE;
                    END IF;
                END;

                BEGIN
                    UPDATE assets_inventory_items
                       SET item_number = 'ITM-000003', version = version + 1
                     WHERE id = first_item;
                    RAISE EXCEPTION 'Item reference immutability guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Asset inventory item source fields are immutable' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    UPDATE assets_inventory_items
                       SET unit_label = 'box', quantity_scale = 2, version = version + 1
                     WHERE id = first_item;
                    RAISE EXCEPTION 'Item unit and scale immutability guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <>
                        'Asset inventory item unit and quantity scale are immutable' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    UPDATE assets_inventory_items SET name = 'Changed without version'
                     WHERE id = first_item;
                    RAISE EXCEPTION 'Item version guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Asset inventory item version must increment by one' THEN
                        RAISE;
                    END IF;
                END;
                UPDATE assets_inventory_items
                   SET status = 'inactive', version = version + 1, updated_by = first_actor
                 WHERE id = first_item;
                IF (SELECT create_request_fingerprint FROM assets_inventory_items
                     WHERE id = first_item) <> REPEAT('a', 64) THEN
                    RAISE EXCEPTION 'Item edit changed its creation fingerprint';
                END IF;
                UPDATE assets_inventory_items
                   SET name = 'Stale update', version = version + 1
                 WHERE id = first_item AND version = 1;
                GET DIAGNOSTICS affected_rows = ROW_COUNT;
                IF affected_rows <> 0 THEN
                    RAISE EXCEPTION 'Stale item version unexpectedly updated a row';
                END IF;

                BEGIN
                    UPDATE assets_inventory_stores
                       SET store_number = 'STR-000003', version = version + 1
                     WHERE id = first_store;
                    RAISE EXCEPTION 'Store reference immutability guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Asset inventory store source fields are immutable' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    UPDATE assets_inventory_stores
                       SET deleted_at = NOW(), deleted_by = first_actor,
                           version = version + 1
                     WHERE id = first_store;
                    RAISE EXCEPTION 'Active store deletion guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Only an inactive asset inventory store can be removed' THEN
                        RAISE;
                    END IF;
                END;
                BEGIN
                    UPDATE assets_inventory_stores SET notes = 'Changed without version'
                     WHERE id = first_store;
                    RAISE EXCEPTION 'Store version guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'Asset inventory store version must increment by one' THEN
                        RAISE;
                    END IF;
                END;
                UPDATE assets_inventory_stores
                   SET status = 'inactive', version = version + 1, updated_by = first_actor
                 WHERE id = first_store;
                IF (SELECT create_request_fingerprint FROM assets_inventory_stores
                     WHERE id = first_store) <> REPEAT('c', 64) THEN
                    RAISE EXCEPTION 'Store edit changed its creation fingerprint';
                END IF;
                UPDATE assets_inventory_stores
                   SET deleted_at = NOW(), deleted_by = first_actor,
                       updated_by = first_actor, version = version + 1
                 WHERE id = first_store AND version = 2;
                GET DIAGNOSTICS affected_rows = ROW_COUNT;
                IF affected_rows <> 1 THEN
                    RAISE EXCEPTION 'Versioned store delete did not affect exactly one row';
                END IF;
                BEGIN
                    UPDATE assets_inventory_stores
                       SET notes = 'Mutation after deletion', version = version + 1
                     WHERE id = first_store;
                    RAISE EXCEPTION 'Deleted store immutability guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'A deleted asset inventory store is immutable' THEN
                        RAISE;
                    END IF;
                END;

                UPDATE assets_inventory_items
                   SET deleted_at = NOW(), deleted_by = first_actor,
                       updated_by = first_actor, version = version + 1
                 WHERE id = first_item AND version = 2;
                GET DIAGNOSTICS affected_rows = ROW_COUNT;
                IF affected_rows <> 1 THEN
                    RAISE EXCEPTION 'Versioned item delete did not affect exactly one row';
                END IF;
                BEGIN
                    UPDATE assets_inventory_items
                       SET name = 'Mutation after deletion', version = version + 1
                     WHERE id = first_item;
                    RAISE EXCEPTION 'Deleted item immutability guard did not fire';
                EXCEPTION WHEN OTHERS THEN
                    GET STACKED DIAGNOSTICS error_message = MESSAGE_TEXT;
                    IF error_message <> 'A deleted asset inventory item is immutable' THEN
                        RAISE;
                    END IF;
                END;
            END;
            $$;
            "#,
        )
        .execute(&mut *transaction)
        .await
        .expect("Assets and inventory database lifecycle contract failed");
        transaction
            .rollback()
            .await
            .expect("Assets and inventory database contract did not roll back");
    }
}
