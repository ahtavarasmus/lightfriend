---
title: "What Does a Verifiable Privacy Architecture Mean?"
slug: "verifiable-ai-privacy"
description: "How Lightfriend uses open-source code, hardware isolation, encrypted storage, and cryptographic attestation to make its production architecture inspectable."
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
  - q: "What parts of Lightfriend's privacy architecture can I inspect?"
    a: "The source code, reproducible build measurements, AWS-signed enclave attestation, public approval registry, and live trust-chain status are all available for independent inspection."
  - q: "What is an AWS Nitro Enclave?"
    a: "An isolated virtual machine provided by AWS. Nitro Enclaves have no persistent storage or direct external networking, and the parent instance is isolated from enclave CPU and memory."
  - q: "Is this better than end-to-end encryption?"
    a: "It solves a different problem. End-to-end encryption protects supported transport legs, while enclave computing addresses data during processing. Lightfriend's SMS leg is not end-to-end encrypted."
related_slugs:
  - "whatsapp-without-smartphone"
  - "signal-without-smartphone"
ai_summary: "Lightfriend publishes its source code and AWS-signed production attestation. Users can compare the reported enclave measurement with the public build and approval registry; attestation verifies deployment identity, not the absence of software bugs."
---

## The Problem With AI Privacy

Every AI company says they care about your privacy. OpenAI has a privacy policy. Google has a privacy policy. They all promise not to misuse your data.

Privacy policies describe commitments, but they do not expose the technical state of a running deployment. They are different from evidence about software identity, hardware isolation, encryption, and key release.

When you give an AI assistant access to messages and personal information, it helps to know which properties can be inspected and which still depend on trust.

## How Lightfriend Is Different

Lightfriend makes more of its architecture independently inspectable.

The system combines operational policy with technical controls. Here's how those controls are implemented:

### Open Source Code

The Lightfriend codebase is public on GitHub under the AGPLv3 license. You can inspect the application code that handles user data and build it independently.

### Sealed Computing Environment

Lightfriend runs inside an AWS Nitro Enclave. This is a hardware-enforced sealed environment with specific properties:

- **No shell access.** The enclave image exposes no SSH login or administrative shell.
- **Memory isolation.** Nitro Enclaves isolate allocated CPU and memory from the parent instance.
- **No persistent storage.** Protected application data written outside the enclave is encrypted with AES-256-GCM.

### Cryptographic Attestation

AWS Nitro Enclaves produce a signed attestation document containing PCR values (Platform Configuration Registers), which measure the enclave image.

Lightfriend publishes reproducible builds. You can:

1. Check the source code on GitHub
2. Build the enclave image yourself
3. Compare your PCR values against the live enclave's attestation
4. Verify they match

If they match, the measurement reported by the live enclave matches the measurement produced by the published build. This verifies deployment identity; it does not prove that the measured code is bug-free.

### Trust Chain on Blockchain

Lightfriend publishes approved enclave measurements to an Arbitrum smart contract. The contract's transaction history provides a public record of additions and removals from the approval registry.

## What This Means In Practice

When you connect your WhatsApp to Lightfriend:

- Stored WhatsApp credentials are encrypted by the application inside the enclave
- Your messages are processed inside the enclave by the open source code
- AI inference is sent to Tinfoil's confidential-computing environment, which publishes its own attestation evidence
- Marlin KMS evaluates enclave attestation and the public approval registry before releasing key material

These are concrete controls whose code, measurements, and live status can be inspected.

## The Honest Limitations

No system is perfect. Here are the real limitations:

- **Verification is an active step.** The tools expose evidence, but users or independent reviewers still need to inspect it.
- **The SMS leg is not encrypted.** Messages between Lightfriend and your phone travel over your carrier's SMS network, which is not end-to-end encrypted.
- **Optional voice calls use OpenAI Realtime.** Call audio and transcripts are processed outside Lightfriend's independently verifiable trust chain. OpenAI states that API data is not used for training unless the customer opts in, but default abuse-monitoring logs may retain Realtime customer content for up to 30 days.
- **Third-party dependencies.** The architecture depends on AWS Nitro Enclaves and Marlin KMS functioning as documented.
- **AI inference.** AI processing happens through Tinfoil's sealed inference, which has its own trust model.

The source, verification tooling, live attestation, and remaining trust assumptions are published so they can be evaluated directly.
