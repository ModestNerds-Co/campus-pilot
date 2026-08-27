//
//  campus-pilot
//  driver-form-modal.tsx - Driver Form Modal
//
//  Created by Ngonidzashe Mangudya on 21/08/2026.
//  Copyright (c) 2025 Codecraft Solutions
//

import React, { useState, useEffect } from "react";
import { Loader2, Search } from "lucide-react";
import { driversService } from "../services/drivers-service";
import type { CreateDriverRequest, UpdateDriverRequest, Driver } from "../types";
import toast from "react-hot-toast";
import { Button } from "@/components/ui/button";
import { Input, Label, Select } from "@/components/ui/input";
import { DialogShell, DialogHeader, DialogBody, DialogFooter } from "@/components/ui/dialog";

interface DriverFormModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: () => void;
  driver?: Driver;
}

const EMPTY_FORM = {
  employee_id: "",
  license_number: "",
  license_class: "",
  license_expiry: "",
  status: "active",
};

export const DriverFormModal: React.FC<DriverFormModalProps> = ({ isOpen, onClose, onSuccess, driver }) => {
  const [formData, setFormData] = useState(EMPTY_FORM);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [candidates, setCandidates] = useState<Driver["employee"][]>([]);
  const [employeeSearch, setEmployeeSearch] = useState("");
  const [isLoadingCandidates, setIsLoadingCandidates] = useState(false);

  useEffect(() => {
    if (!isOpen) return;
    if (driver) {
      setFormData({
        employee_id: driver.employee.id,
        license_number: driver.license_number,
        license_class: driver.license_class ?? "",
        license_expiry: driver.license_expiry ?? "",
        status: driver.status,
      });
    } else {
      setFormData(EMPTY_FORM);
    }
  }, [driver, isOpen]);

  useEffect(() => {
    if (!isOpen || driver) return;
    let active = true;
    setIsLoadingCandidates(true);
    const timer = window.setTimeout(() => {
      void driversService.listCandidates(employeeSearch.trim() || undefined).then((response) => {
        if (active && response.success && response.data) setCandidates(response.data.employees);
      }).finally(() => { if (active) setIsLoadingCandidates(false); });
    }, 180);
    return () => { active = false; window.clearTimeout(timer); };
  }, [driver, employeeSearch, isOpen]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!formData.employee_id || !formData.license_number.trim()) {
      toast.error("Employee and licence number are required");
      return;
    }
    setIsSubmitting(true);
    try {
      const payload: CreateDriverRequest | UpdateDriverRequest = {
        license_number: formData.license_number.trim(),
        license_class: formData.license_class || null,
        license_expiry: formData.license_expiry || null,
        status: formData.status,
        ...(!driver ? { employee_id: formData.employee_id } : {}),
      };

      const response = driver
        ? await driversService.updateDriver(driver.id, payload)
        : await driversService.createDriver(payload as CreateDriverRequest);

      if (response.success) {
        toast.success(driver ? "Driver updated" : "Driver added to the roster");
        onSuccess();
        onClose();
      } else {
        toast.error(response.issues?.[0] || response.message || "Failed to save driver");
      }
    } catch {
      toast.error(driver ? "Failed to update driver" : "Failed to add driver");
    } finally {
      setIsSubmitting(false);
    }
  };

  if (!isOpen) return null;

  return (
    <DialogShell open={isOpen} onClose={onClose}>
      <DialogHeader title={driver ? "Edit driver" : "Add driver"} onClose={onClose} />
      <form onSubmit={handleSubmit}>
        <DialogBody className="space-y-4">
          {driver ? <div className="rounded-[var(--radius-lg)] bg-[var(--surface-muted)] p-4"><p className="font-medium text-[var(--text-strong)]">{driver.employee.display_name}</p><p className="mt-1 text-sm text-[var(--text-muted)]">{driver.employee.employee_number} · Employee details are maintained in HR and payroll.</p></div> : <div className="space-y-2"><Label>Employee <span className="text-[var(--tone-danger)]">*</span></Label><Input leadingIcon={<Search />} onChange={(event) => setEmployeeSearch(event.target.value)} placeholder="Search active employees…" value={employeeSearch} /><Select aria-label="Employee" disabled={isLoadingCandidates} onChange={(event) => setFormData({ ...formData, employee_id: event.target.value })} required value={formData.employee_id}><option value="">{isLoadingCandidates ? "Loading employees…" : "Select an employee"}</option>{candidates.map((employee) => <option key={employee.id} value={employee.id}>{employee.display_name} · {employee.employee_number}</option>)}</Select>{!isLoadingCandidates && candidates.length === 0 ? <p className="text-sm text-[var(--text-muted)]">No eligible employees found. Add or reactivate the employee in HR and payroll first.</p> : null}</div>}

          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div>
              <Label>
                License Number <span className="text-[var(--tone-danger)]">*</span>
              </Label>
              <Input
                value={formData.license_number}
                onChange={(e) => setFormData({ ...formData, license_number: e.target.value })}
                placeholder="DL-99213"
                className="mt-1.5"
                required
              />
            </div>
            <div>
              <Label>License class</Label>
              <Input
                value={formData.license_class}
                onChange={(e) => setFormData({ ...formData, license_class: e.target.value })}
                placeholder="Class 2"
                className="mt-1.5"
              />
            </div>
          </div>

          <div>
            <div>
              <Label>License expiry</Label>
              <Input
                type="date"
                value={formData.license_expiry}
                onChange={(e) => setFormData({ ...formData, license_expiry: e.target.value })}
                className="mt-1.5"
              />
            </div>
          </div>

          <div>
            <Label>Status</Label>
            <Select
              value={formData.status}
              onChange={(e) => setFormData({ ...formData, status: e.target.value })}
              className="mt-1.5"
            >
              <option value="active">Active</option>
              <option value="inactive">Inactive</option>
            </Select>
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
                {driver ? "Saving..." : "Adding..."}
              </>
            ) : (
              <>{driver ? "Save changes" : "Add driver"}</>
            )}
          </Button>
        </DialogFooter>
      </form>
    </DialogShell>
  );
};
