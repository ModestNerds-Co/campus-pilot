//
//  campus-pilot
//  SchoolSetupScreen.tsx - School Configuration Screen
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
  Eye,
  EyeOff,
  Loader2,
  AlertCircle,
  School,
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
import { ThemeToggle } from "../../../../lib/theme";
import { SearchableSelect } from "../../../../components/searchable-select";
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
    <div className="min-h-screen bg-gradient-to-br from-blue-50 via-white to-gray-50 dark:from-gray-900 dark:via-gray-800 dark:to-gray-900">
      {/* Theme Toggle */}
      <div className="absolute top-6 right-6 z-10">
        <ThemeToggle />
      </div>

      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-12">
        {/* Header */}
        <div className="text-center mb-12">
          <div className="w-16 h-16 mx-auto mb-4 bg-blue-100 dark:bg-blue-900 rounded-full flex items-center justify-center">
            <School className="w-8 h-8 text-blue-600 dark:text-blue-400" />
          </div>
          <h1 className="text-3xl font-bold text-gray-900 dark:text-white mb-4">
            Set up your school
          </h1>
          <p className="text-lg text-gray-600 dark:text-gray-300 max-w-xl mx-auto">
            We'll use this to personalize receipts, reports, and the login
            screen.
          </p>
        </div>

        <div className="flex flex-col lg:flex-row gap-8 max-w-6xl mx-auto">
          {/* Form */}
          <div className="flex-1">
            <form onSubmit={handleSubmit} className="space-y-8">
              {/* Branding Section */}
              <div className="bg-white dark:bg-gray-800 rounded-2xl shadow-lg border border-gray-100 dark:border-gray-700 p-8">
                <h2 className="text-xl font-semibold text-gray-900 dark:text-white mb-6">
                  Branding
                </h2>

                <div className="space-y-6">
                  {/* School Name */}
                  <div>
                    <label
                      htmlFor="name"
                      className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
                    >
                      School Name *
                    </label>
                    <input
                      id="name"
                      type="text"
                      value={formData.name}
                      onChange={(e) => updateField("name", e.target.value)}
                      className={`w-full px-4 py-3 border rounded-xl focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-colors text-gray-900 dark:bg-gray-700 dark:text-white ${
                        getFieldError("name")
                          ? "border-red-500"
                          : "border-gray-300 dark:border-gray-600"
                      }`}
                      placeholder="Enter your school's name"
                    />
                    {getFieldError("name") && (
                      <p className="mt-2 text-sm text-red-600 flex items-center gap-2">
                        <AlertCircle className="w-4 h-4" />
                        {getFieldError("name")}
                      </p>
                    )}
                    <p className="mt-2 text-sm text-gray-500">
                      Shown on login, receipts, and reports.
                    </p>
                  </div>

                  {/* Legal Name */}
                  <div>
                    <label
                      htmlFor="legal_name"
                      className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
                    >
                      Registered / Legal Name
                    </label>
                    <input
                      id="legal_name"
                      type="text"
                      value={formData.legal_name}
                      onChange={(e) =>
                        updateField("legal_name", e.target.value)
                      }
                      className="w-full px-4 py-3 border border-gray-300 dark:border-gray-600 text-gray-900 dark:bg-gray-700 dark:text-white rounded-xl focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-colors"
                      placeholder="Official registered name (if different)"
                    />
                  </div>

                  {/* Logo Upload */}
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                    {/* Light Logo */}
                    <div>
                      <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                        Logo (Light)
                      </label>
                      <div className="space-y-3">
                        <div className="flex items-center justify-center w-full h-32 border-2 border-dashed border-gray-300 dark:border-gray-600 rounded-xl hover:border-gray-400 dark:hover:border-gray-500 transition-colors">
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
                                className="absolute -top-2 -right-2 w-6 h-6 bg-red-500 text-white rounded-full flex items-center justify-center hover:bg-red-600"
                              >
                                <X className="w-3 h-3" />
                              </button>
                            </div>
                          ) : (
                            <label
                              htmlFor="logo-light"
                              className="cursor-pointer text-center"
                            >
                              <Upload className="w-8 h-8 text-gray-400 mx-auto mb-2" />
                              <span className="text-sm text-gray-600 dark:text-gray-400">
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
                        <p className="text-xs text-gray-500 dark:text-gray-400">
                          PNG, JPG, SVG • Max 2MB • Used on light backgrounds
                        </p>
                      </div>
                    </div>

                    {/* Dark Logo */}
                    <div>
                      <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                        Logo (Dark)
                      </label>
                      <div className="space-y-3">
                        <div className="flex items-center justify-center w-full h-32 border-2 border-dashed border-gray-300 dark:border-gray-600 rounded-xl hover:border-gray-400 dark:hover:border-gray-500 transition-colors bg-gray-900">
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
                                className="absolute -top-2 -right-2 w-6 h-6 bg-red-500 text-white rounded-full flex items-center justify-center hover:bg-red-600"
                              >
                                <X className="w-3 h-3" />
                              </button>
                            </div>
                          ) : (
                            <label
                              htmlFor="logo-dark"
                              className="cursor-pointer text-center"
                            >
                              <Upload className="w-8 h-8 text-gray-400 mx-auto mb-2" />
                              <span className="text-sm text-gray-400 dark:text-gray-300">
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
                        <p className="text-xs text-gray-500 dark:text-gray-400">
                          PNG, JPG, SVG • Max 2MB • Used on dark backgrounds
                        </p>
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              {/* Contact Information */}
              <div className="bg-white dark:bg-gray-800 rounded-2xl shadow-lg border border-gray-100 dark:border-gray-700 p-8">
                <h2 className="text-xl font-semibold text-gray-900 dark:text-white mb-6">
                  Contact Information
                </h2>

                <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                  {/* EMAP Code */}
                  <div>
                    <label
                      htmlFor="emap_code"
                      className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
                    >
                      EMAP Code
                    </label>
                    <input
                      id="emap_code"
                      type="text"
                      value={formData.emap_code}
                      onChange={(e) => updateField("emap_code", e.target.value)}
                      className="w-full px-4 py-3 border border-gray-300 dark:border-gray-600 text-gray-900 dark:bg-gray-700 dark:text-white rounded-xl focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-colors"
                      placeholder="Ministry registration code"
                    />
                  </div>

                  {/* Email */}
                  <div>
                    <label
                      htmlFor="email"
                      className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
                    >
                      Official Email
                    </label>
                    <input
                      id="email"
                      type="email"
                      value={formData.email}
                      onChange={(e) => updateField("email", e.target.value)}
                      className={`w-full px-4 py-3 border rounded-xl focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-colors text-gray-900 dark:bg-gray-700 dark:text-white ${
                        getFieldError("email")
                          ? "border-red-500"
                          : "border-gray-300 dark:border-gray-600"
                      }`}
                      placeholder="school@example.com"
                    />
                    {getFieldError("email") && (
                      <p className="mt-2 text-sm text-red-600 flex items-center gap-2">
                        <AlertCircle className="w-4 h-4" />
                        {getFieldError("email")}
                      </p>
                    )}
                  </div>

                  {/* Phone */}
                  <div className="md:col-span-2">
                    <label
                      htmlFor="phone"
                      className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
                    >
                      Main Phone Number
                    </label>
                    <input
                      id="phone"
                      type="tel"
                      value={formData.phone}
                      onChange={(e) => updateField("phone", e.target.value)}
                      className={`w-full px-4 py-3 border rounded-xl focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-colors text-gray-900 dark:bg-gray-700 dark:text-white ${
                        getFieldError("phone")
                          ? "border-red-500"
                          : "border-gray-300 dark:border-gray-600"
                      }`}
                      placeholder="+263 123 456 789"
                    />
                    {getFieldError("phone") && (
                      <p className="mt-2 text-sm text-red-600 flex items-center gap-2">
                        <AlertCircle className="w-4 h-4" />
                        {getFieldError("phone")}
                      </p>
                    )}
                  </div>
                </div>
              </div>

              {/* Address */}
              <div className="bg-white dark:bg-gray-800 rounded-2xl shadow-lg border border-gray-100 dark:border-gray-700 p-8">
                <h2 className="text-xl font-semibold text-gray-900 dark:text-white mb-6">
                  Address & Location
                </h2>

                <div className="space-y-6">
                  <div>
                    <label
                      htmlFor="address_line1"
                      className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
                    >
                      Address Line 1
                    </label>
                    <input
                      id="address_line1"
                      type="text"
                      value={formData.address_line1}
                      onChange={(e) =>
                        updateField("address_line1", e.target.value)
                      }
                      className="w-full px-4 py-3 border border-gray-300 dark:border-gray-600 text-gray-900 dark:bg-gray-700 dark:text-white rounded-xl focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-colors"
                      placeholder="Street address"
                    />
                  </div>

                  <div>
                    <label
                      htmlFor="address_line2"
                      className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
                    >
                      Address Line 2
                    </label>
                    <input
                      id="address_line2"
                      type="text"
                      value={formData.address_line2}
                      onChange={(e) =>
                        updateField("address_line2", e.target.value)
                      }
                      className="w-full px-4 py-3 border border-gray-300 dark:border-gray-600 text-gray-900 dark:bg-gray-700 dark:text-white rounded-xl focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-colors"
                      placeholder="Apartment, suite, etc."
                    />
                  </div>

                  <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
                    <div>
                      <label
                        htmlFor="city"
                        className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
                      >
                        City/Town
                      </label>
                      <input
                        id="city"
                        type="text"
                        value={formData.city}
                        onChange={(e) => updateField("city", e.target.value)}
                        className="w-full px-4 py-3 border border-gray-300 dark:border-gray-600 text-gray-900 dark:bg-gray-700 dark:text-white rounded-xl focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-colors"
                        placeholder="Harare"
                      />
                    </div>

                    <div>
                      <label
                        htmlFor="province"
                        className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
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
                        className="w-full px-4 py-3 border border-gray-300 dark:border-gray-600 text-gray-900 dark:bg-gray-700 dark:text-white rounded-xl focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-colors"
                        placeholder="Harare"
                      />
                    </div>

                    <div>
                      <label
                        htmlFor="country"
                        className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
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
                        className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
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
                        className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
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
                  className="px-6 py-3 border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 rounded-xl hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  Skip logos for now
                </button>
                <button
                  type="submit"
                  disabled={isSubmitting}
                  className="px-8 py-3 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white font-semibold rounded-xl transition-colors flex items-center gap-2 disabled:cursor-not-allowed"
                >
                  {isSubmitting ? (
                    <>
                      <Loader2 className="w-5 h-5 animate-spin" />
                      Saving...
                    </>
                  ) : (
                    <>
                      Save & Continue
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
      </div>
    </div>
  );
};
