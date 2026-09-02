#!/bin/bash
set -euo pipefail

REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
ANCHOR_LOADER="$REPO_ROOT/enclave/kms-trust-anchors.sh"
MEASURED_ANCHORS="$REPO_ROOT/enclave/kms-trust-anchors.env"
ENTRYPOINT="$REPO_ROOT/enclave/entrypoint.sh"
MAINTENANCE_SCRIPT="$REPO_ROOT/enclave/enter-maintenance.sh"
DOCKERFILE="$REPO_ROOT/enclave/Dockerfile"
TEST_ROOT=$(mktemp -d)
trap 'rm -rf "$TEST_ROOT"' EXIT

# shellcheck disable=SC1090,SC1091
source "$ANCHOR_LOADER"

export MARLIN_ROOT_SERVER_ENDPOINT="attacker.invalid:9999"
export MARLIN_ROOT_SERVER_X25519_PUBKEY="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
export MARLIN_KMS_CONTRACT_ADDRESS="0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
export MARLIN_BACKUP_KEY_PATH="attacker.path"

load_measured_kms_trust_anchors "$MEASURED_ANCHORS"

[ "$MARLIN_ROOT_SERVER_ENDPOINT" = "arbone-v4.kms.box:1100" ]
[ "$MARLIN_ROOT_SERVER_X25519_PUBKEY" = "5ee189d3b990c284ebfe7fc4c2e1cecdb2a6908d0a1aa152592d30066061b92c" ]
[ "$MARLIN_KMS_CONTRACT_ADDRESS" = "0x2e51F48F7440b415D9De30b4D73a18C8E9428982" ]
[ "$MARLIN_BACKUP_KEY_PATH" = "lightfriend.backup.v1" ]
[ "$KMS_ANCHOR_SOURCE" = "measured-eif" ]

sed 's/arbone-v4.kms.box:1100/arbone-v4.kms.box:70000/' "$MEASURED_ANCHORS" > "$TEST_ROOT/invalid-port.env"
if (load_measured_kms_trust_anchors "$TEST_ROOT/invalid-port.env" 2>/dev/null); then
    echo "invalid endpoint port was accepted" >&2
    exit 1
fi

sed '/MARLIN_KMS_CONTRACT_ADDRESS/d' "$MEASURED_ANCHORS" > "$TEST_ROOT/missing-contract.env"
if (load_measured_kms_trust_anchors "$TEST_ROOT/missing-contract.env" 2>/dev/null); then
    echo "missing contract address was accepted" >&2
    exit 1
fi

cp "$MEASURED_ANCHORS" "$TEST_ROOT/unknown-entry.env"
printf '%s\n' 'UNEXPECTED_ANCHOR=value' >> "$TEST_ROOT/unknown-entry.env"
if (load_measured_kms_trust_anchors "$TEST_ROOT/unknown-entry.env" 2>/dev/null); then
    echo "unknown anchor entry was accepted" >&2
    exit 1
fi

grep -Fq 'COPY enclave/kms-trust-anchors.env /etc/lightfriend/kms-trust-anchors.env' "$DOCKERFILE"
grep -Fq 'COPY enclave/kms-trust-anchors.sh /usr/local/lib/lightfriend/kms-trust-anchors.sh' "$DOCKERFILE"
# shellcheck disable=SC2016
grep -Fq 'RUN printf '\''%s\n'\'' "$BUILD_MODE" > /etc/lightfriend/build-mode' "$DOCKERFILE"
grep -Fq 'load_measured_kms_trust_anchors "/etc/lightfriend/kms-trust-anchors.env"' "$ENTRYPOINT"
# shellcheck disable=SC2016
grep -Fq 'KMS_ANCHOR_SOURCE: ${KMS_ANCHOR_SOURCE}' "$ENTRYPOINT"
grep -Fq 'MARLIN_ROOT_SERVER_ENDPOINT|MARLIN_ROOT_SERVER_X25519_PUBKEY|MARLIN_KMS_CONTRACT_ADDRESS|MARLIN_BACKUP_KEY_PATH)' "$ENTRYPOINT"
if grep -Fq 'source /etc/lightfriend/env' "$MAINTENANCE_SCRIPT"; then
    echo "maintenance script executes the host environment as shell code" >&2
    exit 1
fi

echo "measured KMS trust anchors test passed"
