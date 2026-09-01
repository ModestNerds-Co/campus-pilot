/**
 * Typed client contracts for Hostel records and allocation lifecycle actions.
 * Learner identity is hydrated by the server from SIS and is never client-authored.
 */

export type ResidenceStatus = "active" | "inactive";
export type RoomStatus = "available" | "maintenance" | "inactive";
export type AllocationStatus = "planned" | "active" | "ended" | "cancelled";
export type PastoralCategory = "wellbeing" | "behaviour" | "safeguarding" | "family_contact" | "other";
export type PastoralSeverity = "low" | "moderate" | "high" | "critical";

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

export interface Residence {
  id: string;
  code: string;
  name: string;
  description: string | null;
  status: ResidenceStatus;
  version: number;
  room_count: number;
  bed_capacity: number;
  occupied_count: number;
  available_beds: number;
  created_at: string;
  updated_at: string;
}
export interface Room {
  id: string;
  residence_id: string;
  residence_code: string;
  residence_name: string;
  code: string;
  floor_label: string | null;
  capacity: number;
  occupied_count: number;
  available_beds: number;
  status: RoomStatus;
  version: number;
  created_at: string;
  updated_at: string;
}
export interface HostelLearnerCandidate {
  id: string;
  learner_number: string;
  display_name: string;
  status: string;
  has_current_allocation: boolean;
}
export interface HostelReferences {
  learners: HostelLearnerCandidate[];
  rooms: Room[];
}
export interface Allocation {
  id: string;
  learner_id: string;
  learner_number: string;
  learner_name: string;
  learner_status: string;
  room_id: string;
  room_code: string;
  residence_id: string;
  residence_code: string;
  residence_name: string;
  starts_on: string;
  expected_end_on: string | null;
  ended_on: string | null;
  status: AllocationStatus;
  version: number;
  previous_allocation_id: string | null;
  decision_reason: string | null;
  created_at: string;
  updated_at: string;
}
export interface AllocationPreview {
  learner_id: string;
  learner_number: string;
  learner_name: string;
  room_id: string;
  room_code: string;
  residence_name: string;
  room_version: number;
  capacity: number;
  occupied_count: number;
  available_beds: number;
  starts_on: string;
  expected_end_on: string | null;
  can_allocate: boolean;
  issues: string[];
  fingerprint: string;
}
export interface PastoralRecord {
  id: string;
  learner_id: string;
  learner_number: string;
  learner_name: string;
  allocation_id: string | null;
  residence_name: string | null;
  room_code: string | null;
  category: PastoralCategory;
  severity: PastoralSeverity;
  subject: string;
  details: string;
  occurred_at: string;
  status: "open" | "resolved";
  resolution: string | null;
  version: number;
  resolved_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface ResidencesResponse { residences: Residence[] }
export interface RoomsResponse { rooms: Room[] }
export interface AllocationsResponse { allocations: Allocation[] }
export interface PastoralRecordsResponse { pastoral_records: PastoralRecord[] }
