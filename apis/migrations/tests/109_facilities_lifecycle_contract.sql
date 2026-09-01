-- Facilities lifecycle contract checks. Run after migration 109.
-- All test records are rolled back.

BEGIN;

DO $$
DECLARE
    test_tenant_id UUID;
    test_user_id UUID;
    test_employee_id UUID := gen_random_uuid();
    test_location_id UUID := gen_random_uuid();
    test_request_id UUID := gen_random_uuid();
    test_work_order_id UUID := gen_random_uuid();
    test_completion_id UUID := gen_random_uuid();
BEGIN
    SELECT account.tenant_id, account.id
      INTO test_tenant_id, test_user_id
      FROM users AS account
     WHERE account.deleted_at IS NULL AND account.is_active
     ORDER BY account.created_at, account.id
     LIMIT 1;

    IF test_user_id IS NULL THEN
        RAISE EXCEPTION 'Facilities lifecycle contract requires one active account';
    END IF;

    INSERT INTO employees (
        id, tenant_id, account_id, employee_number, display_name, employment_status
    ) VALUES (
        test_employee_id,
        test_tenant_id,
        test_user_id,
        'FAC-CONTRACT',
        'Facilities contract employee',
        'active'
    );

    INSERT INTO facility_locations (
        id, tenant_id, kind, code, name, created_by, updated_by
    ) VALUES (
        test_location_id,
        test_tenant_id,
        'site',
        'FAC-CONTRACT',
        'Facilities contract site',
        test_user_id,
        test_user_id
    );

    INSERT INTO facility_service_requests (
        id, tenant_id, reference, location_id, reporter_user_id, priority,
        summary, description, created_by, updated_by
    ) VALUES (
        test_request_id,
        test_tenant_id,
        'FSR-CONTRACT',
        test_location_id,
        test_user_id,
        'normal',
        'Lifecycle contract request',
        'Validates the Facilities request and work-order lifecycle.',
        test_user_id,
        test_user_id
    );

    INSERT INTO facility_work_orders (
        id, tenant_id, reference, service_request_id, location_id,
        assigned_employee_id, title, status, created_by, updated_by
    ) VALUES (
        test_work_order_id,
        test_tenant_id,
        'FWO-CONTRACT',
        test_request_id,
        test_location_id,
        test_employee_id,
        'Complete lifecycle contract',
        'assigned',
        test_user_id,
        test_user_id
    );

    UPDATE facility_service_requests
       SET status = 'assigned', version = version + 1, updated_by = test_user_id
     WHERE id = test_request_id AND tenant_id = test_tenant_id;

    UPDATE facility_work_orders
       SET status = 'in_progress', started_by = test_user_id,
           started_at = NOW(), version = version + 1, updated_by = test_user_id
     WHERE id = test_work_order_id AND tenant_id = test_tenant_id;

    INSERT INTO facility_work_order_completion_submissions (
        id, tenant_id, work_order_id, summary, submitted_by
    ) VALUES (
        test_completion_id,
        test_tenant_id,
        test_work_order_id,
        'The assigned facilities work was completed.',
        test_user_id
    );

    UPDATE facility_work_orders
       SET status = 'ready_for_inspection', version = version + 1,
           updated_by = test_user_id
     WHERE id = test_work_order_id AND tenant_id = test_tenant_id;

    INSERT INTO facility_work_order_inspections (
        tenant_id, work_order_id, outcome, notes, inspected_by
    ) VALUES (
        test_tenant_id,
        test_work_order_id,
        'pass',
        'The completed work passed inspection.',
        test_user_id
    );

    UPDATE facility_work_orders
       SET status = 'completed', completed_by = test_user_id,
           completed_at = NOW(), version = version + 1, updated_by = test_user_id
     WHERE id = test_work_order_id AND tenant_id = test_tenant_id;

    UPDATE facility_service_requests
       SET status = 'resolved', resolved_by = test_user_id, resolved_at = NOW(),
           resolution_summary = 'The linked work order passed inspection.',
           version = version + 1, updated_by = test_user_id
     WHERE id = test_request_id AND tenant_id = test_tenant_id;

    IF NOT EXISTS (
        SELECT 1 FROM facility_work_orders
        WHERE id = test_work_order_id AND tenant_id = test_tenant_id
          AND status = 'completed'
    ) OR NOT EXISTS (
        SELECT 1 FROM facility_service_requests
        WHERE id = test_request_id AND tenant_id = test_tenant_id
          AND status = 'resolved'
    ) THEN
        RAISE EXCEPTION 'Facilities lifecycle did not reach its inspected resolution state';
    END IF;

    BEGIN
        UPDATE facility_work_order_completion_submissions
           SET summary = 'Mutated evidence'
         WHERE id = test_completion_id AND tenant_id = test_tenant_id;
        RAISE EXCEPTION 'Facilities completion evidence accepted a mutation';
    EXCEPTION WHEN OTHERS THEN
        IF POSITION('append-only' IN SQLERRM) = 0 THEN
            RAISE;
        END IF;
    END;
END;
$$;

ROLLBACK;

SELECT 'Facilities lifecycle contract passed' AS result;
