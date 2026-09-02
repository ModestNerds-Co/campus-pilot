-- Learning quiz, immutable-attempt, and completion-policy contract checks.

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conname = 'learning_quiz_attempt_answer_choice_fk'
    ) THEN
        RAISE EXCEPTION 'Quiz answers are not constrained to choices from their question';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname = 'learning_quiz_attempt_answers_immutable' AND NOT tgisinternal
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname = 'learning_quiz_attempts_immutable' AND NOT tgisinternal
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname = 'learning_quiz_attempts_validate_start' AND NOT tgisinternal
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname = 'learning_quiz_lifecycle_immutable' AND NOT tgisinternal
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname = 'learning_quiz_questions_require_draft' AND NOT tgisinternal
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname = 'learning_quiz_choices_require_draft' AND NOT tgisinternal
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname = 'learning_quiz_recipients_append_only' AND NOT tgisinternal
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname = 'learning_quiz_recipients_require_draft' AND NOT tgisinternal
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname = 'learning_completion_recipients_append_only' AND NOT tgisinternal
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname = 'learning_completion_recipients_require_draft' AND NOT tgisinternal
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname = 'learning_completion_requirements_immutable' AND NOT tgisinternal
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname = 'learning_completion_policy_lifecycle_immutable' AND NOT tgisinternal
    ) THEN
        RAISE EXCEPTION 'Learning attempt or completion immutability is missing';
    END IF;

    IF NOT EXISTS (
        SELECT 1
          FROM pg_constraint
         WHERE conrelid = 'learning_quiz_attempts'::regclass
           AND contype = 'f'
           AND pg_get_constraintdef(oid) LIKE '%(quiz_recipient_id, learning_quiz_id, tenant_id)%'
    ) THEN
        RAISE EXCEPTION 'Quiz attempts are not constrained to a recipient from the same quiz';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes WHERE indexname = 'idx_learning_quizzes_position'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_indexes WHERE indexname = 'idx_learning_completion_policy_published'
    ) THEN
        RAISE EXCEPTION 'Learning quiz ordering or active completion-policy constraints are missing';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.check_constraints
         WHERE constraint_name = 'learning_activity_events_aggregate_type_check'
           AND check_clause LIKE '%quiz_attempt%'
           AND check_clause LIKE '%completion_policy%'
    ) THEN
        RAISE EXCEPTION 'Learning activity vocabulary was not extended for E-learning';
    END IF;
END;
$$;

SELECT 'Learning quizzes and completion contract passed' AS result;
