-- Add tenant-bound role record-scope evidence without enabling Agent operations.

CREATE UNIQUE INDEX IF NOT EXISTS idx_roles_tenant_id_id
    ON roles(tenant_id, id);

CREATE TABLE IF NOT EXISTS role_record_scope_grants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    role_id UUID NOT NULL,
    scope_family TEXT NOT NULL CHECK (
        LENGTH(scope_family) BETWEEN 3 AND 128
        AND scope_family ~ '^[a-z][a-z0-9_]*([.][a-z][a-z0-9_]*)+$'
    ),
    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('self', 'assigned', 'campus')),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT fk_role_record_scope_grants_tenant_role
        FOREIGN KEY (tenant_id, role_id)
        REFERENCES roles(tenant_id, id)
        ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_role_record_scope_grants_active
    ON role_record_scope_grants(tenant_id, role_id, scope_family, scope_kind)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_role_record_scope_grants_role
    ON role_record_scope_grants(tenant_id, role_id)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_role_record_scope_grants_family
    ON role_record_scope_grants(tenant_id, scope_family, scope_kind)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_role_record_scope_grants_updated_at
    ON role_record_scope_grants;
CREATE TRIGGER update_role_record_scope_grants_updated_at
    BEFORE UPDATE ON role_record_scope_grants
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

-- These rows describe durable role intent only. The Agent policy catalogue and
-- visibility-constrained domain queries remain independent mandatory gates.
INSERT INTO role_record_scope_grants (tenant_id, role_id, scope_family, scope_kind)
SELECT role.tenant_id, role.id, seed.scope_family, seed.scope_kind
FROM roles AS role
INNER JOIN (
    VALUES
        ('registrar', 'sis.account_linking', 'campus'),
        ('registrar', 'sis.imports', 'campus'),
        ('registrar', 'sis.learners', 'campus'),
        ('registrar', 'sis.guardians', 'campus'),
        ('registrar', 'sis.guardian_relationships', 'campus'),
        ('registrar', 'sis.applications', 'campus'),
        ('registrar', 'sis.enrolments', 'campus'),
        ('finance_officer', 'fees.billing', 'campus'),
        ('finance_officer', 'fees.learner_candidates', 'campus'),
        ('finance_officer', 'fees.imports', 'campus'),
        ('finance_officer', 'procurement.requester_candidates', 'campus'),
        ('finance_officer', 'procurement.requests', 'campus'),
        ('teacher', 'academics.teachers', 'self'),
        ('teacher', 'academics.teaching_assignments', 'assigned'),
        ('teacher', 'academics.assessment_components', 'assigned'),
        ('teacher', 'sis.learners', 'assigned'),
        ('teacher', 'sis.guardians', 'assigned'),
        ('teacher', 'sis.guardian_relationships', 'assigned'),
        ('teacher', 'sis.enrolments', 'assigned'),
        ('student', 'fees.billing', 'self'),
        ('staff_member', 'hr.employees', 'self'),
        ('staff_member', 'hr.engagements', 'self'),
        ('staff_member', 'hr.availability', 'self')
) AS seed(role_key, scope_family, scope_kind)
    ON role.key = seed.role_key
   AND role.deleted_at IS NULL
ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
    WHERE deleted_at IS NULL
    DO NOTHING;

-- Future tenant provisioning inserts the seeded roles after this migration has
-- run. Attach defaults to those role rows without granting anything to custom
-- roles, whose scope assignments remain an explicit Administration decision.
CREATE OR REPLACE FUNCTION provision_seed_role_record_scopes()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO role_record_scope_grants (tenant_id, role_id, scope_family, scope_kind)
    SELECT NEW.tenant_id, NEW.id, seed.scope_family, seed.scope_kind
    FROM (
        VALUES
            ('registrar', 'sis.account_linking', 'campus'),
            ('registrar', 'sis.imports', 'campus'),
            ('registrar', 'sis.learners', 'campus'),
            ('registrar', 'sis.guardians', 'campus'),
            ('registrar', 'sis.guardian_relationships', 'campus'),
            ('registrar', 'sis.applications', 'campus'),
            ('registrar', 'sis.enrolments', 'campus'),
            ('finance_officer', 'fees.billing', 'campus'),
            ('finance_officer', 'fees.learner_candidates', 'campus'),
            ('finance_officer', 'fees.imports', 'campus'),
            ('finance_officer', 'procurement.requester_candidates', 'campus'),
            ('finance_officer', 'procurement.requests', 'campus'),
            ('teacher', 'academics.teachers', 'self'),
            ('teacher', 'academics.teaching_assignments', 'assigned'),
            ('teacher', 'academics.assessment_components', 'assigned'),
            ('teacher', 'sis.learners', 'assigned'),
            ('teacher', 'sis.guardians', 'assigned'),
            ('teacher', 'sis.guardian_relationships', 'assigned'),
            ('teacher', 'sis.enrolments', 'assigned'),
            ('student', 'fees.billing', 'self'),
            ('staff_member', 'hr.employees', 'self'),
            ('staff_member', 'hr.engagements', 'self'),
            ('staff_member', 'hr.availability', 'self')
    ) AS seed(role_key, scope_family, scope_kind)
    WHERE seed.role_key = NEW.key
    ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
        WHERE deleted_at IS NULL
        DO NOTHING;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS provision_seed_role_record_scopes_after_insert ON roles;
CREATE TRIGGER provision_seed_role_record_scopes_after_insert
    AFTER INSERT ON roles
    FOR EACH ROW EXECUTE FUNCTION provision_seed_role_record_scopes();
