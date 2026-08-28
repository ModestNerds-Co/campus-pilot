//! Transport DTOs for Academics workflows.
//!
//! Wire values are normalized at the route boundary before typed operations
//! access persistence.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::{Validate, ValidationError};

use crate::models::{
    AcademicGradeLevel, AcademicTerm, AcademicYear, ClassGroupWithYear, Subject,
    TeacherProfileWithEmployee, TeachingAssignmentWithDetails,
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
pub enum AcademicYearStatus {
    Planned,
    Active,
    Closed,
}

impl AcademicYearStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Active => "active",
            Self::Closed => "closed",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DirectoryListQuery<S> {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<S>,
}

#[derive(Debug, Deserialize, Validate)]
#[validate(schema(function = "validate_year_dates"))]
pub struct CreateAcademicYearRequest {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub status: Option<AcademicYearStatus>,
}

fn validate_year_dates(request: &CreateAcademicYearRequest) -> Result<(), ValidationError> {
    if request.ends_on < request.starts_on {
        return Err(ValidationError::new("academic_year_dates"));
    }
    Ok(())
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateAcademicYearRequest {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub status: AcademicYearStatus,
}

impl UpdateAcademicYearRequest {
    pub fn dates_are_valid(&self) -> bool {
        self.ends_on >= self.starts_on
    }
}

#[derive(Debug, Serialize)]
pub struct AcademicYearResponse {
    pub id: Uuid,
    pub name: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub status: String,
}

impl From<AcademicYear> for AcademicYearResponse {
    fn from(value: AcademicYear) -> Self {
        Self {
            id: value.id,
            name: value.name,
            starts_on: value.starts_on,
            ends_on: value.ends_on,
            status: value.status,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedAcademicYearsResponse {
    pub academic_years: Vec<AcademicYearResponse>,
}

#[derive(Debug, Deserialize)]
pub struct AcademicTermListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<AcademicYearStatus>,
    pub academic_year_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateAcademicTermRequest {
    pub academic_year_id: Uuid,
    #[validate(length(min = 1, max = 40))]
    pub code: String,
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub status: Option<AcademicYearStatus>,
}

impl CreateAcademicTermRequest {
    pub fn dates_are_valid(&self) -> bool {
        self.ends_on >= self.starts_on
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateAcademicTermRequest {
    pub academic_year_id: Uuid,
    #[validate(length(min = 1, max = 40))]
    pub code: String,
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub status: AcademicYearStatus,
}

impl UpdateAcademicTermRequest {
    pub fn dates_are_valid(&self) -> bool {
        self.ends_on >= self.starts_on
    }
}

#[derive(Debug, Serialize)]
pub struct AcademicTermResponse {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_year_name: String,
    pub code: String,
    pub name: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub status: String,
}

impl From<AcademicTerm> for AcademicTermResponse {
    fn from(value: AcademicTerm) -> Self {
        Self {
            id: value.id,
            academic_year_id: value.academic_year_id,
            academic_year_name: value.academic_year_name,
            code: value.code,
            name: value.name,
            starts_on: value.starts_on,
            ends_on: value.ends_on,
            status: value.status,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedAcademicTermsResponse {
    pub terms: Vec<AcademicTermResponse>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateSubjectRequest {
    #[validate(length(min = 1, max = 40))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    pub status: Option<ActiveStatus>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateSubjectRequest {
    #[validate(length(min = 1, max = 40))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    pub status: ActiveStatus,
}

#[derive(Debug, Serialize)]
pub struct SubjectResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub status: String,
}

impl From<Subject> for SubjectResponse {
    fn from(value: Subject) -> Self {
        Self {
            id: value.id,
            code: value.code,
            name: value.name,
            status: value.status,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedSubjectsResponse {
    pub subjects: Vec<SubjectResponse>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateAcademicGradeLevelRequest {
    #[validate(length(min = 1, max = 40))]
    pub code: String,
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    #[validate(range(min = 0, max = 999))]
    pub sequence_number: i16,
    pub status: Option<ActiveStatus>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateAcademicGradeLevelRequest {
    #[validate(length(min = 1, max = 40))]
    pub code: String,
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    #[validate(range(min = 0, max = 999))]
    pub sequence_number: i16,
    pub status: ActiveStatus,
}

#[derive(Debug, Serialize)]
pub struct AcademicGradeLevelResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub sequence_number: i16,
    pub status: String,
}

impl From<AcademicGradeLevel> for AcademicGradeLevelResponse {
    fn from(value: AcademicGradeLevel) -> Self {
        Self {
            id: value.id,
            code: value.code,
            name: value.name,
            sequence_number: value.sequence_number,
            status: value.status,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedAcademicGradeLevelsResponse {
    pub grade_levels: Vec<AcademicGradeLevelResponse>,
}

#[derive(Debug, Deserialize)]
pub struct TeacherListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<ActiveStatus>,
}

#[derive(Debug, Deserialize)]
pub struct TeacherCandidateQuery {
    pub search: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTeacherProfileRequest {
    pub employee_id: Uuid,
    pub status: Option<ActiveStatus>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTeacherProfileRequest {
    pub status: ActiveStatus,
}

#[derive(Debug, Serialize)]
pub struct TeacherProfileResponse {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub employee_number: String,
    pub display_name: String,
    pub work_email: Option<String>,
    pub phone: Option<String>,
    pub employment_status: String,
    pub status: String,
}

impl From<TeacherProfileWithEmployee> for TeacherProfileResponse {
    fn from(value: TeacherProfileWithEmployee) -> Self {
        Self {
            id: value.id,
            employee_id: value.employee_id,
            employee_number: value.employee_number,
            display_name: value.display_name,
            work_email: value.work_email,
            phone: value.phone,
            employment_status: value.employment_status,
            status: value.status,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedTeacherProfilesResponse {
    pub teachers: Vec<TeacherProfileResponse>,
}

#[derive(Debug, Deserialize)]
pub struct ClassGroupListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<ActiveStatus>,
    pub academic_year_id: Option<Uuid>,
    pub grade_level_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateClassGroupRequest {
    pub academic_year_id: Uuid,
    #[validate(length(min = 1, max = 40))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    pub grade_level_id: Option<Uuid>,
    pub status: Option<ActiveStatus>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateClassGroupRequest {
    pub academic_year_id: Uuid,
    #[validate(length(min = 1, max = 40))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    pub grade_level_id: Option<Uuid>,
    pub status: ActiveStatus,
}

#[derive(Debug, Serialize)]
pub struct ClassGroupResponse {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_year_name: String,
    pub code: String,
    pub name: String,
    pub grade_level_id: Option<Uuid>,
    pub grade_level: Option<String>,
    pub status: String,
}

impl From<ClassGroupWithYear> for ClassGroupResponse {
    fn from(value: ClassGroupWithYear) -> Self {
        Self {
            id: value.id,
            academic_year_id: value.academic_year_id,
            academic_year_name: value.academic_year_name,
            code: value.code,
            name: value.name,
            grade_level_id: value.grade_level_id,
            grade_level: value.grade_level,
            status: value.status,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedClassGroupsResponse {
    pub classes: Vec<ClassGroupResponse>,
}

#[derive(Debug, Deserialize)]
pub struct AssignmentListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub status: Option<ActiveStatus>,
    pub academic_year_id: Option<Uuid>,
    pub class_group_id: Option<Uuid>,
    pub teacher_profile_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateTeachingAssignmentRequest {
    pub academic_year_id: Uuid,
    pub class_group_id: Uuid,
    pub subject_id: Uuid,
    pub teacher_profile_id: Uuid,
    #[validate(range(min = 1, max = 40))]
    pub periods_per_cycle: i16,
    pub status: Option<ActiveStatus>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateTeachingAssignmentRequest {
    pub academic_year_id: Uuid,
    pub class_group_id: Uuid,
    pub subject_id: Uuid,
    pub teacher_profile_id: Uuid,
    #[validate(range(min = 1, max = 40))]
    pub periods_per_cycle: i16,
    pub status: ActiveStatus,
}

#[derive(Debug, Serialize)]
pub struct TeachingAssignmentResponse {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_year_name: String,
    pub class_group_id: Uuid,
    pub class_group_name: String,
    pub subject_id: Uuid,
    pub subject_name: String,
    pub teacher_profile_id: Uuid,
    pub employee_id: Uuid,
    pub teacher_name: String,
    pub periods_per_cycle: i16,
    pub status: String,
}

impl From<TeachingAssignmentWithDetails> for TeachingAssignmentResponse {
    fn from(value: TeachingAssignmentWithDetails) -> Self {
        Self {
            id: value.id,
            academic_year_id: value.academic_year_id,
            academic_year_name: value.academic_year_name,
            class_group_id: value.class_group_id,
            class_group_name: value.class_group_name,
            subject_id: value.subject_id,
            subject_name: value.subject_name,
            teacher_profile_id: value.teacher_profile_id,
            employee_id: value.employee_id,
            teacher_name: value.teacher_name,
            periods_per_cycle: value.periods_per_cycle,
            status: value.status,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedTeachingAssignmentsResponse {
    pub assignments: Vec<TeachingAssignmentResponse>,
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use validator::Validate;

    use super::{CreateAcademicYearRequest, validate_year_dates};

    #[test]
    fn academic_year_rejects_reverse_dates() {
        let request = CreateAcademicYearRequest {
            name: "2027".to_string(),
            starts_on: NaiveDate::from_ymd_opt(2027, 12, 1).unwrap_or_else(|| unreachable!()),
            ends_on: NaiveDate::from_ymd_opt(2027, 1, 1).unwrap_or_else(|| unreachable!()),
            status: None,
        };
        assert!(validate_year_dates(&request).is_err());
        assert!(request.validate().is_err());
    }
}
