-- Assignment-scoped Attendance and immutable history contract. Run after 105.

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM roles
         WHERE key = 'teacher' AND deleted_at IS NULL
           AND (
               NOT permissions @> ARRAY[
                   'attendance:view', 'attendance:create',
                   'attendance:edit', 'attendance:submit'
               ]::TEXT[]
               OR permissions && ARRAY['attendance:delete', 'attendance:manage']::TEXT[]
           )
    ) THEN
        RAISE EXCEPTION 'a Teacher role has an invalid Attendance authority boundary';
    END IF;

    IF EXISTS (
        SELECT 1 FROM roles AS role
         WHERE role.key IN ('teacher', 'attendance_officer')
           AND role.deleted_at IS NULL
           AND NOT EXISTS (
               SELECT 1 FROM role_record_scope_grants AS scope_grant
                WHERE scope_grant.tenant_id = role.tenant_id
                  AND scope_grant.role_id = role.id
                  AND scope_grant.scope_family = 'attendance.registers'
                  AND scope_grant.scope_kind = CASE
                      WHEN role.key = 'teacher' THEN 'assigned'
                      ELSE 'campus'
                  END
                  AND scope_grant.deleted_at IS NULL
           )
    ) THEN
        RAISE EXCEPTION 'an Attendance role is missing its record scope';
    END IF;
END;
$$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname = 'attendance_submission_events_append_only'
           AND NOT tgisinternal
    ) THEN
        RAISE EXCEPTION 'submitted Attendance evidence is not append-only';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes
         WHERE indexname = 'idx_attendance_submission_events_learner'
    ) THEN
        RAISE EXCEPTION 'learner Attendance history index is missing';
    END IF;
END;
$$;

SELECT 'Attendance access and history contract passed' AS result;
