//
//  campus-pilot
//  storage-service.ts - MinIO Storage Service
//
//  Created by Ngonidzashe Mangudya on 01/10/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { httpClient } from "./http-client";
import { AxiosError } from "axios";
import type {
  ApiEnvelope,
  PresignedUploadRequest,
  PresignedUploadResponse,
} from "../modules/configs/types";

const MINIO_BASE_URL =
  import.meta.env.VITE_MINIO_BASE_URL || "http://localhost:9000";
const MINIO_BUCKET = import.meta.env.VITE_MINIO_BUCKET || "campus-pilot";

class StorageService {
  async generatePresignedUrl(
    filename: string,
    fileType: string,
  ): Promise<PresignedUploadResponse> {
    try {
      const request: PresignedUploadRequest = {
        filename,
        file_type: fileType,
      };

      const response = await httpClient.post<
        ApiEnvelope<PresignedUploadResponse>
      >("/api/1.0/storage/generate-upload-url", request);

      if (!response.data.success || !response.data.data) {
        throw new Error(
          response.data.message || "Failed to generate upload URL",
        );
      }

      return response.data.data;
    } catch (error) {
      if (error instanceof AxiosError) {
        throw new Error(
          error.response?.data?.message || "Failed to generate upload URL",
        );
      }
      throw error;
    }
  }

  async uploadFile(uploadUrl: string, file: File): Promise<void> {
    try {
      const response = await fetch(uploadUrl, {
        method: "PUT",
        body: file,
      });

      if (!response.ok) {
        const errorText = await response.text();
        console.error("Upload failed:", errorText);
        throw new Error(`Upload failed with status: ${response.status}`);
      }
    } catch (error) {
      if (error instanceof Error) {
        throw new Error(`File upload failed: ${error.message}`);
      }
      throw new Error("File upload failed");
    }
  }

  constructFileUrl(fileKey: string): string {
    return `${MINIO_BASE_URL}/${MINIO_BUCKET}/${fileKey}`;
  }

  async uploadFileWithPresignedUrl(file: File): Promise<string> {
    const presignedData = await this.generatePresignedUrl(file.name, file.type);

    await this.uploadFile(presignedData.upload_url, file);

    return this.constructFileUrl(presignedData.file_key);
  }
}

export const storageService = new StorageService();
export default storageService;
