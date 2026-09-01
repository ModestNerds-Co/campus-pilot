-- Communication role, scope, and lifecycle-guard contract. Run after migration 111.

DO $$
DECLARE
    manager_permissions TEXT[] := ARRAY[
        'messaging:create', 'messaging:delete', 'messaging:edit',
        'messaging:manage', 'messaging:send', 'messaging:view'
    ]::TEXT[];
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM roles WHERE key='teacher' AND deleted_at IS NULL
    ) OR NOT EXISTS (
        SELECT 1 FROM roles WHERE key='communication_manager' AND deleted_at IS NULL
    ) THEN
        RAISE EXCEPTION 'Communication role contract requires seeded roles';
    END IF;

    IF EXISTS (
        SELECT 1 FROM roles
         WHERE key='teacher' AND deleted_at IS NULL
           AND (
               NOT permissions @> ARRAY[
                   'messaging:create', 'messaging:edit', 'messaging:view'
               ]::TEXT[]
               OR permissions && ARRAY[
                   'messaging:delete', 'messaging:manage', 'messaging:send'
               ]::TEXT[]
           )
    ) THEN
        RAISE EXCEPTION 'Teacher Communication boundary is invalid';
    END IF;

    IF EXISTS (
        SELECT 1 FROM roles
         WHERE key='communication_manager' AND deleted_at IS NULL
           AND permissions IS DISTINCT FROM manager_permissions
    ) THEN
        RAISE EXCEPTION 'Communication Manager baseline is not exact';
    END IF;
END;
$$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM role_record_scope_grants AS scope_grant
        JOIN roles AS role
          ON role.id=scope_grant.role_id AND role.tenant_id=scope_grant.tenant_id
        WHERE role.key='teacher'
          AND scope_grant.scope_family='messaging.announcements'
          AND scope_grant.scope_kind='assigned'
          AND scope_grant.deleted_at IS NULL
    ) OR NOT EXISTS (
        SELECT 1 FROM role_record_scope_grants AS scope_grant
        JOIN roles AS role
          ON role.id=scope_grant.role_id AND role.tenant_id=scope_grant.tenant_id
        WHERE role.key='communication_manager'
          AND scope_grant.scope_family='messaging.announcements'
          AND scope_grant.scope_kind='campus'
          AND scope_grant.deleted_at IS NULL
    ) OR NOT EXISTS (
        SELECT 1 FROM role_record_scope_grants AS scope_grant
        JOIN roles AS role
          ON role.id=scope_grant.role_id AND role.tenant_id=scope_grant.tenant_id
        WHERE role.key='student'
          AND scope_grant.scope_family='messaging.announcements'
          AND scope_grant.scope_kind='self'
          AND scope_grant.deleted_at IS NULL
    ) THEN
        RAISE EXCEPTION 'Communication record scopes are incomplete';
    END IF;
END;
$$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname='communication_announcements_transition_guard'
           AND NOT tgisinternal
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname='communication_deliveries_transition_guard'
           AND NOT tgisinternal
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname='communication_events_append_only'
           AND NOT tgisinternal
    ) THEN
        RAISE EXCEPTION 'Communication lifecycle guards are incomplete';
    END IF;
END;
$$;

SELECT 'Communication foundation contract passed' AS result;
