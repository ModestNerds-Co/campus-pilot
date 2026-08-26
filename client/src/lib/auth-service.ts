//
//  campus-pilot
//  auth-service.ts - Authentication Service
//
//  Created by Ngonidzashe Mangudya on 02/10/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { httpClient } from "./http-client";
import { AxiosError } from "axios";

export interface LoginRequest {
  email: string;
  password: string;
}

export interface LoginResponse {
  access_token: string;
  refresh_token: string;
  expires_in: number;
  user: User;
}

export interface User {
  id: string;
  email: string;
  full_name: string;
  phone: string | null;
  roles: string[];
  role_names: string[];
  permissions: string[];
  modules: string[];
  is_active: boolean;
  last_login_at: string | null;
}

export interface RefreshRequest {
  refresh_token: string;
}

export interface RefreshResponse {
  access_token: string;
  refresh_token: string;
  expires_in: number;
}

export interface LogoutRequest {
  refresh_token?: string;
}

export interface ApiEnvelope<T = unknown> {
  success: boolean;
  message: string | null;
  data: T | null;
  issues: ValidationIssue[] | null;
  version: string;
  by: string;
}

export interface ValidationIssue {
  code: string;
  detail: string;
  field?: string;
}

class AuthService {
  private readonly BASE_URL = "/api/1.0/auth";

  async login(credentials: LoginRequest): Promise<ApiEnvelope<LoginResponse>> {
    try {
      const response = await httpClient.post<ApiEnvelope<LoginResponse>>(
        `${this.BASE_URL}/login`,
        credentials,
      );
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) {
        return error.response.data;
      }
      throw this.handleError(error);
    }
  }

  async refresh(
    refreshToken: string,
  ): Promise<ApiEnvelope<RefreshResponse>> {
    try {
      const response = await httpClient.post<ApiEnvelope<RefreshResponse>>(
        `${this.BASE_URL}/refresh`,
        { refresh_token: refreshToken },
      );
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) {
        return error.response.data;
      }
      throw this.handleError(error);
    }
  }

  async logout(refreshToken?: string): Promise<ApiEnvelope<void>> {
    try {
      const response = await httpClient.post<ApiEnvelope<void>>(
        `${this.BASE_URL}/logout`,
        { refresh_token: refreshToken },
      );
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) {
        return error.response.data;
      }
      throw this.handleError(error);
    }
  }

  async getMe(): Promise<ApiEnvelope<User>> {
    try {
      const response = await httpClient.get<ApiEnvelope<User>>(
        `${this.BASE_URL}/me`,
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

export const authService = new AuthService();
export default authService;
