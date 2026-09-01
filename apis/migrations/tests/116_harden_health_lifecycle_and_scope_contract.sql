-- Health lifecycle and visibility contract. Run after migration 116.

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
          FROM role_record_scope_grants
         WHERE scope_family IN ('health.patients', 'health.care')
           AND scope_kind NOT IN ('self', 'campus')
           AND deleted_at IS NULL
    ) THEN
        RAISE EXCEPTION 'Health has an unsupported active record scope';
    END IF;
END;
$$;

DO $$
DECLARE
    trigger_count BIGINT;
BEGIN
    SELECT COUNT(*) INTO trigger_count
      FROM pg_trigger
     WHERE tgname IN (
        'enforce_health_record_scope_policy',
        'health_care_items_terminal',
        'health_medication_plans_terminal',
        'health_follow_ups_terminal'
     )
       AND NOT tgisinternal;
    IF trigger_count <> 4 THEN
        RAISE EXCEPTION 'Health scope or terminal lifecycle triggers are missing';
    END IF;
END;
$$;

DO $$
DECLARE
    sample_role RECORD;
BEGIN
    SELECT tenant_id, id INTO sample_role
      FROM roles
     WHERE deleted_at IS NULL
     LIMIT 1;
    IF sample_role.id IS NULL THEN
        RAISE EXCEPTION 'A role is required to verify the Health scope invariant';
    END IF;

    BEGIN
        INSERT INTO role_record_scope_grants (
            tenant_id, role_id, scope_family, scope_kind
        ) VALUES (
            sample_role.tenant_id, sample_role.id, 'health.care', 'assigned'
        );
        RAISE EXCEPTION 'Health assigned scope was accepted';
    EXCEPTION
        WHEN check_violation THEN NULL;
    END;
END;
$$;

CREATE TEMP TABLE health_care_items (status TEXT NOT NULL);
CREATE TRIGGER verify_health_care_items_terminal
    BEFORE UPDATE ON health_care_items
    FOR EACH ROW EXECUTE FUNCTION reject_terminal_health_record_mutation();
INSERT INTO health_care_items (status) VALUES ('resolved');
DO $$
BEGIN
    BEGIN
        UPDATE health_care_items SET status = 'active';
        RAISE EXCEPTION 'Resolved care item mutation was accepted';
    EXCEPTION
        WHEN check_violation THEN NULL;
    END;
END;
$$;
DROP TABLE health_care_items;

CREATE TEMP TABLE health_medication_plans (status TEXT NOT NULL);
CREATE TRIGGER verify_health_medication_plans_terminal
    BEFORE UPDATE ON health_medication_plans
    FOR EACH ROW EXECUTE FUNCTION reject_terminal_health_record_mutation();
INSERT INTO health_medication_plans (status) VALUES ('ended');
DO $$
BEGIN
    BEGIN
        UPDATE health_medication_plans SET status = 'active';
        RAISE EXCEPTION 'Ended medication plan mutation was accepted';
    EXCEPTION
        WHEN check_violation THEN NULL;
    END;
END;
$$;
DROP TABLE health_medication_plans;

CREATE TEMP TABLE health_follow_ups (status TEXT NOT NULL);
CREATE TRIGGER verify_health_follow_ups_terminal
    BEFORE UPDATE ON health_follow_ups
    FOR EACH ROW EXECUTE FUNCTION reject_terminal_health_record_mutation();
INSERT INTO health_follow_ups (status) VALUES ('completed');
DO $$
BEGIN
    BEGIN
        UPDATE health_follow_ups SET status = 'open';
        RAISE EXCEPTION 'Completed follow-up mutation was accepted';
    EXCEPTION
        WHEN check_violation THEN NULL;
    END;
END;
$$;
DROP TABLE health_follow_ups;

SELECT 'health lifecycle and scope contract passed' AS result;
