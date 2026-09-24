---
title: Architecture
description: How ClearQuota collects metrics and keeps provider credentials out of local storage.
sidebar:
  order: 3
---

ClearQuota is a Rust Axum server bound to localhost. Askama renders full pages and HTMX HTML fragments. SQLite holds project and provider metadata, sanitized monthly snapshots, OpenAI cost and activity records, manual credit events, and refresh logs. The database is migrated on startup.

During refresh, the server loads a provider's metadata, resolves its exact Google Secret Manager reference with Google Application Default Credentials, calls the provider API, normalizes the response, and saves only the resulting usage data. The credential value is held transiently in `secrecy::SecretString`; it is never stored in SQLite or returned to the browser. Google authentication uses `gcloud auth application-default login`, and the same ADC identity supplies the access token for Secret Manager.

Provider adapters live in `src/providers/`; Google secret resolution is in `src/secrets/`; `src/router/` contains browser routes; `src/database.rs` owns SQLite operations; and `templates/` contains the rendered UI. There is no automatic scheduler or hosted backend. For retained data and deletion controls, see [local data](../guides/history-and-data.md).
