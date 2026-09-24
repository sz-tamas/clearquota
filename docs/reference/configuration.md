---
title: Configuration reference
description: Environment variables, provider fields, defaults, and validation rules.
sidebar:
  order: 1
---

## Server environment

| Variable | Type and allowed values | Default | Effect |
| --- | --- | --- | --- |
| `USAGE_DASH_PORT` | Integer from 1 through 65535 | `3000` in the binary; `mise.toml` sets `5050` for source tasks | Local HTTP port. The host is always `127.0.0.1`. |
| `USAGE_DASH_DATABASE_PATH` | Filesystem path | `data/clearquota.sqlite3`, relative to the process working directory | SQLite database and migration target. The release launcher changes to its installation directory first. |

For example, `USAGE_DASH_PORT=5051 clearquota` starts the release launcher on another local port. There is no supported host environment variable or command-line flag.

## Google Cloud and provider fields

| Field | Valid values | Initial value or behavior |
| --- | --- | --- |
| Project identifier | 1–63 lowercase ASCII letters, digits, or hyphens | Entered during onboarding; changing it requires authentication again. |
| Provider type | `openai`, `apify`, or `resend` | Chosen when adding a provider. |
| Display name | Nonempty after trimming | Form suggests the provider's name; can be edited. |
| Secret name in add/edit forms | 1–255 ASCII letters, digits, `_`, or `-` | A short name resolves to `projects/<active-project>/secrets/<name>/versions/latest`. Secret values are never valid inputs. |
| Apify monthly credit allowance | Positive finite USD number | Stored as `0` until set; set it before refresh. |
| Resend plan | Nonempty text | Stored as empty until set. |
| Resend monthly and daily quotas | Positive integer email counts | Stored as `0` until set; set both before refresh. |
| OpenAI credit event | `purchase`, `refund`, or `adjustment`; valid date no later than today; positive finite USD amount; optional note | No entries until manually added. |

The provider settings form uses `0.01` as the displayed minimum and step for the Apify allowance and OpenAI credit amount. The server validates positive finite amounts. See the [OpenAI](../guides/openai.md), [Resend](../guides/resend.md), and [Apify](../guides/apify.md) guides for editing steps and key requirements.

The collector also recognizes legacy stored same-project references of the form `projects/<project>/secrets/<secret>` or `projects/<project>/secrets/<secret>/versions/<version>`. The current add/edit forms reject slashes, so enter a short secret name in the UI.

## Collection periods

The dashboard accepts a month in `YYYY-MM` form and rejects future months. Period boundaries use UTC. A missing month selects the current UTC month. Collection is manual and refreshes all configured providers for the selected month.
