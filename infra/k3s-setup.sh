#!/bin/bash
# SynStack K3s Production Setup Script
# Run this on your VPS to set up the entire stack

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log() { echo -e "${GREEN}[INFO]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# =============================================================================
# Configuration
# =============================================================================
DOMAIN="${DOMAIN:-synstack.org}"
EMAIL="${EMAIL:-admin@synstack.org}"
SYNSTACK_DIR="${SYNSTACK_DIR:-/opt/synstack}"

# k3s kubeconfig location
export KUBECONFIG=/etc/rancher/k3s/k3s.yaml

# =============================================================================
# Pre-flight checks
# =============================================================================
log "Running pre-flight checks..."

if [[ $EUID -ne 0 ]]; then
   error "This script must be run as root"
fi

# Check if k3s is already installed
if command -v k3s &> /dev/null; then
    warn "K3s is already installed"
else
    log "Installing K3s..."
    curl -sfL https://get.k3s.io | sh -s - \
        --disable traefik \
        --write-kubeconfig-mode 644

    # Wait for k3s to be ready
    sleep 10
    until kubectl get nodes 2>/dev/null | grep -q "Ready"; do
        log "Waiting for K3s to be ready..."
        sleep 5
    done
fi

log "K3s is ready!"

# =============================================================================
# Install Helm
# =============================================================================
if ! command -v helm &> /dev/null; then
    log "Installing Helm..."
    curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash
fi

# =============================================================================
# Add Helm repositories
# =============================================================================
log "Adding Helm repositories..."
helm repo add traefik https://traefik.github.io/charts
helm repo add jetstack https://charts.jetstack.io
helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
helm repo add grafana https://grafana.github.io/helm-charts
helm repo add bitnami https://charts.bitnami.com/bitnami
helm repo add gitea https://dl.gitea.com/charts/
helm repo update

# =============================================================================
# Install Traefik Ingress Controller
# =============================================================================
log "Installing Traefik..."
helm upgrade --install traefik traefik/traefik \
    --namespace traefik \
    --create-namespace \
    --set ingressClass.enabled=true \
    --set ingressClass.isDefaultClass=true \
    --wait

# Create HTTPS redirect middleware
log "Creating HTTPS redirect middleware..."
cat <<EOF | kubectl apply -f -
apiVersion: traefik.io/v1alpha1
kind: Middleware
metadata:
  name: redirect-https
  namespace: default
spec:
  redirectScheme:
    scheme: https
    permanent: true
EOF

# =============================================================================
# Install cert-manager for TLS
# =============================================================================
log "Installing cert-manager..."
helm upgrade --install cert-manager jetstack/cert-manager \
    --namespace cert-manager \
    --create-namespace \
    --set crds.enabled=true \
    --wait

# Create ClusterIssuer for Let's Encrypt
log "Creating Let's Encrypt ClusterIssuer..."
cat <<EOF | kubectl apply -f -
apiVersion: cert-manager.io/v1
kind: ClusterIssuer
metadata:
  name: letsencrypt-prod
spec:
  acme:
    email: ${EMAIL}
    server: https://acme-v02.api.letsencrypt.org/directory
    privateKeySecretRef:
      name: letsencrypt-prod-account-key
    solvers:
    - http01:
        ingress:
          class: traefik
---
apiVersion: cert-manager.io/v1
kind: ClusterIssuer
metadata:
  name: letsencrypt-staging
spec:
  acme:
    email: ${EMAIL}
    server: https://acme-staging-v02.api.letsencrypt.org/directory
    privateKeySecretRef:
      name: letsencrypt-staging-account-key
    solvers:
    - http01:
        ingress:
          class: traefik
EOF

# =============================================================================
# Install Monitoring Stack
# =============================================================================
log "Installing Prometheus + Grafana..."
helm upgrade --install monitoring prometheus-community/kube-prometheus-stack \
    --namespace monitoring \
    --create-namespace \
    --values ${SYNSTACK_DIR}/infra/helm/monitoring/values-kube-prometheus-stack.yaml \
    --set grafana.adminPassword="$(openssl rand -base64 16)" \
    --wait

log "Installing Loki for logs..."
helm upgrade --install loki grafana/loki-stack \
    --namespace monitoring \
    --values ${SYNSTACK_DIR}/infra/helm/monitoring/values-loki.yaml \
    --wait

# =============================================================================
# Create SynStack namespace
# =============================================================================
kubectl create namespace synstack --dry-run=client -o yaml | kubectl apply -f -

# =============================================================================
# Install SynStack
# =============================================================================
log "Building Helm dependencies..."
cd ${SYNSTACK_DIR}/infra/helm/synstack
helm dependency build

# Generate secrets if not provided
if [[ -z "${POSTGRES_PASSWORD}" ]]; then
    POSTGRES_PASSWORD=$(openssl rand -base64 24 | tr -d '\n')
    warn "Generated PostgreSQL password. Save this: ${POSTGRES_PASSWORD}"
fi

if [[ -z "${ENCRYPTION_KEY}" ]]; then
    ENCRYPTION_KEY=$(openssl rand -base64 32 | tr -d '\n')
    warn "Generated encryption key. Save this: ${ENCRYPTION_KEY}"
fi

if [[ -z "${GITEA_ADMIN_PASSWORD}" ]]; then
    GITEA_ADMIN_PASSWORD=$(openssl rand -base64 16 | tr -d '\n')
    warn "Generated Gitea admin password. Save this: ${GITEA_ADMIN_PASSWORD}"
fi

log "Installing SynStack..."
helm upgrade --install synstack . \
    --namespace synstack \
    --values values.yaml \
    --values values-production.yaml \
    --set postgresql.auth.password="${POSTGRES_PASSWORD}" \
    --set api.secrets.encryptionKey="${ENCRYPTION_KEY}" \
    --set gitea.gitea.admin.password="${GITEA_ADMIN_PASSWORD}" \
    --set api.ingress.hosts[0].host="api.${DOMAIN}" \
    --set api.ingress.tls[0].hosts[0]="api.${DOMAIN}" \
    --set gitea.ingress.hosts[0].host="git.${DOMAIN}" \
    --set gitea.ingress.tls[0].hosts[0]="git.${DOMAIN}" \
    --set gitea.gitea.config.server.DOMAIN="git.${DOMAIN}" \
    --set gitea.gitea.config.server.ROOT_URL="https://git.${DOMAIN}" \
    --set gitea.gitea.config.server.SSH_DOMAIN="git.${DOMAIN}" \
    --wait --timeout 10m

# =============================================================================
# Generate Gitea Admin Token
# =============================================================================
log "Waiting for Gitea to be ready..."
kubectl wait --for=condition=ready pod -l app.kubernetes.io/name=gitea -n synstack --timeout=300s

log "Generating Gitea admin token..."
GITEA_TOKEN=$(kubectl exec -n synstack deploy/synstack-gitea -c gitea -- \
    gitea admin user generate-access-token \
    --config /data/gitea/conf/app.ini \
    --username synstack-admin \
    --token-name synstack-api \
    --scopes all 2>&1 | grep -oP 'Access token was successfully created... \K[a-f0-9]+')

if [[ -n "${GITEA_TOKEN}" ]]; then
    log "Updating API with Gitea token..."
    kubectl create secret generic synstack-api-gitea-token \
        --namespace synstack \
        --from-literal=gitea-admin-token="${GITEA_TOKEN}" \
        --dry-run=client -o yaml | kubectl apply -f -

    # Patch the deployment to use the new secret
    kubectl patch deployment synstack-api -n synstack --type='json' -p='[
        {"op": "add", "path": "/spec/template/spec/containers/0/env/-", "value": {
            "name": "GITEA_ADMIN_TOKEN",
            "valueFrom": {"secretKeyRef": {"name": "synstack-api-gitea-token", "key": "gitea-admin-token"}}
        }}
    ]'

    kubectl rollout restart deployment/synstack-api -n synstack
else
    warn "Could not auto-generate Gitea token. You'll need to create it manually."
fi

# =============================================================================
# Run database migrations
# =============================================================================
log "Running database migrations..."
POD=$(kubectl get pod -l app.kubernetes.io/name=postgresql -n synstack -o jsonpath='{.items[0].metadata.name}')

for f in ${SYNSTACK_DIR}/api/migrations/0*.sql; do
    if [[ -f "$f" ]] && ! echo "$f" | grep -q clickhouse; then
        log "Applying $(basename $f)..."
        kubectl cp "$f" "synstack/${POD}:/tmp/$(basename $f)"
        kubectl exec -n synstack ${POD} -- sh -c \
            "PGPASSWORD='${POSTGRES_PASSWORD}' psql -U synstack -d synstack -f /tmp/$(basename $f)" || true
    fi
done

# =============================================================================
# Summary
# =============================================================================
echo ""
echo "============================================"
echo -e "${GREEN}SynStack Deployment Complete!${NC}"
echo "============================================"
echo ""
echo "Services:"
echo "  API:        https://api.${DOMAIN}"
echo "  Gitea:      https://git.${DOMAIN}"
echo "  Grafana:    https://grafana.${DOMAIN}"
echo ""
echo "Credentials (SAVE THESE!):"
echo "  PostgreSQL:    ${POSTGRES_PASSWORD}"
echo "  Encryption:    ${ENCRYPTION_KEY}"
echo "  Gitea Admin:   synstack-admin / ${GITEA_ADMIN_PASSWORD}"
echo "  Gitea Token:   ${GITEA_TOKEN:-'Generate manually'}"
echo ""
echo "Grafana password:"
kubectl get secret -n monitoring monitoring-grafana -o jsonpath="{.data.admin-password}" | base64 -d
echo ""
echo ""
echo "Next steps:"
echo "  1. Point your DNS to this server's IP"
echo "  2. Wait for TLS certificates to be issued"
echo "  3. Test the API: curl https://api.${DOMAIN}/health"
echo ""
