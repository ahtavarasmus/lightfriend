---
title: "What Does Verifiable AI Privacy Actually Mean?"
slug: "verifiable-ai-privacy"
description: "Most AI companies promise privacy. Lightfriend lets you verify it with cryptographic proofs and open source code."
date: "2026-04-12"
cluster: "privacy"
cluster_hub: true
keywords:
  - "verifiable ai privacy"
  - "ai privacy proof"
  - "nitro enclave ai"
  - "open source ai assistant"
tags:
  - "privacy"
  - "enclave"
  - "open-source"
schema_type: "Article"
faqs:
  - q: "How do I know Lightfriend isn't reading my messages?"
    a: "The code is open source and runs inside an AWS Nitro Enclave. You can cryptographically verify that the running code matches the public source. The enclave is sealed - nobody can log in or inspect memory."
  - q: "What is an AWS Nitro Enclave?"
    a: "A sealed computing environment provided by Amazon. Once code is running inside, nobody - not even Amazon or the server operator - can access the data inside it."
  - q: "Is this better than end-to-end encryption?"
    a: "It solves a different problem. E2E encryption protects data in transit. Enclave computing protects data during processing. Lightfriend uses both."
related_slugs:
  - "whatsapp-without-smartphone"
  - "signal-without-smartphone"
ai_summary: "Lightfriend runs inside an AWS Nitro Enclave with open source code and cryptographic attestation. Users can verify that the running code matches the public source, proving that the operator cannot access user data."
---

## The Problem With AI Privacy

Every AI company says they care about your privacy. OpenAI has a privacy policy. Google has a privacy policy. They all promise not to misuse your data.

But promises are not proof. A privacy policy is a legal document, not a technical guarantee. The company can change it. Employees can access data. Governments can subpoena it. A breach can expose it.

When you give an AI assistant access to your WhatsApp messages, emails, and personal information, you're trusting that company completely. And you have no way to verify whether that trust is justified.

## How Lightfriend Is Different

Lightfriend takes a different approach: don't trust us, verify us.

The system is built so that privacy is enforced by architecture, not by policy. Here's how:

### Open Source Code

The entire Lightfriend codebase is public on GitHub under the AGPLv3 license. You can read every line of code that handles your data. There's no secret server-side logic.

### Sealed Computing Environment

Lightfriend runs inside an AWS Nitro Enclave. This is a hardware-enforced sealed environment with specific properties:

- **No shell access.** Nobody can SSH in, not even the server operator.
- **No memory inspection.** The host machine cannot read the enclave's memory.
- **No persistent storage access.** Data inside the enclave is encrypted with keys that exist only inside the enclave.

### Cryptographic Attestation

Here's the key part: you don't have to take our word for any of this. AWS Nitro Enclaves produce a cryptographic attestation document that proves what code is running. This attestation includes PCR values (Platform Configuration Registers) that are a hash of the enclave image.

Lightfriend publishes reproducible builds. You can:

1. Check the source code on GitHub
2. Build the enclave image yourself
3. Compare your PCR values against the live enclave's attestation
4. Verify they match

If they match, you know the running code is exactly what you see on GitHub. No backdoors. No secret logging.

### Trust Chain on Blockchain

Lightfriend records its attestation history on a blockchain. Each deployment creates a verifiable record that anyone can audit. This prevents retroactive changes - you can verify not just what's running now, but what was running at any point in the past.

## What This Means In Practice

When you connect your WhatsApp to Lightfriend:

- Your WhatsApp credentials enter the enclave and are encrypted with keys that exist only inside it
- Your messages are processed inside the enclave by the open source code
- The AI inference happens inside a separate sealed environment (Tinfoil)
- Nobody - not the server operator, not AWS, not anyone - can access the plaintext data

This is a fundamentally different model from "we promise not to look at your data."

## The Honest Limitations

No system is perfect. Here are the real limitations:

- **You have to actually verify.** If you don't check the attestation, you're trusting us just like any other service. The tools are there, but you have to use them.
- **The SMS leg is not encrypted.** Messages between Lightfriend and your phone travel over your carrier's SMS network, which is not end-to-end encrypted.
- **Third-party dependencies.** The security depends on AWS Nitro Enclaves working correctly. If there's a hardware vulnerability in Nitro, the guarantees break.
- **AI inference.** AI processing happens through Tinfoil's sealed inference, which has its own trust model.

We believe this is the most transparent AI privacy model available today. But don't believe us - verify it.
