//
//  campus-pilot
//  AdminSetupScreen.tsx - Administrator Creation Screen
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
import { ThemeToggle } from "../../../../lib/theme";
import toast from "react-hot-toast";

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
    <div className="min-h-screen bg-gradient-to-br from-blue-50 via-white to-gray-50 dark:from-gray-900 dark:via-gray-800 dark:to-gray-900">
      {/* Theme Toggle */}
      <div className="absolute top-6 right-6 z-10">
        <ThemeToggle />
      </div>

      <div className="max-w-2xl mx-auto px-4 sm:px-6 lg:px-8 py-12">
        {/* Header */}
        <div className="text-center mb-12">
          <div className="w-16 h-16 mx-auto mb-4 bg-green-100 rounded-full flex items-center justify-center">
            <Shield className="w-8 h-8 text-green-600" />
          </div>
          <h1 className="text-3xl font-bold text-gray-900 dark:text-white mb-4">
            Create the first administrator
          </h1>
          <p className="text-lg text-gray-600 dark:text-gray-300 max-w-xl mx-auto">
            This account manages users, classes, fees, and more.
          </p>
        </div>

        {/* Form */}
        <div className="bg-white dark:bg-gray-800 rounded-2xl shadow-lg border border-gray-100 dark:border-gray-700 p-8">
          <form onSubmit={handleSubmit} className="space-y-6">
            {/* Full Name */}
            <div>
              <label
                htmlFor="full_name"
                className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
              >
                Full Name *
              </label>
              <div className="relative">
                <User className="absolute left-3 top-1/2 transform -translate-y-1/2 w-5 h-5 text-gray-400" />
                <input
                  id="full_name"
                  type="text"
                  value={formData.full_name}
                  onChange={(e) => updateField("full_name", e.target.value)}
                  className={`w-full pl-12 pr-4 py-3 border rounded-xl focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-colors text-gray-900 dark:bg-gray-700 dark:text-white ${
                    getFieldError("full_name")
                      ? "border-red-500"
                      : "border-gray-300 dark:border-gray-600"
                  }`}
                  placeholder="Enter your full name"
                />
              </div>
              {getFieldError("full_name") && (
                <p className="mt-2 text-sm text-red-600 flex items-center gap-2">
                  <AlertCircle className="w-4 h-4" />
                  {getFieldError("full_name")}
                </p>
              )}
            </div>

            {/* Email */}
            <div>
              <label
                htmlFor="email"
                className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
              >
                Admin Email (will be your login) *
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
                placeholder="admin@yourschool.com"
              />
              {getFieldError("email") && (
                <p className="mt-2 text-sm text-red-600 flex items-center gap-2">
                  <AlertCircle className="w-4 h-4" />
                  {getFieldError("email")}
                </p>
              )}
            </div>

            {/* Phone (Optional) */}
            <div>
              <label
                htmlFor="phone"
                className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
              >
                Phone Number
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

            {/* Password */}
            <div>
              <label
                htmlFor="password"
                className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
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
                  className={`w-full px-4 py-3 pr-12 border rounded-xl focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-colors text-gray-900 dark:bg-gray-700 dark:text-white ${
                    getFieldError("password")
                      ? "border-red-500"
                      : "border-gray-300 dark:border-gray-600"
                  }`}
                  placeholder="Enter a secure password"
                />
                <button
                  type="button"
                  onClick={() => setShowPassword(!showPassword)}
                  className="absolute right-3 top-1/2 transform -translate-y-1/2 text-gray-400 hover:text-gray-600 dark:text-gray-300"
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
                <p className="mt-2 text-sm text-yellow-600 flex items-center gap-2">
                  <AlertCircle className="w-4 h-4" />
                  Caps Lock is on
                </p>
              )}

              {/* Password Strength Meter */}
              {formData.password && (
                <div className="mt-3 space-y-2">
                  <div className="flex justify-between items-center">
                    <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
                      Password strength
                    </span>
                    <span
                      className={`text-sm font-medium ${
                        passwordStrength.isValid
                          ? "text-green-600"
                          : "text-red-600"
                      }`}
                    >
                      {passwordStrength.label}
                    </span>
                  </div>
                  <div className="w-full bg-gray-200 rounded-full h-2">
                    <div
                      className={`h-2 rounded-full transition-all duration-300 ${getPasswordStrengthColor()}`}
                      style={{ width: `${(passwordStrength.score + 1) * 20}%` }}
                    />
                  </div>
                  {passwordStrength.feedback.length > 0 && (
                    <div className="text-sm text-gray-600 dark:text-gray-300 space-y-1">
                      {passwordStrength.feedback.map((feedback, index) => (
                        <div key={index} className="flex items-center gap-2">
                          <div className="w-1 h-1 bg-gray-400 rounded-full" />
                          {feedback}
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}

              {getFieldError("password") && (
                <p className="mt-2 text-sm text-red-600 flex items-center gap-2">
                  <AlertCircle className="w-4 h-4" />
                  {getFieldError("password")}
                </p>
              )}
              <p className="mt-2 text-sm text-gray-500">
                At least 10 characters, including a number and symbol.
              </p>
            </div>

            {/* Confirm Password */}
            <div>
              <label
                htmlFor="password_confirm"
                className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
              >
                Confirm Password *
              </label>
              <div className="relative">
                <input
                  id="password_confirm"
                  type={showPasswordConfirm ? "text" : "password"}
                  value={formData.password_confirm}
                  onChange={(e) =>
                    updateField("password_confirm", e.target.value)
                  }
                  className={`w-full px-4 py-3 pr-12 border rounded-xl focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-colors text-gray-900 dark:bg-gray-700 dark:text-white ${
                    getFieldError("password_confirm")
                      ? "border-red-500"
                      : "border-gray-300 dark:border-gray-600"
                  }`}
                  placeholder="Confirm your password"
                />
                <button
                  type="button"
                  onClick={() => setShowPasswordConfirm(!showPasswordConfirm)}
                  className="absolute right-3 top-1/2 transform -translate-y-1/2 text-gray-400 hover:text-gray-600 dark:text-gray-300"
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
                      <Check className="w-4 h-4 text-green-600" />
                      <span className="text-sm text-green-600">
                        Passwords match
                      </span>
                    </>
                  ) : (
                    <>
                      <AlertCircle className="w-4 h-4 text-red-600" />
                      <span className="text-sm text-red-600">
                        Passwords do not match
                      </span>
                    </>
                  )}
                </div>
              )}

              {getFieldError("password_confirm") && (
                <p className="mt-2 text-sm text-red-600 flex items-center gap-2">
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
                className="flex items-center gap-2 px-6 py-3 border border-gray-300 text-gray-700 dark:text-gray-300 rounded-xl hover:bg-gray-50 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                <ArrowLeft className="w-5 h-5" />
                Back to School Details
              </button>

              <button
                type="submit"
                disabled={isSubmitting || !passwordStrength.isValid}
                className="flex items-center gap-2 px-8 py-3 bg-green-600 hover:bg-green-700 disabled:bg-green-400 text-white font-semibold rounded-xl transition-colors disabled:cursor-not-allowed"
              >
                {isSubmitting ? (
                  <>
                    <Loader2 className="w-5 h-5 animate-spin" />
                    Creating Admin...
                  </>
                ) : (
                  <>
                    Create Admin & Finish
                    <ArrowRight className="w-5 h-5" />
                  </>
                )}
              </button>
            </div>
          </form>
        </div>

        {/* Security Notice */}
        <div className="mt-6 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-xl p-4">
          <div className="flex items-start gap-3">
            <Shield className="w-5 h-5 text-blue-600 dark:text-blue-400 mt-0.5 flex-shrink-0" />
            <div>
              <h4 className="text-sm font-medium text-blue-800 dark:text-blue-300 mb-1">
                Security Notice
              </h4>
              <p className="text-sm text-blue-700 dark:text-blue-300">
                This administrator account will have full access to the system.
                Keep your credentials secure and use a strong, unique password.
                You can create additional admin accounts later.
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
