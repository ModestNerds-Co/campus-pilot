/**
 * Learner-centred SIS record.
 *
 * The route parameter is the sole record identity. All school data is loaded
 * from the server on direct entry or refresh; no learner data is persisted in
 * browser storage.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { Link } from "@tanstack/react-router";
import {
  ArrowLeft,
  CalendarDays,
  ClipboardList,
  Edit,
  KeyRound,
  Loader2,
  RefreshCw,
  School,
  UserRound,
  UsersRound,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { SIS_ADMINISTRATION_PERMISSIONS } from "./access";
import { SisAccountDrawer, SisPersonDrawer } from "./people-list";
import { responseMessage, sisService } from "./service";
import type { Application, Enrolment, GuardianRelationship, Learner } from "./types";

type RelatedErrors = {
  applications: string | null;
  enrolments: string | null;
  guardians: string | null;
};

const noRelatedErrors: RelatedErrors = {
  applications: null,
  enrolments: null,
  guardians: null,
};

export function LearnerRecord({ learnerId }: { learnerId: string }) {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canEdit = permissions.includes("*") || permissions.includes("sis:edit");
  const canAccessApplications = permissions.includes("*") || SIS_ADMINISTRATION_PERMISSIONS.some((permission) => permissions.includes(permission));
  const requestVersion = useRef(0);
  const [learner, setLearner] = useState<Learner | null>(null);
  const [relationships, setRelationships] = useState<GuardianRelationship[]>([]);
  const [applications, setApplications] = useState<Application[]>([]);
  const [enrolments, setEnrolments] = useState<Enrolment[]>([]);
  const [loading, setLoading] = useState(true);
  const [relatedLoading, setRelatedLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notFound, setNotFound] = useState(false);
  const [relatedErrors, setRelatedErrors] = useState<RelatedErrors>(noRelatedErrors);
  const [editOpen, setEditOpen] = useState(false);
  const [accountOpen, setAccountOpen] = useState(false);

  const load = useCallback(async () => {
    const version = ++requestVersion.current;
    setLoading(true);
    setError(null);
    setNotFound(false);
    setRelatedErrors(noRelatedErrors);

    try {
      const learnerResponse = await sisService.getLearner(learnerId);
      if (version !== requestVersion.current) return;
      if (!learnerResponse.success || !learnerResponse.data) {
        const message = responseMessage(learnerResponse, "The learner record could not be loaded");
        if (message.toLowerCase().includes("not found")) setNotFound(true);
        else setError(message);
        setLearner(null);
        setLoading(false);
        return;
      }

      setLearner(learnerResponse.data);
      setLoading(false);
      setRelatedLoading(true);

      const [guardianResult, applicationResult, enrolmentResult] = await Promise.allSettled([
        sisService.listGuardianRelationships({ learner_id: learnerId, per_page: 100 }),
        canAccessApplications ? sisService.listApplications({ learner_id: learnerId, per_page: 100 }) : Promise.resolve(null),
        sisService.listEnrolments({ learner_id: learnerId, per_page: 100 }),
      ]);
      if (version !== requestVersion.current) return;

      const nextErrors = { ...noRelatedErrors };
      if (guardianResult.status === "fulfilled" && guardianResult.value.success && guardianResult.value.data) {
        setRelationships(guardianResult.value.data.relationships);
      } else {
        setRelationships([]);
        nextErrors.guardians = relatedFailure(guardianResult, "Guardian relationships could not be loaded");
      }
      if (!canAccessApplications) {
        setApplications([]);
      } else if (applicationResult.status === "fulfilled" && applicationResult.value?.success && applicationResult.value.data) {
        setApplications(applicationResult.value.data.applications);
      } else {
        setApplications([]);
        nextErrors.applications = relatedFailure(applicationResult as PromiseSettledResult<{ success: boolean; message: string | null; issues: Array<string | { detail?: string }> | null }>, "Applications could not be loaded");
      }
      if (enrolmentResult.status === "fulfilled" && enrolmentResult.value.success && enrolmentResult.value.data) {
        setEnrolments(enrolmentResult.value.data.enrolments);
      } else {
        setEnrolments([]);
        nextErrors.enrolments = relatedFailure(enrolmentResult, "Enrolments could not be loaded");
      }
      setRelatedErrors(nextErrors);
      setRelatedLoading(false);
    } catch (loadError) {
      if (version !== requestVersion.current) return;
      setLearner(null);
      setError(loadError instanceof Error ? loadError.message : "The learner record could not be loaded");
      setLoading(false);
      setRelatedLoading(false);
    }
  }, [canAccessApplications, learnerId]);

  useEffect(() => {
    void load();
    return () => { requestVersion.current += 1; };
  }, [load]);

  usePageChrome(
    learner?.display_name ?? "Learner record",
    learner && canEdit ? (
      <div className="flex flex-wrap gap-2">
        <Button onClick={() => setAccountOpen(true)} variant="secondary"><KeyRound className="size-4" />{learner.account_id ? "Change login" : "Link login"}</Button>
        <Button onClick={() => setEditOpen(true)}><Edit className="size-4" />Edit learner</Button>
      </div>
    ) : null,
  );

  if (loading) return <RecordLoading />;
  if (notFound) return <RecordUnavailable description="This learner does not exist or is no longer available." title="Learner not found" />;
  if (error || !learner) return <RecordUnavailable description={error ?? "The learner record could not be loaded."} onRetry={() => void load()} title="Learner record unavailable" />;

  return (
    <div className="space-y-6">
      <Link className="inline-flex items-center gap-2 text-sm font-medium text-[var(--text-muted)] hover:text-[var(--brand-strong)]" to="/modules/sis/learners">
        <ArrowLeft className="size-4" />Learners
      </Link>

      <section className="border border-[var(--border)] bg-[var(--surface)]" aria-labelledby="learner-record-title">
        <div className="flex flex-col gap-5 border-b border-[var(--border)] p-5 sm:flex-row sm:items-start sm:justify-between sm:p-6">
          <div className="flex min-w-0 items-start gap-4">
            <span className="flex size-12 shrink-0 items-center justify-center rounded-[var(--radius-lg)] bg-[var(--brand-soft)] text-[var(--brand-strong)]"><UserRound className="size-6" /></span>
            <div className="min-w-0">
              <p className="font-tabular text-xs font-semibold uppercase tracking-[0.12em] text-[var(--text-subtle)]">{learner.learner_number}</p>
              <h1 className="mt-1 break-words text-2xl font-semibold tracking-[-0.03em] text-[var(--text-strong)]" id="learner-record-title">{learner.display_name}</h1>
              <p className="mt-1 text-sm text-[var(--text-muted)]">Born {formatDate(learner.date_of_birth)}</p>
            </div>
          </div>
          <Badge className="self-start" dot tone={learnerStatusTone(learner.status)}>{displayStatus(learner.status)}</Badge>
        </div>
        <dl className="grid sm:grid-cols-2 xl:grid-cols-4">
          <RecordField label="First names" value={learner.first_names || "—"} />
          <RecordField label="Surname" value={learner.surname || "—"} />
          <RecordField label="Contact" value={learner.email || learner.phone || "Not recorded"} />
          <RecordField label="Login account" value={learner.account_email || "Not linked"} />
        </dl>
      </section>

      <div className="grid gap-6 xl:grid-cols-2">
        <RecordSection actionLabel={canEdit ? "Manage relationships" : "View relationships"} actionTo="/modules/sis/guardian-relationships" count={relationships.length} icon={<UsersRound />} title="Guardians">
          {relatedLoading ? <SectionLoading label="Loading guardians…" /> : relatedErrors.guardians ? <SectionError message={relatedErrors.guardians} onRetry={() => void load()} /> : relationships.length === 0 ? <SectionEmpty description="No guardian relationship is recorded for this learner." /> : (
            <div className="divide-y divide-[var(--border-subtle)]">
              {relationships.map((relationship) => (
                <article className="p-4 sm:p-5" key={relationship.id}>
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div><p className="font-medium text-[var(--text-strong)]">{relationship.guardian_name}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{displayStatus(relationship.relationship_type)}</p></div>
                    <Badge tone={relationship.status === "active" ? "success" : "neutral"}>{displayStatus(relationship.status)}</Badge>
                  </div>
                  <div className="mt-3 flex flex-wrap gap-2 text-xs text-[var(--text-muted)]">
                    {relationship.is_primary ? <span className="border border-[var(--border)] px-2 py-1">Primary contact</span> : null}
                    {relationship.can_collect ? <span className="border border-[var(--border)] px-2 py-1">Can collect</span> : null}
                    {relationship.receives_communications ? <span className="border border-[var(--border)] px-2 py-1">Receives communication</span> : null}
                  </div>
                </article>
              ))}
            </div>
          )}
        </RecordSection>

        {canAccessApplications ? <RecordSection actionLabel="Manage applications" actionTo="/modules/sis/applications" count={applications.length} icon={<ClipboardList />} title="Applications">
          {relatedLoading ? <SectionLoading label="Loading applications…" /> : relatedErrors.applications ? <SectionError message={relatedErrors.applications} onRetry={() => void load()} /> : applications.length === 0 ? <SectionEmpty description="No application is recorded for this learner." /> : (
            <div className="divide-y divide-[var(--border-subtle)]">
              {applications.map((application) => (
                <article className="p-4 sm:p-5" key={application.id}>
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div><p className="font-tabular font-medium text-[var(--text-strong)]">{application.application_number}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{application.academic_year_name} · {application.target_grade_level_name || "Grade not recorded"}</p></div>
                    <Badge tone={applicationStatusTone(application.status)}>{displayStatus(application.status)}</Badge>
                  </div>
                  <p className="mt-3 text-xs text-[var(--text-muted)]">{application.submitted_on ? `Submitted ${formatDate(application.submitted_on)}` : "Not submitted"}</p>
                </article>
              ))}
            </div>
          )}
        </RecordSection> : null}
      </div>

      <RecordSection actionLabel={canEdit ? "Manage enrolments" : "View enrolments"} actionTo="/modules/sis/enrolments" count={enrolments.length} icon={<School />} title="Enrolment history">
        {relatedLoading ? <SectionLoading label="Loading enrolments…" /> : relatedErrors.enrolments ? <SectionError message={relatedErrors.enrolments} onRetry={() => void load()} /> : enrolments.length === 0 ? <SectionEmpty description="No class enrolment is recorded for this learner." /> : (
          <div className="grid gap-px bg-[var(--border-subtle)] md:grid-cols-2 xl:grid-cols-3">
            {enrolments.map((enrolment) => (
              <article className="bg-[var(--surface)] p-4 sm:p-5" key={enrolment.id}>
                <div className="flex items-start justify-between gap-3"><School className="mt-0.5 size-5 text-[var(--brand-strong)]" /><Badge tone={enrolment.status === "active" ? "success" : enrolment.status === "withdrawn" ? "danger" : "neutral"}>{displayStatus(enrolment.status)}</Badge></div>
                <p className="mt-4 font-medium text-[var(--text-strong)]">{enrolment.class_group_name}</p>
                <p className="mt-1 text-xs text-[var(--text-muted)]">{enrolment.academic_year_name}</p>
                <p className="mt-3 flex items-center gap-2 text-xs text-[var(--text-muted)]"><CalendarDays className="size-3.5" />{formatDate(enrolment.starts_on)}{enrolment.ends_on ? ` – ${formatDate(enrolment.ends_on)}` : " – current"}</p>
                {enrolment.application_number ? <p className="mt-2 font-tabular text-xs text-[var(--text-subtle)]">Application {enrolment.application_number}</p> : null}
              </article>
            ))}
          </div>
        )}
      </RecordSection>

      <SisPersonDrawer kind="learner" onClose={() => setEditOpen(false)} onSaved={() => { setEditOpen(false); void load(); }} open={editOpen} record={learner} />
      <SisAccountDrawer kind="learner" onClose={() => setAccountOpen(false)} onSaved={() => { setAccountOpen(false); void load(); }} open={accountOpen} record={learner} />
    </div>
  );
}

function RecordSection({ actionLabel, actionTo, children, count, icon, title }: {
  actionLabel: string;
  actionTo: "/modules/sis/guardian-relationships" | "/modules/sis/applications" | "/modules/sis/enrolments";
  children: React.ReactNode;
  count: number;
  icon: React.ReactNode;
  title: string;
}) {
  return (
    <section className="overflow-hidden border border-[var(--border)] bg-[var(--surface)]">
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-[var(--border)] px-4 py-4 sm:px-5">
        <div className="flex items-center gap-3 text-[var(--text-strong)]"><span className="text-[var(--brand-strong)] [&>svg]:size-5">{icon}</span><h2 className="font-semibold">{title}</h2><span className="font-tabular text-xs text-[var(--text-subtle)]">{count}</span></div>
        <Link className="text-xs font-semibold text-[var(--brand-strong)] hover:underline" to={actionTo}>{actionLabel}</Link>
      </div>
      {children}
    </section>
  );
}

function RecordField({ label, value }: { label: string; value: string }) {
  return <div className="border-b border-[var(--border)] px-5 py-4 last:border-b-0 sm:border-r sm:px-6 sm:[&:nth-child(2n)]:border-r-0 xl:border-b-0 xl:[&:nth-child(2n)]:border-r xl:last:border-r-0"><dt className="text-[11px] font-semibold uppercase tracking-[0.12em] text-[var(--text-subtle)]">{label}</dt><dd className="mt-1.5 break-words text-sm text-[var(--text-strong)]">{value}</dd></div>;
}

function RecordLoading() {
  return <div className="flex min-h-[55vh] items-center justify-center" aria-label="Loading learner record"><Loader2 className="size-6 animate-spin text-[var(--brand-strong)]" /></div>;
}

function RecordUnavailable({ description, onRetry, title }: { description: string; onRetry?: () => void; title: string }) {
  return <div className="mx-auto max-w-xl py-16 text-center"><span className="mx-auto flex size-12 items-center justify-center rounded-[var(--radius-lg)] bg-[var(--surface-muted)] text-[var(--text-muted)]"><UserRound className="size-6" /></span><h1 className="mt-4 text-xl font-semibold text-[var(--text-strong)]">{title}</h1><p className="mt-2 text-sm leading-6 text-[var(--text-muted)]">{description}</p><div className="mt-5 flex flex-wrap justify-center gap-3">{onRetry ? <Button onClick={onRetry} variant="secondary"><RefreshCw className="size-4" />Retry</Button> : null}<Link className={buttonVariants({ variant: "secondary" })} to="/modules/sis/learners">Learners</Link></div></div>;
}

function SectionLoading({ label }: { label: string }) {
  return <div className="flex min-h-28 items-center justify-center gap-2 p-5 text-sm text-[var(--text-muted)]"><Loader2 className="size-4 animate-spin" />{label}</div>;
}

function SectionError({ message, onRetry }: { message: string; onRetry: () => void }) {
  return <div className="p-5"><p className="text-sm text-[var(--tone-danger)]">{message}</p><Button className="mt-3" onClick={onRetry} size="sm" variant="secondary"><RefreshCw className="size-3.5" />Retry</Button></div>;
}

function SectionEmpty({ description }: { description: string }) {
  return <p className="p-5 text-sm text-[var(--text-muted)]">{description}</p>;
}

function relatedFailure(result: PromiseSettledResult<{ success: boolean; message: string | null; issues: Array<string | { detail?: string }> | null }>, fallback: string) {
  if (result.status === "rejected") return result.reason instanceof Error ? result.reason.message : fallback;
  return responseMessage(result.value, fallback);
}

function learnerStatusTone(status: Learner["status"]): "success" | "warning" | "neutral" | "danger" {
  if (status === "active") return "success";
  if (status === "prospective") return "warning";
  if (status === "withdrawn") return "danger";
  return "neutral";
}

function applicationStatusTone(status: Application["status"]): "success" | "warning" | "neutral" | "danger" | "info" {
  if (status === "accepted") return "success";
  if (status === "rejected" || status === "withdrawn") return "danger";
  if (status === "offered") return "info";
  if (status === "submitted" || status === "under_review") return "warning";
  return "neutral";
}

function displayStatus(value: string) {
  return value.replace(/_/g, " ");
}

function formatDate(value: string) {
  return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`));
}
