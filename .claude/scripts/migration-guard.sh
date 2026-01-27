#!/bin/bash
# Migration Guard: Blocks destructive SQL patterns in migration files
# Override: Create .claude/.allow-destructive-migration file (auto-deleted after use)

set -euo pipefail

# Only check migration files
if [[ ! "$TOOL_INPUT_FILE_PATH" =~ migrations/.*\.(sql|up\.sql|down\.sql)$ ]]; then
    exit 0
fi

FLAG_FILE="$CLAUDE_PROJECT_DIR/.claude/.allow-destructive-migration"

# If override flag exists, allow and remove it (one-time use)
if [[ -f "$FLAG_FILE" ]]; then
    rm -f "$FLAG_FILE"
    exit 0
fi

# Also check env var for backwards compatibility
if [[ "${ALLOW_DESTRUCTIVE_MIGRATION:-}" == "1" ]]; then
    exit 0
fi

# Get the content being written
CONTENT="$TOOL_INPUT_CONTENT"

# Destructive patterns to block (case-insensitive)
DESTRUCTIVE_PATTERNS=(
    "DROP[[:space:]]+COLUMN"
    "DROP[[:space:]]+TABLE"
    "DROP[[:space:]]+INDEX"
    "DROP[[:space:]]+CONSTRAINT"
    "ALTER[[:space:]]+.*[[:space:]]+TYPE[[:space:]]"
    "TRUNCATE[[:space:]]"
    "DELETE[[:space:]]+FROM"
    "RENAME[[:space:]]+COLUMN"
    "RENAME[[:space:]]+TABLE"
    "RENAME[[:space:]]+TO"
)

# Check for destructive patterns
for pattern in "${DESTRUCTIVE_PATTERNS[@]}"; do
    if echo "$CONTENT" | grep -iE "$pattern" > /dev/null 2>&1; then
        echo "BLOCKED: Migration contains destructive SQL pattern: $pattern"
        echo ""
        echo "File: $TOOL_INPUT_FILE_PATH"
        echo ""
        echo "Matched content:"
        echo "$CONTENT" | grep -iE "$pattern" | head -5
        echo ""
        echo "This is a destructive database change that requires human approval."
        echo ""
        echo "OVERRIDE: touch $FLAG_FILE"
        exit 1
    fi
done

# Safe patterns - no blocking needed
exit 0
