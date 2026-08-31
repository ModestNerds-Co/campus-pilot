-- Owns terminal evidence for capability intent rejected during broker preparation.
-- Rejections are deliberately separate from executable capability calls: unknown
-- operation, normalized input, or scope evidence remains NULL and is never fabricated.

-- This internal fence owns only cross-ledger identity and disposition. Both the
-- executable and rejected ledgers claim it before insert, so concurrent writers
-- cannot win in separate tables for the same call ID or run sequence.
CREATE TABLE IF NOT EXISTS agent_capability_intent_registry (
    capability_call_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    run_id UUID NOT NULL,
    call_sequence SMALLINT NOT NULL CHECK (call_sequence BETWEEN 1 AND 16),
    disposition TEXT NOT NULL CHECK (disposition IN ('executable', 'rejected')),
    deleted_at TIMESTAMPTZ CHECK (deleted_at IS NULL),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_capability_intent_registry_run_sequence_unique
        UNIQUE (run_id, call_sequence),
    CONSTRAINT agent_capability_intent_registry_run_tenant_fk
        FOREIGN KEY (run_id, tenant_id)
        REFERENCES agent_runs(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_capability_intent_registry_immutable_time_check CHECK (
        updated_at = created_at
    )
);

CREATE INDEX IF NOT EXISTS idx_agent_capability_intent_registry_run
    ON agent_capability_intent_registry(tenant_id, run_id, call_sequence);

CREATE OR REPLACE FUNCTION protect_agent_capability_intent_registry()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' OR NEW IS DISTINCT FROM OLD THEN
        RAISE EXCEPTION 'Agent capability intent disposition is immutable';
    END IF;

    -- The registration function uses a no-op conflict update so PostgreSQL
    -- returns the winning row after waiting for a concurrent claimant.
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_capability_intent_registry_protect_mutation
    ON agent_capability_intent_registry;
CREATE TRIGGER agent_capability_intent_registry_protect_mutation
    BEFORE UPDATE OR DELETE ON agent_capability_intent_registry
    FOR EACH ROW
    EXECUTE FUNCTION protect_agent_capability_intent_registry();

DROP TRIGGER IF EXISTS agent_capability_intent_registry_reject_truncate
    ON agent_capability_intent_registry;
CREATE TRIGGER agent_capability_intent_registry_reject_truncate
    BEFORE TRUNCATE ON agent_capability_intent_registry
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_agent_append_only_mutation();

CREATE OR REPLACE FUNCTION register_agent_capability_intent(
    p_capability_call_id UUID,
    p_tenant_id UUID,
    p_run_id UUID,
    p_call_sequence SMALLINT,
    p_disposition TEXT
)
RETURNS VOID AS $$
DECLARE
    stored agent_capability_intent_registry%ROWTYPE;
BEGIN
    INSERT INTO agent_capability_intent_registry (
        capability_call_id, tenant_id, run_id, call_sequence, disposition
    ) VALUES (
        p_capability_call_id, p_tenant_id, p_run_id, p_call_sequence, p_disposition
    )
    ON CONFLICT (run_id, call_sequence) DO UPDATE
    SET capability_call_id = agent_capability_intent_registry.capability_call_id
    RETURNING * INTO stored;

    IF stored.capability_call_id IS DISTINCT FROM p_capability_call_id
       OR stored.tenant_id IS DISTINCT FROM p_tenant_id
       OR stored.run_id IS DISTINCT FROM p_run_id
       OR stored.call_sequence IS DISTINCT FROM p_call_sequence
       OR stored.disposition IS DISTINCT FROM p_disposition THEN
        RAISE EXCEPTION 'Agent capability intent disposition conflict';
    END IF;
END;
$$ LANGUAGE plpgsql;

-- Existing executable calls predate this fence. Backfill them once; replay is
-- a no-op and never changes a previously claimed disposition.
INSERT INTO agent_capability_intent_registry (
    capability_call_id, tenant_id, run_id, call_sequence, disposition
)
SELECT id, tenant_id, run_id, call_sequence, 'executable'
FROM agent_capability_calls
ON CONFLICT DO NOTHING;

CREATE TABLE IF NOT EXISTS agent_capability_rejections (
    capability_call_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    run_id UUID NOT NULL,
    call_sequence SMALLINT NOT NULL CHECK (call_sequence BETWEEN 1 AND 16),
    actor_user_id UUID NOT NULL,
    request_id UUID NOT NULL CHECK (request_id = capability_call_id),
    correlation_id UUID NOT NULL,
    claimed_by_worker_id TEXT NOT NULL CHECK (
        CHAR_LENGTH(BTRIM(claimed_by_worker_id)) BETWEEN 1 AND 120
        AND claimed_by_worker_id !~ '[[:cntrl:]]'
    ),
    claim_fence_version BIGINT NOT NULL CHECK (
        claim_fence_version BETWEEN 1 AND 9007199254740991
    ),
    capability_key TEXT NOT NULL CHECK (
        CHAR_LENGTH(capability_key) BETWEEN 1 AND 200
        AND capability_key = LOWER(BTRIM(capability_key))
        AND capability_key ~ '^[a-z][a-z0-9_.-]*$'
    ),
    capability_version INTEGER NOT NULL CHECK (capability_version > 0),
    normalized_input_digest_sha256 BYTEA CHECK (
        normalized_input_digest_sha256 IS NULL
        OR OCTET_LENGTH(normalized_input_digest_sha256) = 32
    ),
    product_operation_key TEXT CHECK (
        product_operation_key IS NULL
        OR (
            CHAR_LENGTH(product_operation_key) BETWEEN 1 AND 240
            AND product_operation_key = LOWER(BTRIM(product_operation_key))
            AND product_operation_key ~ '^[a-z][a-z0-9_.-]*$'
        )
    ),
    owning_module_key TEXT CHECK (
        owning_module_key IS NULL
        OR (
            CHAR_LENGTH(owning_module_key) BETWEEN 1 AND 160
            AND owning_module_key = LOWER(BTRIM(owning_module_key))
            AND owning_module_key ~ '^[a-z][a-z0-9_.-]*$'
        )
    ),
    required_permission TEXT CHECK (
        required_permission IS NULL
        OR (
            CHAR_LENGTH(required_permission) BETWEEN 3 AND 200
            AND required_permission = LOWER(BTRIM(required_permission))
            AND required_permission ~ '^[a-z][a-z0-9_.-]*:[a-z][a-z0-9_.-]*$'
        )
    ),
    scope_kind TEXT CHECK (
        scope_kind IS NULL OR scope_kind IN ('tenant_wide', 'resources')
    ),
    resource_references JSONB,
    outcome TEXT NOT NULL CHECK (outcome IN ('denied', 'failed')),
    broker_error_code TEXT NOT NULL CHECK (broker_error_code IN (
        'unknown_capability', 'unsupported_version', 'capability_unavailable',
        'approval_required', 'human_only', 'prohibited', 'authority_unavailable',
        'access_denied', 'invalid_input', 'input_too_large',
        'record_scope_denied', 'execution_failed', 'audit_unavailable'
    )),
    reason_code TEXT NOT NULL CHECK (
        CHAR_LENGTH(reason_code) BETWEEN 1 AND 100
        AND reason_code = LOWER(BTRIM(reason_code))
        AND reason_code ~ '^[a-z][a-z0-9_.-]*$'
    ),
    safe_message TEXT NOT NULL CHECK (
        CHAR_LENGTH(BTRIM(safe_message)) BETWEEN 1 AND 500
        AND safe_message !~ '[[:cntrl:]]'
    ),
    rejected_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ CHECK (deleted_at IS NULL),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_capability_rejections_call_tenant_unique
        UNIQUE (capability_call_id, tenant_id),
    CONSTRAINT agent_capability_rejections_call_tenant_run_unique
        UNIQUE (capability_call_id, tenant_id, run_id),
    CONSTRAINT agent_capability_rejections_run_sequence_unique
        UNIQUE (run_id, call_sequence),
    CONSTRAINT agent_capability_rejections_run_tenant_fk
        FOREIGN KEY (run_id, tenant_id)
        REFERENCES agent_runs(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_capability_rejections_actor_tenant_fk
        FOREIGN KEY (actor_user_id, tenant_id)
        REFERENCES users(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT agent_capability_rejections_operation_shape_check CHECK (
        (
            product_operation_key IS NULL
            AND owning_module_key IS NULL
            AND required_permission IS NULL
        )
        OR (
            product_operation_key IS NOT NULL
            AND owning_module_key IS NOT NULL
            AND required_permission IS NOT NULL
        )
    ),
    CONSTRAINT agent_capability_rejections_scope_shape_check CHECK (
        (
            scope_kind IS NULL
            AND resource_references IS NULL
        )
        OR (
            scope_kind = 'tenant_wide'
            AND resource_references IS NOT NULL
            AND resource_references = '[]'::JSONB
            AND product_operation_key IS NOT NULL
        )
        OR (
            scope_kind = 'resources'
            AND resource_references IS NOT NULL
            AND agent_valid_resource_references(resource_references)
            AND product_operation_key IS NOT NULL
        )
    ),
    CONSTRAINT agent_capability_rejections_code_evidence_shape_check CHECK (
        (
            broker_error_code = 'unknown_capability'
            AND product_operation_key IS NULL
            AND scope_kind IS NULL
        )
        OR (
            broker_error_code = 'input_too_large'
            AND product_operation_key IS NOT NULL
            AND normalized_input_digest_sha256 IS NULL
            AND scope_kind IS NULL
        )
        OR (
            broker_error_code IN ('invalid_input', 'execution_failed')
            AND product_operation_key IS NOT NULL
            AND normalized_input_digest_sha256 IS NOT NULL
            AND scope_kind IS NULL
        )
        OR (
            broker_error_code = 'record_scope_denied'
            AND product_operation_key IS NOT NULL
            AND normalized_input_digest_sha256 IS NOT NULL
            AND scope_kind IS NOT NULL
        )
        OR (
            broker_error_code IN (
                'unsupported_version', 'capability_unavailable',
                'approval_required', 'human_only', 'prohibited',
                'authority_unavailable', 'access_denied'
            )
            AND product_operation_key IS NOT NULL
            AND scope_kind IS NULL
        )
        OR (
            broker_error_code = 'audit_unavailable'
            AND (
                product_operation_key IS NOT NULL
                OR (
                    scope_kind IS NULL
                )
            )
            AND (
                scope_kind IS NULL
                OR normalized_input_digest_sha256 IS NOT NULL
            )
        )
    ),
    CONSTRAINT agent_capability_rejections_outcome_code_check CHECK (
        (
            outcome = 'denied'
            AND broker_error_code IN (
                'unknown_capability', 'unsupported_version',
                'capability_unavailable', 'approval_required', 'human_only',
                'prohibited', 'access_denied', 'invalid_input',
                'input_too_large', 'record_scope_denied'
            )
        )
        OR (
            outcome = 'failed'
            AND broker_error_code IN (
                'authority_unavailable', 'execution_failed', 'audit_unavailable'
            )
        )
    ),
    CONSTRAINT agent_capability_rejections_immutable_time_check CHECK (
        rejected_at = created_at AND updated_at = created_at
    )
);

CREATE INDEX IF NOT EXISTS idx_agent_capability_rejections_run_history
    ON agent_capability_rejections(tenant_id, run_id, call_sequence);

CREATE INDEX IF NOT EXISTS idx_agent_capability_rejections_reporting
    ON agent_capability_rejections(
        tenant_id, owning_module_key, capability_key, rejected_at DESC,
        capability_call_id
    );

-- A replay repairs a missing identity fence but never overwrites a conflicting
-- disposition. The following assertion turns any legacy/manual conflict into
-- an explicit migration failure rather than accepting split-brain intent.
INSERT INTO agent_capability_intent_registry (
    capability_call_id, tenant_id, run_id, call_sequence, disposition
)
SELECT
    capability_call_id, tenant_id, run_id, call_sequence, 'rejected'
FROM agent_capability_rejections
ON CONFLICT DO NOTHING;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM agent_capability_calls AS executable
        LEFT JOIN agent_capability_intent_registry AS registry
          ON registry.capability_call_id = executable.id
         AND registry.tenant_id = executable.tenant_id
         AND registry.run_id = executable.run_id
         AND registry.call_sequence = executable.call_sequence
         AND registry.disposition = 'executable'
        WHERE registry.capability_call_id IS NULL
    )
       OR EXISTS (
           SELECT 1
           FROM agent_capability_rejections AS rejection
           LEFT JOIN agent_capability_intent_registry AS registry
             ON registry.capability_call_id = rejection.capability_call_id
            AND registry.tenant_id = rejection.tenant_id
            AND registry.run_id = rejection.run_id
            AND registry.call_sequence = rejection.call_sequence
            AND registry.disposition = 'rejected'
           WHERE registry.capability_call_id IS NULL
       ) THEN
        RAISE EXCEPTION 'Agent capability intent registry conflicts with durable ledgers';
    END IF;
END;
$$;

CREATE OR REPLACE FUNCTION validate_agent_capability_rejection_insert()
RETURNS TRIGGER AS $$
DECLARE
    stored_actor_user_id UUID;
    stored_correlation_id UUID;
    stored_run_status TEXT;
    database_timestamp TIMESTAMPTZ := STATEMENT_TIMESTAMP();
BEGIN
    IF CURRENT_SETTING(
        'campus.agent_capability_rejection_admission',
        TRUE
    ) IS DISTINCT FROM NEW.capability_call_id::TEXT THEN
        RAISE EXCEPTION 'Agent capability rejections require the fenced recorder';
    END IF;
    PERFORM SET_CONFIG(
        'campus.agent_capability_rejection_admission',
        '',
        TRUE
    );

    PERFORM register_agent_capability_intent(
        NEW.capability_call_id,
        NEW.tenant_id,
        NEW.run_id,
        NEW.call_sequence,
        'rejected'
    );

    IF NEW.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'Agent capability rejection evidence must start terminal and immutable';
    END IF;

    -- Evidence time belongs to the database. Callers cannot backdate, future-date,
    -- or split the three immutable timestamps, even through a direct INSERT.
    NEW.request_id := NEW.capability_call_id;
    NEW.rejected_at := database_timestamp;
    NEW.created_at := database_timestamp;
    NEW.updated_at := database_timestamp;

    PERFORM 1
    FROM agent_run_queue
    WHERE run_id = NEW.run_id
      AND tenant_id = NEW.tenant_id
      AND state = 'leased'
      AND leased_by = NEW.claimed_by_worker_id
      AND version = NEW.claim_fence_version
      AND lease_expires_at > database_timestamp
      AND cancel_requested_at IS NULL
      AND checkpoint IN (
          'provider_result_persisted', 'capability_result_persisted'
      )
      AND deleted_at IS NULL
    FOR UPDATE;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Agent capability rejection requires its exact current run lease';
    END IF;

    SELECT requested_by, correlation_id, status
    INTO stored_actor_user_id, stored_correlation_id, stored_run_status
    FROM agent_runs
    WHERE id = NEW.run_id
      AND tenant_id = NEW.tenant_id
    FOR UPDATE;

    IF NOT FOUND
       OR stored_actor_user_id <> NEW.actor_user_id
       OR stored_correlation_id <> NEW.correlation_id
       OR stored_run_status <> 'running' THEN
        RAISE EXCEPTION 'Agent capability rejection requires its exact current run lease';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM agent_capability_calls
        WHERE id = NEW.capability_call_id
           OR (
               run_id = NEW.run_id
               AND call_sequence = NEW.call_sequence
           )
    ) THEN
        RAISE EXCEPTION 'Agent capability rejection cannot replace an executable call';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_capability_rejections_validate_insert
    ON agent_capability_rejections;
CREATE TRIGGER agent_capability_rejections_validate_insert
    BEFORE INSERT ON agent_capability_rejections
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_capability_rejection_insert();

CREATE OR REPLACE FUNCTION prevent_rejected_agent_capability_call_insert()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM register_agent_capability_intent(
        NEW.id,
        NEW.tenant_id,
        NEW.run_id,
        NEW.call_sequence,
        'executable'
    );

    IF EXISTS (
        SELECT 1
        FROM agent_capability_rejections
        WHERE capability_call_id = NEW.id
           OR (
               run_id = NEW.run_id
               AND call_sequence = NEW.call_sequence
           )
    ) THEN
        RAISE EXCEPTION 'rejected Agent capability intent cannot become executable';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_capability_calls_rejection_guard
    ON agent_capability_calls;
CREATE TRIGGER agent_capability_calls_rejection_guard
    BEFORE INSERT ON agent_capability_calls
    FOR EACH ROW
    EXECUTE FUNCTION prevent_rejected_agent_capability_call_insert();

CREATE OR REPLACE FUNCTION reject_agent_capability_rejection_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Agent capability rejection evidence is append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_capability_rejections_reject_mutation
    ON agent_capability_rejections;
CREATE TRIGGER agent_capability_rejections_reject_mutation
    BEFORE UPDATE OR DELETE ON agent_capability_rejections
    FOR EACH ROW
    EXECUTE FUNCTION reject_agent_capability_rejection_mutation();

DROP TRIGGER IF EXISTS agent_capability_rejections_reject_truncate
    ON agent_capability_rejections;
CREATE TRIGGER agent_capability_rejections_reject_truncate
    BEFORE TRUNCATE ON agent_capability_rejections
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_agent_capability_rejection_mutation();

-- The worker supplies one trusted call identity before broker preparation.
-- Exact retries return that same identity; any changed fact fails closed.
CREATE OR REPLACE FUNCTION record_agent_capability_rejection(
    p_capability_call_id UUID,
    p_tenant_id UUID,
    p_run_id UUID,
    p_call_sequence SMALLINT,
    p_actor_user_id UUID,
    p_request_id UUID,
    p_correlation_id UUID,
    p_capability_key TEXT,
    p_capability_version INTEGER,
    p_normalized_input_digest_sha256 BYTEA,
    p_product_operation_key TEXT,
    p_owning_module_key TEXT,
    p_required_permission TEXT,
    p_scope_kind TEXT,
    p_resource_references JSONB,
    p_outcome TEXT,
    p_broker_error_code TEXT,
    p_reason_code TEXT,
    p_safe_message TEXT,
    p_worker_id TEXT,
    p_lease_token UUID,
    p_fence_version BIGINT
)
RETURNS UUID AS $$
DECLARE
    stored agent_capability_rejections%ROWTYPE;
    stored_actor_user_id UUID;
    stored_correlation_id UUID;
    stored_run_status TEXT;
BEGIN
    -- A completed exact write is already terminal evidence. Lost acknowledgements
    -- remain replay-safe after lease expiry or replacement, while changed facts
    -- never gain an idempotency shortcut around the current-lease check.
    SELECT *
    INTO stored
    FROM agent_capability_rejections
    WHERE capability_call_id = p_capability_call_id;

    IF FOUND THEN
        IF stored.tenant_id IS DISTINCT FROM p_tenant_id
           OR stored.run_id IS DISTINCT FROM p_run_id
           OR stored.call_sequence IS DISTINCT FROM p_call_sequence
           OR stored.actor_user_id IS DISTINCT FROM p_actor_user_id
           OR stored.request_id IS DISTINCT FROM p_capability_call_id
           OR stored.correlation_id IS DISTINCT FROM p_correlation_id
           OR stored.capability_key IS DISTINCT FROM p_capability_key
           OR stored.capability_version IS DISTINCT FROM p_capability_version
           OR stored.normalized_input_digest_sha256
                IS DISTINCT FROM p_normalized_input_digest_sha256
           OR stored.product_operation_key IS DISTINCT FROM p_product_operation_key
           OR stored.owning_module_key IS DISTINCT FROM p_owning_module_key
           OR stored.required_permission IS DISTINCT FROM p_required_permission
           OR stored.scope_kind IS DISTINCT FROM p_scope_kind
           OR stored.resource_references IS DISTINCT FROM p_resource_references
           OR stored.outcome IS DISTINCT FROM p_outcome
           OR stored.broker_error_code IS DISTINCT FROM p_broker_error_code
           OR stored.reason_code IS DISTINCT FROM p_reason_code
           OR stored.safe_message IS DISTINCT FROM p_safe_message THEN
            RAISE EXCEPTION 'Agent capability rejection idempotency conflict';
        END IF;

        RETURN stored.capability_call_id;
    END IF;

    IF EXISTS (
        SELECT 1
        FROM agent_capability_rejections
        WHERE run_id = p_run_id
          AND call_sequence = p_call_sequence
    ) THEN
        RAISE EXCEPTION 'Agent capability rejection idempotency conflict';
    END IF;

    -- Lock the queue before the run, matching every worker transition in the
    -- Rust runtime. The raw lease token is consumed only by this predicate and
    -- is deliberately not copied into retained rejection evidence.
    PERFORM 1
    FROM agent_run_queue
    WHERE run_id = p_run_id
      AND tenant_id = p_tenant_id
      AND state = 'leased'
      AND leased_by = p_worker_id
      AND lease_token = p_lease_token
      AND version = p_fence_version
      AND lease_expires_at > STATEMENT_TIMESTAMP()
      AND cancel_requested_at IS NULL
      AND checkpoint IN (
          'provider_result_persisted', 'capability_result_persisted'
      )
      AND deleted_at IS NULL
    FOR UPDATE;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Agent capability rejection requires its exact current run lease';
    END IF;

    SELECT requested_by, correlation_id, status
    INTO stored_actor_user_id, stored_correlation_id, stored_run_status
    FROM agent_runs
    WHERE id = p_run_id
      AND tenant_id = p_tenant_id
    FOR UPDATE;

    IF NOT FOUND
       OR stored_actor_user_id IS DISTINCT FROM p_actor_user_id
       OR stored_correlation_id IS DISTINCT FROM p_correlation_id
       OR stored_run_status <> 'running'
       OR p_request_id IS DISTINCT FROM p_capability_call_id THEN
        RAISE EXCEPTION 'Agent capability rejection has invalid run evidence';
    END IF;

    BEGIN
        PERFORM register_agent_capability_intent(
            p_capability_call_id,
            p_tenant_id,
            p_run_id,
            p_call_sequence,
            'rejected'
        );
    EXCEPTION
        WHEN unique_violation OR raise_exception THEN
            RAISE EXCEPTION 'Agent capability rejection idempotency conflict';
    END;

    LOOP
        SELECT *
        INTO stored
        FROM agent_capability_rejections
        WHERE capability_call_id = p_capability_call_id;

        IF FOUND THEN
            IF stored.tenant_id IS DISTINCT FROM p_tenant_id
               OR stored.run_id IS DISTINCT FROM p_run_id
               OR stored.call_sequence IS DISTINCT FROM p_call_sequence
               OR stored.actor_user_id IS DISTINCT FROM p_actor_user_id
               OR stored.request_id IS DISTINCT FROM p_capability_call_id
               OR stored.correlation_id IS DISTINCT FROM p_correlation_id
               OR stored.capability_key IS DISTINCT FROM p_capability_key
               OR stored.capability_version IS DISTINCT FROM p_capability_version
               OR stored.normalized_input_digest_sha256
                    IS DISTINCT FROM p_normalized_input_digest_sha256
               OR stored.product_operation_key
                    IS DISTINCT FROM p_product_operation_key
               OR stored.owning_module_key IS DISTINCT FROM p_owning_module_key
               OR stored.required_permission IS DISTINCT FROM p_required_permission
               OR stored.scope_kind IS DISTINCT FROM p_scope_kind
               OR stored.resource_references IS DISTINCT FROM p_resource_references
               OR stored.outcome IS DISTINCT FROM p_outcome
               OR stored.broker_error_code IS DISTINCT FROM p_broker_error_code
               OR stored.reason_code IS DISTINCT FROM p_reason_code
               OR stored.safe_message IS DISTINCT FROM p_safe_message THEN
                RAISE EXCEPTION 'Agent capability rejection idempotency conflict';
            END IF;

            RETURN stored.capability_call_id;
        END IF;

        IF EXISTS (
            SELECT 1
            FROM agent_capability_rejections
            WHERE run_id = p_run_id
              AND call_sequence = p_call_sequence
        ) THEN
            RAISE EXCEPTION 'Agent capability rejection idempotency conflict';
        END IF;

        BEGIN
            -- This transaction-local admission is consumed by the insert
            -- trigger. It keeps the shared application role on the fenced
            -- function path without retaining the raw queue lease token.
            PERFORM SET_CONFIG(
                'campus.agent_capability_rejection_admission',
                p_capability_call_id::TEXT,
                TRUE
            );
            INSERT INTO agent_capability_rejections (
                capability_call_id, tenant_id, run_id, call_sequence,
                actor_user_id, request_id, correlation_id,
                claimed_by_worker_id, claim_fence_version, capability_key,
                capability_version, normalized_input_digest_sha256,
                product_operation_key, owning_module_key, required_permission,
                scope_kind, resource_references, outcome, broker_error_code,
                reason_code, safe_message
            ) VALUES (
                p_capability_call_id, p_tenant_id, p_run_id, p_call_sequence,
                p_actor_user_id, p_capability_call_id, p_correlation_id,
                p_worker_id, p_fence_version,
                p_capability_key, p_capability_version,
                p_normalized_input_digest_sha256, p_product_operation_key,
                p_owning_module_key, p_required_permission, p_scope_kind,
                p_resource_references, p_outcome, p_broker_error_code,
                p_reason_code, p_safe_message
            )
            RETURNING * INTO stored;

            RETURN stored.capability_call_id;
        EXCEPTION WHEN unique_violation THEN
            -- A concurrent exact writer won. The loop reloads and compares all
            -- durable facts; a different call or payload becomes a conflict.
        END;
    END LOOP;
END;
$$ LANGUAGE plpgsql;
