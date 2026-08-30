-- Owns the tenant-scoped durable Agent session, run queue, and reduced execution trail.
-- User-visible messages and execution evidence are immutable; provider bodies, credentials,
-- raw capability input/output, and future usage or approval ledgers do not belong here.

CREATE OR REPLACE FUNCTION agent_valid_resource_references(references_json JSONB)
RETURNS BOOLEAN AS $$
DECLARE
    resource JSONB;
BEGIN
    IF references_json IS NULL
       OR JSONB_TYPEOF(references_json) <> 'array'
       OR JSONB_ARRAY_LENGTH(references_json) NOT BETWEEN 1 AND 32
       OR OCTET_LENGTH(references_json::TEXT) > 8192 THEN
        RETURN FALSE;
    END IF;

    FOR resource IN SELECT value FROM JSONB_ARRAY_ELEMENTS(references_json)
    LOOP
        IF JSONB_TYPEOF(resource) <> 'object'
           OR NOT (resource ? 'kind')
           OR NOT (resource ? 'id')
           OR EXISTS (
               SELECT 1
               FROM JSONB_OBJECT_KEYS(resource) AS resource_key(key)
               WHERE resource_key.key NOT IN ('kind', 'id')
           )
           OR JSONB_TYPEOF(resource->'kind') <> 'string'
           OR JSONB_TYPEOF(resource->'id') <> 'string'
           OR CHAR_LENGTH(BTRIM(resource->>'kind')) NOT BETWEEN 1 AND 120
           OR CHAR_LENGTH(BTRIM(resource->>'id')) NOT BETWEEN 1 AND 240 THEN
            RETURN FALSE;
        END IF;
    END LOOP;

    RETURN TRUE;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

CREATE OR REPLACE FUNCTION agent_valid_checkpoint_transition(
    previous_checkpoint TEXT,
    next_checkpoint TEXT
)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN previous_checkpoint = next_checkpoint
        OR (previous_checkpoint = 'queued' AND next_checkpoint = 'before_provider')
        OR (previous_checkpoint = 'before_provider' AND next_checkpoint = 'provider_in_flight')
        OR (
            previous_checkpoint = 'provider_in_flight'
            AND next_checkpoint = 'provider_result_persisted'
        )
        OR (
            previous_checkpoint = 'provider_result_persisted'
            AND next_checkpoint IN ('before_provider', 'capability_in_flight', 'finalizing')
        )
        OR (
            previous_checkpoint = 'capability_in_flight'
            AND next_checkpoint = 'capability_result_persisted'
        )
        OR (
            previous_checkpoint = 'capability_result_persisted'
            AND next_checkpoint IN (
                'before_provider', 'provider_in_flight', 'finalizing'
            )
        );
END;
$$ LANGUAGE plpgsql IMMUTABLE;

ALTER TABLE ai_provider_models
    ADD COLUMN IF NOT EXISTS max_output_tokens BIGINT;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'ai_provider_models_max_output_tokens_check'
          AND conrelid = 'ai_provider_models'::REGCLASS
    ) THEN
        ALTER TABLE ai_provider_models
            ADD CONSTRAINT ai_provider_models_max_output_tokens_check CHECK (
                max_output_tokens IS NULL
                OR max_output_tokens BETWEEN 1 AND 9007199254740991
            );
    END IF;
END;
$$;

CREATE TABLE IF NOT EXISTS agent_threads (
    id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    owner_user_id UUID NOT NULL,
    title TEXT NOT NULL DEFAULT 'New session'
        CHECK (CHAR_LENGTH(BTRIM(title)) BETWEEN 1 AND 120),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
    next_message_sequence BIGINT NOT NULL DEFAULT 1
        CHECK (next_message_sequence BETWEEN 1 AND 9007199254740991),
    version BIGINT NOT NULL DEFAULT 1 CHECK (version BETWEEN 1 AND 9007199254740991),
    last_activity_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ CHECK (deleted_at IS NULL),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_threads_id_tenant_unique UNIQUE (id, tenant_id),
    CONSTRAINT agent_threads_owner_tenant_fk
        FOREIGN KEY (owner_user_id, tenant_id)
        REFERENCES users(id, tenant_id) ON DELETE RESTRICT
);

CREATE INDEX IF NOT EXISTS agent_threads_owner_history_idx
    ON agent_threads (
        tenant_id, owner_user_id, status, last_activity_at DESC, id
    );

CREATE TABLE IF NOT EXISTS agent_thread_members (
    id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    thread_id UUID NOT NULL,
    user_id UUID NOT NULL,
    membership_role TEXT NOT NULL CHECK (membership_role IN ('owner', 'member')),
    added_by UUID NOT NULL,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_thread_members_id_tenant_unique UNIQUE (id, tenant_id),
    CONSTRAINT agent_thread_members_thread_tenant_fk
        FOREIGN KEY (thread_id, tenant_id)
        REFERENCES agent_threads(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_thread_members_user_tenant_fk
        FOREIGN KEY (user_id, tenant_id)
        REFERENCES users(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_thread_members_added_by_tenant_fk
        FOREIGN KEY (added_by, tenant_id)
        REFERENCES users(id, tenant_id) ON DELETE RESTRICT
);

CREATE UNIQUE INDEX IF NOT EXISTS agent_thread_members_active_user_unique
    ON agent_thread_members (thread_id, user_id)
    WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS agent_thread_members_active_owner_unique
    ON agent_thread_members (thread_id)
    WHERE membership_role = 'owner' AND deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS agent_thread_members_user_history_idx
    ON agent_thread_members (tenant_id, user_id, created_at DESC, id)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS agent_messages (
    id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    thread_id UUID NOT NULL,
    sequence BIGINT NOT NULL CHECK (sequence BETWEEN 1 AND 9007199254740991),
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
    user_id UUID,
    content TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(content)) BETWEEN 1 AND 20000),
    deleted_at TIMESTAMPTZ CHECK (deleted_at IS NULL),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_messages_id_tenant_unique UNIQUE (id, tenant_id),
    CONSTRAINT agent_messages_id_tenant_thread_unique UNIQUE (id, tenant_id, thread_id),
    CONSTRAINT agent_messages_thread_sequence_unique UNIQUE (thread_id, sequence),
    CONSTRAINT agent_messages_thread_tenant_fk
        FOREIGN KEY (thread_id, tenant_id)
        REFERENCES agent_threads(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_messages_user_tenant_fk
        FOREIGN KEY (user_id, tenant_id)
        REFERENCES users(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_messages_role_shape_check CHECK (
        (role = 'user' AND user_id IS NOT NULL)
        OR (role = 'assistant' AND user_id IS NULL)
    )
);

CREATE INDEX IF NOT EXISTS agent_messages_thread_history_idx
    ON agent_messages (tenant_id, thread_id, sequence);

CREATE TABLE IF NOT EXISTS agent_runs (
    id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    thread_id UUID NOT NULL,
    request_message_id UUID NOT NULL,
    response_message_id UUID,
    requested_by UUID NOT NULL,
    task_class TEXT NOT NULL CHECK (task_class IN (
        'campus_conversation_search',
        'module_read_reporting',
        'document_extraction',
        'drafting_proposal',
        'approved_operational_action'
    )),
    origin_module_key TEXT NOT NULL CHECK (
        CHAR_LENGTH(origin_module_key) BETWEEN 1 AND 160
        AND origin_module_key = LOWER(BTRIM(origin_module_key))
        AND origin_module_key ~ '^[a-z][a-z0-9_.-]*$'
    ),
    origin_route TEXT NOT NULL CHECK (
        CHAR_LENGTH(origin_route) BETWEEN 1 AND 2048
        AND origin_route LIKE '/%'
        AND origin_route !~ '[[:cntrl:]]'
    ),
    request_id UUID NOT NULL,
    correlation_id UUID NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN (
        'queued', 'running', 'awaiting_approval', 'completed',
        'failed', 'cancelled', 'interrupted'
    )),
    safe_failure_code TEXT CHECK (
        safe_failure_code IS NULL OR (
            CHAR_LENGTH(safe_failure_code) BETWEEN 1 AND 100
            AND safe_failure_code = LOWER(BTRIM(safe_failure_code))
            AND safe_failure_code ~ '^[a-z][a-z0-9_.-]*$'
        )
    ),
    safe_failure_message TEXT CHECK (
        safe_failure_message IS NULL
        OR CHAR_LENGTH(BTRIM(safe_failure_message)) BETWEEN 1 AND 500
    ),
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version BETWEEN 1 AND 9007199254740991),
    deleted_at TIMESTAMPTZ CHECK (deleted_at IS NULL),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_runs_id_tenant_unique UNIQUE (id, tenant_id),
    CONSTRAINT agent_runs_request_message_unique UNIQUE (request_message_id),
    CONSTRAINT agent_runs_thread_tenant_fk
        FOREIGN KEY (thread_id, tenant_id)
        REFERENCES agent_threads(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_runs_request_message_tenant_thread_fk
        FOREIGN KEY (request_message_id, tenant_id, thread_id)
        REFERENCES agent_messages(id, tenant_id, thread_id) ON DELETE RESTRICT,
    CONSTRAINT agent_runs_response_message_tenant_thread_fk
        FOREIGN KEY (response_message_id, tenant_id, thread_id)
        REFERENCES agent_messages(id, tenant_id, thread_id) ON DELETE RESTRICT,
    CONSTRAINT agent_runs_requested_by_tenant_fk
        FOREIGN KEY (requested_by, tenant_id)
        REFERENCES users(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_runs_failure_pair_check CHECK (
        (safe_failure_code IS NULL AND safe_failure_message IS NULL)
        OR (safe_failure_code IS NOT NULL AND safe_failure_message IS NOT NULL)
    ),
    CONSTRAINT agent_runs_state_shape_check CHECK (
        (
            status = 'queued'
            AND started_at IS NULL
            AND finished_at IS NULL
            AND response_message_id IS NULL
            AND safe_failure_code IS NULL
        )
        OR (
            status IN ('running', 'awaiting_approval')
            AND started_at IS NOT NULL
            AND finished_at IS NULL
            AND response_message_id IS NULL
            AND safe_failure_code IS NULL
        )
        OR (
            status = 'completed'
            AND started_at IS NOT NULL
            AND finished_at IS NOT NULL
            AND finished_at >= started_at
            AND response_message_id IS NOT NULL
            AND safe_failure_code IS NULL
        )
        OR (
            status IN ('failed', 'interrupted')
            AND finished_at IS NOT NULL
            AND (started_at IS NULL OR finished_at >= started_at)
            AND response_message_id IS NULL
            AND safe_failure_code IS NOT NULL
        )
        OR (
            status = 'cancelled'
            AND finished_at IS NOT NULL
            AND (started_at IS NULL OR finished_at >= started_at)
            AND response_message_id IS NULL
            AND safe_failure_code IS NULL
        )
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS agent_runs_active_thread_unique
    ON agent_runs (thread_id)
    WHERE status IN ('queued', 'running', 'awaiting_approval');

CREATE UNIQUE INDEX IF NOT EXISTS agent_runs_response_message_unique
    ON agent_runs (response_message_id)
    WHERE response_message_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS agent_runs_thread_history_idx
    ON agent_runs (tenant_id, thread_id, created_at DESC, id);

CREATE INDEX IF NOT EXISTS agent_runs_requester_history_idx
    ON agent_runs (tenant_id, requested_by, created_at DESC, id);

CREATE INDEX IF NOT EXISTS agent_runs_correlation_idx
    ON agent_runs (tenant_id, correlation_id, created_at);

CREATE TABLE IF NOT EXISTS agent_run_queue (
    run_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    state TEXT NOT NULL DEFAULT 'available' CHECK (state IN ('available', 'leased', 'finished')),
    available_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    lease_token UUID,
    leased_by TEXT CHECK (
        leased_by IS NULL OR CHAR_LENGTH(BTRIM(leased_by)) BETWEEN 1 AND 200
    ),
    lease_expires_at TIMESTAMPTZ,
    heartbeat_at TIMESTAMPTZ,
    delivery_attempt SMALLINT NOT NULL DEFAULT 0 CHECK (delivery_attempt BETWEEN 0 AND 3),
    checkpoint TEXT NOT NULL DEFAULT 'queued' CHECK (checkpoint IN (
        'queued', 'before_provider', 'provider_in_flight', 'provider_result_persisted',
        'capability_in_flight', 'capability_result_persisted', 'finalizing'
    )),
    cancel_requested_at TIMESTAMPTZ,
    cancel_requested_by UUID,
    finished_at TIMESTAMPTZ,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version BETWEEN 1 AND 9007199254740991),
    deleted_at TIMESTAMPTZ CHECK (deleted_at IS NULL),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_run_queue_run_tenant_unique UNIQUE (run_id, tenant_id),
    CONSTRAINT agent_run_queue_run_tenant_fk
        FOREIGN KEY (run_id, tenant_id)
        REFERENCES agent_runs(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_run_queue_cancel_user_tenant_fk
        FOREIGN KEY (cancel_requested_by, tenant_id)
        REFERENCES users(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_run_queue_cancel_pair_check CHECK (
        (cancel_requested_at IS NULL AND cancel_requested_by IS NULL)
        OR (cancel_requested_at IS NOT NULL AND cancel_requested_by IS NOT NULL)
    ),
    CONSTRAINT agent_run_queue_state_shape_check CHECK (
        (
            state = 'available'
            AND lease_token IS NULL
            AND leased_by IS NULL
            AND lease_expires_at IS NULL
            AND heartbeat_at IS NULL
            AND finished_at IS NULL
        )
        OR (
            state = 'leased'
            AND lease_token IS NOT NULL
            AND leased_by IS NOT NULL
            AND lease_expires_at IS NOT NULL
            AND heartbeat_at IS NOT NULL
            AND lease_expires_at = heartbeat_at + INTERVAL '30 seconds'
            AND finished_at IS NULL
            AND delivery_attempt > 0
        )
        OR (
            state = 'finished'
            AND lease_token IS NULL
            AND leased_by IS NULL
            AND lease_expires_at IS NULL
            AND heartbeat_at IS NULL
            AND finished_at IS NOT NULL
        )
    )
);

CREATE INDEX IF NOT EXISTS agent_run_queue_claim_idx
    ON agent_run_queue (available_at, run_id)
    WHERE state = 'available';

CREATE INDEX IF NOT EXISTS agent_run_queue_expired_lease_idx
    ON agent_run_queue (lease_expires_at, run_id)
    WHERE state = 'leased';

CREATE INDEX IF NOT EXISTS agent_run_queue_cancellation_idx
    ON agent_run_queue (tenant_id, cancel_requested_at, run_id)
    WHERE cancel_requested_at IS NOT NULL AND state <> 'finished';

-- Event identity is an opaque replay cursor and must be transported as a decimal string;
-- unlike user-facing counters, an identity sequence is not a JavaScript number.
-- Version-one events are notifications only. Consumers reload the reduced run, message,
-- provider-attempt, or capability-call projection instead of trusting event payload data.
CREATE TABLE IF NOT EXISTS agent_run_events (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    run_id UUID NOT NULL,
    event_type TEXT NOT NULL CHECK (event_type IN (
        'queued', 'started', 'provider_attempt_started', 'provider_attempt_finished',
        'capability_call_started', 'capability_call_finished', 'message_created',
        'completed', 'failed', 'cancelled', 'interrupted'
    )),
    payload JSONB NOT NULL DEFAULT '{}'::JSONB CHECK (payload = '{}'::JSONB),
    deleted_at TIMESTAMPTZ CHECK (deleted_at IS NULL),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_run_events_id_tenant_unique UNIQUE (id, tenant_id),
    CONSTRAINT agent_run_events_run_tenant_fk
        FOREIGN KEY (run_id, tenant_id)
        REFERENCES agent_runs(id, tenant_id) ON DELETE RESTRICT
);

CREATE INDEX IF NOT EXISTS agent_run_events_replay_idx
    ON agent_run_events (tenant_id, run_id, id);

CREATE UNIQUE INDEX IF NOT EXISTS agent_run_events_singleton_type_unique
    ON agent_run_events (run_id, event_type)
    WHERE event_type IN (
        'queued', 'started', 'message_created', 'completed',
        'failed', 'cancelled', 'interrupted'
    );

CREATE TABLE IF NOT EXISTS agent_provider_attempts (
    id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    run_id UUID NOT NULL,
    turn_index SMALLINT NOT NULL CHECK (turn_index BETWEEN 1 AND 16),
    attempt_index SMALLINT NOT NULL CHECK (attempt_index BETWEEN 1 AND 3),
    route_set_id UUID NOT NULL,
    route_version BIGINT NOT NULL CHECK (route_version BETWEEN 1 AND 9007199254740991),
    route_target_id UUID NOT NULL,
    connection_id UUID NOT NULL,
    credential_version BIGINT NOT NULL
        CHECK (credential_version BETWEEN 1 AND 9007199254740991),
    model_snapshot_id UUID NOT NULL,
    provider_key TEXT NOT NULL CHECK (provider_key IN ('openai', 'anthropic', 'openrouter')),
    provider_model_id TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(provider_model_id)) BETWEEN 1 AND 240),
    task_class TEXT NOT NULL CHECK (task_class IN (
        'campus_conversation_search',
        'module_read_reporting',
        'document_extraction',
        'drafting_proposal',
        'approved_operational_action'
    )),
    status TEXT NOT NULL DEFAULT 'running' CHECK (status IN (
        'running', 'succeeded', 'failed', 'cancelled', 'interrupted'
    )),
    failure_origin TEXT CHECK (
        failure_origin IS NULL OR failure_origin IN ('preflight', 'upstream')
    ),
    failure_category TEXT CHECK (
        failure_category IS NULL OR failure_category IN (
            'connection_unavailable', 'stale_credential', 'stale_model',
            'tools_unsupported', 'model_context_unavailable',
            'context_window_exceeded', 'credential_unavailable',
            'invalid_configuration', 'invalid_input', 'storage_error',
            'authentication', 'rate_limited', 'unavailable', 'timeout',
            'network', 'invalid_response', 'unsupported'
        )
    ),
    input_tokens BIGINT CHECK (
        input_tokens IS NULL OR input_tokens BETWEEN 0 AND 9007199254740991
    ),
    output_tokens BIGINT CHECK (
        output_tokens IS NULL OR output_tokens BETWEEN 0 AND 9007199254740991
    ),
    cached_tokens BIGINT CHECK (
        cached_tokens IS NULL OR cached_tokens BETWEEN 0 AND 9007199254740991
    ),
    reasoning_tokens BIGINT CHECK (
        reasoning_tokens IS NULL OR reasoning_tokens BETWEEN 0 AND 9007199254740991
    ),
    provider_reported_cost_amount BIGINT CHECK (
        provider_reported_cost_amount IS NULL
        OR provider_reported_cost_amount BETWEEN 0 AND 9007199254740991
    ),
    provider_reported_cost_currency TEXT CHECK (
        provider_reported_cost_currency IS NULL
        OR provider_reported_cost_currency ~ '^[A-Z]{3}$'
    ),
    provider_reported_cost_exponent SMALLINT CHECK (
        provider_reported_cost_exponent IS NULL
        OR provider_reported_cost_exponent BETWEEN 0 AND 9
    ),
    provider_reported_pricing_version TEXT CHECK (
        provider_reported_pricing_version IS NULL
        OR CHAR_LENGTH(BTRIM(provider_reported_pricing_version)) BETWEEN 1 AND 100
    ),
    estimated_cost_amount BIGINT CHECK (
        estimated_cost_amount IS NULL
        OR estimated_cost_amount BETWEEN 0 AND 9007199254740991
    ),
    estimated_cost_currency TEXT CHECK (
        estimated_cost_currency IS NULL OR estimated_cost_currency ~ '^[A-Z]{3}$'
    ),
    estimated_cost_exponent SMALLINT CHECK (
        estimated_cost_exponent IS NULL OR estimated_cost_exponent BETWEEN 0 AND 9
    ),
    estimated_pricing_version TEXT CHECK (
        estimated_pricing_version IS NULL
        OR CHAR_LENGTH(BTRIM(estimated_pricing_version)) BETWEEN 1 AND 100
    ),
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ CHECK (deleted_at IS NULL),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_provider_attempts_id_tenant_unique UNIQUE (id, tenant_id),
    CONSTRAINT agent_provider_attempts_id_tenant_run_unique UNIQUE (id, tenant_id, run_id),
    CONSTRAINT agent_provider_attempts_run_turn_index_unique
        UNIQUE (run_id, turn_index, attempt_index),
    CONSTRAINT agent_provider_attempts_run_tenant_fk
        FOREIGN KEY (run_id, tenant_id)
        REFERENCES agent_runs(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_provider_attempts_route_set_tenant_fk
        FOREIGN KEY (route_set_id, tenant_id)
        REFERENCES ai_route_sets(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_provider_attempts_route_target_tenant_fk
        FOREIGN KEY (route_target_id, tenant_id)
        REFERENCES ai_task_routes(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_provider_attempts_connection_tenant_fk
        FOREIGN KEY (connection_id, tenant_id)
        REFERENCES ai_provider_connections(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_provider_attempts_model_tenant_fk
        FOREIGN KEY (model_snapshot_id, tenant_id)
        REFERENCES ai_provider_models(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_provider_attempts_provider_cost_shape_check CHECK (
        (
            provider_reported_cost_amount IS NULL
            AND provider_reported_cost_currency IS NULL
            AND provider_reported_cost_exponent IS NULL
            AND provider_reported_pricing_version IS NULL
        )
        OR (
            provider_reported_cost_amount IS NOT NULL
            AND provider_reported_cost_currency IS NOT NULL
            AND provider_reported_cost_exponent IS NOT NULL
        )
    ),
    CONSTRAINT agent_provider_attempts_estimated_cost_shape_check CHECK (
        (
            estimated_cost_amount IS NULL
            AND estimated_cost_currency IS NULL
            AND estimated_cost_exponent IS NULL
            AND estimated_pricing_version IS NULL
        )
        OR (
            estimated_cost_amount IS NOT NULL
            AND estimated_cost_currency IS NOT NULL
            AND estimated_cost_exponent IS NOT NULL
            AND estimated_pricing_version IS NOT NULL
        )
    ),
    CONSTRAINT agent_provider_attempts_failure_shape_check CHECK (
        (
            status = 'failed'
            AND (
                (
                    failure_origin = 'preflight'
                    AND failure_category IN (
                        'connection_unavailable', 'stale_credential', 'stale_model',
                        'tools_unsupported', 'model_context_unavailable',
                        'context_window_exceeded', 'credential_unavailable',
                        'invalid_configuration', 'invalid_input', 'storage_error'
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
    ),
    CONSTRAINT agent_provider_attempts_preflight_usage_check CHECK (
        failure_origin <> 'preflight'
        OR (
            input_tokens IS NULL
            AND output_tokens IS NULL
            AND cached_tokens IS NULL
            AND reasoning_tokens IS NULL
            AND provider_reported_cost_amount IS NULL
            AND provider_reported_cost_currency IS NULL
            AND provider_reported_cost_exponent IS NULL
            AND provider_reported_pricing_version IS NULL
            AND estimated_cost_amount IS NULL
            AND estimated_cost_currency IS NULL
            AND estimated_cost_exponent IS NULL
            AND estimated_pricing_version IS NULL
        )
    ),
    CONSTRAINT agent_provider_attempts_state_shape_check CHECK (
        (
            status = 'running'
            AND finished_at IS NULL
            AND failure_origin IS NULL
            AND failure_category IS NULL
            AND input_tokens IS NULL
            AND output_tokens IS NULL
            AND cached_tokens IS NULL
            AND reasoning_tokens IS NULL
            AND provider_reported_cost_amount IS NULL
            AND estimated_cost_amount IS NULL
        )
        OR (
            status = 'succeeded'
            AND finished_at IS NOT NULL
            AND finished_at >= started_at
            AND failure_origin IS NULL
            AND failure_category IS NULL
        )
        OR (
            status = 'failed'
            AND finished_at IS NOT NULL
            AND finished_at >= started_at
            AND failure_origin IS NOT NULL
            AND failure_category IS NOT NULL
        )
        OR (
            status IN ('cancelled', 'interrupted')
            AND finished_at IS NOT NULL
            AND finished_at >= started_at
            AND failure_origin IS NULL
            AND failure_category IS NULL
        )
    )
);

CREATE INDEX IF NOT EXISTS agent_provider_attempts_run_history_idx
    ON agent_provider_attempts (tenant_id, run_id, turn_index, attempt_index);

CREATE INDEX IF NOT EXISTS agent_provider_attempts_reporting_idx
    ON agent_provider_attempts (tenant_id, provider_key, task_class, started_at DESC, id);

CREATE TABLE IF NOT EXISTS agent_capability_calls (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    run_id UUID NOT NULL,
    call_sequence SMALLINT NOT NULL CHECK (call_sequence BETWEEN 1 AND 16),
    capability_key TEXT NOT NULL CHECK (
        CHAR_LENGTH(capability_key) BETWEEN 1 AND 200
        AND capability_key = LOWER(BTRIM(capability_key))
        AND capability_key ~ '^[a-z][a-z0-9_.-]*$'
    ),
    capability_version INTEGER NOT NULL CHECK (capability_version > 0),
    product_operation_key TEXT NOT NULL CHECK (
        CHAR_LENGTH(product_operation_key) BETWEEN 1 AND 240
        AND product_operation_key = LOWER(BTRIM(product_operation_key))
        AND product_operation_key ~ '^[a-z][a-z0-9_.-]*$'
    ),
    owning_module_key TEXT NOT NULL CHECK (
        CHAR_LENGTH(owning_module_key) BETWEEN 1 AND 160
        AND owning_module_key = LOWER(BTRIM(owning_module_key))
        AND owning_module_key ~ '^[a-z][a-z0-9_.-]*$'
    ),
    required_permission TEXT NOT NULL CHECK (
        CHAR_LENGTH(required_permission) BETWEEN 3 AND 200
        AND required_permission = LOWER(BTRIM(required_permission))
        AND required_permission ~ '^[a-z][a-z0-9_.-]*:[a-z][a-z0-9_.-]*$'
    ),
    input_fingerprint BYTEA NOT NULL CHECK (OCTET_LENGTH(input_fingerprint) = 32),
    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('tenant_wide', 'resources')),
    resource_references JSONB NOT NULL DEFAULT '[]'::JSONB,
    approval_state TEXT NOT NULL DEFAULT 'not_required' CHECK (approval_state = 'not_required'),
    status TEXT NOT NULL DEFAULT 'running' CHECK (status IN (
        'running', 'succeeded', 'failed', 'denied', 'cancelled', 'interrupted'
    )),
    safe_failure_code TEXT CHECK (
        safe_failure_code IS NULL OR (
            CHAR_LENGTH(safe_failure_code) BETWEEN 1 AND 100
            AND safe_failure_code = LOWER(BTRIM(safe_failure_code))
            AND safe_failure_code ~ '^[a-z][a-z0-9_.-]*$'
        )
    ),
    duration_ms BIGINT CHECK (
        duration_ms IS NULL OR duration_ms BETWEEN 0 AND 9007199254740991
    ),
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ CHECK (deleted_at IS NULL),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_capability_calls_id_tenant_unique UNIQUE (id, tenant_id),
    CONSTRAINT agent_capability_calls_id_tenant_run_unique UNIQUE (id, tenant_id, run_id),
    CONSTRAINT agent_capability_calls_run_sequence_unique UNIQUE (run_id, call_sequence),
    CONSTRAINT agent_capability_calls_run_tenant_fk
        FOREIGN KEY (run_id, tenant_id)
        REFERENCES agent_runs(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_capability_calls_scope_shape_check CHECK (
        (
            scope_kind = 'tenant_wide'
            AND resource_references = '[]'::JSONB
        )
        OR (
            scope_kind = 'resources'
            AND agent_valid_resource_references(resource_references)
        )
    ),
    CONSTRAINT agent_capability_calls_state_shape_check CHECK (
        (
            status = 'running'
            AND finished_at IS NULL
            AND safe_failure_code IS NULL
            AND duration_ms IS NULL
        )
        OR (
            status = 'succeeded'
            AND finished_at IS NOT NULL
            AND finished_at >= started_at
            AND safe_failure_code IS NULL
            AND duration_ms IS NOT NULL
        )
        OR (
            status IN ('failed', 'denied', 'interrupted')
            AND finished_at IS NOT NULL
            AND finished_at >= started_at
            AND safe_failure_code IS NOT NULL
            AND duration_ms IS NOT NULL
        )
        OR (
            status = 'cancelled'
            AND finished_at IS NOT NULL
            AND finished_at >= started_at
            AND safe_failure_code IS NULL
            AND duration_ms IS NOT NULL
        )
    )
);

CREATE INDEX IF NOT EXISTS agent_capability_calls_run_history_idx
    ON agent_capability_calls (tenant_id, run_id, call_sequence);

CREATE INDEX IF NOT EXISTS agent_capability_calls_reporting_idx
    ON agent_capability_calls (
        tenant_id, owning_module_key, capability_key, started_at DESC, id
    );

-- Private resumability trail. These rows are not projected through public Session APIs.
CREATE TABLE IF NOT EXISTS agent_execution_steps (
    id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    run_id UUID NOT NULL,
    step_index SMALLINT NOT NULL CHECK (step_index BETWEEN 1 AND 65),
    turn_index SMALLINT NOT NULL CHECK (turn_index BETWEEN 1 AND 16),
    step_kind TEXT NOT NULL CHECK (
        step_kind IN ('provider_attempt', 'capability_call', 'finalize')
    ),
    provider_attempt_id UUID,
    capability_call_id UUID,
    input_fingerprint BYTEA NOT NULL CHECK (OCTET_LENGTH(input_fingerprint) = 32),
    status TEXT NOT NULL DEFAULT 'running' CHECK (
        status IN ('running', 'succeeded', 'failed', 'cancelled', 'interrupted')
    ),
    safe_failure_code TEXT CHECK (
        safe_failure_code IS NULL OR (
            CHAR_LENGTH(safe_failure_code) BETWEEN 1 AND 100
            AND safe_failure_code = LOWER(BTRIM(safe_failure_code))
            AND safe_failure_code ~ '^[a-z][a-z0-9_.-]*$'
        )
    ),
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ CHECK (deleted_at IS NULL),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_execution_steps_id_tenant_unique UNIQUE (id, tenant_id),
    CONSTRAINT agent_execution_steps_id_tenant_run_unique UNIQUE (id, tenant_id, run_id),
    CONSTRAINT agent_execution_steps_run_index_unique UNIQUE (run_id, step_index),
    CONSTRAINT agent_execution_steps_run_tenant_fk
        FOREIGN KEY (run_id, tenant_id)
        REFERENCES agent_runs(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_execution_steps_provider_attempt_tenant_run_fk
        FOREIGN KEY (provider_attempt_id, tenant_id, run_id)
        REFERENCES agent_provider_attempts(id, tenant_id, run_id) ON DELETE RESTRICT,
    CONSTRAINT agent_execution_steps_capability_call_tenant_run_fk
        FOREIGN KEY (capability_call_id, tenant_id, run_id)
        REFERENCES agent_capability_calls(id, tenant_id, run_id) ON DELETE RESTRICT,
    CONSTRAINT agent_execution_steps_kind_shape_check CHECK (
        (
            step_kind = 'provider_attempt'
            AND provider_attempt_id IS NOT NULL
            AND capability_call_id IS NULL
        )
        OR (
            step_kind = 'capability_call'
            AND provider_attempt_id IS NULL
            AND capability_call_id IS NOT NULL
        )
        OR (
            step_kind = 'finalize'
            AND provider_attempt_id IS NULL
            AND capability_call_id IS NULL
        )
    ),
    CONSTRAINT agent_execution_steps_state_shape_check CHECK (
        (
            status = 'running'
            AND safe_failure_code IS NULL
            AND finished_at IS NULL
        )
        OR (
            status = 'succeeded'
            AND safe_failure_code IS NULL
            AND finished_at IS NOT NULL
            AND finished_at >= started_at
        )
        OR (
            status IN ('failed', 'interrupted')
            AND safe_failure_code IS NOT NULL
            AND finished_at IS NOT NULL
            AND finished_at >= started_at
        )
        OR (
            status = 'cancelled'
            AND safe_failure_code IS NULL
            AND finished_at IS NOT NULL
            AND finished_at >= started_at
        )
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS agent_execution_steps_provider_attempt_unique
    ON agent_execution_steps (provider_attempt_id)
    WHERE provider_attempt_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS agent_execution_steps_capability_call_unique
    ON agent_execution_steps (capability_call_id)
    WHERE capability_call_id IS NOT NULL;

-- V1 accepts one normalized capability call from each provider turn. Providers may
-- return multiple calls, but the worker rejects that response before durable dispatch.
CREATE UNIQUE INDEX IF NOT EXISTS agent_execution_steps_capability_turn_unique
    ON agent_execution_steps (run_id, turn_index)
    WHERE step_kind = 'capability_call';

CREATE UNIQUE INDEX IF NOT EXISTS agent_execution_steps_finalize_unique
    ON agent_execution_steps (run_id)
    WHERE step_kind = 'finalize';

CREATE INDEX IF NOT EXISTS agent_execution_steps_run_history_idx
    ON agent_execution_steps (tenant_id, run_id, step_index);

-- Result-only continuation envelopes. Plaintext, provider bodies, capability input/output,
-- keys, and credentials are deliberately absent; ciphertext includes AEAD overhead only.
CREATE TABLE IF NOT EXISTS agent_execution_artifacts (
    id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    run_id UUID NOT NULL,
    step_id UUID NOT NULL,
    artifact_sequence SMALLINT NOT NULL CHECK (artifact_sequence BETWEEN 1 AND 33),
    artifact_kind TEXT NOT NULL CHECK (
        artifact_kind IN ('provider_result', 'capability_result', 'final_response')
    ),
    ciphertext BYTEA NOT NULL CHECK (OCTET_LENGTH(ciphertext) BETWEEN 1 AND 65552),
    ciphertext_sha256 BYTEA NOT NULL CHECK (OCTET_LENGTH(ciphertext_sha256) = 32),
    plaintext_sha256 BYTEA NOT NULL CHECK (OCTET_LENGTH(plaintext_sha256) = 32),
    nonce BYTEA NOT NULL CHECK (OCTET_LENGTH(nonce) BETWEEN 12 AND 32),
    encryption_key_id TEXT NOT NULL CHECK (
        CHAR_LENGTH(BTRIM(encryption_key_id)) BETWEEN 1 AND 200
        AND encryption_key_id !~ '[[:cntrl:]]'
    ),
    encryption_key_version BIGINT NOT NULL CHECK (
        encryption_key_version BETWEEN 1 AND 9007199254740991
    ),
    plaintext_length INTEGER NOT NULL CHECK (plaintext_length BETWEEN 1 AND 65536),
    deleted_at TIMESTAMPTZ CHECK (deleted_at IS NULL),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_execution_artifacts_id_tenant_unique UNIQUE (id, tenant_id),
    CONSTRAINT agent_execution_artifacts_run_sequence_unique
        UNIQUE (run_id, artifact_sequence),
    CONSTRAINT agent_execution_artifacts_step_unique UNIQUE (step_id),
    CONSTRAINT agent_execution_artifacts_run_tenant_fk
        FOREIGN KEY (run_id, tenant_id)
        REFERENCES agent_runs(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_execution_artifacts_step_tenant_run_fk
        FOREIGN KEY (step_id, tenant_id, run_id)
        REFERENCES agent_execution_steps(id, tenant_id, run_id) ON DELETE RESTRICT
);

CREATE UNIQUE INDEX IF NOT EXISTS agent_execution_artifacts_final_response_unique
    ON agent_execution_artifacts (run_id)
    WHERE artifact_kind = 'final_response';

CREATE INDEX IF NOT EXISTS agent_execution_artifacts_run_history_idx
    ON agent_execution_artifacts (tenant_id, run_id, artifact_sequence);

CREATE TABLE IF NOT EXISTS agent_request_idempotency (
    id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID NOT NULL,
    operation_key TEXT NOT NULL CHECK (
        CHAR_LENGTH(operation_key) BETWEEN 1 AND 200
        AND operation_key = LOWER(BTRIM(operation_key))
        AND operation_key ~ '^[a-z][a-z0-9_.-]*$'
    ),
    scope_id UUID,
    idempotency_key TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(idempotency_key)) BETWEEN 1 AND 200),
    request_fingerprint BYTEA NOT NULL CHECK (OCTET_LENGTH(request_fingerprint) = 32),
    result_kind TEXT NOT NULL CHECK (result_kind IN ('thread', 'run')),
    result_id UUID NOT NULL,
    deleted_at TIMESTAMPTZ CHECK (deleted_at IS NULL),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_request_idempotency_id_tenant_unique UNIQUE (id, tenant_id),
    CONSTRAINT agent_request_idempotency_user_tenant_fk
        FOREIGN KEY (user_id, tenant_id)
        REFERENCES users(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_request_idempotency_scope_tenant_fk
        FOREIGN KEY (scope_id, tenant_id)
        REFERENCES agent_threads(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_request_idempotency_result_scope_check CHECK (
        (result_kind = 'thread' AND scope_id IS NULL)
        OR (result_kind = 'run' AND scope_id IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS agent_request_idempotency_key_unique
    ON agent_request_idempotency (
        tenant_id,
        user_id,
        operation_key,
        scope_id,
        idempotency_key
    ) NULLS NOT DISTINCT;

CREATE INDEX IF NOT EXISTS agent_request_idempotency_result_idx
    ON agent_request_idempotency (tenant_id, result_kind, result_id);

CREATE OR REPLACE FUNCTION validate_agent_thread_insert()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.status <> 'active'
       OR NEW.next_message_sequence <> 1
       OR NEW.version <> 1
       OR NEW.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'Agent Sessions must start active at their initial version and sequence';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_threads_validate_insert ON agent_threads;
CREATE TRIGGER agent_threads_validate_insert
    BEFORE INSERT ON agent_threads
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_thread_insert();

CREATE OR REPLACE FUNCTION protect_agent_thread_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Agent Sessions are archived, not deleted';
    END IF;

    IF OLD.status = 'archived' THEN
        RAISE EXCEPTION 'archived Agent Sessions are immutable';
    END IF;

    IF NEW.id IS DISTINCT FROM OLD.id
       OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
       OR NEW.owner_user_id IS DISTINCT FROM OLD.owner_user_id
       OR NEW.created_at IS DISTINCT FROM OLD.created_at
       OR NEW.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'Agent Session identity is immutable';
    END IF;

    IF NEW.status NOT IN ('active', 'archived')
       OR NEW.next_message_sequence < OLD.next_message_sequence
       OR NEW.next_message_sequence > OLD.next_message_sequence + 1
       OR NEW.last_activity_at < OLD.last_activity_at
       OR NEW.version <> OLD.version + 1
       OR NEW.updated_at <= OLD.updated_at THEN
        RAISE EXCEPTION 'invalid Agent Session lifecycle update';
    END IF;

    IF NEW.status = 'archived'
       AND EXISTS (
           SELECT 1
           FROM agent_runs
           WHERE thread_id = OLD.id
             AND tenant_id = OLD.tenant_id
             AND status IN ('queued', 'running', 'awaiting_approval')
       ) THEN
        RAISE EXCEPTION 'Agent Sessions with active runs cannot be archived';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_threads_protect_lifecycle ON agent_threads;
CREATE TRIGGER agent_threads_protect_lifecycle
    BEFORE UPDATE OR DELETE ON agent_threads
    FOR EACH ROW
    EXECUTE FUNCTION protect_agent_thread_lifecycle();

CREATE OR REPLACE FUNCTION validate_agent_thread_member()
RETURNS TRIGGER AS $$
DECLARE
    thread_owner UUID;
    thread_status TEXT;
BEGIN
    IF NEW.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'Agent Session memberships must start active';
    END IF;

    SELECT owner_user_id, status
    INTO thread_owner, thread_status
    FROM agent_threads
    WHERE id = NEW.thread_id
      AND tenant_id = NEW.tenant_id;

    IF NOT FOUND OR thread_status <> 'active' THEN
        RAISE EXCEPTION 'Agent Session membership requires an active same-tenant Session';
    END IF;

    IF (NEW.membership_role = 'owner' AND NEW.user_id <> thread_owner)
       OR (NEW.membership_role = 'member' AND NEW.user_id = thread_owner) THEN
        RAISE EXCEPTION 'Agent Session membership role does not match the Session owner';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_thread_members_validate_insert ON agent_thread_members;
CREATE TRIGGER agent_thread_members_validate_insert
    BEFORE INSERT ON agent_thread_members
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_thread_member();

CREATE OR REPLACE FUNCTION protect_agent_thread_member_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Agent Session memberships are revoked, not deleted';
    END IF;

    IF OLD.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'revoked Agent Session memberships are immutable';
    END IF;

    IF NEW.id IS DISTINCT FROM OLD.id
       OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
       OR NEW.thread_id IS DISTINCT FROM OLD.thread_id
       OR NEW.user_id IS DISTINCT FROM OLD.user_id
       OR NEW.membership_role IS DISTINCT FROM OLD.membership_role
       OR NEW.added_by IS DISTINCT FROM OLD.added_by
       OR NEW.created_at IS DISTINCT FROM OLD.created_at
       OR NEW.updated_at <= OLD.updated_at THEN
        RAISE EXCEPTION 'Agent Session membership identity is immutable';
    END IF;

    IF OLD.membership_role = 'owner' OR NEW.deleted_at IS NULL THEN
        RAISE EXCEPTION 'only non-owner Agent Session memberships may be revoked';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_thread_members_protect_lifecycle ON agent_thread_members;
CREATE TRIGGER agent_thread_members_protect_lifecycle
    BEFORE UPDATE OR DELETE ON agent_thread_members
    FOR EACH ROW
    EXECUTE FUNCTION protect_agent_thread_member_lifecycle();

CREATE OR REPLACE FUNCTION validate_agent_message_insert()
RETURNS TRIGGER AS $$
DECLARE
    next_sequence BIGINT;
    thread_status TEXT;
BEGIN
    SELECT next_message_sequence, status
    INTO next_sequence, thread_status
    FROM agent_threads
    WHERE id = NEW.thread_id
      AND tenant_id = NEW.tenant_id;

    IF NOT FOUND OR thread_status <> 'active' THEN
        RAISE EXCEPTION 'Agent messages require an active same-tenant Session';
    END IF;

    IF NEW.sequence <> next_sequence - 1 THEN
        RAISE EXCEPTION 'Agent message sequence must be allocated by its Session';
    END IF;

    IF NEW.role = 'user' AND NOT EXISTS (
        SELECT 1
        FROM agent_thread_members
        WHERE thread_id = NEW.thread_id
          AND tenant_id = NEW.tenant_id
          AND user_id = NEW.user_id
          AND membership_role = 'owner'
          AND deleted_at IS NULL
    ) THEN
        RAISE EXCEPTION 'Agent user messages require active owner membership';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_messages_validate_insert ON agent_messages;
CREATE TRIGGER agent_messages_validate_insert
    BEFORE INSERT ON agent_messages
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_message_insert();

CREATE OR REPLACE FUNCTION reject_agent_append_only_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION '% is append-only', TG_TABLE_NAME;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_messages_reject_mutation ON agent_messages;
CREATE TRIGGER agent_messages_reject_mutation
    BEFORE UPDATE OR DELETE ON agent_messages
    FOR EACH ROW
    EXECUTE FUNCTION reject_agent_append_only_mutation();

CREATE OR REPLACE FUNCTION validate_agent_run_messages()
RETURNS TRIGGER AS $$
DECLARE
    request_role TEXT;
    request_user UUID;
    response_role TEXT;
    thread_owner UUID;
    thread_status TEXT;
BEGIN
    SELECT role, user_id
    INTO request_role, request_user
    FROM agent_messages
    WHERE id = NEW.request_message_id
      AND tenant_id = NEW.tenant_id
      AND thread_id = NEW.thread_id;

    IF NOT FOUND OR request_role <> 'user' OR request_user <> NEW.requested_by THEN
        RAISE EXCEPTION 'Agent run request must be the requester''s same-Session user message';
    END IF;

    SELECT owner_user_id, status
    INTO thread_owner, thread_status
    FROM agent_threads
    WHERE id = NEW.thread_id
      AND tenant_id = NEW.tenant_id;

    IF NOT FOUND
       OR thread_status <> 'active'
       OR thread_owner <> NEW.requested_by
       OR NOT EXISTS (
           SELECT 1
           FROM agent_thread_members
           WHERE thread_id = NEW.thread_id
             AND tenant_id = NEW.tenant_id
             AND user_id = NEW.requested_by
             AND membership_role = 'owner'
             AND deleted_at IS NULL
       ) THEN
        RAISE EXCEPTION 'Agent runs require the active same-tenant Session owner';
    END IF;

    IF NEW.response_message_id IS NOT NULL THEN
        SELECT role
        INTO response_role
        FROM agent_messages
        WHERE id = NEW.response_message_id
          AND tenant_id = NEW.tenant_id
          AND thread_id = NEW.thread_id;

        IF NOT FOUND OR response_role <> 'assistant' THEN
            RAISE EXCEPTION 'Agent run response must be a same-Session assistant message';
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_runs_validate_messages ON agent_runs;
CREATE TRIGGER agent_runs_validate_messages
    BEFORE INSERT OR UPDATE OF response_message_id ON agent_runs
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_run_messages();

CREATE OR REPLACE FUNCTION validate_agent_run_insert()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.status <> 'queued'
       OR NEW.response_message_id IS NOT NULL
       OR NEW.safe_failure_code IS NOT NULL
       OR NEW.safe_failure_message IS NOT NULL
       OR NEW.started_at IS NOT NULL
       OR NEW.finished_at IS NOT NULL
       OR NEW.version <> 1
       OR NEW.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'Agent runs must start in the initial queued state';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_runs_validate_insert ON agent_runs;
CREATE TRIGGER agent_runs_validate_insert
    BEFORE INSERT ON agent_runs
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_run_insert();

CREATE OR REPLACE FUNCTION protect_agent_run_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Agent runs are retained, not deleted';
    END IF;

    IF OLD.status IN ('completed', 'failed', 'cancelled', 'interrupted') THEN
        RAISE EXCEPTION 'terminal Agent runs are immutable';
    END IF;

    IF NEW.id IS DISTINCT FROM OLD.id
       OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
       OR NEW.thread_id IS DISTINCT FROM OLD.thread_id
       OR NEW.request_message_id IS DISTINCT FROM OLD.request_message_id
       OR NEW.requested_by IS DISTINCT FROM OLD.requested_by
       OR NEW.task_class IS DISTINCT FROM OLD.task_class
       OR NEW.origin_module_key IS DISTINCT FROM OLD.origin_module_key
       OR NEW.origin_route IS DISTINCT FROM OLD.origin_route
       OR NEW.request_id IS DISTINCT FROM OLD.request_id
       OR NEW.correlation_id IS DISTINCT FROM OLD.correlation_id
       OR NEW.created_at IS DISTINCT FROM OLD.created_at
       OR NEW.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'Agent run identity is immutable';
    END IF;

    IF NOT (
        (OLD.status = 'queued' AND NEW.status IN ('running', 'failed', 'cancelled'))
        OR (
            OLD.status = 'running'
            AND NEW.status IN (
                'completed', 'failed', 'cancelled', 'interrupted', 'awaiting_approval'
            )
        )
        OR (
            OLD.status = 'awaiting_approval'
            AND NEW.status IN ('queued', 'failed', 'cancelled', 'interrupted')
        )
    )
       OR NEW.version <> OLD.version + 1
       OR NEW.updated_at <= OLD.updated_at THEN
        RAISE EXCEPTION 'invalid Agent run lifecycle transition';
    END IF;

    IF NEW.status IN ('completed', 'failed', 'cancelled', 'interrupted')
       AND (
           EXISTS (
               SELECT 1
               FROM agent_provider_attempts
               WHERE run_id = OLD.id
                 AND tenant_id = OLD.tenant_id
                 AND status = 'running'
           )
           OR EXISTS (
               SELECT 1
               FROM agent_capability_calls
               WHERE run_id = OLD.id
                 AND tenant_id = OLD.tenant_id
                 AND status = 'running'
           )
           OR EXISTS (
               SELECT 1
               FROM agent_execution_steps
               WHERE run_id = OLD.id
                 AND tenant_id = OLD.tenant_id
                 AND status = 'running'
           )
       ) THEN
        RAISE EXCEPTION 'terminal Agent runs require terminal execution children';
    END IF;

    IF NEW.status = 'completed'
       AND NOT EXISTS (
           SELECT 1
           FROM agent_execution_steps AS execution_step
           JOIN agent_execution_artifacts AS execution_artifact
             ON execution_artifact.step_id = execution_step.id
            AND execution_artifact.tenant_id = execution_step.tenant_id
            AND execution_artifact.run_id = execution_step.run_id
            AND execution_artifact.artifact_kind = 'final_response'
           WHERE execution_step.run_id = OLD.id
             AND execution_step.tenant_id = OLD.tenant_id
             AND execution_step.step_kind = 'finalize'
             AND execution_step.status = 'succeeded'
       ) THEN
        RAISE EXCEPTION 'completed Agent runs require durable finalization evidence';
    END IF;

    IF NEW.status = 'cancelled'
       AND NOT EXISTS (
           SELECT 1
           FROM agent_run_queue
           WHERE run_id = OLD.id
             AND tenant_id = OLD.tenant_id
             AND cancel_requested_at IS NOT NULL
       ) THEN
        RAISE EXCEPTION 'cancelled Agent runs require a cooperative cancellation request';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_runs_protect_lifecycle ON agent_runs;
CREATE TRIGGER agent_runs_protect_lifecycle
    BEFORE UPDATE OR DELETE ON agent_runs
    FOR EACH ROW
    EXECUTE FUNCTION protect_agent_run_lifecycle();

CREATE OR REPLACE FUNCTION validate_agent_run_queue_insert()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.state <> 'available'
       OR NEW.lease_token IS NOT NULL
       OR NEW.leased_by IS NOT NULL
       OR NEW.lease_expires_at IS NOT NULL
       OR NEW.heartbeat_at IS NOT NULL
       OR NEW.delivery_attempt <> 0
       OR NEW.checkpoint <> 'queued'
       OR NEW.cancel_requested_at IS NOT NULL
       OR NEW.cancel_requested_by IS NOT NULL
       OR NEW.finished_at IS NOT NULL
       OR NEW.version <> 1
       OR NEW.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'Agent queue rows must start in the initial available state';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_run_queue_validate_insert ON agent_run_queue;
CREATE TRIGGER agent_run_queue_validate_insert
    BEFORE INSERT ON agent_run_queue
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_run_queue_insert();

CREATE OR REPLACE FUNCTION protect_agent_run_queue_lifecycle()
RETURNS TRIGGER AS $$
DECLARE
    is_cancellation_request BOOLEAN;
    stored_requested_by UUID;
    stored_run_status TEXT;
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Agent queue rows are retained, not deleted';
    END IF;

    IF OLD.state = 'finished' THEN
        RAISE EXCEPTION 'finished Agent queue rows are immutable';
    END IF;

    is_cancellation_request := OLD.cancel_requested_at IS NULL
        AND NEW.cancel_requested_at IS NOT NULL;

    IF is_cancellation_request THEN
        SELECT requested_by, status
        INTO stored_requested_by, stored_run_status
        FROM agent_runs
        WHERE id = OLD.run_id
          AND tenant_id = OLD.tenant_id;

        IF NEW.cancel_requested_at IS DISTINCT FROM STATEMENT_TIMESTAMP()
           OR NEW.cancel_requested_by IS DISTINCT FROM stored_requested_by
           OR stored_run_status NOT IN ('queued', 'running', 'awaiting_approval')
           OR NEW.run_id IS DISTINCT FROM OLD.run_id
           OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
           OR NEW.state IS DISTINCT FROM OLD.state
           OR NEW.available_at IS DISTINCT FROM OLD.available_at
           OR NEW.lease_token IS DISTINCT FROM OLD.lease_token
           OR NEW.leased_by IS DISTINCT FROM OLD.leased_by
           OR NEW.lease_expires_at IS DISTINCT FROM OLD.lease_expires_at
           OR NEW.heartbeat_at IS DISTINCT FROM OLD.heartbeat_at
           OR NEW.delivery_attempt IS DISTINCT FROM OLD.delivery_attempt
           OR NEW.checkpoint IS DISTINCT FROM OLD.checkpoint
           OR NEW.finished_at IS DISTINCT FROM OLD.finished_at
           OR NEW.version IS DISTINCT FROM OLD.version
           OR NEW.created_at IS DISTINCT FROM OLD.created_at
           OR NEW.deleted_at IS NOT NULL
           OR NEW.updated_at <= OLD.updated_at THEN
            RAISE EXCEPTION 'invalid cooperative Agent cancellation request';
        END IF;

        RETURN NEW;
    END IF;

    IF NEW.run_id IS DISTINCT FROM OLD.run_id
       OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
       OR NEW.created_at IS DISTINCT FROM OLD.created_at
       OR NEW.deleted_at IS NOT NULL
       OR NEW.version <> OLD.version + 1
       OR NEW.updated_at <= OLD.updated_at THEN
        RAISE EXCEPTION 'invalid Agent queue identity or fence';
    END IF;

    IF OLD.cancel_requested_at IS NOT NULL
       AND (
           NEW.cancel_requested_at IS DISTINCT FROM OLD.cancel_requested_at
           OR NEW.cancel_requested_by IS DISTINCT FROM OLD.cancel_requested_by
           OR (
               NEW.state <> 'finished'
               AND NEW.checkpoint IS DISTINCT FROM OLD.checkpoint
               AND NOT (
                   (OLD.checkpoint = 'provider_in_flight'
                    AND NEW.checkpoint = 'provider_result_persisted')
                   OR (OLD.checkpoint = 'capability_in_flight'
                       AND NEW.checkpoint = 'capability_result_persisted')
               )
           )
       ) THEN
        RAISE EXCEPTION 'Agent run cancellation is monotonic and stops new work';
    END IF;

    IF OLD.state = 'available' AND NEW.state = 'available' THEN
        IF NEW.delivery_attempt <> OLD.delivery_attempt
           OR NEW.checkpoint <> OLD.checkpoint THEN
            RAISE EXCEPTION 'available Agent queue work cannot change execution progress';
        END IF;
    ELSIF OLD.state = 'available' AND NEW.state = 'leased' THEN
        IF OLD.cancel_requested_at IS NOT NULL
           OR NEW.delivery_attempt <> OLD.delivery_attempt + 1
           OR NEW.checkpoint <> OLD.checkpoint
           OR NEW.heartbeat_at IS DISTINCT FROM STATEMENT_TIMESTAMP() THEN
            RAISE EXCEPTION 'Agent queue claims require the next delivery attempt';
        END IF;
    ELSIF OLD.state = 'available' AND NEW.state = 'finished' THEN
        IF NEW.delivery_attempt <> OLD.delivery_attempt
           OR NEW.checkpoint <> OLD.checkpoint
           OR (
               OLD.cancel_requested_at IS NOT NULL
               AND (
                   EXISTS (
                       SELECT 1 FROM agent_provider_attempts
                       WHERE run_id = OLD.run_id
                         AND tenant_id = OLD.tenant_id
                         AND status = 'running'
                   )
                   OR EXISTS (
                       SELECT 1 FROM agent_capability_calls
                       WHERE run_id = OLD.run_id
                         AND tenant_id = OLD.tenant_id
                         AND status = 'running'
                   )
                   OR EXISTS (
                       SELECT 1 FROM agent_execution_steps
                       WHERE run_id = OLD.run_id
                         AND tenant_id = OLD.tenant_id
                         AND status = 'running'
                   )
               )
           ) THEN
            RAISE EXCEPTION 'unclaimed Agent queue work can only finish without progress';
        END IF;
    ELSIF OLD.state = 'leased' AND NEW.state = 'leased' THEN
        IF OLD.lease_expires_at <= STATEMENT_TIMESTAMP()
           OR NEW.delivery_attempt <> OLD.delivery_attempt
           OR NEW.lease_token IS DISTINCT FROM OLD.lease_token
           OR NEW.leased_by IS DISTINCT FROM OLD.leased_by
           OR (
               NEW.heartbeat_at IS DISTINCT FROM OLD.heartbeat_at
               AND NEW.heartbeat_at IS DISTINCT FROM STATEMENT_TIMESTAMP()
           )
           OR NOT agent_valid_checkpoint_transition(OLD.checkpoint, NEW.checkpoint) THEN
            RAISE EXCEPTION 'stale Agent worker lease or invalid checkpoint transition';
        END IF;
    ELSIF OLD.state = 'leased' AND NEW.state = 'available' THEN
        IF OLD.cancel_requested_at IS NOT NULL
           OR OLD.lease_expires_at > STATEMENT_TIMESTAMP()
           OR OLD.checkpoint NOT IN (
               'queued', 'before_provider', 'provider_result_persisted',
               'capability_result_persisted', 'finalizing'
           )
           OR NEW.delivery_attempt <> OLD.delivery_attempt
           OR NEW.checkpoint <> OLD.checkpoint THEN
            RAISE EXCEPTION 'unsafe Agent queue work cannot be reclaimed';
        END IF;
    ELSIF OLD.state = 'leased' AND NEW.state = 'finished' THEN
        IF (
            OLD.lease_expires_at <= STATEMENT_TIMESTAMP()
            AND NOT (
                OLD.checkpoint IN ('provider_in_flight', 'capability_in_flight')
                OR (
                    OLD.delivery_attempt = 3
                    AND OLD.checkpoint IN (
                        'queued', 'before_provider', 'provider_result_persisted',
                        'capability_result_persisted', 'finalizing'
                    )
                )
            )
        )
           OR NEW.delivery_attempt <> OLD.delivery_attempt
           OR NOT agent_valid_checkpoint_transition(OLD.checkpoint, NEW.checkpoint)
           OR (
               OLD.cancel_requested_at IS NOT NULL
               AND (
                   EXISTS (
                       SELECT 1 FROM agent_provider_attempts
                       WHERE run_id = OLD.run_id
                         AND tenant_id = OLD.tenant_id
                         AND status = 'running'
                   )
                   OR EXISTS (
                       SELECT 1 FROM agent_capability_calls
                       WHERE run_id = OLD.run_id
                         AND tenant_id = OLD.tenant_id
                         AND status = 'running'
                   )
                   OR EXISTS (
                       SELECT 1 FROM agent_execution_steps
                       WHERE run_id = OLD.run_id
                         AND tenant_id = OLD.tenant_id
                         AND status = 'running'
                   )
               )
           ) THEN
            RAISE EXCEPTION 'invalid Agent queue finish';
        END IF;
    ELSE
        RAISE EXCEPTION 'invalid Agent queue lifecycle transition';
    END IF;

    IF NEW.checkpoint = 'finalizing'
       AND OLD.checkpoint <> 'finalizing'
       AND NOT EXISTS (
           SELECT 1
           FROM agent_execution_steps AS execution_step
           JOIN agent_execution_artifacts AS execution_artifact
             ON execution_artifact.step_id = execution_step.id
            AND execution_artifact.tenant_id = execution_step.tenant_id
            AND execution_artifact.run_id = execution_step.run_id
            AND execution_artifact.artifact_kind = 'final_response'
           WHERE execution_step.run_id = OLD.run_id
             AND execution_step.tenant_id = OLD.tenant_id
             AND execution_step.step_kind = 'finalize'
             AND execution_step.status = 'succeeded'
       ) THEN
        RAISE EXCEPTION 'finalizing Agent queue work requires durable finalization evidence';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_run_queue_protect_lifecycle ON agent_run_queue;
CREATE TRIGGER agent_run_queue_protect_lifecycle
    BEFORE UPDATE OR DELETE ON agent_run_queue
    FOR EACH ROW
    EXECUTE FUNCTION protect_agent_run_queue_lifecycle();

DROP TRIGGER IF EXISTS agent_run_events_reject_mutation ON agent_run_events;
CREATE TRIGGER agent_run_events_reject_mutation
    BEFORE UPDATE OR DELETE ON agent_run_events
    FOR EACH ROW
    EXECUTE FUNCTION reject_agent_append_only_mutation();

CREATE OR REPLACE FUNCTION validate_agent_run_event_insert()
RETURNS TRIGGER AS $$
DECLARE
    stored_run_status TEXT;
    stored_response_message_id UUID;
    matching_fact_count BIGINT;
    existing_event_count BIGINT;
BEGIN
    SELECT status, response_message_id
    INTO stored_run_status, stored_response_message_id
    FROM agent_runs
    WHERE id = NEW.run_id
      AND tenant_id = NEW.tenant_id;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Agent run events require a same-tenant run';
    END IF;

    IF (NEW.event_type = 'queued' AND stored_run_status <> 'queued')
       OR (NEW.event_type = 'started' AND stored_run_status <> 'running')
       OR (
           NEW.event_type = 'message_created'
           AND (
               stored_run_status <> 'completed'
               OR stored_response_message_id IS NULL
           )
       )
       OR (NEW.event_type = 'completed' AND stored_run_status <> 'completed')
       OR (NEW.event_type = 'failed' AND stored_run_status <> 'failed')
       OR (NEW.event_type = 'cancelled' AND stored_run_status <> 'cancelled')
       OR (NEW.event_type = 'interrupted' AND stored_run_status <> 'interrupted') THEN
        RAISE EXCEPTION 'Agent run event type does not match the current run state';
    END IF;

    IF NEW.event_type = 'provider_attempt_started' THEN
        SELECT COUNT(*)
        INTO matching_fact_count
        FROM agent_provider_attempts
        WHERE run_id = NEW.run_id
          AND tenant_id = NEW.tenant_id;

        SELECT COUNT(*)
        INTO existing_event_count
        FROM agent_run_events
        WHERE run_id = NEW.run_id
          AND tenant_id = NEW.tenant_id
          AND event_type = NEW.event_type;

        IF stored_run_status <> 'running'
           OR matching_fact_count <= existing_event_count THEN
            RAISE EXCEPTION 'Agent provider-start event requires an unreported attempt';
        END IF;
    ELSIF NEW.event_type = 'provider_attempt_finished' THEN
        SELECT COUNT(*)
        INTO matching_fact_count
        FROM agent_provider_attempts
        WHERE run_id = NEW.run_id
          AND tenant_id = NEW.tenant_id
          AND status <> 'running';

        SELECT COUNT(*)
        INTO existing_event_count
        FROM agent_run_events
        WHERE run_id = NEW.run_id
          AND tenant_id = NEW.tenant_id
          AND event_type = NEW.event_type;

        IF matching_fact_count <= existing_event_count THEN
            RAISE EXCEPTION 'Agent provider-finish event requires an unreported terminal attempt';
        END IF;
    ELSIF NEW.event_type = 'capability_call_started' THEN
        SELECT COUNT(*)
        INTO matching_fact_count
        FROM agent_capability_calls
        WHERE run_id = NEW.run_id
          AND tenant_id = NEW.tenant_id;

        SELECT COUNT(*)
        INTO existing_event_count
        FROM agent_run_events
        WHERE run_id = NEW.run_id
          AND tenant_id = NEW.tenant_id
          AND event_type = NEW.event_type;

        IF stored_run_status <> 'running'
           OR matching_fact_count <= existing_event_count THEN
            RAISE EXCEPTION 'Agent capability-start event requires an unreported call';
        END IF;
    ELSIF NEW.event_type = 'capability_call_finished' THEN
        SELECT COUNT(*)
        INTO matching_fact_count
        FROM agent_capability_calls
        WHERE run_id = NEW.run_id
          AND tenant_id = NEW.tenant_id
          AND status <> 'running';

        SELECT COUNT(*)
        INTO existing_event_count
        FROM agent_run_events
        WHERE run_id = NEW.run_id
          AND tenant_id = NEW.tenant_id
          AND event_type = NEW.event_type;

        IF matching_fact_count <= existing_event_count THEN
            RAISE EXCEPTION 'Agent capability-finish event requires an unreported terminal call';
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_run_events_validate_insert ON agent_run_events;
CREATE TRIGGER agent_run_events_validate_insert
    BEFORE INSERT ON agent_run_events
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_run_event_insert();

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
    stored_provider TEXT;
    stored_connection_status TEXT;
    stored_credential_version BIGINT;
    stored_connection_catalog_version BIGINT;
    stored_model_connection UUID;
    stored_provider_model_id TEXT;
    stored_model_credential_version BIGINT;
    stored_model_catalog_version BIGINT;
    stored_model_max_output_tokens BIGINT;
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

    SELECT connection_id, model_id
    INTO stored_target_connection, stored_target_model
    FROM ai_task_routes
    WHERE id = NEW.route_target_id
      AND tenant_id = NEW.tenant_id
      AND route_set_id = NEW.route_set_id
      AND deleted_at IS NULL;

    SELECT provider, status, credential_version, model_catalog_version
    INTO
        stored_provider,
        stored_connection_status,
        stored_credential_version,
        stored_connection_catalog_version
    FROM ai_provider_connections
    WHERE id = NEW.connection_id
      AND tenant_id = NEW.tenant_id
      AND deleted_at IS NULL;

    SELECT
        connection_id,
        provider_model_id,
        credential_version,
        catalog_version,
        max_output_tokens
    INTO
        stored_model_connection,
        stored_provider_model_id,
        stored_model_credential_version,
        stored_model_catalog_version,
        stored_model_max_output_tokens
    FROM ai_provider_models
    WHERE id = NEW.model_snapshot_id
      AND tenant_id = NEW.tenant_id
      AND deleted_at IS NULL;

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
        RAISE EXCEPTION 'Agent provider attempt identity must match its resolved route snapshot';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_provider_attempts_validate_insert ON agent_provider_attempts;
CREATE TRIGGER agent_provider_attempts_validate_insert
    BEFORE INSERT ON agent_provider_attempts
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_provider_attempt_insert();

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

DROP TRIGGER IF EXISTS agent_provider_attempts_protect_lifecycle ON agent_provider_attempts;
CREATE TRIGGER agent_provider_attempts_protect_lifecycle
    BEFORE UPDATE OR DELETE ON agent_provider_attempts
    FOR EACH ROW
    EXECUTE FUNCTION protect_agent_provider_attempt_lifecycle();

CREATE OR REPLACE FUNCTION validate_agent_capability_call_insert()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.status <> 'running'
       OR NEW.approval_state <> 'not_required'
       OR NEW.safe_failure_code IS NOT NULL
       OR NEW.duration_ms IS NOT NULL
       OR NEW.finished_at IS NOT NULL
       OR NEW.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'Agent capability calls must start in the running state';
    END IF;

    PERFORM 1
    FROM agent_runs AS agent_run
    JOIN agent_run_queue AS queue_row
      ON queue_row.run_id = agent_run.id
     AND queue_row.tenant_id = agent_run.tenant_id
     AND queue_row.state = 'leased'
     AND queue_row.cancel_requested_at IS NULL
    WHERE agent_run.id = NEW.run_id
      AND agent_run.tenant_id = NEW.tenant_id
      AND agent_run.status = 'running';

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Agent capability calls require an active uncancelled run lease';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_capability_calls_validate_insert ON agent_capability_calls;
CREATE TRIGGER agent_capability_calls_validate_insert
    BEFORE INSERT ON agent_capability_calls
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_capability_call_insert();

CREATE OR REPLACE FUNCTION protect_agent_capability_call_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Agent capability calls are retained, not deleted';
    END IF;

    IF OLD.status <> 'running' THEN
        RAISE EXCEPTION 'terminal Agent capability calls are immutable';
    END IF;

    IF NEW.id IS DISTINCT FROM OLD.id
       OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
       OR NEW.run_id IS DISTINCT FROM OLD.run_id
       OR NEW.call_sequence IS DISTINCT FROM OLD.call_sequence
       OR NEW.capability_key IS DISTINCT FROM OLD.capability_key
       OR NEW.capability_version IS DISTINCT FROM OLD.capability_version
       OR NEW.product_operation_key IS DISTINCT FROM OLD.product_operation_key
       OR NEW.owning_module_key IS DISTINCT FROM OLD.owning_module_key
       OR NEW.required_permission IS DISTINCT FROM OLD.required_permission
       OR NEW.input_fingerprint IS DISTINCT FROM OLD.input_fingerprint
       OR NEW.scope_kind IS DISTINCT FROM OLD.scope_kind
       OR NEW.resource_references IS DISTINCT FROM OLD.resource_references
       OR NEW.approval_state IS DISTINCT FROM OLD.approval_state
       OR NEW.started_at IS DISTINCT FROM OLD.started_at
       OR NEW.created_at IS DISTINCT FROM OLD.created_at
       OR NEW.deleted_at IS NOT NULL
       OR NEW.status NOT IN ('succeeded', 'failed', 'denied', 'cancelled', 'interrupted')
       OR NEW.updated_at <= OLD.updated_at THEN
        RAISE EXCEPTION 'invalid Agent capability call lifecycle transition';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_capability_calls_protect_lifecycle ON agent_capability_calls;
CREATE TRIGGER agent_capability_calls_protect_lifecycle
    BEFORE UPDATE OR DELETE ON agent_capability_calls
    FOR EACH ROW
    EXECUTE FUNCTION protect_agent_capability_call_lifecycle();

CREATE OR REPLACE FUNCTION validate_agent_execution_step_insert()
RETURNS TRIGGER AS $$
DECLARE
    stored_run_status TEXT;
    stored_child_status TEXT;
    stored_attempt_turn SMALLINT;
BEGIN
    IF NEW.status <> 'running'
       OR NEW.safe_failure_code IS NOT NULL
       OR NEW.finished_at IS NOT NULL
       OR NEW.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'Agent execution steps must start in the running state';
    END IF;

    SELECT status
    INTO stored_run_status
    FROM agent_runs
    WHERE id = NEW.run_id
      AND tenant_id = NEW.tenant_id;

    IF stored_run_status <> 'running' THEN
        RAISE EXCEPTION 'Agent execution steps require a running same-tenant run';
    END IF;

    IF NEW.step_kind = 'provider_attempt' THEN
        SELECT status, turn_index
        INTO stored_child_status, stored_attempt_turn
        FROM agent_provider_attempts
        WHERE id = NEW.provider_attempt_id
          AND run_id = NEW.run_id
          AND tenant_id = NEW.tenant_id;

        IF stored_child_status <> 'running'
           OR stored_attempt_turn <> NEW.turn_index THEN
            RAISE EXCEPTION 'Agent provider execution step must match its running attempt';
        END IF;
    ELSIF NEW.step_kind = 'capability_call' THEN
        SELECT status
        INTO stored_child_status
        FROM agent_capability_calls
        WHERE id = NEW.capability_call_id
          AND run_id = NEW.run_id
          AND tenant_id = NEW.tenant_id;

        IF stored_child_status <> 'running' THEN
            RAISE EXCEPTION 'Agent capability execution step must match its running call';
        END IF;
    END IF;

    IF NEW.step_kind <> 'finalize'
       AND NOT EXISTS (
           SELECT 1
           FROM agent_run_queue
           WHERE run_id = NEW.run_id
             AND tenant_id = NEW.tenant_id
             AND state = 'leased'
             AND cancel_requested_at IS NULL
       ) THEN
        RAISE EXCEPTION 'Agent execution steps require an active uncancelled run lease';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_execution_steps_validate_insert ON agent_execution_steps;
CREATE TRIGGER agent_execution_steps_validate_insert
    BEFORE INSERT ON agent_execution_steps
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_execution_step_insert();

CREATE OR REPLACE FUNCTION validate_agent_execution_artifact_insert()
RETURNS TRIGGER AS $$
DECLARE
    stored_step_kind TEXT;
    stored_step_status TEXT;
    existing_artifact_count INTEGER;
    existing_plaintext_length BIGINT;
BEGIN
    PERFORM 1
    FROM agent_runs
    WHERE id = NEW.run_id
      AND tenant_id = NEW.tenant_id
    FOR UPDATE;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Agent execution artifacts require a same-tenant run';
    END IF;

    SELECT step_kind, status
    INTO stored_step_kind, stored_step_status
    FROM agent_execution_steps
    WHERE id = NEW.step_id
      AND run_id = NEW.run_id
      AND tenant_id = NEW.tenant_id;

    IF stored_step_status <> 'running'
       OR (stored_step_kind = 'provider_attempt' AND NEW.artifact_kind <> 'provider_result')
       OR (stored_step_kind = 'capability_call' AND NEW.artifact_kind <> 'capability_result')
       OR (stored_step_kind = 'finalize' AND NEW.artifact_kind <> 'final_response') THEN
        RAISE EXCEPTION 'Agent execution artifact must match its running step';
    END IF;

    SELECT COUNT(*), COALESCE(SUM(plaintext_length), 0)
    INTO existing_artifact_count, existing_plaintext_length
    FROM agent_execution_artifacts
    WHERE run_id = NEW.run_id
      AND tenant_id = NEW.tenant_id;

    IF NEW.artifact_sequence <> existing_artifact_count + 1
       OR existing_artifact_count >= 33
       OR existing_plaintext_length + NEW.plaintext_length > 2162688 THEN
        RAISE EXCEPTION 'Agent execution artifact exceeds the bounded run envelope';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_execution_artifacts_validate_insert
    ON agent_execution_artifacts;
CREATE TRIGGER agent_execution_artifacts_validate_insert
    BEFORE INSERT ON agent_execution_artifacts
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_execution_artifact_insert();

CREATE OR REPLACE FUNCTION protect_agent_execution_step_lifecycle()
RETURNS TRIGGER AS $$
DECLARE
    stored_child_status TEXT;
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Agent execution steps are retained, not deleted';
    END IF;

    IF OLD.status <> 'running' THEN
        RAISE EXCEPTION 'terminal Agent execution steps are immutable';
    END IF;

    IF NEW.id IS DISTINCT FROM OLD.id
       OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
       OR NEW.run_id IS DISTINCT FROM OLD.run_id
       OR NEW.step_index IS DISTINCT FROM OLD.step_index
       OR NEW.turn_index IS DISTINCT FROM OLD.turn_index
       OR NEW.step_kind IS DISTINCT FROM OLD.step_kind
       OR NEW.provider_attempt_id IS DISTINCT FROM OLD.provider_attempt_id
       OR NEW.capability_call_id IS DISTINCT FROM OLD.capability_call_id
       OR NEW.input_fingerprint IS DISTINCT FROM OLD.input_fingerprint
       OR NEW.started_at IS DISTINCT FROM OLD.started_at
       OR NEW.created_at IS DISTINCT FROM OLD.created_at
       OR NEW.deleted_at IS NOT NULL
       OR NEW.status NOT IN ('succeeded', 'failed', 'cancelled', 'interrupted')
       OR NEW.updated_at <= OLD.updated_at THEN
        RAISE EXCEPTION 'invalid Agent execution step lifecycle transition';
    END IF;

    IF NEW.status = 'succeeded'
       OR (OLD.step_kind = 'capability_call' AND NEW.status = 'failed') THEN
        IF NOT EXISTS (
            SELECT 1
            FROM agent_execution_artifacts
            WHERE step_id = OLD.id
              AND run_id = OLD.run_id
              AND tenant_id = OLD.tenant_id
        ) THEN
            RAISE EXCEPTION 'recoverable Agent execution steps require one encrypted artifact';
        END IF;
    ELSIF EXISTS (
        SELECT 1
        FROM agent_execution_artifacts
        WHERE step_id = OLD.id
          AND run_id = OLD.run_id
          AND tenant_id = OLD.tenant_id
    ) THEN
        RAISE EXCEPTION 'non-recoverable Agent execution steps cannot retain result artifacts';
    END IF;

    IF OLD.step_kind = 'provider_attempt' THEN
        SELECT status
        INTO stored_child_status
        FROM agent_provider_attempts
        WHERE id = OLD.provider_attempt_id
          AND run_id = OLD.run_id
          AND tenant_id = OLD.tenant_id;

        IF stored_child_status IS DISTINCT FROM NEW.status THEN
            RAISE EXCEPTION 'Agent provider execution step must match its terminal attempt';
        END IF;
    ELSIF OLD.step_kind = 'capability_call' THEN
        SELECT status
        INTO stored_child_status
        FROM agent_capability_calls
        WHERE id = OLD.capability_call_id
          AND run_id = OLD.run_id
          AND tenant_id = OLD.tenant_id;

        IF NOT (
            stored_child_status = NEW.status
            OR (stored_child_status = 'denied' AND NEW.status = 'failed')
        ) THEN
            RAISE EXCEPTION 'Agent capability execution step must match its terminal call';
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_execution_steps_protect_lifecycle ON agent_execution_steps;
CREATE TRIGGER agent_execution_steps_protect_lifecycle
    BEFORE UPDATE OR DELETE ON agent_execution_steps
    FOR EACH ROW
    EXECUTE FUNCTION protect_agent_execution_step_lifecycle();

DROP TRIGGER IF EXISTS agent_execution_artifacts_reject_mutation
    ON agent_execution_artifacts;
CREATE TRIGGER agent_execution_artifacts_reject_mutation
    BEFORE UPDATE OR DELETE ON agent_execution_artifacts
    FOR EACH ROW
    EXECUTE FUNCTION reject_agent_append_only_mutation();

CREATE OR REPLACE FUNCTION validate_agent_idempotency_result()
RETURNS TRIGGER AS $$
DECLARE
    result_owner UUID;
    result_thread UUID;
    result_requester UUID;
BEGIN
    IF NEW.result_kind = 'thread' THEN
        SELECT owner_user_id
        INTO result_owner
        FROM agent_threads
        WHERE id = NEW.result_id
          AND tenant_id = NEW.tenant_id;

        IF NOT FOUND OR result_owner <> NEW.user_id THEN
            RAISE EXCEPTION 'Agent Session idempotency result must belong to its requester';
        END IF;
    ELSE
        SELECT thread_id, requested_by
        INTO result_thread, result_requester
        FROM agent_runs
        WHERE id = NEW.result_id
          AND tenant_id = NEW.tenant_id;

        IF NOT FOUND
           OR result_thread <> NEW.scope_id
           OR result_requester <> NEW.user_id THEN
            RAISE EXCEPTION 'Agent run idempotency result must match its requester and Session';
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_request_idempotency_validate_insert ON agent_request_idempotency;
CREATE TRIGGER agent_request_idempotency_validate_insert
    BEFORE INSERT ON agent_request_idempotency
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_idempotency_result();

DROP TRIGGER IF EXISTS agent_request_idempotency_reject_mutation ON agent_request_idempotency;
CREATE TRIGGER agent_request_idempotency_reject_mutation
    BEFORE UPDATE OR DELETE ON agent_request_idempotency
    FOR EACH ROW
    EXECUTE FUNCTION reject_agent_append_only_mutation();

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'actor_audit_events_id_tenant_unique'
          AND conrelid = 'actor_audit_events'::REGCLASS
    ) THEN
        ALTER TABLE actor_audit_events
            ADD CONSTRAINT actor_audit_events_id_tenant_unique UNIQUE (id, tenant_id);
    END IF;

    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'actor_audit_events_agent_run_tenant_fk'
          AND conrelid = 'actor_audit_events'::REGCLASS
    ) THEN
        ALTER TABLE actor_audit_events
            ADD CONSTRAINT actor_audit_events_agent_run_tenant_fk
            FOREIGN KEY (agent_run_id, tenant_id)
            REFERENCES agent_runs(id, tenant_id) ON DELETE RESTRICT;
    END IF;
END;
$$;

CREATE OR REPLACE FUNCTION prevent_actor_audit_event_update()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'actor audit events are append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS immutable_actor_audit_events ON actor_audit_events;
CREATE TRIGGER immutable_actor_audit_events
    BEFORE UPDATE OR DELETE ON actor_audit_events
    FOR EACH ROW
    EXECUTE FUNCTION prevent_actor_audit_event_update();

DROP TRIGGER IF EXISTS agent_threads_reject_truncate ON agent_threads;
CREATE TRIGGER agent_threads_reject_truncate
    BEFORE TRUNCATE ON agent_threads
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_agent_append_only_mutation();

DROP TRIGGER IF EXISTS agent_thread_members_reject_truncate ON agent_thread_members;
CREATE TRIGGER agent_thread_members_reject_truncate
    BEFORE TRUNCATE ON agent_thread_members
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_agent_append_only_mutation();

DROP TRIGGER IF EXISTS agent_messages_reject_truncate ON agent_messages;
CREATE TRIGGER agent_messages_reject_truncate
    BEFORE TRUNCATE ON agent_messages
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_agent_append_only_mutation();

DROP TRIGGER IF EXISTS agent_runs_reject_truncate ON agent_runs;
CREATE TRIGGER agent_runs_reject_truncate
    BEFORE TRUNCATE ON agent_runs
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_agent_append_only_mutation();

DROP TRIGGER IF EXISTS agent_run_queue_reject_truncate ON agent_run_queue;
CREATE TRIGGER agent_run_queue_reject_truncate
    BEFORE TRUNCATE ON agent_run_queue
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_agent_append_only_mutation();

DROP TRIGGER IF EXISTS agent_run_events_reject_truncate ON agent_run_events;
CREATE TRIGGER agent_run_events_reject_truncate
    BEFORE TRUNCATE ON agent_run_events
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_agent_append_only_mutation();

DROP TRIGGER IF EXISTS agent_provider_attempts_reject_truncate ON agent_provider_attempts;
CREATE TRIGGER agent_provider_attempts_reject_truncate
    BEFORE TRUNCATE ON agent_provider_attempts
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_agent_append_only_mutation();

DROP TRIGGER IF EXISTS agent_capability_calls_reject_truncate ON agent_capability_calls;
CREATE TRIGGER agent_capability_calls_reject_truncate
    BEFORE TRUNCATE ON agent_capability_calls
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_agent_append_only_mutation();

DROP TRIGGER IF EXISTS agent_execution_steps_reject_truncate ON agent_execution_steps;
CREATE TRIGGER agent_execution_steps_reject_truncate
    BEFORE TRUNCATE ON agent_execution_steps
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_agent_append_only_mutation();

DROP TRIGGER IF EXISTS agent_execution_artifacts_reject_truncate
    ON agent_execution_artifacts;
CREATE TRIGGER agent_execution_artifacts_reject_truncate
    BEFORE TRUNCATE ON agent_execution_artifacts
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_agent_append_only_mutation();

DROP TRIGGER IF EXISTS agent_request_idempotency_reject_truncate
    ON agent_request_idempotency;
CREATE TRIGGER agent_request_idempotency_reject_truncate
    BEFORE TRUNCATE ON agent_request_idempotency
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_agent_append_only_mutation();

DROP TRIGGER IF EXISTS actor_audit_events_reject_truncate ON actor_audit_events;
CREATE TRIGGER actor_audit_events_reject_truncate
    BEFORE TRUNCATE ON actor_audit_events
    FOR EACH STATEMENT
    EXECUTE FUNCTION prevent_actor_audit_event_update();
