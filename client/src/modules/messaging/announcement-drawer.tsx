import { useEffect, useMemo, useState } from "react";
import { Loader2, Plus, Trash2 } from "lucide-react";
import toast from "react-hot-toast";

import { Button } from "@/components/ui/button";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";

import { communicationService, responseMessage } from "./service";
import type { AnnouncementDetail, AnnouncementPayload, AnnouncementPriority, AudienceKind, AudienceTargetInput, CommunicationReferenceData } from "./types";

export function AnnouncementDrawer({ announcement, onClose, onSaved, open, references }: { announcement?: AnnouncementDetail | null; onClose: () => void; onSaved: (value: AnnouncementDetail) => void; open: boolean; references: CommunicationReferenceData | null }) {
  const [title, setTitle] = useState(""); const [body, setBody] = useState(""); const [priority, setPriority] = useState<AnnouncementPriority>("normal");
  const [targets, setTargets] = useState<AudienceTargetInput[]>([]); const [kind, setKind] = useState<AudienceKind>("class_group"); const [targetValue, setTargetValue] = useState(""); const [saving, setSaving] = useState(false);

  useEffect(() => { if (!open) return; setTitle(announcement?.title ?? ""); setBody(announcement?.body ?? ""); setPriority(announcement?.priority ?? "normal"); setTargets(announcement?.targets.map(({ kind: itemKind, target_id, target_key, label }) => ({ kind: itemKind, target_id, target_key, label })) ?? []); setKind(references?.classes.length ? "class_group" : references?.campus_allowed ? "campus" : "class_group"); setTargetValue(""); }, [announcement, open, references]);
  const options = useMemo(() => audienceOptions(kind, references), [kind, references]);

  const addTarget = () => {
    if (!references) return;
    const option = kind === "campus" ? { value: "campus", label: "Entire campus" } : options.find((item) => item.value === targetValue);
    if (!option) { toast.error("Choose an audience"); return; }
    const next: AudienceTargetInput = { kind, target_id: ["class_group", "department", "individual"].includes(kind) ? option.value : null, target_key: kind === "role" ? option.value : null, label: option.label };
    const identity = targetIdentity(next); if (targets.some((item) => targetIdentity(item) === identity)) { toast.error("That audience is already selected"); return; }
    setTargets((current) => [...current, next]); setTargetValue("");
  };

  const submit = async (event: React.FormEvent) => {
    event.preventDefault(); if (saving || targets.length === 0) return; setSaving(true);
    const payload: AnnouncementPayload = { title: title.trim(), body: body.trim(), priority, targets };
    try {
      const response = announcement ? await communicationService.updateAnnouncement(announcement.id, announcement.version, payload) : await communicationService.createAnnouncement(payload);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Announcement could not be saved"));
      toast.success(announcement ? "Announcement updated" : "Draft created"); onSaved(response.data);
    } catch (error) { toast.error(error instanceof Error ? error.message : "Announcement could not be saved"); } finally { setSaving(false); }
  };

  return <DialogShell onClose={saving ? () => undefined : onClose} open={open} panelClassName="sm:max-w-[720px]"><DialogHeader onClose={saving ? undefined : onClose} title={announcement ? "Edit announcement" : "New announcement"} />
    <form onSubmit={submit}><DialogBody className="space-y-6">
      <div><Label htmlFor="announcement-title">Title</Label><Input data-autofocus="true" id="announcement-title" maxLength={180} onChange={(event) => setTitle(event.target.value)} required value={title} /></div>
      <div><Label htmlFor="announcement-message">Message</Label><Textarea className="mt-1.5 min-h-48 resize-y" id="announcement-message" maxLength={10000} onChange={(event) => setBody(event.target.value)} required value={body} /></div>
      <div><Label htmlFor="announcement-priority">Priority</Label><Select className="mt-1.5" id="announcement-priority" onChange={(event) => setPriority(event.target.value as AnnouncementPriority)} value={priority}><option value="normal">Normal</option><option value="important">Important</option><option value="urgent">Urgent</option></Select></div>
      <section className="space-y-3 border-t border-[var(--border)] pt-5"><div><h3 className="text-sm font-semibold text-[var(--text-strong)]">Audience</h3><p className="mt-1 text-xs leading-5 text-[var(--text-muted)]">Recipients are reviewed and frozen when the draft is submitted.</p></div>
        {!references ? <p className="text-sm text-[var(--text-muted)]">Loading audiences…</p> : <div className="grid gap-3 sm:grid-cols-[180px_1fr_auto]">
          <Select aria-label="Audience type" onChange={(event) => { setKind(event.target.value as AudienceKind); setTargetValue(""); }} value={kind}>{references.campus_allowed ? <option value="campus">Entire campus</option> : null}<option value="class_group">Class</option>{references.campus_allowed ? <><option value="role">Role</option><option value="department">Department</option><option value="individual">Individual</option></> : null}</Select>
          {kind === "campus" ? <div className="flex h-[var(--h-control-md)] items-center rounded-[var(--radius-md)] border border-[var(--border)] px-3 text-sm text-[var(--text-muted)]">All active campus accounts</div> : <Select aria-label="Audience" onChange={(event) => setTargetValue(event.target.value)} value={targetValue}><option value="">Choose {audienceLabel(kind).toLowerCase()}</option>{options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</Select>}
          <Button onClick={addTarget} type="button" variant="secondary"><Plus className="size-4" />Add</Button>
        </div>}
        <div className="space-y-2">{targets.length === 0 ? <p className="border border-dashed border-[var(--border)] p-4 text-sm text-[var(--text-muted)]">No audience selected.</p> : targets.map((target) => <div className="flex items-center justify-between gap-3 border border-[var(--border)] bg-[var(--surface-muted)] px-3 py-2.5" key={targetIdentity(target)}><div className="min-w-0"><p className="truncate text-sm font-medium text-[var(--text-strong)]">{target.label}</p><p className="mt-0.5 text-xs text-[var(--text-muted)]">{audienceLabel(target.kind)}</p></div><Button aria-label={`Remove ${target.label}`} onClick={() => setTargets((current) => current.filter((item) => targetIdentity(item) !== targetIdentity(target)))} size="icon-sm" type="button" variant="ghost"><Trash2 className="size-4" /></Button></div>)}</div>
      </section>
    </DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !title.trim() || !body.trim() || targets.length === 0} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : null}{saving ? "Saving…" : announcement ? "Save changes" : "Create draft"}</Button></DialogFooter></form>
  </DialogShell>;
}

function audienceOptions(kind: AudienceKind, references: CommunicationReferenceData | null) { if (!references) return []; if (kind === "class_group") return references.classes.map((item) => ({ value: item.id, label: `${item.name} · ${item.code}` })); if (kind === "department") return references.departments.map((item) => ({ value: item.id, label: `${item.name} · ${item.code}` })); if (kind === "role") return references.roles.map((item) => ({ value: item.key, label: item.name })); if (kind === "individual") return references.users.map((item) => ({ value: item.id, label: `${item.full_name} · ${item.email}` })); return []; }
function targetIdentity(target: AudienceTargetInput) { return `${target.kind}:${target.target_id ?? ""}:${target.target_key ?? ""}`; }
function audienceLabel(kind: AudienceKind) { return kind === "class_group" ? "Class" : kind === "department" ? "Department" : kind === "individual" ? "Individual" : kind === "role" ? "Role" : "Campus"; }
