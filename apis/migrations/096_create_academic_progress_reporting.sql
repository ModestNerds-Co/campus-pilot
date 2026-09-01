-- Academic result snapshots, report cards, progression decisions, and grading schemes.
--
-- Published Gradebook marks and submitted Attendance remain owned by their
-- modules. Reporting stores immutable calculation snapshots plus reviewable
-- remarks and progression decisions; it never rewrites source records.

CREATE TABLE IF NOT EXISTS academic_grading_schemes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    description TEXT,
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'retired')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_academic_grading_schemes_name
    ON academic_grading_schemes(tenant_id, LOWER(name))
    WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_academic_grading_schemes_default
    ON academic_grading_schemes(tenant_id)
    WHERE is_default = TRUE AND status = 'active' AND deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_academic_grading_schemes_updated_at
    ON academic_grading_schemes;
CREATE TRIGGER update_academic_grading_schemes_updated_at
    BEFORE UPDATE ON academic_grading_schemes
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS academic_grading_bands (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    grading_scheme_id UUID NOT NULL,
    code TEXT NOT NULL,
    label TEXT NOT NULL,
    minimum_basis_points SMALLINT NOT NULL
        CHECK (minimum_basis_points BETWEEN 0 AND 10000),
    is_pass BOOLEAN NOT NULL,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (grading_scheme_id, tenant_id)
        REFERENCES academic_grading_schemes(id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_academic_grading_bands_code
    ON academic_grading_bands(tenant_id, grading_scheme_id, LOWER(code))
    WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_academic_grading_bands_minimum
    ON academic_grading_bands(tenant_id, grading_scheme_id, minimum_basis_points)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_academic_grading_bands_updated_at
    ON academic_grading_bands;
CREATE TRIGGER update_academic_grading_bands_updated_at
    BEFORE UPDATE ON academic_grading_bands
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS academic_report_batches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    assessment_cycle_id UUID NOT NULL,
    class_group_id UUID NOT NULL,
    grading_scheme_id UUID NOT NULL,
    grading_scheme_version INTEGER NOT NULL CHECK (grading_scheme_version > 0),
    grading_scheme_name_snapshot TEXT NOT NULL,
    source_fingerprint TEXT NOT NULL CHECK (source_fingerprint ~ '^[0-9a-f]{64}$'),
    idempotency_key TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(idempotency_key)) BETWEEN 1 AND 200),
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'reviewed', 'published')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    generated_by UUID NOT NULL,
    reviewed_by UUID,
    reviewed_at TIMESTAMPTZ,
    published_by UUID,
    published_at TIMESTAMPTZ,
    reopened_by UUID,
    reopened_at TIMESTAMPTZ,
    reopen_reason TEXT CHECK (
        reopen_reason IS NULL OR CHAR_LENGTH(BTRIM(reopen_reason)) BETWEEN 1 AND 1000
    ),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (assessment_cycle_id, tenant_id)
        REFERENCES assessment_cycles(id, tenant_id),
    FOREIGN KEY (class_group_id, tenant_id)
        REFERENCES class_groups(id, tenant_id),
    FOREIGN KEY (grading_scheme_id, tenant_id)
        REFERENCES academic_grading_schemes(id, tenant_id),
    FOREIGN KEY (generated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (reviewed_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (published_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (reopened_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'draft' AND reviewed_by IS NULL AND reviewed_at IS NULL
            AND published_by IS NULL AND published_at IS NULL)
        OR (status = 'reviewed' AND reviewed_by IS NOT NULL AND reviewed_at IS NOT NULL
            AND published_by IS NULL AND published_at IS NULL)
        OR (status = 'published' AND reviewed_by IS NOT NULL AND reviewed_at IS NOT NULL
            AND published_by IS NOT NULL AND published_at IS NOT NULL)
    ),
    CHECK (
        (reopened_by IS NULL AND reopened_at IS NULL AND reopen_reason IS NULL)
        OR (reopened_by IS NOT NULL AND reopened_at IS NOT NULL AND reopen_reason IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_academic_report_batches_source
    ON academic_report_batches(tenant_id, assessment_cycle_id, class_group_id)
    WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_academic_report_batches_idempotency
    ON academic_report_batches(tenant_id, idempotency_key);
CREATE INDEX IF NOT EXISTS idx_academic_report_batches_worklist
    ON academic_report_batches(tenant_id, status, created_at DESC)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_academic_report_batches_updated_at
    ON academic_report_batches;
CREATE TRIGGER update_academic_report_batches_updated_at
    BEFORE UPDATE ON academic_report_batches
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS academic_report_cards (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    report_batch_id UUID NOT NULL,
    enrolment_id UUID NOT NULL,
    learner_id UUID NOT NULL,
    learner_number_snapshot TEXT NOT NULL CHECK (BTRIM(learner_number_snapshot) <> ''),
    learner_name_snapshot TEXT NOT NULL CHECK (BTRIM(learner_name_snapshot) <> ''),
    overall_percentage_basis_points SMALLINT
        CHECK (overall_percentage_basis_points BETWEEN 0 AND 10000),
    overall_grade_code TEXT,
    overall_grade_label TEXT,
    teacher_comment TEXT CHECK (
        teacher_comment IS NULL OR CHAR_LENGTH(BTRIM(teacher_comment)) BETWEEN 1 AND 2000
    ),
    reviewer_comment TEXT CHECK (
        reviewer_comment IS NULL OR CHAR_LENGTH(BTRIM(reviewer_comment)) BETWEEN 1 AND 2000
    ),
    progression_outcome TEXT NOT NULL DEFAULT 'not_applicable'
        CHECK (progression_outcome IN (
            'not_applicable', 'pending', 'promoted', 'retained', 'completed'
        )),
    target_grade_level_id UUID,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (report_batch_id, tenant_id)
        REFERENCES academic_report_batches(id, tenant_id),
    FOREIGN KEY (enrolment_id, tenant_id, learner_id)
        REFERENCES enrolments(id, tenant_id, learner_id),
    FOREIGN KEY (target_grade_level_id, tenant_id)
        REFERENCES academic_grade_levels(id, tenant_id),
    CHECK (
        (progression_outcome = 'promoted' AND target_grade_level_id IS NOT NULL)
        OR (progression_outcome <> 'promoted' AND target_grade_level_id IS NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_academic_report_cards_batch_learner
    ON academic_report_cards(tenant_id, report_batch_id, learner_id)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_academic_report_cards_learner_history
    ON academic_report_cards(tenant_id, learner_id, created_at DESC)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_academic_report_cards_updated_at ON academic_report_cards;
CREATE TRIGGER update_academic_report_cards_updated_at
    BEFORE UPDATE ON academic_report_cards
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS academic_report_subject_results (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    report_card_id UUID NOT NULL,
    teaching_assignment_id UUID NOT NULL,
    subject_id UUID NOT NULL,
    subject_name_snapshot TEXT NOT NULL CHECK (BTRIM(subject_name_snapshot) <> ''),
    result_status TEXT NOT NULL CHECK (result_status IN ('graded', 'exempt', 'incomplete')),
    percentage_basis_points SMALLINT CHECK (percentage_basis_points BETWEEN 0 AND 10000),
    grade_code TEXT,
    grade_label TEXT,
    is_pass BOOLEAN,
    scored_component_count INTEGER NOT NULL DEFAULT 0 CHECK (scored_component_count >= 0),
    absent_component_count INTEGER NOT NULL DEFAULT 0 CHECK (absent_component_count >= 0),
    exempt_component_count INTEGER NOT NULL DEFAULT 0 CHECK (exempt_component_count >= 0),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (report_card_id, tenant_id)
        REFERENCES academic_report_cards(id, tenant_id),
    FOREIGN KEY (teaching_assignment_id, tenant_id)
        REFERENCES teaching_assignments(id, tenant_id),
    FOREIGN KEY (subject_id, tenant_id) REFERENCES subjects(id, tenant_id),
    CHECK (
        (result_status = 'graded' AND percentage_basis_points IS NOT NULL
            AND grade_code IS NOT NULL AND grade_label IS NOT NULL AND is_pass IS NOT NULL)
        OR (result_status <> 'graded' AND percentage_basis_points IS NULL
            AND grade_code IS NULL AND grade_label IS NULL AND is_pass IS NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_academic_report_subject_results_assignment
    ON academic_report_subject_results(tenant_id, report_card_id, teaching_assignment_id)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS academic_report_attendance (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    report_card_id UUID NOT NULL,
    present_count INTEGER NOT NULL DEFAULT 0 CHECK (present_count >= 0),
    absent_count INTEGER NOT NULL DEFAULT 0 CHECK (absent_count >= 0),
    late_count INTEGER NOT NULL DEFAULT 0 CHECK (late_count >= 0),
    excused_count INTEGER NOT NULL DEFAULT 0 CHECK (excused_count >= 0),
    attendance_percentage_basis_points SMALLINT
        CHECK (attendance_percentage_basis_points BETWEEN 0 AND 10000),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (report_card_id, tenant_id)
        REFERENCES academic_report_cards(id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_academic_report_attendance_card
    ON academic_report_attendance(tenant_id, report_card_id)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS academic_report_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    report_batch_id UUID NOT NULL,
    event_type TEXT NOT NULL CHECK (
        event_type IN (
            'generated', 'remarks_updated', 'reviewed', 'published', 'reopened', 'deleted'
        )
    ),
    from_status TEXT CHECK (
        from_status IS NULL OR from_status IN ('draft', 'reviewed', 'published')
    ),
    to_status TEXT NOT NULL CHECK (
        to_status IN ('draft', 'reviewed', 'published', 'deleted')
    ),
    report_batch_version INTEGER NOT NULL CHECK (report_batch_version > 0),
    actor_id UUID NOT NULL,
    reason TEXT CHECK (reason IS NULL OR CHAR_LENGTH(BTRIM(reason)) BETWEEN 1 AND 1000),
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (report_batch_id, tenant_id)
        REFERENCES academic_report_batches(id, tenant_id),
    FOREIGN KEY (actor_id, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE INDEX IF NOT EXISTS idx_academic_report_events_history
    ON academic_report_events(tenant_id, report_batch_id, created_at, id);

CREATE OR REPLACE FUNCTION enforce_academic_report_batch_lifecycle()
RETURNS TRIGGER AS $$
DECLARE
    cycle_status TEXT;
BEGIN
    SELECT status INTO cycle_status
      FROM assessment_cycles
     WHERE tenant_id = NEW.tenant_id
       AND id = NEW.assessment_cycle_id
       AND deleted_at IS NULL;

    IF cycle_status IS DISTINCT FROM 'closed' THEN
        RAISE EXCEPTION 'Academic reports require a closed assessment cycle';
    END IF;

    IF TG_OP = 'UPDATE' AND OLD.status IS DISTINCT FROM NEW.status THEN
        IF NOT (
            (OLD.status = 'draft' AND NEW.status = 'reviewed')
            OR (OLD.status = 'reviewed' AND NEW.status = 'published')
            OR (OLD.status IN ('reviewed', 'published') AND NEW.status = 'draft'
                AND NEW.reopened_by IS NOT NULL AND NEW.reopened_at IS NOT NULL
                AND NEW.reopen_reason IS NOT NULL)
        ) THEN
            RAISE EXCEPTION 'Academic reports follow draft, reviewed, and published transitions';
        END IF;

        IF OLD.status = 'draft' AND NEW.status = 'reviewed' THEN
            IF NOT EXISTS (
                SELECT 1 FROM academic_report_cards
                 WHERE tenant_id = NEW.tenant_id
                   AND report_batch_id = NEW.id
                   AND deleted_at IS NULL
            ) OR EXISTS (
                SELECT 1
                  FROM academic_report_subject_results AS result
                  JOIN academic_report_cards AS card
                    ON card.id = result.report_card_id
                   AND card.tenant_id = result.tenant_id
                   AND card.deleted_at IS NULL
                 WHERE result.tenant_id = NEW.tenant_id
                   AND card.report_batch_id = NEW.id
                   AND result.result_status = 'incomplete'
                   AND result.deleted_at IS NULL
            ) THEN
                RAISE EXCEPTION 'Resolve every incomplete subject result before review';
            END IF;
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS academic_report_batch_lifecycle_guard ON academic_report_batches;
CREATE TRIGGER academic_report_batch_lifecycle_guard
    BEFORE INSERT OR UPDATE ON academic_report_batches
    FOR EACH ROW EXECUTE FUNCTION enforce_academic_report_batch_lifecycle();

CREATE OR REPLACE FUNCTION enforce_academic_report_card_draft_change()
RETURNS TRIGGER AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM academic_report_batches
         WHERE tenant_id = COALESCE(NEW.tenant_id, OLD.tenant_id)
           AND id = COALESCE(NEW.report_batch_id, OLD.report_batch_id)
           AND status = 'draft'
           AND deleted_at IS NULL
    ) THEN
        RAISE EXCEPTION 'Published or reviewed report cards must be reopened before changes';
    END IF;
    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS academic_report_card_draft_guard ON academic_report_cards;
CREATE TRIGGER academic_report_card_draft_guard
    BEFORE UPDATE OR DELETE ON academic_report_cards
    FOR EACH ROW EXECUTE FUNCTION enforce_academic_report_card_draft_change();

CREATE OR REPLACE FUNCTION prevent_academic_report_result_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Calculated academic report evidence is immutable';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS academic_report_subject_results_immutable
    ON academic_report_subject_results;
CREATE TRIGGER academic_report_subject_results_immutable
    BEFORE UPDATE OR DELETE ON academic_report_subject_results
    FOR EACH ROW EXECUTE FUNCTION prevent_academic_report_result_mutation();

DROP TRIGGER IF EXISTS academic_report_attendance_immutable ON academic_report_attendance;
CREATE TRIGGER academic_report_attendance_immutable
    BEFORE UPDATE OR DELETE ON academic_report_attendance
    FOR EACH ROW EXECUTE FUNCTION prevent_academic_report_result_mutation();

CREATE OR REPLACE FUNCTION prevent_academic_report_event_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Academic report events are append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS academic_report_events_append_only ON academic_report_events;
CREATE TRIGGER academic_report_events_append_only
    BEFORE UPDATE OR DELETE ON academic_report_events
    FOR EACH ROW EXECUTE FUNCTION prevent_academic_report_event_mutation();

DROP TRIGGER IF EXISTS ev_academic_grading_schemes ON academic_grading_schemes;
CREATE TRIGGER ev_academic_grading_schemes
    AFTER INSERT OR UPDATE OR DELETE ON academic_grading_schemes
    FOR EACH ROW EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_academic_report_batches ON academic_report_batches;
CREATE TRIGGER ev_academic_report_batches
    AFTER INSERT OR UPDATE OR DELETE ON academic_report_batches
    FOR EACH ROW EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_academic_report_cards ON academic_report_cards;
CREATE TRIGGER ev_academic_report_cards
    AFTER INSERT OR UPDATE OR DELETE ON academic_report_cards
    FOR EACH ROW EXECUTE FUNCTION log_event();

CREATE OR REPLACE FUNCTION reporting_scope_allows(
    requested_tenant_id UUID,
    requested_batch_id UUID,
    scope_kind TEXT,
    person_account_id UUID
) RETURNS BOOLEAN STABLE LANGUAGE sql AS $$
    SELECT CASE scope_kind
        WHEN 'campus' THEN EXISTS (
            SELECT 1 FROM academic_report_batches AS batch
             WHERE batch.tenant_id = requested_tenant_id
               AND batch.id = requested_batch_id
               AND batch.deleted_at IS NULL
        )
        WHEN 'assigned' THEN EXISTS (
            SELECT 1
              FROM academic_report_batches AS batch
              JOIN assessment_components AS component
                ON component.assessment_cycle_id = batch.assessment_cycle_id
               AND component.tenant_id = batch.tenant_id
               AND component.deleted_at IS NULL
              JOIN teaching_assignments AS assignment
                ON assignment.id = component.teaching_assignment_id
               AND assignment.tenant_id = component.tenant_id
               AND assignment.class_group_id = batch.class_group_id
               AND assignment.deleted_at IS NULL
              JOIN teacher_profiles AS teacher
                ON teacher.id = assignment.teacher_profile_id
               AND teacher.tenant_id = assignment.tenant_id
               AND teacher.deleted_at IS NULL
              JOIN employees AS employee
                ON employee.id = teacher.employee_id
               AND employee.tenant_id = teacher.tenant_id
               AND employee.deleted_at IS NULL
             WHERE batch.tenant_id = requested_tenant_id
               AND batch.id = requested_batch_id
               AND batch.deleted_at IS NULL
               AND employee.account_id = person_account_id
        )
        WHEN 'self' THEN EXISTS (
            SELECT 1
              FROM academic_report_batches AS batch
              JOIN academic_report_cards AS card
                ON card.report_batch_id = batch.id
               AND card.tenant_id = batch.tenant_id
               AND card.deleted_at IS NULL
              JOIN learners AS learner
                ON learner.id = card.learner_id
               AND learner.tenant_id = card.tenant_id
               AND learner.deleted_at IS NULL
              LEFT JOIN learner_guardian_relationships AS relationship
                ON relationship.learner_id = learner.id
               AND relationship.tenant_id = learner.tenant_id
               AND relationship.status = 'active'
               AND relationship.deleted_at IS NULL
              LEFT JOIN guardians AS guardian
                ON guardian.id = relationship.guardian_id
               AND guardian.tenant_id = relationship.tenant_id
               AND guardian.status = 'active'
               AND guardian.deleted_at IS NULL
             WHERE batch.tenant_id = requested_tenant_id
               AND batch.id = requested_batch_id
               AND batch.status = 'published'
               AND batch.deleted_at IS NULL
               AND (
                   learner.account_id = person_account_id
                   OR guardian.account_id = person_account_id
               )
        )
        WHEN 'self_and_assigned' THEN
            reporting_scope_allows(
                requested_tenant_id, requested_batch_id, 'self', person_account_id
            ) OR reporting_scope_allows(
                requested_tenant_id, requested_batch_id, 'assigned', person_account_id
            )
        ELSE FALSE
    END;
$$;

INSERT INTO role_record_scope_grants (tenant_id, role_id, scope_family, scope_kind)
SELECT role.tenant_id, role.id, 'academics.reporting',
       CASE
           WHEN role.key = 'teacher' THEN 'assigned'
           WHEN role.key = 'student' THEN 'self'
           ELSE 'campus'
       END
  FROM roles AS role
 WHERE role.key IN ('teacher', 'student', 'academic_manager')
   AND role.deleted_at IS NULL
ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
    WHERE deleted_at IS NULL
    DO NOTHING;

CREATE OR REPLACE FUNCTION provision_academic_reporting_role_scopes()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.key IN ('teacher', 'student', 'academic_manager') THEN
        INSERT INTO role_record_scope_grants (
            tenant_id, role_id, scope_family, scope_kind
        ) VALUES (
            NEW.tenant_id, NEW.id, 'academics.reporting',
            CASE
                WHEN NEW.key = 'teacher' THEN 'assigned'
                WHEN NEW.key = 'student' THEN 'self'
                ELSE 'campus'
            END
        )
        ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
            WHERE deleted_at IS NULL
            DO NOTHING;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS provision_academic_reporting_role_scopes_after_insert ON roles;
CREATE TRIGGER provision_academic_reporting_role_scopes_after_insert
    AFTER INSERT ON roles
    FOR EACH ROW EXECUTE FUNCTION provision_academic_reporting_role_scopes();
