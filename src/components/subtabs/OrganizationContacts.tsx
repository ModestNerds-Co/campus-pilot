//
//  campus-pilot
//  OrganizationContacts.tsx
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import React, { useState, useEffect } from "react";
import {
  Users,
  Plus,
  Edit2,
  Trash2,
  Mail,
  Phone,
  Loader2,
  AlertCircle,
  X,
  Save,
  Search,
} from "lucide-react";
import { TgOrganisationContact } from "../../types/organization";
import { apiClient } from "../../lib/api";
import { LookupField } from "../LookupField";
import { SearchableSelect } from "../SearchableSelect";
import { debounce } from "../../lib/utils";
import toast from "react-hot-toast";

interface OrganizationContactsProps {
  organizationId: number;
}

interface NewContactForm {
  tgpersonid: number | null;
  contacttypelookupid: number | null;
  contactstatuslookupid: number | null;
  contactnumber: string;
  alternativecontactnumber: string;
  emailaddress: string;
}

interface EditContactForm extends NewContactForm {
  tgorganisationcontactid: number;
}

const initialContactForm: NewContactForm = {
  tgpersonid: null,
  contacttypelookupid: null,
  contactstatuslookupid: null,
  contactnumber: "",
  alternativecontactnumber: "",
  emailaddress: "",
};

export function OrganizationContacts({
  organizationId,
}: OrganizationContactsProps) {
  const [contacts, setContacts] = useState<TgOrganisationContact[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Modal states
  const [showAddModal, setShowAddModal] = useState(false);
  const [showEditModal, setShowEditModal] = useState(false);
  const [editingContact, setEditingContact] =
    useState<TgOrganisationContact | null>(null);

  // Form states
  const [newContactForm, setNewContactForm] =
    useState<NewContactForm>(initialContactForm);
  const [editContactForm, setEditContactForm] =
    useState<EditContactForm | null>(null);

  // Lookup options
  const [contactTypeOptions, setContactTypeOptions] = useState<any[]>([]);
  const [contactStatusOptions, setContactStatusOptions] = useState<any[]>([]);
  const [personOptions, setPersonOptions] = useState<any[]>([]);
  const [loadingLookups, setLoadingLookups] = useState(false);
  const [loadingPersonSearch, setLoadingPersonSearch] = useState(false);

  // Loading states
  const [savingContact, setSavingContact] = useState(false);

  // Fetch contacts
  useEffect(() => {
    const fetchContacts = async () => {
      try {
        setIsLoading(true);
        setError(null);

        const data = await apiClient.getOrganizationContacts(organizationId);
        const processedData = data.map((contact: any) => ({
          ...contact,
          createdate: new Date(contact.createdate),
          modifieddate: contact.modifieddate
            ? new Date(contact.modifieddate)
            : undefined,
        }));
        setContacts(processedData);
      } catch (err) {
        const errorMessage =
          err instanceof Error
            ? err.message
            : "Failed to load organization contacts";
        setError(errorMessage);
        toast.error(errorMessage);
      } finally {
        setIsLoading(false);
      }
    };

    fetchContacts();
  }, [organizationId]);

  // Fetch lookup options
  useEffect(() => {
    const fetchLookupOptions = async () => {
      try {
        setLoadingLookups(true);

        const [contactTypeData, contactStatusData] = await Promise.all([
          apiClient.getLookupsByType("ORGANISATION_PERSON_TYPE"),
          apiClient.getLookupsByType("ORGANISATION_PERSON_STATUS"),
        ]);

        setContactTypeOptions(
          contactTypeData.map((lookup) => ({
            id: lookup.tglookupid,
            value: lookup.lookupvalue,
            label: lookup.lookupdescription || lookup.lookupvalue,
          })),
        );

        setContactStatusOptions(
          contactStatusData.map((lookup) => ({
            id: lookup.tglookupid,
            value: lookup.lookupvalue,
            label: lookup.lookupdescription || lookup.lookupvalue,
          })),
        );
      } catch (err) {
        console.error("Failed to load lookup options:", err);
        toast.error("Failed to load form options");
      } finally {
        setLoadingLookups(false);
      }
    };

    fetchLookupOptions();
  }, []);

  // Person search
  const searchPersons = async (query: string) => {
    if (query.trim().length >= 2) {
      setLoadingPersonSearch(true);
      try {
        const results = await apiClient.searchPersons(query);
        setPersonOptions(
          results.map((person) => ({
            id: person.tgpersonid,
            value: `${person.firstname} ${person.lastname}`,
            label: `${person.firstname} ${person.lastname}`,
            description: `ID: ${person.tgpersonid}`,
          })),
        );
      } catch (err) {
        console.error("Person search failed:", err);
      } finally {
        setLoadingPersonSearch(false);
      }
    } else {
      setPersonOptions([]);
    }
  };

  // Debounced person search
  const debouncedPersonSearch = debounce(searchPersons, 300);

  // Handle person search input change
  const handlePersonSearchChange = (query: string) => {
    debouncedPersonSearch(query);
  };

  // Form handlers
  const handleNewContactFieldChange = (
    field: keyof NewContactForm,
    value: any,
  ) => {
    setNewContactForm((prev) => ({ ...prev, [field]: value }));
  };

  const handleEditContactFieldChange = (
    field: keyof NewContactForm,
    value: any,
  ) => {
    if (editContactForm) {
      setEditContactForm((prev) => (prev ? { ...prev, [field]: value } : null));
    }
  };

  // Modal handlers
  const openAddModal = () => {
    setNewContactForm(initialContactForm);
    setPersonOptions([]);
    setShowAddModal(true);
  };

  const openEditModal = (contact: TgOrganisationContact) => {
    setEditingContact(contact);
    setEditContactForm({
      tgorganisationcontactid: contact.tgorganisationcontactid,
      tgpersonid: contact.tgpersonid ?? null,
      contacttypelookupid: contact.contacttypelookupid ?? null,
      contactstatuslookupid: contact.contactstatuslookupid ?? null,
      contactnumber: contact.contactnumber || "",
      alternativecontactnumber: contact.alternativecontactnumber || "",
      emailaddress: contact.emailaddress || "",
    });

    // If there's a person associated, add them to options
    if (contact.personfullname && contact.tgpersonid) {
      setPersonOptions([
        {
          id: contact.tgpersonid,
          value: contact.personfullname,
          label: contact.personfullname,
          description: `ID: ${contact.tgpersonid}`,
        },
      ]);
    }

    setShowEditModal(true);
  };

  const closeModals = () => {
    setShowAddModal(false);
    setShowEditModal(false);
    setEditingContact(null);
    setEditContactForm(null);
    setNewContactForm(initialContactForm);
    setPersonOptions([]);
  };

  // CRUD operations
  const validateForm = (form: NewContactForm): string | null => {
    if (!form.tgpersonid) return "Please select a person";
    if (!form.contacttypelookupid) return "Please select a contact type";
    if (!form.contactstatuslookupid) return "Please select a contact status";
    return null;
  };

  const handleAddContact = async () => {
    const validationError = validateForm(newContactForm);
    if (validationError) {
      toast.error(validationError);
      return;
    }

    try {
      setSavingContact(true);
      await apiClient.createOrganizationContact({
        tgorganisationid: organizationId,
        ...newContactForm,
      });

      // Refresh contacts list
      const data = await apiClient.getOrganizationContacts(organizationId);
      const processedData = data.map((contact: any) => ({
        ...contact,
        createdate: new Date(contact.createdate),
        modifieddate: contact.modifieddate
          ? new Date(contact.modifieddate)
          : undefined,
      }));
      setContacts(processedData);

      toast.success("Contact added successfully");
      closeModals();
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : "Failed to add contact";
      toast.error(errorMessage);
    } finally {
      setSavingContact(false);
    }
  };

  const handleUpdateContact = async () => {
    if (!editContactForm) return;

    const validationError = validateForm(editContactForm);
    if (validationError) {
      toast.error(validationError);
      return;
    }

    try {
      setSavingContact(true);
      await apiClient.updateOrganizationContact(
        editContactForm.tgorganisationcontactid,
        editContactForm,
      );

      // Refresh contacts list
      const data = await apiClient.getOrganizationContacts(organizationId);
      const processedData = data.map((contact: any) => ({
        ...contact,
        createdate: new Date(contact.createdate),
        modifieddate: contact.modifieddate
          ? new Date(contact.modifieddate)
          : undefined,
      }));
      setContacts(processedData);

      toast.success("Contact updated successfully");
      closeModals();
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : "Failed to update contact";
      toast.error(errorMessage);
    } finally {
      setSavingContact(false);
    }
  };

  const handleVoidContact = async (contact: TgOrganisationContact) => {
    if (
      !confirm(
        `Are you sure you want to void the contact for ${contact.personfullname}? This action cannot be undone.`,
      )
    ) {
      return;
    }

    try {
      await apiClient.voidOrganizationContact(contact.tgorganisationcontactid);

      // Refresh contacts list
      const data = await apiClient.getOrganizationContacts(organizationId);
      const processedData = data.map((contact: any) => ({
        ...contact,
        createdate: new Date(contact.createdate),
        modifieddate: contact.modifieddate
          ? new Date(contact.modifieddate)
          : undefined,
      }));
      setContacts(processedData);

      toast.success("Contact voided successfully");
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : "Failed to void contact";
      toast.error(errorMessage);
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center p-8">
        <Loader2 className="w-6 h-6 animate-spin" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center p-8 text-center">
        <AlertCircle className="w-12 h-12 text-red-500 mb-4" />
        <h3 className="text-lg font-semibold text-gray-900 mb-2">
          Error Loading Contacts
        </h3>
        <p className="text-gray-600 mb-4">{error}</p>
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
          <Users className="w-5 h-5" />
          Organization Contacts ({contacts.length})
        </h2>
        <button
          onClick={openAddModal}
          className="flex items-center gap-2 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700"
        >
          <Plus className="w-4 h-4" />
          Add Contact
        </button>
      </div>

      {/* Contacts List */}
      {contacts.length === 0 ? (
        <div className="bg-white border border-gray-200 rounded-lg p-8 text-center">
          <Users className="w-12 h-12 mx-auto mb-3 text-gray-400" />
          <p className="text-sm text-gray-600 font-medium">No contacts found</p>
          <button
            onClick={openAddModal}
            className="mt-3 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700"
          >
            Add First Contact
          </button>
        </div>
      ) : (
        <div className="space-y-4">
          {contacts.map((contact) => (
            <div
              key={contact.tgorganisationcontactid}
              className="bg-white rounded-lg border border-gray-200 p-6"
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-4">
                  <div className="w-12 h-12 bg-blue-100 rounded-lg flex items-center justify-center">
                    <Users className="w-6 h-6 text-blue-600" />
                  </div>
                  <div>
                    <div className="flex items-center gap-2">
                      <LookupField
                        lookupId={contact.contacttypelookupid}
                        format="both"
                        className="font-medium text-gray-900"
                        fallback="Contact"
                      />
                      {contact.personfullname && (
                        <>
                          <span className="text-gray-400">•</span>
                          <span className="font-medium text-blue-600">
                            {contact.personfullname}
                          </span>
                        </>
                      )}
                      <span className="text-gray-400">•</span>
                      <LookupField
                        lookupId={contact.contactstatuslookupid}
                        format="value"
                        className="text-sm text-gray-600"
                        fallback="Unknown Status"
                      />
                    </div>
                    <div className="flex items-center gap-4 mt-1 text-sm text-gray-500">
                      {contact.emailaddress && (
                        <div className="flex items-center gap-1">
                          <Mail className="w-3 h-3" />
                          {contact.emailaddress}
                        </div>
                      )}
                      {contact.contactnumber && (
                        <div className="flex items-center gap-1">
                          <Phone className="w-3 h-3" />
                          {contact.contactnumber}
                        </div>
                      )}
                    </div>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <button
                    onClick={() => openEditModal(contact)}
                    className="p-2 text-gray-500 hover:text-blue-600 hover:bg-blue-50 rounded-lg"
                    title="Edit contact"
                  >
                    <Edit2 className="w-4 h-4" />
                  </button>
                  <button
                    onClick={() => handleVoidContact(contact)}
                    className="p-2 text-gray-500 hover:text-red-600 hover:bg-red-50 rounded-lg"
                    title="Void contact"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
              </div>

              {/* Meta Information */}
              <div className="mt-4 pt-4 border-t border-gray-100">
                <div className="grid grid-cols-2 lg:grid-cols-4 gap-4 text-xs">
                  <div>
                    <span className="text-gray-500">Contact ID:</span>
                    <br />
                    <span className="font-mono">
                      {contact.tgorganisationcontactid}
                    </span>
                  </div>
                  {contact.personfullname && (
                    <div>
                      <span className="text-gray-500">Person:</span>
                      <br />
                      <span className="text-sm">{contact.personfullname}</span>
                    </div>
                  )}
                  <div>
                    <span className="text-gray-500">Created:</span>
                    <br />
                    <span>{contact.createdate.toLocaleDateString()}</span>
                  </div>
                  <div>
                    <span className="text-gray-500">Modified:</span>
                    <br />
                    <span>
                      {contact.modifieddate?.toLocaleDateString() || "-"}
                    </span>
                  </div>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Add Contact Modal */}
      {showAddModal && (
        <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
          <div className="bg-white rounded-lg w-full max-w-2xl mx-4 max-h-[90vh] overflow-y-auto">
            <div className="flex items-center justify-between p-6 border-b">
              <h3 className="text-lg font-semibold">Add New Contact</h3>
              <button
                onClick={closeModals}
                className="text-gray-400 hover:text-gray-600"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            <div className="p-6 space-y-6">
              {/* Person Search */}
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-2">
                  Search Person <span className="text-red-500">*</span>
                </label>
                <div className="relative mb-3">
                  <div className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                    <Search className="h-4 w-4 text-gray-400" />
                  </div>
                  <input
                    type="text"
                    onChange={(e) => handlePersonSearchChange(e.target.value)}
                    className="w-full pl-10 pr-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    placeholder="Type at least 2 characters to search persons..."
                  />
                  {loadingPersonSearch && (
                    <div className="absolute inset-y-0 right-0 pr-3 flex items-center">
                      <Loader2 className="h-4 w-4 animate-spin text-gray-400" />
                    </div>
                  )}
                </div>

                {/* Person Selection */}
                <SearchableSelect
                  options={personOptions}
                  value={newContactForm.tgpersonid}
                  onChange={(value) =>
                    handleNewContactFieldChange("tgpersonid", value)
                  }
                  placeholder="Select person from search results..."
                  className="w-full"
                />
              </div>

              {/* Contact Type */}
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-2">
                  Contact Type <span className="text-red-500">*</span>
                </label>
                <SearchableSelect
                  options={contactTypeOptions}
                  value={newContactForm.contacttypelookupid}
                  onChange={(value) =>
                    handleNewContactFieldChange("contacttypelookupid", value)
                  }
                  placeholder="Select contact type..."
                  loading={loadingLookups}
                  className="w-full"
                />
              </div>

              {/* Contact Status */}
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-2">
                  Contact Status <span className="text-red-500">*</span>
                </label>
                <SearchableSelect
                  options={contactStatusOptions}
                  value={newContactForm.contactstatuslookupid}
                  onChange={(value) =>
                    handleNewContactFieldChange("contactstatuslookupid", value)
                  }
                  placeholder="Select contact status..."
                  loading={loadingLookups}
                  className="w-full"
                />
              </div>

              {/* Contact Details */}
              <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">
                    Contact Number
                  </label>
                  <input
                    type="text"
                    value={newContactForm.contactnumber}
                    onChange={(e) =>
                      handleNewContactFieldChange(
                        "contactnumber",
                        e.target.value,
                      )
                    }
                    className="w-full border border-gray-300 rounded-lg px-3 py-2 focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    placeholder="Primary contact number"
                  />
                </div>

                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">
                    Alternative Contact Number
                  </label>
                  <input
                    type="text"
                    value={newContactForm.alternativecontactnumber}
                    onChange={(e) =>
                      handleNewContactFieldChange(
                        "alternativecontactnumber",
                        e.target.value,
                      )
                    }
                    className="w-full border border-gray-300 rounded-lg px-3 py-2 focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    placeholder="Alternative contact number"
                  />
                </div>
              </div>

              <div>
                <label className="block text-sm font-medium text-gray-700 mb-2">
                  Email Address
                </label>
                <input
                  type="email"
                  value={newContactForm.emailaddress}
                  onChange={(e) =>
                    handleNewContactFieldChange("emailaddress", e.target.value)
                  }
                  className="w-full border border-gray-300 rounded-lg px-3 py-2 focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                  placeholder="Email address"
                />
              </div>
            </div>

            <div className="flex items-center justify-end gap-3 p-6 border-t bg-gray-50">
              <button
                onClick={closeModals}
                disabled={savingContact}
                className="px-4 py-2 text-gray-700 bg-white border border-gray-300 rounded-lg hover:bg-gray-50 disabled:opacity-50"
              >
                Cancel
              </button>
              <button
                onClick={handleAddContact}
                disabled={savingContact}
                className="flex items-center gap-2 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50"
              >
                {savingContact && <Loader2 className="w-4 h-4 animate-spin" />}
                <Save className="w-4 h-4" />
                Add Contact
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Edit Contact Modal */}
      {showEditModal && editContactForm && (
        <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
          <div className="bg-white rounded-lg w-full max-w-2xl mx-4 max-h-[90vh] overflow-y-auto">
            <div className="flex items-center justify-between p-6 border-b">
              <h3 className="text-lg font-semibold">Edit Contact</h3>
              <button
                onClick={closeModals}
                className="text-gray-400 hover:text-gray-600"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            <div className="p-6 space-y-6">
              {/* Person Search */}
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-2">
                  Search Person <span className="text-red-500">*</span>
                </label>
                <div className="relative mb-3">
                  <div className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                    <Search className="h-4 w-4 text-gray-400" />
                  </div>
                  <input
                    type="text"
                    onChange={(e) => handlePersonSearchChange(e.target.value)}
                    className="w-full pl-10 pr-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    placeholder="Type at least 2 characters to search persons..."
                  />
                  {loadingPersonSearch && (
                    <div className="absolute inset-y-0 right-0 pr-3 flex items-center">
                      <Loader2 className="h-4 w-4 animate-spin text-gray-400" />
                    </div>
                  )}
                </div>

                {/* Person Selection */}
                <SearchableSelect
                  options={personOptions}
                  value={editContactForm.tgpersonid}
                  onChange={(value) =>
                    handleEditContactFieldChange("tgpersonid", value)
                  }
                  placeholder="Select person from search results..."
                  className="w-full"
                />
              </div>

              {/* Contact Type */}
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-2">
                  Contact Type <span className="text-red-500">*</span>
                </label>
                <SearchableSelect
                  options={contactTypeOptions}
                  value={editContactForm.contacttypelookupid}
                  onChange={(value) =>
                    handleEditContactFieldChange("contacttypelookupid", value)
                  }
                  placeholder="Select contact type..."
                  loading={loadingLookups}
                  className="w-full"
                />
              </div>

              {/* Contact Status */}
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-2">
                  Contact Status <span className="text-red-500">*</span>
                </label>
                <SearchableSelect
                  options={contactStatusOptions}
                  value={editContactForm.contactstatuslookupid}
                  onChange={(value) =>
                    handleEditContactFieldChange("contactstatuslookupid", value)
                  }
                  placeholder="Select contact status..."
                  loading={loadingLookups}
                  className="w-full"
                />
              </div>

              {/* Contact Details */}
              <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">
                    Contact Number
                  </label>
                  <input
                    type="text"
                    value={editContactForm.contactnumber}
                    onChange={(e) =>
                      handleEditContactFieldChange(
                        "contactnumber",
                        e.target.value,
                      )
                    }
                    className="w-full border border-gray-300 rounded-lg px-3 py-2 focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    placeholder="Primary contact number"
                  />
                </div>

                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">
                    Alternative Contact Number
                  </label>
                  <input
                    type="text"
                    value={editContactForm.alternativecontactnumber}
                    onChange={(e) =>
                      handleEditContactFieldChange(
                        "alternativecontactnumber",
                        e.target.value,
                      )
                    }
                    className="w-full border border-gray-300 rounded-lg px-3 py-2 focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    placeholder="Alternative contact number"
                  />
                </div>
              </div>

              <div>
                <label className="block text-sm font-medium text-gray-700 mb-2">
                  Email Address
                </label>
                <input
                  type="email"
                  value={editContactForm.emailaddress}
                  onChange={(e) =>
                    handleEditContactFieldChange("emailaddress", e.target.value)
                  }
                  className="w-full border border-gray-300 rounded-lg px-3 py-2 focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                  placeholder="Email address"
                />
              </div>
            </div>

            <div className="flex items-center justify-end gap-3 p-6 border-t bg-gray-50">
              <button
                onClick={closeModals}
                disabled={savingContact}
                className="px-4 py-2 text-gray-700 bg-white border border-gray-300 rounded-lg hover:bg-gray-50 disabled:opacity-50"
              >
                Cancel
              </button>
              <button
                onClick={handleUpdateContact}
                disabled={savingContact}
                className="flex items-center gap-2 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50"
              >
                {savingContact && <Loader2 className="w-4 h-4 animate-spin" />}
                <Save className="w-4 h-4" />
                Update Contact
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
