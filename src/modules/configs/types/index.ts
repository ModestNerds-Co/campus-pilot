//
//  campus-pilot
//  types/index.ts - Configs Module Types
//
//  Created by Ngonidzashe Mangudya on 26/09/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

// API Response Envelope
export interface ApiEnvelope<T = any> {
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

// Bootstrap States
export type BootstrapState = "Uninitialized" | "SchoolConfigured" | "Ready";

export interface BootstrapStatus {
  state: BootstrapState;
}

// School Configuration
export interface SchoolConfiguration {
  name: string;
  legal_name: string | null;
  emap_code: string | null;
  phone: string | null;
  email: string | null;
  address_line1: string | null;
  address_line2: string | null;
  city: string | null;
  province: string | null;
  country: string | null;
  timezone: string | null;
  locale: string | null;
  logo_light_b64: string | null;
  logo_dark_b64: string | null;
}

// Administrator Configuration
export interface AdminConfiguration {
  full_name: string;
  email: string;
  phone: string | null;
  password: string;
}

// Form Data Types (client-side)
export interface SchoolFormData {
  name: string;
  legal_name: string;
  emap_code: string;
  email: string;
  phone: string;
  address_line1: string;
  address_line2: string;
  city: string;
  province: string;
  country: string;
  timezone: string;
  locale: string;
  logo_light?: File;
  logo_dark?: File;
}

export interface AdminFormData {
  full_name: string;
  email: string;
  phone: string;
  password: string;
  password_confirm: string;
}

// UI State Types
export interface FormFieldError {
  field: string;
  message: string;
}

export interface FormState<T = any> {
  data: T;
  errors: FormFieldError[];
  isValid: boolean;
  isDirty: boolean;
  isSubmitting: boolean;
}

// Image/Logo Types
export interface LogoPreview {
  file: File;
  url: string;
  dimensions?: { width: number; height: number };
  size: number;
}

export interface ImageValidationResult {
  isValid: boolean;
  error?: string;
  warnings?: string[];
  dimensions?: { width: number; height: number };
  size: number;
}

// Password Strength
export interface PasswordStrength {
  score: 0 | 1 | 2 | 3 | 4; // weak, fair, good, strong, very strong
  feedback: string[];
  isValid: boolean;
  label: 'Very Weak' | 'Weak' | 'Fair' | 'Good' | 'Strong';
}

// Timezone and Locale Options
export interface SelectOption {
  value: string;
  label: string;
}

// Error Types
export interface ConfigsError extends Error {
  code?: string;
  field?: string;
  statusCode?: number;
}

// Event Types
export interface ConfigsEvent {
  type: 'school_configured' | 'admin_created' | 'setup_completed' | 'setup_error';
  timestamp: Date;
  data?: any;
  error?: string;
}

// Hook Return Types
export interface UseBootstrapResult {
  status: BootstrapState | null;
  isLoading: boolean;
  error: ConfigsError | null;
  checkStatus: () => Promise<void>;
}

export interface UseSchoolConfigResult {
  formData: SchoolFormData;
  formState: FormState<SchoolFormData>;
  logoPreview: {
    light?: LogoPreview;
    dark?: LogoPreview;
  };
  updateField: (field: keyof SchoolFormData, value: any) => void;
  setLogo: (type: 'light' | 'dark', file: File | null) => void;
  validateForm: () => Promise<boolean>;
  submitForm: () => Promise<boolean>;
}

export interface UseAdminConfigResult {
  formData: AdminFormData;
  formState: FormState<AdminFormData>;
  passwordStrength: PasswordStrength;
  updateField: (field: keyof AdminFormData, value: any) => void;
  validateForm: () => boolean;
  submitForm: () => Promise<boolean>;
}
