---
name: commit
description: Quickly commit and push local changes while leaving validation to GitHub checks
---

# Quick Commit and Push

Commit and push local changes quickly. Do not run local formatting, lint, or test commands by default; GitHub Actions runs validation after push.

## Process

### Step 1: Check for Changes

```bash
git status
```

Review what will be committed. If there are no changes, inform the user and stop.

Use `git diff --stat` or `git diff --cached --stat` when a quick scope check is useful. Do not run full validation unless the user explicitly asks for it.

### Step 2: Stage Changes

```bash
git add -A
```

### Step 3: Create Commit

Ask the user for a commit message, or generate one based on the changes.

**IMPORTANT**: Never include:
- "Generated with Codex"
- "Co-Authored-By: Codex"
- Any AI/Codex attribution

Commit message format:
```bash
git commit -m "Short description of changes"
```

For multi-line messages:
```bash
git commit -m "$(cat <<'EOF'
Short summary

Longer description if needed
EOF
)"
```

### Step 4: Push to Remote

```bash
git push origin HEAD
```

If push fails due to upstream changes, pull first:
```bash
git pull --rebase origin HEAD && git push origin HEAD
```

## Failure Handling

If staging, commit, or push fails:
1. Stop immediately
2. Report the failure to the user
3. Do not continue to later git steps
4. Help fix the issues if requested

If GitHub checks fail after push, inspect and fix them in a follow-up workflow.

## Quick Commands Reference

```bash
# Quick commit + push
git status
git add -A
git commit -m "message"
git push origin HEAD
```
