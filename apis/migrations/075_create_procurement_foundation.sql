-- Procurement suppliers and requisitions.
--
-- HR remains authoritative for employees and Finance remains authoritative for
-- currencies. Procurement keeps immutable identity snapshots so historical
-- requisitions remain intelligible when either owning module changes later.

CREATE TABLE IF NOT EXISTS procurement_supplier_sequences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    last_number BIGINT NOT NULL DEFAULT 0 CHECK (last_number >= 0),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id)
);

DROP TRIGGER IF EXISTS update_procurement_supplier_sequences_updated_at
    ON procurement_supplier_sequences;
CREATE TRIGGER update_procurement_supplier_sequences_updated_at
    BEFORE UPDATE ON procurement_supplier_sequences
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS procurement_suppliers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    supplier_number TEXT NOT NULL CHECK (BTRIM(supplier_number) <> ''),
    legal_name TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(legal_name)) BETWEEN 1 AND 180),
    trading_name TEXT CHECK (
        trading_name IS NULL OR CHAR_LENGTH(BTRIM(trading_name)) BETWEEN 1 AND 180
    ),
    registration_number TEXT CHECK (
        registration_number IS NULL
        OR CHAR_LENGTH(BTRIM(registration_number)) BETWEEN 1 AND 100
    ),
    tax_number TEXT CHECK (
        tax_number IS NULL OR CHAR_LENGTH(BTRIM(tax_number)) BETWEEN 1 AND 100
    ),
    email TEXT CHECK (email IS NULL OR CHAR_LENGTH(BTRIM(email)) BETWEEN 3 AND 254),
    phone TEXT CHECK (phone IS NULL OR CHAR_LENGTH(BTRIM(phone)) BETWEEN 3 AND 50),
    address TEXT CHECK (
        address IS NULL OR CHAR_LENGTH(BTRIM(address)) BETWEEN 1 AND 1000
    ),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    idempotency_key TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(idempotency_key)) BETWEEN 1 AND 200),
    created_by UUID NOT NULL,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_procurement_suppliers_number
    ON procurement_suppliers(tenant_id, LOWER(supplier_number)) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_procurement_suppliers_idempotency
    ON procurement_suppliers(tenant_id, idempotency_key) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_procurement_suppliers_registration
    ON procurement_suppliers(tenant_id, LOWER(registration_number))
    WHERE deleted_at IS NULL AND registration_number IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_procurement_suppliers_directory
    ON procurement_suppliers(tenant_id, status, legal_name) WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_procurement_suppliers_updated_at ON procurement_suppliers;
CREATE TRIGGER update_procurement_suppliers_updated_at
    BEFORE UPDATE ON procurement_suppliers
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE OR REPLACE FUNCTION validate_procurement_supplier_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.supplier_number IS DISTINCT FROM NEW.supplier_number
        OR OLD.idempotency_key IS DISTINCT FROM NEW.idempotency_key
        OR OLD.created_by IS DISTINCT FROM NEW.created_by THEN
        RAISE EXCEPTION 'Supplier identity is fixed after creation';
    END IF;
    IF NEW.deleted_at IS NOT NULL AND OLD.deleted_at IS NULL AND NEW.status <> 'inactive' THEN
        RAISE EXCEPTION 'Only inactive suppliers can be removed';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS procurement_supplier_lifecycle_guard ON procurement_suppliers;
CREATE TRIGGER procurement_supplier_lifecycle_guard
    BEFORE UPDATE ON procurement_suppliers
    FOR EACH ROW EXECUTE FUNCTION validate_procurement_supplier_lifecycle();

CREATE TABLE IF NOT EXISTS procurement_requisition_sequences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    calendar_year INTEGER NOT NULL CHECK (calendar_year BETWEEN 2000 AND 9999),
    last_number BIGINT NOT NULL DEFAULT 0 CHECK (last_number >= 0),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, calendar_year)
);

DROP TRIGGER IF EXISTS update_procurement_requisition_sequences_updated_at
    ON procurement_requisition_sequences;
CREATE TRIGGER update_procurement_requisition_sequences_updated_at
    BEFORE UPDATE ON procurement_requisition_sequences
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS procurement_requisitions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    requisition_number TEXT NOT NULL CHECK (BTRIM(requisition_number) <> ''),
    requester_employee_id UUID NOT NULL,
    requester_account_id UUID,
    requester_employee_number TEXT NOT NULL CHECK (BTRIM(requester_employee_number) <> ''),
    requester_name TEXT NOT NULL CHECK (BTRIM(requester_name) <> ''),
    currency_id UUID NOT NULL,
    currency_code TEXT NOT NULL CHECK (currency_code ~ '^[A-Z]{3}$'),
    currency_minor_units SMALLINT NOT NULL CHECK (currency_minor_units BETWEEN 0 AND 4),
    title TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(title)) BETWEEN 1 AND 180),
    purpose TEXT CHECK (purpose IS NULL OR CHAR_LENGTH(BTRIM(purpose)) BETWEEN 1 AND 2000),
    needed_by DATE,
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'submitted', 'approved', 'rejected', 'cancelled')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    idempotency_key TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(idempotency_key)) BETWEEN 1 AND 200),
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
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (requester_employee_id, tenant_id) REFERENCES employees(id, tenant_id),
    FOREIGN KEY (requester_account_id, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (currency_id, tenant_id) REFERENCES finance_currencies(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (submitted_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (decided_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (cancelled_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'draft' AND submitted_by IS NULL AND submitted_at IS NULL
            AND decided_by IS NULL AND decided_at IS NULL AND decision_note IS NULL
            AND cancelled_by IS NULL AND cancelled_at IS NULL AND cancellation_note IS NULL)
        OR (status = 'submitted' AND submitted_by IS NOT NULL AND submitted_at IS NOT NULL
            AND decided_by IS NULL AND decided_at IS NULL AND decision_note IS NULL
            AND cancelled_by IS NULL AND cancelled_at IS NULL AND cancellation_note IS NULL)
        OR (status IN ('approved', 'rejected')
            AND submitted_by IS NOT NULL AND submitted_at IS NOT NULL
            AND decided_by IS NOT NULL AND decided_at IS NOT NULL
            AND cancelled_by IS NULL AND cancelled_at IS NULL AND cancellation_note IS NULL)
        OR (status = 'cancelled' AND cancelled_by IS NOT NULL AND cancelled_at IS NOT NULL
            AND decided_by IS NULL AND decided_at IS NULL AND decision_note IS NULL)
    ),
    CHECK (status <> 'rejected' OR decision_note IS NOT NULL)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_procurement_requisitions_number
    ON procurement_requisitions(tenant_id, LOWER(requisition_number)) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_procurement_requisitions_idempotency
    ON procurement_requisitions(tenant_id, idempotency_key) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_procurement_requisitions_worklist
    ON procurement_requisitions(tenant_id, status, created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_procurement_requisitions_requester
    ON procurement_requisitions(tenant_id, requester_employee_id, created_at DESC)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_procurement_requisitions_updated_at ON procurement_requisitions;
CREATE TRIGGER update_procurement_requisitions_updated_at
    BEFORE UPDATE ON procurement_requisitions
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS procurement_requisition_lines (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    requisition_id UUID NOT NULL,
    line_number INTEGER NOT NULL CHECK (line_number > 0),
    description TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(description)) BETWEEN 1 AND 500),
    quantity INTEGER NOT NULL CHECK (quantity BETWEEN 1 AND 1000000000),
    unit_label TEXT CHECK (
        unit_label IS NULL OR CHAR_LENGTH(BTRIM(unit_label)) BETWEEN 1 AND 40
    ),
    estimated_unit_amount_minor BIGINT NOT NULL
        CHECK (estimated_unit_amount_minor BETWEEN 0 AND 9000000000000000),
    estimated_line_amount_minor BIGINT NOT NULL
        CHECK (estimated_line_amount_minor BETWEEN 0 AND 9000000000000000),
    preferred_supplier_id UUID,
    preferred_supplier_number TEXT,
    preferred_supplier_name TEXT,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (requisition_id, tenant_id)
        REFERENCES procurement_requisitions(id, tenant_id),
    FOREIGN KEY (preferred_supplier_id, tenant_id)
        REFERENCES procurement_suppliers(id, tenant_id),
    UNIQUE (tenant_id, requisition_id, line_number),
    CHECK (
        estimated_line_amount_minor = quantity::BIGINT * estimated_unit_amount_minor
    ),
    CHECK (
        (preferred_supplier_id IS NULL AND preferred_supplier_number IS NULL
            AND preferred_supplier_name IS NULL)
        OR (preferred_supplier_id IS NOT NULL AND preferred_supplier_number IS NOT NULL
            AND preferred_supplier_name IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_procurement_requisition_lines_parent
    ON procurement_requisition_lines(tenant_id, requisition_id, line_number)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_procurement_requisition_lines_updated_at
    ON procurement_requisition_lines;
CREATE TRIGGER update_procurement_requisition_lines_updated_at
    BEFORE UPDATE ON procurement_requisition_lines
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE OR REPLACE FUNCTION validate_procurement_requisition_references()
RETURNS TRIGGER AS $$
DECLARE
    employee_status TEXT;
    employee_number TEXT;
    employee_name TEXT;
    employee_account UUID;
    finance_code TEXT;
    finance_minor_units SMALLINT;
    finance_status TEXT;
BEGIN
    SELECT employment_status, employee_number, display_name, account_id
      INTO employee_status, employee_number, employee_name, employee_account
      FROM employees
     WHERE tenant_id = NEW.tenant_id AND id = NEW.requester_employee_id
       AND deleted_at IS NULL;
    IF employee_status IS DISTINCT FROM 'active' THEN
        RAISE EXCEPTION 'Requisitions require an active HR employee requester';
    END IF;
    IF NEW.requester_employee_number <> employee_number
        OR NEW.requester_name <> employee_name
        OR NEW.requester_account_id IS DISTINCT FROM employee_account THEN
        RAISE EXCEPTION 'Requisition requester snapshots must match HR';
    END IF;

    SELECT code, minor_units, status
      INTO finance_code, finance_minor_units, finance_status
      FROM finance_currencies
     WHERE tenant_id = NEW.tenant_id AND id = NEW.currency_id AND deleted_at IS NULL;
    IF finance_status IS DISTINCT FROM 'active' THEN
        RAISE EXCEPTION 'Requisitions require an active Finance currency';
    END IF;
    IF NEW.currency_code <> finance_code OR NEW.currency_minor_units <> finance_minor_units THEN
        RAISE EXCEPTION 'Requisition currency snapshots must match Finance';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS procurement_requisition_reference_guard ON procurement_requisitions;
CREATE TRIGGER procurement_requisition_reference_guard
    BEFORE INSERT OR UPDATE OF requester_employee_id, requester_account_id,
        requester_employee_number, requester_name, currency_id, currency_code,
        currency_minor_units, status
    ON procurement_requisitions
    FOR EACH ROW EXECUTE FUNCTION validate_procurement_requisition_references();

CREATE OR REPLACE FUNCTION validate_procurement_requisition_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.requisition_number IS DISTINCT FROM NEW.requisition_number
        OR OLD.idempotency_key IS DISTINCT FROM NEW.idempotency_key
        OR OLD.created_by IS DISTINCT FROM NEW.created_by THEN
        RAISE EXCEPTION 'Requisition identity is fixed after creation';
    END IF;
    IF OLD.status IN ('approved', 'rejected', 'cancelled') THEN
        RAISE EXCEPTION 'A completed requisition is immutable';
    END IF;
    IF OLD.status = 'submitted' AND (
        OLD.requester_employee_id IS DISTINCT FROM NEW.requester_employee_id
        OR OLD.currency_id IS DISTINCT FROM NEW.currency_id
        OR OLD.title IS DISTINCT FROM NEW.title
        OR OLD.purpose IS DISTINCT FROM NEW.purpose
        OR OLD.needed_by IS DISTINCT FROM NEW.needed_by
    ) THEN
        RAISE EXCEPTION 'Submitted requisition details are immutable';
    END IF;
    IF OLD.status = 'draft' AND NEW.status NOT IN ('draft', 'submitted', 'cancelled') THEN
        RAISE EXCEPTION 'Draft requisitions can only be submitted or cancelled';
    END IF;
    IF OLD.status = 'submitted' AND NEW.status NOT IN (
        'submitted', 'approved', 'rejected', 'cancelled'
    ) THEN
        RAISE EXCEPTION 'Submitted requisition transition is invalid';
    END IF;
    IF NEW.deleted_at IS NOT NULL AND OLD.deleted_at IS NULL AND OLD.status <> 'draft' THEN
        RAISE EXCEPTION 'Only draft requisitions can be removed';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS procurement_requisition_lifecycle_guard ON procurement_requisitions;
CREATE TRIGGER procurement_requisition_lifecycle_guard
    BEFORE UPDATE ON procurement_requisitions
    FOR EACH ROW EXECUTE FUNCTION validate_procurement_requisition_lifecycle();

CREATE OR REPLACE FUNCTION validate_procurement_requisition_line()
RETURNS TRIGGER AS $$
DECLARE
    parent_status TEXT;
    supplier_number TEXT;
    supplier_name TEXT;
    supplier_status TEXT;
BEGIN
    SELECT status INTO parent_status
      FROM procurement_requisitions
     WHERE tenant_id = NEW.tenant_id AND id = NEW.requisition_id AND deleted_at IS NULL;
    IF parent_status IS DISTINCT FROM 'draft' THEN
        RAISE EXCEPTION 'Only draft requisition lines can change';
    END IF;
    IF NEW.preferred_supplier_id IS NOT NULL THEN
        SELECT supplier_number, legal_name, status
          INTO supplier_number, supplier_name, supplier_status
          FROM procurement_suppliers
         WHERE tenant_id = NEW.tenant_id AND id = NEW.preferred_supplier_id
           AND deleted_at IS NULL;
        IF supplier_status IS DISTINCT FROM 'active' THEN
            RAISE EXCEPTION 'A preferred supplier must be active';
        END IF;
        IF NEW.preferred_supplier_number <> supplier_number
            OR NEW.preferred_supplier_name <> supplier_name THEN
            RAISE EXCEPTION 'Preferred supplier snapshots must match Procurement';
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS procurement_requisition_line_guard
    ON procurement_requisition_lines;
CREATE TRIGGER procurement_requisition_line_guard
    BEFORE INSERT OR UPDATE ON procurement_requisition_lines
    FOR EACH ROW EXECUTE FUNCTION validate_procurement_requisition_line();

CREATE OR REPLACE FUNCTION validate_procurement_requisition_line_delete()
RETURNS TRIGGER AS $$
DECLARE
    parent_status TEXT;
BEGIN
    SELECT status INTO parent_status
      FROM procurement_requisitions
     WHERE tenant_id = OLD.tenant_id AND id = OLD.requisition_id AND deleted_at IS NULL;
    IF parent_status IS DISTINCT FROM 'draft' THEN
        RAISE EXCEPTION 'Only draft requisition lines can be removed';
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS procurement_requisition_line_delete_guard
    ON procurement_requisition_lines;
CREATE TRIGGER procurement_requisition_line_delete_guard
    BEFORE DELETE ON procurement_requisition_lines
    FOR EACH ROW EXECUTE FUNCTION validate_procurement_requisition_line_delete();
