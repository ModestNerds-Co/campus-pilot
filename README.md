# Campus Pilot — Mono Repo

Single repo for **Campus Pilot** school management system.

```
campus-pilot/
├── apis/    → Rust / Actix-web + SQLx + Postgres (from ModestNerds-Co/campus-pilot-apis)
├── client/  → React + Vite + TanStack Router (from ModestNerds-Co/campus-pilot-client)
├── docker-compose.yml  → boots postgres + minio + apis + client
├── .env.example
└── README.md
```

History from both repos is preserved via `git subtree` (no nested `.git` in subfolders).

## Quick start

```bash
cp .env.example .env
# edit JWT_SECRET if you want

docker compose up --build

# Services:
# - Postgres  → localhost:5432  (user: campus_pilot / campus_pilot)
# - MinIO     → localhost:9000  (console :9001, minioadmin/minioadmin)
# - APIs      → http://localhost:8000
# - Client    → http://localhost:3000  (nginx serving Vite build)
```

### Dev without Docker
```bash
# apis
cd apis
cp .env.example .env
cargo run  # needs local postgres + minio running

# client
cd client
pnpm install
pnpm dev  # http://localhost:5173
```

## Structure
- `apis/` — keep Cargo.toml as source of truth, sqlx migrations in `apis/migrations/`
- `client/` — pnpm workspace, build output in `client/dist/`

## Deploy

This host serves `campus.antonlabs.cc` through an external Traefik + cloudflared tunnel that
is **not** part of this repo's own `docker-compose.yml`. Routing labels, the
`media-server_default` network attachment, and the production `VITE_API_BASE_URL` build arg
all live in `docker-compose.prod.yml`. **Always include both files when building or
(re)starting `apis`/`client` on this host:**

```bash
docker compose -f docker-compose.yml -f docker-compose.prod.yml build apis client
docker compose -f docker-compose.yml -f docker-compose.prod.yml up -d apis client
```

Running plain `docker compose build/up` (base file only) on these two services silently
strips the Traefik labels and network on recreate, and bakes `localhost:8000` into the
client's JS bundle instead of the public URL — the container stays "healthy" locally while
the public site 502s. `gh` will be configured for `ModestNerds-Co/campus-pilot` after first push.

