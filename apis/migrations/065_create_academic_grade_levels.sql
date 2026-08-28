-- Canonical tenant-wide grade references used by classes and downstream modules.
--
-- Existing class grade labels are preserved by creating one reference per
-- case-insensitive tenant label before the legacy text column is retired.

CREATE TABLE IF NOT EXISTS academic_grade_levels (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    code TEXT NOT NULL CHECK (BTRIM(code) <> ''),
    name TEXT NOT NULL CHECK (BTRIM(name) <> ''),
    sequence_number SMALLINT NOT NULL
        CHECK (sequence_number BETWEEN 0 AND 999),
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'inactive')),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_academic_grade_levels_tenant_code
    ON academic_grade_levels(tenant_id, LOWER(code))
    WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_academic_grade_levels_tenant_name
    ON academic_grade_levels(tenant_id, LOWER(name))
    WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_academic_grade_levels_tenant_sequence
    ON academic_grade_levels(tenant_id, sequence_number)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS update_academic_grade_levels_updated_at ON academic_grade_levels;
CREATE TRIGGER update_academic_grade_levels_updated_at
    BEFORE UPDATE ON academic_grade_levels
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

WITH legacy_grades AS (
    SELECT tenant_id, LOWER(BTRIM(grade_level)) AS normalized_name,
           MIN(BTRIM(grade_level)) AS name
    FROM class_groups
    WHERE deleted_at IS NULL
      AND grade_level IS NOT NULL
      AND BTRIM(grade_level) <> ''
    GROUP BY tenant_id, LOWER(BTRIM(grade_level))
), numbered AS (
    SELECT tenant_id, normalized_name, name,
           ROW_NUMBER() OVER (PARTITION BY tenant_id ORDER BY normalized_name)::SMALLINT
               AS sequence_number
    FROM legacy_grades
)
INSERT INTO academic_grade_levels (tenant_id, code, name, sequence_number)
SELECT tenant_id,
       'LEGACY-' || LPAD(sequence_number::TEXT, 3, '0'),
       name,
       sequence_number
FROM numbered
ON CONFLICT DO NOTHING;

ALTER TABLE class_groups
    ADD COLUMN IF NOT EXISTS grade_level_id UUID;

UPDATE class_groups AS class_group
SET grade_level_id = grade.id
FROM academic_grade_levels AS grade
WHERE grade.tenant_id = class_group.tenant_id
  AND grade.deleted_at IS NULL
  AND LOWER(grade.name) = LOWER(BTRIM(class_group.grade_level))
  AND class_group.grade_level_id IS NULL
  AND class_group.grade_level IS NOT NULL
  AND BTRIM(class_group.grade_level) <> '';

ALTER TABLE class_groups
    DROP CONSTRAINT IF EXISTS class_groups_grade_level_fk;
ALTER TABLE class_groups
    ADD CONSTRAINT class_groups_grade_level_fk
    FOREIGN KEY (grade_level_id, tenant_id)
    REFERENCES academic_grade_levels(id, tenant_id);

CREATE INDEX IF NOT EXISTS idx_class_groups_tenant_grade_level
    ON class_groups(tenant_id, grade_level_id)
    WHERE deleted_at IS NULL;

ALTER TABLE class_groups
    DROP COLUMN IF EXISTS grade_level;

-- Admissions selects the requested grade. Class placement belongs to enrolment.
ALTER TABLE applications
    ADD COLUMN IF NOT EXISTS target_grade_level_id UUID;

UPDATE applications AS application
SET target_grade_level_id = class_group.grade_level_id
FROM class_groups AS class_group
WHERE class_group.id = application.target_class_group_id
  AND class_group.tenant_id = application.tenant_id
  AND application.target_grade_level_id IS NULL;

ALTER TABLE applications
    DROP COLUMN IF EXISTS target_class_group_id;

ALTER TABLE applications
    DROP CONSTRAINT IF EXISTS applications_target_grade_level_fk;
ALTER TABLE applications
    ADD CONSTRAINT applications_target_grade_level_fk
    FOREIGN KEY (target_grade_level_id, tenant_id)
    REFERENCES academic_grade_levels(id, tenant_id);

CREATE INDEX IF NOT EXISTS idx_applications_tenant_target_grade
    ON applications(tenant_id, target_grade_level_id)
    WHERE deleted_at IS NULL;

DROP TRIGGER IF EXISTS ev_academic_grade_levels ON academic_grade_levels;
CREATE TRIGGER ev_academic_grade_levels
    AFTER INSERT OR UPDATE OR DELETE ON academic_grade_levels
    FOR EACH ROW
    EXECUTE FUNCTION log_event();
