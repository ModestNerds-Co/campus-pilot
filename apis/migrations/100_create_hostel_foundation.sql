-- Boarding operations over canonical SIS learner identity.
--
-- Hostel owns residences, rooms, allocation history, pastoral records, and
-- lifecycle evidence. Learner identity and contact data remain in SIS.

CREATE TABLE IF NOT EXISTS hostel_residences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    code TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(code)) BETWEEN 1 AND 30),
    name TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(name)) BETWEEN 1 AND 160),
    description TEXT CHECK (description IS NULL OR CHAR_LENGTH(BTRIM(description)) <= 1000),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_hostel_residences_code
    ON hostel_residences(tenant_id, LOWER(code));
CREATE UNIQUE INDEX IF NOT EXISTS idx_hostel_residences_name
    ON hostel_residences(tenant_id, LOWER(name));
CREATE INDEX IF NOT EXISTS idx_hostel_residences_status
    ON hostel_residences(tenant_id, status, name);
DROP TRIGGER IF EXISTS update_hostel_residences_updated_at ON hostel_residences;
CREATE TRIGGER update_hostel_residences_updated_at
    BEFORE UPDATE ON hostel_residences
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS hostel_rooms (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    residence_id UUID NOT NULL,
    code TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(code)) BETWEEN 1 AND 40),
    floor_label TEXT CHECK (floor_label IS NULL OR CHAR_LENGTH(BTRIM(floor_label)) <= 80),
    capacity SMALLINT NOT NULL CHECK (capacity BETWEEN 1 AND 50),
    status TEXT NOT NULL DEFAULT 'available'
        CHECK (status IN ('available', 'maintenance', 'inactive')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (residence_id, tenant_id) REFERENCES hostel_residences(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_hostel_rooms_code
    ON hostel_rooms(tenant_id, residence_id, LOWER(code));
CREATE INDEX IF NOT EXISTS idx_hostel_rooms_status
    ON hostel_rooms(tenant_id, residence_id, status, code);
DROP TRIGGER IF EXISTS update_hostel_rooms_updated_at ON hostel_rooms;
CREATE TRIGGER update_hostel_rooms_updated_at
    BEFORE UPDATE ON hostel_rooms
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS hostel_allocations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    learner_id UUID NOT NULL,
    room_id UUID NOT NULL,
    starts_on DATE NOT NULL,
    expected_end_on DATE,
    ended_on DATE,
    status TEXT NOT NULL DEFAULT 'planned'
        CHECK (status IN ('planned', 'active', 'ended', 'cancelled')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    previous_allocation_id UUID,
    decision_reason TEXT CHECK (
        decision_reason IS NULL OR CHAR_LENGTH(BTRIM(decision_reason)) BETWEEN 1 AND 1000
    ),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    ended_by UUID,
    ended_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (learner_id, tenant_id) REFERENCES learners(id, tenant_id),
    FOREIGN KEY (room_id, tenant_id) REFERENCES hostel_rooms(id, tenant_id),
    FOREIGN KEY (previous_allocation_id, tenant_id) REFERENCES hostel_allocations(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (ended_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (expected_end_on IS NULL OR expected_end_on >= starts_on),
    CHECK (
        (status IN ('planned', 'active') AND ended_on IS NULL AND ended_by IS NULL AND ended_at IS NULL)
        OR (status = 'ended' AND ended_on IS NOT NULL AND ended_on >= starts_on AND ended_by IS NOT NULL AND ended_at IS NOT NULL)
        OR (status = 'cancelled' AND ended_on IS NULL AND ended_by IS NOT NULL AND ended_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_hostel_allocations_current_learner
    ON hostel_allocations(tenant_id, learner_id)
    WHERE status IN ('planned', 'active');
CREATE INDEX IF NOT EXISTS idx_hostel_allocations_room
    ON hostel_allocations(tenant_id, room_id, status, starts_on);
CREATE INDEX IF NOT EXISTS idx_hostel_allocations_learner_history
    ON hostel_allocations(tenant_id, learner_id, starts_on DESC, created_at DESC);
DROP TRIGGER IF EXISTS update_hostel_allocations_updated_at ON hostel_allocations;
CREATE TRIGGER update_hostel_allocations_updated_at
    BEFORE UPDATE ON hostel_allocations
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS hostel_pastoral_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    learner_id UUID NOT NULL,
    allocation_id UUID,
    category TEXT NOT NULL
        CHECK (category IN ('wellbeing', 'behaviour', 'safeguarding', 'family_contact', 'other')),
    severity TEXT NOT NULL DEFAULT 'moderate'
        CHECK (severity IN ('low', 'moderate', 'high', 'critical')),
    subject TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(subject)) BETWEEN 1 AND 200),
    details TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(details)) BETWEEN 1 AND 6000),
    occurred_at TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'resolved')),
    resolution TEXT CHECK (resolution IS NULL OR CHAR_LENGTH(BTRIM(resolution)) BETWEEN 1 AND 4000),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    recorded_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    resolved_by UUID,
    resolved_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (learner_id, tenant_id) REFERENCES learners(id, tenant_id),
    FOREIGN KEY (allocation_id, tenant_id) REFERENCES hostel_allocations(id, tenant_id),
    FOREIGN KEY (recorded_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (resolved_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'open' AND resolution IS NULL AND resolved_by IS NULL AND resolved_at IS NULL)
        OR (status = 'resolved' AND resolution IS NOT NULL AND resolved_by IS NOT NULL AND resolved_at IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_hostel_pastoral_worklist
    ON hostel_pastoral_records(tenant_id, status, severity, occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_hostel_pastoral_learner
    ON hostel_pastoral_records(tenant_id, learner_id, occurred_at DESC);
DROP TRIGGER IF EXISTS update_hostel_pastoral_records_updated_at ON hostel_pastoral_records;
CREATE TRIGGER update_hostel_pastoral_records_updated_at
    BEFORE UPDATE ON hostel_pastoral_records
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS hostel_activity_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    aggregate_type TEXT NOT NULL
        CHECK (aggregate_type IN ('residence', 'room', 'allocation', 'pastoral_record')),
    aggregate_id UUID NOT NULL,
    learner_id UUID,
    event_type TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(event_type)) BETWEEN 3 AND 80),
    actor_id UUID NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (learner_id, tenant_id) REFERENCES learners(id, tenant_id),
    FOREIGN KEY (actor_id, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE INDEX IF NOT EXISTS idx_hostel_activity_history
    ON hostel_activity_events(tenant_id, aggregate_type, aggregate_id, created_at DESC, id);

CREATE OR REPLACE FUNCTION reject_hostel_activity_event_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Hostel activity events are append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS hostel_activity_events_append_only ON hostel_activity_events;
CREATE TRIGGER hostel_activity_events_append_only
    BEFORE UPDATE OR DELETE ON hostel_activity_events
    FOR EACH ROW EXECUTE FUNCTION reject_hostel_activity_event_mutation();

INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
SELECT tenant.id, 'hostel_officer', 'Hostel Officer',
       'Maintains residences, rooms, learner allocations, and pastoral records.',
       ARRAY[
           'hostel:view', 'hostel:create', 'hostel:edit',
           'hostel:allocate', 'hostel:pastoral', 'hostel:manage'
       ]::TEXT[], TRUE
  FROM tenants AS tenant
 WHERE NOT EXISTS (
    SELECT 1 FROM roles AS role
     WHERE role.tenant_id = tenant.id AND role.key = 'hostel_officer'
       AND role.deleted_at IS NULL
 );

CREATE OR REPLACE FUNCTION provision_new_tenant_hostel_officer()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
    VALUES (
        NEW.id, 'hostel_officer', 'Hostel Officer',
        'Maintains residences, rooms, learner allocations, and pastoral records.',
        ARRAY[
            'hostel:view', 'hostel:create', 'hostel:edit',
            'hostel:allocate', 'hostel:pastoral', 'hostel:manage'
        ]::TEXT[], TRUE
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_provision_new_tenant_hostel_officer ON tenants;
CREATE TRIGGER zz_provision_new_tenant_hostel_officer
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION provision_new_tenant_hostel_officer();

INSERT INTO role_record_scope_grants (tenant_id, role_id, scope_family, scope_kind)
SELECT role.tenant_id, role.id, seed.scope_family, 'campus'
FROM roles AS role
INNER JOIN (
    VALUES
        ('hostel_officer', 'hostel.occupancy'),
        ('hostel_officer', 'hostel.pastoral')
) AS seed(role_key, scope_family)
    ON role.key = seed.role_key AND role.deleted_at IS NULL
ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
    WHERE deleted_at IS NULL DO NOTHING;

-- Complete replacement of the canonical seed-role scope function.
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
            ('hostel_officer', 'hostel.pastoral', 'campus')
    ) AS seed(role_key, scope_family, scope_kind)
    WHERE seed.role_key = NEW.key
    ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
        WHERE deleted_at IS NULL DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
