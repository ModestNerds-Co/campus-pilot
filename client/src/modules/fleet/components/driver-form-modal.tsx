//
//  campus-pilot
//  driver-form-modal.tsx - Driver Form Modal
//
//  Created by Ngonidzashe Mangudya on 21/08/2026.
//  Copyright (c) 2025 Codecraft Solutions
//

import React, { useState, useEffect } from "react";
import { Loader2 } from "lucide-react";
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
  full_name: "",
  license_number: "",
  license_class: "",
  license_expiry: "",
  phone: "",
  status: "active",
};

export const DriverFormModal: React.FC<DriverFormModalProps> = ({ isOpen, onClose, onSuccess, driver }) => {
  const [formData, setFormData] = useState(EMPTY_FORM);
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    if (!isOpen) return;
    if (driver) {
      setFormData({
        full_name: driver.full_name,
        license_number: driver.license_number,
        license_class: driver.license_class ?? "",
        license_expiry: driver.license_expiry ?? "",
        phone: driver.phone ?? "",
        status: driver.status,
      });
    } else {
      setFormData(EMPTY_FORM);
    }
  }, [driver, isOpen]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!formData.full_name.trim() || !formData.license_number.trim()) {
      toast.error("Full name and license number are required");
      return;
    }
    setIsSubmitting(true);
    try {
      const payload: CreateDriverRequest | UpdateDriverRequest = {
        full_name: formData.full_name.trim(),
        license_number: formData.license_number.trim(),
        license_class: formData.license_class || null,
        license_expiry: formData.license_expiry || null,
        phone: formData.phone || null,
        status: formData.status,
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
          <div>
            <Label>
              Full Name <span className="text-[var(--tone-danger)]">*</span>
            </Label>
            <Input
              value={formData.full_name}
              onChange={(e) => setFormData({ ...formData, full_name: e.target.value })}
              placeholder="Tendai Moyo"
              className="mt-1.5"
              required
            />
          </div>

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

          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div>
              <Label>License expiry</Label>
              <Input
                type="date"
                value={formData.license_expiry}
                onChange={(e) => setFormData({ ...formData, license_expiry: e.target.value })}
                className="mt-1.5"
              />
            </div>
            <div>
              <Label>Phone</Label>
              <Input
                type="tel"
                value={formData.phone}
                onChange={(e) => setFormData({ ...formData, phone: e.target.value })}
                placeholder="+263 77 123 4567"
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
