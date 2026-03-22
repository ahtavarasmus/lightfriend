#!/bin/bash
set -euo pipefail

if [ $# -lt 1 ]; then
    echo "Usage: $0 <domain-or-url> [--rpc-url <url>] [--build-metadata-url <url>]"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cargo run --manifest-path "${REPO_ROOT}/tools/attestation-verifier/Cargo.toml" -- "$@"
