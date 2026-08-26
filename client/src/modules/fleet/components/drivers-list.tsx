//
//  campus-pilot
//  drivers-list.tsx - Fleet Drivers List
//
//  Created by Ngonidzashe Mangudya on 21/08/2026.
//  Copyright (c) 2025 Codecraft Solutions
//

import React, { useState, useEffect } from "react";
import { Contact, Plus, Search, MoreVertical, Edit, Trash2, Phone, AlertTriangle } from "lucide-react";
import { driversService } from "../services/drivers-service";
import type { Driver, DriversListParams } from "../types";
import toast from "react-hot-toast";
import { DriverFormModal } from "./driver-form-modal";
import { Button } from "@/components/ui/button";
import { Input, Select } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import {
  TableWrap,
  TableScroll,
  Table,
  THead,
  TH,
  TBody,
  TR,
  TD,
  TableEmpty,
  TableLoading,
  TableError,
  TableControlsBar,
  TableControlsSearch,
  TableControlsPagination,
} from "@/components/ui/data-table";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";

function isExpiringSoon(dateStr: string | null): boolean {
  if (!dateStr) return false;
  const days = (new Date(dateStr).getTime() - Date.now()) / 86_400_000;
  return days >= 0 && days <= 30;
}

function isExpired(dateStr: string | null): boolean {
  if (!dateStr) return false;
  return new Date(dateStr).getTime() < Date.now();
}

export const DriversList: React.FC = () => {
  const [drivers, setDrivers] = useState<Driver[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<"all" | "active" | "inactive">("all");
  const [currentPage, setCurrentPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [selectedDriver, setSelectedDriver] = useState<Driver | undefined>(undefined);
  const [pendingDelete, setPendingDelete] = useState<Driver | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);

  const fetchDrivers = async () => {
    setIsLoading(true);
    setLoadError(null);
    try {
      const params: DriversListParams = { page: currentPage, per_page: 20 };
      if (searchQuery) params.search = searchQuery;
      if (statusFilter !== "all") params.status = statusFilter;
      const response = await driversService.listDrivers(params);
      if (response.success && response.data) {
        setDrivers(response.data.drivers);
        setTotalPages(response?.pagination?.total_pages ?? 1);
      } else {
        setLoadError(response.message || "The driver roster could not be read.");
      }
    } catch (error) {
      setLoadError("Campus Pilot could not reach the driver roster. Check the connection and try again.");
      console.error(error);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    fetchDrivers();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentPage, statusFilter]);

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault();
    setCurrentPage(1);
    fetchDrivers();
  };

  const handleDelete = async () => {
    if (!pendingDelete || isDeleting) return;
    setIsDeleting(true);
    try {
      const response = await driversService.deleteDriver(pendingDelete.id);
      if (response.success) {
        toast.success("Driver removed");
        setPendingDelete(null);
        void fetchDrivers();
      } else {
        toast.error(response.message || "Failed to remove driver");
      }
    } catch {
      toast.error("Failed to remove driver");
    } finally {
      setIsDeleting(false);
    }
  };

  const handleAdd = () => {
    setSelectedDriver(undefined);
    setIsModalOpen(true);
  };
  const handleEdit = (driver: Driver) => {
    setSelectedDriver(driver);
    setIsModalOpen(true);
    setOpenMenuId(null);
  };

  usePageChrome(
    "Drivers",
    <Button onClick={handleAdd}>
      <Plus className="size-4" />
      Add driver
    </Button>,
  );

  return (
    <div className="space-y-6">
      <p className="text-sm text-[var(--text-muted)]">Maintain licensing, status and contact details for school drivers.</p>

      <TableControlsBar>
        <TableControlsSearch onSubmit={handleSearch}>
          <Input
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search name or license number..."
            leadingIcon={<Search className="size-4" />}
            aria-label="Search drivers"
          />
          <Button type="submit" variant="secondary">
            Search
          </Button>
        </TableControlsSearch>
        <Select
          value={statusFilter}
          onChange={(e) => {
            setCurrentPage(1);
            setStatusFilter(e.target.value as typeof statusFilter);
          }}
          className="sm:w-[180px]"
          aria-label="Status filter"
        >
          <option value="all">All statuses</option>
          <option value="active">Active</option>
          <option value="inactive">Inactive</option>
        </Select>
        {!isLoading && drivers.length > 0 && (
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
          <TableLoading columns={5} label="Loading drivers…" />
        ) : loadError ? (
          <TableError description={loadError} onRetry={() => void fetchDrivers()} title="Drivers could not be loaded" />
        ) : drivers.length === 0 ? (
          <TableEmpty
            icon={<Contact className="size-12" />}
            title={searchQuery || statusFilter !== "all" ? "No drivers match these filters" : "No drivers on the roster yet"}
            description={searchQuery || statusFilter !== "all" ? "Try a different search or status filter." : "Add a driver so they can be assigned to vehicles and daily trip logs."}
          />
        ) : (
          <TableScroll>
            <Table>
              <THead>
                <tr>
                  <TH>Driver</TH>
                  <TH>License</TH>
                  <TH>Contact</TH>
                  <TH>Status</TH>
                  <TH className="text-right">Actions</TH>
                </tr>
              </THead>
              <TBody>
                {drivers.map((driver) => {
                  const licenseWarn = isExpiringSoon(driver.license_expiry) || isExpired(driver.license_expiry);
                  return (
                    <TR key={driver.id}>
                      <TD className="whitespace-nowrap">
                        <div className="flex items-center gap-3">
                          <div className="flex size-10 items-center justify-center rounded-full bg-[var(--brand-soft)]">
                            <span className="text-sm font-medium text-[var(--brand-strong)]">
                              {driver.full_name.charAt(0).toUpperCase()}
                            </span>
                          </div>
                          <div className="text-sm font-medium text-[var(--text-strong)]">{driver.full_name}</div>
                        </div>
                      </TD>
                      <TD className="whitespace-nowrap text-sm text-[var(--text-strong)]">
                        <div>{driver.license_number}</div>
                        <div className="flex items-center gap-1.5 text-xs text-[var(--text-muted)]">
                          {driver.license_class ?? "—"}
                          {licenseWarn && (
                            <span
                              className={`inline-flex items-center gap-1 ${
                                isExpired(driver.license_expiry) ? "text-[var(--tone-danger)]" : "text-[var(--tone-warn)]"
                              }`}
                            >
                              <AlertTriangle className="size-3" />
                              {isExpired(driver.license_expiry) ? "expired" : "expiring soon"}
                            </span>
                          )}
                        </div>
                      </TD>
                      <TD className="whitespace-nowrap text-sm text-[var(--text-strong)]">
                        {driver.phone ? (
                          <span className="inline-flex items-center gap-1.5">
                            <Phone className="size-3.5 text-[var(--text-subtle)]" />
                            {driver.phone}
                          </span>
                        ) : (
                          <span className="text-[var(--text-muted)]">—</span>
                        )}
                      </TD>
                      <TD className="whitespace-nowrap">
                        <Badge tone={driver.status === "active" ? "success" : "neutral"} className="capitalize">
                          {driver.status}
                        </Badge>
                      </TD>
                      <TD className="whitespace-nowrap text-right">
                        <div className="relative flex justify-end">
                          <button
                            onClick={() => setOpenMenuId(openMenuId === driver.id ? null : driver.id)}
                            className="inline-flex size-8 items-center justify-center rounded-[var(--radius-md)] text-[var(--text-muted)] hover:bg-[var(--surface-muted)] hover:text-[var(--text-strong)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]"
                            aria-label="Driver actions"
                          >
                            <MoreVertical className="size-4" />
                          </button>
                          {openMenuId === driver.id && (
                            <div className="absolute right-0 top-9 z-10 w-44 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] py-1 shadow-[var(--shadow-popover)]">
                              <button
                                onClick={() => handleEdit(driver)}
                                className="flex w-full items-center gap-2 px-4 py-2 text-left text-sm text-[var(--text-body)] hover:bg-[var(--surface-muted)]"
                              >
                                <Edit className="size-4" /> Edit
                              </button>
                              <button
                                onClick={() => {
                                  setPendingDelete(driver);
                                  setOpenMenuId(null);
                                }}
                                className="flex w-full items-center gap-2 px-4 py-2 text-left text-sm text-[var(--tone-danger)] hover:bg-[var(--tone-danger-bg)]"
                              >
                                <Trash2 className="size-4" /> Remove
                              </button>
                            </div>
                          )}
                        </div>
                      </TD>
                    </TR>
                  );
                })}
              </TBody>
            </Table>
          </TableScroll>
        )}
      </TableWrap>

      <DriverFormModal
        isOpen={isModalOpen}
        onClose={() => setIsModalOpen(false)}
        onSuccess={fetchDrivers}
        driver={selectedDriver}
      />
      <ConfirmDrawer
        confirmLabel="Remove driver"
        description={`Remove ${pendingDelete?.full_name || "this driver"} from the roster? This action cannot be undone.`}
        isPending={isDeleting}
        onClose={() => setPendingDelete(null)}
        onConfirm={() => void handleDelete()}
        open={pendingDelete !== null}
        title="Remove driver?"
      />
    </div>
  );
};
