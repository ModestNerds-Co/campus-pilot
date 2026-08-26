//
//  campus-pilot
//  vehicle-form-modal.tsx - Vehicle Form Modal
//
//  Created by Ngonidzashe Mangudya on 21/08/2026.
//  Copyright (c) 2025 Codecraft Solutions
//

import React, { useState, useEffect } from "react";
import { Loader2 } from "lucide-react";
import { vehiclesService } from "../services/vehicles-service";
import type { CreateVehicleRequest, UpdateVehicleRequest, Vehicle } from "../types";
import toast from "react-hot-toast";
import { Button } from "@/components/ui/button";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { DialogShell, DialogHeader, DialogBody, DialogFooter } from "@/components/ui/dialog";

interface VehicleFormModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: () => void;
  vehicle?: Vehicle;
}

const EMPTY_FORM = {
  registration_number: "",
  make: "",
  model: "",
  year: "",
  vehicle_type: "bus",
  capacity: "",
  fuel_type: "diesel",
  status: "active",
  current_odometer: "0",
  insurance_expiry: "",
  license_expiry: "",
  notes: "",
};

export const VehicleFormModal: React.FC<VehicleFormModalProps> = ({ isOpen, onClose, onSuccess, vehicle }) => {
  const [formData, setFormData] = useState(EMPTY_FORM);
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    if (!isOpen) return;
    if (vehicle) {
      setFormData({
        registration_number: vehicle.registration_number,
        make: vehicle.make,
        model: vehicle.model,
        year: vehicle.year?.toString() ?? "",
        vehicle_type: vehicle.vehicle_type,
        capacity: vehicle.capacity?.toString() ?? "",
        fuel_type: vehicle.fuel_type,
        status: vehicle.status,
        current_odometer: vehicle.current_odometer.toString(),
        insurance_expiry: vehicle.insurance_expiry ?? "",
        license_expiry: vehicle.license_expiry ?? "",
        notes: vehicle.notes ?? "",
      });
    } else {
      setFormData(EMPTY_FORM);
    }
  }, [vehicle, isOpen]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!formData.registration_number.trim() || !formData.make.trim() || !formData.model.trim()) {
      toast.error("Registration number, make, and model are required");
      return;
    }
    setIsSubmitting(true);
    try {
      const payload: CreateVehicleRequest | UpdateVehicleRequest = {
        registration_number: formData.registration_number.trim(),
        make: formData.make.trim(),
        model: formData.model.trim(),
        year: formData.year ? Number(formData.year) : null,
        vehicle_type: formData.vehicle_type,
        capacity: formData.capacity ? Number(formData.capacity) : null,
        fuel_type: formData.fuel_type,
        status: formData.status,
        current_odometer: Number(formData.current_odometer) || 0,
        insurance_expiry: formData.insurance_expiry || null,
        license_expiry: formData.license_expiry || null,
        notes: formData.notes || null,
      };

      const response = vehicle
        ? await vehiclesService.updateVehicle(vehicle.id, payload)
        : await vehiclesService.createVehicle(payload as CreateVehicleRequest);

      if (response.success) {
        toast.success(vehicle ? "Vehicle updated" : "Vehicle added to the fleet");
        onSuccess();
        onClose();
      } else {
        toast.error(response.issues?.[0] || response.message || "Failed to save vehicle");
      }
    } catch {
      toast.error(vehicle ? "Failed to update vehicle" : "Failed to add vehicle");
    } finally {
      setIsSubmitting(false);
    }
  };

  if (!isOpen) return null;

  return (
    <DialogShell open={isOpen} onClose={onClose}>
      <DialogHeader title={vehicle ? "Edit vehicle" : "Add vehicle"} onClose={onClose} />
      <form onSubmit={handleSubmit}>
        <DialogBody className="space-y-4">
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div>
              <Label>
                Registration Number <span className="text-[var(--tone-danger)]">*</span>
              </Label>
              <Input
                value={formData.registration_number}
                onChange={(e) => setFormData({ ...formData, registration_number: e.target.value })}
                placeholder="ADX 1234"
                className="mt-1.5"
                required
              />
            </div>
            <div>
              <Label>Year</Label>
              <Input
                type="number"
                value={formData.year}
                onChange={(e) => setFormData({ ...formData, year: e.target.value })}
                placeholder="2019"
                className="mt-1.5"
              />
            </div>
          </div>

          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div>
              <Label>
                Make <span className="text-[var(--tone-danger)]">*</span>
              </Label>
              <Input
                value={formData.make}
                onChange={(e) => setFormData({ ...formData, make: e.target.value })}
                placeholder="Toyota"
                className="mt-1.5"
                required
              />
            </div>
            <div>
              <Label>
                Model <span className="text-[var(--tone-danger)]">*</span>
              </Label>
              <Input
                value={formData.model}
                onChange={(e) => setFormData({ ...formData, model: e.target.value })}
                placeholder="Quantum"
                className="mt-1.5"
                required
              />
            </div>
          </div>

          <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
            <div>
              <Label>Vehicle type</Label>
              <Select
                value={formData.vehicle_type}
                onChange={(e) => setFormData({ ...formData, vehicle_type: e.target.value })}
                className="mt-1.5"
              >
                <option value="bus">Bus</option>
                <option value="minibus">Minibus</option>
                <option value="van">Van</option>
                <option value="car">Car</option>
                <option value="truck">Truck</option>
              </Select>
            </div>
            <div>
              <Label>Fuel type</Label>
              <Select
                value={formData.fuel_type}
                onChange={(e) => setFormData({ ...formData, fuel_type: e.target.value })}
                className="mt-1.5"
              >
                <option value="diesel">Diesel</option>
                <option value="petrol">Petrol</option>
                <option value="electric">Electric</option>
                <option value="hybrid">Hybrid</option>
              </Select>
            </div>
            <div>
              <Label>Seating Capacity</Label>
              <Input
                type="number"
                value={formData.capacity}
                onChange={(e) => setFormData({ ...formData, capacity: e.target.value })}
                placeholder="18"
                className="mt-1.5"
              />
            </div>
          </div>

          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div>
              <Label>Current odometer (km)</Label>
              <Input
                type="number"
                value={formData.current_odometer}
                onChange={(e) => setFormData({ ...formData, current_odometer: e.target.value })}
                className="mt-1.5"
              />
            </div>
            <div>
              <Label>Status</Label>
              <Select
                value={formData.status}
                onChange={(e) => setFormData({ ...formData, status: e.target.value })}
                className="mt-1.5"
              >
                <option value="active">Active</option>
                <option value="maintenance">Maintenance</option>
                <option value="decommissioned">Decommissioned</option>
              </Select>
            </div>
          </div>

          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div>
              <Label>Insurance expiry</Label>
              <Input
                type="date"
                value={formData.insurance_expiry}
                onChange={(e) => setFormData({ ...formData, insurance_expiry: e.target.value })}
                className="mt-1.5"
              />
            </div>
            <div>
              <Label>Vehicle license expiry</Label>
              <Input
                type="date"
                value={formData.license_expiry}
                onChange={(e) => setFormData({ ...formData, license_expiry: e.target.value })}
                className="mt-1.5"
              />
            </div>
          </div>

          <div>
            <Label>Notes</Label>
            <Textarea
              value={formData.notes}
              onChange={(e) => setFormData({ ...formData, notes: e.target.value })}
              placeholder="Service history, known issues, anything worth flagging..."
              className="mt-1.5"
            />
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
                {vehicle ? "Saving..." : "Adding..."}
              </>
            ) : (
              <>{vehicle ? "Save changes" : "Add vehicle"}</>
            )}
          </Button>
        </DialogFooter>
      </form>
    </DialogShell>
  );
};
