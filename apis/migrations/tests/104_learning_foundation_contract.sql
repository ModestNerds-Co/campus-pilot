-- E-learning foundation contract checks. Run after migration 104.

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM roles
        WHERE key = 'teacher'
          AND deleted_at IS NULL
          AND (
              NOT permissions @> ARRAY['learning:view', 'learning:teach']::TEXT[]
              OR 'learning:manage' = ANY(permissions)
          )
    ) THEN
        RAISE EXCEPTION 'a Teacher role has an invalid Learning authority boundary';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM roles
        WHERE key = 'student'
          AND deleted_at IS NULL
          AND (
              NOT ('learning:view' = ANY(permissions))
              OR permissions && ARRAY['learning:teach', 'learning:manage']::TEXT[]
          )
    ) THEN
        RAISE EXCEPTION 'a Student role has an invalid Learning authority boundary';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM roles
        WHERE key = 'academic_manager'
          AND deleted_at IS NULL
          AND NOT permissions @> ARRAY[
              'learning:view', 'learning:teach', 'learning:manage'
          ]::TEXT[]
    ) THEN
        RAISE EXCEPTION 'an Academic Manager role is missing Learning authority';
    END IF;
END;
$$;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM roles AS role
        WHERE role.key IN ('teacher', 'student', 'academic_manager')
          AND role.deleted_at IS NULL
          AND NOT EXISTS (
              SELECT 1
              FROM role_record_scope_grants AS scope_grant
              WHERE scope_grant.tenant_id = role.tenant_id
                AND scope_grant.role_id = role.id
                AND scope_grant.scope_family = 'learning.spaces'
                AND scope_grant.scope_kind = CASE role.key
                    WHEN 'teacher' THEN 'assigned'
                    WHEN 'student' THEN 'self'
                    ELSE 'campus'
                END
                AND scope_grant.deleted_at IS NULL
          )
    ) THEN
        RAISE EXCEPTION 'a Learning role is missing its record scope';
    END IF;
END;
$$;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM tenants AS tenant
        WHERE NOT EXISTS (
            SELECT 1
            FROM learning_settings AS settings
            WHERE settings.tenant_id = tenant.id
              AND settings.deleted_at IS NULL
        )
    ) THEN
        RAISE EXCEPTION 'a tenant is missing Learning settings';
    END IF;
END;
$$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger
        WHERE tgname = 'learning_activity_append_only' AND NOT tgisinternal
    ) THEN
        RAISE EXCEPTION 'Learning append-only activity evidence is missing';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE indexname = 'idx_learning_spaces_assignment_term'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE indexname = 'idx_learning_units_position'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE indexname = 'idx_learning_resources_position'
    ) THEN
        RAISE EXCEPTION 'Learning active-record uniqueness is missing';
    END IF;
END;
$$;

SELECT 'E-learning foundation contract passed' AS result;
