-- Campus Pilot remote installation identity, encrypted renewal credential, and signed lease history.

CREATE TABLE IF NOT EXISTS license_installations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    deployment_id UUID NOT NULL DEFAULT gen_random_uuid(),
    remote_installation_id UUID,
    control_plane_url TEXT,
    credential_ciphertext TEXT,
    credential_nonce TEXT,
    credential_hint TEXT,
    status TEXT NOT NULL DEFAULT 'unconfigured'
        CHECK (status IN ('unconfigured', 'active', 'suspended', 'revoked', 'error')),
    latest_lease_sequence BIGINT NOT NULL DEFAULT 0 CHECK (latest_lease_sequence >= 0),
    last_refresh_attempt_at TIMESTAMPTZ,
    last_refresh_success_at TIMESTAMPTZ,
    last_error_code TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_license_installations_tenant
ON license_installations (tenant_id)
WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_license_installations_deployment
ON license_installations (deployment_id)
WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_license_installations_remote
ON license_installations (remote_installation_id)
WHERE deleted_at IS NULL AND remote_installation_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS license_leases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    remote_installation_id UUID NOT NULL,
    lease_id UUID NOT NULL,
    sequence BIGINT NOT NULL CHECK (sequence > 0),
    key_id TEXT NOT NULL,
    token_fingerprint TEXT NOT NULL,
    catalog_version TEXT NOT NULL,
    claims JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'superseded', 'revoked', 'expired', 'invalid')),
    issued_at TIMESTAMPTZ NOT NULL,
    refresh_after TIMESTAMPTZ NOT NULL,
    lease_expires_at TIMESTAMPTZ NOT NULL,
    grace_until TIMESTAMPTZ NOT NULL,
    token_expires_at TIMESTAMPTZ NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('online_activation', 'online_refresh', 'offline_import')),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_license_leases_tenant_sequence
ON license_leases (tenant_id, sequence)
WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_license_leases_remote_id
ON license_leases (remote_installation_id, lease_id)
WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_license_leases_tenant_status
ON license_leases (tenant_id, status, sequence DESC)
WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_license_installations_updated_at ON license_installations;
CREATE TRIGGER update_license_installations_updated_at
    BEFORE UPDATE ON license_installations
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

DROP TRIGGER IF EXISTS update_license_leases_updated_at ON license_leases;
CREATE TRIGGER update_license_leases_updated_at
    BEFORE UPDATE ON license_leases
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

DROP TRIGGER IF EXISTS ev_license_installations ON license_installations;
CREATE TRIGGER ev_license_installations
    AFTER INSERT OR UPDATE OR DELETE ON license_installations
    FOR EACH ROW
    EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_license_leases ON license_leases;
CREATE TRIGGER ev_license_leases
    AFTER INSERT OR UPDATE OR DELETE ON license_leases
    FOR EACH ROW
    EXECUTE FUNCTION log_event();
