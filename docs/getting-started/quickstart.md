---
title: Quickstart
description: Refresh one provider and find its first stored usage result.
sidebar:
  order: 3
---

Prerequisites: [install ClearQuota](installation.md), [authenticate Google Cloud](first-configuration.md), and complete both setup steps in the [OpenAI](../guides/openai.md), [Resend](../guides/resend.md), or [Apify](../guides/apify.md) guide. Resend needs plan quotas and Apify needs a monthly credit allowance before refresh; OpenAI's credit ledger is optional.

1. Open **Dashboard**. It starts on the current UTC calendar month.
2. Select **Refresh** to collect usage for the displayed month. ClearQuota refreshes every configured provider for that month.
3. Check the completion summary for succeeded and failed counts. A successful provider card shows a refresh time and available metrics. Open **Overview** for a cross-provider summary.
4. Open **Refresh logs** to see a recorded success or a sanitized failure message for each attempt.

If no metrics appear, confirm the selected month and provider settings. A provider may have no data for that month, or its refresh may have failed. Follow [diagnostics](../troubleshooting/diagnostics.md) before changing the Secret Manager reference.

ClearQuota saves each month's result locally. You can select an earlier month on Dashboard and refresh that month explicitly. See [monitor usage](../guides/monitor-usage.md) for the ongoing workflow.
