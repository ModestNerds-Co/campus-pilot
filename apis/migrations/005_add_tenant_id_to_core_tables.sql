--
--  campus-pilot-apis
--  005_add_tenant_id_to_core_tables.sql
--
--  Created by Ngonidzashe Mangudya on 2026/08/21.
--  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
--

DO $$
DECLARE
  default_tenant_id UUID;
BEGIN
  SELECT id INTO default_tenant_id FROM tenants WHERE slug = 'default';

  ALTER TABLE school_profile ADD COLUMN IF NOT EXISTS tenant_id UUID REFERENCES tenants(id);
  UPDATE school_profile SET tenant_id = default_tenant_id WHERE tenant_id IS NULL;
  ALTER TABLE school_profile ALTER COLUMN tenant_id SET NOT NULL;

  ALTER TABLE users ADD COLUMN IF NOT EXISTS tenant_id UUID REFERENCES tenants(id);
  UPDATE users SET tenant_id = default_tenant_id WHERE tenant_id IS NULL;
  ALTER TABLE users ALTER COLUMN tenant_id SET NOT NULL;

  ALTER TABLE roles ADD COLUMN IF NOT EXISTS tenant_id UUID REFERENCES tenants(id);
  UPDATE roles SET tenant_id = default_tenant_id WHERE tenant_id IS NULL;
  ALTER TABLE roles ALTER COLUMN tenant_id SET NOT NULL;

  ALTER TABLE refresh_tokens ADD COLUMN IF NOT EXISTS tenant_id UUID REFERENCES tenants(id);
  UPDATE refresh_tokens SET tenant_id = default_tenant_id WHERE tenant_id IS NULL;
  ALTER TABLE refresh_tokens ALTER COLUMN tenant_id SET NOT NULL;

  ALTER TABLE password_reset_tokens ADD COLUMN IF NOT EXISTS tenant_id UUID REFERENCES tenants(id);
  UPDATE password_reset_tokens SET tenant_id = default_tenant_id WHERE tenant_id IS NULL;
  ALTER TABLE password_reset_tokens ALTER COLUMN tenant_id SET NOT NULL;

  -- Nullable: event_log is an audit trail that can also carry platform-level
  -- events not tied to any one tenant, unlike every other table here.
  ALTER TABLE event_log ADD COLUMN IF NOT EXISTS tenant_id UUID REFERENCES tenants(id);
  UPDATE event_log SET tenant_id = default_tenant_id WHERE tenant_id IS NULL;
END $$;

-- Role names were globally unique; now unique per tenant (each tenant gets its own
-- Super Admin / Admin / Faculty / Student rows).
ALTER TABLE roles DROP CONSTRAINT IF EXISTS roles_name_key;
CREATE UNIQUE INDEX IF NOT EXISTS idx_roles_tenant_name ON roles(tenant_id, name) WHERE deleted_at IS NULL;

-- school_profile drops the global-singleton constraint in favor of one-profile-per-tenant;
-- future tenants get a generated id instead of the literal 'singleton'.
DROP INDEX IF EXISTS one_school_only;
ALTER TABLE school_profile ALTER COLUMN id SET DEFAULT gen_random_uuid()::TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_school_profile_tenant ON school_profile(tenant_id) WHERE deleted_at IS NULL;

-- users.email was globally unique; scope to tenant so two schools can each have their own admin@school.
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_email_key;
DROP INDEX IF EXISTS idx_users_email_lower;
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_tenant_email_lower ON users(tenant_id, LOWER(email)) WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_users_tenant_id ON users(tenant_id);
CREATE INDEX IF NOT EXISTS idx_roles_tenant_id ON roles(tenant_id);
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_tenant_id ON refresh_tokens(tenant_id);
CREATE INDEX IF NOT EXISTS idx_event_log_tenant_id ON event_log(tenant_id);
