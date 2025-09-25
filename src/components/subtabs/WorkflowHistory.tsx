//
//  campus-pilot
//  WorkflowHistory.tsx
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { useState, useEffect } from "react";
import {
  History,
  ChevronRight,
  Plus,
  Calendar,
  User,
  MessageSquare,
  Loader2,
  AlertCircle,
  X,
} from "lucide-react";
import { formatDate } from "../../lib/utils";
import { cn } from "../../lib/utils";
import { apiClient, type WorkflowConfiguration } from "../../lib/api";
import toast from "react-hot-toast";
import { getAuthHeaders } from "../../lib/userSession";

// Helper function to get workflow status configuration ID by status name
const getStatusConfigId = async (
  statusName: string,
): Promise<number | null> => {
  try {
    const authHeaders = await getAuthHeaders();
    const response = await fetch(
      `/api/workflow-status-config?statusName=${encodeURIComponent(statusName)}`,
      { headers: authHeaders },
    );
    if (response.ok) {
      const data = await response.json();
      return data.tgworkflowstatusconfigurationid;
    }
  } catch (error) {
    console.error("Failed to get status config ID:", error);
  }
  return null;
};

// Helper function to get next workflow status name based on current status and action
const getNextWorkflowStatus = async (
  currentStatus: string,
  actionName: string,
): Promise<string | null> => {
  try {
    const authHeaders = await getAuthHeaders();
    const response = await fetch(
      `/api/workflow-config-lookup?currentStatus=${encodeURIComponent(currentStatus)}&action=${encodeURIComponent(actionName)}`,
      { headers: authHeaders },
    );
    if (response.ok) {
      const data = await response.json();
      return data.nextworkflowstatusname;
    }
  } catch (error) {
    console.error("Failed to get next workflow status:", error);
  }
  return null;
};

interface WorkflowHistoryProps {
  applicationId: number;
}

interface WorkflowHistoryRecord {
  tgapplicationworkflowhistoryid: number;
  tgapplicationworkflowhistoryparentid?: number;
  tgapplicationid: number;
  applicationtypename: string;
  actiondate: Date;
  currentapplicationstatusname: string;
  actionname: string;
  actionbyentitytype: string;
  actionbyentityid?: number;
  applicationstatuslookupvalue: string;
  remark?: string;
  tgactiondisplayname?: string;
  portalrecordstatuslookupid?: number;
  recordstatuslookupid?: number;
  createdate: Date;
  modifieddate: Date;
  tguserauditdetailid: number;
  createdbysystemuserid: number;
  updatedbysystemuserid?: number;
  dataownerlookupid: number;
  isactive: number;
  nextworkflowstatusname?: string; // Added for display purposes
}

interface WorkflowAction extends WorkflowConfiguration {
  // Inherits all properties from WorkflowConfiguration
}

interface NewWorkflowForm {
  currentapplicationstatusname: string;
  selectedAction: WorkflowAction | null;
}

export function WorkflowHistory({ applicationId }: WorkflowHistoryProps) {
  const [history, setHistory] = useState<WorkflowHistoryRecord[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [availableActions, setAvailableActions] = useState<WorkflowAction[]>(
    [],
  );
  const [showActionPanel, setShowActionPanel] = useState(false);
  const [isSubmittingEntry, setIsSubmittingEntry] = useState(false);
  const [isLoadingActions, setIsLoadingActions] = useState(false);
  const [currentApplicationStatus, setCurrentApplicationStatus] =
    useState<string>("");
  const [currentStatusConfigId, setCurrentStatusConfigId] = useState<
    number | null
  >(null);
  const [targetStatusConfigId, setTargetStatusConfigId] = useState<
    number | null
  >(null);
  const [newWorkflowForm, setNewWorkflowForm] = useState<NewWorkflowForm>({
    currentapplicationstatusname: "",
    selectedAction: null,
  });

  // Rollback modal state
  const [showRollbackModal, setShowRollbackModal] = useState(false);
  const [rollbackStatus, setRollbackStatus] = useState<string | null>(null);

  // Fetch workflow history data and current application status
  useEffect(() => {
    const fetchWorkflowHistory = async () => {
      try {
        setIsLoading(true);
        setError(null);

        // Fetch both workflow history and current application details
        const [historyData, caseDetails] = await Promise.all([
          apiClient.getWorkflowHistory(applicationId),
          apiClient.getCaseDetails(applicationId),
        ]);

        // Convert date strings to Date objects and lookup next workflow status
        const processedData = await Promise.all(
          historyData.map(async (entry: any) => {
            const nextWorkflowStatus = await getNextWorkflowStatus(
              entry.currentapplicationstatusname,
              entry.actionname,
            );
            return {
              ...entry,
              actiondate: new Date(entry.actiondate),
              createdate: new Date(entry.createdate),
              modifieddate: new Date(entry.modifieddate),
              nextworkflowstatusname: nextWorkflowStatus,
            };
          }),
        );
        setHistory(processedData);

        // Set current application status
        const currentStatus = caseDetails.currentapplicationstatusname;
        setCurrentApplicationStatus(currentStatus);
        setNewWorkflowForm((prev) => ({
          ...prev,
          currentapplicationstatusname: currentStatus,
        }));

        // Fetch available actions for the current status
        fetchAvailableActions(currentStatus);
      } catch (err) {
        const errorMessage =
          err instanceof Error
            ? err.message
            : "Failed to load workflow history";
        setError(errorMessage);
        toast.error(errorMessage);
      } finally {
        setIsLoading(false);
      }
    };

    fetchWorkflowHistory();
  }, [applicationId]);

  // Fetch available actions based on current status
  const fetchAvailableActions = async (currentStatus: string) => {
    if (!currentStatus) return;

    try {
      setIsLoadingActions(true);
      const configurations =
        await apiClient.getWorkflowConfigurations(currentStatus);
      setAvailableActions(configurations);
    } catch (error) {
      console.error("Failed to fetch workflow configurations:", error);
      toast.error("Failed to load available actions");
      setAvailableActions([]);
    } finally {
      setIsLoadingActions(false);
    }
  };

  // Fetch status configuration IDs when workflow action is selected
  useEffect(() => {
    const fetchStatusConfigIds = async () => {
      if (newWorkflowForm.selectedAction) {
        // Fetch current status config ID
        const currentConfigId = await getStatusConfigId(
          newWorkflowForm.selectedAction.currentapplicationstatusname,
        );
        setCurrentStatusConfigId(currentConfigId);

        // Fetch target status config ID
        const targetConfigId = await getStatusConfigId(
          newWorkflowForm.selectedAction.nextworkflowstatusname,
        );
        setTargetStatusConfigId(targetConfigId);
      } else {
        setCurrentStatusConfigId(null);
        setTargetStatusConfigId(null);
      }
    };

    fetchStatusConfigIds();
  }, [newWorkflowForm.selectedAction]);

  // Handle status change and fetch corresponding actions
  const handleStatusChange = (status: string) => {
    setNewWorkflowForm((prev) => ({
      ...prev,
      currentapplicationstatusname: status,
      selectedAction: null,
    }));
    fetchAvailableActions(status);
  };

  const handleVoidEntry = async (id: number) => {
    const confirmed = window.confirm(
      "Are you sure you want to void this workflow history entry?",
    );
    if (!confirmed) return;

    try {
      const response = await apiClient.voidWorkflowHistoryEntry(id);
      setHistory((prev) =>
        prev.map((entry) =>
          entry.tgapplicationworkflowhistoryid === id
            ? { ...entry, isactive: 0, modifieddate: new Date() }
            : entry,
        ),
      );
      toast.success("Workflow history entry voided successfully");

      if (response.nextstatus) {
        setShowRollbackModal(true);
        setRollbackStatus(response.nextstatus);
      }
    } catch (error) {
      toast.error("Failed to void workflow history entry");
    }
  };

  const handleRollbackConfirm = async () => {
    if (!rollbackStatus) return;

    try {
      await apiClient.overrideApplicationStatusByName(
        applicationId,
        rollbackStatus,
      );
      setCurrentApplicationStatus(rollbackStatus);
      setShowRollbackModal(false);
      setRollbackStatus(null);

      setNewWorkflowForm({
        currentapplicationstatusname: rollbackStatus,
        selectedAction: null,
      });

      // Fetch new available actions for the updated status
      fetchAvailableActions(rollbackStatus);

      toast.success(`Application status rolled back to ${rollbackStatus}`);
    } catch (error) {
      toast.error("Failed to rollback application status");
    }
  };

  const handleAddWorkflowEntry = async () => {
    if (!newWorkflowForm.currentapplicationstatusname.trim()) {
      toast.error("Please select a current application status");
      return;
    }

    if (!newWorkflowForm.selectedAction) {
      toast.error("Please select an action");
      return;
    }

    // Validate that the current status matches what's available
    if (
      newWorkflowForm.currentapplicationstatusname !== currentApplicationStatus
    ) {
      const confirmProceed = window.confirm(
        `You're creating a workflow entry from "${newWorkflowForm.currentapplicationstatusname}" but the current application status is "${currentApplicationStatus}". This may create an invalid workflow transition. Do you want to proceed?`,
      );
      if (!confirmProceed) {
        return;
      }
    }

    setIsSubmittingEntry(true);
    try {
      const workflowData = {
        applicationId: applicationId,
        currentapplicationstatusname:
          newWorkflowForm.currentapplicationstatusname,
        actionname: newWorkflowForm.selectedAction.actionname,
        nextworkflowstatusname:
          newWorkflowForm.selectedAction.nextworkflowstatusname,
        tgworkflowstatusconfigurationid:
          newWorkflowForm.selectedAction.tgworkflowstatusconfigurationid,
      };

      await apiClient.addWorkflowEntry(workflowData);

      // Refresh the workflow history
      const historyData = await apiClient.getWorkflowHistory(applicationId);
      const processedData = await Promise.all(
        historyData.map(async (entry: any) => {
          const nextWorkflowStatus = await getNextWorkflowStatus(
            entry.currentapplicationstatusname,
            entry.actionname,
          );
          return {
            ...entry,
            actiondate: new Date(entry.actiondate),
            createdate: new Date(entry.createdate),
            modifieddate: new Date(entry.modifieddate),
            nextworkflowstatusname: nextWorkflowStatus,
          };
        }),
      );
      setHistory(processedData);

      // Update current application status
      const newStatus = newWorkflowForm.selectedAction.nextworkflowstatusname;
      setCurrentApplicationStatus(newStatus);

      // Reset form
      setNewWorkflowForm({
        currentapplicationstatusname: newStatus,
        selectedAction: null,
      });

      // Fetch new available actions for the updated status
      fetchAvailableActions(newStatus);
      setShowActionPanel(false);
      toast.success(
        "Workflow entry added and application status updated successfully",
      );
    } catch (error) {
      console.error("Failed to add workflow entry:", error);
      const errorMessage =
        error instanceof Error ? error.message : "Failed to add workflow entry";
      toast.error(`Error: ${errorMessage}`);
    } finally {
      setIsSubmittingEntry(false);
    }
  };

  const MetaStrip = ({ record }: { record: WorkflowHistoryRecord }) => (
    <div className="flex items-center gap-3 text-[10px] text-gray-500">
      <span className="font-medium">
        ID: {record.tgapplicationworkflowhistoryid}
      </span>
      <span className="text-gray-400">•</span>
      <span className="font-medium">{formatDate(record.createdate)}</span>
      <span className="text-gray-400">•</span>
      <span className="font-medium">{formatDate(record.modifieddate)}</span>
      <span className="text-gray-400">•</span>
      <span
        className={cn(
          "px-1.5 py-0.5 rounded text-[9px] font-medium",
          record.isactive
            ? "bg-green-100 text-green-700"
            : "bg-gray-100 text-gray-600",
        )}
      >
        {record.isactive ? "Active" : "Inactive"}
      </span>
    </div>
  );

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="flex items-center gap-3">
          <Loader2 className="w-6 h-6 animate-spin text-blue-600" />
          <span className="text-gray-600">Loading workflow history...</span>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-center">
          <AlertCircle className="w-12 h-12 text-red-500 mx-auto mb-4" />
          <h3 className="text-lg font-semibold text-gray-900 mb-2">
            Failed to Load Workflow History
          </h3>
          <p className="text-gray-600 mb-4">{error}</p>
          <button
            onClick={() => window.location.reload()}
            className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700"
          >
            Retry
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold flex items-center gap-2">
          <History className="w-4 h-4" />
          Workflow History ({history.length} entries)
        </h2>
        <button
          onClick={() => setShowActionPanel(!showActionPanel)}
          className="compact-button bg-primary text-white flex items-center gap-1"
        >
          <Plus className="w-3 h-3" />
          Add Entry
        </button>
      </div>

      {/* Action Panel */}
      {showActionPanel && (
        <div className="bg-card border rounded-lg p-4 space-y-3">
          <h3 className="text-xs font-semibold">Add Workflow Entry</h3>

          <div>
            <label className="block text-xs font-medium text-gray-700 mb-1">
              Current Application Status{" "}
              <span className="text-destructive">*</span>
            </label>
            <div className="flex gap-2">
              <input
                type="text"
                value={newWorkflowForm.currentapplicationstatusname}
                onChange={(e) => handleStatusChange(e.target.value)}
                className="flex-1 p-2 text-xs border rounded"
                placeholder="Enter current status (e.g., LEGACY_DATA_VALIDATION)..."
              />
              <button
                type="button"
                onClick={() => handleStatusChange(currentApplicationStatus)}
                className="compact-button border bg-gray-50 hover:bg-gray-100 text-xs px-3"
                title="Use current application status"
              >
                Use Current
              </button>
            </div>
            <p className="text-xs text-gray-500 mt-1">
              Current status from application:{" "}
              <strong>{currentApplicationStatus}</strong>
            </p>
          </div>

          <div>
            <label className="block text-xs font-medium text-gray-700 mb-1">
              Available Actions <span className="text-destructive">*</span>
            </label>
            {isLoadingActions ? (
              <div className="flex items-center gap-2 p-2 text-xs text-gray-500 bg-gray-50 rounded">
                <Loader2 className="w-3 h-3 animate-spin" />
                Loading available actions...
              </div>
            ) : availableActions.length > 0 ? (
              <div>
                <p className="text-xs mb-2">
                  Found {availableActions.length} actions
                </p>
                <select
                  value={
                    newWorkflowForm.selectedAction?.tgworkflowconfigurationid ||
                    ""
                  }
                  onChange={(e) => {
                    console.log("Select onChange triggered:", e.target.value);
                    console.log("Available actions:", availableActions);
                    const configId = e.target.value;
                    if (!configId) {
                      setNewWorkflowForm((prev) => ({
                        ...prev,
                        selectedAction: null,
                      }));
                      return;
                    }
                    const selectedAction = availableActions.find(
                      (action) =>
                        String(action.tgworkflowconfigurationid) === configId,
                    );
                    console.log("Found action:", selectedAction);
                    setNewWorkflowForm((prev) => ({
                      ...prev,
                      selectedAction: selectedAction || null,
                    }));
                  }}
                  onClick={() =>
                    console.log("Select clicked", availableActions)
                  }
                  className="w-full p-2 text-xs border rounded cursor-pointer bg-white"
                  style={{ minHeight: "32px" }}
                >
                  <option value="">Select an action...</option>
                  {availableActions.map((action, index) => (
                    <option
                      key={`${action.tgworkflowconfigurationid}-${index}`}
                      value={String(action.tgworkflowconfigurationid)}
                    >
                      {action.actionname} → {action.nextworkflowstatusname}
                    </option>
                  ))}
                </select>
              </div>
            ) : newWorkflowForm.currentapplicationstatusname ? (
              <p className="text-xs text-orange-600 p-2 bg-orange-50 rounded border border-orange-200">
                ⚠️ No available actions for status:{" "}
                {newWorkflowForm.currentapplicationstatusname}
              </p>
            ) : (
              <p className="text-xs text-gray-500 p-2 bg-gray-50 rounded">
                Enter a current status to see available actions
              </p>
            )}
          </div>

          {newWorkflowForm.selectedAction && (
            <div className="bg-blue-50 p-3 rounded text-xs border border-blue-200">
              <div className="flex items-center gap-2 mb-2">
                <div className="w-2 h-2 bg-blue-500 rounded-full"></div>
                <strong>Workflow Transition Preview</strong>
              </div>
              <div className="space-y-1">
                <p>
                  <strong>Action:</strong>{" "}
                  {newWorkflowForm.selectedAction.actionname}
                </p>
                <p>
                  <strong>From:</strong>{" "}
                  {newWorkflowForm.currentapplicationstatusname}
                </p>
                <p>
                  <strong>To:</strong>{" "}
                  <span className="bg-blue-100 px-2 py-0.5 rounded">
                    {newWorkflowForm.selectedAction.nextworkflowstatusname}
                  </span>
                </p>
                <p>
                  <strong>Description:</strong>{" "}
                  {newWorkflowForm.selectedAction.nextworkflowstatusdescription}
                </p>
                <p className="text-gray-500">
                  <strong>Current Status ID:</strong>{" "}
                  {currentStatusConfigId !== null
                    ? currentStatusConfigId
                    : "Loading..."}{" "}
                  → <strong>Target Status ID:</strong>{" "}
                  {targetStatusConfigId !== null
                    ? targetStatusConfigId
                    : "Loading..."}
                </p>
              </div>
            </div>
          )}

          <div className="flex justify-end gap-2">
            <button
              onClick={() => {
                setShowActionPanel(false);
                setNewWorkflowForm({
                  currentapplicationstatusname: currentApplicationStatus,
                  selectedAction: null,
                });
                setAvailableActions([]);
              }}
              className="compact-button border"
              disabled={isSubmittingEntry}
            >
              Cancel
            </button>
            <button
              onClick={handleAddWorkflowEntry}
              className="compact-button bg-primary text-white"
              disabled={isSubmittingEntry || !newWorkflowForm.selectedAction}
            >
              {isSubmittingEntry ? (
                <>
                  <Loader2 className="w-3 h-3 mr-1 animate-spin" />
                  Adding...
                </>
              ) : (
                "Add Entry"
              )}
            </button>
          </div>
        </div>
      )}

      {/* History Timeline */}
      <div className="space-y-4">
        {history.map((entry, index) => (
          <div key={entry.tgapplicationworkflowhistoryid} className="relative">
            {/* Timeline connector */}
            {index < history.length - 1 && (
              <div className="absolute left-4 top-10 bottom-0 w-0.5 bg-gray-300" />
            )}

            {/* Timeline entry */}
            <div className="flex gap-4 pb-4">
              {/* Timeline dot */}
              <div className="relative">
                <div
                  className={cn(
                    "w-8 h-8 rounded-full flex items-center justify-center shadow-sm",
                    index === 0 ? "bg-blue-500" : "bg-gray-400",
                  )}
                >
                  <div
                    className={cn(
                      "w-3 h-3 rounded-full",
                      index === 0 ? "bg-white" : "bg-white",
                    )}
                  />
                </div>
              </div>

              {/* Content */}
              <div
                className={cn(
                  "flex-1 border rounded-lg p-4 space-y-3 shadow-sm",
                  entry.isactive === 1
                    ? "bg-white border-gray-200"
                    : "bg-gray-50 border-gray-300 opacity-75",
                )}
              >
                <div className="flex items-start justify-between">
                  <div className="space-y-1">
                    <div className="flex items-center gap-2">
                      <span
                        className={cn(
                          "text-sm font-semibold",
                          entry.isactive === 1
                            ? "text-gray-900"
                            : "text-gray-600",
                        )}
                      >
                        {entry.actionname}
                      </span>
                      <ChevronRight className="w-3 h-3 text-gray-500" />
                      <span
                        className={cn(
                          "px-2 py-1 rounded text-xs font-medium",
                          entry.isactive === 1
                            ? "bg-blue-100 text-blue-800"
                            : "bg-gray-100 text-gray-600",
                        )}
                      >
                        {entry.currentapplicationstatusname}
                        {entry.nextworkflowstatusname && (
                          <> to {entry.nextworkflowstatusname}</>
                        )}
                      </span>
                      {entry.isactive === 0 && (
                        <span className="bg-red-100 text-red-700 px-2 py-1 rounded text-xs font-medium">
                          VOIDED
                        </span>
                      )}
                    </div>
                    <div className="flex items-center gap-4 text-xs text-gray-600">
                      <div className="flex items-center gap-1">
                        <Calendar className="w-3 h-3" />
                        <span className="font-medium">
                          {formatDate(entry.actiondate)}
                        </span>
                      </div>
                      <div className="flex items-center gap-1">
                        <User className="w-3 h-3" />
                        <span className="font-medium">
                          User ID: {entry.createdbysystemuserid}
                        </span>
                      </div>
                    </div>
                  </div>
                </div>

                {entry.remark && (
                  <div className="pt-2 border-t">
                    <div className="flex items-start gap-1">
                      <MessageSquare className="w-3 h-3 text-gray-500 mt-0.5" />
                      <p className="text-xs text-gray-700 font-medium">
                        {entry.remark}
                      </p>
                    </div>
                  </div>
                )}

                <div className="pt-2 border-t border-gray-100">
                  <div className="flex items-center gap-3 text-xs text-gray-600">
                    <span className="font-medium">
                      Type: {entry.applicationtypename}
                    </span>
                    <span className="text-gray-400">•</span>
                    <span className="font-medium">
                      Entity: {entry.actionbyentitytype}
                    </span>
                    {entry.actionbyentityid && (
                      <>
                        <span className="text-gray-400">•</span>
                        <span className="font-medium">
                          Entity ID: {entry.actionbyentityid}
                        </span>
                      </>
                    )}
                    {entry.applicationstatuslookupvalue && (
                      <>
                        <span className="text-gray-400">•</span>
                        <span className="font-medium">
                          Status: {entry.applicationstatuslookupvalue}
                        </span>
                      </>
                    )}
                  </div>
                </div>

                <div className="pt-1">
                  <MetaStrip record={entry} />
                </div>

                {/* Action Buttons */}
                {entry.isactive === 1 && (
                  <div className="flex items-center gap-2 pt-3 border-t border-gray-100">
                    <button
                      className="compact-button border border-red-500 text-red-600 hover:bg-red-50 hover:text-red-700 flex items-center gap-1"
                      onClick={() =>
                        handleVoidEntry(entry.tgapplicationworkflowhistoryid)
                      }
                    >
                      <AlertCircle className="w-3 h-3" />
                      Void Entry
                    </button>
                  </div>
                )}
              </div>
            </div>
          </div>
        ))}
      </div>

      {/* Empty State */}
      {history.length === 0 && (
        <div className="text-center py-12 bg-gradient-to-br from-blue-50 to-indigo-50 rounded-lg border-2 border-dashed border-blue-200">
          <History className="w-16 h-16 text-blue-300 mx-auto mb-4" />
          <h3 className="text-lg font-semibold text-gray-900 mb-2">
            📊 No Workflow History Found
          </h3>
          <p className="text-gray-600 mb-4 max-w-md mx-auto">
            No workflow activity has been recorded for this application yet.
            Status changes, approvals, and system actions will appear here as
            they occur.
          </p>
          <div className="text-sm text-blue-700 bg-blue-100 px-4 py-2 rounded-lg inline-flex items-center gap-2">
            <span className="text-blue-500">💡</span>
            <span>
              Application status changes and workflow actions will appear here
              automatically
            </span>
          </div>
        </div>
      )}

      {/* Pagination */}
      {history.length > 10 && (
        <div className="flex items-center justify-center gap-2 pt-4">
          <button className="compact-button border" disabled>
            Previous
          </button>
          <span className="text-xs text-gray-600 font-medium">Page 1 of 1</span>
          <button className="compact-button border" disabled>
            Next
          </button>
        </div>
      )}

      {/* Rollback Status Confirmation Modal */}
      {showRollbackModal && rollbackStatus && (
        <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
          <div className="bg-white rounded-lg p-6 max-w-md w-full mx-4">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-lg font-semibold text-gray-900">
                Rollback Application Status
              </h3>
              <button
                onClick={() => setShowRollbackModal(false)}
                className="text-gray-400 hover:text-gray-600"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            <div className="space-y-4 mb-6">
              <div className="bg-blue-50 border border-blue-200 rounded-lg p-4">
                <div className="flex items-center gap-2 mb-2">
                  <AlertCircle className="w-5 h-5 text-blue-600" />
                  <span className="font-semibold text-blue-800">
                    Rollback Option Available
                  </span>
                </div>
                <p className="text-sm text-blue-700">
                  The voided workflow entry suggests rolling back the
                  application status to maintain consistency.
                </p>
              </div>

              <div className="space-y-2">
                <div className="text-sm">
                  <span className="font-medium">Current Status:</span>{" "}
                  {currentApplicationStatus}
                </div>
                <div className="text-sm">
                  <span className="font-medium">
                    Suggested Rollback Status:
                  </span>{" "}
                  {rollbackStatus}
                </div>
              </div>
            </div>

            <div className="flex gap-3 justify-end">
              <button
                onClick={() => setShowRollbackModal(false)}
                className="px-4 py-2 text-gray-700 bg-gray-100 rounded-lg hover:bg-gray-200"
              >
                Keep Current Status
              </button>
              <button
                onClick={handleRollbackConfirm}
                className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700"
              >
                Rollback Status
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
