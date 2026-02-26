# Item Lifecycle

## Overview

Items are the unified unit for reminders, monitors, and system alerts.
Two types: **one-shot** (`is_monitor=0`, fires once then user dismisses) and
**monitor** (`is_monitor=1`, persists and watches for matches in incoming content).

```
Schema: items
----------------------------------------------
id              INTEGER PRIMARY KEY
user_id         INTEGER NOT NULL
summary         TEXT NOT NULL        -- human-readable description
is_monitor      INTEGER (0 or 1)    -- 0=one-shot, 1=monitor
due_at          INTEGER nullable     -- when item is due
next_check_at   INTEGER nullable     -- when scheduler should trigger
priority        INTEGER DEFAULT 0    -- 0=normal, 1=elevated, 2=urgent
notification_type TEXT nullable      -- "sms" or "call"
source_id       TEXT nullable        -- dedup key (e.g. "email_12345")
created_at      INTEGER NOT NULL
```

---

## Creation Paths

```
                            HOW ITEMS GET CREATED
 +-----------------------------------------------------------------------+
 |                                                                       |
 |  1. USER REQUEST           2. DASHBOARD API        3. SYSTEM EVENT    |
 |  "remind me to call        POST /items             Bridge health      |
 |   mom at 2pm"              from frontend           check fails        |
 |       |                         |                       |             |
 |       v                         v                       v             |
 |  AI create_task tool      filter_handlers.rs      scheduler.rs        |
 |  management.rs:246        :383                    :144                 |
 |       |                         |                       |             |
 |       v                         v                       v             |
 |  ONE-SHOT item            ONE-SHOT or MONITOR     ONE-SHOT item       |
 |  next_check_at=<2pm>      based on user input     priority=1          |
 |                                                   "System: WhatsApp   |
 |                                                    bridge disconnected"|
 +-----------------------------------------------------------------------+
 |                                                                       |
 |  4. AUTONOMOUS (email)     5. AUTONOMOUS (message)   6. MIGRATION     |
 |  Email arrives, AI         Message arrives, AI       Legacy tasks     |
 |  decides to track it       decides to track it       converted at     |
 |       |                         |                    startup          |
 |       v                         v                         |           |
 |  check_trackable_items()   [PLANNED - NOT YET            v           |
 |  proactive/utils.rs:1033    IMPLEMENTED]            scheduler.rs      |
 |       |                                             :533              |
 |       v                                                               |
 |  LLM analyzes email                                                   |
 |  categories: invoice,                                                 |
 |  shipment, deadline,                                                  |
 |  document, appointment                                                |
 |       |                                                               |
 |  is_trackable=true?                                                   |
 |  yes --> create item                                                  |
 |          silently (no                                                  |
 |          notification)                                                |
 |          source_id=                                                   |
 |          "email_{uid}"                                                |
 |          for dedup                                                    |
 +-----------------------------------------------------------------------+
```

---

## Incoming Email Flow

```
 Scheduler: email monitor job (every 10 min)
 ============================================

 Fetch new emails via IMAP
          |
          v
 For each email:
          |
          +---------------------------+
          |                           |
          v                           v
 AUTONOMOUS CREATION           MONITOR MATCHING
 (create new items)            (check existing monitors)
          |                           |
          v                           v
 check_trackable_items()       check_item_monitor_match()
 proactive/utils.rs            proactive/utils.rs
          |                           |
          v                           v
 LLM: "Is this email          LLM: "Does this email
  worth tracking?"              match any monitor?"
          |                           |
     +----+----+                 +----+----+
     |         |                 |         |
  TRACKABLE  NOT              MATCH     NO MATCH
     |      TRACKABLE            |         |
     v         |                 v         v
  Create       v          should_notify?  (skip)
  new item   (skip)              |
  silently               +------+------+
                         |             |
                      NOTIFY        SILENT
                         |             |
                         v             v
                   Send SMS/call   Update item
                   to user         summary only
                         |             |
                         v             v
                   Update item    Item persists
                   summary with   with new context
                   match context
```

---

## Incoming Message Flow (WhatsApp / Telegram / Signal)

```
 bridge.rs: real-time message from Matrix
 ==========================================

 Message received in bridged room
          |
          v
 Identify service + sender
 (whatsapp/telegram/signal,
  room_id, room_name)
          |
          +---------------------------+
          |                           |
          v                           v
 AUTONOMOUS CREATION           MONITOR MATCHING
 [PLANNED - NOT YET            check_item_monitor_match()
  IMPLEMENTED]                 bridge.rs:1685
                                      |
 Would work like email:               v
 LLM analyzes message,         LLM: "Does this message
 creates tracking items          match any monitor?"
 for questions, requests,             |
 action items from friends       +----+----+
                                 |         |
                              MATCH     NO MATCH
                                 |         |
                            (same flow   (skip)
                             as email
                             matching
                             above)
```

---

## One-Shot Item Trigger Flow

```
 Scheduler: triggered items job (every 1 min)
 ==============================================

 get_triggered_items(now)
 WHERE next_check_at <= current_time
 (monitors have NULL next_check_at, excluded)
          |
          v
 For each triggered item:
          |
          v
 generate_item_notification(summary)
          |
          v
 LLM produces:
   - SMS text (max 160 chars)
   - Voice opener (max 100 chars)
          |
          v
 send_notification()
   "sms" --> send SMS to user
   "call" --> call user with voice message
          |
          v
 Item stays in DB
 (visible on dashboard,
  user dismisses when done)
```

---

## Sender Trust Check (for restricted actions)

```
 When a recurring task/monitor triggers a RESTRICTED tool
 (send_email, respond_to_email, send_chat_message, control_tesla):

 is_sender_trusted(state, user_id, sender_context)
          |
          +------------------+------------------+
          |                  |                  |
    TIME-BASED          EMAIL SENDER      MSG SENDER
    (once_ tasks)       match against     match against
          |             contact profile   contact profile
          v             email_addresses   room_id or
    ALWAYS TRUSTED            |           chat_name
                              v                v
                    +----+--------+    +----+--------+
                    |             |    |             |
                 MATCH        NO MATCH  MATCH     NO MATCH
                    |             |    |             |
                    v             v    v             v
                 TRUSTED      BLOCKED  TRUSTED    BLOCKED
                 (proceed)    (error)  (proceed)  (error)
```

---

## Complete Lifecycle

```
 ONE-SHOT ITEM:

 Created -----> Waiting in DB -----> next_check_at -----> Notify -----> Dashboard
                (next_check_at       reached, scheduler   user via      shows item,
                 = future ts)        triggers item        SMS/call      user dismisses


 MONITOR ITEM:

 Created -----> Watching in DB -----> Email/msg -----> LLM match? --+--> NOTIFY
                (next_check_at        arrives                       |    update summary
                 = NULL)                                            |
                                                                    +--> SILENT UPDATE
                                                                    |    update summary
                                                                    |
                                                                    +--> NO MATCH
                                                                         (keep watching)

                Monitor persists until user dismisses or 30-day cleanup.


 AUTONOMOUS (email):

 Email -----> LLM: trackable? -----> YES -----> Create item silently
 arrives      (invoice, shipment,               (user sees on dashboard)
              deadline, appointment)
                      |
                      NO -----> skip


 AUTONOMOUS (message) [PLANNED]:

 Message -----> LLM: trackable? -----> YES -----> Create item silently
 arrives        (question, request,               (user sees on dashboard)
                action item, etc.)
                       |
                       NO -----> skip
```

---

## Constraints

- Max 100 items per user (enforced in create_item)
- Source dedup: same source_id won't create duplicate item
- 30-day cleanup: old items deleted daily at 3am UTC
- Sender trust: restricted tools blocked for unrecognized senders
