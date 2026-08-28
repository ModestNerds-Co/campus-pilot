-- Assets and inventory catalogue foundations.
-- Items and stores own tenant-local references and immutable creation/scale
-- identity; stock balances and movements deliberately remain out of scope.

CREATE TABLE IF NOT EXISTS assets_inventory_item_sequences (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id),
    last_number BIGINT NOT NULL DEFAULT 0 CHECK (last_number BETWEEN 0 AND 999999),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

DROP TRIGGER IF EXISTS update_assets_inventory_item_sequences_updated_at
    ON assets_inventory_item_sequences;
CREATE TRIGGER update_assets_inventory_item_sequences_updated_at
    BEFORE UPDATE ON assets_inventory_item_sequences
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS assets_inventory_store_sequences (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id),
    last_number BIGINT NOT NULL DEFAULT 0 CHECK (last_number BETWEEN 0 AND 999999),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

DROP TRIGGER IF EXISTS update_assets_inventory_store_sequences_updated_at
    ON assets_inventory_store_sequences;
CREATE TRIGGER update_assets_inventory_store_sequences_updated_at
    BEFORE UPDATE ON assets_inventory_store_sequences
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS assets_inventory_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    item_number TEXT NOT NULL CHECK (item_number ~ '^ITM-[0-9]{6}$'),
    name TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(name)) BETWEEN 1 AND 180),
    description TEXT CHECK (
        description IS NULL OR CHAR_LENGTH(BTRIM(description)) BETWEEN 1 AND 2000
    ),
    barcode TEXT CHECK (
        barcode IS NULL OR CHAR_LENGTH(BTRIM(barcode)) BETWEEN 1 AND 200
    ),
    unit_label TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(unit_label)) BETWEEN 1 AND 40),
    quantity_scale SMALLINT NOT NULL CHECK (quantity_scale BETWEEN 0 AND 6),
    reorder_level_minor BIGINT,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    idempotency_key TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(idempotency_key)) BETWEEN 1 AND 200),
    create_request_fingerprint TEXT NOT NULL,
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    deleted_by UUID,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CONSTRAINT assets_inventory_items_created_by_tenant_fkey
        FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT assets_inventory_items_updated_by_tenant_fkey
        FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT assets_inventory_items_deleted_by_tenant_fkey
        FOREIGN KEY (deleted_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT assets_inventory_items_delete_actor_check CHECK (
        (deleted_at IS NULL AND deleted_by IS NULL)
        OR (deleted_at IS NOT NULL AND deleted_by IS NOT NULL)
    ),
    CONSTRAINT assets_inventory_items_create_fingerprint_check CHECK (
        create_request_fingerprint ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT assets_inventory_items_reorder_level_minor_check CHECK (
        reorder_level_minor IS NULL
        OR reorder_level_minor BETWEEN 0 AND 9007199254740991
    )
);

ALTER TABLE assets_inventory_items
    ADD COLUMN IF NOT EXISTS create_request_fingerprint TEXT;
UPDATE assets_inventory_items
   SET create_request_fingerprint = ENCODE(
        SHA256(CONVERT_TO(id::TEXT || ':legacy-item-create', 'UTF8')),
        'hex'
   )
 WHERE create_request_fingerprint IS NULL;
ALTER TABLE assets_inventory_items
    ALTER COLUMN create_request_fingerprint SET NOT NULL;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM assets_inventory_items
         WHERE reorder_level_minor < 0
            OR reorder_level_minor > 9007199254740991
    ) THEN
        RAISE EXCEPTION
            'Existing asset inventory reorder levels exceed the exact JSON integer boundary';
    END IF;
END;
$$;

ALTER TABLE assets_inventory_items
    DROP CONSTRAINT IF EXISTS assets_inventory_items_reorder_level_minor_check;
ALTER TABLE assets_inventory_items
    ADD CONSTRAINT assets_inventory_items_reorder_level_minor_check
    CHECK (
        reorder_level_minor IS NULL
        OR reorder_level_minor BETWEEN 0 AND 9007199254740991
    );

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conname = 'assets_inventory_items_create_fingerprint_check'
           AND conrelid = 'assets_inventory_items'::regclass
    ) THEN
        ALTER TABLE assets_inventory_items
            ADD CONSTRAINT assets_inventory_items_create_fingerprint_check
            CHECK (create_request_fingerprint ~ '^[0-9a-f]{64}$');
    END IF;
END;
$$;

CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_inventory_items_number
    ON assets_inventory_items(tenant_id, item_number);
CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_inventory_items_idempotency
    ON assets_inventory_items(tenant_id, idempotency_key);
CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_inventory_items_barcode
    ON assets_inventory_items(tenant_id, barcode) WHERE barcode IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_assets_inventory_items_worklist
    ON assets_inventory_items(tenant_id, status, name, item_number)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_assets_inventory_items_updated_at
    ON assets_inventory_items;
CREATE TRIGGER update_assets_inventory_items_updated_at
    BEFORE UPDATE ON assets_inventory_items
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE OR REPLACE FUNCTION validate_assets_inventory_item_lifecycle()
RETURNS TRIGGER AS $$
DECLARE
    allocated_number BIGINT;
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.status IS DISTINCT FROM 'active'
            OR NEW.version IS DISTINCT FROM 1
            OR NEW.updated_by IS DISTINCT FROM NEW.created_by
            OR NEW.deleted_at IS NOT NULL
            OR NEW.deleted_by IS NOT NULL THEN
            RAISE EXCEPTION 'Asset inventory items must begin active at version one';
        END IF;
        SELECT last_number
          INTO allocated_number
          FROM assets_inventory_item_sequences
         WHERE tenant_id = NEW.tenant_id
           AND deleted_at IS NULL
         FOR UPDATE;
        IF NOT FOUND THEN
            RAISE EXCEPTION
                'Asset inventory item reference requires an allocated tenant sequence';
        END IF;
        IF NEW.item_number IS DISTINCT FROM
            'ITM-' || LPAD(allocated_number::TEXT, 6, '0') THEN
            RAISE EXCEPTION
                'Asset inventory item number must match the allocated tenant sequence';
        END IF;
        RETURN NEW;
    END IF;
    IF OLD.id IS DISTINCT FROM NEW.id
        OR OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
        OR OLD.item_number IS DISTINCT FROM NEW.item_number
        OR OLD.idempotency_key IS DISTINCT FROM NEW.idempotency_key
        OR OLD.create_request_fingerprint IS DISTINCT FROM NEW.create_request_fingerprint
        OR OLD.created_by IS DISTINCT FROM NEW.created_by
        OR OLD.created_at IS DISTINCT FROM NEW.created_at THEN
        RAISE EXCEPTION 'Asset inventory item source fields are immutable';
    END IF;
    IF OLD.unit_label IS DISTINCT FROM NEW.unit_label
        OR OLD.quantity_scale IS DISTINCT FROM NEW.quantity_scale THEN
        RAISE EXCEPTION 'Asset inventory item unit and quantity scale are immutable';
    END IF;
    IF OLD.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'A deleted asset inventory item is immutable';
    END IF;
    IF NEW.version IS DISTINCT FROM OLD.version + 1 THEN
        RAISE EXCEPTION 'Asset inventory item version must increment by one';
    END IF;
    IF (NEW.deleted_at IS NULL) IS DISTINCT FROM (NEW.deleted_by IS NULL) THEN
        RAISE EXCEPTION 'Asset inventory item deletion requires an actor and timestamp';
    END IF;
    IF NEW.deleted_at IS NOT NULL AND OLD.status <> 'inactive' THEN
        RAISE EXCEPTION 'Only an inactive asset inventory item can be removed';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS assets_inventory_item_lifecycle_guard
    ON assets_inventory_items;
CREATE TRIGGER assets_inventory_item_lifecycle_guard
    BEFORE INSERT OR UPDATE ON assets_inventory_items
    FOR EACH ROW EXECUTE FUNCTION validate_assets_inventory_item_lifecycle();

CREATE TABLE IF NOT EXISTS assets_inventory_stores (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    store_number TEXT NOT NULL CHECK (store_number ~ '^STR-[0-9]{6}$'),
    name TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(name)) BETWEEN 1 AND 180),
    location_label TEXT CHECK (
        location_label IS NULL OR CHAR_LENGTH(BTRIM(location_label)) BETWEEN 1 AND 200
    ),
    notes TEXT CHECK (notes IS NULL OR CHAR_LENGTH(BTRIM(notes)) BETWEEN 1 AND 2000),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    idempotency_key TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(idempotency_key)) BETWEEN 1 AND 200),
    create_request_fingerprint TEXT NOT NULL,
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    deleted_by UUID,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CONSTRAINT assets_inventory_stores_created_by_tenant_fkey
        FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT assets_inventory_stores_updated_by_tenant_fkey
        FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT assets_inventory_stores_deleted_by_tenant_fkey
        FOREIGN KEY (deleted_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT assets_inventory_stores_delete_actor_check CHECK (
        (deleted_at IS NULL AND deleted_by IS NULL)
        OR (deleted_at IS NOT NULL AND deleted_by IS NOT NULL)
    ),
    CONSTRAINT assets_inventory_stores_create_fingerprint_check CHECK (
        create_request_fingerprint ~ '^[0-9a-f]{64}$'
    )
);

ALTER TABLE assets_inventory_stores
    ADD COLUMN IF NOT EXISTS create_request_fingerprint TEXT;
UPDATE assets_inventory_stores
   SET create_request_fingerprint = ENCODE(
        SHA256(CONVERT_TO(id::TEXT || ':legacy-store-create', 'UTF8')),
        'hex'
   )
 WHERE create_request_fingerprint IS NULL;
ALTER TABLE assets_inventory_stores
    ALTER COLUMN create_request_fingerprint SET NOT NULL;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conname = 'assets_inventory_stores_create_fingerprint_check'
           AND conrelid = 'assets_inventory_stores'::regclass
    ) THEN
        ALTER TABLE assets_inventory_stores
            ADD CONSTRAINT assets_inventory_stores_create_fingerprint_check
            CHECK (create_request_fingerprint ~ '^[0-9a-f]{64}$');
    END IF;
END;
$$;

CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_inventory_stores_number
    ON assets_inventory_stores(tenant_id, store_number);
CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_inventory_stores_idempotency
    ON assets_inventory_stores(tenant_id, idempotency_key);
CREATE INDEX IF NOT EXISTS idx_assets_inventory_stores_worklist
    ON assets_inventory_stores(tenant_id, status, name, store_number)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_assets_inventory_stores_updated_at
    ON assets_inventory_stores;
CREATE TRIGGER update_assets_inventory_stores_updated_at
    BEFORE UPDATE ON assets_inventory_stores
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE OR REPLACE FUNCTION validate_assets_inventory_store_lifecycle()
RETURNS TRIGGER AS $$
DECLARE
    allocated_number BIGINT;
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.status IS DISTINCT FROM 'active'
            OR NEW.version IS DISTINCT FROM 1
            OR NEW.updated_by IS DISTINCT FROM NEW.created_by
            OR NEW.deleted_at IS NOT NULL
            OR NEW.deleted_by IS NOT NULL THEN
            RAISE EXCEPTION 'Asset inventory stores must begin active at version one';
        END IF;
        SELECT last_number
          INTO allocated_number
          FROM assets_inventory_store_sequences
         WHERE tenant_id = NEW.tenant_id
           AND deleted_at IS NULL
         FOR UPDATE;
        IF NOT FOUND THEN
            RAISE EXCEPTION
                'Asset inventory store reference requires an allocated tenant sequence';
        END IF;
        IF NEW.store_number IS DISTINCT FROM
            'STR-' || LPAD(allocated_number::TEXT, 6, '0') THEN
            RAISE EXCEPTION
                'Asset inventory store number must match the allocated tenant sequence';
        END IF;
        RETURN NEW;
    END IF;
    IF OLD.id IS DISTINCT FROM NEW.id
        OR OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
        OR OLD.store_number IS DISTINCT FROM NEW.store_number
        OR OLD.idempotency_key IS DISTINCT FROM NEW.idempotency_key
        OR OLD.create_request_fingerprint IS DISTINCT FROM NEW.create_request_fingerprint
        OR OLD.created_by IS DISTINCT FROM NEW.created_by
        OR OLD.created_at IS DISTINCT FROM NEW.created_at THEN
        RAISE EXCEPTION 'Asset inventory store source fields are immutable';
    END IF;
    IF OLD.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'A deleted asset inventory store is immutable';
    END IF;
    IF NEW.version IS DISTINCT FROM OLD.version + 1 THEN
        RAISE EXCEPTION 'Asset inventory store version must increment by one';
    END IF;
    IF (NEW.deleted_at IS NULL) IS DISTINCT FROM (NEW.deleted_by IS NULL) THEN
        RAISE EXCEPTION 'Asset inventory store deletion requires an actor and timestamp';
    END IF;
    IF NEW.deleted_at IS NOT NULL AND OLD.status <> 'inactive' THEN
        RAISE EXCEPTION 'Only an inactive asset inventory store can be removed';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS assets_inventory_store_lifecycle_guard
    ON assets_inventory_stores;
CREATE TRIGGER assets_inventory_store_lifecycle_guard
    BEFORE INSERT OR UPDATE ON assets_inventory_stores
    FOR EACH ROW EXECUTE FUNCTION validate_assets_inventory_store_lifecycle();

CREATE OR REPLACE FUNCTION prevent_assets_inventory_hard_delete()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION '% must be soft deleted', TG_ARGV[0];
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS assets_inventory_item_hard_delete_guard
    ON assets_inventory_items;
CREATE TRIGGER assets_inventory_item_hard_delete_guard
    BEFORE DELETE ON assets_inventory_items
    FOR EACH ROW EXECUTE FUNCTION prevent_assets_inventory_hard_delete(
        'Asset inventory items'
    );

DROP TRIGGER IF EXISTS assets_inventory_store_hard_delete_guard
    ON assets_inventory_stores;
CREATE TRIGGER assets_inventory_store_hard_delete_guard
    BEFORE DELETE ON assets_inventory_stores
    FOR EACH ROW EXECUTE FUNCTION prevent_assets_inventory_hard_delete(
        'Asset inventory stores'
    );

CREATE OR REPLACE FUNCTION validate_assets_inventory_sequence_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION '% rows cannot be deleted', TG_ARGV[0];
    END IF;
    IF TG_OP = 'INSERT' THEN
        IF NEW.last_number IS DISTINCT FROM 1 OR NEW.deleted_at IS NOT NULL THEN
            RAISE EXCEPTION '% must begin at one', TG_ARGV[0];
        END IF;
        RETURN NEW;
    END IF;
    IF OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
        OR OLD.created_at IS DISTINCT FROM NEW.created_at
        OR OLD.deleted_at IS DISTINCT FROM NEW.deleted_at
        OR NEW.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION '% source fields are immutable', TG_ARGV[0];
    END IF;
    IF NEW.last_number IS DISTINCT FROM OLD.last_number + 1 THEN
        RAISE EXCEPTION '% must advance by one', TG_ARGV[0];
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS assets_inventory_item_sequence_lifecycle_guard
    ON assets_inventory_item_sequences;
CREATE TRIGGER assets_inventory_item_sequence_lifecycle_guard
    BEFORE INSERT OR UPDATE OR DELETE ON assets_inventory_item_sequences
    FOR EACH ROW EXECUTE FUNCTION validate_assets_inventory_sequence_lifecycle(
        'Asset inventory item sequence'
    );

DROP TRIGGER IF EXISTS assets_inventory_store_sequence_lifecycle_guard
    ON assets_inventory_store_sequences;
CREATE TRIGGER assets_inventory_store_sequence_lifecycle_guard
    BEFORE INSERT OR UPDATE OR DELETE ON assets_inventory_store_sequences
    FOR EACH ROW EXECUTE FUNCTION validate_assets_inventory_sequence_lifecycle(
        'Asset inventory store sequence'
    );
