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
