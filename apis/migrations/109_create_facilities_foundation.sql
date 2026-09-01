-- Facilities locations, service requests, work orders, and inspection evidence.
--
-- HR remains authoritative for employee identity. Facilities owns operational
-- location and maintenance lifecycles without creating shadow staff records.

CREATE TABLE IF NOT EXISTS facilities_numbering_policies (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    request_prefix TEXT NOT NULL DEFAULT 'FSR-'
        CHECK (CHAR_LENGTH(BTRIM(request_prefix)) BETWEEN 1 AND 16),
    next_request_sequence BIGINT NOT NULL DEFAULT 1 CHECK (next_request_sequence > 0),
    work_order_prefix TEXT NOT NULL DEFAULT 'FWO-'
        CHECK (CHAR_LENGTH(BTRIM(work_order_prefix)) BETWEEN 1 AND 16),
    next_work_order_sequence BIGINT NOT NULL DEFAULT 1 CHECK (next_work_order_sequence > 0),
    padding SMALLINT NOT NULL DEFAULT 6 CHECK (padding BETWEEN 3 AND 12),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

INSERT INTO facilities_numbering_policies (tenant_id)
SELECT tenant.id FROM tenants AS tenant
ON CONFLICT (tenant_id) DO NOTHING;

CREATE OR REPLACE FUNCTION provision_facilities_numbering_policy()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO facilities_numbering_policies (tenant_id)
    VALUES (NEW.id)
    ON CONFLICT (tenant_id) DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_provision_facilities_numbering_policy ON tenants;
CREATE TRIGGER zz_provision_facilities_numbering_policy
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION provision_facilities_numbering_policy();

DROP TRIGGER IF EXISTS update_facilities_numbering_policies_updated_at
    ON facilities_numbering_policies;
CREATE TRIGGER update_facilities_numbering_policies_updated_at
    BEFORE UPDATE ON facilities_numbering_policies
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS facility_locations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    parent_id UUID,
    kind TEXT NOT NULL CHECK (kind IN ('site', 'building', 'floor', 'room', 'external_area')),
    code TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(code)) BETWEEN 1 AND 40),
    name TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(name)) BETWEEN 1 AND 160),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
    capacity INTEGER CHECK (capacity IS NULL OR capacity > 0),
    notes TEXT CHECK (notes IS NULL OR CHAR_LENGTH(BTRIM(notes)) BETWEEN 1 AND 4000),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    archived_by UUID,
    archived_at TIMESTAMPTZ,
    archive_reason TEXT CHECK (
        archive_reason IS NULL OR CHAR_LENGTH(BTRIM(archive_reason)) BETWEEN 1 AND 2000
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    FOREIGN KEY (parent_id, tenant_id) REFERENCES facility_locations(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (archived_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'active' AND archived_by IS NULL AND archived_at IS NULL AND archive_reason IS NULL)
        OR
        (status = 'archived' AND archived_by IS NOT NULL AND archived_at IS NOT NULL AND archive_reason IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_facility_locations_code
    ON facility_locations(tenant_id, LOWER(code)) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_facility_locations_parent
    ON facility_locations(tenant_id, parent_id, kind) WHERE deleted_at IS NULL;

CREATE OR REPLACE FUNCTION validate_facility_location_hierarchy()
RETURNS TRIGGER AS $$
DECLARE
    parent_kind TEXT;
    parent_status TEXT;
    cycle_found BOOLEAN;
BEGIN
    IF NEW.parent_id = NEW.id THEN
        RAISE EXCEPTION 'A facility location cannot be its own parent';
    END IF;
    IF NEW.kind = 'site' AND NEW.parent_id IS NOT NULL THEN
        RAISE EXCEPTION 'A site cannot have a parent location';
    END IF;
    IF NEW.kind <> 'site' AND NEW.parent_id IS NULL THEN
        RAISE EXCEPTION 'This facility location requires a parent';
    END IF;
    IF NEW.parent_id IS NOT NULL THEN
        SELECT location.kind, location.status
          INTO parent_kind, parent_status
          FROM facility_locations AS location
         WHERE location.id = NEW.parent_id
           AND location.tenant_id = NEW.tenant_id
           AND location.deleted_at IS NULL;
        IF parent_kind IS NULL THEN
            RAISE EXCEPTION 'The parent facility location was not found';
        END IF;
        IF parent_status <> 'active' THEN
            RAISE EXCEPTION 'An archived facility location cannot be used as a parent';
        END IF;
        IF NOT (
            (NEW.kind = 'building' AND parent_kind = 'site')
            OR (NEW.kind = 'floor' AND parent_kind = 'building')
            OR (NEW.kind = 'room' AND parent_kind IN ('building', 'floor'))
            OR (NEW.kind = 'external_area' AND parent_kind = 'site')
        ) THEN
            RAISE EXCEPTION 'The selected facility parent is not valid for this location type';
        END IF;
        WITH RECURSIVE ancestors AS (
            SELECT location.id, location.parent_id
              FROM facility_locations AS location
             WHERE location.id = NEW.parent_id AND location.tenant_id = NEW.tenant_id
            UNION ALL
            SELECT location.id, location.parent_id
              FROM facility_locations AS location
              JOIN ancestors ON ancestors.parent_id = location.id
             WHERE location.tenant_id = NEW.tenant_id
        )
        SELECT EXISTS (SELECT 1 FROM ancestors WHERE id = NEW.id) INTO cycle_found;
        IF cycle_found THEN
            RAISE EXCEPTION 'The facility hierarchy cannot contain a cycle';
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS validate_facility_location_hierarchy_before_write
    ON facility_locations;
CREATE TRIGGER validate_facility_location_hierarchy_before_write
    BEFORE INSERT OR UPDATE OF parent_id, kind, status ON facility_locations
    FOR EACH ROW EXECUTE FUNCTION validate_facility_location_hierarchy();

DROP TRIGGER IF EXISTS update_facility_locations_updated_at ON facility_locations;
CREATE TRIGGER update_facility_locations_updated_at
    BEFORE UPDATE ON facility_locations
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS facility_service_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    reference TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(reference)) BETWEEN 1 AND 40),
    location_id UUID NOT NULL,
    reporter_user_id UUID NOT NULL,
    priority TEXT NOT NULL CHECK (priority IN ('low', 'normal', 'high', 'urgent')),
    summary TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(summary)) BETWEEN 1 AND 200),
    description TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(description)) BETWEEN 1 AND 6000),
    status TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open', 'assigned', 'resolved', 'closed', 'cancelled')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    resolved_by UUID,
    resolved_at TIMESTAMPTZ,
    resolution_summary TEXT CHECK (
        resolution_summary IS NULL OR CHAR_LENGTH(BTRIM(resolution_summary)) BETWEEN 1 AND 6000
    ),
    closed_by UUID,
    closed_at TIMESTAMPTZ,
    closure_reason TEXT CHECK (
        closure_reason IS NULL OR CHAR_LENGTH(BTRIM(closure_reason)) BETWEEN 1 AND 3000
    ),
    cancelled_by UUID,
    cancelled_at TIMESTAMPTZ,
    cancellation_reason TEXT CHECK (
        cancellation_reason IS NULL OR CHAR_LENGTH(BTRIM(cancellation_reason)) BETWEEN 1 AND 3000
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, reference),
    FOREIGN KEY (location_id, tenant_id) REFERENCES facility_locations(id, tenant_id),
    FOREIGN KEY (reporter_user_id, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (resolved_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (closed_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (cancelled_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status IN ('open', 'assigned')
            AND resolved_by IS NULL AND resolved_at IS NULL AND resolution_summary IS NULL
            AND closed_by IS NULL AND closed_at IS NULL AND closure_reason IS NULL
            AND cancelled_by IS NULL AND cancelled_at IS NULL AND cancellation_reason IS NULL)
        OR
        (status = 'resolved'
            AND resolved_by IS NOT NULL AND resolved_at IS NOT NULL AND resolution_summary IS NOT NULL
            AND closed_by IS NULL AND closed_at IS NULL AND closure_reason IS NULL
            AND cancelled_by IS NULL AND cancelled_at IS NULL AND cancellation_reason IS NULL)
        OR
        (status = 'closed'
            AND resolved_by IS NOT NULL AND resolved_at IS NOT NULL AND resolution_summary IS NOT NULL
            AND closed_by IS NOT NULL AND closed_at IS NOT NULL AND closure_reason IS NOT NULL
            AND cancelled_by IS NULL AND cancelled_at IS NULL AND cancellation_reason IS NULL)
        OR
        (status = 'cancelled'
            AND resolved_by IS NULL AND resolved_at IS NULL AND resolution_summary IS NULL
            AND closed_by IS NULL AND closed_at IS NULL AND closure_reason IS NULL
            AND cancelled_by IS NOT NULL AND cancelled_at IS NOT NULL AND cancellation_reason IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_facility_service_requests_status
    ON facility_service_requests(tenant_id, status, priority, created_at DESC)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_facility_service_requests_reporter
    ON facility_service_requests(tenant_id, reporter_user_id, created_at DESC)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_facility_service_requests_updated_at
    ON facility_service_requests;
CREATE TRIGGER update_facility_service_requests_updated_at
    BEFORE UPDATE ON facility_service_requests
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS facility_work_orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    reference TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(reference)) BETWEEN 1 AND 40),
    service_request_id UUID NOT NULL,
    location_id UUID NOT NULL,
    assigned_employee_id UUID NOT NULL,
    title TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(title)) BETWEEN 1 AND 200),
    instructions TEXT CHECK (
        instructions IS NULL OR CHAR_LENGTH(BTRIM(instructions)) BETWEEN 1 AND 6000
    ),
    target_date DATE,
    status TEXT NOT NULL DEFAULT 'assigned'
        CHECK (status IN ('assigned', 'in_progress', 'ready_for_inspection', 'completed', 'cancelled')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    started_by UUID,
    started_at TIMESTAMPTZ,
    completed_by UUID,
    completed_at TIMESTAMPTZ,
    cancelled_by UUID,
    cancelled_at TIMESTAMPTZ,
    cancellation_reason TEXT CHECK (
        cancellation_reason IS NULL OR CHAR_LENGTH(BTRIM(cancellation_reason)) BETWEEN 1 AND 3000
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, reference),
    UNIQUE (tenant_id, service_request_id),
    FOREIGN KEY (service_request_id, tenant_id) REFERENCES facility_service_requests(id, tenant_id),
    FOREIGN KEY (location_id, tenant_id) REFERENCES facility_locations(id, tenant_id),
    FOREIGN KEY (assigned_employee_id, tenant_id) REFERENCES employees(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (started_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (completed_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (cancelled_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'assigned'
            AND started_by IS NULL AND started_at IS NULL
            AND completed_by IS NULL AND completed_at IS NULL
            AND cancelled_by IS NULL AND cancelled_at IS NULL AND cancellation_reason IS NULL)
        OR
        (status = 'in_progress'
            AND started_by IS NOT NULL AND started_at IS NOT NULL
            AND completed_by IS NULL AND completed_at IS NULL
            AND cancelled_by IS NULL AND cancelled_at IS NULL AND cancellation_reason IS NULL)
        OR
        (status = 'ready_for_inspection'
            AND started_by IS NOT NULL AND started_at IS NOT NULL
            AND completed_by IS NULL AND completed_at IS NULL
            AND cancelled_by IS NULL AND cancelled_at IS NULL AND cancellation_reason IS NULL)
        OR
        (status = 'completed'
            AND started_by IS NOT NULL AND started_at IS NOT NULL
            AND completed_by IS NOT NULL AND completed_at IS NOT NULL
            AND cancelled_by IS NULL AND cancelled_at IS NULL AND cancellation_reason IS NULL)
        OR
        (status = 'cancelled'
            AND completed_by IS NULL AND completed_at IS NULL
            AND cancelled_by IS NOT NULL AND cancelled_at IS NOT NULL AND cancellation_reason IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_facility_work_orders_status
    ON facility_work_orders(tenant_id, status, target_date, created_at DESC)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_facility_work_orders_assignee
    ON facility_work_orders(tenant_id, assigned_employee_id, status)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_facility_work_orders_updated_at ON facility_work_orders;
CREATE TRIGGER update_facility_work_orders_updated_at
    BEFORE UPDATE ON facility_work_orders
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS facility_work_order_completion_submissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    work_order_id UUID NOT NULL,
    summary TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(summary)) BETWEEN 1 AND 6000),
    submitted_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    FOREIGN KEY (work_order_id, tenant_id) REFERENCES facility_work_orders(id, tenant_id),
    FOREIGN KEY (submitted_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (updated_at = created_at AND deleted_at IS NULL)
);

CREATE INDEX IF NOT EXISTS idx_facility_completion_submissions_history
    ON facility_work_order_completion_submissions(
        tenant_id, work_order_id, created_at DESC, id DESC
    );

CREATE TABLE IF NOT EXISTS facility_work_order_inspections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    work_order_id UUID NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('pass', 'fail')),
    notes TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(notes)) BETWEEN 1 AND 6000),
    inspected_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    FOREIGN KEY (work_order_id, tenant_id) REFERENCES facility_work_orders(id, tenant_id),
    FOREIGN KEY (inspected_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (updated_at = created_at AND deleted_at IS NULL)
);

CREATE INDEX IF NOT EXISTS idx_facility_inspections_history
    ON facility_work_order_inspections(tenant_id, work_order_id, created_at DESC, id DESC);

CREATE TABLE IF NOT EXISTS facility_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    service_request_id UUID,
    work_order_id UUID,
    event_type TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(event_type)) BETWEEN 3 AND 120),
    actor_id UUID NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    FOREIGN KEY (service_request_id, tenant_id)
        REFERENCES facility_service_requests(id, tenant_id),
    FOREIGN KEY (work_order_id, tenant_id) REFERENCES facility_work_orders(id, tenant_id),
    FOREIGN KEY (actor_id, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (service_request_id IS NOT NULL OR work_order_id IS NOT NULL),
    CHECK (updated_at = created_at AND deleted_at IS NULL)
);

CREATE INDEX IF NOT EXISTS idx_facility_events_request_history
    ON facility_events(tenant_id, service_request_id, created_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_facility_events_work_order_history
    ON facility_events(tenant_id, work_order_id, created_at DESC, id DESC);

CREATE OR REPLACE FUNCTION reject_facilities_evidence_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Facilities inspection and lifecycle evidence is append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS facility_inspections_append_only ON facility_work_order_inspections;
CREATE TRIGGER facility_inspections_append_only
    BEFORE UPDATE OR DELETE ON facility_work_order_inspections
    FOR EACH ROW EXECUTE FUNCTION reject_facilities_evidence_mutation();
DROP TRIGGER IF EXISTS facility_completion_submissions_append_only
    ON facility_work_order_completion_submissions;
CREATE TRIGGER facility_completion_submissions_append_only
    BEFORE UPDATE OR DELETE ON facility_work_order_completion_submissions
    FOR EACH ROW EXECUTE FUNCTION reject_facilities_evidence_mutation();
DROP TRIGGER IF EXISTS facility_events_append_only ON facility_events;
CREATE TRIGGER facility_events_append_only
    BEFORE UPDATE OR DELETE ON facility_events
    FOR EACH ROW EXECUTE FUNCTION reject_facilities_evidence_mutation();

INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
SELECT tenant.id, seed.key, seed.name, seed.description, seed.permissions, TRUE
FROM tenants AS tenant
CROSS JOIN (
    VALUES
        (
            'facilities_officer',
            'Facilities Officer',
            'Reviews campus service requests and carries out assigned facilities work orders.',
            ARRAY['facilities:view', 'facilities:request', 'facilities:operate']::TEXT[]
        ),
        (
            'facilities_manager',
            'Facilities Manager',
            'Configures campus locations and manages service requests, work orders, inspections, and closure.',
            ARRAY['facilities:view', 'facilities:request', 'facilities:operate', 'facilities:manage']::TEXT[]
        )
) AS seed(key, name, description, permissions)
WHERE NOT EXISTS (
    SELECT 1 FROM roles AS role
    WHERE role.tenant_id = tenant.id AND role.key = seed.key AND role.deleted_at IS NULL
);

UPDATE roles
SET permissions = ARRAY(
        SELECT DISTINCT permission
        FROM UNNEST(permissions || ARRAY['facilities:view', 'facilities:request']::TEXT[])
            AS expanded(permission)
        ORDER BY permission
    ),
    updated_at = NOW()
WHERE key IN ('teacher', 'staff_member') AND deleted_at IS NULL;

INSERT INTO role_record_scope_grants (tenant_id, role_id, scope_family, scope_kind)
SELECT role.tenant_id, role.id, scope.scope_family, scope.scope_kind
FROM roles AS role
CROSS JOIN LATERAL (
    VALUES
        (
            'facilities.requests',
            CASE
                WHEN role.key IN ('facilities_manager', 'facilities_officer') THEN 'campus'
                ELSE 'self'
            END
        ),
        (
            'facilities.work_orders',
            CASE
                WHEN role.key = 'facilities_manager' THEN 'campus'
                WHEN role.key = 'facilities_officer' THEN 'assigned'
                ELSE NULL
            END
        )
) AS scope(scope_family, scope_kind)
WHERE role.key IN ('facilities_manager', 'facilities_officer', 'teacher', 'staff_member')
  AND role.deleted_at IS NULL AND scope.scope_kind IS NOT NULL
ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
    WHERE deleted_at IS NULL DO NOTHING;

CREATE OR REPLACE FUNCTION provision_facilities_role_scopes()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.key IN ('facilities_manager', 'facilities_officer', 'teacher', 'staff_member') THEN
        INSERT INTO role_record_scope_grants (
            tenant_id, role_id, scope_family, scope_kind
        ) VALUES (
            NEW.tenant_id, NEW.id, 'facilities.requests',
            CASE
                WHEN NEW.key IN ('facilities_manager', 'facilities_officer') THEN 'campus'
                ELSE 'self'
            END
        )
        ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
            WHERE deleted_at IS NULL DO NOTHING;
    END IF;
    IF NEW.key = 'facilities_manager' THEN
        INSERT INTO role_record_scope_grants (
            tenant_id, role_id, scope_family, scope_kind
        ) VALUES (NEW.tenant_id, NEW.id, 'facilities.work_orders', 'campus')
        ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
            WHERE deleted_at IS NULL DO NOTHING;
    ELSIF NEW.key = 'facilities_officer' THEN
        INSERT INTO role_record_scope_grants (
            tenant_id, role_id, scope_family, scope_kind
        ) VALUES (NEW.tenant_id, NEW.id, 'facilities.work_orders', 'assigned')
        ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
            WHERE deleted_at IS NULL DO NOTHING;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS provision_facilities_role_scopes_after_insert ON roles;
CREATE TRIGGER provision_facilities_role_scopes_after_insert
    AFTER INSERT ON roles
    FOR EACH ROW EXECUTE FUNCTION provision_facilities_role_scopes();

CREATE OR REPLACE FUNCTION provision_new_tenant_facilities_access()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
    VALUES
        (
            NEW.id, 'facilities_officer', 'Facilities Officer',
            'Reviews campus service requests and carries out assigned facilities work orders.',
            ARRAY['facilities:view', 'facilities:request', 'facilities:operate']::TEXT[], TRUE
        ),
        (
            NEW.id, 'facilities_manager', 'Facilities Manager',
            'Configures campus locations and manages service requests, work orders, inspections, and closure.',
            ARRAY['facilities:view', 'facilities:request', 'facilities:operate', 'facilities:manage']::TEXT[], TRUE
        );

    UPDATE roles
    SET permissions = ARRAY(
            SELECT DISTINCT permission
            FROM UNNEST(permissions || ARRAY['facilities:view', 'facilities:request']::TEXT[])
                AS expanded(permission)
            ORDER BY permission
        ),
        updated_at = NOW()
    WHERE tenant_id = NEW.id AND key IN ('teacher', 'staff_member') AND deleted_at IS NULL;

    INSERT INTO role_record_scope_grants (tenant_id, role_id, scope_family, scope_kind)
    SELECT role.tenant_id, role.id, scope.scope_family, scope.scope_kind
    FROM roles AS role
    CROSS JOIN LATERAL (
        VALUES
            (
                'facilities.requests',
                CASE
                    WHEN role.key IN ('facilities_manager', 'facilities_officer') THEN 'campus'
                    ELSE 'self'
                END
            ),
            (
                'facilities.work_orders',
                CASE
                    WHEN role.key = 'facilities_manager' THEN 'campus'
                    WHEN role.key = 'facilities_officer' THEN 'assigned'
                    ELSE NULL
                END
            )
    ) AS scope(scope_family, scope_kind)
    WHERE role.tenant_id = NEW.id
      AND role.key IN ('facilities_manager', 'facilities_officer', 'teacher', 'staff_member')
      AND role.deleted_at IS NULL AND scope.scope_kind IS NOT NULL
    ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
        WHERE deleted_at IS NULL DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_provision_new_tenant_facilities_access ON tenants;
CREATE TRIGGER zz_provision_new_tenant_facilities_access
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION provision_new_tenant_facilities_access();

DROP TRIGGER IF EXISTS ev_facility_locations ON facility_locations;
CREATE TRIGGER ev_facility_locations
    AFTER INSERT OR UPDATE OR DELETE ON facility_locations
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_facility_service_requests ON facility_service_requests;
CREATE TRIGGER ev_facility_service_requests
    AFTER INSERT OR UPDATE OR DELETE ON facility_service_requests
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_facility_work_orders ON facility_work_orders;
CREATE TRIGGER ev_facility_work_orders
    AFTER INSERT OR UPDATE OR DELETE ON facility_work_orders
    FOR EACH ROW EXECUTE FUNCTION log_event();
