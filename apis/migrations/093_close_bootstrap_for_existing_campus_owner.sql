-- Close bootstrap for installations that already have an active Campus Owner.
-- Ready is monotonic: a stale flag must never reopen unauthenticated school or owner setup.

UPDATE system_state AS state
SET state = 'Ready',
    kernel_lock = FALSE,
    updated_at = NOW()
WHERE state.id = 'singleton'
  AND state.deleted_at IS NULL
  AND state.state <> 'Ready'
  AND EXISTS (
      SELECT 1
      FROM tenants AS tenant
      JOIN users AS campus_owner ON campus_owner.tenant_id = tenant.id
      WHERE tenant.slug = 'default'
        AND tenant.deleted_at IS NULL
        AND campus_owner.deleted_at IS NULL
        AND campus_owner.is_active = TRUE
        AND 'campus_owner' = ANY(campus_owner.roles)
  );

CREATE OR REPLACE FUNCTION protect_bootstrap_ready_state()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.state = 'Ready' AND NEW.state <> 'Ready' THEN
        RAISE EXCEPTION 'Campus Pilot bootstrap state cannot leave Ready';
    END IF;

    IF NEW.state <> 'Ready' AND EXISTS (
        SELECT 1
        FROM tenants AS tenant
        JOIN users AS campus_owner ON campus_owner.tenant_id = tenant.id
        WHERE tenant.slug = 'default'
          AND tenant.deleted_at IS NULL
          AND campus_owner.deleted_at IS NULL
          AND campus_owner.is_active = TRUE
          AND 'campus_owner' = ANY(campus_owner.roles)
    ) THEN
        RAISE EXCEPTION 'Campus Pilot bootstrap is complete because an active Campus Owner exists';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS system_state_protect_ready ON system_state;
CREATE TRIGGER system_state_protect_ready
BEFORE UPDATE OF state ON system_state
FOR EACH ROW
WHEN (OLD.id = 'singleton')
EXECUTE FUNCTION protect_bootstrap_ready_state();
