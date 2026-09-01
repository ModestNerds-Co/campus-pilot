//
//  campus-pilot
//  types/index.ts - Users Module Types
//
//  Created by Ngonidzashe Mangudya on 02/10/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

export interface User {
  id: string;
  email: string;
  full_name: string;
  phone: string | null;
  roles: string[];
  is_active: boolean;
  last_login_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateUserRequest {
  email: string;
  full_name: string;
  password: string;
  phone?: string | null;
  roles: string[];
  is_active?: boolean;
}

export interface UpdateUserRequest {
  email?: string;
  full_name?: string;
  phone?: string | null;
  roles?: string[];
  is_active?: boolean;
}

export interface UsersListParams {
  page?: number;
  per_page?: number;
  search?: string;
  role?: string;
  status?: "active" | "inactive" | "all";
  sort?: string;
}

export interface PaginationMeta {
  current_page: number;
  per_page: number;
  total: number;
  total_pages: number;
  has_next: boolean;
  has_prev: boolean;
}

export interface UsersListResponse {
  users: User[];
  pagination: PaginationMeta;
}

export interface ApiEnvelope<T = unknown> {
  success: boolean;
  message: string | null;
  data: T | null;
  pagination: PaginationMeta | null;
  issues: Array<ValidationIssue | string> | null;
  version: number;
  by: string;
}

export interface ValidationIssue {
  code: string;
  detail: string;
  field?: string;
}

export interface Role {
  id: string;
  key: string;
  name: string;
  description: string | null;
  permissions: string[];
  record_scopes: RoleRecordScope[];
  is_system: boolean;
  created_at: string;
  updated_at: string;
}

export interface RoleRecordScope {
  family: string;
  kind: "self" | "assigned" | "campus";
}

export interface CreateRoleRequest {
  name: string;
  description?: string | null;
  permissions: string[];
  record_scopes: RoleRecordScope[];
}

export interface UpdateRoleRequest {
  name?: string;
  description?: string | null;
  permissions?: string[];
  record_scopes?: RoleRecordScope[];
}

export interface RolesListParams {
  page?: number;
  limit?: number;
  query?: string;
}

export interface RolesListResponse {
  roles: Role[];
  pagination: PaginationMeta;
}
