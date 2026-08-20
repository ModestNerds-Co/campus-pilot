//
//  campus-pilot
//  roles-list.tsx - Roles List Component (token-driven)
//

import React, { useState, useEffect } from "react";
import { Shield, Plus, Search, MoreVertical, Edit, Trash2, Loader2 } from "lucide-react";
import { rolesService } from "../services/roles-service";
import type { Role, RolesListParams } from "../types";
import toast from "react-hot-toast";
import { RoleFormModal } from "./role-form-modal";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { TableWrap, TableScroll, Table, THead, TH, TBody, TR, TD, TableEmpty, TableControlsBar, TableControlsSearch, TableControlsPagination } from "@/components/ui/data-table";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

export const RolesList: React.FC = () => {
  const [roles, setRoles] = useState<Role[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState("");
  const [currentPage, setCurrentPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [selectedRole, setSelectedRole] = useState<Role | undefined>(undefined);

  const fetchRoles = async () => {
    setIsLoading(true);
    try {
      const params: RolesListParams = { page: currentPage, limit: 50 };
      if (searchQuery) params.query = searchQuery;
      const response = await rolesService.listRoles(params);
      if (response.success && response.data) {
        setRoles(response.data.roles);
        setTotalPages(response?.pagination?.total_pages || 1);
      }
    } catch (error) {
      toast.error("Failed to load roles");
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

  const handleDelete = async (roleId: string) => {
    if (!confirm("Are you sure you want to delete this role?")) return;
    try {
      const response = await rolesService.deleteRole(roleId);
      if (response.success) {
        toast.success("Role deleted successfully");
        fetchRoles();
      } else {
        toast.error(response.message || "Failed to delete role");
      }
    } catch {
      toast.error("Failed to delete role");
    }
    setOpenMenuId(null);
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
  const handleModalSuccess = () => {
    fetchRoles();
  };

  usePageChrome(
    "Roles",
    <Button onClick={handleAddRole}>
      <Plus className="size-4" />
      Add Role
    </Button>,
  );

  return (
    <div className="space-y-6">
      <p className="text-sm text-[var(--text-muted)]">Manage roles and permissions</p>

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
          <div className="flex items-center justify-center py-12">
            <Loader2 className="size-8 animate-spin text-[var(--brand)]" />
          </div>
        ) : roles.length === 0 ? (
          <TableEmpty icon={<Shield className="size-12" />} title="No roles found" />
        ) : (
          <TableScroll>
            <Table>
              <THead>
                <tr>
                  <TH>Role Name</TH>
                  <TH>Description</TH>
                  <TH>Permissions</TH>
                  <TH>Created</TH>
                  <TH className="text-right">Actions</TH>
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
                        <div className="text-sm font-medium text-[var(--text-strong)]">{role.name}</div>
                      </div>
                    </TD>
                    <TD>
                      <div className="max-w-xs truncate text-sm text-[var(--text-muted)]">{role.description || "—"}</div>
                    </TD>
                    <TD>
                      <div className="flex flex-wrap gap-1">
                        {role.permissions.slice(0, 3).map((permission, index) => (
                          <Badge key={index} tone="info">
                            {permission}
                          </Badge>
                        ))}
                        {role.permissions.length > 3 && (
                          <Badge tone="neutral">+{role.permissions.length - 3} more</Badge>
                        )}
                      </div>
                    </TD>
                    <TD className="whitespace-nowrap text-sm text-[var(--text-muted)]">
                      {new Date(role.created_at).toLocaleDateString()}
                    </TD>
                    <TD className="whitespace-nowrap text-right">
                      <div className="relative flex justify-end">
                        <button
                          onClick={() => setOpenMenuId(openMenuId === role.id ? null : role.id)}
                          className="inline-flex size-8 items-center justify-center rounded-[var(--radius-md)] text-[var(--text-muted)] hover:bg-[var(--surface-muted)] hover:text-[var(--text-strong)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]"
                          aria-label="Role actions"
                        >
                          <MoreVertical className="size-4" />
                        </button>
                        {openMenuId === role.id && (
                          <div className="absolute right-0 top-9 z-10 w-48 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] py-1 shadow-[var(--shadow-popover)]">
                            <button
                              onClick={() => handleEditRole(role)}
                              className="flex w-full items-center gap-2 px-4 py-2 text-left text-sm text-[var(--text-body)] hover:bg-[var(--surface-muted)]"
                            >
                              <Edit className="size-4" /> Edit
                            </button>
                            <button
                              onClick={() => handleDelete(role.id)}
                              className="flex w-full items-center gap-2 px-4 py-2 text-left text-sm text-[var(--tone-danger)] hover:bg-[var(--tone-danger-bg)]"
                            >
                              <Trash2 className="size-4" /> Delete
                            </button>
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

      <RoleFormModal isOpen={isModalOpen} onClose={() => setIsModalOpen(false)} onSuccess={handleModalSuccess} role={selectedRole} />
    </div>
  );
};
