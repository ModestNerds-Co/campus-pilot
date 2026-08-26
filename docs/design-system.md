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

### Admin shell

- Desktop uses a fixed, full-height left rail and a contextual top header.
- The rail owns school identity, grouped navigation, theme control, user identity, and sign-out.
- The rail remains scrollable with wheel, touch, and keyboard, but native scrollbar chrome stays hidden through `.cp-sidebar-scroll`.
- Mobile navigation is an off-canvas drawer with a scrim, Escape support, focus handling, and background scroll lock.
- Main content uses a calm canvas, bounded readable width, and consistent page rhythm.

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
- Destructive confirmations focus the safe action first and clearly identify the affected record.
- Do not use `window.confirm`, centered dialog cards, or ad-hoc fixed overlays.

The current shared implementation is `client/src/components/ui/dialog.tsx`; its historical API name is retained for compatibility, but it renders the right-side drawer pattern.

## 5. Components and page states

- Prefer primitives in `client/src/components/ui/` for buttons, inputs, badges, tables, cards, and drawers.
- A surface has one obvious primary action. Use verb-led labels such as “Add user” or “Log a trip”.
- Search and filters must clearly distinguish an empty collection from no matching results.
- Lists need four deliberate states: loading skeleton, loaded content, helpful empty state, and recoverable error with retry.
- Destructive actions require a drawer confirmation and a visible pending state.
- Long provider or operational option sets should use searchable dropdowns rather than raw HTML selects; short fixed enums may use the shared `Select`.
- Status is communicated with text and, when useful, an icon or dot—not color alone.

## 6. Responsive and accessible behavior

- Support at least 320px width without horizontal page overflow.
- Touch targets are at least 44px on coarse-pointer devices.
- All icon-only controls require accessible labels.
- Use `:focus-visible` and the global focus tokens; never remove focus indication.
- Respect `prefers-reduced-motion`.
- Tables may scroll horizontally when the data cannot responsibly collapse, while surrounding page chrome remains stable.
- Copy errors in plain language and provide a useful recovery action.

## 7. Implementation and QA checklist

Before a UI checkpoint is deployed:

- [ ] No new centered modal or native confirmation exists.
- [ ] Sidebar scroll works and its scrollbar chrome remains hidden.
- [ ] Navigation has no dead links.
- [ ] Loading, empty, error, and success behavior is intentional.
- [ ] Keyboard focus, Escape, and focus return work for drawers and mobile navigation.
- [ ] Desktop and mobile browser checks pass.
- [ ] `pnpm run build` passes in `client/`.
- [ ] The client is rebuilt with both Compose files and the public route remains healthy.

## 8. Canonical files

- `client/src/styles/tokens.css` — color, type, spacing, shape, motion, and elevation tokens.
- `client/src/index.css` — global behavior, accessibility, and reusable shell helpers.
- `client/src/modules/admin/layouts/admin-layout.tsx` — application shell and navigation.
- `client/src/components/ui/dialog.tsx` — shared right-side drawer behavior.
- `client/src/components/ui/data-table.tsx` — shared list states and table structure.
