-- Transactional hard-limit reservations and immutable committed usage evidence.
--
-- Limit definitions remain replaceable projections of the current signed lease.
-- Buckets and usage evidence survive lease refreshes so changing a plan cannot
-- erase consumption from the active reporting period.

CREATE TABLE IF NOT EXISTS entitlement_meter_buckets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    limit_key TEXT NOT NULL CHECK (BTRIM(limit_key) <> ''),
    period_start TIMESTAMPTZ NOT NULL,
    period_end TIMESTAMPTZ,
    committed_value BIGINT NOT NULL DEFAULT 0 CHECK (committed_value >= 0),
    reserved_value BIGINT NOT NULL DEFAULT 0 CHECK (reserved_value >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (period_end IS NULL OR period_end > period_start),
    UNIQUE (tenant_id, limit_key, period_start)
);

CREATE INDEX IF NOT EXISTS idx_entitlement_meter_buckets_current
ON entitlement_meter_buckets (tenant_id, limit_key, period_start DESC);

CREATE TABLE IF NOT EXISTS entitlement_usage_reservations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    bucket_id UUID NOT NULL REFERENCES entitlement_meter_buckets(id) ON DELETE CASCADE,
    source_lease_id UUID NOT NULL,
    limit_key TEXT NOT NULL CHECK (BTRIM(limit_key) <> ''),
    unit TEXT NOT NULL CHECK (BTRIM(unit) <> ''),
    operation_key TEXT NOT NULL CHECK (BTRIM(operation_key) <> ''),
    actor_user_id UUID,
    idempotency_key TEXT NOT NULL CHECK (BTRIM(idempotency_key) <> ''),
    amount BIGINT NOT NULL CHECK (amount > 0),
    status TEXT NOT NULL DEFAULT 'reserved'
        CHECK (status IN ('reserved', 'committed', 'released', 'expired')),
    expires_at TIMESTAMPTZ NOT NULL,
    committed_at TIMESTAMPTZ,
    released_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (
        (status = 'reserved' AND committed_at IS NULL AND released_at IS NULL)
        OR (status = 'committed' AND committed_at IS NOT NULL AND released_at IS NULL)
        OR (status IN ('released', 'expired') AND committed_at IS NULL AND released_at IS NOT NULL)
    ),
    UNIQUE (tenant_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_entitlement_usage_reservations_active
ON entitlement_usage_reservations (bucket_id, expires_at)
WHERE status = 'reserved';

CREATE TABLE IF NOT EXISTS entitlement_usage_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    reservation_id UUID NOT NULL REFERENCES entitlement_usage_reservations(id) ON DELETE RESTRICT,
    source_lease_id UUID NOT NULL,
    limit_key TEXT NOT NULL CHECK (BTRIM(limit_key) <> ''),
    unit TEXT NOT NULL CHECK (BTRIM(unit) <> ''),
    operation_key TEXT NOT NULL CHECK (BTRIM(operation_key) <> ''),
    actor_user_id UUID,
    amount BIGINT NOT NULL CHECK (amount > 0),
    period_start TIMESTAMPTZ NOT NULL,
    period_end TIMESTAMPTZ,
    occurred_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (period_end IS NULL OR period_end > period_start),
    UNIQUE (reservation_id)
);

CREATE INDEX IF NOT EXISTS idx_entitlement_usage_events_reporting
ON entitlement_usage_events (tenant_id, limit_key, occurred_at DESC);

CREATE INDEX IF NOT EXISTS idx_entitlement_usage_events_actor
ON entitlement_usage_events (tenant_id, actor_user_id, occurred_at DESC)
WHERE actor_user_id IS NOT NULL;

DROP TRIGGER IF EXISTS update_entitlement_meter_buckets_updated_at ON entitlement_meter_buckets;
CREATE TRIGGER update_entitlement_meter_buckets_updated_at
    BEFORE UPDATE ON entitlement_meter_buckets
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

DROP TRIGGER IF EXISTS update_entitlement_usage_reservations_updated_at ON entitlement_usage_reservations;
CREATE TRIGGER update_entitlement_usage_reservations_updated_at
    BEFORE UPDATE ON entitlement_usage_reservations
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

DROP TRIGGER IF EXISTS ev_entitlement_meter_buckets ON entitlement_meter_buckets;
CREATE TRIGGER ev_entitlement_meter_buckets
    AFTER INSERT OR UPDATE OR DELETE ON entitlement_meter_buckets
    FOR EACH ROW
    EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_entitlement_usage_reservations ON entitlement_usage_reservations;
CREATE TRIGGER ev_entitlement_usage_reservations
    AFTER INSERT OR UPDATE OR DELETE ON entitlement_usage_reservations
    FOR EACH ROW
    EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_entitlement_usage_events ON entitlement_usage_events;
CREATE TRIGGER ev_entitlement_usage_events
    AFTER INSERT OR DELETE ON entitlement_usage_events
    FOR EACH ROW
    EXECUTE FUNCTION log_event();
