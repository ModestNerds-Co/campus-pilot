//
//  campus-pilot
//  vehicles-list.tsx - Fleet Vehicles List
//
//  Created by Ngonidzashe Mangudya on 21/08/2026.
//  Copyright (c) 2025 Codecraft Solutions
//

import React, { useState, useEffect, useRef } from "react";
import { Truck, Plus, Search, MoreVertical, Edit, Trash2, Gauge, AlertTriangle } from "lucide-react";
import { vehiclesService } from "../services/vehicles-service";
import type { Vehicle, VehiclesListParams } from "../types";
import toast from "react-hot-toast";
import { VehicleFormModal } from "./vehicle-form-modal";
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

const STATUS_TONE: Record<string, "success" | "warn" | "neutral"> = {
  active: "success",
  maintenance: "warn",
  decommissioned: "neutral",
};

function isExpiringSoon(dateStr: string | null): boolean {
  if (!dateStr) return false;
  const days = (new Date(dateStr).getTime() - Date.now()) / 86_400_000;
  return days >= 0 && days <= 30;
}

function isExpired(dateStr: string | null): boolean {
  if (!dateStr) return false;
  return new Date(dateStr).getTime() < Date.now();
}

export const VehiclesList: React.FC = () => {
  const [vehicles, setVehicles] = useState<Vehicle[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<"all" | "active" | "maintenance" | "decommissioned">("all");
  const [currentPage, setCurrentPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [selectedVehicle, setSelectedVehicle] = useState<Vehicle | undefined>(undefined);
  const [pendingDelete, setPendingDelete] = useState<Vehicle | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);
  const actionButtonRefs = useRef<Record<string, HTMLButtonElement | null>>({});

  const fetchVehicles = async () => {
    setIsLoading(true);
    setLoadError(null);
    try {
      const params: VehiclesListParams = { page: currentPage, per_page: 20 };
      if (searchQuery) params.search = searchQuery;
      if (statusFilter !== "all") params.status = statusFilter;
      const response = await vehiclesService.listVehicles(params);
      if (response.success && response.data) {
        setVehicles(response.data.vehicles);
        setTotalPages(response?.pagination?.total_pages ?? 1);
      } else {
        setLoadError(response.message || "The fleet register could not be read.");
      }
    } catch (error) {
      setLoadError("Campus Pilot could not reach the fleet register. Check the connection and try again.");
      console.error(error);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    fetchVehicles();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentPage, statusFilter]);

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault();
    setCurrentPage(1);
    fetchVehicles();
  };

  const handleDelete = async () => {
    if (!pendingDelete || isDeleting) return;
    setIsDeleting(true);
    try {
      const response = await vehiclesService.deleteVehicle(pendingDelete.id);
      if (response.success) {
        toast.success("Vehicle removed");
        setPendingDelete(null);
        void fetchVehicles();
      } else {
        toast.error(response.message || "Failed to remove vehicle");
      }
    } catch {
      toast.error("Failed to remove vehicle");
    } finally {
      setIsDeleting(false);
    }
  };

  const handleAdd = () => {
    setSelectedVehicle(undefined);
    setIsModalOpen(true);
  };
  const handleEdit = (vehicle: Vehicle) => {
    setSelectedVehicle(vehicle);
    setIsModalOpen(true);
    setOpenMenuId(null);
  };
  const handleCloseVehicle = () => {
    const vehicleId = selectedVehicle?.id;
    setIsModalOpen(false);
    if (vehicleId) {
      window.requestAnimationFrame(() => actionButtonRefs.current[vehicleId]?.focus({ preventScroll: true }));
    }
  };
  const handleCloseDelete = () => {
    const vehicleId = pendingDelete?.id;
    setPendingDelete(null);
    if (vehicleId) {
      window.requestAnimationFrame(() => actionButtonRefs.current[vehicleId]?.focus({ preventScroll: true }));
    }
  };

  usePageChrome(
    "Fleet",
    <Button onClick={handleAdd}>
      <Plus className="size-4" />
      Add vehicle
    </Button>,
  );

  return (
    <div className="space-y-6">
      <p className="text-sm text-[var(--text-muted)]">Track registration, condition, odometer readings and compliance dates.</p>

      <TableControlsBar>
        <TableControlsSearch onSubmit={handleSearch}>
          <Input
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search registration, make, model..."
            leadingIcon={<Search className="size-4" />}
            aria-label="Search vehicles"
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
          <option value="maintenance">Maintenance</option>
          <option value="decommissioned">Decommissioned</option>
        </Select>
        {!isLoading && vehicles.length > 0 && (
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
          <TableLoading columns={6} label="Loading vehicles…" />
        ) : loadError ? (
          <TableError description={loadError} onRetry={() => void fetchVehicles()} title="Vehicles could not be loaded" />
        ) : vehicles.length === 0 ? (
          <TableEmpty
            icon={<Truck className="size-12" />}
            title={searchQuery || statusFilter !== "all" ? "No vehicles match these filters" : "No vehicles in the fleet yet"}
            description={searchQuery || statusFilter !== "all" ? "Try a different search or status filter." : "Add the school's first vehicle to start tracking condition and trips."}
          />
        ) : (
          <TableScroll>
            <Table>
              <THead>
                <tr>
                  <TH>Vehicle</TH>
                  <TH>Type</TH>
                  <TH>Odometer</TH>
                  <TH>Compliance</TH>
                  <TH>Status</TH>
                  <TH className="text-right">Actions</TH>
                </tr>
              </THead>
              <TBody>
                {vehicles.map((vehicle) => {
                  const insuranceWarn = isExpiringSoon(vehicle.insurance_expiry) || isExpired(vehicle.insurance_expiry);
                  const licenseWarn = isExpiringSoon(vehicle.license_expiry) || isExpired(vehicle.license_expiry);
                  return (
                    <TR key={vehicle.id}>
                      <TD className="whitespace-nowrap">
                        <div className="flex items-center gap-3">
                          <div className="flex size-10 items-center justify-center rounded-full bg-[var(--brand-soft)]">
                            <Truck className="size-4 text-[var(--brand-strong)]" />
                          </div>
                          <div>
                            <div className="text-sm font-medium text-[var(--text-strong)]">{vehicle.registration_number}</div>
                            <div className="text-sm text-[var(--text-muted)]">
                              {vehicle.make} {vehicle.model}
                              {vehicle.year ? ` · ${vehicle.year}` : ""}
                            </div>
                          </div>
                        </div>
                      </TD>
                      <TD className="whitespace-nowrap text-sm text-[var(--text-strong)]">
                        <span className="capitalize">{vehicle.vehicle_type}</span>
                        {vehicle.capacity ? <span className="text-[var(--text-muted)]"> · {vehicle.capacity} seats</span> : null}
                      </TD>
                      <TD className="whitespace-nowrap text-sm text-[var(--text-strong)]">
                        <span className="inline-flex items-center gap-1.5">
                          <Gauge className="size-3.5 text-[var(--text-subtle)]" />
                          {vehicle.current_odometer.toLocaleString()} km
                        </span>
                      </TD>
                      <TD className="whitespace-nowrap">
                        {insuranceWarn || licenseWarn ? (
                          (() => {
                            const expired = isExpired(vehicle.insurance_expiry) || isExpired(vehicle.license_expiry);
                            return (
                              <span
                                className={`inline-flex items-center gap-1.5 text-xs font-medium ${
                                  expired ? "text-[var(--tone-danger)]" : "text-[var(--tone-warn)]"
                                }`}
                              >
                                <AlertTriangle className="size-3.5" />
                                {expired ? "Expired" : "Expiring soon"}
                              </span>
                            );
                          })()
                        ) : (
                          <span className="text-xs text-[var(--text-subtle)]">Up to date</span>
                        )}
                      </TD>
                      <TD className="whitespace-nowrap">
                        <Badge tone={STATUS_TONE[vehicle.status] ?? "neutral"} className="capitalize">
                          {vehicle.status}
                        </Badge>
                      </TD>
                      <TD className="whitespace-nowrap text-right">
                        <div className="relative flex justify-end">
                          <button
                            aria-controls={openMenuId === vehicle.id ? `vehicle-actions-${vehicle.id}` : undefined}
                            aria-expanded={openMenuId === vehicle.id}
                            aria-haspopup="menu"
                            onClick={() => setOpenMenuId(openMenuId === vehicle.id ? null : vehicle.id)}
                            ref={(element) => { actionButtonRefs.current[vehicle.id] = element; }}
                            className="inline-flex size-8 items-center justify-center rounded-[var(--radius-md)] text-[var(--text-muted)] hover:bg-[var(--surface-muted)] hover:text-[var(--text-strong)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]"
                            aria-label="Vehicle actions"
                          >
                            <MoreVertical className="size-4" />
                          </button>
                          {openMenuId === vehicle.id && (
                            <div className="absolute right-0 top-9 z-10 w-44 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] py-1 shadow-[var(--shadow-popover)]" id={`vehicle-actions-${vehicle.id}`} role="menu">
                              <button
                                onClick={() => handleEdit(vehicle)}
                                role="menuitem"
                                className="flex w-full items-center gap-2 px-4 py-2 text-left text-sm text-[var(--text-body)] hover:bg-[var(--surface-muted)]"
                              >
                                <Edit className="size-4" /> Edit
                              </button>
                              <button
                                onClick={() => {
                                  setPendingDelete(vehicle);
                                  setOpenMenuId(null);
                                }}
                                role="menuitem"
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

      <VehicleFormModal
        isOpen={isModalOpen}
        onClose={handleCloseVehicle}
        onSuccess={fetchVehicles}
        vehicle={selectedVehicle}
      />
      <ConfirmDrawer
        confirmLabel="Remove vehicle"
        description={`Remove ${pendingDelete?.registration_number || "this vehicle"} from the fleet register? This action cannot be undone.`}
        isPending={isDeleting}
        onClose={handleCloseDelete}
        onConfirm={() => void handleDelete()}
        open={pendingDelete !== null}
        title="Remove vehicle?"
      />
    </div>
  );
};
