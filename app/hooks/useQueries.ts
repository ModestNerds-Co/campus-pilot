//
//  campus-pilot
//  useQueries.ts
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "react-hot-toast";
import {
  searchApplications,
  getApplication,
  getPerson,
  getBiometrics,
  getIdentities,
  getWorkflowHistory,
  getValidNextStatuses,
  performDryRun,
  saveChanges,
  checkConflicts,
} from "../server/functions/applications";
import {
  getLookupsByType,
  getLookupById,
  getAllLookups,
  getWorkflowStatusConfigurations,
  getApplicationTypes,
  resolveLookups,
} from "../server/functions/lookups";
import {
  testDatabaseConnection,
  getDatabaseConnectionStatus,
} from "../server/functions/connection";
import type {
  ApplicationSearchParams,
  StagedChanges,
  TgApplication,
  TgPerson,
  TgPersonBiometric,
  TgPersonIdentity,
  TgApplicationWorkflowHistory,
} from "../types/database";

// Query keys factory
export const queryKeys = {
  connection: ["connection"] as const,
  connectionStatus: ["connection", "status"] as const,
  search: (params: ApplicationSearchParams) => ["search", params] as const,
  application: (id: number) => ["application", id] as const,
  person: (id: number) => ["person", id] as const,
  biometrics: (personId: number) => ["biometrics", personId] as const,
  identities: (personId: number) => ["identities", personId] as const,
  workflowHistory: (appId: number) => ["workflow-history", appId] as const,
  validNextStatuses: (statusId: number) =>
    ["valid-next-statuses", statusId] as const,
  lookups: {
    all: ["lookups"] as const,
    byType: (type: string) => ["lookups", type] as const,
    byId: (id: number) => ["lookups", "id", id] as const,
  },
  workflowStatusConfigs: (workflowId?: number) =>
    workflowId
      ? ["workflow-status-configs", workflowId]
      : (["workflow-status-configs"] as const),
  applicationTypes: ["application-types"] as const,
};

// Connection hooks
export function useConnectionTest() {
  return useQuery({
    queryKey: queryKeys.connection,
    queryFn: () => testDatabaseConnection(),
    retry: 3,
    retryDelay: (attemptIndex) => Math.min(1000 * 2 ** attemptIndex, 30000),
    refetchInterval: false,
  });
}

export function useConnectionStatus() {
  return useQuery({
    queryKey: queryKeys.connectionStatus,
    queryFn: () => getDatabaseConnectionStatus(),
    refetchInterval: 5000, // Check every 5 seconds
  });
}

// Search hook
export function useApplicationSearch(params: ApplicationSearchParams | null) {
  return useQuery({
    queryKey: params ? queryKeys.search(params) : ["search", null],
    queryFn: params ? () => searchApplications(params) : undefined,
    enabled: !!params,
    staleTime: 30000, // Consider data stale after 30 seconds
  });
}

// Application data hooks
export function useApplication(tgapplicationid: number | null) {
  return useQuery({
    queryKey: tgapplicationid
      ? queryKeys.application(tgapplicationid)
      : ["application", null],
    queryFn: tgapplicationid
      ? () => getApplication({ tgapplicationid })
      : undefined,
    enabled: !!tgapplicationid,
    staleTime: 60000,
  });
}

export function usePerson(tgpersonid: number | null) {
  return useQuery({
    queryKey: tgpersonid ? queryKeys.person(tgpersonid) : ["person", null],
    queryFn: tgpersonid ? () => getPerson({ tgpersonid }) : undefined,
    enabled: !!tgpersonid,
    staleTime: 60000,
  });
}

export function useBiometrics(tgpersonid: number | null) {
  return useQuery({
    queryKey: tgpersonid
      ? queryKeys.biometrics(tgpersonid)
      : ["biometrics", null],
    queryFn: tgpersonid ? () => getBiometrics({ tgpersonid }) : undefined,
    enabled: !!tgpersonid,
    staleTime: 60000,
  });
}

export function useIdentities(tgpersonid: number | null) {
  return useQuery({
    queryKey: tgpersonid
      ? queryKeys.identities(tgpersonid)
      : ["identities", null],
    queryFn: tgpersonid ? () => getIdentities({ tgpersonid }) : undefined,
    enabled: !!tgpersonid,
    staleTime: 60000,
  });
}

export function useWorkflowHistory(tgapplicationid: number | null) {
  return useQuery({
    queryKey: tgapplicationid
      ? queryKeys.workflowHistory(tgapplicationid)
      : ["workflow-history", null],
    queryFn: tgapplicationid
      ? () => getWorkflowHistory({ tgapplicationid })
      : undefined,
    enabled: !!tgapplicationid,
    staleTime: 60000,
  });
}

export function useValidNextStatuses(currentStatusId: number | null) {
  return useQuery({
    queryKey: currentStatusId
      ? queryKeys.validNextStatuses(currentStatusId)
      : ["valid-next-statuses", null],
    queryFn: currentStatusId
      ? () => getValidNextStatuses({ currentStatusId })
      : undefined,
    enabled: !!currentStatusId,
    staleTime: 300000, // 5 minutes
  });
}

// Lookup hooks
export function useLookups() {
  return useQuery({
    queryKey: queryKeys.lookups.all,
    queryFn: () => getAllLookups(),
    staleTime: 300000, // 5 minutes
    gcTime: 600000, // Keep in cache for 10 minutes
  });
}

export function useLookupsByType(lookupType: string | null) {
  return useQuery({
    queryKey: lookupType
      ? queryKeys.lookups.byType(lookupType)
      : ["lookups", null],
    queryFn: lookupType ? () => getLookupsByType({ lookupType }) : undefined,
    enabled: !!lookupType,
    staleTime: 300000,
  });
}

export function useLookupById(lookupId: number | null) {
  return useQuery({
    queryKey: lookupId
      ? queryKeys.lookups.byId(lookupId)
      : ["lookups", "id", null],
    queryFn: lookupId ? () => getLookupById({ lookupId }) : undefined,
    enabled: !!lookupId,
    staleTime: 300000,
  });
}

export function useWorkflowStatusConfigurations(workflowId?: number) {
  return useQuery({
    queryKey: queryKeys.workflowStatusConfigs(workflowId),
    queryFn: () => getWorkflowStatusConfigurations({ workflowId }),
    staleTime: 300000,
  });
}

export function useApplicationTypes() {
  return useQuery({
    queryKey: queryKeys.applicationTypes,
    queryFn: () => getApplicationTypes(),
    staleTime: 300000,
  });
}

// Batch lookup resolution
export function useBatchLookups(lookupIds: number[]) {
  return useQuery({
    queryKey: ["lookups", "batch", lookupIds.sort().join(",")],
    queryFn: () => resolveLookups({ lookupIds }),
    enabled: lookupIds.length > 0,
    staleTime: 300000,
  });
}

// Mutation hooks
export function useDryRun() {
  return useMutation({
    mutationFn: ({
      tgapplicationid,
      changes,
    }: {
      tgapplicationid: number;
      changes: StagedChanges;
    }) => performDryRun({ tgapplicationid, changes }),
    onError: (error) => {
      toast.error(`Dry run failed: ${error.message}`);
    },
  });
}

export function useSaveChanges() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({
      tgapplicationid,
      changes,
      actor,
    }: {
      tgapplicationid: number;
      changes: StagedChanges;
      actor: string;
    }) => saveChanges({ tgapplicationid, changes, actor }),
    onSuccess: (data: any, variables) => {
      if (data && data.success) {
        toast.success("Changes saved successfully");

        // Invalidate relevant queries
        queryClient.invalidateQueries({
          queryKey: queryKeys.application(variables.tgapplicationid),
        });

        // Also invalidate related data if changed
        if (variables.changes.person) {
          const personId = queryClient.getQueryData<TgApplication>(
            queryKeys.application(variables.tgapplicationid),
          )?.entityid;
          if (personId) {
            queryClient.invalidateQueries({
              queryKey: queryKeys.person(personId),
            });
          }
        }

        if (variables.changes.biometrics) {
          const personId = queryClient.getQueryData<TgApplication>(
            queryKeys.application(variables.tgapplicationid),
          )?.entityid;
          if (personId) {
            queryClient.invalidateQueries({
              queryKey: queryKeys.biometrics(personId),
            });
          }
        }

        if (variables.changes.identities) {
          const personId = queryClient.getQueryData<TgApplication>(
            queryKeys.application(variables.tgapplicationid),
          )?.entityid;
          if (personId) {
            queryClient.invalidateQueries({
              queryKey: queryKeys.identities(personId),
            });
          }
        }

        if (variables.changes.workflowHistory) {
          queryClient.invalidateQueries({
            queryKey: queryKeys.workflowHistory(variables.tgapplicationid),
          });
        }
      } else {
        toast.error(`Save failed: ${(data as any)?.error || "Unknown error"}`);
      }
    },
    onError: (error) => {
      toast.error(`Save failed: ${error.message}`);
    },
  });
}

export function useCheckConflicts() {
  return useMutation({
    mutationFn: ({
      tgapplicationid,
      lastModifiedDates,
    }: {
      tgapplicationid: number;
      lastModifiedDates: {
        application?: Date;
        person?: Date;
      };
    }) => checkConflicts({ tgapplicationid, lastModifiedDates }),
  });
}

// Prefetch functions for optimization
export function usePrefetchApplicationData() {
  const queryClient = useQueryClient();

  return async (tgapplicationid: number) => {
    // Prefetch application
    const app = await queryClient.fetchQuery({
      queryKey: queryKeys.application(tgapplicationid),
      queryFn: () => getApplication({ tgapplicationid }),
      staleTime: 60000,
    });

    if (app?.entityid) {
      // Prefetch person and related data in parallel
      await Promise.all([
        queryClient.prefetchQuery({
          queryKey: queryKeys.person(app.entityid),
          queryFn: () => getPerson({ tgpersonid: app.entityid }),
          staleTime: 60000,
        }),
        queryClient.prefetchQuery({
          queryKey: queryKeys.biometrics(app.entityid),
          queryFn: () => getBiometrics({ tgpersonid: app.entityid }),
          staleTime: 60000,
        }),
        queryClient.prefetchQuery({
          queryKey: queryKeys.identities(app.entityid),
          queryFn: () => getIdentities({ tgpersonid: app.entityid }),
          staleTime: 60000,
        }),
      ]);
    }

    // Prefetch workflow history
    await queryClient.prefetchQuery({
      queryKey: queryKeys.workflowHistory(tgapplicationid),
      queryFn: () => getWorkflowHistory({ tgapplicationid }),
      staleTime: 60000,
    });
  };
}

// Hook to refresh all data for an application
export function useRefreshApplicationData() {
  const queryClient = useQueryClient();

  return async (tgapplicationid: number) => {
    const app = queryClient.getQueryData<TgApplication>(
      queryKeys.application(tgapplicationid),
    );

    const invalidatePromises = [
      queryClient.invalidateQueries({
        queryKey: queryKeys.application(tgapplicationid),
      }),
      queryClient.invalidateQueries({
        queryKey: queryKeys.workflowHistory(tgapplicationid),
      }),
    ];

    if (app?.entityid) {
      invalidatePromises.push(
        queryClient.invalidateQueries({
          queryKey: queryKeys.person(app.entityid),
        }),
        queryClient.invalidateQueries({
          queryKey: queryKeys.biometrics(app.entityid),
        }),
        queryClient.invalidateQueries({
          queryKey: queryKeys.identities(app.entityid),
        }),
      );
    }

    await Promise.all(invalidatePromises);
    toast.success("Data refreshed");
  };
}
