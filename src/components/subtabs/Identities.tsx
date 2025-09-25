//
//  campus-pilot
//  Identities.tsx
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { useState, useEffect } from "react";
import {
  CreditCard,
  Plus,
  Edit2,
  Calendar,
  AlertCircle,
  Loader2,
  Eye,
  CheckCircle,
} from "lucide-react";
import { formatDate, formatDateCompact } from "../../lib/utils";
import { cn } from "../../lib/utils";
import { apiClient } from "../../lib/api";
import { LookupField } from "../LookupField";
import { SearchableSelect } from "../SearchableSelect";
import toast from "react-hot-toast";

interface IdentitiesProps {
  personId: number;
}

interface IdentityRecord {
  tgpersonidentityid: number;
  tgpersonid: number;
  identitytypelookupid: number;
  identitynumber: string;
  issuedate: Date;
  expirydate?: Date;
  countryofissuelookupid?: number;
  identitydocumentstatuslookupid?: number;
  createdate: Date;
  modifieddate: Date;
  isactive: boolean;
}

interface NewIdentityForm {
  identitytypelookupid: number | null;
  identitynumber: string;
  issuedate: string;
  expirydate: string;
  countryofissuelookupid: number | null;
  identitydocumentstatuslookupid: number | null;
}

export function Identities({ personId }: IdentitiesProps) {
  const [identities, setIdentities] = useState<IdentityRecord[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedIdentity, setSelectedIdentity] = useState<number | null>(null);
  const [isAddingNew, setIsAddingNew] = useState(false);
  const [validIdentityIds, setValidIdentityIds] = useState<Set<number>>(
    new Set(),
  );
  const [editingIdentity, setEditingIdentity] = useState<number | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [identityTypeOptions, setIdentityTypeOptions] = useState<any[]>([]);
  const [countryOptions, setCountryOptions] = useState<any[]>([]);
  const [statusOptions, setStatusOptions] = useState<any[]>([]);
  const [loadingOptions, setLoadingOptions] = useState(true);

  const [newIdentityForm, setNewIdentityForm] = useState<NewIdentityForm>({
    identitytypelookupid: null,
    identitynumber: "",
    issuedate: "",
    expirydate: "",
    countryofissuelookupid: null,
    identitydocumentstatuslookupid: null,
  });

  const [editForm, setEditForm] = useState<{
    identitynumber: string;
    issuedate: string;
    expirydate: string;
    countryofissuelookupid: number | null;
    identitydocumentstatuslookupid: number | null;
  }>({
    identitynumber: "",
    issuedate: "",
    expirydate: "",
    countryofissuelookupid: null,
    identitydocumentstatuslookupid: null,
  });

  // Fetch identity data
  useEffect(() => {
    const fetchIdentities = async () => {
      try {
        setIsLoading(true);
        setError(null);

        const data = await apiClient.getIdentities(personId);
        // Convert date strings to Date objects
        const processedData = data.map((identity: any) => ({
          ...identity,
          issuedate: new Date(identity.issuedate),
          expirydate: identity.expirydate
            ? new Date(identity.expirydate)
            : undefined,
          createdate: new Date(identity.createdate),
          modifieddate: new Date(identity.modifieddate),
        }));
        setIdentities(processedData);

        // Check which identities are valid
        const validIds = new Set<number>();
        for (const identity of processedData) {
          if (identity.identitydocumentstatuslookupid) {
            try {
              const lookup = await apiClient.getLookupValue(
                identity.identitydocumentstatuslookupid,
              );
              if (lookup?.lookupvalue === "IDENTITY_DOCUMENT_STATUS_VALID") {
                validIds.add(identity.tgpersonidentityid);
              }
            } catch (error) {
              // Ignore lookup errors
            }
          }
        }
        setValidIdentityIds(validIds);
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to load identity data";
        setError(errorMessage);
        toast.error(errorMessage);
      } finally {
        setIsLoading(false);
      }
    };

    fetchIdentities();
  }, [personId]);

  // Fetch lookup options
  useEffect(() => {
    const fetchLookupOptions = async () => {
      try {
        setLoadingOptions(true);
        const [identityTypeData, countryData, statusData] = await Promise.all([
          apiClient.getLookupsByType("IDENTITY_TYPE"),
          apiClient.getLookupsByType("COUNTRY"),
          apiClient.getLookupsByType("IDENTITY_DOCUMENT_STATUS"),
        ]);

        setIdentityTypeOptions(
          identityTypeData.map((lookup) => ({
            id: lookup.tglookupid,
            value: lookup.lookupvalue,
            label: lookup.lookupdescription || lookup.lookupvalue,
          })),
        );
        setCountryOptions(
          countryData.map((lookup) => ({
            id: lookup.tglookupid,
            value: lookup.lookupvalue,
            label: lookup.lookupdescription || lookup.lookupvalue,
          })),
        );
        setStatusOptions(
          statusData.map((lookup) => ({
            id: lookup.tglookupid,
            value: lookup.lookupvalue,
            label: lookup.lookupdescription || lookup.lookupvalue,
          })),
        );
      } catch (err) {
        console.error("Failed to load lookup options:", err);
      } finally {
        setLoadingOptions(false);
      }
    };

    fetchLookupOptions();
  }, []);

  const isExpired = (expiryDate?: Date) => {
    if (!expiryDate) return false;
    return new Date() > expiryDate;
  };

  const isExpiringSoon = (expiryDate?: Date) => {
    if (!expiryDate) return false;
    const thirtyDaysFromNow = new Date();
    thirtyDaysFromNow.setDate(thirtyDaysFromNow.getDate() + 30);
    return new Date() < expiryDate && expiryDate <= thirtyDaysFromNow;
  };

  const handleAddNew = async () => {
    if (
      !newIdentityForm.identitytypelookupid ||
      !newIdentityForm.identitynumber ||
      !newIdentityForm.issuedate
    ) {
      toast.error("Please fill in all required fields");
      return;
    }

    setIsSubmitting(true);
    try {
      const createData = {
        tgpersonid: personId,
        identitytypelookupid: newIdentityForm.identitytypelookupid,
        identitynumber: newIdentityForm.identitynumber,
        issuedate: newIdentityForm.issuedate || null,
        expirydate: newIdentityForm.expirydate || null,
        countryofissuelookupid: newIdentityForm.countryofissuelookupid || null,
        identitydocumentstatuslookupid:
          newIdentityForm.identitydocumentstatuslookupid || null,
      };

      await apiClient.createIdentity(createData);

      // Refresh identities to get the newly created identity with proper data
      const data = await apiClient.getIdentities(personId);
      const processedData = data.map((identity: any) => ({
        ...identity,
        createdate: new Date(identity.createdate),
        modifieddate: identity.modifieddate
          ? new Date(identity.modifieddate)
          : undefined,
        issuedate: identity.issuedate
          ? new Date(identity.issuedate)
          : undefined,
        expirydate: identity.expirydate
          ? new Date(identity.expirydate)
          : undefined,
        isactive: Boolean(identity.isactive),
      }));
      setIdentities(processedData);
      setIsAddingNew(false);
      setNewIdentityForm({
        identitytypelookupid: null,
        identitynumber: "",
        issuedate: "",
        expirydate: "",
        countryofissuelookupid: null,
        identitydocumentstatuslookupid: null,
      });

      toast.success("Identity document added successfully");
    } catch (error) {
      toast.error("Failed to add identity document");
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleVoid = async (id: number) => {
    const confirmed = window.confirm(
      "Are you sure you want to void this identity document?",
    );
    if (!confirmed) return;

    try {
      await apiClient.voidIdentity(id);
      setIdentities((prev) =>
        prev.map((identity) =>
          identity.tgpersonidentityid === id
            ? { ...identity, isactive: false, modifieddate: new Date() }
            : identity,
        ),
      );
      toast.success("Identity document voided successfully");
    } catch (error) {
      toast.error("Failed to void identity document");
    }
  };

  const handleStartEdit = (identity: IdentityRecord) => {
    setEditForm({
      identitynumber: identity.identitynumber || "",
      issuedate: identity.issuedate
        ? identity.issuedate instanceof Date
          ? identity.issuedate.toISOString().split("T")[0]
          : new Date(identity.issuedate).toISOString().split("T")[0]
        : "",
      expirydate: identity.expirydate
        ? identity.expirydate instanceof Date
          ? identity.expirydate.toISOString().split("T")[0]
          : new Date(identity.expirydate).toISOString().split("T")[0]
        : "",
      countryofissuelookupid: identity.countryofissuelookupid || null,
      identitydocumentstatuslookupid:
        identity.identitydocumentstatuslookupid || null,
    });
    setEditingIdentity(identity.tgpersonidentityid);
  };

  const handleEditIdentity = async () => {
    if (!editingIdentity) return;

    try {
      setIsSubmitting(true);

      const updateData = {
        identitynumber: editForm.identitynumber,
        issuedate: editForm.issuedate,
        expirydate: editForm.expirydate,
        countryofissuelookupid: editForm.countryofissuelookupid,
        identitydocumentstatuslookupid: editForm.identitydocumentstatuslookupid,
      };

      await apiClient.updateIdentity(editingIdentity, updateData);

      // Update in current identities
      setIdentities((prev) =>
        prev.map((identity) =>
          identity.tgpersonidentityid === editingIdentity
            ? ({
                ...identity,
                ...updateData,
                issuedate: updateData.issuedate
                  ? new Date(updateData.issuedate)
                  : identity.issuedate,
                expirydate: updateData.expirydate
                  ? new Date(updateData.expirydate)
                  : identity.expirydate,
                modifieddate: new Date(),
              } as IdentityRecord)
            : identity,
        ),
      );

      toast.success("Identity document updated successfully");
      setEditingIdentity(null);
    } catch (error) {
      toast.error("Failed to update identity document");
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleValidate = async (id: number) => {
    const confirmed = window.confirm(
      "Are you sure you want to validate this identity document?",
    );
    if (!confirmed) return;

    try {
      const response = await fetch(`/api/identities/${id}/validate`, {
        method: "PATCH",
        headers: {
          "Content-Type": "application/json",
        },
      });

      if (!response.ok) {
        const error = await response.json();
        throw new Error(
          error.message || "Failed to validate identity document",
        );
      }

      const result = await response.json();

      // Update local state with the new status
      setIdentities((prev) =>
        prev.map((identity) =>
          identity.tgpersonidentityid === id
            ? {
                ...identity,
                identitydocumentstatuslookupid: result.statusLookupId,
                modifieddate: new Date(),
              }
            : identity,
        ),
      );

      // Add to valid identities set
      setValidIdentityIds((prev) => new Set([...prev, id]));

      toast.success("Identity document validated successfully");
    } catch (error) {
      const errorMessage =
        error instanceof Error
          ? error.message
          : "Failed to validate identity document";
      toast.error(errorMessage);
    }
  };

  const MetaStrip = ({ record }: { record: IdentityRecord }) => (
    <div className="flex items-center gap-3 text-[10px] text-muted-foreground">
      <span>ID: {record.tgpersonidentityid}</span>
      <span>•</span>
      <span>{formatDate(record.createdate)}</span>
      <span>•</span>
      <span>{formatDate(record.modifieddate)}</span>
      <span>•</span>
      <span
        className={cn(
          "badge text-[9px]",
          record.isactive ? "badge-success" : "badge-neutral",
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
          <span className="text-gray-600">Loading identity data...</span>
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
            Failed to Load Identities
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
          <CreditCard className="w-4 h-4" />
          Identity Documents ({identities.length})
        </h2>
        <button
          onClick={() => setIsAddingNew(true)}
          className="compact-button bg-primary text-white flex items-center gap-1"
        >
          <Plus className="w-3 h-3" />
          Add Identity
        </button>
      </div>

      {/* Add New Form */}
      {isAddingNew && (
        <div className="bg-white border rounded-lg p-4 space-y-4">
          <h3 className="text-sm font-semibold">Add New Identity Document</h3>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-medium text-muted-foreground mb-1">
                Identity Type <span className="text-destructive">*</span>
              </label>
              <SearchableSelect
                options={identityTypeOptions}
                value={newIdentityForm.identitytypelookupid}
                onChange={(value) =>
                  setNewIdentityForm((prev) => ({
                    ...prev,
                    identitytypelookupid: value,
                  }))
                }
                placeholder="Select identity type..."
                loading={loadingOptions}
                className="w-full"
              />
            </div>

            <div>
              <label className="block text-xs font-medium text-muted-foreground mb-1">
                Identity Number <span className="text-destructive">*</span>
              </label>
              <input
                type="text"
                value={newIdentityForm.identitynumber}
                onChange={(e) =>
                  setNewIdentityForm((prev) => ({
                    ...prev,
                    identitynumber: e.target.value,
                  }))
                }
                className="w-full p-2 text-xs border rounded"
                placeholder="Enter identity number..."
              />
            </div>

            <div>
              <label className="block text-xs font-medium text-muted-foreground mb-1">
                Issue Date <span className="text-destructive">*</span>
              </label>
              <input
                type="date"
                value={newIdentityForm.issuedate}
                onChange={(e) =>
                  setNewIdentityForm((prev) => ({
                    ...prev,
                    issuedate: e.target.value,
                  }))
                }
                className="w-full p-2 text-xs border rounded"
              />
            </div>

            <div>
              <label className="block text-xs font-medium text-muted-foreground mb-1">
                Expiry Date
              </label>
              <input
                type="date"
                value={newIdentityForm.expirydate}
                onChange={(e) =>
                  setNewIdentityForm((prev) => ({
                    ...prev,
                    expirydate: e.target.value,
                  }))
                }
                className="w-full p-2 text-xs border rounded"
              />
            </div>

            <div>
              <label className="block text-xs font-medium text-muted-foreground mb-1">
                Country of Issue
              </label>
              <SearchableSelect
                options={countryOptions}
                value={newIdentityForm.countryofissuelookupid}
                onChange={(value) =>
                  setNewIdentityForm((prev) => ({
                    ...prev,
                    countryofissuelookupid: value,
                  }))
                }
                placeholder="Select country..."
                loading={loadingOptions}
                className="w-full"
              />
            </div>

            <div>
              <label className="block text-xs font-medium text-muted-foreground mb-1">
                Document Status
              </label>
              <SearchableSelect
                options={statusOptions}
                value={newIdentityForm.identitydocumentstatuslookupid}
                onChange={(value) =>
                  setNewIdentityForm((prev) => ({
                    ...prev,
                    identitydocumentstatuslookupid: value,
                  }))
                }
                placeholder="Select status..."
                loading={loadingOptions}
                className="w-full"
              />
            </div>
          </div>

          <div className="flex justify-end gap-2">
            <button
              onClick={() => {
                setIsAddingNew(false);
                setNewIdentityForm({
                  identitytypelookupid: null,
                  identitynumber: "",
                  issuedate: "",
                  expirydate: "",
                  countryofissuelookupid: null,
                  identitydocumentstatuslookupid: null,
                });
              }}
              className="compact-button border"
              disabled={isSubmitting}
            >
              Cancel
            </button>
            <button
              onClick={handleAddNew}
              className="compact-button bg-primary text-white"
              disabled={isSubmitting}
            >
              {isSubmitting ? (
                <>
                  <Loader2 className="w-3 h-3 mr-1 animate-spin" />
                  Adding...
                </>
              ) : (
                "Add Document"
              )}
            </button>
          </div>
        </div>
      )}

      {/* Identities Table */}
      {identities.length > 0 && (
        <div className="border border-gray-200 rounded-lg overflow-hidden">
          <table className="data-table w-full">
            <thead>
              <tr>
                <th>Type</th>
                <th>Number</th>
                <th>Issue Date</th>
                <th>Expiry Date</th>
                <th>Country</th>
                <th>Status</th>
                <th>Meta</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {identities.map((identity) => (
                <tr
                  key={identity.tgpersonidentityid}
                  className={cn(
                    "hover:bg-muted/50",
                    !identity.isactive && "opacity-60",
                  )}
                >
                  <td>
                    <div className="space-y-0.5">
                      <LookupField
                        lookupId={identity.identitytypelookupid}
                        format="name"
                        className="text-xs font-medium"
                      />
                      <div className="text-[10px] text-muted-foreground">
                        ID: {identity.identitytypelookupid}
                      </div>
                    </div>
                  </td>
                  <td className="font-mono text-xs">
                    {identity.identitynumber}
                  </td>
                  <td className="text-xs">
                    {formatDateCompact(identity.issuedate)}
                  </td>
                  <td>
                    <div className="flex items-center gap-1">
                      <span className="text-xs">
                        {formatDateCompact(identity.expirydate)}
                      </span>
                      {isExpired(identity.expirydate) && (
                        <span className="badge badge-error text-[9px]">
                          Expired
                        </span>
                      )}
                      {isExpiringSoon(identity.expirydate) && (
                        <span className="badge badge-warning text-[9px]">
                          Expiring
                        </span>
                      )}
                    </div>
                  </td>
                  <td>
                    <div className="space-y-0.5">
                      {identity.countryofissuelookupid ? (
                        <>
                          <LookupField
                            lookupId={identity.countryofissuelookupid}
                            format="name"
                            className="text-xs"
                          />
                          <div className="text-[10px] text-muted-foreground">
                            ID: {identity.countryofissuelookupid}
                          </div>
                        </>
                      ) : (
                        <span className="text-xs text-muted-foreground">-</span>
                      )}
                    </div>
                  </td>
                  <td>
                    <div className="space-y-0.5">
                      {identity.identitydocumentstatuslookupid ? (
                        <>
                          <LookupField
                            lookupId={identity.identitydocumentstatuslookupid}
                            format="name"
                            className="text-xs"
                          />
                          <div className="text-[10px] text-muted-foreground">
                            ID: {identity.identitydocumentstatuslookupid}
                          </div>
                        </>
                      ) : (
                        <span className="text-xs text-muted-foreground">-</span>
                      )}
                    </div>
                  </td>
                  <td>
                    <MetaStrip record={identity} />
                  </td>
                  <td>
                    <div className="flex gap-1">
                      <button
                        onClick={() => handleStartEdit(identity)}
                        className="p-1 hover:bg-muted rounded"
                        title="Edit"
                      >
                        <Edit2 className="w-3 h-3" />
                      </button>
                      {identity.isactive &&
                        !validIdentityIds.has(identity.tgpersonidentityid) && (
                          <button
                            onClick={() =>
                              handleValidate(identity.tgpersonidentityid)
                            }
                            className="p-1 hover:bg-muted rounded text-green-600"
                            title="Validate Document"
                          >
                            <CheckCircle className="w-3 h-3" />
                          </button>
                        )}
                      {identity.isactive && (
                        <button
                          onClick={() =>
                            handleVoid(identity.tgpersonidentityid)
                          }
                          className="p-1 hover:bg-red-100 rounded text-red-600 hover:text-red-700"
                          title="Void"
                        >
                          <AlertCircle className="w-3 h-3" />
                        </button>
                      )}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Empty State */}
      {identities.length === 0 && (
        <div className="text-center py-12 bg-gradient-to-br from-blue-50 to-indigo-50 rounded-lg border-2 border-dashed border-blue-200">
          <CreditCard className="w-16 h-16 text-blue-300 mx-auto mb-4" />
          <h3 className="text-lg font-semibold text-gray-900 mb-2">
            📄 No Identity Documents Found
          </h3>
          <p className="text-gray-600 mb-4 max-w-md mx-auto">
            This person has no registered identity documents such as passports,
            national IDs, or driver's licenses in the system.
          </p>
          <div className="flex flex-col items-center gap-3">
            <button
              onClick={() => setIsAddingNew(true)}
              className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 flex items-center gap-2"
            >
              <Plus className="w-4 h-4" />
              Add First Identity Document
            </button>
            <div className="text-sm text-blue-700 bg-blue-100 px-3 py-2 rounded-lg inline-flex items-center gap-2">
              <span className="text-blue-500">💡</span>
              <span>Identity documents will appear here once added</span>
            </div>
          </div>
        </div>
      )}

      {/* Edit Modal */}
      {editingIdentity && (
        <div
          className="fixed inset-0 bg-black bg-opacity-75 flex items-center justify-center z-50"
          onClick={() => setEditingIdentity(null)}
        >
          <div
            className="bg-white rounded-lg p-6 max-w-2xl w-full mx-4"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="text-sm font-semibold mb-4">
              Edit Identity Document
            </h3>

            <div className="space-y-4">
              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Identity Number
                </label>
                <input
                  type="text"
                  value={editForm.identitynumber}
                  onChange={(e) =>
                    setEditForm((prev) => ({
                      ...prev,
                      identitynumber: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-xs border rounded"
                  placeholder="Enter identity number..."
                />
              </div>

              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="block text-xs font-medium text-gray-600 mb-1">
                    Issue Date
                  </label>
                  <input
                    type="date"
                    value={editForm.issuedate}
                    onChange={(e) =>
                      setEditForm((prev) => ({
                        ...prev,
                        issuedate: e.target.value,
                      }))
                    }
                    className="w-full p-2 text-xs border rounded"
                  />
                </div>

                <div>
                  <label className="block text-xs font-medium text-gray-600 mb-1">
                    Expiry Date
                  </label>
                  <input
                    type="date"
                    value={editForm.expirydate}
                    onChange={(e) =>
                      setEditForm((prev) => ({
                        ...prev,
                        expirydate: e.target.value,
                      }))
                    }
                    className="w-full p-2 text-xs border rounded"
                  />
                </div>
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Country of Issue
                </label>
                <SearchableSelect
                  options={countryOptions}
                  value={editForm.countryofissuelookupid}
                  onChange={(value) =>
                    setEditForm((prev) => ({
                      ...prev,
                      countryofissuelookupid: value,
                    }))
                  }
                  placeholder="Select country..."
                  loading={loadingOptions}
                  className="w-full"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Document Status
                </label>
                <SearchableSelect
                  options={statusOptions}
                  value={editForm.identitydocumentstatuslookupid}
                  onChange={(value) =>
                    setEditForm((prev) => ({
                      ...prev,
                      identitydocumentstatuslookupid: value,
                    }))
                  }
                  placeholder="Select status..."
                  loading={loadingOptions}
                  className="w-full"
                />
              </div>
            </div>

            <div className="flex justify-end gap-3 mt-6">
              <button
                onClick={() => setEditingIdentity(null)}
                className="px-4 py-2 text-gray-700 bg-gray-100 rounded-lg hover:bg-gray-200 focus:ring-2 focus:ring-gray-500"
                disabled={isSubmitting}
              >
                Cancel
              </button>
              <button
                onClick={handleEditIdentity}
                className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 focus:ring-2 focus:ring-blue-500 disabled:opacity-50"
                disabled={isSubmitting}
              >
                {isSubmitting ? (
                  <>
                    <Loader2 className="w-4 h-4 mr-2 animate-spin inline" />
                    Saving...
                  </>
                ) : (
                  "Save Changes"
                )}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
