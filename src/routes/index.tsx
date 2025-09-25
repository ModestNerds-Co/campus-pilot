//
//  campus-pilot
//  index.tsx
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { createFileRoute } from "@tanstack/react-router";
import { useState, useEffect } from "react";
import {
  Search,
  Loader2,
  AlertCircle,
  ExternalLink,
  Filter,
  FileText,
  User,
  Clock,
  Building,
  ScanLine,
  Zap,
} from "lucide-react";
import { useUIStore } from "../stores/uiStore";
import { useNavigate } from "@tanstack/react-router";
import { cn } from "../lib/utils";
import { logger } from "../lib/logger";
import { apiClient, type ApplicationSearchResult } from "../lib/api";
import toast from "react-hot-toast";

export const Route = createFileRoute("/")({
  component: SearchPage,
});

type SearchType = "reference" | "applicationId" | "name";

function SearchPage() {
  const [searchType, setSearchType] = useState<SearchType>("reference");
  const [searchValue, setSearchValue] = useState("");
  const [isSearching, setIsSearching] = useState(false);
  const [searchResults, setSearchResults] = useState<ApplicationSearchResult[]>(
    [],
  );
  const [hasSearched, setHasSearched] = useState(false);
  const [isHardwareScannerActive, setIsHardwareScannerActive] = useState(false);
  const [lastScanTime, setLastScanTime] = useState(0);

  const { addTab, createTabId } = useUIStore();
  const navigate = useNavigate();

  useEffect(() => {
    logger.info("Search page loaded", null, "SearchPage", "mount");
    return () => {
      logger.debug("Search page unmounted", null, "SearchPage", "unmount");
    };
  }, []);

  const handleSearch = async () => {
    if (!searchValue.trim()) {
      toast.error("Please enter a search value");
      logger.validation("searchValue", searchValue, "Search value is required");
      return;
    }

    logger.userAction("SearchPage", "search", { searchType, searchValue });
    logger.search(searchType, searchValue, 0);

    setIsSearching(true);
    setHasSearched(true);

    try {
      const results = await apiClient.searchApplications(
        searchType,
        searchValue,
      );
      setSearchResults(results);

      logger.search(searchType, searchValue, results.length);

      if (results.length > 0) {
        logger.info(
          `Search completed: ${results.length} results found`,
          { searchType, searchValue, resultCount: results.length },
          "SearchPage",
          "searchComplete",
        );

        const hasDuplicates = results.some(
          (r: ApplicationSearchResult) => r.isDuplicate,
        );
        if (hasDuplicates) {
          toast(
            "Multiple applications found with the same reference. Please select the specific application ID.",
            {
              icon: "⚠️",
              duration: 6000,
            },
          );
        } else {
          toast.success(`Found ${results.length} application(s)`);
        }
      } else {
        logger.info(
          "Search completed: No results found",
          { searchType, searchValue },
          "SearchPage",
          "searchComplete",
        );
        toast("No results found. Please check your search criteria.", {
          icon: "ℹ️",
        });
      }
    } catch (error) {
      const errorMessage =
        error instanceof Error
          ? error.message
          : "Search failed. Please try again.";
      logger.error("Search failed", error, "SearchPage", "search");
      toast.error(errorMessage);
    } finally {
      setIsSearching(false);
    }
  };

  // Hardware scanner detection - looks for rapid keystrokes followed by Enter
  useEffect(() => {
    let scanBuffer = "";
    let scanTimeout: NodeJS.Timeout;

    const handleKeyPress = (event: KeyboardEvent) => {
      // Only process if we're on reference search and not typing in an input field
      if (searchType !== "reference") return;

      const target = event.target as HTMLElement;
      if (target.tagName === "INPUT" || target.tagName === "TEXTAREA") return;

      const currentTime = Date.now();

      // If Enter key and we have a scan buffer, process as barcode
      if (event.key === "Enter" && scanBuffer.length > 0) {
        event.preventDefault();
        const scannedValue = scanBuffer.trim();

        if (scannedValue.length > 3) {
          // Minimum barcode length
          logger.userAction("SearchPage", "hardwareBarcodeScanned", {
            scannedValue,
            bufferLength: scanBuffer.length,
          });

          // Set search value and trigger search
          setSearchValue(scannedValue);
          setSearchType("reference");
          setLastScanTime(currentTime);
          toast.success(`Hardware scanner detected: ${scannedValue}`);

          // Clear buffer and trigger search
          scanBuffer = "";
          setTimeout(() => handleSearch(), 100);
        }
        return;
      }

      // Build scan buffer for rapid keystrokes (typical of hardware scanners)
      if (event.key.length === 1 && /[a-zA-Z0-9\-_]/.test(event.key)) {
        const timeSinceLastKey = currentTime - lastScanTime;

        // If too much time has passed, start new buffer (not a scanner)
        if (timeSinceLastKey > 100) {
          scanBuffer = "";
        }

        scanBuffer += event.key;
        setLastScanTime(currentTime);

        // Auto-clear buffer after short delay if no Enter comes
        clearTimeout(scanTimeout);
        scanTimeout = setTimeout(() => {
          scanBuffer = "";
        }, 500);

        // Show visual feedback when building scan buffer
        if (scanBuffer.length > 5) {
          setIsHardwareScannerActive(true);
          setTimeout(() => setIsHardwareScannerActive(false), 1000);
        }
      }
    };

    document.addEventListener("keydown", handleKeyPress);

    return () => {
      document.removeEventListener("keydown", handleKeyPress);
      clearTimeout(scanTimeout);
    };
  }, [searchType, handleSearch, lastScanTime]);

  const handleOpenCase = (
    result: ApplicationSearchResult,
    newTab: boolean = false,
  ) => {
    logger.info(
      `Opening case: ${result.tgapplicationid}`,
      {
        applicationId: result.tgapplicationid,
        reference: result.reference,
        newTab,
      },
      "SearchPage",
      "openCase",
    );

    const tabId = createTabId();

    addTab({
      id: tabId,
      tgapplicationid: result.tgapplicationid,
      reference: result.reference,
      isDirty: false,
      stagedChanges: {},
      loadedAt: new Date(),
      modifiedDates: {
        application: new Date(result.modifieddate),
      },
    });

    navigate({
      to: "/case/$applicationId",
      params: { applicationId: result.tgapplicationid.toString() },
    } as any);

    toast.success(`Opened application: ${result.reference}`);
  };

  const handleReactivateApplication = async (applicationId: number) => {
    const confirmed = window.confirm(
      "Are you sure you want to reactivate this application?",
    );

    if (!confirmed) return;

    try {
      const response = await fetch(
        `/api/applications/${applicationId}/reactivate`,
        {
          method: "PATCH",
          headers: {
            "Content-Type": "application/json",
          },
        },
      );

      if (!response.ok) {
        const error = await response.json();
        throw new Error(error.message || "Failed to reactivate application");
      }

      // Refresh search results to show the updated status
      if (searchResults) {
        setSearchResults(
          searchResults.map((result) =>
            result.tgapplicationid === applicationId
              ? { ...result, isactive: 1 }
              : result,
          ),
        );
      }

      toast.success("Application reactivated successfully");
    } catch (error) {
      const errorMessage =
        error instanceof Error
          ? error.message
          : "Failed to reactivate application";
      toast.error(errorMessage);
    }
  };

  const formatDate = (date: Date | string) => {
    const d = typeof date === "string" ? new Date(date) : date;
    if (isNaN(d.getTime())) return "-";

    return new Intl.DateTimeFormat("en-US", {
      month: "short",
      day: "numeric",
      year: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    }).format(d);
  };

  const getStatusColor = (status: string) => {
    const lower = status.toLowerCase();
    if (lower.includes("progress") || lower.includes("pending"))
      return "bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400";
    if (lower.includes("review") || lower.includes("processing"))
      return "bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400";
    if (lower.includes("approved") || lower.includes("complete"))
      return "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400";
    if (lower.includes("rejected") || lower.includes("failed"))
      return "bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400";
    return "bg-gray-100 text-gray-800 dark:bg-gray-900/30 dark:text-gray-400";
  };

  return (
    <div className="min-h-full bg-gradient-to-br from-blue-50 via-white to-gray-50">
      {/* Hero Section */}
      <div className="bg-white/80 backdrop-blur-sm border-b border-gray-200">
        <div className="max-w-4xl mx-auto px-4 sm:px-6 lg:px-8 py-12">
          <div className="text-center mb-12">
            <h1 className="text-4xl font-bold text-gray-900 mb-4">
              Immigration Application Search
            </h1>
            <p className="text-lg text-gray-600 max-w-xl mx-auto leading-relaxed">
              Search and manage immigration applications by reference number,
              application ID, or applicant name. Use hardware barcode scanners
              for instant access to detailed records, biometric data, and
              workflow history.
            </p>
          </div>

          {/* Search Form */}
          <div className="max-w-2xl mx-auto">
            <div className="bg-white rounded-xl shadow-lg border border-gray-200 p-8">
              {/* Search Type Selector */}
              <div className="flex items-center justify-center space-x-8 mb-8">
                <label className="flex items-center cursor-pointer group">
                  <input
                    type="radio"
                    value="reference"
                    checked={searchType === "reference"}
                    onChange={(e) => {
                      setSearchType(e.target.value as SearchType);
                      logger.userAction("SearchPage", "changeSearchType", {
                        searchType: e.target.value,
                      });
                    }}
                    className="w-4 h-4 text-blue-600 border-gray-300 focus:ring-blue-500"
                  />
                  <span className="ml-3 text-base font-medium text-gray-700 group-hover:text-blue-600 transition-colors">
                    <FileText className="inline w-5 h-5 mr-2" />
                    Reference (Barcode)
                  </span>
                </label>
                <label className="flex items-center cursor-pointer group">
                  <input
                    type="radio"
                    value="applicationId"
                    checked={searchType === "applicationId"}
                    onChange={(e) => {
                      setSearchType(e.target.value as SearchType);
                      logger.userAction("SearchPage", "changeSearchType", {
                        searchType: e.target.value,
                      });
                    }}
                    className="w-4 h-4 text-blue-600 border-gray-300 focus:ring-blue-500"
                  />
                  <span className="ml-3 text-base font-medium text-gray-700 group-hover:text-blue-600 transition-colors">
                    <Building className="inline w-5 h-5 mr-2" />
                    Application ID
                  </span>
                </label>
                <label className="flex items-center cursor-pointer group">
                  <input
                    type="radio"
                    value="name"
                    checked={searchType === "name"}
                    onChange={(e) => {
                      setSearchType(e.target.value as SearchType);
                      logger.userAction("SearchPage", "changeSearchType", {
                        searchType: e.target.value,
                      });
                    }}
                    className="w-4 h-4 text-blue-600 border-gray-300 focus:ring-blue-500"
                  />
                  <span className="ml-3 text-base font-medium text-gray-700 group-hover:text-blue-600 transition-colors">
                    <User className="inline w-5 h-5 mr-2" />
                    Applicant Name
                  </span>
                </label>
              </div>

              {/* Search Input */}
              <div className="space-y-4">
                <div className="relative">
                  <Search className="absolute left-4 top-1/2 transform -translate-y-1/2 w-5 h-5 text-gray-400" />
                  <input
                    type="text"
                    value={searchValue}
                    onChange={(e) => {
                      setSearchValue(e.target.value);
                      logger.debug(
                        "Search value changed",
                        e.target.value,
                        "SearchPage",
                        "inputChange",
                      );
                    }}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        logger.userAction("SearchPage", "searchKeyPress", {
                          key: "Enter",
                        });
                        handleSearch();
                      }
                    }}
                    placeholder={
                      searchType === "reference"
                        ? "Enter reference/barcode number or use hardware scanner..."
                        : searchType === "applicationId"
                          ? "Enter application ID..."
                          : "Enter applicant name (first, last, or full name)..."
                    }
                    className={cn(
                      "w-full pl-12 pr-16 py-4 text-base border rounded-xl focus:outline-none bg-white text-gray-900 placeholder-gray-400 transition-all shadow-sm",
                      isHardwareScannerActive
                        ? "border-green-500 ring-2 ring-green-200"
                        : "border-gray-300",
                    )}
                    autoFocus
                  />

                  {/* Hardware Scanner Status Indicator */}
                  {searchType === "reference" && (
                    <div className="absolute right-4 top-1/2 transform -translate-y-1/2">
                      {isHardwareScannerActive ? (
                        <div className="flex items-center gap-2">
                          <Zap className="w-5 h-5 text-green-600 animate-pulse" />
                          <span className="text-xs font-medium text-green-700">
                            Scanning...
                          </span>
                        </div>
                      ) : (
                        <div title="Hardware barcode scanner ready">
                          <ScanLine className="w-5 h-5 text-gray-400" />
                        </div>
                      )}
                    </div>
                  )}
                </div>
                <button
                  onClick={handleSearch}
                  disabled={isSearching}
                  className="w-full px-6 py-4 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white font-semibold rounded-xl transition-colors focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 disabled:cursor-not-allowed flex items-center justify-center gap-3 shadow-lg"
                >
                  {isSearching ? (
                    <>
                      <Loader2 className="w-5 h-5 animate-spin" />
                      Searching...
                    </>
                  ) : (
                    <>
                      <Search className="w-5 h-5" />
                      Search Applications
                    </>
                  )}
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Results Section */}
      <div className="max-w-6xl mx-auto px-4 sm:px-6 lg:px-8 py-12">
        {!hasSearched ? (
          <div className="text-center py-24">
            <div className="w-24 h-24 mx-auto mb-8 bg-gradient-to-br from-blue-100 to-blue-200 rounded-full flex items-center justify-center shadow-lg">
              <Search className="w-12 h-12 text-blue-600" />
            </div>
            <h3 className="text-xl font-semibold text-gray-900 mb-3">
              Ready to Search
            </h3>
            <p className="text-gray-600 text-lg">
              Enter a reference number, application ID, or applicant name above
              to get started
            </p>
          </div>
        ) : !searchResults || searchResults.length === 0 ? (
          <div className="text-center py-24">
            <div className="w-24 h-24 mx-auto mb-8 bg-gradient-to-br from-gray-100 to-gray-200 rounded-full flex items-center justify-center shadow-lg">
              <AlertCircle className="w-12 h-12 text-gray-400" />
            </div>
            <h3 className="text-xl font-semibold text-gray-900 mb-3">
              No Results Found
            </h3>
            <p className="text-gray-600 mb-6 text-lg">
              No applications found for "{searchValue}"
            </p>
            <button
              onClick={() => {
                setSearchValue("");
                setHasSearched(false);
                setSearchResults([]);
              }}
              className="text-blue-600 hover:text-blue-700 font-medium"
            >
              Try a different search
            </button>
          </div>
        ) : (
          <div>
            {/* Results Header */}
            <div className="flex items-center justify-between mb-6">
              <div className="flex items-center gap-3">
                <h2 className="text-xl font-semibold text-gray-900">
                  Search Results
                </h2>
                <div className="flex items-center gap-2">
                  <span className="px-3 py-1 bg-blue-100 text-blue-800 text-sm font-medium rounded-full">
                    {searchResults?.length || 0} found
                  </span>
                  {searchResults && searchResults.length > 0 && (
                    <>
                      <span className="px-3 py-1 bg-green-100 text-green-800 text-sm font-medium rounded-full">
                        {searchResults.filter((r) => r.isactive === 1).length}{" "}
                        active
                      </span>
                      <span className="px-3 py-1 bg-gray-100 text-gray-800 text-sm font-medium rounded-full">
                        {searchResults.filter((r) => r.isactive === 0).length}{" "}
                        inactive
                      </span>
                    </>
                  )}
                </div>
                {searchResults?.some(
                  (r: ApplicationSearchResult) => r.isDuplicate,
                ) && (
                  <span className="px-3 py-1 bg-yellow-100 text-yellow-800 text-sm font-medium rounded-full flex items-center gap-1">
                    <AlertCircle className="w-3 h-3" />
                    Duplicates Detected
                  </span>
                )}
              </div>
            </div>

            {/* Results Grid */}
            <div className="grid gap-4">
              {searchResults?.map((result: ApplicationSearchResult) => (
                <div
                  key={result.tgapplicationid}
                  className="bg-white rounded-xl border border-gray-200 hover:border-blue-300 transition-all shadow-sm hover:shadow-lg"
                >
                  <div className="p-5">
                    <div className="flex items-center justify-between mb-4">
                      {/* Header row with reference and status */}
                      <div className="flex items-center gap-3">
                        <h3 className="text-xl font-bold text-gray-900">
                          {result.reference}
                        </h3>
                        {result.isDuplicate && (
                          <span className="px-2 py-1 bg-yellow-100 text-yellow-800 text-xs font-medium rounded-full">
                            Duplicate
                          </span>
                        )}
                        <span
                          className={cn(
                            "px-2 py-1 text-xs font-medium rounded-full",
                            result.isactive === 1
                              ? "bg-green-100 text-green-800"
                              : "bg-red-100 text-red-800",
                          )}
                        >
                          {result.isactive === 1 ? "Active" : "Inactive"}
                        </span>
                        <span
                          className={cn(
                            "px-3 py-1 text-sm font-medium rounded-full",
                            getStatusColor(result.currentapplicationstatusname),
                          )}
                        >
                          {result.currentapplicationstatusname}
                        </span>
                      </div>

                      {/* Actions */}
                      <div className="flex items-center gap-2">
                        {result.isactive === 0 && (
                          <button
                            onClick={() =>
                              handleReactivateApplication(
                                result.tgapplicationid,
                              )
                            }
                            className="px-4 py-2 bg-green-600 hover:bg-green-700 text-white text-sm font-semibold rounded-lg transition-colors focus:ring-2 focus:ring-green-500 focus:ring-offset-2 shadow-sm"
                            title="Reactivate Application"
                          >
                            Reactivate
                          </button>
                        )}
                        <button
                          onClick={() => {
                            logger.userAction("SearchPage", "clickOpen", {
                              applicationId: result.tgapplicationid,
                            });
                            handleOpenCase(result);
                          }}
                          className="px-6 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-semibold rounded-lg transition-colors focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 shadow-sm"
                        >
                          Open Application
                        </button>
                        <button
                          onClick={() => {
                            logger.userAction("SearchPage", "clickOpenNewTab", {
                              applicationId: result.tgapplicationid,
                            });
                            handleOpenCase(result, true);
                          }}
                          className="p-2 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded-lg transition-colors"
                          title="Open in new tab"
                        >
                          <ExternalLink className="w-4 h-4" />
                        </button>
                      </div>
                    </div>

                    {/* Content grid */}
                    <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-4">
                      <div>
                        <div className="text-xs font-medium text-gray-500 mb-1">
                          Application ID
                        </div>
                        <div className="font-mono text-sm font-semibold text-gray-900">
                          {result.tgapplicationid}
                        </div>
                      </div>
                      <div>
                        <div className="text-xs font-medium text-gray-500 mb-1">
                          Applicant
                        </div>
                        <div className="text-sm font-medium text-gray-900 truncate">
                          {result.personfullname}
                        </div>
                      </div>
                      <div>
                        <div className="text-xs font-medium text-gray-500 mb-1">
                          Created
                        </div>
                        <div className="text-sm text-gray-900">
                          {formatDate(result.createdate)}
                        </div>
                      </div>
                      <div>
                        <div className="text-xs font-medium text-gray-500 mb-1">
                          Modified
                        </div>
                        <div className="text-sm text-gray-900">
                          {formatDate(result.modifieddate)}
                        </div>
                      </div>
                    </div>

                    {/* Stats row */}
                    <div className="flex items-center gap-6 pt-3 border-t border-gray-100">
                      <div className="flex items-center gap-2">
                        <div className="w-3 h-3 bg-blue-500 rounded-full"></div>
                        <span className="text-sm font-medium text-gray-700">
                          {result.biometricscount} Biometrics
                        </span>
                      </div>
                      <div className="flex items-center gap-2">
                        <div className="w-3 h-3 bg-green-500 rounded-full"></div>
                        <span className="text-sm font-medium text-gray-700">
                          {result.identitiescount} Identities
                        </span>
                      </div>
                      <div className="flex items-center gap-2">
                        <div className="w-3 h-3 bg-purple-500 rounded-full"></div>
                        <span className="text-sm font-medium text-gray-700">
                          {result.workflowhistorycount} History
                        </span>
                      </div>
                    </div>
                  </div>
                </div>
              ))}
            </div>

            {/* Duplicate Reference Notice */}
            {searchResults.some(
              (r: ApplicationSearchResult) => r.isDuplicate,
            ) && (
              <div className="mt-6 p-4 bg-yellow-50 border border-yellow-200 rounded-lg">
                <div className="flex items-start gap-3">
                  <AlertCircle className="w-5 h-5 text-yellow-600 mt-0.5" />
                  <div>
                    <h4 className="text-sm font-medium text-yellow-800 mb-1">
                      Multiple Applications Found
                    </h4>
                    <p className="text-sm text-yellow-700">
                      Multiple applications share the same reference number.
                      Please select the specific application ID you want to work
                      with. This may indicate data quality issues that need
                      attention.
                    </p>
                  </div>
                </div>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Hardware Scanner Info */}
      {searchType === "reference" && !hasSearched && (
        <div className="max-w-4xl mx-auto px-4 sm:px-6 lg:px-8 pb-8">
          <div className="bg-blue-50 border border-blue-200 rounded-lg p-4">
            <div className="flex items-start gap-3">
              <ScanLine className="w-5 h-5 text-blue-600 mt-0.5" />
              <div>
                <h4 className="text-sm font-medium text-blue-800 mb-1">
                  Hardware Barcode Scanner Ready
                </h4>
                <p className="text-sm text-blue-700">
                  Simply scan any barcode with your USB or Bluetooth scanner to
                  automatically search. The scanner will be detected and the
                  search will start immediately.
                </p>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Bottom padding for scrolling */}
      <div className="pb-12"></div>
    </div>
  );
}
