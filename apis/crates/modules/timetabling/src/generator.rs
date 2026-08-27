use std::collections::{HashMap, HashSet};

use crate::models::{TimetableConfiguration, TimetableEntry, UnresolvedLesson};

#[derive(Debug, Clone)]
pub struct GenerationResult {
    pub entries: Vec<TimetableEntry>,
    pub unresolved: Vec<UnresolvedLesson>,
    pub quality_score: i32,
}

pub fn validate_configuration(config: &TimetableConfiguration) -> Result<(), Vec<String>> {
    let mut issues = Vec::new();
    if config.cycle_name.trim().is_empty() {
        issues.push("Academic cycle name is required".to_string());
    }
    if config.days.is_empty() || config.days.len() > 7 {
        issues.push("Configure between 1 and 7 teaching days".to_string());
    }
    if config.periods.is_empty() || config.periods.len() > 16 {
        issues.push("Configure between 1 and 16 periods per day".to_string());
    }
    if config.lesson_requirements.len() > 1_000 {
        issues.push("A timetable may contain at most 1,000 lesson requirements".to_string());
    }

    validate_unique(
        &config
            .days
            .iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>(),
        "teaching day",
        &mut issues,
    );
    validate_unique(
        &config
            .periods
            .iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>(),
        "period",
        &mut issues,
    );
    validate_unique(
        &config
            .classes
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        "class",
        &mut issues,
    );
    validate_unique(
        &config
            .subjects
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        "subject",
        &mut issues,
    );
    validate_unique(
        &config
            .teachers
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        "teacher",
        &mut issues,
    );
    validate_unique(
        &config
            .rooms
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        "room",
        &mut issues,
    );

    let classes: HashSet<_> = config.classes.iter().map(|item| item.id.as_str()).collect();
    let subjects: HashSet<_> = config
        .subjects
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    let teachers: HashSet<_> = config
        .teachers
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    let rooms: HashSet<_> = config.rooms.iter().map(|item| item.id.as_str()).collect();
    for lesson in &config.lesson_requirements {
        if lesson.periods_per_cycle == 0 || lesson.periods_per_cycle > 40 {
            issues.push(format!(
                "Lesson {} must request between 1 and 40 periods",
                lesson.id
            ));
        }
        if !classes.contains(lesson.class_id.as_str()) {
            issues.push(format!("Lesson {} references an unknown class", lesson.id));
        }
        if !subjects.contains(lesson.subject_id.as_str()) {
            issues.push(format!(
                "Lesson {} references an unknown subject",
                lesson.id
            ));
        }
        if !teachers.contains(lesson.teacher_id.as_str()) {
            issues.push(format!(
                "Lesson {} references an unknown teacher",
                lesson.id
            ));
        }
        if lesson
            .room_id
            .as_ref()
            .is_some_and(|id| !rooms.contains(id.as_str()))
        {
            issues.push(format!("Lesson {} references an unknown room", lesson.id));
        }
    }
    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

fn validate_unique(values: &[&str], label: &str, issues: &mut Vec<String>) {
    let mut seen = HashSet::new();
    if values
        .iter()
        .any(|value| value.trim().is_empty() || !seen.insert(*value))
    {
        issues.push(format!("Every {label} needs a unique, non-empty key"));
    }
}

pub fn generate(config: &TimetableConfiguration) -> GenerationResult {
    let teacher_unavailability: HashMap<&str, HashSet<&str>> = config
        .teachers
        .iter()
        .map(|teacher| {
            (
                teacher.id.as_str(),
                teacher
                    .unavailable_slots
                    .iter()
                    .map(String::as_str)
                    .collect(),
            )
        })
        .collect();
    let base_slots: Vec<_> = config
        .days
        .iter()
        .flat_map(|day| {
            config
                .periods
                .iter()
                .map(move |period| (day.key.as_str(), period.key.as_str()))
        })
        .collect();
    let mut lesson_instances: Vec<_> = config
        .lesson_requirements
        .iter()
        .flat_map(|lesson| (0..lesson.periods_per_cycle).map(move |_| lesson))
        .collect();
    lesson_instances.sort_by_key(|lesson| {
        let unavailable = teacher_unavailability
            .get(lesson.teacher_id.as_str())
            .map_or(0, HashSet::len);
        (
            base_slots.len().saturating_sub(unavailable),
            std::cmp::Reverse(lesson.periods_per_cycle),
        )
    });

    let attempts = base_slots.len().clamp(1, 64);
    let mut best: Option<GenerationResult> = None;
    for rotation in 0..attempts {
        let mut entries: Vec<TimetableEntry> = Vec::new();
        let mut unresolved = Vec::new();
        let mut occupied_classes = HashSet::new();
        let mut occupied_teachers = HashSet::new();
        let mut occupied_rooms = HashSet::new();
        let mut class_day_load: HashMap<(&str, &str), usize> = HashMap::new();
        let mut subject_days = HashSet::new();
        let mut score = 0;

        for lesson in &lesson_instances {
            let unavailable = teacher_unavailability.get(lesson.teacher_id.as_str());
            let mut candidates: Vec<(usize, &str, &str)> = base_slots
                .iter()
                .enumerate()
                .filter_map(|(index, &(day, period))| {
                    let slot_key = format!("{day}:{period}");
                    let available = !unavailable
                        .is_some_and(|slots| slots.contains(slot_key.as_str()))
                        && !occupied_classes.contains(&(lesson.class_id.as_str(), day, period))
                        && !occupied_teachers.contains(&(lesson.teacher_id.as_str(), day, period))
                        && !lesson.room_id.as_ref().is_some_and(|room| {
                            occupied_rooms.contains(&(room.as_str(), day, period))
                        });
                    available.then_some((index, day, period))
                })
                .collect();
            candidates.sort_by_key(|(index, day, _)| {
                let duplicate_subject_day = subject_days.contains(&(
                    lesson.class_id.as_str(),
                    lesson.subject_id.as_str(),
                    *day,
                ));
                let load = class_day_load
                    .get(&(lesson.class_id.as_str(), *day))
                    .copied()
                    .unwrap_or(0);
                (
                    duplicate_subject_day,
                    load,
                    (*index + rotation) % base_slots.len(),
                )
            });

            if let Some((period_index, day, period)) = candidates.first().copied() {
                occupied_classes.insert((lesson.class_id.as_str(), day, period));
                occupied_teachers.insert((lesson.teacher_id.as_str(), day, period));
                if let Some(room) = lesson.room_id.as_deref() {
                    occupied_rooms.insert((room, day, period));
                }
                *class_day_load
                    .entry((lesson.class_id.as_str(), day))
                    .or_insert(0) += 1;
                if !subject_days.insert((lesson.class_id.as_str(), lesson.subject_id.as_str(), day))
                {
                    score += 10;
                }
                score += (period_index % config.periods.len()) as i32;
                entries.push(TimetableEntry {
                    requirement_id: lesson.id.clone(),
                    day_key: day.to_string(),
                    period_key: period.to_string(),
                    class_id: lesson.class_id.clone(),
                    subject_id: lesson.subject_id.clone(),
                    teacher_id: lesson.teacher_id.clone(),
                    room_id: lesson.room_id.clone(),
                });
            } else {
                unresolved.push(UnresolvedLesson {
                    requirement_id: lesson.id.clone(),
                    reason: "No slot satisfies class, teacher, room, and availability constraints"
                        .to_string(),
                });
            }
        }

        let candidate = GenerationResult {
            entries,
            unresolved,
            quality_score: score,
        };
        let is_better = best.as_ref().is_none_or(|current| {
            (candidate.unresolved.len(), candidate.quality_score)
                < (current.unresolved.len(), current.quality_score)
        });
        if is_better {
            best = Some(candidate);
        }
    }
    best.unwrap_or(GenerationResult {
        entries: Vec::new(),
        unresolved: Vec::new(),
        quality_score: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{LessonRequirement, NamedResource, TeacherResource};

    fn resource(id: &str) -> NamedResource {
        NamedResource {
            id: id.to_string(),
            name: id.to_string(),
        }
    }

    #[test]
    fn avoids_teacher_and_class_collisions() {
        let config = TimetableConfiguration {
            classes: vec![resource("class-a"), resource("class-b")],
            subjects: vec![resource("math")],
            teachers: vec![TeacherResource {
                id: "teacher-a".into(),
                name: "Teacher A".into(),
                unavailable_slots: Vec::new(),
            }],
            lesson_requirements: vec![
                LessonRequirement {
                    id: "a".into(),
                    class_id: "class-a".into(),
                    subject_id: "math".into(),
                    teacher_id: "teacher-a".into(),
                    room_id: None,
                    periods_per_cycle: 5,
                },
                LessonRequirement {
                    id: "b".into(),
                    class_id: "class-b".into(),
                    subject_id: "math".into(),
                    teacher_id: "teacher-a".into(),
                    room_id: None,
                    periods_per_cycle: 5,
                },
            ],
            ..TimetableConfiguration::default()
        };
        let result = generate(&config);
        assert!(result.unresolved.is_empty());
        let slots: HashSet<_> = result
            .entries
            .iter()
            .map(|entry| (&entry.teacher_id, &entry.day_key, &entry.period_key))
            .collect();
        assert_eq!(slots.len(), result.entries.len());
    }

    #[test]
    fn reports_lessons_that_cannot_be_placed() {
        let mut config = TimetableConfiguration::default();
        config.days.truncate(1);
        config.periods.truncate(1);
        config.classes = vec![resource("class-a")];
        config.subjects = vec![resource("math")];
        config.teachers = vec![TeacherResource {
            id: "teacher-a".into(),
            name: "Teacher A".into(),
            unavailable_slots: Vec::new(),
        }];
        config.lesson_requirements = vec![LessonRequirement {
            id: "a".into(),
            class_id: "class-a".into(),
            subject_id: "math".into(),
            teacher_id: "teacher-a".into(),
            room_id: None,
            periods_per_cycle: 2,
        }];
        let result = generate(&config);
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.unresolved.len(), 1);
    }
}
