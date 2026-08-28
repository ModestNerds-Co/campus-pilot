-- Multi-currency Finance foundation.
--
-- Finance owns currency configuration and the chart of accounts. No source
-- module writes ledger rows directly; journals and posting requests arrive in
-- later migrations after these stable references exist.

CREATE TABLE IF NOT EXISTS finance_currencies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    code TEXT NOT NULL CHECK (code ~ '^[A-Z]{3}$'),
    name TEXT NOT NULL CHECK (BTRIM(name) <> ''),
    symbol TEXT CHECK (symbol IS NULL OR CHAR_LENGTH(BTRIM(symbol)) BETWEEN 1 AND 8),
    minor_units SMALLINT NOT NULL DEFAULT 2 CHECK (minor_units BETWEEN 0 AND 4),
    is_reporting BOOLEAN NOT NULL DEFAULT FALSE,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive')),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CHECK (NOT is_reporting OR (status = 'active' AND deleted_at IS NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_finance_currencies_tenant_code
    ON finance_currencies(tenant_id, code) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_finance_currencies_one_reporting
    ON finance_currencies(tenant_id) WHERE is_reporting AND deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_finance_currencies_tenant_status
    ON finance_currencies(tenant_id, status) WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_finance_currencies_updated_at ON finance_currencies;
CREATE TRIGGER update_finance_currencies_updated_at
    BEFORE UPDATE ON finance_currencies
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE OR REPLACE FUNCTION enforce_finance_reporting_currency()
RETURNS TRIGGER AS $$
DECLARE
    scoped_tenant_id UUID;
    active_count INTEGER;
    reporting_count INTEGER;
BEGIN
    scoped_tenant_id := COALESCE(NEW.tenant_id, OLD.tenant_id);

    SELECT COUNT(*), COUNT(*) FILTER (WHERE is_reporting)
      INTO active_count, reporting_count
      FROM finance_currencies
     WHERE tenant_id = scoped_tenant_id
       AND status = 'active'
       AND deleted_at IS NULL;

    IF active_count > 0 AND reporting_count <> 1 THEN
        RAISE EXCEPTION 'Finance requires exactly one active reporting currency';
    END IF;

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS finance_reporting_currency_guard ON finance_currencies;
CREATE CONSTRAINT TRIGGER finance_reporting_currency_guard
    AFTER INSERT OR UPDATE OR DELETE ON finance_currencies
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW
    EXECUTE FUNCTION enforce_finance_reporting_currency();

CREATE TABLE IF NOT EXISTS finance_accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    parent_account_id UUID,
    currency_id UUID,
    code TEXT NOT NULL CHECK (BTRIM(code) <> ''),
    name TEXT NOT NULL CHECK (BTRIM(name) <> ''),
    description TEXT,
    account_type TEXT NOT NULL
        CHECK (account_type IN ('asset', 'liability', 'equity', 'income', 'expense')),
    normal_balance TEXT GENERATED ALWAYS AS (
        CASE
            WHEN account_type IN ('asset', 'expense') THEN 'debit'
            ELSE 'credit'
        END
    ) STORED,
    currency_mode TEXT NOT NULL DEFAULT 'reporting'
        CHECK (currency_mode IN ('reporting', 'single', 'multi')),
    accepts_postings BOOLEAN NOT NULL DEFAULT TRUE,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive')),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (parent_account_id, tenant_id)
        REFERENCES finance_accounts(id, tenant_id),
    FOREIGN KEY (currency_id, tenant_id)
        REFERENCES finance_currencies(id, tenant_id),
    CHECK (
        (currency_mode = 'single' AND currency_id IS NOT NULL)
        OR (currency_mode IN ('reporting', 'multi') AND currency_id IS NULL)
    ),
    CHECK (parent_account_id IS NULL OR parent_account_id <> id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_finance_accounts_tenant_code
    ON finance_accounts(tenant_id, LOWER(code)) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_finance_accounts_tenant_name
    ON finance_accounts(tenant_id, LOWER(name)) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_finance_accounts_tenant_type
    ON finance_accounts(tenant_id, account_type, status) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_finance_accounts_parent
    ON finance_accounts(tenant_id, parent_account_id) WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_finance_accounts_updated_at ON finance_accounts;
CREATE TRIGGER update_finance_accounts_updated_at
    BEFORE UPDATE ON finance_accounts
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE OR REPLACE FUNCTION validate_finance_account_structure()
RETURNS TRIGGER AS $$
DECLARE
    parent_type TEXT;
    parent_accepts_postings BOOLEAN;
    selected_currency_active BOOLEAN;
    creates_cycle BOOLEAN;
BEGIN
    IF NEW.currency_mode = 'single' THEN
        SELECT status = 'active' AND deleted_at IS NULL
          INTO selected_currency_active
          FROM finance_currencies
         WHERE id = NEW.currency_id
           AND tenant_id = NEW.tenant_id;

        IF selected_currency_active IS DISTINCT FROM TRUE THEN
            RAISE EXCEPTION 'A single-currency account requires an active campus currency';
        END IF;
    END IF;

    IF NEW.parent_account_id IS NOT NULL THEN
        SELECT account_type, accepts_postings
          INTO parent_type, parent_accepts_postings
          FROM finance_accounts
         WHERE id = NEW.parent_account_id
           AND tenant_id = NEW.tenant_id
           AND deleted_at IS NULL;

        IF parent_type IS NULL THEN
            RAISE EXCEPTION 'The parent account is not available for this campus';
        END IF;
        IF parent_type <> NEW.account_type THEN
            RAISE EXCEPTION 'A parent account must use the same account type';
        END IF;
        IF parent_accepts_postings THEN
            RAISE EXCEPTION 'A posting account cannot contain child accounts';
        END IF;

        WITH RECURSIVE ancestors AS (
            SELECT parent_account_id
              FROM finance_accounts
             WHERE id = NEW.parent_account_id
               AND tenant_id = NEW.tenant_id
            UNION ALL
            SELECT account.parent_account_id
              FROM finance_accounts AS account
              JOIN ancestors ON account.id = ancestors.parent_account_id
             WHERE account.tenant_id = NEW.tenant_id
               AND account.deleted_at IS NULL
        )
        SELECT EXISTS (
            SELECT 1 FROM ancestors WHERE parent_account_id = NEW.id
        ) INTO creates_cycle;

        IF creates_cycle THEN
            RAISE EXCEPTION 'An account cannot be its own ancestor';
        END IF;
    END IF;

    IF NEW.accepts_postings AND EXISTS (
        SELECT 1
          FROM finance_accounts
         WHERE tenant_id = NEW.tenant_id
           AND parent_account_id = NEW.id
           AND deleted_at IS NULL
    ) THEN
        RAISE EXCEPTION 'An account with child accounts cannot accept postings';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS finance_account_structure_guard ON finance_accounts;
CREATE TRIGGER finance_account_structure_guard
    BEFORE INSERT OR UPDATE ON finance_accounts
    FOR EACH ROW
    EXECUTE FUNCTION validate_finance_account_structure();

DROP TRIGGER IF EXISTS ev_finance_currencies ON finance_currencies;
CREATE TRIGGER ev_finance_currencies
    AFTER INSERT OR UPDATE OR DELETE ON finance_currencies
    FOR EACH ROW
    EXECUTE FUNCTION log_event();

DROP TRIGGER IF EXISTS ev_finance_accounts ON finance_accounts;
CREATE TRIGGER ev_finance_accounts
    AFTER INSERT OR UPDATE OR DELETE ON finance_accounts
    FOR EACH ROW
    EXECUTE FUNCTION log_event();
