# Maintaining the ClearQuota documentation

Review documentation in the same pull request as every application change. Update affected pages in place; preserve unrelated content, filenames, and links unless a structural change is necessary. The code, tests, templates, migrations, and build configuration are the source of truth. Do not regenerate the whole directory from an old outline.

## Impact checklist

- Installation, platform support, prerequisites, release packaging: `getting-started/installation.md`, `reference/commands.md`.
- Commands, routes, arguments, environment variables, defaults: `reference/commands.md`, `reference/configuration.md`, and relevant getting-started examples.
- Provider capabilities, key requirements, settings, validation, metrics, or UI workflow: `getting-started/first-configuration.md`, `getting-started/quickstart.md`, the relevant `guides/openai.md`, `guides/resend.md`, or `guides/apify.md`, `guides/monitor-usage.md`, and `reference/configuration.md`.
- Errors, authentication, and diagnostics: `troubleshooting/common-issues.md`, `troubleshooting/diagnostics.md`.
- Database, credential handling, month history, deletion, or other operational behavior: `guides/history-and-data.md`, `reference/architecture.md`, and security claims in `index.md`.

## Source map

| Source | Documentation to review |
| --- | --- |
| `scripts/install.sh`, `.github/workflows/release.yml`, `mise.toml`, `src/config.rs`, `src/main.rs` | Installation, commands, configuration |
| `src/router/onboarding.rs`, `src/router/account.rs`, `src/secrets/`, `src/utils/helper.rs` | First configuration, configuration reference, authentication troubleshooting |
| `src/router/providers.rs`, `src/providers/`, `src/models.rs`, provider tests | Quickstart, provider key requirements and setup, configuration, common issues |
| `src/router/dashboard.rs`, `src/router/runlogs.rs`, `templates/` | Monitor usage, history, diagnostics |
| `src/database.rs`, `migrations/`, `src/router/account.rs` | History and local data, architecture |

Validate frontmatter and relative Markdown links after edits, then run `mise run check` and `mise run test` for code changes. When the future Starlight site is added, include its build as a lightweight docs check. Do not add a complex generation pipeline or scheduled AI regeneration.
