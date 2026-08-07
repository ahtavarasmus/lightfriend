---
title: "Sending a Message by Voice or SMS, With Time to Cancel"
slug: "send-messages-with-a-cancel-window"
description: "How conversational sends from a phone are queued briefly before Lightfriend attempts delivery through a connected service."
date: "2026-08-01"
cluster: "messaging"
keywords:
  - "send WhatsApp by SMS"
  - "send message by voice assistant"
tags:
  - "sending"
  - "SMS"
schema_type: "Article"
related_slugs:
  - "messages-without-a-smartphone"
  - "when-a-connected-service-is-unavailable"
---

Voice requests are easy to mishear, and short SMS instructions can be ambiguous. A message should not leave instantly just because an assistant found a plausible recipient.

For conversational send and reply requests made through Lightfriend’s phone flow, supported WhatsApp, Signal, Telegram, and email actions are queued for about 60 seconds. Lightfriend returns a confirmation describing the intended action. Replying with the cancellation command during that window discards the queued action.

## Queued is not delivered

The confirmation means Lightfriend accepted and queued an attempt. It does not mean the external service delivered the message. When the delay ends, Lightfriend still has to reach the relevant bridge or email provider, and that step can fail if an account is disconnected or a provider is unavailable.

Some direct web or API actions can use different timing and confirmation behavior, so the cancellation window should not be generalized to every interface. Check the confirmation shown in the channel you are using.

The short delay is a modest safeguard, not an undo system after provider delivery. Its purpose is to catch a wrong recipient, transcription mistake, or sudden change of mind before the send attempt begins.
