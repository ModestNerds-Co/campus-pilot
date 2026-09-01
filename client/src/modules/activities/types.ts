/** Closed client contracts for Activities operations. */

export type ActivityCategory = "sport" | "club" | "arts" | "service" | "society" | "academic_enrichment" | "other";
export type ActivityCatalogStatus = "active" | "archived";
export type ActivityGroupStatus = "draft" | "active" | "closed" | "cancelled";
export type ActivityLeaderRole = "lead" | "leader" | "assistant";
export type ActivityMembershipStatus = "active" | "ended" | "withdrawn";
export type ActivityConsentStatus = "not_required" | "pending" | "granted" | "declined";
export type ActivitySessionStatus = "scheduled" | "completed" | "cancelled";
export type ActivityParticipationMark = "present" | "absent" | "late" | "excused" | "not_required";

export interface PaginationMeta { current_page: number; per_page: number; total: number; total_pages: number; has_next: boolean; has_prev: boolean; }
export interface ApiEnvelope<T> { success: boolean; message: string | null; data: T | null; pagination: PaginationMeta | null; issues: Array<string | { detail?: string }> | null; }

export interface ActivityCatalogItem {
  id: string; code: string; name: string; category: ActivityCategory; description: string | null;
  status: ActivityCatalogStatus; version: number; created_at: string; updated_at: string;
}

export interface ActivityGroupSummary {
  id: string; activity_id: string; activity_code: string; activity_name: string; code: string; name: string;
  starts_on: string; ends_on: string; capacity: number | null; consent_required: boolean;
  status: ActivityGroupStatus; leader_count: number; member_count: number; session_count: number;
  version: number; created_at: string; updated_at: string;
}

export interface ActivityLeader {
  id: string; employee_id: string; employee_number: string; employee_name: string; role: ActivityLeaderRole;
  starts_on: string; ends_on: string | null; ended_at: string | null; end_reason: string | null; version: number;
}

export interface ActivityMembership {
  id: string; learner_id: string; learner_number: string; learner_name: string; joined_on: string;
  ended_on: string | null; status: ActivityMembershipStatus; consent_status: ActivityConsentStatus;
  consent_recorded_at: string | null; consent_notes: string | null; version: number;
}

export interface ActivityEvent { id: string; event_type: string; actor_name: string; metadata: Record<string, unknown>; created_at: string; }

export interface ActivityGroupRecord extends ActivityGroupSummary {
  consent_instructions: string | null; leaders: ActivityLeader[]; memberships: ActivityMembership[]; history: ActivityEvent[];
}

export interface ActivitySessionSummary {
  id: string; reference: string; group_id: string; group_code: string; group_name: string; title: string;
  starts_at: string; ends_at: string; location_note: string | null; status: ActivitySessionStatus;
  roster_count: number; marked_count: number; present_count: number; absent_count: number;
  version: number; created_at: string; updated_at: string;
}

export interface ActivityParticipation {
  membership_id: string; learner_id: string; learner_number: string; learner_name: string;
  mark: ActivityParticipationMark | null; notes: string | null; version: number | null; marked_at: string | null;
}

export interface ActivitySessionRecord extends ActivitySessionSummary {
  notes: string | null; completion_summary: string | null; cancellation_reason: string | null;
  participation: ActivityParticipation[]; history: ActivityEvent[];
}

export interface ActivityLearnerReference { id: string; learner_number: string; display_name: string; status: string; }
export interface ActivityEmployeeReference { id: string; account_id: string | null; employee_number: string; display_name: string; employment_status: string; }
export interface ActivitiesReferences { learners: ActivityLearnerReference[]; employees: ActivityEmployeeReference[]; }

export interface CatalogPayload { code: string; name: string; category: ActivityCategory; description: string | null; }
export interface GroupPayload {
  activity_id: string; code: string; name: string; starts_on: string; ends_on: string;
  capacity: number | null; consent_required: boolean; consent_instructions: string | null;
}
export interface SessionPayload { group_id: string; title: string; starts_at: string; ends_at: string; location_note: string | null; notes: string | null; }
