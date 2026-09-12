# AI usage pricing

Text AI prices are fetched on backend startup and every hour from each configured
provider's OpenAI-compatible `/models` endpoint (Tinfoil, NEAR, and OpenRouter).
The lookup key is the provider plus the exact requested model ID, including the
provider/model selected by fallback. New models in these catalogs need no code
change to their prices.

## Outages and defaults

Valid API rates are cached in memory and persisted in PostgreSQL's
`ai_model_prices` table. Startup loads saved rates before attempting a refresh.
An HTTP error, malformed response, or empty catalog keeps the last valid rates.
Invalid entries do not replace valid entries for other models. Cached prices
remain usable during an outage; their fetch timestamp is visible in admin.

If a model has never had a valid API price, `model_pricing::fallback_quote` uses
a built-in model default, then a generic provider default. These are estimates,
not guaranteed upper bounds. Admin identifies default prices explicitly. The
DeepSeek and Gemma defaults were checked against Tinfoil's public catalog on
2026-09-06; the DeepSeek fallback was carried over to `deepseek-v4-1-flash`
(from the deprecated `deepseek-v4-flash`) on 2026-09-11 pending a catalog
refresh confirming V4.1 Flash pricing. API rates always take precedence over
defaults.

`AI_USAGE_MULTIPLIER` sets the customer usage multiplier (default `1.30`, meaning
a 30% markup). Invalid values or values below 1 use the default. The old text
model-specific rate environment variables are superseded by the API catalog.
OpenAI realtime/audio pricing remains a separate configuration in
`usage_pricing.rs`; this catalog covers text AI requests.

## Usage records and admin

The migration `20260906120000_add_dynamic_ai_pricing` adds saved prices and
optional cost metadata to `llm_usage_logs`. New records capture input/output
tokens, cached input when supplied, provider cost, projected customer charge,
and a snapshot containing rates, source, fetch time, and markup. Later rate
changes do not recalculate these snapshots.

Both streaming parsers request and preserve cumulative usage metadata. Cached
input is a subset of input, not an additional token charge. OpenRouter's
reported `usage.cost` takes precedence when available and is not charged again
for estimated search/request fees. Without reported cost, Sonar Reasoning Pro
uses a $0.005 search estimate plus token prices. Missing cache details, search
estimates, and absent token counts are flagged as incomplete usage.

Historical records without snapshots are estimated at current catalog/default
prices at report time. Missing historical cache counts are treated as uncached;
those estimates can exceed the original invoice. No historical credits are
deducted or backfilled by this change.

Admin's AI section supports rolling 7/14/30/90-day windows, consistent totals by
provider/model, callsite, user, and UTC day, and a current-price table. Daily
averages divide by the full selected period, including days without activity.
30-day projections assume the same usage pace. Projected charges are not proof
of billing: existing billing paths still determine which usage is charged.
Carrier charges and voice costs are outside this text AI report. Failed attempts
without provider usage metadata cannot be accurately reconstructed from tokens.

## Validation

Run the migration against an isolated PostgreSQL database before testing:

```sh
diesel --config-file diesel_pg.toml migration run --database-url "$TEST_PG_DATABASE_URL"
cargo test --test billing_and_usage_tests dynamic_ai_pricing_test
cargo test --test billing_and_usage_tests usage_pricing_test
```

`TEST_PG_DATABASE_URL` must point to the isolated migrated database for the
catalog persistence test. The HTTP tests use local mock servers, not paid APIs.

Price sources:

- https://inference.tinfoil.sh/v1/models
- https://cloud-api.near.ai/v1/models
- https://openrouter.ai/api/v1/models
