-- School health operations over canonical SIS learner and HR employee identity.
--
-- Health owns only care-specific state. Names, contact details, guardian
-- relationships, and employment records remain in their source modules.

CREATE TABLE IF NOT EXISTS health_patients (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    learner_id UUID,
    employee_id UUID,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'inactive')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (learner_id, tenant_id) REFERENCES learners(id, tenant_id),
    FOREIGN KEY (employee_id, tenant_id) REFERENCES employees(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK ((learner_id IS NOT NULL)::INTEGER + (employee_id IS NOT NULL)::INTEGER = 1)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_health_patients_learner
    ON health_patients(tenant_id, learner_id) WHERE learner_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_health_patients_employee
    ON health_patients(tenant_id, employee_id) WHERE employee_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_health_patients_status
    ON health_patients(tenant_id, status, updated_at DESC);

DROP TRIGGER IF EXISTS update_health_patients_updated_at ON health_patients;
CREATE TRIGGER update_health_patients_updated_at
    BEFORE UPDATE ON health_patients
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS health_care_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    patient_id UUID NOT NULL,
    kind TEXT NOT NULL
        CHECK (kind IN ('allergy', 'condition', 'accommodation', 'action_plan')),
    title TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(title)) BETWEEN 1 AND 160),
    details TEXT CHECK (details IS NULL OR CHAR_LENGTH(BTRIM(details)) <= 4000),
    severity TEXT NOT NULL DEFAULT 'moderate'
        CHECK (severity IN ('low', 'moderate', 'high', 'critical')),
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'resolved')),
    reviewed_on DATE,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    resolved_by UUID,
    resolved_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (patient_id, tenant_id) REFERENCES health_patients(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (resolved_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'active' AND resolved_by IS NULL AND resolved_at IS NULL)
        OR (status = 'resolved' AND resolved_by IS NOT NULL AND resolved_at IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_health_care_items_patient
    ON health_care_items(tenant_id, patient_id, status, severity, updated_at DESC);
DROP TRIGGER IF EXISTS update_health_care_items_updated_at ON health_care_items;
CREATE TRIGGER update_health_care_items_updated_at
    BEFORE UPDATE ON health_care_items
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS health_visits (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    patient_id UUID NOT NULL,
    checked_in_at TIMESTAMPTZ NOT NULL,
    category TEXT NOT NULL
        CHECK (category IN ('illness', 'injury', 'medication', 'wellbeing', 'follow_up', 'other')),
    presenting_concern TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(presenting_concern)) BETWEEN 1 AND 2000),
    assessment TEXT CHECK (assessment IS NULL OR CHAR_LENGTH(BTRIM(assessment)) <= 4000),
    care_given TEXT CHECK (care_given IS NULL OR CHAR_LENGTH(BTRIM(care_given)) <= 4000),
    disposition TEXT CHECK (
        disposition IS NULL OR disposition IN (
            'returned_to_class', 'sent_home', 'emergency_referral',
            'guardian_collection', 'staff_released', 'other'
        )
    ),
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    opened_by UUID NOT NULL,
    closed_by UUID,
    closed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (patient_id, tenant_id) REFERENCES health_patients(id, tenant_id),
    FOREIGN KEY (opened_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (closed_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'open' AND disposition IS NULL AND closed_by IS NULL AND closed_at IS NULL)
        OR (status = 'closed' AND disposition IS NOT NULL AND closed_by IS NOT NULL AND closed_at IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_health_visits_worklist
    ON health_visits(tenant_id, status, checked_in_at DESC);
CREATE INDEX IF NOT EXISTS idx_health_visits_patient
    ON health_visits(tenant_id, patient_id, checked_in_at DESC);
DROP TRIGGER IF EXISTS update_health_visits_updated_at ON health_visits;
CREATE TRIGGER update_health_visits_updated_at
    BEFORE UPDATE ON health_visits
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS health_medication_plans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    patient_id UUID NOT NULL,
    medication_name TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(medication_name)) BETWEEN 1 AND 200),
    dosage TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(dosage)) BETWEEN 1 AND 160),
    route TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(route)) BETWEEN 1 AND 80),
    schedule TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(schedule)) BETWEEN 1 AND 300),
    instructions TEXT CHECK (instructions IS NULL OR CHAR_LENGTH(BTRIM(instructions)) <= 2000),
    authorization_reference TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(authorization_reference)) BETWEEN 1 AND 300),
    starts_on DATE NOT NULL,
    ends_on DATE,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'suspended', 'ended')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (patient_id, tenant_id) REFERENCES health_patients(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (ends_on IS NULL OR ends_on >= starts_on)
);

CREATE INDEX IF NOT EXISTS idx_health_medication_plans_patient
    ON health_medication_plans(tenant_id, patient_id, status, starts_on DESC);
DROP TRIGGER IF EXISTS update_health_medication_plans_updated_at ON health_medication_plans;
CREATE TRIGGER update_health_medication_plans_updated_at
    BEFORE UPDATE ON health_medication_plans
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS health_medication_administrations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    medication_plan_id UUID NOT NULL,
    patient_id UUID NOT NULL,
    administered_at TIMESTAMPTZ NOT NULL,
    dose TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(dose)) BETWEEN 1 AND 160),
    outcome TEXT NOT NULL CHECK (outcome IN ('given', 'refused', 'missed', 'held')),
    note TEXT CHECK (note IS NULL OR CHAR_LENGTH(BTRIM(note)) <= 2000),
    recorded_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (medication_plan_id, tenant_id) REFERENCES health_medication_plans(id, tenant_id),
    FOREIGN KEY (patient_id, tenant_id) REFERENCES health_patients(id, tenant_id),
    FOREIGN KEY (recorded_by, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE INDEX IF NOT EXISTS idx_health_medication_administrations_patient
    ON health_medication_administrations(tenant_id, patient_id, administered_at DESC);

CREATE OR REPLACE FUNCTION reject_health_medication_administration_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Medication administration records are append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS health_medication_administrations_append_only
    ON health_medication_administrations;
CREATE TRIGGER health_medication_administrations_append_only
    BEFORE UPDATE OR DELETE ON health_medication_administrations
    FOR EACH ROW EXECUTE FUNCTION reject_health_medication_administration_mutation();

CREATE TABLE IF NOT EXISTS health_follow_ups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    patient_id UUID NOT NULL,
    visit_id UUID,
    assigned_employee_id UUID,
    due_on DATE NOT NULL,
    purpose TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(purpose)) BETWEEN 1 AND 1000),
    status TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open', 'completed', 'cancelled')),
    outcome TEXT CHECK (outcome IS NULL OR CHAR_LENGTH(BTRIM(outcome)) BETWEEN 1 AND 2000),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (patient_id, tenant_id) REFERENCES health_patients(id, tenant_id),
    FOREIGN KEY (visit_id, tenant_id) REFERENCES health_visits(id, tenant_id),
    FOREIGN KEY (assigned_employee_id, tenant_id) REFERENCES employees(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'open' AND outcome IS NULL AND completed_at IS NULL)
        OR (status IN ('completed', 'cancelled') AND outcome IS NOT NULL AND completed_at IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_health_follow_ups_worklist
    ON health_follow_ups(tenant_id, status, due_on, patient_id);
DROP TRIGGER IF EXISTS update_health_follow_ups_updated_at ON health_follow_ups;
CREATE TRIGGER update_health_follow_ups_updated_at
    BEFORE UPDATE ON health_follow_ups
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS health_activity_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    aggregate_type TEXT NOT NULL
        CHECK (aggregate_type IN ('patient', 'care_item', 'visit', 'medication_plan', 'medication_administration', 'follow_up')),
    aggregate_id UUID NOT NULL,
    patient_id UUID NOT NULL,
    event_type TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(event_type)) BETWEEN 3 AND 80),
    actor_id UUID NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (patient_id, tenant_id) REFERENCES health_patients(id, tenant_id),
    FOREIGN KEY (actor_id, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE INDEX IF NOT EXISTS idx_health_activity_history
    ON health_activity_events(tenant_id, patient_id, created_at DESC, id);

CREATE OR REPLACE FUNCTION reject_health_activity_event_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Health activity events are append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS health_activity_events_append_only ON health_activity_events;
CREATE TRIGGER health_activity_events_append_only
    BEFORE UPDATE OR DELETE ON health_activity_events
    FOR EACH ROW EXECUTE FUNCTION reject_health_activity_event_mutation();

INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
SELECT tenant.id, 'health_officer', 'Health Officer',
       'Maintains campus health records, clinic visits, medication, and follow-up.',
       ARRAY[
           'health:view', 'health:create', 'health:edit',
           'health:medication', 'health:follow_up', 'health:manage'
       ]::TEXT[], TRUE
  FROM tenants AS tenant
 WHERE NOT EXISTS (
    SELECT 1 FROM roles AS role
     WHERE role.tenant_id = tenant.id AND role.key = 'health_officer'
       AND role.deleted_at IS NULL
 );

CREATE OR REPLACE FUNCTION provision_new_tenant_health_officer()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
    VALUES (
        NEW.id, 'health_officer', 'Health Officer',
        'Maintains campus health records, clinic visits, medication, and follow-up.',
        ARRAY[
            'health:view', 'health:create', 'health:edit',
            'health:medication', 'health:follow_up', 'health:manage'
        ]::TEXT[], TRUE
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_provision_new_tenant_health_officer ON tenants;
CREATE TRIGGER zz_provision_new_tenant_health_officer
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION provision_new_tenant_health_officer();

INSERT INTO role_record_scope_grants (tenant_id, role_id, scope_family, scope_kind)
SELECT role.tenant_id, role.id, seed.scope_family, 'campus'
FROM roles AS role
INNER JOIN (
    VALUES ('health_officer', 'health.patients'), ('health_officer', 'health.care')
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
            ('health_officer', 'health.care', 'campus')
    ) AS seed(role_key, scope_family, scope_kind)
    WHERE seed.role_key = NEW.key
    ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
        WHERE deleted_at IS NULL DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
