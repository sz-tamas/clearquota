---
title: Set up Apify
description: Store an API token that can read account usage and configure the monthly credit allowance.
sidebar:
  order: 4
---

ClearQuota calls Apify's [monthly usage endpoint](https://docs.apify.com/api/v2/users-me-usage-monthly-get) with a bearer token. Create a token in Apify Console's **API & Integrations** area for the account whose usage you want to see. The token must be able to read that account's monthly usage. Apify [supports scoped API tokens](https://docs.apify.com/integrations/api); the published endpoint documentation lists **403 insufficient permissions**, but does not name a specific minimum scope for this call. A read-only scoped token is suitable **if it can access this endpoint**. A broader token may work, but ClearQuota itself only makes a GET request.

## Add and configure

1. Store the Apify token **value** in Google Secret Manager in the active project, and grant the ClearQuota ADC identity access to that secret.
2. In **Providers**, select **Add provider → Apify**. Enter a display name and the short Secret Manager secret name, such as `APIFY_USAGE_TOKEN`; select **Save provider**. Do not enter the token value or a full path.
3. Select the new provider's name or chevron to expand its table row. Enter your plan's positive **Monthly credit allowance (USD)** and select **Save settings**. The stored allowance starts at zero; set it before refreshing.
4. Open **Dashboard**, refresh the selected UTC month, and inspect the Apify card and **Refresh logs**.

## Results and failures

ClearQuota uses Apify's `totalUsageCreditsUsdAfterVolumeDiscount` as usage. It compares that amount with your allowance to calculate remaining credit and percentage used. Apify returns its **native billing cycle** for the queried date; a historical month selection queries a date at that month's end. This is not a reconstructed UTC calendar-month sum.

If Refresh logs show `provider request failed`, check that the token belongs to the intended account and is permitted to access monthly usage. A 401 or 403 from Apify appears in ClearQuota as a generic provider request failure. See [diagnostics](../troubleshooting/diagnostics.md).
