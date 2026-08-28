-- Add exact scaled requisition quantities, supplier purchase orders, and goods receipts.
-- Purchase-order snapshots survive owning-record changes, while posted receipts are
-- immutable and may never cumulatively exceed the quantities that were issued.

ALTER TABLE procurement_requisition_lines
    ADD COLUMN IF NOT EXISTS quantity_minor BIGINT;
ALTER TABLE procurement_requisition_lines
    ADD COLUMN IF NOT EXISTS quantity_scale SMALLINT DEFAULT 0;

UPDATE procurement_requisition_lines
   SET quantity_minor = quantity::BIGINT,
       quantity_scale = 0
 WHERE quantity_minor IS NULL OR quantity_scale IS NULL;

CREATE OR REPLACE FUNCTION normalize_procurement_requisition_line_quantity()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.quantity_minor IS NULL THEN
        NEW.quantity_minor := NEW.quantity::BIGINT;
        NEW.quantity_scale := 0;
    END IF;
    IF NEW.quantity_scale IS DISTINCT FROM 0
        OR NEW.quantity_minor IS DISTINCT FROM NEW.quantity::BIGINT THEN
        RAISE EXCEPTION 'Legacy requisition quantities require scale zero';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS procurement_requisition_line_quantity_guard
    ON procurement_requisition_lines;
CREATE TRIGGER procurement_requisition_line_quantity_guard
    BEFORE INSERT OR UPDATE OF quantity, quantity_minor, quantity_scale
    ON procurement_requisition_lines
    FOR EACH ROW EXECUTE FUNCTION normalize_procurement_requisition_line_quantity();

ALTER TABLE procurement_requisition_lines
    ALTER COLUMN quantity_minor SET NOT NULL;
ALTER TABLE procurement_requisition_lines
    ALTER COLUMN quantity_scale SET DEFAULT 0;
ALTER TABLE procurement_requisition_lines
    ALTER COLUMN quantity_scale SET NOT NULL;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conname = 'procurement_requisition_lines_quantity_minor_check'
           AND conrelid = 'procurement_requisition_lines'::regclass
    ) THEN
        ALTER TABLE procurement_requisition_lines
            ADD CONSTRAINT procurement_requisition_lines_quantity_minor_check
            CHECK (quantity_minor BETWEEN 1 AND 1000000000);
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conname = 'procurement_requisition_lines_quantity_scale_check'
           AND conrelid = 'procurement_requisition_lines'::regclass
    ) THEN
        ALTER TABLE procurement_requisition_lines
            ADD CONSTRAINT procurement_requisition_lines_quantity_scale_check
            CHECK (quantity_scale = 0 AND quantity_minor = quantity::BIGINT);
    END IF;
END;
$$;

CREATE TABLE IF NOT EXISTS procurement_purchase_order_sequences (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id),
    last_number BIGINT NOT NULL DEFAULT 0 CHECK (last_number >= 0),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

DROP TRIGGER IF EXISTS update_procurement_purchase_order_sequences_updated_at
    ON procurement_purchase_order_sequences;
CREATE TRIGGER update_procurement_purchase_order_sequences_updated_at
    BEFORE UPDATE ON procurement_purchase_order_sequences
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS procurement_purchase_orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    purchase_order_number TEXT NOT NULL CHECK (purchase_order_number ~ '^PO-[0-9]{6}$'),
    requisition_id UUID NOT NULL,
    requisition_number TEXT NOT NULL CHECK (BTRIM(requisition_number) <> ''),
    requisition_title TEXT NOT NULL CHECK (
        CHAR_LENGTH(BTRIM(requisition_title)) BETWEEN 1 AND 180
    ),
    requisition_purpose TEXT CHECK (
        requisition_purpose IS NULL
        OR CHAR_LENGTH(BTRIM(requisition_purpose)) BETWEEN 1 AND 2000
    ),
    requisition_needed_by DATE,
    requester_employee_id UUID NOT NULL,
    requester_account_id UUID,
    requester_employee_number TEXT NOT NULL CHECK (BTRIM(requester_employee_number) <> ''),
    requester_name TEXT NOT NULL CHECK (BTRIM(requester_name) <> ''),
    supplier_id UUID NOT NULL,
    supplier_number TEXT NOT NULL CHECK (BTRIM(supplier_number) <> ''),
    supplier_name TEXT NOT NULL CHECK (BTRIM(supplier_name) <> ''),
    currency_id UUID NOT NULL,
    currency_code TEXT NOT NULL CHECK (currency_code ~ '^[A-Z]{3}$'),
    currency_minor_units SMALLINT NOT NULL CHECK (currency_minor_units BETWEEN 0 AND 4),
    delivery_date DATE,
    notes TEXT CHECK (notes IS NULL OR CHAR_LENGTH(BTRIM(notes)) BETWEEN 1 AND 2000),
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'issued', 'partially_received', 'received', 'cancelled')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    idempotency_key TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(idempotency_key)) BETWEEN 1 AND 200),
    created_by UUID NOT NULL,
    prepared_by UUID NOT NULL,
    issued_by UUID,
    issued_at TIMESTAMPTZ,
    cancelled_by UUID,
    cancelled_at TIMESTAMPTZ,
    cancellation_note TEXT CHECK (
        cancellation_note IS NULL OR CHAR_LENGTH(BTRIM(cancellation_note)) BETWEEN 1 AND 1000
    ),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (requisition_id, tenant_id)
        REFERENCES procurement_requisitions(id, tenant_id),
    FOREIGN KEY (requester_employee_id, tenant_id) REFERENCES employees(id, tenant_id),
    FOREIGN KEY (requester_account_id, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (supplier_id, tenant_id) REFERENCES procurement_suppliers(id, tenant_id),
    FOREIGN KEY (currency_id, tenant_id) REFERENCES finance_currencies(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT procurement_purchase_orders_prepared_by_tenant_fkey
        FOREIGN KEY (prepared_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (issued_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (cancelled_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'draft' AND issued_by IS NULL AND issued_at IS NULL
            AND cancelled_by IS NULL AND cancelled_at IS NULL AND cancellation_note IS NULL)
        OR (status IN ('issued', 'partially_received', 'received')
            AND issued_by IS NOT NULL AND issued_at IS NOT NULL
            AND cancelled_by IS NULL AND cancelled_at IS NULL AND cancellation_note IS NULL)
        OR (status = 'cancelled' AND cancelled_by IS NOT NULL AND cancelled_at IS NOT NULL)
    ),
    CONSTRAINT procurement_purchase_orders_distinct_issuer_check CHECK (
        issued_by IS NULL OR (issued_by <> created_by AND issued_by <> prepared_by)
    )
);

ALTER TABLE procurement_purchase_orders
    ADD COLUMN IF NOT EXISTS prepared_by UUID;
UPDATE procurement_purchase_orders
   SET prepared_by = created_by
 WHERE prepared_by IS NULL;
ALTER TABLE procurement_purchase_orders
    ALTER COLUMN prepared_by SET NOT NULL;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conname = 'procurement_purchase_orders_prepared_by_tenant_fkey'
           AND conrelid = 'procurement_purchase_orders'::regclass
    ) THEN
        ALTER TABLE procurement_purchase_orders
            ADD CONSTRAINT procurement_purchase_orders_prepared_by_tenant_fkey
            FOREIGN KEY (prepared_by, tenant_id) REFERENCES users(id, tenant_id);
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conname = 'procurement_purchase_orders_distinct_issuer_check'
           AND conrelid = 'procurement_purchase_orders'::regclass
    ) THEN
        ALTER TABLE procurement_purchase_orders
            ADD CONSTRAINT procurement_purchase_orders_distinct_issuer_check
            CHECK (
                issued_by IS NULL
                OR (issued_by <> created_by AND issued_by <> prepared_by)
            );
    END IF;
END;
$$;

CREATE UNIQUE INDEX IF NOT EXISTS idx_procurement_purchase_orders_number
    ON procurement_purchase_orders(tenant_id, purchase_order_number);
CREATE UNIQUE INDEX IF NOT EXISTS idx_procurement_purchase_orders_idempotency
    ON procurement_purchase_orders(tenant_id, idempotency_key);
CREATE INDEX IF NOT EXISTS idx_procurement_purchase_orders_worklist
    ON procurement_purchase_orders(tenant_id, status, created_at DESC)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_procurement_purchase_orders_requisition
    ON procurement_purchase_orders(tenant_id, requisition_id, created_at DESC)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_procurement_purchase_orders_supplier
    ON procurement_purchase_orders(tenant_id, supplier_id, created_at DESC)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_procurement_purchase_orders_updated_at
    ON procurement_purchase_orders;
CREATE TRIGGER update_procurement_purchase_orders_updated_at
    BEFORE UPDATE ON procurement_purchase_orders
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS procurement_purchase_order_lines (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    purchase_order_id UUID NOT NULL,
    line_number INTEGER NOT NULL CHECK (line_number > 0),
    requisition_line_id UUID NOT NULL,
    requisition_line_number INTEGER NOT NULL CHECK (requisition_line_number > 0),
    description TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(description)) BETWEEN 1 AND 500),
    unit_label TEXT CHECK (
        unit_label IS NULL OR CHAR_LENGTH(BTRIM(unit_label)) BETWEEN 1 AND 40
    ),
    requisition_quantity_minor BIGINT NOT NULL CHECK (requisition_quantity_minor > 0),
    quantity_minor BIGINT NOT NULL CHECK (quantity_minor > 0),
    quantity_scale SMALLINT NOT NULL CHECK (quantity_scale BETWEEN 0 AND 9),
    unit_amount_minor BIGINT NOT NULL
        CHECK (unit_amount_minor BETWEEN 0 AND 9000000000000000),
    line_amount_minor BIGINT NOT NULL
        CHECK (line_amount_minor BETWEEN 0 AND 9000000000000000),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (purchase_order_id, tenant_id)
        REFERENCES procurement_purchase_orders(id, tenant_id),
    FOREIGN KEY (requisition_line_id, tenant_id)
        REFERENCES procurement_requisition_lines(id, tenant_id),
    UNIQUE (tenant_id, purchase_order_id, line_number),
    UNIQUE (tenant_id, purchase_order_id, requisition_line_id),
    CHECK (
        line_amount_minor::NUMERIC * POWER(10::NUMERIC, quantity_scale)
            = quantity_minor::NUMERIC * unit_amount_minor::NUMERIC
    )
);

CREATE INDEX IF NOT EXISTS idx_procurement_purchase_order_lines_parent
    ON procurement_purchase_order_lines(tenant_id, purchase_order_id, line_number)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_procurement_purchase_order_lines_updated_at
    ON procurement_purchase_order_lines;
CREATE TRIGGER update_procurement_purchase_order_lines_updated_at
    BEFORE UPDATE ON procurement_purchase_order_lines
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS procurement_goods_receipt_sequences (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id),
    last_number BIGINT NOT NULL DEFAULT 0 CHECK (last_number >= 0),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

DROP TRIGGER IF EXISTS update_procurement_goods_receipt_sequences_updated_at
    ON procurement_goods_receipt_sequences;
CREATE TRIGGER update_procurement_goods_receipt_sequences_updated_at
    BEFORE UPDATE ON procurement_goods_receipt_sequences
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS procurement_goods_receipts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    goods_receipt_number TEXT NOT NULL CHECK (goods_receipt_number ~ '^GRN-[0-9]{6}$'),
    purchase_order_id UUID NOT NULL,
    purchase_order_number TEXT NOT NULL CHECK (BTRIM(purchase_order_number) <> ''),
    requisition_id UUID NOT NULL,
    requisition_number TEXT NOT NULL CHECK (BTRIM(requisition_number) <> ''),
    supplier_id UUID NOT NULL,
    supplier_number TEXT NOT NULL CHECK (BTRIM(supplier_number) <> ''),
    supplier_name TEXT NOT NULL CHECK (BTRIM(supplier_name) <> ''),
    currency_id UUID NOT NULL,
    currency_code TEXT NOT NULL CHECK (currency_code ~ '^[A-Z]{3}$'),
    currency_minor_units SMALLINT NOT NULL CHECK (currency_minor_units BETWEEN 0 AND 4),
    received_on DATE NOT NULL,
    delivery_reference TEXT CHECK (
        delivery_reference IS NULL OR CHAR_LENGTH(BTRIM(delivery_reference)) BETWEEN 1 AND 200
    ),
    notes TEXT CHECK (notes IS NULL OR CHAR_LENGTH(BTRIM(notes)) BETWEEN 1 AND 2000),
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'posted')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    idempotency_key TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(idempotency_key)) BETWEEN 1 AND 200),
    created_by UUID NOT NULL,
    prepared_by UUID NOT NULL,
    posted_by UUID,
    posted_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (purchase_order_id, tenant_id)
        REFERENCES procurement_purchase_orders(id, tenant_id),
    FOREIGN KEY (requisition_id, tenant_id)
        REFERENCES procurement_requisitions(id, tenant_id),
    FOREIGN KEY (supplier_id, tenant_id) REFERENCES procurement_suppliers(id, tenant_id),
    FOREIGN KEY (currency_id, tenant_id) REFERENCES finance_currencies(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT procurement_goods_receipts_prepared_by_tenant_fkey
        FOREIGN KEY (prepared_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (posted_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'draft' AND posted_by IS NULL AND posted_at IS NULL)
        OR (status = 'posted' AND posted_by IS NOT NULL AND posted_at IS NOT NULL)
    ),
    CONSTRAINT procurement_goods_receipts_distinct_poster_check CHECK (
        posted_by IS NULL OR (posted_by <> created_by AND posted_by <> prepared_by)
    )
);

ALTER TABLE procurement_goods_receipts
    ADD COLUMN IF NOT EXISTS prepared_by UUID;
UPDATE procurement_goods_receipts
   SET prepared_by = created_by
 WHERE prepared_by IS NULL;
ALTER TABLE procurement_goods_receipts
    ALTER COLUMN prepared_by SET NOT NULL;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conname = 'procurement_goods_receipts_prepared_by_tenant_fkey'
           AND conrelid = 'procurement_goods_receipts'::regclass
    ) THEN
        ALTER TABLE procurement_goods_receipts
            ADD CONSTRAINT procurement_goods_receipts_prepared_by_tenant_fkey
            FOREIGN KEY (prepared_by, tenant_id) REFERENCES users(id, tenant_id);
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conname = 'procurement_goods_receipts_distinct_poster_check'
           AND conrelid = 'procurement_goods_receipts'::regclass
    ) THEN
        ALTER TABLE procurement_goods_receipts
            ADD CONSTRAINT procurement_goods_receipts_distinct_poster_check
            CHECK (
                posted_by IS NULL
                OR (posted_by <> created_by AND posted_by <> prepared_by)
            );
    END IF;
END;
$$;

CREATE UNIQUE INDEX IF NOT EXISTS idx_procurement_goods_receipts_number
    ON procurement_goods_receipts(tenant_id, goods_receipt_number);
CREATE UNIQUE INDEX IF NOT EXISTS idx_procurement_goods_receipts_idempotency
    ON procurement_goods_receipts(tenant_id, idempotency_key);
CREATE INDEX IF NOT EXISTS idx_procurement_goods_receipts_worklist
    ON procurement_goods_receipts(tenant_id, status, created_at DESC)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_procurement_goods_receipts_purchase_order
    ON procurement_goods_receipts(tenant_id, purchase_order_id, created_at DESC)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_procurement_goods_receipts_updated_at
    ON procurement_goods_receipts;
CREATE TRIGGER update_procurement_goods_receipts_updated_at
    BEFORE UPDATE ON procurement_goods_receipts
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS procurement_goods_receipt_lines (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    goods_receipt_id UUID NOT NULL,
    line_number INTEGER NOT NULL CHECK (line_number > 0),
    purchase_order_line_id UUID NOT NULL,
    purchase_order_line_number INTEGER NOT NULL CHECK (purchase_order_line_number > 0),
    requisition_line_id UUID NOT NULL,
    description TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(description)) BETWEEN 1 AND 500),
    unit_label TEXT CHECK (
        unit_label IS NULL OR CHAR_LENGTH(BTRIM(unit_label)) BETWEEN 1 AND 40
    ),
    quantity_minor BIGINT NOT NULL CHECK (quantity_minor > 0),
    quantity_scale SMALLINT NOT NULL CHECK (quantity_scale BETWEEN 0 AND 9),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (goods_receipt_id, tenant_id)
        REFERENCES procurement_goods_receipts(id, tenant_id),
    FOREIGN KEY (purchase_order_line_id, tenant_id)
        REFERENCES procurement_purchase_order_lines(id, tenant_id),
    FOREIGN KEY (requisition_line_id, tenant_id)
        REFERENCES procurement_requisition_lines(id, tenant_id),
    UNIQUE (tenant_id, goods_receipt_id, line_number),
    UNIQUE (tenant_id, goods_receipt_id, purchase_order_line_id)
);

CREATE INDEX IF NOT EXISTS idx_procurement_goods_receipt_lines_parent
    ON procurement_goods_receipt_lines(tenant_id, goods_receipt_id, line_number)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_procurement_goods_receipt_lines_order_line
    ON procurement_goods_receipt_lines(tenant_id, purchase_order_line_id)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_procurement_goods_receipt_lines_updated_at
    ON procurement_goods_receipt_lines;
CREATE TRIGGER update_procurement_goods_receipt_lines_updated_at
    BEFORE UPDATE ON procurement_goods_receipt_lines
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE OR REPLACE FUNCTION validate_procurement_purchase_order_reference()
RETURNS TRIGGER AS $$
DECLARE
    source_requisition procurement_requisitions%ROWTYPE;
    source_supplier procurement_suppliers%ROWTYPE;
BEGIN
    IF NEW.status IS DISTINCT FROM 'draft' OR NEW.version IS DISTINCT FROM 1 THEN
        RAISE EXCEPTION 'Purchase orders must begin in draft at version one';
    END IF;
    IF NEW.prepared_by IS DISTINCT FROM NEW.created_by THEN
        RAISE EXCEPTION 'Purchase order creator must be the initial preparer';
    END IF;
    SELECT * INTO source_requisition
      FROM procurement_requisitions
     WHERE tenant_id = NEW.tenant_id AND id = NEW.requisition_id
       AND deleted_at IS NULL
     FOR SHARE;
    IF source_requisition.status IS DISTINCT FROM 'approved' THEN
        RAISE EXCEPTION 'Purchase orders require an approved requisition';
    END IF;
    SELECT * INTO source_supplier
      FROM procurement_suppliers
     WHERE tenant_id = NEW.tenant_id AND id = NEW.supplier_id
       AND deleted_at IS NULL
     FOR SHARE;
    IF source_supplier.status IS DISTINCT FROM 'active' THEN
        RAISE EXCEPTION 'Purchase orders require an active supplier';
    END IF;
    IF NEW.requisition_number <> source_requisition.requisition_number
        OR NEW.requisition_title <> source_requisition.title
        OR NEW.requisition_purpose IS DISTINCT FROM source_requisition.purpose
        OR NEW.requisition_needed_by IS DISTINCT FROM source_requisition.needed_by
        OR NEW.requester_employee_id <> source_requisition.requester_employee_id
        OR NEW.requester_account_id IS DISTINCT FROM source_requisition.requester_account_id
        OR NEW.requester_employee_number <> source_requisition.requester_employee_number
        OR NEW.requester_name <> source_requisition.requester_name
        OR NEW.currency_id <> source_requisition.currency_id
        OR NEW.currency_code <> source_requisition.currency_code
        OR NEW.currency_minor_units <> source_requisition.currency_minor_units THEN
        RAISE EXCEPTION 'Purchase order requisition snapshots must match Procurement';
    END IF;
    IF NEW.supplier_number <> source_supplier.supplier_number
        OR NEW.supplier_name <> source_supplier.legal_name THEN
        RAISE EXCEPTION 'Purchase order supplier snapshots must match Procurement';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS procurement_purchase_order_reference_guard
    ON procurement_purchase_orders;
CREATE TRIGGER procurement_purchase_order_reference_guard
    BEFORE INSERT ON procurement_purchase_orders
    FOR EACH ROW EXECUTE FUNCTION validate_procurement_purchase_order_reference();

CREATE OR REPLACE FUNCTION validate_procurement_purchase_order_lifecycle()
RETURNS TRIGGER AS $$
DECLARE
    supplier_status TEXT;
    has_received BOOLEAN;
    fully_received BOOLEAN;
    exceeds_order BOOLEAN;
BEGIN
    IF OLD.purchase_order_number IS DISTINCT FROM NEW.purchase_order_number
        OR OLD.requisition_id IS DISTINCT FROM NEW.requisition_id
        OR OLD.requisition_number IS DISTINCT FROM NEW.requisition_number
        OR OLD.requisition_title IS DISTINCT FROM NEW.requisition_title
        OR OLD.requisition_purpose IS DISTINCT FROM NEW.requisition_purpose
        OR OLD.requisition_needed_by IS DISTINCT FROM NEW.requisition_needed_by
        OR OLD.requester_employee_id IS DISTINCT FROM NEW.requester_employee_id
        OR OLD.requester_account_id IS DISTINCT FROM NEW.requester_account_id
        OR OLD.requester_employee_number IS DISTINCT FROM NEW.requester_employee_number
        OR OLD.requester_name IS DISTINCT FROM NEW.requester_name
        OR OLD.supplier_id IS DISTINCT FROM NEW.supplier_id
        OR OLD.supplier_number IS DISTINCT FROM NEW.supplier_number
        OR OLD.supplier_name IS DISTINCT FROM NEW.supplier_name
        OR OLD.currency_id IS DISTINCT FROM NEW.currency_id
        OR OLD.currency_code IS DISTINCT FROM NEW.currency_code
        OR OLD.currency_minor_units IS DISTINCT FROM NEW.currency_minor_units
        OR OLD.idempotency_key IS DISTINCT FROM NEW.idempotency_key
        OR OLD.created_by IS DISTINCT FROM NEW.created_by THEN
        RAISE EXCEPTION 'Purchase order source snapshots are immutable';
    END IF;
    IF OLD.prepared_by IS DISTINCT FROM NEW.prepared_by
        AND (OLD.status <> 'draft' OR NEW.status <> 'draft') THEN
        RAISE EXCEPTION 'Purchase order preparer is immutable after draft';
    END IF;
    IF OLD.status IN ('received', 'cancelled') THEN
        RAISE EXCEPTION 'A completed purchase order is immutable';
    END IF;
    IF OLD.status <> 'draft' AND (
        OLD.delivery_date IS DISTINCT FROM NEW.delivery_date
        OR OLD.notes IS DISTINCT FROM NEW.notes
        OR OLD.issued_by IS DISTINCT FROM NEW.issued_by
        OR OLD.issued_at IS DISTINCT FROM NEW.issued_at
    ) THEN
        RAISE EXCEPTION 'Issued purchase order details are immutable';
    END IF;
    IF OLD.status = 'draft' AND NEW.status NOT IN ('draft', 'issued', 'cancelled') THEN
        RAISE EXCEPTION 'Draft purchase order transition is invalid';
    END IF;
    IF OLD.status = 'issued' AND NEW.status NOT IN (
        'issued', 'partially_received', 'received', 'cancelled'
    ) THEN
        RAISE EXCEPTION 'Issued purchase order transition is invalid';
    END IF;
    IF OLD.status = 'partially_received'
        AND NEW.status NOT IN ('partially_received', 'received') THEN
        RAISE EXCEPTION 'Partially received purchase order transition is invalid';
    END IF;
    IF OLD.status IS DISTINCT FROM NEW.status
        AND NEW.status IN ('partially_received', 'received') THEN
        SELECT COALESCE(BOOL_OR(COALESCE(received.quantity_minor, 0) > 0), FALSE),
               COALESCE(BOOL_AND(
                   COALESCE(received.quantity_minor, 0) = order_line.quantity_minor::NUMERIC
               ), FALSE),
               COALESCE(BOOL_OR(
                   COALESCE(received.quantity_minor, 0) > order_line.quantity_minor::NUMERIC
               ), FALSE)
          INTO has_received, fully_received, exceeds_order
          FROM procurement_purchase_order_lines AS order_line
          LEFT JOIN (
                SELECT receipt_line.purchase_order_line_id,
                       SUM(receipt_line.quantity_minor)::NUMERIC AS quantity_minor
                  FROM procurement_goods_receipt_lines AS receipt_line
                  JOIN procurement_goods_receipts AS receipt
                    ON receipt.id = receipt_line.goods_receipt_id
                   AND receipt.tenant_id = receipt_line.tenant_id
                 WHERE receipt.tenant_id = NEW.tenant_id
                   AND receipt.purchase_order_id = NEW.id
                   AND receipt.status = 'posted' AND receipt.deleted_at IS NULL
                   AND receipt_line.deleted_at IS NULL
                 GROUP BY receipt_line.purchase_order_line_id
          ) AS received ON received.purchase_order_line_id = order_line.id
         WHERE order_line.tenant_id = NEW.tenant_id
           AND order_line.purchase_order_id = NEW.id
           AND order_line.deleted_at IS NULL;
        IF NEW.status = 'partially_received'
            AND (NOT has_received OR fully_received OR exceeds_order) THEN
            RAISE EXCEPTION 'Purchase order partial receipt status requires posted partial receipts';
        END IF;
        IF NEW.status = 'received' AND (NOT fully_received OR exceeds_order) THEN
            RAISE EXCEPTION 'Purchase order received status requires fully posted receipts';
        END IF;
    END IF;
    IF OLD.status = 'draft' AND NEW.status = 'issued' THEN
        PERFORM id FROM procurement_requisitions
         WHERE tenant_id = NEW.tenant_id AND id = NEW.requisition_id
           AND status = 'approved' AND deleted_at IS NULL
         FOR UPDATE;
        IF NOT FOUND THEN
            RAISE EXCEPTION 'Purchase orders require an approved requisition when issued';
        END IF;
        SELECT status INTO supplier_status
          FROM procurement_suppliers
         WHERE tenant_id = NEW.tenant_id AND id = NEW.supplier_id
           AND deleted_at IS NULL;
        IF supplier_status IS DISTINCT FROM 'active' THEN
            RAISE EXCEPTION 'Purchase orders require an active supplier when issued';
        END IF;
        IF NEW.issued_by IS NULL
            OR NEW.issued_by = NEW.created_by
            OR NEW.issued_by = NEW.prepared_by THEN
            RAISE EXCEPTION 'A different actor must issue the purchase order';
        END IF;
        IF NOT EXISTS (
            SELECT 1 FROM procurement_purchase_order_lines
             WHERE tenant_id = NEW.tenant_id AND purchase_order_id = NEW.id
               AND deleted_at IS NULL
        ) THEN
            RAISE EXCEPTION 'A purchase order requires at least one line';
        END IF;
        IF EXISTS (
            SELECT 1
              FROM procurement_purchase_order_lines AS current_line
              JOIN procurement_requisition_lines AS requisition_line
                ON requisition_line.id = current_line.requisition_line_id
               AND requisition_line.tenant_id = current_line.tenant_id
              LEFT JOIN (
                    SELECT other_line.requisition_line_id,
                           SUM(other_line.quantity_minor)::NUMERIC AS quantity_minor
                      FROM procurement_purchase_order_lines AS other_line
                      JOIN procurement_purchase_orders AS other_order
                        ON other_order.id = other_line.purchase_order_id
                       AND other_order.tenant_id = other_line.tenant_id
                     WHERE other_order.tenant_id = NEW.tenant_id
                       AND other_order.requisition_id = NEW.requisition_id
                       AND other_order.id <> NEW.id
                       AND other_order.status IN ('issued', 'partially_received', 'received')
                       AND other_order.deleted_at IS NULL AND other_line.deleted_at IS NULL
                     GROUP BY other_line.requisition_line_id
              ) AS ordered ON ordered.requisition_line_id = current_line.requisition_line_id
             WHERE current_line.tenant_id = NEW.tenant_id
               AND current_line.purchase_order_id = NEW.id
               AND current_line.deleted_at IS NULL
               AND COALESCE(ordered.quantity_minor, 0) + current_line.quantity_minor::NUMERIC
                   > requisition_line.quantity_minor::NUMERIC
        ) THEN
            RAISE EXCEPTION 'Issued purchase order quantities cannot exceed the requisition';
        END IF;
    END IF;
    IF NEW.status = 'cancelled' AND OLD.status <> 'cancelled' AND EXISTS (
        SELECT 1 FROM procurement_goods_receipts
         WHERE tenant_id = NEW.tenant_id AND purchase_order_id = NEW.id
           AND deleted_at IS NULL
    ) THEN
        RAISE EXCEPTION 'A purchase order with receipts cannot be cancelled';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS procurement_purchase_order_lifecycle_guard
    ON procurement_purchase_orders;
CREATE TRIGGER procurement_purchase_order_lifecycle_guard
    BEFORE UPDATE ON procurement_purchase_orders
    FOR EACH ROW EXECUTE FUNCTION validate_procurement_purchase_order_lifecycle();

CREATE OR REPLACE FUNCTION validate_procurement_purchase_order_line()
RETURNS TRIGGER AS $$
DECLARE
    parent_order procurement_purchase_orders%ROWTYPE;
    source_line procurement_requisition_lines%ROWTYPE;
BEGIN
    SELECT * INTO parent_order
      FROM procurement_purchase_orders
     WHERE tenant_id = NEW.tenant_id AND id = NEW.purchase_order_id
       AND deleted_at IS NULL
     FOR UPDATE;
    IF parent_order.status IS DISTINCT FROM 'draft' THEN
        RAISE EXCEPTION 'Only draft purchase order lines can change';
    END IF;
    SELECT * INTO source_line
      FROM procurement_requisition_lines
     WHERE tenant_id = NEW.tenant_id AND id = NEW.requisition_line_id
       AND requisition_id = parent_order.requisition_id AND deleted_at IS NULL
     FOR SHARE;
    IF source_line.id IS NULL THEN
        RAISE EXCEPTION 'Purchase order lines must reference their source requisition';
    END IF;
    IF NEW.requisition_line_number <> source_line.line_number
        OR NEW.description <> source_line.description
        OR NEW.unit_label IS DISTINCT FROM source_line.unit_label
        OR NEW.requisition_quantity_minor <> source_line.quantity_minor
        OR NEW.quantity_scale <> source_line.quantity_scale THEN
        RAISE EXCEPTION 'Purchase order line snapshots must match the requisition';
    END IF;
    IF NEW.quantity_minor > source_line.quantity_minor THEN
        RAISE EXCEPTION 'Purchase order quantity cannot exceed the requisition quantity';
    END IF;
    IF TG_OP = 'UPDATE' AND (
        OLD.purchase_order_id IS DISTINCT FROM NEW.purchase_order_id
        OR OLD.requisition_line_id IS DISTINCT FROM NEW.requisition_line_id
        OR OLD.requisition_line_number IS DISTINCT FROM NEW.requisition_line_number
        OR OLD.description IS DISTINCT FROM NEW.description
        OR OLD.unit_label IS DISTINCT FROM NEW.unit_label
        OR OLD.requisition_quantity_minor IS DISTINCT FROM NEW.requisition_quantity_minor
        OR OLD.quantity_scale IS DISTINCT FROM NEW.quantity_scale
    ) THEN
        RAISE EXCEPTION 'Purchase order line references are immutable';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS procurement_purchase_order_line_guard
    ON procurement_purchase_order_lines;
CREATE TRIGGER procurement_purchase_order_line_guard
    BEFORE INSERT OR UPDATE ON procurement_purchase_order_lines
    FOR EACH ROW EXECUTE FUNCTION validate_procurement_purchase_order_line();

CREATE OR REPLACE FUNCTION validate_procurement_purchase_order_line_delete()
RETURNS TRIGGER AS $$
DECLARE
    parent_status TEXT;
BEGIN
    SELECT status INTO parent_status
      FROM procurement_purchase_orders
     WHERE tenant_id = OLD.tenant_id AND id = OLD.purchase_order_id
       AND deleted_at IS NULL
     FOR UPDATE;
    IF parent_status IS DISTINCT FROM 'draft' THEN
        RAISE EXCEPTION 'Only draft purchase order lines can be removed';
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS procurement_purchase_order_line_delete_guard
    ON procurement_purchase_order_lines;
CREATE TRIGGER procurement_purchase_order_line_delete_guard
    BEFORE DELETE ON procurement_purchase_order_lines
    FOR EACH ROW EXECUTE FUNCTION validate_procurement_purchase_order_line_delete();

CREATE OR REPLACE FUNCTION validate_procurement_goods_receipt_reference()
RETURNS TRIGGER AS $$
DECLARE
    source_order procurement_purchase_orders%ROWTYPE;
BEGIN
    IF NEW.status IS DISTINCT FROM 'draft' OR NEW.version IS DISTINCT FROM 1 THEN
        RAISE EXCEPTION 'Goods receipts must begin in draft at version one';
    END IF;
    IF NEW.prepared_by IS DISTINCT FROM NEW.created_by THEN
        RAISE EXCEPTION 'Goods receipt creator must be the initial preparer';
    END IF;
    SELECT * INTO source_order
      FROM procurement_purchase_orders
     WHERE tenant_id = NEW.tenant_id AND id = NEW.purchase_order_id
       AND deleted_at IS NULL
     FOR UPDATE;
    IF source_order.status IS DISTINCT FROM 'issued'
        AND source_order.status IS DISTINCT FROM 'partially_received' THEN
        RAISE EXCEPTION 'Goods receipts require an open issued purchase order';
    END IF;
    IF NEW.purchase_order_number <> source_order.purchase_order_number
        OR NEW.requisition_id <> source_order.requisition_id
        OR NEW.requisition_number <> source_order.requisition_number
        OR NEW.supplier_id <> source_order.supplier_id
        OR NEW.supplier_number <> source_order.supplier_number
        OR NEW.supplier_name <> source_order.supplier_name
        OR NEW.currency_id <> source_order.currency_id
        OR NEW.currency_code <> source_order.currency_code
        OR NEW.currency_minor_units <> source_order.currency_minor_units THEN
        RAISE EXCEPTION 'Goods receipt purchase order snapshots must match Procurement';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS procurement_goods_receipt_reference_guard
    ON procurement_goods_receipts;
CREATE TRIGGER procurement_goods_receipt_reference_guard
    BEFORE INSERT ON procurement_goods_receipts
    FOR EACH ROW EXECUTE FUNCTION validate_procurement_goods_receipt_reference();

CREATE OR REPLACE FUNCTION validate_procurement_goods_receipt_lifecycle()
RETURNS TRIGGER AS $$
DECLARE
    parent_status TEXT;
BEGIN
    IF OLD.goods_receipt_number IS DISTINCT FROM NEW.goods_receipt_number
        OR OLD.purchase_order_id IS DISTINCT FROM NEW.purchase_order_id
        OR OLD.purchase_order_number IS DISTINCT FROM NEW.purchase_order_number
        OR OLD.requisition_id IS DISTINCT FROM NEW.requisition_id
        OR OLD.requisition_number IS DISTINCT FROM NEW.requisition_number
        OR OLD.supplier_id IS DISTINCT FROM NEW.supplier_id
        OR OLD.supplier_number IS DISTINCT FROM NEW.supplier_number
        OR OLD.supplier_name IS DISTINCT FROM NEW.supplier_name
        OR OLD.currency_id IS DISTINCT FROM NEW.currency_id
        OR OLD.currency_code IS DISTINCT FROM NEW.currency_code
        OR OLD.currency_minor_units IS DISTINCT FROM NEW.currency_minor_units
        OR OLD.idempotency_key IS DISTINCT FROM NEW.idempotency_key
        OR OLD.created_by IS DISTINCT FROM NEW.created_by THEN
        RAISE EXCEPTION 'Goods receipt source snapshots are immutable';
    END IF;
    IF OLD.prepared_by IS DISTINCT FROM NEW.prepared_by
        AND (OLD.status <> 'draft' OR NEW.status <> 'draft') THEN
        RAISE EXCEPTION 'Goods receipt preparer is immutable after draft';
    END IF;
    IF OLD.status = 'posted' THEN
        RAISE EXCEPTION 'A posted goods receipt is immutable';
    END IF;
    IF OLD.status = 'draft' AND NEW.status NOT IN ('draft', 'posted') THEN
        RAISE EXCEPTION 'Draft goods receipt transition is invalid';
    END IF;
    IF OLD.status = 'draft' AND NEW.status = 'posted' THEN
        IF NEW.posted_by IS NULL
            OR NEW.posted_by = NEW.created_by
            OR NEW.posted_by = NEW.prepared_by THEN
            RAISE EXCEPTION 'A different actor must post the goods receipt';
        END IF;
        SELECT status INTO parent_status
          FROM procurement_purchase_orders
         WHERE tenant_id = NEW.tenant_id AND id = NEW.purchase_order_id
           AND deleted_at IS NULL
         FOR UPDATE;
        IF parent_status IS DISTINCT FROM 'issued'
            AND parent_status IS DISTINCT FROM 'partially_received' THEN
            RAISE EXCEPTION 'Goods receipts require an open issued purchase order';
        END IF;
        PERFORM id FROM procurement_purchase_order_lines
         WHERE tenant_id = NEW.tenant_id AND purchase_order_id = NEW.purchase_order_id
           AND deleted_at IS NULL
         ORDER BY id
         FOR UPDATE;
        IF NOT EXISTS (
            SELECT 1 FROM procurement_goods_receipt_lines
             WHERE tenant_id = NEW.tenant_id AND goods_receipt_id = NEW.id
               AND deleted_at IS NULL
        ) THEN
            RAISE EXCEPTION 'A goods receipt requires at least one line';
        END IF;
        IF EXISTS (
            SELECT 1
              FROM procurement_purchase_order_lines AS order_line
              LEFT JOIN (
                    SELECT receipt_line.purchase_order_line_id,
                           SUM(receipt_line.quantity_minor)::NUMERIC AS quantity_minor
                      FROM procurement_goods_receipt_lines AS receipt_line
                      JOIN procurement_goods_receipts AS receipt
                        ON receipt.id = receipt_line.goods_receipt_id
                       AND receipt.tenant_id = receipt_line.tenant_id
                     WHERE receipt.tenant_id = NEW.tenant_id
                       AND receipt.purchase_order_id = NEW.purchase_order_id
                       AND receipt.status = 'posted' AND receipt.deleted_at IS NULL
                       AND receipt_line.deleted_at IS NULL
                     GROUP BY receipt_line.purchase_order_line_id
              ) AS posted ON posted.purchase_order_line_id = order_line.id
              LEFT JOIN (
                    SELECT purchase_order_line_id,
                           SUM(quantity_minor)::NUMERIC AS quantity_minor
                      FROM procurement_goods_receipt_lines
                     WHERE tenant_id = NEW.tenant_id AND goods_receipt_id = NEW.id
                       AND deleted_at IS NULL
                     GROUP BY purchase_order_line_id
              ) AS current ON current.purchase_order_line_id = order_line.id
             WHERE order_line.tenant_id = NEW.tenant_id
               AND order_line.purchase_order_id = NEW.purchase_order_id
               AND order_line.deleted_at IS NULL
               AND COALESCE(posted.quantity_minor, 0)
                   + COALESCE(current.quantity_minor, 0) > order_line.quantity_minor::NUMERIC
        ) THEN
            RAISE EXCEPTION 'Posted receipt quantities cannot exceed the purchase order';
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS procurement_goods_receipt_lifecycle_guard
    ON procurement_goods_receipts;
CREATE TRIGGER procurement_goods_receipt_lifecycle_guard
    BEFORE UPDATE ON procurement_goods_receipts
    FOR EACH ROW EXECUTE FUNCTION validate_procurement_goods_receipt_lifecycle();

CREATE OR REPLACE FUNCTION sync_procurement_purchase_order_receipt_status()
RETURNS TRIGGER AS $$
DECLARE
    fully_received BOOLEAN;
BEGIN
    SELECT BOOL_AND(COALESCE(received.quantity_minor, 0) = order_line.quantity_minor::NUMERIC)
      INTO fully_received
      FROM procurement_purchase_order_lines AS order_line
      LEFT JOIN (
            SELECT receipt_line.purchase_order_line_id,
                   SUM(receipt_line.quantity_minor)::NUMERIC AS quantity_minor
              FROM procurement_goods_receipt_lines AS receipt_line
              JOIN procurement_goods_receipts AS receipt
                ON receipt.id = receipt_line.goods_receipt_id
               AND receipt.tenant_id = receipt_line.tenant_id
             WHERE receipt.tenant_id = NEW.tenant_id
               AND receipt.purchase_order_id = NEW.purchase_order_id
               AND receipt.status = 'posted' AND receipt.deleted_at IS NULL
               AND receipt_line.deleted_at IS NULL
             GROUP BY receipt_line.purchase_order_line_id
      ) AS received ON received.purchase_order_line_id = order_line.id
     WHERE order_line.tenant_id = NEW.tenant_id
       AND order_line.purchase_order_id = NEW.purchase_order_id
       AND order_line.deleted_at IS NULL;

    UPDATE procurement_purchase_orders
       SET status = CASE WHEN fully_received THEN 'received' ELSE 'partially_received' END,
           version = version + 1
     WHERE tenant_id = NEW.tenant_id AND id = NEW.purchase_order_id
       AND status IN ('issued', 'partially_received') AND deleted_at IS NULL;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Goods receipt could not update its purchase order status';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS procurement_goods_receipt_status_sync
    ON procurement_goods_receipts;
CREATE TRIGGER procurement_goods_receipt_status_sync
    AFTER UPDATE OF status ON procurement_goods_receipts
    FOR EACH ROW
    WHEN (OLD.status = 'draft' AND NEW.status = 'posted')
    EXECUTE FUNCTION sync_procurement_purchase_order_receipt_status();

CREATE OR REPLACE FUNCTION validate_procurement_goods_receipt_line()
RETURNS TRIGGER AS $$
DECLARE
    parent_receipt procurement_goods_receipts%ROWTYPE;
    source_line procurement_purchase_order_lines%ROWTYPE;
BEGIN
    SELECT * INTO parent_receipt
      FROM procurement_goods_receipts
     WHERE tenant_id = NEW.tenant_id AND id = NEW.goods_receipt_id
       AND deleted_at IS NULL
     FOR UPDATE;
    IF parent_receipt.status IS DISTINCT FROM 'draft' THEN
        RAISE EXCEPTION 'Only draft goods receipt lines can change';
    END IF;
    SELECT * INTO source_line
      FROM procurement_purchase_order_lines
     WHERE tenant_id = NEW.tenant_id AND id = NEW.purchase_order_line_id
       AND purchase_order_id = parent_receipt.purchase_order_id
       AND deleted_at IS NULL
     FOR SHARE;
    IF source_line.id IS NULL THEN
        RAISE EXCEPTION 'Goods receipt lines must reference their purchase order';
    END IF;
    IF NEW.purchase_order_line_number <> source_line.line_number
        OR NEW.requisition_line_id <> source_line.requisition_line_id
        OR NEW.description <> source_line.description
        OR NEW.unit_label IS DISTINCT FROM source_line.unit_label
        OR NEW.quantity_scale <> source_line.quantity_scale THEN
        RAISE EXCEPTION 'Goods receipt line snapshots must match the purchase order';
    END IF;
    IF TG_OP = 'UPDATE' AND (
        OLD.goods_receipt_id IS DISTINCT FROM NEW.goods_receipt_id
        OR OLD.purchase_order_line_id IS DISTINCT FROM NEW.purchase_order_line_id
        OR OLD.purchase_order_line_number IS DISTINCT FROM NEW.purchase_order_line_number
        OR OLD.requisition_line_id IS DISTINCT FROM NEW.requisition_line_id
        OR OLD.description IS DISTINCT FROM NEW.description
        OR OLD.unit_label IS DISTINCT FROM NEW.unit_label
        OR OLD.quantity_scale IS DISTINCT FROM NEW.quantity_scale
    ) THEN
        RAISE EXCEPTION 'Goods receipt line references are immutable';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS procurement_goods_receipt_line_guard
    ON procurement_goods_receipt_lines;
CREATE TRIGGER procurement_goods_receipt_line_guard
    BEFORE INSERT OR UPDATE ON procurement_goods_receipt_lines
    FOR EACH ROW EXECUTE FUNCTION validate_procurement_goods_receipt_line();

CREATE OR REPLACE FUNCTION validate_procurement_goods_receipt_line_delete()
RETURNS TRIGGER AS $$
DECLARE
    parent_status TEXT;
BEGIN
    SELECT status INTO parent_status
      FROM procurement_goods_receipts
     WHERE tenant_id = OLD.tenant_id AND id = OLD.goods_receipt_id
       AND deleted_at IS NULL
     FOR UPDATE;
    IF parent_status IS DISTINCT FROM 'draft' THEN
        RAISE EXCEPTION 'Only draft goods receipt lines can be removed';
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS procurement_goods_receipt_line_delete_guard
    ON procurement_goods_receipt_lines;
CREATE TRIGGER procurement_goods_receipt_line_delete_guard
    BEFORE DELETE ON procurement_goods_receipt_lines
    FOR EACH ROW EXECUTE FUNCTION validate_procurement_goods_receipt_line_delete();
