--
--  campus-pilot-apis
--  060_create_hr_foundation.sql
--
--  Created by OpenAI Codex on 2026/08/27.
--  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
--

CREATE TABLE IF NOT EXISTS departments (
  id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id   UUID NOT NULL REFERENCES tenants(id),
  code        TEXT NOT NULL,
  name        TEXT NOT NULL,
  status      TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive')),
  notes       TEXT,
  deleted_at  TIMESTAMP WITH TIME ZONE,
  created_at  TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  updated_at  TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  UNIQUE (id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_departments_tenant_code
  ON departments(tenant_id, LOWER(code)) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_departments_tenant_name
  ON departments(tenant_id, LOWER(name)) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_departments_tenant_id ON departments(tenant_id);

DROP TRIGGER IF EXISTS update_departments_updated_at ON departments;
CREATE TRIGGER update_departments_updated_at
  BEFORE UPDATE ON departments
  FOR EACH ROW
  EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS positions (
  id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id      UUID NOT NULL REFERENCES tenants(id),
  department_id  UUID,
  code           TEXT NOT NULL,
  title          TEXT NOT NULL,
  status         TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive')),
  notes          TEXT,
  deleted_at     TIMESTAMP WITH TIME ZONE,
  created_at     TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  updated_at     TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  UNIQUE (id, tenant_id),
  FOREIGN KEY (department_id, tenant_id)
    REFERENCES departments(id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_positions_tenant_code
  ON positions(tenant_id, LOWER(code)) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_positions_tenant_department
  ON positions(tenant_id, department_id) WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_positions_updated_at ON positions;
CREATE TRIGGER update_positions_updated_at
  BEFORE UPDATE ON positions
  FOR EACH ROW
  EXECUTE FUNCTION update_timestamp();

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'users_id_tenant_id_key'
  ) THEN
    ALTER TABLE users ADD CONSTRAINT users_id_tenant_id_key UNIQUE (id, tenant_id);
  END IF;
END $$;

CREATE TABLE IF NOT EXISTS employees (
  id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id          UUID NOT NULL REFERENCES tenants(id),
  account_id         UUID,
  employee_number    TEXT NOT NULL,
  display_name       TEXT NOT NULL,
  first_names        TEXT,
  surname            TEXT,
  work_email         TEXT CHECK (work_email IS NULL OR work_email = LOWER(work_email)),
  phone              TEXT,
  department_id      UUID,
  position_id        UUID,
  employment_status  TEXT NOT NULL DEFAULT 'active'
    CHECK (employment_status IN ('active', 'inactive', 'suspended', 'terminated')),
  hire_date          DATE,
  end_date           DATE,
  deleted_at         TIMESTAMP WITH TIME ZONE,
  created_at         TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  updated_at         TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  UNIQUE (id, tenant_id),
  FOREIGN KEY (account_id, tenant_id) REFERENCES users(id, tenant_id),
  FOREIGN KEY (department_id, tenant_id) REFERENCES departments(id, tenant_id),
  FOREIGN KEY (position_id, tenant_id) REFERENCES positions(id, tenant_id),
  CHECK (end_date IS NULL OR hire_date IS NULL OR end_date >= hire_date)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_employees_tenant_number
  ON employees(tenant_id, LOWER(employee_number)) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_employees_tenant_account
  ON employees(tenant_id, account_id)
  WHERE account_id IS NOT NULL AND deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_employees_tenant_status
  ON employees(tenant_id, employment_status) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_employees_tenant_department
  ON employees(tenant_id, department_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_employees_tenant_position
  ON employees(tenant_id, position_id) WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_employees_updated_at ON employees;
CREATE TRIGGER update_employees_updated_at
  BEFORE UPDATE ON employees
  FOR EACH ROW
  EXECUTE FUNCTION update_timestamp();

-- Existing Fleet installations may contain a standalone driver record. Give it
-- a stable employee identity before removing Fleet-owned person fields.
UPDATE drivers
SET employee_id = gen_random_uuid()
WHERE employee_id IS NULL;

INSERT INTO employees (
  id,
  tenant_id,
  employee_number,
  display_name,
  phone,
  employment_status
)
SELECT DISTINCT ON (driver.employee_id)
  driver.employee_id,
  driver.tenant_id,
  'DRV-' || UPPER(SUBSTRING(REPLACE(driver.employee_id::TEXT, '-', ''), 1, 12)),
  driver.full_name,
  driver.phone,
  CASE WHEN driver.status = 'inactive' THEN 'inactive' ELSE 'active' END
FROM drivers AS driver
WHERE driver.employee_id IS NOT NULL
ORDER BY driver.employee_id, driver.created_at
ON CONFLICT (id) DO NOTHING;

ALTER TABLE drivers ALTER COLUMN employee_id SET NOT NULL;
ALTER TABLE drivers
  ADD CONSTRAINT drivers_employee_tenant_fk
  FOREIGN KEY (employee_id, tenant_id) REFERENCES employees(id, tenant_id);
ALTER TABLE drivers
  ADD CONSTRAINT drivers_status_check CHECK (status IN ('active', 'inactive', 'suspended'));
CREATE UNIQUE INDEX IF NOT EXISTS idx_drivers_tenant_employee
  ON drivers(tenant_id, employee_id) WHERE deleted_at IS NULL;

ALTER TABLE drivers DROP COLUMN full_name;
ALTER TABLE drivers DROP COLUMN phone;
