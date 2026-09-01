-- Document Registry foundation contract checks. Run after migration 101.

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM roles
         WHERE key = 'records_officer' AND deleted_at IS NULL
           AND NOT permissions @> ARRAY[
               'document_registry:view', 'document_registry:create',
               'document_registry:edit', 'document_registry:classify',
               'document_registry:close', 'document_registry:dispose',
               'document_registry:restricted', 'document_registry:manage'
           ]::TEXT[]
    ) THEN
        RAISE EXCEPTION 'a Records Officer role is missing registry permissions';
    END IF;
END;
$$;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM roles AS role
         WHERE role.key = 'records_officer' AND role.deleted_at IS NULL
           AND NOT EXISTS (
               SELECT 1 FROM role_record_scope_grants AS scope_grant
                WHERE scope_grant.tenant_id = role.tenant_id
                  AND scope_grant.role_id = role.id
                  AND scope_grant.scope_family = 'document_registry.records'
                  AND scope_grant.scope_kind = 'campus'
                  AND scope_grant.deleted_at IS NULL
           )
    ) THEN
        RAISE EXCEPTION 'a Records Officer role is missing campus registry scope';
    END IF;
END;
$$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes
         WHERE indexname = 'idx_document_registry_active_disposition_review'
           AND indexdef LIKE '%pending%'
           AND indexdef LIKE '%approved%'
    ) THEN
        RAISE EXCEPTION 'active disposition review uniqueness is missing';
    END IF;
END;
$$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger
         WHERE tgname = 'document_registry_activity_events_append_only'
           AND NOT tgisinternal
    ) THEN
        RAISE EXCEPTION 'Document Registry append-only evidence trigger is missing';
    END IF;
END;
$$;

SELECT 'document registry foundation contract passed' AS result;
