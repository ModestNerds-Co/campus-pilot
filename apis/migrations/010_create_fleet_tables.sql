--
--  campus-pilot-apis
--  010_create_fleet_tables.sql
--
--  Created by Ngonidzashe Mangudya on 2026/08/21.
--  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
--

CREATE TABLE IF NOT EXISTS vehicles (
  id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id           UUID NOT NULL REFERENCES tenants(id),
  registration_number TEXT NOT NULL,
  make                TEXT NOT NULL,
  model               TEXT NOT NULL,
  year                INTEGER,
  vehicle_type        TEXT NOT NULL DEFAULT 'bus',
  capacity            INTEGER,
  fuel_type           TEXT NOT NULL DEFAULT 'diesel',
  status              TEXT NOT NULL DEFAULT 'active',
  current_odometer    INTEGER NOT NULL DEFAULT 0,
  insurance_expiry    DATE,
  license_expiry      DATE,
  notes               TEXT,
  deleted_at          TIMESTAMP WITH TIME ZONE,
  created_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  updated_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_vehicles_tenant_registration
    ON vehicles(tenant_id, registration_number) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_vehicles_tenant_id ON vehicles(tenant_id);

DROP TRIGGER IF EXISTS update_vehicles_updated_at ON vehicles;
CREATE TRIGGER update_vehicles_updated_at
    BEFORE UPDATE ON vehicles
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS drivers (
  id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id      UUID NOT NULL REFERENCES tenants(id),
  employee_id    UUID,
  full_name      TEXT NOT NULL,
  license_number TEXT NOT NULL,
  license_class  TEXT,
  license_expiry DATE,
  phone          TEXT,
  status         TEXT NOT NULL DEFAULT 'active',
  deleted_at     TIMESTAMP WITH TIME ZONE,
  created_at     TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  updated_at     TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_drivers_tenant_license
    ON drivers(tenant_id, license_number) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_drivers_tenant_id ON drivers(tenant_id);

DROP TRIGGER IF EXISTS update_drivers_updated_at ON drivers;
CREATE TRIGGER update_drivers_updated_at
    BEFORE UPDATE ON drivers
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();
