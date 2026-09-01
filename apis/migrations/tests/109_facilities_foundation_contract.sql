-- Facilities foundation contract checks. Run after migration 109.

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM roles
        WHERE key = 'teacher' AND deleted_at IS NULL
          AND 'facilities:request' = ANY(permissions)
          AND NOT (permissions && ARRAY['facilities:operate', 'facilities:manage']::TEXT[])
    ) THEN
        RAISE EXCEPTION 'Teacher Facilities request boundary is missing or over-privileged';
    END IF;
    IF EXISTS (
        SELECT 1 FROM roles
        WHERE key = 'facilities_officer' AND deleted_at IS NULL
          AND 'facilities:manage' = ANY(permissions)
    ) THEN
        RAISE EXCEPTION 'Facilities Officer has manager authority';
    END IF;
END;
$$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM role_record_scope_grants AS scope_grant
        JOIN roles AS role
          ON role.id = scope_grant.role_id
         AND role.tenant_id = scope_grant.tenant_id
        WHERE role.key = 'facilities_officer'
          AND scope_grant.scope_family = 'facilities.work_orders'
          AND scope_grant.scope_kind = 'assigned'
          AND scope_grant.deleted_at IS NULL
    ) THEN
        RAISE EXCEPTION 'Facilities Officer assigned work-order scope is missing';
    END IF;
END;
$$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger
        WHERE tgname = 'facility_inspections_append_only' AND NOT tgisinternal
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_trigger
        WHERE tgname = 'facility_completion_submissions_append_only' AND NOT tgisinternal
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_trigger
        WHERE tgname = 'facility_events_append_only' AND NOT tgisinternal
    ) THEN
        RAISE EXCEPTION 'Facilities immutable evidence triggers are missing';
    END IF;
END;
$$;

SELECT 'Facilities foundation contract passed' AS result;
