//
//  campus-pilot
//  coming-soon.tsx - Scaffolded ERP module placeholder
//
//  Created by Ngonidzashe Mangudya on 21/08/2026.
//  Copyright (c) 2025 Codecraft Solutions
//
//  Placeholder for a module that has a defined scope but no workspace yet.

import React from "react";
import { ArrowRight, CircleDashed } from "lucide-react";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

interface ComingSoonProps {
  title: string;
  description: string;
  icon: React.ComponentType<{ className?: string }>;
  highlights?: string[];
}

export const ComingSoon: React.FC<ComingSoonProps> = ({ title, description, icon: Icon, highlights }) => {
  usePageChrome(title);

  return (
    <div className="space-y-8">
      <section className="relative overflow-hidden rounded-[var(--radius-2xl)] bg-[var(--sidebar)] px-6 py-8 text-[var(--sidebar-foreground)] sm:px-8 sm:py-10">
        <div aria-hidden="true" className="campus-grid-pattern absolute inset-0 opacity-45" />
        <div className="relative max-w-2xl">
          <div className="flex items-start gap-4">
            <span className="flex size-11 shrink-0 items-center justify-center rounded-[10px] bg-[var(--brand-highlight)] text-[var(--sidebar-active-fg)]">
              <Icon className="size-5" />
            </span>
            <div>
              <div className="inline-flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-highlight)]">
                <CircleDashed className="size-3.5" />
                Planned module
              </div>
              <p className="mt-3 max-w-xl text-sm leading-6 text-[var(--sidebar-muted)]">{description}</p>
            </div>
          </div>
        </div>
      </section>

      {highlights && highlights.length > 0 ? (
        <section aria-labelledby="module-scope-title">
          <div className="flex items-end justify-between gap-4 border-b border-[var(--border)] pb-3">
            <div>
              <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]">Module scope</p>
              <h2 className="mt-1 text-lg font-semibold tracking-[-0.02em] text-[var(--text-strong)]" id="module-scope-title">
                Planned areas
              </h2>
            </div>
          </div>
          <ul className="divide-y divide-[var(--border-subtle)]">
            {highlights.map((item, index) => (
              <li className="flex items-center gap-4 py-4" key={item}>
                <span className="font-tabular text-xs font-semibold text-[var(--text-subtle)]">{String(index + 1).padStart(2, "0")}</span>
                <span className="flex-1 text-sm font-medium text-[var(--text-body)]">{item}</span>
                <ArrowRight className="size-4 text-[var(--text-subtle)]" />
              </li>
            ))}
          </ul>
        </section>
      ) : null}
    </div>
  );
};
