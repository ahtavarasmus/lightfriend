---
name: commit-push-pr
description: Quickly commit, push, and create a PR while leaving validation to GitHub checks
---

# Quick Commit, Push, and PR

Commit and push local changes quickly, then create a PR if one does not exist. Do not run local formatting, lint, or test commands by default; GitHub Actions runs validation after push.

## Process

### Step 1: Check for Changes

```bash
git status
```

Review what will be committed. If there are no changes to commit, skip to Step 5 (PR check).

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

### Step 4: Push to Remote

```bash
git push origin HEAD
```

If push fails due to upstream changes:
```bash
git pull --rebase origin HEAD && git push origin HEAD
```

### Step 5: Check for Existing PR

```bash
gh pr list --head $(git branch --show-current) --state open
```

If a PR already exists for this branch, inform the user and provide the PR URL. Done.

### Step 6: Create PR (if none exists)

Get the current branch and base branch:
```bash
CURRENT_BRANCH=$(git branch --show-current)
```

Create the PR:
```bash
gh pr create --title "PR title here" --body "$(cat <<'EOF'
## Summary
- Brief description of changes

## Test plan
- GitHub Actions will run validation after push

EOF
)"
```

**PR Title**: Generate from the commit message or ask user.

**PR Body**: Include:
- Summary of changes (2-3 bullet points)
- Test plan noting that GitHub Actions will run validation

Do NOT include "Generated with Codex" or similar.

### Step 7: Report Success

Provide the PR URL to the user.

## Failure Handling

If staging, commit, push, or PR creation fails:
1. Stop immediately
2. Report the failure to the user
3. Do not continue to later git steps
4. Help fix the issues if requested

If GitHub checks fail after push, inspect and fix them in a follow-up workflow.

## Quick Reference

```bash
# Quick commit + push
git status
git add -A
git commit -m "message"
git push origin HEAD

# Check for existing PR
gh pr list --head $(git branch --show-current) --state open

# Create PR
gh pr create --title "Title" --body "Body"
```
