-- Governed Learning submission-file contract. Run after migration 121.

DO $$
DECLARE
    attachment_guard_count INTEGER;
    version_guard_count INTEGER;
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
         WHERE table_name = 'learning_settings'
           AND column_name = 'learner_submission_series_id'
    ) OR NOT EXISTS (
        SELECT 1 FROM information_schema.columns
         WHERE table_name = 'learning_assignments'
           AND column_name = 'submission_method'
    ) THEN
        RAISE EXCEPTION 'Learning submission-file configuration columns are missing';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conname = 'learning_settings_submission_series_fk'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conname = 'learning_assignments_submission_method_check'
    ) THEN
        RAISE EXCEPTION 'Learning submission-file configuration constraints are missing';
    END IF;

    IF TO_REGCLASS('public.learning_submission_attachments') IS NULL
       OR TO_REGCLASS('public.learning_submission_version_files') IS NULL THEN
        RAISE EXCEPTION 'Learning submission-file evidence tables are missing';
    END IF;

    SELECT COUNT(*) INTO attachment_guard_count
      FROM pg_trigger
     WHERE tgrelid = 'learning_submission_attachments'::REGCLASS
       AND tgname = 'learning_submission_attachment_guard'
       AND NOT tgisinternal;
    SELECT COUNT(*) INTO version_guard_count
      FROM pg_trigger
     WHERE tgrelid = 'learning_submission_version_files'::REGCLASS
       AND tgname = 'learning_submission_version_file_guard'
       AND NOT tgisinternal;
    IF attachment_guard_count <> 1 OR version_guard_count <> 1 THEN
        RAISE EXCEPTION 'Learning submission-file lifecycle guards are missing';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgrelid = 'learning_assignments'::REGCLASS
           AND tgname = 'learning_assignment_lifecycle_guard'
           AND NOT tgisinternal
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgrelid = 'learning_assignment_rubric_criteria'::REGCLASS
           AND tgname = 'learning_rubric_draft_only'
           AND NOT tgisinternal
    ) THEN
        RAISE EXCEPTION 'Learning assignment publication guards are missing';
    END IF;

    IF EXISTS (
        SELECT 1 FROM information_schema.columns
         WHERE table_name = 'learning_submission_versions'
           AND column_name = 'body_snapshot'
           AND is_nullable <> 'YES'
    ) THEN
        RAISE EXCEPTION 'File-only Learning submission versions cannot omit text';
    END IF;
END;
$$;

SELECT 'Governed Learning submission-file contract passed' AS result;
