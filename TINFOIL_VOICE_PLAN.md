# Tinfoil Voice Pipeline - Implementation Plan

Replace ElevenLabs ConvAI with a self-hosted real-time voice pipeline using Tinfoil's confidential computing APIs for STT, LLM, and TTS. All audio processing happens in enclaves - no third party hears user conversations.

## Tinfoil API Details

Base URL: `https://inference.tinfoil.sh`
Auth: `Authorization: Bearer <TINFOIL_API_KEY>` (env var)
All endpoints are OpenAI-compatible.

### Models to Use

| Purpose | Model | Endpoint | Format |
|---------|-------|----------|--------|
| STT | `whisper-large-v3-turbo` | `POST /v1/audio/transcriptions` | Multipart form (file + model) |
| LLM | `llama3-3-70b` | `POST /v1/chat/completions` | JSON, supports streaming + tool calling |
| TTS | `qwen3-tts` | `POST /v1/audio/speech` | JSON `{model, input, voice?}`, returns WAV (PCM 16-bit mono 24kHz) |

### TTS Voices Available
aiden, dylan, eric, ono_anna, ryan, serena, sohee, uncle_fu, vivian

### Measured Latencies (Finland to Tinfoil)
- LLM first token (streaming): ~585ms
- LLM total (short response): ~1.2s
- STT (5s audio): ~1.45s
- TTS: ~2s for short sentence

## Current Architecture (What We're Replacing)

ElevenLabs runs a managed ConvAI platform that handles everything in one WebSocket:
- ASR (speech-to-text) with streaming
- LLM reasoning (GPT-4.1) with tool calling via webhooks back to our server
- TTS (eleven_flash_v2) with streaming audio
- Turn detection, interruption handling

Our server currently only:
1. Provides user context at call start (`POST /api/call/assistant`)
2. Handles tool webhooks (email, SMS, weather, etc.) - ElevenLabs calls our endpoints
3. Processes completion webhooks for billing

### Current Tool Handlers (in `backend/src/api/elevenlabs.rs`)

These are HTTP endpoints that ElevenLabs calls as webhooks during a voice call. Each receives `user_id` as a query param and `x-elevenlabs-secret` header for auth. They return JSON with a `response` field containing voice-friendly text.

| Tool | Current Endpoint | Method | Purpose |
|------|-----------------|--------|---------|
| fetch_emails | `/api/call/email` | GET | Fetch 10 recent emails via IMAP |
| fetch_specific_email | `/api/call/email/specific` | POST | Search emails by keyword |
| respond_to_email | `/api/call/email/respond` | POST | Reply to email by ID |
| send_email | `/api/call/email/send` | POST | Compose and send new email |
| send_sms | `/api/call/sms` | POST | Send SMS via Twilio |
| send_chat_message | `/api/call/send-chat-message` | POST | Send WhatsApp/Telegram/Signal message (60s delay, cancellable) |
| fetch_recent_messages | `/api/call/fetch-recent-messages` | GET | Recent messages across all chats |
| fetch_chat_messages | `/api/call/fetch-chat-messages` | GET | Messages from specific chat |
| search_chat_contacts | `/api/call/search-chat-contacts` | POST | Fuzzy search contacts |
| cancel_message | `/api/call/cancel-message` | GET | Cancel pending message within 60s |
| get_weather | `/api/call/weather` | POST | Weather by location |
| perplexity_search | `/api/call/perplexity` | POST | Internet search via Perplexity |
| firecrawl_search | `/api/call/firecrawl` | POST | Web scraping |
| create_task | `/api/call/items/create` | POST | Create task/reminder |
| end_call | (built-in) | - | End the conversation |

### System Prompt
The full system prompt is in `backend/src/api/elevenlabs_agent_config.json` lines 104-105. Key points:
- Personality: efficient, direct, action-oriented
- Context injected via dynamic variables: user name, location, timezone, nearby_places, recent_contacts, user_info, recent_conversation
- Per-minute billing awareness - be brief
- Message sending requires verbal confirmation before invoking tool
- Inbound vs outbound call handling (outbound delivers notification)

### Dynamic Variables Built at Call Start
See `build_conversation_variables()` in `elevenlabs.rs`:
- name, user_id, user_info, location, nearby_places, timezone, timezone_offset_from_utc
- recent_conversation (last message from history)
- recent_contacts (from ontology)
- email_id, content_type, notification_message (for outbound notification calls)
- now (current UTC timestamp)

### Credit System
- `VOICE_SECOND_COST`: 0.0033 credits/second (env var)
- `CHARGE_BACK_THRESHOLD`: 2.0 credits minimum (env var)
- Credits checked before call starts
- Credits deducted after call ends (duration * cost)
- Failed/unanswered calls: no charge

## New Architecture

```
Phone Call -> Twilio (PSTN)
                |
                | <Connect><Stream url="wss://server/api/voice/ws">
                v
     ┌─────────────────────────────────────────────┐
     │  Axum WebSocket Handler                     │
     │                                              │
     │  1. Receive mulaw 8kHz audio from Twilio    │
     │  2. Decode mulaw -> PCM, resample 8k->16k   │
     │  3. VAD (Silero) - detect speech boundaries │
     │  4. On speech end: send audio to Tinfoil STT│
     │  5. Send transcript to Tinfoil LLM          │
     │     (streaming, with tool definitions)       │
     │  6. Execute tool calls locally               │
     │  7. Send response text to Tinfoil TTS        │
     │     (sentence-by-sentence as LLM streams)    │
     │  8. Convert TTS audio: PCM 24k->8k, mulaw   │
     │  9. Stream audio back to Twilio WebSocket    │
     │ 10. Handle barge-in (clear on user speech)   │
     └─────────────────────────────────────────────┘
```

## Implementation Phases

### Phase 1: Minimal Voice Loop (Start Here)

Goal: Incoming call -> hear user -> transcribe -> LLM response -> speak back. No tool calls yet.

#### 1a. TwiML Endpoint
New endpoint: `GET /api/voice/incoming`
- Twilio calls this when a call comes in
- Look up user by caller phone number (reuse logic from `fetch_assistant`)
- Return TwiML that connects to our WebSocket:
```xml
<Response>
  <Connect>
    <Stream url="wss://your-server/api/voice/ws">
      <Parameter name="user_id" value="123" />
      <Parameter name="call_sid" value="CA..." />
    </Stream>
  </Connect>
</Response>
```

#### 1b. WebSocket Handler
New endpoint: `GET /api/voice/ws` (WebSocket upgrade)

**Twilio Media Streams Protocol:**

Messages FROM Twilio (JSON text frames):
- `connected` - WebSocket established
- `start` - Call metadata: streamSid, callSid, mediaFormat (audio/x-mulaw, 8000Hz, mono), customParameters
- `media` - Audio chunks every 20ms. `media.payload` is base64-encoded mulaw. ~50 messages/second.
- `mark` - Sent when queued audio finishes playing (for barge-in tracking)
- `stop` - Call ended

Messages TO Twilio:
- `media` - Send audio: `{"event":"media","streamSid":"MZ...","media":{"payload":"<base64 mulaw>"}}`
- `mark` - Track playback: `{"event":"mark","streamSid":"MZ...","mark":{"name":"utterance-1"}}`
- `clear` - Barge-in: `{"event":"clear","streamSid":"MZ..."}` - immediately stops all buffered audio

**Audio format details:**
- Twilio sends/receives: mulaw (G.711), 8000 Hz, 8-bit, mono
- Each chunk: 160 bytes raw (20ms at 8kHz), base64-encoded to ~214 bytes
- Silence byte: 0xFF
- IMPORTANT: No WAV/RIFF headers when sending back - raw mulaw samples only

#### 1c. Audio Processing Pipeline

```
Twilio mulaw 8kHz
    -> base64 decode
    -> mulaw decode to PCM i16 (law-encoder crate)
    -> resample 8kHz to 16kHz (rubato crate, FftFixedInOut for clean 1:2 ratio)
    -> feed to VAD (silero-vad-rs crate, ~1ms per 32ms frame)
    -> on speech end: collect buffered PCM, encode as WAV, POST to Tinfoil STT
    -> get transcript text

Tinfoil LLM response text
    -> POST to Tinfoil TTS -> get PCM 24kHz WAV back
    -> strip WAV header, extract raw PCM samples
    -> resample 24kHz to 8kHz (rubato)
    -> PCM i16 to mulaw encode (law-encoder)
    -> base64 encode
    -> send as media message to Twilio WebSocket
```

#### 1d. Conversation State Machine

```rust
enum ConversationState {
    Listening,           // Receiving audio, VAD monitoring
    Processing,          // User stopped speaking, running STT->LLM->TTS
    Speaking,            // Playing audio back to caller
    WaitingForPlayback,  // Audio sent, waiting for mark confirmation
}
```

Track:
- Audio buffer (PCM samples collected during speech)
- Conversation history (Vec of messages for LLM context)
- Stream SID (from Twilio start message)
- User context (loaded at connection start)
- Call start time (for billing)

### Phase 2: Tool Calling

#### 2a. Refactor Tool Handlers

Extract tool logic from HTTP handlers into direct async functions:

```rust
// New file: backend/src/api/voice_tools.rs

pub async fn execute_tool(
    state: &AppState,
    user_id: i32,
    tool_name: &str,
    arguments: &Value,
) -> Result<String, anyhow::Error> {
    match tool_name {
        "get_weather" => execute_weather(state, user_id, arguments).await,
        "fetch_emails" => execute_fetch_emails(state, user_id).await,
        "send_sms" => execute_send_sms(state, user_id, arguments).await,
        "send_chat_message" => execute_send_chat(state, user_id, arguments).await,
        "fetch_recent_messages" => execute_fetch_recent(state, user_id, arguments).await,
        "fetch_chat_messages" => execute_fetch_chat(state, user_id, arguments).await,
        "search_chat_contacts" => execute_search_contacts(state, user_id, arguments).await,
        "cancel_message" => execute_cancel_message(state, user_id).await,
        "respond_to_email" => execute_respond_email(state, user_id, arguments).await,
        "send_email" => execute_send_email(state, user_id, arguments).await,
        "fetch_specific_email" => execute_fetch_specific_email(state, user_id, arguments).await,
        "perplexity_search" => execute_perplexity(state, user_id, arguments).await,
        "firecrawl_search" => execute_firecrawl(state, user_id, arguments).await,
        "create_task" => execute_create_task(state, user_id, arguments).await,
        "end_call" => Ok("__END_CALL__".to_string()),
        _ => Err(anyhow::anyhow!("Unknown tool: {}", tool_name)),
    }
}
```

Each function extracts the same logic from the existing HTTP handlers but without the HTTP boilerplate (no header validation, no query param extraction).

#### 2b. LLM Tool Definitions

Build tool definitions in OpenAI format for the Tinfoil LLM request:

```json
{
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "get_weather",
        "description": "Get weather for a location",
        "parameters": {
          "type": "object",
          "properties": {
            "location": {"type": "string"},
            "units": {"type": "string", "enum": ["metric", "imperial"]},
            "forecast_type": {"type": "string"}
          },
          "required": ["location"]
        }
      }
    }
    // ... same for all other tools
  ]
}
```

#### 2c. Tool Call Flow in Pipeline

When LLM returns `finish_reason: "tool_calls"`:
1. Parse tool_calls from response
2. Execute each tool via `execute_tool()`
3. Append tool results to conversation history
4. Call LLM again with tool results
5. Repeat until LLM returns a text response (or end_call)
6. Send final text response to TTS

### Phase 3: Barge-In and Interruption Handling

- Run VAD continuously on inbound audio, even during Speaking state
- When user speech detected during playback:
  1. Send `clear` message to Twilio (stops audio immediately)
  2. Cancel any in-flight TTS requests
  3. Transition to Listening state
  4. Start collecting new utterance
- Track marks to know what user actually heard vs what was cut off
- Optionally trim conversation history to only include what was spoken before interruption

### Phase 4: Latency Optimizations

1. **Sentence-level TTS**: As LLM streams tokens, detect sentence boundaries (`.`, `!`, `?`). Send each sentence to TTS independently. Start playing first sentence while second is still being synthesized.

2. **Filler audio**: While waiting for STT+LLM (the slow part), play a brief filler sound or "hmm" to signal processing. Natural conversational cue.

3. **Warm connections**: Keep HTTP connections to Tinfoil alive (connection pooling via reqwest).

4. **Explore voxtral-small-24b**: This is an audio-capable LLM on Tinfoil. Could potentially receive audio directly and respond, skipping separate STT. Would dramatically cut latency if it works well enough. Test this.

### Phase 5: Web Calls (Browser)

Replace the ElevenLabs WebSDK signed-URL flow:
- New WebSocket endpoint for browser clients
- Frontend captures mic audio via WebAudio API
- Sends PCM audio over WebSocket to server
- Same pipeline on server side
- Receives audio back over WebSocket, plays via WebAudio

### Phase 6: Billing and Cleanup

- Reuse existing credit check/deduction logic
- Log call start/end to message_history (same [CALL_START]/[CALL_END] markers)
- Log usage to usage_log table
- Remove ElevenLabs code and env vars once fully migrated

## New Files to Create

| File | Purpose | ~Lines |
|------|---------|--------|
| `backend/src/api/voice_pipeline.rs` | WebSocket handler, audio processing, conversation loop | 800-1000 |
| `backend/src/api/tinfoil_client.rs` | HTTP client for Tinfoil STT/LLM/TTS APIs | 300 |
| `backend/src/api/voice_tools.rs` | Refactored tool execution (direct calls, no HTTP) | 400 |

## Files to Modify

| File | Change |
|------|--------|
| `backend/src/main.rs` | Add routes: `/api/voice/incoming` (TwiML), `/api/voice/ws` (WebSocket) |
| `backend/Cargo.toml` | Add deps: silero-vad-rs, law-encoder (or audio-codec-algorithms), rubato, base64 |

## Rust Crates Needed

| Crate | Purpose |
|-------|---------|
| `silero-vad-rs` | Voice activity detection (~1ms per frame, handles 8kHz) |
| `law-encoder` | mulaw G.711 encode/decode (zero deps, no allocations) |
| `rubato` | Audio resampling 8kHz<->16kHz<->24kHz (real-time safe) |
| `base64` | Encode/decode Twilio media payloads (likely already a dep) |

Axum already has WebSocket support (`axum::extract::ws`). reqwest is already available for HTTP to Tinfoil.

## Environment Variables

New:
- `TINFOIL_API_KEY` - API key for Tinfoil inference

Existing (reused):
- `VOICE_SECOND_COST` - Credits per second
- `CHARGE_BACK_THRESHOLD` - Minimum credits

Eventually removed:
- `ELEVENLABS_API_KEY`
- `ELEVENLABS_SERVER_URL_SECRET`
- `ELEVENLABS_WEBHOOK_SECRET`
- `AGENT_ID` / `ASSISTANT_ID`
- `US_VOICE_ID`, `FI_VOICE_ID`, `DE_VOICE_ID`

## System Prompt (Adapt for Tinfoil LLM)

Reuse the existing system prompt from `elevenlabs_agent_config.json` (lines 104-105) with these changes:
- Remove ElevenLabs-specific instructions (tool_call_sound, skip_turn)
- Keep all personality, tone, guardrails, tool usage instructions
- Keep dynamic variable interpolation ({{name}}, {{timezone}}, etc.)
- Add instruction about response brevity for latency (keep responses short, 1-2 sentences)

## Key Risks and Mitigations

| Risk | Mitigation |
|------|-----------|
| 3-4s latency (mouth-to-ear) | Sentence-level TTS, filler audio, test voxtral-small-24b |
| STT is batch-only (no streaming) | Accept for now; explore voxtral as combined audio+LLM |
| TTS may not stream chunks | Send per-sentence, overlap with next sentence generation |
| VAD false positives on phone noise | Tune Silero thresholds for telephony; add energy pre-filter |
| Tool calls add extra LLM round-trip | Acceptable - same as current ElevenLabs behavior |

## Testing Strategy

1. **Unit test** audio conversion: mulaw<->PCM, resampling
2. **Integration test** with a real call: ngrok + Twilio to local dev server
3. **Latency benchmarking**: measure each pipeline stage independently
4. **Compare** side-by-side with ElevenLabs call quality

## Start Command

After implementing Phase 1:
```bash
cd backend && cargo run --bin backend
# Then configure Twilio to point incoming calls to:
# https://your-ngrok-url/api/voice/incoming
```
