// SIS transport contracts. People records remain separate from login accounts.

import type { ApiEnvelope, PaginationMeta } from "@/modules/academics";

export type { ApiEnvelope, PaginationMeta };

export type LearnerStatus = "prospective" | "active" | "inactive" | "graduated" | "withdrawn";
export type DirectoryStatus = "active" | "inactive";
export type RelationshipType = "mother" | "father" | "parent" | "guardian" | "carer" | "sponsor" | "other";
export type ApplicationStatus = "draft" | "submitted" | "under_review" | "offered" | "accepted" | "rejected" | "withdrawn";
export type EnrolmentStatus = "active" | "completed" | "withdrawn";

export interface Learner {
  id: string;
  account_id: string | null;
  account_email: string | null;
  learner_number: string;
  display_name: string;
  first_names: string | null;
  surname: string | null;
  date_of_birth: string;
  email: string | null;
  phone: string | null;
  status: LearnerStatus;
}

export interface Guardian {
  id: string;
  account_id: string | null;
  account_email: string | null;
  display_name: string;
  first_names: string | null;
  surname: string | null;
  email: string | null;
  phone: string | null;
  status: DirectoryStatus;
}

export interface GuardianRelationship {
  id: string;
  learner_id: string;
  learner_name: string;
  learner_number: string;
  guardian_id: string;
  guardian_name: string;
  relationship_type: RelationshipType;
  is_primary: boolean;
  can_collect: boolean;
  receives_communications: boolean;
  status: DirectoryStatus;
}

export interface Application {
  id: string;
  application_number: string;
  learner_id: string;
  learner_name: string;
  learner_number: string;
  academic_year_id: string;
  academic_year_name: string;
  target_grade_level_id: string | null;
  target_grade_level_name: string | null;
  submitted_on: string | null;
  status: ApplicationStatus;
  notes: string | null;
}

export interface Enrolment {
  id: string;
  learner_id: string;
  learner_name: string;
  learner_number: string;
  academic_year_id: string;
  academic_year_name: string;
  class_group_id: string;
  class_group_name: string;
  source_application_id: string | null;
  application_number: string | null;
  starts_on: string;
  ends_on: string | null;
  status: EnrolmentStatus;
}

export interface AccountCandidate { id: string; full_name: string; email: string }

export interface LearnerInput {
  learner_number: string;
  display_name: string;
  first_names?: string | null;
  surname?: string | null;
  date_of_birth: string;
  email?: string | null;
  phone?: string | null;
  status?: LearnerStatus;
}

export interface GuardianInput {
  display_name: string;
  first_names?: string | null;
  surname?: string | null;
  email?: string | null;
  phone?: string | null;
  status?: DirectoryStatus;
}

export interface GuardianRelationshipInput {
  learner_id: string;
  guardian_id: string;
  relationship_type: RelationshipType;
  is_primary?: boolean;
  can_collect?: boolean;
  receives_communications?: boolean;
  status?: DirectoryStatus;
}

export interface ApplicationInput {
  application_number: string;
  learner_id: string;
  academic_year_id: string;
  target_grade_level_id: string;
  submitted_on?: string | null;
  status?: ApplicationStatus;
  notes?: string | null;
}

export interface EnrolmentInput {
  learner_id: string;
  academic_year_id: string;
  class_group_id: string;
  source_application_id?: string | null;
  starts_on: string;
  ends_on?: string | null;
  status?: EnrolmentStatus;
}

export interface ListParams {
  page?: number;
  per_page?: number;
  search?: string;
  status?: string;
  learner_id?: string;
  guardian_id?: string;
  academic_year_id?: string;
  target_grade_level_id?: string;
  class_group_id?: string;
}

export interface LearnersResponse { learners: Learner[] }
export interface GuardiansResponse { guardians: Guardian[] }
export interface GuardianRelationshipsResponse { relationships: GuardianRelationship[] }
export interface ApplicationsResponse { applications: Application[] }
export interface EnrolmentsResponse { enrolments: Enrolment[] }
export interface AccountCandidatesResponse { accounts: AccountCandidate[] }
