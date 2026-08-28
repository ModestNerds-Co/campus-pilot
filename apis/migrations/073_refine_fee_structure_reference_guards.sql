-- Revalidate dependency references when a fee structure is created, edited,
-- or activated. Retirement must remain possible after a referenced academic
-- or Finance record has become inactive because retirement reduces future use.

DROP TRIGGER IF EXISTS fees_fee_structure_reference_guard ON fees_fee_structures;
DROP TRIGGER IF EXISTS fees_fee_structure_activation_guard ON fees_fee_structures;

CREATE TRIGGER fees_fee_structure_reference_guard
    BEFORE INSERT OR UPDATE OF academic_year_id, academic_term_id, grade_level_id,
        currency_id, receivable_account_id, revenue_account_id
    ON fees_fee_structures
    FOR EACH ROW
    EXECUTE FUNCTION validate_fees_fee_structure_references();

CREATE TRIGGER fees_fee_structure_activation_guard
    BEFORE UPDATE OF status ON fees_fee_structures
    FOR EACH ROW
    WHEN (NEW.status = 'active')
    EXECUTE FUNCTION validate_fees_fee_structure_references();
