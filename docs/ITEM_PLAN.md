# Unified Item Model - Architecture Plan

## Problem

We currently have two separate tables handling what is conceptually one thing:

- `triage_items`: incoming attention items (message replies, email tracking, system notices)
- `tasks`: scheduled actions (reminders, digests, recurring checks)

This split is artificial. An invoice due Feb 20 is the same mental object whether it came from email scanning (triage) or the user saying "remind me to pay AWS" (task). The AI has to choose which system to use, the frontend has to combine two data sources, and escalation between them (tracking -> reminder -> notification) is awkward.

## Goal

Build a unified "working memory" for the assistant. One system, one mental model for users: "I told my assistant about it, it's handled."

This matters especially for ADHD users - trust requires predictability. One system with consistent behavior is easier to trust than two systems with different rules.

## Design Principle

The AI models are getting smarter. Our job is not to build complex logic - it's to give the AI the right context at the right time and let it decide. The simpler and more uniform our data model, the easier that becomes.

## Unified Item Schema

Replace both `triage_items` and `tasks` with a single `items` table:

```sql
CREATE TABLE items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,

    -- What is it?
    summary TEXT NOT NULL,              -- "AWS invoice $45 due Feb 20"
    item_type TEXT NOT NULL,            -- category: "invoice", "shipment", "reminder", "digest", "reply", "system"
    context_json TEXT,                  -- flexible blob: original message, conversation snippet, email uid, etc.

    -- Where did it come from?
    source TEXT,                        -- "email", "whatsapp", "telegram", "user", "system"
    source_id TEXT,                     -- room_id, email_uid, etc. for dedup and linking back

    -- When does it matter?
    trigger_type TEXT NOT NULL,         -- "immediate", "scheduled", "recurring", "conditional"
    trigger_at INTEGER,                 -- unix timestamp for scheduled items
    recurrence_rule TEXT,               -- "daily", "weekly:1,3,5", "monthly:15" for recurring
    recurrence_time TEXT,               -- "08:00" HH:MM for recurring
    condition TEXT,                     -- for conditional triggers: "weather < 0C", etc.

    -- What to do when triggered?
    action TEXT NOT NULL,               -- "notify_user", "send_message", "generate_digest", "check_and_notify", "track_silently"
    action_detail TEXT,                 -- the message to send, digest sources, etc.
    notification_type TEXT,             -- "sms", "call" - how to reach the user

    -- Status
    status TEXT NOT NULL DEFAULT 'active',  -- "active", "snoozed", "completed", "dismissed"
    snooze_until INTEGER,
    expires_at INTEGER,

    -- Metadata
    priority INTEGER NOT NULL DEFAULT 0,    -- 0=normal, 1=elevated, 2=urgent
    created_at INTEGER NOT NULL,
    updated_at INTEGER,

    FOREIGN KEY (user_id) REFERENCES users(id)
);
```

## How Current Data Maps

### triage_items -> items

| triage field | items field | notes |
|---|---|---|
| item_type "message_reply" | item_type "reply", trigger_type "immediate", action "send_message" | suggested_action -> action_detail |
| item_type "email_invoice" | item_type "invoice", trigger_type "scheduled" (due date) or "immediate", action "track_silently" | |
| item_type "email_shipment" | item_type "shipment", same pattern | |
| item_type "email_deadline" | item_type "deadline", trigger_type "scheduled", action "notify_user" | |
| item_type "bridge_disconnected" | item_type "system", trigger_type "immediate", action "notify_user" | |

### tasks -> items

| task field | items field | notes |
|---|---|---|
| action "generate_digest" | item_type "digest", trigger_type "recurring", action "generate_digest" | sources -> action_detail |
| action "reminder" | item_type "reminder", trigger_type "scheduled", action "notify_user" | |
| action with condition | trigger_type "conditional", condition field | |
| is_permanent=1 + recurrence | trigger_type "recurring" | |
| is_permanent=0 (one-shot) | trigger_type "scheduled" | completed after execution |

## The Loop (Scheduler)

One unified loop replaces the current separate triage check + task scheduler:

```
every minute:
    items = get_active_items(user_id)
    for item in items:
        if should_trigger(item):
            execute_action(item)
            if item.trigger_type == "recurring":
                reschedule(item)
            else:
                mark_completed(item)
```

`should_trigger` checks:
- "immediate": always true (shown on dashboard, executed when created)
- "scheduled": trigger_at <= now
- "recurring": next occurrence <= now
- "conditional": evaluate condition (weather API, etc.), if true then trigger

## Dashboard Changes

The dashboard summary endpoint returns items grouped by how they should display:

- **Attention items** (trigger_type "immediate", action != "track_silently"): message replies, system alerts
- **Tracked items** (action "track_silently"): invoices, shipments, deadlines - the section we just built
- **Upcoming** (trigger_type "scheduled" or "recurring", trigger_at in future): reminders, digests, checks

Frontend renders the same components we have now, just backed by one data source. The `TrackedItemsList`, `QuickReplyFlow`, timeline - all still work, just query one table.

## AI Tool Calls

Instead of separate `create_task` and `create_triage_item` tools, the AI gets one tool:

```
create_item:
  summary: string
  item_type: string
  trigger_type: "immediate" | "scheduled" | "recurring" | "conditional"
  trigger_at: optional timestamp
  recurrence_rule: optional string
  action: "notify_user" | "send_message" | "generate_digest" | "check_and_notify" | "track_silently"
  action_detail: optional string
  notification_type: optional "sms" | "call"
  priority: optional 0-2
```

The smarter the model gets, the better it decides what trigger and action to use. We just need to give it the user's context.

## Escalation (Key Feature)

With a unified model, escalation is just updating fields:

1. Email detected: invoice due in 30 days -> `action: "track_silently"`, `trigger_type: "scheduled"` at due_date - 3 days
2. 3 days before due: trigger fires -> update `action: "notify_user"`, send SMS
3. User snoozes -> `snooze_until` set
4. Due date passes, still active -> `priority` bumps to 2 (urgent)

No cross-table migrations, no "convert triage to task" logic. Just field updates on one row.

## Migration Strategy

### Step 1: Create the table
- New migration: create `items` table
- Keep `triage_items` and `tasks` tables intact

### Step 2: Data migration
- Write a one-time migration that copies all active triage_items and tasks into items
- Map fields per the tables above

### Step 3: Update backend
- New repository: `ItemRepository` with unified CRUD
- Update scheduler to use items table
- Update dashboard handler to query items instead of both tables
- Update AI tool calls to use `create_item`
- Keep old endpoints working temporarily (they read from items table now)

### Step 4: Update frontend
- Minimal changes - the dashboard already groups by type
- Remove any remaining triage-vs-task distinctions

### Step 5: Cleanup
- Remove old triage_items and tasks tables in a later migration
- Remove old repository methods
- Remove old handler endpoints

## What This Unlocks

1. **Simpler AI integration**: one tool, one mental model for the AI
2. **Natural escalation**: tracking -> reminder -> notification is just field updates
3. **Predictable for users**: one list of "things my assistant is handling"
4. **Easier frontend**: one data source, consistent actions (complete, dismiss, snooze, edit)
5. **Future-proof**: as AI gets smarter, it can set more sophisticated triggers and actions without schema changes
