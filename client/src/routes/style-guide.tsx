//
//  campus-pilot
//  style-guide.tsx — Live design-system showcase (tokens + primitives)
//  Renders at /style-guide in development only.
//

import { createFileRoute, redirect } from "@tanstack/react-router";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter } from "@/components/ui/card";
import { Badge, BadgeGroup } from "@/components/ui/badge";
import { StatusChip, StatusDot } from "@/components/ui/status";
import { Input, Textarea, Select, Label } from "@/components/ui/input";
import { Skeleton, Empty } from "@/components/ui/skeleton";
import { ThemeToggle } from "@/lib/theme";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Mail, Lock, Search, Plus, Trash2, Github, Sparkles, Inbox } from "lucide-react";

export const Route = createFileRoute("/style-guide")({
  beforeLoad: () => {
    if (!import.meta.env.DEV) throw redirect({ to: "/" });
  },
  component: StyleGuidePage,
});

function Swatch({ name, value, className }: { name: string; value: string; className: string }) {
  return (
    <div className="flex flex-col gap-1.5">
      <div className={`h-14 rounded-[var(--radius-lg)] border border-[var(--border)] ${className}`} />
      <div className="text-xs font-medium text-[var(--text-strong)]">{name}</div>
      <div className="text-xs font-mono text-[var(--text-subtle)]">{value}</div>
    </div>
  );
}

function Section({ title, desc, children }: { title: string; desc?: string; children: React.ReactNode }) {
  return (
    <section className="space-y-4">
      <div>
        <h2 className="text-[length:var(--type-section-title-size)] font-bold text-[var(--text-strong)]">{title}</h2>
        {desc ? <p className="mt-1 text-sm text-[var(--text-muted)]">{desc}</p> : null}
      </div>
      <div className="rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface)] p-6 shadow-[var(--shadow-card)]">{children}</div>
    </section>
  );
}

function StyleGuidePage() {
  const [drawerOpen, setDrawerOpen] = useState(false);

  return (
    <div className="min-h-screen bg-[var(--canvas)]">
      {/* Top bar */}
      <header className="sticky top-0 z-[var(--z-nav)] flex h-14 items-center justify-between border-b border-[var(--border)] bg-[var(--surface)] px-6">
        <div className="flex items-center gap-3">
          <div className="flex size-8 items-center justify-center rounded-[var(--radius-md)] bg-[var(--brand)] text-[var(--on-brand)]">
            <Sparkles className="size-4" />
          </div>
          <div>
            <div className="text-sm font-semibold text-[var(--text-strong)]">Campus Pilot — Style guide</div>
            <div className="text-xs text-[var(--text-muted)]">Tokens · primitives · patterns</div>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <a href="/login" className="text-sm text-[var(--text-link)] hover:underline">Sign in</a>
          <a href="/admin" className="text-sm text-[var(--text-link)] hover:underline">Admin</a>
          <ThemeToggle />
        </div>
      </header>

      <main className="mx-auto max-w-6xl space-y-8 p-6 lg:p-8">
        <div className="space-y-2">
          <h1 className="text-[length:var(--type-page-title-size)] font-bold leading-tight text-[var(--text-strong)]">Design system</h1>
          <p className="max-w-3xl text-sm leading-relaxed text-[var(--text-muted)]">
            The CCS-inspired institutional structure adapted to school operations: Yale-blue navigation, calm surfaces, restrained emphasis, and role tokens throughout. Toggle the theme to verify both modes.
          </p>
        </div>

        {/* Palette */}
        <Section title="Palette" desc="Surfaces are neutral; brand and tones carry meaning. No literal hex in call sites.">
          <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-6">
            <Swatch name="Canvas" value="--canvas" className="bg-[var(--canvas)]" />
            <Swatch name="Surface" value="--surface" className="bg-[var(--surface)]" />
            <Swatch name="Muted" value="--surface-muted" className="bg-[var(--surface-muted)]" />
            <Swatch name="Sunken" value="--surface-sunken" className="bg-[var(--surface-sunken)]" />
            <Swatch name="Border" value="--border" className="bg-[var(--border)]" />
            <Swatch name="Strong" value="--border-strong" className="bg-[var(--border-strong)]" />
          </div>
          <div className="mt-6 grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-6">
            <Swatch name="Brand" value="--brand" className="bg-[var(--brand)]" />
            <Swatch name="Brand soft" value="--brand-soft" className="bg-[var(--brand-soft)]" />
            <Swatch name="Success" value="--tone-success" className="bg-[var(--tone-success)]" />
            <Swatch name="Warn" value="--tone-warn" className="bg-[var(--tone-warn)]" />
            <Swatch name="Danger" value="--tone-danger" className="bg-[var(--tone-danger)]" />
            <Swatch name="Info" value="--tone-info" className="bg-[var(--tone-info)]" />
          </div>
          <div className="mt-6 flex flex-wrap gap-3">
            <span className="inline-flex items-center gap-1.5 text-xs"><span className="size-3 rounded-full bg-[var(--tone-success)]" /> success wash <span className="rounded bg-[var(--tone-success-bg)] px-2 py-0.5 text-[var(--tone-success-strong)]">Aa</span></span>
            <span className="inline-flex items-center gap-1.5 text-xs"><span className="size-3 rounded-full bg-[var(--tone-warn)]" /> warn wash <span className="rounded bg-[var(--tone-warn-bg)] px-2 py-0.5 text-[var(--tone-warn-strong)]">Aa</span></span>
            <span className="inline-flex items-center gap-1.5 text-xs"><span className="size-3 rounded-full bg-[var(--tone-danger)]" /> danger wash <span className="rounded bg-[var(--tone-danger-bg)] px-2 py-0.5 text-[var(--tone-danger-strong)]">Aa</span></span>
          </div>
        </Section>

        {/* Typography */}
        <Section title="Typography" desc="Geist Variable with a compact operational hierarchy for headings, body copy, labels, tables, and captions.">
          <div className="space-y-3">
            <div className="text-[length:var(--type-page-title-size)] font-bold leading-tight text-[var(--text-strong)]">Page title 32/700 — Departments</div>
            <div className="text-[length:var(--type-section-title-size)] font-bold leading-tight text-[var(--text-strong)]">Section title 20/700 — Recent activity</div>
            <div className="text-[length:var(--type-label-size)] font-semibold text-[var(--text-strong)]">Label 13/600 — Email address</div>
            <div className="text-[length:var(--type-body-size)] leading-relaxed text-[var(--text-body)]">Body 14/400 — The quick brown fox jumps over the lazy dog. Use for prose, table cells, form help.</div>
            <div className="text-[length:var(--type-table-header-size)] font-semibold uppercase tracking-wider text-[var(--table-header-text)]">Table header 12/600 uppercase</div>
            <div className="text-[length:var(--type-table-cell-size)] font-medium text-[var(--text-strong)]">Table cell 14/500</div>
            <div className="text-[length:var(--type-caption-size)] font-medium text-[var(--text-muted)]">Caption 12/500 — subtle, for hints</div>
            <div className="font-mono text-sm text-[var(--text-body)]">Mono 14 — IDs, codes <span className="font-tabular">123,456.00</span></div>
          </div>
        </Section>

        {/* Radii + shadows + spacing */}
        <Section title="Shape, elevation and spacing">
          <div className="grid gap-6 lg:grid-cols-3">
            <div>
              <div className="mb-2 text-xs font-medium text-[var(--text-muted)]">Radii</div>
              <div className="flex flex-wrap gap-2">
                {[
                  ["xs", "var(--radius-xs)"],
                  ["sm", "var(--radius-sm)"],
                  ["md", "var(--radius-md)"],
                  ["lg", "var(--radius-lg)"],
                  ["xl", "var(--radius-xl)"],
                  ["2xl", "var(--radius-2xl)"],
                ].map(([k, v]) => (
                  <span key={k} className="inline-flex size-10 items-center justify-center border border-[var(--border)] bg-[var(--surface)] text-xs" style={{ borderRadius: v }}>{k}</span>
                ))}
              </div>
            </div>
            <div>
              <div className="mb-2 text-xs font-medium text-[var(--text-muted)]">Elevation</div>
              <div className="flex flex-wrap gap-3">
                <span className="rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-xs shadow-[var(--shadow-rest)]">rest</span>
                <span className="rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-xs shadow-[var(--shadow-hover)]">hover</span>
                <span className="rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-xs shadow-[var(--shadow-popover)]">popover</span>
              </div>
            </div>
            <div>
              <div className="mb-2 text-xs font-medium text-[var(--text-muted)]">Spacing</div>
              <div className="flex items-end gap-2">
                {[4, 8, 12, 16, 24, 32].map((n) => (
                  <div key={n} className="flex flex-col items-center gap-1">
                    <div className="bg-[var(--brand)]" style={{ width: n, height: n, borderRadius: 2 }} />
                    <span className="text-xs text-[var(--text-subtle)]">{n}</span>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </Section>

        {/* Buttons */}
        <Section title="Buttons" desc="One primary per surface. 36px floor. Focus ring + halo.">
          <div className="flex flex-wrap gap-3">
            <Button>Primary</Button>
            <Button variant="secondary"><Plus className="size-4" /> Secondary</Button>
            <Button variant="ghost">Ghost</Button>
            <Button variant="outline">Outline</Button>
            <Button variant="destructive"><Trash2 className="size-4" /> Destructive</Button>
            <Button variant="link">Link</Button>
          </div>
          <div className="mt-4 flex flex-wrap items-center gap-3">
            <Button size="sm">Small</Button>
            <Button size="md">Medium</Button>
            <Button size="lg">Large</Button>
            <Button size="icon" aria-label="GitHub"><Github className="size-4" /></Button>
            <Button disabled>Disabled</Button>
          </div>
        </Section>

        <Section title="Right-side drawers" desc="Forms, confirmations, previews, and secondary workflows enter from the right; centered modals are not used.">
          <Button onClick={() => setDrawerOpen(true)}>Open example drawer</Button>
          <DialogShell onClose={() => setDrawerOpen(false)} open={drawerOpen}>
            <DialogHeader onClose={() => setDrawerOpen(false)} title="Example workflow" />
            <DialogBody>
              <p className="text-sm leading-6 text-[var(--text-muted)]">
                The panel is full-width on small screens and bounded on desktop. Escape closes it, focus stays inside, and returns to the trigger afterward.
              </p>
            </DialogBody>
            <DialogFooter>
              <Button data-autofocus="true" onClick={() => setDrawerOpen(false)} variant="secondary">Cancel</Button>
              <Button onClick={() => setDrawerOpen(false)}>Save example</Button>
            </DialogFooter>
          </DialogShell>
        </Section>

        {/* Cards */}
        <Section title="Cards" desc="Single chrome: border + radius-xl + shadow-card. Compound API.">
          <div className="grid gap-4 lg:grid-cols-3">
            <Card>
              <CardHeader>
                <CardTitle>Metric card</CardTitle>
                <CardDescription>Developer component example</CardDescription>
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-semibold text-[var(--text-strong)]">—</div>
                <div className="text-xs text-[var(--text-muted)]">No operational data</div>
              </CardContent>
              <CardFooter>
                <Button variant="ghost" size="sm">View all</Button>
              </CardFooter>
            </Card>
            <Card className="cp-card-hover">
              <CardHeader>
                <CardTitle>Hoverable</CardTitle>
                <CardDescription>Border + shadow lift on hover</CardDescription>
              </CardHeader>
              <CardContent>
                <p className="text-sm text-[var(--text-body)]">Hover this card to see the elevation token.</p>
              </CardContent>
            </Card>
            <Card>
              <CardContent>
                <Empty icon={<Inbox className="size-5" />} title="No departments yet" description="Create your first department to get started." action={<Button size="sm"><Plus className="size-4" /> New department</Button>} />
              </CardContent>
            </Card>
          </div>
        </Section>

        {/* Badges + status */}
        <Section title="Badges and status">
          <div className="space-y-4">
            <BadgeGroup>
              <Badge tone="neutral">Neutral</Badge>
              <Badge tone="brand">Brand</Badge>
              <Badge tone="info">Info</Badge>
              <Badge tone="success" dot>Success</Badge>
              <Badge tone="warn">Warn</Badge>
              <Badge tone="danger">Danger</Badge>
            </BadgeGroup>
            <div className="flex flex-wrap gap-2">
              <StatusChip tone="success" dot>Completed</StatusChip>
              <StatusChip tone="warn" dot>Running</StatusChip>
              <StatusChip tone="danger" dot>Needs input</StatusChip>
              <StatusChip tone="info" dot>Idle</StatusChip>
              <StatusChip tone="neutral" dot>Not started</StatusChip>
            </div>
            <div className="flex items-center gap-2 text-sm">
              <StatusDot tone="success" /> <span>Active</span>
              <StatusDot tone="danger" /> <span>Inactive</span>
              <StatusDot tone="warn" /> <span>Pending</span>
            </div>
          </div>
        </Section>

        {/* Inputs */}
        <Section title="Inputs" desc="data-slot=input, 36px, leading icons, aria-invalid.">
          <div className="grid gap-6 lg:grid-cols-2">
            <div className="space-y-3">
              <div>
                <Label htmlFor="sg-email">Email</Label>
                <Input id="sg-email" placeholder="you@school.edu" leadingIcon={<Mail />} className="mt-1.5" />
              </div>
              <div>
                <Label htmlFor="sg-pass">Password</Label>
                <Input id="sg-pass" type="password" placeholder="••••••••" leadingIcon={<Lock />} className="mt-1.5" />
                <p className="mt-1 text-xs text-[var(--text-subtle)]">Hint: 8+ characters.</p>
              </div>
              <div>
                <Label htmlFor="sg-err">With error</Label>
                <Input id="sg-err" defaultValue="bad@" aria-invalid leadingIcon={<Mail />} className="mt-1.5" />
                <p role="alert" className="mt-1 text-xs text-[var(--tone-danger)]">Enter a valid email.</p>
              </div>
            </div>
            <div className="space-y-3">
              <div>
                <Label htmlFor="sg-search">Search</Label>
                <Input id="sg-search" placeholder="Search students, staff…" leadingIcon={<Search />} className="mt-1.5" />
              </div>
              <div>
                <Label htmlFor="sg-select">Department</Label>
                <Select id="sg-select" defaultValue="" className="mt-1.5">
                  <option value="" disabled>Select department</option>
                  <option>Science</option>
                  <option>Arts</option>
                  <option>Commerce</option>
                </Select>
              </div>
              <div>
                <Label htmlFor="sg-area">Notes</Label>
                <Textarea id="sg-area" placeholder="Optional notes…" className="mt-1.5" />
              </div>
            </div>
          </div>
        </Section>

        {/* Skeletons / empty / table mock */}
        <Section title="Lists and tables (preview)">
          <div className="space-y-4">
            <div className="space-y-2">
              <Skeleton className="h-4 w-1/3" />
              <Skeleton className="h-4 w-2/3" />
              <Skeleton className="h-20 w-full" />
            </div>
            <div className="overflow-hidden rounded-[var(--radius-xl)] border border-[var(--border)]">
              <div className="flex items-center justify-between bg-[var(--table-header-bg)] px-4 py-2">
                <span className="text-xs font-medium text-[var(--table-header-text)]">Users — 3 rows (mock)</span>
                <Button variant="secondary" size="sm"><Plus className="size-3" /> Invite</Button>
              </div>
              <div className="divide-y divide-[var(--table-divider)]">
                {[
                  ["Ada Lovelace", "Admin", "success"],
                  ["Grace Hopper", "Teacher", "warn"],
                  ["Katherine Johnson", "Student", "neutral"],
                ].map(([name, role, tone]) => (
                  <div key={name} className="flex items-center justify-between bg-[var(--table-row-bg)] px-4 py-3 hover:bg-[var(--table-row-hover-bg)]">
                    <div className="flex items-center gap-3">
                      <div className="flex size-8 items-center justify-center rounded-full bg-[var(--brand-soft)] text-xs font-medium text-[var(--brand-strong)]">{name.split(" ").map((w) => w[0]).join("")}</div>
                      <div>
                        <div className="text-sm font-medium text-[var(--text-strong)]">{name}</div>
                        <div className="text-xs text-[var(--text-muted)]">{role}</div>
                      </div>
                    </div>
                    <Badge tone={tone as never}>{role}</Badge>
                  </div>
                ))}
              </div>
              <div className="flex items-center justify-between border-t border-[var(--border)] bg-[var(--surface)] px-4 py-2 text-xs text-[var(--text-muted)]">
                <span>Showing 3 of 3</span>
                <span className="font-tabular">Page 1 / 1</span>
              </div>
            </div>
          </div>
        </Section>

        <Card>
          <CardContent>
            <p className="text-sm text-[var(--text-muted)]">
              Tokens file: <code className="rounded bg-[var(--surface-muted)] px-1 py-0.5 font-mono text-xs">src/styles/tokens.css</code> · Spec:{" "}
              <code className="rounded bg-[var(--surface-muted)] px-1 py-0.5 font-mono text-xs">docs/design-system.md</code> · Primitives:{" "}
              <code className="rounded bg-[var(--surface-muted)] px-1 py-0.5 font-mono text-xs">src/components/ui/*</code>
            </p>
          </CardContent>
        </Card>
      </main>
    </div>
  );
}
