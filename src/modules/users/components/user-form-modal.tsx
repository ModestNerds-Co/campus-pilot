//
//  campus-pilot
//  user-form-modal.tsx - User Form Modal Component
//
//  Created by Ngonidzashe Mangudya on 03/10/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import React, { useState, useEffect } from "react";
import { X, Loader2, Eye, EyeOff } from "lucide-react";
import { usersService } from "../services/users-service";
import { rolesService } from "../services/roles-service";
import type { User, CreateUserRequest, UpdateUserRequest, Role } from "../types";
import toast from "react-hot-toast";

interface UserFormModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: () => void;
  user?: User;
}

export const UserFormModal: React.FC<UserFormModalProps> = ({
  isOpen,
  onClose,
  onSuccess,
  user,
}) => {
  const [formData, setFormData] = useState({
    email: "",
    full_name: "",
    password: "",
    phone: "",
    roles: [] as string[],
    is_active: true,
  });
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [showPassword, setShowPassword] = useState(false);
  const [availableRoles, setAvailableRoles] = useState<Role[]>([]);
  const [isLoadingRoles, setIsLoadingRoles] = useState(true);

  useEffect(() => {
    if (isOpen) {
      loadRoles();
      if (user) {
        setFormData({
          email: user.email,
          full_name: user.full_name,
          password: "",
          phone: user.phone || "",
          roles: user.roles,
          is_active: user.is_active,
        });
      } else {
        setFormData({
          email: "",
          full_name: "",
          password: "",
          phone: "",
          roles: [],
          is_active: true,
        });
      }
    }
  }, [user, isOpen]);

  const loadRoles = async () => {
    setIsLoadingRoles(true);
    try {
      const response = await rolesService.listRoles({ limit: 100 });
      if (response.success && response.data) {
        setAvailableRoles(response.data.roles);
      }
    } catch (error) {
      toast.error("Failed to load roles");
    } finally {
      setIsLoadingRoles(false);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    if (!formData.email.trim()) {
      toast.error("Email is required");
      return;
    }

    if (!formData.full_name.trim()) {
      toast.error("Full name is required");
      return;
    }

    if (!user && !formData.password.trim()) {
      toast.error("Password is required for new users");
      return;
    }

    if (formData.roles.length === 0) {
      toast.error("At least one role is required");
      return;
    }

    setIsSubmitting(true);

    try {
      if (user) {
        const updateData: UpdateUserRequest = {
          email: formData.email,
          full_name: formData.full_name,
          phone: formData.phone || null,
          roles: formData.roles,
          is_active: formData.is_active,
        };
        if (formData.password) {
          updateData.password = formData.password;
        }
        const response = await usersService.updateUser(user.id, updateData);
        if (response.success) {
          toast.success("User updated successfully");
          onSuccess();
          onClose();
        } else {
          toast.error(response.message || "Failed to update user");
        }
      } else {
        const createData: CreateUserRequest = {
          email: formData.email,
          full_name: formData.full_name,
          password: formData.password,
          phone: formData.phone || null,
          roles: formData.roles,
          is_active: formData.is_active,
        };
        const response = await usersService.createUser(createData);
        if (response.success) {
          toast.success("User created successfully");
          onSuccess();
          onClose();
        } else {
          toast.error(response.message || "Failed to create user");
        }
      }
    } catch (error) {
      toast.error(user ? "Failed to update user" : "Failed to create user");
    } finally {
      setIsSubmitting(false);
    }
  };

  const toggleRole = (roleName: string) => {
    setFormData((prev) => ({
      ...prev,
      roles: prev.roles.includes(roleName)
        ? prev.roles.filter((r) => r !== roleName)
        : [...prev.roles, roleName],
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
              {user ? "Edit User" : "Add New User"}
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
            {/* Email */}
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Email <span className="text-red-500">*</span>
              </label>
              <input
                type="email"
                value={formData.email}
                onChange={(e) =>
                  setFormData({ ...formData, email: e.target.value })
                }
                placeholder="user@example.com"
                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
                required
              />
            </div>

            {/* Full Name */}
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Full Name <span className="text-red-500">*</span>
              </label>
              <input
                type="text"
                value={formData.full_name}
                onChange={(e) =>
                  setFormData({ ...formData, full_name: e.target.value })
                }
                placeholder="John Doe"
                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
                required
              />
            </div>

            {/* Phone */}
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Phone
              </label>
              <input
                type="tel"
                value={formData.phone}
                onChange={(e) =>
                  setFormData({ ...formData, phone: e.target.value })
                }
                placeholder="+1234567890"
                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
              />
            </div>

            {/* Password */}
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Password {!user && <span className="text-red-500">*</span>}
                {user && (
                  <span className="text-xs text-gray-500 ml-1">
                    (leave blank to keep unchanged)
                  </span>
                )}
              </label>
              <div className="relative">
                <input
                  type={showPassword ? "text" : "password"}
                  value={formData.password}
                  onChange={(e) =>
                    setFormData({ ...formData, password: e.target.value })
                  }
                  placeholder={user ? "Enter new password" : "Enter password"}
                  className="w-full px-3 py-2 pr-10 border border-gray-300 dark:border-gray-600 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
                  required={!user}
                />
                <button
                  type="button"
                  onClick={() => setShowPassword(!showPassword)}
                  className="absolute right-3 top-1/2 transform -translate-y-1/2 text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
                >
                  {showPassword ? (
                    <EyeOff className="w-4 h-4" />
                  ) : (
                    <Eye className="w-4 h-4" />
                  )}
                </button>
              </div>
            </div>

            {/* Roles */}
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                Roles <span className="text-red-500">*</span>
              </label>
              {isLoadingRoles ? (
                <div className="flex items-center justify-center py-4">
                  <Loader2 className="w-5 h-5 animate-spin text-blue-600" />
                </div>
              ) : (
                <div className="border border-gray-300 dark:border-gray-600 rounded-lg p-4 space-y-2 max-h-40 overflow-y-auto">
                  {availableRoles.map((role) => (
                    <label
                      key={role.id}
                      className="flex items-center gap-2 cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-700 p-2 rounded transition-colors"
                    >
                      <input
                        type="checkbox"
                        checked={formData.roles.includes(role.name)}
                        onChange={() => toggleRole(role.name)}
                        className="w-4 h-4 text-blue-600 border-gray-300 rounded focus:ring-blue-500"
                      />
                      <div>
                        <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
                          {role.name}
                        </span>
                        {role.description && (
                          <p className="text-xs text-gray-500 dark:text-gray-400">
                            {role.description}
                          </p>
                        )}
                      </div>
                    </label>
                  ))}
                </div>
              )}
              <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                {formData.roles.length} role{formData.roles.length !== 1 ? "s" : ""} selected
              </p>
            </div>

            {/* Active Status */}
            <div>
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={formData.is_active}
                  onChange={(e) =>
                    setFormData({ ...formData, is_active: e.target.checked })
                  }
                  className="w-4 h-4 text-blue-600 border-gray-300 rounded focus:ring-blue-500"
                />
                <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
                  Active user
                </span>
              </label>
              <p className="text-xs text-gray-500 dark:text-gray-400 mt-1 ml-6">
                Inactive users cannot log in to the system
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
                    {user ? "Updating..." : "Creating..."}
                  </>
                ) : (
                  <>{user ? "Update User" : "Create User"}</>
                )}
              </button>
            </div>
          </form>
        </div>
      </div>
    </div>
  );
};
