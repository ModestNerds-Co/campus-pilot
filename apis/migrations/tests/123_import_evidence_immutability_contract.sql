-- Shared staged-import evidence must remain append-only.

\set ON_ERROR_STOP on

BEGIN;

DO $$
DECLARE
    expected_triggers TEXT[] := ARRAY[
        'data_import_mappings_immutable',
        'data_import_previews_immutable',
        'data_import_preview_rows_immutable',
        'data_import_commits_immutable',
        'data_import_row_results_immutable'
    ];
    trigger_name TEXT;
BEGIN
    FOREACH trigger_name IN ARRAY expected_triggers LOOP
        IF NOT EXISTS (
            SELECT 1
              FROM pg_trigger AS trigger
              JOIN pg_proc AS function ON function.oid = trigger.tgfoid
             WHERE trigger.tgname = trigger_name
               AND NOT trigger.tgisinternal
               AND trigger.tgenabled <> 'D'
               AND function.proname = 'prevent_data_import_evidence_mutation'
               AND function.prosrc LIKE '%Data import evidence is immutable%'
        ) THEN
            RAISE EXCEPTION 'Missing immutable import-evidence trigger: %', trigger_name;
        END IF;
    END LOOP;
END;
$$;

ROLLBACK;

SELECT 'Import evidence immutability contract passed' AS result;
