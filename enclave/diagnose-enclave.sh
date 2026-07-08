#!/bin/bash
# Diagnostic script - runs inside the enclave when host connects to VSOCK port 9008.
# Dumps all supervisor service status, bridge health, and recent logs.
#
# PRIVACY MODEL (this script is open-source and auditable on GitHub):
#   Allowed in output: the account owner's own identifiers — their phone number,
#   email, Lightfriend user_id, Matrix appuser id, Twilio SIDs, and operational
#   metadata (connection state, error messages, timing, RSS, etc.).
#   Never in output: message content, contact names, contact phone numbers,
#   contact WhatsApp/Signal/Telegram IDs, group chat identifiers, OAuth
#   bearer tokens, API keys, webhook secrets, JWTs.
# Bridge logs (mautrix-*) get aggressive sanitization because they're full of
# contact data; backend logs get light sanitization (secrets only) because
# operator-visible user identifiers are expected there.

# Bridge log sanitizer: strips message content AND contact identifiers.
# Applied to mautrix-whatsapp, mautrix-signal, mautrix-telegram logs.
sanitize_bridge_log() {
    sed -E \
        -e 's/(body|message|text|content|caption)="[^"]*"/\1="[REDACTED]"/gi' \
        -e 's/(body|message|text|content|caption): .*/\1: [REDACTED]/gi' \
        -e 's/("body"|"message"|"text"|"content"|"caption"): ?"[^"]*"/\1: "[REDACTED]"/gi' \
        -e 's/"name":"[^"]*"/"name":"[CONTACT]"/g' \
        -e 's/"pushname":"[^"]*"/"pushname":"[CONTACT]"/g' \
        -e 's/"first_name":"[^"]*"/"first_name":"[CONTACT]"/g' \
        -e 's/"last_name":"[^"]*"/"last_name":"[CONTACT]"/g' \
        -e 's/sender_name=[^ ]+/sender_name=[CONTACT]/g' \
        -e 's/[0-9]{5,20}@s\.whatsapp\.net/[WA_JID]/g' \
        -e 's/[0-9]{5,20}@lid/[WA_LID]/g' \
        -e 's/[0-9]{5,20}@g\.us/[WA_GROUP]/g' \
        -e 's/lid-[0-9]+/[WA_LID]/g' \
        -e 's/"number":"\+?[0-9]+"/"number":"[CONTACT_PHONE]"/g' \
        -e 's/"sender_login":"[^"]*"/"sender_login":"[CONTACT]"/g' \
        -e 's/destination_service_id=[a-f0-9-]{36}/destination_service_id=[SIG_UUID]/g' \
        -e 's/source_id=[0-9]+/source_id=[CONTACT]/g' \
        -e 's/(telethon\.)[0-9]+(\.)/\1[TG_UID]\2/g'
}

# Backend log sanitizer: the operator is expected to see user identifiers
# (phone, email, user_id), so only strip credentials. Applied to the backend
# (lightfriend) stdout and stderr tails, including rotated files.
sanitize_backend_log() {
    sed -E \
        -e 's/([Bb]earer )[A-Za-z0-9._~+/=-]{8,}/\1[REDACTED]/g' \
        -e 's/([Aa]uthorization:[[:space:]]*)[^[:space:],}"]+/\1[REDACTED]/g' \
        -e 's/("(access_token|refresh_token|id_token|api_key|apiKey|client_secret|secret|password|passwd|webhook_secret|bearer|auth|token)"[[:space:]]*:[[:space:]]*)"[^"]*"/\1"[REDACTED]"/gi' \
        -e 's/(^|[^A-Za-z0-9_])(access_token|refresh_token|id_token|api_key|apiKey|client_secret|password|passwd|webhook_secret|bearer)=([^&"[:space:]]+)/\1\2=[REDACTED]/gi' \
        -e 's/eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}/[JWT]/g' \
        -e 's/sk-[A-Za-z0-9]{20,}/[API_KEY]/g' \
        -e 's/xoxb-[A-Za-z0-9-]{20,}/[API_KEY]/g'
}

echo "=== Enclave Diagnostics $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="
echo ""

# ── CRITICAL: Dump all service logs FIRST before any health checks ──
# Health checks can hang if services are unresponsive, so we dump
# the actual application logs upfront to always capture them.
echo "--- backend stderr (last 80 lines) ---"
tail -80 /var/log/supervisor/lightfriend-err.log 2>/dev/null | sanitize_backend_log || echo "  empty"
echo ""

echo "--- backend stdout (last 100 lines, excluding IDLE noise) ---"
tail -500 /var/log/supervisor/lightfriend.log 2>/dev/null | grep -v "IDLE established\|Spawned IDLE task\|Job creator created\|Uninited" | tail -100 | sanitize_backend_log || echo "  empty"
echo ""

# ── Crash context: previous-life supervisor logs + kernel events ──
# The current lightfriend.log/.err starts fresh after each supervisor restart.
# Panic/backtrace that caused the restart lives in the rotated .log.1/.err.log.1.
# Also surface supervisord's own record of restart events and any kernel OOM kills.
echo "--- backend PREVIOUS stderr (lightfriend-err.log.1 last 120 lines) ---"
tail -120 /var/log/supervisor/lightfriend-err.log.1 2>/dev/null | sanitize_backend_log || echo "  no .1 (never rotated)"
echo ""

echo "--- backend PREVIOUS stdout tail (lightfriend.log.1 last 150 lines, IDLE stripped) ---"
tail -500 /var/log/supervisor/lightfriend.log.1 2>/dev/null \
    | grep -v "IDLE established\|Spawned IDLE task\|Job creator created\|Uninited" \
    | tail -150 | sanitize_backend_log || echo "  no .1 (never rotated)"
echo ""

echo "--- backend two-lives-ago stdout tail (lightfriend.log.2 last 60 lines) ---"
tail -60 /var/log/supervisor/lightfriend.log.2 2>/dev/null | sanitize_backend_log || echo "  no .2"
echo ""

echo "--- crash signatures across all backend log rotations ---"
# Match Rust panics, backtraces, fatal tokio runtime events, signal-based kills.
# Grep both stdout and stderr rotations. Show file:line for correlation.
grep -HnE "panicked at|thread '[^']*' panicked|stack backtrace:|fatal runtime error|process didn't exit successfully|SIGKILL|SIGSEGV|SIGABRT|SIGBUS|abort\(\)|Killed\s*$|\(core dumped\)|thread panic|UNWIND" \
    /var/log/supervisor/lightfriend.log /var/log/supervisor/lightfriend.log.1 /var/log/supervisor/lightfriend.log.2 \
    /var/log/supervisor/lightfriend-err.log /var/log/supervisor/lightfriend-err.log.1 /var/log/supervisor/lightfriend-err.log.2 \
    2>/dev/null | tail -80 | sanitize_backend_log || echo "  none found"
echo ""

echo "--- Tuwunel cleanup instrumentation across backend logs ---"
TUWUNEL_CLEANUP_LOG_LINES=$(grep -hEi "Tuwunel event cleanup|Tuwunel cleanup admin command|cleanup_command_kind|media_delete_by_event|redact_event|cleanup instrumentation|cleanup exhausted|cleanup failed" \
    /var/log/supervisor/lightfriend.log /var/log/supervisor/lightfriend.log.1 /var/log/supervisor/lightfriend.log.2 \
    /var/log/supervisor/lightfriend-err.log /var/log/supervisor/lightfriend-err.log.1 /var/log/supervisor/lightfriend-err.log.2 \
    2>/dev/null | tail -120 || true)
if [ -n "$TUWUNEL_CLEANUP_LOG_LINES" ]; then
    printf '%s\n' "$TUWUNEL_CLEANUP_LOG_LINES" | sanitize_backend_log
else
    echo "  none found"
fi
echo ""

echo "--- Tuwunel cleanup audit table ---"
if command -v psql >/dev/null 2>&1 && [ -n "${PG_DATABASE_URL:-}" ]; then
    TUWUNEL_CLEANUP_SQL='
        SELECT status,
               count(*) AS rows,
               COALESCE(sum(commands_expected), 0) AS commands_expected,
               COALESCE(sum(commands_accepted), 0) AS commands_accepted,
               to_char(to_timestamp(max(updated_at)), '\''YYYY-MM-DD"T"HH24:MI:SS"Z"'\'') AS last_updated
          FROM tuwunel_cleanup_events
         GROUP BY status
         ORDER BY status;

        SELECT service,
               delete_media,
               status,
               count(*) AS rows,
               COALESCE(sum(commands_expected), 0) AS commands_expected,
               COALESCE(sum(commands_accepted), 0) AS commands_accepted,
               to_char(to_timestamp(min(updated_at)), '\''YYYY-MM-DD"T"HH24:MI:SS"Z"'\'') AS first_updated,
               to_char(to_timestamp(max(updated_at)), '\''YYYY-MM-DD"T"HH24:MI:SS"Z"'\'') AS last_updated
          FROM tuwunel_cleanup_events
         WHERE updated_at >= EXTRACT(EPOCH FROM NOW())::INT4 - (6 * 60 * 60)
         GROUP BY service, delete_media, status
         ORDER BY last_updated DESC, service, status;

        WITH now_epoch AS (
            SELECT EXTRACT(EPOCH FROM NOW())::INT4 AS ts
        ),
        windows(label, seconds) AS (
            VALUES ('\''last_5m'\'', 300),
                   ('\''last_1h'\'', 3600),
                   ('\''last_6h'\'', 21600),
                   ('\''last_24h'\'', 86400)
        )
        SELECT windows.label AS time_window,
               count(e.id) AS rows,
               COALESCE(sum(e.commands_expected), 0) AS commands_expected,
               COALESCE(sum(e.commands_accepted), 0) AS commands_accepted,
               count(e.id) FILTER (WHERE e.status IN ('\''exhausted'\'', '\''partial_commands_submitted'\'', '\''retrying'\'')) AS attention_rows,
               to_char(to_timestamp(max(e.updated_at)), '\''YYYY-MM-DD"T"HH24:MI:SS"Z"'\'') AS last_updated
          FROM windows
          CROSS JOIN now_epoch
          LEFT JOIN tuwunel_cleanup_events e
            ON e.enqueued_at >= now_epoch.ts - windows.seconds
         GROUP BY windows.label, windows.seconds
         ORDER BY windows.seconds;

        SELECT COALESCE(last_command_kind, '\''none'\'') AS last_command_kind,
               count(*) AS rows,
               to_char(to_timestamp(max(updated_at)), '\''YYYY-MM-DD"T"HH24:MI:SS"Z"'\'') AS last_updated
          FROM tuwunel_cleanup_events
         WHERE updated_at >= EXTRACT(EPOCH FROM NOW())::INT4 - (6 * 60 * 60)
         GROUP BY COALESCE(last_command_kind, '\''none'\'')
         ORDER BY rows DESC, last_command_kind;

        SELECT status,
               count(*) AS rows,
               to_char(to_timestamp(min(updated_at)), '\''YYYY-MM-DD"T"HH24:MI:SS"Z"'\'') AS oldest_updated,
               to_char(to_timestamp(max(updated_at)), '\''YYYY-MM-DD"T"HH24:MI:SS"Z"'\'') AS newest_updated,
               left(coalesce(max(last_error), '\'''\''), 240) AS sample_error
          FROM tuwunel_cleanup_events
         WHERE status IN ('\''enqueued'\'', '\''attempting'\'', '\''retrying'\'', '\''exhausted'\'', '\''partial_commands_submitted'\'')
            OR commands_accepted < commands_expected
         GROUP BY status
         ORDER BY oldest_updated NULLS LAST, status
         LIMIT 20;

        SELECT id,
               user_id,
               service,
               delete_media,
               commands_expected,
               commands_accepted,
               attempt_count,
               status,
               to_char(to_timestamp(updated_at), '\''YYYY-MM-DD"T"HH24:MI:SS"Z"'\'') AS updated_at,
               left(coalesce(last_error, '\'''\''), 240) AS last_error
          FROM tuwunel_cleanup_events
         ORDER BY updated_at DESC
         LIMIT 20;
    '
    if ! psql "$PG_DATABASE_URL" -v ON_ERROR_STOP=1 -c "$TUWUNEL_CLEANUP_SQL" 2>&1 | sanitize_backend_log; then
        echo "  audit table unavailable or query failed"
    fi
else
    echo "  psql or PG_DATABASE_URL unavailable"
fi
echo ""

echo "--- Tuwunel admin command traces across tuwunel logs ---"
TUWUNEL_ADMIN_LOG_LINES=$(grep -hEi "backup-database|delete-by-event|redact-event|redacted|admin command|admin room|compact|purge|cleanup" \
    /var/log/supervisor/tuwunel.log /var/log/supervisor/tuwunel.log.1 /var/log/supervisor/tuwunel.log.2 \
    /var/log/supervisor/tuwunel-err.log /var/log/supervisor/tuwunel-err.log.1 /var/log/supervisor/tuwunel-err.log.2 \
    2>/dev/null | tail -160 || true)
if [ -n "$TUWUNEL_ADMIN_LOG_LINES" ]; then
    printf '%s\n' "$TUWUNEL_ADMIN_LOG_LINES"
else
    echo "  none found"
fi
echo ""

echo "--- Postgres storage and ontology retention ---"
if command -v psql >/dev/null 2>&1 && [ -n "${PG_DATABASE_URL:-}" ]; then
    POSTGRES_STORAGE_SQL='
        SELECT schemaname || '\''.'\'' || relname AS relation,
               pg_size_pretty(pg_total_relation_size(format('\''%I.%I'\'', schemaname, relname)::regclass)) AS total_size,
               pg_size_pretty(pg_relation_size(format('\''%I.%I'\'', schemaname, relname)::regclass)) AS heap_size,
               pg_size_pretty(pg_indexes_size(format('\''%I.%I'\'', schemaname, relname)::regclass)) AS index_size,
               n_live_tup,
               n_dead_tup
          FROM pg_stat_user_tables
         ORDER BY pg_total_relation_size(format('\''%I.%I'\'', schemaname, relname)::regclass) DESC
         LIMIT 20;

        WITH bounds AS (
            SELECT EXTRACT(EPOCH FROM NOW())::INT4 - (30 * 24 * 60 * 60) AS cutoff
        )
        SELECT count(*) AS total_rows,
               count(*) FILTER (WHERE created_at < cutoff) AS older_than_30d,
               count(*) FILTER (WHERE created_at >= cutoff) AS last_30d,
               pg_size_pretty(pg_total_relation_size('\''ont_messages'\'')) AS ont_messages_total_size,
               to_char(to_timestamp(min(created_at)), '\''YYYY-MM-DD"T"HH24:MI:SS"Z"'\'') AS oldest_message,
               to_char(to_timestamp(max(created_at)), '\''YYYY-MM-DD"T"HH24:MI:SS"Z"'\'') AS newest_message
          FROM ont_messages, bounds;

        WITH bounds AS (
            SELECT EXTRACT(EPOCH FROM NOW())::INT4 - (30 * 24 * 60 * 60) AS cutoff
        )
        SELECT platform,
               count(*) AS total_rows,
               count(*) FILTER (WHERE created_at < cutoff) AS older_than_30d,
               count(*) FILTER (WHERE created_at >= cutoff) AS last_30d
          FROM ont_messages, bounds
         GROUP BY platform
         ORDER BY total_rows DESC
         LIMIT 20;
    '
    if ! psql "$PG_DATABASE_URL" -v ON_ERROR_STOP=1 -c "$POSTGRES_STORAGE_SQL" 2>&1 | sanitize_backend_log; then
        echo "  postgres storage query unavailable or failed"
    fi
else
    echo "  psql or PG_DATABASE_URL unavailable"
fi
echo ""

echo "--- supervisord restart events for lightfriend (last 50) ---"
grep -E "lightfriend.*(entered|exited|terminated|killed|spawned|fatal|backoff|ABNORMAL)" \
    /var/log/supervisor/supervisord.log /var/log/supervisor/supervisord.log.1 /var/log/supervisor/supervisord.log.2 \
    2>/dev/null | tail -50 || echo "  none"
echo ""

echo "--- kernel ring buffer tail (dmesg last 120 lines) ---"
dmesg -T --ctime 2>/dev/null | tail -120 || dmesg 2>/dev/null | tail -120 || echo "  dmesg not readable"
echo ""

echo "--- kernel OOM / killed-process events ---"
dmesg -T --ctime 2>/dev/null | grep -iE "out of memory|oom[_-]kill|killed process|invoked oom-killer|memory cgroup out of memory" | tail -40 \
    || dmesg 2>/dev/null | grep -iE "out of memory|oom[_-]kill|killed process|invoked oom-killer" | tail -40 \
    || echo "  no OOM events or dmesg not readable"
echo ""

echo "--- cloudflared stderr (last 40 lines) ---"
tail -40 /var/log/supervisor/cloudflared-err.log 2>/dev/null || echo "  empty"
echo ""

echo "--- cloudflared stdout (last 20 lines) ---"
tail -20 /var/log/supervisor/cloudflared.log 2>/dev/null || echo "  empty"
echo ""

# ── Bridge logs (privacy-filtered: no message content) ──
echo "--- mautrix-whatsapp stdout (last 60 lines, sanitized) ---"
tail -60 /var/log/supervisor/whatsapp.log 2>/dev/null | sanitize_bridge_log || echo "  empty"
echo ""

echo "--- mautrix-whatsapp stderr (last 40 lines, sanitized) ---"
tail -40 /var/log/supervisor/whatsapp-err.log 2>/dev/null | sanitize_bridge_log || echo "  empty"
echo ""

echo "--- mautrix-signal stdout (last 40 lines, sanitized) ---"
tail -40 /var/log/supervisor/signal.log 2>/dev/null | sanitize_bridge_log || echo "  empty"
echo ""

echo "--- mautrix-signal crypto/session errors (last 20, sanitized) ---"
grep -Ei "Decryption error|failed to decrypt|Failed to verify ACI-PNI mapping|failed to fetch prekey|identity was.?t found in store" \
    /var/log/supervisor/signal.log 2>/dev/null | tail -20 | sanitize_bridge_log || echo "  none"
echo ""

echo "--- mautrix-signal stderr (last 20 lines, sanitized) ---"
tail -20 /var/log/supervisor/signal-err.log 2>/dev/null | sanitize_bridge_log || echo "  empty"
echo ""

echo "--- mautrix-telegram stdout (last 80 lines, sanitized) ---"
tail -80 /var/log/supervisor/telegram.log 2>/dev/null | sanitize_bridge_log || echo "  empty"
echo ""

echo "--- mautrix-telegram stderr (last 40 lines, sanitized) ---"
tail -40 /var/log/supervisor/telegram-err.log 2>/dev/null | sanitize_bridge_log || echo "  empty"
echo ""

echo "--- tuwunel stdout (last 40 lines) ---"
tail -40 /var/log/supervisor/tuwunel.log 2>/dev/null || echo "  empty"
echo ""

echo "--- tuwunel stderr (last 40 lines) ---"
tail -40 /var/log/supervisor/tuwunel-err.log 2>/dev/null || echo "  empty"
echo ""

echo "--- startup-services.log (last 100 lines) ---"
tail -100 /data/seed/startup-services.log 2>/dev/null || echo "  not found"
echo ""

echo "--- service-watchdog stdout (last 200 lines) ---"
tail -200 /var/log/supervisor/service-watchdog.log 2>/dev/null || echo "  empty"
echo ""

echo "--- service-watchdog stderr (last 80 lines) ---"
tail -80 /var/log/supervisor/service-watchdog-err.log 2>/dev/null || echo "  empty"
echo ""

echo "--- storage cleanup last run (last 240 lines) ---"
tail -240 /tmp/storage-cleanup-last.log 2>/dev/null || echo "  no cleanup run recorded yet"
echo ""

echo "--- storage health ---"
if [ -x /app/storage-health.sh ]; then
    /app/storage-health.sh report 2>&1
    echo "--- storage health history (last 80 lines) ---"
    tail -80 /tmp/storage-health-history.log 2>/dev/null || echo "  no history yet"
else
    df -h 2>&1 || true
    df -i 2>&1 || true
fi
echo ""

echo "--- rootfs reserve status ---"
if [ -e /var/lib/lightfriend-reserve/rootfs-reserve.bin ]; then
    ls -lh /var/lib/lightfriend-reserve/rootfs-reserve.bin 2>/dev/null || true
    du -h /var/lib/lightfriend-reserve/rootfs-reserve.bin 2>/dev/null || true
else
    echo "  reserve released or not present"
fi
if [ -f /data/seed/reserve-release-status.json ]; then
    cat /data/seed/reserve-release-status.json 2>/dev/null || true
fi
echo ""

echo "--- memory health ---"
if [ -x /app/memory-health.sh ]; then
    /app/memory-health.sh report 2>&1
    echo "--- memory health history (last 80 lines) ---"
    tail -80 /tmp/memory-health-history.log 2>/dev/null || echo "  no history yet"
else
    free -h 2>&1 || true
    ps -eo pid,ppid,rss,comm,args --sort=-rss 2>/dev/null | head -25 || true
fi
echo ""

echo "--- host health upload ---"
tail -80 /tmp/health-upload-last.log 2>/dev/null || echo "  no upload attempt recorded yet"
echo "--- enclave health upload history (last 80 lines) ---"
tail -80 /tmp/enclave-health-history.log 2>/dev/null || echo "  no persistent health history yet"
echo ""

echo "--- export-watcher-last-run.log (last 80 lines) ---"
tail -80 /tmp/export-watcher-last-run.log 2>/dev/null || echo "  not found (no export has run yet)"
echo ""

echo "--- boot-trace.log restore section (grep DEBUG/restore/tuwunel/bridge) ---"
grep -i "DEBUG\|restore\|tuwunel\|bridge.*tar\|matrix_store\|STEP 2\|decrypt\|Full restore\|checkpoint\|RocksDB\|CURRENT\|IDENTITY\|file count\|total size\|MANIFEST\|sst\|whatsmeow\|user_login" /data/seed/boot-trace.log 2>/dev/null | head -100 || echo "  not found or no matches"
echo ""

echo "--- boot-trace.log (last 40 lines) ---"
tail -40 /data/seed/boot-trace.log 2>/dev/null || echo "  not found"
echo ""

echo "--- supervisorctl status ---"
supervisorctl status 2>&1
echo ""

echo "--- telegram SOCKS5 proxy check ---"
echo "socat 1080 listener: $(ss -tlnp 2>/dev/null | grep ':1080' || echo 'NOT LISTENING')"
echo "socat 1080 processes: $(pgrep -la socat 2>/dev/null | grep 1080 || echo 'none')"
echo "VSOCK 8500 connections: $(ss -tnp 2>/dev/null | grep '8500' | wc -l)"
echo ""

echo "--- env check ---"
echo "PG_DATABASE_URL: ${PG_DATABASE_URL:+set (${#PG_DATABASE_URL} chars)}"
echo "DATABASE_URL: ${DATABASE_URL:+set (${#DATABASE_URL} chars)}"
echo "PORT: ${PORT:-not set}"
echo "HTTP_PROXY: ${HTTP_PROXY:-not set}"
echo "NO_PROXY: ${NO_PROXY:-not set}"
echo "CLOUDFLARE_TUNNEL_TOKEN: ${CLOUDFLARE_TUNNEL_TOKEN:+set (${#CLOUDFLARE_TUNNEL_TOKEN} chars)}"
echo ""

echo "--- postgres check ---"
pg_isready -h localhost -U postgres 2>&1 || echo "pg_isready failed"
echo ""

echo "--- backend health ---"
echo "  port ${PORT:-3100}: $(ss -tlnp 2>/dev/null | grep ":${PORT:-3100}" || echo 'NOT LISTENING')"
# Use subshell with kill to ensure we never hang
(curl -sf --max-time 1 --connect-timeout 1 http://localhost:${PORT:-3100}/api/health 2>&1 & CPID=$!; sleep 2; kill $CPID 2>/dev/null; wait $CPID 2>/dev/null) || echo "backend not responding (hung or crashed)"
echo ""

echo "--- tuwunel health ---"
echo "  port 8008: $(ss -tlnp 2>/dev/null | grep ':8008' || echo 'NOT LISTENING')"
(curl -sf --max-time 1 --connect-timeout 1 http://localhost:8008/_matrix/client/versions 2>&1 & CPID=$!; sleep 2; kill $CPID 2>/dev/null; wait $CPID 2>/dev/null) || echo "tuwunel not responding"
echo ""

echo "--- backend process details ---"
BACKEND_PID=$(pgrep -f '/app/backend' 2>/dev/null | head -1)
if [ -n "$BACKEND_PID" ]; then
    echo "  PID: $BACKEND_PID"
    echo "  RSS: $(ps -o rss= -p "$BACKEND_PID" 2>/dev/null | tr -d ' ')KB"
    echo "  Threads: $(ls /proc/$BACKEND_PID/task 2>/dev/null | wc -l)"
    echo "  FDs: $(ls /proc/$BACKEND_PID/fd 2>/dev/null | wc -l)"
    echo "  Open files: $(ls -la /proc/$BACKEND_PID/fd 2>/dev/null | grep -c socket)"
    echo "  TCP connections from backend:"
    ss -tnp 2>/dev/null | grep "pid=$BACKEND_PID" | head -20
    echo "  State: $(cat /proc/$BACKEND_PID/status 2>/dev/null | grep -E 'State|Threads|VmRSS|VmSize|FDSize')"
else
    echo "  Backend process NOT FOUND!"
fi
echo ""

echo "--- tap0 networking (gvisor-tap-vsock) ---"
if ip addr show tap0 2>/dev/null | grep -q 'inet '; then
    echo "  tap0: UP"
    ip addr show tap0 2>/dev/null | grep -E 'inet |link/ether'
    echo "  default route: $(ip route show default 2>/dev/null)"
    echo "  gvforwarder: $(pgrep -f gvforwarder >/dev/null 2>&1 && echo 'running' || echo 'NOT running')"
    echo "  DNS: $(cat /etc/resolv.conf 2>/dev/null | head -3)"
    echo "  gateway ping: $(timeout 2 ping -c1 -W1 192.168.127.1 >/dev/null 2>&1 && echo 'OK' || echo 'FAIL')"
    echo "  internet test: $(timeout 3 curl -sf --noproxy '*' --max-time 2 https://api.cloudflare.com/cdn-cgi/trace 2>/dev/null | head -1 || echo 'FAIL')"
    echo "  gvforwarder log (last 5 lines):"
    tail -5 /var/log/gvforwarder.log 2>/dev/null || echo "    no log"
else
    echo "  tap0: NOT CONFIGURED"
    echo "  gvforwarder: $(pgrep -f gvforwarder >/dev/null 2>&1 && echo 'running but no IP' || echo 'NOT running')"
    if [ -f /var/log/gvforwarder.log ]; then
        echo "  gvforwarder log (last 10 lines):"
        tail -10 /var/log/gvforwarder.log 2>/dev/null
    fi
fi
echo ""

echo "--- network: all listeners ---"
ss -tlnp 2>/dev/null
echo ""

echo "--- network: all established connections ---"
ss -tnp 2>/dev/null
echo ""

echo "--- /etc/hosts ---"
cat /etc/hosts 2>/dev/null
echo ""

echo "--- DNS resolution ---"
echo "  region1.v2.argotunnel.com: $(getent hosts region1.v2.argotunnel.com 2>&1 || echo 'FAILED')"
echo "  region2.v2.argotunnel.com: $(getent hosts region2.v2.argotunnel.com 2>&1 || echo 'FAILED')"
echo ""

echo "--- lo interface (check 1.1.1.1 bound) ---"
ip addr show lo 2>/dev/null
echo ""

# ── Cloudflared-specific diagnostics ──
echo "=============================================="
echo "=== CLOUDFLARED DIAGNOSTICS ==="
echo "=============================================="
echo ""

echo "--- bridge 7844 (cloudflared edge) ---"
echo "  listening: $(ss -tlnp 2>/dev/null | grep ':7844' || echo 'NOT LISTENING!')"
echo "  active connections: $(ss -tnp 2>/dev/null | grep ':7844' | wc -l)"
echo "  connection states:"
ss -tn 2>/dev/null | grep ':7844' || echo "    none"
echo "  supervisor: $(supervisorctl status vsock-bridge-7844 2>&1)"
echo "  socat PIDs: $(pgrep -f 'socat.*7844' 2>/dev/null | tr '\n' ' ' || echo 'none')"
echo ""

echo "--- bridge 853 (DoT) ---"
echo "  listening: $(ss -tlnp 2>/dev/null | grep ':853' || echo 'NOT LISTENING!')"
echo "  active connections: $(ss -tnp 2>/dev/null | grep ':853' | wc -l)"
echo "  supervisor: $(supervisorctl status vsock-bridge-dot 2>&1)"
echo ""

echo "--- cloudflared process ---"
CF_PID=$(pgrep -f 'cloudflared tunnel' 2>/dev/null | head -1)
if [ -n "$CF_PID" ]; then
    echo "  PID: $CF_PID"
    echo "  RSS: $(ps -o rss= -p "$CF_PID" 2>/dev/null | tr -d ' ')KB"
    echo "  Uptime: $(ps -o etime= -p "$CF_PID" 2>/dev/null | tr -d ' ')"
    echo "  FDs: $(ls /proc/$CF_PID/fd 2>/dev/null | wc -l)"
    echo "  TCP connections from cloudflared:"
    ss -tnp 2>/dev/null | grep "pid=$CF_PID" || echo "    none"
else
    echo "  NOT RUNNING!"
fi
echo ""

echo "--- TCP connectivity test to bridge ---"
if timeout 5 bash -c "echo DIAG_TEST | socat -T3 - TCP:127.0.0.1:7844,connect-timeout=3" 2>&1; then
    echo "  TCP:7844 test: PASS"
else
    echo "  TCP:7844 test: FAIL (rc=$?)"
fi
echo ""

echo "--- cloudflared-diag log (full) ---"
cat /var/log/supervisor/cloudflared-diag.log 2>/dev/null || echo "  no diag log"
echo ""

echo "--- cloudflared-monitor detail log (last 50 lines) ---"
tail -50 /var/log/supervisor/cloudflared-monitor-detail.log 2>/dev/null || echo "  no monitor log"
echo ""

echo "--- cloudflared stderr (last 50 lines) ---"
tail -50 /var/log/supervisor/cloudflared-err.log 2>/dev/null || echo "  empty"
echo ""

echo "--- cloudflared stdout (last 50 lines) ---"
tail -50 /var/log/supervisor/cloudflared.log 2>/dev/null || echo "  empty"
echo ""

echo "--- vsock-7844 bridge log (last 30 lines) ---"
tail -30 /var/log/supervisor/vsock-7844.log 2>/dev/null || echo "  no log"
tail -30 /var/log/supervisor/vsock-7844-err.log 2>/dev/null || echo "  no err log"
echo ""

echo "--- vsock-dot bridge log (last 20 lines) ---"
tail -20 /var/log/supervisor/vsock-dot.log 2>/dev/null || echo "  no log"
tail -20 /var/log/supervisor/vsock-dot-err.log 2>/dev/null || echo "  no err log"
echo ""

# ── End cloudflared section ──

echo "--- port 9080: $(ss -tlnp 2>/dev/null | grep ':9080' || echo 'not listening')"
echo "--- port 9081: $(ss -tlnp 2>/dev/null | grep ':9081' || echo 'not listening')"
echo ""

echo "--- KMS derive test ---"
if curl -sf --max-time 5 http://127.0.0.1:1101/derive/x25519?path=lightfriend/backup > /tmp/diag-key1.bin 2>/dev/null; then
    curl -sf --max-time 5 http://127.0.0.1:1101/derive/x25519?path=lightfriend/backup > /tmp/diag-key2.bin 2>/dev/null
    FP1=$(cat /tmp/diag-key1.bin | base64 | tr -d '\n' | sha256sum | cut -c1-16)
    FP2=$(cat /tmp/diag-key2.bin | base64 | tr -d '\n' | sha256sum | cut -c1-16)
    SZ1=$(stat -c%s /tmp/diag-key1.bin 2>/dev/null || echo "?")
    echo "  derive call 1: ${SZ1} bytes, fp=${FP1}"
    echo "  derive call 2: ${SZ1} bytes, fp=${FP2}"
    if [ "$FP1" = "$FP2" ]; then echo "  DETERMINISTIC: yes"; else echo "  DETERMINISTIC: NO - keys differ!"; fi
    ENVFP=$(printf '%s' "${BACKUP_ENCRYPTION_KEY:-}" | sha256sum | cut -c1-16)
    echo "  env BACKUP_ENCRYPTION_KEY fp=${ENVFP} len=${#BACKUP_ENCRYPTION_KEY}"
    rm -f /tmp/diag-key1.bin /tmp/diag-key2.bin
else
    echo "  derive server not reachable on port 1101"
fi
echo ""

echo "--- post-boot-verify log (last 20 lines) ---"
tail -20 /var/log/supervisor/post-boot-verify.log 2>/dev/null || echo "  no log"
echo ""

echo "--- post-boot-verify-err log (last 10 lines) ---"
tail -10 /var/log/supervisor/post-boot-verify-err.log 2>/dev/null || echo "  no log"
echo ""

echo "--- export-watcher test ---"
echo "  curl 9080: $(curl -sf --max-time 3 http://127.0.0.1:9080/ 2>&1 | head -1 || echo 'unreachable')"
echo "  curl 9081: $(curl -sf --max-time 3 http://127.0.0.1:9081/ 2>&1 | head -1 || echo 'unreachable')"
echo "  last run log: $(tail -5 /tmp/export-watcher-last-run.log 2>/dev/null || echo 'none')"
echo ""

echo "--- all processes ---"
ps aux 2>/dev/null
echo ""

for svc in lightfriend cloudflared tuwunel postgresql whatsapp signal telegram export-watcher vsock-bridge-9080 vsock-bridge-9081 vsock-bridge-7844 vsock-bridge-dot cloudflared-monitor; do
    LOG="/var/log/supervisor/${svc}-err.log"
    if [ -f "$LOG" ] && [ -s "$LOG" ]; then
        echo "--- ${svc} stderr (last 30 lines) ---"
        tail -30 "$LOG"
        echo ""
    fi
    LOG="/var/log/supervisor/${svc}.log"
    if [ -f "$LOG" ] && [ -s "$LOG" ]; then
        echo "--- ${svc} stdout (last 15 lines) ---"
        tail -15 "$LOG"
        echo ""
    fi
done

echo "--- supervisord.log (last 20 lines) ---"
tail -20 /var/log/supervisor/supervisord.log 2>/dev/null
echo ""

echo "=== End Diagnostics ==="
