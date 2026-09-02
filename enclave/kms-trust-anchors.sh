#!/bin/bash

load_measured_kms_trust_anchors() {
    local anchor_file="$1"
    local endpoint=""
    local public_key=""
    local contract_address=""
    local key_path=""
    local name value

    if [ ! -r "$anchor_file" ]; then
        echo "FATAL: measured KMS trust anchors are unavailable" >&2
        return 1
    fi

    while IFS='=' read -r name value || [ -n "$name" ]; do
        value="${value%$'\r'}"
        case "$name" in
            ""|'#'*) continue ;;
            MARLIN_ROOT_SERVER_ENDPOINT)
                [ -z "$endpoint" ] || { echo "FATAL: duplicate KMS endpoint anchor" >&2; return 1; }
                endpoint="$value"
                ;;
            MARLIN_ROOT_SERVER_X25519_PUBKEY)
                [ -z "$public_key" ] || { echo "FATAL: duplicate KMS public key anchor" >&2; return 1; }
                public_key="$value"
                ;;
            MARLIN_KMS_CONTRACT_ADDRESS)
                [ -z "$contract_address" ] || { echo "FATAL: duplicate KMS contract anchor" >&2; return 1; }
                contract_address="$value"
                ;;
            MARLIN_BACKUP_KEY_PATH)
                [ -z "$key_path" ] || { echo "FATAL: duplicate KMS key path anchor" >&2; return 1; }
                key_path="$value"
                ;;
            *)
                echo "FATAL: unknown entry in measured KMS trust anchors" >&2
                return 1
                ;;
        esac
    done < "$anchor_file"

    if [[ ! "$endpoint" =~ ^[A-Za-z0-9.-]+:[0-9]{1,5}$ ]]; then
        echo "FATAL: invalid measured KMS endpoint" >&2
        return 1
    fi
    local endpoint_port="${endpoint##*:}"
    if (( endpoint_port < 1 || endpoint_port > 65535 )); then
        echo "FATAL: invalid measured KMS endpoint port" >&2
        return 1
    fi
    if [[ ! "$public_key" =~ ^[0-9a-fA-F]{64}$ ]]; then
        echo "FATAL: invalid measured KMS public key" >&2
        return 1
    fi
    if [[ ! "$contract_address" =~ ^0x[0-9a-fA-F]{40}$ ]]; then
        echo "FATAL: invalid measured KMS contract address" >&2
        return 1
    fi
    if [[ ! "$key_path" =~ ^[A-Za-z0-9._-]+$ ]]; then
        echo "FATAL: invalid measured KMS key path" >&2
        return 1
    fi

    export MARLIN_ROOT_SERVER_ENDPOINT="$endpoint"
    export MARLIN_ROOT_SERVER_X25519_PUBKEY="$public_key"
    export MARLIN_KMS_CONTRACT_ADDRESS="$contract_address"
    export MARLIN_BACKUP_KEY_PATH="$key_path"
    export KMS_ANCHOR_SOURCE="measured-eif"
    readonly MARLIN_ROOT_SERVER_ENDPOINT
    readonly MARLIN_ROOT_SERVER_X25519_PUBKEY
    readonly MARLIN_KMS_CONTRACT_ADDRESS
    readonly MARLIN_BACKUP_KEY_PATH
    readonly KMS_ANCHOR_SOURCE
}
