//! Assembles production Agent capability adapters over existing domain services.

mod academic_assessments;
mod academic_reporting;
mod academics;
mod administration;
mod administration_access;
mod ai_providers;
mod ai_routing;
mod assets_inventory;
mod attendance;
mod authority;
mod document_registry;
mod facilities;
mod fees;
mod finance;
mod fleet;
pub mod governance;
mod gradebook;
mod health;
mod hostel;
mod hr;
mod internal_audit;
mod learning;
mod library;
mod messaging;
mod origin;
mod procurement;
mod record_scope;
mod session_dtos;
pub mod session_routes;
mod sis;
mod student_support;
mod submission_gate;
mod timetabling;
mod transport;
mod usage_dtos;
pub mod usage_routes;
mod worker_executor;
mod worker_readiness;
mod worker_supervisor;

use cp_agent::CapabilityRegistry;
use sqlx::PgPool;

use crate::config::LicenseConfig;

use crate::services::ai_routing::selectors::{
    routing_capability_option, routing_capability_options, sort_capability_options,
};
use academic_assessments::{
    AssessmentComponentReadCapability, AssessmentComponentsListCapability,
    AssessmentCycleReadCapability, AssessmentCyclesListCapability,
};
use academic_reporting::{
    GradingSchemeReadCapability, GradingSchemesListCapability, ReportBatchReadCapability,
    ReportBatchesListCapability, ReportingReferencesCapability, TranscriptReadCapability,
};
use academics::{
    AcademicsListCapability, AcademicsListKind, AcademicsReadCapability, AcademicsReadKind,
    TeacherCandidatesCapability,
};
use administration::{
    AdministrationCatalogCapability, AdministrationLicensingCapability,
    AdministrationModulesCapability, AdministrationSchoolSettingsCapability,
};
use administration_access::{
    AdministrationRoleReadCapability, AdministrationRolesListCapability,
    AdministrationUserReadCapability, AdministrationUsersListCapability,
};
use ai_providers::{
    AiProviderCatalogCapability, AiProviderConnectionReadCapability,
    AiProviderConnectionsListCapability, AiProviderModelsListCapability,
};
use ai_routing::{
    AiRouteReadCapability, AiRouteResolveCapability, AiRoutesListCapability,
    AiRoutingOptionsCapability,
};
use assets_inventory::{
    AssetsInventoryListCapability, AssetsInventoryListKind, AssetsInventoryReadCapability,
    AssetsInventoryReadKind, GoodsReceiptAllocationsListCapability, StockBalancesListCapability,
    StockMovementReadCapability, StockMovementsListCapability, StockRequestCandidateKind,
    StockRequestCandidatesCapability, StockRequestReadCapability, StockRequestReadKind,
    StockRequestsListCapability,
};
use attendance::{
    AttendanceLearnerHistoryCapability, AttendanceReferencesCapability,
    AttendanceRegisterReadCapability, AttendanceRegistersListCapability,
};
use document_registry::{RegistryReadCapability, RegistryReadKind};
use facilities::{
    FacilitiesLocationsCapability, FacilitiesReadCapability, FacilitiesReadKind,
    FacilitiesRequestsCapability, FacilitiesWorkOrdersCapability,
};
use fees::{
    FeesImportPreviewCapability, FeesImportReadCapability, FeesImportsListCapability,
    FeesLearnerCandidatesCapability, FeesListCapability, FeesListKind, FeesReadCapability,
    FeesReadKind, FeesReferenceDataCapability,
};
use finance::{
    FinanceJournalValidationCapability, FinanceJournalsListCapability, FinanceListCapability,
    FinanceListKind, FinancePeriodsCapability, FinancePostingRequestsListCapability,
    FinanceReadCapability, FinanceReadKind,
};
use fleet::{
    FleetDriverCandidatesListCapability, FleetDriverReadCapability, FleetDriversListCapability,
    FleetVehicleLogReadCapability, FleetVehicleLogsListCapability, FleetVehicleReadCapability,
    FleetVehiclesListCapability,
};
use gradebook::{
    GradebookMarkSheetReadCapability, GradebookMarkSheetsListCapability,
    GradebookReferencesCapability,
};
use health::{
    HealthListCapability, HealthListKind, HealthReadCapability, HealthReadKind,
    HealthReferencesCapability,
};
use hostel::{
    AllocationPreviewCapability, HostelListCapability, HostelListKind, HostelReadCapability,
    HostelReadKind, HostelReferencesCapability, TransferPreviewCapability,
};
use hr::{
    HrDepartmentReadCapability, HrDepartmentsListCapability, HrEmployeeAvailabilityListCapability,
    HrEmployeeAvailabilityReadCapability, HrEmployeeReadCapability, HrEmployeesListCapability,
    HrEmploymentEngagementReadCapability, HrEmploymentEngagementsListCapability,
    HrImportPreviewCapability, HrImportReadCapability, HrImportsListCapability,
    HrPositionReadCapability, HrPositionsListCapability,
};
use internal_audit::{InternalAuditReadCapability, InternalAuditReadKind};
use learning::{
    LearningReferencesCapability, LearningResourceFilesCapability, LearningSettingsCapability,
    LearningSpaceCapability, LearningSpacesCapability,
};
use library::{
    LibraryCopiesCapability, LibraryListCapability, LibraryListKind, LibraryReadCapability,
    LibraryReadKind, LibraryReferencesCapability, LibrarySettingsCapability,
};
use messaging::{
    CommunicationAnnouncementReadCapability, CommunicationAnnouncementsListCapability,
    CommunicationAudiencePreviewCapability, CommunicationDeliveriesCapability,
    CommunicationInboxListCapability, CommunicationInboxReadCapability,
    CommunicationReferencesCapability,
};
use procurement::{
    ProcurementGoodsReceiptsListCapability, ProcurementPurchaseOrdersListCapability,
    ProcurementReadCapability, ProcurementReadKind, ProcurementReferenceDataCapability,
    ProcurementRequesterCandidatesCapability, ProcurementRequisitionsListCapability,
    ProcurementSuppliersListCapability,
};
use sis::{
    AccountCandidatesCapability, LearnerNumberingPolicyCapability, SisImportPreviewCapability,
    SisImportReadCapability, SisImportsListCapability, SisListCapability, SisListKind,
    SisReadCapability, SisReadKind,
};
use student_support::{
    StudentSupportActionsListCapability, StudentSupportCaseReadCapability,
    StudentSupportCasesListCapability,
};
use timetabling::{
    LatestTimetableRunCapability, TimetableConfigurationCapability, TimetableRunReadCapability,
    TimetableRunsListCapability,
};
use transport::{
    TransportRidersListCapability, TransportRouteReadCapability, TransportRoutesListCapability,
    TransportRunReadCapability, TransportRunsListCapability,
};

pub use authority::AppAuthorityLoader;
pub use record_scope::{
    AppRecordScopeAuthorizer, INITIAL_WORKER_OPERATION_KEYS, is_initial_worker_operation,
};
pub use submission_gate::AgentSubmissionGate;
pub use worker_executor::ProviderAgentRunExecutor;
pub use worker_readiness::{
    AgentWorkerCoverageProof, AgentWorkerInstance, AgentWorkerReadiness, AgentWorkerReadinessError,
    AgentWorkerReadinessOps, AgentWorkerReadinessReason,
};
pub use worker_supervisor::{
    AgentExecutionFailure, AgentRunExecutor, AgentWorkerSupervisor, AgentWorkerSupervisorError,
    AgentWorkerTick,
};

#[must_use]
pub fn build_capability_registry(
    pool: PgPool,
    license_config: LicenseConfig,
) -> CapabilityRegistry {
    let mut registry = CapabilityRegistry::from_product_catalog()
        .unwrap_or_else(|error| panic!("invalid product operation catalogue: {error}"));
    registry
        .register(AdministrationCatalogCapability::new())
        .unwrap_or_else(|error| panic!("invalid Administration catalogue capability: {error}"));
    registry
        .register(AdministrationModulesCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Administration modules capability: {error}"));
    registry
        .register(AdministrationRolesListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Administration roles-list capability: {error}"));
    registry
        .register(AdministrationRoleReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Administration role-read capability: {error}"));
    registry
        .register(AdministrationUsersListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Administration users-list capability: {error}"));
    registry
        .register(AdministrationUserReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Administration user-read capability: {error}"));
    registry
        .register(AdministrationSchoolSettingsCapability::new(pool.clone()))
        .unwrap_or_else(|error| {
            panic!("invalid Administration school-settings capability: {error}")
        });
    registry
        .register(AdministrationLicensingCapability::new(
            pool.clone(),
            license_config,
        ))
        .unwrap_or_else(|error| panic!("invalid Administration licensing capability: {error}"));
    registry
        .register(AiProviderCatalogCapability::new())
        .unwrap_or_else(|error| panic!("invalid AI provider-catalog capability: {error}"));
    registry
        .register(AiProviderConnectionsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid AI provider-connections capability: {error}"));
    registry
        .register(AiProviderConnectionReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid AI provider-connection capability: {error}"));
    registry
        .register(AiProviderModelsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid AI provider-models capability: {error}"));
    for kind in [
        AssetsInventoryListKind::Items,
        AssetsInventoryListKind::Stores,
    ] {
        registry
            .register(AssetsInventoryListCapability::new(pool.clone(), kind))
            .unwrap_or_else(|error| panic!("invalid {} capability: {error}", kind.operation_key()));
    }
    for kind in [
        AssetsInventoryReadKind::Item,
        AssetsInventoryReadKind::Store,
    ] {
        registry
            .register(AssetsInventoryReadCapability::new(pool.clone(), kind))
            .unwrap_or_else(|error| panic!("invalid {} capability: {error}", kind.operation_key()));
    }
    registry
        .register(StockBalancesListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Assets stock-balances capability: {error}"));
    registry
        .register(StockMovementsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Assets stock-movements list capability: {error}"));
    registry
        .register(StockMovementReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Assets stock-movement read capability: {error}"));
    registry
        .register(GoodsReceiptAllocationsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| {
            panic!("invalid Assets goods-receipt allocations capability: {error}")
        });
    for kind in [
        StockRequestCandidateKind::Requesters,
        StockRequestCandidateKind::Departments,
    ] {
        registry
            .register(StockRequestCandidatesCapability::new(pool.clone(), kind))
            .unwrap_or_else(|error| panic!("invalid {} capability: {error}", kind.operation_key()));
    }
    registry
        .register(StockRequestsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Assets stock-requests list capability: {error}"));
    for kind in [
        StockRequestReadKind::Request,
        StockRequestReadKind::FulfilmentPreview,
    ] {
        registry
            .register(StockRequestReadCapability::new(pool.clone(), kind))
            .unwrap_or_else(|error| panic!("invalid {} capability: {error}", kind.operation_key()));
    }
    for kind in [
        AcademicsListKind::AcademicYears,
        AcademicsListKind::Terms,
        AcademicsListKind::GradeLevels,
        AcademicsListKind::Subjects,
        AcademicsListKind::Teachers,
        AcademicsListKind::Classes,
        AcademicsListKind::TeachingAssignments,
    ] {
        registry
            .register(AcademicsListCapability::new(pool.clone(), kind))
            .unwrap_or_else(|error| panic!("invalid {} capability: {error}", kind.operation_key()));
    }
    for kind in [
        AcademicsReadKind::AcademicYear,
        AcademicsReadKind::Term,
        AcademicsReadKind::GradeLevel,
        AcademicsReadKind::Subject,
        AcademicsReadKind::Teacher,
        AcademicsReadKind::Class,
        AcademicsReadKind::TeachingAssignment,
    ] {
        registry
            .register(AcademicsReadCapability::new(pool.clone(), kind))
            .unwrap_or_else(|error| panic!("invalid {} capability: {error}", kind.operation_key()));
    }
    registry
        .register(TeacherCandidatesCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Academics teacher-candidates capability: {error}"));
    registry
        .register(AssessmentCyclesListCapability::new(pool.clone()))
        .unwrap_or_else(|error| {
            panic!("invalid Academics assessment-cycles list capability: {error}")
        });
    registry
        .register(AssessmentCycleReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| {
            panic!("invalid Academics assessment-cycle read capability: {error}")
        });
    registry
        .register(AssessmentComponentsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| {
            panic!("invalid Academics assessment-components list capability: {error}")
        });
    registry
        .register(AssessmentComponentReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| {
            panic!("invalid Academics assessment-component read capability: {error}")
        });
    registry
        .register(GradebookReferencesCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Gradebook references capability: {error}"));
    registry
        .register(GradebookMarkSheetsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Gradebook mark-sheets list capability: {error}"));
    registry
        .register(GradebookMarkSheetReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Gradebook mark-sheet read capability: {error}"));
    registry
        .register(ReportingReferencesCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid reporting references capability: {error}"));
    registry
        .register(GradingSchemesListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid grading-schemes list capability: {error}"));
    registry
        .register(GradingSchemeReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid grading-scheme read capability: {error}"));
    registry
        .register(ReportBatchesListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid report-batches list capability: {error}"));
    registry
        .register(ReportBatchReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid report-batch read capability: {error}"));
    registry
        .register(TranscriptReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid transcript read capability: {error}"));
    registry
        .register(AttendanceReferencesCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Attendance references capability: {error}"));
    registry
        .register(AttendanceRegistersListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Attendance registers-list capability: {error}"));
    registry
        .register(AttendanceRegisterReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Attendance register-read capability: {error}"));
    registry
        .register(AttendanceLearnerHistoryCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Attendance learner-history capability: {error}"));
    registry
        .register(LearningSettingsCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid E-learning settings capability: {error}"));
    registry
        .register(LearningReferencesCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid E-learning references capability: {error}"));
    registry
        .register(LearningResourceFilesCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid E-learning resource-files capability: {error}"));
    registry
        .register(LearningSpacesCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid E-learning spaces-list capability: {error}"));
    registry
        .register(LearningSpaceCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid E-learning space-read capability: {error}"));
    registry
        .register(StudentSupportCasesListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Student Support cases-list capability: {error}"));
    registry
        .register(StudentSupportCaseReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Student Support case-read capability: {error}"));
    registry
        .register(StudentSupportActionsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Student Support actions-list capability: {error}"));
    registry
        .register(TransportRoutesListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Transport routes-list capability: {error}"));
    registry
        .register(TransportRouteReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Transport route-read capability: {error}"));
    registry
        .register(TransportRidersListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Transport riders-list capability: {error}"));
    registry
        .register(TransportRunsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Transport runs-list capability: {error}"));
    registry
        .register(TransportRunReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Transport run-read capability: {error}"));
    registry
        .register(CommunicationReferencesCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Communication references capability: {error}"));
    registry
        .register(CommunicationAnnouncementsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| {
            panic!("invalid Communication announcements-list capability: {error}")
        });
    registry
        .register(CommunicationAnnouncementReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| {
            panic!("invalid Communication announcement-read capability: {error}")
        });
    registry
        .register(CommunicationAudiencePreviewCapability::new(pool.clone()))
        .unwrap_or_else(|error| {
            panic!("invalid Communication audience-preview capability: {error}")
        });
    registry
        .register(CommunicationDeliveriesCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Communication deliveries capability: {error}"));
    registry
        .register(CommunicationInboxListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Communication inbox-list capability: {error}"));
    registry
        .register(CommunicationInboxReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Communication inbox-read capability: {error}"));
    for kind in [
        FinanceListKind::Currencies,
        FinanceListKind::Accounts,
        FinanceListKind::FiscalYears,
    ] {
        registry
            .register(FinanceListCapability::new(pool.clone(), kind))
            .unwrap_or_else(|error| panic!("invalid Finance list capability: {error}"));
    }
    for kind in [
        FinanceReadKind::Currency,
        FinanceReadKind::Account,
        FinanceReadKind::FiscalYear,
        FinanceReadKind::Journal,
        FinanceReadKind::PostingRequest,
    ] {
        registry
            .register(FinanceReadCapability::new(pool.clone(), kind))
            .unwrap_or_else(|error| panic!("invalid Finance read capability: {error}"));
    }
    for kind in [
        SisListKind::Learners,
        SisListKind::Guardians,
        SisListKind::GuardianRelationships,
        SisListKind::Applications,
        SisListKind::Enrolments,
    ] {
        registry
            .register(SisListCapability::new(pool.clone(), kind))
            .unwrap_or_else(|error| panic!("invalid {} capability: {error}", kind.operation_key()));
    }
    registry
        .register(FinancePeriodsCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Finance accounting-periods capability: {error}"));
    registry
        .register(FinanceJournalsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Finance journals-list capability: {error}"));
    registry
        .register(FinancePostingRequestsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| {
            panic!("invalid Finance posting-requests-list capability: {error}")
        });
    registry
        .register(FinanceJournalValidationCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Finance journal-validation capability: {error}"));
    registry
        .register(FeesReferenceDataCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Fees reference-data capability: {error}"));
    registry
        .register(FeesLearnerCandidatesCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Fees learner-candidates capability: {error}"));
    registry
        .register(FeesImportsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Fees imports-list capability: {error}"));
    registry
        .register(FeesImportReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Fees import-read capability: {error}"));
    registry
        .register(FeesImportPreviewCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Fees import-preview capability: {error}"));
    for kind in [
        FeesListKind::BillingAccounts,
        FeesListKind::FeeStructures,
        FeesListKind::Invoices,
    ] {
        registry
            .register(FeesListCapability::new(pool.clone(), kind))
            .unwrap_or_else(|error| panic!("invalid {} capability: {error}", kind.operation_key()));
    }
    for kind in [
        FeesReadKind::BillingAccount,
        FeesReadKind::FeeStructure,
        FeesReadKind::Invoice,
    ] {
        registry
            .register(FeesReadCapability::new(pool.clone(), kind))
            .unwrap_or_else(|error| panic!("invalid {} capability: {error}", kind.operation_key()));
    }
    for kind in [
        SisReadKind::Learner,
        SisReadKind::Guardian,
        SisReadKind::GuardianRelationship,
        SisReadKind::Application,
        SisReadKind::Enrolment,
    ] {
        registry
            .register(SisReadCapability::new(pool.clone(), kind))
            .unwrap_or_else(|error| panic!("invalid {} capability: {error}", kind.operation_key()));
    }
    registry
        .register(AccountCandidatesCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid SIS account-candidates capability: {error}"));
    registry
        .register(SisImportsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid SIS imports-list capability: {error}"));
    registry
        .register(SisImportReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid SIS import-read capability: {error}"));
    registry
        .register(SisImportPreviewCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid SIS import-preview capability: {error}"));
    registry
        .register(LearnerNumberingPolicyCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid SIS learner-numbering capability: {error}"));
    registry
        .register(HrImportsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid HR imports-list capability: {error}"));
    registry
        .register(HrImportReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid HR import-read capability: {error}"));
    registry
        .register(HrImportPreviewCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid HR import-preview capability: {error}"));
    registry
        .register(HrDepartmentsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid HR departments-list capability: {error}"));
    registry
        .register(HrDepartmentReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid HR department-read capability: {error}"));
    registry
        .register(HrPositionsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid HR positions-list capability: {error}"));
    registry
        .register(HrPositionReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid HR position-read capability: {error}"));
    registry
        .register(HrEmployeesListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid HR employees-list capability: {error}"));
    registry
        .register(HrEmployeeReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid HR employee-read capability: {error}"));
    registry
        .register(HrEmploymentEngagementsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| {
            panic!("invalid HR employment-engagements-list capability: {error}")
        });
    registry
        .register(HrEmploymentEngagementReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| {
            panic!("invalid HR employment-engagement-read capability: {error}")
        });
    registry
        .register(HrEmployeeAvailabilityListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid HR availability-list capability: {error}"));
    registry
        .register(HrEmployeeAvailabilityReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid HR availability-read capability: {error}"));
    registry
        .register(ProcurementReferenceDataCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Procurement reference-data capability: {error}"));
    registry
        .register(ProcurementRequesterCandidatesCapability::new(pool.clone()))
        .unwrap_or_else(|error| {
            panic!("invalid Procurement requester-candidates capability: {error}")
        });
    registry
        .register(ProcurementSuppliersListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Procurement suppliers-list capability: {error}"));
    registry
        .register(ProcurementRequisitionsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| {
            panic!("invalid Procurement requisitions-list capability: {error}")
        });
    registry
        .register(ProcurementPurchaseOrdersListCapability::new(pool.clone()))
        .unwrap_or_else(|error| {
            panic!("invalid Procurement purchase-orders-list capability: {error}")
        });
    registry
        .register(ProcurementGoodsReceiptsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| {
            panic!("invalid Procurement goods-receipts-list capability: {error}")
        });
    for kind in [
        ProcurementReadKind::Supplier,
        ProcurementReadKind::Requisition,
        ProcurementReadKind::PurchaseOrder,
        ProcurementReadKind::GoodsReceipt,
    ] {
        registry
            .register(ProcurementReadCapability::new(pool.clone(), kind))
            .unwrap_or_else(|error| panic!("invalid {} capability: {error}", kind.operation_key()));
    }
    registry
        .register(FleetDriverCandidatesListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Fleet driver-candidates capability: {error}"));
    registry
        .register(FleetVehiclesListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Fleet vehicles-list capability: {error}"));
    registry
        .register(FleetVehicleReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Fleet vehicle-read capability: {error}"));
    registry
        .register(FleetDriversListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Fleet drivers-list capability: {error}"));
    registry
        .register(FleetDriverReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Fleet driver-read capability: {error}"));
    registry
        .register(FleetVehicleLogsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Fleet vehicle-logs-list capability: {error}"));
    registry
        .register(FleetVehicleLogReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Fleet vehicle-log-read capability: {error}"));
    registry
        .register(FacilitiesLocationsCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Facilities locations-list capability: {error}"));
    registry
        .register(FacilitiesRequestsCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Facilities requests-list capability: {error}"));
    registry
        .register(FacilitiesWorkOrdersCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Facilities work-orders-list capability: {error}"));
    for kind in [
        FacilitiesReadKind::Location,
        FacilitiesReadKind::Request,
        FacilitiesReadKind::WorkOrder,
    ] {
        registry
            .register(FacilitiesReadCapability::new(pool.clone(), kind))
            .unwrap_or_else(|error| panic!("invalid {} capability: {error}", kind.operation_key()));
    }
    registry
        .register(LibrarySettingsCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Library settings capability: {error}"));
    registry
        .register(LibraryReferencesCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Library references capability: {error}"));
    registry
        .register(LibraryCopiesCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Library copies-list capability: {error}"));
    for kind in [
        LibraryListKind::Titles,
        LibraryListKind::Members,
        LibraryListKind::Loans,
        LibraryListKind::Holds,
        LibraryListKind::Fines,
    ] {
        registry
            .register(LibraryListCapability::new(pool.clone(), kind))
            .unwrap_or_else(|error| panic!("invalid {} capability: {error}", kind.operation_key()));
    }
    for kind in [
        LibraryReadKind::Title,
        LibraryReadKind::Copy,
        LibraryReadKind::Member,
        LibraryReadKind::Loan,
        LibraryReadKind::Hold,
        LibraryReadKind::Fine,
    ] {
        registry
            .register(LibraryReadCapability::new(pool.clone(), kind))
            .unwrap_or_else(|error| panic!("invalid {} capability: {error}", kind.operation_key()));
    }
    registry
        .register(HealthReferencesCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Health references capability: {error}"));
    for kind in [
        HealthListKind::Patients,
        HealthListKind::Visits,
        HealthListKind::MedicationPlans,
        HealthListKind::MedicationAdministrations,
        HealthListKind::FollowUps,
    ] {
        registry
            .register(HealthListCapability::new(pool.clone(), kind))
            .unwrap_or_else(|error| panic!("invalid {} capability: {error}", kind.operation_key()));
    }
    for kind in [HealthReadKind::Patient, HealthReadKind::Visit] {
        registry
            .register(HealthReadCapability::new(pool.clone(), kind))
            .unwrap_or_else(|error| panic!("invalid {} capability: {error}", kind.operation_key()));
    }
    registry
        .register(HostelReferencesCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Hostel references capability: {error}"));
    for kind in [
        HostelListKind::Residences,
        HostelListKind::Rooms,
        HostelListKind::Allocations,
        HostelListKind::PastoralRecords,
    ] {
        registry
            .register(HostelListCapability::new(pool.clone(), kind))
            .unwrap_or_else(|error| panic!("invalid {} capability: {error}", kind.operation_key()));
    }
    for kind in [
        HostelReadKind::Residence,
        HostelReadKind::Room,
        HostelReadKind::Allocation,
        HostelReadKind::PastoralRecord,
    ] {
        registry
            .register(HostelReadCapability::new(pool.clone(), kind))
            .unwrap_or_else(|error| panic!("invalid {} capability: {error}", kind.operation_key()));
    }
    registry
        .register(AllocationPreviewCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Hostel allocation-preview capability: {error}"));
    registry
        .register(TransferPreviewCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Hostel transfer-preview capability: {error}"));
    for kind in [
        RegistryReadKind::NumberingPolicy,
        RegistryReadKind::SeriesList,
        RegistryReadKind::SeriesRead,
        RegistryReadKind::FilesList,
        RegistryReadKind::FileRead,
        RegistryReadKind::FileActivity,
        RegistryReadKind::RetentionDue,
        RegistryReadKind::ReviewsList,
        RegistryReadKind::ReviewRead,
    ] {
        registry
            .register(RegistryReadCapability::new(pool.clone(), kind))
            .unwrap_or_else(|error| panic!("invalid {} capability: {error}", kind.operation_key()));
    }
    for kind in [
        InternalAuditReadKind::NumberingPolicy,
        InternalAuditReadKind::PlansList,
        InternalAuditReadKind::PlanRead,
        InternalAuditReadKind::AuditorCandidates,
        InternalAuditReadKind::EngagementsList,
        InternalAuditReadKind::EngagementRead,
        InternalAuditReadKind::EvidenceList,
        InternalAuditReadKind::FindingsList,
        InternalAuditReadKind::FindingRead,
    ] {
        registry
            .register(InternalAuditReadCapability::new(pool.clone(), kind))
            .unwrap_or_else(|error| panic!("invalid {} capability: {error}", kind.operation_key()));
    }
    registry
        .register(TimetableConfigurationCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Timetabling configuration capability: {error}"));
    registry
        .register(TimetableRunsListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Timetabling runs-list capability: {error}"));
    registry
        .register(TimetableRunReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Timetabling run-read capability: {error}"));
    registry
        .register(LatestTimetableRunCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid Timetabling latest-run capability: {error}"));
    registry
        .register(AiRoutesListCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid AI routes-list capability: {error}"));
    registry
        .register(AiRouteReadCapability::new(pool.clone()))
        .unwrap_or_else(|error| panic!("invalid AI route-read capability: {error}"));
    let mut routing_capabilities = routing_capability_options(&registry);
    for (key, label) in [
        (
            "administration.ai_routing.routes.options",
            "List Agent routing options",
        ),
        (
            "administration.ai_routing.routes.resolve",
            "Resolve an Agent route",
        ),
    ] {
        routing_capabilities.push(
            routing_capability_option(key, 1, label)
                .unwrap_or_else(|| panic!("invalid routing capability operation: {key}")),
        );
    }
    sort_capability_options(&mut routing_capabilities);
    registry
        .register(AiRoutingOptionsCapability::new(
            pool.clone(),
            routing_capabilities.clone(),
        ))
        .unwrap_or_else(|error| panic!("invalid AI routing-options capability: {error}"));
    registry
        .register(AiRouteResolveCapability::new(pool, routing_capabilities))
        .unwrap_or_else(|error| panic!("invalid AI route-resolution capability: {error}"));
    registry
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        sync::{Arc, Mutex},
    };

    use async_trait::async_trait;
    use cp_agent::{
        AuthenticatedAgentPrincipal, AuthorityLoadError, AuthorityLoader, AuthorizedRecordScope,
        BrokerAuditError, BrokerAuditRecord, BrokerAuditSink, BrokerErrorCode, CapabilityBroker,
        CapabilityCall, CapabilityCallId, CapabilityExecutionProof, CapabilityResult,
        CapabilityWorkerLease, CurrentAuthority, DurabilityProofRejected,
        PreparedCapabilityCallFacts, PreparedCapabilityCallVerifier, RecordScopeAuthorizer,
        RecordScopeDenied,
    };
    use cp_audit::RequestContext;
    use cp_common::{
        AccessContext, EntitlementSnapshot, LeaseLifecycle, ModuleEntitlementState,
        ProductOperation,
    };
    use serde_json::json;
    use sqlx::postgres::PgPoolOptions;
    use uuid::Uuid;

    use crate::config::LicenseConfig;

    use super::build_capability_registry;

    struct TestAuthorityLoader(CurrentAuthority);

    #[async_trait]
    impl AuthorityLoader for TestAuthorityLoader {
        async fn load(
            &self,
            _principal: AuthenticatedAgentPrincipal,
        ) -> Result<CurrentAuthority, AuthorityLoadError> {
            Ok(self.0.clone())
        }
    }

    struct TenantWideScope;

    #[async_trait]
    impl RecordScopeAuthorizer for TenantWideScope {
        async fn authorize(
            &self,
            _principal: AuthenticatedAgentPrincipal,
            _authority: &CurrentAuthority,
            _operation: &ProductOperation,
            _scope: &cp_agent::CapabilityScope,
        ) -> Result<AuthorizedRecordScope, RecordScopeDenied> {
            Ok(AuthorizedRecordScope::granted())
        }
    }

    struct TestAudit;

    fn call_id() -> CapabilityCallId {
        CapabilityCallId::from_trusted_runtime(Uuid::new_v4())
    }

    fn run_id() -> Uuid {
        Uuid::from_u128(0x410)
    }

    fn lease_token() -> Uuid {
        Uuid::from_u128(0x420)
    }

    fn reservation_id() -> Uuid {
        Uuid::from_u128(0x430)
    }

    struct TestDurabilityVerifier {
        consumed: Mutex<BTreeSet<CapabilityCallId>>,
    }

    #[async_trait]
    impl PreparedCapabilityCallVerifier for TestDurabilityVerifier {
        async fn verify_and_consume(
            &self,
            principal: AuthenticatedAgentPrincipal,
            facts: &PreparedCapabilityCallFacts,
            proof: &CapabilityExecutionProof,
        ) -> Result<(), DurabilityProofRejected> {
            let exact = facts.agent_run_id().is_some_and(|persisted_run_id| {
                proof.tenant_id() == principal.tenant_id()
                    && proof.user_id() == principal.user_id()
                    && proof.capability_call_id() == facts.capability_call_id()
                    && proof.run_id() == persisted_run_id
                    && proof.worker_id() == "app-agent-test-worker"
                    && proof.lease_token() == lease_token()
                    && proof.fence_version() == 1
                    && proof.usage_reservation_id() == reservation_id()
                    && facts.operation_key() == facts.key().as_str()
                    && !facts.module_key().is_empty()
                    && !facts.required_permission().is_empty()
                    && facts.input_binding_sha256() != [0; 32]
            });
            if !exact {
                return Err(DurabilityProofRejected);
            }
            let mut consumed = self
                .consumed
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !consumed.insert(facts.capability_call_id()) {
                return Err(DurabilityProofRejected);
            }
            Ok(())
        }
    }

    fn test_verifier() -> Arc<TestDurabilityVerifier> {
        Arc::new(TestDurabilityVerifier {
            consumed: Mutex::new(BTreeSet::new()),
        })
    }

    #[async_trait]
    trait TestInvoke {
        async fn invoke(
            &self,
            principal: AuthenticatedAgentPrincipal,
            capability_call_id: CapabilityCallId,
            call: CapabilityCall,
        ) -> Result<CapabilityResult, cp_agent::BrokerError>;
    }

    #[async_trait]
    impl TestInvoke for CapabilityBroker {
        async fn invoke(
            &self,
            principal: AuthenticatedAgentPrincipal,
            capability_call_id: CapabilityCallId,
            call: CapabilityCall,
        ) -> Result<CapabilityResult, cp_agent::BrokerError> {
            let call = call.with_agent_run_id(run_id());
            let mut prepared = self.prepare(principal, capability_call_id, call).await?;
            let proof = CapabilityExecutionProof::parse(
                principal,
                capability_call_id,
                run_id(),
                CapabilityWorkerLease::parse("app-agent-test-worker", lease_token(), 1)
                    .unwrap_or_else(|_| unreachable!()),
                reservation_id(),
            )
            .unwrap_or_else(|_| unreachable!());
            self.execute_prepared(&mut prepared, proof).await
        }
    }

    #[async_trait]
    impl BrokerAuditSink for TestAudit {
        async fn record(&self, _record: BrokerAuditRecord) -> Result<(), BrokerAuditError> {
            Ok(())
        }
    }

    fn authority() -> CurrentAuthority {
        CurrentAuthority::from_reloaded_access(AccessContext {
            role_keys: vec!["campus_owner".to_string()],
            permissions: vec![
                "agent:run".to_string(),
                "administration:view".to_string(),
                "roles:view".to_string(),
                "users:view".to_string(),
                "school_settings:view".to_string(),
                "licensing:view".to_string(),
                "sis:view".to_string(),
                "academics:view".to_string(),
                "hr_payroll:view".to_string(),
                "fleet:view".to_string(),
                "timetabling:view".to_string(),
                "finance:view".to_string(),
                "fees:view".to_string(),
                "procurement:view".to_string(),
                "procurement:create".to_string(),
                "assets_inventory:view".to_string(),
                "assets_inventory:receive".to_string(),
                "ai_providers:view".to_string(),
                "ai_routing:view".to_string(),
            ],
            enabled_modules: vec![
                "agent".to_string(),
                "administration".to_string(),
                "sis".to_string(),
                "academics".to_string(),
                "hr_payroll".to_string(),
                "fleet".to_string(),
                "timetabling".to_string(),
                "finance".to_string(),
                "fees".to_string(),
                "procurement".to_string(),
                "assets_inventory".to_string(),
            ],
            entitlements: EntitlementSnapshot::new(
                LeaseLifecycle::Active,
                [
                    ("agent".to_string(), ModuleEntitlementState::Enabled),
                    (
                        "administration".to_string(),
                        ModuleEntitlementState::Enabled,
                    ),
                    ("sis".to_string(), ModuleEntitlementState::Enabled),
                    ("academics".to_string(), ModuleEntitlementState::Enabled),
                    ("hr_payroll".to_string(), ModuleEntitlementState::Enabled),
                    ("fleet".to_string(), ModuleEntitlementState::Enabled),
                    ("timetabling".to_string(), ModuleEntitlementState::Enabled),
                    ("finance".to_string(), ModuleEntitlementState::Enabled),
                    ("fees".to_string(), ModuleEntitlementState::Enabled),
                    ("procurement".to_string(), ModuleEntitlementState::Enabled),
                    (
                        "assets_inventory".to_string(),
                        ModuleEntitlementState::Enabled,
                    ),
                ],
                Vec::<String>::new(),
            )
            .unwrap_or_else(|_| unreachable!()),
        })
    }

    fn license_config() -> LicenseConfig {
        LicenseConfig {
            trusted_public_keys: BTreeMap::new(),
            issuer: "campus-pilot-control-plane".to_string(),
            audience: "campus-pilot".to_string(),
            control_plane_url: Some("https://licensing.invalid".to_string()),
            credential_key_base64: Some("test-key".to_string()),
            installation_name: "Test installation".to_string(),
        }
    }

    #[tokio::test]
    async fn production_registry_executes_the_real_administration_catalogue_read() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
            .unwrap_or_else(|_| unreachable!());
        let registry = build_capability_registry(pool, license_config());
        let keys = registry
            .descriptors()
            .into_iter()
            .map(|descriptor| descriptor.key().as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            keys,
            vec![
                "academics.academic_years.list",
                "academics.academic_years.read",
                "academics.assessment_components.list",
                "academics.assessment_components.read",
                "academics.assessment_cycles.list",
                "academics.assessment_cycles.read",
                "academics.classes.list",
                "academics.classes.read",
                "academics.grade_levels.list",
                "academics.grade_levels.read",
                "academics.gradebook.mark_sheets.list",
                "academics.gradebook.mark_sheets.read",
                "academics.gradebook.references.read",
                "academics.reporting.grading_schemes.list",
                "academics.reporting.grading_schemes.read",
                "academics.reporting.references.read",
                "academics.reporting.report_batches.list",
                "academics.reporting.report_batches.read",
                "academics.reporting.transcripts.read",
                "academics.subjects.list",
                "academics.subjects.read",
                "academics.teacher_candidates.list",
                "academics.teachers.list",
                "academics.teachers.read",
                "academics.teaching_assignments.list",
                "academics.teaching_assignments.read",
                "academics.terms.list",
                "academics.terms.read",
                "administration.ai_providers.catalog.list",
                "administration.ai_providers.connections.list",
                "administration.ai_providers.connections.read",
                "administration.ai_providers.models.list",
                "administration.ai_routing.routes.list",
                "administration.ai_routing.routes.options",
                "administration.ai_routing.routes.read",
                "administration.ai_routing.routes.resolve",
                "administration.catalog.read",
                "administration.licensing.read",
                "administration.modules.list",
                "administration.roles.list",
                "administration.roles.read",
                "administration.school_settings.read",
                "administration.users.list",
                "administration.users.read",
                "assets_inventory.department_candidates.list",
                "assets_inventory.goods_receipt_allocations.list",
                "assets_inventory.items.list",
                "assets_inventory.items.read",
                "assets_inventory.requester_candidates.list",
                "assets_inventory.stock_balances.list",
                "assets_inventory.stock_movements.list",
                "assets_inventory.stock_movements.read",
                "assets_inventory.stock_requests.fulfilment_preview.read",
                "assets_inventory.stock_requests.list",
                "assets_inventory.stock_requests.read",
                "assets_inventory.stores.list",
                "assets_inventory.stores.read",
                "attendance.learners.history.read",
                "attendance.references.read",
                "attendance.registers.list",
                "attendance.registers.read",
                "document_registry.disposition_reviews.list",
                "document_registry.disposition_reviews.read",
                "document_registry.files.activity.list",
                "document_registry.files.list",
                "document_registry.files.read",
                "document_registry.numbering_policy.read",
                "document_registry.retention_due.list",
                "document_registry.series.list",
                "document_registry.series.read",
                "fees.billing_accounts.list",
                "fees.billing_accounts.read",
                "fees.fee_structures.list",
                "fees.fee_structures.read",
                "fees.imports.list",
                "fees.imports.preview.read",
                "fees.imports.read",
                "fees.invoices.list",
                "fees.invoices.read",
                "fees.learner_candidates.list",
                "fees.reference_data.read",
                "finance.accounting_periods.list",
                "finance.accounts.list",
                "finance.accounts.read",
                "finance.currencies.list",
                "finance.currencies.read",
                "finance.fiscal_years.list",
                "finance.fiscal_years.read",
                "finance.journals.list",
                "finance.journals.read",
                "finance.journals.validation.read",
                "finance.posting_requests.list",
                "finance.posting_requests.read",
                "fleet.driver_candidates.list",
                "fleet.drivers.list",
                "fleet.drivers.read",
                "fleet.vehicle_logs.list",
                "fleet.vehicle_logs.read",
                "fleet.vehicles.list",
                "fleet.vehicles.read",
                "health.follow_ups.list",
                "health.medication_administrations.list",
                "health.medication_plans.list",
                "health.patients.list",
                "health.patients.read",
                "health.references.read",
                "health.visits.list",
                "health.visits.read",
                "hostel.allocations.list",
                "hostel.allocations.preview",
                "hostel.allocations.read",
                "hostel.allocations.transfer_preview",
                "hostel.pastoral_records.list",
                "hostel.pastoral_records.read",
                "hostel.references.read",
                "hostel.residences.list",
                "hostel.residences.read",
                "hostel.rooms.list",
                "hostel.rooms.read",
                "hr_payroll.availability.list",
                "hr_payroll.availability.read",
                "hr_payroll.departments.list",
                "hr_payroll.departments.read",
                "hr_payroll.employees.list",
                "hr_payroll.employees.read",
                "hr_payroll.employment_engagements.list",
                "hr_payroll.employment_engagements.read",
                "hr_payroll.imports.list",
                "hr_payroll.imports.preview.read",
                "hr_payroll.imports.read",
                "hr_payroll.positions.list",
                "hr_payroll.positions.read",
                "internal_audit.auditor_candidates.list",
                "internal_audit.engagements.list",
                "internal_audit.engagements.read",
                "internal_audit.evidence.list",
                "internal_audit.findings.list",
                "internal_audit.findings.read",
                "internal_audit.numbering_policy.read",
                "internal_audit.plans.list",
                "internal_audit.plans.read",
                "learning.references.read",
                "learning.resource_files.list",
                "learning.settings.read",
                "learning.spaces.list",
                "learning.spaces.read",
                "library.copies.list",
                "library.copies.read",
                "library.fines.list",
                "library.fines.read",
                "library.holds.list",
                "library.holds.read",
                "library.loans.list",
                "library.loans.read",
                "library.members.list",
                "library.members.read",
                "library.references.read",
                "library.settings.read",
                "library.titles.list",
                "library.titles.read",
                "messaging.announcements.audience_preview.read",
                "messaging.announcements.list",
                "messaging.announcements.read",
                "messaging.deliveries.list",
                "messaging.inbox.list",
                "messaging.inbox.read",
                "messaging.references.read",
                "procurement.goods_receipts.list",
                "procurement.goods_receipts.read",
                "procurement.purchase_orders.list",
                "procurement.purchase_orders.read",
                "procurement.reference_data.read",
                "procurement.requester_candidates.list",
                "procurement.requisitions.list",
                "procurement.requisitions.read",
                "procurement.suppliers.list",
                "procurement.suppliers.read",
                "sis.account_candidates.list",
                "sis.applications.list",
                "sis.applications.read",
                "sis.enrolments.list",
                "sis.enrolments.read",
                "sis.guardian_relationships.list",
                "sis.guardian_relationships.read",
                "sis.guardians.list",
                "sis.guardians.read",
                "sis.imports.list",
                "sis.imports.preview.read",
                "sis.imports.read",
                "sis.learner_numbering.read",
                "sis.learners.list",
                "sis.learners.read",
                "student_support.actions.list",
                "student_support.cases.list",
                "student_support.cases.read",
                "timetabling.configuration.read",
                "timetabling.runs.list",
                "timetabling.runs.read",
                "timetabling.runs.read_latest",
                "transport.riders.list",
                "transport.routes.list",
                "transport.routes.read",
                "transport.runs.list",
                "transport.runs.read"
            ]
        );
        let broker = CapabilityBroker::new(
            registry,
            Arc::new(TestAuthorityLoader(authority())),
            Arc::new(TenantWideScope),
            test_verifier(),
            Arc::new(TestAudit),
        );
        let principal =
            AuthenticatedAgentPrincipal::from_authenticated_request(Uuid::new_v4(), Uuid::new_v4());
        let request_context = RequestContext::from_ids(Uuid::new_v4(), Uuid::new_v4());
        let result = broker
            .invoke(
                principal,
                call_id(),
                CapabilityCall::parse("administration.catalog.read", 1, json!({}), request_context)
                    .unwrap_or_else(|_| unreachable!()),
            )
            .await
            .unwrap_or_else(|_| unreachable!());

        assert_eq!(
            result.content()["modules"].as_array().map(Vec::len),
            Some(21)
        );
        assert!(result.content()["administration_permissions"].is_array());
    }

    #[tokio::test]
    async fn production_modules_capability_returns_a_safe_domain_failure() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
            .unwrap_or_else(|_| unreachable!());
        pool.close().await;
        let broker = CapabilityBroker::new(
            build_capability_registry(pool, license_config()),
            Arc::new(TestAuthorityLoader(authority())),
            Arc::new(TenantWideScope),
            test_verifier(),
            Arc::new(TestAudit),
        );
        let request_context = RequestContext::from_ids(Uuid::new_v4(), Uuid::new_v4());
        let error = broker
            .invoke(
                AuthenticatedAgentPrincipal::from_authenticated_request(
                    Uuid::new_v4(),
                    Uuid::new_v4(),
                ),
                call_id(),
                CapabilityCall::parse("administration.modules.list", 1, json!({}), request_context)
                    .unwrap_or_else(|_| unreachable!()),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());

        assert_eq!(error.code(), BrokerErrorCode::ExecutionFailed);
        assert_eq!(error.request_context(), request_context);
        assert_eq!(
            error.safe_message(),
            "The capability could not be completed."
        );
    }

    #[tokio::test]
    async fn production_role_and_user_capabilities_are_typed_and_fail_safely() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
            .unwrap_or_else(|_| unreachable!());
        pool.close().await;
        let broker = CapabilityBroker::new(
            build_capability_registry(pool, license_config()),
            Arc::new(TestAuthorityLoader(authority())),
            Arc::new(TenantWideScope),
            test_verifier(),
            Arc::new(TestAudit),
        );
        let principal =
            AuthenticatedAgentPrincipal::from_authenticated_request(Uuid::new_v4(), Uuid::new_v4());
        let request_context = RequestContext::from_ids(Uuid::new_v4(), Uuid::new_v4());
        for (key, input) in [
            ("administration.roles.list", json!({})),
            (
                "administration.roles.read",
                json!({ "role_id": Uuid::new_v4() }),
            ),
            (
                "administration.users.list",
                json!({ "status": "active", "sort": "email" }),
            ),
            (
                "administration.users.read",
                json!({ "account_id": Uuid::new_v4() }),
            ),
        ] {
            let error = broker
                .invoke(
                    principal,
                    call_id(),
                    CapabilityCall::parse(key, 1, input, request_context)
                        .unwrap_or_else(|_| unreachable!()),
                )
                .await
                .err()
                .unwrap_or_else(|| unreachable!());
            assert_eq!(
                error.code(),
                BrokerErrorCode::ExecutionFailed,
                "unexpected broker result for {key}"
            );
        }

        let invalid = broker
            .invoke(
                principal,
                call_id(),
                CapabilityCall::parse(
                    "administration.users.list",
                    1,
                    json!({ "status": "maybe" }),
                    request_context,
                )
                .unwrap_or_else(|_| unreachable!()),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(invalid.code(), BrokerErrorCode::InvalidInput);
    }

    #[tokio::test]
    async fn production_school_and_licensing_capabilities_fail_safely() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
            .unwrap_or_else(|_| unreachable!());
        pool.close().await;
        let broker = CapabilityBroker::new(
            build_capability_registry(pool, license_config()),
            Arc::new(TestAuthorityLoader(authority())),
            Arc::new(TenantWideScope),
            test_verifier(),
            Arc::new(TestAudit),
        );
        let principal =
            AuthenticatedAgentPrincipal::from_authenticated_request(Uuid::new_v4(), Uuid::new_v4());
        for key in [
            "administration.school_settings.read",
            "administration.licensing.read",
        ] {
            let error = broker
                .invoke(
                    principal,
                    call_id(),
                    CapabilityCall::parse(
                        key,
                        1,
                        json!({}),
                        RequestContext::from_ids(Uuid::new_v4(), Uuid::new_v4()),
                    )
                    .unwrap_or_else(|_| unreachable!()),
                )
                .await
                .err()
                .unwrap_or_else(|| unreachable!());
            assert_eq!(error.code(), BrokerErrorCode::ExecutionFailed, "{key}");
        }
    }

    #[tokio::test]
    async fn production_ai_provider_reads_are_reduced_typed_and_fail_safely() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
            .unwrap_or_else(|_| unreachable!());
        pool.close().await;
        let broker = CapabilityBroker::new(
            build_capability_registry(pool, license_config()),
            Arc::new(TestAuthorityLoader(authority())),
            Arc::new(TenantWideScope),
            test_verifier(),
            Arc::new(TestAudit),
        );
        let principal =
            AuthenticatedAgentPrincipal::from_authenticated_request(Uuid::new_v4(), Uuid::new_v4());
        let request_context = RequestContext::from_ids(Uuid::new_v4(), Uuid::new_v4());
        let connection_id = Uuid::new_v4();
        for (key, input) in [
            ("administration.ai_providers.connections.list", json!({})),
            (
                "administration.ai_providers.connections.read",
                json!({ "connection_id": connection_id }),
            ),
            (
                "administration.ai_providers.models.list",
                json!({ "connection_id": connection_id }),
            ),
        ] {
            let error = broker
                .invoke(
                    principal,
                    call_id(),
                    CapabilityCall::parse(key, 1, input, request_context)
                        .unwrap_or_else(|_| unreachable!()),
                )
                .await
                .err()
                .unwrap_or_else(|| unreachable!());
            assert_eq!(error.code(), BrokerErrorCode::ExecutionFailed, "{key}");
            assert_eq!(
                error.safe_message(),
                "The capability could not be completed.",
                "unsafe failure message for {key}"
            );
        }

        let invalid = broker
            .invoke(
                principal,
                call_id(),
                CapabilityCall::parse(
                    "administration.ai_providers.connections.list",
                    1,
                    json!({ "tenant_id": Uuid::new_v4() }),
                    request_context,
                )
                .unwrap_or_else(|_| unreachable!()),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(invalid.code(), BrokerErrorCode::InvalidInput);
    }

    #[tokio::test]
    async fn production_ai_routing_reads_are_typed_and_fail_safely() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
            .unwrap_or_else(|_| unreachable!());
        pool.close().await;
        let broker = CapabilityBroker::new(
            build_capability_registry(pool, license_config()),
            Arc::new(TestAuthorityLoader(authority())),
            Arc::new(TenantWideScope),
            test_verifier(),
            Arc::new(TestAudit),
        );
        let principal =
            AuthenticatedAgentPrincipal::from_authenticated_request(Uuid::new_v4(), Uuid::new_v4());
        let request_context = RequestContext::from_ids(Uuid::new_v4(), Uuid::new_v4());
        for (key, input) in [
            ("administration.ai_routing.routes.list", json!({})),
            ("administration.ai_routing.routes.options", json!({})),
            (
                "administration.ai_routing.routes.read",
                json!({ "route_set_id": Uuid::new_v4() }),
            ),
            (
                "administration.ai_routing.routes.resolve",
                json!({
                    "task_class": "module_read_reporting",
                    "module_key": "finance",
                    "operation_class": "read",
                    "capability_key": "finance.journals.list",
                    "capability_version": 1,
                    "requires_tools": true
                }),
            ),
        ] {
            let error = broker
                .invoke(
                    principal,
                    call_id(),
                    CapabilityCall::parse(key, 1, input, request_context)
                        .unwrap_or_else(|_| unreachable!()),
                )
                .await
                .err()
                .unwrap_or_else(|| unreachable!());
            assert_eq!(error.code(), BrokerErrorCode::ExecutionFailed, "{key}");
            assert_eq!(
                error.safe_message(),
                "The capability could not be completed.",
                "unsafe failure message for {key}"
            );
        }

        let invalid = broker
            .invoke(
                principal,
                call_id(),
                CapabilityCall::parse(
                    "administration.ai_routing.routes.list",
                    1,
                    json!({ "tenant_id": Uuid::new_v4() }),
                    request_context,
                )
                .unwrap_or_else(|_| unreachable!()),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(invalid.code(), BrokerErrorCode::InvalidInput);
    }

    #[tokio::test]
    async fn production_procurement_capabilities_are_typed_and_fail_safely() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
            .unwrap_or_else(|_| unreachable!());
        pool.close().await;
        let broker = CapabilityBroker::new(
            build_capability_registry(pool, license_config()),
            Arc::new(TestAuthorityLoader(authority())),
            Arc::new(TenantWideScope),
            test_verifier(),
            Arc::new(TestAudit),
        );
        let principal =
            AuthenticatedAgentPrincipal::from_authenticated_request(Uuid::new_v4(), Uuid::new_v4());
        let request_context = RequestContext::from_ids(Uuid::new_v4(), Uuid::new_v4());
        for (key, input) in [
            ("procurement.reference_data.read", json!({})),
            (
                "procurement.requester_candidates.list",
                json!({ "search": "sam" }),
            ),
            ("procurement.suppliers.list", json!({ "page": 1 })),
            (
                "procurement.suppliers.read",
                json!({ "record_id": Uuid::new_v4() }),
            ),
            (
                "procurement.requisitions.list",
                json!({ "status": "submitted" }),
            ),
            (
                "procurement.requisitions.read",
                json!({ "record_id": Uuid::new_v4() }),
            ),
            (
                "procurement.purchase_orders.list",
                json!({ "supplier_id": Uuid::new_v4() }),
            ),
            (
                "procurement.purchase_orders.read",
                json!({ "record_id": Uuid::new_v4() }),
            ),
            (
                "procurement.goods_receipts.list",
                json!({ "purchase_order_id": Uuid::new_v4() }),
            ),
            (
                "procurement.goods_receipts.read",
                json!({ "record_id": Uuid::new_v4() }),
            ),
        ] {
            let error = broker
                .invoke(
                    principal,
                    call_id(),
                    CapabilityCall::parse(key, 1, input, request_context)
                        .unwrap_or_else(|_| unreachable!()),
                )
                .await
                .err()
                .unwrap_or_else(|| unreachable!());
            assert_eq!(
                error.code(),
                BrokerErrorCode::ExecutionFailed,
                "unexpected broker result for {key}"
            );
            assert_eq!(
                error.safe_message(),
                "The capability could not be completed.",
                "unsafe failure message for {key}"
            );
        }

        let invalid = broker
            .invoke(
                principal,
                call_id(),
                CapabilityCall::parse(
                    "procurement.reference_data.read",
                    1,
                    json!({ "tenant_id": Uuid::new_v4() }),
                    request_context,
                )
                .unwrap_or_else(|_| unreachable!()),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(invalid.code(), BrokerErrorCode::InvalidInput);

        let irrelevant_filter = broker
            .invoke(
                principal,
                call_id(),
                CapabilityCall::parse(
                    "procurement.suppliers.list",
                    1,
                    json!({ "requester_employee_id": Uuid::new_v4() }),
                    request_context,
                )
                .unwrap_or_else(|_| unreachable!()),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(irrelevant_filter.code(), BrokerErrorCode::InvalidInput);
    }

    #[tokio::test]
    async fn production_assets_inventory_capabilities_are_typed_and_fail_safely() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
            .unwrap_or_else(|_| unreachable!());
        pool.close().await;
        let broker = CapabilityBroker::new(
            build_capability_registry(pool, license_config()),
            Arc::new(TestAuthorityLoader(authority())),
            Arc::new(TenantWideScope),
            test_verifier(),
            Arc::new(TestAudit),
        );
        let principal =
            AuthenticatedAgentPrincipal::from_authenticated_request(Uuid::new_v4(), Uuid::new_v4());
        let request_context = RequestContext::from_ids(Uuid::new_v4(), Uuid::new_v4());
        for (key, input) in [
            ("assets_inventory.items.list", json!({ "page": 1 })),
            (
                "assets_inventory.items.read",
                json!({ "record_id": Uuid::new_v4() }),
            ),
            (
                "assets_inventory.stores.list",
                json!({ "status": "active" }),
            ),
            (
                "assets_inventory.stores.read",
                json!({ "record_id": Uuid::new_v4() }),
            ),
            (
                "assets_inventory.stock_balances.list",
                json!({ "item_id": Uuid::new_v4() }),
            ),
            (
                "assets_inventory.stock_movements.list",
                json!({ "kind": "transfer", "store_id": Uuid::new_v4() }),
            ),
            (
                "assets_inventory.stock_movements.read",
                json!({ "record_id": Uuid::new_v4() }),
            ),
            (
                "assets_inventory.goods_receipt_allocations.list",
                json!({ "goods_receipt_id": Uuid::new_v4() }),
            ),
        ] {
            let error = broker
                .invoke(
                    principal,
                    call_id(),
                    CapabilityCall::parse(key, 1, input, request_context)
                        .unwrap_or_else(|_| unreachable!()),
                )
                .await
                .err()
                .unwrap_or_else(|| unreachable!());
            assert_eq!(error.code(), BrokerErrorCode::ExecutionFailed, "{key}");
            assert_eq!(
                error.safe_message(),
                "The capability could not be completed.",
                "unsafe failure message for {key}"
            );
        }

        let invalid = broker
            .invoke(
                principal,
                call_id(),
                CapabilityCall::parse(
                    "assets_inventory.items.list",
                    1,
                    json!({ "tenant_id": Uuid::new_v4() }),
                    request_context,
                )
                .unwrap_or_else(|_| unreachable!()),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(invalid.code(), BrokerErrorCode::InvalidInput);

        let invalid_status = broker
            .invoke(
                principal,
                call_id(),
                CapabilityCall::parse(
                    "assets_inventory.stores.list",
                    1,
                    json!({ "status": "deleted" }),
                    request_context,
                )
                .unwrap_or_else(|_| unreachable!()),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(invalid_status.code(), BrokerErrorCode::InvalidInput);

        let invalid_kind = broker
            .invoke(
                principal,
                call_id(),
                CapabilityCall::parse(
                    "assets_inventory.stock_movements.list",
                    1,
                    json!({ "kind": "invented" }),
                    request_context,
                )
                .unwrap_or_else(|_| unreachable!()),
            )
            .await
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(invalid_kind.code(), BrokerErrorCode::InvalidInput);
    }
}
