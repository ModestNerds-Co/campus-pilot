-- Term-scoped assessment structures owned by Academics.
--
-- Assessment components point at canonical teaching assignments. Class,
-- subject, teacher, and academic-year labels are always resolved from their
-- owning records rather than copied here. Learner marks are intentionally a
-- later slice because SIS remains the source of enrolment eligibility.

CREATE TABLE IF NOT EXISTS assessment_cycles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    academic_term_id UUID NOT NULL,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'open', 'closed')),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (academic_term_id, tenant_id)
        REFERENCES academic_terms(id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_assessment_cycles_term_code
    ON assessment_cycles(tenant_id, academic_term_id, LOWER(code))
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_assessment_cycles_tenant_term_status
    ON assessment_cycles(tenant_id, academic_term_id, status)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS assessment_components (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    assessment_cycle_id UUID NOT NULL,
    teaching_assignment_id UUID NOT NULL,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    assessment_kind TEXT NOT NULL
        CHECK (assessment_kind IN (
            'assignment', 'quiz', 'test', 'project', 'exam', 'practical', 'other'
        )),
    maximum_marks INTEGER NOT NULL CHECK (maximum_marks BETWEEN 1 AND 100000),
    weight_basis_points SMALLINT NOT NULL CHECK (weight_basis_points BETWEEN 1 AND 10000),
    occurs_on DATE,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'inactive')),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (id, tenant_id),
    FOREIGN KEY (assessment_cycle_id, tenant_id)
        REFERENCES assessment_cycles(id, tenant_id),
    FOREIGN KEY (teaching_assignment_id, tenant_id)
        REFERENCES teaching_assignments(id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_assessment_components_cycle_assignment_code
    ON assessment_components(
        tenant_id,
        assessment_cycle_id,
        teaching_assignment_id,
        LOWER(code)
    ) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_assessment_components_cycle_assignment
    ON assessment_components(tenant_id, assessment_cycle_id, teaching_assignment_id)
    WHERE deleted_at IS NULL;

CREATE OR REPLACE FUNCTION validate_assessment_cycle_change()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE
    incomplete_assignment UUID;
    term_status TEXT;
BEGIN
    IF TG_OP = 'INSERT'
       OR NEW.academic_term_id IS DISTINCT FROM OLD.academic_term_id
       OR (TG_OP = 'UPDATE' AND OLD.status = 'draft' AND NEW.status = 'open') THEN
        SELECT status INTO term_status
        FROM academic_terms
        WHERE id = NEW.academic_term_id
          AND tenant_id = NEW.tenant_id
          AND deleted_at IS NULL;

        IF term_status IS NULL THEN
            RAISE EXCEPTION 'Academic term was not found for this campus';
        END IF;

        IF term_status = 'closed' THEN
            RAISE EXCEPTION 'A closed academic term cannot accept assessment changes';
        END IF;
    END IF;

    IF TG_OP = 'UPDATE' THEN
        IF OLD.status = 'closed' AND NEW.status <> 'closed' THEN
            RAISE EXCEPTION 'A closed assessment cycle cannot be reopened';
        END IF;

        IF OLD.status = 'open' AND NEW.status = 'draft' THEN
            RAISE EXCEPTION 'An open assessment cycle cannot return to draft';
        END IF;

        IF OLD.status = 'draft' AND NEW.status = 'closed' THEN
            RAISE EXCEPTION 'Assessment cycles move forward from draft to open to closed';
        END IF;

        IF OLD.status <> 'draft' AND (
            NEW.academic_term_id IS DISTINCT FROM OLD.academic_term_id
            OR NEW.code IS DISTINCT FROM OLD.code
            OR NEW.name IS DISTINCT FROM OLD.name
        ) THEN
            RAISE EXCEPTION 'Only a draft assessment cycle can change its details';
        END IF;

        IF NEW.academic_term_id IS DISTINCT FROM OLD.academic_term_id AND EXISTS (
            SELECT 1 FROM assessment_components
            WHERE tenant_id = OLD.tenant_id
              AND assessment_cycle_id = OLD.id
              AND deleted_at IS NULL
        ) THEN
            RAISE EXCEPTION 'Remove assessment components before changing the academic term';
        END IF;

        IF OLD.status = 'draft' AND NEW.status IN ('open', 'closed') THEN
            IF NOT EXISTS (
                SELECT 1 FROM assessment_components
                WHERE tenant_id = NEW.tenant_id
                  AND assessment_cycle_id = NEW.id
                  AND status = 'active'
                  AND deleted_at IS NULL
            ) THEN
                RAISE EXCEPTION 'Add at least one active assessment component before opening the cycle';
            END IF;

            SELECT teaching_assignment_id
            INTO incomplete_assignment
            FROM assessment_components
            WHERE tenant_id = NEW.tenant_id
              AND assessment_cycle_id = NEW.id
              AND status = 'active'
              AND deleted_at IS NULL
            GROUP BY teaching_assignment_id
            HAVING SUM(weight_basis_points) <> 10000
            LIMIT 1;

            IF incomplete_assignment IS NOT NULL THEN
                RAISE EXCEPTION 'Active assessment component weights must total 100%% for every teaching assignment';
            END IF;
        END IF;
    END IF;

    RETURN NEW;
END$$;

CREATE OR REPLACE FUNCTION validate_assessment_component_change()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE
    cycle_record RECORD;
    assignment_year_id UUID;
    active_weight INTEGER;
BEGIN
    SELECT cycle.status, term.academic_year_id, term.starts_on, term.ends_on
    INTO cycle_record
    FROM assessment_cycles AS cycle
    INNER JOIN academic_terms AS term
      ON term.id = cycle.academic_term_id
     AND term.tenant_id = cycle.tenant_id
     AND term.deleted_at IS NULL
    WHERE cycle.id = COALESCE(NEW.assessment_cycle_id, OLD.assessment_cycle_id)
      AND cycle.tenant_id = COALESCE(NEW.tenant_id, OLD.tenant_id)
      AND cycle.deleted_at IS NULL;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Assessment cycle was not found for this campus';
    END IF;

    IF cycle_record.status <> 'draft' THEN
        RAISE EXCEPTION 'Assessment components can only change while the cycle is draft';
    END IF;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;

    IF NEW.deleted_at IS NOT NULL THEN
        RETURN NEW;
    END IF;

    SELECT academic_year_id INTO assignment_year_id
    FROM teaching_assignments
    WHERE id = NEW.teaching_assignment_id
      AND tenant_id = NEW.tenant_id
      AND deleted_at IS NULL;

    IF assignment_year_id IS NULL THEN
        RAISE EXCEPTION 'Teaching assignment was not found for this campus';
    END IF;

    IF assignment_year_id <> cycle_record.academic_year_id THEN
        RAISE EXCEPTION 'Teaching assignment must belong to the assessment term academic year';
    END IF;

    IF NEW.occurs_on IS NOT NULL AND (
        NEW.occurs_on < cycle_record.starts_on OR NEW.occurs_on > cycle_record.ends_on
    ) THEN
        RAISE EXCEPTION 'Assessment date must fall within the academic term';
    END IF;

    IF NEW.status = 'active' AND EXISTS (
        SELECT 1 FROM teaching_assignments
        WHERE id = NEW.teaching_assignment_id
          AND tenant_id = NEW.tenant_id
          AND status <> 'active'
          AND deleted_at IS NULL
    ) THEN
        RAISE EXCEPTION 'An active assessment component requires an active teaching assignment';
    END IF;

    SELECT COALESCE(SUM(weight_basis_points), 0)
    INTO active_weight
    FROM assessment_components
    WHERE tenant_id = NEW.tenant_id
      AND assessment_cycle_id = NEW.assessment_cycle_id
      AND teaching_assignment_id = NEW.teaching_assignment_id
      AND status = 'active'
      AND deleted_at IS NULL
      AND id <> NEW.id;

    IF NEW.status = 'active' THEN
        active_weight := active_weight + NEW.weight_basis_points;
    END IF;

    IF active_weight > 10000 THEN
        RAISE EXCEPTION 'Active assessment component weights cannot exceed 100%% for a teaching assignment';
    END IF;

    RETURN NEW;
END$$;

CREATE TRIGGER validate_assessment_cycles
    BEFORE INSERT OR UPDATE ON assessment_cycles
    FOR EACH ROW
    EXECUTE FUNCTION validate_assessment_cycle_change();

CREATE TRIGGER validate_assessment_components
    BEFORE INSERT OR UPDATE OR DELETE ON assessment_components
    FOR EACH ROW
    EXECUTE FUNCTION validate_assessment_component_change();

CREATE TRIGGER update_assessment_cycles_updated_at
    BEFORE UPDATE ON assessment_cycles
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE TRIGGER update_assessment_components_updated_at
    BEFORE UPDATE ON assessment_components
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp();

CREATE TRIGGER ev_assessment_cycles
    AFTER INSERT OR UPDATE OR DELETE ON assessment_cycles
    FOR EACH ROW
    EXECUTE FUNCTION log_event();

CREATE TRIGGER ev_assessment_components
    AFTER INSERT OR UPDATE OR DELETE ON assessment_components
    FOR EACH ROW
    EXECUTE FUNCTION log_event();
