// TODO: Fix TanStack Start createServerFn API usage
// import { createServerFn } from "@tanstack/start";
import { query, withTransaction } from "../db";
import type {
  TgApplication,
  TgPerson,
  TgPersonBiometric,
  TgPersonIdentity,
  TgApplicationWorkflowHistory,
  ApplicationSearchParams,
  ApplicationSearchResult,
  StagedChanges,
  SaveResult,
  DryRunResult,
  ValidationError,
} from "../../types/database";

// Search for applications
export const searchApplications = async ({
  searchType,
  searchValue,
}: ApplicationSearchParams) => {
  console.log({ searchType, searchValue });
  let sqlQuery: string;
  let params: any[];

  if (searchType === "reference") {
    sqlQuery = `
        SELECT
          a.tgapplicationid,
          a.reference,
          a.createdate,
          a.modifieddate,
          a.currentapplicationstatusname,
          a.applicationstatuslookupid,
          p.firstname || ' ' || COALESCE(p.middlename, '') || ' ' || p.surname as personfullname,
          p.tgpersonid,
          (SELECT COUNT(*) FROM tgpersonbiometric WHERE tgpersonid = p.tgpersonid AND isactive = true) as biometricscount,
          (SELECT COUNT(*) FROM tgpersonidentity WHERE tgpersonid = p.tgpersonid AND isactive = true) as identitiescount,
          (SELECT COUNT(*) FROM tgapplicationworkflowhistory WHERE tgapplicationid = a.tgapplicationid) as workflowhistorycount
        FROM tgapplication a
        JOIN tgperson p ON a.entityid = p.tgpersonid
        WHERE a.reference = $1
        ORDER BY a.createdate DESC
      `;
    params = [searchValue];
  } else {
    sqlQuery = `
        SELECT
          a.tgapplicationid,
          a.reference,
          a.createdate,
          a.modifieddate,
          a.currentapplicationstatusname,
          a.applicationstatuslookupid,
          p.firstname || ' ' || COALESCE(p.middlename, '') || ' ' || p.surname as personfullname,
          p.tgpersonid,
          (SELECT COUNT(*) FROM tgpersonbiometric WHERE tgpersonid = p.tgpersonid AND isactive = true) as biometricscount,
          (SELECT COUNT(*) FROM tgpersonidentity WHERE tgpersonid = p.tgpersonid AND isactive = true) as identitiescount,
          (SELECT COUNT(*) FROM tgapplicationworkflowhistory WHERE tgapplicationid = a.tgapplicationid) as workflowhistorycount
        FROM tgapplication a
        JOIN tgperson p ON a.entityid = p.tgpersonid
        WHERE a.tgapplicationid = $1
        ORDER BY a.createdate DESC
      `;
    params = [parseInt(searchValue)];
  }

  const result = await query<ApplicationSearchResult>(sqlQuery, params);

  // Mark duplicates
  const referenceCount = new Map<string, number>();
  result.rows.forEach((row) => {
    const count = referenceCount.get(row.reference) || 0;
    referenceCount.set(row.reference, count + 1);
  });

  return result.rows.map((row) => ({
    ...row,
    isDuplicate: (referenceCount.get(row.reference) || 0) > 1,
  }));
};

// Get application details
export const getApplication = async ({
  tgapplicationid,
}: {
  tgapplicationid: number;
}) => {
  const result = await query<TgApplication>(
    `SELECT * FROM tgapplication WHERE tgapplicationid = $1`,
    [tgapplicationid],
  );
  return result.rows[0];
};

// Get person details
export const getPerson = async ({ tgpersonid }: { tgpersonid: number }) => {
  const result = await query<TgPerson>(
    `SELECT * FROM tgperson WHERE tgpersonid = $1`,
    [tgpersonid],
  );
  return result.rows[0];
};

// Get biometrics
export const getBiometrics = async ({ tgpersonid }: { tgpersonid: number }) => {
  const result = await query<TgPersonBiometric>(
    `SELECT
        tgpersonbiometricid,
        tgpersonid,
        modalitylookupid,
        positionlookupid,
        imagetypelookupid,
        deviceid,
        quality,
        template,
        image,
        imageformat,
        serialnumber,
        remark,
        tguserauditdetailid,
        createdate,
        modifieddate,
        createdbysystemuserid,
        updatedbysystemuserid,
        dataownerlookupid,
        isactive
      FROM tgpersonbiometric
      WHERE tgpersonid = $1
      ORDER BY createdate DESC`,
    [tgpersonid],
  );
  return result.rows;
};

// Get identities
export const getIdentities = async ({ tgpersonid }: { tgpersonid: number }) => {
  const result = await query<TgPersonIdentity>(
    `SELECT * FROM tgpersonidentity
       WHERE tgpersonid = $1
       ORDER BY createdate DESC`,
    [tgpersonid],
  );
  return result.rows;
};

// Get workflow history
export const getWorkflowHistory = async ({
  tgapplicationid,
}: {
  tgapplicationid: number;
}) => {
  const result = await query<TgApplicationWorkflowHistory>(
    `SELECT * FROM tgapplicationworkflowhistory
       WHERE tgapplicationid = $1
       ORDER BY createdate DESC`,
    [tgapplicationid],
  );
  return result.rows;
};

// Get valid next workflow statuses
export const getValidNextStatuses = async ({
  currentStatusId,
}: {
  currentStatusId: number;
}) => {
  const result = await query(
    `SELECT
        wc.tgworkflowconfigurationid,
        wc.tostatusid,
        wc.actionname,
        ws.statusname,
        ws.statuscode
      FROM tgworkflowconfiguration wc
      JOIN tgworkflowstatusconfiguration ws ON wc.tostatusid = ws.tgworkflowstatusconfigurationid
      WHERE wc.fromstatusid = $1 AND wc.isactive = true AND ws.isactive = true
      ORDER BY ws.sequence`,
    [currentStatusId],
  );
  return result.rows;
};

// Perform dry run validation
export const performDryRun = async ({
  tgapplicationid,
  changes,
}: {
  tgapplicationid: number;
  changes: StagedChanges;
}) => {
  const results: DryRunResult[] = [];

  // Basic validation placeholder
  if (changes.application) {
    results.push({
      section: "Application",
      changes: [],
      validationErrors: [],
      warnings: [],
    });
  }

  if (changes.person) {
    results.push({
      section: "Person",
      changes: [],
      validationErrors: [],
      warnings: [],
    });
  }

  return results;
};

// Save all changes in a transaction
export const saveChanges = async ({
  tgapplicationid,
  changes,
  actor,
}: {
  tgapplicationid: number;
  changes: StagedChanges;
  actor: string;
}): Promise<SaveResult> => {
  try {
    // TODO: Implement actual save logic
    return {
      success: true,
      savedSections: ["placeholder"],
    };
  } catch (error) {
    console.error("Save failed:", error);
    return {
      success: false,
      savedSections: [],
      error: error instanceof Error ? error.message : "Unknown error occurred",
    };
  }
};

// Check for conflicts
export const checkConflicts = async ({
  tgapplicationid,
  lastModifiedDates,
}: {
  tgapplicationid: number;
  lastModifiedDates: {
    application?: Date;
    person?: Date;
  };
}) => {
  // TODO: Implement actual conflict checking
  return { hasConflicts: false, conflicts: [] };
};
