import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";
import type { ApiEnvelope } from "@/modules/users/types";
import type {
  LicenseUpdateResponse,
  LicensingState,
  ModuleCatalogResponse,
  TenantModulesResponse,
} from "./types";

class AccessService {
  private readonly baseUrl = "/api/1.0/access";

  async getCatalog(): Promise<ApiEnvelope<ModuleCatalogResponse>> {
    return this.request(() =>
      httpClient.get<ApiEnvelope<ModuleCatalogResponse>>(`${this.baseUrl}/catalog`),
    );
  }

  async listModules(): Promise<ApiEnvelope<TenantModulesResponse>> {
    return this.request(() =>
      httpClient.get<ApiEnvelope<TenantModulesResponse>>(`${this.baseUrl}/modules`),
    );
  }

  async getLicensingState(): Promise<ApiEnvelope<LicensingState>> {
    return this.request(() =>
      httpClient.get<ApiEnvelope<LicensingState>>(`${this.baseUrl}/licensing`),
    );
  }

  async connectLicense(activationCode: string): Promise<ApiEnvelope<LicenseUpdateResponse>> {
    return this.request(() =>
      httpClient.put<ApiEnvelope<LicenseUpdateResponse>>(
        `${this.baseUrl}/licensing/connect`,
        { activation_code: activationCode },
      ),
    );
  }

  async refreshLicense(): Promise<ApiEnvelope<LicenseUpdateResponse>> {
    return this.request(() =>
      httpClient.post<ApiEnvelope<LicenseUpdateResponse>>(`${this.baseUrl}/licensing/refresh`),
    );
  }

  async importLicense(bundle: string): Promise<ApiEnvelope<LicenseUpdateResponse>> {
    return this.request(() =>
      httpClient.post<ApiEnvelope<LicenseUpdateResponse>>(
        `${this.baseUrl}/licensing/import`,
        { bundle },
      ),
    );
  }

  async activateLicense(licenseKey: string): Promise<ApiEnvelope<{ activated_modules: string[]; expires_at: string | null }>> {
    return this.request(() =>
      httpClient.put<ApiEnvelope<{ activated_modules: string[]; expires_at: string | null }>>(
        `${this.baseUrl}/licenses/activate`,
        { license_key: licenseKey },
      ),
    );
  }

  async disableModule(moduleKey: string): Promise<ApiEnvelope<{ module_key: string; status: string }>> {
    return this.request(() =>
      httpClient.delete<ApiEnvelope<{ module_key: string; status: string }>>(
        `${this.baseUrl}/modules/${moduleKey}`,
      ),
    );
  }

  private async request<T>(request: () => Promise<{ data: ApiEnvelope<T> }>): Promise<ApiEnvelope<T>> {
    try {
      const response = await request();
      return response.data;
    } catch (error) {
      if (error instanceof AxiosError && error.response) {
        return error.response.data;
      }
      throw error;
    }
  }
}

export const accessService = new AccessService();
