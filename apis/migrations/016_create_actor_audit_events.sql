-- Actor-aware audit evidence for consequential human, Agent, and system work.
-- The legacy event_log remains table-change evidence and is not replaced here.

CREATE TABLE IF NOT EXISTS actor_audit_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    actor_type TEXT NOT NULL CHECK (actor_type IN ('person', 'agent', 'system')),
    actor_user_id UUID,
    action_key TEXT NOT NULL,
    target_type TEXT,
    target_id TEXT,
    outcome TEXT NOT NULL CHECK (outcome IN ('succeeded', 'failed', 'denied', 'cancelled')),
    request_id UUID NOT NULL,
    correlation_id UUID NOT NULL,
    agent_run_id UUID,
    approval_id UUID,
    reason TEXT,
    redacted_metadata JSONB NOT NULL DEFAULT '{}'::JSONB
        CHECK (JSONB_TYPEOF(redacted_metadata) = 'object'),
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT actor_audit_events_actor_identity CHECK (
        (actor_type = 'system' AND actor_user_id IS NULL)
        OR (actor_type IN ('person', 'agent') AND actor_user_id IS NOT NULL)
    ),
    CONSTRAINT actor_audit_events_target_identity CHECK (
        (target_type IS NULL AND target_id IS NULL)
        OR (target_type IS NOT NULL AND target_id IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_actor_audit_events_tenant_time
ON actor_audit_events (tenant_id, occurred_at DESC);

CREATE INDEX IF NOT EXISTS idx_actor_audit_events_correlation
ON actor_audit_events (tenant_id, correlation_id, occurred_at);

CREATE INDEX IF NOT EXISTS idx_actor_audit_events_actor
ON actor_audit_events (tenant_id, actor_user_id, occurred_at DESC)
WHERE actor_user_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_actor_audit_events_action
ON actor_audit_events (tenant_id, action_key, occurred_at DESC);

CREATE INDEX IF NOT EXISTS idx_actor_audit_events_agent_run
ON actor_audit_events (tenant_id, agent_run_id, occurred_at)
WHERE agent_run_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_actor_audit_events_approval
ON actor_audit_events (tenant_id, approval_id, occurred_at)
WHERE approval_id IS NOT NULL;

CREATE OR REPLACE FUNCTION prevent_actor_audit_event_update()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'actor audit events are append-only';
END$$;

DROP TRIGGER IF EXISTS immutable_actor_audit_events ON actor_audit_events;
CREATE TRIGGER immutable_actor_audit_events
    BEFORE UPDATE ON actor_audit_events
    FOR EACH ROW
    EXECUTE FUNCTION prevent_actor_audit_event_update();
