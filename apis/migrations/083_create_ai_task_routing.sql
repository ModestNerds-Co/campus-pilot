-- Owns tenant-scoped, versioned AI route scopes and their immutable ordered targets.
-- Route saves bind to the current provider credential and model snapshot; later drift
-- makes resolution fail closed instead of broadening to a lower-precedence route.

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'ai_provider_models_id_tenant_unique'
          AND conrelid = 'ai_provider_models'::REGCLASS
    ) THEN
        ALTER TABLE ai_provider_models
            ADD CONSTRAINT ai_provider_models_id_tenant_unique UNIQUE (id, tenant_id);
    END IF;
END;
$$;

CREATE TABLE IF NOT EXISTS ai_route_sets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    scope_kind TEXT NOT NULL CHECK (
        scope_kind IN ('tenant_default', 'task_class', 'module_operation', 'capability')
    ),
    task_class TEXT CHECK (
        task_class IS NULL OR task_class IN (
            'campus_conversation_search',
            'module_read_reporting',
            'document_extraction',
            'drafting_proposal',
            'approved_operational_action'
        )
    ),
    module_key TEXT CHECK (
        module_key IS NULL OR (
            CHAR_LENGTH(module_key) BETWEEN 1 AND 160
            AND module_key = LOWER(BTRIM(module_key))
            AND module_key ~ '^[a-z][a-z0-9_.-]*$'
        )
    ),
    operation_class TEXT CHECK (
        operation_class IS NULL OR operation_class IN (
            'read', 'propose', 'mutate', 'external_side_effect'
        )
    ),
    capability_key TEXT CHECK (
        capability_key IS NULL OR (
            CHAR_LENGTH(capability_key) BETWEEN 1 AND 200
            AND capability_key = LOWER(BTRIM(capability_key))
            AND capability_key ~ '^[a-z][a-z0-9_.-]*$'
        )
    ),
    capability_version INTEGER CHECK (capability_version IS NULL OR capability_version > 0),
    configured_by UUID NOT NULL,
    change_reason TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(change_reason)) BETWEEN 3 AND 500),
    archived_reason TEXT CHECK (
        archived_reason IS NULL OR CHAR_LENGTH(BTRIM(archived_reason)) BETWEEN 3 AND 500
    ),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT ai_route_sets_id_tenant_unique UNIQUE (id, tenant_id),
    CONSTRAINT ai_route_sets_configured_by_tenant_fk
        FOREIGN KEY (configured_by, tenant_id)
        REFERENCES users(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT ai_route_sets_scope_shape_check CHECK (
        (
            scope_kind = 'tenant_default'
            AND task_class IS NULL
            AND module_key IS NULL
            AND operation_class IS NULL
            AND capability_key IS NULL
            AND capability_version IS NULL
        )
        OR (
            scope_kind = 'task_class'
            AND task_class IS NOT NULL
            AND module_key IS NULL
            AND operation_class IS NULL
            AND capability_key IS NULL
            AND capability_version IS NULL
        )
        OR (
            scope_kind = 'module_operation'
            AND task_class IS NULL
            AND module_key IS NOT NULL
            AND operation_class IS NOT NULL
            AND capability_key IS NULL
            AND capability_version IS NULL
        )
        OR (
            scope_kind = 'capability'
            AND task_class IS NULL
            AND module_key IS NULL
            AND operation_class IS NULL
            AND capability_key IS NOT NULL
            AND capability_version IS NOT NULL
        )
    ),
    CONSTRAINT ai_route_sets_archive_shape_check CHECK (
        (deleted_at IS NULL AND archived_reason IS NULL)
        OR (deleted_at IS NOT NULL AND archived_reason IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS ai_route_sets_active_scope_unique
    ON ai_route_sets (
        tenant_id,
        scope_kind,
        COALESCE(task_class, ''),
        COALESCE(module_key, ''),
        COALESCE(operation_class, ''),
        COALESCE(capability_key, ''),
        COALESCE(capability_version, 0)
    )
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS ai_route_sets_tenant_scope_idx
    ON ai_route_sets (tenant_id, scope_kind, updated_at DESC)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS ai_task_routes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    route_set_id UUID NOT NULL,
    priority SMALLINT NOT NULL CHECK (priority BETWEEN 1 AND 3),
    connection_id UUID NOT NULL,
    model_id UUID NOT NULL,
    requires_tools BOOLEAN NOT NULL DEFAULT FALSE,
    created_by UUID NOT NULL,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT ai_task_routes_id_tenant_unique UNIQUE (id, tenant_id),
    CONSTRAINT ai_task_routes_route_set_tenant_fk
        FOREIGN KEY (route_set_id, tenant_id)
        REFERENCES ai_route_sets(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT ai_task_routes_connection_tenant_fk
        FOREIGN KEY (connection_id, tenant_id)
        REFERENCES ai_provider_connections(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT ai_task_routes_model_tenant_fk
        FOREIGN KEY (model_id, tenant_id)
        REFERENCES ai_provider_models(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT ai_task_routes_created_by_tenant_fk
        FOREIGN KEY (created_by, tenant_id)
        REFERENCES users(id, tenant_id) ON DELETE RESTRICT
);

CREATE UNIQUE INDEX IF NOT EXISTS ai_task_routes_active_priority_unique
    ON ai_task_routes (route_set_id, priority)
    WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS ai_task_routes_active_target_unique
    ON ai_task_routes (route_set_id, connection_id)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS ai_task_routes_connection_reference_idx
    ON ai_task_routes (tenant_id, connection_id, route_set_id)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS ai_task_routes_route_set_idx
    ON ai_task_routes (tenant_id, route_set_id, priority)
    WHERE deleted_at IS NULL;

CREATE OR REPLACE FUNCTION protect_ai_route_set_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'AI route sets are archived, not deleted';
    END IF;

    IF OLD.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'archived AI route sets are immutable';
    END IF;

    IF NEW.id IS DISTINCT FROM OLD.id
       OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
       OR NEW.scope_kind IS DISTINCT FROM OLD.scope_kind
       OR NEW.task_class IS DISTINCT FROM OLD.task_class
       OR NEW.module_key IS DISTINCT FROM OLD.module_key
       OR NEW.operation_class IS DISTINCT FROM OLD.operation_class
       OR NEW.capability_key IS DISTINCT FROM OLD.capability_key
       OR NEW.capability_version IS DISTINCT FROM OLD.capability_version
       OR NEW.configured_by IS DISTINCT FROM OLD.configured_by
       OR NEW.created_at IS DISTINCT FROM OLD.created_at THEN
        RAISE EXCEPTION 'AI route scope identity is immutable';
    END IF;

    IF NEW.version <> OLD.version + 1 THEN
        RAISE EXCEPTION 'AI route set version must advance exactly once';
    END IF;

    IF NEW.updated_at <= OLD.updated_at THEN
        RAISE EXCEPTION 'AI route set update time must advance';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS ai_route_sets_protect_lifecycle ON ai_route_sets;
CREATE TRIGGER ai_route_sets_protect_lifecycle
    BEFORE UPDATE OR DELETE ON ai_route_sets
    FOR EACH ROW
    EXECUTE FUNCTION protect_ai_route_set_lifecycle();

CREATE OR REPLACE FUNCTION validate_ai_task_route_target()
RETURNS TRIGGER AS $$
DECLARE
    connection_status TEXT;
    current_credential_version BIGINT;
    current_catalog_version BIGINT;
    model_connection_id UUID;
    model_supports_tools BOOLEAN;
BEGIN
    PERFORM 1
    FROM ai_route_sets
    WHERE id = NEW.route_set_id
      AND tenant_id = NEW.tenant_id
      AND deleted_at IS NULL;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'AI route target requires an active same-tenant route set';
    END IF;

    SELECT status, credential_version, model_catalog_version
    INTO connection_status, current_credential_version, current_catalog_version
    FROM ai_provider_connections
    WHERE id = NEW.connection_id
      AND tenant_id = NEW.tenant_id
      AND deleted_at IS NULL;

    IF NOT FOUND OR connection_status <> 'ready' THEN
        RAISE EXCEPTION 'AI route target requires a ready same-tenant connection';
    END IF;

    SELECT connection_id, supports_tools
    INTO model_connection_id, model_supports_tools
    FROM ai_provider_models
    WHERE id = NEW.model_id
      AND tenant_id = NEW.tenant_id
      AND credential_version = current_credential_version
      AND catalog_version = current_catalog_version
      AND deleted_at IS NULL;

    IF NOT FOUND
       OR model_connection_id <> NEW.connection_id THEN
        RAISE EXCEPTION 'AI route target requires the current immutable model snapshot';
    END IF;

    IF NEW.requires_tools AND model_supports_tools IS DISTINCT FROM TRUE THEN
        RAISE EXCEPTION 'AI route target requires a model with confirmed tool support';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM ai_task_routes
        WHERE route_set_id = NEW.route_set_id
          AND deleted_at IS NULL
          AND requires_tools <> NEW.requires_tools
    ) THEN
        RAISE EXCEPTION 'all targets in an AI route must use the same tool requirement';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS ai_task_routes_validate_target ON ai_task_routes;
CREATE TRIGGER ai_task_routes_validate_target
    BEFORE INSERT ON ai_task_routes
    FOR EACH ROW
    EXECUTE FUNCTION validate_ai_task_route_target();

CREATE OR REPLACE FUNCTION protect_ai_task_route_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'AI task route targets are archived, not deleted';
    END IF;

    IF OLD.deleted_at IS NOT NULL
       OR NEW.deleted_at IS NULL
       OR NEW.id IS DISTINCT FROM OLD.id
       OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
       OR NEW.route_set_id IS DISTINCT FROM OLD.route_set_id
       OR NEW.priority IS DISTINCT FROM OLD.priority
       OR NEW.connection_id IS DISTINCT FROM OLD.connection_id
       OR NEW.model_id IS DISTINCT FROM OLD.model_id
       OR NEW.requires_tools IS DISTINCT FROM OLD.requires_tools
       OR NEW.created_by IS DISTINCT FROM OLD.created_by
       OR NEW.created_at IS DISTINCT FROM OLD.created_at
       OR NEW.updated_at <= OLD.updated_at THEN
        RAISE EXCEPTION 'AI task route targets are immutable';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS ai_task_routes_protect_lifecycle ON ai_task_routes;
CREATE TRIGGER ai_task_routes_protect_lifecycle
    BEFORE UPDATE OR DELETE ON ai_task_routes
    FOR EACH ROW
    EXECUTE FUNCTION protect_ai_task_route_lifecycle();

-- This late trigger augments only newly provisioned School Administrators.
-- Existing non-owner roles receive no routing authority during migration.
CREATE OR REPLACE FUNCTION grant_new_tenant_ai_routing_permissions()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE roles
    SET permissions = ARRAY(
            SELECT DISTINCT value
            FROM UNNEST(
                permissions || ARRAY['ai_routing:view', 'ai_routing:edit']::TEXT[]
            ) AS permission(value)
            ORDER BY value
        ),
        updated_at = NOW()
    WHERE tenant_id = NEW.id
      AND key = 'school_administrator'
      AND deleted_at IS NULL;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_grant_new_tenant_ai_routing_permissions ON tenants;
CREATE TRIGGER zz_grant_new_tenant_ai_routing_permissions
    AFTER INSERT ON tenants
    FOR EACH ROW
    EXECUTE FUNCTION grant_new_tenant_ai_routing_permissions();
