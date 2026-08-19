//
//  campus-pilot
//  role-form-modal.tsx - Role Form Modal Component
//
//  Created by Ngonidzashe Mangudya on 03/10/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import React, { useState, useEffect } from "react";
import { X, Loader2 } from "lucide-react";
import { rolesService } from "../services/roles-service";
import type { Role, CreateRoleRequest, UpdateRoleRequest } from "../types";
import toast from "react-hot-toast";

interface RoleFormModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: () => void;
  role?: Role;
}

const AVAILABLE_PERMISSIONS = [
  "users:view",
  "users:create",
  "users:edit",
  "users:delete",
  "roles:view",
  "roles:create",
  "roles:edit",
  "roles:delete",
  "courses:view",
  "courses:create",
  "courses:edit",
  "courses:delete",
  "students:view",
  "students:create",
  "students:edit",
  "students:delete",
  "staff:view",
  "staff:create",
  "staff:edit",
  "staff:delete",
];

export const RoleFormModal: React.FC<RoleFormModalProps> = ({
  isOpen,
  onClose,
  onSuccess,
  role,
}) => {
  const [formData, setFormData] = useState({
    name: "",
    description: "",
    permissions: [] as string[],
  });
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    if (role) {
      setFormData({
        name: role.name,
        description: role.description || "",
        permissions: role.permissions,
      });
    } else {
      setFormData({
        name: "",
        description: "",
        permissions: [],
      });
    }
  }, [role, isOpen]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    if (!formData.name.trim()) {
      toast.error("Role name is required");
      return;
    }

    if (formData.permissions.length === 0) {
      toast.error("At least one permission is required");
      return;
    }

    setIsSubmitting(true);

    try {
      if (role) {
        const updateData: UpdateRoleRequest = {
          name: formData.name,
          description: formData.description || null,
          permissions: formData.permissions,
        };
        const response = await rolesService.updateRole(role.id, updateData);
        if (response.success) {
          toast.success("Role updated successfully");
          onSuccess();
          onClose();
        } else {
          toast.error(response.message || "Failed to update role");
        }
      } else {
        const createData: CreateRoleRequest = {
          name: formData.name,
          description: formData.description || null,
          permissions: formData.permissions,
        };
        const response = await rolesService.createRole(createData);
        if (response.success) {
          toast.success("Role created successfully");
          onSuccess();
          onClose();
        } else {
          toast.error(response.message || "Failed to create role");
        }
      }
    } catch (error) {
      toast.error(role ? "Failed to update role" : "Failed to create role");
    } finally {
      setIsSubmitting(false);
    }
  };

  const togglePermission = (permission: string) => {
    setFormData((prev) => ({
      ...prev,
      permissions: prev.permissions.includes(permission)
        ? prev.permissions.filter((p) => p !== permission)
        : [...prev.permissions, permission],
    }));
  };

  const toggleAllPermissions = () => {
    setFormData((prev) => ({
      ...prev,
      permissions:
        prev.permissions.length === AVAILABLE_PERMISSIONS.length
          ? []
          : [...AVAILABLE_PERMISSIONS],
    }));
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 overflow-y-auto">
      <div className="flex items-center justify-center min-h-screen px-4">
        {/* Backdrop */}
        <div
          className="fixed inset-0 bg-black/50 transition-opacity"
          onClick={onClose}
        />

        {/* Modal */}
        <div className="relative bg-white dark:bg-gray-800 rounded-lg shadow-xl w-full max-w-2xl">
          {/* Header */}
          <div className="flex items-center justify-between px-6 py-4 border-b border-gray-200 dark:border-gray-700">
            <h2 className="text-xl font-semibold text-gray-900 dark:text-white">
              {role ? "Edit Role" : "Add New Role"}
            </h2>
            <button
              onClick={onClose}
              className="p-1 hover:bg-gray-100 dark:hover:bg-gray-700 rounded transition-colors"
            >
              <X className="w-5 h-5 text-gray-500" />
            </button>
          </div>

          {/* Form */}
          <form onSubmit={handleSubmit} className="p-6 space-y-4">
            {/* Name */}
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Role Name <span className="text-red-500">*</span>
              </label>
              <input
                type="text"
                value={formData.name}
                onChange={(e) =>
                  setFormData({ ...formData, name: e.target.value })
                }
                placeholder="e.g., Teacher, Admin"
                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
                required
              />
            </div>

            {/* Description */}
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Description
              </label>
              <textarea
                value={formData.description}
                onChange={(e) =>
                  setFormData({ ...formData, description: e.target.value })
                }
                placeholder="Brief description of this role"
                rows={3}
                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white resize-none"
              />
            </div>

            {/* Permissions */}
            <div>
              <div className="flex items-center justify-between mb-2">
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                  Permissions <span className="text-red-500">*</span>
                </label>
                <button
                  type="button"
                  onClick={toggleAllPermissions}
                  className="text-sm text-blue-600 dark:text-blue-400 hover:underline"
                >
                  {formData.permissions.length === AVAILABLE_PERMISSIONS.length
                    ? "Deselect All"
                    : "Select All"}
                </button>
              </div>
              <div className="border border-gray-300 dark:border-gray-600 rounded-lg p-4 max-h-64 overflow-y-auto">
                <div className="grid grid-cols-2 gap-3">
                  {AVAILABLE_PERMISSIONS.map((permission) => (
                    <label
                      key={permission}
                      className="flex items-center gap-2 cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-700 p-2 rounded transition-colors"
                    >
                      <input
                        type="checkbox"
                        checked={formData.permissions.includes(permission)}
                        onChange={() => togglePermission(permission)}
                        className="w-4 h-4 text-blue-600 border-gray-300 rounded focus:ring-blue-500"
                      />
                      <span className="text-sm text-gray-700 dark:text-gray-300">
                        {permission}
                      </span>
                    </label>
                  ))}
                </div>
              </div>
              <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                {formData.permissions.length} permission
                {formData.permissions.length !== 1 ? "s" : ""} selected
              </p>
            </div>

            {/* Actions */}
            <div className="flex items-center justify-end gap-3 pt-4 border-t border-gray-200 dark:border-gray-700">
              <button
                type="button"
                onClick={onClose}
                className="px-4 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
                disabled={isSubmitting}
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={isSubmitting}
                className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-lg transition-colors flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {isSubmitting ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" />
                    {role ? "Updating..." : "Creating..."}
                  </>
                ) : (
                  <>{role ? "Update Role" : "Create Role"}</>
                )}
              </button>
            </div>
          </form>
        </div>
      </div>
    </div>
  );
};
