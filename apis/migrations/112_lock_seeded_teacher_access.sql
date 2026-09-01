-- Keep the seeded Teacher role inside assigned teaching work.
--
-- Account administration, teacher-profile creation, grade levels, classes,
-- subjects, assessment setup, grading policy, publication, and progression
-- remain separate administrative responsibilities. Schools use a custom role
-- for a head of department or another deliberately combined responsibility.

CREATE OR REPLACE FUNCTION teacher_baseline_permissions(current_permissions TEXT[])
RETURNS TEXT[] AS $$
    SELECT ARRAY[
        'academics:teach',
        'academics:view',
        'attendance:create',
        'attendance:edit',
        'attendance:submit',
        'attendance:view',
        'facilities:request',
        'facilities:view',
        'learning:teach',
        'learning:view',
        'library:borrow',
        'library:view',
        'messaging:create',
        'messaging:edit',
        'messaging:view',
        'sis:view',
        'timetabling:view'
    ]::TEXT[];
$$ LANGUAGE SQL IMMUTABLE;

UPDATE roles
SET permissions = teacher_baseline_permissions(permissions),
    description = 'Works with assigned classes, marks, attendance, learning, library use, and draft communication. School setup and account management remain administrative.',
    updated_at = NOW()
WHERE key = 'teacher'
  AND deleted_at IS NULL
  AND (
      permissions IS DISTINCT FROM teacher_baseline_permissions(permissions)
      OR description IS DISTINCT FROM 'Works with assigned classes, marks, attendance, learning, library use, and draft communication. School setup and account management remain administrative.'
  );

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
