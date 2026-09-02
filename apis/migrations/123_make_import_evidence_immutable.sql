-- Enforce the append-only evidence contract shared by every staged import.
--
-- The import header may advance from uploaded to preview_ready to committed,
-- but mappings, previews, normalized rows, commits, and row results are frozen.

CREATE OR REPLACE FUNCTION prevent_data_import_evidence_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Data import evidence is immutable';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER data_import_mappings_immutable
    BEFORE UPDATE OR DELETE ON data_import_mappings
    FOR EACH ROW EXECUTE FUNCTION prevent_data_import_evidence_mutation();

CREATE TRIGGER data_import_previews_immutable
    BEFORE UPDATE OR DELETE ON data_import_previews
    FOR EACH ROW EXECUTE FUNCTION prevent_data_import_evidence_mutation();

CREATE TRIGGER data_import_preview_rows_immutable
    BEFORE UPDATE OR DELETE ON data_import_preview_rows
    FOR EACH ROW EXECUTE FUNCTION prevent_data_import_evidence_mutation();

CREATE TRIGGER data_import_commits_immutable
    BEFORE UPDATE OR DELETE ON data_import_commits
    FOR EACH ROW EXECUTE FUNCTION prevent_data_import_evidence_mutation();

CREATE TRIGGER data_import_row_results_immutable
    BEFORE UPDATE OR DELETE ON data_import_row_results
    FOR EACH ROW EXECUTE FUNCTION prevent_data_import_evidence_mutation();
