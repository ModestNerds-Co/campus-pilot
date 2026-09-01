//! Internal persistence projections for Library-owned records.

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct SettingsRow {
    pub accession_prefix: String,
    pub accession_next_sequence: i64,
    pub accession_padding: i16,
    pub learner_loan_days: i16,
    pub employee_loan_days: i16,
    pub default_loan_limit: i16,
    pub maximum_renewals: i16,
    pub fine_currency_id: Option<Uuid>,
    pub overdue_fine_minor: i64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct TitleSummaryRow {
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

#[derive(Debug, Clone, FromRow)]
pub struct TitleDetailRow {
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
    pub publisher: Option<String>,
    pub publication_year: Option<i16>,
    pub edition: Option<String>,
    pub language_code: String,
    pub replacement_cost_minor: Option<i64>,
    pub replacement_currency_id: Option<Uuid>,
    pub created_by: Uuid,
}

#[derive(Debug, Clone, FromRow)]
pub struct CopyRow {
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

#[derive(Debug, Clone, FromRow)]
pub struct MembershipRow {
    pub id: Uuid,
    pub learner_id: Option<Uuid>,
    pub employee_id: Option<Uuid>,
    pub card_number: String,
    pub status: String,
    pub loan_limit: i16,
    pub active_loan_count: i64,
    pub active_hold_count: i64,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct BorrowerIdentity {
    pub number: String,
    pub display_name: String,
    pub source_status: String,
    pub account_linked: bool,
}

#[derive(Debug, Clone, FromRow)]
pub struct LoanRow {
    pub id: Uuid,
    pub copy_id: Uuid,
    pub accession_number: String,
    pub title_id: Uuid,
    pub title: String,
    pub membership_id: Uuid,
    pub learner_id: Option<Uuid>,
    pub employee_id: Option<Uuid>,
    pub status: String,
    pub checked_out_on: NaiveDate,
    pub due_on: NaiveDate,
    pub returned_on: Option<NaiveDate>,
    pub renewal_count: i16,
    pub version: i32,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct HoldRow {
    pub id: Uuid,
    pub title_id: Uuid,
    pub title: String,
    pub membership_id: Uuid,
    pub learner_id: Option<Uuid>,
    pub employee_id: Option<Uuid>,
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

#[derive(Debug, Clone, FromRow)]
pub struct FineRow {
    pub id: Uuid,
    pub loan_id: Uuid,
    pub membership_id: Uuid,
    pub learner_id: Option<Uuid>,
    pub employee_id: Option<Uuid>,
    pub title: String,
    pub kind: String,
    pub currency_id: Uuid,
    pub amount_minor: i64,
    pub status: String,
    pub assessed_days: Option<i32>,
    pub fees_charge_request_id: Option<Uuid>,
    pub version: i32,
    pub waiver_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
