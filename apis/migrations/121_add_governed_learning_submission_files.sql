-- Governed learner files for versioned E-learning submissions.
--
-- Document Registry owns restricted bytes and retention. Learning owns only
-- draft attachment links and immutable per-submission-version metadata.

ALTER TABLE learning_settings
    ADD COLUMN IF NOT EXISTS learner_submission_series_id UUID;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
          FROM pg_constraint
         WHERE conname = 'learning_settings_submission_series_fk'
           AND conrelid = 'learning_settings'::REGCLASS
    ) THEN
        ALTER TABLE learning_settings
            ADD CONSTRAINT learning_settings_submission_series_fk
            FOREIGN KEY (learner_submission_series_id, tenant_id)
            REFERENCES document_registry_series(id, tenant_id);
    END IF;
END;
$$;

ALTER TABLE learning_assignments
    ADD COLUMN IF NOT EXISTS submission_method TEXT NOT NULL DEFAULT 'text';

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
          FROM pg_constraint
         WHERE conname = 'learning_assignments_submission_method_check'
           AND conrelid = 'learning_assignments'::REGCLASS
    ) THEN
        ALTER TABLE learning_assignments
            ADD CONSTRAINT learning_assignments_submission_method_check
            CHECK (submission_method IN ('text', 'file', 'text_or_file'));
    END IF;
END;
$$;

ALTER TABLE learning_submission_versions
    ALTER COLUMN body_snapshot DROP NOT NULL;
ALTER TABLE learning_submission_versions
    DROP CONSTRAINT IF EXISTS learning_submission_versions_body_snapshot_check;
ALTER TABLE learning_submission_versions
    ADD CONSTRAINT learning_submission_versions_body_snapshot_check
    CHECK (
        body_snapshot IS NULL
        OR CHAR_LENGTH(BTRIM(body_snapshot)) BETWEEN 1 AND 20000
    );

CREATE TABLE IF NOT EXISTS learning_submission_attachments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    learning_submission_id UUID NOT NULL,
    document_file_id UUID NOT NULL,
    position INTEGER NOT NULL CHECK (position BETWEEN 1 AND 5),
    document_reference_snapshot TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(document_reference_snapshot)) BETWEEN 1 AND 40),
    original_file_name_snapshot TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(original_file_name_snapshot)) BETWEEN 1 AND 255),
    media_type_snapshot TEXT NOT NULL
        CHECK (media_type_snapshot IN ('application/pdf', 'image/jpeg', 'image/png')),
    byte_size_snapshot BIGINT NOT NULL CHECK (byte_size_snapshot BETWEEN 1 AND 15728640),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'removed')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    attached_by UUID NOT NULL,
    removed_by UUID,
    attached_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    removed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    UNIQUE (id, learning_submission_id, tenant_id),
    UNIQUE (tenant_id, document_file_id),
    FOREIGN KEY (learning_submission_id, tenant_id)
        REFERENCES learning_submissions(id, tenant_id),
    FOREIGN KEY (document_file_id, tenant_id)
        REFERENCES document_registry_files(id, tenant_id),
    FOREIGN KEY (attached_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (removed_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'active' AND removed_by IS NULL AND removed_at IS NULL)
        OR (status = 'removed' AND removed_by IS NOT NULL AND removed_at IS NOT NULL)
    ),
    CHECK (deleted_at IS NULL)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_learning_submission_attachment_position
    ON learning_submission_attachments(tenant_id, learning_submission_id, position)
    WHERE status = 'active' AND deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_learning_submission_attachment_worklist
    ON learning_submission_attachments(tenant_id, learning_submission_id, status, position, id)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS learning_submission_version_files (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    learning_submission_id UUID NOT NULL,
    submission_version_id UUID NOT NULL,
    attachment_id UUID NOT NULL,
    document_file_id UUID NOT NULL,
    position INTEGER NOT NULL CHECK (position BETWEEN 1 AND 5),
    document_reference_snapshot TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(document_reference_snapshot)) BETWEEN 1 AND 40),
    original_file_name_snapshot TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(original_file_name_snapshot)) BETWEEN 1 AND 255),
    media_type_snapshot TEXT NOT NULL
        CHECK (media_type_snapshot IN ('application/pdf', 'image/jpeg', 'image/png')),
    byte_size_snapshot BIGINT NOT NULL CHECK (byte_size_snapshot BETWEEN 1 AND 15728640),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    UNIQUE (tenant_id, submission_version_id, attachment_id),
    UNIQUE (tenant_id, submission_version_id, position),
    FOREIGN KEY (submission_version_id, learning_submission_id, tenant_id)
        REFERENCES learning_submission_versions(id, learning_submission_id, tenant_id),
    FOREIGN KEY (attachment_id, learning_submission_id, tenant_id)
        REFERENCES learning_submission_attachments(id, learning_submission_id, tenant_id),
    FOREIGN KEY (document_file_id, tenant_id)
        REFERENCES document_registry_files(id, tenant_id),
    CHECK (updated_at = created_at AND deleted_at IS NULL)
);

CREATE INDEX IF NOT EXISTS idx_learning_submission_version_files_version
    ON learning_submission_version_files(tenant_id, submission_version_id, position, id);

CREATE OR REPLACE FUNCTION protect_learning_assignment_lifecycle()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        IF OLD.status <> 'draft' THEN
            RAISE EXCEPTION 'Published Learning assignments are retained evidence';
        END IF;
        RETURN OLD;
    END IF;

    IF OLD.status = 'closed' THEN
        RAISE EXCEPTION 'Closed Learning assignments are immutable';
    END IF;

    IF OLD.status = 'published' THEN
        IF NEW.status <> 'closed'
           OR NEW.id IS DISTINCT FROM OLD.id
           OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
           OR NEW.learning_unit_id IS DISTINCT FROM OLD.learning_unit_id
           OR NEW.position IS DISTINCT FROM OLD.position
           OR NEW.title IS DISTINCT FROM OLD.title
           OR NEW.instructions IS DISTINCT FROM OLD.instructions
           OR NEW.due_at IS DISTINCT FROM OLD.due_at
           OR NEW.max_score_hundredths IS DISTINCT FROM OLD.max_score_hundredths
           OR NEW.submission_method IS DISTINCT FROM OLD.submission_method
           OR NEW.published_by IS DISTINCT FROM OLD.published_by
           OR NEW.published_at IS DISTINCT FROM OLD.published_at
           OR NEW.created_by IS DISTINCT FROM OLD.created_by
           OR NEW.created_at IS DISTINCT FROM OLD.created_at
           OR NEW.deleted_at IS DISTINCT FROM OLD.deleted_at
           OR NEW.version <> OLD.version + 1 THEN
            RAISE EXCEPTION 'Published Learning assignment content is immutable';
        END IF;
    ELSIF OLD.status = 'draft' AND NEW.status NOT IN ('draft', 'published') THEN
        RAISE EXCEPTION 'Learning assignment lifecycle transition is invalid';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS learning_assignment_lifecycle_guard ON learning_assignments;
CREATE TRIGGER learning_assignment_lifecycle_guard
    BEFORE UPDATE OR DELETE ON learning_assignments
    FOR EACH ROW EXECUTE FUNCTION protect_learning_assignment_lifecycle();

CREATE OR REPLACE FUNCTION protect_learning_rubric_draft_only()
RETURNS TRIGGER AS $$
DECLARE
    target_assignment_id UUID;
    target_tenant_id UUID;
    target_status TEXT;
BEGIN
    IF TG_OP = 'UPDATE' AND (
        NEW.id IS DISTINCT FROM OLD.id
        OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
        OR NEW.learning_assignment_id IS DISTINCT FROM OLD.learning_assignment_id
        OR NEW.created_by IS DISTINCT FROM OLD.created_by
        OR NEW.created_at IS DISTINCT FROM OLD.created_at
    ) THEN
        RAISE EXCEPTION 'Learning rubric criterion ownership is immutable';
    END IF;
    target_assignment_id := CASE WHEN TG_OP = 'DELETE'
        THEN OLD.learning_assignment_id ELSE NEW.learning_assignment_id END;
    target_tenant_id := CASE WHEN TG_OP = 'DELETE'
        THEN OLD.tenant_id ELSE NEW.tenant_id END;
    SELECT status INTO target_status
      FROM learning_assignments
     WHERE id = target_assignment_id
       AND tenant_id = target_tenant_id
       AND deleted_at IS NULL;
    IF target_status IS DISTINCT FROM 'draft' THEN
        RAISE EXCEPTION 'Published Learning rubric criteria are immutable';
    END IF;
    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS learning_rubric_draft_only ON learning_assignment_rubric_criteria;
CREATE TRIGGER learning_rubric_draft_only
    BEFORE INSERT OR UPDATE OR DELETE ON learning_assignment_rubric_criteria
    FOR EACH ROW EXECUTE FUNCTION protect_learning_rubric_draft_only();

CREATE OR REPLACE FUNCTION protect_learning_submission_attachment()
RETURNS TRIGGER AS $$
DECLARE
    parent_status TEXT;
    assignment_status TEXT;
    parent_method TEXT;
    configured_series_id UUID;
    file_series_id UUID;
    file_status TEXT;
    file_sensitivity TEXT;
    file_reference TEXT;
    file_name TEXT;
    file_media_type TEXT;
    file_byte_size BIGINT;
    active_count BIGINT;
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Learning submission attachment evidence is retained';
    END IF;

    SELECT submission.status, assignment.status, assignment.submission_method
      INTO parent_status, assignment_status, parent_method
      FROM learning_submissions AS submission
      JOIN learning_assignments AS assignment
        ON assignment.id = submission.learning_assignment_id
       AND assignment.tenant_id = submission.tenant_id
     WHERE submission.id = NEW.learning_submission_id
       AND submission.tenant_id = NEW.tenant_id
       AND submission.deleted_at IS NULL
       AND assignment.deleted_at IS NULL;

    IF assignment_status IS DISTINCT FROM 'published'
       OR parent_status NOT IN ('draft', 'revision_requested')
       OR parent_method NOT IN ('file', 'text_or_file') THEN
        RAISE EXCEPTION 'This Learning submission cannot change file attachments';
    END IF;

    IF TG_OP = 'INSERT' THEN
        IF NEW.status <> 'active'
           OR NEW.version <> 1
           OR NEW.removed_by IS NOT NULL
           OR NEW.removed_at IS NOT NULL
           OR NEW.deleted_at IS NOT NULL THEN
            RAISE EXCEPTION 'Learning submission attachments must start active';
        END IF;
        SELECT learner_submission_series_id
          INTO configured_series_id
          FROM learning_settings
         WHERE tenant_id = NEW.tenant_id
           AND deleted_at IS NULL;
        SELECT series_id, status, sensitivity, reference, original_file_name,
               media_type, byte_size
          INTO file_series_id, file_status, file_sensitivity, file_reference,
               file_name, file_media_type, file_byte_size
          FROM document_registry_files
         WHERE tenant_id = NEW.tenant_id
           AND id = NEW.document_file_id
           AND deleted_at IS NULL;
        IF configured_series_id IS NULL
           OR file_series_id IS DISTINCT FROM configured_series_id
           OR file_status IS DISTINCT FROM 'closed'
           OR file_sensitivity IS DISTINCT FROM 'restricted'
           OR NEW.document_reference_snapshot IS DISTINCT FROM file_reference
           OR NEW.original_file_name_snapshot IS DISTINCT FROM file_name
           OR NEW.media_type_snapshot IS DISTINCT FROM file_media_type
           OR NEW.byte_size_snapshot IS DISTINCT FROM file_byte_size THEN
            RAISE EXCEPTION 'Learning submission files must use the configured restricted retention classification';
        END IF;
        SELECT COUNT(*) INTO active_count
          FROM learning_submission_attachments
         WHERE tenant_id = NEW.tenant_id
           AND learning_submission_id = NEW.learning_submission_id
           AND status = 'active'
           AND deleted_at IS NULL;
        IF active_count >= 5 THEN
            RAISE EXCEPTION 'A Learning submission accepts at most five files';
        END IF;
        RETURN NEW;
    END IF;

    IF OLD.status <> 'active'
       OR NEW.status <> 'removed'
       OR NEW.id IS DISTINCT FROM OLD.id
       OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
       OR NEW.learning_submission_id IS DISTINCT FROM OLD.learning_submission_id
       OR NEW.document_file_id IS DISTINCT FROM OLD.document_file_id
       OR NEW.position IS DISTINCT FROM OLD.position
       OR NEW.document_reference_snapshot IS DISTINCT FROM OLD.document_reference_snapshot
       OR NEW.original_file_name_snapshot IS DISTINCT FROM OLD.original_file_name_snapshot
       OR NEW.media_type_snapshot IS DISTINCT FROM OLD.media_type_snapshot
       OR NEW.byte_size_snapshot IS DISTINCT FROM OLD.byte_size_snapshot
       OR NEW.attached_by IS DISTINCT FROM OLD.attached_by
       OR NEW.attached_at IS DISTINCT FROM OLD.attached_at
       OR NEW.created_at IS DISTINCT FROM OLD.created_at
       OR NEW.deleted_at IS NOT NULL
       OR NEW.version <> OLD.version + 1
       OR NEW.removed_by IS NULL
       OR NEW.removed_at IS NULL THEN
        RAISE EXCEPTION 'Learning submission attachment transition is invalid';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS learning_submission_attachment_guard ON learning_submission_attachments;
CREATE TRIGGER learning_submission_attachment_guard
    BEFORE INSERT OR UPDATE OR DELETE ON learning_submission_attachments
    FOR EACH ROW EXECUTE FUNCTION protect_learning_submission_attachment();

CREATE OR REPLACE FUNCTION protect_learning_submission_version_file()
RETURNS TRIGGER AS $$
DECLARE
    attachment_status TEXT;
    attachment_position INTEGER;
    attachment_reference TEXT;
    attachment_file_name TEXT;
    attachment_media_type TEXT;
    attachment_byte_size BIGINT;
BEGIN
    IF TG_OP <> 'INSERT' THEN
        RAISE EXCEPTION 'Learning submission version files are append-only';
    END IF;
    SELECT status, position, document_reference_snapshot,
           original_file_name_snapshot, media_type_snapshot, byte_size_snapshot
      INTO attachment_status, attachment_position, attachment_reference,
           attachment_file_name, attachment_media_type, attachment_byte_size
      FROM learning_submission_attachments
     WHERE tenant_id = NEW.tenant_id
       AND id = NEW.attachment_id
       AND learning_submission_id = NEW.learning_submission_id
       AND document_file_id = NEW.document_file_id
       AND deleted_at IS NULL;
    IF attachment_status IS DISTINCT FROM 'active'
       OR NEW.position IS DISTINCT FROM attachment_position
       OR NEW.document_reference_snapshot IS DISTINCT FROM attachment_reference
       OR NEW.original_file_name_snapshot IS DISTINCT FROM attachment_file_name
       OR NEW.media_type_snapshot IS DISTINCT FROM attachment_media_type
       OR NEW.byte_size_snapshot IS DISTINCT FROM attachment_byte_size THEN
        RAISE EXCEPTION 'Only active Learning submission files can be snapshotted';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS learning_submission_version_file_guard ON learning_submission_version_files;
CREATE TRIGGER learning_submission_version_file_guard
    BEFORE INSERT OR UPDATE OR DELETE ON learning_submission_version_files
    FOR EACH ROW EXECUTE FUNCTION protect_learning_submission_version_file();
