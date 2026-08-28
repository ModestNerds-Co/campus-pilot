-- People, guardian relationships, admissions, and enrolment records owned by SIS.
--
-- Learners and guardians may link to login accounts, but account creation is
-- always a separate Administration workflow. Academic placement references
-- canonical Academics records by tenant-scoped identifiers.

CREATE TABLE IF NOT EXISTS learners (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    account_id UUID,
    learner_number TEXT NOT NULL,
    display_name TEXT NOT NULL,
    first_names TEXT,
    surname TEXT,
    date_of_birth DATE NOT NULL,
    email TEXT CHECK (email IS NULL OR email = LOWER(email)),
    phone TEXT,
    status TEXT NOT NULL DEFAULT 'prospective'
        CHECK (status IN ('prospective', 'active', 'inactive', 'graduated', 'withdrawn')),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (account_id, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_learners_tenant_number
    ON learners(tenant_id, LOWER(learner_number)) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_learners_tenant_account
    ON learners(tenant_id, account_id)
    WHERE account_id IS NOT NULL AND deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_learners_tenant_status
    ON learners(tenant_id, status) WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_learners_updated_at ON learners;
CREATE TRIGGER update_learners_updated_at
    BEFORE UPDATE ON learners
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS guardians (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    account_id UUID,
    display_name TEXT NOT NULL,
    first_names TEXT,
    surname TEXT,
    email TEXT CHECK (email IS NULL OR email = LOWER(email)),
    phone TEXT,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'inactive')),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (account_id, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (email IS NOT NULL OR phone IS NOT NULL)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_guardians_tenant_account
    ON guardians(tenant_id, account_id)
    WHERE account_id IS NOT NULL AND deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_guardians_tenant_status
    ON guardians(tenant_id, status) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_guardians_tenant_email
    ON guardians(tenant_id, LOWER(email))
    WHERE email IS NOT NULL AND deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_guardians_updated_at ON guardians;
CREATE TRIGGER update_guardians_updated_at
    BEFORE UPDATE ON guardians
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS learner_guardian_relationships (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    learner_id UUID NOT NULL,
    guardian_id UUID NOT NULL,
    relationship_type TEXT NOT NULL
        CHECK (relationship_type IN ('mother', 'father', 'parent', 'guardian', 'carer', 'sponsor', 'other')),
    is_primary BOOLEAN NOT NULL DEFAULT FALSE,
    can_collect BOOLEAN NOT NULL DEFAULT FALSE,
    receives_communications BOOLEAN NOT NULL DEFAULT TRUE,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'inactive')),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (learner_id, tenant_id) REFERENCES learners(id, tenant_id),
    FOREIGN KEY (guardian_id, tenant_id) REFERENCES guardians(id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_learner_guardian_active_pair
    ON learner_guardian_relationships(tenant_id, learner_id, guardian_id)
    WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_learner_guardian_primary
    ON learner_guardian_relationships(tenant_id, learner_id)
    WHERE is_primary = TRUE AND status = 'active' AND deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_learner_guardian_guardian
    ON learner_guardian_relationships(tenant_id, guardian_id)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_learner_guardian_relationships_updated_at
    ON learner_guardian_relationships;
CREATE TRIGGER update_learner_guardian_relationships_updated_at
    BEFORE UPDATE ON learner_guardian_relationships
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS applications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    application_number TEXT NOT NULL,
    learner_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    target_class_group_id UUID,
    submitted_on DATE,
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'submitted', 'under_review', 'offered', 'accepted', 'rejected', 'withdrawn')),
    notes TEXT,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    UNIQUE (id, tenant_id, learner_id, academic_year_id),
    FOREIGN KEY (learner_id, tenant_id) REFERENCES learners(id, tenant_id),
    FOREIGN KEY (academic_year_id, tenant_id) REFERENCES academic_years(id, tenant_id),
    FOREIGN KEY (target_class_group_id, tenant_id, academic_year_id)
        REFERENCES class_groups(id, tenant_id, academic_year_id),
    CHECK (status = 'draft' OR submitted_on IS NOT NULL)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_applications_tenant_number
    ON applications(tenant_id, LOWER(application_number)) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_applications_tenant_status
    ON applications(tenant_id, status, academic_year_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_applications_tenant_learner
    ON applications(tenant_id, learner_id) WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_applications_updated_at ON applications;
CREATE TRIGGER update_applications_updated_at
    BEFORE UPDATE ON applications
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS enrolments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    learner_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    class_group_id UUID NOT NULL,
    source_application_id UUID,
    starts_on DATE NOT NULL,
    ends_on DATE,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'completed', 'withdrawn')),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (learner_id, tenant_id) REFERENCES learners(id, tenant_id),
    FOREIGN KEY (class_group_id, tenant_id, academic_year_id)
        REFERENCES class_groups(id, tenant_id, academic_year_id),
    FOREIGN KEY (source_application_id, tenant_id, learner_id, academic_year_id)
        REFERENCES applications(id, tenant_id, learner_id, academic_year_id),
    CHECK (ends_on IS NULL OR ends_on >= starts_on)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_enrolments_active_learner_year
    ON enrolments(tenant_id, learner_id, academic_year_id)
    WHERE status = 'active' AND deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_enrolments_tenant_class
    ON enrolments(tenant_id, academic_year_id, class_group_id)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_enrolments_tenant_status
    ON enrolments(tenant_id, status) WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_enrolments_updated_at ON enrolments;
CREATE TRIGGER update_enrolments_updated_at
    BEFORE UPDATE ON enrolments
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

DROP TRIGGER IF EXISTS ev_learners ON learners;
CREATE TRIGGER ev_learners
    AFTER INSERT OR UPDATE OR DELETE ON learners
    FOR EACH ROW EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_guardians ON guardians;
CREATE TRIGGER ev_guardians
    AFTER INSERT OR UPDATE OR DELETE ON guardians
    FOR EACH ROW EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_learner_guardian_relationships
    ON learner_guardian_relationships;
CREATE TRIGGER ev_learner_guardian_relationships
    AFTER INSERT OR UPDATE OR DELETE ON learner_guardian_relationships
    FOR EACH ROW EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_applications ON applications;
CREATE TRIGGER ev_applications
    AFTER INSERT OR UPDATE OR DELETE ON applications
    FOR EACH ROW EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_enrolments ON enrolments;
CREATE TRIGGER ev_enrolments
    AFTER INSERT OR UPDATE OR DELETE ON enrolments
    FOR EACH ROW EXECUTE FUNCTION log_event();
