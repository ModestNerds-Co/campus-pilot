//
//  campus-pilot
//  users-list.tsx - Users List Component (token-driven)
//

import React, { useState, useEffect } from "react";
import {
  Users as UsersIcon,
  Plus,
  Search,
  MoreVertical,
  Edit,
  Trash2,
  UserCheck,
  UserX,
  Loader2,
} from "lucide-react";
import { usersService } from "../services/users-service";
import type { User, UsersListParams } from "../types";
import toast from "react-hot-toast";
import { UserFormModal } from "./user-form-modal";
import { Button } from "@/components/ui/button";
import { Input, Select } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { TableWrap, TableScroll, Table, THead, TH, TBody, TR, TD, TableEmpty, TableControlsBar, TableControlsSearch, TableControlsPagination } from "@/components/ui/data-table";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

export const UsersList: React.FC = () => {
  const [users, setUsers] = useState<User[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<"all" | "active" | "inactive">("all");
  const [currentPage, setCurrentPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [selectedUser, setSelectedUser] = useState<User | undefined>(undefined);

  const fetchUsers = async () => {
    setIsLoading(true);
    try {
      const params: UsersListParams = { page: currentPage, per_page: 20 };
      if (searchQuery) params.search = searchQuery;
      if (statusFilter !== "all") params.status = statusFilter;
      const response = await usersService.listUsers(params);
      if (response.success && response.data) {
        setUsers(response.data.users);
        setTotalPages(response?.pagination?.total_pages ?? 1);
      }
    } catch (error) {
      toast.error("Failed to load users");
      console.error(error);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    fetchUsers();
  }, [currentPage, statusFilter]);

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault();
    setCurrentPage(1);
    fetchUsers();
  };

  const handleToggleActive = async (user: User) => {
    try {
      const response = user.is_active ? await usersService.deactivateUser(user.id) : await usersService.activateUser(user.id);
      if (response.success) {
        toast.success(user.is_active ? "User deactivated successfully" : "User activated successfully");
        fetchUsers();
      } else {
        toast.error(response.message || "Failed to update user status");
      }
    } catch {
      toast.error("Failed to update user status");
    }
    setOpenMenuId(null);
  };

  const handleDelete = async (userId: string) => {
    if (!confirm("Are you sure you want to delete this user?")) return;
    try {
      const response = await usersService.deleteUser(userId);
      if (response.success) {
        toast.success("User deleted successfully");
        fetchUsers();
      } else {
        toast.error(response.message || "Failed to delete user");
      }
    } catch {
      toast.error("Failed to delete user");
    }
    setOpenMenuId(null);
  };

  const handleAddUser = () => {
    setSelectedUser(undefined);
    setIsModalOpen(true);
  };
  const handleEditUser = (user: User) => {
    setSelectedUser(user);
    setIsModalOpen(true);
    setOpenMenuId(null);
  };
  const handleModalSuccess = () => {
    fetchUsers();
  };

  usePageChrome(
    "Users",
    <Button onClick={handleAddUser}>
      <Plus className="size-4" />
      Add User
    </Button>,
  );

  return (
    <div className="space-y-6">
      <p className="text-sm text-[var(--text-muted)]">Manage user accounts and permissions</p>

      <TableControlsBar>
        <TableControlsSearch onSubmit={handleSearch}>
          <Input
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search users..."
            leadingIcon={<Search className="size-4" />}
            aria-label="Search users"
          />
          <Button type="submit" variant="secondary">
            Search
          </Button>
        </TableControlsSearch>
        <Select
          value={statusFilter}
          onChange={(e) => setStatusFilter(e.target.value as any)}
          className="sm:w-[180px]"
          aria-label="Status filter"
        >
          <option value="all">All Status</option>
          <option value="active">Active</option>
          <option value="inactive">Inactive</option>
        </Select>
        {!isLoading && users.length > 0 && (
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
        ) : users.length === 0 ? (
          <TableEmpty icon={<UsersIcon className="size-12" />} title="No users found" />
        ) : (
          <TableScroll>
            <Table>
              <THead>
                <tr>
                  <TH>User</TH>
                  <TH>Contact</TH>
                  <TH>Roles</TH>
                  <TH>Status</TH>
                  <TH>Last Login</TH>
                  <TH className="text-right">Actions</TH>
                </tr>
              </THead>
              <TBody>
                {users.map((user) => (
                  <TR key={user.id}>
                    <TD className="whitespace-nowrap">
                      <div className="flex items-center gap-3">
                        <div className="flex size-10 items-center justify-center rounded-full bg-[var(--brand-soft)]">
                          <span className="text-sm font-medium text-[var(--brand-strong)]">
                            {user.full_name.charAt(0).toUpperCase()}
                          </span>
                        </div>
                        <div>
                          <div className="text-sm font-medium text-[var(--text-strong)]">{user.full_name}</div>
                          <div className="text-sm text-[var(--text-muted)]">{user.email}</div>
                        </div>
                      </div>
                    </TD>
                    <TD className="whitespace-nowrap text-sm text-[var(--text-strong)]">{user.phone || "—"}</TD>
                    <TD>
                      <div className="flex flex-wrap gap-1">
                        {user.roles.map((role, index) => (
                          <Badge key={index} tone="brand" className="capitalize">
                            {role}
                          </Badge>
                        ))}
                      </div>
                    </TD>
                    <TD className="whitespace-nowrap">
                      <Badge tone={user.is_active ? "success" : "danger"}>{user.is_active ? "Active" : "Inactive"}</Badge>
                    </TD>
                    <TD className="whitespace-nowrap text-sm text-[var(--text-muted)]">
                      {user.last_login_at ? new Date(user.last_login_at).toLocaleDateString() : "Never"}
                    </TD>
                    <TD className="whitespace-nowrap text-right">
                      <div className="relative flex justify-end">
                        <button
                          onClick={() => setOpenMenuId(openMenuId === user.id ? null : user.id)}
                          className="inline-flex size-8 items-center justify-center rounded-[var(--radius-md)] text-[var(--text-muted)] hover:bg-[var(--surface-muted)] hover:text-[var(--text-strong)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]"
                          aria-label="User actions"
                        >
                          <MoreVertical className="size-4" />
                        </button>
                        {openMenuId === user.id && (
                          <div className="absolute right-0 top-9 z-10 w-48 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] py-1 shadow-[var(--shadow-popover)]">
                            <button
                              onClick={() => handleEditUser(user)}
                              className="flex w-full items-center gap-2 px-4 py-2 text-left text-sm text-[var(--text-body)] hover:bg-[var(--surface-muted)]"
                            >
                              <Edit className="size-4" /> Edit
                            </button>
                            <button
                              onClick={() => handleToggleActive(user)}
                              className="flex w-full items-center gap-2 px-4 py-2 text-left text-sm text-[var(--text-body)] hover:bg-[var(--surface-muted)]"
                            >
                              {user.is_active ? (
                                <>
                                  <UserX className="size-4" /> Deactivate
                                </>
                              ) : (
                                <>
                                  <UserCheck className="size-4" /> Activate
                                </>
                              )}
                            </button>
                            <button
                              onClick={() => handleDelete(user.id)}
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

      <UserFormModal isOpen={isModalOpen} onClose={() => setIsModalOpen(false)} onSuccess={handleModalSuccess} user={selectedUser} />
    </div>
  );
};
