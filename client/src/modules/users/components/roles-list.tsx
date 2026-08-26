//
//  campus-pilot
//  roles-list.tsx - Roles List Component (token-driven)
//

import React, { useState, useEffect, useRef } from "react";
import { Shield, Plus, Search, MoreVertical, Edit, Trash2 } from "lucide-react";
import { rolesService } from "../services/roles-service";
import type { Role, RolesListParams } from "../types";
import toast from "react-hot-toast";
import { RoleFormModal } from "./role-form-modal";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { TableWrap, TableScroll, Table, THead, TH, TBody, TR, TD, TableEmpty, TableLoading, TableError, TableControlsBar, TableControlsSearch, TableControlsPagination } from "@/components/ui/data-table";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import { useAuthStore } from "@/stores/auth-store";

export const RolesList: React.FC = () => {
  const currentUser = useAuthStore((state) => state.user);
  const [roles, setRoles] = useState<Role[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [currentPage, setCurrentPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [selectedRole, setSelectedRole] = useState<Role | undefined>(undefined);
  const [pendingDelete, setPendingDelete] = useState<Role | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);
  const actionButtonRefs = useRef<Record<string, HTMLButtonElement | null>>({});
  const canCreate = hasPermission(currentUser?.permissions, "roles:create");
  const canEdit = hasPermission(currentUser?.permissions, "roles:edit");
  const canDelete = hasPermission(currentUser?.permissions, "roles:delete");

  const fetchRoles = async () => {
    setIsLoading(true);
    setLoadError(null);
    try {
      const params: RolesListParams = { page: currentPage, limit: 50 };
      if (searchQuery) params.query = searchQuery;
      const response = await rolesService.listRoles(params);
      if (response.success && response.data) {
        setRoles(response.data.roles);
        setTotalPages(response?.pagination?.total_pages || 1);
      } else {
        setLoadError(response.message || "The roles directory could not be read.");
      }
    } catch (error) {
      setLoadError("Campus Pilot could not reach the roles directory. Check the connection and try again.");
      console.error(error);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    fetchRoles();
  }, [currentPage]);

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault();
    setCurrentPage(1);
    fetchRoles();
  };

  const handleDelete = async () => {
    if (!pendingDelete || isDeleting) return;
    setIsDeleting(true);
    try {
      const response = await rolesService.deleteRole(pendingDelete.id);
      if (response.success) {
        toast.success("Role deleted");
        setPendingDelete(null);
        void fetchRoles();
      } else {
        toast.error(response.message || "Failed to delete role");
      }
    } catch {
      toast.error("Failed to delete role");
    } finally {
      setIsDeleting(false);
    }
  };

  const handleAddRole = () => {
    setSelectedRole(undefined);
    setIsModalOpen(true);
  };
  const handleEditRole = (role: Role) => {
    setSelectedRole(role);
    setIsModalOpen(true);
    setOpenMenuId(null);
  };
  const handleCloseRole = () => {
    const roleId = selectedRole?.id;
    setIsModalOpen(false);
    if (roleId) {
      window.requestAnimationFrame(() => actionButtonRefs.current[roleId]?.focus({ preventScroll: true }));
    }
  };
  const handleCloseDelete = () => {
    const roleId = pendingDelete?.id;
    setPendingDelete(null);
    if (roleId) {
      window.requestAnimationFrame(() => actionButtonRefs.current[roleId]?.focus({ preventScroll: true }));
    }
  };
  const handleModalSuccess = () => {
    fetchRoles();
  };

  usePageChrome(
    "Roles and access",
    canCreate ? (
      <Button onClick={handleAddRole}>
        <Plus className="size-4" />
        Create role
      </Button>
    ) : null,
  );

  return (
    <div className="space-y-6">
      <TableControlsBar>
        <TableControlsSearch onSubmit={handleSearch}>
          <Input
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search roles..."
            leadingIcon={<Search className="size-4" />}
            aria-label="Search roles"
          />
          <Button type="submit" variant="secondary">
            Search
          </Button>
        </TableControlsSearch>
        {!isLoading && roles.length > 0 && (
          <TableControlsPagination
            page={currentPage}
            totalPages={totalPages}
            onPrevious={() => setCurrentPage((p) => Math.max(1, p - 1))}
            onNext={() => setCurrentPage((p) => Math.min(totalPages, p + 1))}
          />
        )}
      </TableControlsBar>

      <TableWrap>
        {isLoading ? (
          <TableLoading columns={5} label="Loading roles…" />
        ) : loadError ? (
          <TableError description={loadError} onRetry={() => void fetchRoles()} title="Roles could not be loaded" />
        ) : roles.length === 0 ? (
          <TableEmpty
            description={searchQuery ? "Try a different role name." : "Create a role to group permissions for a school team."}
            icon={<Shield className="size-12" />}
            title={searchQuery ? "No roles match this search" : "No roles yet"}
          />
        ) : (
          <TableScroll>
            <Table className="min-w-[960px] table-fixed">
              <THead>
                <tr>
                  <TH className="w-[220px]">Role name</TH>
                  <TH className="w-[280px]">Description</TH>
                  <TH>Access profile</TH>
                  <TH className="w-[132px]">Created</TH>
                  <TH className="w-[80px] text-right">Actions</TH>
                </tr>
              </THead>
              <TBody>
                {roles.map((role) => (
                  <TR key={role.id}>
                    <TD className="whitespace-nowrap">
                      <div className="flex items-center gap-3">
                        <div className="flex size-10 items-center justify-center rounded-full bg-[var(--accent-100)]">
                          <Shield className="size-5 text-[var(--accent-700)]" />
                        </div>
                        <div>
                          <div className="flex flex-wrap items-center gap-2">
                            <span className="text-sm font-medium text-[var(--text-strong)]">{role.name}</span>
                            {role.is_system ? <Badge tone="neutral">Protected</Badge> : null}
                          </div>
                        </div>
                      </div>
                    </TD>
                    <TD>
                      <div className="max-w-xs truncate text-sm text-[var(--text-muted)]">{role.description || "—"}</div>
                    </TD>
                    <TD>
                      <div>
                        <p className="text-sm font-medium text-[var(--text-strong)]">
                          {role.permissions.includes("*") ? "Full access" : `${permissionNamespaces(role.permissions).length} access areas`}
                        </p>
                        <p className="mt-1 max-w-sm truncate text-xs text-[var(--text-muted)]">
                          {role.permissions.includes("*")
                            ? "All module actions"
                            : permissionNamespaces(role.permissions).map(humanizeKey).join(", ")}
                        </p>
                      </div>
                    </TD>
                    <TD className="whitespace-nowrap text-sm text-[var(--text-muted)]">
                      {new Date(role.created_at).toLocaleDateString()}
                    </TD>
                    <TD className="whitespace-nowrap text-right">
                      <div className="relative flex justify-end">
                        {(canEdit || (canDelete && !role.is_system)) ? <button
                          aria-controls={openMenuId === role.id ? `role-actions-${role.id}` : undefined}
                          aria-expanded={openMenuId === role.id}
                          aria-haspopup="menu"
                          ref={(element) => { actionButtonRefs.current[role.id] = element; }}
                          onClick={() => setOpenMenuId(openMenuId === role.id ? null : role.id)}
                          className="inline-flex size-8 items-center justify-center rounded-[var(--radius-md)] text-[var(--text-muted)] hover:bg-[var(--surface-muted)] hover:text-[var(--text-strong)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]"
                          aria-label={`Actions for ${role.name}`}
                        >
                          <MoreVertical className="size-4" />
                        </button> : null}
                        {openMenuId === role.id && (
                          <div className="absolute right-0 top-9 z-10 w-48 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] py-1 shadow-[var(--shadow-popover)]" id={`role-actions-${role.id}`} role="menu">
                            {canEdit ? <button
                              onClick={() => handleEditRole(role)}
                              role="menuitem"
                              className="flex w-full items-center gap-2 px-4 py-2 text-left text-sm text-[var(--text-body)] hover:bg-[var(--surface-muted)]"
                            >
                              <Edit className="size-4" /> Edit
                            </button> : null}
                            {canDelete && !role.is_system ? <button
                              onClick={() => {
                                setPendingDelete(role);
                                setOpenMenuId(null);
                              }}
                              role="menuitem"
                              className="flex w-full items-center gap-2 px-4 py-2 text-left text-sm text-[var(--tone-danger)] hover:bg-[var(--tone-danger-bg)]"
                            >
                              <Trash2 className="size-4" /> Delete
                            </button> : null}
                          </div>
                        )}
                      </div>
                    </TD>
                  </TR>
                ))}
              </TBody>
            </Table>
          </TableScroll>
        )}
      </TableWrap>

      <RoleFormModal isOpen={isModalOpen} onClose={handleCloseRole} onSuccess={handleModalSuccess} role={selectedRole} />
      <ConfirmDrawer
        confirmLabel="Delete role"
        description={`Delete the ${pendingDelete?.name || "selected"} role? Review any assigned users first; this action cannot be undone.`}
        isPending={isDeleting}
        onClose={handleCloseDelete}
        onConfirm={() => void handleDelete()}
        open={pendingDelete !== null}
        title="Delete role?"
      />
    </div>
  );
};

function permissionNamespaces(permissions: string[]) {
  return Array.from(new Set(permissions.map((permission) => permission.split(":")[0])));
}

function humanizeKey(value: string) {
  return value
    .replace(/_/g, " ")
    .replace(/\b\w/g, (character: string) => character.toUpperCase());
}

function hasPermission(permissions: string[] | undefined, permission: string) {
  return permissions?.includes("*") || permissions?.includes(permission) || false;
}
