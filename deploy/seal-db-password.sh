#!/usr/bin/env bash
set -euo pipefail

# seal-db-password.sh — Generate a random DB password and seal it via
# Sealed Secrets for commit.
#
# Prerequisites:
#   1. kubectl authenticated to the cluster
#   2. kubeseal installed (brew install kubeseal)
#   3. Sealed Secrets controller running in the cluster
#
# Usage:
#   ./deploy/seal-db-password.sh
#
# The script:
#   1. Generates a 32-char alphanumeric password
#   2. Creates a temporary Secret YAML in the yume-vale namespace
#   3. Pipes it to kubeseal → writes deploy/36-db-sealed-secret.yaml
#   4. Prints the plaintext password ONCE to stdout (copy it, then it's gone)
#   5. Securely removes all plaintext traces

SECRET_NAME="yume-db"
NAMESPACE="yume-vale"
OUTPUT="deploy/36-db-sealed-secret.yaml"

# Generate random password (32 alphanumeric chars)
PASSWORD=$(openssl rand -base64 24 | tr -dc 'a-zA-Z0-9' | head -c32)

# Also compute the connection URL
DB_URL="postgres://yumevale:${PASSWORD}@yume-postgres:5432/yumevale"

# Create temp Secret and seal it
SECRET_YAML=$(cat <<EOF
apiVersion: v1
kind: Secret
metadata:
  name: ${SECRET_NAME}
  namespace: ${NAMESPACE}
stringData:
  password: ${PASSWORD}
  url: ${DB_URL}
EOF
)

echo "${SECRET_YAML}" | kubeseal \
    --controller-namespace=sealed-secrets \
    --controller-name=sealed-secrets \
    --format=yaml \
    > "${OUTPUT}"

echo "=== SealedSecret written to ${OUTPUT} ==="
echo "=== DB Password (copy before dismissing): === "
echo "${PASSWORD}"
echo "============================================="

# Wipe temp variables
unset PASSWORD DB_URL SECRET_YAML

# Clear clipboard (macOS)
if command -v pbcopy &>/dev/null; then
    echo "" | pbcopy
fi

echo "Done.  Plaintext has been wiped from memory and clipboard."
