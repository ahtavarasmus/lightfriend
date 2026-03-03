# Backend Refactor Roadmap

## Vision: Unified Agent Architecture

One agent, multiple modes - all sharing the same tool registry, context building, and dispatch infrastructure.

| Mode | Flow | Purpose |
|------|------|---------|
| **Chat** | User query -> agent with tools -> response | Interactive conversation |
| **Scheduled execution** | Trigger -> agent with tools + source data -> execute | Proactive tasks (digests, reminders, condition checks) |
| **Classify** | Incoming message -> agent classifies + creates triage | Single-call classification (replaces current 2-3 separate calls) |
| **Post-classification** | Acts on classification result only | Air gap - no raw message data touches the action layer |

**Safety baseline:** Outgoing tools (send_message, send_email, create_event) require user confirmation before executing. A prompt-injected agent cannot cause harm because the user gates all outgoing actions. Architecture handles safety, not prompt engineering.

---

## Phase 1: Context Builder [NOT STARTED]

**Problem:** Tool call context (user info, timezone, connections, preferences) is assembled ad-hoc in multiple places. The same "what does this user have connected?" logic is duplicated across chat handling, scheduled execution, and classification.

**Goal:** A single `ContextBuilder` that assembles everything the agent needs to know about a user before any LLM call.

**What it builds:**
- User info (timezone, language, subscription tier)
- Active connections (which platforms are connected, room IDs)
- Contact profiles (who the user knows, per-contact settings)
- Available tools (filtered by subscription + connections)
- Conversation history (for chat mode)
- Source data (for scheduled execution mode - fetched emails, messages)

**Key design:**
```
ContextBuilder::new(state, user_id)
    .with_tools()          // filters by subscription + connections
    .with_history(n)       // last N conversation turns
    .with_sources(["email", "chat"])  // fetch source data
    .build()               // returns AgentContext
```

**Why this matters:** Every subsequent phase becomes simpler because context assembly is one function call, not scattered logic.

**Files likely affected:**
- New: `backend/src/agent/context.rs`
- Modified: `backend/src/api/twilio_sms.rs` (extract context assembly from chat handler)
- Modified: `backend/src/proactive/utils.rs` (extract context assembly from scheduled execution)

---

## Phase 2: Tool Registry [DONE]

**Problem:** 800-line if/else chain in `twilio_sms.rs` for tool dispatch. Each branch duplicated JSON parsing, error handling, history writing, and logging. Outgoing tools had ~40 lines of identical boilerplate each.

**Solution:** Trait-based `ToolHandler` with `ToolRegistry` for dispatch.

**What was built:**
- `backend/src/tools/registry.rs` - `ToolHandler` trait, `ToolRegistry`, `ToolContext`, `ToolResult`
- 10 tool handler files in `backend/src/tools/` (email, items, messaging, respond, schedule, search, tesla, weather, youtube)
- Registry initialized in `AppState`, dispatch loop in `twilio_sms.rs` (~30 lines replacing ~800)

**Current tool mapping (18 handlers):**
- Search: `PerplexityHandler`, `FirecrawlHandler`, `QrScanHandler`
- Weather: `WeatherHandler`
- Email: `FetchEmailsHandler`, `FetchSpecificEmailHandler`, `SendEmailHandler`, `RespondEmailHandler`
- Messaging: `SearchContactsHandler`, `FetchRecentHandler`, `FetchMessagesHandler`, `SendMessageHandler`
- Items: `ListTrackedItemsHandler`, `UpdateTrackedItemHandler`
- Schedule: `CreateItemHandler`
- Tesla: `TeslaControlHandler`
- YouTube: `YouTubeHandler`
- Response: `DirectResponseHandler`

**MCP tools** remain separate (dynamic, per-user) with their own dispatch.

---

## Phase 3: Tool Consolidation (21 -> ~12) [NOT STARTED]

**Problem:** The model sees 21 tool definitions, many of which are closely related. This wastes context tokens and creates decision paralysis for the model (e.g., "should I use fetch_emails or fetch_specific_email?").

**Goal:** Consolidate tools that are logically one action with different parameters.

**Proposed consolidation:**

| Current tools | Merged into | How |
|---------------|-------------|-----|
| `fetch_emails` + `fetch_specific_email` | `fetch_emails` | Add optional `message_id` param |
| `send_email` + `respond_to_email` | `send_email` | Add optional `reply_to_id` param |
| `search_chat_contacts` + `fetch_recent_messages` + `fetch_chat_messages` | `messaging` | Action param: "search", "recent", "fetch" |
| `control_tesla` | `tesla` | Already single tool |
| `ask_perplexity` + `search_firecrawl` | `web_search` | Engine param |
| `create_task` + `update_monitoring_status` | `schedule` | Action param |

**This is a model-facing change** - tool names and parameter schemas change, so prompts and model behavior need testing. The underlying handler functions stay the same.

**Risk:** Model might call tools differently. Needs A/B testing or at minimum manual testing of common queries.

---

## Phase 4: Classify Mode Unification [NOT STARTED]

**Problem:** Incoming message classification currently uses 2-3 separate LLM calls:
1. Classify the message (critical/mention/all/ignore)
2. Maybe extract metadata
3. Maybe generate a triage summary

Each call has its own prompt, context building, and error handling.

**Goal:** Single LLM call that does classification + triage creation in one shot using structured output.

**Design:**
```
Input: incoming message + contact profile + user preferences
Output (structured): {
    classification: "critical" | "mention" | "all" | "ignore",
    triage_summary: "Mom asking about Sunday dinner",
    suggested_reply: "Yes, I'll be there around 6",
    confidence: 0.95
}
```

**Safety:** Post-classification actions (notifications, triage creation) operate only on the structured output, never on raw message content. This is the "air gap" - even if the message contains prompt injection, the classification result is a constrained enum + short strings that can't execute arbitrary actions.

**Files likely affected:**
- Modified: `backend/src/utils/bridge.rs` (current classification logic)
- Modified: `backend/src/proactive/utils.rs`
- New: `backend/src/agent/classify.rs`

---

## Phase 5: Scheduled Execution as Agent Calls [NOT STARTED]

**Problem:** Scheduled task execution (digests, condition checks, reminders) has its own code path that duplicates much of what the chat handler does - fetching sources, calling LLM, executing tools.

**Goal:** Scheduled execution uses the same agent loop as chat mode, just with different initial context (source data instead of user message).

**Flow:**
```
Trigger fires
  -> ContextBuilder assembles sources (email, messages)
  -> Agent receives: "Generate morning digest from these sources"
  -> Agent uses tools (same registry) to format and deliver
  -> Result written to history
```

**Depends on:** Phase 1 (Context Builder) and Phase 2 (Tool Registry, already done).

---

## Phase 6: Dead Code and Cleanup Rounds [ONGOING]

Periodic passes to remove accumulated dead code, unused dependencies, and debug artifacts.

**Completed:**
- Round 1 (Feb 2026): Removed ~12,000 lines across 17 deleted files + trimmed dead code from live files. Replaced `println!` debug statements with `tracing::debug!`. See commit `9a98a9f`.

**Future candidates (from adaptive-prancing-bee plan):**
- Unused Cargo dependencies in `backend/Cargo.toml`
- Dead CSS in frontend stylesheets
- Stale image assets
- Old `tool_call_utils/` module (can be removed once all tools use the registry handlers directly, and no other code references the old utils)

---

## Implementation Order

```
Phase 1: Context Builder        <- foundation for everything else
Phase 2: Tool Registry          <- DONE
Phase 3: Tool Consolidation     <- model-facing, needs testing
Phase 4: Classify Unification   <- depends on Phase 1
Phase 5: Scheduled as Agent     <- depends on Phase 1 + 2
Phase 6: Cleanup                <- ongoing, independent
```

Phases 3 and 4 can proceed in parallel once Phase 1 is done. Phase 5 depends on both 1 and 4 being stable.

---

## Design Reference

See `personal-agi-dashboard-brainstorm.md` in `.claude/plans/` for the dashboard UX vision that drives these backend changes - particularly the "dump and forget" pattern, triage mode, and chat-first editing philosophy.
