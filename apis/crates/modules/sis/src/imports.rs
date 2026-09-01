//! Owns staged SIS learner and guardian import workflows.
//!
//! Source parsing is destination-neutral in `cp-imports`; this module owns SIS
//! field mapping, deduplication, immutable previews, and explicit commit rules.

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

use crate::ops::SisCampusReadScope;
use validator::ValidateEmail;

use crate::numbering::align_imported_learner_number;

/// SIS record families supported by the first import adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SisImportTarget {
    Learners,
    Guardians,
}

impl SisImportTarget {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Learners => "learners",
            Self::Guardians => "guardians",
        }
    }

    /// Parses the stable entity key accepted by HTTP and persistence.
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "learners" => Ok(Self::Learners),
            "guardians" => Ok(Self::Guardians),
            _ => bail!("That SIS import target is not supported."),
        }
    }

    #[must_use]
    pub const fn fields(self) -> &'static [ImportField] {
        match self {
            Self::Learners => LEARNER_FIELDS,
            Self::Guardians => GUARDIAN_FIELDS,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ImportField {
    pub key: &'static str,
    pub label: &'static str,
    pub required: bool,
}

const LEARNER_FIELDS: &[ImportField] = &[
    ImportField {
        key: "learner_number",
        label: "Learner number",
        required: true,
    },
    ImportField {
        key: "display_name",
        label: "Display name",
        required: true,
    },
    ImportField {
        key: "first_names",
        label: "First names",
        required: false,
    },
    ImportField {
        key: "surname",
        label: "Surname",
        required: false,
    },
    ImportField {
        key: "date_of_birth",
        label: "Date of birth",
        required: true,
    },
    ImportField {
        key: "email",
        label: "Email",
        required: false,
    },
    ImportField {
        key: "phone",
        label: "Phone",
        required: false,
    },
    ImportField {
        key: "status",
        label: "Status",
        required: false,
    },
];

const GUARDIAN_FIELDS: &[ImportField] = &[
    ImportField {
        key: "display_name",
        label: "Display name",
        required: true,
    },
    ImportField {
        key: "first_names",
        label: "First names",
        required: false,
    },
    ImportField {
        key: "surname",
        label: "Surname",
        required: false,
    },
    ImportField {
        key: "email",
        label: "Email",
        required: false,
    },
    ImportField {
        key: "phone",
        label: "Phone",
        required: false,
    },
    ImportField {
        key: "status",
        label: "Status",
        required: false,
    },
];

/// Explicit date interpretation saved with every learner mapping version.
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

/// Maps canonical SIS field keys to exact source headers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SisImportMapping {
    pub columns: BTreeMap<String, String>,
    pub date_format: Option<ImportDateFormat>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct SisImportRecord {
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

impl SisImportRecord {
    pub fn target(&self) -> Result<SisImportTarget> {
        SisImportTarget::parse(&self.entity_key)
    }
}

#[derive(Debug, Serialize)]
pub struct SisImportListResponse {
    pub imports: Vec<SisImportRecord>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct SisImportPreviewRow {
    pub id: Uuid,
    pub row_number: i32,
    pub canonical_data: Value,
    pub outcome: String,
    pub issues: Value,
    pub duplicate_record_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SisImportPreview {
    pub id: Uuid,
    pub import_id: Uuid,
    pub mapping_version: i32,
    pub mapping: SisImportMapping,
    pub ready_rows: i32,
    pub invalid_rows: i32,
    pub duplicate_rows: i32,
    pub created_at: DateTime<Utc>,
    pub rows: Vec<SisImportPreviewRow>,
    pub total_rows: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct SisImportCommit {
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

/// Parsed upload material accepted by the SIS import service boundary.
pub struct NewSisImport {
    pub target: SisImportTarget,
    pub file_name: String,
    pub content_type: String,
    pub source_bytes: Vec<u8>,
    pub parsed: ParsedSource,
}

#[derive(Debug, Deserialize)]
pub struct ImportListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub target: Option<SisImportTarget>,
}

#[derive(Debug, Deserialize)]
pub struct PreviewRowsQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, FromRow)]
pub struct RetainedImportSource {
    pub id: Uuid,
    pub entity_key: String,
    pub file_name: String,
    pub source_bytes: Vec<u8>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ImportedLearner {
    learner_number: String,
    display_name: String,
    first_names: Option<String>,
    surname: Option<String>,
    date_of_birth: NaiveDate,
    email: Option<String>,
    phone: Option<String>,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ImportedGuardian {
    display_name: String,
    first_names: Option<String>,
    surname: Option<String>,
    email: Option<String>,
    phone: Option<String>,
    status: String,
}

#[derive(Debug)]
struct PreparedPreviewRow {
    row_number: i32,
    source_data: Value,
    canonical_data: Value,
    outcome: &'static str,
    issues: Vec<String>,
    duplicate_record_id: Option<Uuid>,
}

pub struct SisImportOps;

impl SisImportOps {
    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        input: NewSisImport,
    ) -> Result<SisImportRecord> {
        let file_name = safe_file_name(&input.file_name)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start import upload")?;
        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO data_imports (
                tenant_id, module_key, entity_key, file_name, content_type,
                source_format, source_sha256, source_bytes, source_size_bytes,
                source_row_count, source_headers, created_by
            ) VALUES ($1, 'sis', $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(input.target.as_str())
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
        .context("Failed to retain import source")?;
        append_import_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "sis.imports.upload",
            id,
            json!({
                "target": input.target.as_str(),
                "row_count": input.parsed.table.rows.len()
            }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to save import upload")?;
        Self::get(pool, tenant_id, id)
            .await?
            .context("Created import could not be reloaded")
    }

    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        target: Option<SisImportTarget>,
    ) -> Result<(Vec<SisImportRecord>, i64)> {
        let offset = (page - 1) * per_page;
        let target = target.map(SisImportTarget::as_str);
        let rows = sqlx::query_as::<_, SisImportRecord>(&format!(
            "{} WHERE import.tenant_id = $1 AND import.module_key = 'sis' AND ($2::TEXT IS NULL OR import.entity_key = $2) ORDER BY import.created_at DESC LIMIT $3 OFFSET $4",
            import_select()
        ))
        .bind(tenant_id)
        .bind(target)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list SIS imports")?;
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM data_imports WHERE tenant_id = $1 AND module_key = 'sis' AND ($2::TEXT IS NULL OR entity_key = $2)",
        )
        .bind(tenant_id)
        .bind(target)
        .fetch_one(pool)
        .await
        .context("Failed to count SIS imports")?;
        Ok((rows, total))
    }

    /// Lists SIS imports only after the route has proved campus scope.
    pub(crate) async fn list_scoped(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        target: Option<SisImportTarget>,
        _scope: SisCampusReadScope,
    ) -> Result<(Vec<SisImportRecord>, i64)> {
        Self::list(pool, tenant_id, page, per_page, target).await
    }

    pub async fn get(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<Option<SisImportRecord>> {
        sqlx::query_as::<_, SisImportRecord>(&format!(
            "{} WHERE import.tenant_id = $1 AND import.module_key = 'sis' AND import.id = $2",
            import_select()
        ))
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load SIS import")
    }

    /// Reads import metadata only after the route has proved campus scope.
    pub(crate) async fn get_scoped(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
        _scope: SisCampusReadScope,
    ) -> Result<Option<SisImportRecord>> {
        Self::get(pool, tenant_id, id).await
    }

    pub async fn retained_source(
        pool: &PgPool,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<RetainedImportSource>> {
        sqlx::query_as::<_, RetainedImportSource>(
            r#"
            SELECT id, entity_key, file_name, source_bytes, status
            FROM data_imports
            WHERE tenant_id = $1 AND module_key = 'sis' AND id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load retained import source")
    }

    pub async fn create_preview(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        import_id: Uuid,
        mapping: SisImportMapping,
        table: &SourceTable,
    ) -> Result<SisImportPreview> {
        let source = Self::retained_source(pool, tenant_id, import_id)
            .await?
            .context("Import not found")?;
        if source.status == "committed" {
            bail!("A committed import cannot be remapped.");
        }
        let target = SisImportTarget::parse(&source.entity_key)?;
        validate_mapping(target, &mapping, &table.headers)?;
        let rows = prepare_rows(pool, tenant_id, target, &mapping, table).await?;
        let ready_rows = count_outcome(&rows, "ready");
        let invalid_rows = count_outcome(&rows, "invalid");
        let duplicate_rows = count_outcome(&rows, "duplicate");
        let mapping_json = serde_json::to_value(&mapping).context("Failed to encode mapping")?;

        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start import preview")?;
        sqlx::query("SELECT id FROM data_imports WHERE tenant_id = $1 AND id = $2 AND status <> 'committed' FOR UPDATE")
            .bind(tenant_id)
            .bind(import_id)
            .fetch_optional(&mut *transaction)
            .await
            .context("Failed to lock import")?
            .context("Import is no longer available for preview")?;
        let version = sqlx::query_scalar::<_, i32>(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM data_import_mappings WHERE tenant_id = $1 AND import_id = $2",
        )
        .bind(tenant_id)
        .bind(import_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to allocate mapping version")?;
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
        .context("Failed to save import mapping")?;
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
        .context("Failed to save import preview")?;
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
            .context("Failed to save import preview row")?;
        }
        sqlx::query(
            "UPDATE data_imports SET status = 'preview_ready' WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(import_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to update import state")?;
        append_import_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "sis.imports.preview",
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
            .context("Failed to save import preview")?;
        Self::preview(pool, tenant_id, import_id, 1, 100)
            .await?
            .context("Created import preview could not be reloaded")
    }

    pub async fn preview(
        pool: &PgPool,
        tenant_id: Uuid,
        import_id: Uuid,
        page: i64,
        per_page: i64,
    ) -> Result<Option<SisImportPreview>> {
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
              AND import.module_key = 'sis'
            ORDER BY mapping.version DESC
            LIMIT 1
            "#,
        )
        .bind(tenant_id)
        .bind(import_id)
        .fetch_optional(pool)
        .await
        .context("Failed to load import preview")?;
        let Some(header) = header else {
            return Ok(None);
        };
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, SisImportPreviewRow>(
            r#"
            SELECT id, row_number, canonical_data, outcome,
                   issues, duplicate_record_id
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
        .context("Failed to load import preview rows")?;
        let total_rows = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM data_import_preview_rows WHERE tenant_id = $1 AND preview_id = $2",
        )
        .bind(tenant_id)
        .bind(header.id)
        .fetch_one(pool)
        .await
        .context("Failed to count import preview rows")?;
        let mapping =
            serde_json::from_value(header.mapping).context("Stored import mapping is invalid")?;
        Ok(Some(SisImportPreview {
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

    /// Reads preview rows only after the route has proved campus scope.
    pub(crate) async fn preview_scoped(
        pool: &PgPool,
        tenant_id: Uuid,
        import_id: Uuid,
        page: i64,
        per_page: i64,
        _scope: SisCampusReadScope,
    ) -> Result<Option<SisImportPreview>> {
        Self::preview(pool, tenant_id, import_id, page, per_page).await
    }

    pub async fn commit(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        import_id: Uuid,
        preview_id: Uuid,
    ) -> Result<SisImportCommit> {
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start import commit")?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("{tenant_id}:sis-import"))
            .execute(&mut *transaction)
            .await
            .context("Failed to serialize SIS import commit")?;
        if let Some(existing) =
            load_commit(&mut transaction, tenant_id, import_id, preview_id).await?
        {
            transaction
                .commit()
                .await
                .context("Failed to finish import lookup")?;
            return Ok(existing);
        }
        let target = sqlx::query_scalar::<_, String>(
            r#"
            SELECT import.entity_key
            FROM data_imports AS import
            JOIN data_import_previews AS preview
              ON preview.import_id = import.id AND preview.tenant_id = import.tenant_id
            WHERE import.tenant_id = $1 AND import.module_key = 'sis'
              AND import.id = $2 AND preview.id = $3
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
        .bind(import_id)
        .bind(preview_id)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to lock import preview")?
        .context("The selected preview is not available for commit")?;
        let target = SisImportTarget::parse(&target)?;
        let rows = sqlx::query_as::<_, SisImportPreviewRow>(
            r#"
            SELECT id, row_number, canonical_data, outcome,
                   issues, duplicate_record_id
            FROM data_import_preview_rows
            WHERE tenant_id = $1 AND preview_id = $2
            ORDER BY row_number
            "#,
        )
        .bind(tenant_id)
        .bind(preview_id)
        .fetch_all(&mut *transaction)
        .await
        .context("Failed to load rows for import commit")?;
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
                        .context("Failed to start import row savepoint")?;
                    match commit_ready_row(&mut savepoint, tenant_id, target, &row).await {
                        Ok(Some(record_id)) => {
                            savepoint
                                .commit()
                                .await
                                .context("Failed to save imported row")?;
                            RowCommitResult::created(row.id, record_id)
                        }
                        Ok(None) => {
                            savepoint
                                .commit()
                                .await
                                .context("Failed to save duplicate row result")?;
                            RowCommitResult::skipped(row.id, None)
                        }
                        Err(error) => {
                            savepoint
                                .rollback()
                                .await
                                .context("Failed to recover rejected import row")?;
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
        .bind(actor.user_id())
        .bind(committed_at)
        .execute(&mut *transaction)
        .await
        .context("Failed to save import commit")?;
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
            .context("Failed to save import row result")?;
        }
        sqlx::query(
            "UPDATE data_imports SET status = 'committed' WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(import_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to mark import committed")?;
        append_import_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "sis.imports.commit",
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
            .context("Failed to commit import")?;
        Ok(SisImportCommit {
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

fn validate_mapping(
    target: SisImportTarget,
    mapping: &SisImportMapping,
    source_headers: &[String],
) -> Result<()> {
    let allowed = target
        .fields()
        .iter()
        .map(|field| field.key)
        .collect::<HashSet<_>>();
    if mapping
        .columns
        .keys()
        .any(|key| !allowed.contains(key.as_str()))
    {
        bail!("The mapping contains an unsupported destination field.");
    }
    for field in target.fields().iter().filter(|field| field.required) {
        if mapping
            .columns
            .get(field.key)
            .is_none_or(|value| value.trim().is_empty())
        {
            bail!("Map the required {} field.", field.label.to_lowercase());
        }
    }
    if target == SisImportTarget::Learners && mapping.date_format.is_none() {
        bail!("Choose the date format used by the learner source.");
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
        bail!("Each source column can map to only one SIS field.");
    }
    Ok(())
}

async fn prepare_rows(
    pool: &PgPool,
    tenant_id: Uuid,
    target: SisImportTarget,
    mapping: &SisImportMapping,
    table: &SourceTable,
) -> Result<Vec<PreparedPreviewRow>> {
    match target {
        SisImportTarget::Learners => prepare_learner_rows(pool, tenant_id, mapping, table).await,
        SisImportTarget::Guardians => prepare_guardian_rows(pool, tenant_id, mapping, table).await,
    }
}

async fn prepare_learner_rows(
    pool: &PgPool,
    tenant_id: Uuid,
    mapping: &SisImportMapping,
    table: &SourceTable,
) -> Result<Vec<PreparedPreviewRow>> {
    let candidates = table
        .rows
        .iter()
        .map(|row| {
            let source_data = source_json(table, row);
            let (value, issues) = map_learner(table, row, mapping);
            (row, source_data, value, issues)
        })
        .collect::<Vec<_>>();
    let keys = candidates
        .iter()
        .filter_map(|(_, _, value, _)| value.as_ref())
        .map(|value| value.learner_number.to_lowercase())
        .collect::<Vec<_>>();
    let existing = sqlx::query_as::<_, (String, Uuid)>(
        "SELECT LOWER(learner_number), id FROM learners WHERE tenant_id = $1 AND deleted_at IS NULL AND LOWER(learner_number) = ANY($2)",
    )
    .bind(tenant_id)
    .bind(&keys)
    .fetch_all(pool)
    .await
    .context("Failed to check learner import duplicates")?
    .into_iter()
    .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    Ok(candidates
        .into_iter()
        .map(|(row, source_data, value, issues)| {
            let duplicate_record_id = value
                .as_ref()
                .and_then(|value| existing.get(&value.learner_number.to_lowercase()).copied());
            let repeated = value
                .as_ref()
                .is_some_and(|value| !seen.insert(value.learner_number.to_lowercase()));
            finish_preview_row(
                row,
                source_data,
                value.map_or_else(|| json!({}), |value| json!(value)),
                issues,
                duplicate_record_id,
                repeated,
            )
        })
        .collect())
}

async fn prepare_guardian_rows(
    pool: &PgPool,
    tenant_id: Uuid,
    mapping: &SisImportMapping,
    table: &SourceTable,
) -> Result<Vec<PreparedPreviewRow>> {
    let candidates = table
        .rows
        .iter()
        .map(|row| {
            let source_data = source_json(table, row);
            let (value, issues) = map_guardian(table, row, mapping);
            (row, source_data, value, issues)
        })
        .collect::<Vec<_>>();
    let emails = candidates
        .iter()
        .filter_map(|(_, _, value, _)| value.as_ref()?.email.clone())
        .collect::<Vec<_>>();
    let phones = candidates
        .iter()
        .filter_map(|(_, _, value, _)| value.as_ref()?.phone.clone())
        .collect::<Vec<_>>();
    let existing_rows = sqlx::query_as::<_, (Uuid, Option<String>, Option<String>)>(
        r#"
        SELECT id, LOWER(email), phone FROM guardians
        WHERE tenant_id = $1 AND deleted_at IS NULL
          AND ((email IS NOT NULL AND LOWER(email) = ANY($2)) OR (phone IS NOT NULL AND phone = ANY($3)))
        "#,
    )
    .bind(tenant_id)
    .bind(&emails)
    .bind(&phones)
    .fetch_all(pool)
    .await
    .context("Failed to check guardian import duplicates")?;
    let mut existing = HashMap::new();
    for (id, email, phone) in existing_rows {
        if let Some(email) = email {
            existing.insert(format!("email:{email}"), id);
        }
        if let Some(phone) = phone {
            existing.insert(format!("phone:{phone}"), id);
        }
    }
    let mut seen = HashSet::new();
    Ok(candidates
        .into_iter()
        .map(|(row, source_data, value, issues)| {
            let keys = value.as_ref().map(guardian_dedupe_keys).unwrap_or_default();
            let duplicate_record_id = keys.iter().find_map(|key| existing.get(key).copied());
            let repeated = keys.iter().any(|key| !seen.insert(key.clone()));
            finish_preview_row(
                row,
                source_data,
                value.map_or_else(|| json!({}), |value| json!(value)),
                issues,
                duplicate_record_id,
                repeated,
            )
        })
        .collect())
}

fn finish_preview_row(
    row: &SourceRow,
    source_data: Value,
    canonical_data: Value,
    mut issues: Vec<String>,
    duplicate_record_id: Option<Uuid>,
    repeated: bool,
) -> PreparedPreviewRow {
    let outcome = if !issues.is_empty() {
        "invalid"
    } else if duplicate_record_id.is_some() || repeated {
        issues.push(if duplicate_record_id.is_some() {
            "A matching SIS record already exists.".to_string()
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
        canonical_data,
        outcome,
        issues,
        duplicate_record_id,
    }
}

fn map_learner(
    table: &SourceTable,
    row: &SourceRow,
    mapping: &SisImportMapping,
) -> (Option<ImportedLearner>, Vec<String>) {
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
    let display_name = required_mapped(
        table,
        row,
        mapping,
        "display_name",
        "Display name",
        200,
        &mut issues,
    );
    let first_names = optional_mapped(
        table,
        row,
        mapping,
        "first_names",
        "First names",
        120,
        &mut issues,
    );
    let surname = optional_mapped(table, row, mapping, "surname", "Surname", 120, &mut issues);
    let email = normalized_email(
        optional_mapped(table, row, mapping, "email", "Email", 320, &mut issues),
        &mut issues,
    );
    let phone = optional_mapped(table, row, mapping, "phone", "Phone", 50, &mut issues);
    let status = optional_mapped(table, row, mapping, "status", "Status", 30, &mut issues)
        .unwrap_or_else(|| "prospective".to_string())
        .to_lowercase();
    if !matches!(
        status.as_str(),
        "prospective" | "active" | "inactive" | "graduated" | "withdrawn"
    ) {
        issues.push(
            "Status must be prospective, active, inactive, graduated, or withdrawn.".to_string(),
        );
    }
    let date_raw = mapped(table, row, mapping, "date_of_birth");
    let date_of_birth = match (date_raw, mapping.date_format) {
        (Some(value), Some(format)) => {
            NaiveDate::parse_from_str(value, format.chrono_pattern()).ok()
        }
        _ => None,
    };
    if date_of_birth.is_none() {
        issues.push("Date of birth does not match the selected date format.".to_string());
    } else if date_of_birth.is_some_and(|date| date > Utc::now().date_naive()) {
        issues.push("Date of birth cannot be in the future.".to_string());
    }
    if !issues.is_empty() {
        return (None, issues);
    }
    match (learner_number, display_name, date_of_birth) {
        (Some(learner_number), Some(display_name), Some(date_of_birth)) => (
            Some(ImportedLearner {
                learner_number,
                display_name,
                first_names,
                surname,
                date_of_birth,
                email,
                phone,
                status,
            }),
            issues,
        ),
        _ => (
            None,
            vec!["Required learner values are missing.".to_string()],
        ),
    }
}

fn map_guardian(
    table: &SourceTable,
    row: &SourceRow,
    mapping: &SisImportMapping,
) -> (Option<ImportedGuardian>, Vec<String>) {
    let mut issues = Vec::new();
    let display_name = required_mapped(
        table,
        row,
        mapping,
        "display_name",
        "Display name",
        200,
        &mut issues,
    );
    let first_names = optional_mapped(
        table,
        row,
        mapping,
        "first_names",
        "First names",
        120,
        &mut issues,
    );
    let surname = optional_mapped(table, row, mapping, "surname", "Surname", 120, &mut issues);
    let email = normalized_email(
        optional_mapped(table, row, mapping, "email", "Email", 320, &mut issues),
        &mut issues,
    );
    let phone = optional_mapped(table, row, mapping, "phone", "Phone", 50, &mut issues);
    if email.is_none() && phone.is_none() {
        issues.push("Enter an email address or phone number.".to_string());
    }
    let status = optional_mapped(table, row, mapping, "status", "Status", 30, &mut issues)
        .unwrap_or_else(|| "active".to_string())
        .to_lowercase();
    if !matches!(status.as_str(), "active" | "inactive") {
        issues.push("Status must be active or inactive.".to_string());
    }
    if !issues.is_empty() {
        return (None, issues);
    }
    match display_name {
        Some(display_name) => (
            Some(ImportedGuardian {
                display_name,
                first_names,
                surname,
                email,
                phone,
                status,
            }),
            issues,
        ),
        None => (None, vec!["Guardian display name is missing.".to_string()]),
    }
}

fn mapped<'a>(
    table: &'a SourceTable,
    row: &'a SourceRow,
    mapping: &'a SisImportMapping,
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
    mapping: &SisImportMapping,
    key: &str,
    label: &str,
    max: usize,
    issues: &mut Vec<String>,
) -> Option<String> {
    let value = mapped(table, row, mapping, key).map(ToOwned::to_owned);
    if value.is_none() {
        issues.push(format!("{label} is required."));
    }
    if value.as_ref().is_some_and(|value| value.len() > max) {
        issues.push(format!("{label} is too long."));
    }
    value
}

fn optional_mapped(
    table: &SourceTable,
    row: &SourceRow,
    mapping: &SisImportMapping,
    key: &str,
    label: &str,
    max: usize,
    issues: &mut Vec<String>,
) -> Option<String> {
    let value = mapped(table, row, mapping, key).map(ToOwned::to_owned);
    if value.as_ref().is_some_and(|value| value.len() > max) {
        issues.push(format!("{label} is too long."));
    }
    value
}

fn normalized_email(value: Option<String>, issues: &mut Vec<String>) -> Option<String> {
    value.map(|email| email.to_lowercase()).inspect(|email| {
        if !email.validate_email() {
            issues.push("Email is invalid.".to_string());
        }
    })
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

fn guardian_dedupe_keys(value: &ImportedGuardian) -> Vec<String> {
    [
        value.email.as_ref().map(|email| format!("email:{email}")),
        value.phone.as_ref().map(|phone| format!("phone:{phone}")),
    ]
    .into_iter()
    .flatten()
    .collect()
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
            issues: json!(["A matching record exists."]),
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
    target: SisImportTarget,
    row: &SisImportPreviewRow,
) -> Result<Option<Uuid>> {
    match target {
        SisImportTarget::Learners => {
            let value: ImportedLearner = serde_json::from_value(row.canonical_data.clone())
                .context("Stored learner preview is invalid")?;
            // Keep the source identity unchanged. Managed-looking numbers
            // reserve their sequence before insertion, which also preserves
            // lock ordering against ordinary learner creation.
            align_imported_learner_number(transaction, tenant_id, &value.learner_number).await?;
            sqlx::query_scalar::<_, Uuid>(
                r#"
                INSERT INTO learners (
                    tenant_id, learner_number, display_name, first_names, surname,
                    date_of_birth, email, phone, status
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                ON CONFLICT DO NOTHING
                RETURNING id
                "#,
            )
            .bind(tenant_id)
            .bind(value.learner_number)
            .bind(value.display_name)
            .bind(value.first_names)
            .bind(value.surname)
            .bind(value.date_of_birth)
            .bind(value.email)
            .bind(value.phone)
            .bind(value.status)
            .fetch_optional(&mut **transaction)
            .await
            .context("Failed to create imported learner")
        }
        SisImportTarget::Guardians => {
            let value: ImportedGuardian = serde_json::from_value(row.canonical_data.clone())
                .context("Stored guardian preview is invalid")?;
            sqlx::query_scalar::<_, Uuid>(
                r#"
                INSERT INTO guardians (
                    tenant_id, display_name, first_names, surname, email, phone, status
                )
                SELECT $1, $2, $3, $4, $5, $6, $7
                WHERE NOT EXISTS (
                    SELECT 1 FROM guardians
                    WHERE tenant_id = $1 AND deleted_at IS NULL
                      AND (($5::TEXT IS NOT NULL AND LOWER(email) = LOWER($5))
                           OR ($6::TEXT IS NOT NULL AND phone = $6))
                )
                RETURNING id
                "#,
            )
            .bind(tenant_id)
            .bind(value.display_name)
            .bind(value.first_names)
            .bind(value.surname)
            .bind(value.email)
            .bind(value.phone)
            .bind(value.status)
            .fetch_optional(&mut **transaction)
            .await
            .context("Failed to create imported guardian")
        }
    }
}

async fn load_commit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    import_id: Uuid,
    preview_id: Uuid,
) -> Result<Option<SisImportCommit>> {
    sqlx::query_as::<_, SisImportCommit>(
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
    .context("Failed to load import commit")
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
    .context("Failed to audit import operation")?;
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
        "The row could not be saved."
    } else {
        "The stored preview row is invalid."
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use cp_imports::parse_source;

    use super::{
        ImportDateFormat, SisImportMapping, SisImportTarget, map_guardian, map_learner,
        validate_mapping,
    };

    #[test]
    fn learner_mapping_requires_destination_fields_and_explicit_date_format() {
        let table = parse_source("learners.csv", b"number,name,dob\n1,Ada,2010-01-02\n")
            .unwrap_or_else(|error| panic!("fixture should parse: {error}"))
            .table;
        let mapping = SisImportMapping {
            columns: BTreeMap::from([
                ("learner_number".to_string(), "number".to_string()),
                ("display_name".to_string(), "name".to_string()),
                ("date_of_birth".to_string(), "dob".to_string()),
            ]),
            date_format: None,
        };
        assert!(validate_mapping(SisImportTarget::Learners, &mapping, &table.headers).is_err());
        let mapping = SisImportMapping {
            date_format: Some(ImportDateFormat::YyyyMmDd),
            ..mapping
        };
        assert!(validate_mapping(SisImportTarget::Learners, &mapping, &table.headers).is_ok());
        let (learner, issues) = map_learner(&table, &table.rows[0], &mapping);
        assert!(issues.is_empty());
        assert_eq!(
            learner.map(|value| value.learner_number),
            Some("1".to_string())
        );
    }

    #[test]
    fn guardian_mapping_requires_contact_per_row() {
        let table = parse_source("guardians.csv", b"name\nGrace Hopper\n")
            .unwrap_or_else(|error| panic!("fixture should parse: {error}"))
            .table;
        let mapping = SisImportMapping {
            columns: BTreeMap::from([("display_name".to_string(), "name".to_string())]),
            date_format: None,
        };
        let (guardian, issues) = map_guardian(&table, &table.rows[0], &mapping);
        assert!(guardian.is_none());
        assert!(
            issues
                .iter()
                .any(|issue| issue.contains("email address or phone"))
        );
    }
}
