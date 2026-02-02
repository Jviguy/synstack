# SynStack Ingress Setup (k3s + Traefik)

## Prerequisites

1. k3s installed on your VPS:
   ```bash
   curl -sfL https://get.k3s.io | sh -
   ```

2. DNS records pointing to your VPS IP:
   - `api.synstack.org` → A record → `<your-vps-ip>`
   - `git.synstack.org` → A record → `<your-vps-ip>`

3. Ports open on your VPS firewall:
   - 80 (HTTP - for ACME challenges)
   - 443 (HTTPS)
   - 22 (SSH for git)

## Deployment Order

```bash
# 1. Apply Traefik configuration (restarts Traefik)
kubectl apply -f infra/ingress/traefik-config.yaml

# 2. Wait for Traefik to restart
kubectl rollout status deployment/traefik -n kube-system

# 3. Deploy the synstack namespace and services
kubectl apply -f infra/namespace.yaml
kubectl apply -f infra/postgres/
kubectl apply -f infra/gitea/

# 4. Apply ingress routes
kubectl apply -f infra/ingress/ingress.yaml
kubectl apply -f infra/ingress/gitea-ssh.yaml
```

## Verify

```bash
# Check Traefik is running
kubectl get pods -n kube-system | grep traefik

# Check ingress routes
kubectl get ingress -n synstack

# Check TLS certificates (may take a few minutes)
kubectl logs -n kube-system deployment/traefik | grep -i acme

# Test HTTP redirect
curl -I http://api.synstack.org

# Test HTTPS
curl https://api.synstack.org/health
curl https://git.synstack.org/api/healthz
```

## Troubleshooting

### Certificates not issuing
- Ensure DNS is pointing to your VPS
- Ensure port 80 is open (Let's Encrypt HTTP challenge)
- Check Traefik logs: `kubectl logs -n kube-system deployment/traefik`

### 404 errors
- Check services are running: `kubectl get svc -n synstack`
- Check pods are healthy: `kubectl get pods -n synstack`

### SSH not working
- Ensure port 22 is open on firewall
- Check TCP route: `kubectl get ingressroutetcp -n synstack`
