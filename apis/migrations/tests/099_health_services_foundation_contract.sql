-- Health services migration contract checks. Run after migration 099.

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM roles
         WHERE key = 'health_officer' AND deleted_at IS NULL
           AND NOT permissions @> ARRAY[
               'health:view', 'health:create', 'health:edit',
               'health:medication', 'health:follow_up', 'health:manage'
           ]::TEXT[]
    ) THEN
        RAISE EXCEPTION 'a Health Officer role is missing health permissions';
    END IF;
END;
$$;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM roles AS role
         WHERE role.key = 'health_officer' AND role.deleted_at IS NULL
           AND NOT EXISTS (
               SELECT 1 FROM role_record_scope_grants AS scope_grant
                WHERE scope_grant.tenant_id = role.tenant_id
                  AND scope_grant.role_id = role.id
                  AND scope_grant.scope_family = 'health.care'
                  AND scope_grant.scope_kind = 'campus'
                  AND scope_grant.deleted_at IS NULL
           )
    ) THEN
        RAISE EXCEPTION 'a Health Officer role is missing campus care scope';
    END IF;
END;
$$;

DO $$
DECLARE
    mutable_trigger_count BIGINT;
BEGIN
    SELECT COUNT(*) INTO mutable_trigger_count
      FROM pg_trigger
     WHERE tgname IN (
        'health_medication_administrations_append_only',
        'health_activity_events_append_only'
     ) AND NOT tgisinternal;
    IF mutable_trigger_count <> 2 THEN
        RAISE EXCEPTION 'Health append-only evidence triggers are missing';
    END IF;
END;
$$;

SELECT 'health services foundation contract passed' AS result;
