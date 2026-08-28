export interface TimetableDay {
  key: string;
  label: string;
}

export interface TimetablePeriod {
  key: string;
  label: string;
  start_time: string | null;
  end_time: string | null;
}

export interface NamedResource {
  id: string;
  name: string;
}

export interface TeacherResource extends NamedResource {
  unavailable_slots: string[];
}

export interface AcademicPeriodResource {
  academic_year_id: string;
  academic_year_name: string;
  academic_term_id: string;
  academic_term_name: string;
  starts_on: string;
  ends_on: string;
}

export interface WorkforceAvailabilityConstraint {
  id: string;
  teacher_id: string;
  employee_id: string;
  kind: string;
  starts_at: string;
  ends_at: string;
}

export interface LessonRequirement {
  id: string;
  class_id: string;
  subject_id: string;
  teacher_id: string;
  room_id: string | null;
  periods_per_cycle: number;
}

export interface TimetableConfiguration {
  cycle_name: string;
  academic_period: AcademicPeriodResource | null;
  workforce_constraints: WorkforceAvailabilityConstraint[];
  days: TimetableDay[];
  periods: TimetablePeriod[];
  classes: NamedResource[];
  subjects: NamedResource[];
  teachers: TeacherResource[];
  rooms: NamedResource[];
  lesson_requirements: LessonRequirement[];
}

export interface TimetableEntry {
  requirement_id: string;
  day_key: string;
  period_key: string;
  class_id: string;
  subject_id: string;
  teacher_id: string;
  room_id: string | null;
}

export interface UnresolvedLesson {
  requirement_id: string;
  reason: string;
}

export interface TimetableRun {
  id: string;
  status: "draft" | "published" | "superseded";
  configuration: TimetableConfiguration;
  entries: TimetableEntry[];
  unresolved: UnresolvedLesson[];
  quality_score: number;
  created_at: string;
  published_at: string | null;
}

export interface TimetableRunSummary {
  id: string;
  status: "draft" | "published" | "superseded";
  academic_year_name: string | null;
  academic_term_name: string | null;
  entry_count: number;
  unresolved_count: number;
  quality_score: number;
  created_at: string;
  published_at: string | null;
}
