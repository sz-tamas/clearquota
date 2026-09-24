---
title: Set up Resend
description: Store a Full access key and configure plan, monthly quota, and daily quota.
sidebar:
  order: 3
---

ClearQuota reads Resend's `/emails/metrics` endpoint. Use a **Full access** Resend API key. A **Sending access** key cannot retrieve usage statistics; ClearQuota recognizes Resend's restricted-key response and reports this explicitly. Resend's [API key permissions](https://resend.com/changelog/new-api-key-permissions) describe Full access as allowing create, delete, get, and update operations. It is **not a read-only key**, even though ClearQuota only sends GET requests for metrics.

## Add and configure

1. Create a Full access key in Resend. Store its **value** in Google Secret Manager in the active project, and grant the ClearQuota ADC identity access to that secret.
2. In **Providers**, select **Add provider → Resend**. Enter a display name and the short Secret Manager secret name, such as `RESEND_USAGE_KEY`; select **Save provider**. Do not enter the key value or a full path.
3. Select the new provider's name or chevron to expand its table row. Enter the **Plan** label and positive **Monthly quota** and **Daily quota** email counts, then select **Save settings**. These fields start empty or zero, so complete them before refreshing.
4. Open **Dashboard**, refresh the selected UTC month, and inspect the Resend card and **Refresh logs**.

## Results and failures

ClearQuota displays the monthly sent, received, delivered, bounced, complained, and failed counts returned by Resend. Monthly and current-day quota usage is **sent plus received**; the daily view appears only for the current UTC month. The plan is a display label, while your configured quotas determine the percentages and remaining counts.

If the log says `Your Resend API key has Sending access only`, replace the Secret Manager value with a **Full access** key and refresh again. If the request still fails, follow [diagnostics](../troubleshooting/diagnostics.md).
