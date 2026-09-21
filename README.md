# ClearQuota

[![CI](https://github.com/sz-tamas/clearquota/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/sz-tamas/clearquota/actions/workflows/ci.yml)
[![License: AGPL--3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/-Rust-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Tailwind CSS](https://img.shields.io/badge/-Tailwind%20CSS-06B6D4?logo=tailwindcss&logoColor=white)](https://tailwindcss.com/)
[![HTMX](https://img.shields.io/badge/-HTMX-3366CC?logo=htmx&logoColor=white)](https://htmx.org/)
[![Askama](https://img.shields.io/badge/-Askama-000000?logo=rust&logoColor=white)](https://github.com/askama-rs/askama)

ClearQuota is a localhost-only dashboard for checking developer-service usage. It stores provider setup and sanitized usage history in SQLite; API keys are fetched from Google Secret Manager only during a refresh and never stored, logged, or sent to the browser.

![ClearQuota screenshot](clearquota.png)

## What it does

- Manually configure, edit, delete, and refresh providers; browse stored snapshots by UTC calendar month and review refresh run logs.
- Connect to Google Cloud with Application Default Credentials and use a Secret Manager secret name or full reference for each provider.
- Track OpenAI organization spend, spend-alert limits, tokens, requests, prompt-cache rate, project spend, and manually recorded prepaid-credit events.
- Track Apify discounted monthly credit use against a configured USD allowance.
- Track Resend monthly and daily sent-plus-received email usage against configured quotas.

Supported providers: **OpenAI, Apify, and Resend**.

## Start

Prerequisites: [mise](https://mise.jdx.dev/), the [Google Cloud CLI](https://cloud.google.com/sdk), and IAM access to the Secret Manager secrets you intend to use.

```bash
mise run install
mise run start
```

Open `http://127.0.0.1:5050`. The first-run flow asks for a Google Cloud project and starts Google authentication; grant the signed-in identity access only to the intended secrets. Add providers with a secret name (such as `OPENAI_ADMIN_KEY`) or a full Secret Manager reference—never paste an API key into ClearQuota.

The database is `data/clearquota.sqlite3` by default; set `USAGE_DASH_DATABASE_PATH` to use another non-secret path. Set `USAGE_DASH_PORT` to change the port.

## Development

```bash
mise run dev
mise run check
mise run test
```

Tailwind uses its standalone binary: `mise run install` puts it in `.tools/`, and `mise run css:build` rebuilds the CSS. No `package.json` or `node_modules` is required.

## Security

- The server binds only to `127.0.0.1`.
- SQLite contains Secret Manager identifiers and sanitized metrics, never provider credential values.
- Credentials are held transiently as `secrecy::SecretString`, zeroized after use, and exposed only to make the provider authorization request.
- Google ADC is local to `gcloud` and separate from provider credentials.

Use only accounts and secrets you are authorized to access. Before using production credentials, run `mise run check` and `mise run test`, and review the relevant IAM grants.
