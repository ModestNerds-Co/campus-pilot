-- Library migration contract checks. Run after migration 098 in a disposable database.

DO $$
DECLARE
    tenant_count BIGINT;
    settings_count BIGINT;
BEGIN
    SELECT COUNT(*) INTO tenant_count FROM tenants;
    SELECT COUNT(*) INTO settings_count FROM library_settings;
    IF settings_count <> tenant_count THEN
        RAISE EXCEPTION 'every tenant must have exactly one Library settings row';
    END IF;
END;
$$;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM roles
         WHERE key = 'librarian' AND deleted_at IS NULL
           AND NOT (
               permissions @> ARRAY['library:borrow']::TEXT[]
               AND permissions @> ARRAY['library:circulate']::TEXT[]
               AND permissions @> ARRAY['library:manage']::TEXT[]
           )
    ) THEN
        RAISE EXCEPTION 'a librarian role is missing Library operation permissions';
    END IF;
    IF EXISTS (
        SELECT 1 FROM roles
         WHERE key IN ('student', 'teacher', 'staff_member') AND deleted_at IS NULL
           AND NOT permissions @> ARRAY['library:borrow']::TEXT[]
    ) THEN
        RAISE EXCEPTION 'a seeded borrower role is missing library:borrow';
    END IF;
END;
$$;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
          FROM roles AS role
         WHERE role.deleted_at IS NULL
           AND role.key IN ('librarian', 'student', 'teacher', 'staff_member')
           AND NOT EXISTS (
               SELECT 1
                 FROM role_record_scope_grants AS scope_grant
                WHERE scope_grant.tenant_id = role.tenant_id
                  AND scope_grant.role_id = role.id
                  AND scope_grant.scope_family = 'library.borrowing'
                  AND scope_grant.deleted_at IS NULL
           )
    ) THEN
        RAISE EXCEPTION 'a seeded Library role is missing its borrowing scope';
    END IF;
END;
$$;

DO $$
DECLARE
    sample_tenant UUID;
    sample_user UUID;
    sample_event UUID;
BEGIN
    SELECT users.tenant_id, users.id
      INTO sample_tenant, sample_user
      FROM users
     WHERE users.deleted_at IS NULL
     LIMIT 1;
    IF sample_user IS NULL THEN
        RAISE EXCEPTION 'Library contract test requires one active user';
    END IF;
    INSERT INTO library_activity_events (
        tenant_id, aggregate_type, aggregate_id, event_type, actor_id
    ) VALUES (
        sample_tenant, 'settings', sample_tenant, 'contract_test', sample_user
    ) RETURNING id INTO sample_event;
    BEGIN
        UPDATE library_activity_events SET event_type = 'changed' WHERE id = sample_event;
        RAISE EXCEPTION 'Library activity events accepted an update';
    EXCEPTION
        WHEN raise_exception THEN
            IF SQLERRM <> 'Library activity events are append-only' THEN
                RAISE;
            END IF;
    END;
    DELETE FROM library_activity_events WHERE id = sample_event;
EXCEPTION
    WHEN raise_exception THEN
        IF SQLERRM <> 'Library activity events are append-only' THEN
            RAISE;
        END IF;
END;
$$;

SELECT 'library foundation contract passed' AS result;
