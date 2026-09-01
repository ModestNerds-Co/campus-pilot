-- Versioned E-learning assignments, learner submissions, feedback, and progress evidence.
--
-- SIS remains authoritative for learner identity and enrolment. Learning stores
-- an immutable recipient snapshot and text-only submitted attempts; file
-- submissions require a separate governed retention design.

ALTER TABLE learning_activity_events
    DROP CONSTRAINT IF EXISTS learning_activity_events_aggregate_type_check;
ALTER TABLE learning_activity_events
    ADD CONSTRAINT learning_activity_events_aggregate_type_check
    CHECK (aggregate_type IN (
        'settings', 'space', 'unit', 'resource', 'assignment', 'submission', 'review'
    ));

CREATE TABLE IF NOT EXISTS learning_assignments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    learning_unit_id UUID NOT NULL,
    position INTEGER NOT NULL CHECK (position > 0),
    title TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(title)) BETWEEN 1 AND 200),
    instructions TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(instructions)) BETWEEN 1 AND 20000),
    due_at TIMESTAMPTZ NOT NULL,
    max_score_hundredths INTEGER NOT NULL CHECK (max_score_hundredths > 0),
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
    FOREIGN KEY (learning_unit_id, tenant_id)
        REFERENCES learning_units(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (published_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (closed_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'draft' AND published_by IS NULL AND published_at IS NULL
            AND closed_by IS NULL AND closed_at IS NULL AND close_reason IS NULL)
        OR (status = 'published' AND published_by IS NOT NULL AND published_at IS NOT NULL
            AND closed_by IS NULL AND closed_at IS NULL AND close_reason IS NULL)
        OR (status = 'closed' AND published_by IS NOT NULL AND published_at IS NOT NULL
            AND closed_by IS NOT NULL AND closed_at IS NOT NULL AND close_reason IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_learning_assignments_position
    ON learning_assignments(tenant_id, learning_unit_id, position)
    WHERE deleted_at IS NULL AND status <> 'closed';
CREATE INDEX IF NOT EXISTS idx_learning_assignments_worklist
    ON learning_assignments(tenant_id, learning_unit_id, status, due_at, position)
    WHERE deleted_at IS NULL;
DROP TRIGGER IF EXISTS update_learning_assignments_updated_at ON learning_assignments;
CREATE TRIGGER update_learning_assignments_updated_at
    BEFORE UPDATE ON learning_assignments
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS learning_assignment_rubric_criteria (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    learning_assignment_id UUID NOT NULL,
    position INTEGER NOT NULL CHECK (position > 0),
    title TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(title)) BETWEEN 1 AND 200),
    description TEXT CHECK (description IS NULL OR CHAR_LENGTH(BTRIM(description)) <= 4000),
    max_score_hundredths INTEGER NOT NULL CHECK (max_score_hundredths > 0),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    deleted_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    FOREIGN KEY (learning_assignment_id, tenant_id)
        REFERENCES learning_assignments(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (deleted_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK ((deleted_at IS NULL AND deleted_by IS NULL) OR (deleted_at IS NOT NULL AND deleted_by IS NOT NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_learning_rubric_position
    ON learning_assignment_rubric_criteria(tenant_id, learning_assignment_id, position)
    WHERE deleted_at IS NULL;
DROP TRIGGER IF EXISTS update_learning_rubric_updated_at ON learning_assignment_rubric_criteria;
CREATE TRIGGER update_learning_rubric_updated_at
    BEFORE UPDATE ON learning_assignment_rubric_criteria
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS learning_assignment_recipients (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    learning_assignment_id UUID NOT NULL,
    enrolment_id UUID NOT NULL,
    learner_id UUID NOT NULL,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, learning_assignment_id, enrolment_id),
    UNIQUE (tenant_id, learning_assignment_id, learner_id),
    FOREIGN KEY (learning_assignment_id, tenant_id)
        REFERENCES learning_assignments(id, tenant_id),
    FOREIGN KEY (enrolment_id, tenant_id) REFERENCES enrolments(id, tenant_id),
    FOREIGN KEY (learner_id, tenant_id) REFERENCES learners(id, tenant_id)
);

CREATE INDEX IF NOT EXISTS idx_learning_assignment_recipients_learner
    ON learning_assignment_recipients(tenant_id, learner_id, learning_assignment_id);

CREATE TABLE IF NOT EXISTS learning_submissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    learning_assignment_id UUID NOT NULL,
    assignment_recipient_id UUID NOT NULL,
    draft_body TEXT CHECK (draft_body IS NULL OR CHAR_LENGTH(draft_body) <= 20000),
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'submitted', 'revision_requested', 'graded')),
    current_submission_version_id UUID,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    first_submitted_at TIMESTAMPTZ,
    last_submitted_at TIMESTAMPTZ,
    graded_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, learning_assignment_id, assignment_recipient_id),
    FOREIGN KEY (learning_assignment_id, tenant_id)
        REFERENCES learning_assignments(id, tenant_id),
    FOREIGN KEY (assignment_recipient_id, tenant_id)
        REFERENCES learning_assignment_recipients(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'draft' AND graded_at IS NULL AND (
            (current_submission_version_id IS NULL
                AND first_submitted_at IS NULL AND last_submitted_at IS NULL)
            OR (current_submission_version_id IS NOT NULL
                AND first_submitted_at IS NOT NULL AND last_submitted_at IS NOT NULL)
        ))
        OR (status IN ('submitted', 'revision_requested')
            AND current_submission_version_id IS NOT NULL
            AND first_submitted_at IS NOT NULL AND last_submitted_at IS NOT NULL
            AND graded_at IS NULL)
        OR (status = 'graded' AND current_submission_version_id IS NOT NULL
            AND first_submitted_at IS NOT NULL AND last_submitted_at IS NOT NULL
            AND graded_at IS NOT NULL)
    )
);

DROP TRIGGER IF EXISTS update_learning_submissions_updated_at ON learning_submissions;
CREATE TRIGGER update_learning_submissions_updated_at
    BEFORE UPDATE ON learning_submissions
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS learning_submission_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    learning_submission_id UUID NOT NULL,
    revision_number INTEGER NOT NULL CHECK (revision_number > 0),
    body_snapshot TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(body_snapshot)) BETWEEN 1 AND 20000),
    submitted_by UUID NOT NULL,
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    late_snapshot BOOLEAN NOT NULL,
    idempotency_key UUID NOT NULL,
    request_fingerprint TEXT NOT NULL CHECK (request_fingerprint ~ '^[0-9a-f]{64}$'),
    UNIQUE (id, tenant_id),
    UNIQUE (id, learning_submission_id, tenant_id),
    UNIQUE (tenant_id, learning_submission_id, revision_number),
    UNIQUE (tenant_id, learning_submission_id, idempotency_key),
    FOREIGN KEY (learning_submission_id, tenant_id)
        REFERENCES learning_submissions(id, tenant_id),
    FOREIGN KEY (submitted_by, tenant_id) REFERENCES users(id, tenant_id)
);

ALTER TABLE learning_submissions
    ADD CONSTRAINT learning_submissions_current_version_fk
    FOREIGN KEY (current_submission_version_id, id, tenant_id)
    REFERENCES learning_submission_versions(id, learning_submission_id, tenant_id);

CREATE TABLE IF NOT EXISTS learning_submission_reviews (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    submission_version_id UUID NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'released')),
    outcome TEXT CHECK (outcome IS NULL OR outcome IN ('graded', 'revision_requested')),
    overall_feedback TEXT CHECK (
        overall_feedback IS NULL OR CHAR_LENGTH(BTRIM(overall_feedback)) BETWEEN 1 AND 10000
    ),
    total_score_hundredths INTEGER CHECK (total_score_hundredths IS NULL OR total_score_hundredths >= 0),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    reviewed_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    released_by UUID,
    released_at TIMESTAMPTZ,
    release_idempotency_key UUID,
    release_request_fingerprint TEXT CHECK (
        release_request_fingerprint IS NULL OR release_request_fingerprint ~ '^[0-9a-f]{64}$'
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, submission_version_id),
    UNIQUE (tenant_id, release_idempotency_key),
    FOREIGN KEY (submission_version_id, tenant_id)
        REFERENCES learning_submission_versions(id, tenant_id),
    FOREIGN KEY (reviewed_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (released_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'draft' AND outcome IS NULL AND total_score_hundredths IS NULL
            AND released_by IS NULL AND released_at IS NULL
            AND release_idempotency_key IS NULL AND release_request_fingerprint IS NULL)
        OR (status = 'released' AND outcome IS NOT NULL
            AND released_by IS NOT NULL AND released_at IS NOT NULL
            AND release_idempotency_key IS NOT NULL AND release_request_fingerprint IS NOT NULL
            AND (
                (outcome = 'graded' AND total_score_hundredths IS NOT NULL)
                OR (outcome = 'revision_requested' AND total_score_hundredths IS NULL
                    AND overall_feedback IS NOT NULL)
            ))
    )
);

DROP TRIGGER IF EXISTS update_learning_submission_reviews_updated_at ON learning_submission_reviews;
CREATE TRIGGER update_learning_submission_reviews_updated_at
    BEFORE UPDATE ON learning_submission_reviews
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS learning_submission_review_scores (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    review_id UUID NOT NULL,
    rubric_criterion_id UUID NOT NULL,
    earned_score_hundredths INTEGER NOT NULL CHECK (earned_score_hundredths >= 0),
    feedback TEXT CHECK (feedback IS NULL OR CHAR_LENGTH(BTRIM(feedback)) <= 4000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, review_id, rubric_criterion_id),
    FOREIGN KEY (review_id, tenant_id)
        REFERENCES learning_submission_reviews(id, tenant_id) ON DELETE CASCADE,
    FOREIGN KEY (rubric_criterion_id, tenant_id)
        REFERENCES learning_assignment_rubric_criteria(id, tenant_id)
);

DROP TRIGGER IF EXISTS update_learning_review_scores_updated_at ON learning_submission_review_scores;
CREATE TRIGGER update_learning_review_scores_updated_at
    BEFORE UPDATE ON learning_submission_review_scores
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE OR REPLACE FUNCTION reject_learning_snapshot_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Learning snapshot evidence is append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS learning_recipients_append_only ON learning_assignment_recipients;
CREATE TRIGGER learning_recipients_append_only
    BEFORE UPDATE OR DELETE ON learning_assignment_recipients
    FOR EACH ROW EXECUTE FUNCTION reject_learning_snapshot_mutation();
DROP TRIGGER IF EXISTS learning_submission_versions_append_only ON learning_submission_versions;
CREATE TRIGGER learning_submission_versions_append_only
    BEFORE UPDATE OR DELETE ON learning_submission_versions
    FOR EACH ROW EXECUTE FUNCTION reject_learning_snapshot_mutation();

CREATE OR REPLACE FUNCTION protect_released_learning_review()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.status = 'released' THEN
        RAISE EXCEPTION 'Released Learning feedback is immutable';
    END IF;
    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS learning_released_review_immutable ON learning_submission_reviews;
CREATE TRIGGER learning_released_review_immutable
    BEFORE UPDATE OR DELETE ON learning_submission_reviews
    FOR EACH ROW EXECUTE FUNCTION protect_released_learning_review();

CREATE OR REPLACE FUNCTION protect_released_learning_review_score()
RETURNS TRIGGER AS $$
DECLARE
    target_review_id UUID;
    target_status TEXT;
BEGIN
    IF TG_OP = 'DELETE' THEN
        target_review_id := OLD.review_id;
    ELSE
        target_review_id := NEW.review_id;
    END IF;
    SELECT status INTO target_status
      FROM learning_submission_reviews
     WHERE id = target_review_id;
    IF target_status = 'released' THEN
        RAISE EXCEPTION 'Released Learning feedback scores are immutable';
    END IF;
    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS learning_released_review_scores_immutable ON learning_submission_review_scores;
CREATE TRIGGER learning_released_review_scores_immutable
    BEFORE INSERT OR UPDATE OR DELETE ON learning_submission_review_scores
    FOR EACH ROW EXECUTE FUNCTION protect_released_learning_review_score();

UPDATE roles
SET permissions = ARRAY(
        SELECT DISTINCT permission
          FROM UNNEST(permissions || ARRAY['learning:participate']::TEXT[]) AS expanded(permission)
         ORDER BY permission
    ),
    updated_at = NOW()
WHERE key = 'student' AND deleted_at IS NULL;

CREATE OR REPLACE FUNCTION grant_new_tenant_learning_permissions()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE roles
    SET permissions = ARRAY(
            SELECT DISTINCT permission
            FROM UNNEST(
                permissions || CASE key
                    WHEN 'teacher' THEN ARRAY['learning:view', 'learning:teach']::TEXT[]
                    WHEN 'student' THEN ARRAY['learning:view', 'learning:participate']::TEXT[]
                    WHEN 'academic_manager' THEN ARRAY['learning:view', 'learning:teach', 'learning:manage']::TEXT[]
                    ELSE ARRAY[]::TEXT[]
                END
            ) AS expanded(permission)
            ORDER BY permission
        ),
        updated_at = NOW()
    WHERE tenant_id = NEW.id
      AND key IN ('teacher', 'student', 'academic_manager')
      AND deleted_at IS NULL;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS ev_learning_assignments ON learning_assignments;
CREATE TRIGGER ev_learning_assignments
    AFTER INSERT OR UPDATE OR DELETE ON learning_assignments
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_learning_assignment_rubric ON learning_assignment_rubric_criteria;
CREATE TRIGGER ev_learning_assignment_rubric
    AFTER INSERT OR UPDATE OR DELETE ON learning_assignment_rubric_criteria
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_learning_assignment_recipients ON learning_assignment_recipients;
CREATE TRIGGER ev_learning_assignment_recipients
    AFTER INSERT ON learning_assignment_recipients
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_learning_submissions ON learning_submissions;
CREATE TRIGGER ev_learning_submissions
    AFTER INSERT OR UPDATE OR DELETE ON learning_submissions
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_learning_submission_versions ON learning_submission_versions;
CREATE TRIGGER ev_learning_submission_versions
    AFTER INSERT ON learning_submission_versions
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_learning_submission_reviews ON learning_submission_reviews;
CREATE TRIGGER ev_learning_submission_reviews
    AFTER INSERT OR UPDATE OR DELETE ON learning_submission_reviews
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_learning_review_scores ON learning_submission_review_scores;
CREATE TRIGGER ev_learning_review_scores
    AFTER INSERT OR UPDATE OR DELETE ON learning_submission_review_scores
    FOR EACH ROW EXECUTE FUNCTION log_event();
