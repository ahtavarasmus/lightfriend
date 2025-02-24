
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


response from paddle:
sub created:

PaddleWebhookPayload {
    event_type: "subscription.created",
    data: Data {
        subscription_id: "sub_01jmwmx66hxtmvrydq097fdkcm",
        customer_id: Some(
            "ctm_01jmsfnnqy5we78c13mtjxz6p9",
        ),
        status: Some(
            "active",
        ),
        next_billed_at: Some(
            "2025-03-24T18:53:27.114Z",
        ),
        items: Some(
            [
                SubscriptionItem {
                    price: Price {
                        id: "pri_01jmqk1r39nk4h7bbr10jbatsz",
                        unit_price: UnitPrice {
                            amount: "0",
                            currency_code: "USD",
                        },
                    },
                    product: Product {
                        id: "pro_01jmqjz3dps7d59m604tdenh88",
                        name: "lightfriend IQ",
                    },
                    status: "active",
                    quantity: 1,
                },
            ],
        ),
        custom_data: Some(
            Object {
                "user_id": Number(1),
            },
        ),
    },
}

