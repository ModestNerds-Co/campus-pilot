-- Independent audit planning, engagement, evidence, and finding records.
--
-- Internal Audit stores stable references and append-only lifecycle evidence.
-- It may point at governed Document Registry files, but never mutates the
-- source transaction or private document bytes.

CREATE TABLE IF NOT EXISTS internal_audit_numbering_policies (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    plan_prefix TEXT NOT NULL DEFAULT 'APL-' CHECK (CHAR_LENGTH(BTRIM(plan_prefix)) BETWEEN 1 AND 16),
    engagement_prefix TEXT NOT NULL DEFAULT 'AUD-' CHECK (CHAR_LENGTH(BTRIM(engagement_prefix)) BETWEEN 1 AND 16),
    finding_prefix TEXT NOT NULL DEFAULT 'FND-' CHECK (CHAR_LENGTH(BTRIM(finding_prefix)) BETWEEN 1 AND 16),
    padding SMALLINT NOT NULL DEFAULT 6 CHECK (padding BETWEEN 3 AND 12),
    next_plan_sequence BIGINT NOT NULL DEFAULT 1 CHECK (next_plan_sequence > 0),
    next_engagement_sequence BIGINT NOT NULL DEFAULT 1 CHECK (next_engagement_sequence > 0),
    next_finding_sequence BIGINT NOT NULL DEFAULT 1 CHECK (next_finding_sequence > 0),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

INSERT INTO internal_audit_numbering_policies (tenant_id)
SELECT tenant.id FROM tenants AS tenant
ON CONFLICT (tenant_id) DO NOTHING;

CREATE OR REPLACE FUNCTION provision_internal_audit_numbering_policy()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO internal_audit_numbering_policies (tenant_id)
    VALUES (NEW.id)
    ON CONFLICT (tenant_id) DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_provision_internal_audit_numbering_policy ON tenants;
CREATE TRIGGER zz_provision_internal_audit_numbering_policy
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION provision_internal_audit_numbering_policy();

DROP TRIGGER IF EXISTS update_internal_audit_numbering_policies_updated_at
    ON internal_audit_numbering_policies;
CREATE TRIGGER update_internal_audit_numbering_policies_updated_at
    BEFORE UPDATE ON internal_audit_numbering_policies
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS internal_audit_plans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    reference TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(reference)) BETWEEN 1 AND 40),
    title TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(title)) BETWEEN 1 AND 200),
    objective TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(objective)) BETWEEN 1 AND 4000),
    risk_summary TEXT CHECK (risk_summary IS NULL OR CHAR_LENGTH(BTRIM(risk_summary)) <= 4000),
    period_start DATE NOT NULL,
    period_end DATE NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'approved', 'closed')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    approved_by UUID,
    approved_at TIMESTAMPTZ,
    closed_by UUID,
    closed_at TIMESTAMPTZ,
    close_summary TEXT CHECK (close_summary IS NULL OR CHAR_LENGTH(BTRIM(close_summary)) BETWEEN 1 AND 4000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, reference),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (approved_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (closed_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (period_end >= period_start),
    CHECK (
        (status = 'draft' AND approved_by IS NULL AND approved_at IS NULL
            AND closed_by IS NULL AND closed_at IS NULL AND close_summary IS NULL)
        OR (status = 'approved' AND approved_by IS NOT NULL AND approved_at IS NOT NULL
            AND closed_by IS NULL AND closed_at IS NULL AND close_summary IS NULL)
        OR (status = 'closed' AND approved_by IS NOT NULL AND approved_at IS NOT NULL
            AND closed_by IS NOT NULL AND closed_at IS NOT NULL AND close_summary IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_internal_audit_plans_worklist
    ON internal_audit_plans(tenant_id, status, period_start DESC, reference DESC)
    WHERE deleted_at IS NULL;
DROP TRIGGER IF EXISTS update_internal_audit_plans_updated_at ON internal_audit_plans;
CREATE TRIGGER update_internal_audit_plans_updated_at
    BEFORE UPDATE ON internal_audit_plans
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS internal_audit_engagements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    plan_id UUID NOT NULL,
    reference TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(reference)) BETWEEN 1 AND 40),
    title TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(title)) BETWEEN 1 AND 200),
    objective TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(objective)) BETWEEN 1 AND 4000),
    scope_text TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(scope_text)) BETWEEN 1 AND 6000),
    lead_auditor_user_id UUID NOT NULL,
    starts_on DATE NOT NULL,
    due_on DATE NOT NULL,
    status TEXT NOT NULL DEFAULT 'planned'
        CHECK (status IN ('planned', 'fieldwork', 'reporting', 'closed')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    started_by UUID,
    started_at TIMESTAMPTZ,
    reporting_by UUID,
    reporting_at TIMESTAMPTZ,
    closed_by UUID,
    closed_at TIMESTAMPTZ,
    close_summary TEXT CHECK (close_summary IS NULL OR CHAR_LENGTH(BTRIM(close_summary)) BETWEEN 1 AND 4000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, reference),
    FOREIGN KEY (plan_id, tenant_id) REFERENCES internal_audit_plans(id, tenant_id),
    FOREIGN KEY (lead_auditor_user_id, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (started_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (reporting_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (closed_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (due_on >= starts_on),
    CHECK (
        (status = 'planned' AND started_by IS NULL AND started_at IS NULL
            AND reporting_by IS NULL AND reporting_at IS NULL
            AND closed_by IS NULL AND closed_at IS NULL AND close_summary IS NULL)
        OR (status = 'fieldwork' AND started_by IS NOT NULL AND started_at IS NOT NULL
            AND reporting_by IS NULL AND reporting_at IS NULL
            AND closed_by IS NULL AND closed_at IS NULL AND close_summary IS NULL)
        OR (status = 'reporting' AND started_by IS NOT NULL AND started_at IS NOT NULL
            AND reporting_by IS NOT NULL AND reporting_at IS NOT NULL
            AND closed_by IS NULL AND closed_at IS NULL AND close_summary IS NULL)
        OR (status = 'closed' AND started_by IS NOT NULL AND started_at IS NOT NULL
            AND reporting_by IS NOT NULL AND reporting_at IS NOT NULL
            AND closed_by IS NOT NULL AND closed_at IS NOT NULL AND close_summary IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_internal_audit_engagements_worklist
    ON internal_audit_engagements(tenant_id, status, due_on, reference)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_internal_audit_engagements_assignee
    ON internal_audit_engagements(tenant_id, lead_auditor_user_id, status, due_on)
    WHERE deleted_at IS NULL;
DROP TRIGGER IF EXISTS update_internal_audit_engagements_updated_at ON internal_audit_engagements;
CREATE TRIGGER update_internal_audit_engagements_updated_at
    BEFORE UPDATE ON internal_audit_engagements
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS internal_audit_evidence (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    engagement_id UUID NOT NULL,
    document_file_id UUID NOT NULL,
    document_reference_snapshot TEXT NOT NULL,
    document_title_snapshot TEXT NOT NULL,
    document_sensitivity_snapshot TEXT NOT NULL
        CHECK (document_sensitivity_snapshot IN ('general', 'internal', 'confidential', 'restricted')),
    purpose TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(purpose)) BETWEEN 1 AND 2000),
    linked_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, engagement_id, document_file_id),
    FOREIGN KEY (engagement_id, tenant_id) REFERENCES internal_audit_engagements(id, tenant_id),
    FOREIGN KEY (document_file_id, tenant_id) REFERENCES document_registry_files(id, tenant_id),
    FOREIGN KEY (linked_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (updated_at = created_at AND deleted_at IS NULL)
);

CREATE INDEX IF NOT EXISTS idx_internal_audit_evidence_engagement
    ON internal_audit_evidence(tenant_id, engagement_id, created_at DESC);

CREATE TABLE IF NOT EXISTS internal_audit_findings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    engagement_id UUID NOT NULL,
    reference TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(reference)) BETWEEN 1 AND 40),
    title TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(title)) BETWEEN 1 AND 240),
    rating TEXT NOT NULL CHECK (rating IN ('low', 'moderate', 'high', 'critical')),
    criteria TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(criteria)) BETWEEN 1 AND 6000),
    condition TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(condition)) BETWEEN 1 AND 6000),
    risk_effect TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(risk_effect)) BETWEEN 1 AND 6000),
    recommendation TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(recommendation)) BETWEEN 1 AND 6000),
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'issued')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    issued_by UUID,
    issued_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, reference),
    FOREIGN KEY (engagement_id, tenant_id) REFERENCES internal_audit_engagements(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (issued_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'draft' AND issued_by IS NULL AND issued_at IS NULL)
        OR (status = 'issued' AND issued_by IS NOT NULL AND issued_at IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_internal_audit_findings_worklist
    ON internal_audit_findings(tenant_id, status, rating, created_at DESC)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_internal_audit_findings_engagement
    ON internal_audit_findings(tenant_id, engagement_id, status, reference)
    WHERE deleted_at IS NULL;
DROP TRIGGER IF EXISTS update_internal_audit_findings_updated_at ON internal_audit_findings;
CREATE TRIGGER update_internal_audit_findings_updated_at
    BEFORE UPDATE ON internal_audit_findings
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS internal_audit_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    aggregate_type TEXT NOT NULL CHECK (aggregate_type IN ('plan', 'engagement', 'finding', 'evidence')),
    aggregate_id UUID NOT NULL,
    engagement_id UUID,
    event_type TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(event_type)) BETWEEN 3 AND 100),
    actor_id UUID NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    FOREIGN KEY (engagement_id, tenant_id) REFERENCES internal_audit_engagements(id, tenant_id),
    FOREIGN KEY (actor_id, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (updated_at = created_at AND deleted_at IS NULL)
);

CREATE INDEX IF NOT EXISTS idx_internal_audit_event_history
    ON internal_audit_events(tenant_id, engagement_id, created_at DESC, id);

CREATE OR REPLACE FUNCTION reject_internal_audit_evidence_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Internal Audit evidence links are append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS internal_audit_evidence_append_only ON internal_audit_evidence;
CREATE TRIGGER internal_audit_evidence_append_only
    BEFORE UPDATE OR DELETE ON internal_audit_evidence
    FOR EACH ROW EXECUTE FUNCTION reject_internal_audit_evidence_mutation();

CREATE OR REPLACE FUNCTION reject_internal_audit_event_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Internal Audit events are append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS internal_audit_events_append_only ON internal_audit_events;
CREATE TRIGGER internal_audit_events_append_only
    BEFORE UPDATE OR DELETE ON internal_audit_events
    FOR EACH ROW EXECUTE FUNCTION reject_internal_audit_event_mutation();

INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
SELECT tenant.id, seed.key, seed.name, seed.description, seed.permissions, TRUE
FROM tenants AS tenant
CROSS JOIN (
    VALUES
        (
            'internal_auditor',
            'Internal Auditor',
            'Performs assigned audit engagements and drafts findings from governed evidence.',
            ARRAY[
                'internal_audit:view', 'internal_audit:create', 'internal_audit:edit',
                'document_registry:view'
            ]::TEXT[]
        ),
        (
            'audit_manager',
            'Audit Manager',
            'Approves audit plans, oversees engagements, and issues reviewed findings.',
            ARRAY[
                'internal_audit:view', 'internal_audit:create', 'internal_audit:edit',
                'internal_audit:delete', 'internal_audit:issue', 'internal_audit:manage',
                'document_registry:view'
            ]::TEXT[]
        )
) AS seed(key, name, description, permissions)
WHERE NOT EXISTS (
    SELECT 1 FROM roles AS role
    WHERE role.tenant_id = tenant.id AND role.key = seed.key AND role.deleted_at IS NULL
);

CREATE OR REPLACE FUNCTION provision_new_tenant_internal_audit_roles()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
    VALUES
        (
            NEW.id, 'internal_auditor', 'Internal Auditor',
            'Performs assigned audit engagements and drafts findings from governed evidence.',
            ARRAY[
                'internal_audit:view', 'internal_audit:create', 'internal_audit:edit',
                'document_registry:view'
            ]::TEXT[], TRUE
        ),
        (
            NEW.id, 'audit_manager', 'Audit Manager',
            'Approves audit plans, oversees engagements, and issues reviewed findings.',
            ARRAY[
                'internal_audit:view', 'internal_audit:create', 'internal_audit:edit',
                'internal_audit:delete', 'internal_audit:issue', 'internal_audit:manage',
                'document_registry:view'
            ]::TEXT[], TRUE
        );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_provision_new_tenant_internal_audit_roles ON tenants;
CREATE TRIGGER zz_provision_new_tenant_internal_audit_roles
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION provision_new_tenant_internal_audit_roles();

INSERT INTO role_record_scope_grants (tenant_id, role_id, scope_family, scope_kind)
SELECT role.tenant_id, role.id, seed.scope_family, seed.scope_kind
FROM roles AS role
INNER JOIN (
    VALUES
        ('internal_auditor', 'internal_audit.plans', 'campus'),
        ('internal_auditor', 'internal_audit.records', 'assigned'),
        ('internal_auditor', 'document_registry.records', 'campus'),
        ('audit_manager', 'internal_audit.plans', 'campus'),
        ('audit_manager', 'internal_audit.records', 'campus'),
        ('audit_manager', 'document_registry.records', 'campus')
) AS seed(role_key, scope_family, scope_kind)
    ON role.key = seed.role_key AND role.deleted_at IS NULL
ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
    WHERE deleted_at IS NULL DO NOTHING;

-- Complete replacement of the canonical seeded-role scope function.
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
            ('staff_member', 'hr.availability', 'self'),
            ('librarian', 'library.members', 'campus'),
            ('librarian', 'library.borrowing', 'campus'),
            ('student', 'library.members', 'self'),
            ('student', 'library.borrowing', 'self'),
            ('teacher', 'library.members', 'self'),
            ('teacher', 'library.borrowing', 'self'),
            ('staff_member', 'library.members', 'self'),
            ('staff_member', 'library.borrowing', 'self'),
            ('health_officer', 'health.patients', 'campus'),
            ('health_officer', 'health.care', 'campus'),
            ('hostel_officer', 'hostel.occupancy', 'campus'),
            ('hostel_officer', 'hostel.pastoral', 'campus'),
            ('records_officer', 'document_registry.records', 'campus'),
            ('internal_auditor', 'internal_audit.plans', 'campus'),
            ('internal_auditor', 'internal_audit.records', 'assigned'),
            ('internal_auditor', 'document_registry.records', 'campus'),
            ('audit_manager', 'internal_audit.plans', 'campus'),
            ('audit_manager', 'internal_audit.records', 'campus'),
            ('audit_manager', 'document_registry.records', 'campus')
    ) AS seed(role_key, scope_family, scope_kind)
    WHERE seed.role_key = NEW.key
    ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
        WHERE deleted_at IS NULL DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
