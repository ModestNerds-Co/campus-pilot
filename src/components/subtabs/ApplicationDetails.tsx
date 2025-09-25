//
//  campus-pilot
//  ApplicationDetails.tsx
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { useState, useEffect } from "react";
import { useActiveTab, useUIStore } from "../../stores/uiStore";
import {
  Calendar,
  Hash,
  User,
  FileText,
  Flag,
  Clock,
  AlertCircle,
  Loader2,
  Edit3,
  Settings,
  X,
  CalendarClock,
  Plus,
  MapPin,
  RefreshCw,
} from "lucide-react";
import { formatDate } from "../../lib/utils";
import { cn } from "../../lib/utils";
import { LookupField } from "../LookupField";
import { WorkflowConfigField } from "../WorkflowConfigField";
import { SearchableSelect } from "../SearchableSelect";
import {
  apiClient,
  type CaseDetails,
  type LookupValue,
  type WorkflowStatus,
  type WorkflowConfiguration,
  type ApplicationAppointment,
  type AppointmentCreateData,
  type AppointmentUpdateData,
} from "../../lib/api";
import toast from "react-hot-toast";

interface ApplicationDetailsProps {
  applicationId: number;
}

interface EditableData {
  currentapplicationstatusname: string;
  applicationstatuslookupid: number | null;
  prioritylookupid: number | null | undefined;
}

export function ApplicationDetails({ applicationId }: ApplicationDetailsProps) {
  const activeTab = useActiveTab();
  const { updateStagedChanges } = useUIStore();

  const [applicationData, setApplicationData] = useState<CaseDetails | null>(
    null,
  );
  const [formData, setFormData] = useState<EditableData>({
    currentapplicationstatusname: "",
    applicationstatuslookupid: null,
    prioritylookupid: null,
  });
  const [originalData, setOriginalData] = useState<EditableData | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Lookup options
  const [statusLookups, setStatusLookups] = useState<LookupValue[]>([]);
  const [priorityLookups, setPriorityLookups] = useState<LookupValue[]>([]);

  const [workflowStatuses, setWorkflowStatuses] = useState<WorkflowStatus[]>(
    [],
  );
  const [loadingLookups, setLoadingLookups] = useState(true);

  // Override functionality
  const [isOverrideMode, setIsOverrideMode] = useState(false);
  const [selectedOverrideStatus, setSelectedOverrideStatus] = useState<{
    id: number;
    name: string;
  } | null>(null);
  const [showOverrideConfirm, setShowOverrideConfirm] = useState(false);

  // Appointment functionality
  const [appointment, setAppointment] = useState<ApplicationAppointment | null>(
    null,
  );
  const [appointmentLoading, setAppointmentLoading] = useState(false);
  const [appointmentError, setAppointmentError] = useState<string | null>(null);
  const [isEditingAppointment, setIsEditingAppointment] = useState(false);
  const [showCreateAppointment, setShowCreateAppointment] = useState(false);
  const [appointmentForm, setAppointmentForm] = useState({
    appointmentdate: "",
    starttime: "",
    endtime: "",
    appointmentstatuslookupid: null as number | null,
  });
  const [appointmentStatusOptions, setAppointmentStatusOptions] = useState<
    any[]
  >([]);

  // Generate reference state
  const [generatingReference, setGeneratingReference] = useState(false);

  // Check if application is from online portal (isportalorigin = 1)
  const isOnlinePortalApplication = applicationData?.isportalorigin === 1;

  // Helper function to format organization name for display
  const formatOrganizationName = (orgName?: string, maxLength = 20) => {
    if (!orgName) return null;
    return orgName.length > maxLength
      ? `${orgName.substring(0, maxLength)}...`
      : orgName;
  };

  // Fetch application data
  useEffect(() => {
    const fetchData = async () => {
      try {
        setIsLoading(true);
        const data = await apiClient.getCaseDetails(applicationId);
        setApplicationData(data);

        const editableData: EditableData = {
          currentapplicationstatusname: data.currentapplicationstatusname,
          applicationstatuslookupid: data.applicationstatuslookupid,
          prioritylookupid: data.prioritylookupid ?? null,
        };

        setFormData(editableData);
        setOriginalData(editableData);
      } catch (err) {
        const errorMessage =
          err instanceof Error
            ? err.message
            : "Failed to load application data";
        setError(errorMessage);
        toast.error(errorMessage);
      } finally {
        setIsLoading(false);
      }
    };

    fetchData();
  }, [applicationId]);

  // Fetch lookup data
  useEffect(() => {
    const fetchLookups = async () => {
      try {
        setLoadingLookups(true);
        const [statusData, priorityData, workflowStatusData] =
          await Promise.all([
            apiClient.getLookupsByType("APPLICATION_STATUS"),
            apiClient.getLookupsByType("TRANSACTION_PRIORITY"),
            apiClient.getWorkflowStatuses(),
          ]);

        setStatusLookups(statusData);
        setPriorityLookups(priorityData);
        setWorkflowStatuses(workflowStatusData);
      } catch (err) {
        console.warn("Failed to load lookup data:", err);
        toast.error("Failed to load dropdown options");
      } finally {
        setLoadingLookups(false);
      }
    };

    fetchLookups();
  }, []);

  // Fetch appointment data - only for online portal applications
  useEffect(() => {
    const fetchAppointment = async () => {
      try {
        setAppointmentLoading(true);
        setAppointmentError(null);
        const appointmentData =
          await apiClient.getApplicationAppointment(applicationId);
        if (appointmentData) {
          setAppointment(appointmentData);
          // Pre-populate form data if appointment exists
          setAppointmentForm({
            appointmentdate: appointmentData.appointmentdate.split("T")[0],
            starttime: appointmentData.starttime,
            endtime: appointmentData.endtime,
            appointmentstatuslookupid:
              appointmentData.appointmentstatuslookupid,
          });
        }
      } catch (error) {
        console.error("Failed to load appointment:", error);
        const message =
          error instanceof Error
            ? error.message
            : "Failed to load appointment data";
        setAppointmentError(message);
      } finally {
        setAppointmentLoading(false);
      }
    };

    const fetchAppointmentStatusLookups = async () => {
      try {
        const statusData =
          await apiClient.getLookupsByType("APPOINTMENT_STATUS");
        setAppointmentStatusOptions(statusData);
      } catch (error) {
        console.error("Failed to load appointment status options:", error);
        // Don't set error state for lookup failures, just log them
      }
    };

    // Only fetch appointment data for online portal applications
    if (isOnlinePortalApplication) {
      fetchAppointment();
      fetchAppointmentStatusLookups();
    }
  }, [applicationId, isOnlinePortalApplication]);

  const handleFieldChange = (field: keyof EditableData, value: any) => {
    const newFormData = { ...formData, [field]: value };

    setFormData(newFormData);

    // Update staged changes
    if (activeTab && originalData) {
      const changes: any = {};
      Object.keys(newFormData).forEach((key) => {
        const formKey = key as keyof EditableData;
        if (newFormData[formKey] !== originalData[formKey]) {
          changes[formKey] = newFormData[formKey];
        }
      });

      updateStagedChanges(activeTab.id, "application", changes);
    }
  };

  const handleOverrideConfirm = async () => {
    if (!selectedOverrideStatus) return;

    try {
      // Find the selected workflow status configuration
      const selectedWorkflowStatus = workflowStatuses.find(
        (s) => s.tgworkflowstatusconfigurationid === selectedOverrideStatus.id,
      );
      if (!selectedWorkflowStatus) {
        toast.error("Selected workflow status not found");
        return;
      }

      // Update the application status using workflow configuration
      const overrideData = {
        currentapplicationstatusname: selectedWorkflowStatus.statusname,
      };

      await apiClient.overrideApplicationStatus(
        applicationId,
        selectedWorkflowStatus.tgworkflowstatusconfigurationid,
      );

      // Update local state
      const newFormData = { ...formData, ...overrideData };
      setFormData(newFormData);
      setOriginalData(newFormData);

      // Update the application data
      if (applicationData) {
        setApplicationData({
          ...applicationData,
          ...overrideData,
        });
      }

      toast.success("Application status overridden successfully");

      // Reset override state
      setIsOverrideMode(false);
      setSelectedOverrideStatus(null);
      setShowOverrideConfirm(false);
    } catch (error) {
      console.error("Override error:", error);
      const message =
        error instanceof Error ? error.message : "Failed to override status";
      toast.error(message);
    }
  };

  // Appointment handler functions
  const handleCreateAppointment = async () => {
    if (
      !appointmentForm.appointmentdate ||
      !appointmentForm.starttime ||
      !appointmentForm.endtime
    ) {
      toast.error("Please fill in all required fields");
      return;
    }

    try {
      const appointmentData: AppointmentCreateData = {
        tgapplicationid: applicationId,
        appointmentdate: appointmentForm.appointmentdate,
        starttime: appointmentForm.starttime,
        endtime: appointmentForm.endtime,
        appointmentstatuslookupid:
          appointmentForm.appointmentstatuslookupid || undefined,
      };

      const newAppointment =
        await apiClient.createApplicationAppointment(appointmentData);
      setAppointment(newAppointment);
      setShowCreateAppointment(false);
      toast.success("Appointment created successfully");
    } catch (error) {
      console.error("Create appointment error:", error);
      const message =
        error instanceof Error ? error.message : "Failed to create appointment";
      toast.error(message);
    }
  };

  const handleUpdateAppointment = async () => {
    if (
      !appointment ||
      !appointmentForm.appointmentdate ||
      !appointmentForm.starttime ||
      !appointmentForm.endtime
    ) {
      toast.error("Please fill in all required fields");
      return;
    }

    try {
      const updateData: AppointmentUpdateData = {
        appointmentdate: appointmentForm.appointmentdate,
        starttime: appointmentForm.starttime,
        endtime: appointmentForm.endtime,
        appointmentstatuslookupid:
          appointmentForm.appointmentstatuslookupid || undefined,
      };

      const updatedAppointment = await apiClient.updateApplicationAppointment(
        appointment.tgapplicationappointmentid,
        updateData,
      );
      setAppointment(updatedAppointment);
      setIsEditingAppointment(false);
      toast.success("Appointment updated successfully");
    } catch (error) {
      console.error("Update appointment error:", error);
      const message =
        error instanceof Error ? error.message : "Failed to update appointment";
      toast.error(message);
    }
  };

  const handleDeleteAppointment = async () => {
    if (!appointment) return;

    if (
      !confirm(
        "Are you sure you want to delete this appointment? This action cannot be undone.",
      )
    ) {
      return;
    }

    try {
      await apiClient.deleteApplicationAppointment(
        appointment.tgapplicationappointmentid,
      );
      setAppointment(null);
      setAppointmentForm({
        appointmentdate: "",
        starttime: "",
        endtime: "",
        appointmentstatuslookupid: null,
      });
      toast.success("Appointment deleted successfully");
    } catch (error) {
      console.error("Delete appointment error:", error);
      const message =
        error instanceof Error ? error.message : "Failed to delete appointment";
      toast.error(message);
    }
  };

  // Generate reference handler
  const handleGenerateReference = async () => {
    if (
      !confirm(
        "Are you sure you want to generate a new reference number? This will replace the current reference.",
      )
    ) {
      return;
    }

    try {
      setGeneratingReference(true);
      const response = await fetch(`/api/generate-reference/${applicationId}`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
      });

      if (!response.ok) {
        throw new Error("Failed to generate reference");
      }

      const result = await response.json();

      // Update the application data with the new reference
      if (applicationData) {
        setApplicationData({
          ...applicationData,
          reference: result.newReference,
        });
      }

      toast.success(
        `Reference updated from ${result.oldReference} to ${result.newReference}`,
      );
    } catch (error) {
      console.error("Generate reference error:", error);
      const message =
        error instanceof Error ? error.message : "Failed to generate reference";
      toast.error(message);
    } finally {
      setGeneratingReference(false);
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="flex items-center gap-3">
          <Loader2 className="w-6 h-6 animate-spin text-blue-600" />
          <span className="text-gray-600">Loading application data...</span>
        </div>
      </div>
    );
  }

  if (error || !applicationData) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-center">
          <AlertCircle className="w-12 h-12 text-red-500 mx-auto mb-4" />
          <h3 className="text-lg font-semibold text-gray-900 mb-2">
            Failed to Load Application
          </h3>
          <p className="text-gray-600">
            {error || "Application data not found"}
          </p>
        </div>
      </div>
    );
  }

  const MetaStrip = () => (
    <div className="flex items-center gap-6 text-sm text-gray-500 mb-6">
      <div className="flex items-center gap-2">
        <Hash className="w-4 h-4" />
        <span>ID: {applicationData.tgapplicationid}</span>
      </div>
      <div className="flex items-center gap-2">
        <Calendar className="w-4 h-4" />
        <span>Created: {formatDate(applicationData.createdate)}</span>
      </div>
      <div className="flex items-center gap-2">
        <Clock className="w-4 h-4" />
        <span>Modified: {formatDate(applicationData.modifieddate)}</span>
      </div>
      <div className="flex items-center gap-2">
        <span
          className={cn(
            "px-2 py-1 text-xs font-medium rounded-full",
            applicationData.isactive
              ? "bg-green-100 text-green-800"
              : "bg-gray-100 text-gray-800",
          )}
        >
          {applicationData.isactive ? "Active" : "Inactive"}
        </span>
      </div>
      <div className="flex items-center gap-2">
        <MapPin className="w-4 h-4" />
        <span
          className={cn(
            "px-2 py-1 text-xs font-medium rounded-full max-w-xs",
            isOnlinePortalApplication
              ? "bg-blue-100 text-blue-800"
              : "bg-orange-100 text-orange-800",
          )}
          title={
            isOnlinePortalApplication
              ? "Application submitted through online portal"
              : `Application submitted at office${
                  applicationData?.organisationname
                    ? ` - ${applicationData.organisationname}`
                    : ""
                }`
          }
        >
          <span className="truncate">
            {isOnlinePortalApplication ? "Online Portal" : "Office Walk-in"}
            {!isOnlinePortalApplication &&
              applicationData?.organisationname && (
                <span className="ml-1 opacity-75">
                  ({formatOrganizationName(applicationData.organisationname)})
                </span>
              )}
          </span>
        </span>
      </div>
    </div>
  );

  // Convert lookups to options format
  const statusOptions = statusLookups.map((lookup) => ({
    id: lookup.tglookupid,
    value: lookup.lookupvalue,
    label: lookup.lookupdescription || lookup.lookupvalue,
    description: `ID: ${lookup.tglookupid}`,
  }));

  const priorityOptions = priorityLookups.map((lookup) => ({
    id: lookup.tglookupid,
    value: lookup.lookupvalue,
    label: lookup.lookupdescription || lookup.lookupvalue,
    description: `ID: ${lookup.tglookupid}`,
  }));

  const overrideStatusOptions = workflowStatuses.map((status) => ({
    id: status.tgworkflowstatusconfigurationid,
    value: status.statusname,
    label: status.statusname,
    description: `Workflow Config ID: ${status.tgworkflowstatusconfigurationid}`,
  }));

  const appointmentStatusSelectOptions = appointmentStatusOptions.map(
    (lookup) => ({
      id: lookup.tglookupid,
      value: lookup.lookupvalue,
      label: lookup.lookupdescription || lookup.lookupvalue,
      description: `ID: ${lookup.tglookupid}`,
    }),
  );

  return (
    <div className="space-y-6">
      <MetaStrip />

      {/* Editable Application Information */}
      <div className="bg-white rounded-lg border border-gray-200 p-6">
        <h3 className="text-lg font-semibold text-gray-900 mb-6 flex items-center gap-2">
          <Edit3 className="w-5 h-5" />
          Editable Fields
        </h3>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          {/* Current Application Status Name */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Current Status Name
            </label>
            {isOverrideMode ? (
              <div className="space-y-2">
                <SearchableSelect
                  options={overrideStatusOptions}
                  value={selectedOverrideStatus?.id}
                  onChange={(value: number | null) => {
                    if (value) {
                      const option = overrideStatusOptions.find(
                        (opt) => opt.id === value,
                      );
                      setSelectedOverrideStatus({
                        id: value,
                        name: option?.label || "",
                      });
                    } else {
                      setSelectedOverrideStatus(null);
                    }
                  }}
                  placeholder="Select new workflow status..."
                  className="w-full"
                />
                <div className="flex gap-2">
                  <button
                    onClick={() => setShowOverrideConfirm(true)}
                    disabled={!selectedOverrideStatus}
                    className="text-xs px-2 py-1 bg-orange-600 text-white rounded hover:bg-orange-700 disabled:opacity-50"
                  >
                    Confirm Override
                  </button>
                  <button
                    onClick={() => {
                      setIsOverrideMode(false);
                      setSelectedOverrideStatus(null);
                    }}
                    className="text-xs px-2 py-1 bg-gray-500 text-white rounded hover:bg-gray-600"
                  >
                    Cancel
                  </button>
                </div>
              </div>
            ) : (
              <div className="relative">
                <div className="w-full px-3 py-2 bg-gray-50 border border-gray-300 rounded-lg text-gray-700">
                  {formData.currentapplicationstatusname}
                </div>
                <button
                  onClick={() => setIsOverrideMode(true)}
                  className="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-gray-400 hover:text-orange-600 hover:bg-orange-50 rounded"
                  title="Override status"
                >
                  <Settings className="w-4 h-4" />
                </button>
              </div>
            )}
            <div className="text-xs text-gray-500 mt-1">
              {isOverrideMode
                ? "Override mode: Select any status to manually change"
                : "Read-only: Updates when workflow action is selected • Click gear icon to override"}
            </div>
          </div>

          {/* Application Status Lookup ID */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Application Status
            </label>
            <SearchableSelect
              options={statusOptions}
              value={formData.applicationstatuslookupid}
              onChange={(value) =>
                handleFieldChange("applicationstatuslookupid", value)
              }
              placeholder="Select application status..."
              loading={loadingLookups}
              className="w-full"
            />
            <div className="text-xs text-gray-500 mt-1 flex items-center gap-2">
              <span>
                Current ID:{" "}
                {applicationData.applicationstatuslookupid || "Not set"}
              </span>
              {!applicationData.applicationstatuslookupid && (
                <span className="text-amber-600 bg-amber-100 px-2 py-0.5 rounded text-xs">
                  ⚠ Missing status
                </span>
              )}
            </div>
          </div>

          {/* Priority */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Priority Level
            </label>
            <SearchableSelect
              options={priorityOptions}
              value={formData.prioritylookupid}
              onChange={(value) => handleFieldChange("prioritylookupid", value)}
              placeholder="Select priority level..."
              loading={loadingLookups}
              className="w-full"
            />
            <div className="text-xs text-gray-500 mt-1 flex items-center gap-2">
              <span>
                Current ID: {applicationData.prioritylookupid || "Not assigned"}
              </span>
              {!applicationData.prioritylookupid && (
                <span className="text-gray-600 bg-gray-100 px-2 py-0.5 rounded text-xs">
                  💭 Optional field
                </span>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Application Appointment Section - Only for online portal applications */}
      {isOnlinePortalApplication && (
        <div className="bg-white rounded-lg border border-gray-200 p-6">
          <div className="flex items-center justify-between mb-6">
            <h3 className="text-lg font-semibold text-gray-900 flex items-center gap-2">
              <CalendarClock className="w-5 h-5" />
              Application Appointment
              <span
                className="text-xs bg-blue-100 text-blue-800 px-2 py-1 rounded-full ml-2"
                title="This application was submitted through the online portal and supports appointments"
              >
                Online Portal
              </span>
            </h3>
            {!appointment &&
              !showCreateAppointment &&
              !appointmentLoading &&
              !appointmentError && (
                <button
                  onClick={() => setShowCreateAppointment(true)}
                  className="px-3 py-2 bg-blue-600 text-white text-sm rounded-lg hover:bg-blue-700 flex items-center gap-2"
                  title="Create a new appointment for this application"
                >
                  <Plus className="w-4 h-4" />
                  Schedule Appointment
                </button>
              )}
          </div>

          {appointmentLoading ? (
            <div className="flex items-center justify-center py-12">
              <div className="flex items-center gap-3">
                <Loader2 className="w-6 h-6 animate-spin text-blue-600" />
                <span className="text-gray-600">
                  Loading appointment data...
                </span>
              </div>
            </div>
          ) : appointmentError ? (
            <div className="text-center py-12 bg-red-50 rounded-lg border-2 border-dashed border-red-200">
              <AlertCircle className="w-12 h-12 text-red-400 mx-auto mb-3" />
              <h3 className="text-lg font-semibold text-red-900 mb-2">
                Failed to Load Appointment
              </h3>
              <p className="text-red-700 mb-4">{appointmentError}</p>
              <button
                onClick={() => {
                  setAppointmentError(null);
                  // Trigger refetch by toggling the dependency
                  window.location.reload();
                }}
                className="px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 text-sm"
              >
                Try Again
              </button>
            </div>
          ) : appointment ? (
            <div className="space-y-4">
              {isEditingAppointment ? (
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">
                      Appointment Date *
                    </label>
                    <input
                      type="date"
                      value={appointmentForm.appointmentdate}
                      onChange={(e) =>
                        setAppointmentForm({
                          ...appointmentForm,
                          appointmentdate: e.target.value,
                        })
                      }
                      className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
                    />
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">
                      Start Time *
                    </label>
                    <input
                      type="text"
                      placeholder="e.g., 3:00 PM"
                      value={appointmentForm.starttime}
                      onChange={(e) =>
                        setAppointmentForm({
                          ...appointmentForm,
                          starttime: e.target.value,
                        })
                      }
                      className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
                    />
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">
                      End Time *
                    </label>
                    <input
                      type="text"
                      placeholder="e.g., 4:00 PM"
                      value={appointmentForm.endtime}
                      onChange={(e) =>
                        setAppointmentForm({
                          ...appointmentForm,
                          endtime: e.target.value,
                        })
                      }
                      className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
                    />
                  </div>
                  <div className="md:col-span-2 lg:col-span-3">
                    <label className="block text-sm font-medium text-gray-700 mb-1">
                      Appointment Status
                    </label>
                    <SearchableSelect
                      options={appointmentStatusSelectOptions}
                      value={appointmentForm.appointmentstatuslookupid}
                      onChange={(value) =>
                        setAppointmentForm({
                          ...appointmentForm,
                          appointmentstatuslookupid: value,
                        })
                      }
                      placeholder="Select appointment status..."
                      className="w-full max-w-md"
                    />
                  </div>
                  <div className="flex gap-2 md:col-span-2 lg:col-span-3">
                    <button
                      onClick={handleUpdateAppointment}
                      className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700"
                    >
                      Save Changes
                    </button>
                    <button
                      onClick={() => {
                        setIsEditingAppointment(false);
                        if (appointment) {
                          setAppointmentForm({
                            appointmentdate:
                              appointment.appointmentdate.split("T")[0],
                            starttime: appointment.starttime,
                            endtime: appointment.endtime,
                            appointmentstatuslookupid:
                              appointment.appointmentstatuslookupid,
                          });
                        }
                      }}
                      className="px-4 py-2 bg-gray-500 text-white rounded-lg hover:bg-gray-600"
                    >
                      Cancel
                    </button>
                    <button
                      onClick={handleDeleteAppointment}
                      className="px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 ml-auto"
                      title="Permanently remove this appointment"
                    >
                      Delete Appointment
                    </button>
                  </div>
                </div>
              ) : (
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Appointment Date
                    </label>
                    <div className="flex items-center gap-2 text-base text-gray-900">
                      <Calendar className="w-4 h-4" />
                      {formatDate(appointment.appointmentdate)}
                    </div>
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Time Slot
                    </label>
                    <div className="flex items-center gap-2 text-base text-gray-900">
                      <Clock className="w-4 h-4" />
                      {appointment.starttime} - {appointment.endtime}
                    </div>
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Status
                    </label>
                    <LookupField
                      lookupId={appointment.appointmentstatuslookupid}
                      format="value"
                      className="text-base text-gray-900"
                    />
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Reschedule Count
                    </label>
                    <div className="text-base text-gray-900">
                      {appointment.reschedulecount} times
                    </div>
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Created Date
                    </label>
                    <div className="text-base text-gray-900">
                      {formatDate(appointment.createdate)}
                    </div>
                  </div>
                  {appointment.modifieddate && (
                    <div>
                      <label className="block text-sm font-medium text-gray-700 mb-2">
                        Last Modified
                      </label>
                      <div className="text-base text-gray-900">
                        {formatDate(appointment.modifieddate)}
                      </div>
                    </div>
                  )}
                  <div className="flex gap-2 md:col-span-2 lg:col-span-3">
                    <button
                      onClick={() => setIsEditingAppointment(true)}
                      className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 flex items-center gap-2"
                      title="Modify appointment date, time, or status"
                    >
                      <Edit3 className="w-4 h-4" />
                      Edit Appointment
                    </button>
                  </div>
                </div>
              )}
            </div>
          ) : showCreateAppointment ? (
            <div className="space-y-4">
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    Appointment Date *
                  </label>
                  <input
                    type="date"
                    value={appointmentForm.appointmentdate}
                    onChange={(e) =>
                      setAppointmentForm({
                        ...appointmentForm,
                        appointmentdate: e.target.value,
                      })
                    }
                    className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    Start Time *
                  </label>
                  <input
                    type="text"
                    placeholder="e.g., 3:00 PM"
                    value={appointmentForm.starttime}
                    onChange={(e) =>
                      setAppointmentForm({
                        ...appointmentForm,
                        starttime: e.target.value,
                      })
                    }
                    className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    End Time *
                  </label>
                  <input
                    type="text"
                    placeholder="e.g., 4:00 PM"
                    value={appointmentForm.endtime}
                    onChange={(e) =>
                      setAppointmentForm({
                        ...appointmentForm,
                        endtime: e.target.value,
                      })
                    }
                    className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
                  />
                </div>
                <div className="md:col-span-2 lg:col-span-3">
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    Appointment Status
                  </label>
                  <SearchableSelect
                    options={appointmentStatusSelectOptions}
                    value={appointmentForm.appointmentstatuslookupid}
                    onChange={(value) =>
                      setAppointmentForm({
                        ...appointmentForm,
                        appointmentstatuslookupid: value,
                      })
                    }
                    placeholder="Select appointment status..."
                    className="w-full max-w-md"
                  />
                </div>
              </div>
              <div className="flex gap-2">
                <button
                  onClick={handleCreateAppointment}
                  className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700"
                >
                  Create Appointment
                </button>
                <button
                  onClick={() => {
                    setShowCreateAppointment(false);
                    setAppointmentForm({
                      appointmentdate: "",
                      starttime: "",
                      endtime: "",
                      appointmentstatuslookupid: null,
                    });
                  }}
                  className="px-4 py-2 bg-gray-500 text-white rounded-lg hover:bg-gray-600"
                >
                  Cancel
                </button>
              </div>
            </div>
          ) : !appointment && !showCreateAppointment ? (
            <div className="text-center py-12 bg-gradient-to-br from-blue-50 to-indigo-50 rounded-lg border-2 border-dashed border-blue-200">
              <div>
                <CalendarClock className="w-16 h-16 text-blue-300 mx-auto mb-4" />
              </div>
              <h3 className="text-lg font-semibold text-gray-900 mb-2">
                📅 No appointment scheduled
              </h3>
              <p className="text-gray-600 mb-4 max-w-md mx-auto">
                This online portal application is eligible for appointment
                scheduling to streamline the application process.
              </p>
              <div className="text-sm text-blue-700 bg-blue-100 px-4 py-2 rounded-lg inline-flex items-center gap-2">
                <span className="text-blue-500">💡</span>
                <span>Click "Schedule Appointment" above to get started</span>
              </div>
            </div>
          ) : null}
        </div>
      )}

      {/* Info message for office applications */}
      {!isOnlinePortalApplication && (
        <div className="bg-amber-50 rounded-lg border border-amber-200 p-6">
          <div className="flex items-center gap-3">
            <div className="flex-shrink-0">
              <CalendarClock className="w-6 h-6 text-amber-600" />
            </div>
            <div>
              <h3 className="text-lg font-semibold text-amber-800 mb-2">
                Appointments Not Available
              </h3>
              <p className="text-amber-700 text-sm">
                Appointments are only available for applications submitted
                through the online portal. This application was submitted at an
                office location
                {applicationData?.organisationname && (
                  <span className="font-medium">
                    {" "}
                    ({applicationData.organisationname})
                  </span>
                )}
                .
              </p>
              <div className="mt-3 flex flex-wrap gap-2 text-xs">
                <span className="text-amber-600 bg-amber-100 px-2 py-1 rounded">
                  Reference: {applicationData?.reference}
                </span>
                <span className="text-amber-600 bg-amber-100 px-2 py-1 rounded">
                  Office Walk-in Application
                </span>
                {applicationData?.enrolmentofficeid && (
                  <span className="text-amber-600 bg-amber-100 px-2 py-1 rounded">
                    Office ID: {applicationData.enrolmentofficeid}
                  </span>
                )}
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Read-only Application Information */}
      <div className="bg-white rounded-lg border border-gray-200 p-6">
        <h3 className="text-lg font-semibold text-gray-900 mb-6 flex items-center gap-2">
          <FileText className="w-5 h-5" />
          Application Information (Read-Only)
        </h3>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
          {/* Reference */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Reference Number
            </label>
            <div className="flex items-center gap-3">
              <div className="text-lg font-mono font-semibold text-gray-900">
                {applicationData.reference}
              </div>
              <button
                onClick={handleGenerateReference}
                disabled={generatingReference}
                className="flex items-center gap-1 px-3 py-1 text-xs font-medium bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors"
                title="Generate new reference number"
              >
                {generatingReference ? (
                  <Loader2 className="w-3 h-3 animate-spin" />
                ) : (
                  <RefreshCw className="w-3 h-3" />
                )}
                {generatingReference ? "Generating..." : "Generate"}
              </button>
            </div>
            <div className="text-xs text-gray-500 mt-1">Barcode ID</div>
          </div>

          {/* Entity ID */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Entity ID
            </label>
            <div className="text-base font-mono text-gray-900">
              {applicationData.entityid}
            </div>
            <div className="text-xs text-gray-500 mt-1">
              Links to Person: {applicationData.tgpersonid}
            </div>
          </div>

          {/* Application Start Date */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Application Start Date
            </label>
            {applicationData.applicationstartdate ? (
              <div className="text-base text-gray-900">
                {formatDate(applicationData.applicationstartdate)}
              </div>
            ) : (
              <div className="text-sm text-gray-500 italic bg-gray-50 px-3 py-2 rounded border-2 border-dashed border-gray-200 flex items-center gap-2">
                <Calendar className="w-4 h-4 opacity-50" />
                <span>No start date recorded</span>
              </div>
            )}
          </div>

          {/* Document Type */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Document Type
            </label>
            {applicationData.documenttypelookupid ? (
              <div className="space-y-1">
                <LookupField
                  lookupId={applicationData.documenttypelookupid}
                  format="value"
                  className="text-base font-medium text-gray-900"
                />
                <div className="text-xs text-gray-500">
                  ID: {applicationData.documenttypelookupid}
                </div>
              </div>
            ) : (
              <div className="text-sm text-gray-500 italic bg-gray-50 px-3 py-2 rounded border-2 border-dashed border-gray-200 flex items-center gap-2">
                <FileText className="w-4 h-4 opacity-50" />
                <span>No document type specified</span>
              </div>
            )}
          </div>
        </div>

        {/* Additional Information */}
        <div className="bg-white rounded-lg border border-gray-200 p-6 my-5">
          <h3 className="text-lg font-semibold text-gray-900 mb-4 flex items-center gap-2">
            <FileText className="w-5 h-5" />
            Additional Information
          </h3>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Remarks
            </label>
            {applicationData.remark ? (
              <div className="p-4 bg-gray-50 rounded-lg text-sm text-gray-900">
                {applicationData.remark}
              </div>
            ) : (
              <div className="text-center py-8 bg-gradient-to-br from-gray-50 to-gray-100 rounded-lg border-2 border-dashed border-gray-200">
                <div className="relative mb-4">
                  <FileText className="w-12 h-12 text-gray-300 mx-auto" />
                  <div className="absolute -bottom-1 -right-1 w-4 h-4 bg-gray-400 rounded-full flex items-center justify-center">
                    <span className="text-white text-xs">📝</span>
                  </div>
                </div>
                <p className="text-gray-500 text-sm font-medium">
                  No additional remarks recorded
                </p>
                <p className="text-xs text-gray-400 mt-1">
                  Application-specific notes and comments would appear here
                </p>
              </div>
            )}
          </div>
        </div>

        {/* System Information */}
        <div className="bg-white rounded-lg border border-gray-200 p-6">
          <h3 className="text-lg font-semibold text-gray-900 mb-4 flex items-center gap-2">
            <Flag className="w-5 h-5" />
            System Information
          </h3>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-2">
                Created Date
              </label>
              <div className="text-base text-gray-900">
                {formatDate(applicationData.createdate)}
              </div>
            </div>

            <div>
              <label className="block text-sm font-medium text-gray-700 mb-2">
                Last Modified
              </label>
              <div className="text-base text-gray-900">
                {formatDate(applicationData.modifieddate)}
              </div>
            </div>

            <div>
              <label className="block text-sm font-medium text-gray-700 mb-2">
                Active Status
              </label>
              <span
                className={cn(
                  "inline-flex items-center px-3 py-1 text-sm font-medium rounded-full",
                  applicationData.isactive
                    ? "bg-green-100 text-green-800"
                    : "bg-red-100 text-red-800",
                )}
              >
                {applicationData.isactive ? "✓ Active" : "✗ Inactive"}
              </span>
            </div>

            {/* Priority Level */}
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-2">
                Priority Level
              </label>
              {applicationData.prioritylookupid ? (
                <LookupField
                  lookupId={applicationData.prioritylookupid}
                  format="value"
                  className="text-base text-gray-900"
                />
              ) : (
                <div className="text-sm text-gray-500 italic bg-gray-50 px-3 py-2 rounded border-2 border-dashed border-gray-200 flex items-center gap-2">
                  <Flag className="w-4 h-4 opacity-50" />
                  <span>No priority assigned</span>
                  <span className="ml-auto text-xs bg-gray-200 text-gray-600 px-2 py-0.5 rounded">
                    Standard
                  </span>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Override Confirmation Modal */}
      {showOverrideConfirm && selectedOverrideStatus && (
        <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
          <div className="bg-white rounded-lg p-6 max-w-md w-full mx-4">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-lg font-semibold text-gray-900">
                Confirm Status Override
              </h3>
              <button
                onClick={() => setShowOverrideConfirm(false)}
                className="text-gray-400 hover:text-gray-600"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            <div className="space-y-4 mb-6">
              <div className="bg-orange-50 border border-orange-200 rounded-lg p-4">
                <div className="flex items-center gap-2 mb-2">
                  <AlertCircle className="w-5 h-5 text-orange-600" />
                  <span className="font-semibold text-orange-800">Warning</span>
                </div>
                <p className="text-sm text-orange-700">
                  This will directly set the workflow status without following
                  normal workflow progression. No workflow history entry will be
                  created - you would need to add it manually if required.
                </p>
              </div>

              <div className="space-y-2">
                <div className="text-sm">
                  <span className="font-medium">Current Status:</span>{" "}
                  {formData.currentapplicationstatusname}
                </div>
                <div className="text-sm">
                  <span className="font-medium">New Status:</span>{" "}
                  {selectedOverrideStatus.name}
                </div>
                <div className="text-sm">
                  <span className="font-medium">New Workflow Config ID:</span>{" "}
                  {selectedOverrideStatus.id}
                </div>
              </div>
            </div>

            <div className="flex gap-3 justify-end">
              <button
                onClick={() => setShowOverrideConfirm(false)}
                className="px-4 py-2 text-gray-700 bg-gray-100 rounded-lg hover:bg-gray-200"
              >
                Cancel
              </button>
              <button
                onClick={handleOverrideConfirm}
                className="px-4 py-2 bg-orange-600 text-white rounded-lg hover:bg-orange-700"
              >
                Confirm Override
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
