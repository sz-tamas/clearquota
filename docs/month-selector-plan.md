# Dashboard month selector implementation plan

## Decision

The dashboard uses one UTC calendar-month selector for every provider card.
Each manual refresh fetches the current calendar month and the immediately
preceding calendar month. Older months are read exclusively from sanitized
SQLite data and never trigger provider API calls.

## Data model

- Keep `usage_snapshots` as the append-only refresh audit.
- Add a period-keyed `monthly_usage_snapshots` table containing only normalized,
  sanitized metrics.
- Identify a row by provider plus UTC `period_start`; store the exclusive
  `period_end` and refresh timestamp as well.
- Upsert current and previous months so late provider corrections replace the
  normalized result for that period without deleting older months.
- Backfill the latest existing refresh from each provider/month so preserved
  pre-feature snapshots remain available in the selector.
- Continue keeping OpenAI's sanitized daily cost and activity ledgers for chart
  and project aggregation.

## Collection

- Introduce a shared `UsagePeriod` value with exact UTC start/end boundaries.
- Change provider collectors to collect one supplied period.
- Refresh the current and previous periods independently for every provider.
- Save a successful period even if the other period fails; report the provider
  refresh as partial when only one succeeds.
- OpenAI: query costs and completion usage with the period's timestamps.
- Resend: query monthly metrics with the period's inclusive date range; collect
  today's metrics only for the current period.
- Apify: fetch the native billing cycles overlapping the requested month, retain
  their sanitized boundaries and after-discount totals as metadata, and derive
  the displayed UTC calendar-month usage from daily after-discount service
  entries.

## Dashboard

- Accept `?month=YYYY-MM` on the dashboard and provider-list fragment.
- Default to the current UTC month and reject future or malformed months.
- Render a shared previous/next month control above the provider grid.
- Read selected-period normalized data from SQLite.
- Query OpenAI ledger charts using the selected period boundaries.
- Hide current-day Resend quota details outside the current month.
- Show a clear no-data state when a provider has no stored row for the selected
  month.

## Verification

- Test January/year rollover and leap-year month boundaries.
- Test period upserts and preservation of older monthly rows.
- Test that historical dashboard reads use SQLite only.
- Test partial two-period refresh behavior.
- Run `mise run fmt`, `mise run test`, `mise run css:build`, and `mise run check`.
