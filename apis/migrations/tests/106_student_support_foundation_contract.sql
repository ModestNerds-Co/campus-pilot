-- Student Support access, lifecycle, and evidence contract checks.

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM roles
        WHERE key IN ('teacher', 'school_administrator', 'registrar', 'student', 'guardian')
          AND deleted_at IS NULL
          AND permissions && ARRAY[
              'student_support:view', 'student_support:create',
              'student_support:edit', 'student_support:manage'
          ]::TEXT[]
    ) THEN
        RAISE EXCEPTION 'an ordinary campus role has Student Support case authority';
    END IF;

    IF EXISTS (
        SELECT 1 FROM roles
        WHERE key = 'student_support_case_worker'
          AND deleted_at IS NULL
          AND (
              permissions <> ARRAY[
                  'student_support:view', 'student_support:create', 'student_support:edit'
              ]::TEXT[]
              OR 'student_support:manage' = ANY(permissions)
          )
    ) THEN
        RAISE EXCEPTION 'a Student Support Case Worker has an invalid permission boundary';
    END IF;

    IF EXISTS (
        SELECT 1 FROM roles
        WHERE key = 'student_support_manager'
          AND deleted_at IS NULL
          AND NOT permissions @> ARRAY[
              'student_support:view', 'student_support:create',
              'student_support:edit', 'student_support:manage'
          ]::TEXT[]
    ) THEN
        RAISE EXCEPTION 'a Student Support Manager is missing management authority';
    END IF;
END;
$$;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM roles AS role
        WHERE role.key IN ('student_support_case_worker', 'student_support_manager')
          AND role.deleted_at IS NULL
          AND NOT EXISTS (
              SELECT 1
              FROM role_record_scope_grants AS scope_grant
              WHERE scope_grant.tenant_id = role.tenant_id
                AND scope_grant.role_id = role.id
                AND scope_grant.scope_family = 'student_support.cases'
                AND scope_grant.scope_kind = CASE
                    WHEN role.key = 'student_support_manager' THEN 'campus'
                    ELSE 'assigned'
                END
                AND scope_grant.deleted_at IS NULL
          )
    ) THEN
        RAISE EXCEPTION 'a Student Support role is missing its record scope';
    END IF;
END;
$$;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM tenants AS tenant
        WHERE NOT EXISTS (
            SELECT 1 FROM student_support_numbering_policies AS policy
            WHERE policy.tenant_id = tenant.id AND policy.deleted_at IS NULL
        )
    ) THEN
        RAISE EXCEPTION 'a tenant is missing Student Support numbering';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger
        WHERE tgname = 'student_support_actions_append_only' AND NOT tgisinternal
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_trigger
        WHERE tgname = 'student_support_events_append_only' AND NOT tgisinternal
    ) THEN
        RAISE EXCEPTION 'Student Support append-only evidence is missing';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE indexname = 'idx_student_support_case_team_active'
    ) THEN
        RAISE EXCEPTION 'Student Support active case-team uniqueness is missing';
    END IF;
END;
$$;

SELECT 'Student Support foundation contract passed' AS result;
