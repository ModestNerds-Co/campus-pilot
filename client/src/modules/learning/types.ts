export type LearningSpaceStatus = "draft" | "published" | "archived";
export type LearningUnitStatus = "draft" | "published" | "withdrawn";
export type LearningResourceStatus = "draft" | "published" | "withdrawn";

export interface PaginationMeta { current_page:number;per_page:number;total:number;total_pages:number;has_next:boolean;has_prev:boolean }
export interface ApiEnvelope<T>{success:boolean;message:string|null;data:T|null;pagination:PaginationMeta|null;issues:Array<string|{detail?:string}>|null}

export interface LearningSettings { document_series_id:string|null;document_series_name:string|null;version:number;updated_at:string }
export interface LearningTermReference { id:string;academic_year_id:string;academic_year_name:string;code:string;name:string;starts_on:string;ends_on:string }
export interface LearningAssignmentReference { id:string;academic_year_id:string;academic_year_name:string;class_group_id:string;class_group_name:string;subject_id:string;subject_name:string;teacher_name:string }
export interface LearningReferenceData { active_term:LearningTermReference|null;assignments:LearningAssignmentReference[] }
export interface GovernedFileReference { id:string;reference:string;title:string;sensitivity:string;status:string }

export interface LearningSpaceSummary {
  id:string;teaching_assignment_id:string;academic_year_id:string;academic_year_name:string;
  academic_term_id:string;academic_term_name:string;class_group_id:string;class_group_name:string;
  subject_name:string;teacher_name:string;title:string;summary:string|null;status:LearningSpaceStatus;
  version:number;unit_count:number;published_unit_count:number;published_at:string|null;archived_at:string|null;
  archive_reason:string|null;created_at:string;updated_at:string;
}
export interface LearningResource { id:string;learning_unit_id:string;document_file_id:string;document:GovernedFileReference|null;display_title:string;sensitivity_snapshot:string;position:number;status:LearningResourceStatus;version:number;published_at:string|null;withdrawn_at:string|null;withdrawal_reason:string|null;created_at:string;updated_at:string }
export interface LearningUnit { id:string;learning_space_id:string;position:number;title:string;summary:string|null;status:LearningUnitStatus;version:number;published_at:string|null;withdrawn_at:string|null;withdrawal_reason:string|null;resources:LearningResource[];created_at:string;updated_at:string }
export interface LearningSpace extends LearningSpaceSummary { units:LearningUnit[] }
export interface LearningSpacesResponse { spaces:LearningSpaceSummary[] }
export interface LearningFilesResponse { files:GovernedFileReference[] }
export interface LearningDownload { url:string;expires_in_seconds:number }

export interface LearningSpaceListParams { page?:number;per_page?:number;search?:string;status?:LearningSpaceStatus }
export interface CreateLearningSpace { teaching_assignment_id:string;academic_term_id:string;title:string;summary:string|null }
export interface CreateLearningUnit { position:number;title:string;summary:string|null }
export interface CreateLearningResource { document_file_id:string;display_title:string;position:number }
