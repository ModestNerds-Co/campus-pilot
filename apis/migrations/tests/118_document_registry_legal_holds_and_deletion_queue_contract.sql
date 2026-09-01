-- Document Registry legal-hold and deletion-queue contract. Run after migration 118.

DO $$
DECLARE
    trigger_count BIGINT;
BEGIN
    SELECT COUNT(*) INTO trigger_count
      FROM pg_trigger
     WHERE tgname IN (
        'document_registry_legal_holds_lifecycle',
        'document_registry_deletion_jobs_lifecycle',
        'document_registry_reviews_lifecycle'
     )
       AND NOT tgisinternal;
    IF trigger_count <> 3 THEN
        RAISE EXCEPTION 'Document Registry legal-hold or deletion lifecycle triggers are missing';
    END IF;

    IF NOT EXISTS (
        SELECT 1
          FROM information_schema.columns
         WHERE table_name = 'document_registry_disposition_reviews'
           AND column_name = 'status'
           AND table_schema = 'public'
    ) THEN
        RAISE EXCEPTION 'Document Registry disposition status is missing';
    END IF;
END;
$$;

CREATE TEMP TABLE document_registry_legal_holds (
    tenant_id UUID NOT NULL,
    file_id UUID NOT NULL,
    reference TEXT,
    reason TEXT NOT NULL,
    status TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    applied_by UUID NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    released_by UUID,
    released_at TIMESTAMPTZ,
    release_reason TEXT,
    updated_at TIMESTAMPTZ
);
CREATE TRIGGER verify_document_registry_legal_holds_lifecycle
    BEFORE UPDATE ON document_registry_legal_holds
    FOR EACH ROW EXECUTE FUNCTION enforce_document_registry_legal_hold_lifecycle();

INSERT INTO document_registry_legal_holds (
    tenant_id, file_id, reason, status, applied_by
) VALUES (
    gen_random_uuid(), gen_random_uuid(), 'Preserve for review', 'active', gen_random_uuid()
);

DO $$
BEGIN
    BEGIN
        UPDATE document_registry_legal_holds SET reason = 'Rewritten evidence';
        RAISE EXCEPTION 'Legal hold evidence mutation was accepted';
    EXCEPTION
        WHEN check_violation THEN NULL;
    END;
END;
$$;

UPDATE document_registry_legal_holds
   SET status = 'released',
       released_by = gen_random_uuid(),
       released_at = NOW(),
       release_reason = 'Matter concluded',
       version = version + 1;

DO $$
BEGIN
    BEGIN
        UPDATE document_registry_legal_holds SET release_reason = 'Rewritten release';
        RAISE EXCEPTION 'Released legal hold mutation was accepted';
    EXCEPTION
        WHEN check_violation THEN NULL;
    END;
END;
$$;

TRUNCATE document_registry_legal_holds;
INSERT INTO document_registry_legal_holds (
    tenant_id, file_id, reason, status, applied_by
) VALUES (
    '10000000-0000-0000-0000-000000000001',
    '20000000-0000-0000-0000-000000000001',
    'Preserve during proceedings',
    'active',
    gen_random_uuid()
);

CREATE TEMP TABLE document_registry_disposition_reviews (
    tenant_id UUID NOT NULL,
    file_id UUID NOT NULL,
    status TEXT NOT NULL,
    recommendation TEXT NOT NULL,
    request_reason TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    reviewed_by UUID,
    reviewed_at TIMESTAMPTZ,
    review_reason TEXT,
    executed_by UUID,
    executed_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ
);
CREATE TRIGGER verify_document_registry_reviews_lifecycle
    BEFORE UPDATE ON document_registry_disposition_reviews
    FOR EACH ROW EXECUTE FUNCTION enforce_document_registry_review_lifecycle();

INSERT INTO document_registry_disposition_reviews (
    tenant_id, file_id, status, recommendation, request_reason
) VALUES (
    '10000000-0000-0000-0000-000000000001',
    '20000000-0000-0000-0000-000000000001',
    'pending',
    'destroy',
    'Retention elapsed'
);

DO $$
BEGIN
    BEGIN
        UPDATE document_registry_disposition_reviews
           SET status = 'approved',
               reviewed_by = gen_random_uuid(),
               reviewed_at = NOW(),
               review_reason = 'Approved',
               version = version + 1;
        RAISE EXCEPTION 'A legal hold allowed destruction approval';
    EXCEPTION
        WHEN check_violation THEN NULL;
    END;
END;
$$;

UPDATE document_registry_legal_holds
   SET status = 'released',
       released_by = gen_random_uuid(),
       released_at = NOW(),
       release_reason = 'Matter concluded',
       version = version + 1;

UPDATE document_registry_disposition_reviews
   SET status = 'approved',
       reviewed_by = gen_random_uuid(),
       reviewed_at = NOW(),
       review_reason = 'Approved after release',
       version = version + 1;
UPDATE document_registry_disposition_reviews
   SET status = 'deletion_pending',
       version = version + 1;

CREATE TEMP TABLE document_registry_deletion_jobs (
    tenant_id UUID NOT NULL,
    review_id UUID NOT NULL,
    file_id UUID NOT NULL,
    object_key TEXT NOT NULL,
    destruction_reason TEXT NOT NULL,
    status TEXT NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    lease_token UUID,
    lease_expires_at TIMESTAMPTZ,
    last_error_code TEXT,
    requested_by UUID NOT NULL,
    requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    version INTEGER NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ
);
CREATE TRIGGER verify_document_registry_deletion_jobs_lifecycle
    BEFORE INSERT OR UPDATE ON document_registry_deletion_jobs
    FOR EACH ROW EXECUTE FUNCTION enforce_document_registry_deletion_job_lifecycle();

INSERT INTO document_registry_deletion_jobs (
    tenant_id, review_id, file_id, object_key, destruction_reason, status, requested_by
) VALUES (
    '10000000-0000-0000-0000-000000000001',
    gen_random_uuid(),
    '20000000-0000-0000-0000-000000000001',
    'tenant/document.pdf',
    'Approved retention disposition',
    'pending',
    gen_random_uuid()
);

UPDATE document_registry_deletion_jobs
   SET status = 'processing',
       lease_token = gen_random_uuid(),
       lease_expires_at = NOW() + INTERVAL '30 seconds',
       attempt_count = attempt_count + 1,
       version = version + 1;
UPDATE document_registry_deletion_jobs
   SET status = 'retry',
       lease_token = NULL,
       lease_expires_at = NULL,
       next_attempt_at = NOW() + INTERVAL '10 seconds',
       last_error_code = 'storage_delete_failed',
       version = version + 1;
UPDATE document_registry_deletion_jobs
   SET status = 'processing',
       lease_token = gen_random_uuid(),
       lease_expires_at = NOW() + INTERVAL '30 seconds',
       attempt_count = attempt_count + 1,
       version = version + 1;
UPDATE document_registry_deletion_jobs
   SET status = 'completed',
       lease_token = NULL,
       lease_expires_at = NULL,
       completed_at = NOW(),
       last_error_code = NULL,
       version = version + 1;

DO $$
BEGIN
    BEGIN
        UPDATE document_registry_deletion_jobs SET status = 'retry';
        RAISE EXCEPTION 'Completed deletion job mutation was accepted';
    EXCEPTION
        WHEN check_violation THEN NULL;
    END;
END;
$$;

DROP TABLE document_registry_deletion_jobs;
DROP TABLE document_registry_disposition_reviews;
DROP TABLE document_registry_legal_holds;

SELECT 'document registry legal hold and deletion queue contract passed' AS result;
