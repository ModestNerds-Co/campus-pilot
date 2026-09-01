/** Closed client contracts for school Transport. */

export type RouteDirection = "inbound" | "outbound";
export type RouteStatus = "active" | "inactive";
export type RiderStatus = "active" | "ended" | "cancelled";
export type RunStatus = "draft" | "boarding" | "departed" | "completed" | "cancelled";
export type ManifestStatus = "expected" | "boarded" | "no_show" | "exception";
export type ManifestExceptionKind = "not_at_stop" | "illness" | "transport_change" | "conduct" | "safety" | "other";

export interface PaginationMeta { current_page: number; per_page: number; total: number; total_pages: number; has_next: boolean; has_prev: boolean }
export interface ApiEnvelope<T> { success: boolean; message: string | null; data: T | null; pagination: PaginationMeta | null; issues: Array<string | { detail?: string }> | null }

export interface RouteStop { id: string; code: string; name: string; stop_order: number; planned_time: string; latitude: number | null; longitude: number | null; version: number }
export interface RouteSummary { id: string; code: string; name: string; direction: RouteDirection; status: RouteStatus; notes: string | null; version: number; stop_count: number; active_rider_count: number; updated_at: string }
export interface RouteRecord extends RouteSummary { stops: RouteStop[]; created_at: string }
export interface RoutesResponse { routes: RouteSummary[] }

export interface LearnerReference { id: string; learner_number: string; display_name: string; status: string }
export interface VehicleReference { id: string; registration_number: string; make: string; model: string; capacity: number; status: string }
export interface DriverReference { id: string; employee_id: string; display_name: string; license_number: string; license_expiry: string | null; status: string }
export interface TransportReferences { learners: LearnerReference[]; vehicles: VehicleReference[]; drivers: DriverReference[]; routes: RouteRecord[] }

export interface RiderAssignment { id: string; learner_id: string; learner_number: string; learner_name: string; route_id: string; route_code: string; route_name: string; direction: RouteDirection; boarding_stop_id: string; boarding_stop_name: string; alighting_stop_id: string; alighting_stop_name: string; effective_from: string; effective_until: string | null; status: RiderStatus; version: number; updated_at: string }
export interface RidersResponse { riders: RiderAssignment[] }

export interface RunStop { id: string; source_stop_id: string; code: string; name: string; stop_order: number; planned_time: string }
export interface ManifestEntry { id: string; learner_id: string; learner_number: string; learner_name: string; boarding_run_stop_id: string; boarding_stop_name: string; alighting_run_stop_id: string; alighting_stop_name: string; status: ManifestStatus; exception_kind: ManifestExceptionKind | null; note: string | null; marked_at: string | null; version: number }
export interface RunEvent { id: string; event_type: string; manifest_entry_id: string | null; actor_name: string; metadata: Record<string, unknown>; created_at: string }
export interface RunSummary { id: string; reference: string; route_id: string; route_code: string; route_name: string; direction: RouteDirection; service_date: string; vehicle_id: string; vehicle_registration: string; driver_id: string; driver_name: string; capacity: number; status: RunStatus; expected_count: number; boarded_count: number; exception_count: number; version: number; updated_at: string }
export interface RunRecord extends RunSummary { stops: RunStop[]; manifest: ManifestEntry[]; history: RunEvent[]; created_at: string }
export interface RunsResponse { runs: RunSummary[] }

export interface RoutePayload { code: string; name: string; direction: RouteDirection; notes: string | null }
export interface StopPayload { code: string; name: string; stop_order: number; planned_time: string; latitude: number | null; longitude: number | null }

