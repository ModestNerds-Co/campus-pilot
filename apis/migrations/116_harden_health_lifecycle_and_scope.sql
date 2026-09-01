-- Harden Health visibility and terminal clinical decisions.
--
-- Health supports a person's own record or the complete campus worklist. It
-- has no assigned-person query model. Resolved care items, ended medication
-- plans, and decided follow-ups are retained as final evidence.

UPDATE role_record_scope_grants
   SET deleted_at = NOW(), updated_at = NOW()
 WHERE scope_family IN ('health.patients', 'health.care')
   AND scope_kind = 'assigned'
   AND deleted_at IS NULL;

CREATE OR REPLACE FUNCTION enforce_health_record_scope_policy()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.deleted_at IS NULL
       AND NEW.scope_family IN ('health.patients', 'health.care')
       AND NEW.scope_kind NOT IN ('self', 'campus') THEN
        RAISE EXCEPTION 'Health record scopes support only self or campus visibility'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS enforce_health_record_scope_policy
    ON role_record_scope_grants;
CREATE TRIGGER enforce_health_record_scope_policy
    BEFORE INSERT OR UPDATE OF scope_family, scope_kind, deleted_at
    ON role_record_scope_grants
    FOR EACH ROW EXECUTE FUNCTION enforce_health_record_scope_policy();

CREATE OR REPLACE FUNCTION reject_terminal_health_record_mutation()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_TABLE_NAME = 'health_care_items' AND OLD.status = 'resolved' THEN
        RAISE EXCEPTION 'Resolved care items are final and cannot be changed'
            USING ERRCODE = '23514';
    ELSIF TG_TABLE_NAME = 'health_medication_plans' AND OLD.status = 'ended' THEN
        RAISE EXCEPTION 'Ended medication plans are final and cannot be changed'
            USING ERRCODE = '23514';
    ELSIF TG_TABLE_NAME = 'health_follow_ups'
          AND OLD.status IN ('completed', 'cancelled') THEN
        RAISE EXCEPTION 'Completed or cancelled follow-ups are final and cannot be changed'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS health_care_items_terminal
    ON health_care_items;
CREATE TRIGGER health_care_items_terminal
    BEFORE UPDATE ON health_care_items
    FOR EACH ROW EXECUTE FUNCTION reject_terminal_health_record_mutation();

DROP TRIGGER IF EXISTS health_medication_plans_terminal
    ON health_medication_plans;
CREATE TRIGGER health_medication_plans_terminal
    BEFORE UPDATE ON health_medication_plans
    FOR EACH ROW EXECUTE FUNCTION reject_terminal_health_record_mutation();

DROP TRIGGER IF EXISTS health_follow_ups_terminal
    ON health_follow_ups;
CREATE TRIGGER health_follow_ups_terminal
    BEFORE UPDATE ON health_follow_ups
    FOR EACH ROW EXECUTE FUNCTION reject_terminal_health_record_mutation();
