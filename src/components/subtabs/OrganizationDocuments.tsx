//
//  campus-pilot
//  OrganizationDocuments.tsx
//
//  Created by Ngonidzashe Mangudya on 23/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { useState, useEffect } from "react";
import {
  FileText,
  Plus,
  Edit2,
  Calendar,
  AlertCircle,
  Loader2,
  Eye,
  Download,
  X,
  Files,
  FileCheck,
  FilePlus,
  Archive,
  Building,
  Trash2,
} from "lucide-react";
import { formatDate, formatDateCompact } from "../../lib/utils";
import { cn } from "../../lib/utils";
import { apiClient } from "../../lib/api";
import { SearchableSelect } from "../SearchableSelect";
import { LookupField } from "../LookupField";
import { DocumentViewer } from "../DocumentViewer";
import toast from "react-hot-toast";
import {
  TgOrganisationIssuedDocument,
  TgOrganisationSupportingDocument,
  TgOrganisationVisa,
} from "../../types/organization";

interface OrganizationDocumentsProps {
  organizationId: number;
  applicationId?: number;
  applicationTypeName?: string;
}

interface IssuedDocumentRecord extends TgOrganisationIssuedDocument {
  has_document?: boolean;
}

interface SupportingDocumentRecord extends TgOrganisationSupportingDocument {
  has_document?: boolean;
}

interface VisaDocumentRecord extends TgOrganisationVisa {
  has_document?: boolean;
  tgpersonissueddocument?: {
    documentnumber?: string;
    documentserialnumber?: string;
    issuedate?: Date;
    expirydate?: Date;
  };
}

interface NewIssuedDocumentForm {
  documenttypelookupid: number | null;
  documentstatuslookupid: number | null;
  documentobject: string | null;
  issuedate: string;
  validfromdate: string;
  expirydate: string;
}

interface NewSupportingDocumentForm {
  documenttypelookupid: number | null;
  documentcategorylookupid: number | null;
  documentstatuslookupid: number | null;
  documentobject: string | null;
}

interface NewVisaDocumentForm {
  tgpersonissueddocumentid: number | null;
  visatypelookupid: number | null;
}

interface EditIssuedDocumentForm {
  documenttypelookupid: number | null;
  documentstatuslookupid: number | null;
  documentobject: string | null;
  issuedate: string;
  validfromdate: string;
  expirydate: string;
}

interface EditSupportingDocumentForm {
  documenttypelookupid: number | null;
  documentcategorylookupid: number | null;
  documentstatuslookupid: number | null;
  documentobject: string | null;
}

interface EditVisaDocumentForm {
  tgpersonissueddocumentid: number | null;
  visatypelookupid: number | null;
}

// MetaStrip component for displaying record metadata
function MetaStrip({ record }: { record: any }) {
  return (
    <div className="flex items-center gap-3 text-[10px] text-gray-500">
      <span className="font-medium">
        ID:{" "}
        {record.tgorganisationissueddocumentid ||
          record.tgorganisationsupportingdocumentid ||
          record.tgorganisationvisaid}
      </span>
      <span className="text-gray-400">•</span>
      <span className="font-medium">{formatDate(record.createdate)}</span>
      <span className="text-gray-400">•</span>
      <span className="font-medium">
        {record.modifieddate ? formatDate(record.modifieddate) : "-"}
      </span>
      <span className="text-gray-400">•</span>
      <span
        className={cn(
          "px-1.5 py-0.5 rounded text-[9px] font-medium",
          record.isactive === 1
            ? "bg-green-100 text-green-700"
            : "bg-gray-100 text-gray-600",
        )}
      >
        {record.isactive === 1 ? "Active" : "Voided"}
      </span>
    </div>
  );
}

export function OrganizationDocuments({
  organizationId,
  applicationId,
  applicationTypeName,
}: OrganizationDocumentsProps) {
  // Tab state
  const [activeTab, setActiveTab] = useState<"issued" | "supporting" | "visa">(
    "issued",
  );

  // Issued documents state
  const [issuedDocuments, setIssuedDocuments] = useState<
    IssuedDocumentRecord[]
  >([]);
  const [issuedLoading, setIssuedLoading] = useState(true);
  const [issuedError, setIssuedError] = useState<string | null>(null);

  // Supporting documents state
  const [supportingDocuments, setSupportingDocuments] = useState<
    SupportingDocumentRecord[]
  >([]);
  const [supportingLoading, setSupportingLoading] = useState(true);
  const [supportingError, setSupportingError] = useState<string | null>(null);

  // Visa documents state
  const [visaDocuments, setVisaDocuments] = useState<VisaDocumentRecord[]>([]);
  const [visaLoading, setVisaLoading] = useState(true);
  const [visaError, setVisaError] = useState<string | null>(null);

  // Edit states
  const [editingIssuedDocument, setEditingIssuedDocument] = useState<
    number | null
  >(null);
  const [editingSupportingDocument, setEditingSupportingDocument] = useState<
    number | null
  >(null);
  const [editingVisaDocument, setEditingVisaDocument] = useState<number | null>(
    null,
  );
  const [isSubmitting, setIsSubmitting] = useState(false);

  // Lookup options
  const [documentTypeOptions, setDocumentTypeOptions] = useState<any[]>([]);
  const [supportingDocumentTypeOptions, setSupportingDocumentTypeOptions] =
    useState<any[]>([]);
  const [documentCategoryOptions, setDocumentCategoryOptions] = useState<any[]>(
    [],
  );
  const [statusOptions, setStatusOptions] = useState<any[]>([]);
  const [supportingStatusOptions, setSupportingStatusOptions] = useState<any[]>(
    [],
  );
  const [countryOptions, setCountryOptions] = useState<any[]>([]);
  const [visaTypeOptions, setVisaTypeOptions] = useState<any[]>([]);
  const [personDocumentOptions, setPersonDocumentOptions] = useState<any[]>([]);
  const [loadingOptions, setLoadingOptions] = useState(true);

  // Document viewing
  const [viewingDocument, setViewingDocument] = useState<number | null>(null);
  const [documentData, setDocumentData] = useState<string | null>(null);
  const [loadingDocument, setLoadingDocument] = useState(false);

  // Forms
  const [newIssuedDocumentForm, setNewIssuedDocumentForm] =
    useState<NewIssuedDocumentForm>({
      documenttypelookupid: null,
      documentstatuslookupid: null,
      documentobject: null,
      issuedate: "",
      validfromdate: "",
      expirydate: "",
    });

  const [newSupportingDocumentForm, setNewSupportingDocumentForm] =
    useState<NewSupportingDocumentForm>({
      documenttypelookupid: null,
      documentcategorylookupid: null,
      documentstatuslookupid: null,
      documentobject: null,
    });

  const [newVisaDocumentForm, setNewVisaDocumentForm] =
    useState<NewVisaDocumentForm>({
      tgpersonissueddocumentid: null,
      visatypelookupid: null,
    });

  const [editIssuedForm, setEditIssuedForm] = useState<EditIssuedDocumentForm>({
    documenttypelookupid: null,
    documentstatuslookupid: null,
    documentobject: null,
    issuedate: "",
    validfromdate: "",
    expirydate: "",
  });

  const [editSupportingForm, setEditSupportingForm] =
    useState<EditSupportingDocumentForm>({
      documenttypelookupid: null,
      documentcategorylookupid: null,
      documentstatuslookupid: null,
      documentobject: null,
    });

  const [editVisaForm, setEditVisaForm] = useState<EditVisaDocumentForm>({
    tgpersonissueddocumentid: null,
    visatypelookupid: null,
  });

  // Modal states
  const [showNewIssuedModal, setShowNewIssuedModal] = useState(false);
  const [showNewSupportingModal, setShowNewSupportingModal] = useState(false);
  const [showNewVisaModal, setShowNewVisaModal] = useState(false);

  // Fetch issued documents
  useEffect(() => {
    const fetchIssuedDocuments = async () => {
      try {
        setIssuedLoading(true);
        setIssuedError(null);

        const data = await apiClient.getOrganizationIssuedDocuments(
          organizationId,
          applicationId,
        );
        const processedData = data.map((doc: any) => ({
          ...doc,
          issuedate: doc.issuedate ? new Date(doc.issuedate) : undefined,
          validfromdate: doc.validfromdate
            ? new Date(doc.validfromdate)
            : undefined,
          expirydate: doc.expirydate ? new Date(doc.expirydate) : undefined,
          createdate: new Date(doc.createdate),
          modifieddate: doc.modifieddate
            ? new Date(doc.modifieddate)
            : undefined,
        }));
        setIssuedDocuments(processedData);
      } catch (err) {
        const errorMessage =
          err instanceof Error
            ? err.message
            : "Failed to load issued documents";
        setIssuedError(errorMessage);
        toast.error(errorMessage);
      } finally {
        setIssuedLoading(false);
      }
    };

    fetchIssuedDocuments();
  }, [organizationId, applicationId]);

  // Fetch supporting documents
  useEffect(() => {
    const fetchSupportingDocuments = async () => {
      try {
        setSupportingLoading(true);
        setSupportingError(null);

        const data = await apiClient.getOrganizationSupportingDocuments(
          organizationId,
          applicationId,
        );
        const processedData = data.map((doc: any) => ({
          ...doc,
          createdate: new Date(doc.createdate),
          modifieddate: doc.modifieddate
            ? new Date(doc.modifieddate)
            : undefined,
        }));
        setSupportingDocuments(processedData);
      } catch (err) {
        const errorMessage =
          err instanceof Error
            ? err.message
            : "Failed to load supporting documents";
        setSupportingError(errorMessage);
        toast.error(errorMessage);
      } finally {
        setSupportingLoading(false);
      }
    };

    fetchSupportingDocuments();
  }, [organizationId, applicationId]);

  // Fetch visa documents
  useEffect(() => {
    const fetchVisaDocuments = async () => {
      try {
        setVisaLoading(true);
        setVisaError(null);

        const data = await apiClient.getOrganizationVisaDocuments(
          organizationId,
          applicationId,
        );
        const processedData = data.map((doc: any) => ({
          ...doc,
          createdate: new Date(doc.createdate),
          modifieddate: doc.modifieddate
            ? new Date(doc.modifieddate)
            : undefined,
        }));
        setVisaDocuments(processedData);
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to load visa documents";
        setVisaError(errorMessage);
        toast.error(errorMessage);
      } finally {
        setVisaLoading(false);
      }
    };

    fetchVisaDocuments();
  }, [organizationId, applicationId]);

  // Fetch lookup options
  useEffect(() => {
    const fetchLookupOptions = async () => {
      try {
        setLoadingOptions(true);
        const [
          documentTypeData,
          supportingDocumentTypeData,
          documentCategoryData,
          statusData,
          supportingStatusData,
          countryData,
          visaTypeData,
        ] = await Promise.all([
          apiClient.getLookupsByType("PERSONALIZED_DOCUMENT_TYPE"),
          apiClient.getLookupsByType("SUPPORTING_DOCUMENT_TYPE"),
          apiClient.getLookupsByType("DOCUMENT_CATEGORY"),
          apiClient.getLookupsByType("PERSONALIZED_DOCUMENT_STATUS"),
          apiClient.getLookupsByType("SUPPORTING_DOCUMENT_STATUS"),
          apiClient.getLookupsByType("COUNTRY"),
          apiClient.getLookupsByType("VISA_TYPE"),
        ]);

        setDocumentTypeOptions(
          documentTypeData.map((lookup) => ({
            id: lookup.tglookupid,
            value: lookup.lookupvalue,
            label: lookup.lookupdescription || lookup.lookupvalue,
          })),
        );
        setSupportingDocumentTypeOptions(
          supportingDocumentTypeData.map((lookup) => ({
            id: lookup.tglookupid,
            value: lookup.lookupvalue,
            label: lookup.lookupdescription || lookup.lookupvalue,
          })),
        );
        setDocumentCategoryOptions(
          documentCategoryData.map((lookup) => ({
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
        setSupportingStatusOptions(
          supportingStatusData.map((lookup) => ({
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
        setVisaTypeOptions(
          visaTypeData.map((lookup) => ({
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

  // Create handlers
  const handleCreateIssuedDocument = async () => {
    if (!newIssuedDocumentForm.documenttypelookupid) {
      toast.error("Please select a document type");
      return;
    }

    setIsSubmitting(true);
    try {
      const createData = {
        tgorganisationid: organizationId,
        tgapplicationid: applicationId,
        ...newIssuedDocumentForm,
        issuedate: newIssuedDocumentForm.issuedate || null,
        validfromdate: newIssuedDocumentForm.validfromdate || null,
        expirydate: newIssuedDocumentForm.expirydate || null,
      };

      await apiClient.createOrganizationIssuedDocument(createData);
      toast.success("Issued document created successfully");
      setShowNewIssuedModal(false);

      // Reset form
      setNewIssuedDocumentForm({
        documenttypelookupid: null,
        documentstatuslookupid: null,
        documentobject: null,
        issuedate: "",
        validfromdate: "",
        expirydate: "",
      });

      // Refresh documents
      const data = await apiClient.getOrganizationIssuedDocuments(
        organizationId,
        applicationId,
      );
      const processedData = data.map((doc: any) => ({
        ...doc,
        issuedate: doc.issuedate ? new Date(doc.issuedate) : undefined,
        validfromdate: doc.validfromdate
          ? new Date(doc.validfromdate)
          : undefined,
        expirydate: doc.expirydate ? new Date(doc.expirydate) : undefined,
        createdate: new Date(doc.createdate),
        modifieddate: doc.modifieddate ? new Date(doc.modifieddate) : undefined,
      }));
      setIssuedDocuments(processedData);
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : "Failed to create document";
      toast.error(errorMessage);
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleCreateSupportingDocument = async () => {
    if (!newSupportingDocumentForm.documenttypelookupid) {
      toast.error("Please select a document type");
      return;
    }

    setIsSubmitting(true);
    try {
      const createData = {
        tgorganisationid: organizationId,
        tgapplicationid: applicationId,
        ...newSupportingDocumentForm,
      };

      await apiClient.createOrganizationSupportingDocument(createData);
      toast.success("Supporting document created successfully");
      setShowNewSupportingModal(false);

      // Reset form
      setNewSupportingDocumentForm({
        documenttypelookupid: null,
        documentcategorylookupid: null,
        documentstatuslookupid: null,
        documentobject: null,
      });

      // Refresh documents
      const data = await apiClient.getOrganizationSupportingDocuments(
        organizationId,
        applicationId,
      );
      const processedData = data.map((doc: any) => ({
        ...doc,
        createdate: new Date(doc.createdate),
        modifieddate: doc.modifieddate ? new Date(doc.modifieddate) : undefined,
      }));
      setSupportingDocuments(processedData);
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : "Failed to create document";
      toast.error(errorMessage);
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleCreateVisaDocument = async () => {
    if (
      !newVisaDocumentForm.tgpersonissueddocumentid ||
      !newVisaDocumentForm.visatypelookupid
    ) {
      toast.error("Please select both person document and visa type");
      return;
    }

    setIsSubmitting(true);
    try {
      const createData = {
        tgorganisationid: organizationId,
        tgapplicationid: applicationId,
        ...newVisaDocumentForm,
      };

      await apiClient.createOrganizationVisaDocument(createData);
      toast.success("Visa document created successfully");
      setShowNewVisaModal(false);

      // Reset form
      setNewVisaDocumentForm({
        tgpersonissueddocumentid: null,
        visatypelookupid: null,
      });

      // Refresh documents
      const data = await apiClient.getOrganizationVisaDocuments(
        organizationId,
        applicationId,
      );
      const processedData = data.map((doc: any) => ({
        ...doc,
        createdate: new Date(doc.createdate),
        modifieddate: doc.modifieddate ? new Date(doc.modifieddate) : undefined,
      }));
      setVisaDocuments(processedData);
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : "Failed to create document";
      toast.error(errorMessage);
    } finally {
      setIsSubmitting(false);
    }
  };

  // Edit handlers
  const handleEditIssuedDocument = async () => {
    if (!editingIssuedDocument) return;

    setIsSubmitting(true);
    try {
      const updateData = {
        ...editIssuedForm,
        issuedate: editIssuedForm.issuedate || null,
        validfromdate: editIssuedForm.validfromdate || null,
        expirydate: editIssuedForm.expirydate || null,
      };

      await apiClient.updateOrganizationIssuedDocument(
        editingIssuedDocument,
        updateData,
      );
      toast.success("Document updated successfully");
      setEditingIssuedDocument(null);

      // Refresh documents
      const data = await apiClient.getOrganizationIssuedDocuments(
        organizationId,
        applicationId,
      );
      const processedData = data.map((doc: any) => ({
        ...doc,
        issuedate: doc.issuedate ? new Date(doc.issuedate) : undefined,
        validfromdate: doc.validfromdate
          ? new Date(doc.validfromdate)
          : undefined,
        expirydate: doc.expirydate ? new Date(doc.expirydate) : undefined,
        createdate: new Date(doc.createdate),
        modifieddate: doc.modifieddate ? new Date(doc.modifieddate) : undefined,
      }));
      setIssuedDocuments(processedData);
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : "Failed to update document";
      toast.error(errorMessage);
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleEditSupportingDocument = async () => {
    if (!editingSupportingDocument) return;

    setIsSubmitting(true);
    try {
      const updateData = {
        ...editSupportingForm,
      };

      await apiClient.updateOrganizationSupportingDocument(
        editingSupportingDocument,
        updateData,
      );
      toast.success("Document updated successfully");
      setEditingSupportingDocument(null);

      // Refresh documents
      const data = await apiClient.getOrganizationSupportingDocuments(
        organizationId,
        applicationId,
      );
      const processedData = data.map((doc: any) => ({
        ...doc,
        createdate: new Date(doc.createdate),
        modifieddate: doc.modifieddate ? new Date(doc.modifieddate) : undefined,
      }));
      setSupportingDocuments(processedData);
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : "Failed to update document";
      toast.error(errorMessage);
    } finally {
      setIsSubmitting(false);
    }
  };

  // Void handlers
  const handleVoidDocument = async (
    documentId: number,
    type: "issued" | "supporting" | "visa",
  ) => {
    if (!confirm("Are you sure you want to void this document?")) {
      return;
    }

    setIsSubmitting(true);
    try {
      if (type === "issued") {
        await apiClient.voidOrganizationIssuedDocument(documentId);
        const data = await apiClient.getOrganizationIssuedDocuments(
          organizationId,
          applicationId,
        );
        const processedData = data.map((doc: any) => ({
          ...doc,
          issuedate: doc.issuedate ? new Date(doc.issuedate) : undefined,
          validfromdate: doc.validfromdate
            ? new Date(doc.validfromdate)
            : undefined,
          expirydate: doc.expirydate ? new Date(doc.expirydate) : undefined,
          createdate: new Date(doc.createdate),
          modifieddate: doc.modifieddate
            ? new Date(doc.modifieddate)
            : undefined,
        }));
        setIssuedDocuments(processedData);
      } else if (type === "supporting") {
        await apiClient.voidOrganizationSupportingDocument(documentId);
        const data = await apiClient.getOrganizationSupportingDocuments(
          organizationId,
          applicationId,
        );
        const processedData = data.map((doc: any) => ({
          ...doc,
          createdate: new Date(doc.createdate),
          modifieddate: doc.modifieddate
            ? new Date(doc.modifieddate)
            : undefined,
        }));
        setSupportingDocuments(processedData);
      }

      toast.success("Document voided successfully");
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : "Failed to void document";
      toast.error(errorMessage);
    } finally {
      setIsSubmitting(false);
    }
  };

  // View document handler
  const handleViewDocument = async (
    documentId: number,
    type: "issued" | "supporting",
  ) => {
    setLoadingDocument(true);
    setViewingDocument(documentId);
    setDocumentData(null);

    try {
      let data;
      if (type === "issued") {
        data = await apiClient.getOrganizationIssuedDocumentFile(documentId);
      } else {
        data =
          await apiClient.getOrganizationSupportingDocumentFile(documentId);
      }

      if (data.document) {
        setDocumentData(data.document);
      } else {
        throw new Error("No document data available");
      }
    } catch (err) {
      toast.error("Failed to load document");
      setViewingDocument(null);
    } finally {
      setLoadingDocument(false);
    }
  };

  return (
    <div className="space-y-4">
      {/* Tab Navigation with Add buttons */}
      <div className="flex items-center justify-between">
        <div className="border-b border-gray-200 flex-1">
          <nav className="-mb-px flex space-x-8" aria-label="Tabs">
            <button
              onClick={() => setActiveTab("issued")}
              className={cn(
                activeTab === "issued"
                  ? "border-blue-500 text-blue-600"
                  : "border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300",
                "whitespace-nowrap border-b-2 py-2 px-1 text-xs font-medium flex items-center gap-2",
              )}
            >
              <FileCheck className="w-3 h-3" />
              Issued Documents
              {issuedDocuments.length > 0 && (
                <span
                  className={cn(
                    "ml-2 px-1.5 py-0.5 rounded-full text-xs",
                    activeTab === "issued"
                      ? "bg-blue-100 text-blue-600"
                      : "bg-gray-100 text-gray-600",
                  )}
                >
                  {issuedDocuments.length}
                </span>
              )}
            </button>
            <button
              onClick={() => setActiveTab("supporting")}
              className={cn(
                activeTab === "supporting"
                  ? "border-blue-500 text-blue-600"
                  : "border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300",
                "whitespace-nowrap border-b-2 py-2 px-1 text-xs font-medium flex items-center gap-2",
              )}
            >
              <Files className="w-3 h-3" />
              Supporting Documents
              {supportingDocuments.length > 0 && (
                <span
                  className={cn(
                    "ml-2 px-1.5 py-0.5 rounded-full text-xs",
                    activeTab === "supporting"
                      ? "bg-blue-100 text-blue-600"
                      : "bg-gray-100 text-gray-600",
                  )}
                >
                  {supportingDocuments.length}
                </span>
              )}
            </button>
            <button
              onClick={() => setActiveTab("visa")}
              className={cn(
                activeTab === "visa"
                  ? "border-blue-500 text-blue-600"
                  : "border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300",
                "whitespace-nowrap border-b-2 py-2 px-1 text-xs font-medium flex items-center gap-2",
              )}
            >
              <FilePlus className="w-3 h-3" />
              Visa Documents
              {visaDocuments.length > 0 && (
                <span
                  className={cn(
                    "ml-2 px-1.5 py-0.5 rounded-full text-xs",
                    activeTab === "visa"
                      ? "bg-blue-100 text-blue-600"
                      : "bg-gray-100 text-gray-600",
                  )}
                >
                  {visaDocuments.length}
                </span>
              )}
            </button>
          </nav>
        </div>

        {/* Add buttons */}
        <div className="flex items-center gap-2 ml-4">
          {activeTab === "issued" && (
            <button
              onClick={() => setShowNewIssuedModal(true)}
              className="compact-button bg-primary text-white flex items-center gap-1"
            >
              <Plus className="w-3 h-3" />
              Add Issued
            </button>
          )}
          {activeTab === "supporting" && (
            <button
              onClick={() => setShowNewSupportingModal(true)}
              className="compact-button bg-primary text-white flex items-center gap-1"
            >
              <Plus className="w-3 h-3" />
              Add Supporting
            </button>
          )}
          {activeTab === "visa" && (
            <button
              onClick={() => setShowNewVisaModal(true)}
              className="compact-button bg-primary text-white flex items-center gap-1"
            >
              <Plus className="w-3 h-3" />
              Add Visa
            </button>
          )}
        </div>
      </div>

      {/* Content Area */}
      <div className="bg-white border border-gray-200 rounded-lg overflow-hidden">
        {/* Issued Documents Tab */}
        {activeTab === "issued" && (
          <>
            {issuedLoading ? (
              <div className="flex items-center justify-center h-32">
                <div className="flex items-center gap-3">
                  <Loader2 className="w-5 h-5 animate-spin text-blue-600" />
                  <span className="text-sm text-gray-600">
                    Loading issued documents...
                  </span>
                </div>
              </div>
            ) : issuedError ? (
              <div className="flex items-center justify-center h-32">
                <div className="text-center">
                  <AlertCircle className="w-8 h-8 text-red-500 mx-auto mb-2" />
                  <h3 className="text-sm font-semibold text-gray-900 mb-1">
                    Failed to Load Documents
                  </h3>
                  <p className="text-xs text-gray-600">{issuedError}</p>
                </div>
              </div>
            ) : issuedDocuments.length === 0 ? (
              <div className="text-center py-8">
                <FileCheck className="w-8 h-8 text-gray-400 mx-auto mb-3" />
                <p className="text-sm text-gray-600 font-medium">
                  No issued documents found
                </p>
                <p className="text-xs text-gray-500 mt-1">
                  Issued documents will appear here when available
                </p>
                <button
                  onClick={() => setShowNewIssuedModal(true)}
                  className="mt-3 compact-button bg-primary text-white"
                >
                  Add First Document
                </button>
              </div>
            ) : (
              <div className="overflow-x-auto">
                <table className="data-table w-full">
                  <thead>
                    <tr>
                      <th>Document Type</th>
                      <th>Status</th>
                      <th>Dates</th>
                      <th>Document</th>
                      <th>Meta</th>
                      <th>Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {issuedDocuments.map((doc) => (
                      <tr
                        key={doc.tgorganisationissueddocumentid}
                        className={cn(
                          "hover:bg-gray-50 transition-colors",
                          doc.isactive === 0 && "opacity-60 bg-gray-50",
                        )}
                      >
                        <td>
                          <div className="space-y-0.5">
                            {doc.documenttypelookupid ? (
                              <>
                                <LookupField
                                  lookupId={doc.documenttypelookupid}
                                  format="name"
                                  className="text-xs font-medium"
                                />
                                <div className="text-[10px] text-gray-500">
                                  ID: {doc.documenttypelookupid}
                                </div>
                              </>
                            ) : (
                              <span className="text-xs text-gray-500">-</span>
                            )}
                          </div>
                        </td>
                        <td>
                          {doc.documentstatuslookupid ? (
                            <div className="space-y-0.5">
                              <LookupField
                                lookupId={doc.documentstatuslookupid}
                                format="name"
                                className={cn(
                                  "text-[10px] px-2 py-1 rounded-full font-medium",
                                  "bg-blue-50 text-blue-700",
                                )}
                              />
                            </div>
                          ) : (
                            <span className="text-xs text-gray-500">-</span>
                          )}
                        </td>
                        <td>
                          <div className="space-y-1">
                            {doc.issuedate && (
                              <div className="flex items-center gap-1 text-[10px]">
                                <Calendar className="w-3 h-3 text-gray-400" />
                                <span className="text-gray-700">
                                  {formatDateCompact(doc.issuedate)}
                                </span>
                              </div>
                            )}
                            {doc.expirydate && (
                              <div className="flex items-center gap-1 text-[10px]">
                                <span className="text-gray-500">Exp:</span>
                                <span
                                  className={cn(
                                    "font-medium",
                                    isExpired(doc.expirydate)
                                      ? "text-red-600"
                                      : isExpiringSoon(doc.expirydate)
                                        ? "text-yellow-600"
                                        : "text-gray-700",
                                  )}
                                >
                                  {formatDateCompact(doc.expirydate)}
                                </span>
                                {isExpired(doc.expirydate) && (
                                  <AlertCircle className="w-3 h-3 text-red-500" />
                                )}
                                {isExpiringSoon(doc.expirydate) && (
                                  <AlertCircle className="w-3 h-3 text-yellow-500" />
                                )}
                              </div>
                            )}
                          </div>
                        </td>

                        <td>
                          <MetaStrip record={doc} />
                        </td>
                        <td>
                          <div className="flex items-center gap-1">
                            <button
                              onClick={() => {
                                setEditingIssuedDocument(
                                  doc.tgorganisationissueddocumentid,
                                );
                                setEditIssuedForm({
                                  documenttypelookupid:
                                    doc.documenttypelookupid ?? null,
                                  documentstatuslookupid:
                                    doc.documentstatuslookupid ?? null,
                                  documentobject: null,
                                  issuedate: doc.issuedate
                                    ? doc.issuedate.toISOString().split("T")[0]
                                    : "",
                                  validfromdate: doc.validfromdate
                                    ? doc.validfromdate
                                        .toISOString()
                                        .split("T")[0]
                                    : "",
                                  expirydate: doc.expirydate
                                    ? doc.expirydate.toISOString().split("T")[0]
                                    : "",
                                });
                              }}
                              className="compact-button border"
                            >
                              <Edit2 className="w-3 h-3" />
                            </button>
                            {doc.isactive === 1 && (
                              <button
                                onClick={() =>
                                  handleVoidDocument(
                                    doc.tgorganisationissueddocumentid,
                                    "issued",
                                  )
                                }
                                className="compact-button border border-red-200 text-red-600 hover:bg-red-50"
                              >
                                <Trash2 className="w-3 h-3" />
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
          </>
        )}

        {/* Supporting Documents Tab */}
        {activeTab === "supporting" && (
          <>
            {supportingLoading ? (
              <div className="flex items-center justify-center h-32">
                <div className="flex items-center gap-3">
                  <Loader2 className="w-5 h-5 animate-spin text-blue-600" />
                  <span className="text-sm text-gray-600">
                    Loading supporting documents...
                  </span>
                </div>
              </div>
            ) : supportingError ? (
              <div className="flex items-center justify-center h-32">
                <div className="text-center">
                  <AlertCircle className="w-8 h-8 text-red-500 mx-auto mb-2" />
                  <h3 className="text-sm font-semibold text-gray-900 mb-1">
                    Failed to Load Documents
                  </h3>
                  <p className="text-xs text-gray-600">{supportingError}</p>
                </div>
              </div>
            ) : supportingDocuments.length === 0 ? (
              <div className="text-center py-8">
                <Files className="w-8 h-8 text-gray-400 mx-auto mb-3" />
                <p className="text-sm text-gray-600 font-medium">
                  No supporting documents found
                </p>
                <p className="text-xs text-gray-500 mt-1">
                  Supporting documents will appear here when available
                </p>
                <button
                  onClick={() => setShowNewSupportingModal(true)}
                  className="mt-3 compact-button bg-primary text-white"
                >
                  Add First Document
                </button>
              </div>
            ) : (
              <div className="overflow-x-auto">
                <table className="data-table w-full">
                  <thead>
                    <tr>
                      <th>Document Type</th>
                      <th>Category</th>
                      <th>Status</th>
                      <th>Document</th>
                      <th>Meta</th>
                      <th>Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {supportingDocuments.map((doc) => (
                      <tr
                        key={doc.tgorganisationsupportingdocumentid}
                        className={cn(
                          "hover:bg-gray-50 transition-colors",
                          doc.isactive === 0 && "opacity-60 bg-gray-50",
                        )}
                      >
                        <td>
                          <div className="space-y-0.5">
                            {doc.documenttypelookupid ? (
                              <>
                                <LookupField
                                  lookupId={doc.documenttypelookupid}
                                  format="name"
                                  className="text-xs font-medium"
                                />
                                <div className="text-[10px] text-gray-500">
                                  ID: {doc.documenttypelookupid}
                                </div>
                              </>
                            ) : (
                              <span className="text-xs text-gray-500">-</span>
                            )}
                          </div>
                        </td>
                        <td>
                          {doc.documentcategorylookupid ? (
                            <div className="space-y-0.5">
                              <LookupField
                                lookupId={doc.documentcategorylookupid}
                                format="name"
                                className="text-xs font-medium"
                              />
                              <div className="text-[10px] text-gray-500">
                                ID: {doc.documentcategorylookupid}
                              </div>
                            </div>
                          ) : (
                            <span className="text-xs text-gray-500">-</span>
                          )}
                        </td>
                        <td>
                          {doc.documentstatuslookupid ? (
                            <div className="space-y-0.5">
                              <LookupField
                                lookupId={doc.documentstatuslookupid}
                                format="name"
                                className={cn(
                                  "text-[10px] px-2 py-1 rounded-full font-medium",
                                  "bg-green-50 text-green-700",
                                )}
                              />
                            </div>
                          ) : (
                            <span className="text-xs text-gray-500">-</span>
                          )}
                        </td>
                        <td>
                          <div className="flex items-center gap-2">
                            {doc.has_document ? (
                              <button
                                onClick={() =>
                                  handleViewDocument(
                                    doc.tgorganisationsupportingdocumentid,
                                    "supporting",
                                  )
                                }
                                className="compact-button border flex items-center gap-1"
                              >
                                <Eye className="w-3 h-3" />
                                View
                              </button>
                            ) : (
                              <span className="text-xs text-gray-400">
                                No file
                              </span>
                            )}
                          </div>
                        </td>
                        <td>
                          <MetaStrip record={doc} />
                        </td>
                        <td>
                          <div className="flex items-center gap-1">
                            <button
                              onClick={() => {
                                setEditingSupportingDocument(
                                  doc.tgorganisationsupportingdocumentid,
                                );
                                setEditSupportingForm({
                                  documenttypelookupid:
                                    doc.documenttypelookupid ?? null,
                                  documentcategorylookupid:
                                    doc.documentcategorylookupid ?? null,
                                  documentstatuslookupid:
                                    doc.documentstatuslookupid ?? null,
                                  documentobject: null,
                                });
                              }}
                              className="compact-button border"
                            >
                              <Edit2 className="w-3 h-3" />
                            </button>
                            {doc.isactive === 1 && (
                              <button
                                onClick={() =>
                                  handleVoidDocument(
                                    doc.tgorganisationsupportingdocumentid,
                                    "supporting",
                                  )
                                }
                                className="compact-button border border-red-200 text-red-600 hover:bg-red-50"
                              >
                                <Trash2 className="w-3 h-3" />
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
          </>
        )}

        {/* Visa Documents Tab */}
        {activeTab === "visa" && (
          <>
            {visaLoading ? (
              <div className="flex items-center justify-center h-32">
                <div className="flex items-center gap-3">
                  <Loader2 className="w-5 h-5 animate-spin text-blue-600" />
                  <span className="text-sm text-gray-600">
                    Loading visa documents...
                  </span>
                </div>
              </div>
            ) : visaError ? (
              <div className="flex items-center justify-center h-32">
                <div className="text-center">
                  <AlertCircle className="w-8 h-8 text-red-500 mx-auto mb-2" />
                  <h3 className="text-sm font-semibold text-gray-900 mb-1">
                    Failed to Load Documents
                  </h3>
                  <p className="text-xs text-gray-600">{visaError}</p>
                </div>
              </div>
            ) : visaDocuments.length === 0 ? (
              <div className="text-center py-8">
                <FilePlus className="w-8 h-8 text-gray-400 mx-auto mb-3" />
                <p className="text-sm text-gray-600 font-medium">
                  No visa documents found
                </p>
                <p className="text-xs text-gray-500 mt-1">
                  Visa documents will appear here when available
                </p>
                <button
                  onClick={() => setShowNewVisaModal(true)}
                  className="mt-3 compact-button bg-primary text-white"
                >
                  Add First Document
                </button>
              </div>
            ) : (
              <div className="overflow-x-auto">
                <table className="data-table w-full">
                  <thead>
                    <tr>
                      <th>Person Document</th>
                      <th>Visa Type</th>
                      <th>Document Details</th>
                      <th>Meta</th>
                      <th>Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {visaDocuments.map((doc) => (
                      <tr
                        key={doc.tgorganisationvisaid}
                        className={cn(
                          "hover:bg-gray-50 transition-colors",
                          doc.isactive === 0 && "opacity-60 bg-gray-50",
                        )}
                      >
                        <td>
                          <div className="space-y-0.5">
                            <span className="text-xs font-medium">
                              ID: {doc.tgpersonissueddocumentid}
                            </span>
                            {doc.tgpersonissueddocument && (
                              <div className="text-[10px] text-gray-500">
                                {doc.tgpersonissueddocument.documentnumber && (
                                  <div>
                                    No:{" "}
                                    {doc.tgpersonissueddocument.documentnumber}
                                  </div>
                                )}
                              </div>
                            )}
                          </div>
                        </td>
                        <td>
                          {doc.visatypelookupid ? (
                            <div className="space-y-0.5">
                              <LookupField
                                lookupId={doc.visatypelookupid}
                                format="name"
                                className="text-xs font-medium"
                              />
                              <div className="text-[10px] text-gray-500">
                                ID: {doc.visatypelookupid}
                              </div>
                            </div>
                          ) : (
                            <span className="text-xs text-gray-500">-</span>
                          )}
                        </td>
                        <td>
                          {doc.tgpersonissueddocument && (
                            <div className="space-y-1">
                              {doc.tgpersonissueddocument.issuedate && (
                                <div className="flex items-center gap-1 text-[10px]">
                                  <Calendar className="w-3 h-3 text-gray-400" />
                                  <span className="text-gray-700">
                                    {formatDateCompact(
                                      doc.tgpersonissueddocument.issuedate,
                                    )}
                                  </span>
                                </div>
                              )}
                              {doc.tgpersonissueddocument.expirydate && (
                                <div className="flex items-center gap-1 text-[10px]">
                                  <span className="text-gray-500">Exp:</span>
                                  <span className="text-gray-700">
                                    {formatDateCompact(
                                      doc.tgpersonissueddocument.expirydate,
                                    )}
                                  </span>
                                </div>
                              )}
                            </div>
                          )}
                        </td>
                        <td>
                          <MetaStrip record={doc} />
                        </td>
                        <td>
                          <div className="flex items-center gap-1">
                            <button
                              onClick={() => {
                                setEditingVisaDocument(
                                  doc.tgorganisationvisaid,
                                );
                                setEditVisaForm({
                                  tgpersonissueddocumentid:
                                    doc.tgpersonissueddocumentid,
                                  visatypelookupid: doc.visatypelookupid,
                                });
                              }}
                              className="compact-button border"
                            >
                              <Edit2 className="w-3 h-3" />
                            </button>
                          </div>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </>
        )}
      </div>

      <DocumentViewer
        isOpen={!!viewingDocument}
        onClose={() => setViewingDocument(null)}
        documentData={documentData}
        loading={loadingDocument}
        documentId={viewingDocument || undefined}
      />

      {/* New Issued Document Modal */}
      {showNewIssuedModal && (
        <div
          className="fixed inset-0 bg-black bg-opacity-75 flex items-center justify-center z-50"
          onClick={() => setShowNewIssuedModal(false)}
        >
          <div
            className="bg-white rounded-lg p-6 max-w-4xl w-full mx-4 max-h-[90vh] overflow-y-auto"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="text-sm font-semibold mb-4">Add Issued Document</h3>

            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Document Type <span className="text-red-500">*</span>
                </label>
                <SearchableSelect
                  options={documentTypeOptions}
                  value={newIssuedDocumentForm.documenttypelookupid}
                  onChange={(value) =>
                    setNewIssuedDocumentForm((prev) => ({
                      ...prev,
                      documenttypelookupid: value,
                    }))
                  }
                  placeholder="Select document type..."
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
                  value={newIssuedDocumentForm.documentstatuslookupid}
                  onChange={(value) =>
                    setNewIssuedDocumentForm((prev) => ({
                      ...prev,
                      documentstatuslookupid: value,
                    }))
                  }
                  placeholder="Select status..."
                  loading={loadingOptions}
                  className="w-full"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Issue Date
                </label>
                <input
                  type="date"
                  value={newIssuedDocumentForm.issuedate}
                  onChange={(e) =>
                    setNewIssuedDocumentForm((prev) => ({
                      ...prev,
                      issuedate: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-xs border rounded"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Valid From Date
                </label>
                <input
                  type="date"
                  value={newIssuedDocumentForm.validfromdate}
                  onChange={(e) =>
                    setNewIssuedDocumentForm((prev) => ({
                      ...prev,
                      validfromdate: e.target.value,
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
                  value={newIssuedDocumentForm.expirydate}
                  onChange={(e) =>
                    setNewIssuedDocumentForm((prev) => ({
                      ...prev,
                      expirydate: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-xs border rounded"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Document File
                </label>
                <div className="border-2 border-dashed border-gray-300 rounded-lg p-4">
                  <input
                    type="file"
                    accept=".pdf,.jpg,.jpeg,.png"
                    onChange={async (e) => {
                      const file = e.target.files?.[0];
                      if (file) {
                        try {
                          const base64 = await new Promise<string>(
                            (resolve, reject) => {
                              const reader = new FileReader();
                              reader.onload = () => {
                                const result = reader.result as string;
                                resolve(result.split(",")[1]);
                              };
                              reader.onerror = reject;
                              reader.readAsDataURL(file);
                            },
                          );
                          setNewIssuedDocumentForm((prev) => ({
                            ...prev,
                            documentobject: base64,
                          }));
                        } catch (error) {
                          console.error("File upload error:", error);
                          toast.error("Failed to upload file");
                        }
                      }
                    }}
                    className="w-full text-xs"
                  />
                  {newIssuedDocumentForm.documentobject && (
                    <p className="text-xs text-green-600 mt-2">
                      Document uploaded
                    </p>
                  )}
                </div>
              </div>
            </div>

            <div className="flex justify-end gap-2 mt-6">
              <button
                onClick={() => {
                  setShowNewIssuedModal(false);
                  setNewIssuedDocumentForm({
                    documenttypelookupid: null,
                    documentstatuslookupid: null,
                    documentobject: null,
                    issuedate: "",
                    validfromdate: "",
                    expirydate: "",
                  });
                }}
                className="compact-button border"
                disabled={isSubmitting}
              >
                Cancel
              </button>
              <button
                onClick={handleCreateIssuedDocument}
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
        </div>
      )}

      {/* New Supporting Document Modal */}
      {showNewSupportingModal && (
        <div
          className="fixed inset-0 bg-black bg-opacity-75 flex items-center justify-center z-50"
          onClick={() => setShowNewSupportingModal(false)}
        >
          <div
            className="bg-white rounded-lg p-6 max-w-2xl w-full mx-4"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="text-sm font-semibold mb-4">
              Add Supporting Document
            </h3>

            <div className="space-y-4">
              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Document Type *
                </label>
                <SearchableSelect
                  options={supportingDocumentTypeOptions}
                  value={newSupportingDocumentForm.documenttypelookupid}
                  onChange={(value) =>
                    setNewSupportingDocumentForm((prev) => ({
                      ...prev,
                      documenttypelookupid: value,
                    }))
                  }
                  placeholder="Select document type..."
                  loading={loadingOptions}
                  className="w-full"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Document Category
                </label>
                <SearchableSelect
                  options={documentCategoryOptions}
                  value={newSupportingDocumentForm.documentcategorylookupid}
                  onChange={(value) =>
                    setNewSupportingDocumentForm((prev) => ({
                      ...prev,
                      documentcategorylookupid: value,
                    }))
                  }
                  placeholder="Select category..."
                  loading={loadingOptions}
                  className="w-full"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Document Status
                </label>
                <SearchableSelect
                  options={supportingStatusOptions}
                  value={newSupportingDocumentForm.documentstatuslookupid}
                  onChange={(value) =>
                    setNewSupportingDocumentForm((prev) => ({
                      ...prev,
                      documentstatuslookupid: value,
                    }))
                  }
                  placeholder="Select status..."
                  loading={loadingOptions}
                  className="w-full"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Document File
                </label>
                <div className="border-2 border-dashed border-gray-300 rounded-lg p-4">
                  <input
                    type="file"
                    accept=".pdf,.jpg,.jpeg,.png"
                    onChange={async (e) => {
                      const file = e.target.files?.[0];
                      if (file) {
                        try {
                          const base64 = await new Promise<string>(
                            (resolve, reject) => {
                              const reader = new FileReader();
                              reader.onload = () => {
                                const result = reader.result as string;
                                resolve(result.split(",")[1]);
                              };
                              reader.onerror = reject;
                              reader.readAsDataURL(file);
                            },
                          );
                          setNewSupportingDocumentForm((prev) => ({
                            ...prev,
                            documentobject: base64,
                          }));
                        } catch (error) {
                          console.error("File upload error:", error);
                          toast.error("Failed to upload file");
                        }
                      }
                    }}
                    className="w-full text-xs"
                  />
                  {newSupportingDocumentForm.documentobject && (
                    <p className="text-xs text-green-600 mt-2">
                      Document uploaded
                    </p>
                  )}
                </div>
              </div>
            </div>

            <div className="flex justify-end gap-3 mt-6">
              <button
                onClick={() => setShowNewSupportingModal(false)}
                className="compact-button border"
                disabled={isSubmitting}
              >
                Cancel
              </button>
              <button
                onClick={handleCreateSupportingDocument}
                className="compact-button bg-primary text-white"
                disabled={
                  isSubmitting ||
                  !newSupportingDocumentForm.documenttypelookupid
                }
              >
                {isSubmitting ? (
                  <>
                    <Loader2 className="w-3 h-3 mr-1 animate-spin" />
                    Creating...
                  </>
                ) : (
                  "Create Document"
                )}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* New Visa Document Modal */}
      {showNewVisaModal && (
        <div
          className="fixed inset-0 bg-black bg-opacity-75 flex items-center justify-center z-50"
          onClick={() => setShowNewVisaModal(false)}
        >
          <div
            className="bg-white rounded-lg p-6 max-w-2xl w-full mx-4"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="text-sm font-semibold mb-4">Add Visa Document</h3>

            <div className="space-y-4">
              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Person Issued Document *
                </label>
                <SearchableSelect
                  options={personDocumentOptions}
                  value={newVisaDocumentForm.tgpersonissueddocumentid}
                  onChange={(value) =>
                    setNewVisaDocumentForm((prev) => ({
                      ...prev,
                      tgpersonissueddocumentid: value,
                    }))
                  }
                  placeholder="Select person document..."
                  loading={loadingOptions}
                  className="w-full"
                />
                <p className="text-xs text-gray-500 mt-1">
                  Note: Person document options need to be loaded from API
                </p>
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Visa Type *
                </label>
                <SearchableSelect
                  options={visaTypeOptions}
                  value={newVisaDocumentForm.visatypelookupid}
                  onChange={(value) =>
                    setNewVisaDocumentForm((prev) => ({
                      ...prev,
                      visatypelookupid: value,
                    }))
                  }
                  placeholder="Select visa type..."
                  loading={loadingOptions}
                  className="w-full"
                />
              </div>
            </div>

            <div className="flex justify-end gap-3 mt-6">
              <button
                onClick={() => setShowNewVisaModal(false)}
                className="compact-button border"
                disabled={isSubmitting}
              >
                Cancel
              </button>
              <button
                onClick={handleCreateVisaDocument}
                className="compact-button bg-primary text-white"
                disabled={
                  isSubmitting ||
                  !newVisaDocumentForm.tgpersonissueddocumentid ||
                  !newVisaDocumentForm.visatypelookupid
                }
              >
                {isSubmitting ? (
                  <>
                    <Loader2 className="w-3 h-3 mr-1 animate-spin" />
                    Creating...
                  </>
                ) : (
                  "Create Document"
                )}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Edit Issued Document Modal */}
      {editingIssuedDocument && (
        <div
          className="fixed inset-0 bg-black bg-opacity-75 flex items-center justify-center z-50"
          onClick={() => setEditingIssuedDocument(null)}
        >
          <div
            className="bg-white rounded-lg p-6 max-w-2xl w-full mx-4"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="text-sm font-semibold mb-4">Edit Issued Document</h3>

            <div className="space-y-4">
              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Document Status
                </label>
                <SearchableSelect
                  options={statusOptions}
                  value={editIssuedForm.documentstatuslookupid}
                  onChange={(value) =>
                    setEditIssuedForm((prev) => ({
                      ...prev,
                      documentstatuslookupid: value,
                    }))
                  }
                  placeholder="Select status..."
                  loading={loadingOptions}
                  className="w-full"
                />
              </div>

              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="block text-xs font-medium text-gray-600 mb-1">
                    Issue Date
                  </label>
                  <input
                    type="date"
                    value={editIssuedForm.issuedate}
                    onChange={(e) =>
                      setEditIssuedForm((prev) => ({
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
                    value={editIssuedForm.expirydate}
                    onChange={(e) =>
                      setEditIssuedForm((prev) => ({
                        ...prev,
                        expirydate: e.target.value,
                      }))
                    }
                    className="w-full p-2 text-xs border rounded"
                  />
                </div>
              </div>
            </div>

            <div className="flex justify-end gap-3 mt-6">
              <button
                onClick={() => setEditingIssuedDocument(null)}
                className="compact-button border"
                disabled={isSubmitting}
              >
                Cancel
              </button>
              <button
                onClick={handleEditIssuedDocument}
                className="compact-button bg-primary text-white"
                disabled={isSubmitting}
              >
                {isSubmitting ? (
                  <>
                    <Loader2 className="w-3 h-3 mr-1 animate-spin" />
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

      {/* Edit Supporting Document Modal */}
      {editingSupportingDocument && (
        <div
          className="fixed inset-0 bg-black bg-opacity-75 flex items-center justify-center z-50"
          onClick={() => setEditingSupportingDocument(null)}
        >
          <div
            className="bg-white rounded-lg p-6 max-w-2xl w-full mx-4"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="text-sm font-semibold mb-4">
              Edit Supporting Document
            </h3>

            <div className="space-y-4">
              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Document Type
                </label>
                <SearchableSelect
                  options={supportingDocumentTypeOptions}
                  value={editSupportingForm.documenttypelookupid}
                  onChange={(value) =>
                    setEditSupportingForm((prev) => ({
                      ...prev,
                      documenttypelookupid: value,
                    }))
                  }
                  placeholder="Select document type..."
                  loading={loadingOptions}
                  className="w-full"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Document Category
                </label>
                <SearchableSelect
                  options={documentCategoryOptions}
                  value={editSupportingForm.documentcategorylookupid}
                  onChange={(value) =>
                    setEditSupportingForm((prev) => ({
                      ...prev,
                      documentcategorylookupid: value,
                    }))
                  }
                  placeholder="Select category..."
                  loading={loadingOptions}
                  className="w-full"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Document Status
                </label>
                <SearchableSelect
                  options={supportingStatusOptions}
                  value={editSupportingForm.documentstatuslookupid}
                  onChange={(value) =>
                    setEditSupportingForm((prev) => ({
                      ...prev,
                      documentstatuslookupid: value,
                    }))
                  }
                  placeholder="Select status..."
                  loading={loadingOptions}
                  className="w-full"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Document File
                </label>
                <div className="border-2 border-dashed border-gray-300 rounded-lg p-4">
                  <input
                    type="file"
                    accept=".pdf,.jpg,.jpeg,.png"
                    onChange={async (e) => {
                      const file = e.target.files?.[0];
                      if (file) {
                        try {
                          const base64 = await new Promise<string>(
                            (resolve, reject) => {
                              const reader = new FileReader();
                              reader.onload = () => {
                                const result = reader.result as string;
                                resolve(result.split(",")[1]);
                              };
                              reader.onerror = reject;
                              reader.readAsDataURL(file);
                            },
                          );
                          setEditSupportingForm((prev) => ({
                            ...prev,
                            documentobject: base64,
                          }));
                        } catch (error) {
                          console.error("File upload error:", error);
                        }
                      }
                    }}
                    className="w-full text-xs"
                  />
                  {editSupportingForm.documentobject && (
                    <p className="text-xs text-green-600 mt-2">
                      Document uploaded
                    </p>
                  )}
                </div>
              </div>
            </div>

            <div className="flex justify-end gap-3 mt-6">
              <button
                onClick={() => setEditingSupportingDocument(null)}
                className="compact-button border"
                disabled={isSubmitting}
              >
                Cancel
              </button>
              <button
                onClick={handleEditSupportingDocument}
                className="compact-button bg-primary text-white"
                disabled={isSubmitting}
              >
                {isSubmitting ? (
                  <>
                    <Loader2 className="w-3 h-3 mr-1 animate-spin" />
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
