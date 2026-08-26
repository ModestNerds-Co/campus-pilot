//
//  campus-pilot
//  daily-log-list.tsx - Vehicle Daily Log List
//
//  Created by Ngonidzashe Mangudya on 21/08/2026.
//  Copyright (c) 2025 Codecraft Solutions
//

import React, { useState, useEffect, useRef } from "react";
import { NotebookPen, Plus, MoreVertical, Edit, Trash2, Route, Fuel } from "lucide-react";
import { vehicleLogService } from "../services/vehicle-log-service";
import type { VehicleDailyLog, VehicleDailyLogsListParams } from "../types";
import toast from "react-hot-toast";
import { DailyLogFormModal } from "./daily-log-form-modal";
import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/input";
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
  TableControlsPagination,
} from "@/components/ui/data-table";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";

const STATUS_TONE: Record<string, "neutral" | "info" | "success"> = {
  draft: "neutral",
  submitted: "info",
  approved: "success",
};

export const DailyLogList: React.FC = () => {
  const [logs, setLogs] = useState<VehicleDailyLog[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [statusFilter, setStatusFilter] = useState<"all" | "draft" | "submitted" | "approved">("all");
  const [currentPage, setCurrentPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [selectedLog, setSelectedLog] = useState<VehicleDailyLog | undefined>(undefined);
  const [pendingDelete, setPendingDelete] = useState<VehicleDailyLog | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);
  const actionButtonRefs = useRef<Record<string, HTMLButtonElement | null>>({});

  const fetchLogs = async () => {
    setIsLoading(true);
    setLoadError(null);
    try {
      const params: VehicleDailyLogsListParams = { page: currentPage, per_page: 20 };
      if (statusFilter !== "all") params.status = statusFilter;
      const response = await vehicleLogService.listLogs(params);
      if (response.success && response.data) {
        setLogs(response.data.logs);
        setTotalPages(response?.pagination?.total_pages ?? 1);
      } else {
        setLoadError(response.message || "The daily vehicle log could not be read.");
      }
    } catch (error) {
      setLoadError("Campus Pilot could not reach the daily vehicle log. Check the connection and try again.");
      console.error(error);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    fetchLogs();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentPage, statusFilter]);

  const handleDelete = async () => {
    if (!pendingDelete || isDeleting) return;
    setIsDeleting(true);
    try {
      const response = await vehicleLogService.deleteLog(pendingDelete.id);
      if (response.success) {
        toast.success("Daily log entry deleted");
        setPendingDelete(null);
        void fetchLogs();
      } else {
        toast.error(response.message || "Failed to delete entry");
      }
    } catch {
      toast.error("Failed to delete entry");
    } finally {
      setIsDeleting(false);
    }
  };

  const handleAdd = () => {
    setSelectedLog(undefined);
    setIsModalOpen(true);
  };
  const handleEdit = (log: VehicleDailyLog) => {
    setSelectedLog(log);
    setIsModalOpen(true);
    setOpenMenuId(null);
  };
  const handleCloseLog = () => {
    const logId = selectedLog?.id;
    setIsModalOpen(false);
    if (logId) {
      window.requestAnimationFrame(() => actionButtonRefs.current[logId]?.focus({ preventScroll: true }));
    }
  };
  const handleCloseDelete = () => {
    const logId = pendingDelete?.id;
    setPendingDelete(null);
    if (logId) {
      window.requestAnimationFrame(() => actionButtonRefs.current[logId]?.focus({ preventScroll: true }));
    }
  };

  usePageChrome(
    "Daily vehicle log",
    <Button onClick={handleAdd}>
      <Plus className="size-4" />
      Log a trip
    </Button>,
  );

  return (
    <div className="space-y-6">
      <p className="text-sm text-[var(--text-muted)]">Record each trip against the fleet register, driver and odometer history.</p>

      <TableControlsBar>
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
          <option value="draft">Draft</option>
          <option value="submitted">Submitted</option>
          <option value="approved">Approved</option>
        </Select>
        {!isLoading && logs.length > 0 && (
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
          <TableLoading columns={6} label="Loading daily vehicle logs…" />
        ) : loadError ? (
          <TableError description={loadError} onRetry={() => void fetchLogs()} title="Daily logs could not be loaded" />
        ) : logs.length === 0 ? (
          <TableEmpty
            icon={<NotebookPen className="size-12" />}
            title={statusFilter !== "all" ? "No trips match this status" : "No trips logged yet"}
            description={statusFilter !== "all" ? "Try a different status filter." : "Log a trip to record the driver, route, and distance."}
          />
        ) : (
          <TableScroll>
            <Table>
              <THead>
                <tr>
                  <TH>Date</TH>
                  <TH>Vehicle &amp; Driver</TH>
                  <TH>Trip</TH>
                  <TH>Distance</TH>
                  <TH>Status</TH>
                  <TH className="text-right">Actions</TH>
                </tr>
              </THead>
              <TBody>
                {logs.map((log) => {
                  const distance = log.end_odometer != null ? log.end_odometer - log.start_odometer : null;
                  return (
                    <TR key={log.id}>
                      <TD className="whitespace-nowrap text-sm text-[var(--text-strong)]">
                        {new Date(log.log_date).toLocaleDateString(undefined, {
                          year: "numeric",
                          month: "short",
                          day: "numeric",
                        })}
                      </TD>
                      <TD className="whitespace-nowrap">
                        <div className="text-sm font-medium text-[var(--text-strong)]">{log.vehicle_registration}</div>
                        <div className="text-sm text-[var(--text-muted)]">{log.driver_name}</div>
                      </TD>
                      <TD className="max-w-xs">
                        <div className="truncate text-sm text-[var(--text-strong)]">{log.purpose}</div>
                        {log.destination && (
                          <div className="flex items-center gap-1 text-xs text-[var(--text-muted)]">
                            <Route className="size-3" />
                            {log.destination}
                          </div>
                        )}
                        {log.fuel_added_liters != null && (
                          <div className="flex items-center gap-1 text-xs text-[var(--text-subtle)]">
                            <Fuel className="size-3" />
                            {log.fuel_added_liters} L
                          </div>
                        )}
                      </TD>
                      <TD className="whitespace-nowrap text-sm text-[var(--text-strong)]">
                        {distance != null ? `${distance.toLocaleString()} km` : "—"}
                      </TD>
                      <TD className="whitespace-nowrap">
                        <Badge tone={STATUS_TONE[log.status] ?? "neutral"} className="capitalize">
                          {log.status}
                        </Badge>
                      </TD>
                      <TD className="whitespace-nowrap text-right">
                        <div className="relative flex justify-end">
                          <button
                            aria-controls={openMenuId === log.id ? `daily-log-actions-${log.id}` : undefined}
                            aria-expanded={openMenuId === log.id}
                            aria-haspopup="menu"
                            onClick={() => setOpenMenuId(openMenuId === log.id ? null : log.id)}
                            ref={(element) => { actionButtonRefs.current[log.id] = element; }}
                            className="inline-flex size-8 items-center justify-center rounded-[var(--radius-md)] text-[var(--text-muted)] hover:bg-[var(--surface-muted)] hover:text-[var(--text-strong)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]"
                            aria-label="Daily log actions"
                          >
                            <MoreVertical className="size-4" />
                          </button>
                          {openMenuId === log.id && (
                            <div className="absolute right-0 top-9 z-10 w-40 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] py-1 shadow-[var(--shadow-popover)]" id={`daily-log-actions-${log.id}`} role="menu">
                              <button
                                onClick={() => handleEdit(log)}
                                role="menuitem"
                                className="flex w-full items-center gap-2 px-4 py-2 text-left text-sm text-[var(--text-body)] hover:bg-[var(--surface-muted)]"
                              >
                                <Edit className="size-4" /> Edit
                              </button>
                              <button
                                onClick={() => {
                                  setPendingDelete(log);
                                  setOpenMenuId(null);
                                }}
                                role="menuitem"
                                className="flex w-full items-center gap-2 px-4 py-2 text-left text-sm text-[var(--tone-danger)] hover:bg-[var(--tone-danger-bg)]"
                              >
                                <Trash2 className="size-4" /> Delete
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

      <DailyLogFormModal isOpen={isModalOpen} onClose={handleCloseLog} onSuccess={fetchLogs} log={selectedLog} />
      <ConfirmDrawer
        confirmLabel="Delete log"
        description={`Delete the ${pendingDelete ? new Date(pendingDelete.log_date).toLocaleDateString() : "selected"} trip log for ${pendingDelete?.vehicle_registration || "this vehicle"}? This action cannot be undone.`}
        isPending={isDeleting}
        onClose={handleCloseDelete}
        onConfirm={() => void handleDelete()}
        open={pendingDelete !== null}
        title="Delete trip log?"
      />
    </div>
  );
};
