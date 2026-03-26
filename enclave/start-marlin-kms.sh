#!/bin/bash
set -euo pipefail

RUN_DIR="/tmp/marlin-kms"
ATTEST_PORT="1300"
LOCAL_ROOT_TUNNEL_PORT="1102"
LOCAL_DERIVE_PORT="1101"

mkdir -p "${RUN_DIR}"

start_bg() {
    local pid_file="$1"
    shift

    if [ -f "${pid_file}" ]; then
        local existing_pid
        existing_pid="$(cat "${pid_file}" 2>/dev/null || true)"
        if [ -n "${existing_pid}" ] && kill -0 "${existing_pid}" 2>/dev/null; then
            return 0
        fi
    fi

    "$@" >"${RUN_DIR}/$(basename "${pid_file}").log" 2>&1 &
    echo $! > "${pid_file}"
}

wait_http() {
    local url="$1"
    local attempts="${2:-30}"
    local delay="${3:-1}"

    for _ in $(seq 1 "${attempts}"); do
        if curl -fsS "${url}" >/dev/null 2>&1; then
            return 0
        fi
        sleep "${delay}"
    done

    return 1
}

if [ ! -f "${RUN_DIR}/id.sec" ] || [ ! -f "${RUN_DIR}/id.pub" ]; then
    /usr/local/bin/keygen-x25519 --secret "${RUN_DIR}/id.sec" --public "${RUN_DIR}/id.pub"
    chmod 600 "${RUN_DIR}/id.sec" "${RUN_DIR}/id.pub"
fi

printf '%s\n' "${MARLIN_KMS_CONTRACT_ADDRESS}" > "${RUN_DIR}/contract-address"
chmod 600 "${RUN_DIR}/contract-address"

start_bg "${RUN_DIR}/kms-tunnel.pid" \
    /usr/bin/socat TCP-LISTEN:${LOCAL_ROOT_TUNNEL_PORT},reuseaddr,fork VSOCK-CONNECT:3:9010

start_bg "${RUN_DIR}/attestation.pid" \
    /usr/local/bin/oyster-attestation-server \
    --ip-addr "127.0.0.1:${ATTEST_PORT}"

sleep 1
ATTEST_PID=$(cat "${RUN_DIR}/attestation.pid" 2>/dev/null || echo "")
if [ -n "${ATTEST_PID}" ] && kill -0 "${ATTEST_PID}" 2>/dev/null; then
    echo "Attestation server running (PID ${ATTEST_PID})" >&2
else
    echo "Attestation server died immediately (PID ${ATTEST_PID})" >&2
    echo "Log:" >&2
    cat "${RUN_DIR}/attestation.pid.log" 2>/dev/null >&2
    exit 1
fi

# Check what's listening
echo "Checking port ${ATTEST_PORT}..." >&2
ss -tlnp 2>/dev/null | grep "${ATTEST_PORT}" >&2 || echo "Nothing listening on ${ATTEST_PORT}" >&2

if ! wait_http "http://127.0.0.1:${ATTEST_PORT}/attestation/raw" 30 1; then
    echo "Marlin attestation server did not become ready" >&2
    echo "Attestation PID alive: $(kill -0 "${ATTEST_PID}" 2>/dev/null && echo yes || echo no)" >&2
    echo "Last attestation log:" >&2
    tail -20 "${RUN_DIR}/attestation.pid.log" 2>/dev/null >&2
    exit 1
fi

start_bg "${RUN_DIR}/derive.pid" \
    /usr/local/bin/kms-derive-server \
    --kms-endpoint "127.0.0.1:${LOCAL_ROOT_TUNNEL_PORT}" \
    --kms-pubkey "${MARLIN_ROOT_SERVER_X25519_PUBKEY}" \
    --listen-addr "127.0.0.1:${LOCAL_DERIVE_PORT}" \
    --attestation-endpoint "http://127.0.0.1:${ATTEST_PORT}/attestation/raw" \
    --secret-path "${RUN_DIR}/id.sec" \
    --contract-address-file "${RUN_DIR}/contract-address"

if ! wait_http "http://127.0.0.1:${LOCAL_DERIVE_PORT}/derive/x25519?path=${MARLIN_BACKUP_KEY_PATH}" 30 1; then
    echo "Marlin derive server did not become ready or key derivation failed" >&2
    exit 1
fi
