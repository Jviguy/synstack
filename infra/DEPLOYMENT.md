# SynStack Production Deployment

This guide covers deploying SynStack to a VPS with K3s.

## Prerequisites

- A VPS with:
  - Ubuntu 22.04+ or Debian 12+
  - 4GB+ RAM (8GB recommended)
  - 2+ vCPUs
  - 40GB+ disk
- A domain name pointing to your VPS IP
- SSH access as root

## Quick Start

```bash
# 1. Clone the repo on your VPS
git clone https://github.com/jviguy/synstack.git /opt/synstack
cd /opt/synstack

# 2. Set your domain
export DOMAIN="yourdomain.com"
export EMAIL="you@email.com"

# 3. Run the setup script
./infra/k3s-setup.sh
```

The script will:
1. Install K3s
2. Install Traefik ingress controller
3. Install cert-manager for TLS
4. Install Prometheus + Grafana monitoring
5. Install Loki for log aggregation
6. Deploy PostgreSQL, Gitea, and the API
7. Run database migrations
8. Generate and configure Gitea admin token

## Manual Installation

If you prefer to install step by step:

### 1. Install K3s

```bash
curl -sfL https://get.k3s.io | sh -s - --disable traefik
```

### 2. Install Helm

```bash
curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash
```

### 3. Add Helm repos

```bash
helm repo add traefik https://traefik.github.io/charts
helm repo add jetstack https://charts.jetstack.io
helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
helm repo add grafana https://grafana.github.io/helm-charts
helm repo add bitnami https://charts.bitnami.com/bitnami
helm repo add gitea https://dl.gitea.com/charts/
helm repo update
```

### 4. Install Traefik

```bash
helm install traefik traefik/traefik \
    --namespace traefik --create-namespace \
    --set ports.web.redirectTo.port=websecure \
    --set ports.websecure.tls.enabled=true \
    --set ingressClass.enabled=true \
    --set ingressClass.isDefaultClass=true
```

### 5. Install cert-manager

```bash
helm install cert-manager jetstack/cert-manager \
    --namespace cert-manager --create-namespace \
    --set crds.enabled=true

# Create ClusterIssuer
cat <<EOF | kubectl apply -f -
apiVersion: cert-manager.io/v1
kind: ClusterIssuer
metadata:
  name: letsencrypt-prod
spec:
  acme:
    email: your@email.com
    server: https://acme-v02.api.letsencrypt.org/directory
    privateKeySecretRef:
      name: letsencrypt-prod-account-key
    solvers:
    - http01:
        ingress:
          class: traefik
EOF
```

### 6. Install Monitoring

```bash
# Prometheus + Grafana
helm install monitoring prometheus-community/kube-prometheus-stack \
    --namespace monitoring --create-namespace \
    -f infra/helm/monitoring/values-kube-prometheus-stack.yaml

# Loki for logs
helm install loki grafana/loki-stack \
    --namespace monitoring \
    -f infra/helm/monitoring/values-loki.yaml
```

### 7. Install SynStack

```bash
cd infra/helm/synstack
helm dependency build

helm install synstack . \
    --namespace synstack --create-namespace \
    -f values.yaml \
    -f values-production.yaml \
    --set postgresql.auth.password="$(openssl rand -base64 24)" \
    --set api.secrets.encryptionKey="$(openssl rand -base64 32)" \
    --set gitea.gitea.admin.password="$(openssl rand -base64 16)"
```

## DNS Configuration

Point these records to your VPS IP:

| Record | Type | Value |
|--------|------|-------|
| api.yourdomain.com | A | YOUR_VPS_IP |
| git.yourdomain.com | A | YOUR_VPS_IP |
| grafana.yourdomain.com | A | YOUR_VPS_IP |

## Accessing Services

After DNS propagation and TLS certificate issuance:

- **API**: https://api.yourdomain.com
- **Gitea**: https://git.yourdomain.com
- **Grafana**: https://grafana.yourdomain.com

Get Grafana password:
```bash
kubectl get secret -n monitoring monitoring-grafana -o jsonpath="{.data.admin-password}" | base64 -d
```

## Useful Commands

```bash
# View all pods
kubectl get pods -A

# View API logs
kubectl logs -f deploy/synstack-api -n synstack

# View Gitea logs
kubectl logs -f deploy/synstack-gitea -n synstack

# Access PostgreSQL
kubectl exec -it deploy/synstack-postgresql -n synstack -- psql -U synstack

# Restart API after config change
kubectl rollout restart deploy/synstack-api -n synstack

# Check certificate status
kubectl get certificates -A
```

## Scaling

For more traffic, increase API replicas:

```bash
kubectl scale deploy/synstack-api -n synstack --replicas=3
```

Or update values-production.yaml:
```yaml
api:
  replicaCount: 3
```

Then:
```bash
helm upgrade synstack . -n synstack -f values.yaml -f values-production.yaml
```

## Backups

### PostgreSQL Backup

```bash
# Create backup
kubectl exec deploy/synstack-postgresql -n synstack -- \
    pg_dump -U synstack synstack > backup-$(date +%Y%m%d).sql

# Restore
cat backup.sql | kubectl exec -i deploy/synstack-postgresql -n synstack -- \
    psql -U synstack synstack
```

### Gitea Backup

```bash
kubectl exec deploy/synstack-gitea -n synstack -- \
    gitea dump -c /data/gitea/conf/app.ini
```

## Troubleshooting

### TLS certificates not issuing

```bash
# Check cert-manager logs
kubectl logs -n cert-manager deploy/cert-manager

# Check certificate status
kubectl describe certificate -n synstack
```

### API not starting

```bash
# Check if waiting for dependencies
kubectl describe pod -l app.kubernetes.io/component=api -n synstack

# Check API logs
kubectl logs -l app.kubernetes.io/component=api -n synstack
```

### Database connection issues

```bash
# Test connectivity
kubectl run test --rm -it --image=busybox -- nc -z synstack-postgresql 5432
```

## Security Hardening

Before going live:

1. **Change all default passwords** in values-production.yaml
2. **Enable firewall**: Only allow 80, 443, 22
3. **Set up fail2ban** for SSH protection
4. **Enable automatic updates**: `apt install unattended-upgrades`
5. **Set up backup cron job**

## Resource Usage

Expected resource usage on a $35/mo VPS (8GB RAM, 4 vCPU):

| Component | Memory | CPU |
|-----------|--------|-----|
| K3s | 500MB | 10% |
| Traefik | 50MB | 1% |
| cert-manager | 30MB | 1% |
| Prometheus | 500MB | 5% |
| Grafana | 150MB | 2% |
| Loki | 300MB | 3% |
| PostgreSQL | 500MB | 5% |
| Gitea | 300MB | 3% |
| API (x2) | 500MB | 5% |
| **Total** | **~3GB** | **~35%** |

You'll have plenty of headroom for traffic spikes.
