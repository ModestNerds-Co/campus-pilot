-- Governed E-learning quizzes, immutable learner attempts, and completion rules.
--
-- SIS remains authoritative for learner identity and enrolment. Academics
-- remains authoritative for formal marks. Learning freezes eligible rosters,
-- attempt evidence, and versioned completion requirements at publication.

ALTER TABLE learning_activity_events
    DROP CONSTRAINT IF EXISTS learning_activity_events_aggregate_type_check;
ALTER TABLE learning_activity_events
    ADD CONSTRAINT learning_activity_events_aggregate_type_check
    CHECK (aggregate_type IN (
        'settings', 'space', 'unit', 'resource', 'assignment', 'submission', 'review',
        'quiz', 'quiz_question', 'quiz_attempt', 'completion_policy'
    ));

CREATE TABLE IF NOT EXISTS learning_quizzes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    learning_unit_id UUID NOT NULL,
    position INTEGER NOT NULL CHECK (position > 0),
    title TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(title)) BETWEEN 1 AND 200),
    instructions TEXT CHECK (instructions IS NULL OR CHAR_LENGTH(BTRIM(instructions)) <= 10000),
    opens_at TIMESTAMPTZ,
    closes_at TIMESTAMPTZ,
    attempt_limit INTEGER NOT NULL DEFAULT 1 CHECK (attempt_limit BETWEEN 1 AND 10),
    pass_score_basis_points INTEGER NOT NULL DEFAULT 5000
        CHECK (pass_score_basis_points BETWEEN 0 AND 10000),
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'published', 'closed')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    published_by UUID,
    published_at TIMESTAMPTZ,
    closed_by UUID,
    closed_at TIMESTAMPTZ,
    close_reason TEXT CHECK (
        close_reason IS NULL OR CHAR_LENGTH(BTRIM(close_reason)) BETWEEN 1 AND 2000
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    FOREIGN KEY (learning_unit_id, tenant_id) REFERENCES learning_units(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (published_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (closed_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (opens_at IS NULL OR closes_at IS NULL OR opens_at < closes_at),
    CHECK (
        (status = 'draft' AND published_by IS NULL AND published_at IS NULL
            AND closed_by IS NULL AND closed_at IS NULL AND close_reason IS NULL)
        OR (status = 'published' AND published_by IS NOT NULL AND published_at IS NOT NULL
            AND closed_by IS NULL AND closed_at IS NULL AND close_reason IS NULL)
        OR (status = 'closed' AND published_by IS NOT NULL AND published_at IS NOT NULL
            AND closed_by IS NOT NULL AND closed_at IS NOT NULL AND close_reason IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_learning_quizzes_position
    ON learning_quizzes(tenant_id, learning_unit_id, position)
    WHERE deleted_at IS NULL AND status <> 'closed';
CREATE INDEX IF NOT EXISTS idx_learning_quizzes_worklist
    ON learning_quizzes(tenant_id, learning_unit_id, status, opens_at, closes_at, position)
    WHERE deleted_at IS NULL;
DROP TRIGGER IF EXISTS update_learning_quizzes_updated_at ON learning_quizzes;
CREATE TRIGGER update_learning_quizzes_updated_at
    BEFORE UPDATE ON learning_quizzes
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE OR REPLACE FUNCTION protect_learning_quiz_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Learning quizzes cannot be deleted';
    END IF;

    IF OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
        OR OLD.learning_unit_id IS DISTINCT FROM NEW.learning_unit_id
        OR OLD.created_by IS DISTINCT FROM NEW.created_by
        OR OLD.created_at IS DISTINCT FROM NEW.created_at THEN
        RAISE EXCEPTION 'Learning quiz identity is immutable';
    END IF;

    IF OLD.status = 'draft' THEN
        IF NEW.status NOT IN ('draft', 'published') THEN
            RAISE EXCEPTION 'A draft Learning quiz may only be published';
        END IF;
        IF NEW.status = 'published' AND (
            NOT EXISTS (
                SELECT 1 FROM learning_quiz_recipients AS recipient
                 WHERE recipient.tenant_id = NEW.tenant_id
                   AND recipient.learning_quiz_id = NEW.id
            )
            OR NOT EXISTS (
                SELECT 1 FROM learning_quiz_questions AS question
                 WHERE question.tenant_id = NEW.tenant_id
                   AND question.learning_quiz_id = NEW.id
                   AND question.deleted_at IS NULL
            )
            OR EXISTS (
                SELECT 1
                  FROM learning_quiz_questions AS question
                 WHERE question.tenant_id = NEW.tenant_id
                   AND question.learning_quiz_id = NEW.id
                   AND question.deleted_at IS NULL
                   AND (
                       (SELECT COUNT(*) FROM learning_quiz_choices AS choice
                         WHERE choice.tenant_id = question.tenant_id
                           AND choice.learning_quiz_question_id = question.id
                           AND choice.deleted_at IS NULL) NOT BETWEEN 2 AND 8
                       OR (SELECT COUNT(*) FROM learning_quiz_choices AS choice
                            WHERE choice.tenant_id = question.tenant_id
                              AND choice.learning_quiz_question_id = question.id
                              AND choice.deleted_at IS NULL
                              AND choice.is_correct) <> 1
                   )
            )
        ) THEN
            RAISE EXCEPTION 'A Learning quiz needs recipients and a valid answer key before publication';
        END IF;
        RETURN NEW;
    END IF;

    IF OLD.position IS DISTINCT FROM NEW.position
        OR OLD.title IS DISTINCT FROM NEW.title
        OR OLD.instructions IS DISTINCT FROM NEW.instructions
        OR OLD.opens_at IS DISTINCT FROM NEW.opens_at
        OR OLD.closes_at IS DISTINCT FROM NEW.closes_at
        OR OLD.attempt_limit IS DISTINCT FROM NEW.attempt_limit
        OR OLD.pass_score_basis_points IS DISTINCT FROM NEW.pass_score_basis_points
        OR OLD.published_by IS DISTINCT FROM NEW.published_by
        OR OLD.published_at IS DISTINCT FROM NEW.published_at
        OR OLD.deleted_at IS DISTINCT FROM NEW.deleted_at THEN
        RAISE EXCEPTION 'Published Learning quiz definitions are immutable';
    END IF;

    IF OLD.status = 'published' AND NEW.status = 'closed' THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'Published or closed Learning quizzes are immutable';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS learning_quiz_lifecycle_immutable ON learning_quizzes;
CREATE TRIGGER learning_quiz_lifecycle_immutable
    BEFORE UPDATE OR DELETE ON learning_quizzes
    FOR EACH ROW EXECUTE FUNCTION protect_learning_quiz_lifecycle();

CREATE TABLE IF NOT EXISTS learning_quiz_questions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    learning_quiz_id UUID NOT NULL,
    position INTEGER NOT NULL CHECK (position > 0),
    prompt TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(prompt)) BETWEEN 1 AND 4000),
    points INTEGER NOT NULL DEFAULT 1 CHECK (points BETWEEN 1 AND 1000),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    deleted_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    FOREIGN KEY (learning_quiz_id, tenant_id) REFERENCES learning_quizzes(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (deleted_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK ((deleted_at IS NULL AND deleted_by IS NULL) OR (deleted_at IS NOT NULL AND deleted_by IS NOT NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_learning_quiz_questions_position
    ON learning_quiz_questions(tenant_id, learning_quiz_id, position)
    WHERE deleted_at IS NULL;
DROP TRIGGER IF EXISTS update_learning_quiz_questions_updated_at ON learning_quiz_questions;
CREATE TRIGGER update_learning_quiz_questions_updated_at
    BEFORE UPDATE ON learning_quiz_questions
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS learning_quiz_choices (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    learning_quiz_question_id UUID NOT NULL,
    position INTEGER NOT NULL CHECK (position > 0),
    label TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(label)) BETWEEN 1 AND 1000),
    is_correct BOOLEAN NOT NULL DEFAULT FALSE,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    deleted_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    UNIQUE (id, learning_quiz_question_id, tenant_id),
    FOREIGN KEY (learning_quiz_question_id, tenant_id)
        REFERENCES learning_quiz_questions(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (deleted_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK ((deleted_at IS NULL AND deleted_by IS NULL) OR (deleted_at IS NOT NULL AND deleted_by IS NOT NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_learning_quiz_choices_position
    ON learning_quiz_choices(tenant_id, learning_quiz_question_id, position)
    WHERE deleted_at IS NULL;
DROP TRIGGER IF EXISTS update_learning_quiz_choices_updated_at ON learning_quiz_choices;
CREATE TRIGGER update_learning_quiz_choices_updated_at
    BEFORE UPDATE ON learning_quiz_choices
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE OR REPLACE FUNCTION require_draft_learning_quiz_definition()
RETURNS TRIGGER AS $$
DECLARE
    target_tenant_id UUID;
    target_quiz_id UUID;
    target_status TEXT;
BEGIN
    target_tenant_id := CASE WHEN TG_OP = 'DELETE' THEN OLD.tenant_id ELSE NEW.tenant_id END;
    IF TG_TABLE_NAME = 'learning_quiz_questions' THEN
        target_quiz_id := CASE WHEN TG_OP = 'DELETE' THEN OLD.learning_quiz_id ELSE NEW.learning_quiz_id END;
        IF TG_OP = 'UPDATE' AND (
            OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
            OR OLD.learning_quiz_id IS DISTINCT FROM NEW.learning_quiz_id
            OR OLD.created_by IS DISTINCT FROM NEW.created_by
            OR OLD.created_at IS DISTINCT FROM NEW.created_at
        ) THEN
            RAISE EXCEPTION 'Learning quiz question identity is immutable';
        END IF;
    ELSE
        SELECT question.learning_quiz_id
          INTO target_quiz_id
          FROM learning_quiz_questions AS question
         WHERE question.tenant_id = target_tenant_id
           AND question.id = CASE
               WHEN TG_OP = 'DELETE' THEN OLD.learning_quiz_question_id
               ELSE NEW.learning_quiz_question_id
           END;
        IF TG_OP = 'UPDATE' AND (
            OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
            OR OLD.learning_quiz_question_id IS DISTINCT FROM NEW.learning_quiz_question_id
            OR OLD.created_by IS DISTINCT FROM NEW.created_by
            OR OLD.created_at IS DISTINCT FROM NEW.created_at
        ) THEN
            RAISE EXCEPTION 'Learning quiz choice identity is immutable';
        END IF;
    END IF;

    SELECT status INTO target_status
      FROM learning_quizzes
     WHERE tenant_id = target_tenant_id AND id = target_quiz_id;
    IF target_status IS DISTINCT FROM 'draft' THEN
        RAISE EXCEPTION 'Published Learning quiz questions and answer keys are immutable';
    END IF;
    IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS learning_quiz_questions_require_draft ON learning_quiz_questions;
CREATE TRIGGER learning_quiz_questions_require_draft
    BEFORE INSERT OR UPDATE OR DELETE ON learning_quiz_questions
    FOR EACH ROW EXECUTE FUNCTION require_draft_learning_quiz_definition();
DROP TRIGGER IF EXISTS learning_quiz_choices_require_draft ON learning_quiz_choices;
CREATE TRIGGER learning_quiz_choices_require_draft
    BEFORE INSERT OR UPDATE OR DELETE ON learning_quiz_choices
    FOR EACH ROW EXECUTE FUNCTION require_draft_learning_quiz_definition();

CREATE TABLE IF NOT EXISTS learning_quiz_recipients (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    learning_quiz_id UUID NOT NULL,
    enrolment_id UUID NOT NULL,
    learner_id UUID NOT NULL,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    UNIQUE (id, learning_quiz_id, tenant_id),
    UNIQUE (tenant_id, learning_quiz_id, enrolment_id),
    UNIQUE (tenant_id, learning_quiz_id, learner_id),
    FOREIGN KEY (learning_quiz_id, tenant_id) REFERENCES learning_quizzes(id, tenant_id),
    FOREIGN KEY (enrolment_id, tenant_id) REFERENCES enrolments(id, tenant_id),
    FOREIGN KEY (learner_id, tenant_id) REFERENCES learners(id, tenant_id)
);

CREATE INDEX IF NOT EXISTS idx_learning_quiz_recipients_learner
    ON learning_quiz_recipients(tenant_id, learner_id, learning_quiz_id);

CREATE TABLE IF NOT EXISTS learning_quiz_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    learning_quiz_id UUID NOT NULL,
    quiz_recipient_id UUID NOT NULL,
    attempt_number INTEGER NOT NULL CHECK (attempt_number > 0),
    status TEXT NOT NULL DEFAULT 'in_progress'
        CHECK (status IN ('in_progress', 'submitted')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    started_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    submitted_at TIMESTAMPTZ,
    total_points_snapshot INTEGER,
    earned_points_snapshot INTEGER,
    score_basis_points INTEGER,
    passed BOOLEAN,
    idempotency_key UUID,
    request_fingerprint TEXT CHECK (
        request_fingerprint IS NULL OR request_fingerprint ~ '^[0-9a-f]{64}$'
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, learning_quiz_id, quiz_recipient_id, attempt_number),
    UNIQUE (tenant_id, idempotency_key),
    FOREIGN KEY (learning_quiz_id, tenant_id) REFERENCES learning_quizzes(id, tenant_id),
    FOREIGN KEY (quiz_recipient_id, learning_quiz_id, tenant_id)
        REFERENCES learning_quiz_recipients(id, learning_quiz_id, tenant_id),
    FOREIGN KEY (started_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'in_progress' AND submitted_at IS NULL
            AND total_points_snapshot IS NULL AND earned_points_snapshot IS NULL
            AND score_basis_points IS NULL AND passed IS NULL
            AND idempotency_key IS NULL AND request_fingerprint IS NULL)
        OR (status = 'submitted' AND submitted_at IS NOT NULL
            AND total_points_snapshot > 0
            AND earned_points_snapshot BETWEEN 0 AND total_points_snapshot
            AND score_basis_points BETWEEN 0 AND 10000 AND passed IS NOT NULL
            AND idempotency_key IS NOT NULL AND request_fingerprint IS NOT NULL)
    )
);

DROP TRIGGER IF EXISTS update_learning_quiz_attempts_updated_at ON learning_quiz_attempts;
CREATE TRIGGER update_learning_quiz_attempts_updated_at
    BEFORE UPDATE ON learning_quiz_attempts
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE OR REPLACE FUNCTION validate_learning_quiz_attempt_start()
RETURNS TRIGGER AS $$
DECLARE
    quiz_status TEXT;
    quiz_opens_at TIMESTAMPTZ;
    quiz_closes_at TIMESTAMPTZ;
    quiz_attempt_limit INTEGER;
    expected_attempt_number INTEGER;
BEGIN
    SELECT status, opens_at, closes_at, attempt_limit
      INTO quiz_status, quiz_opens_at, quiz_closes_at, quiz_attempt_limit
      FROM learning_quizzes
     WHERE tenant_id = NEW.tenant_id AND id = NEW.learning_quiz_id;
    SELECT COUNT(*)::INTEGER + 1
      INTO expected_attempt_number
      FROM learning_quiz_attempts
     WHERE tenant_id = NEW.tenant_id
       AND learning_quiz_id = NEW.learning_quiz_id
       AND quiz_recipient_id = NEW.quiz_recipient_id;
    IF quiz_status IS DISTINCT FROM 'published'
        OR (quiz_opens_at IS NOT NULL AND quiz_opens_at > NOW())
        OR (quiz_closes_at IS NOT NULL AND quiz_closes_at <= NOW())
        OR NEW.attempt_number > quiz_attempt_limit
        OR NEW.attempt_number IS DISTINCT FROM expected_attempt_number THEN
        RAISE EXCEPTION 'The Learning quiz is not open for this attempt';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS learning_quiz_attempts_validate_start ON learning_quiz_attempts;
CREATE TRIGGER learning_quiz_attempts_validate_start
    BEFORE INSERT ON learning_quiz_attempts
    FOR EACH ROW EXECUTE FUNCTION validate_learning_quiz_attempt_start();

CREATE OR REPLACE FUNCTION protect_learning_quiz_attempt_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' OR OLD.status = 'submitted' THEN
        RAISE EXCEPTION 'Submitted Learning quiz attempts are immutable';
    END IF;
    IF OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
        OR OLD.learning_quiz_id IS DISTINCT FROM NEW.learning_quiz_id
        OR OLD.quiz_recipient_id IS DISTINCT FROM NEW.quiz_recipient_id
        OR OLD.attempt_number IS DISTINCT FROM NEW.attempt_number
        OR OLD.started_by IS DISTINCT FROM NEW.started_by
        OR OLD.started_at IS DISTINCT FROM NEW.started_at
        OR OLD.created_at IS DISTINCT FROM NEW.created_at THEN
        RAISE EXCEPTION 'Learning quiz attempt identity is immutable';
    END IF;
    IF OLD.status = 'in_progress' AND NEW.status IN ('in_progress', 'submitted') THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'Learning quiz attempt lifecycle is invalid';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS learning_quiz_attempts_immutable ON learning_quiz_attempts;
CREATE TRIGGER learning_quiz_attempts_immutable
    BEFORE UPDATE OR DELETE ON learning_quiz_attempts
    FOR EACH ROW EXECUTE FUNCTION protect_learning_quiz_attempt_lifecycle();

CREATE TABLE IF NOT EXISTS learning_quiz_attempt_answers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    learning_quiz_attempt_id UUID NOT NULL,
    learning_quiz_question_id UUID NOT NULL,
    selected_choice_id UUID NOT NULL,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, learning_quiz_attempt_id, learning_quiz_question_id),
    FOREIGN KEY (learning_quiz_attempt_id, tenant_id)
        REFERENCES learning_quiz_attempts(id, tenant_id),
    FOREIGN KEY (learning_quiz_question_id, tenant_id)
        REFERENCES learning_quiz_questions(id, tenant_id),
    CONSTRAINT learning_quiz_attempt_answer_choice_fk
    FOREIGN KEY (selected_choice_id, learning_quiz_question_id, tenant_id)
        REFERENCES learning_quiz_choices(id, learning_quiz_question_id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id)
);

DROP TRIGGER IF EXISTS update_learning_quiz_attempt_answers_updated_at ON learning_quiz_attempt_answers;
CREATE TRIGGER update_learning_quiz_attempt_answers_updated_at
    BEFORE UPDATE ON learning_quiz_attempt_answers
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE OR REPLACE FUNCTION protect_learning_quiz_attempt_answer()
RETURNS TRIGGER AS $$
DECLARE
    target_attempt_id UUID;
    target_status TEXT;
BEGIN
    target_attempt_id := CASE WHEN TG_OP = 'DELETE' THEN OLD.learning_quiz_attempt_id ELSE NEW.learning_quiz_attempt_id END;
    SELECT status INTO target_status
      FROM learning_quiz_attempts
     WHERE tenant_id = CASE WHEN TG_OP = 'DELETE' THEN OLD.tenant_id ELSE NEW.tenant_id END
       AND id = target_attempt_id;
    IF target_status = 'submitted' THEN
        RAISE EXCEPTION 'Submitted Learning quiz attempts are immutable';
    END IF;
    IF TG_OP = 'UPDATE' AND (
        OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
        OR OLD.learning_quiz_attempt_id IS DISTINCT FROM NEW.learning_quiz_attempt_id
        OR OLD.learning_quiz_question_id IS DISTINCT FROM NEW.learning_quiz_question_id
        OR OLD.created_by IS DISTINCT FROM NEW.created_by
        OR OLD.created_at IS DISTINCT FROM NEW.created_at
    ) THEN
        RAISE EXCEPTION 'Learning quiz answer identity is immutable';
    END IF;
    IF TG_OP <> 'DELETE' AND NOT EXISTS (
        SELECT 1
          FROM learning_quiz_attempts AS attempt
          JOIN learning_quiz_questions AS question
            ON question.tenant_id = attempt.tenant_id
           AND question.learning_quiz_id = attempt.learning_quiz_id
         WHERE attempt.tenant_id = NEW.tenant_id
           AND attempt.id = NEW.learning_quiz_attempt_id
           AND question.id = NEW.learning_quiz_question_id
           AND question.deleted_at IS NULL
    ) THEN
        RAISE EXCEPTION 'A Learning quiz answer must belong to the attempt quiz';
    END IF;
    IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS learning_quiz_attempt_answers_immutable ON learning_quiz_attempt_answers;
CREATE TRIGGER learning_quiz_attempt_answers_immutable
    BEFORE INSERT OR UPDATE OR DELETE ON learning_quiz_attempt_answers
    FOR EACH ROW EXECUTE FUNCTION protect_learning_quiz_attempt_answer();

CREATE TABLE IF NOT EXISTS learning_completion_policies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    learning_space_id UUID NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'published', 'superseded')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    published_by UUID,
    published_at TIMESTAMPTZ,
    superseded_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (learning_space_id, tenant_id) REFERENCES learning_spaces(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (published_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'draft' AND published_by IS NULL AND published_at IS NULL AND superseded_at IS NULL)
        OR (status = 'published' AND published_by IS NOT NULL AND published_at IS NOT NULL AND superseded_at IS NULL)
        OR (status = 'superseded' AND published_by IS NOT NULL AND published_at IS NOT NULL AND superseded_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_learning_completion_policy_draft
    ON learning_completion_policies(tenant_id, learning_space_id)
    WHERE status = 'draft';
CREATE UNIQUE INDEX IF NOT EXISTS idx_learning_completion_policy_published
    ON learning_completion_policies(tenant_id, learning_space_id)
    WHERE status = 'published';
DROP TRIGGER IF EXISTS update_learning_completion_policies_updated_at ON learning_completion_policies;
CREATE TRIGGER update_learning_completion_policies_updated_at
    BEFORE UPDATE ON learning_completion_policies
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE OR REPLACE FUNCTION protect_learning_completion_policy_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Learning completion policies cannot be deleted';
    END IF;
    IF OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
        OR OLD.learning_space_id IS DISTINCT FROM NEW.learning_space_id
        OR OLD.created_by IS DISTINCT FROM NEW.created_by
        OR OLD.created_at IS DISTINCT FROM NEW.created_at THEN
        RAISE EXCEPTION 'Learning completion policy identity is immutable';
    END IF;
    IF OLD.status = 'draft' AND NEW.status IN ('draft', 'published') THEN
        IF NEW.status = 'published' AND (
            NOT EXISTS (
                SELECT 1 FROM learning_completion_recipients AS recipient
                 WHERE recipient.tenant_id = NEW.tenant_id
                   AND recipient.completion_policy_id = NEW.id
            )
            OR NOT EXISTS (
                SELECT 1 FROM learning_completion_requirements AS requirement
                 WHERE requirement.tenant_id = NEW.tenant_id
                   AND requirement.completion_policy_id = NEW.id
            )
            OR EXISTS (
                SELECT 1
                  FROM learning_completion_requirements AS requirement
                 WHERE requirement.tenant_id = NEW.tenant_id
                   AND requirement.completion_policy_id = NEW.id
                   AND NOT (
                       (requirement.requirement_type = 'assignment' AND EXISTS (
                           SELECT 1
                             FROM learning_assignments AS assignment
                             JOIN learning_units AS unit
                               ON unit.id = assignment.learning_unit_id
                              AND unit.tenant_id = assignment.tenant_id
                            WHERE assignment.tenant_id = requirement.tenant_id
                              AND assignment.id = requirement.source_id
                              AND unit.learning_space_id = NEW.learning_space_id
                              AND assignment.status IN ('published', 'closed')
                              AND assignment.deleted_at IS NULL
                              AND unit.deleted_at IS NULL
                       ))
                       OR (requirement.requirement_type = 'quiz' AND EXISTS (
                           SELECT 1
                             FROM learning_quizzes AS quiz
                             JOIN learning_units AS unit
                               ON unit.id = quiz.learning_unit_id
                              AND unit.tenant_id = quiz.tenant_id
                            WHERE quiz.tenant_id = requirement.tenant_id
                              AND quiz.id = requirement.source_id
                              AND unit.learning_space_id = NEW.learning_space_id
                              AND quiz.status IN ('published', 'closed')
                              AND quiz.deleted_at IS NULL
                              AND unit.deleted_at IS NULL
                       ))
                   )
            )
        ) THEN
            RAISE EXCEPTION 'A Learning completion policy needs recipients and published activities';
        END IF;
        RETURN NEW;
    END IF;
    IF OLD.status = 'published'
        AND NEW.status = 'superseded'
        AND OLD.published_by IS NOT DISTINCT FROM NEW.published_by
        AND OLD.published_at IS NOT DISTINCT FROM NEW.published_at THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'Published Learning completion policies are immutable';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS learning_completion_policy_lifecycle_immutable ON learning_completion_policies;
CREATE TRIGGER learning_completion_policy_lifecycle_immutable
    BEFORE UPDATE OR DELETE ON learning_completion_policies
    FOR EACH ROW EXECUTE FUNCTION protect_learning_completion_policy_lifecycle();

CREATE TABLE IF NOT EXISTS learning_completion_requirements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    completion_policy_id UUID NOT NULL,
    position INTEGER NOT NULL CHECK (position > 0),
    requirement_type TEXT NOT NULL CHECK (requirement_type IN ('assignment', 'quiz')),
    source_id UUID NOT NULL,
    minimum_score_basis_points INTEGER NOT NULL DEFAULT 0
        CHECK (minimum_score_basis_points BETWEEN 0 AND 10000),
    created_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, completion_policy_id, position),
    UNIQUE (tenant_id, completion_policy_id, requirement_type, source_id),
    FOREIGN KEY (completion_policy_id, tenant_id)
        REFERENCES learning_completion_policies(id, tenant_id) ON DELETE CASCADE,
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE TABLE IF NOT EXISTS learning_completion_recipients (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    completion_policy_id UUID NOT NULL,
    enrolment_id UUID NOT NULL,
    learner_id UUID NOT NULL,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, completion_policy_id, enrolment_id),
    UNIQUE (tenant_id, completion_policy_id, learner_id),
    FOREIGN KEY (completion_policy_id, tenant_id)
        REFERENCES learning_completion_policies(id, tenant_id),
    FOREIGN KEY (enrolment_id, tenant_id) REFERENCES enrolments(id, tenant_id),
    FOREIGN KEY (learner_id, tenant_id) REFERENCES learners(id, tenant_id)
);

CREATE OR REPLACE FUNCTION protect_published_learning_completion_policy()
RETURNS TRIGGER AS $$
DECLARE
    target_policy_id UUID;
    target_status TEXT;
BEGIN
    target_policy_id := CASE WHEN TG_OP = 'DELETE' THEN OLD.completion_policy_id ELSE NEW.completion_policy_id END;
    SELECT status INTO target_status
      FROM learning_completion_policies
     WHERE tenant_id = CASE WHEN TG_OP = 'DELETE' THEN OLD.tenant_id ELSE NEW.tenant_id END
       AND id = target_policy_id;
    IF target_status IN ('published', 'superseded') THEN
        RAISE EXCEPTION 'Published Learning completion requirements are immutable';
    END IF;
    IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS learning_completion_requirements_immutable ON learning_completion_requirements;
CREATE TRIGGER learning_completion_requirements_immutable
    BEFORE INSERT OR UPDATE OR DELETE ON learning_completion_requirements
    FOR EACH ROW EXECUTE FUNCTION protect_published_learning_completion_policy();

DROP TRIGGER IF EXISTS learning_quiz_recipients_append_only ON learning_quiz_recipients;
CREATE TRIGGER learning_quiz_recipients_append_only
    BEFORE UPDATE OR DELETE ON learning_quiz_recipients
    FOR EACH ROW EXECUTE FUNCTION reject_learning_snapshot_mutation();
DROP TRIGGER IF EXISTS learning_completion_recipients_append_only ON learning_completion_recipients;
CREATE TRIGGER learning_completion_recipients_append_only
    BEFORE UPDATE OR DELETE ON learning_completion_recipients
    FOR EACH ROW EXECUTE FUNCTION reject_learning_snapshot_mutation();

CREATE OR REPLACE FUNCTION require_draft_learning_snapshot_parent()
RETURNS TRIGGER AS $$
DECLARE
    parent_status TEXT;
BEGIN
    IF TG_TABLE_NAME = 'learning_quiz_recipients' THEN
        SELECT status INTO parent_status
          FROM learning_quizzes
         WHERE tenant_id = NEW.tenant_id AND id = NEW.learning_quiz_id;
    ELSE
        SELECT status INTO parent_status
          FROM learning_completion_policies
         WHERE tenant_id = NEW.tenant_id AND id = NEW.completion_policy_id;
    END IF;
    IF parent_status IS DISTINCT FROM 'draft' THEN
        RAISE EXCEPTION 'Learning publication rosters can only be captured while drafting';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS learning_quiz_recipients_require_draft ON learning_quiz_recipients;
CREATE TRIGGER learning_quiz_recipients_require_draft
    BEFORE INSERT ON learning_quiz_recipients
    FOR EACH ROW EXECUTE FUNCTION require_draft_learning_snapshot_parent();
DROP TRIGGER IF EXISTS learning_completion_recipients_require_draft ON learning_completion_recipients;
CREATE TRIGGER learning_completion_recipients_require_draft
    BEFORE INSERT ON learning_completion_recipients
    FOR EACH ROW EXECUTE FUNCTION require_draft_learning_snapshot_parent();

DROP TRIGGER IF EXISTS ev_learning_quizzes ON learning_quizzes;
CREATE TRIGGER ev_learning_quizzes AFTER INSERT OR UPDATE OR DELETE ON learning_quizzes
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_learning_quiz_questions ON learning_quiz_questions;
CREATE TRIGGER ev_learning_quiz_questions AFTER INSERT OR UPDATE OR DELETE ON learning_quiz_questions
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_learning_quiz_choices ON learning_quiz_choices;
CREATE TRIGGER ev_learning_quiz_choices AFTER INSERT OR UPDATE OR DELETE ON learning_quiz_choices
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_learning_quiz_recipients ON learning_quiz_recipients;
CREATE TRIGGER ev_learning_quiz_recipients AFTER INSERT ON learning_quiz_recipients
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_learning_quiz_attempts ON learning_quiz_attempts;
CREATE TRIGGER ev_learning_quiz_attempts AFTER INSERT OR UPDATE ON learning_quiz_attempts
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_learning_quiz_attempt_answers ON learning_quiz_attempt_answers;
CREATE TRIGGER ev_learning_quiz_attempt_answers AFTER INSERT OR UPDATE ON learning_quiz_attempt_answers
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_learning_completion_policies ON learning_completion_policies;
CREATE TRIGGER ev_learning_completion_policies AFTER INSERT OR UPDATE ON learning_completion_policies
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_learning_completion_requirements ON learning_completion_requirements;
CREATE TRIGGER ev_learning_completion_requirements AFTER INSERT OR UPDATE OR DELETE ON learning_completion_requirements
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_learning_completion_recipients ON learning_completion_recipients;
CREATE TRIGGER ev_learning_completion_recipients AFTER INSERT ON learning_completion_recipients
    FOR EACH ROW EXECUTE FUNCTION log_event();
