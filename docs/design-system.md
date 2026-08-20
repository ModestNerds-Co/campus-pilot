# Campus Pilot — Design System

**Version:** 2.0 · 20 Aug 2026 — huchu "Warm Paper" full reskin  
**Inspiration:** `iamngoni/huchu` `docs/ux/platform-ux-playbook.md` — adopted directly (palette, type scale, radii, shadow rules), translated only where huchu is CRM-specific (status vocabulary) or unavailable (the `SS Huchu` display font — we stay on `Inter var`).  
**Implementation:** CSS variables in `src/styles/tokens.css` + Tailwind v3.4.3 config + lightweight `src/components/ui/*` primitives. No private package required.

---

## 1. Philosophy (from huchu's platform-ux-playbook, adopted)

1. **One token, one truth.** If you type a hex, stop. Find the token. `src/styles/tokens.css` owns every colour, radius, shadow, and motion value. Tailwind maps to it (`tailwind.config.js` → `var(--*)`).
2. **Warm paper chrome, saturated colour only for action or state.** Page chrome sits on `--canvas` `#FCFCF4` (warm paper, not cool gray) with cards on `--surface` `#FFF`. Indigo `#4C64D4` appears once per surface, on the primary action.
3. **Role tokens, not swatches.** Use `--text-strong / --text-body / --text-muted`, `--border / --border-strong / --hairline`, `--brand / --brand-soft / --brand-tint` — never `gray-400` or `blue-600` literals.
4. **Border-first surfaces.** Cards and panels separate with a 1px border, not a shadow. Shadow is reserved for things that float above the page (popovers, modals) — see §2.6.
5. **Copy is load-bearing.** Sentence case everywhere. Button = verb. Empty state = what + why + next step. Toast = one sentence.
6. **A11y is structural:** `focus-visible` 2px `var(--focus-ring)` + 3px halo, 36px touch floor, `prefers-reduced-motion`, colour + icon + label together.

---

## 2. Tokens

All tokens live in `src/styles/tokens.css` at `:root` with a `.dark` override.

### 2.1 Surfaces (warm paper)

| Token | Light | Dark | Use |
|---|---|---|---|
| `--canvas` | `#FCFCF4` | `#171614` | Page background (`body`) — warm paper, not cool gray |
| `--surface` | `#FFFFFF` | `#211F1B` | Cards, popovers, inputs |
| `--surface-muted` | `#F7F7F2` | `#2A2822` | Hover, skeletons, table head |
| `--surface-sunken` | `#F0F0E8` | `#332F27` | Pressed, active states |
| `--surface-deep` | `#E3E3D8` | `#3D382E` | Deeply inset |

Aliases (`--surface-app`, `--surface-panel`, etc.) all point at the ladder above so legacy screens keep working.

### 2.2 Text

| Token | Light | Dark |
|---|---|---|
| `--text-strong` | `#111111` | `#F5F3EC` |
| `--text-body` | `#111111` | `#F5F3EC` |
| `--text-muted` | `#6B6B6B` | `#B8B4A8` |
| `--text-subtle` | `#9A9A93` | `#8C887C` |
| `--text-inverse` | `#FFF` | `#171614` |
| `--text-link` | `#4C64D4` | `#8A94E8` |

`--text-strong`/`--text-body` share one near-black value per huchu's spec — hierarchy comes from weight (700 headings vs 400 body), not colour. `--text-primary/secondary/tertiary` are aliases.

### 2.3 Borders & edges

`--border` `#E6E6E0` · `--border-strong` `#D6D6C8` · `--border-subtle` `#EFEFE8` · `--hairline` `rgba(17,17,17,.08)`  
`--chrome-edge` / `--chrome-shadow` draw the sidebar/app-bar seam.

### 2.4 Brand / action

Campus blue replaced with huchu's **`#4C64D4`** indigo (`action-primary-bg` in the huchu playbook) — a deliberate hue shift off the old saturated `#0B5DF0` blue, softer and warmer to sit on paper canvas.

```
--brand:        #4C64D4
--brand-strong: #3B4FB0
--brand-deeper: #2E3D8A
--brand-soft:   #EEF0FF   (huchu action-secondary-bg)
--brand-tint:   rgba(76,100,212,.08)
--brand-50/100/200/300/400/500/700/900  full ramp
```

Dark: `--brand` → `#8A94E8`, soft → `rgba(76,100,212,.16)`.

### 2.5 Semantic tones

Each tone has `-bg` (wash), `-bd` (border), and the tone itself — recolored to huchu's canonical status colour mapping:

* **info** → brand `#4C64D4`
* **success** `#2CA47C` / `#E3F4EC` (huchu "Passing")
* **warn** `#F46414` / `#FDEBE0` (huchu "Need changes")
* **danger** `#EC442C` / `#FDE7E3` (huchu `action-destructive-bg`)
* **neutral** `#6B6B6B` / `surface-muted`

Dark tones are same hue at 16–28% opacity washes.

### 2.6 Shape, type, motion, spacing, z

* **Radii (huchu 4-tier, collapsed from the old 8-tier scale):** `--radius-xs 4` / `sm 6` (huchu small) / `md 8` (huchu control default) / `lg 10` / `xl 12` (huchu card/popover) / `2xl` / `3xl` / `4xl` all `16` (huchu extra-large, capped — no size above it) + `full/pill 9999px`.
* **Type (huchu strict scale):** `Inter var` (`--font-sans`) — huchu's `SS Huchu` display face is proprietary/unavailable, so we stay on Inter for both body and display. Page title `32/700` (`--type-page-title-size`), section title `20/700` (`--type-section-title-size`), body `14/400`, label `13/600`, table header `12/600` uppercase, table cell `14/500`, caption `12/500`. Tabular-nums on IDs/money where needed (`font-variant-numeric: tabular-nums`).
* **Motion:** `--motion-ease-default cubic-bezier(0.2,0.8,0.2,1)`, durations 150/200/300ms. `prefers-reduced-motion` snaps to `0.01ms`.
* **Spacing:** `--space-1 4` / `2 8` / `3 12` / `4 16` / `5 20` / `6 24` / `8 32` / `10 40` / `12 48`. Gutters: `content-gutter-x 16`, `content-gutter-y 24`.
* **Z:** `--z-sidebar 50` / `--z-nav 60` / `--z-overlay 100` / `--z-toast 200`. Every overlay shares `--z-overlay`; open order decides stacking.
* **Elevation — border-first:** `--shadow-rest` is `none` — primary surfaces (cards, panels) separate with a 1px border only, per huchu's "minimal elevation" rule. `--shadow-hover` is a bare 1-layer lift. `--shadow-popover` / `--shadow-modal` are the only real shadows, reserved for floating overlays, using huchu's exact overlay shadow (`0 12px 24px -12px rgba(17,17,17,.18), 0 2px 6px rgba(17,17,17,.06)`).

### 2.7 Dark mode

`.dark` redefines the full set (see `tokens.css` second block). Toggle is `ThemeProvider` in `src/lib/theme.tsx` on key `campuspilot-theme` — values `light | dark | system` with `mediaQuery` listener and `meta[name=theme-color]`. The legacy `components/theme-toggle.tsx` is now a shim re-exporting the canonical toggle.

### 2.8 Shadcn compat

HSL triplets (`--background 0 0% 100%` etc.) remain in `:root`/`.dark` so `tailwind.config.js` `hsl(var(--border))` etc. keep working. They are derived from the same palette — no drift.

---

## 3. Implementation choice

**Tailwind config + CSS variables.** Reasoning:

* Keeps Tailwind v3.4.3 stable (no upgrade churn; Docker `node:20-alpine` stays green). Layer order `theme, base, tokens, app, components, utilities` gives tokens authority without beating utilities.
* `src/styles/tokens.css` is the file to edit for any colour/radius/shadow change — Tailwind maps through `var(--*)` so no JS change needed.
* If `@corelithzw/react` becomes available, `tokens.css` can be narrowed to a bridge (`var(--package-token)`) without changing call sites.

Files:

* `src/styles/tokens.css` — the token truth (light + dark).
* `src/index.css` — layer wiring, base resets, scoped transitions, `.cp-card` / `.cp-page-*` helpers, scrollbar, focus.
* `tailwind.config.js` — `colors.canvas/surface/brand` + `radius xl/2xl` extensions mapping to `var(--*)`.
* `src/lib/theme.tsx` — single provider (hotspot fix).
* `src/components/ui/*` — primitives below.

---

## 4. Component specs

### 4.1 Global shell

* **App shell:** `body` on `var(--canvas)` + `var(--app-canvas-wash)` fixed wash. Content max `1280px`, gutter `var(--content-gutter-y)`. Sidebar seam is inset `hairline` shadow, not a border.
* **Header/sidebar nav:** Fixed `h-16` header (`bg-surface border-b`), fixed sidebar `w-64 top-16` → future `SidebarProvider` with cookie `sidebar:state` + mobile drawer at `z-overlay`. Active item = `bg-surface-muted + text-strong`, not brand. Collapse persists, mobile shows drawer with no rail flash (`@media (max-width:767.98px) .cp-sidebar-desktop {display:none}`).
* **Z contract:** sidebar 50, nav 60, overlay 100, toast 200. Newest overlay paints on top.

### 4.2 Cards

Use `src/components/ui/card.tsx` compound:

```tsx
<Card><CardHeader><CardTitle>Departments</CardTitle><CardDescription>12 records</CardDescription></CardHeader><CardContent>…</CardContent><CardFooter>…</CardFooter></Card>
```

Tokens: `bg-surface`, `border-border`, `radius-card (--radius-xl 14)`, `shadow-card`. No `rounded-2xl shadow-lg border-gray-100` literals. The four previous card chromes collapse to one.

### 4.3 Buttons

`src/components/ui/button.tsx`:

| Variant | Token | Use |
|---|---|---|
| `primary` / `default` | `bg-brand text-white` | One per surface, the main action |
| `secondary` | `bg-surface border-border` | Secondary actions |
| `ghost` | `transparent → surface-muted` | Tertiary / icon buttons |
| `outline` | `bg-surface border-border` | Outlined |
| `destructive` | `bg-tone-danger text-white` | Delete |
| `link` | `text-link underline` | Inline |

Sizes `sm (30px)` / `md (36px)` / `lg (40px)` / `icon`. Touch floor 36px, gap `var(--space-2)`, radius `var(--button-radius)`, focus ring + halo, disabled `surface-muted / text-subtle`.

### 4.4 Inputs

`src/components/ui/input.tsx` — `Input`, `Textarea`, `Select`, `Label`:

* `h-36px`, `radius-md 8`, `border-input-border`, `bg-input-bg`.
* `data-slot="input"` so themed CSS can target it (`[data-slot="input"]`).
* `leadingIcon` / `trailingIcon` slots (e.g. `<Input leadingIcon={<Mail/>} />`).
* `aria-invalid` draws `tone-danger` border. Placeholders are `text-subtle`.

Retires `SearchableSelect` per-screen CSS; future `Combobox` will shim onto the same `data-slot`.

### 4.5 Tables / lists

Current milestone: CSS tokens ready (`--table-header-bg/--table-divider/--table-row-hover-bg`). Next batch introduces `DataTable` (`tabletScrollable`, `stickyHeaderOffset`, `persistKey`, `mobile-list` swap) consuming these tokens. No new table chrome should be hand-written.

### 4.6 Modals / sheets

Token wash: `surface-overlay rgba(22,24,29,.14)` for scrim, `z-overlay` for stacking. Next batch: `Dialog/Sheet` on a portal with `.modal-card size-*`, scrim, focus trap, open-order z, bottom-sheet auto-height `max-height: min(var(--drawer-size,420px), calc(100dvh - 3rem))` with safe-area.

### 4.7 Loading / empty states

`src/components/ui/skeleton.tsx`:

* `Skeleton` — `bg-surface-muted`, `rounded-sm`, `animate-pulse`.
* `Empty` — dashed `border-border` on `surface`, centered icon (`surface-muted` circle) + title + description + optional action. Replaces per-list invented empties (`border-dashed` blocks).

### 4.8 Badges & status

* `Badge` (`src/components/ui/badge.tsx`) — `tone: neutral / brand / info / success / warn / danger / outline`, `rounded-full`, optional `dot`.
* `StatusChip` / `StatusDot` (`src/components/ui/status.tsx`) — `tone: neutral / info / success / warn / danger / brand / pending` with `dot + label` (never colour alone).
* Status vocabulary is the canonical five: *Needs input · Running · Completed · Idle · Not started* — do not invent synonyms.

---

## 5. Spacing & typography scale

* 8-point base (`4/8/12/16/20/24/32`). Page = `space-6 (24)`, card head/body = `16/20`, gap between cards = `24`.
* Type (huchu strict 3-tier hierarchy — use exactly these, do not invent in-between sizes):
  * Page title `32/700` — one per screen, in the app-bar per §4.1, not redrawn in page body.
  * Section title `20/700` — card/panel headings.
  * Label `13/600` — form labels, filter labels.
  * Body `14/400` — paragraph copy.
  * Table header `12/600` uppercase.
  * Table cell `14/500`.
  * Caption `12/500` — helper text, timestamps.
* Money/IDs/timestamps use `font-mono` / `font-tabular` and are right-aligned in tables unless context requires otherwise.

---

## 6. Migration guide

1. Stop writing `bg-blue-600`, `gray-100`, `rounded-2xl`, `shadow-lg` literals — use `<Button>`, `<Card>`, `<Badge>`, `<Input>` or token classes (`bg-[var(--surface)]`, `border-[var(--border)]`, `rounded-[var(--radius-xl)]`).
2. Replace `bg-gradient-to-br from-blue-50 via-white to-gray-50` page washes with `bg-[var(--canvas)]`.
3. Replace per-list `Loader2` + hand-built empty with `Skeleton` + `Empty`.
4. Ensure dark mode is read from `useTheme()` (`campuspilot-theme`) — not the deleted `tgpatcher-theme` key.

---

## 7. Style guide

Routed at **`/style-guide`** (`src/routes/style-guide.tsx`). Renders palette swatches, type scale, buttons, cards, inputs, badges, status, skeletons, empties, tables — built from the tokens and primitives above so it is the live proof that the system renders both locally (`vite --port 3000`) and in Docker (`client/Dockerfile` → `node:20-alpine` → `nginx`).

---

## 8. Approval checklist

* [ ] Palette reviewed (light + dark) — brand `#4C64D4` / dark `#8A94E8` and tone washes
* [ ] Type scale + spacing rhythm approved
* [ ] `Button / Card / Badge / Input / Empty / Skeleton` examples approved
* [ ] `/style-guide` renders in `vite build` preview and `docker build -f client/Dockerfile`

---

*Hotspot note:* `components/theme-toggle.tsx` vs `lib/theme.tsx` is unified — first is now a re-export on the same `campuspilot-theme` key. `index.css` no longer has `* { transition-colors }`; transitions are scoped to `a/button/input/select/textarea`.
