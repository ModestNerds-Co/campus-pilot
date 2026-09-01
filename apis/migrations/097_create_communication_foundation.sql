-- Operational school communication with reviewed recipient snapshots.
--
-- Communication owns announcement workflow and in-app delivery state. Audience
-- membership continues to come from core accounts, SIS, Academics, and HR.

CREATE TABLE IF NOT EXISTS communication_announcements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    title TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(title)) BETWEEN 1 AND 180),
    body TEXT NOT NULL CHECK (CHAR_LENGTH(BTRIM(body)) BETWEEN 1 AND 10000),
    priority TEXT NOT NULL DEFAULT 'normal'
        CHECK (priority IN ('normal', 'important', 'urgent')),
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'submitted', 'published', 'cancelled')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_by UUID NOT NULL,
    submitted_by UUID,
    submitted_at TIMESTAMPTZ,
    published_by UUID,
    published_at TIMESTAMPTZ,
    cancelled_by UUID,
    cancelled_at TIMESTAMPTZ,
    cancellation_reason TEXT CHECK (
        cancellation_reason IS NULL
        OR CHAR_LENGTH(BTRIM(cancellation_reason)) BETWEEN 1 AND 1000
    ),
    reopened_by UUID,
    reopened_at TIMESTAMPTZ,
    reopen_reason TEXT CHECK (
        reopen_reason IS NULL
        OR CHAR_LENGTH(BTRIM(reopen_reason)) BETWEEN 1 AND 1000
    ),
    recipient_fingerprint TEXT CHECK (
        recipient_fingerprint IS NULL OR recipient_fingerprint ~ '^[0-9a-f]{64}$'
    ),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CONSTRAINT communication_announcements_creator_tenant_fkey
        FOREIGN KEY (created_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT communication_announcements_submitter_tenant_fkey
        FOREIGN KEY (submitted_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT communication_announcements_publisher_tenant_fkey
        FOREIGN KEY (published_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT communication_announcements_canceller_tenant_fkey
        FOREIGN KEY (cancelled_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT communication_announcements_reopener_tenant_fkey
        FOREIGN KEY (reopened_by, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT communication_announcements_lifecycle_check CHECK (
        (status = 'draft' AND submitted_by IS NULL AND submitted_at IS NULL
            AND published_by IS NULL AND published_at IS NULL
            AND cancelled_by IS NULL AND cancelled_at IS NULL
            AND cancellation_reason IS NULL AND recipient_fingerprint IS NULL)
        OR (status = 'submitted' AND submitted_by IS NOT NULL AND submitted_at IS NOT NULL
            AND published_by IS NULL AND published_at IS NULL
            AND cancelled_by IS NULL AND cancelled_at IS NULL
            AND cancellation_reason IS NULL AND recipient_fingerprint IS NOT NULL)
        OR (status = 'published' AND submitted_by IS NOT NULL AND submitted_at IS NOT NULL
            AND published_by IS NOT NULL AND published_at IS NOT NULL
            AND cancelled_by IS NULL AND cancelled_at IS NULL
            AND cancellation_reason IS NULL AND recipient_fingerprint IS NOT NULL)
        OR (status = 'cancelled' AND submitted_by IS NOT NULL AND submitted_at IS NOT NULL
            AND published_by IS NOT NULL AND published_at IS NOT NULL
            AND cancelled_by IS NOT NULL AND cancelled_at IS NOT NULL
            AND cancellation_reason IS NOT NULL AND recipient_fingerprint IS NOT NULL)
    ),
    CONSTRAINT communication_announcements_reopen_check CHECK (
        (reopened_by IS NULL AND reopened_at IS NULL AND reopen_reason IS NULL)
        OR (reopened_by IS NOT NULL AND reopened_at IS NOT NULL AND reopen_reason IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_communication_announcements_worklist
    ON communication_announcements(tenant_id, status, updated_at DESC)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_communication_announcements_creator
    ON communication_announcements(tenant_id, created_by, updated_at DESC)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_communication_announcements_updated_at
    ON communication_announcements;
CREATE TRIGGER update_communication_announcements_updated_at
    BEFORE UPDATE ON communication_announcements
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS communication_audience_targets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    announcement_id UUID NOT NULL,
    target_kind TEXT NOT NULL
        CHECK (target_kind IN ('campus', 'role', 'class_group', 'department', 'individual')),
    target_id UUID,
    target_key TEXT,
    label_snapshot TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(label_snapshot)) BETWEEN 1 AND 200),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CONSTRAINT communication_targets_parent_tenant_fkey
        FOREIGN KEY (announcement_id, tenant_id)
        REFERENCES communication_announcements(id, tenant_id),
    CONSTRAINT communication_targets_shape_check CHECK (
        (target_kind = 'campus' AND target_id IS NULL AND target_key IS NULL)
        OR (target_kind = 'role' AND target_id IS NULL
            AND target_key ~ '^[a-z][a-z0-9_]{0,79}$')
        OR (target_kind IN ('class_group', 'department', 'individual')
            AND target_id IS NOT NULL AND target_key IS NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_communication_targets_identity
    ON communication_audience_targets(
        tenant_id, announcement_id, target_kind,
        COALESCE(target_id::TEXT, ''), COALESCE(target_key, '')
    ) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_communication_targets_parent
    ON communication_audience_targets(tenant_id, announcement_id)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_communication_audience_targets_updated_at
    ON communication_audience_targets;
CREATE TRIGGER update_communication_audience_targets_updated_at
    BEFORE UPDATE ON communication_audience_targets
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS communication_deliveries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    announcement_id UUID NOT NULL,
    recipient_user_id UUID NOT NULL,
    recipient_name_snapshot TEXT NOT NULL
        CHECK (CHAR_LENGTH(BTRIM(recipient_name_snapshot)) BETWEEN 1 AND 200),
    channel TEXT NOT NULL DEFAULT 'in_app' CHECK (channel = 'in_app'),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'delivered')),
    delivered_at TIMESTAMPTZ,
    read_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CONSTRAINT communication_deliveries_parent_tenant_fkey
        FOREIGN KEY (announcement_id, tenant_id)
        REFERENCES communication_announcements(id, tenant_id),
    CONSTRAINT communication_deliveries_recipient_tenant_fkey
        FOREIGN KEY (recipient_user_id, tenant_id) REFERENCES users(id, tenant_id),
    CONSTRAINT communication_deliveries_state_check CHECK (
        (status = 'pending' AND delivered_at IS NULL AND read_at IS NULL)
        OR (status = 'delivered' AND delivered_at IS NOT NULL
            AND (read_at IS NULL OR read_at >= delivered_at))
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_communication_deliveries_recipient
    ON communication_deliveries(tenant_id, announcement_id, recipient_user_id, channel)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_communication_inbox
    ON communication_deliveries(tenant_id, recipient_user_id, delivered_at DESC)
    WHERE deleted_at IS NULL AND status = 'delivered';
CREATE INDEX IF NOT EXISTS idx_communication_delivery_history
    ON communication_deliveries(tenant_id, announcement_id, status, created_at)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_communication_deliveries_updated_at
    ON communication_deliveries;
CREATE TRIGGER update_communication_deliveries_updated_at
    BEFORE UPDATE ON communication_deliveries
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TABLE IF NOT EXISTS communication_announcement_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    announcement_id UUID NOT NULL,
    event_type TEXT NOT NULL CHECK (
        event_type IN ('created', 'updated', 'submitted', 'reopened', 'published', 'cancelled', 'deleted')
    ),
    from_status TEXT CHECK (
        from_status IS NULL OR from_status IN ('draft', 'submitted', 'published', 'cancelled')
    ),
    to_status TEXT NOT NULL CHECK (
        to_status IN ('draft', 'submitted', 'published', 'cancelled', 'deleted')
    ),
    announcement_version INTEGER NOT NULL CHECK (announcement_version > 0),
    actor_id UUID NOT NULL,
    reason TEXT CHECK (reason IS NULL OR CHAR_LENGTH(BTRIM(reason)) BETWEEN 1 AND 1000),
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    CONSTRAINT communication_events_parent_tenant_fkey
        FOREIGN KEY (announcement_id, tenant_id)
        REFERENCES communication_announcements(id, tenant_id),
    CONSTRAINT communication_events_actor_tenant_fkey
        FOREIGN KEY (actor_id, tenant_id) REFERENCES users(id, tenant_id)
);

CREATE INDEX IF NOT EXISTS idx_communication_events_history
    ON communication_announcement_events(tenant_id, announcement_id, created_at, id);

CREATE OR REPLACE FUNCTION enforce_communication_target_draft()
RETURNS TRIGGER AS $$
DECLARE parent_status TEXT;
BEGIN
    SELECT status INTO parent_status
      FROM communication_announcements
     WHERE tenant_id = COALESCE(NEW.tenant_id, OLD.tenant_id)
       AND id = COALESCE(NEW.announcement_id, OLD.announcement_id)
       AND deleted_at IS NULL;
    IF parent_status IS DISTINCT FROM 'draft' THEN
        RAISE EXCEPTION 'Communication audiences may change only while the announcement is draft';
    END IF;
    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS communication_targets_draft_guard
    ON communication_audience_targets;
CREATE TRIGGER communication_targets_draft_guard
    BEFORE INSERT OR UPDATE OR DELETE ON communication_audience_targets
    FOR EACH ROW EXECUTE FUNCTION enforce_communication_target_draft();

CREATE OR REPLACE FUNCTION prevent_communication_event_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Communication announcement history is append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS communication_events_append_only
    ON communication_announcement_events;
CREATE TRIGGER communication_events_append_only
    BEFORE UPDATE OR DELETE ON communication_announcement_events
    FOR EACH ROW EXECUTE FUNCTION prevent_communication_event_mutation();

DROP TRIGGER IF EXISTS ev_communication_announcements ON communication_announcements;
CREATE TRIGGER ev_communication_announcements
    AFTER INSERT OR UPDATE OR DELETE ON communication_announcements
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_communication_audience_targets ON communication_audience_targets;
CREATE TRIGGER ev_communication_audience_targets
    AFTER INSERT OR UPDATE OR DELETE ON communication_audience_targets
    FOR EACH ROW EXECUTE FUNCTION log_event();
DROP TRIGGER IF EXISTS ev_communication_deliveries ON communication_deliveries;
CREATE TRIGGER ev_communication_deliveries
    AFTER INSERT OR UPDATE OR DELETE ON communication_deliveries
    FOR EACH ROW EXECUTE FUNCTION log_event();

-- Teachers can maintain their own drafts, while publication remains separate.
UPDATE roles
   SET permissions = ARRAY(
       SELECT DISTINCT permission
         FROM UNNEST(permissions || ARRAY['messaging:edit']::TEXT[]) AS permission
        ORDER BY permission
   ), updated_at = NOW()
 WHERE key = 'teacher' AND deleted_at IS NULL;

INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
SELECT tenant.id, 'communication_manager', 'Communication Manager',
       'Prepares, reviews, publishes, and tracks campus communication.',
       ARRAY[
           'messaging:view', 'messaging:create', 'messaging:edit',
           'messaging:delete', 'messaging:send', 'messaging:manage'
       ]::TEXT[], TRUE
  FROM tenants AS tenant
 WHERE NOT EXISTS (
    SELECT 1 FROM roles AS role
     WHERE role.tenant_id = tenant.id AND role.key = 'communication_manager'
       AND role.deleted_at IS NULL
 );

CREATE OR REPLACE FUNCTION provision_new_tenant_communication_access()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE roles
       SET permissions = ARRAY(
           SELECT DISTINCT permission
             FROM UNNEST(permissions || ARRAY['messaging:edit']::TEXT[]) AS permission
            ORDER BY permission
       ), updated_at = NOW()
     WHERE tenant_id = NEW.id AND key = 'teacher' AND deleted_at IS NULL;

    INSERT INTO roles (tenant_id, key, name, description, permissions, is_system)
    VALUES (
        NEW.id, 'communication_manager', 'Communication Manager',
        'Prepares, reviews, publishes, and tracks campus communication.',
        ARRAY[
            'messaging:view', 'messaging:create', 'messaging:edit',
            'messaging:delete', 'messaging:send', 'messaging:manage'
        ]::TEXT[], TRUE
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zz_provision_new_tenant_communication_access ON tenants;
CREATE TRIGGER zz_provision_new_tenant_communication_access
    AFTER INSERT ON tenants
    FOR EACH ROW EXECUTE FUNCTION provision_new_tenant_communication_access();

INSERT INTO role_record_scope_grants (tenant_id, role_id, scope_family, scope_kind)
SELECT role.tenant_id, role.id, 'messaging.announcements',
       CASE
           WHEN role.key = 'teacher' THEN 'assigned'
           WHEN role.key IN ('student', 'staff_member', 'registrar') THEN 'self'
           ELSE 'campus'
       END
  FROM roles AS role
 WHERE role.key IN (
    'teacher', 'student', 'staff_member', 'registrar', 'communication_manager'
 ) AND role.deleted_at IS NULL
ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
    WHERE deleted_at IS NULL
    DO NOTHING;

CREATE OR REPLACE FUNCTION provision_communication_role_scopes()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.key IN (
        'teacher', 'student', 'staff_member', 'registrar', 'communication_manager'
    ) THEN
        INSERT INTO role_record_scope_grants (
            tenant_id, role_id, scope_family, scope_kind
        ) VALUES (
            NEW.tenant_id, NEW.id, 'messaging.announcements',
            CASE
                WHEN NEW.key = 'teacher' THEN 'assigned'
                WHEN NEW.key IN ('student', 'staff_member', 'registrar') THEN 'self'
                ELSE 'campus'
            END
        )
        ON CONFLICT (tenant_id, role_id, scope_family, scope_kind)
            WHERE deleted_at IS NULL
            DO NOTHING;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS provision_communication_role_scopes_after_insert ON roles;
CREATE TRIGGER provision_communication_role_scopes_after_insert
    AFTER INSERT ON roles
    FOR EACH ROW EXECUTE FUNCTION provision_communication_role_scopes();
