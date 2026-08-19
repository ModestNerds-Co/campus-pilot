# Campus-Pilot Frontend Audit — Baseline vs Huchu Reference

**Date:** 20 Aug 2026  
**Auditor:** default agent (kanban t_a4f1e18c)  
**Scope:** Establish baseline elegance, extract reusable design language from `iamngoni/huchu`, and map overhaul batches.  
**Workspace:** `/Users/modestnerd/Developer/Projects/campus-pilot/client`  
**Build verification:** `vite build` — 1770 modules transformed, 40.21 kB CSS / 446.43 kB JS (gzip 6.92 / 130.99 kB) — passes. No TypeScript errors.

---

## 1. How this was audited

1. Ran `ls -R` + `package.json`/`vite.config`/`tailwind.config`/`index.css` inspection.
2. Read every route, layout, module screen, and shared component (`src/routes/*`, `src/modules/*`, `src/components/*`, `src/lib/*`).
3. Cloned `https://github.com/iamngoni/huchu` to `/tmp/huchu` (Next.js 16 / Tailwind 4 / `@corelithzw/react` DS, 57 `ui` primitives). Inspected `app/globals.css`, `app/themes/corelith-bridge.css`, `components/ui/*`, `components/layout/*`, `lib/utils.ts`, and `docs/design-system/*`.
4. Verified production build (`tsc && vite build`). No live-backend run was needed for structural audit; dev server (`vite --port 5179`) starts cleanly and the route guards redirect correctly (`/` → `/boot` → `/setup/*` → `/login` → `/admin`).

Screenshots in this doc are **representative renders derived from code inspection**. A live screenshot pass should be re-run once the API on `:8000` is reachable (the `boot`/`login` states currently 500 without backend — expected).

---

## 2. Campus-Pilot — current inventory

### 2.1 Routes (TanStack Router, file-based)

| Route | File | Guard | Description |
|---|---|---|---|
| `/` | `routes/index.tsx` | `bootstrapService.checkStatus()` | State machine: Uninitialized→`/setup/school`, SchoolConfigured→`/setup/admin`, Ready+auth→`/admin`, Ready+anon→`/login`, else→`/boot` |
| `/boot` | `routes/boot.tsx` | — | Bootstrap/loading-offline-error tri-state screen |
| `/setup/school` | `routes/setup.school.tsx` | — | 725-line school branding + contact + logo upload form |
| `/setup/admin` | `routes/setup.admin.tsx` | — | Admin provisioning (name/email/phone/password + strength meter) |
| `/login` | `routes/login.tsx` | — | Centered card login |
| `/admin` | `routes/admin.tsx` | `ProtectedRoute[Super Admin,Admin]` + `AdminLayout` | Shell (top bar + collapsible sidebar + `<Outlet/>`) |
| `/admin/` | `routes/admin/index.tsx` | inside admin | Dashboard |
| `/admin/users` | `routes/admin/users.tsx` | inside admin | Users list |
| `/admin/roles` | `routes/admin/roles.tsx` | inside admin | Roles list |
| `/dashboard` | `routes/dashboard.tsx` | — | Redirect → `/admin` |
| `__root` | `routes/__root.tsx` | — | Bare `<Outlet/>` + `TanStackRouterDevtools` |
| Not yet routed | `AdminLayout` nav | — | `/admin/departments`, `/admin/classes`, `/admin/staff`, `/admin/students`, `/admin/subjects`, `/admin/settings` — menu entries with no route files (404 today) |

### 2.2 Screens / layouts

| Surface | Component | Notes |
|---|---|---|
| Boot | `modules/configs/components/screens/boot-screen.tsx` | 232 lines. 4 states: loading (spinning `Loader2`), offline (`WifiOff` orange), error (`AlertTriangle` red), success (green). Absolute `ThemeToggle` top-right. Centered white card on `from-blue-50 via-white to-gray-50` gradient. |
| Login | `components/login-screen.tsx` | 255 lines. Same gradient. Centered card `max-w-md rounded-2xl shadow-lg border gray-100`. Branding circle `from-blue-100 to-blue-200` or school logo. Inputs with left icons (`Mail`/`Lock`), right eye-toggle, red error strip. Bottom "Powered by Campus Pilot" + logo. |
| School Setup | `modules/configs/components/screens/school-setup-screen.tsx` | 728 lines. `max-w-7xl` with header + two-column `flex lg:row`. Left: two stacked cards (Branding + Contact Information) with repeated `rounded-2xl shadow-lg`. Logo upload uses 2-column dashed `h-32` dropzone. Right: `SchoolPreviewCard` (live preview). Bottom: `Type → Country/Timezone/Locale` via custom `SearchableSelect`. Needs vertical-rhythm unification. |
| Admin Setup | `modules/configs/components/screens/admin-setup-screen.tsx` | Password strength bar + caps-lock detection (`checkCapsLock`). Back arrow `ArrowLeft`. |
| AdminLayout | `modules/admin/layouts/admin-layout.tsx` | 264 lines. Fixed top bar `h-16` (`bg-white border-b`), fixed sidebar `w-64 top-16` with `transform -translate-x-full lg:translate-x-0`, overlay scrim `bg-black/50 lg:hidden`, `main pt-16 lg:pl-64 p-4 sm:p-6 lg:p-8`. Expanding nav via `ChevronDown rotate-180`. ThemeToggle duplicated in top bar. |
| Dashboard | `modules/admin/components/admin-dashboard.tsx` | 268 lines. Header + date pill + 4 `StatCard`s (`grid-cols-1 md:2 lg:4 gap-6`), placeholder chart card `h-64 border-2 border-dashed`, two-col `Recent Activity | Quick Actions`, "Getting Started" gradient `from-blue-50 to-indigo-50`. Hard-coded `value="0"` stats. |
| Users | `modules/users/components/users-list.tsx` | Table `bg-white border rounded-lg overflow-hidden`. Header `bg-gray-50`, row hover `gray-50`, avatar circle `bg-blue-100`. Filters card + inline search + status `<select>`. Pagination border-t. Row dropdown on click (`w-48 shadow-lg border`). Empty state centered with icon. |
| Roles | `modules/users/components/roles-list.tsx` | Same table chrome as Users (duplication). |
| Modals | `user-form-modal.tsx`, `role-form-modal.tsx`, `changelog-modal.tsx` | Fixed inset `bg-black bg-opacity-50`, white `rounded-lg shadow-xl`, raw `X`/form handling — no shared modal primitive. |
| Shared | `searchable-select.tsx`, `theme-toggle.tsx`, `command-palette.tsx`, `protected-route.tsx` | Hand-rolled dropdown with `mousedown` outside handler, custom theme toggles (2 implementations), thin guard with spinner. |

### 2.3 Shared components

| Component | State |
|---|---|
| `theme.tsx` (`ThemeProvider` + `useTheme` + `ThemeToggle`) | Proper 3-way `light/dark/system` with `mediaQuery` listener, `localStorage` `campuspilot-theme`, `html.light/dark` + `meta[name=theme-color]`. Correct implementation — but `components/theme-toggle.tsx` duplicates it with a 2-way `light/dark` toggle on key `tgpatcher-theme` (legacy name leak, diverges by key + behaviour). |
| `searchable-select.tsx` (255 lines) | `mousedown` outside, `ref` focus, `filteredOptions`, `allowClear` × button, rotate chevron. Works but no `role="listbox"` / `aria-activedescendant`, no roving tabindex, no virtualisation. Huchu replaces this with `@corelithzw` `Combobox`/`Select`. |
| `components/*` (theme-toggle, command-palette, document-viewer, etc.) | Generally single-file concerns with ad-hoc Tailwind. |
| Toasts | `react-hot-toast` bottom-right, custom `success/error` `iconTheme`. Huchu uses `@radix toast` viewport at `z-toast=200`. |
| Stores | `auth-store.ts` (zustand `persist` on `campuspilot_auth`, access/refresh token + `expiresAt`, 401 refresh interceptor in `http-client.ts`). Clean. |

### 2.4 Design tokens (today)

All from `tailwind.config.js` + CSS variables (HSL-mapped):

```
--background / --foreground / --primary / --primary-foreground
--secondary / --muted / --accent / --destructive / --card / --popover
--border / --input / --ring / --radius
+ success / warning extra
```

Concrete values are **un-tokenised literals**: gradient `from-blue-50 via-white to-gray-50`, `gray-100/200/300` everywhere, `blue-600` primary, `rounded-xl` / `rounded-2xl` / `rounded-lg` used interchangeably, `shadow-lg` on setup/login cards but `shadow-md` nowhere else. No elevation scale, no surface stack, no motion tokens, no spacing scale beyond Tailwind defaults. Font: `Inter var` (`sans`) + `Montserrat` (`display`) — declared but `Montserrat` is never loaded (no `@import` and no `next/font`), so display falls back to Inter.

Tailwind itself is **v3.4.3** (stable) but `index.css` does `* { @apply transition-colors duration-200 }` — a universal transition that forces colour animations on every element (including those that shouldn't animate) and makes reduced-motion impossible to honour.

Dark mode: `darkMode: [class]` + duplicate `dark:` variants per class. Works, but every component hand-writes its dark values (e.g. `dark:bg-gray-800` vs `dark:bg-gray-700` vs `dark:bg-gray-900`) — no surface tokens to keep them consistent.

### 2.5 Navigation

`AdminLayout.navigationItems` — flat array with one level of `children`. Expand state is local `string[] expandedItems`. Active detection is exact `location.pathname === href` (no prefix match for nested routes). The sidebar owns its own mobile overlay; there is no command palette integration into nav search (palette exists but isn't wired).

### 2.6 What feels off (the "elegance gap" in plain language)

- **Two theme systems fighting.** The correct `ThemeProvider` is in `lib/theme.tsx`; the shippable `ThemeToggle` component in `components/theme-toggle.tsx` is a second, incompatible toggle that writes a different storage key. Whichever mounts last wins. This is the `hotspot` for this file.
- **Card chrome is inconsistent.** Login = `rounded-2xl shadow-lg border-gray-100 p-8`, Dashboard StatCard = `rounded-lg border-gray-200 p-6 hover:shadow-md`, Users table wrapper = `rounded-lg border`, Getting-Started = `from-blue-50 to-indigo-50 border-blue-200`. Four radii, four border colours, three shadows, two gradients — nothing ties them.
- **No empty/loading/error vocabulary.** Every list invents its own empty state (icon + two lines, sometimes `border-dashed`). Huchu has `Empty` with `variant` + action, the same in every module.
- **Tables duplicate.** Users and Roles render the same `<table>` Chrome separately (header `bg-gray-50`, avatar, badge, status dot, pagination). No `DataTable` primitive, no sticky header, no column picker, no `tabletScrollable` behaviour.
- **Spacing has no rhythm.** `p-8` on login, `p-6` on dashboard, `p-4` on filters, `space-y-6` vs `space-y-8` vs `gap-6` — no 8-point or `--space-*` scale.
- **Icons carry meaning by colour alone in places** (e.g. red/green status without label is borderline, though the badge text does rescue it). Huchu rule: colour + icon + label, never colour alone.
- **6 placeholder admin routes** are in the menu but 404 — the nav promises structure that doesn't exist yet.

---

## 3. Huchu reference — design language distilled

Cloned `iamngoni/huchu` @ `main` (Next.js 16, React 19, Tailwind 4, `@corelithzw/react` `^0.4.1` as visual source of truth). The following is the actionable kernel for Campus-Pilot.

### 3.1 Principles (from `docs/design-system/*`)

1. **Single source of truth is the package.** `@corelithzw/react` owns every colour, radius, shadow, motion value and component shape. App code references **tokens, never literals**. If you're typing a hex or a `px`, stop and find the token.
2. **Saturated colour = action or state, never decoration.** Page chrome is neutral (`--canvas` `#F7F8FA`, `--surface` `#FFFFFF`). Blue appears exactly once per surface, on the primary action.
3. **Role tokens, not swatches.** Use `--text-strong / --text-body / --text-muted / --text-subtle`, `--border / --border-strong / --border-subtle / --hairline`, `--brand / --brand-soft / --brand-tint` — not `gray-400` or `blue-600`.
4. **Copy is load-bearing.** Sentence case everywhere. Button = verb. Empty state = what + why + next action. Toast = one sentence, no end punctuation. Status labels are the canonical five: *Needs input · Running · Completed · Idle · Not started* — do not invent synonyms.
5. **Accessibility contracts.** `focus-visible` ring 2px `--focus-ring` + 3px `--focus-ring-soft` halo, modal focus trap, roving tabindex for composite widgets, `prefers-reduced-motion` snaps to final frame, touch target 36px (44px for mobile-primary), every icon-only button has `aria-label`.

### 3.2 Tokens (157) — the only values to use

| Group | Key tokens | Values |
|---|---|---|
| Surfaces | `--canvas`, `--surface`, `--surface-muted`, `--surface-sunken`, `--surface-deep` | `#F7F8FA`, `#FFFFFF`, `#F1F3F6`, `#E8EBF0`, `#DDE1E7` |
| Text | `--text-strong`, `--text-body`, `--text-muted`, `--text-subtle`, `--text-inverse`, `--text-link` | `#16181D`, `#262A33`, `#565C69`, `#5E6573`, `#FFF`, `#0944C2` |
| Borders | `--border`, `--border-strong`, `--border-subtle`, `--hairline` | `#E5E8EE`, `#D2D7E0`, `#EEF0F4`, `rgba(22,24,29,.08)` |
| Brand | `--brand`, `--brand-strong`, `--brand-deeper`, `--brand-soft`, `--brand-tint` | `#0B5DF0`, `#0944C2`, `#08379C`, `#E8EFFE`, `rgba(11,93,240,.08)` |
| Tones | `--tone-info / --tone-success / --tone-warn / --tone-danger` each with `-bg` + `-bd` | `info` = brand, `success` green, `warn` amber, `danger` `#B83A2A` |
| Actions | `--action-primary-*`, `--action-secondary-*`, `--action-destructive-*` + soft destructive (`#6B655A`) | Typed per-action |
| Shape | `--radius-xs 4` / `sm 6` / `md 8` / `lg 10` / `xl 14` / `2xl 18` + `--radius-full/pill` | 4/6/8/10/14/18 |
| Type | Atkinson Hyperlegible (Google Fonts load in `globals.css`) — `--font-sans` + `--font-mono`, weights 200–800 | Loaded once, not per-component |
| Motion | `--motion-ease-default`, `--motion-duration-*` | Package-owned; respect `prefers-reduced-motion` |
| Spacing | `--space-*` + `--content-gutter-y` | App reads `var(--content-gutter-y)` in `AppShell` |
| Z | `--z-sidebar 50` / `--z-nav 60` / `--z-overlay 100` / `--z-toast 200` — one rung per overlay kind, open order decides stacking | `p-drawer-overlay` uses `!important` to beat inline `zIndex:1100` |
| Bridge | `app/themes/corelith-bridge.css` maps legacy names (`--neutral-*/--primary-*/--success-*/--surface-*`) onto package tokens without literals | `NO LITERALS` — every value is `var(--package-token)` or `color-mix` |

Font-size/leading/weight are inside package `font:` shorthands; campus-pilot should not re-declare them.

### 3.3 Layout

```
AppShell
  isAuthRoute | isMarketingRoute | isPortalRoute | isAdminRoute | isPublicRoute | isPreviewHostRoute
    → bare <div min-h-screen bg-background>  (no chrome)
  else
    SidebarProvider (cookie sidebar:state + mobile drawer)
      AppSidebar (pinned aside on desktop, drawer on mobile — useMediaQuery, hidden on one frame pre-hydration)
      SidebarInset  "shadow-[inset_1px_0_0_0_var(--chrome-edge)]"  // seam, not border
        Navbar
        main.content-shell  flex-1 overflow-y-auto pt-[var(--content-gutter-y)] pb-[max(...safe-area...)]
          RecordTrailProvider → RecordPeekProvider → OnboardingProvider → {children}
```

Key refinements for Campus-Pilot to steal:

- **No desktop rail flash on mobile.** `.sidebar:not(.p-drawer .sidebar){display:none}` below 768px until `useMediaQuery` hydrates. Campus-Pilot today flashes the rail edge on resize.
- **One overlay z per kind** (`--z-overlay`) — newest portal paints on top. Fixes menu-behind-sidebar when the sidebar is a drawer.
- **Flat inset, not framed card.** `SidebarInset` has no gutter/rounding — the workspace edge *is* the surface edge; the only line is the inset hairline seam.
- **Bottom sheets auto-height:** `height:auto; max-height:min(var(--drawer-size,420px), calc(100dvh - 3rem)); border-top-radius:var(--radius-2xl); padding-bottom:env(safe-area-inset-bottom)` — short content hugs, long content scrolls, safe-area respected.

### 3.4 Components (57 primitives under `components/ui` + `@corelithzw/react`)

Significant for Campus-Pilot:

| Primitive | Huchu pattern | Campus-Pilot delta |
|---|---|---|
| `Button` | `@corelithzw Button` with `variant: primary/secondary/ghost/quiet/link/destructive`, `size sm/md/lg/icon*`, `Slot` for link-as-button | Campus-Pilot has no `Button` primitive at all — every call site writes `px-4 py-2 bg-blue-600 hover:bg-blue-700 rounded-lg` by hand |
| `Card` | DS `.card` / `.card-head` / `.card-title` / `.card-sub` / `.card-body` / `.card-foot` compound (styling is package, composition stays local) | Hand-written `bg-white border rounded-lg p-6` each time |
| `Dialog / Sheet` | `Base UI Dialog` engine + DS `.modal-scrim` / `.modal-card size-*` / `.modal-h` / `.modal-f` + `.x` close; open order decides z | Fixed `bg-black bg-opacity-50` + raw `fixed inset-0` — no trap, no scroll lock, no `size`, no shared close |
| `Input / Select / Popover` | DS `Input`/`Select` with `data-slot="input"` (load-bearing for themed CSS) | Custom input styles per screen; `SearchableSelect` is hand-rolled |
| `Badge / StatusChip / StatusDot` | `tone: neutral/info/success/warn/danger/brand` + optional dot; `dot + label` required, `badgeVariants` shim preserves `variant` call sites | Inline `px-2 py-1 rounded-full bg-purple-100 text-purple-700` per role |
| `Table / DataTable` | `.dtable sticky-head` with `tabletScrollable/minTableWidth`, `stickyHeaderOffset`, pagination `mode`, `persistKey`, `pageSizeOptions`, `column-picker`, `mobile-list` swap | Flat `<table>` with none of the above |
| `PageSection / PageHeading / DetailPageShell / ListPageShell` | Consistent chrome for list vs detail vs settings views | Campus-Pilot has none — `AdminDashboard` hand-builds header + date pill |
| `Sidebar / SidebarProvider / SidebarInset` | Collapsible with cookie persistence, mobile drawer, `useIsMobile`, `toggleSidebar` | Ad-hoc `useState` + manual `translate-x` |
| `Tabs / SegmentedControl / SectionTabs` | Roving tabindex, single tab stop, arrows navigate inside | Not present |
| `Skeleton / Empty / Progress` | Shared empty + loading + progress vocabulary | Each list invents its own |
| `Command` (cmdk) | `⌘K` palette from any surface | `command-palette.tsx` exists but is not wired to `⌘K` |
| `Toast / Toaster` | Radix toast viewport at `--z-toast` | `react-hot-toast` floating toasts (different z) |

### 3.5 Whitespace, responsiveness, animation

- **Spacing:** package `--space-*` + Tailwind utilities `p-*`/`gap-*` on the same cadence; `content-shell` owns page gutter so child pages don't guess.
- **Breakpoints:** standard `sm/md/lg/xl` plus `ms:520px` and `3xl:1920px` for POS. Campus-Pilot uses vanilla Tailwind breakpoints — keep that, just harmonise `ms` if POS-style list rows ever appear.
- **Animation:** package `animate-in fade-in-0 zoom-in-[0.985] duration-200 ease-[var(--motion-ease-default)]` on modals; reduced-motion snaps to `duration 0.01ms`. Campus-Pilot's global `* { transition-colors 200ms }` overrides nothing like this — it just taxes layout.
- **Elevation:** `shadow-sm` to `shadow-lg` scale from tokens; cards sit on `--surface` at `shadow-card` — not the bare `shadow-lg` campus-pilot uses on login.
- **Tables:** `dtable` collapses to `mobile-list` below a breakpoint; `tabletStickyFirstColumn` pins the name column. Campus-Pilot's `overflow-x-auto` is the only responsive affordance.

---

## 4. Gap analysis — current → target

| # | Dimension | Campus-Pilot today | Huchu target | Gap severity |
|---|---|---|---|---|
| 1 | Token system | HSL vars with ad-hoc `blue-600/gray-100` literals, 4 radii, 3 border colours | 157 package tokens, one radius/shadow/colour per role, bridge maps legacy names | **High** — every future change forks unless fixed first |
| 2 | Typography | `Inter var` (good) + unused `Montserrat` display, no mono, no loaded `@import`, tabular-nums nowhere | Atkinson Hyperlegible `sans+mono`, weights 200–800, tabular-nums on IDs/money, mono on IDs | Medium |
| 3 | Global shell | Two theme providers fighting (key mismatch), universal `* transition-colors`, bare `__root` | Single `ThemeProvider` + `app-shell` seam + single 200 ms colour + reduced-motion | High (behavioural bug) |
| 4 | Navigation | Fixed `w-64` + manual expand + exact-path active, no `⌘K`, 6 dead links | `SidebarProvider` (cookie persisted, collapsible rail, mobile drawer), `⌘K` palette | High |
| 5 | Cards/containers | 4 variants with different radius/shadow/padding/gradient | One `.card` recipe with `card-head/title/sub/body/foot` | Medium |
| 6 | Tables/lists | Duplicate `Users`/`Roles` tables, no DataTable | `.dtable` with pagination state, `tabletScrollable`, sticky header, column picker, `mobile-list` | High |
| 7 | Forms | Per-screen input CSS, hand-rolled `SearchableSelect`, no `Input`/`Select` primitive | DS `Input`/`Select`/`Combobox`/`Textarea` with `data-slot`, `leadingIcon`, `label` | Medium |
| 8 | Overlays | Raw fixed overlay + `X` close | DS `Dialog`/`Sheet` on Base UI with `.modal-card size-*`, scrim, trap, open-order z | Medium |
| 9 | Feedback/empty | Inline invented empty (icon+2 lines) and `Loader2` per list | `Empty` + `Skeleton` + `Toast` viewport at `z-toast` + consistent status vocabulary | Low-Medium |
|10 | Dashboard | Hard-coded zero stats, `border-dashed` chart placeholder, "Getting Started" tinted block | `PageHeading` + stat metric cards + `Visx` charts + `PageSection` rail | Low (content signal today) |
|11 | Motion/a11y | No focus-visible ring spec, icons not always labelled, touch  `py-3` ~36px inconsistently | 2px focus ring + 3px halo, 36/44px targets, roving tabindex, colour+icon+label | Medium |
|12 | Build hygiene | v3 Tailwind, universal transition, `tgpatcher-theme` legacy leak | Tailwind 4, layered `@layer theme,base,corelith,app,components,utilities`, no literal | Ongoing |

### Representative screens (what a screenshot would show)

- **Boot screen:** centre column card `max-w-md` over `blue-50→white→gray-50` gradient, 4 exclusive states. Target would sit on `--canvas` with a single `card-body` card at `var(--radius-2xl)` and a single `Loader2` at `--brand`.
- **Login:** same gradient + `max-w-md rounded-2xl shadow-lg p-8` card. Target removes the gradient in favour of `--canvas` + `--surface` card, inputs become `<Input leadingIcon={<Mail/>}>` with `data-slot`, button becomes `<Button variant="primary" size="lg" className="w-full">`.
- **School Setup:** `max-w-7xl` header + `lg:flex-row gap-8` two cards with logo dropzones. Target collapses the two dashed dropzones to one `Input type=file` pair with DS preview, form fields to DS `Input`/`Select`/`Textarea`, wrapper to `ListPageShell` with `PageHeading`.
- **AdminLayout:** fixed `h-16` bar + `w-64` drawer + `pt-16 lg:pl-64`. Target removes the extra outer border/padding in favour of the `SidebarInset` inset-shadow seam and `content-shell` gutter.
- **Dashboard:** `grid-cols-4` `StatCard`s with `bg-*-50` icon wells + dashed chart + `Recent Activity` + `Quick Actions`. Target keeps the grid but each `StatCard` becomes a `.card` metric, the dashed block becomes `<Empty>` or `<Skeleton>` chart, quick actions become `Button secondary` stack, getting-started becomes a `PageSection` with `step-progress`.
- **Users/Roles lists:** `bg-white border rounded-lg overflow-hidden` table with `bg-gray-50` head. Target becomes `<Table tabletScrollable>` with `.dtable`, `StatusChip` on status, `BadgeGroup` on roles, row actions in `<DropdownMenu>`.

---

## 5. Reusable patterns to lift from Huchu

Ordered by ROI for Campus-Pilot.

1. **Token bridge + layered stylesheet.** Copy `app/globals.css` layer order and `app/themes/corelith-bridge.css` pattern: import `@corelithzw/react/styles.css` in `@layer corelith`, map every legacy `neutral-*/primary-*/success-*/border-*` name onto package tokens via `var()` / `color-mix` (no literals). This is the precondition for every other polish — without it each component quietly forks.
2. **`cn` + `layer` hygiene.** One `lib/utils.ts cn = twMerge(clsx)` and the `@layer theme,base,corelith,app,components,utilities` declaration (duplicated in the entry and the bridge so bundler order doesn't invert it). Already present but verify the order reaches the browser first.
3. **`Button` + `Badge` + `StatusChip` shims.** The lowest-friction win: keep local `variant/success/warning` prop names, map them onto DS tones behind the shim (`components/ui/button.tsx` / `badge.tsx` pattern). ~212 button call sites in Huchu show this migrates without a call-site churn.
4. **`Card` compound.** Replace `bg-white border rounded-* p-*` per surface with `<Card><CardHeader><CardTitle><CardDescription><CardContent><CardFooter>` that writes `.card*` classes. Fixes the four-card-chrome divergence in one import.
5. **`Input / Textarea / Select / Combobox` with `data-slot`.** Eliminates per-screen input CSS and the hand-rolled `SearchableSelect`; the `data-slot="input"` selector is what themed CSS keys off (`[data-portal="admin"] [data-slot="input"]`).
6. **`Table / DataTable` primitive.** Single `rowCount`-aware table that owns pagination, `stickyHeaderOffset`, `tabletScrollable`, `persistKey`, and swaps to `mobile-list` below `md`.
7. **`Dialog / Sheet` with size + mobile `max-h-[100dvh]`.** One place defines `.modal-scrim`/`.modal-card` and bottom-sheet auto-height/safe-area logic — every modal/sheet inherits it.
8. **`AppShell` + `SidebarProvider` + `PageHeading` + `ListPageShell`/`DetailPageShell`.** Turns every list page (Users, Roles, future Students/Departments) into the same heading/toolbar/table/footer frame. The `isAuthRoute/isMarketingRoute/...` predicate keeps boot/login/setup chrome-free.
9. **`Empty` + `Skeleton` + `Toast` viewport at `--z-toast`.** Replace per-list invented empty and per-list `Loader2` + floating `react-hot-toast` with the DS pair and one viewport at `--z-toast 200`.
10. **`z` scale + single overlay rung.** Declare `:root { --z-sidebar:50 --z-nav:60 --z-overlay:100 --z-toast:200 }` and put every overlay at `--z-overlay` (and draw `!important` on any inline `zIndex:1100` that leaks in from DS drawer). Fixes menu-behind-sidebar automatically.
11. **`Command` (`cmdk`) palette.** Wire `⌘K` to palette + expose navigation search, user search, and quick creates — scaffolding already exists (`command-palette.tsx` + `global-keyboard-handler.tsx`) but isn't bound to `⌘K`.

---

## 6. Prioritised overhaul scope — batches

Each batch is sized to be a single PR that builds and ships.

### Batch 0 — Stabilise the shell (half-day, no visual change)
- [ ] Delete `components/theme-toggle.tsx` legacy toggle or merge it onto `lib/theme.tsx`; unify storage key to `campuspilot-theme` and remove `tgpatcher-theme` read.
- [ ] Remove `* { @apply transition-colors duration-200 }` from `index.css`; keep it only inside `@layer app` on `body`/`a`/`button` where intended, and add `@media (prefers-reduced-motion:reduce)` reset.
- [ ] Add explicit `aliases` parity (`@ → ./src`) already in `vite.config.js` — ensure `tsconfig.json` `paths` matches it; enable `eslint` for `@/*` sort.
- [ ] Thin harnesses: `RouteWrapper` unused? either wire it or remove.
- **Acceptance:** `vite build` + `tsc --noEmit` + no flicker on refresh; `localStorage` has only `campuspilot-theme`.

### Batch 1 — Design tokens (foundation, unlocks all batches)
- [ ] Upgrade Tailwind to `^4.1` (as in Huchu) or keep `3.4.3` but adopt the **bridge pattern**: create `src/themes/corelith-bridge.css` mapping every legacy name (`--neutral-*`, `--primary-*`, `--success-*`, `--surface-*`, `--radius-*`) onto new role tokens with `var()`/`color-mix` — zero literals.
- [ ] Rewrite `index.css` to `@layer theme,base,corelith,app,components,utilities` and `@import "@corelithzw/react/styles.css" layer(corelith)` + bridge + `globals.css` ordering fresh from Huchu's entry.
- [ ] Declare `:root { --z-sidebar:50 --z-nav:60 --z-overlay:100 --z-toast:200 }` and `meta[name=theme-color]` from `ThemeProvider`.
- [ ] Replace gradient page backgrounds (`bg-gradient-to-br from-blue-50 via-white to-gray-50`) with `bg-canvas` (`var(--canvas) #F7F8FA`) + card on `bg-surface`.
- **Acceptance:** No hex/`blue-600`/`gray-100` literal in new code passes `grep -R "#"`
; snapshot of `:root` dumps 157 token names and every legacy alias resolves.

### Batch 2 — Primitives I: Button · Card · Badge · Status (visible everywhere)
- [ ] Add `src/components/ui/{button,card,badge,status-chip,status-dot}.tsx` shims mapping local props (`variant/size`) onto DS classes exactly as Huchu does (keep `variant/destructive/success/warning` so call sites don't churn).
- [ ] Replace all hand-written `px-4 py-2 bg-blue-600 … rounded-lg` call sites (search: `bg-blue-600 hover:bg-blue-700`) with `<Button>`; roles badges and active/inactive pills with `<Badge tone=…>` + `<StatusDot>`.
- [ ] Wrap `Login` card, `Dashboard StatCard`, `Users` filter card, setup cards all with `<Card>` compound.
- **Acceptance:** `grep -R "bg-blue-600" src` drops to 0 in app code (only DS internals remain); Storybook or `vite` preview shows a single card radius/shadow.

### Batch 3 — Forms (school + admin setup)
- [ ] Add DS `Input`/`Textarea`/`Select`/`Combobox` (keep `data-slot="input"`).
- [ ] Retire `SearchableSelect` (or shim it onto DS `Combobox` while preserving `number|null` API) and wire Country/Timezone/Locale there; fix the numeric `id` lookup indirection (Huchu's options use stable `value` as key).
- [ ] Replace per-field `border rounded-xl focus:ring-2` with `<Input leadingIcon=…>` + `aria-invalid` + `role="alert"` on the error line; drop the `validateImage` toast-spam loop (aggregate warnings).
- **Acceptance:** All required fields have `<label for>`; icon-only buttons have `aria-label`; keyboard Tab order matches visual order; `validateEmail/validatePhone/validatePassword` paths unchanged server-side.

### Batch 4 — Overlays (modals + sheets)
- [ ] Add `src/components/ui/{dialog,sheet,dropdown-menu}.tsx` on Base UI `@base-ui/react` engine (as in Huchu) with `.modal-scrim`/`.modal-card size-*`/`.modal-h`/`.modal-f` + `size xl/md`, `inset`, `tabletBehavior`.
- [ ] Replace `UserFormModal`/`RoleFormModal`/`ChangelogModal` fixed overlays with `<Dialog>` + focus trap + Escape-to-close + focus-return-to-trigger.
- [ ] Add bottom-sheet mobile rule (`drawer-bottom { height:auto; max-height:min(...) }`) for any future sheet.
- **Acceptance:** No `fixed inset-0 bg-black bg-opacity-50` literal outside DS wrapper; Escape on modal returns focus to the trigger button.

### Batch 5 — Navigation + Shell
- [ ] Add `src/components/layout/{app-shell,app-sidebar,navbar,page-heading,page-chrome}.tsx` + `hooks/use-mobile.tsx` pattern from Huchu; switch `AdminLayout` to `SidebarProvider` (cookie `sidebar:state`) + `SidebarInset` seam (`shadow-[inset_1px_0_0_0_var(--chrome-edge)]`) + `content-shell`.
- [ ] Wire `⌘K` palette (already scaffolded) to search nav + users + roles; expose quick create.
- [ ] Keep `/admin` guard via `ProtectedRoute` but let `AppShell` predicate exempt `/boot`/`/login`/`/setup/*` from chrome (as Huchu exempts `isAuthRoute/isPublicRoute`).
- [ ] Remove the 6 dead nav entries or stub their pages with `<Empty>` so the menu never 404s.
- **Acceptance:** Sidebar collapses to rail, persists on reload; mobile path shows drawer (no rail flash); `⌘K` opens; every `/admin/*` child shares the same heading/toolbar frame.

### Batch 6 — Data tables / lists
- [ ] Add `src/components/ui/{table,data-table,mobile-list,column-picker,skeleton,empty}.tsx` (`dtable` recipe) with `tabletScrollable`, `stickyHeaderOffset`, `persistKey`, pagination `mode`, and `mobile-list` collapse.
- [ ] Merge `UsersList` and `RolesList` to consume one `<DataTable>` (shared header/row/pagination/empty/skeleton).
- [ ] Move role colours, active dot, and avatar initials onto `StatusChip`/`StatusDot` inside the table, not raw `<span>`s.
- **Acceptance:** Long Users table pages at 25/50/100 rows without DOM cloning; mobile renders list cards; header sticks.

### Batch 7 — Dashboard & content pages
- [ ] Replace `StatCard` hand-build with `<Card>` metric variant; wire live counts when APIs exist (remove hard-coded `"0"`); add `Skeleton` until first fetch.
- [ ] Replace dashed `h-64` chart placeholder with `chart.js` already-installed but now themed (brand `--tone-info` series, `--text-subtle` axis) or `Visx` if preferred; add `PageSection` rail for chart vs recent activity.
- [ ] Standardise Quick Actions as `Button variant="secondary"` stack; make Getting Started an ordered `StepProgress` with check completion (mirrors Huchu's `WorkflowStep`).
- **Acceptance:** Dashboard loads without flicker; dark mode charts keep contrast; every card shares one radius/elevation.

### Batch 8 — Motion, a11y, polish (cross-cutting)
- [ ] Add `focus-visible: 2px solid --focus-ring + 3px --focus-ring-soft` halo globally, and remove every `outline:none` without replacement.
- [ ] Enforce 36px touch floor (`--h-control-md`) on buttons/inputs (44px on mobile primary surfaces like Login CTA); audit with `a11y` scan.
- [ ] Replace `transition-colors 200` on `*` with scoped transitions (`button`, `a`, `.card`) using `ease-[var(--motion-ease-default)]`; gate on `prefers-reduced-motion`.
- [ ] Normalize copy to sentence case + the five canonical status labels; add `font-variant-numeric: tabular-nums` on IDs/money, `font-mono` on IDs.
- **Acceptance:** Lighthouse a11y ≥ 95, no `prefers-reduced-motion` violation, no icon-only button without `aria-label`.

**Suggested sequencing:** `0 → 1 → 2 → 3 → 5 → 4/6 → 7 → 8`. Token bridge must land before any primitive — otherwise each batch quietly forks the palette again. Nav shell (5) should land before tables (6) so tables inherit `content-shell` gutter.

**hotspot:** `client/src/components/theme-toggle.tsx` and `client/src/lib/theme.tsx` both own theming on different storage keys (`tgpatcher-theme` vs `campuspilot-theme`) with different `light/dark/system` semantics — every shell change keeps colliding with the wrong provider.

---

## 7. Risks / caveats

- Tailwind v4 in Huchu vs v3 in Campus-Pilot — layer semantics change between them; the bridge fixes the layer order but the PostCSS pipeline still needs `tailwindcss ^4` + `@tailwindcss/postcss ^4` to match Huchu exactly. Adopting the bridge without the upgrade keeps literal colours working but loses `tw-animate-css`.
- `@corelithzw/react` is a private package — pulling it in requires access to the Codecraft registry token (as Huchu has). If unavailable, treat Batch 1 as "vendor the tokens into repo `src/tokens.css`" and pin the values — still a reversible seam.
- Live screenshot capture needs the API at `VITE_API_BASE_URL` reachable — without it `BootScreen` sits in `error`/`offline` and `Users` stays on its empty state. The inventory above is structural; visual delta should be re-shot after backend boot.

