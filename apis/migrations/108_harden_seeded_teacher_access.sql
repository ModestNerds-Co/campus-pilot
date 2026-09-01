-- Keep the seeded Teacher role inside assigned teaching work.
--
-- Teacher profiles, user accounts, grade levels, academic structures, grading
-- policy, and publication remain administrative. A campus that needs a head of
-- department or another hybrid responsibility creates and assigns a custom
-- role instead of widening the safe Teacher baseline.

CREATE OR REPLACE FUNCTION teacher_baseline_permissions(current_permissions TEXT[])
RETURNS TEXT[] AS $$
    SELECT ARRAY(
        SELECT DISTINCT permission
        FROM UNNEST(
            ARRAY_APPEND(
                COALESCE(current_permissions, ARRAY[]::TEXT[]),
                'academics:teach'
            )
        ) AS expanded(permission)
        WHERE SPLIT_PART(permission, ':', 1) NOT IN (
                  'administration',
                  'users',
                  'roles',
                  'school_settings',
                  'licensing'
              )
          AND permission NOT IN (
              'academics:create',
              'academics:edit',
              'academics:delete',
              'academics:manage'
          )
        ORDER BY permission
    );
$$ LANGUAGE SQL IMMUTABLE STRICT;

UPDATE roles
SET permissions = teacher_baseline_permissions(permissions),
    updated_at = NOW()
WHERE key = 'teacher'
  AND deleted_at IS NULL
  AND permissions IS DISTINCT FROM teacher_baseline_permissions(permissions);

CREATE OR REPLACE FUNCTION enforce_seeded_teacher_access_boundary()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.key = 'teacher' AND NEW.deleted_at IS NULL THEN
        NEW.permissions := teacher_baseline_permissions(NEW.permissions);
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS enforce_seeded_teacher_access_boundary ON roles;
CREATE TRIGGER enforce_seeded_teacher_access_boundary
    BEFORE INSERT OR UPDATE OF key, permissions, deleted_at ON roles
    FOR EACH ROW EXECUTE FUNCTION enforce_seeded_teacher_access_boundary();
