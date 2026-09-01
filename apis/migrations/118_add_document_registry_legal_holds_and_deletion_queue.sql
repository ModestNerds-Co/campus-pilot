-- Add legal holds and a durable, leased object-deletion queue to Document Registry.
--
-- A destruction decision is only complete after private object storage confirms
-- deletion and the database records the terminal file, review, and job states.

ALTER TABLE document_registry_disposition_reviews
    DROP CONSTRAINT IF EXISTS document_registry_disposition_reviews_status_check;
ALTER TABLE document_registry_disposition_reviews
    ADD CONSTRAINT document_registry_disposition_reviews_status_check
    CHECK (status IN ('pending', 'approved', 'rejected', 'deletion_pending', 'executed'));

ALTER TABLE document_registry_disposition_reviews
    DROP CONSTRAINT IF EXISTS document_registry_disposition_reviews_check1;
ALTER TABLE document_registry_disposition_reviews
    ADD CONSTRAINT document_registry_disposition_reviews_state_check
    CHECK (
        (status = 'pending' AND reviewed_by IS NULL AND reviewed_at IS NULL
            AND review_reason IS NULL AND executed_by IS NULL AND executed_at IS NULL)
        OR (status IN ('approved', 'deletion_pending') AND recommendation = 'destroy'
            AND reviewed_by IS NOT NULL AND reviewed_at IS NOT NULL
            AND review_reason IS NOT NULL AND executed_by IS NULL AND executed_at IS NULL)
        OR (status = 'rejected' AND reviewed_by IS NOT NULL AND reviewed_at IS NOT NULL
            AND review_reason IS NOT NULL AND executed_by IS NULL AND executed_at IS NULL)
        OR (status = 'executed' AND reviewed_by IS NOT NULL AND reviewed_at IS NOT NULL
            AND review_reason IS NOT NULL AND executed_by IS NOT NULL AND executed_at IS NOT NULL)
    );

DROP INDEX IF EXISTS idx_document_registry_active_disposition_review;
CREATE UNIQUE INDEX idx_document_registry_active_disposition_review
    ON document_registry_disposition_reviews(tenant_id, file_id)
    WHERE deleted_at IS NULL AND status IN ('pending', 'approved', 'deletion_pending');

CREATE TABLE IF NOT EXISTS document_registry_legal_holds (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    file_id UUID NOT NULL,
    reference TEXT,
    reason TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(reason)) BETWEEN 1 AND 2000),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'released')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    applied_by UUID NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    released_by UUID,
    released_at TIMESTAMPTZ,
    release_reason TEXT CHECK (
        release_reason IS NULL OR CHAR_LENGTH(BTRIM(release_reason)) BETWEEN 1 AND 2000
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    FOREIGN KEY (file_id, tenant_id) REFERENCES document_registry_files(id, tenant_id),
    FOREIGN KEY (applied_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (released_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (reference IS NULL OR CHAR_LENGTH(BTRIM(reference)) BETWEEN 1 AND 120),
    CHECK (
        (status = 'active' AND released_by IS NULL AND released_at IS NULL
            AND release_reason IS NULL)
        OR (status = 'released' AND released_by IS NOT NULL AND released_at IS NOT NULL
            AND release_reason IS NOT NULL)
    ),
    CHECK (deleted_at IS NULL)
);

CREATE INDEX IF NOT EXISTS idx_document_registry_legal_hold_worklist
    ON document_registry_legal_holds(tenant_id, status, applied_at DESC, id);
CREATE INDEX IF NOT EXISTS idx_document_registry_active_file_holds
    ON document_registry_legal_holds(tenant_id, file_id, applied_at DESC)
    WHERE status = 'active';
DROP TRIGGER IF EXISTS update_document_registry_legal_holds_updated_at
    ON document_registry_legal_holds;
CREATE TRIGGER update_document_registry_legal_holds_updated_at
    BEFORE UPDATE ON document_registry_legal_holds
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS document_registry_deletion_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    review_id UUID NOT NULL,
    file_id UUID NOT NULL,
    object_key TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(object_key)) BETWEEN 1 AND 1024),
    destruction_reason TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(destruction_reason)) BETWEEN 1 AND 2000),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'processing', 'retry', 'blocked', 'completed')),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    lease_token UUID,
    lease_expires_at TIMESTAMPTZ,
    last_error_code TEXT CHECK (
        last_error_code IS NULL OR CHAR_LENGTH(BTRIM(last_error_code)) BETWEEN 1 AND 100
    ),
    requested_by UUID NOT NULL,
    requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, review_id),
    UNIQUE (tenant_id, file_id),
    FOREIGN KEY (review_id, tenant_id)
        REFERENCES document_registry_disposition_reviews(id, tenant_id),
    FOREIGN KEY (file_id, tenant_id) REFERENCES document_registry_files(id, tenant_id),
    FOREIGN KEY (requested_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status IN ('pending', 'retry', 'blocked') AND lease_token IS NULL
            AND lease_expires_at IS NULL AND completed_at IS NULL)
        OR (status = 'processing' AND lease_token IS NOT NULL
            AND lease_expires_at IS NOT NULL AND completed_at IS NULL)
        OR (status = 'completed' AND lease_token IS NULL
            AND lease_expires_at IS NULL AND completed_at IS NOT NULL)
    ),
    CHECK (deleted_at IS NULL)
);

CREATE INDEX IF NOT EXISTS idx_document_registry_deletion_jobs_claim
    ON document_registry_deletion_jobs(next_attempt_at, requested_at, id)
    WHERE status IN ('pending', 'retry');
CREATE INDEX IF NOT EXISTS idx_document_registry_deletion_jobs_recover
    ON document_registry_deletion_jobs(lease_expires_at, id)
    WHERE status = 'processing';
DROP TRIGGER IF EXISTS update_document_registry_deletion_jobs_updated_at
    ON document_registry_deletion_jobs;
CREATE TRIGGER update_document_registry_deletion_jobs_updated_at
    BEFORE UPDATE ON document_registry_deletion_jobs
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

ALTER TABLE document_registry_activity_events
    DROP CONSTRAINT IF EXISTS document_registry_activity_events_aggregate_type_check;
ALTER TABLE document_registry_activity_events
    ADD CONSTRAINT document_registry_activity_events_aggregate_type_check
    CHECK (aggregate_type IN (
        'series', 'file', 'disposition_review', 'numbering_policy', 'legal_hold', 'deletion_job'
    ));

CREATE OR REPLACE FUNCTION enforce_document_registry_legal_hold_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.status = 'released' THEN
        RAISE EXCEPTION 'Released legal holds are final and cannot be changed'
            USING ERRCODE = '23514';
    END IF;

    IF NEW.status <> 'released' THEN
        RAISE EXCEPTION 'An active legal hold may only be released'
            USING ERRCODE = '23514';
    END IF;

    IF (
        to_jsonb(NEW) - ARRAY[
            'status', 'version', 'released_by', 'released_at', 'release_reason', 'updated_at'
        ]::TEXT[]
    ) IS DISTINCT FROM (
        to_jsonb(OLD) - ARRAY[
            'status', 'version', 'released_by', 'released_at', 'release_reason', 'updated_at'
        ]::TEXT[]
    ) THEN
        RAISE EXCEPTION 'Legal hold evidence is immutable'
            USING ERRCODE = '23514';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS document_registry_legal_holds_lifecycle
    ON document_registry_legal_holds;
CREATE TRIGGER document_registry_legal_holds_lifecycle
    BEFORE UPDATE ON document_registry_legal_holds
    FOR EACH ROW EXECUTE FUNCTION enforce_document_registry_legal_hold_lifecycle();

CREATE OR REPLACE FUNCTION enforce_document_registry_deletion_job_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.status <> 'pending' THEN
            RAISE EXCEPTION 'A document deletion job must start pending'
                USING ERRCODE = '23514';
        END IF;
        IF EXISTS (
            SELECT 1
              FROM document_registry_legal_holds hold
             WHERE hold.tenant_id = NEW.tenant_id
               AND hold.file_id = NEW.file_id
               AND hold.status = 'active'
        ) THEN
            RAISE EXCEPTION 'An active legal hold blocks document destruction'
                USING ERRCODE = '23514';
        END IF;
        RETURN NEW;
    END IF;

    IF OLD.status = 'completed' THEN
        RAISE EXCEPTION 'Completed document deletion jobs are final and cannot be changed'
            USING ERRCODE = '23514';
    END IF;

    IF OLD.status IN ('pending', 'retry') AND NEW.status NOT IN ('processing', 'blocked') THEN
        RAISE EXCEPTION 'A ready document deletion job may only be claimed or blocked'
            USING ERRCODE = '23514';
    END IF;

    IF OLD.status = 'blocked' AND NEW.status NOT IN ('pending', 'processing') THEN
        RAISE EXCEPTION 'A blocked document deletion job may only be released or claimed'
            USING ERRCODE = '23514';
    END IF;

    IF OLD.status = 'processing' AND NEW.status NOT IN ('retry', 'completed') THEN
        RAISE EXCEPTION 'A claimed document deletion job may only retry or complete'
            USING ERRCODE = '23514';
    END IF;

    IF (
        to_jsonb(NEW) - ARRAY[
            'status', 'attempt_count', 'next_attempt_at', 'lease_token', 'lease_expires_at',
            'last_error_code', 'completed_at', 'version', 'updated_at'
        ]::TEXT[]
    ) IS DISTINCT FROM (
        to_jsonb(OLD) - ARRAY[
            'status', 'attempt_count', 'next_attempt_at', 'lease_token', 'lease_expires_at',
            'last_error_code', 'completed_at', 'version', 'updated_at'
        ]::TEXT[]
    ) THEN
        RAISE EXCEPTION 'Document deletion request evidence is immutable'
            USING ERRCODE = '23514';
    END IF;

    IF NEW.status IN ('pending', 'processing') AND EXISTS (
        SELECT 1
          FROM document_registry_legal_holds hold
         WHERE hold.tenant_id = NEW.tenant_id
           AND hold.file_id = NEW.file_id
           AND hold.status = 'active'
    ) THEN
        RAISE EXCEPTION 'An active legal hold blocks document destruction'
            USING ERRCODE = '23514';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS document_registry_deletion_jobs_lifecycle
    ON document_registry_deletion_jobs;
CREATE TRIGGER document_registry_deletion_jobs_lifecycle
    BEFORE INSERT OR UPDATE ON document_registry_deletion_jobs
    FOR EACH ROW EXECUTE FUNCTION enforce_document_registry_deletion_job_lifecycle();

CREATE OR REPLACE FUNCTION enforce_document_registry_review_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.status IN ('rejected', 'executed') THEN
        RAISE EXCEPTION 'Completed disposition reviews are final and cannot be changed'
            USING ERRCODE = '23514';
    END IF;

    IF OLD.status = 'approved' AND NEW.status <> 'deletion_pending' THEN
        RAISE EXCEPTION 'An approved destruction may only be queued'
            USING ERRCODE = '23514';
    END IF;

    IF OLD.status = 'deletion_pending' AND NEW.status <> 'executed' THEN
        RAISE EXCEPTION 'A queued destruction may only be completed'
            USING ERRCODE = '23514';
    END IF;

    IF OLD.status = 'pending' AND NEW.status NOT IN ('approved', 'rejected', 'executed') THEN
        RAISE EXCEPTION 'A pending disposition review must receive a final decision'
            USING ERRCODE = '23514';
    END IF;

    IF NEW.recommendation = 'destroy'
       AND NEW.status IN ('approved', 'deletion_pending', 'executed')
       AND EXISTS (
            SELECT 1
              FROM document_registry_legal_holds hold
             WHERE hold.tenant_id = NEW.tenant_id
               AND hold.file_id = NEW.file_id
               AND hold.status = 'active'
       ) THEN
        RAISE EXCEPTION 'An active legal hold blocks document destruction'
            USING ERRCODE = '23514';
    END IF;

    IF (
        to_jsonb(NEW) - ARRAY[
            'status', 'version', 'reviewed_by', 'reviewed_at', 'review_reason',
            'executed_by', 'executed_at', 'updated_at'
        ]::TEXT[]
    ) IS DISTINCT FROM (
        to_jsonb(OLD) - ARRAY[
            'status', 'version', 'reviewed_by', 'reviewed_at', 'review_reason',
            'executed_by', 'executed_at', 'updated_at'
        ]::TEXT[]
    ) THEN
        RAISE EXCEPTION 'Disposition review request evidence is immutable'
            USING ERRCODE = '23514';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS document_registry_reviews_lifecycle
    ON document_registry_disposition_reviews;
CREATE TRIGGER document_registry_reviews_lifecycle
    BEFORE UPDATE ON document_registry_disposition_reviews
    FOR EACH ROW EXECUTE FUNCTION enforce_document_registry_review_lifecycle();
