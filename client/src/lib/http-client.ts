/// <reference types="vite/client" />
//
//  campus-pilot
//  httpClient.ts - Global Axios HTTP Client
//
//  Created by Ngonidzashe Mangudya on 01/10/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import axios, {
  AxiosInstance,
  AxiosRequestConfig,
  AxiosResponse,
  AxiosError,
} from "axios";

const API_BASE_URL =
  import.meta.env.VITE_API_BASE_URL || "http://127.0.0.1:8000";
const API_TIMEOUT = 30000;

const getAuthToken = (): string | null => {
  try {
    const authData = localStorage.getItem("campuspilot_auth");
    if (authData) {
      const parsed = JSON.parse(authData);
      return parsed.state?.accessToken || null;
    }
  } catch {
    return null;
  }
  return null;
};

class HttpClient {
  private client: AxiosInstance;

  constructor() {
    this.client = axios.create({
      baseURL: API_BASE_URL,
      timeout: API_TIMEOUT,
      headers: {
        "Content-Type": "application/json",
      },
    });

    this.setupInterceptors();
  }

  private setupInterceptors(): void {
    this.client.interceptors.request.use(
      (config) => {
        const token = getAuthToken();
        if (token) {
          config.headers.Authorization = `Bearer ${token}`;
        }

        console.log(`[HTTP] ${config.method?.toUpperCase()} ${config.url}`);
        return config;
      },
      (error) => {
        console.error("[HTTP] Request error:", error);
        return Promise.reject(error);
      },
    );

    this.client.interceptors.response.use(
      (response) => {
        console.log(`[HTTP] ${response.status} ${response.config.url}`);
        return response;
      },
      async (error: AxiosError) => {
        if (error.response) {
          console.error(
            `[HTTP] ${error.response.status} ${error.config?.url}`,
            error.response.data,
          );

          if (error.response.status === 401) {
            try {
              const authData = localStorage.getItem("campuspilot_auth");
              if (authData) {
                const parsed = JSON.parse(authData);
                const refreshToken = parsed.state?.refreshToken;

                if (refreshToken) {
                  const refreshResponse = await axios.post(
                    `${API_BASE_URL}/api/1.0/auth/refresh`,
                    { refresh_token: refreshToken },
                  );

                  if (
                    refreshResponse.data.success &&
                    refreshResponse.data.data
                  ) {
                    const { access_token, refresh_token, expires_in } =
                      refreshResponse.data.data;

                    parsed.state.accessToken = access_token;
                    parsed.state.refreshToken = refresh_token;
                    parsed.state.expiresAt = Date.now() + expires_in * 1000;
                    localStorage.setItem(
                      "campuspilot_auth",
                      JSON.stringify(parsed),
                    );

                    if (error.config) {
                      error.config.headers.Authorization = `Bearer ${access_token}`;
                      return this.client.request(error.config);
                    }
                  }
                }
              }
            } catch (refreshError) {
              console.error("[HTTP] Token refresh failed:", refreshError);
            }

            localStorage.removeItem("campuspilot_auth");
            window.location.href = "/login";
          }
        } else if (error.request) {
          console.error("[HTTP] No response received:", error.message);
        } else {
          console.error("[HTTP] Request setup error:", error.message);
        }

        return Promise.reject(error);
      },
    );
  }

  public async get<T = any>(
    url: string,
    config?: AxiosRequestConfig,
  ): Promise<AxiosResponse<T>> {
    return this.client.get<T>(url, config);
  }

  public async post<T = any>(
    url: string,
    data?: any,
    config?: AxiosRequestConfig,
  ): Promise<AxiosResponse<T>> {
    return this.client.post<T>(url, data, config);
  }

  public async put<T = any>(
    url: string,
    data?: any,
    config?: AxiosRequestConfig,
  ): Promise<AxiosResponse<T>> {
    return this.client.put<T>(url, data, config);
  }

  public async patch<T = any>(
    url: string,
    data?: any,
    config?: AxiosRequestConfig,
  ): Promise<AxiosResponse<T>> {
    return this.client.patch<T>(url, data, config);
  }

  public async delete<T = any>(
    url: string,
    config?: AxiosRequestConfig,
  ): Promise<AxiosResponse<T>> {
    return this.client.delete<T>(url, config);
  }

  public getInstance(): AxiosInstance {
    return this.client;
  }
}

export const httpClient = new HttpClient();
export default httpClient;
