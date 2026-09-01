-- Harden Document Registry lifecycle evidence at the database boundary.
--
-- Filed records remain editable drafts. Closed records retain their filed and
-- closure evidence and may only receive a governed retention extension or an
-- approved destruction transition. Destroyed records and terminal disposition
-- decisions are final.

CREATE OR REPLACE FUNCTION enforce_document_registry_file_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.status = 'destroyed' THEN
        RAISE EXCEPTION 'Destroyed document records are final and cannot be changed'
            USING ERRCODE = '23514';
    END IF;

    IF OLD.status = 'filed' AND NEW.status NOT IN ('filed', 'closed') THEN
        RAISE EXCEPTION 'A filed document must be closed before destruction'
            USING ERRCODE = '23514';
    END IF;

    IF OLD.status = 'closed' THEN
        IF NEW.status NOT IN ('closed', 'destroyed') THEN
            RAISE EXCEPTION 'A closed document cannot be reopened'
                USING ERRCODE = '23514';
        END IF;

        IF (
            to_jsonb(NEW) - ARRAY[
                'retain_until', 'status', 'object_key', 'version', 'updated_by',
                'updated_at', 'destroyed_by', 'destroyed_at', 'destruction_reason'
            ]::TEXT[]
        ) IS DISTINCT FROM (
            to_jsonb(OLD) - ARRAY[
                'retain_until', 'status', 'object_key', 'version', 'updated_by',
                'updated_at', 'destroyed_by', 'destroyed_at', 'destruction_reason'
            ]::TEXT[]
        ) THEN
            RAISE EXCEPTION 'Closed document evidence is immutable'
                USING ERRCODE = '23514';
        END IF;

        IF NEW.status = 'closed' AND NEW.object_key IS DISTINCT FROM OLD.object_key THEN
            RAISE EXCEPTION 'Closed document bytes remain linked until approved destruction'
                USING ERRCODE = '23514';
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS document_registry_files_lifecycle
    ON document_registry_files;
CREATE TRIGGER document_registry_files_lifecycle
    BEFORE UPDATE ON document_registry_files
    FOR EACH ROW EXECUTE FUNCTION enforce_document_registry_file_lifecycle();

CREATE OR REPLACE FUNCTION enforce_document_registry_review_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.status IN ('rejected', 'executed') THEN
        RAISE EXCEPTION 'Completed disposition reviews are final and cannot be changed'
            USING ERRCODE = '23514';
    END IF;

    IF OLD.status = 'approved' AND NEW.status <> 'executed' THEN
        RAISE EXCEPTION 'An approved destruction may only be executed'
            USING ERRCODE = '23514';
    END IF;

    IF OLD.status = 'pending' AND NEW.status NOT IN ('approved', 'rejected', 'executed') THEN
        RAISE EXCEPTION 'A pending disposition review must receive a final decision'
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
