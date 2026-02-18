# Tinfoil Kimi K2.5 Debugging Notes

## Problem
Lightfriend's LLM calls to Tinfoil (kimi-k2-5) fail with 500/400 errors when using tool calling with medium+ length content.

## Root Cause (confirmed via extensive curl testing)
Tinfoil's kimi-k2-5 model fails on **non-streaming** requests when tools + longer content are combined.
Streaming the exact same request works every time.

## Test Results (2026-02-18)

### Non-streaming + tools + long content: 0/10 success
```
All returned InternalServerError or BadRequestError (Pydantic validation error showing truncated JSON)
```

### Streaming + tools + long content: 10/10 success
```
All returned tool_calls successfully with finish_reason: tool_calls
```

### Non-streaming + tools + SHORT content: 10/10 success
```
Short content like "Alice emailed about Friday deadline. Standup 3PM." works fine non-streaming
```

### Non-streaming + NO tools + long content: 3/3 success
```
Without tools, long content works fine non-streaming
```

## Summary Table

| Content Length | Tools | Stream | Result |
|---------------|-------|--------|--------|
| Short | Yes | No | OK |
| Long | No | No | OK |
| Long | Yes | No | FAILS (~0-10% success) |
| Long | Yes | Yes | OK (100% success) |

## Error Details
Tinfoil returns two types of errors:
1. HTTP 200 with `{"error":{"message":"","type":"InternalServerError","param":null,"code":500}}`
2. HTTP 200 with Pydantic validation error showing truncated JSON:
   `Invalid JSON: EOF while parsing a string at line 1 column 24`
   This suggests Tinfoil's internal processing truncates/corrupts request data in non-streaming mode.

## Why Airmodus Project Works
The Airmodus project at ~/Airmodus/Server/ uses Tinfoil + kimi-k2-5 successfully because:
- `app/services/llm_service.py` ALWAYS sets `payload["stream"] = True` (line 30)
- Uses httpx streaming client to forward SSE chunks
- Tool calling works 100% of the time with streaming

## Fix for Lightfriend
The `chat_completion()` method in `backend/src/ai_config.rs` needs to:
1. When provider is Tinfoil, add `"stream": true` to the request
2. Collect SSE chunks from the streaming response
3. Reassemble into a complete ChatCompletionResponse
4. OpenRouter can stay non-streaming (works fine)

## Code State
- `ai_config.rs` has retry logic added (3 attempts with backoff) but retries alone don't fix the issue - streaming is required
- Provider selection was restored: users with `llm_provider="tinfoil"` preference will use Tinfoil
- The `chat_completion()` wrapper already handles Tinfoil quirks (missing fields, error-in-200-body)
- All call sites already migrated to `state.ai_config.chat_completion(provider, &request)`

## Rate Limiting Ruled Out
Tested with 10-second delays between requests: still 1/5 success for non-streaming + tools + long content.
This is not a rate limiting issue - it's a fundamental bug in Tinfoil's non-streaming tool call handling.

## Comprehensive Streaming Test Results (2026-02-18)

All streaming tests use `"stream": true` in the request.

| Test | Description | Result |
|------|-------------|--------|
| A | Long content + 1 tool + tool_choice:required | **5/5 OK** |
| B | Long content + 5 tools + tool_choice:required | **5/5 OK** |
| C | Rust-serialized payload (openai-api-rs) + stream | **5/5 OK** |
| D | 21 tools (full Lightfriend toolset) + long content | **5/5 OK** |
| E | No tool_choice set (like Airmodus does it) | **5/5 OK** |
| F | Very long content (6 emails, 5 whatsapp, 5 calendar) | **OK but slow (~30-60s due to reasoning)** |

**Streaming is 100% reliable across all test variations.**

## Important: Timeout Considerations
Kimi K2.5 is a reasoning model - it "thinks" before responding. With large inputs:
- Short inputs: ~5-10s
- Medium inputs: ~10-20s
- Large inputs (6 emails, 5 whatsapp, 5 calendar): ~30-60s
The chat_completion timeout needs to be generous (at least 60s, ideally 90s) for Tinfoil.

## Phala Cloud
Phala (phala.com) is an alternative/aggregator for TEE-based private AI. Hosts multiple providers including Tinfoil.
Has its own models and OpenAI-compatible API. Worth evaluating if Tinfoil streaming fix proves insufficient.
