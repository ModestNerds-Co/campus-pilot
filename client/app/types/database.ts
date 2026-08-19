// Database entity types for the immigration system

export interface TgApplication {
  tgapplicationid: number;
  reference: string;
  currentapplicationstatusname: string;
  applicationstatuslookupid: number;
  tgworkflowstatusconfigurationid?: number;
  entityid: number; // Links to tgperson.tgpersonid
  tgapplicationtypeid?: number;
  applicationstartdate?: Date;
  prioritylookupid?: number;
  documenttypelookupid?: number;
  guardiantgpersonid?: number;
  remark?: string;
  assignedsystemorganisationid?: number;
  assignedsystemuserid?: number;
  createdate: Date;
  modifieddate: Date;
  isactive: boolean;
}

export interface TgPerson {
  tgpersonid: number;
  firstname: string;
  middlename?: string;
  surname: string;
  birthdate?: Date;
  genderlookupid?: number;
  nationalitylookupid?: number;
  contactnumber?: string;
  emailaddress?: string;
  maincity?: string;
  mainaddressline1?: string;
  title?: string;
  maritalstatuslookupid?: number;
  watchliststatuslookupid?: number;
  alternatecontactnumber?: string;
  alternateemail?: string;
  createdate: Date;
  modifieddate: Date;
  isactive: boolean;
}

export interface TgPersonBiometric {
  tgpersonbiometricid: number;
  tgpersonid: number;
  modalitylookupid: number;
  positionlookupid?: number;
  imagetypelookupid?: number;
  deviceid?: string;
  quality?: number;
  issuccessful: boolean;
  image?: Buffer;
  innovatricstemplate?: Buffer;
  isotemplate?: Buffer;
  remark?: string;
  createdate: Date;
  modifieddate: Date;
  isactive: boolean;
}

export interface TgPersonIdentity {
  tgpersonidentityid: number;
  tgpersonid: number;
  identitytypelookupid: number;
  identitynumber: string;
  issuedate: Date;
  expirydate?: Date;
  countryofissuelookupid?: number;
  identitydocumentstatuslookupid?: number;
  createdate: Date;
  modifieddate: Date;
  isactive: boolean;
}

export interface TgApplicationWorkflowHistory {
  tgapplicationworkflowhistoryid: number;
  tgapplicationid: number;
  actiondate: Date;
  actionname: string;
  currentapplicationstatusname: string;
  applicationstatuslookupvalue?: string;
  applicationstatuslookupid?: number;
  tgworkflowstatusconfigurationid?: number;
  remark?: string;
  createdbysystemuserid?: number;
  createdate: Date;
  modifieddate: Date;
  isactive: boolean;
}

export interface TgLookup {
  tglookupid: number;
  lookuptype: string;
  lookupvalue: string;
  lookupname: string;
  displayorder?: number;
  parentlookupid?: number;
  createdate: Date;
  modifieddate: Date;
  isactive: boolean;
}

export interface TgWorkflowStatusConfiguration {
  tgworkflowstatusconfigurationid: number;
  statusname: string;
  statuscode: string;
  workflowid: number;
  sequence?: number;
  createdate: Date;
  modifieddate: Date;
  isactive: boolean;
}

export interface TgWorkflowConfiguration {
  tgworkflowconfigurationid: number;
  workflowname: string;
  fromstatusid: number;
  tostatusid: number;
  actionname: string;
  requiresapproval?: boolean;
  createdate: Date;
  modifieddate: Date;
  isactive: boolean;
}

export interface TgApplicationType {
  tgapplicationtypeid: number;
  typename: string;
  typecode: string;
  configuration?: any; // JSON field
  createdate: Date;
  modifieddate: Date;
  isactive: boolean;
}

// Audit log type
export interface AuditLog {
  auditlogid: number;
  actor: string;
  tgapplicationid: number;
  reference: string;
  action: "CREATE" | "UPDATE" | "DELETE" | "TRANSITION";
  section: "APPLICATION" | "PERSON" | "BIOMETRIC" | "IDENTITY" | "WORKFLOW";
  changes: {
    field: string;
    oldValue: any;
    newValue: any;
  }[];
  transition?: {
    from: string;
    to: string;
  };
  outcome: "SUCCESS" | "ERROR";
  errortext?: string;
  createdate: Date;
}

// Search types
export interface ApplicationSearchParams {
  searchType: "reference" | "applicationId";
  searchValue: string;
}

export interface ApplicationSearchResult {
  tgapplicationid: number;
  reference: string;
  currentapplicationstatusname: string;
  applicationstatuslookupid: number;
  createdate: Date;
  modifieddate: Date;
  personfullname: string;
  tgpersonid: number;
  biometricsCount: number;
  identitiesCount: number;
  workflowHistoryCount: number;
  isDuplicate?: boolean;
}

// State management types
export interface StagedChanges {
  application?: Partial<TgApplication>;
  person?: Partial<TgPerson>;
  biometrics?: {
    updates: Map<number, Partial<TgPersonBiometric>>;
    creates: Partial<TgPersonBiometric>[];
    deletes: number[];
  };
  identities?: {
    updates: Map<number, Partial<TgPersonIdentity>>;
    creates: Partial<TgPersonIdentity>[];
    deletes: number[];
  };
  workflowHistory?: Partial<TgApplicationWorkflowHistory>;
}

export interface ValidationError {
  field: string;
  message: string;
  severity: "error" | "warning";
}

export interface DryRunResult {
  section: string;
  changes: {
    field: string;
    oldValue: any;
    newValue: any;
  }[];
  validationErrors: ValidationError[];
  warnings: string[];
}

export interface SaveResult {
  success: boolean;
  savedSections: string[];
  failedSection?: string;
  error?: string;
  auditLogId?: number;
}

// Connection status
export interface ConnectionStatus {
  status: "connecting" | "connected" | "error" | "reconnecting";
  error?: string;
  lastConnected?: Date;
}

// Tab state
export interface TabState {
  id: string;
  tgapplicationid: number;
  reference: string;
  isDirty: boolean;
  stagedChanges: StagedChanges;
  loadedAt: Date;
  modifiedDates: {
    application?: Date;
    person?: Date;
    biometrics?: Map<number, Date>;
    identities?: Map<number, Date>;
  };
}

// UI State
export interface UIState {
  activeTabId: string | null;
  tabs: TabState[];
  rightDrawerOpen: boolean;
  theme: "light" | "dark";
  connectionStatus: ConnectionStatus;
}
