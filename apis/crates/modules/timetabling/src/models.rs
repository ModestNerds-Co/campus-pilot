use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimetableDay {
    pub key: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimetablePeriod {
    pub key: String,
    pub label: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NamedResource {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeacherResource {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub unavailable_slots: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcademicPeriodResource {
    pub academic_year_id: Uuid,
    pub academic_year_name: String,
    pub academic_term_id: Uuid,
    pub academic_term_name: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkforceAvailabilityConstraint {
    pub id: Uuid,
    pub teacher_id: Uuid,
    pub employee_id: Uuid,
    pub kind: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LessonRequirement {
    pub id: String,
    pub class_id: String,
    pub subject_id: String,
    pub teacher_id: String,
    pub room_id: Option<String>,
    pub periods_per_cycle: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimetableConfiguration {
    pub cycle_name: String,
    #[serde(default)]
    pub academic_period: Option<AcademicPeriodResource>,
    #[serde(default)]
    pub workforce_constraints: Vec<WorkforceAvailabilityConstraint>,
    pub days: Vec<TimetableDay>,
    pub periods: Vec<TimetablePeriod>,
    pub classes: Vec<NamedResource>,
    pub subjects: Vec<NamedResource>,
    pub teachers: Vec<TeacherResource>,
    pub rooms: Vec<NamedResource>,
    pub lesson_requirements: Vec<LessonRequirement>,
}

impl Default for TimetableConfiguration {
    fn default() -> Self {
        let days = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday"]
            .into_iter()
            .map(|label| TimetableDay {
                key: label.to_lowercase(),
                label: label.to_string(),
            })
            .collect();
        let periods = (1..=8)
            .map(|number| TimetablePeriod {
                key: format!("period-{number}"),
                label: format!("Period {number}"),
                start_time: None,
                end_time: None,
            })
            .collect();
        Self {
            cycle_name: "Current academic cycle".to_string(),
            academic_period: None,
            workforce_constraints: Vec::new(),
            days,
            periods,
            classes: Vec::new(),
            subjects: Vec::new(),
            teachers: Vec::new(),
            rooms: Vec::new(),
            lesson_requirements: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimetableEntry {
    pub requirement_id: String,
    pub day_key: String,
    pub period_key: String,
    pub class_id: String,
    pub subject_id: String,
    pub teacher_id: String,
    pub room_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnresolvedLesson {
    pub requirement_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimetableRun {
    pub id: Uuid,
    pub status: String,
    pub configuration: TimetableConfiguration,
    pub entries: Vec<TimetableEntry>,
    pub unresolved: Vec<UnresolvedLesson>,
    pub quality_score: i32,
    pub created_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct TimetableRunRow {
    pub id: Uuid,
    pub status: String,
    pub configuration_snapshot: serde_json::Value,
    pub entries: serde_json::Value,
    pub unresolved: serde_json::Value,
    pub quality_score: i32,
    pub created_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TimetableRunSummary {
    pub id: Uuid,
    pub status: String,
    pub academic_year_name: Option<String>,
    pub academic_term_name: Option<String>,
    pub entry_count: i64,
    pub unresolved_count: i64,
    pub quality_score: i32,
    pub created_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
}

impl TryFrom<TimetableRunRow> for TimetableRun {
    type Error = serde_json::Error;

    fn try_from(row: TimetableRunRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            status: row.status,
            configuration: serde_json::from_value(row.configuration_snapshot)?,
            entries: serde_json::from_value(row.entries)?,
            unresolved: serde_json::from_value(row.unresolved)?,
            quality_score: row.quality_score,
            created_at: row.created_at,
            published_at: row.published_at,
        })
    }
}
