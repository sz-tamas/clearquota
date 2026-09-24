---
title: Diagnostics
description: A safe sequence for investigating a missing or partial usage result.
sidebar:
  order: 2
---

1. On **Dashboard**, check the selected UTC month and refresh it. The completion summary reports succeeded and failed providers.
2. Open **Refresh logs** and find the provider's most recent attempt. Its status and message identify the action that failed without exposing the key or provider response body.
3. In **Providers**, check the display name, active project, and secret name. For Apify or Resend, check the configured allowance or quotas.
4. If authentication failed, redo the Google ADC flow in the dashboard. If Secret Manager access failed, inspect the ADC identity's grant and whether the API is enabled. If the provider request failed, check that the stored secret contains an appropriate provider key and that the provider account permits the usage endpoint.
5. Refresh again, then inspect the card and log. A successful OpenAI refresh may still have a partial snapshot when its alerts or activity endpoint fails.

Avoid pasting API keys, access tokens, Secret Manager payloads, or full provider responses into logs or issue reports. The terminal may show endpoint paths, HTTP status, and normalized collection events, which are useful for correlating a failure. For specific messages, see [common issues](common-issues.md).
