//
//  campus-pilot
//  ApplicationStatusRemarks.tsx
//
//  Created by Ngonidzashe Mangudya on 04/09/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { useState, useEffect } from "react";
import { format } from "date-fns";
import { parseRemarkWithAbisCode } from "../../lib/abis-codes";

interface ApplicationStatusRemark {
  tgapplicationstatusremarkid: number;
  tgapplicationid: number;
  tgapplicationworkflowhistoryid: number | null;
  statusreasonlookupid: number;
  applicationstatusreason: string | null;
  remark: string | null;
  portalrecordstatuslookupid: number | null;
  recordstatuslookupid: number | null;
  createdate: string;
  modifieddate: string | null;
  tguserauditdetailid: number;
  createdbysystemuserid: number;
  updatedbysystemuserid: number | null;
  dataownerlookupid: number;
  isactive: number;
  created_by_name?: string;
  updated_by_name?: string;
  status_reason_description?: string;
}

interface ApplicationStatusRemarksProps {
  tgApplicationId: number;
}

export default function ApplicationStatusRemarks({
  tgApplicationId,
}: ApplicationStatusRemarksProps) {
  const [remarks, setRemarks] = useState<ApplicationStatusRemark[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (tgApplicationId) {
      fetchRemarks();
    }
  }, [tgApplicationId]);

  const fetchRemarks = async () => {
    try {
      setLoading(true);
      const response = await fetch(
        `/api/applications/${tgApplicationId}/status-remarks`,
      );

      if (!response.ok) {
        throw new Error(
          `Failed to fetch status remarks: ${response.statusText}`,
        );
      }

      const data = await response.json();
      setRemarks(data);
    } catch (err) {
      console.error("Error fetching status remarks:", err);
      setError(
        err instanceof Error ? err.message : "Failed to fetch status remarks",
      );
    } finally {
      setLoading(false);
    }
  };

  const formatDate = (dateString: string) => {
    try {
      return format(new Date(dateString), "dd/MM/yyyy HH:mm");
    } catch {
      return dateString;
    }
  };

  const renderRemarkContent = (remark: string | null) => {
    if (!remark) return <span className="text-gray-400">No remark</span>;

    const parsed = parseRemarkWithAbisCode(remark);

    if (parsed.hasAbisCode) {
      return (
        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <span className="inline-flex items-center px-2 py-1 text-xs font-medium bg-red-100 text-red-800 rounded-full">
              {parsed.code}
            </span>
            <span className="text-xs text-gray-500">ABIS Error Code</span>
          </div>
          {parsed.description && (
            <div className="text-sm text-gray-700 whitespace-pre-line bg-gray-50 p-3 rounded border-l-4 border-red-400">
              {parsed.description}
            </div>
          )}
        </div>
      );
    }

    return (
      <div className="text-sm text-gray-700 whitespace-pre-line">{remark}</div>
    );
  };

  if (loading) {
    return (
      <div className="p-6">
        <div className="animate-pulse space-y-4">
          <div className="h-4 bg-gray-200 rounded w-1/4"></div>
          <div className="space-y-3">
            <div className="h-20 bg-gray-200 rounded"></div>
            <div className="h-20 bg-gray-200 rounded"></div>
            <div className="h-20 bg-gray-200 rounded"></div>
          </div>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-6">
        <div className="bg-red-50 border border-red-200 rounded-md p-4">
          <div className="flex">
            <div className="flex-shrink-0">
              <svg
                className="h-5 w-5 text-red-400"
                viewBox="0 0 20 20"
                fill="currentColor"
              >
                <path
                  fillRule="evenodd"
                  d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z"
                  clipRule="evenodd"
                />
              </svg>
            </div>
            <div className="ml-3">
              <h3 className="text-sm font-medium text-red-800">
                Error Loading Status Remarks
              </h3>
              <p className="text-sm text-red-700 mt-1">{error}</p>
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-medium text-gray-900">
          Application Status Remarks
        </h3>
        <span className="text-sm text-gray-500">
          {remarks.length} {remarks.length === 1 ? "remark" : "remarks"}
        </span>
      </div>

      {remarks.length === 0 ? (
        <div className="text-center py-12">
          <svg
            className="mx-auto h-12 w-12 text-gray-400"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M7 8h10m0 0V6a2 2 0 00-2-2H9a2 2 0 00-2 2v2m10 0v10a2 2 0 01-2 2H9a2 2 0 01-2-2V8m0 0V6a2 2 0 012-2h10a2 2 0 012 2v2"
            />
          </svg>
          <h3 className="mt-2 text-sm font-medium text-gray-900">
            No status remarks
          </h3>
          <p className="mt-1 text-sm text-gray-500">
            No status remarks have been recorded for this application yet.
          </p>
        </div>
      ) : (
        <div className="space-y-4">
          {remarks.map((remark) => (
            <div
              key={remark.tgapplicationstatusremarkid}
              className="bg-white border border-gray-200 rounded-lg shadow-sm"
            >
              <div className="px-6 py-4 border-b border-gray-200 bg-gray-50">
                <div className="flex items-center justify-between">
                  <div className="flex items-center space-x-4">
                    <div>
                      <p className="text-sm font-medium text-gray-900">
                        {remark.status_reason_description ||
                          `Status Reason ID: ${remark.statusreasonlookupid}`}
                      </p>
                      {remark.applicationstatusreason && (
                        <p className="text-sm text-gray-600">
                          {remark.applicationstatusreason}
                        </p>
                      )}
                    </div>
                  </div>
                  <div className="text-right">
                    <p className="text-sm text-gray-500">
                      Created: {formatDate(remark.createdate)}
                    </p>
                    {remark.created_by_name && (
                      <p className="text-xs text-gray-400">
                        by {remark.created_by_name}
                      </p>
                    )}
                  </div>
                </div>
              </div>

              <div className="px-6 py-4">
                {renderRemarkContent(remark.remark)}
              </div>

              {remark.modifieddate &&
                remark.modifieddate !== remark.createdate && (
                  <div className="px-6 py-2 bg-gray-50 border-t border-gray-200">
                    <p className="text-xs text-gray-500">
                      Last modified: {formatDate(remark.modifieddate)}
                      {remark.updated_by_name &&
                        ` by ${remark.updated_by_name}`}
                    </p>
                  </div>
                )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
