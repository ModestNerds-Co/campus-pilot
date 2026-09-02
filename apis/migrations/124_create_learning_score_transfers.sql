-- Reviewed, idempotent E-learning score transfers into Academics Gradebook.
--
-- Learning owns immutable source snapshots and the human review proposal.
-- Gradebook remains the only owner allowed to change formal assessment marks.

ALTER TABLE learning_activity_events
    DROP CONSTRAINT IF EXISTS learning_activity_events_aggregate_type_check;
ALTER TABLE learning_activity_events
    ADD CONSTRAINT learning_activity_events_aggregate_type_check
    CHECK (aggregate_type IN (
        'settings', 'space', 'unit', 'resource', 'assignment', 'submission', 'review',
        'quiz', 'quiz_question', 'quiz_attempt', 'completion_policy', 'score_transfer'
    ));

ALTER TABLE assessment_mark_sheet_events
    DROP CONSTRAINT IF EXISTS assessment_mark_sheet_events_event_type_check;
ALTER TABLE assessment_mark_sheet_events
    ADD CONSTRAINT assessment_mark_sheet_events_event_type_check CHECK (
        event_type IN (
            'created', 'marks_updated', 'marks_imported', 'marks_transferred',
            'submitted', 'published', 'reopened', 'deleted'
        )
    );

CREATE TABLE learning_score_transfer_proposals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    learning_space_id UUID NOT NULL,
    source_type TEXT NOT NULL CHECK (source_type IN ('assignment', 'quiz')),
    source_id UUID NOT NULL,
    source_version INTEGER NOT NULL CHECK (source_version > 0),
    source_title_snapshot TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(source_title_snapshot)) BETWEEN 1 AND 200),
    target_mark_sheet_id UUID NOT NULL,
    target_mark_sheet_version INTEGER NOT NULL CHECK (target_mark_sheet_version > 0),
    target_maximum_marks INTEGER NOT NULL CHECK (target_maximum_marks > 0),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'applied', 'rejected')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    idempotency_key UUID NOT NULL,
    request_fingerprint TEXT NOT NULL CHECK (request_fingerprint ~ '^[0-9a-f]{64}$'),
    proposed_by UUID NOT NULL,
    proposed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reviewed_by UUID,
    reviewed_at TIMESTAMPTZ,
    review_reason TEXT CHECK (
        review_reason IS NULL OR CHAR_LENGTH(BTRIM(review_reason)) BETWEEN 1 AND 2000
    ),
    applied_mark_sheet_version INTEGER CHECK (applied_mark_sheet_version > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, idempotency_key),
    FOREIGN KEY (learning_space_id, tenant_id)
        REFERENCES learning_spaces(id, tenant_id),
    FOREIGN KEY (target_mark_sheet_id, tenant_id)
        REFERENCES assessment_mark_sheets(id, tenant_id),
    FOREIGN KEY (proposed_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (reviewed_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'pending' AND reviewed_by IS NULL AND reviewed_at IS NULL
            AND review_reason IS NULL AND applied_mark_sheet_version IS NULL)
        OR (status = 'applied' AND reviewed_by IS NOT NULL AND reviewed_at IS NOT NULL
            AND review_reason IS NULL AND applied_mark_sheet_version IS NOT NULL
            AND reviewed_by <> proposed_by)
        OR (status = 'rejected' AND reviewed_by IS NOT NULL AND reviewed_at IS NOT NULL
            AND review_reason IS NOT NULL AND applied_mark_sheet_version IS NULL
            AND reviewed_by <> proposed_by)
    )
);

CREATE INDEX idx_learning_score_transfer_worklist
    ON learning_score_transfer_proposals(tenant_id, status, proposed_at DESC, id);
CREATE INDEX idx_learning_score_transfer_space
    ON learning_score_transfer_proposals(tenant_id, learning_space_id, proposed_at DESC);
CREATE INDEX idx_learning_score_transfer_sheet
    ON learning_score_transfer_proposals(tenant_id, target_mark_sheet_id, proposed_at DESC);

DROP TRIGGER IF EXISTS update_learning_score_transfer_proposals_updated_at
    ON learning_score_transfer_proposals;
CREATE TRIGGER update_learning_score_transfer_proposals_updated_at
    BEFORE UPDATE ON learning_score_transfer_proposals
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE OR REPLACE FUNCTION protect_learning_score_transfer_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Learning score-transfer proposals cannot be deleted';
    END IF;
    IF OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
        OR OLD.learning_space_id IS DISTINCT FROM NEW.learning_space_id
        OR OLD.source_type IS DISTINCT FROM NEW.source_type
        OR OLD.source_id IS DISTINCT FROM NEW.source_id
        OR OLD.source_version IS DISTINCT FROM NEW.source_version
        OR OLD.source_title_snapshot IS DISTINCT FROM NEW.source_title_snapshot
        OR OLD.target_mark_sheet_id IS DISTINCT FROM NEW.target_mark_sheet_id
        OR OLD.target_mark_sheet_version IS DISTINCT FROM NEW.target_mark_sheet_version
        OR OLD.target_maximum_marks IS DISTINCT FROM NEW.target_maximum_marks
        OR OLD.idempotency_key IS DISTINCT FROM NEW.idempotency_key
        OR OLD.request_fingerprint IS DISTINCT FROM NEW.request_fingerprint
        OR OLD.proposed_by IS DISTINCT FROM NEW.proposed_by
        OR OLD.proposed_at IS DISTINCT FROM NEW.proposed_at THEN
        RAISE EXCEPTION 'Learning score-transfer proposal evidence is immutable';
    END IF;
    IF OLD.status <> 'pending' OR NEW.status NOT IN ('applied', 'rejected')
        OR NEW.version <> OLD.version + 1 THEN
        RAISE EXCEPTION 'Learning score-transfer lifecycle transition is invalid';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS learning_score_transfer_lifecycle_immutable
    ON learning_score_transfer_proposals;
CREATE TRIGGER learning_score_transfer_lifecycle_immutable
    BEFORE UPDATE OR DELETE ON learning_score_transfer_proposals
    FOR EACH ROW EXECUTE FUNCTION protect_learning_score_transfer_lifecycle();

CREATE TABLE learning_score_transfer_rows (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    proposal_id UUID NOT NULL,
    target_mark_id UUID NOT NULL,
    enrolment_id UUID NOT NULL,
    learner_id UUID NOT NULL,
    learner_number_snapshot TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(learner_number_snapshot)) BETWEEN 1 AND 100),
    learner_name_snapshot TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(learner_name_snapshot)) BETWEEN 1 AND 240),
    target_mark_version INTEGER NOT NULL CHECK (target_mark_version > 0),
    source_evidence_id UUID,
    source_evidence_version INTEGER CHECK (source_evidence_version > 0),
    source_score_basis_points INTEGER CHECK (source_score_basis_points BETWEEN 0 AND 10000),
    proposed_marks_hundredths BIGINT CHECK (proposed_marks_hundredths >= 0),
    outcome TEXT NOT NULL
        CHECK (outcome IN ('ready', 'missing_source', 'target_already_marked')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, proposal_id, target_mark_id),
    FOREIGN KEY (proposal_id, tenant_id)
        REFERENCES learning_score_transfer_proposals(id, tenant_id),
    FOREIGN KEY (target_mark_id, tenant_id)
        REFERENCES assessment_marks(id, tenant_id),
    FOREIGN KEY (enrolment_id, tenant_id, learner_id)
        REFERENCES enrolments(id, tenant_id, learner_id),
    CHECK (
        (outcome = 'ready' AND source_evidence_id IS NOT NULL
            AND source_evidence_version IS NOT NULL
            AND source_score_basis_points IS NOT NULL
            AND proposed_marks_hundredths IS NOT NULL)
        OR (outcome = 'missing_source' AND source_evidence_id IS NULL
            AND source_evidence_version IS NULL
            AND source_score_basis_points IS NULL
            AND proposed_marks_hundredths IS NULL)
        OR (outcome = 'target_already_marked')
    )
);

CREATE INDEX idx_learning_score_transfer_rows_proposal
    ON learning_score_transfer_rows(tenant_id, proposal_id, outcome, learner_name_snapshot);

CREATE OR REPLACE FUNCTION prevent_learning_score_transfer_row_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Learning score-transfer rows are immutable';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER learning_score_transfer_rows_immutable
    BEFORE UPDATE OR DELETE ON learning_score_transfer_rows
    FOR EACH ROW EXECUTE FUNCTION prevent_learning_score_transfer_row_mutation();
