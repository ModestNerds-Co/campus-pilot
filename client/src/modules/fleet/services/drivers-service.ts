//
//  campus-pilot
//  drivers-service.ts - Fleet Drivers Service
//
//  Created by Ngonidzashe Mangudya on 21/08/2026.
//  Copyright (c) 2025 Codecraft Solutions
//

import { httpClient } from "../../../lib/http-client";
import { AxiosError } from "axios";
import type {
  Driver,
  CreateDriverRequest,
  UpdateDriverRequest,
  DriversListParams,
  DriversListResponse,
  ApiEnvelope,
} from "../types";

class DriversService {
  private readonly BASE_URL = "/api/1.0/fleet/drivers";

  async listDrivers(params?: DriversListParams): Promise<ApiEnvelope<DriversListResponse>> {
    try {
      const response = await httpClient.get<ApiEnvelope<DriversListResponse>>(this.BASE_URL, { params });
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) return error.response.data;
      throw this.handleError(error);
    }
  }

  async getDriver(id: string): Promise<ApiEnvelope<Driver>> {
    try {
      const response = await httpClient.get<ApiEnvelope<Driver>>(`${this.BASE_URL}/${id}`);
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) return error.response.data;
      throw this.handleError(error);
    }
  }

  async createDriver(data: CreateDriverRequest): Promise<ApiEnvelope<Driver>> {
    try {
      const response = await httpClient.post<ApiEnvelope<Driver>>(this.BASE_URL, data);
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) return error.response.data;
      throw this.handleError(error);
    }
  }

  async updateDriver(id: string, data: UpdateDriverRequest): Promise<ApiEnvelope<Driver>> {
    try {
      const response = await httpClient.put<ApiEnvelope<Driver>>(`${this.BASE_URL}/${id}`, data);
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) return error.response.data;
      throw this.handleError(error);
    }
  }

  async deleteDriver(id: string): Promise<ApiEnvelope<void>> {
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

export const driversService = new DriversService();
export default driversService;
