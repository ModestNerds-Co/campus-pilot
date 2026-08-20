//
//  campus-pilot
//  role-form-modal.tsx - Role Form Modal Component (token-driven)
//

import React, { useState, useEffect } from "react";
import { Loader2 } from "lucide-react";
import { rolesService } from "../services/roles-service";
import type { Role, CreateRoleRequest, UpdateRoleRequest } from "../types";
import toast from "react-hot-toast";
import { Button } from "@/components/ui/button";
import { Input, Textarea, Label } from "@/components/ui/input";
import { DialogShell, DialogHeader, DialogBody, DialogFooter } from "@/components/ui/dialog";

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

export const RoleFormModal: React.FC<RoleFormModalProps> = ({ isOpen, onClose, onSuccess, role }) => {
  const [formData, setFormData] = useState({ name: "", description: "", permissions: [] as string[] });
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    if (role) {
      setFormData({ name: role.name, description: role.description || "", permissions: role.permissions });
    } else {
      setFormData({ name: "", description: "", permissions: [] });
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
    } catch {
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
      permissions: prev.permissions.length === AVAILABLE_PERMISSIONS.length ? [] : [...AVAILABLE_PERMISSIONS],
    }));
  };

  if (!isOpen) return null;

  return (
    <DialogShell open={isOpen} onClose={onClose}>
      <DialogHeader title={role ? "Edit Role" : "Add New Role"} onClose={onClose} />
      <form onSubmit={handleSubmit}>
        <DialogBody className="space-y-4">
          <div>
            <Label>
              Role Name <span className="text-[var(--tone-danger)]">*</span>
            </Label>
            <Input
              value={formData.name}
              onChange={(e) => setFormData({ ...formData, name: e.target.value })}
              placeholder="e.g., Teacher, Admin"
              className="mt-1.5"
              required
            />
          </div>

          <div>
            <Label>Description</Label>
            <Textarea
              value={formData.description}
              onChange={(e) => setFormData({ ...formData, description: e.target.value })}
              placeholder="Brief description of this role"
              rows={3}
              className="mt-1.5 resize-none"
            />
          </div>

          <div>
            <div className="mb-2 flex items-center justify-between">
              <Label>
                Permissions <span className="text-[var(--tone-danger)]">*</span>
              </Label>
              <button
                type="button"
                onClick={toggleAllPermissions}
                className="text-sm text-[var(--text-link)] hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] rounded"
              >
                {formData.permissions.length === AVAILABLE_PERMISSIONS.length ? "Deselect All" : "Select All"}
              </button>
            </div>
            <div className="max-h-64 overflow-y-auto rounded-[var(--radius-lg)] border border-[var(--border)] p-4">
              <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                {AVAILABLE_PERMISSIONS.map((permission) => (
                  <label
                    key={permission}
                    className="flex cursor-pointer items-center gap-2 rounded-[var(--radius-md)] p-2 hover:bg-[var(--surface-muted)]"
                  >
                    <input
                      type="checkbox"
                      checked={formData.permissions.includes(permission)}
                      onChange={() => togglePermission(permission)}
                      className="size-4 rounded border-[var(--border)] text-[var(--brand)] focus:ring-[var(--focus-ring)]"
                    />
                    <span className="text-sm text-[var(--text-body)]">{permission}</span>
                  </label>
                ))}
              </div>
            </div>
            <p className="mt-1 text-xs text-[var(--text-subtle)]">
              {formData.permissions.length} permission{formData.permissions.length !== 1 ? "s" : ""} selected
            </p>
          </div>
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="ghost" onClick={onClose} disabled={isSubmitting}>
            Cancel
          </Button>
          <Button type="submit" disabled={isSubmitting}>
            {isSubmitting ? (
              <>
                <Loader2 className="size-4 animate-spin" />
                {role ? "Updating..." : "Creating..."}
              </>
            ) : (
              <>{role ? "Update Role" : "Create Role"}</>
            )}
          </Button>
        </DialogFooter>
      </form>
    </DialogShell>
  );
};
