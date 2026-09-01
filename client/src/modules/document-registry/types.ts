export type Sensitivity = "general" | "internal" | "confidential" | "restricted";
export type FileStatus = "filed" | "closed" | "destroyed";
export type ReviewStatus = "pending" | "approved" | "rejected" | "executed";
export interface PaginationMeta { current_page:number;per_page:number;total:number;total_pages:number;has_next:boolean;has_prev:boolean }
export interface ApiEnvelope<T>{success:boolean;message:string|null;data:T|null;pagination:PaginationMeta|null;issues:Array<string|{detail?:string}>|null}
export interface NumberingPolicy{prefix:string;padding:number;next_sequence:number;next_reference:string;version:number;updated_at:string}
export interface Classification{id:string;code:string;name:string;description:string|null;retention_trigger:"filed"|"closed";retention_period_months:number|null;final_disposition:"review"|"destroy"|"permanent";default_sensitivity:Sensitivity;status:"active"|"inactive";version:number;file_count:number;created_at:string;updated_at:string}
export interface RegistryFile{id:string;reference:string;series_id:string;series_code:string;series_name:string;retention_trigger:"filed"|"closed";retention_period_months:number|null;final_disposition:"review"|"destroy"|"permanent";sensitivity:Sensitivity;title:string;description:string|null;document_date:string|null;filed_on:string;retain_until:string|null;status:FileStatus;original_file_name:string;media_type:string;byte_size:number;sha256_hex:string;scanned_at:string;version:number;closed_at:string|null;close_reason:string|null;destroyed_at:string|null;destruction_reason:string|null;created_at:string;updated_at:string}
export interface DispositionReview{id:string;file_id:string;file_reference:string;file_title:string;recommendation:"retain"|"destroy";proposed_retain_until:string|null;request_reason:string;status:ReviewStatus;version:number;requested_by:string;reviewed_by:string|null;reviewed_at:string|null;review_reason:string|null;executed_by:string|null;executed_at:string|null;created_at:string;updated_at:string}
export interface Activity{id:string;aggregate_type:string;aggregate_id:string;file_id:string|null;event_type:string;actor_id:string;metadata:Record<string,unknown>;created_at:string}
export interface ClassificationsResponse{series:Classification[]}
export interface FilesResponse{files:RegistryFile[]}
export interface ReviewsResponse{reviews:DispositionReview[]}
export interface ActivityResponse{activity:Activity[]}
export interface DownloadResponse{url:string;expires_in_seconds:number}
