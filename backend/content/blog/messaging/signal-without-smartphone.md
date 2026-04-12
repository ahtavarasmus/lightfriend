---
title: "How to Use Signal Without a Smartphone"
slug: "signal-without-smartphone"
description: "Send and receive Signal messages from any basic phone via SMS using Lightfriend's encrypted bridge."
date: "2026-04-12"
cluster: "messaging"
keywords:
  - "signal without smartphone"
  - "signal on dumbphone"
  - "signal on flip phone"
  - "signal sms bridge"
tags:
  - "signal"
  - "messaging"
  - "dumbphone"
  - "privacy"
schema_type: "HowTo"
estimated_time: "PT5M"
faqs:
  - q: "Can I use Signal on a dumbphone?"
    a: "Not natively. But Lightfriend bridges Signal to SMS, so you can send and receive Signal messages from any phone."
  - q: "Is this still encrypted?"
    a: "Signal messages are end-to-end encrypted between Signal users. The bridge decrypts inside Lightfriend's sealed enclave to convert to SMS. Your messages are never visible to Lightfriend operators."
  - q: "Do my Signal contacts see my messages normally?"
    a: "Yes. They see your messages as regular Signal messages."
related_slugs:
  - "whatsapp-without-smartphone"
  - "telegram-without-smartphone"
hub_slug: "whatsapp-without-smartphone"
ai_summary: "Lightfriend bridges Signal to SMS. Any phone that can text can send and receive Signal messages. The bridge runs inside a sealed enclave so operators can't read your messages."
---

## The Problem

Signal is the gold standard for private messaging. But it only works on smartphones and desktops. If you carry a basic phone, you can't use Signal at all. This forces privacy-conscious people to choose between their values (using a minimal phone) and staying connected (using Signal).

## Why This Is Hard

Signal has no SMS fallback. There is no KaiOS app. There is no web-only mode. You need the Signal app on a smartphone, period. The Signal Foundation has no plans to change this.

If you switch to a dumbphone, you lose access to Signal entirely. Your contacts either switch to texting you (unencrypted) or you lose touch.

## How Lightfriend Solves This

Lightfriend runs an open-source Signal bridge inside a sealed computing environment. Here's how it works:

1. **Connect your Signal account** through the Lightfriend web dashboard. This registers Lightfriend as a linked device on your Signal account.
2. **Messages arrive as SMS.** When someone sends you a Signal message, Lightfriend converts it to SMS and sends it to your phone.
3. **Reply by text.** Your SMS reply goes back through Signal to the person who messaged you.
4. **Groups work.** You can read and reply to Signal group conversations.

## What Works

| Feature | Works? |
|---------|--------|
| Receive text messages | Yes |
| Send text messages | Yes |
| Group messages | Yes |
| Receive photos (as descriptions) | Yes |
| Voice/video calls | No |
| Disappearing messages | Messages are delivered then purged |

## Privacy Inside the Bridge

The obvious question: if Signal messages are end-to-end encrypted, doesn't a bridge break that?

Here's how Lightfriend handles it: the bridge runs inside an AWS Nitro Enclave, a sealed computing environment that nobody can access - not even the server operators. Your Signal credentials and decrypted messages exist only inside this enclave. The code is open source and cryptographically verifiable.

The SMS leg (from Lightfriend to your phone) travels over your carrier's network, which is not end-to-end encrypted. This is a real tradeoff. But for people who would otherwise not use Signal at all, this is strictly better than sending everything as plain SMS.

## What You Need

- Any phone with SMS
- A Lightfriend account
- A Signal account (requires a smartphone for initial setup)

After linking, the smartphone is no longer needed.
