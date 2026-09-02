//! Authorizes the fail-closed initial Agent record-scope release.
//!
//! Only reads whose owning query already applies the authenticated tenant are
//! discoverable. Resource-shaped reads additionally prove every exact target
//! belongs to that tenant. Self and assigned families remain unavailable until
//! their domain queries apply visibility before pagination and projection.

use std::{collections::BTreeSet, sync::Arc};

use async_trait::async_trait;
use cp_agent::{
    AuthenticatedAgentPrincipal, AuthorizedRecordScope, CapabilityResource, CapabilityScope,
    CurrentAuthority, RecordScopeAuthorizer, RecordScopeDenied,
};
use cp_common::{ProductOperation, RuntimeAccessChecks};
use sqlx::PgPool;
use uuid::Uuid;

/// Operations offered by the initial worker after record-scope policy filtering.
///
/// This list is a discovery boundary, not an access grant. The broker still
/// checks current authentication, licensing, operation permission, this scope
/// authorizer, and its durable execution proof for every call.
pub const INITIAL_WORKER_OPERATION_KEYS: &[&str] = &[
    "administration.catalog.read",
    "administration.modules.list",
    "administration.licensing.read",
    "administration.school_settings.read",
    "administration.ai_providers.catalog.list",
    "administration.ai_providers.connections.list",
    "administration.ai_providers.connections.read",
    "administration.ai_providers.models.list",
    "administration.ai_routing.routes.list",
    "administration.ai_routing.routes.options",
    "administration.ai_routing.routes.read",
    "administration.ai_routing.routes.resolve",
    "administration.roles.list",
    "administration.roles.read",
    "administration.users.list",
    "administration.users.read",
    "sis.learner_numbering.read",
    "academics.academic_years.list",
    "academics.academic_years.read",
    "academics.terms.list",
    "academics.terms.read",
    "academics.grade_levels.list",
    "academics.grade_levels.read",
    "academics.subjects.list",
    "academics.subjects.read",
    "academics.classes.list",
    "academics.classes.read",
    "academics.assessment_cycles.list",
    "academics.assessment_cycles.read",
    "finance.currencies.list",
    "finance.currencies.read",
    "finance.accounts.list",
    "finance.accounts.read",
    "finance.fiscal_years.list",
    "finance.fiscal_years.read",
    "finance.accounting_periods.list",
    "finance.journals.list",
    "finance.journals.read",
    "finance.journals.validation.read",
    "finance.posting_requests.list",
    "finance.posting_requests.read",
    "fees.reference_data.read",
    "fees.fee_structures.list",
    "fees.fee_structures.read",
    "procurement.reference_data.read",
    "procurement.suppliers.list",
    "procurement.suppliers.read",
    "assets_inventory.items.list",
    "assets_inventory.items.read",
    "assets_inventory.stores.list",
    "assets_inventory.stores.read",
    "assets_inventory.stock_balances.list",
    "assets_inventory.stock_movements.list",
    "assets_inventory.stock_movements.read",
    "assets_inventory.goods_receipt_allocations.list",
    "attendance.references.read",
    "attendance.learners.history.read",
    "attendance.registers.list",
    "attendance.registers.read",
    "attendance.lesson_sessions.list",
    "attendance.lesson_sessions.read",
    "attendance.exceptions.list",
    "attendance.exceptions.read",
    "learning.settings.read",
    "learning.references.read",
    "learning.resource_files.list",
    "learning.spaces.list",
    "learning.spaces.read",
    "learning.assignments.list",
    "learning.assignments.read",
    "learning.submissions.mine.read",
    "learning.submissions.list",
    "learning.submissions.read",
    "learning.progress.mine.read",
    "learning.progress.list",
    "learning.quizzes.list",
    "learning.quizzes.read",
    "learning.quiz_attempts.list",
    "learning.quiz_attempts.read",
    "learning.completion_policy.read",
    "learning.completion.mine.read",
    "learning.completion.list",
    "student_support.actions.list",
    "student_support.cases.list",
    "student_support.cases.read",
    "transport.routes.list",
    "transport.routes.read",
    "transport.riders.list",
    "transport.runs.list",
    "transport.runs.read",
    "messaging.references.read",
    "messaging.announcements.list",
    "messaging.announcements.read",
    "messaging.announcements.audience_preview.read",
    "messaging.deliveries.list",
    "messaging.inbox.list",
    "messaging.inbox.read",
    "hr_payroll.departments.list",
    "hr_payroll.departments.read",
    "hr_payroll.positions.list",
    "hr_payroll.positions.read",
    "fleet.vehicles.list",
    "fleet.vehicles.read",
    "library.settings.read",
    "library.references.read",
    "library.titles.list",
    "library.titles.read",
    "library.copies.list",
    "library.copies.read",
    "library.members.list",
    "library.members.read",
    "library.loans.list",
    "library.loans.read",
    "library.holds.list",
    "library.holds.read",
    "library.fines.list",
    "library.fines.read",
    "health.references.read",
    "health.patients.list",
    "health.patients.read",
    "health.visits.list",
    "health.visits.read",
    "health.medication_plans.list",
    "health.medication_administrations.list",
    "health.follow_ups.list",
    "hostel.references.read",
    "hostel.residences.list",
    "hostel.residences.read",
    "hostel.rooms.list",
    "hostel.rooms.read",
    "hostel.allocations.preview",
    "hostel.allocations.list",
    "hostel.allocations.read",
    "hostel.allocations.transfer_preview",
    "hostel.pastoral_records.list",
    "hostel.pastoral_records.read",
    "document_registry.numbering_policy.read",
    "document_registry.series.list",
    "document_registry.series.read",
    "document_registry.files.list",
    "document_registry.files.read",
    "document_registry.files.activity.list",
    "document_registry.retention_due.list",
    "document_registry.disposition_reviews.list",
    "document_registry.disposition_reviews.read",
    "document_registry.legal_holds.list",
    "document_registry.legal_holds.read",
    "internal_audit.numbering_policy.read",
    "internal_audit.plans.list",
    "internal_audit.plans.read",
    "internal_audit.auditor_candidates.list",
    "internal_audit.engagements.list",
    "internal_audit.engagements.read",
    "internal_audit.evidence.list",
    "internal_audit.findings.list",
    "internal_audit.findings.read",
    "facilities.locations.list",
    "facilities.locations.read",
    "facilities.requests.list",
    "facilities.requests.read",
    "facilities.work_orders.list",
    "facilities.work_orders.read",
    "activities.catalog.list",
    "activities.catalog.read",
    "activities.groups.list",
    "activities.groups.read",
    "activities.sessions.list",
    "activities.sessions.read",
];

/// Directly exposed operations deliberately withheld from initial discovery.
///
/// These operations are still represented in the full diagnostic registry.
/// They must remain unavailable until their family has current role evidence
/// and the owning query applies self/assigned visibility when required.
#[cfg(test)]
pub const WITHHELD_RECORD_SCOPED_OPERATION_KEYS: &[&str] = &[
    "sis.account_candidates.list",
    "sis.imports.list",
    "sis.imports.read",
    "sis.imports.preview.read",
    "sis.learners.list",
    "sis.learners.read",
    "sis.guardians.list",
    "sis.guardians.read",
    "sis.guardian_relationships.list",
    "sis.guardian_relationships.read",
    "sis.applications.list",
    "sis.applications.read",
    "sis.enrolments.list",
    "sis.enrolments.read",
    "academics.teacher_candidates.list",
    "academics.teachers.list",
    "academics.teachers.read",
    "academics.teaching_assignments.list",
    "academics.teaching_assignments.read",
    "academics.assessment_components.list",
    "academics.assessment_components.read",
    "academics.gradebook.references.read",
    "academics.gradebook.mark_sheets.list",
    "academics.gradebook.mark_sheets.read",
    "academics.reporting.references.read",
    "academics.reporting.grading_schemes.list",
    "academics.reporting.grading_schemes.read",
    "academics.reporting.report_batches.list",
    "academics.reporting.report_batches.read",
    "academics.reporting.transcripts.read",
    "fees.learner_candidates.list",
    "fees.imports.list",
    "fees.imports.read",
    "fees.imports.preview.read",
    "fees.billing_accounts.list",
    "fees.billing_accounts.read",
    "fees.invoices.list",
    "fees.invoices.read",
    "procurement.requester_candidates.list",
    "procurement.requisitions.list",
    "procurement.requisitions.read",
    "procurement.purchase_orders.list",
    "procurement.purchase_orders.read",
    "procurement.goods_receipts.list",
    "procurement.goods_receipts.read",
    "assets_inventory.requester_candidates.list",
    "assets_inventory.department_candidates.list",
    "assets_inventory.stock_requests.list",
    "assets_inventory.stock_requests.read",
    "assets_inventory.stock_requests.fulfilment_preview.read",
    "hr_payroll.imports.list",
    "hr_payroll.imports.read",
    "hr_payroll.imports.preview.read",
    "hr_payroll.employees.list",
    "hr_payroll.employees.read",
    "hr_payroll.employment_engagements.list",
    "hr_payroll.employment_engagements.read",
    "hr_payroll.availability.list",
    "hr_payroll.availability.read",
    "fleet.driver_candidates.list",
    "fleet.drivers.list",
    "fleet.drivers.read",
    "fleet.vehicle_logs.list",
    "fleet.vehicle_logs.read",
    "timetabling.configuration.read",
    "timetabling.runs.list",
    "timetabling.runs.read",
    "timetabling.runs.read_latest",
];

/// Returns whether the initial worker may advertise an operation.
#[must_use]
pub fn is_initial_worker_operation(operation_key: &str) -> bool {
    INITIAL_WORKER_OPERATION_KEYS.contains(&operation_key)
}

/// Production record-scope adapter for the Agent broker.
#[derive(Clone)]
pub struct AppRecordScopeAuthorizer {
    resource_probe: Arc<dyn ResourceTenantProbe>,
}

impl AppRecordScopeAuthorizer {
    /// Creates the production adapter over tenant-scoped PostgreSQL probes.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self {
            resource_probe: Arc::new(PostgresResourceTenantProbe { pool }),
        }
    }

    #[cfg(test)]
    fn with_probe(resource_probe: Arc<dyn ResourceTenantProbe>) -> Self {
        Self { resource_probe }
    }

    async fn authorize_operation_scope(
        &self,
        principal: AuthenticatedAgentPrincipal,
        operation_key: &str,
        requested_scope: &CapabilityScope,
    ) -> Result<(), RecordScopeDenied> {
        if principal.tenant_id().is_nil() || principal.user_id().is_nil() {
            return Err(RecordScopeDenied);
        }
        let policy = operation_scope_policy(operation_key).ok_or(RecordScopeDenied)?;
        let resources = policy.parse_requested_scope(requested_scope)?;
        for resource in resources {
            let belongs_to_tenant = self
                .resource_probe
                .belongs_to_tenant(principal.tenant_id(), resource.kind, resource.id)
                .await
                .map_err(|_| RecordScopeDenied)?;
            if !belongs_to_tenant {
                return Err(RecordScopeDenied);
            }
        }
        Ok(())
    }
}

#[async_trait]
impl RecordScopeAuthorizer for AppRecordScopeAuthorizer {
    async fn authorize(
        &self,
        principal: AuthenticatedAgentPrincipal,
        authority: &CurrentAuthority,
        operation: &ProductOperation,
        scope: &CapabilityScope,
    ) -> Result<AuthorizedRecordScope, RecordScopeDenied> {
        let decision = authority
            .access()
            .evaluate_operation(operation, RuntimeAccessChecks::default());
        if !decision.allowed {
            return Err(RecordScopeDenied);
        }
        self.authorize_operation_scope(principal, operation.key(), scope)
            .await?;
        Ok(AuthorizedRecordScope::granted())
    }
}

#[derive(Debug, Clone, Copy)]
enum OperationScopePolicy {
    Dataset,
    OneResource(&'static str),
    DatasetOrResources {
        allowed_kinds: &'static [&'static str],
        maximum: usize,
    },
}

impl OperationScopePolicy {
    fn parse_requested_scope<'a>(
        self,
        requested_scope: &'a CapabilityScope,
    ) -> Result<Vec<ParsedResource<'a>>, RecordScopeDenied> {
        match (self, requested_scope) {
            (Self::Dataset, CapabilityScope::TenantWide) => Ok(Vec::new()),
            (Self::OneResource(expected_kind), CapabilityScope::Resources(resources)) => {
                let [resource] = resources.values() else {
                    return Err(RecordScopeDenied);
                };
                parse_resources([resource], &[expected_kind], 1)
            }
            (Self::DatasetOrResources { .. }, CapabilityScope::TenantWide) => Ok(Vec::new()),
            (
                Self::DatasetOrResources {
                    allowed_kinds,
                    maximum,
                },
                CapabilityScope::Resources(resources),
            ) => parse_resources(resources.values().iter(), allowed_kinds, maximum),
            (Self::Dataset, CapabilityScope::Resources(_))
            | (Self::OneResource(_), CapabilityScope::TenantWide) => Err(RecordScopeDenied),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ParsedResource<'a> {
    kind: &'a str,
    id: Uuid,
}

fn parse_resources<'a>(
    resources: impl IntoIterator<Item = &'a CapabilityResource>,
    allowed_kinds: &[&str],
    maximum: usize,
) -> Result<Vec<ParsedResource<'a>>, RecordScopeDenied> {
    let resources = resources.into_iter().collect::<Vec<_>>();
    if resources.is_empty() || resources.len() > maximum {
        return Err(RecordScopeDenied);
    }

    let mut seen_kinds = BTreeSet::new();
    let mut seen_resources = BTreeSet::new();
    let mut parsed = Vec::with_capacity(resources.len());
    for resource in resources {
        if !allowed_kinds.contains(&resource.kind()) || !seen_kinds.insert(resource.kind()) {
            return Err(RecordScopeDenied);
        }
        let id = Uuid::parse_str(resource.id()).map_err(|_| RecordScopeDenied)?;
        if id.is_nil() || !seen_resources.insert((resource.kind(), id)) {
            return Err(RecordScopeDenied);
        }
        parsed.push(ParsedResource {
            kind: resource.kind(),
            id,
        });
    }
    Ok(parsed)
}

fn operation_scope_policy(operation_key: &str) -> Option<OperationScopePolicy> {
    const ITEM_AND_STORE: &[&str] = &["assets_inventory_item", "assets_inventory_store"];
    match operation_key {
        "administration.catalog.read"
        | "administration.modules.list"
        | "administration.licensing.read"
        | "administration.school_settings.read"
        | "administration.ai_providers.catalog.list"
        | "administration.ai_providers.connections.list"
        | "administration.ai_routing.routes.list"
        | "administration.ai_routing.routes.options"
        | "administration.ai_routing.routes.resolve"
        | "administration.roles.list"
        | "administration.users.list"
        | "sis.learner_numbering.read"
        | "academics.academic_years.list"
        | "academics.grade_levels.list"
        | "academics.subjects.list"
        | "academics.gradebook.references.read"
        | "academics.gradebook.mark_sheets.list"
        | "academics.reporting.references.read"
        | "academics.reporting.grading_schemes.list"
        | "academics.reporting.report_batches.list"
        | "finance.currencies.list"
        | "finance.accounts.list"
        | "finance.fiscal_years.list"
        | "finance.journals.list"
        | "finance.posting_requests.list"
        | "fees.reference_data.read"
        | "fees.fee_structures.list"
        | "procurement.reference_data.read"
        | "procurement.suppliers.list"
        | "assets_inventory.items.list"
        | "assets_inventory.stores.list"
        | "assets_inventory.department_candidates.list"
        | "attendance.references.read"
        | "attendance.registers.list"
        | "attendance.lesson_sessions.list"
        | "attendance.exceptions.list"
        | "learning.settings.read"
        | "learning.references.read"
        | "learning.resource_files.list"
        | "learning.spaces.list"
        | "student_support.cases.list"
        | "transport.routes.list"
        | "transport.riders.list"
        | "transport.runs.list"
        | "messaging.references.read"
        | "messaging.announcements.list"
        | "messaging.inbox.list"
        | "hr_payroll.departments.list"
        | "hr_payroll.positions.list"
        | "fleet.vehicles.list"
        | "library.settings.read"
        | "library.references.read"
        | "library.titles.list"
        | "library.members.list"
        | "library.loans.list"
        | "library.holds.list"
        | "library.fines.list"
        | "health.references.read"
        | "hostel.references.read"
        | "hostel.residences.list"
        | "hostel.rooms.list"
        | "hostel.allocations.preview"
        | "hostel.allocations.list"
        | "hostel.pastoral_records.list"
        | "document_registry.numbering_policy.read"
        | "document_registry.series.list"
        | "document_registry.files.list"
        | "document_registry.retention_due.list"
        | "document_registry.disposition_reviews.list"
        | "document_registry.legal_holds.list"
        | "internal_audit.numbering_policy.read"
        | "internal_audit.plans.list"
        | "internal_audit.auditor_candidates.list"
        | "internal_audit.engagements.list"
        | "internal_audit.findings.list"
        | "facilities.locations.list"
        | "facilities.requests.list"
        | "facilities.work_orders.list"
        | "activities.catalog.list"
        | "activities.groups.list"
        | "activities.sessions.list" => Some(OperationScopePolicy::Dataset),
        "administration.ai_providers.connections.read"
        | "administration.ai_providers.models.list" => {
            Some(OperationScopePolicy::OneResource("ai_provider_connection"))
        }
        "administration.ai_routing.routes.read" => {
            Some(OperationScopePolicy::OneResource("ai_route_set"))
        }
        "administration.roles.read" => Some(OperationScopePolicy::OneResource("role")),
        "administration.users.read" => Some(OperationScopePolicy::OneResource("user")),
        "academics.academic_years.read" => Some(OperationScopePolicy::OneResource("academic_year")),
        "academics.terms.read" => Some(OperationScopePolicy::OneResource("academic_term")),
        "academics.grade_levels.read" => {
            Some(OperationScopePolicy::OneResource("academic_grade_level"))
        }
        "academics.subjects.read" => Some(OperationScopePolicy::OneResource("subject")),
        "academics.classes.read" => Some(OperationScopePolicy::OneResource("class")),
        "academics.assessment_cycles.read" => {
            Some(OperationScopePolicy::OneResource("assessment_cycle"))
        }
        "academics.gradebook.mark_sheets.read" => {
            Some(OperationScopePolicy::OneResource("assessment_mark_sheet"))
        }
        "academics.reporting.grading_schemes.read" => {
            Some(OperationScopePolicy::OneResource("academic_grading_scheme"))
        }
        "academics.reporting.report_batches.read" => {
            Some(OperationScopePolicy::OneResource("academic_report_batch"))
        }
        "academics.reporting.transcripts.read" => {
            Some(OperationScopePolicy::OneResource("learner"))
        }
        "finance.currencies.read" => Some(OperationScopePolicy::OneResource("finance_currency")),
        "finance.accounts.read" => Some(OperationScopePolicy::OneResource("finance_account")),
        "finance.fiscal_years.read" | "finance.accounting_periods.list" => {
            Some(OperationScopePolicy::OneResource("finance_fiscal_year"))
        }
        "finance.journals.read" | "finance.journals.validation.read" => {
            Some(OperationScopePolicy::OneResource("finance_journal"))
        }
        "finance.posting_requests.read" => {
            Some(OperationScopePolicy::OneResource("finance_posting_request"))
        }
        "fees.fee_structures.read" => Some(OperationScopePolicy::OneResource("fees_fee_structure")),
        "procurement.suppliers.read" => {
            Some(OperationScopePolicy::OneResource("procurement_supplier"))
        }
        "assets_inventory.items.read" => {
            Some(OperationScopePolicy::OneResource("assets_inventory_item"))
        }
        "assets_inventory.stores.read" => {
            Some(OperationScopePolicy::OneResource("assets_inventory_store"))
        }
        "assets_inventory.stock_movements.read" => Some(OperationScopePolicy::OneResource(
            "assets_inventory_stock_movement",
        )),
        "assets_inventory.stock_requests.read"
        | "assets_inventory.stock_requests.fulfilment_preview.read" => Some(
            OperationScopePolicy::OneResource("assets_inventory_stock_request"),
        ),
        "attendance.learners.history.read" => Some(OperationScopePolicy::OneResource("learner")),
        "attendance.registers.read" => {
            Some(OperationScopePolicy::OneResource("attendance_register"))
        }
        "attendance.lesson_sessions.read" => Some(OperationScopePolicy::OneResource(
            "attendance_lesson_session",
        )),
        "attendance.exceptions.read" => {
            Some(OperationScopePolicy::OneResource("attendance_exception"))
        }
        "learning.spaces.read"
        | "learning.assignments.list"
        | "learning.progress.mine.read"
        | "learning.progress.list"
        | "learning.quizzes.list"
        | "learning.completion_policy.read"
        | "learning.completion.mine.read"
        | "learning.completion.list" => Some(OperationScopePolicy::OneResource("learning_space")),
        "learning.assignments.read" => {
            Some(OperationScopePolicy::OneResource("learning_assignment"))
        }
        "learning.submissions.mine.read" | "learning.submissions.list" => {
            Some(OperationScopePolicy::OneResource("learning_assignment"))
        }
        "learning.submissions.read" => {
            Some(OperationScopePolicy::OneResource("learning_submission"))
        }
        "learning.quizzes.read" | "learning.quiz_attempts.list" => {
            Some(OperationScopePolicy::OneResource("learning_quiz"))
        }
        "learning.quiz_attempts.read" => {
            Some(OperationScopePolicy::OneResource("learning_quiz_attempt"))
        }
        "student_support.cases.read" | "student_support.actions.list" => {
            Some(OperationScopePolicy::OneResource("student_support_case"))
        }
        "transport.routes.read" => Some(OperationScopePolicy::OneResource("transport_route")),
        "transport.runs.read" => Some(OperationScopePolicy::OneResource("transport_run")),
        "messaging.announcements.read"
        | "messaging.announcements.audience_preview.read"
        | "messaging.deliveries.list" => Some(OperationScopePolicy::OneResource(
            "communication_announcement",
        )),
        "messaging.inbox.read" => Some(OperationScopePolicy::OneResource("communication_delivery")),
        "hr_payroll.departments.read" => Some(OperationScopePolicy::OneResource("department")),
        "hr_payroll.positions.read" => Some(OperationScopePolicy::OneResource("position")),
        "fleet.vehicles.read" => Some(OperationScopePolicy::OneResource("vehicle")),
        "library.titles.read" => Some(OperationScopePolicy::OneResource("library_title")),
        "library.copies.read" => Some(OperationScopePolicy::OneResource("library_copy")),
        "library.members.read" => Some(OperationScopePolicy::OneResource("library_membership")),
        "library.loans.read" => Some(OperationScopePolicy::OneResource("library_loan")),
        "library.holds.read" => Some(OperationScopePolicy::OneResource("library_hold")),
        "library.fines.read" => Some(OperationScopePolicy::OneResource("library_fine")),
        "health.patients.read" => Some(OperationScopePolicy::OneResource("health_patient")),
        "health.visits.read" => Some(OperationScopePolicy::OneResource("health_visit")),
        "hostel.residences.read" => Some(OperationScopePolicy::OneResource("hostel_residence")),
        "hostel.rooms.read" => Some(OperationScopePolicy::OneResource("hostel_room")),
        "hostel.allocations.read" | "hostel.allocations.transfer_preview" => {
            Some(OperationScopePolicy::OneResource("hostel_allocation"))
        }
        "hostel.pastoral_records.read" => {
            Some(OperationScopePolicy::OneResource("hostel_pastoral_record"))
        }
        "document_registry.series.read" => Some(OperationScopePolicy::OneResource(
            "document_registry_series",
        )),
        "document_registry.files.read" | "document_registry.files.activity.list" => {
            Some(OperationScopePolicy::OneResource("document_registry_file"))
        }
        "document_registry.disposition_reviews.read" => Some(OperationScopePolicy::OneResource(
            "document_registry_disposition_review",
        )),
        "document_registry.legal_holds.read" => Some(OperationScopePolicy::OneResource(
            "document_registry_legal_hold",
        )),
        "internal_audit.plans.read" => {
            Some(OperationScopePolicy::OneResource("internal_audit_plan"))
        }
        "internal_audit.engagements.read" | "internal_audit.evidence.list" => Some(
            OperationScopePolicy::OneResource("internal_audit_engagement"),
        ),
        "internal_audit.findings.read" => {
            Some(OperationScopePolicy::OneResource("internal_audit_finding"))
        }
        "facilities.locations.read" => Some(OperationScopePolicy::OneResource("facility_location")),
        "facilities.requests.read" => Some(OperationScopePolicy::OneResource(
            "facility_service_request",
        )),
        "facilities.work_orders.read" => {
            Some(OperationScopePolicy::OneResource("facility_work_order"))
        }
        "activities.catalog.read" => {
            Some(OperationScopePolicy::OneResource("activity_catalog_item"))
        }
        "activities.groups.read" => Some(OperationScopePolicy::OneResource("activity_group")),
        "activities.sessions.read" => Some(OperationScopePolicy::OneResource("activity_session")),
        "academics.terms.list" | "academics.classes.list" => {
            Some(OperationScopePolicy::DatasetOrResources {
                allowed_kinds: &["academic_year"],
                maximum: 1,
            })
        }
        "academics.assessment_cycles.list" => Some(OperationScopePolicy::DatasetOrResources {
            allowed_kinds: &["academic_term"],
            maximum: 1,
        }),
        "assets_inventory.stock_balances.list" | "assets_inventory.stock_movements.list" => {
            Some(OperationScopePolicy::DatasetOrResources {
                allowed_kinds: ITEM_AND_STORE,
                maximum: 2,
            })
        }
        "assets_inventory.goods_receipt_allocations.list" => {
            Some(OperationScopePolicy::DatasetOrResources {
                allowed_kinds: &["procurement_goods_receipt"],
                maximum: 1,
            })
        }
        "assets_inventory.requester_candidates.list" => {
            Some(OperationScopePolicy::DatasetOrResources {
                allowed_kinds: &["hr_department"],
                maximum: 1,
            })
        }
        "assets_inventory.stock_requests.list" => Some(OperationScopePolicy::DatasetOrResources {
            allowed_kinds: &["hr_employee", "hr_department"],
            maximum: 2,
        }),
        "library.copies.list" => Some(OperationScopePolicy::DatasetOrResources {
            allowed_kinds: &["library_title"],
            maximum: 1,
        }),
        "health.patients.list"
        | "health.visits.list"
        | "health.medication_plans.list"
        | "health.medication_administrations.list"
        | "health.follow_ups.list" => Some(OperationScopePolicy::DatasetOrResources {
            allowed_kinds: &["health_patient"],
            maximum: 1,
        }),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
struct ResourceProbeError;

#[async_trait]
trait ResourceTenantProbe: Send + Sync {
    async fn belongs_to_tenant(
        &self,
        tenant_id: Uuid,
        resource_kind: &str,
        resource_id: Uuid,
    ) -> Result<bool, ResourceProbeError>;
}

struct PostgresResourceTenantProbe {
    pool: PgPool,
}

#[async_trait]
impl ResourceTenantProbe for PostgresResourceTenantProbe {
    async fn belongs_to_tenant(
        &self,
        tenant_id: Uuid,
        resource_kind: &str,
        resource_id: Uuid,
    ) -> Result<bool, ResourceProbeError> {
        let query = resource_existence_query(resource_kind).ok_or(ResourceProbeError)?;
        sqlx::query_scalar::<_, bool>(query)
            .bind(tenant_id)
            .bind(resource_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|_| ResourceProbeError)
    }
}

fn resource_existence_query(resource_kind: &str) -> Option<&'static str> {
    match resource_kind {
        "role" => Some(
            "SELECT EXISTS(SELECT 1 FROM roles WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "user" => Some(
            "SELECT EXISTS(SELECT 1 FROM users WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "ai_provider_connection" => Some(
            "SELECT EXISTS(SELECT 1 FROM ai_provider_connections WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "ai_route_set" => Some(
            "SELECT EXISTS(SELECT 1 FROM ai_route_sets WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "academic_year" => Some(
            "SELECT EXISTS(SELECT 1 FROM academic_years WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "academic_term" => Some(
            "SELECT EXISTS(SELECT 1 FROM academic_terms WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "academic_grade_level" => Some(
            "SELECT EXISTS(SELECT 1 FROM academic_grade_levels WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "subject" => Some(
            "SELECT EXISTS(SELECT 1 FROM subjects WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "class" => Some(
            "SELECT EXISTS(SELECT 1 FROM class_groups WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "assessment_cycle" => Some(
            "SELECT EXISTS(SELECT 1 FROM assessment_cycles WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "assessment_mark_sheet" => Some(
            "SELECT EXISTS(SELECT 1 FROM assessment_mark_sheets WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "academic_grading_scheme" => Some(
            "SELECT EXISTS(SELECT 1 FROM academic_grading_schemes WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "academic_report_batch" => Some(
            "SELECT EXISTS(SELECT 1 FROM academic_report_batches WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "learner" => Some(
            "SELECT EXISTS(SELECT 1 FROM learners WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "finance_currency" => Some(
            "SELECT EXISTS(SELECT 1 FROM finance_currencies WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "finance_account" => Some(
            "SELECT EXISTS(SELECT 1 FROM finance_accounts WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "finance_fiscal_year" => Some(
            "SELECT EXISTS(SELECT 1 FROM finance_fiscal_years WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "finance_journal" => Some(
            "SELECT EXISTS(SELECT 1 FROM finance_journals WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "finance_posting_request" => Some(
            "SELECT EXISTS(SELECT 1 FROM finance_posting_requests WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "fees_fee_structure" => Some(
            "SELECT EXISTS(SELECT 1 FROM fees_fee_structures WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "procurement_supplier" => Some(
            "SELECT EXISTS(SELECT 1 FROM procurement_suppliers WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "assets_inventory_item" => Some(
            "SELECT EXISTS(SELECT 1 FROM assets_inventory_items WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "assets_inventory_store" => Some(
            "SELECT EXISTS(SELECT 1 FROM assets_inventory_stores WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "assets_inventory_stock_movement" => Some(
            "SELECT EXISTS(SELECT 1 FROM assets_inventory_stock_movements WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "assets_inventory_stock_request" => Some(
            "SELECT EXISTS(SELECT 1 FROM assets_inventory_stock_requests WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "hr_employee" => Some(
            "SELECT EXISTS(SELECT 1 FROM employees WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "hr_department" => Some(
            "SELECT EXISTS(SELECT 1 FROM departments WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "procurement_goods_receipt" => Some(
            "SELECT EXISTS(SELECT 1 FROM procurement_goods_receipts WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "department" => Some(
            "SELECT EXISTS(SELECT 1 FROM departments WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "position" => Some(
            "SELECT EXISTS(SELECT 1 FROM positions WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "vehicle" => Some(
            "SELECT EXISTS(SELECT 1 FROM vehicles WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "attendance_register" => Some(
            "SELECT EXISTS(SELECT 1 FROM attendance_registers WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "attendance_lesson_session" => Some(
            "SELECT EXISTS(SELECT 1 FROM attendance_lesson_sessions WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "attendance_exception" => Some(
            "SELECT EXISTS(SELECT 1 FROM attendance_exceptions WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "learning_space" => Some(
            "SELECT EXISTS(SELECT 1 FROM learning_spaces WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "learning_assignment" => Some(
            "SELECT EXISTS(SELECT 1 FROM learning_assignments WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "learning_submission" => Some(
            "SELECT EXISTS(SELECT 1 FROM learning_submissions WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "learning_quiz" => Some(
            "SELECT EXISTS(SELECT 1 FROM learning_quizzes WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "learning_quiz_attempt" => Some(
            "SELECT EXISTS(SELECT 1 FROM learning_quiz_attempts WHERE tenant_id = $1 AND id = $2)",
        ),
        "student_support_case" => Some(
            "SELECT EXISTS(SELECT 1 FROM student_support_cases WHERE tenant_id = $1 AND id = $2)",
        ),
        "transport_route" => Some(
            "SELECT EXISTS(SELECT 1 FROM transport_routes WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "transport_run" => Some(
            "SELECT EXISTS(SELECT 1 FROM transport_service_runs WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "communication_announcement" => Some(
            "SELECT EXISTS(SELECT 1 FROM communication_announcements WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "communication_delivery" => Some(
            "SELECT EXISTS(SELECT 1 FROM communication_deliveries WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "library_title" => Some(
            "SELECT EXISTS(SELECT 1 FROM library_titles WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "library_copy" => Some(
            "SELECT EXISTS(SELECT 1 FROM library_copies WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "library_membership" => Some(
            "SELECT EXISTS(SELECT 1 FROM library_memberships WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "library_loan" => {
            Some("SELECT EXISTS(SELECT 1 FROM library_loans WHERE tenant_id = $1 AND id = $2)")
        }
        "library_hold" => {
            Some("SELECT EXISTS(SELECT 1 FROM library_holds WHERE tenant_id = $1 AND id = $2)")
        }
        "library_fine" => {
            Some("SELECT EXISTS(SELECT 1 FROM library_fines WHERE tenant_id = $1 AND id = $2)")
        }
        "health_patient" => {
            Some("SELECT EXISTS(SELECT 1 FROM health_patients WHERE tenant_id = $1 AND id = $2)")
        }
        "health_visit" => {
            Some("SELECT EXISTS(SELECT 1 FROM health_visits WHERE tenant_id = $1 AND id = $2)")
        }
        "hostel_residence" => {
            Some("SELECT EXISTS(SELECT 1 FROM hostel_residences WHERE tenant_id = $1 AND id = $2)")
        }
        "hostel_room" => {
            Some("SELECT EXISTS(SELECT 1 FROM hostel_rooms WHERE tenant_id = $1 AND id = $2)")
        }
        "hostel_allocation" => {
            Some("SELECT EXISTS(SELECT 1 FROM hostel_allocations WHERE tenant_id = $1 AND id = $2)")
        }
        "hostel_pastoral_record" => Some(
            "SELECT EXISTS(SELECT 1 FROM hostel_pastoral_records WHERE tenant_id = $1 AND id = $2)",
        ),
        "document_registry_series" => Some(
            "SELECT EXISTS(SELECT 1 FROM document_registry_series WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "document_registry_file" => Some(
            "SELECT EXISTS(SELECT 1 FROM document_registry_files WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "document_registry_disposition_review" => Some(
            "SELECT EXISTS(SELECT 1 FROM document_registry_disposition_reviews WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "document_registry_legal_hold" => Some(
            "SELECT EXISTS(SELECT 1 FROM document_registry_legal_holds WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "internal_audit_plan" => Some(
            "SELECT EXISTS(SELECT 1 FROM internal_audit_plans WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "internal_audit_engagement" => Some(
            "SELECT EXISTS(SELECT 1 FROM internal_audit_engagements WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "internal_audit_finding" => Some(
            "SELECT EXISTS(SELECT 1 FROM internal_audit_findings WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
        ),
        "facility_location" => {
            Some("SELECT EXISTS(SELECT 1 FROM facility_locations WHERE tenant_id = $1 AND id = $2)")
        }
        "facility_service_request" => Some(
            "SELECT EXISTS(SELECT 1 FROM facility_service_requests WHERE tenant_id = $1 AND id = $2)",
        ),
        "facility_work_order" => Some(
            "SELECT EXISTS(SELECT 1 FROM facility_work_orders WHERE tenant_id = $1 AND id = $2)",
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, sync::Arc};

    use async_trait::async_trait;
    use cp_agent::{
        AuthenticatedAgentPrincipal, CapabilityResource, CapabilityScope, CurrentAuthority,
        RecordScopeAuthorizer, RecordScopeDenied,
    };
    use cp_common::{
        AccessContext, AgentExposure, EntitlementSnapshot, LeaseLifecycle, ModuleEntitlementState,
        OperationEffect, ProductOperation, operation_catalog,
    };
    use uuid::Uuid;

    use super::{
        AppRecordScopeAuthorizer, INITIAL_WORKER_OPERATION_KEYS, ResourceProbeError,
        ResourceTenantProbe, WITHHELD_RECORD_SCOPED_OPERATION_KEYS, is_initial_worker_operation,
        resource_existence_query,
    };

    #[derive(Default)]
    struct FakeResourceProbe {
        visible: BTreeSet<(Uuid, String, Uuid)>,
        unavailable: bool,
    }

    #[async_trait]
    impl ResourceTenantProbe for FakeResourceProbe {
        async fn belongs_to_tenant(
            &self,
            tenant_id: Uuid,
            resource_kind: &str,
            resource_id: Uuid,
        ) -> Result<bool, ResourceProbeError> {
            if self.unavailable {
                return Err(ResourceProbeError);
            }
            Ok(self
                .visible
                .contains(&(tenant_id, resource_kind.to_owned(), resource_id)))
        }
    }

    fn resource_scope(resources: &[(&str, &str)]) -> CapabilityScope {
        CapabilityScope::resources(resources.iter().map(|(kind, id)| {
            CapabilityResource::parse(*kind, *id)
                .unwrap_or_else(|error| panic!("test resource must parse: {error}"))
        }))
        .unwrap_or_else(|error| panic!("test scope must parse: {error}"))
    }

    fn principal(tenant_id: Uuid) -> AuthenticatedAgentPrincipal {
        AuthenticatedAgentPrincipal::from_authenticated_request(tenant_id, Uuid::new_v4())
    }

    fn authority(permissions: &[&str], module_key: &str) -> CurrentAuthority {
        CurrentAuthority::from_reloaded_access(AccessContext {
            role_keys: vec!["test_role".to_owned()],
            permissions: permissions
                .iter()
                .map(|permission| (*permission).to_owned())
                .collect(),
            enabled_modules: vec![module_key.to_owned()],
            entitlements: EntitlementSnapshot::new(
                LeaseLifecycle::Active,
                [(module_key.to_owned(), ModuleEntitlementState::Enabled)],
                [],
            )
            .unwrap_or_else(|error| panic!("test entitlement must parse: {error}")),
        })
    }

    #[test]
    fn discovery_partition_covers_every_directly_exposed_operation_once() {
        assert_eq!(INITIAL_WORKER_OPERATION_KEYS.len(), 168);
        assert_eq!(WITHHELD_RECORD_SCOPED_OPERATION_KEYS.len(), 68);

        let initial = INITIAL_WORKER_OPERATION_KEYS
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let withheld = WITHHELD_RECORD_SCOPED_OPERATION_KEYS
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        assert_eq!(initial.len(), INITIAL_WORKER_OPERATION_KEYS.len());
        assert_eq!(withheld.len(), WITHHELD_RECORD_SCOPED_OPERATION_KEYS.len());
        assert!(initial.is_disjoint(&withheld));

        let exposed = operation_catalog()
            .iter()
            .filter(|entry| entry.operation().agent_exposure() == AgentExposure::Exposed)
            .map(|entry| entry.operation().key())
            .collect::<BTreeSet<_>>();
        assert_eq!(exposed.len(), 236);
        assert_eq!(
            initial.union(&withheld).copied().collect::<BTreeSet<_>>(),
            exposed
        );
        for operation_key in INITIAL_WORKER_OPERATION_KEYS {
            assert!(
                is_initial_worker_operation(operation_key),
                "initial operation is missing policy: {operation_key}"
            );
        }
        for operation_key in WITHHELD_RECORD_SCOPED_OPERATION_KEYS {
            assert!(
                !is_initial_worker_operation(operation_key),
                "withheld operation unexpectedly has executable policy: {operation_key}"
            );
        }
    }

    #[test]
    fn sensitive_and_unknown_operations_are_not_discoverable() {
        assert!(is_initial_worker_operation("finance.journals.list"));
        assert!(!is_initial_worker_operation("sis.learners.list"));
        assert!(!is_initial_worker_operation("timetabling.runs.read_latest"));
        assert!(!is_initial_worker_operation("unknown.records.list"));
    }

    #[actix_web::test]
    async fn exact_same_tenant_resource_is_authorized() {
        let tenant_id = Uuid::new_v4();
        let record_id = Uuid::new_v4();
        let probe = FakeResourceProbe {
            visible: BTreeSet::from([(tenant_id, "finance_journal".to_owned(), record_id)]),
            unavailable: false,
        };
        let authorizer = AppRecordScopeAuthorizer::with_probe(Arc::new(probe));
        let record_id = record_id.to_string();

        assert!(
            authorizer
                .authorize_operation_scope(
                    principal(tenant_id),
                    "finance.journals.read",
                    &resource_scope(&[("finance_journal", &record_id)]),
                )
                .await
                .is_ok()
        );
    }

    #[actix_web::test]
    async fn broker_trait_entry_rechecks_exact_operation_authority() {
        let tenant_id = Uuid::new_v4();
        let authorizer =
            AppRecordScopeAuthorizer::with_probe(Arc::new(FakeResourceProbe::default()));
        let operation = ProductOperation::route(
            "finance.journals.list",
            "finance",
            "finance:view",
            OperationEffect::Read,
            AgentExposure::Exposed,
            true,
        );

        assert!(
            authorizer
                .authorize(
                    principal(tenant_id),
                    &authority(&["finance:view"], "finance"),
                    &operation,
                    &CapabilityScope::TenantWide,
                )
                .await
                .is_ok()
        );
        assert_eq!(
            authorizer
                .authorize(
                    principal(tenant_id),
                    &authority(&[], "finance"),
                    &operation,
                    &CapabilityScope::TenantWide,
                )
                .await,
            Err(RecordScopeDenied)
        );
    }

    #[actix_web::test]
    async fn cross_tenant_resource_is_denied() {
        let tenant_id = Uuid::new_v4();
        let other_tenant_id = Uuid::new_v4();
        let record_id = Uuid::new_v4();
        let probe = FakeResourceProbe {
            visible: BTreeSet::from([(other_tenant_id, "role".to_owned(), record_id)]),
            unavailable: false,
        };
        let authorizer = AppRecordScopeAuthorizer::with_probe(Arc::new(probe));
        let record_id = record_id.to_string();

        assert_eq!(
            authorizer
                .authorize_operation_scope(
                    principal(tenant_id),
                    "administration.roles.read",
                    &resource_scope(&[("role", &record_id)]),
                )
                .await,
            Err(RecordScopeDenied)
        );
    }

    #[actix_web::test]
    async fn malformed_nil_wrong_kind_and_wrong_shape_are_denied() {
        let tenant_id = Uuid::new_v4();
        let authorizer =
            AppRecordScopeAuthorizer::with_probe(Arc::new(FakeResourceProbe::default()));
        let random_id = Uuid::new_v4().to_string();
        let cases = [
            resource_scope(&[("role", "not-a-uuid")]),
            resource_scope(&[("role", &Uuid::nil().to_string())]),
            resource_scope(&[("user", &random_id)]),
        ];
        for scope in cases {
            assert_eq!(
                authorizer
                    .authorize_operation_scope(
                        principal(tenant_id),
                        "administration.roles.read",
                        &scope,
                    )
                    .await,
                Err(RecordScopeDenied)
            );
        }
        assert_eq!(
            authorizer
                .authorize_operation_scope(
                    principal(tenant_id),
                    "administration.roles.read",
                    &CapabilityScope::TenantWide,
                )
                .await,
            Err(RecordScopeDenied)
        );
        assert_eq!(
            authorizer
                .authorize_operation_scope(
                    principal(tenant_id),
                    "administration.roles.list",
                    &resource_scope(&[("role", &random_id)]),
                )
                .await,
            Err(RecordScopeDenied)
        );
    }

    #[actix_web::test]
    async fn duplicate_or_excess_filter_resources_are_denied() {
        let tenant_id = Uuid::new_v4();
        let first = Uuid::new_v4().to_string();
        let second = Uuid::new_v4().to_string();
        let third = Uuid::new_v4().to_string();
        let authorizer =
            AppRecordScopeAuthorizer::with_probe(Arc::new(FakeResourceProbe::default()));

        for scope in [
            resource_scope(&[
                ("assets_inventory_item", &first),
                ("assets_inventory_item", &second),
            ]),
            resource_scope(&[
                ("assets_inventory_item", &first),
                ("assets_inventory_store", &second),
                ("procurement_goods_receipt", &third),
            ]),
        ] {
            assert_eq!(
                authorizer
                    .authorize_operation_scope(
                        principal(tenant_id),
                        "assets_inventory.stock_balances.list",
                        &scope,
                    )
                    .await,
                Err(RecordScopeDenied)
            );
        }
    }

    #[actix_web::test]
    async fn unknown_operation_and_probe_failure_deny_without_fallback() {
        let tenant_id = Uuid::new_v4();
        let record_id = Uuid::new_v4().to_string();
        let authorizer = AppRecordScopeAuthorizer::with_probe(Arc::new(FakeResourceProbe {
            visible: BTreeSet::new(),
            unavailable: true,
        }));

        assert_eq!(
            authorizer
                .authorize_operation_scope(
                    principal(tenant_id),
                    "unknown.records.read",
                    &CapabilityScope::TenantWide,
                )
                .await,
            Err(RecordScopeDenied)
        );
        assert_eq!(
            authorizer
                .authorize_operation_scope(
                    principal(tenant_id),
                    "finance.accounts.read",
                    &resource_scope(&[("finance_account", &record_id)]),
                )
                .await,
            Err(RecordScopeDenied)
        );
    }

    #[actix_web::test]
    async fn nil_authenticated_identity_is_denied() {
        let authorizer =
            AppRecordScopeAuthorizer::with_probe(Arc::new(FakeResourceProbe::default()));
        for principal in [
            AuthenticatedAgentPrincipal::from_authenticated_request(Uuid::nil(), Uuid::new_v4()),
            AuthenticatedAgentPrincipal::from_authenticated_request(Uuid::new_v4(), Uuid::nil()),
        ] {
            assert_eq!(
                authorizer
                    .authorize_operation_scope(
                        principal,
                        "finance.journals.list",
                        &CapabilityScope::TenantWide,
                    )
                    .await,
                Err(RecordScopeDenied)
            );
        }
    }

    #[test]
    fn every_resource_kind_has_a_static_tenant_probe() {
        for resource_kind in [
            "role",
            "user",
            "ai_provider_connection",
            "ai_route_set",
            "academic_year",
            "academic_term",
            "academic_grade_level",
            "subject",
            "class",
            "assessment_cycle",
            "finance_currency",
            "finance_account",
            "finance_fiscal_year",
            "finance_journal",
            "finance_posting_request",
            "fees_fee_structure",
            "procurement_supplier",
            "assets_inventory_item",
            "assets_inventory_store",
            "assets_inventory_stock_movement",
            "procurement_goods_receipt",
            "department",
            "position",
            "vehicle",
            "learning_space",
            "learning_assignment",
            "learning_submission",
            "learning_quiz",
            "internal_audit_plan",
            "internal_audit_engagement",
            "internal_audit_finding",
        ] {
            let query = resource_existence_query(resource_kind)
                .unwrap_or_else(|| panic!("missing query for {resource_kind}"));
            assert!(query.contains("tenant_id = $1"));
            assert!(query.contains("id = $2"));
            assert!(query.contains("deleted_at IS NULL"));
        }
        let attempt_query = resource_existence_query("learning_quiz_attempt")
            .unwrap_or_else(|| panic!("missing query for learning_quiz_attempt"));
        assert!(attempt_query.contains("tenant_id = $1"));
        assert!(attempt_query.contains("id = $2"));
        assert_eq!(resource_existence_query("unknown"), None);
    }
}
