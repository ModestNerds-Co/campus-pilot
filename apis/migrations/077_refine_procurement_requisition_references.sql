-- Preserve historical requester and currency snapshots after draft creation.
-- Active owning-module references are rechecked on submission, while later
-- approval does not rewrite or reinterpret the submitted request.

CREATE OR REPLACE FUNCTION validate_procurement_requisition_references()
RETURNS TRIGGER AS $$
DECLARE
    employee_status TEXT;
    hr_employee_number TEXT;
    hr_employee_name TEXT;
    hr_employee_account UUID;
    finance_code TEXT;
    finance_minor_units SMALLINT;
    finance_status TEXT;
BEGIN
    SELECT employment_status, employee_number, display_name, account_id
      INTO employee_status, hr_employee_number, hr_employee_name, hr_employee_account
      FROM employees AS employee
     WHERE tenant_id = NEW.tenant_id AND id = NEW.requester_employee_id
       AND deleted_at IS NULL;
    SELECT code, minor_units, status
      INTO finance_code, finance_minor_units, finance_status
      FROM finance_currencies
     WHERE tenant_id = NEW.tenant_id AND id = NEW.currency_id AND deleted_at IS NULL;

    IF TG_OP = 'INSERT'
        OR OLD.requester_employee_id IS DISTINCT FROM NEW.requester_employee_id
        OR OLD.requester_account_id IS DISTINCT FROM NEW.requester_account_id
        OR OLD.requester_employee_number IS DISTINCT FROM NEW.requester_employee_number
        OR OLD.requester_name IS DISTINCT FROM NEW.requester_name
        OR OLD.currency_id IS DISTINCT FROM NEW.currency_id
        OR OLD.currency_code IS DISTINCT FROM NEW.currency_code
        OR OLD.currency_minor_units IS DISTINCT FROM NEW.currency_minor_units THEN
        IF employee_status IS DISTINCT FROM 'active' THEN
            RAISE EXCEPTION 'Requisitions require an active HR employee requester';
        END IF;
        IF NEW.requester_employee_number <> hr_employee_number
            OR NEW.requester_name <> hr_employee_name
            OR NEW.requester_account_id IS DISTINCT FROM hr_employee_account THEN
            RAISE EXCEPTION 'Requisition requester snapshots must match HR';
        END IF;
        IF finance_status IS DISTINCT FROM 'active' THEN
            RAISE EXCEPTION 'Requisitions require an active Finance currency';
        END IF;
        IF NEW.currency_code <> finance_code OR NEW.currency_minor_units <> finance_minor_units THEN
            RAISE EXCEPTION 'Requisition currency snapshots must match Finance';
        END IF;
    ELSIF OLD.status = 'draft' AND NEW.status = 'submitted' THEN
        IF employee_status IS DISTINCT FROM 'active' THEN
            RAISE EXCEPTION 'Requisitions require an active HR employee requester';
        END IF;
        IF finance_status IS DISTINCT FROM 'active' THEN
            RAISE EXCEPTION 'Requisitions require an active Finance currency';
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Once submitted, the owning HR and Finance records may continue to evolve,
-- but the requisition must retain the exact requester and currency snapshots
-- that were reviewed.
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
        OR OLD.requester_account_id IS DISTINCT FROM NEW.requester_account_id
        OR OLD.requester_employee_number IS DISTINCT FROM NEW.requester_employee_number
        OR OLD.requester_name IS DISTINCT FROM NEW.requester_name
        OR OLD.currency_id IS DISTINCT FROM NEW.currency_id
        OR OLD.currency_code IS DISTINCT FROM NEW.currency_code
        OR OLD.currency_minor_units IS DISTINCT FROM NEW.currency_minor_units
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

-- An idempotency key is never returned to the pool, even when a draft is
-- removed. This prevents a later request from acquiring an old command identity.
DROP INDEX IF EXISTS idx_procurement_suppliers_idempotency;
CREATE UNIQUE INDEX idx_procurement_suppliers_idempotency
    ON procurement_suppliers(tenant_id, idempotency_key);

DROP INDEX IF EXISTS idx_procurement_requisitions_idempotency;
CREATE UNIQUE INDEX idx_procurement_requisitions_idempotency
    ON procurement_requisitions(tenant_id, idempotency_key);

-- Qualify supplier columns so PL/pgSQL never treats snapshot variables as
-- ambiguous column references.
CREATE OR REPLACE FUNCTION validate_procurement_requisition_line()
RETURNS TRIGGER AS $$
DECLARE
    parent_status TEXT;
    matched_supplier_number TEXT;
    matched_supplier_name TEXT;
    supplier_status TEXT;
BEGIN
    SELECT status INTO parent_status
      FROM procurement_requisitions
     WHERE tenant_id = NEW.tenant_id AND id = NEW.requisition_id AND deleted_at IS NULL;
    IF parent_status IS DISTINCT FROM 'draft' THEN
        RAISE EXCEPTION 'Only draft requisition lines can change';
    END IF;
    IF NEW.preferred_supplier_id IS NOT NULL THEN
        SELECT supplier.supplier_number, supplier.legal_name, supplier.status
          INTO matched_supplier_number, matched_supplier_name, supplier_status
          FROM procurement_suppliers AS supplier
         WHERE supplier.tenant_id = NEW.tenant_id AND supplier.id = NEW.preferred_supplier_id
           AND supplier.deleted_at IS NULL;
        IF supplier_status IS DISTINCT FROM 'active' THEN
            RAISE EXCEPTION 'A preferred supplier must be active';
        END IF;
        IF NEW.preferred_supplier_number <> matched_supplier_number
            OR NEW.preferred_supplier_name <> matched_supplier_name THEN
            RAISE EXCEPTION 'Preferred supplier snapshots must match Procurement';
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
