//
//  campus-pilot
//  HitlistMatches.tsx
//
//  Created by Ngonidzashe Mangudya on 28/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { useState, useEffect } from "react";
import { AlertCircle, ExternalLink, Loader2, RotateCw } from "lucide-react";
import { cn } from "../../lib/utils";
import { apiClient } from "../../lib/api";
import type { tgHitListMatchesDTO } from "../../../src/types/hitlist-client";
import toast from "react-hot-toast";

export interface HitlistMatch {
  ResolveQueueId: number;
  ResolveQueue: string;
  URL: string;
}

export interface HitlistMatchesResponse {
  ExternalId: string;
  Matches: {
    $type: string;
    $values: HitlistMatch[];
  };
  ErrorOccurred: boolean;
}

interface HitlistMatchesProps {
  personId: number;
}

const fetchHitlistMatches = async (
  externalId: number,
): Promise<HitlistMatchesResponse> => {
  return await apiClient.getHitlistMatches(externalId);
};

export function HitlistMatches({ personId }: HitlistMatchesProps) {
  const [hitlistData, setHitlistData] = useState<HitlistMatchesResponse | null>(
    null,
  );
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadHitlistData = async () => {
    try {
      setIsLoading(true);
      setError(null);
      const data = await fetchHitlistMatches(personId);
      setHitlistData(data);
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : "Failed to load hitlist matches";
      setError(errorMessage);
      console.error("Failed to fetch hitlist matches:", err);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadHitlistData();
  }, [personId]);

  const handleRefresh = () => {
    loadHitlistData();
    toast.success("Hitlist matches refreshed");
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="flex items-center gap-3">
          <Loader2 className="w-6 h-6 animate-spin text-blue-600" />
          <span className="text-gray-600">Loading hitlist matches...</span>
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
            Failed to Load Hitlist Matches
          </h3>
          <p className="text-gray-600 mb-4">{error}</p>
          <button
            onClick={handleRefresh}
            className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
          >
            <RotateCw className="w-4 h-4 mr-2 inline" />
            Retry
          </button>
        </div>
      </div>
    );
  }

  if (!hitlistData || hitlistData.ErrorOccurred) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-center">
          <AlertCircle className="w-12 h-12 text-orange-500 mx-auto mb-4" />
          <h3 className="text-lg font-semibold text-gray-900 mb-2">
            Error Loading Hitlist Data
          </h3>
          <p className="text-gray-600 mb-4">
            An error occurred while fetching hitlist matches
          </p>
          <button
            onClick={handleRefresh}
            className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
          >
            <RotateCw className="w-4 h-4 mr-2 inline" />
            Retry
          </button>
        </div>
      </div>
    );
  }

  const matches = hitlistData.Matches?.$values || [];

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-semibold text-gray-900">
            Hitlist Matches
          </h2>
          <p className="text-sm text-gray-600 mt-1">
            ABIS biometric matches and watchlist hits for Person ID: {personId}
          </p>
        </div>
        <button
          onClick={handleRefresh}
          className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors flex items-center gap-2"
        >
          <RotateCw className="w-4 h-4" />
          Refresh
        </button>
      </div>

      {/* Matches Count */}
      <div className="bg-blue-50 border border-blue-200 rounded-lg p-4">
        <div className="flex items-center gap-2">
          <div className="w-2 h-2 bg-blue-500 rounded-full"></div>
          <span className="text-sm font-medium text-blue-900">
            {matches.length} match{matches.length !== 1 ? "es" : ""} found
          </span>
        </div>
      </div>

      {/* Matches List */}
      {matches.length > 0 ? (
        <div className="space-y-4">
          {matches.map((match, index) => (
            <div
              key={index}
              className="bg-white border border-gray-200 rounded-lg p-6 hover:shadow-md transition-shadow"
            >
              <div className="flex items-center justify-between">
                <div className="flex-1">
                  <div className="flex items-center gap-3 mb-2">
                    <span className="text-sm font-medium text-gray-500">
                      Queue ID: {match.ResolveQueueId}
                    </span>
                    <span className="text-gray-400">•</span>
                    <span
                      className={cn(
                        "px-2 py-1 rounded-full text-xs font-medium",
                        match.ResolveQueue === "Duplicates"
                          ? "bg-orange-100 text-orange-800"
                          : "bg-gray-100 text-gray-800",
                      )}
                    >
                      {match.ResolveQueue}
                    </span>
                  </div>
                  <div className="text-sm text-gray-600 font-mono break-all">
                    {match.URL}
                  </div>
                </div>
                <div className="ml-4">
                  <a
                    href={match.URL}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="inline-flex items-center gap-2 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors text-sm font-medium"
                  >
                    <ExternalLink className="w-4 h-4" />
                    Open
                  </a>
                </div>
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div className="text-center py-12">
          <div className="w-16 h-16 bg-green-100 rounded-full flex items-center justify-center mx-auto mb-4">
            <div className="w-6 h-6 bg-green-500 rounded-full"></div>
          </div>
          <h3 className="text-lg font-semibold text-gray-900 mb-2">
            No Matches Found
          </h3>
          <p className="text-gray-600">
            No biometric matches or watchlist hits were found for this person.
          </p>
        </div>
      )}
    </div>
  );
}
