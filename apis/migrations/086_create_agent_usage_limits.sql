-- Owns tenant-scoped Agent usage evidence and transactional hard-limit preparation.
-- Usage facts and measures are append-only; counters are enforcement projections, never reports.
-- Money remains an integer amount plus ISO currency and exponent, with no implicit conversion.

CREATE OR REPLACE FUNCTION agent_usage_valid_key(value TEXT, maximum_length INTEGER)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN value IS NOT NULL
        AND CHAR_LENGTH(value) BETWEEN 1 AND maximum_length
        AND value = LOWER(BTRIM(value))
        AND value ~ '^[a-z][a-z0-9_.-]*$';
END;
$$ LANGUAGE plpgsql IMMUTABLE;

CREATE OR REPLACE FUNCTION agent_usage_valid_role_keys(role_keys TEXT[])
RETURNS BOOLEAN AS $$
BEGIN
    RETURN role_keys IS NOT NULL
        AND CARDINALITY(role_keys) BETWEEN 1 AND 32
        AND NOT EXISTS (
            SELECT 1
            FROM UNNEST(role_keys) AS role_key(value)
            WHERE NOT agent_usage_valid_key(role_key.value, 160)
        )
        AND role_keys = ARRAY(
            SELECT DISTINCT role_key.value
            FROM UNNEST(role_keys) AS role_key(value)
            ORDER BY role_key.value
        );
END;
$$ LANGUAGE plpgsql IMMUTABLE;

CREATE OR REPLACE FUNCTION agent_usage_meter_unit(meter_key TEXT)
RETURNS TEXT AS $$
BEGIN
    RETURN CASE meter_key
        WHEN 'agent.runs' THEN 'run'
        WHEN 'agent.provider_attempts' THEN 'attempt'
        WHEN 'agent.capability_calls' THEN 'call'
        WHEN 'agent.input_tokens' THEN 'token'
        WHEN 'agent.output_tokens' THEN 'token'
        WHEN 'agent.cached_input_tokens' THEN 'token'
        WHEN 'agent.reasoning_tokens' THEN 'token'
        WHEN 'agent.provider_reported_cost' THEN 'money'
        WHEN 'agent.estimated_cost' THEN 'money'
        ELSE NULL
    END;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

CREATE OR REPLACE FUNCTION agent_usage_meter_supports_hard_limit(meter_key TEXT)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN meter_key IN (
        'agent.runs',
        'agent.provider_attempts',
        'agent.capability_calls',
        'agent.input_tokens',
        'agent.output_tokens',
        'agent.estimated_cost'
    );
END;
$$ LANGUAGE plpgsql IMMUTABLE;

CREATE OR REPLACE FUNCTION agent_usage_stage_supports_meter(
    stage_kind TEXT,
    meter_key TEXT
)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN (stage_kind = 'run' AND meter_key = 'agent.runs')
        OR (
            stage_kind = 'provider_attempt'
            AND meter_key IN (
                'agent.provider_attempts',
                'agent.input_tokens',
                'agent.output_tokens',
                'agent.estimated_cost'
            )
        )
        OR (
            stage_kind = 'capability_call'
            AND meter_key = 'agent.capability_calls'
        );
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- The existing entitlement tables remain the only mutable commercial quota truth.
-- These composite identities let the Agent grouping ledger reference that truth
-- without copying signed counters into Agent-owned buckets.
CREATE UNIQUE INDEX IF NOT EXISTS entitlement_meter_buckets_id_tenant_unique
    ON entitlement_meter_buckets (id, tenant_id);

CREATE UNIQUE INDEX IF NOT EXISTS entitlement_usage_reservations_id_tenant_bucket_unique
    ON entitlement_usage_reservations (id, tenant_id, bucket_id);

CREATE TABLE IF NOT EXISTS agent_limit_rules (
    id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    scope_kind TEXT NOT NULL CHECK (scope_kind IN (
        'campus', 'person', 'role', 'origin_module', 'capability_module',
        'capability', 'provider', 'model'
    )),
    person_user_id UUID,
    role_key TEXT CHECK (
        role_key IS NULL OR agent_usage_valid_key(role_key, 160)
    ),
    origin_module_key TEXT CHECK (
        origin_module_key IS NULL OR agent_usage_valid_key(origin_module_key, 160)
    ),
    capability_module_key TEXT CHECK (
        capability_module_key IS NULL
        OR agent_usage_valid_key(capability_module_key, 160)
    ),
    capability_key TEXT CHECK (
        capability_key IS NULL OR agent_usage_valid_key(capability_key, 200)
    ),
    provider_key TEXT CHECK (
        provider_key IS NULL OR provider_key IN ('openai', 'anthropic', 'openrouter')
    ),
    provider_model_id TEXT CHECK (
        provider_model_id IS NULL
        OR CHAR_LENGTH(BTRIM(provider_model_id)) BETWEEN 1 AND 240
    ),
    meter_key TEXT NOT NULL CHECK (agent_usage_meter_unit(meter_key) IS NOT NULL),
    currency_code TEXT CHECK (
        currency_code IS NULL OR currency_code ~ '^[A-Z]{3}$'
    ),
    currency_exponent SMALLINT CHECK (
        currency_exponent IS NULL OR currency_exponent BETWEEN 0 AND 9
    ),
    period TEXT NOT NULL CHECK (period IN ('none', 'day', 'month', 'year')),
    limit_value BIGINT NOT NULL CHECK (
        limit_value BETWEEN 0 AND 9007199254740991
    ),
    enforcement TEXT NOT NULL CHECK (enforcement IN ('report', 'hard')),
    provenance_kind TEXT NOT NULL CHECK (
        (enforcement = 'report' AND provenance_kind = 'campus_reporting')
        OR (enforcement = 'hard' AND provenance_kind = 'campus_tightening')
    ),
    effective_from TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    configured_by UUID NOT NULL,
    change_reason TEXT NOT NULL CHECK (
        CHAR_LENGTH(BTRIM(change_reason)) BETWEEN 3 AND 500
    ),
    version BIGINT NOT NULL DEFAULT 1 CHECK (
        version BETWEEN 1 AND 9007199254740991
    ),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_limit_rules_id_tenant_unique UNIQUE (id, tenant_id),
    CONSTRAINT agent_limit_rules_person_tenant_fk
        FOREIGN KEY (person_user_id, tenant_id)
        REFERENCES users(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_limit_rules_configured_by_tenant_fk
        FOREIGN KEY (configured_by, tenant_id)
        REFERENCES users(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_limit_rules_scope_shape_check CHECK (
        (
            scope_kind = 'campus'
            AND person_user_id IS NULL
            AND role_key IS NULL
            AND origin_module_key IS NULL
            AND capability_module_key IS NULL
            AND capability_key IS NULL
            AND provider_key IS NULL
            AND provider_model_id IS NULL
        )
        OR (
            scope_kind = 'person'
            AND person_user_id IS NOT NULL
            AND role_key IS NULL
            AND origin_module_key IS NULL
            AND capability_module_key IS NULL
            AND capability_key IS NULL
            AND provider_key IS NULL
            AND provider_model_id IS NULL
        )
        OR (
            scope_kind = 'role'
            AND person_user_id IS NULL
            AND role_key IS NOT NULL
            AND origin_module_key IS NULL
            AND capability_module_key IS NULL
            AND capability_key IS NULL
            AND provider_key IS NULL
            AND provider_model_id IS NULL
        )
        OR (
            scope_kind = 'origin_module'
            AND person_user_id IS NULL
            AND role_key IS NULL
            AND origin_module_key IS NOT NULL
            AND capability_module_key IS NULL
            AND capability_key IS NULL
            AND provider_key IS NULL
            AND provider_model_id IS NULL
        )
        OR (
            scope_kind = 'capability_module'
            AND person_user_id IS NULL
            AND role_key IS NULL
            AND origin_module_key IS NULL
            AND capability_module_key IS NOT NULL
            AND capability_key IS NULL
            AND provider_key IS NULL
            AND provider_model_id IS NULL
        )
        OR (
            scope_kind = 'capability'
            AND person_user_id IS NULL
            AND role_key IS NULL
            AND origin_module_key IS NULL
            AND capability_module_key IS NULL
            AND capability_key IS NOT NULL
            AND provider_key IS NULL
            AND provider_model_id IS NULL
        )
        OR (
            scope_kind = 'provider'
            AND person_user_id IS NULL
            AND role_key IS NULL
            AND origin_module_key IS NULL
            AND capability_module_key IS NULL
            AND capability_key IS NULL
            AND provider_key IS NOT NULL
            AND provider_model_id IS NULL
        )
        OR (
            scope_kind = 'model'
            AND person_user_id IS NULL
            AND role_key IS NULL
            AND origin_module_key IS NULL
            AND capability_module_key IS NULL
            AND capability_key IS NULL
            AND provider_key IS NOT NULL
            AND provider_model_id IS NOT NULL
        )
    ),
    CONSTRAINT agent_limit_rules_money_shape_check CHECK (
        (
            agent_usage_meter_unit(meter_key) = 'money'
            AND currency_code IS NOT NULL
            AND currency_exponent IS NOT NULL
        )
        OR (
            agent_usage_meter_unit(meter_key) <> 'money'
            AND currency_code IS NULL
            AND currency_exponent IS NULL
        )
    ),
    CONSTRAINT agent_limit_rules_hard_meter_check CHECK (
        enforcement = 'report'
        OR agent_usage_meter_supports_hard_limit(meter_key)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS agent_limit_rules_active_identity_unique
    ON agent_limit_rules (
        tenant_id,
        scope_kind,
        person_user_id,
        role_key,
        origin_module_key,
        capability_module_key,
        capability_key,
        provider_key,
        provider_model_id,
        meter_key,
        currency_code,
        currency_exponent,
        period
    ) NULLS NOT DISTINCT
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS agent_limit_rules_tenant_scope_idx
    ON agent_limit_rules (tenant_id, scope_kind, meter_key, updated_at DESC)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS agent_limit_buckets (
    id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    campus_rule_id UUID NOT NULL,
    meter_key TEXT NOT NULL CHECK (agent_usage_meter_unit(meter_key) IS NOT NULL),
    currency_code TEXT CHECK (
        currency_code IS NULL OR currency_code ~ '^[A-Z]{3}$'
    ),
    currency_exponent SMALLINT CHECK (
        currency_exponent IS NULL OR currency_exponent BETWEEN 0 AND 9
    ),
    period TEXT NOT NULL CHECK (period IN ('none', 'day', 'month', 'year')),
    period_start TIMESTAMPTZ NOT NULL,
    period_end TIMESTAMPTZ,
    committed_value BIGINT NOT NULL DEFAULT 0 CHECK (
        committed_value BETWEEN 0 AND 9007199254740991
    ),
    reserved_value BIGINT NOT NULL DEFAULT 0 CHECK (
        reserved_value BETWEEN 0 AND 9007199254740991
    ),
    deleted_at TIMESTAMPTZ CHECK (deleted_at IS NULL),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_limit_buckets_id_tenant_unique UNIQUE (id, tenant_id),
    CONSTRAINT agent_limit_buckets_rule_tenant_fk
        FOREIGN KEY (campus_rule_id, tenant_id)
        REFERENCES agent_limit_rules(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_limit_buckets_period_shape_check CHECK (
        (period = 'none' AND period_end IS NULL)
        OR (period <> 'none' AND period_end IS NOT NULL AND period_end > period_start)
    ),
    CONSTRAINT agent_limit_buckets_money_shape_check CHECK (
        (
            agent_usage_meter_unit(meter_key) = 'money'
            AND currency_code IS NOT NULL
            AND currency_exponent IS NOT NULL
        )
        OR (
            agent_usage_meter_unit(meter_key) <> 'money'
            AND currency_code IS NULL
            AND currency_exponent IS NULL
        )
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS agent_limit_buckets_rule_period_unique
    ON agent_limit_buckets (tenant_id, campus_rule_id, period_start)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS agent_limit_buckets_current_idx
    ON agent_limit_buckets (tenant_id, meter_key, period_start DESC);

CREATE TABLE IF NOT EXISTS agent_limit_reservations (
    id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    run_id UUID NOT NULL,
    provider_attempt_id UUID,
    capability_call_id UUID,
    actor_user_id UUID NOT NULL,
    role_keys TEXT[] NOT NULL CHECK (agent_usage_valid_role_keys(role_keys)),
    origin_module_key TEXT NOT NULL CHECK (
        agent_usage_valid_key(origin_module_key, 160)
    ),
    capability_module_key TEXT CHECK (
        capability_module_key IS NULL
        OR agent_usage_valid_key(capability_module_key, 160)
    ),
    capability_key TEXT CHECK (
        capability_key IS NULL OR agent_usage_valid_key(capability_key, 200)
    ),
    provider_key TEXT CHECK (
        provider_key IS NULL OR provider_key IN ('openai', 'anthropic', 'openrouter')
    ),
    provider_model_id TEXT CHECK (
        provider_model_id IS NULL
        OR CHAR_LENGTH(BTRIM(provider_model_id)) BETWEEN 1 AND 240
    ),
    stage_kind TEXT NOT NULL CHECK (
        stage_kind IN ('run', 'provider_attempt', 'capability_call')
    ),
    stage_sequence SMALLINT NOT NULL,
    idempotency_key TEXT NOT NULL CHECK (
        CHAR_LENGTH(BTRIM(idempotency_key)) BETWEEN 8 AND 200
        AND idempotency_key ~ '^[A-Za-z0-9_.:-]+$'
    ),
    request_fingerprint BYTEA NOT NULL CHECK (
        OCTET_LENGTH(request_fingerprint) = 32
    ),
    status TEXT NOT NULL DEFAULT 'preparing' CHECK (status IN (
        'preparing', 'not_limited', 'reserved', 'committed',
        'released', 'expired', 'denied'
    )),
    expires_at TIMESTAMPTZ,
    committed_at TIMESTAMPTZ,
    released_at TIMESTAMPTZ,
    denied_at TIMESTAMPTZ,
    claimed_at TIMESTAMPTZ,
    claimed_by_worker_id TEXT CHECK (
        claimed_by_worker_id IS NULL
        OR (
            CHAR_LENGTH(BTRIM(claimed_by_worker_id)) BETWEEN 1 AND 160
            AND claimed_by_worker_id !~ '[[:cntrl:]]'
        )
    ),
    claim_fence_version BIGINT CHECK (
        claim_fence_version IS NULL
        OR claim_fence_version BETWEEN 1 AND 9007199254740991
    ),
    deleted_at TIMESTAMPTZ CHECK (deleted_at IS NULL),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_limit_reservations_id_tenant_unique UNIQUE (id, tenant_id),
    CONSTRAINT agent_limit_reservations_id_tenant_run_unique
        UNIQUE (id, tenant_id, run_id),
    CONSTRAINT agent_limit_reservations_run_tenant_fk
        FOREIGN KEY (run_id, tenant_id)
        REFERENCES agent_runs(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_limit_reservations_provider_attempt_tenant_run_fk
        FOREIGN KEY (provider_attempt_id, tenant_id, run_id)
        REFERENCES agent_provider_attempts(id, tenant_id, run_id) ON DELETE RESTRICT,
    CONSTRAINT agent_limit_reservations_capability_call_tenant_run_fk
        FOREIGN KEY (capability_call_id, tenant_id, run_id)
        REFERENCES agent_capability_calls(id, tenant_id, run_id) ON DELETE RESTRICT,
    CONSTRAINT agent_limit_reservations_actor_tenant_fk
        FOREIGN KEY (actor_user_id, tenant_id)
        REFERENCES users(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_limit_reservations_stage_shape_check CHECK (
        (
            stage_kind = 'run'
            AND stage_sequence = 0
            AND provider_attempt_id IS NULL
            AND capability_call_id IS NULL
            AND capability_module_key IS NULL
            AND capability_key IS NULL
            AND provider_key IS NULL
            AND provider_model_id IS NULL
        )
        OR (
            stage_kind = 'provider_attempt'
            AND stage_sequence BETWEEN 1 AND 48
            AND provider_attempt_id IS NOT NULL
            AND capability_call_id IS NULL
            AND capability_module_key IS NULL
            AND capability_key IS NULL
            AND provider_key IS NOT NULL
            AND provider_model_id IS NOT NULL
        )
        OR (
            stage_kind = 'capability_call'
            AND stage_sequence BETWEEN 1 AND 16
            AND provider_attempt_id IS NULL
            AND capability_call_id IS NOT NULL
            AND capability_module_key IS NOT NULL
            AND capability_key IS NOT NULL
            AND provider_key IS NULL
            AND provider_model_id IS NULL
        )
    ),
    CONSTRAINT agent_limit_reservations_claim_shape_check CHECK (
        (
            claimed_at IS NULL
            AND claimed_by_worker_id IS NULL
            AND claim_fence_version IS NULL
        )
        OR (
            claimed_at IS NOT NULL
            AND claimed_at >= created_at
            AND claimed_by_worker_id IS NOT NULL
            AND claim_fence_version IS NOT NULL
        )
    ),
    CONSTRAINT agent_limit_reservations_state_shape_check CHECK (
        (
            status = 'preparing'
            AND expires_at IS NULL
            AND committed_at IS NULL
            AND released_at IS NULL
            AND denied_at IS NULL
        )
        OR (
            status = 'not_limited'
            AND expires_at IS NULL
            AND committed_at IS NULL
            AND released_at IS NULL
            AND denied_at IS NULL
        )
        OR (
            status = 'reserved'
            AND expires_at IS NOT NULL
            AND expires_at > created_at
            AND committed_at IS NULL
            AND released_at IS NULL
            AND denied_at IS NULL
        )
        OR (
            status = 'committed'
            AND expires_at IS NOT NULL
            AND committed_at IS NOT NULL
            AND committed_at >= created_at
            AND released_at IS NULL
            AND denied_at IS NULL
        )
        OR (
            status IN ('released', 'expired')
            AND expires_at IS NOT NULL
            AND committed_at IS NULL
            AND released_at IS NOT NULL
            AND released_at >= created_at
            AND denied_at IS NULL
        )
        OR (
            status = 'denied'
            AND expires_at IS NULL
            AND committed_at IS NULL
            AND released_at IS NULL
            AND denied_at IS NOT NULL
            AND denied_at >= created_at
        )
    ),
    UNIQUE (tenant_id, run_id, stage_kind, stage_sequence),
    UNIQUE (tenant_id, idempotency_key),
    UNIQUE (provider_attempt_id),
    UNIQUE (capability_call_id)
);

CREATE INDEX IF NOT EXISTS agent_limit_reservations_active_idx
    ON agent_limit_reservations (tenant_id, expires_at, run_id)
    WHERE status = 'reserved';

CREATE TABLE IF NOT EXISTS agent_limit_reservation_items (
    id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    reservation_id UUID NOT NULL,
    run_id UUID NOT NULL,
    item_sequence SMALLINT NOT NULL CHECK (item_sequence BETWEEN 1 AND 64),
    bucket_id UUID,
    entitlement_bucket_id UUID,
    entitlement_reservation_id UUID,
    definition_kind TEXT NOT NULL CHECK (
        definition_kind IN ('local_rule', 'signed_entitlement')
    ),
    campus_rule_id UUID,
    source_lease_id UUID,
    entitlement_limit_key TEXT CHECK (
        entitlement_limit_key IS NULL
        OR agent_usage_valid_key(entitlement_limit_key, 200)
    ),
    definition_version BIGINT CHECK (
        definition_version IS NULL
        OR definition_version BETWEEN 1 AND 9007199254740991
    ),
    scope_kind TEXT NOT NULL CHECK (scope_kind IN (
        'campus', 'person', 'role', 'origin_module', 'capability_module',
        'capability', 'provider', 'model'
    )),
    scope_value TEXT NOT NULL CHECK (
        CHAR_LENGTH(BTRIM(scope_value)) BETWEEN 1 AND 320
        AND scope_value !~ '[[:cntrl:]]'
    ),
    meter_key TEXT NOT NULL CHECK (agent_usage_meter_unit(meter_key) IS NOT NULL),
    unit TEXT NOT NULL CHECK (unit = agent_usage_meter_unit(meter_key)),
    currency_code TEXT CHECK (
        currency_code IS NULL OR currency_code ~ '^[A-Z]{3}$'
    ),
    currency_exponent SMALLINT CHECK (
        currency_exponent IS NULL OR currency_exponent BETWEEN 0 AND 9
    ),
    pricing_version TEXT CHECK (
        pricing_version IS NULL
        OR CHAR_LENGTH(BTRIM(pricing_version)) BETWEEN 1 AND 100
    ),
    period TEXT NOT NULL CHECK (period IN ('none', 'day', 'month', 'year')),
    period_start TIMESTAMPTZ NOT NULL,
    period_end TIMESTAMPTZ,
    limit_value BIGINT NOT NULL CHECK (
        limit_value BETWEEN 0 AND 9007199254740991
    ),
    committed_before BIGINT NOT NULL CHECK (
        committed_before BETWEEN 0 AND 9007199254740991
    ),
    reserved_before BIGINT NOT NULL CHECK (
        reserved_before BETWEEN 0 AND 9007199254740991
    ),
    requested_amount BIGINT NOT NULL CHECK (
        requested_amount BETWEEN 1 AND 9007199254740991
    ),
    reserved_amount BIGINT NOT NULL CHECK (
        reserved_amount BETWEEN 0 AND 9007199254740991
    ),
    decision TEXT NOT NULL CHECK (decision IN ('allowed', 'denied')),
    deleted_at TIMESTAMPTZ CHECK (deleted_at IS NULL),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_limit_reservation_items_id_tenant_unique UNIQUE (id, tenant_id),
    CONSTRAINT agent_limit_reservation_items_reservation_tenant_run_fk
        FOREIGN KEY (reservation_id, tenant_id, run_id)
        REFERENCES agent_limit_reservations(id, tenant_id, run_id) ON DELETE RESTRICT,
    CONSTRAINT agent_limit_reservation_items_bucket_tenant_fk
        FOREIGN KEY (bucket_id, tenant_id)
        REFERENCES agent_limit_buckets(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_limit_reservation_items_entitlement_bucket_tenant_fk
        FOREIGN KEY (entitlement_bucket_id, tenant_id)
        REFERENCES entitlement_meter_buckets(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_limit_items_entitlement_reservation_fk
        FOREIGN KEY (entitlement_reservation_id, tenant_id, entitlement_bucket_id)
        REFERENCES entitlement_usage_reservations(id, tenant_id, bucket_id)
        ON DELETE RESTRICT,
    CONSTRAINT agent_limit_reservation_items_rule_tenant_fk
        FOREIGN KEY (campus_rule_id, tenant_id)
        REFERENCES agent_limit_rules(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_limit_reservation_items_definition_shape_check CHECK (
        (
            definition_kind = 'local_rule'
            AND bucket_id IS NOT NULL
            AND entitlement_bucket_id IS NULL
            AND entitlement_reservation_id IS NULL
            AND campus_rule_id IS NOT NULL
            AND source_lease_id IS NULL
            AND entitlement_limit_key IS NULL
            AND definition_version IS NOT NULL
        )
        OR (
            definition_kind = 'signed_entitlement'
            AND bucket_id IS NULL
            AND entitlement_bucket_id IS NOT NULL
            AND campus_rule_id IS NULL
            AND source_lease_id IS NOT NULL
            AND entitlement_limit_key IS NOT NULL
            AND definition_version IS NULL
            AND scope_kind = 'campus'
            AND agent_usage_meter_unit(meter_key) <> 'money'
            AND (
                (reserved_amount = 0 AND entitlement_reservation_id IS NULL)
                OR (
                    reserved_amount = requested_amount
                    AND entitlement_reservation_id IS NOT NULL
                )
            )
        )
    ),
    CONSTRAINT agent_limit_reservation_items_period_shape_check CHECK (
        (period = 'none' AND period_end IS NULL)
        OR (period <> 'none' AND period_end IS NOT NULL AND period_end > period_start)
    ),
    CONSTRAINT agent_limit_reservation_items_money_shape_check CHECK (
        (
            agent_usage_meter_unit(meter_key) = 'money'
            AND currency_code IS NOT NULL
            AND currency_exponent IS NOT NULL
            AND meter_key = 'agent.estimated_cost'
            AND pricing_version IS NOT NULL
        )
        OR (
            agent_usage_meter_unit(meter_key) <> 'money'
            AND currency_code IS NULL
            AND currency_exponent IS NULL
            AND pricing_version IS NULL
        )
    ),
    CONSTRAINT agent_limit_reservation_items_decision_shape_check CHECK (
        (decision = 'denied' AND reserved_amount = 0)
        OR (decision = 'allowed' AND reserved_amount IN (0, requested_amount))
    ),
    UNIQUE (reservation_id, item_sequence),
    UNIQUE (entitlement_reservation_id),
    UNIQUE NULLS NOT DISTINCT (reservation_id, definition_kind, bucket_id, entitlement_bucket_id)
);

CREATE INDEX IF NOT EXISTS agent_limit_reservation_items_bucket_idx
    ON agent_limit_reservation_items (tenant_id, bucket_id, created_at);

CREATE INDEX IF NOT EXISTS agent_limit_reservation_items_entitlement_bucket_idx
    ON agent_limit_reservation_items (
        tenant_id, entitlement_bucket_id, created_at
    )
    WHERE entitlement_bucket_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS agent_limit_reservation_items_identity_unique
    ON agent_limit_reservation_items (id, tenant_id, reservation_id, run_id);

-- One immutable reconciliation converts an upper-bound reservation into the
-- amount that is actually committed. Unknown terminal usage retains the full
-- upper bound explicitly; zero is a real outcome and never becomes a fake
-- positive commercial usage event.
CREATE TABLE IF NOT EXISTS agent_limit_reconciliations (
    id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    reservation_id UUID NOT NULL,
    run_id UUID NOT NULL,
    reservation_item_id UUID NOT NULL,
    committed_amount BIGINT NOT NULL CHECK (
        committed_amount BETWEEN 0 AND 9007199254740991
    ),
    enforcement_basis TEXT NOT NULL CHECK (
        enforcement_basis IN ('exact', 'estimated', 'upper_bound')
    ),
    deleted_at TIMESTAMPTZ CHECK (deleted_at IS NULL),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_limit_reconciliations_id_tenant_unique
        UNIQUE (id, tenant_id),
    CONSTRAINT agent_limit_reconciliations_reservation_tenant_run_fk
        FOREIGN KEY (reservation_id, tenant_id, run_id)
        REFERENCES agent_limit_reservations(id, tenant_id, run_id)
        ON DELETE RESTRICT,
    CONSTRAINT agent_limit_reconciliations_item_identity_fk
        FOREIGN KEY (reservation_item_id, tenant_id, reservation_id, run_id)
        REFERENCES agent_limit_reservation_items(
            id, tenant_id, reservation_id, run_id
        ) ON DELETE RESTRICT,
    UNIQUE (reservation_item_id)
);

CREATE INDEX IF NOT EXISTS agent_limit_reconciliations_reservation_idx
    ON agent_limit_reconciliations (tenant_id, reservation_id, reservation_item_id);

CREATE TABLE IF NOT EXISTS agent_usage_events (
    id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    event_kind TEXT NOT NULL CHECK (
        event_kind IN ('run', 'provider_attempt', 'capability_call')
    ),
    run_id UUID NOT NULL,
    thread_id UUID NOT NULL,
    actor_user_id UUID NOT NULL,
    role_keys TEXT[] NOT NULL CHECK (agent_usage_valid_role_keys(role_keys)),
    origin_module_key TEXT NOT NULL CHECK (
        agent_usage_valid_key(origin_module_key, 160)
    ),
    task_class TEXT NOT NULL CHECK (task_class IN (
        'campus_conversation_search',
        'module_read_reporting',
        'document_extraction',
        'drafting_proposal',
        'approved_operational_action'
    )),
    provider_attempt_id UUID,
    provider_turn_index SMALLINT CHECK (
        provider_turn_index IS NULL OR provider_turn_index BETWEEN 1 AND 16
    ),
    provider_attempt_index SMALLINT CHECK (
        provider_attempt_index IS NULL OR provider_attempt_index BETWEEN 1 AND 3
    ),
    provider_connection_id UUID,
    provider_key TEXT CHECK (
        provider_key IS NULL OR provider_key IN ('openai', 'anthropic', 'openrouter')
    ),
    provider_model_id TEXT CHECK (
        provider_model_id IS NULL
        OR CHAR_LENGTH(BTRIM(provider_model_id)) BETWEEN 1 AND 240
    ),
    provider_model_snapshot_id UUID,
    route_priority SMALLINT CHECK (
        route_priority IS NULL OR route_priority BETWEEN 1 AND 3
    ),
    failure_origin TEXT CHECK (
        failure_origin IS NULL OR failure_origin IN ('preflight', 'upstream')
    ),
    failure_category TEXT CHECK (
        failure_category IS NULL
        OR agent_usage_valid_key(failure_category, 100)
    ),
    capability_call_id UUID,
    capability_module_key TEXT CHECK (
        capability_module_key IS NULL
        OR agent_usage_valid_key(capability_module_key, 160)
    ),
    capability_key TEXT CHECK (
        capability_key IS NULL OR agent_usage_valid_key(capability_key, 200)
    ),
    capability_version INTEGER CHECK (
        capability_version IS NULL OR capability_version > 0
    ),
    approval_state TEXT CHECK (
        approval_state IS NULL
        OR approval_state IN (
            'not_required', 'requested', 'approved', 'rejected', 'expired', 'stale'
        )
    ),
    outcome TEXT NOT NULL CHECK (
        outcome IN ('succeeded', 'failed', 'denied', 'cancelled', 'interrupted')
    ),
    safe_failure_code TEXT CHECK (
        safe_failure_code IS NULL
        OR agent_usage_valid_key(safe_failure_code, 100)
    ),
    duration_ms BIGINT NOT NULL CHECK (
        duration_ms BETWEEN 0 AND 9007199254740991
    ),
    request_id UUID NOT NULL,
    correlation_id UUID NOT NULL,
    limit_reservation_id UUID,
    occurred_at TIMESTAMPTZ NOT NULL,
    deleted_at TIMESTAMPTZ CHECK (deleted_at IS NULL),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_usage_events_id_tenant_unique UNIQUE (id, tenant_id),
    CONSTRAINT agent_usage_events_run_tenant_fk
        FOREIGN KEY (run_id, tenant_id)
        REFERENCES agent_runs(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_usage_events_thread_tenant_fk
        FOREIGN KEY (thread_id, tenant_id)
        REFERENCES agent_threads(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_usage_events_actor_tenant_fk
        FOREIGN KEY (actor_user_id, tenant_id)
        REFERENCES users(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_usage_events_provider_attempt_tenant_run_fk
        FOREIGN KEY (provider_attempt_id, tenant_id, run_id)
        REFERENCES agent_provider_attempts(id, tenant_id, run_id) ON DELETE RESTRICT,
    CONSTRAINT agent_usage_events_provider_connection_tenant_fk
        FOREIGN KEY (provider_connection_id, tenant_id)
        REFERENCES ai_provider_connections(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_usage_events_provider_model_tenant_fk
        FOREIGN KEY (provider_model_snapshot_id, tenant_id)
        REFERENCES ai_provider_models(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_usage_events_capability_call_tenant_run_fk
        FOREIGN KEY (capability_call_id, tenant_id, run_id)
        REFERENCES agent_capability_calls(id, tenant_id, run_id) ON DELETE RESTRICT,
    CONSTRAINT agent_usage_events_reservation_tenant_run_fk
        FOREIGN KEY (limit_reservation_id, tenant_id, run_id)
        REFERENCES agent_limit_reservations(id, tenant_id, run_id) ON DELETE RESTRICT,
    CONSTRAINT agent_usage_events_kind_shape_check CHECK (
        (
            event_kind = 'run'
            AND provider_attempt_id IS NULL
            AND provider_turn_index IS NULL
            AND provider_attempt_index IS NULL
            AND provider_connection_id IS NULL
            AND provider_key IS NULL
            AND provider_model_id IS NULL
            AND provider_model_snapshot_id IS NULL
            AND route_priority IS NULL
            AND failure_origin IS NULL
            AND failure_category IS NULL
            AND capability_call_id IS NULL
            AND capability_module_key IS NULL
            AND capability_key IS NULL
            AND capability_version IS NULL
            AND approval_state IS NULL
        )
        OR (
            event_kind = 'provider_attempt'
            AND provider_attempt_id IS NOT NULL
            AND provider_turn_index IS NOT NULL
            AND provider_attempt_index IS NOT NULL
            AND provider_connection_id IS NOT NULL
            AND provider_key IS NOT NULL
            AND provider_model_id IS NOT NULL
            AND provider_model_snapshot_id IS NOT NULL
            AND route_priority IS NOT NULL
            AND capability_call_id IS NULL
            AND capability_module_key IS NULL
            AND capability_key IS NULL
            AND capability_version IS NULL
            AND approval_state IS NULL
        )
        OR (
            event_kind = 'capability_call'
            AND provider_attempt_id IS NULL
            AND provider_turn_index IS NULL
            AND provider_attempt_index IS NULL
            AND provider_connection_id IS NULL
            AND provider_key IS NULL
            AND provider_model_id IS NULL
            AND provider_model_snapshot_id IS NULL
            AND route_priority IS NULL
            AND failure_origin IS NULL
            AND failure_category IS NULL
            AND capability_call_id IS NOT NULL
            AND capability_module_key IS NOT NULL
            AND capability_key IS NOT NULL
            AND capability_version IS NOT NULL
            AND approval_state IS NOT NULL
        )
    ),
    CONSTRAINT agent_usage_events_outcome_shape_check CHECK (
        (outcome IN ('succeeded', 'cancelled') AND safe_failure_code IS NULL)
        OR (outcome IN ('failed', 'denied', 'interrupted') AND safe_failure_code IS NOT NULL)
    ),
    CONSTRAINT agent_usage_events_child_reservation_check CHECK (
        event_kind = 'run' OR limit_reservation_id IS NOT NULL
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS agent_usage_events_run_source_unique
    ON agent_usage_events (run_id)
    WHERE event_kind = 'run';

CREATE UNIQUE INDEX IF NOT EXISTS agent_usage_events_provider_source_unique
    ON agent_usage_events (provider_attempt_id)
    WHERE provider_attempt_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS agent_usage_events_capability_source_unique
    ON agent_usage_events (capability_call_id)
    WHERE capability_call_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS agent_usage_events_reservation_unique
    ON agent_usage_events (limit_reservation_id)
    WHERE limit_reservation_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS agent_usage_events_person_reporting_idx
    ON agent_usage_events (tenant_id, actor_user_id, occurred_at DESC, id);

CREATE INDEX IF NOT EXISTS agent_usage_events_origin_reporting_idx
    ON agent_usage_events (tenant_id, origin_module_key, occurred_at DESC, id);

CREATE INDEX IF NOT EXISTS agent_usage_events_capability_reporting_idx
    ON agent_usage_events (
        tenant_id, capability_module_key, capability_key, occurred_at DESC, id
    )
    WHERE event_kind = 'capability_call';

CREATE INDEX IF NOT EXISTS agent_usage_events_provider_reporting_idx
    ON agent_usage_events (
        tenant_id, provider_key, provider_model_id, occurred_at DESC, id
    )
    WHERE event_kind = 'provider_attempt';

CREATE INDEX IF NOT EXISTS agent_usage_events_role_keys_idx
    ON agent_usage_events USING GIN (role_keys);

CREATE TABLE IF NOT EXISTS agent_usage_measures (
    id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    usage_event_id UUID NOT NULL,
    meter_key TEXT NOT NULL CHECK (agent_usage_meter_unit(meter_key) IS NOT NULL),
    amount BIGINT CHECK (
        amount IS NULL OR amount BETWEEN 0 AND 9007199254740991
    ),
    enforcement_amount BIGINT CHECK (
        enforcement_amount IS NULL
        OR enforcement_amount BETWEEN 0 AND 9007199254740991
    ),
    enforcement_basis TEXT CHECK (
        enforcement_basis IS NULL
        OR enforcement_basis IN ('exact', 'estimated', 'upper_bound')
    ),
    currency_code TEXT CHECK (
        currency_code IS NULL OR currency_code ~ '^[A-Z]{3}$'
    ),
    currency_exponent SMALLINT CHECK (
        currency_exponent IS NULL OR currency_exponent BETWEEN 0 AND 9
    ),
    pricing_version TEXT CHECK (
        pricing_version IS NULL
        OR CHAR_LENGTH(BTRIM(pricing_version)) BETWEEN 1 AND 100
    ),
    deleted_at TIMESTAMPTZ CHECK (deleted_at IS NULL),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_usage_measures_id_tenant_unique UNIQUE (id, tenant_id),
    CONSTRAINT agent_usage_measures_event_tenant_fk
        FOREIGN KEY (usage_event_id, tenant_id)
        REFERENCES agent_usage_events(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_usage_measures_enforcement_shape_check CHECK (
        (enforcement_amount IS NULL AND enforcement_basis IS NULL)
        OR (enforcement_amount IS NOT NULL AND enforcement_basis IS NOT NULL)
    ),
    CONSTRAINT agent_usage_measures_money_shape_check CHECK (
        (
            meter_key = 'agent.provider_reported_cost'
            AND (
                (
                    amount IS NULL
                    AND enforcement_amount IS NULL
                    AND currency_code IS NULL
                    AND currency_exponent IS NULL
                    AND pricing_version IS NULL
                )
                OR (
                    amount IS NOT NULL
                    AND enforcement_amount IS NULL
                    AND currency_code IS NOT NULL
                    AND currency_exponent IS NOT NULL
                )
            )
        )
        OR (
            meter_key = 'agent.estimated_cost'
            AND (
                (
                    amount IS NULL
                    AND enforcement_amount IS NULL
                    AND currency_code IS NULL
                    AND currency_exponent IS NULL
                    AND pricing_version IS NULL
                )
                OR (
                    (amount IS NOT NULL OR enforcement_amount IS NOT NULL)
                    AND currency_code IS NOT NULL
                    AND currency_exponent IS NOT NULL
                    AND pricing_version IS NOT NULL
                )
            )
        )
        OR (
            agent_usage_meter_unit(meter_key) <> 'money'
            AND currency_code IS NULL
            AND currency_exponent IS NULL
            AND pricing_version IS NULL
        )
    ),
    CONSTRAINT agent_usage_measures_report_only_check CHECK (
        meter_key NOT IN (
            'agent.provider_reported_cost',
            'agent.cached_input_tokens',
            'agent.reasoning_tokens'
        )
        OR enforcement_amount IS NULL
    ),
    UNIQUE (usage_event_id, meter_key)
);

CREATE INDEX IF NOT EXISTS agent_usage_measures_reporting_idx
    ON agent_usage_measures (
        tenant_id, meter_key, currency_code, currency_exponent, usage_event_id
    );

CREATE OR REPLACE FUNCTION validate_agent_limit_rule_insert()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.version <> 1 OR NEW.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'Agent limit rules must start active at version one';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_limit_rules_validate_insert ON agent_limit_rules;
CREATE TRIGGER agent_limit_rules_validate_insert
    BEFORE INSERT ON agent_limit_rules
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_limit_rule_insert();

CREATE OR REPLACE FUNCTION protect_agent_limit_rule_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Agent limit rules are archived, not deleted';
    END IF;
    IF OLD.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'archived Agent limit rules are immutable';
    END IF;
    IF NEW.id IS DISTINCT FROM OLD.id
       OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
       OR NEW.scope_kind IS DISTINCT FROM OLD.scope_kind
       OR NEW.person_user_id IS DISTINCT FROM OLD.person_user_id
       OR NEW.role_key IS DISTINCT FROM OLD.role_key
       OR NEW.origin_module_key IS DISTINCT FROM OLD.origin_module_key
       OR NEW.capability_module_key IS DISTINCT FROM OLD.capability_module_key
       OR NEW.capability_key IS DISTINCT FROM OLD.capability_key
       OR NEW.provider_key IS DISTINCT FROM OLD.provider_key
       OR NEW.provider_model_id IS DISTINCT FROM OLD.provider_model_id
       OR NEW.meter_key IS DISTINCT FROM OLD.meter_key
       OR NEW.currency_code IS DISTINCT FROM OLD.currency_code
       OR NEW.currency_exponent IS DISTINCT FROM OLD.currency_exponent
       OR NEW.period IS DISTINCT FROM OLD.period
       OR NEW.provenance_kind IS DISTINCT FROM OLD.provenance_kind
       OR NEW.effective_from IS DISTINCT FROM OLD.effective_from
       OR NEW.configured_by IS DISTINCT FROM OLD.configured_by
       OR NEW.created_at IS DISTINCT FROM OLD.created_at
       OR NEW.version <> OLD.version + 1
       OR NEW.updated_at <= OLD.updated_at THEN
        RAISE EXCEPTION 'invalid Agent limit rule lifecycle update';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_limit_rules_protect_lifecycle ON agent_limit_rules;
CREATE TRIGGER agent_limit_rules_protect_lifecycle
    BEFORE UPDATE OR DELETE ON agent_limit_rules
    FOR EACH ROW
    EXECUTE FUNCTION protect_agent_limit_rule_lifecycle();

CREATE OR REPLACE FUNCTION validate_agent_limit_bucket_insert()
RETURNS TRIGGER AS $$
DECLARE
    stored_meter_key TEXT;
    stored_currency_code TEXT;
    stored_currency_exponent SMALLINT;
    stored_period TEXT;
    stored_enforcement TEXT;
BEGIN
    IF NEW.deleted_at IS NOT NULL
       OR NEW.committed_value <> 0
       OR NEW.reserved_value <> 0 THEN
        RAISE EXCEPTION 'Agent limit buckets must start empty and active';
    END IF;

    SELECT meter_key, currency_code, currency_exponent, period, enforcement
    INTO stored_meter_key, stored_currency_code, stored_currency_exponent,
         stored_period, stored_enforcement
    FROM agent_limit_rules
    WHERE id = NEW.campus_rule_id
      AND tenant_id = NEW.tenant_id
      AND deleted_at IS NULL;

    IF NOT FOUND
       OR stored_enforcement <> 'hard'
       OR stored_meter_key <> NEW.meter_key
       OR stored_currency_code IS DISTINCT FROM NEW.currency_code
       OR stored_currency_exponent IS DISTINCT FROM NEW.currency_exponent
       OR stored_period <> NEW.period THEN
        RAISE EXCEPTION 'Agent limit bucket must match its active local tightening rule';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_limit_buckets_validate_insert ON agent_limit_buckets;
CREATE TRIGGER agent_limit_buckets_validate_insert
    BEFORE INSERT ON agent_limit_buckets
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_limit_bucket_insert();

CREATE OR REPLACE FUNCTION protect_agent_limit_bucket_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Agent limit buckets are retained, not deleted';
    END IF;
    IF NEW.id IS DISTINCT FROM OLD.id
       OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
       OR NEW.campus_rule_id IS DISTINCT FROM OLD.campus_rule_id
       OR NEW.meter_key IS DISTINCT FROM OLD.meter_key
       OR NEW.currency_code IS DISTINCT FROM OLD.currency_code
       OR NEW.currency_exponent IS DISTINCT FROM OLD.currency_exponent
       OR NEW.period IS DISTINCT FROM OLD.period
       OR NEW.period_start IS DISTINCT FROM OLD.period_start
       OR NEW.period_end IS DISTINCT FROM OLD.period_end
       OR NEW.created_at IS DISTINCT FROM OLD.created_at
       OR NEW.deleted_at IS NOT NULL
       OR NEW.committed_value < OLD.committed_value
       OR NEW.updated_at <= OLD.updated_at THEN
        RAISE EXCEPTION 'invalid Agent limit bucket lifecycle update';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_limit_buckets_protect_lifecycle ON agent_limit_buckets;
CREATE TRIGGER agent_limit_buckets_protect_lifecycle
    BEFORE UPDATE OR DELETE ON agent_limit_buckets
    FOR EACH ROW
    EXECUTE FUNCTION protect_agent_limit_bucket_lifecycle();

CREATE OR REPLACE FUNCTION validate_agent_limit_reservation_insert()
RETURNS TRIGGER AS $$
DECLARE
    stored_actor_user_id UUID;
    stored_origin_module_key TEXT;
    stored_child_run_id UUID;
    stored_child_sequence SMALLINT;
    stored_child_input_fingerprint BYTEA;
BEGIN
    IF NEW.status <> 'preparing'
       OR NEW.deleted_at IS NOT NULL
       OR NEW.claimed_at IS NOT NULL
       OR NEW.claimed_by_worker_id IS NOT NULL
       OR NEW.claim_fence_version IS NOT NULL THEN
        RAISE EXCEPTION 'Agent limit reservations must start in preparation';
    END IF;

    SELECT requested_by, origin_module_key
    INTO stored_actor_user_id, stored_origin_module_key
    FROM agent_runs
    WHERE id = NEW.run_id
      AND tenant_id = NEW.tenant_id;

    IF NOT FOUND
       OR stored_actor_user_id <> NEW.actor_user_id
       OR stored_origin_module_key <> NEW.origin_module_key THEN
        RAISE EXCEPTION 'Agent limit reservation identity must match its same-tenant run';
    END IF;

    IF NEW.stage_kind = 'provider_attempt' THEN
        SELECT attempt.run_id,
               ((attempt.turn_index - 1) * 3 + attempt.attempt_index)::SMALLINT,
               execution_step.input_fingerprint
        INTO stored_child_run_id, stored_child_sequence,
             stored_child_input_fingerprint
        FROM agent_provider_attempts AS attempt
        INNER JOIN agent_execution_steps AS execution_step
          ON execution_step.provider_attempt_id = attempt.id
         AND execution_step.tenant_id = attempt.tenant_id
         AND execution_step.run_id = attempt.run_id
         AND execution_step.step_kind = 'provider_attempt'
        WHERE attempt.id = NEW.provider_attempt_id
          AND attempt.tenant_id = NEW.tenant_id;
    ELSIF NEW.stage_kind = 'capability_call' THEN
        SELECT run_id, call_sequence, input_fingerprint
        INTO stored_child_run_id, stored_child_sequence,
             stored_child_input_fingerprint
        FROM agent_capability_calls
        WHERE id = NEW.capability_call_id
          AND tenant_id = NEW.tenant_id;
    ELSE
        RETURN NEW;
    END IF;

    IF NOT FOUND
       OR stored_child_run_id <> NEW.run_id
       OR stored_child_sequence <> NEW.stage_sequence
       OR stored_child_input_fingerprint IS NULL
       OR stored_child_input_fingerprint <> NEW.request_fingerprint THEN
        RAISE EXCEPTION 'Agent limit reservation must bind the exact child input';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_limit_reservations_validate_insert
    ON agent_limit_reservations;
CREATE TRIGGER agent_limit_reservations_validate_insert
    BEFORE INSERT ON agent_limit_reservations
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_limit_reservation_insert();

CREATE OR REPLACE FUNCTION protect_agent_limit_reservation_lifecycle()
RETURNS TRIGGER AS $$
DECLARE
    item_count BIGINT;
    denied_count BIGINT;
    invalid_reserved_count BIGINT;
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Agent limit reservations are retained, not deleted';
    END IF;
    IF NEW.id IS DISTINCT FROM OLD.id
       OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
       OR NEW.run_id IS DISTINCT FROM OLD.run_id
       OR NEW.provider_attempt_id IS DISTINCT FROM OLD.provider_attempt_id
       OR NEW.capability_call_id IS DISTINCT FROM OLD.capability_call_id
       OR NEW.actor_user_id IS DISTINCT FROM OLD.actor_user_id
       OR NEW.role_keys IS DISTINCT FROM OLD.role_keys
       OR NEW.origin_module_key IS DISTINCT FROM OLD.origin_module_key
       OR NEW.capability_module_key IS DISTINCT FROM OLD.capability_module_key
       OR NEW.capability_key IS DISTINCT FROM OLD.capability_key
       OR NEW.provider_key IS DISTINCT FROM OLD.provider_key
       OR NEW.provider_model_id IS DISTINCT FROM OLD.provider_model_id
       OR NEW.stage_kind IS DISTINCT FROM OLD.stage_kind
       OR NEW.stage_sequence IS DISTINCT FROM OLD.stage_sequence
       OR NEW.idempotency_key IS DISTINCT FROM OLD.idempotency_key
       OR NEW.request_fingerprint IS DISTINCT FROM OLD.request_fingerprint
       OR NEW.created_at IS DISTINCT FROM OLD.created_at
       OR NEW.deleted_at IS NOT NULL
       OR NEW.updated_at <= OLD.updated_at THEN
        RAISE EXCEPTION 'Agent limit reservation identity is immutable';
    END IF;

    IF NEW.claimed_at IS DISTINCT FROM OLD.claimed_at
       OR NEW.claimed_by_worker_id IS DISTINCT FROM OLD.claimed_by_worker_id
       OR NEW.claim_fence_version IS DISTINCT FROM OLD.claim_fence_version THEN
        IF OLD.claimed_at IS NULL
           AND OLD.claimed_by_worker_id IS NULL
           AND OLD.claim_fence_version IS NULL
           AND NEW.claimed_at = STATEMENT_TIMESTAMP()
           AND NEW.claimed_by_worker_id IS NOT NULL
           AND NEW.claim_fence_version IS NOT NULL
           AND OLD.status IN ('reserved', 'not_limited')
           AND NEW.status = OLD.status
           AND NEW.expires_at IS NOT DISTINCT FROM OLD.expires_at
           AND NEW.committed_at IS NOT DISTINCT FROM OLD.committed_at
           AND NEW.released_at IS NOT DISTINCT FROM OLD.released_at
           AND NEW.denied_at IS NOT DISTINCT FROM OLD.denied_at THEN
            RETURN NEW;
        END IF;
        RAISE EXCEPTION 'Agent limit execution claim is one-time and immutable';
    END IF;

    IF OLD.status = 'preparing' THEN
        -- Every active local hard rule that matches this stage and scope must be
        -- represented. This makes omission unable to turn an overlap into allow.
        IF EXISTS (
            SELECT 1
            FROM agent_limit_rules AS rule
            WHERE rule.tenant_id = OLD.tenant_id
              AND rule.deleted_at IS NULL
              AND rule.effective_from <= STATEMENT_TIMESTAMP()
              AND rule.enforcement = 'hard'
              AND agent_usage_stage_supports_meter(
                  OLD.stage_kind, rule.meter_key
              )
              AND (
                  rule.scope_kind = 'campus'
                  OR (
                      rule.scope_kind = 'person'
                      AND rule.person_user_id = OLD.actor_user_id
                  )
                  OR (
                      rule.scope_kind = 'role'
                      AND rule.role_key = ANY(OLD.role_keys)
                  )
                  OR (
                      rule.scope_kind = 'origin_module'
                      AND rule.origin_module_key = OLD.origin_module_key
                  )
                  OR (
                      rule.scope_kind = 'capability_module'
                      AND rule.capability_module_key = OLD.capability_module_key
                  )
                  OR (
                      rule.scope_kind = 'capability'
                      AND rule.capability_key = OLD.capability_key
                  )
                  OR (
                      rule.scope_kind = 'provider'
                      AND rule.provider_key = OLD.provider_key
                  )
                  OR (
                      rule.scope_kind = 'model'
                      AND rule.provider_key = OLD.provider_key
                      AND rule.provider_model_id = OLD.provider_model_id
                  )
              )
              AND NOT EXISTS (
                  SELECT 1
                  FROM agent_limit_reservation_items AS item
                  WHERE item.reservation_id = OLD.id
                    AND item.tenant_id = OLD.tenant_id
                    AND item.definition_kind = 'local_rule'
                    AND item.campus_rule_id = rule.id
                    AND item.definition_version = rule.version
                    AND item.limit_value = rule.limit_value
              )
        ) THEN
            RAISE EXCEPTION 'Agent limit preparation omitted a matching local tightening rule';
        END IF;

        -- Signed entitlement_limits are authoritative commercial definitions.
        -- The Agent ledger maps them; it never substitutes an Agent-owned counter.
        IF EXISTS (
            SELECT 1
            FROM entitlement_limits AS entitlement
            WHERE entitlement.tenant_id = OLD.tenant_id
              AND entitlement.enforcement = 'hard'
              AND agent_usage_meter_unit(entitlement.limit_key) IS NOT NULL
              AND agent_usage_meter_unit(entitlement.limit_key) <> 'money'
              AND agent_usage_stage_supports_meter(
                  OLD.stage_kind, entitlement.limit_key
              )
              AND NOT EXISTS (
                  SELECT 1
                  FROM agent_limit_reservation_items AS item
                  WHERE item.reservation_id = OLD.id
                    AND item.tenant_id = OLD.tenant_id
                    AND item.definition_kind = 'signed_entitlement'
                    AND item.source_lease_id = entitlement.source_lease_id
                    AND item.entitlement_limit_key = entitlement.limit_key
                    AND item.unit = entitlement.unit
                    AND item.period = entitlement.period
                    AND item.limit_value = entitlement.limit_value
              )
        ) THEN
            RAISE EXCEPTION 'Agent limit preparation omitted a current signed entitlement';
        END IF;

        SELECT
            COUNT(*),
            COUNT(*) FILTER (WHERE decision = 'denied'),
            COUNT(*) FILTER (
                WHERE reserved_amount NOT IN (0, requested_amount)
            )
        INTO item_count, denied_count, invalid_reserved_count
        FROM agent_limit_reservation_items
        WHERE reservation_id = OLD.id
          AND tenant_id = OLD.tenant_id;

        IF NEW.status = 'not_limited' AND item_count = 0 THEN
            RETURN NEW;
        END IF;
        IF NEW.status = 'denied'
           AND item_count > 0
           AND denied_count > 0
           AND NOT EXISTS (
               SELECT 1
               FROM agent_limit_reservation_items
               WHERE reservation_id = OLD.id
                 AND tenant_id = OLD.tenant_id
                 AND reserved_amount <> 0
           ) THEN
            RETURN NEW;
        END IF;
        IF NEW.status = 'reserved'
           AND item_count > 0
           AND denied_count = 0
           AND invalid_reserved_count = 0
           AND NOT EXISTS (
               SELECT 1
               FROM agent_limit_reservation_items
               WHERE reservation_id = OLD.id
                 AND tenant_id = OLD.tenant_id
                 AND reserved_amount <> requested_amount
           ) THEN
            RETURN NEW;
        END IF;
        RAISE EXCEPTION 'Agent limit preparation must finalize all matching rules atomically';
    END IF;

    IF OLD.status = 'reserved' THEN
        IF NEW.status = 'committed'
           AND (
               OLD.stage_kind = 'run'
               OR OLD.claimed_at IS NOT NULL
           )
           AND NOT EXISTS (
               SELECT 1
               FROM agent_limit_reservation_items AS item
               WHERE item.reservation_id = OLD.id
                 AND item.tenant_id = OLD.tenant_id
                 AND NOT EXISTS (
                     SELECT 1
                     FROM agent_limit_reconciliations AS reconciliation
                     WHERE reconciliation.reservation_item_id = item.id
                       AND reconciliation.tenant_id = item.tenant_id
                       AND reconciliation.reservation_id = item.reservation_id
                       AND reconciliation.run_id = item.run_id
                 )
           )
           AND NOT EXISTS (
               SELECT 1
               FROM agent_limit_reservation_items AS item
               INNER JOIN agent_limit_reconciliations AS reconciliation
                 ON reconciliation.reservation_item_id = item.id
                AND reconciliation.tenant_id = item.tenant_id
                AND reconciliation.reservation_id = item.reservation_id
                AND reconciliation.run_id = item.run_id
               WHERE item.reservation_id = OLD.id
                 AND item.tenant_id = OLD.tenant_id
                 AND item.definition_kind = 'signed_entitlement'
                 AND (
                     (
                         reconciliation.committed_amount > 0
                         AND NOT EXISTS (
                             SELECT 1
                             FROM entitlement_usage_reservations AS source_reservation
                             INNER JOIN entitlement_usage_events AS source_event
                               ON source_event.reservation_id = source_reservation.id
                              AND source_event.tenant_id = item.tenant_id
                              AND source_event.amount = reconciliation.committed_amount
                             WHERE source_reservation.id = item.entitlement_reservation_id
                               AND source_reservation.tenant_id = item.tenant_id
                               AND source_reservation.bucket_id = item.entitlement_bucket_id
                               AND source_reservation.status = 'committed'
                         )
                     )
                     OR (
                         reconciliation.committed_amount = 0
                         AND NOT EXISTS (
                             SELECT 1
                             FROM entitlement_usage_reservations AS source_reservation
                             WHERE source_reservation.id = item.entitlement_reservation_id
                               AND source_reservation.tenant_id = item.tenant_id
                               AND source_reservation.bucket_id = item.entitlement_bucket_id
                               AND source_reservation.status = 'released'
                               AND NOT EXISTS (
                                   SELECT 1 FROM entitlement_usage_events
                                   WHERE reservation_id = source_reservation.id
                               )
                         )
                     )
                 )
           ) THEN
            RETURN NEW;
        END IF;
        IF NEW.status = 'committed'
           AND OLD.stage_kind IN ('provider_attempt', 'capability_call')
           AND OLD.claimed_at IS NULL THEN
            RAISE EXCEPTION 'Agent child limit reservation must be claimed before completion';
        END IF;
        IF NEW.status IN ('released', 'expired')
           AND OLD.claimed_at IS NULL
           AND NOT EXISTS (
               SELECT 1
               FROM agent_limit_reservation_items AS item
               INNER JOIN entitlement_usage_reservations AS source_reservation
                 ON source_reservation.id = item.entitlement_reservation_id
                AND source_reservation.tenant_id = item.tenant_id
                AND source_reservation.bucket_id = item.entitlement_bucket_id
               WHERE item.reservation_id = OLD.id
                 AND item.tenant_id = OLD.tenant_id
                 AND item.definition_kind = 'signed_entitlement'
                 AND source_reservation.status <> NEW.status
        ) THEN
            RETURN NEW;
        END IF;
        IF NEW.status IN ('released', 'expired')
           AND OLD.claimed_at IS NOT NULL THEN
            RAISE EXCEPTION 'claimed Agent limit reservations must be reconciled';
        END IF;
        RAISE EXCEPTION 'Agent limit lifecycle must match canonical entitlement reservations';
    END IF;

    RAISE EXCEPTION 'terminal Agent limit reservations are immutable';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_limit_reservations_protect_lifecycle
    ON agent_limit_reservations;
CREATE TRIGGER agent_limit_reservations_protect_lifecycle
    BEFORE UPDATE OR DELETE ON agent_limit_reservations
    FOR EACH ROW
    EXECUTE FUNCTION protect_agent_limit_reservation_lifecycle();

CREATE OR REPLACE FUNCTION validate_agent_limit_reservation_item_insert()
RETURNS TRIGGER AS $$
DECLARE
    reservation_status TEXT;
    reservation_stage_kind TEXT;
    reservation_actor_user_id UUID;
    reservation_role_keys TEXT[];
    reservation_origin_module_key TEXT;
    reservation_capability_module_key TEXT;
    reservation_capability_key TEXT;
    reservation_provider_key TEXT;
    reservation_provider_model_id TEXT;
    stored_rule_version BIGINT;
    stored_scope_kind TEXT;
    stored_person_user_id UUID;
    stored_role_key TEXT;
    stored_origin_module_key TEXT;
    stored_capability_module_key TEXT;
    stored_capability_key TEXT;
    stored_provider_key TEXT;
    stored_provider_model_id TEXT;
    stored_meter_key TEXT;
    stored_currency_code TEXT;
    stored_currency_exponent SMALLINT;
    stored_period TEXT;
    stored_limit_value BIGINT;
    stored_enforcement TEXT;
    stored_source_lease_id UUID;
    stored_unit TEXT;
    stored_bucket_rule_id UUID;
    stored_bucket_limit_key TEXT;
    bucket_period_start TIMESTAMPTZ;
    bucket_period_end TIMESTAMPTZ;
    bucket_committed_value BIGINT;
    bucket_reserved_value BIGINT;
    source_reservation_status TEXT;
    source_reservation_bucket_id UUID;
    source_reservation_lease_id UUID;
    source_reservation_limit_key TEXT;
    source_reservation_unit TEXT;
    source_reservation_amount BIGINT;
    expected_scope_value TEXT;
BEGIN
    SELECT status, stage_kind, actor_user_id, role_keys, origin_module_key,
           capability_module_key, capability_key, provider_key, provider_model_id
    INTO reservation_status, reservation_stage_kind, reservation_actor_user_id,
         reservation_role_keys, reservation_origin_module_key,
         reservation_capability_module_key, reservation_capability_key,
         reservation_provider_key, reservation_provider_model_id
    FROM agent_limit_reservations
    WHERE id = NEW.reservation_id
      AND tenant_id = NEW.tenant_id
      AND run_id = NEW.run_id;

    IF NOT FOUND OR reservation_status <> 'preparing' THEN
        RAISE EXCEPTION 'Agent limit items require a preparing same-run reservation';
    END IF;

    IF NOT agent_usage_stage_supports_meter(
        reservation_stage_kind, NEW.meter_key
    ) THEN
        RAISE EXCEPTION 'Agent limit item meter does not belong to its stage';
    END IF;

    IF NEW.definition_kind = 'local_rule' THEN
        SELECT version, scope_kind, person_user_id, role_key,
               origin_module_key, capability_module_key, capability_key,
               provider_key, provider_model_id, meter_key, currency_code,
               currency_exponent, period, limit_value, enforcement
        INTO stored_rule_version, stored_scope_kind, stored_person_user_id,
             stored_role_key, stored_origin_module_key,
             stored_capability_module_key, stored_capability_key,
             stored_provider_key, stored_provider_model_id, stored_meter_key,
             stored_currency_code, stored_currency_exponent, stored_period,
             stored_limit_value, stored_enforcement
        FROM agent_limit_rules
        WHERE id = NEW.campus_rule_id
          AND tenant_id = NEW.tenant_id
          AND deleted_at IS NULL
          AND effective_from <= STATEMENT_TIMESTAMP();

        expected_scope_value := CASE stored_scope_kind
            WHEN 'campus' THEN NEW.tenant_id::TEXT
            WHEN 'person' THEN stored_person_user_id::TEXT
            WHEN 'role' THEN stored_role_key
            WHEN 'origin_module' THEN stored_origin_module_key
            WHEN 'capability_module' THEN stored_capability_module_key
            WHEN 'capability' THEN stored_capability_key
            WHEN 'provider' THEN stored_provider_key
            WHEN 'model' THEN stored_provider_model_id
        END;

        IF NOT FOUND
           OR stored_enforcement <> 'hard'
           OR stored_rule_version <> NEW.definition_version
           OR stored_scope_kind <> NEW.scope_kind
           OR stored_meter_key <> NEW.meter_key
           OR stored_currency_code IS DISTINCT FROM NEW.currency_code
           OR stored_currency_exponent IS DISTINCT FROM NEW.currency_exponent
           OR stored_period <> NEW.period
           OR stored_limit_value <> NEW.limit_value
           OR expected_scope_value <> NEW.scope_value
           OR (
               stored_scope_kind = 'person'
               AND stored_person_user_id <> reservation_actor_user_id
           )
           OR (
               stored_scope_kind = 'role'
               AND NOT (stored_role_key = ANY(reservation_role_keys))
           )
           OR (
               stored_scope_kind = 'origin_module'
               AND stored_origin_module_key <> reservation_origin_module_key
           )
           OR (
               stored_scope_kind = 'capability_module'
               AND stored_capability_module_key IS DISTINCT FROM
                   reservation_capability_module_key
           )
           OR (
               stored_scope_kind = 'capability'
               AND stored_capability_key IS DISTINCT FROM reservation_capability_key
           )
           OR (
               stored_scope_kind = 'provider'
               AND stored_provider_key IS DISTINCT FROM reservation_provider_key
           )
           OR (
               stored_scope_kind = 'model'
               AND (
                   stored_provider_key IS DISTINCT FROM reservation_provider_key
                   OR stored_provider_model_id IS DISTINCT FROM
                       reservation_provider_model_id
               )
           ) THEN
            RAISE EXCEPTION 'Agent limit item must snapshot a matching local tightening rule';
        END IF;

        SELECT campus_rule_id, meter_key, currency_code, currency_exponent,
               period, period_start, period_end, committed_value, reserved_value
        INTO stored_bucket_rule_id, stored_meter_key, stored_currency_code,
             stored_currency_exponent, stored_period, bucket_period_start,
             bucket_period_end, bucket_committed_value, bucket_reserved_value
        FROM agent_limit_buckets
        WHERE id = NEW.bucket_id
          AND tenant_id = NEW.tenant_id
          AND deleted_at IS NULL;

        IF NOT FOUND
           OR stored_bucket_rule_id <> NEW.campus_rule_id
           OR stored_meter_key <> NEW.meter_key
           OR stored_currency_code IS DISTINCT FROM NEW.currency_code
           OR stored_currency_exponent IS DISTINCT FROM NEW.currency_exponent
           OR stored_period <> NEW.period
           OR bucket_period_start <> NEW.period_start
           OR bucket_period_end IS DISTINCT FROM NEW.period_end
           OR bucket_committed_value <> NEW.committed_before
           OR bucket_reserved_value <>
               NEW.reserved_before + NEW.reserved_amount THEN
            RAISE EXCEPTION 'Agent limit item must match its locked local bucket';
        END IF;
    ELSE
        SELECT source_lease_id, unit, period, limit_value, enforcement
        INTO stored_source_lease_id, stored_unit, stored_period,
             stored_limit_value, stored_enforcement
        FROM entitlement_limits
        WHERE tenant_id = NEW.tenant_id
          AND limit_key = NEW.entitlement_limit_key;

        IF NOT FOUND
           OR stored_enforcement <> 'hard'
           OR stored_source_lease_id <> NEW.source_lease_id
           OR NEW.entitlement_limit_key <> NEW.meter_key
           OR stored_unit <> NEW.unit
           OR stored_period <> NEW.period
           OR stored_limit_value <> NEW.limit_value
           OR NEW.scope_kind <> 'campus'
           OR NEW.scope_value <> NEW.tenant_id::TEXT THEN
            RAISE EXCEPTION 'Agent limit item must snapshot its current signed entitlement';
        END IF;

        SELECT limit_key, period_start, period_end, committed_value, reserved_value
        INTO stored_bucket_limit_key, bucket_period_start, bucket_period_end,
             bucket_committed_value, bucket_reserved_value
        FROM entitlement_meter_buckets
        WHERE id = NEW.entitlement_bucket_id
          AND tenant_id = NEW.tenant_id
          AND deleted_at IS NULL;

        IF NOT FOUND
           OR stored_bucket_limit_key <> NEW.entitlement_limit_key
           OR bucket_period_start <> NEW.period_start
           OR bucket_period_end IS DISTINCT FROM NEW.period_end
           OR bucket_committed_value <> NEW.committed_before
           OR bucket_reserved_value <>
               NEW.reserved_before + NEW.reserved_amount THEN
            RAISE EXCEPTION 'Agent signed limit item must map the canonical entitlement bucket';
        END IF;

        IF NEW.entitlement_reservation_id IS NOT NULL THEN
            SELECT status, bucket_id, source_lease_id, limit_key, unit, amount
            INTO source_reservation_status, source_reservation_bucket_id,
                 source_reservation_lease_id, source_reservation_limit_key,
                 source_reservation_unit, source_reservation_amount
            FROM entitlement_usage_reservations
            WHERE id = NEW.entitlement_reservation_id
              AND tenant_id = NEW.tenant_id
              AND deleted_at IS NULL;

            IF NOT FOUND
               OR source_reservation_status <> 'reserved'
               OR source_reservation_bucket_id <> NEW.entitlement_bucket_id
               OR source_reservation_lease_id <> NEW.source_lease_id
               OR source_reservation_limit_key <> NEW.entitlement_limit_key
               OR source_reservation_unit <> NEW.unit
               OR source_reservation_amount <> NEW.requested_amount THEN
                RAISE EXCEPTION 'Agent signed limit item must map a canonical reservation';
            END IF;
        END IF;
    END IF;

    IF NEW.decision = 'denied'
       AND NEW.committed_before + NEW.reserved_before + NEW.requested_amount
           <= NEW.limit_value THEN
        RAISE EXCEPTION 'Agent limit denial must prove exhausted capacity';
    END IF;
    IF NEW.decision = 'allowed'
       AND NEW.committed_before + NEW.reserved_before + NEW.requested_amount
           > NEW.limit_value THEN
        RAISE EXCEPTION 'Agent limit allowance exceeds snapshotted capacity';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_limit_reservation_items_validate_insert
    ON agent_limit_reservation_items;
CREATE TRIGGER agent_limit_reservation_items_validate_insert
    BEFORE INSERT ON agent_limit_reservation_items
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_limit_reservation_item_insert();

CREATE OR REPLACE FUNCTION reject_agent_usage_immutable_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION '% is append-only', TG_TABLE_NAME;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_limit_reservation_items_reject_mutation
    ON agent_limit_reservation_items;
CREATE TRIGGER agent_limit_reservation_items_reject_mutation
    BEFORE UPDATE OR DELETE ON agent_limit_reservation_items
    FOR EACH ROW
    EXECUTE FUNCTION reject_agent_usage_immutable_mutation();

CREATE OR REPLACE FUNCTION validate_agent_limit_reconciliation_insert()
RETURNS TRIGGER AS $$
DECLARE
    reservation_status TEXT;
    reservation_stage_kind TEXT;
    reservation_provider_attempt_id UUID;
    reservation_capability_call_id UUID;
    reservation_claimed_at TIMESTAMPTZ;
    item_definition_kind TEXT;
    item_bucket_id UUID;
    item_entitlement_bucket_id UUID;
    item_entitlement_reservation_id UUID;
    item_meter_key TEXT;
    item_currency_code TEXT;
    item_currency_exponent SMALLINT;
    item_pricing_version TEXT;
    item_requested_amount BIGINT;
    item_reserved_amount BIGINT;
    item_decision TEXT;
    source_status TEXT;
    source_failure_origin TEXT;
    source_amount BIGINT;
    expected_amount BIGINT;
    expected_basis TEXT;
    canonical_status TEXT;
    canonical_event_amount BIGINT;
BEGIN
    IF NEW.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'Agent limit reconciliations must start active';
    END IF;

    SELECT status, stage_kind, provider_attempt_id, capability_call_id, claimed_at
    INTO reservation_status, reservation_stage_kind,
         reservation_provider_attempt_id, reservation_capability_call_id,
         reservation_claimed_at
    FROM agent_limit_reservations
    WHERE id = NEW.reservation_id
      AND tenant_id = NEW.tenant_id
      AND run_id = NEW.run_id;

    SELECT definition_kind, bucket_id, entitlement_bucket_id,
           entitlement_reservation_id, meter_key, currency_code,
           currency_exponent, pricing_version, requested_amount,
           reserved_amount, decision
    INTO item_definition_kind, item_bucket_id, item_entitlement_bucket_id,
         item_entitlement_reservation_id, item_meter_key, item_currency_code,
         item_currency_exponent, item_pricing_version, item_requested_amount,
         item_reserved_amount, item_decision
    FROM agent_limit_reservation_items
    WHERE id = NEW.reservation_item_id
      AND tenant_id = NEW.tenant_id
      AND reservation_id = NEW.reservation_id
      AND run_id = NEW.run_id;

    IF reservation_status IS DISTINCT FROM 'reserved'
       OR item_decision IS DISTINCT FROM 'allowed'
       OR item_reserved_amount IS DISTINCT FROM item_requested_amount
       OR item_reserved_amount <= 0
       OR NEW.committed_amount > item_reserved_amount
       OR (
           reservation_stage_kind IN ('provider_attempt', 'capability_call')
           AND reservation_claimed_at IS NULL
       ) THEN
        RAISE EXCEPTION 'Agent limit reconciliation requires a claimed allowed reservation item';
    END IF;

    IF reservation_stage_kind = 'run' THEN
        SELECT status INTO source_status
        FROM agent_runs
        WHERE id = NEW.run_id AND tenant_id = NEW.tenant_id;
        IF source_status NOT IN ('completed', 'failed', 'cancelled', 'interrupted')
           OR item_meter_key <> 'agent.runs' THEN
            RAISE EXCEPTION 'Agent run reconciliation requires a terminal run count';
        END IF;
        expected_amount := 1;
        expected_basis := 'exact';
    ELSIF reservation_stage_kind = 'capability_call' THEN
        SELECT status INTO source_status
        FROM agent_capability_calls
        WHERE id = reservation_capability_call_id
          AND tenant_id = NEW.tenant_id
          AND run_id = NEW.run_id;
        IF source_status IS NULL OR source_status = 'running'
           OR item_meter_key <> 'agent.capability_calls' THEN
            RAISE EXCEPTION 'Agent capability reconciliation requires a terminal call count';
        END IF;
        expected_amount := 1;
        expected_basis := 'exact';
    ELSE
        SELECT status, failure_origin,
               CASE item_meter_key
                   WHEN 'agent.provider_attempts' THEN 1
                   WHEN 'agent.input_tokens' THEN input_tokens
                   WHEN 'agent.output_tokens' THEN output_tokens
                   WHEN 'agent.estimated_cost' THEN estimated_cost_amount
               END
        INTO source_status, source_failure_origin, source_amount
        FROM agent_provider_attempts
        WHERE id = reservation_provider_attempt_id
          AND tenant_id = NEW.tenant_id
          AND run_id = NEW.run_id;

        IF source_status IS NULL OR source_status = 'running'
           OR item_meter_key NOT IN (
               'agent.provider_attempts', 'agent.input_tokens',
               'agent.output_tokens', 'agent.estimated_cost'
           ) THEN
            RAISE EXCEPTION 'Agent provider reconciliation requires a terminal attempt meter';
        END IF;

        IF item_meter_key = 'agent.provider_attempts' THEN
            expected_amount := 1;
            expected_basis := 'exact';
        ELSIF source_amount IS NOT NULL THEN
            expected_amount := source_amount;
            expected_basis := CASE item_meter_key
                WHEN 'agent.estimated_cost' THEN 'estimated'
                ELSE 'exact'
            END;
        ELSIF source_failure_origin = 'preflight' THEN
            expected_amount := 0;
            expected_basis := CASE item_meter_key
                WHEN 'agent.estimated_cost' THEN 'estimated'
                ELSE 'exact'
            END;
        ELSE
            expected_amount := item_reserved_amount;
            expected_basis := 'upper_bound';
        END IF;
    END IF;

    IF NEW.committed_amount <> expected_amount
       OR NEW.enforcement_basis <> expected_basis THEN
        RAISE EXCEPTION 'Agent limit reconciliation must match terminal usage evidence';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM agent_limit_reconciliations AS reconciliation
        INNER JOIN agent_limit_reservation_items AS compared_item
          ON compared_item.id = reconciliation.reservation_item_id
         AND compared_item.tenant_id = reconciliation.tenant_id
        WHERE reconciliation.reservation_id = NEW.reservation_id
          AND reconciliation.tenant_id = NEW.tenant_id
          AND compared_item.meter_key = item_meter_key
          AND compared_item.currency_code IS NOT DISTINCT FROM item_currency_code
          AND compared_item.currency_exponent IS NOT DISTINCT FROM item_currency_exponent
          AND compared_item.pricing_version IS NOT DISTINCT FROM item_pricing_version
          AND (
              reconciliation.committed_amount <> NEW.committed_amount
              OR reconciliation.enforcement_basis <> NEW.enforcement_basis
          )
    ) THEN
        RAISE EXCEPTION 'overlapping Agent hard limits must reconcile identically';
    END IF;

    IF item_definition_kind = 'local_rule' THEN
        IF NOT EXISTS (
            SELECT 1 FROM agent_limit_buckets
            WHERE id = item_bucket_id AND tenant_id = NEW.tenant_id
              AND xmin = (PG_CURRENT_XACT_ID()::TEXT)::xid
        ) THEN
            RAISE EXCEPTION 'local Agent reconciliation requires same-transaction counter movement';
        END IF;
    ELSE
        SELECT status INTO canonical_status
        FROM entitlement_usage_reservations
        WHERE id = item_entitlement_reservation_id
          AND tenant_id = NEW.tenant_id
          AND bucket_id = item_entitlement_bucket_id
          AND xmin = (PG_CURRENT_XACT_ID()::TEXT)::xid;
        SELECT amount INTO canonical_event_amount
        FROM entitlement_usage_events
        WHERE reservation_id = item_entitlement_reservation_id
          AND tenant_id = NEW.tenant_id
          AND xmin = (PG_CURRENT_XACT_ID()::TEXT)::xid;

        IF NOT EXISTS (
            SELECT 1 FROM entitlement_meter_buckets
            WHERE id = item_entitlement_bucket_id AND tenant_id = NEW.tenant_id
              AND xmin = (PG_CURRENT_XACT_ID()::TEXT)::xid
        ) OR (
            NEW.committed_amount > 0
            AND (
                canonical_status IS DISTINCT FROM 'committed'
                OR canonical_event_amount IS DISTINCT FROM NEW.committed_amount
            )
        ) OR (
            NEW.committed_amount = 0
            AND (
                canonical_status IS DISTINCT FROM 'released'
                OR canonical_event_amount IS NOT NULL
            )
        ) THEN
            RAISE EXCEPTION 'signed Agent reconciliation must match canonical quota evidence';
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_limit_reconciliations_validate_insert
    ON agent_limit_reconciliations;
CREATE TRIGGER agent_limit_reconciliations_validate_insert
    BEFORE INSERT ON agent_limit_reconciliations
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_limit_reconciliation_insert();

DROP TRIGGER IF EXISTS agent_limit_reconciliations_reject_mutation
    ON agent_limit_reconciliations;
CREATE TRIGGER agent_limit_reconciliations_reject_mutation
    BEFORE UPDATE OR DELETE ON agent_limit_reconciliations
    FOR EACH ROW
    EXECUTE FUNCTION reject_agent_usage_immutable_mutation();

CREATE OR REPLACE FUNCTION validate_agent_limit_reconciliation_parent()
RETURNS TRIGGER AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM agent_limit_reservations
        WHERE id = NEW.reservation_id
          AND tenant_id = NEW.tenant_id
          AND run_id = NEW.run_id
          AND status = 'committed'
    ) THEN
        RAISE EXCEPTION 'Agent limit reconciliation requires a committed parent at transaction end';
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_limit_reconciliations_parent_constraint
    ON agent_limit_reconciliations;
CREATE CONSTRAINT TRIGGER agent_limit_reconciliations_parent_constraint
    AFTER INSERT ON agent_limit_reconciliations
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_limit_reconciliation_parent();

CREATE OR REPLACE FUNCTION validate_agent_usage_event_insert()
RETURNS TRIGGER AS $$
DECLARE
    stored_thread_id UUID;
    stored_actor_user_id UUID;
    stored_origin_module_key TEXT;
    stored_task_class TEXT;
    stored_request_id UUID;
    stored_correlation_id UUID;
    stored_run_status TEXT;
    stored_source_status TEXT;
    stored_turn_index SMALLINT;
    stored_attempt_index SMALLINT;
    stored_connection_id UUID;
    stored_provider_key TEXT;
    stored_provider_model_id TEXT;
    stored_model_snapshot_id UUID;
    stored_route_priority SMALLINT;
    stored_failure_origin TEXT;
    stored_failure_category TEXT;
    stored_capability_module_key TEXT;
    stored_capability_key TEXT;
    stored_capability_version INTEGER;
    stored_approval_state TEXT;
    stored_reservation_status TEXT;
    stored_reservation_stage_kind TEXT;
    stored_reservation_stage_sequence SMALLINT;
    stored_reservation_provider_attempt_id UUID;
    stored_reservation_capability_call_id UUID;
    stored_reservation_claimed_at TIMESTAMPTZ;
BEGIN
    IF NEW.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'Agent usage events must start active';
    END IF;

    SELECT thread_id, requested_by, origin_module_key, task_class,
           request_id, correlation_id, status
    INTO stored_thread_id, stored_actor_user_id, stored_origin_module_key,
         stored_task_class, stored_request_id, stored_correlation_id,
         stored_run_status
    FROM agent_runs
    WHERE id = NEW.run_id
      AND tenant_id = NEW.tenant_id;

    IF NOT FOUND
       OR stored_thread_id <> NEW.thread_id
       OR stored_actor_user_id <> NEW.actor_user_id
       OR stored_origin_module_key <> NEW.origin_module_key
       OR stored_task_class <> NEW.task_class
       OR stored_request_id <> NEW.request_id
       OR stored_correlation_id <> NEW.correlation_id THEN
        RAISE EXCEPTION 'Agent usage event identity must match its same-tenant run';
    END IF;

    IF NEW.event_kind = 'run' THEN
        IF stored_run_status NOT IN ('completed', 'failed', 'cancelled', 'interrupted')
           OR (stored_run_status = 'completed' AND NEW.outcome <> 'succeeded')
           OR (stored_run_status = 'failed' AND NEW.outcome NOT IN ('failed', 'denied'))
           OR (stored_run_status = 'cancelled' AND NEW.outcome <> 'cancelled')
           OR (stored_run_status = 'interrupted' AND NEW.outcome <> 'interrupted') THEN
            RAISE EXCEPTION 'Agent run usage requires a matching terminal run';
        END IF;
    ELSIF NEW.event_kind = 'provider_attempt' THEN
        SELECT attempt.status, attempt.turn_index, attempt.attempt_index,
               attempt.connection_id, attempt.provider_key,
               attempt.provider_model_id, attempt.model_snapshot_id,
               route.priority, attempt.failure_origin, attempt.failure_category
        INTO stored_source_status, stored_turn_index, stored_attempt_index,
             stored_connection_id, stored_provider_key,
             stored_provider_model_id, stored_model_snapshot_id,
             stored_route_priority, stored_failure_origin, stored_failure_category
        FROM agent_provider_attempts AS attempt
        INNER JOIN ai_task_routes AS route
          ON route.id = attempt.route_target_id
         AND route.tenant_id = attempt.tenant_id
        WHERE attempt.id = NEW.provider_attempt_id
          AND attempt.tenant_id = NEW.tenant_id
          AND attempt.run_id = NEW.run_id;

        IF NOT FOUND
           OR stored_source_status = 'running'
           OR stored_turn_index <> NEW.provider_turn_index
           OR stored_attempt_index <> NEW.provider_attempt_index
           OR stored_connection_id <> NEW.provider_connection_id
           OR stored_provider_key <> NEW.provider_key
           OR stored_provider_model_id <> NEW.provider_model_id
           OR stored_model_snapshot_id <> NEW.provider_model_snapshot_id
           OR stored_route_priority <> NEW.route_priority
           OR stored_failure_origin IS DISTINCT FROM NEW.failure_origin
           OR stored_failure_category IS DISTINCT FROM NEW.failure_category
           OR (stored_source_status = 'succeeded' AND NEW.outcome <> 'succeeded')
           OR (stored_source_status = 'failed' AND NEW.outcome <> 'failed')
           OR (stored_source_status = 'cancelled' AND NEW.outcome <> 'cancelled')
           OR (stored_source_status = 'interrupted' AND NEW.outcome <> 'interrupted') THEN
            RAISE EXCEPTION 'Agent provider usage must match its terminal attempt';
        END IF;
    ELSE
        SELECT status, owning_module_key, capability_key,
               capability_version, approval_state
        INTO stored_source_status, stored_capability_module_key,
             stored_capability_key, stored_capability_version,
             stored_approval_state
        FROM agent_capability_calls
        WHERE id = NEW.capability_call_id
          AND tenant_id = NEW.tenant_id
          AND run_id = NEW.run_id;

        IF NOT FOUND
           OR stored_source_status = 'running'
           OR stored_capability_module_key <> NEW.capability_module_key
           OR stored_capability_key <> NEW.capability_key
           OR stored_capability_version <> NEW.capability_version
           OR stored_approval_state <> NEW.approval_state
           OR (stored_source_status = 'succeeded' AND NEW.outcome <> 'succeeded')
           OR (stored_source_status = 'failed' AND NEW.outcome <> 'failed')
           OR (stored_source_status = 'denied' AND NEW.outcome <> 'denied')
           OR (stored_source_status = 'cancelled' AND NEW.outcome <> 'cancelled')
           OR (stored_source_status = 'interrupted' AND NEW.outcome <> 'interrupted') THEN
            RAISE EXCEPTION 'Agent capability usage must match its terminal call';
        END IF;
    END IF;

    IF NEW.limit_reservation_id IS NOT NULL THEN
        SELECT status, stage_kind, stage_sequence, provider_attempt_id,
               capability_call_id, claimed_at
        INTO stored_reservation_status, stored_reservation_stage_kind,
             stored_reservation_stage_sequence,
             stored_reservation_provider_attempt_id,
             stored_reservation_capability_call_id,
             stored_reservation_claimed_at
        FROM agent_limit_reservations
        WHERE id = NEW.limit_reservation_id
          AND tenant_id = NEW.tenant_id
          AND run_id = NEW.run_id;

        IF NOT FOUND
           OR stored_reservation_status = 'preparing'
           OR (
               stored_reservation_status = 'denied'
               AND NEW.outcome <> 'denied'
           )
           OR (
               stored_reservation_status NOT IN (
                   'denied', 'not_limited', 'committed'
               )
           )
           OR (
               NEW.event_kind = 'run'
               AND stored_reservation_stage_kind <> 'run'
           )
           OR (
               NEW.event_kind = 'provider_attempt'
               AND (
                   stored_reservation_stage_kind <> 'provider_attempt'
                   OR stored_reservation_provider_attempt_id <>
                       NEW.provider_attempt_id
                   OR stored_reservation_claimed_at IS NULL
                   OR stored_reservation_stage_sequence <>
                       ((NEW.provider_turn_index - 1) * 3 + NEW.provider_attempt_index)
               )
           )
           OR (
               NEW.event_kind = 'capability_call'
               AND (
                   stored_reservation_stage_kind <> 'capability_call'
                   OR stored_reservation_capability_call_id <>
                       NEW.capability_call_id
                   OR stored_reservation_claimed_at IS NULL
                   OR stored_reservation_stage_sequence IS DISTINCT FROM (
                       SELECT call_sequence
                       FROM agent_capability_calls
                       WHERE id = NEW.capability_call_id
                         AND tenant_id = NEW.tenant_id
                         AND run_id = NEW.run_id
                   )
               )
           ) THEN
            RAISE EXCEPTION 'Agent usage event requires a compatible terminal limit decision';
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_usage_events_validate_insert ON agent_usage_events;
CREATE TRIGGER agent_usage_events_validate_insert
    BEFORE INSERT ON agent_usage_events
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_usage_event_insert();

DROP TRIGGER IF EXISTS agent_usage_events_reject_mutation ON agent_usage_events;
CREATE TRIGGER agent_usage_events_reject_mutation
    BEFORE UPDATE OR DELETE ON agent_usage_events
    FOR EACH ROW
    EXECUTE FUNCTION reject_agent_usage_immutable_mutation();

CREATE OR REPLACE FUNCTION validate_agent_usage_measure_insert()
RETURNS TRIGGER AS $$
DECLARE
    stored_event_kind TEXT;
    stored_provider_attempt_id UUID;
    stored_limit_reservation_id UUID;
    stored_event_outcome TEXT;
    stored_amount BIGINT;
    stored_currency_code TEXT;
    stored_currency_exponent SMALLINT;
    stored_pricing_version TEXT;
    stored_reconciliation_amount BIGINT;
    stored_reconciliation_basis TEXT;
BEGIN
    IF NEW.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'Agent usage measures must start active';
    END IF;

    SELECT event_kind, provider_attempt_id, limit_reservation_id, outcome
    INTO stored_event_kind, stored_provider_attempt_id,
         stored_limit_reservation_id, stored_event_outcome
    FROM agent_usage_events
    WHERE id = NEW.usage_event_id
      AND tenant_id = NEW.tenant_id;

    IF NOT FOUND
       OR (stored_event_kind = 'run' AND NEW.meter_key <> 'agent.runs')
       OR (
           stored_event_kind = 'capability_call'
           AND NEW.meter_key <> 'agent.capability_calls'
       )
       OR (
           stored_event_kind = 'provider_attempt'
           AND NEW.meter_key NOT IN (
               'agent.provider_attempts',
               'agent.input_tokens',
               'agent.output_tokens',
               'agent.cached_input_tokens',
               'agent.reasoning_tokens',
               'agent.provider_reported_cost',
               'agent.estimated_cost'
           )
       ) THEN
        RAISE EXCEPTION 'Agent usage measure does not belong to its event kind';
    END IF;

    IF NEW.meter_key IN (
        'agent.runs', 'agent.provider_attempts', 'agent.capability_calls'
    ) AND (
        NEW.amount IS DISTINCT FROM 1
        OR (NEW.enforcement_amount IS NOT NULL AND NEW.enforcement_amount <> 1)
    ) THEN
        RAISE EXCEPTION 'Agent usage count measures must equal one';
    END IF;

    IF stored_event_kind = 'provider_attempt'
       AND NEW.meter_key <> 'agent.provider_attempts' THEN
        SELECT
            CASE NEW.meter_key
                WHEN 'agent.input_tokens' THEN attempt.input_tokens
                WHEN 'agent.output_tokens' THEN attempt.output_tokens
                WHEN 'agent.cached_input_tokens' THEN attempt.cached_tokens
                WHEN 'agent.reasoning_tokens' THEN attempt.reasoning_tokens
                WHEN 'agent.provider_reported_cost' THEN
                    attempt.provider_reported_cost_amount
                WHEN 'agent.estimated_cost' THEN attempt.estimated_cost_amount
            END,
            CASE NEW.meter_key
                WHEN 'agent.provider_reported_cost' THEN
                    attempt.provider_reported_cost_currency
                WHEN 'agent.estimated_cost' THEN attempt.estimated_cost_currency
            END,
            CASE NEW.meter_key
                WHEN 'agent.provider_reported_cost' THEN
                    attempt.provider_reported_cost_exponent
                WHEN 'agent.estimated_cost' THEN attempt.estimated_cost_exponent
            END,
            CASE NEW.meter_key
                WHEN 'agent.provider_reported_cost' THEN
                    attempt.provider_reported_pricing_version
                WHEN 'agent.estimated_cost' THEN attempt.estimated_pricing_version
            END
        INTO stored_amount, stored_currency_code, stored_currency_exponent,
             stored_pricing_version
        FROM agent_provider_attempts AS attempt
        WHERE attempt.id = stored_provider_attempt_id
          AND attempt.tenant_id = NEW.tenant_id;

        IF NOT FOUND OR NEW.amount IS DISTINCT FROM stored_amount THEN
            RAISE EXCEPTION 'Agent usage amount must match its provider attempt';
        END IF;

        IF NEW.meter_key = 'agent.provider_reported_cost'
           AND (
               NEW.currency_code IS DISTINCT FROM stored_currency_code
               OR NEW.currency_exponent IS DISTINCT FROM stored_currency_exponent
               OR NEW.pricing_version IS DISTINCT FROM stored_pricing_version
           ) THEN
            RAISE EXCEPTION 'Provider-reported money must retain its source tuple';
        END IF;

        IF NEW.meter_key = 'agent.estimated_cost'
           AND stored_amount IS NOT NULL
           AND (
               NEW.currency_code IS DISTINCT FROM stored_currency_code
               OR NEW.currency_exponent IS DISTINCT FROM stored_currency_exponent
               OR NEW.pricing_version IS DISTINCT FROM stored_pricing_version
           ) THEN
            RAISE EXCEPTION 'Estimated money must retain its source tuple';
        END IF;
    END IF;

    SELECT reconciliation.committed_amount, reconciliation.enforcement_basis
    INTO stored_reconciliation_amount, stored_reconciliation_basis
    FROM agent_limit_reconciliations AS reconciliation
    INNER JOIN agent_limit_reservation_items AS item
      ON item.id = reconciliation.reservation_item_id
     AND item.tenant_id = reconciliation.tenant_id
     AND item.reservation_id = reconciliation.reservation_id
     AND item.run_id = reconciliation.run_id
    WHERE reconciliation.reservation_id = stored_limit_reservation_id
      AND reconciliation.tenant_id = NEW.tenant_id
      AND item.meter_key = NEW.meter_key
      AND item.currency_code IS NOT DISTINCT FROM NEW.currency_code
      AND item.currency_exponent IS NOT DISTINCT FROM NEW.currency_exponent
      AND item.pricing_version IS NOT DISTINCT FROM NEW.pricing_version
    LIMIT 1;

    IF stored_event_outcome = 'denied'
       AND (
           NEW.enforcement_amount IS NOT NULL
           OR NEW.enforcement_basis IS NOT NULL
       ) THEN
        RAISE EXCEPTION 'denied Agent usage cannot claim committed enforcement';
    END IF;
    IF FOUND AND (
        NEW.enforcement_amount IS DISTINCT FROM stored_reconciliation_amount
        OR NEW.enforcement_basis IS DISTINCT FROM stored_reconciliation_basis
    ) THEN
        RAISE EXCEPTION 'Agent usage enforcement must match its reconciliation';
    END IF;
    IF NOT FOUND AND (
        NEW.enforcement_amount IS NOT NULL
        OR NEW.enforcement_basis IS NOT NULL
    ) THEN
        RAISE EXCEPTION 'unreconciled Agent usage cannot claim enforcement';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_usage_measures_validate_insert ON agent_usage_measures;
CREATE TRIGGER agent_usage_measures_validate_insert
    BEFORE INSERT ON agent_usage_measures
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_usage_measure_insert();

DROP TRIGGER IF EXISTS agent_usage_measures_reject_mutation ON agent_usage_measures;
CREATE TRIGGER agent_usage_measures_reject_mutation
    BEFORE UPDATE OR DELETE ON agent_usage_measures
    FOR EACH ROW
    EXECUTE FUNCTION reject_agent_usage_immutable_mutation();

CREATE OR REPLACE FUNCTION validate_agent_usage_measure_set()
RETURNS TRIGGER AS $$
DECLARE
    stored_event_kind TEXT;
    stored_meter_keys TEXT[];
BEGIN
    SELECT event_kind
    INTO stored_event_kind
    FROM agent_usage_events
    WHERE id = NEW.id
      AND tenant_id = NEW.tenant_id;

    SELECT ARRAY_AGG(meter_key ORDER BY meter_key)
    INTO stored_meter_keys
    FROM agent_usage_measures
    WHERE usage_event_id = NEW.id
      AND tenant_id = NEW.tenant_id;

    IF stored_event_kind = 'run'
       AND stored_meter_keys <> ARRAY['agent.runs']::TEXT[] THEN
        RAISE EXCEPTION 'Agent run usage requires its canonical count measure';
    ELSIF stored_event_kind = 'capability_call'
       AND stored_meter_keys <> ARRAY['agent.capability_calls']::TEXT[] THEN
        RAISE EXCEPTION 'Agent capability usage requires its canonical count measure';
    ELSIF stored_event_kind = 'provider_attempt'
       AND stored_meter_keys <> ARRAY[
           'agent.cached_input_tokens',
           'agent.estimated_cost',
           'agent.input_tokens',
           'agent.output_tokens',
           'agent.provider_attempts',
           'agent.provider_reported_cost',
           'agent.reasoning_tokens'
       ]::TEXT[] THEN
        RAISE EXCEPTION 'Agent provider usage requires every canonical nullable measure';
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_usage_events_measure_set_constraint
    ON agent_usage_events;
CREATE CONSTRAINT TRIGGER agent_usage_events_measure_set_constraint
    AFTER INSERT ON agent_usage_events
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_usage_measure_set();

CREATE OR REPLACE FUNCTION validate_agent_usage_denial_audit()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.outcome = 'denied' AND NOT EXISTS (
        SELECT 1
        FROM actor_audit_events
        WHERE tenant_id = NEW.tenant_id
          AND agent_run_id = NEW.run_id
          AND actor_type = 'agent'
          AND actor_user_id = NEW.actor_user_id
          AND outcome = 'denied'
          AND request_id = NEW.request_id
          AND correlation_id = NEW.correlation_id
          AND xmin = (PG_CURRENT_XACT_ID()::TEXT)::xid
    ) THEN
        RAISE EXCEPTION 'Denied Agent usage requires same-transaction actor audit evidence';
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_usage_events_denial_audit_constraint
    ON agent_usage_events;
CREATE CONSTRAINT TRIGGER agent_usage_events_denial_audit_constraint
    AFTER INSERT ON agent_usage_events
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_usage_denial_audit();

DROP TRIGGER IF EXISTS agent_limit_rules_reject_truncate ON agent_limit_rules;
CREATE TRIGGER agent_limit_rules_reject_truncate
    BEFORE TRUNCATE ON agent_limit_rules
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_agent_usage_immutable_mutation();

DROP TRIGGER IF EXISTS agent_limit_buckets_reject_truncate ON agent_limit_buckets;
CREATE TRIGGER agent_limit_buckets_reject_truncate
    BEFORE TRUNCATE ON agent_limit_buckets
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_agent_usage_immutable_mutation();

DROP TRIGGER IF EXISTS agent_limit_reservations_reject_truncate
    ON agent_limit_reservations;
CREATE TRIGGER agent_limit_reservations_reject_truncate
    BEFORE TRUNCATE ON agent_limit_reservations
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_agent_usage_immutable_mutation();

DROP TRIGGER IF EXISTS agent_limit_reservation_items_reject_truncate
    ON agent_limit_reservation_items;
CREATE TRIGGER agent_limit_reservation_items_reject_truncate
    BEFORE TRUNCATE ON agent_limit_reservation_items
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_agent_usage_immutable_mutation();

DROP TRIGGER IF EXISTS agent_limit_reconciliations_reject_truncate
    ON agent_limit_reconciliations;
CREATE TRIGGER agent_limit_reconciliations_reject_truncate
    BEFORE TRUNCATE ON agent_limit_reconciliations
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_agent_usage_immutable_mutation();

DROP TRIGGER IF EXISTS agent_usage_events_reject_truncate ON agent_usage_events;
CREATE TRIGGER agent_usage_events_reject_truncate
    BEFORE TRUNCATE ON agent_usage_events
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_agent_usage_immutable_mutation();

DROP TRIGGER IF EXISTS agent_usage_measures_reject_truncate ON agent_usage_measures;
CREATE TRIGGER agent_usage_measures_reject_truncate
    BEFORE TRUNCATE ON agent_usage_measures
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_agent_usage_immutable_mutation();
