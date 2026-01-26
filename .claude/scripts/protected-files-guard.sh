#!/bin/bash
# Protected Files Guard: Blocks edits to critical security/payment files
# Override: Create .claude/.allow-protected-edit file (auto-deleted after use)

set -euo pipefail

FLAG_FILE="$CLAUDE_PROJECT_DIR/.claude/.allow-protected-edit"

# If override flag exists, allow and remove it (one-time use)
if [[ -f "$FLAG_FILE" ]]; then
    rm -f "$FLAG_FILE"
    exit 0
fi

# Also check env var for backwards compatibility
if [[ "${ALLOW_PROTECTED_EDIT:-}" == "1" ]]; then
    exit 0
fi

# Protected file patterns (relative to project root)
PROTECTED_FILES=(
    "backend/src/utils/encryption.rs"
    "backend/src/handlers/auth_middleware.rs"
    "backend/src/handlers/stripe_webhooks.rs"
)

# Get the file being edited
FILE_PATH="$TOOL_INPUT_FILE_PATH"

# Check if file matches any protected pattern
for protected in "${PROTECTED_FILES[@]}"; do
    if [[ "$FILE_PATH" == *"$protected" ]]; then
        echo "BLOCKED: Cannot edit protected file: $protected"
        echo ""
        echo "This file contains critical security or payment logic."
        echo "Human approval is required before modifications."
        echo ""
        echo "OVERRIDE: touch $FLAG_FILE"
        exit 1
    fi
done

# Not a protected file
exit 0
