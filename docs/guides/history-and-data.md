---
title: Review history and local data
description: Inspect prior months and control the SQLite data retained on your computer.
sidebar:
  order: 6
---

Use the month controls on **Dashboard** to move backward and forward through UTC calendar months; future months are unavailable. Each month has its own stored snapshot. A refresh updates the selected month's result without deleting older months. **Refresh logs** lists sanitized results of collection attempts.

Open **Settings → Local data & privacy** to see the database path, database size including SQLite support files, snapshot count, and log count. The actions there have different effects:

| Action | Removes | Keeps |
| --- | --- | --- |
| Clear usage history | Snapshots, monthly summaries, OpenAI cost and activity records | Project, providers, manual credit events, refresh logs |
| Clear refresh logs | Refresh logs | Project, providers, usage history |
| Erase all local app data | Project, providers, usage, credit events, logs, onboarding state | Secrets in Google Secret Manager |

These actions are permanent for the local database. The erase action requires typing `ERASE`. Clearing records may not immediately shrink the SQLite file because SQLite can reuse its free space. See [architecture](../reference/architecture.md) for the storage boundary.
