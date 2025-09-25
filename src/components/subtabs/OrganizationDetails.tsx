//
//  campus-pilot
//  OrganizationDetails.tsx
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import React, { useState, useEffect } from "react";
import {
  Building,
  Mail,
  Phone,
  MapPin,
  Edit2,
  Save,
  X,
  Loader2,
  AlertCircle,
} from "lucide-react";
import { TgOrganisation } from "../../types/organization";
import { apiClient } from "../../lib/api";
import { LookupField } from "../LookupField";
import { SearchableSelect } from "../SearchableSelect";
import toast from "react-hot-toast";

interface OrganizationDetailsProps {
  organizationId: number;
}

export function OrganizationDetails({
  organizationId,
}: OrganizationDetailsProps) {
  const [organization, setOrganization] = useState<TgOrganisation | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [isEditing, setIsEditing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);

  // Form state for editing
  const [editForm, setEditForm] = useState({
    organisationname: "",
    organisationdescription: "",
    organisationemail: "",
    organisationcontactnumber: "",
    physicaladdressline1: "",
    physicaladdressline2: "",
    physicalcity: "",
    city: "",
    officehours: "",
    externalorganisationtypelookupid: null as number | null,
    organisationstatuslookupid: null as number | null,
    regionlookupid: null as number | null,
    zonelookupid: null as number | null,
    woredalookupid: null as number | null,
    kebelelookupid: null as number | null,
  });

  // Lookup options
  const [organizationTypeOptions, setOrganizationTypeOptions] = useState<any[]>(
    [],
  );
  const [organizationStatusOptions, setOrganizationStatusOptions] = useState<
    any[]
  >([]);
  const [regionOptions, setRegionOptions] = useState<any[]>([]);
  const [zoneOptions, setZoneOptions] = useState<any[]>([]);
  const [woredaOptions, setWoredaOptions] = useState<any[]>([]);
  const [kebeleOptions, setKebeleOptions] = useState<any[]>([]);
  const [loadingOptions, setLoadingOptions] = useState(true);

  useEffect(() => {
    const fetchOrganization = async () => {
      try {
        setIsLoading(true);
        setError(null);

        const data = await apiClient.getOrganization(organizationId);
        // Convert date strings to Date objects
        const processedData = {
          ...data,
          createdate: new Date(data.createdate),
          modifieddate: data.modifieddate
            ? new Date(data.modifieddate)
            : undefined,
        };
        setOrganization(processedData);

        // Initialize edit form with organization data
        setEditForm({
          organisationname: data.organisationname || "",
          organisationdescription: data.organisationdescription || "",
          organisationemail: data.organisationemail || "",
          organisationcontactnumber: data.organisationcontactnumber || "",
          physicaladdressline1: data.physicaladdressline1 || "",
          physicaladdressline2: data.physicaladdressline2 || "",
          physicalcity: data.physicalcity || "",
          city: data.city || "",
          officehours: data.officehours || "",
          externalorganisationtypelookupid:
            data.externalorganisationtypelookupid || null,
          organisationstatuslookupid: data.organisationstatuslookupid || null,
          regionlookupid: data.regionlookupid || null,
          zonelookupid: data.zonelookupid || null,
          woredalookupid: data.woredalookupid || null,
          kebelelookupid: data.kebelelookupid || null,
        });
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to load organization";
        setError(errorMessage);
        toast.error(errorMessage);
      } finally {
        setIsLoading(false);
      }
    };

    const fetchLookupOptions = async () => {
      try {
        const [
          orgTypeData,
          orgStatusData,
          regionData,
          zoneData,
          woredaData,
          kebeleData,
        ] = await Promise.all([
          apiClient.getLookupsByType("EXTERNAL_ORGANIZATION_TYPE"),
          apiClient.getLookupsByType("ORGANISATION_STATUS"),
          apiClient.getLocalityLookupsByType("REGIONS"),
          apiClient.getLocalityLookupsByType("ZONES"),
          apiClient.getLocalityLookupsByType("WOREDAS"),
          apiClient.getLocalityLookupsByType("KEBELES"),
        ]);

        setOrganizationTypeOptions(
          orgTypeData.map((lookup) => ({
            id: lookup.tglookupid,
            value: lookup.lookupvalue,
            label: lookup.lookupdescription || lookup.lookupvalue,
          })),
        );

        setOrganizationStatusOptions(
          orgStatusData.map((lookup) => ({
            id: lookup.tglookupid,
            value: lookup.lookupvalue,
            label: lookup.lookupdescription || lookup.lookupvalue,
          })),
        );

        setRegionOptions(
          regionData.map((lookup) => ({
            id: lookup.tglocalitylookupid,
            value: lookup.lookupvalue,
            label: lookup.lookupdescription || lookup.lookupvalue,
          })),
        );

        setZoneOptions(
          zoneData.map((lookup) => ({
            id: lookup.tglocalitylookupid,
            value: lookup.lookupvalue,
            label: lookup.lookupdescription || lookup.lookupvalue,
          })),
        );

        setWoredaOptions(
          woredaData.map((lookup) => ({
            id: lookup.tglocalitylookupid,
            value: lookup.lookupvalue,
            label: lookup.lookupdescription || lookup.lookupvalue,
          })),
        );

        setKebeleOptions(
          kebeleData.map((lookup) => ({
            id: lookup.tglocalitylookupid,
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

    fetchOrganization();
    fetchLookupOptions();
  }, [organizationId]);

  const handleEdit = () => {
    setIsEditing(true);
  };

  const handleCancel = () => {
    setIsEditing(false);
    // Reset form to original data
    if (organization) {
      setEditForm({
        organisationname: organization.organisationname || "",
        organisationdescription: organization.organisationdescription || "",
        organisationemail: organization.organisationemail || "",
        organisationcontactnumber: organization.organisationcontactnumber || "",
        physicaladdressline1: organization.physicaladdressline1 || "",
        physicaladdressline2: organization.physicaladdressline2 || "",
        physicalcity: organization.physicalcity || "",
        city: organization.city || "",
        officehours: organization.officehours || "",
        externalorganisationtypelookupid:
          organization.externalorganisationtypelookupid || null,
        organisationstatuslookupid:
          organization.organisationstatuslookupid || null,
        regionlookupid: organization.regionlookupid || null,
        zonelookupid: organization.zonelookupid || null,
        woredalookupid: organization.woredalookupid || null,
        kebelelookupid: organization.kebelelookupid || null,
      });
    }
  };

  const handleSave = async () => {
    setIsSaving(true);
    try {
      await apiClient.updateOrganization(organizationId, editForm);

      // Refresh organization data
      const data = await apiClient.getOrganization(organizationId);
      setOrganization(data);
      setIsEditing(false);

      toast.success("Organization updated successfully");
    } catch (error) {
      toast.error("Failed to update organization");
    } finally {
      setIsSaving(false);
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center p-8">
        <Loader2 className="w-6 h-6 animate-spin" />
      </div>
    );
  }

  if (error || !organization) {
    return (
      <div className="flex flex-col items-center justify-center p-8 text-center">
        <AlertCircle className="w-12 h-12 text-red-500 mb-4" />
        <h3 className="text-lg font-semibold text-gray-900 mb-2">
          Error Loading Organization
        </h3>
        <p className="text-gray-600 mb-4">
          {error || "Organization not found"}
        </p>
        <button
          onClick={() => window.location.reload()}
          className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700"
        >
          Retry
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold flex items-center gap-2">
          <Building className="w-5 h-5" />
          Organization Details
        </h2>
        {!isEditing ? (
          <button
            onClick={handleEdit}
            className="flex items-center gap-2 px-4 py-2 text-blue-600 border border-blue-200 rounded-lg hover:bg-blue-50"
          >
            <Edit2 className="w-4 h-4" />
            Edit
          </button>
        ) : (
          <div className="flex gap-2">
            <button
              onClick={handleCancel}
              className="flex items-center gap-2 px-4 py-2 text-gray-600 border border-gray-200 rounded-lg hover:bg-gray-50"
              disabled={isSaving}
            >
              <X className="w-4 h-4" />
              Cancel
            </button>
            <button
              onClick={handleSave}
              className="flex items-center gap-2 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700"
              disabled={isSaving}
            >
              {isSaving ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <Save className="w-4 h-4" />
              )}
              Save
            </button>
          </div>
        )}
      </div>

      {/* Organization Info Grid */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Basic Information */}
        <div className="bg-white rounded-lg border border-gray-200 p-6">
          <h3 className="text-sm font-semibold mb-4 flex items-center gap-2">
            <Building className="w-4 h-4" />
            Basic Information
          </h3>

          <div className="space-y-4">
            <div>
              <label className="block text-xs font-medium text-gray-600 mb-1">
                Organization Name
              </label>
              {isEditing ? (
                <input
                  type="text"
                  value={editForm.organisationname}
                  onChange={(e) =>
                    setEditForm((prev) => ({
                      ...prev,
                      organisationname: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-sm border rounded"
                />
              ) : (
                <p className="text-sm text-gray-900">
                  {organization.organisationname || "-"}
                </p>
              )}
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-600 mb-1">
                Description
              </label>
              {isEditing ? (
                <textarea
                  value={editForm.organisationdescription}
                  onChange={(e) =>
                    setEditForm((prev) => ({
                      ...prev,
                      organisationdescription: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-sm border rounded h-20"
                />
              ) : (
                <p className="text-sm text-gray-900">
                  {organization.organisationdescription || "-"}
                </p>
              )}
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-600 mb-1">
                Organization Type
              </label>
              {isEditing ? (
                <SearchableSelect
                  options={organizationTypeOptions}
                  value={editForm.externalorganisationtypelookupid}
                  onChange={(value) =>
                    setEditForm((prev) => ({
                      ...prev,
                      externalorganisationtypelookupid: value,
                    }))
                  }
                  placeholder="Select organization type..."
                  loading={loadingOptions}
                  className="w-full"
                />
              ) : (
                <LookupField
                  lookupId={organization.externalorganisationtypelookupid}
                  format="both"
                  className="text-sm text-gray-900"
                  fallback="-"
                />
              )}
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-600 mb-1">
                Status
              </label>
              {isEditing ? (
                <SearchableSelect
                  options={organizationStatusOptions}
                  value={editForm.organisationstatuslookupid}
                  onChange={(value) =>
                    setEditForm((prev) => ({
                      ...prev,
                      organisationstatuslookupid: value,
                    }))
                  }
                  placeholder="Select status..."
                  loading={loadingOptions}
                  className="w-full"
                />
              ) : (
                <LookupField
                  lookupId={organization.organisationstatuslookupid}
                  format="both"
                  className="text-sm text-gray-900"
                  fallback="-"
                />
              )}
            </div>
          </div>
        </div>

        {/* Contact Information */}
        <div className="bg-white rounded-lg border border-gray-200 p-6">
          <h3 className="text-sm font-semibold mb-4 flex items-center gap-2">
            <Phone className="w-4 h-4" />
            Contact Information
          </h3>

          <div className="space-y-4">
            <div>
              <label className="block text-xs font-medium text-gray-600 mb-1">
                Email Address
              </label>
              {isEditing ? (
                <input
                  type="email"
                  value={editForm.organisationemail}
                  onChange={(e) =>
                    setEditForm((prev) => ({
                      ...prev,
                      organisationemail: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-sm border rounded"
                />
              ) : (
                <p className="text-sm text-gray-900">
                  {organization.organisationemail || "-"}
                </p>
              )}
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-600 mb-1">
                Contact Number
              </label>
              {isEditing ? (
                <input
                  type="text"
                  value={editForm.organisationcontactnumber}
                  onChange={(e) =>
                    setEditForm((prev) => ({
                      ...prev,
                      organisationcontactnumber: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-sm border rounded"
                />
              ) : (
                <p className="text-sm text-gray-900">
                  {organization.organisationcontactnumber || "-"}
                </p>
              )}
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-600 mb-1">
                Office Hours
              </label>
              {isEditing ? (
                <textarea
                  value={editForm.officehours}
                  onChange={(e) =>
                    setEditForm((prev) => ({
                      ...prev,
                      officehours: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-sm border rounded h-16"
                  placeholder="e.g., Monday-Friday 8:00 AM - 5:00 PM"
                />
              ) : (
                <p className="text-sm text-gray-900">
                  {organization.officehours || "-"}
                </p>
              )}
            </div>
          </div>
        </div>

        {/* Address Information */}
        <div className="bg-white rounded-lg border border-gray-200 p-6">
          <h3 className="text-sm font-semibold mb-4 flex items-center gap-2">
            <MapPin className="w-4 h-4" />
            Address Information
          </h3>

          <div className="space-y-4">
            <div>
              <label className="block text-xs font-medium text-gray-600 mb-1">
                Physical Address Line 1
              </label>
              {isEditing ? (
                <input
                  type="text"
                  value={editForm.physicaladdressline1}
                  onChange={(e) =>
                    setEditForm((prev) => ({
                      ...prev,
                      physicaladdressline1: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-sm border rounded"
                />
              ) : (
                <p className="text-sm text-gray-900">
                  {organization.physicaladdressline1 || "-"}
                </p>
              )}
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-600 mb-1">
                Physical Address Line 2
              </label>
              {isEditing ? (
                <input
                  type="text"
                  value={editForm.physicaladdressline2}
                  onChange={(e) =>
                    setEditForm((prev) => ({
                      ...prev,
                      physicaladdressline2: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-sm border rounded"
                />
              ) : (
                <p className="text-sm text-gray-900">
                  {organization.physicaladdressline2 || "-"}
                </p>
              )}
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-600 mb-1">
                City
              </label>
              {isEditing ? (
                <input
                  type="text"
                  value={editForm.city}
                  onChange={(e) =>
                    setEditForm((prev) => ({ ...prev, city: e.target.value }))
                  }
                  className="w-full p-2 text-sm border rounded"
                />
              ) : (
                <p className="text-sm text-gray-900">
                  {organization.city || "-"}
                </p>
              )}
            </div>
          </div>
        </div>

        {/* Ethiopian Location Hierarchy */}
        <div className="bg-white rounded-lg border border-gray-200 p-6">
          <h3 className="text-sm font-semibold mb-4">
            Ethiopian Administrative Divisions
          </h3>

          <div className="space-y-4">
            <div>
              <label className="block text-xs font-medium text-gray-600 mb-1">
                Region
              </label>
              {isEditing ? (
                <SearchableSelect
                  options={regionOptions}
                  value={editForm.regionlookupid}
                  onChange={(value) =>
                    setEditForm((prev) => ({ ...prev, regionlookupid: value }))
                  }
                  placeholder="Select region..."
                  loading={loadingOptions}
                  className="w-full"
                />
              ) : (
                <LookupField
                  lookupId={organization.regionlookupid}
                  format="both"
                  className="text-sm text-gray-900"
                  fallback="-"
                />
              )}
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-600 mb-1">
                Zone
              </label>
              {isEditing ? (
                <SearchableSelect
                  options={zoneOptions}
                  value={editForm.zonelookupid}
                  onChange={(value) =>
                    setEditForm((prev) => ({ ...prev, zonelookupid: value }))
                  }
                  placeholder="Select zone..."
                  loading={loadingOptions}
                  className="w-full"
                />
              ) : (
                <LookupField
                  lookupId={organization.zonelookupid}
                  format="both"
                  className="text-sm text-gray-900"
                  fallback="-"
                />
              )}
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-600 mb-1">
                Woreda
              </label>
              {isEditing ? (
                <SearchableSelect
                  options={woredaOptions}
                  value={editForm.woredalookupid}
                  onChange={(value) =>
                    setEditForm((prev) => ({ ...prev, woredalookupid: value }))
                  }
                  placeholder="Select woreda..."
                  loading={loadingOptions}
                  className="w-full"
                />
              ) : (
                <LookupField
                  lookupId={organization.woredalookupid}
                  format="both"
                  className="text-sm text-gray-900"
                  fallback="-"
                />
              )}
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-600 mb-1">
                Kebele
              </label>
              {isEditing ? (
                <SearchableSelect
                  options={kebeleOptions}
                  value={editForm.kebelelookupid}
                  onChange={(value) =>
                    setEditForm((prev) => ({ ...prev, kebelelookupid: value }))
                  }
                  placeholder="Select kebele..."
                  loading={loadingOptions}
                  className="w-full"
                />
              ) : (
                <LookupField
                  lookupId={organization.kebelelookupid}
                  format="both"
                  className="text-sm text-gray-900"
                  fallback="-"
                />
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Meta Information */}
      <div className="bg-gray-50 rounded-lg p-4">
        <h3 className="text-sm font-semibold mb-2">Meta Information</h3>
        <div className="grid grid-cols-2 lg:grid-cols-4 gap-4 text-xs">
          <div>
            <span className="text-gray-500">Organization ID:</span>
            <br />
            <span className="font-mono">{organization.tgorganisationid}</span>
          </div>
          <div>
            <span className="text-gray-500">Created:</span>
            <br />
            <span>{organization.createdate.toLocaleDateString()}</span>
          </div>
          <div>
            <span className="text-gray-500">Modified:</span>
            <br />
            <span>
              {organization.modifieddate?.toLocaleDateString() || "-"}
            </span>
          </div>
          <div>
            <span className="text-gray-500">Status:</span>
            <br />
            <span
              className={
                organization.isactive ? "text-green-600" : "text-red-600"
              }
            >
              {organization.isactive ? "Active" : "Inactive"}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}
