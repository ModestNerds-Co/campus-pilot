// TODO: Fix TanStack Start createServerFn API usage
// import { createServerFn } from "@tanstack/start";
import { query } from "../db";
import type {
  TgLookup,
  TgWorkflowStatusConfiguration,
  TgApplicationType,
} from "../../types/database";

// Cache for lookups to reduce database calls
const lookupCache = new Map<string, { data: any; timestamp: number }>();
const CACHE_TTL = 5 * 60 * 1000; // 5 minutes

// Get all lookups by type
export const getLookupsByType = async ({
  lookupType,
}: {
  lookupType: string;
}) => {
  const cacheKey = `lookups_${lookupType}`;
  const cached = lookupCache.get(cacheKey);

  if (cached && Date.now() - cached.timestamp < CACHE_TTL) {
    return cached.data;
  }

  const result = await query<TgLookup>(
    `SELECT * FROM tglookup
       WHERE lookuptype = $1 AND isactive = true
       ORDER BY displayorder, lookupname`,
    [lookupType],
  );

  lookupCache.set(cacheKey, { data: result.rows, timestamp: Date.now() });
  return result.rows;
};

// Get single lookup by ID
export const getLookupById = async ({ lookupId }: { lookupId: number }) => {
  const result = await query<TgLookup>(
    `SELECT * FROM tglookup WHERE tglookupid = $1`,
    [lookupId],
  );
  return result.rows[0];
};

// Get all lookups (for reference panel)
export const getAllLookups = async () => {
  const cacheKey = "all_lookups";
  const cached = lookupCache.get(cacheKey);

  if (cached && Date.now() - cached.timestamp < CACHE_TTL) {
    return cached.data;
  }

  const result = await query<TgLookup>(
    `SELECT * FROM tglookup
       WHERE isactive = true
       ORDER BY lookuptype, displayorder, lookupname`,
  );

  // Group by type for easier access
  const grouped = result.rows.reduce(
    (acc, lookup) => {
      if (!acc[lookup.lookuptype]) {
        acc[lookup.lookuptype] = [];
      }
      acc[lookup.lookuptype].push(lookup);
      return acc;
    },
    {} as Record<string, TgLookup[]>,
  );

  lookupCache.set(cacheKey, { data: grouped, timestamp: Date.now() });
  return grouped;
};

// Get workflow status configurations
export const getWorkflowStatusConfigurations = async ({
  workflowId,
}: {
  workflowId?: number;
}) => {
  const cacheKey = `workflow_status_${workflowId || "all"}`;
  const cached = lookupCache.get(cacheKey);

  if (cached && Date.now() - cached.timestamp < CACHE_TTL) {
    return cached.data;
  }

  let sqlQuery = `SELECT * FROM tgworkflowstatusconfiguration WHERE isactive = true`;
  const params: any[] = [];

  if (workflowId) {
    sqlQuery += ` AND workflowid = $1`;
    params.push(workflowId);
  }

  sqlQuery += ` ORDER BY sequence, statusname`;

  const result = await query<TgWorkflowStatusConfiguration>(sqlQuery, params);

  lookupCache.set(cacheKey, { data: result.rows, timestamp: Date.now() });
  return result.rows;
};

// Get application types
export const getApplicationTypes = async () => {
  const cacheKey = "application_types";
  const cached = lookupCache.get(cacheKey);

  if (cached && Date.now() - cached.timestamp < CACHE_TTL) {
    return cached.data;
  }

  const result = await query<TgApplicationType>(
    `SELECT * FROM tgapplicationtype
       WHERE isactive = true
       ORDER BY typename`,
  );

  lookupCache.set(cacheKey, { data: result.rows, timestamp: Date.now() });
  return result.rows;
};

// Batch lookup resolution - resolve multiple lookup IDs at once
export const resolveLookups = async ({
  lookupIds,
}: {
  lookupIds: number[];
}) => {
  if (lookupIds.length === 0) return {};

  const placeholders = lookupIds.map((_, i) => `$${i + 1}`).join(",");
  const result = await query<TgLookup>(
    `SELECT tglookupid, lookuptype, lookupvalue, lookupname
       FROM tglookup
       WHERE tglookupid IN (${placeholders})`,
    lookupIds,
  );

  // Return as a map for easy access
  return result.rows.reduce(
    (acc, lookup) => {
      acc[lookup.tglookupid] = lookup;
      return acc;
    },
    {} as Record<number, TgLookup>,
  );
};

// Clear lookup cache
export const clearLookupCache = async () => {
  lookupCache.clear();
  return { success: true };
};

// Get lookup statistics (for debugging)
export const getLookupStats = async () => {
  const result = await query(
    `SELECT
        lookuptype,
        COUNT(*) as count,
        COUNT(CASE WHEN isactive = true THEN 1 END) as active_count
       FROM tglookup
       GROUP BY lookuptype
       ORDER BY lookuptype`,
  );

  return result.rows;
};
