---
title: Commands and local interfaces
description: Supported launcher and source-development commands.
sidebar:
  order: 2
---

The installed `clearquota` launcher starts the local web server. It has no documented command-line subcommands or flags. Set [environment variables](configuration.md) before launching it.

In a source checkout, `mise run install` fetches dependencies and builds CSS; `mise run start` checks formatting, builds CSS, and runs the server; `mise run check` checks formatting and compiles; and `mise run test` runs the Rust tests. `mise run css:build` rebuilds CSS after template or style changes. The CI workflow also runs `cargo clippy --locked -- -D warnings` and `cargo audit`.

The browser interface exposes **Overview** at `/`, **Dashboard** at `/dashboard`, **Providers** at `/providers`, **Refresh logs** at `/runlogs`, and **Settings** at `/settings`. These are local UI routes, not a supported external API. The server also mounts static assets under `/static`.
