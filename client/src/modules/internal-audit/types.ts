// Typed client contracts for Internal Audit plans, engagements, evidence, and findings.

export type AuditStatus = "draft" | "approved" | "closed";
export type EngagementStatus = "planned" | "fieldwork" | "reporting" | "closed";
export type FindingStatus = "draft" | "issued";
export type FindingRating = "low" | "moderate" | "high" | "critical";

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

export interface NumberingPolicy {
  plan_prefix: string;
  engagement_prefix: string;
  finding_prefix: string;
  padding: number;
  next_plan_sequence: number;
  next_engagement_sequence: number;
  next_finding_sequence: number;
  next_plan_reference: string;
  next_engagement_reference: string;
  next_finding_reference: string;
  version: number;
  updated_at: string;
}

export interface AuditPlan {
  id: string;
  reference: string;
  title: string;
  objective: string;
  risk_summary: string | null;
  period_start: string;
  period_end: string;
  status: AuditStatus;
  version: number;
  engagement_count: number;
  approved_at: string | null;
  closed_at: string | null;
  close_summary: string | null;
  created_at: string;
  updated_at: string;
}

export interface AuditorCandidate {
  user_id: string;
  full_name: string;
  email: string;
}

export interface AuditEngagement {
  id: string;
  plan_id: string;
  plan_reference: string;
  plan_title: string;
  reference: string;
  title: string;
  objective: string;
  scope_text: string;
  lead_auditor_user_id: string;
  lead_auditor_name: string;
  lead_auditor_email: string;
  starts_on: string;
  due_on: string;
  status: EngagementStatus;
  version: number;
  finding_count: number;
  evidence_count: number;
  started_at: string | null;
  reporting_at: string | null;
  closed_at: string | null;
  close_summary: string | null;
  created_at: string;
  updated_at: string;
}

export interface AuditEvidence {
  id: string;
  engagement_id: string;
  document_file_id: string;
  document_reference: string;
  document_title: string;
  document_sensitivity: string;
  purpose: string;
  linked_at: string;
}

export interface AuditFinding {
  id: string;
  engagement_id: string;
  engagement_reference: string;
  engagement_title: string;
  reference: string;
  title: string;
  rating: FindingRating;
  criteria: string;
  condition: string;
  risk_effect: string;
  recommendation: string;
  status: FindingStatus;
  version: number;
  issued_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface PlansResponse { plans: AuditPlan[] }
export interface EngagementsResponse { engagements: AuditEngagement[] }
export interface EvidenceResponse { evidence: AuditEvidence[] }
export interface FindingsResponse { findings: AuditFinding[] }

export interface PlanPayload {
  title: string;
  objective: string;
  risk_summary: string | null;
  period_start: string;
  period_end: string;
}

export interface EngagementPayload {
  plan_id: string;
  title: string;
  objective: string;
  scope_text: string;
  lead_auditor_user_id: string;
  starts_on: string;
  due_on: string;
}

export interface FindingPayload {
  title: string;
  rating: FindingRating;
  criteria: string;
  condition: string;
  risk_effect: string;
  recommendation: string;
}
