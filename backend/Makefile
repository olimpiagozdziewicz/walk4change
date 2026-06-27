# walk4change backend — local dev + demo orchestration.
# Supabase stays prod-only; everything here is local Docker.
.DEFAULT_GOAL := help
SHELL := /bin/bash

.PHONY: help demo up down seed replay logs

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN{FS=":.*?## "}{printf "  \033[1m%-9s\033[0m %s\n", $$1, $$2}'

demo: ## One-command live demo: stack + walk + browser link + two walkers
	@./scripts/demo.sh

up: ## Bring up the local stack (Postgres + API)
	@./scripts/dev-up.sh

seed: ## Seed demo data (Ana & Bek, password demodemo)
	@./scripts/seed.sh

replay: ## Stream two walkers; pass SESSION=<id> to target an existing walk
	@./scripts/replay.sh $(SESSION)

down: ## Stop the stack (pass ARGS=--purge to wipe the DB volume)
	@./scripts/dev-down.sh $(ARGS)

logs: ## Follow the API logs
	@docker logs -f walk4change-api
