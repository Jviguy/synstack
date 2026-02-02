.PHONY: cluster-up cluster-down infra-up infra-down db-migrate db-shell db-reset ch-migrate ch-shell gitea-setup gitea-token api-build api-deploy api-dev logs-api logs-gitea logs-postgres logs-clickhouse entities setup clean prod-traefik prod-deploy prod-status prod-traefik-logs

# ============================================================
# Cluster Management
# ============================================================

cluster-up:
	@if kind get clusters 2>/dev/null | grep -q synstack; then \
		echo "Cluster 'synstack' already exists."; \
	else \
		echo "Creating kind cluster..."; \
		kind create cluster --config infra/kind-config.yaml; \
	fi
	@kubectl config use-context kind-synstack
	@echo "Cluster ready. Run 'make infra-up' to deploy services."

cluster-down:
	@echo "Deleting kind cluster..."
	kind delete cluster --name synstack

cluster-status:
	@kubectl cluster-info --context kind-synstack

# ============================================================
# Infrastructure Deployment
# ============================================================

infra-up: namespace postgres clickhouse gitea
	@echo ""
	@echo "Infrastructure deployed!"
	@echo "  PostgreSQL:  localhost:5432"
	@echo "  ClickHouse:  localhost:8123 (HTTP) / localhost:9000 (native)"
	@echo "  Gitea:       http://localhost:3000 (auto-configured)"
	@echo "               Login: synstack-admin / synstack-admin-password"

infra-down:
	kubectl delete namespace synstack --ignore-not-found

namespace:
	kubectl apply -f infra/namespace.yaml

postgres:
	kubectl apply -f infra/postgres/postgres.yaml
	@echo "Waiting for PostgreSQL pod to exist..."
	@until kubectl get pod -l app=postgres -n synstack 2>/dev/null | grep -q postgres; do sleep 1; done
	@echo "Waiting for PostgreSQL to be ready..."
	@kubectl wait --for=condition=ready pod -l app=postgres -n synstack --timeout=120s

clickhouse:
	kubectl apply -f infra/clickhouse/clickhouse.yaml
	@echo "Waiting for ClickHouse pod to exist..."
	@until kubectl get pod -l app=clickhouse -n synstack 2>/dev/null | grep -q clickhouse; do sleep 1; done
	@echo "Waiting for ClickHouse to be ready..."
	@kubectl wait --for=condition=ready pod -l app=clickhouse -n synstack --timeout=120s

gitea:
	kubectl apply -f infra/gitea/gitea.yaml
	@echo "Waiting for Gitea pod to exist..."
	@until kubectl get pod -l app=gitea -n synstack 2>/dev/null | grep -q gitea; do sleep 1; done
	@echo "Waiting for Gitea to be ready..."
	@kubectl wait --for=condition=ready pod -l app=gitea -n synstack --timeout=180s

# ============================================================
# Gitea Setup
# ============================================================

gitea-token:
	@echo "Creating Gitea API token..."
	@kubectl exec -n synstack deploy/gitea -c gitea -- \
		gitea admin user generate-access-token \
		--config /etc/gitea/app.ini \
		--username synstack-admin \
		--token-name synstack-api \
		--scopes all 2>&1 | tee /tmp/gitea-token.txt
	@echo ""
	@echo "Token created! Update api/.env with the token above."

# Create api/.env from template with auto-generated Gitea token
env-setup:
	@if [ -f api/.env ]; then \
		echo "api/.env already exists. Delete it first if you want to regenerate."; \
	else \
		echo "Creating api/.env from template..."; \
		cp api/.env.example api/.env; \
		echo "Generating Gitea token..."; \
		TOKEN=$$(kubectl exec -n synstack deploy/gitea -c gitea -- \
			gitea admin user generate-access-token \
			--config /etc/gitea/app.ini \
			--username synstack-admin \
			--token-name synstack-api-$$(date +%s) \
			--scopes all 2>&1 | grep "Access token" | awk '{print $$NF}'); \
		if [ -n "$$TOKEN" ]; then \
			sed -i "s/GITEA_ADMIN_TOKEN=.*/GITEA_ADMIN_TOKEN=$$TOKEN/" api/.env; \
			echo "api/.env created with fresh Gitea token!"; \
		else \
			echo "Warning: Could not generate Gitea token. Update api/.env manually."; \
		fi; \
	fi

# ============================================================
# Database
# ============================================================

db-migrate:
	@echo "Running PostgreSQL migrations..."
	@POD=$$(kubectl get pod -l app=postgres -n synstack -o jsonpath='{.items[0].metadata.name}'); \
	for f in api/migrations/0*.sql; do \
		if [ -f "$$f" ] && echo "$$f" | grep -qv clickhouse; then \
			echo "Applying $$(basename $$f)..."; \
			kubectl cp "$$f" "synstack/$$POD:/tmp/$$(basename $$f)"; \
			kubectl exec -n synstack deploy/postgres -- sh -c "PGPASSWORD=REDACTED_PASSWORD psql -U synstack -d synstack -f /tmp/$$(basename $$f)" || true; \
		fi; \
	done
	@echo "PostgreSQL migrations complete."

db-seed:
	@echo "Seeding test data..."
	@kubectl exec -n synstack deploy/postgres -- sh -c "PGPASSWORD=REDACTED_PASSWORD psql -U synstack -d synstack -c \"INSERT INTO issues (id, title, body, source_type, status, language, difficulty, created_at) VALUES (gen_random_uuid(), 'Fix the greeting bug', 'The hello() function returns wrong greeting. It should return Hello World but returns Hello Wrold (typo).', 'manual', 'open', 'python', 'easy', NOW()) ON CONFLICT DO NOTHING\""
	@kubectl exec -n synstack deploy/postgres -- sh -c "PGPASSWORD=REDACTED_PASSWORD psql -U synstack -d synstack -c \"INSERT INTO issues (id, title, body, source_type, status, language, difficulty, created_at) VALUES (gen_random_uuid(), 'Add input validation', 'The parse_number function crashes on non-numeric input. Add proper validation and return None for invalid input.', 'manual', 'open', 'python', 'medium', NOW()) ON CONFLICT DO NOTHING\""
	@kubectl exec -n synstack deploy/postgres -- sh -c "PGPASSWORD=REDACTED_PASSWORD psql -U synstack -d synstack -c \"INSERT INTO issues (id, title, body, source_type, status, language, difficulty, created_at) VALUES (gen_random_uuid(), 'Implement binary search', 'Add a binary_search function that finds an element in a sorted list. Return the index or -1 if not found.', 'manual', 'open', 'rust', 'medium', NOW()) ON CONFLICT DO NOTHING\""
	@echo "Test data seeded."

db-shell:
	kubectl exec -it -n synstack deploy/postgres -- sh -c 'PGPASSWORD=REDACTED_PASSWORD psql -U synstack -d synstack'

db-reset:
	@echo "Dropping and recreating database..."
	@kubectl exec -n synstack deploy/postgres -- sh -c "PGPASSWORD=REDACTED_PASSWORD psql -U synstack -d postgres -c 'DROP DATABASE IF EXISTS synstack'"
	@kubectl exec -n synstack deploy/postgres -- sh -c "PGPASSWORD=REDACTED_PASSWORD psql -U synstack -d postgres -c 'CREATE DATABASE synstack'"
	@make db-migrate

# ============================================================
# ClickHouse
# ============================================================

ch-migrate:
	@echo "Running ClickHouse migrations..."
	@kubectl cp api/migrations/001_clickhouse_initial.sql synstack/$(shell kubectl get pod -l app=clickhouse -n synstack -o jsonpath='{.items[0].metadata.name}'):/tmp/001_clickhouse_initial.sql
	@kubectl exec -n synstack deploy/clickhouse -- clickhouse-client --password synstack --multiquery --queries-file /tmp/001_clickhouse_initial.sql
	@echo "ClickHouse migrations complete."

ch-shell:
	kubectl exec -it -n synstack deploy/clickhouse -- clickhouse-client --password synstack

# ============================================================
# SeaORM Entity Generation
# ============================================================

entities:
	@echo "Generating SeaORM entities from database..."
	sea-orm-cli generate entity \
		--database-url postgres://synstack:REDACTED_PASSWORD@localhost:5432/synstack \
		--output-dir api/src/entity \
		--with-serde both
	@echo "Entities generated in api/src/entity/"

# ============================================================
# API Development
# ============================================================

api-build:
	@echo "Building API..."
	cd api && cargo build --release
	@echo "Building Docker image..."
	docker build -t synstack-api:latest -f api/Dockerfile api/
	@echo "Loading image into kind..."
	kind load docker-image synstack-api:latest --name synstack

api-deploy: api-build
	kubectl apply -f infra/api/api.yaml
	kubectl rollout restart deployment/api -n synstack
	@echo "Waiting for API to be ready..."
	@kubectl wait --for=condition=ready pod -l app=api -n synstack --timeout=120s

# Run API locally - uses api/.env if present, otherwise defaults
api-dev:
	@echo "Running API locally on port 8081 (connects to k8s services)..."
	@if [ -f api/.env ]; then \
		echo "Loading configuration from api/.env"; \
		cd api && set -a && . ./.env && set +a && cargo run; \
	else \
		echo "Warning: api/.env not found. Copy api/.env.example to api/.env and configure."; \
		echo "Using defaults (may not work without valid GITEA_ADMIN_TOKEN)..."; \
		cd api && DATABASE_URL=postgres://synstack:REDACTED_PASSWORD@localhost:5432/synstack \
			CLICKHOUSE_URL=http://localhost:8123 \
			GITEA_URL=http://localhost:3000 \
			GITEA_ADMIN_TOKEN=your-gitea-token-here \
			ENCRYPTION_KEY=dev-encryption-key-32-bytes-long \
			RUST_LOG=debug \
			PORT=8081 \
			cargo run; \
	fi

api-test:
	@echo "Running unit tests..."
	cd api && cargo test

api-test-integration:
	@echo "Running integration tests (requires database)..."
	cd api && TEST_DATABASE_URL=postgres://synstack:REDACTED_PASSWORD@localhost:5432/synstack \
		cargo test integration_tests -- --ignored --test-threads=1

# ============================================================
# Logs
# ============================================================

logs-api:
	kubectl logs -f deploy/api -n synstack

logs-gitea:
	kubectl logs -f deploy/gitea -n synstack

logs-postgres:
	kubectl logs -f deploy/postgres -n synstack

logs-clickhouse:
	kubectl logs -f deploy/clickhouse -n synstack

# ============================================================
# Shell Access
# ============================================================

shell-api:
	kubectl exec -it deploy/api -n synstack -- /bin/sh

shell-gitea:
	kubectl exec -it deploy/gitea -n synstack -- /bin/sh

shell-postgres:
	kubectl exec -it deploy/postgres -n synstack -- /bin/sh

shell-clickhouse:
	kubectl exec -it deploy/clickhouse -n synstack -- clickhouse-client --password synstack

# ============================================================
# Full Setup (first time)
# ============================================================

setup: cluster-up infra-up db-migrate ch-migrate env-setup
	@echo ""
	@echo "============================================"
	@echo "SynStack local environment ready!"
	@echo "============================================"
	@echo ""
	@echo "Services:"
	@echo "  PostgreSQL:  localhost:5432"
	@echo "  ClickHouse:  localhost:8123"
	@echo "  Gitea:       http://localhost:3000"
	@echo "               Login: synstack-admin / synstack-admin-password"
	@echo "  API:         http://localhost:8081 (after 'make api-dev')"
	@echo ""
	@echo "Quick start:"
	@echo "  make api-dev    # Start the API server"
	@echo ""
	@echo "Dev tools:"
	@echo "  cargo install sea-orm-cli   # for 'make entities'"
	@echo ""

# Quick start for returning developers (assumes infra is already running)
quick-start: env-setup
	@echo "Environment ready! Run 'make api-dev' to start the API."

# Reset test data (clean slate for testing)
test-reset:
	@echo "Resetting test data..."
	@make db-reset
	@echo "Test environment reset complete."

# ============================================================
# Production Deployment (k3s)
# ============================================================

# Deploy Traefik configuration (run once, or when config changes)
prod-traefik:
	kubectl apply -f infra/ingress/traefik-config.yaml
	@echo "Traefik config applied. Waiting for restart..."
	@sleep 5
	kubectl rollout status deployment/traefik -n kube-system --timeout=120s

# Deploy all infrastructure to production k3s
prod-deploy: prod-traefik
	kubectl apply -f infra/namespace.yaml
	kubectl apply -f infra/postgres/postgres.yaml
	@echo "Waiting for PostgreSQL..."
	@until kubectl get pod -l app=postgres -n synstack 2>/dev/null | grep -q postgres; do sleep 1; done
	@kubectl wait --for=condition=ready pod -l app=postgres -n synstack --timeout=120s
	kubectl apply -f infra/gitea/gitea.yaml
	@echo "Waiting for Gitea..."
	@until kubectl get pod -l app=gitea -n synstack 2>/dev/null | grep -q gitea; do sleep 1; done
	@kubectl wait --for=condition=ready pod -l app=gitea -n synstack --timeout=180s
	kubectl apply -f infra/ingress/ingress.yaml
	kubectl apply -f infra/ingress/gitea-ssh.yaml
	@echo ""
	@echo "Production deployed!"
	@echo "  API:   https://api.synstack.org"
	@echo "  Gitea: https://git.synstack.org"
	@echo ""
	@echo "Run migrations: make db-migrate"

# Check production status
prod-status:
	@echo "=== Pods ==="
	kubectl get pods -n synstack
	@echo ""
	@echo "=== Ingress ==="
	kubectl get ingress -n synstack
	@echo ""
	@echo "=== Traefik ==="
	kubectl get pods -n kube-system -l app.kubernetes.io/name=traefik

# View Traefik logs (for TLS debugging)
prod-traefik-logs:
	kubectl logs -n kube-system -l app.kubernetes.io/name=traefik --tail=100 -f

# ============================================================
# Cleanup
# ============================================================

clean: cluster-down
	@echo "Cleaned up!"
