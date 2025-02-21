
request:

curl https://api.groq.com/openai/v1/chat/completions \
-H "Content-Type: application/json" \
-H "Authorization: Bearer $GROQ_API_KEY" \
-d '{
  "model": "llama-3.3-70b-versatile",
  "messages": [
    {
      "role": "user",
      "content": "What'\''s the weather like in Boston today?"
    }
  ],
  "tools": [
    
  ],
  "tool_choice": "auto"
}'


response with tool call:

"model": "llama-3.3-70b-versatile",
"choices": [{
    "index": 0,
    "message": {
        "role": "assistant",
        "tool_calls": [{
            "id": "call_d5wg",
            "type": "function",
            "function": {
                "name": "ask_perplexity",
                "arguments": "{\"message\": \"What's the weather in Tampere 19.2.2024?\"}"
            }
        },
        {
            "id": "call_d5wf",
            "type": "function",
            "function": {
                "name": "other_function",
                "arguments": "{\"param\": \"something\"}"
            }
        }
        ]
    },
    "logprobs": null,
    "finish_reason": "tool_calls"
}],

