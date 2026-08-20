//
//  campus-pilot
//  SchoolPreviewCard.tsx - Live Preview Component (token-driven, huchu elegance)
//  Canvas-neutral chrome, token surfaces/borders/text/brand. No literal grays/blues.
//

import React from "react";
import { School, Mail, Phone, MapPin, WifiOff } from "lucide-react";
import type { SchoolFormData, LogoPreview } from "../../types";

interface SchoolPreviewCardProps {
  schoolData: SchoolFormData;
  logoPreview: {
    light?: LogoPreview;
    dark?: LogoPreview;
  };
}

export const SchoolPreviewCard: React.FC<SchoolPreviewCardProps> = ({
  schoolData,
  logoPreview,
}) => {
  const getInitials = (name: string) => {
    return name
      .split(" ")
      .map((word) => word.charAt(0))
      .join("")
      .toUpperCase()
      .substring(0, 2);
  };

  const formatAddress = () => {
    const parts = [
      schoolData.address_line1,
      schoolData.city,
      schoolData.province,
      schoolData.country,
    ].filter(Boolean);

    return parts.join(", ") || "No address provided";
  };

  return (
    <div className="space-y-6">
      {/* Preview Header */}
      <div className="text-center">
        <h3 className="text-[length:var(--type-section-title-size)] font-bold text-[var(--text-strong)] mb-2">
          Live Preview
        </h3>
        <p className="text-sm text-[var(--text-muted)]">
          See how your school information will appear
        </p>
        <div className="mt-3 inline-flex items-center gap-2 px-3 py-1 bg-[var(--tone-success-bg)] border border-[var(--tone-success-bd)] rounded-full text-sm text-[var(--tone-success-strong)]">
          <WifiOff className="w-3 h-3" />
          Works offline
        </div>
      </div>

      {/* Login Screen Preview */}
      <div className="bg-[var(--surface)] rounded-[var(--radius-2xl)] border border-[var(--border)] p-6 shadow-[var(--shadow-popover)]">
        <div className="text-center space-y-4">
          <div className="w-16 h-16 mx-auto bg-[var(--brand-soft)] border border-[var(--brand-100)] rounded-full flex items-center justify-center">
            {logoPreview.light ? (
              <img
                src={logoPreview.light.url}
                alt="School logo"
                className="w-12 h-12 object-contain rounded-full"
              />
            ) : schoolData.name ? (
              <span className="text-[var(--brand)] font-bold text-lg">
                {getInitials(schoolData.name)}
              </span>
            ) : (
              <School className="w-8 h-8 text-[var(--brand)]" />
            )}
          </div>

          <div>
            <h4 className="text-xl font-bold text-[var(--text-strong)]">
              {schoolData.name || "Your School Name"}
            </h4>
            {schoolData.legal_name && (
              <p className="text-sm text-[var(--text-muted)] mt-1">
                {schoolData.legal_name}
              </p>
            )}
          </div>

          <div className="bg-[var(--surface-muted)] rounded-[var(--radius-lg)] p-4 space-y-3">
            <h5 className="text-sm font-medium text-[var(--text-body)]">
              Login Preview
            </h5>
            <div className="space-y-2">
              <input
                type="text"
                placeholder="Email"
                className="w-full px-3 h-[var(--h-control-md)] border border-[var(--input-border)] bg-[var(--input-bg)] text-[var(--text-strong)] placeholder:text-[var(--text-subtle)] rounded-[var(--radius-md)] text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]"
                disabled
              />
              <input
                type="password"
                placeholder="Password"
                className="w-full px-3 h-[var(--h-control-md)] border border-[var(--input-border)] bg-[var(--input-bg)] text-[var(--text-strong)] placeholder:text-[var(--text-subtle)] rounded-[var(--radius-md)] text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]"
                disabled
              />
              <button className="w-full px-3 h-[var(--h-control-md)] bg-[var(--action-primary-bg)] hover:bg-[var(--action-primary-bg-hover)] active:bg-[var(--action-primary-bg-pressed)] text-[var(--action-primary-fg)] rounded-[var(--radius-md)] text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-2">
                Sign In
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* Receipt Header Preview */}
      <div className="bg-[var(--surface)] rounded-[var(--radius-2xl)] border border-[var(--border)] p-6 shadow-[var(--shadow-popover)]">
        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 bg-[var(--brand-soft)] border border-[var(--brand-100)] rounded-full flex items-center justify-center">
                {logoPreview.light ? (
                  <img
                    src={logoPreview.light.url}
                    alt="School logo"
                    className="w-8 h-8 object-contain rounded-full"
                  />
                ) : schoolData.name ? (
                  <span className="text-[var(--brand)] font-bold text-sm">
                    {getInitials(schoolData.name)}
                  </span>
                ) : (
                  <School className="w-6 h-6 text-[var(--brand)]" />
                )}
              </div>
              <div>
                <h5 className="font-semibold text-[var(--text-strong)] text-sm">
                  {schoolData.name || "Your School Name"}
                </h5>
                <p className="text-xs text-[var(--text-muted)]">
                  Receipt Preview
                </p>
              </div>
            </div>
          </div>

          <div className="border-t border-[var(--border)] pt-4 space-y-2">
            {schoolData.email && (
              <div className="flex items-center gap-2 text-xs text-[var(--text-muted)]">
                <Mail className="w-3 h-3" />
                {schoolData.email}
              </div>
            )}
            {schoolData.phone && (
              <div className="flex items-center gap-2 text-xs text-[var(--text-muted)]">
                <Phone className="w-3 h-3" />
                {schoolData.phone}
              </div>
            )}
            <div className="flex items-start gap-2 text-xs text-[var(--text-muted)]">
              <MapPin className="w-3 h-3 mt-0.5 flex-shrink-0" />
              <span>{formatAddress()}</span>
            </div>
            {schoolData.emap_code && (
              <div className="text-xs text-[var(--text-subtle)]">
                EMAP: {schoolData.emap_code}
              </div>
            )}
          </div>

          <div className="border-t border-[var(--border)] pt-3">
            <div className="bg-[var(--surface-muted)] rounded p-3 text-xs text-[var(--text-muted)]">
              <div className="flex justify-between">
                <span>Date:</span>
                <span>26 Sep 2025</span>
              </div>
              <div className="flex justify-between">
                <span>Receipt #:</span>
                <span>REC-001</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Configuration Summary */}
      <div className="bg-[var(--surface)] rounded-[var(--radius-2xl)] border border-[var(--border)] p-6 shadow-[var(--shadow-popover)]">
        <h5 className="font-semibold text-[var(--text-strong)] mb-4 text-sm">
          Configuration
        </h5>
        <div className="space-y-3 text-xs">
          <div className="flex justify-between">
            <span className="text-[var(--text-muted)]">Timezone:</span>
            <span className="text-[var(--text-strong)]">
              {schoolData.timezone}
            </span>
          </div>
          <div className="flex justify-between">
            <span className="text-[var(--text-muted)]">Language:</span>
            <span className="text-[var(--text-strong)]">
              {schoolData.locale}
            </span>
          </div>
          <div className="flex justify-between">
            <span className="text-[var(--text-muted)]">Logos:</span>
            <span className="text-[var(--text-strong)]">
              {logoPreview.light && logoPreview.dark
                ? "Light + Dark"
                : logoPreview.light
                  ? "Light only"
                  : logoPreview.dark
                    ? "Dark only"
                    : "None"}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
};
