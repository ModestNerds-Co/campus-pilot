//! Authenticated Academics HTTP routes.
//!
//! The application mounts authentication outside this crate. Exact operation
//! permissions and licensing are re-evaluated before these handlers run.

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, delete, get, post, put, web};
use cp_common::{
    ApiResponse, PaginationMeta, RequirePermission, TenantId, flatten_validation_errors,
};
use cp_hr_payroll::models::EmployeeReference;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;

use crate::{
    dtos::{
        AcademicGradeLevelResponse, AcademicTermListQuery, AcademicTermResponse,
        AcademicYearResponse, AssignmentListQuery, ClassGroupListQuery, ClassGroupResponse,
        CreateAcademicGradeLevelRequest, CreateAcademicTermRequest, CreateAcademicYearRequest,
        CreateClassGroupRequest, CreateSubjectRequest, CreateTeacherProfileRequest,
        CreateTeachingAssignmentRequest, DirectoryListQuery, PaginatedAcademicGradeLevelsResponse,
        PaginatedAcademicTermsResponse, PaginatedAcademicYearsResponse,
        PaginatedClassGroupsResponse, PaginatedSubjectsResponse, PaginatedTeacherProfilesResponse,
        PaginatedTeachingAssignmentsResponse, SubjectResponse, TeacherCandidateQuery,
        TeacherListQuery, TeacherProfileResponse, TeachingAssignmentResponse,
        UpdateAcademicGradeLevelRequest, UpdateAcademicTermRequest, UpdateAcademicYearRequest,
        UpdateClassGroupRequest, UpdateSubjectRequest, UpdateTeacherProfileRequest,
        UpdateTeachingAssignmentRequest,
    },
    ops::{
        AcademicGradeLevelOps, AcademicTermOps, AcademicYearOps, ClassGroupOps, DeleteOutcome,
        SubjectOps, TeacherProfileOps, TeachingAssignmentOps,
    },
};

#[get("/academic-years")]
async fn list_academic_years(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<DirectoryListQuery<crate::dtos::AcademicYearStatus>>,
) -> Result<HttpResponse, actix_web::Error> {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    let (years, total) = AcademicYearOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        query.status.map(crate::dtos::AcademicYearStatus::as_str),
    )
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(paginated(
        PaginatedAcademicYearsResponse {
            academic_years: years.into_iter().map(AcademicYearResponse::from).collect(),
        },
        page,
        per_page,
        total,
    ))
}

#[get("/academic-years/{id}")]
async fn read_academic_year(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let year = AcademicYearOps::get_by_id(pool.get_ref(), tenant_id(tenant), path.into_inner())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(found(year.map(AcademicYearResponse::from), "Academic year"))
}

#[post("/academic-years")]
async fn create_academic_year(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<CreateAcademicYearRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(response) = validation_response(&*body) {
        return Ok(response);
    }
    let result = AcademicYearOps::create(pool.get_ref(), tenant_id(tenant), &body).await;
    Ok(created_or_error(result.map(AcademicYearResponse::from)))
}

#[put("/academic-years/{id}")]
async fn update_academic_year(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateAcademicYearRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(response) = validation_response(&*body) {
        return Ok(response);
    }
    if !body.dates_are_valid() {
        return Ok(bad_request(
            "Academic year end date cannot be before its start date",
        ));
    }
    let result =
        AcademicYearOps::update(pool.get_ref(), tenant_id(tenant), path.into_inner(), &body).await;
    Ok(updated_or_error(
        result.map(|value| value.map(AcademicYearResponse::from)),
        "Academic year",
    ))
}

#[delete("/academic-years/{id}")]
async fn delete_academic_year(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let outcome = AcademicYearOps::delete(pool.get_ref(), tenant_id(tenant), path.into_inner())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(delete_response(
        outcome,
        "Academic year",
        "Remove its terms and classes before deleting this academic year.",
    ))
}

#[get("/terms")]
async fn list_academic_terms(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<AcademicTermListQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    let (terms, total) = AcademicTermOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        query.status.map(crate::dtos::AcademicYearStatus::as_str),
        query.academic_year_id,
    )
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(paginated(
        PaginatedAcademicTermsResponse {
            terms: terms.into_iter().map(AcademicTermResponse::from).collect(),
        },
        page,
        per_page,
        total,
    ))
}

#[get("/terms/{id}")]
async fn read_academic_term(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let term = AcademicTermOps::get_by_id(pool.get_ref(), tenant_id(tenant), path.into_inner())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(found(term.map(AcademicTermResponse::from), "Academic term"))
}

#[post("/terms")]
async fn create_academic_term(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<CreateAcademicTermRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(response) = validation_response(&*body) {
        return Ok(response);
    }
    if !body.dates_are_valid() {
        return Ok(bad_request(
            "Academic term end date cannot be before its start date",
        ));
    }
    Ok(created_or_error(
        AcademicTermOps::create(pool.get_ref(), tenant_id(tenant), &body)
            .await
            .map(AcademicTermResponse::from),
    ))
}

#[put("/terms/{id}")]
async fn update_academic_term(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateAcademicTermRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(response) = validation_response(&*body) {
        return Ok(response);
    }
    if !body.dates_are_valid() {
        return Ok(bad_request(
            "Academic term end date cannot be before its start date",
        ));
    }
    Ok(updated_or_error(
        AcademicTermOps::update(pool.get_ref(), tenant_id(tenant), path.into_inner(), &body)
            .await
            .map(|value| value.map(AcademicTermResponse::from)),
        "Academic term",
    ))
}

#[delete("/terms/{id}")]
async fn delete_academic_term(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let result =
        AcademicTermOps::delete(pool.get_ref(), tenant_id(tenant), path.into_inner()).await;
    Ok(match result {
        Ok(outcome) => delete_response(outcome, "Academic term", "This academic term is in use."),
        Err(error) => operation_error(error),
    })
}

#[get("/subjects")]
async fn list_subjects(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<DirectoryListQuery<crate::dtos::ActiveStatus>>,
) -> Result<HttpResponse, actix_web::Error> {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    let (subjects, total) = SubjectOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        query.status.map(crate::dtos::ActiveStatus::as_str),
    )
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(paginated(
        PaginatedSubjectsResponse {
            subjects: subjects.into_iter().map(SubjectResponse::from).collect(),
        },
        page,
        per_page,
        total,
    ))
}

#[get("/subjects/{id}")]
async fn read_subject(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let subject = SubjectOps::get_by_id(pool.get_ref(), tenant_id(tenant), path.into_inner())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(found(subject.map(SubjectResponse::from), "Subject"))
}

#[post("/subjects")]
async fn create_subject(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<CreateSubjectRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(response) = validation_response(&*body) {
        return Ok(response);
    }
    let result = SubjectOps::create(pool.get_ref(), tenant_id(tenant), &body).await;
    Ok(created_or_error(result.map(SubjectResponse::from)))
}

#[put("/subjects/{id}")]
async fn update_subject(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateSubjectRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(response) = validation_response(&*body) {
        return Ok(response);
    }
    let result =
        SubjectOps::update(pool.get_ref(), tenant_id(tenant), path.into_inner(), &body).await;
    Ok(updated_or_error(
        result.map(|value| value.map(SubjectResponse::from)),
        "Subject",
    ))
}

#[delete("/subjects/{id}")]
async fn delete_subject(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let outcome = SubjectOps::delete(pool.get_ref(), tenant_id(tenant), path.into_inner())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(delete_response(
        outcome,
        "Subject",
        "Remove its teaching assignments before deleting this subject.",
    ))
}

#[get("/grade-levels")]
async fn list_grade_levels(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<DirectoryListQuery<crate::dtos::ActiveStatus>>,
) -> Result<HttpResponse, actix_web::Error> {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    let (grade_levels, total) = AcademicGradeLevelOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        query.status.map(crate::dtos::ActiveStatus::as_str),
    )
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(paginated(
        PaginatedAcademicGradeLevelsResponse {
            grade_levels: grade_levels
                .into_iter()
                .map(AcademicGradeLevelResponse::from)
                .collect(),
        },
        page,
        per_page,
        total,
    ))
}

#[get("/grade-levels/{id}")]
async fn read_grade_level(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let grade_level =
        AcademicGradeLevelOps::get_by_id(pool.get_ref(), tenant_id(tenant), path.into_inner())
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(found(
        grade_level.map(AcademicGradeLevelResponse::from),
        "Academic grade level",
    ))
}

#[post("/grade-levels")]
async fn create_grade_level(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<CreateAcademicGradeLevelRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(response) = validation_response(&*body) {
        return Ok(response);
    }
    Ok(created_or_error(
        AcademicGradeLevelOps::create(pool.get_ref(), tenant_id(tenant), &body)
            .await
            .map(AcademicGradeLevelResponse::from),
    ))
}

#[put("/grade-levels/{id}")]
async fn update_grade_level(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateAcademicGradeLevelRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(response) = validation_response(&*body) {
        return Ok(response);
    }
    Ok(updated_or_error(
        AcademicGradeLevelOps::update(pool.get_ref(), tenant_id(tenant), path.into_inner(), &body)
            .await
            .map(|value| value.map(AcademicGradeLevelResponse::from)),
        "Academic grade level",
    ))
}

#[delete("/grade-levels/{id}")]
async fn delete_grade_level(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let outcome =
        AcademicGradeLevelOps::delete(pool.get_ref(), tenant_id(tenant), path.into_inner())
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(delete_response(
        outcome,
        "Academic grade level",
        "Move or remove its classes before deleting this grade level.",
    ))
}

#[get("/teacher-candidates")]
async fn list_teacher_candidates(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<TeacherCandidateQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let employees = TeacherProfileOps::list_candidates(
        pool.get_ref(),
        tenant_id(tenant),
        trimmed(query.search.as_deref()),
    )
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(ok(TeacherCandidatesResponse { employees }))
}

#[get("/teachers")]
async fn list_teachers(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<TeacherListQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    let (teachers, total) = TeacherProfileOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        query.status.map(crate::dtos::ActiveStatus::as_str),
    )
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(paginated(
        PaginatedTeacherProfilesResponse {
            teachers: teachers
                .into_iter()
                .map(TeacherProfileResponse::from)
                .collect(),
        },
        page,
        per_page,
        total,
    ))
}

#[get("/teachers/{id}")]
async fn read_teacher(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let teacher =
        TeacherProfileOps::get_by_id(pool.get_ref(), tenant_id(tenant), path.into_inner())
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(found(teacher.map(TeacherProfileResponse::from), "Teacher"))
}

#[post("/teachers")]
async fn create_teacher(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<CreateTeacherProfileRequest>,
) -> HttpResponse {
    created_or_error(
        TeacherProfileOps::create(pool.get_ref(), tenant_id(tenant), &body)
            .await
            .map(TeacherProfileResponse::from),
    )
}

#[put("/teachers/{id}")]
async fn update_teacher(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateTeacherProfileRequest>,
) -> HttpResponse {
    updated_or_error(
        TeacherProfileOps::update(pool.get_ref(), tenant_id(tenant), path.into_inner(), &body)
            .await
            .map(|value| value.map(TeacherProfileResponse::from)),
        "Teacher",
    )
}

#[delete("/teachers/{id}")]
async fn delete_teacher(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let outcome = TeacherProfileOps::delete(pool.get_ref(), tenant_id(tenant), path.into_inner())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(delete_response(
        outcome,
        "Teacher",
        "Remove this teacher's assignments before deleting the profile.",
    ))
}

#[get("/classes")]
async fn list_classes(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<ClassGroupListQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    let (classes, total) = ClassGroupOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        query.status.map(crate::dtos::ActiveStatus::as_str),
        query.academic_year_id,
        query.grade_level_id,
    )
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(paginated(
        PaginatedClassGroupsResponse {
            classes: classes.into_iter().map(ClassGroupResponse::from).collect(),
        },
        page,
        per_page,
        total,
    ))
}

#[get("/classes/{id}")]
async fn read_class(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let class_group =
        ClassGroupOps::get_by_id(pool.get_ref(), tenant_id(tenant), path.into_inner())
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(found(class_group.map(ClassGroupResponse::from), "Class"))
}

#[post("/classes")]
async fn create_class(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<CreateClassGroupRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(response) = validation_response(&*body) {
        return Ok(response);
    }
    Ok(created_or_error(
        ClassGroupOps::create(pool.get_ref(), tenant_id(tenant), &body)
            .await
            .map(ClassGroupResponse::from),
    ))
}

#[put("/classes/{id}")]
async fn update_class(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateClassGroupRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(response) = validation_response(&*body) {
        return Ok(response);
    }
    Ok(updated_or_error(
        ClassGroupOps::update(pool.get_ref(), tenant_id(tenant), path.into_inner(), &body)
            .await
            .map(|value| value.map(ClassGroupResponse::from)),
        "Class",
    ))
}

#[delete("/classes/{id}")]
async fn delete_class(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let outcome = ClassGroupOps::delete(pool.get_ref(), tenant_id(tenant), path.into_inner())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(delete_response(
        outcome,
        "Class",
        "Remove its teaching assignments before deleting this class.",
    ))
}

#[get("/teaching-assignments")]
async fn list_assignments(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<AssignmentListQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    let (assignments, total) = TeachingAssignmentOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        query.status.map(crate::dtos::ActiveStatus::as_str),
        query.academic_year_id,
        query.class_group_id,
        query.teacher_profile_id,
    )
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(paginated(
        PaginatedTeachingAssignmentsResponse {
            assignments: assignments
                .into_iter()
                .map(TeachingAssignmentResponse::from)
                .collect(),
        },
        page,
        per_page,
        total,
    ))
}

#[get("/teaching-assignments/{id}")]
async fn read_assignment(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let assignment =
        TeachingAssignmentOps::get_by_id(pool.get_ref(), tenant_id(tenant), path.into_inner())
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(found(
        assignment.map(TeachingAssignmentResponse::from),
        "Teaching assignment",
    ))
}

#[post("/teaching-assignments")]
async fn create_assignment(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<CreateTeachingAssignmentRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(response) = validation_response(&*body) {
        return Ok(response);
    }
    Ok(created_or_error(
        TeachingAssignmentOps::create(pool.get_ref(), tenant_id(tenant), &body)
            .await
            .map(TeachingAssignmentResponse::from),
    ))
}

#[put("/teaching-assignments/{id}")]
async fn update_assignment(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateTeachingAssignmentRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(response) = validation_response(&*body) {
        return Ok(response);
    }
    Ok(updated_or_error(
        TeachingAssignmentOps::update(pool.get_ref(), tenant_id(tenant), path.into_inner(), &body)
            .await
            .map(|value| value.map(TeachingAssignmentResponse::from)),
        "Teaching assignment",
    ))
}

#[delete("/teaching-assignments/{id}")]
async fn delete_assignment(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let deleted =
        TeachingAssignmentOps::delete(pool.get_ref(), tenant_id(tenant), path.into_inner())
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(if deleted {
        ok(serde_json::json!({ "deleted": true }))
    } else {
        not_found("Teaching assignment")
    })
}

#[derive(Serialize)]
struct TeacherCandidatesResponse {
    employees: Vec<EmployeeReference>,
}

fn tenant_id(tenant: web::ReqData<TenantId>) -> Uuid {
    tenant.into_inner().into_inner()
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).max(1),
        per_page.unwrap_or(25).clamp(1, 100),
    )
}

fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn validation_response<T: Validate>(value: &T) -> Option<HttpResponse> {
    value.validate().err().map(|errors| {
        HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(flatten_validation_errors(&errors)),
        ))
    })
}

fn ok<T: Serialize>(value: T) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::from_status(StatusCode::OK, Some(value), None))
}

fn paginated<T: Serialize>(value: T, page: i64, per_page: i64, total: i64) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::with_pagination(
        StatusCode::OK,
        Some(value),
        PaginationMeta::new(page as u32, per_page as u32, total),
        None,
    ))
}

fn found<T: Serialize>(value: Option<T>, label: &str) -> HttpResponse {
    value.map_or_else(|| not_found(label), ok)
}

fn not_found(label: &str) -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::from_status(
        StatusCode::NOT_FOUND,
        None::<()>,
        Some(vec![format!("{label} not found")]),
    ))
}

fn bad_request(message: &str) -> HttpResponse {
    HttpResponse::BadRequest().json(ApiResponse::from_status(
        StatusCode::BAD_REQUEST,
        None::<()>,
        Some(vec![message.to_string()]),
    ))
}

fn created_or_error<T: Serialize>(result: anyhow::Result<T>) -> HttpResponse {
    match result {
        Ok(value) => HttpResponse::Created().json(ApiResponse::from_status(
            StatusCode::CREATED,
            Some(value),
            None,
        )),
        Err(error) => operation_error(error),
    }
}

fn updated_or_error<T: Serialize>(result: anyhow::Result<Option<T>>, label: &str) -> HttpResponse {
    match result {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found(label),
        Err(error) => operation_error(error),
    }
}

fn operation_error(error: anyhow::Error) -> HttpResponse {
    let safe_message = error.to_string();
    if let Some(database) = error.root_cause().downcast_ref::<sqlx::Error>()
        && let sqlx::Error::Database(database) = database
        && database.code().as_deref() == Some("23505")
    {
        return HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec!["That academic record already exists.".to_string()]),
        ));
    }
    if safe_message.starts_with("Only ")
        || safe_message.starts_with("An inactive")
        || safe_message.starts_with("An active")
        || safe_message.starts_with("Academic term")
        || safe_message.starts_with("Academic year")
        || safe_message.starts_with("Academic grade level")
        || safe_message.starts_with("Every academic term")
        || safe_message.starts_with("A closed academic year")
        || safe_message.starts_with("The class")
        || safe_message.ends_with("for this campus")
    {
        return bad_request(&safe_message);
    }
    HttpResponse::InternalServerError().json(ApiResponse::from_status(
        StatusCode::INTERNAL_SERVER_ERROR,
        None::<()>,
        Some(vec!["The academic record could not be saved.".to_string()]),
    ))
}

fn delete_response(outcome: DeleteOutcome, label: &str, in_use_message: &str) -> HttpResponse {
    match outcome {
        DeleteOutcome::Deleted => ok(serde_json::json!({ "deleted": true })),
        DeleteOutcome::NotFound => not_found(label),
        DeleteOutcome::InUse => HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec![in_use_message.to_string()]),
        )),
    }
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(RequirePermission::new("academics"))
            .service(list_academic_years)
            .service(read_academic_year)
            .service(create_academic_year)
            .service(update_academic_year)
            .service(delete_academic_year)
            .service(list_academic_terms)
            .service(read_academic_term)
            .service(create_academic_term)
            .service(update_academic_term)
            .service(delete_academic_term)
            .service(list_subjects)
            .service(read_subject)
            .service(create_subject)
            .service(update_subject)
            .service(delete_subject)
            .service(list_grade_levels)
            .service(read_grade_level)
            .service(create_grade_level)
            .service(update_grade_level)
            .service(delete_grade_level)
            .service(list_teacher_candidates)
            .service(list_teachers)
            .service(read_teacher)
            .service(create_teacher)
            .service(update_teacher)
            .service(delete_teacher)
            .service(list_classes)
            .service(read_class)
            .service(create_class)
            .service(update_class)
            .service(delete_class)
            .service(list_assignments)
            .service(read_assignment)
            .service(create_assignment)
            .service(update_assignment)
            .service(delete_assignment)
            .configure(crate::assessment_routes::routes),
    );
}

#[cfg(test)]
mod tests {
    use super::{bounded_page, trimmed};

    #[test]
    fn directory_filters_are_bounded_and_blank_search_is_ignored() {
        assert_eq!(bounded_page(Some(-2), Some(500)), (1, 100));
        assert_eq!(bounded_page(None, None), (1, 25));
        assert_eq!(trimmed(Some("  ")), None);
        assert_eq!(trimmed(Some(" Math ")), Some("Math"));
    }
}
