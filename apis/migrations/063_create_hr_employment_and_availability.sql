--
--  campus-pilot-apis
--  063_create_hr_employment_and_availability.sql
--
--  Created by OpenAI Codex on 2026/08/28.
--  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
--

CREATE TABLE IF NOT EXISTS employment_engagements (
  id                     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id              UUID NOT NULL REFERENCES tenants(id),
  employee_id            UUID NOT NULL,
  reference              TEXT,
  employment_type        TEXT NOT NULL
    CHECK (employment_type IN ('permanent', 'fixed_term', 'temporary', 'casual', 'contractor', 'intern')),
  department_id          UUID,
  position_id            UUID,
  status                 TEXT NOT NULL DEFAULT 'draft'
    CHECK (status IN ('draft', 'active', 'ended', 'cancelled')),
  start_date             DATE,
  end_date               DATE,
  workload_basis_points  INTEGER NOT NULL DEFAULT 10000
    CHECK (workload_basis_points BETWEEN 1 AND 10000),
  notes                  TEXT,
  deleted_at             TIMESTAMP WITH TIME ZONE,
  created_at             TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  updated_at             TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  UNIQUE (id, tenant_id),
  FOREIGN KEY (employee_id, tenant_id) REFERENCES employees(id, tenant_id),
  FOREIGN KEY (department_id, tenant_id) REFERENCES departments(id, tenant_id),
  FOREIGN KEY (position_id, tenant_id) REFERENCES positions(id, tenant_id),
  CHECK (end_date IS NULL OR start_date IS NULL OR end_date >= start_date)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_employment_engagements_tenant_reference
  ON employment_engagements(tenant_id, LOWER(reference))
  WHERE reference IS NOT NULL AND deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_employment_engagements_one_active
  ON employment_engagements(tenant_id, employee_id)
  WHERE status = 'active' AND deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_employment_engagements_employee
  ON employment_engagements(tenant_id, employee_id, start_date DESC)
  WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_employment_engagements_status
  ON employment_engagements(tenant_id, status, start_date DESC)
  WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_employment_engagements_updated_at ON employment_engagements;
CREATE TRIGGER update_employment_engagements_updated_at
  BEFORE UPDATE ON employment_engagements
  FOR EACH ROW
  EXECUTE FUNCTION update_timestamp();

-- Preserve the existing workforce directory as the current projection while
-- establishing dated history. Unknown dates stay unknown instead of being
-- fabricated during the migration.
INSERT INTO employment_engagements (
  tenant_id,
  employee_id,
  employment_type,
  department_id,
  position_id,
  status,
  start_date,
  end_date
)
SELECT
  employee.tenant_id,
  employee.id,
  'permanent',
  employee.department_id,
  employee.position_id,
  CASE
    WHEN employee.employment_status IN ('active', 'suspended') THEN 'active'
    ELSE 'ended'
  END,
  employee.hire_date,
  employee.end_date
FROM employees AS employee
WHERE employee.deleted_at IS NULL
  AND NOT EXISTS (
    SELECT 1
    FROM employment_engagements AS engagement
    WHERE engagement.tenant_id = employee.tenant_id
      AND engagement.employee_id = employee.id
      AND engagement.deleted_at IS NULL
  );

CREATE TABLE IF NOT EXISTS employee_availability_periods (
  id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id     UUID NOT NULL REFERENCES tenants(id),
  employee_id   UUID NOT NULL,
  kind          TEXT NOT NULL
    CHECK (kind IN ('leave', 'training', 'medical', 'personal', 'other')),
  starts_at     TIMESTAMP WITH TIME ZONE NOT NULL,
  ends_at       TIMESTAMP WITH TIME ZONE NOT NULL,
  status        TEXT NOT NULL DEFAULT 'draft'
    CHECK (status IN ('draft', 'submitted', 'approved', 'rejected', 'cancelled')),
  notes         TEXT,
  decided_by    UUID,
  decided_at    TIMESTAMP WITH TIME ZONE,
  deleted_at    TIMESTAMP WITH TIME ZONE,
  created_at    TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  updated_at    TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  UNIQUE (id, tenant_id),
  FOREIGN KEY (employee_id, tenant_id) REFERENCES employees(id, tenant_id),
  FOREIGN KEY (decided_by, tenant_id) REFERENCES users(id, tenant_id),
  CHECK (ends_at > starts_at),
  CHECK (
    (status IN ('approved', 'rejected') AND decided_by IS NOT NULL AND decided_at IS NOT NULL)
    OR (status NOT IN ('approved', 'rejected'))
  )
);

CREATE INDEX IF NOT EXISTS idx_employee_availability_employee
  ON employee_availability_periods(tenant_id, employee_id, starts_at DESC)
  WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_employee_availability_scheduling
  ON employee_availability_periods(tenant_id, starts_at, ends_at)
  WHERE status = 'approved' AND deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_employee_availability_status
  ON employee_availability_periods(tenant_id, status, starts_at DESC)
  WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_employee_availability_periods_updated_at ON employee_availability_periods;
CREATE TRIGGER update_employee_availability_periods_updated_at
  BEFORE UPDATE ON employee_availability_periods
  FOR EACH ROW
  EXECUTE FUNCTION update_timestamp();
