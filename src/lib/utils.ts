//
//  campus-pilot
//  utils.ts
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatDate(date: Date | string | null | undefined): string {
  if (!date) return "-";

  const d = typeof date === "string" ? new Date(date) : date;

  if (isNaN(d.getTime())) return "-";

  return new Intl.DateTimeFormat("en-US", {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(d);
}

export function formatDateCompact(
  date: Date | string | null | undefined,
): string {
  if (!date) return "-";

  const d = typeof date === "string" ? new Date(date) : date;

  if (isNaN(d.getTime())) return "-";

  return new Intl.DateTimeFormat("en-US", {
    month: "2-digit",
    day: "2-digit",
    year: "numeric",
  }).format(d);
}

export function formatDateTime(date: Date | string | null | undefined): string {
  if (!date) return "-";

  const d = typeof date === "string" ? new Date(date) : date;

  if (isNaN(d.getTime())) return "-";

  return new Intl.DateTimeFormat("en-US", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(d);
}

export function formatTimeAgo(date: Date | string | null | undefined): string {
  if (!date) return "-";

  const d = typeof date === "string" ? new Date(date) : date;

  if (isNaN(d.getTime())) return "-";

  const seconds = Math.floor((new Date().getTime() - d.getTime()) / 1000);

  if (seconds < 60) return "just now";
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
  if (seconds < 604800) return `${Math.floor(seconds / 86400)}d ago`;

  return formatDateCompact(d);
}

export function normalizeText(text: string | null | undefined): string {
  if (!text) return "";
  return text.trim().replace(/\s+/g, " ");
}

export function validateEmail(email: string): boolean {
  const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
  return emailRegex.test(email);
}

export function validatePhone(phone: string): boolean {
  // Basic phone validation - adjust regex based on your requirements
  const phoneRegex = /^[\d\s\-\+\(\)]+$/;
  return phoneRegex.test(phone) && phone.replace(/\D/g, "").length >= 10;
}

export function isValidDate(date: Date | string): boolean {
  const d = typeof date === "string" ? new Date(date) : date;
  return !isNaN(d.getTime());
}

export function compareDates(
  date1: Date | string,
  date2: Date | string,
): number {
  const d1 = typeof date1 === "string" ? new Date(date1) : date1;
  const d2 = typeof date2 === "string" ? new Date(date2) : date2;
  return d1.getTime() - d2.getTime();
}

export function truncate(
  text: string | null | undefined,
  maxLength: number = 50,
): string {
  if (!text) return "";
  if (text.length <= maxLength) return text;
  return text.substring(0, maxLength) + "...";
}

export function capitalizeFirst(text: string | null | undefined): string {
  if (!text) return "";
  return text.charAt(0).toUpperCase() + text.slice(1).toLowerCase();
}

export function capitalizeWords(text: string | null | undefined): string {
  if (!text) return "";
  return text
    .split(" ")
    .map((word) => capitalizeFirst(word))
    .join(" ");
}

export function generateId(): string {
  return `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
}

export function debounce<T extends (...args: any[]) => any>(
  func: T,
  wait: number,
): (...args: Parameters<T>) => void {
  let timeoutId: ReturnType<typeof setTimeout> | null = null;

  return function (...args: Parameters<T>) {
    if (timeoutId !== null) {
      clearTimeout(timeoutId);
    }

    timeoutId = setTimeout(() => {
      func(...args);
    }, wait);
  };
}

export function throttle<T extends (...args: any[]) => any>(
  func: T,
  limit: number,
): (...args: Parameters<T>) => void {
  let inThrottle: boolean = false;

  return function (...args: Parameters<T>) {
    if (!inThrottle) {
      func(...args);
      inThrottle = true;
      setTimeout(() => {
        inThrottle = false;
      }, limit);
    }
  };
}

export function deepEqual(obj1: any, obj2: any): boolean {
  if (obj1 === obj2) return true;

  if (obj1 == null || obj2 == null) return false;

  if (typeof obj1 !== "object" || typeof obj2 !== "object") {
    return obj1 === obj2;
  }

  const keys1 = Object.keys(obj1);
  const keys2 = Object.keys(obj2);

  if (keys1.length !== keys2.length) return false;

  for (const key of keys1) {
    if (!keys2.includes(key)) return false;
    if (!deepEqual(obj1[key], obj2[key])) return false;
  }

  return true;
}

export function deepClone<T>(obj: T): T {
  if (obj === null || typeof obj !== "object") return obj;

  if (obj instanceof Date) return new Date(obj.getTime()) as any;

  if (obj instanceof Array) {
    const clonedArr: any[] = [];
    obj.forEach((element) => {
      clonedArr.push(deepClone(element));
    });
    return clonedArr as any;
  }

  if (obj instanceof Object) {
    const clonedObj: any = {};
    for (const key in obj) {
      if (obj.hasOwnProperty(key)) {
        clonedObj[key] = deepClone(obj[key]);
      }
    }
    return clonedObj;
  }

  return obj;
}

export function getChangedFields<T extends Record<string, any>>(
  original: T,
  modified: T,
): Partial<T> {
  const changes: Partial<T> = {};

  for (const key in modified) {
    if (!deepEqual(original[key], modified[key])) {
      changes[key] = modified[key];
    }
  }

  return changes;
}

export function sanitizeInput(input: string): string {
  // Remove any potential SQL injection attempts or dangerous characters
  return input
    .replace(/[<>]/g, "") // Remove HTML tags
    .replace(/['";]/g, "") // Remove quotes and semicolons
    .trim();
}

export function parseBoolean(value: any): boolean {
  if (typeof value === "boolean") return value;
  if (typeof value === "string") {
    return value.toLowerCase() === "true" || value === "1";
  }
  return !!value;
}

export function formatBytes(bytes: number, decimals: number = 2): string {
  if (bytes === 0) return "0 Bytes";

  const k = 1024;
  const dm = decimals < 0 ? 0 : decimals;
  const sizes = ["Bytes", "KB", "MB", "GB", "TB"];

  const i = Math.floor(Math.log(bytes) / Math.log(k));

  return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + " " + sizes[i];
}

export function sortByDate<T>(
  items: T[],
  dateField: keyof T,
  order: "asc" | "desc" = "desc",
): T[] {
  return [...items].sort((a, b) => {
    const dateA = new Date(a[dateField] as any).getTime();
    const dateB = new Date(b[dateField] as any).getTime();
    return order === "desc" ? dateB - dateA : dateA - dateB;
  });
}

export function groupBy<T>(items: T[], key: keyof T): Record<string, T[]> {
  return items.reduce(
    (groups, item) => {
      const groupKey = String(item[key]);
      if (!groups[groupKey]) {
        groups[groupKey] = [];
      }
      groups[groupKey].push(item);
      return groups;
    },
    {} as Record<string, T[]>,
  );
}

export function getErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return "An unknown error occurred";
}

export function isEmptyObject(obj: any): boolean {
  return obj && Object.keys(obj).length === 0 && obj.constructor === Object;
}

export function pick<T extends object, K extends keyof T>(
  obj: T,
  keys: K[],
): Pick<T, K> {
  const result = {} as Pick<T, K>;
  keys.forEach((key) => {
    if (key in obj) {
      result[key] = obj[key];
    }
  });
  return result;
}

export function omit<T extends object, K extends keyof T>(
  obj: T,
  keys: K[],
): Omit<T, K> {
  const result = { ...obj };
  keys.forEach((key) => {
    delete result[key];
  });
  return result as Omit<T, K>;
}

// Document type detection from base64 data
export interface DocumentTypeInfo {
  type: "pdf" | "image" | "unknown";
  mimeType: string;
  extension: string;
}

export function detectDocumentTypeFromBase64(
  base64Data: string,
): DocumentTypeInfo {
  // Guard against null/undefined/non-string input
  if (!base64Data || typeof base64Data !== "string") {
    return {
      type: "unknown",
      mimeType: "application/octet-stream",
      extension: "bin",
    };
  }

  // Remove data URL prefix if present (data:mime/type;base64,)
  const cleanBase64 = base64Data.replace(/^data:[^;]+;base64,/, "");

  // Convert first few characters of base64 to check magic numbers
  try {
    const binaryData = atob(cleanBase64.substring(0, 20));
    const bytes = new Uint8Array(binaryData.length);
    for (let i = 0; i < binaryData.length; i++) {
      bytes[i] = binaryData.charCodeAt(i);
    }

    // Check for PDF signature (%PDF)
    if (
      bytes[0] === 0x25 &&
      bytes[1] === 0x50 &&
      bytes[2] === 0x44 &&
      bytes[3] === 0x46
    ) {
      return {
        type: "pdf",
        mimeType: "application/pdf",
        extension: "pdf",
      };
    }

    // Check for JPEG signature (FF D8 FF)
    if (bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff) {
      return {
        type: "image",
        mimeType: "image/jpeg",
        extension: "jpg",
      };
    }

    // Check for PNG signature (89 50 4E 47)
    if (
      bytes[0] === 0x89 &&
      bytes[1] === 0x50 &&
      bytes[2] === 0x4e &&
      bytes[3] === 0x47
    ) {
      return {
        type: "image",
        mimeType: "image/png",
        extension: "png",
      };
    }

    // Check for GIF signature (GIF8)
    if (
      bytes[0] === 0x47 &&
      bytes[1] === 0x49 &&
      bytes[2] === 0x46 &&
      bytes[3] === 0x38
    ) {
      return {
        type: "image",
        mimeType: "image/gif",
        extension: "gif",
      };
    }

    // Check for WebP signature (RIFF...WEBP)
    if (
      bytes[0] === 0x52 &&
      bytes[1] === 0x49 &&
      bytes[2] === 0x46 &&
      bytes[3] === 0x46
    ) {
      // Check for WEBP at offset 8
      const webpCheck = atob(cleanBase64.substring(0, 32));
      if (webpCheck.includes("WEBP")) {
        return {
          type: "image",
          mimeType: "image/webp",
          extension: "webp",
        };
      }
    }

    // Default to unknown
    return {
      type: "unknown",
      mimeType: "application/octet-stream",
      extension: "bin",
    };
  } catch (error) {
    console.warn("Error detecting document type:", error);
    return {
      type: "unknown",
      mimeType: "application/octet-stream",
      extension: "bin",
    };
  }
}

// Create download link for base64 document
export function createDownloadLinkFromBase64(
  base64Data: string,
  filename: string,
): string {
  const docType = detectDocumentTypeFromBase64(base64Data);
  const cleanBase64 = base64Data.replace(/^data:[^;]+;base64,/, "");

  try {
    const binaryData = atob(cleanBase64);
    const bytes = new Uint8Array(binaryData.length);
    for (let i = 0; i < binaryData.length; i++) {
      bytes[i] = binaryData.charCodeAt(i);
    }

    const blob = new Blob([bytes], { type: docType.mimeType });
    return URL.createObjectURL(blob);
  } catch (error) {
    console.error("Error creating download link:", error);
    throw new Error("Failed to process document data");
  }
}
