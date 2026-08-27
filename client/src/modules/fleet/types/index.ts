//
//  campus-pilot
//  types/index.ts - Fleet Management Module Types
//
//  Created by Ngonidzashe Mangudya on 21/08/2026.
//  Copyright (c) 2025 Codecraft Solutions
//

export type VehicleStatus = "active" | "maintenance" | "decommissioned";
export type VehicleType = "bus" | "car" | "truck" | "van" | "minibus";
export type FuelType = "diesel" | "petrol" | "electric" | "hybrid";
export type DriverStatus = "active" | "inactive";

export interface Vehicle {
  id: string;
  registration_number: string;
  make: string;
  model: string;
  year: number | null;
  vehicle_type: string;
  capacity: number | null;
  fuel_type: string;
  status: string;
  current_odometer: number;
  insurance_expiry: string | null;
  license_expiry: string | null;
  notes: string | null;
}

export interface CreateVehicleRequest {
  registration_number: string;
  make: string;
  model: string;
  year?: number | null;
  vehicle_type?: string;
  capacity?: number | null;
  fuel_type?: string;
  status?: string;
  current_odometer?: number;
  insurance_expiry?: string | null;
  license_expiry?: string | null;
  notes?: string | null;
}

export type UpdateVehicleRequest = Partial<CreateVehicleRequest>;

export interface VehiclesListParams {
  page?: number;
  per_page?: number;
  search?: string;
  status?: VehicleStatus | "all";
}

export interface Driver {
  id: string;
  employee: {
    id: string;
    account_id: string | null;
    employee_number: string;
    display_name: string;
    work_email: string | null;
    phone: string | null;
    employment_status: string;
  };
  license_number: string;
  license_class: string | null;
  license_expiry: string | null;
  status: string;
}

export interface CreateDriverRequest {
  employee_id: string;
  license_number: string;
  license_class?: string | null;
  license_expiry?: string | null;
  status?: string;
}

export type UpdateDriverRequest = Omit<Partial<CreateDriverRequest>, "employee_id">;

export interface DriverCandidatesResponse {
  employees: Driver["employee"][];
}

export interface DriversListParams {
  page?: number;
  per_page?: number;
  search?: string;
  status?: DriverStatus | "all";
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

export interface VehiclesListResponse {
  vehicles: Vehicle[];
}

export interface DriversListResponse {
  drivers: Driver[];
}
