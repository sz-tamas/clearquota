[![Visit clearquota.app](cq_banner_lg.png)](https://clearquota.app)

[![CI](https://github.com/sz-tamas/clearquota/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/sz-tamas/clearquota/actions/workflows/ci.yml)
[![License: AGPL--3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/-Rust-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Tailwind CSS](https://img.shields.io/badge/-Tailwind%20CSS-06B6D4?logo=tailwindcss&logoColor=white)](https://tailwindcss.com/)
[![HTMX](https://img.shields.io/badge/-HTMX-3366CC?logo=htmx&logoColor=white)](https://htmx.org/)
[![Askama](https://img.shields.io/badge/-Askama-000000?logo=rust&logoColor=white)](https://github.com/askama-rs/askama)


---


Local-first dashboard for monitoring usage and quotas across developer services, with provider credentials resolved just-in-time from external secret storage and never persisted locally, logged, or sent to the browser.

## What it does

- Manually configure, edit, delete, and refresh providers; browse stored snapshots by UTC calendar month and review refresh run logs.
- Connect to Google Cloud with Application Default Credentials and use a Secret Manager secret name or full reference for each provider.
- Track OpenAI organization spend, spend-alert limits, tokens, requests, prompt-cache rate, project spend, and manually recorded prepaid-credit events.
- Track Apify discounted monthly credit use against a configured USD allowance.
- Track Resend monthly and daily sent-plus-received email usage against configured quotas.

Supported providers: **OpenAI, Apify, and Resend**.

## Getting started

Prerequisites: [mise](https://mise.jdx.dev/), the [Google Cloud CLI](https://cloud.google.com/sdk), and IAM access to the Secret Manager secrets you intend to use.

```bash
mise run install
mise run start
```

Open `http://127.0.0.1:5050`. The first-run flow asks for a Google Cloud project and starts Google authentication; grant the signed-in identity access only to the intended secrets. Add providers with a secret name (such as `OPENAI_ADMIN_KEY`) or a full Secret Manager reference—never paste an API key into ClearQuota.

The database is `data/clearquota.sqlite3` by default; set `USAGE_DASH_DATABASE_PATH` to use another non-secret path. Set `USAGE_DASH_PORT` to change the port.



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
