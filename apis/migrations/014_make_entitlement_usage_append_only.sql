-- Committed entitlement usage is immutable. Tenant deletion may still cascade
-- rows according to the table's ownership boundary, but usage cannot be edited
-- in place while the tenant exists.

CREATE OR REPLACE FUNCTION prevent_entitlement_usage_event_update()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'entitlement usage events are append-only';
END$$;

DROP TRIGGER IF EXISTS immutable_entitlement_usage_events ON entitlement_usage_events;
CREATE TRIGGER immutable_entitlement_usage_events
    BEFORE UPDATE ON entitlement_usage_events
    FOR EACH ROW
    EXECUTE FUNCTION prevent_entitlement_usage_event_update();
