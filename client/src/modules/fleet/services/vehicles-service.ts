//
//  campus-pilot
//  vehicles-service.ts - Fleet Vehicles Service
//
//  Created by Ngonidzashe Mangudya on 21/08/2026.
//  Copyright (c) 2025 Codecraft Solutions
//

import { httpClient } from "../../../lib/http-client";
import { AxiosError } from "axios";
import type {
  Vehicle,
  CreateVehicleRequest,
  UpdateVehicleRequest,
  VehiclesListParams,
  VehiclesListResponse,
  ApiEnvelope,
} from "../types";

class VehiclesService {
  private readonly BASE_URL = "/api/1.0/fleet/vehicles";

  async listVehicles(params?: VehiclesListParams): Promise<ApiEnvelope<VehiclesListResponse>> {
    try {
      const response = await httpClient.get<ApiEnvelope<VehiclesListResponse>>(this.BASE_URL, { params });
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) return error.response.data;
      throw this.handleError(error);
    }
  }

  async getVehicle(id: string): Promise<ApiEnvelope<Vehicle>> {
    try {
      const response = await httpClient.get<ApiEnvelope<Vehicle>>(`${this.BASE_URL}/${id}`);
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) return error.response.data;
      throw this.handleError(error);
    }
  }

  async createVehicle(data: CreateVehicleRequest): Promise<ApiEnvelope<Vehicle>> {
    try {
      const response = await httpClient.post<ApiEnvelope<Vehicle>>(this.BASE_URL, data);
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) return error.response.data;
      throw this.handleError(error);
    }
  }

  async updateVehicle(id: string, data: UpdateVehicleRequest): Promise<ApiEnvelope<Vehicle>> {
    try {
      const response = await httpClient.put<ApiEnvelope<Vehicle>>(`${this.BASE_URL}/${id}`, data);
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) return error.response.data;
      throw this.handleError(error);
    }
  }

  async deleteVehicle(id: string): Promise<ApiEnvelope<void>> {
    try {
      const response = await httpClient.delete<ApiEnvelope<void>>(`${this.BASE_URL}/${id}`);
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) return error.response.data;
      throw this.handleError(error);
    }
  }

  private handleError(error: any): Error {
    if (error instanceof AxiosError) {
      if (error.code === "ECONNABORTED") return new Error("Request timed out. Please try again.");
      if (!error.response) return new Error("Network error. Please check your connection.");
      return new Error(error.response.data?.message || "Server error occurred");
    }
    return error instanceof Error ? error : new Error("Unknown error occurred");
  }
}

export const vehiclesService = new VehiclesService();
export default vehiclesService;
