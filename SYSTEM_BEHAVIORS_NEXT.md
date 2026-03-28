# System Behaviors - Next Steps

## Current state (shipped)

The importance evaluation system gets:
- Conversation history (last 10 messages) with timestamps in user's timezone
- Each message marked [seen]/[unseen] from bridge read receipts
- Sender signals: 30-day message frequency, last contact recency, user reply rate, avg response time
- Cross-platform escalation: detects when a known person also messaged on another platform in the last hour
- 30-minute per-room cooldown to prevent duplicate notifications
- Group messages blocked entirely (no LLM call)

## Feedback loop

### Confirmed positive signals (reliable)

These indicate the notification was useful:

1. **User asks Lightfriend to reply** - User gets notification about Alice, then tells LF "reply to Alice saying I'll be there." Unambiguous - they acted through our system.
2. **User replies in the chat app** - A "You" message appears in the room within 30 minutes of our notification.
3. **User calls via the chat app** - Voice/video call event in the bridged room shortly after notification.
4. **User asks LF about the person** - "What did Alice say?" or "summarize messages from Alice" within 30 minutes.

### Unreliable / neutral signals

These do NOT indicate the notification was useless:

- User doesn't reply in chat (might have called IRL, handled in person, or just noted the info)
- User doesn't open the app (might have read the SMS notification and acted outside the app)
- Message stays "unseen" (dumbphone users, or user saw SMS but didn't open bridge app)

**Key principle: you can confirm a positive but never confirm a negative.** A notification with no observable response is unknown, not negative. Palantir handles intelligence tips the same way.

### Storage

Table: `system_notify_log`
- user_id, room_id, person_id, notified_at, acted_at (nullable)

Only track confirmed positives (acted_at gets set). No "ignored" state - unknowns stay NULL.

### Usage in prompt

Inject alongside existing sender signals:

> "You've notified about Alice 5 times in the last 30 days. User acted on 3 of those."

Not "ignored 2" - just the positive count and total. The LLM calibrates from the ratio without penalizing unknowns.

### Cold start

New contacts with zero notification history still get evaluated normally. Sender signals (rare contact + fast reply rate) carry the weight. PIRs (below) handle the "always notify about Mom" case explicitly.

## PIRs (Priority Intelligence Requirements)

User-defined standing rules that override LLM judgment:

- "Always notify about [person]" - bypasses evaluation entirely, always sends notification
- "Never notify about [person]" - suppresses notification for this person
- "Notify about any message mentioning [topic/keyword]"

These are the escape valve when automated judgment fails. Palantir calls them Standing Requirements - analysts always define what matters before relying on automated alerting.

Implementation: simple per-user list stored in DB. Checked before the LLM call - if a PIR matches, skip evaluation and act directly.

## Relationship classification

Explicit semantic tags on persons: family, boss/work, friend, acquaintance, service/bot.

Currently the LLM infers relationship importance from frequency and reply patterns, but this is lossy:
- A bot you auto-reply to looks like an important contact
- A family member you rarely text but always call looks unimportant
- Your boss who messages once a month looks like a casual acquaintance

Options:
- User manually tags contacts (most accurate, most friction)
- LLM classifies from message content patterns (automatic, less accurate)
- Hybrid: auto-suggest, user confirms

Store as an ont_person_edit (property: "relationship", value: "family"/"work"/"friend"/etc). Inject into sender context for the importance prompt.

## Temporal anomaly detection

"This person never messages at 2am - a 2am message is anomalous and probably important."

Compute from message history: what hours/days does this person typically message? If the current message falls outside their normal pattern, flag it as anomalous.

Implementation: bucket the person's last 30 days of messages by hour-of-day. If the current message's hour has zero or very few historical messages, add to sender context:

> "This person typically messages between 9am-6pm. This message at 2:17am is outside their usual pattern."

Cheap to compute from data we already load in `compute_sender_signals`. The LLM naturally weighs anomalous timing as a signal.
