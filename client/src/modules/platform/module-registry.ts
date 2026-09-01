import {
  Archive,
  Bot,
  BookOpen,
  Boxes,
  BriefcaseBusiness,
  Building2,
  CalendarClock,
  CalendarCheck2,
  ClipboardCheck,
  GraduationCap,
  HeartHandshake,
  HeartPulse,
  Landmark,
  Library,
  MessageSquareText,
  PackageSearch,
  ReceiptText,
  Settings2,
  ShieldCheck,
  Truck,
  Wrench,
} from "lucide-react";

import type { ModuleVisual } from "./types";

export const moduleVisuals: Record<string, ModuleVisual> = {
  administration: {
    icon: Settings2,
    highlights: ["Users and access", "Module licensing", "School configuration"],
  },
  agent: {
    icon: Bot,
    highlights: ["Sessions and history", "Campus capabilities", "Personal usage"],
  },
  sis: {
    icon: GraduationCap,
    highlights: ["Applications and admissions", "Learner records", "Guardians and enrolment"],
  },
  academics: {
    icon: BookOpen,
    highlights: ["Subjects and classes", "Assessment structures", "Progression and reporting"],
  },
  attendance: {
    icon: CalendarCheck2,
    highlights: ["Daily registers", "Learner marks", "Submission history"],
  },
  learning: {
    icon: BookOpen,
    highlights: ["Class spaces", "Ordered units", "Governed resources"],
  },
  student_support: {
    icon: HeartHandshake,
    highlights: ["Restricted cases", "Assigned case teams", "Lifecycle evidence"],
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
  facilities: {
    icon: Wrench,
    highlights: ["Service requests", "Assigned work orders", "Locations and inspections"],
  },
  transport: {
    icon: Truck,
    highlights: ["Routes and stops", "Rider assignments", "Daily manifests"],
  },
  hostel: {
    icon: Building2,
    highlights: ["Residences and rooms", "Learner allocations", "Pastoral records"],
  },
  health: {
    icon: HeartPulse,
    highlights: ["Patient care records", "Clinic visits", "Medication and follow-up"],
  },
  assets_inventory: {
    icon: Boxes,
    highlights: ["Items and stores", "Stock balances", "Movement history"],
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

export function moduleRouteKey(moduleKey: string) {
  return moduleKey.replace(/_/g, "-");
}
