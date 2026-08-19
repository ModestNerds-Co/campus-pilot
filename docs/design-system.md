# Campus Pilot — Design System

**Version:** 1.0 · 20 Aug 2026  
**Inspiration:** `iamngoni/huchu` + `@corelithzw/react` elegance — translated, not copied.  
**Implementation:** CSS variables in `src/styles/tokens.css` + Tailwind v3.4.3 config + lightweight `src/components/ui/*` primitives. No private package required.

---

## 1. Philosophy (from huchu, adapted)

1. **One token, one truth.** If you type a hex, stop. Find the token. `src/styles/tokens.css` owns every colour, radius, shadow, and motion value. Tailwind maps to it (`tailwind.config.js` → `var(--*)`).
2. **Saturated colour means action or state, never decoration.** Page chrome is neutral — `--canvas` `#F7F8FA` with cards on `--surface` `#FFF`. Blue appears once per surface, on the primary action.
3. **Role tokens, not swatches.** Use `--text-strong / --text-body / --text-muted`, `--border / --border-strong / --hairline`, `--brand / --brand-soft / --brand-tint` — never `gray-400` or `blue-600` literals.
4. **Copy is load-bearing.** Sentence case everywhere. Button = verb. Empty state = what + why + next step. Toast = one sentence.
5. **A11y is structural:** `focus-visible` 2px `var(--focus-ring)` + 3px halo, 36px touch floor, `prefers-reduced-motion`, colour + icon + label together.

---

## 2. Tokens

All tokens live in `src/styles/tokens.css` at `:root` with a `.dark` override.

### 2.1 Surfaces

| Token | Light | Dark | Use |
|---|---|---|---|
| `--canvas` | `#F7F8FA` | `#0F1115` | Page background (`body`) |
| `--surface` | `#FFFFFF` | `#1A1D23` | Cards, popovers, inputs |
| `--surface-muted` | `#F1F3F6` | `#23272F` | Hover, skeletons, table head |
| `--surface-sunken` | `#E8EBF0` | `#2A2F3A` | Pressed, active states |
| `--surface-deep` | `#DDE1E7` | `#343A46` | Deeply inset |

Aliases (`--surface-app`, `--surface-panel`, etc.) all point at the ladder above so legacy screens keep working.

### 2.2 Text

| Token | Light | Dark |
|---|---|---|
| `--text-strong` | `#16181D` | `#F2F4F7` |
| `--text-body` | `#262A33` | `#D5D9E1` |
| `--text-muted` | `#565C69` | `#9AA0AD` |
| `--text-subtle` | `#5E6573` | `#7A8191` |
| `--text-inverse` | `#FFF` | `#0F1115` |
| `--text-link` | `#0944C2` | `#7BA4F5` |

`--text-primary/secondary/tertiary` are aliases.

### 2.3 Borders & edges

`--border` `#E5E8EE` · `--border-strong` `#D2D7E0` · `--border-subtle` `#EEF0F4` · `--hairline` `rgba(22,24,29,.08)`  
`--chrome-edge` / `--chrome-shadow` draw the sidebar/app-bar seam.

### 2.4 Brand

Campus blue refined from `blue-600 (#2563EB)` → **`#0B5DF0`** (Huchu family — same hue, more saturated, better contrast at larger sizes).

```
--brand:        #0B5DF0
--brand-strong: #0944C2
--brand-deeper: #08379C
--brand-soft:   #E8EFFE
--brand-tint:   rgba(11,93,240,.08)
--brand-50/100/200/300/400/500/700/900  full ramp
```

Dark: `--brand` → `#3B82F6`, soft → `rgba(59,130,246,.14)`.

### 2.5 Semantic tones

Each tone has `-bg` (wash), `-bd` (border), and the tone itself:

* **info** → brand
* **success** `#168052` / `#E6F4EC`
* **warn** `#B45309` / `#FEF3C7`
* **danger** `#B83A2A` / `#FEE2E2`
* **neutral** `#565C69` / `surface-muted`

Dark tones are same hue at 14–22% opacity washes.

### 2.6 Shape, type, motion, spacing, z

* **Radii:** `--radius-xs 4` / `sm 6` / `md 8` / `lg 10` / `xl 14` / `2xl 18` / `3xl 22` / `4xl 26` + `full/pill 9999px`.
* **Type:** `Inter var` (`--font-sans`) + monospace fallback. No `Montserrat` load (removed — was never fetched). Weights 400/500/600/700. Tabular-nums on IDs/money where needed (`font-variant-numeric: tabular-nums`).
* **Motion:** `--motion-ease-default cubic-bezier(0.2,0.8,0.2,1)`, durations 150/200/300ms. `prefers-reduced-motion` snaps to `0.01ms`.
* **Spacing:** `--space-1 4` / `2 8` / `3 12` / `4 16` / `5 20` / `6 24` / `8 32` / `10 40` / `12 48`. Gutters: `content-gutter-x 16`, `content-gutter-y 24`.
* **Z:** `--z-sidebar 50` / `--z-nav 60` / `--z-overlay 100` / `--z-toast 200`. Every overlay shares `--z-overlay`; open order decides stacking.
* **Elevation:** `--shadow-rest / -hover / -popover / -modal` — border at rest, shadow only when floating.

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
* Type: title `22/600/tight`, label `13/500`, body `14/relaxed`, caption `12`. Money/IDs may add `font-tabular` / `font-mono`.

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

* [ ] Palette reviewed (light + dark) — brand `#0B5DF0` / dark `#3B82F6` and tone washes
* [ ] Type scale + spacing rhythm approved
* [ ] `Button / Card / Badge / Input / Empty / Skeleton` examples approved
* [ ] `/style-guide` renders in `vite build` preview and `docker build -f client/Dockerfile`

---

*Hotspot note:* `components/theme-toggle.tsx` vs `lib/theme.tsx` is unified — first is now a re-export on the same `campuspilot-theme` key. `index.css` no longer has `* { transition-colors }`; transitions are scoped to `a/button/input/select/textarea`.
