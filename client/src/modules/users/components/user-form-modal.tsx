/**
 * Owns the add/edit user drawer and delegation-aware role selection.
 * Password changes are deliberately outside this editor until a reset workflow exists.
 */

import React, { useEffect, useMemo, useState } from "react";
import { AlertCircle, Eye, EyeOff, Loader2 } from "lucide-react";
import { usersService } from "../services/users-service";
import { rolesService } from "../services/roles-service";
import type { User, CreateUserRequest, UpdateUserRequest, Role } from "../types";
import toast from "react-hot-toast";
import { Button } from "@/components/ui/button";
import { Input, Label } from "@/components/ui/input";
import { DialogShell, DialogHeader, DialogBody, DialogFooter } from "@/components/ui/dialog";
import { useAuthStore } from "@/stores/auth-store";
import { apiErrorMessage, canDelegatePermissions, hasPermission } from "../access-control";

interface UserFormModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: () => void;
  user?: User;
}

export const UserFormModal: React.FC<UserFormModalProps> = ({ isOpen, onClose, onSuccess, user }) => {
  const operatorPermissions = useAuthStore((state) => state.user?.permissions);
  const canAssignRoles = hasPermission(operatorPermissions, "roles:assign");
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
  const [rolesError, setRolesError] = useState<string | null>(null);

  const assignableRoles = useMemo(
    () => availableRoles.filter((role) => canDelegatePermissions(operatorPermissions, role.permissions)),
    [availableRoles, operatorPermissions],
  );

  useEffect(() => {
    if (isOpen) {
      if (canAssignRoles) void loadRoles();
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
  }, [canAssignRoles, user, isOpen]);

  const loadRoles = async () => {
    setIsLoadingRoles(true);
    setRolesError(null);
    try {
      const response = await rolesService.listRoles({ limit: 100 });
      if (response.success && response.data) {
        setAvailableRoles(response.data.roles);
      } else {
        setRolesError(apiErrorMessage(response, "Roles could not be loaded."));
      }
    } catch {
      setRolesError("Campus Pilot could not reach the roles directory.");
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
    if ((!user || canAssignRoles) && formData.roles.length === 0) {
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
          is_active: formData.is_active,
        };
        if (canAssignRoles) updateData.roles = formData.roles;
        const response = await usersService.updateUser(user.id, updateData);
        if (response.success) {
          toast.success("User updated successfully");
          onSuccess();
          onClose();
        } else {
          toast.error(apiErrorMessage(response, "Failed to update user"));
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
          toast.error(apiErrorMessage(response, "Failed to create user"));
        }
      }
    } catch {
      toast.error(user ? "Failed to update user" : "Failed to create user");
    } finally {
      setIsSubmitting(false);
    }
  };

  const toggleRole = (roleKey: string) => {
    setFormData((prev) => ({
      ...prev,
      roles: prev.roles.includes(roleKey) ? prev.roles.filter((key) => key !== roleKey) : [...prev.roles, roleKey],
    }));
  };

  if (!isOpen) return null;

  return (
    <DialogShell open={isOpen} onClose={onClose}>
      <DialogHeader title={user ? "Edit user" : "Add user"} onClose={onClose} />
      <form className="flex min-h-0 flex-1 flex-col overflow-hidden" onSubmit={handleSubmit}>
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

          {!user ? <div>
            <Label>
              Temporary password <span className="text-[var(--tone-danger)]">*</span>
            </Label>
            <div className="relative mt-1.5">
              <Input
                type={showPassword ? "text" : "password"}
                value={formData.password}
                onChange={(e) => setFormData({ ...formData, password: e.target.value })}
                placeholder="Enter a temporary password"
                className="pr-10"
                required
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
            <p className="mt-1 text-xs leading-5 text-[var(--text-subtle)]">Share this securely. Password reset will be a separate action.</p>
          </div> : null}

          {canAssignRoles ? <div>
            <Label className="mb-2">
              Roles <span className="text-[var(--tone-danger)]">*</span>
            </Label>
            {isLoadingRoles ? (
              <div className="flex items-center justify-center py-4">
                <Loader2 className="size-5 animate-spin text-[var(--brand)]" />
              </div>
            ) : rolesError ? (
              <div className="rounded-[var(--radius-lg)] border border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] p-4">
                <div className="flex gap-2 text-sm text-[var(--tone-danger)]"><AlertCircle className="mt-0.5 size-4 shrink-0" />{rolesError}</div>
                <Button className="mt-3" onClick={() => void loadRoles()} type="button" variant="secondary">Try again</Button>
              </div>
            ) : assignableRoles.length === 0 ? (
              <div className="rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface-muted)] p-4 text-sm text-[var(--text-muted)]">No roles are available for you to assign.</div>
            ) : (
              <div className="space-y-2 rounded-[var(--radius-lg)] border border-[var(--border)] p-4">
                {assignableRoles.map((role) => (
                  <label
                    key={role.id}
                    className="flex cursor-pointer items-center gap-2 rounded-[var(--radius-md)] p-2 hover:bg-[var(--surface-muted)]"
                  >
                    <input
                      type="checkbox"
                      checked={formData.roles.includes(role.key)}
                      onChange={() => toggleRole(role.key)}
                      className="size-4 rounded border-[var(--border)] text-[var(--brand)] focus:ring-[var(--focus-ring)]"
                    />
                    <div>
                      <span className="text-sm font-medium text-[var(--text-strong)]">{role.name}</span>
                      {role.description && <p className="text-xs leading-5 text-[var(--text-muted)]">{role.description}</p>}
                    </div>
                  </label>
                ))}
              </div>
            )}
            <p className="mt-1 text-xs text-[var(--text-subtle)]">
              {formData.roles.length} role{formData.roles.length !== 1 ? "s" : ""} selected
            </p>
          </div> : null}

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
