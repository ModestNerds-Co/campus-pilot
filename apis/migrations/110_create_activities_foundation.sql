-- Activities owns co-curricular groups, learner membership, sessions, and
-- immutable participation evidence. SIS and HR remain the person systems of record.

CREATE TABLE IF NOT EXISTS activity_session_numbering_policies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    prefix TEXT NOT NULL DEFAULT 'ACT-',
    padding INTEGER NOT NULL DEFAULT 6 CHECK (padding BETWEEN 1 AND 12),
    next_sequence BIGINT NOT NULL DEFAULT 1 CHECK (next_sequence > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_activity_session_numbering_policy_active
    ON activity_session_numbering_policies(tenant_id)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS activity_catalog_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    category TEXT NOT NULL CHECK (
        category IN ('sport', 'club', 'arts', 'service', 'society', 'academic_enrichment', 'other')
    ),
    description TEXT,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    archived_at TIMESTAMPTZ,
    archived_by UUID,
    archive_reason TEXT,
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (archived_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'active' AND archived_at IS NULL AND archived_by IS NULL AND archive_reason IS NULL)
        OR
        (status = 'archived' AND archived_at IS NOT NULL AND archived_by IS NOT NULL AND length(btrim(archive_reason)) > 0)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_activity_catalog_code_active
    ON activity_catalog_items(tenant_id, lower(code))
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_activity_catalog_search
    ON activity_catalog_items(tenant_id, status, category, name);

CREATE TABLE IF NOT EXISTS activity_groups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    activity_id UUID NOT NULL,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    starts_on DATE NOT NULL,
    ends_on DATE NOT NULL,
    capacity INTEGER CHECK (capacity IS NULL OR capacity > 0),
    consent_required BOOLEAN NOT NULL DEFAULT FALSE,
    consent_instructions TEXT,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'active', 'closed', 'cancelled')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    activated_at TIMESTAMPTZ,
    activated_by UUID,
    closed_at TIMESTAMPTZ,
    closed_by UUID,
    closure_reason TEXT,
    cancelled_at TIMESTAMPTZ,
    cancelled_by UUID,
    cancellation_reason TEXT,
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    UNIQUE (id, tenant_id, activity_id),
    FOREIGN KEY (activity_id, tenant_id) REFERENCES activity_catalog_items(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (activated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (closed_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (cancelled_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (starts_on <= ends_on),
    CHECK (consent_required OR consent_instructions IS NULL),
    CHECK (
        (status = 'draft' AND activated_at IS NULL AND activated_by IS NULL
            AND closed_at IS NULL AND closed_by IS NULL AND closure_reason IS NULL
            AND cancelled_at IS NULL AND cancelled_by IS NULL AND cancellation_reason IS NULL)
        OR
        (status = 'active' AND activated_at IS NOT NULL AND activated_by IS NOT NULL
            AND closed_at IS NULL AND closed_by IS NULL AND closure_reason IS NULL
            AND cancelled_at IS NULL AND cancelled_by IS NULL AND cancellation_reason IS NULL)
        OR
        (status = 'closed' AND activated_at IS NOT NULL AND activated_by IS NOT NULL
            AND closed_at IS NOT NULL AND closed_by IS NOT NULL AND length(btrim(closure_reason)) > 0
            AND cancelled_at IS NULL AND cancelled_by IS NULL AND cancellation_reason IS NULL)
        OR
        (status = 'cancelled' AND cancelled_at IS NOT NULL AND cancelled_by IS NOT NULL
            AND length(btrim(cancellation_reason)) > 0 AND closed_at IS NULL AND closed_by IS NULL
            AND closure_reason IS NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_activity_groups_code_active
    ON activity_groups(tenant_id, lower(code))
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_activity_groups_worklist
    ON activity_groups(tenant_id, status, starts_on, ends_on);

CREATE TABLE IF NOT EXISTS activity_group_leaders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    group_id UUID NOT NULL,
    employee_id UUID NOT NULL,
    leader_role TEXT NOT NULL DEFAULT 'leader' CHECK (leader_role IN ('lead', 'leader', 'assistant')),
    starts_on DATE NOT NULL,
    ends_on DATE,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    ended_at TIMESTAMPTZ,
    ended_by UUID,
    end_reason TEXT,
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    UNIQUE (id, tenant_id, group_id),
    FOREIGN KEY (group_id, tenant_id) REFERENCES activity_groups(id, tenant_id),
    FOREIGN KEY (employee_id, tenant_id) REFERENCES employees(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (ended_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (ends_on IS NULL OR starts_on <= ends_on),
    CHECK (
        (ended_at IS NULL AND ended_by IS NULL AND end_reason IS NULL)
        OR
        (ended_at IS NOT NULL AND ended_by IS NOT NULL AND length(btrim(end_reason)) > 0)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_activity_group_leaders_active
    ON activity_group_leaders(tenant_id, group_id, employee_id)
    WHERE deleted_at IS NULL AND ended_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_activity_group_leaders_employee
    ON activity_group_leaders(tenant_id, employee_id, group_id)
    WHERE deleted_at IS NULL AND ended_at IS NULL;

CREATE TABLE IF NOT EXISTS activity_group_memberships (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    group_id UUID NOT NULL,
    learner_id UUID NOT NULL,
    joined_on DATE NOT NULL,
    ended_on DATE,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'ended', 'withdrawn')),
    consent_status TEXT NOT NULL DEFAULT 'not_required' CHECK (
        consent_status IN ('not_required', 'pending', 'granted', 'declined')
    ),
    consent_recorded_at TIMESTAMPTZ,
    consent_recorded_by UUID,
    consent_notes TEXT,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    ended_at TIMESTAMPTZ,
    ended_by UUID,
    end_reason TEXT,
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    UNIQUE (id, tenant_id, group_id, learner_id),
    FOREIGN KEY (group_id, tenant_id) REFERENCES activity_groups(id, tenant_id),
    FOREIGN KEY (learner_id, tenant_id) REFERENCES learners(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (consent_recorded_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (ended_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (ended_on IS NULL OR joined_on <= ended_on),
    CHECK (
        (consent_status IN ('not_required', 'pending') AND consent_recorded_at IS NULL AND consent_recorded_by IS NULL)
        OR
        (consent_status IN ('granted', 'declined') AND consent_recorded_at IS NOT NULL AND consent_recorded_by IS NOT NULL)
    ),
    CHECK (
        (status = 'active' AND ended_on IS NULL AND ended_at IS NULL AND ended_by IS NULL AND end_reason IS NULL)
        OR
        (status IN ('ended', 'withdrawn') AND ended_on IS NOT NULL AND ended_at IS NOT NULL
            AND ended_by IS NOT NULL AND length(btrim(end_reason)) > 0)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_activity_memberships_active
    ON activity_group_memberships(tenant_id, group_id, learner_id)
    WHERE deleted_at IS NULL AND status = 'active';
CREATE INDEX IF NOT EXISTS idx_activity_memberships_learner
    ON activity_group_memberships(tenant_id, learner_id, group_id)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS activity_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    group_id UUID NOT NULL,
    reference TEXT NOT NULL,
    title TEXT NOT NULL,
    starts_at TIMESTAMPTZ NOT NULL,
    ends_at TIMESTAMPTZ NOT NULL,
    location_note TEXT,
    notes TEXT,
    status TEXT NOT NULL DEFAULT 'scheduled' CHECK (status IN ('scheduled', 'completed', 'cancelled')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    completed_at TIMESTAMPTZ,
    completed_by UUID,
    completion_summary TEXT,
    cancelled_at TIMESTAMPTZ,
    cancelled_by UUID,
    cancellation_reason TEXT,
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    UNIQUE (id, tenant_id, group_id),
    FOREIGN KEY (group_id, tenant_id) REFERENCES activity_groups(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (completed_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (cancelled_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (starts_at < ends_at),
    CHECK (
        (status = 'scheduled' AND completed_at IS NULL AND completed_by IS NULL AND completion_summary IS NULL
            AND cancelled_at IS NULL AND cancelled_by IS NULL AND cancellation_reason IS NULL)
        OR
        (status = 'completed' AND completed_at IS NOT NULL AND completed_by IS NOT NULL
            AND length(btrim(completion_summary)) > 0 AND cancelled_at IS NULL
            AND cancelled_by IS NULL AND cancellation_reason IS NULL)
        OR
        (status = 'cancelled' AND cancelled_at IS NOT NULL AND cancelled_by IS NOT NULL
            AND length(btrim(cancellation_reason)) > 0 AND completed_at IS NULL
            AND completed_by IS NULL AND completion_summary IS NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_activity_sessions_reference_active
    ON activity_sessions(tenant_id, reference)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_activity_sessions_worklist
    ON activity_sessions(tenant_id, status, starts_at, group_id);

CREATE TABLE IF NOT EXISTS activity_session_participation (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    session_id UUID NOT NULL,
    group_id UUID NOT NULL,
    membership_id UUID NOT NULL,
    learner_id UUID NOT NULL,
    mark TEXT NOT NULL CHECK (mark IN ('present', 'absent', 'late', 'excused', 'not_required')),
    notes TEXT,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    marked_by UUID NOT NULL,
    marked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    FOREIGN KEY (session_id, tenant_id, group_id) REFERENCES activity_sessions(id, tenant_id, group_id),
    FOREIGN KEY (membership_id, tenant_id, group_id, learner_id)
        REFERENCES activity_group_memberships(id, tenant_id, group_id, learner_id),
    FOREIGN KEY (marked_by, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_activity_participation_active
    ON activity_session_participation(tenant_id, session_id, membership_id)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS activity_session_completion_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    session_id UUID NOT NULL,
    group_id UUID NOT NULL,
    roster_count INTEGER NOT NULL CHECK (roster_count > 0),
    roster_fingerprint TEXT NOT NULL,
    summary TEXT NOT NULL,
    completed_by UUID NOT NULL,
    completed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    FOREIGN KEY (session_id, tenant_id, group_id) REFERENCES activity_sessions(id, tenant_id, group_id),
    FOREIGN KEY (completed_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (length(btrim(summary)) > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_activity_completion_snapshot_session
    ON activity_session_completion_snapshots(tenant_id, session_id)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS activity_session_completion_members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    snapshot_id UUID NOT NULL,
    session_id UUID NOT NULL,
    group_id UUID NOT NULL,
    membership_id UUID NOT NULL,
    learner_id UUID NOT NULL,
    learner_number_snapshot TEXT NOT NULL,
    learner_name_snapshot TEXT NOT NULL,
    mark TEXT NOT NULL CHECK (mark IN ('present', 'absent', 'late', 'excused', 'not_required')),
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    FOREIGN KEY (snapshot_id, tenant_id) REFERENCES activity_session_completion_snapshots(id, tenant_id),
    FOREIGN KEY (session_id, tenant_id, group_id) REFERENCES activity_sessions(id, tenant_id, group_id),
    FOREIGN KEY (membership_id, tenant_id, group_id, learner_id)
        REFERENCES activity_group_memberships(id, tenant_id, group_id, learner_id),
    FOREIGN KEY (learner_id, tenant_id) REFERENCES learners(id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_activity_completion_membership
    ON activity_session_completion_members(tenant_id, snapshot_id, membership_id)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS activity_lifecycle_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    group_id UUID,
    session_id UUID,
    event_type TEXT NOT NULL,
    actor_id UUID NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    FOREIGN KEY (group_id, tenant_id) REFERENCES activity_groups(id, tenant_id),
    FOREIGN KEY (session_id, tenant_id) REFERENCES activity_sessions(id, tenant_id),
    FOREIGN KEY (actor_id, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (group_id IS NOT NULL OR session_id IS NOT NULL),
    CHECK (updated_at = created_at AND deleted_at IS NULL)
);

CREATE INDEX IF NOT EXISTS idx_activity_events_group
    ON activity_lifecycle_events(tenant_id, group_id, created_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_activity_events_session
    ON activity_lifecycle_events(tenant_id, session_id, created_at DESC, id DESC);

CREATE OR REPLACE FUNCTION reject_activity_evidence_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Activities completion and lifecycle evidence is append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS activity_completion_snapshots_append_only
    ON activity_session_completion_snapshots;
CREATE TRIGGER activity_completion_snapshots_append_only
    BEFORE UPDATE OR DELETE ON activity_session_completion_snapshots
    FOR EACH ROW EXECUTE FUNCTION reject_activity_evidence_mutation();
DROP TRIGGER IF EXISTS activity_completion_members_append_only
    ON activity_session_completion_members;
CREATE TRIGGER activity_completion_members_append_only
    BEFORE UPDATE OR DELETE ON activity_session_completion_members
    FOR EACH ROW EXECUTE FUNCTION reject_activity_evidence_mutation();
DROP TRIGGER IF EXISTS activity_lifecycle_events_append_only
    ON activity_lifecycle_events;
CREATE TRIGGER activity_lifecycle_events_append_only
    BEFORE UPDATE OR DELETE ON activity_lifecycle_events
    FOR EACH ROW EXECUTE FUNCTION reject_activity_evidence_mutation();

INSERT INTO activity_session_numbering_policies (tenant_id)
SELECT tenant.id
FROM tenants AS tenant
WHERE NOT EXISTS (
    SELECT 1 FROM activity_session_numbering_policies AS policy
    WHERE policy.tenant_id = tenant.id AND policy.deleted_at IS NULL
);

INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
SELECT tenant.id, seed.key, seed.name, seed.description, seed.permissions, TRUE
FROM tenants AS tenant
CROSS JOIN (
    VALUES
        (
            'activity_leader',
            'Activity Leader',
            'Runs sessions and records participation for assigned co-curricular groups.',
            ARRAY['activities:view', 'activities:operate']::TEXT[]
        ),
        (
            'activities_coordinator',
            'Activities Coordinator',
            'Manages the campus activity catalog, groups, leaders, learner rosters, consent, and sessions.',
            ARRAY['activities:view', 'activities:operate', 'activities:manage']::TEXT[]
        )
) AS seed(key, name, description, permissions)
WHERE NOT EXISTS (
    SELECT 1 FROM roles AS role
    WHERE role.tenant_id = tenant.id AND role.key = seed.key AND role.deleted_at IS NULL
);

UPDATE roles
SET permissions = ARRAY(
        SELECT DISTINCT permission
        FROM UNNEST(permissions || ARRAY['activities:view']::TEXT[]) AS expanded(permission)
        ORDER BY permission
    ),
    updated_at = NOW()
WHERE key = 'student' AND deleted_at IS NULL;

INSERT INTO role_record_scope_grants (tenant_id, role_id, scope_family, scope_kind)
SELECT role.tenant_id, role.id, scope.scope_family, scope.scope_kind
FROM roles AS role
CROSS JOIN LATERAL (
    VALUES
        ('activities.groups', CASE
            WHEN role.key = 'activities_coordinator' THEN 'campus'
            WHEN role.key = 'activity_leader' THEN 'assigned'
            WHEN role.key = 'student' THEN 'self'
            ELSE NULL
        END),
        ('activities.sessions', CASE
            WHEN role.key = 'activities_coordinator' THEN 'campus'
            WHEN role.key = 'activity_leader' THEN 'assigned'
            WHEN role.key = 'student' THEN 'self'
            ELSE NULL
        END)
) AS scope(scope_family, scope_kind)
WHERE role.key IN ('activities_coordinator', 'activity_leader', 'student')
  AND role.deleted_at IS NULL AND scope.scope_kind IS NOT NULL
ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
    WHERE deleted_at IS NULL DO NOTHING;

CREATE OR REPLACE FUNCTION provision_activities_role_scopes()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.key IN ('activities_coordinator', 'activity_leader', 'student') THEN
        INSERT INTO role_record_scope_grants (
            tenant_id, role_id, scope_family, scope_kind
        )
        SELECT NEW.tenant_id, NEW.id, family.scope_family,
            CASE
                WHEN NEW.key = 'activities_coordinator' THEN 'campus'
                WHEN NEW.key = 'activity_leader' THEN 'assigned'
                ELSE 'self'
            END
        FROM (VALUES ('activities.groups'), ('activities.sessions')) AS family(scope_family)
        ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
            WHERE deleted_at IS NULL DO NOTHING;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS provision_activities_role_scopes_after_insert ON roles;
CREATE TRIGGER provision_activities_role_scopes_after_insert
    AFTER INSERT ON roles
    FOR EACH ROW EXECUTE FUNCTION provision_activities_role_scopes();

CREATE OR REPLACE FUNCTION provision_new_tenant_activities_access()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO activity_session_numbering_policies (tenant_id) VALUES (NEW.id);

    INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
    VALUES
        (
            NEW.id, 'activity_leader', 'Activity Leader',
            'Runs sessions and records participation for assigned co-curricular groups.',
            ARRAY['activities:view', 'activities:operate']::TEXT[], TRUE
        ),
        (
            NEW.id, 'activities_coordinator', 'Activities Coordinator',
            'Manages the campus activity catalog, groups, leaders, learner rosters, consent, and sessions.',
            ARRAY['activities:view', 'activities:operate', 'activities:manage']::TEXT[], TRUE
        );

    UPDATE roles
    SET permissions = ARRAY(
            SELECT DISTINCT permission
            FROM UNNEST(permissions || ARRAY['activities:view']::TEXT[]) AS expanded(permission)
            ORDER BY permission
        ),
        updated_at = NOW()
    WHERE tenant_id = NEW.id AND key = 'student' AND deleted_at IS NULL;

    INSERT INTO role_record_scope_grants (tenant_id, role_id, scope_family, scope_kind)
    SELECT role.tenant_id, role.id, scope.scope_family, scope.scope_kind
    FROM roles AS role
    CROSS JOIN LATERAL (
        VALUES
            ('activities.groups', CASE
                WHEN role.key = 'activities_coordinator' THEN 'campus'
                WHEN role.key = 'activity_leader' THEN 'assigned'
                WHEN role.key = 'student' THEN 'self'
                ELSE NULL
            END),
            ('activities.sessions', CASE
                WHEN role.key = 'activities_coordinator' THEN 'campus'
                WHEN role.key = 'activity_leader' THEN 'assigned'
                WHEN role.key = 'student' THEN 'self'
                ELSE NULL
            END)
    ) AS scope(scope_family, scope_kind)
    WHERE role.tenant_id = NEW.id
      AND role.key IN ('activities_coordinator', 'activity_leader', 'student')
      AND role.deleted_at IS NULL AND scope.scope_kind IS NOT NULL
    ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
        WHERE deleted_at IS NULL DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_provision_new_tenant_activities_access ON tenants;
CREATE TRIGGER zz_provision_new_tenant_activities_access
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION provision_new_tenant_activities_access();

DROP TRIGGER IF EXISTS ev_activity_catalog_items ON activity_catalog_items;
CREATE TRIGGER ev_activity_catalog_items
    AFTER INSERT OR UPDATE OR DELETE ON activity_catalog_items
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_activity_groups ON activity_groups;
CREATE TRIGGER ev_activity_groups
    AFTER INSERT OR UPDATE OR DELETE ON activity_groups
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_activity_group_leaders ON activity_group_leaders;
CREATE TRIGGER ev_activity_group_leaders
    AFTER INSERT OR UPDATE OR DELETE ON activity_group_leaders
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_activity_group_memberships ON activity_group_memberships;
CREATE TRIGGER ev_activity_group_memberships
    AFTER INSERT OR UPDATE OR DELETE ON activity_group_memberships
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_activity_sessions ON activity_sessions;
CREATE TRIGGER ev_activity_sessions
    AFTER INSERT OR UPDATE OR DELETE ON activity_sessions
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_activity_session_participation ON activity_session_participation;
CREATE TRIGGER ev_activity_session_participation
    AFTER INSERT OR UPDATE OR DELETE ON activity_session_participation
    FOR EACH ROW EXECUTE FUNCTION log_event();
