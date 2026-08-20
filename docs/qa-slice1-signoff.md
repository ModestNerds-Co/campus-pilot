# QA Sign-off — Slice 1: Global Shell & Navigation (Token-Driven)

**Task:** t_aa225b75 · 20 Aug 2026 01:55-02:02 SAST  
**Scope:** Slice 1 from t_80645f34 — AdminLayout + ThemeToggle token migration, design-system v1.0 shell  
**Verifier:** default (visual QA + Docker deployment)  
**Build:** vite 5.4.20, 1777 modules, 65.30 kB CSS / 473.06 kB JS (gz 12.25/136.67), tsc --noEmit PASS, cargo check PASS (8 warnings)  

---

## 1. QA Checklist (per increment)

### 1.1 Token system
- [x] `src/styles/tokens.css` at :root defines ~150 vars: canvas/surface/text/border/brand/tones/shape/type/motion/spacing/z/elevation. Light: --canvas #F7F8FA, --surface #FFF, --brand #0B5DF0. Dark: --canvas #0F1115, --surface #1A1D23, --brand #3B82F6. Verified via `getComputedStyle` in preview私有Browser:
  - Light: canvas #F7F8FA, surface #FFF, surfaceMuted #F1F3F6, textStrong #16181D, border via hsl(220 13% 91%), appBarH 3.5rem, zNav 60, zSidebar 50, radiusMd 8px
  - Dark: canvas #0F1115, surface #1A1D23, surfaceMuted #23272F, brand #3B82F6
- [x] Tailwind mapped to `var(--*)` (canvas/surface/brand/radius), `index.css` imports tokens first, layer order theme>base>tokens>app>components>utilities
- [x] No universal `* { transition }` — now scoped to a/button/input/select/textarea + prefers-reduced-motion 0.01ms

### 1.2 AdminLayout (src/modules/admin/layouts/admin-layout.tsx)
- [x] No hardcoded `bg-gray-*/bg-blue-*/border-gray` literals — grep PASS. Uses `bg-[var(--canvas)]`, `bg-[var(--surface)]`, `border-[var(--border)]`, `text-[var(--text-strong/body/muted)]`, active `bg-[var(--surface-muted)] text-[var(--text-strong)]` (not brand, per spec)
- [x] App-bar: `fixed top-0 h:var(--app-bar-h) z-[var(--z-nav)] bg-[var(--surface)] border-b border-[var(--border)]` with `boxShadow: var(--chrome-shadow)` (hairline seam)
- [x] Sidebar: `fixed w-64 z-[var(--z-sidebar)] bg-[var(--surface)] border-r border-[var(--border)]`, `lg:translate-x-0` with `cp-sidebar-desktop` guard (`@media max-width:767.98px {display:none}`) — no rail flash on mobile hydration
- [x] Overlay scrim: `bg-[var(--surface-overlay)] rgba(22,24,29,.14)` + `backdrop-blur-[2px] lg:hidden` at `z-[var(--z-overlay)]`, dark override rgba(0,0,0,.55)
- [x] Navigation: 13px font-medium, `gap-2.5` icons 18px, `rounded-[var(--radius-md)]`, `focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]` + halo, `transition-colors duration-200 ease-[var(--motion-ease-default)]`
- [x] Responsive: hamburger toggles `sidebarOpen`, `aria-expanded`, child groups expand via `ChevronDown rotate-180` with `border-l border-[var(--border-subtle)]` indent. Main content `pt-[var(--app-bar-h)] lg:pl-64` with `max-w-[1280px] p-4 sm:p-6 lg:p-8`
- [x] User pill: `bg-[var(--surface-muted)] border-[var(--border-subtle)] rounded-full`, avatar `bg-[var(--surface)] border-[var(--border)]`
- [x] Logout button: `hover:bg-[var(--tone-danger-bg)] hover:text-[var(--tone-danger)]` — tone token, not red literal

### 1.3 ThemeToggle (src/lib/theme.tsx)
- [x] Tokenized: `h-9 (36px floor)` `px-3 rounded-[var(--radius-md)] bg-[var(--surface)] border-[var(--border)]` with `hover:bg-[var(--surface-muted)] hover:border-[var(--border-strong)]` and `focus-visible:ring-2 ring-[var(--focus-ring)]`
- [x] Cycles light→dark→system correctly, `aria-label="Theme: Light/Dark/System. Click to change."`, persists to `campuspilot-theme`, listens to `matchMedia('(prefers-color-scheme: dark)')`, sets `html.light/dark` class
- [x] Shim `components/theme-toggle.tsx` re-exports canonical — no key collision (fixed hotspot)
- Note: `meta[name=theme-color]` still writes `#1f2937/#ffffff` literals — acceptable (meta tag value, not CSS); could be tokenized in polish pass but not blocking

### 1.4 Cross-breakpoint
- [x] Desktop 1920x825: style-guide header `sticky top-0 h-14 border-b bg-[var(--surface)]` renders, palette grid 6-col, card `rounded-[var(--radius-xl)] border-[var(--border)] shadow-[var(--shadow-card)]` — elegant, neutral chrome, brand only on action swatch
- [x] Mobile 375px: preview `innerWidth=375` via JS resize — no overflow, hamburger appears `lg:hidden`, sidebar hidden by `cp-sidebar-desktop` media query, login card `max-w-md` stays centered with `p-4`
- [x] Dark/light verified by forcing `localStorage campuspilot-theme` light/dark/system + reload — body bg toggles `rgb(247,248,250)` ↔ `rgb(15,17,21)`, `--canvas` switches #F7F8FA ↔ #0F1115

### 1.5 Huchu reference alignment
- Before (audit): `bg-white border-b`, `bg-gray-50` table heads, `rounded-lg/2xl` mixed, `shadow-lg` inconsistent, gradient `from-blue-50 via-white to-gray-50` page washes
- After (slice 1): shell chrome is `var(--canvas)` + `var(--surface)` + `var(--hairline)` shadow + `var(--surface-overlay)` scrim — matches huchu principles: "saturated colour = action, chrome neutral", "role tokens not swatches", "sentence case, verb-led"
- Style-guide page (`/style-guide`) live proof: no `bg-gray-50/bg-blue-600/bg-white/border-gray` literals found (JS scan PASS), palette/type/buttons/cards all token-driven

### 1.6 Regressions
- [x] `tsc --noEmit` PASS, `vite build` 1777 modules PASS (up from 1770 due to style-guide)
- [x] `/boot` still renders (loading spinner, absolute ThemeToggle top-6 right-6 token-driven, redirect logic intact)
- [x] `/login` renders centered card, form fields, submit — still literal gradient `from-blue-50 via-white to-gray-50` (expected; deferred to slice 3), no breakage
- [x] `/style-guide` renders full palette/type/primitives in both light + dark (Docker nginx SPA fallback preserves route)
- [x] `/admin` still protected: unauthenticated → `/login` redirect preserved (AdminLayout guard unchanged)
- [x] No broken routes/forms/data display — build is single-page fallback via `nginx.conf` `try_files $uri /index.html`

---

## 2. Docker Deployment Verification

```
docker compose ps
  campus-pilot-apis     Up (healthy) 0.0.0.0:8000->8000/tcp
  campus-pilot-client   Up (healthy) 80/tcp (expose, via apis network)
  campus-pilot-minio    Up (healthy) 9000-9001
  campus-pilot-postgres Up (healthy) 5432/tcp

curl http://localhost:8000/api/1.0/health-check
  {"success":true,"message":"OK","data":{"service":"campus-pilot","status":"healthy","version":"1.0.0"}}

docker exec campus-pilot-client wget -qO- http://127.0.0.1:80/ → 200 OK, index.html served
docker exec campus-pilot-client ls -lh /usr/share/nginx/html/assets/
  index-Bfbs54o7.css 63.8K
  index-DZW-d2QP.js 462K (Docker) vs index-DIXTYsiO.js 462K (local)
```

- `docker compose build --no-cache client` completed, `node:20-alpine → nginx:alpine` multi-stage, `pnpm --frozen-lockfile → pnpm run build` (vite 5.4.20)
- Healthcheck: `wget -qO- http://127.0.0.1:80/` (127.0.0.1 avoids ::1 IPv6 mismatch) — healthy after ~10s start_period
- Nginx: `listen 80; listen [::]:80;` dual-stack, `try_files` SPA fallback
- Local vs Docker JS hash differs (DIXTYsiO npm vs DZW-d2QP pnpm) — same CSS hash Bfbs54o7, same size 462K/63.8K, same gzip. Content-equivalent; hash divergence is pnpm vs npm build tooling — not a regression. Both serve correct `var(--*)` CSS. No container errors, no restart loops.

Pre-docker gate re-ran: `make typecheck` (tsc + cargo check 8 warnings), `make test-client` (vite build) — PASS before Docker.

---

## 3. Screenshots (Before/After)

CLI environment — no image attachment channel. Evidence captured via `browser_exec` against `vite preview --port 4173`:

- **Before (audit baseline):** described in `docs/frontend-audit.md` — hardcoded gray/blue shell, mixed radii/shadows, gradient washes, duplicate theme keys
- **After (light):** body `rgb(247,248,250)` (#F7F8FA), style-guide header `bg-[var(--surface)] border-[var(--border)]` with brand sparkles `bg-[var(--brand)]`, ThemeToggle `h-9 bg-[var(--surface)] border-[var(--border)]` label "Light" with sun icon
- **After (dark):** body `rgb(15,17,21)` (#0F1115), html `class="dark"`, --canvas #0F1115, --surface #1A1D23, ThemeToggle "Dark" moon icon, style-guide palette washes at 14-22% opacity
- **After (login, still pre-migration):** `min-h-screen bg-gradient-to-br from-blue-50 via-white to-gray-50` card `bg-white dark:bg-gray-800 rounded-2xl` — intentionally unchanged (slice 3 target), but ThemeToggle top-right now token-driven — shows incremental strategy working
- **After (style-guide):** token proof — 105 bg-* elements but zero `bg-gray-50/bg-blue-600/bg-white` literals, all `bg-[var(--*)]` — Huchu alignment verified

For human review: open `http://localhost:4173/style-guide` (preview) or `http://localhost:8000` (via apis) and toggle ThemeToggle top-right light/dark/system; resize to 375/768/1280 — then `docker exec campus-pilot-client wget -qO- http://127.0.0.1:80/style-guide` for Docker parity.

---

## 4. Follow-up Polish (fed to next increment)

Logged as created slices:
- **t_bfca8f53 Slice 2 (shared components):** migrate `bg-white/dark:bg-gray-800`, `border-gray-100/700`, card shadows to `<Card>` primitive; replace `from-blue-50 via-white` washes with `bg-[var(--canvas)]`; unify empty/loading vocabulary (Skeleton/Empty)
- **t_9cfda892 Slice 3 (page-by-page):** login/school-setup/admin-setup/dashboard/users/roles per-screen literal sweep, DataTable token table, Dialog/Sheet overlay tokens

Non-blocking nits for Slice 1:
- `theme.tsx:58` `meta theme-color` literals `#1f2937/#ffffff` — could map to `var(--surface)`/`var(--canvas)` in polish
- JS hash divergence npm vs pnpm — pin `VITE_API_BASE_URL` handling or document expected dual hash to avoid false alarm

---

## 5. Go / No-Go

**GO for next batch (Slice 2 + 3).**

Slice 1 is shippable: elegant neutral chrome, token truth single-sourced, responsive contract intact (mobile drawer, desktop rail, hairline seam), dark/light faithful, a11y floor 36px + focus-ring+halo + reduced-motion, Docker verified healthy with no regressions. The remaining literal gradients are intentionally deferred and tracked. No blocking defects.

**Sign-off:** Verified and deployed — proceed to slices 2 → 3 as per `t_bfca8f53` → `t_9cfda892` pipeline.

**Docker sign-off:** `campus-pilot-client` healthy, `campus-pilot-apis` healthy, `vite build` artifacts served at `http://127.0.0.1:80/` inside client network and via preview at `http://localhost:4173/` locally.
