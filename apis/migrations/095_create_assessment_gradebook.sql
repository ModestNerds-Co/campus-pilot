-- Assessment mark sheets owned by the Academics Gradebook workflow.
--
-- Academics owns assessment structure and SIS owns enrolment identity. Gradebook
-- stores only stable references, exact hundredths-of-a-mark values, and an
-- append-only lifecycle trail.

CREATE TABLE IF NOT EXISTS assessment_mark_sheets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    assessment_component_id UUID NOT NULL,
    roster_on DATE NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'submitted', 'published')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    idempotency_key TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(idempotency_key)) BETWEEN 1 AND 200),
    create_request_fingerprint TEXT NOT NULL
        CHECK (create_request_fingerprint ~ '^[0-9a-f]{64}$'),
    created_by UUID NOT NULL,
    submitted_by UUID,
    submitted_at TIMESTAMPTZ,
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
    CONSTRAINT assessment_mark_sheets_component_tenant_fkey
        FOREIGN KEY (assessment_component_id, tenant_id)
        REFERENCES assessment_components(id, tenant_id),
    CONSTRAINT assessment_mark_sheets_creator_tenant_fkey
        FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT assessment_mark_sheets_submitter_tenant_fkey
        FOREIGN KEY (submitted_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT assessment_mark_sheets_publisher_tenant_fkey
        FOREIGN KEY (published_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT assessment_mark_sheets_reopener_tenant_fkey
        FOREIGN KEY (reopened_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT assessment_mark_sheets_lifecycle_check CHECK (
        (status = 'draft' AND submitted_by IS NULL AND submitted_at IS NULL
            AND published_by IS NULL AND published_at IS NULL)
        OR (status = 'submitted' AND submitted_by IS NOT NULL AND submitted_at IS NOT NULL
            AND published_by IS NULL AND published_at IS NULL)
        OR (status = 'published' AND submitted_by IS NOT NULL AND submitted_at IS NOT NULL
            AND published_by IS NOT NULL AND published_at IS NOT NULL)
    ),
    CONSTRAINT assessment_mark_sheets_reopen_check CHECK (
        (reopened_by IS NULL AND reopened_at IS NULL AND reopen_reason IS NULL)
        OR (reopened_by IS NOT NULL AND reopened_at IS NOT NULL AND reopen_reason IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_assessment_mark_sheets_component
    ON assessment_mark_sheets(tenant_id, assessment_component_id)
    WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_assessment_mark_sheets_idempotency
    ON assessment_mark_sheets(tenant_id, idempotency_key);
CREATE INDEX IF NOT EXISTS idx_assessment_mark_sheets_worklist
    ON assessment_mark_sheets(tenant_id, status, roster_on DESC, created_at DESC)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_assessment_mark_sheets_updated_at ON assessment_mark_sheets;
CREATE TRIGGER update_assessment_mark_sheets_updated_at
    BEFORE UPDATE ON assessment_mark_sheets
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS assessment_marks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    mark_sheet_id UUID NOT NULL,
    enrolment_id UUID NOT NULL,
    learner_id UUID NOT NULL,
    mark_status TEXT NOT NULL DEFAULT 'unmarked'
        CHECK (mark_status IN ('unmarked', 'scored', 'absent', 'exempt')),
    marks_awarded_hundredths BIGINT CHECK (marks_awarded_hundredths >= 0),
    note TEXT CHECK (note IS NULL OR CHAR_LENGTH(BTRIM(note)) BETWEEN 1 AND 1000),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    marked_by UUID,
    marked_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CONSTRAINT assessment_marks_sheet_tenant_fkey
        FOREIGN KEY (mark_sheet_id, tenant_id)
        REFERENCES assessment_mark_sheets(id, tenant_id),
    CONSTRAINT assessment_marks_enrolment_tenant_learner_fkey
        FOREIGN KEY (enrolment_id, tenant_id, learner_id)
        REFERENCES enrolments(id, tenant_id, learner_id),
    CONSTRAINT assessment_marks_marker_tenant_fkey
        FOREIGN KEY (marked_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT assessment_marks_value_check CHECK (
        (mark_status = 'scored' AND marks_awarded_hundredths IS NOT NULL)
        OR (mark_status <> 'scored' AND marks_awarded_hundredths IS NULL)
    ),
    CONSTRAINT assessment_marks_marked_check CHECK (
        (mark_status = 'unmarked' AND marked_by IS NULL AND marked_at IS NULL AND note IS NULL)
        OR (mark_status <> 'unmarked' AND marked_by IS NOT NULL AND marked_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_assessment_marks_roster
    ON assessment_marks(tenant_id, mark_sheet_id, learner_id)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_assessment_marks_learner_history
    ON assessment_marks(tenant_id, learner_id, created_at DESC)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_assessment_marks_updated_at ON assessment_marks;
CREATE TRIGGER update_assessment_marks_updated_at
    BEFORE UPDATE ON assessment_marks
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS assessment_mark_sheet_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    mark_sheet_id UUID NOT NULL,
    event_type TEXT NOT NULL CHECK (
        event_type IN ('created', 'marks_updated', 'submitted', 'published', 'reopened', 'deleted')
    ),
    from_status TEXT CHECK (
        from_status IS NULL OR from_status IN ('draft', 'submitted', 'published')
    ),
    to_status TEXT NOT NULL CHECK (
        to_status IN ('draft', 'submitted', 'published', 'deleted')
    ),
    mark_sheet_version INTEGER NOT NULL CHECK (mark_sheet_version > 0),
    actor_id UUID NOT NULL,
    reason TEXT CHECK (reason IS NULL OR CHAR_LENGTH(BTRIM(reason)) BETWEEN 1 AND 1000),
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CONSTRAINT assessment_mark_sheet_events_parent_tenant_fkey
        FOREIGN KEY (mark_sheet_id, tenant_id)
        REFERENCES assessment_mark_sheets(id, tenant_id),
    CONSTRAINT assessment_mark_sheet_events_actor_tenant_fkey
        FOREIGN KEY (actor_id, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE INDEX IF NOT EXISTS idx_assessment_mark_sheet_events_history
    ON assessment_mark_sheet_events(tenant_id, mark_sheet_id, created_at, id);

CREATE OR REPLACE FUNCTION enforce_assessment_mark_draft_sheet()
RETURNS TRIGGER AS $$
DECLARE
    parent_status TEXT;
    maximum_hundredths BIGINT;
BEGIN
    SELECT sheet.status, component.maximum_marks::BIGINT * 100
      INTO parent_status, maximum_hundredths
      FROM assessment_mark_sheets AS sheet
      JOIN assessment_components AS component
        ON component.id = sheet.assessment_component_id
       AND component.tenant_id = sheet.tenant_id
     WHERE sheet.tenant_id = COALESCE(NEW.tenant_id, OLD.tenant_id)
       AND sheet.id = COALESCE(NEW.mark_sheet_id, OLD.mark_sheet_id)
       AND sheet.deleted_at IS NULL
       AND component.deleted_at IS NULL;

    IF parent_status IS DISTINCT FROM 'draft' THEN
        RAISE EXCEPTION 'Assessment marks may change only while the mark sheet is draft';
    END IF;
    IF TG_OP <> 'DELETE'
       AND NEW.mark_status = 'scored'
       AND NEW.marks_awarded_hundredths > maximum_hundredths THEN
        RAISE EXCEPTION 'An awarded mark cannot exceed the assessment maximum';
    END IF;
    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS assessment_marks_draft_guard ON assessment_marks;
CREATE TRIGGER assessment_marks_draft_guard
    BEFORE INSERT OR UPDATE OR DELETE ON assessment_marks
    FOR EACH ROW EXECUTE FUNCTION enforce_assessment_mark_draft_sheet();

CREATE OR REPLACE FUNCTION enforce_assessment_mark_sheet_lifecycle()
RETURNS TRIGGER AS $$
DECLARE
    cycle_status TEXT;
    mark_count BIGINT;
    unmarked_count BIGINT;
BEGIN
    SELECT cycle.status INTO cycle_status
      FROM assessment_components AS component
      JOIN assessment_cycles AS cycle
        ON cycle.id = component.assessment_cycle_id
       AND cycle.tenant_id = component.tenant_id
     WHERE component.id = NEW.assessment_component_id
       AND component.tenant_id = NEW.tenant_id
       AND component.status = 'active'
       AND component.deleted_at IS NULL
       AND cycle.deleted_at IS NULL;

    IF cycle_status IS NULL THEN
        RAISE EXCEPTION 'The active assessment component is unavailable';
    END IF;
    IF TG_OP = 'INSERT' AND cycle_status <> 'open' THEN
        RAISE EXCEPTION 'Mark sheets can be created only for an open assessment cycle';
    END IF;
    IF TG_OP = 'UPDATE' AND OLD.status IS DISTINCT FROM NEW.status THEN
        IF cycle_status = 'closed' THEN
            RAISE EXCEPTION 'A closed assessment cycle cannot change mark sheets';
        END IF;
        IF NOT (
            (OLD.status = 'draft' AND NEW.status = 'submitted')
            OR (OLD.status = 'submitted' AND NEW.status = 'published')
            OR (OLD.status IN ('submitted', 'published') AND NEW.status = 'draft'
                AND NEW.reopened_by IS NOT NULL AND NEW.reopened_at IS NOT NULL
                AND NEW.reopen_reason IS NOT NULL)
        ) THEN
            RAISE EXCEPTION 'Assessment mark sheets follow draft, submitted, and published transitions';
        END IF;
        IF OLD.status = 'draft' AND NEW.status = 'submitted' THEN
            SELECT COUNT(*), COUNT(*) FILTER (WHERE mark_status = 'unmarked')
              INTO mark_count, unmarked_count
              FROM assessment_marks
             WHERE tenant_id = NEW.tenant_id
               AND mark_sheet_id = NEW.id
               AND deleted_at IS NULL;
            IF mark_count = 0 OR unmarked_count > 0 THEN
                RAISE EXCEPTION 'Every learner must be marked before submission';
            END IF;
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS assessment_mark_sheets_lifecycle_guard ON assessment_mark_sheets;
CREATE TRIGGER assessment_mark_sheets_lifecycle_guard
    BEFORE INSERT OR UPDATE ON assessment_mark_sheets
    FOR EACH ROW EXECUTE FUNCTION enforce_assessment_mark_sheet_lifecycle();

CREATE OR REPLACE FUNCTION prevent_assessment_mark_sheet_event_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Assessment mark sheet history is append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS assessment_mark_sheet_events_append_only ON assessment_mark_sheet_events;
CREATE TRIGGER assessment_mark_sheet_events_append_only
    BEFORE UPDATE OR DELETE ON assessment_mark_sheet_events
    FOR EACH ROW EXECUTE FUNCTION prevent_assessment_mark_sheet_event_mutation();

CREATE OR REPLACE FUNCTION require_published_mark_sheets_before_cycle_close()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.status = 'open' AND NEW.status = 'closed' AND EXISTS (
        SELECT 1
          FROM assessment_components AS component
          LEFT JOIN assessment_mark_sheets AS sheet
            ON sheet.tenant_id = component.tenant_id
           AND sheet.assessment_component_id = component.id
           AND sheet.deleted_at IS NULL
           AND sheet.status = 'published'
         WHERE component.tenant_id = NEW.tenant_id
           AND component.assessment_cycle_id = NEW.id
           AND component.status = 'active'
           AND component.deleted_at IS NULL
           AND sheet.id IS NULL
    ) THEN
        RAISE EXCEPTION 'Publish every active assessment mark sheet before closing the cycle';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS require_published_mark_sheets_on_cycle_close ON assessment_cycles;
CREATE TRIGGER require_published_mark_sheets_on_cycle_close
    BEFORE UPDATE ON assessment_cycles
    FOR EACH ROW EXECUTE FUNCTION require_published_mark_sheets_before_cycle_close();

DROP TRIGGER IF EXISTS ev_assessment_mark_sheets ON assessment_mark_sheets;
CREATE TRIGGER ev_assessment_mark_sheets
    AFTER INSERT OR UPDATE OR DELETE ON assessment_mark_sheets
    FOR EACH ROW EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_assessment_marks ON assessment_marks;
CREATE TRIGGER ev_assessment_marks
    AFTER INSERT OR UPDATE OR DELETE ON assessment_marks
    FOR EACH ROW EXECUTE FUNCTION log_event();

INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
SELECT tenant.id, 'academic_manager', 'Academic Manager',
       'Reviews academic structures, mark sheets, and published results.',
       ARRAY[
           'academics:view', 'academics:create', 'academics:edit',
           'academics:delete', 'academics:manage', 'sis:view'
       ]::TEXT[], TRUE
  FROM tenants AS tenant
 WHERE NOT EXISTS (
    SELECT 1 FROM roles AS role
     WHERE role.tenant_id = tenant.id AND role.key = 'academic_manager'
       AND role.deleted_at IS NULL
 );

CREATE OR REPLACE FUNCTION provision_new_tenant_academic_manager()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
    VALUES (
        NEW.id, 'academic_manager', 'Academic Manager',
        'Reviews academic structures, mark sheets, and published results.',
        ARRAY[
            'academics:view', 'academics:create', 'academics:edit',
            'academics:delete', 'academics:manage', 'sis:view'
        ]::TEXT[], TRUE
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_provision_new_tenant_academic_manager ON tenants;
CREATE TRIGGER zz_provision_new_tenant_academic_manager
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION provision_new_tenant_academic_manager();

INSERT INTO role_record_scope_grants (tenant_id, role_id, scope_family, scope_kind)
SELECT role.tenant_id, role.id, 'academics.gradebook',
       CASE WHEN role.key = 'teacher' THEN 'assigned' ELSE 'campus' END
  FROM roles AS role
 WHERE role.key IN ('teacher', 'academic_manager')
   AND role.deleted_at IS NULL
ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
    WHERE deleted_at IS NULL
    DO NOTHING;

CREATE OR REPLACE FUNCTION provision_gradebook_role_record_scopes()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.key IN ('teacher', 'academic_manager') THEN
        INSERT INTO role_record_scope_grants (
            tenant_id, role_id, scope_family, scope_kind
        )
        VALUES (
            NEW.tenant_id, NEW.id, 'academics.gradebook',
            CASE WHEN NEW.key = 'teacher' THEN 'assigned' ELSE 'campus' END
        )
        ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
            WHERE deleted_at IS NULL
            DO NOTHING;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS provision_gradebook_role_record_scopes_after_insert ON roles;
CREATE TRIGGER provision_gradebook_role_record_scopes_after_insert
    AFTER INSERT ON roles
    FOR EACH ROW EXECUTE FUNCTION provision_gradebook_role_record_scopes();
