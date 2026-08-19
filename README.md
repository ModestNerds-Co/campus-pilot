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
`gh` will be configured for `ModestNerds-Co/campus-pilot` after first push.

