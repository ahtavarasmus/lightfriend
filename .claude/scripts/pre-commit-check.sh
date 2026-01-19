#!/bin/bash
# Pre-commit check script for Claude Code hooks
# Runs cargo fmt --check before allowing commits

# Get project directory from env or derive from script location
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

cd "$PROJECT_ROOT/backend"

# Check formatting
if ! cargo fmt --check 2>/dev/null; then
    echo "BLOCKED: Rust formatting issues found!"
    echo "Run 'cargo fmt' in the backend directory first, then try committing again."
    exit 1
fi

# Check clippy
if ! cargo clippy --quiet -- -D warnings 2>/dev/null; then
    echo "BLOCKED: Clippy found issues!"
    echo "Fix the clippy warnings first, then try committing again."
    exit 1
fi

exit 0
