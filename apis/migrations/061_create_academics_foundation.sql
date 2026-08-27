-- Canonical academic structure and employee-backed teaching profiles.
--
-- This migration follows the already-applied HR migration. Academics owns
-- teaching structure; HR remains the source of employee identity.

CREATE TABLE IF NOT EXISTS academic_years (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    starts_on DATE NOT NULL,
    ends_on DATE NOT NULL,
    status TEXT NOT NULL DEFAULT 'planned'
        CHECK (status IN ('planned', 'active', 'closed')),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CHECK (ends_on >= starts_on)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_academic_years_tenant_name
    ON academic_years(tenant_id, LOWER(name)) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_academic_years_one_active
    ON academic_years(tenant_id) WHERE status = 'active' AND deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_academic_years_tenant_dates
    ON academic_years(tenant_id, starts_on DESC) WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_academic_years_updated_at ON academic_years;
CREATE TRIGGER update_academic_years_updated_at
    BEFORE UPDATE ON academic_years
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS subjects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'inactive')),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_subjects_tenant_code
    ON subjects(tenant_id, LOWER(code)) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_subjects_tenant_name
    ON subjects(tenant_id, LOWER(name)) WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_subjects_updated_at ON subjects;
CREATE TRIGGER update_subjects_updated_at
    BEFORE UPDATE ON subjects
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS teacher_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    employee_id UUID NOT NULL,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'inactive')),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (employee_id, tenant_id)
        REFERENCES employees(id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_teacher_profiles_tenant_employee
    ON teacher_profiles(tenant_id, employee_id) WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_teacher_profiles_updated_at ON teacher_profiles;
CREATE TRIGGER update_teacher_profiles_updated_at
    BEFORE UPDATE ON teacher_profiles
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS class_groups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    academic_year_id UUID NOT NULL,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    grade_level TEXT,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'inactive')),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    UNIQUE (id, tenant_id, academic_year_id),
    FOREIGN KEY (academic_year_id, tenant_id)
        REFERENCES academic_years(id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_class_groups_year_code
    ON class_groups(tenant_id, academic_year_id, LOWER(code))
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_class_groups_tenant_year
    ON class_groups(tenant_id, academic_year_id) WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_class_groups_updated_at ON class_groups;
CREATE TRIGGER update_class_groups_updated_at
    BEFORE UPDATE ON class_groups
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS teaching_assignments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    academic_year_id UUID NOT NULL,
    class_group_id UUID NOT NULL,
    subject_id UUID NOT NULL,
    teacher_profile_id UUID NOT NULL,
    periods_per_cycle SMALLINT NOT NULL
        CHECK (periods_per_cycle BETWEEN 1 AND 40),
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'inactive')),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (class_group_id, tenant_id, academic_year_id)
        REFERENCES class_groups(id, tenant_id, academic_year_id),
    FOREIGN KEY (subject_id, tenant_id)
        REFERENCES subjects(id, tenant_id),
    FOREIGN KEY (teacher_profile_id, tenant_id)
        REFERENCES teacher_profiles(id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_teaching_assignments_unique_active
    ON teaching_assignments(
        tenant_id,
        academic_year_id,
        class_group_id,
        subject_id,
        teacher_profile_id
    ) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_teaching_assignments_timetable
    ON teaching_assignments(tenant_id, academic_year_id, status)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_teaching_assignments_updated_at ON teaching_assignments;
CREATE TRIGGER update_teaching_assignments_updated_at
    BEFORE UPDATE ON teaching_assignments
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

DROP TRIGGER IF EXISTS ev_academic_years ON academic_years;
CREATE TRIGGER ev_academic_years
    AFTER INSERT OR UPDATE OR DELETE ON academic_years
    FOR EACH ROW
    EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_subjects ON subjects;
CREATE TRIGGER ev_subjects
    AFTER INSERT OR UPDATE OR DELETE ON subjects
    FOR EACH ROW
    EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_teacher_profiles ON teacher_profiles;
CREATE TRIGGER ev_teacher_profiles
    AFTER INSERT OR UPDATE OR DELETE ON teacher_profiles
    FOR EACH ROW
    EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_class_groups ON class_groups;
CREATE TRIGGER ev_class_groups
    AFTER INSERT OR UPDATE OR DELETE ON class_groups
    FOR EACH ROW
    EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_teaching_assignments ON teaching_assignments;
CREATE TRIGGER ev_teaching_assignments
    AFTER INSERT OR UPDATE OR DELETE ON teaching_assignments
    FOR EACH ROW
    EXECUTE FUNCTION log_event();
