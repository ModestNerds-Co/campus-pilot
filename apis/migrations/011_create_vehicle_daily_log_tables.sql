--
--  campus-pilot-apis
--  011_create_vehicle_daily_log_tables.sql
--
--  Created by Ngonidzashe Mangudya on 2026/08/21.
--  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
--

CREATE TABLE IF NOT EXISTS vehicle_daily_logs (
  id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id         UUID NOT NULL REFERENCES tenants(id),
  vehicle_id        UUID NOT NULL REFERENCES vehicles(id),
  driver_id         UUID NOT NULL REFERENCES drivers(id),
  log_date          DATE NOT NULL,
  start_odometer    INTEGER NOT NULL,
  end_odometer      INTEGER,
  start_time        TIME,
  end_time          TIME,
  destination       TEXT,
  purpose           TEXT NOT NULL,
  fuel_added_liters DOUBLE PRECISION,
  fuel_cost         DOUBLE PRECISION,
  status            TEXT NOT NULL DEFAULT 'draft',
  deleted_at        TIMESTAMP WITH TIME ZONE,
  created_at        TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  updated_at        TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_vehicle_daily_logs_tenant_id ON vehicle_daily_logs(tenant_id);
CREATE INDEX IF NOT EXISTS idx_vehicle_daily_logs_vehicle_id ON vehicle_daily_logs(vehicle_id);
CREATE INDEX IF NOT EXISTS idx_vehicle_daily_logs_log_date ON vehicle_daily_logs(log_date);

DROP TRIGGER IF EXISTS update_vehicle_daily_logs_updated_at ON vehicle_daily_logs;
CREATE TRIGGER update_vehicle_daily_logs_updated_at
    BEFORE UPDATE ON vehicle_daily_logs
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();
