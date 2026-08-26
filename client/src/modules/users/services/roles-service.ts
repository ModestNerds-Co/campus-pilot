//
//  campus-pilot
//  roles-service.ts - Roles Service
//
//  Created by Ngonidzashe Mangudya on 03/10/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { httpClient } from "../../../lib/http-client";
import { AxiosError } from "axios";
import type {
  Role,
  CreateRoleRequest,
  UpdateRoleRequest,
  RolesListParams,
  RolesListResponse,
  ApiEnvelope,
} from "../types";

class RolesService {
  private readonly BASE_URL = "/api/1.0/roles";

  async listRoles(params?: RolesListParams): Promise<ApiEnvelope<RolesListResponse>> {
    try {
      const response = await httpClient.get<ApiEnvelope<RolesListResponse>>(
        this.BASE_URL,
        { params }
      );
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) {
        return error.response.data;
      }
      throw this.handleError(error);
    }
  }

  async getRole(id: string): Promise<ApiEnvelope<Role>> {
    try {
      const response = await httpClient.get<ApiEnvelope<Role>>(
        `${this.BASE_URL}/${id}`
      );
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) {
        return error.response.data;
      }
      throw this.handleError(error);
    }
  }

  async createRole(data: CreateRoleRequest): Promise<ApiEnvelope<Role>> {
    try {
      const response = await httpClient.post<ApiEnvelope<Role>>(
        this.BASE_URL,
        data
      );
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) {
        return error.response.data;
      }
      throw this.handleError(error);
    }
  }

  async updateRole(
    id: string,
    data: UpdateRoleRequest
  ): Promise<ApiEnvelope<Role>> {
    try {
      const response = await httpClient.put<ApiEnvelope<Role>>(
        `${this.BASE_URL}/${id}`,
        data
      );
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) {
        return error.response.data;
      }
      throw this.handleError(error);
    }
  }

  async deleteRole(id: string): Promise<ApiEnvelope<void>> {
    try {
      const response = await httpClient.delete<ApiEnvelope<void>>(
        `${this.BASE_URL}/${id}`
      );
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) {
        return error.response.data;
      }
      throw this.handleError(error);
    }
  }

  private handleError(error: unknown): Error {
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

export const rolesService = new RolesService();
export default rolesService;
