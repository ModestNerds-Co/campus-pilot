-- Exact seeded Teacher permission contract. Run after migration 112.

DO $$
DECLARE
    expected_permissions TEXT[] := ARRAY[
        'academics:teach', 'academics:view', 'attendance:create', 'attendance:edit',
        'attendance:submit', 'attendance:view', 'facilities:request', 'facilities:view',
        'learning:teach', 'learning:view', 'library:borrow', 'library:view',
        'messaging:create', 'messaging:edit', 'messaging:view', 'sis:view',
        'timetabling:view'
    ]::TEXT[];
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM roles WHERE key='teacher' AND deleted_at IS NULL
    ) THEN
        RAISE EXCEPTION 'Seeded Teacher role is missing';
    END IF;

    IF EXISTS (
        SELECT 1 FROM roles
         WHERE key='teacher' AND deleted_at IS NULL
           AND permissions IS DISTINCT FROM expected_permissions
    ) THEN
        RAISE EXCEPTION 'Seeded Teacher permission baseline is not exact';
    END IF;

    IF EXISTS (
        SELECT 1 FROM roles
         WHERE key='teacher' AND deleted_at IS NULL
           AND permissions && ARRAY[
               'administration:view', 'users:create', 'users:edit',
               'roles:create', 'roles:edit', 'roles:assign',
               'academics:create', 'academics:edit', 'academics:delete',
               'academics:manage'
           ]::TEXT[]
    ) THEN
        RAISE EXCEPTION 'Seeded Teacher has administrative authority';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname='enforce_seeded_teacher_access_boundary'
           AND NOT tgisinternal
    ) THEN
        RAISE EXCEPTION 'Seeded Teacher access trigger is missing';
    END IF;
END;
$$;

SELECT 'Seeded Teacher access contract passed' AS result;
