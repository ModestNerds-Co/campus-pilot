-- Library catalogue, membership, circulation, reservations, and fine evidence.
--
-- SIS and HR remain the owners of learner and employee identity. Library keeps
-- only tenant-bound references plus library-specific membership state. Fees
-- owns charge requests; Library never mutates billing balances or payments.

CREATE TABLE IF NOT EXISTS library_settings (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    accession_prefix TEXT NOT NULL DEFAULT 'LIB'
        CHECK (accession_prefix ~ '^[A-Z0-9][A-Z0-9-]{0,15}$'),
    accession_next_sequence BIGINT NOT NULL DEFAULT 1
        CHECK (accession_next_sequence BETWEEN 1 AND 100000000),
    accession_padding SMALLINT NOT NULL DEFAULT 6
        CHECK (accession_padding BETWEEN 1 AND 8),
    learner_loan_days SMALLINT NOT NULL DEFAULT 14
        CHECK (learner_loan_days BETWEEN 1 AND 365),
    employee_loan_days SMALLINT NOT NULL DEFAULT 21
        CHECK (employee_loan_days BETWEEN 1 AND 365),
    default_loan_limit SMALLINT NOT NULL DEFAULT 4
        CHECK (default_loan_limit BETWEEN 1 AND 100),
    maximum_renewals SMALLINT NOT NULL DEFAULT 1
        CHECK (maximum_renewals BETWEEN 0 AND 20),
    fine_currency_id UUID,
    overdue_fine_minor BIGINT NOT NULL DEFAULT 0
        CHECK (overdue_fine_minor BETWEEN 0 AND 9000000000000000),
    updated_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (fine_currency_id, tenant_id)
        REFERENCES finance_currencies(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (overdue_fine_minor = 0 OR fine_currency_id IS NOT NULL)
);

DROP TRIGGER IF EXISTS update_library_settings_updated_at ON library_settings;
CREATE TRIGGER update_library_settings_updated_at
    BEFORE UPDATE ON library_settings
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

INSERT INTO library_settings (tenant_id)
SELECT id FROM tenants
ON CONFLICT (tenant_id) DO NOTHING;

CREATE OR REPLACE FUNCTION provision_new_tenant_library_settings()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO library_settings (tenant_id) VALUES (NEW.id)
    ON CONFLICT (tenant_id) DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_provision_new_tenant_library_settings ON tenants;
CREATE TRIGGER zz_provision_new_tenant_library_settings
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION provision_new_tenant_library_settings();

CREATE TABLE IF NOT EXISTS library_titles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    isbn TEXT CHECK (isbn IS NULL OR CHAR_LENGTH(BTRIM(isbn)) BETWEEN 10 AND 20),
    title TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(title)) BETWEEN 1 AND 300),
    subtitle TEXT CHECK (subtitle IS NULL OR CHAR_LENGTH(BTRIM(subtitle)) <= 300),
    authors TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[]
        CHECK (CARDINALITY(authors) BETWEEN 1 AND 20),
    publisher TEXT CHECK (publisher IS NULL OR CHAR_LENGTH(BTRIM(publisher)) <= 200),
    publication_year SMALLINT CHECK (publication_year BETWEEN 1000 AND 9999),
    edition TEXT CHECK (edition IS NULL OR CHAR_LENGTH(BTRIM(edition)) <= 80),
    language_code TEXT NOT NULL DEFAULT 'eng'
        CHECK (language_code ~ '^[a-z]{3}$'),
    subject TEXT CHECK (subject IS NULL OR CHAR_LENGTH(BTRIM(subject)) <= 160),
    replacement_cost_minor BIGINT
        CHECK (replacement_cost_minor BETWEEN 1 AND 9000000000000000),
    replacement_currency_id UUID,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'retired')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    retired_by UUID,
    retired_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (replacement_currency_id, tenant_id)
        REFERENCES finance_currencies(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (retired_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK ((replacement_cost_minor IS NULL) = (replacement_currency_id IS NULL)),
    CHECK (
        (status = 'active' AND retired_by IS NULL AND retired_at IS NULL)
        OR (status = 'retired' AND retired_by IS NOT NULL AND retired_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_library_titles_isbn
    ON library_titles(tenant_id, LOWER(isbn))
    WHERE isbn IS NOT NULL AND deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_library_titles_catalogue
    ON library_titles(tenant_id, status, title)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_library_titles_updated_at ON library_titles;
CREATE TRIGGER update_library_titles_updated_at
    BEFORE UPDATE ON library_titles
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS library_copies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    title_id UUID NOT NULL,
    accession_number TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(accession_number)) BETWEEN 1 AND 40),
    barcode TEXT CHECK (barcode IS NULL OR CHAR_LENGTH(BTRIM(barcode)) BETWEEN 1 AND 80),
    location TEXT CHECK (location IS NULL OR CHAR_LENGTH(BTRIM(location)) <= 160),
    condition TEXT NOT NULL DEFAULT 'good'
        CHECK (condition IN ('new', 'good', 'worn', 'damaged')),
    status TEXT NOT NULL DEFAULT 'available'
        CHECK (status IN ('available', 'on_loan', 'reserved', 'lost', 'repair', 'retired')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    retired_by UUID,
    retired_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (title_id, tenant_id) REFERENCES library_titles(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (retired_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'retired' AND retired_by IS NOT NULL AND retired_at IS NOT NULL)
        OR (status <> 'retired' AND retired_by IS NULL AND retired_at IS NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_library_copies_accession
    ON library_copies(tenant_id, LOWER(accession_number)) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_library_copies_barcode
    ON library_copies(tenant_id, LOWER(barcode))
    WHERE barcode IS NOT NULL AND deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_library_copies_title
    ON library_copies(tenant_id, title_id, status) WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_library_copies_updated_at ON library_copies;
CREATE TRIGGER update_library_copies_updated_at
    BEFORE UPDATE ON library_copies
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS library_memberships (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    learner_id UUID,
    employee_id UUID,
    card_number TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(card_number)) BETWEEN 1 AND 40),
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'suspended', 'closed')),
    loan_limit SMALLINT NOT NULL CHECK (loan_limit BETWEEN 1 AND 100),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    closed_by UUID,
    closed_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (learner_id, tenant_id) REFERENCES learners(id, tenant_id),
    FOREIGN KEY (employee_id, tenant_id) REFERENCES employees(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (closed_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK ((learner_id IS NOT NULL)::INTEGER + (employee_id IS NOT NULL)::INTEGER = 1),
    CHECK (
        (status = 'closed' AND closed_by IS NOT NULL AND closed_at IS NOT NULL)
        OR (status <> 'closed' AND closed_by IS NULL AND closed_at IS NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_library_memberships_learner
    ON library_memberships(tenant_id, learner_id)
    WHERE learner_id IS NOT NULL AND deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_library_memberships_employee
    ON library_memberships(tenant_id, employee_id)
    WHERE employee_id IS NOT NULL AND deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_library_memberships_card
    ON library_memberships(tenant_id, LOWER(card_number)) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_library_memberships_status
    ON library_memberships(tenant_id, status, updated_at DESC) WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_library_memberships_updated_at ON library_memberships;
CREATE TRIGGER update_library_memberships_updated_at
    BEFORE UPDATE ON library_memberships
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS library_holds (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    title_id UUID NOT NULL,
    membership_id UUID NOT NULL,
    copy_id UUID,
    queue_position BIGINT NOT NULL CHECK (queue_position > 0),
    status TEXT NOT NULL DEFAULT 'waiting'
        CHECK (status IN ('waiting', 'ready', 'fulfilled', 'cancelled', 'expired')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    placed_by UUID NOT NULL,
    ready_by UUID,
    ready_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    resolved_by UUID,
    resolved_at TIMESTAMPTZ,
    resolution_reason TEXT CHECK (
        resolution_reason IS NULL OR CHAR_LENGTH(BTRIM(resolution_reason)) BETWEEN 1 AND 1000
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (title_id, tenant_id) REFERENCES library_titles(id, tenant_id),
    FOREIGN KEY (membership_id, tenant_id) REFERENCES library_memberships(id, tenant_id),
    FOREIGN KEY (copy_id, tenant_id) REFERENCES library_copies(id, tenant_id),
    FOREIGN KEY (placed_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (ready_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (resolved_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'waiting' AND copy_id IS NULL AND ready_by IS NULL AND ready_at IS NULL
            AND expires_at IS NULL AND resolved_by IS NULL AND resolved_at IS NULL
            AND resolution_reason IS NULL)
        OR (status = 'ready' AND copy_id IS NOT NULL AND ready_by IS NOT NULL
            AND ready_at IS NOT NULL AND expires_at IS NOT NULL
            AND resolved_by IS NULL AND resolved_at IS NULL AND resolution_reason IS NULL)
        OR (status IN ('fulfilled', 'cancelled', 'expired')
            AND resolved_by IS NOT NULL AND resolved_at IS NOT NULL
            AND resolution_reason IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_library_holds_active_member_title
    ON library_holds(tenant_id, membership_id, title_id)
    WHERE status IN ('waiting', 'ready');
CREATE UNIQUE INDEX IF NOT EXISTS idx_library_holds_active_copy
    ON library_holds(tenant_id, copy_id)
    WHERE copy_id IS NOT NULL AND status = 'ready';
CREATE INDEX IF NOT EXISTS idx_library_holds_queue
    ON library_holds(tenant_id, title_id, status, queue_position);

DROP TRIGGER IF EXISTS update_library_holds_updated_at ON library_holds;
CREATE TRIGGER update_library_holds_updated_at
    BEFORE UPDATE ON library_holds
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS library_loans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    copy_id UUID NOT NULL,
    membership_id UUID NOT NULL,
    fulfilled_hold_id UUID,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'returned', 'lost')),
    checked_out_on DATE NOT NULL,
    due_on DATE NOT NULL,
    returned_on DATE,
    renewal_count SMALLINT NOT NULL DEFAULT 0 CHECK (renewal_count BETWEEN 0 AND 20),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    checked_out_by UUID NOT NULL,
    returned_by UUID,
    lost_by UUID,
    notes TEXT CHECK (notes IS NULL OR CHAR_LENGTH(BTRIM(notes)) <= 1000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (copy_id, tenant_id) REFERENCES library_copies(id, tenant_id),
    FOREIGN KEY (membership_id, tenant_id) REFERENCES library_memberships(id, tenant_id),
    FOREIGN KEY (fulfilled_hold_id, tenant_id) REFERENCES library_holds(id, tenant_id),
    FOREIGN KEY (checked_out_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (returned_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (lost_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (due_on >= checked_out_on),
    CHECK (
        (status = 'active' AND returned_on IS NULL AND returned_by IS NULL AND lost_by IS NULL)
        OR (status = 'returned' AND returned_on IS NOT NULL AND returned_by IS NOT NULL
            AND lost_by IS NULL AND returned_on >= checked_out_on)
        OR (status = 'lost' AND returned_on IS NULL AND returned_by IS NULL AND lost_by IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_library_loans_active_copy
    ON library_loans(tenant_id, copy_id) WHERE status = 'active';
CREATE INDEX IF NOT EXISTS idx_library_loans_member
    ON library_loans(tenant_id, membership_id, status, due_on DESC);
CREATE INDEX IF NOT EXISTS idx_library_loans_overdue
    ON library_loans(tenant_id, due_on, membership_id) WHERE status = 'active';

DROP TRIGGER IF EXISTS update_library_loans_updated_at ON library_loans;
CREATE TRIGGER update_library_loans_updated_at
    BEFORE UPDATE ON library_loans
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS fees_charge_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    billing_account_id UUID NOT NULL,
    currency_id UUID NOT NULL,
    source_module TEXT NOT NULL CHECK (source_module ~ '^[a-z][a-z0-9_]{1,63}$'),
    source_record_id UUID NOT NULL,
    description TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(description)) BETWEEN 1 AND 500),
    amount_minor BIGINT NOT NULL CHECK (amount_minor BETWEEN 1 AND 9000000000000000),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'accepted', 'rejected', 'invoiced')),
    submitted_by UUID NOT NULL,
    decided_by UUID,
    decided_at TIMESTAMPTZ,
    decision_reason TEXT CHECK (
        decision_reason IS NULL OR CHAR_LENGTH(BTRIM(decision_reason)) BETWEEN 1 AND 1000
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (billing_account_id, tenant_id)
        REFERENCES fees_billing_accounts(id, tenant_id),
    FOREIGN KEY (currency_id, tenant_id) REFERENCES finance_currencies(id, tenant_id),
    FOREIGN KEY (submitted_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (decided_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'pending' AND decided_by IS NULL AND decided_at IS NULL AND decision_reason IS NULL)
        OR (status <> 'pending' AND decided_by IS NOT NULL AND decided_at IS NOT NULL
            AND decision_reason IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_fees_charge_requests_source
    ON fees_charge_requests(tenant_id, source_module, source_record_id);
CREATE INDEX IF NOT EXISTS idx_fees_charge_requests_worklist
    ON fees_charge_requests(tenant_id, status, created_at);

DROP TRIGGER IF EXISTS update_fees_charge_requests_updated_at ON fees_charge_requests;
CREATE TRIGGER update_fees_charge_requests_updated_at
    BEFORE UPDATE ON fees_charge_requests
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS library_fines (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    loan_id UUID NOT NULL,
    membership_id UUID NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('overdue', 'replacement')),
    currency_id UUID NOT NULL,
    amount_minor BIGINT NOT NULL CHECK (amount_minor BETWEEN 1 AND 9000000000000000),
    status TEXT NOT NULL DEFAULT 'assessed'
        CHECK (status IN ('assessed', 'submitted_to_fees', 'waived')),
    assessed_days INTEGER CHECK (assessed_days IS NULL OR assessed_days > 0),
    fees_charge_request_id UUID,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    assessed_by UUID NOT NULL,
    submitted_by UUID,
    submitted_at TIMESTAMPTZ,
    waived_by UUID,
    waived_at TIMESTAMPTZ,
    waiver_reason TEXT CHECK (
        waiver_reason IS NULL OR CHAR_LENGTH(BTRIM(waiver_reason)) BETWEEN 1 AND 1000
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (loan_id, tenant_id) REFERENCES library_loans(id, tenant_id),
    FOREIGN KEY (membership_id, tenant_id) REFERENCES library_memberships(id, tenant_id),
    FOREIGN KEY (currency_id, tenant_id) REFERENCES finance_currencies(id, tenant_id),
    FOREIGN KEY (fees_charge_request_id, tenant_id) REFERENCES fees_charge_requests(id, tenant_id),
    FOREIGN KEY (assessed_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (submitted_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (waived_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK ((kind = 'overdue' AND assessed_days IS NOT NULL) OR (kind = 'replacement' AND assessed_days IS NULL)),
    CHECK (
        (status = 'assessed' AND fees_charge_request_id IS NULL
            AND submitted_by IS NULL AND submitted_at IS NULL
            AND waived_by IS NULL AND waived_at IS NULL AND waiver_reason IS NULL)
        OR (status = 'submitted_to_fees' AND fees_charge_request_id IS NOT NULL
            AND submitted_by IS NOT NULL AND submitted_at IS NOT NULL
            AND waived_by IS NULL AND waived_at IS NULL AND waiver_reason IS NULL)
        OR (status = 'waived' AND fees_charge_request_id IS NULL
            AND submitted_by IS NULL AND submitted_at IS NULL
            AND waived_by IS NOT NULL AND waived_at IS NOT NULL AND waiver_reason IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_library_fines_loan_kind
    ON library_fines(tenant_id, loan_id, kind);
CREATE INDEX IF NOT EXISTS idx_library_fines_member
    ON library_fines(tenant_id, membership_id, status, created_at DESC);

DROP TRIGGER IF EXISTS update_library_fines_updated_at ON library_fines;
CREATE TRIGGER update_library_fines_updated_at
    BEFORE UPDATE ON library_fines
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS library_activity_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    aggregate_type TEXT NOT NULL
        CHECK (aggregate_type IN ('title', 'copy', 'membership', 'loan', 'hold', 'fine', 'settings')),
    aggregate_id UUID NOT NULL,
    event_type TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(event_type)) BETWEEN 3 AND 80),
    actor_id UUID NOT NULL,
    reason TEXT CHECK (reason IS NULL OR CHAR_LENGTH(BTRIM(reason)) BETWEEN 1 AND 1000),
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (actor_id, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE INDEX IF NOT EXISTS idx_library_activity_history
    ON library_activity_events(tenant_id, aggregate_type, aggregate_id, created_at, id);

CREATE OR REPLACE FUNCTION reject_library_activity_event_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Library activity events are append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS library_activity_events_append_only ON library_activity_events;
CREATE TRIGGER library_activity_events_append_only
    BEFORE UPDATE OR DELETE ON library_activity_events
    FOR EACH ROW EXECUTE FUNCTION reject_library_activity_event_mutation();

-- Campus roles receive explicit operation permissions. Borrower roles can read
-- the catalogue and their own records; circulation remains a librarian action.
UPDATE roles
SET permissions = ARRAY(
        SELECT DISTINCT permission
        FROM UNNEST(permissions || ARRAY['library:borrow', 'library:circulate', 'library:manage']::TEXT[]) AS permission
    ),
    updated_at = NOW()
WHERE key = 'librarian' AND deleted_at IS NULL;

UPDATE roles
SET permissions = ARRAY(
        SELECT DISTINCT permission
        FROM UNNEST(permissions || ARRAY['library:borrow']::TEXT[]) AS permission
    ),
    updated_at = NOW()
WHERE key IN ('student', 'teacher', 'staff_member') AND deleted_at IS NULL;

CREATE OR REPLACE FUNCTION provision_new_tenant_library_access()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE roles
       SET permissions = ARRAY(
           SELECT DISTINCT permission
             FROM UNNEST(
                 permissions || ARRAY['library:borrow', 'library:circulate', 'library:manage']::TEXT[]
             ) AS permission
            ORDER BY permission
       ), updated_at = NOW()
     WHERE tenant_id = NEW.id AND key = 'librarian' AND deleted_at IS NULL;

    UPDATE roles
       SET permissions = ARRAY(
           SELECT DISTINCT permission
             FROM UNNEST(permissions || ARRAY['library:borrow']::TEXT[]) AS permission
            ORDER BY permission
       ), updated_at = NOW()
     WHERE tenant_id = NEW.id AND key IN ('student', 'teacher', 'staff_member')
       AND deleted_at IS NULL;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_provision_new_tenant_library_access ON tenants;
CREATE TRIGGER zz_provision_new_tenant_library_access
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION provision_new_tenant_library_access();

-- Seed record scopes for existing campuses.
INSERT INTO role_record_scope_grants (tenant_id, role_id, scope_family, scope_kind)
SELECT role.tenant_id, role.id, seed.scope_family, seed.scope_kind
FROM roles AS role
INNER JOIN (
    VALUES
        ('librarian', 'library.members', 'campus'),
        ('librarian', 'library.borrowing', 'campus'),
        ('student', 'library.members', 'self'),
        ('student', 'library.borrowing', 'self'),
        ('teacher', 'library.members', 'self'),
        ('teacher', 'library.borrowing', 'self'),
        ('staff_member', 'library.members', 'self'),
        ('staff_member', 'library.borrowing', 'self')
) AS seed(role_key, scope_family, scope_kind)
    ON role.key = seed.role_key AND role.deleted_at IS NULL
ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
    WHERE deleted_at IS NULL DO NOTHING;

-- Keep future seeded roles aligned with the same record-scope intent. This is
-- a complete replacement of the existing provisioning function, not a second
-- trigger or a parallel rules file.
CREATE OR REPLACE FUNCTION provision_seed_role_record_scopes()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO role_record_scope_grants (tenant_id, role_id, scope_family, scope_kind)
    SELECT NEW.tenant_id, NEW.id, seed.scope_family, seed.scope_kind
    FROM (
        VALUES
            ('registrar', 'sis.account_linking', 'campus'),
            ('registrar', 'sis.imports', 'campus'),
            ('registrar', 'sis.learners', 'campus'),
            ('registrar', 'sis.guardians', 'campus'),
            ('registrar', 'sis.guardian_relationships', 'campus'),
            ('registrar', 'sis.applications', 'campus'),
            ('registrar', 'sis.enrolments', 'campus'),
            ('finance_officer', 'fees.billing', 'campus'),
            ('finance_officer', 'fees.learner_candidates', 'campus'),
            ('finance_officer', 'fees.imports', 'campus'),
            ('finance_officer', 'procurement.requester_candidates', 'campus'),
            ('finance_officer', 'procurement.requests', 'campus'),
            ('teacher', 'academics.teachers', 'self'),
            ('teacher', 'academics.teaching_assignments', 'assigned'),
            ('teacher', 'academics.assessment_components', 'assigned'),
            ('teacher', 'sis.learners', 'assigned'),
            ('teacher', 'sis.guardians', 'assigned'),
            ('teacher', 'sis.guardian_relationships', 'assigned'),
            ('teacher', 'sis.enrolments', 'assigned'),
            ('student', 'fees.billing', 'self'),
            ('staff_member', 'hr.employees', 'self'),
            ('staff_member', 'hr.engagements', 'self'),
            ('staff_member', 'hr.availability', 'self'),
            ('librarian', 'library.members', 'campus'),
            ('librarian', 'library.borrowing', 'campus'),
            ('student', 'library.members', 'self'),
            ('student', 'library.borrowing', 'self'),
            ('teacher', 'library.members', 'self'),
            ('teacher', 'library.borrowing', 'self'),
            ('staff_member', 'library.members', 'self'),
            ('staff_member', 'library.borrowing', 'self')
    ) AS seed(role_key, scope_family, scope_kind)
    WHERE seed.role_key = NEW.key
    ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
        WHERE deleted_at IS NULL DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
