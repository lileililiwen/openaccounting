# OpenAccounting convenience Makefile
#
# Builds the Rust binary with persistent build caches so that after the
# first build, subsequent rebuilds take seconds, not minutes.
#
# Requirements:
#   - Docker 23.0+ with BuildKit enabled (default since Docker 23.0)
#   - docker compose plugin (`docker compose`, not `docker-compose`)

COMPOSE   := docker compose
APP       := openaccounting-app
CACHE_TAG := openaccounting:cache

# ---------- High-level targets ----------

.PHONY: help up down restart logs ps shell build rebuild clean prune reset

help: ## Show this help
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

up: build ## Build (with cache) and start all services in the background
	$(COMPOSE) up -d
	@echo "✅  App: http://localhost:3000  ·  Postgres: localhost:5436"

down: ## Stop and remove containers (keeps volumes)
	$(COMPOSE) down

restart: ## Restart the app container
	$(COMPOSE) restart $(APP)

logs: ## Tail logs for the app
	$(COMPOSE) logs -f $(APP)

ps: ## List running containers
	$(COMPOSE) ps

shell: ## Open a shell in the app container
	$(COMPOSE) exec $(APP) sh

# ---------- Build targets ----------
# `cache-from` / `cache-to` use the inline-cache exporter: a layer is
# baked into the tag and re-used on the next build, so the cargo registry
# and target/ are preserved across `make build` invocations.

build: ## Build the app image using the persistent cache image
	docker buildx build \
		--load \
		--cache-from type=image,ref=$(CACHE_TAG) \
		--tag openaccounting:dev \
		--tag $(CACHE_TAG) \
		.
	$(COMPOSE) build

rebuild: ## Force a full rebuild with no cache (slow; only when deps change)
	docker buildx build --no-cache --load --tag openaccounting:dev .
	$(COMPOSE) build --no-cache

# ---------- Maintenance ----------

clean: ## Remove build artifacts and stopped containers
	docker builder prune -f
	$(COMPOSE) down --remove-orphans

prune: ## Aggressive prune: remove all unused images, networks, and build cache
	docker system prune -af

reset: ## DESTRUCTIVE: stop everything, delete volumes, fresh start
	$(COMPOSE) down -v
	$(MAKE) up
