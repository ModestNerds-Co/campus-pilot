-- Exercises migration 085 against a disposable PostgreSQL database.
-- The caller applies migrations first; every fixture and assertion is rolled back.

\set ON_ERROR_STOP on

BEGIN;

CREATE OR REPLACE FUNCTION pg_temp.assert_true(assertion BOOLEAN, message TEXT)
RETURNS VOID AS $$
BEGIN
    IF assertion IS DISTINCT FROM TRUE THEN
        RAISE EXCEPTION 'assertion failed: %', message;
    END IF;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION pg_temp.expect_failure(statement TEXT, expected_fragment TEXT)
RETURNS VOID AS $$
DECLARE
    failed BOOLEAN := FALSE;
    failure_message TEXT;
BEGIN
    BEGIN
        EXECUTE statement;
    EXCEPTION WHEN OTHERS THEN
        failed := TRUE;
        failure_message := SQLERRM;
    END;

    IF NOT failed THEN
        RAISE EXCEPTION 'expected statement to fail: %', statement;
    END IF;

    IF expected_fragment IS NOT NULL
       AND POSITION(expected_fragment IN failure_message) = 0 THEN
        RAISE EXCEPTION 'expected failure containing %, received %',
            expected_fragment, failure_message;
    END IF;
END;
$$ LANGUAGE plpgsql;

INSERT INTO tenants (id, slug, name)
VALUES
    ('a0000000-0000-0000-0000-000000000001', 'agent-085-a', 'Agent 085 A'),
    ('b0000000-0000-0000-0000-000000000001', 'agent-085-b', 'Agent 085 B');

INSERT INTO users (id, tenant_id, email, password_hash, full_name)
VALUES
    (
        'a1000000-0000-0000-0000-000000000001',
        'a0000000-0000-0000-0000-000000000001',
        'owner-a@agent-085.test',
        'test-only',
        'Owner A'
    ),
    (
        'a1000000-0000-0000-0000-000000000002',
        'a0000000-0000-0000-0000-000000000001',
        'member-a@agent-085.test',
        'test-only',
        'Member A'
    ),
    (
        'b1000000-0000-0000-0000-000000000001',
        'b0000000-0000-0000-0000-000000000001',
        'owner-b@agent-085.test',
        'test-only',
        'Owner B'
    );

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_threads (tenant_id, owner_user_id, status)
        VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a1000000-0000-0000-0000-000000000001',
            'archived'
        )
    $statement$,
    'must start active'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_threads (
            tenant_id, owner_user_id, next_message_sequence, version
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a1000000-0000-0000-0000-000000000001',
            2,
            2
        )
    $statement$,
    'initial version and sequence'
);

INSERT INTO agent_threads (id, tenant_id, owner_user_id)
VALUES
    (
        'a3000000-0000-0000-0000-000000000001',
        'a0000000-0000-0000-0000-000000000001',
        'a1000000-0000-0000-0000-000000000001'
    ),
    (
        'b3000000-0000-0000-0000-000000000001',
        'b0000000-0000-0000-0000-000000000001',
        'b1000000-0000-0000-0000-000000000001'
    );

INSERT INTO agent_thread_members (
    id, tenant_id, thread_id, user_id, membership_role, added_by
)
VALUES
    (
        'a3100000-0000-0000-0000-000000000001',
        'a0000000-0000-0000-0000-000000000001',
        'a3000000-0000-0000-0000-000000000001',
        'a1000000-0000-0000-0000-000000000001',
        'owner',
        'a1000000-0000-0000-0000-000000000001'
    ),
    (
        'b3100000-0000-0000-0000-000000000001',
        'b0000000-0000-0000-0000-000000000001',
        'b3000000-0000-0000-0000-000000000001',
        'b1000000-0000-0000-0000-000000000001',
        'owner',
        'b1000000-0000-0000-0000-000000000001'
    );

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_thread_members (
            tenant_id, thread_id, user_id, membership_role, added_by
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'b3000000-0000-0000-0000-000000000001',
            'a1000000-0000-0000-0000-000000000002',
            'member',
            'a1000000-0000-0000-0000-000000000001'
        )
    $statement$,
    'active same-tenant Session'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_thread_members (
            tenant_id, thread_id, user_id, membership_role, added_by, deleted_at
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a3000000-0000-0000-0000-000000000001',
            'a1000000-0000-0000-0000-000000000002',
            'member',
            'a1000000-0000-0000-0000-000000000001',
            NOW()
        )
    $statement$,
    'must start active'
);

INSERT INTO agent_thread_members (
    id, tenant_id, thread_id, user_id, membership_role, added_by
)
VALUES (
    'a3100000-0000-0000-0000-000000000002',
    'a0000000-0000-0000-0000-000000000001',
    'a3000000-0000-0000-0000-000000000001',
    'a1000000-0000-0000-0000-000000000002',
    'member',
    'a1000000-0000-0000-0000-000000000001'
);

UPDATE agent_thread_members
SET deleted_at = NOW(), updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a3100000-0000-0000-0000-000000000002';

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_thread_members
        SET deleted_at = NOW(), updated_at = updated_at + INTERVAL '1 second'
        WHERE id = 'a3100000-0000-0000-0000-000000000001'
    $statement$,
    'only non-owner'
);

INSERT INTO agent_thread_members (
    id, tenant_id, thread_id, user_id, membership_role, added_by
)
VALUES (
    'a3100000-0000-0000-0000-000000000003',
    'a0000000-0000-0000-0000-000000000001',
    'a3000000-0000-0000-0000-000000000001',
    'a1000000-0000-0000-0000-000000000002',
    'member',
    'a1000000-0000-0000-0000-000000000001'
);

UPDATE agent_threads
SET next_message_sequence = 2,
    version = 2,
    last_activity_at = last_activity_at + INTERVAL '1 second',
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a3000000-0000-0000-0000-000000000001';

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_messages (
            tenant_id, thread_id, sequence, role, user_id, content
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a3000000-0000-0000-0000-000000000001',
            1,
            'user',
            'a1000000-0000-0000-0000-000000000002',
            'A shared member cannot submit in the owner-only release.'
        )
    $statement$,
    'active owner membership'
);

INSERT INTO agent_messages (
    id, tenant_id, thread_id, sequence, role, user_id, content
)
VALUES (
    'a4000000-0000-0000-0000-000000000001',
    'a0000000-0000-0000-0000-000000000001',
    'a3000000-0000-0000-0000-000000000001',
    1,
    'user',
    'a1000000-0000-0000-0000-000000000001',
    'List active vehicles.'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_messages
        SET content = 'changed'
        WHERE id = 'a4000000-0000-0000-0000-000000000001'
    $statement$,
    'append-only'
);

SELECT pg_temp.expect_failure(
    $statement$
        DELETE FROM agent_messages
        WHERE id = 'a4000000-0000-0000-0000-000000000001'
    $statement$,
    'append-only'
);

UPDATE agent_threads
SET next_message_sequence = 3,
    version = 3,
    last_activity_at = last_activity_at + INTERVAL '1 second',
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a3000000-0000-0000-0000-000000000001';

INSERT INTO agent_messages (
    id, tenant_id, thread_id, sequence, role, user_id, content
)
VALUES (
    'a4000000-0000-0000-0000-000000000002',
    'a0000000-0000-0000-0000-000000000001',
    'a3000000-0000-0000-0000-000000000001',
    2,
    'user',
    'a1000000-0000-0000-0000-000000000001',
    'List active drivers.'
);

INSERT INTO agent_runs (
    id, tenant_id, thread_id, request_message_id, requested_by, task_class,
    origin_module_key, origin_route, request_id, correlation_id
)
VALUES (
    'a5000000-0000-0000-0000-000000000001',
    'a0000000-0000-0000-0000-000000000001',
    'a3000000-0000-0000-0000-000000000001',
    'a4000000-0000-0000-0000-000000000001',
    'a1000000-0000-0000-0000-000000000001',
    'module_read_reporting',
    'fleet',
    '/modules/fleet',
    'a5100000-0000-0000-0000-000000000001',
    'a5200000-0000-0000-0000-000000000001'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_runs (
            tenant_id, thread_id, request_message_id, requested_by, task_class,
            origin_module_key, origin_route, request_id, correlation_id
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a3000000-0000-0000-0000-000000000001',
            'a4000000-0000-0000-0000-000000000002',
            'a1000000-0000-0000-0000-000000000001',
            'module_read_reporting',
            'fleet',
            '/modules/fleet',
            gen_random_uuid(),
            gen_random_uuid()
        )
    $statement$,
    'agent_runs_active_thread_unique'
);

UPDATE agent_runs
SET status = 'running',
    started_at = NOW(),
    version = 2,
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a5000000-0000-0000-0000-000000000001';

UPDATE agent_threads
SET next_message_sequence = 4,
    version = 4,
    last_activity_at = last_activity_at + INTERVAL '1 second',
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a3000000-0000-0000-0000-000000000001';

INSERT INTO agent_messages (
    id, tenant_id, thread_id, sequence, role, content
)
VALUES (
    'a4000000-0000-0000-0000-000000000003',
    'a0000000-0000-0000-0000-000000000001',
    'a3000000-0000-0000-0000-000000000001',
    3,
    'assistant',
    'There are no active vehicles.'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_runs
        SET status = 'completed',
            response_message_id = 'a4000000-0000-0000-0000-000000000003',
            finished_at = NOW(),
            version = 3,
            updated_at = updated_at + INTERVAL '2 seconds'
        WHERE id = 'a5000000-0000-0000-0000-000000000001'
    $statement$,
    'durable finalization evidence'
);

INSERT INTO agent_execution_steps (
    id, tenant_id, run_id, step_index, turn_index, step_kind, input_fingerprint
)
VALUES (
    'a7600000-0000-0000-0000-000000000010',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000001',
    1,
    1,
    'finalize',
    DECODE(REPEAT('20', 32), 'hex')
);

INSERT INTO agent_execution_artifacts (
    id, tenant_id, run_id, step_id, artifact_sequence, artifact_kind,
    ciphertext, ciphertext_sha256, plaintext_sha256, nonce, encryption_key_id,
    encryption_key_version, plaintext_length
)
VALUES (
    'a7700000-0000-0000-0000-000000000010',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000001',
    'a7600000-0000-0000-0000-000000000010',
    1,
    'final_response',
    DECODE(REPEAT('21', 32), 'hex'),
    DECODE(REPEAT('22', 32), 'hex'),
    DECODE(REPEAT('23', 32), 'hex'),
    DECODE(REPEAT('24', 12), 'hex'),
    'contract-agent-artifact-key',
    1,
    32
);

UPDATE agent_execution_steps
SET status = 'succeeded',
    finished_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a7600000-0000-0000-0000-000000000010';

UPDATE agent_runs
SET status = 'completed',
    response_message_id = 'a4000000-0000-0000-0000-000000000003',
    finished_at = NOW(),
    version = 3,
    updated_at = updated_at + INTERVAL '2 seconds'
WHERE id = 'a5000000-0000-0000-0000-000000000001';

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_runs
        SET updated_at = updated_at + INTERVAL '1 second'
        WHERE id = 'a5000000-0000-0000-0000-000000000001'
    $statement$,
    'terminal Agent runs are immutable'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_runs (
            tenant_id, thread_id, request_message_id, requested_by, task_class,
            origin_module_key, origin_route, request_id, correlation_id,
            status, safe_failure_code, safe_failure_message, finished_at
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a3000000-0000-0000-0000-000000000001',
            'a4000000-0000-0000-0000-000000000002',
            'a1000000-0000-0000-0000-000000000001',
            'module_read_reporting',
            'fleet',
            '/modules/fleet',
            gen_random_uuid(),
            gen_random_uuid(),
            'failed',
            'insert_bypass',
            'A run cannot be inserted in a terminal state.',
            NOW()
        )
    $statement$,
    'must start in the initial queued state'
);

INSERT INTO agent_runs (
    id, tenant_id, thread_id, request_message_id, requested_by, task_class,
    origin_module_key, origin_route, request_id, correlation_id
)
VALUES (
    'a5000000-0000-0000-0000-000000000002',
    'a0000000-0000-0000-0000-000000000001',
    'a3000000-0000-0000-0000-000000000001',
    'a4000000-0000-0000-0000-000000000002',
    'a1000000-0000-0000-0000-000000000001',
    'module_read_reporting',
    'fleet',
    '/modules/fleet',
    'a5100000-0000-0000-0000-000000000002',
    'a5200000-0000-0000-0000-000000000002'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_run_queue (
            run_id, tenant_id, state, lease_token, leased_by,
            heartbeat_at, lease_expires_at, delivery_attempt
        ) VALUES (
            'a5000000-0000-0000-0000-000000000002',
            'a0000000-0000-0000-0000-000000000001',
            'leased',
            gen_random_uuid(),
            'insert-bypass-worker',
            STATEMENT_TIMESTAMP(),
            STATEMENT_TIMESTAMP() + INTERVAL '30 seconds',
            1
        )
    $statement$,
    'must start in the initial available state'
);

INSERT INTO agent_run_queue (run_id, tenant_id)
VALUES (
    'a5000000-0000-0000-0000-000000000002',
    'a0000000-0000-0000-0000-000000000001'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_run_queue
        SET state = 'leased', version = 2, updated_at = updated_at + INTERVAL '1 second'
        WHERE run_id = 'a5000000-0000-0000-0000-000000000002'
    $statement$,
    'Agent queue claims require the next delivery attempt'
);

WITH claim_clock AS (
    SELECT STATEMENT_TIMESTAMP() AS claimed_at
), claim_candidate AS (
    SELECT run_id, tenant_id
    FROM agent_run_queue
    WHERE state = 'available'
      AND available_at <= STATEMENT_TIMESTAMP()
      AND delivery_attempt < 3
    ORDER BY available_at, run_id
    FOR UPDATE SKIP LOCKED
    LIMIT 1
)
UPDATE agent_run_queue AS queue_row
SET state = 'leased',
    lease_token = 'a6000000-0000-0000-0000-000000000001',
    leased_by = 'contract-worker',
    heartbeat_at = claim_clock.claimed_at,
    lease_expires_at = claim_clock.claimed_at + INTERVAL '30 seconds',
    delivery_attempt = 1,
    version = 2,
    updated_at = updated_at + INTERVAL '1 second'
FROM claim_clock, claim_candidate
WHERE claim_candidate.run_id = queue_row.run_id
  AND claim_candidate.tenant_id = queue_row.tenant_id;

UPDATE agent_runs
SET status = 'running',
    started_at = NOW(),
    version = 2,
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a5000000-0000-0000-0000-000000000002';

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_run_queue
        SET heartbeat_at = STATEMENT_TIMESTAMP() + INTERVAL '5 seconds',
            lease_expires_at = STATEMENT_TIMESTAMP() + INTERVAL '35 seconds',
            version = 3,
            updated_at = updated_at + INTERVAL '1 second'
        WHERE run_id = 'a5000000-0000-0000-0000-000000000002'
    $statement$,
    'stale Agent worker lease'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_run_queue
        SET lease_token = gen_random_uuid(),
            heartbeat_at = heartbeat_at + INTERVAL '10 seconds',
            lease_expires_at = lease_expires_at + INTERVAL '10 seconds',
            version = 3,
            updated_at = updated_at + INTERVAL '1 second'
        WHERE run_id = 'a5000000-0000-0000-0000-000000000002'
    $statement$,
    'stale Agent worker lease'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_run_queue
        SET checkpoint = 'capability_in_flight',
            heartbeat_at = heartbeat_at + INTERVAL '10 seconds',
            lease_expires_at = lease_expires_at + INTERVAL '10 seconds',
            version = 3,
            updated_at = updated_at + INTERVAL '1 second'
        WHERE run_id = 'a5000000-0000-0000-0000-000000000002'
    $statement$,
    'invalid checkpoint transition'
);

SELECT pg_temp.assert_true(
    NOT agent_valid_checkpoint_transition(
        'capability_result_persisted',
        'capability_in_flight'
    ),
    'v1 must reject a second capability call in the same provider turn'
);

WITH heartbeat_clock AS (
    SELECT STATEMENT_TIMESTAMP() AS heartbeat_at
)
UPDATE agent_run_queue
SET checkpoint = 'before_provider',
    heartbeat_at = heartbeat_clock.heartbeat_at,
    lease_expires_at = heartbeat_clock.heartbeat_at + INTERVAL '30 seconds',
    version = 3,
    updated_at = updated_at + INTERVAL '1 second'
FROM heartbeat_clock
WHERE run_id = 'a5000000-0000-0000-0000-000000000002';

INSERT INTO ai_provider_connections (
    id, tenant_id, provider, auth_method, account_label, status,
    credential_ciphertext, credential_nonce, credential_key_id,
    credential_fingerprint, configured_by, model_catalog_version
)
VALUES (
    'a7000000-0000-0000-0000-000000000001',
    'a0000000-0000-0000-0000-000000000001',
    'openai',
    'api_key',
    'Contract provider',
    'ready',
    DECODE(REPEAT('ab', 16), 'hex'),
    DECODE(REPEAT('cd', 12), 'hex'),
    'contract-key',
    'contract-fingerprint',
    'a1000000-0000-0000-0000-000000000001',
    1
);

INSERT INTO ai_provider_models (
    id, tenant_id, connection_id, credential_version, catalog_version,
    provider_model_id, display_name, max_output_tokens, supports_tools, refreshed_at
)
VALUES (
    'a7100000-0000-0000-0000-000000000001',
    'a0000000-0000-0000-0000-000000000001',
    'a7000000-0000-0000-0000-000000000001',
    1,
    1,
    'contract-model',
    'Contract Model',
    4096,
    TRUE,
    NOW()
);

INSERT INTO ai_provider_models (
    id, tenant_id, connection_id, credential_version, catalog_version,
    provider_model_id, display_name, supports_tools, refreshed_at
)
VALUES (
    'a7100000-0000-0000-0000-000000000002',
    'a0000000-0000-0000-0000-000000000001',
    'a7000000-0000-0000-0000-000000000001',
    1,
    1,
    'uncapped-contract-model',
    'Uncapped Contract Model',
    TRUE,
    NOW()
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE ai_provider_models
        SET max_output_tokens = 8192
        WHERE id = 'a7100000-0000-0000-0000-000000000001'
    $statement$,
    'model snapshots are immutable'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO ai_provider_models (
            tenant_id, connection_id, credential_version, catalog_version,
            provider_model_id, display_name, max_output_tokens, refreshed_at
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a7000000-0000-0000-0000-000000000001',
            1,
            1,
            'invalid-output-cap',
            'Invalid output cap',
            0,
            NOW()
        )
    $statement$,
    'ai_provider_models_max_output_tokens_check'
);

INSERT INTO ai_route_sets (
    id, tenant_id, scope_kind, configured_by, change_reason
)
VALUES (
    'a7200000-0000-0000-0000-000000000001',
    'a0000000-0000-0000-0000-000000000001',
    'tenant_default',
    'a1000000-0000-0000-0000-000000000001',
    'Agent 085 contract route'
);

INSERT INTO ai_task_routes (
    id, tenant_id, route_set_id, priority, connection_id, model_id,
    requires_tools, created_by
)
VALUES (
    'a7300000-0000-0000-0000-000000000001',
    'a0000000-0000-0000-0000-000000000001',
    'a7200000-0000-0000-0000-000000000001',
    1,
    'a7000000-0000-0000-0000-000000000001',
    'a7100000-0000-0000-0000-000000000001',
    TRUE,
    'a1000000-0000-0000-0000-000000000001'
);

INSERT INTO ai_route_sets (
    id, tenant_id, scope_kind, module_key, operation_class,
    configured_by, change_reason
)
VALUES (
    'a7200000-0000-0000-0000-000000000003',
    'a0000000-0000-0000-0000-000000000001',
    'module_operation',
    'fleet',
    'read',
    'a1000000-0000-0000-0000-000000000001',
    'Uncapped model contract route'
);

INSERT INTO ai_task_routes (
    id, tenant_id, route_set_id, priority, connection_id, model_id,
    requires_tools, created_by
)
VALUES (
    'a7300000-0000-0000-0000-000000000003',
    'a0000000-0000-0000-0000-000000000001',
    'a7200000-0000-0000-0000-000000000003',
    1,
    'a7000000-0000-0000-0000-000000000001',
    'a7100000-0000-0000-0000-000000000002',
    TRUE,
    'a1000000-0000-0000-0000-000000000001'
);

UPDATE ai_provider_connections
SET status = 'error',
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a7000000-0000-0000-0000-000000000001';

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_provider_attempts (
            tenant_id, run_id, turn_index, attempt_index, route_set_id, route_version,
            route_target_id, connection_id, credential_version, model_snapshot_id,
            provider_key, provider_model_id, task_class
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            1,
            1,
            'a7200000-0000-0000-0000-000000000001',
            1,
            'a7300000-0000-0000-0000-000000000001',
            'a7000000-0000-0000-0000-000000000001',
            1,
            'a7100000-0000-0000-0000-000000000001',
            'openai',
            'contract-model',
            'module_read_reporting'
        )
    $statement$,
    'resolved route snapshot'
);

UPDATE ai_provider_connections
SET status = 'ready',
    updated_at = updated_at + INTERVAL '2 seconds'
WHERE id = 'a7000000-0000-0000-0000-000000000001';

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_provider_attempts (
            tenant_id, run_id, turn_index, attempt_index, route_set_id, route_version,
            route_target_id, connection_id, credential_version, model_snapshot_id,
            provider_key, provider_model_id, task_class
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            2,
            1,
            'a7200000-0000-0000-0000-000000000003',
            1,
            'a7300000-0000-0000-0000-000000000003',
            'a7000000-0000-0000-0000-000000000001',
            1,
            'a7100000-0000-0000-0000-000000000002',
            'openai',
            'uncapped-contract-model',
            'module_read_reporting'
        )
    $statement$,
    'resolved route snapshot'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_provider_attempts (
            tenant_id, run_id, turn_index, attempt_index, route_set_id, route_version,
            route_target_id, connection_id, credential_version, model_snapshot_id,
            provider_key, provider_model_id, task_class, status, finished_at
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            1,
            1,
            'a7200000-0000-0000-0000-000000000001',
            1,
            'a7300000-0000-0000-0000-000000000001',
            'a7000000-0000-0000-0000-000000000001',
            1,
            'a7100000-0000-0000-0000-000000000001',
            'openai',
            'contract-model',
            'module_read_reporting',
            'succeeded',
            NOW()
        )
    $statement$,
    'must start in the running state'
);

WITH provider_clock AS (
    SELECT STATEMENT_TIMESTAMP() AS heartbeat_at
)
UPDATE agent_run_queue
SET checkpoint = 'provider_in_flight',
    heartbeat_at = provider_clock.heartbeat_at,
    lease_expires_at = provider_clock.heartbeat_at + INTERVAL '30 seconds',
    version = 4,
    updated_at = updated_at + INTERVAL '2 seconds'
FROM provider_clock
WHERE run_id = 'a5000000-0000-0000-0000-000000000002';

INSERT INTO agent_provider_attempts (
    id, tenant_id, run_id, turn_index, attempt_index, route_set_id, route_version,
    route_target_id, connection_id, credential_version, model_snapshot_id,
    provider_key, provider_model_id, task_class
)
VALUES (
    'a7400000-0000-0000-0000-000000000001',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000002',
    1,
    1,
    'a7200000-0000-0000-0000-000000000001',
    1,
    'a7300000-0000-0000-0000-000000000001',
    'a7000000-0000-0000-0000-000000000001',
    1,
    'a7100000-0000-0000-0000-000000000001',
    'openai',
    'contract-model',
    'module_read_reporting'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_execution_steps (
            tenant_id, run_id, step_index, turn_index, step_kind,
            provider_attempt_id, input_fingerprint
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            66,
            1,
            'provider_attempt',
            'a7400000-0000-0000-0000-000000000001',
            DECODE(REPEAT('10', 32), 'hex')
        )
    $statement$,
    'agent_execution_steps_step_index_check'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_execution_steps (
            tenant_id, run_id, step_index, turn_index, step_kind,
            provider_attempt_id, input_fingerprint
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            1,
            2,
            'provider_attempt',
            'a7400000-0000-0000-0000-000000000001',
            DECODE(REPEAT('10', 32), 'hex')
        )
    $statement$,
    'must match its running attempt'
);

INSERT INTO agent_execution_steps (
    id, tenant_id, run_id, step_index, turn_index, step_kind,
    provider_attempt_id, input_fingerprint
)
VALUES (
    'a7600000-0000-0000-0000-000000000001',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000002',
    65,
    1,
    'provider_attempt',
    'a7400000-0000-0000-0000-000000000001',
    DECODE(REPEAT('10', 32), 'hex')
);

SELECT pg_temp.assert_true(
    EXISTS (
        SELECT 1
        FROM agent_execution_steps
        WHERE id = 'a7600000-0000-0000-0000-000000000001'
          AND step_index = 65
    ),
    'the bounded execution trail must accept its sixty-fifth step'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_execution_artifacts (
            tenant_id, run_id, step_id, artifact_sequence, artifact_kind,
            ciphertext, ciphertext_sha256, plaintext_sha256, nonce, encryption_key_id,
            encryption_key_version, plaintext_length
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            'a7600000-0000-0000-0000-000000000001',
            2,
            'provider_result',
            DECODE(REPEAT('aa', 32), 'hex'),
            DECODE(REPEAT('bb', 32), 'hex'),
            DECODE(REPEAT('bc', 32), 'hex'),
            DECODE(REPEAT('cc', 12), 'hex'),
            'contract-agent-artifact-key',
            1,
            32
        )
    $statement$,
    'bounded run envelope'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_execution_artifacts (
            tenant_id, run_id, step_id, artifact_sequence, artifact_kind,
            ciphertext, ciphertext_sha256, plaintext_sha256, nonce, encryption_key_id,
            encryption_key_version, plaintext_length
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            'a7600000-0000-0000-0000-000000000001',
            1,
            'final_response',
            DECODE(REPEAT('aa', 32), 'hex'),
            DECODE(REPEAT('bb', 32), 'hex'),
            DECODE(REPEAT('bc', 32), 'hex'),
            DECODE(REPEAT('cc', 12), 'hex'),
            'contract-agent-artifact-key',
            1,
            32
        )
    $statement$,
    'must match its running step'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_execution_artifacts (
            tenant_id, run_id, step_id, artifact_sequence, artifact_kind,
            ciphertext, ciphertext_sha256, plaintext_sha256, nonce, encryption_key_id,
            encryption_key_version, plaintext_length
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            'a7600000-0000-0000-0000-000000000001',
            1,
            'provider_result',
            DECODE(REPEAT('aa', 32), 'hex'),
            DECODE(REPEAT('bb', 32), 'hex'),
            DECODE(REPEAT('bc', 32), 'hex'),
            DECODE(REPEAT('cc', 12), 'hex'),
            'contract-agent-artifact-key',
            1,
            65537
        )
    $statement$,
    'agent_execution_artifacts_plaintext_length_check'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_execution_artifacts (
            tenant_id, run_id, step_id, artifact_sequence, artifact_kind,
            ciphertext, ciphertext_sha256, plaintext_sha256, nonce, encryption_key_id,
            encryption_key_version, plaintext_length
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            'a7600000-0000-0000-0000-000000000001',
            1,
            'provider_result',
            DECODE(REPEAT('aa', 65553), 'hex'),
            DECODE(REPEAT('bb', 32), 'hex'),
            DECODE(REPEAT('bc', 32), 'hex'),
            DECODE(REPEAT('cc', 12), 'hex'),
            'contract-agent-artifact-key',
            1,
            65536
        )
    $statement$,
    'agent_execution_artifacts_ciphertext_check'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_execution_artifacts (
            tenant_id, run_id, step_id, artifact_sequence, artifact_kind,
            ciphertext, ciphertext_sha256, plaintext_sha256, nonce, encryption_key_id,
            encryption_key_version, plaintext_length
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            'a7600000-0000-0000-0000-000000000001',
            34,
            'provider_result',
            DECODE(REPEAT('aa', 32), 'hex'),
            DECODE(REPEAT('bb', 32), 'hex'),
            DECODE(REPEAT('bc', 32), 'hex'),
            DECODE(REPEAT('cc', 12), 'hex'),
            'contract-agent-artifact-key',
            1,
            32
        )
    $statement$,
    'bounded run envelope'
);

INSERT INTO agent_execution_artifacts (
    id, tenant_id, run_id, step_id, artifact_sequence, artifact_kind,
    ciphertext, ciphertext_sha256, plaintext_sha256, nonce, encryption_key_id,
    encryption_key_version, plaintext_length
)
VALUES (
    'a7700000-0000-0000-0000-000000000001',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000002',
    'a7600000-0000-0000-0000-000000000001',
    1,
    'provider_result',
    DECODE(REPEAT('aa', 32), 'hex'),
    DECODE(REPEAT('bb', 32), 'hex'),
    DECODE(REPEAT('bc', 32), 'hex'),
    DECODE(REPEAT('cc', 12), 'hex'),
    'contract-agent-artifact-key',
    1,
    32
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_execution_steps
        SET status = 'failed',
            safe_failure_code = 'provider.failed_after_result',
            finished_at = NOW(),
            updated_at = updated_at + INTERVAL '1 second'
        WHERE id = 'a7600000-0000-0000-0000-000000000001'
    $statement$,
    'cannot retain result artifacts'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_provider_attempts
        SET status = 'succeeded',
            input_tokens = 9007199254740992,
            finished_at = NOW(),
            updated_at = updated_at + INTERVAL '1 second'
        WHERE id = 'a7400000-0000-0000-0000-000000000001'
    $statement$,
    'agent_provider_attempts_input_tokens_check'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_runs
        SET status = 'failed',
            safe_failure_code = 'provider_still_running',
            safe_failure_message = 'A running child cannot be hidden by a terminal run.',
            finished_at = NOW(),
            version = 3,
            updated_at = updated_at + INTERVAL '2 seconds'
        WHERE id = 'a5000000-0000-0000-0000-000000000002'
    $statement$,
    'terminal execution children'
);

UPDATE agent_provider_attempts
SET status = 'succeeded',
    input_tokens = 10,
    output_tokens = 5,
    provider_reported_cost_amount = 12,
    provider_reported_cost_currency = 'USD',
    provider_reported_cost_exponent = 6,
    finished_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a7400000-0000-0000-0000-000000000001';

UPDATE agent_execution_steps
SET status = 'succeeded',
    finished_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a7600000-0000-0000-0000-000000000001';

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_provider_attempts (
            tenant_id, run_id, turn_index, attempt_index, route_set_id, route_version,
            route_target_id, connection_id, credential_version, model_snapshot_id,
            provider_key, provider_model_id, task_class
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            17,
            1,
            'a7200000-0000-0000-0000-000000000001',
            1,
            'a7300000-0000-0000-0000-000000000001',
            'a7000000-0000-0000-0000-000000000001',
            1,
            'a7100000-0000-0000-0000-000000000001',
            'openai',
            'contract-model',
            'module_read_reporting'
        )
    $statement$,
    'agent_provider_attempts_turn_index_check'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_provider_attempts (
            tenant_id, run_id, turn_index, attempt_index, route_set_id, route_version,
            route_target_id, connection_id, credential_version, model_snapshot_id,
            provider_key, provider_model_id, task_class
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            1,
            1,
            'a7200000-0000-0000-0000-000000000001',
            1,
            'a7300000-0000-0000-0000-000000000001',
            'a7000000-0000-0000-0000-000000000001',
            1,
            'a7100000-0000-0000-0000-000000000001',
            'openai',
            'contract-model',
            'module_read_reporting'
        )
    $statement$,
    'agent_provider_attempts_run_turn_index_unique'
);

INSERT INTO agent_provider_attempts (
    id, tenant_id, run_id, turn_index, attempt_index, route_set_id, route_version,
    route_target_id, connection_id, credential_version, model_snapshot_id,
    provider_key, provider_model_id, task_class
)
VALUES (
    'a7400000-0000-0000-0000-000000000002',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000002',
    1,
    2,
    'a7200000-0000-0000-0000-000000000001',
    1,
    'a7300000-0000-0000-0000-000000000001',
    'a7000000-0000-0000-0000-000000000001',
    1,
    'a7100000-0000-0000-0000-000000000001',
    'openai',
    'contract-model',
    'module_read_reporting'
);

INSERT INTO agent_execution_steps (
    id, tenant_id, run_id, step_index, turn_index, step_kind,
    provider_attempt_id, input_fingerprint
)
VALUES (
    'a7600000-0000-0000-0000-000000000002',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000002',
    2,
    1,
    'provider_attempt',
    'a7400000-0000-0000-0000-000000000002',
    DECODE(REPEAT('11', 32), 'hex')
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_provider_attempts
        SET status = 'failed',
            failure_origin = 'preflight',
            failure_category = 'authentication',
            finished_at = NOW(),
            updated_at = updated_at + INTERVAL '1 second'
        WHERE id = 'a7400000-0000-0000-0000-000000000002'
    $statement$,
    'agent_provider_attempts_failure_shape_check'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_provider_attempts
        SET status = 'failed',
            failure_origin = 'preflight',
            failure_category = 'invalid_input',
            input_tokens = 1,
            finished_at = NOW(),
            updated_at = updated_at + INTERVAL '1 second'
        WHERE id = 'a7400000-0000-0000-0000-000000000002'
    $statement$,
    'agent_provider_attempts_preflight_usage_check'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_provider_attempts
        SET status = 'succeeded',
            estimated_cost_amount = 10,
            estimated_cost_currency = 'USD',
            estimated_cost_exponent = 6,
            finished_at = NOW(),
            updated_at = updated_at + INTERVAL '1 second'
        WHERE id = 'a7400000-0000-0000-0000-000000000002'
    $statement$,
    'agent_provider_attempts_estimated_cost_shape_check'
);

UPDATE agent_provider_attempts
SET status = 'failed',
    failure_origin = 'preflight',
    failure_category = 'invalid_input',
    finished_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a7400000-0000-0000-0000-000000000002';

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_execution_steps
        SET status = 'succeeded',
            finished_at = NOW(),
            updated_at = updated_at + INTERVAL '1 second'
        WHERE id = 'a7600000-0000-0000-0000-000000000002'
    $statement$,
    'require one encrypted artifact'
);

UPDATE agent_execution_steps
SET status = 'failed',
    safe_failure_code = 'provider.invalid_input',
    finished_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a7600000-0000-0000-0000-000000000002';

INSERT INTO agent_provider_attempts (
    id, tenant_id, run_id, turn_index, attempt_index, route_set_id, route_version,
    route_target_id, connection_id, credential_version, model_snapshot_id,
    provider_key, provider_model_id, task_class
)
VALUES (
    'a7400000-0000-0000-0000-000000000003',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000002',
    1,
    3,
    'a7200000-0000-0000-0000-000000000001',
    1,
    'a7300000-0000-0000-0000-000000000001',
    'a7000000-0000-0000-0000-000000000001',
    1,
    'a7100000-0000-0000-0000-000000000001',
    'openai',
    'contract-model',
    'module_read_reporting'
);

INSERT INTO agent_execution_steps (
    id, tenant_id, run_id, step_index, turn_index, step_kind,
    provider_attempt_id, input_fingerprint
)
VALUES (
    'a7600000-0000-0000-0000-000000000004',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000002',
    4,
    1,
    'provider_attempt',
    'a7400000-0000-0000-0000-000000000003',
    DECODE(REPEAT('13', 32), 'hex')
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_provider_attempts
        SET status = 'failed',
            failure_origin = 'upstream',
            failure_category = 'invalid_input',
            finished_at = NOW(),
            updated_at = updated_at + INTERVAL '1 second'
        WHERE id = 'a7400000-0000-0000-0000-000000000003'
    $statement$,
    'agent_provider_attempts_failure_shape_check'
);

UPDATE agent_provider_attempts
SET status = 'failed',
    failure_origin = 'upstream',
    failure_category = 'timeout',
    input_tokens = 3,
    estimated_cost_amount = 2,
    estimated_cost_currency = 'USD',
    estimated_cost_exponent = 6,
    estimated_pricing_version = 'contract-estimate-v1',
    finished_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a7400000-0000-0000-0000-000000000003';

UPDATE agent_execution_steps
SET status = 'failed',
    safe_failure_code = 'provider.timeout',
    finished_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a7600000-0000-0000-0000-000000000004';

UPDATE ai_provider_connections
SET credential_version = 2,
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a7000000-0000-0000-0000-000000000001';

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_provider_attempts (
            tenant_id, run_id, turn_index, attempt_index, route_set_id, route_version,
            route_target_id, connection_id, credential_version, model_snapshot_id,
            provider_key, provider_model_id, task_class
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            1,
            2,
            'a7200000-0000-0000-0000-000000000001',
            1,
            'a7300000-0000-0000-0000-000000000001',
            'a7000000-0000-0000-0000-000000000001',
            2,
            'a7100000-0000-0000-0000-000000000001',
            'openai',
            'contract-model',
            'module_read_reporting'
        )
    $statement$,
    'resolved route snapshot'
);

UPDATE ai_provider_connections
SET credential_version = 1,
    model_catalog_version = 2,
    updated_at = updated_at + INTERVAL '2 seconds'
WHERE id = 'a7000000-0000-0000-0000-000000000001';

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_provider_attempts (
            tenant_id, run_id, turn_index, attempt_index, route_set_id, route_version,
            route_target_id, connection_id, credential_version, model_snapshot_id,
            provider_key, provider_model_id, task_class
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            1,
            2,
            'a7200000-0000-0000-0000-000000000001',
            1,
            'a7300000-0000-0000-0000-000000000001',
            'a7000000-0000-0000-0000-000000000001',
            1,
            'a7100000-0000-0000-0000-000000000001',
            'openai',
            'contract-model',
            'module_read_reporting'
        )
    $statement$,
    'resolved route snapshot'
);

UPDATE ai_provider_connections
SET model_catalog_version = 1,
    updated_at = updated_at + INTERVAL '3 seconds'
WHERE id = 'a7000000-0000-0000-0000-000000000001';

INSERT INTO ai_route_sets (
    id, tenant_id, scope_kind, task_class, configured_by, change_reason
)
VALUES (
    'a7200000-0000-0000-0000-000000000002',
    'a0000000-0000-0000-0000-000000000001',
    'task_class',
    'document_extraction',
    'a1000000-0000-0000-0000-000000000001',
    'Mismatched task route contract'
);

INSERT INTO ai_task_routes (
    id, tenant_id, route_set_id, priority, connection_id, model_id,
    requires_tools, created_by
)
VALUES (
    'a7300000-0000-0000-0000-000000000002',
    'a0000000-0000-0000-0000-000000000001',
    'a7200000-0000-0000-0000-000000000002',
    1,
    'a7000000-0000-0000-0000-000000000001',
    'a7100000-0000-0000-0000-000000000001',
    TRUE,
    'a1000000-0000-0000-0000-000000000001'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_provider_attempts (
            tenant_id, run_id, turn_index, attempt_index, route_set_id, route_version,
            route_target_id, connection_id, credential_version, model_snapshot_id,
            provider_key, provider_model_id, task_class
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            1,
            2,
            'a7200000-0000-0000-0000-000000000002',
            1,
            'a7300000-0000-0000-0000-000000000002',
            'a7000000-0000-0000-0000-000000000001',
            1,
            'a7100000-0000-0000-0000-000000000001',
            'openai',
            'contract-model',
            'module_read_reporting'
        )
    $statement$,
    'resolved route snapshot'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_provider_attempts
        SET output_tokens = 6, updated_at = updated_at + INTERVAL '1 second'
        WHERE id = 'a7400000-0000-0000-0000-000000000001'
    $statement$,
    'terminal Agent provider attempts are immutable'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_capability_calls (
            id, tenant_id, run_id, call_sequence, capability_key, capability_version,
            product_operation_key, owning_module_key, required_permission,
            input_fingerprint, scope_kind, resource_references,
            status, duration_ms, finished_at
        ) VALUES (
            gen_random_uuid(),
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            1,
            'fleet.vehicles.list',
            1,
            'fleet.vehicles.list',
            'fleet',
            'fleet:view',
            DECODE(REPEAT('01', 32), 'hex'),
            'tenant_wide',
            '[]'::JSONB,
            'succeeded',
            1,
            NOW()
        )
    $statement$,
    'must start in the running state'
);

INSERT INTO agent_capability_calls (
    id, tenant_id, run_id, call_sequence, capability_key, capability_version,
    product_operation_key, owning_module_key, required_permission,
    input_fingerprint, scope_kind, resource_references
)
VALUES (
    'a7500000-0000-0000-0000-000000000001',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000002',
    1,
    'fleet.vehicles.list',
    1,
    'fleet.vehicles.list',
    'fleet',
    'fleet:view',
    DECODE(REPEAT('01', 32), 'hex'),
    'tenant_wide',
    '[]'::JSONB
);

INSERT INTO agent_execution_steps (
    id, tenant_id, run_id, step_index, turn_index, step_kind,
    capability_call_id, input_fingerprint
)
VALUES (
    'a7600000-0000-0000-0000-000000000003',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000002',
    3,
    1,
    'capability_call',
    'a7500000-0000-0000-0000-000000000001',
    DECODE(REPEAT('12', 32), 'hex')
);

INSERT INTO agent_execution_artifacts (
    id, tenant_id, run_id, step_id, artifact_sequence, artifact_kind,
    ciphertext, ciphertext_sha256, plaintext_sha256, nonce, encryption_key_id,
    encryption_key_version, plaintext_length
)
VALUES (
    'a7700000-0000-0000-0000-000000000002',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000002',
    'a7600000-0000-0000-0000-000000000003',
    2,
    'capability_result',
    DECODE(REPEAT('ad', 32), 'hex'),
    DECODE(REPEAT('bd', 32), 'hex'),
    DECODE(REPEAT('be', 32), 'hex'),
    DECODE(REPEAT('cd', 12), 'hex'),
    'contract-agent-artifact-key',
    1,
    32
);

UPDATE agent_capability_calls
SET status = 'succeeded',
    duration_ms = 25,
    finished_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a7500000-0000-0000-0000-000000000001';

UPDATE agent_execution_steps
SET status = 'succeeded',
    finished_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a7600000-0000-0000-0000-000000000003';

SELECT pg_temp.expect_failure(
    $statement$
        WITH consecutive_capability_call AS (
            INSERT INTO agent_capability_calls (
                id, tenant_id, run_id, call_sequence, capability_key,
                capability_version, product_operation_key, owning_module_key,
                required_permission, input_fingerprint, scope_kind,
                resource_references
            ) VALUES (
                'a7500000-0000-0000-0000-000000000099',
                'a0000000-0000-0000-0000-000000000001',
                'a5000000-0000-0000-0000-000000000002',
                2,
                'fleet.drivers.list',
                1,
                'fleet.drivers.list',
                'fleet',
                'fleet:view',
                DECODE(REPEAT('60', 32), 'hex'),
                'tenant_wide',
                '[]'::JSONB
            )
            RETURNING id
        )
        INSERT INTO agent_execution_steps (
            id, tenant_id, run_id, step_index, turn_index, step_kind,
            capability_call_id, input_fingerprint
        )
        SELECT
            'a7600000-0000-0000-0000-000000000099',
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            5,
            1,
            'capability_call',
            id,
            DECODE(REPEAT('61', 32), 'hex')
        FROM consecutive_capability_call
    $statement$,
    'agent_execution_steps_capability_turn_unique'
);

INSERT INTO agent_capability_calls (
    id, tenant_id, run_id, call_sequence, capability_key, capability_version,
    product_operation_key, owning_module_key, required_permission,
    input_fingerprint, scope_kind, resource_references
)
VALUES (
    'a7500000-0000-0000-0000-000000000011',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000002',
    2,
    'fleet.drivers.list',
    1,
    'fleet.drivers.list',
    'fleet',
    'fleet:view',
    DECODE(REPEAT('62', 32), 'hex'),
    'tenant_wide',
    '[]'::JSONB
);

INSERT INTO agent_execution_steps (
    id, tenant_id, run_id, step_index, turn_index, step_kind,
    capability_call_id, input_fingerprint
)
VALUES (
    'a7600000-0000-0000-0000-000000000011',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000002',
    5,
    2,
    'capability_call',
    'a7500000-0000-0000-0000-000000000011',
    DECODE(REPEAT('63', 32), 'hex')
);

SELECT pg_temp.expect_failure(
    $statement$
        DO $atomic_interrupted_artifact$
        BEGIN
            INSERT INTO agent_execution_artifacts (
                tenant_id, run_id, step_id, artifact_sequence, artifact_kind,
                ciphertext, ciphertext_sha256, plaintext_sha256, nonce,
                encryption_key_id, encryption_key_version, plaintext_length
            ) VALUES (
                'a0000000-0000-0000-0000-000000000001',
                'a5000000-0000-0000-0000-000000000002',
                'a7600000-0000-0000-0000-000000000011',
                3,
                'capability_result',
                DECODE(REPEAT('64', 32), 'hex'),
                DECODE(REPEAT('65', 32), 'hex'),
                DECODE(REPEAT('66', 32), 'hex'),
                DECODE(REPEAT('67', 12), 'hex'),
                'contract-agent-artifact-key',
                1,
                32
            );

            UPDATE agent_capability_calls
            SET status = 'interrupted',
                safe_failure_code = 'capability.worker_interrupted',
                duration_ms = 10,
                finished_at = NOW(),
                updated_at = updated_at + INTERVAL '1 second'
            WHERE id = 'a7500000-0000-0000-0000-000000000011';

            UPDATE agent_execution_steps
            SET status = 'interrupted',
                safe_failure_code = 'capability.worker_interrupted',
                finished_at = NOW(),
                updated_at = updated_at + INTERVAL '1 second'
            WHERE id = 'a7600000-0000-0000-0000-000000000011';
        END
        $atomic_interrupted_artifact$
    $statement$,
    'non-recoverable Agent execution steps cannot retain result artifacts'
);

UPDATE agent_capability_calls
SET status = 'interrupted',
    safe_failure_code = 'capability.worker_interrupted',
    duration_ms = 10,
    finished_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a7500000-0000-0000-0000-000000000011';

UPDATE agent_execution_steps
SET status = 'interrupted',
    safe_failure_code = 'capability.worker_interrupted',
    finished_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a7600000-0000-0000-0000-000000000011';

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_execution_artifacts (
            tenant_id, run_id, step_id, artifact_sequence, artifact_kind,
            ciphertext, ciphertext_sha256, plaintext_sha256, nonce,
            encryption_key_id, encryption_key_version, plaintext_length
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            'a7600000-0000-0000-0000-000000000011',
            3,
            'capability_result',
            DECODE(REPEAT('64', 32), 'hex'),
            DECODE(REPEAT('65', 32), 'hex'),
            DECODE(REPEAT('66', 32), 'hex'),
            DECODE(REPEAT('67', 12), 'hex'),
            'contract-agent-artifact-key',
            1,
            32
        )
    $statement$,
    'must match its running step'
);

INSERT INTO agent_capability_calls (
    id, tenant_id, run_id, call_sequence, capability_key, capability_version,
    product_operation_key, owning_module_key, required_permission,
    input_fingerprint, scope_kind, resource_references
)
VALUES (
    'a7500000-0000-0000-0000-000000000012',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000002',
    3,
    'fleet.vehicles.read',
    1,
    'fleet.vehicles.read',
    'fleet',
    'fleet:view',
    DECODE(REPEAT('68', 32), 'hex'),
    'tenant_wide',
    '[]'::JSONB
);

INSERT INTO agent_execution_steps (
    id, tenant_id, run_id, step_index, turn_index, step_kind,
    capability_call_id, input_fingerprint
)
VALUES (
    'a7600000-0000-0000-0000-000000000012',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000002',
    6,
    3,
    'capability_call',
    'a7500000-0000-0000-0000-000000000012',
    DECODE(REPEAT('69', 32), 'hex')
);

UPDATE agent_capability_calls
SET status = 'failed',
    safe_failure_code = 'capability.operation_failed',
    duration_ms = 12,
    finished_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a7500000-0000-0000-0000-000000000012';

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_execution_steps
        SET status = 'failed',
            safe_failure_code = 'capability.operation_failed',
            finished_at = NOW(),
            updated_at = updated_at + INTERVAL '1 second'
        WHERE id = 'a7600000-0000-0000-0000-000000000012'
    $statement$,
    'recoverable Agent execution steps require one encrypted artifact'
);

INSERT INTO agent_execution_artifacts (
    id, tenant_id, run_id, step_id, artifact_sequence, artifact_kind,
    ciphertext, ciphertext_sha256, plaintext_sha256, nonce, encryption_key_id,
    encryption_key_version, plaintext_length
)
VALUES (
    'a7700000-0000-0000-0000-000000000012',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000002',
    'a7600000-0000-0000-0000-000000000012',
    3,
    'capability_result',
    DECODE(REPEAT('6a', 32), 'hex'),
    DECODE(REPEAT('6b', 32), 'hex'),
    DECODE(REPEAT('6c', 32), 'hex'),
    DECODE(REPEAT('6d', 12), 'hex'),
    'contract-agent-artifact-key',
    1,
    32
);

UPDATE agent_execution_steps
SET status = 'failed',
    safe_failure_code = 'capability.operation_failed',
    finished_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a7600000-0000-0000-0000-000000000012';

INSERT INTO agent_capability_calls (
    id, tenant_id, run_id, call_sequence, capability_key, capability_version,
    product_operation_key, owning_module_key, required_permission,
    input_fingerprint, scope_kind, resource_references
)
VALUES (
    'a7500000-0000-0000-0000-000000000013',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000002',
    4,
    'fleet.vehicles.update',
    1,
    'fleet.vehicles.update',
    'fleet',
    'fleet:update',
    DECODE(REPEAT('6e', 32), 'hex'),
    'tenant_wide',
    '[]'::JSONB
);

INSERT INTO agent_execution_steps (
    id, tenant_id, run_id, step_index, turn_index, step_kind,
    capability_call_id, input_fingerprint
)
VALUES (
    'a7600000-0000-0000-0000-000000000013',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000002',
    7,
    4,
    'capability_call',
    'a7500000-0000-0000-0000-000000000013',
    DECODE(REPEAT('6f', 32), 'hex')
);

INSERT INTO agent_execution_artifacts (
    id, tenant_id, run_id, step_id, artifact_sequence, artifact_kind,
    ciphertext, ciphertext_sha256, plaintext_sha256, nonce, encryption_key_id,
    encryption_key_version, plaintext_length
)
VALUES (
    'a7700000-0000-0000-0000-000000000013',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000002',
    'a7600000-0000-0000-0000-000000000013',
    4,
    'capability_result',
    DECODE(REPEAT('70', 32), 'hex'),
    DECODE(REPEAT('71', 32), 'hex'),
    DECODE(REPEAT('72', 32), 'hex'),
    DECODE(REPEAT('73', 12), 'hex'),
    'contract-agent-artifact-key',
    1,
    32
);

UPDATE agent_capability_calls
SET status = 'denied',
    safe_failure_code = 'capability.permission_denied',
    duration_ms = 15,
    finished_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a7500000-0000-0000-0000-000000000013';

UPDATE agent_execution_steps
SET status = 'failed',
    safe_failure_code = 'capability.permission_denied',
    finished_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a7600000-0000-0000-0000-000000000013';

SELECT pg_temp.assert_true(
    (
        SELECT COUNT(*) = 2
        FROM agent_execution_artifacts
        WHERE run_id = 'a5000000-0000-0000-0000-000000000002'
          AND step_id IN (
              'a7600000-0000-0000-0000-000000000012',
              'a7600000-0000-0000-0000-000000000013'
          )
          AND artifact_kind = 'capability_result'
    ),
    'failed and denied capability outcomes must retain encrypted replay artifacts'
);

SELECT pg_temp.expect_failure(
    $statement$
        DELETE FROM agent_capability_calls
        WHERE id = 'a7500000-0000-0000-0000-000000000001'
    $statement$,
    'Agent capability calls are retained, not deleted'
);

SELECT pg_temp.expect_failure(
    $statement$
        DELETE FROM agent_execution_artifacts
        WHERE id = 'a7700000-0000-0000-0000-000000000001'
    $statement$,
    'append-only'
);

WITH provider_result_clock AS (
    SELECT STATEMENT_TIMESTAMP() AS heartbeat_at
)
UPDATE agent_run_queue
SET checkpoint = 'provider_result_persisted',
    heartbeat_at = provider_result_clock.heartbeat_at,
    lease_expires_at = provider_result_clock.heartbeat_at + INTERVAL '30 seconds',
    version = 5,
    updated_at = updated_at + INTERVAL '3 seconds'
FROM provider_result_clock
WHERE run_id = 'a5000000-0000-0000-0000-000000000002';

UPDATE agent_run_queue
SET checkpoint = 'capability_in_flight',
    version = 6,
    updated_at = updated_at + INTERVAL '1 second'
WHERE run_id = 'a5000000-0000-0000-0000-000000000002';

UPDATE agent_run_queue
SET checkpoint = 'capability_result_persisted',
    version = 7,
    updated_at = updated_at + INTERVAL '1 second'
WHERE run_id = 'a5000000-0000-0000-0000-000000000002';

-- Advance only the fixture clock so expiry recovery can be exercised without sleeping.
ALTER TABLE agent_run_queue DISABLE TRIGGER agent_run_queue_protect_lifecycle;
UPDATE agent_run_queue
SET heartbeat_at = STATEMENT_TIMESTAMP() - INTERVAL '40 seconds',
    lease_expires_at = STATEMENT_TIMESTAMP() - INTERVAL '10 seconds'
WHERE run_id = 'a5000000-0000-0000-0000-000000000002';
ALTER TABLE agent_run_queue ENABLE TRIGGER agent_run_queue_protect_lifecycle;

UPDATE agent_run_queue
SET state = 'available',
    lease_token = NULL,
    leased_by = NULL,
    lease_expires_at = NULL,
    heartbeat_at = NULL,
    available_at = NOW(),
    version = 8,
    updated_at = updated_at + INTERVAL '4 seconds'
WHERE run_id = 'a5000000-0000-0000-0000-000000000002';

WITH reclaim_clock AS (
    SELECT STATEMENT_TIMESTAMP() AS claimed_at
)
UPDATE agent_run_queue
SET state = 'leased',
    lease_token = 'a6000000-0000-0000-0000-000000000002',
    leased_by = 'contract-worker',
    heartbeat_at = reclaim_clock.claimed_at,
    lease_expires_at = reclaim_clock.claimed_at + INTERVAL '30 seconds',
    delivery_attempt = 2,
    version = 9,
    updated_at = updated_at + INTERVAL '5 seconds'
FROM reclaim_clock
WHERE run_id = 'a5000000-0000-0000-0000-000000000002';

SELECT pg_temp.assert_true(
    (
        SELECT COUNT(*) = 2
        FROM agent_execution_artifacts
        WHERE run_id = 'a5000000-0000-0000-0000-000000000002'
          AND step_id IN (
              'a7600000-0000-0000-0000-000000000012',
              'a7600000-0000-0000-0000-000000000013'
          )
    ),
    'reclaimed capability results must retain exact encrypted failure evidence'
);

WITH second_heartbeat_clock AS (
    SELECT STATEMENT_TIMESTAMP() AS heartbeat_at
)
UPDATE agent_run_queue
SET checkpoint = 'before_provider',
    heartbeat_at = second_heartbeat_clock.heartbeat_at,
    lease_expires_at = second_heartbeat_clock.heartbeat_at + INTERVAL '30 seconds',
    version = 10,
    updated_at = updated_at + INTERVAL '6 seconds'
FROM second_heartbeat_clock
WHERE run_id = 'a5000000-0000-0000-0000-000000000002';

WITH second_provider_clock AS (
    SELECT STATEMENT_TIMESTAMP() AS heartbeat_at
)
UPDATE agent_run_queue
SET checkpoint = 'provider_in_flight',
    heartbeat_at = second_provider_clock.heartbeat_at,
    lease_expires_at = second_provider_clock.heartbeat_at + INTERVAL '30 seconds',
    version = 11,
    updated_at = updated_at + INTERVAL '7 seconds'
FROM second_provider_clock
WHERE run_id = 'a5000000-0000-0000-0000-000000000002';

ALTER TABLE agent_run_queue DISABLE TRIGGER agent_run_queue_protect_lifecycle;
UPDATE agent_run_queue
SET heartbeat_at = STATEMENT_TIMESTAMP() - INTERVAL '40 seconds',
    lease_expires_at = STATEMENT_TIMESTAMP() - INTERVAL '10 seconds'
WHERE run_id = 'a5000000-0000-0000-0000-000000000002';
ALTER TABLE agent_run_queue ENABLE TRIGGER agent_run_queue_protect_lifecycle;

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_run_queue
        SET state = 'available',
            lease_token = NULL,
            leased_by = NULL,
            lease_expires_at = NULL,
            heartbeat_at = NULL,
            version = 12,
            updated_at = updated_at + INTERVAL '8 seconds'
        WHERE run_id = 'a5000000-0000-0000-0000-000000000002'
    $statement$,
    'unsafe Agent queue work cannot be reclaimed'
);

UPDATE agent_run_queue
SET state = 'finished',
    lease_token = NULL,
    leased_by = NULL,
    lease_expires_at = NULL,
    heartbeat_at = NULL,
    finished_at = NOW(),
    version = 12,
    updated_at = updated_at + INTERVAL '8 seconds'
WHERE run_id = 'a5000000-0000-0000-0000-000000000002';

UPDATE agent_runs
SET status = 'interrupted',
    safe_failure_code = 'ambiguous_provider_acceptance',
    safe_failure_message = 'The provider result could not be recovered safely.',
    finished_at = NOW(),
    version = 3,
    updated_at = updated_at + INTERVAL '2 seconds'
WHERE id = 'a5000000-0000-0000-0000-000000000002';

UPDATE agent_threads
SET next_message_sequence = 5,
    version = 5,
    last_activity_at = last_activity_at + INTERVAL '1 second',
    updated_at = updated_at + INTERVAL '6 seconds'
WHERE id = 'a3000000-0000-0000-0000-000000000001';

INSERT INTO agent_messages (
    id, tenant_id, thread_id, sequence, role, user_id, content
)
VALUES (
    'a4000000-0000-0000-0000-000000000004',
    'a0000000-0000-0000-0000-000000000001',
    'a3000000-0000-0000-0000-000000000001',
    4,
    'user',
    'a1000000-0000-0000-0000-000000000001',
    'List active fleet records again.'
);

INSERT INTO agent_runs (
    id, tenant_id, thread_id, request_message_id, requested_by, task_class,
    origin_module_key, origin_route, request_id, correlation_id
)
VALUES (
    'a5000000-0000-0000-0000-000000000003',
    'a0000000-0000-0000-0000-000000000001',
    'a3000000-0000-0000-0000-000000000001',
    'a4000000-0000-0000-0000-000000000004',
    'a1000000-0000-0000-0000-000000000001',
    'module_read_reporting',
    'fleet',
    '/modules/fleet',
    'a5100000-0000-0000-0000-000000000003',
    'a5200000-0000-0000-0000-000000000003'
);

INSERT INTO agent_run_queue (run_id, tenant_id)
VALUES (
    'a5000000-0000-0000-0000-000000000003',
    'a0000000-0000-0000-0000-000000000001'
);

-- Seed two prior deliveries, then exercise the real third claim and exhausted recovery edge.
ALTER TABLE agent_run_queue DISABLE TRIGGER agent_run_queue_protect_lifecycle;
UPDATE agent_run_queue
SET delivery_attempt = 2
WHERE run_id = 'a5000000-0000-0000-0000-000000000003';
ALTER TABLE agent_run_queue ENABLE TRIGGER agent_run_queue_protect_lifecycle;

WITH exhausted_claim_clock AS (
    SELECT STATEMENT_TIMESTAMP() AS claimed_at
)
UPDATE agent_run_queue
SET state = 'leased',
    lease_token = 'a6000000-0000-0000-0000-000000000003',
    leased_by = 'contract-worker',
    heartbeat_at = exhausted_claim_clock.claimed_at,
    lease_expires_at = exhausted_claim_clock.claimed_at + INTERVAL '30 seconds',
    delivery_attempt = 3,
    version = 2,
    updated_at = updated_at + INTERVAL '1 second'
FROM exhausted_claim_clock
WHERE run_id = 'a5000000-0000-0000-0000-000000000003';

ALTER TABLE agent_run_queue DISABLE TRIGGER agent_run_queue_protect_lifecycle;
UPDATE agent_run_queue
SET heartbeat_at = STATEMENT_TIMESTAMP() - INTERVAL '40 seconds',
    lease_expires_at = STATEMENT_TIMESTAMP() - INTERVAL '10 seconds'
WHERE run_id = 'a5000000-0000-0000-0000-000000000003';
ALTER TABLE agent_run_queue ENABLE TRIGGER agent_run_queue_protect_lifecycle;

UPDATE agent_run_queue
SET state = 'finished',
    lease_token = NULL,
    leased_by = NULL,
    lease_expires_at = NULL,
    heartbeat_at = NULL,
    finished_at = NOW(),
    version = 3,
    updated_at = updated_at + INTERVAL '2 seconds'
WHERE run_id = 'a5000000-0000-0000-0000-000000000003';

SELECT pg_temp.assert_true(
    (
        SELECT state = 'finished' AND delivery_attempt = 3
        FROM agent_run_queue
        WHERE run_id = 'a5000000-0000-0000-0000-000000000003'
    ),
    'expired safe checkpoint terminalizes after the third delivery'
);

UPDATE agent_runs
SET status = 'running',
    started_at = NOW(),
    version = 2,
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a5000000-0000-0000-0000-000000000003';

INSERT INTO agent_execution_steps (
    id, tenant_id, run_id, step_index, turn_index, step_kind, input_fingerprint
)
VALUES (
    'a7600000-0000-0000-0000-000000000020',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000003',
    1,
    1,
    'finalize',
    DECODE(REPEAT('30', 32), 'hex')
);

INSERT INTO agent_execution_artifacts (
    id, tenant_id, run_id, step_id, artifact_sequence, artifact_kind,
    ciphertext, ciphertext_sha256, plaintext_sha256, nonce, encryption_key_id,
    encryption_key_version, plaintext_length
)
VALUES (
    'a7700000-0000-0000-0000-000000000020',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000003',
    'a7600000-0000-0000-0000-000000000020',
    1,
    'final_response',
    DECODE(REPEAT('31', 32), 'hex'),
    DECODE(REPEAT('32', 32), 'hex'),
    DECODE(REPEAT('33', 32), 'hex'),
    DECODE(REPEAT('34', 12), 'hex'),
    'contract-agent-artifact-key',
    1,
    32
);

UPDATE agent_execution_steps
SET status = 'succeeded',
    finished_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a7600000-0000-0000-0000-000000000020';

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_runs
        SET status = 'completed',
            response_message_id = 'a4000000-0000-0000-0000-000000000003',
            finished_at = NOW(),
            version = 3,
            updated_at = updated_at + INTERVAL '2 seconds'
        WHERE id = 'a5000000-0000-0000-0000-000000000003'
    $statement$,
    'agent_runs_response_message_unique'
);

UPDATE agent_runs
SET status = 'failed',
    safe_failure_code = 'contract_cleanup',
    safe_failure_message = 'Contract run closed after uniqueness proof.',
    finished_at = NOW(),
    version = 3,
    updated_at = updated_at + INTERVAL '2 seconds'
WHERE id = 'a5000000-0000-0000-0000-000000000003';

UPDATE agent_threads
SET next_message_sequence = 6,
    version = 6,
    last_activity_at = last_activity_at + INTERVAL '1 second',
    updated_at = updated_at + INTERVAL '7 seconds'
WHERE id = 'a3000000-0000-0000-0000-000000000001';

INSERT INTO agent_messages (
    id, tenant_id, thread_id, sequence, role, user_id, content
)
VALUES (
    'a4000000-0000-0000-0000-000000000005',
    'a0000000-0000-0000-0000-000000000001',
    'a3000000-0000-0000-0000-000000000001',
    5,
    'user',
    'a1000000-0000-0000-0000-000000000001',
    'Create durable final response evidence.'
);

INSERT INTO agent_runs (
    id, tenant_id, thread_id, request_message_id, requested_by, task_class,
    origin_module_key, origin_route, request_id, correlation_id
)
VALUES (
    'a5000000-0000-0000-0000-000000000004',
    'a0000000-0000-0000-0000-000000000001',
    'a3000000-0000-0000-0000-000000000001',
    'a4000000-0000-0000-0000-000000000005',
    'a1000000-0000-0000-0000-000000000001',
    'module_read_reporting',
    'fleet',
    '/modules/fleet',
    'a5100000-0000-0000-0000-000000000004',
    'a5200000-0000-0000-0000-000000000004'
);

INSERT INTO agent_run_queue (run_id, tenant_id)
VALUES (
    'a5000000-0000-0000-0000-000000000004',
    'a0000000-0000-0000-0000-000000000001'
);

WITH final_claim_clock AS (
    SELECT STATEMENT_TIMESTAMP() AS claimed_at
)
UPDATE agent_run_queue
SET state = 'leased',
    lease_token = 'a6000000-0000-0000-0000-000000000004',
    leased_by = 'contract-worker',
    heartbeat_at = final_claim_clock.claimed_at,
    lease_expires_at = final_claim_clock.claimed_at + INTERVAL '30 seconds',
    delivery_attempt = 1,
    version = 2,
    updated_at = updated_at + INTERVAL '1 second'
FROM final_claim_clock
WHERE run_id = 'a5000000-0000-0000-0000-000000000004';

UPDATE agent_runs
SET status = 'running',
    started_at = NOW(),
    version = 2,
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a5000000-0000-0000-0000-000000000004';

UPDATE agent_run_queue
SET checkpoint = 'before_provider', version = 3,
    updated_at = updated_at + INTERVAL '1 second'
WHERE run_id = 'a5000000-0000-0000-0000-000000000004';

UPDATE agent_run_queue
SET checkpoint = 'provider_in_flight', version = 4,
    updated_at = updated_at + INTERVAL '1 second'
WHERE run_id = 'a5000000-0000-0000-0000-000000000004';

UPDATE agent_run_queue
SET checkpoint = 'provider_result_persisted', version = 5,
    updated_at = updated_at + INTERVAL '1 second'
WHERE run_id = 'a5000000-0000-0000-0000-000000000004';

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_run_queue
        SET checkpoint = 'finalizing', version = 6,
            updated_at = updated_at + INTERVAL '1 second'
        WHERE run_id = 'a5000000-0000-0000-0000-000000000004'
    $statement$,
    'durable finalization evidence'
);

INSERT INTO agent_execution_steps (
    id, tenant_id, run_id, step_index, turn_index, step_kind, input_fingerprint
)
VALUES (
    'a7600000-0000-0000-0000-000000000030',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000004',
    1,
    1,
    'finalize',
    DECODE(REPEAT('40', 32), 'hex')
);

INSERT INTO agent_execution_artifacts (
    id, tenant_id, run_id, step_id, artifact_sequence, artifact_kind,
    ciphertext, ciphertext_sha256, plaintext_sha256, nonce, encryption_key_id,
    encryption_key_version, plaintext_length
)
VALUES (
    'a7700000-0000-0000-0000-000000000030',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000004',
    'a7600000-0000-0000-0000-000000000030',
    1,
    'final_response',
    DECODE(REPEAT('41', 32), 'hex'),
    DECODE(REPEAT('42', 32), 'hex'),
    DECODE(REPEAT('43', 32), 'hex'),
    DECODE(REPEAT('44', 12), 'hex'),
    'contract-agent-artifact-key',
    1,
    32
);

UPDATE agent_execution_steps
SET status = 'succeeded', finished_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a7600000-0000-0000-0000-000000000030';

UPDATE agent_run_queue
SET checkpoint = 'finalizing', version = 6,
    updated_at = updated_at + INTERVAL '1 second'
WHERE run_id = 'a5000000-0000-0000-0000-000000000004';

ALTER TABLE agent_run_queue DISABLE TRIGGER agent_run_queue_protect_lifecycle;
UPDATE agent_run_queue
SET heartbeat_at = STATEMENT_TIMESTAMP() - INTERVAL '40 seconds',
    lease_expires_at = STATEMENT_TIMESTAMP() - INTERVAL '10 seconds'
WHERE run_id = 'a5000000-0000-0000-0000-000000000004';
ALTER TABLE agent_run_queue ENABLE TRIGGER agent_run_queue_protect_lifecycle;

UPDATE agent_run_queue
SET state = 'available',
    lease_token = NULL,
    leased_by = NULL,
    lease_expires_at = NULL,
    heartbeat_at = NULL,
    available_at = NOW(),
    version = 7,
    updated_at = updated_at + INTERVAL '1 second'
WHERE run_id = 'a5000000-0000-0000-0000-000000000004';

WITH final_reclaim_clock AS (
    SELECT STATEMENT_TIMESTAMP() AS claimed_at
)
UPDATE agent_run_queue
SET state = 'leased',
    lease_token = 'a6000000-0000-0000-0000-000000000005',
    leased_by = 'contract-worker',
    heartbeat_at = final_reclaim_clock.claimed_at,
    lease_expires_at = final_reclaim_clock.claimed_at + INTERVAL '30 seconds',
    delivery_attempt = 2,
    version = 8,
    updated_at = updated_at + INTERVAL '1 second'
FROM final_reclaim_clock
WHERE run_id = 'a5000000-0000-0000-0000-000000000004';

SELECT pg_temp.assert_true(
    (
        SELECT state = 'leased' AND checkpoint = 'finalizing' AND delivery_attempt = 2
        FROM agent_run_queue
        WHERE run_id = 'a5000000-0000-0000-0000-000000000004'
    ),
    'durable finalizing work is safely recoverable'
);

UPDATE agent_threads
SET next_message_sequence = 7,
    version = 7,
    last_activity_at = last_activity_at + INTERVAL '1 second',
    updated_at = updated_at + INTERVAL '8 seconds'
WHERE id = 'a3000000-0000-0000-0000-000000000001';

INSERT INTO agent_messages (
    id, tenant_id, thread_id, sequence, role, content
)
VALUES (
    'a4000000-0000-0000-0000-000000000006',
    'a0000000-0000-0000-0000-000000000001',
    'a3000000-0000-0000-0000-000000000001',
    6,
    'assistant',
    'The durable response was recovered.'
);

UPDATE agent_runs
SET status = 'completed',
    response_message_id = 'a4000000-0000-0000-0000-000000000006',
    finished_at = NOW(),
    version = 3,
    updated_at = updated_at + INTERVAL '2 seconds'
WHERE id = 'a5000000-0000-0000-0000-000000000004';

UPDATE agent_run_queue
SET state = 'finished',
    lease_token = NULL,
    leased_by = NULL,
    lease_expires_at = NULL,
    heartbeat_at = NULL,
    finished_at = NOW(),
    version = 9,
    updated_at = updated_at + INTERVAL '1 second'
WHERE run_id = 'a5000000-0000-0000-0000-000000000004';

UPDATE agent_threads
SET next_message_sequence = 8,
    version = 8,
    last_activity_at = last_activity_at + INTERVAL '1 second',
    updated_at = updated_at + INTERVAL '9 seconds'
WHERE id = 'a3000000-0000-0000-0000-000000000001';

INSERT INTO agent_messages (
    id, tenant_id, thread_id, sequence, role, user_id, content
)
VALUES (
    'a4000000-0000-0000-0000-000000000007',
    'a0000000-0000-0000-0000-000000000001',
    'a3000000-0000-0000-0000-000000000001',
    7,
    'user',
    'a1000000-0000-0000-0000-000000000001',
    'Cancel the in-flight capability cooperatively.'
);

INSERT INTO agent_runs (
    id, tenant_id, thread_id, request_message_id, requested_by, task_class,
    origin_module_key, origin_route, request_id, correlation_id
)
VALUES (
    'a5000000-0000-0000-0000-000000000005',
    'a0000000-0000-0000-0000-000000000001',
    'a3000000-0000-0000-0000-000000000001',
    'a4000000-0000-0000-0000-000000000007',
    'a1000000-0000-0000-0000-000000000001',
    'module_read_reporting',
    'fleet',
    '/modules/fleet',
    'a5100000-0000-0000-0000-000000000005',
    'a5200000-0000-0000-0000-000000000005'
);

INSERT INTO agent_run_queue (run_id, tenant_id)
VALUES (
    'a5000000-0000-0000-0000-000000000005',
    'a0000000-0000-0000-0000-000000000001'
);

WITH cancel_claim_clock AS (
    SELECT STATEMENT_TIMESTAMP() AS claimed_at
)
UPDATE agent_run_queue
SET state = 'leased',
    lease_token = 'a6000000-0000-0000-0000-000000000006',
    leased_by = 'contract-worker',
    heartbeat_at = cancel_claim_clock.claimed_at,
    lease_expires_at = cancel_claim_clock.claimed_at + INTERVAL '30 seconds',
    delivery_attempt = 1,
    version = 2,
    updated_at = updated_at + INTERVAL '1 second'
FROM cancel_claim_clock
WHERE run_id = 'a5000000-0000-0000-0000-000000000005';

UPDATE agent_runs
SET status = 'running', started_at = NOW(), version = 2,
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a5000000-0000-0000-0000-000000000005';

UPDATE agent_run_queue
SET checkpoint = 'before_provider', version = 3,
    updated_at = updated_at + INTERVAL '1 second'
WHERE run_id = 'a5000000-0000-0000-0000-000000000005';

UPDATE agent_run_queue
SET checkpoint = 'provider_in_flight', version = 4,
    updated_at = updated_at + INTERVAL '1 second'
WHERE run_id = 'a5000000-0000-0000-0000-000000000005';

UPDATE agent_run_queue
SET checkpoint = 'provider_result_persisted', version = 5,
    updated_at = updated_at + INTERVAL '1 second'
WHERE run_id = 'a5000000-0000-0000-0000-000000000005';

UPDATE agent_run_queue
SET checkpoint = 'capability_in_flight', version = 6,
    updated_at = updated_at + INTERVAL '1 second'
WHERE run_id = 'a5000000-0000-0000-0000-000000000005';

INSERT INTO agent_capability_calls (
    id, tenant_id, run_id, call_sequence, capability_key, capability_version,
    product_operation_key, owning_module_key, required_permission,
    input_fingerprint, scope_kind, resource_references
)
VALUES (
    'a7500000-0000-0000-0000-000000000002',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000005',
    1,
    'fleet.vehicles.list',
    1,
    'fleet.vehicles.list',
    'fleet',
    'fleet:view',
    DECODE(REPEAT('50', 32), 'hex'),
    'tenant_wide',
    '[]'::JSONB
);

INSERT INTO agent_execution_steps (
    id, tenant_id, run_id, step_index, turn_index, step_kind,
    capability_call_id, input_fingerprint
)
VALUES (
    'a7600000-0000-0000-0000-000000000040',
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000005',
    1,
    1,
    'capability_call',
    'a7500000-0000-0000-0000-000000000002',
    DECODE(REPEAT('51', 32), 'hex')
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_run_queue
        SET cancel_requested_at = STATEMENT_TIMESTAMP(),
            cancel_requested_by = 'a1000000-0000-0000-0000-000000000002',
            updated_at = updated_at + INTERVAL '1 second'
        WHERE run_id = 'a5000000-0000-0000-0000-000000000005'
    $statement$,
    'invalid cooperative Agent cancellation request'
);

WITH cancellation_clock AS (
    SELECT STATEMENT_TIMESTAMP() AS requested_at
)
UPDATE agent_run_queue
SET cancel_requested_at = cancellation_clock.requested_at,
    cancel_requested_by = 'a1000000-0000-0000-0000-000000000001',
    updated_at = updated_at + INTERVAL '1 second'
FROM cancellation_clock
WHERE run_id = 'a5000000-0000-0000-0000-000000000005';

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_capability_calls (
            id, tenant_id, run_id, call_sequence, capability_key, capability_version,
            product_operation_key, owning_module_key, required_permission,
            input_fingerprint, scope_kind, resource_references
        ) VALUES (
            gen_random_uuid(),
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000005',
            2,
            'fleet.drivers.list',
            1,
            'fleet.drivers.list',
            'fleet',
            'fleet:view',
            DECODE(REPEAT('52', 32), 'hex'),
            'tenant_wide',
            '[]'::JSONB
        )
    $statement$,
    'active uncancelled run lease'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_run_queue
        SET state = 'finished',
            lease_token = NULL,
            leased_by = NULL,
            lease_expires_at = NULL,
            heartbeat_at = NULL,
            finished_at = NOW(),
            version = 7,
            updated_at = updated_at + INTERVAL '1 second'
        WHERE run_id = 'a5000000-0000-0000-0000-000000000005'
    $statement$,
    'invalid Agent queue finish'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_runs
        SET status = 'cancelled', finished_at = NOW(), version = 3,
            updated_at = updated_at + INTERVAL '1 second'
        WHERE id = 'a5000000-0000-0000-0000-000000000005'
    $statement$,
    'terminal execution children'
);

SELECT pg_temp.expect_failure(
    $statement$
        DO $atomic_cancelled_artifact$
        BEGIN
            INSERT INTO agent_execution_artifacts (
                tenant_id, run_id, step_id, artifact_sequence, artifact_kind,
                ciphertext, ciphertext_sha256, plaintext_sha256, nonce,
                encryption_key_id, encryption_key_version, plaintext_length
            ) VALUES (
                'a0000000-0000-0000-0000-000000000001',
                'a5000000-0000-0000-0000-000000000005',
                'a7600000-0000-0000-0000-000000000040',
                1,
                'capability_result',
                DECODE(REPEAT('74', 32), 'hex'),
                DECODE(REPEAT('75', 32), 'hex'),
                DECODE(REPEAT('76', 32), 'hex'),
                DECODE(REPEAT('77', 12), 'hex'),
                'contract-agent-artifact-key',
                1,
                32
            );

            UPDATE agent_capability_calls
            SET status = 'cancelled',
                duration_ms = 10,
                finished_at = NOW(),
                updated_at = updated_at + INTERVAL '1 second'
            WHERE id = 'a7500000-0000-0000-0000-000000000002';

            UPDATE agent_execution_steps
            SET status = 'cancelled',
                finished_at = NOW(),
                updated_at = updated_at + INTERVAL '1 second'
            WHERE id = 'a7600000-0000-0000-0000-000000000040';
        END
        $atomic_cancelled_artifact$
    $statement$,
    'non-recoverable Agent execution steps cannot retain result artifacts'
);

UPDATE agent_capability_calls
SET status = 'cancelled', duration_ms = 10, finished_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a7500000-0000-0000-0000-000000000002';

UPDATE agent_execution_steps
SET status = 'cancelled', finished_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a7600000-0000-0000-0000-000000000040';

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_execution_artifacts (
            tenant_id, run_id, step_id, artifact_sequence, artifact_kind,
            ciphertext, ciphertext_sha256, plaintext_sha256, nonce,
            encryption_key_id, encryption_key_version, plaintext_length
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000005',
            'a7600000-0000-0000-0000-000000000040',
            1,
            'capability_result',
            DECODE(REPEAT('74', 32), 'hex'),
            DECODE(REPEAT('75', 32), 'hex'),
            DECODE(REPEAT('76', 32), 'hex'),
            DECODE(REPEAT('77', 12), 'hex'),
            'contract-agent-artifact-key',
            1,
            32
        )
    $statement$,
    'must match its running step'
);

UPDATE agent_run_queue
SET checkpoint = 'capability_result_persisted', version = 7,
    updated_at = updated_at + INTERVAL '1 second'
WHERE run_id = 'a5000000-0000-0000-0000-000000000005';

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_run_queue
        SET checkpoint = 'before_provider', version = 8,
            updated_at = updated_at + INTERVAL '1 second'
        WHERE run_id = 'a5000000-0000-0000-0000-000000000005'
    $statement$,
    'stops new work'
);

UPDATE agent_run_queue
SET state = 'finished',
    lease_token = NULL,
    leased_by = NULL,
    lease_expires_at = NULL,
    heartbeat_at = NULL,
    finished_at = NOW(),
    version = 8,
    updated_at = updated_at + INTERVAL '1 second'
WHERE run_id = 'a5000000-0000-0000-0000-000000000005';

UPDATE agent_runs
SET status = 'cancelled', finished_at = NOW(), version = 3,
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = 'a5000000-0000-0000-0000-000000000005';

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_run_events (tenant_id, run_id, event_type, payload)
        VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000003',
            'started',
            '{}'::JSONB
        )
    $statement$,
    'event type does not match'
);

INSERT INTO agent_run_events (tenant_id, run_id, event_type, payload)
VALUES (
    'a0000000-0000-0000-0000-000000000001',
    'a5000000-0000-0000-0000-000000000002',
    'interrupted',
    '{}'::JSONB
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_run_events (tenant_id, run_id, event_type, payload)
        VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            'interrupted',
            '{"details":{"safe":"still not persisted"}}'::JSONB
        )
    $statement$,
    'agent_run_events_payload_check'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_run_events (tenant_id, run_id, event_type, payload)
        VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            'interrupted',
            '{"response":"not persisted"}'::JSONB
        )
    $statement$,
    'agent_run_events_payload_check'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_run_events (tenant_id, run_id, event_type, payload)
        VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            'interrupted',
            '{"result":"not persisted"}'::JSONB
        )
    $statement$,
    'agent_run_events_payload_check'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_run_events (tenant_id, run_id, event_type, payload)
        VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            'interrupted',
            '{"data":"not persisted"}'::JSONB
        )
    $statement$,
    'agent_run_events_payload_check'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_run_events (tenant_id, run_id, event_type, payload)
        VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            'interrupted',
            '{"headers":{"x":"not persisted"}}'::JSONB
        )
    $statement$,
    'agent_run_events_payload_check'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_run_events (tenant_id, run_id, event_type, payload)
        VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a5000000-0000-0000-0000-000000000002',
            'interrupted',
            '{"arguments":{"x":"not persisted"}}'::JSONB
        )
    $statement$,
    'agent_run_events_payload_check'
);

SELECT pg_temp.expect_failure(
    $statement$
        DELETE FROM agent_run_events
        WHERE run_id = 'a5000000-0000-0000-0000-000000000002'
    $statement$,
    'append-only'
);

INSERT INTO agent_request_idempotency (
    id, tenant_id, user_id, operation_key, scope_id, idempotency_key,
    request_fingerprint, result_kind, result_id
)
VALUES (
    'a8000000-0000-0000-0000-000000000001',
    'a0000000-0000-0000-0000-000000000001',
    'a1000000-0000-0000-0000-000000000001',
    'agent.messages.submit',
    'a3000000-0000-0000-0000-000000000001',
    'contract-idempotency-key',
    DECODE(REPEAT('02', 32), 'hex'),
    'run',
    'a5000000-0000-0000-0000-000000000002'
);

INSERT INTO agent_request_idempotency (
    id, tenant_id, user_id, operation_key, scope_id, idempotency_key,
    request_fingerprint, result_kind, result_id
)
VALUES (
    'a8000000-0000-0000-0000-000000000002',
    'a0000000-0000-0000-0000-000000000001',
    'a1000000-0000-0000-0000-000000000001',
    'agent.sessions.create',
    NULL,
    'contract-session-key',
    DECODE(REPEAT('05', 32), 'hex'),
    'thread',
    'a3000000-0000-0000-0000-000000000001'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_request_idempotency (
            tenant_id, user_id, operation_key, scope_id, idempotency_key,
            request_fingerprint, result_kind, result_id
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a1000000-0000-0000-0000-000000000001',
            'agent.sessions.create',
            NULL,
            'contract-session-key',
            DECODE(REPEAT('05', 32), 'hex'),
            'thread',
            'a3000000-0000-0000-0000-000000000001'
        )
    $statement$,
    'agent_request_idempotency_key_unique'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_request_idempotency (
            tenant_id, user_id, operation_key, scope_id, idempotency_key,
            request_fingerprint, result_kind, result_id
        ) VALUES (
            'a0000000-0000-0000-0000-000000000001',
            'a1000000-0000-0000-0000-000000000001',
            'agent.messages.submit',
            'a3000000-0000-0000-0000-000000000001',
            'contract-idempotency-key',
            DECODE(REPEAT('03', 32), 'hex'),
            'run',
            'a5000000-0000-0000-0000-000000000002'
        )
    $statement$,
    'agent_request_idempotency_key_unique'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_request_idempotency
        SET request_fingerprint = DECODE(REPEAT('04', 32), 'hex')
        WHERE id = 'a8000000-0000-0000-0000-000000000001'
    $statement$,
    'append-only'
);

INSERT INTO actor_audit_events (
    id, tenant_id, actor_type, actor_user_id, action_key, outcome,
    request_id, correlation_id, agent_run_id
)
VALUES (
    'a9000000-0000-0000-0000-000000000001',
    'a0000000-0000-0000-0000-000000000001',
    'agent',
    'a1000000-0000-0000-0000-000000000001',
    'agent.runs.interrupt',
    'failed',
    'a5100000-0000-0000-0000-000000000002',
    'a5200000-0000-0000-0000-000000000002',
    'a5000000-0000-0000-0000-000000000002'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO actor_audit_events (
            tenant_id, actor_type, actor_user_id, action_key, outcome,
            request_id, correlation_id, agent_run_id
        ) VALUES (
            'b0000000-0000-0000-0000-000000000001',
            'agent',
            'b1000000-0000-0000-0000-000000000001',
            'agent.runs.read',
            'denied',
            gen_random_uuid(),
            gen_random_uuid(),
            'a5000000-0000-0000-0000-000000000002'
        )
    $statement$,
    'actor_audit_events_agent_run_tenant_fk'
);

SELECT pg_temp.expect_failure(
    $statement$
        DELETE FROM actor_audit_events
        WHERE id = 'a9000000-0000-0000-0000-000000000001'
    $statement$,
    'append-only'
);

SELECT pg_temp.expect_failure(
    'TRUNCATE agent_run_events',
    'append-only'
);

SELECT pg_temp.expect_failure(
    'TRUNCATE actor_audit_events',
    'append-only'
);

SELECT pg_temp.expect_failure(
    'TRUNCATE agent_runs CASCADE',
    'append-only'
);

SELECT pg_temp.expect_failure(
    'TRUNCATE agent_execution_steps CASCADE',
    'append-only'
);

SELECT pg_temp.expect_failure(
    'TRUNCATE agent_execution_artifacts',
    'append-only'
);

UPDATE agent_threads
SET status = 'archived',
    version = 9,
    updated_at = updated_at + INTERVAL '10 seconds'
WHERE id = 'a3000000-0000-0000-0000-000000000001';

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_threads
        SET title = 'Changed', version = 10, updated_at = updated_at + INTERVAL '1 second'
        WHERE id = 'a3000000-0000-0000-0000-000000000001'
    $statement$,
    'archived Agent Sessions are immutable'
);

SELECT pg_temp.expect_failure(
    $statement$
        DELETE FROM agent_threads
        WHERE id = 'a3000000-0000-0000-0000-000000000001'
    $statement$,
    'archived, not deleted'
);

SELECT pg_temp.assert_true(
    (
        SELECT COUNT(*)
        FROM agent_provider_attempts
        WHERE status = 'succeeded' AND input_tokens IS NULL
    ) = 0,
    'successful provider token counts remain explicit in the contract fixture'
);

SELECT pg_temp.assert_true(
    (SELECT COUNT(*) FROM agent_provider_attempts WHERE estimated_cost_amount IS NULL) = 2,
    'unknown estimated cost remains NULL'
);

SELECT pg_temp.assert_true(
    (
        SELECT input_tokens IS NULL
           AND output_tokens IS NULL
           AND provider_reported_cost_amount IS NULL
           AND estimated_cost_amount IS NULL
        FROM agent_provider_attempts
        WHERE id = 'a7400000-0000-0000-0000-000000000002'
    ),
    'preflight failures never fabricate usage or cost'
);

ROLLBACK;
