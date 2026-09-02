-- Gradebook mark-import storage contract. Every mutation is rolled back.

\set ON_ERROR_STOP on

BEGIN;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
          FROM information_schema.columns
         WHERE table_schema = current_schema()
           AND table_name = 'data_import_commits'
           AND column_name = 'updated_rows'
           AND is_nullable = 'NO'
    ) THEN
        RAISE EXCEPTION 'updated-row import accounting is missing';
    END IF;

    IF NOT EXISTS (
        SELECT 1
          FROM pg_trigger
         WHERE tgname = 'gradebook_mark_import_links_immutable'
           AND NOT tgisinternal
    ) THEN
        RAISE EXCEPTION 'Gradebook import links are not immutable';
    END IF;

    IF NOT EXISTS (
        SELECT 1
          FROM pg_constraint
         WHERE conname = 'data_import_row_results_outcome_check'
           AND pg_get_constraintdef(oid) LIKE '%updated%'
    ) THEN
        RAISE EXCEPTION 'updated import row results are not accepted';
    END IF;

    IF NOT EXISTS (
        SELECT 1
          FROM pg_constraint
         WHERE conname = 'assessment_mark_sheet_events_event_type_check'
           AND pg_get_constraintdef(oid) LIKE '%marks_imported%'
    ) THEN
        RAISE EXCEPTION 'mark-sheet import lifecycle evidence is unavailable';
    END IF;
END;
$$;

ROLLBACK;

SELECT 'Gradebook mark imports contract passed' AS result;

