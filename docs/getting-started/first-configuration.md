---
title: First configuration
description: Connect Google Cloud and add a supported provider without storing its API key in ClearQuota.
sidebar:
  order: 2
---

Before opening the dashboard, choose a provider guide: [OpenAI](../guides/openai.md), [Resend](../guides/resend.md), or [Apify](../guides/apify.md). Each explains the key permission it needs and how to add and configure its row. Store the provider key **value** in Google Secret Manager, and grant your Google Application Default Credentials (ADC) identity access to that secret. Give ClearQuota only the secret's short name. Never enter the key value in the provider form.

1. Start ClearQuota and open the local address shown in the terminal.
2. In the first-run flow, enter the Google Cloud project ID or numeric project number that contains the secret. Choose **Authenticate with Google**. ClearQuota starts `gcloud auth application-default login` for that project.
3. Complete the Google sign-in, then use the dashboard's authentication check. If no browser window opens when running ClearQuota in WSL, follow the [WSL terminal sign-in steps](../troubleshooting/common-issues.md#google-authentication-is-unavailable) and then return to the authentication check. If the check fails, see [authentication problems](../troubleshooting/common-issues.md).
4. Follow the [OpenAI](../guides/openai.md), [Resend](../guides/resend.md), or [Apify](../guides/apify.md) guide: add a provider with its display name and secret name, then expand its new row in the Providers table to enter provider-specific settings.

The secret name is resolved against the active project with version `latest`. Although the collector can normalize existing same-project references, the current add and edit forms accept only short secret names. The provider form does not test the provider key; a refresh reveals access or API problems. Complete both provider setup steps before the [quickstart](quickstart.md).
