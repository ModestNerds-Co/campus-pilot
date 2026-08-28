-- Effective-dated academic terms owned by Academics.
--
-- SIS continues to own learner enrolment and class placement. Terms provide
-- the teaching period boundary consumed by assessment, attendance, and
-- timetabling without copying learner membership into Academics.

CREATE TABLE IF NOT EXISTS academic_terms (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    academic_year_id UUID NOT NULL,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    starts_on DATE NOT NULL,
    ends_on DATE NOT NULL,
    status TEXT NOT NULL DEFAULT 'planned'
        CHECK (status IN ('planned', 'active', 'closed')),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    UNIQUE (id, tenant_id, academic_year_id),
    FOREIGN KEY (academic_year_id, tenant_id)
        REFERENCES academic_years(id, tenant_id),
    CHECK (ends_on >= starts_on)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_academic_terms_year_code
    ON academic_terms(tenant_id, academic_year_id, LOWER(code))
    WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_academic_terms_one_active
    ON academic_terms(tenant_id)
    WHERE status = 'active' AND deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_academic_terms_tenant_year_dates
    ON academic_terms(tenant_id, academic_year_id, starts_on, ends_on)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_academic_terms_updated_at ON academic_terms;
CREATE TRIGGER update_academic_terms_updated_at
    BEFORE UPDATE ON academic_terms
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

DROP TRIGGER IF EXISTS ev_academic_terms ON academic_terms;
CREATE TRIGGER ev_academic_terms
    AFTER INSERT OR UPDATE OR DELETE ON academic_terms
    FOR EACH ROW
    EXECUTE FUNCTION log_event();
