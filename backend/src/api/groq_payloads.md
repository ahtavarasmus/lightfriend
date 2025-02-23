
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

Paddle webhook payload: PaddleWebhookPayload {
    event_id: "evt_01jmt0kva7mhsn1c04ma5mwrc3",
    event_type: "subscription.created",
    occurred_at: "2025-02-23T18:20:20.679479Z",
    notification_id: "ntf_01jmt0kvfhxx5v1jpmh2496y1a",
    data: SubscriptionData {
        id: "sub_01jmt0kt7t6tvtd859bf4xc1n9",
        status: "active",
        customer_id: "ctm_01jmsfnnqy5we78c13mtjxz6p9",
        items: [
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
        currency_code: "USD",
        billing_cycle: BillingCycle {
            interval: "month",
            frequency: 1,
        },
        next_billed_at: "2025-03-23T18:20:19.571Z",
    },
}

