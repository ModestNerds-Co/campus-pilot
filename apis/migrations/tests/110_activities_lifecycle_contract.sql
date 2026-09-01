-- Activities lifecycle and completion-snapshot contract. All records roll back.

BEGIN;

DO $$
DECLARE
    test_tenant_id UUID;
    test_user_id UUID;
    test_employee_id UUID := gen_random_uuid();
    test_learner_id UUID := gen_random_uuid();
    test_activity_id UUID := gen_random_uuid();
    test_group_id UUID := gen_random_uuid();
    test_leader_id UUID := gen_random_uuid();
    test_membership_id UUID := gen_random_uuid();
    test_session_id UUID := gen_random_uuid();
    test_snapshot_id UUID := gen_random_uuid();
BEGIN
    SELECT account.tenant_id, account.id INTO test_tenant_id, test_user_id
      FROM users AS account
     WHERE account.deleted_at IS NULL AND account.is_active
     ORDER BY account.created_at, account.id LIMIT 1;

    IF test_user_id IS NULL THEN
        RAISE EXCEPTION 'Activities lifecycle contract requires one active account';
    END IF;

    INSERT INTO employees (id, tenant_id, employee_number, display_name, employment_status)
    VALUES (test_employee_id, test_tenant_id, 'ACT-EMP-CONTRACT', 'Activities contract leader', 'active');

    INSERT INTO learners (id, tenant_id, learner_number, display_name, date_of_birth, status)
    VALUES (test_learner_id, test_tenant_id, 'ACT-LRN-CONTRACT', 'Activities contract learner', DATE '2012-01-01', 'active');

    INSERT INTO activity_catalog_items (
        id, tenant_id, code, name, category, created_by, updated_by
    ) VALUES (
        test_activity_id, test_tenant_id, 'ACT-CONTRACT', 'Activities contract', 'club', test_user_id, test_user_id
    );

    INSERT INTO activity_groups (
        id, tenant_id, activity_id, code, name, starts_on, ends_on,
        consent_required, created_by, updated_by
    ) VALUES (
        test_group_id, test_tenant_id, test_activity_id, 'ACT-GRP-CONTRACT',
        'Activities contract group', CURRENT_DATE - 1, CURRENT_DATE + 30,
        FALSE, test_user_id, test_user_id
    );

    INSERT INTO activity_group_leaders (
        id, tenant_id, group_id, employee_id, leader_role, starts_on,
        created_by, updated_by
    ) VALUES (
        test_leader_id, test_tenant_id, test_group_id, test_employee_id, 'lead',
        CURRENT_DATE - 1, test_user_id, test_user_id
    );

    INSERT INTO activity_group_memberships (
        id, tenant_id, group_id, learner_id, joined_on, consent_status,
        created_by, updated_by
    ) VALUES (
        test_membership_id, test_tenant_id, test_group_id, test_learner_id,
        CURRENT_DATE - 1, 'not_required', test_user_id, test_user_id
    );

    UPDATE activity_groups
       SET status='active', activated_at=NOW(), activated_by=test_user_id,
           updated_by=test_user_id, version=version+1, updated_at=NOW()
     WHERE tenant_id=test_tenant_id AND id=test_group_id;

    INSERT INTO activity_sessions (
        id, tenant_id, group_id, reference, title, starts_at, ends_at,
        created_by, updated_by
    ) VALUES (
        test_session_id, test_tenant_id, test_group_id, 'ACT-SESSION-CONTRACT',
        'Activities contract session', NOW(), NOW() + INTERVAL '1 hour',
        test_user_id, test_user_id
    );

    INSERT INTO activity_session_participation (
        tenant_id, session_id, group_id, membership_id, learner_id, mark, marked_by
    ) VALUES (
        test_tenant_id, test_session_id, test_group_id, test_membership_id,
        test_learner_id, 'present', test_user_id
    );

    INSERT INTO activity_session_completion_snapshots (
        id, tenant_id, session_id, group_id, roster_count, roster_fingerprint,
        summary, completed_by
    ) VALUES (
        test_snapshot_id, test_tenant_id, test_session_id, test_group_id, 1,
        'contract-fingerprint', 'Completed contract session', test_user_id
    );

    INSERT INTO activity_session_completion_members (
        tenant_id, snapshot_id, session_id, group_id, membership_id, learner_id,
        learner_number_snapshot, learner_name_snapshot, mark
    ) VALUES (
        test_tenant_id, test_snapshot_id, test_session_id, test_group_id,
        test_membership_id, test_learner_id, 'ACT-LRN-CONTRACT',
        'Activities contract learner', 'present'
    );

    UPDATE activity_sessions
       SET status='completed', completed_at=NOW(), completed_by=test_user_id,
           completion_summary='Completed contract session', updated_by=test_user_id,
           version=version+1, updated_at=NOW()
     WHERE tenant_id=test_tenant_id AND id=test_session_id;

    IF NOT EXISTS (
        SELECT 1 FROM activity_sessions
         WHERE tenant_id=test_tenant_id AND id=test_session_id AND status='completed'
    ) OR NOT EXISTS (
        SELECT 1 FROM activity_session_completion_members
         WHERE tenant_id=test_tenant_id AND snapshot_id=test_snapshot_id AND mark='present'
    ) THEN
        RAISE EXCEPTION 'Activities lifecycle did not retain completed participation';
    END IF;

    BEGIN
        UPDATE activity_session_completion_members
           SET mark='absent'
         WHERE tenant_id=test_tenant_id AND snapshot_id=test_snapshot_id;
        RAISE EXCEPTION 'Activities completion evidence accepted a mutation';
    EXCEPTION WHEN OTHERS THEN
        IF POSITION('append-only' IN SQLERRM)=0 THEN RAISE; END IF;
    END;
END;
$$;

ROLLBACK;

SELECT 'Activities lifecycle contract passed' AS result;
