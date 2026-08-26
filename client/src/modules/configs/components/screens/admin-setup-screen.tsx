//
//  campus-pilot
//  AdminSetupScreen.tsx - Administrator creation screen.
//
//  Created by Ngonidzashe Mangudya on 26/09/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import React, { useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import {
  ArrowLeft,
  ArrowRight,
  User,
  Eye,
  EyeOff,
  Loader2,
  AlertCircle,
  Shield,
  Check,
} from "lucide-react";
import { bootstrapService } from "../../services/bootstrap-service";
import type {
  AdminFormData,
  FormFieldError,
  PasswordStrength,
} from "../../types";
import {
  validateEmail,
  validatePhone,
  validatePassword,
  checkCapsLock,
} from "../../../../lib/validation";
import {
  PASSWORD_STRENGTH_COLORS,
  PASSWORD_STRENGTH_LABELS,
} from "../../constants";
import toast from "react-hot-toast";
import { SetupScaffold } from "../ui/setup-scaffold";

export const AdminSetupScreen: React.FC = () => {
  const navigate = useNavigate();
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [errors, setErrors] = useState<FormFieldError[]>([]);
  const [showPassword, setShowPassword] = useState(false);
  const [showPasswordConfirm, setShowPasswordConfirm] = useState(false);
  const [capsLockOn, setCapsLockOn] = useState(false);
  const [passwordStrength, setPasswordStrength] = useState<PasswordStrength>({
    score: 0,
    feedback: [],
    isValid: false,
    label: "Very Weak",
  });

  const [formData, setFormData] = useState<AdminFormData>({
    full_name: "",
    email: "",
    phone: "",
    password: "",
    password_confirm: "",
  });

  const updateField = (field: keyof AdminFormData, value: string) => {
    setFormData((prev) => ({ ...prev, [field]: value }));

    // Clear field-specific errors
    setErrors((prev) => prev.filter((err) => err.field !== field));

    // Update password strength when password changes
    if (field === "password") {
      const strength = validatePassword(value);
      setPasswordStrength(strength);
    }
  };

  const validateForm = (): boolean => {
    const newErrors: FormFieldError[] = [];

    // Full name validation
    if (!formData.full_name.trim()) {
      newErrors.push({ field: "full_name", message: "Full name is required" });
    } else if (
      formData.full_name.trim().length < 2 ||
      formData.full_name.trim().length > 80
    ) {
      newErrors.push({
        field: "full_name",
        message: "Full name must be between 2 and 80 characters",
      });
    }

    // Email validation
    const emailValidation = validateEmail(formData.email);
    if (!emailValidation.isValid) {
      newErrors.push({ field: "email", message: emailValidation.error! });
    }

    // Phone validation (optional)
    if (formData.phone) {
      const phoneValidation = validatePhone(formData.phone);
      if (!phoneValidation.isValid) {
        newErrors.push({ field: "phone", message: phoneValidation.error! });
      }
    }

    // Password validation
    if (!passwordStrength.isValid) {
      newErrors.push({
        field: "password",
        message: "Password does not meet security requirements",
      });
    }

    // Confirm password
    if (formData.password !== formData.password_confirm) {
      newErrors.push({
        field: "password_confirm",
        message: "Passwords do not match",
      });
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
      const adminConfig = {
        full_name: formData.full_name.trim(),
        email: formData.email.trim(),
        phone: formData.phone.trim() || null,
        password: formData.password,
      };

      const response = await bootstrapService.createAdmin(adminConfig);

      if (response.success) {
        toast.success(
          response.message || "Administrator account created successfully",
        );

        // Show success state briefly before redirect
        setTimeout(() => {
          navigate({ to: "/login" });
        }, 1500);
      } else {
        // Handle validation errors from server
        if (response.issues && response.issues.length > 0) {
          const serverErrors = response.issues.map((issue) => ({
            field: issue.field || "general",
            message: issue.detail,
          }));
          setErrors(serverErrors);
        }
        toast.error(
          response.message || "Failed to create administrator account",
        );
      }
    } catch (error) {
      console.error("Admin setup failed:", error);
      toast.error(
        error instanceof Error
          ? error.message
          : "Setup failed. Please try again.",
      );
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleBack = () => {
    navigate({ to: "/setup/school" });
  };

  const handlePasswordKeyDown = (e: React.KeyboardEvent) => {
    setCapsLockOn(checkCapsLock(e));
  };

  const getFieldError = (field: string) =>
    errors.find((err) => err.field === field)?.message;

  const getPasswordStrengthColor = () => {
    return PASSWORD_STRENGTH_COLORS[passwordStrength.score];
  };

  return (
    <SetupScaffold
      description="Create the first account that can manage school settings, users, and access."
      maxWidth="narrow"
      step={2}
      title="Create the first administrator"
    >
        {/* Form */}
        <div className="bg-[var(--surface)] rounded-[var(--radius-xl)] border border-[var(--border)] p-6 shadow-[var(--shadow-rest)] sm:p-8">
          <form onSubmit={handleSubmit} className="space-y-6">
            {/* Full Name */}
            <div>
              <label
                htmlFor="full_name"
                className="block text-sm font-medium text-[var(--text-strong)] mb-2"
              >
                Full name *
              </label>
              <div className="relative">
                <User className="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-[var(--text-subtle)] pointer-events-none" />
                <input
                  id="full_name"
                  type="text"
                  value={formData.full_name}
                  onChange={(e) => updateField("full_name", e.target.value)}
                  data-slot="input" className={`w-full pl-11 pr-4 h-[var(--h-control-md)] rounded-[var(--radius-md)] border focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-0 transition-colors bg-[var(--input-bg)] text-[var(--text-strong)] placeholder:text-[var(--text-subtle)] text-sm ${
                    getFieldError("full_name")
                      ? "border-[var(--tone-danger)]"
                      : "border-[var(--input-border)]"
                  }`}
                  placeholder="Enter your full name"
                />
              </div>
              {getFieldError("full_name") && (
                <p className="mt-2 text-sm text-[var(--tone-danger-strong)] flex items-center gap-2">
                  <AlertCircle className="w-4 h-4" />
                  {getFieldError("full_name")}
                </p>
              )}
            </div>

            {/* Email */}
            <div>
              <label
                htmlFor="email"
                className="block text-sm font-medium text-[var(--text-strong)] mb-2"
              >
                Administrator email *
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
                placeholder="admin@yourschool.com"
              />
              {getFieldError("email") && (
                <p className="mt-2 text-sm text-[var(--tone-danger-strong)] flex items-center gap-2">
                  <AlertCircle className="w-4 h-4" />
                  {getFieldError("email")}
                </p>
              )}
            </div>

            {/* Phone (Optional) */}
            <div>
              <label
                htmlFor="phone"
                className="block text-sm font-medium text-[var(--text-strong)] mb-2"
              >
                Phone number
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

            {/* Password */}
            <div>
              <label
                htmlFor="password"
                className="block text-sm font-medium text-[var(--text-strong)] mb-2"
              >
                Password *
              </label>
              <div className="relative">
                <input
                  id="password"
                  type={showPassword ? "text" : "password"}
                  value={formData.password}
                  onChange={(e) => updateField("password", e.target.value)}
                  onKeyDown={handlePasswordKeyDown}
                  data-slot="input" className={`w-full px-4 pr-11 h-[var(--h-control-md)] rounded-[var(--radius-md)] border focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-0 transition-colors bg-[var(--input-bg)] text-[var(--text-strong)] placeholder:text-[var(--text-subtle)] text-sm ${
                    getFieldError("password")
                      ? "border-[var(--tone-danger)]"
                      : "border-[var(--input-border)]"
                  }`}
                  placeholder="Enter a secure password"
                />
                <button
                  aria-label={showPassword ? "Hide password" : "Show password"}
                  type="button"
                  onClick={() => setShowPassword(!showPassword)}
                  className="absolute right-3 top-1/2 -translate-y-1/2 inline-flex h-8 w-8 items-center justify-center rounded-[var(--radius-sm)] text-[var(--text-subtle)] hover:text-[var(--text-strong)] hover:bg-[var(--surface-muted)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]"
                >
                  {showPassword ? (
                    <EyeOff className="w-5 h-5" />
                  ) : (
                    <Eye className="w-5 h-5" />
                  )}
                </button>
              </div>

              {/* Caps Lock Warning */}
              {capsLockOn && (
                <p className="mt-2 text-sm text-[var(--tone-warn-strong)] flex items-center gap-2">
                  <AlertCircle className="w-4 h-4" />
                  Caps Lock is on
                </p>
              )}

              {/* Password Strength Meter */}
              {formData.password && (
                <div className="mt-3 space-y-2">
                  <div className="flex justify-between items-center">
                    <span className="text-sm font-medium text-[var(--text-strong)]">
                      Password strength
                    </span>
                    <span
                      className={`text-sm font-medium ${
                        passwordStrength.isValid
                          ? "text-[var(--tone-success-strong)]"
                          : "text-[var(--tone-danger-strong)]"
                      }`}
                    >
                      {passwordStrength.label}
                    </span>
                  </div>
                  <div className="w-full bg-[var(--surface-muted)] border border-[var(--border-subtle)] rounded-full h-2 overflow-hidden">
                    <div
                      className={`h-2 rounded-full transition-all duration-300 ${getPasswordStrengthColor()}`}
                      style={{ width: `${(passwordStrength.score + 1) * 20}%` }}
                    />
                  </div>
                  {passwordStrength.feedback.length > 0 && (
                    <div className="text-sm text-[var(--text-muted)] space-y-1">
                      {passwordStrength.feedback.map((feedback, index) => (
                        <div key={index} className="flex items-center gap-2">
                          <div className="w-1 h-1 bg-[var(--text-subtle)] rounded-full" />
                          {feedback}
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}

              {getFieldError("password") && (
                <p className="mt-2 text-sm text-[var(--tone-danger-strong)] flex items-center gap-2">
                  <AlertCircle className="w-4 h-4" />
                  {getFieldError("password")}
                </p>
              )}
              <p className="mt-2 text-sm text-[var(--text-muted)]">
                At least 10 characters, including a number and symbol.
              </p>
            </div>

            {/* Confirm Password */}
            <div>
              <label
                htmlFor="password_confirm"
                className="block text-sm font-medium text-[var(--text-strong)] mb-2"
              >
                Confirm password *
              </label>
              <div className="relative">
                <input
                  id="password_confirm"
                  type={showPasswordConfirm ? "text" : "password"}
                  value={formData.password_confirm}
                  onChange={(e) =>
                    updateField("password_confirm", e.target.value)
                  }
                  data-slot="input" className={`w-full px-4 pr-11 h-[var(--h-control-md)] rounded-[var(--radius-md)] border focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-0 transition-colors bg-[var(--input-bg)] text-[var(--text-strong)] placeholder:text-[var(--text-subtle)] text-sm ${
                    getFieldError("password_confirm")
                      ? "border-[var(--tone-danger)]"
                      : "border-[var(--input-border)]"
                  }`}
                  placeholder="Confirm your password"
                />
                <button
                  aria-label={showPasswordConfirm ? "Hide password confirmation" : "Show password confirmation"}
                  type="button"
                  onClick={() => setShowPasswordConfirm(!showPasswordConfirm)}
                  className="absolute right-3 top-1/2 -translate-y-1/2 inline-flex h-8 w-8 items-center justify-center rounded-[var(--radius-sm)] text-[var(--text-subtle)] hover:text-[var(--text-strong)] hover:bg-[var(--surface-muted)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]"
                >
                  {showPasswordConfirm ? (
                    <EyeOff className="w-5 h-5" />
                  ) : (
                    <Eye className="w-5 h-5" />
                  )}
                </button>
              </div>

              {/* Password Match Indicator */}
              {formData.password_confirm && (
                <div className="mt-2 flex items-center gap-2">
                  {formData.password === formData.password_confirm ? (
                    <>
                      <Check className="w-4 h-4 text-[var(--tone-success)]" />
                      <span className="text-sm text-[var(--tone-success-strong)]">
                        Passwords match
                      </span>
                    </>
                  ) : (
                    <>
                      <AlertCircle className="w-4 h-4 text-[var(--tone-danger)]" />
                      <span className="text-sm text-[var(--tone-danger-strong)]">
                        Passwords do not match
                      </span>
                    </>
                  )}
                </div>
              )}

              {getFieldError("password_confirm") && (
                <p className="mt-2 text-sm text-[var(--tone-danger-strong)] flex items-center gap-2">
                  <AlertCircle className="w-4 h-4" />
                  {getFieldError("password_confirm")}
                </p>
              )}
            </div>

            {/* Actions */}
            <div className="flex flex-col sm:flex-row gap-4 justify-between pt-6">
              <button
                type="button"
                onClick={handleBack}
                disabled={isSubmitting}
                className="flex items-center gap-2 px-6 h-[var(--h-control-md)] border border-[var(--border)] bg-[var(--surface)] text-[var(--text-strong)] rounded-[var(--radius-md)] hover:bg-[var(--surface-muted)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-2 transition-colors disabled:opacity-50 disabled:cursor-not-allowed text-sm font-medium"
              >
                <ArrowLeft className="w-5 h-5" />
                Back to school details
              </button>

              <button
                type="submit"
                disabled={isSubmitting || !passwordStrength.isValid}
                className="flex items-center gap-2 px-8 h-[var(--h-control-md)] bg-[var(--tone-success)] hover:bg-[var(--tone-success-strong)] disabled:bg-[var(--action-disabled-bg)] disabled:text-[var(--action-disabled-fg)] text-[var(--on-brand)] font-semibold rounded-[var(--radius-md)] transition-colors disabled:cursor-not-allowed focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-2 text-sm"
              >
                {isSubmitting ? (
                  <>
                    <Loader2 className="w-5 h-5 animate-spin" />
                    Creating administrator…
                  </>
                ) : (
                  <>
                    Create administrator
                    <ArrowRight className="w-5 h-5" />
                  </>
                )}
              </button>
            </div>
          </form>
        </div>

        {/* Security Notice */}
        <div className="mt-6 bg-[var(--tone-info-bg)] border border-[var(--tone-info-bd)] rounded-[var(--radius-xl)] p-4">
          <div className="flex items-start gap-3">
            <Shield className="w-5 h-5 text-[var(--tone-info-strong)] mt-0.5 flex-shrink-0" />
            <div>
              <h4 className="text-sm font-medium text-[var(--tone-info-strong)] mb-1">
                Security notice
              </h4>
              <p className="text-sm text-[var(--tone-info-strong)] opacity-90">
                This administrator account will have full access to the system.
                Keep your credentials secure and use a strong, unique password.
                You can create additional admin accounts later.
              </p>
            </div>
          </div>
        </div>
    </SetupScaffold>
  );
};
