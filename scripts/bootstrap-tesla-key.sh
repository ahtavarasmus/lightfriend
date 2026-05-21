#!/usr/bin/env bash
#
# Bootstrap (or rotate) the Tesla vehicle-command signing key in S3.
#
# Background: the enclave fetches `s3://$BUCKET/config/tesla_private_key.pem`
# at boot from the host seed server. Before the tesla_keys.rs derive-public
# fix landed, the backend would regenerate keys on every boot, silently
# unpairing every previously-paired vehicle. This script gives you control
# over the canonical key.
#
# Run this once after deploying the tesla_keys.rs fix. After the next deploy,
# re-pair vehicles in the Tesla app (Add Virtual Key, point at lightfriend.ai).
# All future deploys will preserve the pairing.
#
# Re-running this script rotates the key — every paired vehicle must re-pair.
#
# Requirements:
#   - awscli configured with credentials that can read SSM `/lightfriend/s3-bucket`
#     and read/write `s3://$BUCKET/config/tesla_private_key.pem`
#   - openssl
#
set -euo pipefail

# 1. Resolve bucket name from SSM (same source of truth as user_data.sh).
BUCKET=$(aws ssm get-parameter \
    --name /lightfriend/s3-bucket \
    --query Parameter.Value \
    --output text 2>/dev/null) || {
    echo "ERROR: could not resolve /lightfriend/s3-bucket SSM parameter."
    echo "       Are AWS creds configured for the right account/region?"
    exit 1
}
echo "S3 bucket: $BUCKET"

S3_KEY_URI="s3://$BUCKET/config/tesla_private_key.pem"

# 2. If a key already exists in S3, back it up locally before clobbering.
BACKUP_DIR="$HOME/.lightfriend-tesla-key-backups"
mkdir -p "$BACKUP_DIR"
TS=$(date -u +%Y%m%dT%H%M%SZ)

if aws s3 ls "$S3_KEY_URI" >/dev/null 2>&1; then
    BACKUP_FILE="$BACKUP_DIR/tesla_private_key.pem.$TS.pre-rotate"
    aws s3 cp "$S3_KEY_URI" "$BACKUP_FILE"
    chmod 600 "$BACKUP_FILE"
    echo "Existing S3 key backed up locally: $BACKUP_FILE"
    echo ""
    echo "WARNING: rotating the Tesla signing key. Any currently-paired vehicles"
    echo "         will need to re-pair after the next deploy."
    read -r -p "Proceed and overwrite the S3 key? [y/N] " confirm
    if [[ "${confirm:-N}" != "y" && "${confirm:-N}" != "Y" ]]; then
        echo "Aborted. S3 key unchanged."
        exit 1
    fi
else
    echo "No existing key in S3 — fresh bootstrap."
fi

# 3. Generate a fresh P-256 key in PKCS#8 PEM (what tesla_keys.rs expects).
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

NEW_KEY="$TMPDIR/tesla_private_key.pem"
openssl genpkey \
    -algorithm EC \
    -pkeyopt ec_paramgen_curve:P-256 \
    -out "$NEW_KEY"
chmod 600 "$NEW_KEY"

# Verify it parses as a PKCS#8 EC private key.
if ! openssl pkey -in "$NEW_KEY" -text -noout >/dev/null 2>&1; then
    echo "ERROR: generated key did not parse — refusing to upload."
    exit 1
fi
HEAD_LINE=$(head -1 "$NEW_KEY")
if [[ "$HEAD_LINE" != "-----BEGIN PRIVATE KEY-----" ]]; then
    echo "ERROR: generated key is not in PKCS#8 PEM format (got: '$HEAD_LINE')."
    echo "       Backend expects '-----BEGIN PRIVATE KEY-----'."
    exit 1
fi

# Save a local copy too so you can audit / re-upload without regenerating.
LOCAL_COPY="$BACKUP_DIR/tesla_private_key.pem.$TS.current"
cp "$NEW_KEY" "$LOCAL_COPY"
chmod 600 "$LOCAL_COPY"
echo "Local copy saved: $LOCAL_COPY"

# 4. Upload.
aws s3 cp "$NEW_KEY" "$S3_KEY_URI"
echo "Uploaded: $S3_KEY_URI"

# 5. Print the matching public key so you can sanity-check what the enclave
#    will serve at /.well-known/appspecific/com.tesla.3p.public-key.pem
#    after the next deploy.
PUB_LOCAL="$TMPDIR/tesla_public_key.pem"
openssl pkey -in "$NEW_KEY" -pubout -out "$PUB_LOCAL"
echo ""
echo "Matching public key (this is what Tesla will see after the next deploy):"
echo "---"
cat "$PUB_LOCAL"
echo "---"

echo ""
echo "Next steps:"
echo "  1. Merge the tesla_keys.rs fix to master and let CI deploy."
echo "  2. After the new enclave boots, curl https://lightfriend.ai/.well-known/appspecific/com.tesla.3p.public-key.pem"
echo "     and verify the public key shown matches the one printed above."
echo "  3. In the Tesla app: Add Virtual Key, target lightfriend.ai, re-pair the vehicle."
echo "  4. Test SMS unlock. Subsequent deploys will keep this key. Done."
