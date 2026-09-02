-- Attendance lesson-session and exception contract. Run after migration 119.

DO $$
DECLARE
    trigger_count BIGINT;
BEGIN
    SELECT COUNT(*) INTO trigger_count
      FROM pg_trigger
     WHERE tgname IN (
        'attendance_lesson_sessions_lifecycle',
        'attendance_lesson_session_events_append_only',
        'attendance_exceptions_lifecycle',
        'attendance_exception_events_append_only'
     )
       AND NOT tgisinternal;
    IF trigger_count <> 4 THEN
        RAISE EXCEPTION 'Attendance lesson or exception lifecycle guards are missing';
    END IF;

    IF NOT EXISTS (
        SELECT 1
          FROM pg_constraint
         WHERE conrelid = 'attendance_registers'::REGCLASS
           AND conname = 'attendance_registers_period_check'
           AND PG_GET_CONSTRAINTDEF(oid) LIKE '%lesson:%'
    ) THEN
        RAISE EXCEPTION 'Attendance registers do not accept timetable lesson periods';
    END IF;

    IF EXISTS (
        SELECT 1 FROM roles
         WHERE key = 'teacher' AND deleted_at IS NULL
           AND permissions && ARRAY[
               'attendance:manage', 'attendance:delete',
               'academics:create', 'academics:edit', 'academics:manage'
           ]::TEXT[]
    ) THEN
        RAISE EXCEPTION 'a Teacher role has Attendance or Academics administration authority';
    END IF;

    IF EXISTS (
        SELECT 1 FROM roles role
         WHERE role.key = 'teacher' AND role.deleted_at IS NULL
           AND NOT EXISTS (
               SELECT 1 FROM role_record_scope_grants scope_grant
                WHERE scope_grant.tenant_id = role.tenant_id
                  AND scope_grant.role_id = role.id
                  AND scope_grant.scope_family = 'attendance.registers'
                  AND scope_grant.scope_kind = 'assigned'
                  AND scope_grant.deleted_at IS NULL
           )
    ) THEN
        RAISE EXCEPTION 'a Teacher role is missing assigned Attendance scope';
    END IF;

    IF EXISTS (
        SELECT 1 FROM roles role
         WHERE role.key = 'attendance_officer' AND role.deleted_at IS NULL
           AND NOT EXISTS (
               SELECT 1 FROM role_record_scope_grants scope_grant
                WHERE scope_grant.tenant_id = role.tenant_id
                  AND scope_grant.role_id = role.id
                  AND scope_grant.scope_family = 'attendance.registers'
                  AND scope_grant.scope_kind = 'campus'
                  AND scope_grant.deleted_at IS NULL
           )
    ) THEN
        RAISE EXCEPTION 'an Attendance Officer is missing campus Attendance scope';
    END IF;
END;
$$;

CREATE TEMP TABLE attendance_lesson_sessions (
    tenant_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    class_group_id UUID NOT NULL,
    teaching_assignment_id UUID NOT NULL,
    timetable_run_id UUID NOT NULL,
    timetable_requirement_id TEXT NOT NULL,
    session_date DATE NOT NULL,
    day_key TEXT NOT NULL,
    period_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'scheduled',
    version INTEGER NOT NULL DEFAULT 1,
    register_id UUID,
    created_by UUID NOT NULL,
    opened_by UUID,
    opened_at TIMESTAMPTZ,
    completed_by UUID,
    completed_at TIMESTAMPTZ,
    cancelled_by UUID,
    cancelled_at TIMESTAMPTZ,
    cancellation_reason TEXT,
    deleted_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ
);
CREATE TRIGGER verify_attendance_lesson_sessions_lifecycle
    BEFORE UPDATE ON attendance_lesson_sessions
    FOR EACH ROW EXECUTE FUNCTION enforce_attendance_lesson_session_lifecycle();

INSERT INTO attendance_lesson_sessions (
    tenant_id, academic_term_id, class_group_id, teaching_assignment_id,
    timetable_run_id, timetable_requirement_id, session_date, day_key,
    period_key, created_by
) VALUES (
    gen_random_uuid(), gen_random_uuid(), gen_random_uuid(), gen_random_uuid(),
    gen_random_uuid(), gen_random_uuid()::TEXT, CURRENT_DATE, 'monday',
    'period-1', gen_random_uuid()
);

UPDATE attendance_lesson_sessions
   SET status = 'open', register_id = gen_random_uuid(),
       opened_by = gen_random_uuid(), opened_at = NOW(), version = version + 1;

DO $$
BEGIN
    BEGIN
        UPDATE attendance_lesson_sessions
           SET class_group_id = gen_random_uuid(),
               status = 'completed', completed_by = gen_random_uuid(),
               completed_at = NOW(), version = version + 1;
        RAISE EXCEPTION 'Attendance lesson source identity mutation was accepted';
    EXCEPTION
        WHEN check_violation THEN NULL;
    END;

    BEGIN
        UPDATE attendance_lesson_sessions
           SET status = 'cancelled', register_id = NULL,
               opened_by = NULL, opened_at = NULL,
               cancelled_by = gen_random_uuid(), cancelled_at = NOW(),
               cancellation_reason = 'Cancelled', version = version + 1;
        RAISE EXCEPTION 'An open Attendance lesson was cancelled';
    EXCEPTION
        WHEN check_violation THEN NULL;
    END;
END;
$$;

UPDATE attendance_lesson_sessions
   SET status = 'completed', completed_by = gen_random_uuid(),
       completed_at = NOW(), version = version + 1;
UPDATE attendance_lesson_sessions
   SET status = 'open', completed_by = NULL, completed_at = NULL,
       version = version + 1;

CREATE TEMP TABLE attendance_exceptions (
    tenant_id UUID NOT NULL,
    register_id UUID NOT NULL,
    enrolment_id UUID NOT NULL,
    learner_id UUID NOT NULL,
    class_group_id UUID NOT NULL,
    source_register_version INTEGER NOT NULL,
    attendance_date DATE NOT NULL,
    period TEXT NOT NULL,
    mark TEXT NOT NULL,
    minutes_late INTEGER,
    attendance_note TEXT,
    source_submitted_at TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL DEFAULT 'open',
    version INTEGER NOT NULL DEFAULT 1,
    acknowledged_by UUID,
    acknowledged_at TIMESTAMPTZ,
    acknowledgement_note TEXT,
    resolved_by UUID,
    resolved_at TIMESTAMPTZ,
    resolution TEXT,
    reopened_by UUID,
    reopened_at TIMESTAMPTZ,
    reopen_reason TEXT,
    deleted_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ
);
CREATE TRIGGER verify_attendance_exceptions_lifecycle
    BEFORE UPDATE ON attendance_exceptions
    FOR EACH ROW EXECUTE FUNCTION enforce_attendance_exception_lifecycle();

INSERT INTO attendance_exceptions (
    tenant_id, register_id, enrolment_id, learner_id, class_group_id,
    source_register_version, attendance_date, period, mark,
    source_submitted_at
) VALUES (
    gen_random_uuid(), gen_random_uuid(), gen_random_uuid(), gen_random_uuid(),
    gen_random_uuid(), 1, CURRENT_DATE, 'full_day', 'absent', NOW()
);

UPDATE attendance_exceptions
   SET status = 'acknowledged', acknowledged_by = gen_random_uuid(),
       acknowledged_at = NOW(), acknowledgement_note = 'Contact pending',
       version = version + 1;
UPDATE attendance_exceptions
   SET status = 'resolved', resolved_by = gen_random_uuid(), resolved_at = NOW(),
       resolution = 'Follow-up complete', version = version + 1;
UPDATE attendance_exceptions
   SET status = 'open', acknowledged_by = NULL, acknowledged_at = NULL,
       acknowledgement_note = NULL, resolved_by = NULL, resolved_at = NULL,
       resolution = NULL, reopened_by = gen_random_uuid(), reopened_at = NOW(),
       reopen_reason = 'New evidence', version = version + 1;

DO $$
BEGIN
    BEGIN
        UPDATE attendance_exceptions
           SET register_id = gen_random_uuid(), version = version + 1;
        RAISE EXCEPTION 'Attendance exception identity mutation was accepted';
    EXCEPTION
        WHEN check_violation THEN NULL;
    END;
END;
$$;

CREATE TEMP TABLE attendance_exception_events (id UUID DEFAULT gen_random_uuid());
CREATE TRIGGER verify_attendance_exception_events_append_only
    BEFORE UPDATE OR DELETE ON attendance_exception_events
    FOR EACH ROW EXECUTE FUNCTION prevent_attendance_workflow_event_mutation();
INSERT INTO attendance_exception_events DEFAULT VALUES;

DO $$
BEGIN
    BEGIN
        DELETE FROM attendance_exception_events;
        RAISE EXCEPTION 'Attendance exception history deletion was accepted';
    EXCEPTION
        WHEN raise_exception THEN NULL;
    END;
END;
$$;

SELECT 'Attendance sessions and exceptions contract passed' AS result;
