-- Owns append-only campus approval for provider data handling and pins every
-- route and provider attempt to one exact approval version. Existing provider
-- connections are deliberately backfilled as unapproved; approval is never inferred.

CREATE TABLE IF NOT EXISTS ai_provider_data_approval_versions (
    id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    connection_id UUID NOT NULL,
    approval_version BIGINT NOT NULL
        CHECK (approval_version BETWEEN 1 AND 9007199254740991),
    approval_class TEXT NOT NULL CHECK (
        approval_class IN ('unapproved', 'campus_approved', 'sensitive_data_approved')
    ),
    change_source TEXT NOT NULL CHECK (
        change_source IN ('system_default', 'administrator')
    ),
    changed_by UUID,
    change_reason TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(change_reason)) BETWEEN 3 AND 500),
    deleted_at TIMESTAMPTZ CHECK (deleted_at IS NULL),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT ai_provider_data_approvals_id_tenant_connection_unique
        UNIQUE (id, tenant_id, connection_id),
    CONSTRAINT ai_provider_data_approvals_connection_version_unique
        UNIQUE (tenant_id, connection_id, approval_version),
    CONSTRAINT ai_provider_data_approvals_connection_tenant_fk
        FOREIGN KEY (connection_id, tenant_id)
        REFERENCES ai_provider_connections(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT ai_provider_data_approvals_changed_by_tenant_fk
        FOREIGN KEY (changed_by, tenant_id)
        REFERENCES users(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT ai_provider_data_approvals_source_shape_check CHECK (
        (
            change_source = 'system_default'
            AND approval_class = 'unapproved'
            AND changed_by IS NULL
        )
        OR (
            change_source = 'administrator'
            AND changed_by IS NOT NULL
        )
    ),
    CONSTRAINT ai_provider_data_approvals_timestamps_check
        CHECK (created_at = updated_at)
);

CREATE INDEX IF NOT EXISTS ai_provider_data_approvals_current_idx
    ON ai_provider_data_approval_versions (
        tenant_id, connection_id, approval_version DESC
    );

CREATE OR REPLACE FUNCTION validate_ai_provider_data_approval_insert()
RETURNS TRIGGER AS $$
DECLARE
    connection_status TEXT;
    previous_version BIGINT;
BEGIN
    SELECT status
    INTO connection_status
    FROM ai_provider_connections
    WHERE id = NEW.connection_id
      AND tenant_id = NEW.tenant_id
      AND deleted_at IS NULL
    FOR UPDATE;

    IF NOT FOUND OR connection_status = 'disconnected' THEN
        RAISE EXCEPTION 'provider data approval requires an active same-tenant connection';
    END IF;

    SELECT MAX(approval_version)
    INTO previous_version
    FROM ai_provider_data_approval_versions
    WHERE tenant_id = NEW.tenant_id
      AND connection_id = NEW.connection_id;

    IF previous_version IS NULL THEN
        IF NEW.approval_version <> 1
           OR NEW.change_source <> 'system_default'
           OR NEW.approval_class <> 'unapproved'
           OR NEW.changed_by IS NOT NULL THEN
            RAISE EXCEPTION 'first provider data approval must be version 1 system-default unapproved';
        END IF;
    ELSIF NEW.approval_version <> previous_version + 1
          OR NEW.change_source <> 'administrator'
          OR NEW.changed_by IS NULL THEN
        RAISE EXCEPTION 'provider data approval versions must advance exactly once by an administrator';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Replays must also suspend the live guard while repairing any missing legacy
-- default rows below.
DROP TRIGGER IF EXISTS ai_provider_data_approvals_validate_insert
    ON ai_provider_data_approval_versions;

CREATE OR REPLACE FUNCTION reject_ai_provider_data_approval_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'provider data approval versions are append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS ai_provider_data_approvals_reject_mutation
    ON ai_provider_data_approval_versions;
CREATE TRIGGER ai_provider_data_approvals_reject_mutation
    BEFORE UPDATE OR DELETE ON ai_provider_data_approval_versions
    FOR EACH ROW
    EXECUTE FUNCTION reject_ai_provider_data_approval_mutation();

DROP TRIGGER IF EXISTS ai_provider_data_approvals_reject_truncate
    ON ai_provider_data_approval_versions;
CREATE TRIGGER ai_provider_data_approvals_reject_truncate
    BEFORE TRUNCATE ON ai_provider_data_approval_versions
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_ai_provider_data_approval_mutation();

INSERT INTO ai_provider_data_approval_versions (
    id, tenant_id, connection_id, approval_version, approval_class,
    change_source, changed_by, change_reason
)
SELECT
    GEN_RANDOM_UUID(), c.tenant_id, c.id, 1, 'unapproved',
    'system_default', NULL, 'Initial unapproved provider data eligibility.'
FROM ai_provider_connections c
WHERE NOT EXISTS (
    SELECT 1
    FROM ai_provider_data_approval_versions approval
    WHERE approval.tenant_id = c.tenant_id
      AND approval.connection_id = c.id
);

-- Backfill intentionally precedes the live insert guard so legacy disconnected
-- connections receive the required unapproved version without granting access.
DROP TRIGGER IF EXISTS ai_provider_data_approvals_validate_insert
    ON ai_provider_data_approval_versions;
CREATE TRIGGER ai_provider_data_approvals_validate_insert
    BEFORE INSERT ON ai_provider_data_approval_versions
    FOR EACH ROW
    EXECUTE FUNCTION validate_ai_provider_data_approval_insert();

CREATE OR REPLACE FUNCTION require_ai_provider_default_data_approval()
RETURNS TRIGGER AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM ai_provider_data_approval_versions approval
        WHERE approval.tenant_id = NEW.tenant_id
          AND approval.connection_id = NEW.id
          AND approval.approval_version = 1
          AND approval.approval_class = 'unapproved'
          AND approval.change_source = 'system_default'
          AND approval.changed_by IS NULL
    ) THEN
        RAISE EXCEPTION 'provider connection requires its default unapproved data approval';
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS ai_provider_connections_require_default_data_approval
    ON ai_provider_connections;
CREATE CONSTRAINT TRIGGER ai_provider_connections_require_default_data_approval
    AFTER INSERT ON ai_provider_connections
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW
    EXECUTE FUNCTION require_ai_provider_default_data_approval();

ALTER TABLE ai_task_routes
    ADD COLUMN IF NOT EXISTS provider_data_approval_id UUID;

UPDATE ai_task_routes route
SET provider_data_approval_id = (
    SELECT approval.id
    FROM ai_provider_data_approval_versions approval
    WHERE approval.tenant_id = route.tenant_id
      AND approval.connection_id = route.connection_id
    ORDER BY approval.approval_version DESC
    LIMIT 1
)
WHERE route.provider_data_approval_id IS NULL;

ALTER TABLE ai_task_routes
    ALTER COLUMN provider_data_approval_id SET NOT NULL;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'ai_task_routes_provider_data_approval_fk'
          AND conrelid = 'ai_task_routes'::REGCLASS
    ) THEN
        ALTER TABLE ai_task_routes
            ADD CONSTRAINT ai_task_routes_provider_data_approval_fk
            FOREIGN KEY (provider_data_approval_id, tenant_id, connection_id)
            REFERENCES ai_provider_data_approval_versions(id, tenant_id, connection_id)
            ON DELETE RESTRICT;
    END IF;
END;
$$;

CREATE OR REPLACE FUNCTION validate_ai_task_route_target()
RETURNS TRIGGER AS $$
DECLARE
    connection_status TEXT;
    current_credential_version BIGINT;
    current_catalog_version BIGINT;
    model_connection_id UUID;
    model_supports_tools BOOLEAN;
    pinned_approval_class TEXT;
    latest_approval_id UUID;
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

    SELECT approval_class
    INTO pinned_approval_class
    FROM ai_provider_data_approval_versions
    WHERE id = NEW.provider_data_approval_id
      AND tenant_id = NEW.tenant_id
      AND connection_id = NEW.connection_id;

    SELECT id
    INTO latest_approval_id
    FROM ai_provider_data_approval_versions
    WHERE tenant_id = NEW.tenant_id
      AND connection_id = NEW.connection_id
    ORDER BY approval_version DESC
    LIMIT 1;

    IF pinned_approval_class IS NULL
       OR NEW.provider_data_approval_id <> latest_approval_id
       OR pinned_approval_class = 'unapproved' THEN
        RAISE EXCEPTION 'AI route target requires the latest approved provider data policy';
    END IF;

    SELECT connection_id, supports_tools
    INTO model_connection_id, model_supports_tools
    FROM ai_provider_models
    WHERE id = NEW.model_id
      AND tenant_id = NEW.tenant_id
      AND credential_version = current_credential_version
      AND catalog_version = current_catalog_version
      AND deleted_at IS NULL;

    IF NOT FOUND OR model_connection_id <> NEW.connection_id THEN
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
       OR NEW.provider_data_approval_id IS DISTINCT FROM OLD.provider_data_approval_id
       OR NEW.requires_tools IS DISTINCT FROM OLD.requires_tools
       OR NEW.created_by IS DISTINCT FROM OLD.created_by
       OR NEW.created_at IS DISTINCT FROM OLD.created_at
       OR NEW.updated_at <= OLD.updated_at THEN
        RAISE EXCEPTION 'AI task route targets are immutable';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM agent_provider_attempts LIMIT 1) THEN
        RAISE EXCEPTION 'migration 087 cannot infer provider data eligibility for existing attempts';
    END IF;
END;
$$;

ALTER TABLE agent_provider_attempts
    ADD COLUMN IF NOT EXISTS provider_data_approval_id UUID,
    ADD COLUMN IF NOT EXISTS required_provider_data_class TEXT,
    ADD COLUMN IF NOT EXISTS execution_environment_class TEXT;

ALTER TABLE agent_provider_attempts
    ALTER COLUMN provider_data_approval_id SET NOT NULL,
    ALTER COLUMN required_provider_data_class SET NOT NULL,
    ALTER COLUMN execution_environment_class SET NOT NULL;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'agent_provider_attempts_data_class_check'
          AND conrelid = 'agent_provider_attempts'::REGCLASS
    ) THEN
        ALTER TABLE agent_provider_attempts
            ADD CONSTRAINT agent_provider_attempts_data_class_check CHECK (
                required_provider_data_class IN (
                    'campus_approved', 'sensitive_data_approved', 'local_only'
                )
            );
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'agent_provider_attempts_environment_check'
          AND conrelid = 'agent_provider_attempts'::REGCLASS
    ) THEN
        ALTER TABLE agent_provider_attempts
            ADD CONSTRAINT agent_provider_attempts_environment_check CHECK (
                execution_environment_class = 'external_managed'
            );
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'agent_provider_attempts_data_approval_fk'
          AND conrelid = 'agent_provider_attempts'::REGCLASS
    ) THEN
        ALTER TABLE agent_provider_attempts
            ADD CONSTRAINT agent_provider_attempts_data_approval_fk
            FOREIGN KEY (provider_data_approval_id, tenant_id, connection_id)
            REFERENCES ai_provider_data_approval_versions(id, tenant_id, connection_id)
            ON DELETE RESTRICT;
    END IF;
END;
$$;

ALTER TABLE agent_provider_attempts
    DROP CONSTRAINT IF EXISTS agent_provider_attempts_failure_category_check,
    DROP CONSTRAINT IF EXISTS agent_provider_attempts_failure_shape_check;

ALTER TABLE agent_provider_attempts
    ADD CONSTRAINT agent_provider_attempts_failure_category_check CHECK (
        failure_category IS NULL OR failure_category IN (
            'connection_unavailable', 'stale_credential', 'stale_model',
            'tools_unsupported', 'model_context_unavailable',
            'context_window_exceeded', 'credential_unavailable',
            'invalid_configuration', 'invalid_input', 'storage_error',
            'provider_data_not_approved', 'provider_data_approval_changed',
            'local_execution_required',
            'authentication', 'rate_limited', 'unavailable', 'timeout',
            'network', 'invalid_response', 'unsupported'
        )
    ),
    ADD CONSTRAINT agent_provider_attempts_failure_shape_check CHECK (
        (
            status = 'failed'
            AND (
                (
                    failure_origin = 'preflight'
                    AND failure_category IN (
                        'connection_unavailable', 'stale_credential', 'stale_model',
                        'tools_unsupported', 'model_context_unavailable',
                        'context_window_exceeded', 'credential_unavailable',
                        'invalid_configuration', 'invalid_input', 'storage_error',
                        'provider_data_not_approved', 'provider_data_approval_changed',
                        'local_execution_required'
                    )
                )
                OR (
                    failure_origin = 'upstream'
                    AND failure_category IN (
                        'authentication', 'rate_limited', 'unavailable', 'timeout',
                        'network', 'invalid_response', 'unsupported'
                    )
                )
            )
        )
        OR (
            status <> 'failed'
            AND failure_origin IS NULL
            AND failure_category IS NULL
        )
    );

CREATE OR REPLACE FUNCTION validate_agent_provider_attempt_insert()
RETURNS TRIGGER AS $$
DECLARE
    stored_task_class TEXT;
    stored_run_status TEXT;
    stored_route_version BIGINT;
    stored_route_scope_kind TEXT;
    stored_route_task_class TEXT;
    stored_target_connection UUID;
    stored_target_model UUID;
    stored_target_approval UUID;
    stored_provider TEXT;
    stored_connection_status TEXT;
    stored_credential_version BIGINT;
    stored_connection_catalog_version BIGINT;
    stored_model_connection UUID;
    stored_provider_model_id TEXT;
    stored_model_credential_version BIGINT;
    stored_model_catalog_version BIGINT;
    stored_model_max_output_tokens BIGINT;
    stored_approval_class TEXT;
    latest_approval_id UUID;
    provider_data_allowed BOOLEAN;
BEGIN
    IF NEW.status <> 'running'
       OR NEW.failure_origin IS NOT NULL
       OR NEW.failure_category IS NOT NULL
       OR NEW.input_tokens IS NOT NULL
       OR NEW.output_tokens IS NOT NULL
       OR NEW.cached_tokens IS NOT NULL
       OR NEW.reasoning_tokens IS NOT NULL
       OR NEW.provider_reported_cost_amount IS NOT NULL
       OR NEW.provider_reported_cost_currency IS NOT NULL
       OR NEW.provider_reported_cost_exponent IS NOT NULL
       OR NEW.provider_reported_pricing_version IS NOT NULL
       OR NEW.estimated_cost_amount IS NOT NULL
       OR NEW.estimated_cost_currency IS NOT NULL
       OR NEW.estimated_cost_exponent IS NOT NULL
       OR NEW.estimated_pricing_version IS NOT NULL
       OR NEW.finished_at IS NOT NULL
       OR NEW.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'Agent provider attempts must start in the running state';
    END IF;

    SELECT task_class, status
    INTO stored_task_class, stored_run_status
    FROM agent_runs
    WHERE id = NEW.run_id
      AND tenant_id = NEW.tenant_id;

    SELECT version, scope_kind, task_class
    INTO stored_route_version, stored_route_scope_kind, stored_route_task_class
    FROM ai_route_sets
    WHERE id = NEW.route_set_id
      AND tenant_id = NEW.tenant_id
      AND deleted_at IS NULL;

    SELECT connection_id, model_id, provider_data_approval_id
    INTO stored_target_connection, stored_target_model, stored_target_approval
    FROM ai_task_routes
    WHERE id = NEW.route_target_id
      AND tenant_id = NEW.tenant_id
      AND route_set_id = NEW.route_set_id
      AND deleted_at IS NULL;

    SELECT provider, status, credential_version, model_catalog_version
    INTO stored_provider, stored_connection_status, stored_credential_version,
         stored_connection_catalog_version
    FROM ai_provider_connections
    WHERE id = NEW.connection_id
      AND tenant_id = NEW.tenant_id
      AND deleted_at IS NULL;

    SELECT connection_id, provider_model_id, credential_version, catalog_version,
           max_output_tokens
    INTO stored_model_connection, stored_provider_model_id,
         stored_model_credential_version, stored_model_catalog_version,
         stored_model_max_output_tokens
    FROM ai_provider_models
    WHERE id = NEW.model_snapshot_id
      AND tenant_id = NEW.tenant_id
      AND deleted_at IS NULL;

    SELECT approval_class
    INTO stored_approval_class
    FROM ai_provider_data_approval_versions
    WHERE id = NEW.provider_data_approval_id
      AND tenant_id = NEW.tenant_id
      AND connection_id = NEW.connection_id;

    SELECT id
    INTO latest_approval_id
    FROM ai_provider_data_approval_versions
    WHERE tenant_id = NEW.tenant_id
      AND connection_id = NEW.connection_id
    ORDER BY approval_version DESC
    LIMIT 1;

    provider_data_allowed := CASE NEW.required_provider_data_class
        WHEN 'campus_approved' THEN
            stored_approval_class IN ('campus_approved', 'sensitive_data_approved')
        WHEN 'sensitive_data_approved' THEN
            stored_approval_class = 'sensitive_data_approved'
        WHEN 'local_only' THEN
            stored_approval_class = 'sensitive_data_approved'
            AND NEW.execution_environment_class = 'installation_local'
        ELSE FALSE
    END;

    IF stored_task_class IS NULL
       OR stored_run_status <> 'running'
       OR stored_task_class <> NEW.task_class
       OR stored_route_version IS NULL
       OR stored_route_version <> NEW.route_version
       OR (
           stored_route_scope_kind = 'task_class'
           AND stored_route_task_class <> NEW.task_class
       )
       OR stored_target_connection IS NULL
       OR stored_target_connection <> NEW.connection_id
       OR stored_target_model <> NEW.model_snapshot_id
       OR stored_target_approval <> NEW.provider_data_approval_id
       OR latest_approval_id <> NEW.provider_data_approval_id
       OR provider_data_allowed IS DISTINCT FROM TRUE
       OR stored_provider IS NULL
       OR stored_connection_status <> 'ready'
       OR stored_provider <> NEW.provider_key
       OR stored_credential_version <> NEW.credential_version
       OR stored_model_connection <> NEW.connection_id
       OR stored_provider_model_id <> NEW.provider_model_id
       OR stored_model_credential_version <> stored_credential_version
       OR stored_model_catalog_version <> stored_connection_catalog_version
       OR stored_model_max_output_tokens IS NULL
       OR NOT EXISTS (
           SELECT 1
           FROM agent_run_queue
           WHERE run_id = NEW.run_id
             AND tenant_id = NEW.tenant_id
             AND state = 'leased'
             AND cancel_requested_at IS NULL
       ) THEN
        RAISE EXCEPTION 'Agent provider attempt identity must match its eligible resolved route snapshot';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION protect_agent_provider_attempt_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Agent provider attempts are retained, not deleted';
    END IF;

    IF OLD.status <> 'running' THEN
        RAISE EXCEPTION 'terminal Agent provider attempts are immutable';
    END IF;

    IF NEW.id IS DISTINCT FROM OLD.id
       OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
       OR NEW.run_id IS DISTINCT FROM OLD.run_id
       OR NEW.turn_index IS DISTINCT FROM OLD.turn_index
       OR NEW.attempt_index IS DISTINCT FROM OLD.attempt_index
       OR NEW.route_set_id IS DISTINCT FROM OLD.route_set_id
       OR NEW.route_version IS DISTINCT FROM OLD.route_version
       OR NEW.route_target_id IS DISTINCT FROM OLD.route_target_id
       OR NEW.connection_id IS DISTINCT FROM OLD.connection_id
       OR NEW.credential_version IS DISTINCT FROM OLD.credential_version
       OR NEW.model_snapshot_id IS DISTINCT FROM OLD.model_snapshot_id
       OR NEW.provider_key IS DISTINCT FROM OLD.provider_key
       OR NEW.provider_model_id IS DISTINCT FROM OLD.provider_model_id
       OR NEW.task_class IS DISTINCT FROM OLD.task_class
       OR NEW.provider_data_approval_id IS DISTINCT FROM OLD.provider_data_approval_id
       OR NEW.required_provider_data_class IS DISTINCT FROM OLD.required_provider_data_class
       OR NEW.execution_environment_class IS DISTINCT FROM OLD.execution_environment_class
       OR NEW.started_at IS DISTINCT FROM OLD.started_at
       OR NEW.created_at IS DISTINCT FROM OLD.created_at
       OR NEW.deleted_at IS NOT NULL
       OR NEW.status NOT IN ('succeeded', 'failed', 'cancelled', 'interrupted')
       OR NEW.updated_at <= OLD.updated_at THEN
        RAISE EXCEPTION 'invalid Agent provider attempt lifecycle transition';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
