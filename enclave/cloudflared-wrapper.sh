#!/bin/bash
# Wrapper for cloudflared that logs extensive diagnostics before and during tunnel operation.
# This helps debug edge connectivity issues through the VSOCK bridge chain:
#   cloudflared -> TCP:127.0.0.1:7844 -> socat -> VSOCK:3:7844 -> host socat -> TCP:edge:7844

set -euo pipefail

LOG="/var/log/supervisor/cloudflared-diag.log"
TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)

log() { echo "[$TIMESTAMP] $*" >> "$LOG"; }
logn() { echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $*" >> "$LOG"; }

echo "" >> "$LOG"
echo "================================================================" >> "$LOG"
log "=== cloudflared-wrapper starting ==="
log "PID: $$, PPID: $PPID"

# ── Version ──
log "cloudflared version: $(/usr/local/bin/cloudflared --version 2>&1 || echo 'FAILED')"

# ── Network interfaces ──
log "--- Network interfaces ---"
ip addr show 2>/dev/null >> "$LOG" || log "  ip addr failed"

# ── /etc/hosts ──
log "--- /etc/hosts entries ---"
cat /etc/hosts >> "$LOG" 2>/dev/null || log "  can't read /etc/hosts"

# ── DNS resolution tests ──
log "--- DNS resolution tests ---"
log "  getent region1.v2.argotunnel.com: $(getent hosts region1.v2.argotunnel.com 2>&1 || echo 'FAILED')"
log "  getent region2.v2.argotunnel.com: $(getent hosts region2.v2.argotunnel.com 2>&1 || echo 'FAILED')"

# ── Port listening state ──
log "--- Port listening state ---"
log "  All listeners:"
ss -tlnp 2>/dev/null >> "$LOG" || log "  ss failed"

# ── Check VSOCK bridge for 7844 ──
log "--- VSOCK bridge 7844 health ---"
if ss -tlnp 2>/dev/null | grep -q ':7844'; then
    log "  Port 7844: LISTENING (bridge alive)"
else
    log "  Port 7844: NOT LISTENING - BRIDGE IS DEAD!"
    log "  supervisorctl status: $(supervisorctl status vsock-bridge-7844 2>&1)"
    log "  Attempting restart..."
    supervisorctl restart vsock-bridge-7844 >> "$LOG" 2>&1 || true
    sleep 1
    if ss -tlnp 2>/dev/null | grep -q ':7844'; then
        log "  Port 7844: NOW LISTENING after restart"
    else
        log "  Port 7844: STILL NOT LISTENING after restart - FATAL"
    fi
fi

# ── Check DoT bridge for 853 ──
log "--- DoT bridge 853 health ---"
if ss -tlnp 2>/dev/null | grep -q ':853'; then
    log "  Port 853: LISTENING (DoT bridge alive)"
else
    log "  Port 853: NOT LISTENING - DoT BRIDGE IS DEAD!"
    log "  supervisorctl status: $(supervisorctl status vsock-bridge-dot 2>&1)"
    log "  Attempting restart..."
    supervisorctl restart vsock-bridge-dot >> "$LOG" 2>&1 || true
    sleep 1
fi

# ── TCP connectivity test through bridge ──
log "--- TCP connectivity test to 127.0.0.1:7844 ---"
CONN_START=$(date +%s%N)
if timeout 10 bash -c "echo PING | socat -T5 - TCP:127.0.0.1:7844,connect-timeout=5" >> "$LOG" 2>&1; then
    CONN_END=$(date +%s%N)
    CONN_MS=$(( (CONN_END - CONN_START) / 1000000 ))
    log "  TCP connect succeeded in ${CONN_MS}ms"
else
    RC=$?
    log "  TCP connect FAILED (rc=$RC) - bridge chain may be broken"
    log "  Checking VSOCK device..."
    ls -la /dev/vsock >> "$LOG" 2>&1 || log "  /dev/vsock not found!"
fi

# ── Test DoT connectivity ──
log "--- DoT connectivity test to 1.1.1.1:853 ---"
if timeout 5 bash -c "echo | socat -T3 - TCP:1.1.1.1:853,connect-timeout=3" >> "$LOG" 2>&1; then
    log "  DoT bridge connect succeeded"
else
    log "  DoT bridge connect FAILED (rc=$?)"
fi

# ── Process tree ──
log "--- Process tree (socat and cloudflared) ---"
pgrep -a 'socat|cloudflared|supervisor' >> "$LOG" || true

# ── Environment ──
log "--- Environment (relevant vars) ---"
log "  CLOUDFLARE_TUNNEL_TOKEN: ${CLOUDFLARE_TUNNEL_TOKEN:+set (${#CLOUDFLARE_TUNNEL_TOKEN} chars)}"
log "  HTTP_PROXY: '${HTTP_PROXY:-unset}'"
log "  HTTPS_PROXY: '${HTTPS_PROXY:-unset}'"
log "  http_proxy: '${http_proxy:-unset}'"
log "  https_proxy: '${https_proxy:-unset}'"
log "  NO_PROXY: '${NO_PROXY:-unset}'"

# ── Clear ALL proxy env vars completely ──
# Cloudflared checks these and empty strings can cause Go's HTTP client to misconfigure
unset HTTP_PROXY HTTPS_PROXY http_proxy https_proxy NO_PROXY no_proxy
log "All proxy vars unset"

# ── Verify token exists ──
if [ -z "${CLOUDFLARE_TUNNEL_TOKEN:-}" ]; then
    log "FATAL: CLOUDFLARE_TUNNEL_TOKEN is empty!"
    echo "FATAL: No tunnel token" >&2
    exit 1
fi

log "=== Starting cloudflared ==="
log "Command: cloudflared tunnel --protocol http2 --no-autoupdate --edge-ip-version 4 --retries 10 --grace-period 60s --loglevel debug run --token <redacted>"
log "================================================================"

# Exec cloudflared with:
#   --protocol http2: required (QUIC/UDP can't traverse TCP/VSOCK bridge)
#   --edge-ip-version 4: force IPv4 (no IPv6 in enclave)
#   --retries 10: retry edge connections more aggressively
#   --grace-period 60s: longer grace for in-flight requests on reconnect
#   --loglevel debug: maximum verbosity for debugging
exec /usr/local/bin/cloudflared tunnel \
    --protocol http2 \
    --no-autoupdate \
    --edge-ip-version 4 \
    --retries 10 \
    --grace-period 60s \
    --loglevel debug \
    run --token "${CLOUDFLARE_TUNNEL_TOKEN}"
