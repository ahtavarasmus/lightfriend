# Stripe Metronome billing cutover

Lightfriend keeps its existing Stripe subscription and base price. Metronome
owns usage metering, the monthly $25 allowance, overage rating, usage invoices,
and threshold collection. Do not add the base subscription charge to the
Metronome package; Stripe already collects it.

## Metronome configuration

Create these objects in the Metronome sandbox first:

1. Connect the existing Stripe account.
2. Create the `lightfriend_usage` billable metric. It must sum the numeric
   `cost_usd` event property.
3. Create a usage product priced at $1 per unit of that metric. One event with
   `cost_usd: 0.013` must therefore rate to $0.013.
4. Create a package with alias `lightfriend-monthly` containing:
   - a non-rollover $25 recurring credit every month;
   - weekly usage statements;
   - a $10 spend-threshold configuration using Stripe `INVOICE` collection;
   - the spend threshold present but **disabled by default**.
5. Configure global contract-credit/commit balance alerts at $5 and $0 and send
   all Metronome webhooks to `https://<backend>/api/metronome/webhook`. The $5
   alert sends a short warning; only the $0 alert changes entitlement.
6. Create a credit product for imported legacy balances and set its ID plus the
   USD credit type ID in the environment.

The application toggles the existing contract threshold with
`update_spend_threshold_configuration.is_enabled`. Enabling it immediately
allows post-allowance usage and triggers collection each time outstanding
overage reaches $10. Weekly statement finalization collects any smaller
remainder first.

## Deployment order

1. Deploy the database migration and application with
   `METRONOME_BILLING_ENABLED=false`.
2. Configure the sandbox objects above and all `METRONOME_*` values.
3. Verify a sandbox customer, $25 recurring credit, usage rating, $10 threshold
   invoice, weekly invoice, failed payment, webhook signature, and duplicate
   event handling.
4. Repeat the configuration in production.
5. Confirm every subscribed user with a non-zero legacy purchased-credit
   balance can be imported. Do not enable the cutover if either legacy credit
   environment variable is missing.
6. Set `METRONOME_BILLING_ENABLED=true` and deploy.

On activation, the background migration provisions all hosted subscribers. It
uses `lightfriend-user-<id>` as an idempotent ingest alias, links the existing
Stripe customer, creates the package contract, and imports the old purchased
balance once. For users with an active legacy allowance window, it also emits a
single idempotent cutover event for the portion of that month's $25 already
used, so migration does not grant a second full allowance. It copies the saved
Stripe payment method (falling back to the active subscription's default) to
the customer-level invoice default when needed. This requires no action from
users whose saved card is valid. Users with a missing, expired, or
authentication-required payment method must update it through the Stripe
customer portal before they can enable overage.

The cutover also migrates the legacy `charge_when_under` preference exactly
once. Users who previously enabled automatic overage-credit purchases have the
Metronome spend threshold enabled automatically after a reusable payment method
is confirmed. All other existing users and every new user start with overage
disabled. The migration marker prevents a later user-initiated disable from
being overwritten by the old legacy flag.

New hosted subscribers are provisioned from the subscription webhook, with the
five-minute migration job as a retry backstop. Usage is written transactionally
to `billing_usage_events` and sent every ten seconds. Metronome's transaction
ID deduplication plus the local primary key makes provider status retries safe.

## Cutover behavior

When the flag is enabled:

- Metronome is the only balance and allowance ledger.
- The old one-time purchase and automatic top-up endpoints return `410 Gone`.
- Local `credits` and `credits_left` values are not decremented.
- The dashboard shows one overage control, off by default.
- A failed or action-required payment disables overage and local entitlement.
- A zero-balance alert disables usage unless overage is enabled and the payment
  method is ready.

Keep the old columns during the first release for rollback and audit only. They
must not be reconciled against Metronome or displayed once the cutover flag is
enabled. Remove them in a later migration after the rollback window closes.

## Operational checks

Monitor pending/failed rows in `billing_usage_events`, failed rows in
`billing_accounts`, Metronome integration issues, Stripe invoice failures, and
webhook signature failures. An outbox backlog is a billing incident: usage may
continue from the cached entitlement, but events remain durable and retry with
exponential backoff.

## Metered events

Every event is pre-rated in USD and receives the standard 30% usage margin
before it is queued. Metronome remains the only allowance and overage ledger.

Text inference uses the provider's returned prompt/completion token counts.
Current defaults are Tinfoil Kimi K2.6 at $1.50/M input and $5.25/M output,
Tinfoil Gemma 4 31B at $0.40/M input and $1.00/M output, NEAR GLM 5.1 at
$0.85/M input and $3.30/M output, and NEAR Gemma 4 31B at $0.13/M input and
$0.40/M output. The NEAR rates can be overridden through the documented env
values when its catalog price changes.
Persisted conversation history is also capped at roughly 10k tokens (40k
characters, newest first) so an active user's old 48-hour history cannot make
every new dashboard or Light Tool message disproportionately expensive.

OpenAI Realtime is calculated from detailed usage, not call duration: text
input $4/M, audio input $32/M, cached input $0.40/M, text output $24/M, and
audio output $64/M. The `response.id` is used as the idempotent transaction ID.

- `web_chat`: all Tinfoil/NEAR/OpenRouter model rounds used by one dashboard
  interaction, including verifier retries and SMS-length condensation.
- `light_tool`: the same model-cost calculation as dashboard chat, aggregated
  across the full Light Tool interaction.
- `web_voice_ai`: OpenAI Realtime text, audio, and cached tokens from each
  `response.done`; there is no Twilio leg.
- `phone_voice_ai`: the same OpenAI Realtime token calculation for hosted phone
  calls.
- `twilio_voice`: the hosted inbound/outbound Twilio phone leg. BYOT users pay
  Twilio directly and do not receive this event.
- `twilio_sms`, `telnyx_sms`, and `sinch_sms`: actual carrier cost from the
  provider delivery callback.
- `rule_test`: the explicit rule-test action.

The dashboard reads the current credit balance and active segment end from
Metronome's customer-balance API. Stripe's customer portal continues to show
payment methods, finalized invoices, and collected overage; it is not the live
quota display.
