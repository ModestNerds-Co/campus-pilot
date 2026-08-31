-- Owns cross-process Agent worker readiness and its append-only transition proof.
-- Coverage fields are SHA-256 fingerprints of canonical non-secret inventories;
-- credentials, wrapping keys, provider payloads, and raw configuration never belong here.

CREATE TABLE IF NOT EXISTS agent_worker_instances (
    id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
    worker_key TEXT NOT NULL CHECK (
        LENGTH(worker_key) BETWEEN 3 AND 128
        AND worker_key ~ '^[a-z0-9][a-z0-9._:-]*$'
    ),
    status TEXT NOT NULL DEFAULT 'starting' CHECK (
        status IN ('starting', 'ready', 'draining', 'unavailable')
    ),
    status_reason_code TEXT CHECK (
        status_reason_code IS NULL
        OR (
            LENGTH(status_reason_code) BETWEEN 3 AND 64
            AND status_reason_code ~ '^[a-z][a-z0-9_]*$'
        )
    ),
    artifact_key_coverage_sha256 BYTEA CHECK (
        artifact_key_coverage_sha256 IS NULL
        OR OCTET_LENGTH(artifact_key_coverage_sha256) = 32
    ),
    provider_key_coverage_sha256 BYTEA CHECK (
        provider_key_coverage_sha256 IS NULL
        OR OCTET_LENGTH(provider_key_coverage_sha256) = 32
    ),
    provider_route_coverage_sha256 BYTEA CHECK (
        provider_route_coverage_sha256 IS NULL
        OR OCTET_LENGTH(provider_route_coverage_sha256) = 32
    ),
    startup_coverage_completed_at TIMESTAMPTZ,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    status_changed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    heartbeat_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    heartbeat_expires_at TIMESTAMPTZ NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_worker_instances_heartbeat_window_check CHECK (
        heartbeat_at >= started_at
        AND heartbeat_expires_at > heartbeat_at
        AND heartbeat_expires_at <= heartbeat_at + INTERVAL '120 seconds'
    ),
    CONSTRAINT agent_worker_instances_coverage_shape_check CHECK (
        (
            artifact_key_coverage_sha256 IS NULL
            AND provider_key_coverage_sha256 IS NULL
            AND provider_route_coverage_sha256 IS NULL
            AND startup_coverage_completed_at IS NULL
        )
        OR (
            artifact_key_coverage_sha256 IS NOT NULL
            AND provider_key_coverage_sha256 IS NOT NULL
            AND provider_route_coverage_sha256 IS NOT NULL
            AND startup_coverage_completed_at IS NOT NULL
            AND startup_coverage_completed_at >= started_at
        )
    ),
    CONSTRAINT agent_worker_instances_status_shape_check CHECK (
        (
            status = 'starting'
            AND status_reason_code IS NULL
            AND startup_coverage_completed_at IS NULL
        )
        OR (
            status = 'ready'
            AND status_reason_code IS NULL
            AND startup_coverage_completed_at IS NOT NULL
        )
        OR (
            status IN ('draining', 'unavailable')
            AND status_reason_code IS NOT NULL
        )
    ),
    CONSTRAINT agent_worker_instances_lifecycle_time_check CHECK (
        status_changed_at >= started_at
        AND created_at = started_at
        AND updated_at >= created_at
        AND (deleted_at IS NULL OR status = 'unavailable')
    )
);

CREATE INDEX IF NOT EXISTS idx_agent_worker_instances_readiness
    ON agent_worker_instances(status, heartbeat_expires_at)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_agent_worker_instances_cleanup
    ON agent_worker_instances(status, status_changed_at)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_agent_worker_instances_worker_key
    ON agent_worker_instances(worker_key, started_at DESC);

CREATE TABLE IF NOT EXISTS agent_worker_readiness_events (
    id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
    worker_instance_id UUID NOT NULL REFERENCES agent_worker_instances(id) ON DELETE RESTRICT,
    worker_key TEXT NOT NULL CHECK (
        LENGTH(worker_key) BETWEEN 3 AND 128
        AND worker_key ~ '^[a-z0-9][a-z0-9._:-]*$'
    ),
    worker_version BIGINT NOT NULL CHECK (worker_version > 0),
    event_kind TEXT NOT NULL CHECK (
        event_kind IN ('registered', 'ready', 'draining', 'unavailable', 'retired')
    ),
    status TEXT NOT NULL CHECK (
        status IN ('starting', 'ready', 'draining', 'unavailable')
    ),
    status_reason_code TEXT CHECK (
        status_reason_code IS NULL
        OR (
            LENGTH(status_reason_code) BETWEEN 3 AND 64
            AND status_reason_code ~ '^[a-z][a-z0-9_]*$'
        )
    ),
    artifact_key_coverage_sha256 BYTEA CHECK (
        artifact_key_coverage_sha256 IS NULL
        OR OCTET_LENGTH(artifact_key_coverage_sha256) = 32
    ),
    provider_key_coverage_sha256 BYTEA CHECK (
        provider_key_coverage_sha256 IS NULL
        OR OCTET_LENGTH(provider_key_coverage_sha256) = 32
    ),
    provider_route_coverage_sha256 BYTEA CHECK (
        provider_route_coverage_sha256 IS NULL
        OR OCTET_LENGTH(provider_route_coverage_sha256) = 32
    ),
    startup_coverage_completed_at TIMESTAMPTZ,
    heartbeat_at TIMESTAMPTZ NOT NULL,
    heartbeat_expires_at TIMESTAMPTZ NOT NULL,
    transitioned_at TIMESTAMPTZ NOT NULL,
    deleted_at TIMESTAMPTZ CHECK (deleted_at IS NULL),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_worker_readiness_events_worker_version_unique
        UNIQUE (worker_instance_id, worker_version),
    CONSTRAINT agent_worker_readiness_events_immutable_time_check CHECK (
        updated_at = created_at
    )
);

CREATE INDEX IF NOT EXISTS idx_agent_worker_readiness_events_history
    ON agent_worker_readiness_events(worker_instance_id, transitioned_at DESC);

CREATE OR REPLACE FUNCTION validate_agent_worker_instance_insert()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.status <> 'starting'
       OR NEW.status_reason_code IS NOT NULL
       OR NEW.artifact_key_coverage_sha256 IS NOT NULL
       OR NEW.provider_key_coverage_sha256 IS NOT NULL
       OR NEW.provider_route_coverage_sha256 IS NOT NULL
       OR NEW.startup_coverage_completed_at IS NOT NULL
       OR NEW.version <> 1
       OR NEW.deleted_at IS NOT NULL
       OR NEW.status_changed_at IS DISTINCT FROM NEW.started_at
       OR NEW.heartbeat_at IS DISTINCT FROM NEW.started_at
       OR NEW.created_at IS DISTINCT FROM NEW.started_at
       OR NEW.updated_at IS DISTINCT FROM NEW.created_at THEN
        RAISE EXCEPTION 'Agent worker instances must start in the initial starting state';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_worker_instances_validate_insert
    ON agent_worker_instances;
CREATE TRIGGER agent_worker_instances_validate_insert
    BEFORE INSERT ON agent_worker_instances
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_worker_instance_insert();

CREATE OR REPLACE FUNCTION protect_agent_worker_instance_lifecycle()
RETURNS TRIGGER AS $$
DECLARE
    is_retirement BOOLEAN;
    is_expiry_transition BOOLEAN;
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Agent worker instances are retired, not deleted';
    END IF;

    IF OLD.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'retired Agent worker instances are immutable';
    END IF;

    IF NEW.id IS DISTINCT FROM OLD.id
       OR NEW.worker_key IS DISTINCT FROM OLD.worker_key
       OR NEW.started_at IS DISTINCT FROM OLD.started_at
       OR NEW.created_at IS DISTINCT FROM OLD.created_at
       OR NEW.version <> OLD.version + 1
       OR NEW.updated_at IS DISTINCT FROM STATEMENT_TIMESTAMP()
       OR NEW.updated_at <= OLD.updated_at THEN
        RAISE EXCEPTION 'invalid Agent worker identity or version fence';
    END IF;

    is_retirement := OLD.status = 'unavailable'
        AND NEW.status = 'unavailable'
        AND OLD.deleted_at IS NULL
        AND NEW.deleted_at IS NOT NULL;

    IF is_retirement THEN
        IF NEW.deleted_at IS DISTINCT FROM STATEMENT_TIMESTAMP()
           OR NEW.status_reason_code IS DISTINCT FROM OLD.status_reason_code
           OR NEW.artifact_key_coverage_sha256
                IS DISTINCT FROM OLD.artifact_key_coverage_sha256
           OR NEW.provider_key_coverage_sha256
                IS DISTINCT FROM OLD.provider_key_coverage_sha256
           OR NEW.provider_route_coverage_sha256
                IS DISTINCT FROM OLD.provider_route_coverage_sha256
           OR NEW.startup_coverage_completed_at
                IS DISTINCT FROM OLD.startup_coverage_completed_at
           OR NEW.status_changed_at IS DISTINCT FROM OLD.status_changed_at
           OR NEW.heartbeat_at IS DISTINCT FROM OLD.heartbeat_at
           OR NEW.heartbeat_expires_at IS DISTINCT FROM OLD.heartbeat_expires_at THEN
            RAISE EXCEPTION 'invalid Agent worker retirement';
        END IF;

        RETURN NEW;
    END IF;

    IF NEW.deleted_at IS NOT NULL THEN
        RAISE EXCEPTION 'only unavailable Agent workers can be retired';
    END IF;

    IF OLD.status = NEW.status THEN
        IF OLD.status = 'unavailable'
           OR NEW.status_reason_code IS DISTINCT FROM OLD.status_reason_code
           OR NEW.artifact_key_coverage_sha256
                IS DISTINCT FROM OLD.artifact_key_coverage_sha256
           OR NEW.provider_key_coverage_sha256
                IS DISTINCT FROM OLD.provider_key_coverage_sha256
           OR NEW.provider_route_coverage_sha256
                IS DISTINCT FROM OLD.provider_route_coverage_sha256
           OR NEW.startup_coverage_completed_at
                IS DISTINCT FROM OLD.startup_coverage_completed_at
           OR NEW.status_changed_at IS DISTINCT FROM OLD.status_changed_at
           OR NEW.heartbeat_at IS DISTINCT FROM STATEMENT_TIMESTAMP()
           OR NEW.heartbeat_at <= OLD.heartbeat_at THEN
            RAISE EXCEPTION 'invalid Agent worker heartbeat';
        END IF;

        RETURN NEW;
    END IF;

    IF NOT (
        (OLD.status = 'starting' AND NEW.status IN ('ready', 'draining', 'unavailable'))
        OR (OLD.status = 'ready' AND NEW.status IN ('draining', 'unavailable'))
        OR (OLD.status = 'draining' AND NEW.status = 'unavailable')
    ) THEN
        RAISE EXCEPTION 'invalid Agent worker lifecycle transition';
    END IF;

    IF OLD.startup_coverage_completed_at IS NOT NULL
       AND (
           NEW.artifact_key_coverage_sha256
                IS DISTINCT FROM OLD.artifact_key_coverage_sha256
           OR NEW.provider_key_coverage_sha256
                IS DISTINCT FROM OLD.provider_key_coverage_sha256
           OR NEW.provider_route_coverage_sha256
                IS DISTINCT FROM OLD.provider_route_coverage_sha256
           OR NEW.startup_coverage_completed_at
                IS DISTINCT FROM OLD.startup_coverage_completed_at
       ) THEN
        RAISE EXCEPTION 'Agent worker startup coverage proof is immutable';
    END IF;

    IF OLD.startup_coverage_completed_at IS NULL
       AND NEW.status <> 'ready'
       AND (
           NEW.artifact_key_coverage_sha256 IS NOT NULL
           OR NEW.provider_key_coverage_sha256 IS NOT NULL
           OR NEW.provider_route_coverage_sha256 IS NOT NULL
           OR NEW.startup_coverage_completed_at IS NOT NULL
       ) THEN
        RAISE EXCEPTION 'Agent worker startup coverage is recorded only when ready';
    END IF;

    is_expiry_transition := NEW.status = 'unavailable'
        AND NEW.status_reason_code = 'heartbeat_expired';

    IF is_expiry_transition THEN
        IF OLD.heartbeat_expires_at > STATEMENT_TIMESTAMP()
           OR NEW.status_changed_at IS DISTINCT FROM OLD.heartbeat_expires_at THEN
            RAISE EXCEPTION 'Agent worker expiry requires an expired heartbeat';
        END IF;
    ELSIF NEW.status_changed_at IS DISTINCT FROM STATEMENT_TIMESTAMP() THEN
        RAISE EXCEPTION 'Agent worker status transitions use database time';
    END IF;

    IF NEW.status = 'ready' THEN
        IF NEW.status_reason_code IS NOT NULL
           OR NEW.artifact_key_coverage_sha256 IS NULL
           OR NEW.provider_key_coverage_sha256 IS NULL
           OR NEW.provider_route_coverage_sha256 IS NULL
           OR NEW.startup_coverage_completed_at
                IS DISTINCT FROM STATEMENT_TIMESTAMP()
           OR NEW.heartbeat_at IS DISTINCT FROM STATEMENT_TIMESTAMP() THEN
            RAISE EXCEPTION 'ready Agent workers require current complete startup coverage';
        END IF;
    ELSIF NEW.status_reason_code IS NULL
       OR NEW.heartbeat_at IS DISTINCT FROM OLD.heartbeat_at
       OR NEW.heartbeat_expires_at IS DISTINCT FROM OLD.heartbeat_expires_at THEN
        RAISE EXCEPTION 'non-ready Agent worker transitions retain their last heartbeat';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_worker_instances_protect_lifecycle
    ON agent_worker_instances;
CREATE TRIGGER agent_worker_instances_protect_lifecycle
    BEFORE UPDATE OR DELETE ON agent_worker_instances
    FOR EACH ROW
    EXECUTE FUNCTION protect_agent_worker_instance_lifecycle();

CREATE OR REPLACE FUNCTION validate_agent_worker_readiness_event_insert()
RETURNS TRIGGER AS $$
DECLARE
    stored_worker agent_worker_instances%ROWTYPE;
    expected_event_kind TEXT;
BEGIN
    IF PG_TRIGGER_DEPTH() < 2 THEN
        RAISE EXCEPTION 'Agent worker readiness events are generated by worker lifecycle changes';
    END IF;

    SELECT *
    INTO stored_worker
    FROM agent_worker_instances
    WHERE id = NEW.worker_instance_id;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Agent worker readiness events require a worker instance';
    END IF;

    expected_event_kind := CASE
        WHEN stored_worker.deleted_at IS NOT NULL THEN 'retired'
        WHEN stored_worker.version = 1 THEN 'registered'
        ELSE stored_worker.status
    END;

    IF NEW.worker_key IS DISTINCT FROM stored_worker.worker_key
       OR NEW.worker_version IS DISTINCT FROM stored_worker.version
       OR NEW.event_kind IS DISTINCT FROM expected_event_kind
       OR NEW.status IS DISTINCT FROM stored_worker.status
       OR NEW.status_reason_code IS DISTINCT FROM stored_worker.status_reason_code
       OR NEW.artifact_key_coverage_sha256
            IS DISTINCT FROM stored_worker.artifact_key_coverage_sha256
       OR NEW.provider_key_coverage_sha256
            IS DISTINCT FROM stored_worker.provider_key_coverage_sha256
       OR NEW.provider_route_coverage_sha256
            IS DISTINCT FROM stored_worker.provider_route_coverage_sha256
       OR NEW.startup_coverage_completed_at
            IS DISTINCT FROM stored_worker.startup_coverage_completed_at
       OR NEW.heartbeat_at IS DISTINCT FROM stored_worker.heartbeat_at
       OR NEW.heartbeat_expires_at IS DISTINCT FROM stored_worker.heartbeat_expires_at
       OR NEW.transitioned_at IS DISTINCT FROM stored_worker.status_changed_at
       OR NEW.deleted_at IS NOT NULL
       OR NEW.updated_at IS DISTINCT FROM NEW.created_at THEN
        RAISE EXCEPTION 'Agent worker readiness event does not match its lifecycle state';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_worker_readiness_events_validate_insert
    ON agent_worker_readiness_events;
CREATE TRIGGER agent_worker_readiness_events_validate_insert
    BEFORE INSERT ON agent_worker_readiness_events
    FOR EACH ROW
    EXECUTE FUNCTION validate_agent_worker_readiness_event_insert();

CREATE OR REPLACE FUNCTION record_agent_worker_readiness_event()
RETURNS TRIGGER AS $$
DECLARE
    event_kind_value TEXT;
BEGIN
    IF TG_OP = 'UPDATE'
       AND NEW.status IS NOT DISTINCT FROM OLD.status
       AND NEW.deleted_at IS NOT DISTINCT FROM OLD.deleted_at THEN
        RETURN NEW;
    END IF;

    event_kind_value := CASE
        WHEN NEW.deleted_at IS NOT NULL THEN 'retired'
        WHEN TG_OP = 'INSERT' THEN 'registered'
        ELSE NEW.status
    END;

    INSERT INTO agent_worker_readiness_events (
        worker_instance_id, worker_key, worker_version, event_kind, status,
        status_reason_code, artifact_key_coverage_sha256,
        provider_key_coverage_sha256, provider_route_coverage_sha256,
        startup_coverage_completed_at, heartbeat_at, heartbeat_expires_at,
        transitioned_at
    ) VALUES (
        NEW.id, NEW.worker_key, NEW.version, event_kind_value, NEW.status,
        NEW.status_reason_code, NEW.artifact_key_coverage_sha256,
        NEW.provider_key_coverage_sha256, NEW.provider_route_coverage_sha256,
        NEW.startup_coverage_completed_at, NEW.heartbeat_at,
        NEW.heartbeat_expires_at, NEW.status_changed_at
    );

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_worker_instances_record_readiness_event
    ON agent_worker_instances;
CREATE TRIGGER agent_worker_instances_record_readiness_event
    AFTER INSERT OR UPDATE ON agent_worker_instances
    FOR EACH ROW
    EXECUTE FUNCTION record_agent_worker_readiness_event();

CREATE OR REPLACE FUNCTION reject_agent_worker_readiness_event_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION '% is append-only', TG_TABLE_NAME;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS agent_worker_readiness_events_reject_mutation
    ON agent_worker_readiness_events;
CREATE TRIGGER agent_worker_readiness_events_reject_mutation
    BEFORE UPDATE OR DELETE ON agent_worker_readiness_events
    FOR EACH ROW
    EXECUTE FUNCTION reject_agent_worker_readiness_event_mutation();

DROP TRIGGER IF EXISTS agent_worker_instances_reject_truncate
    ON agent_worker_instances;
CREATE TRIGGER agent_worker_instances_reject_truncate
    BEFORE TRUNCATE ON agent_worker_instances
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_agent_worker_readiness_event_mutation();

DROP TRIGGER IF EXISTS agent_worker_readiness_events_reject_truncate
    ON agent_worker_readiness_events;
CREATE TRIGGER agent_worker_readiness_events_reject_truncate
    BEFORE TRUNCATE ON agent_worker_readiness_events
    FOR EACH STATEMENT
    EXECUTE FUNCTION reject_agent_worker_readiness_event_mutation();

CREATE OR REPLACE FUNCTION agent_has_ready_worker()
RETURNS BOOLEAN AS $$
    SELECT EXISTS (
        SELECT 1
        FROM agent_worker_instances
        WHERE status = 'ready'
          AND deleted_at IS NULL
          AND heartbeat_expires_at > STATEMENT_TIMESTAMP()
          AND startup_coverage_completed_at IS NOT NULL
          AND artifact_key_coverage_sha256 IS NOT NULL
          AND provider_key_coverage_sha256 IS NOT NULL
          AND provider_route_coverage_sha256 IS NOT NULL
    );
$$ LANGUAGE sql STABLE;

CREATE OR REPLACE FUNCTION expire_agent_worker_instances()
RETURNS BIGINT AS $$
DECLARE
    affected_count BIGINT;
BEGIN
    WITH expired AS (
        UPDATE agent_worker_instances
        SET status = 'unavailable',
            status_reason_code = 'heartbeat_expired',
            status_changed_at = heartbeat_expires_at,
            version = version + 1,
            updated_at = STATEMENT_TIMESTAMP()
        WHERE deleted_at IS NULL
          AND status IN ('starting', 'ready', 'draining')
          AND heartbeat_expires_at <= STATEMENT_TIMESTAMP()
        RETURNING 1
    )
    SELECT COUNT(*) INTO affected_count FROM expired;

    RETURN affected_count;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION retire_agent_worker_instances()
RETURNS BIGINT AS $$
DECLARE
    affected_count BIGINT;
BEGIN
    WITH retired AS (
        UPDATE agent_worker_instances
        SET deleted_at = STATEMENT_TIMESTAMP(),
            version = version + 1,
            updated_at = STATEMENT_TIMESTAMP()
        WHERE deleted_at IS NULL
          AND status = 'unavailable'
          AND status_changed_at <= STATEMENT_TIMESTAMP() - INTERVAL '7 days'
        RETURNING 1
    )
    SELECT COUNT(*) INTO affected_count FROM retired;

    RETURN affected_count;
END;
$$ LANGUAGE plpgsql;
