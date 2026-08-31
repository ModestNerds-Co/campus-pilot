-- Assets-owned department stock requests and fulfilment linkage.
-- HR remains authoritative for employees and departments. Approval records
-- quantities but never reserves stock; only fulfilment posts an issue.

CREATE TABLE IF NOT EXISTS assets_inventory_stock_request_sequences (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id),
    last_number BIGINT NOT NULL DEFAULT 0 CHECK (last_number BETWEEN 0 AND 999999),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

DROP TRIGGER IF EXISTS update_assets_inventory_stock_request_sequences_updated_at
    ON assets_inventory_stock_request_sequences;
CREATE TRIGGER update_assets_inventory_stock_request_sequences_updated_at
    BEFORE UPDATE ON assets_inventory_stock_request_sequences
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS assets_inventory_stock_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    request_number TEXT NOT NULL CHECK (request_number ~ '^SRQ-[0-9]{6}$'),
    requester_employee_id UUID NOT NULL,
    department_id UUID NOT NULL,
    purpose TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(purpose)) BETWEEN 1 AND 2000),
    needed_by DATE,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (
        status IN (
            'draft', 'submitted', 'approved', 'rejected', 'cancelled',
            'partially_fulfilled', 'fulfilled', 'closed'
        )
    ),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    idempotency_key TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(idempotency_key)) BETWEEN 1 AND 200),
    create_request_fingerprint TEXT NOT NULL
        CHECK (create_request_fingerprint ~ '^[0-9a-f]{64}$'),
    created_by UUID NOT NULL,
    submitted_by UUID,
    submitted_at TIMESTAMPTZ,
    decided_by UUID,
    decided_at TIMESTAMPTZ,
    decision_note TEXT CHECK (
        decision_note IS NULL OR CHAR_LENGTH(BTRIM(decision_note)) BETWEEN 1 AND 1000
    ),
    cancelled_by UUID,
    cancelled_at TIMESTAMPTZ,
    cancellation_note TEXT CHECK (
        cancellation_note IS NULL OR CHAR_LENGTH(BTRIM(cancellation_note)) BETWEEN 1 AND 1000
    ),
    closed_by UUID,
    closed_at TIMESTAMPTZ,
    closure_note TEXT CHECK (
        closure_note IS NULL OR CHAR_LENGTH(BTRIM(closure_note)) BETWEEN 1 AND 1000
    ),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CONSTRAINT assets_inventory_stock_requests_requester_tenant_fkey
        FOREIGN KEY (requester_employee_id, tenant_id) REFERENCES employees(id, tenant_id),
    CONSTRAINT assets_inventory_stock_requests_department_tenant_fkey
        FOREIGN KEY (department_id, tenant_id) REFERENCES departments(id, tenant_id),
    CONSTRAINT assets_inventory_stock_requests_created_by_tenant_fkey
        FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT assets_inventory_stock_requests_submitted_by_tenant_fkey
        FOREIGN KEY (submitted_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT assets_inventory_stock_requests_decided_by_tenant_fkey
        FOREIGN KEY (decided_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT assets_inventory_stock_requests_cancelled_by_tenant_fkey
        FOREIGN KEY (cancelled_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT assets_inventory_stock_requests_closed_by_tenant_fkey
        FOREIGN KEY (closed_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT assets_inventory_stock_requests_submission_check CHECK (
        (status = 'draft' AND submitted_by IS NULL AND submitted_at IS NULL)
        OR (status <> 'draft' AND submitted_by IS NOT NULL AND submitted_at IS NOT NULL)
    ),
    CONSTRAINT assets_inventory_stock_requests_decision_check CHECK (
        (
            status IN ('draft', 'submitted')
            AND decided_by IS NULL AND decided_at IS NULL AND decision_note IS NULL
        )
        OR (
            status IN ('approved', 'rejected', 'cancelled', 'partially_fulfilled', 'fulfilled', 'closed')
            AND (
                (status = 'cancelled' AND decided_by IS NULL AND decided_at IS NULL)
                OR (decided_by IS NOT NULL AND decided_at IS NOT NULL)
            )
        )
    ),
    CONSTRAINT assets_inventory_stock_requests_cancel_check CHECK (
        (status = 'cancelled' AND cancelled_by IS NOT NULL AND cancelled_at IS NOT NULL
            AND cancellation_note IS NOT NULL)
        OR (status <> 'cancelled' AND cancelled_by IS NULL AND cancelled_at IS NULL
            AND cancellation_note IS NULL)
    ),
    CONSTRAINT assets_inventory_stock_requests_close_check CHECK (
        (status = 'closed' AND closed_by IS NOT NULL AND closed_at IS NOT NULL)
        OR (status <> 'closed' AND closed_by IS NULL AND closed_at IS NULL
            AND closure_note IS NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_inventory_stock_requests_number
    ON assets_inventory_stock_requests(tenant_id, request_number);
CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_inventory_stock_requests_idempotency
    ON assets_inventory_stock_requests(tenant_id, idempotency_key);
CREATE INDEX IF NOT EXISTS idx_assets_inventory_stock_requests_worklist
    ON assets_inventory_stock_requests(tenant_id, status, created_at DESC)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_assets_inventory_stock_requests_requester
    ON assets_inventory_stock_requests(tenant_id, requester_employee_id, created_at DESC)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_assets_inventory_stock_requests_department
    ON assets_inventory_stock_requests(tenant_id, department_id, created_at DESC)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_assets_inventory_stock_requests_updated_at
    ON assets_inventory_stock_requests;
CREATE TRIGGER update_assets_inventory_stock_requests_updated_at
    BEFORE UPDATE ON assets_inventory_stock_requests
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS assets_inventory_stock_request_lines (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    request_id UUID NOT NULL,
    line_number INTEGER NOT NULL CHECK (line_number > 0),
    item_id UUID NOT NULL,
    requested_quantity_minor BIGINT NOT NULL CHECK (
        requested_quantity_minor BETWEEN 1 AND 9007199254740991
    ),
    approved_quantity_minor BIGINT CHECK (
        approved_quantity_minor BETWEEN 0 AND requested_quantity_minor
    ),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CONSTRAINT assets_inventory_stock_request_lines_parent_tenant_fkey
        FOREIGN KEY (request_id, tenant_id)
        REFERENCES assets_inventory_stock_requests(id, tenant_id),
    CONSTRAINT assets_inventory_stock_request_lines_item_tenant_fkey
        FOREIGN KEY (item_id, tenant_id)
        REFERENCES assets_inventory_items(id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_inventory_stock_request_lines_number
    ON assets_inventory_stock_request_lines(tenant_id, request_id, line_number)
    WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_inventory_stock_request_lines_item
    ON assets_inventory_stock_request_lines(tenant_id, request_id, item_id)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_assets_inventory_stock_request_lines_updated_at
    ON assets_inventory_stock_request_lines;
CREATE TRIGGER update_assets_inventory_stock_request_lines_updated_at
    BEFORE UPDATE ON assets_inventory_stock_request_lines
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS assets_inventory_stock_request_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    request_id UUID NOT NULL,
    event_type TEXT NOT NULL CHECK (
        event_type IN (
            'created', 'updated', 'deleted', 'submitted', 'approved', 'rejected',
            'cancelled', 'partially_fulfilled', 'fulfilled', 'closed'
        )
    ),
    from_status TEXT,
    to_status TEXT NOT NULL,
    request_version INTEGER NOT NULL CHECK (request_version > 0),
    actor_id UUID NOT NULL,
    note TEXT CHECK (note IS NULL OR CHAR_LENGTH(BTRIM(note)) BETWEEN 1 AND 1000),
    idempotency_key TEXT CHECK (
        idempotency_key IS NULL
        OR CHAR_LENGTH(BTRIM(idempotency_key)) BETWEEN 1 AND 200
    ),
    request_fingerprint TEXT CHECK (
        request_fingerprint IS NULL OR request_fingerprint ~ '^[0-9a-f]{64}$'
    ),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CONSTRAINT assets_inventory_stock_request_events_parent_tenant_fkey
        FOREIGN KEY (request_id, tenant_id)
        REFERENCES assets_inventory_stock_requests(id, tenant_id),
    CONSTRAINT assets_inventory_stock_request_events_actor_tenant_fkey
        FOREIGN KEY (actor_id, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT assets_inventory_stock_request_events_replay_check CHECK (
        (idempotency_key IS NULL AND request_fingerprint IS NULL)
        OR (idempotency_key IS NOT NULL AND request_fingerprint IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_inventory_stock_request_events_idempotency
    ON assets_inventory_stock_request_events(tenant_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_assets_inventory_stock_request_events_history
    ON assets_inventory_stock_request_events(tenant_id, request_id, created_at, id);

CREATE TABLE IF NOT EXISTS assets_inventory_stock_request_fulfilments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    request_id UUID NOT NULL,
    movement_id UUID NOT NULL,
    request_version INTEGER NOT NULL CHECK (request_version > 0),
    issued_by UUID NOT NULL,
    idempotency_key TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(idempotency_key)) BETWEEN 1 AND 200),
    create_request_fingerprint TEXT NOT NULL
        CHECK (create_request_fingerprint ~ '^[0-9a-f]{64}$'),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CONSTRAINT assets_inventory_stock_request_fulfilments_parent_tenant_fkey
        FOREIGN KEY (request_id, tenant_id)
        REFERENCES assets_inventory_stock_requests(id, tenant_id),
    CONSTRAINT assets_inventory_stock_request_fulfilments_movement_tenant_fkey
        FOREIGN KEY (movement_id, tenant_id)
        REFERENCES assets_inventory_stock_movements(id, tenant_id),
    CONSTRAINT assets_inventory_stock_request_fulfilments_issuer_tenant_fkey
        FOREIGN KEY (issued_by, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_inventory_stock_request_fulfilments_movement
    ON assets_inventory_stock_request_fulfilments(tenant_id, movement_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_inventory_stock_request_fulfilments_idempotency
    ON assets_inventory_stock_request_fulfilments(tenant_id, idempotency_key);
CREATE INDEX IF NOT EXISTS idx_assets_inventory_stock_request_fulfilments_history
    ON assets_inventory_stock_request_fulfilments(tenant_id, request_id, created_at, id);

CREATE TABLE IF NOT EXISTS assets_inventory_stock_request_fulfilment_lines (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    fulfilment_id UUID NOT NULL,
    request_line_id UUID NOT NULL,
    movement_line_id UUID NOT NULL,
    quantity_minor BIGINT NOT NULL CHECK (quantity_minor BETWEEN 1 AND 9007199254740991),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CONSTRAINT assets_inventory_stock_request_fulfilment_lines_parent_tenant_fkey
        FOREIGN KEY (fulfilment_id, tenant_id)
        REFERENCES assets_inventory_stock_request_fulfilments(id, tenant_id),
    CONSTRAINT assets_inventory_stock_request_fulfilment_lines_request_tenant_fkey
        FOREIGN KEY (request_line_id, tenant_id)
        REFERENCES assets_inventory_stock_request_lines(id, tenant_id),
    CONSTRAINT assets_inventory_stock_request_fulfilment_lines_movement_tenant_fkey
        FOREIGN KEY (movement_line_id, tenant_id)
        REFERENCES assets_inventory_stock_movement_lines(id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_inventory_stock_request_fulfilment_lines_movement
    ON assets_inventory_stock_request_fulfilment_lines(tenant_id, movement_line_id);
CREATE INDEX IF NOT EXISTS idx_assets_inventory_stock_request_fulfilment_lines_request
    ON assets_inventory_stock_request_fulfilment_lines(tenant_id, request_line_id);

CREATE OR REPLACE FUNCTION prevent_assets_inventory_stock_request_append_only_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Asset inventory stock request history is append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS assets_inventory_stock_request_events_append_only
    ON assets_inventory_stock_request_events;
CREATE TRIGGER assets_inventory_stock_request_events_append_only
    BEFORE UPDATE OR DELETE ON assets_inventory_stock_request_events
    FOR EACH ROW EXECUTE FUNCTION prevent_assets_inventory_stock_request_append_only_mutation();

DROP TRIGGER IF EXISTS assets_inventory_stock_request_fulfilments_append_only
    ON assets_inventory_stock_request_fulfilments;
CREATE TRIGGER assets_inventory_stock_request_fulfilments_append_only
    BEFORE UPDATE OR DELETE ON assets_inventory_stock_request_fulfilments
    FOR EACH ROW EXECUTE FUNCTION prevent_assets_inventory_stock_request_append_only_mutation();

DROP TRIGGER IF EXISTS assets_inventory_stock_request_fulfilment_lines_append_only
    ON assets_inventory_stock_request_fulfilment_lines;
CREATE TRIGGER assets_inventory_stock_request_fulfilment_lines_append_only
    BEFORE UPDATE OR DELETE ON assets_inventory_stock_request_fulfilment_lines
    FOR EACH ROW EXECUTE FUNCTION prevent_assets_inventory_stock_request_append_only_mutation();

CREATE OR REPLACE FUNCTION validate_assets_inventory_stock_request_sequence_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Asset inventory stock request sequence rows cannot be deleted';
    END IF;
    IF TG_OP = 'INSERT' THEN
        IF NEW.last_number IS DISTINCT FROM 1 OR NEW.deleted_at IS NOT NULL THEN
            RAISE EXCEPTION 'Asset inventory stock request sequence must begin at one';
        END IF;
        RETURN NEW;
    END IF;
    IF OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
        OR OLD.created_at IS DISTINCT FROM NEW.created_at
        OR OLD.deleted_at IS DISTINCT FROM NEW.deleted_at
        OR NEW.deleted_at IS NOT NULL
        OR NEW.last_number IS DISTINCT FROM OLD.last_number + 1 THEN
        RAISE EXCEPTION 'Asset inventory stock request sequence must advance by one';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS assets_inventory_stock_request_sequence_lifecycle_guard
    ON assets_inventory_stock_request_sequences;
CREATE TRIGGER assets_inventory_stock_request_sequence_lifecycle_guard
    BEFORE INSERT OR UPDATE OR DELETE ON assets_inventory_stock_request_sequences
    FOR EACH ROW EXECUTE FUNCTION validate_assets_inventory_stock_request_sequence_lifecycle();

CREATE OR REPLACE FUNCTION validate_assets_inventory_stock_request_lifecycle()
RETURNS TRIGGER AS $$
DECLARE
    allocated_number BIGINT;
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.status IS DISTINCT FROM 'draft' OR NEW.version IS DISTINCT FROM 1
            OR NEW.deleted_at IS NOT NULL THEN
            RAISE EXCEPTION 'Asset inventory stock requests must begin as draft at version one';
        END IF;
        SELECT last_number INTO allocated_number
          FROM assets_inventory_stock_request_sequences
         WHERE tenant_id = NEW.tenant_id AND deleted_at IS NULL
         FOR UPDATE;
        IF NOT FOUND OR NEW.request_number IS DISTINCT FROM
            'SRQ-' || LPAD(allocated_number::TEXT, 6, '0') THEN
            RAISE EXCEPTION 'Asset inventory stock request number must match the tenant sequence';
        END IF;
        RETURN NEW;
    END IF;

    IF OLD.id IS DISTINCT FROM NEW.id
        OR OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
        OR OLD.request_number IS DISTINCT FROM NEW.request_number
        OR OLD.idempotency_key IS DISTINCT FROM NEW.idempotency_key
        OR OLD.create_request_fingerprint IS DISTINCT FROM NEW.create_request_fingerprint
        OR OLD.created_by IS DISTINCT FROM NEW.created_by
        OR OLD.created_at IS DISTINCT FROM NEW.created_at THEN
        RAISE EXCEPTION 'Asset inventory stock request identity is immutable';
    END IF;
    IF NEW.version IS DISTINCT FROM OLD.version + 1 THEN
        RAISE EXCEPTION 'Asset inventory stock request version must advance by one';
    END IF;
    IF OLD.status <> 'draft' AND (
        OLD.requester_employee_id IS DISTINCT FROM NEW.requester_employee_id
        OR OLD.department_id IS DISTINCT FROM NEW.department_id
        OR OLD.purpose IS DISTINCT FROM NEW.purpose
        OR OLD.needed_by IS DISTINCT FROM NEW.needed_by
    ) THEN
        RAISE EXCEPTION 'Submitted stock request details are immutable';
    END IF;
    IF NOT (
        (OLD.status = 'draft' AND NEW.status IN ('draft', 'submitted'))
        OR (OLD.status = 'submitted' AND NEW.status IN ('approved', 'rejected', 'cancelled'))
        OR (OLD.status = 'approved' AND NEW.status IN ('cancelled', 'partially_fulfilled', 'fulfilled'))
        OR (OLD.status = 'partially_fulfilled' AND NEW.status IN ('partially_fulfilled', 'fulfilled', 'closed'))
    ) THEN
        RAISE EXCEPTION 'Asset inventory stock request status transition is invalid';
    END IF;
    IF NEW.deleted_at IS NOT NULL AND (OLD.status <> 'draft' OR NEW.status <> 'draft') THEN
        RAISE EXCEPTION 'Only draft stock requests can be removed';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS assets_inventory_stock_request_lifecycle_guard
    ON assets_inventory_stock_requests;
CREATE TRIGGER assets_inventory_stock_request_lifecycle_guard
    BEFORE INSERT OR UPDATE ON assets_inventory_stock_requests
    FOR EACH ROW EXECUTE FUNCTION validate_assets_inventory_stock_request_lifecycle();

CREATE OR REPLACE FUNCTION validate_assets_inventory_stock_request_line_lifecycle()
RETURNS TRIGGER AS $$
DECLARE
    parent_status TEXT;
BEGIN
    SELECT status INTO parent_status
      FROM assets_inventory_stock_requests
     WHERE tenant_id = COALESCE(NEW.tenant_id, OLD.tenant_id)
       AND id = COALESCE(NEW.request_id, OLD.request_id)
     FOR UPDATE;
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Asset inventory stock request lines cannot be hard deleted';
    END IF;
    IF TG_OP = 'INSERT' THEN
        IF parent_status IS DISTINCT FROM 'draft' OR NEW.approved_quantity_minor IS NOT NULL
            OR NEW.deleted_at IS NOT NULL THEN
            RAISE EXCEPTION 'Stock request lines can only be added to drafts';
        END IF;
        RETURN NEW;
    END IF;
    IF OLD.id IS DISTINCT FROM NEW.id OR OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
        OR OLD.request_id IS DISTINCT FROM NEW.request_id
        OR OLD.created_at IS DISTINCT FROM NEW.created_at THEN
        RAISE EXCEPTION 'Stock request line identity is immutable';
    END IF;
    IF parent_status = 'draft' THEN
        IF NEW.approved_quantity_minor IS NOT NULL THEN
            RAISE EXCEPTION 'Draft stock request lines cannot be approved';
        END IF;
    ELSIF OLD.approved_quantity_minor IS NULL
        AND NEW.approved_quantity_minor IS NOT NULL
        AND parent_status IN ('approved', 'rejected') THEN
        IF OLD.item_id IS DISTINCT FROM NEW.item_id
            OR OLD.requested_quantity_minor IS DISTINCT FROM NEW.requested_quantity_minor
            OR OLD.line_number IS DISTINCT FROM NEW.line_number
            OR OLD.deleted_at IS DISTINCT FROM NEW.deleted_at THEN
            RAISE EXCEPTION 'Stock request approval cannot rewrite request lines';
        END IF;
    ELSE
        RAISE EXCEPTION 'Submitted stock request lines are immutable';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS assets_inventory_stock_request_line_lifecycle_guard
    ON assets_inventory_stock_request_lines;
CREATE TRIGGER assets_inventory_stock_request_line_lifecycle_guard
    BEFORE INSERT OR UPDATE OR DELETE ON assets_inventory_stock_request_lines
    FOR EACH ROW EXECUTE FUNCTION validate_assets_inventory_stock_request_line_lifecycle();

-- Seed a purpose-specific role without changing any existing non-owner role.
INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
SELECT tenant.id, 'inventory_officer', 'Inventory Officer',
       'Manages inventory catalogues, department requests, approvals, and stock issues.',
       ARRAY[
           'assets_inventory:view', 'assets_inventory:create', 'assets_inventory:edit',
           'assets_inventory:delete', 'assets_inventory:request',
           'assets_inventory:approve', 'assets_inventory:issue'
       ]::TEXT[], TRUE
  FROM tenants AS tenant
 WHERE NOT EXISTS (
    SELECT 1 FROM roles AS role
     WHERE role.tenant_id = tenant.id AND role.key = 'inventory_officer'
       AND role.deleted_at IS NULL
 );

CREATE OR REPLACE FUNCTION provision_new_tenant_inventory_officer()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
    VALUES (
        NEW.id, 'inventory_officer', 'Inventory Officer',
        'Manages inventory catalogues, department requests, approvals, and stock issues.',
        ARRAY[
            'assets_inventory:view', 'assets_inventory:create', 'assets_inventory:edit',
            'assets_inventory:delete', 'assets_inventory:request',
            'assets_inventory:approve', 'assets_inventory:issue'
        ]::TEXT[], TRUE
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_provision_new_tenant_inventory_officer ON tenants;
CREATE TRIGGER zz_provision_new_tenant_inventory_officer
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION provision_new_tenant_inventory_officer();
