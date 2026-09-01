/** Closed client contracts for the restricted Student Support module. */

export type ConcernCategory =
  | "wellbeing"
  | "behaviour"
  | "conduct"
  | "safeguarding"
  | "family"
  | "learning_support"
  | "other";

export type CaseSeverity = "low" | "moderate" | "high" | "critical";
export type CaseStatus = "open" | "active" | "escalated" | "resolved" | "closed";
export type CaseActionKind = "note" | "contact" | "meeting" | "referral" | "support_plan" | "review";
export type CaseTeamRole = "member" | "reviewer";

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

export interface LearnerCandidate {
  learner_id: string;
  learner_number: string;
  display_name: string;
  status: string;
}

export interface CaseWorkerCandidate {
  user_id: string;
  full_name: string;
  email: string;
}

export interface StudentSupportReferences {
  learners: LearnerCandidate[];
  case_workers: CaseWorkerCandidate[];
}

export interface CaseSummary {
  id: string;
  reference: string;
  learner_id: string;
  learner_number: string;
  learner_name: string;
  lead_case_worker_user_id: string;
  lead_case_worker_name: string;
  category: ConcernCategory;
  severity: CaseSeverity;
  title: string;
  occurred_on: string | null;
  status: CaseStatus;
  version: number;
  action_count: number;
  team_member_count: number;
  updated_at: string;
}

export interface CaseTeamMember {
  user_id: string;
  full_name: string;
  email: string;
  member_role: "lead" | CaseTeamRole;
  assigned_at: string;
}

export interface CaseEvent {
  id: string;
  case_id: string;
  event_type: string;
  actor_id: string;
  actor_name: string;
  metadata: Record<string, unknown>;
  created_at: string;
}

export interface CaseRecord extends CaseSummary {
  summary: string;
  escalation_reason: string | null;
  escalated_at: string | null;
  resolution_summary: string | null;
  resolved_at: string | null;
  closure_reason: string | null;
  closed_at: string | null;
  team: CaseTeamMember[];
  history: CaseEvent[];
  created_at: string;
}

export interface CaseAction {
  id: string;
  case_id: string;
  action_kind: CaseActionKind;
  summary: string;
  details: string | null;
  occurred_at: string;
  created_by: string;
  created_by_name: string;
  created_at: string;
}

export interface CasesResponse { cases: CaseSummary[] }
export interface CaseActionsResponse { actions: CaseAction[] }

export interface CasePayload {
  learner_id: string;
  lead_case_worker_user_id: string | null;
  category: ConcernCategory;
  severity: CaseSeverity;
  title: string;
  summary: string;
  occurred_on: string | null;
}

export interface UpdateCasePayload extends Omit<CasePayload, "learner_id" | "lead_case_worker_user_id"> {
  expected_version: number;
}
