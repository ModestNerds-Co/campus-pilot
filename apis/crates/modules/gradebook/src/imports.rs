//! Staged CSV/XLSX imports for one existing draft Gradebook mark sheet.
//!
//! The shared import crate parses bounded sources. Gradebook owns roster
//! resolution, mark validation, immutable preview evidence, and the atomic
//! commit. Imports never create people or academic structures and never submit
//! or publish a mark sheet.

use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use cp_audit::{
    AuditActor, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext, append as append_audit,
};
use cp_imports::{ParsedSource, SourceRow, SourceTable};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{GradebookAccessScope, GradebookOps};

const MODULE_KEY: &str = "academics";
const ENTITY_KEY: &str = "assessment_marks";

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkImportDecimalSeparator {
    Dot,
    Comma,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradebookMarkImportMapping {
    pub columns: BTreeMap<String, String>,
    pub decimal_separator: MarkImportDecimalSeparator,
    pub expected_sheet_version: i32,
}

#[derive(Debug, Deserialize)]
pub struct CommitMarkImportRequest {
    pub preview_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct MarkImportListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct MarkImportPreviewQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

pub struct NewGradebookMarkImport {
    pub file_name: String,
    pub content_type: String,
    pub source_bytes: Vec<u8>,
    pub parsed: ParsedSource,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct GradebookMarkImportRecord {
    pub id: Uuid,
    pub mark_sheet_id: Uuid,
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
    pub updated_rows: Option<i32>,
    pub skipped_rows: Option<i32>,
    pub failed_rows: Option<i32>,
    pub committed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct GradebookMarkImportListResponse {
    pub imports: Vec<GradebookMarkImportRecord>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct GradebookMarkImportPreviewRow {
    pub id: Uuid,
    pub row_number: i32,
    pub canonical_data: Value,
    pub outcome: String,
    pub issues: Value,
    pub duplicate_record_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GradebookMarkImportPreview {
    pub id: Uuid,
    pub import_id: Uuid,
    pub mapping_version: i32,
    pub mapping: GradebookMarkImportMapping,
    pub ready_rows: i32,
    pub invalid_rows: i32,
    pub duplicate_rows: i32,
    pub created_at: DateTime<Utc>,
    pub rows: Vec<GradebookMarkImportPreviewRow>,
    pub total_rows: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct GradebookMarkImportCommit {
    pub id: Uuid,
    pub import_id: Uuid,
    pub preview_id: Uuid,
    pub updated_rows: i32,
    pub skipped_rows: i32,
    pub failed_rows: i32,
    pub committed_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub struct RetainedMarkImportSource {
    pub mark_sheet_id: Uuid,
    pub file_name: String,
    pub source_bytes: Vec<u8>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ImportedMark {
    mark_id: Uuid,
    mark_version: i32,
    learner_id: Uuid,
    learner_number: String,
    learner_name: String,
    mark_status: String,
    marks_awarded_hundredths: Option<i64>,
    note: Option<String>,
}

struct PreparedPreviewRow {
    row_number: i32,
    source_data: Value,
    canonical_data: Value,
    outcome: &'static str,
    issues: Vec<String>,
    duplicate_record_id: Option<Uuid>,
}

pub struct GradebookMarkImportOps;

impl GradebookMarkImportOps {
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        pool: &PgPool,
        tenant_id: Uuid,
        mark_sheet_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        input: NewGradebookMarkImport,
    ) -> Result<GradebookMarkImportRecord> {
        let actor_id = person_actor_id(actor)?;
        let file_name = safe_file_name(&input.file_name)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start mark import upload")?;
        let status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM assessment_mark_sheets WHERE tenant_id=$1 AND id=$2 AND deleted_at IS NULL FOR SHARE",
        )
        .bind(tenant_id)
        .bind(mark_sheet_id)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to load the mark sheet for import")?
        .context("Assessment mark sheet not found")?;
        if status != "draft" {
            bail!("Only a draft mark sheet can accept an import");
        }
        let import_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO data_imports (
                tenant_id, module_key, entity_key, file_name, content_type,
                source_format, source_sha256, source_bytes, source_size_bytes,
                source_row_count, source_headers, created_by
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
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
        .bind(actor_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to retain mark import source")?;
        sqlx::query(
            "INSERT INTO gradebook_mark_imports (tenant_id,import_id,mark_sheet_id,created_by) VALUES ($1,$2,$3,$4)",
        )
        .bind(tenant_id)
        .bind(import_id)
        .bind(mark_sheet_id)
        .bind(actor_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to attach the import to its mark sheet")?;
        append_import_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "academics.gradebook.mark_imports.upload",
            mark_sheet_id,
            json!({ "import_id": import_id, "row_count": input.parsed.table.rows.len() }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to save mark import upload")?;
        Self::get(pool, tenant_id, mark_sheet_id, import_id)
            .await?
            .context("Created mark import could not be reloaded")
    }

    pub async fn list(
        pool: &PgPool,
        tenant_id: Uuid,
        mark_sheet_id: Uuid,
        page: i64,
        per_page: i64,
    ) -> Result<(Vec<GradebookMarkImportRecord>, i64)> {
        let rows = sqlx::query_as::<_, GradebookMarkImportRecord>(&format!(
            "{} WHERE import.tenant_id=$1 AND link.mark_sheet_id=$2 ORDER BY import.created_at DESC LIMIT $3 OFFSET $4",
            import_select(),
        ))
        .bind(tenant_id)
        .bind(mark_sheet_id)
        .bind(per_page)
        .bind((page - 1) * per_page)
        .fetch_all(pool)
        .await
        .context("Failed to list mark imports")?;
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM gradebook_mark_imports WHERE tenant_id=$1 AND mark_sheet_id=$2",
        )
        .bind(tenant_id)
        .bind(mark_sheet_id)
        .fetch_one(pool)
        .await
        .context("Failed to count mark imports")?;
        Ok((rows, total))
    }

    pub async fn get(
        pool: &PgPool,
        tenant_id: Uuid,
        mark_sheet_id: Uuid,
        import_id: Uuid,
    ) -> Result<Option<GradebookMarkImportRecord>> {
        sqlx::query_as::<_, GradebookMarkImportRecord>(&format!(
            "{} WHERE import.tenant_id=$1 AND link.mark_sheet_id=$2 AND import.id=$3",
            import_select(),
        ))
        .bind(tenant_id)
        .bind(mark_sheet_id)
        .bind(import_id)
        .fetch_optional(pool)
        .await
        .context("Failed to load mark import")
    }

    pub async fn retained_source(
        pool: &PgPool,
        tenant_id: Uuid,
        mark_sheet_id: Uuid,
        import_id: Uuid,
    ) -> Result<Option<RetainedMarkImportSource>> {
        sqlx::query_as::<_, RetainedMarkImportSource>(
            r#"
            SELECT link.mark_sheet_id, import.file_name, import.source_bytes, import.status
              FROM data_imports AS import
              JOIN gradebook_mark_imports AS link
                ON link.tenant_id=import.tenant_id AND link.import_id=import.id
             WHERE import.tenant_id=$1 AND link.mark_sheet_id=$2 AND import.id=$3
               AND import.module_key=$4 AND import.entity_key=$5
            "#,
        )
        .bind(tenant_id)
        .bind(mark_sheet_id)
        .bind(import_id)
        .bind(MODULE_KEY)
        .bind(ENTITY_KEY)
        .fetch_optional(pool)
        .await
        .context("Failed to load retained mark import source")
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_preview(
        pool: &PgPool,
        tenant_id: Uuid,
        mark_sheet_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        import_id: Uuid,
        mapping: GradebookMarkImportMapping,
        table: &SourceTable,
    ) -> Result<GradebookMarkImportPreview> {
        let source = Self::retained_source(pool, tenant_id, mark_sheet_id, import_id)
            .await?
            .context("Mark import not found")?;
        if source.status == "committed" {
            bail!("A committed mark import cannot be remapped");
        }
        validate_mapping(&mapping, &table.headers)?;
        let mark_sheet =
            GradebookOps::get(pool, tenant_id, mark_sheet_id, GradebookAccessScope::Campus)
                .await?
                .context("Assessment mark sheet not found")?;
        if mark_sheet.summary.status != "draft" {
            bail!("Only a draft mark sheet can be previewed");
        }
        if mark_sheet.summary.version != mapping.expected_sheet_version {
            bail!("This mark sheet changed. Reload it before creating a preview");
        }
        let rows = prepare_rows(&mapping, table, &mark_sheet)?;
        let ready_rows = count_outcome(&rows, "ready");
        let invalid_rows = count_outcome(&rows, "invalid");
        let duplicate_rows = count_outcome(&rows, "duplicate");
        let mapping_json =
            serde_json::to_value(&mapping).context("Failed to encode mark mapping")?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start mark import preview")?;
        let locked_version = sqlx::query_scalar::<_, i32>(
            r#"
            SELECT sheet.version
              FROM assessment_mark_sheets AS sheet
              JOIN gradebook_mark_imports AS link
                ON link.tenant_id=sheet.tenant_id AND link.mark_sheet_id=sheet.id
              JOIN data_imports AS import
                ON import.tenant_id=link.tenant_id AND import.id=link.import_id
             WHERE sheet.tenant_id=$1 AND sheet.id=$2 AND link.import_id=$3
               AND sheet.deleted_at IS NULL AND sheet.status='draft'
               AND import.status <> 'committed'
             FOR UPDATE OF sheet, import
            "#,
        )
        .bind(tenant_id)
        .bind(mark_sheet_id)
        .bind(import_id)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to lock mark import preview")?
        .context("The mark import is no longer available for preview")?;
        if locked_version != mapping.expected_sheet_version {
            bail!("This mark sheet changed. Reload it before creating a preview");
        }
        let version = sqlx::query_scalar::<_, i32>(
            "SELECT COALESCE(MAX(version),0)+1 FROM data_import_mappings WHERE tenant_id=$1 AND import_id=$2",
        )
        .bind(tenant_id)
        .bind(import_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to allocate mark mapping version")?;
        let mapping_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO data_import_mappings (tenant_id,import_id,version,mapping,created_by) VALUES ($1,$2,$3,$4,$5) RETURNING id",
        )
        .bind(tenant_id)
        .bind(import_id)
        .bind(version)
        .bind(&mapping_json)
        .bind(person_actor_id(actor)?)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to save mark import mapping")?;
        let preview_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO data_import_previews (
                tenant_id,import_id,mapping_id,ready_rows,invalid_rows,duplicate_rows,created_by
            ) VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING id
            "#,
        )
        .bind(tenant_id)
        .bind(import_id)
        .bind(mapping_id)
        .bind(ready_rows)
        .bind(invalid_rows)
        .bind(duplicate_rows)
        .bind(person_actor_id(actor)?)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to save mark import preview")?;
        for row in &rows {
            sqlx::query(
                r#"
                INSERT INTO data_import_preview_rows (
                    tenant_id,preview_id,row_number,source_data,canonical_data,
                    outcome,issues,duplicate_record_id
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
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
            .context("Failed to save mark import preview row")?;
        }
        sqlx::query("UPDATE data_imports SET status='preview_ready' WHERE tenant_id=$1 AND id=$2")
            .bind(tenant_id)
            .bind(import_id)
            .execute(&mut *transaction)
            .await
            .context("Failed to update mark import state")?;
        append_import_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "academics.gradebook.mark_imports.preview",
            mark_sheet_id,
            json!({
                "import_id": import_id, "preview_id": preview_id,
                "ready_rows": ready_rows, "invalid_rows": invalid_rows,
                "duplicate_rows": duplicate_rows, "mapping_version": version
            }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to save mark import preview")?;
        Self::preview(pool, tenant_id, mark_sheet_id, import_id, 1, 100)
            .await?
            .context("Created mark import preview could not be reloaded")
    }

    pub async fn preview(
        pool: &PgPool,
        tenant_id: Uuid,
        mark_sheet_id: Uuid,
        import_id: Uuid,
        page: i64,
        per_page: i64,
    ) -> Result<Option<GradebookMarkImportPreview>> {
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
            SELECT preview.id,preview.import_id,mapping.version,mapping.mapping,
                   preview.ready_rows,preview.invalid_rows,preview.duplicate_rows,preview.created_at
              FROM data_import_previews AS preview
              JOIN data_import_mappings AS mapping
                ON mapping.tenant_id=preview.tenant_id AND mapping.id=preview.mapping_id
              JOIN gradebook_mark_imports AS link
                ON link.tenant_id=preview.tenant_id AND link.import_id=preview.import_id
             WHERE preview.tenant_id=$1 AND link.mark_sheet_id=$2 AND preview.import_id=$3
             ORDER BY mapping.version DESC LIMIT 1
            "#,
        )
        .bind(tenant_id)
        .bind(mark_sheet_id)
        .bind(import_id)
        .fetch_optional(pool)
        .await
        .context("Failed to load mark import preview")?;
        let Some(header) = header else {
            return Ok(None);
        };
        let rows = sqlx::query_as::<_, GradebookMarkImportPreviewRow>(
            r#"
            SELECT id,row_number,canonical_data,outcome,issues,duplicate_record_id
              FROM data_import_preview_rows
             WHERE tenant_id=$1 AND preview_id=$2
             ORDER BY row_number LIMIT $3 OFFSET $4
            "#,
        )
        .bind(tenant_id)
        .bind(header.id)
        .bind(per_page)
        .bind((page - 1) * per_page)
        .fetch_all(pool)
        .await
        .context("Failed to load mark import preview rows")?;
        let total_rows = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM data_import_preview_rows WHERE tenant_id=$1 AND preview_id=$2",
        )
        .bind(tenant_id)
        .bind(header.id)
        .fetch_one(pool)
        .await
        .context("Failed to count mark import preview rows")?;
        let mapping = serde_json::from_value(header.mapping)
            .context("Stored mark import mapping is invalid")?;
        Ok(Some(GradebookMarkImportPreview {
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

    #[allow(clippy::too_many_arguments)]
    pub async fn commit(
        pool: &PgPool,
        tenant_id: Uuid,
        mark_sheet_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        import_id: Uuid,
        preview_id: Uuid,
    ) -> Result<GradebookMarkImportCommit> {
        let actor_id = person_actor_id(actor)?;
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to start mark import commit")?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!("{tenant_id}:gradebook:{mark_sheet_id}:mark-import"))
            .execute(&mut *transaction)
            .await
            .context("Failed to serialize mark import commit")?;
        if let Some(existing) =
            load_commit(&mut transaction, tenant_id, import_id, preview_id).await?
        {
            transaction
                .commit()
                .await
                .context("Failed to finish mark import lookup")?;
            return Ok(existing);
        }
        #[derive(FromRow)]
        struct CommitHeader {
            sheet_version: i32,
            expected_sheet_version: i32,
        }
        let header = sqlx::query_as::<_, CommitHeader>(
            r#"
            SELECT sheet.version AS sheet_version,
                   (mapping.mapping->>'expected_sheet_version')::INTEGER AS expected_sheet_version
              FROM data_imports AS import
              JOIN gradebook_mark_imports AS link
                ON link.tenant_id=import.tenant_id AND link.import_id=import.id
              JOIN assessment_mark_sheets AS sheet
                ON sheet.tenant_id=link.tenant_id AND sheet.id=link.mark_sheet_id
              JOIN data_import_previews AS preview
                ON preview.tenant_id=import.tenant_id AND preview.import_id=import.id
              JOIN data_import_mappings AS mapping
                ON mapping.tenant_id=preview.tenant_id AND mapping.id=preview.mapping_id
             WHERE import.tenant_id=$1 AND link.mark_sheet_id=$2 AND import.id=$3
               AND preview.id=$4 AND import.status='preview_ready'
               AND sheet.status='draft' AND sheet.deleted_at IS NULL
               AND preview.mapping_id=(
                    SELECT latest.id FROM data_import_mappings AS latest
                     WHERE latest.tenant_id=import.tenant_id AND latest.import_id=import.id
                     ORDER BY latest.version DESC LIMIT 1
               )
             FOR UPDATE OF import,sheet
            "#,
        )
        .bind(tenant_id)
        .bind(mark_sheet_id)
        .bind(import_id)
        .bind(preview_id)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to lock mark import preview")?
        .context("The selected mark preview is not available for commit")?;
        if header.sheet_version != header.expected_sheet_version {
            bail!("This mark sheet changed. Create a new preview before committing");
        }
        let rows = sqlx::query_as::<_, GradebookMarkImportPreviewRow>(
            "SELECT id,row_number,canonical_data,outcome,issues,duplicate_record_id FROM data_import_preview_rows WHERE tenant_id=$1 AND preview_id=$2 ORDER BY row_number",
        )
        .bind(tenant_id)
        .bind(preview_id)
        .fetch_all(&mut *transaction)
        .await
        .context("Failed to load rows for mark import commit")?;
        let commit_id = Uuid::new_v4();
        let mut updated_rows = 0;
        let mut skipped_rows = 0;
        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            match row.outcome.as_str() {
                "ready" => {
                    let value: ImportedMark = serde_json::from_value(row.canonical_data)
                        .context("Stored mark preview is invalid")?;
                    let changed = sqlx::query_scalar::<_, Uuid>(
                        r#"
                        UPDATE assessment_marks
                           SET mark_status=$1,marks_awarded_hundredths=$2,note=$3,
                               marked_by=CASE WHEN $1='unmarked' THEN NULL ELSE $4 END,
                               marked_at=CASE WHEN $1='unmarked' THEN NULL ELSE NOW() END,
                               version=version+1
                         WHERE tenant_id=$5 AND mark_sheet_id=$6 AND id=$7
                           AND learner_id=$8 AND version=$9 AND deleted_at IS NULL
                        RETURNING id
                        "#,
                    )
                    .bind(&value.mark_status)
                    .bind(value.marks_awarded_hundredths)
                    .bind(&value.note)
                    .bind(actor_id)
                    .bind(tenant_id)
                    .bind(mark_sheet_id)
                    .bind(value.mark_id)
                    .bind(value.learner_id)
                    .bind(value.mark_version)
                    .fetch_optional(&mut *transaction)
                    .await
                    .context("Failed to apply a previewed mark")?
                    .context("A previewed mark changed. Create a new preview before committing")?;
                    updated_rows += 1;
                    results.push((row.id, "updated", Some(changed), json!([])));
                }
                "duplicate" => {
                    skipped_rows += 1;
                    results.push((
                        row.id,
                        "skipped_duplicate",
                        row.duplicate_record_id,
                        row.issues,
                    ));
                }
                "invalid" => {
                    skipped_rows += 1;
                    results.push((row.id, "rejected_validation", None, row.issues));
                }
                _ => bail!("Stored mark preview outcome is invalid"),
            }
        }
        if updated_rows == 0 {
            bail!("The selected preview has no ready mark updates");
        }
        let sheet_version = sqlx::query_scalar::<_, i32>(
            "UPDATE assessment_mark_sheets SET version=version+1 WHERE tenant_id=$1 AND id=$2 AND status='draft' AND version=$3 RETURNING version",
        )
        .bind(tenant_id)
        .bind(mark_sheet_id)
        .bind(header.sheet_version)
        .fetch_optional(&mut *transaction)
        .await
        .context("Failed to version the imported mark sheet")?
        .context("This mark sheet changed. Create a new preview before committing")?;
        let committed_at = Utc::now();
        sqlx::query(
            r#"
            INSERT INTO data_import_commits (
                id,tenant_id,import_id,preview_id,created_rows,updated_rows,
                skipped_rows,failed_rows,requested_by,committed_at
            ) VALUES ($1,$2,$3,$4,0,$5,$6,0,$7,$8)
            "#,
        )
        .bind(commit_id)
        .bind(tenant_id)
        .bind(import_id)
        .bind(preview_id)
        .bind(updated_rows)
        .bind(skipped_rows)
        .bind(actor_id)
        .bind(committed_at)
        .execute(&mut *transaction)
        .await
        .context("Failed to save mark import commit")?;
        for (preview_row_id, outcome, record_id, issues) in results {
            sqlx::query(
                "INSERT INTO data_import_row_results (tenant_id,commit_id,preview_row_id,outcome,record_id,issues) VALUES ($1,$2,$3,$4,$5,$6)",
            )
            .bind(tenant_id)
            .bind(commit_id)
            .bind(preview_row_id)
            .bind(outcome)
            .bind(record_id)
            .bind(issues)
            .execute(&mut *transaction)
            .await
            .context("Failed to save mark import row result")?;
        }
        sqlx::query("UPDATE data_imports SET status='committed' WHERE tenant_id=$1 AND id=$2")
            .bind(tenant_id)
            .bind(import_id)
            .execute(&mut *transaction)
            .await
            .context("Failed to mark mark import committed")?;
        sqlx::query(
            r#"
            INSERT INTO assessment_mark_sheet_events (
                tenant_id,mark_sheet_id,event_type,from_status,to_status,
                mark_sheet_version,actor_id,metadata
            ) VALUES ($1,$2,'marks_imported','draft','draft',$3,$4,$5)
            "#,
        )
        .bind(tenant_id)
        .bind(mark_sheet_id)
        .bind(sheet_version)
        .bind(actor_id)
        .bind(json!({
            "import_id": import_id, "preview_id": preview_id,
            "updated_rows": updated_rows, "skipped_rows": skipped_rows
        }))
        .execute(&mut *transaction)
        .await
        .context("Failed to append mark import history")?;
        append_import_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "academics.gradebook.mark_imports.commit",
            mark_sheet_id,
            json!({
                "import_id": import_id, "preview_id": preview_id,
                "updated_rows": updated_rows, "skipped_rows": skipped_rows,
                "mark_sheet_version": sheet_version
            }),
        )
        .await?;
        transaction
            .commit()
            .await
            .context("Failed to commit imported marks")?;
        Ok(GradebookMarkImportCommit {
            id: commit_id,
            import_id,
            preview_id,
            updated_rows,
            skipped_rows,
            failed_rows: 0,
            committed_at,
        })
    }
}

fn prepare_rows(
    mapping: &GradebookMarkImportMapping,
    table: &SourceTable,
    sheet: &crate::GradebookSheetResponse,
) -> Result<Vec<PreparedPreviewRow>> {
    let roster = sheet
        .marks
        .iter()
        .map(|mark| (mark.learner_number.trim().to_lowercase(), mark))
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    Ok(table
        .rows
        .iter()
        .map(|row| {
            let source_data = source_json(table, row);
            let learner_number = mapped(table, row, mapping, "learner_number").map(str::to_string);
            let mut issues = Vec::new();
            if learner_number.is_none() {
                issues.push("Learner number is required.".to_string());
            }
            let mark = learner_number
                .as_ref()
                .and_then(|number| roster.get(&number.to_lowercase()).copied());
            if learner_number.is_some() && mark.is_none() {
                issues.push("The learner number is not in this mark-sheet roster.".to_string());
            }
            let repeated = learner_number
                .as_ref()
                .is_some_and(|number| !seen.insert(number.trim().to_lowercase()));
            let value = mark.and_then(|mark| {
                map_mark(
                    table,
                    row,
                    mapping,
                    sheet.summary.maximum_marks,
                    mark,
                    &mut issues,
                )
            });
            let unchanged = value.as_ref().is_some_and(|value| {
                mark.is_some_and(|current| {
                    current.mark_status == value.mark_status
                        && current.marks_awarded_hundredths == value.marks_awarded_hundredths
                        && current.note == value.note
                })
            });
            let outcome = if !issues.is_empty() {
                "invalid"
            } else if repeated || unchanged {
                issues.push(if repeated {
                    "This learner appears earlier in the file.".to_string()
                } else {
                    "The imported value matches the current mark.".to_string()
                });
                "duplicate"
            } else {
                "ready"
            };
            PreparedPreviewRow {
                row_number: row.row_number as i32,
                source_data,
                canonical_data: value.map_or_else(
                    || json!({ "learner_number": learner_number }),
                    |value| json!(value),
                ),
                outcome,
                issues,
                duplicate_record_id: (repeated || unchanged)
                    .then(|| mark.map(|value| value.id))
                    .flatten(),
            }
        })
        .collect())
}

fn map_mark(
    table: &SourceTable,
    row: &SourceRow,
    mapping: &GradebookMarkImportMapping,
    maximum_marks: i32,
    mark: &crate::GradebookMarkResponse,
    issues: &mut Vec<String>,
) -> Option<ImportedMark> {
    let mark_text = mapped(table, row, mapping, "mark");
    let raw_status = mapped(table, row, mapping, "status");
    let status = raw_status.map(normalize_status).unwrap_or_else(|| {
        if mark_text.is_some() {
            "scored".to_string()
        } else {
            String::new()
        }
    });
    if !matches!(status.as_str(), "unmarked" | "scored" | "absent" | "exempt") {
        issues.push("Status must be scored, absent, exempt, or unmarked.".to_string());
    }
    let awarded = if status == "scored" {
        match mark_text {
            Some(value) => parse_hundredths(value, mapping.decimal_separator, issues),
            None => {
                issues.push("A scored learner requires a mark.".to_string());
                None
            }
        }
    } else {
        if mark_text.is_some() {
            issues.push("Only a scored learner may have a numeric mark.".to_string());
        }
        None
    };
    if awarded.is_some_and(|value| value > i64::from(maximum_marks) * 100) {
        issues.push(format!("The mark cannot exceed {maximum_marks}."));
    }
    let note = mapped(table, row, mapping, "note").map(str::to_string);
    if note.as_ref().is_some_and(|value| value.len() > 1000) {
        issues.push("The note is longer than 1,000 characters.".to_string());
    }
    if status == "unmarked" && note.is_some() {
        issues.push("An unmarked learner cannot have a note.".to_string());
    }
    if !issues.is_empty() {
        return None;
    }
    Some(ImportedMark {
        mark_id: mark.id,
        mark_version: mark.version,
        learner_id: mark.learner_id,
        learner_number: mark.learner_number.clone(),
        learner_name: mark.learner_name.clone(),
        mark_status: status,
        marks_awarded_hundredths: awarded,
        note,
    })
}

fn validate_mapping(mapping: &GradebookMarkImportMapping, headers: &[String]) -> Result<()> {
    if mapping.expected_sheet_version < 1 {
        bail!("A current mark-sheet version is required");
    }
    let allowed = HashSet::from(["learner_number", "mark", "status", "note"]);
    if mapping
        .columns
        .keys()
        .any(|key| !allowed.contains(key.as_str()))
    {
        bail!("The mapping contains an unsupported mark field");
    }
    if mapping
        .columns
        .get("learner_number")
        .is_none_or(|value| value.trim().is_empty())
    {
        bail!("Map the required learner number field");
    }
    if !mapping.columns.contains_key("mark") && !mapping.columns.contains_key("status") {
        bail!("Map a mark or status field");
    }
    let available = headers.iter().collect::<HashSet<_>>();
    if mapping
        .columns
        .values()
        .any(|header| !available.contains(header))
    {
        bail!("A mapped source column is not present in this file");
    }
    let mut unique = HashSet::new();
    if mapping
        .columns
        .values()
        .any(|header| !unique.insert(header))
    {
        bail!("Each source column can map to only one Gradebook field");
    }
    Ok(())
}

fn parse_hundredths(
    value: &str,
    separator: MarkImportDecimalSeparator,
    issues: &mut Vec<String>,
) -> Option<i64> {
    let value = value.trim();
    let canonical = match separator {
        MarkImportDecimalSeparator::Dot => {
            if value.contains(',') {
                None
            } else {
                Some(value.to_string())
            }
        }
        MarkImportDecimalSeparator::Comma => {
            if value.contains('.') {
                None
            } else {
                Some(value.replace(',', "."))
            }
        }
    };
    let parsed = canonical.and_then(|value| {
        let (whole, fraction) = value.split_once('.').unwrap_or((&value, ""));
        if whole.is_empty()
            || !whole.bytes().all(|byte| byte.is_ascii_digit())
            || fraction.len() > 2
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        let whole = whole.parse::<i64>().ok()?;
        let fraction = match fraction.len() {
            0 => 0,
            1 => fraction.parse::<i64>().ok()? * 10,
            _ => fraction.parse::<i64>().ok()?,
        };
        whole.checked_mul(100)?.checked_add(fraction)
    });
    if parsed.is_none() {
        issues.push(
            "Mark must be a non-negative number with at most two decimal places.".to_string(),
        );
    }
    parsed
}

fn mapped<'a>(
    table: &'a SourceTable,
    row: &'a SourceRow,
    mapping: &GradebookMarkImportMapping,
    key: &str,
) -> Option<&'a str> {
    mapping
        .columns
        .get(key)
        .and_then(|header| table.value(row, header))
        .filter(|value| !value.is_empty())
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

fn count_outcome(rows: &[PreparedPreviewRow], outcome: &str) -> i32 {
    rows.iter().filter(|row| row.outcome == outcome).count() as i32
}

fn import_select() -> &'static str {
    r#"
    SELECT import.id,link.mark_sheet_id,import.file_name,import.content_type,
           import.source_format,import.source_size_bytes,import.source_row_count,
           import.source_headers,import.status,import.created_at,
           latest.preview_id AS latest_preview_id,latest.mapping_version,
           latest.ready_rows,latest.invalid_rows,latest.duplicate_rows,
           commit.updated_rows,commit.skipped_rows,commit.failed_rows,commit.committed_at
      FROM data_imports AS import
      JOIN gradebook_mark_imports AS link
        ON link.tenant_id=import.tenant_id AND link.import_id=import.id
      LEFT JOIN LATERAL (
          SELECT preview.id AS preview_id,mapping.version AS mapping_version,
                 preview.ready_rows,preview.invalid_rows,preview.duplicate_rows
            FROM data_import_previews AS preview
            JOIN data_import_mappings AS mapping
              ON mapping.tenant_id=preview.tenant_id AND mapping.id=preview.mapping_id
           WHERE preview.tenant_id=import.tenant_id AND preview.import_id=import.id
           ORDER BY mapping.version DESC LIMIT 1
      ) AS latest ON TRUE
      LEFT JOIN data_import_commits AS commit
        ON commit.tenant_id=import.tenant_id AND commit.preview_id=latest.preview_id
    "#
}

async fn load_commit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    import_id: Uuid,
    preview_id: Uuid,
) -> Result<Option<GradebookMarkImportCommit>> {
    sqlx::query_as::<_, GradebookMarkImportCommit>(
        r#"
        SELECT id,import_id,preview_id,updated_rows,skipped_rows,failed_rows,committed_at
          FROM data_import_commits
         WHERE tenant_id=$1 AND import_id=$2 AND preview_id=$3
        "#,
    )
    .bind(tenant_id)
    .bind(import_id)
    .bind(preview_id)
    .fetch_optional(&mut **transaction)
    .await
    .context("Failed to load mark import commit")
}

#[allow(clippy::too_many_arguments)]
async fn append_import_audit(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    action: &str,
    mark_sheet_id: Uuid,
    metadata: Value,
) -> Result<()> {
    append_audit(
        &mut **transaction,
        &NewAuditEvent::new(
            tenant_id,
            actor,
            action,
            AuditOutcome::Succeeded,
            request_context,
        )
        .with_target(AuditTarget::new(
            "assessment_mark_sheet",
            mark_sheet_id.to_string(),
        ))
        .with_redacted_metadata(metadata.as_object().cloned().unwrap_or_default()),
    )
    .await
    .context("Failed to audit mark import operation")?;
    Ok(())
}

fn person_actor_id(actor: AuditActor) -> Result<Uuid> {
    actor
        .user_id()
        .context("Mark imports require an authenticated account")
}

fn safe_file_name(value: &str) -> Result<&str> {
    let value = value.trim();
    if value.is_empty() || value.len() > 255 || value.contains(['/', '\\']) {
        bail!("The import filename is invalid");
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use cp_imports::parse_source;

    use super::{
        GradebookMarkImportMapping, MarkImportDecimalSeparator, parse_hundredths, validate_mapping,
    };

    #[test]
    fn mark_mapping_requires_roster_identity_and_a_value_column() {
        let table = parse_source("marks.csv", b"learner,score\nSTU-1,12.5\n")
            .unwrap_or_else(|error| panic!("fixture should parse: {error}"))
            .table;
        let invalid = GradebookMarkImportMapping {
            columns: BTreeMap::from([("learner_number".to_string(), "learner".to_string())]),
            decimal_separator: MarkImportDecimalSeparator::Dot,
            expected_sheet_version: 1,
        };
        assert!(validate_mapping(&invalid, &table.headers).is_err());
        let valid = GradebookMarkImportMapping {
            columns: BTreeMap::from([
                ("learner_number".to_string(), "learner".to_string()),
                ("mark".to_string(), "score".to_string()),
            ]),
            ..invalid
        };
        assert!(validate_mapping(&valid, &table.headers).is_ok());
    }

    #[test]
    fn decimal_parser_keeps_exact_hundredths() {
        let mut issues = Vec::new();
        assert_eq!(
            parse_hundredths("12.5", MarkImportDecimalSeparator::Dot, &mut issues),
            Some(1_250)
        );
        assert!(issues.is_empty());
        assert_eq!(
            parse_hundredths("12,05", MarkImportDecimalSeparator::Comma, &mut issues),
            Some(1_205)
        );
        assert!(issues.is_empty());
        assert_eq!(
            parse_hundredths("12.005", MarkImportDecimalSeparator::Dot, &mut issues),
            None
        );
        assert_eq!(issues.len(), 1);
    }
}
