-- Learner billing accounts and currency-safe fee structures.
--
-- Fees owns the learner subledger configuration. Finance owns ledger rows;
-- later invoice and receipt records submit typed, balanced posting requests.

CREATE TABLE IF NOT EXISTS fees_billing_account_sequences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    last_number BIGINT NOT NULL DEFAULT 0 CHECK (last_number >= 0),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id)
);

DROP TRIGGER IF EXISTS update_fees_billing_account_sequences_updated_at ON fees_billing_account_sequences;
CREATE TRIGGER update_fees_billing_account_sequences_updated_at
    BEFORE UPDATE ON fees_billing_account_sequences
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS fees_billing_accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    learner_id UUID NOT NULL,
    account_number TEXT NOT NULL CHECK (BTRIM(account_number) <> ''),
    opened_on DATE NOT NULL,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'on_hold', 'closed')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    idempotency_key TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(idempotency_key)) BETWEEN 1 AND 200),
    created_by UUID NOT NULL,
    closed_by UUID,
    closed_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (learner_id, tenant_id) REFERENCES learners(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (closed_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'closed' AND closed_by IS NOT NULL AND closed_at IS NOT NULL)
        OR (status <> 'closed' AND closed_by IS NULL AND closed_at IS NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_fees_billing_accounts_learner
    ON fees_billing_accounts(tenant_id, learner_id) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_fees_billing_accounts_number
    ON fees_billing_accounts(tenant_id, LOWER(account_number)) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_fees_billing_accounts_idempotency
    ON fees_billing_accounts(tenant_id, idempotency_key) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_fees_billing_accounts_status
    ON fees_billing_accounts(tenant_id, status, account_number) WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_fees_billing_accounts_updated_at ON fees_billing_accounts;
CREATE TRIGGER update_fees_billing_accounts_updated_at
    BEFORE UPDATE ON fees_billing_accounts
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE OR REPLACE FUNCTION validate_fees_billing_account_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.learner_id IS DISTINCT FROM NEW.learner_id
        OR OLD.account_number IS DISTINCT FROM NEW.account_number
        OR OLD.opened_on IS DISTINCT FROM NEW.opened_on
        OR OLD.idempotency_key IS DISTINCT FROM NEW.idempotency_key
        OR OLD.created_by IS DISTINCT FROM NEW.created_by THEN
        RAISE EXCEPTION 'Billing account identity is fixed after creation';
    END IF;

    IF OLD.status = 'closed' THEN
        RAISE EXCEPTION 'A closed billing account is immutable';
    END IF;

    IF NEW.deleted_at IS NOT NULL AND OLD.deleted_at IS NULL THEN
        RAISE EXCEPTION 'Billing accounts retain their history and cannot be removed';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS fees_billing_account_lifecycle_guard ON fees_billing_accounts;
CREATE TRIGGER fees_billing_account_lifecycle_guard
    BEFORE UPDATE ON fees_billing_accounts
    FOR EACH ROW
    EXECUTE FUNCTION validate_fees_billing_account_lifecycle();

CREATE TABLE IF NOT EXISTS fees_fee_structures (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    academic_year_id UUID NOT NULL,
    academic_term_id UUID,
    grade_level_id UUID,
    currency_id UUID NOT NULL,
    receivable_account_id UUID NOT NULL,
    revenue_account_id UUID NOT NULL,
    code TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(code)) BETWEEN 1 AND 40),
    name TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(name)) BETWEEN 1 AND 160),
    description TEXT CHECK (
        description IS NULL OR CHAR_LENGTH(BTRIM(description)) BETWEEN 1 AND 1000
    ),
    amount_minor BIGINT NOT NULL CHECK (amount_minor BETWEEN 1 AND 9000000000000000),
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'active', 'retired')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    idempotency_key TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(idempotency_key)) BETWEEN 1 AND 200),
    created_by UUID NOT NULL,
    activated_by UUID,
    activated_at TIMESTAMPTZ,
    retired_by UUID,
    retired_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (academic_year_id, tenant_id) REFERENCES academic_years(id, tenant_id),
    FOREIGN KEY (academic_term_id, tenant_id) REFERENCES academic_terms(id, tenant_id),
    FOREIGN KEY (grade_level_id, tenant_id) REFERENCES academic_grade_levels(id, tenant_id),
    FOREIGN KEY (currency_id, tenant_id) REFERENCES finance_currencies(id, tenant_id),
    FOREIGN KEY (receivable_account_id, tenant_id) REFERENCES finance_accounts(id, tenant_id),
    FOREIGN KEY (revenue_account_id, tenant_id) REFERENCES finance_accounts(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (activated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (retired_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (receivable_account_id <> revenue_account_id),
    CHECK (
        (status = 'draft' AND activated_by IS NULL AND activated_at IS NULL
            AND retired_by IS NULL AND retired_at IS NULL)
        OR (status = 'active' AND activated_by IS NOT NULL AND activated_at IS NOT NULL
            AND retired_by IS NULL AND retired_at IS NULL)
        OR (status = 'retired' AND activated_by IS NOT NULL AND activated_at IS NOT NULL
            AND retired_by IS NOT NULL AND retired_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_fees_fee_structures_scope_code
    ON fees_fee_structures(
        tenant_id,
        academic_year_id,
        COALESCE(academic_term_id, '00000000-0000-0000-0000-000000000000'::UUID),
        COALESCE(grade_level_id, '00000000-0000-0000-0000-000000000000'::UUID),
        LOWER(code)
    ) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_fees_fee_structures_idempotency
    ON fees_fee_structures(tenant_id, idempotency_key) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_fees_fee_structures_status
    ON fees_fee_structures(tenant_id, status, academic_year_id, code) WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_fees_fee_structures_updated_at ON fees_fee_structures;
CREATE TRIGGER update_fees_fee_structures_updated_at
    BEFORE UPDATE ON fees_fee_structures
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE OR REPLACE FUNCTION validate_fees_fee_structure_references()
RETURNS TRIGGER AS $$
DECLARE
    year_status TEXT;
    term_year_id UUID;
    term_status TEXT;
    grade_status TEXT;
    currency_status TEXT;
    receivable_type TEXT;
    receivable_status TEXT;
    receivable_posting BOOLEAN;
    receivable_currency_mode TEXT;
    receivable_currency_id UUID;
    revenue_type TEXT;
    revenue_status TEXT;
    revenue_posting BOOLEAN;
    revenue_currency_mode TEXT;
    revenue_currency_id UUID;
BEGIN
    SELECT status INTO year_status
      FROM academic_years
     WHERE tenant_id = NEW.tenant_id AND id = NEW.academic_year_id AND deleted_at IS NULL;
    IF year_status IS NULL OR year_status = 'closed' THEN
        RAISE EXCEPTION 'Fee structures require an available academic year';
    END IF;

    IF NEW.academic_term_id IS NOT NULL THEN
        SELECT academic_year_id, status INTO term_year_id, term_status
          FROM academic_terms
         WHERE tenant_id = NEW.tenant_id AND id = NEW.academic_term_id AND deleted_at IS NULL;
        IF term_year_id IS NULL OR term_year_id <> NEW.academic_year_id OR term_status = 'closed' THEN
            RAISE EXCEPTION 'The fee term must be available inside its academic year';
        END IF;
    END IF;

    IF NEW.grade_level_id IS NOT NULL THEN
        SELECT status INTO grade_status
          FROM academic_grade_levels
         WHERE tenant_id = NEW.tenant_id AND id = NEW.grade_level_id AND deleted_at IS NULL;
        IF grade_status IS DISTINCT FROM 'active' THEN
            RAISE EXCEPTION 'The fee grade level must be active';
        END IF;
    END IF;

    SELECT status INTO currency_status
      FROM finance_currencies
     WHERE tenant_id = NEW.tenant_id AND id = NEW.currency_id AND deleted_at IS NULL;
    IF currency_status IS DISTINCT FROM 'active' THEN
        RAISE EXCEPTION 'The fee currency must be active';
    END IF;

    SELECT account_type, status, accepts_postings, currency_mode, currency_id
      INTO receivable_type, receivable_status, receivable_posting,
           receivable_currency_mode, receivable_currency_id
      FROM finance_accounts
     WHERE tenant_id = NEW.tenant_id AND id = NEW.receivable_account_id AND deleted_at IS NULL;
    IF receivable_type IS DISTINCT FROM 'asset'
        OR receivable_status IS DISTINCT FROM 'active'
        OR receivable_posting IS DISTINCT FROM TRUE THEN
        RAISE EXCEPTION 'Fee receivables require an active posting asset account';
    END IF;

    SELECT account_type, status, accepts_postings, currency_mode, currency_id
      INTO revenue_type, revenue_status, revenue_posting, revenue_currency_mode, revenue_currency_id
      FROM finance_accounts
     WHERE tenant_id = NEW.tenant_id AND id = NEW.revenue_account_id AND deleted_at IS NULL;
    IF revenue_type IS DISTINCT FROM 'income'
        OR revenue_status IS DISTINCT FROM 'active'
        OR revenue_posting IS DISTINCT FROM TRUE THEN
        RAISE EXCEPTION 'Fee revenue requires an active posting income account';
    END IF;

    IF (receivable_currency_mode = 'single' AND receivable_currency_id <> NEW.currency_id)
        OR (revenue_currency_mode = 'single' AND revenue_currency_id <> NEW.currency_id)
        OR (receivable_currency_mode = 'reporting' AND NOT EXISTS (
            SELECT 1 FROM finance_currencies
             WHERE tenant_id = NEW.tenant_id AND id = NEW.currency_id AND is_reporting
        ))
        OR (revenue_currency_mode = 'reporting' AND NOT EXISTS (
            SELECT 1 FROM finance_currencies
             WHERE tenant_id = NEW.tenant_id AND id = NEW.currency_id AND is_reporting
        )) THEN
        RAISE EXCEPTION 'The fee currency is not allowed by its Finance accounts';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS fees_fee_structure_reference_guard ON fees_fee_structures;
CREATE TRIGGER fees_fee_structure_reference_guard
    BEFORE INSERT OR UPDATE OF academic_year_id, academic_term_id, grade_level_id,
        currency_id, receivable_account_id, revenue_account_id, status
    ON fees_fee_structures
    FOR EACH ROW
    EXECUTE FUNCTION validate_fees_fee_structure_references();

CREATE OR REPLACE FUNCTION validate_fees_fee_structure_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.status <> 'draft' AND (
        NEW.academic_year_id IS DISTINCT FROM OLD.academic_year_id
        OR NEW.academic_term_id IS DISTINCT FROM OLD.academic_term_id
        OR NEW.grade_level_id IS DISTINCT FROM OLD.grade_level_id
        OR NEW.currency_id IS DISTINCT FROM OLD.currency_id
        OR NEW.receivable_account_id IS DISTINCT FROM OLD.receivable_account_id
        OR NEW.revenue_account_id IS DISTINCT FROM OLD.revenue_account_id
        OR NEW.code IS DISTINCT FROM OLD.code
        OR NEW.name IS DISTINCT FROM OLD.name
        OR NEW.description IS DISTINCT FROM OLD.description
        OR NEW.amount_minor IS DISTINCT FROM OLD.amount_minor
        OR NEW.idempotency_key IS DISTINCT FROM OLD.idempotency_key
        OR NEW.created_by IS DISTINCT FROM OLD.created_by
    ) THEN
        RAISE EXCEPTION 'An active or retired fee structure is immutable';
    END IF;

    IF OLD.status = 'draft' AND NEW.status NOT IN ('draft', 'active') THEN
        RAISE EXCEPTION 'A draft fee structure can only be activated';
    ELSIF OLD.status = 'active' AND NEW.status NOT IN ('active', 'retired') THEN
        RAISE EXCEPTION 'An active fee structure can only be retired';
    ELSIF OLD.status = 'retired' THEN
        RAISE EXCEPTION 'A retired fee structure is immutable';
    END IF;

    IF NEW.deleted_at IS NOT NULL AND OLD.deleted_at IS NULL AND OLD.status <> 'draft' THEN
        RAISE EXCEPTION 'Only a draft fee structure can be removed';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS fees_fee_structure_lifecycle_guard ON fees_fee_structures;
CREATE TRIGGER fees_fee_structure_lifecycle_guard
    BEFORE UPDATE ON fees_fee_structures
    FOR EACH ROW
    EXECUTE FUNCTION validate_fees_fee_structure_lifecycle();

DROP TRIGGER IF EXISTS ev_fees_billing_account_sequences ON fees_billing_account_sequences;
CREATE TRIGGER ev_fees_billing_account_sequences
    AFTER INSERT OR UPDATE OR DELETE ON fees_billing_account_sequences
    FOR EACH ROW
    EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_fees_billing_accounts ON fees_billing_accounts;
CREATE TRIGGER ev_fees_billing_accounts
    AFTER INSERT OR UPDATE OR DELETE ON fees_billing_accounts
    FOR EACH ROW
    EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_fees_fee_structures ON fees_fee_structures;
CREATE TRIGGER ev_fees_fee_structures
    AFTER INSERT OR UPDATE OR DELETE ON fees_fee_structures
    FOR EACH ROW
    EXECUTE FUNCTION log_event();
