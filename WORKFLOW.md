# WORKFLOW — Iterative Test-Deploy-Verify Loop

This repo uses a **small-batch Docker-first workflow**: every increment is verified
locally, built in Docker, and smoke-tested before the next increment starts.

## Architecture

```
docker-compose.yml → postgres + minio + apis (Rust/Actix :8000) + client (nginx :80)
client/Dockerfile  → node:20 (pnpm build) → nginx:alpine (SPA + gzip, listen 80 + [::]:80)
apis/Dockerfile    → rust:1.91 (SQLX_OFFLINE=true, cached build) → debian:bookworm-slim
```

- **Env**: copy `.env.example` → `.env`, set `JWT_SECRET` (≥32 chars). `VITE_API_BASE_URL` is baked as a build arg.
- **Storage**: MinIO S3 compatible, bucket auto-created by `minio-setup` (public read).
- **Health**: all services have `healthcheck`; `apis` waits for `postgres+minio` healthy, `client` waits for `apis` healthy.

## Quick Start (verified 2026-08-20)

```bash
cp .env.example .env            # edit JWT_SECRET if desired
docker compose up -d --build    # builds apis + client, starts postgres/minio/apis/client
docker compose ps -a            # all should be healthy
curl http://localhost:8000/api/1.0/health-check
docker exec campus-pilot-client wget -qO- http://127.0.0.1:80/ | head
```

Stop: `docker compose down` (add `-v` to wipe DB/storage).

Local dev without Docker:

```bash
# apis needs postgres+minio running (docker compose up postgres minio -d)
cd apis && cargo run
cd client && pnpm install && pnpm dev   # http://localhost:3000 (Vite), prod nginx is :80 in Docker
```

## Iterative Workflow (per increment)

```
1. Implement  — small change, one feature/fix, keep diff < ~300 lines
       ↓
2. Local gate — npm / cargo checks (fast, no Docker)
       make typecheck            # tsc --noEmit + cargo check
       make test-client          # tsc && vite build  (type + build gate)
       make test-apis            # cargo test --lib   (unit tests, no DB)
       # or: make verify / make check
       ↓
3. Docker build — rebuild images with the change
       docker compose build      # or: make build / make rebuild (no-cache)
       ↓
4. Deploy/run  — start stack and wait for healthy
       docker compose up -d
       docker compose ps -a      # expect healthy on all
       ↓
5. Verify      — smoke + manual checks
       curl -sf http://localhost:8000/api/1.0/health-check | jq
       docker exec campus-pilot-client wget -qO- http://127.0.0.1:80/ | head
       docker compose logs -f    # watch for errors
       open http://localhost:8000/api/1.0/health-check  (browser)
       # apis at :8000, client assets via nginx at :80 (internal, expose via Traefik in prod)
       # responsive check: resize browser / devtools device toolbar
       # console check: no errors in browser console
       ↓
6. Sign-off    — only if Definition of Done passes, then next batch
```

### Make shortcuts

| Command              | What it does |
|----------------------|--------------|
| `make help`          | list commands |
| `make build`         | `docker compose build` |
| `make up`            | `up -d --build` |
| `make ps` / `make health` | status + curl/wget health probes |
| `make typecheck`     | client `tsc --noEmit` + `cargo check` |
| `make test-client`   | `tsc && vite build` |
| `make test-apis`     | `cargo test --lib` |
| `make verify`        | typecheck + test-client + test-apis |
| `make check`         | pre-docker gate (typecheck + test-client) |
| `make docker-verify` | build + up + health |
| `make rebuild`       | `build --no-cache` + `up -d` |
| `make clean`         | `down -v --remove-orphans` (wipes DB) |

### npm scripts (client)

- `pnpm typecheck` — `tsc --noEmit`
- `pnpm lint` — alias to typecheck (no eslint configured yet)
- `pnpm build` — `tsc && vite build`
- `pnpm verify` — `typecheck && build`
- `pnpm test` — `tsc --noEmit && vite build` (build is the test gate until unit tests are added)

## Definition of Done (per increment)

Every increment must pass before the next starts:

- [ ] **Lint passes** — `make typecheck` (client `tsc --noEmit`, apis `cargo check`) with 0 errors
- [ ] **Build succeeds** — `make test-client` (`tsc && vite build`) produces `dist/`; `docker compose build` succeeds for both images
- [ ] **Runs in Docker** — `docker compose up -d` → all services `healthy` (`docker compose ps`), `curl /api/1.0/health-check` returns `{"status":"healthy"}`
- [ ] **No console errors** — browser console on client + `docker compose logs apis client` show no errors/warnings
- [ ] **Responsive check** — client renders at 375px, 768px, 1440px (devtools device toolbar), no horizontal overflow, no broken layout
- [ ] **Smoke verified** — at least one happy-path flow of the changed feature exercised manually

Optional (when applicable):

- [ ] `cargo test` relevant tests pass (note: integration tests need `JWT_SECRET` env; `cargo test --lib` covers unit tests without DB)
- [ ] `sqlx prepare` run if `query!` macros changed, `.sqlx/` committed

## Troubleshooting

- **client unhealthy (`wget: can't connect`)** — fixed: healthcheck now uses `127.0.0.1:80` (localhost resolves to `::1` first, nginx listens on `80` + `[::]:80`). If you see this again, `docker exec campus-pilot-client wget -qO- http://127.0.0.1:80/` should return HTML.
- **apis not healthy** — check `docker compose logs apis`, verify `postgres` and `minio` are healthy first, confirm `.env` has `JWT_SECRET` set.
- **Build cache stale** — `make rebuild` (builds with `--no-cache`).
- **Port conflicts** — `apis` publishes `${APP_PORT:-8000}:8000`, `minio` publishes `9000/9001`; `client` is `expose: 80` (internal, for Traefik in prod). For local browser access to client, add a `ports: ["3000:80"]` override or run `pnpm dev`.
- **Vite API base** — `VITE_API_BASE_URL` is a build arg; change it in `.env` then `docker compose build client` to rebake.

## Test command (canonical)

```bash
# fast local gate (no Docker, no DB)
make verify

# full Docker gate
docker compose build && docker compose up -d && make health
```
