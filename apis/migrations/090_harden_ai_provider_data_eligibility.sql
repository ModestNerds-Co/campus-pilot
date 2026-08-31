-- Forward-only hardening for provider data approvals and preflight failures.
-- Migration 087 is already deployed and immutable; repeat the legacy repair
-- here so installations that missed a default row converge without granting
-- any provider data access.

DROP TRIGGER IF EXISTS ai_provider_data_approvals_validate_insert
    ON ai_provider_data_approval_versions;

INSERT INTO ai_provider_data_approval_versions (
    id, tenant_id, connection_id, approval_version, approval_class,
    change_source, changed_by, change_reason
)
SELECT
    GEN_RANDOM_UUID(), connection.tenant_id, connection.id, 1, 'unapproved',
    'system_default', NULL, 'Initial unapproved provider data eligibility.'
FROM ai_provider_connections AS connection
WHERE NOT EXISTS (
    SELECT 1
    FROM ai_provider_data_approval_versions AS approval
    WHERE approval.tenant_id = connection.tenant_id
      AND approval.connection_id = connection.id
);

CREATE TRIGGER ai_provider_data_approvals_validate_insert
    BEFORE INSERT ON ai_provider_data_approval_versions
    FOR EACH ROW
    EXECUTE FUNCTION validate_ai_provider_data_approval_insert();

-- Migration 083 made active route targets immutable before 087 introduced the
-- approval pin. A legacy pre-087 database with application rows therefore
-- needs the lifecycle guard suspended only for this deterministic null-pin
-- repair. Existing non-null pins are deliberately never advanced: a later
-- approval version must leave the old route stale.
DROP TRIGGER IF EXISTS ai_task_routes_protect_lifecycle
    ON ai_task_routes;

UPDATE ai_task_routes AS route
SET provider_data_approval_id = (
    SELECT approval.id
    FROM ai_provider_data_approval_versions AS approval
    WHERE approval.tenant_id = route.tenant_id
      AND approval.connection_id = route.connection_id
    ORDER BY approval.approval_version DESC
    LIMIT 1
)
WHERE route.provider_data_approval_id IS NULL;

ALTER TABLE ai_task_routes
    ALTER COLUMN provider_data_approval_id SET NOT NULL;

CREATE TRIGGER ai_task_routes_protect_lifecycle
    BEFORE UPDATE OR DELETE ON ai_task_routes
    FOR EACH ROW
    EXECUTE FUNCTION protect_ai_task_route_lifecycle();

-- These failures occur before any provider request. They are separate from an
-- upstream invalid response: one means the model catalog lacks a usable output
-- limit, the other means the requested limit exceeds that catalog snapshot.
ALTER TABLE agent_provider_attempts
    DROP CONSTRAINT IF EXISTS agent_provider_attempts_failure_category_check,
    DROP CONSTRAINT IF EXISTS agent_provider_attempts_failure_shape_check;

ALTER TABLE agent_provider_attempts
    ADD CONSTRAINT agent_provider_attempts_failure_category_check CHECK (
        failure_category IS NULL OR failure_category IN (
            'connection_unavailable', 'stale_credential', 'stale_model',
            'tools_unsupported', 'model_context_unavailable',
            'model_output_unavailable', 'context_window_exceeded',
            'output_budget_exceeded', 'credential_unavailable',
            'invalid_configuration', 'invalid_input', 'storage_error',
            'provider_data_not_approved', 'provider_data_approval_changed',
            'local_execution_required',
            'authentication', 'rate_limited', 'unavailable', 'timeout',
            'network', 'invalid_response', 'unsupported'
        )
    ),
    ADD CONSTRAINT agent_provider_attempts_failure_shape_check CHECK (
        (
            status = 'failed'
            AND (
                (
                    failure_origin = 'preflight'
                    AND failure_category IN (
                        'connection_unavailable', 'stale_credential', 'stale_model',
                        'tools_unsupported', 'model_context_unavailable',
                        'model_output_unavailable', 'context_window_exceeded',
                        'output_budget_exceeded', 'credential_unavailable',
                        'invalid_configuration', 'invalid_input', 'storage_error',
                        'provider_data_not_approved', 'provider_data_approval_changed',
                        'local_execution_required'
                    )
                )
                OR (
                    failure_origin = 'upstream'
                    AND failure_category IN (
                        'authentication', 'rate_limited', 'unavailable', 'timeout',
                        'network', 'invalid_response', 'unsupported'
                    )
                )
            )
        )
        OR (
            status <> 'failed'
            AND failure_origin IS NULL
            AND failure_category IS NULL
        )
    );
