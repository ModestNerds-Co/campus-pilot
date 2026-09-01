-- Private official-file registry with reviewed classification and disposition.
--
-- Document bytes live in a tenant-prefixed private object bucket. PostgreSQL
-- owns immutable identity, content hash, retention snapshots, and lifecycle evidence.

CREATE TABLE IF NOT EXISTS document_registry_numbering_policies (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    prefix TEXT NOT NULL DEFAULT 'DOC-' CHECK (CHAR_LENGTH(BTRIM(prefix)) BETWEEN 1 AND 20),
    padding SMALLINT NOT NULL DEFAULT 6 CHECK (padding BETWEEN 3 AND 12),
    next_sequence BIGINT NOT NULL DEFAULT 1 CHECK (next_sequence > 0),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

INSERT INTO document_registry_numbering_policies (tenant_id)
SELECT tenant.id FROM tenants AS tenant
ON CONFLICT (tenant_id) DO NOTHING;

CREATE OR REPLACE FUNCTION provision_document_registry_numbering_policy()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO document_registry_numbering_policies (tenant_id)
    VALUES (NEW.id)
    ON CONFLICT (tenant_id) DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_provision_document_registry_numbering_policy ON tenants;
CREATE TRIGGER zz_provision_document_registry_numbering_policy
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION provision_document_registry_numbering_policy();

DROP TRIGGER IF EXISTS update_document_registry_numbering_policies_updated_at
    ON document_registry_numbering_policies;
CREATE TRIGGER update_document_registry_numbering_policies_updated_at
    BEFORE UPDATE ON document_registry_numbering_policies
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS document_registry_series (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    code TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(code)) BETWEEN 1 AND 30),
    name TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(name)) BETWEEN 1 AND 160),
    description TEXT CHECK (description IS NULL OR CHAR_LENGTH(BTRIM(description)) <= 2000),
    retention_trigger TEXT NOT NULL CHECK (retention_trigger IN ('filed', 'closed')),
    retention_period_months SMALLINT CHECK (retention_period_months BETWEEN 1 AND 1200),
    final_disposition TEXT NOT NULL CHECK (final_disposition IN ('review', 'destroy', 'permanent')),
    default_sensitivity TEXT NOT NULL DEFAULT 'internal'
        CHECK (default_sensitivity IN ('general', 'internal', 'confidential', 'restricted')),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (final_disposition = 'permanent' AND retention_period_months IS NULL)
        OR (final_disposition <> 'permanent' AND retention_period_months IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_document_registry_series_code
    ON document_registry_series(tenant_id, LOWER(code)) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_document_registry_series_name
    ON document_registry_series(tenant_id, LOWER(name)) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_document_registry_series_status
    ON document_registry_series(tenant_id, status, name) WHERE deleted_at IS NULL;
DROP TRIGGER IF EXISTS update_document_registry_series_updated_at ON document_registry_series;
CREATE TRIGGER update_document_registry_series_updated_at
    BEFORE UPDATE ON document_registry_series
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS document_registry_files (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    reference TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(reference)) BETWEEN 1 AND 40),
    series_id UUID NOT NULL,
    series_code_snapshot TEXT NOT NULL,
    series_name_snapshot TEXT NOT NULL,
    retention_trigger_snapshot TEXT NOT NULL CHECK (retention_trigger_snapshot IN ('filed', 'closed')),
    retention_period_months_snapshot SMALLINT,
    final_disposition_snapshot TEXT NOT NULL
        CHECK (final_disposition_snapshot IN ('review', 'destroy', 'permanent')),
    sensitivity TEXT NOT NULL
        CHECK (sensitivity IN ('general', 'internal', 'confidential', 'restricted')),
    title TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(title)) BETWEEN 1 AND 240),
    description TEXT CHECK (description IS NULL OR CHAR_LENGTH(BTRIM(description)) <= 4000),
    document_date DATE,
    filed_on DATE NOT NULL,
    retain_until DATE,
    status TEXT NOT NULL DEFAULT 'filed' CHECK (status IN ('filed', 'closed', 'destroyed')),
    original_file_name TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(original_file_name)) BETWEEN 1 AND 255),
    media_type TEXT NOT NULL CHECK (media_type IN ('application/pdf', 'image/jpeg', 'image/png')),
    byte_size BIGINT NOT NULL CHECK (byte_size BETWEEN 1 AND 15728640),
    sha256_hex TEXT NOT NULL CHECK (sha256_hex ~ '^[0-9a-f]{64}$'),
    object_key TEXT,
    scanned_at TIMESTAMPTZ NOT NULL,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    closed_by UUID,
    closed_at TIMESTAMPTZ,
    close_reason TEXT CHECK (close_reason IS NULL OR CHAR_LENGTH(BTRIM(close_reason)) BETWEEN 1 AND 2000),
    destroyed_by UUID,
    destroyed_at TIMESTAMPTZ,
    destruction_reason TEXT CHECK (
        destruction_reason IS NULL OR CHAR_LENGTH(BTRIM(destruction_reason)) BETWEEN 1 AND 2000
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, reference),
    UNIQUE (tenant_id, sha256_hex),
    FOREIGN KEY (series_id, tenant_id) REFERENCES document_registry_series(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (closed_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (destroyed_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'filed' AND closed_by IS NULL AND closed_at IS NULL AND close_reason IS NULL
            AND destroyed_by IS NULL AND destroyed_at IS NULL AND destruction_reason IS NULL
            AND object_key IS NOT NULL)
        OR (status = 'closed' AND closed_by IS NOT NULL AND closed_at IS NOT NULL
            AND close_reason IS NOT NULL AND destroyed_by IS NULL AND destroyed_at IS NULL
            AND destruction_reason IS NULL AND object_key IS NOT NULL)
        OR (status = 'destroyed' AND closed_by IS NOT NULL AND closed_at IS NOT NULL
            AND close_reason IS NOT NULL AND destroyed_by IS NOT NULL AND destroyed_at IS NOT NULL
            AND destruction_reason IS NOT NULL AND object_key IS NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_document_registry_files_worklist
    ON document_registry_files(tenant_id, status, filed_on DESC, id DESC) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_document_registry_files_series
    ON document_registry_files(tenant_id, series_id, status, filed_on DESC) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_document_registry_files_retention
    ON document_registry_files(tenant_id, retain_until, status)
    WHERE deleted_at IS NULL AND retain_until IS NOT NULL AND status <> 'destroyed';
CREATE INDEX IF NOT EXISTS idx_document_registry_files_search
    ON document_registry_files(tenant_id, LOWER(reference), LOWER(title)) WHERE deleted_at IS NULL;
DROP TRIGGER IF EXISTS update_document_registry_files_updated_at ON document_registry_files;
CREATE TRIGGER update_document_registry_files_updated_at
    BEFORE UPDATE ON document_registry_files
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS document_registry_disposition_reviews (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    file_id UUID NOT NULL,
    recommendation TEXT NOT NULL CHECK (recommendation IN ('retain', 'destroy')),
    proposed_retain_until DATE,
    request_reason TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(request_reason)) BETWEEN 1 AND 2000),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'approved', 'rejected', 'executed')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    requested_by UUID NOT NULL,
    reviewed_by UUID,
    reviewed_at TIMESTAMPTZ,
    review_reason TEXT CHECK (review_reason IS NULL OR CHAR_LENGTH(BTRIM(review_reason)) BETWEEN 1 AND 2000),
    executed_by UUID,
    executed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    FOREIGN KEY (file_id, tenant_id) REFERENCES document_registry_files(id, tenant_id),
    FOREIGN KEY (requested_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (reviewed_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (executed_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (recommendation = 'retain' AND proposed_retain_until IS NOT NULL)
        OR (recommendation = 'destroy' AND proposed_retain_until IS NULL)
    ),
    CHECK (
        (status = 'pending' AND reviewed_by IS NULL AND reviewed_at IS NULL AND review_reason IS NULL
            AND executed_by IS NULL AND executed_at IS NULL)
        OR (status = 'approved' AND recommendation = 'destroy' AND reviewed_by IS NOT NULL
            AND reviewed_at IS NOT NULL AND review_reason IS NOT NULL
            AND executed_by IS NULL AND executed_at IS NULL)
        OR (status = 'rejected' AND reviewed_by IS NOT NULL AND reviewed_at IS NOT NULL
            AND review_reason IS NOT NULL AND executed_by IS NULL AND executed_at IS NULL)
        OR (status = 'executed' AND reviewed_by IS NOT NULL AND reviewed_at IS NOT NULL
            AND review_reason IS NOT NULL AND executed_by IS NOT NULL AND executed_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_document_registry_active_disposition_review
    ON document_registry_disposition_reviews(tenant_id, file_id)
    WHERE deleted_at IS NULL AND status IN ('pending', 'approved');
CREATE INDEX IF NOT EXISTS idx_document_registry_disposition_worklist
    ON document_registry_disposition_reviews(tenant_id, status, created_at DESC)
    WHERE deleted_at IS NULL;
DROP TRIGGER IF EXISTS update_document_registry_disposition_reviews_updated_at
    ON document_registry_disposition_reviews;
CREATE TRIGGER update_document_registry_disposition_reviews_updated_at
    BEFORE UPDATE ON document_registry_disposition_reviews
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS document_registry_activity_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    aggregate_type TEXT NOT NULL CHECK (aggregate_type IN ('series', 'file', 'disposition_review', 'numbering_policy')),
    aggregate_id UUID NOT NULL,
    file_id UUID,
    event_type TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(event_type)) BETWEEN 3 AND 100),
    actor_id UUID NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    FOREIGN KEY (file_id, tenant_id) REFERENCES document_registry_files(id, tenant_id),
    FOREIGN KEY (actor_id, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (updated_at = created_at AND deleted_at IS NULL)
);

CREATE INDEX IF NOT EXISTS idx_document_registry_activity_history
    ON document_registry_activity_events(tenant_id, file_id, created_at DESC, id);

CREATE OR REPLACE FUNCTION reject_document_registry_activity_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Document registry activity events are append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS document_registry_activity_events_append_only
    ON document_registry_activity_events;
CREATE TRIGGER document_registry_activity_events_append_only
    BEFORE UPDATE OR DELETE ON document_registry_activity_events
    FOR EACH ROW EXECUTE FUNCTION reject_document_registry_activity_mutation();

INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
SELECT tenant.id, 'records_officer', 'Records Officer',
       'Maintains official filing, classification, retention, and reviewed disposition.',
       ARRAY[
           'document_registry:view', 'document_registry:create', 'document_registry:edit',
           'document_registry:classify', 'document_registry:close',
           'document_registry:dispose', 'document_registry:restricted',
           'document_registry:manage'
       ]::TEXT[], TRUE
  FROM tenants AS tenant
 WHERE NOT EXISTS (
    SELECT 1 FROM roles AS role
     WHERE role.tenant_id = tenant.id AND role.key = 'records_officer'
       AND role.deleted_at IS NULL
 );

CREATE OR REPLACE FUNCTION provision_new_tenant_records_officer()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
    VALUES (
        NEW.id, 'records_officer', 'Records Officer',
        'Maintains official filing, classification, retention, and reviewed disposition.',
        ARRAY[
            'document_registry:view', 'document_registry:create', 'document_registry:edit',
            'document_registry:classify', 'document_registry:close',
            'document_registry:dispose', 'document_registry:restricted',
            'document_registry:manage'
        ]::TEXT[], TRUE
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_provision_new_tenant_records_officer ON tenants;
CREATE TRIGGER zz_provision_new_tenant_records_officer
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION provision_new_tenant_records_officer();

INSERT INTO role_record_scope_grants (tenant_id, role_id, scope_family, scope_kind)
SELECT role.tenant_id, role.id, 'document_registry.records', 'campus'
FROM roles AS role
WHERE role.key = 'records_officer' AND role.deleted_at IS NULL
ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
    WHERE deleted_at IS NULL DO NOTHING;

-- Complete replacement of the canonical seed-role scope function.
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
            ('staff_member', 'library.borrowing', 'self'),
            ('health_officer', 'health.patients', 'campus'),
            ('health_officer', 'health.care', 'campus'),
            ('hostel_officer', 'hostel.occupancy', 'campus'),
            ('hostel_officer', 'hostel.pastoral', 'campus'),
            ('records_officer', 'document_registry.records', 'campus')
    ) AS seed(role_key, scope_family, scope_kind)
    WHERE seed.role_key = NEW.key
    ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
        WHERE deleted_at IS NULL DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
