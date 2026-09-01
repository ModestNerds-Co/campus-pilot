-- Keep support roles inside their owning module instead of granting the SIS workspace.
--
-- Library and Academics consume learner records through typed SIS operations.
-- Librarians and Academic Managers therefore do not need the broad `sis:view`
-- permission, which also exposes admissions, enrolment, guardian, and import routes.

CREATE OR REPLACE FUNCTION support_role_permission_boundary(
    role_key TEXT,
    current_permissions TEXT[]
)
RETURNS TEXT[] AS $$
    SELECT CASE
        WHEN role_key IN ('librarian', 'academic_manager') THEN ARRAY(
            SELECT DISTINCT permission
            FROM UNNEST(COALESCE(current_permissions, ARRAY[]::TEXT[])) AS existing(permission)
            WHERE permission <> 'sis:view'
            ORDER BY permission
        )
        ELSE COALESCE(current_permissions, ARRAY[]::TEXT[])
    END;
$$ LANGUAGE SQL IMMUTABLE;

UPDATE roles
SET permissions = support_role_permission_boundary(key, permissions),
    updated_at = NOW()
WHERE key IN ('librarian', 'academic_manager')
  AND deleted_at IS NULL
  AND permissions IS DISTINCT FROM support_role_permission_boundary(key, permissions);

CREATE OR REPLACE FUNCTION enforce_support_role_permission_boundary()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.deleted_at IS NULL THEN
        NEW.permissions := support_role_permission_boundary(NEW.key, NEW.permissions);
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS enforce_support_role_permission_boundary ON roles;
CREATE TRIGGER enforce_support_role_permission_boundary
    BEFORE INSERT OR UPDATE OF key, permissions, deleted_at ON roles
    FOR EACH ROW EXECUTE FUNCTION enforce_support_role_permission_boundary();
