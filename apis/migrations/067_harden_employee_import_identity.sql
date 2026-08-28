-- Employee work email is an import deduplication identity within one campus.

CREATE UNIQUE INDEX IF NOT EXISTS idx_employees_tenant_work_email
    ON employees(tenant_id, LOWER(work_email))
    WHERE work_email IS NOT NULL AND deleted_at IS NULL;
