---
title: Installation
description: Install a release or build ClearQuota from source.
sidebar:
  order: 1
---

## Install a release

The release installer supports macOS and Linux on x86-64 and ARM64. Install the [Google Cloud CLI](https://cloud.google.com/sdk/docs/install) first. You also need access to a Google Cloud project and its Secret Manager secrets. The installer requires `curl`, `tar`, and either `sha256sum` or `shasum`.

```sh
curl -fsSL https://clearquota.app/install | sh
exec "$SHELL"
clearquota
```

Open `http://127.0.0.1:3000`. The installer checks the release archive against published SHA256 sums, puts the application under `~/.local/share/clearquota` by default, and creates a launcher in `~/.local/bin`. It adds that bin directory to the startup file for zsh, bash, or fish. The launcher changes to the installation directory before starting ClearQuota, so the default SQLite database lives there under `data/clearquota.sqlite3`.

## Build from source

On macOS, install [mise](https://mise.jdx.dev/), the Google Cloud CLI, and a Rust toolchain through mise. In a repository checkout, run:

```sh
mise run install
mise run start
```

Open `http://127.0.0.1:5050`. `mise run install` fetches Rust dependencies, installs the standalone Tailwind CLI selected for the current operating system and architecture by mise, and builds CSS. `mise run start` checks formatting, rebuilds CSS, and runs the Rust server. Tailwind is installed in mise's tool directory rather than saved in the source checkout.

Next, [configure your project and first provider](first-configuration.md). See [configuration](../reference/configuration.md) to change the local port or database location.
