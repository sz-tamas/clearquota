---
title: Monitor usage
description: Refresh the dashboard and interpret the supported provider metrics.
sidebar:
  order: 5
---

With an authenticated project and configured providers, open **Dashboard** and refresh the selected UTC month. The refresh runs through the configured providers and stores a monthly snapshot per provider. Use the month navigation to inspect an earlier month; refresh that selected month if no snapshot exists. The current month is incomplete until it ends.

[OpenAI](openai.md) shows organization costs and, when available, a spend limit from the largest monthly spend-alert threshold. It also collects completions activity. Costs can remain visible as a partial result when alerts or activity are unavailable.

[Apify](apify.md) compares discounted usage credits from its native billing cycle with your configured monthly USD allowance.

[Resend](resend.md) shows available monthly email metrics and compares sent plus received counts with your configured monthly and current-day quotas.

Use **Overview** for current-month totals and limit indicators, and **Refresh logs** for each collection attempt. The overview's approaching-limit count uses an 80% threshold. For definitions and defaults, see [configuration](../reference/configuration.md); for missing metrics, see [common issues](../troubleshooting/common-issues.md).
