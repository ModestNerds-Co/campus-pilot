//
//  campus-pilot
//  Relations.tsx
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { useState, useEffect } from "react";
import {
  Users,
  Plus,
  Edit2,
  XCircle,
  AlertCircle,
  Loader2,
  Heart,
  Calendar,
  UserCheck,
  ExternalLink,
  X,
} from "lucide-react";
import { formatDate } from "../../lib/utils";
import { cn } from "../../lib/utils";
import { apiClient } from "../../lib/api";
import { LookupField } from "../LookupField";
import { PersonDetails } from "./PersonDetails";
import { Identities } from "./Identities";
import { Biometrics } from "./Biometrics";
import toast from "react-hot-toast";

interface RelationsProps {
  personId: number;
}

interface RelationRecord {
  tgpersonrelationid: number;
  tgpersonid: number;
  relationtypelookupid: number;
  marriagecertificatenumber?: string;
  marriagedate?: Date;
  primarycaregiver?: number;
  relativetgpersonid?: number;
  relationstatuslookupid: number;
  portalrecordstatuslookupid?: number;
  recordstatuslookupid?: number;
  createdate: Date;
  modifieddate?: Date;
  tguserauditdetailid: number;
  createdbysystemuserid: number;
  updatedbysystemuserid?: number;
  dataownerlookupid: number;
  isactive: number;
  // Joined data from related person
  relativename?: string;
}

export function Relations({ personId }: RelationsProps) {
  const [relations, setRelations] = useState<RelationRecord[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [isAddingNew, setIsAddingNew] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [viewingRelatedPerson, setViewingRelatedPerson] = useState<{
    personId: number;
    name: string;
  } | null>(null);
  const [activeModalTab, setActiveModalTab] = useState<
    "person" | "identities" | "biometrics"
  >("person");

  // Fetch relations data
  useEffect(() => {
    const fetchRelations = async () => {
      try {
        setIsLoading(true);
        setError(null);

        const data = await apiClient.getRelations(personId);
        // Convert date strings to Date objects
        const processedData = data.map((rel: any) => ({
          ...rel,
          createdate: new Date(rel.createdate),
          modifieddate: rel.modifieddate
            ? new Date(rel.modifieddate)
            : undefined,
          marriagedate: rel.marriagedate
            ? new Date(rel.marriagedate)
            : undefined,
        }));

        // Deduplicate relations - only show one entry per relationship pair
        // Store all record IDs for each pair so we can void both when needed
        const relationMap = new Map<string, any>();

        processedData.forEach((rel: any) => {
          if (!rel.relativetgpersonid) return;

          // Create a key for the relationship pair (always order smaller ID first)
          const personA = Math.min(personId, rel.relativetgpersonid);
          const personB = Math.max(personId, rel.relativetgpersonid);
          const key = `${personA}-${personB}`;

          // If we already have this relationship pair, combine the record IDs
          if (relationMap.has(key)) {
            const existing = relationMap.get(key);

            // Add this record ID to the list of related record IDs
            if (!existing.allRecordIds) {
              existing.allRecordIds = [existing.tgpersonrelationid];
            }
            existing.allRecordIds.push(rel.tgpersonrelationid);

            // Always prefer the relation where we're looking at the "other" person
            // For guardian-child relationship, prefer showing the guardian
            if (
              rel.tgpersonid !== personId &&
              existing.tgpersonid === personId
            ) {
              existing.tgpersonrelationid = rel.tgpersonrelationid;
              existing.tgpersonid = rel.tgpersonid;
              existing.relativetgpersonid = rel.relativetgpersonid;
              existing.relationshiptype = rel.relationshiptype;
              existing.status = rel.status;
              existing.personname = rel.personname;
              existing.relativename = rel.relativename;
            }
          } else {
            // First occurrence, initialize with this record ID
            rel.allRecordIds = [rel.tgpersonrelationid];
            relationMap.set(key, rel);
          }
        });

        // Convert back to array and sort
        const deduplicatedRelations = Array.from(relationMap.values()).sort(
          (a, b) =>
            new Date(b.createdate).getTime() - new Date(a.createdate).getTime(),
        );
        setRelations(deduplicatedRelations);
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to load relations data";
        setError(errorMessage);
        toast.error(errorMessage);
      } finally {
        setIsLoading(false);
      }
    };

    fetchRelations();
  }, [personId]);

  const handleVoid = async (relation: any) => {
    const confirmed = window.confirm(
      "Are you sure you want to void this relation record?",
    );
    if (!confirmed) return;

    try {
      // Void all records associated with this relationship pair
      const recordIds = relation.allRecordIds || [relation.tgpersonrelationid];

      for (const recordId of recordIds) {
        await apiClient.voidRelation(personId, recordId);
      }

      setRelations((prev) =>
        prev.map((rel) =>
          rel.tgpersonrelationid === relation.tgpersonrelationid
            ? { ...rel, isactive: 0, modifieddate: new Date() }
            : rel,
        ),
      );
      toast.success("Relation record voided successfully");
    } catch (error) {
      toast.error("Failed to void relation record");
    }
  };

  const handleViewRelatedPerson = (relation: RelationRecord) => {
    if (
      relation.relativetgpersonid &&
      relation.relativetgpersonid !== personId
    ) {
      setViewingRelatedPerson({
        personId: relation.relativetgpersonid,
        name: relation.relativename || "Unknown Person",
      });
      setActiveModalTab("person");
    }
  };

  const handleCloseModal = () => {
    setViewingRelatedPerson(null);
    setActiveModalTab("person");
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="flex items-center gap-3">
          <Loader2 className="w-6 h-6 animate-spin text-blue-600" />
          <span className="text-gray-600">Loading relations data...</span>
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
            Failed to Load Relations
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
            <Users className="w-4 h-4" />
            Relations ({relations.length})
          </h2>
        </div>
      </div>

      {/* Relations List */}
      {relations.length === 0 ? (
        <div className="text-center py-12 bg-gradient-to-br from-blue-50 to-indigo-50 rounded-lg border-2 border-dashed border-blue-200">
          <Users className="w-16 h-16 text-blue-300 mx-auto mb-4" />
          <h3 className="text-lg font-semibold text-gray-900 mb-2">
            👥 No Family Relations Found
          </h3>
          <p className="text-gray-600 mb-4 max-w-md mx-auto">
            This person has no recorded family relationships such as spouse,
            children, parents, or other relatives in the system.
          </p>
          <div className="text-sm text-blue-700 bg-blue-100 px-4 py-2 rounded-lg inline-flex items-center gap-2">
            <span className="text-blue-500">💡</span>
            <span>Family relations would appear here when available</span>
          </div>
        </div>
      ) : (
        <div className="space-y-3">
          {relations.map((relation) => (
            <div
              key={relation.tgpersonrelationid}
              className="bg-white rounded-lg border border-gray-200 p-4"
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-4">
                  <div className="w-12 h-12 bg-gray-100 rounded-lg flex items-center justify-center">
                    <Users className="w-6 h-6 text-gray-600" />
                  </div>
                  <div>
                    <div className="flex items-center gap-2">
                      <LookupField
                        lookupId={relation.relationtypelookupid}
                        format="value"
                        className="font-medium text-gray-900"
                        fallback="Unknown Relation Type"
                      />
                      {relation.relativename && (
                        <>
                          <span className="text-gray-400">•</span>
                          <span className="font-medium text-blue-600">
                            {relation.relativename}
                          </span>
                        </>
                      )}
                      <span className="text-gray-400">•</span>
                      <LookupField
                        lookupId={relation.relationstatuslookupid}
                        format="value"
                        className="text-sm text-gray-600"
                        fallback="Unknown Status"
                      />
                    </div>
                    <div className="flex items-center gap-4 mt-1 text-sm text-gray-500">
                      <span>ID: {relation.tgpersonrelationid}</span>
                      {relation.relativetgpersonid && (
                        <span>
                          Related Person ID: {relation.relativetgpersonid}
                        </span>
                      )}
                      {relation.primarycaregiver === 1 && (
                        <span className="px-2 py-0.5 rounded-full font-medium text-xs text-blue-700 bg-blue-100">
                          Primary Caregiver
                        </span>
                      )}
                      {relation.marriagecertificatenumber && (
                        <span>
                          <Heart className="w-3 h-3 inline mr-1" />
                          Cert: {relation.marriagecertificatenumber}
                        </span>
                      )}
                      {relation.marriagedate && (
                        <span>
                          <Calendar className="w-3 h-3 inline mr-1" />
                          Married: {formatDate(relation.marriagedate)}
                        </span>
                      )}
                      <span
                        className={cn(
                          "badge text-[9px]",
                          relation.isactive ? "badge-success" : "badge-neutral",
                        )}
                      >
                        {relation.isactive ? "Active" : "Inactive"}
                      </span>
                    </div>
                    <div className="flex items-center gap-3 text-[10px] text-muted-foreground mt-1">
                      <span>Created: {formatDate(relation.createdate)}</span>
                      {relation.modifieddate && (
                        <>
                          <span>•</span>
                          <span>
                            Modified: {formatDate(relation.modifieddate)}
                          </span>
                        </>
                      )}
                    </div>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  {relation.relativetgpersonid &&
                    relation.relativetgpersonid !== personId && (
                      <button
                        onClick={() => handleViewRelatedPerson(relation)}
                        className="p-2 text-gray-500 hover:text-blue-600 hover:bg-blue-50 rounded-lg transition-colors"
                        title="View Related Person Details"
                      >
                        <ExternalLink className="w-4 h-4" />
                      </button>
                    )}
                  <button
                    onClick={() => handleVoid(relation)}
                    className="p-2 text-gray-500 hover:text-red-600 hover:bg-red-50 rounded-lg transition-colors"
                    title="Void Record"
                  >
                    <XCircle className="w-4 h-4" />
                  </button>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Add New Form Placeholder */}
      {isAddingNew && (
        <div className="bg-card border rounded-lg p-4 space-y-4">
          <h3 className="text-sm font-semibold">Add New Relation Record</h3>
          <p className="text-xs text-muted-foreground">
            Relation creation form would go here
          </p>
          <div className="flex justify-end gap-2">
            <button
              onClick={() => setIsAddingNew(false)}
              className="compact-button border"
              disabled={isSubmitting}
            >
              Cancel
            </button>
            <button
              className="compact-button bg-primary text-white"
              disabled={isSubmitting}
            >
              {isSubmitting ? (
                <>
                  <Loader2 className="w-3 h-3 mr-1 animate-spin" />
                  Adding...
                </>
              ) : (
                "Add Relation"
              )}
            </button>
          </div>
        </div>
      )}

      {/* Related Person Details Slide-over Modal */}
      {viewingRelatedPerson && (
        <div
          className="fixed inset-0 bg-black bg-opacity-50 flex justify-end z-50"
          onClick={handleCloseModal}
        >
          <div
            className="bg-white w-3/4 h-full flex flex-col shadow-xl"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Header */}
            <div className="px-6 py-4 border-b border-gray-200 flex items-center justify-between">
              <div>
                <h2 className="text-lg font-semibold text-gray-900">
                  {viewingRelatedPerson.name}
                </h2>
                <p className="text-sm text-gray-500">
                  Person ID: {viewingRelatedPerson.personId}
                </p>
              </div>
              <button
                onClick={handleCloseModal}
                className="p-2 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded-lg transition-colors"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            {/* Tab Navigation */}
            <div className="px-6 py-3 border-b border-gray-200">
              <div className="flex space-x-8">
                <button
                  onClick={() => setActiveModalTab("person")}
                  className={cn(
                    "py-2 px-1 border-b-2 font-medium text-sm transition-colors",
                    activeModalTab === "person"
                      ? "border-blue-500 text-blue-600"
                      : "border-transparent text-gray-500 hover:text-gray-700",
                  )}
                >
                  Person Details
                </button>
                <button
                  onClick={() => setActiveModalTab("identities")}
                  className={cn(
                    "py-2 px-1 border-b-2 font-medium text-sm transition-colors",
                    activeModalTab === "identities"
                      ? "border-blue-500 text-blue-600"
                      : "border-transparent text-gray-500 hover:text-gray-700",
                  )}
                >
                  Identities
                </button>
                <button
                  onClick={() => setActiveModalTab("biometrics")}
                  className={cn(
                    "py-2 px-1 border-b-2 font-medium text-sm transition-colors",
                    activeModalTab === "biometrics"
                      ? "border-blue-500 text-blue-600"
                      : "border-transparent text-gray-500 hover:text-gray-700",
                  )}
                >
                  Biometrics
                </button>
              </div>
            </div>

            {/* Tab Content */}
            <div className="flex-1 overflow-y-auto p-6">
              {activeModalTab === "person" && (
                <PersonDetails personId={viewingRelatedPerson.personId} />
              )}
              {activeModalTab === "identities" && (
                <Identities personId={viewingRelatedPerson.personId} />
              )}
              {activeModalTab === "biometrics" && (
                <Biometrics personId={viewingRelatedPerson.personId} />
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
