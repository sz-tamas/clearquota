# Agent Guide

## Product boundary

This is a local-first Rust dashboard for developer-service usage. It stores provider configuration and sanitized usage history in SQLite. It is not a credential store or a hosted service.

The non-negotiable rule is: secret values are never persisted locally, logged, or returned to the browser. SQLite may contain only a Secret Manager identifier (a short secret name or a reference such as `projects/<project>/secrets/<secret>/versions/<version>`), never a secret value.

## Architecture

- Rust + Axum owns routing and application state.
- Askama renders pages and HTML fragments on the server.
- HTMX requests fragments and swaps them into the page; do not add a JSON SPA layer for ordinary dashboard actions.
- Tailwind is compiled with the standalone CLI through `mise run css:build`. Do not add npm, `package.json`, or `node_modules`.
- SQLite holds `providers` and sanitized `usage_snapshots`.
- Google Secret Manager access uses Application Default Credentials via `gcloud`.
- The HTTP server must bind only to `127.0.0.1`. The port comes from `USAGE_DASH_PORT` in `mise.toml`.

## Safe collector flow

1. Load provider metadata from SQLite.
2. Resolve its exact secret reference.
3. Call the provider API.
4. Normalize the result into `UsageSnapshot` metrics.
5. Save only the sanitized snapshot, then let the secret go out of scope.

Provider adapters belong in `src/providers/`; Google Cloud behavior belongs in `src/secrets/`. Keep the dashboard core independent of provider-specific response formats.

Resolved provider credentials use `secrecy::SecretString`. Pass that type through the resolver and provider interfaces; call `ExposeSecret::expose_secret()` only at the HTTP authorization boundary. Never clone, format, derive/debug-print, serialize, log, persist, or return a secret. Keep decoded intermediate buffers in `zeroize::Zeroizing`.

## Supported providers

- **OpenAI:** concurrently collect current calendar-month organization costs and spend alerts. Use the largest monthly `threshold_amount` (cents) as the spend limit. Preserve a partial cost snapshot if alert collection fails after costs succeed.
- **Apify:** use `totalUsageCreditsUsdAfterVolumeDiscount` and the user-configured monthly USD allowance to calculate spend, remaining credit, and percentage used.
- **Resend:** collect monthly and daily email metrics concurrently and calculate the configured quotas.

For future providers, do not advertise them as supported until their adapter, validation, display, and focused tests exist.

## Working conventions

- Use `mise run start` for local development and `mise run check` before handing off changes.
- Run `mise run test` for code changes. The GitHub Actions workflow runs `mise run check`, `mise run test`, `cargo clippy --locked -- -D warnings`, and `cargo audit` on pull requests and pushes to `main`.
- When templates change, rebuild `static/css/output.css` with `mise run css:build`.
- Do not commit `data/`, `.tools/`, Tailwind output, environment files, secret material, or private planning documents.
- Keep new dependencies narrow and justified. Prefer standard library facilities when they fit.
- Errors shown to users should identify the provider/action, but must never include credentials, authorization headers, Secret Manager output, or full provider response bodies.
- Safe logs may include provider endpoint paths, HTTP status, and normalized collection state; never log keys, headers, tokens, or response bodies.

## Current MVP scope

Manual provider configuration, manual refresh, normalized snapshots, and a simple dashboard are in scope. Scheduling, alerting, multi-user authentication, cloud deployment, collector isolation, and broad provider coverage are future work unless explicitly requested.
