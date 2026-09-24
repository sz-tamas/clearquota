---
title: ClearQuota documentation
description: Start using the local dashboard to inspect developer-service usage and quotas.
sidebar:
  order: 1
---

ClearQuota is a local dashboard for usage from **OpenAI, Apify, and Resend**. It stores provider settings, sanitized usage snapshots, and refresh logs in SQLite. Provider credentials stay in Google Secret Manager: you enter a secret name or reference, and ClearQuota retrieves the value only when refreshing usage. The web server listens on `127.0.0.1`.

Start with [installation](getting-started/installation.md) and [connect Google Cloud](getting-started/first-configuration.md). Then follow the provider guide for [OpenAI](guides/openai.md), [Resend](guides/resend.md), or [Apify](guides/apify.md). Each covers its API key, the two setup steps in Providers, and the expected result. Finish with the [quickstart](getting-started/quickstart.md) to refresh the dashboard.

Once running, use the [daily workflow](guides/monitor-usage.md) and [history and local data](guides/history-and-data.md). Experienced users can jump to the [configuration reference](reference/configuration.md), [commands](reference/commands.md), [architecture](reference/architecture.md), or [troubleshooting](troubleshooting/common-issues.md).

Collection is manual. Alerts, scheduling, cloud deployment, and additional provider adapters are not part of the current application.
