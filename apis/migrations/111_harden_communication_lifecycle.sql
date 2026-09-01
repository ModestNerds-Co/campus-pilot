-- Enforce the Communication lifecycle and its manager-role boundary.
--
-- Communication drafts may move only through review, publication, and reasoned
-- cancellation. Delivery recipients and identity remain immutable after review.

CREATE OR REPLACE FUNCTION communication_manager_baseline_permissions()
RETURNS TEXT[] AS $$
    SELECT ARRAY[
        'messaging:create',
        'messaging:delete',
        'messaging:edit',
        'messaging:manage',
        'messaging:send',
        'messaging:view'
    ]::TEXT[];
$$ LANGUAGE SQL IMMUTABLE;

UPDATE roles
SET permissions = communication_manager_baseline_permissions(),
    updated_at = NOW()
WHERE key = 'communication_manager'
  AND deleted_at IS NULL
  AND permissions IS DISTINCT FROM communication_manager_baseline_permissions();

CREATE OR REPLACE FUNCTION enforce_seeded_communication_role_boundary()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.deleted_at IS NULL AND NEW.key = 'communication_manager' THEN
        NEW.permissions := communication_manager_baseline_permissions();
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS enforce_seeded_communication_role_boundary ON roles;
CREATE TRIGGER enforce_seeded_communication_role_boundary
    BEFORE INSERT OR UPDATE OF key, permissions, deleted_at ON roles
    FOR EACH ROW EXECUTE FUNCTION enforce_seeded_communication_role_boundary();

CREATE OR REPLACE FUNCTION enforce_communication_announcement_transition()
RETURNS TRIGGER AS $$
DECLARE
    legal_transition BOOLEAN;
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Communication announcements cannot be hard-deleted';
    END IF;

    IF OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
       OR OLD.id IS DISTINCT FROM NEW.id
       OR OLD.created_by IS DISTINCT FROM NEW.created_by
       OR OLD.created_at IS DISTINCT FROM NEW.created_at THEN
        RAISE EXCEPTION 'Communication announcement identity is immutable';
    END IF;

    IF OLD.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'Deleted Communication announcements cannot change';
    END IF;

    IF NEW.version IS DISTINCT FROM OLD.version + 1 THEN
        RAISE EXCEPTION 'Communication announcement versions must advance by one';
    END IF;

    IF NEW.deleted_at IS NOT NULL THEN
        IF OLD.status IS DISTINCT FROM 'draft'
           OR NEW.status IS DISTINCT FROM 'draft'
           OR OLD.title IS DISTINCT FROM NEW.title
           OR OLD.body IS DISTINCT FROM NEW.body
           OR OLD.priority IS DISTINCT FROM NEW.priority THEN
            RAISE EXCEPTION 'Only an unchanged draft Communication announcement can be deleted';
        END IF;
        RETURN NEW;
    END IF;

    legal_transition :=
        (OLD.status = 'draft' AND NEW.status = 'draft')
        OR (OLD.status = 'draft' AND NEW.status = 'submitted')
        OR (OLD.status = 'submitted' AND NEW.status = 'draft')
        OR (OLD.status = 'submitted' AND NEW.status = 'published')
        OR (OLD.status = 'published' AND NEW.status = 'cancelled');

    IF NOT legal_transition THEN
        RAISE EXCEPTION 'Invalid Communication announcement lifecycle transition';
    END IF;

    IF OLD.status IS DISTINCT FROM NEW.status
       AND (OLD.title IS DISTINCT FROM NEW.title
            OR OLD.body IS DISTINCT FROM NEW.body
            OR OLD.priority IS DISTINCT FROM NEW.priority) THEN
        RAISE EXCEPTION 'Communication content cannot change during a lifecycle transition';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS communication_announcements_transition_guard
    ON communication_announcements;
CREATE TRIGGER communication_announcements_transition_guard
    BEFORE UPDATE OR DELETE ON communication_announcements
    FOR EACH ROW EXECUTE FUNCTION enforce_communication_announcement_transition();

CREATE OR REPLACE FUNCTION enforce_communication_delivery_transition()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Communication delivery evidence cannot be deleted';
    END IF;

    IF OLD.tenant_id IS DISTINCT FROM NEW.tenant_id
       OR OLD.id IS DISTINCT FROM NEW.id
       OR OLD.announcement_id IS DISTINCT FROM NEW.announcement_id
       OR OLD.recipient_user_id IS DISTINCT FROM NEW.recipient_user_id
       OR OLD.recipient_name_snapshot IS DISTINCT FROM NEW.recipient_name_snapshot
       OR OLD.channel IS DISTINCT FROM NEW.channel
       OR OLD.created_at IS DISTINCT FROM NEW.created_at THEN
        RAISE EXCEPTION 'Communication delivery identity is immutable';
    END IF;

    IF OLD.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'Retired Communication deliveries cannot change';
    END IF;

    IF OLD.status = 'pending' AND NEW.deleted_at IS NOT NULL THEN
        IF NEW.status IS DISTINCT FROM 'pending'
           OR NEW.delivered_at IS NOT NULL
           OR NEW.read_at IS NOT NULL THEN
            RAISE EXCEPTION 'Only pending Communication deliveries can be retired';
        END IF;
        RETURN NEW;
    END IF;

    IF OLD.status = 'pending' AND NEW.status = 'delivered' THEN
        IF NEW.deleted_at IS NOT NULL
           OR NEW.delivered_at IS NULL
           OR NEW.read_at IS NOT NULL THEN
            RAISE EXCEPTION 'Published Communication deliveries require a delivery timestamp';
        END IF;
        RETURN NEW;
    END IF;

    IF OLD.status = 'delivered' AND NEW.status = 'delivered' THEN
        IF NEW.deleted_at IS NOT NULL
           OR NEW.delivered_at IS DISTINCT FROM OLD.delivered_at
           OR OLD.read_at IS NOT NULL
           OR NEW.read_at IS NULL THEN
            RAISE EXCEPTION 'Delivered Communication evidence permits one read receipt only';
        END IF;
        RETURN NEW;
    END IF;

    RAISE EXCEPTION 'Invalid Communication delivery lifecycle transition';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS communication_deliveries_transition_guard
    ON communication_deliveries;
CREATE TRIGGER communication_deliveries_transition_guard
    BEFORE UPDATE OR DELETE ON communication_deliveries
    FOR EACH ROW EXECUTE FUNCTION enforce_communication_delivery_transition();
