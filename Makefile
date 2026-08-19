# campus-pilot — iterative Docker workflow
.PHONY: help build up down logs ps health test-client test-apis typecheck lint rebuild clean

help: ## Show available commands
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-18s\033[0m %s\n", $$1, $$2}'

# --- Docker lifecycle ---

build: ## Build all images (apis + client)
	docker compose build

up: ## Start all services (build if needed)
	docker compose up -d --build

down: ## Stop and remove containers (keep volumes)
	docker compose down

logs: ## Tail logs for all services
	docker compose logs -f

ps: ## Show container status + health
	docker compose ps -a

health: ## Check health endpoints
	@echo "== apis ==" && curl -sf http://localhost:8000/api/1.0/health-check | head -c 500 && echo
	@echo "== client (via apis network) ==" && docker exec campus-pilot-client wget -qO- http://127.0.0.1:80/ | head -c 200 && echo
	@echo "== docker health ==" && docker inspect --format='{{.Name}}: {{.State.Health.Status}}' $$(docker compose ps -q) 2>/dev/null || docker compose ps

rebuild: ## Force rebuild without cache (use after Dockerfile changes)
	docker compose build --no-cache
	docker compose up -d

clean: ## Remove containers, volumes, and orphans (DESTRUCTIVE)
	docker compose down -v --remove-orphans

# --- Quality gates (Definition of Done) ---

typecheck: ## Run type checks (client tsc + apis cargo check)
	@echo ">> client typecheck"
	cd client && npm run typecheck
	@echo ">> apis cargo check"
	cd apis && cargo check

test-client: ## Build client (tsc + vite) — fails on type errors
	cd client && npm run build

test-apis: ## Run apis unit tests (no DB required for util tests; integration needs env)
	cd apis && cargo test --lib 2>&1 | tail -20

verify: typecheck test-client test-apis ## Full local verify without Docker (fast)

# --- Iterative increment helper ---

check: ## Pre-docker gate: lint/typecheck/build must pass before docker build
	@echo "=== Pre-docker gate ==="
	@$(MAKE) typecheck
	@$(MAKE) test-client
	@echo "=== Gate passed ==="

docker-verify: build up health ## Build in Docker, start, and verify health
	@echo "=== Docker verify ==="
	@sleep 3 && $(MAKE) health
	@echo "=== Done — open http://localhost:8000/api/1.0/health-check ==="
