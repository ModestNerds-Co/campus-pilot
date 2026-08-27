-- Normalized current entitlement evidence derived from the latest accepted signed lease.
-- The signed lease history remains immutable evidence; these tables are replaceable
-- tenant-scoped projections used by runtime authorization and reporting.

CREATE TABLE IF NOT EXISTS tenant_entitlements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    source_lease_id UUID NOT NULL,
    lease_sequence BIGINT NOT NULL CHECK (lease_sequence > 0),
    catalog_version TEXT NOT NULL CHECK (BTRIM(catalog_version) <> ''),
    min_app_version TEXT,
    max_app_version TEXT,
    projected_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_tenant_entitlements_tenant
ON tenant_entitlements (tenant_id);

CREATE TABLE IF NOT EXISTS tenant_entitlement_features (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    feature_key TEXT NOT NULL CHECK (BTRIM(feature_key) <> ''),
    source_lease_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, feature_key)
);

CREATE INDEX IF NOT EXISTS idx_tenant_entitlement_features_lease
ON tenant_entitlement_features (tenant_id, source_lease_id);

CREATE TABLE IF NOT EXISTS entitlement_limits (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    limit_key TEXT NOT NULL CHECK (BTRIM(limit_key) <> ''),
    source_lease_id UUID NOT NULL,
    unit TEXT NOT NULL CHECK (BTRIM(unit) <> ''),
    period TEXT NOT NULL CHECK (period IN ('none', 'day', 'month', 'year')),
    limit_value BIGINT NOT NULL CHECK (limit_value >= 0),
    enforcement TEXT NOT NULL CHECK (enforcement IN ('report', 'hard')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, limit_key)
);

CREATE INDEX IF NOT EXISTS idx_entitlement_limits_lease
ON entitlement_limits (tenant_id, source_lease_id);

DROP TRIGGER IF EXISTS update_tenant_entitlements_updated_at ON tenant_entitlements;
CREATE TRIGGER update_tenant_entitlements_updated_at
    BEFORE UPDATE ON tenant_entitlements
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

DROP TRIGGER IF EXISTS update_entitlement_limits_updated_at ON entitlement_limits;
CREATE TRIGGER update_entitlement_limits_updated_at
    BEFORE UPDATE ON entitlement_limits
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

DROP TRIGGER IF EXISTS ev_tenant_entitlements ON tenant_entitlements;
CREATE TRIGGER ev_tenant_entitlements
    AFTER INSERT OR UPDATE OR DELETE ON tenant_entitlements
    FOR EACH ROW
    EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_tenant_entitlement_features ON tenant_entitlement_features;
CREATE TRIGGER ev_tenant_entitlement_features
    AFTER INSERT OR UPDATE OR DELETE ON tenant_entitlement_features
    FOR EACH ROW
    EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_entitlement_limits ON entitlement_limits;
CREATE TRIGGER ev_entitlement_limits
    AFTER INSERT OR UPDATE OR DELETE ON entitlement_limits
    FOR EACH ROW
    EXECUTE FUNCTION log_event();
