-- Separate Procurement approval and receiving authority for Finance Officers.
-- Existing campuses gain the two workflow permissions, while the late-running
-- tenant trigger applies the same extension after the standard role seed for
-- every future campus.

UPDATE roles
SET permissions = ARRAY(
        SELECT DISTINCT value
        FROM UNNEST(
            permissions
            || ARRAY['procurement:approve', 'procurement:receive']::TEXT[]
        ) AS permission(value)
        ORDER BY value
    ),
    updated_at = NOW()
WHERE key = 'finance_officer'
  AND deleted_at IS NULL;

CREATE OR REPLACE FUNCTION grant_new_tenant_procurement_workflow_permissions()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE roles
    SET permissions = ARRAY(
            SELECT DISTINCT value
            FROM UNNEST(
                permissions
                || ARRAY['procurement:approve', 'procurement:receive']::TEXT[]
            ) AS permission(value)
            ORDER BY value
        ),
        updated_at = NOW()
    WHERE tenant_id = NEW.id
      AND key = 'finance_officer'
      AND deleted_at IS NULL;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_grant_new_tenant_procurement_workflow_permissions ON tenants;
CREATE TRIGGER zz_grant_new_tenant_procurement_workflow_permissions
    AFTER INSERT ON tenants
    FOR EACH ROW
    EXECUTE FUNCTION grant_new_tenant_procurement_workflow_permissions();
