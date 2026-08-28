-- Owns tenant-scoped AI provider credentials and versioned model snapshots.
-- Credentials are encrypted by the application with tenant-bound AAD; routing,
-- Agent sessions, and usage remain separate later migrations.

CREATE TABLE IF NOT EXISTS ai_provider_connections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK (provider IN ('openai', 'anthropic', 'openrouter')),
    auth_method TEXT NOT NULL CHECK (auth_method = 'api_key'),
    account_label TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(account_label)) BETWEEN 1 AND 100),
    status TEXT NOT NULL DEFAULT 'untested' CHECK (status IN ('untested', 'ready', 'error', 'disconnected')),
    credential_ciphertext BYTEA,
    credential_nonce BYTEA,
    credential_key_id TEXT,
    credential_envelope_version SMALLINT NOT NULL DEFAULT 1 CHECK (credential_envelope_version = 1),
    credential_version BIGINT NOT NULL DEFAULT 1 CHECK (credential_version > 0),
    credential_fingerprint TEXT,
    configured_by UUID NOT NULL,
    last_tested_at TIMESTAMPTZ,
    last_test_status TEXT CHECK (last_test_status IS NULL OR last_test_status IN ('succeeded', 'failed')),
    last_failure_category TEXT CHECK (
        last_failure_category IS NULL OR last_failure_category IN (
            'authentication', 'rate_limited', 'unavailable', 'timeout',
            'network', 'invalid_response', 'unsupported'
        )
    ),
    last_used_at TIMESTAMPTZ,
    model_catalog_version BIGINT NOT NULL DEFAULT 0 CHECK (model_catalog_version >= 0),
    model_catalog_refreshed_at TIMESTAMPTZ,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT ai_provider_connections_id_tenant_unique
        UNIQUE (id, tenant_id),
    CONSTRAINT ai_provider_connections_configured_by_tenant_fk
        FOREIGN KEY (configured_by, tenant_id)
        REFERENCES users(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT ai_provider_connections_credential_lifecycle_check CHECK (
        (
            deleted_at IS NULL
            AND status <> 'disconnected'
            AND credential_ciphertext IS NOT NULL
            AND OCTET_LENGTH(credential_ciphertext) >= 16
            AND credential_nonce IS NOT NULL
            AND OCTET_LENGTH(credential_nonce) = 12
            AND credential_key_id IS NOT NULL
            AND CHAR_LENGTH(credential_key_id) BETWEEN 1 AND 128
            AND credential_fingerprint IS NOT NULL
        )
        OR (
            deleted_at IS NOT NULL
            AND status = 'disconnected'
            AND credential_ciphertext IS NULL
            AND credential_nonce IS NULL
            AND credential_key_id IS NULL
            AND credential_fingerprint IS NULL
        )
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS ai_provider_connections_label_unique
    ON ai_provider_connections (tenant_id, provider, LOWER(account_label))
    WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS ai_provider_connections_fingerprint_unique
    ON ai_provider_connections (tenant_id, provider, credential_fingerprint)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS ai_provider_connections_tenant_status_idx
    ON ai_provider_connections (tenant_id, status, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS ai_provider_models (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    connection_id UUID NOT NULL,
    credential_version BIGINT NOT NULL CHECK (credential_version > 0),
    catalog_version BIGINT NOT NULL CHECK (catalog_version > 0),
    provider_model_id TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(provider_model_id)) BETWEEN 1 AND 240),
    display_name TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(display_name)) BETWEEN 1 AND 240),
    context_window_tokens BIGINT CHECK (context_window_tokens IS NULL OR context_window_tokens > 0),
    supports_tools BOOLEAN,
    source TEXT NOT NULL DEFAULT 'provider' CHECK (source = 'provider'),
    refreshed_at TIMESTAMPTZ NOT NULL,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT ai_provider_models_connection_tenant_fk
        FOREIGN KEY (connection_id, tenant_id)
        REFERENCES ai_provider_connections(id, tenant_id) ON DELETE RESTRICT,
    CONSTRAINT ai_provider_models_snapshot_unique
        UNIQUE (connection_id, credential_version, catalog_version, provider_model_id)
);

CREATE INDEX IF NOT EXISTS ai_provider_models_current_lookup_idx
    ON ai_provider_models (
        tenant_id, connection_id, credential_version, catalog_version, provider_model_id
    )
    WHERE deleted_at IS NULL;

CREATE OR REPLACE FUNCTION prevent_ai_provider_model_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'AI provider model snapshots are immutable';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS ai_provider_models_prevent_update ON ai_provider_models;
CREATE TRIGGER ai_provider_models_prevent_update
    BEFORE UPDATE OR DELETE ON ai_provider_models
    FOR EACH ROW
    EXECUTE FUNCTION prevent_ai_provider_model_mutation();

-- New campuses seed provider administration authority for the School
-- Administrator. Existing non-owner roles do not silently gain Agent-related
-- permissions during this migration.
CREATE OR REPLACE FUNCTION grant_new_tenant_ai_provider_permissions()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE roles
    SET permissions = ARRAY(
            SELECT DISTINCT value
            FROM UNNEST(permissions || ARRAY['ai_providers:view', 'ai_providers:edit']::TEXT[])
                AS permission(value)
            ORDER BY value
        ),
        updated_at = NOW()
    WHERE tenant_id = NEW.id
      AND key = 'school_administrator'
      AND deleted_at IS NULL;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_grant_new_tenant_ai_provider_permissions ON tenants;
CREATE TRIGGER zz_grant_new_tenant_ai_provider_permissions
    AFTER INSERT ON tenants
    FOR EACH ROW
    EXECUTE FUNCTION grant_new_tenant_ai_provider_permissions();
