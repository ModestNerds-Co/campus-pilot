//
//  campus-pilot
//  types/index.ts - Vehicle Daily Log Module Types
//
//  Created by Ngonidzashe Mangudya on 21/08/2026.
//  Copyright (c) 2025 Codecraft Solutions
//

export type DailyLogStatus = "draft" | "submitted" | "approved";

export interface VehicleDailyLog {
  id: string;
  vehicle_id: string;
  vehicle_registration: string;
  driver_id: string;
  driver_name: string;
  log_date: string;
  start_odometer: number;
  end_odometer: number | null;
  start_time: string | null;
  end_time: string | null;
  destination: string | null;
  purpose: string;
  fuel_added_liters: number | null;
  fuel_cost: number | null;
  status: string;
}

export interface CreateVehicleDailyLogRequest {
  vehicle_id: string;
  driver_id: string;
  log_date: string;
  start_odometer: number;
  end_odometer?: number | null;
  start_time?: string | null;
  end_time?: string | null;
  destination?: string | null;
  purpose: string;
  fuel_added_liters?: number | null;
  fuel_cost?: number | null;
  status?: string;
}

export type UpdateVehicleDailyLogRequest = Partial<CreateVehicleDailyLogRequest>;

export interface VehicleDailyLogsListParams {
  page?: number;
  per_page?: number;
  vehicle_id?: string;
  driver_id?: string;
  status?: DailyLogStatus | "all";
  from_date?: string;
  to_date?: string;
}

export interface PaginationMeta {
  current_page: number;
  per_page: number;
  total: number;
  total_pages: number;
  has_next: boolean;
  has_prev: boolean;
}

export interface ApiEnvelope<T = any> {
  success: boolean;
  message: string | null;
  data: T | null;
  pagination: PaginationMeta | null;
  issues: string[] | null;
  version: number;
  by: string;
}

export interface VehicleDailyLogsListResponse {
  logs: VehicleDailyLog[];
}
