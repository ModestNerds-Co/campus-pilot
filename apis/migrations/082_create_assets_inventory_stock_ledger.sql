-- Assets and inventory stock ledger.
-- Posted movements and their lines are immutable. A guarded projection keeps
-- exact on-hand quantities while every post reconciles it to the signed ledger.

CREATE TABLE IF NOT EXISTS assets_inventory_movement_sequences (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id),
    last_number BIGINT NOT NULL DEFAULT 0 CHECK (last_number BETWEEN 0 AND 999999),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

DROP TRIGGER IF EXISTS update_assets_inventory_movement_sequences_updated_at
    ON assets_inventory_movement_sequences;
CREATE TRIGGER update_assets_inventory_movement_sequences_updated_at
    BEFORE UPDATE ON assets_inventory_movement_sequences
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS assets_inventory_stock_movements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    movement_number TEXT NOT NULL CHECK (movement_number ~ '^MOV-[0-9]{6}$'),
    kind TEXT NOT NULL CHECK (
        kind IN (
            'manual_receipt', 'issue', 'transfer', 'adjustment',
            'goods_receipt_allocation', 'reversal'
        )
    ),
    effective_on DATE NOT NULL,
    reference TEXT CHECK (
        reference IS NULL OR CHAR_LENGTH(BTRIM(reference)) BETWEEN 1 AND 200
    ),
    reason TEXT CHECK (
        reason IS NULL OR CHAR_LENGTH(BTRIM(reason)) BETWEEN 1 AND 2000
    ),
    source_goods_receipt_id UUID,
    source_goods_receipt_number TEXT,
    reverses_movement_id UUID,
    reverses_movement_number TEXT,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'posted')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    idempotency_key TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(idempotency_key)) BETWEEN 1 AND 200),
    create_request_fingerprint TEXT NOT NULL
        CHECK (create_request_fingerprint ~ '^[0-9a-f]{64}$'),
    created_by UUID NOT NULL,
    posted_by UUID,
    posted_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CONSTRAINT assets_inventory_stock_movements_created_by_tenant_fkey
        FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT assets_inventory_stock_movements_posted_by_tenant_fkey
        FOREIGN KEY (posted_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT assets_inventory_stock_movements_receipt_tenant_fkey
        FOREIGN KEY (source_goods_receipt_id, tenant_id)
        REFERENCES procurement_goods_receipts(id, tenant_id),
    CONSTRAINT assets_inventory_stock_movements_reversal_tenant_fkey
        FOREIGN KEY (reverses_movement_id, tenant_id)
        REFERENCES assets_inventory_stock_movements(id, tenant_id),
    CONSTRAINT assets_inventory_stock_movements_posting_state_check CHECK (
        (status = 'draft' AND posted_by IS NULL AND posted_at IS NULL)
        OR (status = 'posted' AND posted_by IS NOT NULL AND posted_at IS NOT NULL)
    ),
    CONSTRAINT assets_inventory_stock_movements_required_reason_check CHECK (
        kind NOT IN ('adjustment', 'reversal') OR reason IS NOT NULL
    ),
    CONSTRAINT assets_inventory_stock_movements_source_check CHECK (
        (
            kind = 'goods_receipt_allocation'
            AND source_goods_receipt_id IS NOT NULL
            AND source_goods_receipt_number ~ '^GRN-[0-9]{6}$'
            AND reverses_movement_id IS NULL
            AND reverses_movement_number IS NULL
        )
        OR (
            kind = 'reversal'
            AND reverses_movement_id IS NOT NULL
            AND reverses_movement_number ~ '^MOV-[0-9]{6}$'
            AND (
                (source_goods_receipt_id IS NULL AND source_goods_receipt_number IS NULL)
                OR (
                    source_goods_receipt_id IS NOT NULL
                    AND source_goods_receipt_number ~ '^GRN-[0-9]{6}$'
                )
            )
        )
        OR (
            kind IN ('manual_receipt', 'issue', 'transfer', 'adjustment')
            AND source_goods_receipt_id IS NULL
            AND source_goods_receipt_number IS NULL
            AND reverses_movement_id IS NULL
            AND reverses_movement_number IS NULL
        )
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_inventory_stock_movements_number
    ON assets_inventory_stock_movements(tenant_id, movement_number);
CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_inventory_stock_movements_idempotency
    ON assets_inventory_stock_movements(tenant_id, idempotency_key);
CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_inventory_stock_movements_reversal
    ON assets_inventory_stock_movements(tenant_id, reverses_movement_id)
    WHERE reverses_movement_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_assets_inventory_stock_movements_worklist
    ON assets_inventory_stock_movements(tenant_id, effective_on DESC, created_at DESC)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_assets_inventory_stock_movements_receipt
    ON assets_inventory_stock_movements(tenant_id, source_goods_receipt_id)
    WHERE source_goods_receipt_id IS NOT NULL;

DROP TRIGGER IF EXISTS update_assets_inventory_stock_movements_updated_at
    ON assets_inventory_stock_movements;
CREATE TRIGGER update_assets_inventory_stock_movements_updated_at
    BEFORE UPDATE ON assets_inventory_stock_movements
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS assets_inventory_stock_movement_lines (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    movement_id UUID NOT NULL,
    line_number INTEGER NOT NULL CHECK (line_number > 0),
    item_id UUID NOT NULL,
    item_number TEXT NOT NULL CHECK (item_number ~ '^ITM-[0-9]{6}$'),
    item_name TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(item_name)) BETWEEN 1 AND 180),
    store_id UUID NOT NULL,
    store_number TEXT NOT NULL CHECK (store_number ~ '^STR-[0-9]{6}$'),
    store_name TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(store_name)) BETWEEN 1 AND 180),
    quantity_delta_minor BIGINT NOT NULL CHECK (
        quantity_delta_minor BETWEEN -9007199254740991 AND 9007199254740991
        AND quantity_delta_minor <> 0
    ),
    quantity_scale SMALLINT NOT NULL CHECK (quantity_scale BETWEEN 0 AND 6),
    unit_label TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(unit_label)) BETWEEN 1 AND 40),
    on_hand_before_minor BIGINT CHECK (
        on_hand_before_minor BETWEEN 0 AND 9007199254740991
    ),
    on_hand_after_minor BIGINT CHECK (
        on_hand_after_minor BETWEEN 0 AND 9007199254740991
    ),
    source_goods_receipt_line_id UUID,
    source_goods_receipt_line_number INTEGER,
    source_goods_receipt_description TEXT,
    reverses_movement_line_id UUID,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CONSTRAINT assets_inventory_stock_movement_lines_parent_tenant_fkey
        FOREIGN KEY (movement_id, tenant_id)
        REFERENCES assets_inventory_stock_movements(id, tenant_id),
    CONSTRAINT assets_inventory_stock_movement_lines_item_tenant_fkey
        FOREIGN KEY (item_id, tenant_id)
        REFERENCES assets_inventory_items(id, tenant_id),
    CONSTRAINT assets_inventory_stock_movement_lines_store_tenant_fkey
        FOREIGN KEY (store_id, tenant_id)
        REFERENCES assets_inventory_stores(id, tenant_id),
    CONSTRAINT assets_inventory_stock_movement_lines_receipt_tenant_fkey
        FOREIGN KEY (source_goods_receipt_line_id, tenant_id)
        REFERENCES procurement_goods_receipt_lines(id, tenant_id),
    CONSTRAINT assets_inventory_stock_movement_lines_reversal_tenant_fkey
        FOREIGN KEY (reverses_movement_line_id, tenant_id)
        REFERENCES assets_inventory_stock_movement_lines(id, tenant_id),
    CONSTRAINT assets_inventory_stock_movement_lines_balances_check CHECK (
        (on_hand_before_minor IS NULL AND on_hand_after_minor IS NULL)
        OR (
            on_hand_before_minor IS NOT NULL
            AND on_hand_after_minor IS NOT NULL
            AND on_hand_after_minor::NUMERIC
                = on_hand_before_minor::NUMERIC + quantity_delta_minor::NUMERIC
        )
    ),
    CONSTRAINT assets_inventory_stock_movement_lines_source_check CHECK (
        (
            source_goods_receipt_line_id IS NULL
            AND source_goods_receipt_line_number IS NULL
            AND source_goods_receipt_description IS NULL
        )
        OR (
            source_goods_receipt_line_id IS NOT NULL
            AND source_goods_receipt_line_number > 0
            AND CHAR_LENGTH(BTRIM(source_goods_receipt_description)) BETWEEN 1 AND 500
        )
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_inventory_stock_movement_lines_number
    ON assets_inventory_stock_movement_lines(tenant_id, movement_id, line_number);
CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_inventory_stock_movement_lines_reversal
    ON assets_inventory_stock_movement_lines(tenant_id, reverses_movement_line_id)
    WHERE reverses_movement_line_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_assets_inventory_stock_movement_lines_item_store
    ON assets_inventory_stock_movement_lines(tenant_id, item_id, store_id);
CREATE INDEX IF NOT EXISTS idx_assets_inventory_stock_movement_lines_receipt
    ON assets_inventory_stock_movement_lines(tenant_id, source_goods_receipt_line_id)
    WHERE source_goods_receipt_line_id IS NOT NULL;

DROP TRIGGER IF EXISTS update_assets_inventory_stock_movement_lines_updated_at
    ON assets_inventory_stock_movement_lines;
CREATE TRIGGER update_assets_inventory_stock_movement_lines_updated_at
    BEFORE UPDATE ON assets_inventory_stock_movement_lines
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS assets_inventory_stock_balances (
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    item_id UUID NOT NULL,
    store_id UUID NOT NULL,
    on_hand_minor BIGINT NOT NULL DEFAULT 0 CHECK (
        on_hand_minor BETWEEN 0 AND 9007199254740991
    ),
    quantity_scale SMALLINT NOT NULL CHECK (quantity_scale BETWEEN 0 AND 6),
    unit_label TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(unit_label)) BETWEEN 1 AND 40),
    version INTEGER NOT NULL DEFAULT 0 CHECK (version >= 0),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, item_id, store_id),
    CONSTRAINT assets_inventory_stock_balances_item_tenant_fkey
        FOREIGN KEY (item_id, tenant_id)
        REFERENCES assets_inventory_items(id, tenant_id),
    CONSTRAINT assets_inventory_stock_balances_store_tenant_fkey
        FOREIGN KEY (store_id, tenant_id)
        REFERENCES assets_inventory_stores(id, tenant_id)
);

CREATE INDEX IF NOT EXISTS idx_assets_inventory_stock_balances_store
    ON assets_inventory_stock_balances(tenant_id, store_id, item_id);

DROP TRIGGER IF EXISTS update_assets_inventory_stock_balances_updated_at
    ON assets_inventory_stock_balances;
CREATE TRIGGER update_assets_inventory_stock_balances_updated_at
    BEFORE UPDATE ON assets_inventory_stock_balances
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE OR REPLACE FUNCTION validate_assets_inventory_movement_sequence_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Asset inventory movement sequence rows cannot be deleted';
    END IF;
    IF TG_OP = 'INSERT' THEN
        IF NEW.last_number IS DISTINCT FROM 1 OR NEW.deleted_at IS NOT NULL THEN
            RAISE EXCEPTION 'Asset inventory movement sequence must begin at one';
        END IF;
        RETURN NEW;
    END IF;
    IF OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
        OR OLD.created_at IS DISTINCT FROM NEW.created_at
        OR OLD.deleted_at IS DISTINCT FROM NEW.deleted_at
        OR NEW.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'Asset inventory movement sequence source fields are immutable';
    END IF;
    IF NEW.last_number IS DISTINCT FROM OLD.last_number + 1 THEN
        RAISE EXCEPTION 'Asset inventory movement sequence must advance by one';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS assets_inventory_movement_sequence_lifecycle_guard
    ON assets_inventory_movement_sequences;
CREATE TRIGGER assets_inventory_movement_sequence_lifecycle_guard
    BEFORE INSERT OR UPDATE OR DELETE ON assets_inventory_movement_sequences
    FOR EACH ROW EXECUTE FUNCTION validate_assets_inventory_movement_sequence_lifecycle();

CREATE OR REPLACE FUNCTION require_assets_inventory_movement_sequence_reference()
RETURNS TRIGGER AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
          FROM assets_inventory_stock_movements
         WHERE tenant_id = NEW.tenant_id
           AND movement_number = 'MOV-' || LPAD(NEW.last_number::TEXT, 6, '0')
    ) THEN
        RAISE EXCEPTION 'Asset inventory movement sequence must reference a movement';
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS assets_inventory_movement_sequence_reference_guard
    ON assets_inventory_movement_sequences;
CREATE CONSTRAINT TRIGGER assets_inventory_movement_sequence_reference_guard
    AFTER INSERT OR UPDATE ON assets_inventory_movement_sequences
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION require_assets_inventory_movement_sequence_reference();

CREATE OR REPLACE FUNCTION validate_assets_inventory_stock_movement_lifecycle()
RETURNS TRIGGER AS $$
DECLARE
    allocated_number BIGINT;
    source_receipt RECORD;
    source_movement RECORD;
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.status IS DISTINCT FROM 'draft'
            OR NEW.version IS DISTINCT FROM 1
            OR NEW.posted_by IS NOT NULL
            OR NEW.posted_at IS NOT NULL
            OR NEW.deleted_at IS NOT NULL THEN
            RAISE EXCEPTION 'Asset inventory stock movements must begin as draft at version one';
        END IF;
        SELECT last_number
          INTO allocated_number
          FROM assets_inventory_movement_sequences
         WHERE tenant_id = NEW.tenant_id AND deleted_at IS NULL
         FOR UPDATE;
        IF NOT FOUND OR NEW.movement_number IS DISTINCT FROM
            'MOV-' || LPAD(allocated_number::TEXT, 6, '0') THEN
            RAISE EXCEPTION 'Asset inventory movement number must match the allocated tenant sequence';
        END IF;
        IF NEW.kind = 'goods_receipt_allocation' THEN
            SELECT goods_receipt_number, status
              INTO source_receipt
              FROM procurement_goods_receipts
             WHERE tenant_id = NEW.tenant_id AND id = NEW.source_goods_receipt_id
               AND deleted_at IS NULL
             FOR UPDATE;
            IF source_receipt.status IS DISTINCT FROM 'posted'
                OR source_receipt.goods_receipt_number IS DISTINCT FROM
                    NEW.source_goods_receipt_number THEN
                RAISE EXCEPTION 'Goods receipt allocations require a posted Procurement receipt snapshot';
            END IF;
        ELSIF NEW.kind = 'reversal' THEN
            SELECT movement_number, kind, status, source_goods_receipt_id,
                   source_goods_receipt_number
              INTO source_movement
              FROM assets_inventory_stock_movements
             WHERE tenant_id = NEW.tenant_id AND id = NEW.reverses_movement_id
               AND deleted_at IS NULL
             FOR UPDATE;
            IF source_movement.status IS DISTINCT FROM 'posted'
                OR source_movement.kind IS NOT DISTINCT FROM 'reversal'
                OR source_movement.movement_number IS DISTINCT FROM
                    NEW.reverses_movement_number
                OR source_movement.source_goods_receipt_id IS DISTINCT FROM
                    NEW.source_goods_receipt_id
                OR source_movement.source_goods_receipt_number IS DISTINCT FROM
                    NEW.source_goods_receipt_number THEN
                RAISE EXCEPTION 'Reversals require an unreversed posted movement snapshot';
            END IF;
        END IF;
        RETURN NEW;
    END IF;
    IF OLD.id IS DISTINCT FROM NEW.id
        OR OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
        OR OLD.movement_number IS DISTINCT FROM NEW.movement_number
        OR OLD.kind IS DISTINCT FROM NEW.kind
        OR OLD.effective_on IS DISTINCT FROM NEW.effective_on
        OR OLD.reference IS DISTINCT FROM NEW.reference
        OR OLD.reason IS DISTINCT FROM NEW.reason
        OR OLD.source_goods_receipt_id IS DISTINCT FROM NEW.source_goods_receipt_id
        OR OLD.source_goods_receipt_number IS DISTINCT FROM NEW.source_goods_receipt_number
        OR OLD.reverses_movement_id IS DISTINCT FROM NEW.reverses_movement_id
        OR OLD.reverses_movement_number IS DISTINCT FROM NEW.reverses_movement_number
        OR OLD.idempotency_key IS DISTINCT FROM NEW.idempotency_key
        OR OLD.create_request_fingerprint IS DISTINCT FROM NEW.create_request_fingerprint
        OR OLD.created_by IS DISTINCT FROM NEW.created_by
        OR OLD.created_at IS DISTINCT FROM NEW.created_at
        OR OLD.deleted_at IS DISTINCT FROM NEW.deleted_at THEN
        RAISE EXCEPTION 'Asset inventory stock movement source fields are immutable';
    END IF;
    IF OLD.status = 'posted' THEN
        RAISE EXCEPTION 'A posted asset inventory stock movement is immutable';
    END IF;
    IF OLD.status IS DISTINCT FROM 'draft'
        OR NEW.status IS DISTINCT FROM 'posted'
        OR NEW.version IS DISTINCT FROM OLD.version + 1
        OR NEW.posted_by IS NULL
        OR NEW.posted_at IS NULL THEN
        RAISE EXCEPTION 'Asset inventory stock movement posting transition is invalid';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS assets_inventory_stock_movement_lifecycle_guard
    ON assets_inventory_stock_movements;
CREATE TRIGGER assets_inventory_stock_movement_lifecycle_guard
    BEFORE INSERT OR UPDATE ON assets_inventory_stock_movements
    FOR EACH ROW EXECUTE FUNCTION validate_assets_inventory_stock_movement_lifecycle();

CREATE OR REPLACE FUNCTION validate_assets_inventory_stock_movement_line()
RETURNS TRIGGER AS $$
DECLARE
    parent_movement assets_inventory_stock_movements%ROWTYPE;
    source_item assets_inventory_items%ROWTYPE;
    source_store assets_inventory_stores%ROWTYPE;
    source_receipt_line procurement_goods_receipt_lines%ROWTYPE;
    source_reversal_line assets_inventory_stock_movement_lines%ROWTYPE;
    posting_context TEXT;
BEGIN
    SELECT * INTO parent_movement
      FROM assets_inventory_stock_movements
     WHERE tenant_id = NEW.tenant_id AND id = NEW.movement_id
       AND deleted_at IS NULL
     FOR UPDATE;
    IF parent_movement.status IS DISTINCT FROM 'draft' THEN
        RAISE EXCEPTION 'Posted asset inventory stock movement lines are immutable';
    END IF;
    IF TG_OP = 'UPDATE' THEN
        posting_context := CURRENT_SETTING(
            'campus_pilot.stock_posting_movement_id', TRUE
        );
        IF posting_context IS DISTINCT FROM NEW.movement_id::TEXT THEN
            RAISE EXCEPTION 'Asset inventory stock movement lines can only be finalized by posting';
        END IF;
        IF OLD.id IS DISTINCT FROM NEW.id
            OR OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
            OR OLD.movement_id IS DISTINCT FROM NEW.movement_id
            OR OLD.line_number IS DISTINCT FROM NEW.line_number
            OR OLD.item_id IS DISTINCT FROM NEW.item_id
            OR OLD.item_number IS DISTINCT FROM NEW.item_number
            OR OLD.item_name IS DISTINCT FROM NEW.item_name
            OR OLD.store_id IS DISTINCT FROM NEW.store_id
            OR OLD.store_number IS DISTINCT FROM NEW.store_number
            OR OLD.store_name IS DISTINCT FROM NEW.store_name
            OR OLD.quantity_delta_minor IS DISTINCT FROM NEW.quantity_delta_minor
            OR OLD.quantity_scale IS DISTINCT FROM NEW.quantity_scale
            OR OLD.unit_label IS DISTINCT FROM NEW.unit_label
            OR OLD.source_goods_receipt_line_id IS DISTINCT FROM NEW.source_goods_receipt_line_id
            OR OLD.source_goods_receipt_line_number IS DISTINCT FROM NEW.source_goods_receipt_line_number
            OR OLD.source_goods_receipt_description IS DISTINCT FROM NEW.source_goods_receipt_description
            OR OLD.reverses_movement_line_id IS DISTINCT FROM NEW.reverses_movement_line_id
            OR OLD.deleted_at IS DISTINCT FROM NEW.deleted_at
            OR OLD.created_at IS DISTINCT FROM NEW.created_at THEN
            RAISE EXCEPTION 'Asset inventory stock movement line source fields are immutable';
        END IF;
        IF OLD.on_hand_before_minor IS NOT NULL
            OR OLD.on_hand_after_minor IS NOT NULL
            OR NEW.on_hand_before_minor IS NULL
            OR NEW.on_hand_after_minor IS NULL THEN
            RAISE EXCEPTION 'Asset inventory stock movement line balances finalize exactly once';
        END IF;
        RETURN NEW;
    END IF;

    IF NEW.on_hand_before_minor IS NOT NULL OR NEW.on_hand_after_minor IS NOT NULL
        OR NEW.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'Draft asset inventory stock movement lines cannot contain posted balances';
    END IF;
    IF parent_movement.kind = 'reversal' THEN
        SELECT * INTO source_reversal_line
          FROM assets_inventory_stock_movement_lines
         WHERE tenant_id = NEW.tenant_id AND id = NEW.reverses_movement_line_id
           AND movement_id = parent_movement.reverses_movement_id
           AND deleted_at IS NULL
         FOR SHARE;
        IF source_reversal_line.id IS NULL
            OR source_reversal_line.item_id IS DISTINCT FROM NEW.item_id
            OR source_reversal_line.item_number IS DISTINCT FROM NEW.item_number
            OR source_reversal_line.item_name IS DISTINCT FROM NEW.item_name
            OR source_reversal_line.store_id IS DISTINCT FROM NEW.store_id
            OR source_reversal_line.store_number IS DISTINCT FROM NEW.store_number
            OR source_reversal_line.store_name IS DISTINCT FROM NEW.store_name
            OR source_reversal_line.quantity_delta_minor::NUMERIC
                + NEW.quantity_delta_minor::NUMERIC <> 0
            OR source_reversal_line.quantity_scale IS DISTINCT FROM NEW.quantity_scale
            OR source_reversal_line.unit_label IS DISTINCT FROM NEW.unit_label
            OR source_reversal_line.source_goods_receipt_line_id IS DISTINCT FROM
                NEW.source_goods_receipt_line_id
            OR source_reversal_line.source_goods_receipt_line_number IS DISTINCT FROM
                NEW.source_goods_receipt_line_number
            OR source_reversal_line.source_goods_receipt_description IS DISTINCT FROM
                NEW.source_goods_receipt_description THEN
            RAISE EXCEPTION 'Reversal lines must be exact counter-entries';
        END IF;
        RETURN NEW;
    END IF;

    IF NEW.reverses_movement_line_id IS NOT NULL THEN
        RAISE EXCEPTION 'Only reversal movements can reference an original movement line';
    END IF;
    SELECT * INTO source_item
      FROM assets_inventory_items
     WHERE tenant_id = NEW.tenant_id AND id = NEW.item_id
       AND status = 'active' AND deleted_at IS NULL
     FOR SHARE;
    SELECT * INTO source_store
      FROM assets_inventory_stores
     WHERE tenant_id = NEW.tenant_id AND id = NEW.store_id
       AND status = 'active' AND deleted_at IS NULL
     FOR SHARE;
    IF source_item.id IS NULL OR source_store.id IS NULL
        OR source_item.item_number IS DISTINCT FROM NEW.item_number
        OR source_item.name IS DISTINCT FROM NEW.item_name
        OR source_store.store_number IS DISTINCT FROM NEW.store_number
        OR source_store.name IS DISTINCT FROM NEW.store_name
        OR source_item.quantity_scale IS DISTINCT FROM NEW.quantity_scale
        OR source_item.unit_label IS DISTINCT FROM NEW.unit_label THEN
        RAISE EXCEPTION 'Stock movement lines require active item and store snapshots';
    END IF;

    IF parent_movement.kind = 'goods_receipt_allocation' THEN
        SELECT line.* INTO source_receipt_line
          FROM procurement_goods_receipt_lines AS line
          JOIN procurement_goods_receipts AS receipt
            ON receipt.id = line.goods_receipt_id
           AND receipt.tenant_id = line.tenant_id
         WHERE line.tenant_id = NEW.tenant_id
           AND line.id = NEW.source_goods_receipt_line_id
           AND line.goods_receipt_id = parent_movement.source_goods_receipt_id
           AND line.deleted_at IS NULL
           AND receipt.status = 'posted' AND receipt.deleted_at IS NULL
         FOR SHARE OF line;
        IF source_receipt_line.id IS NULL
            OR source_receipt_line.line_number IS DISTINCT FROM
                NEW.source_goods_receipt_line_number
            OR source_receipt_line.description IS DISTINCT FROM
                NEW.source_goods_receipt_description
            OR source_receipt_line.quantity_scale IS DISTINCT FROM NEW.quantity_scale
            OR source_receipt_line.unit_label IS NULL
            OR LOWER(REGEXP_REPLACE(BTRIM(source_receipt_line.unit_label), '\s+', ' ', 'g'))
                IS DISTINCT FROM
                LOWER(REGEXP_REPLACE(BTRIM(NEW.unit_label), '\s+', ' ', 'g'))
            OR NEW.quantity_delta_minor <= 0 THEN
            RAISE EXCEPTION 'Goods receipt allocation lines must match their posted Procurement source';
        END IF;
    ELSIF NEW.source_goods_receipt_line_id IS NOT NULL
        OR NEW.source_goods_receipt_line_number IS NOT NULL
        OR NEW.source_goods_receipt_description IS NOT NULL THEN
        RAISE EXCEPTION 'Only goods receipt allocations can reference Procurement lines';
    END IF;

    IF parent_movement.kind = 'manual_receipt' AND NEW.quantity_delta_minor <= 0 THEN
        RAISE EXCEPTION 'Manual receipt quantities must add stock';
    END IF;
    IF parent_movement.kind = 'issue' AND NEW.quantity_delta_minor >= 0 THEN
        RAISE EXCEPTION 'Issue quantities must remove stock';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS assets_inventory_stock_movement_line_guard
    ON assets_inventory_stock_movement_lines;
CREATE TRIGGER assets_inventory_stock_movement_line_guard
    BEFORE INSERT OR UPDATE ON assets_inventory_stock_movement_lines
    FOR EACH ROW EXECUTE FUNCTION validate_assets_inventory_stock_movement_line();

CREATE OR REPLACE FUNCTION validate_assets_inventory_stock_balance_lifecycle()
RETURNS TRIGGER AS $$
DECLARE
    posting_context TEXT;
BEGIN
    posting_context := CURRENT_SETTING(
        'campus_pilot.stock_posting_movement_id', TRUE
    );
    IF posting_context IS NULL OR posting_context = '' OR PG_TRIGGER_DEPTH() < 2 THEN
        RAISE EXCEPTION 'Asset inventory stock balances are movement-owned projections';
    END IF;
    IF NOT EXISTS (
        SELECT 1
          FROM assets_inventory_stock_movements AS movement
          JOIN assets_inventory_stock_movement_lines AS line
            ON line.movement_id = movement.id
           AND line.tenant_id = movement.tenant_id
         WHERE movement.id::TEXT = posting_context
           AND movement.tenant_id = COALESCE(NEW.tenant_id, OLD.tenant_id)
           AND movement.status = 'draft' AND movement.deleted_at IS NULL
           AND line.item_id = COALESCE(NEW.item_id, OLD.item_id)
           AND line.store_id = COALESCE(NEW.store_id, OLD.store_id)
           AND line.deleted_at IS NULL
    ) THEN
        RAISE EXCEPTION 'Asset inventory stock balance write context is invalid';
    END IF;
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Asset inventory stock balance rows cannot be deleted';
    END IF;
    IF TG_OP = 'INSERT' THEN
        IF NEW.on_hand_minor IS DISTINCT FROM 0
            OR NEW.version IS DISTINCT FROM 0
            OR NEW.deleted_at IS NOT NULL THEN
            RAISE EXCEPTION 'Asset inventory stock balances must begin empty';
        END IF;
        RETURN NEW;
    END IF;
    IF OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
        OR OLD.item_id IS DISTINCT FROM NEW.item_id
        OR OLD.store_id IS DISTINCT FROM NEW.store_id
        OR OLD.quantity_scale IS DISTINCT FROM NEW.quantity_scale
        OR OLD.unit_label IS DISTINCT FROM NEW.unit_label
        OR OLD.deleted_at IS DISTINCT FROM NEW.deleted_at
        OR OLD.created_at IS DISTINCT FROM NEW.created_at
        OR NEW.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'Asset inventory stock balance source fields are immutable';
    END IF;
    IF NEW.version IS DISTINCT FROM OLD.version + 1 THEN
        RAISE EXCEPTION 'Asset inventory stock balance version must increment by one';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS assets_inventory_stock_balance_lifecycle_guard
    ON assets_inventory_stock_balances;
CREATE TRIGGER assets_inventory_stock_balance_lifecycle_guard
    BEFORE INSERT OR UPDATE OR DELETE ON assets_inventory_stock_balances
    FOR EACH ROW EXECUTE FUNCTION validate_assets_inventory_stock_balance_lifecycle();

CREATE OR REPLACE FUNCTION post_assets_inventory_stock_movement()
RETURNS TRIGGER AS $$
DECLARE
    movement_line assets_inventory_stock_movement_lines%ROWTYPE;
    balance_row assets_inventory_stock_balances%ROWTYPE;
    prior_ledger_balance NUMERIC;
    movement_delta NUMERIC;
    next_balance NUMERIC;
    source_line_count BIGINT;
    previous_context TEXT;
BEGIN
    IF OLD.status IS DISTINCT FROM 'draft' OR NEW.status IS DISTINCT FROM 'posted' THEN
        RETURN NEW;
    END IF;
    SELECT COUNT(*) INTO source_line_count
      FROM assets_inventory_stock_movement_lines
     WHERE tenant_id = NEW.tenant_id AND movement_id = NEW.id
       AND deleted_at IS NULL;
    IF source_line_count < 1 OR source_line_count > 400 THEN
        RAISE EXCEPTION 'A stock movement requires between one and four hundred lines';
    END IF;

    IF NEW.kind = 'transfer' AND EXISTS (
        SELECT 1
          FROM assets_inventory_stock_movement_lines
         WHERE tenant_id = NEW.tenant_id AND movement_id = NEW.id
           AND deleted_at IS NULL
         GROUP BY item_id
        HAVING SUM(quantity_delta_minor::NUMERIC) <> 0
            OR BOOL_AND(quantity_delta_minor > 0)
            OR BOOL_AND(quantity_delta_minor < 0)
    ) THEN
        RAISE EXCEPTION 'Transfer movement lines must balance by item';
    END IF;

    IF NEW.kind = 'reversal' THEN
        SELECT COUNT(*) INTO source_line_count
          FROM assets_inventory_stock_movement_lines
         WHERE tenant_id = NEW.tenant_id
           AND movement_id = NEW.reverses_movement_id
           AND deleted_at IS NULL;
        IF source_line_count IS DISTINCT FROM (
            SELECT COUNT(*)
              FROM assets_inventory_stock_movement_lines
             WHERE tenant_id = NEW.tenant_id AND movement_id = NEW.id
               AND reverses_movement_line_id IS NOT NULL AND deleted_at IS NULL
        ) THEN
            RAISE EXCEPTION 'A reversal must counter every original movement line';
        END IF;
    END IF;

    IF NEW.kind = 'goods_receipt_allocation' AND EXISTS (
        SELECT 1
          FROM assets_inventory_stock_movement_lines AS candidate
          JOIN assets_inventory_stock_movement_lines AS historical
            ON historical.tenant_id = candidate.tenant_id
           AND historical.source_goods_receipt_line_id
               = candidate.source_goods_receipt_line_id
          JOIN assets_inventory_stock_movements AS historical_movement
            ON historical_movement.id = historical.movement_id
           AND historical_movement.tenant_id = historical.tenant_id
         WHERE candidate.tenant_id = NEW.tenant_id
           AND candidate.movement_id = NEW.id
           AND candidate.deleted_at IS NULL
           AND historical_movement.status = 'posted'
           AND historical_movement.deleted_at IS NULL
           AND historical.deleted_at IS NULL
           AND historical.item_id <> candidate.item_id
    ) THEN
        RAISE EXCEPTION 'A Procurement receipt line cannot be remapped to another item';
    END IF;

    IF NEW.kind = 'goods_receipt_allocation' AND EXISTS (
        SELECT 1
          FROM procurement_goods_receipt_lines AS source_line
          JOIN assets_inventory_stock_movement_lines AS candidate
            ON candidate.tenant_id = source_line.tenant_id
           AND candidate.source_goods_receipt_line_id = source_line.id
         WHERE candidate.tenant_id = NEW.tenant_id
           AND candidate.movement_id = NEW.id
           AND candidate.deleted_at IS NULL
         GROUP BY source_line.id, source_line.quantity_minor
        HAVING COALESCE((
            SELECT SUM(existing.quantity_delta_minor::NUMERIC)
              FROM assets_inventory_stock_movement_lines AS existing
              JOIN assets_inventory_stock_movements AS existing_movement
                ON existing_movement.id = existing.movement_id
               AND existing_movement.tenant_id = existing.tenant_id
             WHERE existing.tenant_id = NEW.tenant_id
               AND existing.source_goods_receipt_line_id = source_line.id
               AND existing_movement.status = 'posted'
               AND existing_movement.deleted_at IS NULL
               AND existing.deleted_at IS NULL
        ), 0) + SUM(candidate.quantity_delta_minor::NUMERIC)
            > source_line.quantity_minor::NUMERIC
    ) THEN
        RAISE EXCEPTION 'Goods receipt allocation exceeds the unallocated received quantity';
    END IF;

    previous_context := CURRENT_SETTING(
        'campus_pilot.stock_posting_movement_id', TRUE
    );
    PERFORM SET_CONFIG(
        'campus_pilot.stock_posting_movement_id', NEW.id::TEXT, TRUE
    );

    INSERT INTO assets_inventory_stock_balances (
        tenant_id, item_id, store_id, on_hand_minor,
        quantity_scale, unit_label, version
    )
    SELECT DISTINCT line.tenant_id, line.item_id, line.store_id, 0,
           line.quantity_scale, line.unit_label, 0
      FROM assets_inventory_stock_movement_lines AS line
     WHERE line.tenant_id = NEW.tenant_id AND line.movement_id = NEW.id
       AND line.deleted_at IS NULL
     ORDER BY line.tenant_id, line.item_id, line.store_id
    ON CONFLICT (tenant_id, item_id, store_id) DO NOTHING;

    PERFORM 1
      FROM assets_inventory_stock_balances AS balance
      JOIN (
            SELECT DISTINCT item_id, store_id
              FROM assets_inventory_stock_movement_lines
             WHERE tenant_id = NEW.tenant_id AND movement_id = NEW.id
               AND deleted_at IS NULL
      ) AS affected
        ON affected.item_id = balance.item_id
       AND affected.store_id = balance.store_id
     WHERE balance.tenant_id = NEW.tenant_id
     ORDER BY balance.item_id, balance.store_id
     FOR UPDATE OF balance;

    FOR balance_row IN
        SELECT balance.*
          FROM assets_inventory_stock_balances AS balance
          JOIN (
                SELECT DISTINCT item_id, store_id
                  FROM assets_inventory_stock_movement_lines
                 WHERE tenant_id = NEW.tenant_id AND movement_id = NEW.id
                   AND deleted_at IS NULL
          ) AS affected
            ON affected.item_id = balance.item_id
           AND affected.store_id = balance.store_id
         WHERE balance.tenant_id = NEW.tenant_id
         ORDER BY balance.item_id, balance.store_id
    LOOP
        SELECT COALESCE(SUM(line.quantity_delta_minor::NUMERIC), 0)
          INTO prior_ledger_balance
          FROM assets_inventory_stock_movement_lines AS line
          JOIN assets_inventory_stock_movements AS movement
            ON movement.id = line.movement_id
           AND movement.tenant_id = line.tenant_id
         WHERE line.tenant_id = NEW.tenant_id
           AND line.item_id = balance_row.item_id
           AND line.store_id = balance_row.store_id
           AND movement.status = 'posted' AND movement.deleted_at IS NULL
           AND line.deleted_at IS NULL;
        IF prior_ledger_balance <> balance_row.on_hand_minor::NUMERIC THEN
            RAISE EXCEPTION 'Stock balance projection does not reconcile to the posted ledger';
        END IF;
    END LOOP;

    FOR movement_line IN
        SELECT *
          FROM assets_inventory_stock_movement_lines
         WHERE tenant_id = NEW.tenant_id AND movement_id = NEW.id
           AND deleted_at IS NULL
         ORDER BY item_id, store_id, line_number
    LOOP
        SELECT * INTO balance_row
          FROM assets_inventory_stock_balances
         WHERE tenant_id = NEW.tenant_id
           AND item_id = movement_line.item_id
           AND store_id = movement_line.store_id
         FOR UPDATE;
        IF balance_row.quantity_scale IS DISTINCT FROM movement_line.quantity_scale
            OR balance_row.unit_label IS DISTINCT FROM movement_line.unit_label THEN
            RAISE EXCEPTION 'Stock balance unit snapshot does not match the movement line';
        END IF;
        next_balance := balance_row.on_hand_minor::NUMERIC
            + movement_line.quantity_delta_minor::NUMERIC;
        IF next_balance < 0 OR next_balance > 9007199254740991 THEN
            RAISE EXCEPTION 'Stock movement would create a negative or unsafe balance';
        END IF;
        UPDATE assets_inventory_stock_movement_lines
           SET on_hand_before_minor = balance_row.on_hand_minor,
               on_hand_after_minor = next_balance::BIGINT
         WHERE tenant_id = NEW.tenant_id AND id = movement_line.id;
        UPDATE assets_inventory_stock_balances
           SET on_hand_minor = next_balance::BIGINT,
               version = version + 1
         WHERE tenant_id = NEW.tenant_id
           AND item_id = movement_line.item_id
           AND store_id = movement_line.store_id;
    END LOOP;

    FOR balance_row IN
        SELECT balance.*
          FROM assets_inventory_stock_balances AS balance
          JOIN (
                SELECT DISTINCT item_id, store_id
                  FROM assets_inventory_stock_movement_lines
                 WHERE tenant_id = NEW.tenant_id AND movement_id = NEW.id
                   AND deleted_at IS NULL
          ) AS affected
            ON affected.item_id = balance.item_id
           AND affected.store_id = balance.store_id
         WHERE balance.tenant_id = NEW.tenant_id
         ORDER BY balance.item_id, balance.store_id
    LOOP
        SELECT COALESCE(SUM(quantity_delta_minor::NUMERIC), 0)
          INTO movement_delta
          FROM assets_inventory_stock_movement_lines
         WHERE tenant_id = NEW.tenant_id AND movement_id = NEW.id
           AND item_id = balance_row.item_id
           AND store_id = balance_row.store_id
           AND deleted_at IS NULL;
        SELECT COALESCE(SUM(line.quantity_delta_minor::NUMERIC), 0)
          INTO prior_ledger_balance
          FROM assets_inventory_stock_movement_lines AS line
          JOIN assets_inventory_stock_movements AS movement
            ON movement.id = line.movement_id
           AND movement.tenant_id = line.tenant_id
         WHERE line.tenant_id = NEW.tenant_id
           AND line.item_id = balance_row.item_id
           AND line.store_id = balance_row.store_id
           AND movement.status = 'posted' AND movement.deleted_at IS NULL
           AND line.deleted_at IS NULL;
        IF prior_ledger_balance + movement_delta
            <> balance_row.on_hand_minor::NUMERIC THEN
            RAISE EXCEPTION 'Posted stock movement did not reconcile its balance projection';
        END IF;
    END LOOP;

    PERFORM SET_CONFIG(
        'campus_pilot.stock_posting_movement_id',
        COALESCE(previous_context, ''), TRUE
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS assets_inventory_stock_movement_posting_guard
    ON assets_inventory_stock_movements;
CREATE TRIGGER assets_inventory_stock_movement_posting_guard
    BEFORE UPDATE OF status ON assets_inventory_stock_movements
    FOR EACH ROW EXECUTE FUNCTION post_assets_inventory_stock_movement();

CREATE OR REPLACE FUNCTION require_assets_inventory_stock_movement_posted()
RETURNS TRIGGER AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
          FROM assets_inventory_stock_movements
         WHERE tenant_id = NEW.tenant_id AND id = NEW.id
           AND status = 'posted' AND deleted_at IS NULL
    ) THEN
        RAISE EXCEPTION 'Draft stock movements cannot be committed';
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS assets_inventory_stock_movement_committed_state_guard
    ON assets_inventory_stock_movements;
CREATE CONSTRAINT TRIGGER assets_inventory_stock_movement_committed_state_guard
    AFTER INSERT OR UPDATE ON assets_inventory_stock_movements
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION require_assets_inventory_stock_movement_posted();

CREATE OR REPLACE FUNCTION prevent_assets_inventory_stock_hard_delete()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION '% rows cannot be deleted', TG_ARGV[0];
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS assets_inventory_stock_movement_hard_delete_guard
    ON assets_inventory_stock_movements;
CREATE TRIGGER assets_inventory_stock_movement_hard_delete_guard
    BEFORE DELETE ON assets_inventory_stock_movements
    FOR EACH ROW EXECUTE FUNCTION prevent_assets_inventory_stock_hard_delete(
        'Asset inventory stock movement'
    );

DROP TRIGGER IF EXISTS assets_inventory_stock_movement_line_hard_delete_guard
    ON assets_inventory_stock_movement_lines;
CREATE TRIGGER assets_inventory_stock_movement_line_hard_delete_guard
    BEFORE DELETE ON assets_inventory_stock_movement_lines
    FOR EACH ROW EXECUTE FUNCTION prevent_assets_inventory_stock_hard_delete(
        'Asset inventory stock movement line'
    );

CREATE OR REPLACE FUNCTION guard_assets_inventory_item_stock_history()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.deleted_at IS NULL AND NEW.deleted_at IS NOT NULL AND EXISTS (
        SELECT 1
          FROM assets_inventory_stock_movement_lines
         WHERE tenant_id = OLD.tenant_id AND item_id = OLD.id
    ) THEN
        RAISE EXCEPTION 'Items with stock movement history cannot be removed';
    END IF;
    IF OLD.status = 'active' AND NEW.status = 'inactive' AND EXISTS (
        SELECT 1
          FROM assets_inventory_stock_balances
         WHERE tenant_id = OLD.tenant_id AND item_id = OLD.id
           AND on_hand_minor <> 0 AND deleted_at IS NULL
    ) THEN
        RAISE EXCEPTION 'Items with on-hand stock cannot be made inactive';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS assets_inventory_item_stock_history_guard
    ON assets_inventory_items;
CREATE TRIGGER assets_inventory_item_stock_history_guard
    BEFORE UPDATE ON assets_inventory_items
    FOR EACH ROW EXECUTE FUNCTION guard_assets_inventory_item_stock_history();

CREATE OR REPLACE FUNCTION guard_assets_inventory_store_stock_history()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.deleted_at IS NULL AND NEW.deleted_at IS NOT NULL AND EXISTS (
        SELECT 1
          FROM assets_inventory_stock_movement_lines
         WHERE tenant_id = OLD.tenant_id AND store_id = OLD.id
    ) THEN
        RAISE EXCEPTION 'Stores with stock movement history cannot be removed';
    END IF;
    IF OLD.status = 'active' AND NEW.status = 'inactive' AND EXISTS (
        SELECT 1
          FROM assets_inventory_stock_balances
         WHERE tenant_id = OLD.tenant_id AND store_id = OLD.id
           AND on_hand_minor <> 0 AND deleted_at IS NULL
    ) THEN
        RAISE EXCEPTION 'Stores with on-hand stock cannot be made inactive';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS assets_inventory_store_stock_history_guard
    ON assets_inventory_stores;
CREATE TRIGGER assets_inventory_store_stock_history_guard
    BEFORE UPDATE ON assets_inventory_stores
    FOR EACH ROW EXECUTE FUNCTION guard_assets_inventory_store_stock_history();
