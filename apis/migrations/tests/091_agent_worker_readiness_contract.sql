-- Adversarial contract for migration 091. The caller applies migrations first.
-- Fixtures, transition proof, cleanup, and migration replay are rolled back.

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

-- Migration replay must preserve both tables and exactly one lifecycle trigger.
\ir ../091_create_agent_worker_readiness.sql

SELECT pg_temp.assert_true(
    (
        SELECT COUNT(*) = 1
        FROM pg_trigger
        WHERE tgrelid = 'agent_worker_instances'::REGCLASS
          AND tgname = 'agent_worker_instances_protect_lifecycle'
          AND NOT tgisinternal
    ),
    '091 replay must retain exactly one worker lifecycle trigger'
);

SELECT pg_temp.assert_true(
    NOT agent_has_ready_worker(),
    'an empty worker registry must fail closed'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_worker_instances (worker_key, heartbeat_expires_at)
        VALUES ('Agent Worker', NOW() + INTERVAL '30 seconds')
    $statement$,
    'agent_worker_instances_worker_key_check'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_worker_instances (worker_key, heartbeat_expires_at)
        VALUES ('agent-worker-too-long', NOW() + INTERVAL '121 seconds')
    $statement$,
    'agent_worker_instances_heartbeat_window_check'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_worker_instances (
            worker_key, status, artifact_key_coverage_sha256,
            provider_key_coverage_sha256, provider_route_coverage_sha256,
            startup_coverage_completed_at, heartbeat_expires_at
        ) VALUES (
            'agent-worker-invalid-ready', 'ready', DECODE(REPEAT('01', 32), 'hex'),
            DECODE(REPEAT('02', 32), 'hex'), DECODE(REPEAT('03', 32), 'hex'),
            NOW(), NOW() + INTERVAL '30 seconds'
        )
    $statement$,
    'initial starting state'
);

INSERT INTO agent_worker_instances (id, worker_key, heartbeat_expires_at)
VALUES (
    '91000000-0000-0000-0000-000000000001',
    'agent-worker-contract',
    NOW() + INTERVAL '30 seconds'
);

SELECT pg_temp.assert_true(
    (
        SELECT COUNT(*) = 1
        FROM agent_worker_readiness_events
        WHERE worker_instance_id = '91000000-0000-0000-0000-000000000001'
          AND worker_version = 1
          AND event_kind = 'registered'
          AND status = 'starting'
    ),
    'worker registration must create one immutable starting event'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_worker_instances
        SET status = 'ready',
            artifact_key_coverage_sha256 = DECODE(REPEAT('01', 32), 'hex'),
            startup_coverage_completed_at = STATEMENT_TIMESTAMP(),
            status_changed_at = STATEMENT_TIMESTAMP(),
            heartbeat_at = STATEMENT_TIMESTAMP(),
            heartbeat_expires_at = STATEMENT_TIMESTAMP() + INTERVAL '30 seconds',
            version = 2,
            updated_at = STATEMENT_TIMESTAMP()
        WHERE id = '91000000-0000-0000-0000-000000000001'
    $statement$,
    'ready Agent workers require current complete startup coverage'
);

UPDATE agent_worker_instances
SET status = 'ready',
    artifact_key_coverage_sha256 = DECODE(REPEAT('01', 32), 'hex'),
    provider_key_coverage_sha256 = DECODE(REPEAT('02', 32), 'hex'),
    provider_route_coverage_sha256 = DECODE(REPEAT('03', 32), 'hex'),
    startup_coverage_completed_at = STATEMENT_TIMESTAMP(),
    status_changed_at = STATEMENT_TIMESTAMP(),
    heartbeat_at = STATEMENT_TIMESTAMP(),
    heartbeat_expires_at = STATEMENT_TIMESTAMP() + INTERVAL '30 seconds',
    version = 2,
    updated_at = STATEMENT_TIMESTAMP()
WHERE id = '91000000-0000-0000-0000-000000000001';

SELECT pg_temp.assert_true(
    agent_has_ready_worker(),
    'a fresh ready worker with complete startup coverage must admit submissions'
);

SELECT pg_temp.assert_true(
    (
        SELECT COUNT(*) = 1
        FROM agent_worker_readiness_events
        WHERE worker_instance_id = '91000000-0000-0000-0000-000000000001'
          AND worker_version = 2
          AND event_kind = 'ready'
          AND artifact_key_coverage_sha256 = DECODE(REPEAT('01', 32), 'hex')
          AND provider_key_coverage_sha256 = DECODE(REPEAT('02', 32), 'hex')
          AND provider_route_coverage_sha256 = DECODE(REPEAT('03', 32), 'hex')
    ),
    'ready transition proof must retain only coverage fingerprints'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_worker_instances
        SET artifact_key_coverage_sha256 = DECODE(REPEAT('04', 32), 'hex'),
            heartbeat_at = STATEMENT_TIMESTAMP(),
            heartbeat_expires_at = STATEMENT_TIMESTAMP() + INTERVAL '30 seconds',
            version = 3,
            updated_at = STATEMENT_TIMESTAMP()
        WHERE id = '91000000-0000-0000-0000-000000000001'
    $statement$,
    'invalid Agent worker heartbeat'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_worker_instances
        SET heartbeat_at = STATEMENT_TIMESTAMP(),
            heartbeat_expires_at = STATEMENT_TIMESTAMP() + INTERVAL '30 seconds',
            version = 4,
            updated_at = STATEMENT_TIMESTAMP()
        WHERE id = '91000000-0000-0000-0000-000000000001'
    $statement$,
    'version fence'
);

UPDATE agent_worker_instances
SET heartbeat_at = STATEMENT_TIMESTAMP(),
    heartbeat_expires_at = STATEMENT_TIMESTAMP() + INTERVAL '30 seconds',
    version = 3,
    updated_at = STATEMENT_TIMESTAMP()
WHERE id = '91000000-0000-0000-0000-000000000001';

SELECT pg_temp.assert_true(
    (
        SELECT COUNT(*) = 2
        FROM agent_worker_readiness_events
        WHERE worker_instance_id = '91000000-0000-0000-0000-000000000001'
    ),
    'heartbeats must not create an unbounded audit stream'
);

-- Multiple worker processes may use the same bounded deployment key because
-- the random process-boot UUID is the fenced identity.
INSERT INTO agent_worker_instances (id, worker_key, heartbeat_expires_at)
VALUES (
    '91000000-0000-0000-0000-000000000002',
    'agent-worker-contract',
    NOW() + INTERVAL '30 seconds'
);

UPDATE agent_worker_instances
SET status = 'ready',
    artifact_key_coverage_sha256 = DECODE(REPEAT('11', 32), 'hex'),
    provider_key_coverage_sha256 = DECODE(REPEAT('12', 32), 'hex'),
    provider_route_coverage_sha256 = DECODE(REPEAT('13', 32), 'hex'),
    startup_coverage_completed_at = STATEMENT_TIMESTAMP(),
    status_changed_at = STATEMENT_TIMESTAMP(),
    heartbeat_at = STATEMENT_TIMESTAMP(),
    heartbeat_expires_at = STATEMENT_TIMESTAMP() + INTERVAL '30 seconds',
    version = 2,
    updated_at = STATEMENT_TIMESTAMP()
WHERE id = '91000000-0000-0000-0000-000000000002';

UPDATE agent_worker_instances
SET status = 'draining',
    status_reason_code = 'process_shutdown',
    status_changed_at = STATEMENT_TIMESTAMP(),
    version = 4,
    updated_at = STATEMENT_TIMESTAMP()
WHERE id = '91000000-0000-0000-0000-000000000001';

SELECT pg_temp.assert_true(
    agent_has_ready_worker(),
    'one draining worker must not hide another fresh ready worker'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_worker_instances
        SET status = 'ready',
            status_reason_code = NULL,
            status_changed_at = STATEMENT_TIMESTAMP(),
            heartbeat_at = STATEMENT_TIMESTAMP(),
            heartbeat_expires_at = STATEMENT_TIMESTAMP() + INTERVAL '30 seconds',
            version = 5,
            updated_at = STATEMENT_TIMESTAMP()
        WHERE id = '91000000-0000-0000-0000-000000000001'
    $statement$,
    'invalid Agent worker lifecycle transition'
);

UPDATE agent_worker_instances
SET status = 'unavailable',
    status_reason_code = 'process_shutdown',
    status_changed_at = STATEMENT_TIMESTAMP(),
    version = 3,
    updated_at = STATEMENT_TIMESTAMP()
WHERE id = '91000000-0000-0000-0000-000000000002';

SELECT pg_temp.assert_true(
    NOT agent_has_ready_worker(),
    'draining and unavailable workers must fail closed'
);

-- A very short but valid lease proves clock-based expiry without a long sleep.
INSERT INTO agent_worker_instances (id, worker_key, heartbeat_expires_at)
VALUES (
    '91000000-0000-0000-0000-000000000003',
    'agent-worker-expiry',
    NOW() + INTERVAL '30 seconds'
);

UPDATE agent_worker_instances
SET status = 'ready',
    artifact_key_coverage_sha256 = DECODE(REPEAT('21', 32), 'hex'),
    provider_key_coverage_sha256 = DECODE(REPEAT('22', 32), 'hex'),
    provider_route_coverage_sha256 = DECODE(REPEAT('23', 32), 'hex'),
    startup_coverage_completed_at = STATEMENT_TIMESTAMP(),
    status_changed_at = STATEMENT_TIMESTAMP(),
    heartbeat_at = STATEMENT_TIMESTAMP(),
    heartbeat_expires_at = STATEMENT_TIMESTAMP() + INTERVAL '1 millisecond',
    version = 2,
    updated_at = STATEMENT_TIMESTAMP()
WHERE id = '91000000-0000-0000-0000-000000000003';

SELECT PG_SLEEP(0.01);

SELECT pg_temp.assert_true(
    NOT agent_has_ready_worker(),
    'an expired ready heartbeat must fail closed before cleanup runs'
);

SELECT pg_temp.assert_true(
    expire_agent_worker_instances() = 1,
    'expiry cleanup must terminalize exactly the stale live worker'
);

SELECT pg_temp.assert_true(
    (
        SELECT status = 'unavailable'
           AND status_reason_code = 'heartbeat_expired'
           AND status_changed_at = heartbeat_expires_at
        FROM agent_worker_instances
        WHERE id = '91000000-0000-0000-0000-000000000003'
    ),
    'expiry cleanup must retain the actual lease expiry as transition time'
);

-- An instance stale for more than seven days can be expired and soft-retired
-- without deleting its append-only transition history.
INSERT INTO agent_worker_instances (
    id, worker_key, started_at, status_changed_at, heartbeat_at,
    heartbeat_expires_at, created_at, updated_at
) VALUES (
    '91000000-0000-0000-0000-000000000004',
    'agent-worker-stale',
    NOW() - INTERVAL '8 days',
    NOW() - INTERVAL '8 days',
    NOW() - INTERVAL '8 days',
    NOW() - INTERVAL '8 days' + INTERVAL '30 seconds',
    NOW() - INTERVAL '8 days',
    NOW() - INTERVAL '8 days'
);

SELECT pg_temp.assert_true(
    expire_agent_worker_instances() = 1,
    'expiry cleanup must find a stale starting process'
);

SELECT pg_temp.assert_true(
    retire_agent_worker_instances() = 1,
    'retirement cleanup must soft-delete stale unavailable instances'
);

SELECT pg_temp.assert_true(
    (
        SELECT deleted_at IS NOT NULL
        FROM agent_worker_instances
        WHERE id = '91000000-0000-0000-0000-000000000004'
    ),
    'retirement cleanup must keep a tombstoned current-state row'
);

SELECT pg_temp.assert_true(
    (
        SELECT COUNT(*) = 3
        FROM agent_worker_readiness_events
        WHERE worker_instance_id = '91000000-0000-0000-0000-000000000004'
          AND event_kind IN ('registered', 'unavailable', 'retired')
    ),
    'registration, expiry, and retirement must remain provable'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_worker_readiness_events (
            worker_instance_id, worker_key, worker_version, event_kind, status,
            heartbeat_at, heartbeat_expires_at, transitioned_at
        ) VALUES (
            '91000000-0000-0000-0000-000000000001',
            'agent-worker-contract', 99, 'draining', 'draining',
            NOW(), NOW() + INTERVAL '30 seconds', NOW()
        )
    $statement$,
    'generated by worker lifecycle changes'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_worker_readiness_events
        SET status = 'ready'
        WHERE worker_instance_id = '91000000-0000-0000-0000-000000000001'
    $statement$,
    'append-only'
);

SELECT pg_temp.expect_failure(
    $statement$
        DELETE FROM agent_worker_instances
        WHERE id = '91000000-0000-0000-0000-000000000001'
    $statement$,
    'retired, not deleted'
);

SELECT pg_temp.expect_failure(
    'TRUNCATE agent_worker_readiness_events CASCADE',
    'append-only'
);

ROLLBACK;
