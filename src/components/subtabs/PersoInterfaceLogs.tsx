//
//  campus-pilot
//  PersoInterfaceLogs.tsx
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { useState, useEffect } from "react";
import {
  FileText,
  AlertCircle,
  Loader2,
  CheckCircle,
  XCircle,
  Clock,
  ChevronDown,
  ChevronRight,
} from "lucide-react";
import { formatDate } from "../../lib/utils";
import { cn } from "../../lib/utils";
import { apiClient } from "../../lib/api";
import toast from "react-hot-toast";

interface PersoInterfaceLogsProps {
  applicationBarcode: string;
}

interface PersoLogRecord {
  tgpersointerfaceid: number;
  applicationbarcode: string;
  submission_status: "SUBMITTED" | "SUCCESS" | "FAILED";
  errordetail?: string;
  recordstatuslookupid?: number;
  createdate: Date;
  createdbysystemuserid: number;
  dataownerlookupid: number;
}

const getStatusIcon = (status: string) => {
  switch (status) {
    case "SUCCESS":
      return <CheckCircle className="w-5 h-5 text-green-600" />;
    case "FAILED":
      return <XCircle className="w-5 h-5 text-red-600" />;
    case "SUBMITTED":
      return <Clock className="w-5 h-5 text-blue-600" />;
    default:
      return <AlertCircle className="w-5 h-5 text-gray-600" />;
  }
};

const getStatusColor = (status: string) => {
  switch (status) {
    case "SUCCESS":
      return "text-green-700 bg-green-100";
    case "FAILED":
      return "text-red-700 bg-red-100";
    case "SUBMITTED":
      return "text-blue-700 bg-blue-100";
    default:
      return "text-gray-700 bg-gray-100";
  }
};

export function PersoInterfaceLogs({
  applicationBarcode,
}: PersoInterfaceLogsProps) {
  const [logs, setLogs] = useState<PersoLogRecord[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expandedLogs, setExpandedLogs] = useState<Set<number>>(new Set());

  // Fetch perso interface logs
  useEffect(() => {
    const fetchLogs = async () => {
      try {
        setIsLoading(true);
        setError(null);

        const data = await apiClient.getPersoInterfaceLogs(applicationBarcode);
        // Convert date strings to Date objects
        const processedData = data.map((log: any) => ({
          ...log,
          createdate: new Date(log.createdate),
        }));
        setLogs(processedData);
      } catch (err) {
        const errorMessage =
          err instanceof Error
            ? err.message
            : "Failed to load perso interface logs";
        setError(errorMessage);
        toast.error(errorMessage);
      } finally {
        setIsLoading(false);
      }
    };

    if (applicationBarcode) {
      fetchLogs();
    }
  }, [applicationBarcode]);

  const toggleExpanded = (logId: number) => {
    setExpandedLogs((prev) => {
      const newSet = new Set(prev);
      if (newSet.has(logId)) {
        newSet.delete(logId);
      } else {
        newSet.add(logId);
      }
      return newSet;
    });
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="flex items-center gap-3">
          <Loader2 className="w-6 h-6 animate-spin text-blue-600" />
          <span className="text-gray-600">Loading perso interface logs...</span>
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
            Failed to Load Perso Interface Logs
          </h3>
          <p className="text-gray-600 mb-4">{error}</p>
          <button
            onClick={() => window.location.reload()}
            className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700"
          >
            Retry
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-4">
          <h2 className="text-sm font-semibold flex items-center gap-2">
            <FileText className="w-4 h-4" />
            Perso Interface Logs ({logs.length})
          </h2>
          <span className="text-xs text-gray-500">
            Barcode: {applicationBarcode}
          </span>
        </div>
      </div>

      {/* Logs List */}
      {logs.length === 0 ? (
        <div className="text-center py-12 bg-gradient-to-br from-blue-50 to-indigo-50 rounded-lg border-2 border-dashed border-blue-200">
          <FileText className="w-16 h-16 text-blue-300 mx-auto mb-4" />
          <h3 className="text-lg font-semibold text-gray-900 mb-2">
            📝 No Interface Logs Found
          </h3>
          <p className="text-gray-600 mb-4 max-w-md mx-auto">
            No personalization interface logs have been recorded for this
            application yet. Logs will appear here when the system processes
            this application.
          </p>
          <div className="text-sm text-blue-700 bg-blue-100 px-4 py-2 rounded-lg inline-flex items-center gap-2">
            <span className="text-blue-500">💡</span>
            <span>
              Processing logs and system events will appear here automatically
            </span>
          </div>
        </div>
      ) : (
        <div className="space-y-3">
          {logs.map((log) => (
            <div
              key={log.tgpersointerfaceid}
              className="bg-white rounded-lg border border-gray-200 p-4"
            >
              <div className="flex items-start justify-between">
                <div className="flex items-start gap-4 flex-1">
                  <div className="w-12 h-12 bg-gray-100 rounded-lg flex items-center justify-center flex-shrink-0">
                    {getStatusIcon(log.submission_status)}
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-2">
                      <span
                        className={cn(
                          "px-2 py-1 rounded-full font-medium text-xs",
                          getStatusColor(log.submission_status),
                        )}
                      >
                        {log.submission_status}
                      </span>
                      <span className="text-sm text-gray-500">
                        {formatDate(log.createdate)}
                      </span>
                    </div>

                    {log.errordetail && (
                      <div className="mt-2">
                        <button
                          onClick={() => toggleExpanded(log.tgpersointerfaceid)}
                          className="flex items-center gap-1 text-sm text-gray-600 hover:text-gray-800"
                        >
                          {expandedLogs.has(log.tgpersointerfaceid) ? (
                            <ChevronDown className="w-4 h-4" />
                          ) : (
                            <ChevronRight className="w-4 h-4" />
                          )}
                          {expandedLogs.has(log.tgpersointerfaceid)
                            ? "Hide"
                            : "Show"}{" "}
                          Error Details
                        </button>

                        {expandedLogs.has(log.tgpersointerfaceid) && (
                          <div className="mt-2 p-3 bg-red-50 border border-red-200 rounded text-sm text-red-800">
                            <pre className="whitespace-pre-wrap font-mono text-xs">
                              {log.errordetail}
                            </pre>
                          </div>
                        )}
                      </div>
                    )}

                    <div className="flex items-center gap-3 text-[10px] text-muted-foreground mt-2">
                      <span>ID: {log.tgpersointerfaceid}</span>
                      <span>•</span>
                      <span>User: {log.createdbysystemuserid}</span>
                      {log.recordstatuslookupid && (
                        <>
                          <span>•</span>
                          <span>Status ID: {log.recordstatuslookupid}</span>
                        </>
                      )}
                    </div>
                  </div>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
