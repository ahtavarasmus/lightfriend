#!/bin/bash

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
STORAGE_HEALTH="$REPO_ROOT/enclave/storage-health.sh"
TEST_ROOT="$(mktemp -d)"
trap 'rm -rf "$TEST_ROOT"' EXIT

LOG_DIR="$TEST_ROOT/logs"
DATA_DIR="$TEST_ROOT/data"
mkdir -p "$LOG_DIR" "$DATA_DIR"

cat > "$LOG_DIR/tuwunel.log.1" <<'EOF'
| lev  | sst  | keys | dels | size | column |
| ---: | :--- | ---: | ---: | ---: | :---   |
| 6 | 000001.sst    |    999+ |   99- |      9999 | obsolete_column |
startup complete
EOF

cat > "$LOG_DIR/tuwunel.log" <<'EOF'
2026-07-26T08:00:00Z INFO Execute command completed:
2026-07-26T08:00:00Z INFO | lev  | sst  | keys | dels | size | column |
2026-07-26T08:00:00Z INFO | ---: | :--- | ---: | ---: | ---: | :---   |
2026-07-26T08:00:00Z INFO | 6 | 000100.sst    |    100+ |   10- |      1000 | pduid_pdu |
2026-07-26T08:00:00Z INFO | 0 | 000101.sst    |     50+ |    5- |       500 | pduid_pdu |
2026-07-26T08:00:00Z INFO | 2 | 000102.sst    |     20+ |    0- |       200 | stateid_shorteventid |
2026-07-26T08:00:00Z INFO command complete
EOF

truncate -s 1000 "$DATA_DIR/000100.sst"
truncate -s 500 "$DATA_DIR/000101.sst"
truncate -s 200 "$DATA_DIR/000102.sst"

OUTPUT="$(
    TUWUNEL_LOG_DIR="$LOG_DIR" \
    TUWUNEL_DATA_DIR="$DATA_DIR" \
    bash "$STORAGE_HEALTH" rocksdb-columns
)"

grep -Fq "status=available actual_sst_bytes=1700 mapped_sst_bytes=1700 unmapped_sst_bytes=0 coverage_pct=100.000" <<< "$OUTPUT"
grep -Fq $'1500\t150\t15\t135\t2\t1\t6\tpduid_pdu' <<< "$OUTPUT"
grep -Fq $'200\t20\t0\t20\t1\t0\t2\tstateid_shorteventid' <<< "$OUTPUT"

if grep -Fq "obsolete_column" <<< "$OUTPUT"; then
    echo "parser selected an older database-files report" >&2
    exit 1
fi

echo "Tuwunel RocksDB diagnostics parser test passed"
