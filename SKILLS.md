# Project Skills

## Add a provider integration

Use this when adding a usage source. OpenAI, Apify, and Resend are the currently supported examples; Neon and Upstash are not implemented.

1. Add an adapter in `src/providers/` implementing the shared `Provider` trait.
2. Receive the resolved credential only as `&secrecy::SecretString`; call `ExposeSecret::expose_secret()` only when constructing the provider authorization header. Never clone, format, serialize, debug-print, log, persist, or return it, and never add credential fields to a model, form, database query, or HTTP response.
3. Request only the provider's usage or billing endpoint and convert its response into normalized `Metric` values inside a `UsageSnapshot`.
4. Return concise, credential-free errors. Logs may include the endpoint path, HTTP status, and normalized outcome, but never a request body, response body, key, token, or authorization detail.
5. If one independent request succeeds and another fails, preserve the sanitized partial result when it remains meaningful to the dashboard; mark the snapshot partial rather than discarding it.
6. Register the adapter in `ProviderRegistry`, validate any provider-specific configuration in `src/router/providers.rs`, and add focused fixture-based tests containing no real secrets. Put new route DTOs in `src/dtos/providers.rs` and reusable validation in `src/utils/helper.rs`.

### Existing collector conventions

- OpenAI: request costs and spend alerts concurrently; choose the largest monthly `threshold_amount`, converting cents to USD.
- Apify: `totalUsageCreditsUsdAfterVolumeDiscount` is the spend source. The user-configured monthly USD allowance determines remaining credit and percentage used.
- Resend: make monthly and daily metric requests concurrently.

## Modify the dashboard UI

Use Askama templates in `templates/` and Tailwind classes in the markup. Add page and fragment handlers to the relevant `src/router/<context>.rs` file, with template DTOs in `src/dtos/<context>.rs`. Prefer an Axum route that returns a fragment plus HTMX attributes (`hx-get`, `hx-post`, `hx-target`, `hx-swap`) over adding client-side state or a JSON endpoint.

After template or style changes, run:

```bash
mise run css:build
```

The generated CSS is ignored; it is a local build artifact.

## Work with local configuration

`mise.toml` is the project entrypoint. Keep non-secret developer settings there, including `USAGE_DASH_PORT`. Never place API keys, service account JSON, or provider tokens in mise configuration or `.env` files.

## Verify changes

Run:

```bash
mise run check
mise run test
```

For changes that start the app, use `mise run start` and visit `http://127.0.0.1:5050` (or the configured `USAGE_DASH_PORT`). Direct `cargo run` defaults to port `3000`. Do not bind to public interfaces to simplify testing.

GitHub Actions must stay aligned with these checks: `mise run check`, locked tests, clippy with warnings denied, and dependency auditing on pull requests and `main` pushes. Dependabot keeps Cargo and GitHub Actions dependencies current weekly.
