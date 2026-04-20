# WhatsApp send-path refactor: plan + empirical data

Captured 2026-04-20. Use this as the brief for the session that completes
the integration. Every fact below is either upstream-verified (source
code from `mautrix/go` and `mautrix/whatsapp` repos) or prod-probed
(captured directly from the live lightfriend.ai deployment).

---

## Problem

Sending a WhatsApp message to a contact that the user has never DM'd
fails with `Room !xxx:localhost not found in client`. Three related
symptoms all share one root cause:

1. **Cold DM**: User has the contact in their phone book, but no Matrix
   portal room has been created yet (mautrix-whatsapp creates portals
   lazily on first message exchange).
2. **Stale room_id after bridge relink**: User unlinks WhatsApp from
   phone side, re-links. Bridge allocates new portal room IDs under a
   new `login_id`. Ontology still holds the old room IDs.
3. **Bridge sync lag**: Just after reconnect, the bridge hasn't yet
   materialized all portals. Lookups miss briefly.

All three are the same shape: "we need a portal room but don't have one
or have a stale reference."

---

## Verified data

### Bot command reference (prod probes, mautrix-whatsapp v26.04)

Captured via `POST /api/admin/bridge-send/whatsapp` on 2026-04-20.

#### `!wa help` (full text preserved so future sessions don't re-probe)

Under "Starting and managing chats":

- `bridge [login ID] <chat ID>` — Bridge existing remote chat to this room
- `create-group [group type]` — Create new group chat for current room
- `create-portal [login ID] <chat ID>` — Create Matrix room for existing remote chat
- `delete-chat [--for-everyone]` — Delete current chat on remote
- `id` — View internal network ID of current portal room
- `mute [duration]`
- `resolve-identifier [login ID] <identifier>` — Check if identifier exists on WA
- `search <query>` — Search WA for users
- `start-chat [login ID] <identifier>` — **THE command we use**. Start a direct chat.
- `sync-portal`
- `unbridge`

Administration:
- `doin <room ID> <command> [args...]` — Run command in a different room

#### `!wa start-chat +<phone>` on cold contact — success

Probe: `!wa start-chat +358442055570` (contact Perttu, never DM'd before)

Response body (verbatim):
```
Created chat with `358442055570` / Perttu (WA): Perttu (WA) (https://matrix.to/#/!qwwpwelPSZWKBxGQ8M:localhost)
```

Shape:
```
Created chat with `<jid_localpart>` / <display_name>: <room_name> (https://matrix.to/#/<room_id>)
```

The Matrix room ID is embedded directly in the bot reply. **No invite
polling needed** — by the time we see this message, the bridge has
already created the portal, invited our Matrix user, and the invite is
on the sync stream.

#### `!wa start-chat` failure shapes

Both failures start with `Failed to resolve identifier:` — single prefix,
variable reason after the colon.

Probe: `!wa start-chat +10000000000` (unreachable number):
```
Failed to resolve identifier: the server did not respond to the query
```

Probe: `!wa start-chat abc` (non-phone input):
```
Failed to resolve identifier: WhatsApp only supports phone numbers as user identifiers. Number looks like email
```

#### `!wa list-logins` — login ID format

```
* `358442105886` (+358442105886) - `CONNECTED`
```

`login_id` is the bare phone number (no `+`). This equals `user_login.id`
in the bridge DB. `[login ID]` arg to `start-chat` is optional for single-
login users; we omit it.

#### `!wa doin <portal_room> id` — portal row confirmation

Probe on Perttu's portal `!qwwpwelPSZWKBxGQ8M:localhost`:
```
This room is bridged to `358442055570@s.whatsapp.net` (receiver: `358442105886`) on WhatsApp
```

Confirms on prod:
- `portal.id` = full JID with `@s.whatsapp.net` suffix
- `portal.receiver` = user's login phone (bare)
- The bridge can route commands in portal rooms for our user (= user is joined)

#### `!wa resolve-identifier` — lookup only, no side effects

Success: ``Found `358442055570` / Perttu (WA)``
Failure: `Failed to resolve identifier: the server did not respond to the query`

Same failure prefix as `start-chat`. Not needed in the send path (the
bridge DB contact lookup is authoritative and free) but useful for
future diagnostics.

#### `!wa search <query>` — not used in design

```
Search results:

* `358442055570` / Perttu (WA)
```

Current design doesn't use it. Bridge DB contact search is faster,
authoritative, and already in use by the rule builder.

### Schema verification (upstream sources)

#### mautrix-go `bridgev2/database/upgrades/00-latest.sql`

```sql
CREATE TABLE portal (
    bridge_id       TEXT    NOT NULL,   -- set by bridge binary; "whatsapp" for us
    id              TEXT    NOT NULL,   -- the chat_id: "<phone>@s.whatsapp.net" or "<gid>@g.us"
    receiver        TEXT    NOT NULL,   -- login_id (user's WA phone) or '' for shared portals
    mxid            TEXT,               -- Matrix room ID, nullable until materialized
    parent_id       TEXT,
    parent_receiver TEXT    NOT NULL DEFAULT '',
    relay_bridge_id TEXT,
    relay_login_id  TEXT,
    other_user_id   TEXT,               -- for DMs: ghost ID directly (no need to derive from template)
    name            TEXT    NOT NULL,
    topic           TEXT    NOT NULL,
    -- ... several more columns we don't use
    room_type       TEXT    NOT NULL,   -- "DM", "GROUP_DM", etc.
    -- ... more metadata columns
    PRIMARY KEY (bridge_id, id, receiver)
);
CREATE UNIQUE INDEX portal_bridge_mxid_idx ON portal (bridge_id, mxid);
```

#### mautrix-go `bridgev2/database/portal.go` — canonical query

The bridge's own query for "get portal by chat_id, tolerating either a
login-scoped or global receiver" is:

```go
`WHERE bridge_id=$1 AND id=$2 AND (receiver=$3 OR receiver='')`
```

Our repo method uses the same pattern verbatim.

#### mautrix-whatsapp `pkg/connector/connector.go`

```go
NetworkID: "whatsapp"
```

This string populates `portal.bridge_id`. Our query uses
`bridge_id = 'whatsapp'` accordingly.

#### Ghost user templates (enclave configs)

From `enclave/configs/whatsapp.yaml.template`:
```yaml
username_template: whatsapp_{{.}}
```
- `{{.}}` = JID localpart (phone for `s.whatsapp.net` JIDs, LID string
  otherwise)

Ghost user IDs:
- WhatsApp: `@whatsapp_<jid_localpart>:localhost`
- Telegram: `@telegram_<tg_userid>:localhost` (from telegram.yaml.template)
- Signal:   `@signal_<uuid>:localhost` (from signal.yaml.template)

Not used by the final design (we get ghost info from `portal.other_user_id`
instead of deriving), but useful for future work.

---

## Final algorithm (locked, 3 steps)

```
═══════════════════════════════════════════════════════════════════
INPUT: user_id, platform="whatsapp", name_query, message

STEP A — Resolve name -> ChatCandidate
──────────────────────────────────────
  A1. Ontology fast path (existing find_person_room):
      Match by name/nickname against ont_persons with a WA channel.
      Hit: ChatCandidate { chat_id = channel.handle, mxid = channel.room_id, ... }

  A2. If miss: bridge DB search (NEW):
      WhatsAppBridgeRepository::search_chats_for_login(login_phone)
      Returns Vec<ChatCandidate> covering BOTH:
        - DMs from whatsmeow_contacts LEFT JOIN portal for mxid
        - Groups from portal directly (id LIKE '%@g.us')
      Fuzzy-rank in Rust, take best match.

  A3. Both miss: Err("no WhatsApp contact found matching '<name>'")

STEP B — Ensure portal mxid (DM cold path only)
───────────────────────────────────────────────
  candidate.mxid:
    Some(mxid) -> use it directly (warm DM or group, most common)
    None, is_group=false (DM):
      !wa start-chat +<phone> via probe_bridge_room
      classify_whatsapp_start_chat(reply) ->
        Created { room_id, display_name } -> use room_id
        Failed { reason }                  -> Err("start-chat failed: <reason>")
    None, is_group=true:
      Err("group not yet bridged - retry shortly")

STEP C — Send + upsert
──────────────────────
  client.join_room_by_id(mxid).await.ok();    // idempotent, safe after start-chat
  if client.get_room(mxid).is_none() {
      client.sync_once(SyncSettings::new()).await?;
  }
  let room = client.get_room(mxid).ok_or_else(|| ...)?;
  room.send(RoomMessageEventContent::text_plain(message)).await?;

  // Post-send only — never touch ontology on failure
  ontology_repository.upsert_person(
      user_id,
      display_name,
      platform,
      Some(chat_id),
      Some(mxid),
  )?;
═══════════════════════════════════════════════════════════════════
```

### Why this shape

- **One unified SQL search in Step A** instead of separate contacts+portals
  calls. DMs and groups come back with `mxid` pre-fetched.
- **Portal table is authoritative, never cached**. No ontology invalidation,
  no staleness. Every warm send is a single SQL query.
- **`start-chat` only runs on genuine cold paths**. Bot round-trip amortizes
  to once per contact per bridge-login lifetime.
- **No ghost-scan, no `joined_rooms()` enumeration, no derived ghost IDs.**
  We proved the portal table gives us everything directly.

### Reliability audit (locked with user)

| Failure mode | Handling |
|---|---|
| Bridge DB unreachable | Err "bridge database unavailable" |
| Contact not in bridge DB | Err "no contact found" |
| Portal row exists, mxid NULL | Fall into `start-chat` path |
| Portal row exists, mxid set, client hasn't synced | `sync_once` retry |
| `start-chat` bot reply malformed | Err with raw reply |
| Group `mxid=None` | Err "group not yet bridged, retry shortly" |
| User manually left portal room | `get_room` None → Err (same as today) |

---

## What's landed on this branch (additive, zero behavior change)

All files in `backend/`. Nothing wired up yet; no behavior change if we
shipped now.

### `repositories/whatsapp_bridge_repository.rs`

Added:

- `struct ChatCandidate { chat_id, display_name, is_group, mxid }`
- `fn search_chats_for_login(login_phone) -> Vec<ChatCandidate>` —
  unified DM + group search with pre-fetched `mxid`
- `fn get_portal_mxid(chat_id, login_phone) -> Option<String>` —
  single-row portal lookup (used by integration step)

### `utils/bridge_responses.rs`

Added:

- `mod verified::whatsapp_start_chat` with `SUCCESS_PREFIX` and `FAILURE_PREFIX`
- `enum WhatsAppStartChatReply { Created { room_id, display_name }, Failed { reason } }`
- `fn classify_whatsapp_start_chat(body) -> Option<WhatsAppStartChatReply>`

### `tests/bridge_responses_test.rs`

Added 6 tests, all passing:

- `whatsapp_start_chat_success_parses_room_id_and_display_name`
- `whatsapp_start_chat_failure_server_timeout`
- `whatsapp_start_chat_failure_invalid_identifier_shape`
- `whatsapp_start_chat_unknown_body_returns_none`
- `whatsapp_start_chat_success_requires_matrix_to_url`
- `whatsapp_start_chat_success_rejects_non_room_mxid`

All 45 tests in the file pass.

### Yesterday's defense-in-depth (kept for now)

Reconnect-purge of `ont_channels.room_id` on bridge reconnect, in:
- `handlers/whatsapp_auth.rs`
- `handlers/telegram_auth.rs`
- `handlers/signal_auth.rs`

Plus `ontology_repository::clear_platform_room_ids` and
`ontology_repository::replace_channel_room_id` helpers.

Plus the self-heal-via-name-search block in
`utils/bridge.rs::send_bridge_message`.

These become redundant once the new flow lands but are harmless to keep.
Delete after the new path is confirmed in production.

---

## What remains (the integration work for the next session)

### 1. `utils/bridge.rs` — new helper

```rust
/// Send "!wa start-chat +<phone>" to the WA bridge management room,
/// wait for the bot reply, and return the Matrix room ID of the
/// newly-created (or existing) portal.
pub async fn start_chat_whatsapp(
    state: &Arc<AppState>,
    user_id: i32,
    chat_id: &str,   // full JID like "358442055570@s.whatsapp.net"
) -> Result<matrix_sdk::ruma::OwnedRoomId>
```

Implementation: reuse existing `probe_bridge_room`. Feed its output
through `classify_whatsapp_start_chat`. Return `Ok(room_id)` on Created,
`Err` on Failed or unclassifiable.

### 2. `utils/bridge.rs::send_bridge_message` — signature + logic change

Add one optional parameter:

```rust
pub async fn send_bridge_message(
    service: &str,
    state: &Arc<AppState>,
    user_id: i32,
    chat_name: &str,
    message: &str,
    media_url: Option<String>,
    target_room_id: Option<&str>,
    target_chat_id: Option<&str>,   // NEW
) -> Result<BridgeMessage>
```

Logic:

- If `target_room_id` is `Some(rid)` and `client.get_room(rid)` is Some
  → use it (warm path, works today).
- Else if `target_chat_id` is `Some(jid)` AND `service == "whatsapp"`:
  - Query `portal` table (`get_portal_mxid(jid, login_phone)`).
  - `Some(mxid)` → use it.
  - `None` + jid is DM (`@s.whatsapp.net`) → `start_chat_whatsapp`, use result.
  - `None` + jid is group (`@g.us`) → error "group not bridged, retry".
- Else → fall through to existing name-based search (backward compat).

After successful send: call `ontology_repository.upsert_person` with the
resolved `(chat_id, mxid, display_name)`. Only on success; never on
error.

### 3. `tool_call_utils/bridge.rs::handle_send_chat_message` — rewire

Current flow:
```
find_person_room -> Option<BridgeRoom>
  fallback: get_service_rooms + search_best_match -> Option<BridgeRoom>
  error if both miss
queue 60s delayed send with room_id
delayed: send_bridge_message(..., Some(room_id))
```

New flow:
```
Step A resolution ->
  1. find_person_room (existing, gives us both handle AND room_id if present)
  2. if miss: search_chats_for_login (NEW) -> fuzzy match -> ChatCandidate
  3. if miss: error

queue 60s delayed send with BOTH candidate.mxid (if any) and candidate.chat_id

delayed: send_bridge_message(
    ...,
    target_room_id: candidate.mxid.as_deref(),
    target_chat_id: Some(&candidate.chat_id),
)
```

Note: `find_person_room` currently returns `BridgeRoom` which only has
`room_id`. Extend it or pass through a sibling struct that carries `handle`
as well. Ontology's `channel.handle` IS the JID by our convention.

### 4. Delete yesterday's self-heal (once integration is confirmed)

In `utils/bridge.rs::send_bridge_message`, the block starting around
`// Stale room ID - most common after the user re-links...` becomes
redundant. The new `target_chat_id` path subsumes it. Delete after first
successful prod test of the new flow.

Same for `clear_platform_room_ids` calls in the `*_auth.rs` reconnect
finalizers and the helper methods in `ontology_repository.rs`. Delete in
the same follow-up commit.

### 5. Tests

- Unit: `start_chat_whatsapp` reply parsing (already covered by existing
  `classify_whatsapp_start_chat` tests).
- Integration: new `send_bridge_message` path. Current
  `tests/bridge_test.rs` has trait-based mocks for `send_bridge_message_trait`
  — extend with a `send_bridge_message_with_chat_id_trait` or similar
  so we can test the cold-DM path without hitting a real bridge.

---

## Open decisions

- **Telegram and Signal extension**: current design is WhatsApp-only.
  Telegram/Signal need analogous bridge DB repos (they're bridgev2 too
  for v26.04+) before they can use this flow. Scope them as a follow-up
  ticket.
- **Rate limiting `start-chat`**: 50 cold sends in a burst would send 50
  bot commands. Probably fine for typical use, but a per-user semaphore
  is cheap insurance if it becomes an issue.
- **Display name mismatch**: bridge DB has one display name, ontology may
  have a user-edited nickname. Upsert should NOT clobber user edits. The
  existing `upsert_person` logic preserves them via the `ont_person_edits`
  table — verify this in integration testing.

---

## Context for the next session's first prompt

Everything in this doc is locked. The next session should:

1. Read this file.
2. Read the relevant code:
   - `backend/src/utils/bridge.rs` (lines around `send_bridge_message` and
     `probe_bridge_room`)
   - `backend/src/tool_call_utils/bridge.rs` (`handle_send_chat_message`,
     `find_person_room`)
   - `backend/src/repositories/whatsapp_bridge_repository.rs` (already has
     the new methods)
   - `backend/src/utils/bridge_responses.rs` (already has the parser)
   - `backend/src/repositories/ontology_repository.rs` (`upsert_person`)
3. Execute the integration work in §"What remains". No new probes needed,
   no new design decisions needed.
4. `cargo check && cargo test --test bridge_responses_test && cargo test`
   before commit.
5. Do NOT push without explicit permission.
