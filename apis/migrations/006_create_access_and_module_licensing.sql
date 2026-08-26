-- Campus Pilot access model: stable role keys, editable role labels, and tenant module entitlements.

ALTER TABLE roles
ADD COLUMN IF NOT EXISTS key TEXT;

UPDATE roles
SET key = CASE
    WHEN LOWER(name) = 'super admin' THEN 'campus_owner'
    WHEN LOWER(name) = 'admin' THEN 'school_administrator'
    WHEN LOWER(name) = 'faculty' THEN 'teacher'
    WHEN LOWER(name) = 'student' THEN 'student'
    ELSE LOWER(REGEXP_REPLACE(REGEXP_REPLACE(name, '[^a-zA-Z0-9]+', '_', 'g'), '(^_+|_+$)', '', 'g'))
        || '_' || LEFT(id::TEXT, 8)
END
WHERE key IS NULL;

ALTER TABLE roles
ALTER COLUMN key SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_roles_tenant_key
ON roles (tenant_id, key)
WHERE deleted_at IS NULL;

UPDATE roles
SET name = 'Campus Owner',
    description = 'Owns campus configuration and has full access to every enabled campus module.',
    permissions = ARRAY['*']::TEXT[],
    updated_at = NOW()
WHERE key = 'campus_owner';

UPDATE roles
SET name = 'School Administrator',
    description = 'Manages users, access, licensing, and school configuration.',
    permissions = ARRAY[
        'administration:view', 'users:view', 'users:create', 'users:edit',
        'roles:view', 'roles:create', 'roles:edit', 'licensing:view', 'licensing:edit', 'licensing:delete',
        'school_settings:view', 'school_settings:edit'
    ]::TEXT[],
    updated_at = NOW()
WHERE key = 'school_administrator';

UPDATE roles
SET name = 'Teacher',
    description = 'Works with assigned learners, teaching, timetables, library resources, and communication.',
    permissions = ARRAY[
        'academics:view', 'academics:edit', 'sis:view', 'timetabling:view',
        'messaging:view', 'messaging:create', 'library:view'
    ]::TEXT[],
    updated_at = NOW()
WHERE key = 'teacher';

UPDATE roles
SET name = 'Student',
    description = 'Uses learner self-service for learning, timetables, fees, library, and communication.',
    permissions = ARRAY[
        'academics:view', 'timetabling:view', 'fees:view', 'library:view', 'messaging:view'
    ]::TEXT[],
    updated_at = NOW()
WHERE key = 'student';

UPDATE users AS u
SET roles = COALESCE((
    SELECT ARRAY_AGG(r.key ORDER BY r.key)
    FROM roles AS r
    WHERE r.tenant_id = u.tenant_id
      AND r.deleted_at IS NULL
      AND (
          r.name = ANY(u.roles)
          OR r.key = ANY(u.roles)
          OR (r.key = 'campus_owner' AND 'Super Admin' = ANY(u.roles))
          OR (r.key = 'school_administrator' AND 'Admin' = ANY(u.roles))
          OR (r.key = 'teacher' AND 'Faculty' = ANY(u.roles))
      )
), ARRAY[]::TEXT[]);

INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
SELECT
    t.id,
    seed.key,
    seed.name,
    seed.description,
    seed.permissions,
    TRUE
FROM tenants AS t
CROSS JOIN (
    VALUES
        ('campus_owner', 'Campus Owner', 'Owns campus configuration and has full access to every enabled campus module.', ARRAY['*']::TEXT[]),
        ('school_administrator', 'School Administrator', 'Manages users, access, licensing, and school configuration.', ARRAY['administration:view', 'users:view', 'users:create', 'users:edit', 'roles:view', 'roles:create', 'roles:edit', 'licensing:view', 'licensing:edit', 'licensing:delete', 'school_settings:view', 'school_settings:edit']::TEXT[]),
        ('teacher', 'Teacher', 'Works with assigned learners, teaching, timetables, library resources, and communication.', ARRAY['academics:view', 'academics:edit', 'sis:view', 'timetabling:view', 'messaging:view', 'messaging:create', 'library:view']::TEXT[]),
        ('student', 'Student', 'Uses learner self-service for learning, timetables, fees, library, and communication.', ARRAY['academics:view', 'timetabling:view', 'fees:view', 'library:view', 'messaging:view']::TEXT[]),
        ('registrar', 'Registrar', 'Manages admissions, enrolment, and learner records.', ARRAY['sis:view', 'sis:create', 'sis:edit', 'academics:view', 'timetabling:view', 'messaging:view']::TEXT[]),
        ('finance_officer', 'Finance Officer', 'Manages finance, billing, procurement, and finance reporting.', ARRAY['finance:view', 'finance:create', 'finance:edit', 'fees:view', 'fees:create', 'fees:edit', 'procurement:view', 'procurement:create', 'procurement:edit']::TEXT[]),
        ('librarian', 'Librarian', 'Manages the library catalogue, circulation, and learner lookup.', ARRAY['library:view', 'library:create', 'library:edit', 'library:delete', 'sis:view']::TEXT[]),
        ('staff_member', 'Staff Member', 'Uses employee self-service, timetables, communication, and library resources.', ARRAY['hr_payroll:view', 'timetabling:view', 'messaging:view', 'library:view']::TEXT[])
) AS seed(key, name, description, permissions)
WHERE NOT EXISTS (
    SELECT 1
    FROM roles AS existing
    WHERE existing.tenant_id = t.id
      AND existing.key = seed.key
      AND existing.deleted_at IS NULL
);

CREATE TABLE IF NOT EXISTS tenant_modules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    module_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'enabled' CHECK (status IN ('enabled', 'disabled', 'expired', 'revoked')),
    source TEXT NOT NULL DEFAULT 'license' CHECK (source IN ('core', 'legacy', 'license')),
    license_fingerprint TEXT,
    license_expires_at TIMESTAMPTZ,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_tenant_modules_active_key
ON tenant_modules (tenant_id, module_key)
WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_tenant_modules_tenant_id
ON tenant_modules (tenant_id);

CREATE INDEX IF NOT EXISTS idx_tenant_modules_status
ON tenant_modules (status)
WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS module_license_activations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    fingerprint TEXT NOT NULL,
    issuer TEXT NOT NULL,
    entitlement_id TEXT,
    module_keys TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    expires_at TIMESTAMPTZ,
    claims JSONB NOT NULL DEFAULT '{}'::JSONB,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_module_license_activations_fingerprint
ON module_license_activations (tenant_id, fingerprint)
WHERE deleted_at IS NULL;

INSERT INTO tenant_modules (tenant_id, module_key, status, source)
SELECT t.id, core.module_key, 'enabled', 'core'
FROM tenants AS t
CROSS JOIN (VALUES ('home'), ('administration')) AS core(module_key)
WHERE NOT EXISTS (
    SELECT 1
    FROM tenant_modules AS existing
    WHERE existing.tenant_id = t.id
      AND existing.module_key = core.module_key
      AND existing.deleted_at IS NULL
);

-- Preserve the modules already present in existing installations. New campuses
-- are provisioned with core modules only and receive licensed modules by key.
INSERT INTO tenant_modules (tenant_id, module_key, status, source)
SELECT t.id, available.module_key, 'enabled', 'legacy'
FROM tenants AS t
CROSS JOIN (
    VALUES
        ('sis'), ('academics'), ('timetabling'), ('messaging'), ('finance'),
        ('fees'), ('library'), ('hr_payroll'), ('procurement'), ('fleet'),
        ('hostel'), ('health'), ('assets_inventory'), ('document_registry'),
        ('internal_audit')
) AS available(module_key)
WHERE NOT EXISTS (
    SELECT 1
    FROM tenant_modules AS existing
    WHERE existing.tenant_id = t.id
      AND existing.module_key = available.module_key
      AND existing.deleted_at IS NULL
);

DROP TRIGGER IF EXISTS update_tenant_modules_updated_at ON tenant_modules;
CREATE TRIGGER update_tenant_modules_updated_at
    BEFORE UPDATE ON tenant_modules
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

DROP TRIGGER IF EXISTS update_module_license_activations_updated_at ON module_license_activations;
CREATE TRIGGER update_module_license_activations_updated_at
    BEFORE UPDATE ON module_license_activations
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

DROP TRIGGER IF EXISTS ev_tenant_modules ON tenant_modules;
CREATE TRIGGER ev_tenant_modules
    AFTER INSERT OR UPDATE OR DELETE ON tenant_modules
    FOR EACH ROW
    EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_module_license_activations ON module_license_activations;
CREATE TRIGGER ev_module_license_activations
    AFTER INSERT OR UPDATE OR DELETE ON module_license_activations
    FOR EACH ROW
    EXECUTE FUNCTION log_event();
