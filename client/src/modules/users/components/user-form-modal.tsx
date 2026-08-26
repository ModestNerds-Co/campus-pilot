//
//  campus-pilot
//  user-form-modal.tsx - User Form Modal Component (token-driven)
//

import React, { useState, useEffect } from "react";
import { Loader2, Eye, EyeOff } from "lucide-react";
import { usersService } from "../services/users-service";
import { rolesService } from "../services/roles-service";
import type { User, CreateUserRequest, UpdateUserRequest, Role } from "../types";
import toast from "react-hot-toast";
import { Button } from "@/components/ui/button";
import { Input, Label } from "@/components/ui/input";
import { DialogShell, DialogHeader, DialogBody, DialogFooter } from "@/components/ui/dialog";

interface UserFormModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: () => void;
  user?: User;
}

export const UserFormModal: React.FC<UserFormModalProps> = ({ isOpen, onClose, onSuccess, user }) => {
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
        setFormData({ email: "", full_name: "", password: "", phone: "", roles: [], is_active: true });
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
    } catch {
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
        if (formData.password) updateData.password = formData.password;
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
    } catch {
      toast.error(user ? "Failed to update user" : "Failed to create user");
    } finally {
      setIsSubmitting(false);
    }
  };

  const toggleRole = (roleName: string) => {
    setFormData((prev) => ({
      ...prev,
      roles: prev.roles.includes(roleName) ? prev.roles.filter((r) => r !== roleName) : [...prev.roles, roleName],
    }));
  };

  if (!isOpen) return null;

  return (
    <DialogShell open={isOpen} onClose={onClose}>
      <DialogHeader title={user ? "Edit user" : "Add user"} onClose={onClose} />
      <form onSubmit={handleSubmit}>
        <DialogBody className="space-y-4">
          <div>
            <Label>
              Email <span className="text-[var(--tone-danger)]">*</span>
            </Label>
            <Input
              type="email"
              value={formData.email}
              onChange={(e) => setFormData({ ...formData, email: e.target.value })}
              placeholder="user@example.com"
              className="mt-1.5"
              required
            />
          </div>

          <div>
            <Label>
              Full name <span className="text-[var(--tone-danger)]">*</span>
            </Label>
            <Input
              type="text"
              value={formData.full_name}
              onChange={(e) => setFormData({ ...formData, full_name: e.target.value })}
              placeholder="John Doe"
              className="mt-1.5"
              required
            />
          </div>

          <div>
            <Label>Phone</Label>
            <Input
              type="tel"
              value={formData.phone}
              onChange={(e) => setFormData({ ...formData, phone: e.target.value })}
              placeholder="+123****7890"
              className="mt-1.5"
            />
          </div>

          <div>
            <Label>
              Password {!user && <span className="text-[var(--tone-danger)]">*</span>}
              {user && <span className="ml-1 text-xs font-normal text-[var(--text-subtle)]">(leave blank to keep unchanged)</span>}
            </Label>
            <div className="relative mt-1.5">
              <Input
                type={showPassword ? "text" : "password"}
                value={formData.password}
                onChange={(e) => setFormData({ ...formData, password: e.target.value })}
                placeholder={user ? "Enter new password" : "Enter password"}
                className="pr-10"
                required={!user}
              />
              <button
                type="button"
                onClick={() => setShowPassword(!showPassword)}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-[var(--text-muted)] hover:text-[var(--text-strong)]"
                aria-label={showPassword ? "Hide password" : "Show password"}
              >
                {showPassword ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
              </button>
            </div>
          </div>

          <div>
            <Label className="mb-2">
              Roles <span className="text-[var(--tone-danger)]">*</span>
            </Label>
            {isLoadingRoles ? (
              <div className="flex items-center justify-center py-4">
                <Loader2 className="size-5 animate-spin text-[var(--brand)]" />
              </div>
            ) : (
              <div className="max-h-40 space-y-2 overflow-y-auto rounded-[var(--radius-lg)] border border-[var(--border)] p-4">
                {availableRoles.map((role) => (
                  <label
                    key={role.id}
                    className="flex cursor-pointer items-center gap-2 rounded-[var(--radius-md)] p-2 hover:bg-[var(--surface-muted)]"
                  >
                    <input
                      type="checkbox"
                      checked={formData.roles.includes(role.name)}
                      onChange={() => toggleRole(role.name)}
                      className="size-4 rounded border-[var(--border)] text-[var(--brand)] focus:ring-[var(--focus-ring)]"
                    />
                    <div>
                      <span className="text-sm font-medium text-[var(--text-strong)]">{role.name}</span>
                      {role.description && <p className="text-xs text-[var(--text-muted)]">{role.description}</p>}
                    </div>
                  </label>
                ))}
              </div>
            )}
            <p className="mt-1 text-xs text-[var(--text-subtle)]">
              {formData.roles.length} role{formData.roles.length !== 1 ? "s" : ""} selected
            </p>
          </div>

          <div>
            <label className="flex cursor-pointer items-center gap-2">
              <input
                type="checkbox"
                checked={formData.is_active}
                onChange={(e) => setFormData({ ...formData, is_active: e.target.checked })}
                className="size-4 rounded border-[var(--border)] text-[var(--brand)] focus:ring-[var(--focus-ring)]"
              />
              <span className="text-sm font-medium text-[var(--text-strong)]">Active user</span>
            </label>
            <p className="ml-6 mt-1 text-xs text-[var(--text-subtle)]">Inactive users cannot log in to the system</p>
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
                {user ? "Updating…" : "Creating…"}
              </>
            ) : (
              <>{user ? "Save changes" : "Create user"}</>
            )}
          </Button>
        </DialogFooter>
      </form>
    </DialogShell>
  );
};
