#!/bin/bash
# Auto-lint script for Claude Code hooks
# Runs cargo fmt when Rust files are edited

# Read hook input from stdin
INPUT=$(cat)

# Extract file path from JSON input
FILE_PATH=$(echo "$INPUT" | grep -o '"file_path"[[:space:]]*:[[:space:]]*"[^"]*"' | sed 's/.*: *"//' | sed 's/"$//')

# Only run on .rs files
if [[ ! "$FILE_PATH" == *.rs ]]; then
    exit 0
fi

# Get project directory from env or derive from file path
if [[ -n "$CLAUDE_PROJECT_DIR" ]]; then
    PROJECT_ROOT="$CLAUDE_PROJECT_DIR"
else
    # Fallback: derive from script location
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
fi

# Only run if backend exists
if [[ ! -d "$PROJECT_ROOT/backend" ]]; then
    exit 0
fi

# Run cargo fmt on the backend
cd "$PROJECT_ROOT/backend"
cargo fmt 2>/dev/null

# Always exit 0 - we don't want to block Claude's workflow
exit 0
