//
//  campus-pilot
//  DocumentViewer.tsx
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import React from "react";
import { X, Download, FileText, Loader2, AlertCircle } from "lucide-react";
import {
  detectDocumentTypeFromBase64,
  createDownloadLinkFromBase64,
} from "../lib/utils";
import toast from "react-hot-toast";

interface DocumentViewerProps {
  isOpen: boolean;
  onClose: () => void;
  documentData: string | null;
  loading: boolean;
  documentId?: number;
  title?: string;
}

export const DocumentViewer: React.FC<DocumentViewerProps> = ({
  isOpen,
  onClose,
  documentData,
  loading,
  documentId,
  title = "Document Viewer",
}) => {
  if (!isOpen) return null;

  const handleDownload = () => {
    if (!documentData) return;

    try {
      const docType = detectDocumentTypeFromBase64(documentData);
      const filename = documentId
        ? `document_${documentId}.${docType.extension}`
        : `document.${docType.extension}`;

      const downloadUrl = createDownloadLinkFromBase64(documentData, filename);
      const a = document.createElement("a");
      a.href = downloadUrl;
      a.download = filename;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(downloadUrl);
      toast.success("Document downloaded successfully");
    } catch (error) {
      toast.error("Failed to download document");
    }
  };

  const renderDocumentContent = () => {
    if (loading) {
      return (
        <div className="flex items-center justify-center h-full">
          <div className="text-center">
            <Loader2 className="mx-auto mb-2 size-8 animate-spin text-[var(--brand)]" />
            <p className="text-sm text-[var(--text-muted)]">Loading document...</p>
          </div>
        </div>
      );
    }

    if (!documentData) {
      return (
        <div className="flex items-center justify-center h-full">
          <div className="text-center">
            <AlertCircle className="mx-auto mb-2 size-12 text-[var(--text-subtle)]" />
            <p className="text-sm text-[var(--text-muted)]">Failed to load document</p>
          </div>
        </div>
      );
    }

    const docType = detectDocumentTypeFromBase64(documentData);

    if (docType.type === "pdf") {
      const pdfBlob = createDownloadLinkFromBase64(
        documentData,
        "document.pdf",
      );
      return (
        <iframe src={pdfBlob} className="w-full h-full" title="Document PDF" />
      );
    } else if (docType.type === "image") {
      const imageUrl = documentData.startsWith("data:")
        ? documentData
        : `data:${docType.mimeType};base64,${documentData}`;
      return (
        <div className="flex items-center justify-center h-full p-4">
          <img
            src={imageUrl}
            alt="Document"
            className="max-w-full max-h-full object-contain"
            style={{ maxHeight: "calc(100vh - 200px)" }}
          />
        </div>
      );
    } else {
      return (
        <div className="flex flex-col items-center justify-center h-full">
          <FileText className="w-16 h-16 text-[var(--text-subtle)] mb-4" />
          <h3 className="text-lg font-semibold text-[var(--text-strong)] mb-2">
            Unknown Document Type
          </h3>
          <p className="text-[var(--text-muted)] text-center mb-4">
            File type: {docType.mimeType}
          </p>
          <button
            onClick={handleDownload}
            className="h-[var(--h-control-md)] px-4 bg-[var(--action-primary-bg)] hover:bg-[var(--action-primary-bg-hover)] text-[var(--action-primary-fg)] rounded-[var(--radius-md)] flex items-center gap-2 text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-2"
          >
            <Download className="w-4 h-4" />
            Download Document
          </button>
        </div>
      );
    }
  };

  return (
    <div
      className="fixed inset-0 bg-[var(--surface-overlay)] backdrop-blur-[2px] flex items-center justify-center z-[var(--z-overlay)] p-4"
      onClick={onClose}
    >
      <div
        className="bg-[var(--surface)] rounded-[var(--radius-xl)] border border-[var(--border)] shadow-[var(--shadow-modal)] max-w-6xl w-full h-full m-4 overflow-hidden flex flex-col"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between p-4 border-b border-[var(--border)]">
          <h3 className="text-lg font-semibold text-[var(--text-strong)]">
            {documentId ? `${title} - ID: ${documentId}` : title}
          </h3>
          <div className="flex items-center gap-2">
            {documentData && !loading && (
              <button
                onClick={handleDownload}
                className="h-[var(--h-control-md)] px-3 bg-[var(--action-primary-bg)] hover:bg-[var(--action-primary-bg-hover)] text-[var(--action-primary-fg)] rounded-[var(--radius-md)] flex items-center gap-1 text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-2"
              >
                <Download className="w-4 h-4" />
                Download
              </button>
            )}
            <button
              onClick={onClose}
              className="p-2 hover:bg-[var(--surface-muted)] rounded-full text-[var(--text-muted)] hover:text-[var(--text-strong)] transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>
        <div className="flex-1 overflow-hidden">{renderDocumentContent()}</div>
      </div>
    </div>
  );
};
