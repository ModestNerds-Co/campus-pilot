# Database migration runbook

## Legacy AI routing upgrade (083 to 090)

Migration 083 makes route targets immutable. Migration 087 adds an approval pin
to every target, so a database that has not applied 087 **and already contains
`ai_task_routes` rows** needs a maintenance window. Fresh installations and
databases with no route rows can run the migration chain normally.

Before upgrading, stop API and worker writes, take a verified database backup,
and check the state without exposing the database URL:

```sql
SELECT version, success
FROM _sqlx_migrations
WHERE version IN (87, 90)
ORDER BY version;

SELECT COUNT(*) AS existing_route_targets
FROM ai_task_routes;
```

If 087 is absent and the route count is greater than zero, suspend only the 083
route lifecycle trigger while writes remain stopped:

```sql
BEGIN;
LOCK TABLE ai_task_routes IN ACCESS EXCLUSIVE MODE;
DROP TRIGGER IF EXISTS ai_task_routes_protect_lifecycle ON ai_task_routes;
COMMIT;
```

Immediately run the normal SQLx migration command through 090. Migration 090
repairs only null approval pins and recreates the lifecycle trigger; it never
advances an existing pin to a newer approval. If migration fails, do not resume
writes. Recreate the trigger with the existing 083 function before investigating:

```sql
CREATE TRIGGER ai_task_routes_protect_lifecycle
BEFORE UPDATE OR DELETE ON ai_task_routes
FOR EACH ROW EXECUTE FUNCTION protect_ai_task_route_lifecycle();
```

Before restoring traffic, require all three checks to pass:

```sql
SELECT COUNT(*) = 0 AS every_route_is_pinned
FROM ai_task_routes
WHERE provider_data_approval_id IS NULL;

SELECT COUNT(*) = 1 AS lifecycle_guard_is_present
FROM pg_trigger
WHERE tgrelid = 'ai_task_routes'::REGCLASS
  AND tgname = 'ai_task_routes_protect_lifecycle'
  AND NOT tgisinternal;

SELECT COUNT(*) = 2 AS eligibility_migrations_applied
FROM _sqlx_migrations
WHERE version IN (87, 90)
  AND success;
```
