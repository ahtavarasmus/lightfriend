# Unified Item Model - Architecture Plan

## Problem

Two separate tables (`triage_items`, `tasks`) handling one concept: things the assistant tracks for the user. Merging into one `items` table - the agent's working memory.

## Design Principles

1. AI models are getting smarter - give the AI the right context and let it decide
2. Natural language fields for LLM context, deterministic columns only for things that must be machine-parseable
3. Only fields that are fixed throughout the item's lifetime OR must be queried in SQL get their own column
4. Everything else lives in `summary` as natural language
5. Security boundaries are deterministic (no LLM in trust decisions)

## Schema (11 columns)

```sql
CREATE TABLE items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    summary TEXT NOT NULL,
    condition TEXT,
    due_at INTEGER,
    next_check_at INTEGER,
    priority INTEGER NOT NULL DEFAULT 0,
    notification_type TEXT,
    source_id TEXT,
    last_sender TEXT,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE INDEX idx_items_user ON items(user_id);
CREATE INDEX idx_items_next_check ON items(next_check_at);
CREATE INDEX idx_items_source ON items(user_id, source_id);
```

### Column rationale

| Column | Why it's a column | Fixed? |
|--------|-------------------|--------|
| `id` | Identity | Yes |
| `user_id` | Identity, every query filters by this | Yes |
| `summary` | Living natural language doc: what it is, context updates, action instructions, recurrence info. LLM reads this to decide what to do. | Evolves |
| `condition` | Injection-safe filter for matching incoming emails/messages. Separate from summary. Compared against incoming data, never exposed to it. | Fixed |
| `due_at` | The actual deadline. Used for display ("due in 3 days"). NULL if no deadline. | Fixed-ish |
| `next_check_at` | When scheduler should next look at this item. LLM sets this. Pure timestamp comparison - no LLM in scheduler hot path. | Evolves |
| `priority` | ORDER BY for dashboard. LLM can escalate. | Evolves |
| `notification_type` | "sms", "call", or NULL (dashboard-only). Deterministic routing - code needs exact string to call Twilio vs voice API. AI can upgrade channel as urgency increases. | Evolves |
| `source_id` | Dedup key (email_uid, etc.) | Fixed |
| `last_sender` | Email address or room_id of whoever last created/matched this item. Deterministic trust gate at execution time - checked against contacts, no LLM. NULL = user-created (trusted). | Evolves |
| `created_at` | Ordering, cleanup | Fixed |

### No status field

Items exist = active. Delete when done. No "done" state, no history.

### Summary field is the key

The `summary` is the AI's "notes file" about this item. It contains everything in natural language:
- What the item is ("AWS invoice $45")
- Context and updates ("due Feb 20, second notice received Feb 16, payment sent Feb 17")
- Action instructions for scheduler ("notify user via SMS", "generate digest from email and whatsapp")
- Recurrence info ("repeats daily at 08:00 user timezone")
- Source description ("detected from email to user@example.com")
- Escalation logic ("if still unhandled after 2 reminders, upgrade to call")

When the scheduler fires (next_check_at <= now), it calls the LLM with the summary. The LLM decides what to do. The LLM **replaces** the summary with an updated version (keeps it concise, doesn't just append endlessly).

### next_check_at: the scheduling field

Different items need attention at different times relative to their due date:
- Unpaid invoice due Feb 20: notify 3 days before (next_check_at = Feb 17)
- Simple reminder at 5pm: notify exactly at time (next_check_at = 5pm)
- Recurring digest at 8am: fires daily (next_check_at = tomorrow 8am)

The scheduler just does `WHERE next_check_at IS NOT NULL AND next_check_at <= now` every minute. No LLM needed in the hot path. When an item triggers, the LLM processes it and sets the next next_check_at (or NULL/delete).

### Condition field prevents injection

The `condition` field is for recurring monitors: "email from IRS about tax return". It's matched against incoming data in a separate LLM call: "does this email match this condition?" Keeping it separate from summary means a crafted email can't modify the matching logic or action instructions.

---

## Security Model

### `last_sender` = deterministic trust gate (no LLM in the loop)

**When `last_sender` gets set:**
1. Item created from email -> `last_sender = "sender@example.com"`
2. Item created from message -> `last_sender = "!roomid:matrix.org"`
3. Condition match from incoming data -> `last_sender` overwritten with new matching sender
4. User creates item directly via SMS/chat -> `last_sender = NULL` (trusted by default)

**When `last_sender` gets checked:**
- Scheduler triggers on `next_check_at` -> before executing destructive outgoing actions
- `last_sender` checked against contacts table (deterministic DB lookup, no LLM)
- Known contact -> full capabilities (send_message to others, reply to emails, etc.)
- Not known / no contact -> restricted: block destructive outgoing actions only
- `last_sender = NULL` -> trusted (user-created item)
- **Notifying the user is ALWAYS allowed** regardless of trust - it's not destructive

**Why this stops prompt injection:**
```
Attacker sends crafted email matching condition
  -> condition match overwrites last_sender with attacker's address
  -> attacker's address not in contacts
  -> destructive actions blocked at execution time
  -> even if summary was poisoned with malicious instructions, they can't execute
```

**Condition field isolation:**
- Fixed at creation, never modified by incoming data
- Incoming emails/messages compared AGAINST it, never injected INTO it
- If condition matches: summary may be updated with email context (attack vector)
  BUT last_sender trust check blocks restricted actions regardless

Uses existing `is_restricted()` pattern from `ToolHandler` trait in `tools/registry.rs`.

---

## Three AI Interaction Points

Each needs a carefully crafted system prompt.

### 1. Item creation (user creates via SMS/chat)
- Extract: what, when due, how to notify, any condition to monitor
- If info is missing, ASK user before creating ("How should I notify you?")
- Summary must be **self-contained** - a different AI instance reads it later with zero conversation context
- Summary should include: what the item is, action instructions, notification phrasing, escalation logic

### 2. Condition evaluation (email fetch job)
- Input: condition string + incoming email (from, subject, body snippet)
- Output: match yes/no
- Be conservative - false positives are worse than false misses
- Never expose summary/action instructions to this prompt

### 3. Triggered item processing (scheduler fires on next_check_at)
- Input: summary + due_at + priority + notification_type + current_time
- Output: structured actions (send_notification, update_summary, set_next_check_at, set_priority, set_notification_type, delete)
- AI reads natural language summary but outputs deterministic action structs
- Escalation: AI can bump priority AND upgrade notification_type (sms -> call)
- Before execution: last_sender trust check (deterministic, no LLM)

---

## Lifecycle Examples

### Example 1: Email Invoice Detection

**Trigger:** Email scanning detects "AWS invoice $45 due Feb 20"

**Step 1: Item creation** (proactive/utils.rs)
```
INSERT: {
  summary: "Invoice from AWS for $45. Due Feb 20. Detected from email on Feb 14.",
  due_at: Feb 20, next_check_at: Feb 17 (3 days before),
  priority: 0, notification_type: "sms",
  source_id: "email_abc123", last_sender: "billing@aws.com"
}
```
User sees on dashboard at priority 0. Just informational.

**Step 2: Scheduler fires Feb 17** (next_check_at <= now)
- LLM reads summary, sees "due in 3 days", sends SMS to user (always allowed)
- Replaces summary: "Invoice from AWS for $45. Due Feb 20. Reminded user Feb 17."
- Sets next_check_at = Feb 19, priority = 1

**Step 3: Scheduler fires Feb 19**
- LLM sends "AWS invoice $45 due tomorrow"
- Updates: next_check_at = Feb 20, priority = 2

**Step 4a: User says "I paid it"** -> item deleted

**Step 4b: Scheduler fires Feb 20 (no response)**
- LLM sends urgent notification, may upgrade notification_type to "call"
- Sets next_check_at = NULL (stop checking), priority = 3

### Example 2: Simple One-Shot Reminder

**Trigger:** User says "Remind me to call mom at 5pm"

**Step 1: Item creation** (tool_call_utils/management.rs)
```
INSERT: {
  summary: "Reminder: Call mom. One-shot - delete after notifying.",
  due_at: 5pm, next_check_at: 5pm,
  priority: 0, notification_type: "sms",
  last_sender: NULL (user-created, trusted)
}
```

**Step 2: Scheduler fires at 5pm** -> sends SMS "Reminder: Call mom" -> item deleted

### Example 3: Recurring Daily Digest

**Trigger:** User says "Send me a daily digest at 8am from email and whatsapp"

**Step 1: Item creation**
```
INSERT: {
  summary: "Daily digest at 08:00 user timezone. Sources: email, whatsapp.
            Repeats daily. Fetch last 24h of messages, summarize key items.",
  due_at: NULL, next_check_at: tomorrow 8am,
  notification_type: "sms", last_sender: NULL
}
```

**Step 2: Scheduler fires at 8am** -> LLM fetches sources, generates digest, sends SMS
- Replaces summary: "Daily digest at 08:00... Last sent: Feb 15." (overwrites, not append)
- Sets next_check_at = Feb 16 8am

**Repeats forever** until user says "cancel my digest" -> item deleted

### Example 4: Condition-Based Monitor

**Trigger:** User says "Alert me when I get an email from the IRS"

**Step 1: Item creation**
```
INSERT: {
  summary: "Monitor: Alert user when email from IRS arrives.",
  condition: "email from IRS",
  due_at: NULL, next_check_at: NULL (condition-triggered, not time-triggered),
  notification_type: "sms", last_sender: NULL
}
```

**Step 2: Email fetch job** - new email from "irs@irs.gov" arrives
- LLM compares email against condition "email from IRS" -> match
- **Deterministically** set last_sender = "irs@irs.gov"
- Sends SMS to user: "You received an email from the IRS: Your Tax Return Status" (notifying user is always allowed)
- If item had any destructive outgoing actions: trust check last_sender against contacts first
- LLM decides: one-time monitor -> delete item

### Example 5: Quiet Background Tracking (Shipment)

**Trigger:** Email scanning detects "Your order #1234 shipped, expected delivery Feb 18"

**Step 1: Item creation** (system creates silently, no notification)
```
INSERT: {
  summary: "Shipment: Order #1234 shipped. Expected delivery Feb 18. Carrier: FedEx.",
  condition: "email about order #1234 or FedEx tracking",
  due_at: Feb 18, next_check_at: Feb 18,
  priority: 0, notification_type: NULL (dashboard only for now),
  source_id: "email_ship456", last_sender: "shipping@amazon.com"
}
```

**Step 2: New email Feb 16** - "Order #1234 out for delivery"
- Condition matches -> last_sender = "shipping@amazon.com"
- Summary updated with delivery status

**Step 3a: "Order #1234 delivered"** -> condition matches -> item deleted

**Step 3b: No delivery update by Feb 18** -> scheduler fires
- LLM: "expected delivery passed, no confirmation"
- Upgrades: notification_type = "sms", priority = 1
- Sends SMS: "Your order #1234 was expected today - no delivery confirmation"

### Example 6: Bridge Disconnection

**Trigger:** System detects WhatsApp bridge went offline

```
INSERT: {
  summary: "System alert: WhatsApp bridge disconnected. User may miss messages.",
  priority: 1, notification_type: NULL, next_check_at: NULL,
  source_id: "bridge_whatsapp", last_sender: NULL (system-created, trusted)
}
```
Dashboard only. Deleted when bridge reconnects or user dismisses.

### Example 7: Snooze

User snoozes an item for 2 hours:
```sql
UPDATE items SET next_check_at = now + 2h WHERE id = ? AND user_id = ?
```
Item stays on dashboard but won't trigger scheduler for 2 hours.

### Example 8: Unanswered WhatsApp Question

**Trigger:** John asks "Are you free for dinner Saturday?" on WhatsApp, user isn't online

**Step 1: Item creation** (during message processing, quiet tracking)
```
INSERT: {
  summary: "Unanswered question from John on WhatsApp (room !abc123):
            'Are you free for dinner Saturday?' Asked Feb 14 3pm.
            At check time: look at recent messages in this conversation.
            If user responded or due date passed, delete. If not, remind user.",
  due_at: Feb 16 (Saturday - extracted from question context),
  next_check_at: 2 hours from now,
  priority: 0, notification_type: NULL (dashboard only initially),
  last_sender: "!abc123:matrix.org"
}
```

**Step 2: next_check_at fires (+2 hours)**
- LLM checks recent messages in room !abc123
- **User responded** -> delete item
- **User didn't respond** -> upgrades notification_type = "sms", sends reminder, sets next_check_at = tomorrow

**Step 3: Saturday (due_at) passes**
- next_check_at fires -> LLM sees due_at has passed, question is stale -> delete item
- No point nagging about something that's no longer relevant

### Example 9: Escalation (priority + notification upgrade)

As due_at approaches without user action:
```
priority 0, notification_type = NULL     -> dashboard only
priority 0, notification_type = "sms"    -> gentle SMS
priority 1, notification_type = "sms"    -> follow-up SMS
priority 2, notification_type = "call"   -> AI upgrades to phone call
```

---

## What triggers what

| Trigger | Source | next_check_at? | condition? | last_sender? |
|---------|--------|----------------|------------|--------------|
| Scheduler (every min) | `next_check_at <= now` | Yes | No | Checked at execution |
| Email fetch (every 10 min) | New emails | No | Yes - matched | Updated to email sender |
| User message | SMS/chat input | No | No | Not changed |
| System event | Bridge health | No | No | Set to NULL |
| User dashboard action | Click snooze/dismiss | Updated | No | Not changed |

Two input channels: **time-based** (next_check_at) and **data-based** (condition matching).

---

## Data Migration Strategy

### Existing tasks -> items (Rust startup migration)

Same pattern as existing `migrate_digests_to_tasks()` in `jobs/scheduler.rs`. Runs once on startup, uses a flag to avoid re-running.

Reads all active tasks, builds natural language summaries from structured fields:
- once/send_reminder -> `"Reminder: {message}. One-shot - delete after notifying."`
- recurring/generate_digest -> `"Daily digest at {time}. Sources: {sources}. Repeats {rule}."`
- recurring/email monitor -> `"Monitor: {condition}. Check email."`
- quiet_mode -> `"Quiet mode until {end_time}."`

Field mapping:
- `task.trigger` -> `item.next_check_at`
- `task.condition` -> `item.condition` (already natural language)
- `task.notification_type` -> `item.notification_type`
- All tasks are user-created -> `last_sender = NULL` (trusted)
- Only `status = 'active'` tasks migrated

### Triage items (not live yet, no migration needed)

---

## What gets removed

### Auto-reply system
- `message_reply` triage items - no longer created
- `QuickReplyButton`, `QuickReplyFlow` components in `triage_indicator.rs`
- `process_incoming_message()` triage item creation in `bridge.rs`
- `execute_triage_item` message-sending logic in `dashboard_handlers.rs`
- Related frontend callbacks

### Old tables (cleanup PR)
- `triage_items` and `tasks` tables
- All related models, repo methods, handler code

---

## Previous iterations

### v1: Full 24-column schema (abandoned)
Mapped every field from both old tables to explicit columns. Too many fields, most were rarely queried.

### v2: 8-column schema without next_check_at (abandoned)
Used due_at for both scheduling and deadlines. Problem: scheduler had to call LLM for every due item every minute. Also couldn't distinguish "notify 3 days before due" from "notify exactly at due time."

### v3: 9-column schema (evolved into current v4)
Had next_check_at but lacked notification_type and last_sender. Added those two columns to get deterministic notification routing and deterministic trust tracking.

---

## Implementation Progress

### Completed: Foundation (Steps 1-4)

**Migration:** `backend/migrations/2026-02-17-120000_create_items_table/`
- items table with all 11 columns + 3 indexes
- diesel migration run successful, schema.rs auto-updated

**Models:** `backend/src/models/user_models.rs`
- `Item` (Queryable/Selectable/Insertable/Serialize/Deserialize)
- `NewItem` (Insertable)
- Added after existing TriageItem/NewTriageItem structs

**ItemRepository:** `backend/src/repositories/item_repository.rs` - 15 methods:
- create_item, get_items, get_item (ownership check)
- get_triggered_items (scheduler: next_check_at <= now)
- get_monitor_items (condition IS NOT NULL)
- get_dashboard_items (priority desc, created_at desc)
- item_exists_by_source (dedup)
- update_next_check_at, update_priority, update_summary, update_last_sender
- update_item (bulk: summary + next_check_at + priority + notification_type)
- is_item_trusted (last_sender vs contact_profiles - email_addresses + room_ids)
- delete_item, delete_items_by_source, delete_old_items

**Registration:**
- lib.rs: module declared, re-exported, added to AppState
- main.rs: ItemRepository constructed and passed to AppState
- test_utils.rs: included in create_test_state(), TestItemParams builder + helpers

**Tests:** `backend/tests/item_repository_test.rs` - 19 tests, all passing:
- CRUD, ownership, dedup, triggered items, monitor items, dashboard ordering
- Snooze, complete (delete), reschedule, bulk update
- Cleanup old items, delete by source
- Trust: null sender trusted, unknown sender blocked, known contact trusted

**Verification:** cargo build clean, cargo test all pass, cargo clippy clean

### Completed: Steps 5-8

**Step 5 - Data migration:** Handled by startup migration in scheduler.rs.

**Step 6 - Remove auto-reply system:** Done. QuickReplyButton, QuickReplyFlow,
message_reply triage items, on_item_sent/on_item_dismissed callbacks, and
TrackedItemsList all removed.

**Step 7 - Switch callers to ItemRepository:** Done for handlers and most of the
codebase. All `/api/items/` endpoints use ItemRepository. Note: scheduler.rs
still has legacy `create_task()`/`get_user_tasks()` calls for the data migration
path - these reference the old tables and will be removed when tables are dropped.

**Step 8 - Frontend cleanup:** Done. Removed dead `trigger_type` field from
UpcomingTask struct and all usage sites, switched `/api/tasks/` URLs to canonical
`/api/items/` endpoints, replaced "triage items" with "attention items" in
privacy policy.

### Remaining: Steps 9-10

9. Full test + manual verify
10. Drop old tables and remove dead code (cleanup PR)

---

## Cleanup Debt (Step 10 - separate PR)

**Tables to drop (Diesel migration):**
- `triage_items` table
- `tasks` table
- Remove from schema.rs joinable/allow_tables macros

**Models to remove (user_models.rs):**
- `TriageItem`, `NewTriageItem` structs
- `Task`, `NewTask` structs
- Related `use crate::schema::` imports

**Repository methods to remove (user_repository.rs):**
- Triage (12): create_triage_item, get_pending_triage_items, get_triage_item_by_id,
  update_triage_item_status, snooze_triage_item, get_snoozed_items_due, get_expired_items,
  resurface_snoozed_items, expire_old_items, dismiss_triage_items_for_room,
  get_pending_triage_items_for_digest, triage_item_exists_by_source
- Tasks (15+): create_task, get_user_tasks, get_due_once_tasks, get_recurring_tasks_for_user,
  update_task_status, cancel_task, update_task_permanence, reschedule_task,
  update_task_condition, update_task_action, update_task_condition_only,
  update_task_sources, complete_or_reschedule_task, delete_old_tasks,
  get_last_completed_task_time, MAX_ACTIVE_TASKS_PER_USER

**Scheduler (jobs/scheduler.rs):**
- Remove legacy `create_task()`/`get_user_tasks()` calls used for data migration
- Remove any remaining references to old task/triage tables

**Test files to remove:**
- `backend/tests/task_repository_test.rs`
- `TestTaskParams` struct + helpers in test_utils.rs

**Migration to create:**
- `DROP TABLE triage_items`
- `DROP TABLE tasks`
