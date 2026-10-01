# Shutdown analytics export

While the backend and PostgreSQL are running, sign in as an admin, open `/admin`,
and select **Download all analytics (.json)**. The authenticated endpoint is
`GET /api/admin/exports/analytics`. It exports all retained history with no date
filter or row limit. A new tab can show an authentication/database error; an
error response is not an archive. Sign in again and retry if the session expired.

The file contains `schema_version`, `datasets` (named arrays), and `manifest`.
Each dataset's manifest entry explains its meaning and includes a row count,
timestamp field, and earliest/latest recorded timestamp. Times are Unix seconds
in UTC. JSON preserves nulls, numeric types, currency labels, signed prices and
exact email strings without spreadsheet formula interpretation.

## Included records

- Account and waitlist emails; user IDs; current plan, country, balances and BYOT
  status; Stripe and Metronome customer references for financial reconciliation.
- AI calls with models, providers, callsites, input/output/cached token counts,
  recorded provider costs, projected customer charges, and typed pricing snapshots.
- SMS status/prices/currencies/direction/fallback metadata and legacy activity,
  credit deductions, processing time and call duration.
- Metronome usage events (including pending/failed events), usage intents,
  billing-account state, billing webhook processing metadata and latest refund state.
- Estimated bridge bandwidth, bridge disconnections, email-processing counts,
  Light Tool trial counters/run metadata, and agent action counts/statuses.
- Latest saved AI model and country price catalogs.

No content is exported: messages, encrypted bodies, prompts, responses,
transcripts, images, phone numbers, contacts, mailbox identifiers, credentials,
free-form error text and arbitrary metadata are excluded. SQL uses explicit
column allowlists. Legacy activity labels are restricted to known application
categories: this column also stored free-form admin email subjects. Unknown
labels become `other_redacted`, with `activity_type_redacted: true`; numeric
usage and timestamps remain intact. Pricing JSON is deserialized to known types and reserialized
so unexpected embedded fields cannot pass through; invalid snapshots become
null with an `invalid` export status. The email file is still personal data and
should be stored with appropriate access controls.

## Verify before shutting down

Save the download outside the server. Check that it parses and every manifest
count matches the actual array, for example:

```sh
python3 - /path/to/lightfriend-analytics-TIMESTAMP.json <<'PY'
import json, sys
with open(sys.argv[1], encoding="utf-8") as source:
    archive = json.load(source)
assert archive["schema_version"] == 1
assert archive["manifest"]["complete"] is True
assert set(archive["datasets"]) == {d["name"] for d in archive["manifest"]["datasets"]}
for dataset in archive["manifest"]["datasets"]:
    rows = archive["datasets"][dataset["name"]]
    assert len(rows) == dataset["row_count"], dataset["name"]
    print(dataset["name"], len(rows), dataset["earliest_timestamp"], dataset["latest_timestamp"])
print("Archive verified")
PY
```

Check counts/date ranges against what you expect. An empty dataset really means
no retained rows, not a skipped query: any query failure fails the entire
download. The backend builds the complete file before returning it, sends its
content length, and uses a read-only repeatable-read transaction for consistency.
A database cursor and temporary file keep server memory bounded. The temporary
file is automatically removed, including on failure or interrupted download.
No export is published or saved as a permanent server artifact.

Keep an initial archive now, then export again after the last traffic and delayed
provider callbacks/billing reconciliation have finished, before stopping the
backend/database. Keep the final archive separately from earlier snapshots;
each download is a full snapshot, so concatenating downloads double-counts usage.

## Limits on profit analysis

Save Stripe payments, invoices, refunds and processing fees, Metronome invoices,
and infrastructure/provider invoices separately. The application does not store
a complete cash-revenue or vendor-cost ledger. These external records are needed
for profit calculations, including fixed hosting costs and number rental.

`llm_usage.provider_cost_usd` is a saved cost, while `customer_cost_usd` is a
projected charge. `billing_usage.cost_microusd / 1_000_000` is customer usage
queued/submitted for billing, possibly capped. It is not proof of cash collection.
Legacy `credits` are recorded deductions, not verified vendor costs or revenue.
Do not sum overlapping AI/SMS/legacy/billing ledgers as separate costs.

SMS prices retain their original signs and currencies. A row is a logged message,
not a segment; segment counts are not stored here. Null means unknown, not zero.
Old AI rows have no saved cost and are intentionally not repriced at today's rates.
Refund records preserve only the latest credit-pack/refund state, not a full ledger.
Current plans/BYOT settings cannot establish historical plans or who paid old
provider charges. User signup dates and subscription-change history are not stored.
Deleted or pruned events cannot be recovered by this export. Historical rows are
not inner-joined to users, so retained usage without a surviving account is preserved.
