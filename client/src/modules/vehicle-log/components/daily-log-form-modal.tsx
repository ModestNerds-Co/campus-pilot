//
//  campus-pilot
//  daily-log-form-modal.tsx - Vehicle Daily Log Form Modal
//
//  Created by Ngonidzashe Mangudya on 21/08/2026.
//  Copyright (c) 2025 Codecraft Solutions
//

import React, { useState, useEffect } from "react";
import { Loader2 } from "lucide-react";
import { vehicleLogService } from "../services/vehicle-log-service";
import type { CreateVehicleDailyLogRequest, UpdateVehicleDailyLogRequest, VehicleDailyLog } from "../types";
import { vehiclesService, driversService } from "@/modules/fleet";
import type { Vehicle, Driver } from "@/modules/fleet";
import toast from "react-hot-toast";
import { Button } from "@/components/ui/button";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { DialogShell, DialogHeader, DialogBody, DialogFooter } from "@/components/ui/dialog";

interface DailyLogFormModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: () => void;
  log?: VehicleDailyLog;
}

const todayIso = () => new Date().toISOString().slice(0, 10);

const emptyForm = () => ({
  vehicle_id: "",
  driver_id: "",
  log_date: todayIso(),
  start_odometer: "",
  end_odometer: "",
  destination: "",
  purpose: "",
  fuel_added_liters: "",
  fuel_cost: "",
  status: "draft",
});

export const DailyLogFormModal: React.FC<DailyLogFormModalProps> = ({ isOpen, onClose, onSuccess, log }) => {
  const [formData, setFormData] = useState(emptyForm());
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [vehicles, setVehicles] = useState<Vehicle[]>([]);
  const [drivers, setDrivers] = useState<Driver[]>([]);
  const [isLoadingOptions, setIsLoadingOptions] = useState(true);

  useEffect(() => {
    if (!isOpen) return;
    loadOptions();
    if (log) {
      setFormData({
        vehicle_id: log.vehicle_id,
        driver_id: log.driver_id,
        log_date: log.log_date,
        start_odometer: log.start_odometer.toString(),
        end_odometer: log.end_odometer?.toString() ?? "",
        destination: log.destination ?? "",
        purpose: log.purpose,
        fuel_added_liters: log.fuel_added_liters?.toString() ?? "",
        fuel_cost: log.fuel_cost?.toString() ?? "",
        status: log.status,
      });
    } else {
      setFormData(emptyForm());
    }
  }, [log, isOpen]);

  const loadOptions = async () => {
    setIsLoadingOptions(true);
    try {
      const [vehicleRes, driverRes] = await Promise.all([
        vehiclesService.listVehicles({ per_page: 100, status: "active" }),
        driversService.listDrivers({ per_page: 100, status: "active" }),
      ]);
      if (vehicleRes.success && vehicleRes.data) setVehicles(vehicleRes.data.vehicles);
      if (driverRes.success && driverRes.data) setDrivers(driverRes.data.drivers);
    } catch {
      toast.error("Failed to load vehicles and drivers");
    } finally {
      setIsLoadingOptions(false);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!formData.vehicle_id || !formData.driver_id) {
      toast.error("Select a vehicle and a driver");
      return;
    }
    if (!formData.purpose.trim()) {
      toast.error("Purpose of the trip is required");
      return;
    }
    if (!formData.start_odometer) {
      toast.error("Starting odometer reading is required");
      return;
    }
    if (formData.end_odometer && Number(formData.end_odometer) < Number(formData.start_odometer)) {
      toast.error("End odometer can't be less than the start odometer");
      return;
    }
    setIsSubmitting(true);
    try {
      const payload: CreateVehicleDailyLogRequest | UpdateVehicleDailyLogRequest = {
        vehicle_id: formData.vehicle_id,
        driver_id: formData.driver_id,
        log_date: formData.log_date,
        start_odometer: Number(formData.start_odometer),
        end_odometer: formData.end_odometer ? Number(formData.end_odometer) : null,
        destination: formData.destination || null,
        purpose: formData.purpose.trim(),
        fuel_added_liters: formData.fuel_added_liters ? Number(formData.fuel_added_liters) : null,
        fuel_cost: formData.fuel_cost ? Number(formData.fuel_cost) : null,
        status: formData.status,
      };

      const response = log
        ? await vehicleLogService.updateLog(log.id, payload)
        : await vehicleLogService.createLog(payload as CreateVehicleDailyLogRequest);

      if (response.success) {
        toast.success(log ? "Daily log updated" : "Trip logged");
        onSuccess();
        onClose();
      } else {
        toast.error(response.issues?.[0] || response.message || "Failed to save the daily log");
      }
    } catch {
      toast.error(log ? "Failed to update daily log" : "Failed to log the trip");
    } finally {
      setIsSubmitting(false);
    }
  };

  if (!isOpen) return null;

  const startKm = formData.start_odometer ? Number(formData.start_odometer) : null;
  const endKm = formData.end_odometer ? Number(formData.end_odometer) : null;
  const distanceKm = startKm != null && endKm != null ? endKm - startKm : null;
  const endBeforeStart = distanceKm != null && distanceKm < 0;

  return (
    <DialogShell open={isOpen} onClose={onClose}>
      <DialogHeader title={log ? "Edit daily log" : "Log a trip"} onClose={onClose} />
      <form onSubmit={handleSubmit}>
        <DialogBody className="space-y-4">
          {isLoadingOptions ? (
            <div className="flex items-center justify-center py-6">
              <Loader2 className="size-6 animate-spin text-[var(--brand)]" />
            </div>
          ) : (
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <div>
                <Label>
                  Vehicle <span className="text-[var(--tone-danger)]">*</span>
                </Label>
                <Select
                  value={formData.vehicle_id}
                  onChange={(e) => setFormData({ ...formData, vehicle_id: e.target.value })}
                  className="mt-1.5"
                  required
                >
                  <option value="">Select vehicle...</option>
                  {vehicles.map((v) => (
                    <option key={v.id} value={v.id}>
                      {v.registration_number} — {v.make} {v.model}
                    </option>
                  ))}
                </Select>
              </div>
              <div>
                <Label>
                  Driver <span className="text-[var(--tone-danger)]">*</span>
                </Label>
                <Select
                  value={formData.driver_id}
                  onChange={(e) => setFormData({ ...formData, driver_id: e.target.value })}
                  className="mt-1.5"
                  required
                >
                  <option value="">Select driver...</option>
                  {drivers.map((d) => (
                    <option key={d.id} value={d.id}>
                      {d.full_name}
                    </option>
                  ))}
                </Select>
              </div>
            </div>
          )}

          <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
            <div>
              <Label>Date</Label>
              <Input
                type="date"
                value={formData.log_date}
                onChange={(e) => setFormData({ ...formData, log_date: e.target.value })}
                className="mt-1.5"
              />
            </div>
            <div>
              <Label>
                Start Odometer (km) <span className="text-[var(--tone-danger)]">*</span>
              </Label>
              <Input
                type="number"
                value={formData.start_odometer}
                onChange={(e) => setFormData({ ...formData, start_odometer: e.target.value })}
                className="mt-1.5"
                required
              />
            </div>
            <div>
              <Label>End odometer (km)</Label>
              <Input
                type="number"
                value={formData.end_odometer}
                onChange={(e) => setFormData({ ...formData, end_odometer: e.target.value })}
                className="mt-1.5"
                aria-invalid={endBeforeStart}
              />
            </div>
          </div>

          {distanceKm != null && (
            <p className={`-mt-2 text-xs ${endBeforeStart ? "text-[var(--tone-danger)]" : "text-[var(--text-muted)]"}`}>
              {endBeforeStart
                ? "End odometer can't be less than the start odometer"
                : `Distance for this trip: ${distanceKm.toLocaleString()} km`}
            </p>
          )}

          <div>
            <Label>Destination</Label>
            <Input
              value={formData.destination}
              onChange={(e) => setFormData({ ...formData, destination: e.target.value })}
              placeholder="Karoi"
              className="mt-1.5"
            />
          </div>

          <div>
            <Label>
              Purpose of Trip <span className="text-[var(--tone-danger)]">*</span>
            </Label>
            <Textarea
              value={formData.purpose}
              onChange={(e) => setFormData({ ...formData, purpose: e.target.value })}
              placeholder="School run - Chinhoyi to Karoi"
              className="mt-1.5"
              required
            />
          </div>

          <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
            <div>
              <Label>Fuel Added (L)</Label>
              <Input
                type="number"
                step="0.1"
                value={formData.fuel_added_liters}
                onChange={(e) => setFormData({ ...formData, fuel_added_liters: e.target.value })}
                className="mt-1.5"
              />
            </div>
            <div>
              <Label>Fuel cost</Label>
              <Input
                type="number"
                step="0.01"
                value={formData.fuel_cost}
                onChange={(e) => setFormData({ ...formData, fuel_cost: e.target.value })}
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
                <option value="draft">Draft</option>
                <option value="submitted">Submitted</option>
                <option value="approved">Approved</option>
              </Select>
            </div>
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
                Saving...
              </>
            ) : (
              <>{log ? "Save changes" : "Log trip"}</>
            )}
          </Button>
        </DialogFooter>
      </form>
    </DialogShell>
  );
};
