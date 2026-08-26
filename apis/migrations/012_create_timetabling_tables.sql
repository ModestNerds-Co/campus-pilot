-- Conflict-aware timetable configuration and versioned generation runs.

CREATE TABLE IF NOT EXISTS timetable_configurations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL UNIQUE REFERENCES tenants(id) ON DELETE CASCADE,
    cycle_name TEXT NOT NULL DEFAULT 'Current academic cycle',
    days JSONB NOT NULL DEFAULT '[]'::JSONB,
    periods JSONB NOT NULL DEFAULT '[]'::JSONB,
    classes JSONB NOT NULL DEFAULT '[]'::JSONB,
    subjects JSONB NOT NULL DEFAULT '[]'::JSONB,
    teachers JSONB NOT NULL DEFAULT '[]'::JSONB,
    rooms JSONB NOT NULL DEFAULT '[]'::JSONB,
    lesson_requirements JSONB NOT NULL DEFAULT '[]'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS timetable_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'published', 'superseded')),
    configuration_snapshot JSONB NOT NULL,
    entries JSONB NOT NULL DEFAULT '[]'::JSONB,
    unresolved JSONB NOT NULL DEFAULT '[]'::JSONB,
    quality_score INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    published_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_timetable_runs_tenant_created
ON timetable_runs (tenant_id, created_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS idx_timetable_runs_published
ON timetable_runs (tenant_id)
WHERE status = 'published';

DROP TRIGGER IF EXISTS update_timetable_configurations_updated_at ON timetable_configurations;
CREATE TRIGGER update_timetable_configurations_updated_at
    BEFORE UPDATE ON timetable_configurations
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

DROP TRIGGER IF EXISTS ev_timetable_configurations ON timetable_configurations;
CREATE TRIGGER ev_timetable_configurations
    AFTER INSERT OR UPDATE OR DELETE ON timetable_configurations
    FOR EACH ROW
    EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_timetable_runs ON timetable_runs;
CREATE TRIGGER ev_timetable_runs
    AFTER INSERT OR UPDATE OR DELETE ON timetable_runs
    FOR EACH ROW
    EXECUTE FUNCTION log_event();
