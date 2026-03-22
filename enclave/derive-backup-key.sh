#!/bin/bash
set -euo pipefail

# Derive the backup encryption key without persisting it to disk.
#
# Production path:
#   Configure MARLIN_ROOT_SERVER_ENDPOINT, MARLIN_ROOT_SERVER_X25519_PUBKEY,
#   MARLIN_KMS_CONTRACT_ADDRESS and MARLIN_BACKUP_KEY_PATH. The enclave starts
#   a local Marlin derive sidecar and derives the key from localhost.
#
# Alternate production override:
#   MARLIN_BACKUP_KEY_COMMAND=/usr/local/bin/marlin-backup-key
#   The command must print the base64 key to stdout.
#
# Local/dev fallback:
#   ALLOW_INSECURE_BACKUP_KEY_FALLBACK=true
#   BACKUP_ENCRYPTION_KEY provided via container env

if [ -n "${MARLIN_KMS_CONTRACT_ADDRESS:-}" ]; then
    : "${MARLIN_ROOT_SERVER_ENDPOINT:?MARLIN_ROOT_SERVER_ENDPOINT must be set when MARLIN_KMS_CONTRACT_ADDRESS is configured}"
    : "${MARLIN_ROOT_SERVER_X25519_PUBKEY:?MARLIN_ROOT_SERVER_X25519_PUBKEY must be set when MARLIN_KMS_CONTRACT_ADDRESS is configured}"
    : "${MARLIN_BACKUP_KEY_PATH:?MARLIN_BACKUP_KEY_PATH must be set when MARLIN_KMS_CONTRACT_ADDRESS is configured}"

    /usr/local/bin/start-marlin-kms.sh

    curl -fsS "http://127.0.0.1:1101/derive/x25519?path=${MARLIN_BACKUP_KEY_PATH}" \
        | base64 | tr -d '\n'
    exit 0
fi

if [ -n "${MARLIN_BACKUP_KEY_COMMAND:-}" ]; then
    KEY="$(${MARLIN_BACKUP_KEY_COMMAND})"
    if [ -z "${KEY}" ]; then
        echo "MARLIN_BACKUP_KEY_COMMAND returned empty output" >&2
        exit 1
    fi
    printf '%s' "${KEY}"
    exit 0
fi

if [ "${ALLOW_INSECURE_BACKUP_KEY_FALLBACK:-false}" = "true" ] && [ -n "${INSECURE_BACKUP_ENCRYPTION_KEY_FALLBACK:-}" ]; then
    printf '%s' "${INSECURE_BACKUP_ENCRYPTION_KEY_FALLBACK}"
    exit 0
fi

echo "No backup key source configured. Set Marlin KMS env vars, MARLIN_BACKUP_KEY_COMMAND, or enable ALLOW_INSECURE_BACKUP_KEY_FALLBACK for local development only." >&2
exit 1
