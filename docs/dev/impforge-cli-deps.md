<!-- SPDX-License-Identifier: MIT -->
# impforge-cli workspace dependency notes

This repo (impforge-app) consumes 9 crates from `impforge-cli`:

- impforge-core
- impforge-models
- impforge-mcp-server
- impforge-emergence
- impforge-autonomy
- impforge-crown-jewel
- impforge-universal
- impforge-bench
- impforge-remote

For dev work, clone impforge-cli alongside:
```bash
git clone https://github.com/AiImpDevelopment/impforge-cli.git /tmp/impforge-cli
ln -sf /tmp/impforge-cli /opt/ork-station/impforge-cli  # path-dep resolution
```

For CI, the cli is checked out via `actions/checkout` of both repos
(see `.github/workflows/ci.yml` Task 6).

When the cli publishes to crates.io (post Phase 7), update `Cargo.toml`
workspace deps to use `version = "0.x"` instead of `path = "../impforge-cli/crates/..."`.
