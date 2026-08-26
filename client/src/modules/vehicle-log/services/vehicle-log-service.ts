//
//  campus-pilot
//  vehicle-log-service.ts - Vehicle Daily Log Service
//
//  Created by Ngonidzashe Mangudya on 21/08/2026.
//  Copyright (c) 2025 Codecraft Solutions
//

import { httpClient } from "../../../lib/http-client";
import { AxiosError } from "axios";
import type {
  VehicleDailyLog,
  CreateVehicleDailyLogRequest,
  UpdateVehicleDailyLogRequest,
  VehicleDailyLogsListParams,
  VehicleDailyLogsListResponse,
  ApiEnvelope,
} from "../types";

class VehicleLogService {
  private readonly BASE_URL = "/api/1.0/vehicle-logs";

  async listLogs(params?: VehicleDailyLogsListParams): Promise<ApiEnvelope<VehicleDailyLogsListResponse>> {
    try {
      const response = await httpClient.get<ApiEnvelope<VehicleDailyLogsListResponse>>(this.BASE_URL, { params });
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) return error.response.data;
      throw this.handleError(error);
    }
  }

  async getLog(id: string): Promise<ApiEnvelope<VehicleDailyLog>> {
    try {
      const response = await httpClient.get<ApiEnvelope<VehicleDailyLog>>(`${this.BASE_URL}/${id}`);
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) return error.response.data;
      throw this.handleError(error);
    }
  }

  async createLog(data: CreateVehicleDailyLogRequest): Promise<ApiEnvelope<VehicleDailyLog>> {
    try {
      const response = await httpClient.post<ApiEnvelope<VehicleDailyLog>>(this.BASE_URL, data);
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) return error.response.data;
      throw this.handleError(error);
    }
  }

  async updateLog(id: string, data: UpdateVehicleDailyLogRequest): Promise<ApiEnvelope<VehicleDailyLog>> {
    try {
      const response = await httpClient.put<ApiEnvelope<VehicleDailyLog>>(`${this.BASE_URL}/${id}`, data);
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) return error.response.data;
      throw this.handleError(error);
    }
  }

  async deleteLog(id: string): Promise<ApiEnvelope<void>> {
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

export const vehicleLogService = new VehicleLogService();
export default vehicleLogService;
