//! Timetabling domain operations and canonical Academics hydration.
//!
//! Timetabling owns scheduling settings and immutable run snapshots. Academic
//! years, classes, subjects, teachers, and teaching loads remain Academics-owned.

use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, NaiveTime, Utc};
use cp_academics::{models::TimetablingReferenceData, ops::TeachingAssignmentOps};
use cp_hr_payroll::{models::EmployeeAvailabilityReference, ops::EmployeeAvailabilityOps};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    generator::{generate, validate_configuration},
    models::{
        AcademicPeriodResource, LessonRequirement, NamedResource, TeacherResource,
        TimetableConfiguration, TimetableRun, TimetableRunRow, TimetableRunSummary,
        WorkforceAvailabilityConstraint,
    },
};

pub struct TimetablingOps;

impl TimetablingOps {
    /// Loads scheduling settings and rehydrates canonical academic resources.
    pub async fn get_configuration(
        pool: &PgPool,
        tenant_id: Uuid,
    ) -> Result<TimetableConfiguration> {
        let stored = load_stored_configuration(pool, tenant_id).await?;
        let references = TeachingAssignmentOps::timetabling_reference_data(pool, tenant_id).await?;
        let availability = load_workforce_constraints(pool, tenant_id, references.as_ref()).await?;
        Ok(hydrate_configuration(stored, references, availability))
    }

    /// Saves scheduling-owned settings after replacing client academic copies.
    pub async fn save_configuration(
        pool: &PgPool,
        tenant_id: Uuid,
        requested: TimetableConfiguration,
    ) -> Result<TimetableConfiguration> {
        let references = TeachingAssignmentOps::timetabling_reference_data(pool, tenant_id).await?;
        let availability = load_workforce_constraints(pool, tenant_id, references.as_ref()).await?;
        let configuration = hydrate_configuration(requested, references, availability);
        if let Err(issues) = validate_configuration(&configuration) {
            bail!(issues.join("\n"));
        }
        sqlx::query(
            r#"
            INSERT INTO timetable_configurations (
                tenant_id, cycle_name, days, periods, classes, subjects,
                teachers, rooms, lesson_requirements
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (tenant_id) DO UPDATE SET
                cycle_name = EXCLUDED.cycle_name,
                days = EXCLUDED.days,
                periods = EXCLUDED.periods,
                classes = EXCLUDED.classes,
                subjects = EXCLUDED.subjects,
                teachers = EXCLUDED.teachers,
                rooms = EXCLUDED.rooms,
                lesson_requirements = EXCLUDED.lesson_requirements,
                updated_at = NOW()
            "#,
        )
        .bind(tenant_id)
        .bind(&configuration.cycle_name)
        .bind(serde_json::to_value(&configuration.days)?)
        .bind(serde_json::to_value(&configuration.periods)?)
        .bind(serde_json::to_value(&configuration.classes)?)
        .bind(serde_json::to_value(&configuration.subjects)?)
        .bind(serde_json::to_value(&configuration.teachers)?)
        .bind(serde_json::to_value(&configuration.rooms)?)
        .bind(serde_json::to_value(&configuration.lesson_requirements)?)
        .execute(pool)
        .await
        .context("Failed to save timetable configuration")?;
        Ok(configuration)
    }

    pub async fn generate(pool: &PgPool, tenant_id: Uuid) -> Result<TimetableRun> {
        let configuration = Self::get_configuration(pool, tenant_id).await?;
        if configuration.academic_period.is_none() {
            bail!("Activate an academic term in Academics before generating a timetable");
        }
        if configuration.lesson_requirements.is_empty() {
            bail!("Add active teaching assignments in Academics before generating a timetable");
        }
        if let Err(issues) = validate_configuration(&configuration) {
            bail!(issues.join("\n"));
        }
        let generated = generate(&configuration);
        let row = sqlx::query_as::<_, TimetableRunRow>(
            r#"
            INSERT INTO timetable_runs (
                tenant_id, configuration_snapshot, entries, unresolved, quality_score
            )
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, status, configuration_snapshot, entries, unresolved,
                      quality_score, created_at, published_at
            "#,
        )
        .bind(tenant_id)
        .bind(serde_json::to_value(&configuration)?)
        .bind(serde_json::to_value(&generated.entries)?)
        .bind(serde_json::to_value(&generated.unresolved)?)
        .bind(generated.quality_score)
        .fetch_one(pool)
        .await
        .context("Failed to create timetable run")?;
        TimetableRun::try_from(row).context("Failed to decode timetable run")
    }

    pub async fn latest_run(pool: &PgPool, tenant_id: Uuid) -> Result<Option<TimetableRun>> {
        let row = sqlx::query_as::<_, TimetableRunRow>(
            r#"
            SELECT id, status, configuration_snapshot, entries, unresolved,
                   quality_score, created_at, published_at
            FROM timetable_runs
            WHERE tenant_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(tenant_id)
        .fetch_optional(pool)
        .await
        .context("Failed to load latest timetable run")?;
        row.map(TimetableRun::try_from)
            .transpose()
            .context("Failed to decode latest timetable run")
    }

    pub async fn list_runs(
        pool: &PgPool,
        tenant_id: Uuid,
        page: i64,
        per_page: i64,
        status: Option<&str>,
    ) -> Result<(Vec<TimetableRunSummary>, i64)> {
        validate_run_status(status)?;
        let page = page.max(1);
        let per_page = per_page.clamp(1, 100);
        let offset = (page - 1) * per_page;
        let runs = sqlx::query_as::<_, TimetableRunSummary>(
            r#"
            SELECT id, status,
                   configuration_snapshot -> 'academic_period' ->> 'academic_year_name'
                       AS academic_year_name,
                   configuration_snapshot -> 'academic_period' ->> 'academic_term_name'
                       AS academic_term_name,
                   COALESCE(jsonb_array_length(entries), 0)::BIGINT AS entry_count,
                   COALESCE(jsonb_array_length(unresolved), 0)::BIGINT AS unresolved_count,
                   quality_score, created_at, published_at
            FROM timetable_runs
            WHERE tenant_id = $1 AND ($2::TEXT IS NULL OR status = $2)
            ORDER BY created_at DESC, id
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(tenant_id)
        .bind(status)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await
        .context("Failed to list timetable runs")?;
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM timetable_runs WHERE tenant_id = $1 AND ($2::TEXT IS NULL OR status = $2)",
        )
        .bind(tenant_id)
        .bind(status)
        .fetch_one(pool)
        .await
        .context("Failed to count timetable runs")?;
        Ok((runs, total))
    }

    pub async fn get_run(
        pool: &PgPool,
        tenant_id: Uuid,
        run_id: Uuid,
    ) -> Result<Option<TimetableRun>> {
        let row = sqlx::query_as::<_, TimetableRunRow>(
            r#"
            SELECT id, status, configuration_snapshot, entries, unresolved,
                   quality_score, created_at, published_at
            FROM timetable_runs
            WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(run_id)
        .fetch_optional(pool)
        .await
        .context("Failed to load timetable run")?;
        row.map(TimetableRun::try_from)
            .transpose()
            .context("Failed to decode timetable run")
    }

    pub async fn publish(
        pool: &PgPool,
        tenant_id: Uuid,
        run_id: Uuid,
    ) -> Result<Option<TimetableRun>> {
        let unresolved: Option<serde_json::Value> = sqlx::query_scalar(
            "SELECT unresolved FROM timetable_runs WHERE id = $1 AND tenant_id = $2",
        )
        .bind(run_id)
        .bind(tenant_id)
        .fetch_optional(pool)
        .await
        .context("Failed to load timetable run")?;
        let Some(unresolved) = unresolved else {
            return Ok(None);
        };
        if unresolved.as_array().is_some_and(|items| !items.is_empty()) {
            bail!("Resolve every unplaced lesson before publishing");
        }
        let mut transaction = pool.begin().await.context("Failed to begin publication")?;
        sqlx::query(
            "UPDATE timetable_runs SET status = 'superseded' WHERE tenant_id = $1 AND status = 'published'",
        )
        .bind(tenant_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to supersede the previous timetable")?;
        let row = sqlx::query_as::<_, TimetableRunRow>(
            r#"
            UPDATE timetable_runs
            SET status = 'published', published_at = NOW()
            WHERE id = $1 AND tenant_id = $2
            RETURNING id, status, configuration_snapshot, entries, unresolved,
                      quality_score, created_at, published_at
            "#,
        )
        .bind(run_id)
        .bind(tenant_id)
        .fetch_one(&mut *transaction)
        .await
        .context("Failed to publish timetable run")?;
        transaction
            .commit()
            .await
            .context("Failed to commit timetable publication")?;
        Ok(Some(
            TimetableRun::try_from(row).context("Failed to decode published timetable run")?,
        ))
    }
}

async fn load_stored_configuration(
    pool: &PgPool,
    tenant_id: Uuid,
) -> Result<TimetableConfiguration> {
    let row = sqlx::query(
        r#"
        SELECT cycle_name, days, periods, classes, subjects, teachers, rooms,
               lesson_requirements
        FROM timetable_configurations
        WHERE tenant_id = $1
        "#,
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .context("Failed to load timetable configuration")?;
    let Some(row) = row else {
        return Ok(TimetableConfiguration::default());
    };
    Ok(TimetableConfiguration {
        cycle_name: row.try_get("cycle_name")?,
        academic_period: None,
        workforce_constraints: Vec::new(),
        days: serde_json::from_value(row.try_get("days")?)?,
        periods: serde_json::from_value(row.try_get("periods")?)?,
        classes: serde_json::from_value(row.try_get("classes")?)?,
        subjects: serde_json::from_value(row.try_get("subjects")?)?,
        teachers: serde_json::from_value(row.try_get("teachers")?)?,
        rooms: serde_json::from_value(row.try_get("rooms")?)?,
        lesson_requirements: serde_json::from_value(row.try_get("lesson_requirements")?)?,
    })
}

fn hydrate_configuration(
    mut configuration: TimetableConfiguration,
    references: Option<TimetablingReferenceData>,
    workforce_constraints: Vec<WorkforceAvailabilityConstraint>,
) -> TimetableConfiguration {
    let Some(references) = references else {
        configuration.academic_period = None;
        configuration.workforce_constraints.clear();
        configuration.classes.clear();
        configuration.subjects.clear();
        configuration.teachers.clear();
        configuration.lesson_requirements.clear();
        return configuration;
    };
    let unavailability = configuration
        .teachers
        .into_iter()
        .map(|teacher| (teacher.id, teacher.unavailable_slots))
        .collect::<HashMap<_, _>>();
    let assigned_rooms = configuration
        .lesson_requirements
        .into_iter()
        .map(|requirement| (requirement.id, requirement.room_id))
        .collect::<HashMap<_, _>>();

    configuration.academic_period =
        references
            .active_term
            .as_ref()
            .map(|term| AcademicPeriodResource {
                academic_year_id: references.academic_year.id,
                academic_year_name: references.academic_year.name.clone(),
                academic_term_id: term.id,
                academic_term_name: term.name.clone(),
                starts_on: term.starts_on,
                ends_on: term.ends_on,
            });
    configuration.workforce_constraints = workforce_constraints;
    configuration.cycle_name = references.active_term.as_ref().map_or_else(
        || references.academic_year.name.clone(),
        |term| format!("{} · {}", references.academic_year.name, term.name),
    );
    configuration.classes = references
        .classes
        .into_iter()
        .map(|class_group| NamedResource {
            id: class_group.id.to_string(),
            name: class_group.name,
        })
        .collect();
    configuration.subjects = references
        .subjects
        .into_iter()
        .map(|subject| NamedResource {
            id: subject.id.to_string(),
            name: subject.name,
        })
        .collect();
    configuration.teachers = references
        .teachers
        .into_iter()
        .map(|teacher| TeacherResource {
            id: teacher.id.to_string(),
            name: teacher.display_name,
            unavailable_slots: unavailability
                .get(&teacher.id.to_string())
                .cloned()
                .unwrap_or_default(),
        })
        .collect();
    configuration.lesson_requirements = references
        .assignments
        .into_iter()
        .map(|assignment| {
            let id = assignment.id.to_string();
            LessonRequirement {
                room_id: assigned_rooms.get(&id).cloned().flatten(),
                id,
                class_id: assignment.class_group_id.to_string(),
                subject_id: assignment.subject_id.to_string(),
                teacher_id: assignment.teacher_profile_id.to_string(),
                periods_per_cycle: assignment.periods_per_cycle as u16,
            }
        })
        .collect();
    configuration
}

async fn load_workforce_constraints(
    pool: &PgPool,
    tenant_id: Uuid,
    references: Option<&TimetablingReferenceData>,
) -> Result<Vec<WorkforceAvailabilityConstraint>> {
    let Some(references) = references else {
        return Ok(Vec::new());
    };
    let Some(term) = references.active_term.as_ref() else {
        return Ok(Vec::new());
    };
    let employee_to_teacher = references
        .teachers
        .iter()
        .map(|teacher| (teacher.employee_id, teacher.id))
        .collect::<HashMap<_, _>>();
    let employee_ids = employee_to_teacher.keys().copied().collect::<Vec<_>>();
    let starts_at =
        DateTime::<Utc>::from_naive_utc_and_offset(term.starts_on.and_time(NaiveTime::MIN), Utc);
    let end_of_day =
        NaiveTime::from_hms_nano_opt(23, 59, 59, 999_999_999).unwrap_or_else(|| unreachable!());
    let ends_at =
        DateTime::<Utc>::from_naive_utc_and_offset(term.ends_on.and_time(end_of_day), Utc);
    let availability = EmployeeAvailabilityOps::list_approved_for_window(
        pool,
        tenant_id,
        &employee_ids,
        starts_at,
        ends_at,
    )
    .await?;
    Ok(map_workforce_constraints(
        availability,
        &employee_to_teacher,
    ))
}

fn map_workforce_constraints(
    availability: Vec<EmployeeAvailabilityReference>,
    employee_to_teacher: &HashMap<Uuid, Uuid>,
) -> Vec<WorkforceAvailabilityConstraint> {
    availability
        .into_iter()
        .filter_map(|period| {
            employee_to_teacher
                .get(&period.employee_id)
                .copied()
                .map(|teacher_id| WorkforceAvailabilityConstraint {
                    id: period.id,
                    teacher_id,
                    employee_id: period.employee_id,
                    kind: period.kind,
                    starts_at: period.starts_at,
                    ends_at: period.ends_at,
                })
        })
        .collect()
}

fn validate_run_status(status: Option<&str>) -> Result<()> {
    if status.is_some_and(|value| !matches!(value, "draft" | "published" | "superseded")) {
        bail!("Timetable run status is invalid");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, Utc};
    use cp_academics::models::{
        AcademicTerm, AcademicYear, ClassGroupWithYear, Subject, TeacherProfileWithEmployee,
        TeachingAssignmentWithDetails, TimetablingReferenceData,
    };
    use uuid::Uuid;

    use crate::models::{TeacherResource, TimetableConfiguration};

    use super::hydrate_configuration;

    #[test]
    fn canonical_academics_replace_duplicate_timetable_people_and_names() {
        let year_id = Uuid::new_v4();
        let class_id = Uuid::new_v4();
        let subject_id = Uuid::new_v4();
        let teacher_id = Uuid::new_v4();
        let employee_id = Uuid::new_v4();
        let assignment_id = Uuid::new_v4();
        let now = Utc::now();
        let configuration = TimetableConfiguration {
            teachers: vec![TeacherResource {
                id: teacher_id.to_string(),
                name: "Stale copied name".to_string(),
                unavailable_slots: vec!["monday:period-1".to_string()],
            }],
            ..TimetableConfiguration::default()
        };
        let references = TimetablingReferenceData {
            academic_year: AcademicYear {
                id: year_id,
                tenant_id: Uuid::new_v4(),
                name: "2027".to_string(),
                starts_on: NaiveDate::from_ymd_opt(2027, 1, 1).unwrap_or_else(|| unreachable!()),
                ends_on: NaiveDate::from_ymd_opt(2027, 12, 31).unwrap_or_else(|| unreachable!()),
                status: "active".to_string(),
                created_at: now,
                updated_at: now,
                deleted_at: None,
            },
            active_term: Some(AcademicTerm {
                id: Uuid::new_v4(),
                tenant_id: Uuid::new_v4(),
                academic_year_id: year_id,
                academic_year_name: "2027".to_string(),
                code: "T1".to_string(),
                name: "Term 1".to_string(),
                starts_on: NaiveDate::from_ymd_opt(2027, 1, 11).unwrap_or_else(|| unreachable!()),
                ends_on: NaiveDate::from_ymd_opt(2027, 4, 2).unwrap_or_else(|| unreachable!()),
                status: "active".to_string(),
                created_at: now,
                updated_at: now,
                deleted_at: None,
            }),
            classes: vec![ClassGroupWithYear {
                id: class_id,
                tenant_id: Uuid::new_v4(),
                academic_year_id: year_id,
                academic_year_name: "2027".to_string(),
                code: "F1A".to_string(),
                name: "Form 1A".to_string(),
                grade_level: Some("Form 1".to_string()),
                status: "active".to_string(),
                created_at: now,
                updated_at: now,
            }],
            subjects: vec![Subject {
                id: subject_id,
                tenant_id: Uuid::new_v4(),
                code: "MATH".to_string(),
                name: "Mathematics".to_string(),
                status: "active".to_string(),
                created_at: now,
                updated_at: now,
                deleted_at: None,
            }],
            teachers: vec![TeacherProfileWithEmployee {
                id: teacher_id,
                tenant_id: Uuid::new_v4(),
                employee_id,
                employee_number: "EMP-1".to_string(),
                display_name: "Canonical Teacher".to_string(),
                work_email: None,
                phone: None,
                employment_status: "active".to_string(),
                status: "active".to_string(),
                created_at: now,
                updated_at: now,
            }],
            assignments: vec![TeachingAssignmentWithDetails {
                id: assignment_id,
                tenant_id: Uuid::new_v4(),
                academic_year_id: year_id,
                academic_year_name: "2027".to_string(),
                class_group_id: class_id,
                class_group_name: "Form 1A".to_string(),
                subject_id,
                subject_name: "Mathematics".to_string(),
                teacher_profile_id: teacher_id,
                employee_id,
                teacher_name: "Canonical Teacher".to_string(),
                periods_per_cycle: 5,
                status: "active".to_string(),
                created_at: now,
                updated_at: now,
            }],
        };

        let hydrated = hydrate_configuration(configuration, Some(references), Vec::new());
        assert_eq!(hydrated.cycle_name, "2027 · Term 1");
        assert_eq!(
            hydrated
                .academic_period
                .as_ref()
                .map(|period| period.academic_term_name.as_str()),
            Some("Term 1")
        );
        assert_eq!(hydrated.teachers[0].name, "Canonical Teacher");
        assert_eq!(hydrated.teachers[0].unavailable_slots.len(), 1);
        assert_eq!(hydrated.lesson_requirements[0].periods_per_cycle, 5);
    }
}
