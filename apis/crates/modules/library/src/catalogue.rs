//! Tenant-scoped Library title and physical-copy operations.

use anyhow::{Context, Result, anyhow, bail};
use cp_audit::{AuditActor, RequestContext};
use cp_finance::ledger::CurrencyOps;
use serde_json::json;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    CopyResponse, CreateCopyRequest, CreateTitleRequest, DirectoryQuery, TitleDetail, TitleSummary,
    UpdateCopyRequest, UpdateTitleRequest,
    models::{CopyRow, TitleDetailRow, TitleSummaryRow},
    settings::{
        LibraryActivityEvent, allocate_accession, append_domain_audit, append_event,
        person_actor_id,
    },
};

pub struct LibraryCatalogueOps;

impl LibraryCatalogueOps {
    pub async fn list_titles(
        pool: &PgPool,
        tenant_id: Uuid,
        query: &DirectoryQuery,
    ) -> Result<(Vec<TitleSummary>, i64)> {
        validate_title_status(query.status.as_deref())?;
        let (page, per_page) = bounded_page(query.page, query.per_page);
        let search = query
            .search
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| format!("%{value}%"));
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, TitleSummaryRow>(TITLE_SUMMARY_SELECT)
            .bind(tenant_id)
            .bind(&search)
            .bind(query.status.as_deref())
            .bind(per_page)
            .bind(offset)
            .fetch_all(pool)
            .await
            .context("Failed to list Library titles")?;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM library_titles
             WHERE tenant_id = $1 AND deleted_at IS NULL
               AND ($2::TEXT IS NULL OR title ILIKE $2 OR isbn ILIKE $2
                    OR subject ILIKE $2 OR array_to_string(authors, ' ') ILIKE $2)
               AND ($3::TEXT IS NULL OR status = $3)
            "#,
        )
        .bind(tenant_id)
        .bind(&search)
        .bind(query.status.as_deref())
        .fetch_one(pool)
        .await
        .context("Failed to count Library titles")?;
        Ok((rows.into_iter().map(title_summary).collect(), total))
    }

    pub async fn get_title(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<TitleDetail>> {
        let row = sqlx::query_as::<_, TitleDetailRow>(TITLE_DETAIL_SELECT)
            .bind(tenant_id)
            .bind(id)
            .fetch_optional(pool)
            .await
            .context("Failed to load the Library title")?;
        let Some(row) = row else {
            return Ok(None);
        };
        let currency = if let Some(currency_id) = row.replacement_currency_id {
            CurrencyOps::get_by_id(pool, tenant_id, currency_id).await?
        } else {
            None
        };
        Ok(Some(title_detail(row, currency.as_ref())))
    }

    pub async fn create_title(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateTitleRequest,
    ) -> Result<TitleDetail> {
        let actor_id = person_actor_id(actor)?;
        let fields = normalize_title(request)?;
        validate_replacement_currency(
            pool,
            tenant_id,
            request.replacement_cost_minor,
            request.replacement_currency_id,
        )
        .await?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start title creation")?;
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO library_titles (
                id, tenant_id, isbn, title, subtitle, authors, publisher,
                publication_year, edition, language_code, subject,
                replacement_cost_minor, replacement_currency_id, created_by
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .bind(fields.isbn)
        .bind(fields.title)
        .bind(fields.subtitle)
        .bind(fields.authors)
        .bind(fields.publisher)
        .bind(request.publication_year)
        .bind(fields.edition)
        .bind(fields.language_code)
        .bind(fields.subject)
        .bind(request.replacement_cost_minor)
        .bind(request.replacement_currency_id)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "Failed to create the Library title"))?;
        record_change(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "title",
            id,
            "created",
            "library.titles.create",
            json!({ "status": "active" }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit the Library title")?;
        Self::get_title(pool, tenant_id, id)
            .await?
            .ok_or_else(|| anyhow!("The created Library title could not be loaded"))
    }

    pub async fn update_title(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateTitleRequest,
    ) -> Result<Option<TitleDetail>> {
        let actor_id = person_actor_id(actor)?;
        let fields = normalize_updated_title(request)?;
        validate_replacement_currency(
            pool,
            tenant_id,
            request.replacement_cost_minor,
            request.replacement_currency_id,
        )
        .await?;
        let mut transaction = pool.begin().await.context("Failed to start title update")?;
        let affected = sqlx::query(
            r#"
            UPDATE library_titles
               SET isbn = $3, title = $4, subtitle = $5, authors = $6,
                   publisher = $7, publication_year = $8, edition = $9,
                   language_code = $10, subject = $11,
                   replacement_cost_minor = $12, replacement_currency_id = $13,
                   version = version + 1, updated_at = NOW()
             WHERE tenant_id = $1 AND id = $2 AND status = 'active'
               AND version = $14 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(fields.isbn)
        .bind(fields.title)
        .bind(fields.subtitle)
        .bind(fields.authors)
        .bind(fields.publisher)
        .bind(request.publication_year)
        .bind(fields.edition)
        .bind(fields.language_code)
        .bind(fields.subject)
        .bind(request.replacement_cost_minor)
        .bind(request.replacement_currency_id)
        .bind(request.expected_version)
        .execute(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "Failed to update the Library title"))?
        .rows_affected();
        if affected == 0 {
            transaction.rollback().await.ok();
            return current_or_conflict(pool, tenant_id, id, request.expected_version).await;
        }
        append_event(
            &mut transaction,
            LibraryActivityEvent {
                tenant_id,
                aggregate_id: id,
                aggregate_type: "title",
                event_type: "updated",
                actor_id,
                reason: None,
                metadata: json!({ "version": request.expected_version + 1 }),
            },
        )
        .await?;
        append_domain_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "library.titles.update",
            "library_title",
            id,
            json!({ "version": request.expected_version + 1 }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit the Library title update")?;
        Self::get_title(pool, tenant_id, id).await
    }

    pub async fn retire_title(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<TitleDetail>> {
        let actor_id = person_actor_id(actor)?;
        let active_dependencies = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM library_copies
                 WHERE tenant_id = $1 AND title_id = $2
                   AND status <> 'retired' AND deleted_at IS NULL
                UNION ALL
                SELECT 1 FROM library_holds
                 WHERE tenant_id = $1 AND title_id = $2 AND status IN ('waiting', 'ready')
            )
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_one(pool)
        .await
        .context("Failed to inspect Library title dependencies")?;
        if active_dependencies {
            bail!("Retire every copy and resolve active holds before retiring this title");
        }
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start title retirement")?;
        let affected = sqlx::query(
            r#"
            UPDATE library_titles
               SET status = 'retired', retired_by = $3, retired_at = NOW(),
                   version = version + 1, updated_at = NOW()
             WHERE tenant_id = $1 AND id = $2 AND status = 'active'
               AND version = $4 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(actor_id)
        .bind(expected_version)
        .execute(&mut *transaction)
        .await
        .context("Failed to retire the Library title")?
        .rows_affected();
        if affected == 0 {
            transaction.rollback().await.ok();
            return current_or_conflict(pool, tenant_id, id, expected_version).await;
        }
        record_change(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "title",
            id,
            "retired",
            "library.titles.retire",
            json!({ "version": expected_version + 1 }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit title retirement")?;
        Self::get_title(pool, tenant_id, id).await
    }

    pub async fn list_copies(
        pool: &PgPool,
        tenant_id: Uuid,
        title_id: Uuid,
        page: i64,
        per_page: i64,
        status: Option<&str>,
    ) -> Result<(Vec<CopyResponse>, i64)> {
        validate_copy_status(status)?;
        let page = page.max(1);
        let per_page = per_page.clamp(1, 100);
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, CopyRow>(COPY_SELECT)
            .bind(tenant_id)
            .bind(title_id)
            .bind(status)
            .bind(per_page)
            .bind(offset)
            .fetch_all(pool)
            .await
            .context("Failed to list Library copies")?;
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM library_copies WHERE tenant_id = $1 AND title_id = $2 AND deleted_at IS NULL AND ($3::TEXT IS NULL OR status = $3)",
        ).bind(tenant_id).bind(title_id).bind(status).fetch_one(pool).await.context("Failed to count Library copies")?;
        Ok((rows.into_iter().map(copy_response).collect(), total))
    }

    pub async fn get_copy(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<CopyResponse>> {
        sqlx::query_as::<_, CopyRow>(COPY_BY_ID_SELECT)
            .bind(tenant_id)
            .bind(id)
            .fetch_optional(pool)
            .await
            .context("Failed to load the Library copy")
            .map(|value| value.map(copy_response))
    }

    pub async fn create_copy(
        pool: &PgPool,
        tenant_id: Uuid,
        title_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &CreateCopyRequest,
    ) -> Result<CopyResponse> {
        let actor_id = person_actor_id(actor)?;
        let title = Self::get_title(pool, tenant_id, title_id)
            .await?
            .ok_or_else(|| anyhow!("The Library title was not found"))?;
        if title.summary.status != "active" {
            bail!("Copies can be added only to an active title");
        }
        let barcode = optional(&request.barcode);
        let location = optional(&request.location);
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start copy creation")?;
        let accession = allocate_accession(&mut transaction, tenant_id).await?;
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO library_copies (id, tenant_id, title_id, accession_number, barcode, location, condition, created_by)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
        ).bind(id).bind(tenant_id).bind(title_id).bind(&accession).bind(barcode).bind(location)
          .bind(request.condition.as_str()).bind(actor_id).execute(&mut *transaction).await
          .map_err(|error| database_error(error, "Failed to create the Library copy"))?;
        record_change(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "copy",
            id,
            "created",
            "library.copies.create",
            json!({ "title_id": title_id, "accession_number": accession }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit the Library copy")?;
        Self::get_copy(pool, tenant_id, id)
            .await?
            .ok_or_else(|| anyhow!("The created Library copy could not be loaded"))
    }

    pub async fn update_copy(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        request: &UpdateCopyRequest,
    ) -> Result<Option<CopyResponse>> {
        if !matches!(
            request.status,
            crate::CopyStatus::Available | crate::CopyStatus::Repair
        ) {
            bail!("Use circulation commands to change an on-loan, reserved, lost, or retired copy");
        }
        let actor_id = person_actor_id(actor)?;
        let barcode = optional(&request.barcode);
        let location = optional(&request.location);
        let mut transaction = pool.begin().await.context("Failed to start copy update")?;
        let affected = sqlx::query(
            r#"UPDATE library_copies SET barcode = $3, location = $4, condition = $5, status = $6,
                      version = version + 1, updated_at = NOW()
                WHERE tenant_id = $1 AND id = $2 AND status IN ('available', 'repair')
                  AND version = $7 AND deleted_at IS NULL"#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(barcode)
        .bind(location)
        .bind(request.condition.as_str())
        .bind(request.status.as_str())
        .bind(request.expected_version)
        .execute(&mut *transaction)
        .await
        .map_err(|error| database_error(error, "Failed to update the Library copy"))?
        .rows_affected();
        if affected == 0 {
            transaction.rollback().await.ok();
            return copy_current_or_conflict(pool, tenant_id, id, request.expected_version).await;
        }
        append_event(
            &mut transaction,
            LibraryActivityEvent {
                tenant_id,
                aggregate_id: id,
                aggregate_type: "copy",
                event_type: "updated",
                actor_id,
                reason: None,
                metadata: json!({ "status": request.status.as_str(), "version": request.expected_version + 1 }),
            },
        )
        .await?;
        append_domain_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "library.copies.update",
            "library_copy",
            id,
            json!({ "status": request.status.as_str(), "version": request.expected_version + 1 }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit the Library copy update")?;
        Self::get_copy(pool, tenant_id, id).await
    }

    pub async fn retire_copy(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        expected_version: i32,
    ) -> Result<Option<CopyResponse>> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start copy retirement")?;
        let affected = sqlx::query(
            r#"UPDATE library_copies SET status = 'retired', retired_by = $3, retired_at = NOW(),
                      version = version + 1, updated_at = NOW()
                WHERE tenant_id = $1 AND id = $2 AND status IN ('available', 'repair', 'lost')
                  AND version = $4 AND deleted_at IS NULL"#,
        )
        .bind(tenant_id)
        .bind(id)
        .bind(actor_id)
        .bind(expected_version)
        .execute(&mut *transaction)
        .await
        .context("Failed to retire the Library copy")?
        .rows_affected();
        if affected == 0 {
            transaction.rollback().await.ok();
            return copy_current_or_conflict(pool, tenant_id, id, expected_version).await;
        }
        record_change(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "copy",
            id,
            "retired",
            "library.copies.retire",
            json!({ "version": expected_version + 1 }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit copy retirement")?;
        Self::get_copy(pool, tenant_id, id).await
    }
}

struct NormalizedTitle {
    isbn: Option<String>,
    title: String,
    subtitle: Option<String>,
    authors: Vec<String>,
    publisher: Option<String>,
    edition: Option<String>,
    language_code: String,
    subject: Option<String>,
}

fn normalize_title(request: &CreateTitleRequest) -> Result<NormalizedTitle> {
    normalize_title_fields(
        &request.title,
        request.subtitle.as_deref(),
        &request.authors,
        request.isbn.as_deref(),
        request.publisher.as_deref(),
        request.edition.as_deref(),
        &request.language_code,
        request.subject.as_deref(),
        request.replacement_cost_minor,
        request.replacement_currency_id,
    )
}
fn normalize_updated_title(request: &UpdateTitleRequest) -> Result<NormalizedTitle> {
    normalize_title_fields(
        &request.title,
        request.subtitle.as_deref(),
        &request.authors,
        request.isbn.as_deref(),
        request.publisher.as_deref(),
        request.edition.as_deref(),
        &request.language_code,
        request.subject.as_deref(),
        request.replacement_cost_minor,
        request.replacement_currency_id,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "title fields are normalized together"
)]
fn normalize_title_fields(
    title: &str,
    subtitle: Option<&str>,
    authors: &[String],
    isbn: Option<&str>,
    publisher: Option<&str>,
    edition: Option<&str>,
    language_code: &str,
    subject: Option<&str>,
    replacement_cost_minor: Option<i64>,
    replacement_currency_id: Option<Uuid>,
) -> Result<NormalizedTitle> {
    let title = required(title, "Title")?;
    let authors = normalize_authors(authors)?;
    let isbn = isbn.map(normalize_isbn).transpose()?;
    let language_code = language_code.trim().to_ascii_lowercase();
    if language_code.len() != 3
        || !language_code
            .chars()
            .all(|value| value.is_ascii_lowercase())
    {
        bail!("Language code must contain three lowercase letters");
    }
    if replacement_cost_minor.is_some() != replacement_currency_id.is_some() {
        bail!("Replacement cost and currency must be set together");
    }
    Ok(NormalizedTitle {
        isbn,
        title,
        subtitle: trimmed(subtitle),
        authors,
        publisher: trimmed(publisher),
        edition: trimmed(edition),
        language_code,
        subject: trimmed(subject),
    })
}

async fn validate_replacement_currency(
    pool: &PgPool,
    tenant_id: Uuid,
    amount: Option<i64>,
    currency_id: Option<Uuid>,
) -> Result<()> {
    if amount.is_some() != currency_id.is_some() {
        bail!("Replacement cost and currency must be set together");
    }
    if let Some(id) = currency_id {
        let currency = CurrencyOps::get_by_id(pool, tenant_id, id)
            .await?
            .ok_or_else(|| anyhow!("The replacement currency was not found"))?;
        if currency.status != "active" {
            bail!("The replacement currency must be active");
        }
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "domain and audit records are committed together"
)]
async fn record_change(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    aggregate_type: &str,
    aggregate_id: Uuid,
    event_type: &str,
    action: &str,
    metadata: serde_json::Value,
) -> Result<()> {
    let actor_id = person_actor_id(actor)?;
    append_event(
        transaction,
        LibraryActivityEvent {
            tenant_id,
            aggregate_id,
            aggregate_type,
            event_type,
            actor_id,
            reason: None,
            metadata: metadata.clone(),
        },
    )
    .await?;
    append_domain_audit(
        transaction,
        tenant_id,
        actor,
        request_context,
        action,
        &format!("library_{aggregate_type}"),
        aggregate_id,
        metadata,
    )
    .await
}

async fn current_or_conflict(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    expected: i32,
) -> Result<Option<TitleDetail>> {
    let current = LibraryCatalogueOps::get_title(pool, tenant_id, id).await?;
    if current
        .as_ref()
        .is_some_and(|value| value.summary.version != expected)
    {
        bail!("The Library title changed since it was loaded");
    }
    Ok(current)
}
async fn copy_current_or_conflict(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    expected: i32,
) -> Result<Option<CopyResponse>> {
    let current = LibraryCatalogueOps::get_copy(pool, tenant_id, id).await?;
    if current
        .as_ref()
        .is_some_and(|value| value.version != expected)
    {
        bail!("The Library copy changed since it was loaded");
    }
    Ok(current)
}

fn title_summary(row: TitleSummaryRow) -> TitleSummary {
    TitleSummary {
        id: row.id,
        isbn: row.isbn,
        title: row.title,
        subtitle: row.subtitle,
        authors: row.authors,
        subject: row.subject,
        status: row.status,
        version: row.version,
        copy_count: row.copy_count,
        available_copy_count: row.available_copy_count,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}
fn title_detail(
    row: TitleDetailRow,
    currency: Option<&cp_finance::ledger::CurrencyResponse>,
) -> TitleDetail {
    TitleDetail {
        summary: TitleSummary {
            id: row.id,
            isbn: row.isbn,
            title: row.title,
            subtitle: row.subtitle,
            authors: row.authors,
            subject: row.subject,
            status: row.status,
            version: row.version,
            copy_count: row.copy_count,
            available_copy_count: row.available_copy_count,
            created_at: row.created_at,
            updated_at: row.updated_at,
        },
        publisher: row.publisher,
        publication_year: row.publication_year,
        edition: row.edition,
        language_code: row.language_code,
        replacement_cost_minor: row.replacement_cost_minor,
        replacement_currency_id: row.replacement_currency_id,
        replacement_currency_code: currency.map(|value| value.code.clone()),
        replacement_currency_minor_units: currency.map(|value| value.minor_units),
        created_by: row.created_by,
    }
}
fn copy_response(row: CopyRow) -> CopyResponse {
    CopyResponse {
        id: row.id,
        title_id: row.title_id,
        title: row.title,
        accession_number: row.accession_number,
        barcode: row.barcode,
        location: row.location,
        condition: row.condition,
        status: row.status,
        version: row.version,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn normalize_authors(values: &[String]) -> Result<Vec<String>> {
    let authors = values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if authors.is_empty() || authors.len() > 20 || authors.iter().any(|value| value.len() > 160) {
        bail!("Provide between 1 and 20 author names, each no longer than 160 characters");
    }
    Ok(authors)
}
fn normalize_isbn(value: &str) -> Result<String> {
    let value = value
        .chars()
        .filter(|character| !matches!(character, '-' | ' '))
        .map(|character| character.to_ascii_uppercase())
        .collect::<String>();
    let valid = (value.len() == 10
        && value.chars().enumerate().all(|(index, character)| {
            character.is_ascii_digit() || (index == 9 && character == 'X')
        }))
        || (value.len() == 13 && value.chars().all(|character| character.is_ascii_digit()));
    if !valid {
        bail!("ISBN must contain 10 or 13 digits");
    }
    Ok(value)
}
fn required(value: &str, label: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{label} is required");
    }
    Ok(value.to_string())
}
fn trimmed(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
fn optional(value: &Option<String>) -> Option<String> {
    trimmed(value.as_deref())
}
fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).max(1),
        per_page.unwrap_or(25).clamp(1, 100),
    )
}
fn validate_title_status(value: Option<&str>) -> Result<()> {
    if value.is_some_and(|value| !matches!(value, "active" | "retired")) {
        bail!("The title status filter is invalid");
    }
    Ok(())
}
fn validate_copy_status(value: Option<&str>) -> Result<()> {
    if value.is_some_and(|value| {
        !matches!(
            value,
            "available" | "on_loan" | "reserved" | "lost" | "repair" | "retired"
        )
    }) {
        bail!("The copy status filter is invalid");
    }
    Ok(())
}

fn database_error(error: sqlx::Error, context: &str) -> anyhow::Error {
    if let sqlx::Error::Database(database) = &error
        && database.code().as_deref() == Some("23505")
    {
        return anyhow!(
            "A Library record with that ISBN, accession number, or barcode already exists"
        );
    }
    anyhow!("{context}: {error}")
}

const TITLE_SUMMARY_SELECT: &str = r#"
SELECT title.id, title.isbn, title.title, title.subtitle, title.authors, title.subject,
       title.status, title.version, COUNT(copy.id)::BIGINT AS copy_count,
       COUNT(copy.id) FILTER (WHERE copy.status = 'available')::BIGINT AS available_copy_count,
       title.created_at, title.updated_at
  FROM library_titles AS title
  LEFT JOIN library_copies AS copy ON copy.tenant_id = title.tenant_id AND copy.title_id = title.id AND copy.deleted_at IS NULL
 WHERE title.tenant_id = $1 AND title.deleted_at IS NULL
   AND ($2::TEXT IS NULL OR title.title ILIKE $2 OR title.isbn ILIKE $2 OR title.subject ILIKE $2 OR array_to_string(title.authors, ' ') ILIKE $2)
   AND ($3::TEXT IS NULL OR title.status = $3)
 GROUP BY title.id
 ORDER BY title.title, title.id
 LIMIT $4 OFFSET $5
"#;
const TITLE_DETAIL_SELECT: &str = r#"
SELECT title.id, title.isbn, title.title, title.subtitle, title.authors, title.subject,
       title.status, title.version, COUNT(copy.id)::BIGINT AS copy_count,
       COUNT(copy.id) FILTER (WHERE copy.status = 'available')::BIGINT AS available_copy_count,
       title.created_at, title.updated_at, title.publisher, title.publication_year,
       title.edition, title.language_code, title.replacement_cost_minor,
       title.replacement_currency_id, title.created_by
  FROM library_titles AS title
  LEFT JOIN library_copies AS copy ON copy.tenant_id = title.tenant_id AND copy.title_id = title.id AND copy.deleted_at IS NULL
 WHERE title.tenant_id = $1 AND title.id = $2 AND title.deleted_at IS NULL
 GROUP BY title.id
"#;
const COPY_SELECT: &str = r#"SELECT copy.id, copy.title_id, title.title, copy.accession_number, copy.barcode, copy.location, copy.condition, copy.status, copy.version, copy.created_at, copy.updated_at FROM library_copies AS copy JOIN library_titles AS title ON title.tenant_id = copy.tenant_id AND title.id = copy.title_id WHERE copy.tenant_id = $1 AND copy.title_id = $2 AND copy.deleted_at IS NULL AND title.deleted_at IS NULL AND ($3::TEXT IS NULL OR copy.status = $3) ORDER BY copy.accession_number LIMIT $4 OFFSET $5"#;
const COPY_BY_ID_SELECT: &str = r#"SELECT copy.id, copy.title_id, title.title, copy.accession_number, copy.barcode, copy.location, copy.condition, copy.status, copy.version, copy.created_at, copy.updated_at FROM library_copies AS copy JOIN library_titles AS title ON title.tenant_id = copy.tenant_id AND title.id = copy.title_id WHERE copy.tenant_id = $1 AND copy.id = $2 AND copy.deleted_at IS NULL AND title.deleted_at IS NULL"#;

#[cfg(test)]
mod tests {
    use super::{normalize_authors, normalize_isbn};
    #[test]
    fn isbn_normalization_accepts_printed_separators() {
        assert_eq!(
            normalize_isbn("978-1-4028-9462-6").unwrap(),
            "9781402894626"
        );
    }
    #[test]
    fn author_normalization_rejects_empty_lists() {
        assert!(normalize_authors(&["  ".to_string()]).is_err());
    }
}
