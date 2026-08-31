-- Migration 093 must keep Ready monotonic and reject stale setup states when an owner exists.

DO $$
DECLARE
    default_tenant_id UUID;
    owner_id UUID := gen_random_uuid();
    previous_state APP_STATE;
    previous_lock BOOLEAN;
    downgrade_rejected BOOLEAN := FALSE;
BEGIN
    SELECT id INTO default_tenant_id
    FROM tenants
    WHERE slug = 'default' AND deleted_at IS NULL;

    SELECT state, kernel_lock INTO previous_state, previous_lock
    FROM system_state
    WHERE id = 'singleton';

    INSERT INTO users (
        id, tenant_id, email, password_hash, full_name, is_active, roles
    ) VALUES (
        owner_id,
        default_tenant_id,
        'migration-093-owner-' || owner_id::TEXT || '@example.test',
        'not-a-real-password-hash',
        'Migration 093 owner',
        TRUE,
        ARRAY['campus_owner']::TEXT[]
    );

    UPDATE system_state SET state = 'Ready', kernel_lock = FALSE WHERE id = 'singleton';

    BEGIN
        UPDATE system_state SET state = 'SchoolConfigured' WHERE id = 'singleton';
    EXCEPTION
        WHEN raise_exception THEN
            downgrade_rejected := TRUE;
    END;

    IF NOT downgrade_rejected THEN
        RAISE EXCEPTION 'migration 093 must reject a Ready downgrade';
    END IF;

    DELETE FROM users WHERE id = owner_id;

    -- Ready remains monotonic even after owner fixture cleanup. Restore only the
    -- non-state lock field so the contract leaves a valid installation state.
    UPDATE system_state SET kernel_lock = previous_lock WHERE id = 'singleton';

    IF previous_state <> 'Ready' THEN
        RAISE NOTICE 'migration 093 contract intentionally leaves bootstrap Ready';
    END IF;
END;
$$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_trigger
        WHERE tgrelid = 'system_state'::REGCLASS
          AND tgname = 'system_state_protect_ready'
          AND NOT tgisinternal
    ) THEN
        RAISE EXCEPTION 'migration 093 bootstrap trigger is missing';
    END IF;
END;
$$;
