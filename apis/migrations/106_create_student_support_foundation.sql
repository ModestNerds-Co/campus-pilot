-- Restricted learner-support cases, case teams, actions, and lifecycle evidence.
--
-- SIS remains authoritative for learner identity. Student Support stores only
-- stable learner and account identifiers, and every case always has one lead.

CREATE TABLE IF NOT EXISTS student_support_numbering_policies (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    case_prefix TEXT NOT NULL DEFAULT 'SSC-' CHECK (CHAR_LENGTH(BTRIM(case_prefix)) BETWEEN 1 AND 16),
    padding SMALLINT NOT NULL DEFAULT 6 CHECK (padding BETWEEN 3 AND 12),
    next_case_sequence BIGINT NOT NULL DEFAULT 1 CHECK (next_case_sequence > 0),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

INSERT INTO student_support_numbering_policies (tenant_id)
SELECT tenant.id FROM tenants AS tenant
ON CONFLICT (tenant_id) DO NOTHING;

CREATE OR REPLACE FUNCTION provision_student_support_numbering_policy()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO student_support_numbering_policies (tenant_id)
    VALUES (NEW.id)
    ON CONFLICT (tenant_id) DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_provision_student_support_numbering_policy ON tenants;
CREATE TRIGGER zz_provision_student_support_numbering_policy
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION provision_student_support_numbering_policy();

DROP TRIGGER IF EXISTS update_student_support_numbering_policies_updated_at
    ON student_support_numbering_policies;
CREATE TRIGGER update_student_support_numbering_policies_updated_at
    BEFORE UPDATE ON student_support_numbering_policies
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS student_support_cases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    reference TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(reference)) BETWEEN 1 AND 40),
    learner_id UUID NOT NULL,
    lead_case_worker_user_id UUID NOT NULL,
    category TEXT NOT NULL CHECK (
        category IN ('wellbeing', 'behaviour', 'conduct', 'safeguarding', 'family', 'learning_support', 'other')
    ),
    severity TEXT NOT NULL CHECK (severity IN ('low', 'moderate', 'high', 'critical')),
    title TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(title)) BETWEEN 1 AND 200),
    summary TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(summary)) BETWEEN 1 AND 6000),
    occurred_on DATE,
    status TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open', 'active', 'escalated', 'resolved', 'closed')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    escalated_by UUID,
    escalated_at TIMESTAMPTZ,
    escalation_reason TEXT CHECK (
        escalation_reason IS NULL OR CHAR_LENGTH(BTRIM(escalation_reason)) BETWEEN 1 AND 4000
    ),
    resolved_by UUID,
    resolved_at TIMESTAMPTZ,
    resolution_summary TEXT CHECK (
        resolution_summary IS NULL OR CHAR_LENGTH(BTRIM(resolution_summary)) BETWEEN 1 AND 6000
    ),
    closed_by UUID,
    closed_at TIMESTAMPTZ,
    closure_reason TEXT CHECK (
        closure_reason IS NULL OR CHAR_LENGTH(BTRIM(closure_reason)) BETWEEN 1 AND 4000
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, reference),
    FOREIGN KEY (learner_id, tenant_id) REFERENCES learners(id, tenant_id),
    FOREIGN KEY (lead_case_worker_user_id, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (escalated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (resolved_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (closed_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status IN ('open', 'active')
            AND escalated_by IS NULL AND escalated_at IS NULL AND escalation_reason IS NULL
            AND resolved_by IS NULL AND resolved_at IS NULL AND resolution_summary IS NULL
            AND closed_by IS NULL AND closed_at IS NULL AND closure_reason IS NULL)
        OR (status = 'escalated'
            AND escalated_by IS NOT NULL AND escalated_at IS NOT NULL AND escalation_reason IS NOT NULL
            AND resolved_by IS NULL AND resolved_at IS NULL AND resolution_summary IS NULL
            AND closed_by IS NULL AND closed_at IS NULL AND closure_reason IS NULL)
        OR (status = 'resolved'
            AND resolved_by IS NOT NULL AND resolved_at IS NOT NULL AND resolution_summary IS NOT NULL
            AND closed_by IS NULL AND closed_at IS NULL AND closure_reason IS NULL)
        OR (status = 'closed'
            AND resolved_by IS NOT NULL AND resolved_at IS NOT NULL AND resolution_summary IS NOT NULL
            AND closed_by IS NOT NULL AND closed_at IS NOT NULL AND closure_reason IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_student_support_cases_worklist
    ON student_support_cases(tenant_id, status, severity, updated_at DESC)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_student_support_cases_learner
    ON student_support_cases(tenant_id, learner_id, updated_at DESC)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_student_support_cases_lead
    ON student_support_cases(tenant_id, lead_case_worker_user_id, status, updated_at DESC)
    WHERE deleted_at IS NULL;
DROP TRIGGER IF EXISTS update_student_support_cases_updated_at ON student_support_cases;
CREATE TRIGGER update_student_support_cases_updated_at
    BEFORE UPDATE ON student_support_cases
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS student_support_case_team_members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    case_id UUID NOT NULL,
    user_id UUID NOT NULL,
    member_role TEXT NOT NULL CHECK (member_role IN ('member', 'reviewer')),
    assigned_by UUID NOT NULL,
    removed_by UUID,
    removed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    FOREIGN KEY (case_id, tenant_id) REFERENCES student_support_cases(id, tenant_id),
    FOREIGN KEY (user_id, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (assigned_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (removed_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (removed_by IS NULL AND removed_at IS NULL AND deleted_at IS NULL)
        OR (removed_by IS NOT NULL AND removed_at IS NOT NULL AND deleted_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_student_support_case_team_active
    ON student_support_case_team_members(tenant_id, case_id, user_id)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_student_support_case_team_user
    ON student_support_case_team_members(tenant_id, user_id, case_id)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS student_support_case_actions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    case_id UUID NOT NULL,
    action_kind TEXT NOT NULL CHECK (
        action_kind IN ('note', 'contact', 'meeting', 'referral', 'support_plan', 'review')
    ),
    summary TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(summary)) BETWEEN 1 AND 300),
    details TEXT CHECK (details IS NULL OR CHAR_LENGTH(BTRIM(details)) <= 6000),
    occurred_at TIMESTAMPTZ NOT NULL,
    created_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    FOREIGN KEY (case_id, tenant_id) REFERENCES student_support_cases(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (updated_at = created_at AND deleted_at IS NULL)
);

CREATE INDEX IF NOT EXISTS idx_student_support_case_actions_history
    ON student_support_case_actions(tenant_id, case_id, occurred_at DESC, id DESC);

CREATE TABLE IF NOT EXISTS student_support_case_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    case_id UUID NOT NULL,
    event_type TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(event_type)) BETWEEN 3 AND 100),
    actor_id UUID NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    FOREIGN KEY (case_id, tenant_id) REFERENCES student_support_cases(id, tenant_id),
    FOREIGN KEY (actor_id, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (updated_at = created_at AND deleted_at IS NULL)
);

CREATE INDEX IF NOT EXISTS idx_student_support_case_events_history
    ON student_support_case_events(tenant_id, case_id, created_at DESC, id DESC);

CREATE OR REPLACE FUNCTION reject_student_support_action_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Student Support actions are append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS student_support_actions_append_only ON student_support_case_actions;
CREATE TRIGGER student_support_actions_append_only
    BEFORE UPDATE OR DELETE ON student_support_case_actions
    FOR EACH ROW EXECUTE FUNCTION reject_student_support_action_mutation();

CREATE OR REPLACE FUNCTION reject_student_support_event_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Student Support lifecycle evidence is append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS student_support_events_append_only ON student_support_case_events;
CREATE TRIGGER student_support_events_append_only
    BEFORE UPDATE OR DELETE ON student_support_case_events
    FOR EACH ROW EXECUTE FUNCTION reject_student_support_event_mutation();

INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
SELECT tenant.id, seed.key, seed.name, seed.description, seed.permissions, TRUE
FROM tenants AS tenant
CROSS JOIN (
    VALUES
        (
            'student_support_case_worker',
            'Student Support Case Worker',
            'Works only on learner-support cases where they are an active case-team member.',
            ARRAY['student_support:view', 'student_support:create', 'student_support:edit']::TEXT[]
        ),
        (
            'student_support_manager',
            'Student Support Manager',
            'Oversees restricted learner-support cases, case teams, escalation, resolution, and closure.',
            ARRAY['student_support:view', 'student_support:create', 'student_support:edit', 'student_support:manage']::TEXT[]
        )
) AS seed(key, name, description, permissions)
WHERE NOT EXISTS (
    SELECT 1 FROM roles AS role
    WHERE role.tenant_id = tenant.id AND role.key = seed.key AND role.deleted_at IS NULL
);

CREATE OR REPLACE FUNCTION provision_new_tenant_student_support_roles()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
    VALUES
        (
            NEW.id, 'student_support_case_worker', 'Student Support Case Worker',
            'Works only on learner-support cases where they are an active case-team member.',
            ARRAY['student_support:view', 'student_support:create', 'student_support:edit']::TEXT[], TRUE
        ),
        (
            NEW.id, 'student_support_manager', 'Student Support Manager',
            'Oversees restricted learner-support cases, case teams, escalation, resolution, and closure.',
            ARRAY['student_support:view', 'student_support:create', 'student_support:edit', 'student_support:manage']::TEXT[], TRUE
        );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_provision_new_tenant_student_support_roles ON tenants;
CREATE TRIGGER zz_provision_new_tenant_student_support_roles
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION provision_new_tenant_student_support_roles();

INSERT INTO role_record_scope_grants (tenant_id, role_id, scope_family, scope_kind)
SELECT role.tenant_id, role.id, 'student_support.cases',
       CASE WHEN role.key = 'student_support_manager' THEN 'campus' ELSE 'assigned' END
FROM roles AS role
WHERE role.key IN ('student_support_case_worker', 'student_support_manager')
  AND role.deleted_at IS NULL
ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
    WHERE deleted_at IS NULL DO NOTHING;

CREATE OR REPLACE FUNCTION provision_student_support_role_scopes()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.key IN ('student_support_case_worker', 'student_support_manager') THEN
        INSERT INTO role_record_scope_grants (
            tenant_id, role_id, scope_family, scope_kind
        ) VALUES (
            NEW.tenant_id, NEW.id, 'student_support.cases',
            CASE WHEN NEW.key = 'student_support_manager' THEN 'campus' ELSE 'assigned' END
        )
        ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
            WHERE deleted_at IS NULL DO NOTHING;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS provision_student_support_role_scopes_after_insert ON roles;
CREATE TRIGGER provision_student_support_role_scopes_after_insert
    AFTER INSERT ON roles
    FOR EACH ROW EXECUTE FUNCTION provision_student_support_role_scopes();

DROP TRIGGER IF EXISTS ev_student_support_cases ON student_support_cases;
CREATE TRIGGER ev_student_support_cases
    AFTER INSERT OR UPDATE OR DELETE ON student_support_cases
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_student_support_case_team_members ON student_support_case_team_members;
CREATE TRIGGER ev_student_support_case_team_members
    AFTER INSERT OR UPDATE OR DELETE ON student_support_case_team_members
    FOR EACH ROW EXECUTE FUNCTION log_event();

