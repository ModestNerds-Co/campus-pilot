//! Staged CSV/XLSX imports for canonical HR employee records.
//!
//! `cp-imports` owns bounded source parsing. HR owns its destination fields,
//! reference resolution, duplicate policy, immutable previews, and commit.

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
use validator::ValidateEmail;

use crate::ops::HrImportReadScope;

const MODULE_KEY: &str = "hr_payroll";
const ENTITY_KEY: &str = "employees";

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ImportField {
    pub key: &'static str,
    pub label: &'static str,
    pub required: bool,
}

pub const EMPLOYEE_IMPORT_FIELDS: &[ImportField] = &[
    ImportField {
        key: "employee_number",
        label: "Employee number",
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
        key: "work_email",
        label: "Work email",
        required: false,
    },
    ImportField {
        key: "phone",
        label: "Phone",
        required: false,
    },
    ImportField {
        key: "department",
        label: "Department code or name",
        required: false,
    },
    ImportField {
        key: "position",
        label: "Position code or title",
        required: false,
    },
    ImportField {
        key: "employment_status",
        label: "Employment status",
        required: false,
    },
    ImportField {
        key: "hire_date",
        label: "Hire date",
        required: false,
    },
    ImportField {
        key: "end_date",
        label: "End date",
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
pub struct HrImportMapping {
    pub columns: BTreeMap<String, String>,
    pub date_format: Option<ImportDateFormat>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct HrImportRecord {
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
pub struct HrImportListResponse {
    pub imports: Vec<HrImportRecord>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct HrImportPreviewRow {
    pub id: Uuid,
    pub row_number: i32,
    pub canonical_data: Value,
    pub outcome: String,
    pub issues: Value,
    pub duplicate_record_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HrImportPreview {
    pub id: Uuid,
    pub import_id: Uuid,
    pub mapping_version: i32,
    pub mapping: HrImportMapping,
    pub ready_rows: i32,
    pub invalid_rows: i32,
    pub duplicate_rows: i32,
    pub created_at: DateTime<Utc>,
    pub rows: Vec<HrImportPreviewRow>,
    pub total_rows: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct HrImportCommit {
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

pub struct NewHrImport {
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
struct ImportedEmployee {
    employee_number: String,
    display_name: String,
    first_names: Option<String>,
    surname: Option<String>,
    work_email: Option<String>,
    phone: Option<String>,
    department_id: Option<Uuid>,
    department_name: Option<String>,
    position_id: Option<Uuid>,
    position_title: Option<String>,
    employment_status: String,
    hire_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
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

#[derive(Debug, Clone)]
struct DepartmentReference {
    id: Uuid,
    name: String,
}

#[derive(Debug, Clone)]
struct PositionReference {
    id: Uuid,
    title: String,
    department_id: Option<Uuid>,
}

#[derive(Default)]
struct ReferenceIndex {
    departments: HashMap<String, DepartmentReference>,
    position_codes: HashMap<String, PositionReference>,
    position_titles: HashMap<String, Vec<PositionReference>>,
}

pub struct HrImportOps;

impl HrImportOps {
    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        input: NewHrImport,
    ) -> Result<HrImportRecord> {
        let file_name = safe_file_name(&input.file_name)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start employee import upload")?;
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
        .context("Failed to retain employee import source")?;
        append_import_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "hr_payroll.imports.upload",
            id,
            json!({ "target": ENTITY_KEY, "row_count": input.parsed.table.rows.len() }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to save employee import upload")?;
        Self::get(pool, tenant_id, id)
            .await?
            .context("Created employee import could not be reloaded")
    }

    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
    ) -> Result<(Vec<HrImportRecord>, i64)> {
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, HrImportRecord>(&format!(
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
        .context("Failed to list employee imports")?;
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM data_imports WHERE tenant_id = $1 AND module_key = $2 AND entity_key = $3",
        )
        .bind(tenant_id)
        .bind(MODULE_KEY)
        .bind(ENTITY_KEY)
        .fetch_one(pool)
        .await
        .context("Failed to count employee imports")?;
        Ok((rows, total))
    }

    /// Lists import metadata only after the HTTP boundary proves campus scope.
    pub async fn list_for_scope(
        pool: &PgPool,
        tenant_id: Uuid,
        _scope: HrImportReadScope,
        page: i64,
        per_page: i64,
    ) -> Result<(Vec<HrImportRecord>, i64)> {
        Self::list(pool, tenant_id, page, per_page).await
    }

    pub async fn get(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<Option<HrImportRecord>> {
        sqlx::query_as::<_, HrImportRecord>(&format!(
            "{} WHERE import.tenant_id = $1 AND import.module_key = $2 AND import.entity_key = $3 AND import.id = $4",
            import_select()
        ))
        .bind(tenant_id)
        .bind(MODULE_KEY)
        .bind(ENTITY_KEY)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to load employee import")
    }

    /// Loads import metadata only after the HTTP boundary proves campus scope.
    pub async fn get_for_scope(
        pool: &PgPool,
        tenant_id: Uuid,
        _scope: HrImportReadScope,
        id: Uuid,
    ) -> Result<Option<HrImportRecord>> {
        Self::get(pool, tenant_id, id).await
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
        .context("Failed to load retained employee import source")
    }

    pub async fn create_preview(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        import_id: Uuid,
        mapping: HrImportMapping,
        table: &SourceTable,
    ) -> Result<HrImportPreview> {
        let source = Self::retained_source(pool, tenant_id, import_id)
            .await?
            .context("Employee import not found")?;
        if source.status == "committed" {
            bail!("A committed import cannot be remapped.");
        }
        validate_mapping(&mapping, &table.headers)?;
        let references = load_reference_index(pool, tenant_id).await?;
        let rows = prepare_rows(pool, tenant_id, &mapping, table, &references).await?;
        let ready_rows = count_outcome(&rows, "ready");
        let invalid_rows = count_outcome(&rows, "invalid");
        let duplicate_rows = count_outcome(&rows, "duplicate");
        let mapping_json = serde_json::to_value(&mapping).context("Failed to encode mapping")?;

        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start employee import preview")?;
        sqlx::query(
            "SELECT id FROM data_imports WHERE tenant_id = $1 AND module_key = $2 AND id = $3 AND status <> 'committed' FOR UPDATE",
        )
        .bind(tenant_id)
        .bind(MODULE_KEY)
        .bind(import_id)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to lock employee import")?
        .context("Employee import is no longer available for preview")?;
        let version = sqlx::query_scalar::<_, i32>(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM data_import_mappings WHERE tenant_id = $1 AND import_id = $2",
        )
        .bind(tenant_id)
        .bind(import_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to allocate employee mapping version")?;
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
        .context("Failed to save employee import mapping")?;
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
        .context("Failed to save employee import preview")?;
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
            .context("Failed to save employee import preview row")?;
        }
        sqlx::query(
            "UPDATE data_imports SET status = 'preview_ready' WHERE tenant_id = $1 AND module_key = $2 AND id = $3",
        )
        .bind(tenant_id)
        .bind(MODULE_KEY)
        .bind(import_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to update employee import state")?;
        append_import_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "hr_payroll.imports.preview",
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
            .context("Failed to save employee import preview")?;
        Self::preview(pool, tenant_id, import_id, 1, 100)
            .await?
            .context("Created employee import preview could not be reloaded")
    }

    pub async fn preview(
        pool: &PgPool,
        tenant_id: Uuid,
        import_id: Uuid,
        page: i64,
        per_page: i64,
    ) -> Result<Option<HrImportPreview>> {
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
        .context("Failed to load employee import preview")?;
        let Some(header) = header else {
            return Ok(None);
        };
        let offset = (page - 1) * per_page;
        let rows = sqlx::query_as::<_, HrImportPreviewRow>(
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
        .context("Failed to load employee import preview rows")?;
        let total_rows = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM data_import_preview_rows WHERE tenant_id = $1 AND preview_id = $2",
        )
        .bind(tenant_id)
        .bind(header.id)
        .fetch_one(pool)
        .await
        .context("Failed to count employee import preview rows")?;
        let mapping = serde_json::from_value(header.mapping)
            .context("Stored employee import mapping is invalid")?;
        Ok(Some(HrImportPreview {
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

    /// Loads a validated import preview only after campus scope is proved.
    pub async fn preview_for_scope(
        pool: &PgPool,
        tenant_id: Uuid,
        _scope: HrImportReadScope,
        import_id: Uuid,
        page: i64,
        per_page: i64,
    ) -> Result<Option<HrImportPreview>> {
        Self::preview(pool, tenant_id, import_id, page, per_page).await
    }

    pub async fn commit(
        pool: &PgPool,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        import_id: Uuid,
        preview_id: Uuid,
    ) -> Result<HrImportCommit> {
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start employee import commit")?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("{tenant_id}:hr-employee-import"))
            .execute(&mut *transaction)
            .await
            .context("Failed to serialize employee import commit")?;
        if let Some(existing) =
            load_commit(&mut transaction, tenant_id, import_id, preview_id).await?
        {
            transaction
                .commit()
                .await
                .context("Failed to finish employee import lookup")?;
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
        .context("Failed to lock employee import preview")?
        .context("The selected employee preview is not available for commit")?;
        let rows = sqlx::query_as::<_, HrImportPreviewRow>(
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
        .context("Failed to load rows for employee import commit")?;
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
                        .context("Failed to start employee import row savepoint")?;
                    match commit_ready_row(&mut savepoint, tenant_id, &row).await {
                        Ok(Some(record_id)) => {
                            savepoint
                                .commit()
                                .await
                                .context("Failed to save imported employee")?;
                            RowCommitResult::created(row.id, record_id)
                        }
                        Ok(None) => {
                            savepoint
                                .commit()
                                .await
                                .context("Failed to save duplicate employee result")?;
                            RowCommitResult::skipped(row.id, None)
                        }
                        Err(error) => {
                            savepoint
                                .rollback()
                                .await
                                .context("Failed to recover rejected employee row")?;
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
        .context("Failed to save employee import commit")?;
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
            .context("Failed to save employee import row result")?;
        }
        sqlx::query(
            "UPDATE data_imports SET status = 'committed' WHERE tenant_id = $1 AND module_key = $2 AND id = $3",
        )
        .bind(tenant_id)
        .bind(MODULE_KEY)
        .bind(import_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to mark employee import committed")?;
        append_import_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "hr_payroll.imports.commit",
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
            .context("Failed to commit employee import")?;
        Ok(HrImportCommit {
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

fn validate_mapping(mapping: &HrImportMapping, source_headers: &[String]) -> Result<()> {
    let allowed = EMPLOYEE_IMPORT_FIELDS
        .iter()
        .map(|field| field.key)
        .collect::<HashSet<_>>();
    if mapping
        .columns
        .keys()
        .any(|key| !allowed.contains(key.as_str()))
    {
        bail!("The mapping contains an unsupported employee field.");
    }
    for field in EMPLOYEE_IMPORT_FIELDS.iter().filter(|field| field.required) {
        if mapping
            .columns
            .get(field.key)
            .is_none_or(|value| value.trim().is_empty())
        {
            bail!("Map the required {} field.", field.label.to_lowercase());
        }
    }
    if mapping.date_format.is_none()
        && ["hire_date", "end_date"]
            .iter()
            .any(|key| mapping.columns.contains_key(*key))
    {
        bail!("Choose the date format used by the employee source.");
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
        bail!("Each source column can map to only one HR field.");
    }
    Ok(())
}

async fn load_reference_index(pool: &PgPool, tenant_id: Uuid) -> Result<ReferenceIndex> {
    let departments = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT id, code, name FROM departments WHERE tenant_id = $1 AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .context("Failed to load employee import departments")?;
    let positions = sqlx::query_as::<_, (Uuid, String, String, Option<Uuid>)>(
        "SELECT id, code, title, department_id FROM positions WHERE tenant_id = $1 AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .context("Failed to load employee import positions")?;
    let mut index = ReferenceIndex::default();
    for (id, code, name) in departments {
        let reference = DepartmentReference { id, name };
        index
            .departments
            .insert(code.trim().to_lowercase(), reference.clone());
        index
            .departments
            .insert(reference.name.trim().to_lowercase(), reference);
    }
    for (id, code, title, department_id) in positions {
        let reference = PositionReference {
            id,
            title,
            department_id,
        };
        index
            .position_codes
            .insert(code.trim().to_lowercase(), reference.clone());
        index
            .position_titles
            .entry(reference.title.trim().to_lowercase())
            .or_default()
            .push(reference);
    }
    Ok(index)
}

async fn prepare_rows(
    pool: &PgPool,
    tenant_id: Uuid,
    mapping: &HrImportMapping,
    table: &SourceTable,
    references: &ReferenceIndex,
) -> Result<Vec<PreparedPreviewRow>> {
    let candidates = table
        .rows
        .iter()
        .map(|row| {
            let source_data = source_json(table, row);
            let (value, issues) = map_employee(table, row, mapping, references);
            (row, source_data, value, issues)
        })
        .collect::<Vec<_>>();
    let numbers = candidates
        .iter()
        .filter_map(|(_, _, value, _)| value.as_ref())
        .map(|value| value.employee_number.to_lowercase())
        .collect::<Vec<_>>();
    let emails = candidates
        .iter()
        .filter_map(|(_, _, value, _)| value.as_ref()?.work_email.clone())
        .collect::<Vec<_>>();
    let existing_rows = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
        r#"
        SELECT id, LOWER(employee_number), LOWER(work_email)
        FROM employees
        WHERE tenant_id = $1 AND deleted_at IS NULL
          AND (LOWER(employee_number) = ANY($2)
               OR (work_email IS NOT NULL AND LOWER(work_email) = ANY($3)))
        "#,
    )
    .bind(tenant_id)
    .bind(&numbers)
    .bind(&emails)
    .fetch_all(pool)
    .await
    .context("Failed to check employee import duplicates")?;
    let mut existing = HashMap::new();
    for (id, number, email) in existing_rows {
        existing.insert(format!("number:{number}"), id);
        if let Some(email) = email {
            existing.insert(format!("email:{email}"), id);
        }
    }
    let mut seen = HashSet::new();
    Ok(candidates
        .into_iter()
        .map(|(row, source_data, value, mut issues)| {
            let keys = value.as_ref().map(employee_dedupe_keys).unwrap_or_default();
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
                    "A matching employee record already exists.".to_string()
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

fn map_employee(
    table: &SourceTable,
    row: &SourceRow,
    mapping: &HrImportMapping,
    references: &ReferenceIndex,
) -> (Option<ImportedEmployee>, Vec<String>) {
    let mut issues = Vec::new();
    let employee_number = required_mapped(
        table,
        row,
        mapping,
        "employee_number",
        "Employee number",
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
    let work_email = normalized_email(
        optional_mapped(
            table,
            row,
            mapping,
            "work_email",
            "Work email",
            320,
            &mut issues,
        ),
        &mut issues,
    );
    let phone = optional_mapped(table, row, mapping, "phone", "Phone", 50, &mut issues);
    let employment_status = optional_mapped(
        table,
        row,
        mapping,
        "employment_status",
        "Employment status",
        30,
        &mut issues,
    )
    .unwrap_or_else(|| "active".to_string())
    .to_lowercase();
    if !matches!(
        employment_status.as_str(),
        "active" | "inactive" | "suspended" | "terminated"
    ) {
        issues.push(
            "Employment status must be active, inactive, suspended, or terminated.".to_string(),
        );
    }

    let department = resolve_department(
        mapped(table, row, mapping, "department"),
        references,
        &mut issues,
    );
    let position = resolve_position(
        mapped(table, row, mapping, "position"),
        department.as_ref().map(|value| value.id),
        references,
        &mut issues,
    );
    if let (Some(department), Some(position_department_id)) = (
        department.as_ref(),
        position.as_ref().and_then(|value| value.department_id),
    ) && department.id != position_department_id
    {
        issues.push("Position does not belong to the mapped department.".to_string());
    }
    let inferred_department = department.or_else(|| {
        position
            .as_ref()
            .and_then(|value| value.department_id)
            .and_then(|id| {
                references
                    .departments
                    .values()
                    .find(|value| value.id == id)
                    .cloned()
            })
    });
    let hire_date = parse_optional_date(
        mapped(table, row, mapping, "hire_date"),
        mapping.date_format,
        "Hire date",
        &mut issues,
    );
    let end_date = parse_optional_date(
        mapped(table, row, mapping, "end_date"),
        mapping.date_format,
        "End date",
        &mut issues,
    );
    if let (Some(hire_date), Some(end_date)) = (hire_date, end_date)
        && end_date < hire_date
    {
        issues.push("Employment end date cannot be before the hire date.".to_string());
    }
    if !issues.is_empty() {
        return (None, issues);
    }
    match (employee_number, display_name) {
        (Some(employee_number), Some(display_name)) => (
            Some(ImportedEmployee {
                employee_number,
                display_name,
                first_names,
                surname,
                work_email,
                phone,
                department_id: inferred_department.as_ref().map(|value| value.id),
                department_name: inferred_department.map(|value| value.name),
                position_id: position.as_ref().map(|value| value.id),
                position_title: position.map(|value| value.title),
                employment_status,
                hire_date,
                end_date,
            }),
            issues,
        ),
        _ => (None, issues),
    }
}

fn resolve_department(
    value: Option<&str>,
    references: &ReferenceIndex,
    issues: &mut Vec<String>,
) -> Option<DepartmentReference> {
    let value = value?;
    match references.departments.get(&value.to_lowercase()).cloned() {
        Some(reference) => Some(reference),
        None => {
            issues.push(format!("Department '{value}' was not found."));
            None
        }
    }
}

fn resolve_position(
    value: Option<&str>,
    department_id: Option<Uuid>,
    references: &ReferenceIndex,
    issues: &mut Vec<String>,
) -> Option<PositionReference> {
    let value = value?;
    let key = value.to_lowercase();
    if let Some(reference) = references.position_codes.get(&key) {
        return Some(reference.clone());
    }
    let candidates = references
        .position_titles
        .get(&key)
        .into_iter()
        .flatten()
        .filter(|candidate| {
            department_id.is_none()
                || candidate.department_id.is_none()
                || candidate.department_id == department_id
        })
        .cloned()
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [reference] => Some(reference.clone()),
        [] => {
            issues.push(format!("Position '{value}' was not found."));
            None
        }
        _ => {
            issues.push(format!(
                "Position '{value}' is ambiguous; use its unique position code."
            ));
            None
        }
    }
}

fn parse_optional_date(
    value: Option<&str>,
    format: Option<ImportDateFormat>,
    label: &str,
    issues: &mut Vec<String>,
) -> Option<NaiveDate> {
    let value = value?;
    let parsed =
        format.and_then(|format| NaiveDate::parse_from_str(value, format.chrono_pattern()).ok());
    if parsed.is_none() {
        issues.push(format!("{label} does not match the selected date format."));
    }
    parsed
}

fn mapped<'a>(
    table: &'a SourceTable,
    row: &'a SourceRow,
    mapping: &HrImportMapping,
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
    mapping: &HrImportMapping,
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
    mapping: &HrImportMapping,
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
            issues.push("Work email is invalid.".to_string());
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

fn employee_dedupe_keys(value: &ImportedEmployee) -> Vec<String> {
    let mut keys = vec![format!("number:{}", value.employee_number.to_lowercase())];
    if let Some(email) = &value.work_email {
        keys.push(format!("email:{email}"));
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
            issues: json!(["A matching employee exists."]),
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
    row: &HrImportPreviewRow,
) -> Result<Option<Uuid>> {
    let value: ImportedEmployee = serde_json::from_value(row.canonical_data.clone())
        .context("Stored employee preview is invalid")?;
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO employees (
            tenant_id, employee_number, display_name, first_names, surname,
            work_email, phone, department_id, position_id, employment_status,
            hire_date, end_date
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        ON CONFLICT DO NOTHING
        RETURNING id
        "#,
    )
    .bind(tenant_id)
    .bind(value.employee_number)
    .bind(value.display_name)
    .bind(value.first_names)
    .bind(value.surname)
    .bind(value.work_email)
    .bind(value.phone)
    .bind(value.department_id)
    .bind(value.position_id)
    .bind(value.employment_status)
    .bind(value.hire_date)
    .bind(value.end_date)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to create imported employee")
}

async fn load_commit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    import_id: Uuid,
    preview_id: Uuid,
) -> Result<Option<HrImportCommit>> {
    sqlx::query_as::<_, HrImportCommit>(
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
    .context("Failed to load employee import commit")
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
    .context("Failed to audit employee import operation")?;
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
        "The employee row could not be saved."
    } else {
        "The stored employee preview row is invalid."
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use cp_imports::parse_source;

    use super::{
        HrImportMapping, ImportDateFormat, ReferenceIndex, map_employee, validate_mapping,
    };

    #[test]
    fn employee_mapping_requires_identity_fields_and_explicit_date_format() {
        let table = parse_source(
            "employees.csv",
            b"number,name,hire_date\nEMP-1,Ada Lovelace,2026-01-02\n",
        )
        .unwrap_or_else(|error| panic!("fixture should parse: {error}"))
        .table;
        let mapping = HrImportMapping {
            columns: BTreeMap::from([
                ("employee_number".to_string(), "number".to_string()),
                ("display_name".to_string(), "name".to_string()),
                ("hire_date".to_string(), "hire_date".to_string()),
            ]),
            date_format: None,
        };
        assert!(validate_mapping(&mapping, &table.headers).is_err());
        let mapping = HrImportMapping {
            date_format: Some(ImportDateFormat::YyyyMmDd),
            ..mapping
        };
        assert!(validate_mapping(&mapping, &table.headers).is_ok());
        let (value, issues) =
            map_employee(&table, &table.rows[0], &mapping, &ReferenceIndex::default());
        assert!(issues.is_empty());
        assert_eq!(
            value.map(|employee| employee.employee_number),
            Some("EMP-1".to_string())
        );
    }
}
