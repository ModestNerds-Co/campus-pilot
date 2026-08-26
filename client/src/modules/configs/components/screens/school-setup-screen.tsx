//
//  campus-pilot
//  SchoolSetupScreen.tsx - School configuration screen.
//
//  Created by Ngonidzashe Mangudya on 26/09/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import React, { useState, useEffect } from "react";
import { useNavigate } from "@tanstack/react-router";
import {
  ArrowRight,
  Upload,
  X,
  Loader2,
  AlertCircle,
} from "lucide-react";
import { bootstrapService } from "../../services/bootstrap-service";
import { storageService } from "../../../../lib/storage-service";
import { SchoolPreviewCard } from "../ui/school-preview-card";
import type { SchoolFormData, LogoPreview, FormFieldError } from "../../types";
import {
  TIMEZONE_OPTIONS,
  LOCALE_OPTIONS,
  COUNTRY_OPTIONS,
  DEFAULT_VALUES,
} from "../../constants";
import {
  validateEmail,
  validatePhone,
  validateImage,
  fileToBase64,
} from "../../../../lib/validation";
import { SearchableSelect } from "../../../../components/searchable-select";
import { SetupScaffold } from "../ui/setup-scaffold";
import toast from "react-hot-toast";

export const SchoolSetupScreen: React.FC = () => {
  const navigate = useNavigate();
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [errors, setErrors] = useState<FormFieldError[]>([]);

  // Transform options for SearchableSelect
  const countryOptions = COUNTRY_OPTIONS.map((option, index) => ({
    id: index + 1,
    value: option.value,
    label: option.label,
  }));

  const timezoneOptions = TIMEZONE_OPTIONS.map((option, index) => ({
    id: index + 1,
    value: option.value,
    label: option.label,
  }));

  const localeOptions = LOCALE_OPTIONS.map((option, index) => ({
    id: index + 1,
    value: option.value,
    label: option.label,
  }));

  // Helper functions to convert between SearchableSelect IDs and values
  const getCountryId = (value: string) =>
    countryOptions.find((opt) => opt.value === value)?.id || null;
  const getTimezoneId = (value: string) =>
    timezoneOptions.find((opt) => opt.value === value)?.id || null;
  const getLocaleId = (value: string) =>
    localeOptions.find((opt) => opt.value === value)?.id || null;

  const getCountryValue = (id: number | null) =>
    countryOptions.find((opt) => opt.id === id)?.value || "";
  const getTimezoneValue = (id: number | null) =>
    timezoneOptions.find((opt) => opt.id === id)?.value || "";
  const getLocaleValue = (id: number | null) =>
    localeOptions.find((opt) => opt.id === id)?.value || "";

  const [formData, setFormData] = useState<SchoolFormData>({
    name: "",
    legal_name: "",
    emap_code: "",
    email: "",
    phone: "",
    address_line1: "",
    address_line2: "",
    city: "",
    province: "",
    country: DEFAULT_VALUES.COUNTRY,
    timezone: DEFAULT_VALUES.TIMEZONE,
    locale: DEFAULT_VALUES.LOCALE,
  });

  const [logoPreview, setLogoPreview] = useState<{
    light?: LogoPreview;
    dark?: LogoPreview;
  }>({});

  const updateField = (field: keyof SchoolFormData, value: string) => {
    setFormData((prev) => ({ ...prev, [field]: value }));
    // Clear field-specific errors
    setErrors((prev) => prev.filter((err) => err.field !== field));
  };

  const handleLogoUpload = async (
    type: "light" | "dark",
    file: File | null,
  ) => {
    if (!file) {
      setLogoPreview((prev) => {
        const updated = { ...prev };
        delete updated[type];
        return updated;
      });
      return;
    }

    try {
      const validation = await validateImage(file);
      if (!validation.isValid) {
        toast.error(
          `${type === "light" ? "Light" : "Dark"} logo: ${validation.error}`,
        );
        return;
      }

      if (validation.warnings && validation.warnings.length > 0) {
        validation.warnings.forEach((warning) => toast.error(warning));
      }

      const url = URL.createObjectURL(file);
      setLogoPreview((prev) => ({
        ...prev,
        [type]: {
          file,
          url,
          dimensions: validation.dimensions,
          size: validation.size!,
        },
      }));
    } catch (error) {
      toast.error(`Failed to process ${type} logo`);
    }
  };

  const validateForm = (): boolean => {
    const newErrors: FormFieldError[] = [];

    // Required name validation
    if (!formData.name.trim()) {
      newErrors.push({ field: "name", message: "School name is required" });
    } else if (
      formData.name.trim().length < 2 ||
      formData.name.trim().length > 80
    ) {
      newErrors.push({
        field: "name",
        message: "School name must be between 2 and 80 characters",
      });
    }

    // Optional email validation
    if (formData.email) {
      const emailValidation = validateEmail(formData.email);
      if (!emailValidation.isValid) {
        newErrors.push({ field: "email", message: emailValidation.error! });
      }
    }

    // Optional phone validation
    if (formData.phone) {
      const phoneValidation = validatePhone(formData.phone);
      if (!phoneValidation.isValid) {
        newErrors.push({ field: "phone", message: phoneValidation.error! });
      }
    }

    setErrors(newErrors);
    return newErrors.length === 0;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    if (!validateForm()) {
      toast.error("Please correct the errors below");
      return;
    }

    setIsSubmitting(true);

    try {
      // Upload logos to MinIO and get URLs
      let logo_light_url: string | null = null;
      let logo_dark_url: string | null = null;

      if (logoPreview.light) {
        toast.loading("Uploading light logo...");
        logo_light_url = await storageService.uploadFileWithPresignedUrl(
          logoPreview.light.file,
        );
        toast.dismiss();
      }

      if (logoPreview.dark) {
        toast.loading("Uploading dark logo...");
        logo_dark_url = await storageService.uploadFileWithPresignedUrl(
          logoPreview.dark.file,
        );
        toast.dismiss();
      }

      const schoolConfig = {
        ...formData,
        name: formData.name.trim(),
        legal_name: formData.legal_name.trim() || null,
        emap_code: formData.emap_code.trim() || null,
        email: formData.email.trim() || null,
        phone: formData.phone.trim() || null,
        address_line1: formData.address_line1.trim() || null,
        address_line2: formData.address_line2.trim() || null,
        city: formData.city.trim() || null,
        province: formData.province.trim() || null,
        country: formData.country || null,
        timezone: formData.timezone || null,
        locale: formData.locale || null,
        logo_light_url,
        logo_dark_url,
      };

      const response = await bootstrapService.configureSchool(schoolConfig);

      if (response.success) {
        toast.success(
          response.message || "School configuration saved successfully",
        );
        navigate({ to: "/setup/admin" });
      } else {
        // Handle validation errors from server
        if (response.issues && response.issues.length > 0) {
          const serverErrors = response.issues.map((issue) => ({
            field: issue.field || "general",
            message: issue.detail,
          }));
          setErrors(serverErrors);
        }
        toast.error(response.message || "Failed to save school configuration");
      }
    } catch (error) {
      console.error("School setup failed:", error);
      toast.error(
        error instanceof Error
          ? error.message
          : "Setup failed. Please try again.",
      );
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleSkipLogos = () => {
    // Clear logos and continue
    setLogoPreview({});
    handleSubmit(new Event("submit") as any);
  };

  const getFieldError = (field: string) =>
    errors.find((err) => err.field === field)?.message;

  return (
    <SetupScaffold
      description="Add the identity and operating details Campus Pilot will use across sign-in, reports, receipts, and school records."
      maxWidth="wide"
      step={1}
      title="Set up your school"
    >
        <div className="flex flex-col lg:flex-row gap-8 max-w-6xl mx-auto">
          {/* Form */}
          <div className="flex-1">
            <form onSubmit={handleSubmit} className="space-y-8">
              {/* Branding Section */}
              <div className="bg-[var(--surface)] rounded-[var(--radius-xl)] border border-[var(--border)] p-6 sm:p-8 shadow-[var(--shadow-rest)]">
                <h2 className="text-[length:var(--type-section-title-size)] font-bold text-[var(--text-strong)] mb-6">
                  Branding
                </h2>

                <div className="space-y-6">
                  {/* School Name */}
                  <div>
                    <label
                      htmlFor="name"
                      className="block text-sm font-medium text-[var(--text-strong)] mb-2"
                    >
                      School name *
                    </label>
                    <input
                      id="name"
                      type="text"
                      value={formData.name}
                      onChange={(e) => updateField("name", e.target.value)}
                      data-slot="input" className={`w-full px-4 h-[var(--h-control-md)] rounded-[var(--radius-md)] border focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-0 transition-colors bg-[var(--input-bg)] text-[var(--text-strong)] placeholder:text-[var(--text-subtle)] text-sm ${
                        getFieldError("name")
                          ? "border-[var(--tone-danger)]"
                          : "border-[var(--input-border)]"
                      }`}
                      placeholder="Enter your school's name"
                    />
                    {getFieldError("name") && (
                      <p className="mt-2 text-sm text-[var(--tone-danger-strong)] flex items-center gap-2">
                        <AlertCircle className="w-4 h-4" />
                        {getFieldError("name")}
                      </p>
                    )}
                    <p className="mt-2 text-sm text-[var(--text-muted)]">
                      Shown on login, receipts, and reports.
                    </p>
                  </div>

                  {/* Legal Name */}
                  <div>
                    <label
                      htmlFor="legal_name"
                      className="block text-sm font-medium text-[var(--text-strong)] mb-2"
                    >
                      Registered or legal name
                    </label>
                    <input
                      id="legal_name"
                      type="text"
                      value={formData.legal_name}
                      onChange={(e) =>
                        updateField("legal_name", e.target.value)
                      }
                      data-slot="input" className="w-full px-4 h-[var(--h-control-md)] rounded-[var(--radius-md)] border border-[var(--input-border)] bg-[var(--input-bg)] text-[var(--text-strong)] placeholder:text-[var(--text-subtle)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-0 transition-colors text-sm"
                      placeholder="Official registered name (if different)"
                    />
                  </div>

                  {/* Logo Upload */}
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                    {/* Light Logo */}
                    <div>
                      <label className="block text-sm font-medium text-[var(--text-strong)] mb-2">
                        Light logo
                      </label>
                      <div className="space-y-3">
                        <div className="flex items-center justify-center w-full h-32 border-2 border-dashed border-[var(--border)] rounded-[var(--radius-xl)] hover:border-[var(--border-strong)] bg-[var(--surface)] transition-colors">
                          {logoPreview.light ? (
                            <div className="relative">
                              <img
                                src={logoPreview.light.url}
                                alt="Light logo preview"
                                className="max-h-24 max-w-24 object-contain"
                              />
                              <button
                                type="button"
                                onClick={() => handleLogoUpload("light", null)}
                                className="absolute -top-2 -right-2 w-6 h-6 bg-[var(--tone-danger)] text-[var(--on-brand)] rounded-full flex items-center justify-center hover:bg-[var(--tone-danger-strong)] transition-colors"
                              >
                                <X className="w-3 h-3" />
                              </button>
                            </div>
                          ) : (
                            <label
                              htmlFor="logo-light"
                              className="cursor-pointer text-center"
                            >
                              <Upload className="w-8 h-8 text-[var(--text-subtle)] mx-auto mb-2" />
                              <span className="text-sm text-[var(--text-muted)]">
                                Upload light logo
                              </span>
                            </label>
                          )}
                        </div>
                        <input
                          id="logo-light"
                          type="file"
                          accept="image/png,image/jpeg,image/jpg,image/svg+xml"
                          onChange={(e) => {
                            const file = e.target.files?.[0];
                            if (file) handleLogoUpload("light", file);
                          }}
                          className="hidden"
                        />
                        <p className="text-xs text-[var(--text-muted)]">
                          PNG, JPG, SVG • Max 2MB • Used on light backgrounds
                        </p>
                      </div>
                    </div>

                    {/* Dark Logo */}
                    <div>
                      <label className="block text-sm font-medium text-[var(--text-strong)] mb-2">
                        Dark logo
                      </label>
                      <div className="space-y-3">
                        <div className="flex items-center justify-center w-full h-32 border-2 border-dashed border-[var(--border)] rounded-[var(--radius-xl)] hover:border-[var(--border-strong)] bg-[var(--surface-sunken)] transition-colors">
                          {logoPreview.dark ? (
                            <div className="relative">
                              <img
                                src={logoPreview.dark.url}
                                alt="Dark logo preview"
                                className="max-h-24 max-w-24 object-contain"
                              />
                              <button
                                type="button"
                                onClick={() => handleLogoUpload("dark", null)}
                                className="absolute -top-2 -right-2 w-6 h-6 bg-[var(--tone-danger)] text-[var(--on-brand)] rounded-full flex items-center justify-center hover:bg-[var(--tone-danger-strong)] transition-colors"
                              >
                                <X className="w-3 h-3" />
                              </button>
                            </div>
                          ) : (
                            <label
                              htmlFor="logo-dark"
                              className="cursor-pointer text-center"
                            >
                              <Upload className="w-8 h-8 text-[var(--text-subtle)] mx-auto mb-2" />
                              <span className="text-sm text-[var(--text-muted)]">
                                Upload dark logo
                              </span>
                            </label>
                          )}
                        </div>
                        <input
                          id="logo-dark"
                          type="file"
                          accept="image/png,image/jpeg,image/jpg,image/svg+xml"
                          onChange={(e) => {
                            const file = e.target.files?.[0];
                            if (file) handleLogoUpload("dark", file);
                          }}
                          className="hidden"
                        />
                        <p className="text-xs text-[var(--text-muted)]">
                          PNG, JPG, SVG • Max 2MB • Used on dark backgrounds
                        </p>
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              {/* Contact Information */}
              <div className="bg-[var(--surface)] rounded-[var(--radius-xl)] border border-[var(--border)] p-6 sm:p-8 shadow-[var(--shadow-rest)]">
                <h2 className="text-[length:var(--type-section-title-size)] font-bold text-[var(--text-strong)] mb-6">
                  Contact information
                </h2>

                <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                  {/* EMAP Code */}
                  <div>
                    <label
                      htmlFor="emap_code"
                      className="block text-sm font-medium text-[var(--text-strong)] mb-2"
                    >
                      EMAP code
                    </label>
                    <input
                      id="emap_code"
                      type="text"
                      value={formData.emap_code}
                      onChange={(e) => updateField("emap_code", e.target.value)}
                      data-slot="input" className="w-full px-4 h-[var(--h-control-md)] rounded-[var(--radius-md)] border border-[var(--input-border)] bg-[var(--input-bg)] text-[var(--text-strong)] placeholder:text-[var(--text-subtle)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-0 transition-colors text-sm"
                      placeholder="Ministry registration code"
                    />
                  </div>

                  {/* Email */}
                  <div>
                    <label
                      htmlFor="email"
                      className="block text-sm font-medium text-[var(--text-strong)] mb-2"
                    >
                      Official email
                    </label>
                    <input
                      id="email"
                      type="email"
                      value={formData.email}
                      onChange={(e) => updateField("email", e.target.value)}
                      data-slot="input" className={`w-full px-4 h-[var(--h-control-md)] rounded-[var(--radius-md)] border focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-0 transition-colors bg-[var(--input-bg)] text-[var(--text-strong)] placeholder:text-[var(--text-subtle)] text-sm ${
                        getFieldError("email")
                          ? "border-[var(--tone-danger)]"
                          : "border-[var(--input-border)]"
                      }`}
                      placeholder="school@example.com"
                    />
                    {getFieldError("email") && (
                      <p className="mt-2 text-sm text-[var(--tone-danger-strong)] flex items-center gap-2">
                        <AlertCircle className="w-4 h-4" />
                        {getFieldError("email")}
                      </p>
                    )}
                  </div>

                  {/* Phone */}
                  <div className="md:col-span-2">
                    <label
                      htmlFor="phone"
                      className="block text-sm font-medium text-[var(--text-strong)] mb-2"
                    >
                      Main phone number
                    </label>
                    <input
                      id="phone"
                      type="tel"
                      value={formData.phone}
                      onChange={(e) => updateField("phone", e.target.value)}
                      data-slot="input" className={`w-full px-4 h-[var(--h-control-md)] rounded-[var(--radius-md)] border focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-0 transition-colors bg-[var(--input-bg)] text-[var(--text-strong)] placeholder:text-[var(--text-subtle)] text-sm ${
                        getFieldError("phone")
                          ? "border-[var(--tone-danger)]"
                          : "border-[var(--input-border)]"
                      }`}
                      placeholder="+263 123 456 789"
                    />
                    {getFieldError("phone") && (
                      <p className="mt-2 text-sm text-[var(--tone-danger-strong)] flex items-center gap-2">
                        <AlertCircle className="w-4 h-4" />
                        {getFieldError("phone")}
                      </p>
                    )}
                  </div>
                </div>
              </div>

              {/* Address */}
              <div className="bg-[var(--surface)] rounded-[var(--radius-xl)] border border-[var(--border)] p-6 sm:p-8 shadow-[var(--shadow-rest)]">
                <h2 className="text-[length:var(--type-section-title-size)] font-bold text-[var(--text-strong)] mb-6">
                  Address and location
                </h2>

                <div className="space-y-6">
                  <div>
                    <label
                      htmlFor="address_line1"
                      className="block text-sm font-medium text-[var(--text-strong)] mb-2"
                    >
                      Address line 1
                    </label>
                    <input
                      id="address_line1"
                      type="text"
                      value={formData.address_line1}
                      onChange={(e) =>
                        updateField("address_line1", e.target.value)
                      }
                      data-slot="input" className="w-full px-4 h-[var(--h-control-md)] rounded-[var(--radius-md)] border border-[var(--input-border)] bg-[var(--input-bg)] text-[var(--text-strong)] placeholder:text-[var(--text-subtle)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-0 transition-colors text-sm"
                      placeholder="Street address"
                    />
                  </div>

                  <div>
                    <label
                      htmlFor="address_line2"
                      className="block text-sm font-medium text-[var(--text-strong)] mb-2"
                    >
                      Address line 2
                    </label>
                    <input
                      id="address_line2"
                      type="text"
                      value={formData.address_line2}
                      onChange={(e) =>
                        updateField("address_line2", e.target.value)
                      }
                      data-slot="input" className="w-full px-4 h-[var(--h-control-md)] rounded-[var(--radius-md)] border border-[var(--input-border)] bg-[var(--input-bg)] text-[var(--text-strong)] placeholder:text-[var(--text-subtle)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-0 transition-colors text-sm"
                      placeholder="Apartment, suite, etc."
                    />
                  </div>

                  <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
                    <div>
                      <label
                        htmlFor="city"
                        className="block text-sm font-medium text-[var(--text-strong)] mb-2"
                      >
                        City or town
                      </label>
                      <input
                        id="city"
                        type="text"
                        value={formData.city}
                        onChange={(e) => updateField("city", e.target.value)}
                        data-slot="input" className="w-full px-4 h-[var(--h-control-md)] rounded-[var(--radius-md)] border border-[var(--input-border)] bg-[var(--input-bg)] text-[var(--text-strong)] placeholder:text-[var(--text-subtle)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-0 transition-colors text-sm"
                        placeholder="Harare"
                      />
                    </div>

                    <div>
                      <label
                        htmlFor="province"
                        className="block text-sm font-medium text-[var(--text-strong)] mb-2"
                      >
                        Province
                      </label>
                      <input
                        id="province"
                        type="text"
                        value={formData.province}
                        onChange={(e) =>
                          updateField("province", e.target.value)
                        }
                        data-slot="input" className="w-full px-4 h-[var(--h-control-md)] rounded-[var(--radius-md)] border border-[var(--input-border)] bg-[var(--input-bg)] text-[var(--text-strong)] placeholder:text-[var(--text-subtle)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-0 transition-colors text-sm"
                        placeholder="Harare"
                      />
                    </div>

                    <div>
                      <label
                        htmlFor="country"
                        className="block text-sm font-medium text-[var(--text-strong)] mb-2"
                      >
                        Country
                      </label>
                      <SearchableSelect
                        options={countryOptions}
                        value={getCountryId(formData.country)}
                        onChange={(id) =>
                          updateField("country", getCountryValue(id))
                        }
                        placeholder="Select country..."
                        className="w-full"
                      />
                    </div>
                  </div>

                  <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                    <div>
                      <label
                        htmlFor="timezone"
                        className="block text-sm font-medium text-[var(--text-strong)] mb-2"
                      >
                        Timezone
                      </label>
                      <SearchableSelect
                        options={timezoneOptions}
                        value={getTimezoneId(formData.timezone)}
                        onChange={(id) =>
                          updateField("timezone", getTimezoneValue(id))
                        }
                        placeholder="Select timezone..."
                        className="w-full"
                      />
                    </div>

                    <div>
                      <label
                        htmlFor="locale"
                        className="block text-sm font-medium text-[var(--text-strong)] mb-2"
                      >
                        Language
                      </label>
                      <SearchableSelect
                        options={localeOptions}
                        value={getLocaleId(formData.locale)}
                        onChange={(id) =>
                          updateField("locale", getLocaleValue(id))
                        }
                        placeholder="Select language..."
                        className="w-full"
                      />
                    </div>
                  </div>
                </div>
              </div>

              {/* Actions */}
              <div className="flex flex-col sm:flex-row gap-4 justify-end">
                <button
                  type="button"
                  onClick={handleSkipLogos}
                  disabled={isSubmitting}
                  className="px-6 h-[var(--h-control-md)] border border-[var(--border)] bg-[var(--surface)] text-[var(--text-strong)] rounded-[var(--radius-md)] hover:bg-[var(--surface-muted)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-2 transition-colors disabled:opacity-50 disabled:cursor-not-allowed text-sm font-medium"
                >
                  Skip logos for now
                </button>
                <button
                  type="submit"
                  disabled={isSubmitting}
                  className="px-8 h-[var(--h-control-md)] bg-[var(--action-primary-bg)] hover:bg-[var(--action-primary-bg-hover)] active:bg-[var(--action-primary-bg-pressed)] disabled:bg-[var(--action-disabled-bg)] disabled:text-[var(--action-disabled-fg)] text-[var(--action-primary-fg)] font-semibold rounded-[var(--radius-md)] transition-colors flex items-center justify-center gap-2 disabled:cursor-not-allowed focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-2 text-sm"
                >
                  {isSubmitting ? (
                    <>
                      <Loader2 className="w-5 h-5 animate-spin" />
                      Saving…
                    </>
                  ) : (
                    <>
                      Save and continue
                      <ArrowRight className="w-5 h-5" />
                    </>
                  )}
                </button>
              </div>
            </form>
          </div>

          {/* Preview */}
          <div className="lg:w-80">
            <div className="sticky top-8">
              <SchoolPreviewCard
                schoolData={formData}
                logoPreview={logoPreview}
              />
            </div>
          </div>
        </div>
    </SetupScaffold>
  );
};
