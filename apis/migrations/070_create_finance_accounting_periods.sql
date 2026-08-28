-- Finance accounting calendar.
--
-- Fiscal years and their generated accounting periods are the dated posting
-- boundary for the later journal lifecycle. Structure is immutable after
-- creation; lifecycle changes remain explicit and auditable.

CREATE EXTENSION IF NOT EXISTS btree_gist;

CREATE TABLE IF NOT EXISTS finance_fiscal_years (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL CHECK (BTRIM(name) <> ''),
    starts_on DATE NOT NULL,
    ends_on DATE NOT NULL,
    period_cadence TEXT NOT NULL CHECK (period_cadence IN ('monthly', 'quarterly')),
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'open', 'closed')),
    opened_at TIMESTAMPTZ,
    closed_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CHECK (ends_on >= starts_on),
    CHECK (
        (status = 'draft' AND opened_at IS NULL AND closed_at IS NULL)
        OR (status = 'open' AND opened_at IS NOT NULL AND closed_at IS NULL)
        OR (status = 'closed' AND opened_at IS NOT NULL AND closed_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_finance_fiscal_years_tenant_name
    ON finance_fiscal_years(tenant_id, LOWER(name)) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_finance_fiscal_years_tenant_status
    ON finance_fiscal_years(tenant_id, status, starts_on DESC) WHERE deleted_at IS NULL;

ALTER TABLE finance_fiscal_years
    DROP CONSTRAINT IF EXISTS finance_fiscal_years_no_overlap;
ALTER TABLE finance_fiscal_years
    ADD CONSTRAINT finance_fiscal_years_no_overlap
    EXCLUDE USING gist (
        tenant_id WITH =,
        daterange(starts_on, ends_on, '[]') WITH &&
    ) WHERE (deleted_at IS NULL);

DROP TRIGGER IF EXISTS update_finance_fiscal_years_updated_at ON finance_fiscal_years;
CREATE TRIGGER update_finance_fiscal_years_updated_at
    BEFORE UPDATE ON finance_fiscal_years
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS finance_accounting_periods (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    fiscal_year_id UUID NOT NULL,
    period_number SMALLINT NOT NULL CHECK (period_number > 0),
    name TEXT NOT NULL CHECK (BTRIM(name) <> ''),
    starts_on DATE NOT NULL,
    ends_on DATE NOT NULL,
    status TEXT NOT NULL DEFAULT 'planned' CHECK (status IN ('planned', 'open', 'closed')),
    closed_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (fiscal_year_id, tenant_id)
        REFERENCES finance_fiscal_years(id, tenant_id),
    CHECK (ends_on >= starts_on),
    CHECK ((status = 'closed' AND closed_at IS NOT NULL) OR (status <> 'closed' AND closed_at IS NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_finance_accounting_periods_number
    ON finance_accounting_periods(tenant_id, fiscal_year_id, period_number)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_finance_accounting_periods_year_status
    ON finance_accounting_periods(tenant_id, fiscal_year_id, status, starts_on)
    WHERE deleted_at IS NULL;

ALTER TABLE finance_accounting_periods
    DROP CONSTRAINT IF EXISTS finance_accounting_periods_no_overlap;
ALTER TABLE finance_accounting_periods
    ADD CONSTRAINT finance_accounting_periods_no_overlap
    EXCLUDE USING gist (
        tenant_id WITH =,
        fiscal_year_id WITH =,
        daterange(starts_on, ends_on, '[]') WITH &&
    ) WHERE (deleted_at IS NULL);

DROP TRIGGER IF EXISTS update_finance_accounting_periods_updated_at ON finance_accounting_periods;
CREATE TRIGGER update_finance_accounting_periods_updated_at
    BEFORE UPDATE ON finance_accounting_periods
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE OR REPLACE FUNCTION validate_finance_fiscal_year_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF OLD.status <> 'draft' AND (
            NEW.name IS DISTINCT FROM OLD.name
            OR NEW.starts_on IS DISTINCT FROM OLD.starts_on
            OR NEW.ends_on IS DISTINCT FROM OLD.ends_on
            OR NEW.period_cadence IS DISTINCT FROM OLD.period_cadence
        ) THEN
            RAISE EXCEPTION 'An open or closed fiscal year cannot be restructured';
        END IF;

        IF NEW.starts_on IS DISTINCT FROM OLD.starts_on
            OR NEW.ends_on IS DISTINCT FROM OLD.ends_on
            OR NEW.period_cadence IS DISTINCT FROM OLD.period_cadence THEN
            RAISE EXCEPTION 'Fiscal year dates and cadence are fixed after creation';
        END IF;

        IF OLD.status = 'open' AND NEW.status = 'draft' THEN
            RAISE EXCEPTION 'An open fiscal year cannot return to draft';
        END IF;
        IF OLD.status = 'closed' AND NEW.status <> 'closed' THEN
            RAISE EXCEPTION 'A closed fiscal year cannot be reopened';
        END IF;

        IF NEW.deleted_at IS NOT NULL AND OLD.deleted_at IS NULL AND OLD.status <> 'draft' THEN
            RAISE EXCEPTION 'Only a draft fiscal year can be removed';
        END IF;

        IF OLD.status <> 'closed' AND NEW.status = 'closed' AND EXISTS (
            SELECT 1
              FROM finance_accounting_periods
             WHERE tenant_id = NEW.tenant_id
               AND fiscal_year_id = NEW.id
               AND deleted_at IS NULL
               AND status <> 'closed'
        ) THEN
            RAISE EXCEPTION 'Every accounting period must be closed before the fiscal year can close';
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS finance_fiscal_year_lifecycle_guard ON finance_fiscal_years;
CREATE TRIGGER finance_fiscal_year_lifecycle_guard
    BEFORE UPDATE ON finance_fiscal_years
    FOR EACH ROW
    EXECUTE FUNCTION validate_finance_fiscal_year_lifecycle();

CREATE OR REPLACE FUNCTION validate_finance_accounting_period()
RETURNS TRIGGER AS $$
DECLARE
    year_starts_on DATE;
    year_ends_on DATE;
    year_status TEXT;
BEGIN
    SELECT starts_on, ends_on, status
      INTO year_starts_on, year_ends_on, year_status
      FROM finance_fiscal_years
     WHERE id = NEW.fiscal_year_id
       AND tenant_id = NEW.tenant_id
       AND deleted_at IS NULL;

    IF year_status IS NULL THEN
        RAISE EXCEPTION 'The fiscal year is not available for this campus';
    END IF;
    IF NEW.starts_on < year_starts_on OR NEW.ends_on > year_ends_on THEN
        RAISE EXCEPTION 'An accounting period must stay within its fiscal year';
    END IF;

    IF TG_OP = 'UPDATE' THEN
        IF NEW.fiscal_year_id IS DISTINCT FROM OLD.fiscal_year_id
            OR NEW.period_number IS DISTINCT FROM OLD.period_number
            OR NEW.name IS DISTINCT FROM OLD.name
            OR NEW.starts_on IS DISTINCT FROM OLD.starts_on
            OR NEW.ends_on IS DISTINCT FROM OLD.ends_on THEN
            RAISE EXCEPTION 'Accounting period structure is fixed after creation';
        END IF;
        IF year_status <> 'open' AND NEW.status IS DISTINCT FROM OLD.status THEN
            RAISE EXCEPTION 'Period status can change only while its fiscal year is open';
        END IF;
        IF OLD.status = 'planned' AND NEW.status = 'closed' THEN
            RAISE EXCEPTION 'A planned period must open before it can close';
        END IF;
        IF OLD.status = 'closed' AND NEW.status = 'planned' THEN
            RAISE EXCEPTION 'A closed period cannot return to planned';
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS finance_accounting_period_guard ON finance_accounting_periods;
CREATE TRIGGER finance_accounting_period_guard
    BEFORE INSERT OR UPDATE ON finance_accounting_periods
    FOR EACH ROW
    EXECUTE FUNCTION validate_finance_accounting_period();

DROP TRIGGER IF EXISTS ev_finance_fiscal_years ON finance_fiscal_years;
CREATE TRIGGER ev_finance_fiscal_years
    AFTER INSERT OR UPDATE OR DELETE ON finance_fiscal_years
    FOR EACH ROW
    EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_finance_accounting_periods ON finance_accounting_periods;
CREATE TRIGGER ev_finance_accounting_periods
    AFTER INSERT OR UPDATE OR DELETE ON finance_accounting_periods
    FOR EACH ROW
    EXECUTE FUNCTION log_event();
