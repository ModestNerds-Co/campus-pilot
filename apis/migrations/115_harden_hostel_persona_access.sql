-- Separate boarding self-service from campus Hostel administration.
--
-- Boarding learners may inspect only the allocations resolved through their
-- authenticated SIS account. Pastoral records remain campus-only because they
-- contain sensitive safeguarding and wellbeing information.

INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
SELECT tenant.id,
       'hostel_resident',
       'Boarding learner',
       'Views their own current and previous boarding allocations.',
       ARRAY['hostel:view']::TEXT[],
       TRUE
  FROM tenants AS tenant
 WHERE NOT EXISTS (
    SELECT 1
      FROM roles AS role
     WHERE role.tenant_id = tenant.id
       AND role.key = 'hostel_resident'
       AND role.deleted_at IS NULL
 );

UPDATE roles
   SET name = 'Boarding learner',
       description = 'Views their own current and previous boarding allocations.',
       permissions = ARRAY['hostel:view']::TEXT[],
       is_system = TRUE,
       updated_at = NOW()
 WHERE key = 'hostel_resident'
   AND deleted_at IS NULL
   AND (
       name IS DISTINCT FROM 'Boarding learner'
       OR description IS DISTINCT FROM 'Views their own current and previous boarding allocations.'
       OR permissions IS DISTINCT FROM ARRAY['hostel:view']::TEXT[]
       OR is_system IS DISTINCT FROM TRUE
   );

INSERT INTO role_record_scope_grants (tenant_id, role_id, scope_family, scope_kind)
SELECT role.tenant_id, role.id, 'hostel.occupancy', 'self'
  FROM roles AS role
 WHERE role.key = 'hostel_resident'
   AND role.deleted_at IS NULL
ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
    WHERE deleted_at IS NULL DO NOTHING;

UPDATE role_record_scope_grants AS scope_grant
   SET deleted_at = NOW(), updated_at = NOW()
  FROM roles AS role
 WHERE role.id = scope_grant.role_id
   AND role.tenant_id = scope_grant.tenant_id
   AND role.key = 'hostel_resident'
   AND role.deleted_at IS NULL
   AND scope_grant.deleted_at IS NULL
   AND NOT (
       scope_grant.scope_family = 'hostel.occupancy'
       AND scope_grant.scope_kind = 'self'
   );

-- Close any invalid direct-database grants before adding the invariant.
UPDATE role_record_scope_grants
   SET deleted_at = NOW(), updated_at = NOW()
 WHERE (
       (scope_family = 'hostel.pastoral' AND scope_kind <> 'campus')
       OR (scope_family = 'hostel.occupancy' AND scope_kind NOT IN ('campus', 'self'))
   )
   AND deleted_at IS NULL;

CREATE OR REPLACE FUNCTION enforce_hostel_record_scope_policy()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.deleted_at IS NULL
       AND NEW.scope_family = 'hostel.pastoral'
       AND NEW.scope_kind <> 'campus' THEN
        RAISE EXCEPTION 'Hostel pastoral records require campus scope';
    END IF;
    IF NEW.deleted_at IS NULL
       AND NEW.scope_family = 'hostel.occupancy'
       AND NEW.scope_kind NOT IN ('campus', 'self') THEN
        RAISE EXCEPTION 'Hostel occupancy supports only campus or self scope';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS enforce_hostel_record_scope_policy ON role_record_scope_grants;
CREATE TRIGGER enforce_hostel_record_scope_policy
    BEFORE INSERT OR UPDATE OF scope_family, scope_kind, deleted_at
    ON role_record_scope_grants
    FOR EACH ROW EXECUTE FUNCTION enforce_hostel_record_scope_policy();

CREATE OR REPLACE FUNCTION provision_new_tenant_hostel_resident_role()
RETURNS TRIGGER AS $$
DECLARE
    resident_role_id UUID;
BEGIN
    INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
    VALUES (
        NEW.id,
        'hostel_resident',
        'Boarding learner',
        'Views their own current and previous boarding allocations.',
        ARRAY['hostel:view']::TEXT[],
        TRUE
    )
    RETURNING id INTO resident_role_id;

    INSERT INTO role_record_scope_grants (
        tenant_id, role_id, scope_family, scope_kind
    ) VALUES (
        NEW.id, resident_role_id, 'hostel.occupancy', 'self'
    );

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_provision_new_tenant_hostel_resident_role ON tenants;
CREATE TRIGGER zz_provision_new_tenant_hostel_resident_role
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION provision_new_tenant_hostel_resident_role();
