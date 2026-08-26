import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";
import type { ApiEnvelope } from "@/modules/users/types";
import type { TimetableConfiguration, TimetableRun } from "./types";

class TimetablingService {
  private readonly baseUrl = "/api/1.0/timetabling";

  getConfiguration() {
    return this.request<TimetableConfiguration>(() => httpClient.get(`${this.baseUrl}/configuration`));
  }

  saveConfiguration(configuration: TimetableConfiguration) {
    return this.request<TimetableConfiguration>(() => httpClient.put(`${this.baseUrl}/configuration`, configuration));
  }

  getLatestRun() {
    return this.request<TimetableRun>(() => httpClient.get(`${this.baseUrl}/runs/latest`));
  }

  generate() {
    return this.request<TimetableRun>(() => httpClient.post(`${this.baseUrl}/generate`));
  }

  publish(runId: string) {
    return this.request<TimetableRun>(() => httpClient.put(`${this.baseUrl}/runs/${runId}/publish`));
  }

  private async request<T>(request: () => Promise<{ data: ApiEnvelope<T> }>): Promise<ApiEnvelope<T>> {
    try {
      return (await request()).data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) return error.response.data;
      throw error;
    }
  }
}

export const timetablingService = new TimetablingService();
