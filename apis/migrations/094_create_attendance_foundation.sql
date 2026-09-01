-- Daily learner attendance registers owned by Attendance.
--
-- Academics owns terms and classes; SIS owns enrolment and learner identity.
-- Registers retain only stable identifiers and lock their roster on submission.

ALTER TABLE enrolments
    ADD CONSTRAINT enrolments_attendance_identity_unique
    UNIQUE (id, tenant_id, learner_id);

CREATE TABLE IF NOT EXISTS attendance_registers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    academic_term_id UUID NOT NULL,
    class_group_id UUID NOT NULL,
    attendance_date DATE NOT NULL,
    period TEXT NOT NULL CHECK (period IN ('full_day', 'morning', 'afternoon')),
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'submitted')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    idempotency_key TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(idempotency_key)) BETWEEN 1 AND 200),
    create_request_fingerprint TEXT NOT NULL
        CHECK (create_request_fingerprint ~ '^[0-9a-f]{64}$'),
    created_by UUID NOT NULL,
    submitted_by UUID,
    submitted_at TIMESTAMPTZ,
    reopened_by UUID,
    reopened_at TIMESTAMPTZ,
    reopen_reason TEXT CHECK (
        reopen_reason IS NULL OR CHAR_LENGTH(BTRIM(reopen_reason)) BETWEEN 1 AND 1000
    ),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CONSTRAINT attendance_registers_term_tenant_fkey
        FOREIGN KEY (academic_term_id, tenant_id)
        REFERENCES academic_terms(id, tenant_id),
    CONSTRAINT attendance_registers_class_tenant_fkey
        FOREIGN KEY (class_group_id, tenant_id)
        REFERENCES class_groups(id, tenant_id),
    CONSTRAINT attendance_registers_creator_tenant_fkey
        FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT attendance_registers_submitter_tenant_fkey
        FOREIGN KEY (submitted_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT attendance_registers_reopener_tenant_fkey
        FOREIGN KEY (reopened_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT attendance_registers_submission_check CHECK (
        (status = 'draft' AND submitted_by IS NULL AND submitted_at IS NULL)
        OR (status = 'submitted' AND submitted_by IS NOT NULL AND submitted_at IS NOT NULL)
    ),
    CONSTRAINT attendance_registers_reopen_check CHECK (
        (reopened_by IS NULL AND reopened_at IS NULL AND reopen_reason IS NULL)
        OR (reopened_by IS NOT NULL AND reopened_at IS NOT NULL AND reopen_reason IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_attendance_registers_scope
    ON attendance_registers(tenant_id, class_group_id, attendance_date, period)
    WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_attendance_registers_idempotency
    ON attendance_registers(tenant_id, idempotency_key);
CREATE INDEX IF NOT EXISTS idx_attendance_registers_worklist
    ON attendance_registers(tenant_id, attendance_date DESC, status, class_group_id)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_attendance_registers_term
    ON attendance_registers(tenant_id, academic_term_id, attendance_date DESC)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_attendance_registers_updated_at ON attendance_registers;
CREATE TRIGGER update_attendance_registers_updated_at
    BEFORE UPDATE ON attendance_registers
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS attendance_marks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    register_id UUID NOT NULL,
    enrolment_id UUID NOT NULL,
    learner_id UUID NOT NULL,
    mark TEXT NOT NULL DEFAULT 'unmarked'
        CHECK (mark IN ('unmarked', 'present', 'absent', 'late', 'excused')),
    minutes_late INTEGER CHECK (minutes_late BETWEEN 0 AND 1440),
    note TEXT CHECK (note IS NULL OR CHAR_LENGTH(BTRIM(note)) BETWEEN 1 AND 1000),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    marked_by UUID,
    marked_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CONSTRAINT attendance_marks_register_tenant_fkey
        FOREIGN KEY (register_id, tenant_id)
        REFERENCES attendance_registers(id, tenant_id),
    CONSTRAINT attendance_marks_enrolment_tenant_learner_fkey
        FOREIGN KEY (enrolment_id, tenant_id, learner_id)
        REFERENCES enrolments(id, tenant_id, learner_id),
    CONSTRAINT attendance_marks_marker_tenant_fkey
        FOREIGN KEY (marked_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT attendance_marks_marked_check CHECK (
        (mark = 'unmarked' AND marked_by IS NULL AND marked_at IS NULL
            AND minutes_late IS NULL AND note IS NULL)
        OR (mark <> 'unmarked' AND marked_by IS NOT NULL AND marked_at IS NOT NULL)
    ),
    CONSTRAINT attendance_marks_late_check CHECK (
        mark = 'late' OR minutes_late IS NULL
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_attendance_marks_roster
    ON attendance_marks(tenant_id, register_id, learner_id)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_attendance_marks_learner_history
    ON attendance_marks(tenant_id, learner_id, created_at DESC)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_attendance_marks_updated_at ON attendance_marks;
CREATE TRIGGER update_attendance_marks_updated_at
    BEFORE UPDATE ON attendance_marks
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS attendance_register_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    register_id UUID NOT NULL,
    event_type TEXT NOT NULL CHECK (
        event_type IN ('created', 'marks_updated', 'submitted', 'reopened', 'deleted')
    ),
    from_status TEXT CHECK (from_status IS NULL OR from_status IN ('draft', 'submitted')),
    to_status TEXT NOT NULL CHECK (to_status IN ('draft', 'submitted', 'deleted')),
    register_version INTEGER NOT NULL CHECK (register_version > 0),
    actor_id UUID NOT NULL,
    reason TEXT CHECK (reason IS NULL OR CHAR_LENGTH(BTRIM(reason)) BETWEEN 1 AND 1000),
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CONSTRAINT attendance_register_events_parent_tenant_fkey
        FOREIGN KEY (register_id, tenant_id)
        REFERENCES attendance_registers(id, tenant_id),
    CONSTRAINT attendance_register_events_actor_tenant_fkey
        FOREIGN KEY (actor_id, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE INDEX IF NOT EXISTS idx_attendance_register_events_history
    ON attendance_register_events(tenant_id, register_id, created_at, id);

CREATE OR REPLACE FUNCTION enforce_attendance_mark_draft_register()
RETURNS TRIGGER AS $$
DECLARE
    parent_status TEXT;
BEGIN
    SELECT status INTO parent_status
      FROM attendance_registers
     WHERE tenant_id = COALESCE(NEW.tenant_id, OLD.tenant_id)
       AND id = COALESCE(NEW.register_id, OLD.register_id)
       AND deleted_at IS NULL;

    IF parent_status IS DISTINCT FROM 'draft' THEN
        RAISE EXCEPTION 'Attendance marks may change only while the register is draft';
    END IF;
    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS attendance_marks_draft_guard ON attendance_marks;
CREATE TRIGGER attendance_marks_draft_guard
    BEFORE INSERT OR UPDATE OR DELETE ON attendance_marks
    FOR EACH ROW EXECUTE FUNCTION enforce_attendance_mark_draft_register();

CREATE OR REPLACE FUNCTION prevent_attendance_register_event_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Attendance register history is append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS attendance_register_events_append_only ON attendance_register_events;
CREATE TRIGGER attendance_register_events_append_only
    BEFORE UPDATE OR DELETE ON attendance_register_events
    FOR EACH ROW EXECUTE FUNCTION prevent_attendance_register_event_mutation();

DROP TRIGGER IF EXISTS ev_attendance_registers ON attendance_registers;
CREATE TRIGGER ev_attendance_registers
    AFTER INSERT OR UPDATE OR DELETE ON attendance_registers
    FOR EACH ROW EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_attendance_marks ON attendance_marks;
CREATE TRIGGER ev_attendance_marks
    AFTER INSERT OR UPDATE OR DELETE ON attendance_marks
    FOR EACH ROW EXECUTE FUNCTION log_event();

INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
SELECT tenant.id, 'attendance_officer', 'Attendance Officer',
       'Prepares and submits learner attendance registers.',
       ARRAY[
           'attendance:view', 'attendance:create', 'attendance:edit',
           'attendance:delete', 'attendance:submit', 'attendance:manage'
       ]::TEXT[], TRUE
  FROM tenants AS tenant
 WHERE NOT EXISTS (
    SELECT 1 FROM roles AS role
     WHERE role.tenant_id = tenant.id AND role.key = 'attendance_officer'
       AND role.deleted_at IS NULL
 );

CREATE OR REPLACE FUNCTION provision_new_tenant_attendance_officer()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
    VALUES (
        NEW.id, 'attendance_officer', 'Attendance Officer',
        'Prepares and submits learner attendance registers.',
        ARRAY[
            'attendance:view', 'attendance:create', 'attendance:edit',
            'attendance:delete', 'attendance:submit', 'attendance:manage'
        ]::TEXT[], TRUE
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_provision_new_tenant_attendance_officer ON tenants;
CREATE TRIGGER zz_provision_new_tenant_attendance_officer
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION provision_new_tenant_attendance_officer();
