export type AnnouncementPriority = "normal" | "important" | "urgent";
export type AnnouncementStatus = "draft" | "submitted" | "published" | "cancelled";
export type AudienceKind = "campus" | "role" | "class_group" | "department" | "individual";
export type AnnouncementListStatus = "all" | AnnouncementStatus;
export type InboxListFilter = "all" | "unread";

export interface MessagingListSearch {
  q: string;
  status: AnnouncementListStatus;
  page: number;
  filter: InboxListFilter;
}

export interface AudienceTargetInput { kind: AudienceKind; target_id: string | null; target_key: string | null; label: string }
export interface AudienceTarget extends AudienceTargetInput { id: string }
export interface AnnouncementSummary { id: string; title: string; priority: AnnouncementPriority; status: AnnouncementStatus; version: number; creator_name: string; recipient_count: number; read_count: number; created_at: string; updated_at: string; published_at: string | null }
export interface AnnouncementDetail extends AnnouncementSummary { body: string; created_by: string; targets: AudienceTarget[]; submitted_at: string | null; cancelled_at: string | null; cancellation_reason: string | null; reopened_at: string | null; reopen_reason: string | null }
export interface ClassReference { id: string; code: string; name: string; grade_level: string | null }
export interface DepartmentReference { id: string; code: string; name: string }
export interface RoleReference { key: string; name: string }
export interface UserReference { id: string; full_name: string; email: string }
export interface CommunicationReferenceData { classes: ClassReference[]; departments: DepartmentReference[]; roles: RoleReference[]; users: UserReference[]; campus_allowed: boolean }
export interface AudiencePreview { recipient_count: number; recipients: UserReference[] }
export interface DeliveryRecord { id: string; announcement_id: string; recipient_user_id: string; recipient_name: string; channel: "in_app"; status: "pending" | "delivered"; delivered_at: string | null; read_at: string | null }
export interface InboxItem { delivery_id: string; announcement_id: string; title: string; body: string; priority: AnnouncementPriority; announcement_status: AnnouncementStatus; cancellation_reason: string | null; sender_name: string; published_at: string; read_at: string | null }
export interface PaginationMeta { current_page: number; per_page: number; total: number; total_pages: number; has_next: boolean; has_prev: boolean }
export interface ApiEnvelope<T> { success: boolean; message: string | null; data: T | null; pagination: PaginationMeta | null; issues: Array<string | { detail?: string }> | null }
export interface AnnouncementsResponse { announcements: AnnouncementSummary[] }
export interface InboxResponse { messages: InboxItem[] }
export interface AnnouncementPayload { title: string; body: string; priority: AnnouncementPriority; targets: AudienceTargetInput[] }
