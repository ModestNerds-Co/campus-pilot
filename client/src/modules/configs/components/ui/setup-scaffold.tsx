import { Check, School, ShieldCheck } from "lucide-react";

import { ThemeToggle } from "@/lib/theme";
import { cn } from "@/lib/utils";

const steps = [
  { label: "School profile", icon: School },
  { label: "First administrator", icon: ShieldCheck },
];

export function SetupScaffold({
  children,
  description,
  maxWidth = "wide",
  step,
  title,
}: {
  children: React.ReactNode;
  description: string;
  maxWidth?: "narrow" | "wide";
  step: 1 | 2;
  title: string;
}) {
  return (
    <main className="min-h-[100dvh] bg-[var(--canvas)] lg:grid lg:grid-cols-[300px_minmax(0,1fr)]">
      <aside className="relative hidden h-[100dvh] min-h-[100dvh] self-start overflow-hidden bg-[var(--sidebar)] px-8 py-8 text-[var(--sidebar-foreground)] lg:sticky lg:top-0 lg:flex lg:flex-col">
        <div aria-hidden="true" className="campus-grid-pattern absolute inset-0 opacity-55" />
        <div className="relative z-10 flex items-center gap-3">
          <span className="flex size-11 items-center justify-center rounded-[10px] bg-[var(--brand-highlight)]">
            <img alt="" aria-hidden="true" className="size-8 rounded-full object-cover mix-blend-multiply" src="/assets/images/campus-pilot-logo.svg" />
          </span>
          <div>
            <p className="text-base font-bold tracking-[-0.03em]">Campus Pilot</p>
            <p className="text-[11px] font-medium uppercase tracking-[0.16em] text-[var(--sidebar-muted)]">Workspace setup</p>
          </div>
        </div>

        <div className="relative z-10 my-auto py-12">
          <p className="text-[11px] font-semibold uppercase tracking-[0.2em] text-[var(--brand-highlight)]">SETUP {String(step).padStart(2, "0")} OF 02</p>
          <h2 className="mt-5 text-3xl font-semibold tracking-[-0.045em]">Build the campus foundation.</h2>
          <p className="mt-4 text-sm leading-6 text-[var(--sidebar-muted)]">
            Two focused steps establish the school identity and the account responsible for it.
          </p>

          <ol className="mt-9 space-y-3" aria-label="Setup progress">
            {steps.map(({ icon: Icon, label }, index) => {
              const number = index + 1;
              const complete = number < step;
              const active = number === step;
              return (
                <li
                  className={cn(
                    "flex items-center gap-3 rounded-[var(--radius-lg)] border px-3 py-3",
                    active
                      ? "border-[var(--sidebar-active)] bg-white/[0.08] text-[var(--sidebar-foreground)]"
                      : "border-transparent text-[var(--sidebar-muted)]",
                  )}
                  key={label}
                >
                  <span className={cn("flex size-9 items-center justify-center rounded-[8px]", active ? "bg-[var(--brand-highlight)] text-[var(--sidebar-active-fg)]" : "bg-white/5") }>
                    {complete ? <Check className="size-4" /> : <Icon className="size-4" />}
                  </span>
                  <span>
                    <span className="block text-[10px] font-semibold uppercase tracking-[0.14em] opacity-70">Step {number}</span>
                    <span className="mt-0.5 block text-sm font-semibold">{label}</span>
                  </span>
                </li>
              );
            })}
          </ol>
        </div>

        <div className="relative z-10">
          <ThemeToggle className="w-full" variant="sidebar" />
        </div>
      </aside>

      <section className="min-w-0">
        <div className="flex items-center justify-between bg-[var(--sidebar)] px-5 py-4 text-[var(--sidebar-foreground)] lg:hidden">
          <div className="flex items-center gap-2.5">
            <span className="flex size-9 items-center justify-center rounded-[8px] bg-[var(--brand-highlight)]">
              <img alt="" aria-hidden="true" className="size-7 rounded-full object-cover mix-blend-multiply" src="/assets/images/campus-pilot-logo.svg" />
            </span>
            <div>
              <p className="text-sm font-bold">Campus Pilot</p>
              <p className="text-[10px] uppercase tracking-[0.14em] text-[var(--sidebar-muted)]">Step {step} of 2</p>
            </div>
          </div>
          <ThemeToggle variant="sidebar" />
        </div>

        <div className={cn("mx-auto px-5 py-8 sm:px-8 sm:py-12 xl:py-14", maxWidth === "wide" ? "max-w-[1180px]" : "max-w-[720px]") }>
          <header className="mb-8 border-b border-[var(--border)] pb-7 sm:mb-10">
            <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-[var(--brand-strong)]">Workspace setup · Step {step} of 2</p>
            <h1 className="mt-3 text-3xl font-semibold tracking-[-0.045em] text-[var(--text-strong)] sm:text-4xl">{title}</h1>
            <p className="mt-3 max-w-2xl text-sm leading-6 text-[var(--text-muted)] sm:text-base">{description}</p>
          </header>
          {children}
        </div>
      </section>
    </main>
  );
}
