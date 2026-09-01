//
//  campus-pilot
//  role-form-modal.tsx - Module-first role editor drawer
//

import React, { useEffect, useMemo, useState } from "react";
import { Check, ChevronDown, Loader2, ShieldCheck, SlidersHorizontal } from "lucide-react";
import toast from "react-hot-toast";

import { Button } from "@/components/ui/button";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Textarea } from "@/components/ui/input";
import { accessService } from "@/modules/platform/access-service";
import type {
  ModuleCatalogResponse,
  PermissionDefinition,
  RecordScopeFamilyDefinition,
  RecordScopeKind,
} from "@/modules/platform/types";
import { rolesService } from "../services/roles-service";
import type { CreateRoleRequest, Role, RoleRecordScope, UpdateRoleRequest } from "../types";
import { apiErrorMessage, canDelegatePermissions } from "../access-control";
import { useAuthStore } from "@/stores/auth-store";

interface RoleFormModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: () => void;
  role?: Role;
}

type AccessMode = "full" | "custom";

interface PermissionSection {
  key: string;
  label: string;
  description: string;
  permissions: PermissionDefinition[];
  recordScopes: RecordScopeFamilyDefinition[];
}

interface PermissionGroup {
  key: string;
  label?: string;
  description?: string;
  permissions: PermissionDefinition[];
}

export const RoleFormModal: React.FC<RoleFormModalProps> = ({ isOpen, onClose, onSuccess, role }) => {
  const operatorPermissions = useAuthStore((state) => state.user?.permissions);
  const accessIsFixed = role?.is_system ?? false;
  const [formData, setFormData] = useState({
    name: "",
    description: "",
    permissions: [] as string[],
    recordScopes: [] as RoleRecordScope[],
  });
  const [accessMode, setAccessMode] = useState<AccessMode>("custom");
  const [expandedSections, setExpandedSections] = useState<string[]>([]);
  const [catalog, setCatalog] = useState<ModuleCatalogResponse | null>(null);
  const [catalogError, setCatalogError] = useState<string | null>(null);
  const [catalogRequestId, setCatalogRequestId] = useState(0);
  const [isLoadingCatalog, setIsLoadingCatalog] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const sections = useMemo<PermissionSection[]>(() => {
    if (!catalog) return [];
    return [
      {
        key: "administration",
        label: "Administration",
        description: "People, roles, licensing, and campus configuration.",
        permissions: catalog.administration_permissions.filter((permission) =>
          canDelegatePermissions(operatorPermissions, [permission.key]),
        ),
        recordScopes: [],
      },
      ...catalog.modules
        .filter((module) => module.key !== "administration")
        .map((module) => ({
          key: module.key,
          label: module.label,
          description: module.description,
          permissions: module.permissions.filter((permission) =>
            canDelegatePermissions(operatorPermissions, [permission.key]),
          ),
          recordScopes: catalog.record_scope_families.filter((family) => family.module_key === module.key),
        })),
    ].filter((section) => section.permissions.length > 0);
  }, [catalog, operatorPermissions]);

  const allPermissionKeys = useMemo(
    () => sections.flatMap((section) => section.permissions.map((permission) => permission.key)),
    [sections],
  );

  const fixedSections = useMemo(() => {
    if (!accessIsFixed || !role || role.permissions.includes("*")) return [];
    return sections
      .map((section) => ({
        ...section,
        permissions: section.permissions.filter((permission) => role.permissions.includes(permission.key)),
        recordScopes: section.recordScopes.filter((family) =>
          role.record_scopes.some((assignment) => assignment.family === family.key),
        ),
      }))
      .filter((section) => section.permissions.length > 0 || section.recordScopes.length > 0);
  }, [accessIsFixed, role, sections]);

  useEffect(() => {
    if (!isOpen) return;
    const hasFullAccess = role?.permissions.includes("*") ?? false;
    setExpandedSections([]);
    setAccessMode(hasFullAccess ? "full" : "custom");
    setFormData({
      name: role?.name ?? "",
      description: role?.description ?? "",
      permissions: hasFullAccess ? [] : (role?.permissions ?? []),
      recordScopes: hasFullAccess ? [] : (role?.record_scopes ?? []),
    });
  }, [role, isOpen]);

  useEffect(() => {
    if (!isOpen || catalog) return;
    let active = true;
    setIsLoadingCatalog(true);
    setCatalogError(null);
    void accessService
      .getCatalog()
      .then((response) => {
        if (!active) return;
        if (response.success && response.data) setCatalog(response.data);
        else setCatalogError(response.message || "The permission catalogue could not be loaded.");
      })
      .catch(() => {
        if (active) setCatalogError("Campus Pilot could not reach the permission catalogue.");
      })
      .finally(() => {
        if (active) setIsLoadingCatalog(false);
      });
    return () => {
      active = false;
    };
  }, [catalog, catalogRequestId, isOpen]);

  const selectAccessMode = (mode: AccessMode) => {
    if (mode === "custom" && accessMode === "full" && formData.permissions.length === 0) {
      setFormData((current) => ({ ...current, permissions: allPermissionKeys }));
    }
    setAccessMode(mode);
  };

  const togglePermission = (permission: string) => {
    setFormData((current) => {
      const permissions = current.permissions.includes(permission)
        ? current.permissions.filter((item) => item !== permission)
        : [...current.permissions, permission];
      const section = sections.find((candidate) =>
        candidate.permissions.some((item) => item.key === permission),
      );
      const sectionStillSelected = section?.permissions.some((item) => permissions.includes(item.key)) ?? true;
      return {
        ...current,
        permissions,
        recordScopes: section && !sectionStillSelected
          ? current.recordScopes.filter((assignment) =>
              !section.recordScopes.some((family) => family.key === assignment.family),
            )
          : current.recordScopes,
      };
    });
  };

  const toggleRecordScope = (family: string, kind: RecordScopeKind) => {
    setFormData((current) => {
      const selected = current.recordScopes.some(
        (assignment) => assignment.family === family && assignment.kind === kind,
      );
      const withoutFamily = current.recordScopes.filter((assignment) => assignment.family !== family);
      const withoutExact = current.recordScopes.filter(
        (assignment) => !(assignment.family === family && assignment.kind === kind),
      );
      if (selected) return { ...current, recordScopes: withoutExact };
      if (kind === "campus") {
        return { ...current, recordScopes: [...withoutFamily, { family, kind }] };
      }
      return {
        ...current,
        recordScopes: [
          ...current.recordScopes.filter(
            (assignment) => !(assignment.family === family && assignment.kind === "campus"),
          ),
          { family, kind },
        ],
      };
    });
  };

  const toggleSection = (section: PermissionSection) => {
    const keys = section.permissions.map((permission) => permission.key);
    const sectionSelected = keys.every((key) => formData.permissions.includes(key));
    setFormData((current) => ({
      ...current,
      permissions: sectionSelected
        ? current.permissions.filter((permission) => !keys.includes(permission))
        : Array.from(new Set([...current.permissions, ...keys])),
      recordScopes: sectionSelected
        ? current.recordScopes.filter((assignment) =>
            !section.recordScopes.some((family) => family.key === assignment.family),
          )
        : current.recordScopes,
    }));
  };

  const toggleExpanded = (sectionKey: string) => {
    setExpandedSections((current) =>
      current.includes(sectionKey)
        ? current.filter((key) => key !== sectionKey)
        : [...current, sectionKey],
    );
  };

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!formData.name.trim()) {
      toast.error("Role name is required");
      return;
    }
    if (accessMode === "custom" && formData.permissions.length === 0) {
      toast.error("Select at least one permission");
      return;
    }

    const permissions = accessMode === "full" ? ["*"] : formData.permissions;
    const activeFamilies = new Set(
      sections
        .filter((section) => section.permissions.some((permission) => permissions.includes(permission.key)))
        .flatMap((section) => section.recordScopes.map((family) => family.key)),
    );
    const recordScopes = accessMode === "full"
      ? []
      : formData.recordScopes.filter((assignment) => activeFamilies.has(assignment.family));
    setIsSubmitting(true);
    try {
      const response = role
        ? await rolesService.updateRole(role.id, {
            name: formData.name.trim(),
            description: formData.description.trim() || null,
            permissions: accessIsFixed ? undefined : permissions,
            record_scopes: accessIsFixed ? undefined : recordScopes,
          } satisfies UpdateRoleRequest)
        : await rolesService.createRole({
            name: formData.name.trim(),
            description: formData.description.trim() || null,
            permissions,
            record_scopes: recordScopes,
          } satisfies CreateRoleRequest);

      if (response.success) {
        toast.success(role ? "Role updated" : "Role created");
        onSuccess();
        onClose();
      } else {
        toast.error(apiErrorMessage(response, role ? "Failed to update role" : "Failed to create role"));
      }
    } catch {
      toast.error(role ? "Failed to update role" : "Failed to create role");
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <DialogShell open={isOpen} onClose={onClose} panelClassName="max-w-[720px]">
      <DialogHeader title={role ? "Edit role" : "Create role"} onClose={onClose} />
      <form className="flex min-h-0 flex-1 flex-col overflow-hidden" onSubmit={handleSubmit}>
        <DialogBody className="space-y-7">
          <section className="space-y-4" aria-labelledby="role-details-heading">
            <h3 className="text-sm font-semibold text-[var(--text-strong)]" id="role-details-heading">Role details</h3>
            <div>
              <Label htmlFor="role-name">Role name <span className="text-[var(--tone-danger)]">*</span></Label>
              <Input
                className="mt-1.5"
                data-autofocus="true"
                id="role-name"
                onChange={(event) => setFormData({ ...formData, name: event.target.value })}
                placeholder="e.g. Head of department"
                required
                value={formData.name}
              />
            </div>
            <div>
              <Label htmlFor="role-description">Description</Label>
              <Textarea
                className="mt-1.5 resize-none"
                id="role-description"
                onChange={(event) => setFormData({ ...formData, description: event.target.value })}
                placeholder="Explain who should be assigned this role"
                rows={3}
                value={formData.description}
              />
            </div>
          </section>

          <section className="space-y-4" aria-labelledby="access-profile-heading">
            <div>
              <h3 className="text-sm font-semibold text-[var(--text-strong)]" id="access-profile-heading">Access profile</h3>
              <p className="mt-1 text-xs leading-5 text-[var(--text-muted)]">{accessIsFixed ? "Built-in access is fixed. Create a custom role for different responsibilities." : "Choose full access or select permissions by module."}</p>
            </div>
            {accessIsFixed ? (
              <div className="rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface-muted)] p-4">
                <p className="text-sm font-semibold text-[var(--text-strong)]">{role?.permissions.includes("*") ? "Full campus access" : `${role?.permissions.length ?? 0} fixed permissions`}</p>
                <p className="mt-1 text-xs leading-5 text-[var(--text-muted)]">The role name and description can still be changed.</p>
              </div>
            ) : (
              <div className="grid gap-3 sm:grid-cols-2">
                {operatorPermissions?.includes("*") ? <AccessModeCard
                  active={accessMode === "full"}
                  description="Every permission in current and future modules."
                  icon={ShieldCheck}
                  label="Full access"
                  onClick={() => selectAccessMode("full")}
                /> : null}
                <AccessModeCard
                  active={accessMode === "custom"}
                  description="Select permissions by module."
                  icon={SlidersHorizontal}
                  label="Custom access"
                  onClick={() => selectAccessMode("custom")}
                />
              </div>
            )}
          </section>

          {accessIsFixed && fixedSections.length > 0 ? (
            <section className="space-y-3" aria-labelledby="fixed-permissions-heading">
              <div>
                <h3 className="text-sm font-semibold text-[var(--text-strong)]" id="fixed-permissions-heading">Assigned access</h3>
                <p className="mt-1 text-xs leading-5 text-[var(--text-muted)]">These responsibilities are included in this built-in role.</p>
              </div>
              {fixedSections.map((section) => (
                <div className="overflow-hidden rounded-[var(--radius-lg)] border border-[var(--border)]" key={section.key}>
                  <div className="bg-[var(--surface-muted)] px-4 py-3">
                    <p className="text-sm font-semibold text-[var(--text-strong)]">{section.label}</p>
                  </div>
                  <PermissionGroups groups={permissionGroups(section)} readOnly />
                  {section.recordScopes.length > 0 ? <RecordScopeEditor
                    assignments={role?.record_scopes ?? []}
                    families={section.recordScopes}
                    readOnly
                  /> : null}
                </div>
              ))}
            </section>
          ) : null}

          {!accessIsFixed && accessMode === "custom" ? (
            <section className="space-y-3" aria-labelledby="module-permissions-heading">
              <div className="flex items-end justify-between gap-4">
                <div>
                  <h3 className="text-sm font-semibold text-[var(--text-strong)]" id="module-permissions-heading">Module permissions</h3>
                  <p className="mt-1 text-xs text-[var(--text-muted)]">
                    {formData.permissions.length} permission{formData.permissions.length === 1 ? "" : "s"} selected
                  </p>
                </div>
                {allPermissionKeys.length > 0 ? (
                  <button
                    className="rounded text-xs font-semibold text-[var(--text-link)] hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]"
                    onClick={() => setFormData((current) => ({
                      ...current,
                      permissions: current.permissions.length === allPermissionKeys.length ? [] : allPermissionKeys,
                      recordScopes: current.permissions.length === allPermissionKeys.length ? [] : current.recordScopes,
                    }))}
                    type="button"
                  >
                    {formData.permissions.length === allPermissionKeys.length ? "Clear all" : "Select all"}
                  </button>
                ) : null}
              </div>

              {isLoadingCatalog ? (
                <div className="flex min-h-32 items-center justify-center rounded-[var(--radius-lg)] border border-[var(--border)]">
                  <Loader2 className="size-5 animate-spin text-[var(--brand)]" />
                  <span className="ml-2 text-sm text-[var(--text-muted)]">Loading access rules…</span>
                </div>
              ) : catalogError ? (
                <div className="rounded-[var(--radius-lg)] border border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] p-4">
                  <p className="text-sm font-medium text-[var(--tone-danger)]">{catalogError}</p>
                  <Button className="mt-3" onClick={() => setCatalogRequestId((current) => current + 1)} type="button" variant="secondary">Try again</Button>
                </div>
              ) : (
                <div className="space-y-3">
                  {sections.map((section) => {
                    const selectedCount = section.permissions.filter((permission) => formData.permissions.includes(permission.key)).length;
                    const allSelected = selectedCount === section.permissions.length;
                    const expanded = expandedSections.includes(section.key);
                    return (
                      <div className="overflow-hidden rounded-[var(--radius-lg)] border border-[var(--border)]" key={section.key}>
                        <div className="flex items-start justify-between gap-4 bg-[var(--surface-muted)] px-4 py-3">
                          <button
                            aria-expanded={expanded}
                            className="flex min-w-0 flex-1 items-start gap-3 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]"
                            onClick={() => toggleExpanded(section.key)}
                            type="button"
                          >
                            <ChevronDown className={`mt-0.5 size-4 shrink-0 text-[var(--text-muted)] transition-transform ${expanded ? "rotate-180" : ""}`} />
                            <span className="min-w-0">
                              <span className="flex flex-wrap items-center gap-2">
                                <span className="text-sm font-semibold text-[var(--text-strong)]">{section.label}</span>
                                <span className="rounded-full border border-[var(--border)] bg-[var(--surface)] px-2 py-0.5 text-[10px] font-semibold text-[var(--text-muted)]">
                                  {selectedCount} of {section.permissions.length}
                                </span>
                              </span>
                              <span className="mt-0.5 block text-xs leading-5 text-[var(--text-muted)]">{section.description}</span>
                            </span>
                          </button>
                          <button
                            className="shrink-0 rounded text-xs font-semibold text-[var(--text-link)] hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]"
                            onClick={() => toggleSection(section)}
                            type="button"
                          >
                            {allSelected ? "Clear" : "Allow all"}
                          </button>
                        </div>
                        {expanded ? <>
                          <PermissionGroups
                            checkedPermissions={formData.permissions}
                            groups={permissionGroups(section)}
                            onToggle={togglePermission}
                          />
                          {selectedCount > 0 && section.recordScopes.length > 0 ? <RecordScopeEditor
                            assignments={formData.recordScopes}
                            families={section.recordScopes}
                            onToggle={toggleRecordScope}
                          /> : null}
                        </> : null}
                      </div>
                    );
                  })}
                </div>
              )}
            </section>
          ) : !accessIsFixed ? (
            <div className="flex gap-3 rounded-[var(--radius-lg)] border border-[var(--brand-100)] bg-[var(--brand-soft)] p-4">
              <Check className="mt-0.5 size-5 shrink-0 text-[var(--brand-strong)]" />
              <div>
                <p className="text-sm font-semibold text-[var(--text-strong)]">All modules</p>
                <p className="mt-1 text-xs leading-5 text-[var(--text-muted)]">
                  Includes every permission in current and future modules.
                </p>
              </div>
            </div>
          ) : null}
        </DialogBody>
        <DialogFooter>
          <Button disabled={isSubmitting} onClick={onClose} type="button" variant="ghost">Cancel</Button>
          <Button disabled={isSubmitting || (!accessIsFixed && accessMode === "custom" && (isLoadingCatalog || !!catalogError))} type="submit">
            {isSubmitting ? <Loader2 className="size-4 animate-spin" /> : null}
            {isSubmitting ? (role ? "Saving…" : "Creating…") : (role ? "Save changes" : "Create role")}
          </Button>
        </DialogFooter>
      </form>
    </DialogShell>
  );
};

function permissionGroups(section: PermissionSection): PermissionGroup[] {
  if (section.key !== "academics") {
    return [{ key: section.key, permissions: section.permissions }];
  }

  const teachingKeys = new Set(["academics:view", "academics:teach"]);
  const teaching = section.permissions.filter((permission) => teachingKeys.has(permission.key));
  const administration = section.permissions.filter((permission) => !teachingKeys.has(permission.key));

  return [
    {
      key: "academics-teaching",
      label: "Teaching work",
      description: "Enter marks and comments for assigned classes. Teacher profiles, grade levels, grading policy, and publication remain administrative.",
      permissions: teaching,
    },
    {
      key: "academics-administration",
      label: "Academic administration",
      description: "Configure staffing, structures, assessment setup, publication, and progression.",
      permissions: administration,
    },
  ].filter((group) => group.permissions.length > 0);
}

function PermissionGroups({
  checkedPermissions = [],
  groups,
  onToggle,
  readOnly = false,
}: {
  checkedPermissions?: string[];
  groups: PermissionGroup[];
  onToggle?: (permission: string) => void;
  readOnly?: boolean;
}) {
  return (
    <div className="divide-y divide-[var(--border)]">
      {groups.map((group) => (
        <div key={group.key}>
          {group.label ? <div className="border-b border-[var(--border)] bg-[var(--surface)] px-4 py-3">
            <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--brand-strong)]">{group.label}</p>
            {group.description ? <p className="mt-1 text-xs leading-5 text-[var(--text-muted)]">{group.description}</p> : null}
          </div> : null}
          <div className="grid gap-px bg-[var(--border)] sm:grid-cols-2">
            {group.permissions.map((permission) => (
              <label className={`flex gap-3 bg-[var(--surface)] p-4 ${readOnly ? "" : "cursor-pointer hover:bg-[var(--surface-muted)]"}`} key={permission.key}>
                {readOnly ? (
                  <span className="mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full bg-[var(--brand-soft)] text-[var(--brand-strong)]">
                    <Check className="size-3" />
                  </span>
                ) : (
                  <input
                    checked={checkedPermissions.includes(permission.key)}
                    className="mt-0.5 size-4 rounded border-[var(--border)] text-[var(--brand)] focus:ring-[var(--focus-ring)]"
                    onChange={() => onToggle?.(permission.key)}
                    type="checkbox"
                  />
                )}
                <span>
                  <span className="block text-sm font-medium text-[var(--text-strong)]">{permission.label}</span>
                  <span className="mt-0.5 block text-xs leading-5 text-[var(--text-muted)]">{permission.description}</span>
                </span>
              </label>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

function RecordScopeEditor({
  assignments,
  families,
  onToggle,
  readOnly = false,
}: {
  assignments: RoleRecordScope[];
  families: RecordScopeFamilyDefinition[];
  onToggle?: (family: string, kind: RecordScopeKind) => void;
  readOnly?: boolean;
}) {
  return (
    <div className="border-t border-[var(--border)] bg-[var(--surface-muted)] px-4 py-4">
      <div>
        <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--brand-strong)]">Record visibility</p>
        <p className="mt-1 text-xs leading-5 text-[var(--text-muted)]">
          {readOnly
            ? "These limits apply after the permission check."
            : "Choose whose records this role can access. No selection means no records in that group."}
        </p>
      </div>
      <div className="mt-3 divide-y divide-[var(--border)] overflow-hidden rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface)]">
        {families.map((family) => {
          const selected = assignments.filter((assignment) => assignment.family === family.key);
          return (
            <div className="gap-3 px-3 py-3 sm:flex sm:items-center sm:justify-between" key={family.key}>
              <p className="text-sm font-medium text-[var(--text-strong)]">{family.label}</p>
              <div className="mt-2 flex flex-wrap gap-2 sm:mt-0 sm:justify-end">
                {(readOnly ? selected.map((assignment) => assignment.kind) : family.allowed_kinds).map((kind) => {
                  const checked = selected.some((assignment) => assignment.kind === kind);
                  return readOnly ? (
                    <span className="rounded-full border border-[var(--brand-100)] bg-[var(--brand-soft)] px-2.5 py-1 text-xs font-medium text-[var(--brand-strong)]" key={kind}>
                      {scopeKindLabel(kind)}
                    </span>
                  ) : (
                    <label className="flex cursor-pointer items-center gap-2 rounded-full border border-[var(--border)] bg-[var(--surface)] px-2.5 py-1 text-xs text-[var(--text-body)] hover:bg-[var(--surface-muted)]" key={kind}>
                      <input
                        checked={checked}
                        className="size-3.5 rounded border-[var(--border)] text-[var(--brand)] focus:ring-[var(--focus-ring)]"
                        onChange={() => onToggle?.(family.key, kind)}
                        type="checkbox"
                      />
                      {scopeKindLabel(kind)}
                    </label>
                  );
                })}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function scopeKindLabel(kind: RecordScopeKind) {
  if (kind === "self") return "Own record";
  if (kind === "assigned") return "Assigned records";
  return "All campus records";
}

function AccessModeCard({
  active,
  description,
  icon: Icon,
  label,
  onClick,
}: {
  active: boolean;
  description: string;
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      aria-pressed={active}
      className={`relative rounded-[var(--radius-lg)] border p-4 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] ${
        active
          ? "border-[var(--brand)] bg-[var(--brand-soft)]"
          : "border-[var(--border)] bg-[var(--surface)] hover:bg-[var(--surface-muted)]"
      }`}
      onClick={onClick}
      type="button"
    >
      <span className={`flex size-9 items-center justify-center rounded-[var(--radius-md)] ${active ? "bg-[var(--brand)] text-white" : "bg-[var(--surface-muted)] text-[var(--text-muted)]"}`}>
        <Icon className="size-4" />
      </span>
      <span className="mt-3 block text-sm font-semibold text-[var(--text-strong)]">{label}</span>
      <span className="mt-1 block text-xs leading-5 text-[var(--text-muted)]">{description}</span>
      {active ? <Check className="absolute right-4 top-4 size-4 text-[var(--brand-strong)]" /> : null}
    </button>
  );
}
