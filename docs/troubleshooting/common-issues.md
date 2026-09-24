---
title: Common issues
description: Diagnose visible startup, authentication, secret, and provider-refresh failures.
sidebar:
  order: 1
---

## ClearQuota does not start

If the installer says `gcloud is required`, install the Google Cloud CLI and rerun it. If the binary reports `USAGE_DASH_PORT must be an integer from 1 through 65535`, set a valid port or unset the variable. If binding fails, choose an unused local port with `USAGE_DASH_PORT`. For a source build, use the [installation steps](../getting-started/installation.md); mise selects the Tailwind binary for the current operating system and architecture.

## Google authentication is unavailable

The dashboard may say `Google Application Default Credentials are unavailable`. Complete the browser sign-in started by **Authenticate with Google**, then use the authentication check. The collector checks `gcloud auth application-default print-access-token`, so a separate `gcloud auth login` session alone does not establish the ADC identity it needs. Reopen the dashboard or try the refresh after successful verification.

## The secret cannot be read

For `Secret name or Secret Manager reference is invalid for the active Google project`, edit the provider and use a valid short secret name. For `Google Cloud could not access this Secret Manager secret`, confirm that the ADC identity has **Secret Manager Secret Accessor** on that secret and that the Secret Manager API is enabled. Also confirm the secret version exists. See [reference syntax](../reference/configuration.md).

## The provider refresh fails

Check **Refresh logs** for the provider and month. `provider request failed` means the API request did not succeed; verify network access and the provider key's validity and permissions. `provider returned an invalid usage response` means the response could not be parsed as expected. For the required key and provider-specific failure, use the [OpenAI](../guides/openai.md), [Resend](../guides/resend.md), or [Apify](../guides/apify.md) guide. Continue with [diagnostics](diagnostics.md) if the cause is still unclear.
