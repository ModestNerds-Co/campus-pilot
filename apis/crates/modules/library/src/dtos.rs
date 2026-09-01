//! Public Library API contracts and closed workflow values.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryAccessScope {
    Campus,
    SelfFor(Uuid),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BorrowerKind {
    Learner,
    Employee,
}
impl BorrowerKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Learner => "learner",
            Self::Employee => "employee",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopyCondition {
    New,
    Good,
    Worn,
    Damaged,
}
impl CopyCondition {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Good => "good",
            Self::Worn => "worn",
            Self::Damaged => "damaged",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopyStatus {
    Available,
    OnLoan,
    Reserved,
    Lost,
    Repair,
    Retired,
}
impl CopyStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::OnLoan => "on_loan",
            Self::Reserved => "reserved",
            Self::Lost => "lost",
            Self::Repair => "repair",
            Self::Retired => "retired",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MembershipStatus {
    Active,
    Suspended,
    Closed,
}
impl MembershipStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Suspended => "suspended",
            Self::Closed => "closed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FineKind {
    Overdue,
    Replacement,
}
impl FineKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Overdue => "overdue",
            Self::Replacement => "replacement",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DirectoryQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CopyListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BorrowingListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub overdue_only: Option<bool>,
    pub membership_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateLibrarySettingsRequest {
    #[validate(length(min = 1, max = 16))]
    pub accession_prefix: String,
    #[validate(range(min = 1, max = 100000000))]
    pub accession_next_sequence: i64,
    #[validate(range(min = 1, max = 8))]
    pub accession_padding: i16,
    #[validate(range(min = 1, max = 365))]
    pub learner_loan_days: i16,
    #[validate(range(min = 1, max = 365))]
    pub employee_loan_days: i16,
    #[validate(range(min = 1, max = 100))]
    pub default_loan_limit: i16,
    #[validate(range(min = 0, max = 20))]
    pub maximum_renewals: i16,
    pub fine_currency_id: Option<Uuid>,
    #[validate(range(min = 0_i64, max = 9_000_000_000_000_000_i64))]
    pub overdue_fine_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LibrarySettingsResponse {
    pub accession_prefix: String,
    pub accession_next_sequence: i64,
    pub accession_padding: i16,
    pub next_accession_preview: String,
    pub learner_loan_days: i16,
    pub employee_loan_days: i16,
    pub default_loan_limit: i16,
    pub maximum_renewals: i16,
    pub fine_currency_id: Option<Uuid>,
    pub fine_currency_code: Option<String>,
    pub fine_currency_minor_units: Option<i16>,
    pub overdue_fine_minor: i64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CurrencyReference {
    pub id: Uuid,
    pub code: String,
    pub minor_units: i16,
    pub is_reporting: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct BillingAccountReference {
    pub id: Uuid,
    pub learner_id: Uuid,
    pub learner_number: String,
    pub learner_name: String,
    pub account_number: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BorrowerCandidate {
    pub kind: BorrowerKind,
    pub id: Uuid,
    pub number: String,
    pub display_name: String,
    pub source_status: String,
    pub account_linked: bool,
    pub already_member: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LibraryReferenceData {
    pub learners: Vec<BorrowerCandidate>,
    pub employees: Vec<BorrowerCandidate>,
    pub currencies: Vec<CurrencyReference>,
    pub billing_accounts: Vec<BillingAccountReference>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateTitleRequest {
    #[validate(length(min = 1, max = 300))]
    pub title: String,
    #[validate(length(max = 300))]
    pub subtitle: Option<String>,
    #[validate(length(min = 1, max = 20))]
    pub authors: Vec<String>,
    #[validate(length(min = 10, max = 20))]
    pub isbn: Option<String>,
    #[validate(length(max = 200))]
    pub publisher: Option<String>,
    #[validate(range(min = 1000, max = 9999))]
    pub publication_year: Option<i16>,
    #[validate(length(max = 80))]
    pub edition: Option<String>,
    #[validate(length(min = 3, max = 3))]
    pub language_code: String,
    #[validate(length(max = 160))]
    pub subject: Option<String>,
    #[validate(range(min = 1_i64, max = 9_000_000_000_000_000_i64))]
    pub replacement_cost_minor: Option<i64>,
    pub replacement_currency_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateTitleRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 300))]
    pub title: String,
    #[validate(length(max = 300))]
    pub subtitle: Option<String>,
    #[validate(length(min = 1, max = 20))]
    pub authors: Vec<String>,
    #[validate(length(min = 10, max = 20))]
    pub isbn: Option<String>,
    #[validate(length(max = 200))]
    pub publisher: Option<String>,
    #[validate(range(min = 1000, max = 9999))]
    pub publication_year: Option<i16>,
    #[validate(length(max = 80))]
    pub edition: Option<String>,
    #[validate(length(min = 3, max = 3))]
    pub language_code: String,
    #[validate(length(max = 160))]
    pub subject: Option<String>,
    #[validate(range(min = 1_i64, max = 9_000_000_000_000_000_i64))]
    pub replacement_cost_minor: Option<i64>,
    pub replacement_currency_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct VersionRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ReasonedVersionRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 1000))]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TitleSummary {
    pub id: Uuid,
    pub isbn: Option<String>,
    pub title: String,
    pub subtitle: Option<String>,
    pub authors: Vec<String>,
    pub subject: Option<String>,
    pub status: String,
    pub version: i32,
    pub copy_count: i64,
    pub available_copy_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TitleDetail {
    #[serde(flatten)]
    pub summary: TitleSummary,
    pub publisher: Option<String>,
    pub publication_year: Option<i16>,
    pub edition: Option<String>,
    pub language_code: String,
    pub replacement_cost_minor: Option<i64>,
    pub replacement_currency_id: Option<Uuid>,
    pub replacement_currency_code: Option<String>,
    pub replacement_currency_minor_units: Option<i16>,
    pub created_by: Uuid,
}

#[derive(Debug, Serialize)]
pub struct TitlesPage {
    pub titles: Vec<TitleSummary>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateCopyRequest {
    #[validate(length(max = 80))]
    pub barcode: Option<String>,
    #[validate(length(max = 160))]
    pub location: Option<String>,
    pub condition: CopyCondition,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateCopyRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(max = 80))]
    pub barcode: Option<String>,
    #[validate(length(max = 160))]
    pub location: Option<String>,
    pub condition: CopyCondition,
    pub status: CopyStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct CopyResponse {
    pub id: Uuid,
    pub title_id: Uuid,
    pub title: String,
    pub accession_number: String,
    pub barcode: Option<String>,
    pub location: Option<String>,
    pub condition: String,
    pub status: String,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CopiesPage {
    pub copies: Vec<CopyResponse>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateMembershipRequest {
    pub borrower_kind: BorrowerKind,
    pub borrower_id: Uuid,
    #[validate(range(min = 1, max = 100))]
    pub loan_limit: Option<i16>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateMembershipRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    pub status: MembershipStatus,
    #[validate(range(min = 1, max = 100))]
    pub loan_limit: i16,
}

#[derive(Debug, Clone, Serialize)]
pub struct MembershipResponse {
    pub id: Uuid,
    pub borrower_kind: BorrowerKind,
    pub borrower_id: Uuid,
    pub borrower_number: String,
    pub borrower_name: String,
    pub borrower_source_status: String,
    pub account_linked: bool,
    pub card_number: String,
    pub status: String,
    pub loan_limit: i16,
    pub active_loan_count: i64,
    pub active_hold_count: i64,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct MembershipsPage {
    pub memberships: Vec<MembershipResponse>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CheckoutRequest {
    pub copy_id: Uuid,
    pub membership_id: Uuid,
    pub fulfilled_hold_id: Option<Uuid>,
    pub checked_out_on: NaiveDate,
    #[validate(length(max = 1000))]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct RenewLoanRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    pub due_on: Option<NaiveDate>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ReturnLoanRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    pub returned_on: NaiveDate,
    pub copy_condition: CopyCondition,
    #[validate(length(max = 1000))]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoanResponse {
    pub id: Uuid,
    pub copy_id: Uuid,
    pub accession_number: String,
    pub title_id: Uuid,
    pub title: String,
    pub membership_id: Uuid,
    pub borrower_kind: BorrowerKind,
    pub borrower_number: String,
    pub borrower_name: String,
    pub status: String,
    pub checked_out_on: NaiveDate,
    pub due_on: NaiveDate,
    pub returned_on: Option<NaiveDate>,
    pub overdue: bool,
    pub overdue_days: i64,
    pub renewal_count: i16,
    pub version: i32,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct LoansPage {
    pub loans: Vec<LoanResponse>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct PlaceHoldRequest {
    pub title_id: Uuid,
    pub membership_id: Uuid,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ReadyHoldRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    pub copy_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HoldResponse {
    pub id: Uuid,
    pub title_id: Uuid,
    pub title: String,
    pub membership_id: Uuid,
    pub borrower_kind: BorrowerKind,
    pub borrower_number: String,
    pub borrower_name: String,
    pub copy_id: Option<Uuid>,
    pub accession_number: Option<String>,
    pub queue_position: i64,
    pub status: String,
    pub version: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub resolution_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct HoldsPage {
    pub holds: Vec<HoldResponse>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AssessFineRequest {
    pub kind: FineKind,
}

#[derive(Debug, Deserialize, Validate)]
pub struct SubmitFineRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    pub billing_account_id: Uuid,
}

#[derive(Debug, Clone, Serialize)]
pub struct FineResponse {
    pub id: Uuid,
    pub loan_id: Uuid,
    pub membership_id: Uuid,
    pub borrower_kind: BorrowerKind,
    pub borrower_number: String,
    pub borrower_name: String,
    pub title: String,
    pub kind: String,
    pub currency_id: Uuid,
    pub currency_code: String,
    pub currency_minor_units: i16,
    pub amount_minor: i64,
    pub status: String,
    pub assessed_days: Option<i32>,
    pub fees_charge_request_id: Option<Uuid>,
    pub fees_charge_status: Option<String>,
    pub version: i32,
    pub waiver_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct FinesPage {
    pub fines: Vec<FineResponse>,
}
