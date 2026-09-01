-- Hostel foundation contract checks. Run after migration 100.

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM roles
         WHERE key = 'hostel_officer' AND deleted_at IS NULL
           AND NOT permissions @> ARRAY[
               'hostel:view', 'hostel:create', 'hostel:edit',
               'hostel:allocate', 'hostel:pastoral', 'hostel:manage'
           ]::TEXT[]
    ) THEN
        RAISE EXCEPTION 'a Hostel Officer role is missing hostel permissions';
    END IF;
END;
$$;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM roles AS role
         WHERE role.key = 'hostel_officer' AND role.deleted_at IS NULL
           AND NOT EXISTS (
               SELECT 1 FROM role_record_scope_grants AS scope_grant
                WHERE scope_grant.tenant_id = role.tenant_id
                  AND scope_grant.role_id = role.id
                  AND scope_grant.scope_family = 'hostel.occupancy'
                  AND scope_grant.scope_kind = 'campus'
                  AND scope_grant.deleted_at IS NULL
           )
    ) THEN
        RAISE EXCEPTION 'a Hostel Officer role is missing campus occupancy scope';
    END IF;
END;
$$;

DO $$
DECLARE
    mutable_trigger_count BIGINT;
BEGIN
    SELECT COUNT(*) INTO mutable_trigger_count
      FROM pg_trigger
     WHERE tgname = 'hostel_activity_events_append_only' AND NOT tgisinternal;
    IF mutable_trigger_count <> 1 THEN
        RAISE EXCEPTION 'Hostel append-only evidence trigger is missing';
    END IF;
END;
$$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
          FROM pg_index AS index_record
          JOIN pg_class AS index_class ON index_class.oid = index_record.indexrelid
         WHERE index_class.relname = 'idx_hostel_allocations_current_learner'
           AND index_record.indisunique
           AND pg_get_expr(index_record.indpred, index_record.indrelid) LIKE '%planned%'
           AND pg_get_expr(index_record.indpred, index_record.indrelid) LIKE '%active%'
    ) THEN
        RAISE EXCEPTION 'Hostel current-allocation uniqueness is missing';
    END IF;
END;
$$;

SELECT 'hostel foundation contract passed' AS result;
