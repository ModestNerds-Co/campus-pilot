-- Adversarial contract for migration 089. The caller applies migrations first.
-- Every fixture and assertion is rolled back.

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
    ('89000000-0000-0000-0000-000000000001', 'scope-089-a', 'Scope 089 A'),
    ('99000000-0000-0000-0000-000000000001', 'scope-089-b', 'Scope 089 B');

SELECT pg_temp.assert_true(
    (
        SELECT COUNT(*) = 23
        FROM role_record_scope_grants
        WHERE tenant_id = '89000000-0000-0000-0000-000000000001'
          AND deleted_at IS NULL
    ),
    'future tenant provisioning must seed the complete role-scope evidence set'
);

SELECT pg_temp.assert_true(
    (
        SELECT COUNT(*) = 7
        FROM role_record_scope_grants AS scope_grant
        INNER JOIN roles AS role
            ON role.id = scope_grant.role_id
           AND role.tenant_id = scope_grant.tenant_id
        WHERE scope_grant.tenant_id = '89000000-0000-0000-0000-000000000001'
          AND role.key = 'registrar'
          AND scope_grant.scope_kind = 'campus'
          AND scope_grant.deleted_at IS NULL
    ),
    'registrar must receive seven campus SIS families'
);

SELECT pg_temp.assert_true(
    NOT EXISTS (
        SELECT 1
        FROM role_record_scope_grants AS scope_grant
        INNER JOIN roles AS role
            ON role.id = scope_grant.role_id
           AND role.tenant_id = scope_grant.tenant_id
        WHERE scope_grant.tenant_id = '89000000-0000-0000-0000-000000000001'
          AND role.key IN ('campus_owner', 'librarian')
          AND scope_grant.deleted_at IS NULL
    ),
    'wildcard owners and librarians must not receive implicit sensitive-family rows'
);

INSERT INTO roles (
    id, tenant_id, key, name, description, permissions, is_system
) VALUES (
    '89100000-0000-0000-0000-000000000001',
    '89000000-0000-0000-0000-000000000001',
    'custom_records_role',
    'Custom Records Role',
    'Test-only custom role',
    ARRAY['sis:view']::TEXT[],
    FALSE
);

SELECT pg_temp.assert_true(
    NOT EXISTS (
        SELECT 1
        FROM role_record_scope_grants
        WHERE tenant_id = '89000000-0000-0000-0000-000000000001'
          AND role_id = '89100000-0000-0000-0000-000000000001'
    ),
    'custom roles must start with no implicit record scope'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO role_record_scope_grants (
            tenant_id, role_id, scope_family, scope_kind
        ) VALUES (
            '89000000-0000-0000-0000-000000000001',
            (
                SELECT id FROM roles
                WHERE tenant_id = '99000000-0000-0000-0000-000000000001'
                  AND key = 'registrar'
            ),
            'sis.learners',
            'campus'
        )
    $statement$,
    'fk_role_record_scope_grants_tenant_role'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO role_record_scope_grants (
            tenant_id, role_id, scope_family, scope_kind
        ) VALUES (
            '89000000-0000-0000-0000-000000000001',
            '89100000-0000-0000-0000-000000000001',
            'SIS.learners',
            'campus'
        )
    $statement$,
    'role_record_scope_grants_scope_family_check'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO role_record_scope_grants (
            tenant_id, role_id, scope_family, scope_kind
        ) VALUES (
            '89000000-0000-0000-0000-000000000001',
            '89100000-0000-0000-0000-000000000001',
            'sis.learners',
            'tenant_wide'
        )
    $statement$,
    'role_record_scope_grants_scope_kind_check'
);

INSERT INTO role_record_scope_grants (
    tenant_id, role_id, scope_family, scope_kind
) VALUES (
    '89000000-0000-0000-0000-000000000001',
    '89100000-0000-0000-0000-000000000001',
    'sis.learners',
    'campus'
);

SELECT pg_temp.expect_failure(
    $statement$
        INSERT INTO role_record_scope_grants (
            tenant_id, role_id, scope_family, scope_kind
        ) VALUES (
            '89000000-0000-0000-0000-000000000001',
            '89100000-0000-0000-0000-000000000001',
            'sis.learners',
            'campus'
        )
    $statement$,
    'idx_role_record_scope_grants_active'
);

UPDATE role_record_scope_grants
SET deleted_at = NOW(), updated_at = NOW()
WHERE tenant_id = '89000000-0000-0000-0000-000000000001'
  AND role_id = '89100000-0000-0000-0000-000000000001'
  AND scope_family = 'sis.learners'
  AND scope_kind = 'campus'
  AND deleted_at IS NULL;

INSERT INTO role_record_scope_grants (
    tenant_id, role_id, scope_family, scope_kind
) VALUES (
    '89000000-0000-0000-0000-000000000001',
    '89100000-0000-0000-0000-000000000001',
    'sis.learners',
    'campus'
);

DELETE FROM roles
WHERE tenant_id = '89000000-0000-0000-0000-000000000001'
  AND id = '89100000-0000-0000-0000-000000000001';

SELECT pg_temp.assert_true(
    NOT EXISTS (
        SELECT 1
        FROM role_record_scope_grants
        WHERE tenant_id = '89000000-0000-0000-0000-000000000001'
          AND role_id = '89100000-0000-0000-0000-000000000001'
    ),
    'role removal must cascade every active and historical scope grant'
);

ROLLBACK;
