#!/bin/bash
# Wrapper for cloudflared that logs diagnostics before and during tunnel operation.
# This helps debug edge connectivity issues through the VSOCK bridge.

LOG="/var/log/supervisor/cloudflared-diag.log"
echo "=== cloudflared-wrapper started at $(date -u) ===" >> "$LOG"

# Log version
echo "Version: $(/usr/local/bin/cloudflared --version 2>&1)" >> "$LOG"

# Log network state
echo "Port 7844 listener: $(ss -tlnp 2>/dev/null | grep 7844)" >> "$LOG"
echo "/etc/hosts edge entries: $(grep argotunnel /etc/hosts 2>/dev/null)" >> "$LOG"
echo "DNS test (region1): $(getent hosts region1.v2.argotunnel.com 2>&1 | head -1)" >> "$LOG"

# Test TCP connectivity to port 7844 (should reach socat bridge -> VSOCK -> host -> edge)
echo "TCP test to 127.0.0.1:7844..." >> "$LOG"
timeout 5 bash -c "echo | socat - TCP:127.0.0.1:7844" >> "$LOG" 2>&1
echo "TCP test rc=$?" >> "$LOG"

# Clear ALL proxy env vars completely (empty string != unset)
# Cloudflared checks these and empty strings can cause connection issues
unset HTTP_PROXY HTTPS_PROXY http_proxy https_proxy NO_PROXY no_proxy
echo "Proxy vars unset (were: HTTP_PROXY=${HTTP_PROXY+set}, HTTPS_PROXY=${HTTPS_PROXY+set})" >> "$LOG"

# Log the command we're about to run
echo "Running: cloudflared tunnel --protocol http2 --no-autoupdate run --token <redacted>" >> "$LOG"
echo "=== Starting cloudflared ===" >> "$LOG"

# Exec cloudflared - this replaces the wrapper process
exec /usr/local/bin/cloudflared tunnel \
    --protocol http2 \
    --no-autoupdate \
    --loglevel debug \
    run --token "${CLOUDFLARE_TUNNEL_TOKEN}"
