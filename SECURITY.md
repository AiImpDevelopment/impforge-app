<!-- SPDX-License-Identifier: MIT -->
# Security Policy

## Reporting a vulnerability

Please email **security@impforge.com** with:
- Description
- Steps to reproduce
- Affected versions

We commit to:
- Acknowledge within 48 hours
- Patch critical issues within 14 days
- Credit you in the release notes (unless you prefer anonymity)

Do **not** open public GitHub issues for security vulnerabilities.

## Threat Model

This repo is the MIT freemium tier. The Pro tier has additional defenses (Quarantine Layer, Sigstore + SLSA L3 + in-toto).

See [docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md §7](docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md) for the full threat model.

## Supported Versions

We support the latest minor release. Older versions get security backports for 6 months.
