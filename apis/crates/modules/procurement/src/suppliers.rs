//
//  cp-procurement
//  suppliers.rs
//
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//
//! Owns tenant suppliers and their versioned lifecycle.

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Utc};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupplierStatus {
    Active,
    Inactive,
}

impl SupplierStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SupplierListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateSupplierRequest {
    #[validate(length(min = 1, max = 180))]
    pub legal_name: String,
    #[validate(length(max = 180))]
    pub trading_name: Option<String>,
    #[validate(length(max = 100))]
    pub registration_number: Option<String>,
    #[validate(length(max = 100))]
    pub tax_number: Option<String>,
    #[validate(length(max = 254))]
    pub email: Option<String>,
    #[validate(length(max = 50))]
    pub phone: Option<String>,
    #[validate(length(max = 1000))]
    pub address: Option<String>,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateSupplierRequest {
    #[validate(length(min = 1, max = 180))]
    pub legal_name: String,
    #[validate(length(max = 180))]
    pub trading_name: Option<String>,
    #[validate(length(max = 100))]
    pub registration_number: Option<String>,
    #[validate(length(max = 100))]
    pub tax_number: Option<String>,
    #[validate(length(max = 254))]
    pub email: Option<String>,
    #[validate(length(max = 50))]
    pub phone: Option<String>,
    #[validate(length(max = 1000))]
    pub address: Option<String>,
    pub status: SupplierStatus,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct SupplierDeleteQuery {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct SupplierResponse {
    pub id: Uuid,
    pub supplier_number: String,
    pub legal_name: String,
    pub trading_name: Option<String>,
    pub registration_number: Option<String>,
    pub tax_number: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub status: String,
    pub version: i32,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedSuppliersResponse {
    pub suppliers: Vec<SupplierResponse>,
}

#[derive(Debug, Clone)]
pub struct SupplierSnapshot {
    pub id: Uuid,
    pub supplier_number: String,
    pub legal_name: String,
}

pub struct SupplierOps;

impl SupplierOps {
    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        search: Option<&str>,
        status: Option<&str>,
    ) -> Result<(Vec<SupplierResponse>, i64)> {
        validate_status(status)?;
        let search = search.map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, SupplierResponse>(
            r#"
            SELECT id, supplier_number, legal_name, trading_name, registration_number,
                   tax_number, email, phone, address, status, version, created_by,
                   created_at, updated_at
              FROM procurement_suppliers
             WHERE tenant_id = $1 AND deleted_at IS NULL
               AND ($2::TEXT IS NULL OR supplier_number ILIKE $2 OR legal_name ILIKE $2
                    OR trading_name ILIKE $2 OR registration_number ILIKE $2)
               AND ($3::TEXT IS NULL OR status = $3)
             ORDER BY legal_name, supplier_number
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
        .context("Failed to list Procurement suppliers")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
              FROM procurement_suppliers
             WHERE tenant_id = $1 AND deleted_at IS NULL
               AND ($2::TEXT IS NULL OR supplier_number ILIKE $2 OR legal_name ILIKE $2
                    OR trading_name ILIKE $2 OR registration_number ILIKE $2)
               AND ($3::TEXT IS NULL OR status = $3)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(status)
        .fetch_one(pool)
        .await
        .context("Failed to count Procurement suppliers")?;
        Ok((rows, total))
    }

    pub async fn get(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<Option<SupplierResponse>> {
        sqlx::query_as::<_, SupplierResponse>(
            r#"
            SELECT id, supplier_number, legal_name, trading_name, registration_number,
                   tax_number, email, phone, address, status, version, created_by,
                   created_at, updated_at
              FROM procurement_suppliers
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to read Procurement supplier")
    }

    pub async fn active_snapshots(
        pool: &PgPool,
        tenant_id: Uuid,
        ids: &[Uuid],
    ) -> Result<Vec<SupplierSnapshot>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        #[derive(FromRow)]
        struct Row {
            id: Uuid,
            supplier_number: String,
            legal_name: String,
        }
        let rows = sqlx::query_as::<_, Row>(
            r#"
            SELECT id, supplier_number, legal_name
              FROM procurement_suppliers
             WHERE tenant_id = $1 AND id = ANY($2) AND status = 'active'
               AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(ids)
        .fetch_all(pool)
        .await
        .context("Failed to load preferred suppliers")?;
        Ok(rows
            .into_iter()
            .map(|row| SupplierSnapshot {
                id: row.id,
                supplier_number: row.supplier_number,
                legal_name: row.legal_name,
            })
            .collect())
    }

    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateSupplierRequest,
    ) -> Result<SupplierResponse> {
        let actor_id = actor_id(actor)?;
        let values = SupplierValues::from_create(request)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start supplier transaction")?;
        lock_tenant(&mut transaction, tenant_id).await?;
        if let Some(existing) =
            supplier_by_idempotency(&mut transaction, tenant_id, values.idempotency_key.as_str())
                .await?
        {
            if !values.matches(&existing) {
                bail!("Idempotency key already belongs to another supplier request");
            }
            transaction.rollback().await.ok();
            return Ok(existing);
        }
        let supplier_number = next_supplier_number(&mut transaction, tenant_id).await?;
        let supplier = sqlx::query_as::<_, SupplierResponse>(
            r#"
            INSERT INTO procurement_suppliers (
                tenant_id, supplier_number, legal_name, trading_name,
                registration_number, tax_number, email, phone, address,
                idempotency_key, created_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id, supplier_number, legal_name, trading_name, registration_number,
                      tax_number, email, phone, address, status, version, created_by,
                      created_at, updated_at
            "#,
        )
        .bind(tenant_id)
        .bind(&supplier_number)
        .bind(&values.legal_name)
        .bind(&values.trading_name)
        .bind(&values.registration_number)
        .bind(&values.tax_number)
        .bind(&values.email)
        .bind(&values.phone)
        .bind(&values.address)
        .bind(&values.idempotency_key)
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "Failed to create Procurement supplier"))?;
        append_supplier_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "procurement.suppliers.create",
            &supplier,
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit supplier transaction")?;
        Ok(supplier)
    }

    pub async fn update(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateSupplierRequest,
    ) -> Result<Option<SupplierResponse>> {
        let values = SupplierValues::from_update(request)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start supplier transaction")?;
        let current = lock_supplier(&mut transaction, tenant_id, id).await?;
        let Some(current) = current else {
            return Ok(None);
        };
        ensure_version(current.version, request.expected_version, "Supplier")?;
        let supplier = sqlx::query_as::<_, SupplierResponse>(
            r#"
            UPDATE procurement_suppliers
               SET legal_name = $3, trading_name = $4, registration_number = $5,
                   tax_number = $6, email = $7, phone = $8, address = $9,
                   status = $10, version = version + 1
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            RETURNING id, supplier_number, legal_name, trading_name, registration_number,
                      tax_number, email, phone, address, status, version, created_by,
                      created_at, updated_at
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(&values.legal_name)
        .bind(&values.trading_name)
        .bind(&values.registration_number)
        .bind(&values.tax_number)
        .bind(&values.email)
        .bind(&values.phone)
        .bind(&values.address)
        .bind(request.status.as_str())
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "Failed to update Procurement supplier"))?;
        append_supplier_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "procurement.suppliers.update",
            &supplier,
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit supplier transaction")?;
        Ok(Some(supplier))
    }

    pub async fn delete(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<bool> {
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start supplier transaction")?;
        let current = lock_supplier(&mut transaction, tenant_id, id).await?;
        let Some(current) = current else {
            return Ok(false);
        };
        ensure_version(current.version, expected_version, "Supplier")?;
        if current.status != "inactive" {
            bail!("Only an inactive supplier can be removed");
        }
        let referenced = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM procurement_requisition_lines
                 WHERE tenant_id = $1 AND preferred_supplier_id = $2 AND deleted_at IS NULL
            )
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to check supplier references")?;
        if referenced {
            bail!("A supplier used by a requisition cannot be removed");
        }
        sqlx::query(
            "UPDATE procurement_suppliers SET deleted_at = NOW(), version = version + 1 WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(id)
        .execute(&mut *transaction)
        .await
        .context("Failed to remove Procurement supplier")?;
        append_supplier_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "procurement.suppliers.delete",
            &current,
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit supplier transaction")?;
        Ok(true)
    }
}

struct SupplierValues {
    legal_name: String,
    trading_name: Option<String>,
    registration_number: Option<String>,
    tax_number: Option<String>,
    email: Option<String>,
    phone: Option<String>,
    address: Option<String>,
    idempotency_key: String,
}

impl SupplierValues {
    fn from_create(request: &CreateSupplierRequest) -> Result<Self> {
        Ok(Self {
            legal_name: required(&request.legal_name, "Supplier legal name")?,
            trading_name: optional(request.trading_name.as_deref()),
            registration_number: optional(request.registration_number.as_deref()),
            tax_number: optional(request.tax_number.as_deref()),
            email: normalized_email(request.email.as_deref())?,
            phone: optional(request.phone.as_deref()),
            address: optional(request.address.as_deref()),
            idempotency_key: required(&request.idempotency_key, "Idempotency key")?,
        })
    }

    fn from_update(request: &UpdateSupplierRequest) -> Result<Self> {
        Ok(Self {
            legal_name: required(&request.legal_name, "Supplier legal name")?,
            trading_name: optional(request.trading_name.as_deref()),
            registration_number: optional(request.registration_number.as_deref()),
            tax_number: optional(request.tax_number.as_deref()),
            email: normalized_email(request.email.as_deref())?,
            phone: optional(request.phone.as_deref()),
            address: optional(request.address.as_deref()),
            idempotency_key: String::new(),
        })
    }

    fn matches(&self, supplier: &SupplierResponse) -> bool {
        self.legal_name == supplier.legal_name
            && self.trading_name == supplier.trading_name
            && self.registration_number == supplier.registration_number
            && self.tax_number == supplier.tax_number
            && self.email == supplier.email
            && self.phone == supplier.phone
            && self.address == supplier.address
    }
}

async fn supplier_by_idempotency(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    key: &str,
) -> Result<Option<SupplierResponse>> {
    sqlx::query_as::<_, SupplierResponse>(
        r#"
        SELECT id, supplier_number, legal_name, trading_name, registration_number,
               tax_number, email, phone, address, status, version, created_by,
               created_at, updated_at
          FROM procurement_suppliers
         WHERE tenant_id = $1 AND idempotency_key = $2 AND deleted_at IS NULL
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(key)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to resolve supplier idempotency")
}

async fn lock_supplier(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<SupplierResponse>> {
    sqlx::query_as::<_, SupplierResponse>(
        r#"
        SELECT id, supplier_number, legal_name, trading_name, registration_number,
               tax_number, email, phone, address, status, version, created_by,
               created_at, updated_at
          FROM procurement_suppliers
         WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
         FOR UPDATE
        "#,
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to lock Procurement supplier")
}

async fn next_supplier_number(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
) -> Result<String> {
    let number = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO procurement_supplier_sequences (tenant_id, last_number)
        VALUES ($1, 1)
        ON CONFLICT (tenant_id)
        DO UPDATE SET last_number = procurement_supplier_sequences.last_number + 1,
                      deleted_at = NULL
        RETURNING last_number
        "#,
    )
    .bind(tenant_id)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to allocate supplier number")?;
    Ok(format!("SUP-{number:06}"))
}

async fn lock_tenant(transaction: &mut Transaction<'_, Postgres>, tenant_id: Uuid) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("procurement-supplier:{tenant_id}"))
        .execute(&mut **transaction)
        .await
        .context("Failed to lock Procurement supplier numbering")?;
    Ok(())
}

async fn append_supplier_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    operation: &str,
    supplier: &SupplierResponse,
) -> Result<()> {
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
            "procurement_supplier",
            supplier.id.to_string(),
        ))
        .with_redacted_metadata(
            json!({
                "supplier_number": supplier.supplier_number,
                "status": supplier.status,
            })
            .as_object()
            .cloned()
            .unwrap_or_default(),
        ),
    )
    .await
    .context("Failed to audit Procurement supplier")?;
    Ok(())
}

fn actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .ok_or_else(|| anyhow!("Authenticated person actor is required"))
}

fn validate_status(status: Option<&str>) -> Result<()> {
    if status.is_some_and(|value| !matches!(value, "active" | "inactive")) {
        bail!("Supplier status filter is invalid");
    }
    Ok(())
}

fn ensure_version(actual: i32, expected: i32, label: &str) -> Result<()> {
    if actual != expected {
        bail!("{label} changed since it was loaded");
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

fn normalized_email(value: Option<&str>) -> Result<Option<String>> {
    let value = optional(value).map(|value| value.to_ascii_lowercase());
    if value
        .as_deref()
        .is_some_and(|email| !email.contains('@') || email.starts_with('@') || email.ends_with('@'))
    {
        bail!("Supplier email address is invalid");
    }
    Ok(value)
}

fn database_error(error: sqlx::Error, context: &'static str) -> anyhow::Error {
    if let sqlx::Error::Database(database) = &error
        && database.code().as_deref() == Some("23505")
    {
        return anyhow!("A supplier with those identifiers already exists");
    }
    anyhow::Error::new(error).context(context)
}

#[cfg(test)]
mod tests {
    use super::{ensure_version, normalized_email, optional, validate_status};

    #[test]
    fn supplier_boundaries_normalize_optional_values() {
        assert_eq!(
            optional(Some("  Delta Supplies  ")).as_deref(),
            Some("Delta Supplies")
        );
        assert_eq!(optional(Some("  ")), None);
        assert_eq!(
            normalized_email(Some("  OPS@EXAMPLE.COM ")).unwrap(),
            Some("ops@example.com".to_string())
        );
        assert!(normalized_email(Some("not-an-email")).is_err());
    }

    #[test]
    fn supplier_status_and_versions_are_explicit() {
        assert!(validate_status(Some("active")).is_ok());
        assert!(validate_status(Some("pending")).is_err());
        assert!(ensure_version(3, 3, "Supplier").is_ok());
        assert!(ensure_version(3, 2, "Supplier").is_err());
    }
}
