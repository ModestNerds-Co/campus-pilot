import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  AlertTriangle,
  CalendarClock,
  Check,
  CheckCircle2,
  ChevronDown,
  Clock3,
  Edit3,
  Loader2,
  Plus,
  Search,
  Sparkles,
  Trash2,
} from "lucide-react";
import toast from "react-hot-toast";

import { Button } from "@/components/ui/button";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import type { ModuleDefinition } from "@/modules/platform/types";
import { useAuthStore } from "@/stores/auth-store";

import { timetablingService } from "./timetabling-service";
import type {
  LessonRequirement,
  NamedResource,
  TeacherResource,
  TimetableConfiguration,
  TimetableRun,
} from "./types";

type RegistryKind = "classes" | "subjects" | "teachers" | "rooms";
type DrawerState =
  | { kind: "week" }
  | { kind: "registry"; registry: RegistryKind; item?: NamedResource | TeacherResource }
  | { kind: "lesson"; item?: LessonRequirement }
  | null;

const registryLabels: Record<RegistryKind, { singular: string; plural: string }> = {
  classes: { singular: "class", plural: "Classes" },
  subjects: { singular: "subject", plural: "Subjects" },
  teachers: { singular: "teacher", plural: "Teachers" },
  rooms: { singular: "room", plural: "Rooms" },
};

const standardDays = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

export const TimetableWorkspace: React.FC<{ module: ModuleDefinition }> = ({ module }) => {
  const user = useAuthStore((state) => state.user);
  const permissions = user?.permissions ?? [];
  const canEdit = permissions.includes("*") || permissions.includes("timetabling:edit");
  const canGenerate = permissions.includes("*") || permissions.includes("timetabling:create");
  const [configuration, setConfiguration] = useState<TimetableConfiguration | null>(null);
  const [latestRun, setLatestRun] = useState<TimetableRun | null>(null);
  const [drawer, setDrawer] = useState<DrawerState>(null);
  const [activeView, setActiveView] = useState<"setup" | "review">("setup");
  const [selectedClassId, setSelectedClassId] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [isGenerating, setIsGenerating] = useState(false);
  const [isPublishing, setIsPublishing] = useState(false);
  const [isDirty, setIsDirty] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);

  usePageChrome("Overview");

  const load = useCallback(async () => {
    setIsLoading(true);
    setLoadError(null);
    try {
      const [configResponse, runResponse] = await Promise.all([
        timetablingService.getConfiguration(),
        timetablingService.getLatestRun(),
      ]);
      if (!configResponse.success || !configResponse.data) {
        setLoadError(configResponse.message || "The timetable configuration could not be loaded.");
        return;
      }
      setConfiguration(configResponse.data);
      setLatestRun(runResponse.success ? runResponse.data : null);
      setSelectedClassId(runResponse.data?.configuration.classes[0]?.id ?? configResponse.data.classes[0]?.id ?? null);
      setIsDirty(false);
    } catch {
      setLoadError("Campus Pilot could not reach the timetabling service.");
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => { void load(); }, [load]);

  const updateConfiguration = (next: TimetableConfiguration) => {
    setConfiguration(next);
    setIsDirty(true);
  };

  const readiness = useMemo(() => {
    if (!configuration) return [];
    return [
      { label: "Teaching week", ready: configuration.days.length > 0 && configuration.periods.length > 0, detail: `${configuration.days.length} days · ${configuration.periods.length} periods` },
      { label: "Classes and subjects", ready: configuration.classes.length > 0 && configuration.subjects.length > 0, detail: `${configuration.classes.length} classes · ${configuration.subjects.length} subjects` },
      { label: "Teacher assignments", ready: configuration.teachers.length > 0, detail: `${configuration.teachers.length} teachers` },
      { label: "Teaching requirements", ready: configuration.lesson_requirements.length > 0, detail: `${configuration.lesson_requirements.length} requirements` },
    ];
  }, [configuration]);
  const readyToGenerate = readiness.length > 0 && readiness.every((item) => item.ready);

  const save = async (quiet = false) => {
    if (!configuration || isSaving) return false;
    setIsSaving(true);
    try {
      const response = await timetablingService.saveConfiguration(configuration);
      if (!response.success || !response.data) {
        toast.error(response.message || "Timetable setup could not be saved");
        return false;
      }
      setConfiguration(response.data);
      setIsDirty(false);
      if (!quiet) toast.success("Timetable setup saved");
      return true;
    } catch {
      toast.error("Timetable setup could not be saved");
      return false;
    } finally {
      setIsSaving(false);
    }
  };

  const generate = async () => {
    if (!configuration || isGenerating || !readyToGenerate) return;
    setIsGenerating(true);
    try {
      if (isDirty && !(await save(true))) return;
      const response = await timetablingService.generate();
      if (!response.success || !response.data) {
        toast.error(response.message || "A timetable draft could not be generated");
        return;
      }
      setLatestRun(response.data);
      setSelectedClassId(response.data.configuration.classes[0]?.id ?? null);
      setActiveView("review");
      toast.success(response.data.unresolved.length === 0 ? "Conflict-free draft generated" : "Draft generated with unresolved lessons");
    } catch {
      toast.error("A timetable draft could not be generated");
    } finally {
      setIsGenerating(false);
    }
  };

  const publish = async () => {
    if (!latestRun || isPublishing || latestRun.unresolved.length > 0) return;
    setIsPublishing(true);
    try {
      const response = await timetablingService.publish(latestRun.id);
      if (!response.success || !response.data) {
        toast.error(response.message || "The timetable could not be published");
        return;
      }
      setLatestRun(response.data);
      toast.success("Timetable published");
    } catch {
      toast.error("The timetable could not be published");
    } finally {
      setIsPublishing(false);
    }
  };

  if (isLoading) return <div className="h-72 animate-pulse bg-[var(--surface-sunken)]" />;
  if (loadError || !configuration) {
    return <StateMessage title="Timetabling could not be opened" description={loadError ?? "No timetable configuration was returned."} action={<Button onClick={() => void load()} variant="secondary">Try again</Button>} />;
  }

  return (
    <div className="space-y-8">
      <section className="relative overflow-hidden bg-[var(--sidebar)] px-6 py-8 text-[var(--sidebar-foreground)] sm:px-8 sm:py-10">
        <div aria-hidden="true" className="campus-grid-pattern absolute inset-0 opacity-40" />
        <div className="relative flex flex-col gap-6 lg:flex-row lg:items-end lg:justify-between">
          <div className="max-w-3xl">
            <div className="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-highlight)]"><CheckCircle2 className="size-3.5" />Available</div>
            <h1 className="mt-3 text-2xl font-semibold tracking-[-0.035em] sm:text-3xl">{module.label}</h1>
            <p className="mt-3 max-w-2xl text-sm leading-6 text-[var(--sidebar-muted)]">Build from verified school rules, generate without class, teacher, or room collisions, then review before publishing.</p>
          </div>
          <div className="flex flex-wrap gap-3">
            {canEdit ? <Button disabled={!isDirty || isSaving} onClick={() => void save()} variant="secondary">{isSaving ? <Loader2 className="size-4 animate-spin" /> : <Check className="size-4" />}Save setup</Button> : null}
            {canGenerate ? <Button disabled={!readyToGenerate || isGenerating || isSaving} onClick={() => void generate()}>{isGenerating ? <Loader2 className="size-4 animate-spin" /> : <Sparkles className="size-4" />}Generate draft</Button> : null}
          </div>
        </div>
      </section>

      <div className="flex gap-1 border-b border-[var(--border)]" role="tablist" aria-label="Timetable workflow">
        <ViewTab active={activeView === "setup"} label="Rules and setup" onClick={() => setActiveView("setup")} />
        <ViewTab active={activeView === "review"} label={latestRun ? "Review draft" : "Review"} onClick={() => setActiveView("review")} />
      </div>

      {activeView === "setup" ? (
        <div className="space-y-8">
          <section className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4" aria-label="Generation readiness">
            {readiness.map((item) => (
              <div className="border border-[var(--border)] bg-[var(--surface)] p-4" key={item.label}>
                <div className="flex items-center justify-between gap-3">
                  <span className="text-sm font-semibold text-[var(--text-strong)]">{item.label}</span>
                  {item.ready ? <CheckCircle2 className="size-4 text-[var(--tone-success)]" /> : <Clock3 className="size-4 text-[var(--text-subtle)]" />}
                </div>
                <p className="mt-2 text-xs text-[var(--text-muted)]">{item.detail}</p>
              </div>
            ))}
          </section>

          <section className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_360px]">
            <div className="border border-[var(--border)] bg-[var(--surface)] p-5 sm:p-6">
              <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
                <div>
                  <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]">01 · Calendar rules</p>
                  <h2 className="mt-1 text-lg font-semibold text-[var(--text-strong)]">Academic cycle and teaching week</h2>
                </div>
                {canEdit ? <Button onClick={() => setDrawer({ kind: "week" })} size="sm" variant="secondary"><Edit3 className="size-3.5" />Edit week</Button> : null}
              </div>
              <div className="mt-5 grid gap-5 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-end">
                <div>
                  <Label htmlFor="cycle-name">Academic cycle</Label>
                  <Input className="mt-2" disabled={!canEdit} id="cycle-name" onChange={(event) => updateConfiguration({ ...configuration, cycle_name: event.target.value })} value={configuration.cycle_name} />
                </div>
                <p className="pb-2 text-sm text-[var(--text-muted)]">{configuration.days.map((day) => day.label.slice(0, 3)).join(" · ")} · {configuration.periods.length} periods</p>
              </div>
            </div>

            <aside className="bg-[var(--surface-muted)] p-5">
              <div className="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]"><Sparkles className="size-3.5" />Constraint policy</div>
              <p className="mt-3 text-sm leading-6 text-[var(--text-muted)]">Generation blocks overlapping classes, teachers, and rooms. Teacher unavailability is a hard rule; balanced teaching days are optimized as a preference.</p>
            </aside>
          </section>

          <section aria-labelledby="registries-heading">
            <div className="border-b border-[var(--border)] pb-3">
              <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]">02 · Academic setup</p>
              <h2 className="mt-1 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]" id="registries-heading">Scheduling registries</h2>
            </div>
            <div className="grid gap-5 pt-5 md:grid-cols-2">
              {(["classes", "subjects", "teachers", "rooms"] as RegistryKind[]).map((registry) => (
                <RegistryCard
                  canEdit={canEdit}
                  items={configuration[registry]}
                  key={registry}
                  label={registryLabels[registry].plural}
                  onAdd={() => setDrawer({ kind: "registry", registry })}
                  onEdit={(item) => setDrawer({ kind: "registry", registry, item })}
                  onRemove={(id) => removeRegistryItem(configuration, registry, id, updateConfiguration)}
                />
              ))}
            </div>
          </section>

          <LessonRequirements
            canEdit={canEdit}
            configuration={configuration}
            onAdd={() => setDrawer({ kind: "lesson" })}
            onEdit={(item) => setDrawer({ kind: "lesson", item })}
            onRemove={(id) => updateConfiguration({ ...configuration, lesson_requirements: configuration.lesson_requirements.filter((item) => item.id !== id) })}
          />
        </div>
      ) : (
        <RunReview canPublish={canEdit} isPublishing={isPublishing} onPublish={() => void publish()} run={latestRun} selectedClassId={selectedClassId} setSelectedClassId={setSelectedClassId} />
      )}

      <WeekDrawer configuration={configuration} onClose={() => setDrawer(null)} onSave={updateConfiguration} open={drawer?.kind === "week"} />
      <RegistryDrawer configuration={configuration} drawer={drawer?.kind === "registry" ? drawer : null} onClose={() => setDrawer(null)} onSave={updateConfiguration} />
      <LessonDrawer configuration={configuration} item={drawer?.kind === "lesson" ? drawer.item : undefined} onClose={() => setDrawer(null)} onSave={updateConfiguration} open={drawer?.kind === "lesson"} />
    </div>
  );
};

const ViewTab: React.FC<{ active: boolean; label: string; onClick: () => void }> = ({ active, label, onClick }) => (
  <button aria-selected={active} className={`border-b-2 px-4 py-3 text-sm font-semibold ${active ? "border-[var(--brand)] text-[var(--brand-strong)]" : "border-transparent text-[var(--text-muted)] hover:text-[var(--text-strong)]"}`} onClick={onClick} role="tab" type="button">{label}</button>
);

const RegistryCard: React.FC<{ canEdit: boolean; items: Array<NamedResource | TeacherResource>; label: string; onAdd: () => void; onEdit: (item: NamedResource | TeacherResource) => void; onRemove: (id: string) => void }> = ({ canEdit, items, label, onAdd, onEdit, onRemove }) => (
  <div className="border border-[var(--border)] bg-[var(--surface)]">
    <div className="flex items-center justify-between gap-4 border-b border-[var(--border-subtle)] px-5 py-4">
      <div><h3 className="font-semibold text-[var(--text-strong)]">{label}</h3><p className="mt-0.5 text-xs text-[var(--text-muted)]">{items.length} configured</p></div>
      {canEdit ? <Button aria-label={`Add ${label.toLowerCase()}`} onClick={onAdd} size="icon-sm" variant="secondary"><Plus className="size-4" /></Button> : null}
    </div>
    <div className="max-h-64 divide-y divide-[var(--border-subtle)] overflow-y-auto">
      {items.length === 0 ? <p className="px-5 py-7 text-sm text-[var(--text-muted)]">No {label.toLowerCase()} configured yet.</p> : items.map((item) => (
        <div className="flex items-center justify-between gap-4 px-5 py-3" key={item.id}>
          <div className="min-w-0"><p className="truncate text-sm font-medium text-[var(--text-strong)]">{item.name}</p>{"unavailable_slots" in item && item.unavailable_slots.length > 0 ? <p className="mt-0.5 text-xs text-[var(--text-muted)]">{item.unavailable_slots.length} unavailable slots</p> : null}</div>
          {canEdit ? <div className="flex gap-1"><Button aria-label={`Edit ${item.name}`} onClick={() => onEdit(item)} size="icon-sm" variant="ghost"><Edit3 className="size-3.5" /></Button><Button aria-label={`Remove ${item.name}`} onClick={() => onRemove(item.id)} size="icon-sm" variant="ghost"><Trash2 className="size-3.5" /></Button></div> : null}
        </div>
      ))}
    </div>
  </div>
);

const LessonRequirements: React.FC<{ canEdit: boolean; configuration: TimetableConfiguration; onAdd: () => void; onEdit: (item: LessonRequirement) => void; onRemove: (id: string) => void }> = ({ canEdit, configuration, onAdd, onEdit, onRemove }) => (
  <section aria-labelledby="requirements-heading">
    <div className="flex flex-col gap-4 border-b border-[var(--border)] pb-3 sm:flex-row sm:items-end sm:justify-between">
      <div><p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]">03 · Teaching load</p><h2 className="mt-1 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]" id="requirements-heading">Lesson requirements</h2></div>
      {canEdit ? <Button disabled={configuration.classes.length === 0 || configuration.subjects.length === 0 || configuration.teachers.length === 0} onClick={onAdd} size="sm"><Plus className="size-4" />Add requirement</Button> : null}
    </div>
    {configuration.lesson_requirements.length === 0 ? (
      <StateMessage description="Connect a class, subject, and teacher, then define how many periods they need each cycle." title="No teaching requirements yet" />
    ) : (
      <div className="divide-y divide-[var(--border-subtle)] border-b border-[var(--border)]">
        {configuration.lesson_requirements.map((lesson) => (
          <div className="grid gap-3 py-4 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_110px_auto] sm:items-center" key={lesson.id}>
            <div><p className="text-sm font-semibold text-[var(--text-strong)]">{nameFor(configuration.classes, lesson.class_id)} · {nameFor(configuration.subjects, lesson.subject_id)}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{nameFor(configuration.teachers, lesson.teacher_id)}{lesson.room_id ? ` · ${nameFor(configuration.rooms, lesson.room_id)}` : " · Any room"}</p></div>
            <p className="text-sm text-[var(--text-muted)]">{lesson.periods_per_cycle} periods per cycle</p>
            <span className="w-fit rounded-full bg-[var(--brand-soft)] px-2.5 py-1 text-xs font-semibold text-[var(--brand-strong)]">Hard constraints</span>
            {canEdit ? <div className="flex gap-1 sm:justify-end"><Button aria-label="Edit requirement" onClick={() => onEdit(lesson)} size="icon-sm" variant="ghost"><Edit3 className="size-3.5" /></Button><Button aria-label="Remove requirement" onClick={() => onRemove(lesson.id)} size="icon-sm" variant="ghost"><Trash2 className="size-3.5" /></Button></div> : null}
          </div>
        ))}
      </div>
    )}
  </section>
);

const RunReview: React.FC<{ canPublish: boolean; isPublishing: boolean; onPublish: () => void; run: TimetableRun | null; selectedClassId: string | null; setSelectedClassId: (id: string | null) => void }> = ({ canPublish, isPublishing, onPublish, run, selectedClassId, setSelectedClassId }) => {
  if (!run) return <StateMessage description="Complete the timetable setup, then generate a draft." title="No timetable draft yet" />;
  const config = run.configuration;
  const selectedClass = selectedClassId ?? config.classes[0]?.id ?? null;
  return (
    <div className="space-y-6">
      <section className="flex flex-col gap-5 bg-[var(--surface-muted)] p-5 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <div className="flex flex-wrap items-center gap-2"><span className={`rounded-full px-2.5 py-1 text-xs font-semibold ${run.status === "published" ? "bg-[var(--tone-success-soft)] text-[var(--tone-success)]" : "bg-[var(--brand-soft)] text-[var(--brand-strong)]"}`}>{run.status}</span><span className="text-xs text-[var(--text-muted)]">Generated {new Date(run.created_at).toLocaleString()}</span></div>
          <p className="mt-2 text-sm text-[var(--text-body)]">{run.entries.length} placed periods · {run.unresolved.length} unresolved · quality score {run.quality_score}</p>
        </div>
        {canPublish && run.status !== "published" ? <Button disabled={run.unresolved.length > 0 || isPublishing} onClick={onPublish}>{isPublishing ? <Loader2 className="size-4 animate-spin" /> : <CheckCircle2 className="size-4" />}Publish timetable</Button> : null}
      </section>
      {run.unresolved.length > 0 ? <div className="flex items-start gap-3 border border-[var(--tone-warning)] bg-[var(--tone-warning-soft)] p-4"><AlertTriangle className="mt-0.5 size-5 shrink-0 text-[var(--tone-warning-strong)]" /><div><p className="text-sm font-semibold text-[var(--text-strong)]">{run.unresolved.length} lessons could not be placed</p><p className="mt-1 text-sm text-[var(--text-muted)]">Adjust availability, teaching load, rooms, or the number of periods before publishing.</p></div></div> : null}
      <div className="max-w-sm"><Label>Class timetable</Label><div className="mt-2"><StringCombobox options={config.classes} placeholder="Choose a class" value={selectedClass} onChange={setSelectedClassId} /></div></div>
      {selectedClass ? <ScheduleGrid classId={selectedClass} run={run} /> : <StateMessage description="Choose a class to inspect its generated timetable." title="Select a class" />}
    </div>
  );
};

const ScheduleGrid: React.FC<{ classId: string; run: TimetableRun }> = ({ classId, run }) => {
  const { configuration: config } = run;
  return (
    <div className="overflow-x-auto border border-[var(--border)] bg-[var(--surface)]">
      <table className="w-full min-w-[820px] border-collapse text-left text-sm">
        <thead><tr className="bg-[var(--surface-muted)]"><th className="w-32 border-b border-r border-[var(--border)] px-4 py-3 font-semibold text-[var(--text-strong)]">Period</th>{config.days.map((day) => <th className="border-b border-[var(--border)] px-4 py-3 font-semibold text-[var(--text-strong)]" key={day.key}>{day.label}</th>)}</tr></thead>
        <tbody>{config.periods.map((period) => <tr key={period.key}><th className="border-r border-t border-[var(--border-subtle)] px-4 py-4 align-top font-medium text-[var(--text-muted)]">{period.label}</th>{config.days.map((day) => {
          const entry = run.entries.find((item) => item.class_id === classId && item.day_key === day.key && item.period_key === period.key);
          return <td className="border-t border-[var(--border-subtle)] px-4 py-3 align-top" key={day.key}>{entry ? <div><p className="font-semibold text-[var(--text-strong)]">{nameFor(config.subjects, entry.subject_id)}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{nameFor(config.teachers, entry.teacher_id)}{entry.room_id ? ` · ${nameFor(config.rooms, entry.room_id)}` : ""}</p></div> : <span className="text-xs text-[var(--text-subtle)]">Available</span>}</td>;
        })}</tr>)}</tbody>
      </table>
    </div>
  );
};

const WeekDrawer: React.FC<{ configuration: TimetableConfiguration; onClose: () => void; onSave: (next: TimetableConfiguration) => void; open: boolean }> = ({ configuration, onClose, onSave, open }) => {
  const [days, setDays] = useState(configuration.days.map((day) => day.label));
  const [periodCount, setPeriodCount] = useState(configuration.periods.length);
  useEffect(() => { if (open) { setDays(configuration.days.map((day) => day.label)); setPeriodCount(configuration.periods.length); } }, [configuration, open]);
  const submit = () => {
    if (days.length === 0) return toast.error("Select at least one teaching day");
    const nextPeriods = Array.from({ length: periodCount }, (_, index) => configuration.periods[index] ?? { key: `period-${index + 1}`, label: `Period ${index + 1}`, start_time: null, end_time: null });
    onSave({ ...configuration, days: standardDays.filter((day) => days.includes(day)).map((label) => ({ key: label.toLowerCase(), label })), periods: nextPeriods });
    onClose();
  };
  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={onClose} title="Teaching week" /><DialogBody><p className="text-sm leading-6 text-[var(--text-muted)]">Choose the active teaching days and the number of schedulable periods on each day.</p><fieldset className="mt-6"><legend className="text-sm font-semibold text-[var(--text-strong)]">Teaching days</legend><div className="mt-3 grid grid-cols-2 gap-2">{standardDays.map((day) => <label className="flex items-center gap-3 border border-[var(--border)] p-3 text-sm text-[var(--text-body)]" key={day}><input checked={days.includes(day)} onChange={(event) => setDays((current) => event.target.checked ? [...current, day] : current.filter((item) => item !== day))} type="checkbox" />{day}</label>)}</div></fieldset><div className="mt-6"><Label htmlFor="period-count">Periods per day</Label><Input className="mt-2" id="period-count" max={16} min={1} onChange={(event) => setPeriodCount(Math.max(1, Math.min(16, Number(event.target.value))))} type="number" value={periodCount} /></div></DialogBody><DialogFooter><Button onClick={onClose} variant="secondary">Cancel</Button><Button onClick={submit}>Apply week</Button></DialogFooter></DialogShell>;
};

const RegistryDrawer: React.FC<{ configuration: TimetableConfiguration; drawer: Extract<DrawerState, { kind: "registry" }> | null; onClose: () => void; onSave: (next: TimetableConfiguration) => void }> = ({ configuration, drawer, onClose, onSave }) => {
  const [name, setName] = useState("");
  const [unavailable, setUnavailable] = useState<string[]>([]);
  useEffect(() => { if (drawer) { setName(drawer.item?.name ?? ""); setUnavailable(drawer.item && "unavailable_slots" in drawer.item ? drawer.item.unavailable_slots : []); } }, [drawer]);
  if (!drawer) return null;
  const { registry } = drawer;
  const submit = () => {
    if (!name.trim()) return toast.error(`Enter a ${registryLabels[registry].singular} name`);
    const id = drawer.item?.id ?? `${registry.slice(0, -1)}-${crypto.randomUUID()}`;
    const item = registry === "teachers" ? { id, name: name.trim(), unavailable_slots: unavailable } : { id, name: name.trim() };
    const current = configuration[registry] as Array<NamedResource | TeacherResource>;
    const nextItems = drawer.item ? current.map((entry) => entry.id === id ? item : entry) : [...current, item];
    onSave({ ...configuration, [registry]: nextItems });
    onClose();
  };
  return <DialogShell onClose={onClose} open><DialogHeader onClose={onClose} title={`${drawer.item ? "Edit" : "Add"} ${registryLabels[registry].singular}`} /><DialogBody><Label htmlFor="registry-name">Name</Label><Input className="mt-2" data-autofocus="true" id="registry-name" onChange={(event) => setName(event.target.value)} placeholder={`e.g. ${registry === "classes" ? "Grade 7A" : registry === "subjects" ? "Mathematics" : registry === "teachers" ? "T. Moyo" : "Science Lab"}`} value={name} />{registry === "teachers" ? <div className="mt-7"><p className="text-sm font-semibold text-[var(--text-strong)]">Unavailable teaching slots</p><p className="mt-1 text-xs leading-5 text-[var(--text-muted)]">Selected slots become hard constraints during generation.</p><div className="mt-4 overflow-x-auto"><div className="grid min-w-[520px] gap-2" style={{ gridTemplateColumns: `110px repeat(${configuration.days.length}, minmax(72px, 1fr))` }}><span />{configuration.days.map((day) => <span className="text-center text-xs font-semibold text-[var(--text-muted)]" key={day.key}>{day.label.slice(0, 3)}</span>)}{configuration.periods.map((period) => <React.Fragment key={period.key}><span className="self-center text-xs text-[var(--text-muted)]">{period.label}</span>{configuration.days.map((day) => { const slot = `${day.key}:${period.key}`; return <button aria-pressed={unavailable.includes(slot)} className={`h-9 border text-xs ${unavailable.includes(slot) ? "border-[var(--brand)] bg-[var(--brand-soft)] text-[var(--brand-strong)]" : "border-[var(--border)] text-[var(--text-subtle)]"}`} key={day.key} onClick={() => setUnavailable((current) => current.includes(slot) ? current.filter((item) => item !== slot) : [...current, slot])} type="button">{unavailable.includes(slot) ? "Away" : "Free"}</button>; })}</React.Fragment>)}</div></div></div> : null}</DialogBody><DialogFooter><Button onClick={onClose} variant="secondary">Cancel</Button><Button onClick={submit}>{drawer.item ? "Save changes" : `Add ${registryLabels[registry].singular}`}</Button></DialogFooter></DialogShell>;
};

const LessonDrawer: React.FC<{ configuration: TimetableConfiguration; item?: LessonRequirement; onClose: () => void; onSave: (next: TimetableConfiguration) => void; open: boolean }> = ({ configuration, item, onClose, onSave, open }) => {
  const [draft, setDraft] = useState<Omit<LessonRequirement, "id">>({ class_id: "", subject_id: "", teacher_id: "", room_id: null, periods_per_cycle: 1 });
  useEffect(() => { if (open) setDraft(item ? { class_id: item.class_id, subject_id: item.subject_id, teacher_id: item.teacher_id, room_id: item.room_id, periods_per_cycle: item.periods_per_cycle } : { class_id: "", subject_id: "", teacher_id: "", room_id: null, periods_per_cycle: 1 }); }, [item, open]);
  const submit = () => {
    if (!draft.class_id || !draft.subject_id || !draft.teacher_id) return toast.error("Choose a class, subject, and teacher");
    const lesson: LessonRequirement = { ...draft, id: item?.id ?? `lesson-${crypto.randomUUID()}` };
    onSave({ ...configuration, lesson_requirements: item ? configuration.lesson_requirements.map((entry) => entry.id === item.id ? lesson : entry) : [...configuration.lesson_requirements, lesson] });
    onClose();
  };
  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={onClose} title={item ? "Edit teaching requirement" : "Add teaching requirement"} /><DialogBody><p className="text-sm leading-6 text-[var(--text-muted)]">Define the weekly teaching load. Campus Pilot will place each requested period around hard resource constraints.</p><div className="mt-6 space-y-5"><ComboboxField label="Class" options={configuration.classes} value={draft.class_id || null} onChange={(value) => setDraft({ ...draft, class_id: value ?? "" })} /><ComboboxField label="Subject" options={configuration.subjects} value={draft.subject_id || null} onChange={(value) => setDraft({ ...draft, subject_id: value ?? "" })} /><ComboboxField label="Teacher" options={configuration.teachers} value={draft.teacher_id || null} onChange={(value) => setDraft({ ...draft, teacher_id: value ?? "" })} /><ComboboxField allowClear label="Preferred room" options={configuration.rooms} value={draft.room_id} onChange={(value) => setDraft({ ...draft, room_id: value })} /><div><Label htmlFor="periods-per-cycle">Periods per cycle</Label><Input className="mt-2" id="periods-per-cycle" max={40} min={1} onChange={(event) => setDraft({ ...draft, periods_per_cycle: Math.max(1, Math.min(40, Number(event.target.value))) })} type="number" value={draft.periods_per_cycle} /></div></div></DialogBody><DialogFooter><Button onClick={onClose} variant="secondary">Cancel</Button><Button onClick={submit}>{item ? "Save changes" : "Add requirement"}</Button></DialogFooter></DialogShell>;
};

const ComboboxField: React.FC<{ allowClear?: boolean; label: string; onChange: (value: string | null) => void; options: NamedResource[]; value: string | null }> = ({ allowClear, label, onChange, options, value }) => <div><Label>{label}</Label><div className="mt-2"><StringCombobox allowClear={allowClear} onChange={onChange} options={options} placeholder={`Choose ${label.toLowerCase()}`} value={value} /></div></div>;

const StringCombobox: React.FC<{ allowClear?: boolean; onChange: (value: string | null) => void; options: NamedResource[]; placeholder: string; value: string | null }> = ({ allowClear = false, onChange, options, placeholder, value }) => {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const root = useRef<HTMLDivElement>(null);
  const selected = options.find((option) => option.id === value);
  const filtered = options.filter((option) => option.name.toLowerCase().includes(query.toLowerCase()));
  useEffect(() => { const close = (event: MouseEvent) => { if (root.current && !root.current.contains(event.target as Node)) setOpen(false); }; document.addEventListener("mousedown", close); return () => document.removeEventListener("mousedown", close); }, []);
  return <div className="relative" ref={root}><button aria-expanded={open} className="flex h-[var(--h-control-md)] w-full items-center justify-between rounded-[var(--radius-md)] border border-[var(--input-border)] bg-[var(--input-bg)] px-3 text-left text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]" onClick={() => setOpen((current) => !current)} type="button"><span className={selected ? "text-[var(--text-strong)]" : "text-[var(--text-subtle)]"}>{selected?.name ?? placeholder}</span><ChevronDown className="size-4 text-[var(--text-muted)]" /></button>{open ? <div className="absolute z-50 mt-1 w-full overflow-hidden rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-popover)]"><div className="relative border-b border-[var(--border)] p-2"><Search className="absolute left-5 top-1/2 size-4 -translate-y-1/2 text-[var(--text-muted)]" /><input autoFocus className="h-9 w-full rounded-[var(--radius-md)] border border-[var(--input-border)] bg-[var(--input-bg)] pl-9 pr-3 text-sm outline-none focus:ring-2 focus:ring-[var(--focus-ring)]" onChange={(event) => setQuery(event.target.value)} placeholder="Search…" value={query} /></div><div className="max-h-56 overflow-y-auto">{allowClear ? <button className="w-full px-3 py-2 text-left text-sm text-[var(--text-muted)] hover:bg-[var(--surface-muted)]" onClick={() => { onChange(null); setOpen(false); setQuery(""); }} type="button">No preferred room</button> : null}{filtered.map((option) => <button className={`flex w-full items-center justify-between px-3 py-2 text-left text-sm hover:bg-[var(--surface-muted)] ${option.id === value ? "bg-[var(--brand-soft)] text-[var(--brand-strong)]" : "text-[var(--text-strong)]"}`} key={option.id} onClick={() => { onChange(option.id); setOpen(false); setQuery(""); }} type="button">{option.name}{option.id === value ? <Check className="size-4" /> : null}</button>)}{filtered.length === 0 ? <p className="px-3 py-4 text-sm text-[var(--text-muted)]">No matches</p> : null}</div></div> : null}</div>;
};

const StateMessage: React.FC<{ action?: React.ReactNode; description: string; title: string }> = ({ action, description, title }) => <div className="border border-dashed border-[var(--border-strong)] bg-[var(--surface-muted)] px-6 py-12 text-center"><CalendarClock className="mx-auto size-8 text-[var(--brand-strong)]" /><h2 className="mt-4 text-lg font-semibold text-[var(--text-strong)]">{title}</h2><p className="mx-auto mt-2 max-w-xl text-sm leading-6 text-[var(--text-muted)]">{description}</p>{action ? <div className="mt-5">{action}</div> : null}</div>;

function nameFor(items: NamedResource[], id: string) {
  return items.find((item) => item.id === id)?.name ?? "Unknown";
}

function removeRegistryItem(configuration: TimetableConfiguration, registry: RegistryKind, id: string, update: (next: TimetableConfiguration) => void) {
  const referenced = configuration.lesson_requirements.some((lesson) =>
    (registry === "classes" && lesson.class_id === id)
    || (registry === "subjects" && lesson.subject_id === id)
    || (registry === "teachers" && lesson.teacher_id === id)
    || (registry === "rooms" && lesson.room_id === id),
  );
  if (referenced) return toast.error(`Remove teaching requirements that use this ${registryLabels[registry].singular} first`);
  update({ ...configuration, [registry]: configuration[registry].filter((item) => item.id !== id) });
}
