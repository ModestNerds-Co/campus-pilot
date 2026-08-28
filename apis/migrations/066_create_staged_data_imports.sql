-- Retained, versioned import sources, immutable previews, and row results.
--
-- Source bytes are intentionally private database data: the current object
-- bucket is publicly readable and must never receive learner or staff imports.

CREATE TABLE data_imports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    module_key TEXT NOT NULL,
    entity_key TEXT NOT NULL,
    file_name TEXT NOT NULL,
    content_type TEXT NOT NULL,
    source_format TEXT NOT NULL CHECK (source_format IN ('csv', 'xlsx')),
    source_sha256 TEXT NOT NULL CHECK (LENGTH(source_sha256) = 64),
    source_bytes BYTEA NOT NULL,
    source_size_bytes BIGINT NOT NULL CHECK (
        source_size_bytes > 0
        AND source_size_bytes <= 5242880
        AND source_size_bytes = OCTET_LENGTH(source_bytes)
    ),
    source_row_count INTEGER NOT NULL CHECK (source_row_count >= 0 AND source_row_count <= 5000),
    source_headers TEXT[] NOT NULL CHECK (CARDINALITY(source_headers) > 0),
    status TEXT NOT NULL DEFAULT 'uploaded'
        CHECK (status IN ('uploaded', 'preview_ready', 'committed')),
    created_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE INDEX idx_data_imports_tenant_module_created
    ON data_imports(tenant_id, module_key, created_at DESC);

CREATE TABLE data_import_mappings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    import_id UUID NOT NULL,
    version INTEGER NOT NULL CHECK (version > 0),
    mapping JSONB NOT NULL,
    created_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    UNIQUE (import_id, version),
    FOREIGN KEY (import_id, tenant_id) REFERENCES data_imports(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE TABLE data_import_previews (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    import_id UUID NOT NULL,
    mapping_id UUID NOT NULL,
    ready_rows INTEGER NOT NULL CHECK (ready_rows >= 0),
    invalid_rows INTEGER NOT NULL CHECK (invalid_rows >= 0),
    duplicate_rows INTEGER NOT NULL CHECK (duplicate_rows >= 0),
    created_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    UNIQUE (mapping_id),
    FOREIGN KEY (import_id, tenant_id) REFERENCES data_imports(id, tenant_id),
    FOREIGN KEY (mapping_id, tenant_id) REFERENCES data_import_mappings(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE TABLE data_import_preview_rows (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    preview_id UUID NOT NULL,
    row_number INTEGER NOT NULL CHECK (row_number >= 2),
    source_data JSONB NOT NULL,
    canonical_data JSONB NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('ready', 'invalid', 'duplicate')),
    issues JSONB NOT NULL DEFAULT '[]'::JSONB,
    duplicate_record_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    UNIQUE (preview_id, row_number),
    FOREIGN KEY (preview_id, tenant_id) REFERENCES data_import_previews(id, tenant_id)
);

CREATE INDEX idx_data_import_preview_rows_preview
    ON data_import_preview_rows(tenant_id, preview_id, row_number);

CREATE TABLE data_import_commits (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    import_id UUID NOT NULL,
    preview_id UUID NOT NULL,
    created_rows INTEGER NOT NULL CHECK (created_rows >= 0),
    skipped_rows INTEGER NOT NULL CHECK (skipped_rows >= 0),
    failed_rows INTEGER NOT NULL CHECK (failed_rows >= 0),
    requested_by UUID,
    committed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    UNIQUE (preview_id),
    FOREIGN KEY (import_id, tenant_id) REFERENCES data_imports(id, tenant_id),
    FOREIGN KEY (preview_id, tenant_id) REFERENCES data_import_previews(id, tenant_id),
    FOREIGN KEY (requested_by, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE TABLE data_import_row_results (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    commit_id UUID NOT NULL,
    preview_row_id UUID NOT NULL,
    outcome TEXT NOT NULL
        CHECK (outcome IN ('created', 'skipped_duplicate', 'rejected_validation', 'failed')),
    record_id UUID,
    issues JSONB NOT NULL DEFAULT '[]'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (commit_id, preview_row_id),
    FOREIGN KEY (commit_id, tenant_id) REFERENCES data_import_commits(id, tenant_id),
    FOREIGN KEY (preview_row_id, tenant_id) REFERENCES data_import_preview_rows(id, tenant_id)
);

DROP TRIGGER IF EXISTS update_data_imports_updated_at ON data_imports;
CREATE TRIGGER update_data_imports_updated_at
    BEFORE UPDATE ON data_imports
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();
