-- School transport routes, rider assignments, service runs, and manifest evidence.
--
-- SIS owns learners and Fleet owns vehicles/drivers. Transport stores only
-- stable foreign identifiers plus immutable run snapshots needed for history.

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'vehicles_id_tenant_id_key'
    ) THEN
        ALTER TABLE vehicles ADD CONSTRAINT vehicles_id_tenant_id_key UNIQUE (id, tenant_id);
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'drivers_id_tenant_id_key'
    ) THEN
        ALTER TABLE drivers ADD CONSTRAINT drivers_id_tenant_id_key UNIQUE (id, tenant_id);
    END IF;
END;
$$;

CREATE TABLE IF NOT EXISTS transport_numbering_policies (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    run_prefix TEXT NOT NULL DEFAULT 'TRN-' CHECK (CHAR_LENGTH(BTRIM(run_prefix)) BETWEEN 1 AND 16),
    padding SMALLINT NOT NULL DEFAULT 6 CHECK (padding BETWEEN 3 AND 12),
    next_run_sequence BIGINT NOT NULL DEFAULT 1 CHECK (next_run_sequence > 0),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

INSERT INTO transport_numbering_policies (tenant_id)
SELECT tenant.id FROM tenants AS tenant
ON CONFLICT (tenant_id) DO NOTHING;

CREATE OR REPLACE FUNCTION provision_transport_numbering_policy()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO transport_numbering_policies (tenant_id)
    VALUES (NEW.id) ON CONFLICT (tenant_id) DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_provision_transport_numbering_policy ON tenants;
CREATE TRIGGER zz_provision_transport_numbering_policy
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION provision_transport_numbering_policy();
DROP TRIGGER IF EXISTS update_transport_numbering_policies_updated_at ON transport_numbering_policies;
CREATE TRIGGER update_transport_numbering_policies_updated_at
    BEFORE UPDATE ON transport_numbering_policies
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS transport_routes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    code TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(code)) BETWEEN 1 AND 24),
    name TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(name)) BETWEEN 1 AND 160),
    direction TEXT NOT NULL CHECK (direction IN ('inbound', 'outbound')),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive')),
    notes TEXT CHECK (notes IS NULL OR CHAR_LENGTH(BTRIM(notes)) <= 2000),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_transport_routes_code
    ON transport_routes(tenant_id, LOWER(code)) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_transport_routes_worklist
    ON transport_routes(tenant_id, status, direction, name) WHERE deleted_at IS NULL;
DROP TRIGGER IF EXISTS update_transport_routes_updated_at ON transport_routes;
CREATE TRIGGER update_transport_routes_updated_at
    BEFORE UPDATE ON transport_routes
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS transport_route_stops (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    route_id UUID NOT NULL,
    code TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(code)) BETWEEN 1 AND 24),
    name TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(name)) BETWEEN 1 AND 160),
    stop_order INTEGER NOT NULL CHECK (stop_order > 0),
    planned_time TIME NOT NULL,
    latitude DOUBLE PRECISION CHECK (latitude IS NULL OR latitude BETWEEN -90 AND 90),
    longitude DOUBLE PRECISION CHECK (longitude IS NULL OR longitude BETWEEN -180 AND 180),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    UNIQUE (id, tenant_id, route_id),
    FOREIGN KEY (route_id, tenant_id) REFERENCES transport_routes(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_transport_route_stops_order
    ON transport_route_stops(tenant_id, route_id, stop_order) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_transport_route_stops_code
    ON transport_route_stops(tenant_id, route_id, LOWER(code)) WHERE deleted_at IS NULL;
DROP TRIGGER IF EXISTS update_transport_route_stops_updated_at ON transport_route_stops;
CREATE TRIGGER update_transport_route_stops_updated_at
    BEFORE UPDATE ON transport_route_stops
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS transport_rider_assignments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    learner_id UUID NOT NULL,
    route_id UUID NOT NULL,
    boarding_stop_id UUID NOT NULL,
    alighting_stop_id UUID NOT NULL,
    effective_from DATE NOT NULL,
    effective_until DATE,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'ended', 'cancelled')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    ended_by UUID,
    ended_at TIMESTAMPTZ,
    end_reason TEXT CHECK (end_reason IS NULL OR CHAR_LENGTH(BTRIM(end_reason)) BETWEEN 1 AND 1000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    FOREIGN KEY (learner_id, tenant_id) REFERENCES learners(id, tenant_id),
    FOREIGN KEY (route_id, tenant_id) REFERENCES transport_routes(id, tenant_id),
    FOREIGN KEY (boarding_stop_id, tenant_id, route_id)
        REFERENCES transport_route_stops(id, tenant_id, route_id),
    FOREIGN KEY (alighting_stop_id, tenant_id, route_id)
        REFERENCES transport_route_stops(id, tenant_id, route_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (ended_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (boarding_stop_id <> alighting_stop_id),
    CHECK (effective_until IS NULL OR effective_until >= effective_from),
    CHECK (
        (status = 'active' AND ended_by IS NULL AND ended_at IS NULL AND end_reason IS NULL)
        OR (status IN ('ended', 'cancelled')
            AND ended_by IS NOT NULL AND ended_at IS NOT NULL AND end_reason IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_transport_riders_route_date
    ON transport_rider_assignments(tenant_id, route_id, effective_from, effective_until)
    WHERE deleted_at IS NULL AND status = 'active';
CREATE INDEX IF NOT EXISTS idx_transport_riders_learner
    ON transport_rider_assignments(tenant_id, learner_id, status, effective_from DESC)
    WHERE deleted_at IS NULL;
DROP TRIGGER IF EXISTS update_transport_rider_assignments_updated_at ON transport_rider_assignments;
CREATE TRIGGER update_transport_rider_assignments_updated_at
    BEFORE UPDATE ON transport_rider_assignments
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS transport_service_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    reference TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(reference)) BETWEEN 1 AND 48),
    route_id UUID NOT NULL,
    service_date DATE NOT NULL,
    vehicle_id UUID NOT NULL,
    driver_id UUID NOT NULL,
    route_code_snapshot TEXT NOT NULL,
    route_name_snapshot TEXT NOT NULL,
    direction_snapshot TEXT NOT NULL CHECK (direction_snapshot IN ('inbound', 'outbound')),
    vehicle_registration_snapshot TEXT NOT NULL,
    driver_name_snapshot TEXT NOT NULL,
    capacity_snapshot INTEGER NOT NULL CHECK (capacity_snapshot > 0),
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'boarding', 'departed', 'completed', 'cancelled')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    boarding_started_by UUID,
    boarding_started_at TIMESTAMPTZ,
    departed_by UUID,
    departed_at TIMESTAMPTZ,
    completed_by UUID,
    completed_at TIMESTAMPTZ,
    cancelled_by UUID,
    cancelled_at TIMESTAMPTZ,
    cancellation_reason TEXT CHECK (
        cancellation_reason IS NULL OR CHAR_LENGTH(BTRIM(cancellation_reason)) BETWEEN 1 AND 1000
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, reference),
    FOREIGN KEY (route_id, tenant_id) REFERENCES transport_routes(id, tenant_id),
    FOREIGN KEY (vehicle_id, tenant_id) REFERENCES vehicles(id, tenant_id),
    FOREIGN KEY (driver_id, tenant_id) REFERENCES drivers(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (boarding_started_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (departed_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (completed_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (cancelled_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'draft'
            AND boarding_started_by IS NULL AND boarding_started_at IS NULL
            AND departed_by IS NULL AND departed_at IS NULL
            AND completed_by IS NULL AND completed_at IS NULL
            AND cancelled_by IS NULL AND cancelled_at IS NULL AND cancellation_reason IS NULL)
        OR (status = 'boarding'
            AND boarding_started_by IS NOT NULL AND boarding_started_at IS NOT NULL
            AND departed_by IS NULL AND departed_at IS NULL
            AND completed_by IS NULL AND completed_at IS NULL
            AND cancelled_by IS NULL AND cancelled_at IS NULL AND cancellation_reason IS NULL)
        OR (status = 'departed'
            AND boarding_started_by IS NOT NULL AND boarding_started_at IS NOT NULL
            AND departed_by IS NOT NULL AND departed_at IS NOT NULL
            AND completed_by IS NULL AND completed_at IS NULL
            AND cancelled_by IS NULL AND cancelled_at IS NULL AND cancellation_reason IS NULL)
        OR (status = 'completed'
            AND boarding_started_by IS NOT NULL AND boarding_started_at IS NOT NULL
            AND departed_by IS NOT NULL AND departed_at IS NOT NULL
            AND completed_by IS NOT NULL AND completed_at IS NOT NULL
            AND cancelled_by IS NULL AND cancelled_at IS NULL AND cancellation_reason IS NULL)
        OR (status = 'cancelled'
            AND completed_by IS NULL AND completed_at IS NULL
            AND cancelled_by IS NOT NULL AND cancelled_at IS NOT NULL AND cancellation_reason IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_transport_runs_route_date_active
    ON transport_service_runs(tenant_id, route_id, service_date)
    WHERE deleted_at IS NULL AND status <> 'cancelled';
CREATE INDEX IF NOT EXISTS idx_transport_runs_worklist
    ON transport_service_runs(tenant_id, service_date DESC, status, route_id)
    WHERE deleted_at IS NULL;
DROP TRIGGER IF EXISTS update_transport_service_runs_updated_at ON transport_service_runs;
CREATE TRIGGER update_transport_service_runs_updated_at
    BEFORE UPDATE ON transport_service_runs
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS transport_run_stops (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    run_id UUID NOT NULL,
    source_stop_id UUID NOT NULL,
    stop_order INTEGER NOT NULL CHECK (stop_order > 0),
    code_snapshot TEXT NOT NULL,
    name_snapshot TEXT NOT NULL,
    planned_time_snapshot TIME NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    UNIQUE (id, tenant_id, run_id),
    UNIQUE (tenant_id, run_id, source_stop_id),
    UNIQUE (tenant_id, run_id, stop_order),
    FOREIGN KEY (run_id, tenant_id) REFERENCES transport_service_runs(id, tenant_id),
    FOREIGN KEY (source_stop_id, tenant_id) REFERENCES transport_route_stops(id, tenant_id),
    CHECK (updated_at = created_at AND deleted_at IS NULL)
);

CREATE TABLE IF NOT EXISTS transport_manifest_entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    run_id UUID NOT NULL,
    source_assignment_id UUID NOT NULL,
    learner_id UUID NOT NULL,
    learner_number_snapshot TEXT NOT NULL,
    learner_name_snapshot TEXT NOT NULL,
    boarding_run_stop_id UUID NOT NULL,
    alighting_run_stop_id UUID NOT NULL,
    status TEXT NOT NULL DEFAULT 'expected'
        CHECK (status IN ('expected', 'boarded', 'no_show', 'exception')),
    exception_kind TEXT CHECK (
        exception_kind IS NULL OR exception_kind IN (
            'not_at_stop', 'illness', 'transport_change', 'conduct', 'safety', 'other'
        )
    ),
    note TEXT CHECK (note IS NULL OR CHAR_LENGTH(BTRIM(note)) <= 1000),
    marked_by UUID,
    marked_at TIMESTAMPTZ,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, run_id, learner_id),
    FOREIGN KEY (run_id, tenant_id) REFERENCES transport_service_runs(id, tenant_id),
    FOREIGN KEY (source_assignment_id, tenant_id)
        REFERENCES transport_rider_assignments(id, tenant_id),
    FOREIGN KEY (learner_id, tenant_id) REFERENCES learners(id, tenant_id),
    FOREIGN KEY (boarding_run_stop_id, tenant_id, run_id)
        REFERENCES transport_run_stops(id, tenant_id, run_id),
    FOREIGN KEY (alighting_run_stop_id, tenant_id, run_id)
        REFERENCES transport_run_stops(id, tenant_id, run_id),
    FOREIGN KEY (marked_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (boarding_run_stop_id <> alighting_run_stop_id),
    CHECK (
        (status = 'expected'
            AND exception_kind IS NULL AND note IS NULL AND marked_by IS NULL AND marked_at IS NULL)
        OR (status = 'boarded'
            AND exception_kind IS NULL AND marked_by IS NOT NULL AND marked_at IS NOT NULL)
        OR (status = 'no_show'
            AND exception_kind IS NULL AND marked_by IS NOT NULL AND marked_at IS NOT NULL)
        OR (status = 'exception'
            AND exception_kind IS NOT NULL AND note IS NOT NULL
            AND marked_by IS NOT NULL AND marked_at IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_transport_manifest_run_status
    ON transport_manifest_entries(tenant_id, run_id, status, created_at)
    WHERE deleted_at IS NULL;
DROP TRIGGER IF EXISTS update_transport_manifest_entries_updated_at ON transport_manifest_entries;
CREATE TRIGGER update_transport_manifest_entries_updated_at
    BEFORE UPDATE ON transport_manifest_entries
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS transport_run_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    run_id UUID NOT NULL,
    manifest_entry_id UUID,
    event_type TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(event_type)) BETWEEN 3 AND 100),
    actor_id UUID NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    FOREIGN KEY (run_id, tenant_id) REFERENCES transport_service_runs(id, tenant_id),
    FOREIGN KEY (manifest_entry_id, tenant_id)
        REFERENCES transport_manifest_entries(id, tenant_id),
    FOREIGN KEY (actor_id, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (updated_at = created_at AND deleted_at IS NULL)
);

CREATE INDEX IF NOT EXISTS idx_transport_run_events_history
    ON transport_run_events(tenant_id, run_id, created_at, id);

CREATE OR REPLACE FUNCTION reject_transport_snapshot_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Transport run snapshots and lifecycle events are append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS transport_run_stops_append_only ON transport_run_stops;
CREATE TRIGGER transport_run_stops_append_only
    BEFORE UPDATE OR DELETE ON transport_run_stops
    FOR EACH ROW EXECUTE FUNCTION reject_transport_snapshot_mutation();
DROP TRIGGER IF EXISTS transport_run_events_append_only ON transport_run_events;
CREATE TRIGGER transport_run_events_append_only
    BEFORE UPDATE OR DELETE ON transport_run_events
    FOR EACH ROW EXECUTE FUNCTION reject_transport_snapshot_mutation();

INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
SELECT tenant.id, seed.key, seed.name, seed.description, seed.permissions, TRUE
FROM tenants AS tenant
CROSS JOIN (
    VALUES
        (
            'transport_officer',
            'Transport Officer',
            'Runs daily manifests, boarding, no-show, and transport-exception workflows.',
            ARRAY['transport:view', 'transport:operate']::TEXT[]
        ),
        (
            'transport_manager',
            'Transport Manager',
            'Configures routes and riders and oversees daily transport operations.',
            ARRAY['transport:view', 'transport:configure', 'transport:operate', 'transport:manage']::TEXT[]
        )
) AS seed(key, name, description, permissions)
WHERE NOT EXISTS (
    SELECT 1 FROM roles AS role
    WHERE role.tenant_id = tenant.id AND role.key = seed.key AND role.deleted_at IS NULL
);

CREATE OR REPLACE FUNCTION provision_new_tenant_transport_roles()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
    VALUES
        (
            NEW.id, 'transport_officer', 'Transport Officer',
            'Runs daily manifests, boarding, no-show, and transport-exception workflows.',
            ARRAY['transport:view', 'transport:operate']::TEXT[], TRUE
        ),
        (
            NEW.id, 'transport_manager', 'Transport Manager',
            'Configures routes and riders and oversees daily transport operations.',
            ARRAY['transport:view', 'transport:configure', 'transport:operate', 'transport:manage']::TEXT[], TRUE
        );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_provision_new_tenant_transport_roles ON tenants;
CREATE TRIGGER zz_provision_new_tenant_transport_roles
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION provision_new_tenant_transport_roles();

DROP TRIGGER IF EXISTS ev_transport_routes ON transport_routes;
CREATE TRIGGER ev_transport_routes AFTER INSERT OR UPDATE OR DELETE ON transport_routes
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_transport_route_stops ON transport_route_stops;
CREATE TRIGGER ev_transport_route_stops AFTER INSERT OR UPDATE OR DELETE ON transport_route_stops
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_transport_rider_assignments ON transport_rider_assignments;
CREATE TRIGGER ev_transport_rider_assignments AFTER INSERT OR UPDATE OR DELETE ON transport_rider_assignments
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_transport_service_runs ON transport_service_runs;
CREATE TRIGGER ev_transport_service_runs AFTER INSERT OR UPDATE OR DELETE ON transport_service_runs
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_transport_manifest_entries ON transport_manifest_entries;
CREATE TRIGGER ev_transport_manifest_entries AFTER INSERT OR UPDATE OR DELETE ON transport_manifest_entries
    FOR EACH ROW EXECUTE FUNCTION log_event();
