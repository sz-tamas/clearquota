# ClearQuota

[![CI](https://github.com/sz-tamas/clearquota/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/sz-tamas/clearquota/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/-Rust-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Tailwind CSS](https://img.shields.io/badge/-Tailwind%20CSS-06B6D4?logo=tailwindcss&logoColor=white)](https://tailwindcss.com/)
[![HTMX](https://img.shields.io/badge/-HTMX-3366CC?logo=htmx&logoColor=white)](https://htmx.org/)
[![Askama](https://img.shields.io/badge/-Askama-000000?logo=rust&logoColor=white)](https://github.com/askama-rs/askama)

A secure localhost-only dashboard for provider usage. It persists provider metadata and normalized usage snapshots in SQLite; provider credential values are fetched only when a refresh runs and are never stored in the database or sent to the browser.

![ClearQuota screenshot](clearquota.png)

## Prerequisites

- [mise](https://mise.jdx.dev/)
- Google Cloud CLI authenticated with Application Default Credentials: `gcloud auth application-default login`
- IAM access restricted to the specific Secret Manager secrets used by this dashboard

## Start

```bash
mise run install
mise run start
```

`mise run start` opens `http://127.0.0.1:5050`, as configured in `mise.toml`. Direct `cargo run` defaults to `http://127.0.0.1:3000`. Set `USAGE_DASH_PORT` to choose another local port.

The database defaults to `data/clearquota.sqlite3`; override that non-secret path with `USAGE_DASH_DATABASE_PATH` if desired.

`mise run install` downloads the Tailwind standalone binary to `.tools/`. No `package.json` or `node_modules` is used. `mise run css:build` recompiles `static/css/output.css` from `static/css/input.css`.

For Rust development with automatic rebuilds and server restarts, run:

```bash
mise run dev
```

## Adding a provider

Enter a reference in this exact form:

```text
SECRET_ID
```

You can also enter just the Secret Manager secret name (for example, `OPENAI_ADMIN_KEY`); the dashboard expands it to the active Google project and `versions/latest`. Enter a secret reference, never the provider API key itself. The dashboard resolves that reference only while refreshing the provider.

### OpenAI

Use an OpenAI Admin API key. A refresh requests organization costs and spend alerts concurrently, then shows the current calendar-month spend against the largest monthly spend-alert threshold. If the spend-alert request fails after costs are collected, the cost snapshot is retained and marked as partial.

### Apify

Enter the plan's included monthly credit allowance in USD when creating or editing the provider (for example, `19` for a $19 allowance). The dashboard uses `totalUsageCreditsUsdAfterVolumeDiscount` and calculates:

- Usage: discounted monthly spend / allowance
- Remaining: allowance − discounted monthly spend
- Used: discounted monthly spend / allowance × 100

### Resend

Enter the plan label plus monthly and daily email quotas. The dashboard collects sent and received email totals and displays quota usage, remaining allowance, and the percentage used.

### Planned integrations

- Cloudflare
- Neon Cloud
- Upstash
- Google Cloud (including expanded Secret Manager support)
- AWS
- Postmark
- Claude
- Gemini
- GitHub
- GitLab
- ...

## Security boundary

- Localhost only: the server binds to `127.0.0.1`.
- Provider keys never enter SQLite, browser responses, configuration, environment variables, or logs; only Secret Manager identifiers and sanitized metrics are persisted.
- Resolved keys use `secrecy::SecretString` and `zeroize`, and are exposed only for the transient provider authorization request. Google ADC is managed locally by `gcloud` and is separate from provider credentials.

### Security comparison

| Risk / property | ClearQuota | Other local credential-storing dashboard |
| --- | --- | --- |
| Persistent provider secrets on disk | **No** | **Yes**, commonly encrypted in an OS keyring |
| Provider secret present when app is idle | **No** | **Yes**, persisted locally |
| Provider secret present while fetching | **Yes, transiently** | **Yes, after decrypting** |
| Memory cleanup after use | **Explicit zeroization** with `secrecy` / `zeroize` | Depends on implementation |
| Local-only execution | **Yes** | Often yes |
| Third-party server sees credentials | **No** | Typically no |
| Secret source | Google Secret Manager | OS keyring |
| App needs raw provider key stored locally | **No** | **Yes** |
| Theft of app data directory | Stats/meta only; no provider credentials | Credential ciphertext and/or keyring references may exist |
| Theft of OS keyring | Not enough to obtain provider keys that exist only in Google Secret Manager | May expose stored provider credentials |
| Runtime process compromise | Can capture a key during a fetch | Can capture a key whenever decrypted or used |
| Memory inspection | Same fundamental limitation during active use | Same fundamental limitation during active use |
| Post-fetch memory residue | **Mitigated by zeroization** | Depends on handling |
| Credential rotation | Managed centrally in Google Secret Manager | Must update the locally stored secret |
| Multi-device credential consistency | Naturally centralized | Separate local keyring state per machine |

### Authorization and credential disclaimer

This is a local tool run by you, for accounts and secrets you are authorized to use. No dashboard operator, maintainer, hosted service, or browser user is sent your credential value, and the application does not display, persist, or log it. The credential is retrieved locally from the Secret Manager reference you choose and is sent only as a transient HTTPS authorization header to the provider you configured. You are responsible for granting Google IAM access only to the intended secrets and for using provider credentials with the permissions you intend.

Before using production credentials, run `mise run check` and `mise run test`, then review IAM grants and provider-specific response handling.
