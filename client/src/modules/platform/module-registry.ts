import {
  Archive,
  BookOpen,
  Boxes,
  BriefcaseBusiness,
  Building2,
  CalendarClock,
  ClipboardCheck,
  GraduationCap,
  HeartPulse,
  Landmark,
  Library,
  MessageSquareText,
  PackageSearch,
  ReceiptText,
  Settings2,
  ShieldCheck,
  Truck,
} from "lucide-react";

import type { ModuleVisual } from "./types";

export const moduleVisuals: Record<string, ModuleVisual> = {
  administration: {
    icon: Settings2,
    highlights: ["Users and access", "Module licensing", "School configuration"],
  },
  sis: {
    icon: GraduationCap,
    highlights: ["Applications and admissions", "Learner records", "Guardians and enrolment"],
  },
  academics: {
    icon: BookOpen,
    highlights: ["Subjects and classes", "Assessment structures", "Progression and reporting"],
  },
  timetabling: {
    icon: CalendarClock,
    highlights: ["Teaching constraints", "Conflict-aware generation", "Publishing and changes"],
  },
  messaging: {
    icon: MessageSquareText,
    highlights: ["Announcements", "Targeted communication", "Delivery history"],
  },
  finance: {
    icon: Landmark,
    highlights: ["General ledger", "Budgets and controls", "Financial reporting"],
  },
  fees: {
    icon: ReceiptText,
    highlights: ["Fee structures", "Billing and receipts", "Account balances"],
  },
  library: {
    icon: Library,
    highlights: ["Catalogue", "Circulation", "Reservations and resources"],
  },
  hr_payroll: {
    icon: BriefcaseBusiness,
    highlights: ["Staff records", "Leave and contracts", "Payroll operations"],
  },
  procurement: {
    icon: PackageSearch,
    highlights: ["Requests and approvals", "Suppliers and orders", "Receiving"],
  },
  fleet: {
    icon: Truck,
    highlights: ["Vehicles", "Drivers", "Daily vehicle log"],
  },
  hostel: {
    icon: Building2,
    highlights: ["Residences and rooms", "Allocation", "Occupancy records"],
  },
  health: {
    icon: HeartPulse,
    highlights: ["Clinic visits", "Care records", "Wellbeing follow-up"],
  },
  assets_inventory: {
    icon: Boxes,
    highlights: ["Asset register", "Stores and stock", "Custodianship"],
  },
  document_registry: {
    icon: Archive,
    highlights: ["Official filing", "Classification and retention", "Document retrieval"],
  },
  internal_audit: {
    icon: ClipboardCheck,
    highlights: ["Audit planning", "Findings", "Remediation follow-up"],
  },
};

export const defaultModuleVisual: ModuleVisual = {
  icon: ShieldCheck,
  highlights: ["Module workspace", "Campus records", "Operational reports"],
};

export function stageLabel(stage: string) {
  if (stage === "available") return "Ready";
  if (stage === "foundation") return "In setup";
  return "Planned";
}
