-- Timetable-linked lesson attendance and campus exception follow-up.
--
-- Timetabling remains the immutable source of lesson occurrences. Attendance
-- materialises operational sessions, owns their registers, and retains
-- absence evidence without claiming that a guardian notification was sent.

ALTER TABLE attendance_registers
    DROP CONSTRAINT IF EXISTS attendance_registers_period_check;
ALTER TABLE attendance_registers
    ADD CONSTRAINT attendance_registers_period_check CHECK (
        period IN ('full_day', 'morning', 'afternoon')
        OR (
            period LIKE 'lesson:%'
            AND CHAR_LENGTH(period) BETWEEN 8 AND 128
        )
    );

ALTER TABLE attendance_submission_mark_events
    DROP CONSTRAINT IF EXISTS attendance_submission_mark_events_period_check;
ALTER TABLE attendance_submission_mark_events
    ADD CONSTRAINT attendance_submission_mark_events_period_check CHECK (
        period IN ('full_day', 'morning', 'afternoon')
        OR (
            period LIKE 'lesson:%'
            AND CHAR_LENGTH(period) BETWEEN 8 AND 128
        )
    );

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
          FROM pg_constraint
         WHERE conname = 'timetable_runs_id_tenant_id_key'
    ) THEN
        ALTER TABLE timetable_runs
            ADD CONSTRAINT timetable_runs_id_tenant_id_key
            UNIQUE (id, tenant_id);
    END IF;
END;
$$;

CREATE TABLE IF NOT EXISTS attendance_lesson_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    academic_term_id UUID NOT NULL,
    class_group_id UUID NOT NULL,
    teaching_assignment_id UUID NOT NULL,
    timetable_run_id UUID NOT NULL,
    timetable_requirement_id TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(timetable_requirement_id)) BETWEEN 1 AND 100),
    session_date DATE NOT NULL,
    day_key TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(day_key)) BETWEEN 1 AND 80),
    period_key TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(period_key)) BETWEEN 1 AND 120),
    status TEXT NOT NULL DEFAULT 'scheduled'
        CHECK (status IN ('scheduled', 'open', 'completed', 'cancelled')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    register_id UUID,
    created_by UUID NOT NULL,
    opened_by UUID,
    opened_at TIMESTAMPTZ,
    completed_by UUID,
    completed_at TIMESTAMPTZ,
    cancelled_by UUID,
    cancelled_at TIMESTAMPTZ,
    cancellation_reason TEXT CHECK (
        cancellation_reason IS NULL
        OR CHAR_LENGTH(BTRIM(cancellation_reason)) BETWEEN 1 AND 1000
    ),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CONSTRAINT attendance_lesson_sessions_term_tenant_fkey
        FOREIGN KEY (academic_term_id, tenant_id)
        REFERENCES academic_terms(id, tenant_id),
    CONSTRAINT attendance_lesson_sessions_class_tenant_fkey
        FOREIGN KEY (class_group_id, tenant_id)
        REFERENCES class_groups(id, tenant_id),
    CONSTRAINT attendance_lesson_sessions_assignment_tenant_fkey
        FOREIGN KEY (teaching_assignment_id, tenant_id)
        REFERENCES teaching_assignments(id, tenant_id),
    CONSTRAINT attendance_lesson_sessions_timetable_tenant_fkey
        FOREIGN KEY (timetable_run_id, tenant_id)
        REFERENCES timetable_runs(id, tenant_id),
    CONSTRAINT attendance_lesson_sessions_register_tenant_fkey
        FOREIGN KEY (register_id, tenant_id)
        REFERENCES attendance_registers(id, tenant_id),
    CONSTRAINT attendance_lesson_sessions_creator_tenant_fkey
        FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT attendance_lesson_sessions_opener_tenant_fkey
        FOREIGN KEY (opened_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT attendance_lesson_sessions_completer_tenant_fkey
        FOREIGN KEY (completed_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT attendance_lesson_sessions_canceller_tenant_fkey
        FOREIGN KEY (cancelled_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT attendance_lesson_sessions_lifecycle_check CHECK (
        (
            status = 'scheduled'
            AND register_id IS NULL
            AND opened_by IS NULL AND opened_at IS NULL
            AND completed_by IS NULL AND completed_at IS NULL
            AND cancelled_by IS NULL AND cancelled_at IS NULL
            AND cancellation_reason IS NULL
        )
        OR (
            status = 'open'
            AND register_id IS NOT NULL
            AND opened_by IS NOT NULL AND opened_at IS NOT NULL
            AND completed_by IS NULL AND completed_at IS NULL
            AND cancelled_by IS NULL AND cancelled_at IS NULL
            AND cancellation_reason IS NULL
        )
        OR (
            status = 'completed'
            AND register_id IS NOT NULL
            AND opened_by IS NOT NULL AND opened_at IS NOT NULL
            AND completed_by IS NOT NULL AND completed_at IS NOT NULL
            AND cancelled_by IS NULL AND cancelled_at IS NULL
            AND cancellation_reason IS NULL
        )
        OR (
            status = 'cancelled'
            AND register_id IS NULL
            AND opened_by IS NULL AND opened_at IS NULL
            AND completed_by IS NULL AND completed_at IS NULL
            AND cancelled_by IS NOT NULL AND cancelled_at IS NOT NULL
            AND cancellation_reason IS NOT NULL
        )
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_attendance_lesson_sessions_occurrence
    ON attendance_lesson_sessions(
        tenant_id, timetable_run_id, session_date, day_key, period_key,
        timetable_requirement_id
    ) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_attendance_lesson_sessions_register
    ON attendance_lesson_sessions(tenant_id, register_id)
    WHERE register_id IS NOT NULL AND deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_attendance_lesson_sessions_worklist
    ON attendance_lesson_sessions(tenant_id, session_date, status, period_key, id)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_attendance_lesson_sessions_assignment
    ON attendance_lesson_sessions(tenant_id, teaching_assignment_id, session_date)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_attendance_lesson_sessions_updated_at
    ON attendance_lesson_sessions;
CREATE TRIGGER update_attendance_lesson_sessions_updated_at
    BEFORE UPDATE ON attendance_lesson_sessions
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE OR REPLACE FUNCTION enforce_attendance_lesson_session_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.tenant_id <> OLD.tenant_id
       OR NEW.academic_term_id <> OLD.academic_term_id
       OR NEW.class_group_id <> OLD.class_group_id
       OR NEW.teaching_assignment_id <> OLD.teaching_assignment_id
       OR NEW.timetable_run_id <> OLD.timetable_run_id
       OR NEW.timetable_requirement_id <> OLD.timetable_requirement_id
       OR NEW.session_date <> OLD.session_date
       OR NEW.day_key <> OLD.day_key
       OR NEW.period_key <> OLD.period_key
       OR NEW.created_by <> OLD.created_by THEN
        RAISE EXCEPTION 'Attendance lesson session source identity is immutable'
            USING ERRCODE = 'check_violation';
    END IF;
    IF NEW.version <> OLD.version + 1 THEN
        RAISE EXCEPTION 'Attendance lesson session version must increase by one'
            USING ERRCODE = 'check_violation';
    END IF;
    IF NOT (
        (OLD.status = 'scheduled' AND NEW.status IN ('open', 'cancelled'))
        OR (OLD.status = 'open' AND NEW.status IN ('scheduled', 'completed'))
        OR (OLD.status = 'completed' AND NEW.status = 'open')
    ) THEN
        RAISE EXCEPTION 'Invalid Attendance lesson session transition'
            USING ERRCODE = 'check_violation';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS attendance_lesson_sessions_lifecycle
    ON attendance_lesson_sessions;
CREATE TRIGGER attendance_lesson_sessions_lifecycle
    BEFORE UPDATE ON attendance_lesson_sessions
    FOR EACH ROW
    WHEN (OLD.deleted_at IS NULL AND NEW.deleted_at IS NULL)
    EXECUTE FUNCTION enforce_attendance_lesson_session_lifecycle();

CREATE TABLE IF NOT EXISTS attendance_lesson_session_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    lesson_session_id UUID NOT NULL,
    event_type TEXT NOT NULL CHECK (
        event_type IN ('scheduled', 'opened', 'completed', 'reopened', 'cancelled', 'register_deleted')
    ),
    from_status TEXT CHECK (
        from_status IS NULL OR from_status IN ('scheduled', 'open', 'completed', 'cancelled')
    ),
    to_status TEXT NOT NULL
        CHECK (to_status IN ('scheduled', 'open', 'completed', 'cancelled')),
    session_version INTEGER NOT NULL CHECK (session_version > 0),
    actor_id UUID NOT NULL,
    reason TEXT CHECK (
        reason IS NULL OR CHAR_LENGTH(BTRIM(reason)) BETWEEN 1 AND 1000
    ),
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CONSTRAINT attendance_lesson_session_events_parent_tenant_fkey
        FOREIGN KEY (lesson_session_id, tenant_id)
        REFERENCES attendance_lesson_sessions(id, tenant_id),
    CONSTRAINT attendance_lesson_session_events_actor_tenant_fkey
        FOREIGN KEY (actor_id, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE INDEX IF NOT EXISTS idx_attendance_lesson_session_events_history
    ON attendance_lesson_session_events(
        tenant_id, lesson_session_id, created_at, id
    );

CREATE TABLE IF NOT EXISTS attendance_exceptions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    register_id UUID NOT NULL,
    enrolment_id UUID NOT NULL,
    learner_id UUID NOT NULL,
    class_group_id UUID NOT NULL,
    source_register_version INTEGER NOT NULL CHECK (source_register_version > 0),
    attendance_date DATE NOT NULL,
    period TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(period)) BETWEEN 1 AND 128),
    mark TEXT NOT NULL CHECK (mark IN ('absent', 'late', 'excused')),
    minutes_late INTEGER CHECK (minutes_late BETWEEN 0 AND 1440),
    attendance_note TEXT CHECK (
        attendance_note IS NULL
        OR CHAR_LENGTH(BTRIM(attendance_note)) BETWEEN 1 AND 1000
    ),
    source_submitted_at TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open', 'acknowledged', 'resolved')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    acknowledged_by UUID,
    acknowledged_at TIMESTAMPTZ,
    acknowledgement_note TEXT CHECK (
        acknowledgement_note IS NULL
        OR CHAR_LENGTH(BTRIM(acknowledgement_note)) BETWEEN 1 AND 1000
    ),
    resolved_by UUID,
    resolved_at TIMESTAMPTZ,
    resolution TEXT CHECK (
        resolution IS NULL OR CHAR_LENGTH(BTRIM(resolution)) BETWEEN 1 AND 2000
    ),
    reopened_by UUID,
    reopened_at TIMESTAMPTZ,
    reopen_reason TEXT CHECK (
        reopen_reason IS NULL OR CHAR_LENGTH(BTRIM(reopen_reason)) BETWEEN 1 AND 1000
    ),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CONSTRAINT attendance_exceptions_register_tenant_fkey
        FOREIGN KEY (register_id, tenant_id)
        REFERENCES attendance_registers(id, tenant_id),
    CONSTRAINT attendance_exceptions_enrolment_tenant_learner_fkey
        FOREIGN KEY (enrolment_id, tenant_id, learner_id)
        REFERENCES enrolments(id, tenant_id, learner_id),
    CONSTRAINT attendance_exceptions_class_tenant_fkey
        FOREIGN KEY (class_group_id, tenant_id)
        REFERENCES class_groups(id, tenant_id),
    CONSTRAINT attendance_exceptions_acknowledger_tenant_fkey
        FOREIGN KEY (acknowledged_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT attendance_exceptions_resolver_tenant_fkey
        FOREIGN KEY (resolved_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT attendance_exceptions_reopener_tenant_fkey
        FOREIGN KEY (reopened_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT attendance_exceptions_lifecycle_check CHECK (
        (
            status = 'open'
            AND acknowledged_by IS NULL AND acknowledged_at IS NULL
            AND acknowledgement_note IS NULL
            AND resolved_by IS NULL AND resolved_at IS NULL AND resolution IS NULL
        )
        OR (
            status = 'acknowledged'
            AND acknowledged_by IS NOT NULL AND acknowledged_at IS NOT NULL
            AND resolved_by IS NULL AND resolved_at IS NULL AND resolution IS NULL
        )
        OR (
            status = 'resolved'
            AND resolved_by IS NOT NULL AND resolved_at IS NOT NULL
            AND resolution IS NOT NULL
        )
    ),
    CONSTRAINT attendance_exceptions_reopen_check CHECK (
        (reopened_by IS NULL AND reopened_at IS NULL AND reopen_reason IS NULL)
        OR (reopened_by IS NOT NULL AND reopened_at IS NOT NULL AND reopen_reason IS NOT NULL)
    ),
    CONSTRAINT attendance_exceptions_late_check CHECK (
        mark = 'late' OR minutes_late IS NULL
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_attendance_exceptions_current
    ON attendance_exceptions(tenant_id, register_id, learner_id)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_attendance_exceptions_worklist
    ON attendance_exceptions(tenant_id, status, attendance_date DESC, id)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_attendance_exceptions_learner
    ON attendance_exceptions(tenant_id, learner_id, attendance_date DESC)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_attendance_exceptions_updated_at
    ON attendance_exceptions;
CREATE TRIGGER update_attendance_exceptions_updated_at
    BEFORE UPDATE ON attendance_exceptions
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE OR REPLACE FUNCTION enforce_attendance_exception_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.tenant_id <> OLD.tenant_id
       OR NEW.register_id <> OLD.register_id
       OR NEW.enrolment_id <> OLD.enrolment_id
       OR NEW.learner_id <> OLD.learner_id
       OR NEW.class_group_id <> OLD.class_group_id THEN
        RAISE EXCEPTION 'Attendance exception identity is immutable'
            USING ERRCODE = 'check_violation';
    END IF;
    IF NEW.version <> OLD.version + 1 THEN
        RAISE EXCEPTION 'Attendance exception version must increase by one'
            USING ERRCODE = 'check_violation';
    END IF;
    IF NOT (
        (OLD.status = 'open' AND NEW.status IN ('open', 'acknowledged', 'resolved'))
        OR (OLD.status = 'acknowledged' AND NEW.status IN ('open', 'resolved'))
        OR (OLD.status = 'resolved' AND NEW.status = 'open')
    ) THEN
        RAISE EXCEPTION 'Invalid Attendance exception transition'
            USING ERRCODE = 'check_violation';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS attendance_exceptions_lifecycle ON attendance_exceptions;
CREATE TRIGGER attendance_exceptions_lifecycle
    BEFORE UPDATE ON attendance_exceptions
    FOR EACH ROW
    WHEN (OLD.deleted_at IS NULL AND NEW.deleted_at IS NULL)
    EXECUTE FUNCTION enforce_attendance_exception_lifecycle();

CREATE TABLE IF NOT EXISTS attendance_exception_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    exception_id UUID NOT NULL,
    event_type TEXT NOT NULL CHECK (
        event_type IN ('created', 'evidence_refreshed', 'acknowledged', 'resolved', 'reopened', 'auto_resolved')
    ),
    from_status TEXT CHECK (
        from_status IS NULL OR from_status IN ('open', 'acknowledged', 'resolved')
    ),
    to_status TEXT NOT NULL CHECK (to_status IN ('open', 'acknowledged', 'resolved')),
    exception_version INTEGER NOT NULL CHECK (exception_version > 0),
    actor_id UUID NOT NULL,
    reason TEXT CHECK (
        reason IS NULL OR CHAR_LENGTH(BTRIM(reason)) BETWEEN 1 AND 2000
    ),
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CONSTRAINT attendance_exception_events_parent_tenant_fkey
        FOREIGN KEY (exception_id, tenant_id)
        REFERENCES attendance_exceptions(id, tenant_id),
    CONSTRAINT attendance_exception_events_actor_tenant_fkey
        FOREIGN KEY (actor_id, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE INDEX IF NOT EXISTS idx_attendance_exception_events_history
    ON attendance_exception_events(tenant_id, exception_id, created_at, id);

CREATE OR REPLACE FUNCTION prevent_attendance_workflow_event_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Attendance workflow history is append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS attendance_lesson_session_events_append_only
    ON attendance_lesson_session_events;
CREATE TRIGGER attendance_lesson_session_events_append_only
    BEFORE UPDATE OR DELETE ON attendance_lesson_session_events
    FOR EACH ROW EXECUTE FUNCTION prevent_attendance_workflow_event_mutation();

DROP TRIGGER IF EXISTS attendance_exception_events_append_only
    ON attendance_exception_events;
CREATE TRIGGER attendance_exception_events_append_only
    BEFORE UPDATE OR DELETE ON attendance_exception_events
    FOR EACH ROW EXECUTE FUNCTION prevent_attendance_workflow_event_mutation();

DROP TRIGGER IF EXISTS ev_attendance_lesson_sessions ON attendance_lesson_sessions;
CREATE TRIGGER ev_attendance_lesson_sessions
    AFTER INSERT OR UPDATE OR DELETE ON attendance_lesson_sessions
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_attendance_exceptions ON attendance_exceptions;
CREATE TRIGGER ev_attendance_exceptions
    AFTER INSERT OR UPDATE OR DELETE ON attendance_exceptions
    FOR EACH ROW EXECUTE FUNCTION log_event();

-- Rehydrate the current operational follow-up queue from accepted evidence.
INSERT INTO attendance_exceptions (
    tenant_id, register_id, enrolment_id, learner_id, class_group_id,
    source_register_version, attendance_date, period, mark, minutes_late,
    attendance_note, source_submitted_at
)
SELECT event.tenant_id, event.register_id, event.enrolment_id, event.learner_id,
       register.class_group_id, event.register_version, event.attendance_date,
       event.period, event.mark, event.minutes_late, event.note, event.submitted_at
  FROM attendance_submission_mark_events event
  JOIN attendance_registers register
    ON register.id = event.register_id
   AND register.tenant_id = event.tenant_id
   AND register.status = 'submitted'
   AND register.version = event.register_version
   AND register.deleted_at IS NULL
 WHERE event.mark IN ('absent', 'late', 'excused')
ON CONFLICT (tenant_id, register_id, learner_id)
    WHERE deleted_at IS NULL DO NOTHING;

INSERT INTO attendance_exception_events (
    tenant_id, exception_id, event_type, from_status, to_status,
    exception_version, actor_id, metadata
)
SELECT exception.tenant_id, exception.id, 'created', NULL, 'open',
       exception.version, event.submitted_by,
       JSONB_BUILD_OBJECT(
           'register_id', exception.register_id,
           'learner_id', exception.learner_id,
           'mark', exception.mark,
           'backfilled', TRUE
       )
  FROM attendance_exceptions exception
  JOIN attendance_submission_mark_events event
    ON event.tenant_id = exception.tenant_id
   AND event.register_id = exception.register_id
   AND event.learner_id = exception.learner_id
   AND event.register_version = exception.source_register_version
 WHERE NOT EXISTS (
    SELECT 1 FROM attendance_exception_events existing
     WHERE existing.tenant_id = exception.tenant_id
       AND existing.exception_id = exception.id
 );
