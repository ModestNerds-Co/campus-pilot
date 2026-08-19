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
      return <Sparkles className="w-4 h-4 text-green-600" />;
    case "fixed":
      return <Bug className="w-4 h-4 text-blue-600" />;
    case "improved":
      return <Zap className="w-4 h-4 text-purple-600" />;
    case "breaking":
      return <AlertTriangle className="w-4 h-4 text-red-600" />;
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
  new: "border-l-green-500 bg-green-50",
  fixed: "border-l-blue-500 bg-blue-50",
  improved: "border-l-purple-500 bg-purple-50",
  breaking: "border-l-red-500 bg-red-50",
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
      className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4"
      onClick={handleBackdropClick}
    >
      <div className="bg-white rounded-lg shadow-xl max-w-4xl w-full max-h-[90vh] overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between p-6 border-b border-gray-200 bg-gradient-to-r from-blue-600 to-purple-600 text-white">
          <div className="flex items-center gap-3">
            <Tag className="w-6 h-6" />
            <div>
              <h2 className="text-xl font-semibold">What's New</h2>
              <p className="text-blue-100 text-sm">
                TGPatcher v{currentVersion}
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1 hover:bg-white hover:bg-opacity-20 rounded-lg transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Content */}
        <div className="overflow-y-auto max-h-[calc(90vh-140px)]">
          {entries.length === 0 ? (
            <div className="p-8 text-center">
              <p className="text-gray-500">No new changes to display.</p>
            </div>
          ) : (
            <div className="p-6 space-y-8">
              {entries.map((entry, index) => (
                <div key={entry.version} className="space-y-4">
                  {/* Version Header */}
                  <div className="flex items-center gap-3 pb-2 border-b border-gray-200">
                    <div className="flex items-center gap-2">
                      <span className="inline-flex items-center px-3 py-1 rounded-full text-sm font-medium bg-blue-100 text-blue-800">
                        v{entry.version}
                      </span>
                      <div className="flex items-center gap-1 text-gray-500 text-sm">
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
                            <h4 className="font-medium text-gray-900">
                              <ChangeTypeLabel type={type} />
                            </h4>
                            <span className="text-sm text-gray-500">
                              ({changes.length})
                            </span>
                          </div>
                          <div className="space-y-2">
                            {changes.map((change, changeIndex) => (
                              <div
                                key={changeIndex}
                                className={`pl-4 border-l-4 py-2 px-3 rounded-r ${
                                  ChangeTypeColors[
                                    type as keyof typeof ChangeTypeColors
                                  ]
                                }`}
                              >
                                <p className="text-sm text-gray-700">
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
                    <div className="border-t border-gray-200 pt-6" />
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
