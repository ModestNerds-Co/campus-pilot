-- Attach staged CSV/XLSX imports to one existing Gradebook mark sheet.
--
-- Shared import tables retain the private source and immutable previews.
-- Gradebook owns the destination link and records updates rather than creates.

CREATE TABLE gradebook_mark_imports (
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    import_id UUID NOT NULL,
    mark_sheet_id UUID NOT NULL,
    created_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, import_id),
    CONSTRAINT gradebook_mark_imports_import_tenant_fkey
        FOREIGN KEY (import_id, tenant_id)
        REFERENCES data_imports(id, tenant_id),
    CONSTRAINT gradebook_mark_imports_sheet_tenant_fkey
        FOREIGN KEY (mark_sheet_id, tenant_id)
        REFERENCES assessment_mark_sheets(id, tenant_id),
    CONSTRAINT gradebook_mark_imports_creator_tenant_fkey
        FOREIGN KEY (created_by, tenant_id)
        REFERENCES users(id, tenant_id)
);

CREATE INDEX idx_gradebook_mark_imports_sheet
    ON gradebook_mark_imports(tenant_id, mark_sheet_id, created_at DESC);

CREATE OR REPLACE FUNCTION prevent_gradebook_mark_import_link_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Gradebook mark import links are immutable';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER gradebook_mark_import_links_immutable
    BEFORE UPDATE OR DELETE ON gradebook_mark_imports
    FOR EACH ROW EXECUTE FUNCTION prevent_gradebook_mark_import_link_mutation();

ALTER TABLE data_import_commits
    ADD COLUMN updated_rows INTEGER NOT NULL DEFAULT 0
        CHECK (updated_rows >= 0);

ALTER TABLE data_import_row_results
    DROP CONSTRAINT IF EXISTS data_import_row_results_outcome_check;
ALTER TABLE data_import_row_results
    ADD CONSTRAINT data_import_row_results_outcome_check CHECK (
        outcome IN (
            'created', 'updated', 'skipped_duplicate',
            'rejected_validation', 'failed'
        )
    );

ALTER TABLE assessment_mark_sheet_events
    DROP CONSTRAINT IF EXISTS assessment_mark_sheet_events_event_type_check;
ALTER TABLE assessment_mark_sheet_events
    ADD CONSTRAINT assessment_mark_sheet_events_event_type_check CHECK (
        event_type IN (
            'created', 'marks_updated', 'marks_imported', 'submitted',
            'published', 'reopened', 'deleted'
        )
    );

