//
//  campus-pilot
//  bootstrapService.ts - Configs Module Service
//
//  Created by Ngonidzashe Mangudya on 26/09/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { httpClient } from "../../../lib/http-client";
import { AxiosError } from "axios";
import type {
  ApiEnvelope,
  BootstrapStatus,
  SchoolConfiguration,
  AdminConfiguration,
} from "../types";
import { API_ENDPOINTS, API_CONFIG, STORAGE_KEYS } from "../constants";

class BootstrapService {
  /**
   * Check the current bootstrap status
   */
  async checkStatus(): Promise<ApiEnvelope<BootstrapStatus>> {
    try {
      const response = await httpClient.get<ApiEnvelope<BootstrapStatus>>(
        API_ENDPOINTS.BOOTSTRAP_STATUS,
        { timeout: API_CONFIG.TIMEOUT },
      );
      return response.data;
    } catch (error) {
      throw this.handleApiError(error);
    }
  }

  /**
   * Configure school settings
   */
  async configureSchool(
    data: SchoolConfiguration,
  ): Promise<ApiEnvelope<BootstrapStatus>> {
    try {
      const response = await httpClient.post<ApiEnvelope<BootstrapStatus>>(
        API_ENDPOINTS.BOOTSTRAP_SCHOOL,
        data,
        { timeout: API_CONFIG.TIMEOUT },
      );
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) {
        return error.response.data;
      }
      throw this.handleApiError(error);
    }
  }

  /**
   * Create administrator account
   */
  async createAdmin(
    data: AdminConfiguration,
  ): Promise<ApiEnvelope<BootstrapStatus>> {
    try {
      const response = await httpClient.post<ApiEnvelope<BootstrapStatus>>(
        API_ENDPOINTS.BOOTSTRAP_ADMIN,
        data,
        { timeout: API_CONFIG.TIMEOUT },
      );
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) {
        return error.response.data;
      }
      throw this.handleApiError(error);
    }
  }

  private handleApiError(error: any): Error {
    if (error instanceof AxiosError) {
      if (error.code === "ECONNABORTED") {
        return new Error("Request timed out. Please try again.");
      }
      if (!error.response) {
        return new Error("Network error. Please check your connection.");
      }
      return new Error(error.response.data?.message || "Server error occurred");
    }

    return error instanceof Error ? error : new Error("Unknown error occurred");
  }
}

export const bootstrapService = new BootstrapService();
