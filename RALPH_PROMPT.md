# Ralph Wiggum Loop Prompt - Tier 3 Removal

You are running in a Ralph Wiggum loop to remove all tier 3 (self-hosted) code from this codebase.

## CRITICAL: What to KEEP

- **KEEP Textbee** - NOT tier 3 related
- **KEEP /api/sms/server endpoints** - these are for BYOT (Bring Your Own Twilio) users
- **KEEP BYOT functionality** - users providing their own Twilio credentials is NOT tier 3
- **KEEP subaccount management if used by BYOT**

## What IS Tier 3 (remove this)

- **Magic login** - temporary tokens for self-hosted backend setup
- **server_ip validation** - IP address validation for self-hosted users
- **Self-hosted backend** - users running their own instance of the backend
- **Tier 3 subscription option** - the tier level itself

## Your Instructions

1. Read `RALPH_PRD.json` to see the task list
2. Read `RALPH_PROGRESS.txt` to see what has been done
3. Pick the first task where `passes: false`
4. Complete that task thoroughly
5. Update `RALPH_PRD.json` to set `passes: true` for the completed task
6. Append your progress to `RALPH_PROGRESS.txt` with timestamp and details
7. Commit your changes with a descriptive message (no AI attribution)
8. If all tasks pass, write "ALL TASKS COMPLETE" to RALPH_PROGRESS.txt and stop

## Important Rules

- Do NOT remove database migrations - document fields for later cleanup
- Keep tier 1, tier 1.5, and tier 2 functionality working
- Keep code compiling after each task (run cargo check)
- Commit after each completed task

## Search Patterns (Tier 3 only)

- `tier 3`, `tier3`, `tier_3` (when referring to self-hosted)
- `self-hosted`, `self_hosted`, `selfhosted`
- `magic_login`, `magic-login`, `magiclogin`
- `server_ip` (IP validation for self-hosted)

## DO NOT search for / remove

- `textbee` - keep this
- `/api/sms/server` - keep this (BYOT)
- `subaccount` - check if BYOT uses it before removing

Start now. Read the PRD and progress file, then work on the next incomplete task.
