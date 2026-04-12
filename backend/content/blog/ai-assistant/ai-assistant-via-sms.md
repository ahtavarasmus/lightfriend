---
title: "AI Assistant That Works Over SMS"
slug: "ai-assistant-via-sms"
description: "Use a full AI assistant from any phone by texting. No app, no internet needed on your phone."
date: "2026-04-12"
cluster: "ai-assistant"
cluster_hub: true
keywords:
  - "ai assistant sms"
  - "ai via text message"
  - "ai without smartphone"
  - "sms ai assistant"
tags:
  - "ai"
  - "sms"
  - "dumbphone"
schema_type: "Article"
faqs:
  - q: "Can I use AI without a smartphone?"
    a: "Yes. Lightfriend gives you a full AI assistant over SMS. Text it a question and get an answer back as a text message."
  - q: "What can the AI do?"
    a: "It can search the web, check the weather, set reminders, summarize your messages, send emails, and control connected devices like Tesla vehicles."
  - q: "Is it like ChatGPT over text?"
    a: "Similar, but with integrations. It can actually do things - send messages on your behalf, check your email, set reminders - not just answer questions."
related_slugs:
  - "ai-email-on-dumbphone"
  - "whatsapp-without-smartphone"
ai_summary: "Lightfriend provides a full AI assistant via SMS. Users text questions or commands to their Lightfriend number and receive responses as text messages. The AI can search the web, check weather, set reminders, send messages across platforms, and manage email."
---

## The Problem

AI assistants like ChatGPT, Claude, and Gemini require a smartphone or computer. You need an app or a browser. If you carry a basic phone, you have no access to AI.

This is frustrating because AI is genuinely useful for everyday tasks - looking things up, getting quick answers, managing your schedule. But the delivery mechanism (apps on smartphones) excludes anyone who has chosen a simpler phone.

## How Lightfriend Solves This

Lightfriend gives you an AI assistant that works entirely over SMS. You text it, it texts you back. Your phone doesn't need internet, apps, or a screen bigger than a matchbox.

Here's what you can actually do by sending a text:

### Search the Web

Text a question and Lightfriend searches the web in real time using Perplexity and returns a concise answer. No need to open a browser.

"What time does the pharmacy on Main Street close?"

### Check the Weather

Ask about the weather anywhere. Lightfriend returns current conditions or a forecast.

"What's the weather like in Helsinki tomorrow?"

### Set Reminders

Tell it to remind you about something and it will send you an SMS at the right time.

"Remind me to call the dentist tomorrow at 2pm"

### Send Messages Across Platforms

If you've connected WhatsApp, Signal, or Telegram, you can send messages through those platforms by texting Lightfriend.

"Tell Mom on WhatsApp that I'll be late for dinner"

The AI uses fuzzy name matching to find the right person. There's a 60-second delay before sending, so you can cancel if it picked the wrong contact.

### Manage Email

If you've connected an email account, you can read and reply to emails via SMS.

"What emails did I get today?"
"Reply to the email from John saying I'll review it tomorrow"

### Scan QR Codes

Send a photo of a QR code and Lightfriend reads it and tells you what it contains.

## What You Can't Do

Honesty matters. Here's what Lightfriend's AI cannot do over SMS:

- **No voice or video calls** through messaging platforms
- **No image generation** - it's text-only
- **No real-time conversations** - each text is a separate interaction
- **No app store** - you can't install new capabilities on the fly (though MCP server connections add extensibility)

## How It Actually Works

When you text your Lightfriend number:

1. Your SMS arrives at Lightfriend's server via Twilio
2. The AI processes your message, considering your profile and connected services
3. If an action is needed (like sending a WhatsApp message), the AI executes it
4. The response comes back as an SMS to your phone

The entire system runs inside a sealed computing environment (AWS Nitro Enclave). Your conversations are processed inside this enclave and the code is open source, so you can verify how your data is handled.

## Who This Is For

- People who switched to a dumbphone but still want AI assistance
- Parents who want a simple phone for their kids but with AI capabilities
- Anyone who prefers texting over apps
- Travelers who need quick answers without roaming data
