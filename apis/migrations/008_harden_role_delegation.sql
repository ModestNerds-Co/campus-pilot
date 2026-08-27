-- Harden tenant role provisioning and delegation.

-- Role names are presentation labels, but duplicate labels differing only by
-- case are operationally ambiguous in the same campus.
DROP INDEX IF EXISTS idx_roles_tenant_name;
CREATE UNIQUE INDEX IF NOT EXISTS idx_roles_tenant_name_lower
ON roles (tenant_id, LOWER(name))
WHERE deleted_at IS NULL;

-- Separate role definition from role assignment authority for existing campuses.
UPDATE roles
SET permissions = array_append(permissions, 'roles:assign'),
    updated_at = NOW()
WHERE key = 'school_administrator'
  AND deleted_at IS NULL
  AND NOT ('roles:assign' = ANY(permissions));

-- Every tenant is born with the stable role keys and core module records used
-- by authentication. Human-facing seeded role labels remain editable later.
CREATE OR REPLACE FUNCTION provision_tenant_access()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
    VALUES
        (NEW.id, 'campus_owner', 'Campus Owner', 'Owns campus configuration and has full access to every enabled campus module.', ARRAY['*']::TEXT[], TRUE),
        (NEW.id, 'school_administrator', 'School Administrator', 'Manages users, access, licensing, and school configuration.', ARRAY['administration:view', 'users:view', 'users:create', 'users:edit', 'roles:view', 'roles:create', 'roles:edit', 'roles:assign', 'licensing:view', 'licensing:edit', 'licensing:delete', 'school_settings:view', 'school_settings:edit']::TEXT[], TRUE),
        (NEW.id, 'teacher', 'Teacher', 'Works with assigned learners, teaching, timetables, library resources, and communication.', ARRAY['academics:view', 'academics:edit', 'sis:view', 'timetabling:view', 'messaging:view', 'messaging:create', 'library:view']::TEXT[], TRUE),
        (NEW.id, 'student', 'Student', 'Uses learner self-service for learning, timetables, fees, library, and communication.', ARRAY['academics:view', 'timetabling:view', 'fees:view', 'library:view', 'messaging:view']::TEXT[], TRUE),
        (NEW.id, 'registrar', 'Registrar', 'Manages admissions, enrolment, and learner records.', ARRAY['sis:view', 'sis:create', 'sis:edit', 'academics:view', 'timetabling:view', 'messaging:view']::TEXT[], TRUE),
        (NEW.id, 'finance_officer', 'Finance Officer', 'Manages finance, billing, procurement, and finance reporting.', ARRAY['finance:view', 'finance:create', 'finance:edit', 'fees:view', 'fees:create', 'fees:edit', 'procurement:view', 'procurement:create', 'procurement:edit']::TEXT[], TRUE),
        (NEW.id, 'librarian', 'Librarian', 'Manages the library catalogue, circulation, and learner lookup.', ARRAY['library:view', 'library:create', 'library:edit', 'library:delete', 'sis:view']::TEXT[], TRUE),
        (NEW.id, 'staff_member', 'Staff Member', 'Uses employee self-service, timetables, communication, and library resources.', ARRAY['hr_payroll:view', 'timetabling:view', 'messaging:view', 'library:view']::TEXT[], TRUE);

    INSERT INTO tenant_modules (tenant_id, module_key, status, source)
    VALUES
        (NEW.id, 'home', 'enabled', 'core'),
        (NEW.id, 'administration', 'enabled', 'core');

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS provision_tenant_access_after_insert ON tenants;
CREATE TRIGGER provision_tenant_access_after_insert
    AFTER INSERT ON tenants
    FOR EACH ROW
    EXECUTE FUNCTION provision_tenant_access();
