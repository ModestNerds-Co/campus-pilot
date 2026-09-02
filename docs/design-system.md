# Campus Pilot UI Guidelines

This is the canonical UI reference for Campus Pilot. Keep it concise and update it when a durable product or interaction decision changes.

## 1. Product direction

Campus Pilot should feel like a confident institutional operations product: calm, capable, and easy to scan during repetitive administrative work.

- Use `/Users/modestnerd/Developer/Projects/ccs` as the structural reference for shell hierarchy, navigation density, page composition, and interaction quality.
- Adapt the structure to school operations. Do not copy finance-specific content or force CCS behavior where Campus Pilot has different domain needs.
- Prefer clear operational truth over decorative dashboards. Do not invent metrics, activity, or chart data.

## 2. Visual language

The implementation source of truth is `client/src/styles/tokens.css`.

- Typography: Geist Variable through `--font-sans`; sentence case throughout.
- Canvas: calm cool-neutral `--canvas`; primary surfaces use `--surface`.
- Navigation: deep institutional navy via the `--sidebar-*` tokens.
- Brand: Yale-inspired blue anchors primary actions and focus; a lighter blue tint marks selected navigation and deliberate emphasis.
- Semantic success remains green. Do not reuse success green as product branding.
- Surfaces: border-first, restrained shadows, compact radii, and no ornamental gradients.
- Color: use semantic or role tokens such as `--text-muted`, `--border`, and `--tone-danger`; do not introduce parallel hard-coded palettes.
- Dark mode must use the same hierarchy and semantic roles as light mode.

## 3. Application structure

### Campus module launcher

- Successful sign-in opens `/home`, the permission-aware campus module launcher.
- The launcher shows enabled and authorized modules only, with recent modules first and the remainder grouped by school task.
- Administration is one module in this launcher; it is never the default post-login destination.
- Campus Owners see the core Administration workspace pinned first. Never show an owner copy that tells them to contact an administrator.
- Protected routes refresh the authenticated access profile before rendering so persisted browser state cannot hide newly granted roles or modules.
- The launcher uses the same fixed-rail and contextual-header grammar as module shells. Its desktop rail holds campus identity, All modules, authorized shortcuts, task-group jump links, theme, account identity, and sign-out.
- On mobile, the launcher rail becomes the shared off-canvas navigation pattern with a scrim, Escape support, focus containment, focus restoration, and background scroll lock. Keep this drawer short enough to avoid nested scrolling: task-group jump links remain desktop-only.
- Operational modules own their local navigation and provide an “All modules” return path.
- E-learning uses full-page space, unit, assignment, learner-work, and review workspaces. Space, unit, resource, publication, withdrawal, archive, settings, assignment authoring, file removal, submission confirmation, and feedback-release workflows use the shared right-side drawer where secondary focus is needed; published content is presented as read-only rather than with disabled editing forms. Learner file upload remains inline in the learner-work page so a drawer never contains another scrolling file workflow.
- Transport uses full-page route and run workspaces. Route, stop, rider, run, manifest, lifecycle, and confirmation actions use the shared right-side drawer; the manifest stays visible while one learner entry is being marked.
- Access, licensing, role, and module semantics are defined in `docs/access-control.md`.

### Admin shell

- Desktop uses a fixed, full-height left rail and a contextual top header.
- Its navigation is limited to administration concerns; campus operations do not live in the Administration rail.
- The rail owns school identity, grouped navigation, theme control, user identity, and sign-out.
- The rail remains scrollable with wheel, touch, and keyboard, but native scrollbar chrome stays hidden through `.cp-sidebar-scroll`.
- Mobile navigation is an off-canvas drawer with a scrim, Escape support, focus handling, and background scroll lock.
- Main content uses a calm canvas, bounded readable width, and consistent page rhythm.

### Agent surfaces

- Agent has a full module workspace for Sessions, searchable history, approvals, run inspection, and personal usage. “Session” is the user-facing conversation term; a “run” is one Agent execution inside it.
- The authenticated shell exposes one persistent global Agent widget on Home, Administration, and operational modules when Agent is enabled and the person has access. Anchor it to shared shell navigation; do not use a draggable chat bubble that covers page controls.
- The widget can start a Session, resume the active or most recent Session, show a short recent list, and indicate a running or approval-waiting state. Complete history and Session management stay in the full Agent module.
- The widget opens the shared right-side drawer with deliberate current-module or current-record context. Navigating to another page never silently changes the context attached to an existing Session.
- The contextual drawer has one scrolling message region and a stable composer. Do not nest another scrolling workflow inside it; open the full Agent module for long work, history, provider administration, or approvals.
- Provider setup, capability policy, limits, and campus-wide usage reports live in Administration. The interaction and authorization model is defined in `docs/agent-platform.md`.
- Usage reports, capability matrices, and run trails use full pages, not drawers. Drawers remain focused on connect, edit, approval, limit, and confirmation workflows.

### Navigation

- Group links by operational task rather than technical module.
- Active items must remain unmistakable through more than color alone.
- Every link resolves to a real route. Unfinished areas use the shared intentional coming-soon screen.
- Page titles and primary actions live in the contextual header; do not repeat competing page headers in the body.

### Standalone flows

- Login uses the split institutional layout: product/school context beside a focused sign-in form.
- Boot and setup screens use the same typography, token system, feedback language, and responsive rules as the admin product.

## 4. Drawers, never centered modals

Forms, destructive confirmations, changelogs, document previews, and secondary workflows open from the right.

- Desktop drawer: full height, right aligned, bounded to an appropriate width; `640px` is the default form width.
- Mobile drawer: full viewport width and height.
- The scrim closes the drawer when the workflow is safe to dismiss.
- Escape closes; focus is trapped while open and restored to the trigger on close.
- Background scrolling is locked while open.
- Header and action footer stay stable while long drawer content scrolls.
- A dirty form drawer must preserve its input through validation and load failures. Closing it opens a sequential right-side discard confirmation; never stack two drawers or silently discard work.
- Destructive confirmations focus the safe action first and clearly identify the affected record.
- One-time codes and credentials state where they must be used, show copy success or failure, and make intentional loss on close clear.
- Do not use `window.confirm`, centered dialog cards, or ad-hoc fixed overlays.

The current shared implementation is `client/src/components/ui/dialog.tsx`; its historical API name is retained for compatibility, but it renders the right-side drawer pattern.

## 5. Components and page states

- Prefer primitives in `client/src/components/ui/` for buttons, inputs, badges, tables, cards, and drawers.
- A surface has one obvious primary action. Use verb-led labels such as “Add user” or “Log a trip”.
- Search and filters must clearly distinguish an empty collection from no matching results.
- Operational list search, filters, and pagination belong in the URL so refresh, Back, and Forward restore the same list state. Ignore stale responses when a newer request is in flight.
- Lists need four deliberate states: loading skeleton, loaded content, helpful empty state, and recoverable error with retry.
- Destructive actions require a drawer confirmation and a visible pending state.
- Long provider or operational option sets should use searchable dropdowns rather than raw HTML selects; short fixed enums may use the shared `Select`.
- Status is communicated with text and, when useful, an icon or dot—not color alone.
- Multi-stage operational work such as timetable generation keeps setup and review in the page, with focused create/edit actions in drawers. A drawer must not become an entire module inside an overlay.
- Academic report batches and learner transcripts are full-page workspaces. Grading-scheme editing, report generation, remarks, progression review, lifecycle transitions, and confirmations use right-side drawers.

## 6. Responsive and accessible behavior

- Support at least 320px width without horizontal page overflow.
- Touch targets are at least 44px on coarse-pointer devices.
- All icon-only controls require accessible labels.
- Use `:focus-visible` and the global focus tokens; never remove focus indication.
- Respect `prefers-reduced-motion`.
- Tables may scroll horizontally when the data cannot responsibly collapse, while surrounding page chrome remains stable.
- Copy errors in plain language and provide a useful recovery action.

## 7. Product copy

- Keep end-user copy operational. A sentence should report a status, explain a consequence, or tell the operator what to do next.
- Remove slogans, reassurance, and repeated explanations that do not change a decision or action.
- Do not expose implementation details such as seeded roles, stable keys, signatures, fingerprints, entitlement claims, legacy migrations, or internal access mechanics.
- Use familiar product terms such as users, roles, modules, licensing, and school settings. Keep internal terminology in code and technical documentation.
- Preserve concise safety information when it explains an irreversible action, access change, expiry, or recovery step.

## 8. Implementation and QA checklist

Before a UI checkpoint is deployed:

- [ ] No new centered modal or native confirmation exists.
- [ ] Sidebar scroll works and its scrollbar chrome remains hidden.
- [ ] Navigation has no dead links.
- [ ] Loading, empty, error, and success behavior is intentional.
- [ ] Keyboard focus, Escape, and focus return work for drawers and mobile navigation.
- [ ] UI copy contains only operational status, consequences, and actions; no implementation notes or marketing slogans.
- [ ] Desktop and mobile browser checks pass.
- [ ] `pnpm run build` passes in `client/`.
- [ ] The client is rebuilt with both Compose files and the public route remains healthy.

## 9. Canonical files

- `client/src/styles/tokens.css` — color, type, spacing, shape, motion, and elevation tokens.
- `client/src/index.css` — global behavior, accessibility, and reusable shell helpers.
- `client/src/modules/admin/layouts/admin-layout.tsx` — application shell and navigation.
- `client/src/components/ui/dialog.tsx` — shared right-side drawer behavior.
- `client/src/components/ui/data-table.tsx` — shared list states and table structure.
- `docs/agent-platform.md` — Agent, provider, capability, approval, and usage architecture.
