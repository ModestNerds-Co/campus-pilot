-- Adversarial contract for migration 092. The caller applies migrations first.
-- Every rejected-intent fixture and assertion is rolled back.

\set ON_ERROR_STOP on

CREATE EXTENSION IF NOT EXISTS dblink;

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

-- Replay must preserve the ledger and exactly one mutual-exclusion guard.
\ir ../092_create_agent_capability_rejection_evidence.sql

SELECT pg_temp.assert_true(
    (
        SELECT COUNT(*) = 1
        FROM pg_trigger
        WHERE tgrelid = 'agent_capability_calls'::REGCLASS
          AND tgname = 'agent_capability_calls_rejection_guard'
          AND NOT tgisinternal
    ),
    '092 replay must retain exactly one executable-call rejection guard'
);

INSERT INTO tenants (id, slug, name)
VALUES
    ('92000000-0000-0000-0000-000000000001', 'agent-092-a', 'Agent 092 A'),
    ('a2000000-0000-0000-0000-000000000001', 'agent-092-b', 'Agent 092 B');

INSERT INTO users (id, tenant_id, email, password_hash, full_name)
VALUES
    (
        '92100000-0000-0000-0000-000000000001',
        '92000000-0000-0000-0000-000000000001',
        'owner-a@agent-092.test', 'test-only', 'Owner A'
    ),
    (
        'a2100000-0000-0000-0000-000000000001',
        'a2000000-0000-0000-0000-000000000001',
        'owner-b@agent-092.test', 'test-only', 'Owner B'
    );

INSERT INTO agent_threads (id, tenant_id, owner_user_id)
VALUES (
    '92200000-0000-0000-0000-000000000001',
    '92000000-0000-0000-0000-000000000001',
    '92100000-0000-0000-0000-000000000001'
);

INSERT INTO agent_thread_members (
    id, tenant_id, thread_id, user_id, membership_role, added_by
) VALUES (
    '92300000-0000-0000-0000-000000000001',
    '92000000-0000-0000-0000-000000000001',
    '92200000-0000-0000-0000-000000000001',
    '92100000-0000-0000-0000-000000000001',
    'owner',
    '92100000-0000-0000-0000-000000000001'
);

UPDATE agent_threads
SET next_message_sequence = 2,
    version = 2,
    last_activity_at = last_activity_at + INTERVAL '1 second',
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = '92200000-0000-0000-0000-000000000001';

INSERT INTO agent_messages (
    id, tenant_id, thread_id, sequence, role, user_id, content
) VALUES (
    '92400000-0000-0000-0000-000000000001',
    '92000000-0000-0000-0000-000000000001',
    '92200000-0000-0000-0000-000000000001',
    1,
    'user',
    '92100000-0000-0000-0000-000000000001',
    'Attempt one capability.'
);

INSERT INTO agent_runs (
    id, tenant_id, thread_id, request_message_id, requested_by, task_class,
    origin_module_key, origin_route, request_id, correlation_id
) VALUES (
    '92500000-0000-0000-0000-000000000001',
    '92000000-0000-0000-0000-000000000001',
    '92200000-0000-0000-0000-000000000001',
    '92400000-0000-0000-0000-000000000001',
    '92100000-0000-0000-0000-000000000001',
    'module_read_reporting',
    'fleet',
    '/modules/fleet',
    '92510000-0000-0000-0000-000000000001',
    '92520000-0000-0000-0000-000000000001'
);

INSERT INTO agent_run_queue (run_id, tenant_id)
VALUES (
    '92500000-0000-0000-0000-000000000001',
    '92000000-0000-0000-0000-000000000001'
);

UPDATE agent_run_queue
SET state = 'leased',
    lease_token = '92600000-0000-0000-0000-000000000001',
    leased_by = 'agent-092-worker',
    heartbeat_at = STATEMENT_TIMESTAMP(),
    lease_expires_at = STATEMENT_TIMESTAMP() + INTERVAL '30 seconds',
    delivery_attempt = 1,
    version = 2,
    updated_at = STATEMENT_TIMESTAMP()
WHERE run_id = '92500000-0000-0000-0000-000000000001';

UPDATE agent_run_queue
SET checkpoint = 'before_provider', version = 3,
    updated_at = CLOCK_TIMESTAMP()
WHERE run_id = '92500000-0000-0000-0000-000000000001';

UPDATE agent_run_queue
SET checkpoint = 'provider_in_flight', version = 4,
    updated_at = CLOCK_TIMESTAMP()
WHERE run_id = '92500000-0000-0000-0000-000000000001';

UPDATE agent_run_queue
SET checkpoint = 'provider_result_persisted', version = 5,
    updated_at = CLOCK_TIMESTAMP()
WHERE run_id = '92500000-0000-0000-0000-000000000001';

UPDATE agent_runs
SET status = 'running',
    started_at = NOW(),
    version = 2,
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = '92500000-0000-0000-0000-000000000001';

-- A new rejection requires the exact current worker, raw token, and fence.
SELECT pg_temp.expect_failure(
    $statement$
        SELECT record_agent_capability_rejection(
            '92700000-0000-0000-0000-000000000005',
            '92000000-0000-0000-0000-000000000001',
            '92500000-0000-0000-0000-000000000001', 5::SMALLINT,
            '92100000-0000-0000-0000-000000000001',
            '92700000-0000-0000-0000-000000000005',
            '92520000-0000-0000-0000-000000000001',
            'unknown.capability', 1, NULL::BYTEA,
            NULL::TEXT, NULL::TEXT, NULL::TEXT, NULL::TEXT, NULL::JSONB,
            'denied', 'unknown_capability', 'unknown_capability',
            'The requested capability does not exist.',
            'wrong-worker',
            '92600000-0000-0000-0000-000000000001'::UUID, 5::BIGINT
        )
    $statement$,
    'exact current run lease'
);

SELECT pg_temp.expect_failure(
    $statement$
        SELECT record_agent_capability_rejection(
            '92700000-0000-0000-0000-000000000005',
            '92000000-0000-0000-0000-000000000001',
            '92500000-0000-0000-0000-000000000001', 5::SMALLINT,
            '92100000-0000-0000-0000-000000000001',
            '92700000-0000-0000-0000-000000000005',
            '92520000-0000-0000-0000-000000000001',
            'unknown.capability', 1, NULL::BYTEA,
            NULL::TEXT, NULL::TEXT, NULL::TEXT, NULL::TEXT, NULL::JSONB,
            'denied', 'unknown_capability', 'unknown_capability',
            'The requested capability does not exist.',
            'agent-092-worker',
            '92600000-0000-0000-0000-000000000099'::UUID, 5::BIGINT
        )
    $statement$,
    'exact current run lease'
);

SELECT pg_temp.expect_failure(
    $statement$
        SELECT record_agent_capability_rejection(
            '92700000-0000-0000-0000-000000000005',
            '92000000-0000-0000-0000-000000000001',
            '92500000-0000-0000-0000-000000000001', 5::SMALLINT,
            '92100000-0000-0000-0000-000000000001',
            '92700000-0000-0000-0000-000000000005',
            '92520000-0000-0000-0000-000000000001',
            'unknown.capability', 1, NULL::BYTEA,
            NULL::TEXT, NULL::TEXT, NULL::TEXT, NULL::TEXT, NULL::JSONB,
            'denied', 'unknown_capability', 'unknown_capability',
            'The requested capability does not exist.',
            'agent-092-worker',
            '92600000-0000-0000-0000-000000000001'::UUID, 4::BIGINT
        )
    $statement$,
    'exact current run lease'
);

-- Unknown capability: operation and scope are both honestly unavailable.
SELECT pg_temp.assert_true(
    record_agent_capability_rejection(
        '92700000-0000-0000-0000-000000000001',
        '92000000-0000-0000-0000-000000000001',
        '92500000-0000-0000-0000-000000000001',
        1::SMALLINT,
        '92100000-0000-0000-0000-000000000001',
        '92700000-0000-0000-0000-000000000001',
        '92520000-0000-0000-0000-000000000001',
        'unknown.capability',
        1,
        DECODE(REPEAT('31', 32), 'hex'),
        NULL::TEXT,
        NULL::TEXT,
        NULL::TEXT,
        NULL::TEXT,
        NULL::JSONB,
        'denied',
        'unknown_capability',
        'unknown_capability',
        'The requested capability does not exist.',
        'agent-092-worker',
        '92600000-0000-0000-0000-000000000001'::UUID,
        5::BIGINT
    ) = '92700000-0000-0000-0000-000000000001',
    'first rejection insert must return its trusted capability call identity'
);

SELECT pg_temp.assert_true(
    (
        SELECT product_operation_key IS NULL
           AND owning_module_key IS NULL
           AND required_permission IS NULL
           AND scope_kind IS NULL
           AND resource_references IS NULL
           AND normalized_input_digest_sha256 = DECODE(REPEAT('31', 32), 'hex')
           AND request_id = capability_call_id
           AND claimed_by_worker_id = 'agent-092-worker'
           AND claim_fence_version = 5
           AND rejected_at = created_at
           AND updated_at = created_at
        FROM agent_capability_rejections
        WHERE capability_call_id = '92700000-0000-0000-0000-000000000001'
    ),
    'unknown preparation facts must remain NULL rather than tenant-wide defaults'
);

SELECT pg_temp.assert_true(
    (
        SELECT disposition = 'rejected'
        FROM agent_capability_intent_registry
        WHERE capability_call_id = '92700000-0000-0000-0000-000000000001'
          AND run_id = '92500000-0000-0000-0000-000000000001'
          AND call_sequence = 1
    ),
    'the cross-ledger identity fence must terminally claim rejected disposition'
);

-- Exact replay is idempotent and cannot create a second event.
SELECT pg_temp.assert_true(
    record_agent_capability_rejection(
        '92700000-0000-0000-0000-000000000001',
        '92000000-0000-0000-0000-000000000001',
        '92500000-0000-0000-0000-000000000001',
        1::SMALLINT,
        '92100000-0000-0000-0000-000000000001',
        '92700000-0000-0000-0000-000000000001',
        '92520000-0000-0000-0000-000000000001',
        'unknown.capability',
        1,
        DECODE(REPEAT('31', 32), 'hex'),
        NULL::TEXT, NULL::TEXT, NULL::TEXT, NULL::TEXT, NULL::JSONB,
        'denied', 'unknown_capability', 'unknown_capability',
        'The requested capability does not exist.',
        'replacement-worker',
        '92600000-0000-0000-0000-000000000099'::UUID,
        999::BIGINT
    ) = '92700000-0000-0000-0000-000000000001'
    AND (
        SELECT COUNT(*) = 1
        FROM agent_capability_rejections
        WHERE run_id = '92500000-0000-0000-0000-000000000001'
          AND call_sequence = 1
    ),
    'terminal exact replay must not depend on the original live lease'
);

SELECT pg_temp.expect_failure(
    $statement$
        SELECT record_agent_capability_rejection(
            '92700000-0000-0000-0000-000000000001',
            '92000000-0000-0000-0000-000000000001',
            '92500000-0000-0000-0000-000000000001', 1::SMALLINT,
            '92100000-0000-0000-0000-000000000001',
            '92700000-0000-0000-0000-000000000001',
            '92520000-0000-0000-0000-000000000001',
            'unknown.capability', 1, DECODE(REPEAT('31', 32), 'hex'),
            NULL::TEXT, NULL::TEXT, NULL::TEXT, NULL::TEXT, NULL::JSONB,
            'denied', 'unknown_capability', 'changed_reason',
            'The requested capability does not exist.',
            'agent-092-worker',
            '92600000-0000-0000-0000-000000000001'::UUID,
            5::BIGINT
        )
    $statement$,
    'idempotency conflict'
);

SELECT pg_temp.expect_failure(
    $statement$
        SELECT record_agent_capability_rejection(
            '92700000-0000-0000-0000-000000000009',
            '92000000-0000-0000-0000-000000000001',
            '92500000-0000-0000-0000-000000000001', 1::SMALLINT,
            '92100000-0000-0000-0000-000000000001',
            '92700000-0000-0000-0000-000000000009',
            '92520000-0000-0000-0000-000000000001',
            'unknown.capability', 1, NULL::BYTEA,
            NULL::TEXT, NULL::TEXT, NULL::TEXT, NULL::TEXT, NULL::JSONB,
            'denied', 'unknown_capability', 'unknown_capability',
            'The requested capability does not exist.',
            'agent-092-worker',
            '92600000-0000-0000-0000-000000000001'::UUID,
            5::BIGINT
        )
    $statement$,
    'idempotency conflict'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_capability_rejections (
            capability_call_id, tenant_id, run_id, call_sequence,
            actor_user_id, request_id, correlation_id, capability_key,
            claimed_by_worker_id, claim_fence_version,
            capability_version, product_operation_key, outcome,
            broker_error_code, reason_code, safe_message
        ) VALUES (
            '92700000-0000-0000-0000-000000000002',
            '92000000-0000-0000-0000-000000000001',
            '92500000-0000-0000-0000-000000000001', 2,
            '92100000-0000-0000-0000-000000000001',
            '92710000-0000-0000-0000-000000000002',
            '92520000-0000-0000-0000-000000000001',
            'fleet.vehicles.list', 'agent-092-worker', 5,
            1, 'fleet.vehicles.list',
            'denied', 'access_denied', 'permission_missing', 'Access denied.'
        )
    $statement$,
    'require the fenced recorder'
);

SELECT pg_temp.expect_failure(
    $statement$
        SELECT record_agent_capability_rejection(
            '92700000-0000-0000-0000-000000000002',
            '92000000-0000-0000-0000-000000000001',
            '92500000-0000-0000-0000-000000000001', 2::SMALLINT,
            '92100000-0000-0000-0000-000000000001',
            '92700000-0000-0000-0000-000000000002',
            '92520000-0000-0000-0000-000000000001',
            'fleet.vehicles.list', 1, DECODE(REPEAT('32', 32), 'hex'),
            'fleet.vehicles.list', 'fleet', 'fleet:view',
            'tenant_wide', NULL::JSONB,
            'denied', 'record_scope_denied', 'record_scope_denied',
            'The requested records are outside the current access scope.',
            'agent-092-worker',
            '92600000-0000-0000-0000-000000000001'::UUID,
            5::BIGINT
        )
    $statement$,
    'scope_shape_check'
);

-- Operation resolution does not imply scope resolution. Input-too-large is
-- recorded with exact operation evidence while digest and scope remain NULL.
SELECT record_agent_capability_rejection(
    '92700000-0000-0000-0000-000000000002',
    '92000000-0000-0000-0000-000000000001',
    '92500000-0000-0000-0000-000000000001',
    2::SMALLINT,
    '92100000-0000-0000-0000-000000000001',
    '92700000-0000-0000-0000-000000000002',
    '92520000-0000-0000-0000-000000000001',
    'fleet.vehicles.list',
    1,
    NULL::BYTEA,
    'fleet.vehicles.list',
    'fleet',
    'fleet:view',
    NULL::TEXT,
    NULL::JSONB,
    'denied',
    'input_too_large',
    'input_too_large',
    'Capability input exceeds the supported size.',
    'agent-092-worker',
    '92600000-0000-0000-0000-000000000001'::UUID,
    5::BIGINT
);

SELECT pg_temp.assert_true(
    (
        SELECT normalized_input_digest_sha256 IS NULL
           AND product_operation_key = 'fleet.vehicles.list'
           AND owning_module_key = 'fleet'
           AND required_permission = 'fleet:view'
           AND scope_kind IS NULL
           AND resource_references IS NULL
        FROM agent_capability_rejections
        WHERE capability_call_id = '92700000-0000-0000-0000-000000000002'
    ),
    'operation evidence must not fabricate normalized input or scope evidence'
);

SELECT record_agent_capability_rejection(
    '92700000-0000-0000-0000-000000000003',
    '92000000-0000-0000-0000-000000000001',
    '92500000-0000-0000-0000-000000000001',
    3::SMALLINT,
    '92100000-0000-0000-0000-000000000001',
    '92700000-0000-0000-0000-000000000003',
    '92520000-0000-0000-0000-000000000001',
    'fleet.vehicles.read',
    1,
    DECODE(REPEAT('33', 32), 'hex'),
    'fleet.vehicles.read',
    'fleet',
    'fleet:view',
    'resources',
    '[{"kind":"vehicle","id":"92730000-0000-0000-0000-000000000003"}]'::JSONB,
    'denied',
    'record_scope_denied',
    'record_scope_denied',
    'The requested records are outside the current access scope.',
    'agent-092-worker',
    '92600000-0000-0000-0000-000000000001'::UUID,
    5::BIGINT
);

SELECT pg_temp.assert_true(
    (
        SELECT scope_kind = 'resources'
           AND JSONB_ARRAY_LENGTH(resource_references) = 1
        FROM agent_capability_rejections
        WHERE capability_call_id = '92700000-0000-0000-0000-000000000003'
    ),
    'known record scope must retain the exact bounded resource evidence'
);

SELECT pg_temp.expect_failure(
    $statement$
        SELECT record_agent_capability_rejection(
            '92700000-0000-0000-0000-000000000004',
            '92000000-0000-0000-0000-000000000001',
            '92500000-0000-0000-0000-000000000001', 4::SMALLINT,
            '92100000-0000-0000-0000-000000000001',
            '92700000-0000-0000-0000-000000000004',
            '92520000-0000-0000-0000-000000000001',
            'fleet.vehicles.list', 1, DECODE(REPEAT('34', 32), 'hex'),
            'fleet.vehicles.list', 'fleet', 'fleet:view',
            NULL::TEXT, NULL::JSONB,
            'denied', 'authority_unavailable',
            'authority_unavailable', 'Current access could not be loaded.',
            'agent-092-worker',
            '92600000-0000-0000-0000-000000000001'::UUID,
            5::BIGINT
        )
    $statement$,
    'outcome_code_check'
);

SELECT pg_temp.expect_failure(
    $statement$
        SELECT record_agent_capability_rejection(
            '92700000-0000-0000-0000-000000000004',
            '92000000-0000-0000-0000-000000000001',
            '92500000-0000-0000-0000-000000000001', 4::SMALLINT,
            'a2100000-0000-0000-0000-000000000001',
            '92700000-0000-0000-0000-000000000004',
            '92520000-0000-0000-0000-000000000001',
            'fleet.vehicles.list', 1, NULL::BYTEA,
            'fleet.vehicles.list', 'fleet', 'fleet:view',
            NULL::TEXT, NULL::JSONB, 'failed', 'authority_unavailable',
            'authority_unavailable', 'Current access could not be loaded.',
            'agent-092-worker',
            '92600000-0000-0000-0000-000000000001'::UUID,
            5::BIGINT
        )
    $statement$,
    'invalid run evidence'
);

-- Rejected intent and executable call ledgers are mutually exclusive in both
-- insertion orders.
SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_capability_calls (
            id, tenant_id, run_id, call_sequence, capability_key,
            capability_version, product_operation_key, owning_module_key,
            required_permission, input_fingerprint, scope_kind,
            resource_references
        ) VALUES (
            '92700000-0000-0000-0000-000000000001',
            '92000000-0000-0000-0000-000000000001',
            '92500000-0000-0000-0000-000000000001', 1,
            'fleet.vehicles.list', 1, 'fleet.vehicles.list', 'fleet',
            'fleet:view', DECODE(REPEAT('41', 32), 'hex'),
            'tenant_wide', '[]'::JSONB
        )
    $statement$,
    'intent disposition conflict'
);

INSERT INTO agent_capability_calls (
    id, tenant_id, run_id, call_sequence, capability_key,
    capability_version, product_operation_key, owning_module_key,
    required_permission, input_fingerprint, scope_kind, resource_references
) VALUES (
    '92800000-0000-0000-0000-000000000004',
    '92000000-0000-0000-0000-000000000001',
    '92500000-0000-0000-0000-000000000001', 4,
    'fleet.vehicles.list', 1, 'fleet.vehicles.list', 'fleet',
    'fleet:view', DECODE(REPEAT('42', 32), 'hex'),
    'tenant_wide', '[]'::JSONB
);

SELECT pg_temp.assert_true(
    (
        SELECT disposition = 'executable'
        FROM agent_capability_intent_registry
        WHERE capability_call_id = '92800000-0000-0000-0000-000000000004'
          AND run_id = '92500000-0000-0000-0000-000000000001'
          AND call_sequence = 4
    ),
    'a genuine capability call must claim executable disposition'
);

SELECT pg_temp.expect_failure(
    $statement$
        SELECT record_agent_capability_rejection(
            '92700000-0000-0000-0000-000000000004',
            '92000000-0000-0000-0000-000000000001',
            '92500000-0000-0000-0000-000000000001', 4::SMALLINT,
            '92100000-0000-0000-0000-000000000001',
            '92700000-0000-0000-0000-000000000004',
            '92520000-0000-0000-0000-000000000001',
            'fleet.vehicles.list', 1, DECODE(REPEAT('42', 32), 'hex'),
            'fleet.vehicles.list', 'fleet', 'fleet:view',
            'tenant_wide', '[]'::JSONB, 'denied', 'access_denied',
            'permission_missing', 'Access denied.',
            'agent-092-worker',
            '92600000-0000-0000-0000-000000000001'::UUID,
            5::BIGINT
        )
    $statement$,
    'idempotency conflict'
);

SELECT pg_temp.assert_true(
    NOT EXISTS (
        SELECT 1
        FROM agent_limit_reservations
        WHERE run_id = '92500000-0000-0000-0000-000000000001'
          AND stage_kind = 'capability_call'
    )
    AND NOT EXISTS (
        SELECT 1
        FROM agent_usage_events
        WHERE run_id = '92500000-0000-0000-0000-000000000001'
          AND event_kind = 'capability_call'
    ),
    '092 must not fabricate executable usage parents for preparation rejection'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_capability_rejections
        SET reason_code = 'changed'
        WHERE capability_call_id = '92700000-0000-0000-0000-000000000001'
    $statement$,
    'append-only'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_capability_intent_registry
        SET disposition = 'executable'
        WHERE capability_call_id = '92700000-0000-0000-0000-000000000001'
    $statement$,
    'disposition is immutable'
);

SELECT pg_temp.expect_failure(
    $statement$
        DELETE FROM agent_capability_rejections
        WHERE capability_call_id = '92700000-0000-0000-0000-000000000001'
    $statement$,
    'append-only'
);

SELECT pg_temp.expect_failure(
    'TRUNCATE agent_capability_rejections CASCADE',
    'append-only'
);

SELECT pg_temp.expect_failure(
    'TRUNCATE agent_capability_intent_registry CASCADE',
    'append-only'
);

-- Simulate a recovery claim without waiting for the 30-second lease window.
-- Only this fixture transition bypasses the 085 lifecycle trigger; all 092
-- writes below still pass through the production fenced recorder.
ALTER TABLE agent_run_queue
    DISABLE TRIGGER agent_run_queue_protect_lifecycle;
UPDATE agent_run_queue
SET leased_by = 'agent-092-replacement',
    lease_token = '92600000-0000-0000-0000-000000000002',
    heartbeat_at = STATEMENT_TIMESTAMP(),
    lease_expires_at = STATEMENT_TIMESTAMP() + INTERVAL '30 seconds',
    version = 6,
    updated_at = STATEMENT_TIMESTAMP()
WHERE run_id = '92500000-0000-0000-0000-000000000001';
ALTER TABLE agent_run_queue
    ENABLE TRIGGER agent_run_queue_protect_lifecycle;

SELECT pg_temp.expect_failure(
    $statement$
        SELECT record_agent_capability_rejection(
            '92700000-0000-0000-0000-000000000005',
            '92000000-0000-0000-0000-000000000001',
            '92500000-0000-0000-0000-000000000001', 5::SMALLINT,
            '92100000-0000-0000-0000-000000000001',
            '92700000-0000-0000-0000-000000000005',
            '92520000-0000-0000-0000-000000000001',
            'unknown.capability', 1, NULL::BYTEA,
            NULL::TEXT, NULL::TEXT, NULL::TEXT, NULL::TEXT, NULL::JSONB,
            'denied', 'unknown_capability', 'unknown_capability',
            'The requested capability does not exist.',
            'agent-092-worker',
            '92600000-0000-0000-0000-000000000001'::UUID, 5::BIGINT
        )
    $statement$,
    'exact current run lease'
);

COMMIT;

-- Two independent database sessions race opposite dispositions for one run
-- sequence. Exactly one ledger may win, and the registry must match it.
SELECT dblink_connect(
    'agent_092_executable',
    FORMAT('dbname=%s user=%s', CURRENT_DATABASE(), CURRENT_USER)
);
SELECT dblink_connect(
    'agent_092_rejected',
    FORMAT('dbname=%s user=%s', CURRENT_DATABASE(), CURRENT_USER)
);

SELECT pg_temp.assert_true(
    dblink_send_query(
        'agent_092_executable',
        $query$
            INSERT INTO agent_capability_calls (
                id, tenant_id, run_id, call_sequence, capability_key,
                capability_version, product_operation_key, owning_module_key,
                required_permission, input_fingerprint, scope_kind,
                resource_references
            ) VALUES (
                '92900000-0000-0000-0000-000000000006',
                '92000000-0000-0000-0000-000000000001',
                '92500000-0000-0000-0000-000000000001', 6,
                'fleet.vehicles.list', 1, 'fleet.vehicles.list', 'fleet',
                'fleet:view', DECODE(REPEAT('46', 32), 'hex'),
                'tenant_wide', '[]'::JSONB
            )
            RETURNING id::TEXT
        $query$
    ) = 1,
    'executable race query must be dispatched asynchronously'
);
SELECT pg_temp.assert_true(
    dblink_send_query(
        'agent_092_rejected',
        $query$
            SELECT record_agent_capability_rejection(
                '92700000-0000-0000-0000-000000000006',
                '92000000-0000-0000-0000-000000000001',
                '92500000-0000-0000-0000-000000000001', 6::SMALLINT,
                '92100000-0000-0000-0000-000000000001',
                '92700000-0000-0000-0000-000000000006',
                '92520000-0000-0000-0000-000000000001',
                'unknown.capability', 1, NULL::BYTEA,
                NULL::TEXT, NULL::TEXT, NULL::TEXT, NULL::TEXT, NULL::JSONB,
                'denied', 'unknown_capability', 'unknown_capability',
                'The requested capability does not exist.',
                'agent-092-replacement',
                '92600000-0000-0000-0000-000000000002'::UUID, 6::BIGINT
            )::TEXT
        $query$
    ) = 1,
    'rejection race query must be dispatched asynchronously'
);

DO $$
BEGIN
    WHILE dblink_is_busy('agent_092_executable') = 1
       OR dblink_is_busy('agent_092_rejected') = 1 LOOP
        PERFORM PG_SLEEP(0.01);
    END LOOP;
END;
$$;

CREATE TEMP TABLE agent_092_race_successes (
    disposition TEXT NOT NULL,
    durable_id TEXT NOT NULL
);

INSERT INTO agent_092_race_successes
SELECT 'executable', result
FROM dblink_get_result('agent_092_executable', FALSE) AS result(result TEXT);
INSERT INTO agent_092_race_successes
SELECT 'rejected', result
FROM dblink_get_result('agent_092_rejected', FALSE) AS result(result TEXT);

SELECT pg_temp.assert_true(
    (SELECT COUNT(*) = 1 FROM agent_092_race_successes),
    'exactly one concurrent intent disposition must commit'
);
SELECT pg_temp.assert_true(
    (
        SELECT COUNT(*) = 1
        FROM agent_capability_intent_registry
        WHERE run_id = '92500000-0000-0000-0000-000000000001'
          AND call_sequence = 6
          AND (
              (disposition = 'executable' AND EXISTS (
                  SELECT 1 FROM agent_capability_calls
                  WHERE run_id = '92500000-0000-0000-0000-000000000001'
                    AND call_sequence = 6
              ))
              OR (disposition = 'rejected' AND EXISTS (
                  SELECT 1 FROM agent_capability_rejections
                  WHERE run_id = '92500000-0000-0000-0000-000000000001'
                    AND call_sequence = 6
              ))
          )
    ),
    'the concurrent winner ledger must match the shared intent registry'
);

SELECT dblink_disconnect('agent_092_executable');
SELECT dblink_disconnect('agent_092_rejected');
DROP TABLE agent_092_race_successes;
