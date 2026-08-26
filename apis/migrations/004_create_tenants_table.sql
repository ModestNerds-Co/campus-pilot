--
--  campus-pilot-apis
--  004_create_tenants_table.sql
--
--  Created by Ngonidzashe Mangudya on 2026/08/21.
--  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
--

DO $$ BEGIN
    CREATE TYPE TENANT_STATUS AS ENUM ('Active','Suspended','Cancelled');
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

CREATE TABLE IF NOT EXISTS tenants (
  id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  slug        TEXT NOT NULL,
  name        TEXT NOT NULL,
  status      TENANT_STATUS NOT NULL DEFAULT 'Active',
  timezone    TEXT NOT NULL DEFAULT 'Africa/Harare',
  deleted_at  TIMESTAMP WITH TIME ZONE,
  created_at  TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  updated_at  TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_tenants_slug ON tenants(slug) WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_tenants_updated_at ON tenants;
CREATE TRIGGER update_tenants_updated_at
    BEFORE UPDATE ON tenants
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

-- A fresh install (or an existing single-tenant deployment predating tenancy) always ends up
-- with exactly one tenant here, seeded from the school_profile singleton if one already exists.
-- Multi-tenant SaaS installs get additional tenants through a later, separate provisioning flow.
INSERT INTO tenants (slug, name, timezone)
SELECT 'default', COALESCE(sp.name, 'Default School'), COALESCE(sp.timezone, 'Africa/Harare')
FROM school_profile sp
WHERE sp.id = 'singleton'
ON CONFLICT DO NOTHING;

INSERT INTO tenants (slug, name)
SELECT 'default', 'Default School'
WHERE NOT EXISTS (SELECT 1 FROM tenants WHERE slug = 'default');
