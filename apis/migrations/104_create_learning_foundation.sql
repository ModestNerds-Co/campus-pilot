-- Licensed E-learning spaces, ordered units, and governed resources.
--
-- Academics owns teaching structure, SIS owns enrolment, HR owns employee
-- identity, and Document Registry owns file bytes and retention. Learning keeps
-- only stable references plus its own publication lifecycle.

CREATE TABLE IF NOT EXISTS learning_settings (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    document_series_id UUID,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    updated_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    FOREIGN KEY (document_series_id, tenant_id)
        REFERENCES document_registry_series(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id)
);

INSERT INTO learning_settings (tenant_id)
SELECT tenant.id FROM tenants AS tenant
ON CONFLICT (tenant_id) DO NOTHING;

CREATE OR REPLACE FUNCTION provision_learning_settings()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO learning_settings (tenant_id)
    VALUES (NEW.id)
    ON CONFLICT (tenant_id) DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zzzzz_provision_learning_settings ON tenants;
CREATE TRIGGER zzzzz_provision_learning_settings
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION provision_learning_settings();

DROP TRIGGER IF EXISTS update_learning_settings_updated_at ON learning_settings;
CREATE TRIGGER update_learning_settings_updated_at
    BEFORE UPDATE ON learning_settings
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS learning_spaces (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    teaching_assignment_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    class_group_id UUID NOT NULL,
    title TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(title)) BETWEEN 1 AND 200),
    summary TEXT CHECK (summary IS NULL OR CHAR_LENGTH(BTRIM(summary)) <= 4000),
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'published', 'archived')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    published_by UUID,
    published_at TIMESTAMPTZ,
    archived_by UUID,
    archived_at TIMESTAMPTZ,
    archive_reason TEXT CHECK (
        archive_reason IS NULL OR CHAR_LENGTH(BTRIM(archive_reason)) BETWEEN 1 AND 2000
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    FOREIGN KEY (teaching_assignment_id, tenant_id)
        REFERENCES teaching_assignments(id, tenant_id),
    FOREIGN KEY (academic_year_id, tenant_id)
        REFERENCES academic_years(id, tenant_id),
    FOREIGN KEY (academic_term_id, tenant_id)
        REFERENCES academic_terms(id, tenant_id),
    FOREIGN KEY (class_group_id, tenant_id)
        REFERENCES class_groups(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (published_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (archived_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'draft' AND published_by IS NULL AND published_at IS NULL
            AND archived_by IS NULL AND archived_at IS NULL AND archive_reason IS NULL)
        OR (status = 'published' AND published_by IS NOT NULL AND published_at IS NOT NULL
            AND archived_by IS NULL AND archived_at IS NULL AND archive_reason IS NULL)
        OR (status = 'archived' AND published_by IS NOT NULL AND published_at IS NOT NULL
            AND archived_by IS NOT NULL AND archived_at IS NOT NULL AND archive_reason IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_learning_spaces_assignment_term
    ON learning_spaces(tenant_id, teaching_assignment_id, academic_term_id)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_learning_spaces_worklist
    ON learning_spaces(tenant_id, status, academic_term_id, updated_at DESC)
    WHERE deleted_at IS NULL;
DROP TRIGGER IF EXISTS update_learning_spaces_updated_at ON learning_spaces;
CREATE TRIGGER update_learning_spaces_updated_at
    BEFORE UPDATE ON learning_spaces
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS learning_units (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    learning_space_id UUID NOT NULL,
    position INTEGER NOT NULL CHECK (position > 0),
    title TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(title)) BETWEEN 1 AND 200),
    summary TEXT CHECK (summary IS NULL OR CHAR_LENGTH(BTRIM(summary)) <= 4000),
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'published', 'withdrawn')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    published_by UUID,
    published_at TIMESTAMPTZ,
    withdrawn_by UUID,
    withdrawn_at TIMESTAMPTZ,
    withdrawal_reason TEXT CHECK (
        withdrawal_reason IS NULL OR CHAR_LENGTH(BTRIM(withdrawal_reason)) BETWEEN 1 AND 2000
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    FOREIGN KEY (learning_space_id, tenant_id)
        REFERENCES learning_spaces(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (published_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (withdrawn_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'draft' AND published_by IS NULL AND published_at IS NULL
            AND withdrawn_by IS NULL AND withdrawn_at IS NULL AND withdrawal_reason IS NULL)
        OR (status = 'published' AND published_by IS NOT NULL AND published_at IS NOT NULL
            AND withdrawn_by IS NULL AND withdrawn_at IS NULL AND withdrawal_reason IS NULL)
        OR (status = 'withdrawn' AND published_by IS NOT NULL AND published_at IS NOT NULL
            AND withdrawn_by IS NOT NULL AND withdrawn_at IS NOT NULL AND withdrawal_reason IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_learning_units_position
    ON learning_units(tenant_id, learning_space_id, position)
    WHERE deleted_at IS NULL AND status <> 'withdrawn';
CREATE INDEX IF NOT EXISTS idx_learning_units_space
    ON learning_units(tenant_id, learning_space_id, position, created_at)
    WHERE deleted_at IS NULL;
DROP TRIGGER IF EXISTS update_learning_units_updated_at ON learning_units;
CREATE TRIGGER update_learning_units_updated_at
    BEFORE UPDATE ON learning_units
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS learning_resources (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    learning_unit_id UUID NOT NULL,
    document_file_id UUID NOT NULL,
    display_title TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(display_title)) BETWEEN 1 AND 240),
    sensitivity_snapshot TEXT NOT NULL
        CHECK (sensitivity_snapshot IN ('general', 'internal', 'confidential')),
    position INTEGER NOT NULL CHECK (position > 0),
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'published', 'withdrawn')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    published_by UUID,
    published_at TIMESTAMPTZ,
    withdrawn_by UUID,
    withdrawn_at TIMESTAMPTZ,
    withdrawal_reason TEXT CHECK (
        withdrawal_reason IS NULL OR CHAR_LENGTH(BTRIM(withdrawal_reason)) BETWEEN 1 AND 2000
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (id, tenant_id),
    FOREIGN KEY (learning_unit_id, tenant_id)
        REFERENCES learning_units(id, tenant_id),
    FOREIGN KEY (document_file_id, tenant_id)
        REFERENCES document_registry_files(id, tenant_id),
    FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (updated_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (published_by, tenant_id) REFERENCES users(id, tenant_id),
    FOREIGN KEY (withdrawn_by, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (
        (status = 'draft' AND published_by IS NULL AND published_at IS NULL
            AND withdrawn_by IS NULL AND withdrawn_at IS NULL AND withdrawal_reason IS NULL)
        OR (status = 'published' AND published_by IS NOT NULL AND published_at IS NOT NULL
            AND withdrawn_by IS NULL AND withdrawn_at IS NULL AND withdrawal_reason IS NULL)
        OR (status = 'withdrawn' AND published_by IS NOT NULL AND published_at IS NOT NULL
            AND withdrawn_by IS NOT NULL AND withdrawn_at IS NOT NULL AND withdrawal_reason IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_learning_resources_file
    ON learning_resources(tenant_id, learning_unit_id, document_file_id)
    WHERE deleted_at IS NULL AND status <> 'withdrawn';
CREATE UNIQUE INDEX IF NOT EXISTS idx_learning_resources_position
    ON learning_resources(tenant_id, learning_unit_id, position)
    WHERE deleted_at IS NULL AND status <> 'withdrawn';
CREATE INDEX IF NOT EXISTS idx_learning_resources_unit
    ON learning_resources(tenant_id, learning_unit_id, position, created_at)
    WHERE deleted_at IS NULL;
DROP TRIGGER IF EXISTS update_learning_resources_updated_at ON learning_resources;
CREATE TRIGGER update_learning_resources_updated_at
    BEFORE UPDATE ON learning_resources
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS learning_activity_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    aggregate_type TEXT NOT NULL CHECK (aggregate_type IN ('settings', 'space', 'unit', 'resource')),
    aggregate_id UUID NOT NULL,
    learning_space_id UUID,
    event_type TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(event_type)) BETWEEN 3 AND 100),
    actor_id UUID NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    FOREIGN KEY (learning_space_id, tenant_id) REFERENCES learning_spaces(id, tenant_id),
    FOREIGN KEY (actor_id, tenant_id) REFERENCES users(id, tenant_id),
    CHECK (updated_at = created_at AND deleted_at IS NULL)
);

CREATE INDEX IF NOT EXISTS idx_learning_activity_history
    ON learning_activity_events(tenant_id, learning_space_id, created_at DESC, id);

CREATE OR REPLACE FUNCTION reject_learning_activity_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Learning activity is append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS learning_activity_append_only ON learning_activity_events;
CREATE TRIGGER learning_activity_append_only
    BEFORE UPDATE OR DELETE ON learning_activity_events
    FOR EACH ROW EXECUTE FUNCTION reject_learning_activity_mutation();

UPDATE roles
SET permissions = ARRAY(
        SELECT DISTINCT permission
        FROM UNNEST(
            permissions || CASE key
                WHEN 'teacher' THEN ARRAY['learning:view', 'learning:teach']::TEXT[]
                WHEN 'student' THEN ARRAY['learning:view']::TEXT[]
                WHEN 'academic_manager' THEN ARRAY['learning:view', 'learning:teach', 'learning:manage']::TEXT[]
                ELSE ARRAY[]::TEXT[]
            END
        ) AS expanded(permission)
        ORDER BY permission
    ),
    updated_at = NOW()
WHERE key IN ('teacher', 'student', 'academic_manager')
  AND deleted_at IS NULL;

CREATE OR REPLACE FUNCTION grant_new_tenant_learning_permissions()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE roles
    SET permissions = ARRAY(
            SELECT DISTINCT permission
            FROM UNNEST(
                permissions || CASE key
                    WHEN 'teacher' THEN ARRAY['learning:view', 'learning:teach']::TEXT[]
                    WHEN 'student' THEN ARRAY['learning:view']::TEXT[]
                    WHEN 'academic_manager' THEN ARRAY['learning:view', 'learning:teach', 'learning:manage']::TEXT[]
                    ELSE ARRAY[]::TEXT[]
                END
            ) AS expanded(permission)
            ORDER BY permission
        ),
        updated_at = NOW()
    WHERE tenant_id = NEW.id
      AND key IN ('teacher', 'student', 'academic_manager')
      AND deleted_at IS NULL;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zzzzz_grant_new_tenant_learning_permissions ON tenants;
CREATE TRIGGER zzzzz_grant_new_tenant_learning_permissions
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION grant_new_tenant_learning_permissions();

INSERT INTO role_record_scope_grants (tenant_id, role_id, scope_family, scope_kind)
SELECT role.tenant_id, role.id, 'learning.spaces',
       CASE
           WHEN role.key = 'teacher' THEN 'assigned'
           WHEN role.key = 'student' THEN 'self'
           ELSE 'campus'
       END
FROM roles AS role
WHERE role.key IN ('teacher', 'student', 'academic_manager')
  AND role.deleted_at IS NULL
ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
    WHERE deleted_at IS NULL DO NOTHING;

CREATE OR REPLACE FUNCTION provision_learning_role_scopes()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.key IN ('teacher', 'student', 'academic_manager') THEN
        INSERT INTO role_record_scope_grants (
            tenant_id, role_id, scope_family, scope_kind
        ) VALUES (
            NEW.tenant_id, NEW.id, 'learning.spaces',
            CASE
                WHEN NEW.key = 'teacher' THEN 'assigned'
                WHEN NEW.key = 'student' THEN 'self'
                ELSE 'campus'
            END
        )
        ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
            WHERE deleted_at IS NULL DO NOTHING;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS provision_learning_role_scopes_after_insert ON roles;
CREATE TRIGGER provision_learning_role_scopes_after_insert
    AFTER INSERT ON roles
    FOR EACH ROW EXECUTE FUNCTION provision_learning_role_scopes();

DROP TRIGGER IF EXISTS ev_learning_spaces ON learning_spaces;
CREATE TRIGGER ev_learning_spaces
    AFTER INSERT OR UPDATE OR DELETE ON learning_spaces
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_learning_units ON learning_units;
CREATE TRIGGER ev_learning_units
    AFTER INSERT OR UPDATE OR DELETE ON learning_units
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_learning_resources ON learning_resources;
CREATE TRIGGER ev_learning_resources
    AFTER INSERT OR UPDATE OR DELETE ON learning_resources
    FOR EACH ROW EXECUTE FUNCTION log_event();
