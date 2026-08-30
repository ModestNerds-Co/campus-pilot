import { useCallback, useEffect, useRef, useState } from "react";
import { Link } from "@tanstack/react-router";
import { ArrowLeft, BookOpen, CalendarRange, GraduationCap, Loader2, School, UserRound } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { responseMessage as sisResponseMessage, sisService } from "@/modules/sis/service";
import type { Enrolment } from "@/modules/sis/types";
import { useAuthStore } from "@/stores/auth-store";

import { academicsService, responseMessage } from "./service";
import type { ClassGroup, TeachingAssignment } from "./types";

interface RelatedErrors {
  assignments: string | null;
  roster: string | null;
}

const noRelatedErrors: RelatedErrors = { assignments: null, roster: null };

export function AcademicClassRecord({ classId }: { classId: string }) {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const modules = useAuthStore((state) => state.user?.modules ?? []);
  const canViewSis = modules.includes("sis") && (permissions.includes("*") || permissions.includes("sis:view"));
  const [classGroup, setClassGroup] = useState<ClassGroup | null>(null);
  const [assignments, setAssignments] = useState<TeachingAssignment[]>([]);
  const [enrolments, setEnrolments] = useState<Enrolment[]>([]);
  const [loading, setLoading] = useState(true);
  const [relatedLoading, setRelatedLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notFound, setNotFound] = useState(false);
  const [relatedErrors, setRelatedErrors] = useState<RelatedErrors>(noRelatedErrors);
  const requestVersion = useRef(0);

  const load = useCallback(async () => {
    const version = ++requestVersion.current;
    setLoading(true);
    setError(null);
    setNotFound(false);
    setRelatedErrors(noRelatedErrors);
    try {
      const classResponse = await academicsService.getClass(classId);
      if (version !== requestVersion.current) return;
      if (!classResponse.success || !classResponse.data) {
        const message = responseMessage(classResponse, "The class could not be loaded");
        if (message.toLowerCase().includes("not found")) setNotFound(true);
        else setError(message);
        setClassGroup(null);
        setLoading(false);
        return;
      }

      setClassGroup(classResponse.data);
      setLoading(false);
      setRelatedLoading(true);
      const [assignmentResult, rosterResult] = await Promise.allSettled([
        academicsService.listTeachingAssignments({ class_group_id: classId, per_page: 100 }),
        canViewSis ? sisService.listEnrolments({ class_group_id: classId, per_page: 100 }) : Promise.resolve(null),
      ]);
      if (version !== requestVersion.current) return;

      const nextErrors = { ...noRelatedErrors };
      if (assignmentResult.status === "fulfilled" && assignmentResult.value.success && assignmentResult.value.data) {
        setAssignments(assignmentResult.value.data.assignments);
      } else {
        setAssignments([]);
        nextErrors.assignments = assignmentResult.status === "rejected" ? errorMessage(assignmentResult.reason, "Teaching assignments could not be loaded") : responseMessage(assignmentResult.value, "Teaching assignments could not be loaded");
      }

      if (!canViewSis) {
        setEnrolments([]);
      } else if (rosterResult.status === "fulfilled" && rosterResult.value?.success && rosterResult.value.data) {
        setEnrolments(rosterResult.value.data.enrolments);
      } else {
        setEnrolments([]);
        nextErrors.roster = rosterResult.status === "rejected"
          ? errorMessage(rosterResult.reason, "The class roster could not be loaded")
          : rosterResult.value
            ? sisResponseMessage(rosterResult.value, "The class roster could not be loaded")
            : "The class roster could not be loaded";
      }
      setRelatedErrors(nextErrors);
      setRelatedLoading(false);
    } catch (loadError) {
      if (version !== requestVersion.current) return;
      setClassGroup(null);
      setError(errorMessage(loadError, "The class could not be loaded"));
      setLoading(false);
      setRelatedLoading(false);
    }
  }, [canViewSis, classId]);

  useEffect(() => {
    void load();
    return () => { requestVersion.current += 1; };
  }, [load]);

  usePageChrome(classGroup?.name ?? "Class record", null);

  if (loading) return <RecordLoading />;
  if (notFound) return <RecordUnavailable description="This class does not exist or is no longer available." title="Class not found" />;
  if (error || !classGroup) return <RecordUnavailable description={error ?? "The class could not be loaded."} onRetry={() => void load()} title="Class unavailable" />;

  const activeLearners = enrolments.filter((item) => item.status === "active");

  return (
    <div className="space-y-6">
      <Link className="inline-flex items-center gap-2 text-sm font-medium text-[var(--text-muted)] hover:text-[var(--brand-strong)]" to="/modules/academics/classes"><ArrowLeft className="size-4" />Classes</Link>

      <section aria-labelledby="class-record-title" className="border border-[var(--border)] bg-[var(--surface)]">
        <div className="flex flex-col gap-5 border-b border-[var(--border)] p-5 sm:flex-row sm:items-start sm:justify-between sm:p-6">
          <div className="flex min-w-0 items-start gap-4">
            <span className="flex size-12 shrink-0 items-center justify-center rounded-[var(--radius-lg)] bg-[var(--brand-soft)] text-[var(--brand-strong)]"><School className="size-6" /></span>
            <div className="min-w-0">
              <p className="font-tabular text-xs font-semibold uppercase tracking-[0.12em] text-[var(--text-subtle)]">{classGroup.code}</p>
              <h1 className="mt-1 break-words text-2xl font-semibold tracking-[-0.03em] text-[var(--text-strong)]" id="class-record-title">{classGroup.name}</h1>
              <p className="mt-1 text-sm text-[var(--text-muted)]">{classGroup.academic_year_name}</p>
            </div>
          </div>
          <Badge className="self-start" tone={classGroup.status === "active" ? "success" : "neutral"}>{displayStatus(classGroup.status)}</Badge>
        </div>
        <dl className="grid sm:grid-cols-3">
          <RecordField icon={<CalendarRange />} label="Academic year" value={classGroup.academic_year_name} />
          <RecordField icon={<GraduationCap />} label="Grade level" value={classGroup.grade_level || "Not assigned"} />
          <RecordField icon={<BookOpen />} label="Teaching assignments" value={relatedLoading ? "Loading…" : String(assignments.length)} />
        </dl>
      </section>

      <div className="grid gap-6 xl:grid-cols-2">
        <RecordSection actionLabel="Manage enrolments" actionTo="/modules/sis/enrolments" count={canViewSis ? activeLearners.length : null} icon={<UserRound />} showAction={canViewSis} title="Current learners">
          {!canViewSis ? <SectionEmpty description="SIS access is required to view the class roster." /> : relatedLoading ? <SectionLoading label="Loading class roster…" /> : relatedErrors.roster ? <SectionError message={relatedErrors.roster} onRetry={() => void load()} /> : activeLearners.length === 0 ? <SectionEmpty description="No active learners are enrolled in this class." /> : <div className="divide-y divide-[var(--border-subtle)]">{activeLearners.map((enrolment) => <article className="flex items-start justify-between gap-4 p-4 sm:p-5" key={enrolment.id}><div><Link className="font-medium text-[var(--text-strong)] hover:text-[var(--brand-strong)] hover:underline" params={{ learnerId: enrolment.learner_id }} to="/modules/sis/learners/$learnerId">{enrolment.learner_name}</Link><p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{enrolment.learner_number}</p></div><p className="text-xs text-[var(--text-muted)]">Since {formatDate(enrolment.starts_on)}</p></article>)}</div>}
        </RecordSection>

        <RecordSection actionLabel="Manage assignments" actionTo="/modules/academics/teaching-assignments" count={assignments.length} icon={<BookOpen />} showAction title="Teaching assignments">
          {relatedLoading ? <SectionLoading label="Loading teaching assignments…" /> : relatedErrors.assignments ? <SectionError message={relatedErrors.assignments} onRetry={() => void load()} /> : assignments.length === 0 ? <SectionEmpty description="No teacher and subject assignment is recorded for this class." /> : <div className="divide-y divide-[var(--border-subtle)]">{assignments.map((assignment) => <article className="p-4 sm:p-5" key={assignment.id}><div className="flex items-start justify-between gap-3"><div><p className="font-medium text-[var(--text-strong)]">{assignment.subject_name}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{assignment.teacher_name}</p></div><Badge tone={assignment.status === "active" ? "success" : "neutral"}>{displayStatus(assignment.status)}</Badge></div><p className="mt-3 text-xs text-[var(--text-muted)]">{assignment.periods_per_cycle} periods per cycle</p></article>)}</div>}
        </RecordSection>
      </div>
    </div>
  );
}

function RecordSection({ actionLabel, actionTo, children, count, icon, showAction, title }: { actionLabel: string; actionTo: "/modules/sis/enrolments" | "/modules/academics/teaching-assignments"; children: React.ReactNode; count: number | null; icon: React.ReactNode; showAction: boolean; title: string }) {
  return <section className="border border-[var(--border)] bg-[var(--surface)]"><div className="flex flex-wrap items-center justify-between gap-3 border-b border-[var(--border)] p-5 sm:p-6"><div className="flex items-center gap-3"><span className="text-[var(--brand-strong)] [&_svg]:size-5">{icon}</span><h2 className="text-base font-semibold text-[var(--text-strong)]">{title}</h2>{count !== null ? <Badge tone="neutral">{count}</Badge> : null}</div>{showAction ? <Link className="text-xs font-semibold text-[var(--brand-strong)] hover:underline" to={actionTo}>{actionLabel}</Link> : null}</div>{children}</section>;
}

function RecordField({ icon, label, value }: { icon: React.ReactNode; label: string; value: string }) {
  return <div className="border-b border-[var(--border)] p-5 last:border-b-0 sm:border-b-0 sm:border-r sm:last:border-r-0"><dt className="flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.12em] text-[var(--text-subtle)]"><span className="[&_svg]:size-3.5">{icon}</span>{label}</dt><dd className="mt-2 text-sm text-[var(--text-strong)]">{value}</dd></div>;
}

function RecordLoading() { return <div aria-label="Loading class" className="flex min-h-64 items-center justify-center border border-[var(--border)] bg-[var(--surface)]" role="status"><Loader2 className="size-6 animate-spin text-[var(--brand-strong)]" /></div>; }
function SectionLoading({ label }: { label: string }) { return <div aria-label={label} className="flex min-h-28 items-center justify-center" role="status"><Loader2 className="size-5 animate-spin text-[var(--brand-strong)]" /></div>; }
function SectionEmpty({ description }: { description: string }) { return <p className="p-5 text-sm text-[var(--text-muted)] sm:p-6">{description}</p>; }
function SectionError({ message, onRetry }: { message: string; onRetry: () => void }) { return <div className="p-5 sm:p-6"><p className="text-sm text-[var(--tone-danger)]">{message}</p><Button className="mt-4" onClick={onRetry} size="sm" variant="secondary">Retry</Button></div>; }
function RecordUnavailable({ description, onRetry, title }: { description: string; onRetry?: () => void; title: string }) { return <div className="border border-[var(--border)] bg-[var(--surface)] p-8 text-center"><School className="mx-auto size-8 text-[var(--text-subtle)]" /><h1 className="mt-4 text-lg font-semibold text-[var(--text-strong)]">{title}</h1><p className="mx-auto mt-2 max-w-lg text-sm text-[var(--text-muted)]">{description}</p>{onRetry ? <Button className="mt-5" onClick={onRetry} variant="secondary">Retry</Button> : null}</div>; }
function errorMessage(error: unknown, fallback: string) { return error instanceof Error ? error.message : fallback; }
function displayStatus(value: string) { return value.replace(/_/g, " ").replace(/^./, (letter) => letter.toUpperCase()); }
function formatDate(value: string) { return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`)); }
