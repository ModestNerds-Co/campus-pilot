-- Support-role SIS boundary contract. Every fixture and mutation is rolled back.

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

SELECT pg_temp.assert_true(
    NOT EXISTS (
        SELECT 1
        FROM roles
        WHERE key IN ('librarian', 'academic_manager')
          AND deleted_at IS NULL
          AND 'sis:view' = ANY(permissions)
    ),
    'support roles must use typed SIS dependencies instead of direct SIS workspace access'
);

INSERT INTO tenants (id, slug, name)
VALUES (
    '11400000-0000-0000-0000-000000000001',
    'support-role-boundary-114',
    'Support role boundary 114'
);

SELECT pg_temp.assert_true(
    NOT EXISTS (
        SELECT 1
        FROM roles
        WHERE tenant_id = '11400000-0000-0000-0000-000000000001'
          AND key IN ('librarian', 'academic_manager')
          AND 'sis:view' = ANY(permissions)
    ),
    'future support roles must not receive direct SIS workspace access'
);

UPDATE roles
SET permissions = permissions || ARRAY['sis:view']::TEXT[]
WHERE tenant_id = '11400000-0000-0000-0000-000000000001'
  AND key IN ('librarian', 'academic_manager');

SELECT pg_temp.assert_true(
    NOT EXISTS (
        SELECT 1
        FROM roles
        WHERE tenant_id = '11400000-0000-0000-0000-000000000001'
          AND key IN ('librarian', 'academic_manager')
          AND 'sis:view' = ANY(permissions)
    ),
    'direct role writes must not widen support roles into the SIS workspace'
);

ROLLBACK;

SELECT 'Support role permission boundary contract passed' AS result;
