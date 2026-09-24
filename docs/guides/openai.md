---
title: Set up OpenAI
description: Store an organization Admin API key, add OpenAI, and use its optional credit ledger.
sidebar:
  order: 2
---

ClearQuota reads OpenAI organization costs, spend alerts, and completions usage. Its OpenAI connection requires an **organization Admin API key**; an ordinary project API key is insufficient for these organization endpoints. An organization owner can create an Admin key in [OpenAI's Admin Keys settings](https://platform.openai.com/settings/organization/admin-keys). See the [Usage API](https://platform.openai.com/docs/api-reference/usage/audio_transcriptions_object) and [Admin API key](https://platform.openai.com/docs/api-reference/admin-api-keys) references. Admin keys have broad organization privileges, so restrict access to the Google Secret Manager secret that holds the key.

## Add and configure

1. Store the **value** of the Admin API key in a secret in the active Google Cloud project. Give the ClearQuota ADC identity permission to access that secret.
2. In **Providers**, select **Add provider → OpenAI**. Enter a display name and the short Secret Manager secret name, such as `OPENAI_ADMIN_KEY`. Select **Save provider**. Do not paste the key value or a full Secret Manager path into the form.
3. Select the new provider's name or chevron in the table to expand its row. OpenAI has a **Credit ledger**, not a quota form. If you track prepaid credit, add a purchase, refund, or adjustment with a date no later than today, a positive USD amount, and an optional note. You can remove an entry later. This ledger is optional for API collection and is separate from OpenAI spend alerts.
4. Open **Dashboard** and refresh the selected UTC month. Check the OpenAI card and **Refresh logs**.

## Results and failures

ClearQuota collects organization costs and uses the largest monthly spend-alert threshold as its displayed limit. It also shows available tokens, requests, prompt-cache rate, daily trends, and project spend. The manual credit ledger contributes a net credit view on the current-month dashboard. Costs can be stored as a **partial** result if spend alerts or completions usage fail; if costs fail, no OpenAI snapshot is saved for that attempt.

If Refresh logs show `provider request failed`, first confirm that the Secret Manager value is an **Admin API key for the intended organization**, then check whether that key can access the organization endpoints. See [diagnostics](../troubleshooting/diagnostics.md) for the rest of the sequence.
