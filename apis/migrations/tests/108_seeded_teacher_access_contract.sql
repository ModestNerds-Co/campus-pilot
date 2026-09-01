-- Seeded Teacher access contract. Run after migration 108.
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

SELECT pg_temp.assert_true(
    NOT EXISTS (
        SELECT 1
        FROM roles
        WHERE key = 'teacher'
          AND deleted_at IS NULL
          AND (
              permissions && ARRAY[
                  'academics:create',
                  'academics:edit',
                  'academics:delete',
                  'academics:manage',
                  'administration:view',
                  'users:create',
                  'users:edit',
                  'users:delete',
                  'roles:create',
                  'roles:edit',
                  'roles:assign'
              ]::TEXT[]
              OR NOT ('academics:teach' = ANY(permissions))
          )
    ),
    'existing Teacher roles must not administer users, teachers, grades, or academic policy'
);

INSERT INTO tenants (id, slug, name)
VALUES (
    '10800000-0000-0000-0000-000000000001',
    'teacher-boundary-108',
    'Teacher boundary 108'
);

SELECT pg_temp.assert_true(
    EXISTS (
        SELECT 1
        FROM roles
        WHERE tenant_id = '10800000-0000-0000-0000-000000000001'
          AND key = 'teacher'
          AND deleted_at IS NULL
          AND permissions @> ARRAY['academics:view', 'academics:teach']::TEXT[]
          AND NOT (
              permissions && ARRAY[
                  'academics:create',
                  'academics:edit',
                  'academics:delete',
                  'academics:manage',
                  'users:create',
                  'roles:create'
              ]::TEXT[]
          )
    ),
    'future tenants must receive the safe Teacher baseline'
);

UPDATE roles
SET permissions = permissions || ARRAY[
        'users:create',
        'roles:create',
        'academics:create',
        'academics:manage'
    ]::TEXT[]
WHERE tenant_id = '10800000-0000-0000-0000-000000000001'
  AND key = 'teacher';

SELECT pg_temp.assert_true(
    EXISTS (
        SELECT 1
        FROM roles
        WHERE tenant_id = '10800000-0000-0000-0000-000000000001'
          AND key = 'teacher'
          AND NOT (
              permissions && ARRAY[
                  'users:create',
                  'roles:create',
                  'academics:create',
                  'academics:manage'
              ]::TEXT[]
          )
    ),
    'direct writes must not widen the seeded Teacher role into administration'
);

ROLLBACK;

SELECT 'Seeded Teacher access contract passed' AS result;
