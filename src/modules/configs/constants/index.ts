//
//  campus-pilot
//  constants/index.ts - Configs Module Constants
//
//  Created by Ngonidzashe Mangudya on 26/09/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import type { SelectOption } from "../types";

// API Configuration
export const API_ENDPOINTS = {
  BOOTSTRAP_STATUS: "/api/1.0/kernel/status",
  BOOTSTRAP_SCHOOL: "/api/1.0/kernel/setup-school",
  BOOTSTRAP_ADMIN: "/api/1.0/bootstrap/admin",
  STORAGE_PRESIGNED_URL: "/api/1.0/storage/generate-upload-url",
} as const;

export const API_CONFIG = {
  VERSION: "1.0",
  BY: "CampusPilot",
  TIMEOUT: 30000, // 30 seconds
  RETRY_ATTEMPTS: 3,
  RETRY_DELAY: 1000, // 1 second
} as const;

// Form Validation Constants
export const VALIDATION_RULES = {
  SCHOOL_NAME: {
    MIN_LENGTH: 2,
    MAX_LENGTH: 80,
    REQUIRED: true,
  },
  FULL_NAME: {
    MIN_LENGTH: 2,
    MAX_LENGTH: 80,
    REQUIRED: true,
  },
  EMAIL: {
    REQUIRED: true,
    PATTERN: /^[^\s@]+@[^\s@]+\.[^\s@]+$/,
  },
  PHONE: {
    REQUIRED: false,
    PATTERN: /^[\+]?[1-9][\d]{0,15}$/,
  },
  PASSWORD: {
    MIN_LENGTH: 10,
    REQUIRED_PATTERNS: {
      DIGIT: /\d/,
      LOWERCASE: /[a-z]/,
      UPPERCASE: /[A-Z]/,
      SYMBOL: /[!@#$%^&*(),.?":{}|<>]/,
    },
  },
  LOGO: {
    MAX_SIZE: 2 * 1024 * 1024, // 2MB
    MIN_DIMENSIONS: { width: 128, height: 128 },
    ALLOWED_TYPES: ["image/png", "image/jpeg", "image/jpg", "image/svg+xml"],
    EXTREME_ASPECT_RATIO: { min: 0.33, max: 3 },
  },
} as const;

// Default Values
export const DEFAULT_VALUES = {
  COUNTRY: "Zimbabwe",
  TIMEZONE: "Africa/Harare",
  LOCALE: "en-ZW",
} as const;

// Timezone Options
export const TIMEZONE_OPTIONS: SelectOption[] = [
  { value: "Africa/Harare", label: "Africa/Harare (Zimbabwe)" },
  { value: "Africa/Johannesburg", label: "Africa/Johannesburg (South Africa)" },
  { value: "Africa/Nairobi", label: "Africa/Nairobi (Kenya)" },
  { value: "Africa/Cairo", label: "Africa/Cairo (Egypt)" },
  { value: "Africa/Lagos", label: "Africa/Lagos (Nigeria)" },
  { value: "UTC", label: "UTC (Universal Time)" },
  { value: "Europe/London", label: "Europe/London (UK)" },
  { value: "America/New_York", label: "America/New_York (US Eastern)" },
  { value: "America/Los_Angeles", label: "America/Los_Angeles (US Pacific)" },
  { value: "Asia/Tokyo", label: "Asia/Tokyo (Japan)" },
  { value: "Australia/Sydney", label: "Australia/Sydney" },
];

// Locale Options
export const LOCALE_OPTIONS: SelectOption[] = [
  { value: "en-ZW", label: "English (Zimbabwe)" },
  { value: "sn-ZW", label: "Shona (Zimbabwe)" },
  { value: "nd-ZW", label: "Ndebele (Zimbabwe)" },
  { value: "en-US", label: "English (United States)" },
  { value: "en-GB", label: "English (United Kingdom)" },
  { value: "en-ZA", label: "English (South Africa)" },
  { value: "af-ZA", label: "Afrikaans (South Africa)" },
  { value: "sw-KE", label: "Swahili (Kenya)" },
  { value: "fr-FR", label: "French (France)" },
  { value: "pt-PT", label: "Portuguese (Portugal)" },
];

// Country Options (African focus)
export const COUNTRY_OPTIONS: SelectOption[] = [
  { value: "Zimbabwe", label: "Zimbabwe" },
  { value: "South Africa", label: "South Africa" },
  { value: "Kenya", label: "Kenya" },
  { value: "Nigeria", label: "Nigeria" },
  { value: "Ghana", label: "Ghana" },
  { value: "Tanzania", label: "Tanzania" },
  { value: "Uganda", label: "Uganda" },
  { value: "Rwanda", label: "Rwanda" },
  { value: "Botswana", label: "Botswana" },
  { value: "Zambia", label: "Zambia" },
  { value: "Mozambique", label: "Mozambique" },
  { value: "Other", label: "Other" },
];

// UI Constants
export const UI_CONFIG = {
  FORM_ANIMATION_DURATION: 300,
  TOAST_DURATION: 4000,
  PREVIEW_UPDATE_DELAY: 300,
  LOGO_PREVIEW_SIZE: { width: 64, height: 64 },
  CARD_BORDER_RADIUS: "12px",
  INPUT_BORDER_RADIUS: "8px",
} as const;

// Error Messages
export const ERROR_MESSAGES = {
  NETWORK_ERROR: "Network error. Please check your connection and try again.",
  UNKNOWN_ERROR: "An unexpected error occurred. Please try again.",
  VALIDATION_ERROR: "Please correct the errors below and try again.",
  SERVER_ERROR: "Server error. Please try again later.",
  TIMEOUT_ERROR: "Request timed out. Please try again.",
  OFFLINE_ERROR: "You appear to be offline. Please check your connection.",
} as const;

// Success Messages
export const SUCCESS_MESSAGES = {
  SCHOOL_CONFIGURED: "School configuration saved successfully",
  ADMIN_CREATED: "Administrator account created successfully",
  SETUP_COMPLETED: "Setup completed successfully",
  FORM_SAVED: "Changes saved successfully",
} as const;

// Storage Keys
export const STORAGE_KEYS = {
  BOOTSTRAP_STATE: "campuspilot_bootstrap_state",
  FORM_DRAFT: "campuspilot_form_draft",
  LOGO_CACHE: "campuspilot_logo_cache",
} as const;

// Password Strength Labels
export const PASSWORD_STRENGTH_LABELS = [
  "Very Weak",
  "Weak",
  "Fair",
  "Good",
  "Strong",
] as const;

// Password Strength Colors
export const PASSWORD_STRENGTH_COLORS = [
  "bg-red-500", // Very Weak
  "bg-orange-500", // Weak
  "bg-yellow-500", // Fair
  "bg-blue-500", // Good
  "bg-green-500", // Strong
] as const;
