//! Staged CSV/XLSX onboarding for learner billing accounts.
//!
//! `cp-imports` owns bounded parsing and source fingerprinting. Fees owns the
//! destination mapping, learner resolution, duplicate policy, immutable
//! preview, and explicit idempotent commit.

use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, NaiveDate, Utc};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_imports::{ParsedSource, SourceRow, SourceTable};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sqlx::{Acquire, FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

const MODULE_KEY: &str = "fees";
const ENTITY_KEY: &str = "billing_accounts";

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ImportField {
    pub key: &'static str,
    pub label: &'static str,
    pub required: bool,
}

pub const BILLING_ACCOUNT_IMPORT_FIELDS: &[ImportField] = &[
    ImportField {
        key: "learner_number",
        label: "Learner number",
        required: true,
    },
    ImportField {
        key: "account_number",
        label: "Existing account number",
        required: false,
    },
    ImportField {
        key: "opened_on",
        label: "Opened on",
        required: true,
    },
    ImportField {
        key: "status",
        label: "Status",
        required: false,
    },
    ImportField {
        key: "closed_on",
        label: "Closed on",
        required: false,
    },
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportDateFormat {
    YyyyMmDd,
    DdMmYyyy,
    MmDdYyyy,
}

impl ImportDateFormat {
    const fn chrono_pattern(self) -> &'static str {
        match self {
            Self::YyyyMmDd => "%Y-%m-%d",
            Self::DdMmYyyy => "%d/%m/%Y",
            Self::MmDdYyyy => "%m/%d/%Y",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeesImportMapping {
    pub columns: BTreeMap<String, String>,
    pub date_format: Option<ImportDateFormat>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct FeesImportRecord {
    pub id: Uuid,
    pub entity_key: String,
    pub file_name: String,
    pub content_type: String,
    pub source_format: String,
    pub source_size_bytes: i64,
    pub source_row_count: i32,
    pub source_headers: Vec<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub latest_preview_id: Option<Uuid>,
    pub mapping_version: Option<i32>,
    pub ready_rows: Option<i32>,
    pub invalid_rows: Option<i32>,
    pub duplicate_rows: Option<i32>,
    pub created_rows: Option<i32>,
    pub skipped_rows: Option<i32>,
    pub failed_rows: Option<i32>,
    pub committed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct FeesImportListResponse {
    pub imports: Vec<FeesImportRecord>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct FeesImportPreviewRow {
    pub id: Uuid,
    pub row_number: i32,
    pub canonical_data: Value,
    pub outcome: String,
    pub issues: Value,
    pub duplicate_record_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeesImportPreview {
    pub id: Uuid,
    pub import_id: Uuid,
    pub mapping_version: i32,
    pub mapping: FeesImportMapping,
    pub ready_rows: i32,
    pub invalid_rows: i32,
    pub duplicate_rows: i32,
    pub created_at: DateTime<Utc>,
    pub rows: Vec<FeesImportPreviewRow>,
    pub total_rows: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct FeesImportCommit {
    pub id: Uuid,
    pub import_id: Uuid,
    pub preview_id: Uuid,
    pub created_rows: i32,
    pub skipped_rows: i32,
    pub failed_rows: i32,
    pub committed_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CommitImportRequest {
    pub preview_id: Uuid,
}

pub struct NewFeesImport {
    pub file_name: String,
    pub content_type: String,
    pub source_bytes: Vec<u8>,
    pub parsed: ParsedSource,
}

#[derive(Debug, Deserialize)]
pub struct ImportListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PreviewRowsQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, FromRow)]
pub struct RetainedImportSource {
    pub file_name: String,
    pub source_bytes: Vec<u8>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ImportedBillingAccount {
    learner_id: Uuid,
    learner_number: String,
    learner_name: String,
    account_number: Option<String>,
    opened_on: NaiveDate,
    status: String,
    closed_on: Option<NaiveDate>,
}

struct PreparedPreviewRow {
    row_number: i32,
    source_data: Value,
    canonical_data: Value,
    outcome: &'static str,
    issues: Vec<String>,
    duplicate_record_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
struct LearnerReference {
    id: Uuid,
    number: String,
    name: String,
}

pub struct FeesImportOps;

impl FeesImportOps {
    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        input: NewFeesImport,
    ) -> Result<FeesImportRecord> {
        let file_name = safe_file_name(&input.file_name)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start billing import upload")?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO data_imports (
                tenant_id, module_key, entity_key, file_name, content_type,
                source_format, source_sha256, source_bytes, source_size_bytes,
                source_row_count, source_headers, created_by
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(MODULE_KEY)
        .bind(ENTITY_KEY)
        .bind(file_name)
        .bind(&input.content_type)
        .bind(input.parsed.format.as_str())
        .bind(&input.parsed.sha256_hex)
        .bind(&input.source_bytes)
        .bind(input.source_bytes.len() as i64)
        .bind(input.parsed.table.rows.len() as i32)
        .bind(&input.parsed.table.headers)
        .bind(actor.user_id())
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to retain billing import source")?;
        append_import_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "fees.imports.upload",
            id,
            json!({ "target": ENTITY_KEY, "row_count": input.parsed.table.rows.len() }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to save billing import upload")?;
        Self::get(pool, tenant_id, id)
            .await?
            .context("Created billing import could not be reloaded")
    }

    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
    ) -> Result<(Vec<FeesImportRecord>, i64)> {
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, FeesImportRecord>(&format!(
            "{} WHERE import.tenant_id = $1 AND import.module_key = $2 AND import.entity_key = $3 ORDER BY import.created_at DESC LIMIT $4 OFFSET $5",
            import_select()
        ))
        .bind(tenant_id)
        .bind(MODULE_KEY)
        .bind(ENTITY_KEY)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list billing imports")?;
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM data_imports WHERE tenant_id = $1 AND module_key = $2 AND entity_key = $3",
        )
        .bind(tenant_id)
        .bind(MODULE_KEY)
        .bind(ENTITY_KEY)
        .fetch_one(pool)
        .await
        .context("Failed to count billing imports")?;
        Ok((rows, total))
    }

    pub async fn get(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<Option<FeesImportRecord>> {
        sqlx::query_as::<_, FeesImportRecord>(&format!(
            "{} WHERE import.tenant_id = $1 AND import.module_key = $2 AND import.entity_key = $3 AND import.id = $4",
            import_select()
        ))
        .bind(tenant_id)
        .bind(MODULE_KEY)
        .bind(ENTITY_KEY)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load billing import")
    }

    pub async fn retained_source(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<RetainedImportSource>> {
        sqlx::query_as::<_, RetainedImportSource>(
            r#"
            SELECT file_name, source_bytes, status
            FROM data_imports
            WHERE tenant_id = $1 AND module_key = $2 AND entity_key = $3 AND id = $4
            "#,
        )
        .bind(tenant_id)
        .bind(MODULE_KEY)
        .bind(ENTITY_KEY)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load retained billing import source")
    }

    pub async fn create_preview(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        import_id: Uuid,
        mapping: FeesImportMapping,
        table: &SourceTable,
    ) -> Result<FeesImportPreview> {
        let source = Self::retained_source(pool, tenant_id, import_id)
            .await?
            .context("Billing import not found")?;
        if source.status == "committed" {
            bail!("A committed import cannot be remapped.");
        }
        validate_mapping(&mapping, &table.headers)?;
        let rows = prepare_rows(pool, tenant_id, &mapping, table).await?;
        let ready_rows = count_outcome(&rows, "ready");
        let invalid_rows = count_outcome(&rows, "invalid");
        let duplicate_rows = count_outcome(&rows, "duplicate");
        let mapping_json = serde_json::to_value(&mapping).context("Failed to encode mapping")?;

        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start billing import preview")?;
        sqlx::query(
            "SELECT id FROM data_imports WHERE tenant_id = $1 AND module_key = $2 AND entity_key = $3 AND id = $4 AND status <> 'committed' FOR UPDATE",
        )
        .bind(tenant_id)
        .bind(MODULE_KEY)
        .bind(ENTITY_KEY)
        .bind(import_id)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to lock billing import")?
        .context("Billing import is no longer available for preview")?;
        let version = sqlx::query_scalar::<_, i32>(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM data_import_mappings WHERE tenant_id = $1 AND import_id = $2",
        )
        .bind(tenant_id)
        .bind(import_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to allocate billing mapping version")?;
        let mapping_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO data_import_mappings (tenant_id, import_id, version, mapping, created_by) VALUES ($1, $2, $3, $4, $5) RETURNING id",
        )
        .bind(tenant_id)
        .bind(import_id)
        .bind(version)
        .bind(&mapping_json)
        .bind(actor.user_id())
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to save billing import mapping")?;
        let preview_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO data_import_previews (
                tenant_id, import_id, mapping_id, ready_rows, invalid_rows,
                duplicate_rows, created_by
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(import_id)
        .bind(mapping_id)
        .bind(ready_rows)
        .bind(invalid_rows)
        .bind(duplicate_rows)
        .bind(actor.user_id())
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to save billing import preview")?;
        for row in &rows {
            sqlx::query(
                r#"
                INSERT INTO data_import_preview_rows (
                    tenant_id, preview_id, row_number, source_data,
                    canonical_data, outcome, issues, duplicate_record_id
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(tenant_id)
            .bind(preview_id)
            .bind(row.row_number)
            .bind(&row.source_data)
            .bind(&row.canonical_data)
            .bind(row.outcome)
            .bind(json!(row.issues))
            .bind(row.duplicate_record_id)
            .execute(&mut *transaction)
            .await
            .context("Failed to save billing import preview row")?;
        }
        sqlx::query(
            "UPDATE data_imports SET status = 'preview_ready' WHERE tenant_id = $1 AND module_key = $2 AND entity_key = $3 AND id = $4",
        )
        .bind(tenant_id)
        .bind(MODULE_KEY)
        .bind(ENTITY_KEY)
        .bind(import_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to update billing import state")?;
        append_import_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "fees.imports.preview",
            import_id,
            json!({
                "preview_id": preview_id,
                "mapping_version": version,
                "ready_rows": ready_rows,
                "invalid_rows": invalid_rows,
                "duplicate_rows": duplicate_rows
            }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to save billing import preview")?;
        Self::preview(pool, tenant_id, import_id, 1, 100)
            .await?
            .context("Created billing import preview could not be reloaded")
    }

    pub async fn preview(
        pool: &PgPool,
        tenant_id: Uuid,
        import_id: Uuid,
        page: i64,
        per_page: i64,
    ) -> Result<Option<FeesImportPreview>> {
        #[derive(FromRow)]
        struct Header {
            id: Uuid,
            import_id: Uuid,
            version: i32,
            mapping: Value,
            ready_rows: i32,
            invalid_rows: i32,
            duplicate_rows: i32,
            created_at: DateTime<Utc>,
        }
        let header = sqlx::query_as::<_, Header>(
            r#"
            SELECT preview.id, preview.import_id, mapping.version, mapping.mapping,
                   preview.ready_rows, preview.invalid_rows, preview.duplicate_rows,
                   preview.created_at
            FROM data_import_previews AS preview
            JOIN data_import_mappings AS mapping
              ON mapping.id = preview.mapping_id AND mapping.tenant_id = preview.tenant_id
            JOIN data_imports AS import
              ON import.id = preview.import_id AND import.tenant_id = preview.tenant_id
            WHERE preview.tenant_id = $1 AND preview.import_id = $2
              AND import.module_key = $3 AND import.entity_key = $4
            ORDER BY mapping.version DESC
            LIMIT 1
            "#,
        )
        .bind(tenant_id)
        .bind(import_id)
        .bind(MODULE_KEY)
        .bind(ENTITY_KEY)
        .fetch_optional(pool)
        .await
        .context("Failed to load billing import preview")?;
        let Some(header) = header else {
            return Ok(None);
        };
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, FeesImportPreviewRow>(
            r#"
            SELECT id, row_number, canonical_data, outcome, issues, duplicate_record_id
            FROM data_import_preview_rows
            WHERE tenant_id = $1 AND preview_id = $2
            ORDER BY row_number
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(tenant_id)
        .bind(header.id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to load billing import preview rows")?;
        let total_rows = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM data_import_preview_rows WHERE tenant_id = $1 AND preview_id = $2",
        )
        .bind(tenant_id)
        .bind(header.id)
        .fetch_one(pool)
        .await
        .context("Failed to count billing import preview rows")?;
        let mapping = serde_json::from_value(header.mapping)
            .context("Stored billing import mapping is invalid")?;
        Ok(Some(FeesImportPreview {
            id: header.id,
            import_id: header.import_id,
            mapping_version: header.version,
            mapping,
            ready_rows: header.ready_rows,
            invalid_rows: header.invalid_rows,
            duplicate_rows: header.duplicate_rows,
            created_at: header.created_at,
            rows,
            total_rows,
        }))
    }

    pub async fn commit(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        import_id: Uuid,
        preview_id: Uuid,
    ) -> Result<FeesImportCommit> {
        let actor_id = actor
            .user_id()
            .context("Billing imports require an authenticated account")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start billing import commit")?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("{tenant_id}:fees-billing-import"))
            .execute(&mut *transaction)
            .await
            .context("Failed to serialize billing import commit")?;
        if let Some(existing) =
            load_commit(&mut transaction, tenant_id, import_id, preview_id).await?
        {
            transaction
                .commit()
                .await
                .context("Failed to finish billing import lookup")?;
            return Ok(existing);
        }
        sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT import.id
            FROM data_imports AS import
            JOIN data_import_previews AS preview
              ON preview.import_id = import.id AND preview.tenant_id = import.tenant_id
            WHERE import.tenant_id = $1 AND import.module_key = $2
              AND import.entity_key = $3 AND import.id = $4 AND preview.id = $5
              AND import.status = 'preview_ready'
              AND preview.mapping_id = (
                  SELECT mapping.id FROM data_import_mappings AS mapping
                  WHERE mapping.tenant_id = import.tenant_id
                    AND mapping.import_id = import.id
                  ORDER BY mapping.version DESC LIMIT 1
              )
            FOR UPDATE OF import
            "#,
        )
        .bind(tenant_id)
        .bind(MODULE_KEY)
        .bind(ENTITY_KEY)
        .bind(import_id)
        .bind(preview_id)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to lock billing import preview")?
        .context("The selected billing preview is not available for commit")?;
        let rows = sqlx::query_as::<_, FeesImportPreviewRow>(
            r#"
            SELECT id, row_number, canonical_data, outcome, issues, duplicate_record_id
            FROM data_import_preview_rows
            WHERE tenant_id = $1 AND preview_id = $2
            ORDER BY row_number
            "#,
        )
        .bind(tenant_id)
        .bind(preview_id)
        .fetch_all(&mut *transaction)
        .await
        .context("Failed to load rows for billing import commit")?;
        let commit_id = Uuid::new_v4();
        let mut created_rows = 0;
        let mut skipped_rows = 0;
        let mut failed_rows = 0;
        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            let result = match row.outcome.as_str() {
                "invalid" => RowCommitResult::rejected(row.id, row.issues),
                "duplicate" => RowCommitResult::skipped(row.id, row.duplicate_record_id),
                "ready" => {
                    let mut savepoint = transaction
                        .begin()
                        .await
                        .context("Failed to start billing import row savepoint")?;
                    match commit_ready_row(&mut savepoint, tenant_id, actor_id, import_id, &row)
                        .await
                    {
                        Ok(Some(record_id)) => {
                            savepoint
                                .commit()
                                .await
                                .context("Failed to save imported billing account")?;
                            RowCommitResult::created(row.id, record_id)
                        }
                        Ok(None) => {
                            savepoint
                                .commit()
                                .await
                                .context("Failed to save duplicate billing result")?;
                            RowCommitResult::skipped(row.id, None)
                        }
                        Err(error) => {
                            savepoint
                                .rollback()
                                .await
                                .context("Failed to recover rejected billing row")?;
                            RowCommitResult::failed(row.id, safe_commit_issue(&error))
                        }
                    }
                }
                _ => RowCommitResult::failed(row.id, "Stored preview outcome is invalid."),
            };
            match result.outcome {
                "created" => created_rows += 1,
                "skipped_duplicate" | "rejected_validation" => skipped_rows += 1,
                "failed" => failed_rows += 1,
                _ => failed_rows += 1,
            }
            results.push(result);
        }
        let committed_at = Utc::now();
        sqlx::query(
            r#"
            INSERT INTO data_import_commits (
                id, tenant_id, import_id, preview_id, created_rows,
                skipped_rows, failed_rows, requested_by, committed_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(commit_id)
        .bind(tenant_id)
        .bind(import_id)
        .bind(preview_id)
        .bind(created_rows)
        .bind(skipped_rows)
        .bind(failed_rows)
        .bind(actor_id)
        .bind(committed_at)
        .execute(&mut *transaction)
        .await
        .context("Failed to save billing import commit")?;
        for result in results {
            sqlx::query(
                r#"
                INSERT INTO data_import_row_results (
                    tenant_id, commit_id, preview_row_id, outcome, record_id, issues
                ) VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(tenant_id)
            .bind(commit_id)
            .bind(result.preview_row_id)
            .bind(result.outcome)
            .bind(result.record_id)
            .bind(result.issues)
            .execute(&mut *transaction)
            .await
            .context("Failed to save billing import row result")?;
        }
        sqlx::query(
            "UPDATE data_imports SET status = 'committed' WHERE tenant_id = $1 AND module_key = $2 AND entity_key = $3 AND id = $4",
        )
        .bind(tenant_id)
        .bind(MODULE_KEY)
        .bind(ENTITY_KEY)
        .bind(import_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to mark billing import committed")?;
        append_import_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "fees.imports.commit",
            import_id,
            json!({
                "preview_id": preview_id,
                "created_rows": created_rows,
                "skipped_rows": skipped_rows,
                "failed_rows": failed_rows
            }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit billing import")?;
        Ok(FeesImportCommit {
            id: commit_id,
            import_id,
            preview_id,
            created_rows,
            skipped_rows,
            failed_rows,
            committed_at,
        })
    }
}

fn import_select() -> &'static str {
    r#"
    SELECT import.id, import.entity_key, import.file_name, import.content_type,
           import.source_format, import.source_size_bytes, import.source_row_count,
           import.source_headers, import.status, import.created_at,
           latest_preview.id AS latest_preview_id,
           latest_preview.version AS mapping_version,
           latest_preview.ready_rows, latest_preview.invalid_rows,
           latest_preview.duplicate_rows, commit.created_rows,
           commit.skipped_rows, commit.failed_rows, commit.committed_at
    FROM data_imports AS import
    LEFT JOIN LATERAL (
        SELECT preview.id, mapping.version, preview.ready_rows,
               preview.invalid_rows, preview.duplicate_rows
        FROM data_import_previews AS preview
        JOIN data_import_mappings AS mapping
          ON mapping.id = preview.mapping_id AND mapping.tenant_id = preview.tenant_id
        WHERE preview.tenant_id = import.tenant_id AND preview.import_id = import.id
        ORDER BY mapping.version DESC LIMIT 1
    ) AS latest_preview ON TRUE
    LEFT JOIN data_import_commits AS commit
      ON commit.tenant_id = import.tenant_id
     AND commit.preview_id = latest_preview.id
    "#
}

fn validate_mapping(mapping: &FeesImportMapping, source_headers: &[String]) -> Result<()> {
    let allowed = BILLING_ACCOUNT_IMPORT_FIELDS
        .iter()
        .map(|field| field.key)
        .collect::<HashSet<_>>();
    if mapping
        .columns
        .keys()
        .any(|key| !allowed.contains(key.as_str()))
    {
        bail!("The mapping contains an unsupported billing field.");
    }
    for field in BILLING_ACCOUNT_IMPORT_FIELDS
        .iter()
        .filter(|field| field.required)
    {
        if mapping
            .columns
            .get(field.key)
            .is_none_or(|value| value.trim().is_empty())
        {
            bail!("Map the required {} field.", field.label.to_lowercase());
        }
    }
    if mapping.date_format.is_none() {
        bail!("Choose the date format used by the billing source.");
    }
    let headers = source_headers.iter().collect::<HashSet<_>>();
    if mapping
        .columns
        .values()
        .any(|header| !headers.contains(header))
    {
        bail!("A mapped source column is not present in this file.");
    }
    let mut unique = HashSet::new();
    if mapping
        .columns
        .values()
        .any(|header| !unique.insert(header))
    {
        bail!("Each source column can map to only one Fees field.");
    }
    Ok(())
}

async fn prepare_rows(
    pool: &PgPool,
    tenant_id: Uuid,
    mapping: &FeesImportMapping,
    table: &SourceTable,
) -> Result<Vec<PreparedPreviewRow>> {
    let learner_rows = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT id, learner_number, display_name FROM learners WHERE tenant_id = $1 AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .context("Failed to load learner references for billing import")?;
    let learners = learner_rows
        .into_iter()
        .map(|(id, number, name)| {
            (
                number.trim().to_lowercase(),
                LearnerReference { id, number, name },
            )
        })
        .collect::<HashMap<_, _>>();
    let candidates = table
        .rows
        .iter()
        .map(|row| {
            let source_data = source_json(table, row);
            let (value, issues) = map_billing_account(table, row, mapping, &learners);
            (row, source_data, value, issues)
        })
        .collect::<Vec<_>>();
    let learner_ids = candidates
        .iter()
        .filter_map(|(_, _, value, _)| value.as_ref().map(|value| value.learner_id))
        .collect::<Vec<_>>();
    let account_numbers = candidates
        .iter()
        .filter_map(|(_, _, value, _)| value.as_ref()?.account_number.as_ref())
        .map(|value| value.to_lowercase())
        .collect::<Vec<_>>();
    let existing_rows = sqlx::query_as::<_, (Uuid, Uuid, String)>(
        r#"
        SELECT id, learner_id, LOWER(account_number)
        FROM fees_billing_accounts
        WHERE tenant_id = $1 AND deleted_at IS NULL
          AND (learner_id = ANY($2) OR LOWER(account_number) = ANY($3))
        "#,
    )
    .bind(tenant_id)
    .bind(&learner_ids)
    .bind(&account_numbers)
    .fetch_all(pool)
    .await
    .context("Failed to check billing import duplicates")?;
    let mut existing = HashMap::new();
    for (id, learner_id, account_number) in existing_rows {
        existing.insert(format!("learner:{learner_id}"), id);
        existing.insert(format!("account:{account_number}"), id);
    }
    let mut seen = HashSet::new();
    Ok(candidates
        .into_iter()
        .map(|(row, source_data, value, mut issues)| {
            let keys = value.as_ref().map(dedupe_keys).unwrap_or_default();
            let duplicate_record_id = keys.iter().find_map(|key| existing.get(key).copied());
            let mut repeated = false;
            if issues.is_empty() {
                for key in &keys {
                    repeated |= !seen.insert(key.clone());
                }
            }
            let outcome = if !issues.is_empty() {
                "invalid"
            } else if duplicate_record_id.is_some() || repeated {
                issues.push(if duplicate_record_id.is_some() {
                    "A billing account already exists for this learner or account number."
                        .to_string()
                } else {
                    "This row duplicates an earlier row in the file.".to_string()
                });
                "duplicate"
            } else {
                "ready"
            };
            PreparedPreviewRow {
                row_number: row.row_number as i32,
                source_data,
                canonical_data: value.map_or_else(|| json!({}), |value| json!(value)),
                outcome,
                issues,
                duplicate_record_id,
            }
        })
        .collect())
}

fn map_billing_account(
    table: &SourceTable,
    row: &SourceRow,
    mapping: &FeesImportMapping,
    learners: &HashMap<String, LearnerReference>,
) -> (Option<ImportedBillingAccount>, Vec<String>) {
    let mut issues = Vec::new();
    let learner_number = required_mapped(
        table,
        row,
        mapping,
        "learner_number",
        "Learner number",
        80,
        &mut issues,
    );
    let learner = learner_number
        .as_ref()
        .and_then(|value| learners.get(&value.to_lowercase()).cloned());
    if learner_number.is_some() && learner.is_none() {
        issues.push("The learner number was not found in SIS.".to_string());
    }
    let account_number = optional_mapped(
        table,
        row,
        mapping,
        "account_number",
        "Account number",
        80,
        &mut issues,
    );
    let opened_on = parse_required_date(
        mapped(table, row, mapping, "opened_on"),
        mapping.date_format,
        "Opened on",
        &mut issues,
    );
    let status = optional_mapped(table, row, mapping, "status", "Status", 30, &mut issues)
        .map(|value| normalize_status(&value))
        .unwrap_or_else(|| "active".to_string());
    if !matches!(status.as_str(), "active" | "on_hold" | "closed") {
        issues.push("Status must be active, on hold, or closed.".to_string());
    }
    let closed_on = parse_optional_date(
        mapped(table, row, mapping, "closed_on"),
        mapping.date_format,
        "Closed on",
        &mut issues,
    );
    if status == "closed" && closed_on.is_none() {
        issues.push("Closed accounts require a closed-on date.".to_string());
    }
    if status != "closed" && closed_on.is_some() {
        issues.push("Only closed accounts may have a closed-on date.".to_string());
    }
    if let Some(opened_on) = opened_on
        && opened_on > Utc::now().date_naive()
    {
        issues.push("Opened on cannot be in the future.".to_string());
    }
    if let (Some(opened_on), Some(closed_on)) = (opened_on, closed_on)
        && closed_on < opened_on
    {
        issues.push("Closed on cannot be before opened on.".to_string());
    }
    if !issues.is_empty() {
        return (None, issues);
    }
    match (learner, opened_on) {
        (Some(learner), Some(opened_on)) => (
            Some(ImportedBillingAccount {
                learner_id: learner.id,
                learner_number: learner.number,
                learner_name: learner.name,
                account_number,
                opened_on,
                status,
                closed_on,
            }),
            issues,
        ),
        _ => (None, issues),
    }
}

fn mapped<'a>(
    table: &'a SourceTable,
    row: &'a SourceRow,
    mapping: &FeesImportMapping,
    key: &str,
) -> Option<&'a str> {
    mapping
        .columns
        .get(key)
        .and_then(|header| table.value(row, header))
        .filter(|value| !value.is_empty())
}

fn required_mapped(
    table: &SourceTable,
    row: &SourceRow,
    mapping: &FeesImportMapping,
    key: &str,
    label: &str,
    maximum: usize,
    issues: &mut Vec<String>,
) -> Option<String> {
    let value = mapped(table, row, mapping, key).map(ToOwned::to_owned);
    if value.is_none() {
        issues.push(format!("{label} is required."));
    }
    if value.as_ref().is_some_and(|value| value.len() > maximum) {
        issues.push(format!("{label} is too long."));
    }
    value
}

fn optional_mapped(
    table: &SourceTable,
    row: &SourceRow,
    mapping: &FeesImportMapping,
    key: &str,
    label: &str,
    maximum: usize,
    issues: &mut Vec<String>,
) -> Option<String> {
    let value = mapped(table, row, mapping, key).map(ToOwned::to_owned);
    if value.as_ref().is_some_and(|value| value.len() > maximum) {
        issues.push(format!("{label} is too long."));
    }
    value
}

fn parse_required_date(
    value: Option<&str>,
    format: Option<ImportDateFormat>,
    label: &str,
    issues: &mut Vec<String>,
) -> Option<NaiveDate> {
    let Some(value) = value else {
        issues.push(format!("{label} is required."));
        return None;
    };
    parse_date(value, format, label, issues)
}

fn parse_optional_date(
    value: Option<&str>,
    format: Option<ImportDateFormat>,
    label: &str,
    issues: &mut Vec<String>,
) -> Option<NaiveDate> {
    value.and_then(|value| parse_date(value, format, label, issues))
}

fn parse_date(
    value: &str,
    format: Option<ImportDateFormat>,
    label: &str,
    issues: &mut Vec<String>,
) -> Option<NaiveDate> {
    let parsed =
        format.and_then(|format| NaiveDate::parse_from_str(value, format.chrono_pattern()).ok());
    if parsed.is_none() {
        issues.push(format!("{label} does not match the selected date format."));
    }
    parsed
}

fn normalize_status(value: &str) -> String {
    value.trim().to_lowercase().replace([' ', '-'], "_")
}

fn source_json(table: &SourceTable, row: &SourceRow) -> Value {
    Value::Object(
        table
            .headers
            .iter()
            .enumerate()
            .map(|(index, header)| {
                (
                    header.clone(),
                    Value::String(row.values.get(index).cloned().unwrap_or_default()),
                )
            })
            .collect::<Map<_, _>>(),
    )
}

fn dedupe_keys(value: &ImportedBillingAccount) -> Vec<String> {
    let mut keys = vec![format!("learner:{}", value.learner_id)];
    if let Some(account_number) = &value.account_number {
        keys.push(format!("account:{}", account_number.to_lowercase()));
    }
    keys
}

fn count_outcome(rows: &[PreparedPreviewRow], outcome: &str) -> i32 {
    rows.iter().filter(|row| row.outcome == outcome).count() as i32
}

struct RowCommitResult {
    preview_row_id: Uuid,
    outcome: &'static str,
    record_id: Option<Uuid>,
    issues: Value,
}

impl RowCommitResult {
    fn created(preview_row_id: Uuid, record_id: Uuid) -> Self {
        Self {
            preview_row_id,
            outcome: "created",
            record_id: Some(record_id),
            issues: json!([]),
        }
    }

    fn skipped(preview_row_id: Uuid, record_id: Option<Uuid>) -> Self {
        Self {
            preview_row_id,
            outcome: "skipped_duplicate",
            record_id,
            issues: json!(["A matching billing account exists."]),
        }
    }

    fn rejected(preview_row_id: Uuid, issues: Value) -> Self {
        Self {
            preview_row_id,
            outcome: "rejected_validation",
            record_id: None,
            issues,
        }
    }

    fn failed(preview_row_id: Uuid, issue: &str) -> Self {
        Self {
            preview_row_id,
            outcome: "failed",
            record_id: None,
            issues: json!([issue]),
        }
    }
}

async fn commit_ready_row(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor_id: Uuid,
    import_id: Uuid,
    row: &FeesImportPreviewRow,
) -> Result<Option<Uuid>> {
    let value: ImportedBillingAccount = serde_json::from_value(row.canonical_data.clone())
        .context("Stored billing preview is invalid")?;
    if sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM fees_billing_accounts
            WHERE tenant_id = $1 AND deleted_at IS NULL
              AND (learner_id = $2 OR ($3::TEXT IS NOT NULL AND LOWER(account_number) = LOWER($3)))
        )
        "#,
    )
    .bind(tenant_id)
    .bind(value.learner_id)
    .bind(&value.account_number)
    .fetch_one(&mut **transaction)
    .await
    .context("Failed to recheck billing import duplicate")?
    {
        return Ok(None);
    }
    let account_number = match value.account_number {
        Some(number) => {
            if let Some(sequence) = imported_sequence(&number) {
                sqlx::query(
                    r#"
                    INSERT INTO fees_billing_account_sequences (tenant_id, last_number)
                    VALUES ($1, $2)
                    ON CONFLICT (tenant_id) DO UPDATE
                       SET last_number = GREATEST(fees_billing_account_sequences.last_number, EXCLUDED.last_number)
                    "#,
                )
                .bind(tenant_id)
                .bind(sequence)
                .execute(&mut **transaction)
                .await
                .context("Failed to align imported billing account sequence")?;
            }
            number
        }
        None => {
            let sequence = sqlx::query_scalar::<_, i64>(
                r#"
                INSERT INTO fees_billing_account_sequences (tenant_id, last_number)
                VALUES ($1, 1)
                ON CONFLICT (tenant_id) DO UPDATE
                   SET last_number = fees_billing_account_sequences.last_number + 1
                RETURNING last_number
                "#,
            )
            .bind(tenant_id)
            .fetch_one(&mut **transaction)
            .await
            .context("Failed to allocate imported billing account number")?;
            format!("BIL-{sequence:06}")
        }
    };
    let closed_at = value
        .closed_on
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .map(|value| value.and_utc());
    let idempotency_key = format!("fees-import:{import_id}:{}", row.row_number);
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO fees_billing_accounts (
            tenant_id, learner_id, account_number, opened_on, status,
            idempotency_key, created_by, closed_by, closed_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT DO NOTHING
        RETURNING id
        "#,
    )
    .bind(tenant_id)
    .bind(value.learner_id)
    .bind(account_number)
    .bind(value.opened_on)
    .bind(&value.status)
    .bind(idempotency_key)
    .bind(actor_id)
    .bind((value.status == "closed").then_some(actor_id))
    .bind(closed_at)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to create imported billing account")
}

fn imported_sequence(account_number: &str) -> Option<i64> {
    let value = account_number.strip_prefix("BIL-")?;
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse::<i64>().ok().filter(|value| *value >= 0)
}

async fn load_commit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    import_id: Uuid,
    preview_id: Uuid,
) -> Result<Option<FeesImportCommit>> {
    sqlx::query_as::<_, FeesImportCommit>(
        r#"
        SELECT id, import_id, preview_id, created_rows, skipped_rows,
               failed_rows, committed_at
        FROM data_import_commits
        WHERE tenant_id = $1 AND import_id = $2 AND preview_id = $3
        "#,
    )
    .bind(tenant_id)
    .bind(import_id)
    .bind(preview_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to load billing import commit")
}

async fn append_import_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    action: &str,
    import_id: Uuid,
    metadata: Value,
) -> Result<()> {
    let metadata = metadata.as_object().cloned().unwrap_or_default();
    append_audit(
        &mut **transaction,
        &NewAuditEvent::new(
            tenant_id,
            actor,
            action,
            AuditOutcome::Succeeded,
            request_context,
        )
        .with_target(AuditTarget::new("data_import", import_id.to_string()))
        .with_redacted_metadata(metadata),
    )
    .await
    .context("Failed to audit billing import operation")?;
    Ok(())
}

fn safe_file_name(value: &str) -> Result<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 255 || trimmed.contains(['/', '\\']) {
        bail!("The import filename is invalid.");
    }
    Ok(trimmed)
}

fn safe_commit_issue(error: &anyhow::Error) -> &'static str {
    if error.root_cause().downcast_ref::<sqlx::Error>().is_some() {
        "The billing-account row could not be saved."
    } else {
        "The stored billing preview row is invalid."
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use cp_imports::parse_source;
    use uuid::Uuid;

    use super::{
        FeesImportMapping, ImportDateFormat, LearnerReference, imported_sequence,
        map_billing_account, validate_mapping,
    };

    #[test]
    fn billing_mapping_requires_learner_date_and_explicit_format() {
        let table = parse_source(
            "billing.csv",
            b"learner,opened,status\nSTU-1,2026-01-02,active\n",
        )
        .unwrap_or_else(|error| panic!("fixture should parse: {error}"))
        .table;
        let mapping = FeesImportMapping {
            columns: BTreeMap::from([
                ("learner_number".to_string(), "learner".to_string()),
                ("opened_on".to_string(), "opened".to_string()),
                ("status".to_string(), "status".to_string()),
            ]),
            date_format: None,
        };
        assert!(validate_mapping(&mapping, &table.headers).is_err());
        let mapping = FeesImportMapping {
            date_format: Some(ImportDateFormat::YyyyMmDd),
            ..mapping
        };
        assert!(validate_mapping(&mapping, &table.headers).is_ok());
        let learners = HashMap::from([(
            "stu-1".to_string(),
            LearnerReference {
                id: Uuid::new_v4(),
                number: "STU-1".to_string(),
                name: "Ada Lovelace".to_string(),
            },
        )]);
        let (value, issues) = map_billing_account(&table, &table.rows[0], &mapping, &learners);
        assert!(issues.is_empty());
        assert_eq!(
            value.map(|account| account.learner_number),
            Some("STU-1".to_string())
        );
    }

    #[test]
    fn imported_generated_numbers_advance_the_managed_sequence_only() {
        assert_eq!(imported_sequence("BIL-000125"), Some(125));
        assert_eq!(imported_sequence("LEGACY-125"), None);
    }
}
