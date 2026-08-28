-- Controlled multi-currency Finance journals.
--
-- Journals own the immutable double-entry posting boundary. Other modules may
-- submit idempotent posting requests later, but never write these rows.

CREATE TABLE IF NOT EXISTS finance_journal_sequences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    fiscal_year_id UUID NOT NULL,
    last_number BIGINT NOT NULL DEFAULT 0 CHECK (last_number >= 0),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, fiscal_year_id),
    FOREIGN KEY (fiscal_year_id, tenant_id)
        REFERENCES finance_fiscal_years(id, tenant_id)
);

DROP TRIGGER IF EXISTS update_finance_journal_sequences_updated_at ON finance_journal_sequences;
CREATE TRIGGER update_finance_journal_sequences_updated_at
    BEFORE UPDATE ON finance_journal_sequences
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS finance_journals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    fiscal_year_id UUID NOT NULL,
    accounting_period_id UUID NOT NULL,
    reporting_currency_id UUID NOT NULL,
    reversal_of_journal_id UUID,
    journal_number TEXT NOT NULL CHECK (BTRIM(journal_number) <> ''),
    journal_date DATE NOT NULL,
    description TEXT NOT NULL CHECK (BTRIM(description) <> ''),
    reference TEXT CHECK (reference IS NULL OR CHAR_LENGTH(BTRIM(reference)) BETWEEN 1 AND 160),
    source_module_key TEXT CHECK (
        source_module_key IS NULL OR source_module_key ~ '^[a-z][a-z0-9_]{0,63}$'
    ),
    source_record_type TEXT CHECK (
        source_record_type IS NULL OR CHAR_LENGTH(BTRIM(source_record_type)) BETWEEN 1 AND 80
    ),
    source_record_id TEXT CHECK (
        source_record_id IS NULL OR CHAR_LENGTH(BTRIM(source_record_id)) BETWEEN 1 AND 200
    ),
    idempotency_key TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(idempotency_key)) BETWEEN 1 AND 200),
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'submitted', 'approved', 'rejected', 'posted')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    submitted_by UUID,
    submitted_at TIMESTAMPTZ,
    approved_by UUID,
    approved_at TIMESTAMPTZ,
    rejected_by UUID,
    rejected_at TIMESTAMPTZ,
    rejection_reason TEXT CHECK (
        rejection_reason IS NULL OR CHAR_LENGTH(BTRIM(rejection_reason)) BETWEEN 1 AND 1000
    ),
    posted_by UUID,
    posted_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (fiscal_year_id, tenant_id)
        REFERENCES finance_fiscal_years(id, tenant_id),
    FOREIGN KEY (accounting_period_id, tenant_id)
        REFERENCES finance_accounting_periods(id, tenant_id),
    FOREIGN KEY (reporting_currency_id, tenant_id)
        REFERENCES finance_currencies(id, tenant_id),
    FOREIGN KEY (reversal_of_journal_id, tenant_id)
        REFERENCES finance_journals(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (submitted_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (approved_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (rejected_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (posted_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (source_module_key IS NULL AND source_record_type IS NULL AND source_record_id IS NULL)
        OR (source_module_key IS NOT NULL AND source_record_type IS NOT NULL AND source_record_id IS NOT NULL)
    ),
    CHECK (reversal_of_journal_id IS NULL OR reversal_of_journal_id <> id),
    CHECK (
        (status = 'draft' AND submitted_by IS NULL AND submitted_at IS NULL
            AND approved_by IS NULL AND approved_at IS NULL
            AND rejected_by IS NULL AND rejected_at IS NULL AND rejection_reason IS NULL
            AND posted_by IS NULL AND posted_at IS NULL)
        OR (status = 'submitted' AND submitted_by IS NOT NULL AND submitted_at IS NOT NULL
            AND approved_by IS NULL AND approved_at IS NULL
            AND rejected_by IS NULL AND rejected_at IS NULL AND rejection_reason IS NULL
            AND posted_by IS NULL AND posted_at IS NULL)
        OR (status = 'approved' AND submitted_by IS NOT NULL AND submitted_at IS NOT NULL
            AND approved_by IS NOT NULL AND approved_at IS NOT NULL
            AND rejected_by IS NULL AND rejected_at IS NULL AND rejection_reason IS NULL
            AND posted_by IS NULL AND posted_at IS NULL)
        OR (status = 'rejected' AND submitted_by IS NOT NULL AND submitted_at IS NOT NULL
            AND approved_by IS NULL AND approved_at IS NULL
            AND rejected_by IS NOT NULL AND rejected_at IS NOT NULL AND rejection_reason IS NOT NULL
            AND posted_by IS NULL AND posted_at IS NULL)
        OR (status = 'posted' AND submitted_by IS NOT NULL AND submitted_at IS NOT NULL
            AND approved_by IS NOT NULL AND approved_at IS NOT NULL
            AND rejected_by IS NULL AND rejected_at IS NULL AND rejection_reason IS NULL
            AND posted_by IS NOT NULL AND posted_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_finance_journals_tenant_number
    ON finance_journals(tenant_id, journal_number) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_finance_journals_idempotency
    ON finance_journals(tenant_id, idempotency_key) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_finance_journals_one_active_reversal
    ON finance_journals(tenant_id, reversal_of_journal_id)
    WHERE deleted_at IS NULL AND reversal_of_journal_id IS NOT NULL AND status <> 'rejected';
CREATE INDEX IF NOT EXISTS idx_finance_journals_tenant_date_status
    ON finance_journals(tenant_id, journal_date DESC, status) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_finance_journals_period
    ON finance_journals(tenant_id, accounting_period_id, status) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_finance_journals_source
    ON finance_journals(tenant_id, source_module_key, source_record_type, source_record_id)
    WHERE deleted_at IS NULL AND source_module_key IS NOT NULL;

DROP TRIGGER IF EXISTS update_finance_journals_updated_at ON finance_journals;
CREATE TRIGGER update_finance_journals_updated_at
    BEFORE UPDATE ON finance_journals
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS finance_journal_lines (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    journal_id UUID NOT NULL,
    account_id UUID NOT NULL,
    transaction_currency_id UUID NOT NULL,
    line_number SMALLINT NOT NULL CHECK (line_number > 0),
    description TEXT CHECK (description IS NULL OR CHAR_LENGTH(BTRIM(description)) BETWEEN 1 AND 500),
    account_code_snapshot TEXT NOT NULL CHECK (BTRIM(account_code_snapshot) <> ''),
    account_name_snapshot TEXT NOT NULL CHECK (BTRIM(account_name_snapshot) <> ''),
    transaction_currency_code TEXT NOT NULL CHECK (transaction_currency_code ~ '^[A-Z]{3}$'),
    transaction_currency_minor_units SMALLINT NOT NULL
        CHECK (transaction_currency_minor_units BETWEEN 0 AND 4),
    debit_minor BIGINT NOT NULL DEFAULT 0 CHECK (debit_minor BETWEEN 0 AND 9000000000000000),
    credit_minor BIGINT NOT NULL DEFAULT 0 CHECK (credit_minor BETWEEN 0 AND 9000000000000000),
    reporting_debit_minor BIGINT NOT NULL DEFAULT 0
        CHECK (reporting_debit_minor BETWEEN 0 AND 9000000000000000),
    reporting_credit_minor BIGINT NOT NULL DEFAULT 0
        CHECK (reporting_credit_minor BETWEEN 0 AND 9000000000000000),
    exchange_rate NUMERIC(38, 18),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (journal_id, tenant_id) REFERENCES finance_journals(id, tenant_id),
    FOREIGN KEY (account_id, tenant_id) REFERENCES finance_accounts(id, tenant_id),
    FOREIGN KEY (transaction_currency_id, tenant_id)
        REFERENCES finance_currencies(id, tenant_id),
    CHECK (
        (debit_minor > 0 AND credit_minor = 0
            AND reporting_debit_minor > 0 AND reporting_credit_minor = 0)
        OR (credit_minor > 0 AND debit_minor = 0
            AND reporting_credit_minor > 0 AND reporting_debit_minor = 0)
    ),
    CHECK (exchange_rate IS NULL OR exchange_rate > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_finance_journal_lines_number
    ON finance_journal_lines(tenant_id, journal_id, line_number) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_finance_journal_lines_account
    ON finance_journal_lines(tenant_id, account_id, journal_id) WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_finance_journal_lines_updated_at ON finance_journal_lines;
CREATE TRIGGER update_finance_journal_lines_updated_at
    BEFORE UPDATE ON finance_journal_lines
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE OR REPLACE FUNCTION validate_finance_journal_context()
RETURNS TRIGGER AS $$
DECLARE
    year_status TEXT;
    period_year_id UUID;
    period_starts_on DATE;
    period_ends_on DATE;
    period_status TEXT;
    reporting_is_current BOOLEAN;
BEGIN
    SELECT status INTO year_status
      FROM finance_fiscal_years
     WHERE tenant_id = NEW.tenant_id
       AND id = NEW.fiscal_year_id
       AND deleted_at IS NULL;

    SELECT fiscal_year_id, starts_on, ends_on, status
      INTO period_year_id, period_starts_on, period_ends_on, period_status
      FROM finance_accounting_periods
     WHERE tenant_id = NEW.tenant_id
       AND id = NEW.accounting_period_id
       AND deleted_at IS NULL;

    SELECT is_reporting AND status = 'active' AND deleted_at IS NULL
      INTO reporting_is_current
      FROM finance_currencies
     WHERE tenant_id = NEW.tenant_id
       AND id = NEW.reporting_currency_id;

    IF year_status IS DISTINCT FROM 'open' THEN
        RAISE EXCEPTION 'Journal entries require an open fiscal year';
    END IF;
    IF period_year_id IS NULL OR period_year_id <> NEW.fiscal_year_id THEN
        RAISE EXCEPTION 'The accounting period does not belong to the selected fiscal year';
    END IF;
    IF period_status IS DISTINCT FROM 'open' THEN
        RAISE EXCEPTION 'Journal entries require an open accounting period';
    END IF;
    IF NEW.journal_date < period_starts_on OR NEW.journal_date > period_ends_on THEN
        RAISE EXCEPTION 'The journal date must fall inside its accounting period';
    END IF;
    IF reporting_is_current IS DISTINCT FROM TRUE THEN
        RAISE EXCEPTION 'Journal entries require the active reporting currency';
    END IF;
    IF NEW.reversal_of_journal_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM finance_journals
         WHERE tenant_id = NEW.tenant_id
           AND id = NEW.reversal_of_journal_id
           AND status = 'posted'
           AND deleted_at IS NULL
    ) THEN
        RAISE EXCEPTION 'Only a posted journal can be reversed';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS finance_journal_context_guard ON finance_journals;
CREATE TRIGGER finance_journal_context_guard
    BEFORE INSERT OR UPDATE OF fiscal_year_id, accounting_period_id,
        reporting_currency_id, journal_date, reversal_of_journal_id
    ON finance_journals
    FOR EACH ROW
    EXECUTE FUNCTION validate_finance_journal_context();

CREATE OR REPLACE FUNCTION validate_finance_journal_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF OLD.status NOT IN ('draft', 'rejected') AND (
            NEW.fiscal_year_id IS DISTINCT FROM OLD.fiscal_year_id
            OR NEW.accounting_period_id IS DISTINCT FROM OLD.accounting_period_id
            OR NEW.reporting_currency_id IS DISTINCT FROM OLD.reporting_currency_id
            OR NEW.reversal_of_journal_id IS DISTINCT FROM OLD.reversal_of_journal_id
            OR NEW.journal_number IS DISTINCT FROM OLD.journal_number
            OR NEW.journal_date IS DISTINCT FROM OLD.journal_date
            OR NEW.description IS DISTINCT FROM OLD.description
            OR NEW.reference IS DISTINCT FROM OLD.reference
            OR NEW.source_module_key IS DISTINCT FROM OLD.source_module_key
            OR NEW.source_record_type IS DISTINCT FROM OLD.source_record_type
            OR NEW.source_record_id IS DISTINCT FROM OLD.source_record_id
            OR NEW.idempotency_key IS DISTINCT FROM OLD.idempotency_key
            OR NEW.created_by IS DISTINCT FROM OLD.created_by
        ) THEN
            RAISE EXCEPTION 'A submitted journal cannot be restructured';
        END IF;

        IF OLD.status = 'draft' AND NEW.status NOT IN ('draft', 'submitted') THEN
            RAISE EXCEPTION 'A draft journal can only be submitted';
        ELSIF OLD.status = 'submitted' AND NEW.status NOT IN ('submitted', 'approved', 'rejected') THEN
            RAISE EXCEPTION 'A submitted journal can only be approved or rejected';
        ELSIF OLD.status = 'approved' AND NEW.status NOT IN ('approved', 'posted') THEN
            RAISE EXCEPTION 'An approved journal can only be posted';
        ELSIF OLD.status = 'rejected' AND NEW.status NOT IN ('rejected', 'draft') THEN
            RAISE EXCEPTION 'A rejected journal must return to draft before submission';
        END IF;

        IF OLD.status = 'posted' THEN
            RAISE EXCEPTION 'A posted journal is immutable';
        END IF;

        IF NEW.deleted_at IS NOT NULL AND OLD.deleted_at IS NULL
            AND OLD.status NOT IN ('draft', 'rejected') THEN
            RAISE EXCEPTION 'Only a draft or rejected journal can be removed';
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS finance_journal_lifecycle_guard ON finance_journals;
CREATE TRIGGER finance_journal_lifecycle_guard
    BEFORE UPDATE ON finance_journals
    FOR EACH ROW
    EXECUTE FUNCTION validate_finance_journal_lifecycle();

CREATE OR REPLACE FUNCTION validate_finance_journal_line()
RETURNS TRIGGER AS $$
DECLARE
    parent_status TEXT;
    reporting_currency_id UUID;
    account_status TEXT;
    account_accepts_postings BOOLEAN;
    account_currency_mode TEXT;
    account_currency_id UUID;
    currency_status TEXT;
    currency_code TEXT;
    currency_minor_units SMALLINT;
BEGIN
    SELECT status, finance_journals.reporting_currency_id
      INTO parent_status, reporting_currency_id
      FROM finance_journals
     WHERE tenant_id = NEW.tenant_id
       AND id = NEW.journal_id
       AND deleted_at IS NULL;

    IF parent_status IS DISTINCT FROM 'draft' THEN
        RAISE EXCEPTION 'Journal lines can change only while the journal is draft';
    END IF;

    SELECT status, accepts_postings, currency_mode, currency_id
      INTO account_status, account_accepts_postings, account_currency_mode, account_currency_id
      FROM finance_accounts
     WHERE tenant_id = NEW.tenant_id
       AND id = NEW.account_id
       AND deleted_at IS NULL;

    SELECT status, code, minor_units
      INTO currency_status, currency_code, currency_minor_units
      FROM finance_currencies
     WHERE tenant_id = NEW.tenant_id
       AND id = NEW.transaction_currency_id
       AND deleted_at IS NULL;

    IF account_status IS DISTINCT FROM 'active' OR account_accepts_postings IS DISTINCT FROM TRUE THEN
        RAISE EXCEPTION 'Journal lines require an active posting account';
    END IF;
    IF currency_status IS DISTINCT FROM 'active' THEN
        RAISE EXCEPTION 'Journal lines require an active transaction currency';
    END IF;
    IF account_currency_mode = 'reporting' AND NEW.transaction_currency_id <> reporting_currency_id THEN
        RAISE EXCEPTION 'A reporting-currency account accepts only the reporting currency';
    END IF;
    IF account_currency_mode = 'single' AND NEW.transaction_currency_id <> account_currency_id THEN
        RAISE EXCEPTION 'A single-currency account accepts only its configured currency';
    END IF;
    IF NEW.transaction_currency_code <> currency_code
        OR NEW.transaction_currency_minor_units <> currency_minor_units THEN
        RAISE EXCEPTION 'Journal currency snapshots must match the current currency';
    END IF;
    IF NEW.transaction_currency_id = reporting_currency_id THEN
        IF NEW.debit_minor <> NEW.reporting_debit_minor
            OR NEW.credit_minor <> NEW.reporting_credit_minor
            OR NEW.exchange_rate IS NOT NULL THEN
            RAISE EXCEPTION 'Reporting-currency journal lines must keep equal transaction and reporting amounts';
        END IF;
    ELSIF NEW.exchange_rate IS NULL THEN
        RAISE EXCEPTION 'Foreign-currency journal lines require an exchange rate';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS finance_journal_line_guard ON finance_journal_lines;
CREATE TRIGGER finance_journal_line_guard
    BEFORE INSERT OR UPDATE ON finance_journal_lines
    FOR EACH ROW
    EXECUTE FUNCTION validate_finance_journal_line();

CREATE OR REPLACE FUNCTION protect_finance_journal_line_delete()
RETURNS TRIGGER AS $$
DECLARE
    parent_status TEXT;
BEGIN
    SELECT status INTO parent_status
      FROM finance_journals
     WHERE tenant_id = OLD.tenant_id AND id = OLD.journal_id;
    IF parent_status IS DISTINCT FROM 'draft' THEN
        RAISE EXCEPTION 'Journal lines can change only while the journal is draft';
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS finance_journal_line_delete_guard ON finance_journal_lines;
CREATE TRIGGER finance_journal_line_delete_guard
    BEFORE DELETE ON finance_journal_lines
    FOR EACH ROW
    EXECUTE FUNCTION protect_finance_journal_line_delete();

CREATE OR REPLACE FUNCTION protect_finance_reporting_basis()
RETURNS TRIGGER AS $$
BEGIN
    IF (OLD.is_reporting OR NEW.is_reporting) AND EXISTS (
        SELECT 1 FROM finance_journals
         WHERE tenant_id = NEW.tenant_id AND deleted_at IS NULL
    ) AND (
        NEW.code IS DISTINCT FROM OLD.code
        OR NEW.minor_units IS DISTINCT FROM OLD.minor_units
        OR NEW.is_reporting IS DISTINCT FROM OLD.is_reporting
        OR NEW.status IS DISTINCT FROM OLD.status
        OR NEW.deleted_at IS DISTINCT FROM OLD.deleted_at
    ) THEN
        RAISE EXCEPTION 'The reporting currency accounting basis is fixed after the first journal';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS finance_currency_accounting_basis_guard ON finance_currencies;
CREATE TRIGGER finance_currency_accounting_basis_guard
    BEFORE UPDATE ON finance_currencies
    FOR EACH ROW
    EXECUTE FUNCTION protect_finance_reporting_basis();

DROP TRIGGER IF EXISTS ev_finance_journals ON finance_journals;
CREATE TRIGGER ev_finance_journals
    AFTER INSERT OR UPDATE OR DELETE ON finance_journals
    FOR EACH ROW
    EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_finance_journal_lines ON finance_journal_lines;
CREATE TRIGGER ev_finance_journal_lines
    AFTER INSERT OR UPDATE OR DELETE ON finance_journal_lines
    FOR EACH ROW
    EXECUTE FUNCTION log_event();
