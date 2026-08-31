-- Adversarial contract for migration 090. The caller applies migrations first.
-- Fixtures, the simulated legacy state, and the migration replay are rolled back.

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
VALUES ('90000000-0000-0000-0000-000000000001', 'provider-090', 'Provider 090');

INSERT INTO users (id, tenant_id, email, password_hash, full_name)
VALUES (
    '90100000-0000-0000-0000-000000000001',
    '90000000-0000-0000-0000-000000000001',
    'owner@provider-090.test',
    'test-only',
    'Provider 090 Owner'
);

INSERT INTO ai_provider_connections (
    id, tenant_id, provider, auth_method, account_label, status,
    credential_ciphertext, credential_nonce, credential_key_id,
    credential_fingerprint, configured_by, model_catalog_version
) VALUES (
    '90200000-0000-0000-0000-000000000001',
    '90000000-0000-0000-0000-000000000001',
    'openai',
    'api_key',
    'Routed connection',
    'ready',
    DECODE(REPEAT('ab', 16), 'hex'),
    DECODE(REPEAT('cd', 12), 'hex'),
    'test-key',
    'provider-090-fingerprint',
    '90100000-0000-0000-0000-000000000001',
    1
);

INSERT INTO ai_provider_data_approval_versions (
    id, tenant_id, connection_id, approval_version, approval_class,
    change_source, changed_by, change_reason
) VALUES
    (
        '90300000-0000-0000-0000-000000000001',
        '90000000-0000-0000-0000-000000000001',
        '90200000-0000-0000-0000-000000000001',
        1,
        'unapproved',
        'system_default',
        NULL,
        'Initial test approval.'
    ),
    (
        '90300000-0000-0000-0000-000000000002',
        '90000000-0000-0000-0000-000000000001',
        '90200000-0000-0000-0000-000000000001',
        2,
        'sensitive_data_approved',
        'administrator',
        '90100000-0000-0000-0000-000000000001',
        'Approve test provider data.'
    );

INSERT INTO ai_provider_models (
    id, tenant_id, connection_id, credential_version, catalog_version,
    provider_model_id, display_name, context_window_tokens, max_output_tokens,
    supports_tools, refreshed_at
) VALUES (
    '90400000-0000-0000-0000-000000000001',
    '90000000-0000-0000-0000-000000000001',
    '90200000-0000-0000-0000-000000000001',
    1,
    1,
    'provider-090-model',
    'Provider 090 Model',
    8192,
    2048,
    FALSE,
    NOW()
);

INSERT INTO ai_route_sets (
    id, tenant_id, scope_kind, configured_by, change_reason
) VALUES (
    '90500000-0000-0000-0000-000000000001',
    '90000000-0000-0000-0000-000000000001',
    'tenant_default',
    '90100000-0000-0000-0000-000000000001',
    'Provider 090 route.'
);

INSERT INTO ai_task_routes (
    id, tenant_id, route_set_id, priority, connection_id, model_id,
    provider_data_approval_id, requires_tools, created_by
) VALUES (
    '90600000-0000-0000-0000-000000000001',
    '90000000-0000-0000-0000-000000000001',
    '90500000-0000-0000-0000-000000000001',
    1,
    '90200000-0000-0000-0000-000000000001',
    '90400000-0000-0000-0000-000000000001',
    '90300000-0000-0000-0000-000000000002',
    FALSE,
    '90100000-0000-0000-0000-000000000001'
);

-- Simulate the two legacy partial states that 090 repairs: a disconnected
-- connection without its default approval and a pre-pin route target.
INSERT INTO ai_provider_connections (
    id, tenant_id, provider, auth_method, account_label, status,
    configured_by, deleted_at
) VALUES (
    '90200000-0000-0000-0000-000000000002',
    '90000000-0000-0000-0000-000000000001',
    'anthropic',
    'api_key',
    'Disconnected legacy connection',
    'disconnected',
    '90100000-0000-0000-0000-000000000001',
    NOW()
);

DROP TRIGGER ai_task_routes_protect_lifecycle ON ai_task_routes;
ALTER TABLE ai_task_routes ALTER COLUMN provider_data_approval_id DROP NOT NULL;
UPDATE ai_task_routes
SET provider_data_approval_id = NULL
WHERE id = '90600000-0000-0000-0000-000000000001';
CREATE TRIGGER ai_task_routes_protect_lifecycle
    BEFORE UPDATE OR DELETE ON ai_task_routes
    FOR EACH ROW
    EXECUTE FUNCTION protect_ai_task_route_lifecycle();

\ir ../090_harden_ai_provider_data_eligibility.sql

SELECT pg_temp.assert_true(
    (
        SELECT COUNT(*) = 1
        FROM ai_provider_data_approval_versions
        WHERE tenant_id = '90000000-0000-0000-0000-000000000001'
          AND connection_id = '90200000-0000-0000-0000-000000000002'
          AND approval_version = 1
          AND approval_class = 'unapproved'
          AND change_source = 'system_default'
          AND changed_by IS NULL
    ),
    '090 must repair a missing disconnected-connection default as unapproved'
);

SELECT pg_temp.assert_true(
    (
        SELECT provider_data_approval_id = '90300000-0000-0000-0000-000000000002'
        FROM ai_task_routes
        WHERE id = '90600000-0000-0000-0000-000000000001'
    ),
    '090 must pin a legacy null route to its latest approval'
);

SELECT pg_temp.assert_true(
    (
        SELECT attnotnull
        FROM pg_attribute
        WHERE attrelid = 'ai_task_routes'::REGCLASS
          AND attname = 'provider_data_approval_id'
          AND NOT attisdropped
    ),
    '090 must restore the route approval not-null contract'
);

SELECT pg_temp.assert_true(
    (
        SELECT COUNT(*) = 1
        FROM pg_trigger
        WHERE tgrelid = 'ai_task_routes'::REGCLASS
          AND tgname = 'ai_task_routes_protect_lifecycle'
          AND NOT tgisinternal
    ),
    '090 must restore exactly one route lifecycle trigger'
);

INSERT INTO ai_provider_data_approval_versions (
    id, tenant_id, connection_id, approval_version, approval_class,
    change_source, changed_by, change_reason
) VALUES (
    '90300000-0000-0000-0000-000000000003',
    '90000000-0000-0000-0000-000000000001',
    '90200000-0000-0000-0000-000000000001',
    3,
    'sensitive_data_approved',
    'administrator',
    '90100000-0000-0000-0000-000000000001',
    'Rotate the test approval.'
);

SELECT pg_temp.assert_true(
    (
        SELECT provider_data_approval_id = '90300000-0000-0000-0000-000000000002'
        FROM ai_task_routes
        WHERE id = '90600000-0000-0000-0000-000000000001'
    ),
    '090 must not advance an existing route pin when approval changes'
);

SELECT pg_temp.expect_failure(
    $statement$
        UPDATE ai_task_routes
        SET provider_data_approval_id = '90300000-0000-0000-0000-000000000003',
            updated_at = updated_at + INTERVAL '1 second'
        WHERE id = '90600000-0000-0000-0000-000000000001'
    $statement$,
    'AI task route targets are immutable'
);

SELECT pg_temp.expect_failure(
    'TRUNCATE ai_provider_data_approval_versions CASCADE',
    'provider data approval versions are append-only'
);

SELECT pg_temp.assert_true(
    (
        SELECT POSITION(
            'model_output_unavailable' IN pg_get_constraintdef(oid)
        ) > 0
        AND POSITION(
            'output_budget_exceeded' IN pg_get_constraintdef(oid)
        ) > 0
        FROM pg_constraint
        WHERE conrelid = 'agent_provider_attempts'::REGCLASS
          AND conname = 'agent_provider_attempts_failure_category_check'
    ),
    '090 must admit both bounded model-output preflight categories'
);

SELECT pg_temp.assert_true(
    (
        SELECT POSITION(
            '''preflight''::text' IN pg_get_constraintdef(oid)
        ) > 0
        AND POSITION(
            'model_output_unavailable' IN pg_get_constraintdef(oid)
        ) > 0
        AND POSITION(
            'output_budget_exceeded' IN pg_get_constraintdef(oid)
        ) > 0
        FROM pg_constraint
        WHERE conrelid = 'agent_provider_attempts'::REGCLASS
          AND conname = 'agent_provider_attempts_failure_shape_check'
    ),
    '090 must classify both model-output failures as preflight-only'
);

ROLLBACK;
