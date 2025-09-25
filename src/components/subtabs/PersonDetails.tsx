//
//  campus-pilot
//  PersonDetails.tsx
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { useState, useEffect } from "react";
import { useActiveTab, useUIStore } from "../../stores/uiStore";
import {
  User,
  Calendar,
  Hash,
  Clock,
  Mail,
  Phone,
  MapPin,
  AlertCircle,
  Loader2,
  Eye,
  Shield,
  Globe,
  Heart,
  Users,
  Flag,
  Edit2,
  Save,
} from "lucide-react";
import { formatDate } from "../../lib/utils";
import { cn } from "../../lib/utils";
import { LookupField, useLookupPreloader } from "../LookupField";
import { SearchableSelect } from "../SearchableSelect";
import { apiClient } from "../../lib/api";
import {
  createChangeTracker,
  type ChangeTracker,
} from "../../lib/changeTracker";
import toast from "react-hot-toast";

interface PersonDetailsProps {
  personId: number;
}

interface PersonData {
  tgpersonid: number;
  firstname: string;
  firstnamesamharic: string;
  middlename: string;
  middlenameamharic: string;
  surname: string;
  surnameamharic: string;
  initials: string;
  height: string;
  eyecolourlookupid: number;
  haircolourlookupid: number;
  titlelookupid: number;
  genderlookupid: number;
  nationalitylookupid: number;
  placeofbirthlookupid: number;
  cityofbirth: string;
  countryofbirthlookupid: number;
  birthdate: string;
  maritalstatuslookupid: number;
  divorcedate: string;
  isadopted: boolean;
  specialmarks: string;
  halfcast: boolean;
  personstatuslookupid: number;
  contactnumber: string;
  whatsappavailable: boolean;
  alternatecontactnumber: string;
  emailaddress: string;
  alternateemailaddress: string;
  mainregionlookupid: number;
  mainzonelookupid: number;
  mainworedalookupid: number;
  mainkebelelookupid: number;
  maincity: string;
  mainaddressline1: string;
  alternateregionlookupid: number;
  alternatezonelookupid: number;
  alternateworedalookupid: number;
  alternatekebelelookupid: number;
  alternatecity: string;
  alternateaddressline1: string;
  deliveryregionlookupid: number;
  deliveryzonelookupid: number;
  deliveryworedalookupid: number;
  deliverykebelelookupid: number;
  deliverycity: string;
  deliveryaddressline1: string;
  abroadcountrylookupid: number;
  abroadaddressline1: string;
  abroadcontactnumber: string;
  securityquestionlookupid1: number;
  securityanswer1: string;
  securityquestionlookupid2: number;
  securityanswer2: string;
  watchliststatuslookupid: number;
  portalrecordstatuslookupid: number;
  recordstatuslookupid: number;
  createdate: string;
  modifieddate: string;
  tguserauditdetailid: number;
  createdbysystemuserid: number;
  updatedbysystemuserid: number;
  dataownerlookupid: number;
  isactive: boolean;
}

export function PersonDetails({ personId }: PersonDetailsProps) {
  const [personData, setPersonData] = useState<PersonData | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [isEditing, setIsEditing] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [changeTracker, setChangeTracker] = useState<ChangeTracker<any> | null>(
    null,
  );
  const [editForm, setEditForm] = useState({
    firstname: "",
    firstnamesamharic: "",
    middlename: "",
    middlenameamharic: "",
    surname: "",
    surnameamharic: "",
    initials: "",
    height: "",
    eyecolourlookupid: null as number | null,
    haircolourlookupid: null as number | null,
    titlelookupid: null as number | null,
    genderlookupid: null as number | null,
    nationalitylookupid: null as number | null,
    placeofbirthlookupid: null as number | null,
    cityofbirth: "",
    countryofbirthlookupid: null as number | null,
    birthdate: "",
    maritalstatuslookupid: null as number | null,
    divorcedate: "",
    isadopted: false,
    specialmarks: "",
    halfcast: false,
    personstatuslookupid: null as number | null,
    contactnumber: "",
    whatsappavailable: false,
    alternatecontactnumber: "",
    emailaddress: "",
    alternateemailaddress: "",
    mainregionlookupid: null as number | null,
    mainzonelookupid: null as number | null,
    mainworedalookupid: null as number | null,
    mainkebelelookupid: null as number | null,
    maincity: "",
    mainaddressline1: "",
    alternateregionlookupid: null as number | null,
    alternatezonelookupid: null as number | null,
    alternateworedalookupid: null as number | null,
    alternatekebelelookupid: null as number | null,
    alternatecity: "",
    alternateaddressline1: "",
    deliveryregionlookupid: null as number | null,
    deliveryzonelookupid: null as number | null,
    deliveryworedalookupid: null as number | null,
    deliverykebelelookupid: null as number | null,
    deliverycity: "",
    deliveryaddressline1: "",
    abroadcountrylookupid: null as number | null,
    abroadaddressline1: "",
    abroadcontactnumber: "",
    securityquestionlookupid1: null as number | null,
    securityanswer1: "",
    securityquestionlookupid2: null as number | null,
    securityanswer2: "",
    watchliststatuslookupid: null as number | null,
  });

  // Lookup options state
  const [genderOptions, setGenderOptions] = useState<any[]>([]);
  const [hairColorOptions, setHairColorOptions] = useState<any[]>([]);
  const [eyeColorOptions, setEyeColorOptions] = useState<any[]>([]);
  const [nationalityOptions, setNationalityOptions] = useState<any[]>([]);
  const [maritalStatusOptions, setMaritalStatusOptions] = useState<any[]>([]);
  const [loadingOptions, setLoadingOptions] = useState(true);

  // Fetch person data
  useEffect(() => {
    const fetchData = async () => {
      try {
        setIsLoading(true);
        setError(null);

        const data = await apiClient.getPersonDetails(personId);
        setPersonData(data);
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to load person data";
        setError(errorMessage);
        toast.error(errorMessage);
      } finally {
        setIsLoading(false);
      }
    };

    if (personId) {
      fetchData();
    }
  }, [personId]);

  // Fetch lookup options
  useEffect(() => {
    const fetchLookupOptions = async () => {
      try {
        setLoadingOptions(true);
        const [
          genderData,
          hairColorData,
          eyeColorData,
          nationalityData,
          maritalStatusData,
        ] = await Promise.all([
          apiClient.getLookupsByType("GENDER"),
          apiClient.getLookupsByType("HAIR_COLOUR"),
          apiClient.getLookupsByType("EYE_COLOUR"),
          apiClient.getLookupsByType("NATIONALITY"),
          apiClient.getLookupsByType("MARITAL_STATUS"),
        ]);

        setGenderOptions(
          genderData.map((lookup) => ({
            id: lookup.tglookupid,
            value: lookup.lookupvalue,
            label: lookup.lookupdescription || lookup.lookupvalue,
          })),
        );
        setHairColorOptions(
          hairColorData.map((lookup) => ({
            id: lookup.tglookupid,
            value: lookup.lookupvalue,
            label: lookup.lookupdescription || lookup.lookupvalue,
          })),
        );
        setEyeColorOptions(
          eyeColorData.map((lookup) => ({
            id: lookup.tglookupid,
            value: lookup.lookupvalue,
            label: lookup.lookupdescription || lookup.lookupvalue,
          })),
        );
        setNationalityOptions(
          nationalityData.map((lookup) => ({
            id: lookup.tglookupid,
            value: lookup.lookupvalue,
            label: lookup.lookupdescription || lookup.lookupvalue,
          })),
        );
        setMaritalStatusOptions(
          maritalStatusData.map((lookup) => ({
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

  const handleStartEdit = () => {
    if (personData) {
      const formData = {
        firstname: personData.firstname || "",
        firstnamesamharic: personData.firstnamesamharic || "",
        middlename: personData.middlename || "",
        middlenameamharic: personData.middlenameamharic || "",
        surname: personData.surname || "",
        surnameamharic: personData.surnameamharic || "",
        initials: personData.initials || "",
        height: personData.height || "",
        eyecolourlookupid: personData.eyecolourlookupid || null,
        haircolourlookupid: personData.haircolourlookupid || null,
        titlelookupid: personData.titlelookupid || null,
        genderlookupid: personData.genderlookupid || null,
        nationalitylookupid: personData.nationalitylookupid || null,
        placeofbirthlookupid: personData.placeofbirthlookupid || null,
        cityofbirth: personData.cityofbirth || "",
        countryofbirthlookupid: personData.countryofbirthlookupid || null,
        birthdate: personData.birthdate
          ? personData.birthdate.split("T")[0]
          : "",
        maritalstatuslookupid: personData.maritalstatuslookupid || null,
        divorcedate: personData.divorcedate
          ? personData.divorcedate.split("T")[0]
          : "",
        isadopted: personData.isadopted || false,
        specialmarks: personData.specialmarks || "",
        halfcast: personData.halfcast || false,
        personstatuslookupid: personData.personstatuslookupid || null,
        contactnumber: personData.contactnumber || "",
        whatsappavailable: personData.whatsappavailable || false,
        alternatecontactnumber: personData.alternatecontactnumber || "",
        emailaddress: personData.emailaddress || "",
        alternateemailaddress: personData.alternateemailaddress || "",
        mainregionlookupid: personData.mainregionlookupid || null,
        mainzonelookupid: personData.mainzonelookupid || null,
        mainworedalookupid: personData.mainworedalookupid || null,
        mainkebelelookupid: personData.mainkebelelookupid || null,
        maincity: personData.maincity || "",
        mainaddressline1: personData.mainaddressline1 || "",
        alternateregionlookupid: personData.alternateregionlookupid || null,
        alternatezonelookupid: personData.alternatezonelookupid || null,
        alternateworedalookupid: personData.alternateworedalookupid || null,
        alternatekebelelookupid: personData.alternatekebelelookupid || null,
        alternatecity: personData.alternatecity || "",
        alternateaddressline1: personData.alternateaddressline1 || "",
        deliveryregionlookupid: personData.deliveryregionlookupid || null,
        deliveryzonelookupid: personData.deliveryzonelookupid || null,
        deliveryworedalookupid: personData.deliveryworedalookupid || null,
        deliverykebelelookupid: personData.deliverykebelelookupid || null,
        deliverycity: personData.deliverycity || "",
        deliveryaddressline1: personData.deliveryaddressline1 || "",
        abroadcountrylookupid: personData.abroadcountrylookupid || null,
        abroadaddressline1: personData.abroadaddressline1 || "",
        abroadcontactnumber: personData.abroadcontactnumber || "",
        securityquestionlookupid1: personData.securityquestionlookupid1 || null,
        securityanswer1: personData.securityanswer1 || "",
        securityquestionlookupid2: personData.securityquestionlookupid2 || null,
        securityanswer2: personData.securityanswer2 || "",
        watchliststatuslookupid: personData.watchliststatuslookupid || null,
      };

      setEditForm(formData);

      // Initialize change tracker with current form data
      const tracker = createChangeTracker(formData);
      setChangeTracker(tracker);

      setIsEditing(true);
    }
  };

  const handleSaveEdit = async () => {
    if (!changeTracker || !changeTracker.hasChanges()) {
      toast("No changes to save");
      return;
    }

    setIsSubmitting(true);
    try {
      // Generate update payload with only changed fields
      const payload = changeTracker.generateUpdatePayload(personId);

      if (!payload) {
        toast("No changes to save");
        return;
      }

      // Log what's being sent
      console.log(`Updating person ${personId}:`, {
        changedFields: payload.metadata.changedFields,
        changes: payload.changes,
      });

      // Send only the changed fields
      await apiClient.updatePerson(personId, payload.changes);

      // Update local state with changes
      if (personData) {
        setPersonData({
          ...personData,
          ...payload.changes,
          modifieddate: new Date().toISOString(),
        });
      }

      // Commit the changes to tracker
      changeTracker.commit();

      setIsEditing(false);
      toast.success(
        `Updated ${payload.metadata.changedFields.join(", ")} successfully`,
      );
    } catch (error) {
      toast.error("Failed to update person details");
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleCancelEdit = () => {
    setIsEditing(false);
    setEditForm({
      firstname: "",
      firstnamesamharic: "",
      middlename: "",
      middlenameamharic: "",
      surname: "",
      surnameamharic: "",
      initials: "",
      height: "",
      eyecolourlookupid: null,
      haircolourlookupid: null,
      titlelookupid: null,
      genderlookupid: null,
      nationalitylookupid: null,
      placeofbirthlookupid: null,
      cityofbirth: "",
      countryofbirthlookupid: null,
      birthdate: "",
      maritalstatuslookupid: null,
      divorcedate: "",
      isadopted: false,
      specialmarks: "",
      halfcast: false,
      personstatuslookupid: null,
      contactnumber: "",
      whatsappavailable: false,
      alternatecontactnumber: "",
      emailaddress: "",
      alternateemailaddress: "",
      mainregionlookupid: null,
      mainzonelookupid: null,
      mainworedalookupid: null,
      mainkebelelookupid: null,
      maincity: "",
      mainaddressline1: "",
      alternateregionlookupid: null,
      alternatezonelookupid: null,
      alternateworedalookupid: null,
      alternatekebelelookupid: null,
      alternatecity: "",
      alternateaddressline1: "",
      deliveryregionlookupid: null,
      deliveryzonelookupid: null,
      deliveryworedalookupid: null,
      deliverykebelelookupid: null,
      deliverycity: "",
      deliveryaddressline1: "",
      abroadcountrylookupid: null,
      abroadaddressline1: "",
      abroadcontactnumber: "",
      securityquestionlookupid1: null,
      securityanswer1: "",
      securityquestionlookupid2: null,
      securityanswer2: "",
      watchliststatuslookupid: null,
    });
    setChangeTracker(null);
  };

  const handleFieldChange = (
    field: string,
    value: string | number | boolean | null,
  ) => {
    // Update form state
    setEditForm((prev) => ({
      ...prev,
      [field]: value,
    }));

    // Track the change
    if (changeTracker) {
      changeTracker.updateField(field, value);
    }
  };

  const isFieldChanged = (field: string): boolean => {
    return changeTracker
      ? changeTracker.getChangedFields().includes(field)
      : false;
  };

  const MetaStrip = () => (
    <div className="flex items-center gap-6 text-sm text-gray-500 mb-6">
      <div className="flex items-center gap-2">
        <Hash className="w-4 h-4" />
        <span>ID: {personData?.tgpersonid}</span>
      </div>
      <div className="flex items-center gap-2">
        <Calendar className="w-4 h-4" />
        <span>
          Created:{" "}
          {personData?.createdate ? formatDate(personData.createdate) : "N/A"}
        </span>
      </div>
      <div className="flex items-center gap-2">
        <Clock className="w-4 h-4" />
        <span>
          Modified:{" "}
          {personData?.modifieddate
            ? formatDate(personData.modifieddate)
            : "N/A"}
        </span>
      </div>
      <div className="flex items-center gap-2">
        <span
          className={cn(
            "px-2 py-1 text-xs font-medium rounded-full",
            personData?.isactive
              ? "bg-green-100 text-green-800"
              : "bg-gray-100 text-gray-800",
          )}
        >
          {personData?.isactive ? "Active" : "Inactive"}
        </span>
      </div>
    </div>
  );

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="flex items-center gap-3">
          <Loader2 className="w-6 h-6 animate-spin text-blue-600" />
          <span className="text-gray-600">Loading person data...</span>
        </div>
      </div>
    );
  }

  if (error || !personData) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-center">
          <AlertCircle className="w-12 h-12 text-red-500 mx-auto mb-4" />
          <h3 className="text-lg font-semibold text-gray-900 mb-2">
            Failed to Load Person
          </h3>
          <p className="text-gray-600">{error || "Person data not found"}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <MetaStrip />

      {/* Basic Information */}
      <div className="bg-white rounded-lg border border-gray-200 p-6">
        <div className="flex items-center justify-between mb-6">
          <h3 className="text-lg font-semibold text-gray-900 flex items-center gap-2">
            <User className="w-5 h-5" />
            Basic Information
          </h3>
          <div className="flex gap-2">
            {isEditing ? (
              <>
                <button
                  onClick={handleCancelEdit}
                  className="compact-button border"
                  disabled={isSubmitting}
                >
                  Cancel
                </button>
                <button
                  onClick={handleSaveEdit}
                  className={cn(
                    "compact-button",
                    changeTracker?.hasChanges()
                      ? "bg-primary text-white"
                      : "bg-gray-100 text-gray-400 cursor-not-allowed",
                  )}
                  disabled={isSubmitting || !changeTracker?.hasChanges()}
                  title={
                    changeTracker?.hasChanges()
                      ? `Update: ${changeTracker.getChangedFields().join(", ")}`
                      : "No changes to save"
                  }
                >
                  {isSubmitting ? (
                    <>
                      <Loader2 className="w-3 h-3 mr-1 animate-spin" />
                      Saving...
                    </>
                  ) : (
                    <>
                      <Save className="w-3 h-3 mr-1" />
                      {changeTracker?.hasChanges()
                        ? `Save (${changeTracker.getChangedFields().length})`
                        : "Save"}
                    </>
                  )}
                </button>
              </>
            ) : (
              <button
                onClick={handleStartEdit}
                className="compact-button border flex items-center gap-1"
              >
                <Edit2 className="w-3 h-3" />
                Edit Person Details
              </button>
            )}
          </div>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2 flex items-center gap-2">
              First Name
              {isFieldChanged("firstname") && (
                <span className="w-2 h-2 bg-orange-500 rounded-full animate-pulse" />
              )}
            </label>
            {isEditing ? (
              <div className="space-y-2">
                <input
                  type="text"
                  value={editForm.firstname}
                  onChange={(e) =>
                    handleFieldChange("firstname", e.target.value)
                  }
                  className={cn(
                    "w-full p-2 text-sm border rounded",
                    isFieldChanged("firstname")
                      ? "border-orange-300 bg-orange-50 ring-1 ring-orange-200"
                      : "border-gray-300",
                  )}
                  placeholder="Enter first name..."
                />
                <input
                  type="text"
                  value={editForm.firstnamesamharic}
                  onChange={(e) =>
                    handleFieldChange("firstnamesamharic", e.target.value)
                  }
                  className={cn(
                    "w-full p-2 text-sm border rounded",
                    isFieldChanged("firstnamesamharic")
                      ? "border-orange-300 bg-orange-50 ring-1 ring-orange-200"
                      : "border-gray-300",
                  )}
                  placeholder="Enter first name in Amharic..."
                  dir="rtl"
                />
              </div>
            ) : (
              <>
                <div className="text-base font-medium text-gray-900">
                  {personData.firstname || "N/A"}
                </div>
                {personData.firstnamesamharic && (
                  <div className="text-sm text-gray-600 mt-1">
                    Amharic: {personData.firstnamesamharic}
                  </div>
                )}
              </>
            )}
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2 flex items-center gap-2">
              Middle Name
              {isFieldChanged("middlename") && (
                <span className="w-2 h-2 bg-orange-500 rounded-full animate-pulse" />
              )}
            </label>
            {isEditing ? (
              <div className="space-y-2">
                <input
                  type="text"
                  value={editForm.middlename}
                  onChange={(e) =>
                    handleFieldChange("middlename", e.target.value)
                  }
                  className={cn(
                    "w-full p-2 text-sm border rounded",
                    isFieldChanged("middlename")
                      ? "border-orange-300 bg-orange-50 ring-1 ring-orange-200"
                      : "border-gray-300",
                  )}
                  placeholder="Enter middle name..."
                />
                <input
                  type="text"
                  value={editForm.middlenameamharic}
                  onChange={(e) =>
                    handleFieldChange("middlenameamharic", e.target.value)
                  }
                  className={cn(
                    "w-full p-2 text-sm border rounded",
                    isFieldChanged("middlenameamharic")
                      ? "border-orange-300 bg-orange-50 ring-1 ring-orange-200"
                      : "border-gray-300",
                  )}
                  placeholder="Enter middle name in Amharic..."
                  dir="rtl"
                />
              </div>
            ) : (
              <>
                <div className="text-base text-gray-900">
                  {personData.middlename || "N/A"}
                </div>
                {personData.middlenameamharic && (
                  <div className="text-sm text-gray-600 mt-1">
                    Amharic: {personData.middlenameamharic}
                  </div>
                )}
              </>
            )}
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2 flex items-center gap-2">
              Surname
              {isFieldChanged("surname") && (
                <span className="w-2 h-2 bg-orange-500 rounded-full animate-pulse" />
              )}
            </label>
            {isEditing ? (
              <div className="space-y-2">
                <input
                  type="text"
                  value={editForm.surname}
                  onChange={(e) => handleFieldChange("surname", e.target.value)}
                  className={cn(
                    "w-full p-2 text-sm border rounded",
                    isFieldChanged("surname")
                      ? "border-orange-300 bg-orange-50 ring-1 ring-orange-200"
                      : "border-gray-300",
                  )}
                  placeholder="Enter surname..."
                />
                <input
                  type="text"
                  value={editForm.surnameamharic}
                  onChange={(e) =>
                    handleFieldChange("surnameamharic", e.target.value)
                  }
                  className={cn(
                    "w-full p-2 text-sm border rounded",
                    isFieldChanged("surnameamharic")
                      ? "border-orange-300 bg-orange-50 ring-1 ring-orange-200"
                      : "border-gray-300",
                  )}
                  placeholder="Enter surname in Amharic..."
                  dir="rtl"
                />
              </div>
            ) : (
              <>
                <div className="text-base font-medium text-gray-900">
                  {personData.surname || "N/A"}
                </div>
                {personData.surnameamharic && (
                  <div className="text-sm text-gray-600 mt-1">
                    Amharic: {personData.surnameamharic}
                  </div>
                )}
              </>
            )}
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2 flex items-center gap-2">
              Initials
              {isFieldChanged("initials") && (
                <span className="w-2 h-2 bg-orange-500 rounded-full animate-pulse" />
              )}
            </label>
            {isEditing ? (
              <input
                type="text"
                value={editForm.initials}
                onChange={(e) => handleFieldChange("initials", e.target.value)}
                className={cn(
                  "w-full p-2 text-sm border rounded",
                  isFieldChanged("initials")
                    ? "border-orange-300 bg-orange-50 ring-1 ring-orange-200"
                    : "border-gray-300",
                )}
                placeholder="Enter initials..."
              />
            ) : (
              <div className="text-base text-gray-900">
                {personData.initials || "N/A"}
              </div>
            )}
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2 flex items-center gap-2">
              Gender
              {isFieldChanged("genderlookupid") && (
                <span className="w-2 h-2 bg-orange-500 rounded-full animate-pulse" />
              )}
            </label>
            {isEditing ? (
              <SearchableSelect
                options={genderOptions}
                value={editForm.genderlookupid}
                onChange={(value) => handleFieldChange("genderlookupid", value)}
                placeholder="Select gender..."
                className={cn(
                  "w-full",
                  isFieldChanged("genderlookupid")
                    ? "border-orange-300 bg-orange-50 ring-1 ring-orange-200"
                    : "",
                )}
                loading={loadingOptions}
              />
            ) : (
              <div className="text-base text-gray-900">
                <LookupField
                  lookupId={personData.genderlookupid}
                  format="value"
                  fallback="N/A"
                />
              </div>
            )}
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Title
            </label>
            <div className="text-base text-gray-900">
              <LookupField
                lookupId={personData.titlelookupid}
                format="value"
                fallback="N/A"
              />
            </div>
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2 flex items-center gap-2">
              Birth Date
              {isFieldChanged("birthdate") && (
                <span className="w-2 h-2 bg-orange-500 rounded-full animate-pulse" />
              )}
            </label>
            {isEditing ? (
              <input
                type="date"
                value={editForm.birthdate}
                onChange={(e) => handleFieldChange("birthdate", e.target.value)}
                className={cn(
                  "w-full p-2 text-sm border rounded",
                  isFieldChanged("birthdate")
                    ? "border-orange-300 bg-orange-50 ring-1 ring-orange-200"
                    : "border-gray-300",
                )}
              />
            ) : (
              <div className="text-base text-gray-900">
                {personData.birthdate
                  ? formatDate(personData.birthdate)
                  : "N/A"}
              </div>
            )}
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2 flex items-center gap-2">
              Nationality
              {isFieldChanged("nationalitylookupid") && (
                <span className="w-2 h-2 bg-orange-500 rounded-full animate-pulse" />
              )}
            </label>
            {isEditing ? (
              <SearchableSelect
                options={nationalityOptions}
                value={editForm.nationalitylookupid}
                onChange={(value) =>
                  handleFieldChange("nationalitylookupid", value)
                }
                placeholder="Select nationality..."
                className={cn(
                  "w-full",
                  isFieldChanged("nationalitylookupid")
                    ? "border-orange-300 bg-orange-50 ring-1 ring-orange-200"
                    : "",
                )}
                loading={loadingOptions}
              />
            ) : (
              <div className="text-base text-gray-900">
                <LookupField
                  lookupId={personData.nationalitylookupid}
                  format="value"
                  fallback="N/A"
                />
              </div>
            )}
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2 flex items-center gap-2">
              Marital Status
              {isFieldChanged("maritalstatuslookupid") && (
                <span className="w-2 h-2 bg-orange-500 rounded-full animate-pulse" />
              )}
            </label>
            {isEditing ? (
              <SearchableSelect
                options={maritalStatusOptions}
                value={editForm.maritalstatuslookupid}
                onChange={(value) =>
                  handleFieldChange("maritalstatuslookupid", value)
                }
                placeholder="Select marital status..."
                className={cn(
                  "w-full",
                  isFieldChanged("maritalstatuslookupid")
                    ? "border-orange-300 bg-orange-50 ring-1 ring-orange-200"
                    : "",
                )}
                loading={loadingOptions}
              />
            ) : (
              <div className="text-base text-gray-900">
                <LookupField
                  lookupId={personData.maritalstatuslookupid}
                  format="value"
                  fallback="N/A"
                />
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Physical Characteristics */}
      <div className="bg-white rounded-lg border border-gray-200 p-6">
        <h3 className="text-lg font-semibold text-gray-900 mb-6 flex items-center gap-2">
          <Eye className="w-5 h-5" />
          Physical Characteristics
        </h3>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2 flex items-center gap-2">
              Height (cm)
              {isFieldChanged("height") && (
                <span className="w-2 h-2 bg-orange-500 rounded-full animate-pulse" />
              )}
            </label>
            {isEditing ? (
              <input
                type="number"
                value={editForm.height}
                onChange={(e) => handleFieldChange("height", e.target.value)}
                className={cn(
                  "w-full p-2 text-sm border rounded",
                  isFieldChanged("height")
                    ? "border-orange-300 bg-orange-50 ring-1 ring-orange-200"
                    : "border-gray-300",
                )}
                placeholder="Enter height in centimeters..."
                min="0"
                max="300"
              />
            ) : (
              <div className="text-base text-gray-900">
                {personData.height ? `${personData.height} cm` : "N/A"}
              </div>
            )}
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2 flex items-center gap-2">
              Eye Color
              {isFieldChanged("eyecolourlookupid") && (
                <span className="w-2 h-2 bg-orange-500 rounded-full animate-pulse" />
              )}
            </label>
            {isEditing ? (
              <SearchableSelect
                options={eyeColorOptions}
                value={editForm.eyecolourlookupid}
                onChange={(value) =>
                  handleFieldChange("eyecolourlookupid", value)
                }
                placeholder="Select eye color..."
                className={cn(
                  "w-full",
                  isFieldChanged("eyecolourlookupid")
                    ? "border-orange-300 bg-orange-50 ring-1 ring-orange-200"
                    : "",
                )}
                loading={loadingOptions}
              />
            ) : (
              <div className="text-base text-gray-900">
                <LookupField
                  lookupId={personData.eyecolourlookupid}
                  format="value"
                  fallback="N/A"
                />
              </div>
            )}
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2 flex items-center gap-2">
              Hair Color
              {isFieldChanged("haircolourlookupid") && (
                <span className="w-2 h-2 bg-orange-500 rounded-full animate-pulse" />
              )}
            </label>
            {isEditing ? (
              <SearchableSelect
                options={hairColorOptions}
                value={editForm.haircolourlookupid}
                onChange={(value) =>
                  handleFieldChange("haircolourlookupid", value)
                }
                placeholder="Select hair color..."
                className={cn(
                  "w-full",
                  isFieldChanged("haircolourlookupid")
                    ? "border-orange-300 bg-orange-50 ring-1 ring-orange-200"
                    : "",
                )}
                loading={loadingOptions}
              />
            ) : (
              <div className="text-base text-gray-900">
                <LookupField
                  lookupId={personData.haircolourlookupid}
                  format="value"
                  fallback="N/A"
                />
              </div>
            )}
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Special Marks
            </label>
            <div className="text-base text-gray-900">
              {personData.specialmarks || "N/A"}
            </div>
          </div>

          {personData.isadopted && (
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-2">
                Adoption Status
              </label>
              <div className="text-base text-green-700 font-medium">
                Adopted
              </div>
            </div>
          )}

          {personData.halfcast && (
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-2">
                Mixed Heritage
              </label>
              <div className="text-base text-blue-700 font-medium">Yes</div>
            </div>
          )}
        </div>
      </div>

      {/* Contact Information */}
      <div className="bg-white rounded-lg border border-gray-200 p-6">
        <h3 className="text-lg font-semibold text-gray-900 mb-6 flex items-center gap-2">
          <Phone className="w-5 h-5" />
          Contact Information
        </h3>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2 flex items-center gap-2">
              Contact Number
              {isFieldChanged("contactnumber") && (
                <span className="w-2 h-2 bg-orange-500 rounded-full animate-pulse" />
              )}
            </label>
            {isEditing ? (
              <input
                type="tel"
                value={editForm.contactnumber}
                onChange={(e) =>
                  handleFieldChange("contactnumber", e.target.value)
                }
                className={cn(
                  "w-full p-2 text-sm border rounded",
                  isFieldChanged("contactnumber")
                    ? "border-orange-300 bg-orange-50 ring-1 ring-orange-200"
                    : "border-gray-300",
                )}
                placeholder="Enter contact number..."
              />
            ) : (
              <div className="text-base text-gray-900">
                {personData.contactnumber || "N/A"}
              </div>
            )}
            {/* WhatsApp Available checkbox */}
            <div className="mt-2">
              <label className="flex items-center gap-2 text-sm">
                {isEditing ? (
                  <input
                    type="checkbox"
                    checked={editForm.whatsappavailable}
                    onChange={(e) =>
                      handleFieldChange("whatsappavailable", e.target.checked)
                    }
                    className="w-4 h-4 text-blue-600 border-gray-300 rounded focus:ring-blue-500"
                  />
                ) : (
                  <span
                    className={cn(
                      "w-4 h-4 rounded border-2 flex items-center justify-center",
                      personData.whatsappavailable
                        ? "bg-green-100 border-green-500 text-green-600"
                        : "bg-gray-100 border-gray-300",
                    )}
                  >
                    {personData.whatsappavailable && "✓"}
                  </span>
                )}
                <span
                  className={cn(
                    personData.whatsappavailable
                      ? "text-green-600 font-medium"
                      : "text-gray-500",
                  )}
                >
                  WhatsApp Available
                  {isFieldChanged("whatsappavailable") && (
                    <span className="w-2 h-2 bg-orange-500 rounded-full animate-pulse inline-block ml-2" />
                  )}
                </span>
              </label>
            </div>
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2 flex items-center gap-2">
              Alternate Contact Number
              {isFieldChanged("alternatecontactnumber") && (
                <span className="w-2 h-2 bg-orange-500 rounded-full animate-pulse" />
              )}
            </label>
            {isEditing ? (
              <input
                type="tel"
                value={editForm.alternatecontactnumber}
                onChange={(e) =>
                  handleFieldChange("alternatecontactnumber", e.target.value)
                }
                className={cn(
                  "w-full p-2 text-sm border rounded",
                  isFieldChanged("alternatecontactnumber")
                    ? "border-orange-300 bg-orange-50 ring-1 ring-orange-200"
                    : "border-gray-300",
                )}
                placeholder="Enter alternate contact number..."
              />
            ) : (
              <div className="text-base text-gray-900 font-mono">
                {personData.alternatecontactnumber || "N/A"}
              </div>
            )}
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2 flex items-center gap-2">
              Email Address
              {isFieldChanged("emailaddress") && (
                <span className="w-2 h-2 bg-orange-500 rounded-full animate-pulse" />
              )}
            </label>
            {isEditing ? (
              <input
                type="email"
                value={editForm.emailaddress}
                onChange={(e) =>
                  handleFieldChange("emailaddress", e.target.value)
                }
                className={cn(
                  "w-full p-2 text-sm border rounded",
                  isFieldChanged("emailaddress")
                    ? "border-orange-300 bg-orange-50 ring-1 ring-orange-200"
                    : "border-gray-300",
                )}
                placeholder="Enter email address..."
              />
            ) : (
              <div className="text-base text-gray-900">
                {personData.emailaddress || "N/A"}
              </div>
            )}
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2 flex items-center gap-2">
              Alternate Email Address
              {isFieldChanged("alternateemailaddress") && (
                <span className="w-2 h-2 bg-orange-500 rounded-full animate-pulse" />
              )}
            </label>
            {isEditing ? (
              <input
                type="email"
                value={editForm.alternateemailaddress}
                onChange={(e) =>
                  handleFieldChange("alternateemailaddress", e.target.value)
                }
                className={cn(
                  "w-full p-2 text-sm border rounded",
                  isFieldChanged("alternateemailaddress")
                    ? "border-orange-300 bg-orange-50 ring-1 ring-orange-200"
                    : "border-gray-300",
                )}
                placeholder="Enter alternate email address..."
              />
            ) : (
              <div className="text-base text-gray-900">
                {personData.alternateemailaddress || "N/A"}
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Address Information */}
      <div className="bg-white rounded-lg border border-gray-200 p-6">
        <h3 className="text-lg font-semibold text-gray-900 mb-6 flex items-center gap-2">
          <MapPin className="w-5 h-5" />
          Address Information
        </h3>

        <div className="space-y-6">
          {/* Main Address */}
          <div>
            <h4 className="text-base font-semibold text-gray-900 mb-4">
              Main Address
            </h4>
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1">
                  Region
                </label>
                <div className="text-sm text-gray-900">
                  <LookupField
                    lookupId={personData.mainregionlookupid}
                    format="value"
                    fallback="N/A"
                  />
                </div>
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1">
                  Zone
                </label>
                <div className="text-sm text-gray-900">
                  <LookupField
                    lookupId={personData.mainzonelookupid}
                    format="value"
                    fallback="N/A"
                  />
                </div>
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1">
                  Woreda
                </label>
                <div className="text-sm text-gray-900">
                  <LookupField
                    lookupId={personData.mainworedalookupid}
                    format="value"
                    fallback="N/A"
                  />
                </div>
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1">
                  Kebele
                </label>
                <div className="text-sm text-gray-900">
                  <LookupField
                    lookupId={personData.mainkebelelookupid}
                    format="value"
                    fallback="N/A"
                  />
                </div>
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1">
                  City
                </label>
                <div className="text-sm text-gray-900">
                  {personData.maincity || "N/A"}
                </div>
              </div>
              <div className="md:col-span-3">
                <label className="block text-sm font-medium text-gray-700 mb-1 flex items-center gap-2">
                  Address Line
                  {isFieldChanged("mainaddressline1") && (
                    <span className="w-2 h-2 bg-orange-500 rounded-full animate-pulse" />
                  )}
                </label>
                {isEditing ? (
                  <input
                    type="text"
                    value={editForm.mainaddressline1}
                    onChange={(e) =>
                      handleFieldChange("mainaddressline1", e.target.value)
                    }
                    className={cn(
                      "w-full p-2 text-sm border rounded",
                      isFieldChanged("mainaddressline1")
                        ? "border-orange-300 bg-orange-50 ring-1 ring-orange-200"
                        : "border-gray-300",
                    )}
                    placeholder="Enter main address line..."
                  />
                ) : (
                  <div className="text-sm text-gray-900">
                    {personData.mainaddressline1 || "N/A"}
                  </div>
                )}
              </div>
            </div>
          </div>

          {/* Alternate Address */}
          {(personData.alternateregionlookupid ||
            personData.alternatecity ||
            personData.alternateaddressline1) && (
            <div>
              <h4 className="text-base font-semibold text-gray-900 mb-4">
                Alternate Address
              </h4>
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    Region
                  </label>
                  <div className="text-sm text-gray-900">
                    <LookupField
                      lookupId={personData.alternateregionlookupid}
                      format="value"
                      fallback="N/A"
                    />
                  </div>
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    Zone
                  </label>
                  <div className="text-sm text-gray-900">
                    <LookupField
                      lookupId={personData.alternatezonelookupid}
                      format="value"
                      fallback="N/A"
                    />
                  </div>
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    Woreda
                  </label>
                  <div className="text-sm text-gray-900">
                    <LookupField
                      lookupId={personData.alternateworedalookupid}
                      format="value"
                      fallback="N/A"
                    />
                  </div>
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    Kebele
                  </label>
                  <div className="text-sm text-gray-900">
                    <LookupField
                      lookupId={personData.alternatekebelelookupid}
                      format="value"
                      fallback="N/A"
                    />
                  </div>
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    City
                  </label>
                  <div className="text-sm text-gray-900">
                    {personData.alternatecity || "N/A"}
                  </div>
                </div>
                <div className="md:col-span-3">
                  <label className="block text-sm font-medium text-gray-700 mb-1 flex items-center gap-2">
                    Address Line
                    {isFieldChanged("alternateaddressline1") && (
                      <span className="w-2 h-2 bg-orange-500 rounded-full animate-pulse" />
                    )}
                  </label>
                  {isEditing ? (
                    <input
                      type="text"
                      value={editForm.alternateaddressline1}
                      onChange={(e) =>
                        handleFieldChange(
                          "alternateaddressline1",
                          e.target.value,
                        )
                      }
                      className={cn(
                        "w-full p-2 text-sm border rounded",
                        isFieldChanged("alternateaddressline1")
                          ? "border-orange-300 bg-orange-50 ring-1 ring-orange-200"
                          : "border-gray-300",
                      )}
                      placeholder="Enter alternate address line..."
                    />
                  ) : (
                    <div className="text-sm text-gray-900">
                      {personData.alternateaddressline1 || "N/A"}
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}

          {/* Delivery Address */}
          {(personData.deliveryregionlookupid ||
            personData.deliverycity ||
            personData.deliveryaddressline1) && (
            <div>
              <h4 className="text-base font-semibold text-gray-900 mb-4">
                Delivery Address
              </h4>
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    Region
                  </label>
                  <div className="text-sm text-gray-900">
                    <LookupField
                      lookupId={personData.deliveryregionlookupid}
                      format="value"
                      fallback="N/A"
                    />
                  </div>
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    Zone
                  </label>
                  <div className="text-sm text-gray-900">
                    <LookupField
                      lookupId={personData.deliveryzonelookupid}
                      format="value"
                      fallback="N/A"
                    />
                  </div>
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    Woreda
                  </label>
                  <div className="text-sm text-gray-900">
                    <LookupField
                      lookupId={personData.deliveryworedalookupid}
                      format="value"
                      fallback="N/A"
                    />
                  </div>
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    Kebele
                  </label>
                  <div className="text-sm text-gray-900">
                    <LookupField
                      lookupId={personData.deliverykebelelookupid}
                      format="value"
                      fallback="N/A"
                    />
                  </div>
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    City
                  </label>
                  <div className="text-sm text-gray-900">
                    {personData.deliverycity || "N/A"}
                  </div>
                </div>
                <div className="md:col-span-3">
                  <label className="block text-sm font-medium text-gray-700 mb-1 flex items-center gap-2">
                    Address Line
                    {isFieldChanged("deliveryaddressline1") && (
                      <span className="w-2 h-2 bg-orange-500 rounded-full animate-pulse" />
                    )}
                  </label>
                  {isEditing ? (
                    <input
                      type="text"
                      value={editForm.deliveryaddressline1}
                      onChange={(e) =>
                        handleFieldChange(
                          "deliveryaddressline1",
                          e.target.value,
                        )
                      }
                      className={cn(
                        "w-full p-2 text-sm border rounded",
                        isFieldChanged("deliveryaddressline1")
                          ? "border-orange-300 bg-orange-50 ring-1 ring-orange-200"
                          : "border-gray-300",
                      )}
                      placeholder="Enter delivery address line..."
                    />
                  ) : (
                    <div className="text-sm text-gray-900">
                      {personData.deliveryaddressline1 || "N/A"}
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}

          {/* Abroad Address */}
          {(personData.abroadcountrylookupid ||
            personData.abroadaddressline1 ||
            personData.abroadcontactnumber) && (
            <div>
              <h4 className="text-base font-semibold text-gray-900 mb-4">
                International Address
              </h4>
              <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    Country
                  </label>
                  <div className="text-sm text-gray-900">
                    <LookupField
                      lookupId={personData.abroadcountrylookupid}
                      format="value"
                      fallback="N/A"
                    />
                  </div>
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    Address
                  </label>
                  {isEditing ? (
                    <input
                      type="text"
                      value={editForm.abroadaddressline1}
                      onChange={(e) =>
                        handleFieldChange("abroadaddressline1", e.target.value)
                      }
                      className={cn(
                        "w-full p-2 text-sm border rounded",
                        isFieldChanged("abroadaddressline1")
                          ? "border-orange-300 bg-orange-50 ring-1 ring-orange-200"
                          : "border-gray-300",
                      )}
                      placeholder="Enter abroad address line..."
                    />
                  ) : (
                    <div className="text-sm text-gray-900">
                      {personData.abroadaddressline1 || "N/A"}
                    </div>
                  )}
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    Contact Number
                  </label>
                  <div className="text-sm text-gray-900 font-mono">
                    {personData.abroadcontactnumber || "N/A"}
                  </div>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Security Information */}
      {(personData.securityquestionlookupid1 ||
        personData.securityquestionlookupid2 ||
        personData.watchliststatuslookupid) && (
        <div className="bg-white rounded-lg border border-gray-200 p-6">
          <h3 className="text-lg font-semibold text-gray-900 mb-6 flex items-center gap-2">
            <Shield className="w-5 h-5" />
            Security Information
          </h3>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            {personData.securityquestionlookupid1 && (
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-2">
                  Security Question 1
                </label>
                <div className="text-base text-gray-900">
                  <LookupField
                    lookupId={personData.securityquestionlookupid1}
                    format="value"
                    fallback="N/A"
                  />
                </div>
                <div className="text-sm text-gray-600 mt-1">
                  Answer: {personData.securityanswer1 ? "***" : "N/A"}
                </div>
              </div>
            )}

            {personData.securityquestionlookupid2 && (
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-2">
                  Security Question 2
                </label>
                <div className="text-base text-gray-900">
                  <LookupField
                    lookupId={personData.securityquestionlookupid2}
                    format="value"
                    fallback="N/A"
                  />
                </div>
                <div className="text-sm text-gray-600 mt-1">
                  Answer: {personData.securityanswer2 ? "***" : "N/A"}
                </div>
              </div>
            )}

            {personData.watchliststatuslookupid && (
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-2">
                  Watchlist Status
                </label>
                <div className="text-base text-gray-900">
                  <LookupField
                    lookupId={personData.watchliststatuslookupid}
                    format="value"
                    fallback="N/A"
                  />
                </div>
              </div>
            )}
          </div>
        </div>
      )}

      {/* System Information */}
      <div className="bg-white rounded-lg border border-gray-200 p-6">
        <h3 className="text-lg font-semibold text-gray-900 mb-4 flex items-center gap-2">
          <Flag className="w-5 h-5" />
          System Information
        </h3>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Person Status
            </label>
            <div className="text-base text-gray-900">
              <LookupField
                lookupId={personData.personstatuslookupid}
                format="value"
                fallback="N/A"
              />
            </div>
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Created Date
            </label>
            <div className="text-base text-gray-900">
              {formatDate(personData.createdate)}
            </div>
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Last Modified
            </label>
            <div className="text-base text-gray-900">
              {formatDate(personData.modifieddate)}
            </div>
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Active Status
            </label>
            <span
              className={cn(
                "inline-flex items-center px-3 py-1 text-sm font-medium rounded-full",
                personData.isactive
                  ? "bg-green-100 text-green-800"
                  : "bg-gray-100 text-gray-800",
              )}
            >
              {personData.isactive ? "Active" : "Inactive"}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}
