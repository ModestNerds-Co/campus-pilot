-- Assignment-scoped Attendance access and immutable submitted-mark evidence.
--
-- Teachers may prepare and submit registers only for classes they currently
-- teach. Attendance Officers retain campus-wide correction and deletion work.

CREATE TABLE IF NOT EXISTS attendance_submission_mark_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    register_id UUID NOT NULL,
    enrolment_id UUID NOT NULL,
    learner_id UUID NOT NULL,
    register_version INTEGER NOT NULL CHECK (register_version > 0),
    attendance_date DATE NOT NULL,
    period TEXT NOT NULL CHECK (period IN ('full_day', 'morning', 'afternoon')),
    mark TEXT NOT NULL CHECK (mark IN ('present', 'absent', 'late', 'excused')),
    minutes_late INTEGER CHECK (minutes_late BETWEEN 0 AND 1440),
    note TEXT CHECK (note IS NULL OR CHAR_LENGTH(BTRIM(note)) BETWEEN 1 AND 1000),
    submitted_by UUID NOT NULL,
    submitted_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, register_id, learner_id, register_version),
    CONSTRAINT attendance_submission_events_register_tenant_fkey
        FOREIGN KEY (register_id, tenant_id)
        REFERENCES attendance_registers(id, tenant_id),
    CONSTRAINT attendance_submission_events_enrolment_tenant_learner_fkey
        FOREIGN KEY (enrolment_id, tenant_id, learner_id)
        REFERENCES enrolments(id, tenant_id, learner_id),
    CONSTRAINT attendance_submission_events_submitter_tenant_fkey
        FOREIGN KEY (submitted_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT attendance_submission_events_late_check CHECK (
        mark = 'late' OR minutes_late IS NULL
    )
);

CREATE INDEX IF NOT EXISTS idx_attendance_submission_events_learner
    ON attendance_submission_mark_events(
        tenant_id, learner_id, attendance_date DESC, submitted_at DESC
    );
CREATE INDEX IF NOT EXISTS idx_attendance_submission_events_register
    ON attendance_submission_mark_events(tenant_id, register_id, register_version);

INSERT INTO attendance_submission_mark_events (
    tenant_id, register_id, enrolment_id, learner_id, register_version,
    attendance_date, period, mark, minutes_late, note, submitted_by, submitted_at
)
SELECT mark.tenant_id, mark.register_id, mark.enrolment_id, mark.learner_id,
       register.version, register.attendance_date, register.period, mark.mark,
       mark.minutes_late, mark.note, register.submitted_by, register.submitted_at
  FROM attendance_marks AS mark
  JOIN attendance_registers AS register
    ON register.id = mark.register_id
   AND register.tenant_id = mark.tenant_id
 WHERE register.status = 'submitted'
   AND register.deleted_at IS NULL
   AND mark.deleted_at IS NULL
ON CONFLICT (tenant_id, register_id, learner_id, register_version) DO NOTHING;

CREATE OR REPLACE FUNCTION prevent_attendance_submission_event_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Submitted attendance mark evidence is append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS attendance_submission_events_append_only
    ON attendance_submission_mark_events;
CREATE TRIGGER attendance_submission_events_append_only
    BEFORE UPDATE OR DELETE ON attendance_submission_mark_events
    FOR EACH ROW EXECUTE FUNCTION prevent_attendance_submission_event_mutation();

UPDATE roles
SET permissions = ARRAY(
        SELECT DISTINCT permission
          FROM UNNEST(
              ARRAY['attendance:view', 'attendance:create', 'attendance:edit',
                    'attendance:submit']::TEXT[]
              || ARRAY(
                  SELECT existing
                    FROM UNNEST(permissions) AS current_permission(existing)
                   WHERE existing NOT IN (
                       'attendance:view', 'attendance:create', 'attendance:edit',
                       'attendance:submit', 'attendance:delete', 'attendance:manage'
                   )
              )
          ) AS expanded(permission)
         ORDER BY permission
    ),
    updated_at = NOW()
WHERE key = 'teacher' AND deleted_at IS NULL;

CREATE OR REPLACE FUNCTION grant_new_tenant_attendance_teacher_permissions()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE roles
       SET permissions = ARRAY(
               SELECT DISTINCT permission
                 FROM UNNEST(
                     permissions || ARRAY[
                         'attendance:view', 'attendance:create',
                         'attendance:edit', 'attendance:submit'
                     ]::TEXT[]
                 ) AS expanded(permission)
                ORDER BY permission
           ),
           updated_at = NOW()
     WHERE tenant_id = NEW.id AND key = 'teacher' AND deleted_at IS NULL;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zzzzzz_grant_new_tenant_attendance_teacher_permissions ON tenants;
CREATE TRIGGER zzzzzz_grant_new_tenant_attendance_teacher_permissions
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION grant_new_tenant_attendance_teacher_permissions();

INSERT INTO role_record_scope_grants (tenant_id, role_id, scope_family, scope_kind)
SELECT role.tenant_id, role.id, 'attendance.registers',
       CASE WHEN role.key = 'teacher' THEN 'assigned' ELSE 'campus' END
  FROM roles AS role
 WHERE role.key IN ('teacher', 'attendance_officer')
   AND role.deleted_at IS NULL
ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
    WHERE deleted_at IS NULL DO NOTHING;

CREATE OR REPLACE FUNCTION provision_attendance_role_record_scopes()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.key IN ('teacher', 'attendance_officer') THEN
        INSERT INTO role_record_scope_grants (
            tenant_id, role_id, scope_family, scope_kind
        ) VALUES (
            NEW.tenant_id, NEW.id, 'attendance.registers',
            CASE WHEN NEW.key = 'teacher' THEN 'assigned' ELSE 'campus' END
        )
        ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
            WHERE deleted_at IS NULL DO NOTHING;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS provision_attendance_role_record_scopes_after_insert ON roles;
CREATE TRIGGER provision_attendance_role_record_scopes_after_insert
    AFTER INSERT ON roles
    FOR EACH ROW EXECUTE FUNCTION provision_attendance_role_record_scopes();

