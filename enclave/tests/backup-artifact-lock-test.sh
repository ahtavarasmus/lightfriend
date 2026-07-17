#!/bin/bash
set -euo pipefail

if ! command -v flock >/dev/null 2>&1; then
    if [ "${CI:-}" = "true" ]; then
        echo "flock is required for the backup artifact lock regression test" >&2
        exit 1
    fi
    echo "skipping backup artifact lock regression test: flock is unavailable"
    exit 0
fi

REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
STORAGE_HEALTH="$REPO_ROOT/enclave/storage-health.sh"
EXPORT_SCRIPT="$REPO_ROOT/enclave/export.sh"
TEST_ROOT=$(mktemp -d)
LOCK_HOLDER_PID=""

cleanup() {
    if [ -n "$LOCK_HOLDER_PID" ]; then
        kill "$LOCK_HOLDER_PID" 2>/dev/null || true
        wait "$LOCK_HOLDER_PID" 2>/dev/null || true
    fi
    rm -rf "$TEST_ROOT"
}
trap cleanup EXIT

LOCK_FILE="$TEST_ROOT/backup-artifacts.lock"
STAGING_ROOT="$TEST_ROOT/backup-staging"
TMP_ROOT="$TEST_ROOT/tmp"
RESTORE_ROOT="$TEST_ROOT/backup-restore"
SEED_ROOT="$TEST_ROOT/seed"
TUWUNEL_BACKUP_ROOT="$TEST_ROOT/tuwunel-backup"
LOCK_READY="$TEST_ROOT/lock-ready"

mkdir -p "$STAGING_ROOT/active" "$TMP_ROOT" "$RESTORE_ROOT" "$SEED_ROOT" "$TUWUNEL_BACKUP_ROOT"
touch "$STAGING_ROOT/active/data" "$TMP_ROOT/lightfriend-full-backup-test.tar.gz" "$TUWUNEL_BACKUP_ROOT/data"

(
    exec 8>"$LOCK_FILE"
    flock -x 8
    touch "$LOCK_READY"
    sleep 30
) &
LOCK_HOLDER_PID=$!

for _ in $(seq 1 50); do
    [ -e "$LOCK_READY" ] && break
    sleep 0.1
done
[ -e "$LOCK_READY" ] || { echo "lock holder did not start" >&2; exit 1; }

LOCKED_OUTPUT=$(env \
    LIGHTFRIEND_BACKUP_ARTIFACT_LOCK_FILE="$LOCK_FILE" \
    LIGHTFRIEND_BACKUP_STAGING_ROOT="$STAGING_ROOT" \
    LIGHTFRIEND_BACKUP_TMP_ROOT="$TMP_ROOT" \
    LIGHTFRIEND_BACKUP_RESTORE_ROOT="$RESTORE_ROOT" \
    LIGHTFRIEND_BACKUP_SEED_DIR="$SEED_ROOT" \
    TUWUNEL_BACKUP_DIR="$TUWUNEL_BACKUP_ROOT" \
    "$STORAGE_HEALTH" cleanup-backup-artifacts)

grep -q "export holds lock" <<<"$LOCKED_OUTPUT"
[ -f "$STAGING_ROOT/active/data" ]
[ -f "$TMP_ROOT/lightfriend-full-backup-test.tar.gz" ]
[ -f "$TUWUNEL_BACKUP_ROOT/data" ]

kill "$LOCK_HOLDER_PID"
wait "$LOCK_HOLDER_PID" 2>/dev/null || true
LOCK_HOLDER_PID=""

env \
    LIGHTFRIEND_BACKUP_ARTIFACT_LOCK_FILE="$LOCK_FILE" \
    LIGHTFRIEND_BACKUP_STAGING_ROOT="$STAGING_ROOT" \
    LIGHTFRIEND_BACKUP_TMP_ROOT="$TMP_ROOT" \
    LIGHTFRIEND_BACKUP_RESTORE_ROOT="$RESTORE_ROOT" \
    LIGHTFRIEND_BACKUP_SEED_DIR="$SEED_ROOT" \
    TUWUNEL_BACKUP_DIR="$TUWUNEL_BACKUP_ROOT" \
    "$STORAGE_HEALTH" cleanup-backup-artifacts >/dev/null

[ ! -e "$STAGING_ROOT" ]
[ ! -e "$TMP_ROOT/lightfriend-full-backup-test.tar.gz" ]
[ ! -e "$TUWUNEL_BACKUP_ROOT" ]

if grep -q 'rm -rf /tmp/backup-staging' "$EXPORT_SCRIPT"; then
    echo "export cleanup must not remove the global staging root" >&2
    exit 1
fi
grep -Fq "rm -rf \"\${STAGING}\"" "$EXPORT_SCRIPT"
grep -Fq "> \"\${CHECKSUMS_TMP}\"" "$EXPORT_SCRIPT"

echo "backup artifact lock regression test passed"
