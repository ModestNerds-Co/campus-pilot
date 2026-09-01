-- Document Registry lifecycle contract. Run after migration 117.

DO $$
DECLARE
    trigger_count BIGINT;
BEGIN
    SELECT COUNT(*) INTO trigger_count
      FROM pg_trigger
     WHERE tgname IN (
        'document_registry_files_lifecycle',
        'document_registry_reviews_lifecycle'
     )
       AND NOT tgisinternal;
    IF trigger_count <> 2 THEN
        RAISE EXCEPTION 'Document Registry lifecycle triggers are missing';
    END IF;
END;
$$;

CREATE TEMP TABLE document_registry_files (
    status TEXT NOT NULL,
    object_key TEXT,
    title TEXT NOT NULL,
    retain_until DATE,
    version INTEGER NOT NULL DEFAULT 1,
    updated_by UUID,
    updated_at TIMESTAMPTZ,
    destroyed_by UUID,
    destroyed_at TIMESTAMPTZ,
    destruction_reason TEXT
);
CREATE TRIGGER verify_document_registry_files_lifecycle
    BEFORE UPDATE ON document_registry_files
    FOR EACH ROW EXECUTE FUNCTION enforce_document_registry_file_lifecycle();

INSERT INTO document_registry_files (status, object_key, title, retain_until)
VALUES ('closed', 'tenant/file.pdf', 'Closed evidence', CURRENT_DATE);

DO $$
BEGIN
    BEGIN
        UPDATE document_registry_files SET title = 'Rewritten evidence';
        RAISE EXCEPTION 'Closed document evidence mutation was accepted';
    EXCEPTION
        WHEN check_violation THEN NULL;
    END;
END;
$$;

UPDATE document_registry_files
   SET retain_until = CURRENT_DATE + 30,
       version = version + 1
 WHERE status = 'closed';

DO $$
BEGIN
    BEGIN
        UPDATE document_registry_files SET status = 'filed';
        RAISE EXCEPTION 'Closed document reopening was accepted';
    EXCEPTION
        WHEN check_violation THEN NULL;
    END;
END;
$$;

INSERT INTO document_registry_files (status, object_key, title, retain_until)
VALUES ('destroyed', NULL, 'Destroyed evidence', CURRENT_DATE);
DO $$
BEGIN
    BEGIN
        UPDATE document_registry_files
           SET retain_until = CURRENT_DATE + 60
         WHERE status = 'destroyed';
        RAISE EXCEPTION 'Destroyed document mutation was accepted';
    EXCEPTION
        WHEN check_violation THEN NULL;
    END;
END;
$$;
DROP TABLE document_registry_files;

CREATE TEMP TABLE document_registry_disposition_reviews (
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
    status, recommendation, request_reason
) VALUES ('pending', 'destroy', 'Retention elapsed');

DO $$
BEGIN
    BEGIN
        UPDATE document_registry_disposition_reviews
           SET request_reason = 'Rewritten request', status = 'approved';
        RAISE EXCEPTION 'Disposition request evidence mutation was accepted';
    EXCEPTION
        WHEN check_violation THEN NULL;
    END;
END;
$$;

UPDATE document_registry_disposition_reviews
   SET status = 'rejected',
       reviewed_by = gen_random_uuid(),
       reviewed_at = NOW(),
       review_reason = 'Rejected by reviewer',
       version = version + 1
 WHERE status = 'pending';

DO $$
BEGIN
    BEGIN
        UPDATE document_registry_disposition_reviews
           SET review_reason = 'Reopened decision'
         WHERE status = 'rejected';
        RAISE EXCEPTION 'Terminal disposition review mutation was accepted';
    EXCEPTION
        WHEN check_violation THEN NULL;
    END;
END;
$$;
DROP TABLE document_registry_disposition_reviews;

SELECT 'document registry access and lifecycle contract passed' AS result;
