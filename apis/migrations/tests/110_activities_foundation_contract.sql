-- Activities access and immutable-evidence contract. Run after migration 110.

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM roles
         WHERE key = 'teacher' AND deleted_at IS NULL
           AND permissions && ARRAY['activities:view','activities:operate','activities:manage']::TEXT[]
    ) THEN
        RAISE EXCEPTION 'Teacher received Activities authority';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM roles
         WHERE key = 'activity_leader' AND deleted_at IS NULL
           AND permissions @> ARRAY['activities:view','activities:operate']::TEXT[]
           AND NOT ('activities:manage' = ANY(permissions))
    ) THEN
        RAISE EXCEPTION 'Activity Leader boundary is missing or over-privileged';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM roles
         WHERE key = 'activities_coordinator' AND deleted_at IS NULL
           AND permissions @> ARRAY['activities:view','activities:operate','activities:manage']::TEXT[]
    ) THEN
        RAISE EXCEPTION 'Activities Coordinator authority is missing';
    END IF;

    IF EXISTS (
        SELECT 1 FROM roles
         WHERE key = 'student' AND deleted_at IS NULL
           AND permissions && ARRAY['activities:operate','activities:manage']::TEXT[]
    ) OR NOT EXISTS (
        SELECT 1 FROM roles
         WHERE key = 'student' AND deleted_at IS NULL
           AND 'activities:view' = ANY(permissions)
    ) THEN
        RAISE EXCEPTION 'Student Activities boundary is missing or over-privileged';
    END IF;
END;
$$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM role_record_scope_grants AS scope_grant
        JOIN roles AS role ON role.id=scope_grant.role_id AND role.tenant_id=scope_grant.tenant_id
        WHERE role.key='activity_leader' AND scope_grant.scope_family='activities.groups'
          AND scope_grant.scope_kind='assigned' AND scope_grant.deleted_at IS NULL
    ) OR NOT EXISTS (
        SELECT 1 FROM role_record_scope_grants AS scope_grant
        JOIN roles AS role ON role.id=scope_grant.role_id AND role.tenant_id=scope_grant.tenant_id
        WHERE role.key='student' AND scope_grant.scope_family='activities.sessions'
          AND scope_grant.scope_kind='self' AND scope_grant.deleted_at IS NULL
    ) OR NOT EXISTS (
        SELECT 1 FROM role_record_scope_grants AS scope_grant
        JOIN roles AS role ON role.id=scope_grant.role_id AND role.tenant_id=scope_grant.tenant_id
        WHERE role.key='activities_coordinator' AND scope_grant.scope_family='activities.groups'
          AND scope_grant.scope_kind='campus' AND scope_grant.deleted_at IS NULL
    ) THEN
        RAISE EXCEPTION 'Activities role record scopes are incomplete';
    END IF;
END;
$$;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname='activity_completion_snapshots_append_only' AND NOT tgisinternal)
       OR NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname='activity_completion_members_append_only' AND NOT tgisinternal)
       OR NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname='activity_lifecycle_events_append_only' AND NOT tgisinternal)
    THEN
        RAISE EXCEPTION 'Activities immutable evidence triggers are missing';
    END IF;
END;
$$;

SELECT 'Activities foundation contract passed' AS result;
