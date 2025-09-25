//
//  campus-pilot
//  bootstrapService.ts - Configs Module Service
//
//  Created by Ngonidzashe Mangudya on 26/09/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import axios from 'axios';
import type {
  ApiEnvelope,
  BootstrapStatus,
  SchoolConfiguration,
  AdminConfiguration
} from '../types';
import { API_ENDPOINTS, API_CONFIG, STORAGE_KEYS } from '../constants';

class BootstrapService {
  private isMockMode = true; // Set to false when real API is ready

  /**
   * Check the current bootstrap status
   */
  async checkStatus(): Promise<ApiEnvelope<BootstrapStatus>> {
    if (this.isMockMode) {
      await this.delay(500); // Realistic loading time

      const state = this.getMockState();
      return {
        success: true,
        message: null,
        data: { state },
        issues: null,
        version: API_CONFIG.VERSION,
        by: API_CONFIG.BY
      };
    }

    try {
      const response = await axios.get<ApiEnvelope<BootstrapStatus>>(
        API_ENDPOINTS.BOOTSTRAP_STATUS,
        { timeout: API_CONFIG.TIMEOUT }
      );
      return response.data;
    } catch (error) {
      throw this.handleApiError(error);
    }
  }

  /**
   * Configure school settings
   */
  async configureSchool(data: SchoolConfiguration): Promise<ApiEnvelope<BootstrapStatus>> {
    if (this.isMockMode) {
      // Mock validation
      if (!data.name.trim()) {
        return {
          success: false,
          message: "Validation failed",
          data: null,
          issues: [{ code: "REQUIRED_FIELD", detail: "School name is required", field: "name" }],
          version: API_CONFIG.VERSION,
          by: API_CONFIG.BY
        };
      }

      await this.delay(1000);

      // Store school data in mock storage
      this.setMockData({
        state: "SchoolConfigured",
        schoolData: data
      });

      return {
        success: true,
        message: "School configuration saved successfully",
        data: { state: "SchoolConfigured" },
        issues: null,
        version: API_CONFIG.VERSION,
        by: API_CONFIG.BY
      };
    }

    try {
      const response = await axios.post<ApiEnvelope<BootstrapStatus>>(
        API_ENDPOINTS.BOOTSTRAP_SCHOOL,
        data,
        { timeout: API_CONFIG.TIMEOUT }
      );
      return response.data;
    } catch (error) {
      if (axios.isAxiosError(error) && error.response) {
        return error.response.data;
      }
      throw this.handleApiError(error);
    }
  }

  /**
   * Create administrator account
   */
  async createAdmin(data: AdminConfiguration): Promise<ApiEnvelope<BootstrapStatus>> {
    if (this.isMockMode) {
      // Mock validation
      const issues = this.validateAdminData(data);

      if (issues.length > 0) {
        return {
          success: false,
          message: "Validation failed",
          data: null,
          issues,
          version: API_CONFIG.VERSION,
          by: API_CONFIG.BY
        };
      }

      await this.delay(1200);

      // Store admin data and mark as ready
      this.setMockData({
        state: "Ready",
        adminData: { ...data, password: '[PROTECTED]' }
      });

      return {
        success: true,
        message: "Administrator account created successfully",
        data: { state: "Ready" },
        issues: null,
        version: API_CONFIG.VERSION,
        by: API_CONFIG.BY
      };
    }

    try {
      const response = await axios.post<ApiEnvelope<BootstrapStatus>>(
        API_ENDPOINTS.BOOTSTRAP_ADMIN,
        data,
        { timeout: API_CONFIG.TIMEOUT }
      );
      return response.data;
    } catch (error) {
      if (axios.isAxiosError(error) && error.response) {
        return error.response.data;
      }
      throw this.handleApiError(error);
    }
  }

  /**
   * Get stored school data for preview (mock only)
   */
  getMockSchoolData(): SchoolConfiguration | null {
    if (!this.isMockMode) return null;

    const stored = localStorage.getItem(STORAGE_KEYS.BOOTSTRAP_STATE);
    if (stored) {
      const data = JSON.parse(stored);
      return data.schoolData || null;
    }
    return null;
  }

  /**
   * Reset mock state for testing
   */
  resetMockState(): void {
    if (this.isMockMode) {
      localStorage.removeItem(STORAGE_KEYS.BOOTSTRAP_STATE);
    }
  }

  // Private helper methods

  private getMockState() {
    const stored = localStorage.getItem(STORAGE_KEYS.BOOTSTRAP_STATE);
    if (stored) {
      const data = JSON.parse(stored);
      return data.state || "Uninitialized";
    }
    return "Uninitialized";
  }

  private setMockData(data: any): void {
    const existing = JSON.parse(localStorage.getItem(STORAGE_KEYS.BOOTSTRAP_STATE) || '{}');
    localStorage.setItem(STORAGE_KEYS.BOOTSTRAP_STATE, JSON.stringify({
      ...existing,
      ...data,
      lastUpdated: new Date().toISOString()
    }));
  }

  private delay(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
  }

  private validateAdminData(data: AdminConfiguration) {
    const issues = [];

    if (!data.full_name.trim()) {
      issues.push({ code: "REQUIRED_FIELD", detail: "Full name is required", field: "full_name" });
    }

    if (!data.email.trim()) {
      issues.push({ code: "REQUIRED_FIELD", detail: "Email is required", field: "email" });
    }

    if (!data.password || data.password.length < 10) {
      issues.push({ code: "WEAK_PASSWORD", detail: "Password must be at least 10 characters", field: "password" });
    }

    if (!/\d/.test(data.password)) {
      issues.push({ code: "WEAK_PASSWORD", detail: "Password must include at least one number", field: "password" });
    }

    if (!/[!@#$%^&*(),.?":{}|<>]/.test(data.password)) {
      issues.push({ code: "WEAK_PASSWORD", detail: "Password must include at least one symbol", field: "password" });
    }

    return issues;
  }

  private handleApiError(error: any): Error {
    if (axios.isAxiosError(error)) {
      if (error.code === 'ECONNABORTED') {
        return new Error('Request timed out. Please try again.');
      }
      if (!error.response) {
        return new Error('Network error. Please check your connection.');
      }
      return new Error(error.response.data?.message || 'Server error occurred');
    }

    return error instanceof Error ? error : new Error('Unknown error occurred');
  }
}

export const bootstrapService = new BootstrapService();
