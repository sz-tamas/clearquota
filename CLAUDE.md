# Project Instructions

Build a secure, local-first provider-usage dashboard.

## Required stack

- Rust backend using Axum
- Askama templates for pages and reusable HTML fragments
- HTMX for server-driven interactions
- Tailwind compiled by the standalone CLI; no Node.js, npm, `package.json`, or `node_modules`
- SQLite for provider metadata and sanitized usage snapshots
- Google Cloud Secret Manager with Application Default Credentials for credentials

## Security requirements

- Never write credential values to SQLite, files, logs, errors, or browser responses.
- Store only Secret Manager identifiers (a short name or explicit version reference), never credential values. Normalize the identifier to the active project before resolving it.
- Keep secrets in memory only during collection.
- Keep resolved provider credentials in `secrecy::SecretString` and expose them only while constructing the provider authorization header. Use `zeroize::Zeroizing` for decoded intermediate buffers.
- Never clone, format, serialize, debug-print, log, persist, or return secrets.
- Never log authorization headers or raw provider responses that may contain sensitive data.
- Bind the web application only to `127.0.0.1`; read the port from `USAGE_DASH_PORT`.
- Keep Google IAM permissions limited to the required individual secrets.
- Treat the local machine and the ADC identity as trusted: the dashboard cannot protect against a user or process that already controls either one.

## Code organization

- `src/secrets/`: secret resolver abstraction and GCP implementation
- `src/providers/`: provider adapters that produce normalized metrics
- `src/database.rs`: SQLite schema access and persistence
- `src/web/`: Axum routes and Askama rendering
- `templates/`: Tailwind-marked server-rendered pages and fragments
- `static/css/input.css`: Tailwind source; rebuild with `mise run css:build`

Use the same collection path for manual refresh and future scheduling. Implement provider-specific work behind the shared provider interface rather than branching inside web routes.

## Current provider behavior

- OpenAI collects costs and spend alerts concurrently; use the largest monthly alert threshold and save a partial snapshot if only alerts fail.
- Apify uses discounted monthly usage and the configured USD allowance to calculate usage, remaining credit, and percentage used.
- Resend collects monthly and daily metrics concurrently and calculates configured quotas.

Neon and Upstash are not implemented collectors. Do not present a provider as functional until the adapter and UI are complete.

Before completing a change, run `mise run check` and `mise run test`; run `mise run css:build` whenever templates or Tailwind source changes. CI runs `mise run check`, `mise run test`, `cargo clippy --locked -- -D warnings`, and `cargo audit`.
