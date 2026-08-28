-- Typed Finance posting requests and immutable Fees invoices.
--
-- Operational modules own their source records. They submit balanced,
-- idempotent posting requests; only Finance may turn those requests into
-- controlled journal drafts.

CREATE TABLE IF NOT EXISTS finance_posting_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    source_module_key TEXT NOT NULL
        CHECK (source_module_key ~ '^[a-z][a-z0-9_]{0,63}$'),
    source_record_type TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(source_record_type)) BETWEEN 1 AND 80),
    source_record_id TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(source_record_id)) BETWEEN 1 AND 200),
    source_event_key TEXT NOT NULL
        CHECK (source_event_key ~ '^[a-z][a-z0-9_]{0,79}$'),
    posting_date DATE NOT NULL,
    transaction_currency_id UUID NOT NULL,
    description TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(description)) BETWEEN 1 AND 1000),
    reference TEXT
        CHECK (reference IS NULL OR CHAR_LENGTH(BTRIM(reference)) BETWEEN 1 AND 160),
    idempotency_key TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(idempotency_key)) BETWEEN 1 AND 200),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'converted', 'rejected', 'cancelled')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    journal_id UUID,
    created_by UUID NOT NULL,
    resolved_by UUID,
    resolved_at TIMESTAMPTZ,
    resolution_reason TEXT
        CHECK (resolution_reason IS NULL OR CHAR_LENGTH(BTRIM(resolution_reason)) BETWEEN 1 AND 1000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (transaction_currency_id, tenant_id)
        REFERENCES finance_currencies(id, tenant_id),
    FOREIGN KEY (journal_id, tenant_id)
        REFERENCES finance_journals(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (resolved_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'pending' AND journal_id IS NULL AND resolved_by IS NULL
            AND resolved_at IS NULL AND resolution_reason IS NULL)
        OR (status = 'converted' AND journal_id IS NOT NULL AND resolved_by IS NOT NULL
            AND resolved_at IS NOT NULL AND resolution_reason IS NULL)
        OR (status IN ('rejected', 'cancelled') AND journal_id IS NULL
            AND resolved_by IS NOT NULL AND resolved_at IS NOT NULL
            AND resolution_reason IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_finance_posting_requests_idempotency
    ON finance_posting_requests(tenant_id, idempotency_key);
CREATE UNIQUE INDEX IF NOT EXISTS idx_finance_posting_requests_source_event
    ON finance_posting_requests(
        tenant_id, source_module_key, source_record_type, source_record_id, source_event_key
    );
CREATE INDEX IF NOT EXISTS idx_finance_posting_requests_status_date
    ON finance_posting_requests(tenant_id, status, posting_date DESC);
CREATE UNIQUE INDEX IF NOT EXISTS idx_finance_journals_posting_request_source
    ON finance_journals(tenant_id, source_record_id)
    WHERE deleted_at IS NULL
      AND source_module_key = 'finance'
      AND source_record_type = 'posting_request';

DROP TRIGGER IF EXISTS update_finance_posting_requests_updated_at ON finance_posting_requests;
CREATE TRIGGER update_finance_posting_requests_updated_at
    BEFORE UPDATE ON finance_posting_requests
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS finance_posting_request_lines (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    posting_request_id UUID NOT NULL,
    account_id UUID NOT NULL,
    line_number SMALLINT NOT NULL CHECK (line_number BETWEEN 1 AND 100),
    description TEXT
        CHECK (description IS NULL OR CHAR_LENGTH(BTRIM(description)) BETWEEN 1 AND 500),
    account_code_snapshot TEXT NOT NULL CHECK (BTRIM(account_code_snapshot) <> ''),
    account_name_snapshot TEXT NOT NULL CHECK (BTRIM(account_name_snapshot) <> ''),
    debit_minor BIGINT NOT NULL DEFAULT 0 CHECK (debit_minor BETWEEN 0 AND 9000000000000000),
    credit_minor BIGINT NOT NULL DEFAULT 0 CHECK (credit_minor BETWEEN 0 AND 9000000000000000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, posting_request_id, line_number),
    FOREIGN KEY (posting_request_id, tenant_id)
        REFERENCES finance_posting_requests(id, tenant_id),
    FOREIGN KEY (account_id, tenant_id)
        REFERENCES finance_accounts(id, tenant_id),
    CHECK (
        (debit_minor > 0 AND credit_minor = 0)
        OR (credit_minor > 0 AND debit_minor = 0)
    )
);

CREATE INDEX IF NOT EXISTS idx_finance_posting_request_lines_account
    ON finance_posting_request_lines(tenant_id, account_id, posting_request_id);

CREATE OR REPLACE FUNCTION validate_finance_posting_request_line()
RETURNS TRIGGER AS $$
DECLARE
    request_currency_id UUID;
    account_status TEXT;
    account_accepts_postings BOOLEAN;
    account_currency_mode TEXT;
    account_currency_id UUID;
    request_currency_is_reporting BOOLEAN;
BEGIN
    SELECT transaction_currency_id
      INTO request_currency_id
      FROM finance_posting_requests
     WHERE tenant_id = NEW.tenant_id AND id = NEW.posting_request_id;

    SELECT status, accepts_postings, currency_mode, currency_id
      INTO account_status, account_accepts_postings, account_currency_mode, account_currency_id
      FROM finance_accounts
     WHERE tenant_id = NEW.tenant_id AND id = NEW.account_id AND deleted_at IS NULL;

    SELECT is_reporting AND status = 'active' AND deleted_at IS NULL
      INTO request_currency_is_reporting
      FROM finance_currencies
     WHERE tenant_id = NEW.tenant_id AND id = request_currency_id;

    IF request_currency_id IS NULL THEN
        RAISE EXCEPTION 'Posting request currency was not found';
    END IF;
    IF account_status IS DISTINCT FROM 'active' OR account_accepts_postings IS DISTINCT FROM TRUE THEN
        RAISE EXCEPTION 'Posting request lines require active posting accounts';
    END IF;
    IF account_currency_mode = 'reporting' AND request_currency_is_reporting IS DISTINCT FROM TRUE THEN
        RAISE EXCEPTION 'The posting account accepts only the reporting currency';
    END IF;
    IF account_currency_mode = 'single' AND account_currency_id IS DISTINCT FROM request_currency_id THEN
        RAISE EXCEPTION 'The posting account accepts only its configured currency';
    END IF;
    IF account_currency_mode NOT IN ('reporting', 'single', 'multi') THEN
        RAISE EXCEPTION 'The posting account currency mode is invalid';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS finance_posting_request_line_guard ON finance_posting_request_lines;
CREATE TRIGGER finance_posting_request_line_guard
    BEFORE INSERT ON finance_posting_request_lines
    FOR EACH ROW
    EXECUTE FUNCTION validate_finance_posting_request_line();

CREATE OR REPLACE FUNCTION validate_finance_posting_request_balance()
RETURNS TRIGGER AS $$
DECLARE
    target_tenant_id UUID;
    target_request_id UUID;
    line_count INTEGER;
    debit_total NUMERIC;
    credit_total NUMERIC;
BEGIN
    target_tenant_id := COALESCE(NEW.tenant_id, OLD.tenant_id);
    target_request_id := COALESCE(NEW.posting_request_id, OLD.posting_request_id);

    SELECT COUNT(*), COALESCE(SUM(debit_minor), 0), COALESCE(SUM(credit_minor), 0)
      INTO line_count, debit_total, credit_total
      FROM finance_posting_request_lines
     WHERE tenant_id = target_tenant_id AND posting_request_id = target_request_id;

    IF line_count < 2 OR line_count > 100 THEN
        RAISE EXCEPTION 'A posting request requires between 2 and 100 lines';
    END IF;
    IF debit_total <= 0 OR debit_total <> credit_total THEN
        RAISE EXCEPTION 'A posting request must balance in its transaction currency';
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS finance_posting_request_balance_guard ON finance_posting_request_lines;
CREATE CONSTRAINT TRIGGER finance_posting_request_balance_guard
    AFTER INSERT OR UPDATE OR DELETE ON finance_posting_request_lines
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW
    EXECUTE FUNCTION validate_finance_posting_request_balance();

CREATE OR REPLACE FUNCTION guard_finance_posting_request_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF ROW(
        NEW.source_module_key, NEW.source_record_type, NEW.source_record_id,
        NEW.source_event_key, NEW.posting_date, NEW.transaction_currency_id,
        NEW.description, NEW.reference, NEW.idempotency_key, NEW.created_by
    ) IS DISTINCT FROM ROW(
        OLD.source_module_key, OLD.source_record_type, OLD.source_record_id,
        OLD.source_event_key, OLD.posting_date, OLD.transaction_currency_id,
        OLD.description, OLD.reference, OLD.idempotency_key, OLD.created_by
    ) THEN
        RAISE EXCEPTION 'Posting request source data is immutable';
    END IF;
    IF OLD.status <> 'pending' THEN
        RAISE EXCEPTION 'A resolved posting request is immutable';
    END IF;
    IF NEW.status = 'pending' OR NEW.status NOT IN ('converted', 'rejected', 'cancelled') THEN
        RAISE EXCEPTION 'The posting request lifecycle transition is invalid';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS finance_posting_request_lifecycle_guard ON finance_posting_requests;
CREATE TRIGGER finance_posting_request_lifecycle_guard
    BEFORE UPDATE ON finance_posting_requests
    FOR EACH ROW
    EXECUTE FUNCTION guard_finance_posting_request_lifecycle();

CREATE OR REPLACE FUNCTION deny_finance_posting_request_line_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Posting request lines are immutable';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS finance_posting_request_line_immutable ON finance_posting_request_lines;
CREATE TRIGGER finance_posting_request_line_immutable
    BEFORE UPDATE OR DELETE ON finance_posting_request_lines
    FOR EACH ROW
    EXECUTE FUNCTION deny_finance_posting_request_line_mutation();

-- Fees invoices are operational source records. They do not contain ledger
-- rows; an issued invoice references the Finance posting request it produced.
CREATE TABLE IF NOT EXISTS fees_invoice_sequences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    academic_year_id UUID NOT NULL,
    last_number BIGINT NOT NULL DEFAULT 0 CHECK (last_number >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, academic_year_id),
    FOREIGN KEY (academic_year_id, tenant_id)
        REFERENCES academic_years(id, tenant_id)
);

DROP TRIGGER IF EXISTS update_fees_invoice_sequences_updated_at ON fees_invoice_sequences;
CREATE TRIGGER update_fees_invoice_sequences_updated_at
    BEFORE UPDATE ON fees_invoice_sequences
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS fees_invoices (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    billing_account_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    academic_term_id UUID,
    currency_id UUID NOT NULL,
    posting_request_id UUID,
    invoice_number TEXT NOT NULL CHECK (BTRIM(invoice_number) <> ''),
    invoice_date DATE NOT NULL,
    due_date DATE NOT NULL CHECK (due_date >= invoice_date),
    description TEXT
        CHECK (description IS NULL OR CHAR_LENGTH(BTRIM(description)) BETWEEN 1 AND 1000),
    reference TEXT
        CHECK (reference IS NULL OR CHAR_LENGTH(BTRIM(reference)) BETWEEN 1 AND 160),
    total_minor BIGINT NOT NULL CHECK (total_minor BETWEEN 1 AND 9000000000000000),
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'issued')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    idempotency_key TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(idempotency_key)) BETWEEN 1 AND 200),
    created_by UUID NOT NULL,
    issued_by UUID,
    issued_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (billing_account_id, tenant_id)
        REFERENCES fees_billing_accounts(id, tenant_id),
    FOREIGN KEY (academic_year_id, tenant_id)
        REFERENCES academic_years(id, tenant_id),
    FOREIGN KEY (academic_term_id, tenant_id)
        REFERENCES academic_terms(id, tenant_id),
    FOREIGN KEY (currency_id, tenant_id)
        REFERENCES finance_currencies(id, tenant_id),
    FOREIGN KEY (posting_request_id, tenant_id)
        REFERENCES finance_posting_requests(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (issued_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'draft' AND posting_request_id IS NULL AND issued_by IS NULL AND issued_at IS NULL)
        OR (status = 'issued' AND posting_request_id IS NOT NULL
            AND issued_by IS NOT NULL AND issued_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_fees_invoices_number
    ON fees_invoices(tenant_id, invoice_number) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_fees_invoices_idempotency
    ON fees_invoices(tenant_id, idempotency_key) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_fees_invoices_posting_request
    ON fees_invoices(tenant_id, posting_request_id)
    WHERE deleted_at IS NULL AND posting_request_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_fees_invoices_billing_account
    ON fees_invoices(tenant_id, billing_account_id, invoice_date DESC) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_fees_invoices_status_date
    ON fees_invoices(tenant_id, status, invoice_date DESC) WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_fees_invoices_updated_at ON fees_invoices;
CREATE TRIGGER update_fees_invoices_updated_at
    BEFORE UPDATE ON fees_invoices
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS fees_invoice_lines (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    invoice_id UUID NOT NULL,
    fee_structure_id UUID NOT NULL,
    receivable_account_id UUID NOT NULL,
    revenue_account_id UUID NOT NULL,
    line_number SMALLINT NOT NULL CHECK (line_number BETWEEN 1 AND 100),
    fee_code_snapshot TEXT NOT NULL CHECK (BTRIM(fee_code_snapshot) <> ''),
    description TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(description)) BETWEEN 1 AND 500),
    amount_minor BIGINT NOT NULL CHECK (amount_minor BETWEEN 1 AND 9000000000000000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, invoice_id, line_number),
    UNIQUE (tenant_id, invoice_id, fee_structure_id),
    FOREIGN KEY (invoice_id, tenant_id) REFERENCES fees_invoices(id, tenant_id),
    FOREIGN KEY (fee_structure_id, tenant_id)
        REFERENCES fees_fee_structures(id, tenant_id),
    FOREIGN KEY (receivable_account_id, tenant_id)
        REFERENCES finance_accounts(id, tenant_id),
    FOREIGN KEY (revenue_account_id, tenant_id)
        REFERENCES finance_accounts(id, tenant_id)
);

CREATE INDEX IF NOT EXISTS idx_fees_invoice_lines_structure
    ON fees_invoice_lines(tenant_id, fee_structure_id, invoice_id);

CREATE OR REPLACE FUNCTION guard_fees_invoice_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.status = 'issued' THEN
        RAISE EXCEPTION 'An issued invoice is immutable';
    END IF;
    IF NEW.status <> 'issued' THEN
        RAISE EXCEPTION 'The invoice lifecycle transition is invalid';
    END IF;
    IF ROW(
        NEW.billing_account_id, NEW.academic_year_id, NEW.academic_term_id,
        NEW.currency_id, NEW.invoice_number, NEW.invoice_date, NEW.due_date,
        NEW.description, NEW.reference, NEW.total_minor, NEW.idempotency_key, NEW.created_by
    ) IS DISTINCT FROM ROW(
        OLD.billing_account_id, OLD.academic_year_id, OLD.academic_term_id,
        OLD.currency_id, OLD.invoice_number, OLD.invoice_date, OLD.due_date,
        OLD.description, OLD.reference, OLD.total_minor, OLD.idempotency_key, OLD.created_by
    ) THEN
        RAISE EXCEPTION 'Invoice source data is immutable after creation';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS fees_invoice_lifecycle_guard ON fees_invoices;
CREATE TRIGGER fees_invoice_lifecycle_guard
    BEFORE UPDATE ON fees_invoices
    FOR EACH ROW
    WHEN (OLD.deleted_at IS NOT DISTINCT FROM NEW.deleted_at)
    EXECUTE FUNCTION guard_fees_invoice_lifecycle();

CREATE OR REPLACE FUNCTION deny_fees_invoice_line_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Invoice lines are immutable';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS fees_invoice_line_immutable ON fees_invoice_lines;
CREATE TRIGGER fees_invoice_line_immutable
    BEFORE UPDATE OR DELETE ON fees_invoice_lines
    FOR EACH ROW
    EXECUTE FUNCTION deny_fees_invoice_line_mutation();

CREATE OR REPLACE FUNCTION validate_fees_invoice_references()
RETURNS TRIGGER AS $$
DECLARE
    account_status TEXT;
    account_learner_id UUID;
    term_year_id UUID;
    currency_status TEXT;
BEGIN
    SELECT status, learner_id INTO account_status, account_learner_id
      FROM fees_billing_accounts
     WHERE tenant_id = NEW.tenant_id AND id = NEW.billing_account_id AND deleted_at IS NULL;
    IF account_status IS DISTINCT FROM 'active' OR account_learner_id IS NULL THEN
        RAISE EXCEPTION 'Invoices require an active learner billing account';
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM academic_years
         WHERE tenant_id = NEW.tenant_id AND id = NEW.academic_year_id
           AND status IN ('active', 'completed') AND deleted_at IS NULL
    ) THEN
        RAISE EXCEPTION 'The invoice academic year is unavailable';
    END IF;

    IF NEW.academic_term_id IS NOT NULL THEN
        SELECT academic_year_id INTO term_year_id
          FROM academic_terms
         WHERE tenant_id = NEW.tenant_id AND id = NEW.academic_term_id
           AND status IN ('active', 'completed') AND deleted_at IS NULL;
        IF term_year_id IS NULL OR term_year_id <> NEW.academic_year_id THEN
            RAISE EXCEPTION 'The invoice term does not belong to its academic year';
        END IF;
    END IF;

    SELECT status INTO currency_status
      FROM finance_currencies
     WHERE tenant_id = NEW.tenant_id AND id = NEW.currency_id AND deleted_at IS NULL;
    IF currency_status IS DISTINCT FROM 'active' THEN
        RAISE EXCEPTION 'Invoices require an active currency';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS fees_invoice_reference_guard ON fees_invoices;
CREATE TRIGGER fees_invoice_reference_guard
    BEFORE INSERT ON fees_invoices
    FOR EACH ROW
    EXECUTE FUNCTION validate_fees_invoice_references();

CREATE OR REPLACE FUNCTION validate_fees_invoice_lines_total()
RETURNS TRIGGER AS $$
DECLARE
    target_tenant_id UUID;
    target_invoice_id UUID;
    expected_total BIGINT;
    line_total NUMERIC;
    line_count INTEGER;
BEGIN
    target_tenant_id := COALESCE(NEW.tenant_id, OLD.tenant_id);
    target_invoice_id := COALESCE(NEW.invoice_id, OLD.invoice_id);
    SELECT total_minor INTO expected_total
      FROM fees_invoices
     WHERE tenant_id = target_tenant_id AND id = target_invoice_id AND deleted_at IS NULL;
    SELECT COUNT(*), COALESCE(SUM(amount_minor), 0)
      INTO line_count, line_total
      FROM fees_invoice_lines
     WHERE tenant_id = target_tenant_id AND invoice_id = target_invoice_id;
    IF expected_total IS NULL OR line_count < 1 OR line_count > 100 OR line_total <> expected_total THEN
        RAISE EXCEPTION 'Invoice lines must match the invoice total';
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS fees_invoice_lines_total_guard ON fees_invoice_lines;
CREATE CONSTRAINT TRIGGER fees_invoice_lines_total_guard
    AFTER INSERT OR UPDATE OR DELETE ON fees_invoice_lines
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW
    EXECUTE FUNCTION validate_fees_invoice_lines_total();
