<!-- SPDX-License-Identifier: MIT -->
# Contributing to ImpForge App

Thank you for your interest in contributing.

## Code of Conduct

By participating, you agree to abide by our [Code of Conduct](CODE_OF_CONDUCT.md).

## Crown-Jewel Quality Standards (non-negotiable)

This repo enforces:
- **0 errors, 0 warnings** in `cargo check` + `cargo clippy --all-targets -- -D warnings`
- **0 production `.unwrap()`** (use `?` + `AppError`)
- **0 `#[allow(*)]`** attributes (wire dead code into real callers, fix root causes)
- **100% MIT SPDX headers** on every source file
- **No Pro-source identifiers** — Crown-Jewel Dim-9 (`scripts/cj-dim9-no-pro-leak.sh`)

Pre-commit hooks enforce these. Don't bypass with `--no-verify`.

## Development Setup

```bash
git clone https://github.com/AiImpDevelopment/impforge-cli.git /tmp/impforge-cli
git clone https://github.com/AiImpDevelopment/impforge-app.git
cd impforge-app
pnpm install
pnpm tauri:dev
```

## TDD Discipline

Every new feature follows: **failing test → minimum implementation → passing test → commit**.

## License

By contributing, you agree your contributions are MIT-licensed.
