-- Contract checks for Transport ownership, snapshots, roles, and lifecycle guards.

DO $$
DECLARE
    tenant UUID;
    owner_id UUID;
    learner UUID;
    employee UUID;
    vehicle UUID;
    driver UUID;
    route UUID;
    stop_a UUID;
    stop_b UUID;
    assignment UUID;
    run UUID;
    run_stop_a UUID;
    run_stop_b UUID;
    manifest UUID;
BEGIN
    INSERT INTO tenants (name, slug) VALUES ('Transport Contract', 'transport-contract-' || gen_random_uuid()) RETURNING id INTO tenant;
    INSERT INTO users (tenant_id, email, password_hash, full_name, roles)
    VALUES (tenant, gen_random_uuid() || '@example.test', 'contract', 'Contract Owner', ARRAY['campus_owner'])
    RETURNING id INTO owner_id;
    INSERT INTO learners (tenant_id, learner_number, display_name, first_names, surname, date_of_birth, status)
    VALUES (tenant, 'TRN-001', 'Tariro Moyo', 'Tariro', 'Moyo', DATE '2012-01-01', 'active') RETURNING id INTO learner;
    INSERT INTO employees (tenant_id, employee_number, display_name, first_names, surname, work_email, employment_status)
    VALUES (tenant, 'EMP-TRN-1', 'Bus Driver', 'Bus', 'Driver', 'driver-' || gen_random_uuid() || '@example.test', 'active')
    RETURNING id INTO employee;
    INSERT INTO vehicles (tenant_id, registration_number, make, model, capacity, status)
    VALUES (tenant, 'TRN-001', 'Test', 'Bus', 20, 'active') RETURNING id INTO vehicle;
    INSERT INTO drivers (tenant_id, employee_id, license_number, license_expiry, status)
    VALUES (tenant, employee, 'LIC-TRN-1', CURRENT_DATE + 30, 'active') RETURNING id INTO driver;
    INSERT INTO transport_routes (tenant_id, code, name, direction, created_by, updated_by)
    VALUES (tenant, 'AM-01', 'Morning route', 'inbound', owner_id, owner_id) RETURNING id INTO route;
    INSERT INTO transport_route_stops (tenant_id, route_id, code, name, stop_order, planned_time, created_by, updated_by)
    VALUES (tenant, route, 'A', 'First stop', 1, TIME '07:00', owner_id, owner_id) RETURNING id INTO stop_a;
    INSERT INTO transport_route_stops (tenant_id, route_id, code, name, stop_order, planned_time, created_by, updated_by)
    VALUES (tenant, route, 'B', 'Campus', 2, TIME '07:45', owner_id, owner_id) RETURNING id INTO stop_b;
    INSERT INTO transport_rider_assignments (
        tenant_id, learner_id, route_id, boarding_stop_id, alighting_stop_id,
        effective_from, created_by, updated_by
    ) VALUES (tenant, learner, route, stop_a, stop_b, CURRENT_DATE, owner_id, owner_id)
    RETURNING id INTO assignment;
    INSERT INTO transport_service_runs (
        tenant_id, reference, route_id, service_date, vehicle_id, driver_id,
        route_code_snapshot, route_name_snapshot, direction_snapshot,
        vehicle_registration_snapshot, driver_name_snapshot, capacity_snapshot,
        created_by, updated_by
    ) VALUES (
        tenant, 'TRN-' || TO_CHAR(CURRENT_DATE, 'YYYYMMDD') || '-AM-01', route, CURRENT_DATE,
        vehicle, driver, 'AM-01', 'Morning route', 'inbound', 'TRN-001', 'Bus Driver', 20,
        owner_id, owner_id
    ) RETURNING id INTO run;
    INSERT INTO transport_run_stops (
        tenant_id, run_id, source_stop_id, stop_order, code_snapshot, name_snapshot, planned_time_snapshot
    ) VALUES (tenant, run, stop_a, 1, 'A', 'First stop', TIME '07:00') RETURNING id INTO run_stop_a;
    INSERT INTO transport_run_stops (
        tenant_id, run_id, source_stop_id, stop_order, code_snapshot, name_snapshot, planned_time_snapshot
    ) VALUES (tenant, run, stop_b, 2, 'B', 'Campus', TIME '07:45') RETURNING id INTO run_stop_b;
    INSERT INTO transport_manifest_entries (
        tenant_id, run_id, source_assignment_id, learner_id,
        learner_number_snapshot, learner_name_snapshot,
        boarding_run_stop_id, alighting_run_stop_id
    ) VALUES (tenant, run, assignment, learner, 'TRN-001', 'Tariro Moyo', run_stop_a, run_stop_b) RETURNING id INTO manifest;

    BEGIN
        UPDATE transport_run_stops SET name_snapshot = 'Changed' WHERE id = run_stop_a;
        RAISE EXCEPTION 'run stop snapshot unexpectedly changed';
    EXCEPTION WHEN OTHERS THEN
        IF SQLERRM = 'run stop snapshot unexpectedly changed' THEN RAISE; END IF;
    END;

    BEGIN
        UPDATE transport_manifest_entries
        SET status = 'exception', exception_kind = NULL, note = NULL,
            marked_by = owner_id, marked_at = NOW()
        WHERE id = manifest;
        RAISE EXCEPTION 'invalid manifest exception unexpectedly accepted';
    EXCEPTION WHEN CHECK_VIOLATION THEN NULL;
    END;

    IF NOT EXISTS (
        SELECT 1 FROM roles WHERE tenant_id = tenant AND key = 'transport_officer'
          AND permissions @> ARRAY['transport:view', 'transport:operate']::TEXT[]
          AND NOT permissions && ARRAY['transport:configure', 'transport:manage']::TEXT[]
    ) THEN
        RAISE EXCEPTION 'Transport Officer boundary was not provisioned';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM roles WHERE tenant_id = tenant AND key = 'transport_manager'
          AND permissions @> ARRAY['transport:view', 'transport:configure', 'transport:operate', 'transport:manage']::TEXT[]
    ) THEN
        RAISE EXCEPTION 'Transport Manager boundary was not provisioned';
    END IF;

END;
$$;

SELECT 'Transport operations contract passed' AS result;
