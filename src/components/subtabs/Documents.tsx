//
//  campus-pilot
//  Documents.tsx
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
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
} from "lucide-react";
import { formatDate, formatDateCompact } from "../../lib/utils";
import { cn } from "../../lib/utils";
import { apiClient } from "../../lib/api";
import { LookupField } from "../LookupField";
import { SearchableSelect } from "../SearchableSelect";
import { DocumentViewer } from "../DocumentViewer";
import toast from "react-hot-toast";

interface DocumentsProps {
  personId: number;
  applicationId?: number;
  applicationTypeName?: string;
}

interface DocumentRecord {
  tgpersonissueddocumentid: number;
  parenttgpersonissueddocumentid?: number;
  tgpersonid: number;
  tgapplicationid?: number;
  documenttypelookupid?: number;
  documentobject?: string;
  issuedate?: Date;
  validfromdate?: Date;
  expirydate?: Date;
  reasonforissuelookupid?: number;
  reasonforissueother?: string;
  placeofprinting?: string;
  personalizeddocumentstatuslookupid?: number;
  documentserialnumber?: string;
  documentnumber?: string;
  issuingauthority?: string;
  issuingcountrylookupid?: number;
  persofeedback?: string;
  validityperiodlookupid?: number;
  validityperiod?: string;
  visamultipleentry?: number;
  portalrecordstatuslookupid?: number;
  recordstatuslookupid?: number;
  createdate: Date;
  modifieddate: Date;
  tguserauditdetailid: number;
  createdbysystemuserid: number;
  updatedbysystemuserid?: number;
  dataownerlookupid: number;
  isactive: number;
  has_document?: boolean;
}

interface SupportingDocumentRecord {
  tgpersonsupportingdocumentid: number;
  tgpersonid: number;
  documenttypelookupid?: number;
  documentcategorylookupid?: number;
  documentstatuslookupid?: number;
  documentobject?: string;
  tgapplicationid?: number;
  remark?: string;
  portalrecordstatuslookupid?: number;
  recordstatuslookupid?: number;
  createdate: Date;
  modifieddate?: Date;
  tguserauditdetailid: number;
  createdbysystemuserid: number;
  updatedbysystemuserid?: number;
  dataownerlookupid: number;
  isactive: number;
  has_document?: boolean;
}

interface NewDocumentForm {
  documenttypelookupid: number | null;
  documentnumber: string;
  documentserialnumber: string;
  issuedate: string;
  validfromdate: string;
  expirydate: string;
  reasonforissuelookupid: number | null;
  reasonforissueother: string;
  placeofprinting: string;
  issuingauthority: string;
  issuingcountrylookupid: number | null;
  validityperiodlookupid: number | null;
  validityperiod: string;
  visamultipleentry: boolean;
}

interface NewSupportingDocumentForm {
  documenttypelookupid: number | null;
  documentcategorylookupid: number | null;
  documentstatuslookupid: number | null;
  documentobject: string | null;
  remark: string;
}

interface EditDocumentForm {
  documenttypelookupid: number | null;
  personalizeddocumentstatuslookupid: number | null;
  documentnumber: string;
  documentserialnumber: string;
  issuedate: string;
  validfromdate: string;
  expirydate: string;
  reasonforissuelookupid: number | null;
  reasonforissueother: string;
  placeofprinting: string;
  issuingauthority: string;
  issuingcountrylookupid: number | null;
  validityperiodlookupid: number | null;
  validityperiod: string;
  visamultipleentry: boolean;
}

interface EditSupportingDocumentForm {
  documenttypelookupid: number | null;
  documentcategorylookupid: number | null;
  documentstatuslookupid: number | null;
  documentobject: string | null;
  remark: string;
}

export function Documents({
  personId,
  applicationId,
  applicationTypeName,
}: DocumentsProps) {
  // Tab state
  const [activeTab, setActiveTab] = useState<
    "issued" | "supporting" | "previous"
  >("issued");

  // Issued documents state
  const [documents, setDocuments] = useState<DocumentRecord[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Supporting documents state
  const [supportingDocuments, setSupportingDocuments] = useState<
    SupportingDocumentRecord[]
  >([]);
  const [supportingDocsLoading, setSupportingDocsLoading] = useState(true);
  const [supportingDocsError, setSupportingDocsError] = useState<string | null>(
    null,
  );

  // Previous documents state
  const [previousDocuments, setPreviousDocuments] = useState<DocumentRecord[]>(
    [],
  );
  const [previousDocsLoading, setPreviousDocsLoading] = useState(true);
  const [previousDocsError, setPreviousDocsError] = useState<string | null>(
    null,
  );
  const [selectedDocument, setSelectedDocument] = useState<number | null>(null);

  const [editingDocument, setEditingDocument] = useState<number | null>(null);
  const [editingSupportingDocument, setEditingSupportingDocument] = useState<
    number | null
  >(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [documentTypeOptions, setDocumentTypeOptions] = useState<any[]>([]);
  const [supportingDocumentTypeOptions, setSupportingDocumentTypeOptions] =
    useState<any[]>([]);
  const [documentCategoryOptions, setDocumentCategoryOptions] = useState<any[]>(
    [],
  );
  const [reasonForIssueOptions, setReasonForIssueOptions] = useState<any[]>([]);
  const [countryOptions, setCountryOptions] = useState<any[]>([]);
  const [validityPeriodOptions, setValidityPeriodOptions] = useState<any[]>([]);
  const [statusOptions, setStatusOptions] = useState<any[]>([]);
  const [supportingStatusOptions, setSupportingStatusOptions] = useState<any[]>(
    [],
  );

  const [loadingOptions, setLoadingOptions] = useState(true);
  const [viewingDocument, setViewingDocument] = useState<number | null>(null);
  const [documentData, setDocumentData] = useState<string | null>(null);
  const [loadingDocument, setLoadingDocument] = useState(false);
  const [editForm, setEditForm] = useState<EditDocumentForm>({
    documenttypelookupid: null,
    personalizeddocumentstatuslookupid: null,
    documentnumber: "",
    documentserialnumber: "",
    issuedate: "",
    validfromdate: "",
    expirydate: "",
    reasonforissuelookupid: null,
    reasonforissueother: "",
    placeofprinting: "",
    issuingauthority: "",
    issuingcountrylookupid: null,
    validityperiodlookupid: null,
    validityperiod: "",
    visamultipleentry: false,
  });

  const [supportingEditForm, setSupportingEditForm] =
    useState<EditSupportingDocumentForm>({
      documenttypelookupid: null,
      documentcategorylookupid: null,
      documentstatuslookupid: null,
      documentobject: null,
      remark: "",
    });

  const [newDocumentForm, setNewDocumentForm] = useState<NewDocumentForm>({
    documenttypelookupid: null,
    documentnumber: "",
    documentserialnumber: "",
    issuedate: "",
    validfromdate: "",
    expirydate: "",
    reasonforissuelookupid: null,
    reasonforissueother: "",
    placeofprinting: "",
    issuingauthority: "",
    issuingcountrylookupid: null,
    validityperiodlookupid: null,
    validityperiod: "",
    visamultipleentry: false,
  });

  const [newSupportingDocumentForm, setNewSupportingDocumentForm] =
    useState<NewSupportingDocumentForm>({
      documenttypelookupid: null,
      documentcategorylookupid: null,
      documentstatuslookupid: null,
      documentobject: null,
      remark: "",
    });

  const [showNewSupportingDocumentModal, setShowNewSupportingDocumentModal] =
    useState(false);
  const [showNewDocumentModal, setShowNewDocumentModal] = useState(false);

  // Fetch documents data
  useEffect(() => {
    const fetchDocuments = async () => {
      try {
        setIsLoading(true);
        setError(null);

        const data = await apiClient.getDocuments(personId, applicationId);
        // Convert date strings to Date objects
        const processedData = data.map((doc: any) => ({
          ...doc,
          issuedate: doc.issuedate ? new Date(doc.issuedate) : undefined,
          validfromdate: doc.validfromdate
            ? new Date(doc.validfromdate)
            : undefined,
          expirydate: doc.expirydate ? new Date(doc.expirydate) : undefined,
          createdate: new Date(doc.createdate),
          modifieddate: new Date(doc.modifieddate),
        }));
        setDocuments(processedData);
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to load documents";
        setError(errorMessage);
        toast.error(errorMessage);
      } finally {
        setIsLoading(false);
      }
    };

    fetchDocuments();
  }, [personId, applicationId]);

  // Fetch supporting documents
  useEffect(() => {
    const fetchSupportingDocuments = async () => {
      try {
        setSupportingDocsLoading(true);
        setSupportingDocsError(null);

        const data = await apiClient.getSupportingDocuments(
          personId,
          applicationId,
        );
        // Convert date strings to Date objects
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
        setSupportingDocsError(errorMessage);
        toast.error(errorMessage);
      } finally {
        setSupportingDocsLoading(false);
      }
    };

    fetchSupportingDocuments();
  }, [personId, applicationId]);

  // Fetch previous documents
  useEffect(() => {
    const fetchPreviousDocuments = async () => {
      try {
        setPreviousDocsLoading(true);
        setPreviousDocsError(null);

        const data = await apiClient.getPreviousDocuments(
          personId,
          applicationId,
        );
        // Convert date strings to Date objects
        const processedData = data.map((doc: any) => ({
          ...doc,
          issuedate: doc.issuedate ? new Date(doc.issuedate) : undefined,
          validfromdate: doc.validfromdate
            ? new Date(doc.validfromdate)
            : undefined,
          expirydate: doc.expirydate ? new Date(doc.expirydate) : undefined,
          createdate: new Date(doc.createdate),
          modifieddate: new Date(doc.modifieddate),
        }));
        setPreviousDocuments(processedData);
      } catch (err) {
        const errorMessage =
          err instanceof Error
            ? err.message
            : "Failed to load previous documents";
        setPreviousDocsError(errorMessage);
        toast.error(errorMessage);
      } finally {
        setPreviousDocsLoading(false);
      }
    };

    fetchPreviousDocuments();
  }, [personId, applicationId]);

  // Fetch lookup options
  useEffect(() => {
    const fetchLookupOptions = async () => {
      try {
        setLoadingOptions(true);
        const [
          documentTypeData,
          supportingDocumentTypeData,
          documentCategoryData,
          reasonData,
          countryData,
          validityData,
          statusData,
          supportingStatusData,
        ] = await Promise.all([
          apiClient.getLookupsByType("PERSONALIZED_DOCUMENT_TYPE"),
          apiClient.getLookupsByType("SUPPORTING_DOCUMENT_TYPE"),
          apiClient.getLookupsByType("DOCUMENT_CATEGORY"),
          apiClient.getLookupsByType(
            applicationTypeName === "NEW_ORIGIN_ID" ||
              applicationTypeName === "REISSUE_OID"
              ? "ORIGIN_ID_REASON"
              : "DOCUMENT_ISSUE_REASON",
          ),
          apiClient.getLookupsByType("COUNTRY"),
          apiClient.getLookupsByType("VISA_PERMIT_VALIDITY_PERIOD"),
          apiClient.getLookupsByType("PERSONALIZED_DOCUMENT_STATUS"),
          apiClient.getLookupsByType("SUPPORTING_DOCUMENT_STATUS"),
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
        setReasonForIssueOptions(
          reasonData.map((lookup) => ({
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

        // Set default country to Ethiopia
        const ethiopiaCountry = countryData.find(
          (lookup) => lookup.lookupvalue === "ETHIOPIA",
        );
        if (ethiopiaCountry) {
          setNewDocumentForm((prev) => ({
            ...prev,
            issuingcountrylookupid: ethiopiaCountry.tglookupid,
            issuingauthority: "IMMIGRATION AND CITIZENSHIP SERVICE",
          }));
        }
        setValidityPeriodOptions(
          validityData.map((lookup) => ({
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
    if (!newDocumentForm.documenttypelookupid || false) {
      toast.error("Please fill in all required fields");
      return;
    }

    setIsSubmitting(true);
    try {
      const createData = {
        tgpersonid: personId,
        tgapplicationid: applicationId,
        documenttypelookupid: newDocumentForm.documenttypelookupid,
        documentnumber: newDocumentForm.documentnumber || null,
        documentserialnumber: newDocumentForm.documentserialnumber || null,
        issuedate: newDocumentForm.issuedate || null,
        validfromdate: newDocumentForm.validfromdate || null,
        expirydate: newDocumentForm.expirydate || null,
        reasonforissuelookupid: newDocumentForm.reasonforissuelookupid || null,
        reasonforissueother: newDocumentForm.reasonforissueother || null,
        placeofprinting: newDocumentForm.placeofprinting || null,
        issuingauthority: newDocumentForm.issuingauthority || null,
        issuingcountrylookupid: newDocumentForm.issuingcountrylookupid || null,
        validityperiodlookupid: newDocumentForm.validityperiodlookupid || null,
        validityperiod: newDocumentForm.validityperiod || null,
        visamultipleentry: newDocumentForm.visamultipleentry ? 1 : 0,
      };

      const result = await apiClient.createDocument(createData);

      // Refresh documents to get the newly created document with proper data
      const data = await apiClient.getDocuments(personId, applicationId);
      setDocuments(data);
      setShowNewDocumentModal(false);

      // Reset form with defaults
      const ethiopiaCountry = countryOptions.find(
        (c) => c.value === "ETHIOPIA",
      );
      setNewDocumentForm({
        documenttypelookupid: null,
        documentnumber: "",
        documentserialnumber: "",
        issuedate: "",
        validfromdate: "",
        expirydate: "",
        reasonforissuelookupid: null,
        reasonforissueother: "",
        placeofprinting: "",
        issuingauthority: "IMMIGRATION AND CITIZENSHIP SERVICE",
        issuingcountrylookupid: ethiopiaCountry?.id || null,
        validityperiodlookupid: null,
        validityperiod: "",
        visamultipleentry: false,
      });

      toast.success("Document added successfully");
    } catch (error) {
      toast.error("Failed to add document");
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleVoid = async (id: number) => {
    const confirmed = window.confirm(
      "Are you sure you want to void this document?",
    );
    if (!confirmed) return;

    try {
      await apiClient.voidDocument(id);

      // Update in current documents
      setDocuments((prev) =>
        prev.map((doc) =>
          doc.tgpersonissueddocumentid === id
            ? { ...doc, isactive: 0, modifieddate: new Date() }
            : doc,
        ),
      );

      // Update in previous documents as well
      setPreviousDocuments((prev) =>
        prev.map((doc) =>
          doc.tgpersonissueddocumentid === id
            ? { ...doc, isactive: 0, modifieddate: new Date() }
            : doc,
        ),
      );

      toast.success("Document voided successfully");
    } catch (error) {
      toast.error("Failed to void document");
    }
  };

  const handleVoidSupportingDocument = async (id: number) => {
    const confirmed = window.confirm(
      "Are you sure you want to void this supporting document?",
    );
    if (!confirmed) return;

    try {
      await apiClient.voidSupportingDocument(id);

      // Update supporting documents state
      setSupportingDocuments((prev) =>
        prev.map((doc) =>
          doc.tgpersonsupportingdocumentid === id
            ? { ...doc, isactive: 0, modifieddate: new Date() }
            : doc,
        ),
      );

      toast.success("Supporting document voided successfully");
    } catch (error) {
      toast.error("Failed to void supporting document");
    }
  };

  const handleEditDocument = async () => {
    if (!editingDocument) return;

    try {
      setIsSubmitting(true);

      const updateData = {
        documenttypelookupid: editForm.documenttypelookupid,
        personalizeddocumentstatuslookupid:
          editForm.personalizeddocumentstatuslookupid,
        documentnumber: editForm.documentnumber,
        documentserialnumber: editForm.documentserialnumber,
        issuedate: editForm.issuedate,
        validfromdate: editForm.validfromdate,
        expirydate: editForm.expirydate,
        reasonforissuelookupid: editForm.reasonforissuelookupid,
        reasonforissueother: editForm.reasonforissueother,
        placeofprinting: editForm.placeofprinting,
        issuingauthority: editForm.issuingauthority,
        issuingcountrylookupid: editForm.issuingcountrylookupid,
        validityperiodlookupid: editForm.validityperiodlookupid,
        validityperiod: editForm.validityperiod,
        visamultipleentry: editForm.visamultipleentry ? 1 : 0,
      };

      await apiClient.updateDocument(editingDocument, updateData);

      // Update in current documents
      setDocuments((prev) =>
        prev.map((doc) =>
          doc.tgpersonissueddocumentid === editingDocument
            ? ({
                ...doc,
                ...updateData,
                issuedate: updateData.issuedate
                  ? new Date(updateData.issuedate)
                  : undefined,
                validfromdate: updateData.validfromdate
                  ? new Date(updateData.validfromdate)
                  : undefined,
                expirydate: updateData.expirydate
                  ? new Date(updateData.expirydate)
                  : undefined,
                modifieddate: new Date(),
              } as DocumentRecord)
            : doc,
        ),
      );

      // Update in previous documents as well
      setPreviousDocuments((prev) =>
        prev.map((doc) =>
          doc.tgpersonissueddocumentid === editingDocument
            ? ({
                ...doc,
                ...updateData,
                issuedate: updateData.issuedate
                  ? new Date(updateData.issuedate)
                  : undefined,
                validfromdate: updateData.validfromdate
                  ? new Date(updateData.validfromdate)
                  : undefined,
                expirydate: updateData.expirydate
                  ? new Date(updateData.expirydate)
                  : undefined,
                modifieddate: new Date(),
              } as DocumentRecord)
            : doc,
        ),
      );

      setEditingDocument(null);
      toast.success("Document updated successfully");
    } catch (error) {
      toast.error("Failed to update document");
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleEditSupportingDocument = async () => {
    if (!editingSupportingDocument) return;

    try {
      setIsSubmitting(true);

      const updateData = {
        documenttypelookupid: supportingEditForm.documenttypelookupid,
        documentcategorylookupid: supportingEditForm.documentcategorylookupid,
        documentstatuslookupid: supportingEditForm.documentstatuslookupid,
        documentobject: supportingEditForm.documentobject,
        remark: supportingEditForm.remark,
      };

      await apiClient.updateSupportingDocument(
        editingSupportingDocument,
        updateData,
      );

      // Update in current supporting documents
      setSupportingDocuments((prev) =>
        prev.map((doc) =>
          doc.tgpersonsupportingdocumentid === editingSupportingDocument
            ? ({
                ...doc,
                ...updateData,
                modifieddate: new Date(),
              } as SupportingDocumentRecord)
            : doc,
        ),
      );

      toast.success("Supporting document updated successfully");
      setEditingSupportingDocument(null);
    } catch (error) {
      toast.error("Failed to update supporting document");
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleCreateSupportingDocument = async () => {
    if (!newSupportingDocumentForm.documenttypelookupid) return;

    try {
      setIsSubmitting(true);

      const createData = {
        tgpersonid: personId,
        tgapplicationid: applicationId,
        documenttypelookupid: newSupportingDocumentForm.documenttypelookupid,
        documentcategorylookupid:
          newSupportingDocumentForm.documentcategorylookupid,
        documentstatuslookupid:
          newSupportingDocumentForm.documentstatuslookupid,
        documentobject: newSupportingDocumentForm.documentobject,
        remark: newSupportingDocumentForm.remark,
      };

      await apiClient.createSupportingDocument(createData);

      // Refresh supporting documents
      const response = await apiClient.getSupportingDocuments(
        personId,
        applicationId,
      );
      setSupportingDocuments(response || []);

      setShowNewSupportingDocumentModal(false);
      setNewSupportingDocumentForm({
        documenttypelookupid: null,
        documentcategorylookupid: null,
        documentstatuslookupid: null,
        documentobject: null,
        remark: "",
      });
      toast.success("Supporting document created successfully");
    } catch (error) {
      toast.error("Failed to create supporting document");
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleStartEdit = (doc: DocumentRecord) => {
    setEditForm({
      documenttypelookupid: doc.documenttypelookupid || null,
      personalizeddocumentstatuslookupid:
        doc.personalizeddocumentstatuslookupid || null,
      documentnumber: doc.documentnumber || "",
      documentserialnumber: doc.documentserialnumber || "",
      issuedate: doc.issuedate
        ? doc.issuedate instanceof Date
          ? doc.issuedate.toISOString().split("T")[0]
          : new Date(doc.issuedate).toISOString().split("T")[0]
        : "",
      validfromdate: doc.validfromdate
        ? doc.validfromdate instanceof Date
          ? doc.validfromdate.toISOString().split("T")[0]
          : new Date(doc.validfromdate).toISOString().split("T")[0]
        : "",
      expirydate: doc.expirydate
        ? doc.expirydate instanceof Date
          ? doc.expirydate.toISOString().split("T")[0]
          : new Date(doc.expirydate).toISOString().split("T")[0]
        : "",
      reasonforissuelookupid: doc.reasonforissuelookupid || null,
      reasonforissueother: doc.reasonforissueother || "",
      placeofprinting: doc.placeofprinting || "",
      issuingauthority: doc.issuingauthority || "",
      issuingcountrylookupid: doc.issuingcountrylookupid || null,
      validityperiodlookupid: doc.validityperiodlookupid || null,
      validityperiod: doc.validityperiod || "",
      visamultipleentry: doc.visamultipleentry === 1,
    });
    setEditingDocument(doc.tgpersonissueddocumentid);
  };

  const handleStartEditSupportingDocument = (doc: SupportingDocumentRecord) => {
    setSupportingEditForm({
      documenttypelookupid: doc.documenttypelookupid || null,
      documentcategorylookupid: doc.documentcategorylookupid || null,
      documentstatuslookupid: doc.documentstatuslookupid || null,
      documentobject: doc.documentobject || null,
      remark: doc.remark || "",
    });
    setEditingSupportingDocument(doc.tgpersonsupportingdocumentid);
  };

  const handleViewDocument = async (documentId: number) => {
    try {
      setLoadingDocument(true);
      setViewingDocument(documentId);

      const documentResponse = await apiClient.getDocumentData(documentId);

      if (documentResponse.document) {
        setDocumentData(documentResponse.document);
      } else {
        throw new Error("No document data available");
      }
    } catch (error) {
      toast.error("Failed to load document");
      setViewingDocument(null);
    } finally {
      setLoadingDocument(false);
    }
  };

  const handleCloseDocument = () => {
    setViewingDocument(null);
    setDocumentData(null);
  };

  const handleViewSupportingDocument = async (documentId: number) => {
    try {
      setLoadingDocument(true);
      setViewingDocument(documentId);

      const documentResponse =
        await apiClient.getSupportingDocumentData(documentId);

      if (documentResponse.document) {
        setDocumentData(documentResponse.document);
      } else {
        throw new Error("No document data available");
      }
    } catch (error) {
      toast.error("Failed to load supporting document");
      setViewingDocument(null);
    } finally {
      setLoadingDocument(false);
    }
  };

  const refetchAllData = async () => {
    try {
      setIsLoading(true);
      setError(null);
      const data = await apiClient.getDocuments(personId, applicationId);
      const processedData = data.map((doc: any) => ({
        ...doc,
        issuedate: doc.issuedate ? new Date(doc.issuedate) : undefined,
        validfromdate: doc.validfromdate
          ? new Date(doc.validfromdate)
          : undefined,
        expirydate: doc.expirydate ? new Date(doc.expirydate) : undefined,
        createdate: new Date(doc.createdate),
        modifieddate: new Date(doc.modifieddate),
      }));
      setDocuments(processedData);

      setSupportingDocsLoading(true);
      setSupportingDocsError(null);
      const supportingData = await apiClient.getSupportingDocuments(
        personId,
        applicationId,
      );
      const processedSupportingData = supportingData.map((doc: any) => ({
        ...doc,
        createdate: new Date(doc.createdate),
        modifieddate: doc.modifieddate ? new Date(doc.modifieddate) : undefined,
      }));
      setSupportingDocuments(processedSupportingData);

      setPreviousDocsLoading(true);
      setPreviousDocsError(null);
      const previousData = await apiClient.getPreviousDocuments(
        personId,
        applicationId,
      );
      const processedPreviousData = previousData.map((doc: any) => ({
        ...doc,
        issuedate: doc.issuedate ? new Date(doc.issuedate) : undefined,
        validfromdate: doc.validfromdate
          ? new Date(doc.validfromdate)
          : undefined,
        expirydate: doc.expirydate ? new Date(doc.expirydate) : undefined,
        createdate: new Date(doc.createdate),
        modifieddate: new Date(doc.modifieddate),
      }));
      setPreviousDocuments(processedPreviousData);

      toast.success("Data refreshed successfully");
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : "Failed to refresh data";
      setError(errorMessage);
      setSupportingDocsError(errorMessage);
      setPreviousDocsError(errorMessage);
      toast.error(errorMessage);
    } finally {
      setIsLoading(false);
      setSupportingDocsLoading(false);
      setPreviousDocsLoading(false);
    }
  };

  const MetaStrip = ({ record }: { record: DocumentRecord }) => (
    <div className="flex items-center gap-3 text-[10px] text-gray-500">
      <span className="font-medium">ID: {record.tgpersonissueddocumentid}</span>
      <span className="text-gray-400">•</span>
      <span className="font-medium">{formatDate(record.createdate)}</span>
      <span className="text-gray-400">•</span>
      <span className="font-medium">{formatDate(record.modifieddate)}</span>
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

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="flex items-center gap-3">
          <Loader2 className="w-6 h-6 animate-spin text-blue-600" />
          <span className="text-gray-600">Loading documents...</span>
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
            Failed to Load Documents
          </h3>
          <p className="text-gray-600 mb-4">{error}</p>
          <button
            onClick={refetchAllData}
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
      {/* Document Tabs */}
      <div className="border-b border-gray-200">
        <nav className="flex space-x-8">
          <button
            onClick={() => setActiveTab("issued")}
            className={cn(
              "py-2 px-1 border-b-2 font-medium text-sm flex items-center gap-2",
              activeTab === "issued"
                ? "border-blue-500 text-blue-600"
                : "border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300",
            )}
          >
            <FileCheck className="w-4 h-4" />
            Issued Documents ({documents.length})
          </button>
          <button
            onClick={() => setActiveTab("supporting")}
            className={cn(
              "py-2 px-1 border-b-2 font-medium text-sm flex items-center gap-2",
              activeTab === "supporting"
                ? "border-blue-500 text-blue-600"
                : "border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300",
            )}
          >
            <Files className="w-4 h-4" />
            Supporting Documents ({supportingDocuments.length})
          </button>
          <button
            onClick={() => setActiveTab("previous")}
            className={cn(
              "py-2 px-1 border-b-2 font-medium text-sm flex items-center gap-2",
              activeTab === "previous"
                ? "border-blue-500 text-blue-600"
                : "border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300",
            )}
          >
            <Archive className="w-4 h-4" />
            Previously Issued ({previousDocuments.length})
          </button>
        </nav>
      </div>

      {/* Tab Content */}
      {activeTab === "issued" ? (
        <div className="space-y-4">
          {/* Header for Issued Documents */}
          <div className="flex items-center justify-between">
            <h2 className="text-sm font-semibold flex items-center gap-2">
              <FileCheck className="w-4 h-4" />
              Issued Documents ({documents.length})
            </h2>
            <button
              onClick={() => setShowNewDocumentModal(true)}
              className="compact-button bg-primary text-white flex items-center gap-1"
            >
              <Plus className="w-3 h-3" />
              Add Document
            </button>
          </div>

          {/* Documents Table */}
          {documents.length > 0 && (
            <div className="border border-gray-200 rounded-lg overflow-hidden">
              <table className="data-table w-full">
                <thead>
                  <tr>
                    <th>Type</th>
                    <th>Document Number</th>
                    <th>Serial Number</th>
                    <th>Issue Date</th>
                    <th>Expiry Date</th>
                    <th>Issuing Authority</th>
                    <th>Document</th>
                    <th>Status</th>
                    <th>Meta</th>
                    <th>Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {documents.map((doc) => (
                    <tr
                      key={doc.tgpersonissueddocumentid}
                      className={cn(
                        "hover:bg-gray-50",
                        doc.isactive === 0 && "opacity-60",
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
                      <td className="font-mono text-xs">
                        {doc.documentnumber || "-"}
                      </td>
                      <td className="font-mono text-xs">
                        {doc.documentserialnumber || "-"}
                      </td>
                      <td className="text-xs">
                        {doc.issuedate ? formatDateCompact(doc.issuedate) : "-"}
                      </td>
                      <td>
                        <div className="flex items-center gap-1">
                          <span className="text-xs">
                            {doc.expirydate
                              ? formatDateCompact(doc.expirydate)
                              : "-"}
                          </span>
                          {isExpired(doc.expirydate) && (
                            <span className="bg-red-100 text-red-800 px-1 py-0.5 rounded text-[9px] font-medium">
                              Expired
                            </span>
                          )}
                          {isExpiringSoon(doc.expirydate) && (
                            <span className="bg-yellow-100 text-yellow-800 px-1 py-0.5 rounded text-[9px] font-medium">
                              Expiring
                            </span>
                          )}
                        </div>
                      </td>
                      <td className="text-xs">{doc.issuingauthority || "-"}</td>
                      <td>
                        <div className="flex items-center justify-center">
                          {doc.has_document ? (
                            <span className="bg-green-100 text-green-800 px-2 py-1 rounded text-[10px] font-medium">
                              Available
                            </span>
                          ) : (
                            <span className="bg-gray-100 text-gray-600 px-2 py-1 rounded text-[10px] font-medium">
                              No Content
                            </span>
                          )}
                        </div>
                      </td>
                      <td>
                        <div className="space-y-0.5">
                          {doc.personalizeddocumentstatuslookupid ? (
                            <>
                              <LookupField
                                lookupId={
                                  doc.personalizeddocumentstatuslookupid
                                }
                                format="name"
                                className="text-xs"
                              />
                              <div className="text-[10px] text-gray-500">
                                ID: {doc.personalizeddocumentstatuslookupid}
                              </div>
                            </>
                          ) : (
                            <span className="text-xs text-gray-500">-</span>
                          )}
                        </div>
                      </td>
                      <td>
                        <MetaStrip record={doc} />
                      </td>
                      <td>
                        <div className="flex gap-1">
                          <button
                            onClick={() => handleStartEdit(doc)}
                            className="p-1 hover:bg-gray-100 rounded"
                            title="Edit"
                          >
                            <Edit2 className="w-3 h-3" />
                          </button>
                          {doc.has_document && (
                            <button
                              onClick={() =>
                                handleViewDocument(doc.tgpersonissueddocumentid)
                              }
                              className="p-1 hover:bg-gray-100 rounded text-green-600"
                              title="Document Available (Encrypted)"
                            >
                              <FileText className="w-3 h-3" />
                            </button>
                          )}
                          {doc.isactive === 1 && (
                            <button
                              onClick={() =>
                                handleVoid(doc.tgpersonissueddocumentid)
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
          {documents.length === 0 && (
            <div className="bg-white border border-gray-200 rounded-lg p-8 text-center">
              <FileText className="w-12 h-12 mx-auto mb-3 text-gray-400" />
              <p className="text-sm text-gray-600 font-medium">
                No documents found
              </p>
              <button
                onClick={() => setShowNewDocumentModal(true)}
                className="mt-3 compact-button bg-primary text-white"
              >
                Add First Document
              </button>
            </div>
          )}

          {/* Edit Modal */}
          {editingDocument && (
            <div
              className="fixed inset-0 bg-black bg-opacity-75 flex items-center justify-center z-50"
              onClick={() => setEditingDocument(null)}
            >
              <div
                className="bg-white rounded-lg p-6 max-w-2xl w-full mx-4"
                onClick={(e) => e.stopPropagation()}
              >
                <h3 className="text-sm font-semibold mb-4">Edit Document</h3>

                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <label className="block text-xs font-medium text-gray-600 mb-1">
                      Document Type <span className="text-red-500">*</span>
                    </label>
                    <SearchableSelect
                      options={documentTypeOptions}
                      value={editForm.documenttypelookupid}
                      onChange={(value) =>
                        setEditForm((prev) => ({
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
                      value={editForm.personalizeddocumentstatuslookupid}
                      onChange={(value) =>
                        setEditForm((prev) => ({
                          ...prev,
                          personalizeddocumentstatuslookupid: value,
                        }))
                      }
                      placeholder="Select status..."
                      loading={loadingOptions}
                      className="w-full"
                    />
                  </div>

                  <div>
                    <label className="block text-xs font-medium text-gray-600 mb-1">
                      Document Number
                    </label>
                    <input
                      type="text"
                      value={editForm.documentnumber}
                      onChange={(e) =>
                        setEditForm((prev) => ({
                          ...prev,
                          documentnumber: e.target.value,
                        }))
                      }
                      className="w-full p-2 text-xs border rounded"
                      placeholder="Enter document number..."
                    />
                  </div>

                  <div>
                    <label className="block text-xs font-medium text-gray-600 mb-1">
                      Serial Number
                    </label>
                    <input
                      type="text"
                      value={editForm.documentserialnumber}
                      onChange={(e) =>
                        setEditForm((prev) => ({
                          ...prev,
                          documentserialnumber: e.target.value,
                        }))
                      }
                      className="w-full p-2 text-xs border rounded"
                      placeholder="Enter serial number..."
                    />
                  </div>

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
                      Valid From Date
                    </label>
                    <input
                      type="date"
                      value={editForm.validfromdate}
                      onChange={(e) =>
                        setEditForm((prev) => ({
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

                  <div>
                    <label className="block text-xs font-medium text-gray-600 mb-1">
                      Validity Period
                    </label>
                    <SearchableSelect
                      options={validityPeriodOptions}
                      value={editForm.validityperiodlookupid}
                      onChange={(value) =>
                        setEditForm((prev) => ({
                          ...prev,
                          validityperiodlookupid: value,
                        }))
                      }
                      placeholder="Select validity period..."
                      loading={loadingOptions}
                      className="w-full"
                    />
                  </div>

                  <div>
                    <label className="block text-xs font-medium text-gray-600 mb-1">
                      Reason for Issue
                    </label>
                    <SearchableSelect
                      options={reasonForIssueOptions}
                      value={editForm.reasonforissuelookupid}
                      onChange={(value) =>
                        setEditForm((prev) => ({
                          ...prev,
                          reasonforissuelookupid: value,
                        }))
                      }
                      placeholder="Select reason..."
                      loading={loadingOptions}
                      className="w-full"
                    />
                  </div>

                  <div>
                    <label className="block text-xs font-medium text-gray-600 mb-1">
                      Issuing Authority
                    </label>
                    <input
                      type="text"
                      value={editForm.issuingauthority}
                      onChange={(e) =>
                        setEditForm((prev) => ({
                          ...prev,
                          issuingauthority: e.target.value,
                        }))
                      }
                      className="w-full p-2 text-xs border rounded"
                      placeholder="Enter issuing authority..."
                    />
                  </div>

                  <div>
                    <label className="block text-xs font-medium text-gray-600 mb-1">
                      Issuing Country
                    </label>
                    <SearchableSelect
                      options={countryOptions}
                      value={editForm.issuingcountrylookupid}
                      onChange={(value) =>
                        setEditForm((prev) => ({
                          ...prev,
                          issuingcountrylookupid: value,
                        }))
                      }
                      placeholder="Select country..."
                      loading={loadingOptions}
                      className="w-full"
                    />
                  </div>

                  <div>
                    <label className="block text-xs font-medium text-gray-600 mb-1">
                      Place of Printing
                    </label>
                    <input
                      type="text"
                      value={editForm.placeofprinting}
                      onChange={(e) =>
                        setEditForm((prev) => ({
                          ...prev,
                          placeofprinting: e.target.value,
                        }))
                      }
                      className="w-full p-2 text-xs border rounded"
                      placeholder="Enter place of printing..."
                    />
                  </div>

                  <div>
                    <label className="flex items-center text-xs font-medium text-gray-600">
                      <input
                        type="checkbox"
                        checked={editForm.visamultipleentry}
                        onChange={(e) =>
                          setEditForm((prev) => ({
                            ...prev,
                            visamultipleentry: e.target.checked,
                          }))
                        }
                        className="mr-2"
                      />
                      Visa Multiple Entry
                    </label>
                  </div>
                </div>

                <div className="flex justify-end gap-3 mt-6">
                  <button
                    onClick={() => setEditingDocument(null)}
                    className="px-4 py-2 text-gray-700 bg-gray-100 rounded-lg hover:bg-gray-200 focus:ring-2 focus:ring-gray-500"
                    disabled={isSubmitting}
                  >
                    Cancel
                  </button>
                  <button
                    onClick={handleEditDocument}
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
      ) : activeTab === "supporting" ? (
        <div className="space-y-4">
          {/* Header for Supporting Documents */}
          <div className="flex items-center justify-between">
            <h2 className="text-sm font-semibold flex items-center gap-2">
              <Files className="w-4 h-4" />
              Supporting Documents ({supportingDocuments.length})
            </h2>
            <button
              className="compact-button bg-primary text-white flex items-center gap-1"
              onClick={() => setShowNewSupportingDocumentModal(true)}
            >
              <FilePlus className="w-3 h-3" />
              Add Supporting Document
            </button>
          </div>

          {/* Supporting Documents Content */}
          {supportingDocsLoading ? (
            <div className="flex items-center justify-center h-32">
              <div className="flex items-center gap-3">
                <Loader2 className="w-6 h-6 animate-spin text-blue-600" />
                <span className="text-gray-600">
                  Loading supporting documents...
                </span>
              </div>
            </div>
          ) : supportingDocsError ? (
            <div className="flex items-center justify-center h-32">
              <div className="text-center">
                <AlertCircle className="w-12 h-12 text-red-500 mx-auto mb-4" />
                <h3 className="text-lg font-semibold text-gray-900 mb-2">
                  Failed to Load Supporting Documents
                </h3>
                <p className="text-gray-600">{supportingDocsError}</p>
              </div>
            </div>
          ) : supportingDocuments.length === 0 ? (
            <div className="text-center py-12 bg-gray-50 rounded-lg border-2 border-dashed border-gray-200">
              <Files className="w-12 h-12 text-gray-400 mx-auto mb-4" />
              <p className="text-gray-600 font-medium">
                No supporting documents found
              </p>
              <p className="text-sm text-gray-500 mt-1">
                Supporting documents will appear here when available
              </p>
            </div>
          ) : (
            <div className="bg-white border border-gray-200 rounded-lg overflow-hidden">
              <div className="overflow-x-auto">
                <table className="data-table w-full">
                  <thead>
                    <tr>
                      <th>Document Type</th>
                      <th>Category</th>
                      <th>Status</th>
                      <th>Application</th>
                      <th>Remark</th>
                      <th>Document</th>
                      <th>Meta</th>
                      <th>Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {supportingDocuments.map((doc) => (
                      <tr
                        key={doc.tgpersonsupportingdocumentid}
                        className={cn(
                          "hover:bg-gray-50",
                          doc.isactive === 0 && "opacity-60",
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
                                className="text-xs"
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
                                className="text-xs"
                              />
                              <div className="text-[10px] text-gray-500">
                                ID: {doc.documentstatuslookupid}
                              </div>
                            </div>
                          ) : (
                            <span className="text-xs text-gray-500">-</span>
                          )}
                        </td>
                        <td>
                          {doc.tgapplicationid ? (
                            <span className="text-xs font-mono">
                              {doc.tgapplicationid}
                            </span>
                          ) : (
                            <span className="text-xs text-gray-500">-</span>
                          )}
                        </td>
                        <td className="max-w-32">
                          {doc.remark ? (
                            <span
                              className="text-xs text-gray-700 truncate block"
                              title={doc.remark}
                            >
                              {doc.remark}
                            </span>
                          ) : (
                            <span className="text-xs text-gray-500">-</span>
                          )}
                        </td>
                        <td>
                          <div className="flex items-center justify-center">
                            {doc.has_document ? (
                              <span className="bg-green-100 text-green-800 px-2 py-1 rounded text-[10px] font-medium">
                                Available
                              </span>
                            ) : (
                              <span className="bg-gray-100 text-gray-600 px-2 py-1 rounded text-[10px] font-medium">
                                No Content
                              </span>
                            )}
                          </div>
                        </td>
                        <td>
                          <div className="flex items-center gap-3 text-[10px] text-gray-500">
                            <span className="font-medium">
                              ID: {doc.tgpersonsupportingdocumentid}
                            </span>
                            <span className="text-gray-400">•</span>
                            <span className="font-medium">
                              {formatDate(doc.createdate)}
                            </span>
                            <span className="text-gray-400">•</span>
                            <span className="font-medium">
                              {doc.modifieddate
                                ? formatDate(doc.modifieddate)
                                : "-"}
                            </span>
                            <span className="text-gray-400">•</span>
                            <span
                              className={cn(
                                "px-1.5 py-0.5 rounded text-[9px] font-medium",
                                doc.isactive === 1
                                  ? "bg-green-100 text-green-700"
                                  : "bg-gray-100 text-gray-600",
                              )}
                            >
                              {doc.isactive === 1 ? "Active" : "Voided"}
                            </span>
                          </div>
                        </td>
                        <td>
                          <div className="flex gap-1">
                            <button
                              onClick={() =>
                                handleStartEditSupportingDocument(doc)
                              }
                              className="p-1 hover:bg-gray-100 rounded"
                              title="Edit"
                            >
                              <Edit2 className="w-3 h-3" />
                            </button>
                            {doc.has_document && (
                              <button
                                onClick={() =>
                                  handleViewSupportingDocument(
                                    doc.tgpersonsupportingdocumentid,
                                  )
                                }
                                className="p-1 hover:bg-gray-100 rounded text-green-600"
                                title="Document Available (Encrypted)"
                              >
                                <FileText className="w-3 h-3" />
                              </button>
                            )}
                            {doc.isactive === 1 && (
                              <button
                                onClick={() =>
                                  handleVoidSupportingDocument(
                                    doc.tgpersonsupportingdocumentid,
                                  )
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
            </div>
          )}
        </div>
      ) : activeTab === "previous" ? (
        <div className="space-y-4">
          {/* Header for Previously Issued Documents */}
          <div className="flex items-center justify-between">
            <h2 className="text-sm font-semibold flex items-center gap-2">
              <Archive className="w-4 h-4" />
              Previously Issued Documents ({previousDocuments.length})
            </h2>
          </div>

          {/* Previously Issued Documents Content */}
          {previousDocsLoading ? (
            <div className="flex items-center justify-center h-32">
              <div className="flex items-center gap-3">
                <Loader2 className="w-6 h-6 animate-spin text-blue-600" />
                <span className="text-gray-600">
                  Loading previous documents...
                </span>
              </div>
            </div>
          ) : previousDocsError ? (
            <div className="flex items-center justify-center h-32">
              <div className="text-center">
                <AlertCircle className="w-12 h-12 text-red-500 mx-auto mb-4" />
                <h3 className="text-lg font-semibold text-gray-900 mb-2">
                  Failed to Load Previous Documents
                </h3>
                <p className="text-gray-600">{previousDocsError}</p>
              </div>
            </div>
          ) : previousDocuments.length === 0 ? (
            <div className="text-center py-12 bg-gray-50 rounded-lg border-2 border-dashed border-gray-200">
              <Archive className="w-12 h-12 text-gray-400 mx-auto mb-4" />
              <p className="text-gray-600 font-medium">
                No previous documents found
              </p>
              <p className="text-sm text-gray-500 mt-1">
                Previously issued documents from other applications will appear
                here
              </p>
            </div>
          ) : (
            <div className="border rounded-lg overflow-hidden">
              <table className="data-table w-full">
                <thead>
                  <tr>
                    <th>Application ID</th>
                    <th>Type</th>
                    <th>Document Number</th>
                    <th>Serial Number</th>
                    <th>Issue Date</th>
                    <th>Expiry Date</th>
                    <th>Issuing Authority</th>
                    <th>Document</th>
                    <th>Status</th>
                    <th>Meta</th>
                    <th>Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {previousDocuments.map((doc) => (
                    <tr
                      key={doc.tgpersonissueddocumentid}
                      className={cn(
                        "hover:bg-gray-50",
                        doc.isactive === 0 && "opacity-60",
                      )}
                    >
                      <td className="font-mono text-xs text-blue-600">
                        {doc.tgapplicationid || "-"}
                      </td>
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
                      <td className="font-mono text-xs">
                        {doc.documentnumber || "-"}
                      </td>
                      <td className="font-mono text-xs">
                        {doc.documentserialnumber || "-"}
                      </td>
                      <td className="text-xs">
                        {doc.issuedate ? formatDateCompact(doc.issuedate) : "-"}
                      </td>
                      <td>
                        <div className="flex items-center gap-1">
                          <span className="text-xs">
                            {doc.expirydate
                              ? formatDateCompact(doc.expirydate)
                              : "-"}
                          </span>
                          {isExpired(doc.expirydate) && (
                            <span className="bg-red-100 text-red-800 px-1 py-0.5 rounded text-[9px] font-medium">
                              Expired
                            </span>
                          )}
                          {isExpiringSoon(doc.expirydate) && (
                            <span className="bg-yellow-100 text-yellow-800 px-1 py-0.5 rounded text-[9px] font-medium">
                              Expiring
                            </span>
                          )}
                        </div>
                      </td>
                      <td className="text-xs">{doc.issuingauthority || "-"}</td>
                      <td>
                        <div className="flex items-center justify-center">
                          {doc.has_document ? (
                            <span className="bg-green-100 text-green-800 px-2 py-1 rounded text-[10px] font-medium">
                              Available
                            </span>
                          ) : (
                            <span className="bg-gray-100 text-gray-600 px-2 py-1 rounded text-[10px] font-medium">
                              No Content
                            </span>
                          )}
                        </div>
                      </td>
                      <td>
                        <div className="space-y-0.5">
                          {doc.personalizeddocumentstatuslookupid ? (
                            <>
                              <LookupField
                                lookupId={
                                  doc.personalizeddocumentstatuslookupid
                                }
                                format="name"
                                className="text-xs"
                              />
                              <div className="text-[10px] text-gray-500">
                                ID: {doc.personalizeddocumentstatuslookupid}
                              </div>
                            </>
                          ) : (
                            <span className="text-xs text-gray-500">-</span>
                          )}
                        </div>
                      </td>
                      <td>
                        <MetaStrip record={doc} />
                      </td>
                      <td>
                        <div className="flex gap-1">
                          <button
                            onClick={() => handleStartEdit(doc)}
                            className="p-1 hover:bg-gray-100 rounded"
                            title="Edit"
                          >
                            <Edit2 className="w-3 h-3" />
                          </button>
                          {doc.has_document && (
                            <button
                              onClick={() =>
                                handleViewDocument(doc.tgpersonissueddocumentid)
                              }
                              className="p-1 hover:bg-gray-100 rounded text-green-600"
                              title="View Document"
                            >
                              <Eye className="w-3 h-3" />
                            </button>
                          )}
                          {doc.isactive === 1 && (
                            <button
                              onClick={() =>
                                handleVoid(doc.tgpersonissueddocumentid)
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
        </div>
      ) : null}

      {/* Edit Modal */}
      {editingDocument && (
        <div
          className="fixed inset-0 bg-black bg-opacity-75 flex items-center justify-center z-50"
          onClick={() => setEditingDocument(null)}
        >
          <div
            className="bg-white rounded-lg p-6 max-w-2xl w-full mx-4"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="text-sm font-semibold mb-4">Edit Document</h3>

            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Document Type <span className="text-red-500">*</span>
                </label>
                <SearchableSelect
                  options={documentTypeOptions}
                  value={editForm.documenttypelookupid}
                  onChange={(value) =>
                    setEditForm((prev) => ({
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
                  value={editForm.personalizeddocumentstatuslookupid}
                  onChange={(value) =>
                    setEditForm((prev) => ({
                      ...prev,
                      personalizeddocumentstatuslookupid: value,
                    }))
                  }
                  placeholder="Select status..."
                  loading={loadingOptions}
                  className="w-full"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Document Number
                </label>
                <input
                  type="text"
                  value={editForm.documentnumber}
                  onChange={(e) =>
                    setEditForm((prev) => ({
                      ...prev,
                      documentnumber: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-xs border rounded"
                  placeholder="Enter document number..."
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Serial Number
                </label>
                <input
                  type="text"
                  value={editForm.documentserialnumber}
                  onChange={(e) =>
                    setEditForm((prev) => ({
                      ...prev,
                      documentserialnumber: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-xs border rounded"
                  placeholder="Enter serial number..."
                />
              </div>

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

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Issue Reason
                </label>
                <SearchableSelect
                  options={reasonForIssueOptions}
                  value={editForm.reasonforissuelookupid}
                  onChange={(value) =>
                    setEditForm((prev) => ({
                      ...prev,
                      reasonforissuelookupid: value,
                    }))
                  }
                  placeholder="Select issue reason..."
                  loading={loadingOptions}
                  className="w-full"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Issue Reason Other
                </label>
                <input
                  type="text"
                  value={editForm.reasonforissueother}
                  onChange={(e) =>
                    setEditForm((prev) => ({
                      ...prev,
                      reasonforissueother: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-xs border rounded"
                  placeholder="Enter other reason..."
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Valid From Date
                </label>
                <input
                  type="date"
                  value={editForm.validfromdate}
                  onChange={(e) =>
                    setEditForm((prev) => ({
                      ...prev,
                      validfromdate: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-xs border rounded"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Place of Printing
                </label>
                <input
                  type="text"
                  value={editForm.placeofprinting}
                  onChange={(e) =>
                    setEditForm((prev) => ({
                      ...prev,
                      placeofprinting: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-xs border rounded"
                  placeholder="Enter place of printing..."
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Issuing Authority
                </label>
                <input
                  type="text"
                  value={editForm.issuingauthority}
                  onChange={(e) =>
                    setEditForm((prev) => ({
                      ...prev,
                      issuingauthority: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-xs border rounded"
                  placeholder="Enter issuing authority..."
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Issuing Country
                </label>
                <SearchableSelect
                  options={countryOptions}
                  value={editForm.issuingcountrylookupid}
                  onChange={(value) =>
                    setEditForm((prev) => ({
                      ...prev,
                      issuingcountrylookupid: value,
                    }))
                  }
                  placeholder="Select issuing country..."
                  loading={loadingOptions}
                  className="w-full"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Validity Period
                </label>
                <SearchableSelect
                  options={validityPeriodOptions}
                  value={editForm.validityperiodlookupid}
                  onChange={(value) =>
                    setEditForm((prev) => ({
                      ...prev,
                      validityperiodlookupid: value,
                    }))
                  }
                  placeholder="Select validity period..."
                  loading={loadingOptions}
                  className="w-full"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Validity Period Text
                </label>
                <input
                  type="text"
                  value={editForm.validityperiod}
                  onChange={(e) =>
                    setEditForm((prev) => ({
                      ...prev,
                      validityperiod: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-xs border rounded"
                  placeholder="Enter validity period..."
                />
              </div>

              <div className="col-span-2">
                <label className="flex items-center text-xs font-medium text-gray-600">
                  <input
                    type="checkbox"
                    checked={editForm.visamultipleentry}
                    onChange={(e) =>
                      setEditForm((prev) => ({
                        ...prev,
                        visamultipleentry: e.target.checked,
                      }))
                    }
                    className="mr-2"
                  />
                  Visa Multiple Entry
                </label>
              </div>
            </div>

            <div className="flex justify-end gap-3 mt-6">
              <button
                onClick={() => setEditingDocument(null)}
                className="px-4 py-2 text-gray-700 bg-gray-100 rounded-lg hover:bg-gray-200 focus:ring-2 focus:ring-gray-500"
                disabled={isSubmitting}
              >
                Cancel
              </button>
              <button
                onClick={handleEditDocument}
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

      {/* Supporting Document Edit Modal */}
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
                  value={supportingEditForm.documenttypelookupid}
                  onChange={(value) =>
                    setSupportingEditForm((prev) => ({
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
                  value={supportingEditForm.documentcategorylookupid}
                  onChange={(value) =>
                    setSupportingEditForm((prev) => ({
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
                  value={supportingEditForm.documentstatuslookupid}
                  onChange={(value) =>
                    setSupportingEditForm((prev) => ({
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
                          setSupportingEditForm((prev) => ({
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
                  {supportingEditForm.documentobject && (
                    <p className="text-xs text-green-600 mt-2">
                      Document uploaded
                    </p>
                  )}
                </div>
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Remark
                </label>
                <textarea
                  value={supportingEditForm.remark}
                  onChange={(e) =>
                    setSupportingEditForm((prev) => ({
                      ...prev,
                      remark: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-xs border rounded"
                  placeholder="Enter remark..."
                  rows={3}
                />
              </div>
            </div>

            <div className="flex justify-end gap-3 mt-6">
              <button
                onClick={() => setEditingSupportingDocument(null)}
                className="px-4 py-2 text-gray-700 bg-gray-100 rounded-lg hover:bg-gray-200 focus:ring-2 focus:ring-gray-500"
                disabled={isSubmitting}
              >
                Cancel
              </button>
              <button
                onClick={handleEditSupportingDocument}
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

      <DocumentViewer
        isOpen={!!viewingDocument}
        onClose={() => setViewingDocument(null)}
        documentData={documentData}
        loading={loadingDocument}
        documentId={viewingDocument || undefined}
        title="Document"
      />

      {/* New Supporting Document Modal */}
      {showNewSupportingDocumentModal && (
        <div
          className="fixed inset-0 bg-black bg-opacity-75 flex items-center justify-center z-50"
          onClick={() => setShowNewSupportingDocumentModal(false)}
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

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Remark
                </label>
                <textarea
                  value={newSupportingDocumentForm.remark}
                  onChange={(e) =>
                    setNewSupportingDocumentForm((prev) => ({
                      ...prev,
                      remark: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-xs border rounded"
                  placeholder="Enter remark..."
                  rows={3}
                />
              </div>
            </div>

            <div className="flex justify-end gap-3 mt-6">
              <button
                onClick={() => setShowNewSupportingDocumentModal(false)}
                className="px-4 py-2 text-gray-700 bg-gray-100 rounded-lg hover:bg-gray-200 focus:ring-2 focus:ring-gray-500"
                disabled={isSubmitting}
              >
                Cancel
              </button>
              <button
                onClick={handleCreateSupportingDocument}
                className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 focus:ring-2 focus:ring-blue-500 disabled:opacity-50"
                disabled={
                  isSubmitting ||
                  !newSupportingDocumentForm.documenttypelookupid
                }
              >
                {isSubmitting ? (
                  <>
                    <Loader2 className="w-4 h-4 mr-2 animate-spin inline" />
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

      {/* New Document Modal */}
      {showNewDocumentModal && (
        <div
          className="fixed inset-0 bg-black bg-opacity-75 flex items-center justify-center z-50"
          onClick={() => setShowNewDocumentModal(false)}
        >
          <div
            className="bg-white rounded-lg p-6 max-w-4xl w-full mx-4 max-h-[90vh] overflow-y-auto"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="text-sm font-semibold mb-4">Add New Document</h3>

            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Document Type <span className="text-red-500">*</span>
                </label>
                <SearchableSelect
                  options={documentTypeOptions}
                  value={newDocumentForm.documenttypelookupid}
                  onChange={(value) =>
                    setNewDocumentForm((prev) => ({
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
                  Document Number
                </label>
                <input
                  type="text"
                  value={newDocumentForm.documentnumber}
                  onChange={(e) =>
                    setNewDocumentForm((prev) => ({
                      ...prev,
                      documentnumber: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-xs border rounded"
                  placeholder="Enter document number..."
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Serial Number
                </label>
                <input
                  type="text"
                  value={newDocumentForm.documentserialnumber}
                  onChange={(e) =>
                    setNewDocumentForm((prev) => ({
                      ...prev,
                      documentserialnumber: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-xs border rounded"
                  placeholder="Enter serial number..."
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Issue Date
                </label>
                <input
                  type="date"
                  value={newDocumentForm.issuedate}
                  onChange={(e) =>
                    setNewDocumentForm((prev) => ({
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
                  value={newDocumentForm.validfromdate}
                  onChange={(e) =>
                    setNewDocumentForm((prev) => ({
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
                  value={newDocumentForm.expirydate}
                  onChange={(e) =>
                    setNewDocumentForm((prev) => ({
                      ...prev,
                      expirydate: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-xs border rounded"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Validity Period
                </label>
                <SearchableSelect
                  options={validityPeriodOptions}
                  value={newDocumentForm.validityperiodlookupid}
                  onChange={(value) =>
                    setNewDocumentForm((prev) => ({
                      ...prev,
                      validityperiodlookupid: value,
                    }))
                  }
                  placeholder="Select validity period..."
                  loading={loadingOptions}
                  className="w-full"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Reason for Issue
                </label>
                <SearchableSelect
                  options={reasonForIssueOptions}
                  value={newDocumentForm.reasonforissuelookupid}
                  onChange={(value) =>
                    setNewDocumentForm((prev) => ({
                      ...prev,
                      reasonforissuelookupid: value,
                    }))
                  }
                  placeholder="Select reason..."
                  loading={loadingOptions}
                  className="w-full"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Issuing Authority
                </label>
                <input
                  type="text"
                  value={newDocumentForm.issuingauthority}
                  onChange={(e) =>
                    setNewDocumentForm((prev) => ({
                      ...prev,
                      issuingauthority: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-xs border rounded"
                  placeholder="Enter issuing authority..."
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Issuing Country
                </label>
                <SearchableSelect
                  options={countryOptions}
                  value={newDocumentForm.issuingcountrylookupid}
                  onChange={(value) =>
                    setNewDocumentForm((prev) => ({
                      ...prev,
                      issuingcountrylookupid: value,
                    }))
                  }
                  placeholder="Select country..."
                  loading={loadingOptions}
                  className="w-full"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Place of Printing
                </label>
                <input
                  type="text"
                  value={newDocumentForm.placeofprinting}
                  onChange={(e) =>
                    setNewDocumentForm((prev) => ({
                      ...prev,
                      placeofprinting: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-xs border rounded"
                  placeholder="Enter place of printing..."
                />
              </div>

              <div>
                <label className="flex items-center text-xs font-medium text-gray-600">
                  <input
                    type="checkbox"
                    checked={newDocumentForm.visamultipleentry}
                    onChange={(e) =>
                      setNewDocumentForm((prev) => ({
                        ...prev,
                        visamultipleentry: e.target.checked,
                      }))
                    }
                    className="mr-2"
                  />
                  Visa Multiple Entry
                </label>
              </div>
            </div>

            <div className="flex justify-end gap-2 mt-6">
              <button
                onClick={() => {
                  setShowNewDocumentModal(false);
                  const ethiopiaCountry = countryOptions.find(
                    (c) => c.value === "ETHIOPIA",
                  );
                  setNewDocumentForm({
                    documenttypelookupid: null,
                    documentnumber: "",
                    documentserialnumber: "",
                    issuedate: "",
                    validfromdate: "",
                    expirydate: "",
                    reasonforissuelookupid: null,
                    reasonforissueother: "",
                    placeofprinting: "",
                    issuingauthority: "IMMIGRATION AND CITIZENSHIP SERVICE",
                    issuingcountrylookupid: ethiopiaCountry?.id || null,
                    validityperiodlookupid: null,
                    validityperiod: "",
                    visamultipleentry: false,
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
        </div>
      )}
    </div>
  );
}
