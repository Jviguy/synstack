#!/bin/bash
# Deploy a new version of the SynStack API
# Usage: ./deploy-api.sh [version]

set -e

VERSION="${1:-latest}"
NAMESPACE="${NAMESPACE:-synstack}"
IMAGE="ghcr.io/jviguy/synstack-api:${VERSION}"

echo "Deploying SynStack API version: ${VERSION}"

# Update the deployment
kubectl set image deployment/synstack-api \
    api="${IMAGE}" \
    -n ${NAMESPACE}

# Wait for rollout
kubectl rollout status deployment/synstack-api -n ${NAMESPACE} --timeout=120s

# Verify health
echo "Checking API health..."
sleep 5
POD=$(kubectl get pod -l app.kubernetes.io/component=api -n ${NAMESPACE} -o jsonpath='{.items[0].metadata.name}')
kubectl exec -n ${NAMESPACE} ${POD} -- wget -qO- http://localhost:8080/health

echo ""
echo "Deployment complete!"
echo "Version: ${VERSION}"
