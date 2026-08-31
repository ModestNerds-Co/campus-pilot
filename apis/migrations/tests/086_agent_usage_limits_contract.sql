-- Adversarial contract for migration 086. The caller applies migrations first.
-- All fixtures and assertions are rolled back.

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

CREATE OR REPLACE FUNCTION pg_temp.create_queued_run(
    fixture_thread_id UUID,
    fixture_member_id UUID,
    fixture_message_id UUID,
    fixture_run_id UUID,
    fixture_request_id UUID,
    fixture_correlation_id UUID,
    fixture_content TEXT
)
RETURNS VOID AS $$
BEGIN
    INSERT INTO agent_threads (id, tenant_id, owner_user_id)
    VALUES (
        fixture_thread_id,
        '86000000-0000-0000-0000-000000000001',
        '86100000-0000-0000-0000-000000000001'
    );
    INSERT INTO agent_thread_members (
        id, tenant_id, thread_id, user_id, membership_role, added_by
    ) VALUES (
        fixture_member_id,
        '86000000-0000-0000-0000-000000000001',
        fixture_thread_id,
        '86100000-0000-0000-0000-000000000001',
        'owner',
        '86100000-0000-0000-0000-000000000001'
    );
    UPDATE agent_threads
    SET next_message_sequence = 2,
        version = 2,
        last_activity_at = last_activity_at + INTERVAL '1 second',
        updated_at = updated_at + INTERVAL '1 second'
    WHERE id = fixture_thread_id;
    INSERT INTO agent_messages (
        id, tenant_id, thread_id, sequence, role, user_id, content
    ) VALUES (
        fixture_message_id,
        '86000000-0000-0000-0000-000000000001',
        fixture_thread_id,
        1,
        'user',
        '86100000-0000-0000-0000-000000000001',
        fixture_content
    );
    INSERT INTO agent_runs (
        id, tenant_id, thread_id, request_message_id, requested_by,
        task_class, origin_module_key, origin_route, request_id, correlation_id
    ) VALUES (
        fixture_run_id,
        '86000000-0000-0000-0000-000000000001',
        fixture_thread_id,
        fixture_message_id,
        '86100000-0000-0000-0000-000000000001',
        'module_read_reporting',
        'fleet',
        '/modules/fleet',
        fixture_request_id,
        fixture_correlation_id
    );
END;
$$ LANGUAGE plpgsql;

INSERT INTO tenants (id, slug, name)
VALUES
    ('86000000-0000-0000-0000-000000000001', 'agent-086-a', 'Agent 086 A'),
    ('96000000-0000-0000-0000-000000000001', 'agent-086-b', 'Agent 086 B');

INSERT INTO users (id, tenant_id, email, password_hash, full_name)
VALUES
    (
        '86100000-0000-0000-0000-000000000001',
        '86000000-0000-0000-0000-000000000001',
        'owner-a@agent-086.test', 'test-only', 'Owner A'
    ),
    (
        '96100000-0000-0000-0000-000000000001',
        '96000000-0000-0000-0000-000000000001',
        'owner-b@agent-086.test', 'test-only', 'Owner B'
    );

SELECT pg_temp.create_queued_run(
    '86200000-0000-0000-0000-000000000001',
    '86300000-0000-0000-0000-000000000001',
    '86400000-0000-0000-0000-000000000001',
    '86500000-0000-0000-0000-000000000001',
    '86510000-0000-0000-0000-000000000001',
    '86520000-0000-0000-0000-000000000001',
    'Run denied by a signed limit.'
);
SELECT pg_temp.create_queued_run(
    '86200000-0000-0000-0000-000000000002',
    '86300000-0000-0000-0000-000000000002',
    '86400000-0000-0000-0000-000000000002',
    '86500000-0000-0000-0000-000000000002',
    '86510000-0000-0000-0000-000000000002',
    '86520000-0000-0000-0000-000000000002',
    'Execute a provider fallback.'
);
SELECT pg_temp.create_queued_run(
    '86200000-0000-0000-0000-000000000003',
    '86300000-0000-0000-0000-000000000003',
    '86400000-0000-0000-0000-000000000003',
    '86500000-0000-0000-0000-000000000003',
    '86510000-0000-0000-0000-000000000003',
    '86520000-0000-0000-0000-000000000003',
    'Execute a capability.'
);
SELECT pg_temp.create_queued_run(
    '86200000-0000-0000-0000-000000000004',
    '86300000-0000-0000-0000-000000000004',
    '86400000-0000-0000-0000-000000000004',
    '86500000-0000-0000-0000-000000000004',
    '86510000-0000-0000-0000-000000000004',
    '86520000-0000-0000-0000-000000000004',
    'Commit a signed run limit after terminal usage.'
);

-- Every supported reporting dimension has a normalized, tenant-scoped rule.
INSERT INTO agent_limit_rules (
    id, tenant_id, scope_kind, person_user_id, role_key, origin_module_key,
    capability_module_key, capability_key, provider_key, provider_model_id,
    meter_key, currency_code, currency_exponent, period, limit_value,
    enforcement, provenance_kind, configured_by, change_reason
)
VALUES
    (
        '86c00000-0000-0000-0000-000000000001',
        '86000000-0000-0000-0000-000000000001',
        'campus', NULL, NULL, NULL, NULL, NULL, NULL, NULL,
        'agent.reasoning_tokens', NULL, NULL, 'month', 1000,
        'report', 'campus_reporting',
        '86100000-0000-0000-0000-000000000001', 'Campus reporting policy'
    ),
    (
        '86c00000-0000-0000-0000-000000000002',
        '86000000-0000-0000-0000-000000000001',
        'person', '86100000-0000-0000-0000-000000000001', NULL, NULL,
        NULL, NULL, NULL, NULL, 'agent.cached_input_tokens', NULL, NULL,
        'month', 1000, 'report', 'campus_reporting',
        '86100000-0000-0000-0000-000000000001', 'Person reporting policy'
    ),
    (
        '86c00000-0000-0000-0000-000000000003',
        '86000000-0000-0000-0000-000000000001',
        'role', NULL, 'campus_owner', NULL, NULL, NULL, NULL, NULL,
        'agent.provider_reported_cost', 'USD', 6, 'month', 5000000,
        'report', 'campus_reporting',
        '86100000-0000-0000-0000-000000000001', 'Role reporting policy'
    ),
    (
        '86c00000-0000-0000-0000-000000000004',
        '86000000-0000-0000-0000-000000000001',
        'origin_module', NULL, NULL, 'fleet', NULL, NULL, NULL, NULL,
        'agent.input_tokens', NULL, NULL, 'month', 1000,
        'report', 'campus_reporting',
        '86100000-0000-0000-0000-000000000001', 'Origin reporting policy'
    ),
    (
        '86c00000-0000-0000-0000-000000000005',
        '86000000-0000-0000-0000-000000000001',
        'capability_module', NULL, NULL, NULL, 'fleet', NULL, NULL, NULL,
        'agent.output_tokens', NULL, NULL, 'month', 1000,
        'report', 'campus_reporting',
        '86100000-0000-0000-0000-000000000001', 'Capability module report'
    ),
    (
        '86c00000-0000-0000-0000-000000000006',
        '86000000-0000-0000-0000-000000000001',
        'capability', NULL, NULL, NULL, NULL, 'fleet.vehicles.list', NULL, NULL,
        'agent.capability_calls', NULL, NULL, 'month', 1000,
        'report', 'campus_reporting',
        '86100000-0000-0000-0000-000000000001', 'Capability reporting policy'
    ),
    (
        '86c00000-0000-0000-0000-000000000007',
        '86000000-0000-0000-0000-000000000001',
        'provider', NULL, NULL, NULL, NULL, NULL, 'openai', NULL,
        'agent.estimated_cost', 'ZWG', 2, 'month', 100000,
        'report', 'campus_reporting',
        '86100000-0000-0000-0000-000000000001', 'Provider reporting policy'
    ),
    (
        '86c00000-0000-0000-0000-000000000008',
        '86000000-0000-0000-0000-000000000001',
        'model', NULL, NULL, NULL, NULL, NULL, 'openai', 'contract-model',
        'agent.provider_attempts', NULL, NULL, 'month', 1000,
        'report', 'campus_reporting',
        '86100000-0000-0000-0000-000000000001', 'Model reporting policy'
    );

INSERT INTO agent_limit_rules (
    id, tenant_id, scope_kind, person_user_id, role_key, provider_key,
    provider_model_id, meter_key, currency_code, currency_exponent, period,
    limit_value, enforcement, provenance_kind, configured_by, change_reason
)
VALUES
    (
        '86c00000-0000-0000-0000-000000000011',
        '86000000-0000-0000-0000-000000000001',
        'person', '86100000-0000-0000-0000-000000000001', NULL, NULL, NULL,
        'agent.runs', NULL, NULL, 'none', 1,
        'hard', 'campus_tightening',
        '86100000-0000-0000-0000-000000000001', 'Tighten person run use'
    ),
    (
        '86c00000-0000-0000-0000-000000000012',
        '86000000-0000-0000-0000-000000000001',
        'role', NULL, 'campus_owner', NULL, NULL,
        'agent.runs', NULL, NULL, 'none', 5,
        'hard', 'campus_tightening',
        '86100000-0000-0000-0000-000000000001', 'Tighten owner run use'
    ),
    (
        '86c00000-0000-0000-0000-000000000013',
        '86000000-0000-0000-0000-000000000001',
        'provider', NULL, NULL, 'openai', NULL,
        'agent.provider_attempts', NULL, NULL, 'none', 10,
        'hard', 'campus_tightening',
        '86100000-0000-0000-0000-000000000001', 'Tighten provider attempts'
    ),
    (
        '86c00000-0000-0000-0000-000000000014',
        '86000000-0000-0000-0000-000000000001',
        'model', NULL, NULL, 'openai', 'contract-model',
        'agent.estimated_cost', 'USD', 6, 'none', 100,
        'hard', 'campus_tightening',
        '86100000-0000-0000-0000-000000000001', 'Tighten model estimate'
    ),
    (
        '86c00000-0000-0000-0000-000000000015',
        '86000000-0000-0000-0000-000000000001',
        'provider', NULL, NULL, 'openai', NULL,
        'agent.input_tokens', NULL, NULL, 'none', 100,
        'hard', 'campus_tightening',
        '86100000-0000-0000-0000-000000000001', 'Tighten provider input'
    ),
    (
        '86c00000-0000-0000-0000-000000000016',
        '86000000-0000-0000-0000-000000000001',
        'model', NULL, NULL, 'openai', 'contract-model',
        'agent.input_tokens', NULL, NULL, 'none', 100,
        'hard', 'campus_tightening',
        '86100000-0000-0000-0000-000000000001', 'Overlap model input'
    ),
    (
        '86c00000-0000-0000-0000-000000000017',
        '86000000-0000-0000-0000-000000000001',
        'provider', NULL, NULL, 'openai', NULL,
        'agent.output_tokens', NULL, NULL, 'none', 100,
        'hard', 'campus_tightening',
        '86100000-0000-0000-0000-000000000001', 'Tighten provider output'
    );

SELECT pg_temp.assert_true(
    (SELECT COUNT(DISTINCT scope_kind) FROM agent_limit_rules
     WHERE tenant_id = '86000000-0000-0000-0000-000000000001') = 8,
    'all eight usage dimensions must be representable'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_limit_rules (
            tenant_id, scope_kind, role_key, origin_module_key, meter_key,
            period, limit_value, enforcement, provenance_kind,
            configured_by, change_reason
        ) VALUES (
            '86000000-0000-0000-0000-000000000001', 'role',
            'campus_owner', 'fleet', 'agent.runs', 'month', 1,
            'hard', 'campus_tightening',
            '86100000-0000-0000-0000-000000000001', 'Invalid mixed scope'
        )
    $statement$,
    'scope_shape_check'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_limit_rules (
            tenant_id, scope_kind, meter_key, currency_code, currency_exponent,
            period, limit_value, enforcement, provenance_kind,
            configured_by, change_reason
        ) VALUES (
            '86000000-0000-0000-0000-000000000001', 'campus',
            'agent.provider_reported_cost', 'USD', 6, 'month', 1,
            'hard', 'campus_tightening',
            '86100000-0000-0000-0000-000000000001', 'Invalid hard report meter'
        )
    $statement$,
    'hard_meter_check'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_limit_rules (
            tenant_id, scope_kind, meter_key, period, limit_value,
            enforcement, provenance_kind, configured_by, change_reason
        ) VALUES (
            '86000000-0000-0000-0000-000000000001', 'campus',
            'agent.runs', 'year', 1, 'hard', 'campus_reporting',
            '86100000-0000-0000-0000-000000000001', 'Invalid provenance'
        )
    $statement$,
    'agent_limit_rules_check'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_limit_rules (
            tenant_id, scope_kind, meter_key, period, limit_value,
            enforcement, provenance_kind, configured_by, change_reason
        ) VALUES (
            '86000000-0000-0000-0000-000000000001', 'campus',
            'agent.runs', 'day', 1, 'hard', 'campus_tightening',
            '96100000-0000-0000-0000-000000000001', 'Cross tenant author'
        )
    $statement$,
    'configured_by_tenant_fk'
);

-- Signed entitlement_limits and their existing meter tables remain the only
-- mutable commercial quota source. Agent-owned buckets cover local policy only.
INSERT INTO entitlement_limits (
    tenant_id, limit_key, source_lease_id, unit, period, limit_value, enforcement
)
VALUES (
    '86000000-0000-0000-0000-000000000001',
    'agent.runs',
    '86d10000-0000-0000-0000-000000000001',
    'run', 'none', 2, 'hard'
);

INSERT INTO entitlement_meter_buckets (
    id, tenant_id, limit_key, period_start, period_end
)
VALUES (
    '86d20000-0000-0000-0000-000000000001',
    '86000000-0000-0000-0000-000000000001',
    'agent.runs', TIMESTAMPTZ '1970-01-01 00:00:00+00', NULL
);

INSERT INTO agent_limit_buckets (
    id, tenant_id, campus_rule_id, meter_key, period, period_start
)
VALUES
    (
        '86d00000-0000-0000-0000-000000000011',
        '86000000-0000-0000-0000-000000000001',
        '86c00000-0000-0000-0000-000000000011',
        'agent.runs', 'none', TIMESTAMPTZ '1970-01-01 00:00:00+00'
    ),
    (
        '86d00000-0000-0000-0000-000000000012',
        '86000000-0000-0000-0000-000000000001',
        '86c00000-0000-0000-0000-000000000012',
        'agent.runs', 'none', TIMESTAMPTZ '1970-01-01 00:00:00+00'
    );

UPDATE agent_limit_buckets
SET committed_value = 1, updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86d00000-0000-0000-0000-000000000011';

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_limit_buckets (
            tenant_id, campus_rule_id, meter_key, period, period_start
        ) VALUES (
            '86000000-0000-0000-0000-000000000001',
            '86c00000-0000-0000-0000-000000000011',
            'agent.provider_attempts', 'none',
            TIMESTAMPTZ '1970-01-01 00:00:00+00'
        )
    $statement$,
    'active local tightening rule'
);

INSERT INTO agent_limit_reservations (
    id, tenant_id, run_id, actor_user_id, role_keys, origin_module_key,
    stage_kind, stage_sequence, idempotency_key, request_fingerprint
)
VALUES (
    '86e00000-0000-0000-0000-000000000001',
    '86000000-0000-0000-0000-000000000001',
    '86500000-0000-0000-0000-000000000001',
    '86100000-0000-0000-0000-000000000001',
    ARRAY['campus_owner', 'teacher'],
    'fleet', 'run', 0, 'agent086-run-denial',
    DECODE(REPEAT('01', 32), 'hex')
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_limit_reservations (
            tenant_id, run_id, actor_user_id, role_keys, origin_module_key,
            stage_kind, stage_sequence, idempotency_key, request_fingerprint
        ) VALUES (
            '86000000-0000-0000-0000-000000000001',
            '86500000-0000-0000-0000-000000000002',
            '86100000-0000-0000-0000-000000000001',
            ARRAY['campus_owner'], 'fleet', 'run', 0,
            'agent086-run-denial', DECODE(REPEAT('02', 32), 'hex')
        )
    $statement$,
    'agent_limit_reservations_tenant_id_idempotency_key_key'
);

INSERT INTO agent_limit_reservation_items (
    id, tenant_id, reservation_id, run_id, item_sequence, bucket_id,
    definition_kind, campus_rule_id, definition_version,
    scope_kind, scope_value, meter_key, unit, period, period_start,
    limit_value, committed_before, reserved_before, requested_amount,
    reserved_amount, decision
)
VALUES
    (
        '86f00000-0000-0000-0000-000000000001',
        '86000000-0000-0000-0000-000000000001',
        '86e00000-0000-0000-0000-000000000001',
        '86500000-0000-0000-0000-000000000001', 1,
        '86d00000-0000-0000-0000-000000000011',
        'local_rule', '86c00000-0000-0000-0000-000000000011', 1,
        'person', '86100000-0000-0000-0000-000000000001',
        'agent.runs', 'run', 'none', TIMESTAMPTZ '1970-01-01 00:00:00+00',
        1, 1, 0, 1, 0, 'denied'
    ),
    (
        '86f00000-0000-0000-0000-000000000002',
        '86000000-0000-0000-0000-000000000001',
        '86e00000-0000-0000-0000-000000000001',
        '86500000-0000-0000-0000-000000000001', 2,
        '86d00000-0000-0000-0000-000000000012',
        'local_rule', '86c00000-0000-0000-0000-000000000012', 1,
        'role', 'campus_owner', 'agent.runs', 'run', 'none',
        TIMESTAMPTZ '1970-01-01 00:00:00+00',
        5, 0, 0, 1, 0, 'allowed'
    );

INSERT INTO agent_limit_reservation_items (
    id, tenant_id, reservation_id, run_id, item_sequence,
    entitlement_bucket_id, definition_kind, source_lease_id,
    entitlement_limit_key, scope_kind, scope_value, meter_key, unit,
    period, period_start, limit_value, committed_before, reserved_before,
    requested_amount, reserved_amount, decision
)
VALUES (
    '86f00000-0000-0000-0000-000000000003',
    '86000000-0000-0000-0000-000000000001',
    '86e00000-0000-0000-0000-000000000001',
    '86500000-0000-0000-0000-000000000001', 3,
    '86d20000-0000-0000-0000-000000000001',
    'signed_entitlement', '86d10000-0000-0000-0000-000000000001',
    'agent.runs', 'campus', '86000000-0000-0000-0000-000000000001',
    'agent.runs', 'run', 'none', TIMESTAMPTZ '1970-01-01 00:00:00+00',
    2, 0, 0, 1, 0, 'allowed'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_limit_reservations
        SET status = 'reserved', expires_at = NOW() + INTERVAL '5 minutes',
            updated_at = updated_at + INTERVAL '1 second'
        WHERE id = '86e00000-0000-0000-0000-000000000001'
    $statement$,
    'atomically'
);

UPDATE agent_limit_reservations
SET status = 'denied', denied_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86e00000-0000-0000-0000-000000000001';

SELECT pg_temp.assert_true(
    (SELECT status FROM agent_limit_reservations
     WHERE id = '86e00000-0000-0000-0000-000000000001') = 'denied',
    'any matching denial must deny the full preparation without reservations'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_limit_reservations
        SET denied_at = denied_at + INTERVAL '1 second',
            updated_at = updated_at + INTERVAL '2 seconds'
        WHERE id = '86e00000-0000-0000-0000-000000000001'
    $statement$,
    'terminal Agent limit reservations are immutable'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_limit_reservation_items
        SET limit_value = limit_value + 1
        WHERE id = '86f00000-0000-0000-0000-000000000001'
    $statement$,
    'append-only'
);

UPDATE agent_runs
SET status = 'failed', safe_failure_code = 'agent_limit_denied',
    safe_failure_message = 'An Agent usage limit denied this run.',
    finished_at = NOW(), version = 2,
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86500000-0000-0000-0000-000000000001';

INSERT INTO actor_audit_events (
    id, tenant_id, actor_type, actor_user_id, action_key, target_type,
    target_id, outcome, request_id, correlation_id, agent_run_id, reason
)
VALUES (
    '87010000-0000-0000-0000-000000000001',
    '86000000-0000-0000-0000-000000000001', 'agent',
    '86100000-0000-0000-0000-000000000001', 'agent.run.execute',
    'agent_run', '86500000-0000-0000-0000-000000000001', 'denied',
    '86510000-0000-0000-0000-000000000001',
    '86520000-0000-0000-0000-000000000001',
    '86500000-0000-0000-0000-000000000001', 'Agent usage limit denied'
);

INSERT INTO agent_usage_events (
    id, tenant_id, event_kind, run_id, thread_id, actor_user_id,
    role_keys, origin_module_key, task_class, outcome, safe_failure_code,
    duration_ms, request_id, correlation_id, limit_reservation_id, occurred_at
)
VALUES (
    '87000000-0000-0000-0000-000000000001',
    '86000000-0000-0000-0000-000000000001', 'run',
    '86500000-0000-0000-0000-000000000001',
    '86200000-0000-0000-0000-000000000001',
    '86100000-0000-0000-0000-000000000001',
    ARRAY['campus_owner', 'teacher'], 'fleet', 'module_read_reporting',
    'denied', 'agent_limit_denied', 0,
    '86510000-0000-0000-0000-000000000001',
    '86520000-0000-0000-0000-000000000001',
    '86e00000-0000-0000-0000-000000000001', NOW()
);

INSERT INTO agent_usage_measures (
    tenant_id, usage_event_id, meter_key, amount
)
VALUES (
    '86000000-0000-0000-0000-000000000001',
    '87000000-0000-0000-0000-000000000001',
    'agent.runs', 1
);

SET CONSTRAINTS agent_usage_events_measure_set_constraint,
    agent_usage_events_denial_audit_constraint IMMEDIATE;
SET CONSTRAINTS agent_usage_events_measure_set_constraint,
    agent_usage_events_denial_audit_constraint DEFERRED;

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_usage_events
        SET duration_ms = duration_ms + 1
        WHERE id = '87000000-0000-0000-0000-000000000001'
    $statement$,
    'append-only'
);

SELECT pg_temp.expect_failure(
    $statement$
        DELETE FROM agent_usage_measures
        WHERE usage_event_id = '87000000-0000-0000-0000-000000000001'
    $statement$,
    'append-only'
);

-- A current signed hard Agent limit cannot be omitted even after local
-- tightenings are archived; Agent tables do not become commercial truth.
UPDATE agent_limit_rules
SET deleted_at = NOW(), version = 2, change_reason = 'Archive after overlap test',
    updated_at = updated_at + INTERVAL '1 second'
WHERE id IN (
    '86c00000-0000-0000-0000-000000000011',
    '86c00000-0000-0000-0000-000000000012'
);

INSERT INTO agent_limit_reservations (
    id, tenant_id, run_id, actor_user_id, role_keys, origin_module_key,
    stage_kind, stage_sequence, idempotency_key, request_fingerprint
)
VALUES (
    '86e00000-0000-0000-0000-000000000002',
    '86000000-0000-0000-0000-000000000001',
    '86500000-0000-0000-0000-000000000004',
    '86100000-0000-0000-0000-000000000001', ARRAY['campus_owner'],
    'fleet', 'run', 0, 'agent086-signed-required',
    DECODE(REPEAT('03', 32), 'hex')
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_limit_reservations
        SET status = 'not_limited', updated_at = updated_at + INTERVAL '1 second'
        WHERE id = '86e00000-0000-0000-0000-000000000002'
    $statement$,
    'omitted a current signed entitlement'
);

INSERT INTO entitlement_usage_reservations (
    id, tenant_id, bucket_id, source_lease_id, limit_key, unit,
    operation_key, actor_user_id, idempotency_key, amount, expires_at
)
VALUES (
    '86d30000-0000-0000-0000-000000000001',
    '86000000-0000-0000-0000-000000000001',
    '86d20000-0000-0000-0000-000000000001',
    '86d10000-0000-0000-0000-000000000001',
    'agent.runs', 'run', 'agent.runs.execute',
    '86100000-0000-0000-0000-000000000001',
    'agent086-canonical-run-two', 1, NOW() + INTERVAL '5 minutes'
);

UPDATE entitlement_meter_buckets
SET reserved_value = 1, updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86d20000-0000-0000-0000-000000000001';

INSERT INTO agent_limit_reservation_items (
    id, tenant_id, reservation_id, run_id, item_sequence,
    entitlement_bucket_id, entitlement_reservation_id, definition_kind,
    source_lease_id, entitlement_limit_key, scope_kind, scope_value,
    meter_key, unit, period, period_start, limit_value, committed_before,
    reserved_before, requested_amount, reserved_amount, decision
)
VALUES (
    '86f00000-0000-0000-0000-000000000004',
    '86000000-0000-0000-0000-000000000001',
    '86e00000-0000-0000-0000-000000000002',
    '86500000-0000-0000-0000-000000000004', 1,
    '86d20000-0000-0000-0000-000000000001',
    '86d30000-0000-0000-0000-000000000001',
    'signed_entitlement', '86d10000-0000-0000-0000-000000000001',
    'agent.runs', 'campus', '86000000-0000-0000-0000-000000000001',
    'agent.runs', 'run', 'none', TIMESTAMPTZ '1970-01-01 00:00:00+00',
    2, 0, 0, 1, 1, 'allowed'
);

UPDATE agent_limit_reservations
SET status = 'reserved', expires_at = NOW() + INTERVAL '5 minutes',
    updated_at = updated_at + INTERVAL '2 seconds'
WHERE id = '86e00000-0000-0000-0000-000000000002';

UPDATE agent_runs
SET status = 'failed', safe_failure_code = 'contract_terminal',
    safe_failure_message = 'Contract terminal run.', finished_at = NOW(),
    version = 2, updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86500000-0000-0000-0000-000000000004';

UPDATE entitlement_meter_buckets
SET committed_value = 1, reserved_value = 0,
    updated_at = updated_at + INTERVAL '2 seconds'
WHERE id = '86d20000-0000-0000-0000-000000000001';

UPDATE entitlement_usage_reservations
SET status = 'committed', committed_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86d30000-0000-0000-0000-000000000001';

INSERT INTO entitlement_usage_events (
    tenant_id, reservation_id, source_lease_id, limit_key, unit,
    operation_key, actor_user_id, amount, period_start, period_end, occurred_at
)
VALUES (
    '86000000-0000-0000-0000-000000000001',
    '86d30000-0000-0000-0000-000000000001',
    '86d10000-0000-0000-0000-000000000001',
    'agent.runs', 'run', 'agent.runs.execute',
    '86100000-0000-0000-0000-000000000001', 1,
    TIMESTAMPTZ '1970-01-01 00:00:00+00', NULL, NOW()
);

INSERT INTO agent_limit_reconciliations (
    id, tenant_id, reservation_id, run_id, reservation_item_id,
    committed_amount, enforcement_basis
)
VALUES (
    '86f10000-0000-0000-0000-000000000004',
    '86000000-0000-0000-0000-000000000001',
    '86e00000-0000-0000-0000-000000000002',
    '86500000-0000-0000-0000-000000000004',
    '86f00000-0000-0000-0000-000000000004', 1, 'exact'
);

UPDATE agent_limit_reservations
SET status = 'committed', committed_at = NOW(),
    updated_at = updated_at + INTERVAL '3 seconds'
WHERE id = '86e00000-0000-0000-0000-000000000002';

SELECT pg_temp.assert_true(
    (SELECT committed_value FROM entitlement_meter_buckets
     WHERE id = '86d20000-0000-0000-0000-000000000001') = 1
    AND NOT EXISTS (
        SELECT 1 FROM agent_limit_buckets
        WHERE tenant_id = '86000000-0000-0000-0000-000000000001'
          AND meter_key = 'agent.runs'
          AND campus_rule_id IS NULL
    ),
    'signed commercial usage must commit only through canonical entitlement tables'
);

-- Prepare a leased run, provider route, and exact provider child identity.
INSERT INTO agent_run_queue (run_id, tenant_id)
VALUES (
    '86500000-0000-0000-0000-000000000002',
    '86000000-0000-0000-0000-000000000001'
);

UPDATE agent_run_queue
SET state = 'leased',
    lease_token = '86600000-0000-0000-0000-000000000001',
    leased_by = 'agent-086-worker',
    heartbeat_at = STATEMENT_TIMESTAMP(),
    lease_expires_at = STATEMENT_TIMESTAMP() + INTERVAL '30 seconds',
    delivery_attempt = 1, version = 2,
    updated_at = updated_at + INTERVAL '1 second'
WHERE run_id = '86500000-0000-0000-0000-000000000002';

UPDATE agent_runs
SET status = 'running', started_at = NOW(), version = 2,
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86500000-0000-0000-0000-000000000002';

INSERT INTO ai_provider_connections (
    id, tenant_id, provider, auth_method, account_label, status,
    credential_ciphertext, credential_nonce, credential_key_id,
    credential_fingerprint, configured_by, model_catalog_version
)
VALUES (
    '86600000-0000-0000-0000-000000000011',
    '86000000-0000-0000-0000-000000000001',
    'openai', 'api_key', 'Agent 086 provider', 'ready',
    DECODE(REPEAT('ab', 16), 'hex'), DECODE(REPEAT('cd', 12), 'hex'),
    'contract-key', 'contract-fingerprint',
    '86100000-0000-0000-0000-000000000001', 1
);

INSERT INTO ai_provider_models (
    id, tenant_id, connection_id, credential_version, catalog_version,
    provider_model_id, display_name, max_output_tokens, supports_tools, refreshed_at
)
VALUES (
    '86700000-0000-0000-0000-000000000001',
    '86000000-0000-0000-0000-000000000001',
    '86600000-0000-0000-0000-000000000011',
    1, 1, 'contract-model', 'Contract Model', 4096, TRUE, NOW()
);

INSERT INTO ai_route_sets (
    id, tenant_id, scope_kind, configured_by, change_reason
)
VALUES (
    '86800000-0000-0000-0000-000000000001',
    '86000000-0000-0000-0000-000000000001', 'tenant_default',
    '86100000-0000-0000-0000-000000000001', 'Agent 086 route'
);

INSERT INTO ai_task_routes (
    id, tenant_id, route_set_id, priority, connection_id, model_id,
    requires_tools, created_by
)
VALUES (
    '86900000-0000-0000-0000-000000000001',
    '86000000-0000-0000-0000-000000000001',
    '86800000-0000-0000-0000-000000000001', 1,
    '86600000-0000-0000-0000-000000000011',
    '86700000-0000-0000-0000-000000000001', TRUE,
    '86100000-0000-0000-0000-000000000001'
);

INSERT INTO agent_provider_attempts (
    id, tenant_id, run_id, turn_index, attempt_index, route_set_id,
    route_version, route_target_id, connection_id, credential_version,
    model_snapshot_id, provider_key, provider_model_id, task_class
)
VALUES (
    '86a00000-0000-0000-0000-000000000001',
    '86000000-0000-0000-0000-000000000001',
    '86500000-0000-0000-0000-000000000002', 1, 1,
    '86800000-0000-0000-0000-000000000001', 1,
    '86900000-0000-0000-0000-000000000001',
    '86600000-0000-0000-0000-000000000011', 1,
    '86700000-0000-0000-0000-000000000001',
    'openai', 'contract-model', 'module_read_reporting'
);

INSERT INTO agent_execution_steps (
    id, tenant_id, run_id, step_index, turn_index, step_kind,
    provider_attempt_id, input_fingerprint
)
VALUES (
    '86a10000-0000-0000-0000-000000000001',
    '86000000-0000-0000-0000-000000000001',
    '86500000-0000-0000-0000-000000000002', 1, 1,
    'provider_attempt', '86a00000-0000-0000-0000-000000000001',
    DECODE(REPEAT('11', 32), 'hex')
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_limit_reservations (
            tenant_id, run_id, provider_attempt_id, actor_user_id, role_keys,
            origin_module_key, provider_key, provider_model_id, stage_kind,
            stage_sequence, idempotency_key, request_fingerprint
        ) VALUES (
            '86000000-0000-0000-0000-000000000001',
            '86500000-0000-0000-0000-000000000002',
            '86a00000-0000-0000-0000-000000000001',
            '86100000-0000-0000-0000-000000000001', ARRAY['campus_owner'],
            'fleet', 'openai', 'contract-model', 'provider_attempt', 1,
            'agent086-wrong-fingerprint', DECODE(REPEAT('12', 32), 'hex')
        )
    $statement$,
    'exact child input'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_limit_reservations (
            tenant_id, run_id, provider_attempt_id, actor_user_id, role_keys,
            origin_module_key, provider_key, provider_model_id, stage_kind,
            stage_sequence, idempotency_key, request_fingerprint
        ) VALUES (
            '86000000-0000-0000-0000-000000000001',
            '86500000-0000-0000-0000-000000000003',
            '86a00000-0000-0000-0000-000000000001',
            '86100000-0000-0000-0000-000000000001', ARRAY['campus_owner'],
            'fleet', 'openai', 'contract-model', 'provider_attempt', 1,
            'agent086-cross-run-child', DECODE(REPEAT('11', 32), 'hex')
        )
    $statement$,
    'exact child input'
);

INSERT INTO agent_limit_buckets (
    id, tenant_id, campus_rule_id, meter_key, currency_code,
    currency_exponent, period, period_start
)
VALUES
    (
        '86d00000-0000-0000-0000-000000000013',
        '86000000-0000-0000-0000-000000000001',
        '86c00000-0000-0000-0000-000000000013',
        'agent.provider_attempts', NULL, NULL, 'none',
        TIMESTAMPTZ '1970-01-01 00:00:00+00'
    ),
    (
        '86d00000-0000-0000-0000-000000000014',
        '86000000-0000-0000-0000-000000000001',
        '86c00000-0000-0000-0000-000000000014',
        'agent.estimated_cost', 'USD', 6, 'none',
        TIMESTAMPTZ '1970-01-01 00:00:00+00'
    ),
    (
        '86d00000-0000-0000-0000-000000000015',
        '86000000-0000-0000-0000-000000000001',
        '86c00000-0000-0000-0000-000000000015',
        'agent.input_tokens', NULL, NULL, 'none',
        TIMESTAMPTZ '1970-01-01 00:00:00+00'
    ),
    (
        '86d00000-0000-0000-0000-000000000016',
        '86000000-0000-0000-0000-000000000001',
        '86c00000-0000-0000-0000-000000000016',
        'agent.input_tokens', NULL, NULL, 'none',
        TIMESTAMPTZ '1970-01-01 00:00:00+00'
    ),
    (
        '86d00000-0000-0000-0000-000000000017',
        '86000000-0000-0000-0000-000000000001',
        '86c00000-0000-0000-0000-000000000017',
        'agent.output_tokens', NULL, NULL, 'none',
        TIMESTAMPTZ '1970-01-01 00:00:00+00'
    );

UPDATE agent_limit_buckets
SET reserved_value = CASE
        WHEN id = '86d00000-0000-0000-0000-000000000013' THEN 1
        WHEN id = '86d00000-0000-0000-0000-000000000014' THEN 20
        ELSE 10
    END,
    updated_at = updated_at + INTERVAL '1 second'
WHERE id IN (
    '86d00000-0000-0000-0000-000000000013',
    '86d00000-0000-0000-0000-000000000014',
    '86d00000-0000-0000-0000-000000000015',
    '86d00000-0000-0000-0000-000000000016',
    '86d00000-0000-0000-0000-000000000017'
);

INSERT INTO agent_limit_reservations (
    id, tenant_id, run_id, provider_attempt_id, actor_user_id, role_keys,
    origin_module_key, provider_key, provider_model_id, stage_kind,
    stage_sequence, idempotency_key, request_fingerprint
)
VALUES (
    '86e00000-0000-0000-0000-000000000011',
    '86000000-0000-0000-0000-000000000001',
    '86500000-0000-0000-0000-000000000002',
    '86a00000-0000-0000-0000-000000000001',
    '86100000-0000-0000-0000-000000000001', ARRAY['campus_owner'],
    'fleet', 'openai', 'contract-model', 'provider_attempt', 1,
    'agent086-provider-one', DECODE(REPEAT('11', 32), 'hex')
);

INSERT INTO agent_limit_reservation_items (
    id, tenant_id, reservation_id, run_id, item_sequence, bucket_id,
    definition_kind, campus_rule_id, definition_version, scope_kind,
    scope_value, meter_key, unit, currency_code, currency_exponent,
    pricing_version, period, period_start, limit_value, committed_before,
    reserved_before, requested_amount, reserved_amount, decision
)
VALUES
    (
        '86f00000-0000-0000-0000-000000000011',
        '86000000-0000-0000-0000-000000000001',
        '86e00000-0000-0000-0000-000000000011',
        '86500000-0000-0000-0000-000000000002', 1,
        '86d00000-0000-0000-0000-000000000013', 'local_rule',
        '86c00000-0000-0000-0000-000000000013', 1,
        'provider', 'openai', 'agent.provider_attempts', 'attempt',
        NULL, NULL, NULL, 'none', TIMESTAMPTZ '1970-01-01 00:00:00+00',
        10, 0, 0, 1, 1, 'allowed'
    ),
    (
        '86f00000-0000-0000-0000-000000000012',
        '86000000-0000-0000-0000-000000000001',
        '86e00000-0000-0000-0000-000000000011',
        '86500000-0000-0000-0000-000000000002', 2,
        '86d00000-0000-0000-0000-000000000014', 'local_rule',
        '86c00000-0000-0000-0000-000000000014', 1,
        'model', 'contract-model', 'agent.estimated_cost', 'money',
        'USD', 6, 'pricing-v1', 'none',
        TIMESTAMPTZ '1970-01-01 00:00:00+00',
        100, 0, 0, 20, 20, 'allowed'
    ),
    (
        '86f00000-0000-0000-0000-000000000013',
        '86000000-0000-0000-0000-000000000001',
        '86e00000-0000-0000-0000-000000000011',
        '86500000-0000-0000-0000-000000000002', 3,
        '86d00000-0000-0000-0000-000000000015', 'local_rule',
        '86c00000-0000-0000-0000-000000000015', 1,
        'provider', 'openai', 'agent.input_tokens', 'token',
        NULL, NULL, NULL, 'none', TIMESTAMPTZ '1970-01-01 00:00:00+00',
        100, 0, 0, 10, 10, 'allowed'
    ),
    (
        '86f00000-0000-0000-0000-000000000014',
        '86000000-0000-0000-0000-000000000001',
        '86e00000-0000-0000-0000-000000000011',
        '86500000-0000-0000-0000-000000000002', 4,
        '86d00000-0000-0000-0000-000000000016', 'local_rule',
        '86c00000-0000-0000-0000-000000000016', 1,
        'model', 'contract-model', 'agent.input_tokens', 'token',
        NULL, NULL, NULL, 'none', TIMESTAMPTZ '1970-01-01 00:00:00+00',
        100, 0, 0, 10, 10, 'allowed'
    ),
    (
        '86f00000-0000-0000-0000-000000000015',
        '86000000-0000-0000-0000-000000000001',
        '86e00000-0000-0000-0000-000000000011',
        '86500000-0000-0000-0000-000000000002', 5,
        '86d00000-0000-0000-0000-000000000017', 'local_rule',
        '86c00000-0000-0000-0000-000000000017', 1,
        'provider', 'openai', 'agent.output_tokens', 'token',
        NULL, NULL, NULL, 'none', TIMESTAMPTZ '1970-01-01 00:00:00+00',
        100, 0, 0, 10, 10, 'allowed'
    );

UPDATE agent_limit_reservations
SET status = 'reserved', expires_at = NOW() + INTERVAL '5 minutes',
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86e00000-0000-0000-0000-000000000011';

UPDATE agent_limit_reservations
SET claimed_at = STATEMENT_TIMESTAMP(), claimed_by_worker_id = 'agent-086-worker',
    claim_fence_version = 2, updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86e00000-0000-0000-0000-000000000011';

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_limit_reservations
        SET claimed_at = STATEMENT_TIMESTAMP(),
            claimed_by_worker_id = 'replay-worker', claim_fence_version = 3,
            updated_at = updated_at + INTERVAL '1 second'
        WHERE id = '86e00000-0000-0000-0000-000000000011'
    $statement$,
    'one-time and immutable'
);

UPDATE agent_provider_attempts
SET status = 'failed', failure_origin = 'upstream',
    failure_category = 'rate_limited', input_tokens = 3,
    reasoning_tokens = 2,
    provider_reported_cost_amount = 125,
    provider_reported_cost_currency = 'ZWG',
    provider_reported_cost_exponent = 2,
    estimated_cost_amount = 9,
    estimated_cost_currency = 'USD', estimated_cost_exponent = 6,
    estimated_pricing_version = 'pricing-v1',
    finished_at = NOW(), updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86a00000-0000-0000-0000-000000000001';

UPDATE agent_execution_steps
SET status = 'failed', safe_failure_code = 'rate_limited',
    finished_at = NOW(), updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86a10000-0000-0000-0000-000000000001';

SAVEPOINT agent086_failed_reconciliation;
UPDATE agent_limit_buckets
SET committed_value = 2, reserved_value = 0,
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86d00000-0000-0000-0000-000000000015';
SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_limit_reconciliations (
            tenant_id, reservation_id, run_id, reservation_item_id,
            committed_amount, enforcement_basis
        ) VALUES (
            '86000000-0000-0000-0000-000000000001',
            '86e00000-0000-0000-0000-000000000011',
            '86500000-0000-0000-0000-000000000002',
            '86f00000-0000-0000-0000-000000000013', 2, 'exact'
        )
    $statement$,
    'terminal usage evidence'
);
ROLLBACK TO SAVEPOINT agent086_failed_reconciliation;
RELEASE SAVEPOINT agent086_failed_reconciliation;
SELECT pg_temp.assert_true(
    (SELECT committed_value = 0 AND reserved_value = 10
     FROM agent_limit_buckets
     WHERE id = '86d00000-0000-0000-0000-000000000015')
    AND NOT EXISTS (
        SELECT 1 FROM agent_limit_reconciliations
        WHERE reservation_item_id = '86f00000-0000-0000-0000-000000000013'
    ),
    'failed reconciliation transaction must not leak counter movement'
);

UPDATE agent_limit_buckets
SET committed_value = CASE
        WHEN id = '86d00000-0000-0000-0000-000000000013' THEN 1
        WHEN id = '86d00000-0000-0000-0000-000000000014' THEN 9
        WHEN id IN (
            '86d00000-0000-0000-0000-000000000015',
            '86d00000-0000-0000-0000-000000000016'
        ) THEN 3
        ELSE 10
    END,
    reserved_value = 0,
    updated_at = updated_at + INTERVAL '2 seconds'
WHERE id IN (
    '86d00000-0000-0000-0000-000000000013',
    '86d00000-0000-0000-0000-000000000014',
    '86d00000-0000-0000-0000-000000000015',
    '86d00000-0000-0000-0000-000000000016',
    '86d00000-0000-0000-0000-000000000017'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_limit_reconciliations (
            tenant_id, reservation_id, run_id, reservation_item_id,
            committed_amount, enforcement_basis
        ) VALUES (
            '86000000-0000-0000-0000-000000000001',
            '86e00000-0000-0000-0000-000000000011',
            '86500000-0000-0000-0000-000000000002',
            '86f00000-0000-0000-0000-000000000014', 2, 'exact'
        )
    $statement$,
    'terminal usage evidence'
);

INSERT INTO agent_limit_reconciliations (
    id, tenant_id, reservation_id, run_id, reservation_item_id,
    committed_amount, enforcement_basis
)
VALUES
    (
        '86f10000-0000-0000-0000-000000000011',
        '86000000-0000-0000-0000-000000000001',
        '86e00000-0000-0000-0000-000000000011',
        '86500000-0000-0000-0000-000000000002',
        '86f00000-0000-0000-0000-000000000011', 1, 'exact'
    ),
    (
        '86f10000-0000-0000-0000-000000000012',
        '86000000-0000-0000-0000-000000000001',
        '86e00000-0000-0000-0000-000000000011',
        '86500000-0000-0000-0000-000000000002',
        '86f00000-0000-0000-0000-000000000012', 9, 'estimated'
    ),
    (
        '86f10000-0000-0000-0000-000000000013',
        '86000000-0000-0000-0000-000000000001',
        '86e00000-0000-0000-0000-000000000011',
        '86500000-0000-0000-0000-000000000002',
        '86f00000-0000-0000-0000-000000000013', 3, 'exact'
    ),
    (
        '86f10000-0000-0000-0000-000000000014',
        '86000000-0000-0000-0000-000000000001',
        '86e00000-0000-0000-0000-000000000011',
        '86500000-0000-0000-0000-000000000002',
        '86f00000-0000-0000-0000-000000000014', 3, 'exact'
    ),
    (
        '86f10000-0000-0000-0000-000000000015',
        '86000000-0000-0000-0000-000000000001',
        '86e00000-0000-0000-0000-000000000011',
        '86500000-0000-0000-0000-000000000002',
        '86f00000-0000-0000-0000-000000000015', 10, 'upper_bound'
    );

UPDATE agent_limit_reservations
SET status = 'committed', committed_at = NOW(),
    updated_at = updated_at + INTERVAL '2 seconds'
WHERE id = '86e00000-0000-0000-0000-000000000011';

INSERT INTO agent_usage_events (
    id, tenant_id, event_kind, run_id, thread_id, actor_user_id,
    role_keys, origin_module_key, task_class, provider_attempt_id,
    provider_turn_index, provider_attempt_index, provider_connection_id,
    provider_key, provider_model_id, provider_model_snapshot_id,
    route_priority, failure_origin, failure_category, outcome,
    safe_failure_code, duration_ms, request_id, correlation_id,
    limit_reservation_id, occurred_at
)
VALUES (
    '87000000-0000-0000-0000-000000000011',
    '86000000-0000-0000-0000-000000000001', 'provider_attempt',
    '86500000-0000-0000-0000-000000000002',
    '86200000-0000-0000-0000-000000000002',
    '86100000-0000-0000-0000-000000000001', ARRAY['campus_owner'],
    'fleet', 'module_read_reporting',
    '86a00000-0000-0000-0000-000000000001', 1, 1,
    '86600000-0000-0000-0000-000000000011',
    'openai', 'contract-model',
    '86700000-0000-0000-0000-000000000001', 1,
    'upstream', 'rate_limited', 'failed', 'rate_limited', 25,
    '86510000-0000-0000-0000-000000000002',
    '86520000-0000-0000-0000-000000000002',
    '86e00000-0000-0000-0000-000000000011', NOW()
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_usage_measures (
            tenant_id, usage_event_id, meter_key, amount
        ) VALUES (
            '86000000-0000-0000-0000-000000000001',
            '87000000-0000-0000-0000-000000000011',
            'agent.input_tokens', 8
        )
    $statement$,
    'match its provider attempt'
);

INSERT INTO agent_usage_measures (
    tenant_id, usage_event_id, meter_key, amount,
    enforcement_amount, enforcement_basis,
    currency_code, currency_exponent, pricing_version
)
VALUES
    (
        '86000000-0000-0000-0000-000000000001',
        '87000000-0000-0000-0000-000000000011',
        'agent.provider_attempts', 1, 1, 'exact', NULL, NULL, NULL
    ),
    (
        '86000000-0000-0000-0000-000000000001',
        '87000000-0000-0000-0000-000000000011',
        'agent.input_tokens', 3, 3, 'exact', NULL, NULL, NULL
    ),
    (
        '86000000-0000-0000-0000-000000000001',
        '87000000-0000-0000-0000-000000000011',
        'agent.output_tokens', NULL, 10, 'upper_bound', NULL, NULL, NULL
    ),
    (
        '86000000-0000-0000-0000-000000000001',
        '87000000-0000-0000-0000-000000000011',
        'agent.cached_input_tokens', NULL, NULL, NULL, NULL, NULL, NULL
    ),
    (
        '86000000-0000-0000-0000-000000000001',
        '87000000-0000-0000-0000-000000000011',
        'agent.reasoning_tokens', 2, NULL, NULL, NULL, NULL, NULL
    ),
    (
        '86000000-0000-0000-0000-000000000001',
        '87000000-0000-0000-0000-000000000011',
        'agent.provider_reported_cost', 125, NULL, NULL, 'ZWG', 2, NULL
    ),
    (
        '86000000-0000-0000-0000-000000000001',
        '87000000-0000-0000-0000-000000000011',
        'agent.estimated_cost', 9, 9, 'estimated', 'USD', 6, 'pricing-v1'
    );

SET CONSTRAINTS agent_usage_events_measure_set_constraint,
    agent_usage_events_denial_audit_constraint IMMEDIATE;
SET CONSTRAINTS agent_usage_events_measure_set_constraint,
    agent_usage_events_denial_audit_constraint DEFERRED;

SELECT pg_temp.assert_true(
    (SELECT COUNT(DISTINCT currency_code)
     FROM agent_usage_measures
     WHERE usage_event_id = '87000000-0000-0000-0000-000000000011'
       AND currency_code IS NOT NULL) = 2,
    'provider and estimated money must retain separate currencies without FX'
);

-- A claimed preflight failure reconciles unknown token/cost usage to zero.
-- The upper bound is released, the claim cannot be expired, and completion is
-- allowed after the short reservation TTL has elapsed.
INSERT INTO agent_provider_attempts (
    id, tenant_id, run_id, turn_index, attempt_index, route_set_id,
    route_version, route_target_id, connection_id, credential_version,
    model_snapshot_id, provider_key, provider_model_id, task_class
)
VALUES (
    '86a00000-0000-0000-0000-000000000003',
    '86000000-0000-0000-0000-000000000001',
    '86500000-0000-0000-0000-000000000002', 1, 3,
    '86800000-0000-0000-0000-000000000001', 1,
    '86900000-0000-0000-0000-000000000001',
    '86600000-0000-0000-0000-000000000011', 1,
    '86700000-0000-0000-0000-000000000001',
    'openai', 'contract-model', 'module_read_reporting'
);

INSERT INTO agent_execution_steps (
    id, tenant_id, run_id, step_index, turn_index, step_kind,
    provider_attempt_id, input_fingerprint
)
VALUES (
    '86a10000-0000-0000-0000-000000000003',
    '86000000-0000-0000-0000-000000000001',
    '86500000-0000-0000-0000-000000000002', 3, 1,
    'provider_attempt', '86a00000-0000-0000-0000-000000000003',
    DECODE(REPEAT('13', 32), 'hex')
);

UPDATE agent_limit_buckets
SET reserved_value = CASE
        WHEN id = '86d00000-0000-0000-0000-000000000013' THEN 1
        WHEN id = '86d00000-0000-0000-0000-000000000014' THEN 20
        ELSE 10
    END,
    updated_at = updated_at + INTERVAL '1 second'
WHERE id IN (
    '86d00000-0000-0000-0000-000000000013',
    '86d00000-0000-0000-0000-000000000014',
    '86d00000-0000-0000-0000-000000000015',
    '86d00000-0000-0000-0000-000000000016',
    '86d00000-0000-0000-0000-000000000017'
);

INSERT INTO agent_limit_reservations (
    id, tenant_id, run_id, provider_attempt_id, actor_user_id, role_keys,
    origin_module_key, provider_key, provider_model_id, stage_kind,
    stage_sequence, idempotency_key, request_fingerprint
)
VALUES (
    '86e00000-0000-0000-0000-000000000013',
    '86000000-0000-0000-0000-000000000001',
    '86500000-0000-0000-0000-000000000002',
    '86a00000-0000-0000-0000-000000000003',
    '86100000-0000-0000-0000-000000000001', ARRAY['campus_owner'],
    'fleet', 'openai', 'contract-model', 'provider_attempt', 3,
    'agent086-provider-preflight', DECODE(REPEAT('13', 32), 'hex')
);

INSERT INTO agent_limit_reservation_items (
    id, tenant_id, reservation_id, run_id, item_sequence, bucket_id,
    definition_kind, campus_rule_id, definition_version, scope_kind,
    scope_value, meter_key, unit, currency_code, currency_exponent,
    pricing_version, period, period_start, limit_value, committed_before,
    reserved_before, requested_amount, reserved_amount, decision
)
VALUES
    ('86f00000-0000-0000-0000-000000000021', '86000000-0000-0000-0000-000000000001', '86e00000-0000-0000-0000-000000000013', '86500000-0000-0000-0000-000000000002', 1, '86d00000-0000-0000-0000-000000000013', 'local_rule', '86c00000-0000-0000-0000-000000000013', 1, 'provider', 'openai', 'agent.provider_attempts', 'attempt', NULL, NULL, NULL, 'none', TIMESTAMPTZ '1970-01-01 00:00:00+00', 10, 1, 0, 1, 1, 'allowed'),
    ('86f00000-0000-0000-0000-000000000022', '86000000-0000-0000-0000-000000000001', '86e00000-0000-0000-0000-000000000013', '86500000-0000-0000-0000-000000000002', 2, '86d00000-0000-0000-0000-000000000014', 'local_rule', '86c00000-0000-0000-0000-000000000014', 1, 'model', 'contract-model', 'agent.estimated_cost', 'money', 'USD', 6, 'pricing-v1', 'none', TIMESTAMPTZ '1970-01-01 00:00:00+00', 100, 9, 0, 20, 20, 'allowed'),
    ('86f00000-0000-0000-0000-000000000023', '86000000-0000-0000-0000-000000000001', '86e00000-0000-0000-0000-000000000013', '86500000-0000-0000-0000-000000000002', 3, '86d00000-0000-0000-0000-000000000015', 'local_rule', '86c00000-0000-0000-0000-000000000015', 1, 'provider', 'openai', 'agent.input_tokens', 'token', NULL, NULL, NULL, 'none', TIMESTAMPTZ '1970-01-01 00:00:00+00', 100, 3, 0, 10, 10, 'allowed'),
    ('86f00000-0000-0000-0000-000000000024', '86000000-0000-0000-0000-000000000001', '86e00000-0000-0000-0000-000000000013', '86500000-0000-0000-0000-000000000002', 4, '86d00000-0000-0000-0000-000000000016', 'local_rule', '86c00000-0000-0000-0000-000000000016', 1, 'model', 'contract-model', 'agent.input_tokens', 'token', NULL, NULL, NULL, 'none', TIMESTAMPTZ '1970-01-01 00:00:00+00', 100, 3, 0, 10, 10, 'allowed'),
    ('86f00000-0000-0000-0000-000000000025', '86000000-0000-0000-0000-000000000001', '86e00000-0000-0000-0000-000000000013', '86500000-0000-0000-0000-000000000002', 5, '86d00000-0000-0000-0000-000000000017', 'local_rule', '86c00000-0000-0000-0000-000000000017', 1, 'provider', 'openai', 'agent.output_tokens', 'token', NULL, NULL, NULL, 'none', TIMESTAMPTZ '1970-01-01 00:00:00+00', 100, 10, 0, 10, 10, 'allowed');

UPDATE agent_limit_reservations
SET status = 'reserved', expires_at = CLOCK_TIMESTAMP() + INTERVAL '5 milliseconds',
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86e00000-0000-0000-0000-000000000013';

UPDATE agent_limit_reservations
SET claimed_at = STATEMENT_TIMESTAMP(), claimed_by_worker_id = 'agent-086-worker',
    claim_fence_version = 2, updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86e00000-0000-0000-0000-000000000013';

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE agent_limit_reservations
        SET status = 'expired', released_at = NOW(),
            updated_at = updated_at + INTERVAL '1 second'
        WHERE id = '86e00000-0000-0000-0000-000000000013'
    $statement$,
    'claimed Agent limit reservations must be reconciled'
);

SELECT pg_sleep(0.01);
SELECT pg_temp.assert_true(
    (SELECT expires_at < CLOCK_TIMESTAMP()
     FROM agent_limit_reservations
     WHERE id = '86e00000-0000-0000-0000-000000000013'),
    'claimed reservation must be testably past TTL before reconciliation'
);

UPDATE agent_provider_attempts
SET status = 'failed', failure_origin = 'preflight',
    failure_category = 'connection_unavailable', finished_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86a00000-0000-0000-0000-000000000003';

UPDATE agent_execution_steps
SET status = 'failed', safe_failure_code = 'connection_unavailable',
    finished_at = NOW(), updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86a10000-0000-0000-0000-000000000003';

UPDATE agent_limit_buckets
SET committed_value = CASE
        WHEN id = '86d00000-0000-0000-0000-000000000013' THEN 2
        WHEN id = '86d00000-0000-0000-0000-000000000014' THEN 9
        WHEN id IN ('86d00000-0000-0000-0000-000000000015', '86d00000-0000-0000-0000-000000000016') THEN 3
        ELSE 10
    END,
    reserved_value = 0, updated_at = updated_at + INTERVAL '1 second'
WHERE id IN (
    '86d00000-0000-0000-0000-000000000013',
    '86d00000-0000-0000-0000-000000000014',
    '86d00000-0000-0000-0000-000000000015',
    '86d00000-0000-0000-0000-000000000016',
    '86d00000-0000-0000-0000-000000000017'
);

INSERT INTO agent_limit_reconciliations (
    id, tenant_id, reservation_id, run_id, reservation_item_id,
    committed_amount, enforcement_basis
)
VALUES
    ('86f10000-0000-0000-0000-000000000021', '86000000-0000-0000-0000-000000000001', '86e00000-0000-0000-0000-000000000013', '86500000-0000-0000-0000-000000000002', '86f00000-0000-0000-0000-000000000021', 1, 'exact'),
    ('86f10000-0000-0000-0000-000000000022', '86000000-0000-0000-0000-000000000001', '86e00000-0000-0000-0000-000000000013', '86500000-0000-0000-0000-000000000002', '86f00000-0000-0000-0000-000000000022', 0, 'estimated'),
    ('86f10000-0000-0000-0000-000000000023', '86000000-0000-0000-0000-000000000001', '86e00000-0000-0000-0000-000000000013', '86500000-0000-0000-0000-000000000002', '86f00000-0000-0000-0000-000000000023', 0, 'exact'),
    ('86f10000-0000-0000-0000-000000000024', '86000000-0000-0000-0000-000000000001', '86e00000-0000-0000-0000-000000000013', '86500000-0000-0000-0000-000000000002', '86f00000-0000-0000-0000-000000000024', 0, 'exact'),
    ('86f10000-0000-0000-0000-000000000025', '86000000-0000-0000-0000-000000000001', '86e00000-0000-0000-0000-000000000013', '86500000-0000-0000-0000-000000000002', '86f00000-0000-0000-0000-000000000025', 0, 'exact');

UPDATE agent_limit_reservations
SET status = 'committed', committed_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86e00000-0000-0000-0000-000000000013';

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_limit_reconciliations (
            tenant_id, reservation_id, run_id, reservation_item_id,
            committed_amount, enforcement_basis
        ) VALUES (
            '86000000-0000-0000-0000-000000000001',
            '86e00000-0000-0000-0000-000000000013',
            '86500000-0000-0000-0000-000000000002',
            '86f00000-0000-0000-0000-000000000023', 0, 'exact'
        )
    $statement$,
    'claimed allowed reservation item'
);

INSERT INTO agent_usage_events (
    id, tenant_id, event_kind, run_id, thread_id, actor_user_id,
    role_keys, origin_module_key, task_class, provider_attempt_id,
    provider_turn_index, provider_attempt_index, provider_connection_id,
    provider_key, provider_model_id, provider_model_snapshot_id,
    route_priority, failure_origin, failure_category, outcome,
    safe_failure_code, duration_ms, request_id, correlation_id,
    limit_reservation_id, occurred_at
)
VALUES (
    '87000000-0000-0000-0000-000000000013',
    '86000000-0000-0000-0000-000000000001', 'provider_attempt',
    '86500000-0000-0000-0000-000000000002',
    '86200000-0000-0000-0000-000000000002',
    '86100000-0000-0000-0000-000000000001', ARRAY['campus_owner'],
    'fleet', 'module_read_reporting',
    '86a00000-0000-0000-0000-000000000003', 1, 3,
    '86600000-0000-0000-0000-000000000011', 'openai', 'contract-model',
    '86700000-0000-0000-0000-000000000001', 1,
    'preflight', 'connection_unavailable', 'failed',
    'connection_unavailable', 1,
    '86510000-0000-0000-0000-000000000002',
    '86520000-0000-0000-0000-000000000002',
    '86e00000-0000-0000-0000-000000000013', NOW()
);

INSERT INTO agent_usage_measures (
    tenant_id, usage_event_id, meter_key, amount,
    enforcement_amount, enforcement_basis,
    currency_code, currency_exponent, pricing_version
)
VALUES
    ('86000000-0000-0000-0000-000000000001', '87000000-0000-0000-0000-000000000013', 'agent.provider_attempts', 1, 1, 'exact', NULL, NULL, NULL),
    ('86000000-0000-0000-0000-000000000001', '87000000-0000-0000-0000-000000000013', 'agent.input_tokens', NULL, 0, 'exact', NULL, NULL, NULL),
    ('86000000-0000-0000-0000-000000000001', '87000000-0000-0000-0000-000000000013', 'agent.output_tokens', NULL, 0, 'exact', NULL, NULL, NULL),
    ('86000000-0000-0000-0000-000000000001', '87000000-0000-0000-0000-000000000013', 'agent.cached_input_tokens', NULL, NULL, NULL, NULL, NULL, NULL),
    ('86000000-0000-0000-0000-000000000001', '87000000-0000-0000-0000-000000000013', 'agent.reasoning_tokens', NULL, NULL, NULL, NULL, NULL, NULL),
    ('86000000-0000-0000-0000-000000000001', '87000000-0000-0000-0000-000000000013', 'agent.provider_reported_cost', NULL, NULL, NULL, NULL, NULL, NULL),
    ('86000000-0000-0000-0000-000000000001', '87000000-0000-0000-0000-000000000013', 'agent.estimated_cost', NULL, 0, 'estimated', 'USD', 6, 'pricing-v1');

SET CONSTRAINTS agent_usage_events_measure_set_constraint IMMEDIATE;
SET CONSTRAINTS agent_usage_events_measure_set_constraint DEFERRED;

SELECT pg_temp.assert_true(
    (SELECT status = 'committed'
     FROM agent_limit_reservations
     WHERE id = '86e00000-0000-0000-0000-000000000013')
    AND (SELECT committed_value = 3 AND reserved_value = 0
         FROM agent_limit_buckets
         WHERE id = '86d00000-0000-0000-0000-000000000015'),
    'lost-ack replay must leave one terminal reservation and move counters once'
);

-- A second attempt is a separate immutable usage fact. Once local hard rules
-- are archived its exact child reservation finalizes not_limited, is claimed
-- once, and still remains bound to the fallback attempt.
UPDATE agent_limit_rules
SET deleted_at = NOW(), version = 2, change_reason = 'Archive after reserve test',
    updated_at = updated_at + INTERVAL '1 second'
WHERE id IN (
    '86c00000-0000-0000-0000-000000000013',
    '86c00000-0000-0000-0000-000000000014',
    '86c00000-0000-0000-0000-000000000015',
    '86c00000-0000-0000-0000-000000000016',
    '86c00000-0000-0000-0000-000000000017'
);

INSERT INTO agent_provider_attempts (
    id, tenant_id, run_id, turn_index, attempt_index, route_set_id,
    route_version, route_target_id, connection_id, credential_version,
    model_snapshot_id, provider_key, provider_model_id, task_class
)
VALUES (
    '86a00000-0000-0000-0000-000000000002',
    '86000000-0000-0000-0000-000000000001',
    '86500000-0000-0000-0000-000000000002', 1, 2,
    '86800000-0000-0000-0000-000000000001', 1,
    '86900000-0000-0000-0000-000000000001',
    '86600000-0000-0000-0000-000000000011', 1,
    '86700000-0000-0000-0000-000000000001',
    'openai', 'contract-model', 'module_read_reporting'
);

INSERT INTO agent_execution_steps (
    id, tenant_id, run_id, step_index, turn_index, step_kind,
    provider_attempt_id, input_fingerprint
)
VALUES (
    '86a10000-0000-0000-0000-000000000002',
    '86000000-0000-0000-0000-000000000001',
    '86500000-0000-0000-0000-000000000002', 2, 1,
    'provider_attempt', '86a00000-0000-0000-0000-000000000002',
    DECODE(REPEAT('22', 32), 'hex')
);

INSERT INTO agent_limit_reservations (
    id, tenant_id, run_id, provider_attempt_id, actor_user_id, role_keys,
    origin_module_key, provider_key, provider_model_id, stage_kind,
    stage_sequence, idempotency_key, request_fingerprint
)
VALUES (
    '86e00000-0000-0000-0000-000000000012',
    '86000000-0000-0000-0000-000000000001',
    '86500000-0000-0000-0000-000000000002',
    '86a00000-0000-0000-0000-000000000002',
    '86100000-0000-0000-0000-000000000001', ARRAY['campus_owner'],
    'fleet', 'openai', 'contract-model', 'provider_attempt', 2,
    'agent086-provider-two', DECODE(REPEAT('22', 32), 'hex')
);

UPDATE agent_limit_reservations
SET status = 'not_limited', updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86e00000-0000-0000-0000-000000000012';

UPDATE agent_limit_reservations
SET claimed_at = STATEMENT_TIMESTAMP(), claimed_by_worker_id = 'agent-086-worker',
    claim_fence_version = 2, updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86e00000-0000-0000-0000-000000000012';

UPDATE agent_provider_attempts
SET status = 'succeeded', input_tokens = 4, output_tokens = 2,
    finished_at = NOW(), updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86a00000-0000-0000-0000-000000000002';

INSERT INTO agent_execution_artifacts (
    id, tenant_id, run_id, step_id, artifact_sequence, artifact_kind,
    ciphertext, ciphertext_sha256, plaintext_sha256, nonce,
    encryption_key_id, encryption_key_version, plaintext_length
)
VALUES (
    '86a20000-0000-0000-0000-000000000002',
    '86000000-0000-0000-0000-000000000001',
    '86500000-0000-0000-0000-000000000002',
    '86a10000-0000-0000-0000-000000000002', 1, 'provider_result',
    DECODE(REPEAT('ab', 16), 'hex'),
    DECODE(REPEAT('01', 32), 'hex'), DECODE(REPEAT('02', 32), 'hex'),
    DECODE(REPEAT('cd', 12), 'hex'), 'agent-086-artifact-key', 1, 16
);

UPDATE agent_execution_steps
SET status = 'succeeded', finished_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86a10000-0000-0000-0000-000000000002';

INSERT INTO agent_usage_events (
    id, tenant_id, event_kind, run_id, thread_id, actor_user_id,
    role_keys, origin_module_key, task_class, provider_attempt_id,
    provider_turn_index, provider_attempt_index, provider_connection_id,
    provider_key, provider_model_id, provider_model_snapshot_id,
    route_priority, outcome, duration_ms, request_id, correlation_id,
    limit_reservation_id, occurred_at
)
VALUES (
    '87000000-0000-0000-0000-000000000012',
    '86000000-0000-0000-0000-000000000001', 'provider_attempt',
    '86500000-0000-0000-0000-000000000002',
    '86200000-0000-0000-0000-000000000002',
    '86100000-0000-0000-0000-000000000001', ARRAY['campus_owner'],
    'fleet', 'module_read_reporting',
    '86a00000-0000-0000-0000-000000000002', 1, 2,
    '86600000-0000-0000-0000-000000000011',
    'openai', 'contract-model',
    '86700000-0000-0000-0000-000000000001', 1,
    'succeeded', 18,
    '86510000-0000-0000-0000-000000000002',
    '86520000-0000-0000-0000-000000000002',
    '86e00000-0000-0000-0000-000000000012', NOW()
);

INSERT INTO agent_usage_measures (
    tenant_id, usage_event_id, meter_key, amount,
    enforcement_amount, enforcement_basis,
    currency_code, currency_exponent, pricing_version
)
VALUES
    ('86000000-0000-0000-0000-000000000001', '87000000-0000-0000-0000-000000000012', 'agent.provider_attempts', 1, NULL, NULL, NULL, NULL, NULL),
    ('86000000-0000-0000-0000-000000000001', '87000000-0000-0000-0000-000000000012', 'agent.input_tokens', 4, NULL, NULL, NULL, NULL, NULL),
    ('86000000-0000-0000-0000-000000000001', '87000000-0000-0000-0000-000000000012', 'agent.output_tokens', 2, NULL, NULL, NULL, NULL, NULL),
    ('86000000-0000-0000-0000-000000000001', '87000000-0000-0000-0000-000000000012', 'agent.cached_input_tokens', NULL, NULL, NULL, NULL, NULL, NULL),
    ('86000000-0000-0000-0000-000000000001', '87000000-0000-0000-0000-000000000012', 'agent.reasoning_tokens', NULL, NULL, NULL, NULL, NULL, NULL),
    ('86000000-0000-0000-0000-000000000001', '87000000-0000-0000-0000-000000000012', 'agent.provider_reported_cost', NULL, NULL, NULL, NULL, NULL, NULL),
    ('86000000-0000-0000-0000-000000000001', '87000000-0000-0000-0000-000000000012', 'agent.estimated_cost', NULL, NULL, NULL, NULL, NULL, NULL);

SET CONSTRAINTS agent_usage_events_measure_set_constraint,
    agent_usage_events_denial_audit_constraint IMMEDIATE;
SET CONSTRAINTS agent_usage_events_measure_set_constraint,
    agent_usage_events_denial_audit_constraint DEFERRED;

SELECT pg_temp.assert_true(
    (SELECT COUNT(*) FROM agent_usage_events
     WHERE run_id = '86500000-0000-0000-0000-000000000002'
       AND event_kind = 'provider_attempt') = 3,
    'upstream failure, preflight failure, and fallback must remain visible'
);

-- Signed commercial quota retains the reserved upper bound while its immutable
-- event and bucket settle the actual amount. Agent tables only map this source.
INSERT INTO entitlement_limits (
    tenant_id, limit_key, source_lease_id, unit, period, limit_value, enforcement
)
VALUES (
    '86000000-0000-0000-0000-000000000001', 'agent.input_tokens',
    '86d10000-0000-0000-0000-000000000001', 'token', 'none', 100, 'hard'
);

INSERT INTO entitlement_meter_buckets (
    id, tenant_id, limit_key, period_start, period_end, reserved_value
)
VALUES (
    '86d20000-0000-0000-0000-000000000002',
    '86000000-0000-0000-0000-000000000001', 'agent.input_tokens',
    TIMESTAMPTZ '1970-01-01 00:00:00+00', NULL, 10
);

INSERT INTO agent_provider_attempts (
    id, tenant_id, run_id, turn_index, attempt_index, route_set_id,
    route_version, route_target_id, connection_id, credential_version,
    model_snapshot_id, provider_key, provider_model_id, task_class
)
VALUES (
    '86a00000-0000-0000-0000-000000000004',
    '86000000-0000-0000-0000-000000000001',
    '86500000-0000-0000-0000-000000000002', 2, 1,
    '86800000-0000-0000-0000-000000000001', 1,
    '86900000-0000-0000-0000-000000000001',
    '86600000-0000-0000-0000-000000000011', 1,
    '86700000-0000-0000-0000-000000000001',
    'openai', 'contract-model', 'module_read_reporting'
);

INSERT INTO agent_execution_steps (
    id, tenant_id, run_id, step_index, turn_index, step_kind,
    provider_attempt_id, input_fingerprint
)
VALUES (
    '86a10000-0000-0000-0000-000000000004',
    '86000000-0000-0000-0000-000000000001',
    '86500000-0000-0000-0000-000000000002', 4, 2,
    'provider_attempt', '86a00000-0000-0000-0000-000000000004',
    DECODE(REPEAT('14', 32), 'hex')
);

INSERT INTO entitlement_usage_reservations (
    id, tenant_id, bucket_id, source_lease_id, limit_key, unit,
    operation_key, actor_user_id, idempotency_key, amount, expires_at
)
VALUES (
    '86d30000-0000-0000-0000-000000000002',
    '86000000-0000-0000-0000-000000000001',
    '86d20000-0000-0000-0000-000000000002',
    '86d10000-0000-0000-0000-000000000001',
    'agent.input_tokens', 'token', 'agent.runtime',
    '86100000-0000-0000-0000-000000000001',
    'agent086-signed-ten-to-three', 10, NOW() + INTERVAL '5 minutes'
);

INSERT INTO agent_limit_reservations (
    id, tenant_id, run_id, provider_attempt_id, actor_user_id, role_keys,
    origin_module_key, provider_key, provider_model_id, stage_kind,
    stage_sequence, idempotency_key, request_fingerprint
)
VALUES (
    '86e00000-0000-0000-0000-000000000014',
    '86000000-0000-0000-0000-000000000001',
    '86500000-0000-0000-0000-000000000002',
    '86a00000-0000-0000-0000-000000000004',
    '86100000-0000-0000-0000-000000000001', ARRAY['campus_owner'],
    'fleet', 'openai', 'contract-model', 'provider_attempt', 4,
    'agent086-signed-upper-bound', DECODE(REPEAT('14', 32), 'hex')
);

INSERT INTO agent_limit_reservation_items (
    id, tenant_id, reservation_id, run_id, item_sequence,
    entitlement_bucket_id, entitlement_reservation_id, definition_kind,
    source_lease_id, entitlement_limit_key, scope_kind, scope_value,
    meter_key, unit, period, period_start, limit_value, committed_before,
    reserved_before, requested_amount, reserved_amount, decision
)
VALUES (
    '86f00000-0000-0000-0000-000000000031',
    '86000000-0000-0000-0000-000000000001',
    '86e00000-0000-0000-0000-000000000014',
    '86500000-0000-0000-0000-000000000002', 1,
    '86d20000-0000-0000-0000-000000000002',
    '86d30000-0000-0000-0000-000000000002', 'signed_entitlement',
    '86d10000-0000-0000-0000-000000000001', 'agent.input_tokens',
    'campus', '86000000-0000-0000-0000-000000000001',
    'agent.input_tokens', 'token', 'none',
    TIMESTAMPTZ '1970-01-01 00:00:00+00', 100, 0, 0, 10, 10, 'allowed'
);

UPDATE agent_limit_reservations
SET status = 'reserved', expires_at = NOW() + INTERVAL '5 minutes',
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86e00000-0000-0000-0000-000000000014';

UPDATE agent_limit_reservations
SET claimed_at = STATEMENT_TIMESTAMP(), claimed_by_worker_id = 'agent-086-worker',
    claim_fence_version = 2, updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86e00000-0000-0000-0000-000000000014';

UPDATE agent_provider_attempts
SET status = 'failed', failure_origin = 'upstream',
    failure_category = 'rate_limited', input_tokens = 3,
    finished_at = NOW(), updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86a00000-0000-0000-0000-000000000004';

UPDATE agent_execution_steps
SET status = 'failed', safe_failure_code = 'rate_limited',
    finished_at = NOW(), updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86a10000-0000-0000-0000-000000000004';

UPDATE entitlement_meter_buckets
SET committed_value = 3, reserved_value = 0,
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86d20000-0000-0000-0000-000000000002';

UPDATE entitlement_usage_reservations
SET status = 'committed', committed_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86d30000-0000-0000-0000-000000000002';

INSERT INTO entitlement_usage_events (
    tenant_id, reservation_id, source_lease_id, limit_key, unit,
    operation_key, actor_user_id, amount, period_start, period_end, occurred_at
)
VALUES (
    '86000000-0000-0000-0000-000000000001',
    '86d30000-0000-0000-0000-000000000002',
    '86d10000-0000-0000-0000-000000000001',
    'agent.input_tokens', 'token', 'agent.runtime',
    '86100000-0000-0000-0000-000000000001', 3,
    TIMESTAMPTZ '1970-01-01 00:00:00+00', NULL, NOW()
);

INSERT INTO agent_limit_reconciliations (
    id, tenant_id, reservation_id, run_id, reservation_item_id,
    committed_amount, enforcement_basis
)
VALUES (
    '86f10000-0000-0000-0000-000000000031',
    '86000000-0000-0000-0000-000000000001',
    '86e00000-0000-0000-0000-000000000014',
    '86500000-0000-0000-0000-000000000002',
    '86f00000-0000-0000-0000-000000000031', 3, 'exact'
);

UPDATE agent_limit_reservations
SET status = 'committed', committed_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86e00000-0000-0000-0000-000000000014';

SELECT pg_temp.assert_true(
    (SELECT amount FROM entitlement_usage_reservations
     WHERE id = '86d30000-0000-0000-0000-000000000002') = 10
    AND (SELECT amount FROM entitlement_usage_events
         WHERE reservation_id = '86d30000-0000-0000-0000-000000000002') = 3
    AND (SELECT committed_value = 3 AND reserved_value = 0
         FROM entitlement_meter_buckets
         WHERE id = '86d20000-0000-0000-0000-000000000002'),
    'signed 10 to 3 settlement must retain the canonical upper bound'
);

INSERT INTO agent_usage_events (
    id, tenant_id, event_kind, run_id, thread_id, actor_user_id,
    role_keys, origin_module_key, task_class, provider_attempt_id,
    provider_turn_index, provider_attempt_index, provider_connection_id,
    provider_key, provider_model_id, provider_model_snapshot_id,
    route_priority, failure_origin, failure_category, outcome,
    safe_failure_code, duration_ms, request_id, correlation_id,
    limit_reservation_id, occurred_at
)
VALUES (
    '87000000-0000-0000-0000-000000000014',
    '86000000-0000-0000-0000-000000000001', 'provider_attempt',
    '86500000-0000-0000-0000-000000000002',
    '86200000-0000-0000-0000-000000000002',
    '86100000-0000-0000-0000-000000000001', ARRAY['campus_owner'],
    'fleet', 'module_read_reporting',
    '86a00000-0000-0000-0000-000000000004', 2, 1,
    '86600000-0000-0000-0000-000000000011', 'openai', 'contract-model',
    '86700000-0000-0000-0000-000000000001', 1,
    'upstream', 'rate_limited', 'failed', 'rate_limited', 2,
    '86510000-0000-0000-0000-000000000002',
    '86520000-0000-0000-0000-000000000002',
    '86e00000-0000-0000-0000-000000000014', NOW()
);

INSERT INTO agent_usage_measures (
    tenant_id, usage_event_id, meter_key, amount,
    enforcement_amount, enforcement_basis,
    currency_code, currency_exponent, pricing_version
)
VALUES
    ('86000000-0000-0000-0000-000000000001', '87000000-0000-0000-0000-000000000014', 'agent.provider_attempts', 1, NULL, NULL, NULL, NULL, NULL),
    ('86000000-0000-0000-0000-000000000001', '87000000-0000-0000-0000-000000000014', 'agent.input_tokens', 3, 3, 'exact', NULL, NULL, NULL),
    ('86000000-0000-0000-0000-000000000001', '87000000-0000-0000-0000-000000000014', 'agent.output_tokens', NULL, NULL, NULL, NULL, NULL, NULL),
    ('86000000-0000-0000-0000-000000000001', '87000000-0000-0000-0000-000000000014', 'agent.cached_input_tokens', NULL, NULL, NULL, NULL, NULL, NULL),
    ('86000000-0000-0000-0000-000000000001', '87000000-0000-0000-0000-000000000014', 'agent.reasoning_tokens', NULL, NULL, NULL, NULL, NULL, NULL),
    ('86000000-0000-0000-0000-000000000001', '87000000-0000-0000-0000-000000000014', 'agent.provider_reported_cost', NULL, NULL, NULL, NULL, NULL, NULL),
    ('86000000-0000-0000-0000-000000000001', '87000000-0000-0000-0000-000000000014', 'agent.estimated_cost', NULL, NULL, NULL, NULL, NULL, NULL);

SET CONSTRAINTS agent_usage_events_measure_set_constraint IMMEDIATE;
SET CONSTRAINTS agent_usage_events_measure_set_constraint DEFERRED;

-- Capability execution uses the same exact child binding and one-time claim.
INSERT INTO agent_run_queue (run_id, tenant_id)
VALUES (
    '86500000-0000-0000-0000-000000000003',
    '86000000-0000-0000-0000-000000000001'
);

UPDATE agent_run_queue
SET state = 'leased',
    lease_token = '86600000-0000-0000-0000-000000000003',
    leased_by = 'agent-086-worker',
    heartbeat_at = STATEMENT_TIMESTAMP(),
    lease_expires_at = STATEMENT_TIMESTAMP() + INTERVAL '30 seconds',
    delivery_attempt = 1, version = 2,
    updated_at = updated_at + INTERVAL '1 second'
WHERE run_id = '86500000-0000-0000-0000-000000000003';

UPDATE agent_runs
SET status = 'running', started_at = NOW(), version = 2,
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86500000-0000-0000-0000-000000000003';

INSERT INTO agent_capability_calls (
    id, tenant_id, run_id, call_sequence, capability_key, capability_version,
    product_operation_key, owning_module_key, required_permission,
    input_fingerprint, scope_kind, resource_references
)
VALUES (
    '86b00000-0000-0000-0000-000000000001',
    '86000000-0000-0000-0000-000000000001',
    '86500000-0000-0000-0000-000000000003', 1,
    'fleet.vehicles.list', 1, 'fleet.vehicles.list', 'fleet', 'fleet:view',
    DECODE(REPEAT('33', 32), 'hex'), 'tenant_wide', '[]'::JSONB
);

INSERT INTO agent_execution_steps (
    id, tenant_id, run_id, step_index, turn_index, step_kind,
    capability_call_id, input_fingerprint
)
VALUES (
    '86b10000-0000-0000-0000-000000000001',
    '86000000-0000-0000-0000-000000000001',
    '86500000-0000-0000-0000-000000000003', 1, 1,
    'capability_call', '86b00000-0000-0000-0000-000000000001',
    DECODE(REPEAT('33', 32), 'hex')
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_limit_reservations (
            tenant_id, run_id, capability_call_id, actor_user_id, role_keys,
            origin_module_key, capability_module_key, capability_key,
            stage_kind, stage_sequence, idempotency_key, request_fingerprint
        ) VALUES (
            '86000000-0000-0000-0000-000000000001',
            '86500000-0000-0000-0000-000000000003',
            '86b00000-0000-0000-0000-000000000001',
            '86100000-0000-0000-0000-000000000001', ARRAY['campus_owner'],
            'fleet', 'fleet', 'fleet.vehicles.list', 'capability_call', 1,
            'agent086-cap-wrong-fingerprint', DECODE(REPEAT('34', 32), 'hex')
        )
    $statement$,
    'exact child input'
);

INSERT INTO agent_limit_reservations (
    id, tenant_id, run_id, capability_call_id, actor_user_id, role_keys,
    origin_module_key, capability_module_key, capability_key,
    stage_kind, stage_sequence, idempotency_key, request_fingerprint
)
VALUES (
    '86e00000-0000-0000-0000-000000000021',
    '86000000-0000-0000-0000-000000000001',
    '86500000-0000-0000-0000-000000000003',
    '86b00000-0000-0000-0000-000000000001',
    '86100000-0000-0000-0000-000000000001', ARRAY['campus_owner'],
    'fleet', 'fleet', 'fleet.vehicles.list', 'capability_call', 1,
    'agent086-capability-one', DECODE(REPEAT('33', 32), 'hex')
);

UPDATE agent_limit_reservations
SET status = 'not_limited', updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86e00000-0000-0000-0000-000000000021';

UPDATE agent_limit_reservations
SET claimed_at = STATEMENT_TIMESTAMP(), claimed_by_worker_id = 'agent-086-worker',
    claim_fence_version = 2, updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86e00000-0000-0000-0000-000000000021';

UPDATE agent_capability_calls
SET status = 'denied', safe_failure_code = 'permission_denied',
    duration_ms = 3, finished_at = NOW(),
    updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86b00000-0000-0000-0000-000000000001';

INSERT INTO agent_execution_artifacts (
    id, tenant_id, run_id, step_id, artifact_sequence, artifact_kind,
    ciphertext, ciphertext_sha256, plaintext_sha256, nonce,
    encryption_key_id, encryption_key_version, plaintext_length
)
VALUES (
    '86b20000-0000-0000-0000-000000000001',
    '86000000-0000-0000-0000-000000000001',
    '86500000-0000-0000-0000-000000000003',
    '86b10000-0000-0000-0000-000000000001', 1, 'capability_result',
    DECODE(REPEAT('ef', 16), 'hex'),
    DECODE(REPEAT('03', 32), 'hex'), DECODE(REPEAT('04', 32), 'hex'),
    DECODE(REPEAT('dc', 12), 'hex'), 'agent-086-artifact-key', 1, 16
);

UPDATE agent_execution_steps
SET status = 'failed', safe_failure_code = 'permission_denied',
    finished_at = NOW(), updated_at = updated_at + INTERVAL '1 second'
WHERE id = '86b10000-0000-0000-0000-000000000001';

INSERT INTO actor_audit_events (
    id, tenant_id, actor_type, actor_user_id, action_key, target_type,
    target_id, outcome, request_id, correlation_id, agent_run_id, reason
)
VALUES (
    '87010000-0000-0000-0000-000000000021',
    '86000000-0000-0000-0000-000000000001', 'agent',
    '86100000-0000-0000-0000-000000000001', 'fleet.vehicles.list',
    'capability_call', '86b00000-0000-0000-0000-000000000001', 'denied',
    '86510000-0000-0000-0000-000000000003',
    '86520000-0000-0000-0000-000000000003',
    '86500000-0000-0000-0000-000000000003', 'Permission denied'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO agent_usage_events (
            tenant_id, event_kind, run_id, thread_id, actor_user_id,
            role_keys, origin_module_key, task_class, capability_call_id,
            capability_module_key, capability_key, capability_version,
            approval_state, outcome, safe_failure_code, duration_ms,
            request_id, correlation_id, limit_reservation_id, occurred_at
        ) VALUES (
            '86000000-0000-0000-0000-000000000001', 'capability_call',
            '86500000-0000-0000-0000-000000000003',
            '86200000-0000-0000-0000-000000000003',
            '86100000-0000-0000-0000-000000000001', ARRAY['campus_owner'],
            'fleet', 'module_read_reporting',
            '86b00000-0000-0000-0000-000000000001',
            'fleet', 'fleet.vehicles.list', 1, 'not_required',
            'denied', 'permission_denied', 3,
            '86510000-0000-0000-0000-000000000003',
            '86520000-0000-0000-0000-000000000003',
            '86e00000-0000-0000-0000-000000000011', NOW()
        )
    $statement$,
    'compatible terminal limit decision'
);

INSERT INTO agent_usage_events (
    id, tenant_id, event_kind, run_id, thread_id, actor_user_id,
    role_keys, origin_module_key, task_class, capability_call_id,
    capability_module_key, capability_key, capability_version,
    approval_state, outcome, safe_failure_code, duration_ms,
    request_id, correlation_id, limit_reservation_id, occurred_at
)
VALUES (
    '87000000-0000-0000-0000-000000000021',
    '86000000-0000-0000-0000-000000000001', 'capability_call',
    '86500000-0000-0000-0000-000000000003',
    '86200000-0000-0000-0000-000000000003',
    '86100000-0000-0000-0000-000000000001', ARRAY['campus_owner'],
    'fleet', 'module_read_reporting',
    '86b00000-0000-0000-0000-000000000001',
    'fleet', 'fleet.vehicles.list', 1, 'not_required',
    'denied', 'permission_denied', 3,
    '86510000-0000-0000-0000-000000000003',
    '86520000-0000-0000-0000-000000000003',
    '86e00000-0000-0000-0000-000000000021', NOW()
);

INSERT INTO agent_usage_measures (
    tenant_id, usage_event_id, meter_key, amount
)
VALUES (
    '86000000-0000-0000-0000-000000000001',
    '87000000-0000-0000-0000-000000000021',
    'agent.capability_calls', 1
);

SET CONSTRAINTS agent_usage_events_measure_set_constraint,
    agent_usage_events_denial_audit_constraint IMMEDIATE;

SELECT pg_temp.assert_true(
    (SELECT COUNT(DISTINCT event_kind) FROM agent_usage_events
     WHERE tenant_id = '86000000-0000-0000-0000-000000000001') = 3,
    'run, provider-attempt, and capability usage facts must all be exportable'
);

SELECT pg_temp.assert_true(
    (SELECT COUNT(*) FROM agent_usage_measures AS measure
     INNER JOIN agent_usage_events AS event ON event.id = measure.usage_event_id
     WHERE event.event_kind = 'provider_attempt') = 28,
    'each provider attempt must retain all seven canonical nullable measures'
);

SET CONSTRAINTS agent_limit_reconciliations_parent_constraint IMMEDIATE;
SET CONSTRAINTS agent_limit_reconciliations_parent_constraint DEFERRED;

SELECT pg_temp.expect_failure(
    'TRUNCATE TABLE agent_limit_reconciliations',
    'append-only'
);

SELECT pg_temp.expect_failure(
    'TRUNCATE TABLE agent_usage_events CASCADE',
    'append-only'
);

SELECT pg_temp.expect_failure(
    'TRUNCATE TABLE agent_limit_reservations CASCADE',
    'append-only'
);

ROLLBACK;

SELECT '086 agent usage limits contract passed' AS result;
