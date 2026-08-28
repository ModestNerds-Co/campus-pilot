//! Transport contracts for SIS people and admissions workflows.
//!
//! Wire enums and schema checks prevent invalid lifecycle values from reaching
//! persistence. Authentication identity is never accepted from these DTOs.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::{Validate, ValidationError};

use crate::models::{
    AccountCandidate, ApplicationWithDetails, EnrolmentWithDetails,
    GuardianRelationshipWithDetails, GuardianWithAccount, LearnerWithAccount,
};

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveStatus {
    Active,
    Inactive,
}

impl ActiveStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearnerStatus {
    Prospective,
    Active,
    Inactive,
    Graduated,
    Withdrawn,
}

impl LearnerStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Prospective => "prospective",
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::Graduated => "graduated",
            Self::Withdrawn => "withdrawn",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipType {
    Mother,
    Father,
    Parent,
    Guardian,
    Carer,
    Sponsor,
    Other,
}

impl RelationshipType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mother => "mother",
            Self::Father => "father",
            Self::Parent => "parent",
            Self::Guardian => "guardian",
            Self::Carer => "carer",
            Self::Sponsor => "sponsor",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationStatus {
    Draft,
    Submitted,
    UnderReview,
    Offered,
    Accepted,
    Rejected,
    Withdrawn,
}

impl ApplicationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Submitted => "submitted",
            Self::UnderReview => "under_review",
            Self::Offered => "offered",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Withdrawn => "withdrawn",
        }
    }

    pub const fn requires_submission_date(self) -> bool {
        !matches!(self, Self::Draft)
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnrolmentStatus {
    Active,
    Completed,
    Withdrawn,
}

impl EnrolmentStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Withdrawn => "withdrawn",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountProfileKind {
    Learner,
    Guardian,
}

#[derive(Debug, Deserialize)]
pub struct AccountCandidateQuery {
    pub profile_kind: AccountProfileKind,
    pub profile_id: Option<Uuid>,
    pub search: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DirectoryListQuery<S> {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<S>,
}

#[derive(Debug, Deserialize)]
pub struct RelationshipListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<ActiveStatus>,
    pub learner_id: Option<Uuid>,
    pub guardian_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct ApplicationListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<ApplicationStatus>,
    pub academic_year_id: Option<Uuid>,
    pub target_grade_level_id: Option<Uuid>,
    pub learner_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct EnrolmentListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<EnrolmentStatus>,
    pub academic_year_id: Option<Uuid>,
    pub class_group_id: Option<Uuid>,
    pub learner_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct CreateLearnerRequest {
    #[validate(length(min = 1, max = 200))]
    pub display_name: String,
    #[validate(length(max = 120))]
    pub first_names: Option<String>,
    #[validate(length(max = 120))]
    pub surname: Option<String>,
    pub date_of_birth: NaiveDate,
    #[validate(email)]
    pub email: Option<String>,
    #[validate(length(max = 50))]
    pub phone: Option<String>,
    pub status: Option<LearnerStatus>,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct UpdateLearnerRequest {
    #[validate(length(min = 1, max = 200))]
    pub display_name: String,
    #[validate(length(max = 120))]
    pub first_names: Option<String>,
    #[validate(length(max = 120))]
    pub surname: Option<String>,
    pub date_of_birth: NaiveDate,
    #[validate(email)]
    pub email: Option<String>,
    #[validate(length(max = 50))]
    pub phone: Option<String>,
    pub status: LearnerStatus,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct UpdateLearnerNumberingPolicyRequest {
    #[validate(length(min = 1, max = 32))]
    pub number_prefix: String,
    #[validate(range(min = 1, max = 8))]
    pub number_padding: i16,
    #[validate(range(min = 1, max = 100_000_000))]
    pub next_sequence: i64,
    #[validate(range(min = 0))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 1000))]
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct LinkAccountRequest {
    pub account_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate)]
#[validate(schema(function = "guardian_has_contact"))]
pub struct CreateGuardianRequest {
    #[validate(length(min = 1, max = 200))]
    pub display_name: String,
    #[validate(length(max = 120))]
    pub first_names: Option<String>,
    #[validate(length(max = 120))]
    pub surname: Option<String>,
    #[validate(email)]
    pub email: Option<String>,
    #[validate(length(max = 50))]
    pub phone: Option<String>,
    pub status: Option<ActiveStatus>,
}

fn guardian_has_contact(request: &CreateGuardianRequest) -> Result<(), ValidationError> {
    if has_value(request.email.as_deref()) || has_value(request.phone.as_deref()) {
        Ok(())
    } else {
        Err(ValidationError::new("guardian_contact"))
    }
}

#[derive(Debug, Deserialize, Validate)]
#[validate(schema(function = "updated_guardian_has_contact"))]
pub struct UpdateGuardianRequest {
    #[validate(length(min = 1, max = 200))]
    pub display_name: String,
    #[validate(length(max = 120))]
    pub first_names: Option<String>,
    #[validate(length(max = 120))]
    pub surname: Option<String>,
    #[validate(email)]
    pub email: Option<String>,
    #[validate(length(max = 50))]
    pub phone: Option<String>,
    pub status: ActiveStatus,
}

fn updated_guardian_has_contact(request: &UpdateGuardianRequest) -> Result<(), ValidationError> {
    if has_value(request.email.as_deref()) || has_value(request.phone.as_deref()) {
        Ok(())
    } else {
        Err(ValidationError::new("guardian_contact"))
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateGuardianRelationshipRequest {
    pub learner_id: Uuid,
    pub guardian_id: Uuid,
    pub relationship_type: RelationshipType,
    pub is_primary: Option<bool>,
    pub can_collect: Option<bool>,
    pub receives_communications: Option<bool>,
    pub status: Option<ActiveStatus>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateGuardianRelationshipRequest {
    pub relationship_type: RelationshipType,
    pub is_primary: bool,
    pub can_collect: bool,
    pub receives_communications: bool,
    pub status: ActiveStatus,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateApplicationRequest {
    #[validate(length(min = 1, max = 80))]
    pub application_number: String,
    pub learner_id: Uuid,
    pub academic_year_id: Uuid,
    pub target_grade_level_id: Uuid,
    pub submitted_on: Option<NaiveDate>,
    pub status: Option<ApplicationStatus>,
    #[validate(length(max = 4_000))]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateApplicationRequest {
    #[validate(length(min = 1, max = 80))]
    pub application_number: String,
    pub learner_id: Uuid,
    pub academic_year_id: Uuid,
    pub target_grade_level_id: Uuid,
    pub submitted_on: Option<NaiveDate>,
    pub status: ApplicationStatus,
    #[validate(length(max = 4_000))]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
#[validate(schema(function = "create_enrolment_dates"))]
pub struct CreateEnrolmentRequest {
    pub learner_id: Uuid,
    pub academic_year_id: Uuid,
    pub class_group_id: Uuid,
    pub source_application_id: Option<Uuid>,
    pub starts_on: NaiveDate,
    pub ends_on: Option<NaiveDate>,
    pub status: Option<EnrolmentStatus>,
}

fn create_enrolment_dates(request: &CreateEnrolmentRequest) -> Result<(), ValidationError> {
    validate_dates(request.starts_on, request.ends_on)
}

#[derive(Debug, Deserialize, Validate)]
#[validate(schema(function = "update_enrolment_dates"))]
pub struct UpdateEnrolmentRequest {
    pub learner_id: Uuid,
    pub academic_year_id: Uuid,
    pub class_group_id: Uuid,
    pub source_application_id: Option<Uuid>,
    pub starts_on: NaiveDate,
    pub ends_on: Option<NaiveDate>,
    pub status: EnrolmentStatus,
}

fn update_enrolment_dates(request: &UpdateEnrolmentRequest) -> Result<(), ValidationError> {
    validate_dates(request.starts_on, request.ends_on)
}

fn validate_dates(starts_on: NaiveDate, ends_on: Option<NaiveDate>) -> Result<(), ValidationError> {
    if ends_on.is_some_and(|ends_on| ends_on < starts_on) {
        Err(ValidationError::new("enrolment_dates"))
    } else {
        Ok(())
    }
}

fn has_value(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
}

#[derive(Debug, Serialize)]
pub struct LearnerResponse {
    pub id: Uuid,
    pub account_id: Option<Uuid>,
    pub account_email: Option<String>,
    pub learner_number: String,
    pub display_name: String,
    pub first_names: Option<String>,
    pub surname: Option<String>,
    pub date_of_birth: NaiveDate,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub status: String,
}

impl From<LearnerWithAccount> for LearnerResponse {
    fn from(value: LearnerWithAccount) -> Self {
        Self {
            id: value.id,
            account_id: value.account_id,
            account_email: value.account_email,
            learner_number: value.learner_number,
            display_name: value.display_name,
            first_names: value.first_names,
            surname: value.surname,
            date_of_birth: value.date_of_birth,
            email: value.email,
            phone: value.phone,
            status: value.status,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GuardianResponse {
    pub id: Uuid,
    pub account_id: Option<Uuid>,
    pub account_email: Option<String>,
    pub display_name: String,
    pub first_names: Option<String>,
    pub surname: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub status: String,
}

impl From<GuardianWithAccount> for GuardianResponse {
    fn from(value: GuardianWithAccount) -> Self {
        Self {
            id: value.id,
            account_id: value.account_id,
            account_email: value.account_email,
            display_name: value.display_name,
            first_names: value.first_names,
            surname: value.surname,
            email: value.email,
            phone: value.phone,
            status: value.status,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedLearnersResponse {
    pub learners: Vec<LearnerResponse>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedGuardiansResponse {
    pub guardians: Vec<GuardianResponse>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedGuardianRelationshipsResponse {
    pub relationships: Vec<GuardianRelationshipWithDetails>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedApplicationsResponse {
    pub applications: Vec<ApplicationWithDetails>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedEnrolmentsResponse {
    pub enrolments: Vec<EnrolmentWithDetails>,
}

#[derive(Debug, Serialize)]
pub struct AccountCandidatesResponse {
    pub accounts: Vec<AccountCandidate>,
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use validator::Validate;

    use super::{
        ApplicationStatus, CreateEnrolmentRequest, CreateGuardianRequest, CreateLearnerRequest,
        EnrolmentStatus, UpdateLearnerRequest,
    };

    #[test]
    fn ordinary_learner_contracts_reject_caller_supplied_numbers() {
        let create = serde_json::from_value::<CreateLearnerRequest>(serde_json::json!({
            "learner_number": "LEGACY-17",
            "display_name": "Example Learner",
            "date_of_birth": "2012-01-01"
        }));
        let update = serde_json::from_value::<UpdateLearnerRequest>(serde_json::json!({
            "learner_number": "RENUMBERED-17",
            "display_name": "Example Learner",
            "date_of_birth": "2012-01-01",
            "status": "active"
        }));
        assert!(create.is_err());
        assert!(update.is_err());
    }

    #[test]
    fn guardian_requires_at_least_one_contact_method() {
        let request = CreateGuardianRequest {
            display_name: "Guardian".to_string(),
            first_names: None,
            surname: None,
            email: None,
            phone: None,
            status: None,
        };
        assert!(request.validate().is_err());
    }

    #[test]
    fn enrolment_end_cannot_precede_start() {
        let request = CreateEnrolmentRequest {
            learner_id: uuid::Uuid::new_v4(),
            academic_year_id: uuid::Uuid::new_v4(),
            class_group_id: uuid::Uuid::new_v4(),
            source_application_id: None,
            starts_on: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap_or_else(|| unreachable!()),
            ends_on: NaiveDate::from_ymd_opt(2026, 8, 31),
            status: Some(EnrolmentStatus::Active),
        };
        assert!(request.validate().is_err());
    }

    #[test]
    fn submitted_application_states_require_submission_dates() {
        assert!(!ApplicationStatus::Draft.requires_submission_date());
        assert!(ApplicationStatus::UnderReview.requires_submission_date());
    }
}
