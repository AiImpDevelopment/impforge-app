# ImpForge App

<p align="center">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="MIT" />
  <img src="https://img.shields.io/badge/built--with-Rust+Svelte-orange" alt="Rust + Svelte" />
  <img src="https://img.shields.io/badge/tauri-2.10-blue" alt="Tauri 2.10" />
  <img src="https://img.shields.io/badge/local--first-yes-brightgreen" alt="Local-first" />
</p>

<h3 align="center">
  Open-source freemium tier of <a href="https://impforge.com">ImpForge</a> — local-first AI workstation built in Tauri + Svelte + Rust, with the DigiImp animated companion.
</h3>

---

## What this is

ImpForge App (this repo) is the **MIT-licensed freemium tier** of [ImpForge](https://impforge.com), our local-first AI workstation. The Pro version (source-available, commercial) is sold at €25/month permanent.

This repo contains the **stripped-down freemium build**:
- Local Ollama chat with streaming
- SQLite FTS5 knowledge base
- Local file watch + RSS auto-digest
- 5-tier privacy architecture
- DigiImp animated companion (Heartbeat Ribbon + 16x16 pixel actor)
- Static slash-commands

The Pro version adds: 50,000-pattern intent classification, persistent cross-session memory, 3-tier knowledge fabric (SQLite+DuckDB+LanceDB), Universal Messenger Hub for Slack/Discord/Telegram/Signal/WhatsApp/Teams, 5,020 industry SaaS templates, 4-Model Pipeline orchestration, and 472 personas.

## Install

```bash
cargo install impforge-app
# or
brew install impforge/tap/impforge-app
# or
scoop install impforge-app
# or download a binary from https://github.com/AiImpDevelopment/impforge-app/releases
```

## Run

```bash
impforge-app
```

Or for development:
```bash
git clone https://github.com/AiImpDevelopment/impforge-cli.git /tmp/impforge-cli
git clone https://github.com/AiImpDevelopment/impforge-app.git
cd impforge-app
pnpm install
pnpm tauri:dev
```

## Architecture

Two-codebase MIT-rewrite model:
- **This repo** (MIT): freemium UI shell + local features + DigiImp shell
- **ImpForge Pro** (source-available, commercial): full intelligence stack (private repo)

The Pro source code is **never** in this repo — every freemium feature is a **fresh MIT-licensed rewrite** of its Pro counterpart.

See [docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md](docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md) for the full architecture.

## Why open source?

We believe local-first AI workstations should respect users:
- No telemetry. No tracking. No data leaving your machine.
- EU AI Act compliant.
- 5-tier privacy architecture (Fortress → Open).
- Funded by Pro subscribers, not VCs.

The Pro version (€25/mo permanent) funds development. There is no free trial of Pro requiring a credit card. There is no enterprise pricing tier we'll squeeze you with later. Single permanent price, forever.

## Status

- Sprint 16 — Phase 1 (Skeleton) complete
- Phase 2-8 in progress per [docs/superpowers/plans/](docs/superpowers/plans/)

## Contribute

See [CONTRIBUTING.md](CONTRIBUTING.md). We welcome:
- Templates / skills / MCP manifests (cross-published to [impforge-cli](https://github.com/AiImpDevelopment/impforge-cli))
- Bug reports + reproduction steps
- Translation contributions (English + German first)
- Documentation improvements

## License

MIT (this repo). See [LICENSE](LICENSE).

The Pro version (separate private repo) ships under a source-available commercial licence.
