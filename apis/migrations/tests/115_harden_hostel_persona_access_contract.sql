-- Hostel persona access contract checks. Run after migration 115.

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
          FROM tenants AS tenant
         WHERE NOT EXISTS (
            SELECT 1
              FROM roles AS role
             WHERE role.tenant_id = tenant.id
               AND role.key = 'hostel_resident'
               AND role.name = 'Boarding learner'
               AND role.permissions = ARRAY['hostel:view']::TEXT[]
               AND role.is_system
               AND role.deleted_at IS NULL
         )
    ) THEN
        RAISE EXCEPTION 'a tenant is missing the canonical Boarding learner role';
    END IF;
END;
$$;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
          FROM roles AS role
         WHERE role.key = 'hostel_resident'
           AND role.deleted_at IS NULL
           AND NOT EXISTS (
              SELECT 1
                FROM role_record_scope_grants AS scope_grant
               WHERE scope_grant.tenant_id = role.tenant_id
                 AND scope_grant.role_id = role.id
                 AND scope_grant.scope_family = 'hostel.occupancy'
                 AND scope_grant.scope_kind = 'self'
                 AND scope_grant.deleted_at IS NULL
           )
    ) THEN
        RAISE EXCEPTION 'a Boarding learner role is missing self occupancy scope';
    END IF;
END;
$$;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
          FROM role_record_scope_grants AS scope_grant
          JOIN roles AS role
            ON role.id = scope_grant.role_id
           AND role.tenant_id = scope_grant.tenant_id
         WHERE role.key = 'hostel_resident'
           AND role.deleted_at IS NULL
           AND scope_grant.deleted_at IS NULL
           AND NOT (
              scope_grant.scope_family = 'hostel.occupancy'
              AND scope_grant.scope_kind = 'self'
           )
    ) THEN
        RAISE EXCEPTION 'a Boarding learner role has an administrative record scope';
    END IF;
END;
$$;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
          FROM role_record_scope_grants
         WHERE (
               (scope_family = 'hostel.pastoral' AND scope_kind <> 'campus')
               OR (scope_family = 'hostel.occupancy' AND scope_kind NOT IN ('campus', 'self'))
           )
           AND deleted_at IS NULL
    ) THEN
        RAISE EXCEPTION 'an invalid Hostel record scope remains active';
    END IF;
END;
$$;

DO $$
DECLARE
    trigger_count BIGINT;
BEGIN
    SELECT COUNT(*) INTO trigger_count
      FROM pg_trigger
     WHERE tgname = 'enforce_hostel_record_scope_policy'
       AND NOT tgisinternal;
    IF trigger_count <> 1 THEN
        RAISE EXCEPTION 'Hostel record-scope policy trigger is missing';
    END IF;
END;
$$;

DO $$
DECLARE
    sample_tenant_id UUID;
    sample_role_id UUID;
    rejected BOOLEAN := FALSE;
BEGIN
    SELECT tenant_id, id
      INTO sample_tenant_id, sample_role_id
      FROM roles
     WHERE key = 'hostel_resident'
       AND deleted_at IS NULL
     LIMIT 1;

    BEGIN
        INSERT INTO role_record_scope_grants (
            tenant_id, role_id, scope_family, scope_kind
        ) VALUES (
            sample_tenant_id, sample_role_id, 'hostel.pastoral', 'self'
        );
    EXCEPTION WHEN OTHERS THEN
        rejected := TRUE;
    END;

    IF NOT rejected THEN
        RAISE EXCEPTION 'Hostel pastoral self scope was accepted';
    END IF;
END;
$$;

DO $$
DECLARE
    sample_tenant_id UUID;
    sample_role_id UUID;
    rejected BOOLEAN := FALSE;
BEGIN
    SELECT tenant_id, id
      INTO sample_tenant_id, sample_role_id
      FROM roles
     WHERE key = 'hostel_resident'
       AND deleted_at IS NULL
     LIMIT 1;

    BEGIN
        INSERT INTO role_record_scope_grants (
            tenant_id, role_id, scope_family, scope_kind
        ) VALUES (
            sample_tenant_id, sample_role_id, 'hostel.occupancy', 'assigned'
        );
    EXCEPTION WHEN OTHERS THEN
        rejected := TRUE;
    END;

    IF NOT rejected THEN
        RAISE EXCEPTION 'Hostel occupancy assigned scope was accepted';
    END IF;
END;
$$;

SELECT 'Hostel persona access contract passed' AS result;
