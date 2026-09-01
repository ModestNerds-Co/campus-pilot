export type PatientKind = "learner" | "employee";
export type PatientStatus = "active" | "inactive";
export type CareItemKind = "allergy" | "condition" | "accommodation" | "action_plan";
export type CareSeverity = "low" | "moderate" | "high" | "critical";
export type VisitCategory = "illness" | "injury" | "medication" | "wellbeing" | "follow_up" | "other";
export type VisitDisposition = "returned_to_class" | "sent_home" | "emergency_referral" | "guardian_collection" | "staff_released" | "other";
export type MedicationPlanStatus = "active" | "suspended" | "ended";
export type FollowUpStatus = "open" | "completed" | "cancelled";

export interface PaginationMeta {
  current_page: number;
  per_page: number;
  total: number;
  total_pages: number;
  has_next: boolean;
  has_prev: boolean;
}
export interface ApiEnvelope<T> {
  success: boolean;
  message: string | null;
  data: T | null;
  pagination: PaginationMeta | null;
  issues: Array<string | { detail?: string }> | null;
}
export interface PatientCandidate {
  kind: PatientKind;
  id: string;
  number: string;
  display_name: string;
  source_status: string;
  already_patient: boolean;
}
export interface EmployeeCandidate { id: string; number: string; display_name: string }
export interface HealthReferences { patients: PatientCandidate[]; employees: EmployeeCandidate[] }
export interface PatientSummary {
  id: string;
  person_kind: PatientKind;
  person_id: string;
  person_number: string;
  person_name: string;
  source_status: string;
  status: PatientStatus;
  version: number;
  active_care_item_count: number;
  open_visit_count: number;
  active_medication_count: number;
  open_follow_up_count: number;
  created_at: string;
  updated_at: string;
}
export interface GuardianContact {
  guardian_id: string;
  display_name: string;
  relationship_type: string;
  is_primary: boolean;
  can_collect: boolean;
  phone: string | null;
  email: string | null;
}
export interface CareItem {
  id: string;
  patient_id: string;
  kind: CareItemKind;
  title: string;
  details: string | null;
  severity: CareSeverity;
  status: "active" | "resolved";
  reviewed_on: string | null;
  version: number;
  created_at: string;
  updated_at: string;
}
export interface PatientRecord extends PatientSummary {
  guardian_contacts: GuardianContact[];
  care_items: CareItem[];
}
export interface Visit {
  id: string;
  patient_id: string;
  patient_kind: PatientKind;
  patient_number: string;
  patient_name: string;
  checked_in_at: string;
  category: VisitCategory;
  presenting_concern: string;
  assessment: string | null;
  care_given: string | null;
  disposition: VisitDisposition | null;
  status: "open" | "closed";
  version: number;
  closed_at: string | null;
  created_at: string;
  updated_at: string;
}
export interface MedicationPlan {
  id: string;
  patient_id: string;
  patient_kind: PatientKind;
  patient_number: string;
  patient_name: string;
  medication_name: string;
  dosage: string;
  route: string;
  schedule: string;
  instructions: string | null;
  authorization_reference: string;
  starts_on: string;
  ends_on: string | null;
  status: MedicationPlanStatus;
  version: number;
  created_at: string;
  updated_at: string;
}
export interface MedicationAdministration {
  id: string;
  medication_plan_id: string;
  patient_id: string;
  patient_number: string;
  patient_name: string;
  medication_name: string;
  administered_at: string;
  dose: string;
  outcome: "given" | "refused" | "missed" | "held";
  note: string | null;
  created_at: string;
}
export interface FollowUp {
  id: string;
  patient_id: string;
  patient_kind: PatientKind;
  patient_number: string;
  patient_name: string;
  visit_id: string | null;
  assigned_employee_id: string | null;
  assigned_employee_name: string | null;
  due_on: string;
  purpose: string;
  status: FollowUpStatus;
  outcome: string | null;
  version: number;
  completed_at: string | null;
  created_at: string;
  updated_at: string;
}
export interface PatientsResponse { patients: PatientSummary[] }
export interface VisitsResponse { visits: Visit[] }
export interface MedicationPlansResponse { medication_plans: MedicationPlan[] }
export interface MedicationAdministrationsResponse { administrations: MedicationAdministration[] }
export interface FollowUpsResponse { follow_ups: FollowUp[] }
