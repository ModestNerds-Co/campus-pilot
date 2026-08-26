//
//  campus-pilot
//  users-service.ts - Users Service
//
//  Created by Ngonidzashe Mangudya on 02/10/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { httpClient } from "../../../lib/http-client";
import { AxiosError } from "axios";
import type {
  User,
  CreateUserRequest,
  UpdateUserRequest,
  UsersListParams,
  UsersListResponse,
  ApiEnvelope,
  Role,
} from "../types";

class UsersService {
  private readonly BASE_URL = "/api/1.0/users";

  async listUsers(params?: UsersListParams): Promise<ApiEnvelope<UsersListResponse>> {
    try {
      const response = await httpClient.get<ApiEnvelope<UsersListResponse>>(
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

  async getUser(id: string): Promise<ApiEnvelope<User>> {
    try {
      const response = await httpClient.get<ApiEnvelope<User>>(
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

  async createUser(data: CreateUserRequest): Promise<ApiEnvelope<User>> {
    try {
      const response = await httpClient.post<ApiEnvelope<User>>(
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

  async updateUser(
    id: string,
    data: UpdateUserRequest
  ): Promise<ApiEnvelope<User>> {
    try {
      const response = await httpClient.put<ApiEnvelope<User>>(
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

  async deleteUser(id: string): Promise<ApiEnvelope<void>> {
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

  async activateUser(id: string): Promise<ApiEnvelope<User>> {
    try {
      const response = await httpClient.put<ApiEnvelope<User>>(
        `${this.BASE_URL}/${id}/activate`
      );
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) {
        return error.response.data;
      }
      throw this.handleError(error);
    }
  }

  async deactivateUser(id: string): Promise<ApiEnvelope<User>> {
    try {
      const response = await httpClient.put<ApiEnvelope<User>>(
        `${this.BASE_URL}/${id}/deactivate`
      );
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) {
        return error.response.data;
      }
      throw this.handleError(error);
    }
  }

  async listRoles(): Promise<ApiEnvelope<Role[]>> {
    try {
      const response = await httpClient.get<ApiEnvelope<Role[]>>(
        "/api/1.0/roles"
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

export const usersService = new UsersService();
export default usersService;
