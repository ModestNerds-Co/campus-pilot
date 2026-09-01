-- Internal Audit foundation contract checks. Run after migration 103.

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM roles
        WHERE key = 'internal_auditor' AND deleted_at IS NULL
          AND (
              NOT permissions @> ARRAY[
                  'internal_audit:view', 'internal_audit:create',
                  'internal_audit:edit', 'document_registry:view'
              ]::TEXT[]
              OR permissions && ARRAY[
                  'internal_audit:delete', 'internal_audit:issue', 'internal_audit:manage'
              ]::TEXT[]
          )
    ) THEN
        RAISE EXCEPTION 'an Internal Auditor role has an invalid authority boundary';
    END IF;
END;
$$;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM roles
        WHERE key = 'audit_manager' AND deleted_at IS NULL
          AND NOT permissions @> ARRAY[
              'internal_audit:view', 'internal_audit:create', 'internal_audit:edit',
              'internal_audit:delete', 'internal_audit:issue', 'internal_audit:manage',
              'document_registry:view'
          ]::TEXT[]
    ) THEN
        RAISE EXCEPTION 'an Audit Manager role is missing management authority';
    END IF;
END;
$$;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM roles AS role
        WHERE role.key IN ('internal_auditor', 'audit_manager')
          AND role.deleted_at IS NULL
          AND NOT EXISTS (
              SELECT 1 FROM role_record_scope_grants AS scope_grant
              WHERE scope_grant.tenant_id = role.tenant_id
                AND scope_grant.role_id = role.id
                AND scope_grant.scope_family = 'internal_audit.records'
                AND scope_grant.scope_kind = CASE
                    WHEN role.key = 'internal_auditor' THEN 'assigned'
                    ELSE 'campus'
                END
                AND scope_grant.deleted_at IS NULL
          )
    ) THEN
        RAISE EXCEPTION 'an Internal Audit role is missing its record scope';
    END IF;
END;
$$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger
        WHERE tgname = 'internal_audit_evidence_append_only' AND NOT tgisinternal
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_trigger
        WHERE tgname = 'internal_audit_events_append_only' AND NOT tgisinternal
    ) THEN
        RAISE EXCEPTION 'Internal Audit append-only evidence is missing';
    END IF;
END;
$$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE indexname = 'internal_audit_plans_tenant_id_reference_key'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE indexname = 'internal_audit_engagements_tenant_id_reference_key'
    ) OR NOT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE indexname = 'internal_audit_findings_tenant_id_reference_key'
    ) THEN
        RAISE EXCEPTION 'Internal Audit stable reference uniqueness is missing';
    END IF;
END;
$$;

SELECT 'Internal Audit foundation contract passed' AS result;
