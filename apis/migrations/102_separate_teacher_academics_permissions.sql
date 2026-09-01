-- Separate assigned-class teaching work from academic structure administration.
--
-- Teachers keep read access and receive one narrow operational permission for
-- assigned Gradebook and report-comment work. Academic structure, staffing,
-- assessment setup, deletion, and result publication remain with academic
-- management roles through the existing create/edit/delete/manage permissions.

UPDATE roles
SET permissions = ARRAY(
        SELECT DISTINCT permission
        FROM UNNEST(
            ARRAY_APPEND(
                ARRAY_REMOVE(permissions, 'academics:edit'),
                'academics:teach'
            )
        ) AS expanded(permission)
        ORDER BY permission
    ),
    updated_at = NOW()
WHERE key = 'teacher'
  AND deleted_at IS NULL;

CREATE OR REPLACE FUNCTION harden_new_tenant_teacher_academics_permissions()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE roles
    SET permissions = ARRAY(
            SELECT DISTINCT permission
            FROM UNNEST(
                ARRAY_APPEND(
                    ARRAY_REMOVE(permissions, 'academics:edit'),
                    'academics:teach'
                )
            ) AS expanded(permission)
            ORDER BY permission
        ),
        updated_at = NOW()
    WHERE tenant_id = NEW.id
      AND key = 'teacher'
      AND deleted_at IS NULL;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zzzz_harden_new_tenant_teacher_academics ON tenants;
CREATE TRIGGER zzzz_harden_new_tenant_teacher_academics
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION harden_new_tenant_teacher_academics_permissions();
