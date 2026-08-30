import { useCallback, useEffect, useRef, useState } from "react";
import { Link } from "@tanstack/react-router";
import { ArrowLeft, ArrowRight, CalendarDays, ClipboardCheck, Edit, GraduationCap, Loader2, School, UserRound } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { ApplicationDrawer } from "./applications-list";
import { EnrolmentDrawer } from "./enrolments-list";
import { responseMessage, sisService } from "./service";
import type { Application, ApplicationStatus, Enrolment } from "./types";

interface TransitionOption {
  status: ApplicationStatus;
  label: string;
  title: string;
  description: string;
  destructive?: boolean;
}

export function ApplicationRecord({ applicationId }: { applicationId: string }) {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canCreate = permissions.includes("*") || permissions.includes("sis:create");
  const canEdit = permissions.includes("*") || permissions.includes("sis:edit");
  const [application, setApplication] = useState<Application | null>(null);
  const [enrolments, setEnrolments] = useState<Enrolment[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notFound, setNotFound] = useState(false);
  const [editOpen, setEditOpen] = useState(false);
  const [enrolmentOpen, setEnrolmentOpen] = useState(false);
  const [transition, setTransition] = useState<TransitionOption | null>(null);
  const [transitioning, setTransitioning] = useState(false);
  const requestVersion = useRef(0);

  const load = useCallback(async () => {
    const version = ++requestVersion.current;
    setLoading(true);
    setError(null);
    setNotFound(false);
    try {
      const response = await sisService.getApplication(applicationId);
      if (version !== requestVersion.current) return;
      if (!response.success || !response.data) {
        const message = responseMessage(response, "The application could not be loaded");
        if (message.toLowerCase().includes("not found")) setNotFound(true);
        else setError(message);
        setApplication(null);
        setLoading(false);
        return;
      }
      setApplication(response.data);
      const enrolmentResponse = await sisService.listEnrolments({ learner_id: response.data.learner_id, academic_year_id: response.data.academic_year_id, per_page: 100 });
      if (version !== requestVersion.current) return;
      setEnrolments(enrolmentResponse.success && enrolmentResponse.data ? enrolmentResponse.data.enrolments.filter((item) => item.source_application_id === response.data?.id) : []);
      setLoading(false);
    } catch (loadError) {
      if (version !== requestVersion.current) return;
      setApplication(null);
      setError(loadError instanceof Error ? loadError.message : "The application could not be loaded");
      setLoading(false);
    }
  }, [applicationId]);

  useEffect(() => {
    void load();
    return () => { requestVersion.current += 1; };
  }, [load]);

  usePageChrome(
    application?.application_number ?? "Application",
    application && canEdit ? <Button onClick={() => setEditOpen(true)} variant="secondary"><Edit className="size-4" />Edit details</Button> : null,
  );

  const changeStatus = async () => {
    if (!application || !transition || transitioning) return;
    setTransitioning(true);
    const response = await sisService.updateApplication(application.id, {
      application_number: application.application_number,
      learner_id: application.learner_id,
      academic_year_id: application.academic_year_id,
      target_grade_level_id: application.target_grade_level_id ?? "",
      submitted_on: transition.status === "submitted" && !application.submitted_on ? today() : application.submitted_on,
      status: transition.status,
      notes: application.notes,
    });
    setTransitioning(false);
    if (!response.success) return toast.error(responseMessage(response, "Application status could not be changed"));
    toast.success(`Application marked ${displayStatus(transition.status).toLowerCase()}`);
    setTransition(null);
    await load();
  };

  if (loading) return <RecordLoading />;
  if (notFound) return <RecordUnavailable description="This application does not exist or is no longer available." title="Application not found" />;
  if (error || !application) return <RecordUnavailable description={error ?? "The application could not be loaded."} onRetry={() => void load()} title="Application unavailable" />;

  const transitions = nextTransitions(application.status);
  const sourceEnrolment = enrolments[0] ?? null;

  return (
    <div className="space-y-6">
      <Link className="inline-flex items-center gap-2 text-sm font-medium text-[var(--text-muted)] hover:text-[var(--brand-strong)]" to="/modules/sis/applications"><ArrowLeft className="size-4" />Applications</Link>

      <section aria-labelledby="application-record-title" className="border border-[var(--border)] bg-[var(--surface)]">
        <div className="flex flex-col gap-5 border-b border-[var(--border)] p-5 sm:flex-row sm:items-start sm:justify-between sm:p-6">
          <div className="flex min-w-0 items-start gap-4">
            <span className="flex size-12 shrink-0 items-center justify-center rounded-[var(--radius-lg)] bg-[var(--brand-soft)] text-[var(--brand-strong)]"><ClipboardCheck className="size-6" /></span>
            <div className="min-w-0">
              <p className="text-xs font-semibold uppercase tracking-[0.12em] text-[var(--text-subtle)]">Admission application</p>
              <h1 className="mt-1 break-words font-tabular text-2xl font-semibold tracking-[-0.03em] text-[var(--text-strong)]" id="application-record-title">{application.application_number}</h1>
              <p className="mt-1 text-sm text-[var(--text-muted)]">{application.academic_year_name} · {application.target_grade_level_name || "Grade not recorded"}</p>
            </div>
          </div>
          <Badge className="self-start" tone={applicationTone(application.status)}>{displayStatus(application.status)}</Badge>
        </div>
        <dl className="grid sm:grid-cols-2 xl:grid-cols-4">
          <RecordField icon={<UserRound />} label="Learner"><Link className="font-medium text-[var(--text-strong)] hover:text-[var(--brand-strong)] hover:underline" params={{ learnerId: application.learner_id }} to="/modules/sis/learners/$learnerId">{application.learner_name}</Link><p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{application.learner_number}</p></RecordField>
          <RecordField icon={<CalendarDays />} label="Submitted"><span>{application.submitted_on ? formatDate(application.submitted_on) : "Not submitted"}</span></RecordField>
          <RecordField icon={<GraduationCap />} label="Target grade"><span>{application.target_grade_level_name || "Not recorded"}</span></RecordField>
          <RecordField icon={<School />} label="Enrolment"><span>{sourceEnrolment ? `${sourceEnrolment.class_group_name} · ${displayStatus(sourceEnrolment.status)}` : "Not enrolled"}</span></RecordField>
        </dl>
      </section>

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1.25fr)_minmax(20rem,0.75fr)]">
        <section className="border border-[var(--border)] bg-[var(--surface)]" aria-labelledby="application-progress-title">
          <div className="border-b border-[var(--border)] p-5 sm:p-6">
            <h2 className="text-base font-semibold text-[var(--text-strong)]" id="application-progress-title">Application progress</h2>
            <p className="mt-1 text-sm text-[var(--text-muted)]">Move the application through the next valid admissions step.</p>
          </div>
          <div className="p-5 sm:p-6">
            {canEdit && transitions.length > 0 ? <div className="flex flex-wrap gap-2">{transitions.map((option, index) => <Button key={option.status} onClick={() => setTransition(option)} variant={option.destructive ? "outline" : index === 0 ? "default" : "secondary"}>{option.label}<ArrowRight className="size-4" /></Button>)}</div> : null}
            {application.status === "accepted" && canCreate && !sourceEnrolment ? <div className="space-y-3"><p className="text-sm text-[var(--text-muted)]">The accepted application is ready for class placement.</p><Button onClick={() => setEnrolmentOpen(true)}><GraduationCap className="size-4" />Create enrolment</Button></div> : null}
            {sourceEnrolment ? <div className="border border-[var(--border)] bg-[var(--surface-muted)] p-4"><div className="flex items-start justify-between gap-3"><div><p className="font-medium text-[var(--text-strong)]">{sourceEnrolment.class_group_name}</p><p className="mt-1 text-xs text-[var(--text-muted)]">Started {formatDate(sourceEnrolment.starts_on)}</p></div><Badge tone={sourceEnrolment.status === "active" ? "success" : sourceEnrolment.status === "withdrawn" ? "danger" : "neutral"}>{displayStatus(sourceEnrolment.status)}</Badge></div></div> : null}
            {transitions.length === 0 && !sourceEnrolment && application.status !== "accepted" ? <p className="text-sm text-[var(--text-muted)]">No further admissions action is available for this application.</p> : null}
            {!canEdit && transitions.length > 0 ? <p className="text-sm text-[var(--text-muted)]">You have view-only access to this application.</p> : null}
          </div>
        </section>

        <section className="border border-[var(--border)] bg-[var(--surface)]" aria-labelledby="application-notes-title">
          <div className="border-b border-[var(--border)] p-5 sm:p-6"><h2 className="text-base font-semibold text-[var(--text-strong)]" id="application-notes-title">Notes</h2></div>
          <p className="whitespace-pre-wrap p-5 text-sm leading-6 text-[var(--text-muted)] sm:p-6">{application.notes || "No notes recorded."}</p>
        </section>
      </div>

      <ApplicationDrawer onClose={() => setEditOpen(false)} onSaved={() => { setEditOpen(false); void load(); }} open={editOpen} record={application} />
      <EnrolmentDrawer initialApplication={application} onClose={() => setEnrolmentOpen(false)} onSaved={() => { setEnrolmentOpen(false); void load(); }} open={enrolmentOpen} record={null} />
      <TransitionDrawer isPending={transitioning} onClose={() => setTransition(null)} onConfirm={() => void changeStatus()} open={transition !== null} option={transition} />
    </div>
  );
}

function TransitionDrawer({ isPending, onClose, onConfirm, open, option }: { isPending: boolean; onClose: () => void; onConfirm: () => void; open: boolean; option: TransitionOption | null }) {
  return <DialogShell onClose={isPending ? () => undefined : onClose} open={open}><DialogHeader onClose={isPending ? undefined : onClose} title={option?.title ?? "Change application status"} /><DialogBody><p className="text-sm leading-6 text-[var(--text-muted)]">{option?.description}</p></DialogBody><DialogFooter><Button disabled={isPending} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button data-autofocus="true" disabled={isPending} onClick={onConfirm} type="button" variant={option?.destructive ? "destructive" : "default"}>{isPending ? <Loader2 className="size-4 animate-spin" /> : null}{isPending ? "Saving…" : option?.label}</Button></DialogFooter></DialogShell>;
}

function RecordField({ children, icon, label }: { children: React.ReactNode; icon: React.ReactNode; label: string }) {
  return <div className="border-b border-[var(--border)] p-5 last:border-b-0 sm:border-r sm:[&:nth-child(2n)]:border-r-0 xl:border-b-0 xl:[&:nth-child(2n)]:border-r xl:last:border-r-0"><dt className="flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.12em] text-[var(--text-subtle)]"><span className="[&_svg]:size-3.5">{icon}</span>{label}</dt><dd className="mt-2 text-sm text-[var(--text-strong)]">{children}</dd></div>;
}

function RecordLoading() {
  return <div aria-label="Loading application" className="flex min-h-64 items-center justify-center border border-[var(--border)] bg-[var(--surface)]" role="status"><Loader2 className="size-6 animate-spin text-[var(--brand-strong)]" /></div>;
}

function RecordUnavailable({ description, onRetry, title }: { description: string; onRetry?: () => void; title: string }) {
  return <div className="border border-[var(--border)] bg-[var(--surface)] p-8 text-center"><ClipboardCheck className="mx-auto size-8 text-[var(--text-subtle)]" /><h1 className="mt-4 text-lg font-semibold text-[var(--text-strong)]">{title}</h1><p className="mx-auto mt-2 max-w-lg text-sm text-[var(--text-muted)]">{description}</p>{onRetry ? <Button className="mt-5" onClick={onRetry} variant="secondary">Retry</Button> : null}</div>;
}

function nextTransitions(status: ApplicationStatus): TransitionOption[] {
  if (status === "draft") return [{ status: "submitted", label: "Submit application", title: "Submit application?", description: "The learner, academic year, and target grade will be locked after submission." }];
  if (status === "submitted") return [
    { status: "under_review", label: "Start review", title: "Start application review?", description: "Mark this application as under review." },
    rejectOption(),
    withdrawOption(),
  ];
  if (status === "under_review") return [
    { status: "offered", label: "Make offer", title: "Make an offer?", description: "Mark this application as offered to the learner." },
    { status: "accepted", label: "Accept application", title: "Accept application?", description: "Accept this application so the learner can be enrolled." },
    rejectOption(),
    withdrawOption(),
  ];
  if (status === "offered") return [
    { status: "accepted", label: "Accept application", title: "Accept application?", description: "Accept this application so the learner can be enrolled." },
    rejectOption(),
    withdrawOption(),
  ];
  return [];
}

function rejectOption(): TransitionOption { return { status: "rejected", label: "Reject", title: "Reject application?", description: "This closes the admissions workflow for this application.", destructive: true }; }
function withdrawOption(): TransitionOption { return { status: "withdrawn", label: "Withdraw", title: "Withdraw application?", description: "This closes the admissions workflow for this application.", destructive: true }; }
function applicationTone(status: ApplicationStatus): "neutral" | "warning" | "success" | "danger" | "info" { if (status === "accepted") return "success"; if (status === "rejected" || status === "withdrawn") return "danger"; if (status === "submitted" || status === "under_review" || status === "offered") return "info"; return "neutral"; }
function displayStatus(value: string) { return value.replace(/_/g, " ").replace(/^./, (letter) => letter.toUpperCase()); }
function formatDate(value: string) { return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`)); }
function today() { return new Date().toISOString().slice(0, 10); }
