-- Learning assignment, submission-history, and feedback contract checks.

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM roles
         WHERE key = 'student' AND deleted_at IS NULL
           AND 'learning:participate' = ANY(permissions)
    ) THEN
        RAISE EXCEPTION 'Student participation permission is missing';
    END IF;

    IF EXISTS (
        SELECT 1 FROM roles
         WHERE key = 'teacher' AND deleted_at IS NULL
           AND 'learning:participate' = ANY(permissions)
    ) THEN
        RAISE EXCEPTION 'Teacher received learner self-service authority';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conname = 'learning_submissions_current_version_fk'
    ) THEN
        RAISE EXCEPTION 'Current submission version ownership is not constrained';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname = 'learning_submission_versions_append_only' AND NOT tgisinternal
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname = 'learning_released_review_immutable' AND NOT tgisinternal
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname = 'learning_released_review_scores_immutable' AND NOT tgisinternal
    ) THEN
        RAISE EXCEPTION 'Learning submission or released-feedback immutability is missing';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes
         WHERE indexname = 'idx_learning_assignments_position'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_indexes
         WHERE indexname = 'idx_learning_rubric_position'
    ) THEN
        RAISE EXCEPTION 'Learning assignment ordering constraints are missing';
    END IF;

    IF NOT EXISTS (
        SELECT 1
          FROM information_schema.check_constraints
         WHERE constraint_name = 'learning_activity_events_aggregate_type_check'
           AND check_clause LIKE '%assignment%'
           AND check_clause LIKE '%submission%'
           AND check_clause LIKE '%review%'
    ) THEN
        RAISE EXCEPTION 'Learning activity vocabulary was not extended';
    END IF;
END;
$$;

SELECT 'Learning assignment workflow contract passed' AS result;
