#!/bin/bash
set -euo pipefail

# Post-startup verification script for enclave.
# Validates the enclave is fully operational after a restore (or anytime).
# Writes result to /data/seed/verify-result.json.

TIMESTAMP=$(date -u +%Y%m%dT%H%M%SZ)
RESULT_FILE="/data/seed/verify-result.json"
FAILED_CHECKS=()
CHECK_DETAILS=""
ALL_CHECKS=()

echo "=== Lightfriend Enclave Verification ==="
echo "Timestamp: ${TIMESTAMP}"

record_check() {
    local name="$1"
    local passed="$2"
    local detail="${3:-}"
    ALL_CHECKS+=("\"${name}\": ${passed}")
    if [ "${passed}" = "false" ]; then
        FAILED_CHECKS+=("${name}")
        if [ -n "${detail}" ]; then
            CHECK_DETAILS="${CHECK_DETAILS}${name}: ${detail}; "
        fi
    fi
}

# ── 1. All supervisord processes running ────────────────────────────────────

echo "Check 1: supervisord processes..."
SUP_STATUS=$(supervisorctl status 2>/dev/null || echo "FAILED")
SUP_OK=true
for proc in postgresql tuwunel mautrix-whatsapp mautrix-signal mautrix-telegram lightfriend; do
    if ! echo "${SUP_STATUS}" | grep -q "${proc}.*RUNNING"; then
        SUP_OK=false
        echo "  FAIL: ${proc} not RUNNING"
    fi
done
if [ "${SUP_OK}" = "true" ]; then
    echo "  OK: all processes running"
    record_check "supervisord_processes" "true"
else
    record_check "supervisord_processes" "false" "one or more processes not RUNNING"
fi

# ── 2. PostgreSQL accessible ───────────────────────────────────────────────

echo "Check 2: PostgreSQL accessible..."
DB_USER_COUNT=$(psql -h localhost -U lightfriend -d lightfriend_db -t -A \
    -c "SELECT count(*) FROM users" 2>/dev/null || echo "ERROR")
if [ "${DB_USER_COUNT}" != "ERROR" ] && [ "${DB_USER_COUNT}" -gt 0 ] 2>/dev/null; then
    echo "  OK: ${DB_USER_COUNT} users in database"
    record_check "postgresql_accessible" "true"
else
    echo "  FAIL: could not query users (got: ${DB_USER_COUNT})"
    record_check "postgresql_accessible" "false" "user count query returned: ${DB_USER_COUNT}"
fi

# ── 3. Backend health ─────────────────────────────────────────────────────

echo "Check 3: backend health..."
if curl -sf http://localhost:3000/api/health > /dev/null 2>&1; then
    echo "  OK: backend responding"
    record_check "backend_health" "true"
else
    echo "  FAIL: backend not responding at /api/health"
    record_check "backend_health" "false" "curl to /api/health failed"
fi

# ── 4. Tuwunel health ─────────────────────────────────────────────────────

echo "Check 4: Tuwunel health..."
if curl -sf http://localhost:8008/_matrix/client/versions > /dev/null 2>&1; then
    echo "  OK: Tuwunel responding"
    record_check "tuwunel_health" "true"
else
    echo "  FAIL: Tuwunel not responding"
    record_check "tuwunel_health" "false" "curl to /_matrix/client/versions failed"
fi

# ── 5. Bridge processes alive ──────────────────────────────────────────────

echo "Check 5: bridge processes..."
BRIDGE_OK=true
for bridge in mautrix-whatsapp mautrix-signal mautrix-telegram; do
    if echo "${SUP_STATUS}" | grep -q "${bridge}.*RUNNING"; then
        echo "  OK: ${bridge} running"
    else
        BRIDGE_OK=false
        echo "  FAIL: ${bridge} not running"
    fi
done
if [ "${BRIDGE_OK}" = "true" ]; then
    record_check "bridge_processes" "true"
else
    record_check "bridge_processes" "false" "one or more bridges not RUNNING"
fi

# ── 6. Data integrity spot-check ───────────────────────────────────────────

echo "Check 6: data integrity..."
MANIFEST="/tmp/backup-manifest.json"
if [ -f "${MANIFEST}" ]; then
    EXPECTED_COUNT=$(cat "${MANIFEST}" | grep -o '"user_count": [0-9]*' | grep -o '[0-9]*')
    if [ -n "${EXPECTED_COUNT}" ] && [ "${DB_USER_COUNT}" != "ERROR" ]; then
        if [ "${DB_USER_COUNT}" -eq "${EXPECTED_COUNT}" ]; then
            echo "  OK: user count matches manifest (${DB_USER_COUNT})"
            record_check "data_integrity" "true"
        else
            echo "  FAIL: user count ${DB_USER_COUNT} != manifest ${EXPECTED_COUNT}"
            record_check "data_integrity" "false" "user count ${DB_USER_COUNT} != manifest ${EXPECTED_COUNT}"
        fi
    else
        echo "  FAIL: manifest exists but could not parse user count or query database"
        record_check "data_integrity" "false" "manifest present but comparison failed: expected=${EXPECTED_COUNT:-missing} actual=${DB_USER_COUNT}"
    fi
else
    echo "  SKIP: no manifest available (not a restore verification)"
    record_check "data_integrity" "true"
fi

# ── 7. Bridge databases have schema ───────────────────────────────────

echo "Check 7: bridge database schemas..."
BRIDGE_DB_OK=true
for bridge_db in whatsapp_db signal_db telegram_db; do
    TABLE_COUNT=$(psql -h localhost -U postgres -d "$bridge_db" -t -A \
        -c "SELECT count(*) FROM information_schema.tables WHERE table_schema='public'" 2>/dev/null || echo "0")
    if [ "${TABLE_COUNT}" -gt 0 ]; then
        echo "  OK: ${bridge_db} has ${TABLE_COUNT} tables"
    else
        BRIDGE_DB_OK=false
        echo "  FAIL: ${bridge_db} has no tables"
    fi
done
if [ "${BRIDGE_DB_OK}" = "true" ]; then
    record_check "bridge_databases" "true"
else
    record_check "bridge_databases" "false" "one or more bridge databases have no tables"
fi

# ── 8. Tuwunel data directory ────────────────────────────────────────

echo "Check 8: Tuwunel data..."
TUWUNEL_FILES=$(find /var/lib/tuwunel -type f 2>/dev/null | wc -l)
if [ "${TUWUNEL_FILES}" -gt 0 ]; then
    echo "  OK: /var/lib/tuwunel has ${TUWUNEL_FILES} files"
    record_check "tuwunel_data" "true"
else
    echo "  FAIL: /var/lib/tuwunel is empty"
    record_check "tuwunel_data" "false" "tuwunel data directory is empty"
fi

# ── 9. Cloudflared tunnel connected ──────────────────────────────────────

echo "Check 9: cloudflared tunnel..."
if [ -n "${CLOUDFLARE_TUNNEL_TOKEN:-}" ]; then
    CF_STATUS=$(supervisorctl status cloudflared 2>/dev/null || echo "")
    if echo "${CF_STATUS}" | grep -q "RUNNING"; then
        echo "  OK: cloudflared running"
        record_check "cloudflared" "true"
    else
        echo "  FAIL: cloudflared not running"
        record_check "cloudflared" "false" "cloudflared process not RUNNING"
    fi
else
    echo "  SKIP: no CLOUDFLARE_TUNNEL_TOKEN set"
    record_check "cloudflared" "true"
fi

# ── Write result ───────────────────────────────────────────────────────────

CHECKS_JSON=$(printf '%s' "${ALL_CHECKS[*]}" | sed 's/" "/", "/g')

# Determine restore type
if [ -f /tmp/backup-manifest.json ]; then
    MANIFEST_FORMAT=$(cat /tmp/backup-manifest.json | grep -o '"format": "[^"]*"' | grep -o '"[^"]*"$' | tr -d '"')
    if [ "${MANIFEST_FORMAT}" = "lightfriend-pg-backup" ]; then
        RESTORE_TYPE="pg_only_restore"
    else
        RESTORE_TYPE="full_restore"
    fi
else
    RESTORE_TYPE="fresh_start"
fi

if [ ${#FAILED_CHECKS[@]} -eq 0 ]; then
    cat > "${RESULT_FILE}" <<EOF
{"status": "HEALTHY", "restore_type": "${RESTORE_TYPE}", "timestamp": "${TIMESTAMP}", "checks": {${CHECKS_JSON}}, "user_count": ${DB_USER_COUNT:-0}}
EOF
    echo ""
    echo "=== Verification: HEALTHY ==="
    exit 0
else
    FAILED_LIST=$(printf '"%s", ' "${FAILED_CHECKS[@]}")
    FAILED_LIST="${FAILED_LIST%, }"
    cat > "${RESULT_FILE}" <<EOF
{"status": "FAILED", "restore_type": "${RESTORE_TYPE}", "timestamp": "${TIMESTAMP}", "failed_checks": [${FAILED_LIST}], "details": "${CHECK_DETAILS}", "user_count": ${DB_USER_COUNT:-0}}
EOF
    echo ""
    echo "=== Verification: FAILED ==="
    echo "Failed checks: ${FAILED_CHECKS[*]}"
    exit 1
fi
