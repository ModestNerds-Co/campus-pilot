-- Learning-to-Gradebook score-transfer storage contract.

\set ON_ERROR_STOP on

BEGIN;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname = 'learning_score_transfer_lifecycle_immutable'
           AND NOT tgisinternal
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname = 'learning_score_transfer_rows_immutable'
           AND NOT tgisinternal
    ) THEN
        RAISE EXCEPTION 'Learning score-transfer evidence is not immutable';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conrelid = 'learning_score_transfer_proposals'::regclass
           AND contype = 'c'
           AND pg_get_constraintdef(oid) LIKE '%reviewed_by <> proposed_by%'
    ) THEN
        RAISE EXCEPTION 'Learning score transfers do not enforce maker-checker review';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conname = 'assessment_mark_sheet_events_event_type_check'
           AND pg_get_constraintdef(oid) LIKE '%marks_transferred%'
    ) THEN
        RAISE EXCEPTION 'Gradebook score-transfer lifecycle evidence is unavailable';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.check_constraints
         WHERE constraint_name = 'learning_activity_events_aggregate_type_check'
           AND check_clause LIKE '%score_transfer%'
    ) THEN
        RAISE EXCEPTION 'Learning score-transfer activity vocabulary is unavailable';
    END IF;
END;
$$;

ROLLBACK;

SELECT 'Learning score transfers contract passed' AS result;
