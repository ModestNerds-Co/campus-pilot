-- Tenant-owned learner number allocation.
--
-- Ordinary SIS creates consume the sequence transactionally. Staged imports
-- retain supplied legacy numbers and align only values in the managed LRN
-- namespace so a later generated number cannot collide or be reused.

CREATE TABLE IF NOT EXISTS sis_learner_number_sequences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    number_prefix TEXT NOT NULL DEFAULT 'LRN-'
        CHECK (
            CHAR_LENGTH(number_prefix) BETWEEN 1 AND 32
            AND number_prefix = BTRIM(number_prefix)
            AND number_prefix !~ '[[:cntrl:]]'
        ),
    number_padding SMALLINT NOT NULL DEFAULT 6
        CHECK (number_padding BETWEEN 1 AND 8),
    last_number BIGINT NOT NULL DEFAULT 0
        CHECK (last_number BETWEEN 0 AND 99999999),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id)
);

DROP TRIGGER IF EXISTS update_sis_learner_number_sequences_updated_at
    ON sis_learner_number_sequences;
CREATE TRIGGER update_sis_learner_number_sequences_updated_at
    BEFORE UPDATE ON sis_learner_number_sequences
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE OR REPLACE FUNCTION protect_learner_number_identity()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.learner_number IS DISTINCT FROM NEW.learner_number THEN
        RAISE EXCEPTION 'learner numbers are immutable after creation';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS protect_learner_number_identity ON learners;
CREATE TRIGGER protect_learner_number_identity
    BEFORE UPDATE ON learners
    FOR EACH ROW
    EXECUTE FUNCTION protect_learner_number_identity();

-- Preserve the monotonic boundary when this migration follows existing
-- managed learner numbers. Deleted records count because identifiers are not
-- returned to the pool.
INSERT INTO sis_learner_number_sequences (
    tenant_id,
    number_prefix,
    number_padding,
    last_number
)
SELECT tenant_id,
       'LRN-',
       6,
       MAX(SUBSTRING(UPPER(learner_number) FROM '^LRN-([0-9]{6,8})$')::BIGINT)
FROM learners
WHERE UPPER(learner_number) ~ '^LRN-[0-9]{6,8}$'
GROUP BY tenant_id
ON CONFLICT (tenant_id) DO UPDATE
SET last_number = GREATEST(
    sis_learner_number_sequences.last_number,
    EXCLUDED.last_number
);

DROP TRIGGER IF EXISTS ev_sis_learner_number_sequences
    ON sis_learner_number_sequences;
CREATE TRIGGER ev_sis_learner_number_sequences
    AFTER INSERT OR UPDATE OR DELETE ON sis_learner_number_sequences
    FOR EACH ROW EXECUTE FUNCTION log_event();
