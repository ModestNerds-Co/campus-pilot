//
//  campus-pilot
//  ChangelogModal.tsx
//
//  Created by Ngonidzashe Mangudya on 28/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import React from "react";
import {
  X,
  Sparkles,
  Bug,
  Zap,
  AlertTriangle,
  Calendar,
  Tag,
} from "lucide-react";
import { ChangelogEntry } from "../lib/version";

interface ChangelogModalProps {
  isOpen: boolean;
  onClose: () => void;
  entries: ChangelogEntry[];
  currentVersion: string;
}

const ChangeTypeIcon = ({ type }: { type: string }) => {
  switch (type) {
    case "new":
      return <Sparkles className="w-4 h-4 text-[var(--tone-success)]" />;
    case "fixed":
      return <Bug className="w-4 h-4 text-[var(--tone-info)]" />;
    case "improved":
      return <Zap className="w-4 h-4 text-[var(--accent-500)]" />;
    case "breaking":
      return <AlertTriangle className="w-4 h-4 text-[var(--tone-danger)]" />;
    default:
      return null;
  }
};

const ChangeTypeLabel = ({ type }: { type: string }) => {
  const labels = {
    new: "New Features",
    fixed: "Bug Fixes",
    improved: "Improvements",
    breaking: "Breaking Changes",
  };
  return labels[type as keyof typeof labels] || type;
};

const ChangeTypeColors = {
  new: "border-l-[var(--tone-success)] bg-[var(--tone-success-bg)]",
  fixed: "border-l-[var(--tone-info)] bg-[var(--tone-info-bg)]",
  improved: "border-l-[var(--accent-500)] bg-[var(--accent-50)]",
  breaking: "border-l-[var(--tone-danger)] bg-[var(--tone-danger-bg)]",
};

export const ChangelogModal: React.FC<ChangelogModalProps> = ({
  isOpen,
  onClose,
  entries,
  currentVersion,
}) => {
  if (!isOpen) return null;

  const handleBackdropClick = (e: React.MouseEvent) => {
    if (e.target === e.currentTarget) {
      onClose();
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-[var(--surface-overlay)] p-4"
      onClick={handleBackdropClick}
    >
      <div className="max-h-[90vh] w-full max-w-4xl overflow-hidden rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-modal)]">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-[var(--border)] bg-[var(--brand)] p-6 text-white">
          <div className="flex items-center gap-3">
            <Tag className="w-6 h-6" />
            <div>
              <h2 className="text-xl font-semibold">What's New</h2>
              <p className="text-white/80 text-sm">
                TGPatcher v{currentVersion}
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1 hover:bg-white/20 rounded-[var(--radius-sm)] transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--brand)]"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Content */}
        <div className="overflow-y-auto max-h-[calc(90vh-140px)]">
          {entries.length === 0 ? (
            <div className="p-8 text-center">
              <p className="text-[var(--text-muted)]">No new changes to display.</p>
            </div>
          ) : (
            <div className="p-6 space-y-8">
              {entries.map((entry, index) => (
                <div key={entry.version} className="space-y-4">
                  {/* Version Header */}
                  <div className="flex items-center gap-3 pb-2 border-b border-[var(--border)]">
                    <div className="flex items-center gap-2">
                      <span className="inline-flex items-center px-3 py-1 rounded-full text-sm font-medium bg-[var(--badge-brand-bg)] text-[var(--badge-brand-text)] border border-[var(--tone-info-bd)]">
                        v{entry.version}
                      </span>
                      <div className="flex items-center gap-1 text-[var(--text-muted)] text-sm">
                        <Calendar className="w-4 h-4" />
                        {new Date(entry.date).toLocaleDateString()}
                      </div>
                    </div>
                  </div>

                  {/* Changes */}
                  <div className="space-y-6">
                    {Object.entries(entry.changes).map(([type, changes]) => {
                      if (!changes || changes.length === 0) return null;

                      return (
                        <div key={type} className="space-y-2">
                          <div className="flex items-center gap-2">
                            <ChangeTypeIcon type={type} />
                            <h4 className="font-medium text-[var(--text-strong)]">
                              <ChangeTypeLabel type={type} />
                            </h4>
                            <span className="text-sm text-[var(--text-muted)]">
                              ({changes.length})
                            </span>
                          </div>
                          <div className="space-y-2">
                            {changes.map((change, changeIndex) => (
                              <div
                                key={changeIndex}
                                className={`pl-4 border-l-4 py-2 px-3 rounded-r-[var(--radius-sm)] ${
                                  ChangeTypeColors[
                                    type as keyof typeof ChangeTypeColors
                                  ]
                                }`}
                              >
                                <p className="text-sm text-[var(--text-body)]">
                                  {change}
                                </p>
                              </div>
                            ))}
                          </div>
                        </div>
                      );
                    })}
                  </div>

                  {/* Separator for multiple versions */}
                  {index < entries.length - 1 && (
                    <div className="border-t border-[var(--border)] pt-6" />
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
