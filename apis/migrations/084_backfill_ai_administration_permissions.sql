-- Backfills AI provider and routing administration for existing seeded School
-- Administrators. Agent workspace permissions and module entitlements remain unchanged.

WITH school_administrator_permissions AS (
    SELECT
        role.id,
        ARRAY(
            SELECT DISTINCT permission
            FROM UNNEST(
                role.permissions
                || ARRAY[
                    'ai_providers:view',
                    'ai_providers:edit',
                    'ai_routing:view',
                    'ai_routing:edit'
                ]::TEXT[]
            ) AS granted(permission)
            ORDER BY permission
        ) AS desired_permissions
    FROM roles AS role
    WHERE role.key = 'school_administrator'
      AND role.is_system = TRUE
      AND role.deleted_at IS NULL
)
UPDATE roles AS role
SET permissions = backfill.desired_permissions,
    updated_at = NOW()
FROM school_administrator_permissions AS backfill
WHERE role.id = backfill.id
  AND role.permissions IS DISTINCT FROM backfill.desired_permissions;
