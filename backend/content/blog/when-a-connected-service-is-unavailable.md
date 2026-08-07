---
title: "What Happens When a Connected Service Is Unavailable?"
slug: "when-a-connected-service-is-unavailable"
description: "A practical look at disconnects, provider failures, retries, and what Lightfriend can and cannot confirm."
date: "2026-07-31"
cluster: "problems"
keywords:
  - "messaging service disconnected"
  - "WhatsApp bridge unavailable"
tags:
  - "reliability"
  - "connections"
schema_type: "Article"
related_slugs:
  - "keep-whatsapp-linked-device-connected"
  - "send-messages-with-a-cancel-window"
---

Lightfriend sits between your phone and several independent systems: your mobile carrier, messaging bridges, email servers, and the services themselves. A successful request to Lightfriend cannot make all of those systems permanently available.

If a connected service is offline or its credentials are no longer valid, fetching a conversation or sending a reply can fail. Lightfriend reports provider errors where the flow supports a synchronous result. Queued phone sends are different: the first confirmation means an attempt is scheduled, not that the provider delivered it.

## Preserve the distinction

There are three useful states: Lightfriend understood the request, Lightfriend attempted the provider operation, and the provider accepted or delivered it. Product wording should not collapse those states into “sent” unless the available response proves it.

Some ingestion paths retry or deduplicate provider events so a repeated webhook or bridge event does not automatically produce duplicate side effects. That helps with transient delivery behavior, but it is not a general availability guarantee.

When a service remains unavailable, reconnecting or reauthorizing the account may be necessary. WhatsApp also requires periodic activity in the native phone app to keep linked devices active. If a failure persists, contact support with the service and approximate time—never send credentials or message contents unless support specifically provides a secure need for them.
